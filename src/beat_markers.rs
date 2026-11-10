//! Story-beat markers: the EditorCore ops (Roadmap Wave 2,
//! W2-19 — P-15's shot markers).
//!
//! The data and its one validation rule live in `model.rs`
//! ([`crate::model::BeatMarker`], `Project::beat_markers`);
//! these are the editing ops over that list, in the house
//! op style the subtitle / chapter / stored-guide ops
//! established:
//!
//! * A refused op (empty name, no such marker, a rename to
//!   the name the marker already carries, a move to the
//!   frame it already sits on) changes nothing and takes
//!   NO snapshot — it only sets `status`, in the failure
//!   voice ("… failed: …") or the no-change voice.
//! * A successful mutation snapshots exactly once (one
//!   undo step, labelled for the History panel), then
//!   writes.
//! * The list is kept sorted by frame (the cue list's
//!   discipline): `beat_marker_add` inserts at the sorted
//!   position — after every marker on the same frame, so
//!   equal-frame markers keep their insertion order — and
//!   `beat_marker_set_frame` re-seats its marker the same
//!   way. List order is therefore timeline order.
//! * Names are stored VERBATIM (see the model's verbatim
//!   rule): no op trims, case-folds or otherwise rewrites
//!   a name; validation refuses only a name with no
//!   visible character.
//! * Frames are NOT clamped to the timeline (the chapter
//!   / cue posture): a marker may be authored past the
//!   last frame, is stored verbatim, and is inert — the
//!   timeline row marks nothing there and the jump ops do
//!   not target it — until the timeline grows to reach it.
//! * Frame ops do NOT remap beat markers (the no-remap
//!   posture cues and chapters live under; only camera
//!   keys remap, IA-RM18-1). See the model docs and
//!   IA-W2-11-4.
//! * Deleting a marker removes the marker ONLY — frames,
//!   cues, chapters and every other collection are
//!   untouched. A marker is annotation, not content.
//! * The jump ops never wrap (the `goto_next_keyframe`
//!   precedent): next means strictly after the playhead,
//!   previous strictly before, and when there is none the
//!   playhead stays where it is and the op reports so.
//! * Success is reported on `status` with 1-based marker
//!   and frame numbers.

use crate::editor::EditorCore;
use crate::model::BeatMarker;

impl EditorCore {
    /// All beat markers, in timeline order (the list is
    /// kept sorted by frame — see the module docs).
    pub fn beat_markers(&self) -> &[BeatMarker] {
        &self.project.beat_markers
    }

    /// How many beat markers the project carries.
    pub fn beat_marker_count(&self) -> usize {
        self.project.beat_markers.len()
    }

    /// The first beat marker sitting exactly on `frame`,
    /// when one does.
    pub fn beat_marker_at(&self, frame: usize) -> Option<&BeatMarker> {
        self.project.beat_marker_at(frame)
    }

    /// The sorted position a marker on `frame` takes: the
    /// index after every stored marker on a frame `<=
    /// frame`, so a new marker lands behind the markers
    /// already on its frame (stable among equals) and
    /// ahead of every later frame's. The list's one
    /// ordering rule, shared by add and set-frame.
    fn beat_marker_position(&self, frame: usize) -> usize {
        self.project
            .beat_markers
            .partition_point(|m| m.frame <= frame)
    }

    /// Add a beat marker named `name` on authored frame
    /// `frame`, inserted at its sorted position (see
    /// [`EditorCore::beat_marker_position`]). One undo
    /// step. The stored name is the argument VERBATIM.
    /// Returns the new marker's index in timeline order,
    /// or `None` when refused (a name with no visible
    /// character) — a refusal changes nothing and takes
    /// no snapshot.
    pub fn beat_marker_add(&mut self, frame: usize, name: &str) -> Option<usize> {
        if let Err(reason) = BeatMarker::validate(name) {
            self.status = format!("Beat marker add failed: {reason}.");
            return None;
        }
        let index = self.beat_marker_position(frame);
        self.snapshot_labeled("Add beat marker");
        self.project.beat_markers.insert(
            index,
            BeatMarker {
                name: name.to_string(),
                frame,
            },
        );
        self.status = format!(
            "Beat marker {} added at frame {}.",
            index + 1,
            frame + 1
        );
        Some(index)
    }

