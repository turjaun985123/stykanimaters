//! 3D panel for the Rune editor shell (Phase D, section 19):
//! the fig3d session controls — enter/exit, joint posing,
//! camera presets, IK solves, bake, and the 3D figure / pose
//! libraries. Bin-crate code declared from `main.rs` (via
//! `rune_sec`); not part of the engine library.
//!
//! Wiring discipline: every control calls a real engine entry
//! point and nothing is faked.
//!
//! - Session lifecycle, joint posing, bakes and the figure
//!   library go through `EditorCore` ops (`enter_3d`,
//!   `fig3_bake`, …), each of which guards a missing session
//!   itself and reports through `core.status`.
//! - Camera presets, frame-camera, the arm/leg IK solves and
//!   the pose library have **no `EditorCore` wrapper** (grep
//!   verified: their only callers are fig3d's own tests). They
//!   are `stykanimate_rs::fig3d` free functions over the
//!   session state, so the `apply_*` helpers below apply them
//!   to `core.fig3` directly and set `core.status` in the same
//!   wording the editor ops use — exactly how `EditorCore`'s
//!   own 3D ops consume fig3d internally (`fig3_bake` calls
//!   `fig3d::bake_nodes` on the session state the same way).
//! - The joint list's direct selection writes the same field
//!   the one existing selection op writes:
//!   `fig3_select_next` (editor.rs) assigns `st.selected` and
//!   reports `"3D joint: {name}."` — there is no direct-select
//!   op, so `select_joint` below writes that field with that
//!   wording. Pose save and pose mirror are likewise fig3d
//!   free functions with no core wrapper:
//!   `fig3d::save_pose_library` (fig3d.rs:1544) and
//!   `fig3d::mirror_pose` (fig3d.rs:1342), wired by
//!   `save_current_pose` / `apply_mirror_pose` below.
//! - The readouts (Phase F) are pure helpers over the same
//!   session state: [`selected_joint_position`] runs
//!   `fig3d::evaluate3` — the evaluator [`apply_ik`] already
//!   uses — and [`camera_readout_text`] reads the session
//!   camera's orbit fields (azimuth = `Camera3.yaw`,
//!   elevation = `Camera3.pitch`, both degrees; distance in
//!   world units), borrowed, never moved. Both recompute at
//!   every panel build, so they track selection, posing and
//!   orbiting as they happen.
//!
//! - Icons (Phase H) — the panel header and the two
//!   library header rows lead with a `cube` glyph
//!   (`icons::icon_el`); the library buttons render
//!   through `icon_btn`: Save (both libraries) is
//!   `floppy-disk`, Mirror is `arrow-right-arrow-left`
//!   (Phase I; `copy` in Phase H — mirroring swaps left
//!   and right, it does not duplicate), per-entry Load is
//!   `folder-open`, per-entry Apply is `check`. Keys,
//!   labels, active flags and handlers are unchanged —
//!   including the GPUI-era `save-pose-3d` /
//!   `pose3d-lib-{i}` keys the Phase G §36 audit kept
//!   deliberately. Dimmed buttons go through [`ictl`].
//!
//! - Preview (Phase L): the panel's top now carries a
//!   live viewport — a canvas (`panel-3d-preview`, sized
//!   by `fig3_preview::PREVIEW_W` × `PREVIEW_H`) drawing
//!   the segment prims `fig3_preview::preview_prims`
//!   projects from the session, over a recessed
//!   `control_off` field. This closes the Phase K §33
//!   stop: until now the panel rendered no figure at
//!   all, so nothing could be drawn into it. Dragging
//!   the viewport orbits the session camera through
//!   the engine's own `fig3_orbit` — `Drag.dx`/`dy`
//!   are travel *since the press*, not per-event
//!   deltas, so the handler tracks the last pointer
//!   position in a thread-local and converts each
//!   move through the pure [`orbit_degrees`] helper
//!   (0.4° per point, dragging up pitches up). The
//!   Grid toggle (`d3-grid-toggle`) flips
//!   `fig3_preview`'s grid flag through [`ictl`]
//!   with the `border-all` glyph — the glyph was
//!   absent at this section's first census and
//!   landed mid-phase (Phase L §30), so the icon
//!   form is used; the toggle never dims, because
//!   the grid flag is shell view state, meaningful
//!   with or without a session.
//!   No new rotate row was added: the posing row's
//!   `rotx-3d` / `roty-3d` / `rotz-3d` buttons already
//!   call `fig3_rotate`, and `orbit-3d` already calls
//!   `fig3_orbit`.
//!
//! Controls that need a live session render dimmed when
//! there is none. The text controls ([`ctl`]) stay
//! **dimmed but wired**: the press still fires, and the
//! op/helper answers "Enter 3D mode first." in the status
//! line, so the muted look never hides a dead button.
//! The icon library controls ([`ictl`]) went inert in
//! Phase I instead: with no session they render through
//! `icon_btn::icon_btn_inert` — same box, glyph and label
//! in the muted colour, and no press handler at all.
//!
//! Element keys reuse the old GPUI control ids where one
//! existed (`mode-3d`, `bake-3d`, …) so headless `--tree`
//! checks keep finding them.

use rune::tok::{type_, Theme};
use rune::{canvas, col, el, row, text, Drag, El, LineCap, Phase, Shape, Tx};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::fig3d;

use crate::rune_sec::fig3_preview;
use crate::rune_sec::icon_btn::{icon_btn, icon_btn_inert};
use crate::rune_sec::icons::icon_el;
use crate::stage_view::hex_color;
use crate::State;

/// Camera preset buttons: `(fig3d preset name, button label)`.
/// The names are `fig3d::camera_preset`'s exact, case-sensitive
/// keys (fig3d.rs:149).
pub const CAMERA_PRESETS: [(&str, &str); 4] = [
    ("front", "Front"),
    ("side", "Side"),
    ("three-quarter", "3/4"),
    ("top", "Top"),
];

/// The Mirror pose button's glyph (Phase I): Font
/// Awesome's left/right swap arrows. Phase H shipped
/// `copy` here, but mirroring exchanges the pose's left
/// and right sides — it does not duplicate anything.
/// A named constant so the tests can pin the choice (an
/// element's children are not readable from a unit test).
pub const MIRROR_ICON: &str = "arrow-right-arrow-left";

/// One of the four humanoid limbs the IK buttons solve.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Limb {
    ArmL,
    ArmR,
    LegL,
    LegR,
}

