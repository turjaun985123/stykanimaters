//! Wave P (P22) — save→load round-trip hardening pins.
//!
//! The claim under test: a project written by the app's own
//! save path comes back with EVERY field intact. The fixture
//! is the fullest-feature project the public API allows —
//! figures with styled/media-carrying nodes, per-frame bend
//! rows (full, sparse and empty), FX clips (one hidden, one
//! with a gradient end colour), particle events (anchored and
//! free), audio clips (gain + mute), an image asset table,
//! sprite instances with fractional-frame keys (plus a keyless
//! sprite), a text instance, drawing strokes (pressure widths,
//! fill, spray mark, scatter), camera keys (one rotated), graph
//! channels (Hermite/Linear/Step), a movieclip + a figure that
//! references it, weather, subtitle cues + a fully non-default
//! subtitle style (P23's fields, board contract 1), and every
//! project-level scalar set off its default. fps is set
//! explicitly (24.0).
//!
//! Method: serialise with `serde_json::to_string_pretty` —
//! the exact call the save path makes (`projects::save`,
//! projects.rs) — load into a fresh core, re-serialise, and
//! compare the PARSED forms field by field. The writer is
//! deterministic for a given value (struct field order is
//! fixed by declaration; floats print in serde_json's
//! shortest round-trip form, so an f32 survives
//! print→parse→print unchanged), so the suite ALSO asserts
//! byte equality of the two serialisations: a semantic diff
//! names the lost field; a byte-only diff would name a
//! formatting instability instead. Diff paths in failure
//! messages use the JSON shape (`frames[1].bends[0][3]`), so
//! a CI failure points straight at the lossy field.
//!
//! Per-subsystem fixtures (third test) isolate each feature
//! family on its own, so a loss is attributable to exactly
//! one subsystem without bisecting the big fixture.
//!
//! Scope note: P23's Project fields (`subtitles`,
//! `subtitle_style`, `format_version`) ARE in this tree
//! (src/model.rs) and the fixture covers them — cues, a
//! non-default style, and the constructor-stamped version.
//! The suite compiles once P23's src/subtitles.rs and its
//! lib.rs registration land (integrator sequencing, the
//! same class as P16's IA-1).

use std::path::PathBuf;

use serde_json::Value;

use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::graph::{Channel, ChannelTarget, Interp, Key};
use stykanimate_rs::model::{
    AudioClip, CapKind, ClipDef, ClipLoop, Easing, FigureDef, FxClipEntry, ImageAsset,
    InstanceKey, NodeStyle, ParticleEvent, ParticleKind, Project, SegmentMedia, Shape,
    SprayMark, SpriteInstance, Stroke, TextInstance,
};
use stykanimate_rs::projects;
use stykanimate_rs::subtitles::{SubtitleCue, SubtitlePosition, SubtitleStyle};
use stykanimate_rs::vfx::{ClipKind, ParticleClip, Weather, WeatherKind};