    /// Rename beat marker `index` to `name` (validated
    /// first), stored VERBATIM. One undo step. Returns
    /// false — changing nothing and taking no snapshot —
    /// when there is no such marker, the name has no
    /// visible character, or the marker already carries
    /// exactly this name (the no-change discipline).
    pub fn beat_marker_rename(&mut self, index: usize, name: &str) -> bool {
        let Some(marker) = self.project.beat_markers.get(index) else {
            self.status = format!("Beat marker rename failed: no beat marker {}.", index + 1);
            return false;
        };
        if let Err(reason) = BeatMarker::validate(name) {
            self.status = format!("Beat marker rename failed: {reason}.");
            return false;
        }
        if marker.name == name {
            self.status = format!("Beat marker {} is already named {name}.", index + 1);
            return false;
        }
        let name = name.to_string();
        self.snapshot_labeled("Rename beat marker");
        self.project.beat_markers[index].name = name.clone();
        self.status = format!("Beat marker {} renamed to {name}.", index + 1);
        true
    }

    /// Move beat marker `index` to authored frame `frame`
    /// (not clamped to the timeline — the chapter / cue
    /// posture). The marker is re-seated at the frame's
    /// sorted position, so the list stays in timeline
    /// order; the status line reports its NEW position.
    /// One undo step. Returns false — changing nothing
    /// and taking no snapshot — when there is no such
    /// marker or the marker already sits on `frame`.
    pub fn beat_marker_set_frame(&mut self, index: usize, frame: usize) -> bool {
        let Some(marker) = self.project.beat_markers.get(index) else {
            self.status = format!("Beat marker move failed: no beat marker {}.", index + 1);
            return false;
        };
        if marker.frame == frame {
            self.status = format!("Beat marker {} is already at frame {}.", index + 1, frame + 1);
            return false;
        }
        self.snapshot_labeled("Move beat marker");
        let marker = self.project.beat_markers.remove(index);
        let mut marker = marker;
        marker.frame = frame;
        let new_index = self.beat_marker_position(frame);
        self.project.beat_markers.insert(new_index, marker);
        self.status = format!(
            "Beat marker {} moved to frame {}.",
            new_index + 1,
            frame + 1
        );
        true
    }

    /// Delete beat marker `index`. One undo step. The
    /// marker ONLY is removed: frames, cues, chapters and
    /// all other content are untouched — a marker is
    /// annotation, not content. Returns false — changing
    /// nothing and taking no snapshot — when there is no
    /// such marker.
    pub fn beat_marker_delete(&mut self, index: usize) -> bool {
        if self.project.beat_markers.get(index).is_none() {
            self.status = format!("Beat marker delete failed: no beat marker {}.", index + 1);
            return false;
        }
        self.snapshot_labeled("Delete beat marker");
        self.project.beat_markers.remove(index);
        self.status = format!("Beat marker {} deleted.", index + 1);
        true
    }

    /// Jump the playhead to the next reachable beat marker
    /// strictly after the current frame (via
    /// [`EditorCore::goto`]). Reachable means on the
    /// timeline: a marker stored past the last frame is
    /// not a jump target until the timeline grows to it
    /// (the display rule — the marker row marks nothing
    /// there either). Never wraps. Returns false — and
    /// leaves the playhead where it is — when there is no
    /// later reachable marker.
    pub fn beat_marker_next(&mut self) -> bool {
        let count = self.frame_count();
        let target = self
            .project
            .beat_markers
            .iter()
            .filter(|m| m.frame < count)
            .map(|m| m.frame)
            .filter(|&f| f > self.current)
            .min();
        match target {
            Some(target) => {
                let name = self
                    .project
                    .beat_marker_at(target)
                    .map(|m| m.name.clone())
                    .unwrap_or_default();
                self.goto(target);
                self.status = format!("Beat marker '{name}' at frame {}.", target + 1);
                true
            }
            None => {
                self.status = "No later beat marker.".to_string();
                false
            }
        }
    }

