//! Transform gizmo for the Rune stage (Roadmap Wave 2,
//! W2-03 — DESIGN + prototype).
//!
//! Declared from `main.rs` only — bin-crate code like
//! [`crate::stage_view`], not part of the engine library.
//! Registration is IA-W2-03-1; the stage draw call is
//! IA-W2-03-2; the `stage_drag` routing is IA-W2-03-3.
//! Until those land this module is compiled by its own
//! tests and nothing on the live stage changes.
//!
//! ## The design
//!
//! RM15's remainder list named the gap (R-A2-10): the
//! stage had precise *joint* dragging but no *figure*
//! transform affordance — to move a character you grab
//! its root joint, and to turn one you re-aim every
//! limb by hand. The gizmo is the missing affordance: a
//! frame around the current transform target with two
//! handles —
//!
//! * a **move grip** at the frame's centre (a diamond):
//!   drag translates the target's roots, and
//! * a **rotate knob** on a stalk above the frame's
//!   top-centre (a ring): drag orbits the target about
//!   the frame's centre.
//!
//! The **target** is W2-01's selection model, read
//! through its doors: the marquee figure set when it
//! holds two or more figures with a stage box,
//! otherwise the core's single selected figure. The
//! gizmo ADDS no selection state of its own.
//!
//! **No scale handles, deliberately.** Figure scale
//! already has two honest owners — a pose's squash &
//! stretch (`Pose::sx`/`sy`, W2-31) and a sprite's
//! corner handles (RM22) — and a gizmo scale handle
//! would need a third semantics (stage units? pose
//! factors? per-figure or per-set?) that neither owner
//! has ruled on. The handle set is move + rotate; a
//! scale handle is a follow-up seat's decision, not
//! this prototype's smuggle.
//!
//! **The pivot is the frame's centre** (the union
//! bounding box of the target's drawn positions),
//! which is where every comparable tool puts it and
//! where a user turning a *group* expects the group to
//! turn. Turning about the root was considered and
//! rejected for the set case (a set has no single
//! root) and for consistency kept out of the single
//! case too: one pivot rule for both targets.
//!
//! ## What a rotation writes (the model's truth)
//!
//! `model::evaluate` fixes the root node's absolute
//! angle at 0 and reads every first-generation child's
//! angle as world-absolute; deeper angles are relative
//! to their parents. A rigid rotation of a figure by
//! θ about a pivot is therefore exactly:
//!
//! * `root' = pivot + R(θ)·(root − pivot)`, and
//! * `angles[child] += θ` for every child of a
//!   parentless node, each passed through the node's
//!   own [`NodeDef::clamp_angle`] — the drag
//!   discipline (`aim_node` clamps the same way), so a
//!   limited joint stops at its limit and the limb
//!   rotates partially, exactly as a hand drag would
//!   leave it. The root node's OWN angle slot is never
//!   written: `evaluate` does not read it.
//!
//! Everything else is rotation-invariant and untouched:
//! deeper relative angles, the bend channel, and the
//! pose's squash & stretch. That last one is the honest
//! edge: squash is applied by `evaluate` about the
//! root in WORLD axes, so rotating a pose whose scale
//! is not (1, 1) rotates its skeleton while the squash
//! stays world-axis-aligned — the drawn result is the
//! rotated skeleton under the same squash, not a rigid
//! rotation of the squashed drawing. At scale (1, 1) —
//! every pose that never squashed — the write above is
//! a rigid rotation of the evaluated positions exactly,
//! and a test below pins it to the digit.
//!
//! ## Gesture discipline (the stage's standing rules)
//!
//! The session mirrors W2-01's group drag and RM16's
//! path session, because they are the stage's rules:
//! Down records and writes nothing; Move tracks and
//! writes nothing (the draw pass previews the frame
//! under the live transform); Up settles — one
//! `snapshot_labeled` ("Move figure(s)" / "Rotate
//! figure(s)", so RM24's app bar names the undo) plus
//! one write pass — or spends nothing at all: a
//! cancelled gesture, a zero move, and a rotation that
//! snaps to zero are all free. Locked figures in the
//! target stay put and are named in the recap; a target
//! whose figures are ALL locked takes no snapshot and
//! says so, in W2-01's exact sentences.
//!
//! Rotation honours RM15's snap configuration: the
//! ACCUMULATED press-to-release angle snaps to the
//! configured step (15° by default) while snapping is
//! enabled — the preview shows the snapped angle, so
//! what the user sees is what settles. Move drags are
//! not snapped here: `snap_drag_point` in the shell
//! rewrites only pose-drag points, and translating a
//! set by a landmark-snapped delta is W2-01's group
//! drag question, not this seat's.
//!
//! Handle SIZES are screen-constant, RM15's founding
//! rule: layout distances are specified in element
//! points and converted through the live scale, so the
//! knob sits the same 30 pt above the frame at 0.1×
//! and at 8×, and the hit radius is the stage's own
//! 11-ish pt generosity at every zoom (drawn glyphs are
//! smaller than their hit discs, the joint handles'
//! own relationship).

use std::cell::RefCell;

use rune::{Color, LineCap, Shape};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::{self, V2};

use crate::rune_sec::stage_camera::RotTransform;
use crate::stage_view::{self, SELECTION_ACCENT};

/// Screen points between the gizmo frame's top edge and
/// the rotate knob's centre: the stalk length. Long
/// enough that the knob clears the frame's own outline
/// and the topmost joint handles at any zoom (it is
/// screen-constant), short enough to stay visibly the
/// frame's own handle.
pub const GIZMO_STALK_PX: f32 = 30.0;

/// Screen points of padding between a target's drawn
/// bounding box and the gizmo frame — W2-01's marquee
/// outline pad, so for a set the gizmo frame sits
/// exactly on the outline the marquee already draws.
pub const GIZMO_FRAME_PAD_PX: f32 = 6.0;

