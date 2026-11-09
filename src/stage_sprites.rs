//! Wave 13, A2 — STAGE SPRITES: pure stage-space prims for
//! free-standing sprite instances (`Project.sprites`).
//! Declared from `main.rs` only — this is bin-crate code,
//! not part of the engine library (the same arrangement as
//! `stage_view`, whose placeholder pass this module extends
//! to instance scale).
//!
//! ## Why placeholders, again
//!
//! Rune's `Shape` has no bitmap primitive, so the stage
//! cannot draw the PNGs the export rasterizer composites —
//! the hard limit Wave 12's A6 named R1. Wave 12 answered
//! it for SEGMENT media with a geometry-exact placeholder
//! in `stage_view`'s image pass: the item's own quad,
//! filled translucently and outlined in the selection
//! accent. This module is that family's sprite-instance
//! sibling: same accent, same alpha factors, but at
//! instance scale — a sprite is a free placement the user
//! will select, move and keyframe (the A squad's panel and
//! engine ops), so its prims also carry corner marks for
//! the selected instance and a name-label spec.
//!
//! ## Geometry comes from `media::media_items`, verbatim
//!
//! The corners in every prim are the rasterizer's own:
//! [`sprite_quads`] reads `stykanimate_rs::media::media_items`
//! (the exact placement `composite_media` paints, evaluated
//! at the caller's frame) and never recomputes a sprite's
//! transform. Extracting the sprite items from that stream
//! rests on three source facts, all pinned by this module's
//! tests:
//!
//! - `media_items` emits, in order: segment media (figure,
//!   then node order), then sprite instances (in
//!   `Project.sprites` order), then text instances — so the
//!   sprite `Image` items are the TAIL of the image stream.
//! - A sprite contributes an item iff `InstanceKey::sample`
//!   returns a key, that key's `scale > 0.0`, and
//!   `media::png_dims` decodes its asset — the pass's own
//!   three skip rules, re-applied here to know HOW MANY
//!   tail items are sprites and which instances they are.
//! - Each extracted item's `asset` is cross-checked against
//!   its instance's `asset`; a pair that disagrees is
//!   dropped rather than mislabelled, so if the pass's
//!   ordering ever changes this module fails empty, never
//!   wrong.
//!
//! Sprite items carry `alpha = 1.0` in the pass (an instance
//! key has no opacity channel), and the prim alphas below
//! are that alpha times the family's factors — applied
//! exactly once, here, the `stage_text` doctrine.
//!
//! ## The prim contract (for the integrator's stage pass)
//!
//! All coordinates are STAGE coordinates. The consumer
//! maps points through the stage view transform
//! (`stage_view::RotTransform::to_screen`), multiplies
//! widths by the transform's scale, and paints, in
//! `stage_view::SELECTION_ACCENT`:
//!
//! - [`SpritePrim::Fill`] — the quad as a filled polygon
//!   at the prim's `alpha`;
//! - [`SpritePrim::Outline`] — the quad as a closed
//!   stroke at the prim's `width` / `alpha`;
//! - [`SpritePrim::CornerMark`] — a straight segment at
//!   the prim's `width`, full-strength (selected sprite
//!   only — the joint-handles-on-the-selected-figure
//!   convention);
//! - [`SpritePrim::Label`] — NOT drawn here: it is a spec
//!   for the stage-text machinery
//!   (`rune_sec::stage_text::label_boxes_alpha`, whose
//!   owner extends it for sprite labels), carrying the
//!   anchor, run angle, content and glyph size this module
//!   computes. This module emits no glyph boxes — text
//!   layout is not duplicated.
//!
//! Prim order is painter's order: sprites in project
//! order (later instances draw over earlier ones, exactly
//! as `composite_media` composites them), and within one
//! sprite: fill, outline, corner marks (selected only),
//! label.
//!
//! ## Selection is a parameter, honestly
//!
//! At this wave's launch no core field selects a sprite —
//! selection lives in the Sprites panel's own state (A3).
//! So the prim builders take `selected: Option<usize>`
//! from the caller instead of inventing a core field.
//!
//! ## Hit testing
//!
//! [`point_in_quad`] and [`sprite_at_point`] work in stage
//! coordinates; a pointer path converts its element point
//! through `stage_view::to_stage` first (the exact inverse
//! of the drawing transform), then hit-tests against
//! [`sprite_quads`]' output. Topmost wins: the LAST quad
//! in the list containing the point, matching painter's
//! order.

use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::media::{self, MediaItem};
use stykanimate_rs::model::{InstanceKey, Project, V2};

