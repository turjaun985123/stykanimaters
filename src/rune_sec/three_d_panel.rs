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
//!
//! Controls that need a live session render **dimmed but
//! wired** when there is none: the press still fires, and the
//! op/helper answers "Enter 3D mode first." in the status
//! line, so the muted look never hides a dead button.
//!
//! Element keys reuse the old GPUI control ids where one
//! existed (`mode-3d`, `bake-3d`, …) so headless `--tree`
//! checks keep finding them.

use rune::tok::{type_, Theme};
use rune::{col, el, row, text, El};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::fig3d;

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

fn section_label(text_: &str, t: &Theme) -> El {
    text(text_.to_string())
        .font(type_::LABEL)
        .color(t.text_secondary)
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
        )
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
        )
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
            .child(section_label("Figure library", t).grow(1.0))
            .child(ctl("save-3d", "Save current", false, dim, t, |s| {
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
                .child(ctl(
                    &format!("fig3-lib-{i}"),
                    "Load",
                    false,
                    false,
                    t,
                    move |s| s.core.fig3_load_from_library(&load_name),
                )),
        );
    }

    // Pose library: ./library3d/poses via fig3d's scan;
    // Apply sets the session pose (dimmed without one).
    panel = panel.child(section_label("Pose library", t));
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
                .child(ctl(
                    &format!("pose3d-lib-{i}"),
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
        let cam = core.fig3.as_ref().unwrap().camera;
        assert_eq!(cam, fig3d::camera_preset("side").unwrap());
        assert!(apply_frame_camera(&mut core));
        let framed = core.fig3.as_ref().unwrap().camera;
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
}
