//! FX-clip engine ops: the `EditorCore` method family for
//! editing the project's timed particle clips
//! (`Project::fx_clips`) — move, resize, duplicate, delete,
//! colours and anchor (Phase E, section 9).
//!
//! These ops are the engine home for what the Phase D Rune
//! FX-clips panel (`rune_sec/fx_clips.rs`) used to do inline:
//! every panel handler took an undo snapshot and then called
//! the `fx_edit` backend (or, for colour / end colour /
//! anchor, which have no backend function anywhere, wrote
//! the `Project::fx_clips` field directly after the
//! snapshot). The panel keeps the selection and the views;
//! the mutations now live here, next to `add_fx_clip` and
//! the FX preset ops in `editor.rs`, so the MCP tools and
//! any future UI share one implementation.
//!
//! Every op follows the same contract:
//!
//! * An out-of-range clip index changes nothing and takes
//!   NO snapshot — it only sets `status`, in the failure
//!   voice of `fx_save_preset` ("… failed: no FX clip N.").
//! * An in-range op snapshots exactly once, then delegates
//!   to `fx_edit` (`move_clip` / `resize_clip` /
//!   `duplicate_clip` / `delete_clip`, which own the
//!   clamping: start >= 0, duration >= 0.1). Colour, end
//!   colour and anchor have no `fx_edit` function, so —
//!   exactly as the panel did — they write the field after
//!   the snapshot.
//! * Success is reported on `status` in the panel's voice:
//!   1-based clip and figure numbers, the frame wording of
//!   `add_fx_clip`, and for the anchor the panel's exact
//!   "FX clip N re-anchored to fig F node M." line.
//!
//! Integrator wiring: declare `pub mod fx_ops;` in
//! `lib.rs`. Nothing else is needed — this file only adds
//! an inherent `impl EditorCore` block.

use crate::editor::EditorCore;
use crate::fx_edit;

/// Display text for a frame position, mirroring the FX
/// panel's `frame_label`: whole frames render without a
/// fraction (`4`), fractional ones with one decimal
/// (`4.5`). Clip starts/durations are `f32`, so the status
/// line must not silently truncate them.
fn frame_label(frame: f32) -> String {
    if frame.fract() == 0.0 {
        format!("{}", frame as i64)
    } else {
        format!("{frame:.1}")
    }
}

impl EditorCore {
    /// Move FX clip `index` by `delta_frames` frames
    /// (negative moves it earlier). The resulting start is
    /// clamped to frame 0 by `fx_edit::move_clip`. One
    /// undo step; an out-of-range index only sets the
    /// status.
    pub fn fx_move_clip(&mut self, index: usize, delta_frames: i32) {
        let Some(entry) = self.project.fx_clips.get(index) else {
            self.status = format!("FX clip move failed: no FX clip {}.", index + 1);
            return;
        };
        let new_start = entry.clip.start + delta_frames as f32;
        self.snapshot();
        fx_edit::move_clip(&mut self.project, index, new_start);
        let start = self.project.fx_clips[index].clip.start;
        self.status = format!(
            "FX clip {} moved to frame {}.",
            index + 1,
            frame_label(start + 1.0)
        );
    }

    /// Resize FX clip `index` by `delta` frames (negative
    /// shortens it). The resulting duration is clamped to
    /// >= 0.1 by `fx_edit::resize_clip`, so a clip never
    /// collapses to nothing. One undo step; an
    /// out-of-range index only sets the status.
    pub fn fx_resize_clip(&mut self, index: usize, delta: f32) {
        let Some(entry) = self.project.fx_clips.get(index) else {
            self.status = format!("FX clip resize failed: no FX clip {}.", index + 1);
            return;
        };
        let new_duration = entry.clip.duration + delta;
        self.snapshot();
        fx_edit::resize_clip(&mut self.project, index, new_duration);
        let duration = self.project.fx_clips[index].clip.duration;
        self.status = format!(
            "FX clip {} resized to {} frames.",
            index + 1,
            frame_label(duration)
        );
    }

