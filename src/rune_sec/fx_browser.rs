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
//! Phase F adds two view-state conveniences, both held in
//! [`FxBrowserState`] (declared in this file; the shell
//! integrator owns the `State` field):
//!
//! * **Filter** — a text field at the panel top. Its text
//!   substring-filters the kind buttons across all five
//!   categories through [`visible_categories`] (case-
//!   insensitive, matching the snake-case name and the
//!   humanised label); a category with no match left is
//!   hidden entirely, and a filter matching nothing at all
//!   shows a muted "No effects match the filter" line
//!   instead of a silently empty grid. Presets are not
//!   filtered. Phase J: the filter also matches the
//!   category names themselves ([`category_matches`]) —
//!   a query naming a category ("energy", "nat") shows
//!   that category's whole kind list, unioned with the
//!   ordinary kind-name matches.
//! * **Recent** — a row under the filter showing the last
//!   [`MAX_RECENT`] kinds added from this panel, most
//!   recent first, no duplicates ([`record_recent`] is the
//!   bookkeeping). A kind is recorded when its add button
//!   fires — before the op runs, so a press that the op
//!   refuses (no joint selected) still counts as "the user
//!   picked this kind". Recent buttons re-add their kind
//!   (and move it back to the front). The list is
//!   in-memory only: it lives in the shell state, is never
//!   written to disk, and starts empty every launch. It is
//!   deliberately NOT filtered — it is a shortcut row, not
//!   part of the grid.
//!
//! Phase H adds Font Awesome glyphs (via
//! `rune_sec::icons::icon_el`, the frame_ops/menus
//! inline-glyph idiom: 12 pt, secondary colour, leading
//! its row). Each category heading carries its glyph —
//! see [`category_icon`] — the Recent heading a `clock`,
//! and the filter row a leading `magnifying-glass`. The
//! kind buttons themselves keep their text labels: the
//! kind names are the content. No keys change — the
//! glyphs are unkeyed children of existing rows.
//!
//! Phase K adds per-kind usage counts to the kind
//! buttons: a button whose kind the project already
//! holds reads "{label} · {n}" ([`kind_button_label`]
//! over [`kind_usage`]), so the user can see which
//! kinds are already in use, and how often, without
//! leaving the browser. At zero usage the label is
//! byte-identical to before. The counts are display
//! only — the filter still matches the entry's base
//! name / label / category, never the counted string —
//! and the Recent row keeps the bare labels (it is a
//! shortcut row, not part of the grid).
//!
//! Phase N restyles the panel onto the charter's card
//! language: the root is the kit card
//! (`design::panel_col`, key `fx-browser` unchanged)
//! with a `design::panel_title` header — the
//! `wand-magic-sparkles` glyph P13 §3b assigns this
//! panel — over a hairline `design::divider`; section
//! headings render through `design::section_label`
//! (the local LABEL helper retired; the icon-led
//! variant keeps its 12 pt leading glyph). The kind
//! buttons now lead with their category's glyph via
//! `icon_btn` (P13 §1's optional polish, taken): keys,
//! labels — including the Phase K counted form — and
//! press behaviour are byte-identical, and only the
//! Phase H note above ("keep their text labels") is
//! superseded, in that the labels now follow a glyph.
//! The preset-save button gains `floppy-disk` (P13
//! Tier A). Recent and preset-apply buttons stay
//! plain text (P13's keep-text list rows); the two
//! muted empty lines stay as they are — no glyph is
//! assigned to either state in any plan.
//!
//! UI mission Wave 4 supersedes that last clause
//! (audit S13-3): Squad 13 assigned the glyphs, and
//! both lines are now kit [`design::empty_state`]s —
//! "No presets saved yet" under `floppy-disk`, "No
//! effects match the filter" under `magnifying-glass`
//! — with their text byte-identical, the particles
//! panel's treatment. The same wave adds the S13-13
//! overflow guard: kind and preset rows carry
//! `.flex_wrap()` at the card width (see [`PER_ROW`]),
//! so a row whose buttons outgrow the 316 pt budget
//! wraps instead of crossing the card edge. Keys,
//! labels and handlers are unchanged throughout.
//!
//! UI mission Wave 9 closes the guard's residual hole:
//! wrapping bounds a row's SUM, but it cannot shrink
//! ONE button whose label alone is wider than the
//! card — and preset names are arbitrary file stems
//! (`fx_edit::list_clip_presets_in` accepts any
//! `*.json` stem), so a long name still painted past
//! the edge. Preset apply buttons now display
//! [`preset_button_label`] — the name itself while it
//! fits, middle-truncated with "…" once it cannot —
//! while the element key and the apply handler keep
//! the full name, exactly as before.
//!
//! Integrator wiring for the state: add
//! `pub fx_browser: rune_sec::fx_browser::FxBrowserState`
//! to `State`, initialise it with
//! `fx_browser: Default::default(),`, and add
//! `s.fx_browser.query.focused` to the shell's
//! `text_focused` check so global shortcuts stay out of
//! the filter field while typing (the library search got
//! the same treatment in Phase E).
//!
//! Element keys reuse the GPUI control ids: the kind
//! buttons keep their `fxclip-*` ids verbatim, so headless
//! `--tree` checks written against the old UI still find
//! them. The Phase F additions are new keys: `fx-filter`
//! (the field), `fx-filter-row`, and `fx-recent-<name>`
//! for the Recent buttons (kept distinct from the grid's
//! `fxclip-*` ids so a kind never has two elements with
//! the same key on screen at once).

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use rune::tok::{type_, Theme};
use rune::{row, text_input, El, TextEdit};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::vfx::ClipKind;

