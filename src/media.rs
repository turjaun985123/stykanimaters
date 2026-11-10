//! Text + sprite segment rendering.
//!
//! A node may carry [`SegmentMedia`](crate::model::SegmentMedia):
//! an image from the project's asset table, or a text label. This
//! module turns those into concrete draw items for a frame
//! ([`media_items`]) and rasterises them into a rendered frame
//! ([`composite_media`]).
//!
//! Geometry: an image becomes a quad centred on the node's segment
//! (parent joint -> node joint). The quad's long axis follows the
//! segment, its length is the segment length, and its height is
//! `length / aspect`, where the aspect is the decoded image's
//! width / height, so sprites are never stretched. Text sits at the
//! segment midpoint, rotated along the segment direction.
//!
//! Bent segments (Wave X, X4 — GAP proposal P-2): the segment
//! LINES have always arced through `model::bend_mid` as two
//! pieces; segment images now follow the same arc. When the
//! node's bend at the frame is nonzero, [`media_items`] emits
//! TWO image items for the segment: the quad is split at the
//! arc midpoint (the u = 0.5 edge displaced by exactly the
//! bend offset, so the two half-quads share that edge and
//! their centreline runs parent -> bend_mid -> node, verbatim
//! the line pieces' path), and each item's `src_rect` samples
//! its own half of the source image. Text deliberately does
//! NOT follow: it stays on the straight chord as one rigid
//! label (stage_text's documented rule, pinned there).
//!
//! Strip warp (Wave Y, Y9 — GAP proposal P-11): the two-piece
//! split is the K = 2 case of a general strip warp. A bent
//! segment's image is emitted as K strips along the bend arc
//! (K from [`bend_strip_count`]: one strip per ~0.5 rad of
//! bend, 2..=8), where the spine points are
//! `model::bend_arc_point` sampled at i/K and computed ONCE,
//! so adjacent strips share their edge bit-for-bit and the
//! strip chain's end caps are the unbent quad's own corners.
//! At K = 2 the stream is bit-identical to the P-2 halves.
//!
//! Free-standing instances (`Project.sprites` / `Project.texts`,
//! keyframed with `InstanceKey`s — e.g. imported from Stick Nodes
//! animation files) are placed by the same [`media_items`] pass,
//! after the segment media, so instances draw on top. A sprite
//! instance's `InstanceKey` gives its centre, rotation and scale:
//! the drawn quad is the image's pixel size times the key scale,
//! rotated about the centre. A text instance draws with glyph
//! height `size * key.scale` rotated by the key angle.
//!
//! Text is drawn with a built-in 3x5 bitmap font — no font files, no
//! platform dependencies, fully deterministic. The charset is
//! deliberately small: `A-Z` (lowercase is folded to uppercase),
//! `0-9`, space, and `. , ! ? ' " - : ; ( ) + / & % * = < > _ # $ @`.
//! Any other character renders as `?`.
//!
//! Conventions and limits:
//! - Hidden figures contribute no media (same as the rasterizer).
//! - Movieclip-def media: a clip instance's node with no media
//!   of its own samples the same node index on the clip's def
//!   (payload only — placement stays on the instance's segment;
//!   see `media_items` and docs/wave13/CLIP_MEDIA.md §6).
//! - A node with no parent has no segment; its media is placed at
//!   the joint, extending `length` units along +x. (Media normally
//!   belongs on real segments.)
//! - An image whose asset index is out of range, or whose
//!   stored bytes fail to decode, is skipped silently.
//! - Assets are decoded on each call (no cache): fine for
//!   export-time compositing, not a per-frame editor hot path.
//!
//! The asset table's MCP handlers also live in this module
//! (`tool_assets_list`, `tool_asset_info`, `tool_asset_import`,
//! `tool_asset_rename`, `tool_asset_remove`, `tool_asset_attach`):
//! import validates image magic bytes (the PNG signature or
//! the JPEG SOI marker — Wave R, R9) and decodability and caps
//! the stored size; removal refuses while anything references
//! the asset, and when it does remove, references to higher
//! indices are remapped down by one inside the same undo step.
//!
//! Wave 13 adds the sprite-instance handlers beside them
//! (`tool_sprite_list`, `tool_sprite_add`, `tool_sprite_remove`,
//! `tool_sprite_set_key`, `tool_sprite_delete_key`): thin MCP
//! shells over the `sprite_ops` core ops, which own the
//! mutation, the single snapshot per op and the status line;
//! the handlers own argument validation, refusal strings and
//! the reply shapes, and verify the op's effect from the
//! resulting state before reporting success.
//!
//! Wave 14 adds the movieclip-def media handlers beside them
//! (`tool_clip_asset_attach`, `tool_clip_clear_node_media`,
//! `tool_clip_get_node_media`): the clip-def counterparts of
//! the figure-def media tools, writing the same per-node
//! model on `clips[].def.nodes[]` — the model the renderer's
//! clip fallback samples and the asset census counts.

use crate::editor::EditorCore;
use crate::model::{evaluate, ImageAsset, InstanceKey, Project, SegmentMedia, V2};
use image::{Rgba, RgbaImage};
use serde_json::{json, Value};

/// One piece of segment media placed in stage space for a frame.
#[derive(Clone, Debug, PartialEq)]
pub enum MediaItem {
    /// A sprite quad. Corners are ordered so the quad is
    /// `c0 + u*(c1-c0) + v*(c3-c0)` for u, v in [0, 1]: c0/c1 are
    /// the parent-end/node-end corners on one side, c3 mirrors c0.
    /// `alpha` is the opacity the image blends at (Wave 12, A6
    /// §3a): segment media carry figure opacity × node style
    /// opacity — the same product the segment itself draws
    /// with — where before this field existed the compositor
    /// ignored both and drew every image at full strength;
    /// free-standing sprite instances carry 1.0 (an instance
    /// key has no opacity channel).
    ///
    /// `src_rect` is the normalised source rectangle
    /// `[u0, v0, u1, v1]` this quad samples (Wave X, X4): the
    /// whole image is `[0, 0, 1, 1]`, which is what unbent
    /// segment media and every sprite instance carry. A bent
    /// segment's image arrives as K strip items (Wave Y, Y9)
    /// whose rects are the u-slices `[i/K, 0, (i+1)/K, 1]`;
    /// at K = 2 those are X4's u-halves `[0, 0, 0.5, 1]` and
    /// `[0.5, 0, 1, 1]`.
    Image {
        corners: [V2; 4],
        asset: usize,
        alpha: f32,
        src_rect: [f32; 4],
    },
    /// A text label centred at `pos`, running along `angle_deg`
    /// (stage-space degrees, y-down like the rest of the model).
    /// `size` is the glyph height in stage units. `alpha` is
    /// the opacity the glyphs blend at (Wave R, R8 — P17's
    /// remainder): segment text carries the CARRIER node's
    /// style opacity — the same value the stage applies to
    /// its glyph prims (stage_text's "Opacity" settlement) —
    /// where before this field existed every exporter drew
    /// text fully opaque while the stage faded it. Figure
    /// opacity is deliberately NOT a factor (the stage's
    /// figure fade is a colour blend, and export multiplies
    /// figure opacity into segment alpha only, never into
    /// text); free-standing text instances carry 1.0 (an
    /// instance key has no opacity channel).
    Text {
        pos: V2,
        angle_deg: f32,
        content: String,
        size: f32,
        color: u32,
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

/// Pixel dimensions of asset `idx`, or None when it is missing
/// or its stored bytes do not decode. (The name is the
/// original PNG-era one; the decode is by content and serves
/// JPEG assets identically — Wave R, R9.)
pub fn png_dims(project: &Project, idx: usize) -> Option<(u32, u32)> {
    let asset = project.assets.get(idx)?;
    let img = image::load_from_memory(&asset.png).ok()?;
    let (w, h) = (img.width(), img.height());
    if w == 0 || h == 0 {
        return None;
    }
    Some((w, h))
}

/// Width / height of asset `idx`, or None when it is missing or
/// its stored bytes do not decode.
fn png_aspect(project: &Project, idx: usize) -> Option<f32> {
    let (w, h) = png_dims(project, idx)?;
    Some(w as f32 / h as f32)
}

/// Strip count for a bent segment's image warp (Wave Y, Y9 —
/// GAP proposal P-11): one strip per ~0.5 rad of bend,
/// clamped to 2..=8. The minimum of 2 keeps every bend the
/// P-2 split covered on its historical two half-quads (any
/// |bend| < 1.25 yields exactly 2); stronger bends subdivide
/// further so the arc stays smooth, and the cap bounds the
/// item count on pathological bends. The count lives ONLY
/// here: raster, SVG and the stage placeholders all consume
/// the one [`media_items`] stream, so the surfaces can never
/// disagree on it.
fn bend_strip_count(bend: f32) -> usize {
    ((bend.abs() / 0.5).round() as usize).clamp(2, 8)
}

/// All media items visible at fractional frame time `frame_f`:
/// segment media in figure order, then node order; then sprite
/// instances; then text instances (instances draw on top).
/// A movieclip instance whose own node carries no media falls
/// back to the same node index on its clip's def (Wave 14, C1;
/// docs/wave13/CLIP_MEDIA.md §6.1) — the instance's media wins,
/// the clip fills gaps, never both.
pub fn media_items(project: &Project, frame_f: f32) -> Vec<MediaItem> {
    let mut out = Vec::new();
    // Wave P (P17): segment media rides the figure's segments,
    // so it takes the evaluated frame every other surface
    // takes: when the project carries graph channels, poses
    // AND defs come from `graph::apply_channels` — the
    // evaluation behind the stage's `positions_f` (which the
    // stage's own segment-text geometry already follows) and
    // now behind both exporters. With no channels this is
    // exactly the raw `poses_at` path, unchanged. (P21's
    // audit: this raw bypass was the segment-text divergence.)
    let channelled = if project.channels.is_empty() {
        None
    } else {
        Some(crate::graph::apply_channels(project, &project.channels, frame_f))
    };
    let raw_poses = project.poses_at(frame_f);
    let poses: &Vec<crate::model::Pose> = match &channelled {
        Some(r) => &r.poses,
        None => &raw_poses,
    };
    for (fi, fig) in project.figures.iter().enumerate() {
        if fig.hidden {
            continue;
        }
        let Some(pose) = poses.get(fi) else { continue };
        // Per-node bends at this frame (Wave X, X4): the same
        // `bends_at` fetch every segment-line surface makes.
        // Bends have no graph channel (the stored bend is
        // always the rendered bend), so this is correct with
        // or without channels; an empty vec means no bend
        // data — every node reads as straight.
        let bends = project.bends_at(frame_f, fi);
        // Positions from the channelled def when channels
        // exist; the media payload and carrier stay on the
        // figure's own def nodes in the loop below.
        let def = match &channelled {
            Some(r) => &r.defs[fi],
            None => &fig.def,
        };
        let pos = evaluate(def, pose);
        for (ni, node) in fig.def.nodes.iter().enumerate() {
            // Wave 14, C1 — clip-def media fallback
            // (docs/wave13/CLIP_MEDIA.md §6.1): the instance's
            // own media wins; when the instance node carries
            // none and the figure is a movieclip instance, the
            // same node index on the clip's def supplies the
            // media instead. At most one item per (figure,
            // node) — in the normal clone case both defs carry
            // the same media, and sampling both would draw it
            // twice. The clip def contributes the payload only:
            // geometry, item order and the figure-opacity
            // factor all stay the instance's (§6.2); only the
            // style opacity / colour are read from the node
            // that carries the media (`carrier`). A missing
            // clip or an out-of-range clip node index simply
            // has no fallback (§6.3) — `get` chains, never an
            // index.
            let (media, carrier) = match &node.media {
                Some(media) => (media, node),
                None => {
                    let clip_node = fig
                        .clip
                        .and_then(|ci| project.clips.get(ci))
                        .and_then(|clip| clip.def.nodes.get(ni));
                    match clip_node {
                        Some(carrier) => match &carrier.media {
                            Some(media) => (media, carrier),
                            None => continue,
                        },
                        None => continue,
                    }
                }
            };
            let (a, b) = match node.parent {
                Some(p) => (pos[p], pos[ni]),
                None => (pos[ni], add(pos[ni], [node.length, 0.0])),
            };
            let ab = sub(b, a);
            let len = (ab[0] * ab[0] + ab[1] * ab[1]).sqrt();
            if len <= f32::EPSILON {
                continue;
            }
            let d = scale(ab, 1.0 / len);
            let mid = scale(add(a, b), 0.5);
            match media {
                SegmentMedia::Image { asset, offset, scale: media_scale, angle_offset, flip } => {
                    let Some(aspect) = png_aspect(project, *asset) else {
                        continue;
                    };
                    // Wave Y (Y3, GAP P-8): the media's local
                    // transform composes with the segment's own
                    // placement — the centre shifts by the
                    // offset in the segment frame (along `d`,
                    // along `n`, stage units), and the fitted
                    // quad is scaled and rotated by
                    // `angle_offset` about that centre. At the
                    // identity transform every quantity reduces
                    // to the pre-transform arithmetic exactly
                    // (rotation by 0° is the identity map,
                    // `len * 1.0 == len`, the offset adds
                    // literal zeros), so untransformed media
                    // emit the identical items they always
                    // have. Unusable values are sanitised here,
                    // at composition — never in the stored
                    // project (the `StateKey` speed discipline).
                    let finite_or =
                        |v: f32, fallback: f32| if v.is_finite() { v } else { fallback };
                    let offset =
                        [finite_or(offset[0], 0.0), finite_or(offset[1], 0.0)];
                    let media_scale = if media_scale.is_finite() && *media_scale > 0.0 {
                        *media_scale
                    } else {
                        1.0
                    };
                    let angle_offset = finite_or(*angle_offset, 0.0);
                    let flip = *flip;
                    let n = [-d[1], d[0]];
                    let centre = add(add(mid, scale(d, offset[0])), scale(n, offset[1]));
                    let (sin, cos) = angle_offset.to_radians().sin_cos();
                    let rot = |v: V2| -> V2 {
                        [v[0] * cos - v[1] * sin, v[0] * sin + v[1] * cos]
                    };
                    let hl = scale(rot(d), len * media_scale / 2.0);
                    let hh = scale(rot(n), len * media_scale / aspect / 2.0);
                    let corners = [
                        sub(sub(centre, hl), hh),
                        sub(add(centre, hl), hh),
                        add(add(centre, hl), hh),
                        add(sub(centre, hl), hh),
                    ];
                    let alpha = fig.opacity.clamp(0.0, 1.0)
                        * carrier.style.opacity.clamp(0.0, 1.0);
                    // Wave X (X4, GAP P-2): a bent segment's
                    // image follows the arc, exactly as the
                    // segment line does — the quad splits at
                    // `bend_mid(a, b, bend)`: the u = 0.5 edge
                    // (the chord midpoint ± hh) is displaced
                    // by the bend offset `bend_mid - mid`, so
                    // the two half-quads share that edge, each
                    // samples its own half of the source, and
                    // their union is the chord quad sheared at
                    // the middle. A straight segment keeps the
                    // single full-source item it has always
                    // emitted.
                    //
                    // Wave Y (Y3): under a local transform the
                    // middle edge sits at `centre + (arc_mid -
                    // mid)` ± hh — the same bend displacement,
                    // riding on the transformed centre — which
                    // at the identity is `arc_mid` verbatim.
                    // The transform's `flip` reverses each
                    // item's source u range: the raster's
                    // `su = u0 + u·(u1−u0)` and the SVG matrix
                    // fold both sample a reversed range
                    // mirrored, and the stage placeholder draws
                    // corners only, so all three surfaces
                    // mirror together with no surface-specific
                    // code. The bent halves stay continuous at
                    // the shared edge (both sample u = 0.5
                    // there, flipped or not).
                    //
                    // Wave Y (Y9, GAP P-11): the two-half
                    // split generalises to K strips along the
                    // arc (`bend_strip_count`). The spine is
                    // sampled ONCE — `bend_arc_point` at i/K,
                    // each point riding on the transformed
                    // centre by Y3's displacement rule above —
                    // so adjacent strips share their edge
                    // bit-for-bit, and the chain's two end
                    // edges are the quad's own corners. At
                    // K = 2 the spine midpoint is `bend_mid`
                    // verbatim (the sampler's t = 0.5 anchor)
                    // and the emitted pair is the two halves
                    // above, exactly. The flip reversal
                    // generalises per strip: strip i samples
                    // the u range the mirrored image assigns
                    // it, so strips stay continuous at every
                    // shared edge, flipped or not.
                    let bend = bends.get(ni).copied().unwrap_or(0.0);
                    if bend != 0.0 {
                        let k = bend_strip_count(bend);
                        let spine: Vec<V2> = (0..=k)
                            .map(|i| {
                                let p = crate::model::bend_arc_point(
                                    a,
                                    b,
                                    bend,
                                    i as f32 / k as f32,
                                );
                                add(centre, sub(p, mid))
                            })
                            .collect();
                        let mut edge_lo: Vec<V2> = Vec::with_capacity(k + 1);
                        let mut edge_hi: Vec<V2> = Vec::with_capacity(k + 1);
                        for (i, s) in spine.iter().enumerate() {
                            if i == 0 {
                                edge_lo.push(corners[0]);
                                edge_hi.push(corners[3]);
                            } else if i == k {
                                edge_lo.push(corners[1]);
                                edge_hi.push(corners[2]);
                            } else {
                                edge_lo.push(sub(*s, hh));
                                edge_hi.push(add(*s, hh));
                            }
                        }
                        for i in 0..k {
                            let (u0, u1) = if flip {
                                (
                                    1.0 - i as f32 / k as f32,
                                    1.0 - (i + 1) as f32 / k as f32,
                                )
                            } else {
                                (i as f32 / k as f32, (i + 1) as f32 / k as f32)
                            };
                            out.push(MediaItem::Image {
                                corners: [
                                    edge_lo[i],
                                    edge_lo[i + 1],
                                    edge_hi[i + 1],
                                    edge_hi[i],
                                ],
                                asset: *asset,
                                alpha,
                                src_rect: [u0, 0.0, u1, 1.0],
                            });
                        }
                    } else {
                        out.push(MediaItem::Image {
                            corners,
                            asset: *asset,
                            alpha,
                            src_rect: if flip {
                                [1.0, 0.0, 0.0, 1.0]
                            } else {
                                [0.0, 0.0, 1.0, 1.0]
                            },
                        });
                    }
                }
                SegmentMedia::Text { content, size } => {
                    out.push(MediaItem::Text {
                        pos: mid,
                        angle_deg: d[1].atan2(d[0]).to_degrees(),
                        content: content.clone(),
                        size: *size,
                        color: carrier.color.unwrap_or(fig.color),
                        alpha: carrier.style.opacity.clamp(0.0, 1.0),
                    });
                }
            }
        }
    }
    // Free-standing sprite instances: the image's pixel size times
    // the sampled key's scales, rotated by the key angle (y-down
    // rotation, as for text). The key's pivot is the anchor: the
    // image point at (pivot_x, pivot_y) sits at the key position,
    // and each flip mirrors the image around that anchor (the
    // offset along the axis is negated, so the pivot point itself
    // never moves). At the pre-enrichment defaults — centre
    // pivot, no flips, scale_y following scale, opacity 1 — the
    // quad is exactly the centred ±half-extent form this loop
    // has always emitted (Wave X, P-6).
    for inst in &project.sprites {
        let Some(key) = InstanceKey::sample(&inst.keys, frame_f) else {
            continue;
        };
        if key.scale <= 0.0 {
            continue;
        }
        let Some((w, h)) = png_dims(project, inst.asset) else {
            continue;
        };
        let c = [key.x, key.y];
        let rad = key.angle.to_radians();
        let u = [rad.cos(), rad.sin()];
        let v = [-rad.sin(), rad.cos()];
        let wu = w as f32 * key.scale;
        let hv_len = h as f32 * key.eff_scale_y();
        let fx = if key.flip_x { -1.0 } else { 1.0 };
        let fy = if key.flip_y { -1.0 } else { 1.0 };
        // Image corner (i, j), i/j in {0, 1}: its offset from
        // the anchor along u/v, in stage units.
        let corner = |i: f32, j: f32| {
            add(
                add(c, scale(u, fx * (i - key.pivot_x) * wu)),
                scale(v, fy * (j - key.pivot_y) * hv_len),
            )
        };
        out.push(MediaItem::Image {
            corners: [corner(0.0, 0.0), corner(1.0, 0.0), corner(1.0, 1.0), corner(0.0, 1.0)],
            asset: inst.asset,
            alpha: key.opacity.clamp(0.0, 1.0),
            // Free-standing instances are never bent: the
            // whole source, always (Wave X, X4).
            src_rect: [0.0, 0.0, 1.0, 1.0],
        });
    }
    // Free-standing text instances: `size` is the unscaled glyph
    // height; the key scale multiplies it.
    for text in &project.texts {
        let Some(key) = InstanceKey::sample(&text.keys, frame_f) else {
            continue;
        };
        let size = text.size * key.scale;
        if size <= 0.0 {
            continue;
        }
        out.push(MediaItem::Text {
            pos: [key.x, key.y],
            angle_deg: key.angle,
            content: text.content.clone(),
            size,
            color: text.color,
            alpha: 1.0,
        });
    }
    out
}

/// Draw every media item for `frame_f` into `img`. `view` maps
/// stage coordinates to image pixel coordinates (the same closure
/// shape the export rasterizer builds). Images are sampled
/// nearest-neighbour through the quad's affine map and alpha-blended
/// over the destination; text is stamped as glyph blocks blended at the
/// item's alpha.
pub fn composite_media(
    project: &Project,
    frame_f: f32,
    img: &mut RgbaImage,
    view: &dyn Fn(V2) -> [f32; 2],
) {
    // Stage-units -> pixels scale, probed from the view: the
    // length of the mapped x basis vector. Views may also rotate
    // (camera angle, Wave 6) — a basis vector's length is the
    // uniform scale with or without rotation, so this probe
    // stays exact.
    let o = view([0.0, 0.0]);
    let ex = view([1.0, 0.0]);
    let view_scale = ((ex[0] - o[0]).powi(2) + (ex[1] - o[1]).powi(2)).sqrt();
    let view_scale = if view_scale > f32::EPSILON {
        view_scale
    } else {
        1.0
    };

    for item in media_items(project, frame_f) {
        match item {
            MediaItem::Image {
                corners,
                asset,
                alpha,
                src_rect,
            } => {
                composite_image(project, asset, corners, src_rect, alpha, img, view);
            }
            MediaItem::Text {
                pos,
                angle_deg,
                content,
                size,
                color,
                alpha,
            } => composite_text(pos, angle_deg, &content, size, color, alpha, img, view, view_scale),
        }
    }
}

// ---------------------------------------------------------------------
// Asset-table MCP handlers (Wave 12)
//
// `Project.assets` had no MCP surface before this wave: the only
// way an asset entered a project through a tool was
// `attach_image_to_node`, whose core op reads the first image in
// ./import and attaches it in the same breath — there was no way
// to list, inspect, rename, remove or deliberately attach an
// asset. These handlers give the table its full lifecycle. The
// reference census below is the whole set of places an asset
// index can live: segment media on figure defs, segment media
// on movieclip defs (`ClipDef` carries no asset table of its
// own, so its nodes' indices name project assets), and
// free-standing sprite instances. The definitive census —
// including the audited storage that is NOT a reference
// class, and the rule new writers must follow — is the doc
// comment on `asset_references` (Wave 14, C6).
// ---------------------------------------------------------------------

/// PNG signature an imported PNG asset must start with.
const PNG_MAGIC: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

/// JPEG start-of-image marker (SOI + the first marker byte)
/// an imported JPEG asset must start with (Wave R, R9).
const JPEG_MAGIC: [u8; 3] = [0xFF, 0xD8, 0xFF];

/// The import funnel's content gate: `bytes` opens with the
/// PNG signature or the JPEG SOI marker. Content decides,
/// never the file name — every downstream consumer decodes
/// by content (`image::load_from_memory` sniffs the same
/// way), so the gate and the decoders agree by construction.
/// Anything else (GIF included — the crate could decode it,
/// but the funnel's accepted set is PNG + JPEG) is refused
/// before any decode is attempted.
fn importable_image_magic(bytes: &[u8]) -> bool {
    bytes.starts_with(&PNG_MAGIC) || bytes.starts_with(&JPEG_MAGIC)
}

/// Largest file `tool_asset_import` will store, in bytes
/// (8 MiB). Asset bytes are stored verbatim inside the project
/// JSON (as a number array) and inside every undo snapshot, so
/// an unbounded import would bloat each save and each undo
/// step several times over.
pub const ASSET_IMPORT_MAX_BYTES: u64 = 8 * 1024 * 1024;

/// The asset funnel's shared validation (factored out of
/// `tool_asset_import` in Wave Y, P-9, so the layered-set
/// importer in `layered_import.rs` validates package images
/// through exactly the same gate as a single-file import):
/// `bytes` must be at most [`ASSET_IMPORT_MAX_BYTES`], open
/// with the PNG or JPEG magic, decode as an image, and
/// decode non-empty. On success returns the asset to store
/// (bytes verbatim — consumers decode by content) plus the
/// decoded dimensions. `name` is the asset's display name
/// and names it in the refusal strings, whose asserted
/// substrings ("import limit", "not a PNG", "does not
/// decode") are the funnel's original wording.
pub fn asset_from_bytes(
    name: String,
    bytes: Vec<u8>,
) -> Result<(ImageAsset, u32, u32), String> {
    if bytes.len() as u64 > ASSET_IMPORT_MAX_BYTES {
        return Err(format!(
            "asset '{name}' is {} bytes, over the {ASSET_IMPORT_MAX_BYTES}-byte import limit",
            bytes.len()
        ));
    }
    if !importable_image_magic(&bytes) {
        return Err(format!(
            "asset '{name}' is not a PNG or JPEG (bad magic bytes)"
        ));
    }
    let img = image::load_from_memory(&bytes)
        .map_err(|e| format!("asset '{name}' does not decode as an image: {e}"))?;
    let (w, h) = (img.width(), img.height());
    if w == 0 || h == 0 {
        return Err(format!("asset '{name}' decodes to an empty image"));
    }
    Ok((ImageAsset { name, png: bytes }, w, h))
}

fn ast_get<'a>(args: &'a Value, key: &str) -> Option<&'a Value> {
    args.as_object().and_then(|o| o.get(key))
}

