//! Stage strokes layer for the Rune editor shell (Phase E,
//! section 2): pure geometry for the current frame's drawing
//! strokes, in **stage coordinates**.
//!
//! The Phase C/D stage (`src/stage_view.rs`) draws onion skins,
//! figures and joint handles — but not the project's brush
//! strokes, so drawings are invisible in the Rune editor even
//! though every export shows them. This module is the missing
//! layer's data: it turns the authored strokes of the frame
//! being displayed into a flat list of primitives
//! ([`StrokePrim`]) that the integrator converts through
//! `stage_view`'s `ViewTransform` (`to_screen` + `scale`) into
//! a Rune `Shape` — polylines via `Shape::stroke_with`/`line`,
//! filled polygons via `Shape::fill`, dots as small filled
//! discs (the `ngon` workaround, since `Shape` has no circle
//! primitive — docs/RUNE_API_AUDIT.md). No Rune types appear
//! here: points stay in stage units, widths in stage units,
//! colours as `0xRRGGBB`, exactly as the model stores them.
//!
//! ## Which frame
//!
//! `EditorCore::display_frame` (`src/editor.rs:723`) is an
//! `f32` — fractional while playing with tween playback on —
//! while `Stroke::frame` (`src/model.rs:482`) is an authored
//! `usize`. The selection mirrors the rasterizer exactly:
//! `authored = (display.floor() as usize).min(frames - 1)`
//! (`src/export.rs:403`), then strokes with
//! `stroke.frame == authored` (`src/export.rs:406`). With
//! tween playback off this is simply `core.current`, the
//! frame the GPUI canvas filtered on (deleted `src/app.rs`
//! at `1ea3633`, line 2414). Strokes keep their stored order,
//! as in both renderers (`src/export.rs:405`,
//! `src/svg.rs:57`).
//!
//! ## The three kinds, and the evidence for each
//!
//! * **Spray** (`stroke.spray` is `Some`): dots only, no
//!   polyline — the rasterizer's decision (`src/export.rs:407`,
//!   "a sprayed path must read as spray, not as a line with
//!   dots on it"), the SVG exporter's (`src/svg.rs:61`) and
//!   the GPUI canvas's (deleted `src/app.rs` at `1ea3633`,
//!   line 5125). Dot centres and per-dot alphas come from
//!   `paint::spray_dots` (`src/paint.rs:512`), the single
//!   spray-geometry source all renderers share; dot radius is
//!   `(stroke.width * 0.5).max(1.0)` (`src/export.rs:408`,
//!   `src/svg.rs:68`) and the colour is `stroke.color`.
//!   **One [`StrokePrim::Dots`] is emitted per dot**, not one
//!   per stroke: a dot's alpha is
//!   `mark.opacity * (0.45 + 0.55 * frac)` (`src/paint.rs:505`),
//!   which varies dot by dot, and the prim carries a single
//!   `opacity` — grouping would silently flatten the falloff.
//!   This also matches the rasterizer's own intermediate form,
//!   a per-dot `(centre, radius, colour, alpha)` list
//!   (`src/export.rs:404`). Six dots per stroke point.
//! * **Filled** (`stroke.fill` is `Some`, at least 3 points):
//!   a filled polygon of the stroke's points in the fill
//!   colour. `fill` is "closed-shape fill colour for this
//!   stroke" (`src/model.rs:489`); `fill_current_stroke`
//!   sets it to the stroke's own colour
//!   (`src/editor.rs:2514`). Honest status: **no current
//!   renderer reads `Stroke::fill`** — the rasterizer and SVG
//!   exporter draw only the outline (plus the serpentine
//!   scanline companion stroke `fill_current_stroke` appends,
//!   `src/editor.rs:2516`), and the GPUI canvas ignored the
//!   field too. The prim exists because the brief asks for it
//!   and the field is real authored data; note the scanline
//!   companion also carries `fill`, so under this rule it
//!   becomes a filled (self-overlapping) polygon rather than
//!   its serpentine polyline — the one place this layer can
//!   differ from the exports, flagged for the integrator.
//!   A `fill` on fewer than 3 points cannot bound a polygon
//!   (`fill_current_stroke` itself refuses under 3 points,
//!   `src/editor.rs:2494`), so such a stroke falls through to
//!   the polyline rule instead of vanishing.
//! * **Plain**: one [`StrokePrim::Polyline`] per stroke with
//!   at least 2 points (a 1-point stroke draws nothing in
//!   either renderer: the rasterizer iterates `windows(2)`,
//!   `src/export.rs:416`; the SVG exporter requires
//!   `points.len() > 1`, `src/svg.rs:80`). The width is the
//!   stroke's uniform `width` field. The rasterizer refines
//!   *pressured* strokes per segment —
//!   `(widths[i] + widths[i + 1]) / 2.0`
//!   (`src/export.rs:417`) — which a single-width polyline
//!   cannot express; the other single-width renderer, the
//!   SVG exporter, uses `stroke.width` flat for the whole
//!   polyline (`src/svg.rs:93`), and that is the precedent
//!   followed here. For unpressured strokes (empty `widths`,
//!   the common case) the two renderers agree exactly.
//!   Colour is `stroke.color` (`src/export.rs:425`,
//!   `src/svg.rs:92`).
//!
//! Spray is tested before fill, fill before plain — the
//! renderers' own precedence (spray first:
//! `src/export.rs:407`, `src/svg.rs:61`).
//!
//! ## Stroke opacity (Phase M, section 14)
//!
//! `Stroke::opacity` (`src/model.rs`) is the per-stroke
//! opacity recorded from the brush when the stroke was
//! drawn (serde default 1.0 — older projects deserialize
//! as fully opaque). Every [`StrokePrim`] now carries an
//! `opacity: f32` — the mechanism [`StrokePrim::Dots`]
//! has always had, extended to `Polyline` and `Filled`
//! so the whole layer speaks one alpha language. (The
//! colour stays `0xRRGGBB`: `stage_view::hex_color`
//! masks to 24 bits, so there is no alpha byte to carry
//! it in, and the RGB must stay unchanged anyway.)
//!
//! * **Base layer** ([`stroke_geometry`]): a plain or
//!   filled stroke's prims carry the stroke's opacity,
//!   clamped to `[0.0, 1.0]` — the point-of-use clamp
//!   the SVG exporter applies in its `stroke_opacity`
//!   attribute (`src/svg.rs`). The integrator applies
//!   the carried value as the colour's alpha at
//!   conversion, exactly as it already does for `Dots`;
//!   at opacity 1.0 that is fully opaque, so the drawn
//!   result is unchanged from before this field existed.
//! * **Spray is exempt**: a spray stroke's dots already
//!   carry the *spray mark's* per-dot alpha (above), and
//!   `Stroke::opacity` is recorded as 1.0 for spray
//!   strokes. Belt and braces, the spray branch never
//!   reads `Stroke::opacity` at all, so even a
//!   hand-edited project with a translucent spray stroke
//!   sprays exactly as before.
//! * **Highlight** ([`highlight_geometry`]): the prims
//!   carry the *same* opacity values as the base layer
//!   (the stroke's for plain/filled, the mark's for
//!   spray dots) — this module stays composition-free,
//!   and the drawing side owns how the highlight alpha
//!   composes: `stage_view`'s highlight conversion
//!   applies its `HIGHLIGHT_ALPHA` *flat* on a polyline
//!   or fill's colour (the carried opacity is not
//!   multiplied in), so a selected translucent stroke's
//!   glow stays at full half-strength and remains
//!   visible; for spray dots it multiplies the carried
//!   mark alpha, as it always has.
//! * **Vertex dots** ([`stroke_points_geometry`]) keep
//!   opacity 1.0 — they are selection handles, not the
//!   stroke's rendering, and follow the highlight's
//!   reasoning: a faint stroke's handles stay findable.
//!
//! ## Selection highlight (Phase F, section 2)
//!
//! [`highlight_geometry`] produces the glow/outline version
//! of ONE authored stroke, for the integrator to draw when
//! the stroke-inspector row for that stroke is selected.
//! The contract:
//!
//! * **Indexing is project-wide**: `project_index` is the
//!   index into `core.project.strokes` — the same indexing
//!   `EditorCore::delete_stroke_at` (`src/editor.rs:2462`)
//!   and the stroke inspector's rows use. An out-of-range
//!   index yields an empty vec, never a panic.
//! * **Frame rule matches the base layer**: a stroke whose
//!   `frame` is not the authored frame ([`authored_frame`])
//!   yields an empty vec — a highlight is only ever produced
//!   for a stroke the stage is actually drawing, so the
//!   integrator can draw the result unconditionally.
//! * **Same decomposition as [`stroke_geometry`]**, same
//!   spray → filled → plain precedence, with each prim
//!   expanded: a plain stroke becomes one [`StrokePrim::
//!   Polyline`] through the *same points* at a grown width
//!   (a translucent underlay — see the colour note below);
//!   a spray stroke becomes one [`StrokePrim::Dots`] per
//!   dot with the *same centres and alphas* as the base
//!   layer and a grown radius; a filled stroke (≥ 3 points)
//!   becomes one [`StrokePrim::Polyline`] tracing the
//!   polygon's outline **expanded outward from its
//!   centroid**, closed (first point repeated at the end
//!   unless the stored points already close the loop).
//!   A filled stroke under 3 points falls through to the
//!   plain rule, and a sub-2-point plain stroke yields an
//!   empty vec — exactly the base layer's fallbacks, so the
//!   highlight never invents a shape the stage doesn't draw.
//! * **Colours are the stroke's own** (line colour, fill
//!   colour), carried for type consistency with the base
//!   prims, and so is the carried `opacity` (the stroke's
//!   recorded opacity for plain/filled prims, the mark
//!   alpha for spray dots — see "Stroke opacity" above).
//!   The highlight pass is the integrator's accent pass:
//!   it recolours every prim to the accent colour and
//!   applies its highlight alpha at conversion time —
//!   flat on a polyline/fill's colour, multiplied into a
//!   `Dots` prim's carried opacity. This module stays
//!   colour-policy-free.
//! * **Draw order (integrator)**: draw the highlight prims
//!   in the stroke layer's slot, *before* the base stroke
//!   prims, so the glow sits under the stroke it marks and
//!   the stroke itself stays crisp on top.
//!
//! ## Vertex dots (Phase J, section 34)
//!
//! [`stroke_points_geometry`] marks the *edit points* of
//! the selected stroke: [`highlight_geometry`] shows the
//! stroke's path as a glow, but the vertices a user would
//! grab to reshape it were invisible. The contract:
//!
//! * **One dot per vertex** of the stroke's *base
//!   polyline* (`Stroke::points`), for every kind of
//!   stroke: a plain stroke's path points, a filled
//!   stroke's polygon vertices, and a spray stroke's base
//!   points — **not** its scatter dots (the Phase G
//!   picking convention, below: the authored points are
//!   the stroke's geometry; the scatter is decoration
//!   derived from them). No spray → filled → plain
//!   decomposition is needed here: vertices are vertices
//!   however the stroke renders.
//! * **Representation**: one [`StrokePrim::Dots`] per
//!   vertex, `pts` holding exactly that vertex and
//!   `radius` = [`VERTEX_HALF`]. `Dots` is the prim the
//!   stage draws as a filled disc (the `ngon` workaround),
//!   and one prim per dot mirrors this layer's spray
//!   convention, keeping the list homogeneous with the
//!   highlight list it joins. A filled stroke's vertices
//!   are its *stored* points — not the highlight
//!   outline's expanded/shifted ones — because the dots
//!   mark where the authored points actually are.
//! * **Indexing and frame rule match
//!   [`highlight_geometry`]**: project-wide index, and a
//!   stroke not on the authored frame yields an empty
//!   vec — the dots are part of the highlight, so they
//!   obey its rule and the integrator can draw the
//!   result unconditionally. An out-of-range index
//!   yields an empty vec, never a panic.
//! * **Degenerate strokes**: a single-point stroke gets
//!   its one dot (the lone point is real geometry — the
//!   picking pass treats it so, below), even though
//!   `highlight_geometry` draws no underlay for it; a
//!   zero-point stroke gets none. Unlike the highlight,
//!   there is no ≥ 2-point rule: the dots mark points,
//!   not the drawn shape.
//! * **Colours are the stroke's own**, `opacity` 1.0 —
//!   deliberately *not* the stroke's recorded opacity
//!   (see "Stroke opacity" above): the dots are
//!   selection handles, and a translucent stroke's
//!   handles stay at full strength. Colour-policy-free
//!   like the highlight: the integrator's accent pass
//!   recolours the dots to the same accent and the
//!   conversion multiplies the opacity by the highlight
//!   alpha, so dots land at the same strength as the
//!   glow they accompany.
//! * **Draw order (integrator)**: draw the dots in the
//!   same highlight pass as [`highlight_geometry`],
//!   immediately after its prims — same slot, same
//!   accent, so path and points read as one selection.
//!
//! ## Picking (Phase G, section 5)
//!
//! [`stroke_at_point`] is the inverse of the draw pass:
//! given a point in **stage coordinates** (the shell
//! converts the pointer through `stage_view::to_stage`
//! before calling), it returns the project-wide index —
//! the same indexing as [`highlight_geometry`] and
//! `EditorCore::delete_stroke_at` — of the nearest stroke
//! on the authored frame whose geometry passes within
//! `tolerance` stage units of the point, or `None`. The
//! frame rule is the base layer's: strokes with
//! `stroke.frame != authored_frame(core)` are never
//! pickable, so a click can only select what the stage is
//! drawing. Per-kind measured geometry (documented
//! choice, following the base layer's spray → filled →
//! plain precedence):
//!
//! * **Spray**: the stroke's *base polyline* — the scatter
//!   dots do not count. Dots spread up to `mark.radius`
//!   stage units off the path; measuring them would make
//!   the pickable area a fuzzy disc cloud that spills
//!   past the visible line of the stroke and steals
//!   clicks meant for strokes underneath.
//! * **Filled** (≥ 3 points): the polygon's *outline*,
//!   closed (the last-to-first segment counts) — the
//!   interior does not count, because the base layer's
//!   `Filled` prim is the one place this layer already
//!   differs from the exports (no renderer fills), and a
//!   click "on" a shape the exports only outline should
//!   select by that outline. Under 3 points the base
//!   layer falls back to the plain rule, and so does
//!   picking.
//! * **Plain**: the open polyline through the points.
//!
//! A single-point stroke is measured by its distance to
//! that point (its geometry degenerates to the point), so
//! it stays selectable even though the base layer draws
//! nothing for it; an empty-points stroke has no geometry
//! and is never pickable. Distance to a polyline is the
//! minimum of [`point_segment_distance`] over its
//! segments. Ties are deterministic: strictly nearer
//! wins, and equal distances keep the **lowest index**
//! (the iteration is in project order and only a strictly
//! smaller distance replaces the current best — the same
//! discipline as `EditorCore::hit_test`,
//! `src/editor.rs:272`).
//!
//! **Tolerance for the shell**: pass [`PICK_TOLERANCE`]
//! (13.0 stage units). That is the radius the core's own
//! joint hit-test accepts (`EditorCore::hit_test`,
//! `src/editor.rs:272`), in the same stage space, so a
//! click that would grab a joint also grabs a stroke
//! equally far away. Cross-check against the view: the
//! default stage is 960×720 (`src/model.rs:753`) and
//! `ViewTransform::scale` is `fit * zoom`
//! (`src/stage_view.rs`), so at zoom 1 the 13 units are
//! ~13 screen points when the stage element is ~960
//! points wide (fewer on a narrower element, more when
//! zoomed) — a workable pointer target either way, and
//! being stage-constant it matches `hit_test` exactly. A
//! shell that wants a pixel-constant target can pass
//! `13.0 / vt.scale` instead; the helper takes the
//! tolerance as a parameter precisely so that choice
//! stays in the shell.
//!
//! ## Draw order (integrator)
//!
//! The GPUI canvas painted this layer **under the figures**
//! — "Brush strokes for this frame, under the figures"
//! (deleted `src/app.rs` at `1ea3633`, line 5122) — and
//! `stage_view` mirrors the GPUI stage's decisions, so the
//! converted shape belongs between the onion skins and the
//! figure pass, below the joint handles. (The rasterizer
//! composites differently — stroke segments join the figure
//! segment pass and spray dots ride the over-figure particle
//! pass; that is an export-compositing choice, not the
//! stage's.)

