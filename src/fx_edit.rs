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
        ClipKind::Fireworks => "fireworks",
        ClipKind::Aurora => "aurora",
        ClipKind::Comet => "comet",
        ClipKind::Galaxy => "galaxy",
        ClipKind::Prism => "prism",
        ClipKind::Vortex => "vortex",
        ClipKind::Glitter => "glitter",
        ClipKind::Wave => "wave",
        ClipKind::Shard => "shard",
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

// ----------------------------------------------------------------
// FX clip presets on disk
//
// A preset is one `ParticleClip` stored as JSON in
// `./fx-presets/<slug>.json`, mirroring how the figure library
// persists JSON (`fig3d::save_library_figure`): the directory is
// created on save, the value is written with
// `serde_json::to_string_pretty`, and listing scans the directory
// for `*.json` files. The public functions fix the directory to
// `./fx-presets/`; the private `*_in` helpers take the directory
// explicitly so tests can exercise the real file I/O against a
// temporary directory instead of the process working directory.
// ----------------------------------------------------------------

/// Directory (relative to the working directory) holding the FX
/// clip preset files.
const PRESET_DIR: &str = "fx-presets";

/// Slug a preset name maps to on disk: ASCII letters are
/// lowercased, `[A-Za-z0-9-_]` are kept as they are, and every
/// other character becomes `-`. The mapping is per character (no
/// collapsing or trimming), so the slug is empty only when the
/// name itself is empty.
fn preset_slug(name: &str) -> String {
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}

/// Path of the preset file for `slug` inside directory `dir`.
fn preset_path(dir: &std::path::Path, slug: &str) -> std::path::PathBuf {
    dir.join(format!("{slug}.json"))
}

/// Save `clip` as preset `name` in directory `dir` (created if
/// missing), returning the file's path. An empty slug (an empty
/// name) is an error and writes nothing.
fn save_clip_preset_in(
    dir: &std::path::Path,
    clip: &crate::vfx::ParticleClip,
    name: &str,
) -> Result<std::path::PathBuf, String> {
    let slug = preset_slug(name);
    if slug.is_empty() {
        return Err(format!(
            "cannot save FX preset '{name}': the name produces an empty file slug"
        ));
    }
    std::fs::create_dir_all(dir)
        .map_err(|e| format!("could not create FX preset directory {}: {e}", dir.display()))?;
    let text = serde_json::to_string_pretty(clip)
        .map_err(|e| format!("could not serialise FX preset '{name}': {e}"))?;
    // Wave 10 (C7): never silently destroy a different
    // preset. The shell's auto-names are positional
    // (`<kind>-<index+1>`), so after the clip list changes
    // a save can land on an earlier preset's name — and
    // the old write replaced that file wholesale. Now the
    // write claims its file with `create_new`: an existing
    // file holding the SAME preset (byte-identical — it
    // was written by this same serialiser) is a successful
    // no-op re-save; an existing file holding a different
    // preset keeps its name and the new preset takes the
    // next free suffix (`<slug>-2`, `<slug>-3`, …). The
    // returned path always names the file actually
    // written, so callers report the truth.
    use std::io::Write as _;
    let mut suffix = 1usize;
    loop {
        let candidate = if suffix == 1 {
            slug.clone()
        } else {
            format!("{slug}-{suffix}")
        };
        let path = preset_path(dir, &candidate);
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                return file
                    .write_all(text.as_bytes())
                    .map(|_| path.clone())
                    .map_err(|e| {
                        format!(
                            "could not write FX preset '{name}' to {}: {e}",
                            path.display()
                        )
                    });
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                if std::fs::read_to_string(&path).map(|t| t == text).unwrap_or(false) {
                    return Ok(path);
                }
                suffix += 1;
                if suffix > 999 {
                    return Err(format!(
                        "cannot save FX preset '{name}': every suffixed name is taken"
                    ));
                }
            }
            Err(e) => {
                return Err(format!(
                    "could not write FX preset '{name}' to {}: {e}",
                    path.display()
                ));
            }
        }
    }
}