    /// Duplicate FX clip `index`: the copy is inserted
    /// right after the original and placed right after it
    /// in time, with a bumped seed (see
    /// `fx_edit::duplicate_clip`). Returns the copy's
    /// index — the panel selects it, as the clip the user
    /// most likely wants to adjust next. One undo step;
    /// an out-of-range index only sets the status and
    /// returns `None`.
    pub fn fx_duplicate_clip(&mut self, index: usize) -> Option<usize> {
        if self.project.fx_clips.get(index).is_none() {
            self.status = format!("FX clip duplicate failed: no FX clip {}.", index + 1);
            return None;
        }
        self.snapshot();
        let new_index = fx_edit::duplicate_clip(&mut self.project, index);
        if let Some(new_index) = new_index {
            self.status = format!(
                "FX clip {} duplicated as clip {}.",
                index + 1,
                new_index + 1
            );
        }
        new_index
    }

    /// Delete FX clip `index`. One undo step; an
    /// out-of-range index only sets the status.
    pub fn fx_delete_clip(&mut self, index: usize) {
        if self.project.fx_clips.get(index).is_none() {
            self.status = format!("FX clip delete failed: no FX clip {}.", index + 1);
            return;
        }
        self.snapshot();
        fx_edit::delete_clip(&mut self.project, index);
        self.status = format!("FX clip {} deleted.", index + 1);
    }

    /// Set FX clip `index`'s primary colour (`0xRRGGBB`).
    /// No `fx_edit` backend exists for colour (the MCP
    /// `add_fx_clip` tool sets it only at creation), so —
    /// as the panel did — the field is written after the
    /// snapshot. One undo step; an out-of-range index
    /// only sets the status.
    pub fn fx_set_color(&mut self, index: usize, color: u32) {
        if self.project.fx_clips.get(index).is_none() {
            self.status = format!("FX clip colour failed: no FX clip {}.", index + 1);
            return;
        }
        self.snapshot();
        self.project.fx_clips[index].clip.color = color;
        self.status = format!("FX clip {} colour set to #{color:06X}.", index + 1);
    }

    /// Set FX clip `index`'s gradient end colour, or clear
    /// it with `None` (solid colour). Same no-backend
    /// pattern as [`Self::fx_set_color`]. One undo step;
    /// an out-of-range index only sets the status.
    pub fn fx_set_color2(&mut self, index: usize, color2: Option<u32>) {
        if self.project.fx_clips.get(index).is_none() {
            self.status = format!("FX clip end colour failed: no FX clip {}.", index + 1);
            return;
        }
        self.snapshot();
        self.project.fx_clips[index].clip.color2 = color2;
        self.status = match color2 {
            Some(color2) => format!(
                "FX clip {} end colour set to #{color2:06X}.",
                index + 1
            ),
            None => format!("FX clip {} end colour cleared.", index + 1),
        };
    }

