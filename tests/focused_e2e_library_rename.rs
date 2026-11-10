//! Focused Wave (post-Wave 14), Squad A (session A3) —
//! integration tests: the `figure_library_rename` MCP tool,
//! driven end-to-end through real MCP dispatch
//! (`McpServer::handle`, the stdio surface — this is a core
//! registry tool, NOT a shell tool, so the stdio dispatch
//! reaches it exactly as the live dispatch does; Wave 14's
//! transport-parity suite pins that the two dispatches are
//! one router).
//!
//! The tool under test (Squad A1, `src/library_ops.rs`):
//! `figure_library_rename`, handler
//! `crate::library_ops::tool_figure_library_rename` — the
//! library panel's Rename (`rename_saved`,
//! `src/rune_sec/library_panel.rs`) given a lib home and an
//! MCP surface. It renames a saved figure-library entry:
//! the backing file on disk first (`std::fs::rename`),
//! then the in-core `core.library` entry, which keeps its
//! slot. The refusal rules are the panel's, verbatim:
//! built-in entries, a name that sanitises to empty, a
//! collision with another entry's stem, and a target file
//! that already exists in the library folder (even one the
//! loader skipped, which `fs::rename` would silently
//! overwrite on Unix).
//!
//! ARGUMENT SCHEMA, as briefed: A1's remit names
//! `pose_library_rename` (`src/figure_ops.rs`) as the
//! pattern, so the calls below pass that tool's schema —
//! `name` (the entry's current name) + `new_name` (the raw
//! new name, sanitised by the tool) — and ALSO pass the
//! entry's `index`, which the panel op is addressed by and
//! `figure_library_insert` already accepts alongside
//! `name`. Both identifiers always denote the same entry,
//! so either addressing reads the same rename. If the
//! landed handler addresses entries some third way, the
//! first call in the happy-path test fails loudly and this
//! paragraph names the assumption to reconcile.
//!
//! SEEDING — purely through MCP, no fixture state inside
//! the core: a saved (non-built-in) library entry is
//! created exactly the way a client's would be, with
//! `add_figure` (a figure with a known name) followed by
//! `figure_library_save` (the panel Save's tool: writes
//! `./library/<stem>.json` and reloads the library, so the
//! entry appears under the figure's exact name — stored
//! inside the file since Wave V (V1); the file keeps the
//! slug). The one
//! fixture that is NOT an MCP call is the scratch
//! directory itself, and it is forced by the subsystem's
//! shape: the library folder is `./library` relative to
//! the process current directory (`load_library`,
//! `library_file_for`, `save_figure_to_library` all
//! resolve it from `std::env::current_dir()`), so each
//! test enters its own temp dir with the Wave 11/12
//! suites' `CwdGuard` pattern and constructs its server
//! only afterwards — a fresh temp dir has no `library/`
//! folder, so every core starts from exactly the three
//! built-ins and no test can see another's files, the
//! repo's or the developer's. Because the current
//! directory is process-global, the tests in this file
//! serialise on one mutex (the same medicine Wave 14's
//! bridge-singleton tests take). Direct filesystem access
//! is used only for what an MCP client observes from
//! outside the protocol: that the old entry file is gone
//! and the renamed file exists, and one deliberately
//! unparseable `.json` written into the folder as the
//! target-file-exists fixture (the loader skips it, so it
//! is a file with no entry — the one state no MCP call
//! can manufacture).
//!
//! THE UNDO CONTRACT PINNED HERE: a library rename
//! spends NO undo step, and Undo does not revert it.
//! Evidence, read from source before writing:
//!   * the panel op spends none — `rename_saved` calls
//!     no `snapshot()`; it renames the file, updates the
//!     vec entry and sets `core.status`, nothing else;
//!   * the pattern tool's own doc says it outright:
//!     `tool_pose_library_rename` is "A file operation,
//!     NOT an undo step", as are `pose_library_save`,
//!     `pose_library_delete` and `figure_library_save`
//!     ("A file write, not an undo step");
//!   * the undo stack stores Project snapshots only, and
//!     the library is core state outside the project — a
//!     snapshot could not record the rename even if one
//!     were taken, and the file on disk is beyond Undo's
//!     reach by construction.
//! The undo test below therefore asserts the depth is
//! unchanged across the rename, then proves the negative
//! is not vacuous: a real project edit IS undone by the
//! `undo` tool while the renamed entry (and its file)
//! stay renamed.
//!
//! Every test builds its own server, temp dir and library
//! and shares nothing but the serialisation lock.

