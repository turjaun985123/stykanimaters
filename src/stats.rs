//! Project statistics: pure, read-only analysis over a
//! [`Project`] — the counts and timing an inspector, an export
//! summary or a project browser can show without touching the
//! editor. Nothing here mutates, and degenerate input (no
//! figures, no frames, a zero fps) reports zeros instead of
//! panicking or dividing by zero.

use crate::model::{norm_deg, Pose, Project};

/// A root move larger than this (stage units) counts as a pose
/// change. Same threshold as the dopesheet in `ui_dope.rs`.
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
/// vec. Consistent with the dopesheet's rule in `ui_dope.rs`,
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
            },
            figure: 0,
            node: 0,
        });
        p.audio.push(AudioClip {
            path: "clip.wav".into(),
            start_frame: 0,
        });
        p.strokes.push(Stroke {
            frame: 0,
            points: vec![[0.0, 0.0], [10.0, 10.0]],
            color: 0xFFFFFF,
            width: 2.0,
            widths: Vec::new(),
            fill: None,
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