/// Screen-point radius a press may land from a handle's
/// centre and still grab it. Drawn glyphs are ~6–7 pt;
/// the hit disc is the generous half of the pair, the
/// joint handles' relationship (13 pt pick, smaller
/// glyph).
pub const GIZMO_HIT_PX: f32 = 11.0;

/// Which gizmo handle a press or a drag belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GizmoHandle {
    /// The centre diamond: drag translates the target.
    Move,
    /// The stalked ring: drag rotates the target about
    /// the frame's centre.
    Rotate,
}

/// A gizmo target's handle layout, in stage
/// coordinates: everything the hit test, the session
/// and the draw pass need, computed from the target's
/// drawn bounding box at the live display scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GizmoLayout {
    /// The PADDED frame the gizmo draws:
    /// `[min_x, min_y, max_x, max_y]` in stage units.
    pub frame: [f32; 4],
    /// The rotation pivot and the move grip's centre:
    /// the frame's centre.
    pub pivot: V2,
    /// The move handle's centre (== `pivot`, kept as a
    /// named field so hit/draw code reads by handle).
    pub move_at: V2,
    /// The rotate knob's centre: above the frame's
    /// top-centre by [`GIZMO_STALK_PX`] screen points.
    pub rotate_at: V2,
    /// The frame's top-centre, where the stalk meets
    /// the frame.
    pub stalk_base: V2,
}

/// The layout for an already-known target frame (a
/// drawn bounding box, stage units) at display scale
/// `scale`: the pure geometry half of [`gizmo_layout`],
/// separated so the maths is testable without a core.
/// All screen-point distances convert through `scale`
/// (floored like the marquee pad's, so a degenerate
/// scale can never divide by zero).
pub fn gizmo_layout_for_frame(bbox: [f32; 4], scale: f32) -> GizmoLayout {
    let s = scale.max(0.05);
    let pad = GIZMO_FRAME_PAD_PX / s;
    let frame = [
        bbox[0] - pad,
        bbox[1] - pad,
        bbox[2] + pad,
        bbox[3] + pad,
    ];
    let pivot = [(frame[0] + frame[2]) / 2.0, (frame[1] + frame[3]) / 2.0];
    GizmoLayout {
        frame,
        pivot,
        move_at: pivot,
        rotate_at: [pivot[0], frame[1] - GIZMO_STALK_PX / s],
        stalk_base: [pivot[0], frame[1]],
    }
}

/// The figures the gizmo would transform right now, in
/// figure order: W2-01's marquee set when two or more
/// of its members still have a stage box (a member
/// hidden after selection drops out of the FRAME; the
/// settle still writes only the figures this returns),
/// otherwise the core's selected figure when it has a
/// stage box. Empty when there is nothing to transform
/// — no gizmo is drawn and no press is consumed.
pub fn gizmo_target(core: &EditorCore) -> Vec<usize> {
    let set: Vec<usize> = stage_view::figure_set(core)
        .into_iter()
        .filter(|&fi| stage_view::figure_stage_bbox(core, fi).is_some())
        .collect();
    if set.len() >= 2 {
        return set;
    }
    if stage_view::figure_stage_bbox(core, core.selected_figure).is_some() {
        return vec![core.selected_figure];
    }
    Vec::new()
}

/// The current target's layout, or `None` when there
/// is no target: the union of the target figures' drawn
/// bounding boxes (the set's outline geometry, shared
/// with W2-01) framed at display scale `scale`.
pub fn gizmo_layout(core: &EditorCore, scale: f32) -> Option<GizmoLayout> {
    let target = gizmo_target(core);
    let mut it = target
        .iter()
        .filter_map(|&fi| stage_view::figure_stage_bbox(core, fi));
    let first = it.next()?;
    let mut bbox = first;
    for b in it {
        bbox[0] = bbox[0].min(b[0]);
        bbox[1] = bbox[1].min(b[1]);
        bbox[2] = bbox[2].max(b[2]);
        bbox[3] = bbox[3].max(b[3]);
    }
    Some(gizmo_layout_for_frame(bbox, scale))
}

/// Which handle stage point `p` grabs under `layout`
/// at display scale `scale`, if any. The rotate knob
/// wins a tie: it is the smaller, rarer target, and
/// the stalk keeps the two hit discs apart at every
/// scale anyway (30 pt of stalk against two 11 pt
/// discs) — the priority only bites for synthetic
/// layouts and degenerate frames, where the rarer
/// gesture should still win.
pub fn gizmo_hit(layout: &GizmoLayout, p: V2, scale: f32) -> Option<GizmoHandle> {
    let r = GIZMO_HIT_PX / scale.max(0.05);
    if model::dist(p, layout.rotate_at) <= r {
        return Some(GizmoHandle::Rotate);
    }
    if model::dist(p, layout.move_at) <= r {
        return Some(GizmoHandle::Move);
    }
    None
}

/// Point `p` rotated about `pivot` by `deg` degrees,
/// in the stage's own convention: the y-down rotation
/// matrix `stage_camera` documents (`[dx·cos − dy·sin,
/// dx·sin + dy·cos]`), so a positive angle turns the
/// point clockwise on screen and lowers
/// [`model::angle_of`] of the offset by exactly `deg`
/// (`angle_of` is `atan2(x, y)`, which falls as the point
/// moves right→down) — the same direction a pose's angles
/// measure in.
pub fn rotate_point(p: V2, pivot: V2, deg: f32) -> V2 {
    let (s, c) = deg.to_radians().sin_cos();
    let dx = p[0] - pivot[0];
    let dy = p[1] - pivot[1];
    [pivot[0] + dx * c - dy * s, pivot[1] + dx * s + dy * c]
}

