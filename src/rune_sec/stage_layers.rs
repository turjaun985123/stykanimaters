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
//!   prims. The highlight pass is the integrator's accent
//!   pass: it is expected to recolour every prim to the
//!   accent colour and apply translucency at conversion
//!   time (`StrokePrim::Polyline` has no alpha field; for
//!   `Dots`, multiply the carried `opacity` by the accent
//!   alpha). This module stays colour-policy-free.
//! * **Draw order (integrator)**: draw the highlight prims
//!   in the stroke layer's slot, *before* the base stroke
//!   prims, so the glow sits under the stroke it marks and
//!   the stroke itself stays crisp on top.
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
use stykanimate_rs::paint;

/// One drawable piece of the current frame's stroke layer,
/// in stage coordinates. The integrator maps `pts` through
/// `stage_view::to_screen`, multiplies `width`/`radius` by
/// the transform's `scale`, and converts `color` with
/// `stage_view::hex_color` (applying `opacity` as the
/// colour's alpha for [`StrokePrim::Dots`]).
#[derive(Clone, Debug, PartialEq)]
pub enum StrokePrim {
    /// A plain brush stroke: one polyline through `pts` at
    /// uniform `width`, round caps (both renderers' choice:
    /// `src/export.rs:61`, `src/svg.rs:90`).
    Polyline {
        pts: Vec<[f32; 2]>,
        width: f32,
        color: u32,
    },
    /// A filled closed shape: the polygon through `pts`,
    /// filled with `color` (the stroke's stored fill colour).
    Filled {
        pts: Vec<[f32; 2]>,
        color: u32,
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
        // stroke still draws as its outline).
        if let Some(fill) = stroke.fill {
            if stroke.points.len() >= 3 {
                out.push(StrokePrim::Filled {
                    pts: stroke.points.clone(),
                    color: fill,
                });
                continue;
            }
        }
        // Plain stroke: one polyline at the uniform width.
        if stroke.points.len() >= 2 {
            out.push(StrokePrim::Polyline {
                pts: stroke.points.clone(),
                width: stroke.width,
                color: stroke.color,
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
    // Filled: the expanded outline of the polygon.
    if let Some(fill) = stroke.fill {
        if stroke.points.len() >= 3 {
            return vec![StrokePrim::Polyline {
                pts: expanded_outline(&stroke.points, HIGHLIGHT_EXPAND),
                width: stroke.width + HIGHLIGHT_OUTLINE_PAD,
                color: fill,
            }];
        }
    }
    // Plain: the same polyline, wider — an underlay the
    // integrator draws translucent in the accent colour.
    if stroke.points.len() >= 2 {
        return vec![StrokePrim::Polyline {
            pts: stroke.points.clone(),
            width: stroke.width * HIGHLIGHT_WIDTH_SCALE + HIGHLIGHT_WIDTH_PAD,
            color: stroke.color,
        }];
    }
    Vec::new()
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
            }] => {
                assert_eq!(*color, 0x112233, "outline carries the fill colour");
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
}
