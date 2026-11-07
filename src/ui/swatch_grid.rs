//! Swatch-grid building blocks: the colour picker's swatches
//! laid out as a flat list or pre-chunked into rows.
//!
//! Data and presentation only — no state, no handlers, no editor
//! knowledge (the same contract as [`crate::ui_kit`] and the
//! colour picker's own [`crate::ui_color::swatch`]). Two shapes
//! are offered because callers want two different things:
//!
//! - [`swatch_grid`] returns one bare [`crate::ui_color::swatch`]
//!   per colour, in order. The caller attaches the id
//!   `("swatch", i)`, the cursor and the click listener to each
//!   one — exactly how the colour panel in `app.rs` already
//!   wires individual swatches — then arranges them as it likes.
//! - [`grid_rows`] / [`palette_grid`] return the swatches
//!   already chunked into flex rows, ready to drop into a panel
//!   as children. Pre-arranged rows cannot carry per-swatch ids
//!   or listeners (those must be attached before layout, and
//!   this module attaches none), so they suit read-only grids —
//!   previews, legends, "current palette" displays. Callers who
//!   need clickable swatches should lay out [`swatch_grid`]
//!   themselves.
//!
//! Which swatch is "current" is the caller's state in both
//! shapes: it passes the active index in on every render.

use gpui::*;

use crate::ui_color::{swatch, PICKER_PALETTE};

/// One swatch per colour, in colour order: the swatch at index
/// `i` is [`swatch`] with `active = (Some(i) == active)`.
///
/// An out-of-range `active` is not an error: it simply
/// highlights nothing, so a caller whose selection was just
/// removed can keep passing its stale index until it picks a
/// new one. `None` highlights nothing too. No ids or listeners
/// are attached here — the caller adds both, using the
/// `("swatch", i)` id convention from the colour panel.
pub fn swatch_grid(colours: &[u32], active: Option<usize>) -> Vec<Div> {
    colours
        .iter()
        .enumerate()
        .map(|(i, colour)| swatch(*colour, Some(i) == active))
        .collect()
}

/// How many rows [`grid_rows`] produces for `n` colours at
/// `per_row` per row: the ceiling of `n / per_row`, with
/// `per_row == 0` treated as 1 (one colour per row) and
/// `n == 0` producing no rows at all.
pub fn row_count(n: usize, per_row: usize) -> usize {
    n.div_ceil(per_row.max(1))
}

/// The swatches of [`swatch_grid`] chunked into flex rows of
/// at most `per_row` swatches each (gap 4, the kit's standard
/// spacing), in order, for direct use as panel children. The
/// last row holds the remainder and may be short. Active
/// highlighting follows the global colour index, not the
/// position within a row. A `per_row` of 0 is treated as 1 —
/// no panic, no infinite loop. Empty `colours` yields no rows.
pub fn grid_rows(colours: &[u32], active: Option<usize>, per_row: usize) -> Vec<Div> {
    let per_row = per_row.max(1);
    let mut rows = Vec::with_capacity(row_count(colours.len(), per_row));
    let mut start = 0;
    while start < colours.len() {
        let end = (start + per_row).min(colours.len());
        let mut row = div().flex().gap(px(4.0));
        for (offset, colour) in colours[start..end].iter().enumerate() {
            let i = start + offset;
            row = row.child(swatch(*colour, Some(i) == active));
        }
        rows.push(row);
        start = end;
    }
    rows
}

/// [`grid_rows`] over the picker's fixed palette
/// ([`PICKER_PALETTE`], 16 colours): at `per_row = 8` this is
/// the familiar two-row palette block.
pub fn palette_grid(active: Option<usize>, per_row: usize) -> Vec<Div> {
    grid_rows(&PICKER_PALETTE, active, per_row)
}

#[cfg(test)]
mod tests {
    use super::{grid_rows, palette_grid, row_count, swatch_grid};
    use crate::ui_color::PICKER_PALETTE;

    #[test]
    fn row_count_chunks_evenly_and_rounds_up() {
        assert_eq!(row_count(16, 8), 2);
        assert_eq!(row_count(16, 16), 1);
        assert_eq!(row_count(16, 5), 4);
        assert_eq!(row_count(1, 8), 1);
    }

    #[test]
    fn row_count_edge_cases() {
        assert_eq!(row_count(0, 8), 0);
        // per_row == 0 is treated as 1: one colour per row.
        assert_eq!(row_count(5, 0), 5);
        assert_eq!(row_count(0, 0), 0);
    }

    #[test]
    fn grid_returns_one_swatch_per_colour() {
        // Construction only — the caller owns layout and wiring.
        let grid = swatch_grid(&[0xFF0000, 0x00FF00, 0x0000FF], Some(1));
        assert_eq!(grid.len(), 3);
    }

    #[test]
    fn grid_with_no_colours_is_empty() {
        assert!(swatch_grid(&[], None).is_empty());
        assert!(swatch_grid(&[], Some(0)).is_empty());
    }

    #[test]
    fn grid_out_of_range_active_highlights_nothing_and_does_not_panic() {
        // Documented clamp: a stale index is legal and simply
        // yields an all-inactive grid.
        let grid = swatch_grid(&[0x111111, 0x222222], Some(9));
        assert_eq!(grid.len(), 2);
    }

    #[test]
    fn rows_chunk_sixteen_colours_by_eight_into_two_rows() {
        let colours: Vec<u32> = (0..16).collect();
        let rows = grid_rows(&colours, None, 8);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows.len(), row_count(colours.len(), 8));
    }

    #[test]
    fn rows_with_no_colours_is_empty() {
        assert!(grid_rows(&[], None, 8).is_empty());
    }

    #[test]
    fn rows_per_row_zero_behaves_as_one_per_row() {
        let rows = grid_rows(&[0x111111, 0x222222, 0x333333], None, 0);
        assert_eq!(rows.len(), 3);
    }

    #[test]
    fn palette_grid_covers_the_whole_palette() {
        assert_eq!(PICKER_PALETTE.len(), 16);
        assert_eq!(palette_grid(None, 8).len(), 2);
        assert_eq!(palette_grid(Some(0), 4).len(), 4);
        assert_eq!(palette_grid(None, 0).len(), 16);
    }
}
