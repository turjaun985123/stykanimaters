//! App-side theme construction — the UI mission's palette
//! seam (Squad 02 audit `docs/ui_mission/audit/
//! s02-design-system.md`, foundation items F1/F2/F4/F5).
//! Declared from `rune_sec/mod.rs` only — bin-crate code,
//! not part of the engine library.
//!
//! The shell's colours live in the pinned Rune crate's
//! `tok::Theme` — a struct with exactly 16 `Color`
//! fields and no palette file in this repository. The
//! lawful adoption path (audit §0) is app-side
//! construction: [`app_theme`] starts from
//! `tok::theme(dark)` and overrides the fields the
//! mission brief names, in this one place, so every
//! panel downstream inherits the brief's palette
//! unchanged. `main.rs::view` is the single call site.
//!
//! # Dark mapping (brief → `Theme` field)
//!
//! | Brief role | Brief value | Field | Set to |
//! |---|---|---|---|
//! | Main background | `#101216` | `bg` | `#101216` |
//! | Panel surface | `#1D222B` | `card` | `#1D222B` |
//! | Primary text | `#F1F3F7` | `text` | `#F1F3F7` |
//! | Secondary text | `#A1A9B7` | `text_secondary` | `#A1A9B7` |
//! | Subtle border | `#303846` | `card_border` | `#303846` solid (see below) |
//! | Subtle border | `#303846` | `separator` | `#303846` @ α 0.55 (see below) |
//!
//! **The border mechanism exception.** Upstream,
//! `card_border`/`separator` are *alpha hairlines*
//! (white at α 0.05/0.055) whose effective colour
//! changes with whatever is behind them — there is no
//! solid border colour in the struct's design. The
//! brief's `#303846` is a solid. `card_border` takes
//! the brief value solid: a card edge sits on the app
//! background or between panels, the two adjacencies
//! the brief's palette pairs `#303846` with, so the
//! solid reads as specified in both. `separator` —
//! hairlines *inside* a card, which must stay
//! subordinate to the card edge — keeps the alpha
//! mechanism with the brief colour as its tint source:
//! composited over the `#1D222B` card it lands at
//! ≈ `#272E3A`, a dimmer sibling of the border.
//! Squad 19's pixel pass is the judge of both choices.
//!
//! Every other dark field (`control_track`,
//! `control_off`, `hover`, `press`, `indicator`,
//! `knob`, the four shadows) is a *mechanism* value the
//! brief does not name — white/black alpha overlays and
//! shadow depths — and is retained from upstream
//! verbatim.
//!
//! # Light mapping
//!
//! The brief names no light values (its palette is
//! dark-only). Per the audit's recommendation (§2),
//! the light overrides mirror the dark deltas — the
//! same blue-grey hue shift, lightness preserved —
//! rather than inventing a second palette:
//!
//! | Field | Upstream ≈ | Set to | Why |
//! |---|---|---|---|
//! | `bg` | `#F0F1F4` | `#EDF0F5` | same lightness, brief's blue-grey cast |
//! | `card` | `#FFFFFF` | retained | white panels are the brief's light direction |
//! | `text` | `#1A1C1F` | `#171A21` | blue-grey ink, matching the dark text family |
//! | `text_secondary` | `#606369` | `#5B6577` | the dark secondary's hue shift, mirrored |
//! | `card_border` | black α 0.10 | `#2A3242` @ α 0.14 | tinted hairline (≈ `#E1E4E9` over the white card) |
//! | `separator` | black α 0.13 | `#2A3242` @ α 0.19 | tinted hairline (≈ `#D6D9E0` over the white card) |
//!
//! Remaining light fields are mechanism values,
//! retained as in the dark theme.
//!
//! # Roles `Theme` cannot hold
//!
//! The brief names four roles with no field in the
//! struct. They are provided here as derived helpers
//! instead of being scattered as literals:
//!
//! * **Secondary (chrome) surface** `#171B22` —
//!   [`chrome_surface`]. Dark: the brief value.
//!   Light: derived midway between `bg` and `card`.
//! * **Elevated surface** `#252C37` — [`elevated`].
//!   Dark: the brief value. Light: the card itself —
//!   the card is already the lightest surface, so in
//!   light theme elevation is carried by shadow and
//!   border, and the fill intentionally equals `card`.
//! * **Muted (third-level) text** `#737D8C` —
//!   [`muted_text`]. Dark: the brief value. Light:
//!   derived from `text_secondary` toward `bg`.
//!   (`design::text_muted` remains the alpha-derived
//!   sibling; it now derives from the overridden
//!   secondary, so the two agree in family.)
//! * **Accent** — accent is a function upstream
//!   (`tok::accent(i, dark)`), not a `Theme` field, and
//!   13 files hard-code its dark variant. [`accent`] /
//!   [`accent_on`] are the theme-aware successors; the
//!   call-site migration belongs to the adoption wave.
//!
//! # Semantic tokens, and the danger-fill exception
//!
//! [`success`], [`warning`] and [`error_ink`] are the
//! brief's semantic values (`#43C995`, `#F1B85B`,
//! `#EF6974`) as single definitions, ending the toast
//! dot/glyph drift the audit found in `toasts.rs`
//! (§1.5) once adoption points `Severity` here. The
//! `*_on` variants darken each tint for light-theme
//! surfaces (≈ 5:1 on the white card).
//!
//! **Exception, measured, do not "fix":** the brief's
//! error red is an *ink* — a text/glyph tint on a
//! surface. It is **not** the danger-button fill.
//! `#EF6974` under white label text measures ≈ 2.9:1,
//! and Phase N deliberately deepened the fill family
//! to `#C93A44`/`#B2323E`/`#9C2934`
//! (`chrome_view::DANGER_FILL` and siblings, ≈ 5:1 at
//! rest). Those fills stay exactly as measured; this
//! module's [`error_ink`] must never be substituted
//! for them.

