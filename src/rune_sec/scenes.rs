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
//! key op). The Phase J scenes add two more of the shell's
//! own calls: `workspaces::select_workspace` (the workspace
//! chips' and the palette `go-*` commands' op) and
//! `EditorCore::particle_add_event` (the Particles panel's
//! Add op, `particle_ops.rs`), with the selection written to
//! the pub `particle_selected` field exactly as the panel's
//! own add handler writes it. No scene invents state the
//! editor cannot reach.
//!
//! The Phase K scenes keep that discipline with ONE named
//! exception: `graph-duplicates` plants its two `Channel`
//! values by writing the pub `project.channels` field
//! directly, because no core op creates a channel with
//! chosen keys (the graph panel's key op keys the CURRENT
//! value at the playhead) — the graph panel's own tests
//! construct `Channel` values exactly this way, and the
//! planted data is ordinary project data the panel then
//! reads through its normal paths. `fx-split-mid` adds its
//! clip through `EditorCore::add_fx_clip` — the FX
//! browser's op, called the way the browser calls it —
//! after selecting a node the way the `selected` scene
//! does, since the op refuses without one.
//!
//! The Phase L scenes keep the same discipline:
//! `three-d` enters its session through
//! `EditorCore::enter_3d` — the exact call the 3D panel's
//! start control makes — then orbits and selects through
//! the panel's own session ops (`fig3_orbit`,
//! `fig3_select_next`); `loop-range` sets its range
//! through `EditorCore::set_loop_range` (the timeline's
//! op) and plants its one keyed channel by the
//! `graph-duplicates` direct-write exception, for the
//! same reason: no op keys chosen values at chosen
//! frames.

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

/// `"export-open"` — the shell switched to the Export
/// workspace, so the snap renders the Export centre
/// (`export-panel`) with the Animate panels gated out. The
/// switch is `workspaces::select_workspace` — the exact call
/// the workspace chips and the command palette's `go-*`
/// commands make (it sets `s.workspace` and persists the
/// choice in the layout prefs, like a real chip press).
fn scene_export_open(s: &mut State) {
    crate::rune_sec::workspaces::select_workspace(
        s,
        crate::rune_sec::workspaces::Workspace::Export,
    );
}

/// `"fx-selected"` — the Effects workspace with one particle
/// event added at the current frame and selected, so the
/// snap shows the stage's FX gizmo for the event and the
/// Particles panel's detail block (not its guidance line).
/// The add replicates the panel's own add handler
/// (`particles_panel::add_event`) step for step: the event
/// goes through `EditorCore::particle_add_event` — the
/// Phase G `particle_ops` op the panel calls — placed at
/// the stage centre, and the selection is taken (the pub
/// `particle_selected` field) only when the list actually
/// grew by one, so a refused add can never leave a stale
/// selection behind.
fn scene_fx_selected(s: &mut State) {
    crate::rune_sec::workspaces::select_workspace(
        s,
        crate::rune_sec::workspaces::Workspace::Effects,
    );
    let frame = s.core.current;
    let [stage_w, stage_h] = s.core.project.stage;
    let len_before = s.core.project.particles.len();
    s.core
        .particle_add_event(frame, stage_w / 2.0, stage_h / 2.0);
    if s.core.project.particles.len() == len_before + 1 {
        s.particle_selected = Some(len_before);
    }
}

/// `"graph-duplicates"` — the Graph workspace with TWO
/// graph channels planted for figure 0 with the SAME
/// target (`ChannelTarget::RootX`), so a `--tree` dump can
/// prove the graph panel's channel list shows both rows
/// (`graph-chan-0` and `graph-chan-1`) and the Phase J
/// index-based selection has a duplicate pair to bite on.
/// The twins are told apart by their keys — the first has
/// one key at value 1.0, the second two keys at values 2.0
/// and 4.0 — the graph panel's own duplicate fixture
/// (its test module's `core_with_duplicate_channels`)
/// built the same way. Figure 0 is left selected, since
/// channel resolution scopes to the selected figure. The
/// plant is a direct write of the pub `project.channels`
/// field (see the module docs for why no op can do this);
/// on a figure-less project the scene stops after the
/// workspace switch rather than planting channels for a
/// figure that does not exist.
fn scene_graph_duplicates(s: &mut State) {
    use stykanimate_rs::graph::{Channel, ChannelTarget, Key};
    crate::rune_sec::workspaces::select_workspace(
        s,
        crate::rune_sec::workspaces::Workspace::Graph,
    );
    if s.core.project.figures.is_empty() {
        return;
    }
    s.core.select_figure(0);
    s.core.project.channels.push(Channel {
        figure: 0,
        target: ChannelTarget::RootX,
        keys: vec![Key::flat(0.0, 1.0)],
    });
    s.core.project.channels.push(Channel {
        figure: 0,
        target: ChannelTarget::RootX,
        keys: vec![Key::flat(0.0, 2.0), Key::flat(3.0, 4.0)],
    });
}

