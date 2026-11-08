//! Phase L, section 1 — 3D PREVIEW GEOMETRY: pure viewport-space
//! segments for the 3D panel's figure preview, so the panel can
//! draw the fig3d session figure (and a ground grid) without any
//! of the projection maths living in the view code.
//!
//! ## Source of truth — the engine (`src/fig3d.rs`)
//!
//! Nothing here re-implements 3D maths. The figure's world points
//! come from `fig3d::evaluate3` (fig3d.rs:383) and the projection
//! from `fig3d::project` (fig3d.rs:995) — the same pair the bake
//! paths use (editor.rs fig3_bake / fig3_bake_animation). The
//! grid lines come from `fig3d::grid_line_points` and the
//! no-session camera from `fig3d::preview_camera` (both Phase L
//! section 3's additions to fig3d.rs). This file only decides
//! WHICH segments exist, in which colour and width, in viewport
//! coordinates; a consumer draws the returned prims in order.
//!
//! Conventions carried from fig3d:
//! - World is Y-up, the figure faces +Z. The engine defines
//!   no ground plane — `project` fixes only the Y-up
//!   convention — so the grid (drawn by `grid_line_points`
//!   on the y = 0 plane) is OFFSET here to the session
//!   figure's lowest evaluated joint: a session starts with
//!   its hips at the origin and its feet below it, and a grid
//!   left on y = 0 would slice through the hips instead of
//!   lying under the feet. With no session there is no
//!   figure to follow, and the grid stays on the origin
//!   plane. The ground shadow follows the same offset.
//! - `project` flips Y (world Y-up → stage Y-down) and clamps
//!   points at/behind the near plane to the stage centre with
//!   `depth = fig3d::BEHIND_DEPTH` (1e6). Any segment touching a
//!   clamped endpoint is SKIPPED here rather than drawn to the
//!   centre — a clamped xy is not a real position.
//! - `Proj.scale` is pixels-per-world-unit at the point's depth;
//!   the ground shadow's radius is measured through it.
//!
//! ## Prim order (painter's order)
//!
//! Grid first, then the ground shadow, then bones, then joint
//! crosses — later prims draw over earlier ones.
//!
//! ## Colours — the palette (UI mission I-7)
//!
//! Grid, selection and shadow colours arrive in a
//! [`Fig3Palette`] built by the caller from the shell's
//! `&Theme` ([`Fig3Palette::from_theme`]), replacing the
//! dark-theme literals this file used to bake in — in the
//! light theme the old grid (`0x3A4356`) and pure-black
//! shadow painted a dark grid and a black ring on a light
//! stage. The prims still carry packed `0xRRGGBB` (the
//! consumers' contract), so theme colours with alpha are
//! composited over the theme background before packing —
//! see [`Fig3Palette::from_theme`]. [`Fig3Palette::LEGACY`]
//! (also the `Default`) keeps the pre-I-7 literals for
//! callers with no theme in hand.
//!
//! ## Honest notes
//!
//! - Segments only, by design: joints are drawn as two 3 px
//!   crossing segments and the shadow as an 8-segment ring, so a
//!   consumer needs nothing but line drawing.
//! - The shadow ring is squashed vertically (radius × 0.35) so
//!   it reads as an ellipse lying on the ground plane rather
//!   than a circle floating on the screen.
//! - Joint crosses are 1.5 px wide — the brief fixed the bone
//!   (2.5), grid (1.0) and shadow (3.0) widths only; 1.5 keeps
//!   the crosses subordinate to the bones they sit on.

use std::cell::Cell;

use rune::tok::Theme;
use rune::Color;
use stykanimate_rs::fig3d::{self, Fig3State};

use crate::rune_sec::app_theme;

/// One segment in VIEWPORT coordinates (the space `fig3d::project`
/// returns for the stage size it was given), ready to stroke.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreviewPrim {
    pub a: [f32; 2],
    pub b: [f32; 2],
    /// Packed `0xRRGGBB`, the packing the rest of the shell uses.
    pub color: u32,
    pub width: f32,
}

/// The 3D panel preview's nominal width in points; the actual
/// size is passed per call, this is the size the calibration
/// test (and the panel's layout) is written against.
pub const PREVIEW_W: f32 = 308.0;
/// The 3D panel preview's nominal height in points.
pub const PREVIEW_H: f32 = 220.0;

