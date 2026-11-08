//! Runtime interaction tests for the Rune editor shell
//! (Phase D, section 36 — the preservation inventory made
//! executable).
//!
//! These tests drive the REAL shell — `crate::view`, the
//! AppSpec `poll`/`on_key` hooks and `crate::init` — through
//! rune's in-process `Runtime` (docs/RUNE_API_AUDIT.md Q8): no
//! window, no GPU, a deterministic virtual clock. Pointer input
//! is queued by `pointer_down`/`pointer_move`/`pointer_up` and
//! lands on the next `rt.frame(..)`, exactly as the worked test
//! in rune's `examples/hello` does it.
//!
//! This file is test-only. The integrator declares it from
//! `main.rs` (see the section report for the exact line); it is
//! never compiled into the shipped binary.
//!
//! API notes vs the audit (verified against the pinned rune
//! source, `crates/rune/src/runtime*`):
//! * `Runtime::new(state, view, viewport, env)` leaves the
//!   `poll` and `on_key` fields `None`; both are `pub`, and the
//!   tests set them to the app's real hooks so playback runs
//!   through the same code path as the windowed app.
//! * `Runtime::presented(key)` returns
//!   `Option<([f32; 4], f32)>` — drawn rect `[x, y, w, h]` in
//!   window points plus effective opacity.
//! * `Runtime::find(key)` returns the element's drawn centre,
//!   which is what the taps below aim at.
//!
//! Round 2 (Phase E, section 37) adds four tests over the
//! Phase D surfaces — menus, the command palette, the
//! settings theme buttons and the workspace chips. New API
//! facts they rest on, likewise verified in the pinned
//! source:
//! * `Runtime::key(ev)` queues a key event (dispatched on
//!   the next `frame`), and `Runtime::state` is a `pub`
//!   field — the tests read section state (`state.menus`,
//!   `state.palette`, `state.workspace`, …) straight off
//!   it, the same way round 1 read `state.core`.
//! * `KeyEvent` is exactly `{ key: Key, mods: Mods }` (both
//!   `pub`, `edit/keys.rs`). `KeyEvent::is_command(c)`
//!   matches `Key::Command(c)` ONLY — a `Key::Text` event
//!   with ctrl/cmd mods does not open the palette — so the
//!   Ctrl+K event below is built as `Key::Command('k')`,
//!   the shape rune's own edit tests use for chords.
//! * Typing needs no seeding: `route_key`
//!   (runtime/input/keys.rs) delivers `Key::Text` events
//!   to the focused input's editor, and a field whose
//!   `TextEdit.focused` flag is set claims that focus at
//!   commit (`sync_requested_focus`) — `open_palette`
//!   sets the flag, so after a settling frame the queued
//!   characters land in `state.palette.query.text` for
//!   real (rune's own `on_key_tests` prove the same path:
//!   flag + frame + `Key::Text` ⇒ the field's text grows).
//! * `Runtime::find` returns the FIRST node in tree order
//!   with a key, so the menu test taps only dropdown-unique
//!   keys (`export` is asserted, never tapped; the Edit
//!   item tapped is `copy-frame`) — `save`/`load`/`undo`
//!   also live in the app bar, earlier in the tree.
//!
//! Round 3 (Phase F, section 39) adds four tests over the
//! Phase E surfaces rounds 1–2 only touched indirectly:
//! workspace gating (panel keys against the `workspaces`
//! table), the View menu's sidebar toggles, the settings
//! guides toggle, and a transport playback pass asserted
//! on `core.current`. Their extra discipline: a test that
//! depends on the workspace seeds it explicitly through
//! the chips (the persisted prefs are shared across the
//! test process), and structural unmounts are given a
//! bounded settle loop before an absence assert, the
//! same courtesy the dropdown's presence fade needed.
//!
//! Round 4 (Phase G, section 39) adds four tests over the
//! Phase G surfaces: stroke selection (Draw workspace),
//! the split guide toggles in the settings card, the
//! workspace blurb line, and the particles panel's Add
//! button. Extra facts they rest on:
//! * The demo project has NO strokes and NO particle
//!   events (`Project::new_demo` builds both vecs empty),
//!   so the stroke test asserts the selection row's
//!   empty-project behaviour — the press proven landed by
//!   the handler's own status line — while the particles
//!   test starts from an empty list and a real first Add.
//! * The guide children (`settings-guides-frustum`,
//!   `settings-guides-safe`) write flags of their own
//!   (`State.guides_frustum` / `guides_safe`), independent
//!   of the parent `guides` flag round 3 exercised.
//! * The particles panel's section gating is the
//!   integrator's step (expected: the Effects workspace);
//!   the test navigates by the `ws-effects` chip and
//!   asserts on state, so it exercises the panel as soon
//!   as it is mounted there.
//!
//! Round 5 (Phase H, section 39) adds three tests: the
//! command palette's loop-playback toggle (over section
//! 36's State-level dispatch migration), the transport
//! play control's key/state behaviour, and the Project
//! section's audit-map key coverage — the Phase G §12
//! audit's closing item, which belongs here because a
//! unit test inside project_tools cannot walk the built
//! element tree (rune's `El` exposes no public tree
//! walk); only the Runtime's presented tree can prove
//! the emitted keys. Facts they rest on, verified
//! against the tree:
//! * Section 36's migration landed as briefed:
//!   `Command::run` is `fn(&mut State)`, and the toggle
//!   command's key is exactly `toggle-loop-playback`
//!   ("Toggle loop playback", Transport group); its
//!   shim flips `State::loop_playback` and nothing else
//!   — the `pb-loop` panel button's own flip. Typing in
//!   the palette resets the highlight to row 0 (the
//!   field's `on_input_change`), and `display_commands`
//!   / `COMMANDS` are the same pub listing
//!   `run_selected` resolves through, so the test
//!   locates the toggle's row in that listing for the
//!   typed query and walks the highlight to it with
//!   Down — the palette's own selection key — instead
//!   of assuming the fuzzy ranking.
//! * The transport's play/pause control is a single
//!   element keyed `play` in both states
//!   (chrome_view::transport_row; its label and active
//!   flag flip with `core.playing`) — no element keyed
//!   `pause` exists anywhere in the shell, and
//!   playback_controls' own transport-family keys
//!   (`pb-restart` / `pb-play-range` / `pb-loop`) never
//!   toggle play. The transport test therefore asserts
//!   the one key's presence in both states plus
//!   `core.playing` — key presence and state only,
//!   never pixels.
//! * `Section::Project` is gated to Animate, Export and
//!   Home (workspaces.rs); every static key in the §12
//!   audit map is emitted unconditionally by
//!   `project_section`, including `proj-list` (the
//!   empty-list text covers the no-projects case), so
//!   all of them must appear in Animate's presented
//!   tree. The dynamic `open-proj-{i}` rows need a saved
//!   project on disk and are not asserted.

