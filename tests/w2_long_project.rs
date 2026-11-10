//! Roadmap Wave 2 (W2-74, PL-13) — long-project stress fixture pins.
//!
//! `assets/fixtures/long-project.json` is the wave's stress
//! fixture: 600 frames, 12 figures, a realistic dialogue load.
//! These tests are its loader contract, following
//! `w2_sample_episode.rs`: the file must load through
//! [`projects::load`] itself, carry the structure its README
//! promises at the scale it promises, evaluate finite
//! everywhere, and survive a re-serialisation round-trip, so
//! the fixture can never rot into a file the app cannot open —
//! and so the measured behaviour in W2-74's findings stays
//! reproducible against the exact file that was measured.

use stykanimate_rs::graph::ChannelTarget;
use stykanimate_rs::media;
use stykanimate_rs::model::{self, CameraEase, InstanceKey, Project, FORMAT_VERSION_CURRENT};
use stykanimate_rs::projects;
use stykanimate_rs::subtitles::SubtitleCue;

fn load_fixture() -> Project {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/fixtures/long-project.json"
    );
    projects::load(std::path::Path::new(path))
        .unwrap_or_else(|e| panic!("stress fixture must load through projects::load: {e}"))
}

#[test]
fn long_project_loads_at_full_scale() {
    let p = load_fixture();
    assert_eq!(p.format_version, FORMAT_VERSION_CURRENT);
    assert_eq!(p.name, "The Long Platform (Stress Fixture)");
    assert_eq!(p.fps, 24.0);
    assert_eq!(p.stage, [1280.0, 720.0]);
    assert_eq!(p.figures.len(), 12, "twelve figures");
    let names: Vec<&str> = p.figures.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Ash", "Bea", "Cy", "Dee", "Eli", "Fay", "Gus", "Hal", "Ivy", "Jay", "Tot", "Biscuit"
        ]
    );
    let mut colours: Vec<u32> = p.figures.iter().map(|f| f.color).collect();
    colours.sort_unstable();
    colours.dedup();
    assert_eq!(colours.len(), 12, "each character keeps a unique colour");
    // Three rigs: ten humanoids, one simple, one quadruped.
    let nodes: Vec<usize> = p.figures.iter().map(|f| f.def.nodes.len()).collect();
    assert_eq!(nodes, [11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 7, 13]);
    assert_eq!(p.frames.len(), 600, "six hundred frames");
    for (i, f) in p.frames.iter().enumerate() {
        assert_eq!(f.poses.len(), 12, "frame {i}: one pose per figure");
        for (fi, pose) in f.poses.iter().enumerate() {
            assert_eq!(
                pose.angles.len(),
                nodes[fi],
                "frame {i} figure {fi}: one angle per node"
            );
        }
    }
    assert!(p.tween_playback, "the fixture plays its baked motion");
}

#[test]
fn long_project_chapters_partition_and_bind_cues() {
    let p = load_fixture();
    let chapters: Vec<(&str, usize, usize)> = p
        .chapters
        .iter()
        .map(|c| (c.title.as_str(), c.start_frame, c.end_frame))
        .collect();
    assert_eq!(
        chapters,
        [
            ("Arrival", 0, 120),
            ("Introductions", 120, 240),
            ("Rehearsals", 240, 360),
            ("The Storm", 360, 480),
            ("Finale", 480, 600),
        ],
        "five chapters partitioning the timeline, half-open"
    );
    assert_eq!(p.chapter_index_at(119), Some(0));
    assert_eq!(p.chapter_index_at(120), Some(1), "frame 120 opens chapter two");
    assert_eq!(p.chapter_index_at(599), Some(4));
    assert_eq!(p.chapter_index_at(600), None, "the range is half-open at the end");
    // Every cue binds by start frame to a chapter; the
    // distribution across chapters is pinned.
    let mut per_chapter = [0usize; 5];
    for cue in &p.subtitles {
        let idx = p
            .chapter_index_at(cue.start_frame)
            .expect("every cue starts inside a chapter");
        per_chapter[idx] += 1;
    }
    assert_eq!(per_chapter, [10, 7, 8, 8, 7]);
}

#[test]
fn long_project_dialogue_load_is_realistic_and_valid() {
    let p = load_fixture();
    assert_eq!(p.subtitles.len(), 40, "forty cues");
    let first = &p.subtitles[0];
    assert_eq!((first.start_frame, first.end_frame), (4, 13));
    assert_eq!(first.text, "Ready?");
    assert_eq!(first.speaker.as_deref(), Some("Ash"));
    let last = &p.subtitles[p.subtitles.len() - 1];
    assert_eq!((last.start_frame, last.end_frame), (576, 591));
    assert_eq!(last.text, "Finale time!");
    let mut prev_end = 0;
    for cue in &p.subtitles {
        SubtitleCue::validate(cue.start_frame, cue.end_frame, &cue.text)
            .unwrap_or_else(|e| panic!("fixture cue must validate: {e}"));
        assert!(
            cue.start_frame >= prev_end,
            "cues are sorted and never overlap"
        );
        prev_end = cue.end_frame;
        let speaker = cue.speaker.as_deref().expect("every cue is credited");
        assert!(
            p.figures.iter().any(|f| f.name == speaker),
            "speaker {speaker} is a figure in the cast"
        );
        let seconds = (cue.end_frame - cue.start_frame) as f32 / p.fps;
        let cps = cue.text.chars().count() as f32 / seconds;
        assert!(cps <= 21.0, "cue at {} runs {cps:.1} CPS", cue.start_frame);
        assert!(cue.end_frame <= p.frames.len(), "cue ends within the timeline");
    }
}

