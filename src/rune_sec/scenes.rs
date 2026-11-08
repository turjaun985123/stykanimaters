//! Snap scenes for headless CI checks (Rune Phase D, §37).
//!
//! Rune's tooling can render any binary offscreen
//! (`--snap out.png`, `--tree out.json`) after first applying a
//! NAMED SCENE to the freshly initialised state
//! (`--scene NAME`, `--list-scenes` to enumerate). A scene is a
//! plain function pointer that mutates the initial state:
//!
//! ```text
//! AppSpec::scenes: &'static [(&'static str, fn(&mut S))]
//!     (rune-src/crates/rune/src/tooling.rs:25)
//! ```
//!
//! `with_scene` (tooling.rs:283) looks the name up in that slice
//! and calls the fn on the `init()` state before the first
//! frame; an unknown name aborts listing the valid ones. So a
//! scene fn receives `&mut State` (NOT a builder — it cannot
//! construct or replace the state, and it must be total: every
//! fn below only calls guarded core ops / pub-field writes and
//! cannot panic on the demo project or an empty one).
//!
//! Integrator wiring (in `main.rs`, next to the other AppSpec
//! fields — the field is a `&'static` slice, so the Vec this
//! module returns is leaked once at startup):
//!
//! ```text
//! scenes: Box::leak(crate::rune_sec::scenes::scenes().into_boxed_slice()),
//! ```
//!
//! Every mutation below goes through the REAL ops the GUI
//! itself uses — `EditorCore::goto` (the timeline chips' op),
//! `EditorCore::select_figure` (the sidebar's op) plus the pub
//! `selected_node` field a canvas hit-test would set, and
//! `chrome_view::toggle_play` (the transport button / Space
//! key op). No scene invents state the editor cannot reach.

use crate::State;

/// `"default"` — the demo project exactly as `EditorCore::new()`
/// seeds it (frame 0, figure 0 selected, nothing playing).
/// The no-op body is deliberate: `--scene default` must equal
/// a run with no `--scene` at all.
fn scene_default(_s: &mut State) {}

/// `"frame-3"` — the playhead on frame index 3 (0-based), i.e.
/// the FOURTH frame, chip key `frame-4`, status "Frame 4/5" in
/// the 5-frame demo. Uses `EditorCore::goto`, which guards
/// `frame < frames.len()` and syncs `playhead`, so this is a
/// no-op rather than a panic on a shorter project.
fn scene_frame_3(s: &mut State) {
    s.core.goto(3);
}

/// `"selected"` — figure 0 selected with one of its nodes
/// selected, so the snap shows the stage's selection handles
/// (drawn on the selected figure; the selected joint larger /
/// amber, per `stage_view`). `select_figure` is the real op
/// (it also clears the node selection); the node itself is
/// then set through the pub `selected_node` field, the same
/// field `begin_drag`'s hit-test writes. The node chosen is
/// the figure's `head` node when it has one (the demo
/// humanoid does: nodes are hips/chest/head/…, model.rs
/// `FigureDef::humanoid`), falling back to node 1, then 0 —
/// always bounds-checked, never assuming a rig size.
fn scene_selected(s: &mut State) {
    if s.core.project.figures.is_empty() {
        return;
    }
    s.core.select_figure(0);
    let nodes = &s.core.project.figures[0].def.nodes;
    if nodes.is_empty() {
        return;
    }
    let head = nodes.iter().position(|n| n.name == "head");
    s.core.selected_node = Some(head.unwrap_or_else(|| 1.min(nodes.len() - 1)));
}

/// `"playing"` — transport running. There is no core
/// play/pause op; the editor's own op is
/// `chrome_view::toggle_play` (transport button + Space key),
/// which syncs `playhead` to the shown frame before flipping
/// the pub `playing` field — the scene calls exactly that,
/// guarded so it only ever turns playback ON from the fresh
/// initial state.
///
/// Representable, with one caveat for CI: a `--snap` renders
/// at `--at` seconds on the virtual clock, and `poll` advances
/// the playhead while `playing` is set — so the still is only
/// deterministic at `--at 0` (frame 0, transport showing its
/// playing state). For motion, use `--frames`/`--strip`
/// instead of a single snap.
fn scene_playing(s: &mut State) {
    if !s.core.playing {
        crate::chrome_view::toggle_play(s);
    }
}

/// The scene table for `AppSpec::scenes`, in `--list-scenes`
/// display order. Element type is Rune's own, verbatim:
/// `(&'static str, fn(&mut State))` (tooling.rs:25).
pub fn scenes() -> Vec<(&'static str, fn(&mut State))> {
    vec![
        ("default", scene_default),
        ("frame-3", scene_frame_3),
        ("selected", scene_selected),
        ("playing", scene_playing),
    ]
}

/// Layout facts a CI `--tree` check should assert.
///
/// CI usage: run the binary with `--tree out.json` (plus
/// `--scene NAME` per scene) and assert each fact below
/// against the dumped element tree. Every fact is about KEYS
/// and COUNTS only — those are what the tree dump carries;
/// state that lives inside the stage canvas (selection
/// handles, the current frame's pose) is drawn in the
/// `Shape`, not the tree, and can only be checked in the
/// `--snap` PNG.
///
/// 1. Exactly one element is keyed `stage` (the canvas; the
///    key is set in `main.rs`'s view).
/// 2. An element keyed `app-bar` exists (chrome_view).
/// 3. An element keyed `transport` exists (chrome_view).
/// 4. An element keyed `timeline` exists (chrome_view).
/// 5. The number of elements keyed `frame-N` equals the
///    project frame count — one chip per frame, keys 1-based
///    (`frame-1` … `frame-N`); the built-in demo project has
///    5 frames, so the default tree holds exactly
///    `frame-1`..=`frame-5`.
/// 6. Every `frame-N` key for N in 1..=frame_count occurs
///    exactly once (no duplicate or missing chips).
/// 7. Facts 1–6 hold unchanged under `--scene frame-3`,
///    `--scene selected` and `--scene playing`: scenes change
///    editor STATE, never the element inventory — a chip-count
///    or stage-key drift under a scene means the view started
///    branching on state it should only draw from.
pub fn layout_assertions() -> Vec<&'static str> {
    vec![
        "exactly one element keyed \"stage\"",
        "an element keyed \"app-bar\" exists",
        "an element keyed \"transport\" exists",
        "an element keyed \"timeline\" exists",
        "count of elements keyed \"frame-N\" == project frame count (5 in the demo)",
        "each \"frame-N\" key for N in 1..=frame_count occurs exactly once",
        "facts above are identical under --scene default/frame-3/selected/playing",
    ]
}
