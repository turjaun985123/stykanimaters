//! Shared UI kit for the editor redesign.
//!
//! Design language: dark panels with thin borders, small muted
//! uppercase captions naming each tool group, and one wrapping row of
//! buttons per group. This replaces the old layout — dozens of flat,
//! undifferentiated button rows — with a small set of labelled,
//! bordered panels ([`tool_group`]) arranged in a top bar and a side
//! panel, so every control sits under a caption that says what it is.
//!
//! Everything here is presentation only: plain `Div` builders with no
//! state, no handlers and no editor knowledge. Callers compose a
//! [`tool_group`], then add one [`group_row`] of their own buttons.

use gpui::*;

/// Primary text colour.
pub const C_INK: u32 = 0xE8E8EC;
/// Secondary / caption text colour.
pub const C_MUTED: u32 = 0x9AA3B2;
/// Panel background.
pub const C_PANEL: u32 = 0x22262F;
/// Panel and divider border colour.
pub const C_BORDER: u32 = 0x3A4150;
/// Active / accent background.
pub const C_ACCENT: u32 = 0x3D5A99;

/// Small caption naming a group of controls: 10px, muted, uppercase.
pub fn section_label(text: &str) -> Div {
    div()
        .text_size(px(10.0))
        .text_color(rgb(C_MUTED))
        .child(text.to_uppercase())
}

/// A bordered panel for one family of tools. The panel already
/// contains its [`section_label`] as the first child; callers add one
/// [`group_row`] of buttons after it.
pub fn tool_group(label: &str) -> Div {
    div()
        .flex_col()
        .gap(px(4.0))
        .bg(rgb(C_PANEL))
        .border_1()
        .border_color(rgb(C_BORDER))
        .rounded(px(8.0))
        .p(px(6.0))
        .child(section_label(label))
}

/// The wrapping row of buttons that lives inside a [`tool_group`].
pub fn group_row() -> Div {
    div().flex().flex_wrap().gap(px(4.0)).items_center()
}

/// Vertical separator: 1px wide, 20px tall.
pub fn vsep() -> Div {
    div()
        .w(px(1.0))
        .h(px(20.0))
        .mx(px(4.0))
        .bg(rgb(C_BORDER))
}

/// Horizontal separator: full width, 1px tall.
pub fn hsep() -> Div {
    div().w_full().h(px(1.0)).bg(rgb(C_BORDER))
}

/// Container for a sidebar: a vertical stack of tool groups.
pub fn side_panel() -> Div {
    div().flex_col().gap(px(6.0)).p(px(8.0))
}

/// Container for the top bar: a horizontal row of aligned items.
pub fn top_bar() -> Div {
    div().flex().items_center().gap(px(6.0))
}

/// Read-only pill showing a piece of state (frame counter, camera
/// readout, …). Accent background when `active`, neutral otherwise.
pub fn info_chip(text: String, active: bool) -> Div {
    div()
        .px(px(8.0))
        .py(px(4.0))
        .rounded(px(999.0))
        .bg(rgb(if active { C_ACCENT } else { 0x2C313A }))
        .text_size(px(12.0))
        .text_color(rgb(C_INK))
        .child(text)
}

/// The app brand mark for the top bar: "StykAnimate" in bold ink with
/// a muted "RS" suffix.
pub fn brand() -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .child(
            div()
                .text_size(px(15.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(C_INK))
                .child("StykAnimate"),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(rgb(C_MUTED))
                .child("RS"),
        )
}
