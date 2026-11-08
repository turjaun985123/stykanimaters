//! File / Edit / View / Help menus for the Rune editor shell
//! (Phase D, section 22 — menus). Declared from `main.rs` by
//! the integrator; bin-crate code, not part of the engine
//! library.
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
//! Onion button flips — there is no op for it). Element keys
//! carry the Wave 15 menu ids (`menu-file` …) on the
//! triggers and the original GPUI button ids on the items,
//! so headless `--tree` checks can find them.

use rune::tok::Theme;
use rune::{col, el, row, Align, El, Side};

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

/// One menu item: a chrome button that runs `f` and then
/// closes the menu (selecting an item always dismisses it,
/// as in the GPUI dropdowns).
fn item(
    key: &str,
    label: &str,
    active: bool,
    t: &Theme,
    f: impl Fn(&mut State) + 'static,
) -> El {
    btn(key, label, active, t, move |s: &mut State| {
        f(s);
        s.menus.close();
    })
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
            // Only real core view state/ops: `core.onion`
            // is a public field (no op exists — the app
            // bar's Onion button flips it too), and the
            // camera-framing ops from the GPUI View buttons.
            // There is no guides toggle in the core (grep:
            // no `guides` in editor.rs/model.rs), and panel
            // toggles are integrator view state, not core
            // ops, so neither appears here.
            list = list
                .child(item("onion", "Onion skin", core.onion, t, |s| {
                    s.core.onion = !s.core.onion;
                }))
                .child(separator("menu-view-sep", t))
                .child(item("view-fit", "Fit view", false, t, |s| {
                    s.core.frame_all_figures()
                }))
                .child(item("view-centre", "Centre on selection", false, t, |s| {
                    s.core.centre_on_selection()
                }));
        }
        _ => {
            // HELP — "Shortcuts" flips the flag the
            // integrator's help panel renders from.
            list = list.child(item("help", "Shortcuts", s.menus.help_open, t, |s| {
                s.menus.help_open = !s.menus.help_open;
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
}
