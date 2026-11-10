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
//! - **Clip-def fallback** (Wave 14, C2): a movieclip
//!   instance whose own node carries no media reads the
//!   same node index on its clip's def — the funnel's
//!   per-node fallback (`media::media_items`, Wave 14 C1;
//!   `docs/wave13/CLIP_MEDIA.md` §6.1), mirrored here
//!   because this pass re-derives segment text instead of
//!   consuming the funnel (whose text items carry no
//!   alpha — see "Opacity" below). The carrier node
//!   supplies the media, the colour and the style
//!   opacity; placement stays on the instance's segment.
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
//!
//! ## Sprite name labels (shell-only, NOT a rasterizer feature)
//!
//! A sprite instance reaches the stage as a placeholder
//! outline — Rune `Shape` has no bitmap primitive, so the
//! quad the sprite stage pass draws (the same corners
//! `media::media_items` computes for the rasterizer) would
//! be anonymous without a name. [`sprite_label_boxes`] lays
//! the asset's name over its quad with this module's own
//! layout — no second text engine: the label is centred on
//! the quad and runs along its top edge (corners[0] →
//! corners[1], the quad's local +x, so the name rotates
//! with the sprite), at a glyph height fitted to the quad
//! — small enough that the label's run fits inside the top
//! edge, and never more than
//! [`SPRITE_LABEL_HEIGHT_FRACTION`] of the quad's height.
//! Its output is [`text_geometry`]'s stream shape —
//! backing plates interleaved with fully opaque glyph
//! boxes (a sprite instance has no opacity channel, the
//! same 1.0 `media_items` gives its image) — so a painter
//! can append it to the same prim list. The rasterizer
//! draws the actual image and no name; the label exists
//! only so stage placeholders stay identifiable, the same
//! honesty rule as the segment-media placeholder outlines.
//!
//! ## Subtitles (Wave P, P28)
//!
//! [`subtitle_geometry`] renders the frame's live subtitle
//! cues (`Project::subtitles` under the half-open liveness
//! rule, in stored order) through this module's own glyph
//! machinery — the stage is the WYSIWYG promise for the
//! exports, so the placement comes from the shared
//! layout source in `subtitles.rs` (Wave P, P23):
//! `subtitle_font_size`, `subtitle_line_height`,
//! `subtitle_stack_line_offset` and
//! `subtitle_line_center_y` decide the font size, the line
//! step, each line's distance from the anchored edge and
//! its centre height, and `live_subtitle_cues` decides
//! which cues show. The SVG writer's inline arithmetic
//! reproduces these helpers constant-for-constant (its
//! helper swap is IA-P27-1), so stage and SVG cannot
//! drift apart in where a line sits; the raster burn-in
//! still sizes its stamps proportionally (6% of stage
//! height) until IA-P26-1 lands the same helpers there —
//! the stacking order already agrees on all three
//! surfaces. [`subtitle_bands`] is the factored,
//! unit-pinned composition of those helpers for this
//! stage.
//!
//! The glyphs themselves are [`label_boxes`] output at the
//! shared font size in the style's text colour, anchored
//! at the band centre — so they inherit `composite_text`'s
//! half-cell vertical quirk exactly as every other
//! stage/raster text pair in this module does (the glyph
//! block sits half a cell low in its band; the rasterizer
//! stamps it the same way from the same anchor). The
//! backing follows the style's settled cross-surface
//! semantics (`subtitles.rs`'s style note: NO surface draws
//! a filled backing box — `box_color` is the backing /
//! outline colour): each glyph box is preceded by a plate
//! in the style's box colour — this module's own backing
//! device ([`with_backing`]'s scaled plate), recoloured
//! from the fixed black to the project's box colour and
//! fully opaque, the bitmap analogue of SVG's stroke halo
//! and the raster's offset stamps. Both colours are fully
//! opaque — the style carries no alpha channel.
//!
//! Nothing clips the stack to the stage: a stack taller
//! than the stage paints past the opposite edge, the same
//! unsettled overflow the other surfaces have (the raster
//! clips only because a bitmap ends). A frame with no live
//! cue yields no prims at all, so cue-free projects draw
//! exactly as before.

use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::{InstanceKey, SegmentMedia, V2};
use stykanimate_rs::subtitles::{
    live_subtitle_cues, subtitle_font_size, subtitle_line_center_y, subtitle_line_height,
    subtitle_stack_line_offset, SubtitleCue, SubtitlePosition, SubtitleStyle,
};

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

/// Sprite name-label height cap, as a fraction of the
/// quad's height (its corners[0] → corners[3] edge): the
/// label names the quad, it must not fill it — see
/// [`sprite_label_boxes`].
pub const SPRITE_LABEL_HEIGHT_FRACTION: f32 = 0.5;

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

