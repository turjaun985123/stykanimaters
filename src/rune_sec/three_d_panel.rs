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
//!   deliberately. Dimmed buttons go through `ictl`.
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
//!   `fig3_preview`'s grid flag through `ictl`
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
//! - Preview framing + zoom (Phase M): the Preview row
//!   also carries Frame (`d3-frame`, `ictl` with the
//!   `expand` glyph) and the distance steppers
//!   `d3-zoom-dn` / `d3-zoom-up` (`ctl`). Frame runs
//!   the closed-form `fig3d::frame_camera_points` over
//!   the evaluated session points at the preview's own
//!   aspect and writes back only the returned target
//!   and distance — yaw, pitch and FOV stay the user's.
//!   That is the preview counterpart of the Camera
//!   row's stage-aspect "Frame camera", which replaces
//!   the whole camera. The steppers scale the camera
//!   distance through the pure [`zoom_distance`].
//!   Camera moves in this panel take no undo snapshot
//!   (`fig3_orbit` doesn't either — the camera is
//!   session state, not project state), so both follow
//!   it: snapshot-free.
//!
//! - Readout audit, round 2 (Wave 10): every readout's
//!   source field, checked for the unwrapped-accumulator
//!   class Wave 9 fixed for the azimuth. Pitch is NOT
//!   an accumulator: the one op that moves it,
//!   `fig3_orbit`, clamps the stored value to ±89.9°
//!   (and the projector clamps again), so the stored
//!   pitch the readout prints can never grow. There is
//!   no roll field on `Camera3` at all. Distance is
//!   clamped on both panel paths that write it
//!   ([`zoom_distance`]'s range, and
//!   `frame_camera_points`' own clamp to the same
//!   range), and the joint position derives from
//!   `evaluate3`, which is periodic in the pose's
//!   accumulated rotations. Only the azimuth needed
//!   the display wrap. The library rows and the
//!   session summary did carry the OTHER unbounded
//!   input — names: a library entry's stored name is
//!   file data with no length limit, and the figure /
//!   pose row name texts grew without Rune's one-line
//!   truncate mode, so a long name forced the row past
//!   the 316 pt card (the figures panel's documented
//!   layout truth). The row names now truncate (the
//!   full name stays in the Load / Apply handler and
//!   in the text's accessibility string), and the
//!   session summary — which carries the loaded
//!   figure's name — wraps at full width (the settings
//!   `note_line` pattern) instead of painting one
//!   unwrapped line past the card.
//!
//! Controls that need a live session render dimmed when
//! there is none. The text controls (`ctl`) stay
//! **dimmed but wired**: the press still fires, and the
//! op/helper answers "Enter 3D mode first." in the status
//! line, so the muted look never hides a dead button.
//! The icon library controls (`ictl`) went inert in
//! Phase I instead: with no session they render through
//! `icon_btn::icon_btn_inert` — same box, glyph and label
//! in the muted colour, and no press handler at all.
//!
//! - Phase N adoption: the root is the kit card
//!   (`design::panel_col`), the header is
//!   `design::panel_title` over a `design::divider`, and
//!   the group captions come from
//!   `design::section_label`. The local `ctl` / `ictl`
//!   dialects are retired — call sites dispatch on the
//!   same `dim` flag directly: text controls to
//!   `chrome_view::btn` when live and
//!   `chrome_view::btn_muted_live` when dimmed (the
//!   muted visual with the handler kept, so the op's
//!   "Enter 3D mode first." refusal still answers), icon
//!   controls to `icon_btn` when live and
//!   `icon_btn_inert` when dimmed. Keys, labels and
//!   handlers are unchanged at every site.
//!
//! Element keys reuse the old GPUI control ids where one
//! existed (`mode-3d`, `bake-3d`, …) so headless `--tree`
//! checks keep finding them.
//!
//! - Localisation (Wave S, S14): every user-visible label
//!   in the panel — the section captions, the button and
//!   icon-button labels, the Enter/Exit toggle and the
//!   empty states — resolves through `i18n_wire::tr`
//!   under the `threed_*` keys (IA-S14-1; the keys land in
//!   `i18n.rs` at integration, until then `tr` falls back
//!   to the key itself). English output is byte-identical
//!   to the pre-wiring literals, pinned by
//!   `threed_i18n_keys_exist_and_match_the_english_labels`.
//!   Locale-invariant notation stays literal by design
//!   (the R6 precedent): the `3D` title, `3/4`, the
//!   `RotX/RotY/RotZ/Orbit +15` step labels and the
//!   `−` / `+` distance steppers. The composed readouts
//!   (`session_summary`, `joint_position_text`,
//!   `camera_readout_text`) are dynamic text, not labels,
//!   and stayed English this wave.
//!
//! - Localisation (Wave U, U9): the composed readouts
//!   now resolve through the table too, as `{slot}`
//!   template keys substituted with `.replace()` at the
//!   composing site (the camera `follow_label`
//!   precedent; IA-U9-1): `threed_no_session`,
//!   `threed_session_summary`, `threed_pose_b_marked`,
//!   `threed_pose_b_not_marked`, `threed_joint_position`,
//!   `threed_camera_readout`. English output is
//!   byte-identical, pinned by
//!   `threed_readout_templates_exist_and_compose_byte_identically`.
//!   Status sentences stay English.

use rune::tok::{type_, Theme};
use rune::{canvas, row, text, Drag, El, LineCap, Phase, Shape, Tx};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::fig3d;

use crate::rune_sec::design;
use crate::rune_sec::fig3_preview;
use crate::rune_sec::i18n_wire;
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

/// The Frame button's glyph (Phase M): Font Awesome's
/// `expand` — four corners pushing outward, the
/// frame-the-whole-figure gesture. It is in the icon
/// table, so no fallback glyph is needed. A named
/// constant so the tests can pin the choice (an
/// element's children are not readable from a unit
/// test), like [`MIRROR_ICON`].
pub const FRAME_ICON: &str = "expand";

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

/// One preview-zoom step's distance factor: each press
/// multiplies or divides the camera distance by this.
pub const ZOOM_FACTOR: f32 = 1.25;

/// The camera-distance range the preview zoom stays
/// inside — the same clamp `fig3d::frame_camera_points`
/// applies to the distances it returns, so a framed
/// distance and a stepped distance live in one range.
pub const ZOOM_DISTANCE_MIN: f32 = 40.0;
/// See [`ZOOM_DISTANCE_MIN`].
pub const ZOOM_DISTANCE_MAX: f32 = 4000.0;