/// Per-process, per-test scratch directory under the OS temp
/// dir (the export_determinism.rs idiom).
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "styk-wave-p-roundtrip-{}-{tag}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// The fullest-feature fixture the public API allows.
fn full_fixture() -> Project {
    let mut p = Project::new_demo();

    // -- Project scalars, all off their defaults. ----------
    p.name = "Wave P Round Trip".into();
    p.fps = 24.0;
    p.stage = [800.0, 600.0];
    p.background = 0x1A2B3C;
    p.tween_playback = true;
    p.easing = Easing::EaseOut;
    p.camera_follow = Some(1);
    p.head_ring_scale = 1.6;

    // -- Assets (sprites reference these by index). --------
    p.assets = vec![
        ImageAsset {
            name: "dot.png".into(),
            png: vec![
                0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 1, 2, 250, 251, 252, 253,
                254, 255,
            ],
        },
        ImageAsset {
            name: "empty.bin".into(),
            png: Vec::new(),
        },
    ];

    // -- Figure 0: the demo figure, restyled deep. ---------
    p.figures[0].color = 0x3366CC;
    p.figures[0].opacity = 0.85;
    p.figures[0].locked = true;
    // Head node: full non-default NodeStyle.
    p.figures[0].def.nodes[2].style = NodeStyle {
        taper: 0.4,
        cap: CapKind::Square,
        color2: Some(0x00FF00),
        opacity: 0.5,
    };
    // Chest node: per-segment colour override.
    p.figures[0].def.nodes[1].color = Some(0xFF0000);
    // Hand L: a text label riding the segment.
    p.figures[0].def.nodes[4].media = Some(SegmentMedia::Text {
        content: "hand".into(),
        size: 14.0,
    });
    // Upper arm R: a circle node with an image on it.
    p.figures[0].def.nodes[5].shape = Shape::Circle;
    p.figures[0].def.nodes[5].media = Some(SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false, opacity: 1.0 });

    // -- A movieclip, and figure 1 referencing it. ---------
    let clip_def = FigureDef::humanoid();
    let mut clip_pose_a = clip_def.rest_pose([480.0, 470.0]);
    clip_pose_a.angles[5] = -90.0;
    let mut clip_pose_b = clip_def.rest_pose([490.0, 465.0]);
    clip_pose_b.angles[5] = -45.0;
    clip_pose_b.angles[6] = 10.0;
    p.clips = vec![ClipDef {
        name: "wave-clip".into(),
        def: clip_def,
        frames: vec![clip_pose_a, clip_pose_b],
        loop_mode: ClipLoop::PingPong,
    }];

    let mut def2 = FigureDef::humanoid();
    def2.nodes[1].name = "partner-chest".into();
    def2.nodes[1].shape = Shape::Polygon(5);
    // Node-scalar depth: non-default thickness, bend side
    // and IK-end flag ride along in the def.
    def2.nodes[3].thickness = 7.5;
    def2.nodes[3].bend = -1.0;
    def2.nodes[4].ik_end = true;
    for (i, frame) in p.frames.iter_mut().enumerate() {
        frame
            .poses
            .push(def2.rest_pose([300.0 + i as f32 * 4.0, 410.0]));
    }
    p.figures.push(stykanimate_rs::model::FigureInstance {
        name: "Partner".into(),
        color: 0x22AA44,
        def: def2,
        hidden: true,
        clip: Some(0),
        states: Vec::new(),
        locked: false,
        opacity: 1.0,
    });

    // -- Frames: bends (full / both-figures / sparse), -----
    // -- per-frame duration + easing overrides. ------------
    p.frames[0].bends = vec![vec![
        0.0, 0.25, -0.5, 0.0, 0.125, 0.0, 0.0, -0.75, 0.0, 0.0, 0.3,
    ]];
    p.frames[1].duration = 2.0;
    p.frames[1].easing = Some(Easing::Smooth);
    p.frames[1].bends = vec![
        vec![0.1, 0.0, -0.1, 0.2, 0.0, 0.0, 0.3, 0.0, 0.0, -0.2, 0.0],
        vec![-0.2, 0.15, 0.0],
    ];
    p.frames[2].duration = 0.5;
    p.frames[2].easing = Some(Easing::EaseIn);
    // Sparse: figure 0's row present but empty, figure 1's short.
    p.frames[2].bends = vec![Vec::new(), vec![0.1, 0.2]];
    // Frames 3 and 4 keep the demo's empty bends — the empty
    // case is part of the pinned set.

    // -- Camera: two keys, the second rotated. -------------
    p.camera = vec![
        stykanimate_rs::model::CameraKey {
            frame: 0,
            x: 400.0,
            y: 300.0,
            zoom: 1.0,
            angle: 0.0,
            easing: stykanimate_rs::model::CameraEase::Linear,
        },
        stykanimate_rs::model::CameraKey {
            frame: 3,
            x: 420.5,
            y: 280.25,
            zoom: 1.75,
            angle: -12.5,
            easing: stykanimate_rs::model::CameraEase::Linear,
        },
    ];

    // -- Particle events: one anchored, one free. ----------
    p.particles = vec![
        ParticleEvent {
            frame: 2,
            figure: Some(0),
            node: Some(6),
            x: 10.5,
            y: -4.25,
            kind: ParticleKind::Burst,
            color: 0xFFAA00,
            seed: 12345,
        },
        ParticleEvent {
            frame: 4,
            figure: None,
            node: None,
            x: 640.0,
            y: 120.0,
            kind: ParticleKind::Smoke,
            color: 0x8899AA,
            seed: 777,
        },
    ];

    // -- FX clips: gradient + hidden, and a plain one. -----
    p.fx_clips = vec![
        FxClipEntry {
            clip: ParticleClip {
                start: 1.0,
                duration: 3.5,
                kind: ClipKind::Beam,
                color: 0xFFCC33,
                color2: Some(0x0033FF),
                seed: 99,
                angle: 45.0,
                length: 220.0,
            },
            figure: 0,
            node: 2,
            hidden: true,
        },
        FxClipEntry {
            clip: ParticleClip {
                start: 0.0,
                duration: 2.0,
                kind: ClipKind::Confetti,
                color: 0x00CC66,
                color2: None,
                seed: 5,
                angle: 0.0,
                length: 0.0,
            },
            figure: 1,
            node: 0,
            hidden: false,
        },
    ];

    // -- Strokes: pressure+fill+scatter, spray, minimal. ---
    p.strokes = vec![
        Stroke {
            frame: 1,
            points: vec![[10.0, 20.0], [30.5, 40.25], [50.0, 60.0]],
            color: 0x102030,
            width: 4.5,
            widths: vec![2.0, 4.5, 3.25],
            fill: Some(0x445566),
            spray: None,
            opacity: 0.7,
            scatter: 3.5,
        },
        Stroke {
            frame: 2,
            points: vec![[100.0, 100.0], [200.0, 150.0]],
            color: 0xFF00FF,
            width: 8.0,
            widths: Vec::new(),
            fill: None,
            spray: Some(SprayMark {
                seed: 42,
                radius: 18.0,
                opacity: 0.6,
            }),
            opacity: 0.9,
            scatter: 0.0,
        },
        Stroke {
            frame: 0,
            points: vec![[5.0, 5.0]],
            color: 0,
            width: 1.0,
            widths: Vec::new(),
            fill: None,
            spray: None,
            opacity: 1.0,
            scatter: 0.0,
        },
    ];

    // -- Audio: gain + mute exercised both ways. -----------
    p.audio = vec![
        AudioClip {
            path: "audio/theme song.wav".into(),
            start_frame: 0,
            gain: 0.5,
            muted: false,
        },
        AudioClip {
            path: "audio/ünïcode-♪.ogg".into(),
            start_frame: 7,
            gain: 1.75,
            muted: true,
        },
    ];

    // -- Graph channels: Hermite tangents, Linear, Step, ---
    // -- and a length channel on the second figure. --------
    p.channels = vec![
        Channel {
            figure: 0,
            target: ChannelTarget::RootX,
            keys: vec![
                Key {
                    frame: 0.0,
                    value: 480.0,
                    in_tan: 0.0,
                    out_tan: 12.5,
                    interp: Interp::Hermite,
                },
                Key {
                    frame: 2.0,
                    value: 505.25,
                    in_tan: -3.75,
                    out_tan: 0.0,
                    interp: Interp::Hermite,
                },
            ],
        },
        Channel {
            figure: 0,
            target: ChannelTarget::Angle(5),
            keys: vec![
                Key {
                    frame: 0.0,
                    value: -118.0,
                    in_tan: 0.0,
                    out_tan: 0.0,
                    interp: Interp::Linear,
                },
                Key {
                    frame: 3.0,
                    value: -60.0,
                    in_tan: 0.0,
                    out_tan: 0.0,
                    interp: Interp::Step,
                },
            ],
        },
        Channel {
            figure: 1,
            target: ChannelTarget::Length(7),
            keys: vec![Key::flat(1.0, 44.0), Key::flat(3.0, 51.5)],
        },
    ];

    // -- Sprites: fractional-frame keys, and a keyless one. -
    p.sprites = vec![
        SpriteInstance {
            asset: 0,
            keys: vec![
                InstanceKey {
                    frame: 0.0,
                    x: 100.0,
                    y: 120.0,
                    scale: 1.0,
                    angle: 0.0,
                    opacity: 1.0,
                    flip_x: false,
                    flip_y: false,
                    scale_y: 0.0,
                    pivot_x: 0.5,
                    pivot_y: 0.5,
                },
                InstanceKey {
                    frame: 2.5,
                    x: 160.5,
                    y: 90.25,
                    scale: 1.5,
                    angle: 30.0,
                    opacity: 1.0,
                    flip_x: false,
                    flip_y: false,
                    scale_y: 0.0,
                    pivot_x: 0.5,
                    pivot_y: 0.5,
                },
            ],
        },
        SpriteInstance {
            asset: 1,
            keys: Vec::new(),
        },
    ];

    // -- Text instance with a unicode label. ---------------
    p.texts = vec![TextInstance {
        content: "Héllo — wörld ✓".into(),
        size: 28.5,
        color: 0xFFEEDD,
        keys: vec![InstanceKey {
            frame: 1.0,
            x: 50.0,
            y: 60.0,
            scale: 1.25,
            angle: -5.0,
            opacity: 1.0,
            flip_x: false,
            flip_y: false,
            scale_y: 0.0,
            pivot_x: 0.5,
            pivot_y: 0.5,
        }],
    }];

    // -- Weather. -------------------------------------------
    p.weather = Some(Weather {
        kind: WeatherKind::Snow,
        amount: 55.5,
        wind: -12.25,
    });

    // -- Subtitles: two cues (one multi-line, one unicode) --
    // -- and a fully non-default style (P23, contract 1). ---
    p.subtitles = vec![
        SubtitleCue {
            start_frame: 0,
            end_frame: 12,
            text: "First line\nsecond line".into(),
            speaker: None,
        },
        SubtitleCue {
            start_frame: 12,
            end_frame: 30,
            text: "Ünïcode cue — ♪".into(),
            speaker: None,
        },
    ];
    p.subtitle_style = SubtitleStyle {
        position: SubtitlePosition::Top,
        font_scale: 1.5,
        text_color: 0xFFE680,
        box_color: 0x102040,
    };

    p
}

