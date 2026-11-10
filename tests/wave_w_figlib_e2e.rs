//! Wave W, W19 — the figure library's migration
//! behaviour, end to end (Wave V, V1's contract).
//!
//! THE CONTRACT BEING PINNED (V1, findings
//! `wave-v/findings/v1.md`): since V1 a saved figure's
//! display name is stored INSIDE its library file as a
//! top-level `"name"` field, while the file itself
//! keeps the historical slug (spaces→`-`, lowercased).
//! Loading prefers the stored name and falls back to
//! the file stem for files that carry none — every
//! library written before the stored name existed.
//! Nothing is renamed, moved, or rewritten at load: a
//! legacy library shows exactly the names its user
//! already sees. Rename resolves the backing file
//! through `library_entry_file` (verbatim `<name>.json`
//! when it exists, else the slug's file), settles
//! same-name on a STEM comparison, takes the sanitised
//! stem as the entry's new name, and syncs the stored
//! name to that stem BEFORE moving the file, so a
//! reload can never resurrect the pre-rename name.
//!
//! THE FIXTURE: a legacy library mixing all three file
//! shapes the migration must survive, planted by hand
//! in a temp dir (the library is `./library` relative
//! to the process current directory, so each test
//! enters its own temp dir, serialised on a mutex —
//! the `wave_p_figlib.rs` discipline):
//!   * `legacy-one.json` — a bare `FigureDef` document
//!     with NO stored name: the pre-V1 save's shape.
//!     Listed by its stem, "legacy-one".
//!   * `styk.json` — a definition plus a stored name
//!     "Styk": the post-V1 save's shape, whose display
//!     name its file's stem cannot express.
//!   * `Foo Bar.json` — a hand-placed verbatim file:
//!     its name is no slug (space, capitals) and it
//!     carries no stored name. Listed by its stem,
//!     "Foo Bar", and resolved verbatim-first.
//! The steps then list, insert, save, rename, and
//! delete against that library through the MCP
//! registry (`McpServer`, the tree's e2e idiom) plus
//! the library ops, asserting DISPLAY NAMES and
//! BACKING FILES at every step: the on-disk slug
//! never changes, and nothing is ever orphaned.
//!
//! DELETE SURFACE, stated plainly: there is no
//! `figure_library_delete` tool, and the panel's
//! `delete_saved` lives in the binary
//! (`src/rune_sec/library_panel.rs`), which an
//! integration test cannot link — the same
//! reachability limit U15 recorded for the Home
//! light read (`tests/wave_u_e2e.rs`). `delete_saved`'s
//! entire name-resolution content is the lib fn
//! `library_entry_file`, which it calls on the
//! entry's display name; its own refusals (built-in,
//! out-of-range, keep-the-entry-on-failure) are
//! pinned by `library_panel.rs`'s in-crate tests.
//! The delete legs below therefore drive that exact
//! resolution, perform the removal it exists to aim,
//! and assert the truth a fresh load sees. What is
//! NOT re-proven here: the panel fn's status lines.

use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use stykanimate_rs::library_ops::{is_builtin, library_entry_file};
use stykanimate_rs::mcp::McpServer;
use stykanimate_rs::model::FigureDef;

/// Serialises this file's tests: they each change the
/// process current directory, which is process-global.
/// Poison-tolerant, as in `wave_p_figlib.rs`.
static FIGLIB_CWD_LOCK: Mutex<()> = Mutex::new(());

fn lock_cwd() -> std::sync::MutexGuard<'static, ()> {
    FIGLIB_CWD_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "styk-wave-w-figlib-{}-{tag}",
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
// MCP dispatch driver (the tree's e2e idiom, copied
// from wave_p_figlib.rs)
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

// ---------------------------------------------------------------------
// Library readings
// ---------------------------------------------------------------------

/// The library as `figure_library_list` reports it:
/// entry names in slot order (built-ins included).
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

