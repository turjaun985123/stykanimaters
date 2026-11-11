//! Wave U (U15) — cross-feature e2e: Wave T's seams,
//! proven end to end.
//!
//! Wave T landed three things whose pieces were pinned
//! separately but never together:
//!
//! * T6 put the Home missing-audio marker on BOTH Home
//!   lines — the light-read line (`metadata_for`, over
//!   `read_metadata_counts`) and the scan-fallback line
//!   (the projects scan's full parse) — arguing the two
//!   can never disagree because both run the one shared
//!   classification (`audio::missing_audio_clips`). The
//!   scan side is pinned in `projects.rs`'s own tests and
//!   the light-read side in `home_panel.rs`'s, but no leg
//!   ran the two over the SAME on-disk fixture and
//!   demanded exact agreement. Leg 1 does.
//! * T7 gave the JPEG sequence a scaled entry
//!   (`export_jpeg_sequence_scaled`) after S5/S11 had
//!   proven scaled burn-in on PNG frames and T19 on
//!   GIF/APNG/sheet bytes. T7's own pins check JPEG
//!   dimensions and 1.0 identity in-crate; no leg decoded
//!   a scaled JPEG and measured the burn-in band in it.
//!   Leg 2 does, and leg 4 repeats the 1.0 identity on a
//!   subtitled fixture (T19 did GIF/APNG/sheet; the JPEG
//!   sequence is the format T7 added).
//! * T8 made `export_bundle` validate `scale` BEFORE
//!   anything is written, after finding the Wave R
//!   routing left partial bundles behind successful
//!   calls. T8 pinned it in-crate; leg 3 drives the
//!   refusal through the real registry
//!   (`mcp::all_tools()`) and asserts the target
//!   directory is never created — the atomicity claim at
//!   the surface a caller actually touches.
//!
//! Reachability note for leg 1, stated plainly:
//! `read_metadata_counts` lives in the binary's
//! `rune_sec::home_panel` and cannot be linked from an
//! integration test. Its computation is, however, a
//! short exact algorithm over library types (its own
//! doc comment states it): the file's raw JSON, the
//! `figures`/`frames` array lengths (whose absence
//! fails the whole read), the `audio` array — absent
//! key = empty list — deserialised into
//! `model::AudioClip`s and classified by
//! `audio::missing_audio_clips`. `light_read_counts`
//! below performs exactly that computation, step for
//! step; the parity claim is therefore between the
//! scan's full parse and the light read's own
//! algorithm, run over the same bytes on disk.
//!
//! Fixtures: the subtitled stage is T19's bare-stage
//! shape (480×400, background 0x333333, no figures,
//! identical empty authored frames, fps 24.0, default
//! subtitle style). JPEG is lossy (quality 90), so its
//! frames are measured with the near-white face
//! predicate and S11's geometry ranges — span 25..=33
//! rows and centre offset 50..=62 px from the bottom
//! edge at 2.0 (shaped cap-ink ~0.7 × house font 20 × 2,
//! centre 28 × 2) —
//! never byte equality. The only byte-equality claims
//! are leg 4's, where the 1.0 entries delegate to the
//! unscaled entry, making both sides the same encoder
//! path by construction.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use stykanimate_rs::audio::missing_audio_clips;
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::export::{
    export_jpeg_sequence, export_jpeg_sequence_range_scaled, export_jpeg_sequence_scaled,
};
use stykanimate_rs::mcp::all_tools;
use stykanimate_rs::model::{AudioClip, Frame, Project};
use stykanimate_rs::projects::{save, scan, ProjectInfo};
use stykanimate_rs::subtitles::{SubtitleCue, SubtitleStyle};

// ---------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------

fn cue(start_frame: usize, end_frame: usize, text: &str) -> SubtitleCue {
    SubtitleCue {
        start_frame,
        end_frame,
        text: text.to_string(),
        speaker: None,
    }
}

fn credited(start_frame: usize, end_frame: usize, text: &str, speaker: &str) -> SubtitleCue {
    SubtitleCue {
        start_frame,
        end_frame,
        text: text.to_string(),
        speaker: Some(speaker.to_string()),
    }
}

