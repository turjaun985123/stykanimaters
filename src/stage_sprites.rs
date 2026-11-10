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
//! order. Since Wave X (X5) the pointer path exists:
//! `main.rs::stage_drag` calls [`sprite_pick_at`] on a
//! joint miss and writes the hit into the Sprites panel's
//! selection.
//!
//! ## Dragging (Wave Z, Z3)
//!
//! The same press that picks a sprite also arms a drag:
//! [`sprite_drag_down`] records the grab, [`sprite_drag_move`]
//! follows the pointer, [`sprite_drag_up`] settles. The
//! gesture's shape is the house one (the graph canvas's
//! drag, the subtitle lane's chip drag): the pointer moves
//! the sprite by its DELTA from the press point — the
//! sprite never jumps to the cursor, whatever its pivot —
//! and the write lands at the authored frame,
//! `core.current`, the frame every other key editor in
//! the shell writes at (the Sprites panel's doctrine).
//!
//! The undo discipline is the seat's contract — exactly
//! one undo step per drag, through the sprite-key op
//! family, enrichment preserved:
//!
//! - During the gesture the target key is written IN
//!   PLACE (replace/insert at the drag frame, the op's
//!   own placement rule) with NO snapshot and NO status —
//!   `drag_to`'s live half: the stage redraws from the
//!   project, so the sprite follows the pointer, and the
//!   live key is exactly the key the commit will store.
//! - On a clean release the pre-drag key list (recorded
//!   at the grab) is restored in place and the final key
//!   is committed through `EditorCore::set_sprite_key_full`
//!   — the op takes the gesture's ONE snapshot against
//!   the restored pre-drag project and posts its own
//!   status sentence. The committed key is built from
//!   the panel's `base_key` (an exact key at the frame
//!   wins verbatim, so its enrichment — including a
//!   stored follow-`scale` sentinel — survives untouched;
//!   at a keyless frame the base is the sample, whose
//!   `scale_y` the write materialises to the sampled
//!   effective value, `base_key`'s documented behaviour)
//!   with only `x` / `y` shifted by the pointer delta.
//! - On a cancelled release the recorded pre-drag key
//!   list is restored in place and nothing is committed:
//!   no snapshot was ever taken, so the gesture's net
//!   undo cost is zero and the project is byte-identical
//!   to before the press. `main.rs::stage_drag`'s own
//!   cancel-undo never sees this gesture (it keys on
//!   `core.dragging` / draw mode, neither of which a
//!   sprite drag sets), so the two never double-restore.
//! - A press that never moves the sprite — or moves it
//!   out and back to the exact start — is a click: the
//!   X5 selection is its whole effect, no key is written
//!   and no undo step is spent (the panel's Add-key
//!   refusal of a phantom step, applied to a gesture).
//! - Pointer travel inside [`DRAG_THRESHOLD`] of the
//!   press point is jitter, not a drag (Wave AA, AA8):
//!   it writes nothing at all — not even a live key —
//!   so a click whose release lands a hair off the press
//!   is still a click, and a keyless frame authors no
//!   key from hand shake alone (the subtitle lane's
//!   "a wiggle inside one frame spends nothing", in
//!   stage units). The first application at or past
//!   the threshold starts the drag, and its target is
//!   still measured from the press point, so crossing
//!   costs no travel and the sprite does not jump.
//!
//! The record lives in this module's `thread_local`
//! (the subtitle lane's `CUE_DRAG` precedent): gesture
//! state with the gesture's own lifetime, needing no
//! new shell `State` field. Arming is refused while the
//! core is playing (`begin_drag`'s own refusal — the
//! display frame advances under a playing grab) and on a
//! stale sprite index; the pick that precedes it still
//! selects either way. If the record stops naming its
//! sprite mid-gesture (index out of range, or the
//! instance there now uses a different asset — the lane's
//! identity rule), the gesture is abandoned: nothing
//! further is written, restored or committed, and the
//! world that rewrote the sprite table owns what happens
//! next. A record somehow left over from an earlier
//! gesture (its Up never arrived) is settled as a cancel
//! before a new grab arms.
//!
//! The recorded frame is the gesture's frame for its
//! whole life (Wave AB, AB9 audit): a keyboard frame
//! step mid-gesture — the shell's arrow keys call
//! `core.goto` with no drag-in-flight gate — does NOT
//! retarget the drag, and neither live writes nor the
//! commit chase `current`. The base is the grab frame's
//! own data (the exact key there verbatim, else its
//! sample, `scale_y` materialised), so committing it at
//! a stepped-to frame would author one frame's sampled
//! transform as another frame's key. The stage gestures
//! that DO follow `current` — the pose drag, the camera
//! drag, the eraser — carry no grab-time payload: each
//! of their moves computes its value from the pointer
//! against the then-current frame's own data, so
//! retargeting costs them nothing. The record-carrying
//! gestures — this one, the brush stroke (its frame is
//! stamped into the `Stroke` at `begin_draw`), the
//! subtitle lane's cue drag, the graph canvas's key
//! drag — all hold their grab target the same way, and
//! the pin `sprite_drag_keeps_the_grab_frame_across_a_
//! mid_gesture_frame_step` holds this one to it.