/// `"fx-split-mid"` — the Effects workspace with one FX
/// clip added, selected, and the playhead parked strictly
/// inside the clip's span, so a `--tree` dump shows the
/// FX inspector's detail block (its Split at playhead
/// button, `fx-split`, mounts only with a selection) and a
/// `--snap` shows the clip mid-flight at the playhead.
/// The clip goes through `EditorCore::add_fx_clip` — the
/// FX browser's op, called with the browser's own first
/// kind — after a node is selected exactly as the
/// `selected` scene selects one, because the op refuses
/// ("Select a joint first") without a selected node. The
/// selection is taken (the pub `fx_selected` field) only
/// when the list actually grew by one, the same guard the
/// `fx-selected` scene uses for particles. The park frame
/// is read back from the clip itself: `start + duration/2`
/// (rounded down) — the op's default clip is 16 frames,
/// far longer than the 5-frame demo timeline, so on the
/// demo the target clamps to the LAST frame, which is
/// still strictly inside the clip; on a project whose
/// timeline covers the midpoint the playhead lands
/// exactly mid-clip. (Had the clip been shorter than 2
/// frames, the scene would first lengthen it through
/// `fx_resize_clip`, the panel's own resize op, rather
/// than write a duration by hand.)
fn scene_fx_split_mid(s: &mut State) {
    crate::rune_sec::workspaces::select_workspace(
        s,
        crate::rune_sec::workspaces::Workspace::Effects,
    );
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
    let len_before = s.core.project.fx_clips.len();
    s.core.add_fx_clip(stykanimate_rs::vfx::ClipKind::Burst);
    if s.core.project.fx_clips.len() != len_before + 1 {
        return;
    }
    s.fx_selected = Some(len_before);
    // Read the clip's span back and lengthen it through
    // the resize op if it is too short to park inside.
    let duration = s.core.project.fx_clips[len_before].clip.duration;
    if duration < 2.0 {
        s.core.fx_resize_clip(len_before, 2.0 - duration);
    }
    let clip = &s.core.project.fx_clips[len_before].clip;
    let (start, duration) = (clip.start, clip.duration);
    let mid = start + (duration / 2.0).floor();
    let last_frame = s.core.frame_count().saturating_sub(1);
    let target = (mid as usize).min(last_frame);
    if (target as f32) > start {
        s.core.goto(target);
    }
}

/// `"three-d"` — the 3D workspace with a live fig3
/// session: the 3D humanoid at rest, the camera orbited
/// off its default (yaw +35°, pitch +18°), and joint 1
/// selected. The session starts through
/// `EditorCore::enter_3d` — the single call the 3D
/// panel's start control makes (`three_d_panel`'s
/// handler calls exactly it) — which loads
/// `fig3d::humanoid3()` at rest with the default camera
/// and joint 0 selected. The orbit goes through
/// `EditorCore::fig3_orbit`, the panel's camera op, and
/// the selection through `EditorCore::fig3_select_next`,
/// the panel's joint-cycling op: one call from the start
/// op's joint 0 lands on joint 1. The orbited camera is
/// what makes the Phase L preview's ground grid read as
/// a floor rather than a flat backdrop in the snap.
fn scene_three_d(s: &mut State) {
    crate::rune_sec::workspaces::select_workspace(
        s,
        crate::rune_sec::workspaces::Workspace::Build3D,
    );
    s.core.enter_3d();
    s.core.fig3_orbit(35.0, 18.0);
    s.core.fig3_select_next();
}

