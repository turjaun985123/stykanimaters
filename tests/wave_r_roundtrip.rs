//! Wave R (R19) — round-trip extension: P22's save→load
//! discipline carried onto the persisted state Wave R adds.
//!
//! What is new since P22's all-fields suite
//! (tests/wave_p_roundtrip.rs, untouched by this file):
//!
//! - **Segment-text alpha (R8).** As landed (r8.md's
//!   discrepancy report, reconciled by the integrator —
//!   this file's original draft pinned a stored `alpha`
//!   field on `SegmentMedia::Text` that R8's settlement
//!   deliberately did NOT add): the text label's alpha is
//!   the CARRIER node's `style.opacity` — already stored,
//!   already serde-defaulted — threaded into the evaluated
//!   `media::MediaItem::Text` and on to SVG + raster. That
//!   closes P17's last remainder: `MediaItem::Text` was the
//!   one media kind with no alpha channel, so segment text
//!   drew at full strength on the exporters while the stage
//!   faded it. Pins: a carrier opacity of 0.35 survives
//!   save→load EXACTLY (typed equality + byte identity, the
//!   P22 method), the edge values 0.0 and the default 1.0
//!   ride along on the other carriers (figure node, clip-def
//!   node), a legacy document whose carrier node carries NO
//!   `style` object loads its text as exactly 1.0-opaque
//!   and re-saves stably, and the loaded opacity reaches
//!   the evaluated media stream (`media::media_items`) —
//!   persistence is only worth pinning if the render funnel
//!   sees it.
//! - **Subtitle persistence at the Wave R baseline (R2).**
//!   P22 already round-trips two cues + a non-default style;
//!   this file is the going-forward guard for the whole
//!   subtitle surface: a fuller cue set (multi-line, unicode,
//!   back-to-back adjacency), both style anchors at in-range
//!   scales, and two load-identity pins (below).
//! - **Nothing else in Wave R persists.** R9's JPEG support
//!   keeps the stored-bytes asset model unchanged, R10's
//!   export scale is a render-call parameter, R3/R4/R5/R6
//!   add no model fields — checked against the board
//!   contracts; if a sibling lands persisted state anyway,
//!   it belongs in this file (see r19.md).
//!
//! The R2 settlement pinned here — stated so a future
//! reader can tell whether it still matches r2.md:
//! subtitle validation is an EDIT-TIME and IMPORT-TIME
//! rule. The write paths (core ops, SRT/VTT converters)
//! may refuse zero-length / inverted / blank-text /
//! overlapping cues and may keep stored order as written;
//! the project LOAD path may not silently rewrite what a
//! file contains. That is the only settlement R2's file
//! ownership can produce: `projects::load` is a plain
//! `serde_json::from_str` (src/projects.rs) and lives
//! outside R2's files, and `SubtitleCue`/`SubtitleStyle`
//! carry plain derived (de)serialisation. So a file holding
//! cues the ops would refuse — written by hand, by another
//! build, or before the rules existed — loads VERBATIM and
//! re-saves byte-identically (test 6). If R2 instead lands
//! load-time normalisation, test 6 is the leg to flip, and
//! r19.md flags exactly that for the integrator.
//!
//! Method mirrors P22: serialise with
//! `serde_json::to_string_pretty` — the exact call the save
//! path makes — load, re-serialise, compare parsed forms
//! field by field (diff paths use the JSON shape) AND
//! assert byte equality of the two serialisations. fps is
//! set explicitly (24.0) in every fixture (board contract
//! 6).

use std::path::PathBuf;

use serde_json::Value;

use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::media::{media_items, MediaItem};
use stykanimate_rs::model::{
    ClipDef, ClipLoop, FigureDef, Project, SegmentMedia,
};
use stykanimate_rs::projects;
use stykanimate_rs::subtitles::{SubtitleCue, SubtitlePosition, SubtitleStyle};

/// Per-process, per-test scratch directory under the OS temp
/// dir (P22's idiom, wave-r tag).
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "styk-wave-r-roundtrip-{}-{tag}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Recursive semantic diff over parsed JSON (P22's method):
/// one line per differing leaf, addressed by JSON path.
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

