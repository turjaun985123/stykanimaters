# Phase L — the 3D preview, the scalable head ring, the last loop shape, plus depth

Phase L opened on Phase K's named carry-over and its
standing remainder list: the 3D ground grid that
could not land in Phase K because the 3D panel
rendered no figure at all, the head-ring radius that
Phase K centralised but left a constant, the
full-loop-in-one-tick shape the audio syncs could
not see, and the command palette's recents, which
died with the process. The grid's blocker is closed
the way Phase K §33 said it had to be — a preview
first, the grid drawn into it — with one design
decision taken and recorded below: the preview lives
in the 3D panel as its own canvas, not on the main
stage. Around those, the phase added a second pass
of panel depth.

> **A note on provenance.** This document was
> written by the phase's docs section under the rule
> Phase J established: every row below was checked
> against the tree before it was written, and rows
> whose work is not in the tree at writing time are
> marked **planned — not landed** rather than
> described as done. The integrator re-verifies at
> integration time, as before. Two rows carry that
> mark in this census (§31, §37), and one landed
> section's shell wiring is still owed (§14 — see
> §5). Section numbers are pinned by the files'
> own in-file numbering where a file states one
> (§§1, 3, 5, 12, 20, 21, 22, 23, 28, 30, 32, 35,
> 36, 39); the remaining numbers are this
> document's assignment of the brief's leftover
> slots to the verified work — every file:line
> anchor was grep-verified, but if the parent plan
> numbered those slots differently, only the
> numbers shift; the anchors stand.

## 1. What Phase L set out to close

From Phase K's remainder list (§7 of PHASE_K.md
and QA_RUNE §13):

