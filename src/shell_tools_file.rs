//! Shell-control MCP handlers — File menu + Home /
//! Project file actions (Wave 14, Squad B, agent B9).
//! Declared from `main.rs` by the integrator; bin-crate
//! code, not part of the engine library.
//!
//! The handlers here drive the GUI shell's file-level
//! actions — [`crate::State`] in hand — rather than the
//! engine core. Like B3's handlers
//! (`shell_tools_state.rs`, whose entry-spec format this
//! file follows) they cannot be ordinary registry
//! entries: the registry's handler type is
//! `fn(&mut EditorCore, &Value)` and `State` lives in
//! this binary crate. B2's shell-command plumbing routes
//! live-endpoint calls for these names through the shell
//! command queue, whose drain — in the shell's poll
//! loop, on the UI thread — calls the handlers below.
//!
//! **Live-only.** Every tool in this file exists only
//! on the live HTTP endpoint hosted by the running
//! editor. The stdio server has no shell — no recents
//! list, no Home scan cache, no confirm-arm state — so
//! it refuses each name with the wording proposed in
//! the entry specs below: an error result, never a
//! silent no-op.
//!
//! ---- What the audit found (menus.rs + home_panel.rs
//! ---- + project_tools.rs, Wave 13 closing tree) ----
//!
//! * **Open recent is a genuinely shell path.** The
//!   core io tool `load_project` loads an explicit path
//!   (`mcp_io.rs`: read + parse + `replace_project_open`
//!   + `note_project_loaded` + `touch_recent`). The File
//!   menu's press (`menus::open_recent`,
//!   `src/rune_sec/menus.rs:709`) is addressed by INDEX
//!   into the shell's recents list, resolves the name
//!   against the Home panel's cached library scan
//!   (exact stored-name match, then file-stem fallback —
//!   `menus::recent_items`), and after the open applies
//!   shell-only follow-ups: `clear_stale_solo`,
//!   `sync_name_edit`, and clearing the stroke
//!   selection. On the two points the audit was asked
//!   to check: (1) **recovery-note behaviour does NOT
//!   differ** — neither path writes the recovery file
//!   on open (`note_recovery_written` is a save-path
//!   note only); both restart the dirty baseline via
//!   `note_project_swapped`, called inside the open op
//!   itself. (2) **Selection resets DO differ in
//!   mechanism**: the core open resets core selection
//!   fields; the shell path additionally clears the
//!   shell's stroke selection and re-syncs the name
//!   field (on the live endpoint the poll's
//!   `project_swaps` watcher independently settles
//!   panel selections after any swap — Wave 11 E9 —
//!   but the menu path does its own, immediately).
//! * **Reload from disk and Recover snapshot have no
//!   core tool at all.** Both live only as Project
//!   section / Home banner presses
//!   (`project_tools::reload_pressed` /
//!   `recover_pressed`), each behind the shell's
//!   two-press discard gate (`crate::ConfirmAction`).
//!   The handlers below replicate those presses
//!   verbatim, gate included: a first call on a dirty
//!   session arms the same `ConfirmAction` the button
//!   arms and reports `armed: true`; the identical
//!   second call runs the action. Reload's target is
//!   the shell's last-opened record
//!   (`project_tools::reload_target` reads
//!   `shell::last_opened_project`) — shell data no core
//!   tool can see.
//! * **Home's per-row Duplicate / Delete act on ANY
//!   library file.** The registry's `duplicate_project`
//!   duplicates only the current project's file; no
//!   core tool deletes a project file. The handlers
//!   below resolve a row by its stored name against
//!   the same scan the rows render from and call the
//!   panel's own dispatches
//!   (`home_panel::dispatch_duplicate`,
//!   `arm_delete` + `dispatch_delete_confirm` — the
//!   delete's arm lives in the panel state itself, so
//!   a tool arm and a GUI confirm interoperate).
//! * **Save a copy is NOT distinct** — the registry's
//!   `save_project_copy` already performs the panel's
//!   action exactly (derived `"{name} copy"`,
//!   create-new semantics, selector refresh), so no
//!   shell tool is written for it.
//! * **The View menu's sidebar toggles** flip
//!   `State::show_left` / `show_right` and nothing
//!   else; B8's `shell_get_layout` reads them but no
//!   sibling tool writes them, so they are written
//!   here (`shell_set_sidebar`). Every other menu item
//!   is already tool-covered or owned by a sibling
//!   squad — the full table is in B9's report.
//!
//! ---- Standing exclusions honoured ------------------
//!
//! * **Project-swap undo semantics: untouched.** The
//!   open / reload / recover handlers call the GUI's
//!   own swap paths verbatim (`dispatch_open`,
//!   `reload_from_disk`, `recover_snapshot`); whatever
//!   those do to the undo stacks today is exactly what
//!   they do through these tools. No swap behaviour is
//!   reimplemented or adjusted here.
//! * **MCP-save `write_recovery` behaviour:
//!   untouched.** Nothing in this file saves. Recover
//!   only READS `./projects/.recovery.json` through
//!   the panel's own `recover_snapshot`.
//! * **Dialog-bound actions get NO tool** (named
//!   exclusions, in B9's report): the app bar's Load
//!   button and any native file picker — a headless
//!   client cannot answer a native dialog, and the
//!   by-path capability already exists as the core
//!   `load_project` / `import_file` tools.
//!
//! ---- Undo / dirty discipline ------------------------
//!
//! Project swaps are not undo steps in the GUI (the
//! swap ops take no snapshot — `reload_from_disk`'s
//! own doc: "neither the rows' open nor this reload
//! creates an undo step for a project swap"), and the
//! file actions touch no project data, so no handler
//! here takes an undo step or marks dirty. That is the
//! house contract kept, not broken: undo snapshots
//! store the `Project` only (Wave 11), and a swap
//! restarts the dirty baseline through the op's own
//! `note_project_swapped`.
//!
//! Integrator splice (one line, `src/main.rs` with the
//! other `mod` declarations):
//!
//! ```ignore
//! mod shell_tools_file;
//! ```