/// The name label for one sprite instance's stage quad:
/// `name` (the asset's name) centred on `corners`, running
/// along the quad's top edge — the sprite stage pass's
/// entry point into this module's label machinery (see
/// "Sprite name labels" in the module docs).
///
/// `corners` are a `media::media_items` image quad:
/// corners[0] is the local (−x, −y) corner, corners[1]
/// the (+x, −y) corner, so the top edge corners[0] →
/// corners[1] is the quad's local +x (the sprite's own
/// rotation direction) and corners[0] → corners[3] its
/// height. The glyph height is fitted to the quad: the
/// label spans `4n − 1` cells at `cell = size / 5`
/// ([`label_boxes`]' layout), so keeping its run inside
/// the top edge caps the size at
/// `5 · top_len / (4n − 1)`, and the height is separately
/// capped at [`SPRITE_LABEL_HEIGHT_FRACTION`] of the
/// quad's height — whichever cap is smaller wins. The
/// colour is the caller's (the sprite pass owns its
/// placeholder palette); glyph boxes are fully opaque.
///
/// Empty names, blank-only names and degenerate quads
/// (a zero-length top edge or height) yield no prims —
/// the same early-outs [`label_boxes`] makes, reached
/// through a fitted size of 0.
pub fn sprite_label_boxes(corners: [V2; 4], name: &str, color: u32) -> Vec<TextPrim> {
    let n = name.chars().count();
    if n == 0 {
        return Vec::new();
    }
    let mut centre = [0.0, 0.0];
    for c in &corners {
        centre[0] += c[0] / 4.0;
        centre[1] += c[1] / 4.0;
    }
    let run = [
        corners[1][0] - corners[0][0],
        corners[1][1] - corners[0][1],
    ];
    let top_len = (run[0] * run[0] + run[1] * run[1]).sqrt();
    let side = [
        corners[3][0] - corners[0][0],
        corners[3][1] - corners[0][1],
    ];
    let side_len = (side[0] * side[0] + side[1] * side[1]).sqrt();
    let angle_deg = run[1].atan2(run[0]).to_degrees();
    let total_units = n as f32 * CHAR_ADVANCE - 1.0;
    let fit = GLYPH_ROWS as f32 * top_len / total_units;
    let size = fit.min(side_len * SPRITE_LABEL_HEIGHT_FRACTION);
    with_backing(label_boxes(centre, angle_deg, name, size, color))
}

