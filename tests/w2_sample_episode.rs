//! Roadmap Wave 2 (W2-92, E-2) — sample episode pins.
//!
//! `assets/samples/the-bus-stop.json` is a complete demo
//! episode in the app's own format, shipped as sample
//! content. These tests are its loader contract, following
//! `w2_templates.rs`: the file must load through
//! [`projects::load`] itself, carry the episode its README
//! promises (chapters, subtitled dialogue with speakers, a
//! camera move, a placed sprite whose asset really decodes),
//! and survive a re-serialisation round-trip, so the sample
//! can never rot into a file the app cannot open.

use stykanimate_rs::media;
use stykanimate_rs::model::{self, CameraEase, InstanceKey, Project, FORMAT_VERSION_CURRENT};
use stykanimate_rs::projects;
use stykanimate_rs::subtitles::SubtitleCue;

fn load_sample() -> Project {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/samples/the-bus-stop.json");
    projects::load(std::path::Path::new(path))
        .unwrap_or_else(|e| panic!("sample episode must load through projects::load: {e}"))
}

#[test]
fn sample_episode_loads_with_episode_structure() {
    let p = load_sample();
    assert_eq!(p.format_version, FORMAT_VERSION_CURRENT);
    assert_eq!(p.name, "The Bus Stop");
    assert_eq!(p.fps, 24.0);
    assert_eq!(p.stage, [960.0, 720.0]);
    assert_eq!(p.figures.len(), 2);
    assert_eq!(p.figures[0].name, "Pip");
    assert_eq!(p.figures[1].name, "Rue");
    assert_ne!(
        p.figures[0].color, p.figures[1].color,
        "each character keeps a unique colour (stickman channel rule)"
    );
    for fig in &p.figures {
        assert_eq!(fig.def.nodes.len(), 11, "{} rides the house humanoid rig", fig.name);
    }
    assert_eq!(p.frames.len(), 96);
    for (i, f) in p.frames.iter().enumerate() {
        assert_eq!(f.poses.len(), 2, "frame {i}: one pose per figure");
        for pose in &f.poses {
            assert_eq!(pose.angles.len(), 11, "frame {i}: one angle per node");
        }
    }
    // Two real chapters partitioning the timeline, half-open.
    let chapters: Vec<(&str, usize, usize)> = p
        .chapters
        .iter()
        .map(|c| (c.title.as_str(), c.start_frame, c.end_frame))
        .collect();
    assert_eq!(chapters, [("Waiting", 0, 48), ("The Bus Arrives", 48, 96)]);
    assert_eq!(p.chapter_at(0).map(|c| c.title.as_str()), Some("Waiting"));
    assert_eq!(p.chapter_at(47).map(|c| c.title.as_str()), Some("Waiting"));
    assert_eq!(p.chapter_index_at(48), Some(1), "frame 48 opens chapter two");
    assert_eq!(p.chapter_at(95).map(|c| c.title.as_str()), Some("The Bus Arrives"));
    assert_eq!(p.chapter_index_at(96), None, "the range is half-open at the end");
}

#[test]
fn sample_episode_dialogue_cues_bind_speakers_and_chapters() {
    let p = load_sample();
    let want: [(usize, usize, &str, &str, usize); 3] = [
        (4, 44, "It should be here by now.", "Pip", 0),
        (50, 74, "Listen — I hear it!", "Rue", 1),
        (78, 95, "There it is!", "Pip", 1),
    ];
    assert_eq!(p.subtitles.len(), want.len());
    for (cue, (start, end, text, speaker, chapter)) in p.subtitles.iter().zip(want.iter()) {
        assert_eq!(cue.start_frame, *start);
        assert_eq!(cue.end_frame, *end);
        assert_eq!(cue.text, *text, "dialogue is verbatim");
        assert_eq!(cue.speaker.as_deref(), Some(*speaker));
        SubtitleCue::validate(cue.start_frame, cue.end_frame, &cue.text)
            .unwrap_or_else(|e| panic!("sample cue must validate: {e}"));
        assert!(
            p.figures.iter().any(|f| f.name == *speaker),
            "speaker {speaker} is a figure in the episode"
        );
        // The dialogue lane's binding: a line's chapter is the
        // chapter covering its start frame.
        assert_eq!(
            p.chapter_index_at(cue.start_frame),
            Some(*chapter),
            "cue at {} binds to chapter {chapter}",
            cue.start_frame
        );
        assert!(cue.end_frame <= p.frames.len(), "cue ends within the timeline");
    }
}

