//! Colour readout for the segment colour panel: a large swatch
//! with the colour spelled out as a hex string and per-channel
//! values, so the panel can show *which* colour a segment carries,
//! not just offer swatches to pick from.
//!
//! Data and presentation only — no state, no handlers, no editor
//! knowledge (the same contract as [`crate::ui_kit`] and the
//! sibling picker blocks in `color.rs`). The conversions
//! ([`rgb_parts`], [`hex_string`]) are pure functions over the
//! codebase's `0xRRGGBB` colour representation.

use gpui::*;

use crate::ui_kit::{C_ACCENT, C_BORDER, C_INK, C_MUTED};

/// Split a `0xRRGGBB` colour into its red, green and blue
/// channels, each `0..=255`. Bits above the blue channel (there
/// are none in this codebase's colours) are ignored.
pub fn rgb_parts(rgb: u32) -> (u8, u8, u8) {
    (
        ((rgb >> 16) & 0xFF) as u8,
        ((rgb >> 8) & 0xFF) as u8,
        (rgb & 0xFF) as u8,
    )
}

/// The colour as a `"#RRGGBB"` string with uppercase hex digits,
/// zero-padded — the form colour fields and CSS use.
pub fn hex_string(rgb: u32) -> String {
    format!("#{:06X}", rgb & 0xFF_FFFF)
}

/// Width in px of a channel meter; a channel value fills this
/// much at 255.
const BAR_W: f32 = 64.0;

/// A mini meter for one channel value: a neutral track with an
/// accent fill covering `channel / 255` of its width, so 0 reads
/// empty and 255 reads full.
pub fn channel_bar(channel: u8) -> Div {
    let fill = BAR_W * (channel as f32 / 255.0);
    div()
        .flex()
        .w(px(BAR_W))
        .h(px(6.0))
        .rounded(px(999.0))
        .bg(rgb(0x2C313A))
        .child(
            div()
                .w(px(fill))
                .h(px(6.0))
                .rounded(px(999.0))
                .bg(rgb(C_ACCENT)),
        )
}

/// One channel row inside the readout: the muted `"R 255"`-style
/// label and value, then the channel's meter.
fn channel_row(label: &str, value: u8) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .child(
            div()
                .w(px(46.0))
                .text_size(px(11.0))
                .text_color(rgb(C_MUTED))
                .child(format!("{label} {value}")),
        )
        .child(channel_bar(value))
}

/// The full readout for one colour: a 40px swatch (rounded,
/// bordered so white and near-black stay visible against the
/// dark panel) beside a column with the hex string in ink and
/// the three channel rows under it.
pub fn color_readout(rgb: u32) -> Div {
    let (r, g, b) = rgb_parts(rgb);
    div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .child(
            div()
                .w(px(40.0))
                .h(px(40.0))
                .rounded(px(6.0))
                .bg(rgb(rgb))
                .border_1()
                .border_color(rgb(C_BORDER)),
        )
        .child(
            div()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(rgb(C_INK))
                        .child(hex_string(rgb)),
                )
                .child(channel_row("R", r))
                .child(channel_row("G", g))
                .child(channel_row("B", b)),
        )
}

#[cfg(test)]
mod tests {
    use super::{channel_bar, color_readout, hex_string, rgb_parts};

    #[test]
    fn rgb_parts_splits_the_channels() {
        assert_eq!(rgb_parts(0xFF8000), (255, 128, 0));
        assert_eq!(rgb_parts(0x000000), (0, 0, 0));
        assert_eq!(rgb_parts(0xFFFFFF), (255, 255, 255));
        // The app accent: exercises all three shifts at once.
        assert_eq!(rgb_parts(0x3D5A99), (61, 90, 153));
    }

    #[test]
    fn hex_string_is_uppercase_and_padded() {
        assert_eq!(hex_string(0xFF8000), "#FF8000");
        assert_eq!(hex_string(0x3D5A99), "#3D5A99");
        assert_eq!(hex_string(0x00000A), "#00000A");
        assert_eq!(hex_string(0x000000), "#000000");
    }

    #[test]
    fn hex_string_ignores_bits_above_the_blue_channel() {
        assert_eq!(hex_string(0xFF3D5A99), "#3D5A99");
    }

    #[test]
    fn readout_and_bars_construct_without_panicking() {
        // Div construction needs no window (the ui_toast tests
        // rely on the same fact); this pins the builders to stay
        // pure presentation.
        let _readout = color_readout(0xFF8000);
        let _empty = channel_bar(0);
        let _full = channel_bar(255);
    }
}
