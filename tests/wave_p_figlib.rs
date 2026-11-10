//! Wave P, P20 — the figure-library rename's undo
//! semantics, pinned on THIS tree (master = the Rune
//! line).
//!
//! THE DESIGN QUESTION: a library rename mutates state
//! the user can see (an entry's name, and a file on
//! disk). Does it spend an undo step, and can Undo
//! revert it? On the Tauri line the settled answer was
//! ZERO steps BY DESIGN — a rename is a file operation
//! on the library, not a project edit — and a past wave
//! nearly "fixed" it into spending one. This file
//! measures THIS tree's behaviour and pins the truth.
//!
//! MEASURED TRUTH (read from source, then pinned
//! below): this tree holds the same settlement, and
//! states it in so many words — the module doc of
//! `src/library_ops.rs` declares the rename "a file
//! operation, NOT an undo step", and the op
//! (`EditorCore::rename_library_entry`) calls neither
//! `snapshot()` nor `mark_dirty()`. Two independent
//! counters therefore stay frozen across a rename:
//! the undo stack (`undo_depth()` — Project snapshots
//! only; `core.library` lives outside the Project, so
//! a snapshot could not record a rename even if one
//! were taken) and the autosave edit counter
//! (`edits_since_autosave()`, advanced by `snapshot()`
//! and `mark_dirty()` alike). Undo after a rename
//! reverts project edits only; the entry and its file
//! stay renamed. NO DIVERGENCE from the Tauri
//! settlement — nothing here asks for a behaviour
//! change.
//!
//! ALREADY PINNED ELSEWHERE (not duplicated):
//! `tests/focused_e2e_library_rename.rs` pins the
//! happy path (entry + file renamed via MCP), the
//! refusal set's state-unchanged half (built-in, name
//! collision, empty name, unparseable target file),
//! and the success-side undo contract (a successful
//! rename spends no step; a live Undo reverts a real
//! project edit and leaves the rename standing).
//!
//! THIS FILE PINS THE GAPS THAT FILE LEAVES:
//!   * a rename addressed at a MISSING entry (stale /
//!     out-of-range index) — the MCP tool refuses, the
//!     core op settles silently, and neither spends
//!     anything;
//!   * the refusals' BOOKKEEPING half: undo depth and
//!     the edit counter are unchanged across refused
//!     renames (the Focused file proved the listing
//!     and files unchanged; bookkeeping is measured
//!     here);
//!   * the op level directly (no MCP): a successful
//!     rename freezes BOTH counters, a same-name
//!     settle spends nothing, and refusals spend
//!     nothing.
//!
//! Fixtures follow the Focused file's discipline:
//! the library folder is `./library` relative to the
//! process current directory, so every test enters
//! its own temp dir (serialised on a mutex — the
//! current directory is process-global) and restores
//! it on drop.

use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::mcp::McpServer;
use stykanimate_rs::model::FigureDef;

/// Serialises this file's tests: they each change the
/// process current directory, which is process-global.
/// Poison-tolerant, as in the Focused file's lock.
static FIGLIB_CWD_LOCK: Mutex<()> = Mutex::new(());

fn lock_cwd() -> std::sync::MutexGuard<'static, ()> {
    FIGLIB_CWD_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "styk-wave-p-figlib-{}-{tag}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Restores the process current directory on drop,
/// including when the test panics.
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

fn library_dir(dir: &Path) -> PathBuf {
    dir.join("library")
}

// ---------------------------------------------------------------------
// MCP dispatch driver (the tree's e2e idiom, trimmed
// to what these pins call)
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

    fn call(&mut self, name: &str, args: Value) -> CallOutcome {
        let response = self.request(
            "tools/call",
            json!({ "name": name, "arguments": args }),
        );
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
        assert!(!outcome.is_error, "tool {name} failed: {}", outcome.text);
        outcome.payload
    }
}