    /// Re-anchor FX clip `index` to joint `node` of figure
    /// `figure` — the anchoring `add_fx_clip` performs at
    /// creation. Only the clip index is bounds-checked
    /// (the panel passes the core's selected figure/joint,
    /// which are valid by construction). One undo step;
    /// an out-of-range index only sets the status.
    pub fn fx_set_anchor(&mut self, index: usize, figure: usize, node: usize) {
        if self.project.fx_clips.get(index).is_none() {
            self.status = format!("FX clip anchor failed: no FX clip {}.", index + 1);
            return;
        }
        self.snapshot();
        let entry = &mut self.project.fx_clips[index];
        entry.figure = figure;
        entry.node = node;
        self.status = format!(
            "FX clip {} re-anchored to fig {} node {}.",
            index + 1,
            figure + 1,
            node
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::FxClipEntry;
    use crate::vfx::{ClipKind, ParticleClip};

    /// Clip construction mirrored from the `fx_clips.rs`
    /// panel tests: entries are pushed straight onto
    /// `Project::fx_clips` because `add_fx_clip` needs a
    /// selected joint, which is not what is under test.
    fn entry(start: f32, duration: f32, kind: ClipKind, color: u32) -> FxClipEntry {
        FxClipEntry {
            clip: ParticleClip {
                start,
                duration,
                kind,
                color,
                color2: None,
                seed: 7,
                angle: 0.0,
                length: 130.0,
            },
            figure: 1,
            node: 5,
        }
    }

    fn core_with(clips: Vec<FxClipEntry>) -> EditorCore {
        let mut core = EditorCore::new();
        core.project.fx_clips = clips;
        core
    }

    /// The whole project as its snapshot JSON — the exact
    /// bytes `snapshot()` would store. `Project` has no
    /// `PartialEq`, so this is how "untouched" is asserted.
    fn project_json(core: &EditorCore) -> String {
        serde_json::to_string(&core.project).unwrap()
    }

    #[test]
    fn move_clip_shifts_start_clamps_at_zero_and_snapshots_once() {
        let mut core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let edits = core.edits_since_autosave();
        core.fx_move_clip(0, 3);
        assert_eq!(core.project.fx_clips[0].clip.start, 7.0);
        // Status names the 1-based first covered frame.
        assert_eq!(core.status, "FX clip 1 moved to frame 8.");
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        // Moving earlier past frame 0 clamps (fx_edit::move_clip).
        core.fx_move_clip(0, -20);
        assert_eq!(core.project.fx_clips[0].clip.start, 0.0);
        assert_eq!(core.status, "FX clip 1 moved to frame 1.");
        assert_eq!(core.edits_since_autosave(), edits + 2);
    }

    #[test]
    fn move_clip_out_of_range_is_status_only() {
        let mut core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        core.fx_move_clip(5, 3);
        assert_eq!(core.status, "FX clip move failed: no FX clip 6.");
        assert_eq!(project_json(&core), before, "project untouched");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot taken");
    }

    #[test]
    fn resize_clip_applies_delta_clamps_and_snapshots_once() {
        let mut core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let edits = core.edits_since_autosave();
        core.fx_resize_clip(0, -4.0);
        assert_eq!(core.project.fx_clips[0].clip.duration, 6.0);
        assert_eq!(core.status, "FX clip 1 resized to 6 frames.");
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        // A delta that would collapse the clip clamps to 0.1.
        core.fx_resize_clip(0, -100.0);
        assert_eq!(core.project.fx_clips[0].clip.duration, 0.1);
        assert_eq!(core.edits_since_autosave(), edits + 2);
    }

    #[test]
    fn resize_clip_out_of_range_is_status_only() {
        let mut core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        core.fx_resize_clip(1, 2.0);
        assert_eq!(core.status, "FX clip resize failed: no FX clip 2.");
        assert_eq!(project_json(&core), before, "project untouched");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot taken");
    }

    #[test]
    fn duplicate_clip_returns_copy_index_and_snapshots_once() {
        let mut core = core_with(vec![
            entry(4.0, 10.0, ClipKind::Bubbles, 0xFFC65D),
            entry(50.0, 5.0, ClipKind::Burst, 0xFF5D5D),
        ]);
        let edits = core.edits_since_autosave();
        let new_index = core.fx_duplicate_clip(0).expect("valid index");
        assert_eq!(new_index, 1);
        assert_eq!(core.project.fx_clips.len(), 3);
        // The copy sits right after the original in time
        // (fx_edit::duplicate_clip), the neighbour shifted.
        assert_eq!(core.project.fx_clips[1].clip.start, 14.0);
        assert_eq!(core.project.fx_clips[2].clip.start, 50.0);
        assert_eq!(core.status, "FX clip 1 duplicated as clip 2.");
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
    }

    #[test]
    fn duplicate_clip_out_of_range_returns_none_and_changes_nothing() {
        let mut core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        assert_eq!(core.fx_duplicate_clip(9), None);
        assert_eq!(core.status, "FX clip duplicate failed: no FX clip 10.");
        assert_eq!(project_json(&core), before, "project untouched");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot taken");
    }

    #[test]
    fn delete_clip_removes_snapshots_once_and_undo_restores() {
        let mut core = core_with(vec![
            entry(0.0, 5.0, ClipKind::Burst, 0xFF5D5D),
            entry(10.0, 5.0, ClipKind::Smoke, 0x9AA3B2),
            entry(20.0, 5.0, ClipKind::Aura, 0xC792FF),
        ]);
        let edits = core.edits_since_autosave();
        core.fx_delete_clip(1);
        assert_eq!(core.project.fx_clips.len(), 2);
        assert_eq!(core.project.fx_clips[1].clip.kind, ClipKind::Aura);
        assert_eq!(core.status, "FX clip 2 deleted.");
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
        // The one snapshot is a real undo step.
        core.undo();
        assert_eq!(core.status, "Undone.");
        assert_eq!(core.project.fx_clips.len(), 3);
        assert_eq!(core.project.fx_clips[1].clip.kind, ClipKind::Smoke);
    }

    #[test]
    fn delete_clip_out_of_range_is_status_only() {
        let mut core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        core.fx_delete_clip(3);
        assert_eq!(core.status, "FX clip delete failed: no FX clip 4.");
        assert_eq!(project_json(&core), before, "project untouched");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot taken");
        // Same on an empty list.
        let mut empty = core_with(vec![]);
        empty.fx_delete_clip(0);
        assert_eq!(empty.status, "FX clip delete failed: no FX clip 1.");
        assert!(empty.project.fx_clips.is_empty());
    }

    #[test]
    fn set_color_writes_colour_and_snapshots_once() {
        let mut core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let edits = core.edits_since_autosave();
        core.fx_set_color(0, 0x5DB2FF);
        assert_eq!(core.project.fx_clips[0].clip.color, 0x5DB2FF);
        assert_eq!(core.status, "FX clip 1 colour set to #5DB2FF.");
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
    }

    #[test]
    fn set_color2_sets_then_clears_and_snapshots_each_time() {
        let mut core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let edits = core.edits_since_autosave();
        core.fx_set_color2(0, Some(0xFF7BAC));
        assert_eq!(core.project.fx_clips[0].clip.color2, Some(0xFF7BAC));
        assert_eq!(core.status, "FX clip 1 end colour set to #FF7BAC.");
        core.fx_set_color2(0, None);
        assert_eq!(core.project.fx_clips[0].clip.color2, None);
        assert_eq!(core.status, "FX clip 1 end colour cleared.");
        assert_eq!(core.edits_since_autosave(), edits + 2, "one snapshot per op");
    }

    #[test]
    fn set_anchor_uses_the_panels_exact_wording() {
        let mut core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let edits = core.edits_since_autosave();
        core.fx_set_anchor(0, 2, 4);
        assert_eq!(core.project.fx_clips[0].figure, 2);
        assert_eq!(core.project.fx_clips[0].node, 4);
        // Byte-identical to the Phase D panel's re-anchor line.
        assert_eq!(core.status, "FX clip 1 re-anchored to fig 3 node 4.");
        assert_eq!(core.edits_since_autosave(), edits + 1, "exactly one snapshot");
    }

    #[test]
    fn colour_and_anchor_ops_out_of_range_are_status_only() {
        let mut core = core_with(vec![entry(4.0, 10.0, ClipKind::Burst, 0xFF5D5D)]);
        let before = project_json(&core);
        let edits = core.edits_since_autosave();
        core.fx_set_color(9, 0xFFFFFF);
        assert_eq!(core.status, "FX clip colour failed: no FX clip 10.");
        core.fx_set_color2(9, Some(0xFFFFFF));
        assert_eq!(core.status, "FX clip end colour failed: no FX clip 10.");
        core.fx_set_anchor(9, 0, 0);
        assert_eq!(core.status, "FX clip anchor failed: no FX clip 10.");
        assert_eq!(project_json(&core), before, "project untouched");
        assert_eq!(core.edits_since_autosave(), edits, "no snapshot taken");
    }
}
