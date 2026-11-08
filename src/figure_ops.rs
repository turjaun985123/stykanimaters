//! Figure operations promoted from the Rune shell into the
//! engine (Rune migration Phase E): setting a figure's colour
//! and duplicating a figure.
//!
//! Both began as dispatch helpers in the Phase D figure
//! inspector (`src/rune_sec/inspector_figure.rs`), because
//! the core offered no op for either. They live here now so
//! every surface — shell, MCP, scripts — shares one
//! implementation. The bookkeeping is the helpers', unchanged:
//!
//! * [`EditorCore::set_figure_color`] is a bounds-checked
//!   field write plus a status line, with **no snapshot** —
//!   the same shape as `set_node_color` (editor.rs) and the
//!   MCP layer's figure-colour write.
//! * [`EditorCore::duplicate_figure`] mirrors
//!   `delete_figure`'s bookkeeping (editor.rs) run in
//!   reverse, plus the index shifts an insertion requires.
//! * [`EditorCore::recolor_figure`] is the whole-figure
//!   counterpart of the per-node colour setters: one
//!   snapshot, then every node's colour written directly.
//! * [`EditorCore::mirror_figure_all_frames`] applies the
//!   transform `mirror_selected_pose` (editor.rs) performs
//!   on one frame — every angle negated, the root left in
//!   place — to the figure's pose on every frame, under
//!   one snapshot.
//!
//! Bookkeeping scope: duplication inserts cloned pose rows
//! (and bend rows, where a frame stores them) into every
//! frame, remaps every stored figure index in the project
//! (see [`remap_figures_after_insert`]), shifts the session's
//! solo target above the insertion point, and selects the
//! copy. The remap helpers below are the one deliberate pass
//! over the whole census of figure-index-bearing fields:
//!
//! * `Project::particles` — `ParticleEvent::figure: Option<usize>`;
//! * `Project::fx_clips` — `FxClipEntry::figure: usize`
//!   (model.rs; the anchor lives on the entry, not inside
//!   `vfx::ParticleClip`);
//! * `Project::channels` — `graph::Channel::figure: usize`;
//! * `Project::camera_follow: Option<usize>`.
//!
//! Fields checked and found NOT to carry a figure index:
//! `FigureInstance::clip` (a movieclip id), `ClipDef`
//! (self-contained def), `SpriteInstance::asset` (an asset
//! index), `TextInstance`, `AudioClip`, `Stroke`. Session
//! state is not the helpers' business: `EditorCore::solo`
//! is shifted by the ops themselves, and `selected_figure`
//! is set explicitly. `delete_figure` (editor.rs) wires the
//! removal helper with a one-line call — see
//! [`remap_figures_after_remove`].
//!
//! Reordering is covered by [`remap_figures_after_move`] and
//! its per-index core [`remap_index_after_move`]: a move
//! drops nothing, so every reference simply travels with
//! its figure to the figure's new index.

use crate::editor::EditorCore;
use crate::model::{Pose, Project};

/// Remap every stored figure index in `project` after a new
/// figure was inserted at `new_index` (the index the new
/// figure now occupies). References at or above `new_index`
/// named the figure that used to sit there — and everything
/// above it — so they shift up by one; references below it
/// are untouched. In particular a reference equal to the
/// *duplicated* figure's own index (`new_index - 1` for
/// `duplicate_figure`, which inserts the copy right after
/// the original) keeps naming the original.
pub fn remap_figures_after_insert(project: &mut Project, new_index: usize) {
    for e in &mut project.particles {
        if let Some(f) = e.figure {
            if f >= new_index {
                e.figure = Some(f + 1);
            }
        }
    }
    for entry in &mut project.fx_clips {
        if entry.figure >= new_index {
            entry.figure += 1;
        }
    }
    for ch in &mut project.channels {
        if ch.figure >= new_index {
            ch.figure += 1;
        }
    }
    if let Some(f) = project.camera_follow {
        if f >= new_index {
            project.camera_follow = Some(f + 1);
        }
    }
}