/// The fixture really carries every feature it claims to —
/// a fixture regression (an emptied vec, a dropped push)
/// must fail HERE, not silently weaken the round-trip pins.
fn assert_fixture_is_full(p: &Project) {
    assert_eq!(p.figures.len(), 2, "fixture: two figures");
    assert_eq!(p.frames.len(), 5, "fixture: five frames");
    assert_eq!(p.clips.len(), 1, "fixture: one movieclip");
    assert_eq!(p.assets.len(), 2, "fixture: two assets");
    assert_eq!(p.camera.len(), 2, "fixture: two camera keys");
    assert_eq!(p.particles.len(), 2, "fixture: two particle events");
    assert_eq!(p.fx_clips.len(), 2, "fixture: two FX clips");
    assert_eq!(p.strokes.len(), 3, "fixture: three strokes");
    assert_eq!(p.audio.len(), 2, "fixture: two audio clips");
    assert_eq!(p.channels.len(), 3, "fixture: three channels");
    assert_eq!(p.sprites.len(), 2, "fixture: two sprites");
    assert_eq!(p.texts.len(), 1, "fixture: one text instance");
    assert!(p.weather.is_some(), "fixture: weather set");
    assert_eq!(p.subtitles.len(), 2, "fixture: two subtitle cues");
    assert_eq!(
        p.format_version, 1,
        "fixture: format_version stamped at the current version"
    );
    assert_eq!(
        p.subtitle_style.font_scale, 1.5,
        "fixture: subtitle style is non-default (scale)"
    );
    assert_eq!(
        p.subtitle_style.text_color, 0xFFE680,
        "fixture: subtitle style is non-default (text colour)"
    );
    assert!(
        p.frames.iter().any(|f| !f.bends.is_empty()),
        "fixture: at least one frame carries bend rows"
    );
    assert_eq!(
        p.figures[1].clip,
        Some(0),
        "fixture: figure 1 references the movieclip"
    );
}

