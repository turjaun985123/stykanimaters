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
//!
//! The Phase M scene keeps the same discipline:
//! `three-d-stage` reaches exactly the state `three-d`
//! reaches — Build3D workspace, a session started
//! through `EditorCore::enter_3d`, the same orbit and
//! selection — and exists under its own name because
//! the proof it feeds is different: this phase's stage
//! pass draws the 3D figure on the MAIN STAGE while the
//! Build3D workspace is active with a session live, and
//! the scene's full-window `--snap` is that drawing's
//! pixel proof (its `--tree` dump is the same Build3D
//! inventory fact 12 checks under `three-d`).
//!
//! The Phase N scenes keep the same discipline, one
//! setter each, all of them the shell's own open paths:
//! `palette-open` calls `command_palette::open_palette`
//! (the palette's own open fn, the one Ctrl+K reaches
//! through `palette_handle_key`); `menu-open` calls
//! `MenusState::toggle` with `menus::FILE` — exactly
//! the File trigger's handler in `menu_bar`;
//! `settings-open` calls
//! `settings_popover::toggle_open`, the settings-toggle
//! button's own handler; `draw-open` switches workspace
//! through `workspaces::select_workspace`, the chips'
//! op, like `export-open` before it. The first three
//! exist so the phase's restyled overlay surfaces (a
//! card palette, a card dropdown, the settings card)
//! get `--snap` pixel proofs; `draw-open` gives the
//! Draw workspace — the adoption wave's densest — its
//! first scene at all.
//!
//! The UI mission Wave 7 scenes close the audits'
//! theme/size coverage gap (S19 §6, S03 C2) as far as
//! the scene mechanism reaches — and the boundary is
//! worth recording exactly:
//!
//! - **Theme is state, so it is a scene.** The shell's
//!   theme lives in the pub `EditorCore.light_theme`
//!   field (`editor.rs:63`), and the settings card's
//!   Theme buttons set it through
//!   `settings_popover::set_theme` — so a scene flips
//!   it through that very call. `default-light` and
//!   `settings-open-light` are the two light scenes.
//!   (In the running app `init()` seeds the field from
//!   the layout prefs and `poll` persists flips back;
//!   a scene runs after `init`, so its flip is what
//!   the snap renders, and the headless probe wipes
//!   `./projects` per run, so nothing leaks between
//!   probes.)
//! - **Window size is NOT state, so it cannot be a
//!   scene.** A scene fn receives only `&mut State`
//!   (tooling.rs:25, quoted above); the headless
//!   runtime takes its viewport from the `--size WxH`
//!   CLI flag, falling back to the AppSpec size —
//!   `let size = args.size.unwrap_or(app.spec.size)`
//!   (`crates/rune/src/tooling/headless.rs:81`; the
//!   flag itself is `args.rs:139`) — and the spec size
//!   is fixed at (1440, 900) in `main.rs`'s AppSpec
//!   literal. No scene can reach it. The 1280×720
//!   variants the audits ask for are therefore PROBE
//!   PAIRINGS, not scenes: `--scene default --size
//!   1280x720` (dark, minimum window) and `--scene
//!   default-light --size 1280x720` (the combined
//!   case). Wiring those invocations into the
//!   headless workflow is an integrator step; the
//!   scene half of each pairing exists in this table.
//!
//! The UI mission Wave 9 scenes close the remaining
//! workspace coverage gap: after them, every one of the
//! eight workspaces has at least one scene that lands on
//! it. Five switches are appended — `home`, `build-open`,
//! `graph-open`, `effects-open`, `audio-open` — each a
//! bare `workspaces::select_workspace` call, the chips'
//! op, exactly like `export-open` and `draw-open` before
//! them; those two already existed, so they are not
//! duplicated. The new five are deliberately BARE
//! switches: `build-open` proves the Build3D gating
//! mounts the 3D panel with NO session live (where
//! `three-d` proves it with one), `graph-open` the graph
//! panel with no channels planted (where
//! `graph-duplicates` proves it with two), and
//! `effects-open` the FX browser with nothing added
//! (where `fx-selected` / `fx-split-mid` prove the
//! populated states). `home` gives the Home workspace —
//! the project library every launch can land on — its
//! first scene at all.
//!
//! The Wave 10 scene gives the timeline's new dopesheet
//! lane a populated state: `dopesheet-keys` authors real
//! keyframes on two figures, mostly through the core's
//! own keyframe ops — `EditorCore::delete_keyframe`
//! (the keyframe tools' hold-previous op) flattens the
//! demo wave's extra keys, `EditorCore::add_figure`
//! adds the second figure, and
//! `EditorCore::nudge_path_point` (the motion-path
//! primitive) authors its one change. It carries the
//! module's named-exception pattern twice, each time
//! because the op set has a real gap: no op removes one
//! figure's pose from one frame (the scene truncates
//! the trailing slot — the absence state
//! `stats::figure_keyframes` documents), and no op
//! makes a NON-key frame repeat a chosen earlier pose
//! (`delete_keyframe` only holds the immediately
//! previous pose over an existing key), so the scene's
//! last repeat is a direct `poses` assignment. The
//! scene's unit test pins the resulting census through
//! `stats::figure_keyframes` itself — the lane's own
//! input.

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

