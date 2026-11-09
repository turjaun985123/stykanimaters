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
//!
//! ## Bent segments: the label stays on the chord (export's rule)
//!
//! Both segment renderers arc a bent segment through
//! `model::bend_mid` as two pieces — the rasterizer at
//! export.rs:388, the stage at stage_view.rs:676
//! (`segment_pieces`). But the rasterizer's TEXT placement
//! has no bend term at all: `media_items` computes the
//! label's anchor and angle from the straight parent→node
//! chord over joint positions that bends never move
//! (export.rs's own comment at its bend split: "the joint
//! positions themselves don't move"). So in the shipped
//! output a label on a bent segment sits on the chord,
//! inside the arc — one rigid label, not two half-labels.
//! The stage mirrors that exactly (Wave 12, A7): segment
//! text does NOT follow `bend_mid` here, deliberately, so
//! stage and export place the label at the same point.
//! `bent_segment_text_stays_on_the_export_chord` pins it.
//!
//! ## Opacity: segment text carries the node's style opacity
//!
//! Segment-text glyph boxes carry the node's
//! `style.opacity` (clamped to 0..=1) as their prim alpha,
//! applied exactly once — here, in [`label_boxes_alpha`];
//! the stage painter applies a prim's alpha verbatim
//! (stage_view.rs:1196) and nothing else multiplies it.
//! This is a DELIBERATE divergence from the rasterizer,
//! which stamps text fully opaque (`composite_text` builds
//! `Rgba([…, 255])`, media.rs:326) — the stage fades a
//! segment's label with its segment, per the Wave 12
//! stage-fidelity directive. Figure opacity is NOT
//! multiplied in: export multiplies it into SEGMENT alpha
//! only, never into text, and the stage's figure fade is a
//! colour blend, not an alpha. Free-standing texts have no
//! node style (model.rs:675-680) and keep alpha 1.0. The
//! backing plates are unaffected — they stay at
//! [`BACKING_ALPHA`] whatever the glyph alpha.
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
//!
//! ## Backing plates (shell-only, NOT a rasterizer feature)
//!
//! Bare bitmap cells are hard to read over busy stage art,
//! so [`text_geometry`] wraps every label's glyph boxes in a
//! readability aid the export rasterizer does not have: each
//! glyph box is preceded in the output by a backing plate —
//! the same box scaled by [`BACKING_SCALE`] about its own
//! centre, filled with [`BACKING_COLOR`] at [`BACKING_ALPHA`]
//! (translucent black). Neighbouring plates overlap into one
//! continuous dark backing per glyph cluster, separating the
//! text from whatever is drawn behind it. [`label_boxes`] is
//! untouched by this and remains the exact `composite_text`
//! mirror; the plates exist only in `text_geometry`'s
//! output, so exported renders are unchanged and the glyph
//! boxes themselves (corners, order, colour) are identical
//! to what `label_boxes` produced before plates existed.

use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::{InstanceKey, SegmentMedia, V2};

/// One primitive of stage text geometry, in stage coordinates.
#[derive(Clone, Debug, PartialEq)]
pub enum TextPrim {
    /// One filled glyph pixel: a square (rotated with its
    /// label) whose corners are ordered like `media.rs`'s
    /// image quads — starting at the local (−x, −y) corner and
    /// walking the local +x edge first — filled with `color`
    /// (a `0xRRGGBB` engine colour) at opacity `alpha`
    /// (`1.0` = fully opaque; segment-text glyphs carry
    /// their node's `style.opacity`, free-standing texts
    /// carry 1.0, and the backing plates `text_geometry`
    /// interleaves carry [`BACKING_COLOR`]/[`BACKING_ALPHA`]
    /// instead — see the module docs).
    GlyphBox { corners: [V2; 4], color: u32, alpha: f32 },
}

/// The glyph pixel grid: 3 columns × 5 rows per character.
const GLYPH_COLS: usize = 3;
const GLYPH_ROWS: usize = 5;
/// Character advance in cells: 3 glyph columns + 1 gap
/// (media.rs:322).
const CHAR_ADVANCE: f32 = 4.0;

/// Backing-plate fill (shell-only readability aid — see the
/// module docs): black, so a plate darkens busy art behind
/// glyphs of any colour.
pub const BACKING_COLOR: u32 = 0x000000;
/// Backing-plate opacity: dark enough to separate the
/// glyphs from the art, light enough to keep the art
/// visible through the plate.
pub const BACKING_ALPHA: f32 = 0.55;
/// Backing-plate size relative to its glyph box, scaled
/// about the box centre: 1.5× extends the plate a quarter
/// cell past the glyph on every side (a cell box is 2 half-
/// extents wide), so plates of adjacent lit pixels overlap
/// into one continuous backing.
pub const BACKING_SCALE: f32 = 1.5;

