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
//!
//! Round 6 (Phase I, section 39) adds four tests over the
//! Phase I surfaces: the palette's workspace-jump
//! commands, the transport's Prev key / Next key jumps,
//! the clickable timeline markers, and the pose panel's
//! inert Paste control. Facts they rest on, verified
//! against the tree:
//! * The jump commands are ordinary palette commands
//!   (`go-animate` … `go-home`, Workspaces group) whose
//!   shims call `workspaces::select_workspace`, so round
//!   5's locate-and-walk driving pattern reaches them.
//!   The query "export" matches seven commands (the six
//!   `export-*` plus `go-export`), inside the palette's
//!   `MAX_RESULTS` cap of 12, so the jump is always
//!   listed for that query.
//! * `pb-prev-key` / `pb-next-key` jump over
//!   `playback_controls::key_frames` — camera keys union
//!   graph keys, sorted and deduped. The demo project
//!   has neither (asserted as the tests' premise), and
//!   the camera panel's `cam-set-key` control keys the
//!   camera at the current frame (its handler is
//!   `rotate_camera(0.0)`, which sets a key), so two
//!   presses at two frames build the exact jump list
//!   [1, 3] the transport test walks. Prev at the first
//!   key is a no-op (strict inequality), never a wrap.
//! * A timeline marker cell (`marker-cam-{frame}`) is
//!   presented only for a frame a camera key sits on,
//!   and its press is `core.goto(frame)` — the chips'
//!   own seek, on a separate element the chips' drag
//!   gesture never sees.
//! * The pose panel's Paste pose button is the control
//!   that applies the clipboard pose; with an empty
//!   clipboard it renders through the section's inert
//!   branch, which attaches NO press handler — a press
//!   routes nowhere, so the honest assertions are
//!   presence plus a tap that changes nothing (status,
//!   undo depth, current frame, joint positions).
//!
//! Round 7 (Phase J, section 37) adds four tests: two
//! GEOMETRIC regressions for the Phase I screenshot's
//! defects, the timeline ruler, and the palette's
//! mirror-figure command. Facts they rest on, verified
//! against the tree:
//! * The screenshot's Keyframes overlap is fixed by the
//!   keyframe section giving the no-joint note its own
//!   row: `keyframe-no-joint-note` (the section's
//!   `NO_JOINT_NOTE_ROW_KEY`), with the actions row
//!   still keyed `keyframe-actions` and the Delete key
//!   button still keyed `key-delete`. The note's text
//!   element itself is unkeyed, but `Runtime::elements`
//!   reports every element's text and drawn rect
//!   (`ElementInfo.rect`, runtime/elements.rs), and
//!   "No joint selected" is unique shell-wide — so the
//!   test can assert on the TEXT's rect, not just its
//!   row's. A fresh core selects no joint
//!   (`EditorCore::new` seeds `selected_node: None`),
//!   which is the note's premise.
//! * The screenshot's status-strip overlap is fixed in
//!   the shell's column split (main.rs `view`): the
//!   side viewports keep fixed widths (300 / 340) and
//!   the centre — which carries the strip, keyed
//!   `status-strip` by the status section — takes the
//!   rest, instead of all three columns growing to
//!   equal thirds and the strip's floored tail painting
//!   over the right column. The right column's own root
//!   is keyed `side-right`, so the strip's right edge
//!   can be asserted against the column's left edge
//!   directly, at this file's 1440×900 viewport (the
//!   Runtime's window — `AppSpec`'s own size).
//! * The timeline's time ruler is keyed
//!   `timeline-ruler` (timeline_marks), one slot per
//!   chip in the chips' geometry (`timeline-chips`
//!   below it, same left edge); frame 0 always carries
//!   the label "0:00" (the section's `ruler_label`).
//! * `mirror-figure` is a palette command
//!   (command_palette's Phase J block) whose shim calls
//!   `EditorCore::mirror_figure_all_frames`. That op's
//!   transform — the core's own mirror definition, in
//!   figure_ops and editor.rs — negates every pose
//!   angle on every frame and leaves the root where it
//!   is: the evaluated figure reflects about the
//!   VERTICAL AXIS THROUGH ITS ROOT, so a joint at
//!   (x, y) lands at (2·root_x − x, y). It does NOT
//!   move the root about the stage centre; the test
//!   asserts the real contract (root unmoved, angles
//!   negated, a witness joint reflected) rather than
//!   the stage-centre reading.
//!
//! Round 8 (Phase K, section 37) adds four tests over
//! the Phase J behaviours rounds 1–7 asserted only at
//! unit level or in the headless tree dump: the column
//! split as a layout pin, the figure inspector's
//! Centre button, Pose tools' Mirror-all button, and
//! the FX inspector's Split button. Facts they rest
//! on, verified against the tree:
//! * The stage's 744-pt width is the Phase J column
//!   split measured at this file's own viewport: the
//!   window's 1440 minus margins and gaps and the two
//!   fixed side widths (300 / 340) the split gave the
//!   columns. The sidebar visibility flags are
//!   unpersisted shell state (`init` defaults both
//!   shown — round 3's own note), so a fresh Runtime
//!   always lays out the full split; the pin is a
//!   plain presented-rect assert, no settling.
//! * `fig-centre` calls the figure section's
//!   `centre_selected_figure`, which shifts ONLY the
//!   current frame's root (through `nudge_path_point`)
//!   by the delta its own `figure_bbox` helper
//!   computes — so the test reads the before/after
//!   bbox through that same helper (an exact-name
//!   import, like round 6's `key_frames`) and asserts
//!   the centre lands on the stage centre while frame
//!   1's root stays untouched. The premise is the
//!   demo's own geometry: root [480, 470] against a
//!   [960, 720] stage, so the bbox centre starts well
//!   below the stage centre on y.
//! * No test in this file writes core state during
//!   setup — every round drives real controls and
//!   reads `state.core` only for assertions — and
//!   Mirror-all needs no write either: `new_demo`'s
//!   wave pose carries non-zero angles by
//!   construction (angles[5] is −118° on frame 0), so
//!   the test asserts that premise, then the button's
//!   effect: every angle on every frame negated
//!   exactly (round 7's contract, driven here through
//!   the Pose tools button instead of the palette),
//!   every root byte-identical.
//! * The FX split is driven end to end through the
//!   panels a user uses. Adding a clip needs a
//!   selected joint (`add_fx_clip` refuses without
//!   one), selected by a stage press at the root
//!   joint's drawn point — round 1's aiming idiom,
//!   press only, no move. The browser's kind button
//!   (`fxclip-burst`) adds the clip at the current
//!   frame with the add path's fixed 16-frame
//!   duration; the FX lane's row (`fx-clip-0`) selects
//!   it (the browser add does NOT select); a timeline
//!   chip parks the playhead mid-clip; and `fx-split`
//!   — rendered only inside the inspector's
//!   selected-clip detail — cuts at the playhead
//!   frame's centre (frame 2 → 2.5), the first clip
//!   keeping [0, 2.5) and the new one taking the rest.
//!
//! Round 9 (Phase L, section 26) adds four tests over
//! the Phase K/L surfaces rounds 1–8 never drove
//! through the Runtime: the transport's speed cycle,
//! the figure inspector's head-ring stepper, the
//! timeline's own loop row, and the 3D panel's new
//! preview viewport. Facts they rest on, verified
//! against the tree:
//! * `pb-speed` lives in the Playback panel (Animate
//!   and Audio workspaces) and its handler is
//!   `playback_controls::cycle_playback_speed`, which
//!   only rewrites `State::playback_speed` — shell
//!   state `init` seeds at 1.0 and nothing persists —
//!   through the `SPEEDS` table, whose next step above
//!   1.0 is 1.5. The button stays mounted after the
//!   press; only its label (the speed itself) and its
//!   active flag change, so the test asserts the field
//!   and the key's continued presence, never pixels.
//! * `fig-ring-up` is the figure inspector's head-ring
//!   stepper: `step_head_ring_scale` adds
//!   `HEAD_RING_STEP` (0.1) and hands the sum to
//!   `EditorCore::set_head_ring_scale`, which writes
//!   `Project::head_ring_scale` (a fresh demo's field
//!   is exactly 1.0, model.rs) under a [0.5, 2.0]
//!   clamp — one press therefore lands on 1.1, above
//!   1.0 by the step, nowhere near the clamp.
//! * The timeline's loop row (`timeline_marks`'
//!   `loop_row`, Phase L) renders under the chips in
//!   the centre column: with no loop range it carries
//!   only `loop-set` ("Set loop"), whose handler asks
//!   for `loop_set_range(current, frame_count)` —
//!   from the playhead, four frames on, clamped to
//!   the last frame; the demo (playhead 0, five
//!   frames) gets (0, 4). With a range the row swaps
//!   to the readout, endpoint steppers and
//!   `loop-clear`. That key is shared by three homes
//!   (this row, the Playback panel, the Frames
//!   section) — `find` returns the first in tree
//!   order, and the centre column mounts before the
//!   right column, so after Set loop the timeline
//!   row's own Clear is the one a tap lands on; all
//!   three handlers call the same core op
//!   (`clear_loop_range`) in any case. No absence
//!   assert is made on `loop-clear`: the other homes
//!   render it whatever the range, so only the state
//!   transitions and `loop-set`'s return are honest
//!   signals.
//! * The 3D preview is a canvas keyed
//!   `panel-3d-preview` (`three_d_panel::PREVIEW_KEY`)
//!   mounted unconditionally at the top of the 3D
//!   panel — with no session it draws an empty shape
//!   over its field, so presence needs no session
//!   setup. The `ThreeD` section is gated to the 3D
//!   workspace alone, whose chip key is `ws-3d`
//!   (workspaces.rs — the persisted name is "3D", the
//!   key is not `ws-build3d`). Presence is asserted
//!   on the presented tree, round 5's coverage-test
//!   mechanism, not on pixels.
//!
//! Round 10 (Phase M, section 35) adds four tests over
//! the phase's new surfaces: the Frames section's swap
//! buttons, the 3D panel's session toggle, the
//! settings card's direct-select speed chips, and the
//! timeline's clips lane. Facts they rest on:
//! * The swap buttons are a live/inert pair under one
//!   key (`fop-swap-next`): `icon_btn` while
//!   `swap_next_enabled` holds, `icon_btn_inert` at
//!   the last frame — so the end-of-timeline assertion
//!   is round 6's inert pattern (presence, then a tap
//!   that changes nothing), not absence. The swap
//!   itself is `frame_swap::swap_frame_with_neighbour`:
//!   whole `Frame` values trade slots (durations
//!   travel), the playhead keeps its slot, and a
//!   success takes exactly one snapshot. Durations are
//!   planted through the section's own ×2 stepper
//!   (`fop-dur-double` → `adjust_frame_duration`, a
//!   direct field write with no snapshot), never by
//!   writing project state in setup.
//! * The 3D panel's header toggle (`mode-3d`) enters
//!   and exits the session (`EditorCore::enter_3d` /
//!   `exit_3d`), so session state is assertable off
//!   `rt.state.core.fig3` — round 9's preview-presence
//!   test needed no session at all.
//! * The settings card is a right-column panel (not an
//!   overlay), so its deep rows — the Playback block's
//!   speed chips, `settings-speed-{i}` over the
//!   transport's `SPEEDS` table — are reached with the
//!   same `scroll_into_view` discipline as any other
//!   right-column row.
//! * The clips lane (`timeline-clips`, timeline_marks)
//!   renders only when the project carries at least
//!   one audio or FX clip span, so the lane test
//!   plants an FX clip by direct project write (the
//!   graph-duplicates precedent) and asserts presence,
//!   then clears it and asserts absence after a
//!   bounded settle loop. (This test was added by
//!   the integrator: at section 35's census the lane
//!   had not landed yet, and this file never asserts
//!   against an unverified key.)
//!
//! Round 11 (Phase N, verification wave) adds ten tests
//! over the Phase N card restructure: every panel root
//! became a `design::panel_col` card, the timeline stack
//! moved into one unkeyed card, the menus / palette /
//! settings surfaces were restyled onto the kit, and the
//! root column gained the charter §15 hairline. The
//! round's discipline is P17's (§0 of the plan): the
//! Runtime sees keys, rects and opacity — never fills,
//! borders or radii — so these tests prove the
//! restructure kept every surface MOUNTED, REACHABLE and
//! NON-OVERLAPPING; the look itself is the headless
//! snaps' verdict, not this file's. Facts they rest on,
//! verified against the adopted tree:
//! * Geometry is asserted as RELATIONS (containment,
//!   ordering, non-overlap), never as fresh absolute
//!   pins. The stage's height is the one number the
//!   phase provably moves: the root column's new
//!   hairline costs ≈13 pt (the 1 pt divider plus its
//!   12 pt gap) and the timeline card ≈18 pt (8+8
//!   padding, 2×1 border), both absorbed by the stage
//!   as the centre column's grow child — down from the
//!   Phase L tree's 522 pt to roughly 491 pt, the exact
//!   figure being a headless-tree reading for the
//!   integrator, not a pin taken here. The WIDTH is not
//!   moved: the column contract in main.rs (root
//!   padding 16, viewports 300 / 340, row gaps 12) is
//!   unchanged, so round 8's 744 pt width pin stands
//!   unadjusted. It is the only absolute geometry pin
//!   in the file; no other existing test needed one.
//! * The FX clips panel's card root is keyed
//!   `fx-section` (fx_clips.rs); `fx-clips` is the
//!   inner lane list inside it, built unconditionally.
//!   P17's draft named the root `fx-clips` — the tests
//!   assert the real root and the list both.
//! * P17's conditional empty-state test is NOT
//!   written: the kit's `empty_state` signature
//!   (design.rs) carries no key parameter, so there is
//!   no key to assert and none was invented (the
//!   charter allows new keys only where a brief names
//!   one). Empty states are snap-proven only.
//! * P17's conditional help test IS written: the help
//!   panel's card root kept its `help-panel` key
//!   through adoption, and Help ▸ Shortcuts (the
//!   dropdown item keyed `help`, toggling
//!   `menus.help_open`) mounts it at the top of the
//!   right column (main.rs `view`).
//! * The Edit menu's separator is the kit's
//!   `divider_keyed` under its surviving key
//!   `menu-edit-sep` (menus.rs) — the charter §9
//!   divider in its keyed form, asserted presented
//!   while the Edit dropdown is open.
//!
//! Round 12 (Wave 11, MCP end-to-end) adds four tests
//! over the live-MCP-visible shell surface — the facts
//! an external MCP client's session depends on, pinned
//! at the Runtime level (the socket hop itself is
//! `live_mcp`'s own e2e proof in the lib; this file
//! proves what the SHELL does with what the bridge
//! delivers). Facts they rest on, verified against the
//! tree:
//! * `live_mcp::drain` runs a queued request as
//!   `McpServer::handle_on(&mut core, &msg)`
//!   (live_mcp.rs:346) — so calling `handle_on` against
//!   `rt.state.core` is the exact mutation a live call
//!   makes, minus the TCP hop. Direct state writes
//!   follow the file's discipline: `rt.invalidate()`
//!   plus a frame before any tree assert.
//! * `poll` (main.rs) toasts a status the DRAIN wrote
//!   with the toasts module's `MCP: ` attribution
//!   prefix stamped at the push site only — never into
//!   `core.status`, which stays the bare engine text
//!   (it is also an MCP API return value). A status
//!   written any other way — including a bare
//!   `handle_on` in these tests — toasts UNSTAMPED;
//!   the stamped forms are pinned here through
//!   `push_status`, the exact call `poll` makes with
//!   them, against `classify_status`'s rule 0
//!   (toasts.rs): the prefix reads through to the
//!   body's own severity, and the MCP-layer failure
//!   shapes (`unknown tool: …`) are Error outright.
//! * The Settings card's Live MCP row
//!   (`settings-live-mcp`, settings_popover.rs) reads
//!   the bridge itself — `live_mcp::is_enabled()`, no
//!   mirrored bool — and `init` (main.rs) starts the
//!   bridge from the persisted `LayoutPrefs.live_mcp`.
//!   This suite has NO prefs-seeding pattern (scenes
//!   set state through handlers, never the file), so
//!   the settings test seeds `./projects/.layout.json`
//!   through `shell::save_layout` — load-modify-save,
//!   the app's own write path — and restores the file
//!   and the bridge afterwards. The toggle's state is
//!   observable in the tree only as its pill's knob
//!   (design.rs `toggle_pill`: a 12×12 knob at inset 2
//!   in a 28 pt pill — centre 20 pt from the row's
//!   right edge when off, 8 pt when on; the gap is
//!   translation-invariant, so the card's entrance
//!   transition cannot move the reading).
//!
//! Round 13 (Wave 12) adds three tests over the state
//! this wave made visible in the shell. Facts they
//! rest on, verified against the tree:
//! * The stage draws EVERY figure segment through
//!   `paint::styled_segment_pieces` (stage_view.rs —
//!   the one construction the stage and both exporters
//!   now share), so a node's `NodeStyle` (model.rs:
//!   taper, `CapKind`, opacity) reaches the canvas
//!   pieces directly. The pieces are canvas draws, not
//!   elements: the Runtime tree cannot see their
//!   widths or alphas, so the styled-stage test pins
//!   what the Runtime CAN prove — the style state the
//!   core ops write (through round 12's `mcp_call`,
//!   the registered `set_node_taper` / `set_node_style`
//!   tools), the stage mounted at real size over the
//!   styled project across settled frames, and joint
//!   press selection still landing on the styled
//!   figure. Piece geometry itself is the lib's own
//!   unit-test surface (paint.rs, stage_view.rs).
//! * The segment inspector's Media section
//!   (inspector_segment.rs) renders for the selected
//!   node: readout `seg-media-readout` (its text is
//!   `media_label` — `Sprite: {name}` for an attached
//!   sprite), one `seg-media-attach-{i}` button per
//!   asset in `project.assets`, and `seg-media-clear`
//!   only while media is attached. The attach press
//!   calls the module's pub `attach_asset`.
//! * The Home panel's rows come from a DISK scan of
//!   `./projects` (`HomePanelState::projects` —
//!   `projects::scan`), not from core state, so the
//!   populated-Home test seeds through the app's own
//!   write path — `EditorCore::save_named_project`,
//!   twice under two names — exactly as the scene
//!   seeding must, then removes its two files again.
//!   Rows key as `home-row-{i}` under `home-rows`,
//!   with the `home-totals` footer in the non-empty
//!   branch only, and the empty branch's line ("No
//!   saved projects yet — …") absent. The panel's
//!   `open` flag starts false without a recovery
//!   offer, so the test sets it directly (a State
//!   write, with the file's invalidate discipline).
//!
//! Round 14 (Wave 13) adds two tests over the surfaces
//! this wave lands. Facts they rest on:
//! * The engine side of sprites is landed:
//!   `EditorCore::add_sprite` (sprite_ops.rs) pushes
//!   one `SpriteInstance` holding a single key at
//!   frame 0 — stage centre, scale 1.0, angle 0.0 —
//!   and `remove_sprite` deletes the whole instance;
//!   both are one snapshot each, refusals status-only.
//!   The panel (A3, in flight) forwards to those ops.
//! * The panel's keys, reconciled against A3's
//!   landed source (sprites_panel.rs) during this
//!   wave: the panel roots at `sprite-panel`; the
//!   add-from-assets picker renders one button per
//!   asset, `sprite-add-{a}`, under the `sprite-add`
//!   container (its press is `add_from_asset` — the
//!   op plus selection of the new instance); rows are
//!   `sprite-row-{i}` shells holding `sprite-select-{i}`
//!   and the per-row `sprite-del-{i}`. Which workspace
//!   hosts the panel is E8's gating call, in flight
//!   when this round was written, so the sprite test
//!   DISCOVERS the host: it walks the eight workspace
//!   chips and settles where `sprite-panel` presents,
//!   rather than presuming the gating. With exactly
//!   one asset seeded (a direct `project.assets`
//!   push — round 13's media-test discipline; the
//!   asset table is in-project state, no files are
//!   involved), the picker offers exactly one button,
//!   `sprite-add-0`.
//! * Library rows render one thumbnail canvas per
//!   entry, keyed `library-thumb-{i}` with `i` the
//!   entry's index in `core.library`
//!   (library_panel.rs `entry_row`); the panel is
//!   gated to the Animate workspace (`Sec::Library`).
//!   `core.library` always carries the three built-ins
//!   from `load_library`; the populated state D1's
//!   library-populated scene pins is entries added in
//!   memory, so the library test mirrors the scene —
//!   it clones a def, styles one node and pushes two
//!   entries directly (a State write, with the file's
//!   invalidate discipline). No library files are
//!   read or written, so there is nothing to clean up.
//! * The sprite panel's column is E8's choice, so
//!   round 14's scroll helper wheels over the target's
//!   own column (its `find` centre x), generalising
//!   round 6's right-column-only `scroll_into_view`.

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
fn stage_rect_is_identical_with_a_menu_open() {
    let mut rt = editor();
    // Seed the Animate workspace through its chip, as the
    // shell test above does (the persisted prefs are
    // shared across the test process).
    tap(&mut rt, "ws-animate");
    rt.frame(DT);

    // The closed pin: at this file's 1440×900 viewport
    // the stage is exactly the rect the headless tree
    // dump measures in the real binary — the centre
    // column's share after margins, gaps, the fixed
    // 300 / 340 side widths and the centre's own rows.
    // Wave 10: 530 -> 504 — the dopesheet lane joined
    // the timeline card (header + the demo's one figure
    // row) while the empty marker row stopped mounting;
    // the timeline's net growth is the stage's 26 pt.
    let (closed, _) = rt.presented("stage").expect("stage is presented");
    for (want, got) in [328.0, 157.0, 744.0, 504.0].iter().zip(closed.iter()) {
        assert!(
            (want - got).abs() <= 1.0,
            "closed stage rect pins [328, 157, 744, 504]: {closed:?}"
        );
    }

    // Open the File menu (a settling frame for the
    // anchored dropdown, round 2's discipline) and read
    // the stage again: the rect must not move. Before
    // the Wave 8 fix the scrim/dropdown builders WERE
    // the root column's last two in-flow children while
    // closed and absolute layers while open, so opening
    // a menu freed two 12 pt root gaps and the stage
    // grew 24 pt ([328, 157, 744, 554] in the dump) for
    // as long as the menu was up; the shell now keeps
    // zero-size spacers in those slots in both states
    // and appends the layers only while open.
    tap(&mut rt, "menu-file");
    rt.frame(DT);
    assert_eq!(rt.state.menus.open, Some(crate::rune_sec::menus::FILE));
    assert!(
        presented_keys(&rt).contains("menu-file-dropdown"),
        "dropdown surface presented"
    );
    let (open, _) = rt
        .presented("stage")
        .expect("stage is presented with a menu open");
    for (a, b) in closed.iter().zip(open.iter()) {
        assert!(
            (a - b).abs() <= 0.5,
            "stage rect moved while a menu was open: closed {closed:?}, open {open:?}"
        );
    }

    // And back: a scrim press closes the menu (the
    // dropdown's presence fade is an anchored overlay —
    // out of flow at every instant), and the stage is
    // still exactly where it started.
    tap(&mut rt, "menu-scrim");
    assert_eq!(rt.state.menus.open, None, "scrim press closes the menu");
    rt.frame(DT);
    let (settled, _) = rt.presented("stage").expect("stage is presented");
    for (a, b) in closed.iter().zip(settled.iter()) {
        assert!(
            (a - b).abs() <= 0.5,
            "stage rect moved after the menu closed: closed {closed:?}, settled {settled:?}"
        );
    }

    // Wave 9 strengthening: the Edit menu gets the same
    // pass. Its dropdown is a different surface (the kit
    // divider and the frame-clipboard items) mounted
    // through the same two root slots, so the invariant
    // must hold for it too, not only for File — a
    // per-menu mount difference is exactly what this
    // test exists to catch.
    tap(&mut rt, "menu-edit");
    rt.frame(DT);
    assert_eq!(rt.state.menus.open, Some(crate::rune_sec::menus::EDIT));
    assert!(
        presented_keys(&rt).contains("menu-edit-dropdown"),
        "Edit dropdown surface presented"
    );
    let (edit_open, _) = rt
        .presented("stage")
        .expect("stage is presented with the Edit menu open");
    for (a, b) in closed.iter().zip(edit_open.iter()) {
        assert!(
            (a - b).abs() <= 0.5,
            "stage rect moved while the Edit menu was open: closed {closed:?}, open {edit_open:?}"
        );
    }

    // And back again, the same scrim close as File's.
    tap(&mut rt, "menu-scrim");
    assert_eq!(rt.state.menus.open, None, "scrim press closes the Edit menu");
    rt.frame(DT);
    let (edit_settled, _) = rt.presented("stage").expect("stage is presented");
    for (a, b) in closed.iter().zip(edit_settled.iter()) {
        assert!(
            (a - b).abs() <= 0.5,
            "stage rect moved after the Edit menu closed: closed {closed:?}, settled {edit_settled:?}"
        );
    }
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
    // Normalise the layout-pref-seeded fields (Focused
    // Wave D1 wired them): `init` seeds `show_left` from
    // the persisted `sidebar_visible` and the timeline
    // flag from `timeline_collapsed`, both in the layout
    // file every test in this process shares — a sibling
    // test's persisted sidebar-off would otherwise hide
    // the Figures column this test asserts on. Force the
    // defaults, then render once with them in place.
    rt.state.show_left = true;
    rt.state.show_right = true;
    rt.state.frame_ops.timeline_collapsed = false;
    rt.frame(DT);
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
    // The row renders always, empty project or not.
    // Bring it into the right column's viewport and
    // settle through the freshly mounted panel's
    // presence fade before pressing inside it — the
    // draw panel grew in Wave 9 (labelled stamp
    // groups), which can leave the row below the fold,
    // where a press cannot land.
    scroll_into_view(&mut rt, "stroke-sel-next");
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
        "save-copy-proj",
        "recover",
    ] {
        assert!(keys.contains(want), "missing audit-map key {want:?}");
    }
}

