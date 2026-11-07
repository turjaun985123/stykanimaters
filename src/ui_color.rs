//! Colour-picker building blocks for the segment colour panel.
//!
//! Data and presentation only — no state, no handlers, no editor
//! knowledge (the same contract as [`crate::ui_kit`]). The panel
//! in `app.rs` turns each palette entry into a clickable element
//! with id `("swatch", i)` whose listener calls
//! `EditorCore::set_node_color`, which writes the same
//! `NodeDef.color` field the "Seg colour" cycle button steps
//! through.

use gpui::*;

use crate::ui_kit::{C_ACCENT, C_BORDER};

/// The picker's fixed palette: 16 colours covering white/black,
/// the hues the segment-colour cycle already uses (red, blue,
/// green, amber, purple, near-black), the app accent, and a
/// spread of common drawing colours in between.
pub const PICKER_PALETTE: [u32; 16] = [
    0xFFFFFF, // white
    0x9AA3B2, // grey
    0x232830, // near-black (also in the cycle palette)
    0x000000, // black
    0xFF5D5D, // red (cycle)
    0xFF9F43, // orange
    0xFFC65D, // amber (cycle)
    0xFFE066, // yellow
    0x7BE495, // green (cycle)
    0x2EC4B6, // teal
    0x4DD0E1, // cyan
    0x5DB2FF, // blue (cycle)
    0x3D5A99, // app accent
    0xC792FF, // purple (cycle)
    0xFF7BAC, // pink
    0x8D6E63, // brown
];

/// One swatch cell: an 18px rounded square filled with `color`,
/// bordered with the accent colour when `active` (the selected
/// segment already has this colour) and the standard panel
/// border otherwise — the border keeps white and near-black
/// swatches visible against the dark panel. Callers add the id,
/// cursor and click listener.
pub fn swatch(color: u32, active: bool) -> Div {
    div()
        .w(px(18.0))
        .h(px(18.0))
        .rounded(px(4.0))
        .bg(rgb(color))
        .border_1()
        .border_color(rgb(if active { C_ACCENT } else { C_BORDER }))
}
