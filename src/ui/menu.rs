//! App-bar menus: the button faces and dropdown container for
//! the shell's File / Edit / View / Help menus.
//!
//! Wave 15 replaces the flat button wall with a real menu bar.
//! The bar itself shows one [`menu_button`] per entry in
//! [`MENUS`]; while a menu is open, its commands render inside a
//! [`menu_dropdown`] row directly under the bar. The command
//! buttons inside keep their existing ids and listeners — this
//! module only supplies the chrome around them.
//!
//! Presentation only, like the rest of the component library:
//! plain builders, no state, no handlers, no element ids.
//! Callers attach `.id()` / `.on_click()` to the returned `Div`s
//! (the established swatch pattern), and the open menu's accent
//! background is the state signal — labels carry no suffix
//! glyphs.

use crate::ui_kit::{C_ACCENT, C_BORDER, C_INK, C_PANEL, C_SURFACE, RAD_MD, SP_SM, TEXT_MD};
use gpui::*;

/// The app bar's menus, in bar order. The command set under
/// each one lives in the app bar's wiring, not here.
pub const MENUS: [&str; 4] = ["File", "Edit", "View", "Help"];

/// One menu's face on the app bar: the same metrics as the
/// app's `btn()` helper (10×6 padding, 13px text, medium
/// radius), so the menus sit flush with the bar's other
/// controls. Background is the accent while the menu is
/// `open`, the bar's own surface colour while closed; the
/// label is plain — no arrow suffix; the accent is the state
/// signal. The caller attaches the id and click handler that
/// toggle the menu.
pub fn menu_button(label: &str, open: bool) -> Div {
    div()
        .px(px(10.0))
        .py(px(6.0))
        .rounded(px(RAD_MD))
        .bg(rgb(if open { C_ACCENT } else { C_SURFACE }))
        .text_size(px(TEXT_MD))
        .text_color(rgb(C_INK))
        .child(label.to_string())
}

/// The container an open menu's commands render into: a
/// bordered panel-toned row placed under the app bar by the
/// caller. Callers add the command buttons (which keep their
/// own ids and listeners) as children; this is only the shared
/// surface so every menu's dropdown reads as the same object.
pub fn menu_dropdown() -> Div {
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(SP_SM))
        .bg(rgb(C_PANEL))
        .border_1()
        .border_color(rgb(C_BORDER))
        .rounded(px(RAD_MD))
        .p(px(SP_SM))
}

#[cfg(test)]
mod tests {
    use super::{menu_button, menu_dropdown, MENUS};

    #[test]
    fn menus_are_the_four_app_bar_menus_in_order() {
        assert_eq!(MENUS, ["File", "Edit", "View", "Help"]);
        assert_eq!(MENUS.len(), 4);
    }

    #[test]
    fn menu_button_builds_open_and_closed() {
        // Construction only — a built Div's style is not
        // inspectable from a unit test; the point is that both
        // states build (the same construction-only contract the
        // sibling components use).
        let _closed = menu_button("File", false);
        let _open = menu_button("File", true);
        let _empty = menu_button("", false);
    }

    #[test]
    fn menu_dropdown_builds() {
        let _row = menu_dropdown();
    }
}