impl Limb {
    /// `(upper, fore, end)` node indices — the humanoid-layout
    /// constants `fig3d::solve_arm_ik` / `solve_leg_ik`
    /// resolve internally (fig3d.rs:638-680); duplicated here
    /// only to compute the solve target from the live pose.
    pub fn chain(self) -> (usize, usize, usize) {
        match self {
            Limb::ArmL => (3, 4, 5),
            Limb::ArmR => (6, 7, 8),
            Limb::LegL => (9, 10, 11),
            Limb::LegR => (12, 13, 14),
        }
    }

    pub fn is_arm(self) -> bool {
        matches!(self, Limb::ArmL | Limb::ArmR)
    }

    pub fn is_left(self) -> bool {
        matches!(self, Limb::ArmL | Limb::LegL)
    }

    pub fn label(self) -> &'static str {
        match self {
            Limb::ArmL => "arm L",
            Limb::ArmR => "arm R",
            Limb::LegL => "leg L",
            Limb::LegR => "leg R",
        }
    }
}

/// The IK solve target for one limb, in world space (the
/// space `fig3d::solve_two_bone_ik` takes its target in):
/// from the chain's anchor (shoulder/hip world position),
/// step along a fixed natural-hang direction scaled by the
/// chain's full reach `l1 + l2`:
///
/// - arms hang down, slightly out and forward:
///   `(±0.35, −0.55, +0.45) · reach` — length ≈ 0.79 · reach;
/// - legs plant nearly straight down, slightly out and
///   forward: `(±0.18, −0.96, +0.12) · reach` — ≈ 0.98 · reach.
///
/// Both stay inside full reach, so the solver can actually
/// land the end effector on the target instead of just
/// pointing at it. `outward` is `+1.0`/`−1.0` — the side of
/// the figure the limb sits on, read from the live pose by
/// the caller (never assumed from a naming convention).
/// The figure faces +Z (fig3d's camera convention), so +Z is
/// forward. Pure and unit-tested.
pub fn ik_target(anchor: [f32; 3], outward: f32, reach: f32, arm: bool) -> [f32; 3] {
    let (fx, fy, fz) = if arm {
        (0.35, -0.55, 0.45)
    } else {
        (0.18, -0.96, 0.12)
    };
    [
        anchor[0] + outward * fx * reach,
        anchor[1] + fy * reach,
        anchor[2] + fz * reach,
    ]
}

/// Apply a named `fig3d::camera_preset` to the session
/// camera. Returns false (with the status set, editor-op
/// style) when there is no session or the name is unknown.
pub fn apply_camera_preset(core: &mut EditorCore, name: &str) -> bool {
    let Some(cam) = fig3d::camera_preset(name) else {
        return false;
    };
    let mut ok = false;
    if let Some(st) = &mut core.fig3 {
        st.camera = cam;
        ok = true;
    }
    if ok {
        core.status = format!("3D camera preset: {name}.");
    } else {
        core.status = "Enter 3D mode first.".into();
    }
    ok
}

/// Frame the session figure with `fig3d::frame_camera`
/// (centroid-targeted, figure inside the central 80% of the
/// stage). Returns false when there is no session.
pub fn apply_frame_camera(core: &mut EditorCore) -> bool {
    let stage = core.project.stage;
    let mut ok = false;
    if let Some(st) = &mut core.fig3 {
        let cam = fig3d::frame_camera(&st.figure, &st.pose, stage);
        st.camera = cam;
        ok = true;
    }
    if ok {
        core.status = "3D camera framed on the figure.".into();
    } else {
        core.status = "Enter 3D mode first.".into();
    }
    ok
}

/// Apply a saved pose-library entry to the session figure:
/// `fig3d::load_pose_library` → `st.pose`. Returns false
/// (status set) when there is no session or the entry
/// cannot be loaded.
pub fn apply_pose(core: &mut EditorCore, name: &str) -> bool {
    if core.fig3.is_none() {
        core.status = "Enter 3D mode first.".into();
        return false;
    }
    match fig3d::load_pose_library(name) {
        Ok(pose) => {
            if let Some(st) = &mut core.fig3 {
                st.pose = pose;
            }
            core.status = format!("Applied 3D pose '{name}'.");
            true
        }
        Err(e) => {
            core.status = format!("3D pose '{name}' failed to load: {e}");
            false
        }
    }
}

/// Solve two-bone IK for one limb toward [`ik_target`]:
/// `fig3d::solve_arm_ik` / `solve_leg_ik` over the session
/// pose, target computed from the live evaluated world
/// positions. Returns false (status set) when there is no
/// session or the figure is too small to hold the chain.
pub fn apply_ik(core: &mut EditorCore, limb: Limb) -> bool {
    let (upper, fore, end) = limb.chain();
    let mut ran = false;
    if let Some(st) = &mut core.fig3 {
        if end < st.figure.nodes.len() {
            let world = fig3d::evaluate3(&st.figure, &st.pose);
            let anchor = world[upper];
            let off1 = st.figure.nodes[fore].offset;
            let off2 = st.figure.nodes[end].offset;
            let reach = (off1[0] * off1[0] + off1[1] * off1[1] + off1[2] * off1[2]).sqrt()
                + (off2[0] * off2[0] + off2[1] * off2[1] + off2[2] * off2[2]).sqrt();
            let outward = if anchor[0] >= st.pose.root[0] {
                1.0
            } else {
                -1.0
            };
            let target = ik_target(anchor, outward, reach, limb.is_arm());
            let pose = if limb.is_arm() {
                fig3d::solve_arm_ik(&st.figure, &st.pose, limb.is_left(), target)
            } else {
                fig3d::solve_leg_ik(&st.figure, &st.pose, limb.is_left(), target)
            };
            st.pose = pose;
            ran = true;
        }
    }
    if ran {
        core.status = format!("IK solved: {}.", limb.label());
        true
    } else if core.fig3.is_some() {
        core.status = "That limb's chain is missing on this figure.".into();
        false
    } else {
        core.status = "Enter 3D mode first.".into();
        false
    }
}

/// The session figure's joints as `(index, name)` rows for
/// the joint list, in node order. Empty without a session.
/// Pure over the core.
pub fn joint_rows(core: &EditorCore) -> Vec<(usize, String)> {
    match &core.fig3 {
        None => Vec::new(),
        Some(st) => st
            .figure
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (i, n.name.clone()))
            .collect(),
    }
}