use crate::chrome_view::btn;
use crate::rune_sec::design;
use crate::rune_sec::icon_btn::icon_btn;
use crate::rune_sec::icons::icon_el;
use crate::State;

/// Kind buttons per row inside a category (the Particles
/// group has nine kinds; rows of three keep the panel
/// narrow). Rows also carry `.flex_wrap()` at the card's
/// content width (UI mission Wave 4, audit S13-13): the
/// chunking alone cannot bound a row — counted labels
/// ("Shock ring · 12") widen every button, and preset
/// names are unbounded filenames — so a row that outgrows
/// the 316 pt budget wraps its last button onto a second
/// line instead of painting past the card edge.
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

/// How many clips of `kind` the project already holds:
/// a straight count over `project.fx_clips`, comparing
/// each entry's `clip.kind` (`FxClipEntry.clip` is the
/// engine's `vfx::ParticleClip`, whose `kind` field is
/// the clip's kind). A pure read — the builder calls it
/// once per visible kind button per build. Phase K.
pub fn kind_usage(core: &EditorCore, kind: ClipKind) -> usize {
    core.project
        .fx_clips
        .iter()
        .filter(|e| e.clip.kind == kind)
        .count()
}

/// The grid kind button's label (Phase K): the kind's
/// humanised label alone while the project holds none
/// of it — byte-identical to the pre-Phase-K label —
/// and "{label} · {n}" once it holds `n` clips of the
/// kind ("Sparkle · 3"), so the browser itself shows
/// which kinds are in use. Display only: filtering
/// ([`kind_matches`]) matches the entry's base name /
/// label / category and never this counted string.
pub fn kind_button_label(core: &EditorCore, entry: &FxKindEntry) -> String {
    let label = kind_label(entry.name);
    match kind_usage(core, entry.kind) {
        0 => label,
        n => format!("{label} · {n}"),
    }
}

/// A muted section heading inside the panel (Phase N:
/// the kit's [`design::section_label`] — SECTION in
/// `text_secondary`; the local LABEL rendering retired).
fn section(label: &str, t: &Theme) -> El {
    design::section_label(label, t)
}

/// The Font Awesome glyph shown before a category
/// heading (Phase H). Presentation only, like the
/// categories themselves — the engine has no category
/// concept, so this table lives and dies here. Every
/// name is in the curated icon set (`icon_data`); note
/// "bomb" is NOT in that set, so Explosion burns
/// ("fire") instead. An unknown category gets no glyph.
fn category_icon(category: &str) -> Option<&'static str> {
    Some(match category {
        "Explosion" => "fire",
        "Energy" => "bolt",
        "Magic" => "wand-magic-sparkles",
        "Nature" => "leaf",
        "Particles" => "shapes",
        _ => return None,
    })
}

/// A section heading with a leading glyph (Phase H):
/// the [`section`] label preceded by a 12 pt [`icon_el`]
/// in the heading's secondary colour, in a centred row
/// — the inline-glyph idiom frame_ops and menus use.
/// A `None` glyph falls back to the plain [`section`]
/// heading, so unrecognised headings stay honest text.
fn section_with_icon(icon: Option<&str>, label: &str, t: &Theme) -> El {
    let Some(icon) = icon else {
        return section(label, t);
    };
    row()
        .gap(6.0)
        .center_y()
        .child(icon_el(icon, 12.0, t.text_secondary))
        .child(design::section_label(label, t))
}

// ----------------------------------------------------------------
// Panel view-state (Phase F): the filter field and the
// recently-added kinds. Everything about clips themselves
// lives in `EditorCore`; this is shell state only.
// ----------------------------------------------------------------

/// How many kinds the Recent row remembers.
pub const MAX_RECENT: usize = 3;

