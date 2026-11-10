//! Rune-shell localisation wiring (Phase D, section 38 — i18n).
//!
//! The Rune shell does NOT adopt `rune-i18n` (audit
//! `docs/RUNE_API_AUDIT.md` Q12: it would replace a solved problem
//! with a build.rs + ICU4X stack). It reuses the project's own
//! table in `stykanimate_rs::i18n` — 28 languages × 693 keys
//! (78 through Phase E; +18 mission-era label keys in Wave 9
//! and +3 dopesheet keys in Wave 10; Wave R added the
//! subtitle block, the camera block and `subtitle_speaker`;
//! Wave S added the export-panel block (16 `export_*` keys)
//! and the 3D-panel block (34 `threed_*` keys); Wave T added
//! the export class-C block (35 `export_*` keys) and the
//! audio (33 `audio_*`), frame-operations (24 `frame_*`),
//! segment-inspector (22 `segment_*`), FX-clips (28 `fx_*`)
//! and particles (16 `particles_*`) panel blocks, carried as
//! English placeholders in the untranslated tables per the
//! table's own convention; Wave U added the stroke-inspector
//! (53 `stroke_*`), clips (37 `clips_*`), menus (23 `menu_*`),
//! FX-kind (27 `fx_kind_*`), audio-readout (21 `audio_*`),
//! frame-readout (6 `frame_*`), particles/segment-readout
//! (21 `particles_*` / `segment_*`), FX-readout (9 `fx_*`)
//! and 3D-readout (6 `threed_*`) blocks on the same
//! convention; Wave X added the joint-limit (4
//! `segment_limit_*`) and figure-states (8 `clips_states_*`)
//! labels on the same convention), with
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
//!   in the table. Since Phase E added the last 8 keys
//!   (`settings`, `frame`, `frames`, `figures`, `prev_frame`,
//!   `next_frame`, `fps`, `zoom`) to `i18n.rs` with genuine
//!   translations in all 28 languages, coverage is complete:
//!   [`missing_shell_keys`] returns an empty list, and the helper
//!   remains as the guard that would report any future gap
//!   instead of letting it render silently.

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
/// Every entry names a label the shell actually shows (see
/// `src/chrome_view.rs`, whose app-bar and transport labels
/// resolve through [`tr`] since Phase E). All 24 keys exist in
/// the table — the last 8 arrived with the Phase E translation
/// pass in `i18n.rs`, and `dopesheet` (registered with the
/// Wave 10 panel rebuild) was already in every table — so
/// [`missing_shell_keys`] is empty; the coverage split stays
/// as the regression guard. (Wave V, IA-U10-1: the dead
/// `transport_first` / `transport_last` / `figures_layers` /
/// `add_segment` / `delete_segment` / `inspector` entries left
/// with their catalogue keys, and `onion_skin` left too — its
/// catalogue key was removed by the same IA, so keeping it
/// here would report it missing.)
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
    // Status line
    "fps",
    "zoom",
    // Panels / workspaces the shell links to
    "help",
    "dopesheet",
    "export_png",
    "export_mp4",
    "ws_animate",
    "ws_export",
    // Settings (language picker lives here)
    "settings",
];

/// Split [`SHELL_KEYS`] into `(present, missing)` against the
/// live table. Since Phase E, every shell key is present and
/// `missing` is empty; the split is kept so a future key added
/// to [`SHELL_KEYS`] ahead of its translations is reported
/// here instead of failing silently in the UI.
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

/// The shell keys with no entry in any language table.
/// Empty since the Phase E translation pass completed
/// coverage; retained as the standing report channel — if a
/// future shell key lands before its translations, it shows
/// up here (and [`tr`] falls back to the key text meanwhile).
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
        // key rather than a blank. ("settings" itself resolved
        // to the key until Phase E added it to the table; it is
        // asserted as a real translation below.)
        assert_eq!(tr(&core, "no_such_key_anywhere"), "no_such_key_anywhere");
    }

    #[test]
    fn phase_e_shell_keys_translate() {
        let mut core = core();
        // The 8 keys Phase E added resolve to real text, in
        // English and in the core's current language.
        assert_eq!(tr(&core, "settings"), "Settings");
        assert_eq!(tr(&core, "prev_frame"), "Previous frame");
        assert_eq!(tr(&core, "next_frame"), "Next frame");
        assert_eq!(tr(&core, "frame"), "Frame");
        assert_eq!(tr(&core, "frames"), "Frames");
        assert_eq!(tr(&core, "figures"), "Figures");
        assert_eq!(tr(&core, "fps"), "FPS");
        assert_eq!(tr(&core, "zoom"), "Zoom");
        core.lang = Lang::Bn;
        assert_eq!(tr(&core, "settings"), "সেটিংস");
        assert_eq!(tr(&core, "prev_frame"), "আগের ফ্রেম");
        core.lang = Lang::De;
        assert_eq!(tr(&core, "settings"), "Einstellungen");
        assert_eq!(tr(&core, "zoom"), "Zoom");
        core.lang = Lang::ZhHans;
        assert_eq!(tr(&core, "next_frame"), "下一帧");
        assert_eq!(tr(&core, "fps"), "帧率");
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
        // Phase E completed coverage: the keys that were the
        // known gaps are present now, and nothing is missing.
        for key in [
            "settings",
            "frame",
            "frames",
            "figures",
            "prev_frame",
            "next_frame",
            "fps",
            "zoom",
        ] {
            assert!(present.contains(&key), "{key} should be in the table now");
        }
        assert!(missing.is_empty(), "missing should be empty, got {missing:?}");
        assert_eq!(missing_shell_keys(), missing);
    }

    #[test]
    fn dopesheet_keys_resolve_through_tr() {
        let mut core = core();
        // The dopesheet panel's title key. `dopesheet` predates
        // the rebuild and is genuinely translated in all 28
        // languages. (The Wave 10 navigation and empty-state
        // keys were removed with IA-U10-1 in Wave V.)
        assert_eq!(tr(&core, "dopesheet"), "Dopesheet");
        core.lang = Lang::Es;
        assert_eq!(tr(&core, "dopesheet"), "Hoja de tiempos");
        core.lang = Lang::Bn;
        assert_eq!(tr(&core, "dopesheet"), "ডোপশিট");
        assert!(key_in_table("dopesheet"));
    }

    #[test]
    fn key_in_table_matches_known_keys() {
        assert!(key_in_table("save"));
        assert!(key_in_table("ws_animate"));
        // All 8 Phase E keys are in the table now.
        for key in [
            "settings", "frame", "frames", "figures", "prev_frame", "next_frame", "fps", "zoom",
        ] {
            assert!(key_in_table(key), "{key} should be in the table");
        }
        assert!(!key_in_table("definitely_not_a_key"));
    }
}