/// Recursive semantic diff over parsed JSON. Appends one
/// line per differing leaf, addressed by JSON path, capped
/// so a catastrophic diff stays readable.
fn diff_values(path: &str, a: &Value, b: &Value, out: &mut Vec<String>) {
    if out.len() >= 40 {
        return;
    }
    match (a, b) {
        (Value::Object(ma), Value::Object(mb)) => {
            for (k, va) in ma {
                let p = format!("{path}.{k}");
                match mb.get(k) {
                    Some(vb) => diff_values(&p, va, vb, out),
                    None => out.push(format!("{p}: present before load, missing after")),
                }
            }
            for k in mb.keys() {
                if !ma.contains_key(k) {
                    out.push(format!("{path}.{k}: absent before load, present after"));
                }
            }
        }
        (Value::Array(aa), Value::Array(ab)) => {
            if aa.len() != ab.len() {
                out.push(format!(
                    "{path}: array length {} before load, {} after",
                    aa.len(),
                    ab.len()
                ));
            }
            for (i, (va, vb)) in aa.iter().zip(ab.iter()).enumerate() {
                diff_values(&format!("{path}[{i}]"), va, vb, out);
            }
        }
        _ => {
            if a != b {
                out.push(format!("{path}: {a} before load, {b} after"));
            }
        }
    }
}

