//! Wave P regression e2e — the Squad 2 fixes proven end
//! to end through the public API (`EditorCore` ops and the
//! project open path), not through the internals that
//! implemented them:
//!
//! - Bend pairing survives figure reorders: the full
//!   layers move (`EditorCore::move_figure_full`, P13's
//!   fix in `layers::move_figure`) forth and back, AND
//!   single adjacent steps (`EditorCore::move_figure`)
//!   across the sparse padded-prefix boundary — the
//!   single-step sibling P13 flagged, fixed by P46 with
//!   the same pad-then-swap remedy. Before that fix, a
//!   length-1 bend table swapping figures 0<->1 left
//!   figure 0's bend row sitting on figure 1.
//! - A project whose audio clip points at a file that
//!   does not exist surfaces the loss on open (P14):
//!   `EditorCore::missing_audio_clips` names the stored
//!   path and the open status carries it, while the load
//!   itself drops nothing.
//! - A legacy project JSON with no `format_version`
//!   field loads as the current version (the P16/P23
//!   contract), both at the serde pair and through the
//!   real file-open path.
//!
//! Fixtures set fps explicitly (24.0) per the wave's
//! timing contract; nothing here assumes a demo fps.

use std::path::PathBuf;

use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::{AudioClip, Project, FORMAT_VERSION_CURRENT};

// ---------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------

/// A three-figure core (Styk, B, C) at an explicit
/// 24 fps, built from the production demo project plus
/// the production `add_figure` op — the layers test
/// module's `three_figures` shape, one level up.
fn three_figure_core() -> EditorCore {
    let mut core = EditorCore::new();
    core.project.fps = 24.0;
    core.project.add_figure("B", 0xFF0000, [200.0, 200.0]);
    core.project.add_figure("C", 0x00FF00, [300.0, 200.0]);
    core
}

fn names(core: &EditorCore) -> Vec<&str> {
    core.project.figures.iter().map(|f| f.name.as_str()).collect()
}

/// One figure's signature bend row: distinct per figure,
/// so a row found on the wrong figure is unmistakable.
fn expected_row(name: &str) -> Vec<f32> {
    match name {
        "Styk" => vec![0.25, -0.5],
        "B" => vec![0.5, -1.0],
        "C" => vec![0.75, -1.5],
        other => panic!("unexpected figure {other}"),
    }
}

/// Give every frame a FULL bend table (one signature row
/// per figure, in figure order).
fn fill_full_bends(core: &mut EditorCore) {
    for frame in &mut core.project.frames {
        frame.bends = vec![expected_row("Styk"), expected_row("B"), expected_row("C")];
    }
}

/// The pairing contract, checked in every frame: the row
/// stored at a slot belongs to the figure now in that
/// slot, and the evaluated read (`Project::bends_at`)
/// agrees — the stored row's values up front, straight
/// (0.0) beyond them.
fn assert_bend_pairing(core: &EditorCore) {
    for (f, frame) in core.project.frames.iter().enumerate() {
        assert_eq!(frame.bends.len(), 3, "frame {f}: full table expected");
        for (slot, fig) in core.project.figures.iter().enumerate() {
            let want = expected_row(&fig.name);
            assert_eq!(
                frame.bends[slot], want,
                "frame {f} slot {slot}: stored bend row must belong to {}",
                fig.name
            );
            let read = core.project.bends_at(f as f32, slot);
            assert!(read.len() >= 2, "frame {f} slot {slot}: evaluated row too short");
            assert_eq!(
                &read[..2],
                &want[..],
                "frame {f} slot {slot}: evaluated bends must belong to {}",
                fig.name
            );
            assert!(
                read[2..].iter().all(|v| *v == 0.0),
                "frame {f} slot {slot}: bends beyond the stored row read as straight"
            );
        }
    }
}

/// A per-process, per-test scratch directory under the
/// OS temp dir (the tree's established fixture shape).
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "styk-wave-p-e2e-{}-{tag}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

