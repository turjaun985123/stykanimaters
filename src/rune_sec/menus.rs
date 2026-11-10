//! File / Edit / View / Help menus for the Rune editor shell
//! (Phase D, section 22 — menus; Phase E upgrade: View ▸
//! sidebar toggles and Help ▸ Command palette; Phase F:
//! right-aligned shortcut hints on the items that have
//! real shell key bindings, and View ▸ Stage guides;
//! Phase G: View ▸ Camera frame / Safe frames, the two
//! child toggles under Stage guides; Phase H: leading
//! Font Awesome glyphs on the high-value items — see
//! [`menu_icon`] — with keys, labels, active behaviour
//! and handlers unchanged; Phase J: File ▸ Open recent
//! entries (under Load until Wave 3's dedup — they head
//! the menu now) — see [`recent_items`]; Phase K:
//! the File ▸ Save item's Ctrl+S hint, shown only once
//! the binding really landed in `on_key` — see
//! [`shortcut_hint`] — and View ▸ Playback speed, an
//! action item cycling the shell's playback-speed
//! factor through the playback section's
//! `cycle_playback_speed`; Phase L: Edit ▸ Delete
//! stroke / Delete particle event, offered only while
//! that selection exists and running the Delete key's
//! own branches — see the EDIT arm of
//! [`menu_dropdown`]; Phase M: Edit ▸ Swap frame with
//! next and Edit ▸ Mute/Unmute audio clip, two more
//! conditional items on the same offer-only-while-
//! the-action-can-run discipline — see the EDIT arm).
//! Phase N (foundation F5): the charter §9 menu restyle
//! — the dropdown at radius 12 / padding 6 with the
//! P15 M2 fade-and-rise entrance, items 28 high at
//! px 8 with a fixed 18-wide leading icon slot so
//! labels share one left edge, shortcut hints as kit
//! `kbd` chips, and the separators as keyed kit
//! dividers — keys, handlers and offer conditions
//! unchanged throughout.
//! UI mission Wave 3 (audit S04 D4): the File and Edit
//! menus no longer re-mount Save / Load / Undo / Redo —
//! the app bar's buttons are those four keys' single
//! mount now, so an open menu never presents a key
//! twice. The app bar gates its Undo/Redo on the core's
//! stack depths (D2), which is also why the Edit menu
//! carries no inert undo/redo rows of its own: these
//! menus have never had an inert item form (state-gated
//! items are offered only while they can run — the
//! Phase L/M discipline), and the commands' one
//! remaining mount is honest about when it can run.
//! The keyboard bindings (Ctrl+S / Ctrl+Z /
//! Ctrl+Shift+Z) still route in `main.rs::on_key` to
//! the same ops, so [`shortcut_hint`] and [`menu_icon`]
//! keep their entries for the four keys: the maps
//! describe the commands, whose home is the app bar
//! now. Every other entry, hint and handler is
//! unchanged; the one separator consequence is
//! recorded at the Edit arm (menu-edit-sep removed
//! with the group it divided) and the File arm
//! (menu-file-sep now sits between recents and
//! Export, where the division actually is).
//! Wave 13 (D5, dimmed recents): an Open recent entry
//! whose file no longer resolves now renders DIMMED —
//! the muted-live treatment (see [`dimmed_item`]):
//! glyph and label in the kit's muted-text role, no
//! hover/press light-up, press handler kept, because
//! the press's refusal status ("Recent project not
//! found") is the designed feedback. Resolved entries
//! render exactly as before. This closes the Wave 12
//! D7 remainder: the entries stayed visible but were
//! indistinguishable from live ones.
//! Wave 12 (D7, recents audit): the File menu's Open
//! recent resolution gained the file-stem fallback
//! Home ▸ Reopen last has had since Wave 10 — see
//! [`resolve_recent`] — closing the one divergence the
//! audit found between the two readers of
//! `recents.json` (after a Home rename the menu entry
//! for the renamed project reported "not found" while
//! Reopen resolved it). Cap (5 of the writer's 10),
//! order, dedup (the writer's job, shell.rs) and the
//! empty state audited clean; no keys added or changed.
//! Wave U (U3, menus i18n): every menu-bar trigger and
//! dropdown label now resolves through `i18n_wire::tr`
//! under a `menu_*` key (the subtitle/camera/3D idiom;
//! keys via IA-U3-1, English byte-identical once they
//! land). That includes Edit ▸ Mute/Unmute audio clip —
//! Wave T's label pass (T2) excluded the menus' own
//! mute labels because it did not own this file;
//! [`mute_item_parts`] now builds its label with `tr`
//! on the core IA-T2-2 already threaded into it, so its
//! return carries a `String` label. Not wired, by
//! design: the Open recent entries' labels are the
//! projects' own display names (user data, not UI
//! copy); the shortcut hints (`shortcut_hint`'s
//! "Ctrl+S" …) are key-chord notation, locale-invariant
//! like the stepper glyphs (the R6 precedent), and no
//! code parses them; and `open_recent`'s "Recent
//! project not found: …" is a handler status sentence,
//! not a label — status sentences stay English in the
//! label passes (the T2/T20 discipline), and this one
//! is pinned verbatim by a test below.
//! Declared from `main.rs` by the integrator; bin-crate
//! code, not part of the engine library.
//!
//! # Approach (chosen from source evidence)
//!
//! Three ways to build menus on the pinned Rune rev were
//! audited (`docs/RUNE_API_AUDIT.md` Q10):
//!
//! - **Core `rune::PopupMenu`** is a *native* OS menu, and
//!   `popup_menu_supported()` is macOS/Windows only — on
//!   Linux `Tx::popup_menu` calls back with `None` without
//!   showing anything (crates/rune/src/popup_menu/mod.rs).
//!   Rejected: the target desktop is Linux.
//! - **Kit `rune_kit::Menu`** fits (`MenuItem::on_select` is
//!   generic over the app state), but `Menu::build` needs a
//!   `&rune_kit::Kit` and a `&mut Vec<El>` overlay layer —
//!   the shell uses `rune::tok` theming and has no kit/layer
//!   plumbing, so adopting it here would force the
//!   integrator to re-theme the whole shell around `Kit`.
//!   Rejected for this section; revisit if the shell adopts
//!   rune-kit wholesale.
//! - **State-toggled anchored column (chosen):** triggers
//!   are ordinary chrome buttons that flip
//!   [`MenusState::open`]; the open menu is a column of the
//!   same buttons, positioned with core
//!   `El::anchored(trigger_key, Side::Bottom, Align::Start)`
//!   (crates/rune/src/el/anchor.rs — out-of-flow, follows
//!   the anchor's drawn rect, flips/clamps at the window
//!   edges). Dismissal mirrors rune-kit's own pattern
//!   (crates/rune-kit/src/overlay.rs `scrim`): a transparent
//!   full-window press target under the surface, plus
//!   close-on-select and trigger toggle.
//!
//! # Integrator wiring (required)
//!
//! ```ignore
//! // main.rs
//! mod rune_sec { pub mod menus; }  // (or a rune_sec.rs)
//! // State: add `pub menus: menus::MenusState`,
//! // init with `MenusState::default()`.
//! // State (Phase E): add `pub show_left: bool` and
//! // `pub show_right: bool`, both init `true`; view()
//! // mounts the left (figures) column only while
//! // `show_left` is set and the right (inspector) column
//! // only while `show_right` is set — the View menu's
//! // sidebar toggles flip them.
//! // view(): mount `menus::menu_bar(s, &t)` as its own row
//! // under the app bar; keep two zero-size `el()` spacers
//! // as the LAST in-flow root-column children (the menu
//! // layer's slots, mounted in both states so the root's
//! // gap accounting never depends on the menu state —
//! // UI mission Wave 8), and append, only while a menu
//! // is open, in this order:
//! //     menus::menu_scrim(s)
//! //     menus::menu_dropdown(s, &t)
//! // on_key: if Escape and `s.menus.open.is_some()`,
//! // close (`s.menus.open = None`) and return true.
//! ```
//!
//! Every item calls exactly one real `EditorCore` op (or,
//! for Onion, flips the same public core field the app-bar
//! Onion button flips — there is no op for it). Phase E
//! added the two View ▸ sidebar toggles, which flip the
//! shell's own `State::show_left` / `State::show_right`
//! layout fields (integrator-owned, see the wiring above),
//! and Help ▸ Command palette, which calls the palette
//! section's `open_palette`. Element keys
//! carry the Wave 15 menu ids (`menu-file` …) on the
//! triggers and the original GPUI button ids on the items,
//! so headless `--tree` checks can find them.

use std::path::{Path, PathBuf};

use rune::tok::{self, type_, Theme};
use rune::{col, el, row, text, Align, Color, El, Side, Transition};

use crate::chrome_view::btn;
use crate::rune_sec::i18n_wire;
use crate::State;

/// Menu indices in [`MenusState::open`] — the same numbering
/// the GPUI app's `open_menu` used: File, Edit, View, Help.
pub const FILE: usize = 0;
pub const EDIT: usize = 1;
pub const VIEW: usize = 2;
pub const HELP: usize = 3;

/// Trigger labels (canonical English), in index order.
/// The bar renders each through its [`MENU_LABEL_KEYS`]
/// key; this const stays as the English source the
/// catalogue values must match (pinned by
/// `menu_tables_line_up` and the Wave U i18n pin below).
pub const MENUS: [&str; 4] = ["File", "Edit", "View", "Help"];

/// Trigger element keys, in index order (the Wave 15 menu
/// ids). The dropdown for a menu anchors to the element
/// carrying its trigger's key.
pub const MENU_KEYS: [&str; 4] = ["menu-file", "menu-edit", "menu-view", "menu-help"];

/// Trigger label keys, in index order — parallel to
/// [`MENUS`] (the canonical English) and [`MENU_KEYS`]
/// (the element keys): the bar renders
/// `tr(core, MENU_LABEL_KEYS[i])` under element key
/// `MENU_KEYS[i]`, and each key's English value is
/// exactly `MENUS[i]` (Wave U, U3; keys via IA-U3-1).
pub const MENU_LABEL_KEYS: [&str; 4] = ["menu_file", "menu_edit", "menu_view", "menu_help"];