use std::cell::RefCell;

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
            // Sprite items always carry the full source rect
            // (Wave X, X4); the field is not this pass's concern.
            ..
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

/// [`sprite_at_point`] over the core's own quads at its
/// display frame — the stage pointer path's pick (Wave X,
/// X5, SpriteLoop GAP P-3): `main.rs::stage_drag` converts
/// the press to stage coordinates and calls this on a
/// joint miss, so the sprite the user sees on top is the
/// sprite the click selects (the result feeds the Sprites
/// panel's selection, which the stage's own prim pass
/// already draws corner marks for). Read-only: picking
/// never snapshots and never sets the status — selection
/// is panel state, as everywhere else in the shell.
pub fn sprite_pick_at(core: &EditorCore, p: V2) -> Option<usize> {
    sprite_at_point(&sprite_quads_at(core), p)
}

/// Pointer travel from the press point, in stage units,
/// below which a sprite grab is a click rather than a
/// drag (Wave AA, AA8 — see the module docs'
/// "Dragging"): sub-threshold applications write
/// nothing, so release jitter cannot author a key or
/// spend an undo step. Stage units are the drag API's
/// only currency (the stage pick tolerance is likewise
/// stage-space); at the fit view one stage unit is
/// roughly one window point.
pub const DRAG_THRESHOLD: f32 = 2.0;

/// One stage sprite drag in flight (see the module docs,
/// "Dragging"): everything the settle needs, captured at
/// the grab — the sprite's identity (index + the asset it
/// used then), the press point the pointer delta is
/// measured from, the authored frame the write lands at,
/// the `base_key` the committed key is built from, and
/// the instance's pre-drag key list, which the cancel
/// path restores and the commit path restores-then-ops
/// over.
#[derive(Clone, Debug)]
pub struct SpriteDrag {
    /// Index into `Project.sprites`.
    pub sprite: usize,
    /// The instance's asset at grab time — the identity
    /// check (the lane's rule): if the instance at
    /// `sprite` no longer uses this asset, the record no
    /// longer names the sprite the user grabbed.
    pub asset: usize,
    /// The press point, in stage coordinates.
    pub press: V2,
    /// The authored frame (`core.current` at the grab)
    /// the drag writes at.
    pub frame: usize,
    /// `sprites_panel::base_key` at `frame`, captured at
    /// the grab: the exact key verbatim, else the sample
    /// (its `scale_y` materialised, the documented
    /// behaviour), else the stage-centre default. Only
    /// `x` / `y` ever change on it.
    pub base: InstanceKey,
    /// Whether the pointer has travelled at least
    /// [`DRAG_THRESHOLD`] from the press point — the
    /// jitter gate (Wave AA, AA8): until it has, no
    /// application writes anything; once it has, the
    /// drag stays a drag even if the pointer comes back
    /// inside the threshold (the release then settles
    /// by the ordinary net-effect rules).
    pub moved: bool,
    /// The instance's key list at the grab, verbatim.
    pub orig_keys: Vec<InstanceKey>,
}

thread_local! {
    /// The sprite drag in flight, if any. A
    /// `thread_local` keeps the record out of the shell's
    /// `State` (the subtitle lane's `CUE_DRAG`
    /// precedent); the shell's three drag phases call
    /// the `sprite_drag_*` functions below, which own
    /// the slot.
    static SPRITE_DRAG: RefCell<Option<SpriteDrag>> = const { RefCell::new(None) };
}