// ----------------------------------------------------------------
// Round 6 (Phase I, section 39): palette workspace jumps,
// transport keyframe jumps, timeline markers, and the pose
// panel's inert Paste control.
// ----------------------------------------------------------------

/// Scroll the right column until the element keyed `key`
/// sits inside the viewport, then settle (bounded) until
/// it is fully presented — round 4's particles pattern,
/// generalised: the column's wheel target and step are
/// the ones that test proved, and the settle is the
/// presence-fade courtesy rounds 3–5 use before pressing
/// inside a freshly mounted panel.
fn scroll_into_view(rt: &mut Runtime<State>, key: &str) {
    // Phase N: the helper scrolls BOTH ways. The P16 mount
    // order put Playback above Camera, so a test that first
    // scrolls deep (to the camera panel) can leave its next
    // target ABOVE the viewport, where find() reports a
    // negative centre y — which the old one-way check read
    // as "already in view" and never scrolled at all.
    for _ in 0..24 {
        let (_, y) = rt
            .find(key)
            .unwrap_or_else(|| panic!("no element keyed {key:?}"));
        if y > 850.0 {
            rt.wheel(1255.0, 450.0, -600.0);
        } else if y < 200.0 {
            rt.wheel(1255.0, 450.0, 600.0);
        } else {
            break;
        }
        rt.frame(DT);
    }
    for _ in 0..90 {
        rt.frame(DT);
        if let Some((_, opacity)) = rt.presented(key) {
            if opacity > 0.99 {
                break;
            }
        }
    }
}

