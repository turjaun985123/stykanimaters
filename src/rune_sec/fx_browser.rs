//! Effects browser for the Rune editor shell (Phase D,
//! section 13 — GPUI→Rune migration). Declared from `main.rs`
//! only (as `rune_sec::fx_browser`) — bin-crate code, not
//! part of the engine library.
//!
//! Two groups, both wired to real `EditorCore` ops:
//!
//! * **Kinds** — the engine's 27 `vfx::ClipKind`s
//!   (`src/vfx.rs:22`), grouped under the five category
//!   headings the Wave 15 GPUI tool group used (Explosion /
//!   Energy / Magic / Nature / Particles — presentation
//!   only; the engine has no category concept. Membership
//!   verified line-by-line against the categorised rows in
//!   `git show 1ea3633:src/app.rs`, the Wave 15 commit).
//!   Each kind is one button calling
//!   `EditorCore::add_fx_clip` (`editor.rs:2072`): the clip
//!   starts at the current frame, anchored to the SELECTED
//!   node, in the selected figure's colour, 16 frames long.
//!   Honest consequence: with no joint selected the op adds
//!   nothing and only sets the status line ("Select a joint
//!   first, then add the FX clip.") — the buttons are live
//!   regardless and the status line is the feedback, exactly
//!   as in the GPUI app, whose buttons were always rendered.
//! * **Presets** — the `.json` presets in `./fx-presets/`,
//!   listed by `EditorCore::fx_preset_names`
//!   (`editor.rs:2153`, delegating to
//!   `fx_edit::list_clip_presets`, `fx_edit.rs:288`). One
//!   apply button per name calls
//!   `EditorCore::fx_apply_preset` (`editor.rs:2133`): the
//!   preset's clip is added on the selected figure (node 0)
//!   at the current frame. The name list goes through
//!   [`preset_names_cached`]: a thread-local cache keyed on
//!   the presets directory's mtime — the Wave 15 Home-scan
//!   design (see `home_panel`) — so a panel build costs one
//!   `stat` instead of a full directory read. Staleness
//!   contract: the cache can only go stale on NAMES, never
//!   on contents (apply loads the preset file live), and a
//!   save through this very panel invalidates it —
//!   `save_clip_preset_in` (`fx_edit.rs`) creates the preset
//!   file inside the directory, and adding a directory entry
//!   bumps the directory mtime, so the next build stats a
//!   new key and re-reads. The only theoretical miss is an
//!   external writer adding a file within the same mtime
//!   tick on a coarse-grained filesystem; on Linux (ns
//!   mtimes) a save is visible on the very next build,
//!   exactly as it was before the cache. When the directory
//!   cannot be statted at all there is no trustworthy key,
//!   so the cache is bypassed and the list is read live.
//!
//! The Save button calls `EditorCore::fx_save_preset`
//! (`editor.rs:2110`) with the LAST clip's index — the op
//! names the preset `<kind>-<index+1>` itself. Saving one
//! specific earlier clip stays with the clips section,
//! whose rows carry their own per-clip save buttons (the
//! GPUI arrangement). The button renders only when the
//! project has at least one clip: with none, the op could
//! only report an error, and the GPUI save buttons likewise
//! existed only on clip rows.
//!
//! Element keys reuse the GPUI control ids: the kind
//! buttons keep their `fxclip-*` ids verbatim, so headless
//! `--tree` checks written against the old UI still find
//! them.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use rune::tok::{type_, Theme};
use rune::{col, row, text, El};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::vfx::ClipKind;

use crate::chrome_view::btn;
use crate::State;

/// Kind buttons per row inside a category (the Particles
/// group has nine kinds; rows of three keep the panel
/// narrow).
pub const PER_ROW: usize = 3;

/// One browser entry: a clip kind, the category it is
/// presented under, its engine snake-case name (must equal
/// `fx_edit::kind_name` — pinned by a test) and its element
/// key (the GPUI button id).
pub struct FxKindEntry {
    pub category: &'static str,
    pub kind: ClipKind,
    pub name: &'static str,
    pub key: &'static str,
}

/// The five presentation categories, in the Wave 15 order.
pub const FX_CATEGORIES: [&str; 5] = ["Explosion", "Energy", "Magic", "Nature", "Particles"];

