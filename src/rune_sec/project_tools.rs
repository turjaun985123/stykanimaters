//! Project tools section (Rune Phase D, section 34) — project
//! name field, New / Save-to-projects / Save a copy / Reload /
//! Recover, and the saved project selector. Bin-crate code, declared from `main.rs`
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
//! | `project-tools` | section root — the kit card (`design::panel_col`, Phase N) | — container, no handler | ok |
//! | *(title row, unkeyed)* | "Project" header (`design::panel_title`: folder-open glyph + SECTION title, kit divider under — Phase N) | — container, no handler | ok |
//! | *(autosave line, unkeyed)* | the autosave status line on its own full-width row under the title (kit prose treatment: `.wrap().w_full()`) | renders `autosave_line(core.edits_since_autosave())` (editor.rs:1599) | fixed in Wave 10 — it trailed the title as one unwrapped text until the Wave 9 headless scan caught it at +25.6 pt past the card edge whenever edits ≥ 1 (see [`project_section`]); the counter it renders is the core's — see the autosave note on [`autosave_line`] |
//! | `proj-name-field` | name-field wrapper (el) | — container, no handler | ok |
//! | `proj-name` | text input (kit `design::text_field`, Wave Q) | `on_edit` → `&mut State::name_edit`; `on_submit` → [`apply_name`] | ok — Enter and "Set name" share one path; the kit field carries the focus border + caret/selection colours inside the unchanged `proj-name-field` wrapper |
//! | `proj-set-name` | "Set name" button (`icon_btn`, "check" icon) | [`apply_name`] → [`apply_project_name`]: trims, writes `Project::name` directly (no core rename op or setter exists — editor.rs has no `rename*` fn; the file-level rename is Home's `projects::rename_project`) | ok — Wave 10: a rename that changes the name takes a core snapshot first (a counted edit, so the dirty signal and the autosave cadence see it, and Undo can restore the old name); a same-name press counts nothing |
//! | `proj-current-file` | "Current file: …" muted line under the name field (conditional — present only when the line resolves) | renders [`current_file_line`]: the `shell::last_opened_project` display name, shown only when it exactly matches `Project::name` | ok — Phase I §31; the match guard is the Phase G §32 lesson (recents store names, not the open file, so a mismatch would lie). `home_panel::current_project_path` was NOT used: it is the selector cursor, which `cycle-project` moves without opening |
//! | `proj-current-meta` | "{size} · modified {time}" muted line under `proj-current-file` (conditional — present only when the §31 match holds AND the file resolves and stats) | renders [`current_file_meta`]: the §31 match (via [`current_file_path`]) gates it, the path resolves over `core.project_files` by the crate's slug rule ([`resolve_project_file`]), size via [`human_size`], time via [`modified_label`] | ok — Phase J §27; gated on the §31 decision itself, so the two lines can never disagree about which file the session is on |
//! | `proj-reload` | "Reload from disk" button (`icon_btn`, "rotate-right" icon — Phase N; p13: rotate-right, not Redo's arrow-rotate-right) | [`reload_pressed`] → [`reload_target`] (the §27 gate as an owned path) → [`reload_from_disk`]: no target → status "No saved file to reload.", open never called; target → the open rows' exact discipline (point `project_index` at the file, `EditorCore::open_project_file` — it takes no path argument and returns `()`, editor.rs:3787 — [`clear_stale_solo`], [`sync_name_edit`]), with the success status overridden to name the discard | ok — Phase K §27; see the reload note below. UI mission Wave 4 (S16 D2): with a target and a dirty session the first press arms instead (`ConfirmAction::ReloadProject`; the button re-renders as the danger "Discard changes?") |
//! | `new-proj` | "New" button (`icon_btn`, "file" icon) | [`new_pressed`] → [`new_project`] + [`sync_name_edit`]: installs [`blank_project`]`("Untitled")` — field-for-field the MCP `new_project` tool's defaults and reset set (mcp_tools.rs `tool_new_project`) — clears solo, stops playback, no snapshot | ok — UI mission Wave 4 (S16 D2): a dirty session arms first (`ConfirmAction::NewProject`, danger "Discard changes?" re-render); clean, the press runs at once |
//! | `save-proj` | "Save project" button (`icon_btn`, "floppy-disk" icon) | [`save_named_pressed`] → `EditorCore::save_named_project` (slugifies the name, writes `./projects/<slug>.json`, refreshes `project_files`, sets status), then — only on success — `shell::write_recovery` + `shell::touch_recent` to keep snapshot/recents in step, the recovery write noted on the core via [`note_recovery_if_written`] only when the write lands | fixed in Phase H — the Phase G caveat is closed: the op returns `bool` and [`after_save`] gates the recovery/recents sync on it, so a failed save no longer plants a phantom recent. Wave 13 (E2): the note itself is gated on the recovery write's `io::Result` — before, the result was discarded and the counter reset even when the snapshot write failed |
//! | `save-copy-proj` | "Save a copy" button (`icon_btn`, "copy" icon) on its own row under the New / Save / Recover row | [`save_copy_pressed`] → [`save_project_copy`] → `projects::save_copy` (projects.rs): writes a renamed clone of the current project to `./projects/<slug>.json`, refusing to overwrite; on success the path joins `project_files` at its sorted position ([`insert_sorted_project_file`]) so the copy's selector row appears at once; the session itself — its name, current file and recovery association — is untouched, and recents is deliberately NOT updated (the copy is not the session's file, and the §31 match reads recents) | ok — Wave 9 (D5): the copy's name is derived, not typed: "{project name} copy" ([`copy_project_name`]). A name input would need a second `TextEdit` on `State` (the integrator's field to add, not this section's to invent), and the name field's own buffer is not reusable input — it is the rename draft (see [`save_project_copy`]). A second press therefore collides with the first copy's file and the refusal lands in the status line |
//! | `recover` | "Recover" button (`icon_btn`, "arrow-rotate-left" icon) | [`recover_pressed`] → [`recover_snapshot`] (`shell::read_recovery` = `./projects/.recovery.json`, shell.rs) → [`apply_recovered_project`] + [`sync_name_edit`] | fixed in Phase G — the reset now also clears solo and stops playback (see the fn). UI mission Wave 4: offered only while the snapshot is newer than the last save (S06-2 — otherwise `icon_btn_inert` under the same key), and a dirty session arms first (S06-3 / S16 D2, `ConfirmAction::RecoverSnapshot`) |
//! | `proj-list` | selector list (col) | — container, no handler | ok |
//! | `open-proj-{i}` | "Open" button per row (plain text `btn` — list rows stay text; no icon), accent-styled when `i == core.project_index` | [`open_row_pressed`] → sets `project_index`, `EditorCore::open_project_file` (editor.rs:3636: reads the file; resets current/playhead/figure/node; sets status), [`clear_stale_solo`], [`sync_name_edit`] | fixed in Phase G — stale-solo guard added; the highlight tracks the core's cycle cursor (the palette's `cycle-project` moves it without opening), exactly as [`ProjectRow`] documents. UI mission Wave 4 (S16 D2): a dirty session arms first (`ConfirmAction::OpenProject`; every row's button re-renders as the danger "Discard changes?", and the confirming press opens the row it sits on) |
//! | *(empty state, unkeyed)* | kit `design::empty_state` (folder-open glyph) shown when the selector list is empty — UI mission Wave 4 (audit S06-7) | — display only; the message wording is `cycle_project_file`'s own status string (editor.rs), verbatim | ok |
//!
//! Reload note (Phase K §27): reloading re-opens the saved
//! file and throws the session's unsaved work away — the
//! success status says so outright ("Reloaded {name} from
//! disk — unsaved changes discarded."), because a silent
//! discard is the one unforgivable version of this button.
//! The discard is not quite total, and the limit is worth
//! knowing: opening a file never touches the recovery
//! snapshot, so the state being discarded still exists in
//! `./projects/.recovery.json` — the Recover button can
//! bring it back — until the next autosave overwrites the
//! snapshot (`poll()`'s `maybe_autosave` cadence, every 25
//! counted edits; a named save also rewrites it).
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
use rune::{col, el, row, text, El, Tx};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::{Easing, FigureDef, FigureInstance, Frame, Project};

