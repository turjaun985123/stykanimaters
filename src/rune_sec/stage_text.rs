//! Phase E, section 5/40 — STAGE TEXT RENDERING (geometry).
//! Declared from `main.rs` only (bin-crate code, not part of
//! the engine library).
//!
//! Pure stage-space geometry for every text label the export
//! rasterizer would draw at the core's display frame, so the
//! Rune stage can paint segment text and free-standing text
//! instances into its `Shape` (one filled quad per glyph
//! pixel). This module computes corners only — the caller maps
//! them through the stage view transform and fills them.
//!
//! ## Placement mirrors the rasterizer exactly
//!
//! Placement rules are `media::media_items`' (src/media.rs:97):
//!
//! - **Segment text** (`SegmentMedia::Text`, media.rs:137-149):
//!   the label is centred on the segment midpoint, rotated
//!   along the segment direction (`atan2` of the parent→node
//!   vector, y-down stage degrees), glyph height = the media's
//!   `size`, colour = `node.color.unwrap_or(fig.color)` — the
//!   media itself carries no colour. A root node's media sits
//!   on a notional segment from the joint along +x with the
//!   node's `length` (media.rs:113-116); a zero-length segment
//!   is skipped. ONE deliberate divergence: positions come
//!   from `EditorCore::positions_f` at `display_frame()` (the
//!   graph-aware evaluation the stage itself draws with)
//!   instead of `Project::poses_at`, and figures follow the
//!   stage's visibility (`layers::visible_figures` — hidden
//!   and non-solo figures draw no segments, so their text
//!   must not float alone). `media_items` skips hidden
//!   figures only; under solo the difference is visible.
//! - **Free-standing texts** (`Project.texts`, media.rs:180-
//!   200): the instance key is sampled with
//!   `InstanceKey::sample` (model.rs:645) — linear interp,
//!   CLAMPED outside the key range, `None` only for an empty
//!   key list. Label centre = key position, rotation = key
//!   angle, glyph height = `text.size * key.scale` (skipped
//!   when ≤ 0), colour = the instance's own `color` field.
//!
//! ## Glyph boxes mirror `composite_text`
//!
//! Glyph layout is `media::composite_text`'s (media.rs:307):
//! a 3×5 bitmap font, `cell = size / 5` stage units per glyph
//! pixel (media.rs:321), each character 4 cells wide (3 +
//! 1 gap) so the whole string spans `4n − 1` cells, centred
//! on the label position (media.rs:322-323). Each set pixel
//! becomes one [`TextPrim::GlyphBox`]: the cell×cell square
//! centred on the pixel centre — the stage-space form of the
//! block `composite_text` stamps. Pixel centres use its own
//! local offsets: `lx = (start + 4·char + col + 0.5) · cell`,
//! `ly = (row − 2 + 0.5) · cell`, rotated by the label angle
//! with the y-down rotation `composite_text` applies. Note
//! the inherited quirk in that formula: rows span local y
//! ∈ [−2·cell, +3·cell], so a label's box centre sits half a
//! cell BELOW its anchor point (horizontally it is exact).
//! Mirrored deliberately — stage text must land where the
//! export rasterizer puts it, not where looks tidier.
//!
//! The glyph table below is copied verbatim from
//! `media::glyph_rows` (media.rs:367) — the same mirroring
//! `pivot.rs`'s `bitmap_glyph_rows` already does — because
//! the original is private to `media.rs`. If the font ever
//! changes there, this copy (and pivot.rs's) must follow.

use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::{InstanceKey, SegmentMedia, V2};

/// One primitive of stage text geometry, in stage coordinates.
#[derive(Clone, Debug, PartialEq)]
pub enum TextPrim {
    /// One filled glyph pixel: a square (rotated with its
    /// label) whose corners are ordered like `media.rs`'s
    /// image quads — starting at the local (−x, −y) corner and
    /// walking the local +x edge first — filled with `color`
    /// (a `0xRRGGBB` engine colour).
    GlyphBox { corners: [V2; 4], color: u32 },
}