/// Select joint `index` directly: writes `st.selected` —
/// the field `EditorCore::fig3_select_next` cycles — with
/// that op's status wording. Returns false (status set)
/// without a session or for an out-of-range index, leaving
/// the selection untouched.
pub fn select_joint(core: &mut EditorCore, index: usize) -> bool {
    let mut ok = false;
    if let Some(st) = &mut core.fig3 {
        if index < st.figure.nodes.len() {
            st.selected = index;
            ok = true;
        }
    }
    if ok {
        let name = core.fig3.as_ref().unwrap().figure.nodes[index].name.clone();
        core.status = format!("3D joint: {name}.");
    } else if core.fig3.is_some() {
        core.status = "That joint does not exist on this figure.".into();
    } else {
        core.status = "Enter 3D mode first.".into();
    }
    ok
}

/// The name a pose is saved under by [`save_current_pose`]:
/// the pose library's convention is a free display name
/// (stored in the entry, slugified for the file name by
/// fig3d), and the panel has no text field, so the session
/// figure's name carries it: `"<figure> pose"`.
pub fn pose_save_name(core: &EditorCore) -> Option<String> {
    core.fig3
        .as_ref()
        .map(|st| format!("{} pose", st.figure.name))
}

/// Save the session pose into the 3D pose library via
/// `fig3d::save_pose_library`, under [`pose_save_name`].
/// Returns false (status set) without a session or on a
/// write error. The saved entry appears in the pose list
/// below on the next build (the panel re-scans per build).
pub fn save_current_pose(core: &mut EditorCore) -> bool {
    let Some(name) = pose_save_name(core) else {
        core.status = "Enter 3D mode first.".into();
        return false;
    };
    let pose = core.fig3.as_ref().unwrap().pose.clone();
    match fig3d::save_pose_library(&pose, &name) {
        Ok(_) => {
            core.status = format!("Saved 3D pose '{name}'.");
            true
        }
        Err(e) => {
            core.status = format!("3D pose '{name}' failed to save: {e}");
            false
        }
    }
}

/// Mirror the session pose across the YZ plane via
/// `fig3d::mirror_pose` (left/right partner joints exchange
/// rotations; an exact involution). Returns false (status
/// set) without a session.
pub fn apply_mirror_pose(core: &mut EditorCore) -> bool {
    let mut ok = false;
    if let Some(st) = &mut core.fig3 {
        st.pose = fig3d::mirror_pose(&st.figure, &st.pose);
        ok = true;
    }
    if ok {
        core.status = "3D pose mirrored.".into();
    } else {
        core.status = "Enter 3D mode first.".into();
    }
    ok
}

/// The Enter/Exit toggle's label, from real session state.
pub fn toggle_label(in_session: bool) -> &'static str {
    if in_session {
        "Exit 3D"
    } else {
        "Enter 3D"
    }
}

/// One line describing the session for the panel header:
/// figure, selected joint, and whether pose B (the bake
/// animation target) is marked. Pure over the core.
pub fn session_summary(core: &EditorCore) -> String {
    match &core.fig3 {
        None => "No 3D session — Enter 3D to pose the humanoid.".into(),
        Some(st) => {
            let joint = st
                .figure
                .nodes
                .get(st.selected)
                .map(|n| n.name.as_str())
                .unwrap_or("?");
            format!(
                "{} · joint {} · pose B {}",
                st.figure.name,
                joint,
                if st.pose_b.is_some() {
                    "marked"
                } else {
                    "not marked"
                }
            )
        }
    }
}

/// Format `v` with `places` decimals, never printing a
/// negative zero: `-0.0` itself, and small negatives that
/// round to zero at this precision, both print as `0.00…`
/// — a readout that says "-0.00" reads as a bug. Real
/// negatives keep their sign. Pure and unit-tested.
pub fn fmt_fixed(v: f32, places: usize) -> String {
    let v = if v == 0.0 { 0.0 } else { v };
    let s = format!("{:.*}", places, v);
    if s.starts_with('-') && s[1..].chars().all(|c| c == '0' || c == '.') {
        s[1..].to_string()
    } else {
        s
    }
}

/// The selected joint's solved world position, from the
/// evaluator the engine (and [`apply_ik`] above) already
/// uses: `fig3d::evaluate3(&st.figure, &st.pose)` at
/// `st.selected`. `None` without a session; an out-of-range
/// selection (the ops clamp it, but the readout never
/// panics) also yields `None`.
pub fn selected_joint_position(core: &EditorCore) -> Option<[f32; 3]> {
    let st = core.fig3.as_ref()?;
    fig3d::evaluate3(&st.figure, &st.pose)
        .get(st.selected)
        .copied()
}

/// The position readout line: `"<joint> · x .. · y .. ·
/// z .."` with two decimals per axis ([`fmt_fixed`]).
/// `None` without a session. Recomputed from the live
/// session at every panel build, so it follows both
/// selection and pose changes.
pub fn joint_position_text(core: &EditorCore) -> Option<String> {
    let st = core.fig3.as_ref()?;
    let p = selected_joint_position(core)?;
    let name = st
        .figure
        .nodes
        .get(st.selected)
        .map(|n| n.name.as_str())
        .unwrap_or("?");
    Some(format!(
        "{name} · x {} · y {} · z {}",
        fmt_fixed(p[0], 2),
        fmt_fixed(p[1], 2),
        fmt_fixed(p[2], 2)
    ))
}

/// The camera orbit readout line: azimuth, elevation and
/// distance of the session camera. The engine's field
/// names are `Camera3.yaw` (azimuth) and `Camera3.pitch`
/// (elevation), both in degrees; distance is in world
/// units (fig3d.rs:84-90). The camera is borrowed, never
/// moved — `Camera3` is not `Copy`. Angles print with one
/// decimal, distance with two ([`fmt_fixed`]). `None`
/// without a session.
pub fn camera_readout_text(core: &EditorCore) -> Option<String> {
    let cam = &core.fig3.as_ref()?.camera;
    Some(format!(
        "azimuth {}° · elevation {}° · distance {}",
        fmt_fixed(cam.yaw, 1),
        fmt_fixed(cam.pitch, 1),
        fmt_fixed(cam.distance, 2)
    ))
}

