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
//!
//! ## Per-key audit map (Phase G, section 12)
//!
//! Every element key [`project_section`] emits, what handles
//! it, and what that handler actually does — each row checked
//! against the core source, not against the label's promise:
//!
//! | key | control | handler → action | verdict |
//! |---|---|---|---|
//! | `project-tools` | section root (col) | — container, no handler | ok |
//! | *(title row, unkeyed)* | "Project" title + autosave text | renders `autosave_line(core.edits_since_autosave())` (editor.rs:1599) | ok — counter is the core's; see the autosave note on [`autosave_line`] |
//! | `proj-name-field` | name-field wrapper (el) | — container, no handler | ok |
//! | `proj-name` | text input | `on_edit` → `&mut State::name_edit`; `on_submit` → [`apply_name`] | ok — Enter and "Set name" share one path |
//! | `proj-set-name` | "Set name" button (`icon_btn`, "check" icon) | [`apply_name`] → [`apply_project_name`]: trims, writes `Project::name` directly (no core rename op or setter exists — editor.rs has no `rename*` fn; the file-level rename is Home's `projects::rename_project`) | ok |
//! | `proj-current-file` | "Current file: …" muted line under the name field (conditional — present only when the line resolves) | renders [`current_file_line`]: the `shell::last_opened_project` display name, shown only when it exactly matches `Project::name` | ok — Phase I §31; the match guard is the Phase G §32 lesson (recents store names, not the open file, so a mismatch would lie). `home_panel::current_project_path` was NOT used: it is the selector cursor, which `cycle-project` moves without opening |
//! | `proj-current-meta` | "{size} · modified {time}" muted line under `proj-current-file` (conditional — present only when the §31 match holds AND the file resolves and stats) | renders [`current_file_meta`]: the §31 match (via [`current_file_path`]) gates it, the path resolves over `core.project_files` by the crate's slug rule ([`resolve_project_file`]), size via [`human_size`], time via [`modified_label`] | ok — Phase J §27; gated on the §31 decision itself, so the two lines can never disagree about which file the session is on |
//! | `new-proj` | "New" button (`icon_btn`, "file" icon) | [`new_pressed`] → [`new_project`] + [`sync_name_edit`]: installs [`blank_project`]`("Untitled")` — field-for-field the MCP `new_project` tool's defaults and reset set (mcp_tools.rs `tool_new_project`) — clears solo, stops playback, no snapshot/confirm | ok |
//! | `save-proj` | "Save project" button (`icon_btn`, "floppy-disk" icon) | [`save_named_pressed`] → `EditorCore::save_named_project` (slugifies the name, writes `./projects/<slug>.json`, refreshes `project_files`, sets status), then — only on success — `shell::write_recovery` + `shell::touch_recent` to keep snapshot/recents in step | fixed in Phase H — the Phase G caveat is closed: the op returns `bool` and [`after_save`] gates the recovery/recents sync on it, so a failed save no longer plants a phantom recent |
//! | `recover` | "Recover" button (`icon_btn`, "arrow-rotate-left" icon) | [`recover_pressed`] → [`recover_snapshot`] (`shell::read_recovery` = `./projects/.recovery.json`, shell.rs) → [`apply_recovered_project`] + [`sync_name_edit`] | fixed in Phase G — the reset now also clears solo and stops playback (see the fn) |
//! | `proj-list` | selector list (col) | — container, no handler | ok |
//! | `open-proj-{i}` | "Open" button per row (plain text `btn` — list rows stay text; no icon), accent-styled when `i == core.project_index` | [`open_row_pressed`] → sets `project_index`, `EditorCore::open_project_file` (editor.rs:3636: reads the file; resets current/playhead/figure/node; sets status), [`clear_stale_solo`], [`sync_name_edit`] | fixed in Phase G — stale-solo guard added; the highlight tracks the core's cycle cursor (the palette's `cycle-project` moves it without opening), exactly as [`ProjectRow`] documents |
//! | *(empty-list text, unkeyed)* | "No saved projects in ./projects/ yet." | — display only; wording is `cycle_project_file`'s own status string (editor.rs), verbatim | ok |
//!
//! Name-field sync is not duplicated work: the handlers above
//! call [`sync_name_edit`] for an immediate refresh after their
//! own switches, and the integrator's per-tick `poll()` call
//! covers switches made elsewhere (app-bar Load, Home open,
//! MCP rename). The fn is a no-op unless the text differs, so
//! the overlap costs one string compare.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use rune::tok::{type_, Theme};
use rune::{col, el, row, text, text_input, El, Tx};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::{Easing, FigureDef, FigureInstance, Frame, Project};

