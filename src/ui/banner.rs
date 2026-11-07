//! A full-width inline notice bar: a status dot, a line of text
//! and a [`Tone`] background, for panels that need to say
//! something in place rather than in a toast.
//!
//! Toasts ([`crate::ui_toast`]) are transient and live in the
//! bottom bar; a banner is the same message pinned inside the
//! panel it belongs to — "weather is off", "no clips yet", "the
//! last save failed" — for as long as the state it describes
//! lasts. Presentation only, like the rest of the kit: plain
//! data in, a `Div` out, no handlers, no ids.
//!
//! Tones are not redefined here. The bar borrows [`Tone`],
//! [`tone_colors`] and [`status_dot`] from [`crate::ui_badge`],
//! so a banner, a badge and a dot describing the same state
//! always agree on colour. [`tone_for_status`] classifies raw
//! editor status strings into tones with exactly the rules the
//! app's toast feed uses (see `render` in `app.rs`), so a
//! message reads the same colour in both places. One visible
//! consequence of borrowing the palette: a `Neutral` banner's
//! dot is the same grey as the bar behind it, so it effectively
//! disappears — the neutral bar is just quiet text, which is
//! the point of neutral.

use crate::ui_badge::{status_dot, tone_colors, Tone};
use gpui::*;

/// An inline notice bar: full width, a status dot in `tone`,
/// then `text` at 12px in the tone's foreground over the tone's
/// background. The text is copied into an owned `String`
/// before it is stored in the tree.
pub fn banner(text: &str, tone: Tone) -> Div {
    let (bg, fg) = tone_colors(tone);
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .w_full()
        .px(px(8.0))
        .py(px(6.0))
        .rounded(px(6.0))
        .bg(rgb(bg))
        .text_size(px(12.0))
        .text_color(rgb(fg))
        .child(status_dot(tone))
        .child(text.to_string())
}

/// A [`banner`] in the neutral tone — the quiet default for
/// informational notices that assert nothing about state.
pub fn banner_plain(text: &str) -> Div {
    banner(text, Tone::Neutral)
}

/// Classify an editor status string into a [`Tone`], mirroring
/// the toast feed in `app.rs` exactly: the string is lowercased
/// first, so every check below is case-insensitive.
///
/// - contains "failed", "could not" or "error" → [`Tone::Error`]
///   (checked first, so an error word wins over a success
///   prefix);
/// - starts with "exported", "saved" or "imported" →
///   [`Tone::Success`];
/// - anything else → [`Tone::Neutral`] (the toast feed's
///   `Info` kind, which has no separate badge tone).
pub fn tone_for_status(status: &str) -> Tone {
    let lower = status.to_lowercase();
    if lower.contains("failed") || lower.contains("could not") || lower.contains("error") {
        Tone::Error
    } else if lower.starts_with("exported")
        || lower.starts_with("saved")
        || lower.starts_with("imported")
    {
        Tone::Success
    } else {
        Tone::Neutral
    }
}

#[cfg(test)]
mod tests {
    use super::{banner, banner_plain, tone_for_status};
    use crate::ui_badge::Tone;

    #[test]
    fn error_words_classify_as_error() {
        assert_eq!(tone_for_status("Export failed."), Tone::Error);
        assert_eq!(tone_for_status("Could not open the project."), Tone::Error);
        assert_eq!(tone_for_status("error: bad frame data"), Tone::Error);
    }

    #[test]
    fn error_classification_is_case_insensitive() {
        assert_eq!(tone_for_status("FAILED to write file"), Tone::Error);
        assert_eq!(tone_for_status("Could NOT load"), Tone::Error);
        assert_eq!(tone_for_status("An ERROR occurred"), Tone::Error);
    }

    #[test]
    fn success_prefixes_classify_as_success() {
        assert_eq!(tone_for_status("Exported animation.gif."), Tone::Success);
        assert_eq!(tone_for_status("Saved project."), Tone::Success);
        assert_eq!(tone_for_status("Imported figure.stk."), Tone::Success);
    }

    #[test]
    fn success_prefixes_are_case_insensitive_like_the_toast_feed() {
        // app.rs lowercases before the starts_with checks, so a
        // capitalised success word still classifies as success.
        assert_eq!(tone_for_status("EXPORTED 24 frames."), Tone::Success);
        assert_eq!(tone_for_status("Saved."), Tone::Success);
    }

    #[test]
    fn error_beats_a_success_prefix() {
        // The error contains-check runs first in app.rs.
        assert_eq!(tone_for_status("Saved, but an error occurred"), Tone::Error);
    }

    #[test]
    fn plain_messages_are_neutral() {
        assert_eq!(tone_for_status("Figure added."), Tone::Neutral);
        assert_eq!(tone_for_status("Weather is off."), Tone::Neutral);
        assert_eq!(tone_for_status(""), Tone::Neutral);
        // "saved" mid-string is not a prefix, so no success.
        assert_eq!(tone_for_status("Project saved later"), Tone::Neutral);
    }

    #[test]
    fn builders_construct() {
        // A built Div's styling is not inspectable from a unit
        // test; construction itself is the smoke check, the same
        // limitation the sibling components document.
        let _ = banner("Weather is off.", Tone::Neutral);
        let _ = banner("Save failed.", Tone::Error);
        let _ = banner_plain("3 clips on the timeline.");
    }
}