/// Serialise → parse → load → re-serialise → parse, and
/// return the semantic diffs between the two parsed forms.
fn round_trip_diffs(project: &Project) -> Vec<String> {
    let text1 = serde_json::to_string_pretty(project).expect("serialise fixture");
    let back: Project = serde_json::from_str(&text1).expect("reload fixture");
    let text2 = serde_json::to_string_pretty(&back).expect("re-serialise loaded fixture");
    let v1: Value = serde_json::from_str(&text1).expect("parse first serialisation");
    let v2: Value = serde_json::from_str(&text2).expect("parse second serialisation");
    let mut diffs = Vec::new();
    diff_values("$", &v1, &v2, &mut diffs);
    diffs
}

#[test]
fn full_fixture_round_trip_is_lossless_per_section() {
    let project = full_fixture();
    assert_fixture_is_full(&project);

    let text1 = serde_json::to_string_pretty(&project).expect("serialise fixture");
    // Non-vacuity markers: the serialised form visibly
    // carries the features (guards against a fixture that
    // silently stopped exercising them).
    for marker in [
        "\"spray\"",
        "\"PingPong\"",
        "\"bends\"",
        "\"channels\"",
        "\"weather\"",
        "\"subtitles\"",
        "\"subtitle_style\"",
        "\"format_version\"",
        "\"Ünïcode cue — ♪\"",
        "\"audio/ünïcode-♪.ogg\"",
    ] {
        assert!(
            text1.contains(marker),
            "serialised fixture is missing the {marker} marker"
        );
    }

    let back: Project = serde_json::from_str(&text1).expect("reload fixture");
    let text2 = serde_json::to_string_pretty(&back).expect("re-serialise loaded fixture");
    let v1: Value = serde_json::from_str(&text1).unwrap();
    let v2: Value = serde_json::from_str(&text2).unwrap();
    let (o1, o2) = (
        v1.as_object().expect("project serialises to an object"),
        v2.as_object().expect("reloaded project serialises to an object"),
    );

    // Per-section asserts: a failure names the section, and
    // the diff lines name the exact fields inside it.
    let mut sections: Vec<&String> = o1.keys().collect();
    for k in o2.keys() {
        if !o1.contains_key(k) {
            sections.push(k);
        }
    }
    sections.sort();
    for section in sections {
        let mut diffs = Vec::new();
        match (o1.get(section), o2.get(section)) {
            (Some(a), Some(b)) => diff_values(&format!("$.{section}"), a, b, &mut diffs),
            (Some(_), None) => diffs.push(format!("$.{section}: missing after load")),
            (None, Some(_)) => diffs.push(format!("$.{section}: appeared after load")),
            (None, None) => unreachable!(),
        }
        assert!(
            diffs.is_empty(),
            "section {section} is lossy across save→load:\n{}",
            diffs.join("\n")
        );
    }
}

