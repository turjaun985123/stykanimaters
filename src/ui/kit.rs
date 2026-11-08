//! Shared UI kit for the editor redesign.
//!
//! Design language: dark panels with thin borders, small muted
//! uppercase captions naming each tool group, and one wrapping row of
//! buttons per group. This replaces the old layout — dozens of flat,
//! undifferentiated button rows — with a small set of labelled,
//! bordered panels ([`tool_group`]) arranged in a top bar and a side
//! panel, so every control sits under a caption that says what it is.
//!
//! Colours come from the redesign directive's canonical palette (see
//! the `C_*` constants below). The flat constants are, and remain,
//! the default path: most components in the app read them directly
//! and are therefore dark-themed. [`Palette`] (with [`Palette::DARK`]
//! and [`Palette::LIGHT`]) is the migration vehicle for themed
//! components (e.g. a themed panel shell) that take their colours as
//! data; full per-component adoption of `Palette` is phased and has
//! NOT happened yet — components that were not migrated still use
//! the flat dark constants regardless of any theme setting.
//!
//! Everything here is presentation only: plain `Div` builders with no
//! state, no handlers and no editor knowledge. Callers compose a
//! [`tool_group`], then add one [`group_row`] of their own buttons.

use gpui::*;

/// Application background (the darkest surface).
pub const C_BG: u32 = 0x111318;
/// Secondary background, one step above [`C_BG`].
pub const C_SURFACE: u32 = 0x171A21;
/// Panel background.
pub const C_PANEL: u32 = 0x1D222B;
/// Elevated surface, above panels (menus, popovers, chips).
pub const C_ELEVATED: u32 = 0x252B35;
/// Hover surface, one step above [`C_ELEVATED`].
pub const C_HOVER: u32 = 0x303846;
/// Primary text colour.
pub const C_INK: u32 = 0xF1F3F7;
/// Secondary / caption text colour.
pub const C_MUTED: u32 = 0xA2AAB8;
/// Tertiary text colour (hints, disabled labels).
pub const C_TEXT_MUTED: u32 = 0x707B8C;
/// Panel and divider border colour.
pub const C_BORDER: u32 = 0x303744;
/// Active / accent background.
pub const C_ACCENT: u32 = 0x6D8CFF;
/// Secondary accent (used sparingly, beside [`C_ACCENT`]).
pub const C_ACCENT2: u32 = 0xA184FF;
/// Idle background for chips and other small neutral controls.
pub const C_CELL: u32 = 0x252B35;
/// Error / destructive accent: the 2.0 directive's error red,
/// shared with the toast system (`ui::toast::C_ERROR` re-exports
/// this constant).
pub const C_ERROR: u32 = 0xF16F78;
/// Success accent (confirmations, positive status).
pub const C_SUCCESS: u32 = 0x55C99A;
/// Warning accent (caution states).
pub const C_WARN: u32 = 0xE8B65D;

/// Spacing scale (reference values for new code; existing layout
/// code keeps the literals it already has).
pub const SP_XS: f32 = 2.0;
/// Spacing scale: small gap/padding.
pub const SP_SM: f32 = 4.0;
/// Spacing scale: medium gap/padding.
pub const SP_MD: f32 = 8.0;
/// Spacing scale: large gap/padding.
pub const SP_LG: f32 = 12.0;
/// Spacing scale: extra-large gap/padding.
pub const SP_XL: f32 = 16.0;

/// Type scale: extra-small text (captions).
pub const TEXT_XS: f32 = 11.0;
/// Type scale: small text (chips, secondary labels).
pub const TEXT_SM: f32 = 12.0;
/// Type scale: default control text.
pub const TEXT_MD: f32 = 13.0;
/// Type scale: large text (brand, headings).
pub const TEXT_LG: f32 = 15.0;
/// Type scale: extra-large text (titles).
pub const TEXT_XL: f32 = 18.0;

/// Corner radius scale: small (swatches, cells).
pub const RAD_SM: f32 = 4.0;
/// Corner radius scale: medium (buttons).
pub const RAD_MD: f32 = 6.0;
/// Corner radius scale: large (panels).
pub const RAD_LG: f32 = 8.0;

/// A complete set of theme colours, as data. The flat `C_*`
/// constants above are the dark palette spelled out as constants;
/// `Palette` exists so themed components can receive either theme
/// without reading globals. See the module comment for the honest
/// adoption status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    /// Application background.
    pub bg: u32,
    /// Secondary background.
    pub surface: u32,
    /// Panel background.
    pub panel: u32,
    /// Elevated surface background.
    pub elevated: u32,
    /// Hover surface background.
    pub hover: u32,
    /// Primary text.
    pub ink: u32,
    /// Secondary text.
    pub muted: u32,
    /// Tertiary text.
    pub text_muted: u32,
    /// Borders and dividers.
    pub border: u32,
    /// Primary accent.
    pub accent: u32,
    /// Secondary accent.
    pub accent2: u32,
    /// Idle chip / neutral control background.
    pub cell: u32,
    /// Error / destructive accent.
    pub error: u32,
    /// Success accent.
    pub success: u32,
    /// Warning accent.
    pub warn: u32,
}