/// The signed rotation, in degrees normalised to
/// (−180, 180], that carries the direction
/// `pivot → press` onto `pivot → at`: a drag's total
/// rotation, measured from the gesture's own start so
/// repeated Moves can never accumulate solver drift.
/// A press or release ON the pivot has no direction
/// and reads as zero.
pub fn gizmo_rotation_delta(pivot: V2, press: V2, at: V2) -> f32 {
    let a = [press[0] - pivot[0], press[1] - pivot[1]];
    let b = [at[0] - pivot[0], at[1] - pivot[1]];
    if model::len(a) <= 0.0 || model::len(b) <= 0.0 {
        return 0.0;
    }
    // The rotation that carries press onto at: `angle_of` falls as
    // `rotate_point` turns clockwise on screen, so the delta is
    // angle_of(press) minus angle_of(at) — the subtraction the
    // previous line had backwards.
    model::norm_deg(model::angle_of(a) - model::angle_of(b))
}

/// The rotation a release would settle for a raw
/// press-to-release `raw_deg`: RM15's snap applied to
/// the ACCUMULATED angle while the session snap is
/// enabled (the aim drag's step idiom, read through
/// [`stage_view::snap_config`]), the raw angle
/// untouched otherwise. Shared by the preview and the
/// settle so the drawn angle is the written angle.
pub fn gizmo_settle_rotation(raw_deg: f32) -> f32 {
    let cfg = stage_view::snap_config();
    if cfg.enabled {
        stage_view::snap_angle_deg(raw_deg, cfg.angle_step)
    } else {
        raw_deg
    }
}

// ----------------------------------------------------------------
// The gizmo drag session
// ----------------------------------------------------------------

/// One in-flight gizmo drag: the handle grabbed, the
/// gesture's press and current points, and the target
/// as it stood at Down — figures, pivot and frame
/// snapshotted so a mid-gesture selection change
/// cannot retarget the gesture (W2-01's group record
/// discipline).
#[derive(Clone)]
struct GizmoDrag {
    handle: GizmoHandle,
    press: V2,
    at: V2,
    pivot: V2,
    frame: [f32; 4],
    rotate_at: V2,
    figures: Vec<usize>,
}

thread_local! {
    /// The in-flight gizmo drag, if any.
    static GIZMO_DRAG: RefCell<Option<GizmoDrag>> = const { RefCell::new(None) };
}

/// A live gizmo drag as the draw pass needs it: the
/// press-time frame and pivot, the pointer's current
/// point, and the transform the release would settle —
/// `delta` for a move drag, `angle_deg` (already
/// snapped) for a rotate drag.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GizmoPreview {
    pub handle: GizmoHandle,
    pub frame: [f32; 4],
    pub pivot: V2,
    pub rotate_at: V2,
    pub at: V2,
    pub delta: V2,
    pub angle_deg: f32,
}

/// The live gizmo drag's preview, or `None` when no
/// gizmo drag is in flight.
pub fn gizmo_preview() -> Option<GizmoPreview> {
    GIZMO_DRAG.with(|c| {
        c.borrow().as_ref().map(|d| GizmoPreview {
            handle: d.handle,
            frame: d.frame,
            pivot: d.pivot,
            rotate_at: d.rotate_at,
            at: d.at,
            delta: match d.handle {
                GizmoHandle::Move => [d.at[0] - d.press[0], d.at[1] - d.press[1]],
                GizmoHandle::Rotate => [0.0, 0.0],
            },
            angle_deg: match d.handle {
                GizmoHandle::Rotate => {
                    gizmo_settle_rotation(gizmo_rotation_delta(d.pivot, d.press, d.at))
                }
                GizmoHandle::Move => 0.0,
            },
        })
    })
}

/// Stage drag, Down phase for the gizmo: grab a handle
/// of the current target. Returns true when the press
/// was consumed. The shell routes presses here after
/// the motion-path session and before the pose drag
/// (IA-W2-03-3): the handles are drawn OVER the figures
/// precisely so they win their own hit discs, while
/// every joint outside them keeps the pose drag. The
/// guards are the stage gestures' own — refused while
/// playing, in setup mode (rest poses are not what the
/// gizmo edits), in draw/camera mode, while a pose drag
/// owns the gesture, or while a sibling session
/// (marquee, group drag) is live. No snapshot: the
/// gesture's one undo step is the release's.
pub fn gizmo_down(core: &mut EditorCore, p: V2, scale: f32) -> bool {
    GIZMO_DRAG.with(|c| *c.borrow_mut() = None);
    if core.playing
        || core.setup_mode
        || core.draw_mode
        || core.camera_mode
        || core.dragging.is_some()
        || stage_view::marquee_live_rect().is_some()
        || stage_view::group_drag_delta().is_some()
    {
        return false;
    }
    let Some(layout) = gizmo_layout(core, scale) else {
        return false;
    };
    let Some(handle) = gizmo_hit(&layout, p, scale) else {
        return false;
    };
    // Roadmap Wave 2 (W2-03): a press this close to an evaluated joint is
    // a pose press, not a gizmo grab — the demo figure's root sits 4.48
    // stage units from the move grip (the bbox centre), inside the grip's
    // hit disc, so without this veto every press on the root joint is
    // swallowed and the joint can never be selected. The veto runs after
    // the hit test, so a press on a handle away from any joint still grabs.
    let pick = stage_view::pick_radius(scale);
    for fi in gizmo_target(core) {
        if core
            .positions(core.current, fi)
            .iter()
            .any(|j| model::dist(*j, p) <= pick)
        {
            return false;
        }
    }
    let drag = GizmoDrag {
        handle,
        press: p,
        at: p,
        pivot: layout.pivot,
        frame: layout.frame,
        rotate_at: layout.rotate_at,
        figures: gizmo_target(core),
    };
    GIZMO_DRAG.with(|c| *c.borrow_mut() = Some(drag));
    true
}