/// Load the preset called `name` from directory `dir`. A missing
/// file is an error whose message names the preset, as is an
/// unreadable or malformed one.
fn load_clip_preset_in(
    dir: &std::path::Path,
    name: &str,
) -> Result<crate::vfx::ParticleClip, String> {
    let slug = preset_slug(name);
    if slug.is_empty() {
        return Err(format!(
            "cannot load FX preset '{name}': the name produces an empty file slug"
        ));
    }
    let path = preset_path(dir, &slug);
    let text = std::fs::read_to_string(&path).map_err(|e| {
        format!(
            "could not read FX preset '{name}' from {}: {e}",
            path.display()
        )
    })?;
    serde_json::from_str(&text)
        .map_err(|e| format!("FX preset '{name}' in {} is not valid: {e}", path.display()))
}

/// Sorted slugs of every preset stored in directory `dir` (the
/// `*.json` file stems). A missing or unreadable directory yields
/// an empty list.
fn list_clip_presets_in(dir: &std::path::Path) -> Vec<String> {
    let mut slugs = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            if let Some(file_name) = file_name.to_str() {
                if let Some(slug) = file_name.strip_suffix(".json") {
                    slugs.push(slug.to_string());
                }
            }
        }
    }
    slugs.sort();
    slugs
}

/// Save `clip` as a named preset in `./fx-presets/<slug>.json`
/// (the directory is created if missing) and return the file's
/// path. See [`preset_slug`] for how the name maps to the slug;
/// an empty slug is an error.
pub fn save_clip_preset(
    clip: &crate::vfx::ParticleClip,
    name: &str,
) -> Result<std::path::PathBuf, String> {
    save_clip_preset_in(std::path::Path::new(PRESET_DIR), clip, name)
}

/// Load the preset called `name` from `./fx-presets/` (the same
/// slug rule as [`save_clip_preset`]). A missing file is an error
/// whose message names the preset.
pub fn load_clip_preset(name: &str) -> Result<crate::vfx::ParticleClip, String> {
    load_clip_preset_in(std::path::Path::new(PRESET_DIR), name)
}

/// Sorted slugs of every preset in `./fx-presets/`; empty when
/// the directory does not exist.
pub fn list_clip_presets() -> Vec<String> {
    list_clip_presets_in(std::path::Path::new(PRESET_DIR))
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
                color2: None,
                seed,
                angle: 90.0,
                length: 160.0,
            },
            figure: 2,
            node: 5,
            hidden: false,
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
            ClipKind::Fireworks,
            ClipKind::Aurora,
            ClipKind::Comet,
            ClipKind::Galaxy,
            ClipKind::Prism,
            ClipKind::Vortex,
            ClipKind::Glitter,
            ClipKind::Wave,
            ClipKind::Shard,
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

#[cfg(test)]
mod preset_tests {
    // The public preset functions are hard-wired to ./fx-presets/,
    // and no test in this crate changes the process working
    // directory (it is process-global: parallel tests and other
    // modules' ./-relative paths would race). These tests instead
    // drive the dir-taking helpers the public functions delegate
    // to, against a pid-unique temporary directory, so the real
    // file I/O, slug and JSON code paths are all exercised.
    use super::{list_clip_presets_in, load_clip_preset_in, preset_slug, save_clip_preset_in};
    use crate::vfx::{ClipKind, ParticleClip};
    use std::path::PathBuf;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("styk-fx-presets-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn sample_clip() -> ParticleClip {
        ParticleClip {
            start: 12.5,
            duration: 30.0,
            kind: ClipKind::Galaxy,
            color: 0xFFC65D,
            color2: None,
            seed: 424_242,
            angle: -45.0,
            length: 220.0,
        }
    }

    fn assert_same_clip(a: &ParticleClip, b: &ParticleClip) {
        assert_eq!(a.kind, b.kind, "kind");
        assert_eq!(a.start, b.start, "start");
        assert_eq!(a.duration, b.duration, "duration");
        assert_eq!(a.color, b.color, "colour");
        assert_eq!(a.color2, b.color2, "gradient end colour");
        assert_eq!(a.seed, b.seed, "seed");
        assert_eq!(a.angle, b.angle, "angle");
        assert_eq!(a.length, b.length, "length");
    }