fn ast_as_usize(v: &Value, key: &str) -> Result<usize, String> {
    if let Some(u) = v.as_u64() {
        return usize::try_from(u).map_err(|_| format!("argument '{key}' is too large"));
    }
    if let Some(f) = v.as_f64() {
        if f >= 0.0 && f.fract() == 0.0 && f <= usize::MAX as f64 {
            return Ok(f as usize);
        }
    }
    Err(format!("argument '{key}' must be a non-negative integer"))
}

fn ast_req_usize(args: &Value, key: &str) -> Result<usize, String> {
    match ast_get(args, key) {
        Some(v) => ast_as_usize(v, key),
        None => Err(format!("missing required argument '{key}'")),
    }
}

fn ast_req_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    match ast_get(args, key) {
        Some(v) => v
            .as_str()
            .ok_or_else(|| format!("argument '{key}' must be a string")),
        None => Err(format!("missing required argument '{key}'")),
    }
}

fn ast_opt_str<'a>(args: &'a Value, key: &str) -> Result<Option<&'a str>, String> {
    match ast_get(args, key) {
        Some(v) => v
            .as_str()
            .map(Some)
            .ok_or_else(|| format!("argument '{key}' must be a string")),
        None => Ok(None),
    }
}

fn ast_check_asset(core: &EditorCore, index: usize) -> Result<(), String> {
    let n = core.project.assets.len();
    if index >= n {
        return Err(format!(
            "asset index {index} out of range (project has {n} asset(s))"
        ));
    }
    Ok(())
}

fn ast_check_node(core: &EditorCore, fi: usize, ni: usize) -> Result<(), String> {
    let figures = core.project.figures.len();
    if fi >= figures {
        return Err(format!(
            "figure index {fi} out of range (project has {figures} figure(s))"
        ));
    }
    let n = core.project.figures[fi].def.nodes.len();
    if ni >= n {
        return Err(format!(
            "node index {ni} out of range (figure {fi} has {n} node(s))"
        ));
    }
    Ok(())
}

/// Pixel dimensions of one asset's bytes, or None when they do
/// not decode (the same tolerance `png_dims` applies at render
/// time, reported here instead of skipped silently).
fn asset_dims(asset: &ImageAsset) -> Option<(u32, u32)> {
    let img = image::load_from_memory(&asset.png).ok()?;
    let (w, h) = (img.width(), img.height());
    if w == 0 || h == 0 {
        return None;
    }
    Some((w, h))
}

/// Count every reference to asset `asset`, as
/// (figure segment images, clip-def segment images, sprite
/// instances).
///
/// # The definitive asset-reference census (Wave 14, C6)
///
/// An asset index can be STORED in exactly three places in a
/// `Project`, and this fn counts all three; `remap_asset_refs`
/// shifts the same three on removal, and `tool_asset_remove`
/// refuses while any of the three is non-zero. Every counted
/// class also RENDERS, so the refusal never locks an asset
/// over an image nothing draws (the Wave 13 state of the
/// clip-def row, closed by Wave 14 C1–C4):
///
/// | Class | Stored at | Counted | Renders via |
/// |---|---|---|---|
/// | Figure segment image | `figures[].def.nodes[].media = Image` | `segments` | `media_items` figure pass (stage / raster / SVG / thumbnails) |
/// | Clip-def segment image | `clips[].def.nodes[].media = Image` | `clip_segments` | `media_items` clip fallback (C1) → the same four surfaces |
/// | Sprite instance | `sprites[].asset` | `sprites` | `media_items` sprite pass + the stage sprite pass |
///
/// Audited storage that is NOT a reference class, and why it
/// must stay uncounted:
/// - **Figure library** (`EditorCore::library`): not part of
///   `Project` at all — bare `FigureDef`s loaded from
///   `./library/*.json`. A library file can carry an image
///   index, but `insert_library_figure` strips image media
///   at the project boundary (it would resolve against the
///   wrong table), so a library entry can never become a
///   live project reference.
/// - **3D figures** (`fig3d::Figure3`): name / nodes /
///   colour only — no media or texture field exists, and
///   the 3D session state is not stored in `Project`.
/// - **FX clips and particles** (`FxClipEntry`,
///   `ParticleEvent`): procedural kinds and colours; no
///   asset index field.
/// - **Text** (`SegmentMedia::Text`, `TextInstance`):
///   self-contained content; no asset index.
/// - **Audio clips** (`AudioClip`): name a file PATH on
///   disk, not an asset-table index, so table removal can
///   never orphan one.
///
/// Forward rule for new writers (Wave 14 C5's clip attach
/// path included): any tool or op that stores
/// `SegmentMedia::Image` on `clips[].def.nodes[]` is counted
/// and remapped HERE with no census change — the reference
/// lands in `clip_segments` by construction. A writer that
/// introduces a genuinely NEW storage place for an asset
/// index must extend this fn AND `remap_asset_refs` in the
/// same change, or removal will silently orphan it.
fn asset_references(project: &Project, asset: usize) -> (usize, usize, usize) {
    let mut segments = 0;
    for fig in &project.figures {
        for node in &fig.def.nodes {
            if matches!(&node.media, Some(SegmentMedia::Image { asset: a, .. }) if *a == asset) {
                segments += 1;
            }
        }
    }
    let mut clip_segments = 0;
    for clip in &project.clips {
        for node in &clip.def.nodes {
            if matches!(&node.media, Some(SegmentMedia::Image { asset: a, .. }) if *a == asset) {
                clip_segments += 1;
            }
        }
    }
    let sprites = project.sprites.iter().filter(|s| s.asset == asset).count();
    (segments, clip_segments, sprites)
}

/// After removing asset `removed` from a table that had
/// `old_len` entries, shift every still-valid reference above
/// `removed` down by one, across all three reference kinds;
/// returns how many references moved. References that were
/// already dangling (>= `old_len`) are left untouched — they
/// named nothing before the removal either, and the renderers
/// skip them.
fn remap_asset_refs(project: &mut Project, removed: usize, old_len: usize) -> usize {
    fn shift(media: &mut Option<SegmentMedia>, removed: usize, old_len: usize) -> bool {
        match media {
            Some(SegmentMedia::Image { asset, .. }) if *asset > removed && *asset < old_len => {
                *asset -= 1;
                true
            }
            _ => false,
        }
    }
    let mut moved = 0;
    for fig in &mut project.figures {
        for node in &mut fig.def.nodes {
            if shift(&mut node.media, removed, old_len) {
                moved += 1;
            }
        }
    }
    for clip in &mut project.clips {
        for node in &mut clip.def.nodes {
            if shift(&mut node.media, removed, old_len) {
                moved += 1;
            }
        }
    }
    for sprite in &mut project.sprites {
        if sprite.asset > removed && sprite.asset < old_len {
            sprite.asset -= 1;
            moved += 1;
        }
    }
    moved
}

fn asset_json(project: &Project, index: usize, asset: &ImageAsset) -> Value {
    let dims = asset_dims(asset);
    let (segments, clip_segments, sprites) = asset_references(project, index);
    json!({
        "index": index,
        "name": asset.name,
        "bytes": asset.png.len(),
        "width": dims.map(|(w, _)| w),
        "height": dims.map(|(_, h)| h),
        "references": {
            "segments": segments,
            "clip_segments": clip_segments,
            "sprites": sprites,
        },
    })
}

/// MCP handler: list the project's asset table — index, name,
/// stored byte size, decoded pixel dimensions (null when the
/// stored bytes do not decode) and live reference counts per
/// asset. Read-only; no undo step.
pub fn tool_assets_list(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    let assets: Vec<Value> = core
        .project
        .assets
        .iter()
        .enumerate()
        .map(|(i, a)| asset_json(&core.project, i, a))
        .collect();
    Ok(json!({
        "ok": true,
        "count": assets.len(),
        "assets": assets,
    }))
}

/// MCP handler: one asset's list entry, addressed by `index`.
/// Read-only; no undo step.
pub fn tool_asset_info(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let index = ast_req_usize(args, "index")?;
    ast_check_asset(core, index)?;
    let mut out = asset_json(&core.project, index, &core.project.assets[index]);
    out["ok"] = json!(true);
    Ok(out)
}

/// MCP handler: import a PNG or JPEG file from disk into the
/// asset table. The file must start with the PNG magic bytes
/// or the JPEG SOI marker, decode as an image, and be at most
/// [`ASSET_IMPORT_MAX_BYTES`] — anything else is refused
/// before any snapshot is taken. The
/// asset name defaults to the file's name (the
/// `attach_image_to_node` op's rule); an optional `name`
/// overrides it. The new asset is appended; its index is
/// returned. One undo step.
pub fn tool_asset_import(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let path = ast_req_str(args, "path")?;
    let meta =
        std::fs::metadata(path).map_err(|e| format!("cannot read '{path}': {e}"))?;
    if !meta.is_file() {
        return Err(format!("'{path}' is not a file"));
    }
    if meta.len() > ASSET_IMPORT_MAX_BYTES {
        return Err(format!(
            "'{path}' is {} bytes, over the {ASSET_IMPORT_MAX_BYTES}-byte import limit",
            meta.len()
        ));
    }
    let bytes = std::fs::read(path).map_err(|e| format!("cannot read '{path}': {e}"))?;
    let name = match ast_opt_str(args, "name")? {
        Some(n) if !n.trim().is_empty() => n.trim().to_string(),
        _ => std::path::Path::new(path)
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_else(|| "image.png".into()),
    };
    let size = bytes.len();
    // The content gate itself is the shared funnel
    // (`asset_from_bytes`), so this handler and the
    // layered-set importer can never drift apart on what
    // counts as an importable image.
    let (asset, w, h) = asset_from_bytes(name.clone(), bytes)?;
    core.snapshot();
    let index = core.project.assets.len();
    core.project.assets.push(asset);
    core.status = format!("Asset imported ('{name}', asset {}).", index + 1);
    Ok(json!({
        "ok": true,
        "index": index,
        "name": name,
        "bytes": size,
        "width": w,
        "height": h,
        "status": core.status,
    }))
}

/// MCP handler: rename asset `index` to `name` (trimmed;
/// empty is refused). Renaming to the current name is a no-op
/// with "changed": false and spends no undo step; a real
/// rename is one undo step.
pub fn tool_asset_rename(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let index = ast_req_usize(args, "index")?;
    ast_check_asset(core, index)?;
    let name = ast_req_str(args, "name")?.trim().to_string();
    if name.is_empty() {
        return Err("argument 'name' must not be empty".into());
    }
    if core.project.assets[index].name == name {
        return Ok(json!({
            "ok": true,
            "index": index,
            "name": name,
            "changed": false,
            "status": core.status,
        }));
    }
    core.snapshot();
    core.project.assets[index].name = name.clone();
    core.status = format!("Asset {} renamed to '{name}'.", index + 1);
    Ok(json!({
        "ok": true,
        "index": index,
        "name": name,
        "changed": true,
        "status": core.status,
    }))
}

/// MCP handler: remove asset `index` from the table.
///
/// Removal rule: if ANY reference to the asset exists — a
/// figure segment image, a movieclip-def segment image, or a
/// sprite instance — the removal is REFUSED with the counts,
/// so a reference can never be silently orphaned. When the
/// asset is unreferenced, removal shifts every later asset
/// down one index, and every still-valid reference to a higher
/// index is remapped down by one in the SAME undo step, so
/// references keep naming the same image. (References that
/// were already dangling before the removal are left as-is.)
/// One undo step.
pub fn tool_asset_remove(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let index = ast_req_usize(args, "index")?;
    ast_check_asset(core, index)?;
    let (segments, clip_segments, sprites) = asset_references(&core.project, index);
    if segments + clip_segments + sprites > 0 {
        return Err(format!(
            "asset {index} ('{}') is still referenced by {segments} segment image(s), \
             {clip_segments} clip segment(s) and {sprites} sprite instance(s); \
             remove those references first — removal never orphans a reference",
            core.project.assets[index].name
        ));
    }
    let old_len = core.project.assets.len();
    let name = core.project.assets[index].name.clone();
    core.snapshot();
    core.project.assets.remove(index);
    let remapped = remap_asset_refs(&mut core.project, index, old_len);
    core.status = format!(
        "Asset {} ('{name}') removed; {remapped} reference(s) shifted.",
        index + 1
    );
    Ok(json!({
        "ok": true,
        "index": index,
        "name": name,
        "remapped": remapped,
        "count": core.project.assets.len(),
        "status": core.status,
    }))
}

/// MCP handler: attach an EXISTING asset to a segment —
/// `SegmentMedia::Image` on figure `figure`, node `node`,
/// replacing whatever media the segment carried (the
/// `attach_image_to_node` op's overwrite rule, but addressed
/// by index instead of the ./import folder: that op stays as
/// it is). One undo step; the status line is the op's own
/// success string.
pub fn tool_asset_attach(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = ast_req_usize(args, "figure")?;
    let ni = ast_req_usize(args, "node")?;
    let asset = ast_req_usize(args, "asset")?;
    ast_check_node(core, fi, ni)?;
    ast_check_asset(core, asset)?;
    // The P-8 transform args (Wave Y, Y4), parsed
    // before the snapshot: a refused value spends no
    // undo step. Re-attaching the SAME asset with new
    // transform args is how an attached sprite's
    // transform is set — attach replaces the media
    // wholesale, as it always has.
    let t = ast_opt_media_transform(args)?;
    let (offset, scale, angle_offset, flip) = t.fields();
    core.snapshot();
    core.project.figures[fi].def.nodes[ni].media = Some(SegmentMedia::Image {
        asset,
        offset,
        scale,
        angle_offset,
        flip,
    });
    core.status = format!("Sprite attached (asset {}).", asset + 1);
    let mut reply = json!({
        "ok": true,
        "figure": fi,
        "node": ni,
        "asset": asset,
        "status": core.status,
    });
    if t.any() {
        reply["media"] =
            segment_media_json(&core.project.figures[fi].def.nodes[ni].media);
    }
    Ok(reply)
}

// ---------------------------------------------------------------------
// Sprite-instance MCP handlers (Wave 13)
//
// `Project.sprites` had no MCP surface before this wave either:
// instances entered only via .nodes import, and the asset
// removal census could count sprite references to an asset but
// no tool could create, move or delete an instance. These five
// handlers drive the `sprite_ops` core ops (`add_sprite`,
// `remove_sprite`, `set_sprite_key`, `delete_sprite_key`),
// which own the mutation, exactly one snapshot per successful
// op and the status line; an op refusal is status-only and
// spends no snapshot. The handlers validate every argument up
// front (so a bad call never reaches the op), then verify the
// op's effect from the resulting project state — an op that
// refused despite valid arguments surfaces as its status line,
// never as a false success.
// ---------------------------------------------------------------------

/// Check a sprite instance index against the table, with the
/// same message shape as `ast_check_asset`.
fn ast_check_sprite(core: &EditorCore, index: usize) -> Result<(), String> {
    let n = core.project.sprites.len();
    if index >= n {
        return Err(format!(
            "sprite index {index} out of range (project has {n} sprite instance(s))"
        ));
    }
    Ok(())
}

/// A required finite f32 argument. JSON numbers arrive as f64;
/// a value that is not a number, or that overflows f32 to an
/// infinity on the cast, is refused — the Wave 11 finiteness
/// contract the registry pins.
fn ast_req_f32(args: &Value, key: &str) -> Result<f32, String> {
    match ast_get(args, key) {
        Some(v) => ast_as_f32(v, key),
        None => Err(format!("missing required argument '{key}'")),
    }
}