#[test]
fn palette_workspace_jump_switches_workspace() {
    use crate::rune_sec::command_palette::{display_commands, COMMANDS};
    use crate::rune_sec::workspaces::Workspace;

    let mut rt = editor();
    // Seed Animate through its chip (the shared-prefs
    // discipline), and settle out whatever panels a
    // sibling-restored workspace left fading.
    tap(&mut rt, "ws-animate");
    rt.frame(DT);
    assert_eq!(rt.state.workspace, Workspace::Animate);
    for _ in 0..30 {
        if !presented_keys(&rt).contains("export-panel") {
            break;
        }
        rt.frame(DT);
    }
    // The gating table: Export is {Export, Stats, Project}
    // only, so Animate mounts FrameOps (`frames`, round 3's
    // Animate-only key) and never the Export centre.
    assert!(
        presented_keys(&rt).contains("frames"),
        "Animate mounts FrameOps"
    );
    assert!(
        !presented_keys(&rt).contains("export-panel"),
        "Animate gates the Export centre out"
    );

    // Open the palette the round 2/5 way and type the
    // query through the real input path.
    press_key(
        &mut rt,
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
    for c in "export".chars() {
        type_char(&mut rt, c);
    }
    assert_eq!(
        rt.state.palette.query.text, "export",
        "typed characters landed in the query field"
    );

    // Locate the jump command in the palette's own
    // listing for this query — the same `display_commands`
    // `run_selected` resolves through — and walk the
    // highlight to its row with Down, exactly as round
    // 5's loop-toggle test drove the palette.
    let (results, _) =
        display_commands(&rt.state.palette.query.text, &rt.state.palette.recent);
    let row = results
        .iter()
        .position(|&ci| COMMANDS[ci].key == "go-export")
        .expect("the export query lists go-export (the Phase I jump command)");
    let len = results.len();
    let cur = rt.state.palette.selected % len;
    for _ in 0..(row + len - cur) % len {
        press_key(&mut rt, Key::Down, Mods::default());
    }
    assert_eq!(
        rt.state.palette.selected, row,
        "the highlight sits on the jump command's row"
    );

    // Enter runs it: the palette closes, the workspace
    // field flips, the Export centre mounts and the
    // Animate-only FrameOps panel leaves (settled,
    // bounded, before the absence assert).
    press_key(&mut rt, Key::Enter, Mods::default());
    assert!(
        !rt.state.palette.open,
        "running a command closes the palette"
    );
    assert_eq!(
        rt.state.workspace,
        Workspace::Export,
        "go-export switched the workspace"
    );
    assert!(
        presented_keys(&rt).contains("export-panel"),
        "Export mounts the Export centre"
    );
    for _ in 0..30 {
        if !presented_keys(&rt).contains("frames") {
            break;
        }
        rt.frame(DT);
    }
    assert!(
        !presented_keys(&rt).contains("frames"),
        "Export gates FrameOps out"
    );

    // Restore Animate for the shared prefs, as rounds 2–5 do.
    tap(&mut rt, "ws-animate");
    assert_eq!(rt.state.workspace, Workspace::Animate);
}

#[test]
fn prev_next_key_jumps_between_camera_keys() {
    use crate::rune_sec::playback_controls::key_frames;
    use crate::rune_sec::workspaces::Workspace;

    let mut rt = editor();
    tap(&mut rt, "ws-animate");
    rt.frame(DT);
    assert_eq!(rt.state.workspace, Workspace::Animate);
    // Premise: the demo project carries no camera keys
    // and no graph keys, so the transport's jump list
    // starts empty and everything below is this test's
    // own doing.
    assert!(
        key_frames(&rt.state.core).is_empty(),
        "demo project has no key frames"
    );

    // Key the camera on frames 1 and 3 through the camera
    // panel's own Set key control (`cam-set-key`; its
    // handler keys the camera at the current frame). The
    // camera panel sits in the right column, below the
    // fold — scroll it into view before pressing.
    tap(&mut rt, "frame-2");
    assert_eq!(rt.state.core.current, 1);
    scroll_into_view(&mut rt, "cam-set-key");
    tap(&mut rt, "cam-set-key");
    tap(&mut rt, "frame-4");
    assert_eq!(rt.state.core.current, 3);
    scroll_into_view(&mut rt, "cam-set-key");
    tap(&mut rt, "cam-set-key");
    assert_eq!(
        key_frames(&rt.state.core),
        vec![1, 3],
        "both Set key presses landed"
    );

    // From frame 0 the Next key button walks the keys in
    // order; the Playback panel is further down the same
    // column, so scroll to it first.
    tap(&mut rt, "frame-1");
    assert_eq!(rt.state.core.current, 0);
    scroll_into_view(&mut rt, "pb-next-key");
    tap(&mut rt, "pb-next-key");
    assert_eq!(
        rt.state.core.current, 1,
        "Next key lands on the first key"
    );
    tap(&mut rt, "pb-next-key");
    assert_eq!(
        rt.state.core.current, 3,
        "Next key lands on the second key"
    );

    // Prev key walks back — and at the first key it is a
    // no-op, not a wrap (the section's strict inequality).
    tap(&mut rt, "pb-prev-key");
    assert_eq!(
        rt.state.core.current, 1,
        "Prev key returns to the first key"
    );
    tap(&mut rt, "pb-prev-key");
    assert_eq!(
        rt.state.core.current, 1,
        "Prev key at the first key is a no-op"
    );
}

#[test]
fn timeline_marker_press_seeks() {
    use crate::rune_sec::workspaces::Workspace;

    let mut rt = editor();
    tap(&mut rt, "ws-animate");
    rt.frame(DT);
    assert_eq!(rt.state.workspace, Workspace::Animate);
    // Premise: no camera keys in the demo project, so no
    // camera marker is presented for any frame.
    assert!(
        rt.state.core.project.camera.is_empty(),
        "demo project starts without camera keys"
    );
    assert!(
        !presented_keys(&rt).contains("marker-cam-2"),
        "no camera marker before a key exists"
    );

    // Put a camera key on frame 2 through the camera
    // panel's Set key control, as the transport test does.
    tap(&mut rt, "frame-3");
    assert_eq!(rt.state.core.current, 2);
    scroll_into_view(&mut rt, "cam-set-key");
    tap(&mut rt, "cam-set-key");
    assert!(
        presented_keys(&rt).contains("marker-cam-2"),
        "the key's marker cell is presented"
    );

    // Seek away with a chip, then press the marker: its
    // press is a plain `goto` to its own frame.
    tap(&mut rt, "frame-1");
    assert_eq!(rt.state.core.current, 0);
    tap(&mut rt, "marker-cam-2");
    assert_eq!(
        rt.state.core.current, 2,
        "the marker press seeks to its frame"
    );
}

#[test]
fn pose_panel_inert_apply_is_present_without_a_pose() {
    use crate::rune_sec::workspaces::Workspace;

    let mut rt = editor();
    tap(&mut rt, "ws-animate");
    rt.frame(DT);
    assert_eq!(rt.state.workspace, Workspace::Animate);
    // Premise: nothing has been copied in this fresh
    // shell, so Paste pose — the control that applies the
    // clipboard pose — renders in its inert form. Inert
    // and live share the key `pose-paste`; the inert form
    // differs only visually (muted, and no press handler
    // is attached at all), so presence plus a tap that
    // changes nothing is the honest assertion.
    assert!(
        rt.state.pose_clipboard.is_none(),
        "no pose on the clipboard"
    );

    // The pose panel sits in the right column, below the
    // fold — scroll it into view before pressing.
    scroll_into_view(&mut rt, "pose-paste");
    let keys = presented_keys(&rt);
    assert!(keys.contains("pose-tools"), "Animate mounts Pose tools");
    assert!(
        keys.contains("pose-paste"),
        "the inert Paste pose control is presented"
    );

    // The tap routes nowhere: status, undo depth, the
    // current frame and the figure's joint positions are
    // all exactly as they were, and the clipboard stays
    // empty.
    let status = rt.state.core.status.clone();
    let undo = rt.state.core.undo_depth();
    let current = rt.state.core.current;
    let joints = rt.state.core.positions(current, 0);
    tap(&mut rt, "pose-paste");
    assert_eq!(
        rt.state.core.status, status,
        "the inert press writes no status"
    );
    assert_eq!(
        rt.state.core.undo_depth(),
        undo,
        "the inert press adds no undo step"
    );
    assert_eq!(
        rt.state.core.current, current,
        "the inert press does not seek"
    );
    assert_eq!(
        rt.state.core.positions(current, 0),
        joints,
        "the inert press leaves the pose untouched"
    );
    assert!(
        rt.state.pose_clipboard.is_none(),
        "the clipboard stays empty"
    );
}

// ----------------------------------------------------------------
// Round 7 (Phase J, section 37): geometric regressions
// for the Phase I screenshot's two overlaps, the
// timeline ruler, and the palette's mirror-figure
// command.
// ----------------------------------------------------------------

/// Do two drawn rects (`[x, y, w, h]`, window points)
/// share any area? Touching edges do NOT count as an
/// overlap — neighbouring rows and columns abut by
/// design; the screenshot defects were one element
/// painting ACROSS another, which is strict area
/// overlap.
fn rects_intersect(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
}

/// Is `inner` fully inside `outer` (half a point of
/// slack for rounding at the edges)?
fn rect_inside(inner: [f32; 4], outer: [f32; 4]) -> bool {
    inner[0] >= outer[0] - 0.5
        && inner[1] >= outer[1] - 0.5
        && inner[0] + inner[2] <= outer[0] + outer[2] + 0.5
        && inner[1] + inner[3] <= outer[1] + outer[3] + 0.5
}

#[test]
fn keyframes_note_and_delete_button_do_not_overlap() {
    use crate::rune_sec::workspaces::Workspace;

    let mut rt = editor();
    tap(&mut rt, "ws-animate");
    rt.frame(DT);
    assert_eq!(rt.state.workspace, Workspace::Animate);
    // Premise: a fresh core selects no joint, so the
    // Keyframes section shows its no-joint note — the
    // state the Phase I screenshot was taken in, where
    // the note's text painted across the Delete key
    // button below it.
    assert_eq!(
        rt.state.core.selected_node, None,
        "no joint selected in a fresh shell"
    );

    // The Keyframes panel sits in the right column,
    // below the fold — scroll the Delete button into
    // view (the note row sits directly above it).
    scroll_into_view(&mut rt, "key-delete");
    let keys = presented_keys(&rt);
    for want in [
        "keyframe-tools",
        "keyframe-no-joint-note",
        "keyframe-actions",
        "key-delete",
    ] {
        assert!(keys.contains(want), "missing element keyed {want:?}");
    }

    let (note_row, note_opacity) = rt
        .presented("keyframe-no-joint-note")
        .expect("the note row is presented");
    let (actions, _) = rt
        .presented("keyframe-actions")
        .expect("the actions row is presented");
    let (delete, delete_opacity) = rt
        .presented("key-delete")
        .expect("the Delete key button is presented");
    assert!(note_opacity > 0.99, "note row fully presented");
    assert!(delete_opacity > 0.99, "Delete button fully presented");

    // The note's TEXT element is unkeyed; find it by its
    // (shell-wide unique) text in the elements list,
    // which reports the drawn rect of every element.
    let note_text = rt
        .elements()
        .into_iter()
        .find(|e| e.text.as_deref() == Some("No joint selected"))
        .expect("the note text is presented");

    // The text stays inside its own row…
    assert!(
        rect_inside(note_text.rect, note_row),
        "note text stays inside its own row: text {0:?}, row {note_row:?}",
        note_text.rect
    );
    // …the row and the actions row are separate rows,
    // the note's row wholly above the actions row…
    assert!(
        !rects_intersect(note_row, actions),
        "note row and actions row overlap: note {note_row:?}, actions {actions:?}"
    );
    assert!(
        note_row[1] + note_row[3] <= actions[1] + 0.5,
        "note row sits above the actions row: note {note_row:?}, actions {actions:?}"
    );
    // …and neither the row nor the text itself touches
    // the Delete key button — the screenshot's defect.
    assert!(
        !rects_intersect(note_row, delete),
        "note row overlaps the Delete button: note {note_row:?}, delete {delete:?}"
    );
    assert!(
        !rects_intersect(note_text.rect, delete),
        "note text paints across the Delete button: text {0:?}, delete {delete:?}",
        note_text.rect
    );
}

#[test]
fn status_strip_is_not_covered_by_the_right_column() {
    use crate::rune_sec::workspaces::Workspace;

    let mut rt = editor();
    tap(&mut rt, "ws-animate");
    rt.frame(DT);
    assert_eq!(rt.state.workspace, Workspace::Animate);

    // The strip (keyed `status-strip` by its section)
    // lives at the bottom of the CENTRE column; the
    // right column's root is keyed `side-right`. The
    // Phase I screenshot's defect: the strip ran the
    // window's full width under the right column, whose
    // bottom rows painted over its tail segments.
    let (strip, _) = rt.presented("status-strip").expect("status strip is presented");
    let (right, _) = rt
        .presented("side-right")
        .expect("right column is presented");

    // The strip lies inside the window…
    assert!(
        strip[0] >= 0.0
            && strip[1] >= 0.0
            && strip[0] + strip[2] <= VIEWPORT.0
            && strip[1] + strip[3] <= VIEWPORT.1,
        "status strip lies inside the viewport: {strip:?}"
    );
    // …and ends before the right column begins — the
    // Phase J column split's whole point (fixed side
    // widths, the centre takes only what is left).
    assert!(
        !rects_intersect(strip, right),
        "status strip runs under the right column: strip {strip:?}, column {right:?}"
    );
    assert!(
        strip[0] + strip[2] <= right[0] + 0.5,
        "status strip's right edge crosses the right column's left edge: \
         strip {strip:?}, column {right:?}"
    );

    // The right column's own controls — the Keyframes
    // buttons that sat beside the strip's tail in the
    // screenshot, and the FrameOps controls below them
    // in the column's workflow order — none of them may
    // share area with the strip either.
    for key in [
        "key-angle",
        "key-delete",
        "key-auto",
        "key-copy",
        "key-paste",
        "key-clear-channel",
        "key-all-here",
        "key-copy-to-frame",
        "inbetween",
        "copy-frame",
        "fop-dur-half",
    ] {
        let (rect, _) = rt
            .presented(key)
            .unwrap_or_else(|| panic!("right-column control {key:?} is presented"));
        assert!(
            !rects_intersect(strip, rect),
            "status strip overlaps {key}: strip {strip:?}, {key} {rect:?}"
        );
    }
}

#[test]
fn timeline_ruler_shows_the_zero_label() {
    use crate::rune_sec::workspaces::Workspace;

    let mut rt = editor();
    // Wave 10 (F2): the marker row mounts only when a
    // frame carries a camera or graph marker — the demo
    // has none, so plant one camera key to keep this
    // test's stacking assertions about a mounted row.
    rt.state.core.project.set_camera_key(2, 0.0, 0.0, 1.0);
    rt.invalidate();
    tap(&mut rt, "ws-animate");
    rt.frame(DT);
    assert_eq!(rt.state.workspace, Workspace::Animate);

    // The ruler, the chips and the marker row are all
    // presented, stacked in that order inside the one
    // scrolled timeline column.
    let keys = presented_keys(&rt);
    for want in ["timeline", "timeline-ruler", "timeline-chips", "timeline-markers"] {
        assert!(keys.contains(want), "missing element keyed {want:?}");
    }
    let (ruler, _) = rt.presented("timeline-ruler").expect("ruler is presented");
    let (chips, _) = rt.presented("timeline-chips").expect("chips are presented");

    // The ruler sits directly above the chips, starting
    // at the same left edge — its slots are the chips'
    // own geometry, so the labels line up frame for
    // frame.
    assert!(
        ruler[1] + ruler[3] <= chips[1] + 0.5,
        "ruler sits above the chips: ruler {ruler:?}, chips {chips:?}"
    );
    assert!(
        (ruler[0] - chips[0]).abs() < 0.5,
        "ruler starts at the chips' left edge: ruler {ruler:?}, chips {chips:?}"
    );

    // Frame 0's slot carries the zero label: a "0:00"
    // text element drawn INSIDE the ruler's rect (the
    // status strip's project segment also mentions
    // 0:00, but as part of a longer string and far
    // outside the ruler — the containment is what pins
    // this text to the ruler).
    let zero = rt
        .elements()
        .into_iter()
        .find(|e| e.text.as_deref() == Some("0:00") && rect_inside(e.rect, ruler))
        .expect("the ruler shows the 0:00 label at frame 0");
    assert!(
        rect_inside(zero.rect, ruler),
        "the zero label is drawn on the ruler: {0:?}",
        zero.rect
    );

    // The chips under the ruler still seek: chip
    // `frame-3` is the third frame — labels are 1-based
    // over 0-based frames, so the playhead lands on
    // frame index 2 (round 6's marker semantics).
    assert_eq!(rt.state.core.current, 0);
    tap(&mut rt, "frame-3");
    assert_eq!(
        rt.state.core.current, 2,
        "chip frame-3 seeks to frame index 2"
    );
}

#[test]
fn palette_mirror_figure_command_exists_and_runs() {
    use crate::rune_sec::command_palette::{display_commands, COMMANDS};
    use crate::rune_sec::workspaces::Workspace;

    let mut rt = editor();
    tap(&mut rt, "ws-animate");
    rt.frame(DT);
    assert_eq!(rt.state.workspace, Workspace::Animate);

    // The before picture: every frame's evaluated joints
    // and stored angles for the selected figure.
    let fi = rt.state.core.selected_figure;
    let frame_count = rt.state.core.frame_count();
    let before_positions: Vec<Vec<[f32; 2]>> = (0..frame_count)
        .map(|f| rt.state.core.positions(f, fi))
        .collect();
    let before_angles: Vec<Vec<f32>> = rt
        .state
        .core
        .project
        .frames
        .iter()
        .map(|fr| fr.poses[fi].angles.clone())
        .collect();

    // A witness joint: the joint sitting furthest
    // sideways from its frame's root — the joint whose
    // reflection is largest and least mistakable. The
    // demo figure reaches sideways (its arms), so one
    // must exist.
    let mut witness: Option<(usize, usize, f32)> = None;
    for (f, joints) in before_positions.iter().enumerate() {
        let root = joints[0];
        for (j, p) in joints.iter().enumerate().skip(1) {
            let dx = (p[0] - root[0]).abs();
            if witness.map_or(true, |(_, _, best)| dx > best) {
                witness = Some((f, j, dx));
            }
        }
    }
    let (wf, wj, wdx) = witness.expect("the demo figure has joints");
    assert!(
        wdx > 1.0,
        "the demo figure reaches sideways somewhere (best offset {wdx})"
    );

    // Run the command through the palette's real path —
    // Ctrl+K, type the command's key as the query (the
    // dashed-key query round 2's palette facts rely on),
    // locate its row in the palette's own listing, walk
    // the highlight to it, Enter — exactly as rounds 5
    // and 6 drove the palette.
    press_key(
        &mut rt,
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
    for c in "mirror-figure".chars() {
        type_char(&mut rt, c);
    }
    assert_eq!(
        rt.state.palette.query.text, "mirror-figure",
        "typed characters landed in the query field"
    );
    let (results, _) =
        display_commands(&rt.state.palette.query.text, &rt.state.palette.recent);
    let row = results
        .iter()
        .position(|&ci| COMMANDS[ci].key == "mirror-figure")
        .expect("the query lists mirror-figure (the Phase J command)");
    let len = results.len();
    let cur = rt.state.palette.selected % len;
    for _ in 0..(row + len - cur) % len {
        press_key(&mut rt, Key::Down, Mods::default());
    }
    assert_eq!(
        rt.state.palette.selected, row,
        "the highlight sits on mirror-figure's row"
    );
    press_key(&mut rt, Key::Enter, Mods::default());
    assert!(
        !rt.state.palette.open,
        "running a command closes the palette"
    );
    assert!(
        rt.state.core.status.contains("mirrored on all"),
        "the op reported the mirror: {}",
        rt.state.core.status
    );

    // The op's contract (see the round's header note):
    // every angle on every frame negated, the root
    // unmoved, and the figure reflected about the
    // vertical axis through its root.
    for (f, frame) in rt.state.core.project.frames.iter().enumerate() {
        for (a, b) in frame.poses[fi].angles.iter().zip(&before_angles[f]) {
            assert_eq!(*a, -*b, "frame {f}: every angle is negated");
        }
    }
    let root_before = before_positions[wf][0];
    let joint_before = before_positions[wf][wj];
    let after_positions = rt.state.core.positions(wf, fi);
    let joint_after = after_positions[wj];
    assert!(
        (after_positions[0][0] - root_before[0]).abs() < 0.01
            && (after_positions[0][1] - root_before[1]).abs() < 0.01,
        "the root does not move: {root_before:?} -> {0:?}",
        after_positions[0]
    );
    assert!(
        (joint_after[0] - (2.0 * root_before[0] - joint_before[0])).abs() < 0.05,
        "the witness joint reflects about the root's vertical axis: \
         {joint_before:?} -> {joint_after:?} around root {root_before:?}"
    );
    assert!(
        (joint_after[1] - joint_before[1]).abs() < 0.05,
        "the witness joint keeps its height: {joint_before:?} -> {joint_after:?}"
    );
    assert!(
        (joint_after[0] - joint_before[0]).abs() > 1.0,
        "the witness joint actually moved: {joint_before:?} -> {joint_after:?}"
    );
}

// ----------------------------------------------------------------
// Round 8 (Phase K, section 37): the Phase J column
// split as a layout pin, the figure inspector's Centre
// button, Pose tools' Mirror-all button, and the FX
// inspector's Split button — each driven through the
// real shell, none set up by writing core state.
// ----------------------------------------------------------------

#[test]
fn stage_width_pins_the_phase_j_column_split() {
    let mut rt = editor();
    tap(&mut rt, "ws-animate");
    rt.frame(DT);

    // Both side columns are presented: their visibility
    // flags are unpersisted shell state and `init`
    // defaults them shown (round 3's note), so a fresh
    // shell always lays out the full three-column
    // split this pin measures.
    rt.presented("side-left").expect("left column is presented");
    rt.presented("side-right")
        .expect("right column is presented");

    // The pin: at this file's 1440×900 viewport the
    // centre — and with it the stage — is exactly what
    // the window leaves after margins, gaps and the
    // two FIXED side widths (300 / 340): 744 points,
    // the number the Phase J headless tree dump
    // measured in the real binary. Before the fix all
    // three columns grew to equal thirds and the stage
    // was squeezed to ~461; a future layout change
    // that moves this number must reopen the fix
    // deliberately, here first.
    let (stage, _) = rt.presented("stage").expect("stage is presented");
    assert!(
        (stage[2] - 744.0).abs() <= 1.0,
        "stage width pins the Phase J column split at 744: {stage:?}"
    );
}

#[test]
fn fig_centre_button_centres_the_figure_on_the_stage() {
    use crate::rune_sec::inspector_figure::figure_bbox;
    use crate::rune_sec::workspaces::Workspace;

    let mut rt = editor();
    tap(&mut rt, "ws-animate");
    rt.frame(DT);
    assert_eq!(rt.state.workspace, Workspace::Animate);

    // The before picture, through the section's own
    // bbox helper (evaluated joints at the display
    // frame, head-flagged points grown by the ring
    // radius): the demo figure stands on frame 0 with
    // its root at [480, 470] against a [960, 720]
    // stage, so its bbox centre starts meaningfully
    // below the stage centre on y. Assert the premise
    // rather than assume it.
    let stage = rt.state.core.project.stage;
    let centre = [stage[0] / 2.0, stage[1] / 2.0];
    let before = figure_bbox(&rt.state.core).expect("the demo figure has a bbox");
    let before_c = [
        (before[0] + before[2]) / 2.0,
        (before[1] + before[3]) / 2.0,
    ];
    assert!(
        (before_c[1] - centre[1]).abs() > 20.0,
        "premise: the demo figure starts off-centre: \
         bbox centre {before_c:?}, stage centre {centre:?}"
    );
    let root_f1_before = rt.state.core.project.frames[1].poses[0].root;

    // The figure inspector sits in the right column —
    // scroll the Centre button into view and press it.
    scroll_into_view(&mut rt, "fig-centre");
    tap(&mut rt, "fig-centre");
    assert!(
        rt.state.core.status.contains("centred"),
        "the op reported the centring: {}",
        rt.state.core.status
    );

    // The bbox centre now sits on the stage centre:
    // the op computes its delta from this same helper,
    // so anything beyond float noise is a real miss.
    let after = figure_bbox(&rt.state.core).expect("the figure still has a bbox");
    let after_c = [(after[0] + after[2]) / 2.0, (after[1] + after[3]) / 2.0];
    assert!(
        (after_c[0] - centre[0]).abs() <= 1.0 && (after_c[1] - centre[1]).abs() <= 1.0,
        "the bbox centre lands on the stage centre: {after_c:?} vs {centre:?}"
    );
    // And the placement is current-frame only (the
    // section's contract): frame 1's root is exactly
    // where the demo put it.
    assert_eq!(
        rt.state.core.project.frames[1].poses[0].root, root_f1_before,
        "centring moves only the current frame's root"
    );
}

#[test]
fn pose_mirror_all_button_negates_every_angle_and_keeps_roots() {
    use crate::rune_sec::workspaces::Workspace;

    let mut rt = editor();
    tap(&mut rt, "ws-animate");
    rt.frame(DT);
    assert_eq!(rt.state.workspace, Workspace::Animate);

    // Setup writes no core state (this file never has
    // — see the round's header note): the demo pose
    // itself carries non-zero angles, its wave raising
    // an arm (−118° at angles[5] on frame 0), so the
    // negation below has something real to bite on.
    // Record every frame's angles and roots, and
    // assert the non-zero premise rather than assume
    // it.
    let fi = rt.state.core.selected_figure;
    let before_angles: Vec<Vec<f32>> = rt
        .state
        .core
        .project
        .frames
        .iter()
        .map(|fr| fr.poses[fi].angles.clone())
        .collect();
    let before_roots: Vec<[f32; 2]> = rt
        .state
        .core
        .project
        .frames
        .iter()
        .map(|fr| fr.poses[fi].root)
        .collect();
    assert!(
        before_angles.iter().flatten().any(|a| *a != 0.0),
        "premise: the demo pose has non-zero angles to negate"
    );

    // Pose tools are an Animate panel in the right
    // column, below the fold — scroll and press. (The
    // button is live here: its gate is a pose existing
    // for the selected figure, which the demo has on
    // every frame.)
    scroll_into_view(&mut rt, "pose-mirror-all");
    tap(&mut rt, "pose-mirror-all");
    assert!(
        rt.state.core.status.contains("mirrored on all"),
        "the op reported the mirror: {}",
        rt.state.core.status
    );

    // The op's contract (round 7, there through the
    // palette): every stored angle on every frame is
    // the exact negation of before, and no frame's
    // root moved at all.
    for (f, frame) in rt.state.core.project.frames.iter().enumerate() {
        assert_eq!(
            frame.poses[fi].angles.len(),
            before_angles[f].len(),
            "frame {f}: the angle count is unchanged"
        );
        for (a, b) in frame.poses[fi].angles.iter().zip(&before_angles[f]) {
            assert_eq!(*a, -*b, "frame {f}: every angle is negated");
        }
        assert_eq!(
            frame.poses[fi].root, before_roots[f],
            "frame {f}: the root does not move"
        );
    }
}

#[test]
fn fx_split_button_cuts_the_selected_clip_at_the_playhead() {
    use crate::rune_sec::workspaces::Workspace;

    let mut rt = editor();
    tap(&mut rt, "ws-animate");
    rt.frame(DT);
    assert_eq!(rt.state.workspace, Workspace::Animate);
    assert!(
        rt.state.core.project.fx_clips.is_empty(),
        "premise: the demo project has no FX clips"
    );

    // Adding an FX clip needs a selected joint (the
    // core's add path refuses without one). Select the
    // root joint the way round 1's drag test aims: a
    // press on the stage at the joint's drawn point —
    // press only, no move, so the pose stays put.
    // Settle first: `presented` returns the DRAWN box,
    // which still moves while the stage's entrance runs
    // — a press aimed from an unsettled rect can land
    // off the joint (the flake this settle removes; the
    // same settle the clips-lane block below uses).
    for _ in 0..30 {
        rt.frame(DT);
    }
    let (rect, _) = rt.presented("stage").expect("stage is presented");
    let [sx, sy, sw, sh] = rect;
    let root = rt.state.core.positions(rt.state.core.current, 0)[0];
    let vt = view_transform(
        [sw, sh],
        rt.state.core.project.stage,
        rt.state.core.camera_here(),
    );
    let local = to_screen(&vt, root);
    rt.pointer_down(sx + local[0], sy + local[1]);
    rt.frame(DT);
    assert_eq!(
        rt.state.core.selected_node,
        Some(0),
        "the stage press selected the root joint"
    );
    rt.pointer_up(sx + local[0], sy + local[1]);
    rt.frame(DT);

    // To Effects, and add a Burst clip through the
    // browser's own kind button: the clip starts at
    // the current frame (0) with the add path's fixed
    // 16-frame duration.
    tap(&mut rt, "ws-effects");
    rt.frame(DT);
    assert_eq!(rt.state.workspace, Workspace::Effects);
    scroll_into_view(&mut rt, "fxclip-burst");
    tap(&mut rt, "fxclip-burst");
    assert_eq!(
        rt.state.core.project.fx_clips.len(),
        1,
        "the browser press added one FX clip"
    );
    {
        let clip = &rt.state.core.project.fx_clips[0].clip;
        assert_eq!(clip.start, 0.0, "the clip starts at the current frame");
        assert_eq!(clip.duration, 16.0, "the add path's fixed duration");
    }

    // Select the clip through its lane row (the
    // browser add does not select), park the playhead
    // mid-clip — frame 2 of [0, 16) — with a timeline
    // chip, then press the inspector's Split button,
    // which renders only inside the selected clip's
    // detail block.
    scroll_into_view(&mut rt, "fx-clip-0");
    tap(&mut rt, "fx-clip-0");
    assert_eq!(
        rt.state.fx_selected,
        Some(0),
        "the lane row selects the clip"
    );
    tap(&mut rt, "frame-3");
    assert_eq!(rt.state.core.current, 2, "the playhead sits mid-clip");
    scroll_into_view(&mut rt, "fx-split");
    tap(&mut rt, "fx-split");

    // The cut lands at the playhead frame's centre
    // (frame 2 → 2.5): the first clip keeps [0, 2.5),
    // the new second clip takes [2.5, 16), and the
    // selection still names the first clip (the
    // panel's documented post-split rule).
    let clips = &rt.state.core.project.fx_clips;
    assert_eq!(clips.len(), 2, "the split added a second clip");
    assert!(
        (clips[0].clip.duration - 2.5).abs() < 0.001,
        "the first clip shrank to the cut: {}",
        clips[0].clip.duration
    );
    assert!(
        (clips[1].clip.start - 2.5).abs() < 0.001
            && (clips[1].clip.duration - 13.5).abs() < 0.001,
        "the second clip takes the remainder: start {}, duration {}",
        clips[1].clip.start,
        clips[1].clip.duration
    );
    assert_eq!(
        rt.state.fx_selected,
        Some(0),
        "the selection still names the first clip"
    );

    // Restore Animate for the shared prefs, as rounds
    // 2–7 do.
    tap(&mut rt, "ws-animate");
    assert_eq!(rt.state.workspace, Workspace::Animate);
}

// ----------------------------------------------------------------
// Round 9 (Phase L, section 26): the transport's speed
// cycle, the figure inspector's head-ring stepper, the
// timeline's loop row, and the 3D panel's preview
// viewport — each driven through the real shell, none
// set up by writing core or shell state.
// ----------------------------------------------------------------

#[test]
fn pb_speed_button_cycles_the_shell_playback_speed() {
    use crate::rune_sec::workspaces::Workspace;

    let mut rt = editor();
    tap(&mut rt, "ws-animate");
    rt.frame(DT);
    assert_eq!(rt.state.workspace, Workspace::Animate);

    // Premise: the speed is shell state seeded by
    // `init` at normal speed — no test in this file
    // writes it during setup (this file never writes
    // state in setup at all), and nothing persists it
    // between shells.
    assert_eq!(
        rt.state.playback_speed, 1.0,
        "premise: a fresh shell plays at normal speed"
    );

    // The Playback panel sits in the right column,
    // below the fold — scroll the speed button into
    // view and press it. Its handler is the section's
    // `cycle_playback_speed`: the field alone moves,
    // to the SPEEDS table's next step above 1.0.
    scroll_into_view(&mut rt, "pb-speed");
    tap(&mut rt, "pb-speed");
    assert_eq!(
        rt.state.playback_speed, 1.5,
        "one press cycles the speed 1.0 -> 1.5"
    );

    // The button itself persists across the press —
    // it re-renders with the new speed in its label
    // and its active flag set, under the same key.
    assert!(
        presented_keys(&rt).contains("pb-speed"),
        "the speed button is still presented after the press"
    );
    assert!(
        rt.elements().iter().any(|e| e.text.as_deref() == Some("Speed 1.5×")),
        "the button's label now names the new speed"
    );
}

#[test]
fn fig_ring_up_button_raises_the_head_ring_scale() {
    use crate::rune_sec::workspaces::Workspace;

    let mut rt = editor();
    tap(&mut rt, "ws-animate");
    rt.frame(DT);
    assert_eq!(rt.state.workspace, Workspace::Animate);

    // Premise: a fresh demo project's head-ring scale
    // is exactly 1.0 (the model's field default), so
    // one step of the inspector's 0.1 stepper has the
    // whole distance to the setter's 2.0 clamp.
    assert_eq!(
        rt.state.core.project.head_ring_scale, 1.0,
        "premise: the demo project starts at ring scale 1.0"
    );

    // The figure inspector is a right-column panel
    // and the ring row sits deep inside it — scroll
    // the + stepper into view and press it.
    scroll_into_view(&mut rt, "fig-ring-up");
    tap(&mut rt, "fig-ring-up");

    // The field the renderers scale the ring by has
    // risen by exactly the step, and the setter's own
    // status line says so.
    let scale = rt.state.core.project.head_ring_scale;
    assert!(
        scale > 1.0,
        "one press raises the head-ring scale above 1.0: {scale}"
    );
    assert!(
        (scale - 1.1).abs() < 1e-6,
        "the raise is the 0.1 step, to 1.1: {scale}"
    );
    assert!(
        rt.state.core.status.contains("Head ring size set"),
        "the setter reported the change: {}",
        rt.state.core.status
    );
}

#[test]
fn timeline_loop_row_sets_and_clears_the_loop_range() {
    use crate::rune_sec::workspaces::Workspace;

    let mut rt = editor();
    tap(&mut rt, "ws-animate");
    rt.frame(DT);
    assert_eq!(rt.state.workspace, Workspace::Animate);

    // Premise: the demo project carries no loop range
    // (a fresh core's field is None), so the timeline's
    // loop row — under the chips in the centre column,
    // no scrolling — offers Set loop, and only that.
    assert_eq!(
        rt.state.core.loop_range, None,
        "premise: the demo project has no loop range"
    );
    assert!(
        presented_keys(&rt).contains("loop-set"),
        "the loop row offers Set loop while no range is set"
    );

    // Set loop asks for the playhead's range: from the
    // current frame (0), four frames on, clamped to
    // the last frame — the whole five-frame demo.
    tap(&mut rt, "loop-set");
    assert_eq!(
        rt.state.core.loop_range,
        Some((0, 4)),
        "Set loop spans the demo from the playhead"
    );
    assert!(
        rt.state.core.status.contains("Looping frames 1–5."),
        "the core reported the range: {}",
        rt.state.core.status
    );

    // With a range set, the row carries its own Clear
    // under the shared `loop-clear` key — first in
    // tree order here, because the centre column
    // mounts before the right column's two other
    // homes of the same key (see the round's header
    // note). One tap clears through the core's op.
    tap(&mut rt, "loop-clear");
    assert_eq!(
        rt.state.core.loop_range, None,
        "Clear removes the loop range"
    );
    assert!(
        rt.state.core.status.contains("Loop range cleared."),
        "the core reported the clear: {}",
        rt.state.core.status
    );

    // And the row is back to offering Set loop.
    assert!(
        presented_keys(&rt).contains("loop-set"),
        "the loop row offers Set loop again after the clear"
    );
}

#[test]
fn build3d_workspace_presents_the_3d_preview() {
    use crate::rune_sec::workspaces::Workspace;

    let mut rt = editor();
    // Seed Animate through its chip (the shared-prefs
    // discipline), so the switch below is a real switch
    // whatever a sibling test left persisted.
    tap(&mut rt, "ws-animate");
    rt.frame(DT);
    assert_eq!(rt.state.workspace, Workspace::Animate);

    // To the 3D workspace through its chip — key
    // `ws-3d` (workspaces.rs; the workspace's name is
    // "3D", its key is not `ws-build3d`).
    tap(&mut rt, "ws-3d");
    rt.frame(DT);
    assert_eq!(rt.state.workspace, Workspace::Build3D);

    // The 3D panel mounts there (its section is gated
    // to this workspace alone), and the Phase L
    // preview canvas sits at the top of it — present
    // in the presented tree with no 3D session needed:
    // the viewport renders its field and an empty
    // shape until a session projects into it.
    let keys = presented_keys(&rt);
    assert!(keys.contains("panel-3d"), "the 3D panel is presented");
    assert!(
        keys.contains("panel-3d-preview"),
        "the 3D preview viewport is presented"
    );
    assert!(
        rt.find("panel-3d-preview").is_some(),
        "the preview has a findable centre"
    );

    // Restore Animate for the shared prefs, as rounds
    // 2–8 do.
    tap(&mut rt, "ws-animate");
    assert_eq!(rt.state.workspace, Workspace::Animate);
}

// ----------------------------------------------------------------
// Round 10 (Phase M, section 35): the Frames section's
// frame swap, the 3D panel's session toggle, and the
// settings card's direct-select speed chips — each
// driven through the real shell. (The round's fourth
// planned test, the timeline's FX clips lane, is not
// here: its key existed nowhere in the tree at census
// time — see the module docs.)
// ----------------------------------------------------------------

#[test]
fn swap_next_button_exchanges_frame_contents() {
    use crate::rune_sec::workspaces::Workspace;

    let mut rt = editor();
    tap(&mut rt, "ws-animate");
    rt.frame(DT);
    assert_eq!(rt.state.workspace, Workspace::Animate);

    // The frames' durations are the contents this test
    // watches trade places: the swap moves whole
    // `Frame` values, so a planted duration is a
    // planted identity.
    let durations = |rt: &Runtime<State>| -> Vec<f32> {
        rt.state
            .core
            .project
            .frames
            .iter()
            .map(|f| f.duration)
            .collect()
    };
    assert_eq!(
        durations(&rt),
        vec![1.0; 5],
        "premise: the demo's five frames sit at the default duration"
    );
    assert_eq!(
        rt.state.core.current, 0,
        "premise: the playhead starts on frame 0"
    );

    // Plant distinguishable durations through the
    // Frames section's own ×2 stepper (right column):
    // frame 0 to 2×, then seek to frame 1 and double
    // it twice, to 4×. The stepper writes the field
    // directly (no snapshot), so the undo depth must
    // not move while planting.
    let undo0 = rt.state.core.undo_depth();
    scroll_into_view(&mut rt, "fop-dur-double");
    tap(&mut rt, "fop-dur-double");
    assert_eq!(durations(&rt)[0], 2.0, "frame 0 planted at 2x");
    tap(&mut rt, "frame-2");
    assert_eq!(rt.state.core.current, 1, "the chip seeks to frame 1");
    scroll_into_view(&mut rt, "fop-dur-double");
    tap(&mut rt, "fop-dur-double");
    tap(&mut rt, "fop-dur-double");
    assert_eq!(
        durations(&rt),
        vec![2.0, 4.0, 1.0, 1.0, 1.0],
        "both plants landed"
    );
    assert_eq!(
        rt.state.core.undo_depth(),
        undo0,
        "planting takes no undo steps"
    );

    // Back on frame 0, Swap with next trades the two
    // frames' contents: the durations move with their
    // frames, the playhead keeps naming slot 0, and
    // the swap costs exactly one undo step.
    tap(&mut rt, "frame-1");
    assert_eq!(rt.state.core.current, 0);
    scroll_into_view(&mut rt, "fop-swap-next");
    tap(&mut rt, "fop-swap-next");
    assert_eq!(
        durations(&rt),
        vec![4.0, 2.0, 1.0, 1.0, 1.0],
        "the two frames traded contents"
    );
    assert_eq!(
        rt.state.core.current, 0,
        "the playhead stays on its slot"
    );
    assert_eq!(
        rt.state.core.undo_depth(),
        undo0 + 1,
        "one swap is one undo step"
    );

    // On the last frame the same key renders in its
    // inert form (`icon_btn_inert`: same key, no
    // handler attached). Presence plus a tap that
    // changes nothing is the honest assertion — round
    // 6's pose-paste pattern.
    tap(&mut rt, "frame-5");
    assert_eq!(
        rt.state.core.current, 4,
        "the playhead sits on the last frame"
    );
    scroll_into_view(&mut rt, "fop-swap-next");
    assert!(
        presented_keys(&rt).contains("fop-swap-next"),
        "the inert Swap with next is presented on the last frame"
    );
    let status = rt.state.core.status.clone();
    let undo = rt.state.core.undo_depth();
    tap(&mut rt, "fop-swap-next");
    assert_eq!(
        durations(&rt),
        vec![4.0, 2.0, 1.0, 1.0, 1.0],
        "the inert press swaps nothing"
    );
    assert_eq!(
        rt.state.core.status, status,
        "the inert press writes no status"
    );
    assert_eq!(
        rt.state.core.undo_depth(),
        undo,
        "the inert press adds no undo step"
    );
    assert_eq!(
        rt.state.core.current, 4,
        "the inert press does not seek"
    );
}

#[test]
fn build3d_workspace_shows_fig3_stage_state() {
    use crate::rune_sec::workspaces::Workspace;

    let mut rt = editor();
    // Seed Animate through its chip (the shared-prefs
    // discipline), as round 9's preview test does.
    tap(&mut rt, "ws-animate");
    rt.frame(DT);
    assert_eq!(rt.state.workspace, Workspace::Animate);

    tap(&mut rt, "ws-3d");
    rt.frame(DT);
    assert_eq!(rt.state.workspace, Workspace::Build3D);

    // Premise: a 3D session exists only once the
    // panel's own toggle starts one — round 9 asserted
    // the preview's presence without one; this test
    // pins the session state itself. (The preview's
    // Shape contents are not visible to the tree; the
    // headless snap is the pixel proof, not this test.)
    assert!(
        rt.state.core.fig3.is_none(),
        "premise: no 3D session yet"
    );
    let keys = presented_keys(&rt);
    assert!(keys.contains("panel-3d"), "the 3D panel is presented");
    assert!(
        keys.contains("panel-3d-preview"),
        "the preview viewport is presented"
    );
    assert!(
        keys.contains("stage"),
        "the stage stays mounted in the 3D workspace"
    );

    // Enter through the panel header's toggle
    // (`mode-3d`): with no session, its handler runs
    // the core's `enter_3d`.
    scroll_into_view(&mut rt, "mode-3d");
    tap(&mut rt, "mode-3d");
    assert!(
        rt.state.core.fig3.is_some(),
        "the toggle started a 3D session"
    );
    let keys = presented_keys(&rt);
    assert!(
        keys.contains("panel-3d-preview"),
        "the preview stays presented in session"
    );
    assert!(
        keys.contains("stage"),
        "the stage stays presented in session"
    );

    // The same toggle exits, leaving no session state
    // behind in this shell.
    scroll_into_view(&mut rt, "mode-3d");
    tap(&mut rt, "mode-3d");
    assert!(
        rt.state.core.fig3.is_none(),
        "the toggle exits the session"
    );

    // Restore Animate for the shared prefs.
    tap(&mut rt, "ws-animate");
    assert_eq!(rt.state.workspace, Workspace::Animate);
}

#[test]
fn settings_speed_chips_set_playback_speed() {
    let mut rt = editor();

    // Premise, mirrored from round 9's pb-speed test:
    // the speed is shell state seeded by `init` at
    // normal speed, and nothing persists it between
    // shells.
    assert_eq!(
        rt.state.playback_speed, 1.0,
        "premise: a fresh shell plays at normal speed"
    );

    // Open the settings card as rounds 3/5 do. The
    // card is a right-column panel, so its Playback
    // block — deep in the card, past the theme and
    // language blocks — is reached with the usual
    // scroll-into-view discipline (which also settles
    // the children's presence fade).
    tap(&mut rt, "settings-toggle");
    assert!(rt.state.settings.open, "Settings toggle opens the card");
    scroll_into_view(&mut rt, "settings-speed-3");
    assert!(
        presented_keys(&rt).contains("settings-speed-row"),
        "the speed row is presented"
    );

    // Chip 3 of the transport's SPEEDS table
    // ([0.25, 0.5, 1.0, 1.5, 2.0]) is 1.5×: one press
    // writes the shell field outright (the section's
    // `set_playback_speed` — silent, exactly like the
    // pb-speed cycle round 9 pinned).
    tap(&mut rt, "settings-speed-3");
    assert_eq!(
        rt.state.playback_speed, 1.5,
        "chip 3 sets the playback speed to 1.5x"
    );

    // Chip 2 is normal speed, in the same row.
    tap(&mut rt, "settings-speed-2");
    assert_eq!(
        rt.state.playback_speed, 1.0,
        "chip 2 restores normal speed"
    );
}

#[test]
fn timeline_clips_lane_appears_with_a_planted_clip() {
    use crate::rune_sec::workspaces::Workspace;

    let mut rt = editor();
    // Seed Animate through its chip (the shared-prefs
    // discipline), as the other round 10 tests do.
    tap(&mut rt, "ws-animate");
    rt.frame(DT);
    assert_eq!(rt.state.workspace, Workspace::Animate);

    // Premise: the demo project carries no audio or FX
    // clips, so the lane is absent — the timeline is
    // byte-identical to its pre-lane shape.
    assert!(
        rt.state.core.project.fx_clips.is_empty()
            && rt.state.core.project.audio.is_empty(),
        "premise: the demo project has no clips"
    );
    assert!(
        !presented_keys(&rt).contains("timeline-clips"),
        "no clips, no clips lane"
    );

    // Plant an FX clip by direct project write (the
    // graph-duplicates precedent): a Burst clip on
    // figure 0 starting at frame 2 for 3 frames.
    rt.state.core.project.fx_clips.push(
        stykanimate_rs::model::FxClipEntry {
            clip: stykanimate_rs::vfx::ParticleClip {
                start: 2.0,
                duration: 3.0,
                kind: stykanimate_rs::vfx::ClipKind::Burst,
                color: 0xFFC65D,
                color2: None,
                seed: 7,
                angle: 0.0,
                length: 130.0,
            },
            figure: 0,
            node: 0,
            hidden: false,
        },
    );
    // A direct write is outside any handler: the
    // Runtime re-runs the view only when state is
    // marked changed, so say so (Runtime::invalidate
    // — "marks app state changed outside a handler").
    rt.invalidate();
    rt.frame(DT);
    // The lane mounts through the presence fade, so
    // allow the bounded settle loop before asserting
    // presence (the same discipline as absence,
    // below).
    let mut appeared = false;
    for _ in 0..30 {
        rt.frame(DT);
        if presented_keys(&rt).contains("timeline-clips") {
            appeared = true;
            break;
        }
    }
    assert!(appeared, "the clips lane appears once a clip exists");

    // Clearing the clips removes the lane again; the
    // row leaves through the presence fade, so allow
    // the bounded settle loop before asserting absence.
    rt.state.core.project.fx_clips.clear();
    rt.invalidate();
    let mut gone = false;
    for _ in 0..30 {
        rt.frame(DT);
        if !presented_keys(&rt).contains("timeline-clips") {
            gone = true;
            break;
        }
    }
    assert!(gone, "the clips lane leaves with the last clip");
}

// ----------------------------------------------------------------
// Round 11 (Phase N, verification wave): the card
// restructure — panel card roots across every workspace,
// card containment in the columns, the Edit menu's kit
// divider, the palette and settings cards, the timeline
// card's stacking, the shell furniture, and the help
// panel. Relations and presence only (see the module
// docs): the Runtime cannot see a fill or a radius, and
// no test here pretends otherwise.
// ----------------------------------------------------------------

#[test]
fn animate_panel_card_roots_present() {
    use crate::rune_sec::workspaces::Workspace;

    let mut rt = editor();
    // Seed Animate through its chip (the shared-prefs
    // discipline) and settle out whatever panels a
    // sibling-restored workspace left presence-fading.
    tap(&mut rt, "ws-animate");
    rt.frame(DT);
    assert_eq!(rt.state.workspace, Workspace::Animate);
    for _ in 0..30 {
        let keys = presented_keys(&rt);
        if !keys.contains("draw-tools")
            && !keys.contains("graph-panel")
            && !keys.contains("home-panel")
        {
            break;
        }
        rt.frame(DT);
    }

    // Every gated root Animate mounts (workspaces.rs's
    // ANIMATE table against main.rs's mount order),
    // each now the root of a `design::panel_col` card.
    // A builder mistake in the restructure loses the
    // root key or the panel — presence in the live
    // tree is the proof neither happened. The gate
    // diffs key sets statically; this is the mounted
    // counterpart.
    let keys = presented_keys(&rt);
    for want in [
        // Left column: project, figures, library, layers.
        "project-tools",
        "figures-panel",
        "library-panel",
        "layers-panel",
        // Right column: the three inspectors, pose and
        // keyframe tools, playback, frame ops, camera,
        // onion, movieclips, scene, stats.
        "inspector-segment",
        "inspector-figure",
        "inspector-style",
        "pose-tools",
        "keyframe-tools",
        "playback-controls",
        "frames",
        "camera-panel",
        "onion-controls",
        "clips-panel",
        "scene-panel",
        "stats-panel",
    ] {
        assert!(keys.contains(want), "Animate card root {want:?} is presented");
    }
    // And the workspaces Animate gates out stay out.
    assert!(
        !keys.contains("graph-panel"),
        "Animate gates the Graph panel out"
    );
    assert!(
        !keys.contains("home-panel"),
        "Animate gates the Home panel out"
    );
}

#[test]
fn panel_card_rects_stay_inside_their_columns() {
    use crate::rune_sec::workspaces::Workspace;

    let mut rt = editor();
    tap(&mut rt, "ws-animate");
    rt.frame(DT);
    assert_eq!(rt.state.workspace, Workspace::Animate);

    // The columns' content roots and one card per
    // column, plus the stage. Drawn rects of scroll
    // content translate with the column's own offset,
    // so containment between a column and its cards
    // holds at any scroll position (round 7/8's rect
    // discipline).
    let (left, _) = rt.presented("side-left").expect("left column is presented");
    let (right, _) = rt
        .presented("side-right")
        .expect("right column is presented");
    let (centre, _) = rt.presented("centre").expect("centre column is presented");
    let (figures, _) = rt
        .presented("figures-panel")
        .expect("figures card is presented");
    let (frames, _) = rt.presented("frames").expect("frames card is presented");
    let (stage, _) = rt.presented("stage").expect("stage is presented");

    // The figures card lies fully inside its column's
    // content rect: card padding lives INSIDE the card
    // (charter §1), so a correct card can touch the
    // column's edges but never cross them. The frames
    // card gets the x-range treatment below instead of
    // full containment: the right column is a scroll
    // viewport, so a card below the fold legitimately
    // presents past the viewport's bottom edge (its
    // unscrolled y is ~1773) — P17's test 2 specifies
    // x-range containment for exactly this reason.
    assert!(
        rect_inside(figures, left),
        "figures card escapes the left column: card {figures:?}, column {left:?}"
    );

    // The column contract as x-ranges (the charter
    // changes padding inside surfaces, never the
    // 300 / 744 / 340 split — main.rs): at this
    // viewport the left content spans [16, 316] (root
    // padding 16 + viewport 300) and the right content
    // [1084, 1424] (16 + 300 + 12 + 744 + 12, then
    // 340). A card swollen by its own padding would
    // break its range even inside a taller column rect.
    assert!(
        figures[0] >= 16.0 - 0.5 && figures[0] + figures[2] <= 316.0 + 0.5,
        "figures card crosses the left column's x-range: {figures:?}"
    );
    assert!(
        frames[0] >= right[0] - 0.5 && frames[0] + frames[2] <= right[0] + right[2] + 0.5,
        "frames card crosses the right column's x-range: {frames:?}"
    );

    // The stage stays inside the centre column and off
    // both side columns' cards — the Phase J overlap
    // class, re-checked after the widest structural
    // change since (the module docs carry the height
    // arithmetic; relations are the durable form).
    assert!(
        rect_inside(stage, centre),
        "stage escapes the centre column: stage {stage:?}, centre {centre:?}"
    );
    assert!(
        !rects_intersect(stage, figures),
        "stage paints over the figures card: stage {stage:?}, card {figures:?}"
    );
    assert!(
        !rects_intersect(stage, frames),
        "stage paints over the frames card: stage {stage:?}, card {frames:?}"
    );
}

#[test]
fn workspace_card_roots_sweep() {
    use crate::rune_sec::workspaces::Workspace;

    let mut rt = editor();
    // Normalise the layout-pref-seeded fields (Focused
    // Wave D1 wired them): `init` seeds `show_left` from
    // the persisted `sidebar_visible` and the timeline
    // flag from `timeline_collapsed`, both in the layout
    // file every test in this process shares — a sibling
    // test's persisted sidebar-off would otherwise hide
    // the Figures column this test asserts on. Force the
    // defaults, then render once with them in place.
    rt.state.show_left = true;
    rt.state.show_right = true;
    rt.state.frame_ops.timeline_collapsed = false;
    rt.frame(DT);
    // One row per workspace chip: the workspace it
    // selects, two or three card roots that must mount
    // there, and one root that must be gated out. Rows
    // are filled from workspaces.rs's gating table and
    // each panel's adopted root key (the module docs
    // carry the fx-section correction). The sweep walks
    // every workspace's roots in one test because the
    // adoption wave touched every panel's root.
    let rows: [(&str, Workspace, &[&str], &str); 8] = [
        (
            "ws-animate",
            Workspace::Animate,
            &["figures-panel", "frames", "stats-panel"],
            "graph-panel",
        ),
        (
            "ws-draw",
            Workspace::Draw,
            &["draw-tools", "stroke-inspector", "figures-panel"],
            "frames",
        ),
        (
            "ws-graph",
            Workspace::Graph,
            &["graph-panel", "easing-panel", "figures-panel"],
            "frames",
        ),
        (
            "ws-effects",
            Workspace::Effects,
            &["fx-browser", "fx-section", "particles"],
            "frames",
        ),
        (
            "ws-audio",
            Workspace::Audio,
            &["audio-panel", "waveform-card", "playback-controls"],
            "frames",
        ),
        (
            "ws-3d",
            Workspace::Build3D,
            &["panel-3d", "panel-3d-preview", "camera-panel"],
            "frames",
        ),
        (
            "ws-export",
            Workspace::Export,
            &["export-panel", "stats-panel", "project-tools"],
            "figures-panel",
        ),
        (
            "ws-home",
            Workspace::Home,
            &["home-panel", "project-tools", "stats-panel"],
            "figures-panel",
        ),
    ];
    for (chip, ws, present, absent) in rows {
        tap(&mut rt, chip);
        rt.frame(DT);
        assert_eq!(rt.state.workspace, ws, "{chip} selects its workspace");
        // Mounts land through the presence fade and
        // gate-outs leave through it — settle (bounded)
        // both ways before asserting.
        for _ in 0..30 {
            if present
                .iter()
                .all(|k| presented_keys(&rt).contains(*k))
            {
                break;
            }
            rt.frame(DT);
        }
        for _ in 0..30 {
            if !presented_keys(&rt).contains(absent) {
                break;
            }
            rt.frame(DT);
        }
        let keys = presented_keys(&rt);
        for want in present {
            assert!(
                keys.contains(*want),
                "{chip}: card root {want:?} is presented"
            );
        }
        assert!(
            !keys.contains(absent),
            "{chip}: {absent:?} is gated out"
        );
    }

    // Restore Animate for the shared prefs, as rounds
    // 2–10 do.
    tap(&mut rt, "ws-animate");
    assert_eq!(rt.state.workspace, Workspace::Animate);
}

// ----------------------------------------------------------------
// Round 12 (UI mission Wave 9): the workspace scenes.
// scenes.rs carries one scene per workspace so the
// headless `--snap`/`--tree` runs can render each
// workspace's own set; the layout facts there (8, 18,
// 21–25) name, per scene, a signature panel key that
// must mount and a key the workspace gates out. This
// test is the in-process counterpart: it resolves each
// scene BY NAME in the `scenes()` table (the same
// lookup `with_scene` performs), applies it to a fresh
// runtime's state, and pins the workspace it lands on
// plus the fact's key pair in the presented tree.
// ----------------------------------------------------------------

#[test]
fn workspace_scenes_mount_their_signature_panels() {
    use crate::rune_sec::workspaces::Workspace;

    // One row per workspace scene: table name, the
    // workspace the scene must land on, the signature
    // panel key, and the gated-out key (scenes.rs's
    // layout facts carry the same pairs for the CI
    // `--tree` checks). Wave 9 integration correction:
    // `figures-panel` is gated out ONLY in Home and
    // Export — the Figures section mounts in Draw,
    // Graph, Effects, Audio and Build3D too (E9's
    // gating audit), so those rows gate-check against
    // `frames` (FrameOps, Animate-only) instead, and
    // the loop below additionally asserts figures-panel
    // is PRESENT in them.
    let rows: [(&str, Workspace, &str, &str); 7] = [
        ("home", Workspace::Home, "home-panel", "figures-panel"),
        ("draw-open", Workspace::Draw, "stroke-inspector", "frames"),
        ("build-open", Workspace::Build3D, "panel-3d", "frames"),
        ("graph-open", Workspace::Graph, "graph-panel", "frames"),
        (
            "effects-open",
            Workspace::Effects,
            "fx-browser",
            "frames",
        ),
        ("audio-open", Workspace::Audio, "audio-panel", "frames"),
        (
            "export-open",
            Workspace::Export,
            "export-panel",
            "figures-panel",
        ),
    ];
    let table = crate::rune_sec::scenes::scenes();
    for (name, ws, present, absent) in rows {
        // The scene must be registered under exactly
        // this name — the headless runner looks it up
        // the same way, and a rename that only updated
        // one side would silently lose the scene's
        // pixel proof.
        let scene = table
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, f)| *f)
            .unwrap_or_else(|| panic!("scene {name:?} is registered in scenes()"));

        let mut rt = editor();
        scene(&mut rt.state);
        // The scene fn is a direct state write, outside
        // the runtime's dispatch — mark the tree changed
        // so the next frame re-runs the view (the suite's
        // own discipline after direct writes; without it
        // the runtime keeps presenting the seeded
        // workspace's panels and the scene's panels never
        // mount).
        rt.invalidate();
        assert_eq!(
            rt.state.workspace, ws,
            "scene {name:?} lands on its workspace"
        );
        rt.frame(DT);
        // Mounts and gate-outs travel through the
        // presence fade — settle (bounded) both ways
        // before asserting, the sweep's discipline.
        for _ in 0..30 {
            if presented_keys(&rt).contains(present) {
                break;
            }
            rt.frame(DT);
        }
        for _ in 0..30 {
            if !presented_keys(&rt).contains(absent) {
                break;
            }
            rt.frame(DT);
        }
        let keys = presented_keys(&rt);
        assert!(
            keys.contains(present),
            "scene {name:?}: signature panel {present:?} is presented"
        );
        assert!(
            !keys.contains(absent),
            "scene {name:?}: {absent:?} is gated out"
        );
        // Figures mounts in every workspace except Home
        // and Export (E9's gating audit): in the rows
        // whose gate-out key is `frames`, figures-panel
        // must be PRESENT, not absent.
        if absent == "frames" {
            assert!(
                keys.contains("figures-panel"),
                "scene {name:?}: figures-panel mounts in this workspace"
            );
        }

        // The scene's switch persisted through
        // `select_workspace`, like a chip press — put
        // the shared prefs back to Animate before the
        // next runtime seeds from them (the rounds'
        // discipline).
        tap(&mut rt, "ws-animate");
        assert_eq!(rt.state.workspace, Workspace::Animate);
    }
}