/// Stage drag, Move phase for the gizmo: track the
/// pointer for the draw pass's preview. Writes nothing
/// to the project. Returns true while a gizmo drag owns
/// the gesture; a record whose figures have all been
/// deleted under it ends the drag instead of writing
/// anywhere (W2-01's group-move discipline).
pub fn gizmo_move(core: &mut EditorCore, p: V2) -> bool {
    let alive = GIZMO_DRAG.with(|c| {
        let mut cell = c.borrow_mut();
        match cell.as_mut() {
            Some(d) if d.figures.iter().any(|&fi| fi < core.project.figures.len()) => {
                d.at = p;
                true
            }
            _ => false,
        }
    });
    if !alive {
        GIZMO_DRAG.with(|c| *c.borrow_mut() = None);
    }
    alive
}

/// Stage drag, Up phase for the gizmo: settle the
/// gesture. A cancelled gesture drops the record —
/// nothing was written, so there is nothing to restore
/// and no undo step to spend. A move release at zero
/// displacement, and a rotate release whose snapped
/// angle is zero, spend nothing either: they were
/// clicks on a handle. Otherwise the transform lands
/// as exactly ONE undo step — one `snapshot_labeled`,
/// then the write pass — over the figures that can
/// move: locked figures stay put and are counted in
/// the recap, and a target whose figures are ALL
/// locked (or gone) takes no snapshot and says so in
/// W2-01's sentence.
pub fn gizmo_up(core: &mut EditorCore, p: V2, cancelled: bool) {
    let Some(d) = GIZMO_DRAG.with(|c| c.borrow_mut().take()) else {
        return;
    };
    if cancelled {
        return;
    }
    let frame = core.current;
    let movable: Vec<usize> = d
        .figures
        .iter()
        .copied()
        .filter(|&fi| fi < core.project.figures.len() && !core.project.figures[fi].locked)
        .collect();
    let skipped = d.figures.len() - movable.len();
    match d.handle {
        GizmoHandle::Move => {
            let delta = [p[0] - d.press[0], p[1] - d.press[1]];
            if delta == [0.0, 0.0] {
                return;
            }
            if movable.is_empty() {
                core.status =
                    "Selected figures are locked — unlock them in the figures list.".into();
                return;
            }
            core.snapshot_labeled(if movable.len() == 1 {
                "Move figure"
            } else {
                "Move figures"
            });
            for fi in &movable {
                if let Some(pose) = core
                    .project
                    .frames
                    .get_mut(frame)
                    .and_then(|f| f.poses.get_mut(*fi))
                {
                    pose.root = [pose.root[0] + delta[0], pose.root[1] + delta[1]];
                }
            }
            core.status = move_recap(movable.len(), frame, skipped);
        }
        GizmoHandle::Rotate => {
            let deg = gizmo_settle_rotation(gizmo_rotation_delta(d.pivot, d.press, p));
            if deg == 0.0 {
                return;
            }
            if movable.is_empty() {
                core.status =
                    "Selected figures are locked — unlock them in the figures list.".into();
                return;
            }
            core.snapshot_labeled(if movable.len() == 1 {
                "Rotate figure"
            } else {
                "Rotate figures"
            });
            for fi in &movable {
                rotate_figure_at_frame(core, *fi, frame, d.pivot, deg);
            }
            core.status = rotate_recap(movable.len(), deg, frame, skipped);
        }
    }
}

/// Rotate one figure's pose at `frame` by `deg` about
/// `pivot`, in place: the root travels the pivot orbit
/// and every first-generation angle shifts by `deg`
/// through its node's own limit clamp — the module
/// docs carry the `evaluate` reasoning. The angle
/// shifts are computed from the definition BEFORE the
/// pose borrow, so no borrow of the project is held
/// across the write.
fn rotate_figure_at_frame(core: &mut EditorCore, fi: usize, frame: usize, pivot: V2, deg: f32) {
    let Some(fig) = core.project.figures.get(fi) else {
        return;
    };
    let roots: Vec<usize> = fig
        .def
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.parent.is_none())
        .map(|(i, _)| i)
        .collect();
    let Some(pose) = core
        .project
        .frames
        .get(frame)
        .and_then(|f| f.poses.get(fi))
    else {
        return;
    };
    let shifts: Vec<(usize, f32)> = fig
        .def
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.parent.is_some_and(|p| roots.contains(&p)))
        .filter_map(|(ni, node)| {
            pose.angles
                .get(ni)
                .map(|a| (ni, node.clamp_angle(model::norm_deg(a + deg))))
        })
        .collect();
    let new_root = rotate_point(pose.root, pivot, deg);
    if let Some(pose) = core
        .project
        .frames
        .get_mut(frame)
        .and_then(|f| f.poses.get_mut(fi))
    {
        pose.root = new_root;
        for (ni, a) in shifts {
            if let Some(slot) = pose.angles.get_mut(ni) {
                *slot = a;
            }
        }
    }
}

/// The move settle's status sentence, in W2-01's
/// grammar (singular/plural, the locked suffix).
fn move_recap(moved: usize, frame: usize, skipped: usize) -> String {
    let base = if moved == 1 {
        format!("Moved 1 figure at frame {}.", frame + 1)
    } else {
        format!("Moved {} figures at frame {}.", moved, frame + 1)
    };
    locked_suffix(base, skipped)
}

/// The rotate settle's status sentence, same grammar.
fn rotate_recap(rotated: usize, deg: f32, frame: usize, skipped: usize) -> String {
    let base = if rotated == 1 {
        format!("Rotated 1 figure by {} at frame {}.", fmt_deg(deg), frame + 1)
    } else {
        format!(
            "Rotated {} figures by {} at frame {}.",
            rotated,
            fmt_deg(deg),
            frame + 1
        )
    };
    locked_suffix(base, skipped)
}