/// Filled boxes for one label: `content` centred at `pos`,
/// running along `angle_deg` (stage-space degrees, y-down),
/// with glyph height `size` stage units, in `color`. Empty
/// content, a non-positive size, and all-blank glyphs (e.g.
/// spaces) yield no boxes — the same early-outs
/// `composite_text` makes (media.rs:307). The glyph boxes
/// are fully opaque; [`label_boxes_alpha`] is the same
/// layout with a caller-chosen glyph alpha.
pub fn label_boxes(pos: V2, angle_deg: f32, content: &str, size: f32, color: u32) -> Vec<TextPrim> {
    label_boxes_alpha(pos, angle_deg, content, size, color, 1.0)
}

/// [`label_boxes`] with every glyph box at opacity `alpha`
/// (clamped to 0..=1): the segment-text path passes the
/// node's `style.opacity` here — the single place that
/// opacity enters a text prim (see the module docs).
pub fn label_boxes_alpha(
    pos: V2,
    angle_deg: f32,
    content: &str,
    size: f32,
    color: u32,
    alpha: f32,
) -> Vec<TextPrim> {
    let alpha = alpha.clamp(0.0, 1.0);
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
                out.push(TextPrim::GlyphBox { corners, color, alpha });
            }
        }
    }
    out
}

/// `corners` scaled by `k` about their own centre (the
/// average of the four — glyph boxes are centre-symmetric,
/// so the average is the box centre).
fn scaled_about_centre(corners: [V2; 4], k: f32) -> [V2; 4] {
    let mut centre = [0.0, 0.0];
    for c in &corners {
        centre[0] += c[0] / 4.0;
        centre[1] += c[1] / 4.0;
    }
    corners.map(|c| [
        centre[0] + (c[0] - centre[0]) * k,
        centre[1] + (c[1] - centre[1]) * k,
    ])
}

/// Interleave backing plates into one label's glyph boxes:
/// every glyph box is preceded by its plate (same box,
/// scaled by [`BACKING_SCALE`] about its centre, in
/// [`BACKING_COLOR`] at [`BACKING_ALPHA`]), so a painter
/// walking the output in order draws plate, glyph, plate,
/// glyph. The glyph boxes pass through unchanged and in
/// order. Shell-only — see the module docs.
fn with_backing(boxes: Vec<TextPrim>) -> Vec<TextPrim> {
    let mut out = Vec::with_capacity(boxes.len() * 2);
    for prim in boxes {
        let TextPrim::GlyphBox { corners, .. } = &prim;
        out.push(TextPrim::GlyphBox {
            corners: scaled_about_centre(*corners, BACKING_SCALE),
            color: BACKING_COLOR,
            alpha: BACKING_ALPHA,
        });
        out.push(prim);
    }
    out
}