/// Every text label visible at the core's display frame, as
/// glyph boxes in stage coordinates: segment text for every
/// stage-visible figure (in figure, then node order —
/// including clip-def text via the per-node fallback, see
/// "Clip-def fallback" in the module docs), then
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
            // Wave 14, C2 — resolve the CARRIER node: the
            // instance's own node when it carries media,
            // else the same node index on the clip's def
            // (the funnel's per-node fallback, mirrored —
            // see "Clip-def fallback" in the module docs).
            // A missing clip or an out-of-range clip node
            // simply has no fallback. A carrier whose media
            // is an image yields no text below — the image
            // is the stage placeholder pass's business.
            let carrier = match &node.media {
                Some(_) => Some(node),
                None => fig
                    .clip
                    .and_then(|ci| project.clips.get(ci))
                    .and_then(|clip| clip.def.nodes.get(ni)),
            };
            let Some(carrier) = carrier else { continue };
            let Some(SegmentMedia::Text { content, size }) = &carrier.media else {
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
            // the module docs). The glyph alpha is the
            // CARRIER node's style opacity — its one
            // application (the painter applies prim alpha
            // verbatim) — and the colour is the carrier's,
            // falling back to the figure's (CLIP_MEDIA
            // §6.2 for clip-sourced text).
            let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
            let angle_deg = ab[1].atan2(ab[0]).to_degrees();
            let color = carrier.color.unwrap_or(fig.color);
            let alpha = carrier.style.opacity.clamp(0.0, 1.0);
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

/// One laid-out subtitle line: the band it occupies and
/// everything the painter needs to draw the line's glyphs,
/// in stage coordinates. Produced by [`subtitle_bands`]
/// (the factored composition of `subtitles.rs`'s shared
/// layout helpers — see the module docs), consumed by
/// [`subtitle_geometry`].
#[derive(Clone, Debug, PartialEq)]
pub struct SubtitleBand {
    /// The line's text (one line of a cue's text).
    pub text: String,
    /// Centre of the line's band: the stage's horizontal
    /// centre, and `subtitle_line_center_y`'s per-anchor
    /// centre height. The glyph anchor [`label_boxes`] is
    /// called with.
    pub center: V2,
    /// The band rect the line occupies, ordered like a
    /// glyph box's corners (starting at the (−x, −y)
    /// corner, walking the +x edge first): one line height
    /// tall, centred on `center`, and as wide as the line's
    /// bitmap run plus half a font size of padding per
    /// side. This is the line's layout extent — the region
    /// a pixel proof probes — NOT a drawn fill: no surface
    /// fills the backing box (see the module docs); the
    /// drawn backing is the per-glyph plate stream
    /// [`subtitle_geometry`] emits inside this extent.
    pub corners: [V2; 4],
    /// Glyph height for the line: `subtitle_font_size` of
    /// the style (base × sanitised scale).
    pub font_size: f32,
    /// The band height: `subtitle_line_height` of the
    /// style (base × sanitised scale).
    pub line_height: f32,
    /// The style's text colour (the glyphs' fill).
    pub text_color: u32,
    /// The style's box colour (the backing plates' fill).
    pub box_color: u32,
}

/// The shared subtitle layout, composed for the stage:
/// every line of every cue in `cues` (already filtered to
/// the frame's live cues, in stored order) laid out on a
/// `stage`-sized picture under `style`, in cue order,
/// lines in text order. Pure maths — this is the
/// unit-pinned half of the stage subtitle draw. Every
/// placement value comes from `subtitles.rs`'s helpers
/// (`subtitle_font_size`, `subtitle_line_height`,
/// `subtitle_stack_line_offset`, `subtitle_line_center_y`
/// — scale sanitisation included), so the stage's bands
/// are the SVG exporter's bands by construction (the
/// raster burn-in joins them via IA-P26-1); the only
/// stage-local maths is the band WIDTH, which the bitmap
/// font fixes: a line's run is `4n − 1` cells at
/// `font_size / 5` per cell ([`label_boxes`]' layout),
/// plus half a font size of padding per side.
///
/// A cue whose text has no lines (empty text) contributes
/// nothing and does not advance the stack — `"".lines()`
/// is empty, the same split the SVG writer makes.
pub fn subtitle_bands(
    stage: [f32; 2],
    style: &SubtitleStyle,
    cues: &[&SubtitleCue],
) -> Vec<SubtitleBand> {
    let font_size = subtitle_font_size(*style);
    let line_height = subtitle_line_height(*style);
    let cx = stage[0] / 2.0;
    let cell = font_size / GLYPH_ROWS as f32;
    let mut bands = Vec::new();
    let mut lines_before = 0usize;
    for cue in cues {
        let lines: Vec<&str> = cue.text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let position: SubtitlePosition = style.position;
            let from_edge = subtitle_stack_line_offset(lines_before, lines.len(), i, position);
            let y = subtitle_line_center_y(*style, stage[1], from_edge);
            let n = line.chars().count() as f32;
            let run = if n > 0.0 {
                (n * CHAR_ADVANCE - 1.0) * cell
            } else {
                0.0
            };
            let half_w = (run + font_size) / 2.0;
            let half_h = line_height / 2.0;
            bands.push(SubtitleBand {
                text: (*line).to_string(),
                center: [cx, y],
                corners: [
                    [cx - half_w, y - half_h],
                    [cx + half_w, y - half_h],
                    [cx + half_w, y + half_h],
                    [cx - half_w, y + half_h],
                ],
                font_size,
                line_height,
                text_color: style.text_color,
                box_color: style.box_color,
            });
        }
        lines_before += lines.len();
    }
    bands
}

/// Interleave subtitle backing plates into one line's
/// glyph boxes: every glyph box is preceded by its plate —
/// the same box scaled by [`BACKING_SCALE`] about its own
/// centre, in the style's `box_color`, fully opaque.
/// [`with_backing`] is the same device for labels, with
/// the shell's fixed translucent black; the subtitle
/// backing is project data (the cross-surface backing /
/// outline colour — see the module docs), so the colour
/// and opacity are the style's, verbatim.
fn with_subtitle_backing(boxes: Vec<TextPrim>, box_color: u32) -> Vec<TextPrim> {
    let mut out = Vec::with_capacity(boxes.len() * 2);
    for prim in boxes {
        let TextPrim::GlyphBox { corners, .. } = &prim;
        out.push(TextPrim::GlyphBox {
            corners: scaled_about_centre(*corners, BACKING_SCALE),
            color: box_color,
            alpha: 1.0,
        });
        out.push(prim);
    }
    out
}