fn locked_suffix(base: String, skipped: usize) -> String {
    match skipped {
        0 => base,
        1 => format!("{}; 1 locked figure stayed.", base.trim_end_matches('.')),
        n => format!("{}; {} locked figures stayed.", base.trim_end_matches('.'), n),
    }
}

/// A rotation for a status sentence: whole degrees
/// print whole (the snapped common case), fractions
/// keep one decimal — never a rounded lie.
fn fmt_deg(deg: f32) -> String {
    if (deg - deg.round()).abs() < 0.05 {
        format!("{}°", deg.round() as i32)
    } else {
        format!("{:.1}°", deg)
    }
}

// ----------------------------------------------------------------
// The draw pass (called from stage_shape via IA-W2-03-2)
// ----------------------------------------------------------------

/// The stage pass for the transform gizmo, called by
/// `stage_shape` after the marquee pass and before
/// RM15's feedback pass (IA-W2-03-2): the target's
/// frame, its stalk and rotate knob, and the move grip
/// at its centre, all in the selection accent. While a
/// gizmo drag is in flight the frame and handles ride
/// the drag's preview transform (translated for a move,
/// rotated about the pivot for a rotate, with a spoke
/// from the pivot to the pointer), so the user turns
/// the FRAME they will get — the figures themselves
/// settle at release, the stage's preview discipline.
/// With no target the pass adds nothing at all: the
/// idle stage is byte-for-byte the stage without it.
pub fn draw_gizmo(
    mut shape: Shape,
    core: &EditorCore,
    rt: &RotTransform,
    s: f32,
) -> Shape {
    let accent = stage_view::hex_color(SELECTION_ACCENT).unwrap_or(Color::WHITE);
    let preview = gizmo_preview();
    let (frame, pivot, rotate_at) = match &preview {
        Some(pv) => (pv.frame, pv.pivot, pv.rotate_at),
        None => match gizmo_layout(core, s) {
            Some(l) => (l.frame, l.pivot, l.rotate_at),
            None => return shape,
        },
    };
    // The preview transform: identity at rest, the
    // drag's own transform in flight.
    let xf = |q: V2| -> V2 {
        match &preview {
            Some(pv) if pv.handle == GizmoHandle::Move => {
                [q[0] + pv.delta[0], q[1] + pv.delta[1]]
            }
            Some(pv) if pv.handle == GizmoHandle::Rotate => {
                rotate_point(q, pv.pivot, pv.angle_deg)
            }
            _ => q,
        }
    };
    let corners = [
        [frame[0], frame[1]],
        [frame[2], frame[1]],
        [frame[2], frame[3]],
        [frame[0], frame[3]],
    ];
    let screen: Vec<[f32; 2]> = corners.iter().map(|&q| rt.to_screen(xf(q))).collect();
    shape = shape.stroke_with(screen, true, 1.5, LineCap::Round, accent.alpha(0.9));
    // Stalk: frame top-centre to the knob.
    let top_centre = [(frame[0] + frame[2]) / 2.0, frame[1]];
    let knob = rt.to_screen(xf(rotate_at));
    let stalk: Vec<[f32; 2]> = vec![rt.to_screen(xf(top_centre)), knob];
    shape = shape.stroke_with(stalk, false, 1.0, LineCap::Round, accent.alpha(0.7));
    // Knob: a ring with a centre dot, screen-sized.
    shape = shape.fill(stage_view::ngon(knob, 6.0, 24), accent.alpha(0.25));
    let ring = stage_view::ngon(knob, 6.0, 24);
    shape = shape.stroke_with(ring, true, 1.5, LineCap::Round, accent.alpha(0.95));
    shape = shape.fill(stage_view::ngon(knob, 1.8, 12), accent.alpha(0.95));
    // Move grip: a diamond at the (transformed) centre
    // with a white heart — the accent-on-accent problem
    // the frame itself doesn't have.
    let centre = rt.to_screen(xf(pivot));
    shape = shape.fill(stage_view::ngon(centre, 7.0, 4), accent.alpha(0.9));
    shape = shape.fill(stage_view::ngon(centre, 1.8, 12), Color::WHITE.alpha(0.95));
    // Rotate in flight: the spoke from pivot to pointer
    // makes the angle legible against the stage.
    if let Some(pv) = &preview {
        if pv.handle == GizmoHandle::Rotate {
            let spoke: Vec<[f32; 2]> = vec![rt.to_screen(pv.pivot), rt.to_screen(pv.at)];
            shape = shape.stroke_with(spoke, false, 1.0, LineCap::Round, accent.alpha(0.45));
        }
    }
    shape
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stage_view::{clear_figure_set, set_figure_set, SnapConfig};

    /// Three figures rooted far apart on every frame
    /// (W2-01's fixture, verbatim roots): fig0 [80, 90],
    /// fig1 [700, 90], fig2 [80, 600].
    fn gizmo_core() -> EditorCore {
        let mut core = EditorCore::new();
        core.add_figure();
        core.add_figure();
        let roots = [[80.0, 90.0], [700.0, 90.0], [80.0, 600.0]];
        for frame in core.project.frames.iter_mut() {
            for (fi, r) in roots.iter().enumerate() {
                frame.poses[fi].root = *r;
            }
        }
        // `add_figure` selects the new figure, so the fixture ends on
        // figure 2 — the tests below read and move figure 0, so
        // re-select it here.
        core.selected_figure = 0;
        core
    }

    fn close(a: V2, b: V2, tol: f32) -> bool {
        (a[0] - b[0]).abs() <= tol && (a[1] - b[1]).abs() <= tol
    }

    #[test]
    fn layout_geometry_is_screen_constant() {
        let bbox = [100.0, 100.0, 300.0, 260.0];
        let l1 = gizmo_layout_for_frame(bbox, 1.0);
        // Pad 6, stalk 30 at scale 1.
        assert_eq!(l1.frame, [94.0, 94.0, 306.0, 266.0]);
        assert_eq!(l1.pivot, [200.0, 180.0]);
        assert_eq!(l1.move_at, [200.0, 180.0]);
        assert_eq!(l1.stalk_base, [200.0, 94.0]);
        assert_eq!(l1.rotate_at, [200.0, 64.0]);
        // At 2x the same screen distances halve in stage
        // units: pad 3, stalk 15.
        let l2 = gizmo_layout_for_frame(bbox, 2.0);
        assert_eq!(l2.frame, [97.0, 97.0, 303.0, 263.0]);
        assert_eq!(l2.pivot, [200.0, 180.0], "centre is scale-free");
        assert_eq!(l2.rotate_at, [200.0, 82.0]);
        // At 0.5x they double: pad 12, stalk 60.
        let l05 = gizmo_layout_for_frame(bbox, 0.5);
        assert_eq!(l05.frame, [88.0, 88.0, 312.0, 272.0]);
        assert_eq!(l05.rotate_at, [200.0, 28.0]);
    }

    #[test]
    fn hit_discs_are_screen_constant_and_rotate_wins_ties() {
        let layout = gizmo_layout_for_frame([100.0, 100.0, 300.0, 260.0], 1.0);
        assert_eq!(gizmo_hit(&layout, layout.rotate_at, 1.0), Some(GizmoHandle::Rotate));
        assert_eq!(gizmo_hit(&layout, layout.move_at, 1.0), Some(GizmoHandle::Move));
        // 10 pt from the knob still grabs; 12 pt misses.
        assert_eq!(
            gizmo_hit(&layout, [layout.rotate_at[0] + 10.0, layout.rotate_at[1]], 1.0),
            Some(GizmoHandle::Rotate)
        );
        assert_eq!(
            gizmo_hit(&layout, [layout.rotate_at[0] + 12.0, layout.rotate_at[1]], 1.0),
            None
        );
        // The same screen offsets at 2x are half the
        // stage distance.
        let l2 = gizmo_layout_for_frame([100.0, 100.0, 300.0, 260.0], 2.0);
        assert_eq!(
            gizmo_hit(&l2, [l2.rotate_at[0] + 5.0, l2.rotate_at[1]], 2.0),
            Some(GizmoHandle::Rotate)
        );
        assert_eq!(
            gizmo_hit(&l2, [l2.rotate_at[0] + 6.0, l2.rotate_at[1]], 2.0),
            None
        );
        // A bare stage point far from both handles misses.
        assert_eq!(gizmo_hit(&layout, [900.0, 900.0], 1.0), None);
        // Synthetic overlap: the knob wins.
        let overlap = GizmoLayout {
            frame: [0.0, 0.0, 10.0, 10.0],
            pivot: [5.0, 5.0],
            move_at: [5.0, 5.0],
            rotate_at: [5.0, 5.0],
            stalk_base: [5.0, 0.0],
        };
        assert_eq!(gizmo_hit(&overlap, [5.0, 5.0], 1.0), Some(GizmoHandle::Rotate));
    }

    #[test]
    fn rotate_point_and_delta_follow_the_stage_convention() {
        let pivot = [100.0, 100.0];
        // A point right of the pivot turns CLOCKWISE on
        // screen (y down): +90 puts it BELOW the pivot,
        // and its angle_of rises by exactly the turn.
        let q = rotate_point([200.0, 100.0], pivot, 90.0);
        assert!(close(q, [100.0, 200.0], 0.01), "got {q:?}");
        assert!(close(rotate_point([200.0, 100.0], pivot, -90.0), [100.0, 0.0], 0.01));
        assert!(close(rotate_point([200.0, 100.0], pivot, 0.0), [200.0, 100.0], 0.001));
        assert!(close(rotate_point([200.0, 100.0], pivot, 360.0), [200.0, 100.0], 0.01));
        // Deltas measure press -> release, normalised.
        assert!((gizmo_rotation_delta(pivot, [200.0, 100.0], [100.0, 200.0]) - 90.0).abs() < 0.01);
        assert!((gizmo_rotation_delta(pivot, [200.0, 100.0], [100.0, 0.0]) + 90.0).abs() < 0.01);
        // Across the ±180 seam the delta stays small.
        let a = [100.0 + 100.0 * (-170.0f32).to_radians().cos(), 100.0 + 100.0 * (-170.0f32).to_radians().sin()];
        let b = [100.0 + 100.0 * (170.0f32).to_radians().cos(), 100.0 + 100.0 * (170.0f32).to_radians().sin()];
        assert!((gizmo_rotation_delta(pivot, a, b) + 20.0).abs() < 0.05);
        // On the pivot there is no direction.
        assert_eq!(gizmo_rotation_delta(pivot, pivot, [200.0, 100.0]), 0.0);
    }

    #[test]
    fn settle_rotation_snaps_the_accumulated_angle() {
        stage_view::set_snap_config(SnapConfig::DEFAULT);
        assert_eq!(gizmo_settle_rotation(14.0), 15.0);
        assert_eq!(gizmo_settle_rotation(7.0), 0.0);
        assert_eq!(gizmo_settle_rotation(93.0), 90.0);
        assert_eq!(gizmo_settle_rotation(-22.0), -15.0);
        stage_view::set_snap_config(SnapConfig { enabled: false, ..SnapConfig::DEFAULT });
        assert_eq!(gizmo_settle_rotation(14.0), 14.0);
        stage_view::set_snap_config(SnapConfig::DEFAULT);
    }

    #[test]
    fn target_resolution_prefers_the_set_and_falls_back_to_the_selection() {
        let mut core = gizmo_core();
        // No set: the single selected figure.
        assert_eq!(gizmo_target(&core), vec![0]);
        let l = gizmo_layout(&core, 1.0).expect("selected figure has a layout");
        let bb = stage_view::figure_stage_bbox(&core, 0).unwrap();
        assert_eq!(l.frame, [bb[0] - 6.0, bb[1] - 6.0, bb[2] + 6.0, bb[3] + 6.0]);
        // A two-figure set takes over, union frame.
        set_figure_set(&mut core, vec![0, 1]);
        assert_eq!(gizmo_target(&core), vec![0, 1]);
        let l = gizmo_layout(&core, 1.0).unwrap();
        let bb1 = stage_view::figure_stage_bbox(&core, 1).unwrap();
        assert_eq!(l.frame[0], bb[0] - 6.0);
        assert_eq!(l.frame[2], bb1[2] + 6.0);
        // A one-figure set is not a set target: the
        // selection still rules.
        set_figure_set(&mut core, vec![2]);
        assert_eq!(gizmo_target(&core), vec![0]);
        // No valid selection and no set: no gizmo.
        clear_figure_set(&mut core);
        core.selected_figure = 99;
        assert!(gizmo_target(&core).is_empty());
        assert!(gizmo_layout(&core, 1.0).is_none());
    }

    #[test]
    fn move_drag_settles_one_step_and_previews_write_nothing() {
        stage_view::set_snap_config(SnapConfig::DEFAULT);
        let mut core = gizmo_core();
        let layout = gizmo_layout(&core, 1.0).unwrap();
        let root0 = core.project.frames[0].poses[0].root;
        let depth = core.undo_depth();
        assert!(gizmo_down(&mut core, layout.move_at, 1.0));
        assert!(gizmo_move(&mut core, [layout.move_at[0] + 25.0, layout.move_at[1] - 10.0]));
        let pv = gizmo_preview().expect("drag in flight");
        assert_eq!(pv.handle, GizmoHandle::Move);
        assert_eq!(pv.delta, [25.0, -10.0]);
        // Preview writes nothing.
        assert_eq!(core.project.frames[0].poses[0].root, root0);
        gizmo_up(&mut core, [layout.move_at[0] + 25.0, layout.move_at[1] - 10.0], false);
        assert_eq!(core.undo_depth(), depth + 1, "one snapshot per gesture");
        assert_eq!(
            core.project.frames[0].poses[0].root,
            [root0[0] + 25.0, root0[1] - 10.0]
        );
        assert_eq!(core.status, "Moved 1 figure at frame 1.");
        assert!(gizmo_preview().is_none());
    }

    #[test]
    fn rotate_drag_rotates_the_evaluated_figure_rigidly() {
        stage_view::set_snap_config(SnapConfig::DEFAULT);
        let mut core = gizmo_core();
        let layout = gizmo_layout(&core, 1.0).unwrap();
        let pivot = layout.pivot;
        let before = core.positions(0, 0);
        let depth = core.undo_depth();
        assert!(gizmo_down(&mut core, layout.rotate_at, 1.0));
        // Orbit the press point a quarter turn about the
        // pivot (already on the 15-degree snap grid).
        let at = rotate_point(layout.rotate_at, pivot, 90.0);
        assert!(gizmo_move(&mut core, at));
        let pv = gizmo_preview().expect("drag in flight");
        assert_eq!(pv.handle, GizmoHandle::Rotate);
        assert!((pv.angle_deg - 90.0).abs() < 0.01);
        // Preview writes nothing.
        assert_eq!(core.positions(0, 0), before);
        gizmo_up(&mut core, at, false);
        assert_eq!(core.undo_depth(), depth + 1, "one snapshot per gesture");
        // THE pin: at pose scale (1, 1) the evaluated
        // figure is the rigid rotation of the original.
        let after = core.positions(0, 0);
        for (b, a) in before.iter().zip(after.iter()) {
            let want = rotate_point(*b, pivot, 90.0);
            assert!(close(*a, want, 0.05), "joint {b:?} -> {a:?}, want {want:?}");
        }
        assert_eq!(core.status, "Rotated 1 figure by 90° at frame 1.");
        assert!(gizmo_preview().is_none());
    }

    #[test]
    fn rotate_settles_a_set_about_the_union_centre() {
        stage_view::set_snap_config(SnapConfig::DEFAULT);
        let mut core = gizmo_core();
        set_figure_set(&mut core, vec![0, 1]);
        let layout = gizmo_layout(&core, 1.0).unwrap();
        let before0 = core.positions(0, 0);
        let before1 = core.positions(0, 1);
        let before2 = core.positions(0, 2);
        let depth = core.undo_depth();
        assert!(gizmo_down(&mut core, layout.rotate_at, 1.0));
        let at = rotate_point(layout.rotate_at, layout.pivot, 90.0);
        gizmo_up(&mut core, at, false);
        assert_eq!(core.undo_depth(), depth + 1);
        for (before, fi) in [(&before0, 0usize), (&before1, 1usize)] {
            let after = core.positions(0, fi);
            for (b, a) in before.iter().zip(after.iter()) {
                let want = rotate_point(*b, layout.pivot, 90.0);
                assert!(close(*a, want, 0.05), "figure {fi} joint {b:?} -> {a:?}");
            }
        }
        // Figure 2 is outside the set: untouched.
        assert_eq!(core.positions(0, 2), before2);
        assert_eq!(core.status, "Rotated 2 figures by 90° at frame 1.");
    }

    #[test]
    fn rotate_respects_joint_limits_like_a_drag() {
        stage_view::set_snap_config(SnapConfig::DEFAULT);
        let mut core = gizmo_core();
        // Pin figure 0's first first-generation joint to
        // a 10-degree arc; a 90-degree gizmo rotation
        // must stop there (aim_node's clamp discipline),
        // while the root still orbits fully.
        let root_idx = core.project.figures[0]
            .def
            .nodes
            .iter()
            .position(|n| n.parent.is_none())
            .expect("demo figure has a root");
        let child = core.project.figures[0]
            .def
            .nodes
            .iter()
            .position(|n| n.parent == Some(root_idx))
            .expect("demo figure has a first-generation child");
        core.project.figures[0].def.nodes[child].min_angle = Some(0.0);
        core.project.figures[0].def.nodes[child].max_angle = Some(10.0);
        // Start from inside the arc.
        for frame in core.project.frames.iter_mut() {
            frame.poses[0].angles[child] = 4.0;
        }
        let layout = gizmo_layout(&core, 1.0).unwrap();
        let root_before = core.project.frames[0].poses[0].root;
        assert!(gizmo_down(&mut core, layout.rotate_at, 1.0));
        let at = rotate_point(layout.rotate_at, layout.pivot, 90.0);
        gizmo_up(&mut core, at, false);
        let pose = &core.project.frames[0].poses[0];
        assert!(
            (pose.angles[child] - 10.0).abs() < 0.01,
            "clamped at the limit, got {}",
            pose.angles[child]
        );
        assert!(close(pose.root, rotate_point(root_before, layout.pivot, 90.0), 0.01));
    }

    #[test]
    fn locked_members_stay_and_are_named() {
        stage_view::set_snap_config(SnapConfig::DEFAULT);
        let mut core = gizmo_core();
        core.project.figures[1].locked = true;
        set_figure_set(&mut core, vec![0, 1]);
        let layout = gizmo_layout(&core, 1.0).unwrap();
        let before1 = core.positions(0, 1);
        assert!(gizmo_down(&mut core, layout.rotate_at, 1.0));
        let at = rotate_point(layout.rotate_at, layout.pivot, 90.0);
        gizmo_up(&mut core, at, false);
        assert_eq!(core.positions(0, 1), before1, "locked figure untouched");
        assert_eq!(
            core.status,
            "Rotated 1 figure by 90° at frame 1; 1 locked figure stayed."
        );
        // All locked: no snapshot, the standing sentence.
        core.project.figures[0].locked = true;
        let depth = core.undo_depth();
        let layout = gizmo_layout(&core, 1.0).unwrap();
        assert!(gizmo_down(&mut core, layout.move_at, 1.0));
        gizmo_up(&mut core, [layout.move_at[0] + 40.0, layout.move_at[1]], false);
        assert_eq!(core.undo_depth(), depth, "all-locked spends nothing");
        assert_eq!(
            core.status,
            "Selected figures are locked — unlock them in the figures list."
        );
    }

    #[test]
    fn zero_spend_releases_and_guards() {
        stage_view::set_snap_config(SnapConfig::DEFAULT);
        let mut core = gizmo_core();
        let layout = gizmo_layout(&core, 1.0).unwrap();
        let depth = core.undo_depth();
        let root0 = core.project.frames[0].poses[0].root;
        // A click on the move grip (no travel): free.
        assert!(gizmo_down(&mut core, layout.move_at, 1.0));
        gizmo_up(&mut core, layout.move_at, false);
        assert_eq!(core.undo_depth(), depth);
        // A rotation under half the snap step settles to
        // zero: free, and nothing turns.
        assert!(gizmo_down(&mut core, layout.rotate_at, 1.0));
        let at = rotate_point(layout.rotate_at, layout.pivot, 5.0);
        gizmo_up(&mut core, at, false);
        assert_eq!(core.undo_depth(), depth);
        assert_eq!(core.project.frames[0].poses[0].root, root0);
        // A cancelled move: free.
        assert!(gizmo_down(&mut core, layout.move_at, 1.0));
        assert!(gizmo_move(&mut core, [layout.move_at[0] + 60.0, layout.move_at[1]]));
        gizmo_up(&mut core, [layout.move_at[0] + 60.0, layout.move_at[1]], true);
        assert_eq!(core.undo_depth(), depth);
        assert_eq!(core.project.frames[0].poses[0].root, root0);
        assert!(gizmo_preview().is_none());
        // A press that misses both handles is not ours.
        assert!(!gizmo_down(&mut core, [2000.0, 2000.0], 1.0));
        // Playing refuses outright.
        core.playing = true;
        assert!(!gizmo_down(&mut core, layout.move_at, 1.0));
        core.playing = false;
        // Setup mode refuses (rest poses have no roots to
        // orbit and are not the gizmo's edit).
        core.setup_mode = true;
        assert!(!gizmo_down(&mut core, layout.move_at, 1.0));
    }

    #[test]
    fn draw_pass_paints_the_gizmo_only_with_a_target() {
        let mut core = gizmo_core();
        let rt = RotTransform::from_parts([800.0, 600.0], core.project.stage, (0.0, 0.0, 1.0), 0.0);
        let count = |core: &EditorCore, token: &str| {
            format!("{:?}", draw_gizmo(Shape::new(), core, &rt, 1.0))
                .matches(token)
                .count()
        };
        // Frame stroke + stalk + knob ring = 3 strokes;
        // knob fill + knob dot + grip + grip heart = 4 fills.
        assert_eq!(count(&core, "Stroke { points:"), 3);
        assert_eq!(count(&core, "Fill { points:"), 4);
        // No target: nothing at all.
        core.selected_figure = 99;
        assert_eq!(count(&core, "Stroke { points:"), 0);
        assert_eq!(count(&core, "Fill { points:"), 0);
    }
}