/// The saved entries only (the product's own
/// `is_builtin` predicate filters the code-built
/// three), in listing order — listing order is file
/// order: `load_library` sorts the folder's paths.
fn saved_names(mcp: &mut Dispatch) -> Vec<String> {
    library_names(mcp)
        .into_iter()
        .filter(|n| !is_builtin(n))
        .collect()
}

/// The listing slot of the entry called `name`.
fn entry_index(mcp: &mut Dispatch, name: &str) -> u64 {
    let names = library_names(mcp);
    names
        .iter()
        .position(|n| n == name)
        .unwrap_or_else(|| panic!("no library entry named '{name}' in {names:?}"))
        as u64
}

fn undo_depth(mcp: &mut Dispatch) -> u64 {
    mcp.call_ok("get_editor_status", json!({}))["undo_depth"]
        .as_u64()
        .expect("get_editor_status reports undo_depth")
}

/// The exact set of files in the library folder,
/// sorted — the nothing-orphaned witness.
fn library_files(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(library_dir(dir))
        .expect("read the library folder")
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect();
    names.sort();
    names
}

/// The display name stored inside the library file
/// `file`, or `None` when it carries none.
fn stored_name(dir: &Path, file: &str) -> Option<String> {
    let text = fs::read_to_string(library_dir(dir).join(file))
        .unwrap_or_else(|_| panic!("read library file {file}"));
    let value: Value = serde_json::from_str(&text)
        .unwrap_or_else(|_| panic!("library file {file} parses as JSON"));
    value
        .get("name")
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// Just the file name a resolved library path points
/// at — the resolution assertions compare file names
/// (and existence), never whole paths, so a temp-dir
/// symlink can never make them lie.
fn resolved_file(name: &str) -> String {
    let path = library_entry_file(name);
    assert!(
        path.exists(),
        "the resolution for '{name}' points at a real file: {}",
        path.display()
    );
    path.file_name()
        .expect("a library path carries a file name")
        .to_string_lossy()
        .into_owned()
}

// ---------------------------------------------------------------------
// The mixed legacy library (the fixture)
// ---------------------------------------------------------------------

const LEGACY_FILE: &str = "legacy-one.json";
const STYK_FILE: &str = "styk.json";
const VERBATIM_FILE: &str = "Foo Bar.json";

/// A bare definition document — the pre-V1 save's
/// file shape. The honesty assert is part of the
/// fixture: if `FigureDef` ever grows a top-level
/// `name`, this stops being the legacy shape and the
/// file must be re-derived, not silently re-purposed.
fn bare_def_text() -> String {
    let text = serde_json::to_string_pretty(&FigureDef::simple())
        .expect("serialise the definition");
    let value: Value = serde_json::from_str(&text).expect("the document parses");
    assert!(
        value
            .as_object()
            .map(|obj| !obj.contains_key("name"))
            .unwrap_or(false),
        "a bare definition carries no top-level name — the pre-V1 file shape"
    );
    text
}

/// A definition plus a stored display name — the
/// post-V1 save's file shape, built exactly as
/// `EditorCore::library_file_text` builds it.
fn stored_def_text(name: &str) -> String {
    let mut value =
        serde_json::to_value(FigureDef::simple()).expect("serialise the definition");
    value
        .as_object_mut()
        .expect("a definition serialises to an object")
        .insert("name".to_string(), Value::String(name.to_string()));
    serde_json::to_string_pretty(&value).expect("serialise the stored-name document")
}

/// Plant the three-shape legacy library in `dir`.
fn plant_mixed_library(dir: &Path) {
    let lib = library_dir(dir);
    fs::create_dir_all(&lib).expect("create the library folder");
    fs::write(lib.join(LEGACY_FILE), bare_def_text()).expect("plant the legacy entry");
    fs::write(lib.join(STYK_FILE), stored_def_text("Styk"))
        .expect("plant the stored-name entry");
    fs::write(lib.join(VERBATIM_FILE), bare_def_text())
        .expect("plant the hand-placed verbatim entry");
}

// ---------------------------------------------------------------------
// Listing, inserting, saving — the read paths
// ---------------------------------------------------------------------

/// The mixed library lists under the migration's
/// names — stored name where one is stored, stem
/// where none is, the verbatim file under its own
/// stem — and every entry inserts under exactly the
/// name it lists. A new save still slugs its FILE
/// while storing the exact name inside it.
#[test]
fn mixed_legacy_library_lists_inserts_and_saves_with_exact_names() {
    let _lock = lock_cwd();
    let dir = temp_dir("mixed-read");
    let _cwd = CwdGuard::enter(&dir);
    plant_mixed_library(&dir);

    // Fixture honesty: the stored-name file still
    // parses as a bare definition (the extra field is
    // invisible to the definition reader), and the
    // legacy file carries no stored name to prefer.
    let styk_text = fs::read_to_string(library_dir(&dir).join(STYK_FILE))
        .expect("read the stored-name file");
    assert!(
        serde_json::from_str::<FigureDef>(&styk_text).is_ok(),
        "a stored-name file still parses as a bare FigureDef"
    );
    assert_eq!(stored_name(&dir, STYK_FILE).as_deref(), Some("Styk"));
    assert_eq!(stored_name(&dir, LEGACY_FILE), None);

    let mut mcp = Dispatch::new();

    // The listing: file order (Foo Bar.json <
    // legacy-one.json < styk.json), each entry under
    // the migration's name for its shape — and the
    // lossy stem "styk" appears NOWHERE as a name.
    assert_eq!(
        saved_names(&mut mcp),
        vec![
            "Foo Bar".to_string(),
            "legacy-one".to_string(),
            "Styk".to_string()
        ],
        "the mixed library lists stored names, stems, and the verbatim stem"
    );
    assert!(
        !library_names(&mut mcp).iter().any(|n| n == "styk"),
        "no entry is listed under the lower-cased stem any more"
    );

    // Insert by exact name: the inserted figure takes
    // the entry's display name, for every shape.
    for name in ["Foo Bar", "legacy-one", "Styk"] {
        let inserted = mcp.call_ok("figure_library_insert", json!({ "name": name }));
        assert_eq!(
            inserted["name"],
            json!(name),
            "inserting '{name}' yields a figure named '{name}'"
        );
    }
    // The lossy name is not addressable: nothing is
    // named "styk", so nothing inserts under it.
    let lossy = mcp.call("figure_library_insert", json!({ "name": "styk" }));
    assert!(lossy.is_error, "inserting the lossy stem must refuse");
    assert!(
        lossy.text.contains("no library entry named 'styk'"),
        "the refusal names the missing entry, got: {}",
        lossy.text
    );

    // A fresh save: the FILE is the slug, the stored
    // name (and so the listing) is exact.
    let added = mcp.call_ok(
        "add_figure",
        json!({ "kind": "mini", "name": "Brand New" }),
    );
    let figure = added["figure"]
        .as_u64()
        .unwrap_or_else(|| panic!("add_figure reply carries no figure index: {added}"));
    mcp.call_ok("figure_library_save", json!({ "figure": figure }));
    assert!(
        library_dir(&dir).join("brand-new.json").exists(),
        "the new save's file is the slug, as it always was"
    );
    assert_eq!(
        stored_name(&dir, "brand-new.json").as_deref(),
        Some("Brand New"),
        "the new save stores the figure's exact name inside the slug's file"
    );
    let names = saved_names(&mut mcp);
    assert!(
        names.iter().any(|n| n == "Brand New"),
        "the new entry lists under its exact name: {names:?}"
    );
    assert!(
        !names.iter().any(|n| n == "brand-new"),
        "the new entry is not listed under its slug: {names:?}"
    );
    assert_eq!(
        library_files(&dir),
        vec![
            VERBATIM_FILE.to_string(),
            "brand-new.json".to_string(),
            LEGACY_FILE.to_string(),
            STYK_FILE.to_string()
        ],
        "the folder holds exactly the three planted files plus the new slug"
    );

    drop(_cwd);
    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------
// Rename and delete — the mutation paths
// ---------------------------------------------------------------------

/// Against the same mixed library: the two collision
/// refusals the stored name makes possible (a DISPLAY
/// collision and a target-FILE collision, which are
/// different sentences), the same-name settle for a
/// stored-name entry, a real rename whose stored name
/// is synced before the file moves so a reload cannot
/// resurrect the old name, a rename of the verbatim
/// entry through its verbatim file — and finally the
/// delete legs, driven through `library_entry_file`,
/// the exact resolution the panel's delete delegates
/// to (see the module doc's delete-surface note).
#[test]
fn renames_refuse_settle_move_sync_and_never_resurrect() {
    let _lock = lock_cwd();
    let dir = temp_dir("mixed-mutate");
    let _cwd = CwdGuard::enter(&dir);
    plant_mixed_library(&dir);
    let mut mcp = Dispatch::new();

    let depth_before = undo_depth(&mut mcp);
    let names_before = saved_names(&mut mcp);
    assert_eq!(
        names_before,
        vec![
            "Foo Bar".to_string(),
            "legacy-one".to_string(),
            "Styk".to_string()
        ]
    );

    // The resolution the delete (and the rename)
    // aim with, per shape: the stored-name entry
    // resolves to the SLUG's file (no verbatim
    // "Styk.json" exists), the hand-placed entry to
    // its verbatim file, the legacy entry to its own.
    assert_eq!(resolved_file("Styk"), STYK_FILE);
    assert_eq!(resolved_file("Foo Bar"), VERBATIM_FILE);
    assert_eq!(resolved_file("legacy-one"), LEGACY_FILE);

    let legacy_slot = entry_index(&mut mcp, "legacy-one");

    // Collision A — DISPLAY: renaming onto a name
    // another entry displays. Only possible since
    // display names left the stems ("Styk" is no
    // entry's stem, yet it is taken — as a display).
    let display = mcp.call(
        "figure_library_rename",
        json!({ "index": legacy_slot, "new_name": "Styk" }),
    );
    assert!(display.is_error, "the display collision must refuse");
    assert!(
        display
            .text
            .contains("A library figure named \"Styk\" already exists."),
        "the refusal quotes the typed name, got: {}",
        display.text
    );

    // Collision B — FILE: a typed name nobody
    // displays ("STYK") whose stem's file is another
    // entry's backing file.
    let file = mcp.call(
        "figure_library_rename",
        json!({ "index": legacy_slot, "new_name": "STYK" }),
    );
    assert!(file.is_error, "the file collision must refuse");
    assert!(
        file.text
            .contains("A file named \"styk.json\" already exists"),
        "the refusal names the occupied file, got: {}",
        file.text
    );

    // Both refusals are pure: names, files, the
    // stored name, and the undo depth are untouched.
    assert_eq!(saved_names(&mut mcp), names_before, "no refusal renamed an entry");
    assert_eq!(
        library_files(&dir),
        vec![
            VERBATIM_FILE.to_string(),
            LEGACY_FILE.to_string(),
            STYK_FILE.to_string()
        ],
        "no refusal moved a file"
    );
    assert_eq!(stored_name(&dir, STYK_FILE).as_deref(), Some("Styk"));
    assert_eq!(
        undo_depth(&mut mcp),
        depth_before,
        "refused renames spend no undo step"
    );

    // The same-name settle, for a stored-name entry:
    // "Styk"→"Styk" compares stems (the resolved
    // file's stem is "styk") and settles — before V1
    // this hunted a nonexistent "Styk.json" and
    // failed. Not an error, nothing renamed.
    let styk_slot = entry_index(&mut mcp, "Styk");
    let settle = mcp.call_ok(
        "figure_library_rename",
        json!({ "index": styk_slot, "new_name": "Styk" }),
    );
    assert_eq!(settle["renamed"], json!(false), "the same name settles unrenamed");
    assert_eq!(settle["name"], json!("Styk"));
    assert!(
        settle["status"]
            .as_str()
            .unwrap_or_default()
            .contains("already has that name"),
        "the settle says so, got: {}",
        settle["status"]
    );
    assert_eq!(stored_name(&dir, STYK_FILE).as_deref(), Some("Styk"));
    assert!(library_dir(&dir).join(STYK_FILE).exists());

    // A real rename: the entry takes the sanitised
    // stem, the file moves to the stem's file, and
    // the stored name inside it was synced to the
    // stem first — it no longer says "Styk" anywhere.
    let renamed = mcp.call_ok(
        "figure_library_rename",
        json!({ "index": styk_slot, "new_name": "Styk Prime" }),
    );
    assert_eq!(renamed["renamed"], json!(true));
    assert_eq!(renamed["name"], json!("styk-prime"));
    assert_eq!(
        saved_names(&mut mcp),
        vec![
            "Foo Bar".to_string(),
            "legacy-one".to_string(),
            "styk-prime".to_string()
        ],
        "the renamed entry lists under its new stem"
    );
    assert!(
        !library_dir(&dir).join(STYK_FILE).exists(),
        "the old backing file moved"
    );
    assert_eq!(
        stored_name(&dir, "styk-prime.json").as_deref(),
        Some("styk-prime"),
        "the stored name was synced to the new stem, not left as the old name"
    );
    assert_eq!(
        undo_depth(&mut mcp),
        depth_before,
        "a successful rename is a file operation: no undo step"
    );

    // RELOAD — a fresh core over the same folder.
    // The renamed entry loads under its new stem; the
    // pre-rename display name is nowhere: the synced
    // stored name is the only thing a load can read,
    // and it does not resurrect "Styk".
    let mut mcp2 = Dispatch::new();
    assert_eq!(
        saved_names(&mut mcp2),
        vec![
            "Foo Bar".to_string(),
            "legacy-one".to_string(),
            "styk-prime".to_string()
        ],
        "a reload shows the renamed entry under its new name only"
    );

    // The verbatim entry renames through its verbatim
    // file: the file that moves is "Foo Bar.json",
    // and the entry takes the typed name's stem.
    let foo_slot = entry_index(&mut mcp2, "Foo Bar");
    let foo = mcp2.call_ok(
        "figure_library_rename",
        json!({ "index": foo_slot, "new_name": "Foo Baz" }),
    );
    assert_eq!(foo["renamed"], json!(true));
    assert_eq!(foo["name"], json!("foo-baz"));
    assert!(
        !library_dir(&dir).join(VERBATIM_FILE).exists(),
        "the verbatim backing file moved"
    );
    assert!(
        library_dir(&dir).join("foo-baz.json").exists(),
        "the renamed entry's file is its new stem's file"
    );

    // DELETE legs — through `library_entry_file`,
    // the resolution the panel's `delete_saved`
    // delegates to (module doc): aim it at two of
    // the three remaining entries, remove what it
    // names, and let a fresh load tell the truth.
    assert_eq!(resolved_file("legacy-one"), LEGACY_FILE);
    fs::remove_file(library_entry_file("legacy-one"))
        .expect("remove the legacy entry's backing file");
    let mut mcp3 = Dispatch::new();
    assert_eq!(
        saved_names(&mut mcp3),
        vec!["foo-baz".to_string(), "styk-prime".to_string()],
        "the deleted legacy entry is gone from a fresh load; the rest stand"
    );

    assert_eq!(resolved_file("foo-baz"), "foo-baz.json");
    fs::remove_file(library_entry_file("foo-baz"))
        .expect("remove the renamed entry's backing file");

    // The folder's final state, exactly: the one
    // surviving entry's file — nothing orphaned, no
    // ghost of any earlier name.
    assert_eq!(
        library_files(&dir),
        vec!["styk-prime.json".to_string()],
        "only the surviving entry's file remains"
    );
    let mut mcp4 = Dispatch::new();
    assert_eq!(
        saved_names(&mut mcp4),
        vec!["styk-prime".to_string()],
        "a final load lists exactly the surviving entry"
    );

    drop(_cwd);
    let _ = fs::remove_dir_all(&dir);
}
