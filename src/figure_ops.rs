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
}