/// `"loop-range"` — the Animate workspace with a loop
/// range over frames 1..=3 (0-based, the timeline marks'
/// loop edges) and ONE graph channel planted for
/// figure 0 with flat keys at exactly the range's ends,
/// so a `--tree` dump shows the timeline's loop row in
/// its set state and a `--snap` shows the loop edges
/// drawn around chips 2–4. The range goes through
/// `EditorCore::set_loop_range` — the op the timeline's
/// loop-edge drag writes — which orders and clamps its
/// ends against the timeline. The channel is planted by
/// the `graph-duplicates` direct-write exception (see
/// the module docs): flat keys at frames 1.0 and 3.0 on
/// the RootX target, values 0.0 and 48.0 so the channel
/// visibly moves the root across the loop. Figure 0 is
/// left selected, since channel resolution scopes to
/// the selected figure; on a figure-less project the
/// scene stops after setting the range.
fn scene_loop_range(s: &mut State) {
    use stykanimate_rs::graph::{Channel, ChannelTarget, Key};
    crate::rune_sec::workspaces::select_workspace(
        s,
        crate::rune_sec::workspaces::Workspace::Animate,
    );
    s.core.set_loop_range(1, 3);
    if s.core.project.figures.is_empty() {
        return;
    }
    s.core.select_figure(0);
    s.core.project.channels.push(Channel {
        figure: 0,
        target: ChannelTarget::RootX,
        keys: vec![Key::flat(1.0, 0.0), Key::flat(3.0, 48.0)],
    });
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
        ("export-open", scene_export_open),
        ("fx-selected", scene_fx_selected),
        ("graph-duplicates", scene_graph_duplicates),
        ("fx-split-mid", scene_fx_split_mid),
        ("three-d", scene_three_d),
        ("loop-range", scene_loop_range),
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
/// 8. Under `--scene export-open` (Phase J) the inventory
///    DOES change — by workspace gating, not by state: an
///    element keyed `export-panel` exists (the Export
///    centre, mounted only in the Export workspace) and NO
///    element is keyed `figures-panel` (an Animate-only
///    panel, gated out). A tree that still shows
///    `figures-panel` here means the workspace switch or
///    the gating regressed.
/// 9. Under `--scene fx-selected` (Phase J): an element
///    keyed `particles` exists (the Particles panel, gated
///    to the Effects workspace) and its detail block —
///    keyed `particles-detail` — holds the editing
///    controls only a valid selection mounts, e.g. an
///    element keyed `particles-kind-prev`; with no
///    selection that block renders only a guidance line,
///    so the stepper's presence is what proves the scene's
///    selection landed. (The stage's FX gizmo for the
///    selected event is canvas-drawn, so it is a `--snap`
///    fact, not a tree fact.)
/// 10. Under `--scene graph-duplicates` (Phase K): an
///    element keyed `graph-chan-1` exists — the SECOND of
///    the two planted channel rows in the graph panel's
///    channel list (`graph-chan-{i}` keys are the channel's
///    project index). A tree holding only `graph-chan-0`
///    means the duplicate plant or the list's
///    every-channel listing regressed.
/// 11. Under `--scene fx-split-mid` (Phase K): an element
///    keyed `fx-split` exists — the FX inspector's Split
///    at playhead button, which the inspector builds only
///    inside its detail block, and the detail block only
///    for a valid clip selection; the button's presence is
///    what proves the scene's selection stuck. (The parked
///    playhead itself is timeline state: the `frame-N`
///    chips' active styling is drawn, not keyed, so the
///    park is a `--snap` fact.)
/// 12. Under `--scene three-d` (Phase L): an element
///    keyed `panel-3d-preview` exists (the 3D panel's
///    preview canvas, landed by the phase's panel
///    section). The scene was written before the canvas
///    landed and carried a census substitution (the
///    `panel-3d` root + the session state, which the
///    scene's unit test still pins); the integrator
///    upgraded this fact to the real key once the canvas
///    was in the tree. The grid and figure are
///    canvas-drawn, so they are `--snap` facts, not
///    tree facts.
/// 13. Under `--scene loop-range` (Phase L): an element
///    keyed `timeline-loop-row` exists (the timeline's
///    loop row in its set state, landed by the phase's
///    timeline section). The scene was written before
///    the row landed and carried a census substitution
///    (the loop-range state itself, which the scene's
///    unit test still pins); the integrator upgraded
///    this fact to the real key once the row was in the
///    tree. (The loop edges themselves are drawn on the
///    timeline canvas, so they are a `--snap` fact.)
pub fn layout_assertions() -> Vec<&'static str> {
    vec![
        "exactly one element keyed \"stage\"",
        "an element keyed \"app-bar\" exists",
        "an element keyed \"transport\" exists",
        "an element keyed \"timeline\" exists",
        "count of elements keyed \"frame-N\" == project frame count (5 in the demo)",
        "each \"frame-N\" key for N in 1..=frame_count occurs exactly once",
        "facts above are identical under --scene default/frame-3/selected/playing",
        "under --scene export-open: an element keyed \"export-panel\" exists and no element is keyed \"figures-panel\"",
        "under --scene fx-selected: an element keyed \"particles\" exists and \"particles-detail\" contains an element keyed \"particles-kind-prev\"",
        "under --scene graph-duplicates: an element keyed \"graph-chan-1\" exists (the second planted channel row)",
        "under --scene fx-split-mid: an element keyed \"fx-split\" exists (the Split at playhead button, mounted only with a clip selection)",
        "under --scene three-d: an element keyed \"panel-3d-preview\" exists (the 3D preview canvas — see fact 12)",
        "under --scene loop-range: an element keyed \"timeline-loop-row\" exists (the loop row in its set state — see fact 13)",

    ]
}