/// `"three-d-stage"` — the SAME end state as `three-d`
/// (Build3D workspace, a live fig3 session entered
/// through `EditorCore::enter_3d`, the camera orbited
/// yaw +35° / pitch +18°, joint 1 selected), reached
/// through the same real ops in the same order. The
/// separate name exists for the proof, not the state:
/// Phase M's stage pass draws the 3D figure on the
/// MAIN STAGE while the Build3D workspace is active
/// with a session live, so this scene's full-window
/// `--snap` is the pixel proof of that drawing (under
/// `three-d` the snaps exercised the panel preview).
/// Its `--tree` dump carries the same Build3D panel
/// inventory as `three-d`'s (fact 14 mirrors fact 12).
fn scene_three_d_stage(s: &mut State) {
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

/// `"palette-open"` — the command palette open over the
/// default Animate landing, so a `--snap` shows the
/// Phase N card-styled overlay (the palette mounts in
/// the centre column while open) and a `--tree` dump
/// can prove the overlay mounted (fact 15). The open
/// goes through `command_palette::open_palette` — the
/// palette's own open fn, the same call
/// `palette_handle_key` makes on Ctrl+K: it sets the
/// pub `open` flag, clears the query, selects the first
/// row and focuses the field.
fn scene_palette_open(s: &mut State) {
    crate::rune_sec::command_palette::open_palette(s);
}

/// `"menu-open"` — the File menu's dropdown open, so a
/// `--snap` shows the Phase N card dropdown under its
/// trigger (and the scrim state around it) and a
/// `--tree` dump can prove both mounted (fact 16). The
/// toggle is `MenusState::toggle` with `menus::FILE` —
/// exactly the File trigger's own handler (`menu_bar`
/// wires every trigger to `s.menus.toggle(index)`),
/// and from the fresh state (nothing open) it lands on
/// `Some(FILE)`.
fn scene_menu_open(s: &mut State) {
    s.menus.toggle(crate::rune_sec::menus::FILE);
}

/// `"settings-open"` — the settings card expanded at
/// the top of the right column, so a `--snap` shows
/// the Phase N card with its toggle pills and speed
/// chips and a `--tree` dump can prove the card
/// mounted (fact 17). The flip is
/// `settings_popover::toggle_open` — the
/// settings-toggle button's own handler — and from the
/// fresh state (card closed) it lands open.
fn scene_settings_open(s: &mut State) {
    crate::rune_sec::settings_popover::toggle_open(s);
}

/// `"draw-open"` — the shell switched to the Draw
/// workspace, so the snap renders the drawing set
/// (draw tools, stroke inspector, text tool) with the
/// Animate-only panels gated out. The switch is
/// `workspaces::select_workspace` — the exact call the
/// workspace chips and the palette's `go-*` commands
/// make, as in `export-open`. Draw had no scene before
/// Phase N: it is the adoption wave's densest
/// workspace and had no pixel proof.
fn scene_draw_open(s: &mut State) {
    crate::rune_sec::workspaces::select_workspace(
        s,
        crate::rune_sec::workspaces::Workspace::Draw,
    );
}

/// `"default-light"` — the default shell (the demo
/// project exactly as `scene_default` leaves it) in
/// the LIGHT theme, so a `--snap` gives the light
/// shell its first pixel proof and a `--tree` dump
/// proves the theme changes paint, not structure
/// (fact 19). The flip is
/// `settings_popover::set_theme` — the exact call
/// the settings card's Light button makes — writing
/// the core's real `light_theme` field; every themed
/// surface downstream reads that field through
/// `app_theme(!core.light_theme)` in `main.rs::view`.
/// Nothing else is touched: no workspace switch, no
/// selection, no prefs write (the scene runs after
/// `init` seeded the field from the prefs, and its
/// write is the one the snap renders).
fn scene_default_light(s: &mut State) {
    crate::rune_sec::settings_popover::set_theme(&mut s.core, true);
}

/// `"settings-open-light"` — the settings card
/// expanded (as in `settings-open`) in the LIGHT
/// theme: the shell's longest card is also its most
/// token-dense surface (theme pills, the language
/// grid, stage steppers, speed chips), so its light
/// rendering is the strictest single-snap check the
/// light theme gets (fact 20). Both halves go
/// through the shell's own paths, in the order a
/// user would reach them: the Theme flip through
/// `settings_popover::set_theme` (the Light
/// button's call), then the card flip through
/// `settings_popover::toggle_open` (the
/// settings-toggle button's handler) — from the
/// fresh state (card closed) the toggle lands open.
fn scene_settings_open_light(s: &mut State) {
    crate::rune_sec::settings_popover::set_theme(&mut s.core, true);
    crate::rune_sec::settings_popover::toggle_open(s);
}

/// `"home"` — the shell switched to the Home
/// workspace, so the snap renders the project
/// library (`home-panel`) with every editing panel
/// gated out. The switch is
/// `workspaces::select_workspace` — the exact call
/// the workspace chips and the palette's `go-*`
/// commands make, as in `export-open`. Home had no
/// scene before UI mission Wave 9: it is the
/// workspace a launch can land on (the layout prefs
/// restore it), yet it had no pixel proof at all.
fn scene_home(s: &mut State) {
    crate::rune_sec::workspaces::select_workspace(
        s,
        crate::rune_sec::workspaces::Workspace::Home,
    );
}

/// `"build-open"` — the shell switched to the
/// Build3D workspace with NO fig3 session started,
/// so the snap renders the 3D panel in its
/// no-session state (its start control and guidance,
/// not the preview session `three-d` exercises).
/// The switch is `workspaces::select_workspace`
/// alone — deliberately no `enter_3d`: this scene
/// proves the workspace gating mounts the panel,
/// where `three-d` proves the session state behind
/// it.
fn scene_build_open(s: &mut State) {
    crate::rune_sec::workspaces::select_workspace(
        s,
        crate::rune_sec::workspaces::Workspace::Build3D,
    );
}

/// `"graph-open"` — the shell switched to the Graph
/// workspace with no channels planted, so the snap
/// renders the graph panel in its empty state (its
/// no-channels guidance, not the populated channel
/// list `graph-duplicates` exercises). The switch
/// is `workspaces::select_workspace` alone, as in
/// `export-open`.
fn scene_graph_open(s: &mut State) {
    crate::rune_sec::workspaces::select_workspace(
        s,
        crate::rune_sec::workspaces::Workspace::Graph,
    );
}

/// `"effects-open"` — the shell switched to the
/// Effects workspace with nothing added, so the
/// snap renders the FX browser and lanes in their
/// empty states (not the populated states
/// `fx-selected` and `fx-split-mid` exercise). The
/// switch is `workspaces::select_workspace` alone,
/// as in `export-open`.
fn scene_effects_open(s: &mut State) {
    crate::rune_sec::workspaces::select_workspace(
        s,
        crate::rune_sec::workspaces::Workspace::Effects,
    );
}

/// `"audio-open"` — the shell switched to the Audio
/// workspace, so the snap renders the audio panel
/// (`audio-panel`) with the Animate panels gated
/// out. The switch is
/// `workspaces::select_workspace`, as in
/// `export-open`. Audio had no scene before UI
/// mission Wave 9 — it was the last editing
/// workspace with no pixel proof.
fn scene_audio_open(s: &mut State) {
    crate::rune_sec::workspaces::select_workspace(
        s,
        crate::rune_sec::workspaces::Workspace::Audio,
    );
}

/// `"clips-populated"` — the Animate workspace with
/// two movieclips built from the demo figure and an
/// edit session open on the first (Wave 9
/// integration). Until this scene existed, NO headless
/// tree contained a clip row: the clips panel's rows
/// were the one panel surface the overflow scanner had
/// never seen, which is exactly how a 108–188 pt row
/// overflow survived Waves 1–8 (C9's finding). The
/// clips are real (`create_clip_from_figure` over the
/// demo project's figure 0); the second is renamed so
/// the two rows differ; `enter_clip` opens the session
/// so the session section and the row's Editing marker
/// render too.
fn scene_clips_populated(s: &mut State) {
    crate::rune_sec::workspaces::select_workspace(
        s,
        crate::rune_sec::workspaces::Workspace::Animate,
    );
    if s.core.create_clip_from_figure(0).is_some() {
        if s.core.create_clip_from_figure(0).is_some() {
            if let Some(second) = s.core.project.clips.get_mut(1) {
                second.name = "Second clip".to_string();
            }
        }
        s.core.enter_clip(0);
    }
}

/// `"dopesheet-keys"` — the Animate workspace with real
/// keyframes authored on TWO figures, so the snap/tree
/// renders the timeline's dopesheet lane (Wave 10) with
/// keyed cells for both figures instead of an empty or
/// single-figure lane. The census the lane reads is
/// `stats::figure_keyframes`: a frame keys when the
/// figure has a pose there that differs from the
/// previous frame's (root moved beyond 0.5 units or any
/// angle beyond 0.01°); frame 0 keys whenever a pose is
/// present; a figure MISSING from a frame (poses are
/// positional — a shorter `poses` vec) never keys, and
/// its reappearance does. The scene authors exactly
/// this census: figure 0 keyed at frames 0, 2, 4;
/// figure 1 keyed at frames 1, 3; the playhead parked
/// on frame 2 (a figure-0 key, with both figures posed
/// there).
///
/// The starting point fights back: the demo has ONE
/// figure, and its built-in wave changes the pose on
/// EVERY frame, so the raw demo keys figure 0 at all
/// five frames. The recipe, real ops first:
/// 1. Figure 0's unwanted keys at frames 1 and 3 are
///    deleted through the real op
///    `EditorCore::delete_keyframe` — the keyframe
///    tools' hold-previous op, which replaces the
///    frame's pose with the previous frame's (bends
///    held too). Both frames are genuine keys in the
///    raw demo, so neither refusal branch (frame 0,
///    non-key, appearance) can fire. The demo's own
///    wave then supplies the surviving changes at
///    frames 2 and 4 — root 28 units and ≈57° off the
///    new predecessors, far beyond the epsilons.
/// 2. Figure 1 is added through the real op
///    `EditorCore::add_figure`, which plants the same
///    rest pose on every frame (its census would read
///    {0} as-is).
/// 3. Figure 1 is made MISSING from frame 0 by
///    truncating that frame's `poses` vec to the first
///    figure — a direct write, and the recipe's first
///    named exception: no core op removes one figure's
///    pose from one frame, and for a TRAILING figure
///    the shorter vec IS the absence state
///    `figure_keyframes` documents. Frame 1 keeps the
///    rest pose and keys as the reappearance; frame 2
///    repeats it and does not key.
/// 4. Figure 1's frame-3 key is authored through the
///    real op `EditorCore::nudge_path_point` — the
///    motion-path primitive (move one figure's root at
///    one frame, nothing else touched) — +36 units on
///    x, far beyond the 0.5-unit epsilon. Frame 4 must
///    then repeat frame 3's pose: the second named
///    exception, a direct assignment, because
///    `delete_keyframe` only holds the immediately
///    previous pose over an EXISTING key and frame 4
///    holds no key to delete.
/// The playhead parks through `EditorCore::goto`, the
/// timeline chips' op.
fn scene_dopesheet_keys(s: &mut State) {
    crate::rune_sec::workspaces::select_workspace(
        s,
        crate::rune_sec::workspaces::Workspace::Animate,
    );
    let demo_shaped = {
        let p = &s.core.project;
        !p.figures.is_empty()
            && p.frames.len() >= 5
            && p.frames.iter().all(|f| !f.poses.is_empty())
    };
    if !demo_shaped {
        return;
    }
    // Step 1: figure 0's repeats, through the real
    // hold-previous op (see the doc comment).
    s.core.delete_keyframe(0, 1);
    s.core.delete_keyframe(0, 3);
    // Step 2: the second figure, through the real op.
    s.core.add_figure();
    if s.core.project.figures.len() < 2 {
        return;
    }
    // Step 3: figure 1 missing from frame 0 — the
    // trailing slot's absence is the shorter vec.
    if s.core.project.frames[0].poses.len() > 1 {
        s.core.project.frames[0].poses.truncate(1);
    }
    // Step 4: the frame-3 key through the real op,
    // then frame 4 repeats it (direct assignment —
    // the doc comment's second exception).
    s.core.nudge_path_point(3, 1, [36.0, 0.0]);
    if s.core.project.frames[3].poses.len() > 1 && s.core.project.frames[4].poses.len() > 1 {
        let keyed = s.core.project.frames[3].poses[1].clone();
        s.core.project.frames[4].poses[1] = keyed;
    }
    s.core.goto(2);
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
        ("three-d-stage", scene_three_d_stage),
        ("palette-open", scene_palette_open),
        ("menu-open", scene_menu_open),
        ("settings-open", scene_settings_open),
        ("draw-open", scene_draw_open),
        ("default-light", scene_default_light),
        ("settings-open-light", scene_settings_open_light),
        ("home", scene_home),
        ("build-open", scene_build_open),
        ("graph-open", scene_graph_open),
        ("effects-open", scene_effects_open),
        ("audio-open", scene_audio_open),
        ("clips-populated", scene_clips_populated),
        ("dopesheet-keys", scene_dopesheet_keys),
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
/// 14. Under `--scene three-d-stage` (Phase M): the same
///    Build3D tree as fact 12 — an element keyed
///    `panel-3d` exists (the 3D panel root the fact-12
///    census named) and an element keyed
///    `panel-3d-preview` exists (the preview canvas
///    itself), because the scene mounts the same panels
///    as `three-d`. The scene's actual point is NOT the
///    tree but the `--snap`: with the Build3D workspace
///    active and a session live, the stage pass draws
///    the 3D figure on the MAIN STAGE, and only the PNG
///    proves that drawing happened.
/// 15. Under `--scene palette-open` (Phase N): an element
///    keyed `command-palette` exists (the palette overlay
///    surface) and an element keyed `palette-query`
///    exists (its search field). The view mounts the
///    palette in the centre column only while the open
///    flag is set, so either key missing means the
///    scene's open call or the view's gate regressed.
///    (The card's styling is a `--snap` fact, not a tree
///    fact.)
/// 16. Under `--scene menu-open` (Phase N): an element
///    keyed `menu-file-dropdown` exists (the File
///    dropdown surface), an element keyed `menu-scrim`
///    exists (the click-away layer the open menu mounts
///    with it), and an element keyed `export` exists —
///    the File menu's Export PNG item, which is built
///    only inside the open dropdown, so its presence
///    proves the dropdown's contents mounted, not just
///    its shell.
/// 17. Under `--scene settings-open` (Phase N): an element
///    keyed `settings-card` exists (the expanded
///    settings card). The `settings` shell element
///    exists under EVERY scene (the toggle is always
///    mounted), so only the card's presence proves the
///    scene's open flip landed. (The pills and chips
///    inside are a `--snap` fact.)
/// 18. Under `--scene draw-open` (Phase N): an element
///    keyed `stroke-inspector` exists (the Draw set's
///    stroke panel) and NO element is keyed `frames`
///    (FrameOps is Animate-only, not Draw) —
///    fact 8's shape with the pair swapped: the Draw
///    gating mounted the drawing set and left the frame
///    tools out.
/// 19. Under `--scene default-light` (UI mission
///    Wave 7): the element inventory is IDENTICAL to
///    the default tree's (facts 1–6, key for key) —
///    the theme is a paint property of the same
///    elements, never structure, so any key drift
///    against `default` means a view started
///    branching on the theme. The light rendering
///    itself is a `--snap` fact, not a tree fact:
///    the snap's background pixels sit in the light
///    range (the light `bg` is `#EDF0F5`) where the
///    default snap's sit in the dark range.
/// 20. Under `--scene settings-open-light` (UI
///    mission Wave 7): an element keyed
///    `settings-card` exists — fact 17's key proof,
///    under the light theme — and the rest of the
///    inventory matches `--scene settings-open`'s.
///    The card's light rendering (pills, language
///    grid, chips on the light card surface) is a
///    `--snap` fact.
/// 21. Under `--scene home` (UI mission Wave 9): an
///    element keyed `home-panel` exists (the project
///    library root) and NO element is keyed
///    `figures-panel` — fact 8's shape: the Home
///    gating mounted the library and left the editing
///    panels out.
/// 22. Under `--scene build-open` (UI mission
///    Wave 9): an element keyed `panel-3d` exists
///    (the 3D panel root, as in fact 14) and an element
///    IS keyed `figures-panel` (the Figures section
///    mounts in every workspace except Home/Export —
///    figure selection is global editor state; Wave 9
///    audit E9). Unlike
///    fact 12 there is NO session here, so no
///    preview-canvas key is asserted: this fact
///    proves the workspace switch alone mounts the
///    panel.
/// 23. Under `--scene graph-open` (UI mission
///    Wave 9): an element keyed `graph-panel` exists
///    (the graph panel root) and an element IS keyed
///    `figures-panel` (see fact 22). No channel rows
///    are asserted: the scene plants none, where
///    fact 10's scene plants two.
/// 24. Under `--scene effects-open` (UI mission
///    Wave 9): an element keyed `fx-browser` exists
///    (the FX browser root) and an element IS keyed
///    `figures-panel` — the Effects gating with
///    nothing added, where fact 9's scene adds and
///    selects an event.
/// 25. Under `--scene audio-open` (UI mission
///    Wave 9): an element keyed `audio-panel` exists
///    (the audio panel root) and an element IS keyed
///    `figures-panel` — the last workspace that had
///    no scene.
/// 26. Under `--scene clips-populated` (Wave 9
///    integration): elements keyed `clip-dup-0` and
///    `clip-dup-1` exist — two real clip rows in the
///    clips panel, the surface no earlier scene
///    populated — and the open edit session mounts
///    its section under the list.
/// 27. Under `--scene dopesheet-keys` (Wave 10): the
///    timeline's dopesheet lane — root element keyed
///    `dopesheet-lane`, figure header cells keyed
///    `dope-row-{fi}`, frame cells keyed
///    `dope-cell-{fi}-{frame}` (0-based) — carries TWO
///    figure rows with keyed cells at exactly the
///    scene's authored census — figure 0 keyed at
///    frames 0, 2 and 4 (3 keys), figure 1 keyed at
///    frames 1 and 3 (2 keys), no other cell keyed —
///    counted from `stats::figure_keyframes`, the
///    census the lane renders from. (The playhead's
///    park on frame 2 is chip styling — drawn, not
///    keyed — so it is a `--snap` fact, as in fact 11.)
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
        "under --scene three-d-stage: an element keyed \"panel-3d\" exists and an element keyed \"panel-3d-preview\" exists (the 3D panel, as under three-d — see fact 14)",
        "under --scene palette-open: an element keyed \"command-palette\" exists and an element keyed \"palette-query\" exists (the palette overlay, mounted only while its open flag is set — see fact 15)",
        "under --scene menu-open: an element keyed \"menu-file-dropdown\" exists, an element keyed \"menu-scrim\" exists and an element keyed \"export\" exists (the File dropdown, its scrim and its contents — see fact 16)",
        "under --scene settings-open: an element keyed \"settings-card\" exists (the expanded settings card; the \"settings\" shell exists under every scene — see fact 17)",
        "under --scene draw-open: an element keyed \"stroke-inspector\" exists and no element is keyed \"frames\" (the Draw set, with FrameOps gated out — see fact 18)",
        "under --scene default-light: the element inventory is identical to the default tree's (theme is paint, not structure — see fact 19)",
        "under --scene settings-open-light: an element keyed \"settings-card\" exists and the inventory matches settings-open's (see fact 20)",
        "under --scene home: an element keyed \"home-panel\" exists and no element is keyed \"figures-panel\" (the project library, with the editing panels gated out — see fact 21)",
        "under --scene build-open: an element keyed \"panel-3d\" exists and an element is keyed \"figures-panel\" (the 3D panel mounted by the switch alone, no session; Figures mounts outside Home/Export — see fact 22)",
        "under --scene graph-open: an element keyed \"graph-panel\" exists and an element is keyed \"figures-panel\" (the graph panel with no channels planted — see fact 23)",
        "under --scene effects-open: an element keyed \"fx-browser\" exists and an element is keyed \"figures-panel\" (the FX browser with nothing added — see fact 24)",
        "under --scene audio-open: an element keyed \"audio-panel\" exists and an element is keyed \"figures-panel\" (the audio panel — see fact 25)",
        "under --scene clips-populated: elements keyed \"clip-dup-0\" and \"clip-dup-1\" exist (two real clip rows — see fact 26)",
        "under --scene dopesheet-keys: the dopesheet lane shows two figure rows, figure 0 keyed at frames 0, 2, 4 (3 keys) and figure 1 keyed at frames 1, 3 (2 keys), no other cell keyed (the stats::figure_keyframes census — see fact 27)",
    ]
}