use rune::tok::{self, Theme};
use rune::Color;

// ---- Brief palette anchors (dark) ----

/// Brief main background.
pub const DARK_BG: &str = "#101216";
/// Brief panel surface.
pub const DARK_CARD: &str = "#1D222B";
/// Brief primary text.
pub const DARK_TEXT: &str = "#F1F3F7";
/// Brief secondary text.
pub const DARK_TEXT_SECONDARY: &str = "#A1A9B7";
/// Brief subtle border (solid; see the module doc's
/// mechanism note for how the two border fields use
/// it).
pub const BRIEF_BORDER: &str = "#303846";
/// Separator alpha: the brief border tinted down over
/// the card, so in-card hairlines stay subordinate to
/// the card edge.
pub const DARK_SEPARATOR_ALPHA: f32 = 0.55;
/// Brief secondary (chrome) surface — no `Theme`
/// field; served by [`chrome_surface`].
pub const DARK_CHROME_SURFACE: &str = "#171B22";
/// Brief elevated surface — no `Theme` field; served
/// by [`elevated`].
pub const DARK_ELEVATED: &str = "#252C37";
/// Brief muted (third-level) text — no `Theme` field;
/// served by [`muted_text`].
pub const DARK_MUTED_TEXT: &str = "#737D8C";
/// Brief primary accent (dark).
pub const ACCENT_DARK: &str = "#6C8CFF";
/// Brief secondary accent (dark).
pub const SECONDARY_ACCENT_DARK: &str = "#9B7CFF";

// ---- Derived light anchors ----
// The brief names no light values; these mirror the
// dark deltas (audit §2): same blue-grey hue shift,
// lightness preserved. See the module doc's table.

/// Light background: upstream `#F0F1F4` shifted to the
/// brief's blue-grey cast at the same lightness.
pub const LIGHT_BG: &str = "#EDF0F5";
/// Light primary text: blue-grey ink.
pub const LIGHT_TEXT: &str = "#171A21";
/// Light secondary text.
pub const LIGHT_TEXT_SECONDARY: &str = "#5B6577";
/// Tint source for the light hairlines (the light ink
/// family), applied at alpha — the light border keeps
/// the upstream alpha mechanism.
pub const LIGHT_BORDER_TINT: &str = "#2A3242";
/// Light card-border alpha (upstream: black @ 0.10).
pub const LIGHT_BORDER_ALPHA: f32 = 0.14;
/// Light separator alpha (upstream: black @ 0.13).
pub const LIGHT_SEPARATOR_ALPHA: f32 = 0.19;
/// Light primary accent: the brief accent's hue
/// family darkened to ≈ 5:1 on the white card (the
/// brief value itself measures only ≈ 3:1 there).
pub const ACCENT_LIGHT: &str = "#4666D8";
/// Light secondary accent: the brief value's hue
/// family darkened to ≈ 4.8:1 on the white card.
pub const SECONDARY_ACCENT_LIGHT: &str = "#7A5AE0";

// ---- Semantic tokens (brief values) ----

/// Brief success tint.
pub const SUCCESS_HEX: &str = "#43C995";
/// Brief warning tint.
pub const WARNING_HEX: &str = "#F1B85B";
/// Brief error *ink* — text/glyph tint only. Never a
/// button fill: see the module doc's danger-fill
/// exception (`chrome_view::DANGER_FILL` stays).
pub const ERROR_INK_HEX: &str = "#EF6974";
/// Success tint darkened for light-theme surfaces
/// (≈ 5.2:1 on the white card).
pub const SUCCESS_LIGHT_HEX: &str = "#177A52";
/// Warning tint darkened for light-theme surfaces
/// (≈ 5.3:1 on the white card).
pub const WARNING_LIGHT_HEX: &str = "#8F6108";
/// Error ink darkened for light-theme surfaces
/// (≈ 5.4:1 on the white card).
pub const ERROR_LIGHT_HEX: &str = "#C62F3B";