/// View-state for this section: the filter field's contents
/// and the recently-added kinds. Owned by `State` (see the
/// module docs for the exact integrator wiring); nothing
/// here is persisted — the recents list in particular is
/// in-memory only and starts empty every launch.
///
/// The derive drops `Eq` because Rune's `TextEdit` is
/// `PartialEq` but not `Eq` (rune `edit.rs`:
/// `#[derive(Clone, Default, PartialEq)]`) — the
/// `library_panel` state hit the same fact in Phase E.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct FxBrowserState {
    /// Filter box contents (Rune core text edit). The kind
    /// grid filters through [`visible_categories`] on this
    /// text every render; typing is the only writer.
    pub query: TextEdit,
    /// The last kinds added from this panel, most recent
    /// first, no duplicates, capped at [`MAX_RECENT`].
    /// Written only by [`record_recent`], from the kind
    /// buttons' handlers.
    pub recent: Vec<ClipKind>,
}

/// True when the filter `query` names `category`:
/// case-insensitive substring against the category name
/// itself (so "energy" and "nat" both name their
/// category). An empty (or whitespace-only) query is
/// NOT a category match — matching everything is
/// [`kind_matches`]' empty-query rule, and conflating
/// the two would make every category "match" an empty
/// filter. Phase J: this is the rule that lets a filter
/// show a whole category (see [`kind_matches`]).
pub fn category_matches(category: &str, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    !q.is_empty() && category.to_lowercase().contains(&q)
}

/// True when `entry` matches the filter `query`:
/// case-insensitive substring against the kind's
/// snake-case name and its humanised label (so "ring"
/// finds Shock ring either way) — or against the name
/// of the category the kind belongs to, so a query
/// naming a category matches every kind in it (Phase
/// J; see [`category_matches`]). An empty (or
/// whitespace-only) query matches everything — the
/// `library_panel::name_matches` rule, mirrored.
pub fn kind_matches(entry: &FxKindEntry, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    q.is_empty()
        || entry.name.to_lowercase().contains(&q)
        || kind_label(entry.name).to_lowercase().contains(&q)
        || category_matches(entry.category, query)
}

/// The categories the grid should show for `query`, in
/// [`FX_CATEGORIES`] order, each with its matching kinds
/// in table order. A category whose kinds all fail the
/// filter is dropped entirely (its heading hides with
/// it). An empty query yields all five categories at
/// full size; a query matching nothing yields an empty
/// vec, which the builder turns into a "no match" line.
/// Because [`kind_matches`] also matches category
/// names, a category the query names appears at full
/// size; the union needs no deduplication step — every
/// kind sits in exactly one category and is visited
/// exactly once, so category order and table order
/// fully determine the result.
pub fn visible_categories(
    query: &str,
) -> Vec<(&'static str, Vec<&'static FxKindEntry>)> {
    FX_CATEGORIES
        .iter()
        .filter_map(|cat| {
            let kinds: Vec<&'static FxKindEntry> = kinds_in(cat)
                .into_iter()
                .filter(|e| kind_matches(e, query))
                .collect();
            if kinds.is_empty() {
                None
            } else {
                Some((*cat, kinds))
            }
        })
        .collect()
}

/// Record `kind` as the most recently added: it moves to
/// the front, any earlier occurrence is removed (no
/// duplicates), and the list is truncated to
/// [`MAX_RECENT`]. Pure bookkeeping over the state vec —
/// no core access — so the buttons' handlers stay thin.
pub fn record_recent(recent: &mut Vec<ClipKind>, kind: ClipKind) {
    recent.retain(|&k| k != kind);
    recent.insert(0, kind);
    recent.truncate(MAX_RECENT);
}

/// The table entries for the recents list, in list order
/// (most recent first), for the Recent row's buttons.
/// Kinds always resolve — recents are only ever recorded
/// from table buttons — but an unresolvable kind is
/// skipped rather than panicking.
pub fn recent_entries(recent: &[ClipKind]) -> Vec<&'static FxKindEntry> {
    recent
        .iter()
        .filter_map(|k| FX_KINDS.iter().find(|e| e.kind == *k))
        .collect()
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

/// Text-width budget for one preset apply button's label,
/// in points: the panel's content width (316) minus the
/// kit `btn`'s horizontal padding (10 + 10), with a small
/// safety margin. Wave 4's `.flex_wrap()` bounds a preset
/// row's SUM, but wrapping cannot shrink one button whose
/// name alone is wider than the card — and preset names
/// are arbitrary file stems, unbounded in length. The
/// display label is therefore bounded to this budget by
/// [`preset_button_label`]; the element key and the apply
/// handler always keep the full name (Wave 9).
pub const PRESET_LABEL_BUDGET: f32 = 288.0;