/// Ground-grid colour of the LEGACY palette (see
/// [`Fig3Palette`]): a muted blue-grey, visible on the dark
/// stage without competing with the figure. Themed builds
/// derive the grid from the theme's border colour instead.
pub const GRID_COLOR: u32 = 0x3A4356;
/// Grid stroke width.
pub const GRID_WIDTH: f32 = 1.0;
/// Bone stroke width.
pub const BONE_WIDTH: f32 = 2.5;
/// Joint-cross stroke width (see the module docs).
pub const JOINT_WIDTH: f32 = 1.5;
/// Half-length of a joint cross's arms, in pixels.
pub const JOINT_HALF: f32 = 3.0;
/// Selection accent of the LEGACY palette — the pre-brief
/// shell accent. Themed builds use the brief accent via
/// `app_theme::accent` (`0x6C8CFF` dark).
pub const SELECTED_COLOR: u32 = 0x6D8CFF;
/// Ground-shadow colour of the LEGACY palette (pure black).
/// Themed builds bake black at [`SHADOW_ALPHA`] over the
/// theme background instead, so the light theme's shadow is
/// a soft grey rather than a black ring.
pub const SHADOW_COLOR: u32 = 0x000000;
/// Ground-shadow stroke width.
pub const SHADOW_WIDTH: f32 = 3.0;
/// The alpha at which the ground shadow's black is laid
/// over the theme background when [`Fig3Palette::from_theme`]
/// bakes it into a packed colour: dark enough to read as a
/// shadow on either theme, soft enough not to glare on the
/// light one.
pub const SHADOW_ALPHA: f32 = 0.35;
/// Shadow ring radius in WORLD units, scaled by the projected
/// point's `Proj.scale` (pixels per world unit).
pub const SHADOW_RADIUS: f32 = 30.0;
/// Number of segments in the shadow ring.
pub const SHADOW_SEGMENTS: usize = 8;
/// Vertical squash of the shadow ring (see the module docs).
pub const SHADOW_SQUASH: f32 = 0.35;
/// Grid extent/step handed to `fig3d::grid_line_points`: lines
/// every 50 world units out to ±5 steps (±250 units).
pub const GRID_EXTENT: i32 = 5;
/// Grid spacing in world units.
pub const GRID_STEP: f32 = 50.0;

/// The drawing colours for one prim build: ground grid,
/// selected-joint accent and ground shadow, as packed
/// `0xRRGGBB` (the prims' contract). Built from the shell
/// theme by [`Fig3Palette::from_theme`] at the call sites;
/// [`Fig3Palette::LEGACY`] is the pre-I-7 literal set.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fig3Palette {
    /// Ground-grid colour.
    pub grid: u32,
    /// Selected joint's cross colour.
    pub selected: u32,
    /// Ground-shadow ring colour.
    pub shadow: u32,
}

impl Fig3Palette {
    /// The pre-I-7 literals ([`GRID_COLOR`],
    /// [`SELECTED_COLOR`], [`SHADOW_COLOR`]) — the sensible
    /// default for a caller with no theme in hand (tests,
    /// headless probes). Also the `Default`.
    pub const LEGACY: Fig3Palette = Fig3Palette {
        grid: GRID_COLOR,
        selected: SELECTED_COLOR,
        shadow: SHADOW_COLOR,
    };

    /// Build the palette from the shell theme (UI mission
    /// I-7):
    ///
    /// - **grid** — the theme's border colour
    ///   (`t.card_border`), composited over `t.bg`. Dark:
    ///   the brief border `#303846` (solid). Light: the
    ///   tinted hairline over the light background lands at
    ///   a pale blue-grey (≈ `#DEE0E6`), so the light theme
    ///   gets a light grid instead of the old dark literal.
    /// - **selected** — the brief accent for the theme's
    ///   appearance, via `app_theme::accent` (`#6C8CFF`
    ///   dark, `#4666D8` light).
    /// - **shadow** — black at [`SHADOW_ALPHA`] composited
    ///   over `t.bg`: the packed-prim contract has no alpha
    ///   channel, so the translucency is baked against the
    ///   theme's ground. Dark: a near-black a step softer
    ///   than pure black; light: a soft mid-grey.
    pub fn from_theme(t: &Theme) -> Fig3Palette {
        Fig3Palette {
            grid: pack(over(t.card_border, t.bg)),
            selected: pack(app_theme::accent(t)),
            shadow: pack(over(Color::BLACK.alpha(SHADOW_ALPHA), t.bg)),
        }
    }
}