use crate::chrome_view::btn;
use crate::rune_sec::icon_btn::icon_btn;
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

/// The selector row's "Open" button key for row `i`
/// (`open-proj-{i}`). One definition, used by the builder, so
/// the audit map in the module docs and the emitted keys
/// cannot drift apart on the dynamic pattern.
pub fn open_row_key(i: usize) -> String {
    format!("open-proj-{i}")
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
/// have accumulated since the last autosave
/// (`EditorCore::edits_since_autosave` — edits minus the count
/// at the last successful `maybe_autosave`). Display only.
///
/// Phase G audit note, so the line is not over-read: the
/// counter is the core's and this section renders it
/// faithfully, but (a) the cadence behind it is the shell's:
/// `poll()` in main.rs calls `maybe_autosave` every tick
/// (wired in the Phase G integration), so the snapshot
/// refreshes every 25 counted edits; and
/// (b) a recovery write that bypasses `maybe_autosave` —
/// "Save project" writes one via `shell::write_recovery` —
/// does not reset the counter (the core keeps it private and
/// exposes no "note a manual write" API), so right after a
/// save the line can overstate how stale the snapshot is.
pub fn autosave_line(edits_since_autosave: u64) -> String {
    match edits_since_autosave {
        0 => "Recovery snapshot up to date.".to_string(),
        1 => "1 edit since the last recovery snapshot.".to_string(),
        n => format!("{n} edits since the last recovery snapshot."),
    }
}

/// The "Current file: …" line's decision, factored out of
/// [`current_file_line`] so it is testable without touching
/// the disk: `last_opened` is the display name
/// `shell::last_opened_project` returns (the front of the
/// recents list — recents store names, not paths), and
/// `project_name` is the open project's `Project::name`.
///
/// The line appears only on an exact, non-empty match.
/// `last_opened` names the most recent project the shell
/// recorded, which is not necessarily the one now open —
/// the user may have pressed New since, or be working on a
/// project whose save never landed in recents — so showing
/// it on a mismatch would name a file this session is not
/// working on. That is the Phase G §32 lesson: home_panel's
/// reopen target resolves the same recents name by match,
/// never by assumption.
pub fn file_line(last_opened: Option<String>, project_name: &str) -> Option<String> {
    let name = last_opened?;
    if name.is_empty() || name != project_name {
        return None;
    }
    Some(format!("Current file: {name}"))
}

/// The current-file line for the open project, or `None`
/// when the shell's last-opened record does not name this
/// project (see [`file_line`] for the match rule).
///
/// Data source: `shell::last_opened_project`. The core
/// keeps no loaded-path field, and the one path accessor
/// that exists — `home_panel::current_project_path`
/// (`project_files[project_index]`) — is the selector
/// cursor, which the palette's `cycle-project` moves
/// without opening anything, so it can point at a file
/// that was never opened. The recents name plus the exact
/// match is the only source that cannot lie about which
/// file this session last opened or saved.
pub fn current_file_line(core: &EditorCore) -> Option<String> {
    file_line(stykanimate_rs::shell::last_opened_project(), &core.project.name)
}

/// The file-name slug for a project name: the crate-wide
/// naming convention (`editor.rs::save_named_project`,
/// `projects::save`, `shell::project_file_slug`) — the name
/// lowercased with every non-alphanumeric character replaced
/// by `-`, giving the project's file as `<slug>.json` in the
/// projects directory. Duplicated here because every owner
/// keeps its copy private; keep the rule in sync with them.
fn project_file_slug(name: &str) -> String {
    name.to_lowercase()
        .replace(|c: char| !c.is_alphanumeric(), "-")
}

/// Resolve a project name to its file within a list of
/// project paths (the core's `project_files`): the entry
/// whose file stem is the name's slug — the exact file a
/// named save of this project writes and a named open
/// reads. This is the name→path mapping the selector rows
/// already imply (their labels are these stems) and the
/// one `shell` uses for its recents bookkeeping. The core's
/// list is pre-filtered by `EditorCore::list_project_files`,
/// so shell-owned entries (recovery, recents, layout) can
/// never be returned. `None` when no entry matches — the
/// file was deleted, renamed, or never saved.
pub fn resolve_project_file<'a>(name: &str, files: &'a [PathBuf]) -> Option<&'a PathBuf> {
    let slug = project_file_slug(name);
    files
        .iter()
        .find(|path| path.file_stem().and_then(|s| s.to_str()) == Some(slug.as_str()))
}

