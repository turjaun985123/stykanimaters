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
//!
//! [`workspace_segmented`] is the same joined pill at the shell's
//! workspace-chip scale — taller padding and the kit's `TEXT_SM`
//! type token — for the workspace selector in the app bar, where
//! the control is a primary navigation element rather than a
//! panel-local option. Everything else (colours, borders, joined
//! corners, caller-owned ids and listeners) is shared with
//! [`segmented`].

use gpui::*;

use crate::ui_kit::{C_ACCENT, C_BORDER, C_CELL, C_INK, SP_LG, TEXT_SM};

/// Corner radius of the group's outer corners, in pixels.
const RADIUS: f32 = 6.0;

/// Background of one segment: the accent for the active option,
/// the kit's neutral chip grey ([`C_CELL`], the inactive
/// `info_chip` fill) otherwise.
pub fn segment_bg(active: bool) -> u32 {
    if active {
        C_ACCENT
    } else {
        C_CELL
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

/// Workspace-selector variant of [`segmented`]: one styled
/// [`Div`] per label, forming the same joined pill, but at the
/// kit's workspace-chip metrics — the padding
/// `crate::ui_kit::workspace_chip` uses (`SP_LG` / 6px, against
/// [`segmented`]'s 8px / 4px) and the kit's `TEXT_SM` type size,
/// matching the chips the shell uses for top-level navigation.
///
/// Colours, borders and corner joining are identical to
/// [`segmented`] ([`segment_bg`] active fill, 1px [`C_BORDER`]
/// per segment, only the outer corners rounded), and the same
/// caller contract applies: place the segments in a gapless flex
/// row, then add each segment's id, cursor and click listener.
/// An out-of-range `active` leaves every segment inactive; an
/// empty `labels` slice yields an empty vec.
pub fn workspace_segmented(labels: &[&str], active: usize) -> Vec<Div> {
    let n = labels.len();
    labels
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let seg = div()
                .px(px(SP_LG))
                .py(px(6.0))
                .bg(rgb(segment_bg(i == active)))
                .border_1()
                .border_color(rgb(C_BORDER))
                .text_size(px(TEXT_SM))
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
    use super::{segment_bg, segmented, workspace_segmented};
    use crate::ui_kit::{C_ACCENT, C_CELL};

    #[test]
    fn segment_bg_uses_accent_only_when_active() {
        assert_eq!(segment_bg(true), C_ACCENT);
        // The kit's neutral chip grey for the inactive state.
        assert_eq!(segment_bg(false), C_CELL);
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

    #[test]
    fn workspace_segmented_returns_one_div_per_label() {
        // A Div's style is not inspectable from a unit test, so
        // the workspace metrics themselves are pinned by the
        // construction in workspace_segmented; what is testable
        // here is the shape of the result: one segment per label,
        // in label order.
        assert_eq!(workspace_segmented(&["Animate"], 0).len(), 1);
        assert_eq!(
            workspace_segmented(&["Animate", "Draw", "Build", "Graph"], 0).len(),
            4
        );
        assert_eq!(
            workspace_segmented(
                &["Animate", "Draw", "Build", "Graph", "Effects", "Audio", "3D", "Export"],
                7
            )
            .len(),
            8
        );
    }

    #[test]
    fn workspace_segmented_handles_empty_and_out_of_range() {
        // Same edge behaviour as segmented(): no labels -> no
        // segments (and no panic on n - 1); a stale active index
        // still yields every segment, all inactive.
        assert_eq!(workspace_segmented(&[], 0).len(), 0);
        assert_eq!(workspace_segmented(&["Animate", "Draw"], 2).len(), 2);
        assert_eq!(workspace_segmented(&["Animate", "Draw"], usize::MAX).len(), 2);
    }
}