impl Default for Fig3Palette {
    fn default() -> Self {
        Fig3Palette::LEGACY
    }
}

/// Composite `fg` over an opaque `bg`, in linear light
/// (Rune's `Color::mix` space): the result's RGB is
/// `fg·a + bg·(1−a)` per channel. The result's alpha is
/// meaningless to the caller — packing drops it.
fn over(fg: Color, bg: Color) -> Color {
    fg.mix(bg, 1.0 - fg.a)
}

/// Pack a Rune colour into `0xRRGGBB`, through its sRGB
/// hex form so the value round-trips exactly through the
/// consumers' `hex_color` (which parses sRGB hex back into
/// a linear `Color`). Alpha is dropped: the prim contract
/// is opaque, so callers composite first ([`over`]).
fn pack(c: Color) -> u32 {
    let hex = c.hex();
    u32::from_str_radix(&hex[1..7], 16).unwrap_or(0)
}

thread_local! {
    /// Whether the ground grid shows in the 3D preview. Shell
    /// view state only (like the clips panel's selection): it is
    /// deliberately NOT part of `Fig3State`, which is engine
    /// session state. Defaults to visible.
    static SHOW_GRID: Cell<bool> = const { Cell::new(true) };
}

/// Whether the ground grid currently shows in the 3D preview.
pub fn grid_visible() -> bool {
    SHOW_GRID.with(|g| g.get())
}

/// Flip the ground grid's visibility; returns the new value.
pub fn toggle_grid() -> bool {
    SHOW_GRID.with(|g| {
        let next = !g.get();
        g.set(next);
        next
    })
}

/// True when a projected point was clamped behind the camera
/// (its `xy` is the stage centre, not a real position).
fn behind(p: &fig3d::Proj) -> bool {
    p.depth >= fig3d::BEHIND_DEPTH
}

/// The ground height for an evaluated 3D figure: the lowest
/// joint's y — the figure's feet. The engine defines no
/// ground plane, so the ground follows the figure (see the
/// module docs). Non-finite joint heights are ignored; an
/// empty world, or one with no finite joint height at all,
/// puts the ground at 0.0.
///
/// This is the single source for the feet rule: the panel
/// preview ([`preview_prims`]) and `stage_fig3`'s on-stage
/// drawing both call it, so the two can never disagree
/// about where the ground is.
pub fn ground_y_of(world: &[[f32; 3]]) -> f32 {
    world
        .iter()
        .map(|p| p[1])
        .filter(|y| y.is_finite())
        .reduce(f32::min)
        .unwrap_or(0.0)
}

/// Build the preview's segments for one frame of the panel.
///
/// - `state`: the editor's fig3d session, if one is active. The
///   figure is drawn from `state.camera`; with no session the
///   camera is `fig3d::preview_camera()` and only the grid can
///   appear.
/// - `show_grid`: draw the ground grid (the panel passes
///   [`grid_visible`], but the decision stays the caller's, so
///   this function is a pure function of its arguments).
/// - `viewport`: the preview's size in points, handed to
///   `fig3d::project` as its stage — returned coordinates are in
///   this space, origin top-left, Y down.
/// - `palette`: the drawing colours (grid / selection /
///   shadow), built from the shell theme by the caller —
///   [`Fig3Palette::from_theme`]; [`Fig3Palette::LEGACY`]
///   for theme-less callers.
pub fn preview_prims(
    state: Option<&Fig3State>,
    show_grid: bool,
    viewport: [f32; 2],
    palette: Fig3Palette,
) -> Vec<PreviewPrim> {
    let fallback_cam;
    let cam = match state {
        Some(s) => &s.camera,
        None => {
            fallback_cam = fig3d::preview_camera();
            &fallback_cam
        }
    };
    // One evaluation serves both the ground offset and the
    // figure pass below. With no session there is no world
    // and the ground stays at 0.0.
    let world = state.map(|s| fig3d::evaluate3(&s.figure, &s.pose));
    let ground_y = world.as_deref().map(ground_y_of).unwrap_or(0.0);

    let mut prims = Vec::new();

    if show_grid {
        push_grid(&mut prims, cam, viewport, ground_y, palette);
    }

    if let (Some(s), Some(w)) = (state, world.as_deref()) {
        push_figure(&mut prims, s, w, cam, viewport, ground_y, palette);
    }

    // The panel's canvas does not clip its drawing to its
    // own rect (proven by the Phase L headless three-d
    // snap: the grid's long projected lines painted over
    // the shell below the panel), so every segment is cut
    // to the viewport here — the one place all preview
    // drawing passes through.
    prims
        .into_iter()
        .filter_map(|p| {
            clip_segment_to_viewport(p.a, p.b, viewport).map(|(a, b)| PreviewPrim {
                a,
                b,
                color: p.color,
                width: p.width,
            })
        })
        .collect()
}

