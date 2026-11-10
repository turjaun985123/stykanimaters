//! Roadmap Wave 2 (W2-91, E-1) — starter template pins.
//!
//! The three files under `assets/templates/` are real project
//! files offered by the Home New-project flow. These tests are
//! their loader contract: every template must load through
//! [`projects::load`] itself (the exact rule a user's open
//! takes), carry the structure its name promises, and survive
//! a re-serialisation round-trip, so a template can never rot
//! into a file the app cannot open.

use stykanimate_rs::model::{self, InstanceKey, Project, FORMAT_VERSION_CURRENT};
use stykanimate_rs::projects;
use stykanimate_rs::subtitles::SubtitleCue;

fn load_template(file: &str) -> Project {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/templates/").to_string() + file;
    projects::load(std::path::Path::new(&path))
        .unwrap_or_else(|e| panic!("template {file} must load through projects::load: {e}"))
}

#[test]
fn blank_episode_template_loads_and_scaffolds_chapters() {
    let p = load_template("episode-blank.json");
    assert_eq!(p.format_version, FORMAT_VERSION_CURRENT);
    assert_eq!(p.name, "Blank Episode");
    assert_eq!(p.fps, 24.0);
    assert_eq!(p.stage, [960.0, 720.0]);
    assert_eq!(p.figures.len(), 1);
    assert_eq!(p.figures[0].name, "Hero");
    assert_eq!(p.figures[0].def.nodes.len(), 11, "house humanoid rig");
    // Three chapters of sixteen frames; each chapter opens with
    // a held title frame (duration 24 = one second at 24 fps).
    assert_eq!(p.frames.len(), 48);
    for (i, f) in p.frames.iter().enumerate() {
        assert_eq!(f.poses.len(), 1, "frame {i}: one pose per figure");
        assert_eq!(f.poses[0].angles.len(), 11, "frame {i}: one angle per node");
        let want = if i % 16 == 0 { 24.0 } else { 1.0 };
        assert_eq!(f.duration, want, "frame {i} duration");
    }
    let cards: Vec<&str> = p.texts.iter().map(|t| t.content.as_str()).collect();
    assert_eq!(cards, ["CHAPTER ONE", "CHAPTER TWO", "CHAPTER THREE"]);
    assert!(p.subtitles.is_empty(), "a blank scaffold carries no cues");
}

#[test]
fn blank_episode_title_cards_show_only_in_their_chapter() {
    let p = load_template("episode-blank.json");
    // (probe frame in authored-frame space, the card index that
    // must be fully visible there) — title holds included.
    for (frame_f, visible) in [(0.5, 0), (8.0, 0), (16.5, 1), (24.0, 1), (32.5, 2), (40.0, 2)] {
        for (i, card) in p.texts.iter().enumerate() {
            let key = InstanceKey::sample(&card.keys, frame_f)
                .unwrap_or_else(|| panic!("card {i} must sample at {frame_f}"));
            if i == visible {
                assert_eq!(key.opacity, 1.0, "card {i} fully visible at {frame_f}");
            } else {
                assert_eq!(key.opacity, 0.0, "card {i} hidden at {frame_f}");
            }
        }
    }
}

#[test]
fn dialogue_scene_template_loads_with_cues_and_speakers() {
    let p = load_template("dialogue-scene.json");
    assert_eq!(p.name, "Dialogue Scene");
    assert_eq!(p.figures.len(), 2);
    assert_eq!(p.figures[0].name, "Nova");
    assert_eq!(p.figures[1].name, "Bram");
    assert_ne!(
        p.figures[0].color, p.figures[1].color,
        "each character keeps a unique colour (stickman channel rule)"
    );
    assert_eq!(p.frames.len(), 144);
    // The gesture beats are baked: frame 36 is Nova's raised-arm
    // key, frame 72 Bram's.
    assert_eq!(p.frames[36].poses[0].angles[5], -60.0, "Nova gestures at 36");
    assert_eq!(p.frames[72].poses[1].angles[3], 60.0, "Bram answers at 72");
    let want: [(usize, usize, &str, &str); 3] = [
        (6, 60, "We should go home before dark.", "Nova"),
        (66, 114, "One more minute. Look at the sky.", "Bram"),
        (114, 144, "Then we run.", "Nova"),
    ];
    assert_eq!(p.subtitles.len(), want.len());
    for (cue, (start, end, text, speaker)) in p.subtitles.iter().zip(want.iter()) {
        assert_eq!(cue.start_frame, *start);
        assert_eq!(cue.end_frame, *end);
        assert_eq!(cue.text, *text);
        assert_eq!(cue.speaker.as_deref(), Some(*speaker));
        // The ops' own rule, so the panel accepts every cue.
        SubtitleCue::validate(cue.start_frame, cue.end_frame, &cue.text)
            .unwrap_or_else(|e| panic!("template cue must validate: {e}"));
        // Speakers name figures — the dialogue lane's binding.
        assert!(
            p.figures.iter().any(|f| f.name == *speaker),
            "speaker {speaker} is a figure in the scene"
        );
    }
}

