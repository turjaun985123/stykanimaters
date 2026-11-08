//! Rune-shell localisation wiring (Phase D, section 38 — i18n).
//!
//! The Rune shell does NOT adopt `rune-i18n` (audit
//! `docs/RUNE_API_AUDIT.md` Q12: it would replace a solved problem
//! with a build.rs + ICU4X stack). It reuses the project's own
//! table in `stykanimate_rs::i18n` — 28 languages × 70 keys, with
//! fallback current language → English → the key itself
//! (`i18n::t`, src/i18n.rs:235; fallback chain in `resolve`,
//! src/i18n.rs:223).
//!
//! Where the language lives: on the engine, not in shell state —
//! `EditorCore.lang: crate::i18n::Lang` (src/editor.rs:82,
//! default `Lang::En` at src/editor.rs:137, advanced by the
//! `cycle_lang` op at src/editor.rs:1906). So this layer needs
//! NO new `State` field: every helper takes `&EditorCore` and
//! reads `core.lang`, exactly the value the shell already holds.
//!
//! What this module adds over raw `i18n::t`:
//! - [`tr`] — the one call shell views make (core in, `String`
//!   out, matching Rune's `text(...)` consumption).
//! - [`language_options`] — the picker's data, derived from the
//!   table's own `Lang::all()` / `code()` / `label()`
//!   (src/i18n.rs:150 / :82 / :116), so a 29th language appears
//!   here automatically.
//! - A coverage helper ([`shell_key_coverage`]) listing which of
//!   the keys the new shell wants ([`SHELL_KEYS`]) actually exist
//!   in the table. Missing keys are REPORTED, never invented —
//!   their translations are a later pass; until then `tr` shows
//!   the English-fallback/key text via `i18n::t`'s existing
//!   behaviour.

use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::i18n::{self, Lang};

/// The current UI language, read from the engine (the shell
/// stores nothing of its own).
pub fn lang_of(core: &EditorCore) -> Lang {
    core.lang
}

/// Translate `key` into the core's current language.
///
/// Thin wrapper over `i18n::t(core.lang, key)` (src/i18n.rs:235):
/// missing translations fall back to English, and a key missing
/// from every table falls back to the key itself — so the shell
/// can adopt a key ahead of its translation pass and still show
/// debuggable text instead of a blank.
pub fn tr(core: &EditorCore, key: &str) -> String {
    i18n::t(core.lang, key).to_string()
}

/// Translate `key` into an explicit `lang` (e.g. previewing a
/// language in a picker before it is applied to the core).
pub fn tr_lang(lang: Lang, key: &str) -> String {
    i18n::t(lang, key).to_string()
}

/// All 28 languages as `(BCP 47 code, native name)` pairs, in
/// the table's picker order — the data behind a language
/// dropdown. Pure: derived only from `Lang::all()` /
/// `Lang::code()` / `Lang::label()` (src/i18n.rs:150/82/116).
pub fn language_options() -> Vec<(&'static str, &'static str)> {
    Lang::all().iter().map(|l| (l.code(), l.label())).collect()
}

/// Does `key` exist in the i18n table?
///
/// Probed through the public API only (the tables themselves
/// are private): `i18n::t(Lang::En, key)` returns the key itself
/// iff the key is absent from every table, and every real key's
/// English text differs from its key string, so
/// `t(En, key) != key` is a sound existence test.
///
/// Caveat: probing a missing key makes `i18n::t` leak that one
/// small string (its documented last-resort behaviour). Call
/// this in tests / settings screens, not per frame.
pub fn key_in_table(key: &str) -> bool {
    i18n::t(Lang::En, key) != key
}

/// Keys the Rune shell's labels are written against: the app bar
/// (Save/Load/Undo/Redo/+Figure), the transport (Play/Pause,
/// first/last, prev/next), the timeline (frame ops + headings),
/// the figures sidebar, the status line (Frame / fps / Zoom),
/// panels and workspaces it links to, and Settings.
///
/// Every entry names a label the shell actually shows or is
/// about to show (see `src/chrome_view.rs`, which today hardcodes
/// the English text for the first group). Keys that already
/// exist in the table resolve today; the rest are listed by
/// [`missing_shell_keys`] for the translation pass — they are
/// deliberately NOT added to `i18n.rs` from here.
pub const SHELL_KEYS: &[&str] = &[
    // App bar
    "save",
    "load",
    "undo",
    "redo",
    "add_figure",
    // Transport
    "play",
    "pause",
    "transport_first",
    "transport_last",
    "prev_frame",
    "next_frame",
    // Timeline / frame ops
    "add_frame",
    "duplicate_frame",
    "delete_frame",
    "frame",
    "frames",
    // Figures sidebar / building
    "figures",
    "figures_layers",
    "add_segment",
    "delete_segment",
    "onion_skin",
    // Status line
    "fps",
    "zoom",
    // Panels / workspaces the shell links to
    "help",
    "inspector",
    "export_png",
    "export_mp4",
    "ws_animate",
    "ws_export",
    // Settings (language picker lives here)
    "settings",
];