/// Clip a 2D segment to the viewport rectangle
/// `[0, w] × [0, h]` (Liang–Barsky); `None` when the
/// segment lies wholly outside or the viewport is empty.
///
/// Also used by `stage_fig3` to clip the on-stage 3D
/// drawing to the stage rect — for the same reason the
/// preview clips here: the canvas does not clip its
/// drawing to the element rect, so any line that projects
/// outside the rect would paint over the rest of the UI.
pub fn clip_segment_to_viewport(
    a: [f32; 2],
    b: [f32; 2],
    viewport: [f32; 2],
) -> Option<([f32; 2], [f32; 2])> {
    let (w, h) = (viewport[0], viewport[1]);
    if !(w > 0.0 && h > 0.0) {
        return None;
    }
    let d = [b[0] - a[0], b[1] - a[1]];
    let mut t0 = 0.0f32;
    let mut t1 = 1.0f32;
    // One (p, q) pair per edge — left, right, top, bottom —
    // in the form p·t <= q.
    let edges = [
        (-d[0], a[0]),
        (d[0], w - a[0]),
        (-d[1], a[1]),
        (d[1], h - a[1]),
    ];
    for (p, q) in edges {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                if r > t1 {
                    return None;
                }
                if r > t0 {
                    t0 = r;
                }
            } else {
                if r < t0 {
                    return None;
                }
                if r < t1 {
                    t1 = r;
                }
            }
        }
    }
    // The final clamp absorbs float rounding at the edges,
    // so a cut endpoint lands exactly on the boundary.
    let cut = |t: f32| {
        [
            (a[0] + t * d[0]).clamp(0.0, w),
            (a[1] + t * d[1]).clamp(0.0, h),
        ]
    };
    Some((cut(t0), cut(t1)))
}

/// The ground grid: `fig3d::grid_line_points` segments,
/// lifted from their y = 0 plane onto `ground_y` (the
/// session figure's lowest joint, or 0 with no session —
/// see the module docs), projected; any segment with a
/// behind-camera endpoint is skipped whole.
fn push_grid(
    prims: &mut Vec<PreviewPrim>,
    cam: &fig3d::Camera3,
    viewport: [f32; 2],
    ground_y: f32,
    palette: Fig3Palette,
) {
    let lines = fig3d::grid_line_points(GRID_EXTENT, GRID_STEP);
    let mut points = Vec::with_capacity(lines.len() * 2);
    for (a, b) in &lines {
        points.push([a[0], ground_y, a[2]]);
        points.push([b[0], ground_y, b[2]]);
    }
    let projected = fig3d::project(&points, cam, viewport);
    for (i, _) in lines.iter().enumerate() {
        let (a, b) = (&projected[2 * i], &projected[2 * i + 1]);
        if behind(a) || behind(b) {
            continue;
        }
        prims.push(PreviewPrim {
            a: a.xy,
            b: b.xy,
            color: palette.grid,
            width: GRID_WIDTH,
        });
    }
}