/// All 27 clip kinds, grouped by category in the Wave 15
/// order (within each category, the old row order).
pub const FX_KINDS: [FxKindEntry; 27] = [
    // Explosion
    FxKindEntry { category: "Explosion", kind: ClipKind::Burst, name: "burst", key: "fxclip-burst" },
    FxKindEntry { category: "Explosion", kind: ClipKind::ShockRing, name: "shock_ring", key: "fxclip-shockring" },
    FxKindEntry { category: "Explosion", kind: ClipKind::Shard, name: "shard", key: "fxclip-shard" },
    FxKindEntry { category: "Explosion", kind: ClipKind::Splash, name: "splash", key: "fxclip-splash" },
    // Energy
    FxKindEntry { category: "Energy", kind: ClipKind::Beam, name: "beam", key: "fxclip-beam" },
    FxKindEntry { category: "Energy", kind: ClipKind::Aura, name: "aura", key: "fxclip-aura" },
    FxKindEntry { category: "Energy", kind: ClipKind::Lightning, name: "lightning", key: "fxclip-lightning" },
    FxKindEntry { category: "Energy", kind: ClipKind::Prism, name: "prism", key: "fxclip-prism" },
    // Magic
    FxKindEntry { category: "Magic", kind: ClipKind::Sparkle, name: "sparkle", key: "fxclip-sparkle" },
    FxKindEntry { category: "Magic", kind: ClipKind::Orbit, name: "orbit", key: "fxclip-orbit" },
    FxKindEntry { category: "Magic", kind: ClipKind::Galaxy, name: "galaxy", key: "fxclip-galaxy" },
    FxKindEntry { category: "Magic", kind: ClipKind::Vortex, name: "vortex", key: "fxclip-vortex" },
    FxKindEntry { category: "Magic", kind: ClipKind::Fireworks, name: "fireworks", key: "fxclip-fireworks" },
    FxKindEntry { category: "Magic", kind: ClipKind::Spiral, name: "spiral", key: "fxclip-spiral" },
    // Nature
    FxKindEntry { category: "Nature", kind: ClipKind::Leaves, name: "leaves", key: "fxclip-leaves" },
    FxKindEntry { category: "Nature", kind: ClipKind::Petals, name: "petals", key: "fxclip-petals" },
    FxKindEntry { category: "Nature", kind: ClipKind::Bubbles, name: "bubbles", key: "fxclip-bubbles" },
    FxKindEntry { category: "Nature", kind: ClipKind::Aurora, name: "aurora", key: "fxclip-aurora" },
    // Particles
    FxKindEntry { category: "Particles", kind: ClipKind::Smoke, name: "smoke", key: "fxclip-smoke" },
    FxKindEntry { category: "Particles", kind: ClipKind::Ember, name: "ember", key: "fxclip-ember" },
    FxKindEntry { category: "Particles", kind: ClipKind::Fountain, name: "fountain", key: "fxclip-fountain" },
    FxKindEntry { category: "Particles", kind: ClipKind::Trail, name: "trail", key: "fxclip-trail" },
    FxKindEntry { category: "Particles", kind: ClipKind::Confetti, name: "confetti", key: "fxclip-confetti" },
    FxKindEntry { category: "Particles", kind: ClipKind::Meteor, name: "meteor", key: "fxclip-meteor" },
    FxKindEntry { category: "Particles", kind: ClipKind::Comet, name: "comet", key: "fxclip-comet" },
    FxKindEntry { category: "Particles", kind: ClipKind::Glitter, name: "glitter", key: "fxclip-glitter" },
    FxKindEntry { category: "Particles", kind: ClipKind::Wave, name: "wave", key: "fxclip-wave" },
];

/// The entries presented under `category`, in table order.
pub fn kinds_in(category: &str) -> Vec<&'static FxKindEntry> {
    FX_KINDS.iter().filter(|e| e.category == category).collect()
}

/// Button label for a kind: its snake-case engine name,
/// humanised ("shock_ring" → "Shock ring").
pub fn kind_label(name: &str) -> String {
    let spaced = name.replace('_', " ");
    let mut chars = spaced.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => spaced,
    }
}

/// A muted section heading inside the panel.
fn section(label: &str, t: &Theme) -> El {
    text(label.to_string())
        .font(type_::LABEL)
        .color(t.text_secondary)
}