#[cfg(test)]
mod tests {
    use super::{
        scene_audio_open, scene_build_open, scene_clips_populated, scene_default_light,
        scene_dopesheet_keys, scene_draw_open,
        scene_effects_open, scene_export_open, scene_fx_selected, scene_fx_split_mid,
        scene_graph_duplicates, scene_graph_open, scene_home, scene_loop_range,
        scene_menu_open, scene_palette_open, scene_settings_open,
        scene_settings_open_light, scene_three_d, scene_three_d_stage, scenes,
    };
    use crate::init;
    use crate::rune_sec::workspaces::Workspace;

    /// The scene table is the headless contract:
    /// twenty-four scenes, each name unique (a duplicate
    /// would shadow the later entry in `with_scene`'s
    /// lookup), with the Phase J scenes in their display
    /// positions, the two Phase K scenes appended after
    /// them, the two Phase L scenes appended after those,
    /// the Phase M scene appended after them, the four
    /// Phase N scenes appended after those, the two
    /// UI-mission Wave 7 light scenes appended after
    /// those, the five UI-mission Wave 9 workspace
    /// scenes appended after those, the Wave 9
    /// integration clips-populated scene appended after
    /// those, and the Wave 10 dopesheet-keys scene last.
    #[test]
    fn scene_table_has_twentyfour_uniquely_named_scenes() {
        let table = scenes();
        assert_eq!(table.len(), 24, "twenty-four scenes are registered");
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
        assert_eq!(table[10].0, "three-d-stage");
        assert_eq!(table[11].0, "palette-open");
        assert_eq!(table[12].0, "menu-open");
        assert_eq!(table[13].0, "settings-open");
        assert_eq!(table[14].0, "draw-open");
        assert_eq!(table[15].0, "default-light");
        assert_eq!(table[16].0, "settings-open-light");
        assert_eq!(table[17].0, "home");
        assert_eq!(table[18].0, "build-open");
        assert_eq!(table[19].0, "graph-open");
        assert_eq!(table[20].0, "effects-open");
        assert_eq!(table[21].0, "audio-open");
        assert_eq!(table[22].0, "clips-populated");
        assert_eq!(table[23].0, "dopesheet-keys");
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

    /// The three-d-stage scene lands on exactly the end
    /// state `three-d` lands on — the Build3D workspace
    /// with a live fig3 session, the camera orbited to
    /// yaw 35 / pitch 18 and joint 1 selected — because
    /// the stage pass it exercises gates on precisely
    /// that state (Build3D active + session live). This
    /// is also the state behind layout fact 14 (the same
    /// 3D panel inventory as `three-d` mounts).
    #[test]
    fn three_d_stage_scene_starts_a_session_on_build3d() {
        let mut s = init();
        assert!(s.core.fig3.is_none(), "no session before the scene");
        scene_three_d_stage(&mut s);
        assert_eq!(s.workspace, Workspace::Build3D);
        let st = s
            .core
            .fig3
            .as_ref()
            .expect("the scene started a session");
        assert_eq!(st.camera.yaw, 35.0, "yaw orbited by +35 degrees");
        assert_eq!(st.camera.pitch, 18.0, "pitch orbited by +18 degrees");
        assert_eq!(st.selected, 1, "joint 1 is selected");
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

    /// The palette scene opens the palette through its
    /// own open fn: the open flag set, the query cleared
    /// and the first row selected — the state Ctrl+K
    /// leaves behind. This is also the state behind
    /// layout fact 15 (the overlay mounts only while the
    /// flag is set).
    #[test]
    fn palette_open_scene_opens_the_palette_with_a_cleared_query() {
        let mut s = init();
        assert!(!s.palette.open, "the palette starts closed");
        scene_palette_open(&mut s);
        assert!(s.palette.open, "the palette is open");
        assert!(
            s.palette.query.text.is_empty(),
            "the query starts cleared"
        );
        assert_eq!(s.palette.selected, 0, "the first row is selected");
    }

    /// The menu scene opens the File menu through the
    /// trigger's own toggle: from the fresh state
    /// (nothing open) the toggle lands on `Some(FILE)` —
    /// the state behind layout fact 16 (the dropdown and
    /// its scrim mount only while a menu is open).
    #[test]
    fn menu_open_scene_opens_the_file_menu() {
        let mut s = init();
        assert_eq!(s.menus.open, None, "no menu starts open");
        scene_menu_open(&mut s);
        assert_eq!(
            s.menus.open,
            Some(crate::rune_sec::menus::FILE),
            "the File menu is open"
        );
    }

    /// The settings scene expands the settings card
    /// through the toggle button's own handler: from the
    /// fresh state (card closed) the flip lands open —
    /// the state behind layout fact 17 (only the card,
    /// not the always-mounted shell, proves it).
    #[test]
    fn settings_open_scene_expands_the_settings_card() {
        let mut s = init();
        assert!(!s.settings.open, "the settings card starts closed");
        scene_settings_open(&mut s);
        assert!(s.settings.open, "the settings card is expanded");
    }

    /// The draw scene reaches the Draw workspace through
    /// the shell's own switch — the same end state a chip
    /// press or the palette's `go-draw` command produces.
    /// (Like those real presses, the scene persists the
    /// workspace to the layout prefs on disk; the
    /// snap-test suite already lives with that
    /// shared-prefs discipline.)
    #[test]
    fn draw_open_scene_lands_on_the_draw_workspace() {
        let mut s = init();
        scene_draw_open(&mut s);
        assert_eq!(s.workspace, Workspace::Draw);
    }

    /// The home scene reaches the Home workspace
    /// through the shell's own switch — the same end
    /// state a chip press or the palette's `go-home`
    /// command produces. This is the state behind
    /// layout fact 21 (the library mounted, the
    /// editing panels gated out). (The switch
    /// persists the workspace to the layout prefs,
    /// the shared-prefs discipline the other scene
    /// tests already live with.)
    #[test]
    fn home_scene_lands_on_the_home_workspace() {
        let mut s = init();
        scene_home(&mut s);
        assert_eq!(s.workspace, Workspace::Home);
    }

    /// The build-open scene reaches the Build3D
    /// workspace through the switch ALONE: no fig3
    /// session is started — the deliberate contrast
    /// with `three-d`, whose session this test's
    /// sibling pins. This is the state behind layout
    /// fact 22 (the panel mounted by the switch,
    /// with no session behind it).
    #[test]
    fn build_open_scene_lands_on_build3d_without_a_session() {
        let mut s = init();
        assert!(s.core.fig3.is_none(), "no session before the scene");
        scene_build_open(&mut s);
        assert_eq!(s.workspace, Workspace::Build3D);
        assert!(
            s.core.fig3.is_none(),
            "the bare switch starts no session"
        );
    }

    /// The graph-open scene reaches the Graph
    /// workspace through the switch ALONE: no
    /// channels are planted — the deliberate
    /// contrast with `graph-duplicates`, which
    /// plants two. This is the state behind layout
    /// fact 23 (the panel in its empty state).
    #[test]
    fn graph_open_scene_lands_on_graph_without_channels() {
        let mut s = init();
        let len_before = s.core.project.channels.len();
        scene_graph_open(&mut s);
        assert_eq!(s.workspace, Workspace::Graph);
        assert_eq!(
            s.core.project.channels.len(),
            len_before,
            "the bare switch plants no channels"
        );
    }

    /// The effects-open scene reaches the Effects
    /// workspace through the switch ALONE: no
    /// particle event is added and no FX clip is
    /// created — the deliberate contrast with
    /// `fx-selected` and `fx-split-mid`. This is
    /// the state behind layout fact 24 (the browser
    /// in its empty state).
    #[test]
    fn effects_open_scene_lands_on_effects_with_nothing_added() {
        let mut s = init();
        let particles_before = s.core.project.particles.len();
        let clips_before = s.core.project.fx_clips.len();
        scene_effects_open(&mut s);
        assert_eq!(s.workspace, Workspace::Effects);
        assert_eq!(
            s.core.project.particles.len(),
            particles_before,
            "the bare switch adds no particle event"
        );
        assert_eq!(
            s.core.project.fx_clips.len(),
            clips_before,
            "the bare switch adds no FX clip"
        );
        assert_eq!(s.particle_selected, None, "nothing is selected");
        assert_eq!(s.fx_selected, None, "nothing is selected");
    }

    /// The audio-open scene reaches the Audio
    /// workspace through the shell's own switch —
    /// the same end state a chip press or the
    /// palette's `go-audio` command produces. This
    /// is the state behind layout fact 25.
    #[test]
    fn audio_open_scene_lands_on_the_audio_workspace() {
        let mut s = init();
        scene_audio_open(&mut s);
        assert_eq!(s.workspace, Workspace::Audio);
    }

    /// The clips-populated scene (Wave 9 integration)
    /// builds two real clips from the demo figure and
    /// opens the edit session on the first — the state
    /// behind layout fact 26, and the first scene whose
    /// tree contains clip rows at all.
    #[test]
    fn clips_populated_scene_builds_two_clips_and_opens_a_session() {
        let mut s = init();
        scene_clips_populated(&mut s);
        assert_eq!(s.workspace, Workspace::Animate);
        assert_eq!(s.core.project.clips.len(), 2, "two clips are built");
        assert_eq!(s.core.project.clips[1].name, "Second clip");
        assert_eq!(
            s.core.clip_edit.as_ref().map(|session| session.clip),
            Some(0),
            "the edit session is open on the first clip"
        );
    }

    /// The dopesheet-keys scene (Wave 10) authors the
    /// exact keyframe census the dopesheet lane renders,
    /// proven through the lane's own input —
    /// `stats::figure_keyframes`: figure 0 keyed at
    /// frames 0, 2, 4 (the demo wave's changes at 2 and
    /// 4 survive; frames 1 and 3 were held to their
    /// predecessors by `delete_keyframe`), figure 1
    /// keyed at frames 1 and 3 (missing from frame 0,
    /// appearing at 1, repeated at 2 and 4, changed at
    /// 3 by the path nudge), the frame count untouched,
    /// and the playhead parked on frame 2 — a figure-0
    /// key with both figures posed. This is the state
    /// behind layout fact 27.
    #[test]
    fn dopesheet_keys_scene_authors_the_two_figure_census() {
        let mut s = init();
        scene_dopesheet_keys(&mut s);
        assert_eq!(s.workspace, Workspace::Animate);
        assert_eq!(s.core.project.figures.len(), 2, "two figures are present");
        assert_eq!(s.core.project.frames.len(), 5, "the frame count is untouched");
        assert_eq!(
            crate::stats::figure_keyframes(&s.core.project, 0),
            vec![0, 2, 4],
            "figure 0 keys at frames 0, 2, 4"
        );
        assert_eq!(
            crate::stats::figure_keyframes(&s.core.project, 1),
            vec![1, 3],
            "figure 1 keys at frames 1, 3"
        );
        assert_eq!(s.core.current, 2, "the playhead parks on a keyed frame");
    }

    /// The default-light scene flips ONLY the theme,
    /// through the settings card's own setter: after
    /// it, the core's `light_theme` is set (whatever
    /// `init` seeded from the layout prefs), and the
    /// rest of the landing state is untouched — the
    /// workspace stays whatever `init` produced it
    /// as (the layout prefs may restore one; the
    /// scene must not move it), frame 0, nothing
    /// playing, the palette and settings card closed.
    /// This is the state behind layout fact 19 (the
    /// light tree's inventory equals the default's).
    #[test]
    fn default_light_scene_flips_only_the_theme() {
        let mut s = init();
        let workspace_before = s.workspace;
        scene_default_light(&mut s);
        assert!(s.core.light_theme, "the light theme is set");
        assert_eq!(s.workspace, workspace_before, "the workspace is unmoved");
        assert_eq!(s.core.current, 0, "the playhead stays on frame 0");
        assert!(!s.core.playing, "playback stays stopped");
        assert!(!s.palette.open, "the palette stays closed");
        assert!(!s.settings.open, "the settings card stays closed");
    }

    /// The settings-open-light scene reaches the
    /// settings card's own open state in the light
    /// theme: both flips land — `light_theme` set and
    /// `settings.open` set — the state behind layout
    /// fact 20 (the card mounted, in light).
    #[test]
    fn settings_open_light_scene_opens_the_card_in_light() {
        let mut s = init();
        assert!(!s.settings.open, "the settings card starts closed");
        scene_settings_open_light(&mut s);
        assert!(s.core.light_theme, "the light theme is set");
        assert!(s.settings.open, "the settings card is expanded");
    }
}