/// Split [`SHELL_KEYS`] into `(present, missing)` against the
/// live table. `present` keys translate today; `missing` keys
/// are the translation pass's work list.
pub fn shell_key_coverage() -> (Vec<&'static str>, Vec<&'static str>) {
    let mut present = Vec::new();
    let mut missing = Vec::new();
    for key in SHELL_KEYS {
        if key_in_table(key) {
            present.push(*key);
        } else {
            missing.push(*key);
        }
    }
    (present, missing)
}

/// The shell keys with no entry in any language table yet.
/// Reported, not invented: until the translation pass adds
/// them, [`tr`] falls back to the key text itself.
pub fn missing_shell_keys() -> Vec<&'static str> {
    shell_key_coverage().1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn core() -> EditorCore {
        EditorCore::new()
    }

    #[test]
    fn tr_uses_core_language_defaulting_to_english() {
        let core = core();
        assert_eq!(lang_of(&core), Lang::En);
        assert_eq!(tr(&core, "save"), "Save");
        assert_eq!(tr(&core, "play"), "Play");
        assert_eq!(tr(&core, "undo"), "Undo");
    }

    #[test]
    fn tr_follows_core_lang_when_changed() {
        let mut core = core();
        core.lang = Lang::Bn;
        assert_eq!(tr(&core, "save"), "সংরক্ষণ");
        core.lang = Lang::De;
        assert_eq!(tr(&core, "save"), "Speichern");
        assert_eq!(tr_lang(Lang::Fr, "load"), "Charger");
    }

    #[test]
    fn tr_falls_back_to_the_key_for_unknown_keys() {
        let core = core();
        // i18n::t's documented last resort — the shell shows the
        // key rather than a blank.
        assert_eq!(tr(&core, "settings"), "settings");
        assert_eq!(tr(&core, "no_such_key_anywhere"), "no_such_key_anywhere");
    }

    #[test]
    fn language_options_cover_all_28_from_the_table() {
        let opts = language_options();
        assert_eq!(opts.len(), 28);
        assert_eq!(opts.len(), Lang::all().len());
        assert_eq!(opts[0], ("en", "English"));
        assert!(opts.contains(&("bn", "বাংলা")));
        assert!(opts.contains(&("zh-Hans", "简体中文")));
        assert!(opts.contains(&("pt-BR", "Português (Brasil)")));
        // No duplicate codes.
        let mut codes: Vec<&str> = opts.iter().map(|(c, _)| *c).collect();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), 28);
    }

    #[test]
    fn coverage_helper_runs_and_partitions_shell_keys() {
        let (present, missing) = shell_key_coverage();
        // The helper runs against the real table and partitions
        // SHELL_KEYS exactly.
        assert_eq!(present.len() + missing.len(), SHELL_KEYS.len());
        for key in &present {
            assert!(!missing.contains(key), "{key} in both lists");
            assert!(key_in_table(key), "{key} listed present but not in table");
        }
        // Keys the shell already depends on must exist today.
        for key in ["save", "load", "undo", "redo", "play", "pause"] {
            assert!(present.contains(&key), "{key} must be in the table");
        }
        // The known gaps are reported as missing, not invented.
        for key in ["settings", "frames", "figures", "prev_frame", "next_frame"] {
            assert!(missing.contains(&key), "{key} should be reported missing");
        }
        assert_eq!(missing_shell_keys(), missing);
    }

    #[test]
    fn key_in_table_matches_known_keys() {
        assert!(key_in_table("save"));
        assert!(key_in_table("ws_animate"));
        assert!(!key_in_table("settings"));
        assert!(!key_in_table("definitely_not_a_key"));
    }
}
