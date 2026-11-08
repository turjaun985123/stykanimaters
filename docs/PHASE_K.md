# Phase K — the Phase J remainders, audited honestly, plus depth

Phase K opened on Phase J's remainder list: the drift
prediction that ignored frame durations, the graph
channels that could be duplicated but never deleted,
the project files that could not be deleted at all,
the head-ring radius copied in five places, the
palette's search, and the status strip's glyphless
Audio segment. Two of those six turned out to be
different problems than their remainder text claimed —
the palette was never substring-only, and the glyph
still does not fit at the new width — and this document
records both outcomes as found, not as briefed. Around
the remainders, the phase added playback speed, a
project delete, and a second pass of panel depth.

> **A note on provenance.** This document was written
> by the phase's docs section under the rule Phase J
> established after Phase I's fabricated-docs failure:
> every row below was checked against the tree before
> it was written, and rows whose work is not in the
> tree at writing time are marked **planned — not
> landed** rather than described as done. The
> integrator re-verifies at integration time, as
> before. One row carried that mark in this census
> (§33); §37 also carried it at first writing and was
> corrected by the integrator after verifying the
> round in the tree.

## 1. What Phase K set out to close

From Phase J's remainder list (§7 of PHASE_J.md and
QA_RUNE §12):

- **Duration-blind drift prediction** — Phase J's
  drift sync compared the playhead against a naive
  prediction that charged every frame at the project
  rate, so a long frame with a duration multiplier
  could trip the 2.5-frame tolerance on its own.
- **No channel delete** — graph channels could be
  created, duplicated (Phase J) and edited, but never
  removed; a mistaken channel was permanent.
- **No project delete** — the Home panel could open,
  list and measure project files, but not delete one.
- **Head-ring constant in five copies** — the 17.0
  head-ring radius was a literal in the rasterizer,
  the SVG export, the stage, the figure thumbnails
  and the figure inspector's bounds maths.
- **Palette search** — recorded as substring-only.
  Section 17's audit found the premise false (see §3):
  the palette has matched by case-insensitive
  subsequence, with quality scoring, since Phase D.
  There was nothing to fix; the section pinned the
  facts instead.
- **The Audio segment glyph** — refused in Phase H on
  width maths from the squeezed ~461 pt layout.
  Phase J's layout fixed the widths, so §16 re-ran
  the refusal at the strip's real 744 pt. It still
  does not fit (see §3) — the refusal now stands on
  current numbers, pinned by a test.

And the new depth: playback speed (transport, menu,
and the audio prediction all reading one factor),
Ctrl+S quick-save, right-press selection on the
stage, FX and particle playhead buttons, stroke
width steppers, gradient steppers, a ruler drag,
the waveform span bracket, reset layout, project
reload, and a 3D ground grid (§33 — not landed).

## 2. Groundwork (integrator, before spawning)

