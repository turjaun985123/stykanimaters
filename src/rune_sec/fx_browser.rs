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
//!   `EditorCore::add_fx_clip` (`editor.rs:2005`): the clip
//!   starts at the current frame, anchored to the SELECTED
//!   node, in the selected figure's colour, 16 frames long.
//!   Honest consequence: with no joint selected the op adds
//!   nothing and only sets the status line ("Select a joint
//!   first, then add the FX clip.") — the buttons are live
//!   regardless and the status line is the feedback, exactly
//!   as in the GPUI app, whose buttons were always rendered.
//! * **Presets** — the `.json` presets in `./fx-presets/`,
//!   listed by `EditorCore::fx_preset_names`
//!   (`editor.rs:2086`, delegating to
//!   `fx_edit::list_clip_presets`, `fx_edit.rs:288`). One
//!   apply button per name calls
//!   `EditorCore::fx_apply_preset` (`editor.rs:2066`): the
//!   preset's clip is added on the selected figure (node 0)
//!   at the current frame. The names are read when the panel
//!   is built; the GPUI view cached the same listing behind
//!   the directory mtime, but Rune re-runs the view only on
//!   state changes, so a preset saved by the button below is
//!   listed on the very next build without a cache.
//!
//! The Save button calls `EditorCore::fx_save_preset`
//! (`editor.rs:2043`) with the LAST clip's index — the op
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

use rune::tok::{type_, Theme};
use rune::{col, row, text, El};
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
    let preset_names = core.fx_preset_names();
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
}
