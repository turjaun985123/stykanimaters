//! Stage navigator (Roadmap Wave 2, W2-04): a minimap of
//! the whole scene pinned to the stage canvas's top-left
//! corner, drawn inside [`super::stage_shape`] as a Shape
//! pass (the marquee/feedback precedent — canvas paint in
//! the existing stage element, so no new element keys
//! exist) and driven by a session in this module (the
//! RM16 path-session shape: Down records, Move follows,
//! Up settles; moves write nothing but view state).
//!
//! The map shows the WHOLE scene — the union of the
//! stage rectangle and every visible figure's drawn
//! bounding box, so a figure posed off-stage is still on
//! the map — with each visible figure's segments drawn
//! in miniature at the display frame, in the figure's
//! own colour. Over it, the viewport rectangle: the
//! stage region the canvas currently shows, mapped from
//! the element's corners through the composed display
//! transform (a true quad, so it stays honest under
//! camera rotation). A press inside the map jumps the
//! view — the stage point under the press becomes the
//! element's centre, at the current view zoom — and a
//! drag keeps jumping, so the user scrubs the viewport
//! around the scene. The jump writes the shell
//! [`super::ViewNav`] state only: no project data, no
//! undo step, exactly like the wheel zoom it rides
//! beside.
//!
//! Session state is thread-local, the `VIEW_NAV`
//! pattern. The shell routing that feeds the session
//! lives in main.rs (IA-W2-04-2): the press must be
//! consulted FIRST in the stage drag's Down phase,
//! because the map is chrome over the canvas — a press
//! on it is never a pose/marquee/draw press.

use std::cell::Cell;

use rune::{Color, LineCap, Shape};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::V2;

use super::{display_transform, feedback_positions, figure_stage_bbox, hex_color};
use super::{set_view_nav, view_nav, ViewNav};

/// Navigator plate width in element points.
pub const NAV_W: f32 = 176.0;
/// Navigator plate height in element points.
pub const NAV_H: f32 = 132.0;
/// Gap between the plate and the canvas element's edges.
pub const NAV_MARGIN: f32 = 12.0;
/// Padding inside the plate around the mapped scene.
pub const NAV_PAD: f32 = 6.0;

/// The plate's rectangle in element points,
/// `[min_x, min_y, max_x, max_y]`, pinned to the canvas's
/// top-left corner — the one corner no other stage
/// chrome claims (toasts dock top-right, the camera
/// pill bottom-right, RM15's coordinate readout stands
/// at the bottom-left). `None` when the element is too
/// small to spare the plate: below the threshold the
/// navigator hides rather than crowding the stage.
pub fn nav_rect(el_size: [f32; 2]) -> Option<[f32; 4]> {
    if el_size[0] < NAV_W + 2.0 * NAV_MARGIN + 60.0
        || el_size[1] < NAV_H + 2.0 * NAV_MARGIN + 60.0
    {
        return None;
    }
    Some([
        NAV_MARGIN,
        NAV_MARGIN,
        NAV_MARGIN + NAV_W,
        NAV_MARGIN + NAV_H,
    ])
}

/// Whether element point `local` lands on the plate.
pub fn nav_contains(local: [f32; 2], el_size: [f32; 2]) -> bool {
    match nav_rect(el_size) {
        Some(r) => local[0] >= r[0] && local[0] <= r[2] && local[1] >= r[1] && local[1] <= r[3],
        None => false,
    }
}

/// The scene the map must contain, as a stage-space
/// rectangle `[min_x, min_y, max_x, max_y]`: the stage
/// rectangle unioned with every visible figure's drawn
/// bounding box (setup-aware, through
/// [`figure_stage_bbox`] — the very boxes W2-01's
/// marquee outlines), so off-stage content is on the
/// map too. A project with a degenerate stage and no
/// visible figures yields the stage rect as authored
/// (possibly empty); [`nav_map`] refuses a zero-area
/// bounds rather than dividing by it.
pub fn scene_bounds(core: &EditorCore) -> [f32; 4] {
    let stage = core.project.stage;
    let mut rect = [0.0, 0.0, stage[0].max(0.0), stage[1].max(0.0)];
    for fi in 0..core.project.figures.len() {
        if let Some(bb) = figure_stage_bbox(core, fi) {
            rect[0] = rect[0].min(bb[0]);
            rect[1] = rect[1].min(bb[1]);
            rect[2] = rect[2].max(bb[2]);
            rect[3] = rect[3].max(bb[3]);
        }
    }
    rect
}