/// Menu-bar state. Owned by the integrator's `State` (field
/// `menus`); this section's builders read it and its item
/// handlers mutate it through `&mut State`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MenusState {
    /// Which menu is open, by index (`FILE`…`HELP`); `None`
    /// when all menus are closed.
    pub open: Option<usize>,
    /// Set by the Help ▸ Shortcuts item; the integrator
    /// renders the help panel while this is true.
    pub help_open: bool,
}

impl MenusState {
    /// Trigger press: close if this menu is the open one,
    /// otherwise open it (switching from any other menu).
    pub fn toggle(&mut self, index: usize) {
        self.open = if self.open == Some(index) {
            None
        } else {
            Some(index)
        };
    }

    /// Close any open menu (item selection, scrim press,
    /// Escape).
    pub fn close(&mut self) {
        self.open = None;
    }
}

/// The item handler wrapper: run the item's action, then
/// close the menu (selecting an item always dismisses it,
/// as in the GPUI dropdowns). Every item — hinted or not —
/// goes through this same wrapper.
fn closing(f: impl Fn(&mut State) + 'static) -> impl Fn(&mut State) + 'static {
    move |s: &mut State| {
        f(s);
        s.menus.close();
    }
}

/// The keyboard-shortcut hint shown at a menu item's right
/// edge, keyed by the item's element key — or `None` when
/// the shell binds no key to the item's action. Every entry
/// was verified against the shell's actual key routing
/// (`main.rs::on_key`, which runs
/// `command_palette::palette_handle_key` first):
///
/// - `save` — Ctrl+S (`on_key`'s `is_command('s')` arm
///   runs `core.save()` — the Save item's own op, no
///   second save path; the binding landed in Phase K,
///   and the hint was added in the same phase, not
///   before it existed);
///
/// - `undo` — Ctrl+Z (`on_key`: `is_command('z')` without
///   shift runs `core.undo()`);
/// - `redo` — Ctrl+Shift+Z (the same arm with shift;
///   Ctrl+Y is a second binding for the same op — the
///   hint shows the primary one);
/// - `onion` — O (`on_key`'s plain-`o` arm flips
///   `core.onion`, the same field this item flips);
/// - `menu-palette` — Ctrl+K (`palette_handle_key`:
///   `is_command('k')` opens the palette while closed).
///
/// Everything else in these menus — Load, Export
/// PNG, Copy/Paste frame, Fit view, Centre on selection,
/// the sidebar toggles, Stage guides, Camera frame, Safe
/// frames, Playback speed, Shortcuts — has NO
/// shell key binding and gets no hint: an invented hint
/// would be a lie on screen. The Edit menu's selection
/// deletes (Phase L: Delete stroke / Delete particle
/// event) get no hint either, for a subtler reason: the
/// shell's Delete key IS bound, but to `on_key`'s
/// priority cascade — stroke first, then particle
/// event, then the selected segment — not to either
/// item's single action. A "Delete" hint on the stroke
/// item would claim the key targets strokes, when with
/// no stroke selected it deletes something else
/// entirely. The items stay mouse-only doors onto the
/// cascade's first two branches. Phase M's two
/// conditional Edit items — Swap frame with next and
/// the audio clip Mute/Unmute toggle — get no hint
/// either: both are state-dependent actions (a next
/// frame must exist; an audio clip must be targeted)
/// and no global key is bound to either one alone.
/// The spellings match the
/// help panel's shortcut table (`help_panel.rs`). Pinned
/// by the `shortcut_hints_name_only_real_bindings` test.
///
/// Roadmap Wave 1 (RM26, PL-5): the table itself now
/// lives in the binding registry (`keymap::BINDINGS`) —
/// this fn is a delegate, so the menu hints, the help
/// rows and the palette chips share one spelling by
/// construction. PL-4 added the Edit menu's Copy/Paste
/// frame pair (Ctrl+C / Ctrl+V — the items call the same
/// `copy_frame` / `paste_frame` ops the chords run).
fn shortcut_hint(key: &str) -> Option<&'static str> {
    crate::rune_sec::keymap::menu_hint(key)
}

/// The fixed width (pt) of the leading icon slot every
/// menu item reserves (Phase N, P07 §1.2): room for the
/// 12 pt glyph plus air, identical on icon, hinted and
/// plain items alike, so labels share one left edge down
/// the whole menu instead of zig-zagging between icon
/// and plain rows.
const ICON_SLOT_W: f32 = 18.0;

/// The leading slot + label group every item builder
/// shares (Phase N): an [`ICON_SLOT_W`]-wide slot holding
/// the item's 12 pt glyph at its left edge — or nothing,
/// for hinted and plain items — then the label, 6 pt
/// apart (the glyph-to-label rhythm the icon items
/// always had, now every row's geometry). `color` is
/// the label's own colour (white while active, theme
/// text otherwise); the glyph takes the same colour,
/// exactly as [`item_icon`] always drew it.
fn slot_and_label(icon: Option<&str>, label: &str, color: Color) -> El {
    let slot = el().w(ICON_SLOT_W);
    let slot = match icon {
        Some(name) => slot.child(crate::rune_sec::icons::icon_el(name, 12.0, color)),
        None => slot,
    };
    row()
        .gap(6.0)
        .center_y()
        .child(slot)
        .child(text(label).font(type_::LABEL).color(color))
}

/// A menu item's right-edge shortcut hint (Phase N,
/// P07 §1.2): a kit `kbd` chip while the item is
/// inactive. While the item is active (accent fill) a
/// `control_off` chip would sit low-contrast on the
/// accent, so the hint stays the pre-Phase N treatment
/// instead — plain text, white at 75% alpha, mirroring
/// the white label.
fn hint_el(hint: &str, active: bool, t: &Theme) -> El {
    if active {
        text(hint)
            .font(type_::LABEL)
            .color(Color::WHITE.alpha(0.75))
    } else {
        crate::rune_sec::design::kbd(hint, t)
    }
}

/// A menu item carrying a shortcut hint: the same chrome
/// as `chrome_view::btn` (radius 6, the same
/// fill/hover/press states and label font), rebuilt as a
/// row so the hint rides inside the same pressable
/// element — the item's key, press handler and hit area
/// cover the whole row, hint included, like a native
/// menu. Phase N (charter §9, P07 §1.2) fixed the row at
/// 28 high / px 8, gave it the shared leading icon slot
/// (empty here — see [`slot_and_label`]) and made the
/// hint a `kbd` chip (see [`hint_el`]).
fn hinted_item(
    key: &str,
    label: &str,
    hint: &str,
    active: bool,
    t: &Theme,
    f: impl Fn(&mut State) + 'static,
) -> El {
    let accent = tok::accent(0, true);
    let ink = if active { Color::WHITE } else { t.text };
    row()
        .key(key)
        .px(8.0)
        .py(5.0)
        .h(28.0)
        .radius(6.0)
        .fill(if active { accent } else { t.card })
        .hover(if active { accent } else { t.control_track })
        .press(if active { accent } else { t.control_off })
        .on_press(f)
        .center_y()
        .gap(12.0)
        .child(slot_and_label(None, label, ink))
        .child(row().grow(1.0))
        .child(hint_el(hint, active, t))
}

/// The Font Awesome glyph an item leads with (Phase H),
/// keyed by the item's element key — or `None` when the
/// item stays plain text. Only the high-value items are
/// mapped: the File actions, Edit's undo/redo, frame
/// clipboard, selection deletes and the frame swap,
/// View's visibility
/// toggles, its Playback speed action and sidebar
/// switches, and Help's two entries. Fit view / Centre on
/// selection stay unmapped (no glyph reads as clearly,
/// and an unclear icon is worse than none). The File ▸
/// Open recent entries (Phase J) are the one dynamic
/// family: their keys are `file-recent-{i}`, so they
/// resolve by prefix to the clock glyph. Edit's
/// Mute/Unmute audio clip item (Phase M) is
/// deliberately absent from the map: its glyph depends
/// on the targeted clip's state, so its builder picks
/// the icon (via `mute_item_parts`) and hands it to
/// `item_icon` directly.
///
/// Every name was verified against the icon set
/// (`icon_data::ICON_NAMES`); even so, [`item`] checks
/// `icons::has_icon` before rendering the glyph and
/// falls back to the plain/hinted row when the lookup
/// fails, so a renamed or dropped glyph degrades the
/// item instead of breaking the menu. Pinned by the
/// `menu_icons_name_real_glyphs_for_the_high_value_items`
/// test.
fn menu_icon(key: &str) -> Option<&'static str> {
    // File ▸ Open recent entries (Phase J) carry dynamic
    // keys (`file-recent-{i}`), so they resolve by prefix:
    // every recent leads with the clock glyph.
    if key.starts_with("file-recent-") {
        return Some("clock");
    }
    match key {
        // File: open / save / export out.
        "load" => Some("folder-open"),
        "save" => Some("floppy-disk"),
        "export" => Some("download"),
        // Edit: undo / redo, copy / paste frame, and the
        // selection deletes (Phase L) — the trash can
        // reads as removal at a glance. The frame swap
        // (Phase M) leads with the two-way arrows: the
        // frames trade places.
        "undo" => Some("arrow-rotate-left"),
        "redo" => Some("arrow-rotate-right"),
        "copy-frame" => Some("copy"),
        "paste-frame" => Some("clipboard"),
        "edit-delete-stroke" => Some("trash-can"),
        "edit-delete-particle" => Some("trash-can"),
        "edit-swap-next" => Some("arrow-right-arrow-left"),
        // View: visibility (eye), the guides sub-toggles
        // (crosshairs), the sidebar switches (list).
        "onion" => Some("eye"),
        "view-paths" => Some("bezier-curve"),
        "view-guides" => Some("eye"),
        "view-guides-frustum" => Some("crosshairs"),
        "view-guides-safe" => Some("crosshairs"),
        // View ▸ Playback speed (Phase K): the gauge —
        // a dial reads as speed at a glance, and the
        // glyph is in the curated set (`icon_data`).
        "view-speed" => Some("gauge"),
        "view-toggle-left" => Some("list"),
        "view-toggle-right" => Some("list"),
        // Help: palette search, shortcuts question.
        "menu-palette" => Some("magnifying-glass"),
        "help" => Some("circle-question"),
        _ => None,
    }
}