use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use stykanimate_rs::mcp::McpServer;

/// Serialises this file's tests: they each change the
/// process current directory (see the module doc), and
/// the current directory is process-global. Poison-tolerant,
/// as in `live_mcp`'s bridge-test lock: a panicking test
/// must not cascade failures into the others.
static LIBRARY_CWD_LOCK: Mutex<()> = Mutex::new(());

fn lock_cwd() -> std::sync::MutexGuard<'static, ()> {
    LIBRARY_CWD_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

// ---------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------

/// A per-test scratch directory under the OS temp dir:
/// stale leftovers from a crashed run are removed up front
/// and the caller removes the directory again when done.
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "styk-focused-e2e-lib-rename-{}-{tag}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Restores the process current directory on drop,
/// including when the test panics (the Wave 11/12 pattern).
struct CwdGuard(PathBuf);

impl CwdGuard {
    fn enter(dir: &Path) -> Self {
        let old = std::env::current_dir().expect("read current dir");
        std::env::set_current_dir(dir).expect("enter temp dir");
        CwdGuard(old)
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.0);
    }
}

/// The library folder inside the scratch dir a test has
/// entered: `./library`, the folder the core resolves.
fn library_dir(dir: &Path) -> PathBuf {
    dir.join("library")
}

// ---------------------------------------------------------------------
// The dispatch driver (the Wave 11 D-suite pattern, carried
// through Waves 12–14 unchanged, plus a raw-response call
// for refusal inspection)
// ---------------------------------------------------------------------

struct CallOutcome {
    payload: Value,
    text: String,
    is_error: bool,
}

struct Dispatch {
    server: McpServer,
    next_id: u64,
}

impl Dispatch {
    fn new() -> Self {
        Dispatch {
            server: McpServer::new(),
            next_id: 1,
        }
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        let response = self
            .server
            .handle(&json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": method,
                "params": params,
            }))
            .expect("a request with an id must be answered");
        assert_eq!(response["jsonrpc"], "2.0", "{method}: bad envelope");
        assert_eq!(response["id"], json!(id), "{method}: id not echoed");
        response
    }

    /// One `tools/call`, returned as the raw response so a
    /// test can distinguish a protocol error (an `error`
    /// member, no `result`) from a tool result.
    fn call_raw(&mut self, name: &str, args: Value) -> Value {
        self.request("tools/call", json!({ "name": name, "arguments": args }))
    }

    fn call(&mut self, name: &str, args: Value) -> CallOutcome {
        let response = self.call_raw(name, args);
        if let Some(error) = response.get("error") {
            return CallOutcome {
                payload: Value::Null,
                text: error["message"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                is_error: true,
            };
        }
        let result = &response["result"];
        let is_error = result["isError"]
            .as_bool()
            .expect("tools/call result carries isError");
        let text = result["content"][0]["text"]
            .as_str()
            .expect("tools/call result carries one text block")
            .to_string();
        let payload =
            serde_json::from_str(&text).unwrap_or_else(|_| Value::String(text.clone()));
        CallOutcome {
            payload,
            text,
            is_error,
        }
    }

    fn call_ok(&mut self, name: &str, args: Value) -> Value {
        let outcome = self.call(name, args);
        assert!(
            !outcome.is_error,
            "tool {name} failed: {}",
            outcome.text
        );
        outcome.payload
    }
}

// ---------------------------------------------------------------------
// Library reads — through `figure_library_list` only
// ---------------------------------------------------------------------

