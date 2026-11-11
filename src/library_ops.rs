//! Figure-library operations that live in the engine so
//! every surface — shell, MCP, scripts — shares one
//! implementation.
//!
//! The first resident is the saved-entry rename,
//! [`EditorCore::rename_library_entry`]: until the
//! Focused Wave (post-Wave 14, squad A) it existed only
//! as `rename_saved` in the shell's library panel
//! (`src/rune_sec/library_panel.rs`), which is why the
//! Wave 14 parity census recorded figure-library entry
//! rename as gap G5 — file-backed shell logic with no
//! lib op beneath it. The op below is that panel
//! function moved unchanged: every refusal rule and
//! every status line is the panel's own, so the panel
//! wrapper and this file can never drift apart on what
//! a rename allows.
//!
//! Bookkeeping contract, stated plainly because it is
//! the deliberate exception to the MCP layer's
//! one-undo-step-per-mutation rule: **a library rename
//! is a file operation, NOT an undo step.** It renames
//! a file on disk and rewrites the entry's name in
//! `EditorCore::library` — neither is part of the
//! project the undo stack snapshots — and the panel
//! rename it mirrors takes no snapshot and marks no
//! dirty. `tool_pose_library_rename` (figure_ops.rs)
//! follows the same contract for the pose library.
//! Nothing here calls `snapshot` or `mark_dirty`.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::editor::EditorCore;

/// The prefix `load_library` (editor.rs) gives the
/// entries it constructs in code: `"Built-in: Humanoid"`,
/// `"Built-in: Quadruped"`, `"Built-in: Mini"`. The
/// shell's library panel carries the same literal as
/// its own `BUILTIN_PREFIX` (rune_sec is a bin-side
/// module this crate cannot import); the two spellings
/// name one fact — an entry whose name starts with
/// this prefix is built in and has no backing file.
pub const LIBRARY_BUILTIN_PREFIX: &str = "Built-in: ";

/// True when a library entry is one of `load_library`'s
/// built-ins (name carries [`LIBRARY_BUILTIN_PREFIX`]).
pub fn is_builtin(name: &str) -> bool {
    name.starts_with(LIBRARY_BUILTIN_PREFIX)
}

/// The directory saved library entries live in: `./library`
/// relative to the process current directory — the SAME
/// folder `load_library` reads (editor.rs) and
/// `save_figure_to_library` writes. It MUST stay that
/// folder: the rename/delete resolver below addresses the
/// files the loader sees, and the e2e suites isolate
/// libraries by changing the current directory.
///
/// Roadmap Wave 1 (RM9) moved this one resolver to the
/// AppDirs store dir (`shell::library_dir`) while load
/// and save stayed CWD-relative, so rename/delete
/// addressed files in a directory the loader never reads
/// — every rename failed with "No such file or directory"
/// (round 21: all three `focused_e2e_library_rename`
/// tests). Reverted to the pre-RM9 resolution. Completing
/// the AppDirs migration for the 2D library means moving
/// load + save together with this resolver, as one change
/// — not this round.
fn library_dir() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("library")
}

/// The backing file of the library entry whose STEM is
/// `name`: `./library/<name>.json` — the stem→file
/// step. Only meaningful for saved entries; built-ins
/// have no file. Since Wave V (V1) an entry's DISPLAY
/// name is stored inside its file and need not equal
/// its stem ("Styk" lives in `styk.json`); resolving a
/// display name to its file is [`library_entry_file`]'s
/// job, not this function's.
pub fn library_file_for(name: &str) -> PathBuf {
    library_dir().join(format!("{name}.json"))
}

/// Resolve a saved entry's DISPLAY name to the file
/// that backs it. An entry's file is the one it was
/// saved or renamed under, whose stem is the slug of
/// the name at that moment — and since Wave V (V1) the
/// display name is stored inside the file, the two can
/// differ. Resolution: the verbatim file
/// (`<name>.json`) when it exists — every entry whose
/// name IS its stem (all libraries written before the
/// stored name existed, and any hand-placed file)
/// resolves exactly as it always did; otherwise the
/// slug's file (`<sanitize_stem(name)>.json`), where a
/// stored-name entry lives. When neither exists the
/// slug path is returned, so a caller's fs operation
/// fails against the canonical location with the
/// honest error.
pub fn library_entry_file(name: &str) -> PathBuf {
    library_entry_file_in(&library_dir(), name)
}

/// [`library_entry_file`] against an explicit
/// directory — the `*_in` test seam the pose library's
/// `fop_*_in` helpers (figure_ops.rs) set the pattern
/// for.
pub fn library_entry_file_in(dir: &Path, name: &str) -> PathBuf {
    let verbatim = dir.join(format!("{name}.json"));
    if verbatim.exists() {
        return verbatim;
    }
    dir.join(format!("{}.json", sanitize_stem(name)))
}