#[cfg(test)]
mod tests {
    use super::{
        scene_export_open, scene_fx_selected, scene_fx_split_mid, scene_graph_duplicates,
        scene_loop_range, scene_three_d, scenes,
    };
    use crate::init;
    use crate::rune_sec::workspaces::Workspace;

    /// The scene table is the headless contract: ten
    /// scenes, each name unique (a duplicate would shadow
    /// the later entry in `with_scene`'s lookup), with the
    /// Phase J scenes in their display positions, the two
    /// Phase K scenes appended after them, and the two
    /// Phase L scenes appended after those.
    #[test]
    fn scene_table_has_ten_uniquely_named_scenes() {
        let table = scenes();
        assert_eq!(table.len(), 10, "ten scenes are registered");
        let mut names: Vec<&str> = table.iter().map(|(name, _)| *name).collect();
        let total = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), total, "scene names are unique");
        assert_eq!(table[4].0, "export-open");
        assert_eq!(table[5].0, "fx-selected");
        assert_eq!(table[6].0, "graph-duplicates");
        assert_eq!(table[7].0, "fx-split-mid");
        assert_eq!(table[8].0, "three-d");
        assert_eq!(table[9].0, "loop-range");
    }

    /// The export scene reaches the Export workspace through
    /// the shell's own switch — the same end state a chip
    /// press or the palette's `go-export` command produces.
    /// (Like those real presses, the scene persists the
    /// workspace to the layout prefs on disk; the snap-test
    /// suite already lives with that shared-prefs discipline
    /// by forcing its workspace through the chips.)
    #[test]
    fn export_open_scene_lands_on_the_export_workspace() {
        let mut s = init();
        scene_export_open(&mut s);
        assert_eq!(s.workspace, Workspace::Export);
    }

    /// The fx scene adds exactly one event at the frame the
    /// playhead was on, selects it, and lands on the Effects
    /// workspace — the state the Particles panel's own Add
    /// button leaves behind.
    #[test]
    fn fx_selected_scene_adds_and_selects_one_event() {
        let mut s = init();
        let frame = s.core.current;
        let len_before = s.core.project.particles.len();
        scene_fx_selected(&mut s);
        assert_eq!(s.workspace, Workspace::Effects);
        assert_eq!(
            s.core.project.particles.len(),
            len_before + 1,
            "exactly one event was added"
        );
        assert_eq!(
            s.particle_selected,
            Some(len_before),
            "the new event is the selection"
        );
        assert_eq!(
            s.core.project.particles[len_before].frame, frame,
            "the event sits at the scene's starting frame"
        );
    }

    /// The graph scene lands on the Graph workspace with
    /// figure 0 selected and exactly two channels planted —
    /// both for figure 0, both on the RootX target, told
    /// apart by their keys — the duplicate pair the graph
    /// panel's list and index selection are exercised
    /// against. (The workspace switch persists to the
    /// layout prefs, the shared-prefs discipline the other
    /// scene tests already live with.)
    #[test]
    fn graph_duplicates_scene_plants_two_rootx_channels() {
        use stykanimate_rs::graph::ChannelTarget;
        let mut s = init();
        let len_before = s.core.project.channels.len();
        scene_graph_duplicates(&mut s);
        assert_eq!(s.workspace, Workspace::Graph);
        assert_eq!(s.core.selected_figure, 0, "figure 0 is left selected");
        let channels = &s.core.project.channels;
        assert_eq!(
            channels.len(),
            len_before + 2,
            "exactly two channels were planted"
        );
        let first = &channels[len_before];
        let second = &channels[len_before + 1];
        for ch in [first, second] {
            assert_eq!(ch.figure, 0, "both channels belong to figure 0");
            assert_eq!(
                ch.target,
                ChannelTarget::RootX,
                "both channels share the RootX target"
            );
        }
        assert_ne!(
            first.keys, second.keys,
            "the twins are distinguishable by their keys"
        );
    }

    /// The fx-split scene lands on the Effects workspace
    /// with the added clip selected and the playhead parked
    /// strictly inside the clip's span — on the 5-frame
    /// demo that is the last frame, inside the default
    /// clip's 16-frame span, and past the frame the scene
    /// started on.
    #[test]
    fn fx_split_mid_scene_selects_the_clip_and_parks_inside_it() {
        let mut s = init();
        let start_frame = s.core.current;
        let len_before = s.core.project.fx_clips.len();
        scene_fx_split_mid(&mut s);
        assert_eq!(s.workspace, Workspace::Effects);
        assert_eq!(
            s.core.project.fx_clips.len(),
            len_before + 1,
            "exactly one clip was added"
        );
        assert_eq!(
            s.fx_selected,
            Some(len_before),
            "the new clip is the selection"
        );
        let clip = &s.core.project.fx_clips[len_before].clip;
        let now = s.core.current as f32;
        assert!(
            now > clip.start && now < clip.start + clip.duration,
            "playhead {now} is strictly inside the clip's span \
             ({}, {})",
            clip.start,
            clip.start + clip.duration
        );
        assert!(
            s.core.current > start_frame,
            "the playhead actually moved off the starting frame"
        );
    }

    /// The three-d scene lands on the 3D workspace with a
    /// live fig3 session started through the panel's own
    /// start op: the camera orbited to yaw 35 / pitch 18
    /// off the default camera's 0 / 0, and joint 1
    /// selected (the start op leaves joint 0; one
    /// select-next lands on 1). This is also layout fact
    /// 12's census substitute: until the preview canvas
    /// lands, the session state is what proves the scene.
    #[test]
    fn three_d_scene_starts_a_session_orbits_and_selects_joint_1() {
        let mut s = init();
        assert!(s.core.fig3.is_none(), "no session before the scene");
        scene_three_d(&mut s);
        assert_eq!(s.workspace, Workspace::Build3D);
        let st = s.core.fig3.as_ref().expect("the scene started a session");
        assert_eq!(st.camera.yaw, 35.0, "yaw orbited by +35 degrees");
        assert_eq!(st.camera.pitch, 18.0, "pitch orbited by +18 degrees");
        assert_eq!(st.selected, 1, "joint 1 is selected");
        assert!(
            st.figure.nodes.len() > 1,
            "the session figure has a joint 1 to select"
        );
    }

    /// The loop-range scene lands on the Animate workspace
    /// with the loop range set over frames 1..=3 and
    /// exactly one channel planted for figure 0 — RootX,
    /// flat keys at exactly the range's two ends. This is
    /// also the state behind layout fact 13 (the keyed
    /// loop row renders exactly this range).
    #[test]
    fn loop_range_scene_sets_the_range_and_plants_one_channel() {
        use stykanimate_rs::graph::ChannelTarget;
        let mut s = init();
        let len_before = s.core.project.channels.len();
        scene_loop_range(&mut s);
        assert_eq!(s.workspace, Workspace::Animate);
        assert_eq!(
            s.core.loop_range,
            Some((1, 3)),
            "the loop range covers frames 1..=3"
        );
        assert_eq!(s.core.selected_figure, 0, "figure 0 is left selected");
        let channels = &s.core.project.channels;
        assert_eq!(
            channels.len(),
            len_before + 1,
            "exactly one channel was planted"
        );
        let ch = &channels[len_before];
        assert_eq!(ch.figure, 0, "the channel belongs to figure 0");
        assert_eq!(ch.target, ChannelTarget::RootX, "the channel is RootX");
        let frames: Vec<f32> = ch.keys.iter().map(|k| k.frame).collect();
        assert_eq!(
            frames,
            vec![1.0, 3.0],
            "the keys sit at exactly the loop range's ends"
        );
    }
}