use std::path::PathBuf;

use serde_json::{json, Value};
use stykanimate_rs::shell;

use crate::rune_sec::{home_panel, menus, project_tools};
use crate::{ConfirmAction, State};

/// A required string argument, in the registry
/// handlers' own wording (`mcp_tools.rs`'s `req_str`,
/// which is library-private and cannot be reused from
/// here) — same helper as B3's file.
fn req_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    match args.get(key) {
        Some(v) => v
            .as_str()
            .ok_or_else(|| format!("argument '{key}' must be a string")),
        None => Err(format!("missing required argument '{key}'")),
    }
}

/// An optional boolean argument: absent means false,
/// a non-boolean value is refused rather than guessed.
fn opt_bool(args: &Value, key: &str) -> Result<bool, String> {
    match args.get(key) {
        None => Ok(false),
        Some(v) => v
            .as_bool()
            .ok_or_else(|| format!("argument '{key}' must be a boolean")),
    }
}

/// A whole-number index argument, validated the way
/// the sprite handlers validate their frame argument:
/// an unsigned or non-negative integer, or a float
/// with no fractional part. Anything else (negative,
/// fractional, wrong type) is refused, quoting the
/// value.
fn req_index(args: &Value, key: &str) -> Result<usize, String> {
    let raw = args
        .get(key)
        .ok_or_else(|| format!("missing required argument: {key}"))?;
    if let Some(v) = raw.as_u64() {
        return Ok(v as usize);
    }
    if let Some(v) = raw.as_i64() {
        if v >= 0 {
            return Ok(v as usize);
        }
        return Err(format!("argument '{key}' must not be negative (got {v})"));
    }
    if let Some(v) = raw.as_f64() {
        if v.fract() != 0.0 {
            return Err(format!(
                "argument '{key}' must be a whole number (got {v})"
            ));
        }
        if v < 0.0 {
            return Err(format!("argument '{key}' must not be negative (got {v})"));
        }
        return Ok(v as usize);
    }
    Err(format!("argument '{key}' must be a whole number (got {raw})"))
}