/// Best-effort: rewrite the display name stored inside
/// the library file at `path` to `name` (the stored
/// name is `save_figure_to_library`'s — see
/// `EditorCore::library_file_text`). A rename moves a
/// file whose stored name still says the OLD name;
/// without this the next load would resurrect it.
/// Best-effort by design: the rename op never parses
/// its backing files (it only moves them), so a file
/// that does not parse as a JSON object is left
/// byte-identical and the rename proceeds. Every file
/// the app itself writes parses, so in practice the
/// stored name always follows the rename.
fn sync_library_stored_name(path: &Path, name: &str) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let Ok(mut value) = serde_json::from_str::<Value>(&text) else {
        return;
    };
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    if obj.get("name").and_then(Value::as_str) == Some(name) {
        return;
    }
    obj.insert("name".to_string(), Value::String(name.to_string()));
    if let Ok(text) = serde_json::to_string_pretty(&value) {
        let _ = std::fs::write(path, text);
    }
}

/// Sanitise typed rename text into a library file stem:
/// trim, then the save path's own slug — spaces→`-`,
/// lowercased (`save_figure_to_library`, editor.rs)
/// — plus one guard the save path never needs because
/// figure names come from inside the app: path
/// separators (`/`, `\`) also become `-`, so a typed
/// name can never make [`library_file_for`] point
/// outside `./library`. May return an empty string
/// (blank input); the caller refuses that case.
pub fn sanitize_stem(raw: &str) -> String {
    raw.trim()
        .replace(' ', "-")
        .replace('/', "-")
        .replace('\\', "-")
        .to_lowercase()
}

/// True when library slot `except` may NOT take the stem
/// `stem` because a DIFFERENT entry already carries it.
/// The entry's own current name is not a collision (the
/// same-name case is settled by the caller before this
/// is consulted). Built-ins are entries too and are
/// checked like any other, though a sanitised stem can
/// never actually equal a `Built-in: ` name (the slug
/// lowercases and dashes the prefix away).
pub fn name_taken(core: &EditorCore, stem: &str, except: usize) -> bool {
    core.library
        .iter()
        .enumerate()
        .any(|(j, (name, _))| j != except && name == stem)
}

impl EditorCore {
    /// Rename saved library entry `i` to the stem
    /// sanitised from `raw_new`: rename the backing
    /// file FIRST (`std::fs::rename`), then update the
    /// vec entry — the library delete contract mirrored.
    /// The entry keeps its slot, so `library_index`
    /// needs no clamp.
    ///
    /// Wave V (V1): the entry's DISPLAY name need not be
    /// its file's stem any more (the name is stored
    /// inside the file), so the backing file is resolved
    /// through [`library_entry_file`] and "already has
    /// that name" compares stems — renaming "Styk" to
    /// "Styk" settles, as renaming "alpha" to "Alpha"
    /// always did. The entry still TAKES the sanitised
    /// stem (the rename contract is unchanged), and the
    /// name stored inside the file is first synced to
    /// that stem (best-effort, see
    /// [`sync_library_stored_name`]), so a reload can
    /// never resurrect the pre-rename name. If the fs
    /// rename then fails, the entry keeps its old name
    /// in-session; the synced stored name is the only
    /// trace, and it names the file by the stem the user
    /// asked for.
    ///
    /// Returns whether the rename session is settled:
    /// `true` when the caller should close (renamed,
    /// the sanitised name already IS the entry's stem,
    /// or `i` is a stale out-of-range slot — a silent
    /// no-op, as in the library delete); `false` when
    /// the user should keep editing, with the reason on
    /// `self.status` (built-in refusal, empty stem,
    /// name/file collision, or the fs rename failing).
    /// On any failure the entry keeps its old name —
    /// the caller never claims a rename the disk
    /// refused. A target file that exists but is not a
    /// loaded entry (e.g. an unparseable `.json` the
    /// loader skipped) is also a refusal: `fs::rename`
    /// would silently overwrite it on Unix.
    pub fn rename_library_entry(&mut self, i: usize, raw_new: &str) -> bool {
        let Some((name, _)) = self.library.get(i) else {
            return true;
        };
        let old = name.clone();
        if is_builtin(&old) {
            self.status = format!("\"{old}\" is built in and can't be renamed.");
            return false;
        }
        let new_stem = sanitize_stem(raw_new);
        if new_stem.is_empty() {
            self.status = "Type a new name for the library figure first.".to_string();
            return false;
        }
        // The entry's backing file, resolved through the
        // stored-name rule: the display name need not be
        // the file's stem. The same-name settle compares
        // STEMS — the entry already sits under the stem
        // the typed name sanitises to.
        let old_path = library_entry_file(&old);
        let old_stem = old_path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| sanitize_stem(&old));
        if new_stem == old_stem {
            self.status = format!("\"{old}\" already has that name.");
            return true;
        }
        if name_taken(self, &new_stem, i) {
            self.status = format!("A library figure named \"{new_stem}\" already exists.");
            return false;
        }
        // A different entry already DISPLAYS the typed
        // name (possible since display names left the
        // stems): refuse with the same sentence, quoting
        // the name the user actually typed.
        let new_display = raw_new.trim();
        if name_taken(self, new_display, i) {
            self.status = format!("A library figure named \"{new_display}\" already exists.");
            return false;
        }
        let new_path = library_file_for(&new_stem);
        if new_path != old_path && new_path.exists() {
            self.status =
                format!("A file named \"{new_stem}.json\" already exists in the library folder.");
            return false;
        }
        // Every refusal is behind us: sync the name
        // stored inside the file to the stem the entry
        // is about to take, then move the file.
        sync_library_stored_name(&old_path, &new_stem);
        match std::fs::rename(&old_path, &new_path) {
            Ok(()) => {
                self.library[i].0 = new_stem.clone();
                self.status = format!("Renamed \"{old}\" to \"{new_stem}\".");
                true
            }
            Err(e) => {
                self.status = format!("Library rename failed for \"{old}\": {e}");
                false
            }
        }
    }
}

