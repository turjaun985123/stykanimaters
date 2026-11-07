//! Layer operations: solo / lock / opacity / ordering semantics over
//! [`Project`].
//!
//! Pure functions on the document model — no editor or GPUI types —
//! so the sidebar, the renderer and tests all share one definition of
//! what "visible", "editable" and "on top" mean. Draw order is the
//! order of `Project::figures` (index 0 is drawn first, i.e. at the
//! back).
//!
//! # Permutation audit for [`move_figure`]
//!
//! Reordering figures is only safe if every figure-indexed structure
//! in [`Project`] is permuted or remapped identically — poses are
//! stored per figure index in every frame, so moving a figure in
//! `figures` without moving its poses would silently attach each
//! pose to the wrong figure. Audited against `model.rs`:
//!
//! | Field | Figure-indexed? | What `move_figure` does |
//! |---|---|---|
//! | `figures` | the vector itself | remove + insert |
//! | `frames[].poses` | yes, one `Pose` per figure per frame | same remove + insert per frame (indices past the end of a short pose vec are skipped defensively) |
//! | `camera_follow` | yes, `Option<usize>` | remapped |
//! | `particles[].figure` | yes, `Option<usize>` | remapped when `Some`; `node` is a node index *within* that figure and is unchanged |
//! | `fx_clips[].figure` | yes | remapped; `node` unchanged; `ParticleClip` itself carries no figure index |
//! | `channels[].figure` | yes (`graph::Channel`) | remapped; `ChannelTarget` payloads are node indices and are unchanged |
//! | `camera` | no — keys are frame indices | untouched |
//! | `strokes[].frame` | no — a frame index | untouched |
//! | `clips` + `FigureInstance::clip` | no — clip indices, and the reference lives on the instance, so it travels with the figure | untouched |
//! | `assets`, `sprites`, `texts` | no — asset indices / free-floating instances, unrelated to figure order | untouched |
//! | `audio`, `weather`, scalars | no | untouched |
//!
//! Indices held *outside* the project (the editor's selected figure,
//! a `solo` argument, …) are the caller's responsibility; use
//! [`remapped_index`] with the same `from`/`to` to translate them.

use crate::model::Project;

/// The layer opacity bounds: a figure can be made faint, never fully
/// invisible through opacity alone (that is what `hidden` is for).
pub const MIN_OPACITY: f32 = 0.05;
pub const MAX_OPACITY: f32 = 1.0;

/// Where the element at `index` ends up when the element at `from`
/// is moved to `to` (remove + insert semantics): the moved element
/// lands on `to`, elements strictly between `from` and `to` shift by
/// one toward `from`, everything else — including stale indices past
/// the moved range — is unchanged.
pub fn remapped_index(index: usize, from: usize, to: usize) -> usize {
    if index == from {
        to
    } else if from < to && index > from && index <= to {
        index - 1
    } else if to < from && index >= to && index < from {
        index + 1
    } else {
        index
    }
}

/// Effective visibility of every figure, in figure order.
///
/// A figure is visible when it is not `hidden` and either no solo is
/// active or it *is* the solo figure. (`locked` has no effect on
/// visibility.) Note that soloing a hidden figure hides everything —
/// the hidden flag always wins.
pub fn visible_figures(project: &Project, solo: Option<usize>) -> Vec<bool> {
    project
        .figures
        .iter()
        .enumerate()
        .map(|(i, f)| !f.hidden && solo.map_or(true, |s| s == i))
        .collect()
}

/// Flip a figure's `locked` flag. Returns the new state, or `None`
/// when `fi` is not a figure index.
pub fn toggle_lock(project: &mut Project, fi: usize) -> Option<bool> {
    let fig = project.figures.get_mut(fi)?;
    fig.locked = !fig.locked;
    Some(fig.locked)
}