/// Whether a sprite drag is currently armed (the shell
/// routes its Move phase on this; the tests pin the
/// gesture's lifecycle on it).
pub fn sprite_drag_active() -> bool {
    SPRITE_DRAG.with(|cell| cell.borrow().is_some())
}

/// Field-wise key equality (`InstanceKey` carries no
/// `PartialEq`): every one of the eleven fields equal.
/// The values compared here are copies and sums of
/// copies of stored data, so exact comparison is the
/// honest test — a drag target computed from a zero
/// delta IS the base, bit for bit.
fn key_eq(a: &InstanceKey, b: &InstanceKey) -> bool {
    a.frame == b.frame
        && a.x == b.x
        && a.y == b.y
        && a.scale == b.scale
        && a.angle == b.angle
        && a.opacity == b.opacity
        && a.flip_x == b.flip_x
        && a.flip_y == b.flip_y
        && a.scale_y == b.scale_y
        && a.pivot_x == b.pivot_x
        && a.pivot_y == b.pivot_y
}

/// Whether `record` still names its sprite: the index is
/// in range and the instance there still uses the asset
/// it used at the grab.
fn drag_identity_holds(core: &EditorCore, record: &SpriteDrag) -> bool {
    core.project
        .sprites
        .get(record.sprite)
        .is_some_and(|inst| inst.asset == record.asset)
}

/// Restore `record`'s pre-drag key list onto its sprite,
/// in place — the cancel path's whole write, and the
/// commit path's first half. No snapshot, no status:
/// undo history is the committing op's business, and a
/// cancel must leave it untouched.
fn restore_orig_keys(core: &mut EditorCore, record: &SpriteDrag) {
    if let Some(inst) = core.project.sprites.get_mut(record.sprite) {
        inst.keys = record.orig_keys.clone();
    }
}

/// The key the drag would store for pointer point `p`:
/// the recorded base with `x` / `y` shifted by the
/// pointer's delta from the press point. `None` when the
/// result would be non-finite (a wild pointer conversion)
/// — such an application writes nothing, since the
/// committing op would refuse the same values.
fn drag_target(record: &SpriteDrag, p: V2) -> Option<InstanceKey> {
    let key = InstanceKey {
        x: record.base.x + (p[0] - record.press[0]),
        y: record.base.y + (p[1] - record.press[1]),
        ..record.base
    };
    (key.x.is_finite() && key.y.is_finite()).then_some(key)
}

/// Write `key` onto sprite `sprite`'s key list IN PLACE:
/// replace the key stored at `key.frame` (exact frame
/// equality — the sprite ops' own placement rule), or
/// insert it at its sorted position. No snapshot, no
/// status — the live half of the gesture; the commit at
/// release is what history records. Returns true when
/// the stored list actually changed.
fn write_key_live(core: &mut EditorCore, sprite: usize, key: InstanceKey) -> bool {
    let Some(inst) = core.project.sprites.get_mut(sprite) else {
        return false;
    };
    match inst.keys.iter().position(|k| k.frame == key.frame) {
        Some(pos) => {
            if key_eq(&inst.keys[pos], &key) {
                false
            } else {
                inst.keys[pos] = key;
                true
            }
        }
        None => {
            let pos = inst.keys.partition_point(|k| k.frame < key.frame);
            inst.keys.insert(pos, key);
            true
        }
    }
}

/// Point the grabbed sprite at the drag target for
/// pointer point `p` (the Move phase's application, and
/// the release's final one): a live, snapshot-less write
/// of the target key. If the record no longer names its
/// sprite the gesture is abandoned — the record is
/// dropped and nothing is written (see the module docs).
fn sprite_drag_apply(core: &mut EditorCore, p: V2) {
    let record = SPRITE_DRAG.with(|cell| cell.borrow().clone());
    let Some(mut record) = record else {
        return;
    };
    if !drag_identity_holds(core, &record) {
        SPRITE_DRAG.with(|cell| *cell.borrow_mut() = None);
        return;
    }
    if !record.moved {
        // The jitter gate (Wave AA, AA8): until the
        // pointer has travelled DRAG_THRESHOLD from the
        // press point the grab is still a click in
        // progress — write nothing, not even a live key,
        // so a keyless frame can gain no key from hand
        // shake and the release's net-nil check sees the
        // untouched list. Crossing flips `moved` on the
        // stored record; the target below is still
        // measured from the press point, so the sprite
        // starts moving with the full delta, no jump.
        let dx = p[0] - record.press[0];
        let dy = p[1] - record.press[1];
        if dx * dx + dy * dy < DRAG_THRESHOLD * DRAG_THRESHOLD {
            return;
        }
        record.moved = true;
        SPRITE_DRAG.with(|cell| *cell.borrow_mut() = Some(record.clone()));
    }
    if let Some(key) = drag_target(&record, p) {
        write_key_live(core, record.sprite, key);
    }
}

