//! File / Edit / View / Help menus for the Rune editor shell
//! (Phase D, section 22 — menus; Phase E upgrade: View ▸
//! sidebar toggles and Help ▸ Command palette; Phase F:
//! right-aligned shortcut hints on the items that have
//! real shell key bindings, and View ▸ Stage guides;
//! Phase G: View ▸ Camera frame / Safe frames, the two
//! child toggles under Stage guides; Phase H: leading
//! Font Awesome glyphs on the high-value items — see
//! [`menu_icon`] — with keys, labels, active behaviour
//! and handlers unchanged).
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
//! // under the app bar, and append as the LAST root-column
//! // children, in this order:
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

use rune::tok::{self, type_, Theme};
use rune::{col, el, row, text, Align, Color, El, Side};

use crate::chrome_view::btn;
use crate::State;

/// Menu indices in [`MenusState::open`] — the same numbering
/// the GPUI app's `open_menu` used: File, Edit, View, Help.
pub const FILE: usize = 0;
pub const EDIT: usize = 1;
pub const VIEW: usize = 2;
pub const HELP: usize = 3;

/// Trigger labels, in index order.
pub const MENUS: [&str; 4] = ["File", "Edit", "View", "Help"];

/// Trigger element keys, in index order (the Wave 15 menu
/// ids). The dropdown for a menu anchors to the element
/// carrying its trigger's key.
pub const MENU_KEYS: [&str; 4] = ["menu-file", "menu-edit", "menu-view", "menu-help"];

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
/// Everything else in these menus — Save, Load, Export
/// PNG, Copy/Paste frame, Fit view, Centre on selection,
/// the sidebar toggles, Stage guides, Camera frame, Safe
/// frames, Shortcuts — has NO
/// shell key binding and gets no hint: an invented hint
/// would be a lie on screen. The spellings match the
/// help panel's shortcut table (`help_panel.rs`). Pinned
/// by the `shortcut_hints_name_only_real_bindings` test.
fn shortcut_hint(key: &str) -> Option<&'static str> {
    match key {
        "undo" => Some("Ctrl+Z"),
        "redo" => Some("Ctrl+Shift+Z"),
        "onion" => Some("O"),
        "menu-palette" => Some("Ctrl+K"),
        _ => None,
    }
}

/// A menu item carrying a shortcut hint: the same chrome
/// as `chrome_view::btn` (10/5 padding, radius 6, the
/// same fill/hover/press states and label font), rebuilt
/// as a row so the hint rides inside the same pressable
/// element — the item's key, press handler and hit area
/// cover the whole row, hint included, like a native
/// menu. The hint is inert decoration in the theme's
/// secondary text colour (75% white over the accent fill
/// while the item is active, mirroring the white label).
fn hinted_item(
    key: &str,
    label: &str,
    hint: &str,
    active: bool,
    t: &Theme,
    f: impl Fn(&mut State) + 'static,
) -> El {
    let accent = tok::accent(0, true);
    row()
        .key(key)
        .px(10.0)
        .py(5.0)
        .radius(6.0)
        .fill(if active { accent } else { t.card })
        .hover(if active { accent } else { t.control_track })
        .press(if active { accent } else { t.control_off })
        .on_press(f)
        .center_y()
        .gap(12.0)
        .child(
            text(label)
                .font(type_::LABEL)
                .color(if active { Color::WHITE } else { t.text }),
        )
        .child(row().grow(1.0))
        .child(
            text(hint).font(type_::LABEL).color(if active {
                Color::WHITE.alpha(0.75)
            } else {
                t.text_secondary
            }),
        )
}