/// Conservative advance estimate for one character of a
/// preset name at the button's LABEL font (12 pt, weight
/// 500). Every class is an UPPER bound against measured
/// tree text ("Shock ring" measures 61.5 pt for ten
/// characters; this model charges 66), so a label the
/// model passes truly fits — the model may truncate a
/// borderline name early, never late. Non-ASCII
/// characters are charged the full em (12 pt), the CJK
/// advance at this size.
fn preset_char_width(c: char) -> f32 {
    match c {
        ' ' => 4.0,
        'i' | 'l' | 'j' | 't' | 'f' | '.' | ',' | ':' | ';' | '!' | '|' | '\'' | '`' => 4.0,
        '-' | '_' | '(' | ')' | '[' | ']' | '{' | '}' => 6.0,
        'm' | 'w' => 10.0,
        'M' | 'W' => 12.0,
        '0'..='9' => 7.0,
        'A'..='Z' => 9.0,
        'a'..='z' => 7.0,
        _ if c.is_ascii() => 7.0,
        _ => 12.0,
    }
}

/// The estimated painted width of `name` as a preset
/// button label, per [`preset_char_width`].
pub fn preset_label_width(name: &str) -> f32 {
    name.chars().map(preset_char_width).sum()
}

/// The apply button's DISPLAY label for preset `name`:
/// the name itself while it fits [`PRESET_LABEL_BUDGET`],
/// otherwise the name middle-truncated with "…" — head
/// and tail both kept, growing alternately so neither
/// side starves — until the estimate fits. Display only:
/// the caller keys the button by, and applies, the full
/// name, so a truncated label loses no function (the
/// shell's filename treatment; Rune's own text system
/// middle-truncates paths the same way). The truncating
/// branch is only reached when the whole name exceeds
/// the budget, which is more than the ellipsis leaves
/// room for, so head and tail can never meet.
pub fn preset_button_label(name: &str) -> String {
    if preset_label_width(name) <= PRESET_LABEL_BUDGET {
        return name.to_string();
    }
    // The ellipsis is charged a full em, like any wide
    // glyph in the model above.
    let room = PRESET_LABEL_BUDGET - 12.0;
    let chars: Vec<char> = name.chars().collect();
    let (mut head, mut tail, mut used) = (0usize, 0usize, 0.0f32);
    loop {
        let mut progressed = false;
        if head + tail < chars.len() {
            let w = preset_char_width(chars[head]);
            if used + w <= room {
                head += 1;
                used += w;
                progressed = true;
            }
        }
        if head + tail < chars.len() {
            let w = preset_char_width(chars[chars.len() - 1 - tail]);
            if used + w <= room {
                tail += 1;
                used += w;
                progressed = true;
            }
        }
        if !progressed {
            break;
        }
    }
    let mut out: String = chars[..head].iter().collect();
    out.push('…');
    out.extend(&chars[chars.len() - tail..]);
    out
}