/// The stage→map mapping for one draw/press: the plate
/// rect, the scene bounds it contains, and the uniform
/// scale + origin that fit the bounds inside the plate's
/// padding, centred on the slack axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NavMap {
    /// Plate rectangle in element points.
    pub rect: [f32; 4],
    /// Scene bounds in stage coordinates.
    pub bounds: [f32; 4],
    /// Element points per stage unit on the map.
    pub scale: f32,
    /// Element point the bounds' min corner maps to.
    pub origin: [f32; 2],
}

impl NavMap {
    /// Stage point → element point on the map.
    pub fn to_screen(&self, p: V2) -> [f32; 2] {
        [
            self.origin[0] + (p[0] - self.bounds[0]) * self.scale,
            self.origin[1] + (p[1] - self.bounds[1]) * self.scale,
        ]
    }

    /// Element point on the map → stage point (exact
    /// inverse of [`NavMap::to_screen`]). Points outside
    /// the plate still convert — a drag that leaves the
    /// plate keeps scrubbing, clamped by the caller to
    /// the scene bounds if it wants a clamp; the jump
    /// itself is meaningful anywhere.
    pub fn to_stage(&self, local: [f32; 2]) -> V2 {
        [
            self.bounds[0] + (local[0] - self.origin[0]) / self.scale,
            self.bounds[1] + (local[1] - self.origin[1]) / self.scale,
        ]
    }
}

/// The map for the current scene and element size, or
/// `None` when the navigator is hidden (small element)
/// or the scene bounds have no area to map.
pub fn nav_map(core: &EditorCore, el_size: [f32; 2]) -> Option<NavMap> {
    let rect = nav_rect(el_size)?;
    let bounds = scene_bounds(core);
    let bw = bounds[2] - bounds[0];
    let bh = bounds[3] - bounds[1];
    if bw <= 0.0 || bh <= 0.0 {
        return None;
    }
    let inner_w = (rect[2] - rect[0] - 2.0 * NAV_PAD).max(1.0);
    let inner_h = (rect[3] - rect[1] - 2.0 * NAV_PAD).max(1.0);
    let scale = (inner_w / bw).min(inner_h / bh);
    let origin = [
        rect[0] + NAV_PAD + (inner_w - bw * scale) / 2.0,
        rect[1] + NAV_PAD + (inner_h - bh * scale) / 2.0,
    ];
    Some(NavMap {
        rect,
        bounds,
        scale,
        origin,
    })
}

/// The stage region the canvas currently shows, as a
/// four-corner quad in stage coordinates (element
/// corners run backwards through the composed display
/// transform). A quad, not an AABB: under a rotated
/// camera the visible region is a rotated rectangle,
/// and the map draws it as it is.
pub fn viewport_quad(core: &EditorCore, el_size: [f32; 2]) -> [[f32; 2]; 4] {
    let rt = display_transform(core, el_size);
    [
        rt.to_stage([0.0, 0.0]),
        rt.to_stage([el_size[0], 0.0]),
        rt.to_stage([el_size[0], el_size[1]]),
        rt.to_stage([0.0, el_size[1]]),
    ]
}

/// Centre the VIEW on stage point `p` at the current
/// view zoom, with the element size explicit — the
/// [`super::view_centre_on`] formula without its
/// last-drawn-size dependency, because a press knows
/// its own element size exactly. View state only.
fn centre_view_on(core: &EditorCore, el_size: [f32; 2], p: [f32; 2]) {
    let base = crate::rune_sec::stage_camera::RotTransform::for_display(core, el_size);
    let b = base.to_screen(p);
    let centre = [el_size[0] / 2.0, el_size[1] / 2.0];
    let nav = view_nav();
    set_view_nav(ViewNav {
        zoom: nav.zoom,
        pan: [
            nav.zoom * (centre[0] - b[0]),
            nav.zoom * (centre[1] - b[1]),
        ],
    });
}