/// Arm a drag on sprite `sprite`, grabbed at stage point
/// `p` — called by `main.rs::stage_drag`'s Down phase
/// immediately after the X5 pick writes the selection, so
/// the press that selects is the press that can drag.
/// Takes no snapshot and sets no status (a grab is not an
/// edit). Arming is refused — the pick alone stands —
/// while the core is playing or the index is stale. A
/// leftover record from an earlier gesture is settled as
/// a cancel first, so no gesture's live writes can leak
/// into this one.
pub fn sprite_drag_down(core: &mut EditorCore, sprite: usize, p: V2) {
    if let Some(old) = SPRITE_DRAG.with(|cell| cell.borrow_mut().take()) {
        if drag_identity_holds(core, &old) {
            restore_orig_keys(core, &old);
        }
    }
    if core.playing {
        return;
    }
    let Some(inst) = core.project.sprites.get(sprite) else {
        return;
    };
    let frame = core.current;
    let record = SpriteDrag {
        sprite,
        asset: inst.asset,
        press: p,
        frame,
        base: crate::rune_sec::sprites_panel::base_key(core, inst, frame as f32),
        moved: false,
        orig_keys: inst.keys.clone(),
    };
    SPRITE_DRAG.with(|cell| *cell.borrow_mut() = Some(record));
}

/// The drag's Move phase: apply the target for stage
/// point `p` live (see [`sprite_drag_apply`]). Returns
/// true when a drag is armed — the shell then does NOT
/// fall through to the pose drag, which a sprite grab
/// never started (`begin_drag` missed, by construction
/// of the pick branch that armed this).
pub fn sprite_drag_move(core: &mut EditorCore, p: V2) -> bool {
    if !sprite_drag_active() {
        return false;
    }
    sprite_drag_apply(core, p);
    // The apply may have abandoned the gesture (identity
    // lost); the grab still owned this Move either way.
    true
}