/// A menu item leading with a Font Awesome glyph: the
/// same row chrome as [`hinted_item`] (radius 6, the
/// same fill/hover/press states), with the glyph at
/// 12 pt in the label's own colour (white while active,
/// theme text otherwise) inside the shared leading slot
/// (see [`slot_and_label`]), and the shortcut hint —
/// for the items that have one — at the right edge
/// exactly as `hinted_item` renders it (see
/// [`hint_el`]). Key, press handler and hit area cover
/// the whole row, icon included. Phase N (charter §9,
/// P07 §1.2) fixed the row at 28 high / px 8.
fn item_icon(
    key: &str,
    icon: &str,
    label: &str,
    active: bool,
    hint: Option<&'static str>,
    t: &Theme,
    f: impl Fn(&mut State) + 'static,
) -> El {
    let accent = tok::accent(0, true);
    let label_color = if active { Color::WHITE } else { t.text };
    let mut r = row()
        .key(key)
        .px(8.0)
        .py(5.0)
        .h(28.0)
        .radius(6.0)
        .fill(if active { accent } else { t.card })
        .hover(if active { accent } else { t.control_track })
        .press(if active { accent } else { t.control_off })
        .on_press(f)
        .center_y()
        .gap(12.0)
        .child(slot_and_label(Some(icon), label, label_color))
        .child(row().grow(1.0));
    if let Some(hint) = hint {
        r = r.child(hint_el(hint, active, t));
    }
    r
}

/// A plain menu item — no glyph, no hint (Phase N,
/// P07 §1.2): the menus-local builder for the items the
/// [`menu_icon`] map leaves unmapped and
/// [`shortcut_hint`] leaves hint-less (today exactly
/// View ▸ Fit view and Centre on selection). It mirrors
/// `chrome_view::btn`'s fills and radius at the menu
/// item metrics (28 high, px 8) and reserves the same
/// empty leading slot as every other item (see
/// [`slot_and_label`]), so its label lines up with the
/// icon items' labels instead of starting 18 pt
/// earlier. Before Phase N these items were bare `btn`s
/// — the last label zig-zag in the menus.
fn plain_item(
    key: &str,
    label: &str,
    active: bool,
    t: &Theme,
    f: impl Fn(&mut State) + 'static,
) -> El {
    let accent = tok::accent(0, true);
    let ink = if active { Color::WHITE } else { t.text };
    row()
        .key(key)
        .px(8.0)
        .py(5.0)
        .h(28.0)
        .radius(6.0)
        .fill(if active { accent } else { t.card })
        .hover(if active { accent } else { t.control_track })
        .press(if active { accent } else { t.control_off })
        .on_press(f)
        .center_y()
        .child(slot_and_label(None, label, ink))
}

/// One menu item: a chrome button that runs `f` and then
/// closes the menu (see [`closing`]). Items in the
/// [`menu_icon`] map lead with their glyph (see
/// [`item_icon`]); items whose action has a real shell
/// key binding (see [`shortcut_hint`]) render it as a
/// muted hint at the row's right edge; the rest go
/// through the menus-local plain builder (see
/// [`plain_item`]), which reserves the same leading
/// slot so their labels align with the icon items'.
fn item(
    key: &str,
    label: &str,
    active: bool,
    t: &Theme,
    f: impl Fn(&mut State) + 'static,
) -> El {
    match menu_icon(key) {
        Some(icon) if crate::rune_sec::icons::has_icon(icon) => {
            item_icon(key, icon, label, active, shortcut_hint(key), t, closing(f))
        }
        _ => match shortcut_hint(key) {
            Some(hint) => hinted_item(key, label, hint, active, t, closing(f)),
            None => plain_item(key, label, active, t, closing(f)),
        },
    }
}

/// A dimmed menu item (Wave 13, D5): [`item_icon`]'s
/// row geometry — same key, same 28-high / px 8 box,
/// same leading slot, glyph included — rendered in the
/// kit's MUTED-LIVE treatment, the menu form of
/// `chrome_view::btn_muted_live` (P05 §4's rule): the
/// resting card fill with NO hover or press light-up,
/// so the row reads as listed-but-inert rather than as
/// a live control. Glyph + label take
/// `text_secondary`: the label is a project name the
/// user must read to choose it — real text — and
/// Roadmap Wave 1 (RM25 / PL-1) retired the muted
/// role as its ink (it measured ≈ 3.8 dark / ≈ 2.0–2.6
/// light, under every text bar); the muted-live read
/// is carried by the absent light-up, as the
/// treatment always intended. The press handler is KEPT (through the
/// same [`closing`] wrapper as every item): the one
/// caller is an Open recent entry whose file is gone,
/// and its press's refusal status ("Recent project not
/// found: …", see [`open_recent`]) is the designed
/// feedback — the chrome_view rule for muted-live is
/// exactly "use only where a test pins that refusal
/// status", and the `open_recent` no-file test below
/// pins this one. A
/// fully inert row (no handler at all, the `btn_inert`
/// form) would swallow the press silently and the user
/// would never learn why the entry does nothing.
///
/// Only the Open recent block builds these (see
/// [`file_recent_items`]); the glyph dispatch mirrors
/// [`item`]'s — the key's mapped glyph when it exists
/// in the set, the empty slot otherwise — and recents
/// carry no shortcut hint, so there is no hint slot to
/// dim.
fn dimmed_item(
    key: &str,
    label: &str,
    t: &Theme,
    f: impl Fn(&mut State) + 'static,
) -> El {
    let ink = t.text_secondary;
    let icon = match menu_icon(key) {
        Some(name) if crate::rune_sec::icons::has_icon(name) => Some(name),
        _ => None,
    };
    row()
        .key(key)
        .px(8.0)
        .py(5.0)
        .h(28.0)
        .radius(6.0)
        .fill(t.card)
        .hover(t.card)
        .press(t.card)
        .on_press(closing(f))
        .center_y()
        .gap(12.0)
        .child(slot_and_label(icon, label, ink))
        .child(row().grow(1.0))
}

/// How many Open recent entries the File menu shows at
/// most (Phase J). The recents list itself keeps more
/// (`shell::MAX_RECENTS`); a menu is a quick door, not
/// the whole history.
pub const MAX_RECENT_ITEMS: usize = 5;

/// The file-name slug for a project name — lowercased,
/// every non-alphanumeric character replaced by '-':
/// the crate-wide naming convention. Each owner keeps a
/// private copy (`shell::project_file_slug`,
/// `home_panel::name_file_slug`, `projects::slug`); keep
/// this one in sync with them.
fn recent_name_slug(name: &str) -> String {
    name.to_lowercase()
        .replace(|c: char| !c.is_alphanumeric(), "-")
}

/// One recent name's file (Wave 12, D7): the two-pass
/// resolution Home ▸ Reopen last runs
/// (`home_panel::reopen_target`), over the (stored name,
/// path) pairs.
///
/// Pass 1 is the exact stored-name match. Pass 2 — the
/// fallback `reopen_target` gained in Wave 10 and this
/// menu lacked until Wave 12 — matches the name's slug
/// against the files' stems: a Home rename is file-only
/// (the stored name inside the file updates only when
/// the project is saved again), yet `confirm_rename`
/// re-touches recents with the typed name, so after a
/// rename NO stored name matches the recents entry while
/// the renamed file sits right there under the name's
/// slug. Without this pass the menu showed that entry
/// but its press reported "Recent project not found"
/// for a project Home could reopen fine — the two
/// readers of `recents.json` disagreed. The exact match
/// keeps precedence (the stem pass runs only when no
/// stored name matches), and both passes search the
/// same pre-filtered pairs, so the recovery snapshot
/// and shell-owned entries stay excluded from both.
fn resolve_recent(name: &str, files: &[(String, PathBuf)]) -> Option<PathBuf> {
    if let Some(path) = files
        .iter()
        .find(|(file_name, _)| file_name == name)
        .map(|(_, path)| path.clone())
    {
        return Some(path);
    }
    let stem = recent_name_slug(name);
    if stem.is_empty() {
        return None;
    }
    files
        .iter()
        .find(|(_, path)| {
            path.file_stem().and_then(|s| s.to_str()) == Some(stem.as_str())
        })
        .map(|(_, path)| path.clone())
}

/// The File menu's Open recent resolution (Phase J;
/// Wave 12, D7: the stem fallback): pair each of the
/// first [`MAX_RECENT_ITEMS`] recent display names with
/// the project file that carries it.
///
/// `recents` are display names, most recent first
/// (`stykanimate_rs::shell::recents` — recents store
/// names, not paths). `files` are the (stored name, path)
/// pairs to resolve against; the menu passes the Home
/// panel's cached library scan, filtered the way its rows
/// are (`home_panel::visible_projects`), so a name
/// resolves exactly the way Home ▸ Reopen last resolves
/// it — exact stored-name match first, file-stem match
/// second (see [`resolve_recent`]), with the recovery
/// snapshot and shell-owned entries already excluded.
///
/// A name with no matching file comes back as
/// `(name, None)` rather than being dropped: the entry
/// stays in the menu — rendered dimmed since Wave 13
/// (see [`file_recent_items`]) — and its press reports
/// "Recent project not found: {name}" on the status
/// line (see [`open_recent`]). Silently dropping it
/// would make the
/// menu's contents depend on disk state without telling
/// the user why an entry vanished — and the first recent
/// is exactly the one whose disappearance would confuse.
/// The cap counts names, resolved or not, so the menu
/// never grows past five recents plus the separator.
pub fn recent_items(
    recents: &[String],
    files: &[(String, PathBuf)],
) -> Vec<(String, Option<PathBuf>)> {
    recents
        .iter()
        .take(MAX_RECENT_ITEMS)
        .map(|name| (name.clone(), resolve_recent(name, files)))
        .collect()
}

/// The (stored name, path) pairs the File menu resolves
/// recents against: the Home section's cached library
/// scan (`HomePanelState::projects` — scanned at most
/// once per change to the projects directory), filtered
/// to the entries its rows show
/// (`home_panel::visible_projects` with no query), so
/// the recovery snapshot can never shadow a real project
/// of the same stored name.
fn recent_file_pairs(s: &State) -> Vec<(String, PathBuf)> {
    let entries = s.home_panel.projects();
    crate::rune_sec::home_panel::visible_projects(&entries, "")
        .into_iter()
        .map(|info| (info.name, info.path))
        .collect()
}