/// The effects browser body. Mounting (and any show/hide
/// state) is the caller's business; this builds the full
/// panel for the current core state and the panel's own
/// [`FxBrowserState`] (filter text, recents).
pub fn fx_browser(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let query = s.fx_browser.query.text.clone();

    let mut panel = design::panel_col("fx-browser", t)
        .child(
            row()
                .w_full()
                .gap(design::PANEL_GAP)
                .center_y()
                .child(design::panel_title(
                    Some("wand-magic-sparkles"),
                    "Effects",
                    None,
                    t,
                )),
        )
        .child(design::divider(t));

    // ---- Filter field (Phase F): substring-filters the
    // kind grid below through `visible_categories`.
    panel = panel.child(
        row()
            .key("fx-filter-row")
            .gap(8.0)
            .center_y()
            .child(icon_el("magnifying-glass", 12.0, t.text_secondary))
            .child(
                text_input(&s.fx_browser.query, type_::BODY.lh(26.0))
                    .key("fx-filter")
                    .single_line()
                    .grow(1.0)
                    .color(t.text)
                    .placeholder("Filter effects", t.text_secondary)
                    .on_edit(|s: &mut State| &mut s.fx_browser.query),
            ),
    );

    // ---- Recent kinds (Phase F): the last kinds added
    // from this panel, most recent first. Shown whenever
    // the list is non-empty, independent of the filter.
    if !s.fx_browser.recent.is_empty() {
        panel = panel.child(section_with_icon(Some("clock"), "Recent", t));
        let mut recent_row = row().gap(6.0);
        for entry in recent_entries(&s.fx_browser.recent) {
            let kind = entry.kind;
            recent_row = recent_row.child(btn(
                &format!("fx-recent-{}", entry.name),
                &kind_label(entry.name),
                false,
                t,
                move |s| {
                    record_recent(&mut s.fx_browser.recent, kind);
                    s.core.add_fx_clip(kind);
                },
            ));
        }
        panel = panel.child(recent_row);
    }

    // ---- Kinds, grouped by category, filtered by the
    // query (empty categories hide with their headings).
    let visible = visible_categories(&query);
    if visible.is_empty() {
        // Kit empty state with the audit's glyph
        // (S13-3): the search that found nothing is a
        // magnifying glass. Text unchanged.
        panel = panel.child(design::empty_state(
            "magnifying-glass",
            "No effects match the filter",
            t,
        ));
    }
    for (cat, entries) in &visible {
        panel = panel.child(section_with_icon(category_icon(cat), cat, t));
        for chunk in entries.chunks(PER_ROW) {
            // Wrapped at the card width (S13-13): with
            // counted labels the three buttons can sum
            // past the 316 pt budget; the row wraps
            // instead of crossing the card edge.
            let mut kinds_row = row().gap(6.0).w_full().flex_wrap();
            for entry in chunk {
                let kind = entry.kind;
                // Phase K: the counted label — bare
                // at zero usage, "{label} · {n}" once
                // the project holds the kind. Key and
                // press behaviour are unchanged.
                let label = kind_button_label(core, entry);
                let press = move |s: &mut State| {
                    record_recent(&mut s.fx_browser.recent, kind);
                    s.core.add_fx_clip(kind);
                };
                // Phase N (P13 §1, adopter's call taken):
                // the button leads with its category's
                // glyph via `icon_btn`; the kind name —
                // the content — stays the label. A kind
                // whose category had no glyph would fall
                // back to the plain family `btn` (all
                // five categories resolve today).
                kinds_row = kinds_row.child(match category_icon(entry.category) {
                    Some(icon) => icon_btn(entry.key, icon, &label, false, t, press),
                    None => btn(entry.key, &label, false, t, press),
                });
            }
            panel = panel.child(kinds_row);
        }
    }

    // ---- Presets from ./fx-presets/.
    panel = panel.child(section("Presets", t));
    let preset_names = preset_names_cached(core);
    if preset_names.is_empty() {
        // Kit empty state with the audit's glyph
        // (S13-3): saved presets live on disk, so the
        // empty store is a floppy disk. Text unchanged.
        panel = panel.child(design::empty_state(
            "floppy-disk",
            "No presets saved yet",
            t,
        ));
    } else {
        for chunk in preset_names.chunks(PER_ROW) {
            // Wrapped at the card width (S13-13), like
            // the kind rows: preset names are filenames
            // and unbounded, so three apply buttons can
            // sum past the 316 pt budget. Wave 9: the
            // wrap bounds the row's sum only, so the
            // button's DISPLAY label is additionally
            // bounded by `preset_button_label` — the
            // key and the apply closure keep the full
            // name.
            let mut preset_row = row().gap(6.0).w_full().flex_wrap();
            for name in chunk {
                let apply_name = name.clone();
                preset_row = preset_row.child(btn(
                    &format!("fx-preset-apply-{name}"),
                    &preset_button_label(name),
                    false,
                    t,
                    move |s| s.core.fx_apply_preset(&apply_name),
                ));
            }
            panel = panel.child(preset_row);
        }
    }
    if !core.project.fx_clips.is_empty() {
        // Phase N (P13 Tier A): Save carries the
        // shell-wide save glyph, `floppy-disk`.
        panel = panel.child(icon_btn(
            "fx-preset-save",
            "floppy-disk",
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
    fn category_icons_cover_the_five_categories_and_resolve() {
        // The Phase H heading glyphs, pinned exactly.
        assert_eq!(category_icon("Explosion"), Some("fire"));
        assert_eq!(category_icon("Energy"), Some("bolt"));
        assert_eq!(category_icon("Magic"), Some("wand-magic-sparkles"));
        assert_eq!(category_icon("Nature"), Some("leaf"));
        assert_eq!(category_icon("Particles"), Some("shapes"));
        // Every presented category has one, and every
        // glyph named here resolves in the curated icon
        // set (a typo'd name would render as nothing).
        for cat in FX_CATEGORIES {
            let icon = category_icon(cat).expect("category has a glyph");
            assert!(
                crate::rune_sec::icon_data::icon_data(icon).is_some(),
                "{icon} is not in the curated icon set"
            );
        }
        // Unknown headings get no glyph (plain text).
        assert_eq!(category_icon("Presets"), None);
        assert_eq!(category_icon("No such category"), None);
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

    // ---- Usage counts on the kind buttons (Phase K) ----

    /// A core whose project holds one clip per kind in
    /// `kinds`, planted the way the sibling panels' tests
    /// plant clips (fx_clips' `entry` fixture): straight
    /// into `project.fx_clips` as full `FxClipEntry`s.
    fn core_with_kinds(kinds: &[ClipKind]) -> EditorCore {
        let mut core = EditorCore::new();
        for &kind in kinds {
            core.project.fx_clips.push(stykanimate_rs::model::FxClipEntry {
                clip: stykanimate_rs::vfx::ParticleClip {
                    start: 0.0,
                    duration: 16.0,
                    kind,
                    color: 0xFFFFFF,
                    color2: None,
                    seed: 1,
                    angle: 0.0,
                    length: 130.0,
                },
                figure: 0,
                node: 0,
                hidden: false,
            });
        }
        core
    }

    #[test]
    fn kind_usage_counts_the_projects_clips_of_each_kind() {
        let core = core_with_kinds(&[
            ClipKind::Sparkle,
            ClipKind::Burst,
            ClipKind::Sparkle,
            ClipKind::Sparkle,
        ]);
        assert_eq!(kind_usage(&core, ClipKind::Sparkle), 3);
        assert_eq!(kind_usage(&core, ClipKind::Burst), 1);
        // Kinds the project does not use count zero.
        assert_eq!(kind_usage(&core, ClipKind::Beam), 0);
        assert_eq!(kind_usage(&core, ClipKind::Wave), 0);
        // An empty project uses nothing.
        let empty = EditorCore::new();
        assert_eq!(kind_usage(&empty, ClipKind::Sparkle), 0);
    }

    #[test]
    fn kind_button_label_is_the_bare_label_at_zero_usage() {
        // With nothing planted, every button's label is
        // exactly the humanised label it had before
        // Phase K — byte-identical, no suffix.
        let core = EditorCore::new();
        for entry in FX_KINDS {
            assert_eq!(kind_button_label(&core, &entry), kind_label(entry.name));
        }
        assert_eq!(
            kind_button_label(&core, entry_of(ClipKind::Sparkle)),
            "Sparkle"
        );
    }

    #[test]
    fn kind_button_label_appends_the_count_once_the_kind_is_used() {
        let core = core_with_kinds(&[
            ClipKind::Sparkle,
            ClipKind::Sparkle,
            ClipKind::Sparkle,
            ClipKind::Burst,
        ]);
        assert_eq!(
            kind_button_label(&core, entry_of(ClipKind::Sparkle)),
            "Sparkle · 3"
        );
        assert_eq!(
            kind_button_label(&core, entry_of(ClipKind::Burst)),
            "Burst · 1"
        );
        // Unused kinds in the same project stay bare.
        assert_eq!(
            kind_button_label(&core, entry_of(ClipKind::Beam)),
            "Beam"
        );
    }

    // ---- Preset label budget (Wave 9) ----

    #[test]
    fn preset_label_width_charges_upper_bounds() {
        // Pinned model values: each class is an upper
        // bound against measured tree text ("Shock ring"
        // paints 61.5 pt wide in the headless dumps; the
        // model charges 66), so a passing label truly
        // fits and only a borderline name truncates
        // early.
        assert_eq!(preset_label_width("Shock ring"), 66.0);
        assert_eq!(preset_label_width("lightning-12"), 71.0);
        assert_eq!(preset_label_width(""), 0.0);
        // The widest classes: capital W and any
        // non-ASCII character cost a full em.
        assert_eq!(preset_label_width("W"), 12.0);
        assert_eq!(preset_label_width("界"), 12.0);
    }

    #[test]
    fn preset_button_label_keeps_names_that_fit() {
        // Everything the app itself can generate (kind
        // slug + index) and ordinary user names pass
        // through byte-identical.
        for name in ["alpha", "lightning-12", "my-big-explosion-preset-v2"] {
            assert_eq!(preset_button_label(name), name);
        }
    }

    #[test]
    fn preset_button_label_truncates_a_long_name_in_the_middle() {
        // 30 + 1 + 30 characters at 7 pt each = 427 pt
        // of label in a 316 pt card: the row wrap cannot
        // help a single button this wide.
        let name = format!("{}-{}", "a".repeat(30), "z".repeat(30));
        let label = preset_button_label(&name);
        assert_ne!(label, name);
        assert!(label.contains('…'), "the cut is marked: {label}");
        assert!(label.starts_with("aaa"), "the head survives: {label}");
        assert!(label.ends_with("zzz"), "the tail survives: {label}");
        assert!(
            preset_label_width(&label) <= PRESET_LABEL_BUDGET,
            "the truncated label fits its budget: {}",
            preset_label_width(&label)
        );
    }

    #[test]
    fn preset_button_label_bounds_pathological_names() {
        // All-widest-glyph names, Latin and CJK: the
        // truncated label still fits the budget, by the
        // same estimate the builder trusts.
        for name in ["W".repeat(40), "界".repeat(40)] {
            let label = preset_button_label(&name);
            assert!(label.contains('…'), "{label}");
            assert!(
                preset_label_width(&label) <= PRESET_LABEL_BUDGET,
                "label for {name:?} fits: {}",
                preset_label_width(&label)
            );
        }
    }

    #[test]
    fn filter_still_finds_a_kind_by_name_when_it_is_in_use() {
        // Regression: the counted button label must not
        // leak into filtering. Sparkle is in use (×3, so
        // its button reads "Sparkle · 3"), and the filter
        // still finds it by its base name and label…
        let core = core_with_kinds(&[
            ClipKind::Sparkle,
            ClipKind::Sparkle,
            ClipKind::Sparkle,
        ]);
        assert_eq!(kind_usage(&core, ClipKind::Sparkle), 3);
        let sparkle = entry_of(ClipKind::Sparkle);
        assert!(kind_matches(sparkle, "sparkle"));
        assert!(kind_matches(sparkle, "Sparkle"));
        let visible = visible_categories("sparkle");
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].0, "Magic");
        assert_eq!(visible[0].1.len(), 1);
        assert_eq!(visible[0].1[0].kind, ClipKind::Sparkle);
        // …while the count suffix itself is not a match
        // source: no name, label or category contains it,
        // so filtering by it finds nothing.
        assert!(!kind_matches(sparkle, "· 3"));
        assert!(visible_categories("· 3").is_empty());
    }

    // ---- Filter + recents (Phase F) ----

    fn entry_of(kind: ClipKind) -> &'static FxKindEntry {
        FX_KINDS.iter().find(|e| e.kind == kind).expect("kind is in the table")
    }

    #[test]
    fn kind_matches_is_case_insensitive_substring_on_name_and_label() {
        let shock = entry_of(ClipKind::ShockRing);
        assert!(kind_matches(shock, "shock"));
        assert!(kind_matches(shock, "SHOCK"));
        assert!(kind_matches(shock, "Shock Ring"), "the humanised label matches too");
        assert!(kind_matches(shock, "ring"));
        assert!(kind_matches(shock, "  ring  "), "the query is trimmed");
        assert!(!kind_matches(shock, "beam"));
        // Empty and whitespace-only queries are "no filter".
        assert!(kind_matches(shock, ""));
        assert!(kind_matches(shock, "   "));
        assert!(kind_matches(entry_of(ClipKind::Beam), ""));
    }

    #[test]
    fn visible_categories_with_empty_query_is_the_full_grid() {
        let visible = visible_categories("");
        let names: Vec<&str> = visible.iter().map(|(cat, _)| *cat).collect();
        assert_eq!(names, FX_CATEGORIES);
        let sizes: Vec<usize> = visible.iter().map(|(_, kinds)| kinds.len()).collect();
        assert_eq!(sizes, vec![4, 4, 6, 4, 9]);
    }

    #[test]
    fn visible_categories_drops_categories_with_no_matches() {
        // One match: only its category survives, with only it.
        let visible = visible_categories("beam");
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].0, "Energy");
        assert_eq!(visible[0].1.len(), 1);
        assert_eq!(visible[0].1[0].kind, ClipKind::Beam);

        // Matches across three categories: all survive, in
        // category order, each holding only its matches in
        // table order. ("or" is a substring of orbit,
        // vortex and fireworks — Magic; aurora — Nature;
        // meteor — Particles. Verified against the table.)
        let visible = visible_categories("or");
        let names: Vec<&str> = visible.iter().map(|(cat, _)| *cat).collect();
        assert_eq!(names, vec!["Magic", "Nature", "Particles"]);
        let magic: Vec<ClipKind> = visible[0].1.iter().map(|e| e.kind).collect();
        assert_eq!(
            magic,
            vec![ClipKind::Orbit, ClipKind::Vortex, ClipKind::Fireworks]
        );
        let nature: Vec<ClipKind> = visible[1].1.iter().map(|e| e.kind).collect();
        assert_eq!(nature, vec![ClipKind::Aurora]);
        let particles: Vec<ClipKind> = visible[2].1.iter().map(|e| e.kind).collect();
        assert_eq!(particles, vec![ClipKind::Meteor]);

        // Nothing matches: no categories at all (the
        // builder renders the "no match" line for this).
        assert!(visible_categories("zzz-no-such-effect").is_empty());
    }

    // ---- Category-name matching (Phase J) ----

    #[test]
    fn category_matches_is_case_insensitive_substring_on_the_name() {
        assert!(category_matches("Energy", "energy"));
        assert!(category_matches("Energy", "ENER"));
        assert!(category_matches("Nature", "nat"));
        assert!(category_matches("Particles", "part"));
        assert!(!category_matches("Energy", "magic"));
        assert!(
            !category_matches("Energy", "beam"),
            "kind names are not categories"
        );
        // An empty filter is not a category match (that
        // is kind_matches' match-everything rule instead).
        assert!(!category_matches("Energy", ""));
        assert!(!category_matches("Energy", "   "));
    }

    #[test]
    fn kind_matches_also_matches_through_the_kinds_category() {
        // A kind matches when the query names its
        // category, even though neither its snake-case
        // name nor its humanised label contains the
        // query ("prism" has no "energy" in it).
        assert!(kind_matches(entry_of(ClipKind::Beam), "energy"));
        assert!(kind_matches(entry_of(ClipKind::Prism), "ENER"));
        // A kind from another category does not match.
        assert!(!kind_matches(entry_of(ClipKind::Burst), "energy"));
        assert!(!kind_matches(entry_of(ClipKind::Sparkle), "energy"));
    }

    #[test]
    fn visible_categories_for_a_category_query_is_that_category_at_full_size() {
        // "energy" names no kind; it names the category,
        // so the whole category shows, in table order.
        let visible = visible_categories("energy");
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].0, "Energy");
        let kinds: Vec<ClipKind> = visible[0].1.iter().map(|e| e.kind).collect();
        assert_eq!(
            kinds,
            vec![
                ClipKind::Beam,
                ClipKind::Aura,
                ClipKind::Lightning,
                ClipKind::Prism,
            ]
        );

        // A substring of a category name works the same
        // way: "nat" is Nature (and matches no kind name
        // anywhere in the table — verified against
        // FX_KINDS when this test was written).
        let visible = visible_categories("nat");
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].0, "Nature");
        let kinds: Vec<ClipKind> = visible[0].1.iter().map(|e| e.kind).collect();
        assert_eq!(
            kinds,
            vec![
                ClipKind::Leaves,
                ClipKind::Petals,
                ClipKind::Bubbles,
                ClipKind::Aurora,
            ]
        );
    }

    #[test]
    fn visible_categories_unions_kind_and_category_matches_without_duplicates() {
        // "par" is a substring of the kind name "sparkle"
        // (Magic) and of the category name "Particles":
        // the result is the union — Sparkle under Magic,
        // then the whole Particles category — in category
        // order, each kind exactly once.
        let visible = visible_categories("par");
        let names: Vec<&str> = visible.iter().map(|(cat, _)| *cat).collect();
        assert_eq!(names, vec!["Magic", "Particles"]);
        let magic: Vec<ClipKind> = visible[0].1.iter().map(|e| e.kind).collect();
        assert_eq!(magic, vec![ClipKind::Sparkle]);
        let particles: Vec<ClipKind> = visible[1].1.iter().map(|e| e.kind).collect();
        assert_eq!(
            particles,
            vec![
                ClipKind::Smoke,
                ClipKind::Ember,
                ClipKind::Fountain,
                ClipKind::Trail,
                ClipKind::Confetti,
                ClipKind::Meteor,
                ClipKind::Comet,
                ClipKind::Glitter,
                ClipKind::Wave,
            ]
        );
        let all: Vec<ClipKind> = visible
            .iter()
            .flat_map(|(_, kinds)| kinds.iter().map(|e| e.kind))
            .collect();
        assert_eq!(all.len(), 10);
        for kind in all.iter() {
            assert_eq!(
                all.iter().filter(|k| *k == kind).count(),
                1,
                "{kind:?} appears more than once"
            );
        }
    }

    #[test]
    fn record_recent_is_most_recent_first_deduped_and_capped() {
        let mut recent: Vec<ClipKind> = Vec::new();
        record_recent(&mut recent, ClipKind::Burst);
        assert_eq!(recent, vec![ClipKind::Burst]);
        record_recent(&mut recent, ClipKind::Beam);
        assert_eq!(recent, vec![ClipKind::Beam, ClipKind::Burst]);
        // Re-adding an existing kind moves it to the front
        // instead of duplicating it.
        record_recent(&mut recent, ClipKind::Burst);
        assert_eq!(recent, vec![ClipKind::Burst, ClipKind::Beam]);
        record_recent(&mut recent, ClipKind::Aura);
        assert_eq!(recent, vec![ClipKind::Aura, ClipKind::Burst, ClipKind::Beam]);
        // A fourth distinct kind pushes the oldest out —
        // the cap is MAX_RECENT (3).
        record_recent(&mut recent, ClipKind::Smoke);
        assert_eq!(recent, vec![ClipKind::Smoke, ClipKind::Aura, ClipKind::Burst]);
        assert_eq!(recent.len(), MAX_RECENT);
    }

    #[test]
    fn recent_entries_maps_back_to_table_entries_in_order() {
        let recent = vec![ClipKind::Wave, ClipKind::Burst];
        let entries = recent_entries(&recent);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].key, "fxclip-wave");
        assert_eq!(entries[1].key, "fxclip-burst");
        assert!(recent_entries(&[]).is_empty());
    }

    #[test]
    fn browser_state_defaults_to_empty_query_and_no_recents() {
        let state = FxBrowserState::default();
        assert!(state.query.text.is_empty());
        assert!(state.recent.is_empty());
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