/// The library as the listing tool reports it: (index,
/// name) pairs in slot order.
fn library_entries(mcp: &mut Dispatch) -> Vec<(u64, String)> {
    let payload = mcp.call_ok("figure_library_list", json!({}));
    let entries = payload["entries"]
        .as_array()
        .unwrap_or_else(|| panic!("figure_library_list carries no entries array: {payload}"));
    entries
        .iter()
        .map(|entry| {
            (
                entry["index"]
                    .as_u64()
                    .expect("a library entry carries its index"),
                entry["name"]
                    .as_str()
                    .expect("a library entry carries its name")
                    .to_string(),
            )
        })
        .collect()
}

/// The slot of the entry called `name`, per the listing.
fn entry_index(mcp: &mut Dispatch, name: &str) -> u64 {
    library_entries(mcp)
        .iter()
        .find(|(_, n)| n.as_str() == name)
        .unwrap_or_else(|| panic!("no library entry named '{name}'"))
        .0
}

/// Seed one saved library entry, purely through MCP: add
/// a figure called `figure_name`, then save it to the
/// library with `figure_library_save`. Returns the entry
/// name the library knows it by — the figure's exact
/// name, stored inside the file at save time (Wave V,
/// V1). The FILE stem is still the save path's slug
/// (spaces to dashes, lowercased).
fn seed_saved_entry(mcp: &mut Dispatch, figure_name: &str) -> String {
    let added = mcp.call_ok(
        "add_figure",
        json!({ "kind": "mini", "name": figure_name }),
    );
    let figure = added["figure"]
        .as_u64()
        .unwrap_or_else(|| panic!("add_figure reply carries no figure index: {added}"));
    mcp.call_ok("figure_library_save", json!({ "figure": figure }));
    // The save is only real once the listing shows it.
    let names: Vec<String> = library_entries(mcp)
        .into_iter()
        .map(|(_, n)| n)
        .collect();
    assert!(
        names.iter().any(|n| n == figure_name),
        "figure_library_save left no entry named '{figure_name}' in {names:?}"
    );
    figure_name.to_string()
}

/// One rename call, addressed both ways (see the module
/// doc's schema note): the entry's slot and its current
/// name, plus the raw new name.
fn rename_args(index: u64, name: &str, new_name: &str) -> Value {
    json!({ "index": index, "name": name, "new_name": new_name })
}

/// Assert one refused rename: an `isError` TOOL RESULT
/// (never a protocol error — the response carries a
/// `result`, no `error` member), whose text carries the
/// panel's refusal voice (`voice`).
fn assert_refused(mcp: &mut Dispatch, args: Value, voice: &str, label: &str) -> String {
    let response = mcp.call_raw("figure_library_rename", args);
    assert!(
        response.get("error").is_none(),
        "{label}: a refusal must not be a protocol error: {response}"
    );
    let result = &response["result"];
    assert_eq!(
        result["isError"],
        json!(true),
        "{label}: a refused rename must be an isError tool result: {response}"
    );
    let text = result["content"][0]["text"]
        .as_str()
        .expect("the refusal carries one text block")
        .to_string();
    assert!(
        text.contains(voice),
        "{label}: refusal text should carry the panel voice '{voice}', got: {text}"
    );
    text
}