// ---------------------------------------------------------------------
// MCP handlers. mcp_tools.rs's argument helpers are
// private to that file, so the small parsers below are
// deliberate local duplicates with `lop_` names (the
// same convention figure_ops/fx_ops follow). Every
// handler validates first and returns Err(String) on
// bad input — never panics — and mirrors the refusal
// wording of the panel/op it stands in for.
// ---------------------------------------------------------------------

fn lop_get<'a>(args: &'a Value, key: &str) -> Option<&'a Value> {
    args.get(key).filter(|v| !v.is_null())
}

fn lop_as_usize(v: &Value, key: &str) -> Result<usize, String> {
    if let Some(u) = v.as_u64() {
        return usize::try_from(u).map_err(|_| format!("argument '{key}' is too large"));
    }
    if let Some(f) = v.as_f64() {
        if f >= 0.0 && f.fract() == 0.0 && f <= usize::MAX as f64 {
            return Ok(f as usize);
        }
    }
    Err(format!("argument '{key}' must be a non-negative integer"))
}

fn lop_req_usize(args: &Value, key: &str) -> Result<usize, String> {
    match lop_get(args, key) {
        Some(v) => lop_as_usize(v, key),
        None => Err(format!("missing required argument '{key}'")),
    }
}

fn lop_req_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    match lop_get(args, key) {
        Some(v) => v
            .as_str()
            .ok_or_else(|| format!("argument '{key}' must be a string")),
        None => Err(format!("missing required argument '{key}'")),
    }
}