/// The Font Awesome glyph an item leads with (Phase H),
/// keyed by the item's element key — or `None` when the
/// item stays plain text. Only the high-value items are
/// mapped: the File actions, Edit's undo/redo and frame
/// clipboard, View's visibility toggles and sidebar
/// switches, and Help's two entries. Fit view / Centre on
/// selection stay unmapped (no glyph reads as clearly,
/// and an unclear icon is worse than none).
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
    match key {
        // File: open / save / export out.
        "load" => Some("folder-open"),
        "save" => Some("floppy-disk"),
        "export" => Some("download"),
        // Edit: undo / redo, copy / paste frame.
        "undo" => Some("arrow-rotate-left"),
        "redo" => Some("arrow-rotate-right"),
        "copy-frame" => Some("copy"),
        "paste-frame" => Some("clipboard"),
        // View: visibility (eye), the guides sub-toggles
        // (crosshairs), the sidebar switches (list).
        "onion" => Some("eye"),
        "view-guides" => Some("eye"),
        "view-guides-frustum" => Some("crosshairs"),
        "view-guides-safe" => Some("crosshairs"),
        "view-toggle-left" => Some("list"),
        "view-toggle-right" => Some("list"),
        // Help: palette search, shortcuts question.
        "menu-palette" => Some("magnifying-glass"),
        "help" => Some("circle-question"),
        _ => None,
    }
}

/// A menu item leading with a Font Awesome glyph: the
/// same row chrome as [`hinted_item`] (10/5 padding,
/// radius 6, the same fill/hover/press states), with
/// the glyph at 12 pt in the label's own colour
/// (white while active, theme text otherwise) ahead
/// of the label, and the shortcut hint — for the items
/// that have one — at the right edge exactly as
/// `hinted_item` renders it. Key, press handler and
/// hit area cover the whole row, icon included.
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
        .px(10.0)
        .py(5.0)
        .radius(6.0)
        .fill(if active { accent } else { t.card })
        .hover(if active { accent } else { t.control_track })
        .press(if active { accent } else { t.control_off })
        .on_press(f)
        .center_y()
        .gap(12.0)
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(crate::rune_sec::icons::icon_el(icon, 12.0, label_color))
                .child(text(label).font(type_::LABEL).color(label_color)),
        )
        .child(row().grow(1.0));
    if let Some(hint) = hint {
        r = r.child(text(hint).font(type_::LABEL).color(if active {
            Color::WHITE.alpha(0.75)
        } else {
            t.text_secondary
        }));
    }
    r
}

/// One menu item: a chrome button that runs `f` and then
/// closes the menu (see [`closing`]). Items in the
/// [`menu_icon`] map lead with their glyph (see
/// [`item_icon`]); items whose action has a real shell
/// key binding (see [`shortcut_hint`]) render it as a
/// muted hint at the row's right edge; the rest are
/// plain `btn`s, exactly as before.
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
            None => btn(key, label, active, t, closing(f)),
        },
    }
}

/// A thin divider line between item groups.
fn separator(key: &str, t: &Theme) -> El {
    el().key(key).h(1.0).fill(t.separator)
}

