//! Integration tests: the project and library lifecycle, driven
//! end-to-end through real MCP dispatch (`McpServer::handle`).
//!
//! The session in this file uses MCP messages for every editor
//! operation: `initialize` / `tools/list`, the registry's
//! `new_project`, figure/frame edits, the named save, the IO
//! `load_project`, and the read-only status/state tools. Direct
//! filesystem access is used only for the two things an MCP
//! client itself would observe from outside the protocol: that
//! the promised files exist under the per-test projects
//! directory, and what bytes were written in them.
//!
//! The named-project tools resolve `./projects/` (and the
//! figure library resolves `./library/`) against the process
//! current directory, so the session enters a per-test temp
//! directory before constructing the server and restores the
//! original directory before the temp tree is removed. This
//! file deliberately contains one test: the current directory
//! is process-global, and a single session keeps that safe.
//!
//! Steps the entering MCP surface could not perform were
//! marked `WAVE11-GAP` at the exact point in the session
//! where they belong; the Wave 11 splice landed every one
//! (the dirty counter, Save a copy / duplicate, rename, the
//! 2D figure library), so those steps now run as real MCP
//! calls and the markers are gone. Nothing is faked with
//! editor internals: this suite's contract is "MCP messages
//! only".

use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use stykanimate_rs::mcp::McpServer;
use stykanimate_rs::model::Project;

/// A per-test scratch directory under the OS temp dir,
/// following the pattern the other integration/unit tests use:
/// stale leftovers from a crashed run are removed up front and
/// the caller removes the directory again when done.
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "styk-wave11-e2e-projects-{}-{tag}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Restores the process current directory on drop, including
/// when the session panics.
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

/// One MCP `tools/call`, decoded the way a client decodes it:
/// the JSON document in the single text content block, plus
/// the protocol's error flag. A protocol-level error response
/// (for example an unknown tool) is folded into the same shape
/// with its message as the text.
struct CallOutcome {
    payload: Value,
    text: String,
    is_error: bool,
}

/// A small dispatch driver over one `McpServer`: sequential
/// request ids, JSON-RPC envelope assertions on every reply,
/// and ok/err helpers so the session below reads as a session.
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
        assert!(
            !outcome.is_error,
            "tool {name} failed: {}",
            outcome.text
        );
        outcome.payload
    }

    fn call_err(&mut self, name: &str, args: Value) -> String {
        let outcome = self.call(name, args);
        assert!(
            outcome.is_error,
            "tool {name} unexpectedly succeeded: {}",
            outcome.text
        );
        outcome.text
    }
}

/// Every entry under `root`, relative and sorted, directories
/// with a trailing `/` — the exact-contents witness for the
/// invalid-name steps: a refused save must leave this vector
/// byte-for-byte unchanged, and a sanitised save must change
/// it in exactly the one promised place.
fn relative_tree(root: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        let mut entries: Vec<PathBuf> = fs::read_dir(dir)
            .expect("read temp tree")
            .map(|e| e.expect("temp tree entry").path())
            .collect();
        entries.sort();
        for path in entries {
            let rel = path
                .strip_prefix(root)
                .expect("walk stays under the root")
                .display()
                .to_string();
            if path.is_dir() {
                out.push(format!("{rel}/"));
                walk(root, &path, out);
            } else {
                out.push(rel);
            }
        }
    }
    let mut out = Vec::new();
    if root.exists() {
        walk(root, root, &mut out);
    }
    out.sort();
    out
}

fn read_project(path: &Path) -> Project {
    let text = fs::read_to_string(path).expect("read saved project");
    serde_json::from_str(&text).expect("saved file is a project document")
}

