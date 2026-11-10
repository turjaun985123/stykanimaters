//! Project statistics: pure, read-only analysis over a
//! [`Project`] — the counts and timing an inspector, an export
//! summary or a project browser can show without touching the
//! editor. Nothing here mutates, and degenerate input (no
//! figures, no frames, a zero fps) reports zeros instead of
//! panicking or dividing by zero.

use crate::model::{norm_deg, Pose, Project};

/// A root move larger than this (stage units) counts as a pose
/// change. Same threshold as the dopesheet in
/// `rune_sec/dopesheet.rs`.
const ROOT_EPS: f32 = 0.5;
/// A single-node angle change larger than this (degrees,
/// shortest path around the circle) counts as a pose change.
const ANGLE_EPS: f32 = 0.01;

/// Headline numbers for a project, computed by [`project_stats`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProjectStats {
    /// Figures on the timeline (`Project::figures`).
    pub figures: usize,
    /// Authored frames (`Project::frames`).
    pub frames: usize,
    /// Timeline length in seconds: the frames' hold multipliers
    /// summed, divided by the project fps (see [`project_stats`]).
    pub duration_secs: f32,
    /// Sum over every frame of every pose's angle count — the
    /// total number of authored joint angles in the project.
    pub total_angle_keys: usize,
    /// Figures driven by a movieclip (`FigureInstance::clip` set).
    pub figures_with_clips: usize,
    /// Timed particle clips (`Project::fx_clips`).
    pub fx_clips: usize,
    /// Audio clips placed on the timeline (`Project::audio`).
    pub audio_clips: usize,
    /// Freehand strokes drawn on frames (`Project::strokes`).
    pub strokes: usize,
    /// The largest node count of any figure definition — the
    /// most nodes any single figure in the project uses.
    pub used_nodes_max: usize,
}

/// Compute the headline statistics for `p`.
///
/// Duration: the model carries per-frame hold multipliers
/// (`Frame::duration`, 1.0 = one frame tick, 2.0 = held twice as
/// long — the exporter repeats each frame `duration` times at
/// the project fps), so the timeline runs
/// `sum(frame.duration) / p.fps` seconds. A project with no
/// frames, or a non-positive fps, reports 0.
pub fn project_stats(p: &Project) -> ProjectStats {
    let duration_secs = if p.fps > 0.0 {
        p.frames.iter().map(|f| f.duration).sum::<f32>() / p.fps
    } else {
        0.0
    };
    ProjectStats {
        figures: p.figures.len(),
        frames: p.frames.len(),
        duration_secs,
        total_angle_keys: p
            .frames
            .iter()
            .flat_map(|f| f.poses.iter())
            .map(|pose| pose.angles.len())
            .sum(),
        figures_with_clips: p.figures.iter().filter(|f| f.clip.is_some()).count(),
        fx_clips: p.fx_clips.len(),
        audio_clips: p.audio.len(),
        strokes: p.strokes.len(),
        used_nodes_max: p
            .figures
            .iter()
            .map(|f| f.def.nodes.len())
            .max()
            .unwrap_or(0),
    }
}

/// True when `cur` differs from `prev` by more than the
/// epsilons: root moved, angle count changed, or any one angle
/// moved (shortest path). The same rule the dopesheet uses.
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

/// Frame indices where figure `fi`'s pose changes — its
/// keyframes, in practice. Frame 0 counts when the figure has a
/// pose there (the first authored pose is a key by definition).
/// Poses are positional in each frame's pose vec, so a figure
/// can be *missing* from a frame: a missing frame is never a
/// key, and the frame the figure reappears on is (nothing →
/// something is a change). An out-of-range `fi` yields an empty
/// vec. Consistent with the dopesheet's rule in
/// `rune_sec/dopesheet.rs`,
/// but returns indices instead of a per-frame boolean map.
pub fn figure_keyframes(p: &Project, fi: usize) -> Vec<usize> {
    if fi >= p.figures.len() {
        return Vec::new();
    }
    let mut keys = Vec::new();
    for f in 0..p.frames.len() {
        let cur = p.frames[f].poses.get(fi);
        let is_key = if f == 0 {
            cur.is_some()
        } else {
            match (cur, p.frames[f - 1].poses.get(fi)) {
                (Some(cur), Some(prev)) => pose_changed(prev, cur),
                (Some(_), None) => true,
                (None, _) => false,
            }
        };
        if is_key {
            keys.push(f);
        }
    }
    keys
}