/// The distance a zoom step starts from when the
/// current distance is not finite:
/// `fig3d::preview_camera`'s distance, the preview's
/// own default (a test pins the two together so they
/// cannot drift apart).
pub const ZOOM_FALLBACK_DISTANCE: f32 = 300.0;

/// One preview-zoom step applied to a camera distance:
/// `dir > 0` zooms in (distance ÷ [`ZOOM_FACTOR`]),
/// `dir < 0` zooms out (× [`ZOOM_FACTOR`]), `dir == 0`
/// leaves the distance alone. The result is clamped to
/// [`ZOOM_DISTANCE_MIN`]..=[`ZOOM_DISTANCE_MAX`] (an
/// overflowing step saturates at the cap); a non-finite
/// input yields [`ZOOM_FALLBACK_DISTANCE`]. Pure and
/// unit-tested.
pub fn zoom_distance(distance: f32, dir: i32) -> f32 {
    if !distance.is_finite() {
        return ZOOM_FALLBACK_DISTANCE;
    }
    let stepped = if dir > 0 {
        distance / ZOOM_FACTOR
    } else if dir < 0 {
        distance * ZOOM_FACTOR
    } else {
        distance
    };
    stepped.clamp(ZOOM_DISTANCE_MIN, ZOOM_DISTANCE_MAX)
}

/// Frame the session figure in the PREVIEW (Phase M):
/// evaluate the session pose and run the closed-form
/// `fig3d::frame_camera_points` at the preview's aspect
/// on the session camera's own yaw/pitch/FOV, then
/// write back only the returned target and distance —
/// the orbit angles and FOV are the user's and stay
/// put. (The Camera row's [`apply_frame_camera`]
/// instead replaces the whole camera, framed for the
/// stage.) Snapshot-free, like every camera move in
/// this panel: `EditorCore::fig3_orbit` takes no
/// snapshot, the camera being session state. Returns
/// false (status set) without a session.
pub fn apply_frame_preview(core: &mut EditorCore) -> bool {
    let mut ok = false;
    if let Some(st) = &mut core.fig3 {
        let points = fig3d::evaluate3(&st.figure, &st.pose);
        let (fov, yaw, pitch) = (st.camera.fov_deg, st.camera.yaw, st.camera.pitch);
        let framed = fig3d::frame_camera_points(
            &points,
            fov,
            fig3_preview::PREVIEW_W / fig3_preview::PREVIEW_H,
            yaw,
            pitch,
        );
        st.camera.target = framed.target;
        st.camera.distance = framed.distance;
        ok = true;
    }
    if ok {
        core.status = "3D preview framed on the figure.".into();
    } else {
        core.status = "Enter 3D mode first.".into();
    }
    ok
}