/// The File menu's recents, resolved exactly as the
/// menu resolves them: the shell's recents list
/// (`shell::recents` — display names, most recent
/// first) paired against the Home panel's cached
/// library scan filtered to the entries its rows show
/// (`menus::recent_file_pairs`' own construction,
/// reproduced because that fn is private to menus.rs;
/// the pairing rule itself is the pub
/// `menus::recent_items`). A name whose file is gone
/// pairs with `None` and stays listed — the menu's
/// deliberate discipline, kept here.
fn resolved_recents(s: &State) -> Vec<(String, Option<PathBuf>)> {
    let entries = s.home_panel.projects();
    let files: Vec<(String, PathBuf)> = home_panel::visible_projects(&entries, "")
        .into_iter()
        .map(|info| (info.name, info.path))
        .collect();
    menus::recent_items(&shell::recents(), &files)
}

/// The recents list as JSON, for replies: every entry
/// with its menu index, its name, its resolved path
/// (null when the file is gone) and a `missing` flag.
fn recents_json(recents: &[(String, Option<PathBuf>)]) -> Value {
    Value::Array(
        recents
            .iter()
            .enumerate()
            .map(|(i, (name, path))| {
                json!({
                    "index": i,
                    "name": name,
                    "path": path.as_ref().map(|p| p.to_string_lossy().into_owned()),
                    "missing": path.is_none(),
                })
            })
            .collect(),
    )
}

/// Resolve a Home library row by its stored name
/// against the scan the rows render from (the visible
/// entries, no query). Pure over the entries slice so
/// the rule is unit-testable without a shell: exact
/// stored-name match, first in scan order. Returns the
/// row's path.
fn resolve_library_row(
    entries: &[stykanimate_rs::projects::ProjectInfo],
    name: &str,
) -> Option<PathBuf> {
    home_panel::visible_projects(entries, "")
        .into_iter()
        .find(|info| info.name == name)
        .map(|info| info.path)
}

// TOOL ENTRY SPEC (for B2's live-endpoint registration
// and B10/E3's MCP.md pass — this handler's signature
// takes `&mut State`, so it is NOT a `mcp::Tool` entry;
// register it in the live shell-command table beside
// B2's other shell tools, under exactly this name,
// description and schema):
//
//   name: "shell_list_recents"
//   description: "List the running editor's recent
//     projects — the File menu's Open recent entries,
//     most recent first, at most five: each with its
//     menu index, its display name, the project file
//     it resolves to (exact stored-name match, then
//     file-stem fallback, against the Home library
//     scan) or null with missing: true when the file
//     is gone. Read-only; no undo step. Live endpoint
//     only — the stdio server refuses this tool."
//   schema: obj_schema(json!({}), &[])
//   handler: crate::shell_tools_file::tool_shell_list_recents
//
//   stdio refusal (proposed wording for B1/B2):
//     "shell_list_recents is live-only: the recents
//     list belongs to the running editor's shell and
//     has no meaning for the stdio server — call it
//     through the editor's live endpoint instead."
//
//   MCP.md catalogue row (the "Shell control — live
//   endpoint only" table; columns Tool | Purpose |
//   Required | Optional):
//   | `shell_list_recents` | List the File menu's Open recent entries (at most five, most recent first): index, display name, resolved project file or `missing: true` when the file is gone. Read-only. Live endpoint only. | — | — |

/// MCP handler (live endpoint only): list the File
/// menu's Open recent entries, resolved — the read
/// side of [`tool_shell_open_recent`]. Read-only; no
/// undo step.
pub fn tool_shell_list_recents(s: &mut State, _args: &Value) -> Result<Value, String> {
    let recents = resolved_recents(s);
    Ok(json!({
        "ok": true,
        "count": recents.len(),
        "recents": recents_json(&recents),
    }))
}