/// Every text label visible at the core's display frame, as
/// glyph boxes in stage coordinates: segment text for every
/// stage-visible figure (in figure, then node order), then
/// the free-standing `Project.texts` instances (which the
/// rasterizer also draws last, on top). Each label's glyph
/// boxes carry the shell-only backing plates described in
/// the module docs (see [`with_backing`]); the glyph boxes
/// themselves are exactly [`label_boxes`]' output, except
/// segment-text glyphs, which carry their node's
/// `style.opacity` as alpha (see the module docs).
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
            // Chord midpoint and chord angle, whatever this
            // node's bend: the rasterizer's placement rule,
            // mirrored deliberately (see "Bent segments" in
            // the module docs). The glyph alpha is the node's
            // style opacity — its one application (the painter
            // applies prim alpha verbatim).
            let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
            let angle_deg = ab[1].atan2(ab[0]).to_degrees();
            let color = node.color.unwrap_or(fig.color);
            let alpha = node.style.opacity.clamp(0.0, 1.0);
            out.extend(with_backing(label_boxes_alpha(
                mid, angle_deg, content, *size, color, alpha,
            )));
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
        out.extend(with_backing(label_boxes(
            [key.x, key.y],
            key.angle,
            &text.content,
            size,
            text.color,
        )));
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

    /// The glyph boxes of a `text_geometry` output with the
    /// backing plates dropped: glyphs are the opaque prims
    /// (plates carry `BACKING_ALPHA`). The rasterizer-parity
    /// assertions below run over this subset — plates are a
    /// shell-only aid layered on top of the same glyph
    /// stream.
    fn glyphs(prims: &[TextPrim]) -> Vec<TextPrim> {
        prims
            .iter()
            .filter(|p| matches!(p, TextPrim::GlyphBox { alpha, .. } if *alpha == 1.0))
            .cloned()
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
        assert_eq!(prims.len(), 40, "20 glyph pixels, each with a backing plate");
        let prims = glyphs(&prims);
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
        assert!(colours(&glyphs(&prims)).iter().all(|c| *c == 0x00FF00));
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
        assert_eq!(prims.len(), 20, "10 glyph pixels, each with a backing plate");
        let prims = glyphs(&prims);
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
        assert_eq!(prims.len(), 20, "10 glyph pixels, each with a backing plate");
        let prims = glyphs(&prims);
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

    #[test]
    fn backing_plate_precedes_every_glyph_with_the_same_centre() {
        // Rotated label, so the check runs on real geometry,
        // not just axis-aligned boxes.
        let boxes = label_boxes([10.0, 20.0], 30.0, "HI", 25.0, 0xAABBCC);
        let prims = with_backing(boxes.clone());
        assert_eq!(prims.len(), boxes.len() * 2);
        for (pair, glyph) in prims.chunks_exact(2).zip(boxes.iter()) {
            let (backing, g) = (&pair[0], &pair[1]);
            assert_eq!(g, glyph, "glyph prim passes through unchanged");
            let TextPrim::GlyphBox { color, alpha, .. } = backing;
            assert_eq!(*color, BACKING_COLOR, "plate colour");
            assert_eq!(*alpha, BACKING_ALPHA, "plate alpha");
            // Same centre: the plate is the glyph box grown
            // about its own centre.
            let (bc, gc) = (box_centre(backing), box_centre(g));
            assert!(
                (bc[0] - gc[0]).abs() < 1e-4 && (bc[1] - gc[1]).abs() < 1e-4,
                "plate centre {bc:?} vs glyph centre {gc:?}"
            );
        }
    }

    #[test]
    fn backing_plate_is_larger_by_a_quarter_cell_per_side() {
        // Unrotated, size 25: cell = 5, glyph box 5x5, plate
        // 7.5x7.5 at BACKING_SCALE 1.5 — 1.25 units of margin
        // on every side.
        let prims = with_backing(label_boxes([100.0, 200.0], 0.0, "A", 25.0, 0x112233));
        let TextPrim::GlyphBox { corners: plate, .. } = &prims[0];
        let TextPrim::GlyphBox { corners: glyph, .. } = &prims[1];
        let span = |cs: &[V2; 4]| [cs[1][0] - cs[0][0], cs[3][1] - cs[0][1]];
        let (gs, ps) = (span(glyph), span(plate));
        assert!((gs[0] - 5.0).abs() < 1e-4 && (gs[1] - 5.0).abs() < 1e-4, "glyph span {gs:?}");
        assert!((ps[0] - 7.5).abs() < 1e-4 && (ps[1] - 7.5).abs() < 1e-4, "plate span {ps:?}");
        // The extra size is split evenly around the glyph:
        // the plate's (-x, -y) corner sits 1.25 beyond the
        // glyph's on both axes.
        assert!((plate[0][0] - (glyph[0][0] - 1.25)).abs() < 1e-4, "plate lo x {}", plate[0][0]);
        assert!((plate[0][1] - (glyph[0][1] - 1.25)).abs() < 1e-4, "plate lo y {}", plate[0][1]);
    }

    #[test]
    fn text_geometry_interleaves_plates_then_glyphs() {
        let (core, _, _, _) = core_with_segment_text("A", 25.0);
        let prims = text_geometry(&core);
        assert_eq!(prims.len(), 20, "10 glyph pixels + 10 plates");
        for pair in prims.chunks_exact(2) {
            let TextPrim::GlyphBox { alpha: pa, color: pc, .. } = &pair[0];
            let TextPrim::GlyphBox { alpha: ga, .. } = &pair[1];
            assert_eq!((*pc, *pa), (BACKING_COLOR, BACKING_ALPHA), "plate first");
            assert_eq!(*ga, 1.0, "glyph second, opaque");
        }
        // The glyph subset keeps the label's own colour
        // (figure fallback here) — plates never recolour it.
        let fig_color = core.project.figures[0].color;
        assert!(colours(&glyphs(&prims)).iter().all(|c| *c == fig_color));
        // Empty labels stay empty: no plates without glyphs.
        assert!(with_backing(Vec::new()).is_empty());
    }

    #[test]
    fn bent_segment_text_stays_on_the_export_chord() {
        // The bend-aware position of a segment label is the
        // export rasterizer's: media_items places it on the
        // straight chord (module docs, "Bent segments"), even
        // though the segment itself arcs through bend_mid.
        // A bend on the frame must not move, rotate or
        // re-alpha a single glyph box.
        let (mut core, _ni, a, b) = core_with_segment_text("HI", 25.0);
        let straight = text_geometry(&core);
        assert!(!straight.is_empty());
        let node_count = core.project.figures[0].def.nodes.len();
        core.project.frames[0].bends = vec![vec![0.7; node_count]];
        // The fixture's bend is real: bend_mid leaves the
        // chord by several units on this segment, so a
        // bend-following label WOULD move measurably.
        let arc_mid = stykanimate_rs::model::bend_mid(a, b, 0.7);
        let chord_mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
        let off = ((arc_mid[0] - chord_mid[0]).powi(2)
            + (arc_mid[1] - chord_mid[1]).powi(2))
        .sqrt();
        assert!(off > 1.0, "bend_mid only {off} from the chord — fixture bend too weak");
        assert_eq!(text_geometry(&core), straight, "a bend must not change segment text");
        // And the anchor is the chord midpoint (plus the
        // half-cell quirk), exactly as in the unbent case.
        let prims = glyphs(&straight);
        let (lo, hi) = bbox(&prims);
        let centre = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
        let ang = (b[1] - a[1]).atan2(b[0] - a[0]);
        let expected = [
            chord_mid[0] - ang.sin() * 2.5,
            chord_mid[1] + ang.cos() * 2.5,
        ];
        assert!(
            (centre[0] - expected[0]).abs() < 0.01 && (centre[1] - expected[1]).abs() < 0.01,
            "bent label centre {centre:?} vs chord expectation {expected:?}"
        );
    }

    #[test]
    fn segment_text_alpha_is_the_node_style_opacity_applied_once() {
        let (mut core, ni, _, _) = core_with_segment_text("H", 25.0);
        // A free-standing text in the same project: it has no
        // node style, so its glyphs must stay opaque whatever
        // the segment's opacity does.
        core.project.texts.push(TextInstance {
            content: "A".into(),
            size: 20.0,
            color: 0xABCDEF,
            keys: vec![InstanceKey { frame: 0.0, x: 50.0, y: 60.0, scale: 1.0, angle: 0.0 }],
        });
        let seg_color = core.project.figures[0].def.nodes[ni]
            .color
            .unwrap_or(core.project.figures[0].color);
        // Glyph alphas for one label colour: glyphs are the
        // second prim of each (plate, glyph) pair.
        let alphas_for = |prims: &[TextPrim], color: u32| -> Vec<f32> {
            prims
                .chunks_exact(2)
                .map(|pair| &pair[1])
                .filter(|p| matches!(p, TextPrim::GlyphBox { color: c, .. } if *c == color))
                .map(|p| {
                    let TextPrim::GlyphBox { alpha, .. } = p;
                    *alpha
                })
                .collect()
        };
        // Default style: opaque.
        let prims = text_geometry(&core);
        let seg_alphas = alphas_for(&prims, seg_color);
        assert!(!seg_alphas.is_empty());
        assert!(seg_alphas.iter().all(|a| *a == 1.0), "default alphas {seg_alphas:?}");
        // Opacity 0.5: every segment glyph at exactly 0.5 —
        // not 0.25 (applied twice) and not 1.0 (dropped).
        core.project.figures[0].def.nodes[ni].style.opacity = 0.5;
        let prims = text_geometry(&core);
        let seg_alphas = alphas_for(&prims, seg_color);
        assert!(!seg_alphas.is_empty());
        assert!(seg_alphas.iter().all(|a| *a == 0.5), "segment alphas {seg_alphas:?}");
        // The free-standing text is untouched.
        let free_alphas = alphas_for(&prims, 0xABCDEF);
        assert!(!free_alphas.is_empty());
        assert!(free_alphas.iter().all(|a| *a == 1.0), "free-text alphas {free_alphas:?}");
        // Plates keep their own alpha throughout.
        assert!(prims.chunks_exact(2).all(|pair| matches!(
            &pair[0],
            TextPrim::GlyphBox { alpha, .. } if *alpha == BACKING_ALPHA
        )));
        // Out-of-range stored opacity clamps, as the segment
        // renderers clamp theirs.
        core.project.figures[0].def.nodes[ni].style.opacity = 1.4;
        let prims = text_geometry(&core);
        assert!(alphas_for(&prims, seg_color).iter().all(|a| *a == 1.0));
        core.project.figures[0].def.nodes[ni].style.opacity = -0.2;
        let prims = text_geometry(&core);
        assert!(alphas_for(&prims, seg_color).iter().all(|a| *a == 0.0));
    }
}