/// Every subtitle prim visible at the core's display frame,
/// in stage coordinates: for each live cue's every line, in
/// stored order, the line's glyph boxes (the shared
/// [`label_boxes`] layout at the shared font size, in the
/// style's text colour, fully opaque), each preceded by
/// its backing plate in the style's box colour (see
/// [`with_subtitle_backing`]). The stage painter fills
/// them in stream order, so every plate lands under its own
/// glyph and the lines stack exactly as [`subtitle_bands`]
/// laid them out.
///
/// Liveness is `live_subtitle_cues` over the frame the
/// display frame falls in (`display_frame` floored, the
/// core's own conversion in `advance_play`) — the
/// contract's half-open rule. A project with no cues, a
/// frame with no live cue, and a degenerate stage (a
/// non-positive dimension — there is no picture to pin
/// furniture to) all yield an empty vec, so those frames
/// draw exactly as they did before subtitles.
pub fn subtitle_geometry(core: &EditorCore) -> Vec<TextPrim> {
    let project = &core.project;
    if project.subtitles.is_empty() || project.stage[0] <= 0.0 || project.stage[1] <= 0.0 {
        return Vec::new();
    }
    let frame = core.display_frame().floor() as usize;
    let live = live_subtitle_cues(project, frame);
    let mut out = Vec::new();
    for band in subtitle_bands(project.stage, &project.subtitle_style, &live) {
        out.extend(with_subtitle_backing(
            label_boxes(band.center, 0.0, &band.text, band.font_size, band.text_color),
            band.box_color,
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

    /// Wave 14, C2 — turn a `core_with_segment_text` core
    /// into the clip-def-only arrangement: the figure
    /// becomes a movieclip instance of a clip whose def is
    /// a clone of the figure's own (media included), and
    /// the instance's own node media is then cleared, so
    /// the text exists ONLY on the clip def. The clip's
    /// single frame pose is the figure's timeline pose, so
    /// the instance evaluates to identical geometry.
    fn move_segment_text_to_clip_def(core: &mut EditorCore, ni: usize) {
        let def = core.project.figures[0].def.clone();
        let pose = core.project.frames[0].poses[0].clone();
        core.project.clips.push(stykanimate_rs::model::ClipDef {
            name: "clip".into(),
            def,
            frames: vec![pose],
            loop_mode: stykanimate_rs::model::ClipLoop::Loop,
        });
        core.project.figures[0].clip = Some(0);
        core.project.figures[0].def.nodes[ni].media = None;
    }

    /// The glyph prims of a `text_geometry` output — every
    /// GlyphBox that is not a backing plate (plates carry
    /// BACKING_ALPHA; glyphs carry their own alpha, which
    /// the clip tests below deliberately set to 0.5, so
    /// the `glyphs` helper's alpha == 1.0 filter cannot
    /// be reused here).
    fn glyph_prims(prims: &[TextPrim]) -> Vec<&TextPrim> {
        prims
            .iter()
            .filter(|p| matches!(p, TextPrim::GlyphBox { alpha, .. } if *alpha != BACKING_ALPHA))
            .collect()
    }

    #[test]
    fn clip_def_text_draws_on_stage_exactly_like_figure_def_text() {
        // Wave 14, C2: clip-def TEXT media was the one
        // stage gap the funnel fix (C1) could not close —
        // this pass re-derives segment text from the
        // figure defs, so the fallback is mirrored in
        // `text_geometry` itself. Text that exists only
        // on the clip def must produce the identical prim
        // stream to the same text on the instance's def.
        let (mut core, ni, _a, _b) = core_with_segment_text("HI", 25.0);
        let instance_drawn = text_geometry(&core);
        assert!(!instance_drawn.is_empty(), "the fixture's label draws");
        move_segment_text_to_clip_def(&mut core, ni);
        assert_eq!(
            text_geometry(&core),
            instance_drawn,
            "clip-def text draws glyph-for-glyph like figure-def text"
        );
        // The CARRIER supplies colour and alpha (CLIP_MEDIA
        // §6.2): the clip node's values appear; the
        // media-free instance node's colour must NOT leak
        // in, even when set.
        core.project.clips[0].def.nodes[ni].color = Some(0x00FF00);
        core.project.clips[0].def.nodes[ni].style.opacity = 0.5;
        core.project.figures[0].def.nodes[ni].color = Some(0xFF0000);
        let geometry = text_geometry(&core);
        let glyphs = glyph_prims(&geometry);
        assert!(!glyphs.is_empty(), "clip-def text still draws");
        assert!(
            glyphs.iter().all(|p| matches!(p, TextPrim::GlyphBox { color, .. } if *color == 0x00FF00)),
            "every glyph carries the clip node's colour"
        );
        assert!(
            glyphs.iter().all(|p| matches!(p, TextPrim::GlyphBox { alpha, .. } if *alpha == 0.5)),
            "every glyph carries the clip node's style opacity"
        );
        // The instance wins when it carries media again —
        // the funnel's rule, mirrored (§6.1): its colour
        // and its own (default) style opacity return.
        core.project.figures[0].def.nodes[ni].media = Some(SegmentMedia::Text {
            content: "HI".into(),
            size: 25.0,
        });
        let geometry = text_geometry(&core);
        let glyphs = glyph_prims(&geometry);
        assert!(!glyphs.is_empty(), "the instance's text draws again");
        assert!(
            glyphs.iter().all(|p| matches!(p, TextPrim::GlyphBox { color, alpha, .. } if *color == 0xFF0000 && *alpha == 1.0)),
            "the instance node's colour and opacity win"
        );
    }

    #[test]
    fn media_free_clip_adds_no_stage_text() {
        // The absent side: a plain figure with a media-free
        // clip attached draws exactly what it drew with no
        // clip at all — attaching a clip must not invent
        // stage text.
        let mut core = EditorCore::new();
        let clean = text_geometry(&core);
        let def = core.project.figures[0].def.clone();
        let pose = core.project.frames[0].poses[0].clone();
        core.project.clips.push(stykanimate_rs::model::ClipDef {
            name: "clip".into(),
            def,
            frames: vec![pose],
            loop_mode: stykanimate_rs::model::ClipLoop::Loop,
        });
        core.project.figures[0].clip = Some(0);
        assert_eq!(text_geometry(&core), clean);
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

    /// The `media_items` sprite quad for a 100×50 image
    /// centred at (150, 100), unrotated (the corner values
    /// media.rs's own tests pin): corners[0] is the local
    /// (−x, −y) corner and the top edge runs along +x.
    const SPRITE_QUAD: [V2; 4] = [[100.0, 75.0], [200.0, 75.0], [200.0, 125.0], [100.0, 125.0]];

    #[test]
    fn sprite_label_is_the_shared_layout_centred_on_the_quad() {
        // A = 10 lit pixels, B = 10: 20 glyphs + 20 plates.
        let prims = sprite_label_boxes(SPRITE_QUAD, "AB", 0x112233);
        assert_eq!(prims.len(), 40);
        // The size rule, read back: the width fit
        // (5 · 100 / 7 ≈ 71.4) loses to the height cap
        // (50 · 0.5 = 25), so the output is EXACTLY the
        // shared layout at the quad centre, angle 0 (the
        // top edge's direction), size 25, plates
        // interleaved — no second text engine involved.
        assert_eq!(
            prims,
            with_backing(label_boxes([150.0, 100.0], 0.0, "AB", 25.0, 0x112233))
        );
        // Geometry read-back over the glyph subset: both
        // chars touch the label's extremes, so the bbox
        // centre is the quad centre plus the half-cell
        // quirk along local +y (down, unrotated).
        let g = glyphs(&prims);
        assert_eq!(g.len(), 20);
        let (lo, hi) = bbox(&g);
        let centre = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
        assert!(
            (centre[0] - 150.0).abs() < 0.01 && (centre[1] - 102.5).abs() < 0.01,
            "label centre {centre:?}"
        );
        // Plates interleave exactly as text_geometry's do,
        // and every glyph carries the caller's colour at
        // full opacity (a sprite has no opacity channel).
        for pair in prims.chunks_exact(2) {
            let TextPrim::GlyphBox { color: pc, alpha: pa, .. } = &pair[0];
            let TextPrim::GlyphBox { color: gc, alpha: ga, .. } = &pair[1];
            assert_eq!((*pc, *pa), (BACKING_COLOR, BACKING_ALPHA), "plate first");
            assert_eq!((*gc, *ga), (0x112233, 1.0), "glyph second, caller's colour, opaque");
        }
    }

    #[test]
    fn sprite_label_rotates_with_the_quad() {
        // The same 100×50 sprite rotated 90° (y-down) about
        // its centre: local +x now points down-stage, so
        // corners[0] → corners[1] is the vertical edge
        // (175, 50) → (175, 150) and corners[0] →
        // corners[3] the 50-long height edge.
        let quad = [[175.0, 50.0], [175.0, 150.0], [125.0, 150.0], [125.0, 50.0]];
        let prims = sprite_label_boxes(quad, "AB", 0x112233);
        assert_eq!(prims.len(), 40, "same name, same glyph count");
        let g = glyphs(&prims);
        let (lo, hi) = bbox(&g);
        // The run is vertical now: the label's 7-cell run
        // (35 units at size 25 — the same two caps as the
        // unrotated case) spans y, its 5-cell height spans x.
        assert!((hi[1] - lo[1] - 35.0).abs() < 0.01, "run span {}", hi[1] - lo[1]);
        assert!((hi[0] - lo[0] - 25.0).abs() < 0.01, "height span {}", hi[0] - lo[0]);
        // Centre: the quad centre, with the half-cell quirk
        // rotated along — local +y points along −x at 90°.
        let centre = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
        assert!(
            (centre[0] - 147.5).abs() < 0.01 && (centre[1] - 100.0).abs() < 0.01,
            "rotated centre {centre:?}"
        );
    }

    #[test]
    fn sprite_label_size_fits_a_long_name_inside_the_top_edge() {
        // A narrow quad (40 wide, 50 tall) and a 10-char
        // name: the width fit (5 · 40 / 39 ≈ 5.13) beats the
        // height cap (25), so the glyph size shrinks until
        // the run exactly spans the top edge — and no more.
        let quad = [[0.0, 0.0], [40.0, 0.0], [40.0, 50.0], [0.0, 50.0]];
        let prims = sprite_label_boxes(quad, "AAAAAAAAAA", 0xFF0000);
        let g = glyphs(&prims);
        assert_eq!(g.len(), 100, "10 chars × 10 lit pixels");
        let (lo, hi) = bbox(&g);
        let span = hi[0] - lo[0];
        assert!((span - 40.0).abs() < 0.05, "run span {span} vs top edge 40");
        // 'A' touches both extremes, so the span is the
        // whole fitted run; the height is the same size's
        // 5 cells, far under the quad's height.
        assert!((hi[1] - lo[1] - 5.0 * (40.0 / 39.0)).abs() < 0.05, "height {}", hi[1] - lo[1]);
    }

    #[test]
    fn sprite_label_size_is_capped_at_half_the_quad_height() {
        // A huge quad and a 1-char name: the width fit
        // (5 · 400 / 3 ≈ 666.7) is no cap at all, so the
        // height fraction rules — size 50 on a 100-tall
        // quad, a 30×50 label.
        let quad = [[0.0, 0.0], [400.0, 0.0], [400.0, 100.0], [0.0, 100.0]];
        let g = glyphs(&sprite_label_boxes(quad, "A", 0x00FF00));
        assert_eq!(g.len(), 10);
        let (lo, hi) = bbox(&g);
        assert!((hi[1] - lo[1] - 50.0).abs() < 0.01, "height {}", hi[1] - lo[1]);
        assert!((hi[0] - lo[0] - 30.0).abs() < 0.01, "width {}", hi[0] - lo[0]);
        // Centred on the quad (200, 50) plus the quirk.
        let centre = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
        assert!(
            (centre[0] - 200.0).abs() < 0.01 && (centre[1] - 55.0).abs() < 0.01,
            "centre {centre:?}"
        );
    }

    #[test]
    fn sprite_label_degenerate_inputs_yield_nothing() {
        // No name at all, and a blank-only name (advances
        // but stamps nothing, like label_boxes).
        assert!(sprite_label_boxes(SPRITE_QUAD, "", 0).is_empty());
        assert!(sprite_label_boxes(SPRITE_QUAD, "   ", 0).is_empty());
        // A collapsed quad: no top edge, no height.
        let point = [[10.0, 10.0]; 4];
        assert!(sprite_label_boxes(point, "AB", 0).is_empty());
        // Zero-width quad (top edge has no length): the
        // fitted size is 0.
        let no_width = [[100.0, 75.0], [100.0, 75.0], [100.0, 125.0], [100.0, 125.0]];
        assert!(sprite_label_boxes(no_width, "AB", 0).is_empty());
        // Zero-height quad: the height cap is 0.
        let no_height = [[100.0, 75.0], [200.0, 75.0], [200.0, 75.0], [100.0, 75.0]];
        assert!(sprite_label_boxes(no_height, "AB", 0).is_empty());
    }

    // ---- Wave P, P28: subtitle bands --------------------------------

    fn subtitle_style(position: SubtitlePosition, font_scale: f32) -> SubtitleStyle {
        SubtitleStyle {
            position,
            font_scale,
            text_color: 0xFFEE00,
            box_color: 0x112233,
        }
    }

    fn cue(text: &str) -> SubtitleCue {
        SubtitleCue { start_frame: 0, end_frame: 100, text: text.into() }
    }

    #[test]
    fn subtitle_band_bottom_anchor_on_a_known_stage() {
        // 640x360 stage, default-scale bottom style, one
        // single-line cue: the band's centre is the stage's
        // horizontal centre, one edge margin (16) plus half
        // a line height (12) above the bottom edge.
        let style = subtitle_style(SubtitlePosition::Bottom, 1.0);
        let c = cue("HELLO");
        let bands = subtitle_bands([640.0, 360.0], &style, &[&c]);
        assert_eq!(bands.len(), 1);
        let b = &bands[0];
        assert_eq!(b.text, "HELLO");
        assert_eq!(b.center, [320.0, 332.0]);
        assert_eq!(b.font_size, 20.0);
        assert_eq!(b.line_height, 24.0);
        assert_eq!((b.text_color, b.box_color), (0xFFEE00, 0x112233));
        // Band: one line height tall around the centre; the
        // run is (4*5 - 1) cells * 4 = 76 wide, plus half a
        // font size (10) of padding per side = 96.
        assert_eq!(
            b.corners,
            [[272.0, 320.0], [368.0, 320.0], [368.0, 344.0], [272.0, 344.0]]
        );
    }

    #[test]
    fn subtitle_band_top_anchor_on_a_known_stage() {
        // Same stage and cue at the top anchor: the centre
        // is one edge margin plus half a line height BELOW
        // the top edge; x layout is unchanged.
        let style = subtitle_style(SubtitlePosition::Top, 1.0);
        let c = cue("HELLO");
        let bands = subtitle_bands([640.0, 360.0], &style, &[&c]);
        assert_eq!(bands.len(), 1);
        assert_eq!(bands[0].center, [320.0, 28.0]);
        assert_eq!(
            bands[0].corners,
            [[272.0, 16.0], [368.0, 16.0], [368.0, 40.0], [272.0, 40.0]]
        );
    }

    #[test]
    fn subtitle_bands_stack_away_from_the_anchor_in_stored_order() {
        // Two single-line cues: the FIRST cue sits closest
        // to the anchored edge at both anchors — at the
        // bottom it is the LOWER band, at the top the UPPER.
        let (a, b) = (cue("A"), cue("B"));
        let bottom = subtitle_bands(
            [640.0, 360.0],
            &subtitle_style(SubtitlePosition::Bottom, 1.0),
            &[&a, &b],
        );
        assert_eq!(bottom.len(), 2);
        assert_eq!((bottom[0].text.as_str(), bottom[0].center[1]), ("A", 332.0));
        assert_eq!((bottom[1].text.as_str(), bottom[1].center[1]), ("B", 308.0));
        let top = subtitle_bands(
            [640.0, 360.0],
            &subtitle_style(SubtitlePosition::Top, 1.0),
            &[&a, &b],
        );
        assert_eq!((top[0].text.as_str(), top[0].center[1]), ("A", 28.0));
        assert_eq!((top[1].text.as_str(), top[1].center[1]), ("B", 52.0));
    }

    #[test]
    fn subtitle_multiline_cue_hugs_the_anchor_with_its_last_line_at_bottom() {
        // One two-line cue: at the bottom anchor the cue's
        // LAST line is offset 0 (it lands where a single-line
        // cue would), the first line stacks above it; at the
        // top the lines count away in their own order.
        let c = cue("X\nY");
        let bottom = subtitle_bands(
            [640.0, 360.0],
            &subtitle_style(SubtitlePosition::Bottom, 1.0),
            &[&c],
        );
        assert_eq!(bottom.len(), 2);
        assert_eq!((bottom[0].text.as_str(), bottom[0].center[1]), ("X", 308.0));
        assert_eq!((bottom[1].text.as_str(), bottom[1].center[1]), ("Y", 332.0));
        let top = subtitle_bands(
            [640.0, 360.0],
            &subtitle_style(SubtitlePosition::Top, 1.0),
            &[&c],
        );
        assert_eq!((top[0].text.as_str(), top[0].center[1]), ("X", 28.0));
        assert_eq!((top[1].text.as_str(), top[1].center[1]), ("Y", 52.0));
        // A trailing newline adds no line ("X\n".lines() is
        // one line), and an empty-text cue adds no band and
        // does not advance the stack.
        let (t, e) = (cue("X\n"), cue(""));
        let bands = subtitle_bands(
            [640.0, 360.0],
            &subtitle_style(SubtitlePosition::Bottom, 1.0),
            &[&t, &e],
        );
        assert_eq!(bands.len(), 1);
        assert_eq!(bands[0].center[1], 332.0);
        assert!(subtitle_bands([640.0, 360.0], &subtitle_style(SubtitlePosition::Bottom, 1.0), &[]).is_empty());
    }

    #[test]
    fn subtitle_font_scale_scales_and_sanitises() {
        let c = cue("A");
        // Scale 2: font 40, line height 48, bottom centre
        // one margin (16) plus half a line height (24) up.
        let bands = subtitle_bands(
            [640.0, 360.0],
            &subtitle_style(SubtitlePosition::Bottom, 2.0),
            &[&c],
        );
        assert_eq!((bands[0].font_size, bands[0].line_height), (40.0, 48.0));
        assert_eq!(bands[0].center[1], 320.0);
        // Out-of-range scales clamp into [0.5, 3.0]; a
        // non-finite scale falls back to 1.0 — the SVG
        // writer's sanitisation, mirrored.
        let hi = subtitle_bands([640.0, 360.0], &subtitle_style(SubtitlePosition::Bottom, 10.0), &[&c]);
        assert_eq!(hi[0].font_size, 60.0);
        let lo = subtitle_bands([640.0, 360.0], &subtitle_style(SubtitlePosition::Bottom, 0.1), &[&c]);
        assert_eq!((lo[0].font_size, lo[0].line_height), (10.0, 12.0));
        let nan = subtitle_bands([640.0, 360.0], &subtitle_style(SubtitlePosition::Bottom, f32::NAN), &[&c]);
        assert_eq!((nan[0].font_size, nan[0].line_height), (20.0, 24.0));
    }

    fn core_with_cue(text: &str, start: usize, end: usize) -> EditorCore {
        let mut core = EditorCore::new();
        core.project.stage = [640.0, 360.0];
        core.project.subtitle_style = subtitle_style(SubtitlePosition::Bottom, 1.0);
        core.project.subtitles.push(SubtitleCue {
            start_frame: start,
            end_frame: end,
            text: text.into(),
        });
        core
    }

    #[test]
    fn subtitle_geometry_is_empty_without_a_live_cue() {
        // The demo project carries no cues: the stage draws
        // exactly as before.
        assert!(subtitle_geometry(&EditorCore::new()).is_empty());
        // A cue outside the display frame (frame 0) is not
        // live — including one that starts exactly at the
        // half-open end of another's range.
        assert!(subtitle_geometry(&core_with_cue("HI", 5, 9)).is_empty());
        let mut core = core_with_cue("HI", 0, 0);
        assert!(subtitle_geometry(&core).is_empty(), "an empty range is never live");
        core.project.subtitles[0].end_frame = 1;
        assert!(!subtitle_geometry(&core).is_empty(), "frame 0 is inside [0, 1)");
    }

    /// The glyph boxes of a `subtitle_geometry` output:
    /// the odd prims of each (plate, glyph) pair. (The
    /// older `glyphs` helper's alpha filter cannot be
    /// reused here — subtitle plates are fully opaque too,
    /// in the style's box colour.)
    fn subtitle_glyphs(prims: &[TextPrim]) -> Vec<TextPrim> {
        prims.iter().skip(1).step_by(2).cloned().collect()
    }

    #[test]
    fn subtitle_geometry_paints_backing_then_glyphs_in_the_style_colours() {
        // "HI" at the default scale: H = 11 lit pixels,
        // I = 9 — 20 glyph boxes, each preceded by its
        // backing plate.
        let core = core_with_cue("HI", 0, 10);
        let prims = subtitle_geometry(&core);
        assert_eq!(prims.len(), 40);
        for pair in prims.chunks_exact(2) {
            let TextPrim::GlyphBox { color: pc, alpha: pa, .. } = &pair[0];
            let TextPrim::GlyphBox { color: gc, alpha: ga, .. } = &pair[1];
            assert_eq!((*pc, *pa), (0x112233, 1.0), "plate first, box colour, opaque");
            assert_eq!((*gc, *ga), (0xFFEE00, 1.0), "glyph second, text colour, opaque");
            // The plate is the glyph's own box grown about
            // its centre (the shared backing device).
            let (bc, gc2) = (box_centre(&pair[0]), box_centre(&pair[1]));
            assert!(
                (bc[0] - gc2[0]).abs() < 1e-4 && (bc[1] - gc2[1]).abs() < 1e-4,
                "plate centre {bc:?} vs glyph centre {gc2:?}"
            );
        }
        // Glyph placement is label_boxes at the band centre,
        // half-cell quirk included: "HI" spans 7 cells (28
        // units) wide centred on x=320, and its block centre
        // sits half a cell (2 units) below the band centre.
        let (lo, hi) = bbox(&subtitle_glyphs(&prims));
        assert!((lo[0] - 306.0).abs() < 1e-4 && (hi[0] - 334.0).abs() < 1e-4, "x span {lo:?}..{hi:?}");
        let centre = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0];
        assert!(
            (centre[0] - 320.0).abs() < 1e-4 && (centre[1] - 334.0).abs() < 1e-4,
            "glyph centre {centre:?} (band centre 332 + half a cell)"
        );
    }

    #[test]
    fn subtitle_geometry_stacks_cues_in_stored_order() {
        // Two live single-line cues at the bottom anchor:
        // the stream is (plate, glyph) pairs per cue in
        // stored order, and the first cue's glyphs sit BELOW
        // the second's (closest to the anchored edge).
        let mut core = core_with_cue("A", 0, 10);
        core.project.subtitles.push(SubtitleCue {
            start_frame: 0,
            end_frame: 10,
            text: "B".into(),
        });
        let prims = subtitle_geometry(&core);
        // "A" and "B" are 10 lit pixels each: 10 * 2 * 2.
        assert_eq!(prims.len(), 40);
        let (lo, hi) = bbox(&subtitle_glyphs(&prims[..20]));
        let first_centre_y = (lo[1] + hi[1]) / 2.0;
        let (lo, hi) = bbox(&subtitle_glyphs(&prims[20..]));
        let second_centre_y = (lo[1] + hi[1]) / 2.0;
        // Band centres 332 / 308, each plus the half-cell
        // quirk (2 units at this scale).
        assert!((first_centre_y - 334.0).abs() < 1e-4, "first cue centre {first_centre_y}");
        assert!((second_centre_y - 310.0).abs() < 1e-4, "second cue centre {second_centre_y}");
        // Each cue's stream opens with a box-colour plate.
        assert!(matches!(&prims[0], TextPrim::GlyphBox { color, .. } if *color == 0x112233));
        assert!(matches!(&prims[20], TextPrim::GlyphBox { color, .. } if *color == 0x112233));
    }

    #[test]
    fn subtitle_geometry_top_anchor_puts_the_band_at_the_top() {
        let mut core = core_with_cue("HI", 0, 10);
        core.project.subtitle_style = subtitle_style(SubtitlePosition::Top, 1.0);
        let prims = subtitle_geometry(&core);
        assert_eq!(prims.len(), 40);
        // The glyph block centre is the top band's centre
        // (28) plus the half-cell quirk (2), and the whole
        // stream — plates included — sits in the band's
        // neighbourhood at the top of the 360-tall stage.
        let (lo, hi) = bbox(&subtitle_glyphs(&prims));
        let centre_y = (lo[1] + hi[1]) / 2.0;
        assert!((centre_y - 30.0).abs() < 1e-4, "glyph centre y {centre_y}");
        let (lo, hi) = bbox(&prims);
        assert!(lo[1] >= 16.0 - 3.0 && hi[1] <= 40.0 + 3.0, "stream y span {lo:?}..{hi:?}");
    }
}
