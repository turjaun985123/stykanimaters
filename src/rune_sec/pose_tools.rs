//! Pose tools section for the Rune editor shell (Phase D,
//! section 30/40): mirror / copy / paste / copy-between /
//! reset / graph-bake controls for the selected figure's pose
//! on the current frame. Declared from `main.rs` only — bin
//! crate code, not part of the engine library.
//!
//! INTEGRATOR NOTE — one new `crate::State` field is required
//! (this file does not compile against `State` without it):
//!
//! ```ignore
//! pub pose_clipboard: Option<stykanimate_rs::model::Pose>,
//! ```
//!
//! initialised to `None` in `init()`. The core's own clipboard
//! (`EditorCore::clipboard`, editor.rs:53) is private and holds
//! a whole `Frame`; a pose-scoped clipboard has nowhere to live
//! but the shell state — the same reason `State::wheel_zoom`
//! exists for the camera gesture.
//!
//! Control → behaviour map (every control is real; there is
//! no placeholder in this section):
//!
//! - **Mirror pose** → `EditorCore::mirror_selected_pose`
//!   (editor.rs:1405). This is also the brief's "Flip figure":
//!   no separate flip op exists anywhere in the engine
//!   (verified by grep over editor.rs / fig3d.rs / model.rs),
//!   and mirroring *is* the horizontal flip — it negates every
//!   angle about the vertical axis through the root
//!   (editor.rs:1390-1404). Two buttons calling one op would
//!   be a fake distinction, so there is one button.
//! - **Copy pose** → composed: stores the selected figure's
//!   current-frame pose in `State::pose_clipboard`. No core op
//!   copies a single pose to a clipboard; the body follows
//!   `EditorCore::copy_frame`'s clipboard pattern
//!   (editor.rs:1655) scoped to one pose. Not an undo step,
//!   exactly like `copy_frame`.
//! - **Paste pose** → composed: `EditorCore::snapshot`
//!   (editor.rs:202) + write the clipboard pose over the
//!   selected figure's current-frame pose — one undo step,
//!   mirroring `copy_pose_between`'s structure. Guarded on
//!   angle count: `model::evaluate` indexes `pose.angles[i]`
//!   directly (model.rs:326-330), so pasting a pose from a
//!   differently-rigged figure would panic the renderer; the
//!   paste refuses with a status message instead.
//! - **Copy pose to…** (one button per other figure) →
//!   `EditorCore::copy_pose_between(selected, target)`
//!   (editor.rs:1438), reading the selection live at press
//!   time.
//! - **Reset pose** → composed: `snapshot` + replace the
//!   current pose with `FigureDef::rest_pose(current root)`
//!   (model.rs:279) — the same rest pose a figure is created
//!   with (`Project::add_figure_with`, model.rs:976). The root
//!   position is kept; only the angles reset.
//! - **Bake graph** → `EditorCore::bake_graph_channels(0,
//!   last)` (editor.rs:1360) over the whole timeline — the op
//!   is pose-adjacent: it writes graph-evaluated poses into
//!   real frames, after which they are ordinary editable
//!   poses.
//!
//! Buttons whose op would no-op on the current state render
//! muted (secondary text, no press handler — inert, not fake):
//! mirror/copy/reset need a pose on this frame, paste needs a
//! copied pose *and* a target pose, copy-to needs both poses,
//! bake needs at least one graph channel and one frame.

use rune::tok::{type_, Theme};
use rune::{col, el, row, text, El};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::Pose;

use crate::State;

/// True when figure `fi` has a pose on the current frame —
/// the same existence check `mirror_selected_pose` performs
/// before acting (editor.rs:1411-1416).
pub fn pose_exists(core: &EditorCore, fi: usize) -> bool {
    core.project
        .frames
        .get(core.current)
        .and_then(|f| f.poses.get(fi))
        .is_some()
}

/// The selected figure's pose on the current frame, if any.
pub fn selected_pose(core: &EditorCore) -> Option<&Pose> {
    core.project
        .frames
        .get(core.current)?
        .poses
        .get(core.selected_figure)
}

/// True when the selected figure has a pose here — the gate
/// for Mirror / Copy / Reset.
pub fn has_selected_pose(core: &EditorCore) -> bool {
    pose_exists(core, core.selected_figure)
}

/// Indices of every figure except the selected one — the
/// targets for Copy-pose-to. Empty for a one-figure project.
pub fn pose_targets(core: &EditorCore) -> Vec<usize> {
    (0..core.project.figures.len())
        .filter(|&i| i != core.selected_figure)
        .collect()
}

/// True when Bake graph would do work: the op no-ops with a
/// status message when there are no channels (editor.rs:1364)
/// or no frames (editor.rs:1368).
pub fn can_bake(core: &EditorCore) -> bool {
    !core.project.channels.is_empty() && core.frame_count() > 0
}