#[test]
fn sample_episode_acting_beats_are_baked() {
    let p = load_sample();
    // Pip waits stage-left, waves through frame 24, then steps
    // toward Rue across the chapter break, arriving by 72.
    assert_eq!(p.frames[0].poses[0].root, [330.0, 470.0]);
    assert_eq!(p.frames[24].poses[0].angles[5], -170.0, "Pip's wave peaks at 24");
    assert_eq!(p.frames[24].poses[0].angles[6], -20.0, "forearm bends into the wave");
    assert_eq!(p.frames[48].poses[0].angles[5], -118.0, "arm back at rest for chapter two");
    assert_eq!(p.frames[72].poses[0].root, [430.0, 470.0], "Pip has stepped closer");
    // Rue answers with her left arm from frame 78 on.
    assert_eq!(p.frames[78].poses[1].angles[3], 170.0, "Rue waves back at 78");
    assert_eq!(p.frames[95].poses[1].angles[3], 170.0, "and holds it to the end");
    assert!(p.tween_playback, "the episode plays its baked motion");
}

#[test]
fn sample_episode_camera_move_pushes_in() {
    let p = load_sample();
    let keys: Vec<usize> = p.camera.iter().map(|k| k.frame).collect();
    assert_eq!(keys, [0, 48, 95], "three keys, strictly ascending");
    assert_eq!(p.camera[0].easing, CameraEase::Smooth);
    assert_eq!(p.camera[1].easing, CameraEase::Smooth);
    // Endpoints are exact (the W2-32 contract); the move pans
    // right and pushes in.
    assert_eq!(p.camera_at(0.0), (480.0, 360.0, 1.0));
    assert_eq!(p.camera_at(95.0), (520.0, 350.0, 1.3));
    let (_, _, mid_zoom) = p.camera_at(24.0);
    assert!(
        mid_zoom > 1.0 && mid_zoom < 1.15,
        "mid-segment zoom {mid_zoom} is inside the move"
    );
}

#[test]
fn sample_episode_sprite_asset_really_decodes() {
    let p = load_sample();
    assert_eq!(p.assets.len(), 1);
    assert_eq!(p.assets[0].name, "sun.png");
    // The app's own decode path — a stored byte salad would
    // fail here even though serde accepted it.
    let (w, h, _) = media::asset_pixels(&p, 0).expect("the sun asset decodes");
    assert_eq!((w, h), (16, 16));
    assert_eq!(p.sprites.len(), 1);
    assert_eq!(p.sprites[0].asset, 0, "the sprite places asset 0");
    let key = InstanceKey::sample(&p.sprites[0].keys, 40.0).expect("sprite samples mid-episode");
    assert_eq!((key.x, key.y, key.scale), (118.0, 104.0, 3.0), "the sun hangs over the stop");
}

#[test]
fn sample_episode_evaluates_and_round_trips() {
    let p = load_sample();
    for frame_f in [0.0, 47.5, 95.0] {
        let poses = p.poses_at(frame_f);
        assert_eq!(poses.len(), p.figures.len(), "poses at {frame_f}");
        for (fig, pose) in p.figures.iter().zip(poses.iter()) {
            let joints = model::evaluate(&fig.def, pose);
            assert_eq!(joints.len(), fig.def.nodes.len());
            for (j, joint) in joints.iter().enumerate() {
                assert!(
                    joint.iter().all(|v| v.is_finite()),
                    "figure {} joint {j} finite at {frame_f}",
                    fig.name
                );
            }
        }
    }
    // A fixed point of the app's own serialisation, chapters
    // and sprite included.
    let text = serde_json::to_string_pretty(&p).expect("sample serialises");
    let again: Project = serde_json::from_str(&text).expect("sample re-loads");
    assert_eq!(again.name, p.name);
    assert_eq!(again.frames.len(), p.frames.len());
    assert_eq!(again.figures.len(), p.figures.len());
    assert_eq!(again.chapters, p.chapters, "chapters survive the round-trip");
    assert_eq!(again.subtitles.len(), p.subtitles.len());
    assert_eq!(again.camera.len(), p.camera.len());
    assert_eq!(again.sprites.len(), p.sprites.len());
    assert_eq!(again.assets.len(), p.assets.len());
}