/// Remap every stored figure index in `project` after the
/// figure at `removed` was deleted. References above
/// `removed` shift down by one. References TO the removed
/// figure are resolved per type, because their anchor is
/// gone:
///
/// * particle events anchored to it are dropped (the same
///   rule `delete_figure` already applied to particles);
/// * FX clip entries anchored to it are dropped —
///   `FxClipEntry::figure` is a plain `usize`, so there is
///   no "unanchored" value to fall back to, and a clip whose
///   anchor node no longer exists has no position to draw
///   at;
/// * graph channels targeting it are dropped — the channel
///   drove that figure's property and has nothing left to
///   drive (the graph evaluator tolerates stale channels,
///   but keeping dead ones would silently accumulate);
/// * `camera_follow` becomes `None` — following nothing is
///   the field's own "off" state.
///
/// Integrator note: `delete_figure` (editor.rs) gains the
/// full remap by replacing its `particles.retain(...)` block
/// with `crate::figure_ops::remap_figures_after_remove(&mut
/// self.project, fi);` after the figure/pose removal (plus
/// the session solo shift: `if self.solo == Some(fi) {
/// self.solo = None } else if let Some(s) = self.solo { if
/// s > fi { self.solo = Some(s - 1) } }`).
pub fn remap_figures_after_remove(project: &mut Project, removed: usize) {
    project.particles.retain(|e| e.figure != Some(removed));
    for e in &mut project.particles {
        if let Some(f) = e.figure {
            if f > removed {
                e.figure = Some(f - 1);
            }
        }
    }
    project.fx_clips.retain(|entry| entry.figure != removed);
    for entry in &mut project.fx_clips {
        if entry.figure > removed {
            entry.figure -= 1;
        }
    }
    project.channels.retain(|ch| ch.figure != removed);
    for ch in &mut project.channels {
        if ch.figure > removed {
            ch.figure -= 1;
        }
    }
    match project.camera_follow {
        Some(f) if f == removed => project.camera_follow = None,
        Some(f) if f > removed => project.camera_follow = Some(f - 1),
        _ => {}
    }
}

