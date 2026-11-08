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

    // State-level only, by design: the guides are drawn
    // into the stage canvas `Shape`, whose pixels the
    // Runtime cannot read back — the flag flip through
    // the real button handler (`toggle_guides`, the same
    // fn the View menu item calls) is the provable part.
    // Value-agnostic, like round 2's theme test: whatever
    // the flag starts at, each tap must invert it.
    let before = rt.state.guides;
    tap(&mut rt, "settings-guides");
    assert_eq!(rt.state.guides, !before, "Guides tap flips the flag");
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