// ----------------------------------------------------------------
// Round 13 (Wave 10): the dopesheet lane. The lane
// (`dopesheet.rs`) mounts inside Animate's timeline
// card only: a root keyed `dopesheet-lane`, one row
// header per figure (`dope-row-{fi}`), and one cell
// per frame (`dope-cell-{fi}-{frame}`, carrying the
// 0-based frame index — the same index
// `stats::figure_keyframes` returns and `core.current`
// holds). A cell press goes to the cell's frame and
// selects the cell's figure. The navigation test reads
// its targets back from state via `figure_keyframes`
// instead of hardcoding the `dopesheet-keys` scene's
// frame choices, so the scene may re-scatter its keys
// without breaking it.
// ----------------------------------------------------------------

#[test]
fn dopesheet_lane_mounts_in_animate_and_gates_out_in_draw() {
    use crate::rune_sec::workspaces::Workspace;

    let mut rt = editor();
    // Seed Animate through its chip (the shared-prefs
    // discipline), like every other workspace test.
    tap(&mut rt, "ws-animate");
    rt.frame(DT);
    assert_eq!(rt.state.workspace, Workspace::Animate);
    // The lane mounts with the timeline card — settle
    // (bounded) through the presence fade before
    // reading the tree, the sweep's discipline.
    for _ in 0..30 {
        if presented_keys(&rt).contains("dopesheet-lane") {
            break;
        }
        rt.frame(DT);
    }
    let keys = presented_keys(&rt);
    assert!(
        keys.contains("dopesheet-lane"),
        "Animate presents the dopesheet lane"
    );
    // One row header per figure in the project — the
    // count comes from state, not a hardcoded figure
    // count.
    let figures = rt.state.core.project.figures.len();
    assert!(figures >= 1, "the demo project has a figure");
    for fi in 0..figures {
        let header = format!("dope-row-{fi}");
        assert!(
            keys.contains(header.as_str()),
            "Animate presents row header {header:?}"
        );
    }

    // The lane is Animate's timeline card only: Draw
    // gates it out. Settle (bounded) through the
    // fade-out before the absence assert.
    tap(&mut rt, "ws-draw");
    rt.frame(DT);
    assert_eq!(rt.state.workspace, Workspace::Draw);
    for _ in 0..30 {
        if !presented_keys(&rt).contains("dopesheet-lane") {
            break;
        }
        rt.frame(DT);
    }
    assert!(
        !presented_keys(&rt).contains("dopesheet-lane"),
        "Draw gates the dopesheet lane out"
    );

    // Restore Animate for the shared prefs, as rounds
    // 2–12 do.
    tap(&mut rt, "ws-animate");
    assert_eq!(rt.state.workspace, Workspace::Animate);
}