Phase K is the first phase since Phase I to add a
shell State field, so the Phase I §23 trap was closed
by construction rather than by luck: the integrator
added `State.playback_speed: f32` (src/main.rs:117,
initialised `1.0` at main.rs:190) *before* the
sections were spawned, and patched every full
`State {…}` literal in the tree — the small diffs in
clips_panel.rs (×4), camera_panel.rs and
audio_panel.rs are exactly those literal patches,
nothing else. The §38 gate's State-literal census
verifies the result mechanically: **43 fields, 20
full literals, `playback_speed` present in every
one** (gate run at this document's census).

## 3. The 40 sections

| § | File | What landed |
|---|---|---|
| 1 | src/audio_play.rs | Duration-aware drift prediction: `expected_frame_walk` (:354) walks the timeline the way `advance_play` moves — each frame costs its real `duration / fps` (duration floored at 0.05, as the engine floors it), scaled by the noted speed, folding at the loop-range or timeline end; `sync_playback_audio_on_drift` (:653) now compares against the walk. The naive `expected_frame` (:311) stays as the walk's all-durations-1 case. Two approximations remain, documented in the module docs: boundary slicing vs per-tick charging, and the walk's crossing cap (2 × frame count + 2), after which the prediction saturates and the drift reads as exceeded — the safe answer |
| 2 | src/main.rs | Shell wiring: `poll` notes the speed to the audio side every playing tick (`note_playback_speed`, :353) *before* advancing the playhead by the speed-scaled step (`advance_play(dt * s.playback_speed)`, :358); Ctrl+S quick-save in `on_key` (:451) — exactly the app-bar Save press (`core.save()`), no second save path; `view` passes `fx_selected` into `StageOverlays` (:648) for §9's gizmo |
| 3 | src/graph_keys.rs | Channel removal, library side: `delete_channel(core, channel_index) -> bool` (:268) — one snapshot, status names the channel's number, target label and key count; `keys_in_range` (:293, pure) + `delete_keys_in_range` (:324) — range clearing over the (figure, target) channel, first-match resolution as the core's own graph ops use |
| 4 | src/rune_sec/graph_panel.rs | Delete channel in the panel: `graph-chan-delete` button beside Phase J's duplicate button, shown while a channel is drawn; `delete_drawn_channel` (:574) resolves through `effective_channel_index` — a picked duplicate row deletes THAT twin — and repairs the stored pick via `selection_after_delete` (:551: before → unmoved, on → cleared to target resolution, after → shifted down one) |
| 5 | src/mcp_tools.rs | MCP parity for the channel pair + the FX kind setter (header :1417): `duplicate_graph_channel` (:1438), `delete_graph_channel` (:1459), `fx_set_kind` (:1473) over the file's one kind parser; shared `check_graph_channel` pre-check (:1430). Registry 81 → 84, total 92 → 95 |
| 6 | src/model.rs | `pub const HEAD_RING_RADIUS: f32 = 17.0` (:22) — the single source for the head-ring radius, pinned by a test (:1013). The value did not change; its copies did |
| 7 | src/export.rs | The CPU rasterizer reads the constant (:300, imported :7); its local 17.0 literal is gone |
| 8 | src/svg.rs | The SVG export reads the constant (:177, imported :15) |
| 9 | src/rune_sec/stage_fx.rs | FX clip gizmo geometry: `fx_clip_gizmo_geometry(core, clip_index)` (:412) — the selected clip's anchor marker prims, sibling to the particle-event gizmo; empty for a bad index or an unanchored clip. Drawn by stage_view (§34) |
| 10 | src/rune_sec/figures_panel.rs | Thumbnails re-export the constant under the panel's standing name (`pub use … HEAD_RING_RADIUS as HEAD_RING_R`, :214); the thumbnail bounds growth (:274) and ring drawing (:316) read it |
| 11 | src/rune_sec/inspector_figure.rs | The inspector re-exports the constant the same way (:300); Phase J's `figure_bbox` head growth (:329) reads it |
| 12 | src/editor.rs | Direct setters for the settings steppers: `set_project_fps` (:1761) and `set_stage_size` (:1782) — a discrete settings choice is an undoable edit (exactly one snapshot on a real change), where the steppers above them were not; trim-friendly status formatting shared by both |
| 13 | src/rune_sec/scene_panel.rs | Frame-rate + fine stage steppers over §12's setters: the rate stepper press (:532, op call :537), the finer stage-axis step (:546, press :562, op call :572), rows at :677/:702; the stage readout gains its rate suffix (:489) |
| 14 | src/rune_sec/home_panel.rs | Project delete, panel side — two-step by design: a row's Delete arms the confirmation (`arm_delete`, :670; the armed path lives in `HomePanelState::confirm_delete`, :225) and the confirm press dispatches (`dispatch_delete_confirm`, :703) to §15's op at :711; caches are invalidated on success so a deleted file never renders as a stale row |
| 15 | src/shell.rs | `delete_project_file(path) -> io::Result<()>` (:182) — refuses every shell-owned name (`.layout.json`, `.recovery.json`, `recents.json` — the Phase F §7 filter family), a missing path is `NotFound`, never a panic; tests at :601/:612/:626 |
| 16 | src/rune_sec/status_strip.rs | The Audio glyph refusal, **recomputed and standing**: at the strip's real 744 pt the default state's floors total 512 pt + the audio segment's 64 + the `volume-high` glyph's painted 17.75 = **593.75 pt**, and the tail absorber's 160 pt minimum makes 753.75 > 744 — short by 9.75 pt (without the glyph: 576 + 160 = 736 ≤ 744, 8 pt spare). Phase H's per-segment budget independently agrees. Nothing in the rendering changed; the arithmetic is pinned by `audio_glyph_width_recomputation_at_744pt` (:1343) so any future label, icon-size or width change re-opens the question loudly. Full working in the module docs (:238) |
| 17 | src/rune_sec/command_palette.rs | **Audit outcome: no behaviour changed.** The briefed "substring tier, then subsequence fallback" premise does not match this tree — `fuzzy_score` has matched by case-insensitive subsequence with quality scoring since Phase D, over title, key AND group; the tiering would have dropped the group field from the fuzzy tier and silently broken queries that work today ("vew" reaches the View toggles through their group alone). The section pinned the two facts instead: `exprt_finds_go_export_by_subsequence`, `group_field_matches_by_subsequence_too`. Module docs at :103 |
| 18 | src/rune_sec/fx_clips.rs | Playhead row beside the split row: `fx-to-playhead` / `fx-end-playhead` (:1313/:1316) — To playhead lands the clip's start on the playhead frame (`move_delta` over `fx_move_clip`, handler :827), End at playhead resizes so the playhead is its last covered frame (`end_delta` over `fx_resize_clip`, :847); both edit in place, so the selection index never moves |
| 19 | src/rune_sec/particles_panel.rs | Two detail-block actions: `particles-to-playhead` (:514) — the selected event jumps to `core.current` via `particle_set_frame`; `particles-anchor-node` (:523) — anchor to the selected figure AND joint via `particle_set_anchor` + `anchor_target`, rendered only while a joint is selected (figure-only anchoring stays the existing button's job) |
| 20 | src/rune_sec/timeline_marks.rs | Ruler drag: the Phase J ruler row now seeks — press and drag call `core.goto` on the frame under the pointer (`ruler_frame_at_x`, :446, the chips' own slot pitch), handler on the ruler row itself (:640), so the gesture can never be mistaken for the chips' range drag; it writes nothing but the playhead and never touches the play state |
| 21 | src/rune_sec/waveform_view.rs | Span bracket (section self-numbered in-file): `span_bracket` (:352) computes the bracket over the selected clip's span — a rule at `SPAN_BRACKET_Y` across the span's x edges with a tick at each edge — drawn in project coordinates in both modes (in clip mode it brackets the very span the seek maps onto) |
| 22 | src/rune_sec/fx_browser.rs | Per-kind usage counts on the kind buttons: a kind the project already holds reads "{label} · {n}" (`kind_button_label` over `kind_usage`); at zero usage the label is byte-identical to before. Display only — the filter still matches the base name/label/category, and the Recent row keeps bare labels |
| 23 | src/rune_sec/playback_controls.rs | The speed control: `pb-speed` cycles `State::playback_speed` through `SPEEDS` [0.25, 0.5, 1.0, 1.5, 2.0] (:249) via `next_speed` (:259) / the press at :283; the label IS the current speed (`speed_label`). Shell-level state, like the loop toggle — no snapshot, no core op |
| 24 | src/rune_sec/stroke_inspector.rs | Selected-stroke width steppers (section self-numbered in-file): row `stroke-width-row` under the measure line, rendered only with a valid selection; `width_after` (:743) steps by `STROKE_WIDTH_STEP` 0.5 (:714), clamped [0.5, 40.0]; the write is the Phase F bulk composition (one snapshot + direct `project.strokes` write), and a non-empty `widths` taper scales by the same ratio so the stroke keeps its shape |
| 25 | src/rune_sec/draw_tools.rs | Brush width readout: `draw-width-readout` shows the active preset's base width (`active_brush_width`, :233; `width_label`, :247; `width_readout`, :260). A readout, not a stepper, for Phase J §35's reason: width lives on the `paint::Brush` preset (`paint.rs:430`), which the core hands out as a copy — a stepper would write to a throwaway |
| 26 | src/rune_sec/inspector_segment.rs | Joint navigation: `select_parent` (:562) / `select_first_child` (:577) walk the selection along the rig from the inspector (buttons at :733); a press with nowhere to go is a silent no-op, matching the panel's discipline |
| 27 | src/rune_sec/project_tools.rs | Reload from disk: `proj-reload` re-opens the current project's saved file through the open rows' exact discipline (the §27 gate resolves the target as an owned path; `clear_stale_solo`, `sync_name_edit`), discarding unsaved edits — and the success status says so outright ("Reloaded {name} from disk…"). No saved file → "No saved file to reload.", open never called. Audit-map row at :42, note at :50 |
| 28 | src/rune_sec/menus.rs | Menu surface for the phase's shell features, under the Phase F §9 discipline (a hint exists only for a binding that exists): File ▸ Save gains its Ctrl+S hint now that §2 landed the binding, and View ▸ Playback speed cycles the same factor as §23's button through `cycle_playback_speed` |
| 29 | src/rune_sec/inspector_style.rs | Gradient RGB steppers: one stepper row per channel (:597, built :869) stepping the gradient end colour through `set_node_color2` (:335) — the exact twin of the primary-colour steppers; a stepped end colour is recorded into Phase J's recent-colours list from the stepper press (:96) |
| 30 | src/rune_sec/text_tool.rs | Rotate the loaded instance (section self-numbered in-file): the rotate steppers (:851) turn every transform key's angle of the loaded text (`rotate_loaded`, :493; handler :622) — the one placement dimension the Phase G/J nudge + resize surface could not reach |
| 31 | src/rune_sec/keyframe_tools.rs | Clear keys in range: `key-clear-range` (eraser glyph, :155) clears the selected joint's channel keys inside the loop range (or the whole timeline) via `clear_keys_in_range_core` (:639) over §3's `delete_keys_in_range`; the refusal set is the nudge family's, and a range holding no keys is a status, not a snapshot |
| 32 | src/rune_sec/settings_popover.rs | Reset layout (section self-numbered in-file): `reset_layout` (:375) drives the layout to its defaults in one press — the Animate workspace through `select_workspace` (so the reset is also the persisted startup default), both side panels shown, all three guide flags set ON outright (never flipped through the toggles). Scope is layout only, by design: not the theme (persisted outside State), not playback, not project data |
| 33 | — | **Planned — not landed at this document's census** (3D ground grid). No file carries it: `three_d_panel.rs` and `fig3d.rs` are unmodified in this phase's diff, and no ground/floor grid code exists anywhere in the tree (the only "grid" in src is the contact sheet's). The integrator rewrites this row if it lands before integration; otherwise it is the phase's named carry-over |
| 34 | src/stage_view.rs | Stage completion of three contracts: the head ring reads `model::HEAD_RING_RADIUS` (:669, §6's family); the FX gizmo prims from §9 are drawn in the last overlay slot (:715, call :723) for `overlays.fx_selected`; right-press selection (:730) — a secondary press selects what it lands on through the same hit paths as a primary press, without starting a drag |
| 35 | src/audio_ops.rs | `audio_duplicate_clip_at(&mut self, index) -> Option<usize>` (:464) — the Phase H indexed family's missing duplicate: clone inserted right after the chosen clip, one snapshot, sibling statuses ("Audio clip {n} duplicated as clip {n+1}."); an audio clip carries no seed, so the clone is exact. **No caller yet — see §5** |
| 36 | src/rune_sec/scenes.rs | Two new headless scenes (table :271/:272): `graph-duplicates` (`scene_graph_duplicates`, :177) — two RootX channels planted for figure 0 so a `--tree` dump proves both rows list; `fx-split-mid` (`scene_fx_split_mid`, :223) — an FX clip added through `add_fx_clip` and selected, the state §9's gizmo draws for. Scene count 6 → 8 (`scene_table_has_eight_uniquely_named_scenes`, :370). One named discipline exception, documented in-file: the duplicates scene writes the pub `project.channels` field directly, because no core op creates a channel with chosen keys |
| 37 | src/rune_sec/snap_tests.rs | Runtime round 8 (landed after this document's first census; the integrator verified it in the tree): four tests (27 → 31) — the Phase J column split pinned by the stage's presented width (744 ± 1), fig-centre moving the figure's bbox centre onto the stage centre, pose-mirror-all negating every angle with roots byte-identical, and the FX split driven through the real panel (browser add → lane select → Split at playhead) |
| 38 | scripts/phase_j_checks.py | The gate, widened in place (it is now the "Phase J+K" gate): the caller census gains `PHASE_K_OPS` — §3's pair, §12's setters, §15's delete, §35's duplicate, §9's gizmo geometry, §1/§2's `note_playback_speed`, and one Phase J op the Phase J list never watched, `duplicate_channel_to_figure`. Count claims stay read-from-source: palette 103, MCP registry 84, icons 110, scenes 8. The State census covers `playback_speed` automatically (the field list is parsed live from main.rs). Census run for this document: checks 1/2/3/5 PASS, check 4 fails on exactly one op — §35's, the §5 item below |
| 39 | src/rune_sec/help_panel.rs | 34 → 42 rows (the file's count test pins 42): six new Features rows for the phase's landed features, each verified against the tree as §29 of Phase J verified its own; two more (Right-press selects, Reload from disk) were restored at the close-out — §39's census ran before §27's button and the integrator's right-press registration landed, so both were dropped mid-phase as not user-reachable; both verifiably are |
| 40 | docs/ | This document + QA_RUNE §13 |

## 4. Cross-section contracts

- **Speed + drift (§§1/2).** One factor, three
  readers: §23/§28 set `State::playback_speed`; §2's
  poll notes it to the audio side
  (`note_playback_speed`, audio_play.rs:536) *before*
  advancing, and advances the playhead by the scaled
  step; §1's `expected_frame_walk` consumes the noted
  speed, so the drift prediction runs on the same
  clock as the picture. Note the gate's docstring
  still describes `note_playback_speed` as
  expected-unwired — that was true mid-phase; at this
  census the gate itself reports its caller
  (main.rs:353). The wiring is done; the script's
  comment is stale, not the tree.
- **Channel ops (§§3/4/5/31).** §3 owns the ops and
  their snapshot discipline; §4 (panel), §5 (MCP) and
  §31 (range clear) are its only consumers, each
  resolving the channel the same way its own surface
  already did — the panel by effective index (twins
  stay distinct), the MCP tools and the range clear
  by first match, the engine's own convention.
- **The constant (§§6/7/8/9/10/11).** One definition
  (model.rs:22), five consumers, zero value change:
  every renderer draws the same 17.0 ring it drew
  before, now from one source. One neighbour is
  deliberately NOT in the family: stage_fx's
  `GIZMO_RING_R` (stage_fx.rs:247, also 17.0) rings a
  particle anchor point, not a head — a different
  ring that happens to share the value, left as its
  own named constant.
- **The FX gizmo overlay (§§9/2/36).** §9 computes
  the prims from the project alone; §2's `view`
  supplies the selection (`overlays.fx_selected`,
  main.rs:648) and §34 draws; §36's `fx-split-mid`
  scene plants exactly the selected-clip state the
  overlay needs, so a headless snap can prove it
  renders. No section holds the whole feature —
  geometry, selection and proof are three files by
  design.
- **fps / stage ops (§§12/13).** §12's setters own
  the snapshot discipline (a settings choice is an
  edit); §13's rows are pure dispatch over them —
  the gate's caller census reports exactly one
  production caller per setter, both in scene_panel
  (:537/:572).
- **Project delete (§§14/15).** §15 owns the refusal
  (shell-owned names, missing files); §14 owns the
  two-step interaction (arm, then confirm) and the
  cache invalidation. Neither half deletes alone:
  the panel never touches the filesystem, the lib
  never asks the user anything.
- **Speed control (§§23/28).** The transport button
  and the View menu item call the same
  `cycle_playback_speed`; the menu's Ctrl+S hint
  rides along under the standing rule that a hint
  exists only once its binding does (§2).
- **Counts are pinned in the files that own them**:
  palette 103 (§17 changed no commands), MCP
  registry 84 (§5's test), help 40 rows (§39's
  test), scenes 8 (§36's test). A count in these
  documents can be checked against its pin in
  seconds.

## 5. Integrator wiring (owed at integration, found by census)

- **`audio_duplicate_clip_at` — WIRED by the
  integrator (this document's census predates it).**
  §35's op landed with its tests (audio_ops.rs:464);
  the integrator added the Duplicate button
  (`audio-duplicate`, copy glyph) to the audio
  panel's operations row, dispatching through the
  panel's `effective_target` discipline like its
  Phase G/H siblings and selecting the copy on
  success (audio_panel.rs `duplicate_target`). The
  gate (§38) was re-run after the wiring: its caller
  census now reads the op WIRED and the whole gate
  PASSes (see §6).
- **Right-press selection — REGISTERED by the
  integrator.** §9's `secondary_select` helper
  (stage_view.rs) landed tested but, as §39's report
  flagged, no element called it: the stage canvas in
  main.rs now registers `on_secondary_press_local`,
  converting the point through `RotTransform` built
  from the stage size `view` last laid out (handed
  over via the `STAGE_EL_SIZE` thread-local, since a
  press handler receives neither the element size
  nor a `Cx`). Everything else the census watches is
  wired, including the two entries the gate's
  docstring expected to flag (`note_playback_speed`
  landed with §2; `duplicate_channel_to_figure` now
  has two callers — graph_panel.rs:541 and
  mcp_tools.rs:1444).

## 6. Inventory snapshot

**INTEGRATOR (final)** — post-wiring gate run
(`python3 scripts/phase_j_checks.py 36fff5e`):
**PASS on all five checks.** Inventory: **674
unique keys (577 static + 97 dynamic)**, key-set
diff against the Phase J tree (641 keys) clean —
**zero keys lost**. Caller census: every watched
op wired, including `audio_duplicate_clip_at`
(audio_panel.rs) after §5's wiring. Counts:
palette **103** commands, MCP registry **84**
(total **95** tools), curated icons **110**,
headless scenes **8**. (The docs census's own
pre-wiring run read 673 keys; the +1 is the
integrator's `audio-duplicate` button.)

## 7. Honest remainders after Phase K

- Sound on a real screen is still the user's to
  confirm: the headless snap covers pixels, never
  audio. Phase K removed the drift prediction's
  known false-positive (duration blindness); the
  correction is still a bounded restart inside the
  2.5-frame tolerance, not a sample-accurate clock.
- A full loop inside one poll tick, landing exactly
  on the previous frame (or one past it), is still
  missed by the wrap and seek detection — the
  standing edge case from Phase I, unchanged: such
  a tick is indistinguishable from no movement at
  all.
- Two "New project" results still disagree (panel
  blank vs Home demo) — the user's product call,
  untouched again.
- `core.onion_frames` / `cycle_onion_frames` remain
  orphaned engine API, by the Phase G design (the
  shell's asymmetric onion depths cannot be
  expressed by the engine's single symmetric
  count).
- MCP (95 tools) and the command palette (103
  commands) are different surfaces; the counts are
  close but the sets are not identical, and parity
  between them is noted, not closed.
- The Audio segment still has no glyph — now a
  *measured* refusal at the current layout, not a
  stale one (§16): it re-opens by itself the day a
  default label shortens, the icon shrinks, or the
  strip widens, because the arithmetic is pinned
  by a test.
- §33 (3D ground grid) had not landed at this
  document's census — named here so it is a
  carry-over on the record, not a silent gap.
  (§37's Runtime round landed after the census and
  is recorded in its row above.)