/// Serialise → load → re-serialise; assert the parsed forms
/// are identical (diff names any lost field) and the bytes
/// are identical (a byte-only diff would name a formatting
/// instability instead).
fn assert_round_trip_lossless(project: &Project, what: &str) {
    let text1 = serde_json::to_string_pretty(project).expect("serialise fixture");
    let back: Project = serde_json::from_str(&text1).expect("reload fixture");
    let text2 = serde_json::to_string_pretty(&back).expect("re-serialise loaded fixture");
    let v1: Value = serde_json::from_str(&text1).expect("parse first serialisation");
    let v2: Value = serde_json::from_str(&text2).expect("parse second serialisation");
    let mut diffs = Vec::new();
    diff_values("$", &v1, &v2, &mut diffs);
    assert!(
        diffs.is_empty(),
        "{what} is lossy across save→load:\n{}",
        diffs.join("\n")
    );
    assert_eq!(
        text2, text1,
        "{what}: save→load→re-serialise is not byte-identical"
    );
}

/// The alpha fixture: segment text on three carriers whose
/// NODE STYLE OPACITIES sit at three values — 0.35 on a
/// figure node (the brief's value), 0.0 on the neighbouring
/// figure node (fully transparent edge), the default 1.0
/// left untouched on a movieclip-def node (the second
/// SegmentMedia carrier) — plus an image media on another
/// node, proving the sibling variant is undisturbed. Under
/// R8's settlement the alpha IS the carrier's style
/// opacity; `SegmentMedia::Text` itself stays
/// `{ content, size }`.
fn alpha_fixture() -> Project {
    let mut p = Project::new_demo();
    p.fps = 24.0;

    p.figures[0].def.nodes[4].media = Some(SegmentMedia::Text {
        content: "hand".into(),
        size: 14.0,
    });
    p.figures[0].def.nodes[4].style.opacity = 0.35;
    p.figures[0].def.nodes[5].media = Some(SegmentMedia::Text {
        content: "arm".into(),
        size: 10.0,
    });
    p.figures[0].def.nodes[5].style.opacity = 0.0;
    p.figures[0].def.nodes[2].media = Some(SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false });

    let mut clip_def = FigureDef::humanoid();
    clip_def.nodes[4].media = Some(SegmentMedia::Text {
        content: "clip label".into(),
        size: 12.0,
    });
    let clip_pose = clip_def.rest_pose([480.0, 470.0]);
    p.clips = vec![ClipDef {
        name: "alpha-clip".into(),
        def: clip_def,
        frames: vec![clip_pose],
        loop_mode: ClipLoop::PingPong,
    }];

    p
}

#[test]
fn text_media_alpha_survives_save_load_exactly() {
    let project = alpha_fixture();

    // Non-vacuity: the serialised form visibly carries the
    // fixture's carrier opacities at the brief's value (and
    // at the 0.0 edge) — a fixture that silently stopped
    // exercising alpha fails here, not by passing vacuously.
    // (The 1.0 default is not marker-checked: every untouched
    // node in the demo carries it too.)
    let text1 = serde_json::to_string_pretty(&project).expect("serialise fixture");
    for marker in ["\"opacity\": 0.35", "\"opacity\": 0.0"] {
        assert!(
            text1.contains(marker),
            "serialised fixture is missing the {marker} marker"
        );
    }

    assert_round_trip_lossless(&project, "segment-text alpha fixture");

    // Typed asserts on the reloaded values — byte identity
    // already implies these, but a failure here names the
    // carrier instead of a JSON path alone.
    let back: Project = serde_json::from_str(&text1).expect("reload fixture");
    assert_eq!(
        back.figures[0].def.nodes[4].media,
        Some(SegmentMedia::Text {
            content: "hand".into(),
            size: 14.0,
        }),
        "figure-node text media changed across the round trip"
    );
    assert_eq!(
        back.figures[0].def.nodes[4].style.opacity, 0.35,
        "figure-node carrier opacity 0.35 did not survive exactly"
    );
    assert_eq!(
        back.figures[0].def.nodes[5].media,
        Some(SegmentMedia::Text {
            content: "arm".into(),
            size: 10.0,
        }),
        "figure-node text media (0.0 carrier) changed across the round trip"
    );
    assert_eq!(
        back.figures[0].def.nodes[5].style.opacity, 0.0,
        "figure-node carrier opacity 0.0 (fully transparent) did not survive exactly"
    );
    assert_eq!(
        back.clips[0].def.nodes[4].media,
        Some(SegmentMedia::Text {
            content: "clip label".into(),
            size: 12.0,
        }),
        "clip-def-node text media changed across the round trip"
    );
    assert_eq!(
        back.clips[0].def.nodes[4].style.opacity, 1.0,
        "clip-def-node carrier opacity (default 1.0) did not survive exactly"
    );
    assert_eq!(
        back.figures[0].def.nodes[2].media,
        Some(SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false }),
        "the image media sibling variant changed across the round trip"
    );
}