#[test]
fn project_and_library_lifecycle_over_mcp() {
    let root = temp_dir("lifecycle");
    // Canonicalised so paths built from this root spell
    // identically to paths the editor builds from
    // `current_dir()` (which resolves symlinks) — the save
    // status assertion compares the two spellings.
    let root = fs::canonicalize(&root).expect("canonicalise temp dir");
    let cwd_guard = CwdGuard::enter(&root);
    let projects_dir = root.join("projects");
    let mut mcp = Dispatch::new();

    // -- Handshake and registry census -------------------------

    let init = mcp.request("initialize", json!({}));
    assert_eq!(init["result"]["protocolVersion"], "2024-11-05");
    assert_eq!(init["result"]["serverInfo"]["name"], "stykanimate-rs");

    let list = mcp.request("tools/list", json!({}));
    let tool_names: BTreeSet<String> = list["result"]["tools"]
        .as_array()
        .expect("tools/list returns an array")
        .iter()
        .map(|t| {
            t["name"]
                .as_str()
                .expect("every tool carries a name")
                .to_string()
        })
        .collect();
    for required in [
        "new_project",
        "add_figure",
        "add_frame",
        "list_figures",
        "get_state",
        "project_summary",
        "get_editor_status",
        "save_named_project",
        "save_project",
        "load_project",
    ] {
        assert!(
            tool_names.contains(required),
            "tools/list is missing {required}, which this session drives"
        );
    }

    // -- Create, edit, save ------------------------------------

    let created = mcp.call_ok(
        "new_project",
        json!({
            "name": "Wave11 Lifecycle",
            "fps": 12,
            "width": 800,
            "height": 600,
        }),
    );
    assert_eq!(created["project"]["name"], "Wave11 Lifecycle");
    assert_eq!(created["project"]["figures"], 1);
    assert_eq!(created["project"]["frames"], 1);
    assert_eq!(created["project"]["stage"], json!([800.0, 600.0]));
    assert_eq!(created["project"]["current_frame"], 0);

    // Two undoable edits through the registry: a second figure
    // with an identity worth finding again in the saved bytes,
    // and a second frame.
    let added = mcp.call_ok(
        "add_figure",
        json!({
            "kind": "humanoid",
            "name": "Lifecycle Partner",
            "color": 0x3366CC,
        }),
    );
    assert_eq!(added["figure"], 1);
    assert_eq!(added["name"], "Lifecycle Partner");
    assert_eq!(added["color"], 0x3366CC);
    assert_eq!(added["nodes"], 11);

    let framed = mcp.call_ok("add_frame", json!({}));
    assert_eq!(framed["frames"], 2);
    assert_eq!(framed["current_frame"], 1);

    let figures_before_save = mcp.call_ok("list_figures", json!({}));
    assert_eq!(figures_before_save["figures"].as_array().unwrap().len(), 2);
    assert_eq!(
        figures_before_save["figures"][1]["name"],
        "Lifecycle Partner"
    );
    assert_eq!(figures_before_save["figures"][1]["color"], 0x3366CC);
    assert_eq!(figures_before_save["figures"][1]["nodes"], 11);

    // The observable pre-save state: two undoable edits on the
    // history. `get_editor_status` documents that the undo
    // depth is the observable edit history — there is no
    // saved/dirty read until Wave 11 (see the dirty-counter
    // assertion after the save).
    let status_before_save = mcp.call_ok("get_editor_status", json!({}));
    assert_eq!(status_before_save["undo_depth"], 2);
    assert_eq!(status_before_save["can_undo"], true);
    assert_eq!(status_before_save["current_frame"], 1);

    let saved = mcp.call_ok("save_named_project", json!({}));
    assert_eq!(saved["ok"], true);
    assert_eq!(saved["saved"], true);
    assert_eq!(saved["project"], "Wave11 Lifecycle");

    // The promised file: the slug of the project name, inside
    // the temp projects dir. The only other entry is
    // recents.json — a named save records the project in the
    // recents list, exactly like the GUI's Save.
    let main_path = projects_dir.join("wave11-lifecycle.json");
    assert!(
        saved["status"]
            .as_str()
            .expect("save reports a status")
            .contains(&main_path.display().to_string()),
        "save status names the written path: {}",
        saved["status"]
    );
    assert!(main_path.exists(), "named save wrote {main_path:?}");
    assert_eq!(
        relative_tree(&root),
        [
            "projects/",
            "projects/recents.json",
            "projects/wave11-lifecycle.json",
        ],
        "the temp tree holds exactly the named save (+ recents)"
    );

    // The written bytes are the edited project, not the
    // pre-edit one.
    let on_disk = read_project(&main_path);
    assert_eq!(on_disk.name, "Wave11 Lifecycle");
    assert!((on_disk.fps - 12.0).abs() < f32::EPSILON);
    assert_eq!(on_disk.stage, [800.0, 600.0]);
    assert_eq!(on_disk.frames.len(), 2);
    assert_eq!(on_disk.figures.len(), 2);
    assert_eq!(on_disk.figures[0].name, "Styk");
    assert_eq!(on_disk.figures[1].name, "Lifecycle Partner");
    assert_eq!(on_disk.figures[1].color, 0x3366CC);
    assert_eq!(on_disk.figures[1].def.nodes.len(), 11);

    // After the save the status line is the save's own, and
    // the undo history is untouched (a save is not an undo).
    let status_after_save = mcp.call_ok("get_editor_status", json!({}));
    assert_eq!(status_after_save["status"], saved["status"]);
    assert_eq!(status_after_save["undo_depth"], 2);
    assert_eq!(status_after_save["can_undo"], true);

    // Wave 10's save semantics restart the dirty baseline
    // inside EditorCore (`save_named_project_to` sets
    // `last_autosave_count = edit_count`), and since the
    // Wave 11 splice `get_editor_status` exposes the counter
    // as `edits_since_autosave`: after a named save it reads
    // exactly 0. (The other candidate field names were never
    // the contract and stay absent.)
    assert_eq!(
        status_after_save["edits_since_autosave"], 0,
        "a named save restarts the dirty baseline"
    );
    for field in ["dirty", "edit_count", "last_autosave_count", "unsaved_edits"] {
        assert!(
            status_after_save.get(field).is_none(),
            "get_editor_status unexpectedly exposes {field}"
        );
    }

    // -- Save a copy / duplicate -------------------------------

    // The panel's never-overwrite copy operations, reachable
    // since the Wave 11 splice. `save_project_copy` derives
    // the copy's name ("{project} copy", the GUI's own rule)
    // and claims its file with create-new semantics;
    // `duplicate_project` byte-copies the current file with
    // the -copy / -copy-2 suffix walk. (The `save_project` IO
    // tool is not a substitute for either: it writes the
    // session project to a caller-chosen path verbatim,
    // overwriting, with no derived name and no refusal.)
    let copy = mcp.call_ok("save_project_copy", json!({}));
    assert_eq!(copy["name"], "Wave11 Lifecycle copy");
    let copy_path = projects_dir.join("wave11-lifecycle-copy.json");
    assert!(
        copy["path"].as_str().unwrap_or("").ends_with("wave11-lifecycle-copy.json"),
        "the copy lands at the derived name's slug: {copy}"
    );
    assert!(copy_path.exists(), "the copy file exists");
    assert_eq!(
        read_project(&copy_path).name,
        "Wave11 Lifecycle copy",
        "the copy stores the derived name"
    );
    let copy_bytes = fs::read(&copy_path).expect("read the copy");

    // A second copy refuses (create-new) and the first copy's
    // bytes are untouched — the panel's second-press behaviour.
    let err = mcp.call_err("save_project_copy", json!({}));
    assert!(
        err.contains("already exists"),
        "a second Save a copy refuses: {err}"
    );
    assert_eq!(
        fs::read(&copy_path).expect("re-read the copy"),
        copy_bytes,
        "the refused second copy left the first copy byte-identical"
    );

    // Duplicate walks PAST the copy's name: -copy is taken,
    // so the duplicate lands at -copy-2.
    let dup = mcp.call_ok("duplicate_project", json!({}));
    let dup_path = projects_dir.join("wave11-lifecycle-copy-2.json");
    assert!(
        dup["path"].as_str().unwrap_or("").ends_with("wave11-lifecycle-copy-2.json"),
        "the suffix walk skipped the taken -copy name: {dup}"
    );
    assert_eq!(
        fs::read(&dup_path).expect("read the duplicate"),
        fs::read(&main_path).expect("read the original"),
        "a duplicate is a byte copy of the current file"
    );
    assert_eq!(
        relative_tree(&root),
        [
            "projects/",
            "projects/recents.json",
            "projects/wave11-lifecycle-copy-2.json",
            "projects/wave11-lifecycle-copy.json",
            "projects/wave11-lifecycle.json",
        ],
        "the tree holds the original, its copy and its duplicate"
    );

    // -- Open back ----------------------------------------------

    // Replace the session with a different project (MCP only),
    // then open the saved file back through the IO tool and
    // assert the contents survived the round trip.
    let distraction = mcp.call_ok(
        "new_project",
        json!({ "name": "Distraction", "fps": 24 }),
    );
    assert_eq!(distraction["project"]["name"], "Distraction");
    assert_eq!(distraction["project"]["figures"], 1);

    let opened = mcp.call_ok(
        "load_project",
        json!({ "path": main_path.to_string_lossy() }),
    );
    assert_eq!(opened["name"], "Wave11 Lifecycle");
    assert_eq!(opened["figure_count"], 2);
    assert_eq!(
        opened["figures"],
        json!(["Styk", "Lifecycle Partner"])
    );
    assert_eq!(opened["frames"], 2);
    assert_eq!(opened["stage"], json!([800.0, 600.0]));

    let state_after_open = mcp.call_ok("get_state", json!({}));
    assert_eq!(state_after_open["project"], "Wave11 Lifecycle");
    assert_eq!(state_after_open["frame_count"], 2);
    assert_eq!(state_after_open["current_frame"], 0);
    let figures_after_open = mcp.call_ok("list_figures", json!({}));
    assert_eq!(
        figures_after_open, figures_before_save,
        "the reopened project's figures match the saved ones"
    );

    // `load_project`'s replace path performs the editor's
    // own open bookkeeping since the Wave 11 wiring:
    // `note_project_swapped` restarts the dirty baseline (the
    // counter reads 0 on the freshly opened project, asserted
    // here), and `note_project_loaded` repoints the file
    // selector at the opened file — the rename step below
    // depends on that repointing to find its file.
    let status_after_open = mcp.call_ok("get_editor_status", json!({}));
    assert_eq!(
        status_after_open["edits_since_autosave"], 0,
        "an MCP open restarts the dirty baseline like a GUI open"
    );

    // -- Rename --------------------------------------------------

    // `project_rename` wraps the Home panel's rename
    // (`projects::rename_project`): the current project's
    // file moves to the new name's slug, the session's name
    // follows, and the selector is repointed. (The project
    // card's Set name has no MCP twin — it only rewrites
    // Project::name, which `new_project` already covers as a
    // fresh session, not a rename.)
    let renamed = mcp.call_ok(
        "project_rename",
        json!({ "new_name": "Wave11 Renamed" }),
    );
    let renamed_path = projects_dir.join("wave11-renamed.json");
    assert!(
        renamed["path"].as_str().unwrap_or("").ends_with("wave11-renamed.json"),
        "the rename reports the new path: {renamed}"
    );
    assert!(renamed_path.exists(), "the renamed file exists");
    assert!(
        !main_path.exists(),
        "the old file is gone after the rename"
    );
    let summary = mcp.call_ok("project_summary", json!({}));
    assert_eq!(
        summary["name"], "Wave11 Renamed",
        "the session reports the new name"
    );

    // Renaming onto an existing different project refuses,
    // leaving both files byte-identical.
    let renamed_bytes = fs::read(&renamed_path).expect("read the renamed file");
    let err = mcp.call_err(
        "project_rename",
        json!({ "new_name": "Wave11 Lifecycle copy" }),
    );
    assert!(
        !err.is_empty(),
        "rename onto an existing project must refuse"
    );
    assert_eq!(
        fs::read(&renamed_path).expect("re-read the renamed file"),
        renamed_bytes,
        "the refused rename left the source file untouched"
    );
    assert_eq!(
        fs::read(&copy_path).expect("re-read the copy"),
        copy_bytes,
        "the refused rename left the target file untouched"
    );

    // Rename back, restoring the session's original file for
    // the steps below.
    let back = mcp.call_ok(
        "project_rename",
        json!({ "new_name": "Wave11 Lifecycle" }),
    );
    assert_eq!(back["name"], "Wave11 Lifecycle");
    assert!(main_path.exists(), "the rename back restored the main file");
    assert!(!renamed_path.exists(), "the renamed file is gone again");

    // -- Figure library ------------------------------------------

    // The 2D figure library has a full MCP surface since the
    // splice (the fig3_* library tools below in tools/list
    // serve ./library3d and 3D sessions — different library):
    // figure_library_save / figure_library_list /
    // figure_library_insert / figure_library_cycle wrap
    // EditorCore::save_figure_to_library / insert_library_figure
    // / cycle_library.
    let saved_fig = mcp.call_ok(
        "figure_library_save",
        json!({ "figure": 1 }),
    );
    assert_eq!(saved_fig["ok"], true);
    let lib = mcp.call_ok("figure_library_list", json!({}));
    // The library always carries the three built-in entries
    // (Humanoid / Quadruped / Mini, in-memory — no files);
    // the save adds the file-backed entry beside them.
    assert_eq!(
        lib["count"], 4,
        "three built-ins plus the saved entry: {lib}"
    );
    let entries = lib["entries"].as_array().expect("entries array");
    let partner = entries
        .iter()
        .find(|e| e["name"] == "lifecycle-partner")
        .expect("the saved entry is listed by its file stem (the format's rule)");
    assert_eq!(partner["nodes"], 11, "the def's node count is stored");

    // Insert it into a fresh project: the def (and so the
    // node count) survives the round trip. Two format truths
    // ride along, asserted rather than papered over: the
    // figure's NAME becomes the entry's stem name, and its
    // colour is the palette colour for the slot — the
    // library stores FigureDef only, never the instance
    // colour (0x3366CC here is NOT preserved, by format
    // design, exactly as in the GUI).
    mcp.call_ok("new_project", json!({ "name": "Library Host" }));
    let inserted = mcp.call_ok(
        "figure_library_insert",
        json!({ "name": "lifecycle-partner" }),
    );
    assert_eq!(inserted["ok"], true);
    let figures = mcp.call_ok("list_figures", json!({}));
    let figs = figures["figures"].as_array().expect("figures array");
    assert_eq!(figs.len(), 2, "the host project gained the library figure");
    assert_eq!(figs[1]["name"], "lifecycle-partner");
    assert_eq!(figs[1]["nodes"], 11, "the inserted def kept its 11 nodes");
    assert_ne!(
        figs[1]["color"], 0x3366CC,
        "the library does not store instance colour — insert assigns a palette colour"
    );

    // Restore the lifecycle session for the steps below.
    mcp.call_ok(
        "load_project",
        json!({ "path": main_path.to_string_lossy() }),
    );

    // -- Invalid project names -----------------------------------

    // Blank and punctuation-only names are refused by the
    // named save (the `projects::file_stem_for` rule) and —
    // the point of the exact-tree witness — create nothing.
    let tree_before_invalid = relative_tree(&root);
    assert_eq!(
        tree_before_invalid,
        [
            "library/",
            "library/lifecycle-partner.json",
            "projects/",
            "projects/recents.json",
            "projects/wave11-lifecycle-copy-2.json",
            "projects/wave11-lifecycle-copy.json",
            "projects/wave11-lifecycle.json",
        ]
    );
    for bad_name in ["", "   ", "!!! ☕"] {
        mcp.call_ok("new_project", json!({ "name": bad_name }));
        let err = mcp.call_err("save_named_project", json!({}));
        assert!(
            err.contains("cannot name a file"),
            "save of {bad_name:?} refused with the naming rule: {err}"
        );
        assert_eq!(
            relative_tree(&root),
            tree_before_invalid,
            "refused save of {bad_name:?} created or removed a file"
        );
    }

    // A name carrying path separators is the brief's third
    // invalid case, but the source contract is deliberately
    // different from a refusal: the slug IS the path-safety
    // rule, so every separator and dot becomes a dash and the
    // save lands INSIDE the projects dir under the neutralised
    // name (the behaviour `projects.rs` pins in its own
    // tests). What this session asserts is the safety
    // property end-to-end: exactly one new file, inside the
    // library dir, nothing written at or above the temp root.
    mcp.call_ok(
        "new_project",
        json!({ "name": "../../Escape Attempt" }),
    );
    let traversal_save = mcp.call_ok("save_named_project", json!({}));
    assert_eq!(traversal_save["saved"], true);
    let traversal_path = projects_dir.join("------escape-attempt.json");
    assert!(traversal_path.exists());
    assert_eq!(
        read_project(&traversal_path).name,
        "../../Escape Attempt",
        "the stored name is verbatim; only the FILE name is neutralised"
    );
    assert_eq!(
        relative_tree(&root),
        [
            "library/",
            "library/lifecycle-partner.json",
            "projects/",
            "projects/recents.json",
            "projects/------escape-attempt.json",
            "projects/wave11-lifecycle-copy-2.json",
            "projects/wave11-lifecycle-copy.json",
            "projects/wave11-lifecycle.json",
        ],
        "the traversal-named save stayed inside the projects dir"
    );

    // -- Open a project that does not exist ----------------------

    // Restore the known project first (MCP only), snapshot
    // every MCP-visible read, then attempt the missing open:
    // an honest error, and the session must be untouched.
    mcp.call_ok(
        "load_project",
        json!({ "path": main_path.to_string_lossy() }),
    );
    let state_before_miss = mcp.call_ok("get_state", json!({}));
    let figures_before_miss = mcp.call_ok("list_figures", json!({}));
    let editor_before_miss = mcp.call_ok("get_editor_status", json!({}));
    let tree_before_miss = relative_tree(&root);

    let missing_path = projects_dir.join("no-such-project.json");
    assert!(!missing_path.exists());
    let err = mcp.call_err(
        "load_project",
        json!({ "path": missing_path.to_string_lossy() }),
    );
    assert!(
        err.contains("could not read"),
        "missing open reports the read failure: {err}"
    );
    assert!(
        err.contains("no-such-project.json"),
        "missing open names the file it tried: {err}"
    );

    assert_eq!(
        mcp.call_ok("get_state", json!({})),
        state_before_miss,
        "a failed open changed the project"
    );
    assert_eq!(
        mcp.call_ok("list_figures", json!({})),
        figures_before_miss,
        "a failed open changed the figures"
    );
    assert_eq!(
        mcp.call_ok("get_editor_status", json!({})),
        editor_before_miss,
        "a failed open changed the session state"
    );
    assert_eq!(
        relative_tree(&root),
        tree_before_miss,
        "a failed open changed the temp tree"
    );

    // Restore the caller's working directory before removing
    // the tree the session ran in.
    drop(cwd_guard);
    let _ = fs::remove_dir_all(&root);
}