/// An optional finite f32 argument (same refusal rules as
/// [`ast_req_f32`] when present).
fn ast_opt_f32(args: &Value, key: &str) -> Result<Option<f32>, String> {
    match ast_get(args, key) {
        Some(v) => ast_as_f32(v, key).map(Some),
        None => Ok(None),
    }
}

/// An optional boolean argument (Wave X, P-6 — the sprite
/// key flips); a non-boolean value is refused.
fn ast_opt_bool(args: &Value, key: &str) -> Result<Option<bool>, String> {
    match ast_get(args, key) {
        Some(v) => v
            .as_bool()
            .map(Some)
            .ok_or_else(|| format!("argument '{key}' must be a boolean")),
        None => Ok(None),
    }
}

/// The P-8 local-transform arguments shared by the
/// two segment-media attach handlers (Wave Y, Y4):
/// every field optional, an absent field taking the
/// identity component, so a call naming none of them
/// is the historical attach byte for byte. Both
/// handlers parse them UP FRONT, before any snapshot
/// — a refused value spends no undo step and writes
/// nothing. `angle_offset` is in degrees (the
/// payload's own unit; the sprite-key angle
/// convention). A non-positive `scale` is refused:
/// the composed quad would vanish or mirror, and the
/// funnel's sanitised read would silently rewrite it
/// to 1 — better an honest refusal at the door.
struct MediaTransformArgs {
    offset: Option<[f32; 2]>,
    scale: Option<f32>,
    angle_offset: Option<f32>,
    flip: Option<bool>,
}

impl MediaTransformArgs {
    /// True when the call named at least one transform
    /// argument. The attach replies carry the
    /// resulting media only then, keeping the
    /// no-argument reply byte-identical to before.
    fn any(&self) -> bool {
        self.offset.is_some()
            || self.scale.is_some()
            || self.angle_offset.is_some()
            || self.flip.is_some()
    }

    /// The payload's `(offset, scale, angle_offset,
    /// flip)` field set, identity components filled
    /// in for the unnamed arguments.
    fn fields(&self) -> ([f32; 2], f32, f32, bool) {
        (
            self.offset.unwrap_or([0.0, 0.0]),
            self.scale.unwrap_or(1.0),
            self.angle_offset.unwrap_or(0.0),
            self.flip.unwrap_or(false),
        )
    }
}

fn ast_opt_media_transform(args: &Value) -> Result<MediaTransformArgs, String> {
    let offset = match ast_get(args, "offset") {
        Some(v) => {
            let arr = v.as_array().ok_or_else(|| {
                "argument 'offset' must be an array of two numbers".to_string()
            })?;
            if arr.len() != 2 {
                return Err(
                    "argument 'offset' must have exactly two elements \
                     (along, across)"
                        .to_string(),
                );
            }
            Some([
                ast_as_f32(&arr[0], "offset")?,
                ast_as_f32(&arr[1], "offset")?,
            ])
        }
        None => None,
    };
    let scale = ast_opt_f32(args, "scale")?;
    if let Some(s) = scale {
        if s <= 0.0 {
            return Err("argument 'scale' must be greater than zero".to_string());
        }
    }
    Ok(MediaTransformArgs {
        offset,
        scale,
        angle_offset: ast_opt_f32(args, "angle_offset")?,
        flip: ast_opt_bool(args, "flip")?,
    })
}

fn ast_as_f32(v: &Value, key: &str) -> Result<f32, String> {
    let n = v
        .as_f64()
        .ok_or_else(|| format!("argument '{key}' must be a number"))?;
    let f = n as f32;
    if !f.is_finite() {
        return Err(format!("argument '{key}' must be a finite number"));
    }
    Ok(f)
}

fn key_json(key: &InstanceKey) -> Value {
    json!({
        "frame": key.frame,
        "x": key.x,
        "y": key.y,
        "scale": key.scale,
        "angle": key.angle,
        // The P-6 enrichment (Wave X), stored values as they
        // sit on the key — `scale_y` 0.0 is the follow-`scale`
        // form; read it through `eff_scale_y` for the axis the
        // passes draw.
        "opacity": key.opacity,
        "flip_x": key.flip_x,
        "flip_y": key.flip_y,
        "scale_y": key.scale_y,
        "pivot_x": key.pivot_x,
        "pivot_y": key.pivot_y,
    })
}

fn sprite_json(project: &Project, index: usize, inst: &crate::model::SpriteInstance) -> Value {
    json!({
        "index": index,
        "asset": inst.asset,
        "asset_name": project.assets.get(inst.asset).map(|a| a.name.as_str()),
        "key_count": inst.keys.len(),
        "keys": inst.keys.iter().map(key_json).collect::<Vec<_>>(),
    })
}

/// MCP handler: list the project's sprite instances — index,
/// asset index and name (null when the instance's asset index
/// dangles), and every keyframe. Read-only; no undo step.
pub fn tool_sprite_list(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    let sprites: Vec<Value> = core
        .project
        .sprites
        .iter()
        .enumerate()
        .map(|(i, s)| sprite_json(&core.project, i, s))
        .collect();
    Ok(json!({
        "ok": true,
        "count": sprites.len(),
        "sprites": sprites,
    }))
}

/// MCP handler: add a sprite instance for asset `asset` — the
/// `add_sprite` core op places it at the stage centre with a
/// single key at frame 0 (scale 1, no rotation). One undo step
/// (inside the op).
pub fn tool_sprite_add(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let asset = ast_req_usize(args, "asset")?;
    ast_check_asset(core, asset)?;
    let before = core.project.sprites.len();
    let _ = core.add_sprite(asset);
    let added = core.project.sprites.get(before);
    match added {
        Some(inst) if inst.asset == asset && core.project.sprites.len() == before + 1 => {
            Ok(json!({
                "ok": true,
                "index": before,
                "sprite": sprite_json(&core.project, before, inst),
                "status": core.status,
            }))
        }
        _ => Err(core.status.clone()),
    }
}

/// MCP handler: remove sprite instance `sprite` from the
/// project. Later instances shift down one index (the
/// instance table's own rule — nothing references an instance
/// by index, so no remapping is owed). One undo step (inside
/// the op).
pub fn tool_sprite_remove(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let index = ast_req_usize(args, "sprite")?;
    ast_check_sprite(core, index)?;
    let asset = core.project.sprites[index].asset;
    let before = core.project.sprites.len();
    let _ = core.remove_sprite(index);
    if core.project.sprites.len() == before - 1 {
        Ok(json!({
            "ok": true,
            "index": index,
            "asset": asset,
            "count": core.project.sprites.len(),
            "status": core.status,
        }))
    } else {
        Err(core.status.clone())
    }
}

/// MCP handler: write sprite `sprite`'s transform key at
/// `frame` — insert a new key or replace the one there. `x`,
/// `y`, `rotation` and `scale` are each optional: an omitted
/// field takes the value the sprite samples at that frame
/// (or, for a sprite with no keys, x 0, y 0, rotation 0,
/// scale 1). `scale` must be greater than 0 — a key at scale
/// 0 or less never draws. One undo step (inside the op).
///
/// Wave X (P-6): the enrichment fields are writable here too —
/// `opacity` (0 to 1), `flip_x`, `flip_y`, `scale_y` (greater
/// than 0; the key's second-axis scale) and `pivot_x` /
/// `pivot_y` (the anchor, image-relative) — each optional with
/// the same sampled-base fallback, and the write goes through
/// `set_sprite_key_full`, so all eleven fields land in the one
/// undo step.
pub fn tool_sprite_set_key(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let index = ast_req_usize(args, "sprite")?;
    ast_check_sprite(core, index)?;
    let frame = ast_req_f32(args, "frame")?;
    if frame < 0.0 {
        return Err(format!("argument 'frame' must not be negative (got {frame})"));
    }
    if frame.fract() != 0.0 {
        return Err(format!(
            "argument 'frame' must be a whole frame number (got {frame})"
        ));
    }
    // Wave 13 splice (integrator): A1's op takes an integral
    // frame and clamps it to the project's last frame (the
    // op family's rule). Normalise once here so the sample,
    // the created check and the stored verification all read
    // the frame the op will actually write; the refusals
    // above keep the requested frame.
    let frame = frame.min(core.frame_count().saturating_sub(1) as f32);
    let scale = ast_opt_f32(args, "scale")?;
    if let Some(s) = scale {
        if s <= 0.0 {
            return Err(format!(
                "argument 'scale' must be greater than 0 (got {s}); \
                 a sprite key at scale 0 or less never draws"
            ));
        }
    }
    let opacity = ast_opt_f32(args, "opacity")?;
    if let Some(o) = opacity {
        if !(0.0..=1.0).contains(&o) {
            return Err(format!(
                "argument 'opacity' must be between 0 and 1 (got {o})"
            ));
        }
    }
    let scale_y = ast_opt_f32(args, "scale_y")?;
    if let Some(s) = scale_y {
        if s <= 0.0 {
            return Err(format!(
                "argument 'scale_y' must be greater than 0 (got {s}); \
                 a sprite key at scale 0 or less never draws"
            ));
        }
    }
    let inst = &core.project.sprites[index];
    let base = InstanceKey::sample(&inst.keys, frame).unwrap_or(InstanceKey {
        frame,
        x: 0.0,
        y: 0.0,
        scale: 1.0,
        angle: 0.0,
        opacity: 1.0,
        flip_x: false,
        flip_y: false,
        scale_y: 0.0,
        pivot_x: 0.5,
        pivot_y: 0.5,
    });
    let key = InstanceKey {
        frame,
        x: ast_opt_f32(args, "x")?.unwrap_or(base.x),
        y: ast_opt_f32(args, "y")?.unwrap_or(base.y),
        scale: scale.unwrap_or(base.scale),
        angle: ast_opt_f32(args, "rotation")?.unwrap_or(base.angle),
        opacity: opacity.unwrap_or(base.opacity),
        flip_x: ast_opt_bool(args, "flip_x")?.unwrap_or(base.flip_x),
        flip_y: ast_opt_bool(args, "flip_y")?.unwrap_or(base.flip_y),
        scale_y: scale_y.unwrap_or(base.scale_y),
        pivot_x: ast_opt_f32(args, "pivot_x")?.unwrap_or(base.pivot_x),
        pivot_y: ast_opt_f32(args, "pivot_y")?.unwrap_or(base.pivot_y),
    };
    let created = !inst.keys.iter().any(|k| k.frame == frame);
    let _ = core.set_sprite_key_full(index, frame as usize, key);
    let stored = core.project.sprites[index]
        .keys
        .iter()
        .find(|k| k.frame == frame);
    match stored {
        Some(k)
            if k.x == key.x
                && k.y == key.y
                && k.scale == key.scale
                && k.angle == key.angle
                && k.opacity == key.opacity
                && k.flip_x == key.flip_x
                && k.flip_y == key.flip_y
                && k.scale_y == key.scale_y
                && k.pivot_x == key.pivot_x
                && k.pivot_y == key.pivot_y =>
        {
            Ok(json!({
                "ok": true,
                "sprite": index,
                "created": created,
                "key": key_json(k),
                "status": core.status,
            }))
        }
        _ => Err(core.status.clone()),
    }
}

/// MCP handler: delete sprite `sprite`'s key at `frame`. A
/// frame at/past the project's frame count is refused
/// before the op is called (Wave P, P38 — it used to be
/// clamped onto the last frame, deleting THAT frame's key
/// while the reply reported the requested frame), and a
/// frame the sprite has no key at is refused the same way,
/// so no undo step is spent. One undo step (inside the op)
/// on success. A stale key past the end is deleted by
/// exact frame via `sprite_delete_key_at`
/// (src/mcp_sprite_tools.rs).
pub fn tool_sprite_delete_key(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let index = ast_req_usize(args, "sprite")?;
    ast_check_sprite(core, index)?;
    let frame = ast_req_f32(args, "frame")?;
    if frame.fract() != 0.0 {
        return Err(format!(
            "argument 'frame' must be a whole frame number (got {frame})"
        ));
    }
    // Wave P (P38): refuse a frame at/past the frame count
    // instead of clamping onto the last frame (the Wave 13
    // splice this guard replaces did exactly that — the
    // existence check, the op call and the verification
    // all read the clamped frame while the success reply
    // reported the requested one, so naming frame 999
    // deleted the last frame's key and called it frame
    // 999). The refusal is the handler family's
    // frame-range wording (draw_ops / figure_ops /
    // frame_swap). Nothing is spent: the refusal precedes
    // any snapshot, and the core status is left untouched.
    // The core op refuses the same frames in its own
    // voice, so the layers agree; below this guard the
    // frame is in range and the op acts on it exactly.
    if frame >= core.frame_count() as f32 {
        return Err(format!(
            "frame index {frame} out of range (project has {} frame(s))",
            core.frame_count()
        ));
    }
    let inst = &core.project.sprites[index];
    if !inst.keys.iter().any(|k| k.frame == frame) {
        return Err(format!(
            "sprite {index} has no key at frame {frame} (it has {} key(s))",
            inst.keys.len()
        ));
    }
    let _ = core.delete_sprite_key(index, frame as usize);
    let inst = &core.project.sprites[index];
    if inst.keys.iter().any(|k| k.frame == frame) {
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "sprite": index,
        "frame": frame,
        "key_count": inst.keys.len(),
        "status": core.status,
    }))
}

// TOOL ENTRY SPEC: add to tools() in src/mcp_tools.rs,
// in the Wave 13 sprite block beside sprite_delete_key.
//   Tool {
//       name: "sprite_sample",
//       description: "Read sprite 'sprite''s sampled transform at 'frame' (fractional frames allowed): the interpolated key between its authored keys, clamped outside the keyed range — the transform the stage and exports draw. Read-only; no undo step. Errors when the sprite has no keys.",
//       schema: obj_schema(
//           json!({
//               "sprite": { "type": "integer" },
//               "frame": { "type": "number", "description": "Frame to sample at; may be fractional" }
//           }),
//           &["sprite", "frame"],
//       ),
//       handler: crate::media::tool_sprite_sample,
//   }
// MCP.md row:
//   | `sprite_sample` | Read sprite 'sprite''s sampled transform at 'frame' (fractional frames allowed): the interpolated key between its authored keys, clamped outside the keyed range — the transform the stage and exports draw. Read-only; no undo step. | `sprite`, `frame` | — |
/// MCP handler: sample sprite `sprite`'s transform at
/// `frame` (fractional allowed) — the read side of
/// `sprite_set_key`, answering the transform the stage
/// actually draws without the client reimplementing
/// `InstanceKey::sample`. Drives the core's
/// `sprite_sample` read (Wave 13 A1). A sprite with no
/// keys has nothing to sample and is refused. Read-only;
/// no undo step.
pub fn tool_sprite_sample(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let index = ast_req_usize(args, "sprite")?;
    ast_check_sprite(core, index)?;
    let frame = ast_req_f32(args, "frame")?;
    if frame < 0.0 {
        return Err(format!(
            "argument 'frame' must not be negative (got {frame})"
        ));
    }
    match core.sprite_sample(index, frame) {
        Some(key) => Ok(json!({
            "ok": true,
            "sprite": index,
            "frame": frame,
            "key": key_json(&key),
        })),
        None => Err(format!(
            "sprite {index} has no keys to sample (it has 0 key(s))"
        )),
    }
}

// ---------------------------------------------------------------------
// Movieclip-def media MCP handlers (Wave 14, C5)
//
// The clip family could create, edit and pose movieclip
// definitions but had no media surface at all: a clip def's
// node media could only arrive by cloning a media-carrying
// figure (docs/wave13/CLIP_MEDIA.md §4), and once it existed
// only in the def it was counted by the asset census while
// no tool could attach, clear or even read it. These three
// handlers are the clip-def counterparts of `asset_attach`
// (which writes a figure's own definition),
// `clear_node_media` (selection-scoped to a figure) and the
// `media` field of `get_node` — addressed by (clip, node)
// indices, because a clip def has no selection to scope to.
// Attach writes image media only, exactly like
// `asset_attach` (no MCP tool attaches text media; text
// arrives via the GUI text path); clear and read cover
// either kind.
//
// The write lands on `clips[ci].def.nodes[ni].media` — the
// same per-node model the renderer samples (the
// `media_items` clip fallback) and the asset census counts
// (`asset_references`' clip-def row): no new storage place
// is introduced, so removal refusal and remapping cover
// references created here with no census change.
// ---------------------------------------------------------------------

/// Check a clip index, and a node index within that clip's
/// def, against the project, with the same message shapes
/// as `ast_check_node`.
fn ast_check_clip_node(core: &EditorCore, ci: usize, ni: usize) -> Result<(), String> {
    let clips = core.project.clips.len();
    if ci >= clips {
        return Err(format!(
            "clip index {ci} out of range (project has {clips} clip(s))"
        ));
    }
    let n = core.project.clips[ci].def.nodes.len();
    if ni >= n {
        return Err(format!(
            "node index {ni} out of range (clip {ci} has {n} node(s))"
        ));
    }
    Ok(())
}

/// A node's segment media as JSON, in `get_node`'s shape:
/// null when the node carries none, otherwise
/// {kind: "image", asset, offset, scale, angle_offset,
/// flip} or {kind: "text", content, size}. The image
/// arm carries the P-8 local transform in full (Wave Y,
/// Y4) — stored values as they sit on the payload, the
/// `key_json` precedent — so a read can never hide
/// state the attach tools can write. Shared by
/// `tool_clip_get_node_media` and the integrator's
/// `clip_list` splice (see the SPLICE SPEC on that
/// handler), so every clip-def media read speaks the
/// one shape.
pub fn segment_media_json(media: &Option<SegmentMedia>) -> Value {
    match media {
        Some(SegmentMedia::Image {
            asset,
            offset,
            scale,
            angle_offset,
            flip,
        }) => json!({
            "kind": "image",
            "asset": asset,
            "offset": [offset[0], offset[1]],
            "scale": scale,
            "angle_offset": angle_offset,
            "flip": flip,
        }),
        Some(SegmentMedia::Text { content, size }) => {
            json!({ "kind": "text", "content": content, "size": size })
        }
        None => Value::Null,
    }
}

// TOOL ENTRY SPEC: add to tools() in src/mcp_tools.rs,
// in the Wave 12 asset block beside asset_attach.
//   Tool {
//       name: "clip_asset_attach",
//       description: "Attach the EXISTING asset at index 'asset' to movieclip definition 'clip', node 'node' as that segment's image media, replacing whatever media the definition's segment carried (the clip-def counterpart of asset_attach, which writes a figure's own definition). The media renders on every instance of the clip whose own definition carries no media at that node — an instance's own media wins. One undo step.",
//       schema: obj_schema(
//           json!({
//               "clip": { "type": "integer" },
//               "node": { "type": "integer" },
//               "asset": { "type": "integer" }
//           }),
//           &["clip", "node", "asset"],
//       ),
//       handler: crate::media::tool_clip_asset_attach,
//   }
// MCP.md row (merge beside the asset_attach row):
//   | `clip_asset_attach` | Attach the EXISTING asset at index 'asset' to movieclip definition 'clip', node 'node' as that segment's image media, replacing whatever media the definition's segment carried (the clip-def counterpart of asset_attach, which writes a figure's own definition). The media renders on every instance of the clip whose own definition carries no media at that node — an instance's own media wins. One undo step. | `clip`, `node`, `asset` | — |
/// MCP handler: attach an EXISTING asset to a movieclip
/// definition's segment — `SegmentMedia::Image` on clip
/// `clip`, node `node`, replacing whatever media the
/// definition's segment carried (the clip-def counterpart
/// of `tool_asset_attach`). One undo step.
pub fn tool_clip_asset_attach(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let ci = ast_req_usize(args, "clip")?;
    let ni = ast_req_usize(args, "node")?;
    let asset = ast_req_usize(args, "asset")?;
    ast_check_clip_node(core, ci, ni)?;
    ast_check_asset(core, asset)?;
    // The P-8 transform args (Wave Y, Y4), exactly as
    // `tool_asset_attach` takes them — parsed before
    // the snapshot, so a refusal spends nothing.
    let t = ast_opt_media_transform(args)?;
    let (offset, scale, angle_offset, flip) = t.fields();
    core.snapshot();
    core.project.clips[ci].def.nodes[ni].media = Some(SegmentMedia::Image {
        asset,
        offset,
        scale,
        angle_offset,
        flip,
    });
    core.status = format!(
        "Sprite attached to clip {} (asset {}).",
        ci + 1,
        asset + 1
    );
    let mut reply = json!({
        "ok": true,
        "clip": ci,
        "node": ni,
        "asset": asset,
        "status": core.status,
    });
    if t.any() {
        reply["media"] =
            segment_media_json(&core.project.clips[ci].def.nodes[ni].media);
    }
    Ok(reply)
}