use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::Stroke;
use stykanimate_rs::paint;

/// One drawable piece of the current frame's stroke layer,
/// in stage coordinates. The integrator maps `pts` through
/// `stage_view::to_screen`, multiplies `width`/`radius` by
/// the transform's `scale`, and converts `color` with
/// `stage_view::hex_color`, applying the prim's `opacity`
/// as the colour's alpha (1.0 = fully opaque).
#[derive(Clone, Debug, PartialEq)]
pub enum StrokePrim {
    /// A plain brush stroke: one polyline through `pts` at
    /// uniform `width`, round caps (both renderers' choice:
    /// `src/export.rs:61`, `src/svg.rs:90`). `opacity` is
    /// the stroke's recorded opacity (Phase M, section
    /// 14 — see the module docs, "Stroke opacity").
    Polyline {
        pts: Vec<[f32; 2]>,
        width: f32,
        color: u32,
        opacity: f32,
    },
    /// A filled closed shape: the polygon through `pts`,
    /// filled with `color` (the stroke's stored fill
    /// colour) at the stroke's recorded `opacity`.
    Filled {
        pts: Vec<[f32; 2]>,
        color: u32,
        opacity: f32,
    },
    /// Spray dots sharing one radius/colour/opacity. Spray
    /// strokes emit one of these **per dot** (see the module
    /// docs), so `pts` normally holds exactly one centre; the
    /// field is a list so an integrator-side grouping pass
    /// never needs a new type.
    Dots {
        pts: Vec<[f32; 2]>,
        radius: f32,
        color: u32,
        opacity: f32,
    },
}