/// A fixed sRGB hex literal as a Rune colour, via the
/// shell's hex idiom (the same conversion
/// `chrome_view::danger_color` performs). The fallback
/// is unreachable — every literal in this module is a
/// valid 6-digit hex — and exists only because the
/// parser is fallible in general.
fn srgb(hex: &str) -> Color {
    rune::color::hex(hex).unwrap_or(Color::WHITE)
}

/// The shell's theme: `tok::theme(dark)` with the
/// mission brief's palette applied centrally (see the
/// module doc's mapping tables). The single theme
/// construction site — `main.rs::view` calls this
/// instead of `tok::theme`.
pub fn app_theme(dark: bool) -> Theme {
    let base = tok::theme(dark);
    if dark {
        Theme {
            bg: srgb(DARK_BG),
            card: srgb(DARK_CARD),
            text: srgb(DARK_TEXT),
            text_secondary: srgb(DARK_TEXT_SECONDARY),
            card_border: srgb(BRIEF_BORDER),
            separator: srgb(BRIEF_BORDER).alpha(DARK_SEPARATOR_ALPHA),
            ..base
        }
    } else {
        Theme {
            bg: srgb(LIGHT_BG),
            // card retained: upstream's pure white is
            // the brief's light panel direction.
            text: srgb(LIGHT_TEXT),
            text_secondary: srgb(LIGHT_TEXT_SECONDARY),
            card_border: srgb(LIGHT_BORDER_TINT).alpha(LIGHT_BORDER_ALPHA),
            separator: srgb(LIGHT_BORDER_TINT).alpha(LIGHT_SEPARATOR_ALPHA),
            ..base
        }
    }
}

/// Whether a theme is the dark appearance. `Theme`
/// carries no dark flag, so this reads the one field
/// whose lightness defines the appearance: the
/// background's luminance (dark ≈ 0.005, light
/// ≈ 0.85 — the 0.5 threshold sits far from both, for
/// upstream and overridden backgrounds alike).
pub fn is_dark(t: &Theme) -> bool {
    t.bg.luminance() < 0.5
}

/// The brief's primary accent for a theme's
/// appearance: `#6C8CFF` in dark, the darkened
/// [`ACCENT_LIGHT`] in light. The theme-aware
/// successor to the 13 hard-coded `tok::accent(0,
/// true)` call sites (audit §4.1); those sites
/// migrate in the adoption wave.
pub fn accent(t: &Theme) -> Color {
    accent_on(is_dark(t))
}

/// [`accent`] for a caller that already knows the
/// appearance flag (the form `main.rs`-side state
/// naturally holds).
pub fn accent_on(dark: bool) -> Color {
    if dark {
        srgb(ACCENT_DARK)
    } else {
        srgb(ACCENT_LIGHT)
    }
}

/// The brief's secondary accent (`#9B7CFF`) for a
/// theme's appearance, with the same light darkening
/// as [`accent`].
pub fn secondary_accent(t: &Theme) -> Color {
    if is_dark(t) {
        srgb(SECONDARY_ACCENT_DARK)
    } else {
        srgb(SECONDARY_ACCENT_LIGHT)
    }
}

/// The secondary (chrome) surface: between the app
/// background and the panel card — app bar, status
/// strip and other shell furniture. `Theme` has no
/// field for it. Dark: the brief's `#171B22`. Light:
/// derived midway between `bg` and `card`.
pub fn chrome_surface(t: &Theme) -> Color {
    if is_dark(t) {
        srgb(DARK_CHROME_SURFACE)
    } else {
        t.bg.mix(t.card, 0.5)
    }
}

/// The elevated surface: menus, popovers and dialogs
/// raised above the panel card. `Theme` has no field
/// for it. Dark: the brief's `#252C37`. Light: the
/// card itself — already the lightest surface, so
/// elevation there is carried by shadow and border.
pub fn elevated(t: &Theme) -> Color {
    if is_dark(t) {
        srgb(DARK_ELEVATED)
    } else {
        t.card
    }
}

/// Muted (third-level) text, below even
/// `text_secondary`: captions, hints, disabled-adjacent
/// labels — non-essential text only. `Theme` has no
/// field for it. Dark: the brief's `#737D8C`. Light:
/// `text_secondary` mixed toward `bg`, a greyer step
/// down at the same family.
pub fn muted_text(t: &Theme) -> Color {
    if is_dark(t) {
        srgb(DARK_MUTED_TEXT)
    } else {
        t.text_secondary.mix(t.bg, 0.3)
    }
}