/// A panel button. Live buttons delegate to
/// `chrome_view::btn` so the geometry matches the rest of
/// the chrome exactly; `dim` renders the same shape muted
/// (track fill, secondary text, no hover/press change) but
/// keeps the handler wired — see the module docs.
fn ctl(
    key: &str,
    label: &str,
    active: bool,
    dim: bool,
    t: &Theme,
    f: impl Fn(&mut State) + 'static,
) -> El {
    if !dim {
        return crate::chrome_view::btn(key, label, active, t, f);
    }
    el()
        .key(key)
        .px(10.0)
        .py(5.0)
        .radius(6.0)
        .fill(t.control_off)
        .hover(t.control_off)
        .press(t.control_off)
        .on_press(f)
        .child(text(label).font(type_::LABEL).color(t.text_secondary))
}

/// A panel button with a Font Awesome glyph (Phase H):
/// [`ctl`]'s icon-bearing sibling for the library
/// controls. Live buttons delegate to
/// `icon_btn::icon_btn` — the glyph-and-label button that
/// mirrors the kit button's styling value for value.
///
/// `dim` (Phase I) delegates to
/// `icon_btn::icon_btn_inert`, the shared inert variant:
/// the same box, glyph and label in the muted colour,
/// and NO press handler at all. Phase H's dim branch
/// hand-rolled [`ctl`]'s muted-but-wired shape here; the
/// inert form is stronger — with no session a press
/// cannot fire the refusing op, so there is no status
/// churn and nothing to snapshot. The dimming condition
/// itself is unchanged: callers pass the same `dim` they
/// always did, and the handler argument is simply not
/// attached on the dim branch.
fn ictl(
    key: &str,
    icon: &str,
    label: &str,
    active: bool,
    dim: bool,
    t: &Theme,
    f: impl Fn(&mut State) + 'static,
) -> El {
    if !dim {
        return icon_btn(key, icon, label, active, t, f);
    }
    icon_btn_inert(key, icon, label, t)
}

fn section_label(text_: &str, t: &Theme) -> El {
    text(text_.to_string())
        .font(type_::LABEL)
        .color(t.text_secondary)
}

/// The 3D preview canvas's element key (Phase L). A named
/// constant so the tests can pin the string the headless
/// `--tree` probes look for — an element's key is not
/// readable back from a built `El` in a unit test.
pub const PREVIEW_KEY: &str = "panel-3d-preview";

/// Orbit sensitivity for the preview drag, in degrees of
/// camera travel per point of pointer travel (Phase L).
pub const ORBIT_DEG_PER_POINT: f32 = 0.4;

thread_local! {
    /// The preview drag's last pointer position, in
    /// canvas points. `Drag.dx` / `dy` report travel
    /// since the press, so per-move orbit steps are
    /// computed against this instead (the shell's
    /// thread-local state pattern — cf. the camera
    /// panel's clipboard).
    static ORBIT_LAST: std::cell::Cell<Option<[f32; 2]>> =
        const { std::cell::Cell::new(None) };
}

/// The camera orbit a preview-drag move applies, as
/// `[dyaw, dpitch]` in degrees: pointer travel since
/// `prev`, scaled by [`ORBIT_DEG_PER_POINT`], with the
/// pitch sign flipped — dragging up (falling y) pitches
/// the camera up. A missing `prev` (the press itself)
/// orbits nothing. Pure and unit-tested.
pub fn orbit_degrees(prev: Option<[f32; 2]>, cur: [f32; 2]) -> [f32; 2] {
    match prev {
        Some(p) => [
            (cur[0] - p[0]) * ORBIT_DEG_PER_POINT,
            -(cur[1] - p[1]) * ORBIT_DEG_PER_POINT,
        ],
        None => [0.0, 0.0],
    }
}

/// The preview canvas's drag handler (Phase L): Down
/// seeds the tracked position, Move orbits the session
/// camera through `fig3_orbit` by the step since the
/// last position, Up clears the tracking. Without a
/// session the gesture is tracked but never applied —
/// a silent no-op, so a stray drag over the empty
/// viewport cannot churn the status line with the op's
/// "Enter 3D mode first." refusal.
fn preview_drag(s: &mut State, d: &mut Drag) {
    match d.phase {
        Phase::Down => {
            ORBIT_LAST.with(|c| c.set(Some([d.x, d.y])));
        }
        Phase::Move => {
            let prev = ORBIT_LAST.with(|c| c.get());
            ORBIT_LAST.with(|c| c.set(Some([d.x, d.y])));
            if s.core.fig3.is_none() {
                return;
            }
            let [dyaw, dpitch] = orbit_degrees(prev, [d.x, d.y]);
            if dyaw != 0.0 || dpitch != 0.0 {
                s.core.fig3_orbit(dyaw, dpitch);
            }
        }
        Phase::Up => {
            ORBIT_LAST.with(|c| c.set(None));
        }
    }
}

/// The preview viewport's shape (Phase L): the segment
/// prims `fig3_preview::preview_prims` projects from
/// the live session (bones, plus the ground grid while
/// `fig3_preview::grid_visible()`), drawn the way the
/// main stage draws its segments — one round-capped
/// line per prim at the prim's own width, colours
/// through the stage's `hex_color`. The projection
/// takes the session as an `Option` and handles its
/// absence itself: with no session only the grid
/// appears, through its fallback camera, so the empty
/// viewport still shows the floor the figure will
/// stand on.
pub fn preview_shape(core: &EditorCore) -> Shape {
    let mut shape = Shape::new();
    let prims = fig3_preview::preview_prims(
        core.fig3.as_ref(),
        fig3_preview::grid_visible(),
        [fig3_preview::PREVIEW_W, fig3_preview::PREVIEW_H],
    );
    for prim in prims {
        if let Some(c) = hex_color(prim.color) {
            shape = shape.line(prim.a, prim.b, prim.width, LineCap::Round, c);
        }
    }
    shape
}

/// The preview viewport element (Phase L): a fixed
/// `PREVIEW_W` × `PREVIEW_H` canvas keyed
/// [`PREVIEW_KEY`], recessed `control_off` fill, with
/// the orbit drag attached. Mounted at the top of the
/// panel by [`three_d_panel`].
pub fn preview_el(s: &State, t: &Theme) -> El {
    canvas(preview_shape(&s.core))
        .key(PREVIEW_KEY)
        .size(fig3_preview::PREVIEW_W, fig3_preview::PREVIEW_H)
        .fill(t.control_off)
        .radius(6.0)
        .on_drag(|s: &mut State, d: &mut Drag, _tx: &mut Tx| preview_drag(s, d))
}