/// The jump a press at element point `local` performs:
/// the stage point under the press, clamped to the
/// scene bounds (a press in the plate's padding, or a
/// drag that ran off the mapped scene, still lands on
/// the nearest scene edge rather than flying past it),
/// becomes the view's centre.
fn jump_to(core: &EditorCore, map: &NavMap, local: [f32; 2], el_size: [f32; 2]) {
    let p = map.to_stage(local);
    let p = [
        p[0].clamp(map.bounds[0], map.bounds[2]),
        p[1].clamp(map.bounds[1], map.bounds[3]),
    ];
    centre_view_on(core, el_size, p);
}

thread_local! {
    /// A navigator drag in flight (press inside the
    /// plate, release not yet seen). Session-only, like
    /// every stage gesture record.
    static NAV_DRAG: Cell<bool> = const { Cell::new(false) };
}

/// Whether a navigator drag owns the gesture right now.
pub fn nav_active() -> bool {
    NAV_DRAG.with(|c| c.get())
}

/// Stage drag, Down phase for the navigator: a press
/// inside the plate starts a navigator drag and jumps
/// the view to the pressed scene point. Returns true
/// when the press was the navigator's — the shell
/// routes it before every content gesture (see the
/// module docs). No mode guards: the map is chrome,
/// and jumping the view is as available in draw,
/// camera, setup and playback as the wheel zoom is.
pub fn nav_down(core: &EditorCore, local: [f32; 2], el_size: [f32; 2]) -> bool {
    NAV_DRAG.with(|c| c.set(false));
    let Some(map) = nav_map(core, el_size) else {
        return false;
    };
    if !nav_contains(local, el_size) {
        return false;
    }
    NAV_DRAG.with(|c| c.set(true));
    jump_to(core, &map, local, el_size);
    true
}

/// Stage drag, Move phase for the navigator: while a
/// navigator drag is in flight, the view keeps
/// centring on the scene point under the pointer.
/// Writes view state only. Returns true when the
/// session owns the gesture.
pub fn nav_move(core: &EditorCore, local: [f32; 2], el_size: [f32; 2]) -> bool {
    if !nav_active() {
        return false;
    }
    if let Some(map) = nav_map(core, el_size) {
        jump_to(core, &map, local, el_size);
    }
    true
}

/// Stage drag, Up phase for the navigator: end the
/// drag. A cancelled gesture (Escape, focus loss)
/// simply stops — the view stays where the last jump
/// put it. That is a deliberate difference from the
/// project gestures' rollback rule: view state is not
/// project data, has no undo stack to restore through,
/// and the wheel zoom a cancel could interrupt has
/// never restored either. Returns true when a drag
/// was in flight.
pub fn nav_up(_cancelled: bool) -> bool {
    let was = nav_active();
    NAV_DRAG.with(|c| c.set(false));
    was
}