/// Set a figure's layer opacity, clamped to
/// [`MIN_OPACITY`]..=[`MAX_OPACITY`]. Returns `false` when `fi` is
/// not a figure index. NaN is treated as fully opaque rather than
/// stored (it would poison every blend it touched).
pub fn set_opacity(project: &mut Project, fi: usize, opacity: f32) -> bool {
    let Some(fig) = project.figures.get_mut(fi) else {
        return false;
    };
    fig.opacity = if opacity.is_nan() {
        MAX_OPACITY
    } else {
        opacity.clamp(MIN_OPACITY, MAX_OPACITY)
    };
    true
}

/// Cycle a figure's opacity through 1.0 -> 0.6 -> 0.3 -> 1.0 (the
/// sidebar's one-button opacity control). A value between steps
/// snaps to the next step down. Returns the new opacity, or `None`
/// when `fi` is not a figure index.
pub fn cycle_opacity(project: &mut Project, fi: usize) -> Option<f32> {
    let fig = project.figures.get_mut(fi)?;
    fig.opacity = if fig.opacity > 0.9 {
        0.6
    } else if fig.opacity > 0.45 {
        0.3
    } else {
        1.0
    };
    Some(fig.opacity)
}

/// Move the figure at `fi` by `delta` slots (negative = toward the
/// back / index 0), clamped to the ends of the figure list, keeping
/// every figure-indexed structure consistent — see the module docs
/// for the full audit. Returns `true` when the figure actually
/// changed position; `false` for an out-of-range `fi` or a move
/// that clamps back onto the figure's current slot.
pub fn move_figure(project: &mut Project, fi: usize, delta: i32) -> bool {
    let n = project.figures.len();
    if fi >= n {
        return false;
    }
    let to = (fi as i64 + delta as i64).clamp(0, n as i64 - 1) as usize;
    if to == fi {
        return false;
    }

    let fig = project.figures.remove(fi);
    project.figures.insert(to, fig);

    // Poses are per-figure-indexed in every frame: permute identically.
    for frame in &mut project.frames {
        if fi < frame.poses.len() {
            let pose = frame.poses.remove(fi);
            let at = to.min(frame.poses.len());
            frame.poses.insert(at, pose);
        }
    }

    if let Some(follow) = &mut project.camera_follow {
        *follow = remapped_index(*follow, fi, to);
    }
    for event in &mut project.particles {
        if let Some(f) = &mut event.figure {
            *f = remapped_index(*f, fi, to);
        }
    }
    for entry in &mut project.fx_clips {
        entry.figure = remapped_index(entry.figure, fi, to);
    }
    for channel in &mut project.channels {
        channel.figure = remapped_index(channel.figure, fi, to);
    }
    true
}

/// The first figure that can be selected and posed: neither `hidden`
/// nor `locked`. For selection fix-ups after a hide/lock/reorder.
pub fn first_editable(project: &Project) -> Option<usize> {
    project.figures.iter().position(|f| !f.hidden && !f.locked)
}