#[test]
fn long_project_camera_track_spans_the_episode() {
    let p = load_fixture();
    let frames: Vec<usize> = p.camera.iter().map(|k| k.frame).collect();
    assert_eq!(
        frames,
        [0, 60, 120, 180, 240, 300, 360, 420, 480, 540, 599],
        "eleven keys, strictly ascending"
    );
    assert_eq!(p.camera[6].easing, CameraEase::Hold, "one held segment");
    // Endpoints are exact (the W2-32 contract); the move pans
    // right and pushes in to 1.3x at the middle key.
    assert_eq!(p.camera_at(0.0), (340.0, 355.0, 1.0));
    assert_eq!(p.camera_at(599.0), (940.0, 355.0, 1.0));
    let (mx, my, mz) = p.camera_at(300.0);
    assert!((mx - 640.501).abs() < 0.01, "mid key x {mx}");
    assert!((my - 347.0).abs() < 0.01, "mid key y {my}");
    assert!((mz - 1.3).abs() < 0.01, "mid key zoom {mz}");
}

#[test]
fn long_project_channels_bends_and_squash_are_live_data() {
    let p = load_fixture();
    assert_eq!(p.channels.len(), 6, "six graph channels");
    assert_eq!(p.channels[0].target, ChannelTarget::Angle(5));
    assert_eq!(p.channels[0].figure, 0);
    assert_eq!(p.channels[1].target, ChannelTarget::RootX);
    assert_eq!(p.channels[2].target, ChannelTarget::Bend(4));
    for ch in &p.channels {
        assert!(ch.figure < p.figures.len());
        let frames: Vec<f32> = ch.keys.iter().map(|k| k.frame).collect();
        let mut sorted = frames.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(frames, sorted, "channel keys stay in ascending frame order");
    }
    // Bends: figure 4 bows through Rehearsals; frame 0 is bare.
    assert!(p.frames[0].bends.is_empty(), "no bends before the beat");
    let bends = p.bends_at(300.0, 4);
    assert_eq!(bends.len(), 11);
    assert!((bends[4] - 0.12).abs() < 0.001, "forearm bend at 300: {}", bends[4]);
    assert!((bends[8] - 0.08).abs() < 0.001, "shin bend at 300: {}", bends[8]);
    // Squash: Gus (figure 6) squashes mid-beat and only there.
    assert_eq!(p.frames[0].poses[6].sx, 1.0, "no squash stored at rest");
    let gus = &p.frames[300].poses[6];
    assert!(gus.sx > 1.1 && gus.sy < 0.85, "squash at 300: {} x {}", gus.sx, gus.sy);
    let poses = p.poses_at(300.0);
    assert!(poses[6].sx > 1.1, "poses_at carries the squash");
}

#[test]
fn long_project_sprites_assets_and_text_really_work() {
    let p = load_fixture();
    assert_eq!(p.assets.len(), 2);
    assert_eq!(p.assets[0].name, "sun.png");
    assert_eq!(p.assets[1].name, "banner.png");
    // The app's own decode path — a stored byte salad would
    // fail here even though serde accepted it.
    let (w, h, _) = media::asset_pixels(&p, 0).expect("the sun asset decodes");
    assert_eq!((w, h), (16, 16));
    let (w, h, _) = media::asset_pixels(&p, 1).expect("the banner asset decodes");
    assert_eq!((w, h), (24, 12));
    assert_eq!(p.sprites.len(), 3);
    assert_eq!(p.sprites[1].asset, 1, "the banner places asset 1");
    let key = InstanceKey::sample(&p.sprites[1].keys, 300.0).expect("banner samples at 300");
    assert_eq!((key.x, key.y), (660.0, 86.0), "the banner drifts mid-episode");
    assert_eq!(p.texts.len(), 1);
    assert_eq!(p.texts[0].content, "THE LONG PLATFORM");
    let title = InstanceKey::sample(&p.texts[0].keys, 100.0).expect("title samples at 100");
    assert_eq!(title.opacity, 1.0, "the title is fully up mid-episode");
    assert_eq!(p.stored_guides.len(), 2, "two stored guides ride the fixture");
}

#[test]
fn long_project_evaluates_finite_and_round_trips() {
    let p = load_fixture();
    for frame_f in [0.0, 150.5, 300.0, 479.5, 599.0] {
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
                assert!(
                    joint[0] >= 0.0 && joint[0] <= 1280.0 && joint[1] >= 0.0 && joint[1] <= 720.0,
                    "figure {} joint {j} on stage at {frame_f}: {joint:?}",
                    fig.name
                );
            }
        }
    }
    // A fixed point of the app's own serialisation, at scale.
    let text = serde_json::to_string_pretty(&p).expect("fixture serialises");
    let again: Project = serde_json::from_str(&text).expect("fixture re-loads");
    assert_eq!(again.frames.len(), 600);
    assert_eq!(again.figures.len(), 12);
    assert_eq!(again.chapters, p.chapters, "chapters survive the round-trip");
    assert_eq!(again.subtitles.len(), 40);
    assert_eq!(again.camera.len(), 11);
    assert_eq!(again.channels.len(), 6, "channels survive the round-trip");
    assert_eq!(again.sprites.len(), 3);
    assert_eq!(again.assets.len(), 2);
    assert_eq!(
        again.frames[300].poses[6].sx, p.frames[300].poses[6].sx,
        "the squash survives the round-trip"
    );
}