/// The authored frame whose strokes the stage shows for the
/// core's current display time: the rasterizer's derivation
/// (`src/export.rs:403`) over `EditorCore::display_frame`
/// (`src/editor.rs:723`). Exposed so other stage layers can
/// key on exactly the same frame.
pub fn authored_frame(core: &EditorCore) -> usize {
    (core.display_frame().floor() as usize)
        .min(core.project.frames.len().saturating_sub(1))
}

/// The alpha a stroke's prims carry: its recorded
/// opacity clamped to `[0.0, 1.0]` — the point-of-use
/// clamp the SVG exporter applies to the same field
/// (`src/svg.rs`, its `stroke_opacity` attribute), so
/// every renderer reads the field the same way even if
/// a hand-edited project stores an out-of-range value.
fn stroke_alpha(stroke: &Stroke) -> f32 {
    stroke.opacity.clamp(0.0, 1.0)
}

/// Geometry for the current frame's drawing strokes, in
/// stage coordinates and stored stroke order. Pure: reads
/// only `core.project.strokes` and the display frame, so the
/// same project state always yields the same primitives —
/// spray scatter included, since `paint::spray_dots` is
/// seeded solely from the stroke's own mark
/// (`src/paint.rs:512`).
pub fn stroke_geometry(core: &EditorCore) -> Vec<StrokePrim> {
    let project = &core.project;
    let authored = authored_frame(core);
    let mut out = Vec::new();
    for stroke in &project.strokes {
        if stroke.frame != authored {
            continue;
        }
        // Spray first (the renderers' precedence): dots only,
        // one prim per dot so each dot keeps its own alpha.
        // `stroke.opacity` is deliberately NOT applied here:
        // it is recorded as 1.0 for spray strokes, and the
        // dots' alpha already comes from the spray mark —
        // scaling by the stroke field as well would fade
        // spray twice (see the module docs, "Stroke
        // opacity").
        if let Some(mark) = &stroke.spray {
            let radius = (stroke.width * 0.5).max(1.0);
            for (centre, alpha) in paint::spray_dots(&stroke.points, mark) {
                out.push(StrokePrim::Dots {
                    pts: vec![centre],
                    radius,
                    color: stroke.color,
                    opacity: alpha,
                });
            }
            continue;
        }
        // Filled closed shape (needs at least 3 points to
        // bound a polygon; below that, fall through so the
        // stroke still draws as its outline). The fill
        // colour is the prim's colour; the alpha is the
        // stroke's recorded opacity.
        if let Some(fill) = stroke.fill {
            if stroke.points.len() >= 3 {
                out.push(StrokePrim::Filled {
                    pts: stroke.points.clone(),
                    color: fill,
                    opacity: stroke_alpha(stroke),
                });
                continue;
            }
        }
        // Plain stroke: one polyline at the uniform width,
        // at the stroke's recorded opacity.
        if stroke.points.len() >= 2 {
            out.push(StrokePrim::Polyline {
                pts: stroke.points.clone(),
                width: stroke.width,
                color: stroke.color,
                opacity: stroke_alpha(stroke),
            });
        }
    }
    out
}

/// Width growth for a plain stroke's highlight underlay:
/// `width * HIGHLIGHT_WIDTH_SCALE + HIGHLIGHT_WIDTH_PAD`
/// (stage units). The pad keeps even a hairline stroke's
/// highlight visible; the scale keeps a fat stroke's
/// highlight proportional.
const HIGHLIGHT_WIDTH_SCALE: f32 = 1.5;
const HIGHLIGHT_WIDTH_PAD: f32 = 3.0;
/// Radius growth for a spray dot's highlight:
/// `radius * HIGHLIGHT_DOT_SCALE + HIGHLIGHT_DOT_PAD`
/// (stage units), same reasoning as the width growth.
const HIGHLIGHT_DOT_SCALE: f32 = 1.8;
const HIGHLIGHT_DOT_PAD: f32 = 1.5;
/// How far (stage units) a filled stroke's highlight
/// outline sits outside the polygon, measured along the
/// ray from the polygon's centroid through each vertex.
const HIGHLIGHT_EXPAND: f32 = 3.0;
/// Extra width (stage units) on the filled outline
/// polyline over the stroke's own `width`.
const HIGHLIGHT_OUTLINE_PAD: f32 = 2.0;

/// The polygon through `pts` pushed outward by `expand`
/// stage units along each vertex's ray from the centroid,
/// closed: the first point is appended at the end unless
/// the stored points already close the loop. A vertex
/// sitting on the centroid (degenerate) stays put — it has
/// no outward direction.
fn expanded_outline(pts: &[[f32; 2]], expand: f32) -> Vec<[f32; 2]> {
    let n = pts.len() as f32;
    let centroid = [
        pts.iter().map(|p| p[0]).sum::<f32>() / n,
        pts.iter().map(|p| p[1]).sum::<f32>() / n,
    ];
    let mut out: Vec<[f32; 2]> = pts
        .iter()
        .map(|&p| {
            let d = [p[0] - centroid[0], p[1] - centroid[1]];
            let len = (d[0] * d[0] + d[1] * d[1]).sqrt();
            if len > 1e-6 {
                [p[0] + d[0] / len * expand, p[1] + d[1] / len * expand]
            } else {
                p
            }
        })
        .collect();
    if let (Some(&first), Some(&last)) = (out.first(), out.last()) {
        if first != last {
            out.push(first);
        }
    }
    out
}