/// The current project's file path, gated on the §31 match:
/// the shell's last-opened record must name this project
/// (the [`file_line`] decision itself, so the meta line can
/// never appear under a "Current file" line that resolved
/// to a different name), and the name must resolve to a
/// file in the core's list (see [`resolve_project_file`]).
/// Factored out of [`current_file_meta`] so the gating is
/// testable without touching the disk.
pub fn current_file_path<'a>(
    last_opened: Option<String>,
    project_name: &str,
    files: &'a [PathBuf],
) -> Option<&'a PathBuf> {
    file_line(last_opened, project_name)?;
    resolve_project_file(project_name, files)
}

/// A byte count humanised for the meta line: plain bytes
/// under a thousand, then decimal KB and MB with one
/// decimal place ("999 B", "1.0 KB", "2.5 MB"). Decimal
/// units keep the boundaries round — a file the editor
/// reports as 1,000 bytes is "1.0 KB" here. Sizes beyond
/// the MB range keep growing in MB; project JSON does not
/// reach a scale where a GB unit would read better.
pub fn human_size(bytes: u64) -> String {
    if bytes < 1_000 {
        format!("{bytes} B")
    } else if bytes < 1_000_000 {
        format!("{:.1} KB", bytes as f64 / 1_000.0)
    } else {
        format!("{:.1} MB", bytes as f64 / 1_000_000.0)
    }
}

/// The "modified {time}" part of the meta line: the
/// wall-clock "HH:MM" when the file was modified on the
/// same calendar day as `now`, the "YYYY-MM-DD" date
/// otherwise. Both stamps come from
/// `home_panel::format_modified` — the tree's existing
/// UTC civil-from-unix conversion — so this fn adds no
/// date maths of its own; it only chooses which half of
/// that helper's "YYYY-MM-DD HH:MM" output to show (the
/// split is at that format's single space, which the
/// helper pins by test). "Today" is the UTC calendar
/// day, the convention every timestamp in this codebase
/// is rendered in.
pub fn modified_label(modified: SystemTime, now: SystemTime) -> String {
    let stamp = crate::rune_sec::home_panel::format_modified(Some(modified));
    let today = crate::rune_sec::home_panel::format_modified(Some(now));
    let (date, time) = stamp
        .split_once(' ')
        .expect("format_modified always renders \"YYYY-MM-DD HH:MM\"");
    let (today_date, _) = today
        .split_once(' ')
        .expect("format_modified always renders \"YYYY-MM-DD HH:MM\"");
    if date == today_date {
        time.to_string()
    } else {
        date.to_string()
    }
}

/// The meta line's text: "{size} · modified {time}" (see
/// [`human_size`] and [`modified_label`]). Pure — the
/// caller supplies the stat results and the clock — so
/// the whole format is testable without a filesystem.
pub fn meta_text(size_bytes: u64, modified: SystemTime, now: SystemTime) -> String {
    format!(
        "{} · modified {}",
        human_size(size_bytes),
        modified_label(modified, now)
    )
}

