//! Stage geometry and `Shape` building for the Rune editor shell
//! (Phase C of the GPUI→Rune migration). Declared from `main.rs`
//! only — this is bin-crate code, not part of the engine library.
//!
//! ## Coordinate design
//!
//! The stage is drawn in the canvas element's own layout points
//! (no `Shape::view_box`: view-box stretches coordinates but keeps
//! stroke widths in points, which would break thickness scaling).
//! One [`ViewTransform`] maps stage coordinates to element points
//! and back, using the convention both previous renderers used
//! (rasterizer `export::render_frame_at`'s view closure; GPUI
//! `paint_stage`/`to_stage`):
//!
//! * the camera key's `(x, y)` is the stage point shown at the
//!   centre of the view;
//! * `fit = min(el_w / stage_w, el_h / stage_h)` letterboxes the
//!   stage into the element, and `scale = fit * zoom`;
//! * `to_screen(p) = el_centre + (p - cam) * scale`;
//! * `to_stage(local) = cam + (local - el_centre) / scale` — the
//!   exact inverse, so a pointer point converts back through the
//!   same transform the drawing used.
//!
//! Drawing reads the element size from the previous commit's
//! layout (`Cx::layout_size("stage")` in `main.rs`); drag events
//! carry the element's `w`/`h` themselves, so both paths feed the
//! same function the same inputs. Phase E applies camera
//! *rotation* too, through `stage_camera::RotTransform` (the
//! rasterizer's own convention), and draws the full stage stack:
//! guides (optional), onion skins, drawing strokes, figures,
//! FX particles, text labels, weather — the export order.

use rune::{Color, LineCap, Shape};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::{self, V2};

/// Convert a `0xRRGGBB` engine colour to a Rune colour.
pub fn hex_color(rgb: u32) -> Option<Color> {
    rune::color::hex(&format!("#{:06x}", rgb & 0xFF_FFFF))
}

/// The camera mapping between stage coordinates and canvas
/// element points (see the module docs for the convention).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewTransform {
    /// Element points per stage unit (`fit * zoom`); always > 0.
    pub scale: f32,
    /// Camera centre in stage coordinates.
    pub cam: [f32; 2],
    /// Canvas element size in points.
    pub size: [f32; 2],
}

/// Build the transform for an element of `el_size` points showing
/// `stage`-sized content under camera `(x, y, zoom)`. A degenerate
/// element or stage size falls back to a 1:1 fit so the scale can
/// never be zero (which would make [`to_stage`] divide by zero).
pub fn view_transform(el_size: [f32; 2], stage: [f32; 2], cam: (f32, f32, f32)) -> ViewTransform {
    let fit = if el_size[0] > 0.0 && el_size[1] > 0.0 && stage[0] > 0.0 && stage[1] > 0.0 {
        (el_size[0] / stage[0]).min(el_size[1] / stage[1])
    } else {
        1.0
    };
    ViewTransform {
        scale: fit * cam.2.max(0.01),
        cam: [cam.0, cam.1],
        size: el_size,
    }
}

/// Stage point → canvas element point.
pub fn to_screen(vt: &ViewTransform, p: V2) -> [f32; 2] {
    [
        vt.size[0] / 2.0 + (p[0] - vt.cam[0]) * vt.scale,
        vt.size[1] / 2.0 + (p[1] - vt.cam[1]) * vt.scale,
    ]
}

/// Canvas element point → stage point (exact inverse of
/// [`to_screen`]).
pub fn to_stage(vt: &ViewTransform, local: [f32; 2]) -> V2 {
    [
        vt.cam[0] + (local[0] - vt.size[0] / 2.0) / vt.scale,
        vt.cam[1] + (local[1] - vt.size[1] / 2.0) / vt.scale,
    ]
}

/// Points of a regular `n`-gon of radius `r` around `centre`
/// (first vertex at the top). Used for joint handles (octagons)
/// and the circle/polygon shape nodes, since `Shape` has no
/// circle primitive.
pub fn ngon(centre: [f32; 2], r: f32, n: usize) -> Vec<[f32; 2]> {
    let n = n.max(3);
    (0..n)
        .map(|i| {
            let a = i as f32 * std::f32::consts::TAU / n as f32 - std::f32::consts::FRAC_PI_2;
            [centre[0] + r * a.cos(), centre[1] + r * a.sin()]
        })
        .collect()
}