/// Fill alpha for a sprite quad, as a factor of the media
/// item's alpha — Wave 12's segment-placeholder factor
/// (`stage_view`'s image pass), kept identical so segment
/// images and sprite instances read as one family.
pub const FILL_ALPHA_FACTOR: f32 = 0.16;
/// Outline alpha factor for an unselected sprite — Wave
/// 12's segment-placeholder factor, as above.
pub const OUTLINE_ALPHA_FACTOR: f32 = 0.7;
/// Outline alpha factor for the SELECTED sprite: full
/// strength, so the selection reads before its corner
/// marks are even noticed.
pub const SELECTED_OUTLINE_ALPHA_FACTOR: f32 = 1.0;
/// Outline width in stage units (the consumer multiplies
/// by the view scale, like every other stage prim width).
pub const OUTLINE_WIDTH: f32 = 2.0;
/// Corner-mark width in stage units — a touch heavier
/// than the outline, like the selected joint's handle is
/// a touch larger than the others.
pub const CORNER_MARK_WIDTH: f32 = 2.5;
/// Corner-mark length as a fraction of the quad's shorter
/// edge, before clamping (see [`CORNER_MARK_MIN`] /
/// [`CORNER_MARK_MAX`]).
pub const CORNER_MARK_FRACTION: f32 = 0.18;
/// Smallest corner-mark length in stage units: below this
/// a mark stops reading as a mark.
pub const CORNER_MARK_MIN: f32 = 2.0;
/// Largest corner-mark length in stage units: above this
/// a mark on a huge sprite stops being a corner tick and
/// starts being a second outline.
pub const CORNER_MARK_MAX: f32 = 24.0;
/// Name-label glyph height in stage units (the `size`
/// `stage_text::label_boxes_alpha` takes).
pub const LABEL_SIZE: f32 = 16.0;
/// Clear stage units between the quad's top edge and the
/// label's nearest edge — the label anchor sits a further
/// `LABEL_SIZE / 2` out (see [`SpritePrim::Label`]).
pub const LABEL_GAP: f32 = 4.0;

/// One sprite instance placed in stage space for one
/// frame: the hit-test and panel-facing record the prim
/// stream is built from.
#[derive(Clone, Debug, PartialEq)]
pub struct SpriteQuad {
    /// Index into `Project.sprites`.
    pub sprite: usize,
    /// Index into `Project.assets` (the media item's own
    /// asset, cross-checked against the instance's).
    pub asset: usize,
    /// The asset's display name: its stored name, or
    /// `Asset N` (1-based) when the stored name is blank —
    /// the fallback the label spec also uses, so a sprite
    /// is never anonymous on stage.
    pub name: String,
    /// The media item's corners, in `media_items` order:
    /// `c0 + u*(c1-c0) + v*(c3-c0)` for u, v in [0, 1].
    pub corners: [V2; 4],
    /// Quad centre (the average of the corners — exact
    /// for the parallelogram a sprite quad always is).
    pub centre: V2,
    /// The media item's alpha (1.0 for sprite instances
    /// today — an instance key has no opacity channel).
    pub alpha: f32,
}

/// One drawable piece of a sprite instance, in stage
/// coordinates (see the module docs for the consumer
/// contract).
#[derive(Clone, Debug, PartialEq)]
pub enum SpritePrim {
    /// The quad filled at `alpha` (already factored —
    /// apply verbatim).
    Fill {
        sprite: usize,
        corners: [V2; 4],
        alpha: f32,
    },
    /// The quad outlined (closed) at `width`, at `alpha`
    /// (already factored — apply verbatim).
    Outline {
        sprite: usize,
        corners: [V2; 4],
        width: f32,
        alpha: f32,
    },
    /// One straight tick of the selected sprite's corner
    /// marks: from a corner, inward along one edge.
    CornerMark {
        sprite: usize,
        a: V2,
        b: V2,
        width: f32,
    },
    /// The name-label SPEC (not glyph geometry — see the
    /// module docs): `content` centred at `pos`, running
    /// along `angle_deg` (stage degrees, y-down), glyph
    /// height `size`, at `alpha`. The anchor sits above
    /// the quad's top edge (corners[0]→corners[1]): the
    /// edge's midpoint pushed outward — away from the
    /// centre — by `LABEL_SIZE / 2 + LABEL_GAP`, and the
    /// run angle is the top edge's own direction, so the
    /// label turns with the sprite exactly as a text
    /// instance turns with its key.
    Label {
        sprite: usize,
        pos: V2,
        angle_deg: f32,
        content: String,
        size: f32,
        alpha: f32,
    },
}

fn sub(a: V2, b: V2) -> V2 {
    [a[0] - b[0], a[1] - b[1]]
}

fn add(a: V2, b: V2) -> V2 {
    [a[0] + b[0], a[1] + b[1]]
}

fn scale(a: V2, s: f32) -> V2 {
    [a[0] * s, a[1] * s]
}

fn len(a: V2) -> f32 {
    (a[0] * a[0] + a[1] * a[1]).sqrt()
}

/// The display name for `asset`: the stored name when it
/// has non-whitespace content, else `Asset N` (1-based —
/// the numbering the asset tools' status strings use).
fn sprite_name(project: &Project, asset: usize) -> String {
    match project.assets.get(asset) {
        Some(a) if !a.name.trim().is_empty() => a.name.clone(),
        _ => format!("Asset {}", asset + 1),
    }
}