- **The 3D ground grid (Phase K §33's stop)** —
  the 3D panel rendered no figure preview at all:
  no canvas, no Shape, no projection; its readouts
  were text. `fig3d::project` existed but fed only
  the editor's bake paths. Phase K's stop named the
  design decision owed: a grid on the main stage
  during a 3D session, or a new panel canvas.
  **Phase L took the panel-canvas branch**: §3
  added the two engine helpers a preview needs
  (`grid_line_points`, `preview_camera`), §1 the
  pure viewport geometry, and §2 the panel canvas
  itself, with drag-orbit and the grid toggle. The
  main-stage branch remains unbuilt — see §7.
- **Head-ring radius centralised but constant** —
  Phase K made `HEAD_RING_RADIUS` (17.0) the single
  source; the ring still could not be resized.
  Phase L makes it a per-project scale:
  `Project::head_ring_scale` (§4), one editor
  setter (§7), every Phase K consumer switched to
  the scaled radius (§§8–12), and an MCP tool
  (§13). At scale 1.0 every renderer draws exactly
  the ring it drew before.
- **The full-loop-in-one-tick miss** — a loop
  completed inside one poll tick, landing on the
  previous frame (or one past it), was
  indistinguishable from no movement, so the wrap
  and seek syncs never fired. §14 counts the
  playback walk's folds (`walk_wraps`) and restarts
  the sound when a tick consumed a whole loop —
  the op and its decision helper are landed and
  tested; the one-line poll wiring is the
  integrator's (§5).
- **Palette recents died with the process** — the
  recents list was shell memory only. §15 persists
  it in the layout preferences; §16 loads, merges
  and writes it back from the palette.

And the new depth: playback-speed-aware loop
jumps, loop-range steppers on the timeline, a
graph value readout, per-key interpolation
cycling, single-stroke simplify, a camera key
delete, FX gradient-end colour, text colouring, a
particle nudge pad, background presets, layer
joint counts, waveform boundary ticks for every
clip, Edit-menu selection deletes, two headless
scenes, and the gate widened again.

## 2. Groundwork

None this phase beyond premise greps — stated
plainly, in the Phase K manner: no shell State
field was added by the integrator before spawning
(the phase's one piece of new shell view state,
the 3D preview's grid flag, lives in §1's file as
a thread-local, the clips-selection pattern; the
palette recents persist through the layout
preferences, not through State). Every section
premise was grep-verified against the tree before
the sections were briefed, as in Phase K.

## 3. The 40 sections

| § | File | What landed |
|---|---|---|
| 1 | src/rune_sec/fig3_preview.rs | 3D preview geometry (file self-numbered §1): `preview_prims` (:137) — pure viewport-space segments for the panel canvas: ground grid first, then the ground shadow, then bones, then joint crosses (painter's order), all projected through the engine's own `fig3d::evaluate3` + `fig3d::project`; segments touching a behind-camera clamped endpoint are skipped, never drawn to the stage centre. Widths/colours are named constants (grid 1.0 `GRID_COLOR`, bones 2.5, shadow 3.0). The grid's visibility flag lives here as a thread-local (`grid_visible` :106 / `toggle_grid` :111) — shell view state, deliberately not part of `Fig3State` |
| 2 | src/rune_sec/three_d_panel.rs | The preview itself — Phase K §33's stop closed: a canvas keyed `panel-3d-preview` (`PREVIEW_KEY`, :571; `preview_shape` :646; `preview_el` :666) at the panel's top, drawing §1's prims over a recessed field. Dragging it orbits the session camera through the engine's `fig3_orbit`: `Drag` travel is since-the-press, so the handler converts through the pure `orbit_degrees` (:594, 0.4° per point, dragging up pitches up) with the last position in a thread-local. The Grid toggle (`d3-grid-toggle`, :723) renders through `ictl` with §30's `border-all` glyph, active = §1's flag. No new rotate row: `rotx-3d`/`roty-3d`/`rotz-3d` and `orbit-3d` already existed |
| 3 | src/fig3d.rs | The engine helpers the preview needed (named as §3 by §1's docs): `grid_line_points(extent, step)` (:183) — square grid segments on the y = 0 ground plane, `2 * (2 * extent + 1)` of them in deterministic order, degenerate inputs (extent ≤ 0, non-finite or ≤ 0 step) empty rather than unbounded; `preview_camera()` (:223) — the no-session camera framing the rest figure in a 308×220 viewport (yaw 25° / pitch 10° / distance 300, the exact-fill distance ×≈1.7 headroom, worked in the doc comment from `project`'s own maths) |
| 4 | src/model.rs | `Project::head_ring_scale: f32` (:741, serde default 1.0) + `Project::head_ring_radius()` (:806) = `HEAD_RING_RADIUS` × the scale, sanitised at read: a non-finite scale reads as 1.0 and the factor clamps to [0.25, 4.0] at read, so a hand-edited project file can neither vanish nor explode the ring. At 1.0 the radius is the Phase K constant, bit-identical; pinned by tests (:1038–:1072) |
| 5 | src/audio_ops.rs | Clip duration probes (file self-numbered §5): `clip_duration_secs` (:174) + `clip_duration_frames` (:190) — read-only, in the `clip_peaks` discipline (same loader path, no snapshot, no status), every failure (bad index, unreadable file, zero rate, zero frames) reading as `None`; frames are seconds × fps rounded half away from zero, the shell's convention |
| 6 | src/rune_sec/audio_panel.rs | The clip duration readout: `clip_readout` (Phase I's line) gains a ` · {d:.1}s` segment when §5's probe resolves for the effective target, and is byte-identical to the Phase I line when it does not (module docs :58, readout :251) — no placeholder, no fake duration |
| 7 | src/editor.rs | `set_head_ring_scale` (:1816) — the Phase K settings-setter discipline: non-finite is a status-only refusal ("Head ring size unchanged."), a real change takes exactly one snapshot. The setter clamps to [0.5, 2.0] — narrower than §4's read clamp, deliberately: the UI's range is a product choice, the read clamp is the file-format safety net; both are documented where they live |
| 8 | src/export.rs | The rasterizer reads `project.head_ring_radius()` (:302); a test pins scale 2.0 doubling the rendered ring (:1731) |
| 9 | src/svg.rs | The SVG export reads `project.head_ring_radius()` (:177) |
| 10 | src/stage_view.rs | The stage reads `core.project.head_ring_radius()` (:671) — the drawn ring now follows the project scale live |
| 11 | src/rune_sec/inspector_figure.rs | Head-ring steppers in the figure inspector: `step_head_ring_scale` (:717) over §7's setter, `fig-ring-dn` / `fig-ring-up` around a scale readout (:763–:782); Phase J's `figure_bbox` head growth reads the scaled radius too (:347), so Centre/Ground stay correct at any scale |
| 12 | src/rune_sec/figures_panel.rs | Thumbnails (file self-numbered §12): the fit maths takes the radius as a parameter (:270) so the ring — and the room the fit leaves for it — scales with the project (:316); pinned at scale 1.0 against the pre-Phase-L geometry (:1003, :1030) |
| 13 | src/mcp_tools.rs | MCP parity for the head ring: `set_head_ring_scale` tool (registry :3120, handler `tool_set_head_ring_scale` :1525, dispatching to §7's setter and reporting the stored scale). The same section's parity additions take the registry 84 → 87 (the settings family: the fps/stage setters gained their callers here in the census — gate §38) for a total of 95 → **98 tools** |
| 14 | src/audio_play.rs | The full-loop fix: `walk_wraps` (:472) counts how many whole folds the Phase K duration walk completes in a tick's elapsed time; `sync_playback_audio_on_full_loop` (:879) restarts the sound when the count is ≥ 1 and the playhead did not decrease — the decrease is the wrap sync's shape, and the pure decision (`full_loop_detected`, :615) requires equality to pass, since landing exactly on the previous frame after a full loop is precisely the missed shape. **The op is landed and tested; its poll call is not in main.rs at this census — integrator wiring, see §5** |
| 15 | src/shell.rs | Palette recents persistence, storage half: `LayoutPrefs::palette_recents: Vec<String>` (:275, serde default empty, so older prefs files load unchanged) + `palette_recents()` / `save_palette_recents()` (:333 ff.) — the save is a load-modify-save replacing only the recents and preserving every other field, the `prefs_with_workspace` pattern |
| 16 | src/rune_sec/command_palette.rs | Palette recents persistence, palette half: on open the working list is seeded by `merge_persisted(&shell::palette_recents())` (:1154) — persisted keys that still name real commands, in persisted order, ahead of the session's own; `record_recent` (:1181) keeps the working list and writes it back through `shell::save_palette_recents`. The command table is unchanged (103) |
| 17 | src/rune_sec/timeline_marks.rs | Loop-range steppers: a loop row (`timeline-loop-row`, builder `loop_row` :864) on the timeline's loop edge — with no range, one `loop-set` button plants the range at the playhead; with a range, four endpoint steppers (`loop-a-dn`/`loop-a-up`, `loop-b-dn`/`loop-b-up`) walk the ends a frame at a time over `set_loop_range`'s ordering/clamping, plus `loop-clear`. Dragging the edges (Phase F) still works; the steppers are the precise counterpart |
| 18 | src/rune_sec/playback_controls.rs | Loop jumps: `pb-loop-start` / `pb-loop-end` beside the loop toggle go to the range's start/end frames; targets come from the pure `loop_jump_targets` (:216) — `None` with no range, and the buttons follow the pb-loop discipline (shell state, no snapshot) |
| 19 | src/rune_sec/graph_panel.rs | Value readout: `graph-value` (`value_line`, :719) under the canvas states the drawn channel's real `Channel::value_at` at the playhead ("Value at frame {n}: {v:.1}"), resolved through `effective_channel_index` exactly as the canvas is — a picked twin reads its own value; the line exists only while a channel is drawn |
| 20 | src/rune_sec/library_panel.rs | Library sort (file self-numbered §20): `sort_rows` (:426) orders the rows by name or leaves the standing order, behind the panel's sort toggle (`toggle_sort`, :693); Enter-to-insert resolves through the sorted order (`enter_insert_index_sorted` :645 / `insert_on_enter_sorted` :667), so the row Enter inserts is the row the user sees first — the Phase J feature kept honest under the new order |
| 21 | src/draw_ops.rs | Single-stroke simplify, engine half (file self-numbered §21): `simplify_stroke_at(project, index, epsilon)` + `douglas_peucker_indices` — the Phase H Douglas-Peucker kept set exposed by index, so widths are carried by index, not resampled; a `widths` vector that is not exactly parallel is left exactly as found rather than normalised (index-exactness is the contract, stated in the docs) |
| 22 | src/rune_sec/stroke_inspector.rs | Selected-stroke simplify (file self-numbered §22): `stroke-simplify` (wand glyph, the Draw panel's own) under the width row, at `SIMPLIFY_EPSILON` 2.0 — the same tolerance as the frame-wide press, so the gesture is predictable. The press probes with §21's index form first: a stroke already simple at this epsilon reports "Stroke {n} is already simple." with **no snapshot**; otherwise exactly one snapshot precedes the op and the status reports the surviving count |
| 23 | src/rune_sec/text_tool.rs | Text colouring (file self-numbered §23): the loaded instance's `color` gets the colouring half the panel already had for size/rotation — swatch presses (`:697` handler) recolour the loaded text instance through one snapshot, mirroring `update_text_instance`'s refusal discipline |
| 24 | src/rune_sec/fx_clips.rs | FX gradient-end row: the detail block's colour block gains the gradient end (`fx-color2-row`, :1341) beside the primary — Solid clears `color2` to `None`, a swatch sets it through `EditorCore::fx_set_color2` (:695), so a clip's two-colour gradient is editable where the clip itself is edited |
| 25 | src/rune_sec/home_panel.rs | Home totals footer: `home-totals` (`totals_line`, :628) under the project list — "{n} projects · {f} figures · {frames} frames" summed over the *visible* rows' metadata, so a filtered list reports the filtered totals; inputs gathered at :1192 from the same keyed metadata the rows show |
| 26 | src/rune_sec/menus.rs | Edit ▸ Delete stroke / Delete particle: `edit-delete-stroke` / `edit-delete-particle` (:644/:653, trash-can glyphs :290/:291) run exactly the Delete key's selection-first discipline (the handlers at :473/:489) — the menu names what the keyboard already did, under the standing rule that a menu item exists only for a behaviour that exists |
| 27 | src/rune_sec/scenes.rs | Two new headless scenes (table :345/:346): `three-d` (`scene_three_d`, :287) — the 3D workspace with a live fig3 session, so a `--tree` dump proves `panel-3d-preview` exists; `loop-range` (`scene_loop_range`, :314) — the Animate workspace with a loop range set, proving the loop row renders its stepper state. Scene count 8 → 10 |
| 28 | src/rune_sec/help_panel.rs | Help refresh (file self-numbered §28): 42 → **48 rows** (Features 18 → 24) — six new rows (3D preview, Head ring size, Loop range steppers, Stroke simplify, Palette recents persist, Camera key delete), each verified against the tree. The file's own census note is worth keeping verbatim in substance: three of the six landed *during* the section's census — the first greps found no `stroke-simplify`, no `cam-delete-key` and no `palette_recents` call, and only re-verification found all three present. No candidate was dropped this phase |
| 29 | src/graph_keys.rs | Per-key interpolation, library half: `set_key_interp` sets one key's `Interp` by (channel, key) index — any channel, any key, where the core's own `set_graph_key_interp` reaches only the playhead-nearest key on the selected figure's angle channel; `key_interp_at` is its read-only half, resolved with the shell's key-at-frame convention. House discipline: refusal is status-only with no snapshot, success snapshots exactly once |
| 30 | src/rune_sec/icon_data.rs | One glyph: `border-all` (448.0) added to the curated table, generated byte-exact from the downloaded Font Awesome SVG like the other 110 — the grid glyph §2's toggle needed. Table 110 → **111 icons**; the name/set pins updated in step |
| 31 | src/rune_sec/status_strip.rs | Playback-speed segment (landed after this document's census; row rewritten by the integrator after verifying the section in the tree): `speed_segment(speed)` returns `None` at exactly 1.0× (and for non-finite speeds) and `"{x}×"` otherwise, trimmed the way `speed_label` trims; `status_strip_state` composes it after the play segment only off 1.0×, so the default strip — and Phase K's pinned floor arithmetic — is byte-identical. At other speeds the segment's own floor (16–32 pt) can push the fixed sum past the old totals and the tail truncates, the strip's designed behaviour |
| 32 | src/rune_sec/keyframe_tools.rs | Interp cycling (file self-numbered §32): `key-interp` (bezier-curve glyph) in its own row walks the selected joint's current key through Hermite → Linear → Step → Hermite (`next_interp`, label "Interp: {current}" via `interp_button_label` :353), over §29's `set_key_interp`, the current mode read back through `key_interp_at` (`current_key_interp` :333); live/inert forms under the one key on the Phase I pattern, the guard being `interp_cycle_enabled` |
| 33 | src/rune_sec/particles_panel.rs | Particle nudge pad: `particles-nudge-l`/`r`/`u`/`d` move the selected event by `NUDGE_STEP` 4.0 (defined locally, :483 — the text tool's step, its own constant) through `particle_set_position`, reading the event's current x/y first; one snapshot per press via the op, the panel's standing discipline. Until this phase the panel deliberately offered no position editing at all |
| 34 | src/rune_sec/scene_panel.rs | Background presets: `BG_PRESETS` (:634) — six named colours in one row (`scene-bg-presets`, chips `scene-bg-preset-{i}`, positional keys, never name-based), a press sets the stage background through the panel's existing discipline and the active chip is the preset whose colour matches (`bg_preset_active`) |
| 35 | src/rune_sec/layers_panel.rs | Layer joint counts (file self-numbered §35): a muted trailing readout on each layer row — `joint_label` (:248) names the figure's joint count, so the list answers "which of these is the big rig?" without opening anything |
| 36 | src/rune_sec/waveform_view.rs | All-clip boundary ticks (file self-numbered §36): `all_clip_edges` + `with_all_clip_ticks` (:721) draw a faint tick at every clip's start and end across the waveform in project mode — the selection accent at reduced alpha — so the strip shows the whole project's clip layout, not only the selected clip's Phase K bracket |
| 37 | src/rune_sec/snap_tests.rs | Runtime round 9 (landed after this document's census; row rewritten by the integrator after verifying the round in the tree): four tests (31 → 35) — `pb-speed` cycles the shell speed to 1.5×, `fig-ring-up` raises the project's head-ring scale to 1.1, the timeline loop row sets then clears the loop range, and `ws-3d` presents `panel-3d-preview`. All four targets were verified present at the round's own census, so no substitutions were needed |
| 38 | scripts/phase_j_checks.py | The gate, widened in place (it is now the "Phase J+K+L" gate): the caller census gains the Phase L ops — `set_head_ring_scale` (callers: mcp_tools.rs, inspector_figure.rs), `clip_duration_secs` (audio_panel.rs), `simplify_stroke_at` (stroke_inspector.rs), `set_key_interp` (keyframe_tools.rs) — plus one panel-local entry, §39's `delete_camera_key_at`, watched through a new same-file-caller mechanism the script documents (the older `clear_key_at` in the same file is the non-undoable clear and is deliberately not the entry). The script's Phase L paragraph was written mid-phase while sections were still landing; its final census output reports all 21 watched ops wired. Count claims stay read-from-source: palette 103, MCP registry 87, icons 111, scenes 10. **Gate run: PASS on all five checks** (see §6) |
| 39 | src/rune_sec/camera_panel.rs | Camera key delete (file self-numbered §39): `cam-delete-key` (trash-can) deletes the camera key at the current frame — `delete_camera_key_at` (:478) is the undoable delete over the panel's older `clear_key_at` (:262); a frame with no key is a refusal, not a snapshot |
| 40 | docs/ | This document + QA_RUNE §14 |

## 4. Cross-section contracts

- **The 3D preview (§§1/2/3/27/30).** Three files
  own three layers and none owns the feature
  alone: §3 owns the world maths' new helpers (the
  grid's shape, the default camera), §1 owns which
  segments exist in viewport space and the grid
  flag, §2 owns the canvas, the orbit gesture and
  the toggle. §30 supplies the toggle's glyph;
  §27's `three-d` scene plants the session state
  the preview needs, so a headless tree dump can
  prove the canvas exists. The projection itself
  is not new: `fig3d::evaluate3` + `fig3d::project`
  are the bake paths' own pair, so the preview
  shows the figure the baker would bake.
- **The head ring (§§4/7/8/9/10/11/12/13).** One
  field, one setter, six readers: §4 owns the
  value and its read-time sanitation; §7 owns the
  write discipline (one snapshot, UI clamp
  [0.5, 2.0]); §§8–12 read only through
  `head_ring_radius()`, so no consumer can observe
  an unsanitised scale; §13 exposes the setter.
  The gate's caller census reports exactly two
  production callers of the setter — the inspector
  steppers and the MCP tool. Phase K's neighbour
  rule still holds: stage_fx's `GIZMO_RING_R` is a
  particle-anchor ring, not a head, and is not
  scaled.
- **The full loop (§14, wiring §5).** §14's count
  reads the same clock and the same duration walk
  as Phase K's drift sync, so the three restart
  syncs (wrap, seek/drift, full-loop) all reason
  about one prediction. The decision helper's
  no-decrease guard is the contract that keeps the
  syncs from double-restarting on one tick.
- **Palette recents (§§15/16).** §15 owns the
  storage (one field, load-modify-save); §16 owns
  the merge rule (persisted order wins, dead keys
  dropped) and the write-back. Neither half
  persists alone: the palette never touches the
  prefs file's other fields, the shell never
  interprets a command key.
- **Interp (§§29/32).** §29 owns the op and its
  snapshot discipline; §32 is its only consumer,
  resolving the current key the way the panel
  already resolved its channel. The gate watches
  the pair (`set_key_interp` caller:
  keyframe_tools.rs:771).
- **Simplify (§§21/22).** §21 owns the kept set;
  §22 owns the press discipline (probe first, no
  snapshot for a no-op) — the probe and the op run
  the same algorithm, so the probe's answer is the
  op's answer.
- **Counts are pinned in the files that own
  them**: palette 103 (§16 changed no commands),
  MCP registry 87 (§13's additions), help 48
  rows (§28's test), scenes 10 (§27's test), icons
  111 (§30's pins). A count in these documents can
  be checked against its pin in seconds.

## 5. Integrator wiring (owed at integration, found by census)

- **`sync_playback_audio_on_full_loop` — WIRED by
  the integrator.** §14's op landed with its tests
  (audio_play.rs) but no caller; the integrator
  added the poll line in main.rs beside the wrap
  and seek syncs, called with the tick's previous
  frame, so the full-loop shape is now detected at
  runtime. The gate does not watch this op (it is
  an `EditorCore` method, not a free function),
  which is why this item was a census finding, not
  a gate failure.
- **`pub mod fig3_preview;`** declared in
  rune_sec/mod.rs by the integrator (§1's module
  existed on disk undeclared; §2's panel imports
  it).
- **Project literals** — §4's new field broke
  every exhaustive `Project { … }` literal outside
  model.rs; the integrator added
  `head_ring_scale: 1.0,` to all eight (pivot.rs
  ×2, nodes.rs, nodes_anim.rs, mcp_tools.rs,
  rune_sec/project_tools.rs, and the media.rs /
  export.rs test literals), plus the one full
  `LayoutPrefs` literal in workspaces.rs (§16's
  flag) and the icons test's 110 → 111 count pin
  (§30's flag).
- **Preview ground offset** — the integrator
  moved §1's grid and shadow onto the figure's
  lowest joint (see §7); §3's `grid_line_points`
  doc no longer claims project() defines a ground
  plane.
- Everything else the census watches is wired,
  including the entries landed during other
  sections' censuses (§28's note): `set_key_interp`
  (keyframe_tools.rs), `delete_camera_key_at`
  (camera_panel.rs, the gate's same-file entry),
  and the palette's `palette_recents` round-trip
  (§16).

## 6. Inventory snapshot

**INTEGRATOR (final)** — post-wiring gate run
(`python3 scripts/phase_j_checks.py 16cade9`):
**PASS on all five checks.** Inventory: **704
unique keys (605 static + 99 dynamic)**, key-set
diff against the Phase K tree (674 keys) clean —
**zero keys lost**. Caller census: all 21 watched
ops wired (7 J + 9 K + 4 L + 1 same-file). Counts:
palette **103** commands, MCP registry **87**
(total **98** tools), curated icons **111**,
headless scenes **10**.

## 7. Honest remainders after Phase L

- Sound on a real screen is still the user's to
  confirm: the headless snap covers pixels, never
  audio. Phase L's full-loop fix is, at this
  document's census, an op awaiting its poll call
  (§5) — and even wired, the correction remains a
  bounded restart, not a sample-accurate clock.
- The 3D preview is **panel-canvas only**: the
  figure is drawn inside the 3D panel's viewport,
  with its grid, shadow and orbit. Drawing the 3D
  figure on the **main stage** — Phase K §33's
  other branch — remains unbuilt, and nothing in
  this phase claims it.
- Two "New project" results still disagree (panel
  blank vs Home demo) — the user's product call,
  untouched again.
- `core.onion_frames` / `cycle_onion_frames`
  remain orphaned engine API, by the Phase G
  design (the shell's asymmetric onion depths
  cannot be expressed by the engine's single
  symmetric count).
- The status strip's Audio segment still has no
  glyph — Phase K's measured refusal stands
  (753.75 pt of floors against the strip's
  744 pt). §31's speed segment fits the same
  arithmetic by existing only off 1.0×.
- MCP (98 tools) and the command palette (103
  commands) are different surfaces; the counts are
  close but the sets are not identical, and parity
  between them is noted, not closed.
- Two integrator corrections stand on the record:
  the preview grid follows the figure's feet (the
  engine defines no ground plane — §1's y = 0 grid
  would have sliced through a session figure's
  hips; fig3_preview offsets the grid and the
  shadow to the lowest evaluated joint), and
  §27's two headless scenes were written against
  census substitutions that the integrator
  upgraded to the real keys (`panel-3d-preview`,
  `timeline-loop-row`) once both had landed.