// ----------------------------------------------------------------
// Preset-name cache (Wave 15 Home-scan design, mirrored from
// `home_panel`): the name list is keyed on the presets
// directory's mtime, so building this panel costs one `stat`
// per build instead of a `read_dir` over `./fx-presets/`.
// ----------------------------------------------------------------

/// The presets directory, relative to the working directory.
/// This is the same literal the engine reads and writes:
/// `fx_edit::PRESET_DIR` (`src/fx_edit.rs:177`) — private to
/// the engine crate, so this bin-side mirror is the one
/// maintenance hazard (if the engine ever moves the
/// directory, this constant must move with it).
pub const PRESET_DIR: &str = "fx-presets";

/// The presets directory as a path (`./fx-presets`).
pub fn preset_dir() -> PathBuf {
    PathBuf::from(PRESET_DIR)
}

/// A directory's modification time, swallowing every error
/// into `None` — `home_panel`'s helper, same rule: a
/// missing/unreadable dir is a state, not a failure
/// (`fx_edit::list_clip_presets` yields an empty list for
/// it). Callers treat `None` as "no trustworthy cache key",
/// not as a key.
fn dir_mtime(dir: &Path) -> Option<SystemTime> {
    std::fs::metadata(dir).and_then(|m| m.modified()).ok()
}

/// One preset-cache slot: the dir-mtime key the names were
/// read under, plus the names themselves.
type PresetCache = Option<(Option<SystemTime>, Vec<String>)>;

thread_local! {
    /// The preset-name cache. Thread-local because Rune
    /// builds the view and runs its handlers on the UI
    /// thread (so the panel's own Save press and the next
    /// build share one cache), and because every test thread
    /// then starts with an empty cache.
    static PRESET_CACHE: RefCell<PresetCache> = RefCell::new(None);
}

/// The cache state machine, pure over its inputs so tests
/// can drive it with a temp dir and a counting reader:
/// serve the stored names when `key` matches the key they
/// were read under, otherwise call `read`, store its result
/// under `key`, and return it. Keys are opaque here — the
/// "unstattable directory bypasses the cache" policy lives
/// in [`preset_names_cached`], not in this function.
fn cached_names(
    cache: &mut PresetCache,
    key: Option<SystemTime>,
    read: impl FnOnce() -> Vec<String>,
) -> Vec<String> {
    if let Some((cached_key, names)) = cache.as_ref() {
        if *cached_key == key {
            return names.clone();
        }
    }
    let names = read();
    *cache = Some((key, names.clone()));
    names
}

/// The preset names for the browser, cached behind the
/// presets directory's mtime (see the module docs for the
/// full staleness contract). Falls back to a live read —
/// caching nothing — when the directory cannot be statted.
pub fn preset_names_cached(core: &EditorCore) -> Vec<String> {
    let Some(key) = dir_mtime(&preset_dir()) else {
        // Usually this just means `./fx-presets/` does not
        // exist yet (nothing saved): the live read returns
        // the same empty list the cache would hold. Once a
        // save creates the directory, stat succeeds and the
        // cache takes over.
        return core.fx_preset_names();
    };
    PRESET_CACHE.with(|slot| {
        let mut slot = slot.borrow_mut();
        cached_names(&mut slot, Some(key), || core.fx_preset_names())
    })
}