/// The glyph pixel grid: 3 columns × 5 rows per character.
const GLYPH_COLS: usize = 3;
const GLYPH_ROWS: usize = 5;
/// Character advance in cells: 3 glyph columns + 1 gap
/// (media.rs:322).
const CHAR_ADVANCE: f32 = 4.0;

/// Filled boxes for one label: `content` centred at `pos`,
/// running along `angle_deg` (stage-space degrees, y-down),
/// with glyph height `size` stage units, in `color`. Empty
/// content, a non-positive size, and all-blank glyphs (e.g.
/// spaces) yield no boxes — the same early-outs
/// `composite_text` makes (media.rs:307).
pub fn label_boxes(pos: V2, angle_deg: f32, content: &str, size: f32, color: u32) -> Vec<TextPrim> {
    let chars: Vec<char> = content.chars().collect();
    if chars.is_empty() || size <= 0.0 {
        return Vec::new();
    }
    let cell = size / GLYPH_ROWS as f32; // stage units per glyph pixel
    let total_units = chars.len() as f32 * CHAR_ADVANCE - 1.0; // 3 wide + 1 gap
    let start = -total_units / 2.0;
    let rad = angle_deg.to_radians();
    let (cs, sn) = (rad.cos(), rad.sin());
    // Local axes under the y-down rotation composite_text
    // applies: +x along the text run, +y down the glyph.
    let u: V2 = [cs, sn];
    let v: V2 = [-sn, cs];
    let half = cell / 2.0;
    let mut out = Vec::new();
    for (ci, ch) in chars.iter().enumerate() {
        let glyph = glyph_rows(*ch);
        for row in 0..GLYPH_ROWS {
            for col in 0..GLYPH_COLS {
                if (glyph[row] >> (2 - col)) & 1 == 0 {
                    continue;
                }
                let lx = (start + ci as f32 * CHAR_ADVANCE + col as f32 + 0.5) * cell;
                let ly = (row as f32 - 2.0 + 0.5) * cell;
                let centre = [
                    pos[0] + lx * u[0] + ly * v[0],
                    pos[1] + lx * u[1] + ly * v[1],
                ];
                let corners = [
                    [
                        centre[0] - u[0] * half - v[0] * half,
                        centre[1] - u[1] * half - v[1] * half,
                    ],
                    [
                        centre[0] + u[0] * half - v[0] * half,
                        centre[1] + u[1] * half - v[1] * half,
                    ],
                    [
                        centre[0] + u[0] * half + v[0] * half,
                        centre[1] + u[1] * half + v[1] * half,
                    ],
                    [
                        centre[0] - u[0] * half + v[0] * half,
                        centre[1] - u[1] * half + v[1] * half,
                    ],
                ];
                out.push(TextPrim::GlyphBox { corners, color });
            }
        }
    }
    out
}

/// Every text label visible at the core's display frame, as
/// glyph boxes in stage coordinates: segment text for every
/// stage-visible figure (in figure, then node order), then
/// the free-standing `Project.texts` instances (which the
/// rasterizer also draws last, on top).
pub fn text_geometry(core: &EditorCore) -> Vec<TextPrim> {
    let project = &core.project;
    let display = core.display_frame();
    let mut out = Vec::new();

    let visible = stykanimate_rs::layers::visible_figures(project, core.solo);
    for (fi, fig) in project.figures.iter().enumerate() {
        if !visible[fi] {
            continue;
        }
        let pos = core.positions_f(display, fi);
        for (ni, node) in fig.def.nodes.iter().enumerate() {
            let Some(SegmentMedia::Text { content, size }) = &node.media else {
                continue;
            };
            let (a, b) = match node.parent {
                Some(p) => (pos[p], pos[ni]),
                None => (pos[ni], [pos[ni][0] + node.length, pos[ni][1]]),
            };
            let ab = [b[0] - a[0], b[1] - a[1]];
            let len = (ab[0] * ab[0] + ab[1] * ab[1]).sqrt();
            if len <= f32::EPSILON {
                continue;
            }
            let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
            let angle_deg = ab[1].atan2(ab[0]).to_degrees();
            let color = node.color.unwrap_or(fig.color);
            out.extend(label_boxes(mid, angle_deg, content, *size, color));
        }
    }

    for text in &project.texts {
        let Some(key) = InstanceKey::sample(&text.keys, display) else {
            continue;
        };
        let size = text.size * key.scale;
        if size <= 0.0 {
            continue;
        }
        out.extend(label_boxes(
            [key.x, key.y],
            key.angle,
            &text.content,
            size,
            text.color,
        ));
    }
    out
}