// TOOL ENTRY SPEC (same registration route as above):
//
//   name: "shell_open_recent"
//   description: "Open a recent project by its File
//     menu index (0-based, most recent first; at most
//     five entries — see shell_list_recents). The
//     menu's own press, verbatim: the name resolves
//     against the Home library scan (exact stored
//     name, then file stem), the file opens through
//     the shell's open path (selector insert-or-select,
//     then the core open, recents re-touched), and the
//     shell follow-ups run (stale-solo guard, name
//     field re-sync, stroke selection cleared). An
//     entry whose file is gone is refused with the
//     menu's own not-found wording and changes
//     nothing; an out-of-range index is refused,
//     quoting the index and the entry count. Like the
//     menu press, this replaces the session WITHOUT
//     the Home rows' two-press discard confirm — a
//     client that wants the guard checks the dirty
//     state first (get_editor_status). A project swap
//     is not an undo step (the GUI's opens take none).
//     Live endpoint only — the stdio server refuses
//     this tool."
//   schema: obj_schema(
//       json!({ "index": { "type": "integer" } }),
//       &["index"],
//   )
//   handler: crate::shell_tools_file::tool_shell_open_recent
//
//   stdio refusal (proposed wording for B1/B2):
//     "shell_open_recent is live-only: it opens a
//     project from the running editor's recents list
//     and has no meaning for the stdio server — call
//     it through the editor's live endpoint, or use
//     load_project with an explicit path instead."
//
//   MCP.md catalogue row:
//   | `shell_open_recent` | Open a recent project by its File menu index (0-based; see `shell_list_recents`): the menu's own press — scan resolution, the shell open path and its follow-ups (stale-solo guard, name re-sync, stroke selection cleared). Unresolved entries and out-of-range indices are refused, quoting the value. Replaces the session without a discard confirm, like the menu. Not an undo step. Live endpoint only. | `index` | — |

/// MCP handler (live endpoint only): open the recent
/// project at `index` — `menus::open_recent`'s body
/// verbatim (that fn is private to menus.rs; the steps
/// below are its steps, in its order, over the same
/// pub dispatches). Not an undo step (see the module
/// docs).
pub fn tool_shell_open_recent(s: &mut State, args: &Value) -> Result<Value, String> {
    let index = req_index(args, "index")?;
    let recents = resolved_recents(s);
    let Some((name, path)) = recents.get(index).cloned() else {
        return Err(format!(
            "recent index {index} is out of range: the File menu lists {} recent project(s)",
            recents.len()
        ));
    };
    match path {
        Some(path) => {
            home_panel::dispatch_open(&mut s.core, &path, &name);
            project_tools::clear_stale_solo(&mut s.core);
            project_tools::sync_name_edit(s);
            s.stroke_selected = None;
            Ok(json!({
                "ok": true,
                "opened": name,
                "path": path.to_string_lossy(),
                "project": s.core.project.name,
                "status": s.core.status,
                "recents": recents_json(&resolved_recents(s)),
            }))
        }
        None => Err(format!("Recent project not found: {name}")),
    }
}

// TOOL ENTRY SPEC (same registration route as above):
//
//   name: "shell_reload_project"
//   description: "Reload the current project from its
//     saved file, discarding unsaved changes — the
//     Project section's Reload from disk press,
//     verbatim, including its two-press discard gate:
//     on a dirty session the first call only arms the
//     reload (reply ok: false, armed: true, with the
//     button's own arming wording in status) and the
//     identical second call runs it. The target is
//     the file the shell's last-opened record names
//     for this project; with no saved file the call
//     is refused ('No saved file to reload.'). The
//     reload itself is the core open of that file,
//     with the discard named in the resulting status.
//     A project swap is not an undo step. Live
//     endpoint only — the stdio server refuses this
//     tool."
//   schema: obj_schema(json!({}), &[])
//   handler: crate::shell_tools_file::tool_shell_reload_project
//
//   stdio refusal (proposed wording for B1/B2):
//     "shell_reload_project is live-only: its target
//     comes from the running editor shell's
//     last-opened record and its discard gate is
//     shell state — call it through the editor's live
//     endpoint instead."
//
//   MCP.md catalogue row:
//   | `shell_reload_project` | Reload the current project from its saved file (the Project section's Reload from disk), discarding unsaved changes. Two-press gate: on a dirty session the first call arms (`armed: true`) and the second runs. Refused when the project has no saved file. Not an undo step. Live endpoint only. | — | — |

