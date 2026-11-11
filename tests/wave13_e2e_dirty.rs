//! Wave 13, Squad B (session B10) — integration tests: the
//! dirty-marking / snapshot-coverage fix, driven end-to-end
//! through real MCP dispatch (`McpServer::handle`).
//!
//! The entering defect (Wave 12's E8 finding, restated in
//! the Wave 13 roster): `edit_count` bumped ONLY inside
//! `snapshot()`, so whole families of project mutations —
//! figure rename, figure colour writes, layer writes,
//! `clip_set_angle` / `clip_set_root`, and the MCP
//! `project_rename` name write — never marked the project
//! dirty: the status strip's dirty dot under-reported and
//! the 25-edit autosave cadence could miss real changes.
//!
//! The observable surface for "dirty" over MCP is
//! `get_editor_status` (src/mcp.rs): it exposes
//! `edits_since_autosave` — the counter the strip's
//! autosave segment reads (the dirty dot itself read it
//! until RM24 moved the dot to the named-save state) —
//! alongside `undo_depth` / `redo_depth`. So every pin in
//! this suite is a pair of readings of that one status
//! call, taken before and after one MCP edit:
//!
//!   * a SNAPSHOT family advances BOTH counters by one
//!     (one counted edit, one undo step — the house rule);
//!   * a MARK_DIRTY family advances `edits_since_autosave`
//!     by one and leaves the undo depth untouched (dirty +
//!     autosave counting WITHOUT an undo step, the roster's
//!     `mark_dirty()` seam contract);
//!   * a REFUSAL advances NEITHER counter and changes no
//!     state — refusals spend nothing.
//!
//! The families pinned here, and the verdict each pin
//! encodes:
//!
//!   figure_recolor   SNAPSHOT — not an assumption: the
//!                    entering `recolor_figure` already
//!                    takes exactly one snapshot (its own
//!                    doc says so); the wave's work is the
//!                    dirty half, which the snapshot now
//!                    also being a counted edit provides.
//!   figure_rename    MARK_DIRTY — ASSUMED (see below).
//!   figure_set_color MARK_DIRTY — ASSUMED (see below).
//!   project_rename   MARK_DIRTY — B5's roster brief says
//!                    the name write "marks dirty" in so
//!                    many words; the undo half (no step)
//!                    is the assumed part.
//!   clip_set_angle   MARK_DIRTY — ASSUMED (see below).
//!   clip_set_root    MARK_DIRTY — ASSUMED (see below).
//!
//! RM24 DISPOSITION (Roadmap Wave 1, P-14 — audit A15
//! H3): the census this suite waited for eventually
//! ruled the other way for every family above except
//! one. The non-undoable-mutation census is CLOSED:
//! figure_rename, figure_set_color and the clip pose
//! tweaks are SNAPSHOT family now (the tweaks
//! coalesced — a run of them is ONE step, which is
//! what the flood concern actually needed), and their
//! pins below flipped mechanically, exactly as the
//! assumptions note promised they would.
//! project_rename's session-name write is the ONE
//! surviving MARK_DIRTY pin: undoing it would restore
//! a name whose file no longer exists under it, so it
//! stays on the seam by the census's own reasoning.
//!
//! ASSUMPTIONS, flagged honestly: B1's census
//! (docs/wave13/DIRTY_CENSUS.md) did not exist when this
//! suite was written, so the mark_dirty verdicts above are
//! coded against the roster contract plus the entering
//! source's own stated design, not against the census:
//! figure rename and `figure_set_color` are documented
//! plain field writes that take no snapshot BY DESIGN on
//! both the panel and MCP paths (figure_ops.rs), and the
//! clip pose tweaks' tool descriptions pin "NO undo
//! snapshot (pose-tweak rule)" as deliberate — for each,
//! the only fix consistent with the wave's contract
//! (dirty without disturbing the deliberate undo
//! behaviour) is the seam. If the census rules otherwise
//! for any family, that family's pin here is the flip the
//! integrator makes — the counter deltas are stated per
//! family precisely so the flip is mechanical.
//!
//! Deliberately NOT pinned: the layer locked/hidden/
//! opacity writes (the third E8 family). MCP does carry
//! layer-adjacent writers (`layers_set_all_visible`,
//! `layers_invert_visibility`, `figure_cycle_opacity`),
//! but no census verdict for them was available to this
//! session and their entering snapshot behaviour is a
//! bulk-op question for B1/B2; their unit pins belong to
//! the owning sessions. Nothing here fakes coverage of
//! them.
//!
//! Every step goes through MCP messages; direct
//! filesystem access is used only for what an MCP client
//! itself would observe from outside the protocol (that
//! the promised project files exist). The project-rename
//! session enters a per-test temp directory because the
//! named-project tools resolve against `shell::projects_dir()`,
//! which under portable mode is `./projects/` against the
//! process current directory; it is this file's only test
//! that does so, and it restores the directory on drop.

