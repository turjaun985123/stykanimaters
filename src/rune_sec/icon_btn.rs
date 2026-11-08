//! Icon-bearing buttons for the Rune editor shell (Phase H,
//! section 3). Declared from `rune_sec/mod.rs` only — bin-crate
//! code, not part of the engine library.
//!
//! Why this element exists: the shell's button —
//! `crate::chrome_view::btn` (chrome_view.rs:37), the helper
//! every `rune_sec` panel imports — renders a SINGLE text run;
//! there is no icon slot. Panels that want a Font Awesome
//! glyph beside (or instead of) a label use the two builders
//! here, which mirror `btn`'s styling value for value so an
//! icon button is indistinguishable from a text one:
//!
//! * padding `.px(10.0).py(5.0)`, radius `6.0`
//!   (chrome_view.rs:44-46);
//! * fill: active → `tok::accent(0, true)`, inactive →
//!   `t.card`; hover: active → accent, inactive →
//!   `t.control_track`; press: active → accent, inactive →
//!   `t.control_off` (chrome_view.rs:47-49);
//! * label in `type_::LABEL` (12 pt / 500, rune tok.rs:139),
//!   coloured `Color::WHITE` when active, `t.text` when not
//!   (chrome_view.rs:52-55) — the glyph takes the SAME colour
//!   as the label in each state, via [`glyph_color`].
//!
//! The handler type matches `btn`'s exactly:
//! `impl Fn(&mut State) + 'static`, attached with `.on_press`
//! on the root element.
//!
//! The glyph itself is drawn by section 2's
//! `crate::rune_sec::icons::icon_el(name, size, colour)`;
//! this file owns no path data.
//!
//! FALLBACK RULE: an icon name the catalogue doesn't know
//! must never break a panel. [`icon_btn`] checks
//! [`use_icon`] first and, when the name is unknown, returns
//! the plain `btn(key, label, active, t, on_press)` — same
//! key, same label, same handler, just no glyph.
//! [`icon_only_btn`] has no label to fall back to, so an
//! unknown name yields the same square button with no glyph
//! child: right size, right key, still pressable.

use rune::tok::{self, type_, Theme};
use rune::{row, text, Color, El};

use crate::chrome_view::btn;
use crate::rune_sec::icons;
use crate::State;

/// Glyph edge length (pt) in [`icon_btn`] — close to the
/// 12 pt label's cap height, so glyph and text sit on one
/// visual line.
pub const ICON_SIZE: f32 = 13.0;

/// Horizontal gap (pt) between glyph and label in
/// [`icon_btn`]. The kit button has no icon slot, so there
/// is no kit value to mirror; 6 pt matches the gap the
/// shell's dense rows use between small controls.
pub const ICON_LABEL_GAP: f32 = 6.0;

/// Whether `icon` names a glyph the icon catalogue can
/// draw — the fallback decision for both builders, factored
/// out so it is unit-testable without building elements.
pub fn use_icon(icon: &str) -> bool {
    icons::has_icon(icon)
}

/// The colour a button's foreground (label text AND glyph)
/// takes in each state — `btn`'s own choice
/// (chrome_view.rs:54): white on the accent fill, the
/// theme's text colour on the card fill.
pub fn glyph_color(active: bool, t: &Theme) -> Color {
    if active {
        Color::WHITE
    } else {
        t.text
    }
}

/// Resting fill, mirroring `btn` (chrome_view.rs:47).
pub fn fill_color(active: bool, t: &Theme) -> Color {
    if active {
        tok::accent(0, true)
    } else {
        t.card
    }
}

/// Hover fill, mirroring `btn` (chrome_view.rs:48).
pub fn hover_color(active: bool, t: &Theme) -> Color {
    if active {
        tok::accent(0, true)
    } else {
        t.control_track
    }
}

/// Pressed fill, mirroring `btn` (chrome_view.rs:49).
pub fn press_color(active: bool, t: &Theme) -> Color {
    if active {
        tok::accent(0, true)
    } else {
        t.control_off
    }
}

/// Glyph edge length inside an [`icon_only_btn`] of
/// `button_size`: the shared [`ICON_SIZE`], but never
/// larger than the button itself, so a small square can
/// never overflow its own box.
pub fn glyph_size_for(button_size: f32) -> f32 {
    ICON_SIZE.min(button_size)
}