#[test]
fn text_media_alpha_survives_the_file_path_and_a_fresh_core() {
    let project = alpha_fixture();
    let dir = temp_dir("alpha-file");

    let path = projects::save(&project, &dir).expect("save fixture via projects::save");
    let loaded = projects::load(&path).expect("load fixture via projects::load");
    let mut core = EditorCore::new();
    core.project = loaded;

    let text1 = serde_json::to_string_pretty(&project).expect("serialise fixture");
    let text2 =
        serde_json::to_string_pretty(&core.project).expect("re-serialise the fresh core's project");
    assert_eq!(
        text2, text1,
        "alpha fixture: save→file→fresh core→re-serialise is not byte-identical"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn legacy_project_without_carrier_style_loads_text_as_opaque_and_resaves_stably() {
    // The legacy wire shape under R8's settlement: the
    // text alpha rides the carrier node's `style` object
    // (`NodeDef.style` carries `#[serde(default)]`), so a
    // pre-style document — the carrier node objects with NO
    // `style` key at all — loads with the default style,
    // opacity exactly 1.0, and its text therefore evaluates
    // fully opaque. (A present-but-partial `style` object
    // is NOT a loadable shape in this tree — `NodeStyle`'s
    // fields carry no individual serde defaults — so it is
    // deliberately not pinned; r8.md §5.)
    let project = alpha_fixture();
    let mut doc: Value =
        serde_json::from_str(&serde_json::to_string_pretty(&project).unwrap()).unwrap();
    fn strip_style(node: &mut Value) {
        let obj = node.as_object_mut().expect("a node serialises to an object");
        assert!(
            obj.remove("style").is_some(),
            "fixture carrier node carried no style key to strip \u{2014} the legacy leg is vacuous"
        );
    }
    strip_style(&mut doc["figures"][0]["def"]["nodes"][4]);
    strip_style(&mut doc["figures"][0]["def"]["nodes"][5]);
    strip_style(&mut doc["clips"][0]["def"]["nodes"][4]);
    let legacy_text = serde_json::to_string_pretty(&doc).expect("serialise legacy document");

    // It loads with every carrier at exactly 1.0 and the
    // text media itself untouched …
    let back: Project = serde_json::from_str(&legacy_text).expect("load legacy project");
    for (carrier, node) in [
        ("figure node 4", &back.figures[0].def.nodes[4]),
        ("figure node 5", &back.figures[0].def.nodes[5]),
        ("clip node 4", &back.clips[0].def.nodes[4]),
    ] {
        assert_eq!(
            node.style.opacity, 1.0,
            "legacy {carrier} must load with carrier opacity exactly 1.0 (opaque)"
        );
        assert!(
            matches!(node.media, Some(SegmentMedia::Text { .. })),
            "legacy {carrier} lost its text media across the load: {:?}",
            node.media
        );
    }

    // … and re-saves stably: the first re-save writes the
    // defaulted style explicitly, and from there the
    // document is a fixed point of save→load.
    let resaved = serde_json::to_string_pretty(&back).expect("re-serialise legacy load");
    let reparsed: Value = serde_json::from_str(&resaved).unwrap();
    assert_eq!(
        reparsed["figures"][0]["def"]["nodes"][4]["style"]["opacity"],
        Value::from(1.0),
        "re-saved legacy project must carry the defaulted opacity (1.0) explicitly"
    );
    assert_round_trip_lossless(&back, "legacy-loaded project (carrier style defaulted)");
}

#[test]
fn loaded_text_alpha_reaches_the_evaluated_media_stream() {
    // Persistence is only worth pinning if the render funnel
    // sees the value: after a save→load, evaluating the
    // frame must produce the text item at the stored alpha
    // (R8's threading: SegmentMedia → MediaItem → renderers).
    let project = alpha_fixture();
    let text1 = serde_json::to_string_pretty(&project).expect("serialise fixture");
    let back: Project = serde_json::from_str(&text1).expect("reload fixture");

    let items = media_items(&back, 0.0);
    let hand = items.iter().find_map(|item| match item {
        MediaItem::Text {
            content, alpha, ..
        } if content.as_str() == "hand" => Some(*alpha),
        _ => None,
    });
    assert_eq!(
        hand,
        Some(0.35),
        "the evaluated media stream must carry the loaded text alpha 0.35 \
         (items seen: {items:?})"
    );
}

#[test]
fn subtitle_surface_round_trips_at_the_wave_r_baseline() {
    // The going-forward guard for the whole subtitle
    // persistence surface: a fuller cue set than P22's
    // (multi-line, unicode, back-to-back adjacency where
    // one cue's end is the next cue's start) and both style
    // anchors at in-range, off-default scales.
    let style_cases: [(&str, SubtitleStyle); 2] = [
        (
            "top / small",
            SubtitleStyle {
                position: SubtitlePosition::Top,
                font_scale: 0.75,
                text_color: 0xFFD94D,
                box_color: 0x201008,
            },
        ),
        (
            "bottom / large",
            SubtitleStyle {
                position: SubtitlePosition::Bottom,
                font_scale: 2.5,
                text_color: 0x7BE495,
                box_color: 0x000000,
            },
        ),
    ];
    for (name, style) in style_cases {
        let mut p = Project::new_demo();
        p.fps = 24.0;
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
            SubtitleCue {
                start_frame: 30,
                end_frame: 31,
                text: "x".into(),
                speaker: None,
            },
        ];
        p.subtitle_style = style;

        assert_round_trip_lossless(&p, &format!("subtitle surface ({name})"));

        let text1 = serde_json::to_string_pretty(&p).unwrap();
        let back: Project = serde_json::from_str(&text1).unwrap();
        assert_eq!(
            back.subtitles, p.subtitles,
            "subtitle cues changed across save→load ({name})"
        );
        assert_eq!(
            back.subtitle_style, style,
            "subtitle style changed across save→load ({name})"
        );
    }
}

#[test]
fn state_the_write_paths_refuse_still_loads_verbatim() {
    // Load-identity for state no write path would accept
    // (the R2 settlement pinned by this file — see the
    // module docs): zero-length and inverted cue ranges,
    // blank texts, overlapping cues, a stored order that is
    // not chronological, a cue running past the end of the
    // timeline, and a style scale above the write-path
    // clamp. A file can hold all of these (hand-written,
    // another build, older rules); loading must not
    // silently rewrite any of it. The cues are assigned
    // directly to the field — the ops are deliberately NOT
    // involved, exactly as a foreign file's contents are
    // not.
    let mut p = Project::new_demo();
    p.fps = 24.0;
    p.subtitles = vec![
        SubtitleCue {
            start_frame: 24,
            end_frame: 30,
            text: "stored first, runs late".into(),
            speaker: None,
        },
        SubtitleCue {
            start_frame: 5,
            end_frame: 5,
            text: "zero length".into(),
            speaker: None,
        },
        SubtitleCue {
            start_frame: 2,
            end_frame: 8,
            text: "overlap a".into(),
            speaker: None,
        },
        SubtitleCue {
            start_frame: 9,
            end_frame: 4,
            text: "inverted".into(),
            speaker: None,
        },
        SubtitleCue {
            start_frame: 0,
            end_frame: 3,
            text: String::new(),
            speaker: None,
        },
        SubtitleCue {
            start_frame: 6,
            end_frame: 20,
            text: "overlap b".into(),
            speaker: None,
        },
        SubtitleCue {
            start_frame: 12,
            end_frame: 15,
            text: "   ".into(),
            speaker: None,
        },
        SubtitleCue {
            start_frame: 0,
            end_frame: 999,
            text: "past the end".into(),
            speaker: None,
        },
        SubtitleCue {
            start_frame: 1,
            end_frame: 2,
            text: "stored last, runs early".into(),
            speaker: None,
        },
    ];
    p.subtitle_style = SubtitleStyle {
        position: SubtitlePosition::Top,
        font_scale: 4.0,
        text_color: 0xFFFFFF,
        box_color: 0x000000,
    };
    let stored_cues = p.subtitles.clone();
    let stored_style = p.subtitle_style;

    // Through the real file path — the load path itself is
    // what this leg guards.
    let dir = temp_dir("edge");
    let path = projects::save(&p, &dir).expect("save edge-case fixture");
    let loaded = projects::load(&path).expect("edge-case project must load");
    let mut core = EditorCore::new();
    core.project = loaded;

    assert_eq!(
        core.project.subtitles, stored_cues,
        "load rewrote stored cues the write paths would refuse \
         (order and values must be verbatim)"
    );
    assert_eq!(
        core.project.subtitle_style, stored_style,
        "load rewrote a stored style the write paths would refuse (scale 4.0)"
    );
    // Byte-level: the document that comes back out of the
    // fresh core is the document that went into the file.
    let text_in = serde_json::to_string_pretty(&p).expect("serialise edge-case fixture");
    let text_out =
        serde_json::to_string_pretty(&core.project).expect("re-serialise the fresh core's project");
    assert_eq!(
        text_out, text_in,
        "edge-case subtitle state: save→file→fresh core→re-serialise is not byte-identical"
    );
    assert_round_trip_lossless(&core.project, "edge-case subtitle state after file load");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn legacy_project_without_subtitle_fields_loads_defaults_and_resaves_stably() {
    // A pre-subtitles project file: the `subtitles` and
    // `subtitle_style` keys absent entirely (P23 gave both
    // fields serde defaults — this leg guards that the
    // defaults are the documented ones and that the loaded
    // project re-saves as a fixed point).
    let mut p = Project::new_demo();
    p.fps = 24.0;
    p.subtitles = vec![SubtitleCue {
        start_frame: 0,
        end_frame: 10,
        text: "will be stripped to make the document legacy".into(),
        speaker: None,
    }];
    p.subtitle_style = SubtitleStyle {
        position: SubtitlePosition::Top,
        font_scale: 2.0,
        text_color: 0xFFD94D,
        box_color: 0x102040,
    };

    let mut doc: Value =
        serde_json::from_str(&serde_json::to_string_pretty(&p).unwrap()).unwrap();
    let obj = doc.as_object_mut().expect("project serialises to an object");
    assert!(
        obj.remove("subtitles").is_some(),
        "serialised project carried no subtitles key — the legacy leg is vacuous"
    );
    assert!(
        obj.remove("subtitle_style").is_some(),
        "serialised project carried no subtitle_style key — the legacy leg is vacuous"
    );
    let legacy_text = serde_json::to_string_pretty(&doc).unwrap();

    let back: Project = serde_json::from_str(&legacy_text).expect("load legacy project");
    assert!(
        back.subtitles.is_empty(),
        "legacy project must load with no cues, got {:?}",
        back.subtitles
    );
    assert_eq!(
        back.subtitle_style,
        SubtitleStyle::default(),
        "legacy project must load with the default subtitle style"
    );
    // The documented default, pinned explicitly so a silent
    // default change fails here rather than passing through
    // the self-comparison above.
    assert_eq!(back.subtitle_style.position, SubtitlePosition::Bottom);
    assert_eq!(back.subtitle_style.font_scale, 1.0);
    assert_eq!(back.subtitle_style.text_color, 0xFF_FFFF);
    assert_eq!(back.subtitle_style.box_color, 0x00_0000);

    assert_round_trip_lossless(&back, "legacy-loaded project (subtitle defaults)");
}