/// The effects browser body. Mounting (and any show/hide
/// state) is the caller's business; this builds the full
/// panel for the current core state.
pub fn fx_browser(s: &State, t: &Theme) -> El {
    let core = &s.core;

    let mut panel = col()
        .key("fx-browser")
        .gap(8.0)
        .padding(10.0)
        .fill(t.card)
        .radius(8.0)
        .child(text("Effects").font(type_::TITLE).color(t.text));

    // ---- Kinds, grouped by category.
    for cat in FX_CATEGORIES {
        panel = panel.child(section(cat, t));
        for chunk in kinds_in(cat).chunks(PER_ROW) {
            let mut kinds_row = row().gap(6.0);
            for entry in chunk {
                let kind = entry.kind;
                kinds_row = kinds_row.child(btn(
                    entry.key,
                    &kind_label(entry.name),
                    false,
                    t,
                    move |s| s.core.add_fx_clip(kind),
                ));
            }
            panel = panel.child(kinds_row);
        }
    }

    // ---- Presets from ./fx-presets/.
    panel = panel.child(section("Presets", t));
    let preset_names = preset_names_cached(core);
    if preset_names.is_empty() {
        panel = panel.child(
            text("No presets saved yet")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    } else {
        for chunk in preset_names.chunks(PER_ROW) {
            let mut preset_row = row().gap(6.0);
            for name in chunk {
                let apply_name = name.clone();
                preset_row = preset_row.child(btn(
                    &format!("fx-preset-apply-{name}"),
                    name,
                    false,
                    t,
                    move |s| s.core.fx_apply_preset(&apply_name),
                ));
            }
            panel = panel.child(preset_row);
        }
    }
    if !core.project.fx_clips.is_empty() {
        panel = panel.child(btn(
            "fx-preset-save",
            "Save last clip as preset",
            false,
            t,
            |s| {
                if let Some(last) = s.core.project.fx_clips.len().checked_sub(1) {
                    s.core.fx_save_preset(last);
                }
            },
        ));
    }

    panel
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every `ClipKind` variant, listed explicitly — the
    /// table must cover exactly this set.
    const ALL_KINDS: [ClipKind; 27] = [
        ClipKind::Burst,
        ClipKind::Sparkle,
        ClipKind::Smoke,
        ClipKind::Beam,
        ClipKind::Aura,
        ClipKind::Trail,
        ClipKind::Confetti,
        ClipKind::ShockRing,
        ClipKind::Ember,
        ClipKind::Fountain,
        ClipKind::Orbit,
        ClipKind::Lightning,
        ClipKind::Bubbles,
        ClipKind::Leaves,
        ClipKind::Meteor,
        ClipKind::Petals,
        ClipKind::Splash,
        ClipKind::Spiral,
        ClipKind::Fireworks,
        ClipKind::Aurora,
        ClipKind::Comet,
        ClipKind::Galaxy,
        ClipKind::Prism,
        ClipKind::Vortex,
        ClipKind::Glitter,
        ClipKind::Wave,
        ClipKind::Shard,
    ];

    #[test]
    fn table_lists_all_27_kinds_exactly_once() {
        assert_eq!(FX_KINDS.len(), 27);
        for kind in ALL_KINDS {
            let count = FX_KINDS.iter().filter(|e| e.kind == kind).count();
            assert_eq!(count, 1, "{kind:?} appears {count} times in the table");
        }
    }

    #[test]
    fn every_entry_name_is_the_engine_kind_name() {
        // `fx_edit::kind_name` is the engine's canonical
        // kind→name mapping (the MCP vocabulary, and the
        // names presets are saved under). Note the engine's
        // name→kind parser (`mcp_tools::parse_clip_kind`)
        // is private to that module, so the public name fn
        // is what a table can be pinned against.
        for entry in FX_KINDS {
            assert_eq!(
                stykanimate_rs::fx_edit::kind_name(entry.kind),
                entry.name,
                "{:?}: table name disagrees with the engine",
                entry.kind
            );
        }
    }

    #[test]
    fn categories_are_the_five_wave15_groups_with_their_sizes() {
        let mut seen: Vec<&str> = Vec::new();
        for entry in FX_KINDS {
            if !seen.contains(&entry.category) {
                seen.push(entry.category);
            }
        }
        assert_eq!(seen, FX_CATEGORIES);
        assert_eq!(kinds_in("Explosion").len(), 4);
        assert_eq!(kinds_in("Energy").len(), 4);
        assert_eq!(kinds_in("Magic").len(), 6);
        assert_eq!(kinds_in("Nature").len(), 4);
        assert_eq!(kinds_in("Particles").len(), 9);
        assert!(kinds_in("No such category").is_empty());
    }

    #[test]
    fn keys_are_unique_gpui_fxclip_ids() {
        for (i, entry) in FX_KINDS.iter().enumerate() {
            assert!(
                entry.key.starts_with("fxclip-"),
                "{} is not an fxclip-* id",
                entry.key
            );
            assert!(
                FX_KINDS[..i].iter().all(|e| e.key != entry.key),
                "duplicate key {}",
                entry.key
            );
        }
        // The one irregular GPUI id: ShockRing's key drops
        // the underscore its engine name carries.
        let shock = FX_KINDS
            .iter()
            .find(|e| e.kind == ClipKind::ShockRing)
            .expect("ShockRing is in the table");
        assert_eq!(shock.name, "shock_ring");
        assert_eq!(shock.key, "fxclip-shockring");
    }

    #[test]
    fn kind_label_humanises_snake_names() {
        assert_eq!(kind_label("burst"), "Burst");
        assert_eq!(kind_label("shock_ring"), "Shock ring");
        assert_eq!(kind_label("fireworks"), "Fireworks");
        assert_eq!(kind_label(""), "");
    }

    // ---- Preset-name cache ----

    /// A unique temp directory per test so parallel test
    /// runs cannot collide (the home panel helper's rule).
    fn temp_dir(test: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "stykanimate-fx-browser-{test}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Two distinct mtime keys for the pure cache tests.
    fn key(secs: u64) -> Option<SystemTime> {
        Some(SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(secs))
    }

    #[test]
    fn cache_hit_returns_the_identical_list_without_rereading() {
        let mut cache: PresetCache = None;
        let reads = std::cell::Cell::new(0usize);
        let first = cached_names(&mut cache, key(10), || {
            reads.set(reads.get() + 1);
            vec!["alpha".to_string(), "beta".to_string()]
        });
        let second = cached_names(&mut cache, key(10), || {
            reads.set(reads.get() + 1);
            vec!["alpha".to_string(), "beta".to_string()]
        });
        assert_eq!(first, second);
        assert_eq!(first, vec!["alpha".to_string(), "beta".to_string()]);
        assert_eq!(reads.get(), 1, "second call must be served from the cache");
    }

    #[test]
    fn cache_rereads_when_the_mtime_key_changes() {
        let mut cache: PresetCache = None;
        let reads = std::cell::Cell::new(0usize);
        let first = cached_names(&mut cache, key(10), || {
            reads.set(reads.get() + 1);
            vec!["alpha".to_string()]
        });
        let second = cached_names(&mut cache, key(11), || {
            reads.set(reads.get() + 1);
            vec!["alpha".to_string(), "beta".to_string()]
        });
        assert_eq!(first, vec!["alpha".to_string()]);
        assert_eq!(second, vec!["alpha".to_string(), "beta".to_string()]);
        assert_eq!(reads.get(), 2, "a new key must invalidate the cache");
    }

    #[test]
    fn dir_mtime_is_none_for_a_missing_dir_and_some_for_a_real_one() {
        let dir = temp_dir("mtime");
        assert!(dir_mtime(&dir).is_some());
        let missing = dir.join("no-such-subdir");
        assert_eq!(dir_mtime(&missing), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn writing_a_preset_file_invalidates_through_real_mtimes() {
        // End to end over a real temp dir. The reader mirrors
        // `fx_edit::list_clip_presets_in`'s rule (sorted
        // `*.json` stems) because that helper is private to
        // the engine crate and the real `./fx-presets/` path
        // is a fixed constant — tests cannot redirect the
        // core op at a temp dir. What is exercised for real
        // here is the load-bearing assumption of the whole
        // design: creating a file inside a directory changes
        // the directory's mtime, so the panel's own Save
        // (which writes a new file via
        // `fx_edit::save_clip_preset_in`) invalidates the
        // cache and the new preset appears next build.
        let dir = temp_dir("invalidate");
        let list = |d: &Path| {
            let mut names: Vec<String> = std::fs::read_dir(d)
                .unwrap()
                .flatten()
                .filter_map(|e| e.file_name().into_string().ok())
                .filter_map(|n| n.strip_suffix(".json").map(str::to_string))
                .collect();
            names.sort();
            names
        };
        let mut cache: PresetCache = None;

        let k1 = dir_mtime(&dir);
        assert!(k1.is_some());
        assert!(cached_names(&mut cache, k1, || list(&dir)).is_empty());
        // Same key again: served from the cache even though
        // the reader would now still return the same list —
        // proven by the counting tests above; here the point
        // is what happens after a write.
        std::fs::write(dir.join("alpha.json"), "{}").unwrap();

        let k2 = dir_mtime(&dir);
        assert_ne!(
            k1, k2,
            "creating a file must change the directory mtime"
        );
        assert_eq!(cached_names(&mut cache, k2, || list(&dir)), vec!["alpha".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