/// An empty stage (background 0x333333, no figures) with
/// `frames` identical empty authored frames, carrying
/// exactly `cues` under the default style, at 24 fps
/// (wave_s_e2e / T19's bare-stage shape).
fn bare_stage_n(stage: [f32; 2], frames: usize, cues: Vec<SubtitleCue>) -> Project {
    let mut project = Project::new_demo();
    project.fps = 24.0;
    project.stage = stage;
    project.background = 0x333333;
    project.figures.clear();
    project.frames = (0..frames)
        .map(|_| Frame {
            poses: Vec::new(),
            duration: 1.0,
            easing: None,
            bends: Vec::new(),
        })
        .collect();
    project.subtitles = cues;
    project.subtitle_style = SubtitleStyle::default();
    project
}

/// A per-process, per-test scratch directory under the
/// OS temp dir (the tree's established fixture shape).
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "styk-wave-u-e2e-{}-{tag}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Invoke one tool's handler by name through the real
/// registry (`mcp::all_tools()`), the undo census's
/// idiom.
fn invoke(core: &mut EditorCore, name: &str, args: &Value) -> Result<Value, String> {
    let tools = all_tools();
    let tool = tools
        .iter()
        .find(|t| t.name == name)
        .unwrap_or_else(|| panic!("tool {name} missing from all_tools()"));
    (tool.handler)(core, args)
}

// ---------------------------------------------------------------------
// Pixel helpers (T19's — the GIF near-white predicate is
// also the right one for lossy JPEG: the background is
// 51 and the backing box 0, far below the threshold)
// ---------------------------------------------------------------------

type Rgb8 = image::ImageBuffer<image::Rgb<u8>, Vec<u8>>;

fn is_face_white(px: [u8; 3]) -> bool {
    px[0] >= 240 && px[1] >= 240 && px[2] >= 240
}

/// Rows of a decoded frame holding face ink.
fn rows_with_face(img: &Rgb8) -> Vec<u32> {
    let mut rows = Vec::new();
    for y in 0..img.height() {
        if (0..img.width()).any(|x| is_face_white(img.get_pixel(x, y).0)) {
            rows.push(y);
        }
    }
    rows
}

/// How many face-ink pixels a decoded frame holds.
fn count_face(img: &Rgb8) -> usize {
    let mut n = 0;
    for y in 0..img.height() {
        for x in 0..img.width() {
            if is_face_white(img.get_pixel(x, y).0) {
                n += 1;
            }
        }
    }
    n
}

/// The horizontal extent (min..=max) of face ink in a
/// decoded frame. Only called on frames known to carry
/// a face.
fn face_col_extent(img: &Rgb8) -> (u32, u32) {
    let mut lo = u32::MAX;
    let mut hi = 0u32;
    for y in 0..img.height() {
        for x in 0..img.width() {
            if is_face_white(img.get_pixel(x, y).0) {
                lo = lo.min(x);
                hi = hi.max(x);
            }
        }
    }
    (lo, hi)
}

/// Group sorted rows into runs of consecutive rows.
fn row_clusters(rows: &[u32]) -> Vec<(u32, u32)> {
    let mut out: Vec<(u32, u32)> = Vec::new();
    for &y in rows {
        match out.last_mut() {
            Some((_, last)) if *last + 1 == y => *last = y,
            _ => out.push((y, y)),
        }
    }
    out
}

/// Centre of a row cluster.
fn centre(cluster: (u32, u32)) -> f32 {
    (cluster.0 + cluster.1) as f32 / 2.0
}

/// Distance from the frame's BOTTOM edge to a cluster's
/// centre — the anchored-edge offset for the default
/// bottom style.
fn edge_offset(img: &Rgb8, cluster: (u32, u32)) -> f32 {
    img.height() as f32 - centre(cluster)
}