/// The opacity a figure should actually be drawn with: 0 when it is
/// not visible under [`visible_figures`] (or `fi` is out of range),
/// otherwise its layer opacity.
pub fn effective_opacity(project: &Project, fi: usize, solo: Option<usize>) -> f32 {
    let Some(fig) = project.figures.get(fi) else {
        return 0.0;
    };
    if fig.hidden || solo.map_or(false, |s| s != fi) {
        0.0
    } else {
        fig.opacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ParticleEvent, ParticleKind};

    /// Three figures (Styk, B, C) whose frame poses have distinct
    /// root x per figure, so a botched permutation is visible.
    fn three_figures() -> Project {
        let mut p = Project::new_demo();
        p.add_figure("B", 0xFF0000, [200.0, 200.0]);
        p.add_figure("C", 0x00FF00, [300.0, 200.0]);
        for frame in &mut p.frames {
            for (i, pose) in frame.poses.iter_mut().enumerate() {
                pose.root = [100.0 * (i as f32 + 1.0), 50.0];
            }
        }
        p
    }

    fn names(p: &Project) -> Vec<&str> {
        p.figures.iter().map(|f| f.name.as_str()).collect()
    }

    fn roots(p: &Project, frame: usize) -> Vec<f32> {
        p.frames[frame].poses.iter().map(|pose| pose.root[0]).collect()
    }

    #[test]
    fn remapped_index_covers_all_cases() {
        // Move 0 -> 2 in a 4-list: 0→2, 1→0, 2→1, 3→3.
        assert_eq!(remapped_index(0, 0, 2), 2);
        assert_eq!(remapped_index(1, 0, 2), 0);
        assert_eq!(remapped_index(2, 0, 2), 1);
        assert_eq!(remapped_index(3, 0, 2), 3);
        // Move 3 -> 1: 3→1, 1→2, 2→3, 0→0.
        assert_eq!(remapped_index(3, 3, 1), 1);
        assert_eq!(remapped_index(1, 3, 1), 2);
        assert_eq!(remapped_index(2, 3, 1), 3);
        assert_eq!(remapped_index(0, 3, 1), 0);
        // Stale indices pass through untouched.
        assert_eq!(remapped_index(99, 0, 2), 99);
    }

    #[test]
    fn move_figure_permutates_poses_with_figures() {
        let mut p = three_figures();
        assert_eq!(names(&p), ["Styk", "B", "C"]);
        assert_eq!(roots(&p, 0), [100.0, 200.0, 300.0]);

        // Styk moves one slot toward the front: order B, Styk, C.
        assert!(move_figure(&mut p, 0, 1));
        assert_eq!(names(&p), ["B", "Styk", "C"]);
        for f in 0..p.frames.len() {
            assert_eq!(roots(&p, f), [200.0, 100.0, 300.0], "frame {f}");
        }

        // And straight back again restores the original pairing.
        assert!(move_figure(&mut p, 1, -1));
        assert_eq!(names(&p), ["Styk", "B", "C"]);
        assert_eq!(roots(&p, 0), [100.0, 200.0, 300.0]);
    }

    #[test]
    fn move_figure_references_follow_the_figure() {
        let mut p = three_figures();
        p.camera_follow = Some(2); // follows C
        p.channels.push(crate::graph::Channel {
            figure: 2, // on C
            target: crate::graph::ChannelTarget::RootX,
            keys: Vec::new(),
        });
        p.fx_clips.push(crate::model::FxClipEntry {
            clip: crate::vfx::ParticleClip {
                start: 0.0,
                duration: 8.0,
                kind: crate::vfx::ClipKind::Burst,
                color: 0xFFFFFF,
                seed: 1,
                angle: 0.0,
                length: 100.0,
            },
            figure: 0, // on Styk
            node: 2,
        });
        p.particles.push(ParticleEvent {
            frame: 0,
            figure: Some(1), // on B
            node: Some(3),
            x: 0.0,
            y: 0.0,
            kind: ParticleKind::Burst,
            color: 0xFFFFFF,
            seed: 0,
        });
        p.particles.push(ParticleEvent {
            frame: 0,
            figure: None, // fixed stage point: must not move
            node: None,
            x: 10.0,
            y: 10.0,
            kind: ParticleKind::Sparkle,
            color: 0xFFFFFF,
            seed: 0,
        });

        // Styk (0) jumps to the front (2): B and C shift down one.
        assert!(move_figure(&mut p, 0, 2));
        assert_eq!(names(&p), ["B", "C", "Styk"]);
        assert_eq!(roots(&p, 0), [200.0, 300.0, 100.0]);
        assert_eq!(p.camera_follow, Some(1), "C moved 2 -> 1");
        assert_eq!(p.channels[0].figure, 1, "C's channel moved 2 -> 1");
        assert_eq!(p.fx_clips[0].figure, 2, "Styk's fx clip moved 0 -> 2");
        assert_eq!(p.fx_clips[0].node, 2, "node indices do not remap");
        assert_eq!(p.particles[0].figure, Some(0), "B moved 1 -> 0");
        assert_eq!(p.particles[1].figure, None);
    }

    #[test]
    fn move_figure_clamps_and_rejects_noops() {
        let mut p = three_figures();
        // A huge positive delta clamps to the last slot and moves.
        assert!(move_figure(&mut p, 0, 99));
        assert_eq!(names(&p), ["B", "C", "Styk"]);
        // Already at the end: further positive moves are no-ops.
        assert!(!move_figure(&mut p, 2, 1));
        assert!(!move_figure(&mut p, 2, 0));
        // Back at the start, negative moves clamp to a no-op too.
        assert!(move_figure(&mut p, 2, -99));
        assert!(!move_figure(&mut p, 0, -1));
        // Out-of-range index.
        assert!(!move_figure(&mut p, 7, 1));
        assert_eq!(names(&p), ["Styk", "B", "C"]);
    }

    #[test]
    fn solo_visibility_matrix() {
        let mut p = three_figures();
        p.figures[1].hidden = true;
        assert_eq!(visible_figures(&p, None), [true, false, true]);
        assert_eq!(visible_figures(&p, Some(0)), [true, false, false]);
        assert_eq!(visible_figures(&p, Some(2)), [false, false, true]);
        // Soloing the hidden figure shows nothing: hidden always wins.
        assert_eq!(visible_figures(&p, Some(1)), [false, false, false]);
        // Locking does not affect visibility.
        p.figures[0].locked = true;
        assert_eq!(visible_figures(&p, None), [true, false, true]);
        // A stale solo index hides everything rather than panicking.
        assert_eq!(visible_figures(&p, Some(9)), [false, false, false]);
    }

    #[test]
    fn lock_and_opacity_ops() {
        let mut p = three_figures();
        assert_eq!(toggle_lock(&mut p, 1), Some(true));
        assert!(p.figures[1].locked);
        assert_eq!(toggle_lock(&mut p, 1), Some(false));
        assert_eq!(toggle_lock(&mut p, 9), None);

        assert!(set_opacity(&mut p, 0, 0.6));
        assert_eq!(p.figures[0].opacity, 0.6);
        assert!(set_opacity(&mut p, 0, 4.0));
        assert_eq!(p.figures[0].opacity, 1.0, "clamped to max");
        assert!(set_opacity(&mut p, 0, 0.0));
        assert_eq!(p.figures[0].opacity, MIN_OPACITY, "clamped to min");
        assert!(set_opacity(&mut p, 0, f32::NAN));
        assert_eq!(p.figures[0].opacity, 1.0, "NaN becomes opaque");
        assert!(!set_opacity(&mut p, 9, 0.5));

        p.figures[2].opacity = 1.0;
        assert_eq!(cycle_opacity(&mut p, 2), Some(0.6));
        assert_eq!(cycle_opacity(&mut p, 2), Some(0.3));
        assert_eq!(cycle_opacity(&mut p, 2), Some(1.0));
        assert_eq!(cycle_opacity(&mut p, 9), None);
    }

    #[test]
    fn first_editable_skips_hidden_and_locked() {
        let mut p = three_figures();
        assert_eq!(first_editable(&p), Some(0));
        p.figures[0].locked = true;
        assert_eq!(first_editable(&p), Some(1));
        p.figures[1].hidden = true;
        assert_eq!(first_editable(&p), Some(2));
        p.figures[2].locked = true;
        assert_eq!(first_editable(&p), None);
    }

    #[test]
    fn effective_opacity_combines_visibility_and_opacity() {
        let mut p = three_figures();
        p.figures[0].opacity = 0.6;
        assert_eq!(effective_opacity(&p, 0, None), 0.6);
        assert_eq!(effective_opacity(&p, 0, Some(0)), 0.6);
        assert_eq!(effective_opacity(&p, 0, Some(1)), 0.0, "not the solo figure");
        p.figures[0].hidden = true;
        assert_eq!(effective_opacity(&p, 0, None), 0.0, "hidden");
        assert_eq!(effective_opacity(&p, 9, None), 0.0, "out of range");
    }
}