/// The brief's success tint (`#43C995`).
pub fn success() -> Color {
    srgb(SUCCESS_HEX)
}

/// The brief's warning tint (`#F1B85B`).
pub fn warning() -> Color {
    srgb(WARNING_HEX)
}

/// The brief's error ink (`#EF6974`) — a text/glyph
/// tint on a surface, **never** a fill behind white
/// text (see the module doc's measured exception;
/// the danger fill stays `chrome_view::DANGER_FILL`).
pub fn error_ink() -> Color {
    srgb(ERROR_INK_HEX)
}

/// [`success`] for a theme's appearance: the brief
/// value in dark, darkened to ≈ 5:1 on the white card
/// in light.
pub fn success_on(t: &Theme) -> Color {
    if is_dark(t) {
        success()
    } else {
        srgb(SUCCESS_LIGHT_HEX)
    }
}

/// [`warning`] for a theme's appearance.
pub fn warning_on(t: &Theme) -> Color {
    if is_dark(t) {
        warning()
    } else {
        srgb(WARNING_LIGHT_HEX)
    }
}

/// [`error_ink`] for a theme's appearance.
pub fn error_on(t: &Theme) -> Color {
    if is_dark(t) {
        error_ink()
    } else {
        srgb(ERROR_LIGHT_HEX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_theme_carries_the_brief_palette() {
        let t = app_theme(true);
        assert_eq!(t.bg.hex(), "#101216ff");
        assert_eq!(t.card.hex(), "#1d222bff");
        assert_eq!(t.text.hex(), "#f1f3f7ff");
        assert_eq!(t.text_secondary.hex(), "#a1a9b7ff");
        assert_eq!(t.card_border.hex(), "#303846ff");
        // Separator: the brief border at the separator
        // alpha — tint source exact, alpha preserved.
        let sep = t.separator;
        let sep_hex = sep.hex();
        assert_eq!(&sep_hex[..7], "#303846");
        assert!((sep.a - DARK_SEPARATOR_ALPHA).abs() < 1e-6);
    }

    #[test]
    fn dark_theme_retains_upstream_mechanism_fields() {
        let t = app_theme(true);
        let up = tok::theme(true);
        assert_eq!(t.hover, up.hover);
        assert_eq!(t.press, up.press);
        assert_eq!(t.control_off, up.control_off);
        assert_eq!(t.knob, up.knob);
        assert_eq!(t.shadow, up.shadow);
    }

    #[test]
    fn light_theme_mirrors_the_dark_deltas() {
        let t = app_theme(false);
        assert_eq!(t.bg.hex(), "#edf0f5ff");
        assert_eq!(t.text.hex(), "#171a21ff");
        assert_eq!(t.text_secondary.hex(), "#5b6577ff");
        // Card retained from upstream (pure white).
        assert_eq!(t.card, tok::theme(false).card);
        // Hairlines keep the alpha mechanism, tinted.
        assert!((t.card_border.a - LIGHT_BORDER_ALPHA).abs() < 1e-6);
        assert!((t.separator.a - LIGHT_SEPARATOR_ALPHA).abs() < 1e-6);
    }

    #[test]
    fn accent_follows_the_appearance() {
        assert_eq!(accent(&app_theme(true)).hex(), "#6c8cffff");
        assert_eq!(accent(&app_theme(false)).hex(), "#4666d8ff");
        // Also correct on raw upstream themes, so
        // not-yet-migrated callers get the right value.
        assert_eq!(accent(&tok::theme(true)).hex(), "#6c8cffff");
        assert_eq!(accent(&tok::theme(false)).hex(), "#4666d8ff");
        assert_eq!(secondary_accent(&app_theme(true)).hex(), "#9b7cffff");
    }

    #[test]
    fn missing_roles_resolve_to_the_brief_values_in_dark() {
        let t = app_theme(true);
        assert_eq!(chrome_surface(&t).hex(), "#171b22ff");
        assert_eq!(elevated(&t).hex(), "#252c37ff");
        assert_eq!(muted_text(&t).hex(), "#737d8cff");
        let light = app_theme(false);
        assert_eq!(elevated(&light), light.card);
    }

    #[test]
    fn semantic_tokens_are_the_brief_values() {
        assert_eq!(success().hex(), "#43c995ff");
        assert_eq!(warning().hex(), "#f1b85bff");
        assert_eq!(error_ink().hex(), "#ef6974ff");
        let light = app_theme(false);
        assert_eq!(success_on(&light).hex(), "#177a52ff");
        assert_eq!(warning_on(&light).hex(), "#8f6108ff");
        assert_eq!(error_on(&light).hex(), "#c62f3bff");
        let dark = app_theme(true);
        assert_eq!(success_on(&dark), success());
        assert_eq!(error_on(&dark), error_ink());
    }
}