#[test]
fn dopesheet_cell_press_goes_to_frame_and_selects_its_figure() {
    use crate::rune_sec::workspaces::Workspace;

    // Resolve the scene by name in the `scenes()`
    // table, the workspace-scene test's way — a rename
    // on either side fails loudly here instead of
    // silently losing the pixel proof's in-process
    // counterpart.
    let table = crate::rune_sec::scenes::scenes();
    let scene = table
        .iter()
        .find(|(n, _)| *n == "dopesheet-keys")
        .map(|(_, f)| *f)
        .unwrap_or_else(|| panic!("scene \"dopesheet-keys\" is registered in scenes()"));

    let mut rt = editor();
    scene(&mut rt.state);
    // The scene fn is a direct state write, outside
    // the runtime's dispatch — mark the tree changed
    // so the next frame re-runs the view (the suite's
    // own discipline after direct writes; see the
    // workspace-scene test).
    rt.invalidate();
    assert_eq!(
        rt.state.workspace,
        Workspace::Animate,
        "the dopesheet-keys scene lands on Animate"
    );
    rt.frame(DT);
    for _ in 0..30 {
        if presented_keys(&rt).contains("dopesheet-lane") {
            break;
        }
        rt.frame(DT);
    }
    assert!(
        presented_keys(&rt).contains("dopesheet-lane"),
        "the scene presents the dopesheet lane"
    );

    // The scene's guarantees, read back from state
    // rather than assumed: at least two figures,
    // figure 1 keyed somewhere, and the playhead
    // starting on a keyed frame (either figure's).
    let keys0 =
        stykanimate_rs::stats::figure_keyframes(&rt.state.core.project, 0);
    let keys1 =
        stykanimate_rs::stats::figure_keyframes(&rt.state.core.project, 1);
    assert!(
        rt.state.core.project.figures.len() >= 2,
        "the scene keys two figures"
    );
    assert!(
        !keys1.is_empty(),
        "the scene keys figure 1 at at least one frame"
    );
    assert!(
        keys0.contains(&rt.state.core.current)
            || keys1.contains(&rt.state.core.current),
        "the scene starts the playhead on a keyed frame"
    );

    // Press figure 1's keyed cell. `figure_keyframes`
    // returns frames in ascending order, so the first
    // key away from the playhead is the nearest one —
    // the goto is observable and the cell sits as far
    // left in the timeline as a differing key can. If
    // figure 1's only key IS the playhead's frame, the
    // press lands there and the selection half of the
    // contract still carries the test.
    let start = rt.state.core.current;
    let target = keys1
        .iter()
        .copied()
        .find(|f| *f != start)
        .unwrap_or(keys1[0]);
    let cell = format!("dope-cell-1-{target}");
    // Settle (bounded) until the cell itself is fully
    // presented through the lane's presence fade —
    // `scroll_into_view`'s second half, without its
    // right-column wheel (the timeline card is fixed
    // chrome, always in view).
    for _ in 0..90 {
        rt.frame(DT);
        if let Some((_, opacity)) = rt.presented(&cell) {
            if opacity > 0.99 {
                break;
            }
        }
    }
    assert!(
        presented_keys(&rt).contains(cell.as_str()),
        "figure 1's keyed cell {cell:?} is presented"
    );
    tap(&mut rt, &cell);
    assert_eq!(
        rt.state.core.current, target,
        "the cell press goes to its frame"
    );
    assert_eq!(
        rt.state.core.selected_figure, 1,
        "the cell press selects its figure"
    );

    // The scene's switch persisted Animate through
    // `select_workspace`, so the shared prefs already
    // hold the restore state — assert it, so a scene
    // change that moves workspaces fails here rather
    // than in a sibling test.
    assert_eq!(rt.state.workspace, Workspace::Animate);
}