// TOOL ENTRY SPEC: add to tools() in src/mcp_tools.rs,
// in the Wave 12 asset block beside asset_attach.
//   Tool {
//       name: "clip_clear_node_media",
//       description: "Clear the media (image or text) attached to movieclip definition 'clip', node 'node' (the clip-def counterpart of clear_node_media, which is selection-scoped to a figure). Clearing a node that carries no media is reported with 'cleared': false, not an error, and spends no undo step; a clear that removes media is one undo step. The reply's 'media' names what was cleared (null when nothing was).",
//       schema: obj_schema(
//           json!({
//               "clip": { "type": "integer" },
//               "node": { "type": "integer" }
//           }),
//           &["clip", "node"],
//       ),
//       handler: crate::media::tool_clip_clear_node_media,
//   }
// MCP.md row (merge beside the asset_attach row):
//   | `clip_clear_node_media` | Clear the media (image or text) attached to movieclip definition 'clip', node 'node' (the clip-def counterpart of clear_node_media, which is selection-scoped to a figure). Clearing a node that carries no media is reported, not an error. One undo step when media was actually cleared. | `clip`, `node` | — |
/// MCP handler: clear the media (image or text) on clip
/// `clip`, node `node` — the clip-def counterpart of
/// `clear_node_media`, addressed by index because a clip
/// def has no selection. Presence is resolved BEFORE the
/// snapshot (the Wave 12 B2 doctrine): clearing a node
/// that carries no media changes nothing, so it spends no
/// phantom undo step and is reported with "cleared":
/// false; a real clear is exactly one undo step. The
/// reply's "media" names what was cleared, in
/// `get_node`'s shape.
pub fn tool_clip_clear_node_media(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let ci = ast_req_usize(args, "clip")?;
    let ni = ast_req_usize(args, "node")?;
    ast_check_clip_node(core, ci, ni)?;
    let old = core.project.clips[ci].def.nodes[ni].media.clone();
    let Some(old) = old else {
        core.status = format!("Clip {} node {} carries no media.", ci + 1, ni + 1);
        return Ok(json!({
            "ok": true,
            "clip": ci,
            "node": ni,
            "cleared": false,
            "media": Value::Null,
            "status": core.status,
        }));
    };
    core.snapshot();
    core.project.clips[ci].def.nodes[ni].media = None;
    core.status = "Segment media cleared.".into();
    Ok(json!({
        "ok": true,
        "clip": ci,
        "node": ni,
        "cleared": true,
        "media": segment_media_json(&Some(old)),
        "status": core.status,
    }))
}

// TOOL ENTRY SPEC: add to tools() in src/mcp_tools.rs,
// in the Wave 12 asset block beside asset_attach.
//   Tool {
//       name: "clip_get_node_media",
//       description: "Read the segment media attached to movieclip definition 'clip', node 'node': null, or {kind: 'image', asset} / {kind: 'text', content, size} — the same shape get_node returns for a figure's node. Read-only.",
//       schema: obj_schema(
//           json!({
//               "clip": { "type": "integer" },
//               "node": { "type": "integer" }
//           }),
//           &["clip", "node"],
//       ),
//       handler: crate::media::tool_clip_get_node_media,
//   }
// MCP.md row (merge beside the asset_attach row):
//   | `clip_get_node_media` | Read the segment media attached to movieclip definition 'clip', node 'node': null, or {kind: 'image', asset} / {kind: 'text', content, size} — the same shape get_node returns for a figure's node. Read-only. | `clip`, `node` | — |
//
// SPLICE SPEC (integrator): `tool_clip_list`
// (src/mcp_tools.rs) reports node COUNTS only, so a clip
// def's media is invisible in the list reply — a read that
// hides state this wave makes writable. Add the per-node
// media to its per-clip json!:
//   "media": c.def.nodes.iter()
//       .map(|n| crate::media::segment_media_json(&n.media))
//       .collect::<Vec<_>>(),
// extend the clip_list registry description with
// ", per-node segment media", and append the matching
// clause to docs/MCP.md's clip_list row (the clause text
// is in docs/wave14/CLIP_MEDIA_ROWS.md §0). The dedicated
// read above and the list field are complements, not
// alternatives: the list gives the whole def at a glance,
// this tool addresses one node.
/// MCP handler: read the segment media on clip `clip`,
/// node `node`, in `get_node`'s shape (null, or
/// {kind: "image", asset} / {kind: "text", content,
/// size}). Read-only; no undo step.
pub fn tool_clip_get_node_media(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let ci = ast_req_usize(args, "clip")?;
    let ni = ast_req_usize(args, "node")?;
    ast_check_clip_node(core, ci, ni)?;
    Ok(json!({
        "ok": true,
        "clip": ci,
        "node": ni,
        "media": segment_media_json(&core.project.clips[ci].def.nodes[ni].media),
    }))
}