/// One Open recent press (Phase J). With a resolved file
/// it takes the same route as every other project open:
/// the Home section's `dispatch_open` (select the file in
/// the core's list — adding it if unlisted — then
/// `EditorCore::open_project_file` and record the
/// recent), followed by the same post-open guards the
/// Project section's row Open applies
/// (`project_tools::clear_stale_solo`,
/// `project_tools::sync_name_edit`), so the name field
/// shows the opened project. Without a file — the
/// project was deleted or renamed outside the editor —
/// it sets a clear status and changes nothing else.
fn open_recent(s: &mut State, name: &str, path: Option<&Path>) {
    match path {
        Some(path) => {
            crate::rune_sec::home_panel::dispatch_open(&mut s.core, path, name);
            crate::rune_sec::project_tools::clear_stale_solo(&mut s.core);
            crate::rune_sec::project_tools::sync_name_edit(s);
            // Wave 11 (E9): the stroke selection indexes the
            // old project's strokes — clear it on every swap,
            // as Home's open does (this path missed it).
            s.stroke_selected = None;
        }
        None => {
            s.core.status = format!("Recent project not found: {name}");
        }
    }
}

/// One Edit ▸ Delete stroke press (Phase L): exactly
/// the Delete key's stroke branch in `main.rs::on_key`
/// — delete the selected stroke through the core op,
/// then clear the selection. The op refuses safely
/// (status only) on a stale index and the selection is
/// cleared either way, matching the key. A no-op when
/// nothing is selected (the item is only offered while
/// a stroke is selected, so this is a guard, not a
/// reachable path).
fn delete_selected_stroke(s: &mut State) {
    if let Some(i) = s.stroke_selected {
        s.core.delete_stroke_at(i);
        s.stroke_selected = None;
    }
}

/// One Edit ▸ Delete particle event press (Phase L):
/// exactly the Delete key's particle branch in
/// `main.rs::on_key` — delete the selected event
/// through the particle op, then clear the selection.
/// Same stale-index discipline as
/// [`delete_selected_stroke`].
fn delete_selected_particle(s: &mut State) {
    if let Some(i) = s.particle_selected {
        s.core.particle_delete_event(i);
        s.particle_selected = None;
    }
}

/// One Edit ▸ Swap frame with next press (Phase M):
/// the frame_swap section's one-step op with dir +1 —
/// the current frame trades contents with its
/// successor (attached data remapped, one snapshot)
/// and the playhead index stays where it was, looking
/// at the neighbour's content; the op sets its own
/// status either way. A refusal on the last frame;
/// the item is only offered while a next frame
/// exists, so this is a guard, not a reachable path.
fn swap_frame_with_next(s: &mut State) {
    stykanimate_rs::frame_swap::swap_frame_with_neighbour(&mut s.core, 1);
}

/// One Edit ▸ Mute/Unmute audio clip press (Phase M):
/// toggle the muted flag on the Audio panel's
/// effective target — the live selection, or the last
/// clip when nothing is selected (see
/// `audio_panel::effective_target`) — through the
/// audio_ops indexed toggle, which sets its own
/// status and snapshots. A no-op when the project has
/// no clips; the item is only offered while a target
/// exists, so this is a guard, not a reachable path.
fn toggle_targeted_clip_muted(s: &mut State) {
    if let Some(target) = crate::rune_sec::audio_panel::effective_target(
        s.audio_panel.selected,
        s.core.project.audio.len(),
    ) {
        stykanimate_rs::audio_ops::audio_toggle_muted_at(&mut s.core, target);
    }
}

/// Edit ▸ Mute/Unmute audio clip's (label, icon) for
/// the targeted clip's current state (Phase M): the
/// pair names the ACTION a press performs, not the
/// state — an audible clip offers "Mute audio clip",
/// a muted one "Unmute audio clip". The glyph comes
/// from `audio_panel::mute_toggle_parts`, so the menu
/// and the Audio panel's own Mute toggle always show
/// the same icon for the same state; the menu spells
/// the label out in full where the panel's tight row
/// says just "Mute" / "Unmute". (The item cannot go
/// through the static [`menu_icon`] map: its glyph
/// depends on state the map cannot see, so the EDIT
/// branch hands the icon to [`item_icon`] directly —
/// see [`menu_dropdown`].) Wave U (U3): the label
/// resolves through `i18n_wire::tr` under
/// `menu_mute_clip` / `menu_unmute_clip` (keys via
/// IA-U3-1) on the core IA-T2-2 threaded in for the
/// glyph — the same construction
/// `audio_panel::mute_toggle_parts` uses for the
/// panel's own "Mute" / "Unmute" — so the return
/// carries the label as a `String`.
fn mute_item_parts(core: &stykanimate_rs::editor::EditorCore, muted: bool) -> (String, &'static str) {
    let (icon, _) = crate::rune_sec::audio_panel::mute_toggle_parts(core, muted);
    let label = i18n_wire::tr(
        core,
        if muted { "menu_unmute_clip" } else { "menu_mute_clip" },
    );
    (label, icon)
}

/// Whether a resolved Open recent entry renders
/// dimmed (Wave 13, D5): exactly the entries whose
/// file did not resolve — the project was deleted or
/// renamed outside the editor, so there is nothing to
/// open. Resolved entries never dim. The pure dispatch
/// [`file_recent_items`] branches on, pinned by the
/// `recent_dimming_follows_resolution` test (Rune's
/// `El` exposes no tree-walk, so — the Phase J/L style
/// — the branch is pinned here and the builders are
/// exercised by construction).
fn recent_is_dimmed(path: &Option<PathBuf>) -> bool {
    path.is_none()
}

/// The File menu's Open recent block (Phase J): a
/// separator plus one item per entry from
/// [`recent_items`], keyed `file-recent-{i}` in recents
/// order (most recent first) and labelled with the
/// recent's display name. Empty when there are no
/// recents — no disabled placeholder item: a menu that
/// cannot do anything stays honest by not offering it.
///
/// Wave 13 (D5): an entry whose file did not resolve
/// renders through [`dimmed_item`] — same key, same
/// label, muted glyph and text, no hover/press
/// light-up — so a dead entry is plainly inert next to
/// the live ones; its press still runs [`open_recent`],
/// whose not-found status is the designed answer to
/// "why is this here but grey?". Resolved entries go
/// through [`item`] exactly as before.
fn file_recent_items(
    recents: &[String],
    files: &[(String, PathBuf)],
    t: &Theme,
) -> Vec<El> {
    let resolved = recent_items(recents, files);
    if resolved.is_empty() {
        return Vec::new();
    }
    let mut items = Vec::new();
    for (i, (name, path)) in resolved.into_iter().enumerate() {
        let key = format!("file-recent-{i}");
        let label = name.clone();
        let dimmed = recent_is_dimmed(&path);
        let handler = move |s: &mut State| {
            open_recent(s, &name, path.as_deref());
        };
        if dimmed {
            items.push(dimmed_item(&key, &label, t, handler));
        } else {
            items.push(item(&key, &label, false, t, handler));
        }
    }
    items
}

/// A thin divider line between item groups — the kit's
/// keyed divider (Phase N, charter §9 / P07 §1.2): the
/// key rides on the divider element itself, so the five
/// separator keys survive the swap from the hand-rolled
/// line byte-identical.
fn separator(key: &str, t: &Theme) -> El {
    crate::rune_sec::design::divider_keyed(key, t)
}

/// The menu bar: one trigger button per menu, keyed with
/// the Wave 15 menu ids; a trigger renders active while its
/// menu is open.
pub fn menu_bar(s: &State, t: &Theme) -> El {
    let mut bar = row().key("menu-bar").gap(4.0).center_y();
    for index in 0..MENUS.len() {
        bar = bar.child(btn(
            MENU_KEYS[index],
            &i18n_wire::tr(&s.core, MENU_LABEL_KEYS[index]),
            s.menus.open == Some(index),
            t,
            move |s: &mut State| s.menus.toggle(index),
        ));
    }
    bar
}

/// The dismissal scrim: a transparent full-window press
/// target under the open dropdown (rune-kit's `scrim`
/// pattern). Mount it as a root-column child immediately
/// before [`menu_dropdown`], and only while a menu is
/// open — the shell keeps a zero-size in-flow spacer in
/// this slot in both states (UI mission Wave 8; see
/// [`menu_dropdown`]). Inert when no menu is open.
pub fn menu_scrim(s: &State) -> El {
    if s.menus.open.is_none() {
        return el();
    }
    el()
        .key("menu-scrim")
        .role("none")
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .on_press(|s: &mut State| s.menus.close())
}