/// The figure: ground shadow, bones, then joint crosses, all
/// from one `project` pass over the session pose's evaluated
/// points (evaluated once by the caller).
fn push_figure(
    prims: &mut Vec<PreviewPrim>,
    state: &Fig3State,
    world: &[[f32; 3]],
    cam: &fig3d::Camera3,
    viewport: [f32; 2],
    ground_y: f32,
    palette: Fig3Palette,
) {
    let fig = &state.figure;
    if world.is_empty() {
        return;
    }
    let projected = fig3d::project(world, cam, viewport);

    push_shadow(prims, fig, world, cam, viewport, ground_y, palette);

    // Bones: one segment per node that has a parent.
    for (i, node) in fig.nodes.iter().enumerate() {
        if let Some(p) = node.parent {
            let (a, b) = (&projected[p], &projected[i]);
            if behind(a) || behind(b) {
                continue;
            }
            prims.push(PreviewPrim {
                a: a.xy,
                b: b.xy,
                color: fig.color,
                width: BONE_WIDTH,
            });
        }
    }

    // Joint crosses: two short segments per joint, accent for
    // the selected joint, figure colour otherwise.
    for (i, p) in projected.iter().enumerate() {
        if behind(p) {
            continue;
        }
        let color = if i == state.selected {
            palette.selected
        } else {
            fig.color
        };
        let [x, y] = p.xy;
        prims.push(PreviewPrim {
            a: [x - JOINT_HALF, y],
            b: [x + JOINT_HALF, y],
            color,
            width: JOINT_WIDTH,
        });
        prims.push(PreviewPrim {
            a: [x, y - JOINT_HALF],
            b: [x, y + JOINT_HALF],
            color,
            width: JOINT_WIDTH,
        });
    }
}