/// MCP `figure_library_rename`: the library panel's
/// per-row Rename — [`EditorCore::rename_library_entry`],
/// the op the panel itself calls. Entries are addressed
/// by library index, the panel's own addressing (the
/// rows carry real library slots). Every refusal the op
/// reports on `core.status` — built-in entry, empty
/// sanitised name, another entry with that name, a
/// target file already on disk, a failed fs rename —
/// becomes the `Err`, quoting the offending name
/// exactly as the panel words it. The same-name case is
/// not an error (the panel settles the session): it
/// answers `renamed: false`, mirroring
/// `tool_pose_library_rename`. A file operation, NOT an
/// undo step — see the module docs.
pub fn tool_figure_library_rename(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let index = lop_req_usize(args, "index")?;
    let raw_new = lop_req_str(args, "new_name")?;
    let Some((name, _)) = core.library.get(index) else {
        return Err(format!("no library entry at index {index}"));
    };
    let old = name.clone();
    if !core.rename_library_entry(index, raw_new) {
        return Err(core.status.clone());
    }
    let new_name = core.library[index].0.clone();
    let renamed = new_name != old;
    Ok(json!({
        "ok": true,
        "renamed": renamed,
        "index": index,
        "name": new_name,
        "status": core.status,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_prefix_matches_load_library_names() {
        assert!(is_builtin("Built-in: Humanoid"));
        assert!(is_builtin("Built-in: Mini"));
        assert!(!is_builtin("My Figure"));
        assert!(!is_builtin("Built-in:"));
        assert!(!is_builtin(""));
    }

    #[test]
    fn stem_sanitising_matches_the_panel_rules() {
        assert_eq!(sanitize_stem("My Hero"), "my-hero");
        assert_eq!(sanitize_stem("CAPS LOCK"), "caps-lock");
        assert_eq!(sanitize_stem("already-a-stem"), "already-a-stem");
        assert_eq!(sanitize_stem("  Padded Name  "), "padded-name");
        assert_eq!(sanitize_stem("a/b\\c"), "a-b-c");
        assert_eq!(sanitize_stem(""), "");
        assert_eq!(sanitize_stem("   "), "");
    }

    #[test]
    fn library_file_is_the_stem_plus_json_in_library_dir() {
        let p = library_file_for("my-hero");
        assert_eq!(p.file_name().unwrap(), "my-hero.json");
        assert_eq!(p.parent().unwrap().file_name().unwrap(), "library");
        assert_eq!(library_file_for("a.b").file_name().unwrap(), "a.b.json");
    }

    /// A scratch dir under the OS temp dir for the
    /// resolution/sync pins (the `*_in` seam exists so
    /// these never touch the process CWD's `./library`).
    fn scratch_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "stykanimate-v1-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create the scratch dir");
        dir
    }

    #[test]
    fn entry_file_resolution_prefers_the_verbatim_file() {
        // An entry whose name IS its stem (every library
        // written before the stored name existed, and any
        // hand-placed file) resolves to <name>.json even
        // when the slug's file also exists.
        let dir = scratch_dir("verbatim");
        std::fs::write(dir.join("Foo Bar.json"), "{}").expect("plant the verbatim file");
        std::fs::write(dir.join("foo-bar.json"), "{}").expect("plant the slug file");
        let got = library_entry_file_in(&dir, "Foo Bar");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(got, dir.join("Foo Bar.json"));
    }

    #[test]
    fn entry_file_resolution_falls_to_the_slug() {
        // A stored-name entry ("Styk" in styk.json) has
        // no verbatim file: the slug's file backs it.
        let dir = scratch_dir("slug");
        std::fs::write(dir.join("styk.json"), "{}").expect("plant the slug file");
        let got = library_entry_file_in(&dir, "Styk");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(got, dir.join("styk.json"));
    }

    #[test]
    fn entry_file_resolution_defaults_to_the_slug_path() {
        // Neither file exists: the slug path is returned
        // so the caller's fs operation fails there with
        // the honest error.
        let dir = scratch_dir("missing");
        let got = library_entry_file_in(&dir, "Ghost Figure");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(got, dir.join("ghost-figure.json"));
    }

    #[test]
    fn sync_library_stored_name_rewrites_and_preserves() {
        let dir = scratch_dir("sync");
        let path = dir.join("styk.json");
        std::fs::write(
            &path,
            "{\"name\": \"Styk\", \"nodes\": []}",
        )
        .expect("plant a stored-name file");
        sync_library_stored_name(&path, "styk-renamed");
        let text = std::fs::read_to_string(&path).expect("read it back");
        let value: Value = serde_json::from_str(&text).expect("still JSON");
        assert_eq!(value["name"], json!("styk-renamed"));
        assert_eq!(value["nodes"], json!([]), "the definition rides along untouched");
        // An unparseable file is left byte-identical —
        // the rename op never parses its backing files.
        let bad = dir.join("bad.json");
        std::fs::write(&bad, b"this is not json").expect("plant an unparseable file");
        sync_library_stored_name(&bad, "anything");
        let bytes = std::fs::read(&bad).expect("read the unparseable file back");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(bytes, b"this is not json");
    }

    #[test]
    fn rename_to_own_display_name_settles_for_stored_name_entries() {
        // Wave V (V1): a stored-name entry ("Styk", file
        // styk.json) renamed to its own display name
        // settles on the stem comparison — before the
        // fix this hunted for a `Styk.json` that does not
        // exist and failed the fs rename. The settle
        // precedes every fs step, so no files are needed.
        let mut core = EditorCore::default();
        core.library = vec![("Styk".to_string(), crate::model::FigureDef::simple())];
        assert!(core.rename_library_entry(0, "Styk"));
        assert!(
            core.status.contains("already has that name"),
            "status: {}",
            core.status
        );
        assert_eq!(core.library[0].0, "Styk", "the entry is unchanged");
    }

    #[test]
    fn name_taken_ignores_the_entry_itself() {
        let mut core = EditorCore::default();
        core.library = vec![
            ("alpha".to_string(), crate::model::FigureDef::simple()),
            ("beta".to_string(), crate::model::FigureDef::simple()),
        ];
        let alpha = 0;
        let beta = 1;
        assert!(name_taken(&core, "beta", alpha), "another entry's name is taken");
        assert!(!name_taken(&core, "alpha", alpha), "own name is not a collision");
        assert!(!name_taken(&core, "gamma", alpha), "unused name is free");
        assert!(name_taken(&core, "alpha", beta));
    }
}
