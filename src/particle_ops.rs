//! Particle-event engine ops: the `EditorCore` method
//! family for editing the project's particle EVENTS
//! (`Project::particles`) — add, delete, retime, re-anchor
//! and reposition (Phase G, section 4), plus kind and
//! colour (Phase I, section 6).
//!
//! Particle events are the one-shot effects the rasterizer
//! draws from `export::particles_at` (a burst / sparkle /
//! smoke puff at a stored stage point, alive for
//! `[frame, frame + 0.7 * fps]`). Before this module the
//! only op that created one was `EditorCore::add_particle`
//! (in `editor.rs`), which always anchors to the current
//! selection; nothing could delete, retime, retarget or
//! move a single event afterwards — the shell could draw
//! events (and Phase F built gizmo geometry for a selected
//! one) but no panel could manage them. These ops are the
//! engine home the new particles panel calls, written in
//! the `fx_ops.rs` / `audio_ops.rs` house style:
//!
//! * An out-of-range event index changes nothing and
//!   takes NO snapshot — it only sets `status`, in the
//!   failure voice of the neighbouring ops
//!   ("… failed: no particle event N.").
//! * An in-range op snapshots exactly once, then writes
//!   the field directly (there is no backend module for
//!   particle events the way `fx_edit` backs `fx_ops`).
//! * Success is reported on `status` with 1-based event
//!   and frame numbers, like `fx_move_clip` /
//!   `audio_move_last`.
//!
//! Defaults for a newly added event: NO file importer
//! constructs `ParticleEvent`s — `pivot.rs`, `nodes.rs`,
//! `nodes_anim.rs` and `media.rs` all initialise
//! `particles: Vec::new()` — so the defaults mirror the
//! one production construction site,
//! `EditorCore::add_particle` (editor.rs): kind `Burst`
//! (index 0 of its `PARTICLE_KINDS` table) and the seed
//! `(len * 2654435761) + 12345` (wrapping), taken from the
//! event count BEFORE the push. The colour is the one
//! deliberate delta: `add_particle` colours from the
//! figure it anchors to; an unanchored event has no
//! figure, so the colour comes from the currently
//! selected figure instead (falling back to white
//! `0xFFFFFF`, the colour of the free events in the
//! `layers.rs` tests, when the project has no figures).
//!
//! Frame arguments are clamped to the project's last
//! frame: an event placed past the end could never be
//! seen (its live window starts after the timeline ends),
//! and `add_particle`'s frame (`self.current`) is in
//! range by construction, so clamping is the same
//! contract for panel-supplied frames.
//!
//! Position vs anchor: the model stores both. The
//! rasterizer renders from the stored `[x, y]` ALWAYS
//! (`particles_at` reads `ev.x` / `ev.y` and never the
//! anchor); the anchor (`figure` + `node`, both stored)
//! is what the shell's gizmo resolves live
//! (`rune_sec/stage_fx.rs::event_anchor`). So
//! `particle_set_position` moves where the effect renders,
//! and `particle_set_anchor` moves where the editor marks
//! it — both are real, distinct fields, and both ops
//! exist.
//!
//! Integrator wiring: declare `pub mod particle_ops;` in
//! `lib.rs` (alphabetical: after `pub mod paint;`, before
//! `pub mod pivot;`). Nothing else is needed — this file
//! only adds an inherent `impl EditorCore` block.

use crate::editor::EditorCore;
use crate::model::{ParticleEvent, ParticleKind};

impl EditorCore {
    /// Clamp a frame argument to the project's last frame
    /// (0 for a project with no frames — defensive; a real
    /// project always has at least one).
    fn particle_clamp_frame(&self, frame: usize) -> usize {
        frame.min(self.frame_count().saturating_sub(1))
    }