/// MCP handler (live endpoint only): the Project
/// section's Reload press — `project_tools::
/// reload_pressed`'s body verbatim (that fn is
/// private; its steps run here over the same pub
/// helpers), arm gate included. Not an undo step.
pub fn tool_shell_reload_project(s: &mut State, _args: &Value) -> Result<Value, String> {
    let target = project_tools::reload_target(&s.core);
    if target.is_some()
        && s.confirm_action != Some(ConfirmAction::ReloadProject)
        && project_tools::session_dirty(&s.core)
    {
        s.confirm_action = Some(ConfirmAction::ReloadProject);
        s.core.status =
            "Reloading would discard unsaved changes — press Reload again to confirm.".into();
        return Ok(json!({
            "ok": false,
            "armed": true,
            "status": s.core.status,
        }));
    }
    s.confirm_action = None;
    let landed = project_tools::reload_from_disk(&mut s.core, target.as_deref());
    s.stroke_selected = None;
    project_tools::sync_name_edit(s);
    if landed {
        Ok(json!({
            "ok": true,
            "armed": false,
            "project": s.core.project.name,
            "status": s.core.status,
        }))
    } else {
        Err(s.core.status.clone())
    }
}

// TOOL ENTRY SPEC (same registration route as above):
//
//   name: "shell_recover_snapshot"
//   description: "Recover the autosave snapshot
//     (./projects/.recovery.json) as the current
//     project — the Project section's / Home banner's
//     Recover press, verbatim, including its two-press
//     discard gate: on a dirty session the first call
//     only arms (reply ok: false, armed: true, with
//     the button's own arming wording in status) and
//     the identical second call runs it. With no
//     snapshot on disk the call is refused ('No
//     recovery snapshot found in ./projects/.'). The
//     recovery file is only READ — this tool never
//     writes it. A project swap is not an undo step.
//     Live endpoint only — the stdio server refuses
//     this tool."
//   schema: obj_schema(json!({}), &[])
//   handler: crate::shell_tools_file::tool_shell_recover_snapshot
//
//   stdio refusal (proposed wording for B1/B2):
//     "shell_recover_snapshot is live-only: it
//     replaces the running editor's session and its
//     discard gate is shell state — call it through
//     the editor's live endpoint instead."
//
//   MCP.md catalogue row:
//   | `shell_recover_snapshot` | Recover the autosave snapshot (`./projects/.recovery.json`) as the current project (the Recover press), replacing the session. Two-press gate: on a dirty session the first call arms (`armed: true`) and the second runs. Refused when no snapshot exists. The recovery file is read, never written. Not an undo step. Live endpoint only. | — | — |

/// MCP handler (live endpoint only): the Recover
/// press — `project_tools::recover_pressed`'s body
/// verbatim (that fn is private; its steps run here
/// over the same pub `recover_snapshot`), arm gate
/// included. Reads the recovery file; never writes
/// it. Not an undo step.
pub fn tool_shell_recover_snapshot(s: &mut State, _args: &Value) -> Result<Value, String> {
    if s.confirm_action != Some(ConfirmAction::RecoverSnapshot)
        && project_tools::session_dirty(&s.core)
    {
        s.confirm_action = Some(ConfirmAction::RecoverSnapshot);
        s.core.status =
            "Recovering the snapshot would discard unsaved changes — press Recover again to confirm."
                .into();
        return Ok(json!({
            "ok": false,
            "armed": true,
            "status": s.core.status,
        }));
    }
    s.confirm_action = None;
    let recovered = project_tools::recover_snapshot(&mut s.core);
    s.stroke_selected = None;
    project_tools::sync_name_edit(s);
    if recovered {
        Ok(json!({
            "ok": true,
            "armed": false,
            "project": s.core.project.name,
            "status": s.core.status,
        }))
    } else {
        Err(s.core.status.clone())
    }
}