/// The library as `figure_library_list` reports it:
/// entry names in slot order.
fn library_names(mcp: &mut Dispatch) -> Vec<String> {
    let payload = mcp.call_ok("figure_library_list", json!({}));
    payload["entries"]
        .as_array()
        .unwrap_or_else(|| panic!("figure_library_list carries no entries array: {payload}"))
        .iter()
        .map(|entry| {
            entry["name"]
                .as_str()
                .expect("a library entry carries its name")
                .to_string()
        })
        .collect()
}

/// Seed one saved library entry through MCP (the
/// Focused file's recipe): add a figure, then
/// `figure_library_save`. Returns the entry's name —
/// since Wave V (V1) the library stores the figure's
/// name inside its file, so the entry is listed under
/// the figure's exact name. (Its FILE is still the
/// slug: spaces to dashes, lowercased.)
fn seed_saved_entry(mcp: &mut Dispatch, figure_name: &str) -> String {
    let added = mcp.call_ok(
        "add_figure",
        json!({ "kind": "mini", "name": figure_name }),
    );
    let figure = added["figure"]
        .as_u64()
        .unwrap_or_else(|| panic!("add_figure reply carries no figure index: {added}"));
    mcp.call_ok("figure_library_save", json!({ "figure": figure }));
    assert!(
        library_names(mcp).iter().any(|n| n == figure_name),
        "figure_library_save left no entry named '{figure_name}'"
    );
    figure_name.to_string()
}

fn undo_depth(mcp: &mut Dispatch) -> u64 {
    mcp.call_ok("get_editor_status", json!({}))["undo_depth"]
        .as_u64()
        .expect("get_editor_status reports undo_depth")
}

// ---------------------------------------------------------------------
// MCP-level gap pins
// ---------------------------------------------------------------------

/// A rename addressed at a MISSING entry — an index
/// past the end of the library, the stale-slot case a
/// client holding an old listing can produce — is
/// refused by the tool ("no library entry at index
/// N", an isError result), and the refusal spends
/// nothing: undo depth unchanged, listing unchanged.
#[test]
fn rename_of_a_missing_entry_refuses_and_spends_nothing() {
    let _lock = lock_cwd();
    let dir = temp_dir("missing");
    let _cwd = CwdGuard::enter(&dir);
    let mut mcp = Dispatch::new();

    seed_saved_entry(&mut mcp, "Alpha");
    let before = library_names(&mut mcp);
    let depth_before = undo_depth(&mut mcp);

    // Far past the end, and exactly one past the end:
    // both are "no library entry", never a clamp onto
    // the last entry.
    for index in [999u64, before.len() as u64] {
        let outcome = mcp.call(
            "figure_library_rename",
            json!({ "index": index, "new_name": "Ghost" }),
        );
        assert!(
            outcome.is_error,
            "renaming missing entry {index} must refuse, got: {}",
            outcome.text
        );
        assert!(
            outcome.text.contains(&format!("no library entry at index {index}")),
            "the refusal names the missing index, got: {}",
            outcome.text
        );
    }

    assert_eq!(
        undo_depth(&mut mcp),
        depth_before,
        "a refused rename of a missing entry spends no undo step"
    );
    assert_eq!(
        library_names(&mut mcp),
        before,
        "the library is unchanged after the refused renames"
    );

    drop(_cwd);
    let _ = fs::remove_dir_all(&dir);
}

