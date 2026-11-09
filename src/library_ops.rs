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

use std::path::PathBuf;

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

/// The backing file of a saved library entry:
/// `./library/<name>.json`. Exact by construction —
/// `load_library` names entries from file stems, so the
/// stem→file step is the precise inverse. Only
/// meaningful for saved entries; built-ins have no file.
pub fn library_file_for(name: &str) -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("library")
        .join(format!("{name}.json"))
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
    /// Returns whether the rename session is settled:
    /// `true` when the caller should close (renamed,
    /// the sanitised name already IS the entry's name,
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
        if new_stem == old {
            self.status = format!("\"{old}\" already has that name.");
            return true;
        }
        if name_taken(self, &new_stem, i) {
            self.status = format!("A library figure named \"{new_stem}\" already exists.");
            return false;
        }
        let old_path = library_file_for(&old);
        let new_path = library_file_for(&new_stem);
        if new_path.exists() {
            self.status =
                format!("A file named \"{new_stem}.json\" already exists in the library folder.");
            return false;
        }
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
