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
//! Bookkeeping scope, stated plainly: duplication inserts
//! cloned pose rows (and bend rows, where a frame stores
//! them) into every frame, shifts particle anchors and the
//! session's solo target above the insertion point, and
//! selects the copy. It does **not** remap FX-clip anchors
//! (`Project::fx_clips`) or graph channels that name a
//! figure — neither did the shell helper, and neither does
//! `delete_figure`, so all three figure-structure ops share
//! the same scope. Widening it belongs in one deliberate
//! pass over all of them, not in this promotion.

use crate::editor::EditorCore;
use crate::model::Pose;

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
        // Stored figure indices above the insertion point
        // shift by one, so anchors keep pointing at the same
        // figure: particle anchors in the project, and the
        // session's solo target. (`delete_figure` performs
        // the mirror bookkeeping for its removal.)
        for e in &mut self.project.particles {
            if let Some(f) = e.figure {
                if f > fi {
                    e.figure = Some(f + 1);
                }
            }
        }
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