/// The 3D panel: session toggle + summary, joint posing,
/// camera, IK, bake, and the figure / pose libraries.
pub fn three_d_panel(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let in_session = core.fig3.is_some();
    let dim = !in_session;
    let pose_b_marked = core.fig3.as_ref().is_some_and(|st| st.pose_b.is_some());

    let mut panel = col()
        .key("panel-3d")
        .w_full()
        .gap(8.0)
        .padding(12.0)
        .radius(8.0)
        .fill(t.card)
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(icon_el("cube", 14.0, t.text))
                .child(
                    text("3D")
                        .font(type_::TITLE)
                        .color(t.text)
                        .grow(1.0),
                )
                .child(ctl("mode-3d", toggle_label(in_session), in_session, false, t, |s| {
                    if s.core.fig3.is_some() {
                        s.core.exit_3d();
                    } else {
                        s.core.enter_3d();
                    }
                })),
        )
        // Preview viewport (Phase L): the session figure
        // projected live, at the top of the panel above
        // the control groups; dragging it orbits the
        // camera. The Grid toggle under it flips the
        // ground-grid flag the projection reads; it
        // never dims (the flag is shell view state, not
        // session state).
        .child(preview_el(s, t))
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(section_label("Preview", t))
                .child(ictl(
                    "d3-grid-toggle",
                    "border-all",
                    "Grid",
                    fig3_preview::grid_visible(),
                    false,
                    t,
                    |_s| {
                        fig3_preview::toggle_grid();
                    },
                )),
        )
        .child(
            text(session_summary(core))
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        // Joint posing (the GPUI 3D group's controls).
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(ctl("joint-3d", "Joint>", false, dim, t, |s| {
                    s.core.fig3_select_next()
                }))
                .child(ctl("rotx-3d", "RotX +15", false, dim, t, |s| {
                    s.core.fig3_rotate(0, 15.0)
                }))
                .child(ctl("roty-3d", "RotY +15", false, dim, t, |s| {
                    s.core.fig3_rotate(1, 15.0)
                }))
                .child(ctl("rotz-3d", "RotZ +15", false, dim, t, |s| {
                    s.core.fig3_rotate(2, 15.0)
                }))
                .child(ctl("orbit-3d", "Orbit +15", false, dim, t, |s| {
                    s.core.fig3_orbit(15.0, 0.0)
                })),
        );

    // Joint list: one row per joint of the session figure;
    // pressing a row selects that joint directly (the field
    // `fig3_select_next` cycles), the current joint renders
    // active. Without a session there is no figure to list.
    panel = panel.child(section_label("Joints", t));
    if in_session {
        let selected = core.fig3.as_ref().unwrap().selected;
        for (i, name) in joint_rows(core) {
            panel = panel.child(
                row().gap(6.0).center_y().child(ctl(
                    &format!("joint-sel-3d-{i}"),
                    &format!("{i} · {name}"),
                    i == selected,
                    false,
                    t,
                    move |s| {
                        select_joint(&mut s.core, i);
                    },
                )),
            );
        }
    } else {
        panel = panel.child(
            text("Enter 3D to list the figure's joints.")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    }

    // Selected-joint position readout (Phase F): the
    // solved world position of `st.selected`, recomputed
    // from the live session on every build, so it tracks
    // selection and posing changes as they happen.
    if in_session {
        if let Some(line) = joint_position_text(core) {
            panel = panel.child(
                row()
                    .gap(6.0)
                    .center_y()
                    .child(section_label("Position", t))
                    .child(text(line).font(type_::LABEL).color(t.text)),
            );
        }
    }

    panel = panel
        // Camera presets + frame.
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(section_label("Camera", t))
                .child(ctl("cam-front-3d", "Front", false, dim, t, |s| {
                    apply_camera_preset(&mut s.core, "front");
                }))
                .child(ctl("cam-side-3d", "Side", false, dim, t, |s| {
                    apply_camera_preset(&mut s.core, "side");
                }))
                .child(ctl("cam-three-quarter-3d", "3/4", false, dim, t, |s| {
                    apply_camera_preset(&mut s.core, "three-quarter");
                }))
                .child(ctl("cam-top-3d", "Top", false, dim, t, |s| {
                    apply_camera_preset(&mut s.core, "top");
                }))
                .child(ctl("cam-frame-3d", "Frame camera", false, dim, t, |s| {
                    apply_frame_camera(&mut s.core);
                })),
        );

    // Camera orbit readout (Phase F): azimuth / elevation /
    // distance of the session camera, directly under the
    // camera controls. The panel's only orbit stepper is
    // the "Orbit +15" button in the posing row — there are
    // no per-axis steppers for the readout to sit beside.
    if in_session {
        if let Some(line) = camera_readout_text(core) {
            panel = panel.child(
                row()
                    .gap(6.0)
                    .center_y()
                    .child(section_label("Orbit", t))
                    .child(text(line).font(type_::LABEL).color(t.text)),
            );
        }
    }

    panel = panel
        // IK solves.
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(section_label("IK", t))
                .child(ctl("ik-arm-l-3d", "Arm L", false, dim, t, |s| {
                    apply_ik(&mut s.core, Limb::ArmL);
                }))
                .child(ctl("ik-arm-r-3d", "Arm R", false, dim, t, |s| {
                    apply_ik(&mut s.core, Limb::ArmR);
                }))
                .child(ctl("ik-leg-l-3d", "Leg L", false, dim, t, |s| {
                    apply_ik(&mut s.core, Limb::LegL);
                }))
                .child(ctl("ik-leg-r-3d", "Leg R", false, dim, t, |s| {
                    apply_ik(&mut s.core, Limb::LegR);
                })),
        )
        // Bake: pose B mark, animation to frames, still to a
        // 2D figure. 8 frames matches the GPUI Bake anim.
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(section_label("Bake", t))
                .child(ctl("mark-b-3d", "Mark pose B", pose_b_marked, dim, t, |s| {
                    s.core.fig3_mark_pose_b()
                }))
                .child(ctl("bake-anim-3d", "Bake to frames", false, dim, t, |s| {
                    s.core.fig3_bake_animation(8)
                }))
                .child(ctl("bake-3d", "Bake to 2D", false, dim, t, |s| {
                    s.core.fig3_bake()
                })),
        );

    // Figure library: names from the core's scan of
    // ./library3d. Loading an entry STARTS a session (the
    // op replaces `core.fig3`), so these rows are live even
    // with no session; Save needs one.
    panel = panel.child(
        row()
            .gap(6.0)
            .center_y()
            .child(icon_el("cube", 12.0, t.text_secondary))
            .child(section_label("Figure library", t).grow(1.0))
            .child(ictl("save-3d", "floppy-disk", "Save current", false, dim, t, |s| {
                s.core.fig3_save_to_library()
            })),
    );
    let figures = core.fig3_library_list();
    if figures.is_empty() {
        panel = panel.child(
            text("No saved 3D figures.")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    }
    for (i, name) in figures.iter().enumerate() {
        let load_name = name.clone();
        panel = panel.child(
            row()
                .gap(6.0)
                .center_y()
                .child(
                    text(name.clone())
                        .font(type_::LABEL)
                        .color(t.text)
                        .grow(1.0),
                )
                .child(icon_btn(
                    &format!("fig3-lib-{i}"),
                    "folder-open",
                    "Load",
                    false,
                    t,
                    move |s| s.core.fig3_load_from_library(&load_name),
                )),
        );
    }

    // Pose library: ./library3d/poses via fig3d's scan;
    // Apply sets the session pose (dimmed without one).
    // Save stores the current pose under "<figure> pose"
    // (fig3d::save_pose_library); Mirror flips it across
    // the YZ plane (fig3d::mirror_pose).
    panel = panel.child(
        row()
            .gap(6.0)
            .center_y()
            .child(icon_el("cube", 12.0, t.text_secondary))
            .child(section_label("Pose library", t).grow(1.0))
            .child(ictl("save-pose-3d", "floppy-disk", "Save pose", false, dim, t, |s| {
                save_current_pose(&mut s.core);
            }))
            .child(ictl("mirror-pose-3d", MIRROR_ICON, "Mirror pose", false, dim, t, |s| {
                apply_mirror_pose(&mut s.core);
            })),
    );
    let poses = fig3d::scan_pose_library();
    if poses.is_empty() {
        panel = panel.child(
            text("No saved 3D poses.")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    }
    for (i, name) in poses.iter().enumerate() {
        let pose_name = name.clone();
        panel = panel.child(
            row()
                .gap(6.0)
                .center_y()
                .child(
                    text(name.clone())
                        .font(type_::LABEL)
                        .color(t.text)
                        .grow(1.0),
                )
                .child(ictl(
                    &format!("pose3d-lib-{i}"),
                    "check",
                    "Apply",
                    false,
                    dim,
                    t,
                    move |s| {
                        apply_pose(&mut s.core, &pose_name);
                    },
                )),
        );
    }

    panel
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
        ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
    }

    #[test]
    fn camera_presets_cover_fig3ds_named_views() {
        for (name, _) in CAMERA_PRESETS {
            assert!(
                fig3d::camera_preset(name).is_some(),
                "preset {name} missing from fig3d"
            );
        }
    }

    #[test]
    fn ik_target_stays_inside_reach_and_hangs_down_forward() {
        let anchor = [100.0, 200.0, 0.0];
        for arm in [true, false] {
            for outward in [1.0, -1.0] {
                let tgt = ik_target(anchor, outward, 50.0, arm);
                assert!(
                    dist(anchor, tgt) < 50.0,
                    "target must be reachable: {tgt:?}"
                );
                assert!(tgt[1] < anchor[1], "limb must hang below its anchor");
                assert!(tgt[2] > anchor[2], "limb must reach forward (+Z)");
                assert!(
                    (tgt[0] - anchor[0]) * outward > 0.0,
                    "target must bow out on the limb's own side"
                );
            }
        }
    }

    #[test]
    fn ik_target_with_zero_reach_is_the_anchor() {
        assert_eq!(ik_target([1.0, 2.0, 3.0], 1.0, 0.0, true), [1.0, 2.0, 3.0]);
    }

    #[test]
    fn toggle_label_follows_session_state() {
        assert_eq!(toggle_label(false), "Enter 3D");
        assert_eq!(toggle_label(true), "Exit 3D");
    }

    #[test]
    fn session_helpers_refuse_without_a_session() {
        let mut core = EditorCore::new();
        assert_eq!(
            session_summary(&core),
            "No 3D session — Enter 3D to pose the humanoid."
        );
        assert!(!apply_camera_preset(&mut core, "side"));
        assert_eq!(core.status, "Enter 3D mode first.");
        assert!(!apply_frame_camera(&mut core));
        assert!(!apply_ik(&mut core, Limb::ArmL));
        assert!(!apply_pose(&mut core, "anything"));
        assert_eq!(core.status, "Enter 3D mode first.");
    }

    #[test]
    fn camera_preset_and_frame_apply_inside_a_session() {
        let mut core = EditorCore::new();
        core.enter_3d();
        assert!(apply_camera_preset(&mut core, "side"));
        let cam = &core.fig3.as_ref().unwrap().camera;
        assert_eq!(*cam, fig3d::camera_preset("side").unwrap());
        assert!(apply_frame_camera(&mut core));
        let framed = &core.fig3.as_ref().unwrap().camera;
        assert!(framed.distance > 0.0);
        // Framing targets the figure, not the world origin
        // default — the rest humanoid's centroid sits above
        // its root at the origin.
        assert!(framed.target[1] > 0.0, "unexpected target {framed:?}");
    }

    #[test]
    fn ik_solve_moves_the_hand_to_the_target() {
        let mut core = EditorCore::new();
        core.enter_3d();
        let (before, target) = {
            let st = core.fig3.as_ref().unwrap();
            let world = fig3d::evaluate3(&st.figure, &st.pose);
            let (upper, _, end) = Limb::ArmL.chain();
            let off1 = st.figure.nodes[4].offset;
            let off2 = st.figure.nodes[5].offset;
            let reach = (off1[0] * off1[0] + off1[1] * off1[1] + off1[2] * off1[2]).sqrt()
                + (off2[0] * off2[0] + off2[1] * off2[1] + off2[2] * off2[2]).sqrt();
            let outward = if world[upper][0] >= st.pose.root[0] {
                1.0
            } else {
                -1.0
            };
            (world[end], ik_target(world[upper], outward, reach, true))
        };
        assert!(apply_ik(&mut core, Limb::ArmL));
        let st = core.fig3.as_ref().unwrap();
        let world = fig3d::evaluate3(&st.figure, &st.pose);
        let after = world[Limb::ArmL.chain().2];
        assert!(
            dist(after, target) < 5.0,
            "hand should land on the IK target: before {before:?}, after {after:?}, target {target:?}"
        );
        assert_eq!(core.status, "IK solved: arm L.");
    }

    #[test]
    fn session_summary_names_figure_joint_and_pose_b() {
        let mut core = EditorCore::new();
        core.enter_3d();
        let txt = session_summary(&core);
        assert!(txt.contains("pose B not marked"), "unexpected: {txt}");
        core.fig3_mark_pose_b();
        let txt = session_summary(&core);
        assert!(txt.contains("pose B marked"), "unexpected: {txt}");
    }

    #[test]
    fn joint_rows_come_from_the_real_session_figure() {
        let mut core = EditorCore::new();
        assert!(joint_rows(&core).is_empty(), "no session, no rows");
        core.enter_3d();
        let rows = joint_rows(&core);
        let st = core.fig3.as_ref().unwrap();
        assert_eq!(rows.len(), st.figure.nodes.len());
        assert!(rows.len() >= 15, "humanoid chain set: {rows:?}");
        for (i, (idx, name)) in rows.iter().enumerate() {
            assert_eq!(*idx, i, "rows are in node order");
            assert_eq!(name, &st.figure.nodes[i].name);
        }
    }

    #[test]
    fn select_joint_dispatches_directly() {
        let mut core = EditorCore::new();
        // No session: refused, editor-op wording.
        assert!(!select_joint(&mut core, 0));
        assert_eq!(core.status, "Enter 3D mode first.");
        core.enter_3d();
        // Direct dispatch to a mid-chain joint, with the
        // cycling op's exact status wording.
        let name = core.fig3.as_ref().unwrap().figure.nodes[5].name.clone();
        assert!(select_joint(&mut core, 5));
        assert_eq!(core.fig3.as_ref().unwrap().selected, 5);
        assert_eq!(core.status, format!("3D joint: {name}."));
        // Rotation now lands on the selected joint — the
        // selection the rotate op reads is the one we set.
        let before = core.fig3.as_ref().unwrap().pose.clone();
        core.fig3_rotate(0, 30.0);
        assert_ne!(core.fig3.as_ref().unwrap().pose, before);
        // Out of range: refused, selection untouched.
        assert!(!select_joint(&mut core, 999));
        assert_eq!(core.fig3.as_ref().unwrap().selected, 5);
    }

    #[test]
    fn save_current_pose_writes_a_loadable_library_entry() {
        let mut core = EditorCore::new();
        assert!(!save_current_pose(&mut core));
        assert_eq!(core.status, "Enter 3D mode first.");
        core.enter_3d();
        // Make the pose distinctive, then save it.
        assert!(select_joint(&mut core, 3));
        core.fig3_rotate(2, 40.0);
        let saved_pose = core.fig3.as_ref().unwrap().pose.clone();
        let name = pose_save_name(&core).unwrap();
        assert!(save_current_pose(&mut core));
        assert_eq!(core.status, format!("Saved 3D pose '{name}'."));
        // The entry is in the library scan and loads back as
        // the exact pose that was saved.
        assert!(
            fig3d::scan_pose_library().contains(&name),
            "saved pose missing from the library scan"
        );
        assert_eq!(fig3d::load_pose_library(&name).unwrap(), saved_pose);
        // And the panel's own Apply path restores it after
        // the session pose moves on.
        core.fig3_rotate(2, 15.0);
        assert_ne!(core.fig3.as_ref().unwrap().pose, saved_pose);
        assert!(apply_pose(&mut core, &name));
        assert_eq!(core.fig3.as_ref().unwrap().pose, saved_pose);
    }

    #[test]
    fn apply_pose_reports_a_missing_library_entry() {
        let mut core = EditorCore::new();
        core.enter_3d();
        // Pose the figure so "pose untouched" is observable.
        assert!(select_joint(&mut core, 3));
        core.fig3_rotate(2, 25.0);
        let before = core.fig3.as_ref().unwrap().pose.clone();
        // A name that was never saved (the scan can race a
        // file deleted outside the app): the load fails, the
        // handler reports it, and the session pose is left
        // exactly as it was — no partial apply, no panic.
        assert!(!apply_pose(&mut core, "Audit Missing Pose Qqzzx"));
        assert!(
            core.status.contains("failed to load"),
            "unexpected status: {}",
            core.status
        );
        assert_eq!(core.fig3.as_ref().unwrap().pose, before);
    }

    #[test]
    fn mirror_pose_flips_and_restores() {
        let mut core = EditorCore::new();
        assert!(!apply_mirror_pose(&mut core));
        assert_eq!(core.status, "Enter 3D mode first.");
        core.enter_3d();
        // An asymmetric pose: left arm raised only.
        assert!(select_joint(&mut core, 3));
        core.fig3_rotate(2, 40.0);
        let original = core.fig3.as_ref().unwrap().pose.clone();
        assert!(apply_mirror_pose(&mut core));
        assert_eq!(core.status, "3D pose mirrored.");
        let mirrored = core.fig3.as_ref().unwrap().pose.clone();
        assert_ne!(mirrored, original, "mirror must change an asymmetric pose");
        // mirror_pose is an exact involution: a second press
        // restores the original pose bit-for-bit.
        assert!(apply_mirror_pose(&mut core));
        assert_eq!(core.fig3.as_ref().unwrap().pose, original);
    }

    #[test]
    fn fmt_fixed_never_prints_a_negative_zero() {
        assert_eq!(fmt_fixed(0.0, 2), "0.00");
        assert_eq!(fmt_fixed(-0.0, 2), "0.00");
        // Small negatives that round to zero lose the sign.
        assert_eq!(fmt_fixed(-0.004, 2), "0.00");
        assert_eq!(fmt_fixed(-0.04, 1), "0.0");
        // Real negatives keep it.
        assert_eq!(fmt_fixed(-0.006, 2), "-0.01");
        assert_eq!(fmt_fixed(-1.5, 2), "-1.50");
        // Ordinary values at both precisions the readouts use.
        assert_eq!(fmt_fixed(400.0, 2), "400.00");
        assert_eq!(fmt_fixed(89.9, 1), "89.9");
        assert_eq!(fmt_fixed(1234.567, 2), "1234.57");
    }

    #[test]
    fn readouts_are_none_without_a_session() {
        let core = EditorCore::new();
        assert_eq!(selected_joint_position(&core), None);
        assert_eq!(joint_position_text(&core), None);
        assert_eq!(camera_readout_text(&core), None);
    }

    #[test]
    fn joint_position_readout_matches_the_evaluator() {
        let mut core = EditorCore::new();
        core.enter_3d();
        // Whatever is selected, the readout is exactly the
        // evaluator's world position for that node, named
        // and formatted at two decimals.
        let (name, expected) = {
            let st = core.fig3.as_ref().unwrap();
            let world = fig3d::evaluate3(&st.figure, &st.pose);
            (st.figure.nodes[st.selected].name.clone(), world[st.selected])
        };
        assert_eq!(selected_joint_position(&core), Some(expected));
        assert_eq!(
            joint_position_text(&core).unwrap(),
            format!(
                "{name} · x {} · y {} · z {}",
                fmt_fixed(expected[0], 2),
                fmt_fixed(expected[1], 2),
                fmt_fixed(expected[2], 2)
            )
        );
        // Changing the selection moves the readout to the
        // newly selected joint.
        assert!(select_joint(&mut core, 5));
        let st = core.fig3.as_ref().unwrap();
        let world = fig3d::evaluate3(&st.figure, &st.pose);
        assert_eq!(selected_joint_position(&core), Some(world[5]));
        // Posing moves it too: rotate the shoulder (3) and
        // the hand (5) lands somewhere else.
        let before = selected_joint_position(&core).unwrap();
        assert!(select_joint(&mut core, 3));
        core.fig3_rotate(2, 40.0);
        assert!(select_joint(&mut core, 5));
        assert_ne!(selected_joint_position(&core).unwrap(), before);
    }

    #[test]
    fn camera_readout_tracks_presets_and_orbit() {
        let mut core = EditorCore::new();
        core.enter_3d();
        // A fresh session starts on the default camera.
        assert_eq!(
            camera_readout_text(&core).unwrap(),
            "azimuth 0.0° · elevation 0.0° · distance 400.00"
        );
        assert!(apply_camera_preset(&mut core, "side"));
        assert_eq!(
            camera_readout_text(&core).unwrap(),
            "azimuth 90.0° · elevation 0.0° · distance 400.00"
        );
        // Orbiting (the panel's Orbit button op) moves the
        // azimuth the readout prints.
        core.fig3_orbit(15.0, 0.0);
        assert_eq!(
            camera_readout_text(&core).unwrap(),
            "azimuth 105.0° · elevation 0.0° · distance 400.00"
        );
        // And the readout always agrees with the borrowed
        // camera's fields, whatever produced them.
        let cam = &core.fig3.as_ref().unwrap().camera;
        assert_eq!(
            camera_readout_text(&core).unwrap(),
            format!(
                "azimuth {}° · elevation {}° · distance {}",
                fmt_fixed(cam.yaw, 1),
                fmt_fixed(cam.pitch, 1),
                fmt_fixed(cam.distance, 2)
            )
        );
    }

    #[test]
    fn mirror_button_glyph_is_the_left_right_swap() {
        // Phase I: the Mirror pose button's glyph is the
        // left/right swap, not Phase H's `copy` — mirroring
        // exchanges sides, it does not duplicate. The name
        // is in the icon catalogue, so neither the live
        // button nor its inert twin degrades to a
        // glyph-less box.
        assert_eq!(MIRROR_ICON, "arrow-right-arrow-left");
        assert_ne!(MIRROR_ICON, "copy");
        assert!(crate::rune_sec::icons::has_icon(MIRROR_ICON));
    }

    // ---- Preview viewport (Phase L) ----

    #[test]
    fn preview_key_is_the_headless_tree_name() {
        // The key the `--tree` probes look for, pinned
        // because a built element's key cannot be read
        // back from a unit test.
        assert_eq!(PREVIEW_KEY, "panel-3d-preview");
    }

    #[test]
    fn orbit_degrees_scales_pointer_travel() {
        // The press itself orbits nothing.
        assert_eq!(orbit_degrees(None, [50.0, 60.0]), [0.0, 0.0]);
        // Dragging right 10 pt yaws +4°; the pitch is
        // untouched when y does not move.
        assert_eq!(
            orbit_degrees(Some([50.0, 60.0]), [60.0, 60.0]),
            [4.0, 0.0]
        );
        // Dragging up 10 pt (y falls) pitches +4° —
        // the sign flip that makes the drag feel like
        // grabbing the figure, not the floor.
        assert_eq!(
            orbit_degrees(Some([50.0, 60.0]), [50.0, 50.0]),
            [0.0, 4.0]
        );
        // Dragging left and down negates both.
        assert_eq!(
            orbit_degrees(Some([50.0, 60.0]), [40.0, 70.0]),
            [-4.0, -4.0]
        );
    }

    #[test]
    fn panel_and_preview_build_with_and_without_a_session() {
        // The established shell-test fixture: a real
        // State from `crate::init()` and a real theme.
        // Without a session the preview shows the grid
        // only; entering 3D adds the figure — both
        // builds must construct.
        let mut state = crate::init();
        let t = rune::tok::theme(true);
        assert!(state.core.fig3.is_none());
        let _ = preview_el(&state, &t);
        let _ = three_d_panel(&state, &t);
        state.core.enter_3d();
        assert!(state.core.fig3.is_some());
        let _ = preview_el(&state, &t);
        let _ = three_d_panel(&state, &t);
    }

    #[test]
    fn ictl_builds_live_and_inert_forms() {
        // Element construction is pure (a press needs a
        // Runtime), so this pins that both branches of the
        // Phase I migration construct: live → `icon_btn`,
        // dim → `icon_btn_inert`. The dimming condition is
        // the caller's `dim` flag, unchanged.
        let t = rune::tok::theme(true);
        let _ = ictl("k-live", "floppy-disk", "Save pose", false, false, &t, |_| {});
        let _ = ictl("k-dim", "floppy-disk", "Save pose", false, true, &t, |_| {});
        let _ = ictl("k-dim-mirror", MIRROR_ICON, "Mirror pose", false, true, &t, |_| {});
        let _ = ictl("k-dim-apply", "check", "Apply", false, true, &t, |_| {});
    }
}