/// The current-file metadata line for the open project —
/// "{size} · modified {time}" for the file the §31 line
/// names — or `None` when there is nothing honest to show:
/// the §31 match does not hold, the name resolves to no
/// file in the core's list, or the file cannot be stat'ed
/// (deleted between the list refresh and this render). As
/// with the §31 line, an absent line beats a wrong one.
pub fn current_file_meta(core: &EditorCore) -> Option<String> {
    let path = current_file_path(
        stykanimate_rs::shell::last_opened_project(),
        &core.project.name,
        &core.project_files,
    )?;
    let metadata = std::fs::metadata(path).ok()?;
    let modified = metadata.modified().ok()?;
    Some(meta_text(metadata.len(), modified, SystemTime::now()))
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
/// returned a snapshot (app.rs, pre-removal) — plus, since the
/// Phase G audit, the solo/playback clearing this file's own
/// [`new_project`] already did for the same kind of swap:
/// `layers::visible_figures` hides EVERY figure when `solo`
/// names an index the swapped-in project does not have, so a
/// stale solo would blank the stage with no explanation.
pub fn apply_recovered_project(core: &mut EditorCore, project: Project) {
    core.project = project;
    core.current = 0;
    core.playhead = 0.0;
    core.selected_figure = 0;
    core.selected_node = None;
    core.solo = None;
    core.playing = false;
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

/// The follow-up work a named save earns, decided by
/// [`after_save`] from the save op's success report. Pure
/// and factored out of the press handler so the gating is
/// testable without touching the disk.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AfterSave {
    /// The file was written: refresh the recovery snapshot
    /// and add the project to the recents list.
    SyncRecoveryAndRecents,
    /// The save failed: do neither. A failed save must not
    /// plant a phantom name in recents (no file backs it),
    /// and the op's own failure status stays on screen —
    /// nothing here overwrites it.
    Nothing,
}

/// Decide the follow-up work for a named save. The op
/// reports success as `bool` (Phase H); before that it
/// returned `()` and the sync ran unconditionally — the
/// Phase G audit's phantom-recent caveat.
pub fn after_save(saved: bool) -> AfterSave {
    if saved {
        AfterSave::SyncRecoveryAndRecents
    } else {
        AfterSave::Nothing
    }
}

/// "Save project": the GPUI `save-proj` listener — save
/// under the project's name in `./projects/`, then keep the
/// recovery snapshot and recents list in step with the
/// explicit save. Phase H gates those follow-ups on the
/// op's success report ([`after_save`]): on failure the
/// op's status ("Project save failed: …") is the last word
/// and recents stays untouched.
fn save_named_pressed(s: &mut State) {
    let saved = s.core.save_named_project();
    if after_save(saved) == AfterSave::SyncRecoveryAndRecents {
        let _ = stykanimate_rs::shell::write_recovery(&s.core.project);
        stykanimate_rs::shell::touch_recent(&s.core.project.name);
    }
}

/// Drop a solo index the current project does not have
/// (Phase G audit). `open_project_file` leaves `solo`
/// untouched, and `layers::visible_figures` hides EVERY
/// figure when the solo index names none of them — opening a
/// smaller project while soloing a late figure would blank
/// the stage with no explanation. (The core's dialog-open op
/// clears solo outright; the shared fix belongs in
/// `open_project_file` itself — integrator item. This guard
/// is the panel-local equivalent: it fires only when the
/// solo is actually stale, so it is a no-op when the open
/// failed and the project never changed, and it keeps a
/// solo that still names a figure in the opened project.)
pub fn clear_stale_solo(core: &mut EditorCore) {
    if let Some(solo) = core.solo {
        if solo >= core.project.figures.len() {
            core.solo = None;
        }
    }
}

/// One selector row's "Open": point the core's index at the
/// row and open that file (`open_project_file` reads
/// `project_index`), clear any solo the opened project made
/// stale, then re-sync the name field.
fn open_row_pressed(s: &mut State, i: usize) {
    s.core.project_index = i;
    s.core.open_project_file();
    clear_stale_solo(&mut s.core);
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

/// The Project section: name field + Set name, the
/// current-file line and its metadata line when the
/// shell's last-opened record names this project,
/// New / Save project / Recover, the autosave line, and
/// one Open row per saved project in the core's file list.
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
                .child(icon_btn("proj-set-name", "check", "Set name", false, t, apply_name)),
        );

    // The current-file line sits directly under the name
    // field, and only when the shell's last-opened record
    // names this project (see current_file_line) — a session
    // that never opened or saved a matching file shows
    // nothing rather than another project's name.
    if let Some(line) = current_file_line(core) {
        section = section.child(
            text(line)
                .key("proj-current-file")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    }

    // The metadata line sits directly under the current-file
    // line: the resolved file's size and modification time.
    // It is gated on the same §31 match (current_file_meta
    // re-runs that decision), so it appears exactly when the
    // line above it does and its file stats resolve — never
    // under another project's name.
    if let Some(meta) = current_file_meta(core) {
        section = section.child(
            text(meta)
                .key("proj-current-meta")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    }

    section = section.child(
        row()
            .w_full()
            .gap(8.0)
            .center_y()
            .child(icon_btn("new-proj", "file", "New", false, t, new_pressed))
            .child(icon_btn("save-proj", "floppy-disk", "Save project", false, t, save_named_pressed))
            .child(icon_btn("recover", "arrow-rotate-left", "Recover", false, t, recover_pressed)),
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
                        &open_row_key(i),
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
stroke_selected: None,
particle_selected: None,
guides_frustum: true,
guides_safe: true,
onion_ui: Default::default(),
last_frame: 0,
pose_rename: crate::rune_sec::pose_tools::PoseRenameState::new(),
pose_library: crate::rune_sec::pose_tools::PoseLibraryState::new(),
loop_playback: true,
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
        core.solo = Some(0);
        core.playing = true;
        let mut snapshot = Project::new_demo();
        snapshot.name = "Recovered Work".into();
        apply_recovered_project(&mut core, snapshot);
        assert_eq!(core.project.name, "Recovered Work");
        assert_eq!(core.current, 0);
        assert_eq!(core.playhead, 0.0);
        // Phase G: the swap also clears solo and stops
        // playback, like new_project in this file — a stale
        // solo would hide every figure in the snapshot.
        assert!(core.solo.is_none());
        assert!(!core.playing);
        assert!(core.status.contains("Recovered the snapshot"));
    }

    #[test]
    fn stale_solo_is_cleared_and_valid_solo_kept() {
        let mut core = EditorCore::new();
        // The demo project has one figure: index 0 is the
        // only solo that can still name a figure.
        assert_eq!(core.project.figures.len(), 1);
        core.solo = Some(9);
        clear_stale_solo(&mut core);
        assert!(core.solo.is_none(), "stale solo cleared");
        core.solo = Some(0);
        clear_stale_solo(&mut core);
        assert_eq!(core.solo, Some(0), "valid solo kept");
        core.solo = None;
        clear_stale_solo(&mut core);
        assert!(core.solo.is_none(), "no solo stays no solo");
    }

    #[test]
    fn open_row_key_matches_the_audited_pattern() {
        assert_eq!(open_row_key(0), "open-proj-0");
        assert_eq!(open_row_key(12), "open-proj-12");
    }

    #[test]
    fn file_line_shows_an_exact_name_match() {
        assert_eq!(
            file_line(Some("My Film".to_string()), "My Film"),
            Some("Current file: My Film".to_string())
        );
    }

    #[test]
    fn file_line_absent_without_a_last_opened_name() {
        assert_eq!(file_line(None, "My Film"), None);
    }

    #[test]
    fn file_line_absent_when_the_name_differs() {
        // The Phase G §32 guard: recents name the most recent
        // project the shell recorded, not necessarily the open
        // one — after New, the open project is "Untitled" while
        // recents still front the previous film, and showing
        // that name would claim a file this session is not on.
        assert_eq!(file_line(Some("Other Film".to_string()), "Untitled"), None);
        // Case is part of the name; there is no fuzzy match.
        assert_eq!(file_line(Some("my film".to_string()), "My Film"), None);
        // An empty record never renders a bare "Current file: ".
        assert_eq!(file_line(Some(String::new()), ""), None);
    }

    #[test]
    fn human_size_uses_decimal_units_with_one_decimal() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(1), "1 B");
        assert_eq!(human_size(999), "999 B");
        // The KB boundary: 1,000 bytes is the first KB value.
        assert_eq!(human_size(1_000), "1.0 KB");
        assert_eq!(human_size(1_500), "1.5 KB");
        assert_eq!(human_size(999_500), "999.5 KB");
        // The MB boundary, and one decimal within MB.
        assert_eq!(human_size(1_000_000), "1.0 MB");
        assert_eq!(human_size(2_500_000), "2.5 MB");
        assert_eq!(human_size(1_234_567), "1.2 MB");
    }

    #[test]
    fn resolve_project_file_matches_the_saved_slug() {
        let files = vec![
            PathBuf::from("./projects/alpha.json"),
            PathBuf::from("./projects/my-film.json"),
        ];
        // The named save of "My Film" writes my-film.json —
        // the resolution lands on exactly that entry.
        assert_eq!(
            resolve_project_file("My Film", &files),
            Some(&files[1])
        );
        assert_eq!(resolve_project_file("Alpha", &files), Some(&files[0]));
    }

    #[test]
    fn resolve_project_file_absent_without_a_slug_match() {
        let files = vec![PathBuf::from("./projects/alpha.json")];
        // A project whose file is gone (or never saved) does
        // not resolve to another project's file.
        assert_eq!(resolve_project_file("Other Film", &files), None);
        assert_eq!(resolve_project_file("Alpha", &[]), None);
    }

    #[test]
    fn file_meta_present_exactly_when_the_31_match_holds() {
        let files = vec![
            PathBuf::from("./projects/alpha.json"),
            PathBuf::from("./projects/my-film.json"),
        ];
        // Match + resolvable file: the meta line's path.
        assert_eq!(
            current_file_path(Some("My Film".to_string()), "My Film", &files),
            Some(&files[1])
        );
        // The §31 guards gate the meta line identically:
        // no last-opened record, a differing name, and an
        // empty record all yield no meta, match or not.
        assert_eq!(current_file_path(None, "My Film", &files), None);
        assert_eq!(
            current_file_path(Some("Other Film".to_string()), "Untitled", &files),
            None
        );
        assert_eq!(current_file_path(Some(String::new()), "", &files), None);
        // A match whose file is no longer in the list (deleted
        // or renamed since) also yields no meta — the line
        // never stats a file the session cannot name.
        assert_eq!(
            current_file_path(Some("Ghost".to_string()), "Ghost", &files),
            None
        );
    }

    #[test]
    fn modified_label_shows_clock_today_and_date_before() {
        let at = |secs: u64| SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(secs);
        // 1_794_117_426 is 2026-11-08 05:57 UTC (the value
        // home_panel's format_modified tests pin).
        let modified = at(1_794_117_426);
        // Later the same UTC day: only the clock shows.
        assert_eq!(modified_label(modified, at(1_794_117_426 + 10 * 3_600)), "05:57");
        // The next day: only the date shows.
        assert_eq!(
            modified_label(modified, at(1_794_117_426 + 86_400)),
            "2026-11-08"
        );
        // The epoch edge: both stamps at day zero are "today".
        assert_eq!(modified_label(at(0), at(0)), "00:00");
    }

    #[test]
    fn meta_text_combines_size_and_modified() {
        let at = |secs: u64| SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(secs);
        let modified = at(1_794_117_426);
        assert_eq!(
            meta_text(1_500, modified, at(1_794_117_426 + 60)),
            "1.5 KB · modified 05:57"
        );
        assert_eq!(
            meta_text(999, modified, at(1_794_117_426 + 86_400)),
            "999 B · modified 2026-11-08"
        );
    }

    #[test]
    fn after_save_gates_the_recovery_and_recents_sync() {
        // A written file earns both follow-ups.
        assert_eq!(after_save(true), AfterSave::SyncRecoveryAndRecents);
        // A failed save earns neither: no recovery refresh,
        // and above all no phantom name in recents — the
        // Phase G audit caveat this gating closes. The
        // handler runs the sync only on the first arm, so
        // this decision is the whole behaviour.
        assert_eq!(after_save(false), AfterSave::Nothing);
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