/// The sprite instances `media_items` will emit an item
/// for at `frame_f`, as `Project.sprites` indices in
/// project order — the pass's three skip rules (see the
/// module docs), re-applied so the tail of its image
/// stream can be attributed.
fn active_sprites(project: &Project, frame_f: f32) -> Vec<usize> {
    project
        .sprites
        .iter()
        .enumerate()
        .filter(|(_, inst)| {
            let Some(key) = InstanceKey::sample(&inst.keys, frame_f) else {
                return false;
            };
            if key.scale <= 0.0 {
                return false;
            }
            media::png_dims(project, inst.asset).is_some()
        })
        .map(|(i, _)| i)
        .collect()
}

/// Every sprite instance visible at fractional frame time
/// `frame_f`, as stage-space quads in project (painter's)
/// order. Empty when the project has no active sprite —
/// including the cases the rasterizer also skips: an
/// instance with no keys, a non-positive sampled scale, or
/// an asset that is missing or does not decode.
pub fn sprite_quads(project: &Project, frame_f: f32) -> Vec<SpriteQuad> {
    let active = active_sprites(project, frame_f);
    if active.is_empty() {
        return Vec::new();
    }
    let items = media::media_items(project, frame_f);
    let images: Vec<&MediaItem> = items
        .iter()
        .filter(|i| matches!(i, MediaItem::Image { .. }))
        .collect();
    // Sprite items are the tail of the image stream (see
    // the module docs). If the stream is shorter than the
    // active list — a sign the pass changed shape — fail
    // empty rather than attribute a segment's quad to a
    // sprite.
    if images.len() < active.len() {
        return Vec::new();
    }
    let tail = &images[images.len() - active.len()..];
    let mut out = Vec::with_capacity(active.len());
    for (sprite, item) in active.iter().zip(tail.iter()) {
        let MediaItem::Image {
            corners,
            asset,
            alpha,
        } = item
        else {
            unreachable!("filtered to Image items above");
        };
        // Cross-check: a pair whose assets disagree is
        // dropped, never mislabelled (module docs).
        if *asset != project.sprites[*sprite].asset {
            continue;
        }
        let centre = scale(add(corners[0], corners[2]), 0.5);
        out.push(SpriteQuad {
            sprite: *sprite,
            asset: *asset,
            name: sprite_name(project, *asset),
            corners: *corners,
            centre,
            alpha: *alpha,
        });
    }
    out
}

/// [`sprite_quads`] at the core's display frame — the
/// frame every other stage layer draws at.
pub fn sprite_quads_at(core: &EditorCore) -> Vec<SpriteQuad> {
    sprite_quads(&core.project, core.display_frame())
}

/// The corner-mark length for a quad: a fraction of its
/// shorter edge, clamped to a readable range and never
/// more than 45% of that edge (opposite corners' marks
/// must not meet). Zero for a degenerate quad.
fn corner_mark_len(corners: &[V2; 4]) -> f32 {
    let mut min_edge = f32::INFINITY;
    for i in 0..4 {
        min_edge = min_edge.min(len(sub(corners[(i + 1) % 4], corners[i])));
    }
    if !(min_edge > f32::EPSILON) {
        return 0.0;
    }
    (min_edge * CORNER_MARK_FRACTION)
        .clamp(CORNER_MARK_MIN, CORNER_MARK_MAX)
        .min(min_edge * 0.45)
}

/// The full prim stream for every sprite at `frame_f`:
/// per sprite, in project order — fill, outline, corner
/// marks when `selected == Some(sprite)`, label. See the
/// module docs for order and the consumer contract.
pub fn sprite_prims(
    project: &Project,
    frame_f: f32,
    selected: Option<usize>,
) -> Vec<SpritePrim> {
    let mut out = Vec::new();
    for quad in sprite_quads(project, frame_f) {
        let sprite = quad.sprite;
        let is_selected = selected == Some(sprite);
        out.push(SpritePrim::Fill {
            sprite,
            corners: quad.corners,
            alpha: quad.alpha * FILL_ALPHA_FACTOR,
        });
        out.push(SpritePrim::Outline {
            sprite,
            corners: quad.corners,
            width: OUTLINE_WIDTH,
            alpha: quad.alpha
                * if is_selected {
                    SELECTED_OUTLINE_ALPHA_FACTOR
                } else {
                    OUTLINE_ALPHA_FACTOR
                },
        });
        if is_selected {
            let mark = corner_mark_len(&quad.corners);
            if mark > 0.0 {
                for i in 0..4 {
                    let c = quad.corners[i];
                    for nb in [
                        quad.corners[(i + 1) % 4],
                        quad.corners[(i + 3) % 4],
                    ] {
                        let edge = sub(nb, c);
                        let el = len(edge);
                        if el <= f32::EPSILON {
                            continue;
                        }
                        out.push(SpritePrim::CornerMark {
                            sprite,
                            a: c,
                            b: add(c, scale(edge, mark / el)),
                            width: CORNER_MARK_WIDTH,
                        });
                    }
                }
            }
        }
        // The label spec: above the top edge, running
        // along it (module docs, SpritePrim::Label).
        let top_mid = scale(add(quad.corners[0], quad.corners[1]), 0.5);
        let outward = sub(top_mid, quad.centre);
        let out_len = len(outward);
        if out_len > f32::EPSILON {
            let pos = add(
                top_mid,
                scale(outward, (LABEL_SIZE / 2.0 + LABEL_GAP) / out_len),
            );
            let edge = sub(quad.corners[1], quad.corners[0]);
            out.push(SpritePrim::Label {
                sprite,
                pos,
                angle_deg: edge[1].atan2(edge[0]).to_degrees(),
                content: quad.name.clone(),
                size: LABEL_SIZE,
                alpha: quad.alpha,
            });
        }
    }
    out
}