use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

use stykanimate_rs::mcp::McpServer;

// ---------------------------------------------------------------------
// Fixtures and the dispatch driver (the Wave 12 B10 pattern)
// ---------------------------------------------------------------------

/// A per-test scratch directory under the OS temp dir:
/// stale leftovers from a crashed run are removed up front
/// and the caller removes the directory again when done.
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "styk-wave13-e2e-dirty-{}-{tag}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Restores the process current directory on drop,
/// including when the session panics. Only the
/// project-rename session needs it.
struct CwdGuard(PathBuf);

impl CwdGuard {
    fn enter(dir: &std::path::Path) -> Self {
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

/// One MCP `tools/call`, decoded the way a client decodes
/// it: the JSON document in the single text content block,
/// plus the protocol's error flag. A protocol-level error
/// response (for example an unknown tool) is folded into
/// the same shape with its message as the text.
struct CallOutcome {
    payload: Value,
    text: String,
    is_error: bool,
}

/// A small dispatch driver over one `McpServer`: sequential
/// request ids, JSON-RPC envelope assertions on every reply,
/// and ok/err helpers so the sessions below read as sessions.
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

// ---------------------------------------------------------------------
// The counter read — the suite's one witness
// ---------------------------------------------------------------------

/// The three counters `get_editor_status` exposes, read in
/// one status call: edits since the autosave baseline (the
/// dirty dot's own input) and the undo/redo depths (the
/// undo-step contract's witness).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Counters {
    edits: u64,
    undo: u64,
    redo: u64,
}

fn counters(mcp: &mut Dispatch) -> Counters {
    let status = mcp.call_ok("get_editor_status", json!({}));
    Counters {
        edits: status["edits_since_autosave"]
            .as_u64()
            .expect("get_editor_status reports edits_since_autosave"),
        undo: status["undo_depth"]
            .as_u64()
            .expect("get_editor_status reports undo_depth"),
        redo: status["redo_depth"]
            .as_u64()
            .expect("get_editor_status reports redo_depth"),
    }
}

fn assert_counters(mcp: &mut Dispatch, want: Counters, what: &str) {
    assert_eq!(counters(mcp), want, "{what}");
}

// ---------------------------------------------------------------------
// Figure families: rename + base colour via their own
// snapshots (RM24; Wave 13 had them on the seam),
// recolour via its one snapshot
// ---------------------------------------------------------------------

#[test]
fn figure_rename_and_set_color_take_one_undo_step_each() {
    let mut mcp = Dispatch::new();
    mcp.call_ok("new_project", json!({ "name": "Wave13 Dirty Figures" }));

    // `new_project` swaps the project in, which restarts
    // the dirty baseline: a fresh session reads clean.
    let base = counters(&mut mcp);
    assert_eq!(base.edits, 0, "a fresh project reads clean: {base:?}");

    // (1) Rename: the write lands (the reply and the
    // figure list both read the trimmed name back) and
    // the project is dirty afterwards — one counted edit
    // AND one undo step (SNAPSHOT family since RM24;
    // the Wave 13 mark_dirty verdict is closed, see the
    // header's disposition note).
    let renamed = mcp.call_ok(
        "figure_rename",
        json!({ "figure": 0, "name": "  Hero  " }),
    );
    assert_eq!(renamed["name"], "Hero", "the rename reads back: {renamed}");
    let figures = mcp.call_ok("list_figures", json!({}));
    assert_eq!(figures["figures"][0]["name"], "Hero");
    assert_counters(
        &mut mcp,
        Counters { edits: base.edits + 1, undo: base.undo + 1, ..base },
        "figure_rename counts one edit and takes one undo step",
    );

    // (2) The base-colour write (`figure_set_color`, the
    // colour-picker path): same contract — dirty, one
    // (coalesced-class) undo step.
    let coloured = mcp.call_ok(
        "figure_set_color",
        json!({ "figure": 0, "color": 0x3366CC }),
    );
    assert_eq!(coloured["color"], 0x3366CC);
    let figures = mcp.call_ok("list_figures", json!({}));
    assert_eq!(figures["figures"][0]["color"], 0x3366CC);
    assert_counters(
        &mut mcp,
        Counters { edits: base.edits + 2, undo: base.undo + 2, ..base },
        "figure_set_color counts one more edit and one more undo step",
    );

    // (3) Refusals spend nothing: a blank rename, and
    // both writers aimed at a figure that does not
    // exist, are tool errors; the counters and the
    // stored values are exactly as they were.
    let before = counters(&mut mcp);
    let err = mcp.call_err(
        "figure_rename",
        json!({ "figure": 0, "name": "   " }),
    );
    assert!(
        err.contains("empty"),
        "the blank rename names its refusal: {err}"
    );
    mcp.call_err("figure_rename", json!({ "figure": 99, "name": "Ghost" }));
    mcp.call_err("figure_set_color", json!({ "figure": 99, "color": 1 }));
    assert_counters(&mut mcp, before, "refused figure writes spend nothing");
    let figures = mcp.call_ok("list_figures", json!({}));
    assert_eq!(figures["figures"][0]["name"], "Hero");
    assert_eq!(figures["figures"][0]["color"], 0x3366CC);
}

#[test]
fn figure_recolor_is_exactly_one_undo_step_and_counts_as_an_edit() {
    let mut mcp = Dispatch::new();
    mcp.call_ok("new_project", json!({ "name": "Wave13 Recolour" }));
    let base = counters(&mut mcp);
    assert_eq!(base.edits, 0, "a fresh project reads clean: {base:?}");

    // The whole-figure recolour writes every node's own
    // colour under ONE snapshot however many nodes the
    // figure has (the op's entering, documented
    // behaviour): the counters advance together — one
    // edit, one undo step. The reply's colour is the
    // stored override read back from the model, not the
    // argument echoed.
    let out = mcp.call_ok(
        "figure_recolor",
        json!({ "figure": 0, "color": 0x224466 }),
    );
    assert_eq!(out["color"], 0x224466, "the stored colour reads back: {out}");
    assert_counters(
        &mut mcp,
        Counters { edits: base.edits + 1, undo: base.undo + 1, ..base },
        "figure_recolor is one counted edit and exactly one undo step",
    );

    // The step is a real one: undo spends it into the
    // redo stack, redo takes it back. (The edit counter
    // is deliberately not pinned across undo/redo — the
    // session still differs from its saved state, which
    // is what the counter exists to say.)
    mcp.call_ok("undo", json!({}));
    let after_undo = counters(&mut mcp);
    assert_eq!(after_undo.undo, base.undo, "one undo spends the recolour step");
    assert_eq!(after_undo.redo, base.redo + 1, "the spent step waits on the redo stack");
    mcp.call_ok("redo", json!({}));
    let after_redo = counters(&mut mcp);
    assert_eq!(after_redo.undo, base.undo + 1, "redo restores the recolour step");
    assert_eq!(after_redo.redo, base.redo, "the redo stack drains");

    // A refused recolour (no such figure) spends
    // nothing — no snapshot, no counted edit.
    let before = counters(&mut mcp);
    mcp.call_err("figure_recolor", json!({ "figure": 99, "color": 1 }));
    assert_counters(&mut mcp, before, "a refused recolour spends nothing");
}

// ---------------------------------------------------------------------
// Project rename: the session name write marks dirty
// ---------------------------------------------------------------------

#[test]
fn project_rename_marks_dirty_without_an_undo_step() {
    let root = temp_dir("rename");
    let root = fs::canonicalize(&root).expect("canonicalise temp dir");
    let _cwd_guard = CwdGuard::enter(&root);
    let projects_dir = root.join("projects");
    let mut mcp = Dispatch::new();

    // Give the session a current file: create, then the
    // named save (which is also the dirty baseline — the
    // Wave 11-pinned behaviour this session leans on).
    mcp.call_ok("new_project", json!({ "name": "Wave13 Dirty Project" }));
    let saved = mcp.call_ok("save_named_project", json!({}));
    assert_eq!(saved["saved"], true, "the named save lands: {saved}");
    let main_path = projects_dir.join("wave13-dirty-project.json");
    assert!(main_path.exists(), "the named save wrote {main_path:?}");
    let base = counters(&mut mcp);
    assert_eq!(base.edits, 0, "a named save restarts the dirty baseline");

    // The rename: the file moves to the new name's slug,
    // the session's name follows — and that name write is
    // a real project change, so the session reads dirty
    // afterwards. NO undo step (mark_dirty verdict; see
    // the header's assumptions note).
    let renamed = mcp.call_ok(
        "project_rename",
        json!({ "new_name": "Wave13 Renamed Project" }),
    );
    assert_eq!(renamed["name"], "Wave13 Renamed Project");
    let renamed_path = projects_dir.join("wave13-renamed-project.json");
    assert!(renamed_path.exists(), "the renamed file exists");
    assert!(!main_path.exists(), "the old file is gone after the rename");
    let summary = mcp.call_ok("project_summary", json!({}));
    assert_eq!(summary["name"], "Wave13 Renamed Project");
    assert_counters(
        &mut mcp,
        Counters { edits: base.edits + 1, ..base },
        "project_rename counts one edit and takes no undo step",
    );

    // Refusals spend nothing. A copy on disk gives the
    // second refusal its existing target: renaming onto
    // it must refuse, leaving the counters, the session
    // name and both files exactly as they were. (A blank
    // name refuses even earlier, in the handler.)
    let copy = mcp.call_ok("save_project_copy", json!({}));
    assert_eq!(copy["name"], "Wave13 Renamed Project copy");
    let copy_path = projects_dir.join("wave13-renamed-project-copy.json");
    assert!(copy_path.exists(), "the copy file exists");
    let before = counters(&mut mcp);
    mcp.call_err("project_rename", json!({ "new_name": "   " }));
    let err = mcp.call_err(
        "project_rename",
        json!({ "new_name": "Wave13 Renamed Project copy" }),
    );
    assert!(!err.is_empty(), "the refusal says why: {err}");
    assert_counters(&mut mcp, before, "refused renames spend nothing");
    let summary = mcp.call_ok("project_summary", json!({}));
    assert_eq!(summary["name"], "Wave13 Renamed Project");
    assert!(renamed_path.exists(), "the refused rename left the source file");
    assert!(copy_path.exists(), "the refused rename left the target file");

    let _ = fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------------
// Clip pose tweaks: dirty via one coalesced step per
// gesture run (RM24; Wave 13 had them on the seam)
// ---------------------------------------------------------------------

#[test]
fn clip_pose_tweaks_spend_one_coalesced_step_per_run() {
    let mut mcp = Dispatch::new();
    mcp.call_ok("new_project", json!({ "name": "Wave13 Dirty Clip" }));

    // Build a clip the only way MCP can: four timeline
    // frames, then `clip_create` from figure 0, then
    // enter its edit session. The counter baseline is
    // taken AFTER the setup, so the setup's own undo
    // steps are not this test's business.
    for want in [2, 3, 4] {
        let framed = mcp.call_ok("add_frame", json!({}));
        assert_eq!(framed["frames"], want, "add_frame reports {want} frames");
    }
    mcp.call_ok("clip_create", json!({ "figure": 0 }));
    let entered = mcp.call_ok("clip_enter", json!({ "index": 0 }));
    assert_eq!(entered["ok"], true, "the session opens: {entered}");
    let base = counters(&mut mcp);

    // (1) Angle tweak: the write lands in the clip's own
    // frame (read back through `clip_get_pose`) and the
    // project counts the edit — WITH one undo step.
    // Per-angle tweaks taking a step EACH would flood
    // the history (the entering tool description's
    // stated reason for the pose-tweak rule); RM24's
    // answer is coalescing, not the seam: a run of
    // tweaks is one step (see the header's disposition
    // note).
    let set = mcp.call_ok(
        "clip_set_angle",
        json!({ "node": 3, "degrees": 30 }),
    );
    assert_eq!(set["degrees"], 30.0, "the stored angle reads back: {set}");
    let pose = mcp.call_ok("clip_get_pose", json!({}));
    assert_eq!(
        pose["angles"][3].as_f64(),
        Some(30.0),
        "the clip frame holds the new angle: {pose}"
    );
    assert_counters(
        &mut mcp,
        Counters { edits: base.edits + 1, undo: base.undo + 1, ..base },
        "clip_set_angle counts one edit and takes one undo step",
    );

    // (2) Root tweak: one more counted edit, and the
    // step FOLDS into the angle tweak's — same gesture
    // family ("Movieclip frame"), no history navigation
    // between them — so the depth does not move again.
    let rooted = mcp.call_ok(
        "clip_set_root",
        json!({ "root": [123.0, 234.0] }),
    );
    assert_eq!(rooted["ok"], true, "the root write lands: {rooted}");
    let pose = mcp.call_ok("clip_get_pose", json!({}));
    assert_eq!(
        pose["root"],
        json!([123.0, 234.0]),
        "the clip frame holds the new root: {pose}"
    );
    assert_counters(
        &mut mcp,
        Counters { edits: base.edits + 2, undo: base.undo + 1, ..base },
        "clip_set_root counts one more edit folded into the same step",
    );

    // (3) A refused tweak spends nothing: a joint the
    // clip does not have is a tool error, and neither
    // counter moves.
    let before = counters(&mut mcp);
    let err = mcp.call_err(
        "clip_set_angle",
        json!({ "node": 99, "degrees": 10 }),
    );
    assert!(
        err.contains("no joint"),
        "the refusal names the missing joint: {err}"
    );
    assert_counters(&mut mcp, before, "a refused clip tweak spends nothing");

    mcp.call_ok("clip_exit", json!({}));
}