use crate::chrome_view::{btn, danger_btn};
use crate::rune_sec::design;
use crate::rune_sec::icon_btn::{danger_icon_btn, icon_btn, icon_btn_inert};
use crate::{ConfirmAction, State};

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

/// The name "Save a copy" writes under: the project name as
/// it stands, plus " copy" ("Untitled" → "Untitled copy").
/// Derived, never typed — see [`save_project_copy`] for why
/// the card offers no name input for the copy.
pub fn copy_project_name(project_name: &str) -> String {
    format!("{project_name} copy")
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

/// Estimated painted advance of one character in
/// `type_::LABEL` (12 pt, weight 500) — the fx_browser's
/// measured class-maxima model (`preset_char_width`,
/// fx_browser.rs), charges rounded UP to a clean step so
/// a label the model passes truly fits: the model may
/// truncate a borderline name early, never late.
/// Non-ASCII is charged the full em. Keep the charges in
/// sync with fx_browser's table; both measure the same
/// font class.
fn label_char_width(c: char) -> f32 {
    match c {
        // Real space advance: 3.20.
        ' ' => 4.0,
        // Widest member: 'f' at 4.55.
        'i' | 'l' | 'j' | 't' | 'f' | '.' | ',' | ':' | ';' | '!' | '|' | '\'' | '`' => 5.0,
        // Widest members: '-' and '_' at 5.55.
        '-' | '_' | '(' | ')' | '[' | ']' | '{' | '}' => 6.0,
        // 'm' at 10.66 ('w' is 9.95).
        'm' | 'w' => 11.0,
        // 'W' at 12.04 ('M' is 10.95).
        'M' | 'W' => 12.5,
        // Widest digit: '4' at 7.88.
        '0'..='9' => 8.0,
        // Widest remaining capital: 'Q' at 9.22.
        'A'..='Z' => 9.5,
        // Widest remaining lowercase: 'b'/'d'/'g'/'p'/'q'
        // at 7.42.
        'a'..='z' => 7.5,
        // Widest remaining ASCII: '%' at 11.92 ('@' is
        // 11.79) — a full em, like any wide glyph.
        _ if c.is_ascii() => 12.0,
        _ => 12.0,
    }
}

/// The estimated painted width of `text` in
/// `type_::LABEL`, per [`label_char_width`].
pub fn label_width(text: &str) -> f32 {
    text.chars().map(label_char_width).sum()
}

/// The inner width (pt) the Project card's rows lay out
/// in at the left column's default 300 pt: border 1×2 +
/// `design::PANEL_PAD` 12×2 come off, leaving 274. The
/// shell's budgets are written against the default
/// column widths (the Phase N convention); the splitter
/// can narrow the column further, and the wrapped /
/// truncated treatments below degrade gracefully there
/// instead of spilling. (Private: its consumers are this
/// file's width tests.)
const CARD_INNER_W: f32 = 274.0;

/// The selector-row label's display budget (pt): the
/// card inner width minus the row gap (8) minus the
/// widest button the row can carry — the armed state's
/// "Discard changes?" `danger_btn` (LABEL text estimated
/// at 120.5 by [`label_width`], plus its 10 pt padding
/// each side ≈ 140.5). Sized for the armed state so the
/// label renders identically armed or not: when the
/// discard arm engages, only the button changes, as
/// Wave 4 designed — the label never jumps.
pub const ROW_LABEL_BUDGET: f32 = 124.0;

/// The selector row's DISPLAY label for a saved project's
/// file stem: the stem itself while it fits
/// [`ROW_LABEL_BUDGET`], otherwise middle-truncated with
/// "…" — head and tail both kept, growing alternately so
/// neither side starves (the fx_browser's
/// `preset_button_label` algorithm; the shell's filename
/// treatment). Display only: [`ProjectRow`] keeps the
/// full label, the row's key is its index and its Open
/// handler opens by index, so a truncated label loses no
/// function. Row labels are file stems — a project name
/// is unbounded (the name field takes any length, and a
/// named save slugifies it verbatim), so without this an
/// long name's row pushes its Open button past the
/// card's edge (Wave 10 audit; the figures panel's Wave 9
/// row overflow was the same shape).
pub fn row_display_label(label: &str) -> String {
    if label_width(label) <= ROW_LABEL_BUDGET {
        return label.to_string();
    }
    // The ellipsis is charged a full em, like any wide
    // glyph in the model above.
    let room = ROW_LABEL_BUDGET - 12.0;
    let chars: Vec<char> = label.chars().collect();
    let (mut head, mut tail, mut used) = (0usize, 0usize, 0.0f32);
    loop {
        let mut progressed = false;
        if head + tail < chars.len() {
            let w = label_char_width(chars[head]);
            if used + w <= room {
                head += 1;
                used += w;
                progressed = true;
            }
        }
        if head + tail < chars.len() {
            let w = label_char_width(chars[chars.len() - 1 - tail]);
            if used + w <= room {
                tail += 1;
                used += w;
                progressed = true;
            }
        }
        if !progressed {
            break;
        }
    }
    let mut out: String = chars[..head].iter().collect();
    out.push('…');
    out.extend(&chars[chars.len() - tail..]);
    out
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
            states: Vec::new(),
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
        head_ring_scale: 1.0,
        subtitles: Vec::new(),
        subtitle_style: Default::default(),
        format_version: stykanimate_rs::model::FORMAT_VERSION_CURRENT,
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
/// IS accounted since Wave 12 (E8): the core exposes
/// `note_recovery_written` for exactly that write, and
/// [`save_named_pressed`] runs it through
/// [`note_recovery_if_written`], gated on the write
/// landing (Wave 13, E2) — so after a successful save the
/// line reads "up to date", and after a failed snapshot
/// write it keeps counting, truthfully. (The Phase G
/// version of this note claimed no such API existed and
/// the line could overstate staleness after a save; both
/// halves were true then and neither is now.)
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

/// Insert a just-written project file into the core's file
/// list at its sorted position — the list's own invariant
/// (`EditorCore::list_project_files` collects the same
/// directory in the same path shape and sorts it; that fn
/// is private, and the refreshes through it live on the
/// open/save op paths, so a copy written by the
/// library-level `projects::save_copy` joins the list here
/// instead). An already-listed path is left alone — the
/// list never holds a file twice. `project_index` is
/// deliberately untouched, exactly as the core's own
/// refresh inside `save_named_project` leaves it: the
/// index is the selector cursor, not the open file (the
/// Phase G §32 lesson).
pub fn insert_sorted_project_file(files: &mut Vec<PathBuf>, path: &Path) {
    if let Err(pos) = files.binary_search_by(|p| p.as_path().cmp(path)) {
        files.insert(pos, path.to_path_buf());
    }
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

/// The file "Reload from disk" would re-open: the §27 gate
/// ([`current_file_path`] — the shell's last-opened record
/// must name this project, and the name must resolve over
/// the core's file list) as an owned path, so the caller
/// can hold it across the `&mut` open call. `None` is the
/// refusal case: no saved file backs this session (never
/// saved, saved under another name, or the file has left
/// the list), and there is nothing honest to reload.
///
/// Like [`current_file_meta`], this wrapper reads the
/// shell's recents from disk, so it is not itself
/// unit-tested (the §31 precedent); the decision it wraps
/// is [`current_file_path`], which is — see its tests and
/// the reload-gate test below.
pub fn reload_target(core: &EditorCore) -> Option<PathBuf> {
    current_file_path(
        stykanimate_rs::shell::last_opened_project(),
        &core.project.name,
        &core.project_files,
    )
    .cloned()
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
///
/// Wave 10 audit (dirty truth): a rename is a project
/// mutation, so when the name actually changes the press
/// takes a core snapshot first — the core's own discipline
/// for every mutating op. Before this, the field write
/// bypassed the edit counter entirely: the dirty signal
/// (`edits_since_autosave`) never saw a rename, the
/// recovery snapshot kept the old name until 25 unrelated
/// counted edits accumulated, the rename was silently
/// losable on exit, and Undo could not restore the old
/// name. The snapshot fixes all four at once (one counted
/// edit, and an undo step whose restore carries the old
/// name). A press that re-applies the name the project
/// already has is not a mutation: no snapshot, no counted
/// edit.
pub fn apply_project_name(core: &mut EditorCore, raw: &str) -> bool {
    match clean_project_name(raw) {
        Some(name) => {
            if name != core.project.name {
                core.snapshot();
                core.project.name = name.clone();
            }
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
/// fields reset, with **no undo snapshot** — this fn is
/// the raw swap, and it drops unsaved work the moment it
/// runs, exactly as the tool drops it. The section's
/// button no longer calls it unguarded: [`new_pressed`]
/// arms a confirm first while the session is dirty (UI
/// mission Wave 4, audit S16 D2). Undo history is left
/// untouched, as the tool leaves it.
pub fn new_project(core: &mut EditorCore) {
    core.project = blank_project("Untitled");
    core.note_project_swapped();
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
    core.note_project_swapped();
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

/// Reload the project from its saved file, discarding the
/// session's unsaved work (Phase K §27). `target` is the
/// [`reload_target`] decision; taking it as a parameter —
/// rather than re-reading the shell record here — is what
/// makes the refusal path testable without a disk.
///
/// The open itself is the selector rows' mechanism,
/// unchanged: `open_project_file` (editor.rs:3787) takes
/// no path argument and returns `()` — it opens
/// `project_files[project_index]` — so the target is
/// first located in the core's list and the index pointed
/// at it, exactly as [`open_row_pressed`] does. A target
/// outside the list cannot be opened that way and is
/// refused with the same status as no target: as far as
/// this session is concerned there is no saved file at
/// that path to reload. No snapshot is taken on any path —
/// neither the rows' open nor this reload creates an undo
/// step for a project swap.
///
/// Post-open discipline mirrors the rows': the stale-solo
/// guard runs ([`clear_stale_solo`] — the op also clears
/// solo itself on success; the guard covers the failure
/// path, where the project never changed), and the
/// caller re-syncs the name field. The one deliberate
/// difference is the final status. The op reports only
/// through the status line — "Opened {path}" on success,
/// "Open failed: {e}" on failure — so success is read off
/// that wording immediately after the call, before
/// anything else can write status (the solo guard writes
/// none). On success the status is replaced with one that
/// names the discard: the reload has just thrown the
/// session's unsaved work away, and the user is told so,
/// not left to discover it. On failure the op's own
/// "Open failed: …" stands — nothing was discarded, and
/// the status must not claim otherwise. Opening never
/// touches the recovery snapshot, so even after a
/// successful reload the discarded state survives in
/// `./projects/.recovery.json` until the next autosave
/// (see the module docs' reload note).
///
/// Returns whether the reload landed.
pub fn reload_from_disk(core: &mut EditorCore, target: Option<&Path>) -> bool {
    let Some(target) = target else {
        core.status = "No saved file to reload.".into();
        return false;
    };
    let Some(index) = core
        .project_files
        .iter()
        .position(|path| path.as_path() == target)
    else {
        core.status = "No saved file to reload.".into();
        return false;
    };
    core.project_index = index;
    core.open_project_file();
    clear_stale_solo(core);
    if core.status.starts_with("Opened ") {
        core.status = format!(
            "Reloaded {} from disk — unsaved changes discarded.",
            core.project.name
        );
        true
    } else {
        false
    }
}

/// The status line a "Save a copy" attempt earns, factored
/// out of [`save_project_copy`] so both arms are testable
/// without writing a file (the write itself targets the
/// real `./projects/`, which tests must not pollute — the
/// factored-wrapper precedent of [`reload_target`]).
/// Success names the copy and its file; failure passes the
/// library's own wording through under the save family's
/// failure prefix (the named save's is "Project save
/// failed: …"), so a collision reads "Save a copy failed:
/// project file already exists: …".
pub fn copy_status(copy_name: &str, result: &Result<PathBuf, String>) -> String {
    match result {
        Ok(path) => format!("Saved a copy as \"{copy_name}\" ({}).", path.display()),
        Err(e) => format!("Save a copy failed: {e}"),
    }
}

/// "Save a copy" at the core level: write the current
/// project — unsaved changes included, exactly as it sits
/// in memory — to `./projects/` under a derived new name
/// ([`copy_project_name`]) via `projects::save_copy`, which
/// stores the copy carrying that name in its JSON and
/// refuses to overwrite an existing file. The session is
/// not re-pointed at the copy: its name, current file,
/// recovery association and recents all keep naming the
/// original. Recents especially — the current-file line's
/// §31 match reads them, and the copy is not the file this
/// session is on.
///
/// Why the name is derived rather than typed: a name
/// input would need a second text-edit state, which lives
/// on `State` — the integrator's field to add, not this
/// section's to invent. The one field the card does have
/// is not a substitute: its buffer is the *rename* draft
/// (unapplied typing for "Set name"), and copying under a
/// half-typed rename the user never confirmed would name
/// the file after text they may be about to discard. So
/// the button always writes "{project name} copy"; a
/// second press meets the first copy's file, the library
/// refuses, and the refusal is the status line — rename
/// the project (or remove the old copy) to copy again.
///
/// On success the written path joins the selector's list
/// at its sorted position ([`insert_sorted_project_file`]),
/// so the copy's Open row appears at once. Returns
/// whether the copy was written.
///
/// Counter discipline (Wave 13, E2 audit): this path
/// writes NO recovery snapshot — `projects::save_copy`
/// writes only the copy's own file, and the session's
/// `.recovery.json` keeps whatever it already held — so
/// it deliberately never calls
/// `EditorCore::note_recovery_written`. The copy is not
/// the session's file and not its snapshot; noting would
/// zero the outstanding-edit counter on coverage that
/// does not exist.
pub fn save_project_copy(core: &mut EditorCore) -> bool {
    let copy_name = copy_project_name(&core.project.name);
    let result = stykanimate_rs::projects::save_copy(core, &copy_name);
    if let Ok(path) = &result {
        insert_sorted_project_file(&mut core.project_files, path);
    }
    let saved = result.is_ok();
    core.status = copy_status(&copy_name, &result);
    saved
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

/// True while the session holds work the recovery
/// snapshot has not captured — the shell's one truthful
/// saved-state signal, `edits_since_autosave() > 0` (see
/// [`ConfirmAction`]'s docs for the rationale and the
/// counter's limits). Defined here and shared by the
/// Home panel and the app bar, so every discard gate in
/// the shell reads the same counter the same way.
pub fn session_dirty(core: &EditorCore) -> bool {
    core.edits_since_autosave() > 0
}

/// "Set name" (and Enter in the field): apply the field's text.
fn apply_name(s: &mut State) {
    // A press elsewhere in the panel stands down any
    // armed discard (the confirm_delete discipline).
    s.confirm_action = None;
    let raw = s.name_edit.text.clone();
    apply_project_name(&mut s.core, &raw);
}

/// "New": [`new_project`], then re-sync the name field.
///
/// UI mission Wave 4 (audit S16 D2): while the session
/// is dirty the first press only arms — the button
/// re-renders as a danger confirm (see
/// [`project_section`]) and the status names the
/// discard; the second press runs the swap. Clean, the
/// press runs immediately, as before.
fn new_pressed(s: &mut State) {
    if s.confirm_action != Some(ConfirmAction::NewProject) && session_dirty(&s.core) {
        s.confirm_action = Some(ConfirmAction::NewProject);
        s.core.status =
            "New project would discard unsaved changes — press New again to confirm.".into();
        return;
    }
    s.confirm_action = None;
    new_project(&mut s.core);
    // Wave 10 (C6): a stroke selection is an index into
    // the OLD project's stroke list — after a swap it
    // would silently name (and edit, or delete) a stroke
    // the user never selected in this project. Every
    // session-replacing press in this section clears it,
    // the clear_stale_solo discipline for shell state the
    // core cannot curate.
    s.stroke_selected = None;
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

/// Note a recovery write on the core — but only when the
/// write actually landed. `recovery_written` is the
/// success report of the `shell::write_recovery` call the
/// named save runs as its follow-up; the note
/// (`EditorCore::note_recovery_written`, Wave 12 E8) zeroes
/// the outstanding-edit counter on the claim that the
/// snapshot now covers those edits, so running it after a
/// FAILED write would state the opposite of the truth:
/// the counter would read "up to date" while
/// `./projects/.recovery.json` still holds the older
/// state, and the autosave cadence would wait a fresh 25
/// edits before retrying. The core's own autosave holds
/// exactly this discipline (`maybe_autosave` notes only
/// on `Ok` and leaves the counter alone on `Err`); this
/// gate is the same rule for the shell-side write.
///
/// Wave 13 (E2) audit: before this gate existed, the one
/// call site in this file discarded the write's
/// `io::Result` (`let _ =`) and noted unconditionally.
/// Factored out of the press handler in the
/// [`after_save`] idiom, so the gating is testable
/// without touching the disk.
pub fn note_recovery_if_written(core: &mut EditorCore, recovery_written: bool) {
    if recovery_written {
        core.note_recovery_written();
    }
}

/// "Save project": the GPUI `save-proj` listener — save
/// under the project's name in `./projects/`, then keep the
/// recovery snapshot and recents list in step with the
/// explicit save. Phase H gates those follow-ups on the
/// op's success report ([`after_save`]): on failure the
/// op's status ("Project save failed: …") is the last word
/// and recents stays untouched. The recovery write itself
/// is noted on the core only when it lands
/// ([`note_recovery_if_written`] — Wave 13, E2: the write
/// returns an `io::Result`, and a failed snapshot write
/// must not zero the outstanding-edit counter).
fn save_named_pressed(s: &mut State) {
    // Saving is the alternative to discarding: the press
    // stands down any armed discard in this panel.
    s.confirm_action = None;
    let saved = s.core.save_named_project();
    if after_save(saved) == AfterSave::SyncRecoveryAndRecents {
        let recovery_written =
            stykanimate_rs::shell::write_recovery(&s.core.project).is_ok();
        note_recovery_if_written(&mut s.core, recovery_written);
        stykanimate_rs::shell::touch_recent(&s.core.project.name);
    }
}

/// "Save a copy": [`save_project_copy`] — the current
/// state written under a derived "{name} copy" file; the
/// session stays on the original project. Copying
/// discards nothing, so no confirm arm gates it (unlike
/// New / Open / Recover / Reload); like every press in
/// this panel it stands down an arm left by another
/// button (the [`apply_name`] discipline). `pub(crate)`
/// since Wave 9 integration: the command palette's
/// "Save a copy" entry names this handler directly, the
/// settings-toggle idiom.
pub(crate) fn save_copy_pressed(s: &mut State) {
    s.confirm_action = None;
    save_project_copy(&mut s.core);
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
///
/// UI mission Wave 4 (audit S16 D2): gated like
/// [`new_pressed`] — while the session is dirty, a press
/// on any row's Open arms instead of opening (the arm is
/// the action, not the row: the confirming press may be
/// a different row's Open, and it opens *that* row — the
/// user pressed a "Discard changes?" button naming the
/// file they are about to open either way).
fn open_row_pressed(s: &mut State, i: usize) {
    if s.confirm_action != Some(ConfirmAction::OpenProject) && session_dirty(&s.core) {
        s.confirm_action = Some(ConfirmAction::OpenProject);
        s.core.status =
            "Opening a project would discard unsaved changes — press Open again to confirm."
                .into();
        return;
    }
    s.confirm_action = None;
    s.core.project_index = i;
    s.core.open_project_file();
    clear_stale_solo(&mut s.core);
    s.stroke_selected = None;
    sync_name_edit(s);
}

/// "Recover": [`recover_snapshot`], then re-sync the name field.
///
/// UI mission Wave 4 (audit S16 D2 + S06-3): gated like
/// [`new_pressed`] — recovering replaces the session, so
/// a dirty session arms first. The button itself is
/// only live while a snapshot newer than the last save
/// exists (audit S06-2, gated at build in
/// [`project_section`]), so this handler never runs in
/// the no-snapshot case.
fn recover_pressed(s: &mut State) {
    if s.confirm_action != Some(ConfirmAction::RecoverSnapshot) && session_dirty(&s.core) {
        s.confirm_action = Some(ConfirmAction::RecoverSnapshot);
        s.core.status =
            "Recovering the snapshot would discard unsaved changes — press Recover again to confirm."
                .into();
        return;
    }
    s.confirm_action = None;
    recover_snapshot(&mut s.core);
    s.stroke_selected = None;
    sync_name_edit(s);
}

/// "Reload from disk": resolve the target from the shell
/// record ([`reload_target`]), run the reload
/// ([`reload_from_disk`] owns the refusal status, the open
/// and the discard-naming success status), then re-sync
/// the name field — the rows' post-open discipline, as
/// with every switch this section performs.
///
/// UI mission Wave 4 (audit S16 D2): gated like
/// [`new_pressed`] when a target exists — the discard is
/// real, so a dirty session arms first. With no target
/// there is nothing to discard into, so the refusal
/// status answers immediately, armed or not.
fn reload_pressed(s: &mut State) {
    let target = reload_target(&s.core);
    if target.is_some()
        && s.confirm_action != Some(ConfirmAction::ReloadProject)
        && session_dirty(&s.core)
    {
        s.confirm_action = Some(ConfirmAction::ReloadProject);
        s.core.status =
            "Reloading would discard unsaved changes — press Reload again to confirm.".into();
        return;
    }
    s.confirm_action = None;
    reload_from_disk(&mut s.core, target.as_deref());
    s.stroke_selected = None;
    sync_name_edit(s);
}

// ---------------------------------------------------------------------
// View
// ---------------------------------------------------------------------

/// The Project section: name field + Set name, the
/// current-file line and its metadata line when the
/// shell's last-opened record names this project,
/// Reload from disk, New / Save project / Recover, a
/// Save-a-copy row, the autosave line, and one Open row
/// per saved project in the core's file list.
pub fn project_section(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let rows = project_rows(core);

    let mut section = design::panel_col("project-tools", t)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(design::panel_title(Some("folder-open"), "Project", None, t)),
        )
        // Wave 10 (integrator tip, Wave 9 headless proof):
        // the autosave line used to trail the title in its
        // row as a single unwrapped text — at 2 counted
        // edits the clips-populated tree measured it at
        // x=98.13 with width 231.5, its right edge 329.6,
        // +25.6 pt past the left column's 304 content
        // edge. It only fires when edits ≥ 1 (the 0-edit
        // line, "Recovery snapshot up to date.", is short
        // enough to fit), which is why every default-state
        // tree looked clean — and why in real use, where
        // the counter is ≥ 1 almost always, it painted
        // outside the card constantly. The line now gets
        // its own full-width row under the title with the
        // shell's proven prose treatment (.wrap().w_full(),
        // the audio summary line's pattern): at the card's
        // 274 pt inner width the current strings fit on
        // one line for any plausible count (see the width
        // test), and an extreme count — or a splitter-
        // narrowed column — wraps inside the card instead
        // of spilling. The strings themselves are
        // unchanged.
        .child(
            text(autosave_line(core.edits_since_autosave()))
                .font(type_::LABEL)
                .color(t.text_secondary)
                .wrap()
                .w_full(),
        )
        .child(design::divider(t))
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
                            design::text_field(&s.name_edit, type_::BODY, s.name_edit.focused, t)
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

    // Reload from disk sits beside the current-file lines
    // it acts on. It renders unconditionally — unlike the
    // lines above, which are conditional on the §31 match —
    // because the no-target case is a real press outcome
    // (the status explains there is no saved file to
    // reload), and a button that vanishes exactly when it
    // would have something to say teaches nothing.
    // The discard arm (UI mission Wave 4): while an
    // action is armed, its button re-renders in place —
    // same key, same icon — as the danger family member
    // labelled "Discard changes?", and the confirming
    // press is the same handler (it sees the arm and
    // runs). See the handlers' docs.
    let armed = s.confirm_action;
    section = section.child(
        row()
            .w_full()
            .gap(8.0)
            .center_y()
            .child(if armed == Some(ConfirmAction::ReloadProject) {
                danger_icon_btn("proj-reload", "rotate-right", "Discard changes?", false, t, reload_pressed)
            } else {
                icon_btn("proj-reload", "rotate-right", "Reload from disk", false, t, reload_pressed)
            }),
    );

    // Recover's offer gate (audit S06-2): the button is
    // live only while the recovery snapshot holds work
    // newer than the last named save — the Home banner's
    // verdict (`shell::recovery_is_newer_than_last_project`,
    // read at panel build, as the audit prescribes).
    // Otherwise it renders inert under the same key: a
    // snapshot that is absent, or no newer than the last
    // save, has nothing to offer this session, and the
    // header's autosave line already says which. The
    // reload escape hatch survives the gate: the
    // snapshot a reload discards *to* is by construction
    // newer than the file that was reloaded.
    let recover_live = stykanimate_rs::shell::recovery_is_newer_than_last_project();
    section = section.child(
        row()
            .w_full()
            .gap(8.0)
            .center_y()
            .child(if armed == Some(ConfirmAction::NewProject) {
                danger_icon_btn("new-proj", "file", "Discard changes?", false, t, new_pressed)
            } else {
                icon_btn("new-proj", "file", "New", false, t, new_pressed)
            })
            .child(icon_btn("save-proj", "floppy-disk", "Save project", false, t, save_named_pressed))
            .child(if !recover_live {
                icon_btn_inert("recover", "arrow-rotate-left", "Recover", t)
            } else if armed == Some(ConfirmAction::RecoverSnapshot) {
                danger_icon_btn("recover", "arrow-rotate-left", "Discard changes?", false, t, recover_pressed)
            } else {
                icon_btn("recover", "arrow-rotate-left", "Recover", false, t, recover_pressed)
            }),
    );

    // Save a copy gets its own row under the New / Save /
    // Recover row: that row is full, and the copy belongs
    // beside the Save it variants — same write family,
    // different file. The press is never gated and never
    // arms: copying discards nothing (see
    // save_copy_pressed).
    section = section.child(
        row()
            .w_full()
            .gap(8.0)
            .center_y()
            .child(icon_btn("save-copy-proj", "copy", "Save a copy", false, t, save_copy_pressed)),
    );

    if rows.is_empty() {
        // The kit empty state (UI mission Wave 4, audit
        // S06-7): the message is the core's own wording
        // (cycle_project_file), byte-identical — only the
        // presentation moves to the kit, matching Home's
        // empty library.
        section = section.child(design::empty_state(
            "folder-open",
            "No saved projects in ./projects/ yet.",
            t,
        ));
    } else {
        let mut list = col().key("proj-list").w_full().gap(4.0);
        for (i, row_data) in rows.iter().enumerate() {
            list = list.child(
                row()
                    .w_full()
                    .gap(8.0)
                    .center_y()
                    .child(
                        // Display label only — the row
                        // opens by index, and ProjectRow
                        // keeps the full stem (see
                        // row_display_label for the budget).
                        text(row_display_label(&row_data.label))
                            .font(type_::LABEL)
                            .color(t.text)
                            .grow(1.0),
                    )
                    .child(if armed == Some(ConfirmAction::OpenProject) {
                        danger_btn(
                            &open_row_key(i),
                            "Discard changes?",
                            t,
                            move |s: &mut State| open_row_pressed(s, i),
                        )
                    } else {
                        btn(
                            &open_row_key(i),
                            "Open",
                            row_data.selected,
                            t,
                            move |s: &mut State| open_row_pressed(s, i),
                        )
                    }),
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
            sprites_panel: crate::rune_sec::sprites_panel::SpritesPanelState::default(),
            home_panel: crate::rune_sec::home_panel::HomePanelState::default(),
            frame_ops: crate::rune_sec::frame_ops::FrameOpsState::default(),
            graph_panel: crate::rune_sec::graph_panel::GraphPanelState::default(),
            audio_panel: crate::rune_sec::audio_panel::AudioPanelState::new(),
            subtitle_panel: crate::rune_sec::subtitle_panel::SubtitlePanelState::new(),
            menus: crate::rune_sec::menus::MenusState::default(),
            palette: crate::rune_sec::command_palette::PaletteState::new(),
            settings: crate::rune_sec::settings_popover::SettingsState::default(),
            toasts: crate::rune_sec::toasts::ToastsState::new(),
            text_tool: crate::rune_sec::text_tool::TextToolState::new(),
            name_edit: rune::TextEdit::new(""),
            export_use_range: false,
            fx_selected: None,
            subtitle_selected: None,
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
            export_job: None,
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
playback_speed: 1.0,
            was_playing: false,
            confirm_action: None,
            home_rename: crate::rune_sec::home_panel::HomeRenameState::new(),
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
    fn autosave_line_widths_fit_the_card() {
        // The Wave 10 fix's budget, under the conservative
        // width model (charges rounded up; the Wave 9 tree
        // measured the 2-edit line at 231.5 pt where the
        // model estimates 267 — the model may pass a line
        // late, never early). On the line's own full-width
        // row the common counts fit on ONE line inside the
        // card's 274 pt inner width; larger counts fit
        // within two lines, which the row's .wrap() absorbs
        // instead of spilling past the card edge — the
        // pre-fix title-row layout spilled at +25.6 pt
        // already at 2 edits.
        for n in [0, 1, 2] {
            assert!(
                label_width(&autosave_line(n)) <= CARD_INNER_W,
                "count {n} fits one line: {}",
                autosave_line(n)
            );
        }
        for n in [25, 999] {
            assert!(
                label_width(&autosave_line(n)) <= 2.0 * CARD_INNER_W,
                "count {n} wraps within two lines: {}",
                autosave_line(n)
            );
        }
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
    fn rename_takes_a_snapshot_and_counts_as_an_edit() {
        // Wave 10 (dirty truth): before the fix a rename
        // wrote the field directly and the edit counter
        // never moved — the dirty dot stayed dark, the
        // recovery snapshot kept the old name, and Undo
        // could not restore it.
        let mut core = EditorCore::new();
        let before = core.project.name.clone();
        assert_eq!(core.edits_since_autosave(), 0, "a fresh core is clean");
        assert!(apply_project_name(&mut core, "Renamed Film"));
        assert_eq!(core.edits_since_autosave(), 1, "the rename is one counted edit");
        assert!(session_dirty(&core), "the dirty signal sees the rename");
        core.undo();
        assert_eq!(core.project.name, before, "undo restores the old name");
    }

    #[test]
    fn reapplying_the_same_name_counts_nothing() {
        // A "Set name" press that changes nothing is not
        // a mutation: no snapshot, no counted edit, the
        // undo stack untouched.
        let mut core = EditorCore::new();
        let name = core.project.name.clone();
        let depth = core.undo_depth();
        assert!(apply_project_name(&mut core, &name));
        assert_eq!(core.edits_since_autosave(), 0);
        assert_eq!(core.undo_depth(), depth);
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
    fn new_pressed_arms_first_while_dirty_then_confirms() {
        // UI mission Wave 4 (S16 D2): one snapshot-counted
        // edit makes the session dirty; the first New
        // press only arms, the second runs the swap.
        let mut core = EditorCore::new();
        core.project.name = "Unsaved Work".into();
        core.snapshot();
        let mut s = test_state(core);
        assert!(session_dirty(&s.core), "one counted edit is dirty");
        new_pressed(&mut s);
        assert_eq!(s.confirm_action, Some(ConfirmAction::NewProject));
        assert_eq!(s.core.project.name, "Unsaved Work", "first press only arms");
        assert!(s.core.status.contains("press New again"), "{}", s.core.status);
        new_pressed(&mut s);
        assert_eq!(s.confirm_action, None, "confirm clears the arm");
        assert_eq!(s.core.project.name, "Untitled", "second press runs the swap");
    }

    #[test]
    fn new_pressed_runs_immediately_while_clean() {
        let mut s = test_state(EditorCore::new());
        assert!(!session_dirty(&s.core), "a fresh core is clean");
        new_pressed(&mut s);
        assert_eq!(s.confirm_action, None);
        assert_eq!(s.core.status, "New project.");
    }

    #[test]
    fn open_row_arms_first_while_dirty() {
        let mut core = EditorCore::new();
        core.project_files = vec![
            PathBuf::from("./projects/alpha.json"),
            PathBuf::from("./projects/beta.json"),
        ];
        core.snapshot();
        let mut s = test_state(core);
        open_row_pressed(&mut s, 1);
        assert_eq!(s.confirm_action, Some(ConfirmAction::OpenProject));
        assert_eq!(s.core.project_index, 0, "arming does not move the index");
        // The confirming press runs the open path (its
        // disk outcome is the core's business) and
        // clears the arm either way.
        open_row_pressed(&mut s, 1);
        assert_eq!(s.confirm_action, None);
        assert_eq!(s.core.project_index, 1, "confirm opens the pressed row");
    }

    #[test]
    fn open_row_arm_leaves_the_session_untouched() {
        // Wave 10 audit (the cancel contract): the arming
        // press is the flow's "cancel" surface — walking
        // away from it must cost nothing, so it may touch
        // ONLY the arm and the status line. Project,
        // selector index, playhead, playback and selection
        // all stay exactly as they were.
        let mut core = EditorCore::new();
        core.project.name = "Unsaved Work".into();
        core.project_files = vec![
            PathBuf::from("./projects/alpha.json"),
            PathBuf::from("./projects/beta.json"),
        ];
        core.project_index = 0;
        core.current = 1;
        core.playing = true;
        core.solo = Some(0);
        core.snapshot();
        let mut s = test_state(core);
        open_row_pressed(&mut s, 1);
        assert_eq!(s.confirm_action, Some(ConfirmAction::OpenProject));
        assert!(s.core.status.contains("press Open again"), "{}", s.core.status);
        assert_eq!(s.core.project.name, "Unsaved Work", "project untouched");
        assert_eq!(s.core.project_index, 0, "index untouched");
        assert_eq!(s.core.current, 1, "playhead untouched");
        assert!(s.core.playing, "playback untouched");
        assert_eq!(s.core.solo, Some(0), "selection untouched");
    }

    #[test]
    fn recover_arms_first_while_dirty() {
        let mut core = EditorCore::new();
        core.project.name = "Unsaved Work".into();
        core.snapshot();
        let mut s = test_state(core);
        recover_pressed(&mut s);
        assert_eq!(s.confirm_action, Some(ConfirmAction::RecoverSnapshot));
        assert_eq!(s.core.project.name, "Unsaved Work", "first press only arms");
        recover_pressed(&mut s);
        assert_eq!(s.confirm_action, None, "second press runs and clears");
    }

    #[test]
    fn a_press_elsewhere_in_the_panel_stands_the_arm_down() {
        let mut core = EditorCore::new();
        core.snapshot();
        let mut s = test_state(core);
        new_pressed(&mut s);
        assert_eq!(s.confirm_action, Some(ConfirmAction::NewProject));
        // "Set name" is another press in the same panel:
        // it stands the arm down without running New.
        s.name_edit.replace_text("Renamed".to_string());
        apply_name(&mut s);
        assert_eq!(s.confirm_action, None);
        assert_eq!(s.core.project.name, "Renamed");
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
    fn recovery_note_follows_the_recovery_write_result() {
        // Wave 13 (E2): the named save's recovery follow-up
        // notes the write on the core ONLY when the write
        // landed — the core's own maybe_autosave discipline
        // (on Err it leaves the counter alone, so the next
        // tick retries instead of waiting a fresh 25 edits).
        // Before the gate, save_named_pressed discarded
        // write_recovery's io::Result and noted
        // unconditionally: a failed snapshot write (full
        // disk, permissions) still zeroed the counter,
        // claiming coverage the snapshot never had, while
        // the status strip's dirty read of the same counter
        // contradicted it.
        let mut core = EditorCore::new();
        core.snapshot();
        assert_eq!(
            core.edits_since_autosave(),
            1,
            "one counted edit outstanding"
        );
        note_recovery_if_written(&mut core, false);
        assert_eq!(
            core.edits_since_autosave(),
            1,
            "a failed recovery write must not reset the counter"
        );
        assert!(session_dirty(&core), "the session still reads dirty");
        note_recovery_if_written(&mut core, true);
        assert_eq!(
            core.edits_since_autosave(),
            0,
            "a landed recovery write covers the edit"
        );
        assert!(!session_dirty(&core));
    }

    #[test]
    fn swap_paths_zero_the_counter_by_the_swap_not_by_a_note() {
        // The swap paths in this file (New, Recover — and
        // the core's Open/Reload behind the rows) write no
        // recovery snapshot, so none of them may call
        // note_recovery_written; their counter still lands
        // at zero, through the swap itself
        // (note_project_swapped zeroes both counters in the
        // core). This pins that the zero is the swap's
        // doing: a dirty session swapped either way reads
        // clean without any recovery write having happened.
        let mut core = EditorCore::new();
        core.snapshot();
        assert_eq!(core.edits_since_autosave(), 1);
        new_project(&mut core);
        assert_eq!(
            core.edits_since_autosave(),
            0,
            "New zeroes the counter via the swap"
        );
        core.snapshot();
        assert_eq!(core.edits_since_autosave(), 1);
        apply_recovered_project(&mut core, Project::new_demo());
        assert_eq!(
            core.edits_since_autosave(),
            0,
            "Recover zeroes the counter via the swap"
        );
    }

    #[test]
    fn copy_name_appends_copy_to_the_project_name() {
        assert_eq!(copy_project_name("Untitled"), "Untitled copy");
        assert_eq!(copy_project_name("My Film"), "My Film copy");
        // A project already named like a copy copies again
        // under its own derivation — the rule is the plain
        // suffix, with no special case.
        assert_eq!(copy_project_name("My Film copy"), "My Film copy copy");
    }

    #[test]
    fn copy_status_names_the_copy_and_passes_failures_through() {
        // Success names the copy and the file it landed in.
        let written: Result<PathBuf, String> =
            Ok(PathBuf::from("/home/user/projects/my-film-copy.json"));
        assert_eq!(
            copy_status("My Film copy", &written),
            "Saved a copy as \"My Film copy\" (/home/user/projects/my-film-copy.json)."
        );
        // The collision wording is the library's own,
        // carried verbatim under the save family's failure
        // prefix — the user is told which file blocked the
        // copy and why.
        let collision: Result<PathBuf, String> = Err(
            "project file already exists: /home/user/projects/my-film-copy.json"
                .to_string(),
        );
        assert_eq!(
            copy_status("My Film copy", &collision),
            "Save a copy failed: project file already exists: \
             /home/user/projects/my-film-copy.json"
        );
    }

    #[test]
    fn sorted_insert_keeps_the_file_list_sorted_and_unique() {
        let mut files = vec![
            PathBuf::from("/p/alpha.json"),
            PathBuf::from("/p/zeta.json"),
        ];
        // Middle, front and end insertions all land at
        // their sorted positions — the list_project_files
        // invariant the selector's row order comes from.
        insert_sorted_project_file(&mut files, Path::new("/p/mid.json"));
        insert_sorted_project_file(&mut files, Path::new("/p/a-first.json"));
        insert_sorted_project_file(&mut files, Path::new("/p/zz-last.json"));
        let labels: Vec<String> = files.iter().map(|p| project_file_label(p)).collect();
        assert_eq!(labels, ["a-first", "alpha", "mid", "zeta", "zz-last"]);
        // A path already listed is not duplicated.
        insert_sorted_project_file(&mut files, Path::new("/p/mid.json"));
        assert_eq!(files.len(), 5);
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
    fn row_display_label_keeps_names_that_fit() {
        assert_eq!(row_display_label("alpha"), "alpha");
        assert_eq!(row_display_label("beta-cut"), "beta-cut");
        assert_eq!(row_display_label("my-film-copy"), "my-film-copy");
    }

    #[test]
    fn row_display_label_truncates_a_long_stem_in_the_middle() {
        // Project names are unbounded, so their slugs are:
        // this stem estimates ~420 pt against the row's
        // 124 pt budget (the armed "Discard changes?"
        // button is the binding width).
        let label = row_display_label(
            "the-quick-brown-fox-jumps-over-the-lazy-dog-again-and-again",
        );
        assert!(label.contains('…'), "the cut is marked: {label}");
        assert!(
            label_width(&label) <= ROW_LABEL_BUDGET,
            "the truncated label fits its budget: {label}"
        );
        // Head and tail both survive — the stem's start
        // and its distinguishing end stay readable.
        assert!(label.starts_with("the"), "{label}");
        assert!(label.ends_with("again"), "{label}");
    }

    #[test]
    fn row_display_label_truncates_wide_glyphs_a_char_count_would_pass() {
        // Twenty characters sounds short; twenty 'm's
        // estimate 220 pt — width, not length, is the
        // budget (the fx_browser model's lesson).
        let label = row_display_label("mmmmmmmmmmmmmmmmmmmm");
        assert!(label.contains('…'), "{label}");
        assert!(label_width(&label) <= ROW_LABEL_BUDGET, "{label}");
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

    #[test]
    fn project_section_constructs_with_the_name_field_in_both_focus_branches() {
        // Wave Q: `proj-name` is the kit
        // `design::text_field` — the section builds
        // with the field resting (hairline border)
        // and focused (accent border + caret colours)
        // under both themes.
        let mut s = test_state(EditorCore::new());
        for dark in [true, false] {
            let t = rune::tok::theme(dark);
            s.name_edit.focused = false;
            let _ = project_section(&s, &t);
            s.name_edit.focused = true;
            let _ = project_section(&s, &t);
        }
    }

    /// A scratch path for the reload tests, unique to this
    /// test within the process (the reload action reads real
    /// files — that is its job — so its success and failure
    /// paths are exercised against real temp files, never
    /// against `./projects/`).
    fn reload_scratch(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "stykanimate-rs-proj-reload-{}-{tag}.json",
            std::process::id()
        ))
    }

    #[test]
    fn reload_gate_resolves_the_current_file_as_an_owned_path() {
        // The gate reload_target evaluates, in the §27 test
        // pattern: the factored decision (current_file_path)
        // plus the clone that makes the path owned. The
        // wrapper itself reads the shell's recents from disk
        // and — like current_file_meta, the §31 precedent —
        // is not unit-tested directly; everything it decides
        // is decided here.
        let files = vec![
            PathBuf::from("./projects/alpha.json"),
            PathBuf::from("./projects/my-film.json"),
        ];
        // Name matches a listed file: the reload has a target.
        assert_eq!(
            current_file_path(Some("My Film".to_string()), "My Film", &files).cloned(),
            Some(files[1].clone())
        );
        // Name absent from the list (deleted, renamed, never
        // saved): no target.
        assert_eq!(
            current_file_path(Some("Ghost".to_string()), "Ghost", &files).cloned(),
            None
        );
        // No last-opened record at all: no target.
        assert_eq!(current_file_path(None, "My Film", &files).cloned(), None);
        // A differing record: no target (the §31 guard).
        assert_eq!(
            current_file_path(Some("Other Film".to_string()), "Untitled", &files)
                .cloned(),
            None
        );
    }

    #[test]
    fn reload_without_a_target_is_refused_and_changes_nothing() {
        let mut core = EditorCore::new();
        core.project.name = "Unsaved Work".into();
        let depth = core.undo_depth();
        assert!(!reload_from_disk(&mut core, None));
        // The refusal names itself, and that is all that
        // happens: open_project_file is never called, the
        // project is untouched, no snapshot is taken.
        assert_eq!(core.status, "No saved file to reload.");
        assert_eq!(core.project.name, "Unsaved Work");
        assert_eq!(core.undo_depth(), depth);
    }

    #[test]
    fn reload_with_a_target_outside_the_list_is_refused() {
        // open_project_file opens by index into the core's
        // list, so a path the list does not hold cannot be
        // reloaded — the same refusal as no target, and the
        // open is never attempted.
        let mut core = EditorCore::new();
        core.project.name = "Unsaved Work".into();
        core.project_files = vec![PathBuf::from("./projects/alpha.json")];
        let elsewhere = PathBuf::from("./projects/elsewhere.json");
        assert!(!reload_from_disk(&mut core, Some(elsewhere.as_path())));
        assert_eq!(core.status, "No saved file to reload.");
        assert_eq!(core.project.name, "Unsaved Work");
    }

    #[test]
    fn reload_reads_the_saved_file_and_names_the_discard() {
        // A real saved file: a demo project under the name
        // "Disk Film", serialised exactly as a named save
        // writes it.
        let path = reload_scratch("disk-film");
        let mut saved = Project::new_demo();
        saved.name = "Disk Film".into();
        std::fs::write(&path, serde_json::to_string(&saved).expect("serialise"))
            .expect("write scratch project");

        // The session has since diverged: an unsaved rename
        // the reload must throw away.
        let mut core = EditorCore::new();
        core.project_files = vec![path.clone()];
        core.project.name = "Unsaved Rename".into();

        assert!(reload_from_disk(&mut core, Some(path.as_path())));
        // The core's index was pointed at the file (the open
        // op's mechanism), the saved project is now open,
        // and the status names the discard outright.
        assert_eq!(core.project_index, 0);
        assert_eq!(core.project.name, "Disk Film");
        assert_eq!(
            core.status,
            "Reloaded Disk Film from disk — unsaved changes discarded."
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn reload_of_an_unreadable_file_keeps_the_ops_failure_status() {
        // A listed file whose contents are not a project:
        // the open fails, so nothing was discarded and the
        // status must not claim a reload — the op's own
        // failure wording stands.
        let path = reload_scratch("corrupt");
        std::fs::write(&path, "this is not project json").expect("write scratch file");
        let mut core = EditorCore::new();
        core.project_files = vec![path.clone()];
        core.project.name = "Unsaved Rename".into();

        assert!(!reload_from_disk(&mut core, Some(path.as_path())));
        assert_eq!(core.project.name, "Unsaved Rename");
        assert!(core.status.starts_with("Open failed:"), "{}", core.status);
        let _ = std::fs::remove_file(&path);
    }
}
