//! Segmented control: a joined pill group for mutually exclusive
//! options (easing mode, weather kind, …).
//!
//! Presentation only — no state, no handlers, no editor knowledge
//! (the same contract as [`crate::ui_kit`] and the swatch pattern
//! in [`crate::ui_color`]). [`segmented`] returns one styled
//! [`Div`] per option, in option order; the caller places them in
//! a gapless flex row, then adds each segment's id, cursor and
//! click listener, e.g. `.id(("seg", i))` plus a listener that
//! writes the chosen index into the editor.
//!
//! Outline approach: the kit has no group-outline primitive, so
//! each segment carries its own 1px [`C_BORDER`] border and the
//! inactive fill ([`segment_bg`]) is the kit's neutral chip grey,
//! which contrasts with the [`C_PANEL`] background the group sits
//! on. Joined neighbours share an edge, so the doubled seam reads
//! as a divider and the row reads as one outlined pill. Only the
//! outer corners are rounded — first segment left side, last
//! segment right side — so the joined edges stay square.

use gpui::*;

use crate::ui_kit::{C_ACCENT, C_BORDER, C_INK};

/// Corner radius of the group's outer corners, in pixels.
const RADIUS: f32 = 6.0;

/// Background of one segment: the accent for the active option,
/// the kit's neutral chip grey (`0x2C313A`, the inactive
/// `info_chip` fill) otherwise.
pub fn segment_bg(active: bool) -> u32 {
    if active {
        C_ACCENT
    } else {
        0x2C313A
    }
}

/// One styled [`Div`] per option, in option order, forming a
/// joined pill group when placed in a gapless row.
///
/// `active` is the index of the selected option; an out-of-range
/// index leaves every segment inactive. An empty `options` slice
/// yields an empty vec. Callers add ids and listeners per segment
/// (see the module docs).
pub fn segmented(options: &[&str], active: usize) -> Vec<Div> {
    let n = options.len();
    options
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let seg = div()
                .px(px(8.0))
                .py(px(4.0))
                .bg(rgb(segment_bg(i == active)))
                .border_1()
                .border_color(rgb(C_BORDER))
                .text_size(px(12.0))
                .text_color(rgb(C_INK))
                .child(label.to_string());
            match (i, n) {
                // A lone segment is the whole pill: round it fully.
                (0, 1) => seg.rounded(px(RADIUS)),
                (0, _) => seg.rounded_tl(px(RADIUS)).rounded_bl(px(RADIUS)),
                (last, _) if last == n - 1 => {
                    seg.rounded_tr(px(RADIUS)).rounded_br(px(RADIUS))
                }
                // Middle segments stay square so the joins are flush.
                _ => seg,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{segment_bg, segmented};
    use crate::ui_kit::C_ACCENT;

    #[test]
    fn segment_bg_uses_accent_only_when_active() {
        assert_eq!(segment_bg(true), C_ACCENT);
        // The kit's neutral chip grey for the inactive state.
        assert_eq!(segment_bg(false), 0x2C313A);
        assert_ne!(segment_bg(true), segment_bg(false));
    }

    #[test]
    fn segmented_returns_one_div_per_option() {
        assert_eq!(segmented(&["a"], 0).len(), 1);
        assert_eq!(segmented(&["a", "b", "c"], 1).len(), 3);
        assert_eq!(segmented(&["a", "b", "c", "d", "e"], 4).len(), 5);
    }

    #[test]
    fn segmented_handles_empty_and_out_of_range() {
        // No options -> no segments (and no panic on n - 1).
        assert_eq!(segmented(&[], 0).len(), 0);
        // Out-of-range active still yields every segment; the
        // styling consequence (none active) follows from
        // segment_bg, which is covered above — a built Div's
        // style is not inspectable from a unit test.
        assert_eq!(segmented(&["a", "b"], 2).len(), 2);
        assert_eq!(segmented(&["a", "b"], usize::MAX).len(), 2);
    }
}