#[test]
fn full_fixture_round_trip_through_projects_file_and_fresh_core() {
    let project = full_fixture();
    assert_fixture_is_full(&project);
    let dir = temp_dir("file");

    // The app's own save path writes the file.
    let path = projects::save(&project, &dir).expect("save fixture via projects::save");
    let written = std::fs::read_to_string(&path).expect("read the saved file");

    // The save path's bytes are exactly the pretty
    // serialisation — the writer is byte-stable, so the
    // round trip below may assert byte equality, not just
    // semantic equality.
    let text1 = serde_json::to_string_pretty(&project).expect("serialise fixture");
    assert_eq!(
        written, text1,
        "projects::save wrote something other than the plain pretty serialisation"
    );

    // Load the file and put it in a FRESH core — the
    // editor's own open shape.
    let loaded = projects::load(&path).expect("load fixture via projects::load");
    let mut core = EditorCore::new();
    core.project = loaded;

    let text2 =
        serde_json::to_string_pretty(&core.project).expect("re-serialise the fresh core's project");
    assert_eq!(
        text2, text1,
        "save→file→fresh core→re-serialise is not byte-identical"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn subsystem_fixtures_round_trip_losslessly() {
    // Each case dresses a fresh demo project with ONE
    // subsystem from the full fixture, so a loss is
    // attributable to that subsystem alone. The mutators
    // mirror full_fixture's values for their slice.
    let full = full_fixture();
    let cases: Vec<(&str, Box<dyn Fn(&mut Project)>)> = vec![
        (
            "scalars",
            Box::new(|p: &mut Project| {
                p.name = "Subsystem Scalars".into();
                p.fps = 24.0;
                p.stage = [800.0, 600.0];
                p.background = 0x1A2B3C;
                p.tween_playback = true;
                p.easing = Easing::EaseOut;
                p.camera_follow = Some(0);
                p.head_ring_scale = 1.6;
            }),
        ),
        (
            "figure-def-depth",
            Box::new(|p: &mut Project| {
                p.figures[0].def.nodes[2].style = NodeStyle {
                    taper: 0.4,
                    cap: CapKind::Square,
                    color2: Some(0x00FF00),
                    opacity: 0.5,
                };
                p.figures[0].def.nodes[1].color = Some(0xFF0000);
                p.figures[0].def.nodes[4].media = Some(SegmentMedia::Text {
                    content: "hand".into(),
                    size: 14.0,
                });
                p.figures[0].def.nodes[5].shape = Shape::Circle;
            }),
        ),
        (
            "bends-and-frame-overrides",
            Box::new(|p: &mut Project| {
                p.frames[0].bends = vec![vec![
                    0.0, 0.25, -0.5, 0.0, 0.125, 0.0, 0.0, -0.75, 0.0, 0.0, 0.3,
                ]];
                p.frames[1].duration = 2.0;
                p.frames[1].easing = Some(Easing::Smooth);
                p.frames[2].duration = 0.5;
                p.frames[2].easing = Some(Easing::EaseIn);
                p.frames[2].bends = vec![Vec::new(), vec![0.1, 0.2]];
            }),
        ),
        (
            "camera",
            Box::new(|p: &mut Project| {
                p.camera = vec![
                    stykanimate_rs::model::CameraKey {
                        frame: 0,
                        x: 400.0,
                        y: 300.0,
                        zoom: 1.0,
                        angle: 0.0,
                        easing: stykanimate_rs::model::CameraEase::Linear,
                    },
                    stykanimate_rs::model::CameraKey {
                        frame: 3,
                        x: 420.5,
                        y: 280.25,
                        zoom: 1.75,
                        angle: -12.5,
                        easing: stykanimate_rs::model::CameraEase::Linear,
                    },
                ];
            }),
        ),
        (
            "particles",
            Box::new(|p: &mut Project| {
                p.particles = vec![
                    ParticleEvent {
                        frame: 2,
                        figure: Some(0),
                        node: Some(6),
                        x: 10.5,
                        y: -4.25,
                        kind: ParticleKind::Burst,
                        color: 0xFFAA00,
                        seed: 12345,
                    },
                    ParticleEvent {
                        frame: 4,
                        figure: None,
                        node: None,
                        x: 640.0,
                        y: 120.0,
                        kind: ParticleKind::Smoke,
                        color: 0x8899AA,
                        seed: 777,
                    },
                ];
            }),
        ),
        (
            "fx-clips",
            Box::new(|p: &mut Project| {
                p.fx_clips = vec![
                    FxClipEntry {
                        clip: ParticleClip {
                            start: 1.0,
                            duration: 3.5,
                            kind: ClipKind::Beam,
                            color: 0xFFCC33,
                            color2: Some(0x0033FF),
                            seed: 99,
                            angle: 45.0,
                            length: 220.0,
                        },
                        figure: 0,
                        node: 2,
                        hidden: true,
                    },
                    FxClipEntry {
                        clip: ParticleClip {
                            start: 0.0,
                            duration: 2.0,
                            kind: ClipKind::Confetti,
                            color: 0x00CC66,
                            color2: None,
                            seed: 5,
                            angle: 0.0,
                            length: 0.0,
                        },
                        figure: 0,
                        node: 0,
                        hidden: false,
                    },
                ];
            }),
        ),
        (
            "strokes",
            Box::new(|p: &mut Project| {
                p.strokes = vec![
                    Stroke {
                        frame: 1,
                        points: vec![[10.0, 20.0], [30.5, 40.25], [50.0, 60.0]],
                        color: 0x102030,
                        width: 4.5,
                        widths: vec![2.0, 4.5, 3.25],
                        fill: Some(0x445566),
                        spray: None,
                        opacity: 0.7,
                        scatter: 3.5,
                    },
                    Stroke {
                        frame: 2,
                        points: vec![[100.0, 100.0], [200.0, 150.0]],
                        color: 0xFF00FF,
                        width: 8.0,
                        widths: Vec::new(),
                        fill: None,
                        spray: Some(SprayMark {
                            seed: 42,
                            radius: 18.0,
                            opacity: 0.6,
                        }),
                        opacity: 0.9,
                        scatter: 0.0,
                    },
                ];
            }),
        ),
        (
            "audio",
            Box::new(|p: &mut Project| {
                p.audio = vec![
                    AudioClip {
                        path: "audio/theme song.wav".into(),
                        start_frame: 0,
                        gain: 0.5,
                        muted: false,
                    },
                    AudioClip {
                        path: "audio/ünïcode-♪.ogg".into(),
                        start_frame: 7,
                        gain: 1.75,
                        muted: true,
                    },
                ];
            }),
        ),
        (
            "assets-and-sprites-and-texts",
            Box::new(|p: &mut Project| {
                p.assets = vec![
                    ImageAsset {
                        name: "dot.png".into(),
                        png: vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 255],
                    },
                    ImageAsset {
                        name: "empty.bin".into(),
                        png: Vec::new(),
                    },
                ];
                p.sprites = vec![
                    SpriteInstance {
                        asset: 0,
                        keys: vec![
                            InstanceKey {
                                frame: 0.0,
                                x: 100.0,
                                y: 120.0,
                                scale: 1.0,
                                angle: 0.0,
                                opacity: 1.0,
                                flip_x: false,
                                flip_y: false,
                                scale_y: 0.0,
                                pivot_x: 0.5,
                                pivot_y: 0.5,
                            },
                            InstanceKey {
                                frame: 2.5,
                                x: 160.5,
                                y: 90.25,
                                scale: 1.5,
                                angle: 30.0,
                                opacity: 1.0,
                                flip_x: false,
                                flip_y: false,
                                scale_y: 0.0,
                                pivot_x: 0.5,
                                pivot_y: 0.5,
                            },
                        ],
                    },
                    SpriteInstance {
                        asset: 1,
                        keys: Vec::new(),
                    },
                ];
                p.texts = vec![TextInstance {
                    content: "Héllo — wörld ✓".into(),
                    size: 28.5,
                    color: 0xFFEEDD,
                    keys: vec![InstanceKey {
                        frame: 1.0,
                        x: 50.0,
                        y: 60.0,
                        scale: 1.25,
                        angle: -5.0,
                        opacity: 1.0,
                        flip_x: false,
                        flip_y: false,
                        scale_y: 0.0,
                        pivot_x: 0.5,
                        pivot_y: 0.5,
                    }],
                }];
            }),
        ),
        (
            "channels",
            Box::new(|p: &mut Project| {
                p.channels = vec![
                    Channel {
                        figure: 0,
                        target: ChannelTarget::RootX,
                        keys: vec![
                            Key {
                                frame: 0.0,
                                value: 480.0,
                                in_tan: 0.0,
                                out_tan: 12.5,
                                interp: Interp::Hermite,
                            },
                            Key {
                                frame: 2.0,
                                value: 505.25,
                                in_tan: -3.75,
                                out_tan: 0.0,
                                interp: Interp::Hermite,
                            },
                        ],
                    },
                    Channel {
                        figure: 0,
                        target: ChannelTarget::Angle(5),
                        keys: vec![
                            Key {
                                frame: 0.0,
                                value: -118.0,
                                in_tan: 0.0,
                                out_tan: 0.0,
                                interp: Interp::Linear,
                            },
                            Key {
                                frame: 3.0,
                                value: -60.0,
                                in_tan: 0.0,
                                out_tan: 0.0,
                                interp: Interp::Step,
                            },
                        ],
                    },
                    Channel {
                        figure: 0,
                        target: ChannelTarget::Length(7),
                        keys: vec![Key::flat(1.0, 44.0), Key::flat(3.0, 51.5)],
                    },
                ];
            }),
        ),
        (
            "clips-and-clip-reference",
            Box::new(move |p: &mut Project| {
                p.clips = full.clips.clone();
                p.figures[0].clip = Some(0);
            }),
        ),
        (
            "weather",
            Box::new(|p: &mut Project| {
                p.weather = Some(Weather {
                    kind: WeatherKind::Snow,
                    amount: 55.5,
                    wind: -12.25,
                });
            }),
        ),
        (
            "subtitles-and-style",
            Box::new(|p: &mut Project| {
                p.subtitles = vec![
                    SubtitleCue {
                        start_frame: 0,
                        end_frame: 12,
                        text: "First line\nsecond line".into(),
                        speaker: None,
                    },
                    SubtitleCue {
                        start_frame: 12,
                        end_frame: 30,
                        text: "Ünïcode cue — ♪".into(),
                        speaker: None,
                    },
                ];
                p.subtitle_style = SubtitleStyle {
                    position: SubtitlePosition::Top,
                    font_scale: 1.5,
                    text_color: 0xFFE680,
                    box_color: 0x102040,
                };
            }),
        ),
    ];

    for (name, mutate) in &cases {
        let mut project = Project::new_demo();
        mutate(&mut project);
        let diffs = round_trip_diffs(&project);
        assert!(
            diffs.is_empty(),
            "subsystem {name} is lossy across save→load:\n{}",
            diffs.join("\n")
        );
    }
}