/// Total distance (stage units) figure `fi`'s root travels
/// across the timeline: the sum, over every pair of consecutive
/// frames, of the Euclidean distance between the figure's root
/// positions in those two frames. A pair counts only when the
/// figure has a pose in BOTH frames — poses are positional, so
/// a figure missing from a frame breaks the chain and the pairs
/// touching the gap contribute nothing (no teleport credit for
/// reappearing somewhere else). An out-of-range `fi`, a figure
/// with no poses, or a project with fewer than two frames
/// reports 0.
pub fn root_travel(p: &Project, fi: usize) -> f32 {
    if fi >= p.figures.len() {
        return 0.0;
    }
    let mut total = 0.0f32;
    for pair in p.frames.windows(2) {
        if let (Some(a), Some(b)) = (pair[0].poses.get(fi), pair[1].poses.get(fi)) {
            let dx = b.root[0] - a.root[0];
            let dy = b.root[1] - a.root[1];
            total += (dx * dx + dy * dy).sqrt();
        }
    }
    total
}

/// Index of the figure whose root travels the most across the
/// timeline (see [`root_travel`]), or `None` when the project
/// has no figures. Ties resolve to the lowest index. A project
/// whose figures never move still has a busiest figure — the
/// first one, with travel 0.
pub fn busiest_figure(p: &Project) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for fi in 0..p.figures.len() {
        let travel = root_travel(p, fi);
        // Strictly greater: an equal travel keeps the earlier
        // (lowest) index already recorded.
        if best.map_or(true, |(_, best_travel)| travel > best_travel) {
            best = Some((fi, travel));
        }
    }
    best.map(|(fi, _)| fi)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AudioClip, FigureDef, FigureInstance, Frame, Stroke};
    use crate::vfx::{ClipKind, ParticleClip};

    fn figure(name: &str) -> FigureInstance {
        FigureInstance {
            name: name.into(),
            color: 0xFFFFFF,
            def: FigureDef::humanoid(),
            hidden: false,
            clip: None,
            states: Vec::new(),
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

    fn frame(poses: Vec<Pose>, duration: f32) -> Frame {
        Frame {
            poses,
            duration,
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

    /// Alpha drifts under both thresholds on frame 1, swings an
    /// angle on frame 2, holds on frame 3. Beta is missing from
    /// frame 0 (short pose vec), appears on frame 1, holds on
    /// frame 2, moves its root on frame 3. Durations sum to 4.5
    /// frame ticks; the project's fps is set to 10 below.
    fn two_figure_project() -> Project {
        let alpha = [
            pose(100.0, 10.0),
            pose(100.3, 10.004),
            pose(100.3, 25.0),
            pose(100.3, 25.0),
        ];
        let beta = pose(300.0, 0.0);
        let frames = vec![
            frame(vec![alpha[0].clone()], 1.0),
            frame(vec![alpha[1].clone(), beta.clone()], 2.0),
            frame(vec![alpha[2].clone(), beta.clone()], 1.0),
            frame(vec![alpha[3].clone(), pose(310.0, 0.0)], 0.5),
        ];
        let mut p = project(vec![figure("Alpha"), figure("Beta")], frames);
        p.fps = 10.0;
        p
    }

    #[test]
    fn demo_project_stats_are_sane() {
        let p = Project::new_demo();
        let s = project_stats(&p);
        assert_eq!(s.figures, 1);
        assert_eq!(s.frames, 5);
        // Five frames held 1.0 each at 12 fps.
        assert!((s.duration_secs - 5.0 / 12.0).abs() < 1e-6);
        // One pose per frame, one angle per node of the demo figure.
        assert_eq!(s.total_angle_keys, 5 * p.figures[0].def.nodes.len());
        assert_eq!(s.used_nodes_max, p.figures[0].def.nodes.len());
        assert_eq!(s.figures_with_clips, 0);
        assert_eq!(s.fx_clips, 0);
        assert_eq!(s.audio_clips, 0);
        assert_eq!(s.strokes, 0);
    }

    #[test]
    fn empty_project_reports_zeros() {
        let p = project(Vec::new(), Vec::new());
        let s = project_stats(&p);
        assert_eq!(
            s,
            ProjectStats {
                figures: 0,
                frames: 0,
                duration_secs: 0.0,
                total_angle_keys: 0,
                figures_with_clips: 0,
                fx_clips: 0,
                audio_clips: 0,
                strokes: 0,
                used_nodes_max: 0,
            }
        );
        assert!(figure_keyframes(&p, 0).is_empty());
    }

    #[test]
    fn stats_count_clips_fx_audio_and_strokes() {
        let mut p = two_figure_project();
        p.figures[0].clip = Some(0);
        p.fx_clips.push(crate::model::FxClipEntry {
            clip: ParticleClip {
                start: 0.0,
                duration: 1.0,
                kind: ClipKind::Burst,
                color: 0xFFFFFF,
                seed: 1,
                angle: 0.0,
                length: 10.0,
                color2: None,
            },
            figure: 0,
            node: 0,
            hidden: false,
        });
        p.audio.push(AudioClip {
            path: "clip.wav".into(),
            start_frame: 0,
            gain: 1.0,
            muted: false,
        });
        p.strokes.push(Stroke {
            frame: 0,
            points: vec![[0.0, 0.0], [10.0, 10.0]],
            color: 0xFFFFFF,
            width: 2.0,
            widths: Vec::new(),
            fill: None,
            spray: None,
            opacity: 1.0,
            scatter: 0.0,
        });
        let s = project_stats(&p);
        assert_eq!(s.figures, 2);
        assert_eq!(s.frames, 4);
        // (1 + 2 + 1 + 0.5) ticks at 10 fps.
        assert!((s.duration_secs - 0.45).abs() < 1e-6);
        // 3 angles per pose; poses per frame: 1, 2, 2, 2.
        assert_eq!(s.total_angle_keys, 3 * (1 + 2 + 2 + 2));
        assert_eq!(s.figures_with_clips, 1);
        assert_eq!(s.fx_clips, 1);
        assert_eq!(s.audio_clips, 1);
        assert_eq!(s.strokes, 1);
        assert_eq!(s.used_nodes_max, p.figures[0].def.nodes.len());
    }

    #[test]
    fn keyframes_follow_the_dopesheet_rule() {
        let p = two_figure_project();
        // Alpha: authored at 0, under-threshold drift at 1, real
        // swing at 2, hold at 3.
        assert_eq!(figure_keyframes(&p, 0), vec![0, 2]);
        // Beta: no pose at 0 (not a key), appears at 1, holds at
        // 2, root moves at 3.
        assert_eq!(figure_keyframes(&p, 1), vec![1, 3]);
        // Out-of-range figure: empty, no panic.
        assert!(figure_keyframes(&p, 7).is_empty());
    }

    #[test]
    fn keyframes_on_the_demo_wave() {
        // Every demo frame moves the root and the raised arm, so
        // all five frames are keys, starting at frame 0.
        let p = Project::new_demo();
        assert_eq!(figure_keyframes(&p, 0), vec![0, 1, 2, 3, 4]);
    }
}

#[cfg(test)]
mod motion_tests {
    use super::{busiest_figure, root_travel};
    use crate::model::{FigureDef, FigureInstance, Frame, Pose, Project};

    fn figure(name: &str) -> FigureInstance {
        FigureInstance {
            name: name.into(),
            color: 0xFFFFFF,
            def: FigureDef::humanoid(),
            hidden: false,
            clip: None,
            states: Vec::new(),
            locked: false,
            opacity: 1.0,
        }
    }

    fn at(x: f32, y: f32) -> Pose {
        Pose {
            root: [x, y],
            angles: Vec::new(),
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
    fn root_travel_sums_consecutive_root_distances() {
        // (0,0) -> (3,4) is 5, (3,4) -> (3,4) is 0,
        // (3,4) -> (6,8) is 5: total exactly 10.
        let p = project(
            vec![figure("Walker")],
            vec![
                frame(vec![at(0.0, 0.0)]),
                frame(vec![at(3.0, 4.0)]),
                frame(vec![at(3.0, 4.0)]),
                frame(vec![at(6.0, 8.0)]),
            ],
        );
        assert_eq!(root_travel(&p, 0), 10.0);
    }

    #[test]
    fn root_travel_gap_frames_contribute_nothing() {
        // The figure is missing from frame 1, so the pairs
        // (0,1) and (1,2) count nothing — the reappearance at
        // (30,40) earns no teleport distance. Only (2,3)
        // counts: (3,4) -> (6,8) is 5.
        let p = project(
            vec![figure("Ghost")],
            vec![
                frame(vec![at(0.0, 0.0)]),
                frame(Vec::new()),
                frame(vec![at(3.0, 4.0)]),
                frame(vec![at(6.0, 8.0)]),
            ],
        );
        assert_eq!(root_travel(&p, 0), 5.0);
        // A figure absent from every middle frame and never
        // reappearing travels nothing at all.
        let p = project(
            vec![figure("Ghost")],
            vec![
                frame(vec![at(0.0, 0.0)]),
                frame(Vec::new()),
                frame(vec![at(30.0, 40.0)]),
            ],
        );
        assert_eq!(root_travel(&p, 0), 0.0);
    }

    #[test]
    fn root_travel_degenerate_inputs_are_zero() {
        let empty = project(Vec::new(), Vec::new());
        assert_eq!(root_travel(&empty, 0), 0.0);
        // Out-of-range figure and figure-less frames: no panic.
        let p = project(vec![figure("Solo")], vec![frame(Vec::new())]);
        assert_eq!(root_travel(&p, 0), 0.0);
        assert_eq!(root_travel(&p, 9), 0.0);
    }

    #[test]
    fn busiest_figure_picks_the_greatest_travel() {
        // Figure 0 travels 5, figure 1 travels 10.
        let p = project(
            vec![figure("Calm"), figure("Busy")],
            vec![
                frame(vec![at(0.0, 0.0), at(0.0, 0.0)]),
                frame(vec![at(3.0, 4.0), at(3.0, 4.0)]),
                frame(vec![at(3.0, 4.0), at(6.0, 8.0)]),
            ],
        );
        assert_eq!(root_travel(&p, 0), 5.0);
        assert_eq!(root_travel(&p, 1), 10.0);
        assert_eq!(busiest_figure(&p), Some(1));
    }

    #[test]
    fn busiest_figure_ties_resolve_to_the_lowest_index() {
        // Both figures travel exactly 5: index 0 wins. A
        // stationary project also resolves to its first figure.
        let p = project(
            vec![figure("A"), figure("B")],
            vec![
                frame(vec![at(0.0, 0.0), at(10.0, 10.0)]),
                frame(vec![at(3.0, 4.0), at(13.0, 14.0)]),
            ],
        );
        assert_eq!(busiest_figure(&p), Some(0));
        let still = project(
            vec![figure("A"), figure("B")],
            vec![frame(vec![at(1.0, 1.0), at(2.0, 2.0)])],
        );
        assert_eq!(busiest_figure(&still), Some(0));
    }

    #[test]
    fn busiest_figure_empty_project_is_none() {
        let p = project(Vec::new(), Vec::new());
        assert_eq!(busiest_figure(&p), None);
        // Frames without any figures are likewise None.
        let p = project(Vec::new(), vec![frame(Vec::new()), frame(Vec::new())]);
        assert_eq!(busiest_figure(&p), None);
    }
}