// TOOL ENTRY SPEC (same registration route as above):
//
//   name: "shell_duplicate_project_file"
//   description: "Duplicate a saved project file from
//     the Home library — any row, addressed by its
//     stored name (see list_projects): the copy lands
//     beside the source as '<stem>-copy.json',
//     incrementing if taken, exactly as the row's
//     Duplicate button does it, and the session's
//     cached file list is refreshed to name the copy.
//     The current session is untouched (the panel's
//     semantics; distinct from the duplicate_project
//     tool, which duplicates only the current
//     project's file). An unknown name is refused,
//     quoting it. Not an undo step; does not mark the
//     session dirty. Live endpoint only — the stdio
//     server refuses this tool."
//   schema: obj_schema(
//       json!({ "name": { "type": "string" } }),
//       &["name"],
//   )
//   handler: crate::shell_tools_file::tool_shell_duplicate_project_file
//
//   stdio refusal (proposed wording for B1/B2):
//     "shell_duplicate_project_file is live-only: it
//     addresses rows of the running editor's Home
//     library — call it through the editor's live
//     endpoint instead."
//
//   MCP.md catalogue row:
//   | `shell_duplicate_project_file` | Duplicate any saved project file from the Home library, addressed by its stored name: the copy lands beside the source as `<stem>-copy.json` (incrementing if taken), as the row's Duplicate button does it. The session is untouched. An unknown name is refused, quoting it. Not an undo step. Live endpoint only. | `name` | — |

/// MCP handler (live endpoint only): a Home row's
/// Duplicate press — the row resolved by stored name
/// against the live scan, then the panel's own
/// `dispatch_duplicate`. Not an undo step; the
/// session project is not touched.
pub fn tool_shell_duplicate_project_file(
    s: &mut State,
    args: &Value,
) -> Result<Value, String> {
    let name = req_str(args, "name")?;
    let entries = s.home_panel.projects();
    let Some(path) = resolve_library_row(&entries, name) else {
        return Err(format!(
            "no saved project named '{name}' in the Home library"
        ));
    };
    drop(entries);
    home_panel::dispatch_duplicate(&mut s.core, &path);
    let status = s.core.status.clone();
    if status.starts_with("Duplicated project: ") {
        Ok(json!({
            "ok": true,
            "name": name,
            "status": status,
        }))
    } else {
        Err(status)
    }
}

// TOOL ENTRY SPEC (same registration route as above):
//
//   name: "shell_delete_project_file"
//   description: "Delete a saved project file from
//     the Home library, addressed by its stored name
//     (see list_projects) — the row's Delete button
//     and its confirm line, as a two-call protocol
//     over the panel's own arm state: the first call
//     (no 'confirm') only arms the deletion (reply
//     ok: false, armed: true) and touches no file;
//     the second call, with 'confirm': true, carries
//     it out — but only if the arm for THIS name is
//     the one standing, otherwise it is refused. A
//     GUI arm and a tool confirm (or the reverse)
//     interoperate, since both use the panel's arm.
//     Shell-owned files (the recovery snapshot, the
//     recents and layout files) are refused by the
//     shell's own guard. Only the file is removed:
//     deleting the currently open project's file
//     leaves the session's in-memory copy exactly as
//     it is, as in the GUI. An unknown name is
//     refused, quoting it. Not an undo step. Live
//     endpoint only — the stdio server refuses this
//     tool."
//   schema: obj_schema(
//       json!({
//           "name": { "type": "string" },
//           "confirm": { "type": "boolean" },
//       }),
//       &["name"],
//   )
//   handler: crate::shell_tools_file::tool_shell_delete_project_file
//
//   stdio refusal (proposed wording for B1/B2):
//     "shell_delete_project_file is live-only: it
//     addresses rows of the running editor's Home
//     library — call it through the editor's live
//     endpoint instead."
//
//   MCP.md catalogue row:
//   | `shell_delete_project_file` | Delete any saved project file from the Home library, addressed by its stored name. Two calls over the panel's own arm state: the first arms (`armed: true`, no file touched), the second with `confirm: true` deletes — only while the arm for that name stands. Shell-owned files are refused by the shell's guard. Deleting the open project's file leaves the session's copy untouched. Not an undo step. Live endpoint only. | `name` | `confirm` |

