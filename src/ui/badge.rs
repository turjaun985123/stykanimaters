//! Small badge and pill components: tone-coloured state badges,
//! count badges, status dots and tags.
//!
//! Where [`crate::ui_kit`]'s `info_chip` is a read-only pill for
//! a piece of live state, these are the smaller, dumber marks:
//! a badge that says a figure is locked, a dot that says a save
//! is clean or dirty, a count on a panel toggle, a tag naming a
//! format. Presentation only, like the rest of the kit — every
//! builder is a plain `Div` with no state, no handlers and no
//! editor knowledge; the caller decides the tone, the component
//! just owns the colours and metrics.
//!
//! Badge metrics deliberately mirror `info_chip` (8px × 4px
//! padding, fully rounded, 12px text) so badges and chips can
//! sit in the same row without a visible size step.

use crate::ui_kit::{C_ACCENT, C_BORDER, C_CELL, C_ERROR, C_INK, C_MUTED, C_PANEL};
use gpui::*;

/// Semantic tone of a badge or dot. The tone picks a background
/// colour via [`tone_colors`]; it carries no meaning beyond the
/// colour, so callers map their own states onto it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    /// Quiet, informational — the neutral chip grey.
    Neutral,
    /// The kit accent — the thing being counted or current.
    Accent,
    /// Done / healthy / saved.
    Success,
    /// Needs attention but not broken.
    Warn,
    /// Failed / dirty / broken.
    Error,
}

/// Background and foreground colours for a tone, as 0xRRGGBB.
/// Pure data, split out from the builders so the mapping is
/// testable without a window. The neutral background is the
/// same grey `info_chip` uses when inactive.
pub fn tone_colors(tone: Tone) -> (u32, u32) {
    match tone {
        Tone::Neutral => (C_CELL, C_MUTED),
        Tone::Accent => (C_ACCENT, C_INK),
        Tone::Success => (0x2E7D4F, C_INK),
        Tone::Warn => (0x8A6D1F, C_INK),
        Tone::Error => (C_ERROR, C_INK),
    }
}

/// A small rounded pill with `text`, coloured by `tone`.
/// Same metrics as `info_chip`: 8px × 4px padding, fully
/// rounded, 12px text.
pub fn badge(text: &str, tone: Tone) -> Div {
    let (bg, fg) = tone_colors(tone);
    div()
        .px(px(8.0))
        .py(px(4.0))
        .rounded(px(999.0))
        .bg(rgb(bg))
        .text_size(px(12.0))
        .text_color(rgb(fg))
        .child(text.to_string())
}

/// The label a count badge shows: the number itself up to 99,
/// then "99+" so a runaway count can never stretch the pill.
fn count_label(n: usize) -> String {
    if n > 99 {
        "99+".to_string()
    } else {
        n.to_string()
    }
}

/// An accent pill showing a count (clips on a track, frames in
/// a range, projects in a folder, …). Counts over 99 render as
/// "99+"; see [`count_label`].
pub fn count_badge(n: usize) -> Div {
    badge(&count_label(n), Tone::Accent)
}

/// An 8px round dot in the tone's background colour — the
/// smallest possible status mark, for list rows and headers
/// where a whole badge would shout.
pub fn status_dot(tone: Tone) -> Div {
    let (bg, _) = tone_colors(tone);
    div()
        .w(px(8.0))
        .h(px(8.0))
        .rounded(px(999.0))
        .bg(rgb(bg))
}

/// A square-ish outlined chip naming a category or format:
/// panel background, thin kit border, only slightly rounded,
/// 11px muted text. Unlike a badge it asserts nothing about
/// state — it just labels.
pub fn tag(text: &str) -> Div {
    div()
        .px(px(6.0))
        .py(px(2.0))
        .rounded(px(4.0))
        .bg(rgb(C_PANEL))
        .border_1()
        .border_color(rgb(C_BORDER))
        .text_size(px(11.0))
        .text_color(rgb(C_MUTED))
        .child(text.to_string())
}

#[cfg(test)]
mod tests {
    use super::{count_label, tone_colors, Tone};
    use crate::ui_kit::{C_ACCENT, C_CELL, C_ERROR, C_INK, C_MUTED};

    #[test]
    fn tone_colors_match_the_kit_palette() {
        assert_eq!(tone_colors(Tone::Neutral), (C_CELL, C_MUTED));
        assert_eq!(tone_colors(Tone::Accent), (C_ACCENT, C_INK));
        assert_eq!(tone_colors(Tone::Success), (0x2E7D4F, C_INK));
        assert_eq!(tone_colors(Tone::Warn), (0x8A6D1F, C_INK));
        assert_eq!(tone_colors(Tone::Error), (C_ERROR, C_INK));
    }

    #[test]
    fn tone_backgrounds_are_all_distinct() {
        let tones = [
            Tone::Neutral,
            Tone::Accent,
            Tone::Success,
            Tone::Warn,
            Tone::Error,
        ];
        let mut bgs: Vec<u32> = tones.iter().map(|t| tone_colors(*t).0).collect();
        bgs.sort();
        bgs.dedup();
        assert_eq!(bgs.len(), tones.len(), "each tone needs its own colour");
    }

    #[test]
    fn count_label_caps_at_ninety_nine_plus() {
        assert_eq!(count_label(0), "0");
        assert_eq!(count_label(7), "7");
        assert_eq!(count_label(99), "99");
        assert_eq!(count_label(100), "99+");
        assert_eq!(count_label(12_345), "99+");
    }
}