/// The menu bar: one trigger button per menu, keyed with
/// the Wave 15 menu ids; a trigger renders active while its
/// menu is open.
pub fn menu_bar(s: &State, t: &Theme) -> El {
    let mut bar = row().key("menu-bar").gap(4.0).center_y();
    for index in 0..MENUS.len() {
        bar = bar.child(btn(
            MENU_KEYS[index],
            MENUS[index],
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
/// before [`menu_dropdown`]. Inert when no menu is open.
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
/// last root-column child, after [`menu_scrim`]. Inert
/// when no menu is open.
pub fn menu_dropdown(s: &State, t: &Theme) -> El {
    let Some(open) = s.menus.open else {
        return el();
    };
    let core = &s.core;
    let mut list = col()
        .key(format!("{}-dropdown", MENU_KEYS[open]))
        .min_w(190.0)
        .padding(4.0)
        .gap(2.0)
        .radius(8.0)
        .fill(t.card)
        .border(1.0, t.card_border)
        .items_stretch();
    match open {
        FILE => {
            list = list
                .child(item("save", "Save", false, t, |s| s.core.save()))
                .child(item("load", "Load", false, t, |s| s.core.load()))
                .child(item("export", "Export PNG", false, t, |s| {
                    s.core.export_png()
                }));
            // No "Open Home" item: the Rune shell has no
            // home view or route — `State` carries no home
            // flag and `EditorCore` has no home op (grep:
            // no `home` in editor.rs) — so there is no real
            // action to wire it to.
        }
        EDIT => {
            list = list
                .child(item("undo", "Undo", false, t, |s| s.core.undo()))
                .child(item("redo", "Redo", false, t, |s| s.core.redo()))
                .child(separator("menu-edit-sep", t))
                .child(item("copy-frame", "Copy frame", false, t, |s| {
                    s.core.copy_frame()
                }))
                .child(item("paste-frame", "Paste frame", false, t, |s| {
                    s.core.paste_frame()
                }));
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
            // (shell state, not core).
            list = list
                .child(item("onion", "Onion skin", core.onion, t, |s| {
                    s.core.onion = !s.core.onion;
                }))
                .child(item("view-guides", "Stage guides", s.guides, t, |s| {
                    crate::rune_sec::settings_popover::toggle_guides(s);
                }))
                .child(item(
                    "view-guides-frustum",
                    "Camera frame",
                    s.guides_frustum,
                    t,
                    |s| {
                        crate::rune_sec::settings_popover::toggle_guides_frustum(s);
                    },
                ))
                .child(item(
                    "view-guides-safe",
                    "Safe frames",
                    s.guides_safe,
                    t,
                    |s| {
                        crate::rune_sec::settings_popover::toggle_guides_safe(s);
                    },
                ))
                .child(separator("menu-view-sep", t))
                .child(item("view-fit", "Fit view", false, t, |s| {
                    s.core.frame_all_figures()
                }))
                .child(item("view-centre", "Centre on selection", false, t, |s| {
                    s.core.centre_on_selection()
                }))
                .child(separator("menu-view-sep2", t))
                .child(item(
                    "view-toggle-left",
                    "Figures sidebar",
                    s.show_left,
                    t,
                    |s| {
                        s.show_left = !s.show_left;
                    },
                ))
                .child(item(
                    "view-toggle-right",
                    "Inspector sidebar",
                    s.show_right,
                    t,
                    |s| {
                        s.show_right = !s.show_right;
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
                .child(item("help", "Shortcuts", s.menus.help_open, t, |s| {
                    s.menus.help_open = !s.menus.help_open;
                }))
                .child(separator("menu-help-sep", t))
                .child(item("menu-palette", "Command palette", false, t, |s| {
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
        assert_eq!(MENUS.len(), MENU_KEYS.len());
    }

    #[test]
    fn shortcut_hints_name_only_real_bindings() {
        // The verified list, from `main.rs::on_key` and
        // `command_palette::palette_handle_key` (see the
        // `shortcut_hint` docs): exactly four menu items
        // have shell key bindings.
        assert_eq!(shortcut_hint("undo"), Some("Ctrl+Z"));
        assert_eq!(shortcut_hint("redo"), Some("Ctrl+Shift+Z"));
        assert_eq!(shortcut_hint("onion"), Some("O"));
        assert_eq!(shortcut_hint("menu-palette"), Some("Ctrl+K"));
        // Every other item key used by `menu_dropdown`
        // has no binding and must stay hint-less — if a
        // future binding is added to the shell, add its
        // entry to `shortcut_hint` and to this list in
        // the same change.
        for key in [
            "save",
            "load",
            "export",
            "copy-frame",
            "paste-frame",
            "view-guides",
            "view-guides-frustum",
            "view-guides-safe",
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
            ("onion", "eye"),
            ("view-guides", "eye"),
            ("view-guides-frustum", "crosshairs"),
            ("view-guides-safe", "crosshairs"),
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
        for key in ["view-fit", "view-centre", "menu-file", "bogus"] {
            assert_eq!(menu_icon(key), None, "{key} stays plain");
        }
    }

    #[test]
    fn all_dropdowns_build_with_hinted_items_and_guides_states() {
        // Phase F: hinted items (Edit ▸ Undo/Redo, View ▸
        // Onion skin, Help ▸ Command palette) and the
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
}