/// Onion-skin segment sets around the current frame, mirroring
/// the GPUI stage exactly: previous frames keep each figure's own
/// colour (alpha `0x4D` for the nearest, `0x26` for older), next
/// frames share one green tint (`0x7BE495`) with the same falloff.
/// Returns `(segments, colour, alpha)` entries in draw order.
fn onion_skins(core: &EditorCore) -> Vec<(Vec<(V2, V2)>, u32, f32)> {
    let mut out = Vec::new();
    if !core.onion {
        return out;
    }
    let project = &core.project;
    let segs_at = |frame: usize, fi: usize| -> Vec<(V2, V2)> {
        let pos = core.positions(frame, fi);
        let def = &project.figures[fi].def;
        def.nodes
            .iter()
            .enumerate()
            .filter_map(|(i, node)| node.parent.map(|p| (pos[p], pos[i])))
            .collect()
    };
    if core.current > 0 {
        for fi in 0..project.figures.len() {
            if project.figures[fi].hidden {
                continue;
            }
            out.push((segs_at(core.current - 1, fi), project.figures[fi].color, 0x4D as f32 / 255.0));
        }
        for back in 2..=(core.onion_frames as usize) {
            if core.current < back {
                break;
            }
            for fi in 0..project.figures.len() {
                if project.figures[fi].hidden {
                    continue;
                }
                out.push((segs_at(core.current - back, fi), project.figures[fi].color, 0x26 as f32 / 255.0));
            }
        }
    }
    for ahead in 1..=(core.onion_frames as usize) {
        let frame = core.current + ahead;
        if frame >= core.frame_count() {
            break;
        }
        let alpha = (if ahead == 1 { 0x4D } else { 0x26 }) as f32 / 255.0;
        for fi in 0..project.figures.len() {
            if project.figures[fi].hidden {
                continue;
            }
            out.push((segs_at(frame, fi), 0x7BE495, alpha));
        }
    }
    out
}

/// Convert one stage-space particle primitive (FX clip mote or
/// weather particle) into the shape being built, through the
/// rotated transform: discs become small polygons (Shape has no
/// circle primitive), lines keep a hairline-plus width.
fn particle_prim(
    shape: Shape,
    rt: &crate::rune_sec::stage_camera::RotTransform,
    s: f32,
    prim: crate::rune_sec::stage_fx::ParticlePrim,
) -> Shape {
    use crate::rune_sec::stage_fx::ParticlePrim;
    match prim {
        ParticlePrim::Disc { pos, radius, color, alpha } => {
            let c = hex_color(color).unwrap_or(Color::WHITE).alpha(alpha);
            shape.fill(ngon(rt.to_screen(pos), (radius * s).max(0.8), 10), c)
        }
        ParticlePrim::Line { a, b, color, alpha } => {
            let c = hex_color(color).unwrap_or(Color::WHITE).alpha(alpha);
            shape.line(rt.to_screen(a), rt.to_screen(b), (1.5 * s).max(1.0), LineCap::Round, c)
        }
    }
}

/// Sub-segments a gradient segment is split into on the canvas.
/// The exporters blend continuously (per pixel / SVG gradient);
/// the canvas has no gradient stroke, so it approximates with
/// this many flat-coloured pieces — enough that the steps are
/// hard to see at stage zoom, cheap enough to rebuild per frame.
const GRADIENT_STEPS: usize = 6;

/// Colour at parameter `t` along a gradient segment, in the
/// exporters' convention (export.rs `StyledSeg`, svg.rs
/// `linearGradient`): `t = 0` at the parent joint is the node's
/// own colour, `t = 1` at the node's joint is its `color2`,
/// blended per channel with `style::lerp_color` — the exact
/// function both exporters call (export.rs:102, style.rs:25).
fn gradient_color(near: u32, far: u32, t: f32) -> u32 {
    stykanimate_rs::style::lerp_color(near, far, t)
}

/// The per-piece colours for a segment from hex `near` to the
/// node's `color2`, or `None` when the node has no `color2` —
/// the caller then keeps the single flat line, unchanged.
/// Each piece is coloured at its midpoint (`t = (k + 0.5) / n`),
/// the sample that minimises the worst error against the
/// exporters' continuous blend.
fn gradient_steps(near: u32, color2: Option<u32>) -> Option<Vec<u32>> {
    let far = color2?;
    Some(
        (0..GRADIENT_STEPS)
            .map(|k| gradient_color(near, far, (k as f32 + 0.5) / GRADIENT_STEPS as f32))
            .collect(),
    )
}