// ---------------------------------------------------------------------
// Bend pairing — the full move (P13's layers fix, end to end)
// ---------------------------------------------------------------------

#[test]
fn bend_pairing_survives_move_figure_full_forth_and_back() {
    let mut core = three_figure_core();
    fill_full_bends(&mut core);
    assert_bend_pairing(&core);

    // Forth: Styk jumps two slots to the front.
    core.select_figure(0);
    core.move_figure_full(2);
    assert_eq!(names(&core), vec!["B", "C", "Styk"]);
    assert_bend_pairing(&core);

    // Back: the selection followed Styk, so -2 restores
    // the original order — and the original pairing.
    core.move_figure_full(-2);
    assert_eq!(names(&core), vec!["Styk", "B", "C"]);
    assert_bend_pairing(&core);

    // Single-slot full moves keep the pairing too.
    core.select_figure(0);
    core.move_figure_full(1);
    assert_eq!(names(&core), vec!["B", "Styk", "C"]);
    assert_bend_pairing(&core);
    core.move_figure_full(-1);
    assert_eq!(names(&core), vec!["Styk", "B", "C"]);
    assert_bend_pairing(&core);
}

// ---------------------------------------------------------------------
// Bend pairing — single adjacent steps at the sparse boundary (P46)
// ---------------------------------------------------------------------

/// The boundary P13 flagged: the bend writer pads a
/// frame's table only up to the highest figure edited,
/// so "only figure 0 has bends" is a length-1 table.
/// Swapping figures 0<->1 must carry that row with its
/// figure — before the pad-then-swap fix the both-rows
/// guard never fired and the row stayed on figure 1.
#[test]
fn single_move_figure_steps_carry_bends_across_the_sparse_boundary() {
    let mut core = three_figure_core();
    let row = vec![0.5, -0.25];
    for frame in &mut core.project.frames {
        frame.bends = vec![row.clone()];
    }

    core.select_figure(0);
    core.move_figure(1);
    assert_eq!(names(&core), vec!["B", "Styk", "C"]);
    for (f, frame) in core.project.frames.iter().enumerate() {
        // The pad-then-swap remedy's storage shape: the
        // table is padded to the figure count and the row
        // sits on its figure's new slot.
        assert_eq!(frame.bends.len(), 3, "frame {f}: table padded for the swap");
        assert!(frame.bends[0].is_empty(), "frame {f}: slot 0 is bend-free B");
        assert_eq!(frame.bends[1], row, "frame {f}: the row travelled with Styk");
        assert!(frame.bends[2].is_empty(), "frame {f}: slot 2 is bend-free C");
        // The evaluated reads agree: slot 1 bends, the
        // bend-free neighbours read straight.
        let read = core.project.bends_at(f as f32, 1);
        assert_eq!(&read[..2], &row[..], "frame {f}: evaluated row on Styk");
        assert!(
            core.project.bends_at(f as f32, 0).iter().all(|v| *v == 0.0),
            "frame {f}: B reads straight"
        );
        assert!(
            core.project.bends_at(f as f32, 2).iter().all(|v| *v == 0.0),
            "frame {f}: C reads straight"
        );
    }

    // Step back (the selection followed Styk to slot 1):
    // the row returns to slot 0 with its figure.
    core.move_figure(-1);
    assert_eq!(names(&core), vec!["Styk", "B", "C"]);
    for (f, frame) in core.project.frames.iter().enumerate() {
        assert_eq!(frame.bends[0], row, "frame {f}: the row is home again");
        assert!(
            core.project.bends_at(f as f32, 1).iter().all(|v| *v == 0.0),
            "frame {f}: B reads straight after the return"
        );
    }
}