/// The navigator pass, called by [`super::stage_shape`]
/// in its tail (after the marquee pass, before RM15's
/// feedback pass — the HUD annotates over the map, and
/// the map never covers the HUD). Draws the plate, the
/// scene's miniature (stage outline + every visible
/// figure's segments in its own colour, the selected
/// figure at full strength) and the viewport quad in
/// the selection accent. With the navigator hidden
/// (small element / empty scene) it draws nothing at
/// all.
pub fn draw_navigator(mut shape: Shape, core: &EditorCore, el_size: [f32; 2]) -> Shape {
    let Some(map) = nav_map(core, el_size) else {
        return shape;
    };
    let theme = crate::rune_sec::app_theme::app_theme(!core.light_theme);
    let rect = map.rect;
    let corners = [
        [rect[0], rect[1]],
        [rect[2], rect[1]],
        [rect[2], rect[3]],
        [rect[0], rect[3]],
    ];
    // The plate: a theme card at near-full strength over
    // the stage, with the card border for an edge.
    shape = shape.fill(corners.to_vec(), theme.card.alpha(0.88));
    for (a, b) in [
        (corners[0], corners[1]),
        (corners[1], corners[2]),
        (corners[2], corners[3]),
        (corners[3], corners[0]),
    ] {
        shape = shape.line(a, b, 1.0, LineCap::Round, theme.card_border);
    }
    // The stage rectangle inside the map: the frame the
    // scene is authored in, in the border ink.
    let stage = core.project.stage;
    if stage[0] > 0.0 && stage[1] > 0.0 {
        let sc = [
            map.to_screen([0.0, 0.0]),
            map.to_screen([stage[0], 0.0]),
            map.to_screen([stage[0], stage[1]]),
            map.to_screen([0.0, stage[1]]),
        ];
        shape = shape.stroke_with(sc.to_vec(), true, 1.0, LineCap::Round, theme.card_border);
    }
    // The miniature: every visible figure's segments at
    // the display frame, in the figure's own colour.
    // Hidden / solo'd-away figures are not on the map —
    // they are not on the stage either.
    let visible = stykanimate_rs::layers::visible_figures(&core.project, core.solo);
    for (fi, fig) in core.project.figures.iter().enumerate() {
        if !visible.get(fi).copied().unwrap_or(false) {
            continue;
        }
        let pos = feedback_positions(core, fi);
        let selected = core.selected_figure == Some(fi);
        let ink = hex_color(fig.color)
            .unwrap_or(Color::WHITE)
            .alpha(if selected { 1.0 } else { 0.75 });
        let width = if selected { 2.0 } else { 1.25 };
        for (ni, node) in fig.def.nodes.iter().enumerate() {
            let Some(parent) = node.parent else { continue };
            let (Some(&a), Some(&b)) = (pos.get(parent), pos.get(ni)) else {
                continue;
            };
            shape = shape.line(
                map.to_screen(a),
                map.to_screen(b),
                width,
                LineCap::Round,
                ink,
            );
        }
    }
    // The viewport quad in the selection accent: where
    // the canvas is looking, inside the whole scene.
    let accent = hex_color(super::SELECTION_ACCENT).unwrap_or(Color::WHITE);
    let quad: Vec<[f32; 2]> = viewport_quad(core, el_size)
        .iter()
        .map(|p| map.to_screen(*p))
        .collect();
    shape = shape.stroke_with(quad, true, 1.5, LineCap::Round, accent.alpha(0.95));
    shape
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stage_view::{reset_view_nav, view_nav, ViewNav};

    const EL: [f32; 2] = [1280.0, 800.0];

    fn core() -> EditorCore {
        EditorCore::new()
    }

    #[test]
    fn nav_rect_pins_to_the_top_left_and_hides_on_small_elements() {
        assert_eq!(nav_rect(EL), Some([12.0, 12.0, 188.0, 144.0]));
        assert_eq!(nav_rect([200.0, 800.0]), None);
        assert_eq!(nav_rect([1280.0, 200.0]), None);
        assert!(nav_contains([100.0, 80.0], EL));
        assert!(!nav_contains([300.0, 80.0], EL));
        assert!(!nav_contains([100.0, 80.0], [200.0, 200.0]));
    }

    #[test]
    fn scene_bounds_contain_the_stage_and_every_visible_figure() {
        let c = core();
        let b = scene_bounds(&c);
        assert!(b[0] <= 0.0 && b[1] <= 0.0, "bounds {b:?} start at/above the stage origin");
        assert!(
            b[2] >= c.project.stage[0] && b[3] >= c.project.stage[1],
            "bounds {b:?} cover the stage {:?}",
            c.project.stage
        );
    }

    #[test]
    fn nav_map_fits_the_bounds_inside_the_plate_and_roundtrips() {
        let c = core();
        let map = nav_map(&c, EL).expect("demo scene maps");
        // The mapped bounds sit inside the padded plate.
        let lo = map.to_screen([map.bounds[0], map.bounds[1]]);
        let hi = map.to_screen([map.bounds[2], map.bounds[3]]);
        assert!(lo[0] >= map.rect[0] + NAV_PAD - 0.01 && lo[1] >= map.rect[1] + NAV_PAD - 0.01);
        assert!(hi[0] <= map.rect[2] - NAV_PAD + 0.01 && hi[1] <= map.rect[3] - NAV_PAD + 0.01);
        // One axis touches the padding exactly (the fit is tight).
        let tight = (lo[0] - (map.rect[0] + NAV_PAD)).abs() < 0.01
            || (lo[1] - (map.rect[1] + NAV_PAD)).abs() < 0.01;
        assert!(tight, "fit is tight on one axis: lo {lo:?}");
        for p in [[0.0, 0.0], [480.0, 360.0], [960.0, 720.0], [-40.0, 800.0]] {
            let back = map.to_stage(map.to_screen(p));
            assert!(
                (back[0] - p[0]).abs() < 1e-3 && (back[1] - p[1]).abs() < 1e-3,
                "map roundtrip drift for {p:?}: {back:?}"
            );
        }
    }

    #[test]
    fn viewport_quad_shrinks_as_the_view_zooms_in() {
        let c = core();
        reset_view_nav();
        let w0 = {
            let q = viewport_quad(&c, EL);
            q[1][0] - q[0][0]
        };
        set_view_nav(ViewNav { zoom: 2.0, pan: [0.0, 0.0] });
        let w1 = {
            let q = viewport_quad(&c, EL);
            q[1][0] - q[0][0]
        };
        reset_view_nav();
        assert!(w0 > 0.0 && (w1 - w0 / 2.0).abs() < 0.01, "2x view zoom halves the visible width: {w0} -> {w1}");
    }

    #[test]
    fn nav_down_jumps_the_pressed_scene_point_to_the_element_centre() {
        let c = core();
        reset_view_nav();
        let map = nav_map(&c, EL).unwrap();
        // Press the map point for a scene point well off centre.
        let target = [map.bounds[0] + 40.0, map.bounds[1] + 30.0];
        let local = map.to_screen(target);
        assert!(nav_down(&c, local, EL));
        assert!(nav_active());
        // The composed display transform now puts the target at the centre.
        let screen = display_transform(&c, EL).to_screen(target);
        assert!(
            (screen[0] - EL[0] / 2.0).abs() < 0.5 && (screen[1] - EL[1] / 2.0).abs() < 0.5,
            "target {target:?} landed at {screen:?}, not the centre"
        );
        // A drag keeps the view centred on the pointer's scene point.
        let target2 = [map.bounds[2] - 40.0, map.bounds[3] - 30.0];
        assert!(nav_move(&c, map.to_screen(target2), EL));
        let screen2 = display_transform(&c, EL).to_screen(target2);
        assert!(
            (screen2[0] - EL[0] / 2.0).abs() < 0.5 && (screen2[1] - EL[1] / 2.0).abs() < 0.5,
            "drag target {target2:?} landed at {screen2:?}"
        );
        assert!(nav_up(false));
        assert!(!nav_active());
        reset_view_nav();
    }

    #[test]
    fn nav_press_outside_the_plate_and_idle_moves_are_refused() {
        let c = core();
        reset_view_nav();
        let before = view_nav();
        assert!(!nav_down(&c, [600.0, 400.0], EL));
        assert!(!nav_active());
        assert!(!nav_move(&c, [100.0, 80.0], EL));
        assert!(!nav_up(false));
        assert_eq!(view_nav(), before, "refused gestures never touch the view");
        // A hidden navigator (small element) refuses too.
        assert!(!nav_down(&c, [20.0, 20.0], [200.0, 200.0]));
        reset_view_nav();
    }

    #[test]
    fn draw_navigator_paints_the_plate_and_hides_with_the_element() {
        let c = core();
        reset_view_nav();
        let drawn = format!("{:?}", draw_navigator(Shape::new(), &c, EL));
        assert!(drawn.matches("Fill { points:").count() >= 1, "plate fill painted");
        // Strokes: the plate border (4) + the stage outline (1 closed)
        // + one per miniature segment + the viewport quad (1 closed).
        assert!(
            drawn.matches("Stroke { points:").count() >= 6,
            "border, stage outline, miniature segments and viewport quad painted"
        );
        let hidden = format!("{:?}", draw_navigator(Shape::new(), &c, [200.0, 200.0]));
        assert_eq!(hidden.matches("Fill { points:").count(), 0);
        assert_eq!(hidden.matches("Stroke { points:").count(), 0);
        reset_view_nav();
    }
}
