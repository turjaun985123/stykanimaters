//! Modal dialog scaffolding: a dimmed scrim with a centred card.
//!
//! Panels (see [`crate::ui_panel`]) live inside the editor layout;
//! a dialog is the opposite — it floats above everything, blocks
//! the app behind a dark overlay, and asks for one decision. This
//! component supplies the two presentation pieces: [`scrim`], the
//! full-size translucent overlay, and [`dialog_card`], the bordered
//! card that sits in the middle of it. [`dialog`] composes the two.
//!
//! Presentation only, like the rest of this folder: no listeners,
//! no element ids, no editor state. The caller builds the body and
//! the action buttons (with their own ids and click handlers) and
//! passes them in; it also decides when the dialog exists — the
//! caller is expected to place the returned `Div` so it covers the
//! whole window (for example as the last child of a full-size root).

use gpui::*;

use crate::ui_kit::{section_label, top_bar, C_BORDER, C_PANEL};

/// The scrim colour in GPUI's `0xRRGGBBAA` packing: pure black at
/// alpha `0x8C` (about 55% opaque). The kit's `rgb()` has no alpha
/// channel, so translucent colours go through `rgba()` with this
/// packing — the same encoding the canvas painters in `crate::app`
/// use (e.g. `rgba((color << 8) | alpha)`).
pub const SCRIM_COLOR: u32 = 0x0000_008C;

/// The dialog card's fixed width in pixels. Dialogs are short
/// prompts, not documents, so the card stays narrow; wider content
/// belongs in a panel.
pub const CARD_WIDTH: f32 = 420.0;

/// The scrim's alpha byte: `0x8C`, about 55% opaque.
pub fn scrim_alpha() -> u8 {
    0x8C
}

/// The dialog card's width in pixels; see [`CARD_WIDTH`].
pub fn card_width() -> f32 {
    CARD_WIDTH
}

/// The full-size dark overlay that blocks the UI behind a dialog.
///
/// Sized with `w_full` / `h_full`, so it covers whatever box the
/// caller places it in. On its own it is just the dim layer —
/// [`dialog`] is the composed form that also centres a card in it.
pub fn scrim() -> Div {
    div().w_full().h_full().bg(rgba(SCRIM_COLOR))
}

/// The centred dialog card: a sibling of
/// [`crate::ui_panel::panel_shell`] with the metrics bumped up for
/// a floating surface (softer corners, wider padding).
///
/// Layout is a column: a header row holding `title` as a
/// [`section_label`], then the caller's `body`, then a
/// right-aligned row of the caller's `actions` (typically Cancel /
/// Confirm buttons). The action `Div`s are used exactly as passed —
/// their ids and listeners stay with the caller.
///
/// The title is copied into an owned `String` before it is stored
/// in the tree: element children must be `'static`, so the borrowed
/// `&str` itself cannot be kept.
pub fn dialog_card(title: &str, body: Div, actions: Vec<Div>) -> Div {
    let title = title.to_string();

    let mut action_row = div().flex().justify_end().gap(px(6.0));
    for action in actions {
        action_row = action_row.child(action);
    }

    div()
        .flex_col()
        .w(px(card_width()))
        .bg(rgb(C_PANEL))
        .border_1()
        .border_color(rgb(C_BORDER))
        .rounded(px(12.0))
        .p(px(16.0))
        .gap(px(10.0))
        .child(top_bar().child(section_label(&title)))
        .child(body)
        .child(action_row)
}

/// A complete modal: the [`scrim`] as a flex container with a
/// [`dialog_card`] centred in it, both axes.
pub fn dialog(title: &str, body: Div, actions: Vec<Div>) -> Div {
    scrim()
        .flex()
        .items_center()
        .justify_center()
        .child(dialog_card(title, body, actions))
}

#[cfg(test)]
mod tests {
    // NOTE: no `use super::*;` here — this file's `use gpui::*;`
    // would re-export GPUI's own `test` macro through the glob and
    // shadow the built-in `#[test]`, which then expands into itself
    // forever (CI failure, 2026-11-08). Import exact names only.
    //
    // The builders need a GPUI window to exercise, so the tests
    // cover the pure values the builders are defined in terms of.
    use super::{card_width, scrim_alpha, CARD_WIDTH, SCRIM_COLOR};

    #[test]
    fn scrim_colour_is_black_at_the_documented_alpha() {
        assert_eq!(SCRIM_COLOR, 0x0000_008C);
        // The RGB part is pure black; the low byte is the alpha.
        assert_eq!(SCRIM_COLOR >> 8, 0);
        assert_eq!(SCRIM_COLOR & 0xFF, scrim_alpha() as u32);
        assert_eq!(scrim_alpha(), 0x8C);
    }

    #[test]
    fn card_width_matches_the_constant() {
        assert_eq!(CARD_WIDTH, 420.0);
        assert_eq!(card_width(), CARD_WIDTH);
    }
}