#[test]
fn edit_menu_dropdown_renders_after_restyle() {
    let mut rt = editor();
    assert_eq!(rt.state.menus.open, None);

    // Round 2 proved the File menu end to end; this is
    // the Edit menu, whose dropdown carries a kit
    // divider after the restyle (charter §9). The
    // dropdown is anchored, so a settling frame before
    // reading the tree (round 2's lesson).
    tap(&mut rt, "menu-edit");
    rt.frame(DT);
    assert_eq!(
        rt.state.menus.open,
        Some(crate::rune_sec::menus::EDIT),
        "Edit trigger opens the Edit menu"
    );
    let keys = presented_keys(&rt);
    assert!(
        keys.contains("menu-edit-dropdown"),
        "Edit dropdown surface presented"
    );
    // UI mission wave 3: the Edit menu no longer
    // re-mounts Undo/Redo (the app bar is their single
    // mount), and the separator that divided them from
    // the frame-clipboard group went with them — a
    // divider heading a dropdown divides nothing.
    assert!(
        !keys.contains("menu-edit-sep"),
        "no leading separator in the Edit dropdown after the dedup"
    );
    assert!(keys.contains("menu-scrim"), "dismissal scrim presented");

    // Scrim closes, and the dropdown leaves the tree
    // through the presence fade (bounded settle, round
    // 2's discipline).
    tap(&mut rt, "menu-scrim");
    assert_eq!(rt.state.menus.open, None, "scrim press closes the menu");
    for _ in 0..30 {
        if !presented_keys(&rt).contains("menu-edit-dropdown") {
            break;
        }
        rt.frame(DT);
    }
    assert!(
        !presented_keys(&rt).contains("menu-edit-dropdown"),
        "Edit dropdown dismissed"
    );
}

#[test]
fn palette_open_card_renders_and_closes() {
    let mut rt = editor();

    // Open the palette the real way — Ctrl+K, built
    // exactly as round 2's palette test builds it
    // (`is_command('k')` matches `Key::Command('k')`
    // only). The palette is now a kit card (charter
    // §9); what this proves is the mount: the restyled
    // overlay still appears as a centre-column child
    // gated on `s.palette.open` (main.rs `view`).
    press_key(
        &mut rt,
        Key::Command('k'),
        Mods {
            cmd: true,
            ..Mods::default()
        },
    );
    assert!(rt.state.palette.open, "Ctrl+K opens the palette");
    rt.frame(DT);
    let keys = presented_keys(&rt);
    assert!(
        keys.contains("command-palette"),
        "the palette card is mounted"
    );
    assert!(
        keys.contains("palette-query"),
        "the palette's query field is mounted"
    );

    // Escape closes through `palette_handle_key` (the
    // shell's first key hook), and the card leaves the
    // presented set through the presence fade (bounded
    // settle, as for the menus).
    press_key(&mut rt, Key::Escape, Mods::default());
    assert!(!rt.state.palette.open, "Escape closes the palette");
    for _ in 0..30 {
        if !presented_keys(&rt).contains("command-palette") {
            break;
        }
        rt.frame(DT);
    }
    assert!(
        !presented_keys(&rt).contains("command-palette"),
        "the palette card leaves the tree"
    );
}

#[test]
fn settings_card_renders_after_restyle() {
    let mut rt = editor();

    // The settings shell (keyed `settings`) is mounted
    // ungated at the top of the right column in every
    // workspace; the card inside it starts closed
    // (round 2's premise).
    assert!(!rt.state.settings.open, "settings card starts closed");
    let keys = presented_keys(&rt);
    assert!(keys.contains("settings"), "the settings shell is mounted");
    assert!(
        !keys.contains("settings-card"),
        "no settings card while closed"
    );

    // Open: the charter §9 popover surface in miniature
    // — the card mounts with its theme row and its
    // Playback speed row (round 10's chips live there).
    // Mounting lands through the presence fade, so
    // settle (bounded) before reading the set.
    tap(&mut rt, "settings-toggle");
    assert!(rt.state.settings.open, "Settings toggle opens the card");
    for _ in 0..30 {
        if presented_keys(&rt).contains("settings-card") {
            break;
        }
        rt.frame(DT);
    }
    let keys = presented_keys(&rt);
    for want in ["settings-card", "settings-theme", "settings-speed-row"] {
        assert!(keys.contains(want), "missing element keyed {want:?}");
    }

    // Close: the card unmounts (settled through the
    // fade) while the shell — and its toggle — remain.
    tap(&mut rt, "settings-toggle");
    assert!(!rt.state.settings.open, "Settings toggle closes the card");
    for _ in 0..30 {
        if !presented_keys(&rt).contains("settings-card") {
            break;
        }
        rt.frame(DT);
    }
    let keys = presented_keys(&rt);
    assert!(
        !keys.contains("settings-card"),
        "the settings card unmounts on close"
    );
    assert!(
        keys.contains("settings"),
        "the settings shell remains after the card closes"
    );
}

#[test]
fn timeline_card_keeps_chips_marks_and_rows() {
    use crate::rune_sec::workspaces::Workspace;

    let mut rt = editor();
    // Wave 10 (F2): the marker row mounts only when a
    // frame carries a camera or graph marker — the demo
    // has none, so plant one camera key to keep this
    // test's assertions about a mounted row.
    rt.state.core.project.set_camera_key(2, 0.0, 0.0, 1.0);
    rt.invalidate();
    tap(&mut rt, "ws-animate");
    rt.frame(DT);
    assert_eq!(rt.state.workspace, Workspace::Animate);

    // Charter §13 wrapped the whole timeline stack in
    // one UNKEYED card — the phase's riskiest
    // restructure for key survival. The card has no key
    // to assert, so its presence is proven by its keyed
    // children and their stacking relations (below):
    // one test, three sub-surfaces, because all three
    // moved into the card together.
    let keys = presented_keys(&rt);
    for want in [
        "timeline",
        "timeline-ruler",
        "timeline-chips",
        "timeline-markers",
        "timeline-loop-row",
        "loop-set",
    ] {
        assert!(keys.contains(want), "missing element keyed {want:?}");
    }
    // Exactly the demo's five chips — and no sixth.
    for n in 1..=5 {
        assert!(
            keys.contains(&format!("frame-{n}")),
            "chip frame-{n} is presented"
        );
    }
    assert!(
        !keys.contains("frame-6"),
        "no sixth chip in the five-frame demo"
    );

    // Stacking inside the card: a chip sits inside the
    // chips row, the chips row inside the scroll row,
    // and the loop row stacks directly under the scroll
    // row on the same left edge (the card's inset) —
    // the card wraps OUTSIDE the scroll row (charter
    // amendment), so the scrolled content's geometry
    // is untouched by the card's padding.
    let (scroll, _) = rt.presented("timeline").expect("scroll row is presented");
    let (chips, _) = rt
        .presented("timeline-chips")
        .expect("chips row is presented");
    let (chip1, _) = rt.presented("frame-1").expect("first chip is presented");
    assert!(
        rect_inside(chip1, chips),
        "chip inside the chips row: chip {chip1:?}, row {chips:?}"
    );
    assert!(
        rect_inside(chips, scroll),
        "chips row inside the scroll row: row {chips:?}, scroll {scroll:?}"
    );
    let (loop_row, _) = rt
        .presented("timeline-loop-row")
        .expect("loop row is presented");
    assert!(
        loop_row[1] >= scroll[1] + scroll[3] - 0.5,
        "loop row stacks below the scroll row: loop {loop_row:?}, scroll {scroll:?}"
    );
    assert!(
        (loop_row[0] - scroll[0]).abs() < 0.5,
        "loop row shares the scroll row's left edge (the card's inset): \
         loop {loop_row:?}, scroll {scroll:?}"
    );

    // The loop row's own control still works inside
    // the card — Set loop spans the demo from the
    // playhead (round 9's contract), the row stays
    // mounted with a range set, and Clear restores it.
    // (The timeline's own copies of these keys are the
    // first in tree order: the centre column mounts
    // before the right column's two other homes.)
    tap(&mut rt, "loop-set");
    assert_eq!(
        rt.state.core.loop_range,
        Some((0, 4)),
        "Set loop spans the demo from the playhead"
    );
    assert!(
        presented_keys(&rt).contains("timeline-loop-row"),
        "the loop row stays mounted with a range set"
    );
    tap(&mut rt, "loop-clear");
    assert_eq!(rt.state.core.loop_range, None, "Clear removes the range");

    // The clips lane joins the stack between the
    // scroll row and the loop row once a clip exists —
    // planted by direct project write exactly as round
    // 10's lane test plants it, with `invalidate`
    // because a direct write is outside any handler.
    rt.state.core.project.fx_clips.push(
        stykanimate_rs::model::FxClipEntry {
            clip: stykanimate_rs::vfx::ParticleClip {
                start: 2.0,
                duration: 3.0,
                kind: stykanimate_rs::vfx::ClipKind::Burst,
                color: 0xFFC65D,
                color2: None,
                seed: 7,
                angle: 0.0,
                length: 130.0,
            },
            figure: 0,
            node: 0,
            hidden: false,
        },
    );
    rt.invalidate();
    rt.frame(DT);
    let mut appeared = false;
    for _ in 0..30 {
        rt.frame(DT);
        if presented_keys(&rt).contains("timeline-clips") {
            appeared = true;
            break;
        }
    }
    assert!(appeared, "the clips lane joins the card once a clip exists");
    // Settle a few frames past the appearance: the
    // lane's entrance moves its drawn rect before it
    // rests.
    for _ in 0..30 {
        rt.frame(DT);
    }
    let (lane, _) = rt
        .presented("timeline-clips")
        .expect("clips lane is presented");
    let (loop_after, _) = rt
        .presented("timeline-loop-row")
        .expect("loop row is still presented");
    // Wave 10: the centre-order assertion this block
    // used to make (scroll < lane < loop) is no longer
    // a sound relation — Wave 5 moved the clips lane
    // INSIDE the scroll column (one shared offset), and
    // the dopesheet lane joined it there, so the scroll
    // row's drawn box now CONTAINS the lane and its
    // centre sits below the lane's. What the Runtime
    // can soundly prove: the lane mounts inside the
    // scroll row on the card's left inset, and the loop
    // row still stacks below the scroll row. The exact
    // stacking inside the scroll column is proven by
    // the headless tree dumps (overflow scanner rule 4
    // bounds the whole timeline subtree).
    // Inside-ness is asserted with the documented
    // presented-box allowance: `Runtime::presented`
    // returns the DRAWN box, and a mid-session-mounted
    // canvas (this lane mounted when the clip was
    // planted) can rest up to its own height above its
    // layout slot — a strict `rect_inside` on the top
    // edge flakes by exactly that displacement. The
    // horizontal span stays strict, the bottom stays
    // strict, and the top allows one lane height; the
    // EXACT stacking inside the scroll column is proven
    // by the headless tree dumps (overflow rule 4).
    assert!(
        lane[0] >= scroll[0] - 0.5
            && lane[0] + lane[2] <= scroll[0] + scroll[2] + 0.5
            && lane[1] >= scroll[1] - lane[3] - 0.5
            && lane[1] + lane[3] <= scroll[1] + scroll[3] + 0.5,
        "clips lane inside the scroll row (presented-box allowance): \
         lane {lane:?}, scroll {scroll:?}"
    );
    assert!(
        loop_after[1] >= scroll[1] + scroll[3] - 0.5,
        "loop row stacks below the scroll row: loop {loop_after:?}, scroll {scroll:?}"
    );
    assert!(
        (lane[0] - scroll[0]).abs() < 0.5,
        "lane shares the scroll row's left edge (the card's inset): \
         lane {lane:?}, scroll {scroll:?}"
    );
}

#[test]
fn stage_and_shell_furniture_survive() {
    use crate::rune_sec::workspaces::Workspace;

    let mut rt = editor();
    tap(&mut rt, "ws-animate");
    rt.frame(DT);
    assert_eq!(rt.state.workspace, Workspace::Animate);

    // The permanent furniture (charter §12, §14, §15 —
    // status strip, stage, app bar) is ungated: its
    // keys survive every workspace, and they survive
    // the restyle in this one.
    let keys = presented_keys(&rt);
    for want in [
        "app-bar",
        "menu-bar",
        "ws-animate",
        "main-row",
        "centre",
        "stage",
        "transport",
        "status-strip",
    ] {
        assert!(keys.contains(want), "missing shell furniture {want:?}");
    }

    // The stage (radius 12 now, a snap-proven fact)
    // keeps its place: fully inside the centre column,
    // sharing no area with either side column — the
    // relations round 7 pinned for the status strip,
    // applied to the stage itself after its restyle.
    let (stage, _) = rt.presented("stage").expect("stage is presented");
    let (centre, _) = rt.presented("centre").expect("centre column is presented");
    let (left, _) = rt.presented("side-left").expect("left column is presented");
    let (right, _) = rt
        .presented("side-right")
        .expect("right column is presented");
    assert!(
        rect_inside(stage, centre),
        "stage escapes the centre column: stage {stage:?}, centre {centre:?}"
    );
    assert!(
        !rects_intersect(stage, left),
        "stage runs under the left column: stage {stage:?}, column {left:?}"
    );
    assert!(
        !rects_intersect(stage, right),
        "stage runs under the right column: stage {stage:?}, column {right:?}"
    );
}

#[test]
fn fx_browser_and_draw_roots_present() {
    use crate::rune_sec::workspaces::Workspace;

    let mut rt = editor();
    // Effects: the FX browser (the charter's list-row
    // showcase) and the FX clips panel — whose card
    // root is `fx-section`, with the `fx-clips` lane
    // list inside it (the module docs carry the
    // correction to P17's draft).
    tap(&mut rt, "ws-effects");
    rt.frame(DT);
    assert_eq!(rt.state.workspace, Workspace::Effects);
    for _ in 0..30 {
        if presented_keys(&rt).contains("fx-browser") {
            break;
        }
        rt.frame(DT);
    }
    let keys = presented_keys(&rt);
    for want in ["fx-browser", "fx-section", "fx-clips"] {
        assert!(keys.contains(want), "Effects root {want:?} is presented");
    }

    // Draw: the stroke inspector (the heaviest
    // stepper/list adoption in that workspace) mounts
    // with the draw tools and the text tool, Figures
    // stays, and Animate's FrameOps leaves (settled,
    // bounded, before the absence assert).
    tap(&mut rt, "ws-draw");
    rt.frame(DT);
    assert_eq!(rt.state.workspace, Workspace::Draw);
    for _ in 0..30 {
        let keys = presented_keys(&rt);
        if keys.contains("stroke-inspector") && !keys.contains("frames") {
            break;
        }
        rt.frame(DT);
    }
    let keys = presented_keys(&rt);
    for want in ["stroke-inspector", "draw-tools", "text-tool", "figures-panel"] {
        assert!(keys.contains(want), "Draw root {want:?} is presented");
    }
    assert!(!keys.contains("frames"), "Draw gates FrameOps out");

    // Restore Animate for the shared prefs, as rounds
    // 2–10 do.
    tap(&mut rt, "ws-animate");
    assert_eq!(rt.state.workspace, Workspace::Animate);
}