    /// Append a default particle event at `frame` /
    /// `(x, y)`, unanchored (`figure: None`, `node: None`)
    /// — a free stage effect. Defaults mirror
    /// `add_particle` (see the module docs): `Burst` kind,
    /// the selected figure's colour, and the count-based
    /// wrapping seed. `frame` is clamped to the last
    /// frame. One undo step. Returns the new event's
    /// index, so the caller can select what it just made.
    pub fn particle_add_event(&mut self, frame: usize, x: f32, y: f32) -> usize {
        let frame = self.particle_clamp_frame(frame);
        let color = self
            .project
            .figures
            .get(self.selected_figure)
            .map(|f| f.color)
            .unwrap_or(0xFFFFFF);
        let seed = (self.project.particles.len() as u32)
            .wrapping_mul(2654435761)
            .wrapping_add(12345);
        self.snapshot();
        self.project.particles.push(ParticleEvent {
            frame,
            figure: None,
            node: None,
            x,
            y,
            kind: ParticleKind::Burst,
            color,
            seed,
        });
        let index = self.project.particles.len() - 1;
        self.status = format!(
            "Particle event {} added at frame {}.",
            index + 1,
            frame + 1
        );
        index
    }

    /// Delete particle event `index`. One undo step; an
    /// out-of-range index only sets the status.
    pub fn particle_delete_event(&mut self, index: usize) {
        if self.project.particles.get(index).is_none() {
            self.status = format!(
                "Particle event delete failed: no particle event {}.",
                index + 1
            );
            return;
        }
        self.snapshot();
        self.project.particles.remove(index);
        self.status = format!("Particle event {} deleted.", index + 1);
    }

    /// Move particle event `index` to `frame`, clamped to
    /// the project's last frame. One undo step; an
    /// out-of-range index only sets the status.
    pub fn particle_set_frame(&mut self, index: usize, frame: usize) {
        if self.project.particles.get(index).is_none() {
            self.status = format!(
                "Particle event frame failed: no particle event {}.",
                index + 1
            );
            return;
        }
        let frame = self.particle_clamp_frame(frame);
        self.snapshot();
        self.project.particles[index].frame = frame;
        self.status = format!(
            "Particle event {} moved to frame {}.",
            index + 1,
            frame + 1
        );
    }

    /// Set particle event `index`'s anchor: figure
    /// `figure`, joint `node` — the model stores BOTH as
    /// `Option`s (unlike `FxClipEntry`, whose anchor is a
    /// plain pair). Pass `figure: None` to clear the anchor
    /// back to a free stage event; the node is then cleared
    /// too, whatever was passed (a node anchor without a
    /// figure is meaningless — every stored free event in
    /// the codebase has `node: None`). A `node: None` with
    /// a figure anchors to the figure's root (the shell's
    /// `event_anchor` reads it as node 0, and the status
    /// reports it as node 0). Only the event index is
    /// bounds-checked — the panel passes the core's
    /// selected figure/joint, valid by construction, the
    /// same contract as `fx_set_anchor`. One undo step; an
    /// out-of-range index only sets the status.
    pub fn particle_set_anchor(
        &mut self,
        index: usize,
        figure: Option<usize>,
        node: Option<usize>,
    ) {
        if self.project.particles.get(index).is_none() {
            self.status = format!(
                "Particle event anchor failed: no particle event {}.",
                index + 1
            );
            return;
        }
        let node = if figure.is_some() { node } else { None };
        self.snapshot();
        let event = &mut self.project.particles[index];
        event.figure = figure;
        event.node = node;
        self.status = match figure {
            Some(fi) => format!(
                "Particle event {} anchored to fig {} node {}.",
                index + 1,
                fi + 1,
                node.unwrap_or(0)
            ),
            None => format!("Particle event {} anchor cleared.", index + 1),
        };
    }