    /// Jump the playhead to the previous beat marker
    /// strictly before the current frame —
    /// [`EditorCore::beat_marker_next`]'s mirror. A marker
    /// before the playhead is always on the timeline (the
    /// playhead itself is), so no reachability filter is
    /// needed here. Never wraps; returns false and leaves
    /// the playhead where it is when there is none.
    pub fn beat_marker_prev(&mut self) -> bool {
        match self.project.prev_beat_frame(self.current) {
            Some(target) => {
                let name = self
                    .project
                    .beat_marker_at(target)
                    .map(|m| m.name.clone())
                    .unwrap_or_default();
                self.goto(target);
                self.status = format!("Beat marker '{name}' at frame {}.", target + 1);
                true
            }
            None => {
                self.status = "No earlier beat marker.".to_string();
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Project;

    /// The whole project as its snapshot JSON — the exact
    /// bytes `snapshot()` would store (the house pattern,
    /// from the chapter ops tests).
    fn project_json(core: &EditorCore) -> String {
        serde_json::to_string(&core.project).unwrap()
    }

    fn names(core: &EditorCore) -> Vec<&str> {
        core.beat_markers().iter().map(|m| m.name.as_str()).collect()
    }

    fn frames(core: &EditorCore) -> Vec<usize> {
        core.beat_markers().iter().map(|m| m.frame).collect()
    }

    #[test]
    fn markerless_projects_save_byte_identically() {
        // The pre-W2-19 serialised form: with no markers the
        // field is skipped on save, so the bytes are exactly
        // what a pre-beat-marker build wrote.
        let core = EditorCore::new();
        let json = project_json(&core);
        assert!(!json.contains("beat_markers"), "{json}");
        // And the round trip is byte-stable.
        let loaded: Project = serde_json::from_str(&json).unwrap();
        assert!(loaded.beat_markers.is_empty());
        assert_eq!(serde_json::to_string(&loaded).unwrap(), json);
        // A stored file with the key removed (every file
        // written before this seat) loads with no markers,
        // and the minimal file loads too.
        let minimal: Project = serde_json::from_str("{}").unwrap();
        assert!(minimal.beat_markers.is_empty());
    }

    #[test]
    fn marker_set_round_trips_verbatim() {
        let mut core = EditorCore::new();
        core.beat_marker_add(4, "  The Turn  ");
        core.beat_marker_add(2, "Cold open — the joke lands");
        let json = project_json(&core);
        assert!(json.contains("beat_markers"), "{json}");
        let loaded: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.beat_markers, core.project.beat_markers);
        // Names survive the round trip byte-for-byte,
        // surrounding whitespace and punctuation included.
        assert_eq!(loaded.beat_markers[0].name, "Cold open — the joke lands");
        assert_eq!(loaded.beat_markers[1].name, "  The Turn  ");
        assert_eq!(serde_json::to_string(&loaded).unwrap(), json);
        // A hand-trimmed marker object loads on per-field
        // defaults.
        let trimmed: Project =
            serde_json::from_str(r#"{"beat_markers":[{}]}"#).unwrap();
        assert_eq!(trimmed.beat_markers.len(), 1);
        assert_eq!(trimmed.beat_markers[0].frame, 0);
        assert_eq!(trimmed.beat_markers[0].name, "");
    }

    #[test]
    fn add_inserts_in_frame_order_stable_among_equals() {
        let mut core = EditorCore::new();
        assert_eq!(core.beat_marker_add(6, "B"), Some(0));
        assert_eq!(core.beat_marker_add(2, "A"), Some(0));
        assert_eq!(core.beat_marker_add(9, "C"), Some(2));
        // A second marker on frame 6 lands BEHIND the one
        // already there: equal frames keep insertion order.
        assert_eq!(core.beat_marker_add(6, "B2"), Some(2));
        assert_eq!(frames(&core), [2, 6, 6, 9]);
        assert_eq!(names(&core), ["A", "B", "B2", "C"]);
        assert_eq!(core.beat_marker_count(), 4);
        // The last add (B2, frame 6) reports its own frame,
        // 1-based.
        assert!(core.status.contains("added at frame 7"), "{}", core.status);
    }

    #[test]
    fn names_are_stored_verbatim_and_empty_names_refused_loudly() {
        let mut core = EditorCore::new();
        assert_eq!(core.beat_marker_add(2, "  The Reveal  "), Some(0));
        assert_eq!(core.beat_markers()[0].name, "  The Reveal  ");
        // Empty and all-whitespace names refuse: nothing
        // changes, no snapshot, the failure voice speaks.
        let depth = core.undo_depth();
        let before = project_json(&core);
        assert_eq!(core.beat_marker_add(3, ""), None);
        assert!(core.status.contains("Beat marker add failed: name is empty."), "{}", core.status);
        assert_eq!(core.beat_marker_add(3, "   "), None);
        assert_eq!(core.undo_depth(), depth);
        assert_eq!(project_json(&core), before);
        // The shared rule speaks the chapter shape.
        assert_eq!(BeatMarker::validate("").unwrap_err(), "name is empty");
        assert_eq!(BeatMarker::validate(" \t ").unwrap_err(), "name is empty");
        assert!(BeatMarker::validate(" x ").is_ok());
    }

    #[test]
    fn each_successful_mutation_takes_exactly_one_undo_step() {
        let mut core = EditorCore::new();
        assert_eq!(core.undo_depth(), 0);
        core.beat_marker_add(2, "A");
        assert_eq!(core.undo_depth(), 1);
        core.beat_marker_add(6, "B");
        assert_eq!(core.undo_depth(), 2);
        core.beat_marker_rename(0, "Opening");
        assert_eq!(core.undo_depth(), 3);
        core.beat_marker_set_frame(1, 9);
        assert_eq!(core.undo_depth(), 4);
        core.beat_marker_delete(0);
        assert_eq!(core.undo_depth(), 5);
        // Undo restores the pre-delete list verbatim.
        core.undo();
        assert_eq!(frames(&core), [2, 9]);
        assert_eq!(names(&core), ["Opening", "B"]);
    }

    #[test]
    fn refused_ops_change_nothing_and_take_no_snapshot() {
        let mut core = EditorCore::new();
        core.beat_marker_add(2, "A");
        let depth = core.undo_depth();
        let before = project_json(&core);
        assert!(!core.beat_marker_rename(5, "X"));
        assert!(core.status.contains("no beat marker 6"), "{}", core.status);
        assert!(!core.beat_marker_rename(0, "  "));
        assert!(core.status.contains("name is empty"), "{}", core.status);
        assert!(!core.beat_marker_set_frame(5, 3));
        assert!(!core.beat_marker_delete(5));
        assert!(core.status.contains("Beat marker delete failed"), "{}", core.status);
        assert_eq!(core.undo_depth(), depth);
        assert_eq!(project_json(&core), before);
    }

    #[test]
    fn rename_to_the_same_name_spends_nothing() {
        let mut core = EditorCore::new();
        core.beat_marker_add(2, "The Reveal");
        let depth = core.undo_depth();
        assert!(!core.beat_marker_rename(0, "The Reveal"));
        assert!(core.status.contains("already named The Reveal"), "{}", core.status);
        assert_eq!(core.undo_depth(), depth);
        // A genuinely new name — even one differing only in
        // surrounding whitespace — is a real rename, stored
        // verbatim.
        assert!(core.beat_marker_rename(0, " The Reveal "));
        assert_eq!(core.beat_markers()[0].name, " The Reveal ");
        assert_eq!(core.undo_depth(), depth + 1);
    }

    #[test]
    fn set_frame_reseats_the_marker_in_timeline_order() {
        let mut core = EditorCore::new();
        core.beat_marker_add(2, "A");
        core.beat_marker_add(6, "B");
        // Moving A past B re-seats it behind B; the status
        // reports the NEW position.
        assert!(core.beat_marker_set_frame(0, 9));
        assert_eq!(frames(&core), [6, 9]);
        assert_eq!(names(&core), ["B", "A"]);
        assert!(core.status.contains("Beat marker 2 moved to frame 10"), "{}", core.status);
        // A move to the frame it already sits on is a
        // reported no-op spending nothing.
        let depth = core.undo_depth();
        assert!(!core.beat_marker_set_frame(1, 9));
        assert!(core.status.contains("already at frame 10"), "{}", core.status);
        assert_eq!(core.undo_depth(), depth);
    }

    #[test]
    fn delete_removes_the_marker_and_nothing_else() {
        let mut core = EditorCore::new();
        core.beat_marker_add(2, "A");
        core.beat_marker_add(4, "B");
        let frame_count = core.frame_count();
        let cue_count = core.project.subtitles.len();
        let chapter_count = core.project.chapters.len();
        assert!(core.beat_marker_delete(0));
        assert_eq!(names(&core), ["B"]);
        assert_eq!(core.frame_count(), frame_count);
        assert_eq!(core.project.subtitles.len(), cue_count);
        assert_eq!(core.project.chapters.len(), chapter_count);
    }

    #[test]
    fn jumps_are_strict_and_never_wrap() {
        let mut core = EditorCore::new();
        core.beat_marker_add(4, "The Turn");
        core.beat_marker_add(2, "The Reveal");
        // Before the first beat: next lands on it.
        assert_eq!(core.current, 0);
        assert!(core.beat_marker_next());
        assert_eq!(core.current, 2);
        assert!(core.status.contains("'The Reveal' at frame 3"), "{}", core.status);
        // ON a beat: next is strict — it goes to the
        // FOLLOWING beat, never itself.
        assert!(core.beat_marker_next());
        assert_eq!(core.current, 4);
        // Past the last beat: no wrap, the playhead stays.
        assert!(!core.beat_marker_next());
        assert_eq!(core.current, 4);
        assert_eq!(core.status, "No later beat marker.");
        // Previous is the mirror: strict, no wrap.
        assert!(core.beat_marker_prev());
        assert_eq!(core.current, 2);
        assert!(!core.beat_marker_prev());
        assert_eq!(core.current, 2);
        assert_eq!(core.status, "No earlier beat marker.");
        // Between beats: prev lands on the earlier one.
        core.goto(3);
        assert!(core.beat_marker_prev());
        assert_eq!(core.current, 2);
    }

    #[test]
    fn jumps_on_an_empty_list_and_a_shared_frame() {
        let mut core = EditorCore::new();
        assert!(!core.beat_marker_next());
        assert!(!core.beat_marker_prev());
        assert_eq!(core.current, 0);
        // Two markers sharing one frame: a single landing
        // frame — next reaches it once, and on it both
        // directions refuse.
        core.beat_marker_add(3, "The reveal");
        core.beat_marker_add(3, "The joke lands");
        assert!(core.beat_marker_next());
        assert_eq!(core.current, 3);
        assert!(!core.beat_marker_next());
        assert!(!core.beat_marker_prev());
        assert_eq!(core.current, 3);
    }

    #[test]
    fn markers_past_the_end_are_stored_but_inert() {
        let mut core = EditorCore::new();
        assert_eq!(core.frame_count(), 5);
        // Not clamped: frame 99 is stored verbatim on a
        // five-frame project (the chapter / cue posture).
        assert_eq!(core.beat_marker_add(99, "Finale"), Some(0));
        assert_eq!(frames(&core), [99]);
        // The pure queries still see it...
        assert_eq!(core.project.next_beat_frame(0), Some(99));
        assert_eq!(core.beat_marker_at(99).map(|m| m.name.as_str()), Some("Finale"));
        // ...but the jump op does not target it: it is not
        // on the timeline until the timeline grows to it.
        assert!(!core.beat_marker_next());
        assert_eq!(core.current, 0);
    }

    #[test]
    fn frame_ops_do_not_remap_beat_markers() {
        // The no-remap posture, pinned against the one
        // citizen that DOES remap: on a frame delete the
        // camera key shifts with its frame (IA-RM18-1)
        // while both markers keep their authored numbers.
        let mut core = EditorCore::new();
        core.project.camera = vec![crate::model::CameraKey {
            frame: 4,
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
            angle: 0.0,
            easing: crate::model::CameraEase::Linear,
        }];
        core.beat_marker_add(2, "The Reveal");
        core.beat_marker_add(4, "The Turn");
        core.goto(0);
        core.delete_frame();
        assert_eq!(core.frame_count(), 4);
        assert_eq!(core.project.camera[0].frame, 3, "camera remaps (IA-RM18-1)");
        assert_eq!(frames(&core), [2, 4], "beat markers do not remap");
        assert_eq!(core.beat_marker_count(), 2, "deleting a frame deletes no marker");
        // Inserting a frame is the same story.
        core.add_frame();
        assert_eq!(frames(&core), [2, 4]);
    }
}