#[test]
fn acting_scene_template_loads_with_camera_moves() {
    let p = load_template("acting-scene.json");
    assert_eq!(p.name, "Two-Figure Acting Scene");
    assert_eq!(p.figures.len(), 2);
    assert_eq!(p.figures[0].name, "Mara");
    assert_eq!(p.figures[1].name, "Theo");
    assert_ne!(p.figures[0].color, p.figures[1].color);
    assert_eq!(p.frames.len(), 120);
    // The walk travelled: Mara starts stage-left, arrives by 30.
    assert_eq!(p.frames[0].poses[0].root[0], 260.0);
    assert_eq!(p.frames[60].poses[0].root[0], 380.0);
    // The acting beats: Mara's arm is up through the hold, Theo
    // answers at 90.
    assert_eq!(p.frames[60].poses[0].angles[5], -170.0, "Mara's arm raised");
    assert_eq!(p.frames[90].poses[1].angles[3], 170.0, "Theo answers");
    // Camera: four keys, ascending, a push-in (zoom grows).
    let keys: Vec<usize> = p.camera.iter().map(|k| k.frame).collect();
    assert_eq!(keys, [0, 45, 90, 119]);
    assert!(
        p.camera.last().unwrap().zoom > p.camera.first().unwrap().zoom,
        "the move pushes in"
    );
}

#[test]
fn all_templates_evaluate_finite_poses() {
    for file in ["episode-blank.json", "dialogue-scene.json", "acting-scene.json"] {
        let p = load_template(file);
        let last = (p.frames.len() - 1) as f32;
        for frame_f in [0.0, last / 2.0, last] {
            let poses = p.poses_at(frame_f);
            assert_eq!(poses.len(), p.figures.len(), "{file} at {frame_f}");
            for (fig, pose) in p.figures.iter().zip(poses.iter()) {
                let joints = model::evaluate(&fig.def, pose);
                assert_eq!(joints.len(), fig.def.nodes.len());
                for (j, joint) in joints.iter().enumerate() {
                    assert!(
                        joint.iter().all(|v| v.is_finite()),
                        "{file} figure {} joint {j} finite at {frame_f}",
                        fig.name
                    );
                }
            }
        }
    }
}

#[test]
fn templates_reserialise_to_loadable_projects() {
    // A template is a fixed point of the app's own serialisation:
    // what serde_json writes from the loaded value loads again,
    // with the same name and shape.
    for file in ["episode-blank.json", "dialogue-scene.json", "acting-scene.json"] {
        let p = load_template(file);
        let text = serde_json::to_string_pretty(&p).expect("template serialises");
        let again: Project = serde_json::from_str(&text).expect("template re-loads");
        assert_eq!(again.name, p.name, "{file} name stable");
        assert_eq!(again.frames.len(), p.frames.len(), "{file} frames stable");
        assert_eq!(again.figures.len(), p.figures.len(), "{file} figures stable");
        assert_eq!(again.subtitles.len(), p.subtitles.len(), "{file} cues stable");
        assert_eq!(again.camera.len(), p.camera.len(), "{file} camera stable");
    }
}

#[test]
fn templates_are_distinct_starters_at_house_defaults() {
    let mut names = Vec::new();
    for file in ["episode-blank.json", "dialogue-scene.json", "acting-scene.json"] {
        let p = load_template(file);
        assert_ne!(p.name, "Untitled", "{file} is a named starter");
        assert_eq!(p.fps, 24.0, "{file} at the episode rate");
        assert_eq!(p.stage, [960.0, 720.0], "{file} on the house stage");
        assert!(p.tween_playback, "{file} plays its baked motion");
        names.push(p.name.clone());
    }
    names.sort();
    names.dedup();
    assert_eq!(names.len(), 3, "three distinct starters");
}