fn undo_depth(mcp: &mut Dispatch) -> u64 {
    mcp.call_ok("get_editor_status", json!({}))["undo_depth"]
        .as_u64()
        .expect("get_editor_status reports undo_depth")
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

/// The happy path: a saved entry renamed through
/// `figure_library_rename` only — the listing afterwards
/// shows the new stem at the SAME slot and the old name
/// nowhere, and on disk the old file is gone and the new
/// file exists (the file rename a client would observe).
#[test]
fn rename_happy_path_renames_entry_and_file_via_mcp_only() {
    let _lock = lock_cwd();
    let dir = temp_dir("happy");
    let _cwd = CwdGuard::enter(&dir);
    let mut mcp = Dispatch::new();

    // The tool is on the shipped surface.
    let list = mcp.request("tools/list", json!({}));
    let names: Vec<&str> = list["result"]["tools"]
        .as_array()
        .expect("tools/list returns an array")
        .iter()
        .map(|tool| tool["name"].as_str().expect("every tool carries a name"))
        .collect();
    assert!(
        names.contains(&"figure_library_rename"),
        "tools/list is missing figure_library_rename"
    );

    // A fresh library is exactly the three built-ins.
    let entering = library_entries(&mut mcp);
    assert_eq!(
        entering.len(),
        3,
        "a fresh temp library holds only the built-ins: {entering:?}"
    );
    assert!(
        entering.iter().all(|(_, n)| n.starts_with("Built-in: ")),
        "the entering entries are the built-ins: {entering:?}"
    );

    // Seed one saved entry through MCP; it lands after
    // the built-ins, backed by ./library/alpha.json.
    let name = seed_saved_entry(&mut mcp, "Alpha");
    let slot = entry_index(&mut mcp, &name);
    let old_file = library_dir(&dir).join("alpha.json");
    assert!(
        old_file.exists(),
        "the seeded entry is backed by {}",
        old_file.display()
    );

    // The rename: raw display text in, sanitised stem out
    // ("Alpha Prime" -> "alpha-prime", the save slug).
    let outcome = mcp.call(
        "figure_library_rename",
        rename_args(slot, &name, "Alpha Prime"),
    );
    assert!(
        !outcome.is_error,
        "the rename was refused: {}",
        outcome.text
    );

    // Read back through the listing only: the new name at
    // the same slot, the old name gone, built-ins intact,
    // and no entry added or lost.
    let after = library_entries(&mut mcp);
    assert_eq!(after.len(), entering.len() + 1, "no entry added or lost: {after:?}");
    assert!(
        !after.iter().any(|(_, n)| n == &name),
        "the old name is gone from the listing: {after:?}"
    );
    assert_eq!(
        entry_index(&mut mcp, "alpha-prime"),
        slot,
        "the renamed entry keeps its slot"
    );
    assert!(
        after
            .iter()
            .take(3)
            .all(|(_, n)| n.starts_with("Built-in: ")),
        "the built-ins are untouched: {after:?}"
    );

    // The file followed the entry.
    assert!(
        !old_file.exists(),
        "the old entry file {} is gone",
        old_file.display()
    );
    let new_file = library_dir(&dir).join("alpha-prime.json");
    assert!(
        new_file.exists(),
        "the renamed entry file {} exists",
        new_file.display()
    );

    // Restore the process directory before removing the
    // scratch dir — never delete the directory the
    // process is standing in.
    drop(_cwd);
    let _ = fs::remove_dir_all(&dir);
}

/// Every refusal the panel rules name, each an `isError`
/// tool result in the panel's voice, and after all of
/// them the library — listing and files alike — is
/// exactly as seeded.
#[test]
fn rename_refusals_are_iserror_results_and_change_nothing() {
    let _lock = lock_cwd();
    let dir = temp_dir("refusals");
    let _cwd = CwdGuard::enter(&dir);
    let mut mcp = Dispatch::new();

    let alpha = seed_saved_entry(&mut mcp, "Alpha");
    let beta = seed_saved_entry(&mut mcp, "Beta");
    let before = library_entries(&mut mcp);
    let alpha_slot = entry_index(&mut mcp, &alpha);

    // 1. A built-in entry refuses (built-ins have no file
    // and are refused before any fs access).
    assert_refused(
        &mut mcp,
        rename_args(0, "Built-in: Humanoid", "Mortal"),
        "built in",
        "built-in entry",
    );

    // 2. A collision with another entry's stem refuses:
    // "Beta" sanitises to the second entry's name.
    assert_refused(
        &mut mcp,
        rename_args(alpha_slot, &alpha, "Beta"),
        "already exists",
        "name collision",
    );

    // 3. A whitespace-only name sanitises to empty and
    // refuses before anything moves.
    assert_refused(
        &mut mcp,
        rename_args(alpha_slot, &alpha, "   "),
        "Type a new name",
        "empty name",
    );

    // 4. A target file that exists but is NOT a loaded
    // entry refuses: an unparseable .json the loader
    // skips (fixture write — the one state no MCP call
    // can manufacture, see the module doc).
    let blocked_file = library_dir(&dir).join("blocked.json");
    fs::write(&blocked_file, b"this is not a figure definition")
        .expect("write the blocked-file fixture");
    assert!(
        !library_entries(&mut mcp)
            .iter()
            .any(|(_, n)| n.as_str() == "blocked"),
        "the unparseable fixture must not appear as an entry"
    );
    let text = assert_refused(
        &mut mcp,
        rename_args(alpha_slot, &alpha, "Blocked"),
        "already exists",
        "existing target file",
    );
    assert!(
        text.contains("library folder"),
        "the target-file refusal names the folder, got: {text}"
    );

    // Nothing moved: the listing is byte-identical to the
    // seeded state, and every file is where it started.
    let after = library_entries(&mut mcp);
    assert_eq!(
        after, before,
        "refused renames changed the library listing"
    );
    for stem in ["alpha", "beta"] {
        assert!(
            library_dir(&dir).join(format!("{stem}.json")).exists(),
            "the seeded file for '{stem}' is still in place"
        );
    }
    assert!(
        blocked_file.exists(),
        "the blocked target file was not overwritten"
    );

    // Restore the process directory before removing the
    // scratch dir — never delete the directory the
    // process is standing in.
    drop(_cwd);
    let _ = fs::remove_dir_all(&dir);
}

/// The undo contract (see the module doc for the
/// evidence): the rename spends no undo step, and a
/// later Undo — proven live by reverting a real project
/// edit — leaves the renamed entry and its file renamed.
#[test]
fn rename_spends_no_undo_step_and_undo_does_not_revert_it() {
    let _lock = lock_cwd();
    let dir = temp_dir("undo");
    let _cwd = CwdGuard::enter(&dir);
    let mut mcp = Dispatch::new();

    let name = seed_saved_entry(&mut mcp, "Alpha");
    let slot = entry_index(&mut mcp, &name);

    // The rename itself spends no undo step.
    let depth_before = undo_depth(&mut mcp);
    let outcome = mcp.call(
        "figure_library_rename",
        rename_args(slot, &name, "Alpha Prime"),
    );
    assert!(
        !outcome.is_error,
        "the rename was refused: {}",
        outcome.text
    );
    assert_eq!(
        undo_depth(&mut mcp),
        depth_before,
        "a library rename is a file operation and spends no undo step"
    );
    assert_eq!(
        entry_index(&mut mcp, "alpha-prime"),
        slot,
        "the rename landed before the undo probe"
    );

    // A real project edit: one undo step, one more frame.
    let frames_before = mcp.call_ok("get_state", json!({}))["frame_count"]
        .as_u64()
        .expect("get_state reports frame_count");
    mcp.call_ok("add_frame", json!({}));
    assert_eq!(undo_depth(&mut mcp), depth_before + 1, "add_frame spends its step");
    assert_eq!(
        mcp.call_ok("get_state", json!({}))["frame_count"].as_u64(),
        Some(frames_before + 1),
        "the frame was added"
    );

    // Undo reverts the project edit — and ONLY the
    // project edit: the library entry keeps its new name
    // and the renamed file keeps its new path.
    mcp.call_ok("undo", json!({}));
    assert_eq!(
        mcp.call_ok("get_state", json!({}))["frame_count"].as_u64(),
        Some(frames_before),
        "undo reverted the frame add, so the undo was live"
    );
    let after = library_entries(&mut mcp);
    assert!(
        after.iter().any(|(_, n)| n.as_str() == "alpha-prime"),
        "undo must not revert the library rename: {after:?}"
    );
    assert!(
        !after.iter().any(|(_, n)| n == &name),
        "the old entry name did not come back: {after:?}"
    );
    assert!(
        library_dir(&dir).join("alpha-prime.json").exists(),
        "the renamed file is still in place after undo"
    );
    assert!(
        !library_dir(&dir).join("alpha.json").exists(),
        "the old file did not come back after undo"
    );

    // Restore the process directory before removing the
    // scratch dir — never delete the directory the
    // process is standing in.
    drop(_cwd);
    let _ = fs::remove_dir_all(&dir);
}
