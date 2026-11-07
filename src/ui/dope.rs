//! Dopesheet strip for the bottom timeline bar.
//!
//! One slim row per figure; a dot marks every frame where that
//! figure's pose *changes* relative to the previous frame — in
//! practice, the frames an animator would call keyframes. The data
//! half ([`keyframe_map`]) is a pure function over the project; the
//! builder half ([`dopesheet`]) is presentation only, mirroring the
//! frame chips' metrics (min_w 26, px 6, gap 4, rounded 6) so its
//! columns land on the same pitch as the numbered chips below it.

use gpui::*;

use crate::model::{norm_deg, Pose, Project};
use crate::ui_kit::{C_ACCENT, C_BORDER, C_INK, C_MUTED};

/// A root move larger than this (stage units) counts as a change.
const ROOT_EPS: f32 = 0.5;
/// A single-node angle change larger than this (degrees, shortest
/// path around the circle) counts as a change.
const ANGLE_EPS: f32 = 0.01;

/// True when `cur` differs from `prev` by more than the epsilons:
/// root moved, angle-count changed, or any one angle moved.
fn pose_changed(prev: &Pose, cur: &Pose) -> bool {
    let dx = cur.root[0] - prev.root[0];
    let dy = cur.root[1] - prev.root[1];
    if (dx * dx + dy * dy).sqrt() > ROOT_EPS {
        return true;
    }
    if prev.angles.len() != cur.angles.len() {
        return true;
    }
    prev.angles
        .iter()
        .zip(cur.angles.iter())
        .any(|(a, b)| norm_deg(b - a).abs() > ANGLE_EPS)
}

/// Per figure, per frame: does the figure's pose change at that
/// frame? Frame 0 is always `true` for every figure in the project
/// (the first authored pose is a key by definition). A figure
/// missing from a frame's pose vec records `false` there — no pose,
/// no change — and a figure that reappears after missing frames
/// records `true` on the frame it returns (nothing → something is a
/// change). Rows follow `project.figures` order and every row has
/// exactly `project.frames.len()` entries.
pub fn keyframe_map(project: &Project) -> Vec<Vec<bool>> {
    let n_frames = project.frames.len();
    let mut out = Vec::with_capacity(project.figures.len());
    for fi in 0..project.figures.len() {
        let mut row = vec![false; n_frames];
        for (f, slot) in row.iter_mut().enumerate() {
            *slot = if f == 0 {
                true
            } else {
                match (
                    project.frames[f].poses.get(fi),
                    project.frames[f - 1].poses.get(fi),
                ) {
                    (Some(cur), Some(prev)) => pose_changed(prev, cur),
                    (Some(_), None) => true,
                    (None, _) => false,
                }
            };
        }
        out.push(row);
    }
    out
}

/// The strip itself: one 10px row per figure — a muted name
/// (truncated to 8 characters) followed by one cell per frame on the
/// frame chips' pitch. A keyframe shows a 6px fully-rounded accent
/// dot; other frames show the same dot faint in `C_BORDER`. The
/// current frame's cell is highlighted with the accent background,
/// like the current chip (its keyframe dot turns ink so it stays
/// visible on the accent).
pub fn dopesheet(map: &[Vec<bool>], current: usize, names: &[String]) -> Div {
    let mut sheet = div().flex_col().gap(px(2.0));
    for (fi, row) in map.iter().enumerate() {
        let name: String = names
            .get(fi)
            .map(|n| n.chars().take(8).collect())
            .unwrap_or_default();
        let mut cells = div().flex().flex_row().gap(px(4.0));
        for (f, &key) in row.iter().enumerate() {
            let is_current = f == current;
            let dot = if key {
                if is_current {
                    C_INK
                } else {
                    C_ACCENT
                }
            } else {
                C_BORDER
            };
            cells = cells.child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .min_w(px(26.0))
                    .px(px(6.0))
                    .py(px(2.0))
                    .rounded(px(6.0))
                    .bg(rgb(if is_current { C_ACCENT } else { 0x2C313A }))
                    .child(div().w(px(6.0)).h(px(6.0)).rounded(px(999.0)).bg(rgb(dot))),
            );
        }
        sheet = sheet.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.0))
                .child(
                    div()
                        .w(px(56.0))
                        .overflow_hidden()
                        .text_size(px(10.0))
                        .text_color(rgb(C_MUTED))
                        .child(name),
                )
                .child(cells),
        );
    }
    sheet
}

#[cfg(test)]
mod tests {
    use super::keyframe_map;
    use crate::model::{FigureDef, FigureInstance, Frame, Pose, Project};

    fn figure(name: &str) -> FigureInstance {
        FigureInstance {
            name: name.into(),
            color: 0xFFFFFF,
            def: FigureDef::humanoid(),
            hidden: false,
            clip: None,
            locked: false,
            opacity: 1.0,
        }
    }

    fn pose(root_x: f32, angle: f32) -> Pose {
        Pose {
            root: [root_x, 200.0],
            angles: vec![angle, 0.0, 0.0],
        }
    }

    fn frame(poses: Vec<Pose>) -> Frame {
        Frame {
            poses,
            duration: 1.0,
            easing: None,
            bends: Vec::new(),
        }
    }

    fn project(figures: Vec<FigureInstance>, frames: Vec<Frame>) -> Project {
        let mut p = Project::new_demo();
        p.figures = figures;
        p.frames = frames;
        p
    }

    #[test]
    fn keyframe_map_marks_only_real_changes() {
        // Alpha: frame 1 drifts under both thresholds (root +0.3,
        // angle +0.004 deg) so it is NOT a change; frame 2 swings an
        // angle, a real change; frame 3 holds frame 2's pose.
        // Beta never changes at all.
        let alpha = [
            pose(100.0, 10.0),
            pose(100.3, 10.004),
            pose(100.3, 25.0),
            pose(100.3, 25.0),
        ];
        let frames: Vec<Frame> = (0..4)
            .map(|i| frame(vec![alpha[i].clone(), pose(300.0, 0.0)]))
            .collect();
        let p = project(vec![figure("Alpha"), figure("Beta")], frames);
        let map = keyframe_map(&p);
        assert_eq!(map.len(), 2);
        assert_eq!(map[0], vec![true, false, true, false]);
        assert_eq!(map[1], vec![true, false, false, false]);
    }

    #[test]
    fn keyframe_map_handles_missing_poses() {
        // Poses are positional, so a short pose vec drops the LAST
        // figures. Beta is missing on frames 0 and 2, present on 1
        // and 3. Frame 0 is still a key (frame-0 rule), the missing
        // frame 2 reads false, and each reappearance (frames 1 and
        // 3) counts as a change. Alpha holds one pose throughout.
        let a = pose(100.0, 10.0);
        let b = pose(300.0, 0.0);
        let frames = vec![
            frame(vec![a.clone()]),
            frame(vec![a.clone(), b.clone()]),
            frame(vec![a.clone()]),
            frame(vec![a.clone(), b.clone()]),
        ];
        let p = project(vec![figure("Alpha"), figure("Beta")], frames);
        let map = keyframe_map(&p);
        assert_eq!(map[0], vec![true, false, false, false]);
        assert_eq!(map[1], vec![true, true, false, true]);
    }

    #[test]
    fn keyframe_map_empty_timeline_gives_empty_rows() {
        let p = project(vec![figure("Alpha"), figure("Beta")], Vec::new());
        let map = keyframe_map(&p);
        assert_eq!(map, vec![Vec::<bool>::new(), Vec::<bool>::new()]);
    }
}