/// MCP handler (live endpoint only): a Home row's
/// Delete press + confirm line, over the panel's own
/// arm (`home_panel::arm_delete` /
/// `dispatch_delete_confirm`) — see the entry spec
/// for the two-call protocol. Not an undo step.
pub fn tool_shell_delete_project_file(s: &mut State, args: &Value) -> Result<Value, String> {
    let name = req_str(args, "name")?;
    let confirm = opt_bool(args, "confirm")?;
    let entries = s.home_panel.projects();
    let Some(path) = resolve_library_row(&entries, name) else {
        return Err(format!(
            "no saved project named '{name}' in the Home library"
        ));
    };
    drop(entries);
    if !confirm {
        home_panel::arm_delete(&mut s.home_panel, path.clone());
        return Ok(json!({
            "ok": false,
            "armed": true,
            "name": name,
            "path": path.to_string_lossy(),
            "status": format!(
                "Delete armed for '{name}' — call again with \"confirm\": true to delete the file. No file has been touched."
            ),
        }));
    }
    if s.home_panel.confirm_delete.as_deref() != Some(path.as_path()) {
        return Err(format!(
            "no deletion is armed for '{name}' — call once without 'confirm' to arm it"
        ));
    }
    home_panel::dispatch_delete_confirm(&mut s.core, &mut s.home_panel, name);
    let status = s.core.status.clone();
    if status.starts_with("Deleted project file ") {
        Ok(json!({
            "ok": true,
            "armed": false,
            "name": name,
            "status": status,
        }))
    } else {
        Err(status)
    }
}

// TOOL ENTRY SPEC (same registration route as above):
//
//   name: "shell_set_sidebar"
//   description: "Show or hide one of the editor's
//     side columns — the View menu's 'Figures sidebar'
//     (side: 'left') and 'Inspector sidebar' (side:
//     'right') items: flips the same State fields the
//     menu items flip, nothing else; the columns'
//     contents and widths are untouched (widths are
//     B8's shell_set_column_widths; visibility is
//     read back by B8's shell_get_layout). An unknown
//     side is refused, quoting it. Session state only:
//     no undo step, never marks dirty. Live endpoint
//     only — the stdio server refuses this tool."
//   schema: obj_schema(
//       json!({
//           "side": { "type": "string", "enum": ["left", "right"] },
//           "visible": { "type": "boolean" },
//       }),
//       &["side", "visible"],
//   )
//   handler: crate::shell_tools_file::tool_shell_set_sidebar
//
//   stdio refusal (proposed wording for B1/B2):
//     "shell_set_sidebar is live-only: it flips the
//     running editor's column visibility and has no
//     meaning for the stdio server — call it through
//     the editor's live endpoint instead."
//
//   MCP.md catalogue row:
//   | `shell_set_sidebar` | Show or hide a side column — the View menu's Figures sidebar (`side: "left"`) and Inspector sidebar (`side: "right"`) items, flipping the same shell fields. Contents and widths untouched (see `shell_get_layout` / `shell_set_column_widths`). An unknown side is refused, quoting it. Session state only: no undo step. Live endpoint only. | `side`, `visible` | — |