    #[test]
    fn slug_keeps_word_chars_maps_the_rest_and_lowercases() {
        assert_eq!(preset_slug("Golden Burst"), "golden-burst");
        assert_eq!(preset_slug("My FX_2"), "my-fx_2");
        assert_eq!(preset_slug("a--b"), "a--b", "existing dashes are kept");
        assert_eq!(preset_slug("!!!"), "---", "per-character mapping, no collapse");
        assert_eq!(preset_slug("Café"), "caf-", "non-ASCII maps to '-'");
        assert_eq!(preset_slug(""), "", "only an empty name slugs to empty");
    }

    #[test]
    fn save_with_an_empty_name_is_an_error_and_writes_nothing() {
        let dir = temp_dir("empty-name");
        let err = save_clip_preset_in(&dir, &sample_clip(), "").unwrap_err();
        assert!(err.contains("empty"), "error explains the slug: {err}");
        assert!(!dir.exists(), "the slug check runs before any directory is created");
    }

    #[test]
    fn save_then_load_round_trips_every_field() {
        let dir = temp_dir("round-trip");
        let clip = sample_clip();
        let path = save_clip_preset_in(&dir, &clip, "My Galaxy!").unwrap();
        assert_eq!(path, dir.join("my-galaxy-.json"));
        assert!(path.is_file());
        let loaded = load_clip_preset_in(&dir, "My Galaxy!").unwrap();
        assert_same_clip(&loaded, &clip);
        // The slug itself is a valid name for the same file.
        let by_slug = load_clip_preset_in(&dir, "my-galaxy-").unwrap();
        assert_same_clip(&by_slug, &clip);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_never_overwrites_a_different_preset() {
        // Wave 10 (C7): the shell's auto-names are
        // positional, so a save can land on an earlier
        // preset's name after the clip list changed. The
        // first preset must survive byte-for-byte and the
        // new one takes the next free suffix; re-saving
        // the SAME preset is a no-op on the same path.
        let dir = temp_dir("no-overwrite");
        let first = sample_clip();
        let p1 = save_clip_preset_in(&dir, &first, "sparkle-1").unwrap();
        assert_eq!(p1, dir.join("sparkle-1.json"));
        let before = std::fs::read_to_string(&p1).unwrap();

        let mut other = sample_clip();
        other.duration = first.duration + 5.0;
        let p2 = save_clip_preset_in(&dir, &other, "sparkle-1").unwrap();
        assert_eq!(p2, dir.join("sparkle-1-2.json"));
        assert_eq!(std::fs::read_to_string(&p1).unwrap(), before);
        assert_same_clip(&load_clip_preset_in(&dir, "sparkle-1").unwrap(), &first);
        assert_same_clip(&load_clip_preset_in(&dir, "sparkle-1-2").unwrap(), &other);

        let p3 = save_clip_preset_in(&dir, &first, "sparkle-1").unwrap();
        assert_eq!(p3, p1, "re-saving the same preset keeps its path");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_of_a_missing_preset_errors_with_the_name() {
        let dir = temp_dir("missing");
        let err = load_clip_preset_in(&dir, "no-such-preset").unwrap_err();
        assert!(
            err.contains("no-such-preset"),
            "error names the preset: {err}"
        );
    }

    #[test]
    fn list_is_empty_without_a_directory_then_sorted_slugs() {
        let dir = temp_dir("list");
        assert!(
            list_clip_presets_in(&dir).is_empty(),
            "missing directory lists empty"
        );
        let clip = sample_clip();
        save_clip_preset_in(&dir, &clip, "Beta").unwrap();
        save_clip_preset_in(&dir, &clip, "alpha").unwrap();
        std::fs::write(dir.join("notes.txt"), "not a preset").unwrap();
        assert_eq!(list_clip_presets_in(&dir), vec!["alpha", "beta"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