/// The drag's Up phase, at release point `p`. Cancelled:
/// the pre-drag key list is restored in place — the
/// gesture leaves the project byte-identical and spends
/// no undo step. Clean release: the release position
/// applies once more; then, if the gesture's net effect
/// on the key list is nil (a click, or an excursion that
/// returned to the exact start), the live writes are
/// simply restored away and nothing is committed;
/// otherwise the pre-drag list is restored and the final
/// key is committed through
/// `EditorCore::set_sprite_key_full`, whose snapshot —
/// taken against the restored project — is the drag's
/// exactly one undo step, and whose status sentence is
/// the gesture's recap. No record: nothing happens (the
/// release belongs to some other gesture).
pub fn sprite_drag_up(core: &mut EditorCore, p: V2, cancelled: bool) {
    if !sprite_drag_active() {
        return;
    }
    if cancelled {
        if let Some(record) = SPRITE_DRAG.with(|cell| cell.borrow_mut().take()) {
            if drag_identity_holds(core, &record) {
                restore_orig_keys(core, &record);
            }
        }
        return;
    }
    sprite_drag_apply(core, p);
    let Some(record) = SPRITE_DRAG.with(|cell| cell.borrow_mut().take()) else {
        return;
    };
    if !drag_identity_holds(core, &record) {
        return;
    }
    let Some(inst) = core.project.sprites.get(record.sprite) else {
        return;
    };
    if inst.keys.len() == record.orig_keys.len()
        && inst
            .keys
            .iter()
            .zip(record.orig_keys.iter())
            .all(|(a, b)| key_eq(a, b))
    {
        // Net-nil gesture: the live list already equals
        // the pre-drag list, so there is nothing to
        // restore and nothing to commit.
        return;
    }
    let Some(final_key) = drag_target(&record, p) else {
        restore_orig_keys(core, &record);
        return;
    };
    restore_orig_keys(core, &record);
    core.set_sprite_key_full(record.sprite, record.frame, final_key);
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
            opacity: 1.0,
            flip_x: false,
            flip_y: false,
            scale_y: 0.0,
            pivot_x: 0.5,
            pivot_y: 0.5,
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
            Some(SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false, opacity: 1.0 });
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
            project_with_clip_def_media(Some(SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false, opacity: 1.0 }));
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
        clip_def.nodes[1].media = Some(SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false, opacity: 1.0 });
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
    fn sprite_pick_at_reads_the_core_display_frame() {
        // The pointer path's pick (Wave X, X5): sprite 0
        // sits at (300,200) at frame 0 and rotates away to
        // (340,200) by frame 2; sprite 1 sits at (100,100).
        let mut core = EditorCore::new();
        core.project = project_with_sprites();
        assert_eq!(sprite_pick_at(&core, [300.0, 200.0]), Some(0));
        assert_eq!(sprite_pick_at(&core, [100.0, 100.0]), Some(1));
        assert_eq!(sprite_pick_at(&core, [10.0, 10.0]), None);
        // Picking is a read: no snapshot, no status change.
        let status = core.status.clone();
        let _ = sprite_pick_at(&core, [300.0, 200.0]);
        assert_eq!(core.status, status);
        assert_eq!(core.undo_depth(), 0);
        // At frame 2 sprite 0's quad has moved/rotated:
        // its old centre now misses, its new centre hits.
        core.goto(2);
        assert_eq!(sprite_pick_at(&core, [300.0, 200.0]), None);
        assert_eq!(sprite_pick_at(&core, [340.0, 200.0]), Some(0));
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

    /// The serialised key list of sprite `sprite` — the
    /// byte-level identity the drag tests compare before /
    /// after a gesture (`InstanceKey` has no `PartialEq`).
    fn keys_json(core: &EditorCore, sprite: usize) -> String {
        serde_json::to_string(&core.project.sprites[sprite].keys)
            .expect("sprite keys serialise")
    }

    #[test]
    fn sprite_drag_moves_by_pointer_delta_and_commits_one_undo_step() {
        // Wave Z (Z3): the press lands at (310,195) — inside
        // sprite 0's quad but offset from its key position
        // (300,200) — and travels to (340,215): the sprite
        // must move by the DELTA (30,20), never jump to the
        // pointer.
        let mut core = EditorCore::new();
        core.project = project_with_sprites();
        let before = keys_json(&core, 0);
        assert!(!sprite_drag_active());
        sprite_drag_down(&mut core, 0, [310.0, 195.0]);
        assert!(sprite_drag_active());
        // The grab alone is not an edit.
        assert_eq!(core.undo_depth(), 0);
        assert_eq!(keys_json(&core, 0), before);
        // Live half: the stored key follows the pointer,
        // still with no snapshot spent.
        assert!(sprite_drag_move(&mut core, [340.0, 215.0]));
        let live = core.sprite_key_at(0, 0).expect("a key at frame 0");
        assert!(close([live.x, live.y], [330.0, 220.0]), "live {live:?}");
        assert_eq!(core.undo_depth(), 0, "live writes spend no snapshot");
        // Release: exactly one undo step, committed through
        // the op (its status sentence is the recap).
        sprite_drag_up(&mut core, [340.0, 215.0], false);
        assert!(!sprite_drag_active());
        assert_eq!(core.undo_depth(), 1, "one undo step per drag");
        assert_eq!(core.status, "Sprite 1 key set at frame 1.");
        let key = core.sprite_key_at(0, 0).expect("a key at frame 0");
        assert!(close([key.x, key.y], [330.0, 220.0]), "committed {key:?}");
        assert_eq!(key.scale, 10.0);
        assert_eq!(key.angle, 0.0);
        // The other key is untouched, and the one undo
        // restores the pre-drag list byte for byte.
        let end = core.sprite_key_at(0, 2).expect("a key at frame 2");
        assert!(close([end.x, end.y], [340.0, 200.0]));
        assert_eq!(end.angle, 90.0);
        core.undo();
        assert_eq!(keys_json(&core, 0), before);
    }

    #[test]
    fn sprite_drag_preserves_the_exact_keys_enrichment_verbatim() {
        // The committed key is the grabbed key with only
        // x/y shifted: opacity, flips, an EXPLICIT scale_y
        // and the pivot all ride through set_sprite_key_full
        // untouched.
        let mut core = EditorCore::new();
        let mut project = project_with_sprites();
        project.sprites[0].keys[0] = InstanceKey {
            opacity: 0.5,
            flip_x: true,
            scale_y: 3.0,
            pivot_x: 0.25,
            pivot_y: 0.75,
            ..project.sprites[0].keys[0]
        };
        core.project = project;
        sprite_drag_down(&mut core, 0, [300.0, 200.0]);
        assert!(sprite_drag_move(&mut core, [315.0, 190.0]));
        sprite_drag_up(&mut core, [315.0, 190.0], false);
        let key = core.sprite_key_at(0, 0).expect("a key at frame 0");
        assert!(close([key.x, key.y], [315.0, 190.0]), "{key:?}");
        assert_eq!(key.opacity, 0.5);
        assert!(key.flip_x);
        assert!(!key.flip_y);
        assert_eq!(key.scale_y, 3.0, "an explicit scale_y is not the sentinel");
        assert_eq!((key.pivot_x, key.pivot_y), (0.25, 0.75));
        assert_eq!(core.undo_depth(), 1);
    }

    #[test]
    fn sprite_drag_at_a_keyless_frame_authors_from_the_sample() {
        // Frame 1 has no key: the drag authors one there
        // from the sampled transform (320,200 / 45 degrees
        // between the frame-0 and frame-2 keys), and the
        // sampled follow-`scale` sentinel MATERIALISES to
        // the effective value on write — base_key's
        // documented behaviour, preserved.
        let mut core = EditorCore::new();
        core.project = project_with_sprites();
        core.goto(1);
        assert!(core.sprite_key_at(0, 1).is_none());
        sprite_drag_down(&mut core, 0, [320.0, 200.0]);
        assert!(sprite_drag_move(&mut core, [330.0, 190.0]));
        sprite_drag_up(&mut core, [330.0, 190.0], false);
        let key = core.sprite_key_at(0, 1).expect("the drag authored a key");
        assert_eq!(key.frame, 1.0);
        assert!(close([key.x, key.y], [330.0, 190.0]), "{key:?}");
        assert!((key.angle - 45.0).abs() < 0.01, "sampled angle {key:?}");
        assert_eq!(key.scale, 10.0);
        assert_eq!(
            key.scale_y, 10.0,
            "the follow sentinel materialises to the sampled effective scale_y"
        );
        assert_eq!(core.project.sprites[0].keys.len(), 3);
        assert_eq!(core.undo_depth(), 1);
    }

    #[test]
    fn sprite_drag_cancel_restores_the_pre_drag_state() {
        let mut core = EditorCore::new();
        core.project = project_with_sprites();
        let before = keys_json(&core, 0);
        let status = core.status.clone();
        sprite_drag_down(&mut core, 0, [300.0, 200.0]);
        assert!(sprite_drag_move(&mut core, [360.0, 260.0]));
        // The live half really did move the stored key…
        let live = core.sprite_key_at(0, 0).expect("a key at frame 0");
        assert!(close([live.x, live.y], [360.0, 260.0]));
        // …and the cancel puts it back, byte for byte,
        // with no snapshot spent and no status posted.
        sprite_drag_up(&mut core, [360.0, 260.0], true);
        assert!(!sprite_drag_active());
        assert_eq!(keys_json(&core, 0), before);
        assert_eq!(core.undo_depth(), 0);
        assert_eq!(core.status, status);
    }

    #[test]
    fn sprite_click_and_returned_excursion_write_nothing() {
        // A press with no travel is the X5 click: selection
        // only (done by the caller), no key, no undo step.
        let mut core = EditorCore::new();
        core.project = project_with_sprites();
        let before = keys_json(&core, 0);
        sprite_drag_down(&mut core, 0, [300.0, 200.0]);
        sprite_drag_up(&mut core, [300.0, 200.0], false);
        assert_eq!(keys_json(&core, 0), before);
        assert_eq!(core.undo_depth(), 0);
        // An excursion that returns to the exact start is
        // likewise net-nil: no phantom undo step (the
        // panel's Add-key discipline, applied to a gesture).
        sprite_drag_down(&mut core, 0, [300.0, 200.0]);
        assert!(sprite_drag_move(&mut core, [350.0, 240.0]));
        assert!(sprite_drag_move(&mut core, [300.0, 200.0]));
        sprite_drag_up(&mut core, [300.0, 200.0], false);
        assert_eq!(keys_json(&core, 0), before);
        assert_eq!(core.undo_depth(), 0);
    }

    #[test]
    fn sprite_drag_jitter_below_threshold_is_a_click() {
        // Wave AA (AA8): pointer travel inside
        // DRAG_THRESHOLD of the press is hand jitter, not
        // a drag: no live write, no key, no undo step, no
        // status — even though the release point is not
        // the press point, so the pre-AA8 gesture would
        // have committed a key here.
        let mut core = EditorCore::new();
        core.project = project_with_sprites();
        let before = keys_json(&core, 0);
        let status = core.status.clone();
        sprite_drag_down(&mut core, 0, [300.0, 200.0]);
        assert!(sprite_drag_move(&mut core, [301.0, 201.0]));
        // Nothing was written live: the stored key is
        // exactly where it was.
        let live = core.sprite_key_at(0, 0).expect("a key at frame 0");
        assert!(close([live.x, live.y], [300.0, 200.0]), "live {live:?}");
        sprite_drag_up(&mut core, [301.5, 200.5], false);
        assert!(!sprite_drag_active());
        assert_eq!(keys_json(&core, 0), before);
        assert_eq!(core.undo_depth(), 0);
        assert_eq!(core.status, status);
        // At a KEYLESS frame the same jitter must not
        // materialise the sample into a stored key.
        core.goto(1);
        let before = keys_json(&core, 0);
        sprite_drag_down(&mut core, 0, [320.0, 200.0]);
        assert!(sprite_drag_move(&mut core, [321.0, 201.0]));
        sprite_drag_up(&mut core, [321.0, 201.0], false);
        assert_eq!(core.project.sprites[0].keys.len(), 2, "no key authored");
        assert_eq!(keys_json(&core, 0), before);
        assert_eq!(core.undo_depth(), 0);
    }

    #[test]
    fn sprite_drag_threshold_crossing_moves_by_the_full_delta() {
        // Wave AA (AA8): the first sub-threshold Move
        // writes nothing; the Move that crosses applies
        // the target measured from the PRESS point — the
        // threshold is not subtracted, so the sprite does
        // not jump — and the release commits the usual
        // single undo step through the op.
        let mut core = EditorCore::new();
        core.project = project_with_sprites();
        sprite_drag_down(&mut core, 0, [300.0, 200.0]);
        assert!(sprite_drag_move(&mut core, [301.0, 200.0]));
        let live = core.sprite_key_at(0, 0).expect("a key at frame 0");
        assert!(close([live.x, live.y], [300.0, 200.0]), "live {live:?}");
        assert!(sprite_drag_move(&mut core, [310.0, 204.0]));
        let live = core.sprite_key_at(0, 0).expect("a key at frame 0");
        assert!(close([live.x, live.y], [310.0, 204.0]), "live {live:?}");
        assert_eq!(core.undo_depth(), 0, "live writes spend no snapshot");
        sprite_drag_up(&mut core, [310.0, 204.0], false);
        assert_eq!(core.undo_depth(), 1, "one undo step per drag");
        assert_eq!(core.status, "Sprite 1 key set at frame 1.");
        let key = core.sprite_key_at(0, 0).expect("a key at frame 0");
        assert!(close([key.x, key.y], [310.0, 204.0]), "committed {key:?}");
    }

    #[test]
    fn sprite_drag_refused_while_playing_and_on_a_stale_index() {
        let mut core = EditorCore::new();
        core.project = project_with_sprites();
        let before = keys_json(&core, 0);
        core.playing = true;
        sprite_drag_down(&mut core, 0, [300.0, 200.0]);
        assert!(!sprite_drag_active(), "no grab while playing");
        assert!(!sprite_drag_move(&mut core, [360.0, 260.0]));
        sprite_drag_up(&mut core, [360.0, 260.0], false);
        core.playing = false;
        sprite_drag_down(&mut core, 9, [300.0, 200.0]);
        assert!(!sprite_drag_active(), "no grab on a stale index");
        assert_eq!(keys_json(&core, 0), before);
        assert_eq!(core.undo_depth(), 0);
    }

    #[test]
    fn sprite_drag_abandons_when_the_sprite_changes_under_it() {
        // The lane's identity rule: the instance at the
        // grabbed index is re-pointed at another asset
        // mid-gesture (an external rewrite) — the record
        // no longer names the grabbed sprite, so the
        // gesture writes, restores and commits nothing.
        let mut core = EditorCore::new();
        let mut project = project_with_sprites();
        project.assets.push(ImageAsset {
            name: "other.png".into(),
            png: tiny_png(),
        });
        core.project = project;
        sprite_drag_down(&mut core, 0, [300.0, 200.0]);
        core.project.sprites[0].asset = 1;
        let midway = keys_json(&core, 0);
        assert!(sprite_drag_move(&mut core, [360.0, 260.0]), "the Move was owned");
        assert!(!sprite_drag_active(), "…and the gesture abandoned itself");
        sprite_drag_up(&mut core, [360.0, 260.0], false);
        assert_eq!(keys_json(&core, 0), midway);
        assert_eq!(core.undo_depth(), 0);
        // The table emptied under a grab: Move/Up are
        // harmless no-ops, nothing panics, nothing writes.
        sprite_drag_down(&mut core, 0, [300.0, 200.0]);
        core.project.sprites.clear();
        assert!(sprite_drag_move(&mut core, [10.0, 10.0]));
        sprite_drag_up(&mut core, [10.0, 10.0], false);
        assert!(core.project.sprites.is_empty());
        assert_eq!(core.undo_depth(), 0);
    }

    #[test]
    fn sprite_drag_keeps_the_grab_frame_across_a_mid_gesture_frame_step() {
        // Wave AB (AB9): the shell's arrow keys step
        // `core.current` with no drag-in-flight gate, so a
        // frame step CAN land mid-gesture. The record's
        // frame — and the base sampled/captured at it —
        // stays the gesture's target throughout: the base
        // is the grab frame's own data, and committing it
        // at a stepped-to frame would author frame 0's
        // transform as frame 2's key. (The pose drag's
        // per-move read of `current` is no counter-model:
        // it carries no grab-time payload — see the
        // module docs' Dragging section.)
        let mut core = EditorCore::new();
        core.project = project_with_sprites();
        let before = keys_json(&core, 0);
        sprite_drag_down(&mut core, 0, [310.0, 195.0]);
        assert!(sprite_drag_move(&mut core, [340.0, 215.0]));
        // The mid-gesture step: navigation only — no
        // snapshot, and the drag stays armed.
        core.goto(2);
        assert!(sprite_drag_active());
        assert!(sprite_drag_move(&mut core, [350.0, 225.0]));
        // Live writes still land at the grab frame, at
        // the full press-relative delta (40, 30) on the
        // frame-0 base (300, 200)…
        let live = core.sprite_key_at(0, 0).expect("a key at frame 0");
        assert!(close([live.x, live.y], [340.0, 230.0]), "live {live:?}");
        // …and frame 2's own key is untouched by them.
        let end = core.sprite_key_at(0, 2).expect("a key at frame 2");
        assert!(close([end.x, end.y], [340.0, 200.0]));
        assert_eq!(end.angle, 90.0);
        assert_eq!(core.undo_depth(), 0, "still no snapshot mid-gesture");
        // Release at the stepped-to frame: the commit
        // still goes through the op at the GRAB frame —
        // one undo step, and the op's recap names frame 1
        // (1-based), not the displayed frame 3.
        sprite_drag_up(&mut core, [350.0, 225.0], false);
        assert!(!sprite_drag_active());
        assert_eq!(core.undo_depth(), 1, "one undo step per drag");
        assert_eq!(core.status, "Sprite 1 key set at frame 1.");
        let key = core.sprite_key_at(0, 0).expect("a key at frame 0");
        assert!(close([key.x, key.y], [340.0, 230.0]), "committed {key:?}");
        assert_eq!(core.project.sprites[0].keys.len(), 2, "no key authored at the stepped-to frame");
        core.undo();
        assert_eq!(keys_json(&core, 0), before);
    }
}