#[test]
fn help_panel_renders_via_help_menu() {
    let mut rt = editor();
    // P17's conditional test, written because its
    // precondition held: the help panel's card root
    // kept the key `help-panel` through adoption. The
    // panel is ungated but mounts only while
    // `menus.help_open` is set (main.rs `view`, top of
    // the right column).
    assert!(!rt.state.menus.help_open, "help starts closed");
    assert!(
        !presented_keys(&rt).contains("help-panel"),
        "no help panel before Shortcuts is chosen"
    );

    // Open the Help menu (settling frame, round 2's
    // lesson) and choose Shortcuts — the dropdown item
    // keyed `help`, whose handler is the `help_open`
    // flip itself.
    tap(&mut rt, "menu-help");
    rt.frame(DT);
    assert_eq!(
        rt.state.menus.open,
        Some(crate::rune_sec::menus::HELP),
        "Help trigger opens the Help menu"
    );
    assert!(
        presented_keys(&rt).contains("help"),
        "the Shortcuts item is presented"
    );
    tap(&mut rt, "help");
    assert!(rt.state.menus.help_open, "Shortcuts toggles the help panel on");
    assert_eq!(
        rt.state.menus.open, None,
        "selecting the item closes the menu"
    );
    for _ in 0..30 {
        if presented_keys(&rt).contains("help-panel") {
            break;
        }
        rt.frame(DT);
    }
    assert!(
        presented_keys(&rt).contains("help-panel"),
        "the help card mounts at the top of the right column"
    );

    // The same path toggles it back off, leaving the
    // shell as the test found it.
    tap(&mut rt, "menu-help");
    rt.frame(DT);
    tap(&mut rt, "help");
    assert!(!rt.state.menus.help_open, "Shortcuts toggles the help panel off");
    for _ in 0..30 {
        if !presented_keys(&rt).contains("help-panel") {
            break;
        }
        rt.frame(DT);
    }
    assert!(
        !presented_keys(&rt).contains("help-panel"),
        "the help card unmounts"
    );
}

// Round 12 (Wave 11, MCP end-to-end): the live-MCP-visible
// shell surface. See the file header's round 12 note for
// the facts these tests rest on; each test names the
// Wave 11 guarantee it pins and the agent change it
// depends on, so a failure triages to one owner.

/// Run one MCP `tools/call` against the runtime's LIVE
/// core through the real dispatcher — the exact call
/// `live_mcp::drain` makes for a queued request
/// (live_mcp.rs:346), minus the TCP hop. Returns the
/// JSON-RPC response.
fn mcp_call(
    rt: &mut Runtime<State>,
    name: &str,
    args: serde_json::Value,
) -> serde_json::Value {
    let mut server = stykanimate_rs::mcp::McpServer::new();
    let msg = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": { "name": name, "arguments": args },
    });
    server
        .handle_on(&mut rt.state.core, &msg)
        .expect("tools/call is a request and must be answered")
}

/// The parsed JSON document a `tools/call` result carries
/// in its single text content block (the `mcp.rs` test
/// suite's own extraction, `call_payload_on`).
fn mcp_payload(resp: &serde_json::Value) -> serde_json::Value {
    let text = resp["result"]["content"][0]["text"]
        .as_str()
        .expect("content block carries text");
    serde_json::from_str(text).unwrap_or(serde_json::Value::String(text.to_string()))
}

/// The (items index, severity) of the live toast whose
/// text is exactly `text`, if the shell is showing one.
fn toast_with_text(
    rt: &Runtime<State>,
    text: &str,
) -> Option<(usize, crate::rune_sec::toasts::Severity)> {
    rt.state
        .toasts
        .items
        .iter()
        .position(|t| t.text == text)
        .map(|i| (i, rt.state.toasts.items[i].severity))
}

/// Every text the last frame's elements carry (the round
/// 7 technique: `Runtime::elements` reports text the key
/// set cannot see).
fn element_texts(rt: &Runtime<State>) -> Vec<String> {
    rt.elements()
        .iter()
        .filter_map(|e| e.text.clone())
        .collect()
}

#[test]
fn live_mcp_handle_on_add_figure_renders_in_the_figures_panel() {
    // Wave 11 guarantee: a mutation an MCP client makes
    // against the live core — no GUI handler involved —
    // is what the shell renders on its next frame: the
    // figures panel gains the row and the count, and the
    // stage's shell names the new figure. Depends on:
    // A3 (main.rs poll/redraw — the frame must re-run the
    // view over the mutated core) and the dispatcher's
    // `handle_on` contract (mcp.rs:161, live_mcp.rs:346).
    let mut rt = editor();
    tap(&mut rt, "ws-animate");
    rt.frame(DT);
    assert_eq!(
        rt.state.core.project.figures.len(),
        1,
        "the demo project starts with one figure"
    );
    assert!(
        !presented_keys(&rt).contains("fig-row-1"),
        "no second figure row before the call"
    );

    // The client's call, verbatim: the registry's
    // add_figure tool (mcp_tools.rs) against the live
    // core, behind the shell's back.
    let resp = mcp_call(&mut rt, "add_figure", serde_json::json!({ "kind": "humanoid" }));
    let payload = mcp_payload(&resp);
    assert_eq!(payload["ok"], true, "the tool reports success: {payload}");
    assert_eq!(
        rt.state.core.project.figures.len(),
        2,
        "the live core gained the figure"
    );
    assert_eq!(
        rt.state.core.status, "Figure 2 added.",
        "the core's status is the bare engine text — the MCP attribution \
         prefix is a toast-site stamp, never written here (main.rs, A3)"
    );

    // Direct state writes need the invalidate discipline
    // (the file's standing rule) before a tree assert.
    rt.invalidate();
    rt.frame(DT);

    let keys = presented_keys(&rt);
    assert!(
        keys.contains("fig-row-1"),
        "the figures panel renders the MCP-added figure's row"
    );
    assert!(
        keys.contains("stage"),
        "the stage stays mounted over the mutated project"
    );
    let texts = element_texts(&rt);
    assert!(
        texts.iter().any(|t| t == "Styk 2"),
        "the new figure's name is on screen (panel row / status strip): {texts:?}"
    );
    // The panel header count: the bare total ("2",
    // figures_panel::count_line) as a text element in
    // the "Figures" title's own band, right of the title.
    let title = rt
        .elements()
        .into_iter()
        .find(|e| e.text.as_deref() == Some("Figures"))
        .expect("the figures panel title is presented");
    let title_cy = title.rect[1] + title.rect[3] / 2.0;
    let count_shown = rt.elements().iter().any(|e| {
        e.text.as_deref() == Some("2")
            && e.rect[0] > title.rect[0]
            && ((e.rect[1] + e.rect[3] / 2.0) - title_cy).abs() < 8.0
    });
    assert!(
        count_shown,
        "the figures panel header count reads 2 after the MCP add"
    );
}

#[test]
fn live_mcp_refusal_status_toasts_through_poll_unstamped() {
    // Wave 11 guarantee: a status an MCP call writes on
    // the live core reaches the user as a toast through
    // the ordinary poll path, classified by its body —
    // and a bare `handle_on` (this test's simulation)
    // does NOT earn the `MCP: ` stamp: the stamp is
    // applied only when the drain inside poll wrote the
    // status (A3, main.rs). Depends on: A3 (poll's
    // status-to-toast wiring + the drain-window stamp
    // boundary) and A7 (classify_status's grammar,
    // toasts.rs — a "No …" refusal body is Warn).
    let mut rt = editor();
    tap(&mut rt, "ws-animate");
    rt.frame(DT);

    // Build a frame with no keyframe for figure 0: the
    // duplicate holds the previous pose, so by the
    // dopesheet's definition (a key is a pose CHANGE)
    // frame 2 (1-based) is not keyed.
    let dup = mcp_call(&mut rt, "duplicate_frame", serde_json::json!({}));
    assert_eq!(mcp_payload(&dup)["ok"], true, "the duplicate lands");
    assert_eq!(rt.state.core.project.frames.len(), 6);

    // The refusal: deleting a keyframe that is not
    // there. The tool returns an error result carrying
    // the status, AND the op leaves that status on the
    // core (editor.rs delete_keyframe) — the line poll
    // will toast.
    let resp = mcp_call(
        &mut rt,
        "delete_keyframe",
        serde_json::json!({ "figure": 0, "frame": 1 }),
    );
    assert_eq!(
        resp["result"]["isError"], true,
        "the refusal is an error result: {resp}"
    );
    assert_eq!(
        rt.state.core.status, "No keyframe for figure 1 at frame 2.",
        "the refusal status sits on the live core"
    );

    rt.invalidate();
    rt.frame(DT);

    let (index, severity) =
        toast_with_text(&rt, "No keyframe for figure 1 at frame 2.")
            .expect("poll toasted the MCP-written status");
    assert_eq!(
        severity,
        crate::rune_sec::toasts::Severity::Warn,
        "a \"No …\" refusal body classifies as Warn"
    );
    assert!(
        presented_keys(&rt).contains(&format!("toast-{index}")),
        "the toast card is in the tree under its items-index key"
    );
    assert!(
        rt.state
            .toasts
            .items
            .iter()
            .all(|t| !t.text.starts_with(crate::rune_sec::toasts::MCP_STATUS_PREFIX)),
        "no toast is MCP-stamped: the stamp belongs to the drain in poll, \
         not to the dispatch (A3's boundary, main.rs)"
    );
}

#[test]
fn mcp_stamped_statuses_classify_by_their_body_at_the_shell_surface() {
    // Wave 11 guarantee: when poll DOES stamp a status
    // (the drain wrote it — main.rs, A3), the stamped
    // text the user sees classifies by its BODY, not by
    // the prefix: `"MCP: No …"` is the refusal it names
    // (Warn — the unstamped grammar alone would fall
    // through to Info, because the string no longer
    // starts with "No "), a `"… failed"` body is Error,
    // and the MCP-layer's own failure shape
    // (`unknown tool: …`) is Error outright. Pinned here
    // through `push_status` — the exact call poll makes
    // with the stamped string — against the CURRENT
    // contract of A7's grammar (toasts.rs rule 0 +
    // MCP_STATUS_PREFIX). DEPENDENCY: if A7 changes the
    // prefix or rule 0, this test is the reconciliation
    // point — its strings are built from the module's
    // own constant, so a prefix change fails HERE, by
    // design, for the integrator to reconcile.
    use crate::rune_sec::toasts::{Severity, MCP_STATUS_PREFIX};

    let mut rt = editor();
    rt.frame(DT);
    let now = rt.state.toast_now;

    let refusal = format!("{MCP_STATUS_PREFIX}No keyframe for figure 1 at frame 2.");
    let failure = format!("{MCP_STATUS_PREFIX}Audio fade failed: no clip 1.");
    let unknown = format!("{MCP_STATUS_PREFIX}unknown tool: frobnicate");
    rt.state.toasts.push_status(refusal.clone(), now);
    rt.state.toasts.push_status(failure.clone(), now);
    rt.state.toasts.push_status(unknown.clone(), now);
    rt.invalidate();
    rt.frame(DT);

    for (text, want) in [
        (refusal.as_str(), Severity::Warn),
        (failure.as_str(), Severity::Error),
        (unknown.as_str(), Severity::Error),
    ] {
        let (index, severity) =
            toast_with_text(&rt, text).expect("the stamped status is a live toast");
        assert_eq!(
            severity, want,
            "stamped status {text:?} classifies by its body"
        );
        assert!(
            presented_keys(&rt).contains(&format!("toast-{index}")),
            "the stamped toast's card is in the tree: {text:?}"
        );
    }
    let texts = element_texts(&rt);
    assert!(
        texts.iter().any(|t| t == &refusal),
        "the card shows the stamped text verbatim, prefix included: {texts:?}"
    );
}

/// The settings Live MCP toggle's knob, measured as the
/// distance from the toggle row's right edge to the
/// knob's centre: the kit pill is 28 pt wide with a
/// 12 pt knob at a 2 pt inset (design.rs `toggle_pill`),
/// so OFF reads ≈ 20 pt and ON reads ≈ 8 pt. The gap is
/// translation-invariant — the settings card's entrance
/// transition shifts row and knob together.
fn live_mcp_knob_gap(rt: &Runtime<State>) -> Option<f32> {
    let (row, _) = rt.presented("settings-live-mcp")?;
    let row_right = row[0] + row[2];
    let knob = rt.elements().into_iter().find(|e| {
        let cx = e.rect[0] + e.rect[2] / 2.0;
        let cy = e.rect[1] + e.rect[3] / 2.0;
        (e.rect[2] - 12.0).abs() < 1.0
            && (e.rect[3] - 12.0).abs() < 1.0
            && cx > row[0]
            && cx < row_right
            && cy > row[1]
            && cy < row[1] + row[3]
    })?;
    Some(row_right - (knob.rect[0] + knob.rect[2] / 2.0))
}

/// Open the settings card and wait (bounded) for the
/// Live MCP row to be presented; returns the knob gap.
fn open_settings_and_read_knob(rt: &mut Runtime<State>) -> f32 {
    tap(rt, "settings-toggle");
    assert!(rt.state.settings.open, "Settings toggle opens the card");
    for _ in 0..30 {
        if presented_keys(rt).contains("settings-live-mcp") {
            break;
        }
        rt.frame(DT);
    }
    assert!(
        presented_keys(rt).contains("settings-live-mcp"),
        "the Live MCP row is presented in the open card"
    );
    live_mcp_knob_gap(rt).expect("the toggle pill's knob is in the tree")
}

#[test]
fn settings_live_mcp_toggle_shows_the_persisted_state_for_both_prefs_values() {
    // This test starts and stops the singleton bridge:
    // serialise against live_mcp's socket tests (Wave 14
    // CI fix 6).
    let _bridge_guard = crate::live_mcp::BRIDGE_TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    // Wave 11 guarantee: the Settings card's Live MCP
    // toggle SHOWS the persisted preference — prefs
    // `live_mcp: false` starts the editor with the
    // bridge off and the pill off; `true` starts it on
    // with the pill on — and a press flips the bridge,
    // persists the RESULT, and says so. Depends on: A4
    // (init's prefs-to-bridge start, main.rs) and A6
    // (the settings row reading the bridge itself,
    // settings_popover.rs — no mirrored bool to drift).
    //
    // Seeding: this suite has no prefs-seeding pattern
    // (scenes drive handlers, never the file), so the
    // test writes ./projects/.layout.json through
    // `shell::save_layout` — the app's own load-modify-
    // save path, cloned from the live prefs so ONLY
    // `live_mcp` differs — and restores the file and
    // the bridge at the end. One asymmetry the seeding
    // must respect: `init` only ever switches the
    // bridge ON from prefs (main.rs), so the off phase
    // also calls `set_enabled(false)` — the state a
    // fresh process would hold.
    use stykanimate_rs::shell::{load_layout, save_layout};

    let orig_prefs = load_layout();
    let orig_bridge = crate::live_mcp::is_enabled();

    // --- Phase 1: persisted OFF. ---
    let mut prefs = orig_prefs.clone();
    prefs.live_mcp = false;
    save_layout(&prefs);
    let _ = crate::live_mcp::set_enabled(false);
    let mut rt = editor();
    assert!(
        !crate::live_mcp::is_enabled(),
        "prefs live_mcp=false starts the editor with the bridge off"
    );
    let gap = open_settings_and_read_knob(&mut rt);
    assert!(
        gap > 14.0,
        "the toggle pill reads OFF (knob gap {gap:.1}, off ≈ 20, on ≈ 8)"
    );

    // A press flips the bridge, persists the result and
    // repaints the pill — the row reads the bridge, so
    // the same card now shows ON.
    tap(&mut rt, "settings-live-mcp");
    assert!(
        crate::live_mcp::is_enabled(),
        "the press started the bridge"
    );
    assert!(
        load_layout().live_mcp,
        "the press persisted the resulting state to the prefs"
    );
    assert!(
        rt.state.core.status.starts_with("Live MCP on"),
        "the press reports itself on the status line: {}",
        rt.state.core.status
    );
    for _ in 0..30 {
        if let Some(g) = live_mcp_knob_gap(&rt) {
            if g < 14.0 {
                break;
            }
        }
        rt.frame(DT);
    }
    let gap = live_mcp_knob_gap(&rt).expect("the knob is still in the tree");
    assert!(
        gap < 14.0,
        "the toggle pill now reads ON (knob gap {gap:.1})"
    );

    // --- Phase 2: persisted ON, a fresh editor. ---
    let mut prefs = orig_prefs.clone();
    prefs.live_mcp = true;
    save_layout(&prefs);
    let mut rt2 = editor();
    assert!(
        crate::live_mcp::is_enabled(),
        "prefs live_mcp=true starts the bridge in init (A4)"
    );
    let gap = open_settings_and_read_knob(&mut rt2);
    assert!(
        gap < 14.0,
        "the toggle pill reads ON from the persisted pref (knob gap {gap:.1})"
    );

    // Leave the process as the test found it.
    save_layout(&orig_prefs);
    let _ = crate::live_mcp::set_enabled(orig_bridge);
}

// ----------------------------------------------------------------
// Round 13 (Wave 12): the styled stage, the segment
// inspector's attached-media state, and the populated
// Home library. See the file header's round 13 note for
// the facts these tests rest on; each test names the
// Wave 12 guarantee it pins and the agent change it
// depends on, so a failure triages to one owner.
// ----------------------------------------------------------------

/// A real, decodable 4×2 PNG (solid green), built the
/// way the lib's own fixtures build one (pivot.rs's
/// `test_png`): an asset's bytes must decode wherever
/// the render paths ask for dimensions, so a dummy byte
/// string would silently exercise only the failure
/// path.
fn tiny_png() -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(4, 2, image::Rgba([0, 200, 0, 255]));
    let mut buf = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .expect("encoding a 4x2 PNG cannot fail");
    buf
}