/// Point at parameter `t` along the stage-space segment `a`→`b`.
fn point_at(a: V2, b: V2, t: f32) -> V2 {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

/// The whole stage as one vector `Shape` in canvas-element
/// points: onion skins (under), then every visible figure at the
/// display frame — segments in per-node colour at real thickness
/// with round caps (a node with a `color2` gradient draws as
/// [`GRADIENT_STEPS`] blended sub-segments, mirroring the
/// exporters), shape-node rings/polygons, head rings — plus
/// joint handles on the selected figure. Mirrors the GPUI stage's
/// `draw_data`/`paint_stage` decisions (opacity fades toward the
/// background; solo/hidden figures skipped via
/// `layers::visible_figures`).
pub fn stage_shape(core: &EditorCore, el_size: [f32; 2], guides: bool) -> Shape {
    let project = &core.project;
    let display = core.display_frame();
    let cam = project.camera_at(display);
    let vt = view_transform(el_size, project.stage, cam);
    let rt = crate::rune_sec::stage_camera::RotTransform::for_display(core, el_size);
    let s = vt.scale;
    let mut shape = Shape::new();

    if guides {
        let guide_color = Color::WHITE.alpha(0.22);
        for prim in crate::rune_sec::stage_camera::guides_geometry(project.stage) {
            let crate::rune_sec::stage_camera::GuidePrim::Line { a, b } = prim;
            shape = shape.line(rt.to_screen(a), rt.to_screen(b), 1.0, LineCap::Round, guide_color);
        }
        // Camera frustum + safe frames (Phase F stage_camera):
        // what an export frames, and the action/title-safe
        // insets, in the same ink as the thirds grid.
        for prim in crate::rune_sec::stage_camera::frustum_geometry(core) {
            let crate::rune_sec::stage_camera::GuidePrim::Line { a, b } = prim;
            shape = shape.line(rt.to_screen(a), rt.to_screen(b), 1.0, LineCap::Round, guide_color);
        }
        for prim in crate::rune_sec::stage_camera::safe_frame_geometry(project.stage) {
            let crate::rune_sec::stage_camera::GuidePrim::Line { a, b } = prim;
            shape = shape.line(rt.to_screen(a), rt.to_screen(b), 1.0, LineCap::Round, guide_color);
        }
    }

    for (segs, color_hex, alpha) in onion_skins(core) {
        let color = hex_color(color_hex).unwrap_or(Color::WHITE).alpha(alpha);
        for (a, b) in segs {
            shape = shape.line(rt.to_screen(a), rt.to_screen(b), 5.0 * s, LineCap::Round, color);
        }
    }

    // Drawing strokes under the figures (the GPUI canvas's
    // order): polylines at real width, filled shapes, spray dots.
    for prim in crate::rune_sec::stage_layers::stroke_geometry(core) {
        use crate::rune_sec::stage_layers::StrokePrim;
        match prim {
            StrokePrim::Polyline { pts, width, color } => {
                let screen: Vec<[f32; 2]> = pts.iter().map(|p| rt.to_screen(*p)).collect();
                shape = shape.stroke_with(
                    screen,
                    false,
                    (width * s).max(1.0),
                    LineCap::Round,
                    hex_color(color).unwrap_or(Color::WHITE),
                );
            }
            StrokePrim::Filled { pts, color } => {
                let screen: Vec<[f32; 2]> = pts.iter().map(|p| rt.to_screen(*p)).collect();
                shape = shape.fill(screen, hex_color(color).unwrap_or(Color::WHITE));
            }
            StrokePrim::Dots { pts, radius, color, opacity } => {
                let c = hex_color(color).unwrap_or(Color::WHITE).alpha(opacity);
                for p in pts {
                    shape = shape.fill(ngon(rt.to_screen(p), (radius * s).max(0.8), 8), c);
                }
            }
        }
    }

    let visible = stykanimate_rs::layers::visible_figures(project, core.solo);
    for (fi, fig) in project.figures.iter().enumerate() {
        if !visible[fi] {
            continue;
        }
        let pos = core.positions_f(display, fi);
        // Canvas fade for layer opacity (the rasterizer uses true
        // alpha for segments; the canvas blends toward the bg —
        // the GPUI stage's decision).
        let faded = stykanimate_rs::style::lerp_color(
            project.background,
            fig.color,
            fig.opacity.clamp(0.05, 1.0),
        );
        let base = hex_color(faded).unwrap_or(Color::WHITE);
        let node_color = |node: &model::NodeDef| -> Color {
            node.color
                .map(|hex| hex_color(hex).unwrap_or(base))
                .unwrap_or(base)
        };
        for (i, node) in fig.def.nodes.iter().enumerate() {
            if let Some(p) = node.parent {
                // Gradient segments (node.style.color2 set) draw
                // as GRADIENT_STEPS sub-segments blending node
                // colour → color2, parent joint → node joint —
                // the exporters' direction and lerp. The near
                // hex is exactly the colour the flat path draws
                // (node colour, else the faded figure colour), so
                // the gradient starts where the flat segment did;
                // color2 is used raw, as export.rs uses it.
                match gradient_steps(node.color.unwrap_or(faded), node.style.color2) {
                    Some(steps) => {
                        for (k, hex) in steps.iter().enumerate() {
                            let n = GRADIENT_STEPS as f32;
                            shape = shape.line(
                                rt.to_screen(point_at(pos[p], pos[i], k as f32 / n)),
                                rt.to_screen(point_at(pos[p], pos[i], (k + 1) as f32 / n)),
                                node.thickness * s,
                                LineCap::Round,
                                hex_color(*hex).unwrap_or(Color::WHITE),
                            );
                        }
                    }
                    None => {
                        shape = shape.line(
                            rt.to_screen(pos[p]),
                            rt.to_screen(pos[i]),
                            node.thickness * s,
                            LineCap::Round,
                            node_color(node),
                        );
                    }
                }
            }
        }
        // Shape segments: circle rings and regular polygons
        // centred on the node (rasterizer + GPUI parity).
        for (i, node) in fig.def.nodes.iter().enumerate() {
            let centre = rt.to_screen(pos[i]);
            let r = node.length / 2.0 * s;
            match node.shape {
                model::Shape::Circle => {
                    shape = shape.stroke_with(
                        ngon(centre, r, 24),
                        true,
                        node.thickness * s,
                        LineCap::Round,
                        node_color(node),
                    );
                }
                model::Shape::Polygon(sides) => {
                    shape = shape.fill(ngon(centre, r, sides as usize), node_color(node));
                }
                model::Shape::Line => {}
            }
        }
        // Head rings for every figure (both prior renderers draw
        // them; a figure without its head ring is a broken
        // drawing, not a scope choice).
        for (i, node) in fig.def.nodes.iter().enumerate() {
            if node.head {
                shape = shape.stroke_with(
                    ngon(rt.to_screen(pos[i]), 17.0 * s, 24),
                    true,
                    5.0 * s,
                    LineCap::Round,
                    base,
                );
            }
        }
        // Joint handles on the selected figure only (the Phase C
        // scope): red octagons, the selected joint amber and
        // larger — the GPUI stage's colours and radii.
        if fi == core.selected_figure {
            for (ni, joint) in pos.iter().enumerate() {
                let selected = core.selected_node == Some(ni);
                let r = if selected { 8.0 } else { 5.0 } * s.max(0.6);
                let color = hex_color(if selected { 0xFFC65D } else { 0xFF5D5D })
                    .unwrap_or(Color::WHITE);
                shape = shape.fill(ngon(rt.to_screen(*joint), r, 8), color);
            }
        }
    }
    // FX particles, text labels, then weather on top — the
    // rasterizer's pass order (export.rs render_frame_at).
    for prim in crate::rune_sec::stage_fx::fx_geometry(core) {
        shape = particle_prim(shape, &rt, s, prim);
    }
    for prim in crate::rune_sec::stage_text::text_geometry(core) {
        let crate::rune_sec::stage_text::TextPrim::GlyphBox { corners, color, alpha } = prim;
        let screen: Vec<[f32; 2]> = corners.iter().map(|p| rt.to_screen(*p)).collect();
        shape = shape.fill(screen, hex_color(color).unwrap_or(Color::WHITE).alpha(alpha));
    }
    for prim in crate::rune_sec::stage_fx::weather_geometry(core) {
        shape = particle_prim(shape, &rt, s, prim);
    }
    shape
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cam() -> (f32, f32, f32) {
        (400.0, 300.0, 2.0)
    }

    #[test]
    fn camera_centre_maps_to_element_centre() {
        let vt = view_transform([800.0, 600.0], [960.0, 720.0], cam());
        assert_eq!(to_screen(&vt, [400.0, 300.0]), [400.0, 300.0]);
        assert_eq!(to_stage(&vt, [400.0, 300.0]), [400.0, 300.0]);
    }

    #[test]
    fn fit_uses_the_limiting_axis() {
        // Element is wider (relative to the stage) than tall:
        // height limits, fit = 600/720.
        let vt = view_transform([1600.0, 600.0], [960.0, 720.0], (0.0, 0.0, 1.0));
        assert!((vt.scale - 600.0 / 720.0).abs() < 1e-6);
        // Zoom multiplies the fit.
        let vt = view_transform([1600.0, 600.0], [960.0, 720.0], (0.0, 0.0, 3.0));
        assert!((vt.scale - 3.0 * 600.0 / 720.0).abs() < 1e-6);
    }

    #[test]
    fn screen_stage_roundtrip_is_exact() {
        for el_size in [[800.0, 600.0], [1440.0, 900.0], [333.0, 777.0]] {
            for zoom in [0.3, 1.0, 2.5, 4.0] {
                let vt = view_transform(el_size, [960.0, 720.0], (123.0, 456.0, zoom));
                for p in [[0.0, 0.0], [960.0, 720.0], [123.4, 567.8], [-50.0, 1000.0]] {
                    let back = to_stage(&vt, to_screen(&vt, p));
                    assert!(
                        (back[0] - p[0]).abs() < 1e-3 && (back[1] - p[1]).abs() < 1e-3,
                        "roundtrip drift for {p:?} at zoom {zoom}: {back:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn degenerate_sizes_never_give_zero_scale() {
        let vt = view_transform([0.0, 0.0], [960.0, 720.0], cam());
        assert!(vt.scale > 0.0);
        let vt = view_transform([800.0, 600.0], [0.0, 0.0], cam());
        assert!(vt.scale > 0.0);
    }

    #[test]
    fn ngon_has_n_points_on_the_radius() {
        let pts = ngon([10.0, 20.0], 5.0, 8);
        assert_eq!(pts.len(), 8);
        for p in &pts {
            let d = ((p[0] - 10.0).powi(2) + (p[1] - 20.0).powi(2)).sqrt();
            assert!((d - 5.0).abs() < 1e-4, "vertex off radius: {p:?}");
        }
        // First vertex at the top.
        assert!((pts[0][0] - 10.0).abs() < 1e-4 && (pts[0][1] - 15.0).abs() < 1e-4);
        // Fewer than 3 sides clamp to a triangle.
        assert_eq!(ngon([0.0, 0.0], 1.0, 2).len(), 3);
    }

    #[test]
    fn gradient_color_endpoints_are_exact() {
        // t = 0 is the node colour, t = 1 is color2 — the
        // exporters' endpoints, reproduced exactly.
        assert_eq!(gradient_color(0xFF0000, 0x0000FF, 0.0), 0xFF0000);
        assert_eq!(gradient_color(0xFF0000, 0x0000FF, 1.0), 0x0000FF);
        assert_eq!(gradient_color(0x000000, 0xFFFFFF, 0.0), 0x000000);
        assert_eq!(gradient_color(0x000000, 0xFFFFFF, 1.0), 0xFFFFFF);
    }

    #[test]
    fn gradient_color_midpoint_matches_the_export_lerp() {
        // The same values style.rs pins for lerp_color itself:
        // 255 * 0.5 = 127.5 rounds to 128 per channel.
        assert_eq!(gradient_color(0xFF0000, 0x0000FF, 0.5), 0x800080);
        assert_eq!(gradient_color(0x000000, 0xFFFFFF, 0.5), 0x808080);
    }

    #[test]
    fn gradient_steps_passthrough_without_color2() {
        // No color2 → None → the caller keeps the flat line.
        assert_eq!(gradient_steps(0x123456, None), None);
    }

    #[test]
    fn gradient_steps_samples_six_midpoints() {
        let steps = gradient_steps(0x000000, Some(0xFFFFFF)).expect("color2 set");
        assert_eq!(steps.len(), GRADIENT_STEPS);
        assert_eq!(steps.len(), 6);
        // Piece k is coloured at t = (k + 0.5) / 6 along
        // black → white (255 * t, rounded, per channel):
        // t = 1/12 → 21, t = 5/12 → 106, t = 7/12 → 149,
        // t = 11/12 → 234.
        assert_eq!(steps[0], 0x151515);
        assert_eq!(steps[2], 0x6A6A6A);
        assert_eq!(steps[3], 0x959595);
        assert_eq!(steps[5], 0xEAEAEA);
        // Strictly brightening piece by piece.
        for w in steps.windows(2) {
            assert!(w[0] < w[1], "gradient not monotonic: {steps:?}");
        }
    }

    #[test]
    fn point_at_endpoints_and_midpoint() {
        let (a, b) = ([0.0, 0.0], [10.0, 20.0]);
        assert_eq!(point_at(a, b, 0.0), a);
        assert_eq!(point_at(a, b, 1.0), b);
        assert_eq!(point_at(a, b, 0.5), [5.0, 10.0]);
    }
}
