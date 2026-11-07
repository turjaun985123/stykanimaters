//! Figure layer row shell for the sidebar.
//!
//! The sidebar lists one row per figure: a colour dot and name on
//! the left, the opacity readout, then a cluster of small per-figure
//! buttons (reorder, lock, solo, opacity steppers, visibility,
//! delete). `app.rs` used to hand-build that whole row inline for
//! every figure; this component owns only the row's *shell* — the
//! rounded container, its background state, the left/right split —
//! while the caller keeps building the contents. That split is
//! deliberate: every button in the cluster carries a click listener
//! bound to editor state, and listeners stay with the caller. The
//! caller composes its dot/name/opacity elements into one `leading`
//! element and hands the buttons over as `trailing`; the shell lays
//! them out and paints the background.
//!
//! Presentation only, like the rest of the kit: no state, no
//! handlers, no editor knowledge.

use gpui::*;

use crate::ui_kit::{C_ACCENT, C_PANEL};

/// Background colour for a layer row, as a GPUI **RGBA** word:
/// `0xRRGGBBAA`, the encoding [`rgba()`] expects (the same
/// `(colour << 8) | alpha` packing the canvas paint code uses).
/// Kit colours are plain `0xRRGGBB` values meant for [`rgb()`], so
/// they are shifted up and given a full alpha here — do not pass
/// this function's result to `rgb()`, it would read the alpha byte
/// as part of the colour.
///
/// - selected → the kit accent ([`C_ACCENT`]), fully opaque.
///   Selection wins over dimming: a hidden figure can still be the
///   selected one, and the selection must stay visible.
/// - dimmed → the kit panel colour ([`C_PANEL`]), fully opaque: a
///   quiet step up from the bare sidebar, for rows the caller marks
///   as inactive (a hidden figure, say).
/// - otherwise → `0x00000000`, fully transparent black: the row
///   paints nothing and the sidebar behind it shows through. This
///   is the one case that needs the alpha byte — `rgb()` has no way
///   to say "no background".
pub fn row_bg(selected: bool, dimmed: bool) -> u32 {
    if selected {
        (C_ACCENT << 8) | 0xFF
    } else if dimmed {
        (C_PANEL << 8) | 0xFF
    } else {
        0x0000_0000
    }
}

/// The row container: a rounded horizontal strip on the metrics the
/// hand-built sidebar rows already used (padding 6, radius 6, 4px
/// gaps, children centred vertically), backgrounded by [`row_bg`].
/// `leading` sits at the left (the caller typically composes the
/// colour dot, the clickable name and the opacity readout into one
/// row element); a flexible spacer pushes `trailing` — the caller's
/// per-figure button cluster, laid out left to right in order — to
/// the right edge.
pub fn layer_row_shell(
    selected: bool,
    dimmed: bool,
    leading: Div,
    trailing: Vec<Div>,
) -> Div {
    let mut cluster = div().flex().flex_row().items_center().gap(px(4.0));
    for el in trailing {
        cluster = cluster.child(el);
    }
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .p(px(6.0))
        .rounded(px(6.0))
        .bg(rgba(row_bg(selected, dimmed)))
        .child(leading)
        .child(div().flex_1())
        .child(cluster)
}

#[cfg(test)]
mod tests {
    use super::row_bg;
    use crate::ui_kit::{C_ACCENT, C_PANEL};

    #[test]
    fn selected_row_is_the_opaque_accent() {
        assert_eq!(row_bg(true, false), (C_ACCENT << 8) | 0xFF);
        // The accent itself, pinned: 0x3D5A99 with full alpha.
        assert_eq!(row_bg(true, false), 0x3D5A_99FF);
    }

    #[test]
    fn selection_wins_over_dimming() {
        assert_eq!(row_bg(true, true), row_bg(true, false));
    }

    #[test]
    fn dimmed_row_is_the_opaque_panel_colour() {
        assert_eq!(row_bg(false, true), (C_PANEL << 8) | 0xFF);
        assert_eq!(row_bg(false, true), 0x2226_2FFF);
    }

    #[test]
    fn plain_row_is_fully_transparent() {
        // 0xRRGGBBAA with alpha 0: paints nothing, in any colour
        // channel. This is why row_bg speaks RGBA, not RGB.
        assert_eq!(row_bg(false, false), 0x0000_0000);
        assert_eq!(row_bg(false, false) & 0xFF, 0);
    }

    #[test]
    fn opaque_states_carry_full_alpha() {
        assert_eq!(row_bg(true, false) & 0xFF, 0xFF);
        assert_eq!(row_bg(false, true) & 0xFF, 0xFF);
    }
}