/// MCP handler (live endpoint only): the View menu's
/// sidebar items — the field flips verbatim
/// (`s.show_left = !s.show_left` /
/// `s.show_right = !s.show_right` in `menus.rs`,
/// expressed as sets so a tool call is idempotent
/// rather than a blind toggle). Session state only.
pub fn tool_shell_set_sidebar(s: &mut State, args: &Value) -> Result<Value, String> {
    let side = req_str(args, "side")?;
    let visible = match args.get("visible") {
        Some(v) => v
            .as_bool()
            .ok_or_else(|| "argument 'visible' must be a boolean".to_string()),
        None => Err("missing required argument 'visible'".to_string()),
    }?;
    match side {
        "left" => s.show_left = visible,
        "right" => s.show_right = visible,
        other => {
            return Err(format!(
                "unknown sidebar '{other}': valid sides are left, right"
            ))
        }
    }
    Ok(json!({
        "ok": true,
        "side": side,
        "visible": visible,
        "show_left": s.show_left,
        "show_right": s.show_right,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_index_accepts_whole_numbers_and_refuses_the_rest() {
        assert_eq!(req_index(&json!({ "index": 0 }), "index"), Ok(0));
        assert_eq!(req_index(&json!({ "index": 4 }), "index"), Ok(4));
        assert_eq!(req_index(&json!({ "index": 2.0 }), "index"), Ok(2));
        let err = req_index(&json!({ "index": 1.5 }), "index")
            .expect_err("a fractional index must be refused");
        assert!(err.contains("whole number"), "err: {err}");
        let err = req_index(&json!({ "index": -1 }), "index")
            .expect_err("a negative index must be refused");
        assert!(err.contains("must not be negative"), "err: {err}");
        let err = req_index(&json!({ "index": "0" }), "index")
            .expect_err("a string index must be refused");
        assert!(err.contains("whole number"), "err: {err}");
        let err = req_index(&json!({}), "index")
            .expect_err("a missing index must be refused");
        assert!(err.contains("missing required argument"), "err: {err}");
    }

    #[test]
    fn recents_json_marks_missing_files_and_keeps_menu_order() {
        let recents = vec![
            ("Alpha".to_string(), Some(PathBuf::from("projects/alpha.json"))),
            ("Gone".to_string(), None),
        ];
        let out = recents_json(&recents);
        assert_eq!(out[0]["index"], json!(0));
        assert_eq!(out[0]["name"], json!("Alpha"));
        assert_eq!(out[0]["missing"], json!(false));
        assert_eq!(out[1]["index"], json!(1));
        assert_eq!(out[1]["path"], Value::Null);
        assert_eq!(out[1]["missing"], json!(true));
    }

    #[test]
    fn set_sidebar_flips_only_the_named_column() {
        let mut s = crate::init();
        let (left0, right0) = (s.show_left, s.show_right);
        let out = tool_shell_set_sidebar(
            &mut s,
            &json!({ "side": "left", "visible": !left0 }),
        )
        .expect("the left flip succeeds");
        assert_eq!(s.show_left, !left0);
        assert_eq!(s.show_right, right0, "the other column is untouched");
        assert_eq!(out["show_left"], json!(!left0));
        // Idempotent as a set, unlike the menu's toggle:
        // repeating the call changes nothing further.
        tool_shell_set_sidebar(&mut s, &json!({ "side": "left", "visible": !left0 }))
            .expect("the repeat succeeds");
        assert_eq!(s.show_left, !left0);
        let err = tool_shell_set_sidebar(
            &mut s,
            &json!({ "side": "middle", "visible": true }),
        )
        .expect_err("an unknown side must be refused");
        assert!(err.contains("'middle'"), "err: {err}");
    }

    #[test]
    fn open_recent_refuses_an_out_of_range_index_before_touching_anything() {
        // `crate::init()`'s shell has whatever recents the
        // developer's ./projects holds — the refusal is
        // index-relative, so an index beyond the menu cap
        // (MAX_RECENT_ITEMS = 5) refuses on any machine.
        let mut s = crate::init();
        let name_before = s.core.project.name.clone();
        let err = tool_shell_open_recent(&mut s, &json!({ "index": 99 }))
            .expect_err("index 99 is beyond any recents list");
        assert!(err.contains("recent index 99 is out of range"), "err: {err}");
        assert_eq!(s.core.project.name, name_before, "nothing was opened");
        assert_eq!(s.stroke_selected, None);
    }

    #[test]
    fn delete_project_file_refuses_an_unknown_name_unarmed() {
        let mut s = crate::init();
        let err = tool_shell_delete_project_file(
            &mut s,
            &json!({ "name": "b9-no-such-project-zz", "confirm": true }),
        )
        .expect_err("an unknown library name must be refused");
        assert!(
            err.contains("no saved project named 'b9-no-such-project-zz'"),
            "err: {err}"
        );
        assert!(
            s.home_panel.confirm_delete.is_none(),
            "a refused call arms nothing"
        );
    }
}