impl Palette {
    /// The redesign directive's dark palette. Built from the flat
    /// `C_*` constants so the two can never drift apart.
    pub const DARK: Palette = Palette {
        bg: C_BG,
        surface: C_SURFACE,
        panel: C_PANEL,
        elevated: C_ELEVATED,
        hover: C_HOVER,
        ink: C_INK,
        muted: C_MUTED,
        text_muted: C_TEXT_MUTED,
        border: C_BORDER,
        accent: C_ACCENT,
        accent2: C_ACCENT2,
        cell: C_CELL,
        error: C_ERROR,
        success: C_SUCCESS,
        warn: C_WARN,
    };

    /// A professional light theme at the same standard as the dark
    /// one: near-white panels, the dark theme's surface colour as
    /// ink, and accents darkened enough to keep contrast on white.
    /// The status colours follow the same rule: `error` is a
    /// darkened variant of the dark theme's bright red (which is
    /// too light to read on white), and `success`/`warn` reuse the
    /// deeper tones the badge/gauge components already use.
    pub const LIGHT: Palette = Palette {
        bg: 0xF2F3F6,
        surface: 0xE8EAEF,
        panel: 0xFFFFFF,
        elevated: 0xFFFFFF,
        hover: 0xDDE1E8,
        ink: 0x171A21,
        muted: 0x5A6474,
        text_muted: 0x8A94A3,
        border: 0xD3D8E1,
        accent: 0x4A6FE0,
        accent2: 0x7C5CD6,
        cell: 0xE8EAEF,
        error: 0xC93A44,
        success: 0x2E7D4F,
        warn: 0x8A6D1F,
    };
}

/// The palette for the requested theme: dark when `dark` is true,
/// light otherwise.
pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette::DARK
    } else {
        Palette::LIGHT
    }
}

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
        .gap(px(SP_SM))
        .bg(rgb(C_PANEL))
        .border_1()
        .border_color(rgb(C_BORDER))
        .rounded(px(RAD_LG))
        .p(px(6.0))
        .child(section_label(label))
}

/// The wrapping row of buttons that lives inside a [`tool_group`].
pub fn group_row() -> Div {
    div().flex().flex_wrap().gap(px(SP_SM)).items_center()
}

/// Vertical separator: 1px wide, 20px tall.
pub fn vsep() -> Div {
    div()
        .w(px(1.0))
        .h(px(20.0))
        .mx(px(SP_SM))
        .bg(rgb(C_BORDER))
}

/// Horizontal separator: full width, 1px tall.
pub fn hsep() -> Div {
    div().w_full().h(px(1.0)).bg(rgb(C_BORDER))
}

/// Container for a sidebar: a vertical stack of tool groups.
pub fn side_panel() -> Div {
    div().flex_col().gap(px(6.0)).p(px(SP_MD))
}

/// Container for the top bar: a horizontal row of aligned items.
pub fn top_bar() -> Div {
    div().flex().items_center().gap(px(6.0))
}

/// Read-only pill showing a piece of state (frame counter, camera
/// readout, …). Accent background when `active`, neutral otherwise.
pub fn info_chip(text: String, active: bool) -> Div {
    div()
        .px(px(SP_MD))
        .py(px(SP_SM))
        .rounded(px(999.0))
        .bg(rgb(if active { C_ACCENT } else { C_CELL }))
        .text_size(px(TEXT_SM))
        .text_color(rgb(C_INK))
        .child(text)
}

/// Larger pill for the workspace selector: accent background when
/// `active`, neutral cell background otherwise.
pub fn workspace_chip(label: String, active: bool) -> Div {
    div()
        .px(px(SP_LG))
        .py(px(6.0))
        .rounded(px(999.0))
        .bg(rgb(if active { C_ACCENT } else { C_CELL }))
        .text_size(px(TEXT_SM))
        .text_color(rgb(C_INK))
        .child(label)
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
                .text_size(px(TEXT_LG))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(C_INK))
                .child("StykAnimate"),
        )
        .child(
            div()
                .text_size(px(TEXT_SM))
                .text_color(rgb(C_MUTED))
                .child("RS"),
        )
}

#[cfg(test)]
mod tests {
    use super::{
        palette, Palette, C_ACCENT, C_ACCENT2, C_BG, C_BORDER, C_CELL, C_ELEVATED, C_ERROR,
        C_HOVER, C_INK, C_MUTED, C_PANEL, C_SUCCESS, C_SURFACE, C_TEXT_MUTED, C_WARN, RAD_LG,
        RAD_MD, RAD_SM, SP_LG, SP_MD, SP_SM, SP_XL, SP_XS, TEXT_LG, TEXT_MD, TEXT_SM, TEXT_XL,
        TEXT_XS,
    };

    /// sRGB relative luminance of a 0xRRGGBB colour, in 0.0..=1.0.
    fn luminance(colour: u32) -> f32 {
        fn linear(channel: u32) -> f32 {
            let v = channel as f32 / 255.0;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        }
        let r = linear((colour >> 16) & 0xFF);
        let g = linear((colour >> 8) & 0xFF);
        let b = linear(colour & 0xFF);
        0.2126 * r + 0.7152 * g + 0.0722 * b
    }