/// The selected figure's rest pose at its current root —
/// what Reset pose restores. `None` when there is no pose
/// here (or no such figure).
pub fn rest_pose_for(core: &EditorCore) -> Option<Pose> {
    let pose = selected_pose(core)?;
    let fig = core.project.figures.get(core.selected_figure)?;
    Some(fig.def.rest_pose(pose.root))
}

/// The status text the core itself uses when a pose op finds
/// nothing to act on — `mirror_selected_pose`'s two messages
/// (editor.rs:1408-1424), reused so the composed ops speak
/// with the same voice.
fn no_pose_status(core: &EditorCore) -> String {
    if core.selected_figure >= core.project.figures.len() {
        "No such figure.".to_string()
    } else {
        format!(
            "Figure {} has no pose on frame {}.",
            core.selected_figure + 1,
            core.current + 1
        )
    }
}

/// Display label for figure `j`: its project name, falling
/// back to `Figure {n}` when the name is blank.
fn figure_label(core: &EditorCore, j: usize) -> String {
    match core.project.figures.get(j) {
        Some(f) if !f.name.trim().is_empty() => f.name.clone(),
        _ => format!("Figure {}", j + 1),
    }
}

/// Copy the selected figure's current-frame pose into the
/// shell clipboard. See the module docs for why this is a
/// composition and not a core op.
pub fn copy_pose(core: &mut EditorCore, clip: &mut Option<Pose>) {
    let Some(pose) = selected_pose(core).cloned() else {
        core.status = no_pose_status(core);
        return;
    };
    let (fi, fr) = (core.selected_figure + 1, core.current + 1);
    *clip = Some(pose);
    core.status = format!("Pose copied from figure {fi} on frame {fr}.");
}

/// Paste a previously copied pose onto the selected figure
/// on the current frame, as one undo step. Refuses (status
/// only) when nothing was copied, when there is no target
/// pose, or when the copied pose's angle count does not
/// match this figure's rig — `model::evaluate` indexes
/// angles directly, so a mismatched paste would panic
/// instead of merely looking wrong.
pub fn paste_pose(core: &mut EditorCore, clip: &Option<Pose>) {
    let Some(clip) = clip else {
        core.status = "Nothing copied yet — copy a pose first.".into();
        return;
    };
    let Some(cur) = selected_pose(core) else {
        core.status = no_pose_status(core);
        return;
    };
    if clip.angles.len() != cur.angles.len() {
        core.status = format!(
            "Paste pose failed: copied pose has {} angles, figure {} has {} segments.",
            clip.angles.len(),
            core.selected_figure + 1,
            cur.angles.len()
        );
        return;
    }
    let clip = clip.clone();
    core.snapshot();
    let (fi, fr) = (core.selected_figure, core.current);
    core.project.frames[fr].poses[fi] = clip;
    core.status = format!(
        "Pose pasted onto figure {} on frame {}.",
        fi + 1,
        fr + 1
    );
}

/// Reset the selected figure's current-frame pose to its
/// rest pose (root position kept), as one undo step.
pub fn reset_pose(core: &mut EditorCore) {
    let Some(rest) = rest_pose_for(core) else {
        core.status = no_pose_status(core);
        return;
    };
    core.snapshot();
    let (fi, fr) = (core.selected_figure, core.current);
    core.project.frames[fr].poses[fi] = rest;
    core.status = format!(
        "Figure {} reset to its rest pose on frame {}.",
        fi + 1,
        fr + 1
    );
}

/// One pose-tools button, in the chrome's metrics
/// (`chrome_view::btn`): padded rounded label. When
/// `enabled` is false the button renders muted and carries
/// NO press handler — an inert element, not a control that
/// silently does nothing.
fn pbtn(
    key: String,
    label: &str,
    enabled: bool,
    t: &Theme,
    f: impl Fn(&mut State) + 'static,
) -> El {
    let base = el().key(key).px(10.0).py(5.0).radius(6.0);
    if enabled {
        base.fill(t.card)
            .hover(t.control_track)
            .press(t.control_off)
            .on_press(f)
            .child(text(label).font(type_::LABEL).color(t.text))
    } else {
        base.fill(t.control_off)
            .child(text(label).font(type_::LABEL).color(t.text_secondary))
    }
}