/// The bookkeeping half of the refusal contract: each
/// panel-rule refusal (name collision, built-in,
/// empty name) leaves the undo depth exactly where it
/// was. (The Focused e2e pins the same refusals leave
/// the listing and files unchanged; depth is measured
/// here.)
#[test]
fn refused_renames_spend_no_undo_step() {
    let _lock = lock_cwd();
    let dir = temp_dir("refusal-depth");
    let _cwd = CwdGuard::enter(&dir);
    let mut mcp = Dispatch::new();

    let alpha = seed_saved_entry(&mut mcp, "Alpha");
    seed_saved_entry(&mut mcp, "Beta");
    let names = library_names(&mut mcp);
    let alpha_slot = names.iter().position(|n| n == &alpha).unwrap() as u64;

    let depth_before = undo_depth(&mut mcp);

    // Collision: "Beta" sanitises to the second
    // entry's stem.
    let collision = mcp.call(
        "figure_library_rename",
        json!({ "index": alpha_slot, "new_name": "Beta" }),
    );
    assert!(collision.is_error, "the collision must refuse: {}", collision.text);
    assert!(
        collision.text.contains("already exists"),
        "the collision carries the panel voice, got: {}",
        collision.text
    );
    assert_eq!(
        undo_depth(&mut mcp),
        depth_before,
        "a collision refusal spends no undo step"
    );

    // Built-in: slot 0 is a built-in in a fresh
    // library (the Focused file pins the same order).
    let builtin = mcp.call(
        "figure_library_rename",
        json!({ "index": 0, "new_name": "Mortal" }),
    );
    assert!(builtin.is_error, "the built-in must refuse: {}", builtin.text);
    assert!(
        builtin.text.contains("built in"),
        "the built-in refusal carries the panel voice, got: {}",
        builtin.text
    );
    assert_eq!(
        undo_depth(&mut mcp),
        depth_before,
        "a built-in refusal spends no undo step"
    );

    // Empty: whitespace sanitises to an empty stem.
    let empty = mcp.call(
        "figure_library_rename",
        json!({ "index": alpha_slot, "new_name": "   " }),
    );
    assert!(empty.is_error, "the empty name must refuse: {}", empty.text);
    assert_eq!(
        undo_depth(&mut mcp),
        depth_before,
        "an empty-name refusal spends no undo step"
    );

    assert_eq!(
        library_names(&mut mcp),
        names,
        "no refusal moved the library"
    );

    drop(_cwd);
    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------
// Op-level pins (EditorCore directly, no MCP)
// ---------------------------------------------------------------------

/// A core whose library is exactly the given saved
/// entries, each backed by a planted file in the
/// entered temp dir's `./library`. The rename op
/// never parses the backing files — it only moves
/// them — so the planted bytes are a minimal figure
/// document placeholder.
fn core_with_saved_entries(dir: &Path, stems: &[&str]) -> EditorCore {
    let lib = library_dir(dir);
    fs::create_dir_all(&lib).expect("create the library folder");
    for stem in stems {
        fs::write(lib.join(format!("{stem}.json")), b"{}")
            .expect("plant the entry's backing file");
    }
    let mut core = EditorCore::new();
    core.library = stems
        .iter()
        .map(|stem| (stem.to_string(), FigureDef::simple()))
        .collect();
    core
}

/// The two counters a rename must never move: the
/// undo stack depth and the autosave edit counter
/// (advanced by `snapshot()` and `mark_dirty()`
/// alike, so a rename that secretly did either would
/// show here).
fn counters(core: &EditorCore) -> (usize, usize, u64) {
    (
        core.undo_depth(),
        core.redo_depth(),
        core.edits_since_autosave(),
    )
}

/// A successful op-level rename renames the entry and
/// moves its file — and freezes BOTH bookkeeping
/// counters: no undo step, no redo disturbance, no
/// autosave edit. This is the design's centre: the
/// rename is a file operation, not a project edit.
#[test]
fn op_rename_spends_no_undo_step_and_marks_no_edit() {
    let _lock = lock_cwd();
    let dir = temp_dir("op-success");
    let _cwd = CwdGuard::enter(&dir);
    let mut core = core_with_saved_entries(&dir, &["alpha", "beta"]);

    let before = counters(&core);
    assert!(
        core.rename_library_entry(0, "Alpha Prime"),
        "the rename settles: {}",
        core.status
    );
    assert_eq!(core.library[0].0, "alpha-prime", "the entry took the sanitised stem");
    assert_eq!(core.library[1].0, "beta", "the other entry is untouched");
    assert!(
        !library_dir(&dir).join("alpha.json").exists(),
        "the old backing file moved"
    );
    assert!(
        library_dir(&dir).join("alpha-prime.json").exists(),
        "the renamed backing file exists"
    );
    assert_eq!(
        counters(&core),
        before,
        "a library rename is a file operation: no undo step, no redo change, no autosave edit"
    );

    drop(_cwd);
    let _ = fs::remove_dir_all(&dir);
}

/// The op's stale-slot settle: an out-of-range index
/// returns `true` (the rename session closes, exactly
/// as the library delete treats a stale slot) WITHOUT
/// touching the status line, the library, or either
/// counter. The MCP tool refuses the same address
/// with an error (pinned above) — the two surfaces
/// differ deliberately: the panel owns the session,
/// the tool owns the answer.
#[test]
fn op_rename_of_a_stale_slot_settles_silently_and_spends_nothing() {
    let _lock = lock_cwd();
    let dir = temp_dir("op-stale");
    let _cwd = CwdGuard::enter(&dir);
    let mut core = core_with_saved_entries(&dir, &["alpha"]);
    core.status = "sentinel".to_string();

    let before = counters(&core);
    assert!(
        core.rename_library_entry(42, "Whatever"),
        "a stale slot settles the session"
    );
    assert_eq!(core.status, "sentinel", "a stale slot writes no status");
    assert_eq!(core.library.len(), 1, "the library is unchanged");
    assert_eq!(core.library[0].0, "alpha", "the entry is unchanged");
    assert_eq!(
        counters(&core),
        before,
        "a stale-slot settle spends nothing"
    );

    drop(_cwd);
    let _ = fs::remove_dir_all(&dir);
}

/// The op-level refusals and the same-name settle,
/// each spending nothing: collision and built-in
/// return `false` with the reason on the status line;
/// renaming to the entry's own current name returns
/// `true` (settled, nothing renamed) — the no-change
/// success a snapshot-first implementation would
/// wrongly charge a step for.
#[test]
fn op_refusals_and_same_name_settle_spend_nothing() {
    let _lock = lock_cwd();
    let dir = temp_dir("op-refusals");
    let _cwd = CwdGuard::enter(&dir);
    let mut core = EditorCore::new();
    core.library = vec![
        ("Built-in: Humanoid".to_string(), FigureDef::simple()),
        ("alpha".to_string(), FigureDef::simple()),
        ("beta".to_string(), FigureDef::simple()),
    ];
    let lib = library_dir(&dir);
    fs::create_dir_all(&lib).expect("create the library folder");
    for stem in ["alpha", "beta"] {
        fs::write(lib.join(format!("{stem}.json")), b"{}")
            .expect("plant the entry's backing file");
    }
    let alpha = 1;

    let before = counters(&core);

    // Rename to an existing name refuses.
    assert!(
        !core.rename_library_entry(alpha, "Beta"),
        "the collision refuses"
    );
    assert!(
        core.status.contains("already exists"),
        "the collision names itself on the status line, got: {}",
        core.status
    );

    // A built-in entry refuses.
    assert!(
        !core.rename_library_entry(0, "Mortal"),
        "the built-in refuses"
    );
    assert!(
        core.status.contains("built in"),
        "the built-in refusal names itself, got: {}",
        core.status
    );

    // The same name settles without renaming.
    assert!(
        core.rename_library_entry(alpha, "Alpha"),
        "the same-name case settles"
    );
    assert!(
        core.status.contains("already has that name"),
        "the same-name settle says so, got: {}",
        core.status
    );

    assert_eq!(core.library[alpha].0, "alpha", "no refusal or settle renamed the entry");
    assert!(
        library_dir(&dir).join("alpha.json").exists(),
        "the backing file never moved"
    );
    assert_eq!(
        counters(&core),
        before,
        "refusals and the same-name settle spend nothing: no undo step, no autosave edit"
    );

    drop(_cwd);
    let _ = fs::remove_dir_all(&dir);
}