/// The open menu's dropdown surface, anchored under its
/// trigger (below, start-aligned, 4 pt gap; the runtime
/// flips/clamps it at the window edges). Mount it as the
/// last root-column child, after [`menu_scrim`], and
/// only while a menu is open: the shell keeps a
/// zero-size in-flow spacer in this slot in both states,
/// so the root column's gap accounting never depends on
/// the menu state (UI mission Wave 8 — mounted in both
/// states, this layer and the scrim used to trade their
/// in-flow placeholders for absolute positioning on
/// open, freeing two root gaps and growing the stage
/// 24 pt for as long as a menu was up). Inert when no
/// menu is open. Phase N (charter §9) set the
/// surface to radius 12 / padding 6 — on this `col`, not
/// `design::panel_col`, whose 12/8 padding/gap is the
/// panel recipe — and gave it the P15 M2 entrance: a
/// fade plus a 4 pt rise on the zero-bounce SNAP spring
/// pair. Reduced motion (Roadmap Wave 2, W2-53 —
/// verified from the pinned Rune rev 87caf2a): the
/// host Reduce Motion flag gates this pair inside the
/// runtime (`PropClass::reduce` in rune-motion's
/// store) — the 4 pt rise lands at once and the fade
/// becomes Rune's 0.18 s REDUCED_FADE, so no call-site
/// gate is needed or wanted here. The `.anchored(...)`
/// placement stays the outermost call on the returned
/// element.
pub fn menu_dropdown(s: &State, t: &Theme) -> El {
    let Some(open) = s.menus.open else {
        return el();
    };
    let core = &s.core;
    let mut list = col()
        .key(format!("{}-dropdown", MENU_KEYS[open]))
        .min_w(190.0)
        .padding(6.0)
        .gap(2.0)
        .radius(12.0)
        .fill(t.card)
        .border(1.0, t.card_border)
        .items_stretch()
        .transition(
            Transition::offset(0.0, -4.0)
                .combined(Transition::opacity())
                .springs(tok::motion::pair::SNAP),
        );
    match open {
        FILE => {
            // Save / Load are NOT mounted here (UI
            // mission Wave 3, audit S04 D4): the app
            // bar's buttons carry the `save` / `load`
            // keys, and mounting them again in this
            // dropdown put each key in the presented
            // tree twice while the menu was open. The
            // Ctrl+S binding still routes to `core.save()`
            // from `main.rs::on_key`, unchanged.
            // Open recent (Phase J): up to five entries
            // from the shell's recents list, resolved
            // against the Home library scan (see
            // `recent_items`). The recents file is read
            // only while this menu is open, and not at
            // all when it is empty — no scan is triggered
            // for a menu with nothing to offer.
            let recents = stykanimate_rs::shell::recents();
            if !recents.is_empty() {
                let files = recent_file_pairs(s);
                let entries = file_recent_items(&recents, &files, t);
                let has_entries = !entries.is_empty();
                for entry in entries {
                    list = list.child(entry);
                }
                // The separator divides the recents from
                // Export PNG (integrator, wave 3): it used
                // to head the recents block from inside
                // `file_recent_items`, back when Save/Load
                // sat above it; with those gone it belongs
                // here, and only when recents rendered.
                if has_entries {
                    list = list.child(separator("menu-file-sep", t));
                }
            }
            list = list.child(item("export", &i18n_wire::tr(core, "menu_export_png"), false, t, |s| {
                s.core.export_png()
            }));
            // No "Open Home" item: the Rune shell has no
            // home view or route — `State` carries no home
            // flag and `EditorCore` has no home op (grep:
            // no `home` in editor.rs) — so there is no real
            // action to wire it to.
        }
        EDIT => {
            // Undo / Redo are NOT mounted here (UI
            // mission Wave 3, audit S04 D4): the app
            // bar's buttons carry the `undo` / `redo`
            // keys — depth-gated inert there while their
            // stacks are empty (D2) — and mounting them
            // again in this dropdown put each key in
            // the presented tree twice while the menu
            // was open. Ctrl+Z / Ctrl+Shift+Z still
            // route to the same ops from
            // `main.rs::on_key`, unchanged. The
            // separator that divided Undo/Redo from the
            // frame-clipboard group is REMOVED with them
            // (integrator, wave 3): a divider heading a
            // dropdown divides nothing. `menu-edit-sep`
            // leaves the key census as a recorded
            // consequence of the dedup, and the snap test
            // now pins its absence.
            list = list
                .child(item("copy-frame", &i18n_wire::tr(core, "menu_copy_frame"), false, t, |s| {
                    s.core.copy_frame()
                }))
                .child(item("paste-frame", &i18n_wire::tr(core, "menu_paste_frame"), false, t, |s| {
                    s.core.paste_frame()
                }));
            // Selection deletes (Phase L): the Delete
            // key's first two branches as menu items, so
            // the removals are discoverable without the
            // keyboard. Each item exists only while its
            // selection exists — no inert rows, the
            // Open-recent discipline — and when both
            // selections are live both items appear,
            // stroke first, matching the key's priority.
            // The handlers are the key's branches
            // verbatim (see `delete_selected_stroke` /
            // `delete_selected_particle`); the shared
            // `item` wrapper closes the menu on select,
            // as for every item. Neither item carries a
            // shortcut hint — see `shortcut_hint` for why
            // the Delete key does not count as their
            // binding.
            if s.stroke_selected.is_some() {
                list = list.child(item(
                    "edit-delete-stroke",
                    &i18n_wire::tr(core, "menu_delete_stroke"),
                    false,
                    t,
                    delete_selected_stroke,
                ));
            }
            if s.particle_selected.is_some() {
                list = list.child(item(
                    "edit-delete-particle",
                    &i18n_wire::tr(core, "menu_delete_particle"),
                    false,
                    t,
                    delete_selected_particle,
                ));
            }
            // Frame swap + clip mute (Phase M): two more
            // conditional items, after the deletes, on
            // the same discipline — each exists only
            // while its action can run, no inert rows.
            // Swap needs a next frame: on the last frame
            // (or an empty timeline) there is nothing to
            // trade with. Mute needs the Audio panel's
            // effective target — its live selection, or
            // the last clip as fallback; with no clips
            // there is nothing to mute. Neither carries
            // a shortcut hint — see `shortcut_hint`.
            if s.core.current + 1 < s.core.project.frames.len() {
                list = list.child(item(
                    "edit-swap-next",
                    &i18n_wire::tr(core, "menu_swap_next"),
                    false,
                    t,
                    swap_frame_with_next,
                ));
            }
            if let Some(target) = crate::rune_sec::audio_panel::effective_target(
                s.audio_panel.selected,
                s.core.project.audio.len(),
            ) {
                // `item_icon` directly, not `item`: the
                // glyph depends on the targeted clip's
                // muted state, which the static
                // `menu_icon` map cannot express. The
                // hint is `shortcut_hint`'s None and the
                // handler is wrapped in `closing`, exactly
                // as `item` would build it.
                let (label, icon) = mute_item_parts(&s.core, s.core.project.audio[target].muted);
                list = list.child(item_icon(
                    "edit-mute-clip",
                    icon,
                    &label,
                    false,
                    shortcut_hint("edit-mute-clip"),
                    t,
                    closing(toggle_targeted_clip_muted),
                ));
            }
        }
        VIEW => {
            // Core view state/ops: `core.onion` is a public
            // field (no op exists — the app bar's Onion
            // button flips it too), and the camera-framing
            // ops from the GPUI View buttons. Stage guides
            // (Phase F) is shell state, not core: the flag
            // is `State::guides` and its single writer is
            // the settings section's `toggle_guides` (the
            // Settings card's toggle) — this item calls
            // that same fn rather than growing a second
            // toggle, and renders active while the flag is
            // set. Camera frame / Safe frames (Phase G) are
            // Stage guides' two child toggles, built the
            // same way directly under it: their flags are
            // `State::guides_frustum` / `State::guides_safe`
            // (both default on; the stage draws those
            // overlays only while Guides is also on), their
            // writers are the settings section's
            // `toggle_guides_frustum` / `toggle_guides_safe`
            // (the Settings card's child rows call the same
            // fns), and each item renders active while its
            // flag is set. Neither child has a shell key
            // binding, so neither gets a shortcut hint (see
            // `shortcut_hint`). The two sidebar toggles are Phase E:
            // they flip the shell's own layout fields
            // (`State::show_left` / `show_right`,
            // integrator-owned, default visible) and render
            // active while their column is shown — the same
            // field-flip pattern as Onion, one level up
            // (shell state, not core). Playback speed
            // (Phase K) sits directly under the guides
            // children, before the separator: an ACTION
            // item, not a toggle — each press cycles the
            // shell's `State::playback_speed` factor
            // (0.25× … 2×) through the playback section's
            // `cycle_playback_speed`, the exact fn the
            // transport row's `pb-speed` button calls, so
            // it renders inactive like Fit view and carries
            // no shortcut hint (the shell binds no key to
            // it — see `shortcut_hint`). Its gauge glyph
            // comes from `menu_icon`.
            list = list
                .child(item("onion", &i18n_wire::tr(core, "menu_onion_skin"), core.onion, t, |s| {
                    s.core.onion = !s.core.onion;
                }))
                // Motion path (Roadmap Wave 1, RM16 — P-4):
                // the same field-flip pattern as Onion, one
                // row down — `core.show_paths` is a public
                // core field (no op exists), and the stage's
                // motion-path pass reads it directly. No
                // shell key is bound to it, so it gets no
                // shortcut hint (see `shortcut_hint`).
                .child(item("view-paths", &i18n_wire::tr(core, "menu_motion_path"), core.show_paths, t, |s| {
                    s.core.show_paths = !s.core.show_paths;
                }))
                .child(item("view-guides", &i18n_wire::tr(core, "menu_stage_guides"), s.guides, t, |s| {
                    crate::rune_sec::settings_popover::toggle_guides(s);
                }))
                .child(item(
                    "view-guides-frustum",
                    &i18n_wire::tr(core, "menu_camera_frame"),
                    s.guides_frustum,
                    t,
                    |s| {
                        crate::rune_sec::settings_popover::toggle_guides_frustum(s);
                    },
                ))
                .child(item(
                    "view-guides-safe",
                    &i18n_wire::tr(core, "menu_safe_frames"),
                    s.guides_safe,
                    t,
                    |s| {
                        crate::rune_sec::settings_popover::toggle_guides_safe(s);
                    },
                ))
                .child(item("view-speed", &i18n_wire::tr(core, "menu_playback_speed"), false, t, |s| {
                    crate::rune_sec::playback_controls::cycle_playback_speed(s);
                }))
                .child(separator("menu-view-sep", t))
                .child(item("view-fit", &i18n_wire::tr(core, "menu_fit_view"), false, t, |s| {
                    s.core.frame_all_figures()
                }))
                .child(item("view-centre", &i18n_wire::tr(core, "menu_centre_selection"), false, t, |s| {
                    s.core.centre_on_selection()
                }))
                .child(separator("menu-view-sep2", t))
                .child(item(
                    "view-toggle-left",
                    &i18n_wire::tr(core, "menu_figures_sidebar"),
                    s.show_left,
                    t,
                    |s| {
                        s.show_left = !s.show_left;
                    },
                ))
                .child(item(
                    "view-toggle-right",
                    &i18n_wire::tr(core, "menu_inspector_sidebar"),
                    s.show_right,
                    t,
                    |s| {
                        s.show_right = !s.show_right;
                    },
                ))
                .child(separator("menu-view-sep3", t))
                .child(item(
                    "view-stage-focus",
                    &i18n_wire::tr(core, "menu_stage_focus"),
                    s.stage_focus_restore.is_some(),
                    t,
                    |s| {
                        crate::rune_sec::workspaces::toggle_stage_focus(s);
                    },
                ));
        }
        _ => {
            // HELP — "Shortcuts" flips the flag the
            // integrator's help panel renders from.
            // "Command palette" (Phase E) calls the
            // palette section's open fn — the menu is the
            // discoverable door next to the Ctrl+K handler
            // the integrator routes in `on_key`. It is an
            // action item (no active state), and the shared
            // `item` wrapper closes this menu after opening
            // the palette, as for every other item.
            list = list
                .child(item("help", &i18n_wire::tr(core, "menu_shortcuts"), s.menus.help_open, t, |s| {
                    s.menus.help_open = !s.menus.help_open;
                }))
                .child(separator("menu-help-sep", t))
                .child(item("menu-palette", &i18n_wire::tr(core, "menu_command_palette"), false, t, |s| {
                    crate::rune_sec::command_palette::open_palette(s);
                }));
        }
    }
    list.anchored(MENU_KEYS[open], Side::Bottom, Align::Start, 4.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menus_state_defaults_closed() {
        let m = MenusState::default();
        assert_eq!(m.open, None);
        assert!(!m.help_open);
    }

    #[test]
    fn toggle_opens_closes_and_switches() {
        let mut m = MenusState::default();
        m.toggle(FILE);
        assert_eq!(m.open, Some(FILE));
        m.toggle(EDIT); // switch without closing first
        assert_eq!(m.open, Some(EDIT));
        m.toggle(EDIT); // same trigger again closes
        assert_eq!(m.open, None);
    }

    #[test]
    fn close_clears_open_but_keeps_help_flag() {
        let mut m = MenusState {
            open: Some(VIEW),
            help_open: true,
        };
        m.close();
        assert_eq!(m.open, None);
        assert!(m.help_open);
    }

    #[test]
    fn menu_tables_line_up() {
        assert_eq!(MENUS, ["File", "Edit", "View", "Help"]);
        assert_eq!(
            MENU_KEYS,
            ["menu-file", "menu-edit", "menu-view", "menu-help"]
        );
        assert_eq!(
            MENU_LABEL_KEYS,
            ["menu_file", "menu_edit", "menu_view", "menu_help"]
        );
        assert_eq!(MENUS.len(), MENU_KEYS.len());
        assert_eq!(MENUS.len(), MENU_LABEL_KEYS.len());
    }

    #[test]
    fn menus_i18n_keys_exist_and_match_the_english_labels() {
        // Wave U (U3): every menu-bar and dropdown label
        // resolves through i18n_wire::tr under a `menu_*`
        // key. This pin passes only once IA-U3-1's keys
        // land in src/i18n.rs at integration — until then
        // tr falls back to the key itself (the Q7 pattern)
        // and both asserts fail by design.
        let core = stykanimate_rs::editor::EditorCore::new();
        for (key, english) in [
            ("menu_file", "File"),
            ("menu_edit", "Edit"),
            ("menu_view", "View"),
            ("menu_help", "Help"),
            ("menu_export_png", "Export PNG"),
            ("menu_copy_frame", "Copy frame"),
            ("menu_paste_frame", "Paste frame"),
            ("menu_delete_stroke", "Delete stroke"),
            ("menu_delete_particle", "Delete particle event"),
            ("menu_swap_next", "Swap frame with next"),
            ("menu_mute_clip", "Mute audio clip"),
            ("menu_unmute_clip", "Unmute audio clip"),
            ("menu_onion_skin", "Onion skin"),
            ("menu_motion_path", "Motion path"),
            ("menu_stage_guides", "Stage guides"),
            ("menu_camera_frame", "Camera frame"),
            ("menu_safe_frames", "Safe frames"),
            ("menu_playback_speed", "Playback speed"),
            ("menu_fit_view", "Fit view"),
            ("menu_centre_selection", "Centre on selection"),
            ("menu_figures_sidebar", "Figures sidebar"),
            ("menu_inspector_sidebar", "Inspector sidebar"),
            ("menu_shortcuts", "Shortcuts"),
            ("menu_command_palette", "Command palette"),
        ] {
            assert!(i18n_wire::key_in_table(key), "missing i18n key: {key}");
            assert_eq!(i18n_wire::tr(&core, key), english);
        }
    }

    #[test]
    fn shortcut_hints_name_only_real_bindings() {
        // The verified list, from `main.rs::on_key` and
        // `command_palette::palette_handle_key` (see the
        // `shortcut_hint` docs): exactly five menu items
        // have shell key bindings. Save joined the list
        // in Phase K, when `on_key` gained its real
        // Ctrl+S arm (`is_command('s')` → `core.save()`,
        // the Save item's own op) — the hint names the
        // binding only because the binding exists.
        // Roadmap Wave 1 (RM26, PL-4): the Edit menu's
        // Copy/Paste frame pair joined the list — the
        // items call `core.copy_frame()` /
        // `core.paste_frame()`, the exact ops the new
        // Ctrl+C / Ctrl+V chords run (IA-RM26-1). The
        // spellings are read from the keymap registry.
        assert_eq!(shortcut_hint("save"), Some("Ctrl+S"));
        assert_eq!(shortcut_hint("undo"), Some("Ctrl+Z"));
        assert_eq!(shortcut_hint("redo"), Some("Ctrl+Shift+Z"));
        assert_eq!(shortcut_hint("onion"), Some("O"));
        assert_eq!(shortcut_hint("menu-palette"), Some("Ctrl+K"));
        assert_eq!(shortcut_hint("copy-frame"), Some("Ctrl+C"));
        assert_eq!(shortcut_hint("paste-frame"), Some("Ctrl+V"));
        // Every other item key used by `menu_dropdown`
        // has no binding and must stay hint-less — if a
        // future binding is added to the shell, add its
        // entry to `shortcut_hint` and to this list in
        // the same change.
        for key in [
            "load",
            "export",
            "edit-delete-stroke",
            "edit-delete-particle",
            "edit-swap-next",
            "edit-mute-clip",
            "view-guides",
            "view-paths",
            "view-guides-frustum",
            "view-guides-safe",
            "view-speed",
            "view-fit",
            "view-centre",
            "view-toggle-left",
            "view-toggle-right",
            "help",
        ] {
            assert_eq!(shortcut_hint(key), None, "{key} has no shell binding");
        }
    }

    #[test]
    fn menu_icons_name_real_glyphs_for_the_high_value_items() {
        // Phase H: the icon map covers exactly the
        // high-value items, and every name it returns
        // must be a glyph the icon set actually carries
        // (checked through the icons section's own
        // lookup — the same one `item` falls back on),
        // or the item would silently render plain.
        for (key, icon) in [
            ("load", "folder-open"),
            ("save", "floppy-disk"),
            ("export", "download"),
            ("undo", "arrow-rotate-left"),
            ("redo", "arrow-rotate-right"),
            ("copy-frame", "copy"),
            ("paste-frame", "clipboard"),
            ("edit-delete-stroke", "trash-can"),
            ("edit-delete-particle", "trash-can"),
            ("edit-swap-next", "arrow-right-arrow-left"),
            ("onion", "eye"),
            ("view-paths", "bezier-curve"),
            ("view-guides", "eye"),
            ("view-guides-frustum", "crosshairs"),
            ("view-guides-safe", "crosshairs"),
            ("view-speed", "gauge"),
            ("view-toggle-left", "list"),
            ("view-toggle-right", "list"),
            ("menu-palette", "magnifying-glass"),
            ("help", "circle-question"),
        ] {
            assert_eq!(menu_icon(key), Some(icon), "{key}");
            assert!(
                crate::rune_sec::icons::has_icon(icon),
                "{icon} must exist in the icon set"
            );
            assert!(
                crate::rune_sec::icon_data::ICON_NAMES.contains(&icon),
                "{icon} must be listed in ICON_NAMES"
            );
        }
        // Items outside the high-value set — and the
        // menu triggers themselves — stay icon-less.
        // Edit's Mute/Unmute item stays out of the map
        // too, by design: its glyph is chosen from the
        // targeted clip's state at build time (see
        // `mute_item_parts`), not from this static map.
        for key in ["view-fit", "view-centre", "menu-file", "edit-mute-clip", "bogus"] {
            assert_eq!(menu_icon(key), None, "{key} stays plain");
        }
    }

    #[test]
    fn all_dropdowns_build_with_hinted_items_and_guides_states() {
        // Phase F: hinted items (View ▸ Onion skin,
        // Help ▸ Command palette — Edit ▸ Undo/Redo
        // carried hints too until UI mission Wave 3
        // unmounted them for the app bar) and the
        // View ▸ Stage guides item build without
        // panicking, with the guides flag off and on.
        let t = rune::tok::theme(true);
        let mut s = crate::init();
        assert!(!s.guides, "stage guides default to off");
        for menu in [FILE, EDIT, VIEW, HELP] {
            s.menus.open = Some(menu);
            let _ = menu_dropdown(&s, &t);
        }
        crate::rune_sec::settings_popover::toggle_guides(&mut s);
        assert!(s.guides);
        s.menus.open = Some(VIEW);
        let _ = menu_dropdown(&s, &t);
    }

    #[test]
    fn view_dropdown_builds_in_all_guides_flag_states() {
        // Phase G: the View dropdown carries Stage guides'
        // two child items (Camera frame / Safe frames),
        // which read `guides_frustum` / `guides_safe` for
        // their active state. Both flags default to on
        // (integrator seed in `init`), unlike `guides`
        // itself. Build the dropdown in the default state,
        // with each child flipped off via the settings
        // section's toggles (the same fns the menu items
        // call), and with Guides itself on — construction
        // must not panic in any combination.
        let t = rune::tok::theme(true);
        let mut s = crate::init();
        assert!(!s.guides, "stage guides default to off");
        assert!(s.guides_frustum, "camera frame defaults to on");
        assert!(s.guides_safe, "safe frames default to on");
        s.menus.open = Some(VIEW);
        let _ = menu_dropdown(&s, &t);
        crate::rune_sec::settings_popover::toggle_guides_frustum(&mut s);
        assert!(!s.guides_frustum);
        let _ = menu_dropdown(&s, &t);
        crate::rune_sec::settings_popover::toggle_guides_safe(&mut s);
        assert!(!s.guides_safe);
        let _ = menu_dropdown(&s, &t);
        crate::rune_sec::settings_popover::toggle_guides(&mut s);
        assert!(s.guides);
        let _ = menu_dropdown(&s, &t);
    }

    #[test]
    fn view_and_help_dropdowns_build_in_both_sidebar_states() {
        // Phase E: the View dropdown reads the shell's
        // `show_left` / `show_right` fields for the sidebar
        // toggles' active state, and the Help dropdown
        // carries the Command palette item. Build both
        // dropdowns with the sidebars shown and hidden —
        // construction must not panic in either state, and
        // the integrator contract is that both fields
        // default to visible.
        let t = rune::tok::theme(true);
        let mut s = crate::init();
        assert!(s.show_left, "left sidebar defaults to visible");
        assert!(s.show_right, "right sidebar defaults to visible");
        s.menus.open = Some(VIEW);
        let _ = menu_dropdown(&s, &t);
        s.show_left = false;
        s.show_right = false;
        let _ = menu_dropdown(&s, &t);
        s.menus.open = Some(HELP);
        let _ = menu_dropdown(&s, &t);
    }

    #[test]
    fn view_menu_carries_the_playback_speed_item() {
        // Phase K: View ▸ Playback speed (`view-speed`)
        // sits under the guides children in the VIEW arm
        // of `menu_dropdown`. Rune's `El` has no public
        // tree-walk, so — in the Phase J menu-structure
        // tests' style — this file pins the item through
        // the pure maps that decide its whole rendering
        // (`item` dispatches on exactly these two) and
        // exercises the VIEW builder that constructs it;
        // presented-tree proof is the Runtime suite's
        // pattern (snap_tests), not a unit test's.
        assert_eq!(menu_icon("view-speed"), Some("gauge"));
        assert!(crate::rune_sec::icons::has_icon("gauge"));
        assert!(crate::rune_sec::icon_data::ICON_NAMES.contains(&"gauge"));
        // No shell key cycles the speed, so no hint — a
        // hint here would be a lie on screen.
        assert_eq!(shortcut_hint("view-speed"), None);
        // The builder constructs the item in the shell's
        // default state…
        let t = rune::tok::theme(true);
        let mut s = crate::init();
        assert_eq!(s.playback_speed, 1.0, "speed factor defaults to normal");
        s.menus.open = Some(VIEW);
        let _ = menu_dropdown(&s, &t);
        // …and the fn its press calls really cycles the
        // factor: one call moves normal speed to the next
        // step of the playback section's table (1.0 →
        // 1.5), the cross-section contract this item is
        // wired against.
        crate::rune_sec::playback_controls::cycle_playback_speed(&mut s);
        assert_eq!(s.playback_speed, 1.5);
    }

    /// (name, path) pairs in the shape `recent_items`
    /// resolves against, for the Phase J tests below.
    fn recent_pairs(names: &[&str]) -> Vec<(String, std::path::PathBuf)> {
        names
            .iter()
            .map(|n| {
                (
                    n.to_string(),
                    std::path::PathBuf::from(format!("projects/{n}.json")),
                )
            })
            .collect()
    }

    #[test]
    fn recent_items_caps_at_five_in_recents_order() {
        // Phase J: the menu shows at most five recents,
        // most recent first — the recents order is kept,
        // not the file list's order (the pairs are
        // deliberately reversed here).
        let recents: Vec<String> = ["One", "Two", "Three", "Four", "Five", "Six", "Seven"]
            .iter()
            .map(|n| n.to_string())
            .collect();
        let mut files = recent_pairs(&["One", "Two", "Three", "Four", "Five", "Six", "Seven"]);
        files.reverse();
        let items = recent_items(&recents, &files);
        assert_eq!(items.len(), MAX_RECENT_ITEMS);
        let names: Vec<&str> = items.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["One", "Two", "Three", "Four", "Five"]);
        assert!(items.iter().all(|(_, p)| p.is_some()));
    }

    #[test]
    fn recent_items_resolves_exact_names_and_marks_missing_files() {
        // Phase J: an exact stored-name match resolves to
        // that file's path; a recent whose file is gone
        // stays in the list with `None` so its press can
        // report the not-found status; matching is exact
        // (case-sensitive), and files no recent names are
        // ignored.
        let recents: Vec<String> = ["Alpha", "Gone", "Beta"]
            .iter()
            .map(|n| n.to_string())
            .collect();
        let files = recent_pairs(&["Beta", "Alpha", "Other"]);
        let items = recent_items(&recents, &files);
        assert_eq!(
            items,
            vec![
                (
                    "Alpha".to_string(),
                    Some(std::path::PathBuf::from("projects/Alpha.json")),
                ),
                ("Gone".to_string(), None),
                (
                    "Beta".to_string(),
                    Some(std::path::PathBuf::from("projects/Beta.json")),
                ),
            ]
        );
        let lower: Vec<String> = vec!["alpha".to_string()];
        assert_eq!(
            recent_items(&lower, &files),
            vec![("alpha".to_string(), None)],
            "a differently-cased name must not resolve"
        );
    }

    #[test]
    fn recent_items_resolves_a_renamed_recent_by_file_stem() {
        // Wave 12 (D7): the Home-rename case. The rename
        // is file-only — the file moved to the typed
        // name's slug while the name stored inside it is
        // still the old one — and `confirm_rename`
        // re-touched recents with the typed name, so no
        // stored name matches the entry. Home ▸ Reopen
        // last resolves it by file stem
        // (`home_panel::reopen_target`'s second pass);
        // the menu must resolve the same entry to the
        // same file instead of reporting it not found.
        let recents: Vec<String> = vec!["New Name".to_string()];
        let files = vec![(
            "Old Name".to_string(),
            std::path::PathBuf::from("projects/new-name.json"),
        )];
        assert_eq!(
            recent_items(&recents, &files),
            vec![(
                "New Name".to_string(),
                Some(std::path::PathBuf::from("projects/new-name.json")),
            )]
        );
        // A name whose slug matches no stem stays
        // unresolved — the fallback invents nothing.
        let other: Vec<String> = vec!["Something Else".to_string()];
        assert_eq!(
            recent_items(&other, &files),
            vec![("Something Else".to_string(), None)]
        );
    }

    #[test]
    fn recent_items_exact_match_beats_the_stem_fallback() {
        // Wave 12 (D7): precedence, mirroring
        // `reopen_target` — when a stored name matches
        // exactly, that file wins even if another file's
        // stem would match the name's slug and the stem
        // candidate sits earlier in the list.
        let recents: Vec<String> = vec!["Foo Bar".to_string()];
        let files = vec![
            (
                "Unrelated".to_string(),
                std::path::PathBuf::from("projects/foo-bar.json"),
            ),
            (
                "Foo Bar".to_string(),
                std::path::PathBuf::from("projects/kept.json"),
            ),
        ];
        assert_eq!(
            recent_items(&recents, &files),
            vec![(
                "Foo Bar".to_string(),
                Some(std::path::PathBuf::from("projects/kept.json")),
            )]
        );
    }

    #[test]
    fn recent_items_with_no_recents_is_empty() {
        assert!(recent_items(&[], &recent_pairs(&["Alpha"])).is_empty());
    }

    #[test]
    fn file_recent_items_build_nothing_without_recents_and_a_block_with_recents() {
        // Phase J: no recents → no block at all; two
        // recents → two items, resolved and unresolved
        // alike. UI mission wave 3: the separator that
        // used to head this block moved to the File arm,
        // between the recents and Export PNG — heading
        // the menu, it divided nothing.
        let t = rune::tok::theme(true);
        assert!(file_recent_items(&[], &[], &t).is_empty());
        let recents: Vec<String> = ["Alpha", "Gone"]
            .iter()
            .map(|n| n.to_string())
            .collect();
        let files = recent_pairs(&["Alpha"]);
        let block = file_recent_items(&recents, &files, &t);
        assert_eq!(block.len(), 2, "two recent items, separator lives in the File arm");
    }

    #[test]
    fn recent_dimming_follows_resolution() {
        // Wave 13 (D5): the dimmed branch is taken by
        // exactly the entries whose file did not
        // resolve — no resolved entry dims, every
        // missing one does. Pinned on the pure pair
        // `file_recent_items` dispatches on
        // (`recent_items` + `recent_is_dimmed`), the
        // Phase J/L style: Rune's `El` exposes no
        // tree-walk to read the built rows back.
        let recents: Vec<String> = ["Alpha", "Gone", "Beta"]
            .iter()
            .map(|n| n.to_string())
            .collect();
        let files = recent_pairs(&["Alpha", "Beta"]);
        let resolved = recent_items(&recents, &files);
        let dimmed: Vec<bool> = resolved
            .iter()
            .map(|(_, path)| recent_is_dimmed(path))
            .collect();
        assert_eq!(dimmed, vec![false, true, false]);

        // All resolved → nothing dims; none resolved →
        // everything dims (the entries still list).
        let all_found = recent_items(&recents[..1], &files);
        assert!(all_found.iter().all(|(_, p)| !recent_is_dimmed(p)));
        let none_found = recent_items(&recents, &[]);
        assert_eq!(none_found.len(), 3);
        assert!(none_found.iter().all(|(_, p)| recent_is_dimmed(p)));
    }

    #[test]
    fn missing_recent_items_stay_listed_and_build_dimmed() {
        // Wave 13 (D5): a mixed block still lists BOTH
        // entries (the missing one is dimmed, not
        // dropped), and both builders construct under
        // the same `file-recent-{i}` key family — the
        // resolved path through `item` (unchanged) and
        // the missing path through `dimmed_item`, in
        // its glyph branch (recents map to the clock)
        // and its empty-slot fallback (an unmapped key).
        let t = rune::tok::theme(true);
        let recents: Vec<String> = ["Alpha", "Gone"]
            .iter()
            .map(|n| n.to_string())
            .collect();
        let files = recent_pairs(&["Alpha"]);
        let block = file_recent_items(&recents, &files, &t);
        assert_eq!(block.len(), 2, "resolved and missing recents both list");

        let _ = item("file-recent-0", "Alpha", false, &t, |_| {});
        let _ = dimmed_item("file-recent-1", "Gone", &t, |_| {});
        let _ = dimmed_item("unmapped-key", "Gone", &t, |_| {});

        // The dimmed ink is the secondary tone
        // (RM25 / PL-1: the muted role was retired as
        // this label's ink — real text under every
        // text bar), visibly distinct from the live
        // label colour; the row's dimmed read comes
        // from its absent hover/press light-up.
        assert_ne!(
            t.text_secondary, t.text,
            "dimmed ink must differ from live ink"
        );

        // The dimmed entry's press is the existing
        // honest refusal, not a silent no-op: the
        // handler is `open_recent` with no file, whose
        // status and no-side-effects contract is pinned
        // by `open_recent_without_a_file_reports_not_
        // found_and_changes_nothing` below — the
        // chrome_view muted-live rule (a kept handler
        // only where a test pins its refusal status).
    }

    #[test]
    fn recent_entries_carry_the_clock_glyph_and_no_hint() {
        // Phase J: the dynamic recent keys resolve their
        // glyph by prefix, the glyph exists in the icon
        // set, and no recent entry claims a shortcut hint
        // (the shell binds no key to opening a recent).
        assert_eq!(menu_icon("file-recent-0"), Some("clock"));
        assert_eq!(menu_icon("file-recent-4"), Some("clock"));
        assert!(crate::rune_sec::icons::has_icon("clock"));
        assert_eq!(shortcut_hint("file-recent-0"), None);
    }

    #[test]
    fn open_recent_without_a_file_reports_not_found_and_changes_nothing() {
        // Phase J: pressing a recent whose file vanished
        // sets the not-found status and leaves the open
        // project, the file list and the selector index
        // exactly as they were.
        let mut s = crate::init();
        let name_before = s.core.project.name.clone();
        let files_before = s.core.project_files.len();
        let index_before = s.core.project_index;
        open_recent(&mut s, "Gone", None);
        assert_eq!(s.core.status, "Recent project not found: Gone");
        assert_eq!(s.core.project.name, name_before);
        assert_eq!(s.core.project_files.len(), files_before);
        assert_eq!(s.core.project_index, index_before);
    }

    #[test]
    fn edit_menu_delete_items_follow_the_selections() {
        // Phase L: Edit ▸ Delete stroke / Delete particle
        // event exist only while their selection exists.
        // Rune's `El` exposes no public tree-walk, so — in
        // the Phase J menu-structure style — presence is
        // pinned through the pure maps `item` dispatches
        // on, plus a build of the EDIT dropdown in each
        // selection state; the handlers are exercised
        // directly (they are the fns the items call).
        use stykanimate_rs::model::Stroke;

        // Both items lead with the trash can, which
        // exists in the curated set, and neither claims a
        // hint: the Delete key is bound to `on_key`'s
        // priority cascade, not to either item (see
        // `shortcut_hint`).
        for key in ["edit-delete-stroke", "edit-delete-particle"] {
            assert_eq!(menu_icon(key), Some("trash-can"), "{key}");
            assert_eq!(shortcut_hint(key), None, "{key}");
        }
        assert!(crate::rune_sec::icons::has_icon("trash-can"));
        assert!(crate::rune_sec::icon_data::ICON_NAMES.contains(&"trash-can"));

        let t = rune::tok::theme(true);
        let mut s = crate::init();
        // The demo project starts with no strokes and no
        // particle events, and nothing selected: the EDIT
        // dropdown builds with neither delete item.
        assert!(s.core.project.strokes.is_empty());
        assert!(s.core.project.particles.is_empty());
        assert_eq!(s.stroke_selected, None);
        assert_eq!(s.particle_selected, None);
        s.menus.open = Some(EDIT);
        let _ = menu_dropdown(&s, &t);

        // Plant one stroke and one particle event and
        // select both: the dropdown builds with both
        // items offered (their rendering is fully
        // decided by the maps pinned above).
        s.core.project.strokes.push(Stroke {
            frame: 0,
            points: vec![[0.0, 0.0], [10.0, 10.0]],
            color: 0xFFFFFF,
            width: 2.0,
            widths: Vec::new(),
            fill: None,
            spray: None,
            opacity: 1.0,
            scatter: 0.0,
        });
        let ev = s.core.particle_add_event(0, 100.0, 100.0);
        s.stroke_selected = Some(0);
        s.particle_selected = Some(ev);
        let _ = menu_dropdown(&s, &t);

        // The stroke item's press: the stroke is deleted
        // and the selection cleared; the particle event
        // and its selection are untouched — the key's
        // priority, one branch at a time.
        delete_selected_stroke(&mut s);
        assert!(s.core.project.strokes.is_empty());
        assert_eq!(s.stroke_selected, None);
        assert_eq!(s.core.project.particles.len(), 1);
        assert_eq!(s.particle_selected, Some(ev));

        // The particle item's press: the event is
        // deleted and its selection cleared.
        delete_selected_particle(&mut s);
        assert!(s.core.project.particles.is_empty());
        assert_eq!(s.particle_selected, None);

        // With neither selection the dropdown builds
        // with neither item again, and both handlers are
        // clean no-ops.
        let _ = menu_dropdown(&s, &t);
        delete_selected_stroke(&mut s);
        delete_selected_particle(&mut s);
        assert!(s.core.project.strokes.is_empty());
        assert!(s.core.project.particles.is_empty());
    }

    #[test]
    fn edit_menu_swap_and_mute_items_follow_the_state() {
        // Phase M: Edit ▸ Swap frame with next exists
        // only while a next frame exists; Edit ▸ Mute /
        // Unmute audio clip exists only while the Audio
        // panel has an effective target, its label and
        // glyph naming the action for the targeted
        // clip's current state. Presence is pinned in
        // the Phase J/L menu-structure style (see
        // `edit_menu_delete_items_follow_the_selections`):
        // the pure maps and parts fns `menu_dropdown`
        // dispatches on, a dropdown build in each state,
        // and the handlers exercised directly.
        use stykanimate_rs::model::AudioClip;

        // The swap item leads with the two-way arrows,
        // which exist in the curated set; neither new
        // item claims a hint (no shell key is bound to
        // either action alone — see `shortcut_hint`).
        // The mute item is NOT in the static icon map:
        // its glyph is state-dependent, so its parts
        // come from `mute_item_parts`.
        assert_eq!(menu_icon("edit-swap-next"), Some("arrow-right-arrow-left"));
        assert_eq!(shortcut_hint("edit-swap-next"), None);
        assert_eq!(menu_icon("edit-mute-clip"), None);
        assert_eq!(shortcut_hint("edit-mute-clip"), None);
        for icon in ["arrow-right-arrow-left", "volume-xmark", "volume-high"] {
            assert!(crate::rune_sec::icons::has_icon(icon), "{icon}");
            assert!(
                crate::rune_sec::icon_data::ICON_NAMES.contains(&icon),
                "{icon}"
            );
        }
        // The parts name the action, not the state —
        // and reuse the Audio panel's glyph choice. The
        // label resolves through `i18n_wire::tr` (Wave U,
        // U3), so it matches the English below once
        // IA-U3-1's keys land at integration (Q7 pattern).
        assert_eq!(mute_item_parts(&stykanimate_rs::editor::EditorCore::new(), false), ("Mute audio clip".to_string(), "volume-xmark"));
        assert_eq!(mute_item_parts(&stykanimate_rs::editor::EditorCore::new(), true), ("Unmute audio clip".to_string(), "volume-high"));

        let t = rune::tok::theme(true);
        let mut s = crate::init();
        s.menus.open = Some(EDIT);
        // The demo project: 5 frames, playhead on frame
        // 0, no audio clips. The swap item is offered
        // (a next frame exists); the mute item is not
        // (no target) — the dropdown builds either way.
        assert_eq!(s.core.project.frames.len(), 5);
        assert_eq!(s.core.current, 0);
        assert!(s.core.project.audio.is_empty());
        let _ = menu_dropdown(&s, &t);

        // The swap press: frames 0 and 1 trade contents
        // under one snapshot and the playhead index
        // stays. Durations are the visible difference —
        // set them apart first so the trade shows.
        s.core.project.frames[0].duration = 1.0;
        s.core.project.frames[1].duration = 3.0;
        let depth0 = s.core.undo_depth();
        swap_frame_with_next(&mut s);
        assert_eq!(s.core.project.frames[0].duration, 3.0);
        assert_eq!(s.core.project.frames[1].duration, 1.0);
        assert_eq!(s.core.current, 0);
        assert_eq!(s.core.undo_depth(), depth0 + 1);

        // On the last frame the swap item is gone, and
        // the handler is a clean refusal: no snapshot,
        // frames untouched, the op's own status.
        s.core.goto(4);
        let _ = menu_dropdown(&s, &t);
        let depth1 = s.core.undo_depth();
        swap_frame_with_next(&mut s);
        assert_eq!(s.core.undo_depth(), depth1);
        assert!(
            s.core.status.contains("no neighbour to swap with"),
            "status: {}",
            s.core.status
        );

        // Mute: plant two clips with no panel
        // selection — the effective target falls back
        // to the LAST clip, so the item appears and its
        // press mutes clip 1 only.
        s.core.project.audio.push(AudioClip {
            path: "a.wav".into(),
            start_frame: 0,
            gain: 1.0,
            muted: false,
        });
        s.core.project.audio.push(AudioClip {
            path: "b.wav".into(),
            start_frame: 2,
            gain: 1.0,
            muted: false,
        });
        assert_eq!(s.audio_panel.selected, None);
        let _ = menu_dropdown(&s, &t);
        toggle_targeted_clip_muted(&mut s);
        assert!(!s.core.project.audio[0].muted);
        assert!(s.core.project.audio[1].muted);
        // The targeted clip now names the way back.
        assert_eq!(
            mute_item_parts(&s.core, s.core.project.audio[1].muted),
            ("Unmute audio clip".to_string(), "volume-high")
        );

        // A live selection retargets the item: clip 0
        // mutes, clip 1 is untouched.
        s.audio_panel.selected = Some(0);
        let _ = menu_dropdown(&s, &t);
        toggle_targeted_clip_muted(&mut s);
        assert!(s.core.project.audio[0].muted);
        assert!(s.core.project.audio[1].muted);

        // No clips: the dropdown builds with no mute
        // item and the handler is a clean no-op.
        s.core.project.audio.clear();
        s.audio_panel.selected = None;
        let _ = menu_dropdown(&s, &t);
        toggle_targeted_clip_muted(&mut s);
        assert!(s.core.project.audio.is_empty());
    }
}