#[test]
fn stage_renders_a_tapered_point_cap_translucent_segment() {
    // Wave 12 guarantee: a segment whose node carries a
    // non-default style — taper ≠ 1, a Point cap,
    // opacity < 1 — is what the stage draws, because the
    // stage's segment pass now runs through
    // `paint::styled_segment_pieces` (stage_view.rs),
    // the shared construction both exporters already
    // used. Depends on: A-squad's stage adoption of the
    // styled pieces. The pieces are canvas draws, not
    // elements, so this test pins the Runtime-visible
    // contract honestly: the style state lands through
    // the registered core ops (via round 12's `mcp_call`
    // — the writes a live MCP client would make), the
    // stage stays mounted at real size over the styled
    // project across settled frames (the styled draw
    // path executes every frame; a panic in it fails
    // here), and joint pressing still selects on the
    // styled figure. Piece widths/alphas are proven at
    // the lib level (paint.rs's styled-pieces tests),
    // not here.
    let mut rt = editor();
    tap(&mut rt, "ws-animate");
    rt.frame(DT);

    // Premise: the demo figure's node 2 ("head", parent
    // node 1 — a real segment) carries the default
    // style.
    let style = rt.state.core.project.figures[0].def.nodes[2].style;
    assert_eq!(style.taper, 1.0, "demo segment starts untapered");
    assert_eq!(
        style.cap,
        stykanimate_rs::model::CapKind::Round,
        "demo segment starts round-capped"
    );
    assert_eq!(style.opacity, 1.0, "demo segment starts opaque");

    // The writes, through the real dispatcher: taper by
    // its own tool, cap + opacity by the atomic style
    // tool (the style panel's write path, mcp_tools.rs).
    let resp = mcp_call(
        &mut rt,
        "set_node_taper",
        serde_json::json!({ "figure": 0, "node": 2, "taper": 0.3 }),
    );
    assert_eq!(
        mcp_payload(&resp)["ok"], true,
        "the taper write reports success: {resp}"
    );
    let resp = mcp_call(
        &mut rt,
        "set_node_style",
        serde_json::json!({
            "figure": 0, "node": 2,
            "color": 0x3366CC, "cap": "point", "opacity": 0.5,
        }),
    );
    let payload = mcp_payload(&resp);
    assert_eq!(payload["ok"], true, "the style write reports success: {payload}");
    assert_eq!(payload["cap"], "point", "the write names the cap it stored");

    // The state the stage will read on its next frame.
    let style = rt.state.core.project.figures[0].def.nodes[2].style;
    assert!(
        (style.taper - 0.3).abs() < 1e-6,
        "taper stored as written: {}",
        style.taper
    );
    assert_eq!(
        style.cap,
        stykanimate_rs::model::CapKind::Point,
        "the Point cap is stored"
    );
    assert!(
        (style.opacity - 0.5).abs() < 1e-6,
        "opacity stored as written: {}",
        style.opacity
    );
    assert_eq!(
        rt.state.core.selected_node,
        Some(2),
        "the style writes select the styled node (sty_select_node)"
    );

    rt.invalidate();
    rt.frame(DT);

    // The stage mounts over the styled project and the
    // styled draw path runs, frame after frame.
    let (rect, _opacity) = rt.presented("stage").expect("stage is presented");
    assert!(
        rect[2] > 100.0 && rect[3] > 100.0,
        "stage has a real size over the styled project: {rect:?}"
    );
    for _ in 0..10 {
        rt.frame(DT);
    }
    assert!(
        presented_keys(&rt).contains("stage"),
        "the stage stays mounted across settled frames"
    );

    // Joint pressing still lands on the styled figure:
    // aim at the root joint exactly as round 1's drag
    // test does, and watch the selection move OFF the
    // styled node onto the root.
    let core = &rt.state.core;
    let frame = core.current;
    let root = core.positions(frame, 0)[0];
    let vt = view_transform([rect[2], rect[3]], core.project.stage, core.camera_here());
    let local = to_screen(&vt, root);
    rt.pointer_down(rect[0] + local[0], rect[1] + local[1]);
    rt.frame(DT);
    assert_eq!(
        rt.state.core.selected_node,
        Some(0),
        "a press on the styled figure's root joint selects it"
    );
    rt.pointer_up(rect[0] + local[0], rect[1] + local[1]);
    rt.frame(DT);
}

#[test]
fn segment_inspector_media_section_shows_an_attached_sprite() {
    // Wave 12 guarantee: a segment with a sprite
    // attached shows that attachment in the segment
    // inspector — the readout names the sprite, the
    // per-asset picker offers the project's assets, and
    // Clear renders because media is attached. Depends
    // on: B2 (the inspector's Media section,
    // inspector_segment.rs — before this wave the panel
    // had no media surface at all). Construction is
    // direct model state plus the panel's own attach
    // press: B2's MCP handlers exist in media.rs but
    // their Tool entries are the integrator's splice,
    // so the registered-dispatch path cannot attach yet
    // — the button press below IS the panel path a user
    // takes (its handler is the module's `attach_asset`).
    let mut rt = editor();
    tap(&mut rt, "ws-animate");
    rt.frame(DT);

    // Seed: one asset in the project's table, and the
    // demo figure's node 2 selected (the inspector's
    // full form renders for a selected node).
    rt.state.core.project.assets.push(stykanimate_rs::model::ImageAsset {
        name: "badge.png".into(),
        png: tiny_png(),
    });
    rt.state.core.selected_node = Some(2);
    rt.invalidate();
    rt.frame(DT);

    // Before the attach: the section renders its empty
    // state — readout names nothing, the picker offers
    // the one asset, and Clear is absent (it renders
    // only while media is attached).
    let keys = presented_keys(&rt);
    assert!(
        keys.contains("inspector-segment"),
        "the segment inspector is mounted"
    );
    assert!(
        keys.contains("seg-media-readout"),
        "the media readout renders for the selected segment"
    );
    assert!(
        keys.contains("seg-media-attach-0"),
        "the picker offers the project's one asset"
    );
    assert!(
        !keys.contains("seg-media-clear"),
        "no Clear button before anything is attached"
    );
    let texts = element_texts(&rt);
    assert!(
        texts.iter().any(|t| t == "No media attached"),
        "the readout names the empty slot: {texts:?}"
    );

    // The attach press — the picker's own button, deep
    // in the right column, so round 6's scroll discipline
    // brings it into view first.
    scroll_into_view(&mut rt, "seg-media-attach-0");
    tap(&mut rt, "seg-media-attach-0");
    assert_eq!(
        rt.state.core.project.figures[0].def.nodes[2].media,
        Some(stykanimate_rs::model::SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false, opacity: 1.0 }),
        "the press attached asset 0 to the selected segment"
    );

    rt.invalidate();
    rt.frame(DT);

    // The attached state renders: the readout names the
    // sprite (`media_label`'s Sprite form) and Clear is
    // now on the panel.
    let keys = presented_keys(&rt);
    assert!(
        keys.contains("seg-media-clear"),
        "Clear renders once media is attached"
    );
    assert!(
        keys.contains("seg-media-attach-0"),
        "the picker still offers the asset (its button now renders active)"
    );
    let texts = element_texts(&rt);
    assert!(
        texts.iter().any(|t| t == "Sprite: badge.png"),
        "the readout names the attached sprite: {texts:?}"
    );
    assert!(
        !texts.iter().any(|t| t == "No media attached"),
        "the empty-slot line is gone"
    );
}

#[test]
fn home_workspace_lists_the_seeded_projects() {
    // Wave 12 guarantee: with saved projects on disk the
    // Home workspace renders the library ROWS — the
    // populated state D2's home-populated scene pins for
    // the headless snaps — not the empty message. The
    // rows come from a disk scan of ./projects
    // (home_panel.rs `projects()`), so the seed goes
    // through the app's own write path:
    // `save_named_project` twice under two names (the
    // state-form mirror of the scene's seeding), and the
    // test removes its two files again afterwards, the
    // restore discipline round 12's settings test set
    // for this directory. Depends on: D2 (the populated
    // library rendering itself) and the panel's row keys
    // (`home-row-{i}` under `home-rows`, `home-totals`
    // in the non-empty branch only).
    let mut rt = editor();

    // --- Seed: two saves under two names. ---
    rt.state.core.project.name = "Wave12 Alpha".into();
    assert!(
        rt.state.core.save_named_project(),
        "the first seed save writes its file: {}",
        rt.state.core.status
    );
    let alpha_path =
        rt.state.core.project_files[rt.state.core.project_index].clone();
    rt.state.core.project.name = "Wave12 Beta".into();
    assert!(
        rt.state.core.save_named_project(),
        "the second seed save writes its file: {}",
        rt.state.core.status
    );
    let beta_path =
        rt.state.core.project_files[rt.state.core.project_index].clone();
    assert_ne!(alpha_path, beta_path, "the two saves are two files");

    // --- Mount Home, panel open. ---
    tap(&mut rt, "ws-home");
    rt.state.home_panel.open = true;
    rt.invalidate();
    rt.frame(DT);
    // The scan (and its per-file thumbnail renders) is
    // the panel's first-render cost; give the mount the
    // bounded settle the file's structural asserts use.
    for _ in 0..30 {
        if presented_keys(&rt).contains("home-rows") {
            break;
        }
        rt.frame(DT);
    }

    let keys = presented_keys(&rt);
    assert!(keys.contains("home-panel"), "the Home panel is mounted");
    assert!(
        keys.contains("home-rows"),
        "the row list renders for a populated library"
    );
    assert!(
        keys.contains("home-row-0") && keys.contains("home-row-1"),
        "at least the two seeded projects render as rows"
    );
    assert!(
        keys.contains("home-totals"),
        "the totals footer renders (non-empty branch only)"
    );
    let texts = element_texts(&rt);
    assert!(
        texts.iter().any(|t| t == "Wave12 Alpha"),
        "the first seeded project names its row: {texts:?}"
    );
    assert!(
        texts.iter().any(|t| t == "Wave12 Beta"),
        "the second seeded project names its row: {texts:?}"
    );
    assert!(
        !texts.iter().any(|t| t
            == "No saved projects yet — use \"Save project\" to add one."),
        "the empty-library line is absent from a populated library"
    );

    // Leave the directory as the test found it.
    let _ = std::fs::remove_file(&alpha_path);
    let _ = std::fs::remove_file(&beta_path);
}

// ----------------------------------------------------------------
// Round 14 (Wave 13): the Sprites panel flow and the
// populated library's thumbnail canvases. See the file
// header's round 14 note for the facts these tests rest
// on; each test names the Wave 13 guarantee it pins and
// the agent change it depends on, so a failure triages
// to one owner.
// ----------------------------------------------------------------

/// Scroll whichever column holds the element keyed
/// `key` until it sits inside the viewport, then settle
/// (bounded) until it is fully presented — round 6's
/// `scroll_into_view` generalised: that helper wheels
/// over the right column only, and the Sprites panel's
/// column is E8's gating choice, so this one wheels
/// over the target's own centre x, in both directions,
/// with the same step and settle discipline.
fn scroll_any_column_into_view(rt: &mut Runtime<State>, key: &str) {
    for _ in 0..24 {
        let (x, y) = rt
            .find(key)
            .unwrap_or_else(|| panic!("no element keyed {key:?}"));
        if y > 850.0 {
            rt.wheel(x, 450.0, -600.0);
        } else if y < 200.0 {
            rt.wheel(x, 450.0, 600.0);
        } else {
            break;
        }
        rt.frame(DT);
    }
    for _ in 0..90 {
        rt.frame(DT);
        if let Some((_, opacity)) = rt.presented(key) {
            if opacity > 0.99 {
                break;
            }
        }
    }
}

#[test]
fn sprites_panel_adds_and_deletes_an_instance() {
    // Wave 13 guarantee: a sprite can be added and
    // deleted from the Sprites panel itself — before
    // this wave instances arrived only via .nodes
    // import and could not be removed in the app at
    // all. Depends on: A3 (the panel and its shipped
    // keys — the picker's `sprite-add-{a}` buttons,
    // the rows' `sprite-select-{i}` / `sprite-del-{i}`),
    // E8 (the workspace gating — the test discovers
    // the host rather than presuming it) and A1 (the
    // engine ops the panel forwards to, landed:
    // `add_sprite` / `remove_sprite` in
    // sprite_ops.rs). The asserted core state is A1's
    // shipped contract: one instance for the picked
    // asset, holding one key at frame 0 at the stage
    // centre, scale 1.0, angle 0.0.
    let mut rt = editor();

    // Seed: exactly one asset in the project's table
    // (round 13's direct-write discipline — the asset
    // table is in-project state, no files involved),
    // so the panel's add has exactly one asset it can
    // pick. The demo project starts with no sprites.
    assert!(
        rt.state.core.project.sprites.is_empty(),
        "the demo project starts with no sprite instances"
    );
    rt.state.core.project.assets.push(stykanimate_rs::model::ImageAsset {
        name: "sprite.png".into(),
        png: tiny_png(),
    });
    rt.invalidate();
    rt.frame(DT);

    // Discover the hosting workspace: walk the chips
    // in row order and settle where the panel itself
    // presents. Every workspace gets the same bounded
    // settle the populated-Home test uses for its
    // first render.
    let mut hosted = false;
    for chip in [
        "ws-animate",
        "ws-draw",
        "ws-graph",
        "ws-effects",
        "ws-audio",
        "ws-3d",
        "ws-export",
        "ws-home",
    ] {
        tap(&mut rt, chip);
        for _ in 0..30 {
            if presented_keys(&rt).contains("sprite-panel") {
                hosted = true;
                break;
            }
            rt.frame(DT);
        }
        if hosted {
            break;
        }
    }
    assert!(
        hosted,
        "no workspace presents the Sprites panel (sprite-panel)"
    );

    // The add press — the picker's button for the one
    // seeded asset, wherever in its column it sits.
    scroll_any_column_into_view(&mut rt, "sprite-add-0");
    tap(&mut rt, "sprite-add-0");

    // The instance exists in core state, shaped as
    // A1's op writes it.
    {
        let core = &rt.state.core;
        assert_eq!(
            core.project.sprites.len(),
            1,
            "the panel's add created exactly one instance: {}",
            core.status
        );
        let sprite = &core.project.sprites[0];
        assert_eq!(sprite.asset, 0, "the instance references the seeded asset");
        assert_eq!(
            sprite.keys.len(),
            1,
            "the new instance holds its single frame-0 key"
        );
        let key = sprite.keys[0];
        assert_eq!(key.frame, 0.0, "the key sits at frame 0");
        let centre = [core.project.stage[0] / 2.0, core.project.stage[1] / 2.0];
        assert!(
            (key.x - centre[0]).abs() < 1e-6 && (key.y - centre[1]).abs() < 1e-6,
            "the key places the sprite at the stage centre: ({}, {})",
            key.x,
            key.y
        );
        assert_eq!(key.scale, 1.0, "the key's scale is 1.0");
        assert_eq!(key.angle, 0.0, "the key's angle is 0.0");
    }

    rt.invalidate();
    rt.frame(DT);
    assert!(
        presented_keys(&rt).contains("sprite-row-0"),
        "the panel lists the new instance as its first row"
    );

    // Delete through the row's own Delete button: the
    // instance leaves core state and the row leaves
    // the tree. (The add press already selected the
    // new instance — `add_from_asset` selects what it
    // places — and `sprite-del-{i}` is per-row, so no
    // separate selection step is needed.)
    scroll_any_column_into_view(&mut rt, "sprite-del-0");
    tap(&mut rt, "sprite-del-0");
    assert!(
        rt.state.core.project.sprites.is_empty(),
        "the panel's delete removed the instance: {}",
        rt.state.core.status
    );

    // The row leaves the presented set through the
    // presence settle (bounded loop, the palette card's
    // pattern at `command_palette_dismisses` above): a
    // single frame is not the house guarantee for a
    // leaving element.
    rt.invalidate();
    for _ in 0..30 {
        if !presented_keys(&rt).contains("sprite-row-0") {
            break;
        }
        rt.frame(DT);
    }
    assert!(
        !presented_keys(&rt).contains("sprite-row-0"),
        "the deleted instance's row is gone from the panel"
    );
}

#[test]
fn library_panel_presents_thumbnail_canvases_for_seeded_entries() {
    // Wave 13 guarantee: in the populated library state
    // D1's library-populated scene pins for the
    // headless snaps, every entry's row presents its
    // thumbnail canvas (`library-thumb-{i}`, `i` the
    // entry's index in `core.library`) — the canvases
    // Wave 12's D5 built (each row's own def, drawn at
    // render) and whose pixel proof was the Wave 12
    // remainder this scene closes. Depends on: D1
    // (the scene's in-memory seeding this test mirrors
    // in state form) and D5's row construction
    // (library_panel.rs `entry_row`). Thumbnails are
    // canvas draws, not elements with children: the
    // Runtime proves the canvases are PRESENTED for
    // the built-in and both seeded indices; the drawn
    // figure itself is the lib's unit-test surface
    // (library_panel.rs's thumb-prims tests) and the
    // headless snap's.
    let mut rt = editor();
    tap(&mut rt, "ws-animate");
    rt.frame(DT);

    // The built-ins alone populate the library
    // (`load_library`'s three), so the panel and its
    // first thumbnails exist before any seeding.
    let base = rt.state.core.library.len();
    assert!(
        base >= 3,
        "the library starts with at least the three built-ins: {base}"
    );

    // Seed the populated state in memory, as D1's
    // scene does: a styled entry (one node tapered,
    // with a gradient end colour — the styled
    // thumbnail path) and a plain copy beside it. No
    // files are read or written, so nothing needs
    // cleaning up afterwards.
    let plain = rt.state.core.library[0].1.clone();
    let mut styled = plain.clone();
    if let Some(node) = styled.nodes.get_mut(1) {
        node.style.taper = 0.4;
        node.style.color2 = Some(0x2B6FD2);
    }
    rt.state.core.library.push(("Wave13 Styled".into(), styled));
    rt.state.core.library.push(("Wave13 Mini".into(), plain));
    rt.invalidate();
    rt.frame(DT);

    // The seeded entries' first render is the bounded
    // settle the file's structural asserts use.
    for _ in 0..30 {
        if presented_keys(&rt).contains(&format!("library-thumb-{}", base + 1)) {
            break;
        }
        rt.frame(DT);
    }

    let keys = presented_keys(&rt);
    assert!(keys.contains("library-panel"), "the Library panel is mounted");
    assert!(
        keys.contains(&format!("library-thumb-0")),
        "the first built-in entry presents its thumbnail canvas"
    );
    assert!(
        keys.contains(&format!("library-row-{base}")),
        "the styled seed renders as a row"
    );
    assert!(
        keys.contains(&format!("library-thumb-{base}")),
        "the styled seed presents its thumbnail canvas"
    );
    assert!(
        keys.contains(&format!("library-thumb-{}", base + 1)),
        "the plain seed presents its thumbnail canvas"
    );
    let texts = element_texts(&rt);
    assert!(
        texts.iter().any(|t| t == "Wave13 Styled"),
        "the styled seed names its row: {texts:?}"
    );
    assert!(
        texts.iter().any(|t| t == "Wave13 Mini"),
        "the plain seed names its row: {texts:?}"
    );
}