#[allow(clippy::too_many_arguments)]
fn composite_image(
    project: &Project,
    asset: usize,
    corners: [V2; 4],
    src_rect: [f32; 4],
    alpha: f32,
    img: &mut RgbaImage,
    view: &dyn Fn(V2) -> [f32; 2],
) {
    let alpha = alpha.clamp(0.0, 1.0);
    if alpha <= 0.0 {
        return;
    }
    let Some(asset) = project.assets.get(asset) else {
        return;
    };
    let Ok(decoded) = image::load_from_memory(&asset.png) else {
        return;
    };
    let src = decoded.to_rgba8();
    let (sw, sh) = (src.width() as f32, src.height() as f32);
    if sw < 1.0 || sh < 1.0 {
        return;
    }
    let c: [[f32; 2]; 4] = [
        view(corners[0]),
        view(corners[1]),
        view(corners[2]),
        view(corners[3]),
    ];
    let e1 = [c[1][0] - c[0][0], c[1][1] - c[0][1]];
    let e2 = [c[3][0] - c[0][0], c[3][1] - c[0][1]];
    let det = e1[0] * e2[1] - e1[1] * e2[0];
    if det.abs() <= f32::EPSILON {
        return;
    }
    let (iw, ih) = (img.width() as i32, img.height() as i32);
    let min_x = c.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min).floor() as i32;
    let max_x = c.iter().map(|p| p[0]).fold(f32::NEG_INFINITY, f32::max).ceil() as i32;
    let min_y = c.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min).floor() as i32;
    let max_y = c.iter().map(|p| p[1]).fold(f32::NEG_INFINITY, f32::max).ceil() as i32;
    for y in min_y.max(0)..=max_y.min(ih - 1) {
        for x in min_x.max(0)..=max_x.min(iw - 1) {
            let rel = [x as f32 + 0.5 - c[0][0], y as f32 + 0.5 - c[0][1]];
            let u = (rel[0] * e2[1] - rel[1] * e2[0]) / det;
            let v = (e1[0] * rel[1] - e1[1] * rel[0]) / det;
            if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
                continue;
            }
            // The quad's (u, v) map into the item's source
            // rect (Wave X, X4): at the full rect this is the
            // historical `u * sw` / `v * sh` verbatim; a bent
            // segment's half-quad samples only its half of
            // the source.
            let su = src_rect[0] + u * (src_rect[2] - src_rect[0]);
            let sv = src_rect[1] + v * (src_rect[3] - src_rect[1]);
            let sx = ((su * sw) as u32).min(src.width() - 1);
            let sy = ((sv * sh) as u32).min(src.height() - 1);
            let sp = *src.get_pixel(sx, sy);
            let sa = sp[3] as f32 / 255.0 * alpha;
            if sa <= 0.0 {
                continue;
            }
            let dp = img.get_pixel_mut(x as u32, y as u32);
            let da = dp[3] as f32 / 255.0;
            let out_a = sa + da * (1.0 - sa);
            if out_a <= 0.0 {
                continue;
            }
            for ch in 0..3 {
                let blended =
                    (sp[ch] as f32 * sa + dp[ch] as f32 * da * (1.0 - sa)) / out_a;
                dp[ch] = blended.round().clamp(0.0, 255.0) as u8;
            }
            dp[3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn composite_text(
    pos: V2,
    angle_deg: f32,
    content: &str,
    size: f32,
    color: u32,
    alpha: f32,
    img: &mut RgbaImage,
    view: &dyn Fn(V2) -> [f32; 2],
    view_scale: f32,
) {
    let chars: Vec<char> = content.chars().collect();
    if chars.is_empty() || size <= 0.0 {
        return;
    }
    // Wave R (R8): the item alpha, clamped and applied
    // exactly as `composite_image` applies its own. At 1.0
    // the blend below is the old opaque stamp byte for
    // byte: source-over at full strength is the source
    // pixel (out_a = 1, channels = the glyph colour).
    let alpha = alpha.clamp(0.0, 1.0);
    if alpha <= 0.0 {
        return;
    }
    let cell = size / 5.0; // stage units per glyph pixel
    let total_units = chars.len() as f32 * 4.0 - 1.0; // 3 wide + 1 gap
    let start = -total_units / 2.0;
    let rad = angle_deg.to_radians();
    let (cs, sn) = (rad.cos(), rad.sin());
    let rgba = Rgba([
        ((color >> 16) & 0xFF) as u8,
        ((color >> 8) & 0xFF) as u8,
        (color & 0xFF) as u8,
        255,
    ]);
    let block = (cell * view_scale).round().max(1.0) as i32;
    for (ci, ch) in chars.iter().enumerate() {
        let glyph = glyph_rows(*ch);
        for row in 0..5usize {
            for col in 0..3usize {
                if (glyph[row] >> (2 - col)) & 1 == 0 {
                    continue;
                }
                let lx = (start + ci as f32 * 4.0 + col as f32 + 0.5) * cell;
                let ly = (row as f32 - 2.0 + 0.5) * cell;
                let world = [
                    pos[0] + lx * cs - ly * sn,
                    pos[1] + lx * sn + ly * cs,
                ];
                let px = view(world);
                let cx = px[0].round() as i32;
                let cy = px[1].round() as i32;
                for dy in 0..block {
                    for dx in 0..block {
                        let x = cx - block / 2 + dx;
                        let y = cy - block / 2 + dy;
                        if x >= 0 && y >= 0 && (x as u32) < img.width() && (y as u32) < img.height()
                        {
                            // Source-over at the item alpha:
                            // `composite_image`'s maths with
                            // the glyph colour as the source
                            // (its alpha is 255, so sa = alpha).
                            let dp = img.get_pixel_mut(x as u32, y as u32);
                            let sa = alpha;
                            let da = dp[3] as f32 / 255.0;
                            let out_a = sa + da * (1.0 - sa);
                            if out_a > 0.0 {
                                for ch in 0..3 {
                                    let blended = (rgba[ch] as f32 * sa
                                        + dp[ch] as f32 * da * (1.0 - sa))
                                        / out_a;
                                    dp[ch] = blended.round().clamp(0.0, 255.0) as u8;
                                }
                                dp[3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
                            }
                        }
                    }
                }
            }
        }
    }
}

/// 3x5 bitmap glyph for `ch`: five rows, 3 bits each, bit 2 is the
/// leftmost pixel. Lowercase folds to uppercase; unknown printable
/// characters fall back to `?`.
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
    use crate::model::{
        ClipDef, ClipLoop, Easing, FigureDef, FigureInstance, Frame, ImageAsset, NodeDef,
        NodeStyle, Pose, Shape, SpriteInstance, TextInstance,
    };
    use std::io::Cursor;

    fn png_bytes(w: u32, h: u32, px: Rgba<u8>) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, px);
        let mut buf = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        buf
    }

    /// A real JPEG fixture, encoded by the `image` crate's
    /// JPEG encoder (the `jpeg` feature carries both decode
    /// and encode). JPEG has no alpha, so the fixture is an
    /// RGB image; the pixel values are lossy by design and
    /// no pin reads them back — dimensions and the stored
    /// bytes are what the funnel contracts on.
    fn jpeg_bytes(w: u32, h: u32, px: image::Rgb<u8>) -> Vec<u8> {
        let img = image::RgbImage::from_pixel(w, h, px);
        let mut buf = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Jpeg)
            .unwrap();
        buf
    }

    fn node(
        name: &str,
        parent: Option<usize>,
        length: f32,
        media: Option<SegmentMedia>,
    ) -> NodeDef {
        NodeDef {
            min_angle: None,
            max_angle: None,
            name: name.into(),
            parent,
            length,
            rest_angle: 0.0,
            head: false,
            bend: 1.0,
            thickness: 5.0,
            color: None,
            ik_end: false,
            shape: Shape::Line,
            style: NodeStyle::default(),
            media,
        }
    }

    /// One figure, root at (100, 100): an image segment running
    /// +x for 100 units (pose angle 90 in this model's dir()), and
    /// a text segment running down (+y) for 80. Asset 0 is a 4x2
    /// green PNG (aspect 2.0).
    fn demo_project() -> Project {
        let fig = FigureInstance {
            locked: false,
            opacity: 1.0,
            name: "Media".into(),
            color: 0xFF0000,
            def: FigureDef {
                nodes: vec![
                    node("root", None, 0.0, None),
                    node(
                        "sprite",
                        Some(0),
                        100.0,
                        Some(SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false }),
                    ),
                    node(
                        "label",
                        Some(0),
                        80.0,
                        Some(SegmentMedia::Text {
                            content: "HI".into(),
                            size: 25.0,
                        }),
                    ),
                ],
            },
            hidden: false,
            clip: None,
            states: Vec::new(),
        };
        Project {
            name: "media-test".into(),
            fps: 12.0,
            stage: [800.0, 600.0],
            background: 0,
            figures: vec![fig],
            frames: vec![Frame {
                poses: vec![Pose {
                    root: [100.0, 100.0],
                    angles: vec![0.0, 90.0, 0.0],
                }],
                duration: 1.0,
                easing: None,
                bends: Vec::new(),
            }],
            camera: Vec::new(),
            tween_playback: false,
            easing: Easing::Linear,
            camera_follow: None,
            particles: Vec::new(),
            strokes: Vec::new(),
            audio: Vec::new(),
            clips: Vec::new(),
            fx_clips: Vec::new(),
            assets: vec![ImageAsset {
                name: "green".into(),
                png: png_bytes(4, 2, Rgba([0, 200, 0, 255])),
            }],
            channels: Vec::new(),
            sprites: Vec::new(),
            texts: Vec::new(),
            weather: None,
            head_ring_scale: 1.0,
            subtitles: Vec::new(),
            subtitle_style: Default::default(),
            format_version: crate::model::FORMAT_VERSION_CURRENT,
        }
    }

    fn close(a: V2, b: V2) -> bool {
        (a[0] - b[0]).abs() < 0.01 && (a[1] - b[1]).abs() < 0.01
    }

    /// Wave P (P17 — the settlement pin for P21's divergence):
    /// segment media takes the evaluated (graph-channelled)
    /// frame. P21 proved the stage's segment text follows
    /// channels (its geometry derives from `positions_f`, i.e.
    /// `evaluate(apply_channels(..))`) while `media_items` used
    /// raw poses, so exported text sat on the un-channelled
    /// segment. Both sides now derive from the same evaluation:
    /// under a RootX channel the Text item must sit exactly on
    /// the channelled segment, and the Image item rides along.
    #[test]
    fn segment_media_follows_graph_channels() {
        use crate::graph::{Channel, ChannelTarget, Key};
        let text_item = |project: &Project| -> (V2, f32) {
            media_items(project, 0.0)
                .iter()
                .find_map(|i| match i {
                    MediaItem::Text {
                        pos, angle_deg, ..
                    } => Some((*pos, *angle_deg)),
                    _ => None,
                })
                .expect("a text item")
        };
        // Raw frame: the label segment runs root (100,100) ->
        // (100,180); midpoint (100,140), direction +y = 90 deg.
        let plain = demo_project();
        let (pos, angle) = text_item(&plain);
        assert!(close(pos, [100.0, 140.0]), "raw text pos {pos:?}");
        assert!((angle - 90.0).abs() < 0.01, "raw text angle {angle}");

        let mut channelled = demo_project();
        channelled.channels = vec![Channel {
            figure: 0,
            target: ChannelTarget::RootX,
            keys: vec![Key::flat(0.0, 50.0)],
        }];
        let (pos, angle) = text_item(&channelled);
        // The channelled evaluation itself — the very input the
        // stage's text geometry consumes via `positions_f`.
        let r = crate::graph::apply_channels(&channelled, &channelled.channels, 0.0);
        let pts = evaluate(&r.defs[0], &r.poses[0]);
        let mid = scale(add(pts[0], pts[2]), 0.5);
        assert!(
            close(pos, mid),
            "text must sit on the channelled segment: {pos:?} vs {mid:?}"
        );
        assert!(close(pos, [50.0, 140.0]), "channelled text pos {pos:?}");
        assert!((angle - 90.0).abs() < 0.01, "channelled text angle {angle}");
        // The segment image rides the same funnel: its quad's
        // first corner moves from x=100 to x=50.
        let img_x = media_items(&channelled, 0.0)
            .iter()
            .find_map(|i| match i {
                MediaItem::Image { corners, .. } => Some(corners[0][0]),
                _ => None,
            })
            .expect("an image item");
        assert!(
            (img_x - 50.0).abs() < 0.01,
            "image must follow the channel: {img_x}"
        );
    }

    #[test]
    fn image_item_quad_follows_segment_with_png_aspect() {
        let project = demo_project();
        let items = media_items(&project, 0.0);
        let img = items.iter().find_map(|i| match i {
            MediaItem::Image { corners, asset, .. } => Some((corners, asset)),
            _ => None,
        });
        let (corners, asset) = img.expect("an image item");
        assert_eq!(*asset, 0);
        // Wave 12 (A6 §3a): a default figure/style carries
        // alpha 1.0 — the opacity product starts at unity.
        // Segment (100,100)->(200,100); aspect 2 -> height 50.
        assert!(close(corners[0], [100.0, 75.0]), "c0 {:?}", corners[0]);
        assert!(close(corners[1], [200.0, 75.0]), "c1 {:?}", corners[1]);
        assert!(close(corners[2], [200.0, 125.0]), "c2 {:?}", corners[2]);
        assert!(close(corners[3], [100.0, 125.0]), "c3 {:?}", corners[3]);
        // Long-edge midpoints lie exactly on the segment.
        let top_mid = scale(add(corners[0], corners[1]), 0.5);
        assert!(close(top_mid, [150.0, 75.0]));
    }

    /// Wave X (X4, GAP P-2): the image-side chord contract is
    /// REVERSED BY CHOICE, on the record. Before this wave a
    /// bent segment's image sat on the straight chord while
    /// its segment line arced through `bend_mid` — visibly
    /// wrong in exported output. The image now follows the
    /// arc: two half-quad items split at bend_mid, each
    /// sampling its own half of the source. The TEXT half of
    /// the old contract stands — a label stays one rigid item
    /// on the chord even on a bent segment (stage_text pins
    /// the same rule on its side).
    #[test]
    fn bent_segment_image_splits_into_two_half_quads_at_bend_mid() {
        let mut project = demo_project();
        // Both media nodes bend by 0.9 rad (root, image,
        // label): the image splits, the label must not.
        project.frames[0].bends = vec![vec![0.0, 0.9, 0.9]];
        let items = media_items(&project, 0.0);
        let images: Vec<&MediaItem> = items
            .iter()
            .filter(|i| matches!(i, MediaItem::Image { .. }))
            .collect();
        assert_eq!(images.len(), 2, "a bent segment image is two half-quads");
        let (c1, r1) = match images[0] {
            MediaItem::Image {
                corners, src_rect, ..
            } => (*corners, *src_rect),
            _ => unreachable!(),
        };
        let (c2, r2) = match images[1] {
            MediaItem::Image {
                corners, src_rect, ..
            } => (*corners, *src_rect),
            _ => unreachable!(),
        };
        assert_eq!(r1, [0.0, 0.0, 0.5, 1.0], "first half samples the left source half");
        assert_eq!(r2, [0.5, 0.0, 1.0, 1.0], "second half samples the right source half");
        // The outer corners are the straight quad's own
        // (segment (100,100)->(200,100), aspect 2 -> height 50).
        assert!(close(c1[0], [100.0, 75.0]), "c1[0] {:?}", c1[0]);
        assert!(close(c1[3], [100.0, 125.0]), "c1[3] {:?}", c1[3]);
        assert!(close(c2[1], [200.0, 75.0]), "c2[1] {:?}", c2[1]);
        assert!(close(c2[2], [200.0, 125.0]), "c2[2] {:?}", c2[2]);
        // The shared edge is exactly the bend arc midpoint
        // displaced by the quad's half-height — the same
        // bend_mid the segment line splits at.
        let arc_mid = crate::model::bend_mid([100.0, 100.0], [200.0, 100.0], 0.9);
        assert!(
            (arc_mid[1] - 100.0).abs() > 1.0,
            "the fixture bend must displace the midpoint off the chord: {arc_mid:?}"
        );
        assert!(close(c1[1], [arc_mid[0], arc_mid[1] - 25.0]), "edge lo {:?}", c1[1]);
        assert!(close(c1[2], [arc_mid[0], arc_mid[1] + 25.0]), "edge hi {:?}", c1[2]);
        assert_eq!(c1[1], c2[0], "the halves share the split edge exactly");
        assert_eq!(c1[2], c2[3], "the halves share the split edge exactly");
        // The bent label segment still yields ONE text item,
        // on the chord (midpoint (100, 140), angle 90).
        let texts: Vec<&MediaItem> = items
            .iter()
            .filter(|i| matches!(i, MediaItem::Text { .. }))
            .collect();
        assert_eq!(texts.len(), 1, "text never splits");
        match texts[0] {
            MediaItem::Text { pos, angle_deg, .. } => {
                assert!(close(*pos, [100.0, 140.0]), "label stays on the chord: {pos:?}");
                assert!((*angle_deg - 90.0).abs() < 0.01, "label angle {angle_deg}");
            }
            _ => unreachable!(),
        }
        // Control: the same project unbent emits ONE image
        // item with the full source rect.
        let straight = demo_project();
        let items = media_items(&straight, 0.0);
        let images: Vec<&MediaItem> = items
            .iter()
            .filter(|i| matches!(i, MediaItem::Image { .. }))
            .collect();
        assert_eq!(images.len(), 1, "an unbent segment image stays one item");
        match images[0] {
            MediaItem::Image { src_rect, .. } => {
                assert_eq!(*src_rect, [0.0, 0.0, 1.0, 1.0]);
            }
            _ => unreachable!(),
        }
    }

    /// Wave X (X4): the compositor honours the split — each
    /// half-quad samples its own half of the source, and the
    /// painted region follows the arc off the old chord quad.
    #[test]
    fn bent_segment_image_composites_along_the_arc() {
        // Two-tone asset: left half red, right half blue.
        let mut project = demo_project();
        let mut img = RgbaImage::new(4, 2);
        for x in 0..4 {
            for y in 0..2 {
                img.put_pixel(
                    x,
                    y,
                    if x < 2 {
                        Rgba([255, 0, 0, 255])
                    } else {
                        Rgba([0, 0, 255, 255])
                    },
                );
            }
        }
        let mut buf = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        project.assets[0].png = buf;
        project.frames[0].bends = vec![vec![0.0, 1.0, 0.0]];
        let mut out = RgbaImage::from_pixel(400, 300, Rgba([255, 255, 255, 255]));
        composite_media(&project, 0.0, &mut out, &|p| p);
        // Half-quad centres sample their own source halves.
        assert_eq!(*out.get_pixel(125, 106), Rgba([255, 0, 0, 255]), "half 1 is red");
        assert_eq!(*out.get_pixel(175, 106), Rgba([0, 0, 255, 255]), "half 2 is blue");
        // (140, 130) lay BELOW the old chord quad (which
        // ended at y=125 on this segment): the bent quad
        // paints it now, from half 1's source (red).
        assert_eq!(
            *out.get_pixel(140, 130),
            Rgba([255, 0, 0, 255]),
            "the arc paints below the old chord"
        );
        // (140, 80) lay INSIDE the old chord quad: the arc
        // has moved off it, so it stays background.
        assert_eq!(
            *out.get_pixel(140, 80),
            Rgba([255, 255, 255, 255]),
            "the image left the chord"
        );
    }

    /// Wave Y (Y3, GAP P-8): the demo segment's image node
    /// (index 1) re-attached with a local transform.
    fn project_with_transform(
        offset: [f32; 2],
        scale: f32,
        angle_offset: f32,
        flip: bool,
    ) -> Project {
        let mut project = demo_project();
        project.figures[0].def.nodes[1].media = Some(SegmentMedia::Image {
            asset: 0,
            offset,
            scale,
            angle_offset,
            flip,
        });
        project
    }

    fn image_items(project: &Project) -> Vec<([V2; 4], [f32; 4])> {
        media_items(project, 0.0)
            .iter()
            .filter_map(|i| match i {
                MediaItem::Image { corners, src_rect, .. } => {
                    Some((*corners, *src_rect))
                }
                _ => None,
            })
            .collect()
    }

    /// Wave Y (Y3, GAP P-8): the local transform composes in
    /// the funnel — offset in the segment frame, scale about
    /// the centre, angle_offset rotating the fitted axes.
    /// Demo segment (100,100)->(200,100), aspect 2: identity
    /// corners [[100,75],[200,75],[200,125],[100,125]].
    #[test]
    fn segment_image_transform_offset_scale_angle_compose_in_the_funnel() {
        // Offset [10, 20]: the centre moves +10 along the
        // segment direction, +20 along its normal.
        let items = image_items(&project_with_transform([10.0, 20.0], 1.0, 0.0, false));
        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0].0,
            [[110.0, 95.0], [210.0, 95.0], [210.0, 145.0], [110.0, 145.0]]
        );
        assert_eq!(items[0].1, [0.0, 0.0, 1.0, 1.0]);
        // Scale 2 about the (offset) centre: both half-extents
        // double.
        let items = image_items(&project_with_transform([10.0, 0.0], 2.0, 0.0, false));
        assert_eq!(
            items[0].0,
            [[60.0, 50.0], [260.0, 50.0], [260.0, 150.0], [60.0, 150.0]]
        );
        // Angle 90°: the fitted axes swap — the quad stands
        // across the segment, 50 wide and 100 tall.
        let items = image_items(&project_with_transform([0.0, 0.0], 1.0, 90.0, false));
        let want = [[175.0, 50.0], [175.0, 150.0], [125.0, 150.0], [125.0, 50.0]];
        for (got, want) in items[0].0.iter().zip(want.iter()) {
            assert!(close(*got, *want), "rotated corner {got:?} != {want:?}");
        }
    }

    /// Wave Y (Y3, GAP P-8): flip leaves the geometry alone
    /// and reverses the sampled source — the compositor paints
    /// the two-tone asset mirrored.
    #[test]
    fn segment_image_flip_reverses_src_rect_and_mirrors_the_composite() {
        let items = image_items(&project_with_transform([0.0, 0.0], 1.0, 0.0, true));
        assert_eq!(items.len(), 1);
        assert_eq!(
            items[0].0,
            [[100.0, 75.0], [200.0, 75.0], [200.0, 125.0], [100.0, 125.0]],
            "flip does not move the quad"
        );
        assert_eq!(items[0].1, [1.0, 0.0, 0.0, 1.0], "flip reverses the u range");
        // Two-tone asset: left half red, right half blue
        // (the X4 composite fixture's construction).
        let two_tone = |flip: bool| {
            let mut project = project_with_transform([0.0, 0.0], 1.0, 0.0, flip);
            let mut img = RgbaImage::new(4, 2);
            for x in 0..4 {
                for y in 0..2 {
                    img.put_pixel(
                        x,
                        y,
                        if x < 2 { Rgba([255, 0, 0, 255]) } else { Rgba([0, 0, 255, 255]) },
                    );
                }
            }
            let mut buf = Vec::new();
            image::DynamicImage::ImageRgba8(img)
                .write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Png)
                .unwrap();
            project.assets[0].png = buf;
            let mut out = RgbaImage::from_pixel(400, 300, Rgba([255, 255, 255, 255]));
            composite_media(&project, 0.0, &mut out, &|p| p);
            out
        };
        let plain = two_tone(false);
        assert_eq!(*plain.get_pixel(125, 100), Rgba([255, 0, 0, 255]), "control: parent end is red");
        assert_eq!(*plain.get_pixel(175, 100), Rgba([0, 0, 255, 255]), "control: node end is blue");
        let flipped = two_tone(true);
        assert_eq!(*flipped.get_pixel(125, 100), Rgba([0, 0, 255, 255]), "flipped: parent end is blue");
        assert_eq!(*flipped.get_pixel(175, 100), Rgba([255, 0, 0, 255]), "flipped: node end is red");
    }

    /// Wave Y (Y3 on Y9's strips): a flipped BENT image
    /// reverses every strip's u range and the strips still
    /// share their edges exactly.
    #[test]
    fn segment_image_flip_bent_reverses_each_strip_rect() {
        let mut project = project_with_transform([0.0, 0.0], 1.0, 0.0, true);
        project.frames[0].bends = vec![vec![0.0, 0.9, 0.0]];
        let items = image_items(&project);
        assert_eq!(items.len(), 2, "bend 0.9 is two strips");
        assert_eq!(items[0].1, [1.0, 0.0, 0.5, 1.0]);
        assert_eq!(items[1].1, [0.5, 0.0, 0.0, 1.0]);
        assert_eq!(items[0].0[1], items[1].0[0], "shared edge lo meets exactly");
        assert_eq!(items[0].0[2], items[1].0[3], "shared edge hi meets exactly");
    }

    /// Wave Y (Y3): unusable transform values are sanitised
    /// at composition — the funnel emits the identity items
    /// rather than a degenerate quad.
    #[test]
    fn segment_image_unusable_transform_values_compose_as_identity() {
        let project = project_with_transform(
            [f32::INFINITY, 0.0],
            f32::NAN,
            f32::NEG_INFINITY,
            false,
        );
        assert_eq!(
            image_items(&project),
            image_items(&demo_project()),
            "non-finite offset/scale/angle sanitise to the identity composition"
        );
        let project = project_with_transform([0.0, 0.0], 0.0, 0.0, false);
        assert_eq!(
            image_items(&project),
            image_items(&demo_project()),
            "a zero scale sanitises to 1 rather than collapsing the quad"
        );
    }

    /// Wave Y (Y9, GAP P-11): from 1.25 rad of bend the
    /// two half-quads generalise to K strips along the arc.
    /// The strips are one chain: the spine is sampled once,
    /// so consecutive strips share their edge VALUES (assert_eq
    /// on the floats, not a tolerance), the source rects tile
    /// [0, 1] exactly, and the chain's end caps are the unbent
    /// quad's own corners.
    #[test]
    fn bent_segment_image_warps_into_k_strips_sharing_spine_points() {
        let mut project = demo_project();
        // Bend 2.0 rad -> K = 4 strips (one per ~0.5 rad).
        project.frames[0].bends = vec![vec![0.0, 2.0, 0.0]];
        let items = media_items(&project, 0.0);
        let strips: Vec<([V2; 4], [f32; 4])> = items
            .iter()
            .filter_map(|i| match i {
                MediaItem::Image {
                    corners, src_rect, ..
                } => Some((*corners, *src_rect)),
                _ => None,
            })
            .collect();
        assert_eq!(strips.len(), 4, "bend 2.0 warps into 4 strips");
        // Shared spine points: strip i's end edge IS strip
        // i+1's start edge, float for float.
        for w in strips.windows(2) {
            assert_eq!(w[0].0[1], w[1].0[0], "shared edge lo meets exactly");
            assert_eq!(w[0].0[2], w[1].0[3], "shared edge hi meets exactly");
        }
        // The source tiles exactly: contiguous u ranges from
        // the same computed fractions, covering [0, 1].
        assert_eq!(
            strips.iter().map(|s| s.1).collect::<Vec<_>>(),
            vec![
                [0.0, 0.0, 0.25, 1.0],
                [0.25, 0.0, 0.5, 1.0],
                [0.5, 0.0, 0.75, 1.0],
                [0.75, 0.0, 1.0, 1.0],
            ],
            "strip src_rects tile the source"
        );
        // End caps are the unbent item's own corners.
        let straight = media_items(&demo_project(), 0.0);
        let cap = straight
            .iter()
            .find_map(|i| match i {
                MediaItem::Image { corners, .. } => Some(*corners),
                _ => None,
            })
            .expect("the unbent item");
        assert_eq!(strips[0].0[0], cap[0], "chain starts at the quad's c0");
        assert_eq!(strips[0].0[3], cap[3], "chain starts at the quad's c3");
        assert_eq!(strips[3].0[1], cap[1], "chain ends at the quad's c1");
        assert_eq!(strips[3].0[2], cap[2], "chain ends at the quad's c2");
        // Interior edges ride the arc: each shared edge's
        // midpoint is the arc point at that fraction (the
        // demo segment runs (100,100)->(200,100), hh = 25).
        for i in 1..4 {
            let edge_mid = scale(add(strips[i].0[0], strips[i].0[3]), 0.5);
            let on_arc = crate::model::bend_arc_point(
                [100.0, 100.0],
                [200.0, 100.0],
                2.0,
                i as f32 / 4.0,
            );
            assert!(
                close(edge_mid, on_arc),
                "edge {i} midpoint {edge_mid:?} rides the arc point {on_arc:?}"
            );
        }
    }

    /// Wave Y (Y9): the strip count follows the bend — the
    /// P-2 pair below 1.25 rad, then one strip per ~0.5 rad,
    /// capped at 8; the sign of the bend is irrelevant.
    #[test]
    fn bend_strip_count_grows_with_bend_and_caps() {
        for (bend, want) in [
            (0.2, 2),
            (0.9, 2),
            (1.5, 3),
            (2.0, 4),
            (-2.0, 4),
            (3.5, 7),
            (6.0, 8),
        ] {
            let mut project = demo_project();
            project.frames[0].bends = vec![vec![0.0, bend, 0.0]];
            let n = media_items(&project, 0.0)
                .iter()
                .filter(|i| matches!(i, MediaItem::Image { .. }))
                .count();
            assert_eq!(n, want, "bend {bend} yields {want} strips");
        }
    }

    /// Wave Y (Y9): a straight segment still emits the single
    /// historical item, byte-identical — the strip warp is a
    /// bent-only path and the unbent stream must not drift
    /// (the demo quad: segment (100,100)->(200,100), aspect
    /// 2 -> height 50, alpha 1).
    #[test]
    fn straight_segment_image_is_the_historical_single_item() {
        let project = demo_project();
        let items = media_items(&project, 0.0);
        let images: Vec<&MediaItem> = items
            .iter()
            .filter(|i| matches!(i, MediaItem::Image { .. }))
            .collect();
        assert_eq!(images.len(), 1, "an unbent segment image stays one item");
        assert_eq!(
            images[0],
            &MediaItem::Image {
                corners: [
                    [100.0, 75.0],
                    [200.0, 75.0],
                    [200.0, 125.0],
                    [100.0, 125.0],
                ],
                asset: 0,
                alpha: 1.0,
                src_rect: [0.0, 0.0, 1.0, 1.0],
            },
            "the unbent item is byte-identical to the historical form"
        );
    }

    /// Wave Y (Y9): the raster honours the strips — with a
    /// 4-band source and bend 2.0, each strip's centroid
    /// paints its own band's colour, so the raster strip
    /// count agrees with the funnel's (4) by construction.
    #[test]
    fn bent_segment_image_strips_composite_their_own_source_slices() {
        let bands = [
            Rgba([255, 0, 0, 255]),
            Rgba([0, 200, 0, 255]),
            Rgba([0, 0, 255, 255]),
            Rgba([255, 200, 0, 255]),
        ];
        let mut project = demo_project();
        let mut img = RgbaImage::new(4, 2);
        for x in 0..4 {
            for y in 0..2 {
                img.put_pixel(x, y, bands[x as usize]);
            }
        }
        let mut buf = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        project.assets[0].png = buf;
        project.frames[0].bends = vec![vec![0.0, 2.0, 0.0]];
        let strips: Vec<[V2; 4]> = media_items(&project, 0.0)
            .iter()
            .filter_map(|i| match i {
                MediaItem::Image { corners, .. } => Some(*corners),
                _ => None,
            })
            .collect();
        assert_eq!(strips.len(), 4, "the funnel emits 4 strips");
        let mut out = RgbaImage::from_pixel(400, 300, Rgba([255, 255, 255, 255]));
        composite_media(&project, 0.0, &mut out, &|p| p);
        for (i, c) in strips.iter().enumerate() {
            let cx = (c[0][0] + c[1][0] + c[2][0] + c[3][0]) / 4.0;
            let cy = (c[0][1] + c[1][1] + c[2][1] + c[3][1]) / 4.0;
            assert_eq!(
                *out.get_pixel(cx.round() as u32, cy.round() as u32),
                bands[i],
                "strip {i}'s centroid samples band {i}"
            );
        }
    }

    #[test]
    fn image_item_alpha_is_figure_opacity_times_style_opacity() {
        // Wave 12 (A6 §3a): segment media used to composite at
        // full strength whatever the figure or the node style
        // said. The item now carries the product of both.
        let alpha_of = |project: &Project| -> f32 {
            media_items(project, 0.0)
                .iter()
                .find_map(|i| match i {
                    MediaItem::Image { alpha, .. } => Some(*alpha),
                    _ => None,
                })
                .expect("an image item")
        };
        let project = demo_project();
        assert_eq!(alpha_of(&project), 1.0, "defaults multiply to unity");
        let mut project = demo_project();
        project.figures[0].opacity = 0.5;
        assert_eq!(alpha_of(&project), 0.5, "figure opacity alone");
        project.figures[0].def.nodes[1].style.opacity = 0.5;
        assert_eq!(alpha_of(&project), 0.25, "figure × style");
        // Out-of-range stored values clamp per factor, exactly
        // as the segment's own alpha does.
        project.figures[0].opacity = 1.4;
        project.figures[0].def.nodes[1].style.opacity = 2.0;
        assert_eq!(alpha_of(&project), 1.0, "each factor clamps to 1");
    }

    #[test]
    fn text_item_sits_at_midpoint_along_segment() {
        let project = demo_project();
        let items = media_items(&project, 0.0);
        let text = items.iter().find_map(|i| match i {
            MediaItem::Text {
                pos,
                angle_deg,
                content,
                size,
                color,
                ..
            } => Some((pos, angle_deg, content, size, color)),
            _ => None,
        });
        let (pos, angle, content, size, color) = text.expect("a text item");
        assert!(close(*pos, [100.0, 140.0]), "pos {pos:?}");
        assert!((angle - 90.0).abs() < 0.01, "angle {angle}");
        assert_eq!(content, "HI");
        assert_eq!(*size, 25.0);
        assert_eq!(*color, 0xFF0000, "falls back to the figure colour");
    }

    #[test]
    fn composite_paints_image_quad_and_text_pixels() {
        let project = demo_project();
        let mut img = RgbaImage::from_pixel(400, 300, Rgba([255, 255, 255, 255]));
        composite_media(&project, 0.0, &mut img, &|p| p);
        // Centre of the sprite quad is the asset's green.
        assert_eq!(*img.get_pixel(150, 100), Rgba([0, 200, 0, 255]));
        // Inside the quad but off-centre, too.
        assert_eq!(*img.get_pixel(110, 80), Rgba([0, 200, 0, 255]));
        // A known "on" cell of the H: text runs downward from
        // (100, 140); the first glyph cell centre lands at
        // (107.5, 125), just below the sprite quad's bottom edge.
        assert_eq!(*img.get_pixel(108, 125), Rgba([255, 0, 0, 255]));
        // Far corner untouched.
        assert_eq!(*img.get_pixel(10, 10), Rgba([255, 255, 255, 255]));
    }

    #[test]
    fn missing_or_broken_assets_are_skipped() {
        let mut project = demo_project();
        project.assets.clear();
        project.assets.push(ImageAsset {
            name: "junk".into(),
            png: vec![1, 2, 3],
        });
        let items = media_items(&project, 0.0);
        assert_eq!(items.len(), 1, "only the text item survives");
        assert!(matches!(items[0], MediaItem::Text { .. }));
    }

    #[test]
    fn glyph_table_covers_charset_and_folds_case() {
        for ch in "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.,!?'-:;()+/&%*=<>_#$@\"".chars() {
            let g = glyph_rows(ch);
            let lit: u32 = g.iter().map(|r| r.count_ones()).sum();
            assert!(lit > 0, "glyph for {ch:?} is blank");
        }
        assert_eq!(glyph_rows('a'), glyph_rows('A'));
        assert_eq!(glyph_rows('~'), glyph_rows('?'), "unknown -> ?");
    }

    fn image_items(items: &[MediaItem]) -> Vec<([V2; 4], usize)> {
        items
            .iter()
            .filter_map(|i| match i {
                MediaItem::Image { corners, asset, .. } => Some((*corners, *asset)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn sprite_instance_item_geometry_and_sampling() {
        let mut project = demo_project();
        project.sprites.push(SpriteInstance {
            asset: 0,
            keys: vec![
                InstanceKey {
                    frame: 0.0,
                    x: 300.0,
                    y: 200.0,
                    scale: 10.0,
                    angle: 0.0,
                    opacity: 1.0,
                    flip_x: false,
                    flip_y: false,
                    scale_y: 0.0,
                    pivot_x: 0.5,
                    pivot_y: 0.5,
                },
                InstanceKey {
                    frame: 2.0,
                    x: 340.0,
                    y: 200.0,
                    scale: 10.0,
                    angle: 90.0,
                    opacity: 1.0,
                    flip_x: false,
                    flip_y: false,
                    scale_y: 0.0,
                    pivot_x: 0.5,
                    pivot_y: 0.5,
                },
            ],
        });
        // Frame 0: asset is 4x2 px, scale 10 -> 40x20 quad at
        // (300, 200), unrotated.
        let items = media_items(&project, 0.0);
        let images = image_items(&items);
        let (corners, _) = images.last().expect("sprite instance item");
        assert!(close(corners[0], [280.0, 190.0]), "c0 {corners:?}");
        assert!(close(corners[1], [320.0, 190.0]), "c1 {corners:?}");
        assert!(close(corners[2], [320.0, 210.0]), "c2 {corners:?}");
        assert!(close(corners[3], [280.0, 210.0]), "c3 {corners:?}");
        // Frame 1: keys interpolate — centre (320, 200).
        let items = media_items(&project, 1.0);
        let images = image_items(&items);
        let (corners, _) = images.last().expect("sprite instance item");
        let centre = scale(add(corners[0], corners[2]), 0.5);
        assert!(close(centre, [320.0, 200.0]), "centre {centre:?}");
        // Frame 2: rotated 90 degrees — the quad is 20 wide, 40 tall.
        let items = media_items(&project, 2.0);
        let images = image_items(&items);
        let (corners, _) = images.last().expect("sprite instance item");
        assert!(close(corners[0], [350.0, 180.0]), "c0 {corners:?}");
        assert!(close(corners[1], [350.0, 220.0]), "c1 {corners:?}");
    }

    #[test]
    fn text_instance_item_uses_size_times_key_scale() {
        let mut project = demo_project();
        project.texts.push(TextInstance {
            content: "YO".into(),
            size: 25.0,
            color: 0x0000FF,
            keys: vec![InstanceKey {
                frame: 0.0,
                x: 100.0,
                y: 400.0,
                scale: 2.0,
                angle: 0.0,
                opacity: 1.0,
                flip_x: false,
                flip_y: false,
                scale_y: 0.0,
                pivot_x: 0.5,
                pivot_y: 0.5,
            }],
        });
        let items = media_items(&project, 0.0);
        // The instance text comes after the segment "HI" text.
        let text = items.iter().rev().find_map(|i| match i {
            MediaItem::Text {
                pos,
                content,
                size,
                color,
                ..
            } => Some((pos, content, size, color)),
            _ => None,
        });
        let (pos, content, size, color) = text.expect("a text instance item");
        assert!(close(*pos, [100.0, 400.0]), "pos {pos:?}");
        assert_eq!(content, "YO");
        assert_eq!(*size, 50.0, "25 * key scale 2");
        assert_eq!(*color, 0x0000FF);
    }

    #[test]
    fn composite_instances_draw_over_segment_media() {
        let mut project = demo_project();
        project.assets.push(ImageAsset {
            name: "red".into(),
            png: png_bytes(2, 2, Rgba([255, 0, 0, 255])),
        });
        // Red sprite instance (2x2 px * scale 10 = 20x20) centred
        // on the segment image's centre (150, 100).
        project.sprites.push(SpriteInstance {
            asset: 1,
            keys: vec![InstanceKey {
                frame: 0.0,
                x: 150.0,
                y: 100.0,
                scale: 10.0,
                angle: 0.0,
                opacity: 1.0,
                flip_x: false,
                flip_y: false,
                scale_y: 0.0,
                pivot_x: 0.5,
                pivot_y: 0.5,
            }],
        });
        project.texts.push(TextInstance {
            content: "H".into(),
            size: 25.0,
            color: 0x0000FF,
            keys: vec![InstanceKey {
                frame: 0.0,
                x: 100.0,
                y: 250.0,
                scale: 1.0,
                angle: 0.0,
                opacity: 1.0,
                flip_x: false,
                flip_y: false,
                scale_y: 0.0,
                pivot_x: 0.5,
                pivot_y: 0.5,
            }],
        });
        let mut img = RgbaImage::from_pixel(400, 300, Rgba([255, 255, 255, 255]));
        composite_media(&project, 0.0, &mut img, &|p| p);
        // The instance sprite paints over the segment's green.
        assert_eq!(*img.get_pixel(150, 100), Rgba([255, 0, 0, 255]));
        // Green still visible outside the instance quad.
        assert_eq!(*img.get_pixel(110, 80), Rgba([0, 200, 0, 255]));
        // The text instance stamps blue glyph pixels in its band.
        let mut blue = 0;
        for y in 238..262 {
            for x in 80..120 {
                if *img.get_pixel(x, y) == Rgba([0, 0, 255, 255]) {
                    blue += 1;
                }
            }
        }
        assert!(blue > 10, "text instance painted only {blue} blue pixels");
    }

    // ---- Wave 12: asset-table MCP handlers ----

    use crate::editor::EditorCore;
    use serde_json::json;

    fn asset_core() -> EditorCore {
        let mut core = EditorCore::new();
        core.project = demo_project();
        core
    }

    fn temp_asset_file(tag: &str, bytes: &[u8]) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "stykanimate-assets-{tag}-{}.png",
            std::process::id()
        ));
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn asset_import_refuses_bad_magic_oversize_and_undecodable() {
        let mut core = asset_core();
        let undo_before = core.undo_depth();
        // Bad magic: not a PNG at all.
        let bad = temp_asset_file("badmagic", b"this is not a png file");
        let err = tool_asset_import(
            &mut core,
            &json!({ "path": bad.to_string_lossy() }),
        )
        .expect_err("bad magic must be refused");
        assert!(err.contains("not a PNG"), "err: {err}");
        // Oversize: valid magic, but over the byte cap (the size
        // check fires on metadata before any decode).
        let mut big = PNG_MAGIC.to_vec();
        big.resize(ASSET_IMPORT_MAX_BYTES as usize + 1, 0);
        let big_path = temp_asset_file("oversize", &big);
        let err = tool_asset_import(
            &mut core,
            &json!({ "path": big_path.to_string_lossy() }),
        )
        .expect_err("oversize must be refused");
        assert!(err.contains("import limit"), "err: {err}");
        // Valid magic, undecodable body.
        let mut junk = PNG_MAGIC.to_vec();
        junk.extend_from_slice(b"garbagegarbage");
        let junk_path = temp_asset_file("undecodable", &junk);
        let err = tool_asset_import(
            &mut core,
            &json!({ "path": junk_path.to_string_lossy() }),
        )
        .expect_err("undecodable must be refused");
        assert!(err.contains("does not decode"), "err: {err}");
        // Nothing was stored and no undo step was spent.
        assert_eq!(core.project.assets.len(), 1);
        assert_eq!(core.undo_depth(), undo_before);
        for p in [bad, big_path, junk_path] {
            let _ = std::fs::remove_file(p);
        }
    }

    #[test]
    fn asset_import_appends_one_asset_in_one_undo_step() {
        let mut core = asset_core();
        let undo_before = core.undo_depth();
        let bytes = png_bytes(6, 3, Rgba([9, 9, 9, 255]));
        let path = temp_asset_file("ok", &bytes);
        let out = tool_asset_import(
            &mut core,
            &json!({ "path": path.to_string_lossy() }),
        )
        .expect("a valid PNG imports");
        assert_eq!(out["index"].as_u64(), Some(1));
        assert_eq!(out["width"].as_u64(), Some(6));
        assert_eq!(out["height"].as_u64(), Some(3));
        assert_eq!(core.project.assets.len(), 2);
        assert_eq!(core.project.assets[1].png, bytes);
        assert_eq!(core.undo_depth(), undo_before + 1, "exactly one undo step");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn asset_import_accepts_jpeg_by_content_and_refuses_jpeg_magic_junk() {
        // Wave R (R9): JPEG imports through the same funnel
        // as PNG. The temp file's name ends in `.png` (the
        // helper's fixed shape) while its bytes are JPEG —
        // the import must accept on content, never on name.
        let mut core = asset_core();
        let undo_before = core.undo_depth();
        let bytes = jpeg_bytes(6, 3, image::Rgb([200, 30, 30]));
        assert!(bytes.starts_with(&JPEG_MAGIC), "fixture is a real JPEG");
        let path = temp_asset_file("jpegok", &bytes);
        let out = tool_asset_import(
            &mut core,
            &json!({ "path": path.to_string_lossy() }),
        )
        .expect("a valid JPEG imports");
        assert_eq!(out["index"].as_u64(), Some(1));
        assert_eq!(out["width"].as_u64(), Some(6));
        assert_eq!(out["height"].as_u64(), Some(3));
        assert_eq!(core.project.assets.len(), 2);
        assert_eq!(core.project.assets[1].png, bytes, "stored verbatim");
        assert_eq!(core.undo_depth(), undo_before + 1, "exactly one undo step");
        // The dimension funnel the renderer reads reports the
        // JPEG's dims exactly like a PNG's.
        assert_eq!(png_dims(&core.project, 1), Some((6, 3)));
        // JPEG magic over a garbage body is refused at the
        // decode step, in the funnel's existing voice, and
        // spends nothing.
        let mut junk = JPEG_MAGIC.to_vec();
        junk.extend_from_slice(b"garbagegarbage");
        let junk_path = temp_asset_file("jpegundecodable", &junk);
        let err = tool_asset_import(
            &mut core,
            &json!({ "path": junk_path.to_string_lossy() }),
        )
        .expect_err("JPEG magic over garbage must be refused");
        assert!(err.contains("does not decode"), "err: {err}");
        assert_eq!(core.project.assets.len(), 2);
        assert_eq!(core.undo_depth(), undo_before + 1);
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(junk_path);
    }

    #[test]
    fn asset_remove_refused_while_any_reference_kind_points_at_it() {
        let mut core = asset_core();
        let undo_before = core.undo_depth();
        // (a) The demo's segment image references asset 0.
        let err = tool_asset_remove(&mut core, &json!({ "index": 0 }))
            .expect_err("segment-referenced asset must be refused");
        assert!(err.contains("1 segment image(s)"), "err: {err}");
        // (b) Clear the segment; a sprite instance references it.
        core.project.figures[0].def.nodes[1].media = None;
        core.project.sprites.push(SpriteInstance {
            asset: 0,
            keys: vec![InstanceKey {
                frame: 0.0,
                x: 0.0,
                y: 0.0,
                scale: 1.0,
                angle: 0.0,
                opacity: 1.0,
                flip_x: false,
                flip_y: false,
                scale_y: 0.0,
                pivot_x: 0.5,
                pivot_y: 0.5,
            }],
        });
        let err = tool_asset_remove(&mut core, &json!({ "index": 0 }))
            .expect_err("sprite-referenced asset must be refused");
        assert!(err.contains("1 sprite instance(s)"), "err: {err}");
        // (c) Clear the sprite; a movieclip def references it.
        core.project.sprites.clear();
        core.project.clips.push(ClipDef {
            name: "clip".into(),
            def: FigureDef {
                nodes: vec![
                    node("root", None, 0.0, None),
                    node("img", Some(0), 50.0, Some(SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false })),
                ],
            },
            frames: Vec::new(),
            loop_mode: ClipLoop::Loop,
        });
        let err = tool_asset_remove(&mut core, &json!({ "index": 0 }))
            .expect_err("clip-referenced asset must be refused");
        assert!(err.contains("1 clip segment(s)"), "err: {err}");
        // Refusals never mutate and never snapshot.
        assert_eq!(core.project.assets.len(), 1);
        assert_eq!(core.undo_depth(), undo_before);
    }

    #[test]
    fn asset_remove_unreferenced_remaps_higher_references_in_same_step() {
        let mut core = asset_core();
        // Three assets; the demo segment and a sprite both use
        // asset 2; asset 0 is unreferenced.
        core.project.assets.push(ImageAsset {
            name: "spare".into(),
            png: png_bytes(2, 2, Rgba([1, 2, 3, 255])),
        });
        core.project.assets.push(ImageAsset {
            name: "hero".into(),
            png: png_bytes(2, 2, Rgba([4, 5, 6, 255])),
        });
        core.project.figures[0].def.nodes[1].media =
            Some(SegmentMedia::Image { asset: 2, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false });
        core.project.sprites.push(SpriteInstance {
            asset: 2,
            keys: vec![InstanceKey {
                frame: 0.0,
                x: 0.0,
                y: 0.0,
                scale: 1.0,
                angle: 0.0,
                opacity: 1.0,
                flip_x: false,
                flip_y: false,
                scale_y: 0.0,
                pivot_x: 0.5,
                pivot_y: 0.5,
            }],
        });
        let undo_before = core.undo_depth();
        let out = tool_asset_remove(&mut core, &json!({ "index": 0 }))
            .expect("unreferenced removal succeeds");
        assert_eq!(out["remapped"].as_u64(), Some(2));
        assert_eq!(core.project.assets.len(), 2);
        assert_eq!(core.project.assets[1].name, "hero");
        // Both references followed their image down one index.
        assert_eq!(
            core.project.figures[0].def.nodes[1].media,
            Some(SegmentMedia::Image { asset: 1, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false })
        );
        assert_eq!(core.project.sprites[0].asset, 1);
        assert_eq!(core.undo_depth(), undo_before + 1, "remove + remap = one undo step");
    }

    #[test]
    fn asset_remove_remaps_clip_def_references_and_info_counts_them() {
        // Wave 14 (C6): the clip-def row of the reference
        // census — pinned through the public surface both
        // ways: `asset_info` reports the clip count, and a
        // removal below the referenced index shifts the
        // clip-def reference down with the others. (The
        // sibling remap test above covers segments +
        // sprites; the refusal test covers clip blocking.)
        let mut core = asset_core();
        core.project.assets.push(ImageAsset {
            name: "spare".into(),
            png: png_bytes(2, 2, Rgba([1, 2, 3, 255])),
        });
        core.project.assets.push(ImageAsset {
            name: "hero".into(),
            png: png_bytes(2, 2, Rgba([4, 5, 6, 255])),
        });
        core.project.clips.push(ClipDef {
            name: "clip".into(),
            def: FigureDef {
                nodes: vec![
                    node("root", None, 0.0, None),
                    node("img", Some(0), 50.0, Some(SegmentMedia::Image { asset: 2, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false })),
                ],
            },
            frames: Vec::new(),
            loop_mode: ClipLoop::Loop,
        });
        // The census counts the clip-def reference, in its
        // own bucket only.
        let info = tool_asset_info(&mut core, &json!({ "index": 2 }))
            .expect("info on the clip-referenced asset succeeds");
        assert_eq!(info["references"]["clip_segments"].as_u64(), Some(1));
        assert_eq!(info["references"]["segments"].as_u64(), Some(0));
        assert_eq!(info["references"]["sprites"].as_u64(), Some(0));
        // Remove the unreferenced spare at index 1: the
        // clip-def reference follows its image down to 1,
        // in the same single undo step.
        let undo_before = core.undo_depth();
        let out = tool_asset_remove(&mut core, &json!({ "index": 1 }))
            .expect("unreferenced removal succeeds");
        assert_eq!(out["remapped"].as_u64(), Some(1));
        assert_eq!(core.project.assets.len(), 2);
        assert_eq!(core.project.assets[1].name, "hero");
        assert_eq!(
            core.project.clips[0].def.nodes[1].media,
            Some(SegmentMedia::Image { asset: 1, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false })
        );
        assert_eq!(core.undo_depth(), undo_before + 1, "remove + remap = one undo step");
    }

    #[test]
    fn asset_attach_uses_an_existing_asset_and_validates_indices() {
        let mut core = asset_core();
        let undo_before = core.undo_depth();
        // Node 2 carries the demo's text; attach replaces it.
        let out = tool_asset_attach(
            &mut core,
            &json!({ "figure": 0, "node": 2, "asset": 0 }),
        )
        .expect("attach succeeds");
        assert_eq!(out["status"].as_str(), Some("Sprite attached (asset 1)."));
        assert_eq!(
            core.project.figures[0].def.nodes[2].media,
            Some(SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false })
        );
        assert_eq!(core.undo_depth(), undo_before + 1);
        // A missing asset index is an error, not a snapshot.
        let err = tool_asset_attach(
            &mut core,
            &json!({ "figure": 0, "node": 2, "asset": 7 }),
        )
        .expect_err("missing asset must be refused");
        assert!(err.contains("out of range"), "err: {err}");
        assert_eq!(core.undo_depth(), undo_before + 1);
    }

    #[test]
    fn asset_attach_with_transform_args_writes_them_in_one_step() {
        // Wave Y, P-8 (Y4): the attach tools take the
        // local transform as optional arguments.
        let mut core = asset_core();
        let undo_before = core.undo_depth();
        // A no-argument attach is the historical call:
        // identity transform, and the reply carries NO
        // media member (byte-parity with before).
        let out = tool_asset_attach(
            &mut core,
            &json!({ "figure": 0, "node": 2, "asset": 0 }),
        )
        .expect("plain attach succeeds");
        assert!(
            out.as_object().map(|o| !o.contains_key("media")).unwrap_or(false),
            "plain attach reply must not grow a media member: {out}"
        );
        assert_eq!(
            core.project.figures[0].def.nodes[2].media,
            Some(SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false })
        );
        // Re-attaching the same asset WITH transform
        // args is the set path: the media is replaced
        // wholesale in one more undo step, and the
        // reply names the resulting media in the read
        // shape.
        let out = tool_asset_attach(
            &mut core,
            &json!({
                "figure": 0, "node": 2, "asset": 0,
                "offset": [4.0, -2.0], "scale": 2.0,
                "angle_offset": 30.0, "flip": true,
            }),
        )
        .expect("transform attach succeeds");
        assert_eq!(core.undo_depth(), undo_before + 2, "one step per attach");
        assert_eq!(
            core.project.figures[0].def.nodes[2].media,
            Some(SegmentMedia::Image { asset: 0, offset: [4.0, -2.0], scale: 2.0, angle_offset: 30.0, flip: true })
        );
        assert_eq!(
            out["media"],
            json!({
                "kind": "image", "asset": 0,
                "offset": [4.0, -2.0], "scale": 2.0,
                "angle_offset": 30.0, "flip": true,
            })
        );
        // A partial transform resets the unnamed
        // components to the identity (wholesale
        // replacement — there is no merge semantics to
        // guess at).
        let out = tool_asset_attach(
            &mut core,
            &json!({ "figure": 0, "node": 2, "asset": 0, "scale": 0.5 }),
        )
        .expect("partial transform attach succeeds");
        assert_eq!(
            core.project.figures[0].def.nodes[2].media,
            Some(SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 0.5, angle_offset: 0.0, flip: false })
        );
        assert_eq!(out["media"]["scale"], json!(0.5));
        assert_eq!(core.undo_depth(), undo_before + 3);
    }

    #[test]
    fn asset_attach_transform_refusals_spend_nothing() {
        // Every malformed transform value is refused
        // BEFORE the snapshot: no undo step, and the
        // node's existing media is untouched.
        let mut core = asset_core();
        let before = core.project.figures[0].def.nodes[2].media.clone();
        let undo_before = core.undo_depth();
        for (args, needle) in [
            (json!({ "scale": 0.0 }), "greater than zero"),
            (json!({ "scale": -1.0 }), "greater than zero"),
            (json!({ "offset": [1.0] }), "exactly two"),
            (json!({ "offset": "x" }), "array of two numbers"),
            (json!({ "offset": [1.0, "y"] }), "must be a number"),
            (json!({ "flip": "yes" }), "must be a boolean"),
            (json!({ "angle_offset": "x" }), "must be a number"),
        ] {
            let mut call = json!({ "figure": 0, "node": 2, "asset": 0 });
            for (k, v) in args.as_object().unwrap() {
                call[k] = v.clone();
            }
            let err = tool_asset_attach(&mut core, &call)
                .expect_err("malformed transform must be refused");
            assert!(err.contains(needle), "err for {args}: {err}");
        }
        assert_eq!(core.undo_depth(), undo_before, "refusals spend nothing");
        assert_eq!(core.project.figures[0].def.nodes[2].media, before);
    }

    #[test]
    fn clip_asset_attach_with_transform_args_reads_back() {
        // The clip-def twin takes the same args, and
        // `clip_get_node_media` reads the stored
        // transform back in the extended shape.
        let mut core = clip_media_core();
        let undo_before = core.undo_depth();
        let out = tool_clip_asset_attach(
            &mut core,
            &json!({
                "clip": 0, "node": 1, "asset": 0,
                "offset": [1.5, 0.5], "flip": true,
            }),
        )
        .expect("clip attach with transform succeeds");
        assert_eq!(core.undo_depth(), undo_before + 1, "one undo step");
        assert_eq!(
            core.project.clips[0].def.nodes[1].media,
            Some(SegmentMedia::Image { asset: 0, offset: [1.5, 0.5], scale: 1.0, angle_offset: 0.0, flip: true })
        );
        assert_eq!(
            out["media"],
            json!({
                "kind": "image", "asset": 0,
                "offset": [1.5, 0.5], "scale": 1.0,
                "angle_offset": 0.0, "flip": true,
            })
        );
        let read = tool_clip_get_node_media(&mut core, &json!({ "clip": 0, "node": 1 }))
            .expect("read succeeds");
        assert_eq!(read["media"], out["media"], "read and write shapes agree");
        assert_eq!(core.undo_depth(), undo_before + 1, "the read spent nothing");
    }

    #[test]
    fn assets_list_and_info_report_dimensions_and_references() {
        let mut core = asset_core();
        let out = tool_assets_list(&mut core, &json!({})).expect("list succeeds");
        assert_eq!(out["count"].as_u64(), Some(1));
        let a = &out["assets"][0];
        assert_eq!(a["name"].as_str(), Some("green"));
        assert_eq!(a["width"].as_u64(), Some(4));
        assert_eq!(a["height"].as_u64(), Some(2));
        assert!(a["bytes"].as_u64().unwrap_or(0) > 0);
        assert_eq!(a["references"]["segments"].as_u64(), Some(1));
        assert_eq!(a["references"]["sprites"].as_u64(), Some(0));
        let info = tool_asset_info(&mut core, &json!({ "index": 0 })).expect("info succeeds");
        assert_eq!(info["ok"].as_bool(), Some(true));
        assert_eq!(info["name"].as_str(), Some("green"));
        let err = tool_asset_info(&mut core, &json!({ "index": 3 }))
            .expect_err("missing asset must be refused");
        assert!(err.contains("out of range"), "err: {err}");
    }

    #[test]
    fn asset_rename_spends_an_undo_step_only_on_a_real_change() {
        let mut core = asset_core();
        let undo_before = core.undo_depth();
        let out = tool_asset_rename(
            &mut core,
            &json!({ "index": 0, "name": "hero" }),
        )
        .expect("rename succeeds");
        assert_eq!(out["changed"].as_bool(), Some(true));
        assert_eq!(core.project.assets[0].name, "hero");
        assert_eq!(core.undo_depth(), undo_before + 1);
        let out = tool_asset_rename(
            &mut core,
            &json!({ "index": 0, "name": "hero" }),
        )
        .expect("same-name rename is a no-op, not an error");
        assert_eq!(out["changed"].as_bool(), Some(false));
        assert_eq!(core.undo_depth(), undo_before + 1, "no-op spends no snapshot");
        let err = tool_asset_rename(
            &mut core,
            &json!({ "index": 0, "name": "   " }),
        )
        .expect_err("blank name must be refused");
        assert!(err.contains("must not be empty"), "err: {err}");
    }

    // ---- Wave 13: sprite-instance MCP handlers ----

    fn sprite_core() -> EditorCore {
        // demo_project(): one asset ("green", index 0), no
        // sprite instances.
        asset_core()
    }

    #[test]
    fn sprite_list_is_empty_on_a_fresh_project() {
        let mut core = sprite_core();
        let out = tool_sprite_list(&mut core, &json!({})).expect("list succeeds");
        assert_eq!(out["count"].as_u64(), Some(0));
        assert_eq!(out["sprites"].as_array().map(|a| a.len()), Some(0));
    }

    #[test]
    fn sprite_sample_reads_the_sampled_transform_and_refuses_keyless() {
        // Wave 14 (A8): the sample read interpolates
        // between authored keys at fractional frames and
        // refuses — quoting the sprite — when there is
        // nothing to sample.
        let mut core = sprite_core();
        // Grow the one-frame fixture so key frames 0 and
        // 4 are both in the timeline (the set_key op
        // clamps to the last frame).
        let f0 = core.project.frames[0].clone();
        while core.project.frames.len() <= 4 {
            core.project.frames.push(f0.clone());
        }
        tool_sprite_add(&mut core, &json!({ "asset": 0 })).expect("add succeeds");
        // The seed key sits at the stage centre (x 100 in
        // the fixture's stage); author a second key at
        // frame 4 with x 200.
        let seed_x = core.project.sprites[0].keys[0].x;
        tool_sprite_set_key(
            &mut core,
            &json!({ "sprite": 0, "frame": 4, "x": seed_x + 100.0 }),
        )
        .expect("set_key succeeds");
        let undo_before = core.undo_depth();
        let out = tool_sprite_sample(
            &mut core,
            &json!({ "sprite": 0, "frame": 2.0 }),
        )
        .expect("sample succeeds");
        assert_eq!(
            out["key"]["x"].as_f64(),
            Some((seed_x + 50.0) as f64),
            "frame 2 samples halfway between the keys"
        );
        assert_eq!(core.undo_depth(), undo_before, "a read spends no undo step");
        // A keyless sprite (delete the only keys' owner
        // state by removing all keys via the op) refuses.
        let _ = core.delete_sprite_key(0, 0);
        let _ = core.delete_sprite_key(0, 4);
        let err = tool_sprite_sample(&mut core, &json!({ "sprite": 0, "frame": 0 }))
            .expect_err("a keyless sprite must be refused");
        assert!(err.contains("sprite 0 has no keys"), "err: {err}");
        // Out-of-range sprite and negative frame refuse too.
        let err = tool_sprite_sample(&mut core, &json!({ "sprite": 9, "frame": 0 }))
            .expect_err("a missing sprite must be refused");
        assert!(err.contains("sprite index 9 out of range"), "err: {err}");
        let err = tool_sprite_sample(&mut core, &json!({ "sprite": 0, "frame": -1 }))
            .expect_err("a negative frame must be refused");
        assert!(err.contains("must not be negative"), "err: {err}");
    }

    #[test]
    fn sprite_add_places_one_instance_in_one_undo_step() {
        let mut core = sprite_core();
        let undo_before = core.undo_depth();
        let out = tool_sprite_add(&mut core, &json!({ "asset": 0 })).expect("add succeeds");
        assert_eq!(out["index"].as_u64(), Some(0));
        assert_eq!(out["sprite"]["asset"].as_u64(), Some(0));
        assert_eq!(out["sprite"]["asset_name"], json!("green"));
        assert_eq!(out["sprite"]["key_count"].as_u64(), Some(1));
        assert_eq!(out["sprite"]["keys"][0]["frame"].as_f64(), Some(0.0));
        assert_eq!(core.project.sprites.len(), 1);
        assert_eq!(core.project.sprites[0].asset, 0);
        assert_eq!(core.project.sprites[0].keys.len(), 1);
        assert_eq!(
            core.undo_depth(),
            undo_before + 1,
            "the op spends exactly one undo step"
        );
        // And the list now reports the same instance.
        let list = tool_sprite_list(&mut core, &json!({})).expect("list succeeds");
        assert_eq!(list["count"].as_u64(), Some(1));
        assert_eq!(list["sprites"][0]["asset_name"], json!("green"));
    }

    #[test]
    fn sprite_add_refuses_a_bad_asset_and_spends_nothing() {
        let mut core = sprite_core();
        let undo_before = core.undo_depth();
        let err = tool_sprite_add(&mut core, &json!({ "asset": 99 }))
            .expect_err("a missing asset must be refused");
        assert!(err.contains("out of range"), "err: {err}");
        assert!(core.project.sprites.is_empty());
        assert_eq!(core.undo_depth(), undo_before, "a refusal spends no undo step");
    }

    #[test]
    fn sprite_set_key_inserts_then_replaces_and_fills_from_the_sample() {
        let mut core = sprite_core();
        // Wave 13 (integrator): the fixture project has one
        // frame and A1's op clamps keys to the timeline
        // (its pinned contract), so grow the timeline past
        // frame 10 for the insert/replace below.
        let f0 = core.project.frames[0].clone();
        while core.project.frames.len() <= 10 {
            core.project.frames.push(f0.clone());
        }
        tool_sprite_add(&mut core, &json!({ "asset": 0 })).expect("add succeeds");
        let key0 = core.project.sprites[0].keys[0];
        let undo_before = core.undo_depth();
        // Insert at frame 10 with only x given: the other
        // fields come from the value sampled at frame 10,
        // which is key0's (the only key clamps).
        let out = tool_sprite_set_key(
            &mut core,
            &json!({ "sprite": 0, "frame": 10, "x": 42.5 }),
        )
        .expect("set_key succeeds");
        assert_eq!(out["created"].as_bool(), Some(true));
        assert_eq!(out["key"]["x"].as_f64(), Some(42.5));
        assert_eq!(out["key"]["y"].as_f64(), Some(key0.y as f64));
        assert_eq!(out["key"]["scale"].as_f64(), Some(key0.scale as f64));
        assert_eq!(out["key"]["angle"].as_f64(), Some(key0.angle as f64));
        assert_eq!(core.project.sprites[0].keys.len(), 2);
        assert_eq!(core.undo_depth(), undo_before + 1);
        // Replace the same frame: created flips false, the
        // stored values change, no new key appears.
        let out = tool_sprite_set_key(
            &mut core,
            &json!({ "sprite": 0, "frame": 10, "scale": 2.0, "rotation": 30.0 }),
        )
        .expect("replace succeeds");
        assert_eq!(out["created"].as_bool(), Some(false));
        assert_eq!(out["key"]["scale"].as_f64(), Some(2.0));
        assert_eq!(out["key"]["angle"].as_f64(), Some(30.0));
        assert_eq!(core.project.sprites[0].keys.len(), 2);
        assert_eq!(core.undo_depth(), undo_before + 2);
    }

    #[test]
    fn sprite_set_key_refuses_bad_values_and_spends_nothing() {
        let mut core = sprite_core();
        tool_sprite_add(&mut core, &json!({ "asset": 0 })).expect("add succeeds");
        let undo_before = core.undo_depth();
        for (args, needle) in [
            (json!({ "sprite": 9, "frame": 1 }), "out of range"),
            (json!({ "sprite": 0, "frame": -1 }), "must not be negative"),
            (json!({ "sprite": 0, "frame": 1, "scale": 0 }), "greater than 0"),
            (json!({ "sprite": 0, "frame": 1, "scale": -2 }), "greater than 0"),
            // 1e300 is a finite f64 that overflows f32.
            (json!({ "sprite": 0, "frame": 1e300 }), "must be a finite number"),
            (json!({ "sprite": 0, "frame": 1, "x": "wide" }), "must be a number"),
        ] {
            let err = tool_sprite_set_key(&mut core, &args)
                .expect_err("bad set_key args must be refused");
            assert!(err.contains(needle), "err: {err} (want '{needle}')");
        }
        assert_eq!(core.project.sprites[0].keys.len(), 1, "no key was written");
        assert_eq!(core.undo_depth(), undo_before, "refusals spend no undo step");
    }

    #[test]
    fn sprite_set_key_writes_enrichment_and_reports_it() {
        // Wave X (P-6): the optional enrichment args land
        // on the stored key in the same single undo step,
        // and the reply + list read them back verbatim.
        let mut core = sprite_core();
        tool_sprite_add(&mut core, &json!({ "asset": 0 })).expect("add succeeds");
        let undo_before = core.undo_depth();
        let out = tool_sprite_set_key(
            &mut core,
            &json!({
                "sprite": 0, "frame": 0,
                "opacity": 0.5, "flip_x": true, "scale_y": 3.0,
                "pivot_x": 0.25, "pivot_y": 0.75,
            }),
        )
        .expect("enriched set_key succeeds");
        assert_eq!(core.undo_depth(), undo_before + 1, "still one undo step");
        assert_eq!(out["key"]["opacity"].as_f64(), Some(0.5));
        assert_eq!(out["key"]["flip_x"].as_bool(), Some(true));
        assert_eq!(out["key"]["flip_y"].as_bool(), Some(false));
        assert_eq!(out["key"]["scale_y"].as_f64(), Some(3.0));
        assert_eq!(out["key"]["pivot_x"].as_f64(), Some(0.25));
        assert_eq!(out["key"]["pivot_y"].as_f64(), Some(0.75));
        let stored = core.project.sprites[0].keys[0];
        assert_eq!(stored.opacity, 0.5);
        assert!(stored.flip_x && !stored.flip_y);
        assert_eq!(stored.scale_y, 3.0);
        assert_eq!((stored.pivot_x, stored.pivot_y), (0.25, 0.75));
        let list = tool_sprite_list(&mut core, &json!({})).expect("list succeeds");
        assert_eq!(list["sprites"][0]["keys"][0]["opacity"].as_f64(), Some(0.5));
        assert_eq!(list["sprites"][0]["keys"][0]["pivot_y"].as_f64(), Some(0.75));
        // The sample read reports the enrichment too (the
        // only key clamps, so frame 0 samples verbatim).
        let sample = tool_sprite_sample(&mut core, &json!({ "sprite": 0, "frame": 0 }))
            .expect("sample succeeds");
        assert_eq!(sample["key"]["flip_x"].as_bool(), Some(true));
        assert_eq!(sample["key"]["scale_y"].as_f64(), Some(3.0));
    }

    #[test]
    fn sprite_set_key_refuses_bad_enrichment_and_spends_nothing() {
        let mut core = sprite_core();
        tool_sprite_add(&mut core, &json!({ "asset": 0 })).expect("add succeeds");
        let undo_before = core.undo_depth();
        for (args, needle) in [
            (json!({ "sprite": 0, "frame": 0, "opacity": 1.5 }), "between 0 and 1"),
            (json!({ "sprite": 0, "frame": 0, "opacity": -0.1 }), "between 0 and 1"),
            (json!({ "sprite": 0, "frame": 0, "scale_y": 0 }), "greater than 0"),
            (json!({ "sprite": 0, "frame": 0, "scale_y": -1 }), "greater than 0"),
            (json!({ "sprite": 0, "frame": 0, "flip_x": "yes" }), "must be a boolean"),
            (json!({ "sprite": 0, "frame": 0, "flip_y": 1 }), "must be a boolean"),
            (json!({ "sprite": 0, "frame": 0, "pivot_x": "wide" }), "must be a number"),
        ] {
            let err = tool_sprite_set_key(&mut core, &args)
                .expect_err("bad enrichment args must be refused");
            assert!(err.contains(needle), "err: {err} (want '{needle}')");
        }
        let stored = core.project.sprites[0].keys[0];
        assert_eq!(stored.opacity, 1.0, "no refusal wrote anything");
        assert_eq!(stored.scale_y, 0.0);
        assert_eq!(core.undo_depth(), undo_before, "refusals spend no undo step");
    }

    #[test]
    fn sprite_pass_applies_pivot_flips_scale_y_and_opacity() {
        // Wave X (P-6): the sprite pass anchors the quad
        // on the key's pivot, mirrors around it under the
        // flips, stretches the second axis by the explicit
        // scale_y and reports the key opacity as alpha.
        // Asset 0 is 4x2 px; key: centre (300, 200), scale
        // 10 (width 40), scale_y 5 (height 10), pivot
        // (0.25, 0.75), flip_x only, opacity 0.5.
        let mut project = demo_project();
        project.sprites.push(SpriteInstance {
            asset: 0,
            keys: vec![InstanceKey {
                frame: 0.0,
                x: 300.0,
                y: 200.0,
                scale: 10.0,
                angle: 0.0,
                opacity: 0.5,
                flip_x: true,
                flip_y: false,
                scale_y: 5.0,
                pivot_x: 0.25,
                pivot_y: 0.75,
            }],
        });
        let items = media_items(&project, 0.0);
        let (corners, alpha) = items
            .iter()
            .rev()
            .find_map(|i| match i {
                MediaItem::Image {
                    corners, alpha, ..
                } => Some((*corners, *alpha)),
                _ => None,
            })
            .expect("sprite instance item");
        assert_eq!(alpha, 0.5, "key opacity is the item alpha");
        // Mirrored around the pivot: corner (0,0) sits a
        // quarter-width RIGHT of centre (the flip negates
        // the -0.25 offset), corner (1,0) three-quarters
        // LEFT; vertically the pivot at 0.75 puts the top
        // edge 7.5 above centre, the bottom 2.5 below.
        assert!(close(corners[0], [310.0, 192.5]), "c0 {corners:?}");
        assert!(close(corners[1], [270.0, 192.5]), "c1 {corners:?}");
        assert!(close(corners[2], [270.0, 202.5]), "c2 {corners:?}");
        assert!(close(corners[3], [310.0, 202.5]), "c3 {corners:?}");
    }

    #[test]
    fn sprite_delete_key_removes_it_and_refuses_a_missing_frame() {
        let mut core = sprite_core();
        // Wave 13 (integrator): as the set_key test above —
        // grow the one-frame fixture past frame 5.
        let f0 = core.project.frames[0].clone();
        while core.project.frames.len() <= 5 {
            core.project.frames.push(f0.clone());
        }
        tool_sprite_add(&mut core, &json!({ "asset": 0 })).expect("add succeeds");
        tool_sprite_set_key(&mut core, &json!({ "sprite": 0, "frame": 5, "x": 10.0 }))
            .expect("set_key succeeds");
        assert_eq!(core.project.sprites[0].keys.len(), 2);
        let undo_before = core.undo_depth();
        let out = tool_sprite_delete_key(&mut core, &json!({ "sprite": 0, "frame": 5 }))
            .expect("delete succeeds");
        assert_eq!(out["key_count"].as_u64(), Some(1));
        assert_eq!(core.project.sprites[0].keys.len(), 1);
        assert_eq!(core.undo_depth(), undo_before + 1);
        // The same frame again: refused up front, no step spent.
        let err = tool_sprite_delete_key(&mut core, &json!({ "sprite": 0, "frame": 5 }))
            .expect_err("a missing key must be refused");
        assert!(err.contains("has no key at frame 5"), "err: {err}");
        assert_eq!(core.undo_depth(), undo_before + 1);
    }

    #[test]
    fn sprite_delete_key_past_the_end_refuses_and_keeps_the_last_key() {
        // Wave P (P38): the clamped-delete hazard at the
        // MCP layer — naming a frame at/past the frame
        // count used to delete the LAST frame's key and
        // report the requested frame. It now refuses in
        // the handler family's frame-range wording, and
        // the last frame's key survives.
        let mut core = sprite_core();
        let f0 = core.project.frames[0].clone();
        while core.project.frames.len() <= 5 {
            core.project.frames.push(f0.clone());
        }
        tool_sprite_add(&mut core, &json!({ "asset": 0 })).expect("add succeeds");
        tool_sprite_set_key(&mut core, &json!({ "sprite": 0, "frame": 5, "x": 10.0 }))
            .expect("a key at the last frame");
        assert_eq!(core.project.sprites[0].keys.len(), 2);
        let undo_before = core.undo_depth();
        // The first frame past the end, and one far past it.
        for frame in [6, 999] {
            let err = tool_sprite_delete_key(&mut core, &json!({ "sprite": 0, "frame": frame }))
                .expect_err("a past-end frame must be refused");
            assert_eq!(
                err,
                format!("frame index {frame} out of range (project has 6 frame(s))"),
                "err: {err}"
            );
        }
        assert_eq!(core.project.sprites[0].keys.len(), 2, "both keys survive");
        assert_eq!(core.undo_depth(), undo_before, "refusals spend no undo step");
        // The last frame itself is in range and deletes.
        let out = tool_sprite_delete_key(&mut core, &json!({ "sprite": 0, "frame": 5 }))
            .expect("the last frame deletes");
        assert_eq!(out["key_count"].as_u64(), Some(1));
    }

    #[test]
    fn sprite_set_key_reply_reports_the_frame_actually_written() {
        // Wave P (P38): set_key's clamp stays ONLY because
        // the reply reports the effect — the stored key in
        // the reply carries the clamped frame, and the
        // status names it too (1-based). Pin both halves:
        // a request for frame 999 in a 6-frame project
        // writes frame 5 and says so.
        let mut core = sprite_core();
        let f0 = core.project.frames[0].clone();
        while core.project.frames.len() <= 5 {
            core.project.frames.push(f0.clone());
        }
        tool_sprite_add(&mut core, &json!({ "asset": 0 })).expect("add succeeds");
        let out = tool_sprite_set_key(
            &mut core,
            &json!({ "sprite": 0, "frame": 999, "x": 42.0 }),
        )
        .expect("set_key succeeds at the clamped frame");
        assert_eq!(out["created"], json!(true));
        assert_eq!(
            out["key"]["frame"].as_f64(),
            Some(5.0),
            "the reply's key names the frame actually written"
        );
        assert_eq!(out["status"], json!("Sprite 1 key set at frame 6."));
        assert!(
            core.project.sprites[0].keys.iter().any(|k| k.frame == 5.0),
            "the key is stored at the last frame"
        );
    }

    #[test]
    fn sprite_remove_drops_the_instance_and_undo_restores_it() {
        let mut core = sprite_core();
        tool_sprite_add(&mut core, &json!({ "asset": 0 })).expect("first add");
        tool_sprite_add(&mut core, &json!({ "asset": 0 })).expect("second add");
        assert_eq!(core.project.sprites.len(), 2);
        let undo_before = core.undo_depth();
        let out = tool_sprite_remove(&mut core, &json!({ "sprite": 0 })).expect("remove");
        assert_eq!(out["asset"].as_u64(), Some(0));
        assert_eq!(out["count"].as_u64(), Some(1));
        assert_eq!(core.project.sprites.len(), 1);
        assert_eq!(core.undo_depth(), undo_before + 1);
        let err = tool_sprite_remove(&mut core, &json!({ "sprite": 7 }))
            .expect_err("a missing instance must be refused");
        assert!(err.contains("out of range"), "err: {err}");
        assert_eq!(core.undo_depth(), undo_before + 1, "the refusal spent nothing");
        core.undo();
        assert_eq!(core.project.sprites.len(), 2, "one undo restores the instance");
    }

    /// Wave 14, C1 — clip-def media fixtures
    /// (docs/wave13/CLIP_MEDIA.md §6.6). The demo skeleton
    /// (root at (100, 100), node 1 running +x for 100, node 2
    /// running down for 80) as a movieclip instance of clip
    /// 0: the figure's own def and the clip's def are built
    /// from the same skeleton with independent media, and
    /// the clip's single frame pose matches the demo pose,
    /// so geometry expectations are the demo ones.
    fn clip_project(
        inst_media: (Option<SegmentMedia>, Option<SegmentMedia>),
        clip_media: (Option<SegmentMedia>, Option<SegmentMedia>),
    ) -> Project {
        let skeleton = |m: (Option<SegmentMedia>, Option<SegmentMedia>)| FigureDef {
            nodes: vec![
                node("root", None, 0.0, None),
                node("arm", Some(0), 100.0, m.0),
                node("leg", Some(0), 80.0, m.1),
            ],
        };
        let pose = Pose {
            root: [100.0, 100.0],
            angles: vec![0.0, 90.0, 0.0],
        };
        let mut project = demo_project();
        project.figures[0].def = skeleton(inst_media);
        project.figures[0].clip = Some(0);
        project.clips = vec![ClipDef {
            name: "clip".into(),
            def: skeleton(clip_media),
            frames: vec![pose],
            loop_mode: ClipLoop::Loop,
        }];
        project
    }

    #[test]
    fn clip_def_image_media_renders_for_media_free_instance() {
        // §6.6.1: the media exists only in the clip def; the
        // instance's own def is media-free. One image item at
        // the instance's segment quad, alpha = instance
        // figure opacity × CLIP node's style opacity (§6.2).
        let mut project = clip_project(
            (None, None),
            (Some(SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false }), None),
        );
        project.figures[0].opacity = 0.5;
        project.clips[0].def.nodes[1].style.opacity = 0.5;
        let items = media_items(&project, 0.0);
        assert_eq!(items.len(), 1, "exactly the clip-sourced image");
        match &items[0] {
            MediaItem::Image {
                corners,
                asset,
                alpha,
                src_rect,
            } => {
                assert_eq!(*asset, 0);
                assert_eq!(*alpha, 0.25, "0.5 figure × 0.5 clip-node style");
                assert!(close(corners[0], [100.0, 75.0]), "c0 {:?}", corners[0]);
                assert!(close(corners[2], [200.0, 125.0]), "c2 {:?}", corners[2]);
                // Unbent: one item sampling the whole source
                // (Wave X, X4).
                assert_eq!(*src_rect, [0.0, 0.0, 1.0, 1.0]);
            }
            other => panic!("expected an image item, got {other:?}"),
        }
        // §6.3: a hidden figure stays skipped wholesale,
        // clip-sourced media included.
        project.figures[0].hidden = true;
        assert!(media_items(&project, 0.0).is_empty());
    }

    #[test]
    fn clip_and_instance_media_never_double_draw() {
        // §6.6.2: the normal clone case — identical media on
        // both defs yields exactly ONE item for the node, and
        // its style comes from the instance node (the
        // carrier): instance style 0.5 beats clip style 0.25
        // in the alpha product even though the payloads are
        // identical.
        let mut project = clip_project(
            (Some(SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false }), None),
            (Some(SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false }), None),
        );
        project.figures[0].def.nodes[1].style.opacity = 0.5;
        project.clips[0].def.nodes[1].style.opacity = 0.25;
        let items = media_items(&project, 0.0);
        assert_eq!(items.len(), 1, "one item per node, never two");
        match &items[0] {
            MediaItem::Image { alpha, .. } => {
                assert_eq!(*alpha, 0.5, "the instance node's style carries")
            }
            other => panic!("expected an image item, got {other:?}"),
        }
    }

    #[test]
    fn instance_media_wins_over_clip_media() {
        // §6.6.3: differing media at the same index on both
        // defs — the instance's item, unchanged, at both an
        // image node and a text node.
        let mut project = clip_project(
            (
                Some(SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false }),
                Some(SegmentMedia::Text {
                    content: "MINE".into(),
                    size: 25.0,
                }),
            ),
            (
                Some(SegmentMedia::Image { asset: 1, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false }),
                Some(SegmentMedia::Text {
                    content: "CLIP".into(),
                    size: 25.0,
                }),
            ),
        );
        project.assets.push(ImageAsset {
            name: "red".into(),
            png: png_bytes(2, 2, Rgba([255, 0, 0, 255])),
        });
        let items = media_items(&project, 0.0);
        assert_eq!(items.len(), 2);
        let images = image_items(&items);
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].1, 0, "the instance's asset, not the clip's");
        let text = items.iter().find_map(|i| match i {
            MediaItem::Text { content, .. } => Some(content),
            _ => None,
        });
        assert_eq!(text.map(String::as_str), Some("MINE"));
    }

    #[test]
    fn clip_def_text_media_uses_clip_payload_and_colour_fallback() {
        // §6.6.4: clip-def text yields a Text item with the
        // clip node's content/size; colour is the clip node's
        // colour, falling back to the figure colour (§6.2).
        let mut project = clip_project(
            (None, None),
            (
                None,
                Some(SegmentMedia::Text {
                    content: "CLIP".into(),
                    size: 25.0,
                }),
            ),
        );
        project.clips[0].def.nodes[2].color = Some(0x00FF00);
        let text_of = |p: &Project| {
            media_items(p, 0.0)
                .into_iter()
                .find_map(|i| match i {
                    MediaItem::Text {
                        pos,
                        angle_deg,
                        content,
                        size,
                        color,
                        ..
                    } => Some((pos, angle_deg, content, size, color)),
                    _ => None,
                })
                .expect("a text item")
        };
        let (pos, angle, content, size, color) = text_of(&project);
        assert!(close(pos, [100.0, 140.0]), "pos {pos:?}");
        assert!((angle - 90.0).abs() < 0.01, "angle {angle}");
        assert_eq!(content, "CLIP");
        assert_eq!(size, 25.0);
        assert_eq!(color, 0x00FF00, "the clip node's own colour wins");
        project.clips[0].def.nodes[2].color = None;
        let (_, _, _, _, color) = text_of(&project);
        assert_eq!(color, 0xFF0000, "else the figure colour, as figures do");
    }

    #[test]
    fn clip_def_longer_and_shorter_than_instance_def_is_safe() {
        // §6.6.5 / §6.3: divergent def lengths skip, never
        // panic.
        // Longer clip def: a media-carrying node beyond the
        // instance's node count has no segment to attach to
        // and is skipped; the in-range fallback still lands.
        let mut project = clip_project(
            (None, None),
            (Some(SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false }), None),
        );
        project.clips[0].def.nodes.push(node(
            "extra",
            Some(0),
            50.0,
            Some(SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false }),
        ));
        let items = media_items(&project, 0.0);
        assert_eq!(items.len(), 1, "the extra clip node contributes nothing");
        // Shorter clip def: trailing instance nodes simply
        // have no fallback.
        let mut project = clip_project(
            (None, None),
            (Some(SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false }), None),
        );
        project.clips[0].def.nodes.truncate(2);
        let items = media_items(&project, 0.0);
        assert_eq!(items.len(), 1, "node 1's fallback survives truncation");
        assert!(matches!(items[0], MediaItem::Image { .. }));
    }

    #[test]
    fn missing_clip_and_media_free_clip_def_change_nothing() {
        // §6.3: `clip` naming a missing clip has no fallback —
        // the instance's own media renders exactly as before
        // this fix (the demo project's two items).
        let mut project = demo_project();
        project.figures[0].clip = Some(0);
        let items = media_items(&project, 0.0);
        assert_eq!(items.len(), 2, "own image + own text, no fallback");
        // A clip that exists but carries no media anywhere
        // likewise leaves the enumeration untouched.
        let project = clip_project(
            (
                Some(SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false }),
                Some(SegmentMedia::Text {
                    content: "HI".into(),
                    size: 25.0,
                }),
            ),
            (None, None),
        );
        let items = media_items(&project, 0.0);
        assert_eq!(items.len(), 2, "the instance's two items only");
    }

    #[test]
    fn clip_sourced_media_keeps_the_segment_slot_in_order() {
        // §6.2: order is unchanged — a clip-sourced segment
        // item takes its figure's existing slot, ahead of the
        // free-standing sprite and text instances.
        let mut project = clip_project(
            (None, None),
            (Some(SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false }), None),
        );
        project.sprites.push(SpriteInstance {
            asset: 0,
            keys: vec![InstanceKey {
                frame: 0.0,
                x: 400.0,
                y: 300.0,
                scale: 1.0,
                angle: 0.0,
                opacity: 1.0,
                flip_x: false,
                flip_y: false,
                scale_y: 0.0,
                pivot_x: 0.5,
                pivot_y: 0.5,
            }],
        });
        project.texts.push(TextInstance {
            content: "HEY".into(),
            size: 20.0,
            color: 0xFFFFFF,
            keys: vec![InstanceKey {
                frame: 0.0,
                x: 100.0,
                y: 500.0,
                scale: 1.0,
                angle: 0.0,
                opacity: 1.0,
                flip_x: false,
                flip_y: false,
                scale_y: 0.0,
                pivot_x: 0.5,
                pivot_y: 0.5,
            }],
        });
        let items = media_items(&project, 0.0);
        assert_eq!(items.len(), 3);
        match &items[0] {
            MediaItem::Image { corners, .. } => assert!(
                close(corners[0], [100.0, 75.0]),
                "first item is the clip-sourced segment quad: {corners:?}"
            ),
            other => panic!("expected the segment image first, got {other:?}"),
        }
        assert!(matches!(items[1], MediaItem::Image { .. }), "sprite second");
        assert!(matches!(items[2], MediaItem::Text { .. }), "text last");
    }

    #[test]
    fn media_items_order_is_segment_then_sprite_then_text_instances() {
        // The ordering audit's pin (see this module's docs):
        // segment media first (figure order, node order),
        // then sprite instances, then text instances — the
        // exact sequence composite_media draws in.
        let mut project = demo_project();
        project.sprites.push(SpriteInstance {
            asset: 0,
            keys: vec![InstanceKey {
                frame: 0.0,
                x: 400.0,
                y: 300.0,
                scale: 1.0,
                angle: 0.0,
                opacity: 1.0,
                flip_x: false,
                flip_y: false,
                scale_y: 0.0,
                pivot_x: 0.5,
                pivot_y: 0.5,
            }],
        });
        project.texts.push(TextInstance {
            content: "HEY".into(),
            size: 20.0,
            color: 0xFFFFFF,
            keys: vec![InstanceKey {
                frame: 0.0,
                x: 100.0,
                y: 500.0,
                scale: 1.0,
                angle: 0.0,
                opacity: 1.0,
                flip_x: false,
                flip_y: false,
                scale_y: 0.0,
                pivot_x: 0.5,
                pivot_y: 0.5,
            }],
        });
        let items = media_items(&project, 0.0);
        let kinds: Vec<&str> = items
            .iter()
            .map(|i| match i {
                MediaItem::Image { .. } => "image",
                MediaItem::Text { .. } => "text",
            })
            .collect();
        // demo_project contributes a segment image and a
        // segment text ("HI"); the instance image and the
        // instance text follow in that order.
        assert_eq!(kinds, ["image", "text", "image", "text"]);
        let texts: Vec<&str> = items
            .iter()
            .filter_map(|i| match i {
                MediaItem::Text { content, .. } => Some(content.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(texts, ["HI", "HEY"], "segment text precedes instance text");
    }

    // ---- Wave 14: clip-def media MCP handlers (C5) ----

    fn clip_media_core() -> EditorCore {
        // asset_core()'s project (one asset, one figure)
        // plus one movieclip definition whose def mirrors
        // the demo skeleton — root, arm (node 1), leg
        // (node 2) — all media-free.
        let mut core = asset_core();
        core.project.clips.push(ClipDef {
            name: "clip".into(),
            def: FigureDef {
                nodes: vec![
                    node("root", None, 0.0, None),
                    node("arm", Some(0), 100.0, None),
                    node("leg", Some(0), 80.0, None),
                ],
            },
            frames: Vec::new(),
            loop_mode: ClipLoop::Loop,
        });
        core
    }

    #[test]
    fn clip_asset_attach_writes_the_clip_def_in_one_undo_step() {
        let mut core = clip_media_core();
        let undo_before = core.undo_depth();
        let out = tool_clip_asset_attach(
            &mut core,
            &json!({ "clip": 0, "node": 1, "asset": 0 }),
        )
        .expect("attach succeeds");
        assert_eq!(out["clip"].as_u64(), Some(0));
        assert_eq!(out["node"].as_u64(), Some(1));
        assert_eq!(out["asset"].as_u64(), Some(0));
        assert_eq!(
            core.project.clips[0].def.nodes[1].media,
            Some(SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false })
        );
        assert_eq!(core.undo_depth(), undo_before + 1, "exactly one undo step");
        // Attaching again replaces (the asset_attach
        // overwrite rule): a second asset takes the node.
        core.project.assets.push(ImageAsset {
            name: "spare".into(),
            png: png_bytes(2, 2, Rgba([1, 2, 3, 255])),
        });
        let out = tool_clip_asset_attach(
            &mut core,
            &json!({ "clip": 0, "node": 1, "asset": 1 }),
        )
        .expect("re-attach succeeds");
        assert_eq!(out["asset"].as_u64(), Some(1));
        assert_eq!(
            core.project.clips[0].def.nodes[1].media,
            Some(SegmentMedia::Image { asset: 1, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false })
        );
        assert_eq!(core.undo_depth(), undo_before + 2);
    }

    #[test]
    fn clip_asset_attach_refuses_bad_indices_without_a_snapshot() {
        let mut core = clip_media_core();
        let undo_before = core.undo_depth();
        let err = tool_clip_asset_attach(
            &mut core,
            &json!({ "clip": 4, "node": 1, "asset": 0 }),
        )
        .expect_err("missing clip must be refused");
        assert!(err.contains("clip index 4 out of range"), "err: {err}");
        let err = tool_clip_asset_attach(
            &mut core,
            &json!({ "clip": 0, "node": 9, "asset": 0 }),
        )
        .expect_err("missing node must be refused");
        assert!(
            err.contains("node index 9 out of range (clip 0 has 3 node(s))"),
            "err: {err}"
        );
        let err = tool_clip_asset_attach(
            &mut core,
            &json!({ "clip": 0, "node": 1, "asset": 7 }),
        )
        .expect_err("missing asset must be refused");
        assert!(err.contains("asset index 7 out of range"), "err: {err}");
        // No refusal wrote anything or spent a step.
        assert!(
            core.project.clips[0]
                .def
                .nodes
                .iter()
                .all(|n| n.media.is_none())
        );
        assert_eq!(core.undo_depth(), undo_before);
    }

    #[test]
    fn clip_get_node_media_reads_image_text_and_none_in_get_node_shape() {
        let mut core = clip_media_core();
        core.project.clips[0].def.nodes[2].media = Some(SegmentMedia::Text {
            content: "HEY".into(),
            size: 24.0,
        });
        let undo_before = core.undo_depth();
        let out = tool_clip_get_node_media(&mut core, &json!({ "clip": 0, "node": 1 }))
            .expect("read succeeds");
        assert!(out["media"].is_null(), "a media-free node reads null");
        tool_clip_asset_attach(&mut core, &json!({ "clip": 0, "node": 1, "asset": 0 }))
            .expect("attach succeeds");
        let out = tool_clip_get_node_media(&mut core, &json!({ "clip": 0, "node": 1 }))
            .expect("read succeeds");
        assert_eq!(out["media"], json!({ "kind": "image", "asset": 0, "offset": [0.0, 0.0], "scale": 1.0, "angle_offset": 0.0, "flip": false }));
        let out = tool_clip_get_node_media(&mut core, &json!({ "clip": 0, "node": 2 }))
            .expect("read succeeds");
        assert_eq!(
            out["media"],
            json!({ "kind": "text", "content": "HEY", "size": 24.0 })
        );
        // The reads spent nothing: only the attach moved
        // the undo depth.
        assert_eq!(core.undo_depth(), undo_before + 1);
        let err = tool_clip_get_node_media(&mut core, &json!({ "clip": 3, "node": 0 }))
            .expect_err("missing clip must be refused");
        assert!(err.contains("clip index 3 out of range"), "err: {err}");
    }

    #[test]
    fn clip_clear_node_media_clears_either_kind_and_reports_an_empty_node() {
        let mut core = clip_media_core();
        core.project.clips[0].def.nodes[1].media =
            Some(SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false });
        core.project.clips[0].def.nodes[2].media = Some(SegmentMedia::Text {
            content: "HEY".into(),
            size: 24.0,
        });
        let undo_before = core.undo_depth();
        let out = tool_clip_clear_node_media(&mut core, &json!({ "clip": 0, "node": 1 }))
            .expect("clear succeeds");
        assert_eq!(out["cleared"].as_bool(), Some(true));
        assert_eq!(out["media"], json!({ "kind": "image", "asset": 0, "offset": [0.0, 0.0], "scale": 1.0, "angle_offset": 0.0, "flip": false }));
        assert_eq!(core.project.clips[0].def.nodes[1].media, None);
        assert_eq!(core.undo_depth(), undo_before + 1, "one undo step");
        // Text clears the same way.
        let out = tool_clip_clear_node_media(&mut core, &json!({ "clip": 0, "node": 2 }))
            .expect("clear succeeds");
        assert_eq!(out["cleared"].as_bool(), Some(true));
        assert_eq!(
            out["media"],
            json!({ "kind": "text", "content": "HEY", "size": 24.0 })
        );
        assert_eq!(core.project.clips[0].def.nodes[2].media, None);
        assert_eq!(core.undo_depth(), undo_before + 2);
        // Clearing an empty node is reported, not an error,
        // and spends no phantom step.
        let out = tool_clip_clear_node_media(&mut core, &json!({ "clip": 0, "node": 1 }))
            .expect("an empty clear is a report, not an error");
        assert_eq!(out["cleared"].as_bool(), Some(false));
        assert!(out["media"].is_null());
        assert_eq!(
            core.undo_depth(),
            undo_before + 2,
            "the empty clear spent nothing"
        );
        let err = tool_clip_clear_node_media(&mut core, &json!({ "clip": 0, "node": 8 }))
            .expect_err("missing node must be refused");
        assert!(err.contains("node index 8 out of range"), "err: {err}");
    }

    #[test]
    fn clip_attached_media_counts_in_the_census_until_cleared() {
        // The C6 flag, through the public surface: a
        // reference created by clip_asset_attach lands in
        // the census's clip-def bucket — the asset's
        // removal is refused while the reference stands,
        // and the bucket empties when
        // clip_clear_node_media detaches it.
        let mut core = clip_media_core();
        tool_clip_asset_attach(&mut core, &json!({ "clip": 0, "node": 1, "asset": 0 }))
            .expect("attach succeeds");
        let info = tool_asset_info(&mut core, &json!({ "index": 0 }))
            .expect("info succeeds");
        assert_eq!(info["references"]["clip_segments"].as_u64(), Some(1));
        let err = tool_asset_remove(&mut core, &json!({ "index": 0 }))
            .expect_err("a clip-referenced asset must be refused");
        assert!(err.contains("1 clip segment(s)"), "err: {err}");
        tool_clip_clear_node_media(&mut core, &json!({ "clip": 0, "node": 1 }))
            .expect("clear succeeds");
        let info = tool_asset_info(&mut core, &json!({ "index": 0 }))
            .expect("info succeeds");
        assert_eq!(info["references"]["clip_segments"].as_u64(), Some(0));
        // The demo figure's own segment still references
        // asset 0; detach it directly and removal passes.
        core.project.figures[0].def.nodes[1].media = None;
        tool_asset_remove(&mut core, &json!({ "index": 0 }))
            .expect("removal succeeds once nothing references the asset");
        assert_eq!(core.project.assets.len(), 0);
    }

    #[test]
    fn clip_attached_media_is_what_the_renderer_samples() {
        // The attach path writes the same per-node model
        // the funnel samples: a media-free clip instance
        // picks the attached image up through the clip
        // fallback (C1).
        let mut core = EditorCore::new();
        core.project = clip_project((None, None), (None, None));
        tool_clip_asset_attach(&mut core, &json!({ "clip": 0, "node": 1, "asset": 0 }))
            .expect("attach succeeds");
        let items = media_items(&core.project, 0.0);
        assert_eq!(items.len(), 1, "exactly the clip-attached image");
        assert!(
            matches!(&items[0], MediaItem::Image { asset: 0, .. }),
            "got {:?}",
            items[0]
        );
    }
}
