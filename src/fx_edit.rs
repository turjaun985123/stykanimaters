//! FX-clip management: the editing backend for the project's
//! timed particle clips (`Project::fx_clips`).
//!
//! `vfx.rs` knows how a clip *renders*; this module knows how the
//! list of clips is *edited* — listing, moving and resizing clips
//! on the timeline, duplicating and deleting them, and asking
//! which clips are live at a given time. Everything is a pure
//! function over the project, so an FX timeline panel (or any
//! other caller) can build on it without editor state.
//!
//! The clip window convention matches `vfx::clip_particles`:
//! a clip is live for `start <= frame < start + duration`, and a
//! clip with a non-positive duration is never live.

use crate::model::Project;
use crate::vfx::ClipKind;

/// A read-only summary of one entry in `Project::fx_clips`.
#[derive(Clone, Debug, PartialEq)]
pub struct FxInfo {
    /// Position in `Project::fx_clips`.
    pub index: usize,
    /// Figure the clip is anchored to.
    pub figure: usize,
    /// Node of that figure the clip is anchored to.
    pub node: usize,
    /// Snake-case kind name, matching the MCP tool vocabulary.
    pub kind: &'static str,
    /// First frame the clip covers.
    pub start: f32,
    /// Length of the clip in frames.
    pub duration: f32,
    /// `start + duration`: the first frame the clip no longer
    /// covers.
    pub end: f32,
}

/// Snake-case name of a clip kind (the same vocabulary the MCP
/// tools accept).
pub fn kind_name(kind: ClipKind) -> &'static str {
    match kind {
        ClipKind::Burst => "burst",
        ClipKind::Sparkle => "sparkle",
        ClipKind::Smoke => "smoke",
        ClipKind::Beam => "beam",
        ClipKind::Aura => "aura",
        ClipKind::Trail => "trail",
        ClipKind::Confetti => "confetti",
        ClipKind::ShockRing => "shock_ring",
        ClipKind::Ember => "ember",
        ClipKind::Fountain => "fountain",
        ClipKind::Orbit => "orbit",
        ClipKind::Lightning => "lightning",
        ClipKind::Bubbles => "bubbles",
        ClipKind::Leaves => "leaves",
        ClipKind::Meteor => "meteor",
        ClipKind::Petals => "petals",
        ClipKind::Splash => "splash",
        ClipKind::Spiral => "spiral",
    }
}

/// Summaries of every FX clip in the project, in list order.
pub fn list(project: &Project) -> Vec<FxInfo> {
    project
        .fx_clips
        .iter()
        .enumerate()
        .map(|(index, entry)| FxInfo {
            index,
            figure: entry.figure,
            node: entry.node,
            kind: kind_name(entry.clip.kind),
            start: entry.clip.start,
            duration: entry.clip.duration,
            end: entry.clip.start + entry.clip.duration,
        })
        .collect()
}

/// Move clip `idx` to start at `new_start` (clamped to >= 0).
/// Returns false when `idx` is out of range.
pub fn move_clip(project: &mut Project, idx: usize, new_start: f32) -> bool {
    let Some(entry) = project.fx_clips.get_mut(idx) else {
        return false;
    };
    entry.clip.start = new_start.max(0.0);
    true
}

/// Set clip `idx`'s duration to `new_duration` (clamped to
/// >= 0.1, so a clip never collapses to nothing). Returns false
/// when `idx` is out of range.
pub fn resize_clip(project: &mut Project, idx: usize, new_duration: f32) -> bool {
    let Some(entry) = project.fx_clips.get_mut(idx) else {
        return false;
    };
    entry.clip.duration = new_duration.max(0.1);
    true
}

/// Duplicate clip `idx`: the copy is inserted right after the
/// original in the list and placed right after it in time (it
/// starts where the original ends). The copy's seed is bumped
/// deterministically (a golden-ratio hash step), so it does not
/// render identically to the original. Returns the copy's index,
/// or None when `idx` is out of range.
pub fn duplicate_clip(project: &mut Project, idx: usize) -> Option<usize> {
    let src = project.fx_clips.get(idx)?;
    let mut copy = src.clone();
    copy.clip.start = src.clip.start + src.clip.duration.max(0.0);
    copy.clip.seed = src.clip.seed.wrapping_add(0x9E37_79B9);
    project.fx_clips.insert(idx + 1, copy);
    Some(idx + 1)
}