/// One preview-zoom step (Phase M): scale the session
/// camera's distance through [`zoom_distance`]. The
/// panel's `d3-zoom-dn` / `d3-zoom-up` buttons call it
/// with −1 / +1. Snapshot-free (see
/// [`apply_frame_preview`]). Returns false (status set)
/// without a session.
pub fn apply_preview_zoom(core: &mut EditorCore, dir: i32) -> bool {
    let mut stepped = None;
    if let Some(st) = &mut core.fig3 {
        st.camera.distance = zoom_distance(st.camera.distance, dir);
        stepped = Some(st.camera.distance);
    }
    match stepped {
        Some(d) => {
            core.status = format!("3D camera distance {}.", fmt_fixed(d, 2));
            true
        }
        None => {
            core.status = "Enter 3D mode first.".into();
            false
        }
    }
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

/// The Enter/Exit toggle's label, from real session state
/// (Wave S, S14: resolved through the i18n table like the
/// shell's other panel labels — `threed_enter` /
/// `threed_exit`, IA-S14-1).
pub fn toggle_label(core: &EditorCore, in_session: bool) -> String {
    if in_session {
        i18n_wire::tr(core, "threed_exit")
    } else {
        i18n_wire::tr(core, "threed_enter")
    }
}

/// One line describing the session for the panel header:
/// figure, selected joint, and whether pose B (the bake
/// animation target) is marked. Pure over the core.
///
/// Wave U (U9): the line resolves through the i18n table
/// like the panel's labels — `threed_no_session` for the
/// no-session sentence, the `threed_session_summary`
/// template (`{figure}` / `{joint}` / `{state}` slots
/// substituted with `.replace()`, the camera
/// `follow_label` precedent) with the pose-B state word
/// itself keyed (`threed_pose_b_marked` /
/// `threed_pose_b_not_marked`). The keys land via
/// IA-U9-1; until then `tr` falls back to the key itself
/// (the Q7 pattern). English output is byte-identical
/// to the pre-wiring format string.
pub fn session_summary(core: &EditorCore) -> String {
    match &core.fig3 {
        None => i18n_wire::tr(core, "threed_no_session"),
        Some(st) => {
            let joint = st
                .figure
                .nodes
                .get(st.selected)
                .map(|n| n.name.as_str())
                .unwrap_or("?");
            let state = if st.pose_b.is_some() {
                i18n_wire::tr(core, "threed_pose_b_marked")
            } else {
                i18n_wire::tr(core, "threed_pose_b_not_marked")
            };
            i18n_wire::tr(core, "threed_session_summary")
                .replace("{figure}", &st.figure.name)
                .replace("{joint}", joint)
                .replace("{state}", &state)
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
/// selection and pose changes. Wave U (U9): the line is
/// the `threed_joint_position` template (`{name}` /
/// `{x}` / `{y}` / `{z}` slots, IA-U9-1), English
/// byte-identical to the pre-wiring format string.
pub fn joint_position_text(core: &EditorCore) -> Option<String> {
    let st = core.fig3.as_ref()?;
    let p = selected_joint_position(core)?;
    let name = st
        .figure
        .nodes
        .get(st.selected)
        .map(|n| n.name.as_str())
        .unwrap_or("?");
    Some(
        i18n_wire::tr(core, "threed_joint_position")
            .replace("{name}", name)
            .replace("{x}", &fmt_fixed(p[0], 2))
            .replace("{y}", &fmt_fixed(p[1], 2))
            .replace("{z}", &fmt_fixed(p[2], 2)),
    )
}

/// The camera orbit readout line: azimuth, elevation and
/// distance of the session camera. The engine's field
/// names are `Camera3.yaw` (azimuth) and `Camera3.pitch`
/// (elevation), both in degrees; distance is in world
/// units (fig3d.rs:84-90). The camera is borrowed, never
/// moved — `Camera3` is not `Copy`. Angles print with one
/// decimal, distance with two ([`fmt_fixed`]). `None`
/// without a session.
///
/// The azimuth prints as the equivalent heading inside
/// one turn (`rem_euclid(360.0)`): `fig3_orbit`
/// accumulates yaw without bound (editor.rs — twelve
/// Orbit presses are a full 360°, and preview drags add
/// hundreds of degrees), while the projection is
/// periodic in yaw, so 725° and 5° are the same view.
/// Printing the raw accumulation would also let this
/// line grow past the panel card — at the measured
/// ≈5.7 pt per LABEL character, an azimuth of −720°
/// already overruns the 316 pt column; wrapped into
/// [0, 360) the longest possible line is the pinned
/// 52-character extreme in the tests. Display-only: the
/// stored camera is untouched.
///
/// The other two fields need no wrap (Wave 10 round-2
/// audit): `fig3_orbit` clamps the STORED pitch to
/// ±89.9° on every move, so elevation cannot
/// accumulate, and the distance is clamped to
/// [`ZOOM_DISTANCE_MIN`]..=[`ZOOM_DISTANCE_MAX`] by
/// both panel paths that write it ([`zoom_distance`]
/// and `frame_camera_points`).
pub fn camera_readout_text(core: &EditorCore) -> Option<String> {
    let cam = &core.fig3.as_ref()?.camera;
    // Wave U (U9): the line is the `threed_camera_readout`
    // template (`{azimuth}` / `{elevation}` / `{distance}`
    // slots, IA-U9-1), English byte-identical to the
    // pre-wiring format string.
    Some(
        i18n_wire::tr(core, "threed_camera_readout")
            .replace("{azimuth}", &fmt_fixed(cam.yaw.rem_euclid(360.0), 1))
            .replace("{elevation}", &fmt_fixed(cam.pitch, 1))
            .replace("{distance}", &fmt_fixed(cam.distance, 2)),
    )
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
pub fn preview_shape(core: &EditorCore, palette: fig3_preview::Fig3Palette) -> Shape {
    let mut shape = Shape::new();
    let prims = fig3_preview::preview_prims(
        core.fig3.as_ref(),
        fig3_preview::grid_visible(),
        [fig3_preview::PREVIEW_W, fig3_preview::PREVIEW_H],
        palette,
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
    canvas(preview_shape(
        &s.core,
        fig3_preview::Fig3Palette::from_theme(t),
    ))
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

    let mut panel = design::panel_col("panel-3d", t)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(design::panel_title(Some("cube"), "3D", None, t))
                .child(crate::chrome_view::btn(
                    "mode-3d",
                    &toggle_label(core, in_session),
                    in_session,
                    t,
                    |s| {
                        if s.core.fig3.is_some() {
                            s.core.exit_3d();
                        } else {
                            s.core.enter_3d();
                        }
                    },
                )),
        )
        .child(design::divider(t))
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
                .child(design::section_label(&i18n_wire::tr(core, "threed_preview"), t))
                .child(icon_btn(
                    "d3-grid-toggle",
                    "border-all",
                    &i18n_wire::tr(core, "threed_grid"),
                    fig3_preview::grid_visible(),
                    t,
                    |_s| {
                        fig3_preview::toggle_grid();
                    },
                ))
                // Frame + distance steppers (Phase M):
                // Frame fits the whole figure into the
                // preview (target + distance only, via
                // the icon family's inert-when-no-session
                // form); the − / + steppers scale the
                // camera distance (the text family's
                // dimmed-but-wired form, like the camera
                // preset row).
                .child(if dim {
                    icon_btn_inert("d3-frame", FRAME_ICON, &i18n_wire::tr(core, "threed_frame"), t)
                } else {
                    icon_btn("d3-frame", FRAME_ICON, &i18n_wire::tr(core, "threed_frame"), false, t, |s| {
                        apply_frame_preview(&mut s.core);
                    })
                })
                .child(if dim {
                    crate::chrome_view::btn_muted_live("d3-zoom-dn", "−", false, t, |s| {
                        apply_preview_zoom(&mut s.core, -1);
                    })
                } else {
                    crate::chrome_view::btn("d3-zoom-dn", "−", false, t, |s| {
                        apply_preview_zoom(&mut s.core, -1);
                    })
                })
                .child(if dim {
                    crate::chrome_view::btn_muted_live("d3-zoom-up", "+", false, t, |s| {
                        apply_preview_zoom(&mut s.core, 1);
                    })
                } else {
                    crate::chrome_view::btn("d3-zoom-up", "+", false, t, |s| {
                        apply_preview_zoom(&mut s.core, 1);
                    })
                }),
        )
        .child(
            // The summary carries the loaded figure's
            // name — library file data, unbounded in
            // length — so it wraps at the card's full
            // width (the settings `note_line` pattern)
            // rather than painting one unwrapped line
            // past the card edge (Wave 10 round 2).
            text(session_summary(core))
                .font(type_::LABEL)
                .color(t.text_secondary)
                .wrap()
                .w_full(),
        )
        // Joint posing (the GPUI 3D group's controls).
        .child(
            row()
                .gap(6.0)
                .center_y()
                // UI-mission overflow repair (audit s15 D1,
                // p05 O-7): the five controls measure 365 pt
                // against the 316 pt column, leaving Orbit
                // +15 cut off past the card edge. Wrapping
                // packs them onto two lines inside budget.
                .flex_wrap()
                .child(if dim {
                    crate::chrome_view::btn_muted_live("joint-3d", &i18n_wire::tr(core, "threed_joint_next"), false, t, |s| {
                        s.core.fig3_select_next()
                    })
                } else {
                    crate::chrome_view::btn("joint-3d", &i18n_wire::tr(core, "threed_joint_next"), false, t, |s| {
                        s.core.fig3_select_next()
                    })
                })
                .child(if dim {
                    crate::chrome_view::btn_muted_live("rotx-3d", "RotX +15", false, t, |s| {
                        s.core.fig3_rotate(0, 15.0)
                    })
                } else {
                    crate::chrome_view::btn("rotx-3d", "RotX +15", false, t, |s| {
                        s.core.fig3_rotate(0, 15.0)
                    })
                })
                .child(if dim {
                    crate::chrome_view::btn_muted_live("roty-3d", "RotY +15", false, t, |s| {
                        s.core.fig3_rotate(1, 15.0)
                    })
                } else {
                    crate::chrome_view::btn("roty-3d", "RotY +15", false, t, |s| {
                        s.core.fig3_rotate(1, 15.0)
                    })
                })
                .child(if dim {
                    crate::chrome_view::btn_muted_live("rotz-3d", "RotZ +15", false, t, |s| {
                        s.core.fig3_rotate(2, 15.0)
                    })
                } else {
                    crate::chrome_view::btn("rotz-3d", "RotZ +15", false, t, |s| {
                        s.core.fig3_rotate(2, 15.0)
                    })
                })
                .child(if dim {
                    crate::chrome_view::btn_muted_live("orbit-3d", "Orbit +15", false, t, |s| {
                        s.core.fig3_orbit(15.0, 0.0)
                    })
                } else {
                    crate::chrome_view::btn("orbit-3d", "Orbit +15", false, t, |s| {
                        s.core.fig3_orbit(15.0, 0.0)
                    })
                }),
        );

    // Joint list: one row per joint of the session figure;
    // pressing a row selects that joint directly (the field
    // `fig3_select_next` cycles), the current joint renders
    // active. Without a session there is no figure to list.
    // A session figure with no nodes at all — only
    // reachable from a hand-made library file, since
    // `fig3d::load_library_figure` deserialises without
    // validating the node count — gets the same kit
    // treatment with its own message, so the section never
    // renders as a bare label.
    panel = panel.child(design::section_label(&i18n_wire::tr(core, "threed_joints"), t));
    let rows = joint_rows(core);
    if rows.is_empty() {
        // RM28 (PL-7): with no session, the state offers
        // its filling action — the panel's own Enter 3D
        // toggle op. In session (a hand-made figure with
        // no joints) there is nothing to offer, and the
        // plain state stands.
        panel = panel.child(if in_session {
            design::empty_state(
                "cube",
                &i18n_wire::tr(core, "threed_no_joints"),
                t,
            )
        } else {
            design::empty_state_action(
                "cube",
                &i18n_wire::tr(core, "threed_enter_to_list_joints"),
                "threed-empty-enter",
                &i18n_wire::tr(core, "threed_enter"),
                t,
                |s| s.core.enter_3d(),
            )
        });
    } else {
        let selected = core.fig3.as_ref().unwrap().selected;
        for (i, name) in rows {
            panel = panel.child(
                row().gap(6.0).center_y().child(crate::chrome_view::btn(
                    &format!("joint-sel-3d-{i}"),
                    &format!("{i} · {name}"),
                    i == selected,
                    t,
                    move |s| {
                        select_joint(&mut s.core, i);
                    },
                )),
            );
        }
    }

    // Selected-joint position readout (Phase F): the
    // solved world position of `st.selected`, recomputed
    // from the live session on every build, so it tracks
    // selection and posing changes as they happen. The
    // text truncates (the status strip tail's one-line
    // mode): a loaded figure's long joint name or large
    // coordinates elide inside the row instead of
    // painting past the card — at every ordinary value
    // the line fits whole and nothing is elided.
    if in_session {
        if let Some(line) = joint_position_text(core) {
            panel = panel.child(
                row()
                    .gap(6.0)
                    .center_y()
                    .child(design::section_label(&i18n_wire::tr(core, "threed_position"), t))
                    .child(text(line).font(type_::LABEL).color(t.text).truncate()),
            );
        }
    }

    panel = panel
        // Camera presets + frame.
        .child(
            row()
                .gap(6.0)
                .center_y()
                // UI-mission overflow repair (audit s15 D1,
                // p05 O-7): label + five presets measure
                // 353.5 pt against the 316 pt column, leaving
                // Frame camera past the card edge. Wrapping
                // packs them onto two lines inside budget.
                .flex_wrap()
                .child(design::section_label(&i18n_wire::tr(core, "threed_camera"), t))
                .child(if dim {
                    crate::chrome_view::btn_muted_live("cam-front-3d", &i18n_wire::tr(core, "threed_front"), false, t, |s| {
                        apply_camera_preset(&mut s.core, "front");
                    })
                } else {
                    crate::chrome_view::btn("cam-front-3d", &i18n_wire::tr(core, "threed_front"), false, t, |s| {
                        apply_camera_preset(&mut s.core, "front");
                    })
                })
                .child(if dim {
                    crate::chrome_view::btn_muted_live("cam-side-3d", &i18n_wire::tr(core, "threed_side"), false, t, |s| {
                        apply_camera_preset(&mut s.core, "side");
                    })
                } else {
                    crate::chrome_view::btn("cam-side-3d", &i18n_wire::tr(core, "threed_side"), false, t, |s| {
                        apply_camera_preset(&mut s.core, "side");
                    })
                })
                .child(if dim {
                    crate::chrome_view::btn_muted_live(
                        "cam-three-quarter-3d",
                        "3/4",
                        false,
                        t,
                        |s| {
                            apply_camera_preset(&mut s.core, "three-quarter");
                        },
                    )
                } else {
                    crate::chrome_view::btn("cam-three-quarter-3d", "3/4", false, t, |s| {
                        apply_camera_preset(&mut s.core, "three-quarter");
                    })
                })
                .child(if dim {
                    crate::chrome_view::btn_muted_live("cam-top-3d", &i18n_wire::tr(core, "threed_top"), false, t, |s| {
                        apply_camera_preset(&mut s.core, "top");
                    })
                } else {
                    crate::chrome_view::btn("cam-top-3d", &i18n_wire::tr(core, "threed_top"), false, t, |s| {
                        apply_camera_preset(&mut s.core, "top");
                    })
                })
                .child(if dim {
                    crate::chrome_view::btn_muted_live(
                        "cam-frame-3d",
                        &i18n_wire::tr(core, "threed_frame_camera"),
                        false,
                        t,
                        |s| {
                            apply_frame_camera(&mut s.core);
                        },
                    )
                } else {
                    crate::chrome_view::btn("cam-frame-3d", &i18n_wire::tr(core, "threed_frame_camera"), false, t, |s| {
                        apply_frame_camera(&mut s.core);
                    })
                }),
        );

    // Camera orbit readout (Phase F): azimuth / elevation /
    // distance of the session camera, directly under the
    // camera controls. The panel's only orbit stepper is
    // the "Orbit +15" button in the posing row — there are
    // no per-axis steppers for the readout to sit beside.
    // The text truncates like the Position readout's: the
    // azimuth is already wrapped into one turn (see
    // `camera_readout_text`), and this keeps even the
    // longest remaining line — widest azimuth, clamped
    // pitch, maximum distance at once — inside the card.
    if in_session {
        if let Some(line) = camera_readout_text(core) {
            panel = panel.child(
                row()
                    .gap(6.0)
                    .center_y()
                    .child(design::section_label(&i18n_wire::tr(core, "threed_orbit"), t))
                    .child(text(line).font(type_::LABEL).color(t.text).truncate()),
            );
        }
    }

    panel = panel
        // IK solves.
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(design::section_label(&i18n_wire::tr(core, "threed_ik"), t))
                .child(if dim {
                    crate::chrome_view::btn_muted_live("ik-arm-l-3d", &i18n_wire::tr(core, "threed_arm_l"), false, t, |s| {
                        apply_ik(&mut s.core, Limb::ArmL);
                    })
                } else {
                    crate::chrome_view::btn("ik-arm-l-3d", &i18n_wire::tr(core, "threed_arm_l"), false, t, |s| {
                        apply_ik(&mut s.core, Limb::ArmL);
                    })
                })
                .child(if dim {
                    crate::chrome_view::btn_muted_live("ik-arm-r-3d", &i18n_wire::tr(core, "threed_arm_r"), false, t, |s| {
                        apply_ik(&mut s.core, Limb::ArmR);
                    })
                } else {
                    crate::chrome_view::btn("ik-arm-r-3d", &i18n_wire::tr(core, "threed_arm_r"), false, t, |s| {
                        apply_ik(&mut s.core, Limb::ArmR);
                    })
                })
                .child(if dim {
                    crate::chrome_view::btn_muted_live("ik-leg-l-3d", &i18n_wire::tr(core, "threed_leg_l"), false, t, |s| {
                        apply_ik(&mut s.core, Limb::LegL);
                    })
                } else {
                    crate::chrome_view::btn("ik-leg-l-3d", &i18n_wire::tr(core, "threed_leg_l"), false, t, |s| {
                        apply_ik(&mut s.core, Limb::LegL);
                    })
                })
                .child(if dim {
                    crate::chrome_view::btn_muted_live("ik-leg-r-3d", &i18n_wire::tr(core, "threed_leg_r"), false, t, |s| {
                        apply_ik(&mut s.core, Limb::LegR);
                    })
                } else {
                    crate::chrome_view::btn("ik-leg-r-3d", &i18n_wire::tr(core, "threed_leg_r"), false, t, |s| {
                        apply_ik(&mut s.core, Limb::LegR);
                    })
                }),
        )
        // Bake: pose B mark, animation to frames, still to a
        // 2D figure. 8 frames matches the GPUI Bake anim.
        .child(
            row()
                .gap(6.0)
                .center_y()
                // UI-mission overflow repair (audit s15 D1,
                // p05 O-7): label + three bake controls
                // measure 325.5 pt against the 316 pt
                // column, leaving Bake to 2D +9.5 past the
                // card edge. Wrapping packs them onto two
                // lines inside budget.
                .flex_wrap()
                .child(design::section_label(&i18n_wire::tr(core, "threed_bake"), t))
                .child(if dim {
                    crate::chrome_view::btn_muted_live(
                        "mark-b-3d",
                        &i18n_wire::tr(core, "threed_mark_pose_b"),
                        pose_b_marked,
                        t,
                        |s| {
                            s.core.fig3_mark_pose_b()
                        },
                    )
                } else {
                    crate::chrome_view::btn("mark-b-3d", &i18n_wire::tr(core, "threed_mark_pose_b"), pose_b_marked, t, |s| {
                        s.core.fig3_mark_pose_b()
                    })
                })
                .child(if dim {
                    crate::chrome_view::btn_muted_live(
                        "bake-anim-3d",
                        &i18n_wire::tr(core, "threed_bake_frames"),
                        false,
                        t,
                        |s| {
                            s.core.fig3_bake_animation(8)
                        },
                    )
                } else {
                    crate::chrome_view::btn("bake-anim-3d", &i18n_wire::tr(core, "threed_bake_frames"), false, t, |s| {
                        s.core.fig3_bake_animation(8)
                    })
                })
                .child(if dim {
                    crate::chrome_view::btn_muted_live("bake-3d", &i18n_wire::tr(core, "threed_bake_2d"), false, t, |s| {
                        s.core.fig3_bake()
                    })
                } else {
                    crate::chrome_view::btn("bake-3d", &i18n_wire::tr(core, "threed_bake_2d"), false, t, |s| {
                        s.core.fig3_bake()
                    })
                }),
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
            .child(design::section_label(&i18n_wire::tr(core, "threed_figure_library"), t).grow(1.0))
            .child(if dim {
                icon_btn_inert("save-3d", "floppy-disk", &i18n_wire::tr(core, "threed_save_current"), t)
            } else {
                icon_btn("save-3d", "floppy-disk", &i18n_wire::tr(core, "threed_save_current"), false, t, |s| {
                    s.core.fig3_save_to_library()
                })
            }),
    );
    let figures = core.fig3_library_list();
    if figures.is_empty() {
        // RM28 (PL-7): in session, the state offers its
        // filling action — the Save current op the row
        // above runs. With no session Save is inert
        // there too, so the plain state stands.
        panel = panel.child(if dim {
            design::empty_state(
                "cube",
                &i18n_wire::tr(core, "threed_no_figures"),
                t,
            )
        } else {
            design::empty_state_action(
                "cube",
                &i18n_wire::tr(core, "threed_no_figures"),
                "threed-empty-save",
                &i18n_wire::tr(core, "threed_save_current"),
                t,
                |s| s.core.fig3_save_to_library(),
            )
        });
    }
    for (i, name) in figures.iter().enumerate() {
        let load_name = name.clone();
        panel = panel.child(
            row()
                .gap(6.0)
                .center_y()
                .child(
                    // The stored name is library file
                    // data, unbounded in length: without
                    // Rune's one-line truncate mode its
                    // min-content width forces the whole
                    // row past the card edge and the Load
                    // button with it (the figures panel's
                    // documented layout truth). Truncation
                    // is display-only — the handler below
                    // loads by the full name (Wave 10
                    // round 2).
                    text(name.clone())
                        .font(type_::LABEL)
                        .color(t.text)
                        .grow(1.0)
                        .truncate(),
                )
                .child(icon_btn(
                    &format!("fig3-lib-{i}"),
                    "folder-open",
                    &i18n_wire::tr(core, "threed_load"),
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
            .child(design::section_label(&i18n_wire::tr(core, "threed_pose_library"), t).grow(1.0))
            .child(if dim {
                icon_btn_inert("save-pose-3d", "floppy-disk", &i18n_wire::tr(core, "threed_save_pose"), t)
            } else {
                icon_btn("save-pose-3d", "floppy-disk", &i18n_wire::tr(core, "threed_save_pose"), false, t, |s| {
                    save_current_pose(&mut s.core);
                })
            })
            .child(if dim {
                icon_btn_inert("mirror-pose-3d", MIRROR_ICON, &i18n_wire::tr(core, "threed_mirror_pose"), t)
            } else {
                icon_btn("mirror-pose-3d", MIRROR_ICON, &i18n_wire::tr(core, "threed_mirror_pose"), false, t, |s| {
                    apply_mirror_pose(&mut s.core);
                })
            }),
    );
    let poses = fig3d::scan_pose_library();
    if poses.is_empty() {
        panel = panel.child(design::empty_state(
            "cube",
            &i18n_wire::tr(core, "threed_no_poses"),
            t,
        ));
    }
    for (i, name) in poses.iter().enumerate() {
        let pose_name = name.clone();
        panel = panel.child(
            row()
                .gap(6.0)
                .center_y()
                .child(
                    // Same unbounded stored name as the
                    // figure rows above: truncate the
                    // display, keep the full name in the
                    // Apply handler (Wave 10 round 2).
                    text(name.clone())
                        .font(type_::LABEL)
                        .color(t.text)
                        .grow(1.0)
                        .truncate(),
                )
                .child(if dim {
                    icon_btn_inert(&format!("pose3d-lib-{i}"), "check", &i18n_wire::tr(core, "threed_apply"), t)
                } else {
                    icon_btn(
                        &format!("pose3d-lib-{i}"),
                        "check",
                        &i18n_wire::tr(core, "threed_apply"),
                        false,
                        t,
                        move |s| {
                            apply_pose(&mut s.core, &pose_name);
                        },
                    )
                }),
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
        // Wave S (S14): the label resolves through the
        // i18n table now, so this asserts the English
        // values of `threed_enter` / `threed_exit` and,
        // like the pin below, passes once IA-S14-1 lands
        // the keys (until then `tr` falls back to the key
        // itself — the Q7 pattern).
        let core = EditorCore::new();
        assert_eq!(toggle_label(&core, false), "Enter 3D");
        assert_eq!(toggle_label(&core, true), "Exit 3D");
    }

    #[test]
    fn threed_i18n_keys_exist_and_match_the_english_labels() {
        // Wave S (S14): every label the panel wires
        // through `i18n_wire::tr` must resolve to exactly
        // the English string the panel rendered before
        // the wiring. The keys land via IA-S14-1 (the
        // integrator appends them to all 28 tables after
        // S1 settles i18n.rs), so this pin passes only
        // once that IA lands — until then `tr` falls back
        // to the key itself and `key_in_table` is false
        // (the Q7 pattern: wiring and keys land together
        // at integration).
        let core = EditorCore::new();
        for (key, english) in [
            ("threed_preview", "Preview"),
            ("threed_joints", "Joints"),
            ("threed_position", "Position"),
            ("threed_camera", "Camera"),
            ("threed_orbit", "Orbit"),
            ("threed_ik", "IK"),
            ("threed_bake", "Bake"),
            ("threed_figure_library", "Figure library"),
            ("threed_pose_library", "Pose library"),
            ("threed_enter", "Enter 3D"),
            ("threed_exit", "Exit 3D"),
            ("threed_grid", "Grid"),
            ("threed_frame", "Frame"),
            ("threed_joint_next", "Joint>"),
            ("threed_front", "Front"),
            ("threed_side", "Side"),
            ("threed_top", "Top"),
            ("threed_frame_camera", "Frame camera"),
            ("threed_arm_l", "Arm L"),
            ("threed_arm_r", "Arm R"),
            ("threed_leg_l", "Leg L"),
            ("threed_leg_r", "Leg R"),
            ("threed_mark_pose_b", "Mark pose B"),
            ("threed_bake_frames", "Bake to frames"),
            ("threed_bake_2d", "Bake to 2D"),
            ("threed_save_current", "Save current"),
            ("threed_load", "Load"),
            ("threed_save_pose", "Save pose"),
            ("threed_mirror_pose", "Mirror pose"),
            ("threed_apply", "Apply"),
            ("threed_no_joints", "This figure has no joints."),
            (
                "threed_enter_to_list_joints",
                "Enter 3D to list the figure's joints.",
            ),
            ("threed_no_figures", "No saved 3D figures."),
            ("threed_no_poses", "No saved 3D poses."),
        ] {
            assert!(i18n_wire::key_in_table(key), "missing i18n key: {key}");
            assert_eq!(i18n_wire::tr(&core, key), english);
        }
    }

    #[test]
    fn threed_readout_templates_exist_and_compose_byte_identically() {
        // Wave U (U9): the composed readouts resolve
        // through `{slot}` template keys (IA-U9-1), the
        // camera `follow_label` shape. The stored values
        // must carry the slots verbatim, and composing
        // them with representative values must reproduce
        // the exact English lines the readouts printed
        // before the wiring. Like the label pin above,
        // this passes only once IA-U9-1 lands — until
        // then `tr` falls back to the key itself and
        // `key_in_table` is false (the Q7 pattern: wiring
        // and keys land together at integration). The
        // readouts' own behaviour tests
        // (`session_summary_names_figure_joint_and_pose_b`,
        // `camera_readout_tracks_presets_and_orbit`, …)
        // pin the same English lines end to end and
        // likewise resolve once the IA lands.
        let core = EditorCore::new();
        for (key, stored) in [
            (
                "threed_no_session",
                "No 3D session — Enter 3D to pose the humanoid.",
            ),
            (
                "threed_session_summary",
                "{figure} · joint {joint} · pose B {state}",
            ),
            ("threed_pose_b_marked", "marked"),
            ("threed_pose_b_not_marked", "not marked"),
            (
                "threed_joint_position",
                "{name} · x {x} · y {y} · z {z}",
            ),
            (
                "threed_camera_readout",
                "azimuth {azimuth}° · elevation {elevation}° · distance {distance}",
            ),
        ] {
            assert!(i18n_wire::key_in_table(key), "missing i18n key: {key}");
            assert_eq!(i18n_wire::tr(&core, key), stored);
        }
        // Composition spot checks: composed from the
        // stored templates exactly as the readout
        // functions compose them, the lines are
        // byte-identical to the pre-wiring format output.
        assert_eq!(
            i18n_wire::tr(&core, "threed_session_summary")
                .replace("{figure}", "Humanoid")
                .replace("{joint}", "head")
                .replace(
                    "{state}",
                    &i18n_wire::tr(&core, "threed_pose_b_not_marked")
                ),
            "Humanoid · joint head · pose B not marked"
        );
        assert_eq!(
            i18n_wire::tr(&core, "threed_joint_position")
                .replace("{name}", "head")
                .replace("{x}", "1.00")
                .replace("{y}", "2.50")
                .replace("{z}", "-3.25"),
            "head · x 1.00 · y 2.50 · z -3.25"
        );
        assert_eq!(
            i18n_wire::tr(&core, "threed_camera_readout")
                .replace("{azimuth}", "90.0")
                .replace("{elevation}", "0.0")
                .replace("{distance}", "400.00"),
            "azimuth 90.0° · elevation 0.0° · distance 400.00"
        );
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
    fn camera_readout_wraps_the_azimuth_into_one_turn() {
        let mut core = EditorCore::new();
        core.enter_3d();
        // fig3_orbit accumulates yaw without bound
        // (editor.rs): a -15° orbit from the default
        // camera stores -15, and the readout shows the
        // equivalent heading inside one turn.
        core.fig3_orbit(-15.0, 0.0);
        assert_eq!(core.fig3.as_ref().unwrap().camera.yaw, -15.0);
        assert!(
            camera_readout_text(&core)
                .unwrap()
                .starts_with("azimuth 345.0°")
        );
        // Past a full turn the stored value keeps
        // growing; the displayed heading does not — and
        // the stored camera is left exactly as it was
        // (the wrap is display-only).
        core.fig3.as_mut().unwrap().camera.yaw = 725.0;
        assert!(
            camera_readout_text(&core)
                .unwrap()
                .starts_with("azimuth 5.0°")
        );
        assert_eq!(core.fig3.as_ref().unwrap().camera.yaw, 725.0);
        core.fig3.as_mut().unwrap().camera.yaw = 360.0;
        assert!(
            camera_readout_text(&core)
                .unwrap()
                .starts_with("azimuth 0.0°")
        );
    }

    #[test]
    fn camera_readout_line_is_the_pinned_extreme_at_the_bounds() {
        let mut core = EditorCore::new();
        core.enter_3d();
        {
            let st = core.fig3.as_mut().unwrap();
            st.camera.yaw = 359.9;
            st.camera.pitch = -89.9;
            st.camera.distance = 4000.0;
        }
        // The longest line the readout can produce once
        // the azimuth wraps: widest wrapped heading, the
        // projector's pitch clamp, and the zoom range's
        // maximum distance. 52 characters ≈ 295 pt at
        // the LABEL advance measured from the headless
        // tree (278.5 pt for the 49-character default
        // line) — the row's truncate() is what keeps
        // this corner inside the card (see the Orbit
        // row's comment in `three_d_panel`).
        let line = camera_readout_text(&core).unwrap();
        assert_eq!(
            line,
            "azimuth 359.9° · elevation -89.9° · distance 4000.00"
        );
        assert_eq!(line.chars().count(), 52);
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
    fn panel_builds_with_a_zero_node_session_figure() {
        // `fig3d::load_library_figure` deserialises a
        // Figure3 without validating its node count, so
        // a hand-made library entry can start a session
        // whose figure has no joints at all. The Joints
        // section must fall back to its empty state
        // rather than render as a bare label, and the
        // whole panel — preview, readouts, libraries —
        // must still build.
        let mut state = crate::init();
        let t = rune::tok::theme(true);
        state.core.enter_3d();
        state.core.fig3.as_mut().unwrap().figure.nodes.clear();
        assert!(joint_rows(&state.core).is_empty());
        let _ = three_d_panel(&state, &t);
    }

    #[test]
    fn icon_controls_build_live_and_inert_forms() {
        // Element construction is pure (a press needs a
        // Runtime), so this pins that both forms of the
        // Phase I migration construct: live → `icon_btn`,
        // dim → `icon_btn_inert`. The dimming condition is
        // the caller's `dim` flag, unchanged. (Phase N
        // retired the local `ictl` dispatch wrapper; this
        // test — formerly `ictl_builds_live_and_inert_forms`
        // — now builds the family members the call sites
        // dispatch to directly, under the same keys.)
        let t = rune::tok::theme(true);
        let _ = icon_btn("k-live", "floppy-disk", "Save pose", false, &t, |_| {});
        let _ = icon_btn_inert("k-dim", "floppy-disk", "Save pose", &t);
        let _ = icon_btn_inert("k-dim-mirror", MIRROR_ICON, "Mirror pose", &t);
        let _ = icon_btn_inert("k-dim-apply", "check", "Apply", &t);
    }

    // ---- Preview framing + zoom (Phase M) ----

    #[test]
    fn frame_button_glyph_is_in_the_icon_table() {
        // Like MIRROR_ICON: the pinned glyph must exist
        // in the catalogue, so neither the live button
        // nor its inert twin degrades to a glyph-less
        // box.
        assert_eq!(FRAME_ICON, "expand");
        assert!(crate::rune_sec::icons::has_icon(FRAME_ICON));
    }

    #[test]
    fn zoom_distance_steps_in_and_out_by_the_factor() {
        // dir > 0 zooms in (closer), dir < 0 back out;
        // 1.25 is exact in binary, so the round trip is
        // exact too.
        assert_eq!(zoom_distance(300.0, 1), 240.0);
        assert_eq!(zoom_distance(300.0, -1), 375.0);
        assert_eq!(zoom_distance(zoom_distance(300.0, 1), -1), 300.0);
        // No direction, no move.
        assert_eq!(zoom_distance(300.0, 0), 300.0);
    }

    #[test]
    fn zoom_distance_clamps_to_the_framing_range() {
        assert_eq!(zoom_distance(4000.0, -1), ZOOM_DISTANCE_MAX);
        assert_eq!(zoom_distance(3900.0, -1), ZOOM_DISTANCE_MAX);
        assert_eq!(zoom_distance(40.0, 1), ZOOM_DISTANCE_MIN);
        assert_eq!(zoom_distance(41.0, 1), ZOOM_DISTANCE_MIN);
        // A huge finite distance steps without escaping
        // the range: the product saturates at the cap.
        assert_eq!(zoom_distance(1.0e30, -1), ZOOM_DISTANCE_MAX);
        assert_eq!(zoom_distance(1.0e30, 1), ZOOM_DISTANCE_MAX);
    }

    #[test]
    fn zoom_distance_non_finite_is_the_preview_default() {
        assert_eq!(zoom_distance(f32::NAN, 1), ZOOM_FALLBACK_DISTANCE);
        assert_eq!(zoom_distance(f32::INFINITY, -1), ZOOM_FALLBACK_DISTANCE);
        assert_eq!(zoom_distance(f32::NEG_INFINITY, 1), ZOOM_FALLBACK_DISTANCE);
        // The fallback is preview_camera's distance —
        // the preview's own default — pinned so the two
        // cannot drift apart.
        assert_eq!(ZOOM_FALLBACK_DISTANCE, fig3d::preview_camera().distance);
    }

    #[test]
    fn frame_preview_pulls_a_far_camera_in_and_keeps_the_orbit() {
        let mut core = EditorCore::new();
        core.enter_3d();
        // Park the camera far away, off-target, on a
        // distinctive orbit: Frame may move target and
        // distance only.
        {
            let st = core.fig3.as_mut().unwrap();
            st.camera.distance = 3000.0;
            st.camera.target = [0.0, 0.0, 0.0];
            st.camera.yaw = 33.0;
            st.camera.pitch = 12.0;
        }
        let fov_before = core.fig3.as_ref().unwrap().camera.fov_deg;
        assert!(apply_frame_preview(&mut core));
        assert_eq!(core.status, "3D preview framed on the figure.");
        let cam = &core.fig3.as_ref().unwrap().camera;
        assert!(
            cam.distance < 3000.0,
            "framing must pull the camera in: {cam:?}"
        );
        assert!(cam.distance >= ZOOM_DISTANCE_MIN);
        assert_eq!((cam.yaw, cam.pitch, cam.fov_deg), (33.0, 12.0, fov_before));
        // The target is the evaluated figure's centroid.
        let st = core.fig3.as_ref().unwrap();
        let points = fig3d::evaluate3(&st.figure, &st.pose);
        let n = points.len() as f32;
        let centroid = [
            points.iter().map(|p| p[0]).sum::<f32>() / n,
            points.iter().map(|p| p[1]).sum::<f32>() / n,
            points.iter().map(|p| p[2]).sum::<f32>() / n,
        ];
        for axis in 0..3 {
            assert!(
                (cam.target[axis] - centroid[axis]).abs() < 1.0,
                "target {cam:?} vs centroid {centroid:?}"
            );
        }
    }

    #[test]
    fn frame_preview_and_zoom_refuse_without_a_session() {
        let mut core = EditorCore::new();
        assert!(!apply_frame_preview(&mut core));
        assert_eq!(core.status, "Enter 3D mode first.");
        assert!(!apply_preview_zoom(&mut core, 1));
        assert!(!apply_preview_zoom(&mut core, -1));
        assert_eq!(core.status, "Enter 3D mode first.");
    }

    #[test]
    fn preview_zoom_steps_the_session_distance() {
        let mut core = EditorCore::new();
        core.enter_3d();
        // A fresh session's default camera sits at 400.
        assert_eq!(core.fig3.as_ref().unwrap().camera.distance, 400.0);
        assert!(apply_preview_zoom(&mut core, 1));
        assert_eq!(core.fig3.as_ref().unwrap().camera.distance, 320.0);
        assert_eq!(core.status, "3D camera distance 320.00.");
        assert!(apply_preview_zoom(&mut core, -1));
        assert_eq!(core.fig3.as_ref().unwrap().camera.distance, 400.0);
    }

    // ---- Round-2 readout audit (Wave 10) ----

    #[test]
    fn pitch_cannot_accumulate_past_the_orbit_clamp() {
        // The azimuth needed a display wrap because
        // `fig3_orbit` accumulates yaw without bound.
        // Pitch must NOT need one: the same op clamps
        // the STORED pitch to ±89.9° on every move, so
        // no sequence of orbits — button presses or
        // preview drags, which call the same op — can
        // grow it, and the readout's raw print is safe.
        let mut core = EditorCore::new();
        core.enter_3d();
        core.fig3_orbit(0.0, -500.0);
        assert_eq!(core.fig3.as_ref().unwrap().camera.pitch, -89.9);
        assert!(
            camera_readout_text(&core)
                .unwrap()
                .contains("elevation -89.9°")
        );
        core.fig3_orbit(0.0, 1000.0);
        assert_eq!(core.fig3.as_ref().unwrap().camera.pitch, 89.9);
        assert!(
            camera_readout_text(&core)
                .unwrap()
                .contains("elevation 89.9°")
        );
    }

    #[test]
    fn zoom_readout_saturates_at_the_clamped_range() {
        // The zoom half of the audit: the only panel
        // path that steps the distance clamps it, so
        // repeated steps saturate at the range ends
        // instead of accumulating — and the readout
        // (and the zoom status line) print the clamped
        // value, never a runaway one.
        let mut core = EditorCore::new();
        core.enter_3d();
        for _ in 0..40 {
            assert!(apply_preview_zoom(&mut core, -1));
        }
        assert_eq!(
            core.fig3.as_ref().unwrap().camera.distance,
            ZOOM_DISTANCE_MAX
        );
        assert!(
            camera_readout_text(&core).unwrap().ends_with("distance 4000.00")
        );
        for _ in 0..40 {
            assert!(apply_preview_zoom(&mut core, 1));
        }
        assert_eq!(
            core.fig3.as_ref().unwrap().camera.distance,
            ZOOM_DISTANCE_MIN
        );
        assert!(camera_readout_text(&core).unwrap().ends_with("distance 40.00"));
    }

    #[test]
    fn panel_builds_with_a_long_named_library_figure() {
        // Library entry names are file data with no
        // length limit (a hand-made or renamed entry
        // can carry a name far wider than the card).
        // Save the session figure under a very long
        // name so BOTH library lists carry it — the
        // figure list verbatim, the pose list as
        // "<name> pose" — then build the panel: the
        // summary (which wraps) and both library rows
        // (whose names truncate) must construct with
        // the long name in them. Element text is not
        // readable back from a built `El`, so — like
        // the zero-node build test — this pins
        // construction, and the pure summary helper
        // pins that the name really flows in.
        let mut state = crate::init();
        let t = rune::tok::theme(true);
        state.core.enter_3d();
        let long_name = "A".repeat(180);
        state.core.fig3.as_mut().unwrap().figure.name = long_name.clone();
        assert!(
            session_summary(&state.core).starts_with(&long_name),
            "the summary carries the loaded figure's name"
        );
        state.core.fig3_save_to_library();
        assert!(state.core.fig3_library_list().contains(&long_name));
        assert!(save_current_pose(&mut state.core));
        assert!(
            fig3d::scan_pose_library().contains(&format!("{long_name} pose")),
            "the pose list carries the long name too"
        );
        let _ = three_d_panel(&state, &t);
        let _ = preview_el(&state, &t);
    }
}