/// The Pose tools section: a labelled group with the pose
/// ops for the selected figure on the current frame.
pub fn pose_tools(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let has_pose = has_selected_pose(core);
    let can_paste = s.pose_clipboard.is_some() && has_pose;
    let caption = if core.selected_figure < core.project.figures.len() {
        figure_label(core, core.selected_figure)
    } else {
        "No figure".to_string()
    };
    let mut body = col()
        .key("pose-tools")
        .gap(6.0)
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(
                    text("Pose tools")
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                )
                .child(el().grow(1.0))
                .child(text(caption).font(type_::LABEL).color(t.text_secondary)),
        )
        .child(
            row()
                .gap(4.0)
                .child(pbtn(
                    "pose-mirror".to_string(),
                    "Mirror pose",
                    has_pose,
                    t,
                    |s| s.core.mirror_selected_pose(),
                ))
                .child(pbtn(
                    "pose-copy".to_string(),
                    "Copy pose",
                    has_pose,
                    t,
                    |s| copy_pose(&mut s.core, &mut s.pose_clipboard),
                ))
                .child(pbtn(
                    "pose-paste".to_string(),
                    "Paste pose",
                    can_paste,
                    t,
                    |s| paste_pose(&mut s.core, &s.pose_clipboard),
                )),
        )
        .child(
            row()
                .gap(4.0)
                .child(pbtn(
                    "pose-reset".to_string(),
                    "Reset pose",
                    has_pose,
                    t,
                    |s| reset_pose(&mut s.core),
                ))
                .child(pbtn(
                    "pose-bake-graph".to_string(),
                    "Bake graph",
                    can_bake(core),
                    t,
                    |s| {
                        let last = s.core.frame_count().saturating_sub(1);
                        s.core.bake_graph_channels(0, last);
                    },
                )),
        );
    let targets = pose_targets(core);
    if !targets.is_empty() {
        body = body.child(
            text("Copy pose to")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
        let mut trow = row().gap(4.0);
        for j in targets {
            let enabled = has_pose && pose_exists(core, j);
            trow = trow.child(pbtn(
                format!("pose-copy-to-{}", j + 1),
                &figure_label(core, j),
                enabled,
                t,
                move |s: &mut State| {
                    s.core.copy_pose_between(s.core.selected_figure, j)
                },
            ));
        }
        body = body.child(trow.scroll_x().w_full());
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    fn core() -> EditorCore {
        EditorCore::new()
    }

    #[test]
    fn demo_core_has_a_selected_pose_and_no_targets() {
        let c = core();
        assert!(has_selected_pose(&c));
        assert!(selected_pose(&c).is_some());
        // The demo project has one figure, so there is
        // nothing to copy a pose *to* yet.
        assert!(pose_targets(&c).is_empty());
    }

    #[test]
    fn bake_is_unavailable_without_channels() {
        let mut c = core();
        c.project.channels.clear();
        assert!(!can_bake(&c));
    }

    #[test]
    fn copy_then_paste_moves_the_pose_to_another_frame() {
        let mut c = core();
        let mut clip = None;
        // Make frame 0's pose distinctive, then copy it.
        c.mirror_selected_pose();
        let mirrored = selected_pose(&c).unwrap().clone();
        copy_pose(&mut c, &mut clip);
        assert!(clip.is_some());
        let last = c.frame_count() - 1;
        assert!(last > 0, "demo project needs at least two frames");
        c.goto(last);
        paste_pose(&mut c, &clip);
        let pasted = selected_pose(&c).unwrap();
        assert_eq!(pasted.angles, mirrored.angles);
        assert_eq!(pasted.root, mirrored.root);
    }

    #[test]
    fn paste_without_copy_is_a_status_only_noop() {
        let mut c = core();
        let before = selected_pose(&c).unwrap().clone();
        paste_pose(&mut c, &None);
        assert_eq!(selected_pose(&c).unwrap().angles, before.angles);
        assert!(
            c.status.contains("Nothing copied"),
            "unexpected status: {}",
            c.status
        );
    }

    #[test]
    fn paste_is_exactly_one_undo_step() {
        let mut c = core();
        let mut clip = None;
        c.mirror_selected_pose();
        copy_pose(&mut c, &mut clip);
        c.mirror_selected_pose(); // back to the original pose
        let orig = selected_pose(&c).unwrap().clone();
        paste_pose(&mut c, &clip);
        assert_ne!(selected_pose(&c).unwrap().angles, orig.angles);
        c.undo();
        assert_eq!(selected_pose(&c).unwrap().angles, orig.angles);
    }

    #[test]
    fn reset_pose_restores_rest_angles_and_keeps_root() {
        let mut c = core();
        c.mirror_selected_pose();
        let root_before = selected_pose(&c).unwrap().root;
        reset_pose(&mut c);
        let p = selected_pose(&c).unwrap();
        let rest = c.project.figures[c.selected_figure].def.rest_pose(p.root);
        assert_eq!(p.angles, rest.angles);
        assert_eq!(p.root, root_before);
    }

    #[test]
    fn targets_exclude_the_selected_figure() {
        let mut c = core();
        c.add_figure();
        c.add_figure();
        // add_figure selects the figure it just added.
        assert_eq!(c.selected_figure, 2);
        assert_eq!(pose_targets(&c), vec![0, 1]);
    }

    #[test]
    fn paste_refuses_a_mismatched_rig() {
        let mut c = core();
        let mut short = selected_pose(&c).unwrap().clone();
        short.angles.clear();
        let clip = Some(short);
        let before = selected_pose(&c).unwrap().clone();
        paste_pose(&mut c, &clip);
        assert_eq!(selected_pose(&c).unwrap().angles, before.angles);
        assert!(
            c.status.contains("Paste pose failed"),
            "unexpected status: {}",
            c.status
        );
    }
}