/// The same boundary, one slot over: swapping the two
/// bend-free figures (1<->2) must leave figure 0's lone
/// row exactly where it was.
#[test]
fn single_move_figure_swap_outside_the_padded_prefix_keeps_the_row() {
    let mut core = three_figure_core();
    let row = vec![0.5, -0.25];
    for frame in &mut core.project.frames {
        frame.bends = vec![row.clone()];
    }

    core.select_figure(1);
    core.move_figure(1);
    assert_eq!(names(&core), vec!["Styk", "C", "B"]);
    for (f, frame) in core.project.frames.iter().enumerate() {
        assert_eq!(frame.bends[0], row, "frame {f}: Styk's row untouched");
        assert_eq!(
            &core.project.bends_at(f as f32, 0)[..2],
            &row[..],
            "frame {f}: evaluated row still on Styk"
        );
        assert!(
            core.project.bends_at(f as f32, 1).iter().all(|v| *v == 0.0)
                && core.project.bends_at(f as f32, 2).iter().all(|v| *v == 0.0),
            "frame {f}: the swapped pair reads straight"
        );
    }
}

/// The carrier one slot in: a length-2 table whose only
/// row belongs to figure 1. Swapping 1<->2 crosses the
/// table's end — before the fix the guard refused the
/// swap and the row landed on figure 2's predecessor.
#[test]
fn single_move_figure_carries_a_row_past_the_table_end() {
    let mut core = three_figure_core();
    for frame in &mut core.project.frames {
        frame.bends = vec![Vec::new(), vec![0.75]];
    }

    core.select_figure(1);
    core.move_figure(1);
    assert_eq!(names(&core), vec!["Styk", "C", "B"]);
    for (f, frame) in core.project.frames.iter().enumerate() {
        assert_eq!(frame.bends.len(), 3, "frame {f}: table padded for the swap");
        assert_eq!(frame.bends[2], vec![0.75], "frame {f}: B's row travelled to slot 2");
        assert!(
            core.project.bends_at(f as f32, 1).iter().all(|v| *v == 0.0),
            "frame {f}: C reads straight"
        );
        assert_eq!(
            core.project.bends_at(f as f32, 2)[0],
            0.75,
            "frame {f}: evaluated bend on B"
        );
    }
}

/// The empty-table guard: a bend-free project gains no
/// rows from a reorder — padding must not materialise
/// tables the writer never wrote.
#[test]
fn single_move_figure_leaves_bend_free_frames_empty() {
    let mut core = three_figure_core();
    core.select_figure(0);
    core.move_figure(1);
    assert_eq!(names(&core), vec!["B", "Styk", "C"]);
    for (f, frame) in core.project.frames.iter().enumerate() {
        assert!(
            frame.bends.is_empty(),
            "frame {f}: a bend-free frame stays bend-free across a swap"
        );
    }
}

// ---------------------------------------------------------------------
// Missing audio surfaces on open (P14's landed shape)
// ---------------------------------------------------------------------

/// Write `project` into `dir` and open it through the
/// editor's real file-open path (the `project_files`
/// selector + `open_project_file`), returning the core
/// that landed on it.
fn open_from_disk(dir: &PathBuf, project: &Project) -> EditorCore {
    let file = dir.join("proj.styk.json");
    std::fs::write(&file, serde_json::to_string_pretty(project).unwrap())
        .expect("write project file");
    let mut core = EditorCore::new();
    core.project_files = vec![file];
    core.project_index = 0;
    core.open_project_file();
    core
}