/// A kit-`btn`-styled button with a Font Awesome glyph
/// before its label: a horizontal row of
/// `icons::icon_el(icon, 13.0, glyph_colour)` and the label
/// text, root keyed `key`, press handler on the root.
///
/// Unknown icon name → the plain kit `btn` (see the module
/// docs' fallback rule).
pub fn icon_btn(
    key: &str,
    icon: &str,
    label: &str,
    active: bool,
    t: &Theme,
    on_press: impl Fn(&mut State) + 'static,
) -> El {
    if !use_icon(icon) {
        return btn(key, label, active, t, on_press);
    }
    let glyph = glyph_color(active, t);
    row()
        .key(key)
        .px(10.0)
        .py(5.0)
        .radius(6.0)
        .fill(fill_color(active, t))
        .hover(hover_color(active, t))
        .press(press_color(active, t))
        .on_press(on_press)
        .gap(ICON_LABEL_GAP)
        .center_y()
        .child(icons::icon_el(icon, ICON_SIZE, glyph))
        .child(text(label).font(type_::LABEL).color(glyph))
}

/// A square icon-only button, `size` × `size`, same fills
/// as [`icon_btn`], glyph centred. Unlike [`icon_btn`] it
/// takes NO padding: the caller fixes the box, and padding
/// would break the square contract — centring does the
/// placement instead.
///
/// Unknown icon name → the same square with no glyph child
/// (see the module docs' fallback rule).
pub fn icon_only_btn(
    key: &str,
    icon: &str,
    size: f32,
    active: bool,
    t: &Theme,
    on_press: impl Fn(&mut State) + 'static,
) -> El {
    let mut root = row()
        .key(key)
        .w(size)
        .h(size)
        .radius(6.0)
        .fill(fill_color(active, t))
        .hover(hover_color(active, t))
        .press(press_color(active, t))
        .on_press(on_press)
        .center();
    if use_icon(icon) {
        root = root.child(icons::icon_el(
            icon,
            glyph_size_for(size),
            glyph_color(active, t),
        ));
    }
    root
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_color_matches_the_kit_label_choice() {
        let t = rune::tok::theme(true);
        assert_eq!(glyph_color(true, &t), Color::WHITE);
        assert_eq!(glyph_color(false, &t), t.text);
        let light = rune::tok::theme(false);
        assert_eq!(glyph_color(true, &light), Color::WHITE);
        assert_eq!(glyph_color(false, &light), light.text);
    }

    #[test]
    fn state_fills_mirror_the_kit_button() {
        let t = rune::tok::theme(true);
        let accent = tok::accent(0, true);
        assert_eq!(fill_color(true, &t), accent);
        assert_eq!(hover_color(true, &t), accent);
        assert_eq!(press_color(true, &t), accent);
        assert_eq!(fill_color(false, &t), t.card);
        assert_eq!(hover_color(false, &t), t.control_track);
        assert_eq!(press_color(false, &t), t.control_off);
    }

    #[test]
    fn use_icon_follows_the_catalogue() {
        // "play" ships in the curated Font Awesome set
        // (icon_data.rs) the icon system is built on.
        assert!(use_icon("play"));
        assert!(!use_icon("definitely-not-an-icon"));
        assert!(!use_icon(""));
    }

    #[test]
    fn glyph_size_is_icon_size_capped_by_the_button() {
        assert_eq!(glyph_size_for(26.0), ICON_SIZE);
        assert_eq!(glyph_size_for(13.0), ICON_SIZE);
        assert_eq!(glyph_size_for(10.0), 10.0);
    }

    #[test]
    fn builders_construct_on_both_fallback_branches() {
        // Element construction is pure (a press needs a
        // Runtime; the tree's keys/children are not readable
        // from a bin-crate unit test — the Phase G §12
        // finding), so these pin that neither branch panics:
        // known icon → glyph row, unknown icon → the plain
        // kit btn, and the icon-only square both ways.
        let t = rune::tok::theme(true);
        let _ = icon_btn("k-icon", "play", "Play", false, &t, |_| {});
        let _ = icon_btn("k-fallback", "nope-not-real", "Play", true, &t, |_| {});
        let _ = icon_only_btn("k-only", "play", 26.0, false, &t, |_| {});
        let _ = icon_only_btn("k-only-empty", "nope-not-real", 26.0, true, &t, |_| {});
    }
}