/// Selection-highlight geometry for one authored stroke,
/// in stage coordinates. See the module docs ("Selection
/// highlight") for the full contract: project-wide indexing
/// (the `delete_stroke_at` indexing), the authored-frame
/// rule, the per-kind expansion, and the colour policy
/// (prims carry the stroke's own colours; the integrator's
/// accent pass recolours and fades them). Pure, like
/// [`stroke_geometry`]: same project state, same result.
pub fn highlight_geometry(core: &EditorCore, project_index: usize) -> Vec<StrokePrim> {
    let project = &core.project;
    let Some(stroke) = project.strokes.get(project_index) else {
        return Vec::new();
    };
    // Only a stroke the stage draws now can be highlighted.
    if stroke.frame != authored_frame(core) {
        return Vec::new();
    }
    // Spray first (the base layer's precedence): one prim
    // per dot, same centres and alphas, grown radius.
    if let Some(mark) = &stroke.spray {
        let radius = (stroke.width * 0.5).max(1.0) * HIGHLIGHT_DOT_SCALE + HIGHLIGHT_DOT_PAD;
        return paint::spray_dots(&stroke.points, mark)
            .into_iter()
            .map(|(centre, alpha)| StrokePrim::Dots {
                pts: vec![centre],
                radius,
                color: stroke.color,
                opacity: alpha,
            })
            .collect();
    }
    // Filled: the expanded outline of the polygon. The
    // carried opacity is the stroke's own, exactly as the
    // base layer's filled prim carries it — how the
    // highlight alpha composes with it is the drawing
    // side's decision (module docs, "Stroke opacity").
    if let Some(fill) = stroke.fill {
        if stroke.points.len() >= 3 {
            return vec![StrokePrim::Polyline {
                pts: expanded_outline(&stroke.points, HIGHLIGHT_EXPAND),
                width: stroke.width + HIGHLIGHT_OUTLINE_PAD,
                color: fill,
                opacity: stroke_alpha(stroke),
            }];
        }
    }
    // Plain: the same polyline, wider — an underlay the
    // integrator draws translucent in the accent colour.
    // The carried opacity is the stroke's own, as in the
    // base layer (the spray branch above likewise keeps
    // the mark's alphas untouched).
    if stroke.points.len() >= 2 {
        return vec![StrokePrim::Polyline {
            pts: stroke.points.clone(),
            width: stroke.width * HIGHLIGHT_WIDTH_SCALE + HIGHLIGHT_WIDTH_PAD,
            color: stroke.color,
            opacity: stroke_alpha(stroke),
        }];
    }
    Vec::new()
}

/// Half-extent (stage units) of one vertex dot: the
/// disc radius a vertex of the selected stroke is
/// marked with. Sized against the highlight underlay's
/// pad ([`HIGHLIGHT_WIDTH_PAD`], also 3.0): a dot on a
/// hairline stroke reads at the same scale as the glow
/// around it, and on a fat stroke it stays a point
/// marker rather than a blob.
pub const VERTEX_HALF: f32 = 3.0;

/// Vertex-dot geometry for one authored stroke, in
/// stage coordinates: one [`StrokePrim::Dots`] per
/// vertex of the stroke's base polyline, each dot a
/// disc of radius [`VERTEX_HALF`] centred exactly on
/// its vertex. See the module docs ("Vertex dots") for
/// the full contract: project-wide indexing and the
/// authored-frame rule (both shared with
/// [`highlight_geometry`] — an out-of-range index or an
/// off-frame stroke yields an empty vec), the base
/// polyline for every stroke kind (spray marks its
/// authored points, never its scatter), and the colour
/// policy (the stroke's own colour at opacity 1.0; the
/// integrator's accent pass recolours and fades). Pure,
/// like [`highlight_geometry`]: same project state,
/// same result.
pub fn stroke_points_geometry(core: &EditorCore, project_index: usize) -> Vec<StrokePrim> {
    let project = &core.project;
    let Some(stroke) = project.strokes.get(project_index) else {
        return Vec::new();
    };
    // Only a stroke the stage draws now gets its points
    // marked — the highlight's frame rule.
    if stroke.frame != authored_frame(core) {
        return Vec::new();
    }
    // The base polyline's vertices, one dot each,
    // whatever kind of stroke carries them.
    stroke
        .points
        .iter()
        .map(|&p| StrokePrim::Dots {
            pts: vec![p],
            radius: VERTEX_HALF,
            color: stroke.color,
            opacity: 1.0,
        })
        .collect()
}

/// The pick radius, in stage units, the shell should
/// pass to [`stroke_at_point`] for a plain stage click.
/// See the module docs ("Picking") for the grounding:
/// the core's own joint hit-test radius, in the same
/// stage space.
pub const PICK_TOLERANCE: f32 = 13.0;

/// Euclidean distance from point `p` to the segment
/// `a`–`b`, with the projection clamped to the segment
/// (so the nearest point may be an endpoint). A
/// zero-length segment measures the distance to `a`.
/// Pure; the picking pass is built on this alone.
pub fn point_segment_distance(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let len2 = ab[0] * ab[0] + ab[1] * ab[1];
    let nearest = if len2 > 0.0 {
        let t = (((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1]) / len2).clamp(0.0, 1.0);
        [a[0] + t * ab[0], a[1] + t * ab[1]]
    } else {
        a
    };
    let d = [p[0] - nearest[0], p[1] - nearest[1]];
    (d[0] * d[0] + d[1] * d[1]).sqrt()
}

/// The distance from `point` to the geometry picking
/// measures for `stroke` (module docs, "Picking"): the
/// base polyline for spray and plain strokes, the closed
/// outline for filled strokes with at least 3 points,
/// the lone point itself for a single-point stroke.
/// `None` when the stroke has no points at all.
fn stroke_pick_distance(stroke: &Stroke, point: [f32; 2]) -> Option<f32> {
    let pts = &stroke.points;
    match pts.len() {
        0 => None,
        // Degenerate segment: distance to the point.
        1 => Some(point_segment_distance(point, pts[0], pts[0])),
        _ => {
            // Filled shapes pick by their closed outline;
            // spray counts by its base polyline even if a
            // fill is also stored (the base layer's spray
            // precedence), and a fill under 3 points falls
            // back to the open polyline like the base layer.
            let closed = stroke.spray.is_none() && stroke.fill.is_some() && pts.len() >= 3;
            let mut best = f32::INFINITY;
            for seg in pts.windows(2) {
                best = best.min(point_segment_distance(point, seg[0], seg[1]));
            }
            if closed {
                best = best.min(point_segment_distance(point, pts[pts.len() - 1], pts[0]));
            }
            Some(best)
        }
    }
}