#[test]
fn a_missing_audio_file_surfaces_on_project_open() {
    let dir = temp_dir("missing-audio");
    let present = dir.join("present.wav");
    std::fs::write(&present, b"").expect("write present clip file");
    let present_s = present.to_string_lossy().into_owned();
    let missing_s = dir.join("gone.wav").to_string_lossy().into_owned();

    let mut project = Project::new_demo();
    project.fps = 24.0;
    project.audio = vec![
        AudioClip {
            path: present_s.clone(),
            start_frame: 0,
            gain: 1.0,
            muted: false,
        },
        AudioClip {
            path: missing_s.clone(),
            start_frame: 3,
            gain: 1.0,
            muted: false,
        },
    ];
    let core = open_from_disk(&dir, &project);

    // The surface: exactly the missing clip, named by its
    // stored path; the present clip is not reported.
    assert_eq!(core.missing_audio_clips().len(), 1);
    assert_eq!(core.missing_audio_clips()[0], missing_s);
    // The load dropped nothing: both clips survive, in
    // order, exactly as saved.
    assert_eq!(core.project.audio.len(), 2, "the load drops nothing");
    assert_eq!(core.project.audio[0].path, present_s);
    assert_eq!(core.project.audio[0].start_frame, 0);
    assert_eq!(core.project.audio[1].path, missing_s);
    assert_eq!(core.project.audio[1].start_frame, 3);
    assert_eq!(core.project.fps, 24.0, "the opened project is the saved one");
    // The open status carries the finding and names the
    // missing file — and not the present one.
    assert!(core.status.starts_with("Opened "), "status: {}", core.status);
    assert!(core.status.contains(&missing_s), "status: {}", core.status);
    assert!(
        !core.status.contains("present.wav"),
        "status: {}",
        core.status
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_project_with_all_audio_present_opens_without_a_missing_report() {
    let dir = temp_dir("all-present");
    let a = dir.join("a.wav");
    let b = dir.join("b.wav");
    std::fs::write(&a, b"").expect("write clip a");
    std::fs::write(&b, b"").expect("write clip b");
    let mut project = Project::new_demo();
    project.fps = 24.0;
    project.audio = vec![
        AudioClip {
            path: a.to_string_lossy().into_owned(),
            start_frame: 0,
            gain: 1.0,
            muted: false,
        },
        AudioClip {
            path: b.to_string_lossy().into_owned(),
            start_frame: 5,
            gain: 1.0,
            muted: false,
        },
    ];
    let file = dir.join("proj.styk.json");
    let core = open_from_disk(&dir, &project);

    assert!(core.missing_audio_clips().is_empty());
    // A clean load's status is byte-identical to before
    // the report existed: just the opened path.
    assert_eq!(core.status, format!("Opened {}", file.display()));
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------
// Legacy load — no format_version field (P16/P23 contract)
// ---------------------------------------------------------------------

#[test]
fn a_legacy_project_json_without_format_version_loads() {
    let mut project = Project::new_demo();
    project.fps = 24.0;

    // The legacy document: today's saved JSON with the
    // version stamp removed by hand — the shape every
    // project file written before the field existed has.
    let mut value = serde_json::to_value(&project).expect("serialise fixture");
    let removed = value
        .as_object_mut()
        .expect("project serialises to an object")
        .remove("format_version");
    assert!(
        removed.is_some(),
        "the fixture must start from a stamped document"
    );
    let text = serde_json::to_string_pretty(&value).expect("serialise legacy document");
    assert!(!text.contains("format_version"), "the stamp is really gone");

    // The production load pair (projects::load's
    // `serde_json::from_str::<Project>`) accepts it and
    // the absent field defaults to the current version.
    let loaded: Project = serde_json::from_str(&text).expect("legacy document loads");
    assert_eq!(loaded.format_version, FORMAT_VERSION_CURRENT);
    assert_eq!(loaded.format_version, 1);
    assert_eq!(loaded.fps, 24.0, "the rest of the document survives intact");

    // And through the editor's real open path, the same
    // legacy document on disk lands at the current
    // version.
    let dir = temp_dir("legacy-open");
    let file = dir.join("legacy.styk.json");
    std::fs::write(&file, &text).expect("write legacy file");
    let mut core = EditorCore::new();
    core.project_files = vec![file];
    core.project_index = 0;
    core.open_project_file();
    assert_eq!(core.project.format_version, FORMAT_VERSION_CURRENT);
    assert_eq!(core.project.fps, 24.0);
    let _ = std::fs::remove_dir_all(&dir);
}