/// Remove clip `idx` from the list. Returns false when `idx`
/// is out of range.
pub fn delete_clip(project: &mut Project, idx: usize) -> bool {
    if idx < project.fx_clips.len() {
        project.fx_clips.remove(idx);
        true
    } else {
        false
    }
}

/// Indices of the clips live at fractional frame `frame_f`
/// (half-open window, same rule as `vfx::clip_particles`),
/// sorted by clip start; clips sharing a start keep list order.
pub fn clips_at(project: &Project, frame_f: f32) -> Vec<usize> {
    let mut active: Vec<(f32, usize)> = project
        .fx_clips
        .iter()
        .enumerate()
        .filter(|(_, entry)| {
            let clip = &entry.clip;
            clip.duration > 0.0
                && frame_f >= clip.start
                && frame_f < clip.start + clip.duration
        })
        .map(|(index, entry)| (entry.clip.start, index))
        .collect();
    active.sort_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.1.cmp(&b.1))
    });
    active.into_iter().map(|(_, index)| index).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::FxClipEntry;
    use crate::vfx::ParticleClip;

    fn entry(start: f32, duration: f32, kind: ClipKind, seed: u32) -> FxClipEntry {
        FxClipEntry {
            clip: ParticleClip {
                start,
                duration,
                kind,
                color: 0xFFC65D,
                seed,
                angle: 90.0,
                length: 160.0,
            },
            figure: 2,
            node: 5,
        }
    }

    fn project_with(clips: Vec<FxClipEntry>) -> Project {
        let mut project = Project::new_demo();
        project.fx_clips = clips;
        project
    }

    #[test]
    fn list_reports_index_anchor_kind_and_window() {
        let project = project_with(vec![
            entry(4.0, 10.0, ClipKind::Burst, 1),
            entry(20.0, 6.0, ClipKind::Lightning, 2),
            entry(30.0, 8.0, ClipKind::Leaves, 3),
        ]);
        let infos = list(&project);
        assert_eq!(infos.len(), 3);
        assert_eq!(
            infos[0],
            FxInfo {
                index: 0,
                figure: 2,
                node: 5,
                kind: "burst",
                start: 4.0,
                duration: 10.0,
                end: 14.0,
            }
        );
        assert_eq!(infos[1].kind, "lightning");
        assert_eq!(infos[1].end, 26.0);
        assert_eq!(infos[2].kind, "leaves");
        // Empty project lists nothing.
        assert!(list(&project_with(vec![])).is_empty());
    }

    #[test]
    fn kind_names_cover_every_variant() {
        // One name per ClipKind variant; all distinct.
        let kinds = [
            ClipKind::Burst,
            ClipKind::Sparkle,
            ClipKind::Smoke,
            ClipKind::Beam,
            ClipKind::Aura,
            ClipKind::Trail,
            ClipKind::Confetti,
            ClipKind::ShockRing,
            ClipKind::Ember,
            ClipKind::Fountain,
            ClipKind::Orbit,
            ClipKind::Lightning,
            ClipKind::Bubbles,
            ClipKind::Leaves,
            ClipKind::Meteor,
            ClipKind::Petals,
            ClipKind::Splash,
            ClipKind::Spiral,
        ];
        let mut names: Vec<&str> = kinds.iter().map(|k| kind_name(*k)).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), kinds.len(), "kind names must be unique");
    }

    #[test]
    fn move_clip_clamps_to_zero_and_validates_index() {
        let mut project = project_with(vec![entry(4.0, 10.0, ClipKind::Burst, 1)]);
        assert!(move_clip(&mut project, 0, 12.5));
        assert_eq!(project.fx_clips[0].clip.start, 12.5);
        assert!(move_clip(&mut project, 0, -8.0));
        assert_eq!(project.fx_clips[0].clip.start, 0.0, "start clamps to 0");
        assert!(!move_clip(&mut project, 1, 5.0), "out of range");
        assert!(!move_clip(&mut project, usize::MAX, 5.0), "out of range");
        assert_eq!(project.fx_clips[0].clip.start, 0.0, "failed moves change nothing");
    }

    #[test]
    fn resize_clip_clamps_to_minimum_and_validates_index() {
        let mut project = project_with(vec![entry(4.0, 10.0, ClipKind::Burst, 1)]);
        assert!(resize_clip(&mut project, 0, 3.0));
        assert_eq!(project.fx_clips[0].clip.duration, 3.0);
        assert!(resize_clip(&mut project, 0, 0.0));
        assert_eq!(project.fx_clips[0].clip.duration, 0.1, "duration clamps to 0.1");
        assert!(resize_clip(&mut project, 0, -5.0));
        assert_eq!(project.fx_clips[0].clip.duration, 0.1);
        assert!(!resize_clip(&mut project, 7, 5.0), "out of range");
        assert_eq!(project.fx_clips[0].clip.duration, 0.1, "failed resize changes nothing");
    }

    #[test]
    fn duplicate_clip_places_copy_after_original_in_time_and_list() {
        let mut project = project_with(vec![
            entry(4.0, 10.0, ClipKind::Bubbles, 42),
            entry(50.0, 5.0, ClipKind::Burst, 7),
        ]);
        let new_idx = duplicate_clip(&mut project, 0).expect("valid index");
        assert_eq!(new_idx, 1);
        assert_eq!(project.fx_clips.len(), 3);
        let copy = &project.fx_clips[1];
        assert_eq!(copy.clip.start, 14.0, "copy starts where the original ends");
        assert_eq!(copy.clip.duration, 10.0);
        assert_eq!(copy.clip.kind, ClipKind::Bubbles);
        assert_eq!(copy.figure, 2);
        assert_eq!(copy.node, 5);
        assert_ne!(copy.clip.seed, 42, "seed bumped so the copy differs");
        // The original and the following clip are untouched.
        assert_eq!(project.fx_clips[0].clip.seed, 42);
        assert_eq!(project.fx_clips[0].clip.start, 4.0);
        assert_eq!(project.fx_clips[2].clip.start, 50.0);
        // Duplicating again from the copy chains further in time.
        let third = duplicate_clip(&mut project, 1).expect("copy is duplicable");
        assert_eq!(project.fx_clips[third].clip.start, 24.0);
        // Out of range.
        assert_eq!(duplicate_clip(&mut project, 99), None);
        assert_eq!(project.fx_clips.len(), 4, "failed duplicate changes nothing");
    }

    #[test]
    fn delete_clip_removes_and_shifts() {
        let mut project = project_with(vec![
            entry(0.0, 5.0, ClipKind::Burst, 1),
            entry(10.0, 5.0, ClipKind::Smoke, 2),
            entry(20.0, 5.0, ClipKind::Aura, 3),
        ]);
        assert!(delete_clip(&mut project, 1));
        assert_eq!(project.fx_clips.len(), 2);
        assert_eq!(project.fx_clips[1].clip.kind, ClipKind::Aura);
        assert_eq!(project.fx_clips[1].clip.start, 20.0);
        assert!(!delete_clip(&mut project, 2), "now out of range");
        assert!(!delete_clip(&mut project_with(vec![]), 0), "empty list");
        assert_eq!(project.fx_clips.len(), 2, "failed deletes change nothing");
    }

    #[test]
    fn clips_at_uses_the_half_open_window() {
        let project = project_with(vec![
            entry(10.0, 10.0, ClipKind::Burst, 1), // live [10, 20)
            entry(15.0, 10.0, ClipKind::Smoke, 2), // live [15, 25)
            entry(0.0, 0.0, ClipKind::Aura, 3),    // never live
        ]);
        assert!(clips_at(&project, 9.99).is_empty());
        assert_eq!(clips_at(&project, 10.0), vec![0], "start is inclusive");
        assert_eq!(clips_at(&project, 17.0), vec![0, 1]);
        assert_eq!(clips_at(&project, 20.0), vec![1], "end is exclusive");
        assert_eq!(clips_at(&project, 24.99), vec![1]);
        assert!(clips_at(&project, 25.0).is_empty());
        assert!(clips_at(&project_with(vec![]), 12.0).is_empty());
    }

    #[test]
    fn clips_at_sorts_by_start_not_list_order() {
        let project = project_with(vec![
            entry(30.0, 20.0, ClipKind::Burst, 1), // index 0, starts last
            entry(10.0, 40.0, ClipKind::Smoke, 2), // index 1, starts first
            entry(10.0, 5.0, ClipKind::Aura, 3),   // index 2, ties with 1
        ]);
        // At 35: clips 0 and 1 are live; sorted by start -> [1, 0].
        assert_eq!(clips_at(&project, 35.0), vec![1, 0]);
        // At 12: clips 1 and 2 are live with equal starts; the
        // tie keeps list order.
        assert_eq!(clips_at(&project, 12.0), vec![1, 2]);
    }
}