/// [`sprite_prims`] at the core's display frame.
pub fn sprite_prims_at(core: &EditorCore, selected: Option<usize>) -> Vec<SpritePrim> {
    sprite_prims(&core.project, core.display_frame(), selected)
}

/// True when stage point `p` lies inside the quad
/// `corners` (in `media_items` order), boundary included.
/// Sprite quads are parallelograms, hence convex, so the
/// consistent-cross-sign test is exact. A degenerate quad
/// (zero area — collapsed corners) contains nothing, not
/// even its own points.
pub fn point_in_quad(p: V2, corners: &[V2; 4]) -> bool {
    // Shoelace area: the degeneracy guard.
    let mut area2 = 0.0f32;
    for i in 0..4 {
        let a = corners[i];
        let b = corners[(i + 1) % 4];
        area2 += a[0] * b[1] - b[0] * a[1];
    }
    if area2.abs() < 1e-9 {
        return false;
    }
    let mut sign = 0i32;
    for i in 0..4 {
        let a = corners[i];
        let b = corners[(i + 1) % 4];
        let cross = (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
        if cross > 1e-6 {
            if sign < 0 {
                return false;
            }
            sign = 1;
        } else if cross < -1e-6 {
            if sign > 0 {
                return false;
            }
            sign = -1;
        }
    }
    true
}

/// The topmost sprite at stage point `p`: the LAST quad
/// in `quads` containing it (painter's order — later
/// sprites draw over earlier ones), as a `Project.sprites`
/// index. `None` when no quad contains the point.
pub fn sprite_at_point(quads: &[SpriteQuad], p: V2) -> Option<usize> {
    quads
        .iter()
        .rev()
        .find(|q| point_in_quad(p, &q.corners))
        .map(|q| q.sprite)
}

#[cfg(test)]
mod tests {
    use super::*;
    use stykanimate_rs::model::{
        ClipDef, ClipLoop, ImageAsset, InstanceKey, SegmentMedia, SpriteInstance,
    };

    /// A real, decodable 4x2 PNG (solid green), built the
    /// way the shell's other fixtures build one (the lib's
    /// `media.rs` tests / `snap_tests::tiny_png`): an
    /// asset's bytes must decode wherever the render paths
    /// ask for dimensions, so a dummy byte string would
    /// exercise only the failure path.
    fn tiny_png() -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(4, 2, image::Rgba([0, 200, 0, 255]));
        let mut buf = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .expect("encoding a 4x2 PNG cannot fail");
        buf
    }

    fn key(frame: f32, x: f32, y: f32, scale_: f32, angle: f32) -> InstanceKey {
        InstanceKey {
            frame,
            x,
            y,
            scale: scale_,
            angle,
        }
    }

    /// Demo project + one 4x2 asset ("badge.png") + two
    /// sprites: sprite 0 keyed (300,200) scale 10 angle 0
    /// at frame 0 -> (340,200) scale 10 angle 90 at frame
    /// 2; sprite 1 a single key at (100,100) scale 5.
    fn project_with_sprites() -> Project {
        let mut project = Project::new_demo();
        project.assets.push(ImageAsset {
            name: "badge.png".into(),
            png: tiny_png(),
        });
        project.sprites.push(SpriteInstance {
            asset: 0,
            keys: vec![key(0.0, 300.0, 200.0, 10.0, 0.0), key(2.0, 340.0, 200.0, 10.0, 90.0)],
        });
        project.sprites.push(SpriteInstance {
            asset: 0,
            keys: vec![key(0.0, 100.0, 100.0, 5.0, 0.0)],
        });
        project
    }

    fn close(a: V2, b: V2) -> bool {
        (a[0] - b[0]).abs() < 0.01 && (a[1] - b[1]).abs() < 0.01
    }

    /// The sprite Image items of a media pass, in order —
    /// with no segment media in the fixture, every image
    /// item is a sprite item.
    fn media_sprite_corners(project: &Project, frame_f: f32) -> Vec<[V2; 4]> {
        media::media_items(project, frame_f)
            .iter()
            .filter_map(|i| match i {
                MediaItem::Image { corners, .. } => Some(*corners),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn no_sprites_no_quads_no_prims() {
        let project = Project::new_demo();
        assert!(project.sprites.is_empty(), "the demo project has no sprites");
        assert!(sprite_quads(&project, 0.0).is_empty());
        assert!(sprite_prims(&project, 0.0, None).is_empty());
        assert!(sprite_prims(&project, 0.0, Some(0)).is_empty());
    }

    #[test]
    fn quads_match_media_items_geometry_at_key_and_mid_frames() {
        let project = project_with_sprites();
        // Frame 0: the rasterizer's own corners. Sprite 0's
        // asset is 4x2 px at scale 10 -> a 40x20 quad at
        // (300,200); sprite 1 is 20x10 at (100,100).
        let quads = sprite_quads(&project, 0.0);
        assert_eq!(quads.len(), 2);
        assert!(close(quads[0].corners[0], [280.0, 190.0]), "{:?}", quads[0].corners);
        assert!(close(quads[0].corners[1], [320.0, 190.0]), "{:?}", quads[0].corners);
        assert!(close(quads[0].corners[2], [320.0, 210.0]), "{:?}", quads[0].corners);
        assert!(close(quads[0].corners[3], [280.0, 210.0]), "{:?}", quads[0].corners);
        assert!(close(quads[0].centre, [300.0, 200.0]));
        assert!(close(quads[1].corners[0], [90.0, 95.0]), "{:?}", quads[1].corners);
        assert!(close(quads[1].centre, [100.0, 100.0]));
        assert_eq!(quads[0].sprite, 0);
        assert_eq!(quads[1].sprite, 1);
        assert_eq!(quads[0].asset, 0);
        assert_eq!(quads[0].name, "badge.png");
        assert_eq!(quads[0].alpha, 1.0, "instance keys have no opacity channel");
        // And they ARE the media pass's corners, item for
        // item, at every sampled frame — including frame 1
        // (keys interpolate) and frame 2 (rotated 90°).
        for frame in [0.0, 1.0, 2.0] {
            let quads = sprite_quads(&project, frame);
            let media = media_sprite_corners(&project, frame);
            assert_eq!(quads.len(), media.len(), "frame {frame}");
            for (quad, corners) in quads.iter().zip(media.iter()) {
                assert_eq!(&quad.corners, corners, "frame {frame}");
            }
        }
        let mid = sprite_quads(&project, 1.0);
        assert!(close(mid[0].centre, [320.0, 200.0]), "mid centre {:?}", mid[0].centre);
        let end = sprite_quads(&project, 2.0);
        assert!(close(end[0].corners[0], [350.0, 180.0]), "{:?}", end[0].corners);
        assert!(close(end[0].corners[1], [350.0, 220.0]), "{:?}", end[0].corners);
    }

    #[test]
    fn segment_media_never_leaks_into_sprite_quads() {
        // A segment image on the demo figure: the media
        // pass now leads with a SEGMENT item, and the
        // sprite quads must still be exactly the sprites'
        // items — the tail attribution the module docs
        // describe.
        let mut project = project_with_sprites();
        project.figures[0].def.nodes[1].media =
            Some(SegmentMedia::Image { asset: 0 });
        let items = media::media_items(&project, 0.0);
        let image_count = items
            .iter()
            .filter(|i| matches!(i, MediaItem::Image { .. }))
            .count();
        assert_eq!(image_count, 3, "one segment item + two sprite items");
        let quads = sprite_quads(&project, 0.0);
        assert_eq!(quads.len(), 2, "only the two sprites");
        assert!(close(quads[0].corners[0], [280.0, 190.0]), "{:?}", quads[0].corners);
        assert!(close(quads[1].corners[0], [90.0, 95.0]), "{:?}", quads[1].corners);
    }

    /// Wave 14, C2 — clip-def media fixture
    /// (docs/wave13/CLIP_MEDIA.md §6): the demo figure as a
    /// movieclip instance of clip 0 whose def is a clone of
    /// the figure's own. `clip_media` is placed on the CLIP
    /// def's node 1; the instance's own def stays
    /// media-free. The clip's single frame pose is the
    /// figure's timeline pose, so the instance evaluates to
    /// exactly the demo geometry — the same trick C1's
    /// `media.rs` fixture uses.
    fn project_with_clip_def_media(clip_media: Option<SegmentMedia>) -> Project {
        let mut project = Project::new_demo();
        project.assets.push(ImageAsset {
            name: "badge.png".into(),
            png: tiny_png(),
        });
        let mut clip_def = project.figures[0].def.clone();
        clip_def.nodes[1].media = clip_media;
        let pose = project.frames[0].poses[0].clone();
        project.clips.push(ClipDef {
            name: "clip".into(),
            def: clip_def,
            frames: vec![pose],
            loop_mode: ClipLoop::Loop,
        });
        project.figures[0].clip = Some(0);
        project
    }

    #[test]
    fn clip_def_image_media_is_a_placeholder_item_not_a_sprite() {
        // Wave 14, C2 — the stage verdict for clip-sourced
        // IMAGE media is AUTOMATIC, and this test pins the
        // two premises of the stage's placeholder pass
        // (`stage_view::stage_shape`): the pass paints every
        // `MediaItem::Image` the funnel yields EXCEPT the
        // ones the sprite pass claims (corner equality with
        // `sprite_quads`). So a clip item reaches the stage
        // placeholder exactly like figure-def media iff
        // (1) the funnel yields it and (2) no sprite quad
        // claims its corners. The stage draws no bitmap —
        // Rune has no bitmap primitive; the placeholder is
        // the stage treatment, real pixels are export-only.
        let mut project =
            project_with_clip_def_media(Some(SegmentMedia::Image { asset: 0 }));
        project.figures[0].opacity = 0.5;
        project.clips[0].def.nodes[1].style.opacity = 0.5;
        let items = media::media_items(&project, 0.0);
        assert_eq!(items.len(), 1, "exactly the clip-sourced image");
        match &items[0] {
            MediaItem::Image { asset, alpha, .. } => {
                assert_eq!(*asset, 0);
                // The alpha the placeholder pass factors
                // (0.16 fill / 0.7 outline — stage_view):
                // instance figure opacity × CLIP node's
                // style opacity (CLIP_MEDIA §6.2).
                assert_eq!(*alpha, 0.25, "0.5 figure × 0.5 clip-node style");
            }
            other => panic!("expected an image item, got {other:?}"),
        }
        // (2) Nothing diverts it to the sprite pass: this
        // project has no sprites, so no quads and no prims —
        // the generic placeholder pass paints the item.
        assert!(sprite_quads(&project, 0.0).is_empty());
        assert!(sprite_prims(&project, 0.0, None).is_empty());
        assert!(sprite_prims(&project, 0.0, Some(0)).is_empty());
    }

    #[test]
    fn clip_def_media_does_not_disturb_sprite_tail_attribution() {
        // The funnel's clip fallback grows the SEGMENT
        // portion of the image stream; the sprite items are
        // still its tail, so the sprite quads must stay
        // exactly the two sprites — and the clip item's
        // corners must match neither sprite quad (the
        // placeholder pass's skip test is corner equality,
        // so a match would divert the segment item into
        // the sprite pass).
        let mut project = project_with_sprites();
        let mut clip_def = project.figures[0].def.clone();
        clip_def.nodes[1].media = Some(SegmentMedia::Image { asset: 0 });
        let pose = project.frames[0].poses[0].clone();
        project.clips.push(ClipDef {
            name: "clip".into(),
            def: clip_def,
            frames: vec![pose],
            loop_mode: ClipLoop::Loop,
        });
        project.figures[0].clip = Some(0);
        let items = media::media_items(&project, 0.0);
        let images: Vec<&MediaItem> = items
            .iter()
            .filter(|i| matches!(i, MediaItem::Image { .. }))
            .collect();
        assert_eq!(images.len(), 3, "one clip segment item + two sprite items");
        let quads = sprite_quads(&project, 0.0);
        assert_eq!(quads.len(), 2, "only the two sprites");
        assert!(close(quads[0].corners[0], [280.0, 190.0]), "{:?}", quads[0].corners);
        assert!(close(quads[1].corners[0], [90.0, 95.0]), "{:?}", quads[1].corners);
        let MediaItem::Image {
            corners: clip_corners,
            ..
        } = images[0]
        else {
            unreachable!("filtered to Image items above");
        };
        assert!(
            quads.iter().all(|q| &q.corners != clip_corners),
            "the clip item must not be claimed by the sprite pass"
        );
    }

    #[test]
    fn media_free_clip_and_plain_figure_yield_no_stage_media_items() {
        // The absent side of the pin: a clip instance whose
        // clip def carries NO media (and whose own def is
        // media-free) yields no items at all — attaching a
        // clip alone must not invent stage media.
        let project = project_with_clip_def_media(None);
        assert!(media::media_items(&project, 0.0)
            .iter()
            .all(|i| !matches!(i, MediaItem::Image { .. })));
        assert!(sprite_quads(&project, 0.0).is_empty());
        assert!(sprite_prims(&project, 0.0, None).is_empty());
    }

    #[test]
    fn skipped_sprites_stay_skipped_exactly_as_the_rasterizer_skips_them() {
        let mut project = project_with_sprites();
        // No keys: sample returns None.
        project.sprites.push(SpriteInstance { asset: 0, keys: Vec::new() });
        // Non-positive scale at the sampled frame.
        project.sprites.push(SpriteInstance {
            asset: 0,
            keys: vec![key(0.0, 500.0, 500.0, 0.0, 0.0)],
        });
        // Missing asset.
        project.sprites.push(SpriteInstance {
            asset: 99,
            keys: vec![key(0.0, 500.0, 500.0, 5.0, 0.0)],
        });
        // Undecodable asset.
        project.assets.push(ImageAsset { name: "junk".into(), png: vec![1, 2, 3] });
        project.sprites.push(SpriteInstance {
            asset: 1,
            keys: vec![key(0.0, 500.0, 500.0, 5.0, 0.0)],
        });
        let quads = sprite_quads(&project, 0.0);
        assert_eq!(quads.len(), 2, "only the two well-formed sprites");
        assert_eq!(quads[0].sprite, 0);
        assert_eq!(quads[1].sprite, 1);
    }

    #[test]
    fn prim_stream_is_fill_outline_label_per_sprite_in_project_order() {
        let project = project_with_sprites();
        let prims = sprite_prims(&project, 0.0, None);
        assert_eq!(prims.len(), 6, "3 prims x 2 sprites, nothing selected");
        for (i, sprite) in [0usize, 1].iter().enumerate() {
            let (fill, outline, label) = (&prims[3 * i], &prims[3 * i + 1], &prims[3 * i + 2]);
            match fill {
                SpritePrim::Fill { sprite: s, alpha, .. } => {
                    assert_eq!(s, sprite);
                    assert_eq!(*alpha, FILL_ALPHA_FACTOR, "0.16 x item alpha 1.0");
                }
                other => panic!("expected Fill, got {other:?}"),
            }
            match outline {
                SpritePrim::Outline { sprite: s, width, alpha, .. } => {
                    assert_eq!(s, sprite);
                    assert_eq!(*width, OUTLINE_WIDTH);
                    assert_eq!(*alpha, OUTLINE_ALPHA_FACTOR, "0.7 x item alpha 1.0");
                }
                other => panic!("expected Outline, got {other:?}"),
            }
            match label {
                SpritePrim::Label { sprite: s, content, size, alpha, .. } => {
                    assert_eq!(s, sprite);
                    assert_eq!(content, "badge.png");
                    assert_eq!(*size, LABEL_SIZE);
                    assert_eq!(*alpha, 1.0);
                }
                other => panic!("expected Label, got {other:?}"),
            }
        }
        // Fill/outline corners are the quad's own corners.
        let quads = sprite_quads(&project, 0.0);
        match &prims[0] {
            SpritePrim::Fill { corners, .. } => assert_eq!(corners, &quads[0].corners),
            other => panic!("expected Fill, got {other:?}"),
        }
    }

    #[test]
    fn label_sits_above_the_top_edge_and_runs_along_it() {
        let project = project_with_sprites();
        let prims = sprite_prims(&project, 0.0, None);
        let SpritePrim::Label { pos, angle_deg, .. } = &prims[2] else {
            panic!("expected Label, got {:?}", prims[2]);
        };
        // Sprite 0 unrotated: top edge (280,190)->(320,190),
        // midpoint (300,190); the anchor is LABEL_SIZE/2 +
        // LABEL_GAP = 12 further out (up, away from the
        // centre at (300,200)).
        assert!(close(*pos, [300.0, 178.0]), "label pos {pos:?}");
        assert!(angle_deg.abs() < 0.01, "angle {angle_deg}");
        // Frame 2: sprite 0 is rotated 90 degrees — the top
        // edge runs (350,180)->(350,220), so the label runs
        // downward and sits 12 to the +x side of it.
        let prims = sprite_prims(&project, 2.0, None);
        let SpritePrim::Label { pos, angle_deg, .. } = &prims[2] else {
            panic!("expected Label, got {:?}", prims[2]);
        };
        assert!(close(*pos, [362.0, 200.0]), "rotated label pos {pos:?}");
        assert!((*angle_deg - 90.0).abs() < 0.01, "angle {angle_deg}");
    }

    #[test]
    fn selected_sprite_gets_corner_marks_and_a_full_strength_outline() {
        let project = project_with_sprites();
        let prims = sprite_prims(&project, 0.0, Some(0));
        // Sprite 0: fill, outline, 8 marks, label = 11;
        // sprite 1 unselected: 3.
        assert_eq!(prims.len(), 14);
        match &prims[1] {
            SpritePrim::Outline { alpha, .. } => {
                assert_eq!(*alpha, SELECTED_OUTLINE_ALPHA_FACTOR)
            }
            other => panic!("expected Outline, got {other:?}"),
        }
        let marks: Vec<&SpritePrim> = prims
            .iter()
            .filter(|p| matches!(p, SpritePrim::CornerMark { .. }))
            .collect();
        assert_eq!(marks.len(), 8, "two ticks per corner, selected sprite only");
        assert!(marks.iter().all(|p| matches!(p, SpritePrim::CornerMark { sprite, width, .. } if *sprite == 0 && *width == CORNER_MARK_WIDTH)));
        // Corner c0 (280,190) of the 40x20 quad: mark length
        // is 0.18 x shorter edge (20) = 3.6, inward along
        // both edges — toward c1 (+x) and toward c3 (+y).
        let has_mark = |a: V2, b: V2| {
            marks.iter().any(|p| matches!(p, SpritePrim::CornerMark { a: ma, b: mb, .. } if close(*ma, a) && close(*mb, b)))
        };
        assert!(has_mark([280.0, 190.0], [283.6, 190.0]), "c0 +x tick");
        assert!(has_mark([280.0, 190.0], [280.0, 193.6]), "c0 +y tick");
        assert!(has_mark([320.0, 210.0], [316.4, 210.0]), "c2 -x tick");
        assert!(has_mark([320.0, 210.0], [320.0, 206.4]), "c2 -y tick");
        // Selecting a sprite index that does not exist
        // selects nothing: no marks anywhere.
        let prims = sprite_prims(&project, 0.0, Some(99));
        assert_eq!(prims.len(), 6);
        assert!(prims.iter().all(|p| !matches!(p, SpritePrim::CornerMark { .. })));
    }

    #[test]
    fn blank_asset_name_falls_back_to_a_numbered_label() {
        let mut project = project_with_sprites();
        project.assets[0].name = "  ".into();
        let quads = sprite_quads(&project, 0.0);
        assert_eq!(quads[0].name, "Asset 1", "1-based, the asset tools' numbering");
        let prims = sprite_prims(&project, 0.0, None);
        let SpritePrim::Label { content, .. } = &prims[2] else {
            panic!("expected Label, got {:?}", prims[2]);
        };
        assert_eq!(content, "Asset 1");
    }

    #[test]
    fn point_in_quad_covers_centre_boundary_and_rotated_quads() {
        // Sprite 0 at frame 0: axis-aligned 40x20 at (300,200).
        let quad = sprite_quads(&project_with_sprites(), 0.0)[0].corners;
        assert!(point_in_quad([300.0, 200.0], &quad), "centre");
        assert!(point_in_quad([280.0, 190.0], &quad), "corner counts (boundary)");
        assert!(point_in_quad([300.0, 190.0], &quad), "edge midpoint counts");
        assert!(!point_in_quad([279.0, 200.0], &quad), "1 left of the quad");
        assert!(!point_in_quad([300.0, 211.0], &quad), "1 below the quad");
        assert!(!point_in_quad([340.0, 200.0], &quad), "on the edge's line, past the corner");
        // Rotated 90 degrees at frame 2: the quad is 20 wide,
        // 40 tall around (340,200).
        let quad = sprite_quads(&project_with_sprites(), 2.0)[0].corners;
        assert!(point_in_quad([340.0, 200.0], &quad), "rotated centre");
        assert!(point_in_quad([349.0, 200.0], &quad), "inside the tall quad");
        assert!(!point_in_quad([355.0, 200.0], &quad), "past the tall quad's side");
        assert!(!point_in_quad([340.0, 175.0], &quad), "above the tall quad");
        // Degenerate: collapsed corners contain nothing.
        let flat = [[10.0, 10.0]; 4];
        assert!(!point_in_quad([10.0, 10.0], &flat));
        assert!(!point_in_quad([0.0, 0.0], &flat));
    }

    #[test]
    fn sprite_at_point_returns_the_topmost_sprite() {
        // Sprite 1 (100,100, 20x10) moved onto sprite 0's
        // centre: at the overlap the LATER sprite in
        // project order is on top (painter's order).
        let mut project = project_with_sprites();
        project.sprites[1].keys[0] = key(0.0, 300.0, 200.0, 5.0, 0.0);
        let quads = sprite_quads(&project, 0.0);
        assert_eq!(sprite_at_point(&quads, [300.0, 200.0]), Some(1), "overlap -> later sprite");
        assert_eq!(sprite_at_point(&quads, [285.0, 195.0]), Some(0), "only sprite 0 here");
        assert_eq!(sprite_at_point(&quads, [10.0, 10.0]), None, "empty stage point");
        assert_eq!(sprite_at_point(&[], [300.0, 200.0]), None, "no quads at all");
        // The contract is list order, not sprite index: a
        // reversed list reports the other sprite on top.
        let mut reversed = quads.clone();
        reversed.reverse();
        assert_eq!(sprite_at_point(&reversed, [300.0, 200.0]), Some(0));
    }

    #[test]
    fn core_wrappers_use_the_display_frame() {
        let mut core = EditorCore::new();
        core.project = project_with_sprites();
        assert_eq!(core.display_frame(), 0.0, "a fresh core displays frame 0");
        assert_eq!(
            sprite_quads_at(&core),
            sprite_quads(&core.project, 0.0),
        );
        assert_eq!(
            sprite_prims_at(&core, Some(1)),
            sprite_prims(&core.project, 0.0, Some(1)),
        );
    }
}