    /// Move particle event `index`'s stored stage point to
    /// `(x, y)` — the point the rasterizer renders the
    /// effect from, independent of the anchor (see the
    /// module docs). One undo step; an out-of-range index
    /// only sets the status.
    pub fn particle_set_position(&mut self, index: usize, x: f32, y: f32) {
        if self.project.particles.get(index).is_none() {
            self.status = format!(
                "Particle event position failed: no particle event {}.",
                index + 1
            );
            return;
        }
        self.snapshot();
        let event = &mut self.project.particles[index];
        event.x = x;
        event.y = y;
        self.status = format!(
            "Particle event {} moved to ({x:.0}, {y:.0}).",
            index + 1
        );
    }

    /// Set particle event `index`'s kind (burst / sparkle
    /// / smoke) — the effect shape the rasterizer draws
    /// for the event's live window (Phase I, section 6).
    /// One undo step; an out-of-range index only sets the
    /// status.
    pub fn particle_set_kind(&mut self, index: usize, kind: ParticleKind) {
        if self.project.particles.get(index).is_none() {
            self.status = format!(
                "Particle event kind failed: no particle event {}.",
                index + 1
            );
            return;
        }
        self.snapshot();
        self.project.particles[index].kind = kind;
        self.status = format!(
            "Particle event {} kind set to {kind:?}.",
            index + 1
        );
    }