use std::collections::BTreeSet;

use rune::{Env, Key, KeyEvent, Mods, Runtime};

use crate::stage_view::{to_screen, to_stage, view_transform};
use crate::{init, on_key, poll, view, State};

/// The AppSpec's window size, so layout matches the real app.
const VIEWPORT: (f32, f32) = (1440.0, 900.0);
/// One 60 fps step of the virtual clock.
const DT: f64 = 1.0 / 60.0;

/// A runtime holding a fresh editor (the demo project), with
/// the app's real `poll`/`on_key` hooks installed and two
/// frames already run so layout — and the stage's
/// `layout_size` read — has settled.
fn editor() -> Runtime<State> {
    let mut rt = Runtime::new(
        init(),
        view,
        VIEWPORT,
        Env {
            system_dark: true,
            scale: 1.0,
        },
    );
    rt.poll = Some(poll);
    rt.on_key = Some(on_key);
    rt.frame(0.0);
    rt.frame(DT);
    rt
}

/// Press-and-release the centre of the element keyed `key`,
/// then run one frame so the queued press handler lands.
fn tap(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt
        .find(key)
        .unwrap_or_else(|| panic!("no element keyed {key:?}"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(DT);
}

/// The set of element keys the last frame presented.
fn presented_keys(rt: &Runtime<State>) -> BTreeSet<String> {
    rt.elements()
        .iter()
        .filter_map(|e| e.key.clone())
        .collect()
}

/// Queue one key event, then run one frame so the runtime
/// dispatches it through the real routing (app `on_key`,
/// focused input, editor — `route_key` in the pinned
/// source).
fn press_key(rt: &mut Runtime<State>, key: Key, mods: Mods) {
    rt.key(KeyEvent { key, mods });
    rt.frame(DT);
}

/// Type one character into whatever field holds focus, the
/// way the OS delivers it: a plain `Key::Text` event, no
/// modifiers.
fn type_char(rt: &mut Runtime<State>, c: char) {
    press_key(rt, Key::Text(c.to_string()), Mods::default());
}

#[test]
fn shell_mounts_stage_chrome_and_timeline() {
    let mut rt = editor();
    // Seed the Animate workspace through its chip (the
    // persisted prefs are shared across the test process):
    // the panel-sourced keys below mount only where their
    // sections are gated visible.
    tap(&mut rt, "ws-animate");
    rt.frame(DT);
    let keys = presented_keys(&rt);
    for want in [
        // The stage canvas and the chrome rows.
        "stage",
        "app-bar",
        "transport",
        "timeline",
        // App bar controls, plus the panel homes of the
        // controls the app bar used to duplicate (Phase F):
        // add-figure lives on the figures panel, onion-toggle
        // on the onion panel.
        "save",
        "load",
        "undo",
        "redo",
        "add-figure",
        "onion-toggle",
        // Transport + frame ops.
        "prev",
        "play",
        "next",
        "add-frame",
        "dup-frame",
        "del-frame",
        // One chip per demo frame (the demo project has five).
        "frame-1",
        "frame-2",
        "frame-3",
        "frame-4",
        "frame-5",
    ] {
        assert!(keys.contains(want), "missing element keyed {want:?}");
    }
    assert!(
        rt.find("stage").is_some(),
        "stage has a findable centre"
    );
    let (rect, _opacity) = rt.presented("stage").expect("stage is presented");
    assert!(
        rect[2] > 100.0 && rect[3] > 100.0,
        "stage has a real size: {rect:?}"
    );
    assert!(
        rect[0] >= 0.0 && rect[1] >= 0.0 && rect[0] + rect[2] <= VIEWPORT.0 && rect[1] + rect[3] <= VIEWPORT.1,
        "stage lies inside the viewport: {rect:?}"
    );
    // State: a fresh editor shows the demo project's first of
    // five frames, and the status line says so.
    assert_eq!(rt.state.core.frame_count(), 5);
    assert_eq!(rt.state.core.current, 0);
    assert!(
        rt.elements().iter().any(|e| e
            .text
            .as_deref()
            .is_some_and(|t| t.starts_with("Frame 1/5"))),
        "status line reads Frame 1/5"
    );
}

#[test]
fn add_frame_button_and_chip_navigation() {
    let mut rt = editor();
    assert_eq!(rt.state.core.frame_count(), 5);

    tap(&mut rt, "add-frame");
    // `add_frame` inserts after the current frame and steps
    // onto the new one (editor.rs op semantics).
    assert_eq!(rt.state.core.frame_count(), 6);
    assert_eq!(rt.state.core.current, 1);
    assert!(
        presented_keys(&rt).contains("frame-6"),
        "a sixth chip appears"
    );

    // A chip press goes to that frame (`goto`).
    tap(&mut rt, "frame-3");
    assert_eq!(rt.state.core.current, 2);

    // The Next transport button steps one frame forward.
    tap(&mut rt, "next");
    assert_eq!(rt.state.core.current, 3);
}

#[test]
fn play_advances_the_playhead_on_the_virtual_clock() {
    let mut rt = editor();
    assert!(!rt.state.core.playing);
    assert_eq!(rt.state.core.current, 0);

    tap(&mut rt, "play");
    assert!(rt.state.core.playing, "Play flips core.playing");

    // A quarter second of virtual time at the demo's 12 fps is
    // three frames of playhead. `poll` measures from the
    // runtime clock and `advance_play` honours fps, so the
    // playhead must land near 3.0 and the current frame must
    // have left frame 0 — deterministically, with no wall
    // clock involved.
    for _ in 0..15 {
        rt.frame(DT);
    }
    let playhead = rt.state.core.playhead;
    assert!(
        playhead > 2.0,
        "playhead advanced with the virtual clock: {playhead}"
    );
    assert!(
        rt.state.core.current >= 2,
        "current frame followed the playhead: {}",
        rt.state.core.current
    );
    assert!(rt.state.core.playing, "still playing (no end stop)");

    // Pausing freezes the playhead exactly: more frames, no
    // movement.
    tap(&mut rt, "play");
    assert!(!rt.state.core.playing);
    let frozen = rt.state.core.playhead;
    for _ in 0..10 {
        rt.frame(DT);
    }
    assert_eq!(rt.state.core.playhead, frozen, "paused playhead is frozen");
}

#[test]
fn stage_drag_grabs_the_root_joint_and_moves_the_pose() {
    let mut rt = editor();
    let (rect, _opacity) = rt.presented("stage").expect("stage is presented");
    let [sx, sy, sw, sh] = rect;
    assert!(sw > 100.0 && sh > 100.0, "stage has real size: {rect:?}");

    // The demo figure's root joint (node 0), in stage
    // coordinates, and where the stage transform draws it.
    // The root hit-tests to itself by construction (distance
    // zero), which pins the press point to a real joint
    // without depending on the demo pose's exact numbers.
    let core = &rt.state.core;
    let frame = core.current;
    let root = core.positions(frame, 0)[0];
    assert_eq!(
        core.hit_test(root),
        Some((0, 0)),
        "root joint hit-tests to itself"
    );
    let vt = view_transform([sw, sh], core.project.stage, core.camera_here());
    let local = to_screen(&vt, root);
    let down = (sx + local[0], sy + local[1]);

    // Press: `begin_drag` via the canvas `on_drag` Phase::Down
    // selects the figure and the joint it grabbed.
    rt.pointer_down(down.0, down.1);
    rt.frame(DT);
    assert_eq!(rt.state.core.selected_figure, 0);
    assert_eq!(
        rt.state.core.selected_node,
        Some(0),
        "the press grabbed the root joint"
    );

    // Move: a root drag sets the pose root to the pointer's
    // stage point, so the joint must track the pointer through
    // the same transform the drawing uses.
    let delta = [48.0f32, 30.0];
    rt.pointer_move(down.0 + delta[0], down.1 + delta[1]);
    rt.frame(DT);
    let moved = rt.state.core.positions(frame, 0)[0];
    let expected = to_stage(&vt, [local[0] + delta[0], local[1] + delta[1]]);
    assert!(
        (moved[0] - root[0]).abs() > 20.0 || (moved[1] - root[1]).abs() > 10.0,
        "the root actually moved: {root:?} -> {moved:?}"
    );
    assert!(
        (moved[0] - expected[0]).abs() < 2.0 && (moved[1] - expected[1]).abs() < 2.0,
        "the root tracks the pointer: expected {expected:?}, got {moved:?}"
    );

    // Release: the pose stays where the drag left it.
    rt.pointer_up(down.0 + delta[0], down.1 + delta[1]);
    rt.frame(DT);
    let settled = rt.state.core.positions(frame, 0)[0];
    assert!(
        (settled[0] - moved[0]).abs() < 0.01 && (settled[1] - moved[1]).abs() < 0.01,
        "the pose stays where the drag left it: {moved:?} -> {settled:?}"
    );
}

// ----------------------------------------------------------------
// Round 2 (Phase E, section 37): menus, palette, settings,
// workspaces — the Phase D surfaces driven end to end.
// ----------------------------------------------------------------

#[test]
fn file_menu_opens_lists_items_and_scrim_closes() {
    let mut rt = editor();
    assert_eq!(rt.state.menus.open, None);
    assert!(
        !presented_keys(&rt).contains("export"),
        "no File dropdown before the trigger is pressed"
    );

    // Trigger: the menu opens (state) and its dropdown is
    // presented with its items. `save`/`load` also exist in
    // the app bar, so the dropdown-only item key `export`
    // (and the dropdown surface's own key) are the proof
    // the items are really there.
    tap(&mut rt, "menu-file");
    // A settling frame: the dropdown is `anchored` to the
    // trigger, so let its first commit land before reading
    // the presented tree.
    rt.frame(DT);
    assert_eq!(
        rt.state.menus.open,
        Some(crate::rune_sec::menus::FILE),
        "File trigger opens the File menu"
    );
    let keys = presented_keys(&rt);
    assert!(keys.contains("menu-file-dropdown"), "dropdown surface presented");
    assert!(keys.contains("export"), "File ▸ Export PNG item presented");
    assert!(keys.contains("menu-scrim"), "dismissal scrim presented");

    // Scrim: a press on the full-window dismissal layer
    // closes the menu and the dropdown leaves the tree.
    tap(&mut rt, "menu-scrim");
    assert_eq!(rt.state.menus.open, None, "scrim press closes the menu");
    // The anchored dropdown animates out (presence springs):
    // its nodes stay "presented" for a few frames after the
    // state closes — the container key goes first, the item
    // keys a few frames later — so settle until the tree has
    // dropped them (bounded, like a real fade-out).
    for _ in 0..30 {
        if !presented_keys(&rt).contains("export") {
            break;
        }
        rt.frame(DT);
    }
    let keys = presented_keys(&rt);
    assert!(!keys.contains("menu-file-dropdown"), "dropdown dismissed");
    assert!(!keys.contains("export"), "dropdown items dismissed");

    // Close-on-select: File ▸ Export PNG runs its op and
    // dismisses the menu (every item goes through the menus
    // section's closing `item` wrapper). `export` is the
    // provably dropdown-unique item key (see the opening
    // asserts); the Edit menu's items cannot drive this —
    // `find` returns the first key match in tree order, and
    // `undo` is shadowed by the app bar while `copy-frame`
    // and `paste-frame` are shadowed by the frame-ops panel.
    tap(&mut rt, "menu-file");
    rt.frame(DT);
    assert_eq!(rt.state.menus.open, Some(crate::rune_sec::menus::FILE));
    tap(&mut rt, "export");
    assert_eq!(rt.state.menus.open, None, "selecting an item closes the menu");
}

#[test]
fn command_palette_ctrl_k_types_and_enter_runs_undo() {
    let mut rt = editor();

    // Something undoable first, so the palette's Undo has a
    // visible effect: the demo project's five frames -> six.
    tap(&mut rt, "add-frame");
    assert_eq!(rt.state.core.frame_count(), 6);

    // Ctrl+K opens the palette. `palette_handle_key` (called
    // first by the shell's `on_key`) checks
    // `ev.is_command('k')`, which matches `Key::Command('k')`
    // only — hence this event shape (rune's own edit tests
    // build command chords the same way).
    press_key(
        &mut rt,
        Key::Command('k'),
        Mods {
            cmd: true,
            ..Mods::default()
        },
    );
    assert!(rt.state.palette.open, "Ctrl+K opens the palette");
    assert!(
        presented_keys(&rt).contains("command-palette"),
        "palette panel is mounted"
    );
    // A settling frame: the freshly mounted query field
    // claims runtime focus from the `focused` flag
    // `open_palette` set on its TextEdit.
    rt.frame(DT);

    // Type the query through the real input path — queued
    // Key::Text events routed to the focused field, no
    // state seeding. The text landing in the field's own
    // edit state is the proof the path is real.
    for c in "undo".chars() {
        type_char(&mut rt, c);
    }
    assert_eq!(
        rt.state.palette.query.text, "undo",
        "typed characters landed in the query field"
    );

    // Enter runs the highlighted row: `filter_commands`
    // ranks Undo first for "undo" (the palette section's own
    // unit tests prove the ranking), and running a command
    // closes the palette. Either Enter route lands there —
    // the single-line field's `on_submit` and
    // `palette_handle_key` both call `run_selected`.
    press_key(&mut rt, Key::Enter, Mods::default());
    assert_eq!(
        rt.state.core.frame_count(),
        5,
        "the palette's Undo reverted the added frame"
    );
    assert!(!rt.state.palette.open, "running a command closes the palette");
}

#[test]
fn settings_theme_buttons_flip_core_light_theme() {
    let mut rt = editor();

    // The settings card starts closed and its theme buttons
    // exist only while it is open — open it first. (The
    // settings section is mounted in every workspace, so
    // this does not depend on the seeded workspace.)
    tap(&mut rt, "settings-toggle");
    assert!(rt.state.settings.open, "Settings toggle opens the card");

    // Each theme button sets its own value outright, never
    // a blind toggle, so these assertions hold whatever
    // theme the persisted layout prefs seeded at `init`.
    tap(&mut rt, "settings-theme-light");
    assert!(rt.state.core.light_theme, "Light sets core.light_theme");
    tap(&mut rt, "settings-theme-dark");
    assert!(!rt.state.core.light_theme, "Dark clears core.light_theme");
}

#[test]
fn workspace_chip_switches_the_workspace_field() {
    use crate::rune_sec::workspaces::Workspace;

    let mut rt = editor();
    // The chip row is permanent shell furniture (mounted
    // under the menu bar in every workspace). A press runs
    // the workspaces section's `select_workspace`: the
    // field flips (and the choice persists to the layout
    // prefs — the section's own behaviour, not faked here).
    tap(&mut rt, "ws-draw");
    assert_eq!(
        rt.state.workspace,
        Workspace::Draw,
        "Draw chip selects the Draw workspace"
    );
    // And back — which also leaves the shared layout prefs
    // naming Animate again, as they started.
    tap(&mut rt, "ws-animate");
    assert_eq!(
        rt.state.workspace,
        Workspace::Animate,
        "Animate chip switches back"
    );
}

// ----------------------------------------------------------------
// Round 3 (Phase F, section 39): workspace gating, View
// menu column toggles, settings guides, transport playback.
// ----------------------------------------------------------------

#[test]
fn workspace_gating_hides_and_shows_panels() {
    use crate::rune_sec::workspaces::Workspace;

    let mut rt = editor();
    // Seed the workspace explicitly: `init` restores it
    // from the persisted layout prefs, which every test in
    // this process shares, so the starting workspace is
    // whatever a sibling test last persisted. The chips
    // are permanent furniture — use them to force Animate
    // before asserting anything about its panel set.
    tap(&mut rt, "ws-animate");
    rt.frame(DT);
    assert_eq!(rt.state.workspace, Workspace::Animate);
    // If `init` restored a sibling test's Draw workspace,
    // the Draw-only panel is still presence-fading out of
    // the tree; settle (bounded) before asserting absence —
    // the same fade the menu tests settle through.
    for _ in 0..30 {
        if !presented_keys(&rt).contains("draw-tools") {
            break;
        }
        rt.frame(DT);
    }

    // The gating table (workspaces.rs): Animate mounts
    // Figures and FrameOps but not DrawTools. Panel root
    // keys verified in their section files: figures_panel
    // roots at `figures-panel`, frame_ops at `frames`,
    // draw_tools at `draw-tools` — each key exists exactly
    // once in the shell, so presence here means the panel
    // is mounted, absence that gating left it out.
    let keys = presented_keys(&rt);
    assert!(keys.contains("figures-panel"), "Animate mounts Figures");
    assert!(keys.contains("frames"), "Animate mounts FrameOps");
    assert!(
        !keys.contains("draw-tools"),
        "Animate gates DrawTools out"
    );

    // Switch to Draw: its set is DrawTools + StrokeInspector
    // + TextTool + Figures. The Draw-only panel appears,
    // the Animate-only one leaves, and Figures — listed in
    // both workspaces — stays: gating is per-section, not
    // a whole-tree swap.
    tap(&mut rt, "ws-draw");
    rt.frame(DT);
    assert_eq!(rt.state.workspace, Workspace::Draw);
    // Same presence fade on the way out for FrameOps.
    for _ in 0..30 {
        if !presented_keys(&rt).contains("frames") {
            break;
        }
        rt.frame(DT);
    }
    let keys = presented_keys(&rt);
    assert!(keys.contains("draw-tools"), "Draw mounts DrawTools");
    assert!(!keys.contains("frames"), "Draw gates FrameOps out");
    assert!(
        keys.contains("figures-panel"),
        "Figures is in both workspaces and stays"
    );

    // Restore Animate for the shared prefs, as round 2 did.
    tap(&mut rt, "ws-animate");
    assert_eq!(rt.state.workspace, Workspace::Animate);
}

#[test]
fn view_menu_toggles_hide_columns() {
    let mut rt = editor();
    // Fresh shell state: both sidebars shown (the flags are
    // shell state, not persisted, so `init` defaults them).
    assert!(rt.state.show_left, "left sidebar starts shown");
    assert!(
        presented_keys(&rt).contains("figures-panel"),
        "left column content is presented"
    );

    // Open the View menu. The dropdown is anchored, so a
    // settling frame before reading the tree (round 2's
    // lesson); `view-toggle-left` is dropdown-unique.
    tap(&mut rt, "menu-view");
    rt.frame(DT);
    assert_eq!(
        rt.state.menus.open,
        Some(crate::rune_sec::menus::VIEW),
        "View trigger opens the View menu"
    );
    assert!(
        presented_keys(&rt).contains("view-toggle-left"),
        "View dropdown lists the sidebar toggle"
    );

    // Selecting the item flips the flag and closes the
    // menu (the shared `item` wrapper), and the whole left
    // column unmounts. The unmount is structural, but give
    // it the same bounded settle as the presence fades
    // before asserting absence.
    tap(&mut rt, "view-toggle-left");
    assert_eq!(rt.state.menus.open, None, "selecting the item closes the menu");
    assert!(!rt.state.show_left, "the toggle flipped show_left");
    for _ in 0..30 {
        if !presented_keys(&rt).contains("figures-panel") {
            break;
        }
        rt.frame(DT);
    }
    let keys = presented_keys(&rt);
    assert!(!keys.contains("side-left"), "left column unmounted");
    assert!(
        !keys.contains("figures-panel"),
        "left column content left the tree"
    );

    // Toggle back through the same menu path: the column
    // and its panels return.
    tap(&mut rt, "menu-view");
    rt.frame(DT);
    tap(&mut rt, "view-toggle-left");
    assert!(rt.state.show_left, "the toggle restored show_left");
    rt.frame(DT);
    assert!(
        presented_keys(&rt).contains("figures-panel"),
        "left column content is back"
    );
}

#[test]
fn guides_toggle_flips_state() {
    let mut rt = editor();

    // The guides button lives inside the settings card,
    // which starts closed — open it first (round 2's
    // pattern). The settings section is ungated, so this
    // holds in whatever workspace the prefs seeded.
    tap(&mut rt, "settings-toggle");
    assert!(rt.state.settings.open, "Settings toggle opens the card");
    // The opened card's children presence-fade/flight in:
    // settle (bounded) until the Guides button is fully
    // presented before pressing it — the same courtesy the
    // menu dropdown needed in round 2.
    let mut settled = None;
    for _ in 0..90 {
        rt.frame(DT);
        if let Some((rect, opacity)) = rt.presented("settings-guides") {
            if opacity > 0.99 {
                settled = Some((rect, opacity));
                break;
            }
        }
    }

    // State-level only, by design: the guides are drawn
    // into the stage canvas `Shape`, whose pixels the
    // Runtime cannot read back — the flag flip through
    // the real button handler (`toggle_guides`, the same
    // fn the View menu item calls) is the provable part.
    // Value-agnostic, like round 2's theme test: whatever
    // the flag starts at, each tap must invert it.
    let before = rt.state.guides;
    let theme_rect = rt.presented("settings-theme-light");
    let guides_rect = rt.presented("settings-guides");
    tap(&mut rt, "settings-guides");
    assert_eq!(
        rt.state.guides, !before,
        "Guides tap flips the flag (settled: {settled:?}, theme: {theme_rect:?}, guides: {guides_rect:?})"
    );
    tap(&mut rt, "settings-guides");
    assert_eq!(rt.state.guides, before, "second tap flips it back");
}

#[test]
fn playback_advances_and_pauses() {
    let mut rt = editor();
    assert!(!rt.state.core.playing);
    assert_eq!(rt.state.core.current, 0);

    // The transport Play button (chrome_view's `play` key;
    // its label flips to Pause while playing, the key does
    // not). Round 1 proved the playhead maths on this same
    // path; this pass asserts the frame counter itself.
    tap(&mut rt, "play");
    assert!(rt.state.core.playing, "Play flips core.playing");

    // Twelve virtual frames is a fifth of a second — over
    // two frame intervals at the demo's 12 fps — so the
    // playhead must have carried `current` off frame 0.
    for _ in 0..12 {
        rt.frame(DT);
    }
    assert!(
        rt.state.core.current >= 1,
        "current frame advanced while playing: {}",
        rt.state.core.current
    );
    assert!(rt.state.core.playing, "still playing (loops, no end stop)");

    // The same button is now Pause: playing flips off and
    // the frame counter freezes where it stopped.
    tap(&mut rt, "play");
    assert!(!rt.state.core.playing, "Pause flips core.playing off");
    let stopped = rt.state.core.current;
    for _ in 0..10 {
        rt.frame(DT);
    }
    assert_eq!(
        rt.state.core.current, stopped,
        "paused frame counter is frozen"
    );
}

// ----------------------------------------------------------------
// Round 4 (Phase G, section 39): stroke selection, the
// split guide toggles, the workspace blurb line, and the
// particles panel.
// ----------------------------------------------------------------

#[test]
fn stroke_selection_row_reports_empty_project() {
    use crate::rune_sec::workspaces::Workspace;

    let mut rt = editor();
    // The demo project has no strokes: `Project::new_demo`
    // builds `strokes` empty, and nothing in this test adds
    // one. The selection row's contract on an empty project
    // (the stroke section's module docs) is that Next and
    // Clear only report via the status line and the
    // selection stays None — that is the behaviour true of
    // this shell state, and it is what is asserted. (With
    // strokes present, Next from None selects index 0 and
    // steps with wraparound; the section's own unit tests
    // prove that stepping over synthetic states.)
    assert!(rt.state.core.project.strokes.is_empty());
    assert_eq!(rt.state.stroke_selected, None);

    // The StrokeInspector section is gated to Draw (the
    // gating table round 3's test exercised).
    tap(&mut rt, "ws-draw");
    rt.frame(DT);
    assert_eq!(rt.state.workspace, Workspace::Draw);
    // The row renders always, empty project or not. Settle
    // (bounded) through the freshly mounted panel's
    // presence fade before pressing inside it — round 3's
    // guides lesson.
    for _ in 0..90 {
        rt.frame(DT);
        if let Some((_, opacity)) = rt.presented("stroke-sel-next") {
            if opacity > 0.99 {
                break;
            }
        }
    }
    assert!(
        presented_keys(&rt).contains("stroke-sel-row"),
        "Draw mounts the stroke selection row"
    );

    // Next on an empty project: no selection appears, and
    // the handler's own status line proves the press
    // landed (a missed press would leave the status
    // untouched).
    tap(&mut rt, "stroke-sel-next");
    assert_eq!(
        rt.state.stroke_selected, None,
        "no strokes -> Next selects nothing"
    );
    assert_eq!(
        rt.state.core.status, "No strokes to select.",
        "the empty-project press reports via the status line"
    );

    // Clear behaves the same on an empty project.
    tap(&mut rt, "stroke-sel-clear");
    assert_eq!(rt.state.stroke_selected, None);
    assert_eq!(rt.state.core.status, "No strokes to select.");

    // Restore the shared prefs' workspace, as rounds 2–3 do.
    tap(&mut rt, "ws-animate");
    assert_eq!(rt.state.workspace, Workspace::Animate);
}

#[test]
fn guide_split_toggles_flip_independently() {
    let mut rt = editor();

    // Open the settings card exactly as round 3's guides
    // test did, and settle (bounded) until a guide child
    // button is fully presented before pressing it — the
    // card's children presence-fade in.
    tap(&mut rt, "settings-toggle");
    assert!(rt.state.settings.open, "Settings toggle opens the card");
    for _ in 0..90 {
        rt.frame(DT);
        if let Some((_, opacity)) = rt.presented("settings-guides-safe") {
            if opacity > 0.99 {
                break;
            }
        }
    }
    assert!(
        presented_keys(&rt).contains("settings-guides-frustum"),
        "the guide children row is presented"
    );

    // The two children write flags of their own
    // (`State.guides_safe` / `guides_frustum`), independent
    // of the parent Guides flag: flipping either child
    // moves only its own flag. Value-agnostic flip
    // assertions, like round 3's theme and guides tests —
    // whatever the flags start at, each tap inverts its
    // own and leaves the other two alone.
    let guides0 = rt.state.guides;
    let safe0 = rt.state.guides_safe;
    let frustum0 = rt.state.guides_frustum;

    tap(&mut rt, "settings-guides-safe");
    assert_eq!(
        rt.state.guides_safe, !safe0,
        "Safe frames tap flips guides_safe"
    );
    assert_eq!(rt.state.guides, guides0, "Guides itself is untouched");
    assert_eq!(
        rt.state.guides_frustum, frustum0,
        "Camera frame is untouched"
    );

    tap(&mut rt, "settings-guides-frustum");
    assert_eq!(
        rt.state.guides_frustum, !frustum0,
        "Camera frame tap flips guides_frustum"
    );
    assert_eq!(rt.state.guides, guides0, "Guides itself is still untouched");
    assert_eq!(
        rt.state.guides_safe, !safe0,
        "Safe frames keeps its flipped value"
    );

    // Restore both flags to where the test found them.
    tap(&mut rt, "settings-guides-safe");
    tap(&mut rt, "settings-guides-frustum");
    assert_eq!(rt.state.guides_safe, safe0, "Safe frames restored");
    assert_eq!(rt.state.guides_frustum, frustum0, "Camera frame restored");
    assert_eq!(rt.state.guides, guides0);
}

#[test]
fn workspace_blurb_line_names_the_active_workspace() {
    use crate::rune_sec::workspaces::Workspace;

    let mut rt = editor();
    // The line under the chips (key `ws-blurb`) shows the
    // ACTIVE workspace's one-sentence summary — the
    // workspaces section's `blurb_line`, i.e. the active
    // workspace's own `blurb()`. The startup workspace
    // comes from the shared prefs, so force Animate
    // through its chip first (round 3's seeding
    // discipline) — which also leaves the prefs naming
    // Animate, as they started.
    tap(&mut rt, "ws-animate");
    rt.frame(DT);
    assert_eq!(rt.state.workspace, Workspace::Animate);

    // The element is presented and its text is the Animate
    // blurb verbatim — the section's `blurb()` is the
    // source of truth for the wording, so no string is
    // duplicated here.
    let blurb = rt
        .elements()
        .iter()
        .find(|e| e.key.as_deref() == Some("ws-blurb"))
        .expect("blurb line is presented")
        .text
        .clone()
        .expect("blurb line carries text");
    assert!(!blurb.is_empty(), "blurb text is non-empty");
    assert_eq!(
        blurb,
        Workspace::Animate.blurb(),
        "the line shows the active workspace's blurb"
    );
}

#[test]
fn particles_panel_add_selects_the_new_event() {
    use crate::rune_sec::workspaces::Workspace;

    let mut rt = editor();
    // The demo project starts with no particle events
    // (`Project::new_demo` builds `particles` empty) and
    // no event selected.
    assert!(rt.state.core.project.particles.is_empty());
    assert_eq!(rt.state.particle_selected, None);

    // The particles panel is gated to Effects (the
    // integrator's Section for it); navigate by the chip
    // like every workspace-dependent test here.
    tap(&mut rt, "ws-effects");
    rt.frame(DT);
    assert_eq!(rt.state.workspace, Workspace::Effects);
    assert!(
        presented_keys(&rt).contains("particles"),
        "Effects mounts the particles panel"
    );
    // The panel sits below Settings, Scene and the FX
    // panels in the right column — below the 900 pt
    // viewport's fold. Scroll the column (wheel over it,
    // the real user's path) until the Add button's centre
    // is inside the window, or the press lands nowhere.
    for _ in 0..24 {
        let (_, y) = rt.find("particles-add").expect("particles-add presented");
        if y < 850.0 {
            break;
        }
        rt.wheel(1255.0, 450.0, -600.0);
        rt.frame(DT);
    }
    // Settle (bounded) through the freshly mounted panel's
    // presence fade before pressing inside it — round 3's
    // guides lesson.
    for _ in 0..90 {
        rt.frame(DT);
        if let Some((_, opacity)) = rt.presented("particles-add") {
            if opacity > 0.99 {
                break;
            }
        }
    }

    // Add: the panel's `add_event` appends one event and
    // selects it (its landing frame/point details are the
    // section's own unit tests' business; here the real
    // button proves the panel is wired to the project and
    // the shell's selection state).
    tap(&mut rt, "particles-add");
    assert_eq!(
        rt.state.core.project.particles.len(),
        1,
        "Add appends one particle event"
    );
    assert_eq!(
        rt.state.particle_selected,
        Some(0),
        "the new event is selected"
    );

    // Restore the shared prefs' workspace, as rounds 2–3 do.
    tap(&mut rt, "ws-animate");
    assert_eq!(rt.state.workspace, Workspace::Animate);
}

// ----------------------------------------------------------------
// Round 5 (Phase H, section 39): the palette's
// loop-playback toggle over the State-level dispatch, the
// transport play control's key/state behaviour, and the
// Project section's audit-map key coverage.
// ----------------------------------------------------------------

#[test]
fn palette_toggles_loop_playback() {
    use crate::rune_sec::command_palette::{display_commands, COMMANDS};

    let mut rt = editor();
    // Shell state, not persisted: every fresh shell starts
    // with loop playback on (the integrator's `init`
    // value, the Phase G default that preserved the
    // engine's unconditional wrap).
    assert!(rt.state.loop_playback, "loop playback starts on");

    // One full pass through the palette's real run path —
    // Ctrl+K, type the query, walk the highlight to the
    // toggle command's row, Enter — returning the loop
    // flag as the pass left it.
    let run_toggle = |rt: &mut Runtime<State>| -> bool {
        // Ctrl+K, built exactly as round 2's palette test
        // builds it (`is_command('k')` matches
        // `Key::Command('k')` only).
        press_key(
            rt,
            Key::Command('k'),
            Mods {
                cmd: true,
                ..Mods::default()
            },
        );
        assert!(rt.state.palette.open, "Ctrl+K opens the palette");
        // A settling frame: the query field claims runtime
        // focus from the flag `open_palette` set.
        rt.frame(DT);
        for c in "loop".chars() {
            type_char(rt, c);
        }
        assert_eq!(
            rt.state.palette.query.text, "loop",
            "typed characters landed in the query field"
        );
        // Locate the toggle in the palette's own listing
        // for this query — the same `display_commands`
        // `run_selected` resolves through — and walk the
        // highlight to its row with Down, the palette's
        // own selection key. (Typing reset the highlight
        // to row 0; the delta walk holds either way.)
        let (results, _) =
            display_commands(&rt.state.palette.query.text, &rt.state.palette.recent);
        let row = results
            .iter()
            .position(|&ci| COMMANDS[ci].key == "toggle-loop-playback")
            .expect("the loop query lists toggle-loop-playback (section 36's key)");
        let len = results.len();
        let cur = rt.state.palette.selected % len;
        for _ in 0..(row + len - cur) % len {
            press_key(rt, Key::Down, Mods::default());
        }
        assert_eq!(
            rt.state.palette.selected, row,
            "the highlight sits on the toggle's row"
        );
        // Enter runs the selection (the field's `on_submit`
        // and `palette_handle_key` both call `run_selected`)
        // and running a command closes the palette. The
        // command's shim is the `pb-loop` flip itself, now
        // reachable because dispatch is State-level.
        press_key(rt, Key::Enter, Mods::default());
        assert!(
            !rt.state.palette.open,
            "running a command closes the palette"
        );
        rt.state.loop_playback
    };

    assert!(
        !run_toggle(&mut rt),
        "first run flips loop playback off"
    );
    assert!(
        run_toggle(&mut rt),
        "second run flips loop playback back on"
    );
}

#[test]
fn transport_shows_icon_buttons() {
    use crate::rune_sec::workspaces::Workspace;

    let mut rt = editor();
    // Seed Animate (the shared-prefs discipline) — which
    // is also the state this test leaves behind.
    tap(&mut rt, "ws-animate");
    rt.frame(DT);
    assert_eq!(rt.state.workspace, Workspace::Animate);

    // The transport's play/pause control is ONE element:
    // chrome_view::transport_row keys it `play` in both
    // states — the label (Play/Pause via i18n) and the
    // active flag flip with `core.playing`; there is no
    // separate `pause` key anywhere in the shell, so the
    // pause state is this same control, presented active.
    // (playback_controls' own transport-family keys —
    // `pb-restart`, `pb-play-range`, `pb-loop` — start or
    // loop playback but never toggle it.) Assertions stay
    // at key presence + `core.playing`, as the round's
    // discipline requires.
    assert!(!rt.state.core.playing, "playback starts stopped");
    assert!(
        presented_keys(&rt).contains("play"),
        "Animate presents the transport's play control"
    );

    // The press starts playback; the same key now
    // presents the control in its pause state.
    tap(&mut rt, "play");
    assert!(rt.state.core.playing, "the press starts playback");
    assert!(
        presented_keys(&rt).contains("play"),
        "the same control presents the pause state while playing"
    );

    // Restore: the same control pauses again.
    tap(&mut rt, "play");
    assert!(!rt.state.core.playing, "the second press pauses");
}

#[test]
fn project_section_key_coverage() {
    use crate::rune_sec::workspaces::Workspace;

    let mut rt = editor();
    // `Section::Project` is gated to Animate (also Export
    // and Home in the workspaces table). Seed Animate —
    // which also leaves the shared prefs naming Animate,
    // as they started.
    tap(&mut rt, "ws-animate");
    rt.frame(DT);
    assert_eq!(rt.state.workspace, Workspace::Animate);
    // The freshly mounted panel presence-fades in; settle
    // (bounded) until its root is presented before
    // reading the whole key set.
    for _ in 0..30 {
        if presented_keys(&rt).contains("project-tools") {
            break;
        }
        rt.frame(DT);
    }

    // The Phase G §12 audit map's static keys (the map
    // lives in project_tools' module docs). The section
    // root, the name field's wrapper, its Set button and
    // the New/Save/Recover row are emitted unconditionally
    // by `project_section`, so all of them must be in the
    // presented tree. `proj-list` is NOT asserted: it is
    // the non-empty branch's container — with no saved
    // projects the section renders its "No saved projects"
    // text instead (project_tools.rs, the `rows.is_empty()`
    // branch), and writing a project file from a Runtime
    // test would pollute the shared on-disk ./projects for
    // every sibling test. The dynamic `open-proj-{i}` rows
    // need a saved project on disk for the same reason, so
    // they are NOT asserted here either.
    let keys = presented_keys(&rt);
    for want in [
        "project-tools",
        "proj-name-field",
        "proj-set-name",
        "new-proj",
        "save-proj",
        "recover",
    ] {
        assert!(keys.contains(want), "missing audit-map key {want:?}");
    }
}