/// Pick the stroke at a stage-space point: the
/// project-wide index (the `delete_stroke_at` /
/// [`highlight_geometry`] indexing) of the nearest
/// stroke on the authored frame whose measured geometry
/// passes within `tolerance` stage units of `point`, or
/// `None` when nothing is that close. See the module
/// docs ("Picking") for the frame rule, the per-kind
/// geometry and the deterministic tie-break (nearer
/// wins; equal distances keep the lowest index). Pure:
/// same project state and point, same answer.
pub fn stroke_at_point(core: &EditorCore, point: [f32; 2], tolerance: f32) -> Option<usize> {
    let authored = authored_frame(core);
    let mut best: Option<(usize, f32)> = None;
    for (index, stroke) in core.project.strokes.iter().enumerate() {
        if stroke.frame != authored {
            continue;
        }
        let Some(d) = stroke_pick_distance(stroke, point) else {
            continue;
        };
        if d <= tolerance && best.map(|(_, bd)| d < bd).unwrap_or(true) {
            best = Some((index, d));
        }
    }
    best.map(|(index, _)| index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use stykanimate_rs::model::{SprayMark, Stroke};

    fn plain(frame: usize, points: &[[f32; 2]], color: u32, width: f32) -> Stroke {
        Stroke {
            frame,
            points: points.to_vec(),
            color,
            width,
            widths: Vec::new(),
            fill: None,
            spray: None,
            opacity: 1.0,
        }
    }

    fn core_with(strokes: Vec<Stroke>) -> EditorCore {
        let mut core = EditorCore::new();
        core.project.strokes = strokes;
        core
    }

    #[test]
    fn plain_stroke_becomes_one_polyline() {
        let pts = [[10.0, 20.0], [30.0, 40.0], [50.0, 20.0]];
        let core = core_with(vec![plain(0, &pts, 0x2686E8, 6.0)]);
        let geo = stroke_geometry(&core);
        assert_eq!(
            geo,
            vec![StrokePrim::Polyline {
                pts: pts.to_vec(),
                width: 6.0,
                color: 0x2686E8,
                opacity: 1.0,
            }]
        );
    }

    #[test]
    fn pressured_stroke_uses_the_uniform_width_field() {
        // The documented single-width choice (the SVG
        // exporter's precedent): per-point widths stay a
        // rasterizer-only refinement.
        let mut s = plain(0, &[[0.0, 0.0], [10.0, 0.0], [20.0, 0.0]], 0x111111, 4.0);
        s.widths = vec![2.0, 10.0, 2.0];
        let core = core_with(vec![s]);
        match &stroke_geometry(&core)[..] {
            [StrokePrim::Polyline { width, .. }] => assert_eq!(*width, 4.0),
            other => panic!("expected one polyline, got {other:?}"),
        }
    }

    #[test]
    fn sub_two_point_plain_strokes_emit_nothing() {
        let core = core_with(vec![
            plain(0, &[[5.0, 5.0]], 0x111111, 4.0),
            plain(0, &[], 0x222222, 4.0),
        ]);
        assert!(stroke_geometry(&core).is_empty());
    }

    #[test]
    fn filled_stroke_becomes_filled_polygon_in_the_fill_colour() {
        let pts = [[0.0, 0.0], [40.0, 0.0], [40.0, 30.0], [0.0, 0.0]];
        let mut s = plain(0, &pts, 0x445566, 3.0);
        s.fill = Some(0x112233);
        let core = core_with(vec![s]);
        assert_eq!(
            stroke_geometry(&core),
            vec![StrokePrim::Filled {
                pts: pts.to_vec(),
                color: 0x112233,
                opacity: 1.0,
            }]
        );
    }

    #[test]
    fn filled_stroke_with_two_points_falls_back_to_polyline() {
        let mut s = plain(0, &[[0.0, 0.0], [10.0, 10.0]], 0x445566, 3.0);
        s.fill = Some(0x112233);
        let core = core_with(vec![s]);
        match &stroke_geometry(&core)[..] {
            [StrokePrim::Polyline { color, .. }] => assert_eq!(*color, 0x445566),
            other => panic!("expected one fallback polyline, got {other:?}"),
        }
    }

    #[test]
    fn spray_stroke_becomes_per_dot_dots() {
        let mut s = plain(0, &[[100.0, 100.0], [160.0, 100.0]], 0xE8862E, 6.0);
        s.spray = Some(SprayMark {
            seed: 7,
            radius: 10.0,
            opacity: 0.8,
        });
        let core = core_with(vec![s]);
        let geo = stroke_geometry(&core);
        // 2 points x 6 dots per point (paint::spray_dots).
        assert_eq!(geo.len(), 12);
        for prim in &geo {
            match prim {
                StrokePrim::Dots {
                    pts,
                    radius,
                    color,
                    opacity,
                } => {
                    assert_eq!(pts.len(), 1);
                    assert_eq!(*radius, 3.0, "radius is (width * 0.5).max(1.0)");
                    assert_eq!(*color, 0xE8862E);
                    // alpha = 0.8 * (0.45 + 0.55 * frac), frac in [0, 1).
                    assert!(*opacity >= 0.8 * 0.45 - 1e-6 && *opacity <= 0.8);
                    // Dots scatter within mark.radius of a stroke point.
                    let c = pts[0];
                    let near = [100.0, 160.0]
                        .iter()
                        .any(|&x| (c[0] - x).abs() <= 10.0 && (c[1] - 100.0).abs() <= 10.0);
                    assert!(near, "dot {c:?} outside both spray discs");
                }
                other => panic!("spray must emit only Dots, got {other:?}"),
            }
        }
    }

    #[test]
    fn spray_dot_radius_clamps_to_one() {
        let mut s = plain(0, &[[0.0, 0.0]], 0x111111, 1.0);
        s.spray = Some(SprayMark {
            seed: 3,
            radius: 5.0,
            opacity: 1.0,
        });
        let core = core_with(vec![s]);
        match &stroke_geometry(&core)[0] {
            StrokePrim::Dots { radius, .. } => assert_eq!(*radius, 1.0),
            other => panic!("expected Dots, got {other:?}"),
        }
    }

    #[test]
    fn spray_geometry_is_deterministic() {
        let build = || {
            let mut s = plain(0, &[[10.0, 10.0], [30.0, 14.0], [50.0, 10.0]], 0x3FA34D, 8.0);
            s.spray = Some(SprayMark {
                seed: 42,
                radius: 12.0,
                opacity: 0.6,
            });
            core_with(vec![s])
        };
        // Same state -> identical geometry, twice over, and
        // two identical strokes spray identical dot streams.
        assert_eq!(stroke_geometry(&build()), stroke_geometry(&build()));
        let mut s1 = plain(0, &[[10.0, 10.0]], 0x3FA34D, 8.0);
        s1.spray = Some(SprayMark {
            seed: 42,
            radius: 12.0,
            opacity: 0.6,
        });
        let s2 = s1.clone();
        let core = core_with(vec![s1, s2]);
        let geo = stroke_geometry(&core);
        assert_eq!(geo.len(), 12);
        assert_eq!(geo[..6], geo[6..], "identical strokes must spray identically");
    }

    #[test]
    fn wrong_frame_strokes_are_excluded() {
        let core = core_with(vec![
            plain(0, &[[0.0, 0.0], [10.0, 0.0]], 0x111111, 4.0),
            plain(1, &[[0.0, 0.0], [10.0, 0.0]], 0x222222, 4.0),
        ]);
        let geo = stroke_geometry(&core);
        assert_eq!(geo.len(), 1);
        match &geo[0] {
            StrokePrim::Polyline { color, .. } => assert_eq!(*color, 0x111111),
            other => panic!("expected the frame-0 polyline, got {other:?}"),
        }
        // Moving to frame 1 flips the selection.
        let mut core = core;
        core.goto(1);
        let geo = stroke_geometry(&core);
        assert_eq!(geo.len(), 1);
        match &geo[0] {
            StrokePrim::Polyline { color, .. } => assert_eq!(*color, 0x222222),
            other => panic!("expected the frame-1 polyline, got {other:?}"),
        }
    }

    #[test]
    fn fractional_display_frame_floors_and_clamps_like_the_rasterizer() {
        let mut core = core_with(vec![
            plain(1, &[[0.0, 0.0], [10.0, 0.0]], 0x111111, 4.0),
            plain(2, &[[0.0, 0.0], [10.0, 0.0]], 0x222222, 4.0),
        ]);
        assert!(core.frame_count() >= 3, "demo project needs 3+ frames");
        core.playing = true;
        core.project.tween_playback = true;
        core.playhead = 1.7;
        assert_eq!(authored_frame(&core), 1);
        assert_eq!(stroke_geometry(&core).len(), 1);
        // Past the end, the authored frame clamps to the last.
        core.playhead = 99.0;
        assert_eq!(authored_frame(&core), core.frame_count() - 1);
    }

    #[test]
    fn stored_order_is_preserved_across_kinds() {
        let mut filled = plain(0, &[[0.0, 0.0], [20.0, 0.0], [20.0, 20.0]], 0x111111, 3.0);
        filled.fill = Some(0x111111);
        let core = core_with(vec![
            plain(0, &[[0.0, 0.0], [10.0, 0.0]], 0x222222, 4.0),
            filled,
        ]);
        let geo = stroke_geometry(&core);
        assert!(matches!(geo[0], StrokePrim::Polyline { .. }));
        assert!(matches!(geo[1], StrokePrim::Filled { .. }));
    }

    #[test]
    fn no_strokes_no_geometry() {
        let core = core_with(Vec::new());
        assert!(stroke_geometry(&core).is_empty());
    }

    /// The `opacity` any prim carries, whichever variant
    /// it is — the one alpha language the layer speaks.
    fn prim_opacity(prim: &StrokePrim) -> f32 {
        match prim {
            StrokePrim::Polyline { opacity, .. }
            | StrokePrim::Filled { opacity, .. }
            | StrokePrim::Dots { opacity, .. } => *opacity,
        }
    }

    #[test]
    fn opaque_strokes_match_the_pre_change_construction() {
        // The Phase M pin: at the recorded default (1.0)
        // the prims are exactly what this layer produced
        // before prims carried alpha — same points, same
        // width, same colours — with opacity exactly 1.0,
        // which the conversion applies as fully opaque.
        let pts = [[10.0, 20.0], [30.0, 40.0], [50.0, 20.0]];
        let mut filled = plain(0, &pts, 0x445566, 3.0);
        filled.fill = Some(0x112233);
        let core = core_with(vec![plain(0, &pts, 0x2686E8, 6.0), filled]);
        assert_eq!(
            stroke_geometry(&core),
            vec![
                StrokePrim::Polyline {
                    pts: pts.to_vec(),
                    width: 6.0,
                    color: 0x2686E8,
                    opacity: 1.0,
                },
                StrokePrim::Filled {
                    pts: pts.to_vec(),
                    color: 0x112233,
                    opacity: 1.0,
                },
            ]
        );
    }

    #[test]
    fn translucent_strokes_scale_prim_alpha() {
        // Opacity 0.5 -> the carried alpha halves; the
        // colour RGB is untouched (the alpha rides in the
        // separate field, never in the colour).
        let pts = [[10.0, 20.0], [30.0, 40.0], [50.0, 20.0]];
        let mut plain_half = plain(0, &pts, 0x2686E8, 6.0);
        plain_half.opacity = 0.5;
        let mut filled_half = plain(0, &pts, 0x445566, 3.0);
        filled_half.fill = Some(0x112233);
        filled_half.opacity = 0.5;
        let core = core_with(vec![plain_half, filled_half]);
        let geo = stroke_geometry(&core);
        assert_eq!(geo.len(), 2);
        for prim in &geo {
            assert!(
                (prim_opacity(prim) - 0.5).abs() < 1e-6,
                "alpha halved, got {}",
                prim_opacity(prim)
            );
        }
        match &geo[0] {
            StrokePrim::Polyline { color, .. } => {
                assert_eq!(*color, 0x2686E8, "plain colour unchanged")
            }
            other => panic!("expected a polyline, got {other:?}"),
        }
        match &geo[1] {
            StrokePrim::Filled { color, .. } => {
                assert_eq!(*color, 0x112233, "fill colour unchanged")
            }
            other => panic!("expected a filled prim, got {other:?}"),
        }
    }

    #[test]
    fn stroke_opacity_is_clamped_to_the_unit_range() {
        // The SVG exporter's point-of-use clamp, mirrored:
        // an out-of-range stored value can neither brighten
        // a prim past opaque nor push its alpha negative.
        let pts = [[0.0, 0.0], [10.0, 0.0]];
        let mut over = plain(0, &pts, 0x111111, 4.0);
        over.opacity = 1.4;
        let mut under = plain(0, &pts, 0x222222, 4.0);
        under.opacity = -0.25;
        let core = core_with(vec![over, under]);
        let geo = stroke_geometry(&core);
        assert_eq!(prim_opacity(&geo[0]), 1.0);
        assert_eq!(prim_opacity(&geo[1]), 0.0);
    }

    #[test]
    fn spray_stroke_opacity_is_never_applied() {
        // A spray stroke with `opacity` 0.5 — a state the
        // recorder never produces (spray records 1.0) —
        // must spray exactly as the same stroke at 1.0:
        // the dots' alpha comes from the spray mark alone,
        // never scaled by the stroke field.
        let build = |stroke_opacity: f32| {
            let mut s = plain(0, &[[100.0, 100.0], [160.0, 100.0]], 0xE8862E, 6.0);
            s.spray = Some(SprayMark {
                seed: 7,
                radius: 10.0,
                opacity: 0.8,
            });
            s.opacity = stroke_opacity;
            core_with(vec![s])
        };
        assert_eq!(
            stroke_geometry(&build(0.5)),
            stroke_geometry(&build(1.0)),
            "spray geometry must not depend on Stroke::opacity"
        );
    }

    #[test]
    fn pressured_stroke_scales_every_prim() {
        // A per-point-width stroke at opacity 0.5: however
        // the stroke decomposes, every prim it produces
        // carries the scaled alpha — no sub-prim escapes
        // at full strength.
        let mut s = plain(0, &[[0.0, 0.0], [10.0, 0.0], [20.0, 0.0]], 0x111111, 4.0);
        s.widths = vec![2.0, 10.0, 2.0];
        s.opacity = 0.5;
        let core = core_with(vec![s]);
        let geo = stroke_geometry(&core);
        assert!(!geo.is_empty());
        for prim in &geo {
            assert!(
                (prim_opacity(prim) - 0.5).abs() < 1e-6,
                "every sub-prim scaled, got {}",
                prim_opacity(prim)
            );
        }
    }

    #[test]
    fn highlight_carries_the_stroke_opacity_unchanged() {
        // The composition this side of the boundary: the
        // highlight prims carry the stroke's opacity
        // exactly as the base layer does — NOT pre-scaled
        // by the highlight alpha. How the accent pass
        // composes the two is the drawing side's decision
        // (module docs, "Stroke opacity").
        let pts = [[10.0, 20.0], [30.0, 40.0], [50.0, 20.0]];
        let mut plain_q = plain(0, &pts, 0x2686E8, 6.0);
        plain_q.opacity = 0.4;
        let core = core_with(vec![plain_q]);
        let geo = highlight_geometry(&core, 0);
        assert_eq!(geo.len(), 1);
        assert!(
            (prim_opacity(&geo[0]) - 0.4).abs() < 1e-6,
            "highlight carries 0.4 unchanged, got {}",
            prim_opacity(&geo[0])
        );
        // The filled outline carries it too.
        let mut filled_q = plain(0, &pts, 0x445566, 3.0);
        filled_q.fill = Some(0x112233);
        filled_q.opacity = 0.4;
        let core = core_with(vec![filled_q]);
        let geo = highlight_geometry(&core, 0);
        assert_eq!(geo.len(), 1);
        assert!(
            (prim_opacity(&geo[0]) - 0.4).abs() < 1e-6,
            "filled highlight carries 0.4 unchanged, got {}",
            prim_opacity(&geo[0])
        );
    }

    #[test]
    fn highlight_polyline_is_wider_with_the_same_points() {
        let pts = [[10.0, 20.0], [30.0, 40.0], [50.0, 20.0]];
        let core = core_with(vec![plain(0, &pts, 0x2686E8, 6.0)]);
        let geo = highlight_geometry(&core, 0);
        assert_eq!(
            geo,
            vec![StrokePrim::Polyline {
                pts: pts.to_vec(),
                // 6.0 * 1.5 + 3.0 — grown, per the contract.
                width: 12.0,
                color: 0x2686E8,
                opacity: 1.0,
            }]
        );
        // And strictly wider than the base layer's prim.
        match &stroke_geometry(&core)[0] {
            StrokePrim::Polyline { width, .. } => assert!(*width < 12.0),
            other => panic!("expected base polyline, got {other:?}"),
        }
    }

    #[test]
    fn highlight_uses_the_project_wide_index() {
        // The delete_stroke_at indexing: index 1 is the
        // second stroke in project order, whatever its kind.
        let core = core_with(vec![
            plain(0, &[[0.0, 0.0], [10.0, 0.0]], 0x111111, 4.0),
            plain(0, &[[0.0, 5.0], [10.0, 5.0]], 0x222222, 4.0),
        ]);
        match &highlight_geometry(&core, 1)[..] {
            [StrokePrim::Polyline { color, .. }] => assert_eq!(*color, 0x222222),
            other => panic!("expected the second stroke's highlight, got {other:?}"),
        }
    }

    #[test]
    fn highlight_missing_index_is_empty() {
        let core = core_with(vec![plain(0, &[[0.0, 0.0], [10.0, 0.0]], 0x111111, 4.0)]);
        assert!(highlight_geometry(&core, 1).is_empty());
        assert!(highlight_geometry(&core, usize::MAX).is_empty());
        let empty = core_with(Vec::new());
        assert!(highlight_geometry(&empty, 0).is_empty());
    }

    #[test]
    fn highlight_spray_expands_the_dot_radius_only() {
        let mut s = plain(0, &[[100.0, 100.0], [160.0, 100.0]], 0xE8862E, 6.0);
        s.spray = Some(SprayMark {
            seed: 7,
            radius: 10.0,
            opacity: 0.8,
        });
        let core = core_with(vec![s]);
        let base = stroke_geometry(&core);
        let geo = highlight_geometry(&core, 0);
        // Same decomposition: one Dots prim per dot, same
        // centres and alphas as the base layer.
        assert_eq!(geo.len(), base.len());
        for (hl, bs) in geo.iter().zip(base.iter()) {
            let (
                StrokePrim::Dots {
                    pts: hp,
                    radius: hr,
                    color: hc,
                    opacity: ho,
                },
                StrokePrim::Dots {
                    pts: bp,
                    radius: br,
                    color: bc,
                    opacity: bo,
                },
            ) = (hl, bs)
            else {
                panic!("spray highlight must be Dots, got {hl:?}");
            };
            assert_eq!(hp, bp, "highlight keeps the dot centre");
            assert_eq!(ho, bo, "highlight keeps the dot alpha");
            assert_eq!(hc, bc);
            // Base radius 3.0 -> 3.0 * 1.8 + 1.5 = 6.9.
            assert!((*hr - 6.9).abs() < 1e-6, "grown radius, got {hr}");
            assert!(hr > br);
        }
    }

    #[test]
    fn highlight_filled_is_an_expanded_closed_outline() {
        // An unclosed triangle: the outline must close it.
        let pts = [[0.0, 0.0], [40.0, 0.0], [20.0, 30.0]];
        let mut s = plain(0, &pts, 0x445566, 3.0);
        s.fill = Some(0x112233);
        let core = core_with(vec![s]);
        match &highlight_geometry(&core, 0)[..] {
            [StrokePrim::Polyline {
                pts: outline,
                width,
                color,
                opacity,
            }] => {
                assert_eq!(*color, 0x112233, "outline carries the fill colour");
                assert_eq!(*opacity, 1.0, "outline carries the stroke's opacity");
                assert_eq!(*width, 5.0, "width is stroke width + 2.0 pad");
                assert_eq!(outline.len(), 4, "3 vertices + closing repeat");
                assert_eq!(outline[0], outline[3], "outline is closed");
                // Every vertex sits ~HIGHLIGHT_EXPAND (3.0)
                // farther from the centroid than its source.
                let c = [20.0, 10.0];
                for (o, p) in outline[..3].iter().zip(pts.iter()) {
                    let d_out = ((o[0] - c[0]).powi(2) + (o[1] - c[1]).powi(2)).sqrt();
                    let d_in = ((p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2)).sqrt();
                    assert!(
                        (d_out - d_in - 3.0).abs() < 1e-4,
                        "vertex {p:?} expanded by {}, expected 3.0",
                        d_out - d_in
                    );
                }
            }
            other => panic!("expected one outline polyline, got {other:?}"),
        }
    }

    #[test]
    fn highlight_other_frame_stroke_is_empty() {
        let core = core_with(vec![plain(1, &[[0.0, 0.0], [10.0, 0.0]], 0x111111, 4.0)]);
        // The stroke exists (index 0) but is not on the
        // authored frame the stage is showing.
        assert!(highlight_geometry(&core, 0).is_empty());
    }

    #[test]
    fn highlight_sub_two_point_plain_stroke_is_empty() {
        let core = core_with(vec![plain(0, &[[5.0, 5.0]], 0x111111, 4.0)]);
        assert!(highlight_geometry(&core, 0).is_empty());
    }

    #[test]
    fn vertex_dots_one_per_point_at_the_polyline_positions() {
        let pts = [[10.0, 20.0], [30.0, 40.0], [50.0, 20.0]];
        let core = core_with(vec![plain(0, &pts, 0x2686E8, 6.0)]);
        let geo = stroke_points_geometry(&core, 0);
        assert_eq!(geo.len(), pts.len(), "one dot prim per vertex");
        for (prim, &p) in geo.iter().zip(pts.iter()) {
            match prim {
                StrokePrim::Dots {
                    pts: dot_pts,
                    radius,
                    color,
                    opacity,
                } => {
                    assert_eq!(*dot_pts, vec![p], "the dot sits on its vertex");
                    assert_eq!(*radius, VERTEX_HALF);
                    assert_eq!(*radius, 3.0, "the pinned half-extent");
                    assert_eq!(*color, 0x2686E8, "dots carry the stroke's colour");
                    assert_eq!(*opacity, 1.0, "the accent pass applies the fade");
                }
                other => panic!("vertex dots must be Dots, got {other:?}"),
            }
        }
    }

    #[test]
    fn vertex_dots_uses_the_project_wide_index() {
        // The delete_stroke_at indexing, shared with
        // highlight_geometry: index 1 marks the second
        // stroke's points, not the first's.
        let core = core_with(vec![
            plain(0, &[[0.0, 0.0], [10.0, 0.0]], 0x111111, 4.0),
            plain(0, &[[0.0, 5.0], [10.0, 5.0], [20.0, 5.0]], 0x222222, 4.0),
        ]);
        let geo = stroke_points_geometry(&core, 1);
        assert_eq!(geo.len(), 3);
        match &geo[0] {
            StrokePrim::Dots { pts, color, .. } => {
                assert_eq!(*pts, vec![[0.0, 5.0]]);
                assert_eq!(*color, 0x222222);
            }
            other => panic!("expected a Dots prim, got {other:?}"),
        }
    }

    #[test]
    fn vertex_dots_missing_index_is_empty() {
        let core = core_with(vec![plain(0, &[[0.0, 0.0], [10.0, 0.0]], 0x111111, 4.0)]);
        assert!(stroke_points_geometry(&core, 1).is_empty());
        assert!(stroke_points_geometry(&core, usize::MAX).is_empty());
        let empty = core_with(Vec::new());
        assert!(stroke_points_geometry(&empty, 0).is_empty());
    }

    #[test]
    fn vertex_dots_spray_marks_the_base_points_not_the_scatter() {
        let pts = [[100.0, 100.0], [160.0, 100.0]];
        let mut s = plain(0, &pts, 0xE8862E, 6.0);
        s.spray = Some(SprayMark {
            seed: 7,
            radius: 10.0,
            opacity: 0.8,
        });
        let core = core_with(vec![s]);
        let geo = stroke_points_geometry(&core, 0);
        // Two authored points -> two dots, NOT the twelve
        // scatter dots the base layer draws for this
        // stroke (2 points x 6 dots per point).
        assert_eq!(geo.len(), 2);
        for (prim, &p) in geo.iter().zip(pts.iter()) {
            match prim {
                StrokePrim::Dots { pts: dot_pts, .. } => {
                    assert_eq!(*dot_pts, vec![p], "dot on the base point, not a scatter dot");
                }
                other => panic!("vertex dots must be Dots, got {other:?}"),
            }
        }
    }

    #[test]
    fn vertex_dots_filled_marks_the_stored_polygon_vertices() {
        // The stored points, not the highlight outline's
        // expanded ones: the dots mark where the authored
        // points actually are.
        let pts = [[0.0, 0.0], [40.0, 0.0], [20.0, 30.0]];
        let mut s = plain(0, &pts, 0x445566, 3.0);
        s.fill = Some(0x112233);
        let core = core_with(vec![s]);
        let geo = stroke_points_geometry(&core, 0);
        assert_eq!(geo.len(), 3);
        for (prim, &p) in geo.iter().zip(pts.iter()) {
            match prim {
                StrokePrim::Dots {
                    pts: dot_pts,
                    color,
                    ..
                } => {
                    assert_eq!(*dot_pts, vec![p]);
                    assert_eq!(*color, 0x445566, "the stroke's colour, not the fill colour");
                }
                other => panic!("vertex dots must be Dots, got {other:?}"),
            }
        }
    }

    #[test]
    fn vertex_dots_other_frame_stroke_is_empty() {
        // The highlight's frame rule: a stroke the stage
        // is not drawing gets no dots either.
        let core = core_with(vec![plain(1, &[[0.0, 0.0], [10.0, 0.0]], 0x111111, 4.0)]);
        assert!(stroke_points_geometry(&core, 0).is_empty());
    }

    #[test]
    fn vertex_dots_degenerate_strokes() {
        // A lone point is real geometry (the picking pass
        // treats it so): it gets its one dot, even though
        // highlight_geometry draws no underlay for it.
        let core = core_with(vec![plain(0, &[[5.0, 5.0]], 0x111111, 4.0)]);
        let geo = stroke_points_geometry(&core, 0);
        assert_eq!(geo.len(), 1);
        match &geo[0] {
            StrokePrim::Dots { pts, .. } => assert_eq!(*pts, vec![[5.0, 5.0]]),
            other => panic!("expected a Dots prim, got {other:?}"),
        }
        // No points: nothing to mark.
        let core = core_with(vec![plain(0, &[], 0x111111, 4.0)]);
        assert!(stroke_points_geometry(&core, 0).is_empty());
    }

    #[test]
    fn point_segment_distance_axis_aligned_cases() {
        let a = [0.0, 0.0];
        let b = [10.0, 0.0];
        // On the segment, at an endpoint, perpendicular
        // above its middle — all hand-computed.
        assert_eq!(point_segment_distance([5.0, 0.0], a, b), 0.0);
        assert_eq!(point_segment_distance([10.0, 0.0], a, b), 0.0);
        assert_eq!(point_segment_distance([5.0, 3.0], a, b), 3.0);
        assert_eq!(point_segment_distance([5.0, -3.0], a, b), 3.0);
    }

    #[test]
    fn point_segment_distance_clamps_to_the_endpoints() {
        let a = [0.0, 0.0];
        let b = [10.0, 0.0];
        // Past b: nearest is b itself; 3-4-5 triangle.
        assert_eq!(point_segment_distance([13.0, 4.0], a, b), 5.0);
        // Before a: nearest is a itself; 3-4-5 again.
        assert_eq!(point_segment_distance([-3.0, -4.0], a, b), 5.0);
    }

    #[test]
    fn point_segment_distance_zero_length_segment() {
        // A degenerate segment measures the point itself.
        assert_eq!(point_segment_distance([2.0, 6.0], [2.0, 2.0], [2.0, 2.0]), 4.0);
        assert_eq!(point_segment_distance([5.0, 6.0], [2.0, 2.0], [2.0, 2.0]), 5.0);
    }

    #[test]
    fn point_segment_distance_diagonal_projection() {
        // Segment (0,0)-(2,2), point (0,2): the projection
        // falls at t = 0.5, nearest (1,1), distance sqrt(2).
        let d = point_segment_distance([0.0, 2.0], [0.0, 0.0], [2.0, 2.0]);
        assert!((d - std::f32::consts::SQRT_2).abs() < 1e-6, "got {d}");
    }

    #[test]
    fn pick_empty_project_is_none() {
        let core = core_with(Vec::new());
        assert_eq!(stroke_at_point(&core, [0.0, 0.0], PICK_TOLERANCE), None);
    }

    #[test]
    fn pick_pointless_stroke_is_none() {
        // A stroke with no points has no geometry at all.
        let core = core_with(vec![plain(0, &[], 0x111111, 4.0)]);
        assert_eq!(stroke_at_point(&core, [0.0, 0.0], PICK_TOLERANCE), None);
    }

    #[test]
    fn pick_respects_the_tolerance_boundary() {
        // Stroke along y = 0; clicks straight above it at
        // hand-computed distances.
        let core = core_with(vec![plain(0, &[[0.0, 0.0], [100.0, 0.0]], 0x111111, 4.0)]);
        // Exactly at the tolerance counts (the hit_test
        // convention: <=).
        assert_eq!(stroke_at_point(&core, [50.0, 13.0], 13.0), Some(0));
        assert_eq!(stroke_at_point(&core, [50.0, 10.0], PICK_TOLERANCE), Some(0));
        // Just outside a smaller / the same tolerance.
        assert_eq!(stroke_at_point(&core, [50.0, 10.0], 9.0), None);
        assert_eq!(stroke_at_point(&core, [50.0, 13.5], 13.0), None);
    }

    #[test]
    fn pick_only_sees_the_display_frame() {
        // Same frame rule as highlight_geometry: a stroke
        // on another frame is not pickable, even dead-on.
        let core = core_with(vec![plain(1, &[[0.0, 0.0], [100.0, 0.0]], 0x111111, 4.0)]);
        assert_eq!(stroke_at_point(&core, [50.0, 0.0], PICK_TOLERANCE), None);
        let mut core = core;
        core.goto(1);
        assert_eq!(stroke_at_point(&core, [50.0, 0.0], PICK_TOLERANCE), Some(0));
    }

    #[test]
    fn pick_returns_the_project_wide_index() {
        // Index 0 sits on frame 1 (not displayed); the
        // pickable stroke keeps its project-wide index 1 —
        // the index delete_stroke_at would take.
        let core = core_with(vec![
            plain(1, &[[0.0, 0.0], [100.0, 0.0]], 0x111111, 4.0),
            plain(0, &[[0.0, 0.0], [100.0, 0.0]], 0x222222, 4.0),
        ]);
        assert_eq!(stroke_at_point(&core, [50.0, 2.0], PICK_TOLERANCE), Some(1));
    }

    #[test]
    fn pick_nearer_stroke_wins_over_lower_index() {
        // Index 0 runs along y = 10, index 1 along y = 0;
        // the click at y = 3 is 7 from index 0, 3 from
        // index 1 — nearer beats earlier.
        let core = core_with(vec![
            plain(0, &[[0.0, 10.0], [100.0, 10.0]], 0x111111, 4.0),
            plain(0, &[[0.0, 0.0], [100.0, 0.0]], 0x222222, 4.0),
        ]);
        assert_eq!(stroke_at_point(&core, [50.0, 3.0], PICK_TOLERANCE), Some(1));
    }

    #[test]
    fn pick_equal_distance_keeps_the_lowest_index() {
        // The click at y = 5 is exactly 5 from both
        // strokes; the tie resolves deterministically to
        // the lowest project-wide index.
        let core = core_with(vec![
            plain(0, &[[0.0, 10.0], [100.0, 10.0]], 0x111111, 4.0),
            plain(0, &[[0.0, 0.0], [100.0, 0.0]], 0x222222, 4.0),
        ]);
        assert_eq!(stroke_at_point(&core, [50.0, 5.0], PICK_TOLERANCE), Some(0));
    }

    #[test]
    fn pick_filled_shape_is_by_its_outline_not_its_interior() {
        // A square stored unclosed: (0,0) (40,0) (40,40)
        // (0,40); the left edge exists only as the closing
        // segment.
        let pts = [[0.0, 0.0], [40.0, 0.0], [40.0, 40.0], [0.0, 40.0]];
        let mut s = plain(0, &pts, 0x445566, 3.0);
        s.fill = Some(0x112233);
        let core = core_with(vec![s]);
        // The centre is 20 from every edge: not a pick,
        // even though it is inside the filled area.
        assert_eq!(stroke_at_point(&core, [20.0, 20.0], 5.0), None);
        // Just outside the bottom edge: a pick.
        assert_eq!(stroke_at_point(&core, [20.0, -4.0], 5.0), Some(0));
        // Just outside the left edge: a pick only if the
        // closing segment counts.
        assert_eq!(stroke_at_point(&core, [-4.0, 20.0], 5.0), Some(0));
    }

    #[test]
    fn pick_spray_counts_by_its_base_polyline_not_its_dots() {
        // Spray radius 30: dots land up to 30 units off
        // the path, but picking measures the path itself.
        let mut s = plain(0, &[[0.0, 0.0], [100.0, 0.0]], 0xE8862E, 6.0);
        s.spray = Some(SprayMark {
            seed: 1,
            radius: 30.0,
            opacity: 1.0,
        });
        let core = core_with(vec![s]);
        // 25 above the path: inside the spray cloud,
        // outside the pick tolerance — no pick.
        assert_eq!(stroke_at_point(&core, [50.0, 25.0], PICK_TOLERANCE), None);
        // 10 above the path: a pick.
        assert_eq!(stroke_at_point(&core, [50.0, 10.0], PICK_TOLERANCE), Some(0));
    }

    #[test]
    fn pick_single_point_stroke_is_by_that_point() {
        // Draws nothing in the base layer, but its lone
        // point is real geometry: 10 away picks, 14 misses.
        let core = core_with(vec![plain(0, &[[30.0, 30.0]], 0x111111, 4.0)]);
        assert_eq!(stroke_at_point(&core, [30.0, 40.0], PICK_TOLERANCE), Some(0));
        assert_eq!(stroke_at_point(&core, [30.0, 44.0], PICK_TOLERANCE), None);
    }
}