    #[test]
    fn colour_tokens_match_the_directive_palette() {
        assert_eq!(C_BG, 0x111318);
        assert_eq!(C_SURFACE, 0x171A21);
        assert_eq!(C_PANEL, 0x1D222B);
        assert_eq!(C_ELEVATED, 0x252B35);
        assert_eq!(C_HOVER, 0x303846);
        assert_eq!(C_INK, 0xF1F3F7);
        assert_eq!(C_MUTED, 0xA2AAB8);
        assert_eq!(C_TEXT_MUTED, 0x707B8C);
        assert_eq!(C_BORDER, 0x303744);
        assert_eq!(C_ACCENT, 0x6D8CFF);
        assert_eq!(C_ACCENT2, 0xA184FF);
        assert_eq!(C_CELL, 0x252B35);
        assert_eq!(C_ERROR, 0xF16F78);
        assert_eq!(C_SUCCESS, 0x55C99A);
        assert_eq!(C_WARN, 0xE8B65D);
    }

    #[test]
    fn scale_tokens_are_pinned() {
        assert_eq!(SP_XS, 2.0);
        assert_eq!(SP_SM, 4.0);
        assert_eq!(SP_MD, 8.0);
        assert_eq!(SP_LG, 12.0);
        assert_eq!(SP_XL, 16.0);
        assert_eq!(TEXT_XS, 11.0);
        assert_eq!(TEXT_SM, 12.0);
        assert_eq!(TEXT_MD, 13.0);
        assert_eq!(TEXT_LG, 15.0);
        assert_eq!(TEXT_XL, 18.0);
        assert_eq!(RAD_SM, 4.0);
        assert_eq!(RAD_MD, 6.0);
        assert_eq!(RAD_LG, 8.0);
    }

    #[test]
    fn dark_palette_matches_the_flat_constants() {
        let dark = Palette::DARK;
        assert_eq!(dark.bg, C_BG);
        assert_eq!(dark.surface, C_SURFACE);
        assert_eq!(dark.panel, C_PANEL);
        assert_eq!(dark.elevated, C_ELEVATED);
        assert_eq!(dark.hover, C_HOVER);
        assert_eq!(dark.ink, C_INK);
        assert_eq!(dark.muted, C_MUTED);
        assert_eq!(dark.text_muted, C_TEXT_MUTED);
        assert_eq!(dark.border, C_BORDER);
        assert_eq!(dark.accent, C_ACCENT);
        assert_eq!(dark.accent2, C_ACCENT2);
        assert_eq!(dark.cell, C_CELL);
        assert_eq!(dark.error, C_ERROR);
        assert_eq!(dark.success, C_SUCCESS);
        assert_eq!(dark.warn, C_WARN);
    }

    #[test]
    fn palette_function_returns_the_named_palettes() {
        assert_eq!(palette(true), Palette::DARK);
        assert_eq!(palette(false), Palette::LIGHT);
    }

    #[test]
    fn light_palette_values_are_pinned() {
        let light = Palette::LIGHT;
        assert_eq!(light.bg, 0xF2F3F6);
        assert_eq!(light.surface, 0xE8EAEF);
        assert_eq!(light.panel, 0xFFFFFF);
        assert_eq!(light.elevated, 0xFFFFFF);
        assert_eq!(light.hover, 0xDDE1E8);
        assert_eq!(light.ink, 0x171A21);
        assert_eq!(light.muted, 0x5A6474);
        assert_eq!(light.text_muted, 0x8A94A3);
        assert_eq!(light.border, 0xD3D8E1);
        assert_eq!(light.accent, 0x4A6FE0);
        assert_eq!(light.accent2, 0x7C5CD6);
        assert_eq!(light.cell, 0xE8EAEF);
        assert_eq!(light.error, 0xC93A44);
        assert_eq!(light.success, 0x2E7D4F);
        assert_eq!(light.warn, 0x8A6D1F);
    }

    #[test]
    fn light_theme_ink_on_panel_has_strong_contrast() {
        let light = Palette::LIGHT;
        let ink_lum = luminance(light.ink);
        let panel_lum = luminance(light.panel);
        // Ink is genuinely dark, panel is genuinely light.
        assert!(ink_lum < 0.05, "light ink luminance {ink_lum}");
        assert!(panel_lum > 0.8, "light panel luminance {panel_lum}");
        // WCAG contrast ratio between them clears AAA for text (7:1).
        let ratio = (panel_lum + 0.05) / (ink_lum + 0.05);
        assert!(ratio > 7.0, "light ink/panel contrast ratio {ratio}");
    }

    #[test]
    fn dark_theme_ink_on_panel_has_strong_contrast() {
        let dark = Palette::DARK;
        let ratio = (luminance(dark.ink) + 0.05) / (luminance(dark.panel) + 0.05);
        assert!(ratio > 7.0, "dark ink/panel contrast ratio {ratio}");
    }
}