/// The figure index a stored reference takes after the
/// figure at `from` is moved to `to` — the permutation a
/// remove-at-`from`, insert-at-`to` reorder applies to the
/// figures vec. A reference to the moved figure becomes
/// `to`; references strictly between `from` and `to` shift
/// by one toward `from` (those figures slide into the
/// vacated slot); every other reference is unchanged. An
/// adjacent swap is this same permutation with
/// `to == from ± 1`, so callers that swap neighbours use it
/// too. This is the same mapping `layers::remapped_index`
/// applies inside `layers::move_figure`, kept here so the
/// session state below and the project pass share one
/// definition.
///
/// Editor session state that stores a figure index —
/// `EditorCore::solo`, and a selection an op does not set
/// explicitly — is remapped by applying this fn to the
/// stored value; it takes plain indices because that state
/// lives on `EditorCore`, not on `Project`.
pub fn remap_index_after_move(index: usize, from: usize, to: usize) -> usize {
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

/// Remap every stored figure index in `project` after the
/// figure at `from` was moved to `to` (the caller performs
/// the reorder itself — figures vec plus the parallel
/// per-frame pose/bend rows — this pass only fixes the
/// references). Unlike removal, nothing is dropped: every
/// anchor keeps its figure, just at the figure's new index,
/// via [`remap_index_after_move`]. `from == to` and indices
/// outside the figures vec are no-ops, mirroring the move
/// itself being refused there.
///
/// Integrator note: `EditorCore::move_figure` (editor.rs),
/// which currently swaps the figures/poses and remaps
/// nothing, gains the full remap with one call after its
/// swap — `crate::figure_ops::remap_figures_after_move(&mut
/// self.project, fi, target);` — plus the session fix-up
/// `if let Some(s) = self.solo { self.solo =
/// Some(crate::figure_ops::remap_index_after_move(s, fi,
/// target)); }` (its `selected_figure` is already assigned
/// `target` directly). `layers::move_figure` already
/// applies the same permutation internally, so
/// `move_figure_full` needs no change.
pub fn remap_figures_after_move(project: &mut Project, from: usize, to: usize) {
    let n = project.figures.len();
    if from == to || from >= n || to >= n {
        return;
    }
    for e in &mut project.particles {
        if let Some(f) = e.figure {
            e.figure = Some(remap_index_after_move(f, from, to));
        }
    }
    for entry in &mut project.fx_clips {
        entry.figure = remap_index_after_move(entry.figure, from, to);
    }
    for ch in &mut project.channels {
        ch.figure = remap_index_after_move(ch.figure, from, to);
    }
    if let Some(f) = project.camera_follow {
        project.camera_follow = Some(remap_index_after_move(f, from, to));
    }
}

/// Mirror one pose in place, with exactly the transform
/// `EditorCore::mirror_selected_pose` (editor.rs) applies
/// to a single frame's pose: every joint angle is negated
/// and the root is left where it is. (An earlier guess
/// that the core op also mirrors the root across the
/// stage centre is wrong — its own test asserts the root
/// must not move.) Kept as a free helper so the
/// all-frames op below shares the one definition of the
/// transform instead of drifting from the core op.
fn mirror_pose_in_place(pose: &mut Pose) {
    for a in &mut pose.angles {
        *a = -*a;
    }
}

impl EditorCore {
    /// Set figure `fi`'s colour directly (the colour picker
    /// panels). Writes the same `FigureInstance.color` the
    /// add-figure ops assign from the `FIGURE_COLORS` cycle;
    /// like `set_node_color` it does not snapshot, so a
    /// colour change is not an undo step. An out-of-range
    /// `fi` only sets the status line.
    pub fn set_figure_color(&mut self, fi: usize, color: u32) {
        let Some(fig) = self.project.figures.get_mut(fi) else {
            self.status = "No such figure.".into();
            return;
        };
        fig.color = color;
        self.status = format!("Figure {} colour changed.", fi + 1);
    }

    /// Duplicate figure `fi`: insert a clone of the instance
    /// right after the original, with the original's pose
    /// (and bend row, where one is stored) cloned into every
    /// frame at the same index, so the parallel per-frame
    /// tables stay parallel. Stored figure indices above the
    /// insertion point shift by one — particle anchors in
    /// the project, and the session's solo target — so they
    /// keep pointing at the same figure. The copy is named
    /// `"<name> copy"`, becomes the selection (with no node
    /// selected), and its index is returned. A movieclip
    /// figure's copy shares the same clip, like a second
    /// instance of it. Returns `None`, changing nothing but
    /// the status line, when `fi` names no figure.
    pub fn duplicate_figure(&mut self, fi: usize) -> Option<usize> {
        let Some(fig) = self.project.figures.get(fi).cloned() else {
            self.status = "No figure to duplicate.".into();
            return None;
        };
        // Clone the per-frame rows before mutating anything.
        let frame_poses: Vec<Option<Pose>> = self
            .project
            .frames
            .iter()
            .map(|f| f.poses.get(fi).cloned())
            .collect();
        let frame_bends: Vec<Option<Vec<f32>>> = self
            .project
            .frames
            .iter()
            .map(|f| f.bends.get(fi).cloned())
            .collect();
        self.snapshot();
        let mut copy = fig;
        copy.name = format!("{} copy", copy.name);
        self.project.figures.insert(fi + 1, copy);
        for (frame, pose) in self.project.frames.iter_mut().zip(frame_poses) {
            if let Some(p) = pose {
                frame.poses.insert(fi + 1, p);
            }
        }
        for (frame, bends) in self.project.frames.iter_mut().zip(frame_bends) {
            if let Some(b) = bends {
                frame.bends.insert(fi + 1, b);
            }
        }
        // Every stored figure index in the project remaps
        // (particles, FX-clip anchors, graph channels,
        // camera follow), so anchors keep pointing at the
        // same figure; the session's solo target shifts here
        // because it is editor state, not project data.
        remap_figures_after_insert(&mut self.project, fi + 1);
        if let Some(solo) = self.solo {
            if solo > fi {
                self.solo = Some(solo + 1);
            }
        }
        self.selected_figure = fi + 1;
        self.selected_node = None;
        self.status = "Figure duplicated.".into();
        Some(fi + 1)
    }

    /// Recolour figure `fi` as a whole: write `color` as
    /// every node's own colour — `NodeDef.color =
    /// Some(color)`, the same field `set_node_color`
    /// (editor.rs) writes one node at a time — under
    /// exactly ONE snapshot, so the recolour is a single
    /// undo step however many nodes the figure has.
    /// `set_node_color` itself never snapshots; this op
    /// snapshots once up front and then writes the field
    /// directly in the loop, so no per-node snapshots
    /// sneak in.
    ///
    /// Only the colour is written. A node's gradient end
    /// colour (`NodeStyle.color2`) is deliberately NOT
    /// recoloured: a gradient segment keeps its end
    /// colour and now runs from `color` to that end. The
    /// figure's own base colour (`FigureInstance.color`)
    /// is likewise left alone — with every node carrying
    /// an explicit colour it colours nothing until a
    /// node's colour is cleared back to `None`.
    ///
    /// Returns `false`, changing nothing but the status
    /// line (the refusal shape `duplicate_figure` uses),
    /// when `fi` names no figure or the figure has no
    /// nodes to recolour; a refusal takes no snapshot.
    pub fn recolor_figure(&mut self, fi: usize, color: u32) -> bool {
        let Some(fig) = self.project.figures.get(fi) else {
            self.status = "No figure to recolour.".into();
            return false;
        };
        if fig.def.nodes.is_empty() {
            self.status = "Figure has no segments to recolour.".into();
            return false;
        }
        self.snapshot();
        for node in &mut self.project.figures[fi].def.nodes {
            node.color = Some(color);
        }
        self.status = format!("Figure {} recoloured.", fi + 1);
        true
    }

    /// Mirror figure `fi` on EVERY frame in the project,
    /// as one undo step. Each frame's pose for the figure
    /// is transformed with the same maths
    /// `mirror_selected_pose` (editor.rs) uses for a
    /// single frame — see [`mirror_pose_in_place`]: every
    /// angle negated, the root left in place. Exactly one
    /// snapshot is taken up front, so a single undo
    /// restores every frame's original pose.
    ///
    /// Frames whose pose table stores no row for the
    /// figure are skipped, matching `duplicate_figure`'s
    /// tolerance for ragged per-frame rows.
    ///
    /// Limit, shared with `mirror_selected_pose`: graph
    /// channels are NOT touched. An `Angle` channel
    /// overrides its joint's angle when a frame is
    /// evaluated, so a figure driven by graph channels
    /// will not visually mirror — the stored poses flip,
    /// the evaluated poses do not. Mirroring channel keys
    /// would be a graph op, not a figure op, and is
    /// deliberately out of scope here.
    ///
    /// Returns `false`, changing nothing but the status
    /// line (the refusal shape `recolor_figure` uses),
    /// when `fi` names no figure; a refusal takes no
    /// snapshot.
    pub fn mirror_figure_all_frames(&mut self, fi: usize) -> bool {
        if fi >= self.project.figures.len() {
            self.status = "No figure to mirror.".into();
            return false;
        }
        self.snapshot();
        let frame_count = self.project.frames.len();
        for frame in &mut self.project.frames {
            if let Some(pose) = frame.poses.get_mut(fi) {
                mirror_pose_in_place(pose);
            }
        }
        self.status = format!(
            "Figure {} mirrored on all {} frames.",
            fi + 1,
            frame_count
        );
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ParticleEvent, ParticleKind};

    #[test]
    fn set_figure_color_writes_the_figure() {
        let mut core = EditorCore::new();
        let fi = core.selected_figure;
        core.set_figure_color(fi, 0x2686E8);
        assert_eq!(core.project.figures[fi].color, 0x2686E8);
        assert!(core.status.contains("colour changed"));
    }

    #[test]
    fn set_figure_color_guards_a_stale_index() {
        let mut core = EditorCore::new();
        let before = core.project.figures[0].color;
        core.set_figure_color(999, 0x2686E8);
        assert_eq!(core.status, "No such figure.");
        assert_eq!(core.project.figures[0].color, before);
    }

    #[test]
    fn recolor_figure_colours_every_node_as_one_undo_step() {
        let mut core = EditorCore::new();
        let fi = core.selected_figure;
        let before: Vec<Option<u32>> = core.project.figures[fi]
            .def
            .nodes
            .iter()
            .map(|n| n.color)
            .collect();
        assert!(before.len() > 1, "the demo figure has several nodes");
        let depth = core.undo_depth();
        assert!(core.recolor_figure(fi, 0xE8862E));
        assert_eq!(
            core.undo_depth(),
            depth + 1,
            "exactly one snapshot for the whole figure"
        );
        assert!(core.project.figures[fi]
            .def
            .nodes
            .iter()
            .all(|n| n.color == Some(0xE8862E)));
        assert_eq!(core.status, format!("Figure {} recoloured.", fi + 1));
        // A single undo restores every node's original
        // colour — the proof the writes shared one snapshot.
        core.undo();
        let after: Vec<Option<u32>> = core.project.figures[fi]
            .def
            .nodes
            .iter()
            .map(|n| n.color)
            .collect();
        assert_eq!(after, before);
    }

    #[test]
    fn recolor_figure_keeps_gradient_end_colours() {
        let mut core = EditorCore::new();
        let fi = core.selected_figure;
        core.project.figures[fi].def.nodes[0].style.color2 = Some(0x112233);
        assert!(core.recolor_figure(fi, 0x445566));
        assert_eq!(core.project.figures[fi].def.nodes[0].color, Some(0x445566));
        assert_eq!(
            core.project.figures[fi].def.nodes[0].style.color2,
            Some(0x112233),
            "the gradient end colour is not part of a recolour"
        );
    }

    #[test]
    fn recolor_figure_guards_a_stale_index_without_a_snapshot() {
        let mut core = EditorCore::new();
        let depth = core.undo_depth();
        assert!(!core.recolor_figure(999, 0xE8862E));
        assert_eq!(core.undo_depth(), depth, "a refusal takes no snapshot");
        assert_eq!(core.status, "No figure to recolour.");
    }

    #[test]
    fn recolor_figure_refuses_a_figure_with_no_nodes() {
        let mut core = EditorCore::new();
        core.project.figures[0].def.nodes.clear();
        let depth = core.undo_depth();
        assert!(!core.recolor_figure(0, 0xE8862E));
        assert_eq!(core.undo_depth(), depth, "a refusal takes no snapshot");
        assert_eq!(core.status, "Figure has no segments to recolour.");
    }

    #[test]
    fn mirror_figure_all_frames_mirrors_every_frame_as_one_undo_step() {
        let mut core = EditorCore::new();
        let fi = core.selected_figure;
        // Record every frame's pose for the figure before
        // the mirror; the demo project carries a pose row
        // for it on each frame.
        let before: Vec<Pose> = core
            .project
            .frames
            .iter()
            .map(|f| f.poses[fi].clone())
            .collect();
        assert!(before.len() > 1, "the demo project has several frames");
        assert!(
            before.iter().all(|p| !p.angles.is_empty()),
            "the demo figure has angles to negate"
        );
        let depth = core.undo_depth();
        assert!(core.mirror_figure_all_frames(fi));
        assert_eq!(
            core.undo_depth(),
            depth + 1,
            "exactly one snapshot for the whole figure"
        );
        for (frame, orig) in core.project.frames.iter().zip(&before) {
            let pose = &frame.poses[fi];
            assert_eq!(
                pose.root, orig.root,
                "mirroring negates angles only — the root stays put, as in mirror_selected_pose"
            );
            for (m, b) in pose.angles.iter().zip(&orig.angles) {
                assert_eq!(*m, -*b, "every angle on every frame is negated");
            }
        }
        assert_eq!(
            core.status,
            format!("Figure {} mirrored on all {} frames.", fi + 1, before.len())
        );
        // A single undo restores every frame's original
        // pose — the proof the writes shared one snapshot.
        core.undo();
        for (frame, orig) in core.project.frames.iter().zip(&before) {
            assert_eq!(frame.poses[fi].angles, orig.angles);
            assert_eq!(frame.poses[fi].root, orig.root);
        }
    }

    #[test]
    fn mirror_figure_all_frames_skips_frames_without_a_pose_row() {
        let mut core = EditorCore::new();
        let fi = core.selected_figure;
        // Make the pose table ragged: drop the figure's
        // pose row from the last frame.
        let last = core.project.frames.len() - 1;
        core.project.frames[last].poses.remove(fi);
        let before: Vec<Vec<f32>> = core
            .project
            .frames
            .iter()
            .map(|f| {
                f.poses
                    .get(fi)
                    .map(|p| p.angles.clone())
                    .unwrap_or_default()
            })
            .collect();
        assert!(core.mirror_figure_all_frames(fi));
        assert!(
            core.project.frames[last].poses.get(fi).is_none(),
            "a frame with no pose row stays without one"
        );
        for (idx, frame) in core.project.frames.iter().enumerate() {
            if let Some(pose) = frame.poses.get(fi) {
                for (m, b) in pose.angles.iter().zip(&before[idx]) {
                    assert_eq!(*m, -*b);
                }
            }
        }
    }

    #[test]
    fn mirror_figure_all_frames_guards_a_stale_index_without_a_snapshot() {
        let mut core = EditorCore::new();
        let depth = core.undo_depth();
        assert!(!core.mirror_figure_all_frames(999));
        assert_eq!(core.undo_depth(), depth, "a refusal takes no snapshot");
        assert_eq!(core.status, "No figure to mirror.");
    }

    #[test]
    fn duplicate_figure_inserts_a_selected_copy_with_its_poses() {
        let mut core = EditorCore::new();
        let n = core.project.figures.len();
        let fi = core.selected_figure;
        let orig = core.project.figures[fi].clone();
        assert_eq!(core.duplicate_figure(fi), Some(fi + 1));
        assert_eq!(core.project.figures.len(), n + 1);
        let copy = &core.project.figures[fi + 1];
        assert_eq!(copy.name, format!("{} copy", orig.name));
        assert_eq!(copy.color, orig.color);
        assert_eq!(copy.def.nodes.len(), orig.def.nodes.len());
        assert_eq!(copy.opacity, orig.opacity);
        // The original is untouched and every frame's pose
        // table stays parallel to the figure list.
        assert_eq!(core.project.figures[fi].name, orig.name);
        for frame in &core.project.frames {
            assert_eq!(frame.poses.len(), core.project.figures.len());
        }
        if let Some(frame) = core.project.frames.first() {
            assert_eq!(
                frame.poses[fi + 1].angles,
                frame.poses[fi].angles,
                "the copy starts from the original's pose"
            );
        }
        assert_eq!(core.selected_figure, fi + 1);
        assert!(core.selected_node.is_none());
        assert_eq!(core.status, "Figure duplicated.");
    }

    #[test]
    fn duplicate_figure_without_figures_is_a_no_op() {
        let mut core = EditorCore::new();
        core.project.figures.clear();
        assert_eq!(core.duplicate_figure(0), None);
        assert!(core.project.figures.is_empty());
        assert_eq!(core.status, "No figure to duplicate.");
    }

    #[test]
    fn duplicate_figure_guards_a_stale_index() {
        let mut core = EditorCore::new();
        let n = core.project.figures.len();
        assert_eq!(core.duplicate_figure(999), None);
        assert_eq!(core.project.figures.len(), n);
        assert_eq!(core.status, "No figure to duplicate.");
    }

    fn fx_entry(figure: usize) -> crate::model::FxClipEntry {
        crate::model::FxClipEntry {
            clip: crate::vfx::ParticleClip {
                start: 0.0,
                duration: 6.0,
                kind: crate::vfx::ClipKind::Burst,
                color: 0xFFCC33,
                color2: None,
                seed: 7,
                angle: 0.0,
                length: 130.0,
            },
            figure,
            node: 0,
            hidden: false,
        }
    }

    fn channel(figure: usize) -> crate::graph::Channel {
        crate::graph::Channel {
            figure,
            target: crate::graph::ChannelTarget::Angle(0),
            keys: Vec::new(),
        }
    }

    fn particle(figure: Option<usize>) -> ParticleEvent {
        ParticleEvent {
            frame: 0,
            figure,
            node: None,
            x: 0.0,
            y: 0.0,
            kind: ParticleKind::Burst,
            color: 0xFFFFFF,
            seed: 0,
        }
    }

    #[test]
    fn duplicate_figure_remaps_fx_channels_and_camera_follow() {
        let mut core = EditorCore::new();
        core.add_figure();
        assert_eq!(core.project.figures.len(), 2);
        // Anchors on the later figure (shift), on the
        // duplicated figure itself (stay).
        core.project.fx_clips = vec![fx_entry(1), fx_entry(0)];
        core.project.channels = vec![channel(1), channel(0)];
        core.project.camera_follow = Some(1);
        assert_eq!(core.duplicate_figure(0), Some(1));
        assert_eq!(core.project.fx_clips[0].figure, 2);
        assert_eq!(core.project.fx_clips[1].figure, 0);
        assert_eq!(core.project.channels[0].figure, 2);
        assert_eq!(core.project.channels[1].figure, 0);
        assert_eq!(core.project.camera_follow, Some(2));
        // Following the duplicated figure itself stays put.
        core.project.camera_follow = Some(0);
        assert_eq!(core.duplicate_figure(0), Some(1));
        assert_eq!(core.project.camera_follow, Some(0));
    }

    #[test]
    fn remap_after_remove_drops_dead_anchors_and_shifts_the_rest() {
        let mut project = Project::new_demo();
        let fig = project.figures[0].clone();
        project.figures.push(fig.clone());
        project.figures.push(fig);
        project.particles = vec![
            particle(Some(0)),
            particle(Some(1)),
            particle(Some(2)),
            particle(None),
        ];
        project.fx_clips = vec![fx_entry(0), fx_entry(1), fx_entry(2)];
        project.channels = vec![channel(0), channel(1), channel(2)];
        project.camera_follow = Some(2);
        remap_figures_after_remove(&mut project, 1);
        // Anchored to the removed figure: dropped everywhere.
        // Above it: shifted down. Below / unanchored: untouched.
        let pfigs: Vec<Option<usize>> = project.particles.iter().map(|e| e.figure).collect();
        assert_eq!(pfigs, vec![Some(0), Some(1), None]);
        let fxfigs: Vec<usize> = project.fx_clips.iter().map(|e| e.figure).collect();
        assert_eq!(fxfigs, vec![0, 1]);
        let chfigs: Vec<usize> = project.channels.iter().map(|c| c.figure).collect();
        assert_eq!(chfigs, vec![0, 1]);
        assert_eq!(project.camera_follow, Some(1));
        // Following the removed figure itself: follow turns off.
        project.camera_follow = Some(1);
        remap_figures_after_remove(&mut project, 1);
        assert_eq!(project.camera_follow, None);
    }

    #[test]
    fn remap_after_insert_shifts_at_and_above_only() {
        let mut project = Project::new_demo();
        project.particles = vec![particle(Some(0)), particle(Some(1))];
        project.fx_clips = vec![fx_entry(0), fx_entry(1)];
        project.channels = vec![channel(0), channel(1)];
        project.camera_follow = Some(0);
        remap_figures_after_insert(&mut project, 1);
        assert_eq!(project.particles[0].figure, Some(0));
        assert_eq!(project.particles[1].figure, Some(2));
        assert_eq!(project.fx_clips[0].figure, 0);
        assert_eq!(project.fx_clips[1].figure, 2);
        assert_eq!(project.channels[0].figure, 0);
        assert_eq!(project.channels[1].figure, 2);
        assert_eq!(project.camera_follow, Some(0));
    }

    #[test]
    fn duplicate_figure_shifts_particle_anchors_and_solo() {
        let mut core = EditorCore::new();
        core.add_figure();
        assert_eq!(core.project.figures.len(), 2);
        // Anchors on the figure above the insertion point, on
        // the duplicated figure itself, and on no figure.
        for figure in [Some(1), Some(0), None] {
            core.project.particles.push(ParticleEvent {
                frame: 0,
                figure,
                node: None,
                x: 0.0,
                y: 0.0,
                kind: ParticleKind::Burst,
                color: 0xFFFFFF,
                seed: 0,
            });
        }
        core.solo = Some(1);
        assert_eq!(core.duplicate_figure(0), Some(1));
        assert_eq!(core.project.particles[0].figure, Some(2));
        assert_eq!(core.project.particles[1].figure, Some(0));
        assert_eq!(core.project.particles[2].figure, None);
        assert_eq!(core.solo, Some(2));
    }

    #[test]
    fn remap_index_after_move_matches_the_move_permutation() {
        // Move 1 -> 3 in a five-figure project: the moved
        // figure lands at 3, figures 2 and 3 slide down one.
        let fwd: Vec<usize> = (0..5).map(|i| remap_index_after_move(i, 1, 3)).collect();
        assert_eq!(fwd, vec![0, 3, 1, 2, 4]);
        // The inverse move undoes it exactly.
        let back: Vec<usize> = fwd
            .iter()
            .map(|&i| remap_index_after_move(i, 3, 1))
            .collect();
        assert_eq!(back, vec![0, 1, 2, 3, 4]);
        // Adjacent swap: only the two neighbours exchange.
        let swap: Vec<usize> = (0..4).map(|i| remap_index_after_move(i, 2, 3)).collect();
        assert_eq!(swap, vec![0, 1, 3, 2]);
        // from == to is the identity, and a reference outside
        // the moved span is untouched either way.
        assert_eq!(remap_index_after_move(2, 2, 2), 2);
        assert_eq!(remap_index_after_move(9, 1, 3), 9);
        assert_eq!(remap_index_after_move(0, 1, 3), 0);
    }

    /// A four-figure project whose figures carry distinct
    /// names, with every census field referencing figures
    /// across the whole range (plus one unanchored particle),
    /// and frames whose poses are distinct per figure so any
    /// accidental frame write shows up in a snapshot compare.
    fn named_project() -> Project {
        let mut project = Project::new_demo();
        let proto = project.figures[0].clone();
        project.figures = ["alpha", "bravo", "charlie", "delta"]
            .iter()
            .map(|name| {
                let mut fig = proto.clone();
                fig.name = (*name).into();
                fig
            })
            .collect();
        for (fr, frame) in project.frames.iter_mut().enumerate() {
            frame.poses = (0..4)
                .map(|i| Pose {
                    root: [i as f32 * 10.0, fr as f32],
                    angles: vec![i as f32 + 0.5],
                })
                .collect();
        }
        project.particles = vec![
            particle(Some(0)),
            particle(Some(1)),
            particle(Some(2)),
            particle(Some(3)),
            particle(None),
        ];
        project.fx_clips = vec![fx_entry(3), fx_entry(0), fx_entry(2), fx_entry(1)];
        project.channels = vec![channel(1), channel(3), channel(0), channel(2)];
        project.camera_follow = Some(2);
        project
    }

    /// The NAME each stored reference resolves to, in field
    /// order — identity, not indices, so a test can prove a
    /// reference still names the same figure after a move.
    fn ref_names(
        project: &Project,
    ) -> (
        Vec<Option<String>>,
        Vec<String>,
        Vec<String>,
        Option<String>,
    ) {
        let name = |f: usize| project.figures[f].name.clone();
        (
            project
                .particles
                .iter()
                .map(|e| e.figure.map(&name))
                .collect(),
            project.fx_clips.iter().map(|e| name(e.figure)).collect(),
            project.channels.iter().map(|c| name(c.figure)).collect(),
            project.camera_follow.map(&name),
        )
    }

    /// (root, angles) of every pose in every frame.
    fn frame_snapshot(project: &Project) -> Vec<Vec<([f32; 2], Vec<f32>)>> {
        project
            .frames
            .iter()
            .map(|f| f.poses.iter().map(|p| (p.root, p.angles.clone())).collect())
            .collect()
    }

    #[test]
    fn remap_after_move_keeps_every_reference_on_its_figure() {
        // first->last, last->first, middle->middle, and both
        // directions of an adjacent swap.
        for (from, to) in [(0, 3), (3, 0), (1, 2), (2, 1), (2, 3)] {
            let mut project = named_project();
            let before = ref_names(&project);
            let frames_before = frame_snapshot(&project);
            // The caller's half of a move: reorder the
            // figures vec; the helper fixes the references.
            let fig = project.figures.remove(from);
            project.figures.insert(to, fig);
            remap_figures_after_move(&mut project, from, to);
            assert_eq!(
                ref_names(&project),
                before,
                "move {from} -> {to} must keep every reference on its figure"
            );
            assert_eq!(
                frame_snapshot(&project),
                frames_before,
                "the remap pass must not touch frame data"
            );
        }
        // The simulation itself: moving 0 -> 3 rotates the
        // vec, it does not swap the ends.
        let mut project = named_project();
        let fig = project.figures.remove(0);
        project.figures.insert(3, fig);
        let order: Vec<&str> = project.figures.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(order, vec!["bravo", "charlie", "delta", "alpha"]);
    }

    #[test]
    fn remap_after_move_no_ops_leave_references_untouched() {
        let raw_refs = |p: &Project| {
            (
                p.particles.iter().map(|e| e.figure).collect::<Vec<_>>(),
                p.fx_clips.iter().map(|e| e.figure).collect::<Vec<_>>(),
                p.channels.iter().map(|c| c.figure).collect::<Vec<_>>(),
                p.camera_follow,
            )
        };
        // from == to, target out of range, source out of
        // range: the raw indices must be byte-identical.
        for (from, to) in [(1, 1), (0, 9), (9, 0), (4, 4)] {
            let mut project = named_project();
            let before = raw_refs(&project);
            remap_figures_after_move(&mut project, from, to);
            assert_eq!(raw_refs(&project), before, "move {from} -> {to} is a no-op");
        }
    }
}