    /// Set particle event `index`'s colour (`0xRRGGBB`)
    /// — the tint the rasterizer draws every particle of
    /// the event in (Phase I, section 6). The status
    /// wording mirrors `fx_set_color` in `fx_ops.rs`. One
    /// undo step; an out-of-range index only sets the
    /// status.
    pub fn particle_set_color(&mut self, index: usize, color: u32) {
        if self.project.particles.get(index).is_none() {
            self.status = format!(
                "Particle event colour failed: no particle event {}.",
                index + 1
            );
            return;
        }
        self.snapshot();
        self.project.particles[index].color = color;
        self.status = format!(
            "Particle event {} colour set to #{color:06X}.",
            index + 1
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Event construction for tests: distinct frame /
    /// position / seed per event so removals and edits are
    /// identifiable, in the field order of the model.
    fn event(frame: usize, x: f32, seed: u32) -> ParticleEvent {
        ParticleEvent {
            frame,
            figure: None,
            node: None,
            x,
            y: 0.0,
            kind: ParticleKind::Burst,
            color: 0xFFFFFF,
            seed,
        }
    }

    fn core_with(events: Vec<ParticleEvent>) -> EditorCore {
        let mut core = EditorCore::new();
        core.project.particles = events;
        core
    }

    /// The whole project as its snapshot JSON — the exact
    /// bytes `snapshot()` would store. `Project` has no
    /// `PartialEq`, so this is how "untouched" is asserted
    /// (the `fx_ops` pattern).
    fn project_json(core: &EditorCore) -> String {
        serde_json::to_string(&core.project).unwrap()
    }

    #[test]
    fn add_event_lands_with_add_particle_defaults() {
        let mut core = core_with(vec![]);
        core.project.figures[0].color = 0xFF5D5D;
        core.selected_figure = 0;
        let edits = core.edits_since_autosave();
        let index = core.particle_add_event(1, 120.0, 240.0);
        assert_eq!(index, 0);
        assert_eq!(core.project.particles.len(), 1);
        let ev = &core.project.particles[0];
        assert_eq!(ev.frame, 1);
        assert_eq!(ev.figure, None, "added events are unanchored");
        assert_eq!(ev.node, None);
        assert_eq!((ev.x, ev.y), (120.0, 240.0));
        // The defaults `add_particle` (editor.rs) builds:
        // kind index 0 of PARTICLE_KINDS, the figure's
        // colour, and seed (len * 2654435761) + 12345 with
        // len = 0 before the push.
        assert_eq!(ev.kind, ParticleKind::Burst);
        assert_eq!(ev.color, 0xFF5D5D);
        assert_eq!(ev.seed, 12345);
        assert_eq!(core.status, "Particle event 1 added at frame 2.");
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
    }

    #[test]
    fn add_event_seed_counts_existing_events_and_frame_clamps() {
        let mut core = core_with(vec![event(0, 0.0, 1)]);
        let last = core.frame_count() - 1;
        let index = core.particle_add_event(usize::MAX, 5.0, 6.0);
        assert_eq!(index, 1);
        let ev = &core.project.particles[1];
        // len was 1 before the push:
        // 1 * 2654435761 + 12345 (wrapping), as add_particle.
        assert_eq!(ev.seed, 2654435761u32.wrapping_add(12345));
        assert_eq!(ev.frame, last, "frame clamps to the last frame");
        assert_eq!(
            core.status,
            format!("Particle event 2 added at frame {}.", last + 1)
        );
    }

    #[test]
    fn delete_event_removes_the_right_one_and_undo_restores() {
        let mut core = core_with(vec![
            event(0, 10.0, 1),
            event(1, 20.0, 2),
            event(2, 30.0, 3),
        ]);
        let edits = core.edits_since_autosave();
        core.particle_delete_event(1);
        assert_eq!(core.project.particles.len(), 2);
        assert_eq!(core.project.particles[0].seed, 1);
        assert_eq!(core.project.particles[1].seed, 3, "the middle event went");
        assert_eq!(core.status, "Particle event 2 deleted.");
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        // The one snapshot is a real undo step.
        core.undo();
        assert_eq!(core.status, "Undone.");
        assert_eq!(core.project.particles.len(), 3);
        assert_eq!(core.project.particles[1].seed, 2);
    }

    #[test]
    fn delete_event_out_of_range_is_status_only() {
        let mut core = core_with(vec![event(0, 0.0, 1)]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        core.particle_delete_event(4);
        assert_eq!(
            core.status,
            "Particle event delete failed: no particle event 5."
        );
        assert_eq!(project_json(&core), before, "project untouched");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot taken");
    }

    #[test]
    fn set_frame_moves_and_clamps_to_the_last_frame() {
        let mut core = core_with(vec![event(0, 0.0, 1)]);
        let last = core.frame_count() - 1;
        let edits = core.edits_since_autosave();
        core.particle_set_frame(0, 2);
        assert_eq!(core.project.particles[0].frame, 2);
        assert_eq!(core.status, "Particle event 1 moved to frame 3.");
        core.particle_set_frame(0, usize::MAX);
        assert_eq!(core.project.particles[0].frame, last);
        assert_eq!(
            core.status,
            format!("Particle event 1 moved to frame {}.", last + 1)
        );
        assert_eq!(core.edits_since_autosave(), edits + 2, "one snapshot per op");
    }

    #[test]
    fn set_frame_out_of_range_is_status_only() {
        let mut core = core_with(vec![event(0, 0.0, 1)]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        core.particle_set_frame(1, 0);
        assert_eq!(
            core.status,
            "Particle event frame failed: no particle event 2."
        );
        assert_eq!(project_json(&core), before, "project untouched");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot taken");
    }

    #[test]
    fn set_anchor_sets_then_clears_and_round_trips() {
        let mut core = core_with(vec![event(0, 50.0, 1)]);
        let edits = core.edits_since_autosave();
        core.particle_set_anchor(0, Some(1), Some(3));
        let ev = &core.project.particles[0];
        assert_eq!((ev.figure, ev.node), (Some(1), Some(3)));
        // Anchoring does not touch the stored render point.
        assert_eq!((ev.x, ev.y), (50.0, 0.0));
        assert_eq!(
            core.status,
            "Particle event 1 anchored to fig 2 node 3."
        );
        // A figure with no node anchors to the root, node 0.
        core.particle_set_anchor(0, Some(0), None);
        let ev = &core.project.particles[0];
        assert_eq!((ev.figure, ev.node), (Some(0), None));
        assert_eq!(
            core.status,
            "Particle event 1 anchored to fig 1 node 0."
        );
        // Clearing the figure clears the node too, even
        // when a node is passed alongside.
        core.particle_set_anchor(0, None, Some(2));
        let ev = &core.project.particles[0];
        assert_eq!((ev.figure, ev.node), (None, None));
        assert_eq!(core.status, "Particle event 1 anchor cleared.");
        assert_eq!(core.edits_since_autosave(), edits + 3, "one snapshot per op");
    }

    #[test]
    fn set_anchor_out_of_range_is_status_only() {
        let mut core = core_with(vec![event(0, 0.0, 1)]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        core.particle_set_anchor(2, Some(0), Some(0));
        assert_eq!(
            core.status,
            "Particle event anchor failed: no particle event 3."
        );
        assert_eq!(project_json(&core), before, "project untouched");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot taken");
    }

    #[test]
    fn set_position_moves_the_stored_render_point() {
        let mut core = core_with(vec![event(0, 10.0, 1)]);
        let edits = core.edits_since_autosave();
        core.particle_set_position(0, 300.0, 150.0);
        let ev = &core.project.particles[0];
        assert_eq!((ev.x, ev.y), (300.0, 150.0));
        assert_eq!(core.status, "Particle event 1 moved to (300, 150).");
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
    }

    #[test]
    fn set_position_out_of_range_is_status_only() {
        let mut core = core_with(vec![event(0, 0.0, 1)]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        core.particle_set_position(1, 0.0, 0.0);
        assert_eq!(
            core.status,
            "Particle event position failed: no particle event 2."
        );
        assert_eq!(project_json(&core), before, "project untouched");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot taken");
    }

    #[test]
    fn set_kind_lands_and_leaves_the_other_fields_alone() {
        let mut core = core_with(vec![event(2, 40.0, 7)]);
        let edits = core.edits_since_autosave();
        core.particle_set_kind(0, ParticleKind::Sparkle);
        let ev = &core.project.particles[0];
        assert_eq!(ev.kind, ParticleKind::Sparkle);
        assert_eq!(core.status, "Particle event 1 kind set to Sparkle.");
        // Only the kind changed: frame, point, seed and
        // colour all keep the fixture's values.
        assert_eq!((ev.frame, ev.x, ev.seed, ev.color), (2, 40.0, 7, 0xFFFFFF));
        core.particle_set_kind(0, ParticleKind::Smoke);
        assert_eq!(core.project.particles[0].kind, ParticleKind::Smoke);
        assert_eq!(core.status, "Particle event 1 kind set to Smoke.");
        assert_eq!(core.edits_since_autosave(), edits + 2, "one snapshot per op");
    }

    #[test]
    fn set_kind_out_of_range_is_status_only() {
        let mut core = core_with(vec![event(0, 0.0, 1)]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        core.particle_set_kind(1, ParticleKind::Smoke);
        assert_eq!(
            core.status,
            "Particle event kind failed: no particle event 2."
        );
        assert_eq!(project_json(&core), before, "project untouched");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot taken");
    }

    #[test]
    fn set_color_lands_with_padded_hex_status() {
        let mut core = core_with(vec![event(0, 0.0, 1)]);
        let edits = core.edits_since_autosave();
        core.particle_set_color(0, 0x0A0B0C);
        let ev = &core.project.particles[0];
        assert_eq!(ev.color, 0x0A0B0C);
        assert_eq!(ev.kind, ParticleKind::Burst, "kind untouched");
        // The status pads to six hex digits, like fx_ops.
        assert_eq!(core.status, "Particle event 1 colour set to #0A0B0C.");
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
    }

    #[test]
    fn set_color_out_of_range_is_status_only() {
        let mut core = core_with(vec![event(0, 0.0, 1)]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        core.particle_set_color(3, 0xFF0000);
        assert_eq!(
            core.status,
            "Particle event colour failed: no particle event 4."
        );
        assert_eq!(project_json(&core), before, "project untouched");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot taken");
    }
}