/// The single face cluster of a one-line frame, asserted
/// to be exactly one cluster.
fn single_face(rows: &[u32], what: &str) -> (u32, u32) {
    let clusters = row_clusters(rows);
    assert_eq!(
        clusters.len(),
        1,
        "{what}: a one-line cue must paint exactly one face cluster: {clusters:?}"
    );
    clusters[0]
}

/// Decode one exported JPEG to RGB.
fn decode_jpeg(path: &Path) -> Rgb8 {
    image::open(path)
        .unwrap_or_else(|e| panic!("JPEG {} decodes: {e}", path.display()))
        .to_rgb8()
}

/// The sorted file names present in a directory.
fn file_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()))
        .map(|e| {
            e.expect("dir entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

// ---------------------------------------------------------------------
// Leg 1 — scan vs light-read missing-audio parity
// ---------------------------------------------------------------------

/// The light read's counts for one project file —
/// `rune_sec::home_panel::read_metadata_counts`' exact
/// computation (see the header's reachability note):
/// `(figures, frames, missing_audio)`, or `None` when
/// the read fails wholesale (unparseable file, or no
/// `figures`/`frames` arrays, or an `audio` array that
/// will not deserialise). An absent `audio` key is an
/// empty clip list, not a failure.
fn light_read_counts(path: &Path) -> Option<(usize, usize, usize)> {
    let text = std::fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    let figures = value.get("figures")?.as_array()?.len();
    let frames = value.get("frames")?.as_array()?.len();
    let missing_audio = match value.get("audio") {
        None => 0,
        Some(raw) => {
            let clips: Vec<AudioClip> = serde_json::from_value(raw.clone()).ok()?;
            missing_audio_clips(&clips).len()
        }
    };
    Some((figures, frames, missing_audio))
}

#[test]
fn u15_scan_and_light_read_agree_on_missing_audio() {
    let dir = temp_dir("scan-parity");
    let present = dir.join("present.wav");
    std::fs::write(&present, b"wav").unwrap();
    let clip = |path: &Path, muted: bool| AudioClip {
        path: path.to_string_lossy().into_owned(),
        start_frame: 0,
        gain: 1.0,
        muted,
    };

    // Mixed: one present file, one missing, one missing
    // but muted — the shared classification counts
    // exactly one, and the muted leg is what proves the
    // two paths apply the same rule (a path that counted
    // muted clips, or tested a different field, would
    // diverge here).
    let mut mixed = Project::new_demo();
    mixed.name = "Audio Mixed".into();
    mixed.audio = vec![
        clip(&present, false),
        clip(&dir.join("gone.wav"), false),
        clip(&dir.join("gone-muted.wav"), true),
    ];
    save(&mixed, &dir).unwrap();

    // Healthy: every clip present — a verified zero.
    let mut healthy = Project::new_demo();
    healthy.name = "Audio Healthy".into();
    healthy.audio = vec![clip(&present, false)];
    save(&healthy, &dir).unwrap();

    // Silent: no clips at all — also a verified zero.
    let mut silent = Project::new_demo();
    silent.name = "Audio Silent".into();
    let silent_path = save(&silent, &dir).unwrap();

    // Keyless: the silent project's JSON with the
    // `audio` key removed entirely. The light read
    // treats an absent key as an empty list; the scan's
    // full parse serde-defaults it to the same. Both
    // must still agree, at zero.
    let mut keyless_json: Value =
        serde_json::from_str(&std::fs::read_to_string(&silent_path).unwrap()).unwrap();
    keyless_json["name"] = json!("Audio Keyless");
    keyless_json
        .as_object_mut()
        .unwrap()
        .remove("audio")
        .expect("the saved JSON carries an audio key to remove");
    std::fs::write(
        dir.join("audio-keyless.json"),
        serde_json::to_string(&keyless_json).unwrap(),
    )
    .unwrap();

    // Broken: unparseable. The scan lists no entry for
    // it — there is no count that could be mistaken for
    // a verified zero — and the light read fails the
    // same way.
    let broken = dir.join("broken.json");
    std::fs::write(&broken, "{ not a project").unwrap();

    let infos = scan(&dir);
    assert_eq!(infos.len(), 4, "scan found {infos:?}");
    let by_name = |name: &str| -> &ProjectInfo {
        infos
            .iter()
            .find(|i| i.name == name)
            .unwrap_or_else(|| panic!("scan lists {name}"))
    };

    // The parity claim, per project: the scan's verified
    // count and the light read's count are the SAME
    // number, and it is the expected one; the structural
    // counts the light read also produces agree too.
    for (name, expected_missing) in [
        ("Audio Mixed", 1usize),
        ("Audio Healthy", 0),
        ("Audio Silent", 0),
        ("Audio Keyless", 0),
    ] {
        let info = by_name(name);
        assert_eq!(
            info.missing_audio,
            Some(expected_missing),
            "{name}: the scan's full parse counts {expected_missing} missing"
        );
        let (figures, frames, missing) = light_read_counts(&info.path)
            .unwrap_or_else(|| panic!("{name}: the light read succeeds"));
        assert_eq!(
            missing, expected_missing,
            "{name}: the light read counts the same missing clips as the scan"
        );
        assert_eq!(
            (figures, frames),
            (info.figures, info.frames),
            "{name}: the light read's structural counts agree with the scan's"
        );
    }

    // The failure verdicts agree as well: no scan entry
    // carries the broken file's path, and the light read
    // returns None for it.
    assert!(
        infos.iter().all(|i| i.path != broken),
        "the scan lists no entry for the unparseable file"
    );
    assert_eq!(
        light_read_counts(&broken),
        None,
        "the light read fails on the unparseable file, like the scan"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------
// Leg 2 — scaled JPEG sequence carries the speaker
// burn-in at scaled geometry
// ---------------------------------------------------------------------

#[test]
fn u15_scaled_jpeg_sequence_carries_the_speaker_burn_in() {
    // The credited cue is live on authored frames 1..3
    // (half-open [1, 3)) of four — the sequence's second
    // and third files are burned in, the first and
    // fourth are clean.
    let project = bare_stage_n(
        [480.0, 400.0],
        4,
        vec![credited(1, 3, "HELLO", "NARRATOR")],
    );
    let dir = temp_dir("jpeg2x");
    let scaled_dir = dir.join("scaled");
    let control_dir = dir.join("control");
    let plain_dir = dir.join("plain");

    let n = export_jpeg_sequence_scaled(&project, &scaled_dir, 2.0)
        .expect("scaled JPEG sequence exports");
    assert_eq!(n, 4, "the scaled entry reports the four authored frames");
    assert_eq!(
        file_names(&scaled_dir),
        (1..=4)
            .map(|i| format!("frame-{i:04}.jpg"))
            .collect::<Vec<_>>(),
        "the scaled sequence keeps the unscaled entry's file naming"
    );

    let frames: Vec<Rgb8> = (1..=4)
        .map(|i| decode_jpeg(&scaled_dir.join(format!("frame-{i:04}.jpg"))))
        .collect();
    for (i, f) in frames.iter().enumerate() {
        assert_eq!(
            (f.width(), f.height()),
            (960, 800),
            "scaled JPEG frame {} is the doubled stage size",
            i + 1
        );
    }

    // Out-of-cue frames are clean: no face ink anywhere.
    // (A clean frame is flat background — JPEG writes it
    // uniformly, so the near-white predicate finds
    // nothing to isolate.)
    for i in [0usize, 3] {
        assert_eq!(
            count_face(&frames[i]),
            0,
            "scaled JPEG frame {} is outside the cue's range and must carry no burn-in",
            i + 1
        );
    }

    // Frame index 1 (frame-0002.jpg): one face cluster
    // at the doubled geometry — S11's derivation (span
    // ~28 rows, centre ~56 px from the bottom edge),
    // measured with the near-white predicate because
    // JPEG is lossy; ranges, never byte equality.
    let face2 = single_face(&rows_with_face(&frames[1]), "scaled JPEG frame 2");
    let span2 = face2.1 - face2.0 + 1;
    assert!(
        (25..=33).contains(&span2),
        "scaled JPEG: face spans {span2} rows, expected the doubled ~28-row shaped cap-ink"
    );
    let edge2 = edge_offset(&frames[1], face2);
    assert!(
        (50.0..=62.0).contains(&edge2),
        "scaled JPEG: face centre sits {edge2} px from the bottom edge, expected ~56 \
         (subtitle_line_center_y at scale 2)"
    );

    // The unscaled control of the same frame, same
    // predicate: spans and offsets double between the
    // two (the relational half of the scale claim).
    export_jpeg_sequence(&project, &control_dir).expect("control JPEG sequence exports");
    let control1 = decode_jpeg(&control_dir.join("frame-0002.jpg"));
    assert_eq!((control1.width(), control1.height()), (480, 400));
    let face1 = single_face(&rows_with_face(&control1), "control JPEG frame 2");
    let span1 = face1.1 - face1.0 + 1;
    assert!(
        (12..=16).contains(&span1),
        "control JPEG: face spans {span1} rows, expected the ~14-row shaped cap-ink"
    );
    let ratio = span2 as f32 / span1 as f32;
    assert!(
        (1.7..=2.3).contains(&ratio),
        "scaled/control face span ratio is {ratio}, expected 2.0"
    );
    let edge1 = edge_offset(&control1, face1);
    assert!(
        (edge2 - 2.0 * edge1).abs() <= 6.0,
        "scaled edge offset {edge2} must double the control's {edge1}"
    );

    // Frame index 2 is the cue's second live frame:
    // burned in at the same geometry (the range is
    // [1, 3), not a single frame).
    let face2b = single_face(&rows_with_face(&frames[2]), "scaled JPEG frame 3");
    assert_eq!(
        face2b, face2,
        "both live frames of the scaled sequence carry the face on the same rows"
    );

    // The prefix is inside the JPEG, not just any text:
    // an uncredited export of the same cue at the same
    // scale paints the bare 5-char line; the credited
    // line's extent is ~3.1x wider (T19's derivation
    // from the face run), floored at 2x for edge-glyph
    // slack and JPEG edge softening.
    let uncredited = bare_stage_n([480.0, 400.0], 4, vec![cue(1, 3, "HELLO")]);
    export_jpeg_sequence_scaled(&uncredited, &plain_dir, 2.0)
        .expect("uncredited scaled JPEG sequence exports");
    let plain1 = decode_jpeg(&plain_dir.join("frame-0002.jpg"));
    let (lo_c, hi_c) = face_col_extent(&frames[1]);
    let (lo_p, hi_p) = face_col_extent(&plain1);
    let width_credited = hi_c - lo_c + 1;
    let width_plain = hi_p - lo_p + 1;
    assert!(
        width_credited >= 2 * width_plain,
        "credited JPEG line is {width_credited} px wide vs the uncredited \
         {width_plain} px — the 'NARRATOR: ' prefix must widen the burned-in line"
    );
}

// ---------------------------------------------------------------------
// Leg 3 — bundle refusal atomicity through the registry
// ---------------------------------------------------------------------

#[test]
fn u15_bundle_refuses_an_invalid_scale_before_writing_anything() {
    // T8's contract, driven through mcp::all_tools():
    // a present 'scale' is validated before the bundle
    // creates its directory or writes any member — a
    // refusal must leave no trace on disk and spend no
    // undo step.
    let dir = temp_dir("bundle-refusal");
    let mut core = EditorCore::new();
    assert_eq!(core.undo_depth(), 0, "a fresh core has an empty undo stack");

    for (tag, scale, sentence) in [
        // A zero scale: a number, but not positive
        // finite — check_export_scale's sentence.
        ("zero", json!(0), "export scale must be a positive finite number"),
        // A negative scale: same refusal.
        ("negative", json!(-2.5), "export scale must be a positive finite number"),
        // 1e300 is a finite f64 whose f32 cast overflows
        // to infinity: still refused, as +inf.
        ("overflow", json!(1e300), "export scale must be a positive finite number"),
        // A non-number scale is not silently treated as
        // absent (the pre-T8 behaviour): it refuses in
        // the helpers' voice.
        ("string", json!("2"), "argument 'scale' must be a number"),
    ] {
        let target = dir.join(format!("bundle-{tag}"));
        let err = invoke(
            &mut core,
            "export_bundle",
            &json!({
                "kind": "images",
                "path": target.to_string_lossy(),
                "scale": scale,
            }),
        )
        .expect_err("an invalid scale must refuse the bundle call");
        assert!(
            err.contains(sentence),
            "{tag}: refusal names the scale rule ({sentence}), got: {err}"
        );
        assert!(
            !target.exists(),
            "{tag}: the refused bundle must not create its target directory"
        );
        assert_eq!(
            core.undo_depth(),
            0,
            "{tag}: a refused bundle spends no undo step"
        );
    }

    // Nothing at all was written: the scratch dir holds
    // no bundle output of any kind.
    assert_eq!(
        file_names(&dir),
        Vec::<String>::new(),
        "four refused bundles leave the scratch dir empty"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------
// Leg 4 — JPEG sequence 1.0 identity on a subtitled
// fixture
// ---------------------------------------------------------------------

#[test]
fn u15_jpeg_sequence_at_1_0_is_byte_identical_on_a_subtitled_fixture() {
    // T7's delegation claim, on a fixture whose every
    // frame carries the credited burn-in (T19's identity
    // shape): the scaled entry at exactly 1.0 and the
    // range entry at exactly 1.0 write the unscaled
    // entry's files, byte for byte — subtitle stamps
    // included in the compared bytes.
    let project = bare_stage_n(
        [480.0, 400.0],
        4,
        vec![credited(0, 4, "HELLO", "NARRATOR")],
    );
    let dir = temp_dir("jpeg1x");
    let plain_dir = dir.join("plain");
    let scaled_dir = dir.join("scaled");
    let ranged_dir = dir.join("ranged");

    assert_eq!(
        export_jpeg_sequence(&project, &plain_dir).expect("unscaled sequence"),
        4
    );
    assert_eq!(
        export_jpeg_sequence_scaled(&project, &scaled_dir, 1.0)
            .expect("scaled-at-1.0 sequence"),
        4
    );
    assert_eq!(
        export_jpeg_sequence_range_scaled(&project, 0, 3, &ranged_dir, 1.0)
            .expect("range-scaled-at-1.0 sequence"),
        4
    );

    let expected_names: Vec<String> = (1..=4).map(|i| format!("frame-{i:04}.jpg")).collect();
    assert_eq!(file_names(&plain_dir), expected_names);
    assert_eq!(
        file_names(&scaled_dir),
        expected_names,
        "the scaled-at-1.0 entry writes the same file set"
    );
    assert_eq!(
        file_names(&ranged_dir),
        expected_names,
        "the range entry at 1.0 writes the same file set"
    );

    for name in &expected_names {
        let plain_bytes = std::fs::read(plain_dir.join(name)).expect("read unscaled JPEG");
        assert!(
            !plain_bytes.is_empty(),
            "{name}: the unscaled file is a real JPEG, not an empty write"
        );
        assert_eq!(
            plain_bytes,
            std::fs::read(scaled_dir.join(name)).expect("read scaled-at-1.0 JPEG"),
            "{name}: the scaled entry at 1.0 is byte-identical"
        );
        assert_eq!(
            plain_bytes,
            std::fs::read(ranged_dir.join(name)).expect("read ranged-at-1.0 JPEG"),
            "{name}: the range entry at 1.0 is byte-identical"
        );
    }

    // Non-vacuity: the compared bytes actually carry
    // the burn-in — the identity above is not four
    // blank frames agreeing with themselves.
    let first = decode_jpeg(&plain_dir.join("frame-0001.jpg"));
    assert!(
        count_face(&first) > 0,
        "frame 1 of the identity fixture carries the credited burn-in"
    );
}