/// 3x5 bitmap glyph for `ch`: five rows, 3 bits each, bit 2 is
/// the leftmost pixel. Lowercase folds to uppercase; unknown
/// printable characters fall back to `?`.
///
/// Verbatim copy of `media.rs`'s private `glyph_rows`
/// (media.rs:367) — see the module docs for why this mirrors
/// instead of importing.
fn glyph_rows(ch: char) -> [u8; 5] {
    let up = ch.to_ascii_uppercase();
    match up {
        'A' => [0b010, 0b101, 0b111, 0b101, 0b101],
        'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
        'C' => [0b011, 0b100, 0b100, 0b100, 0b011],
        'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
        'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
        'F' => [0b111, 0b100, 0b110, 0b100, 0b100],
        'G' => [0b011, 0b100, 0b101, 0b101, 0b011],
        'H' => [0b101, 0b101, 0b111, 0b101, 0b101],
        'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
        'J' => [0b001, 0b001, 0b001, 0b101, 0b010],
        'K' => [0b101, 0b101, 0b110, 0b101, 0b101],
        'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
        'M' => [0b101, 0b111, 0b111, 0b101, 0b101],
        'N' => [0b101, 0b111, 0b101, 0b101, 0b101],
        'O' => [0b010, 0b101, 0b101, 0b101, 0b010],
        'P' => [0b110, 0b101, 0b110, 0b100, 0b100],
        'Q' => [0b010, 0b101, 0b101, 0b010, 0b001],
        'R' => [0b110, 0b101, 0b110, 0b101, 0b101],
        'S' => [0b011, 0b100, 0b010, 0b001, 0b110],
        'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
        'U' => [0b101, 0b101, 0b101, 0b101, 0b111],
        'V' => [0b101, 0b101, 0b101, 0b101, 0b010],
        'W' => [0b101, 0b101, 0b101, 0b111, 0b101],
        'X' => [0b101, 0b101, 0b010, 0b101, 0b101],
        'Y' => [0b101, 0b101, 0b010, 0b010, 0b010],
        'Z' => [0b111, 0b001, 0b010, 0b100, 0b111],
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b110, 0b001, 0b010, 0b100, 0b111],
        '3' => [0b110, 0b001, 0b010, 0b001, 0b110],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b110, 0b001, 0b110],
        '6' => [0b011, 0b100, 0b110, 0b101, 0b010],
        '7' => [0b111, 0b001, 0b001, 0b010, 0b010],
        '8' => [0b010, 0b101, 0b010, 0b101, 0b010],
        '9' => [0b010, 0b101, 0b011, 0b001, 0b110],
        ' ' => [0, 0, 0, 0, 0],
        '.' => [0, 0, 0, 0, 0b010],
        ',' => [0, 0, 0, 0b010, 0b100],
        '!' => [0b010, 0b010, 0b010, 0, 0b010],
        '?' => [0b110, 0b001, 0b010, 0, 0b010],
        '\'' => [0b010, 0b010, 0, 0, 0],
        '"' => [0b101, 0b101, 0, 0, 0],
        '-' => [0, 0, 0b111, 0, 0],
        ':' => [0, 0b010, 0, 0b010, 0],
        ';' => [0, 0b010, 0, 0b010, 0b100],
        '(' => [0b001, 0b010, 0b010, 0b010, 0b001],
        ')' => [0b100, 0b010, 0b010, 0b010, 0b100],
        '+' => [0, 0b010, 0b111, 0b010, 0],
        '/' => [0b001, 0b001, 0b010, 0b100, 0b100],
        '&' => [0b010, 0b101, 0b010, 0b101, 0b011],
        '%' => [0b101, 0b001, 0b010, 0b100, 0b101],
        '*' => [0, 0b101, 0b010, 0b101, 0],
        '=' => [0, 0b111, 0, 0b111, 0],
        '<' => [0b001, 0b010, 0b100, 0b010, 0b001],
        '>' => [0b100, 0b010, 0b001, 0b010, 0b100],
        '_' => [0, 0, 0, 0, 0b111],
        '#' => [0b101, 0b111, 0b101, 0b111, 0b101],
        '$' => [0b011, 0b110, 0b010, 0b011, 0b110],
        '@' => [0b010, 0b101, 0b111, 0b100, 0b011],
        _ => {
            if ch.is_control() {
                [0, 0, 0, 0, 0]
            } else {
                glyph_rows('?')
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stykanimate_rs::model::TextInstance;

    /// Centre of one glyph box (average of its corners).
    fn box_centre(p: &TextPrim) -> V2 {
        let TextPrim::GlyphBox { corners, .. } = p;
        let mut c = [0.0, 0.0];
        for corner in corners {
            c[0] += corner[0] / 4.0;
            c[1] += corner[1] / 4.0;
        }
        c
    }

    fn bbox(prims: &[TextPrim]) -> (V2, V2) {
        let mut lo = [f32::INFINITY, f32::INFINITY];
        let mut hi = [f32::NEG_INFINITY, f32::NEG_INFINITY];
        for TextPrim::GlyphBox { corners, .. } in prims {
            for c in corners {
                lo[0] = lo[0].min(c[0]);
                lo[1] = lo[1].min(c[1]);
                hi[0] = hi[0].max(c[0]);
                hi[1] = hi[1].max(c[1]);
            }
        }
        (lo, hi)
    }

    fn colours(prims: &[TextPrim]) -> Vec<u32> {
        prims
            .iter()
            .map(|TextPrim::GlyphBox { color, .. }| *color)
            .collect()
    }

    /// The first real segment (parented, non-degenerate) of
    /// figure 0 in the demo project — its index and endpoints.
    fn first_segment(core: &EditorCore) -> (usize, V2, V2) {
        let pos = core.positions_f(core.display_frame(), 0);
        let fig = &core.project.figures[0];
        for (ni, node) in fig.def.nodes.iter().enumerate() {
            if let Some(p) = node.parent {
                let (a, b) = (pos[p], pos[ni]);
                if (b[0] - a[0]).abs() + (b[1] - a[1]).abs() > 1.0 {
                    return (ni, a, b);
                }
            }
        }
        panic!("demo figure has no real segment");
    }

    fn core_with_segment_text(content: &str, size: f32) -> (EditorCore, usize, V2, V2) {
        let mut core = EditorCore::new();
        let (ni, a, b) = first_segment(&core);
        core.project.figures[0].def.nodes[ni].media = Some(SegmentMedia::Text {
            content: content.into(),
            size,
        });
        (core, ni, a, b)
    }

    #[test]
    fn glyph_counts_match_the_bitmap_font() {
        // H = 2+2+3+2+2 = 11 lit pixels; I = 3+1+1+1+3 = 9.
        assert_eq!(label_boxes([0.0, 0.0], 0.0, "H", 25.0, 0).len(), 11);
        assert_eq!(label_boxes([0.0, 0.0], 0.0, "HI", 25.0, 0).len(), 20);
        // Case folds, unknown chars render as '?'
        // (? = 2+1+1+0+1 = 5).
        assert_eq!(label_boxes([0.0, 0.0], 0.0, "h", 25.0, 0).len(), 11);
        assert_eq!(label_boxes([0.0, 0.0], 0.0, "~", 25.0, 0).len(), 5);
    }

    #[test]
    fn box_count_scales_with_content_length() {
        let one = label_boxes([0.0, 0.0], 0.0, "HI", 25.0, 0).len();
        let two = label_boxes([0.0, 0.0], 0.0, "HIHI", 25.0, 0).len();
        assert!(one > 0);
        assert_eq!(two, 2 * one, "doubling the text doubles the boxes");
        // A space advances but stamps nothing.
        let spaced = label_boxes([0.0, 0.0], 0.0, "H H", 25.0, 0).len();
        assert_eq!(spaced, 2 * 11);
    }

    #[test]
    fn empty_or_degenerate_labels_yield_nothing() {
        assert!(label_boxes([0.0, 0.0], 0.0, "", 25.0, 0).is_empty());
        assert!(label_boxes([0.0, 0.0], 0.0, "HI", 0.0, 0).is_empty());
        assert!(label_boxes([0.0, 0.0], 0.0, "HI", -5.0, 0).is_empty());
        assert!(label_boxes([0.0, 0.0], 0.0, "   ", 25.0, 0).is_empty());
    }

    #[test]
    fn unrotated_boxes_are_cell_squares_centred_on_the_label() {
        // 'A' lights the full 3x5 extent, so the bbox is the
        // whole label: 3 cells wide, 5 tall. Horizontally it
        // is centred on pos; vertically it spans
        // [-2, +3] cells (composite_text's half-cell quirk,
        // see the module docs).
        let prims = label_boxes([100.0, 200.0], 0.0, "A", 25.0, 0x112233);
        assert_eq!(prims.len(), 10); // A = 1+2+3+2+2
        let (lo, hi) = bbox(&prims);
        assert!((lo[0] - 92.5).abs() < 1e-4 && (hi[0] - 107.5).abs() < 1e-4, "x span {lo:?}..{hi:?}");
        assert!((lo[1] - 190.0).abs() < 1e-4 && (hi[1] - 215.0).abs() < 1e-4, "y span {lo:?}..{hi:?}");
        // Every box is a 5x5 cell (size 25 / 5 rows).
        for TextPrim::GlyphBox { corners, .. } in &prims {
            let w = (corners[1][0] - corners[0][0]).abs();
            let h = (corners[3][1] - corners[0][1]).abs();
            assert!((w - 5.0).abs() < 1e-4 && (h - 5.0).abs() < 1e-4, "cell {w}x{h}");
        }
        assert!(colours(&prims).iter().all(|c| *c == 0x112233));
    }

    #[test]
    fn rotation_turns_the_label_run() {
        // At 90 degrees (y-down) the run goes downward: the
        // label's width becomes vertical extent.
        let prims = label_boxes([0.0, 0.0], 90.0, "HI", 25.0, 0);
        let (lo, hi) = bbox(&prims);
        let w = hi[0] - lo[0];
        let h = hi[1] - lo[1];
        // "HI" spans 7 cells wide x 5 tall unrotated.
        assert!((w - 25.0).abs() < 1e-3, "rotated width {w}");
        assert!((h - 35.0).abs() < 1e-3, "rotated height {h}");
        // The half-cell vertical quirk rotates with the
        // label: at 90 degrees the bbox centre shifts half a
        // cell along local +y, which points along -x here.
        let centre = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
        assert!((centre[0] - -2.5).abs() < 1e-3 && centre[1].abs() < 1e-3, "centre {centre:?}");
    }

    #[test]
    fn segment_text_sits_on_the_segment_midpoint() {
        let (core, _ni, a, b) = core_with_segment_text("HI", 25.0);
        let prims = text_geometry(&core);
        assert_eq!(prims.len(), 20, "H+I lit pixels");
        // Both glyphs touch the label's extremes, so the bbox
        // centre is the label centre: the segment midpoint,
        // shifted half a cell along the label's local +y
        // (composite_text's quirk, rotated with the segment).
        let (lo, hi) = bbox(&prims);
        let centre = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
        let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
        let ang = (b[1] - a[1]).atan2(b[0] - a[0]);
        let expected = [
            mid[0] - ang.sin() * 2.5, // 0.5 * cell (size 25 / 5)
            mid[1] + ang.cos() * 2.5,
        ];
        assert!(
            (centre[0] - expected[0]).abs() < 0.01 && (centre[1] - expected[1]).abs() < 0.01,
            "label centre {centre:?} vs expected {expected:?} (mid {mid:?})"
        );
        // Colour falls back to the figure colour (the demo
        // node carries no colour of its own).
        let fig_color = core.project.figures[0].color;
        assert!(colours(&prims).iter().all(|c| *c == fig_color));
    }

    #[test]
    fn segment_text_colour_prefers_the_node_colour() {
        let (mut core, ni, _, _) = core_with_segment_text("H", 25.0);
        core.project.figures[0].def.nodes[ni].color = Some(0x00FF00);
        let prims = text_geometry(&core);
        assert!(!prims.is_empty());
        assert!(colours(&prims).iter().all(|c| *c == 0x00FF00));
    }

    #[test]
    fn segment_text_empty_content_and_hidden_figure_yield_nothing() {
        let (core, _, _, _) = core_with_segment_text("", 25.0);
        assert!(text_geometry(&core).is_empty());
        let (mut core, _, _, _) = core_with_segment_text("HI", 25.0);
        core.project.figures[0].hidden = true;
        assert!(text_geometry(&core).is_empty());
    }

    #[test]
    fn text_instance_inside_its_key_range() {
        let mut core = EditorCore::new();
        core.project.texts.push(TextInstance {
            content: "A".into(),
            size: 20.0,
            color: 0x112233,
            keys: vec![
                InstanceKey { frame: 0.0, x: 50.0, y: 60.0, scale: 1.0, angle: 0.0 },
                InstanceKey { frame: 4.0, x: 90.0, y: 60.0, scale: 1.0, angle: 0.0 },
            ],
        });
        let prims = text_geometry(&core);
        assert_eq!(prims.len(), 10, "A lit pixels");
        assert!(colours(&prims).iter().all(|c| *c == 0x112233));
        // Display frame is 0: the label sits at the first
        // key, with the half-cell vertical quirk (+0.5 * 4).
        let (lo, hi) = bbox(&prims);
        let centre = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
        assert!((centre[0] - 50.0).abs() < 0.01 && (centre[1] - 62.0).abs() < 0.01, "centre {centre:?}");
        // One box centre sanity: boxes are 4 units (20 / 5).
        let c0 = box_centre(&prims[0]);
        assert!(c0[0] > 40.0 && c0[0] < 60.0 && c0[1] > 50.0 && c0[1] < 70.0);
    }

    #[test]
    fn text_instance_without_keys_draws_nothing() {
        // InstanceKey::sample returns None only for an empty
        // key list — that is the rasterizer's only "inactive"
        // state for a text instance.
        let mut core = EditorCore::new();
        core.project.texts.push(TextInstance {
            content: "A".into(),
            size: 20.0,
            color: 0x112233,
            keys: Vec::new(),
        });
        assert!(text_geometry(&core).is_empty());
    }

    #[test]
    fn text_instance_clamps_outside_its_key_range_like_the_rasterizer() {
        // Keys start at frame 3 but the display frame is 0:
        // sample() clamps to the first key (media.rs uses the
        // same call), so the label still renders, at the first
        // key's position and with size * key.scale.
        let mut core = EditorCore::new();
        core.project.texts.push(TextInstance {
            content: "A".into(),
            size: 20.0,
            color: 0x445566,
            keys: vec![InstanceKey { frame: 3.0, x: 70.0, y: 80.0, scale: 2.0, angle: 0.0 }],
        });
        let prims = text_geometry(&core);
        assert_eq!(prims.len(), 10);
        let (lo, hi) = bbox(&prims);
        let centre = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
        // Centre = key pos + half a cell (0.5 * 8) vertically.
        assert!((centre[0] - 70.0).abs() < 0.01 && (centre[1] - 84.0).abs() < 0.01, "centre {centre:?}");
        // Glyph height is 20 * 2 = 40, so the label is 40 tall.
        assert!((hi[1] - lo[1] - 40.0).abs() < 0.01, "height {}", hi[1] - lo[1]);
        // A non-positive scaled size is skipped, as in media.rs.
        core.project.texts[0].keys[0].scale = 0.0;
        assert!(text_geometry(&core).is_empty());
    }
}