/// The ground shadow: an 8-segment ring centred on the point
/// of the ground plane (at `ground_y`, the figure's lowest
/// joint — see the module docs) directly under the figure's
/// root (the node with no parent), radius `SHADOW_RADIUS`
/// world units measured through that point's projected scale.
/// Skipped when the root or its ground point is behind the
/// camera.
fn push_shadow(
    prims: &mut Vec<PreviewPrim>,
    fig: &fig3d::Figure3,
    world: &[[f32; 3]],
    cam: &fig3d::Camera3,
    viewport: [f32; 2],
    ground_y: f32,
    palette: Fig3Palette,
) {
    let Some(root) = fig.nodes.iter().position(|n| n.parent.is_none()) else {
        return;
    };
    let r = world[root];
    let ground = fig3d::project(&[[r[0], ground_y, r[2]]], cam, viewport);
    let centre = ground[0];
    if behind(&centre) {
        return;
    }
    let rx = SHADOW_RADIUS * centre.scale;
    let ry = rx * SHADOW_SQUASH;
    let mut ring = Vec::with_capacity(SHADOW_SEGMENTS);
    for k in 0..SHADOW_SEGMENTS {
        let t = std::f32::consts::TAU * k as f32 / SHADOW_SEGMENTS as f32;
        ring.push([
            centre.xy[0] + rx * t.cos(),
            centre.xy[1] + ry * t.sin(),
        ]);
    }
    for k in 0..SHADOW_SEGMENTS {
        prims.push(PreviewPrim {
            a: ring[k],
            b: ring[(k + 1) % SHADOW_SEGMENTS],
            color: palette.shadow,
            width: SHADOW_WIDTH,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stykanimate_rs::fig3d::{Figure3, Pose3};

    const VP: [f32; 2] = [PREVIEW_W, PREVIEW_H];

    /// A session state over the rest humanoid, as `enter_3d`
    /// builds it but with the preview camera.
    fn rest_state(selected: usize) -> Fig3State {
        let figure = fig3d::humanoid3();
        let pose = Pose3::rest(&figure, [0.0, 0.0, 0.0]);
        Fig3State {
            figure,
            pose,
            camera: fig3d::preview_camera(),
            selected,
            pose_b: None,
        }
    }

    fn inside(p: [f32; 2]) -> bool {
        (0.0..=PREVIEW_W).contains(&p[0]) && (0.0..=PREVIEW_H).contains(&p[1])
    }

    #[test]
    fn rest_humanoid_projects_inside_the_viewport() {
        // The calibration proof: with §3's preview camera, every
        // endpoint of every figure prim (bones, joint crosses,
        // shadow ring) lands inside the preview viewport.
        let state = rest_state(0);
        let prims = preview_prims(Some(&state), false, VP, Fig3Palette::LEGACY);
        assert!(!prims.is_empty(), "a rest figure draws something");
        for prim in &prims {
            assert!(inside(prim.a), "prim starts outside: {:?}", prim.a);
            assert!(inside(prim.b), "prim ends outside: {:?}", prim.b);
        }
        // Composition: one bone per parented node, two cross
        // segments per joint, one shadow ring.
        let bones = state
            .figure
            .nodes
            .iter()
            .filter(|n| n.parent.is_some())
            .count();
        let joints = state.figure.nodes.len();
        assert_eq!(prims.len(), bones + 2 * joints + SHADOW_SEGMENTS);
    }

    #[test]
    fn grid_prims_exist_when_enabled_and_vanish_when_disabled() {
        let on = preview_prims(None, true, VP, Fig3Palette::LEGACY);
        assert!(!on.is_empty(), "the grid draws with no session");
        assert!(on
            .iter()
            .all(|p| p.color == GRID_COLOR && p.width == GRID_WIDTH));
        let off = preview_prims(None, false, VP, Fig3Palette::LEGACY);
        assert!(off.is_empty(), "no session + no grid = no prims");
    }

    #[test]
    fn none_state_yields_grid_only() {
        let prims = preview_prims(None, true, VP, Fig3Palette::LEGACY);
        assert!(prims.iter().all(|p| p.color == GRID_COLOR));
        // With a session the grid comes first, then figure work.
        // The session's grid sits at the figure's feet rather
        // than at y = 0, so its surviving segment count can
        // differ from the no-session grid's — what is pinned
        // here is the order: a solid leading run of grid
        // prims, then exactly the figure-only prims.
        let state = rest_state(0);
        let both = preview_prims(Some(&state), true, VP, Fig3Palette::LEGACY);
        let fig_only = preview_prims(Some(&state), false, VP, Fig3Palette::LEGACY);
        let grid_run = both.iter().take_while(|p| p.color == GRID_COLOR).count();
        assert!(grid_run > 0, "the grid leads the prim list");
        assert_eq!(both.len(), grid_run + fig_only.len());
    }

    #[test]
    fn selected_joint_uses_the_accent_colour() {
        let state = rest_state(2);
        let prims = preview_prims(Some(&state), false, VP, Fig3Palette::LEGACY);
        let accent: Vec<&PreviewPrim> =
            prims.iter().filter(|p| p.color == SELECTED_COLOR).collect();
        assert_eq!(accent.len(), 2, "exactly the selected joint's cross");
        // The cross is centred on the head's projected position.
        let world = fig3d::evaluate3(&state.figure, &state.pose);
        let head = fig3d::project(&world, &state.camera, VP)[2];
        for prim in accent {
            let mid = [
                (prim.a[0] + prim.b[0]) / 2.0,
                (prim.a[1] + prim.b[1]) / 2.0,
            ];
            assert!((mid[0] - head.xy[0]).abs() < 1e-3);
            assert!((mid[1] - head.xy[1]).abs() < 1e-3);
        }
        // No other joint borrows the accent (the humanoid's own
        // colour is not the accent).
        assert_ne!(state.figure.color, SELECTED_COLOR);
    }

    #[test]
    fn bones_use_the_figure_colour_and_width() {
        let state = rest_state(0);
        let prims = preview_prims(Some(&state), false, VP, Fig3Palette::LEGACY);
        let bones: Vec<&PreviewPrim> = prims
            .iter()
            .filter(|p| p.width == BONE_WIDTH)
            .collect();
        let expected = state
            .figure
            .nodes
            .iter()
            .filter(|n| n.parent.is_some())
            .count();
        assert_eq!(bones.len(), expected);
        assert!(bones.iter().all(|p| p.color == state.figure.color));
    }

    #[test]
    fn shadow_ring_sits_under_the_root() {
        let state = rest_state(0);
        let prims = preview_prims(Some(&state), false, VP, Fig3Palette::LEGACY);
        let ring: Vec<&PreviewPrim> =
            prims.iter().filter(|p| p.color == SHADOW_COLOR).collect();
        assert_eq!(ring.len(), SHADOW_SEGMENTS);
        assert!(ring.iter().all(|p| p.width == SHADOW_WIDTH));
        // The ring's centroid matches the projected ground point
        // under the root: the root's x/z (the origin at rest) at
        // the figure's lowest joint (its feet, below the origin).
        let world = fig3d::evaluate3(&state.figure, &state.pose);
        let feet = world
            .iter()
            .map(|p| p[1])
            .fold(f32::INFINITY, f32::min);
        let ground = fig3d::project(&[[0.0, feet, 0.0]], &state.camera, VP)[0];
        let cx = ring.iter().map(|p| p.a[0]).sum::<f32>() / SHADOW_SEGMENTS as f32;
        let cy = ring.iter().map(|p| p.a[1]).sum::<f32>() / SHADOW_SEGMENTS as f32;
        assert!((cx - ground.xy[0]).abs() < 1e-3);
        assert!((cy - ground.xy[1]).abs() < 1e-3);
    }

    #[test]
    fn figure_behind_the_camera_draws_nothing() {
        // Root far on the camera's side of the target: every
        // point clamps behind, so bones, joints and the shadow
        // are all skipped (the grid is a separate concern).
        let figure: Figure3 = fig3d::humanoid3();
        let pose = Pose3::rest(&figure, [0.0, 0.0, 10_000.0]);
        let state = Fig3State {
            figure,
            pose,
            camera: fig3d::preview_camera(),
            selected: 0,
            pose_b: None,
        };
        let prims = preview_prims(Some(&state), false, VP, Fig3Palette::LEGACY);
        assert!(prims.is_empty());
    }

    #[test]
    fn grid_follows_the_figures_feet_with_a_session() {
        // The engine defines no ground plane; with a session
        // the grid sits at the figure's lowest joint (its feet,
        // below the origin at rest), so on screen (Y down) the
        // grid reaches LOWER than the empty world's y = 0 grid.
        let state = rest_state(0);
        let world = fig3d::evaluate3(&state.figure, &state.pose);
        let feet = world.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
        assert!(feet < 0.0, "the rest humanoid's feet are below the origin");
        let bare = preview_prims(None, true, VP, Fig3Palette::LEGACY);
        let with = preview_prims(Some(&state), true, VP, Fig3Palette::LEGACY);
        // The whole grid sits lower on screen (Y down), so
        // its TOPMOST line is lower too. The topmost extent
        // is the statistic that survives viewport clipping:
        // both grids' lowest lines run off the canvas and
        // are cut at its bottom edge, so a max-y comparison
        // would read the clip bound on both sides.
        let grid_min_y = |prims: &[PreviewPrim]| {
            prims
                .iter()
                .filter(|p| p.color == GRID_COLOR)
                .flat_map(|p| [p.a[1], p.b[1]])
                .fold(f32::INFINITY, f32::min)
        };
        assert!(grid_min_y(&with) > grid_min_y(&bare));
        assert_ne!(with, bare);
    }

    #[test]
    fn grid_prims_stay_inside_the_viewport() {
        // Regression for the bleed in the Phase L headless
        // three-d snap: the canvas does not clip, so the
        // grid's projected lines — hundreds of points long —
        // painted over the shell below the panel. Every
        // grid endpoint now lands inside the viewport, with
        // and without a session.
        for prims in [
            preview_prims(None, true, VP, Fig3Palette::LEGACY),
            preview_prims(Some(&rest_state(0)), true, VP, Fig3Palette::LEGACY),
        ] {
            assert!(!prims.is_empty());
            for prim in &prims {
                assert!(inside(prim.a), "prim starts outside: {:?}", prim.a);
                assert!(inside(prim.b), "prim ends outside: {:?}", prim.b);
            }
        }
    }

    #[test]
    fn grid_visibility_defaults_on_and_toggles() {
        assert!(grid_visible(), "the grid starts visible");
        assert!(!toggle_grid());
        assert!(!grid_visible());
        assert!(toggle_grid());
        assert!(grid_visible());
    }

    #[test]
    fn ground_y_of_empty_world_is_zero() {
        // No joints at all: the ground stays at the origin
        // height, the same fallback the no-session preview
        // uses.
        assert_eq!(ground_y_of(&[]), 0.0);
    }

    #[test]
    fn ground_y_of_takes_the_lowest_joint() {
        // The feet rule: the minimum y over the evaluated
        // joints — x and z play no part.
        let world = [
            [10.0, 58.0, 0.0],
            [-4.0, -70.0, 12.0],
            [0.0, 0.0, -30.0],
        ];
        assert_eq!(ground_y_of(&world), -70.0);
        // A figure entirely above the origin grounds at its
        // own lowest joint, not at 0.
        let world = [[0.0, 12.5, 0.0], [0.0, 40.0, 0.0]];
        assert_eq!(ground_y_of(&world), 12.5);
    }

    #[test]
    fn ground_y_of_ignores_non_finite_joints() {
        // NaN and both infinities are skipped, not folded
        // into the minimum: the finite joints decide.
        let world = [
            [0.0, f32::NAN, 0.0],
            [0.0, f32::INFINITY, 0.0],
            [0.0, f32::NEG_INFINITY, 0.0],
            [0.0, -42.0, 0.0],
            [0.0, 10.0, 0.0],
        ];
        assert_eq!(ground_y_of(&world), -42.0);
    }

    #[test]
    fn ground_y_of_all_non_finite_is_zero() {
        // Nothing finite to stand on: the 0.0 fallback.
        let world = [
            [0.0, f32::NAN, 0.0],
            [0.0, f32::INFINITY, 0.0],
            [0.0, f32::NEG_INFINITY, 0.0],
        ];
        assert_eq!(ground_y_of(&world), 0.0);
    }

    #[test]
    fn preview_prims_with_a_session_still_places_grid_prims() {
        // Sanity that the ground_y_of extraction changed no
        // behaviour: a session with the grid on still leads
        // with grid prims, exactly as before the extraction.
        let state = rest_state(0);
        let prims = preview_prims(Some(&state), true, VP, Fig3Palette::LEGACY);
        assert!(
            prims.iter().any(|p| p.color == GRID_COLOR),
            "the session preview still draws its ground grid"
        );
        assert_eq!(prims[0].color, GRID_COLOR, "the grid still leads");
    }

    #[test]
    fn legacy_palette_is_the_pre_theme_literals() {
        // The default stays the historic look, so a caller
        // with no theme in hand reproduces the pre-I-7
        // drawing exactly.
        let p = Fig3Palette::LEGACY;
        assert_eq!(p.grid, GRID_COLOR);
        assert_eq!(p.selected, SELECTED_COLOR);
        assert_eq!(p.shadow, SHADOW_COLOR);
        assert_eq!(Fig3Palette::default(), Fig3Palette::LEGACY);
    }

    #[test]
    fn dark_theme_palette_uses_the_brief_border_and_accent() {
        let p = Fig3Palette::from_theme(&app_theme::app_theme(true));
        // The dark card_border is the brief border, solid,
        // so the grid packs to it exactly.
        assert_eq!(p.grid, 0x303846);
        // The brief accent, not the legacy 0x6D8CFF.
        assert_eq!(p.selected, 0x6C8CFF);
        // Black at SHADOW_ALPHA over the dark background: a
        // near-black a step softer than the legacy pure
        // black, not pure black itself.
        assert_ne!(p.shadow, SHADOW_COLOR);
        assert!(
            p.shadow < 0x20_2020,
            "dark shadow stays near-black: {:#08x}",
            p.shadow
        );
    }

    #[test]
    fn light_theme_palette_is_light() {
        let p = Fig3Palette::from_theme(&app_theme::app_theme(false));
        // The light accent is the brief hue darkened for
        // the white card (app_theme's pinned value).
        assert_eq!(p.selected, 0x4666D8);
        // The grid is a pale blue-grey now, not the dark
        // legacy literal: every channel is bright.
        assert_ne!(p.grid, GRID_COLOR);
        for ch in [
            (p.grid >> 16) & 0xFF,
            (p.grid >> 8) & 0xFF,
            p.grid & 0xFF,
        ] {
            assert!(ch > 0xB0, "light grid channel too dark: {:#08x}", p.grid);
        }
        // The shadow is a soft mid-grey: neither the legacy
        // pure black nor as bright as the grid above it.
        assert_ne!(p.shadow, SHADOW_COLOR);
        let sr = (p.shadow >> 16) & 0xFF;
        assert!(
            (0x90..0xE0).contains(&sr),
            "light shadow is not a soft grey: {:#08x}",
            p.shadow
        );
    }

    #[test]
    fn themed_palette_flows_into_the_prims() {
        // The I-7 proof at this layer: with a themed palette
        // the grid, the selected joint's cross and the
        // shadow ring all carry the palette's colours, and
        // no prim carries a legacy literal any more (the
        // humanoid's own colour, 0xE8E8EC, is none of them).
        let palette = Fig3Palette::from_theme(&app_theme::app_theme(true));
        let state = rest_state(2);
        let prims = preview_prims(Some(&state), true, VP, palette);
        assert!(prims.iter().any(|p| p.color == palette.grid));
        let accent: Vec<&PreviewPrim> =
            prims.iter().filter(|p| p.color == palette.selected).collect();
        assert_eq!(accent.len(), 2, "exactly the selected joint's cross");
        assert!(prims
            .iter()
            .any(|p| p.color == palette.shadow && p.width == SHADOW_WIDTH));
        assert!(prims.iter().all(|p| {
            p.color != GRID_COLOR && p.color != SELECTED_COLOR && p.color != SHADOW_COLOR
        }));
    }
}
