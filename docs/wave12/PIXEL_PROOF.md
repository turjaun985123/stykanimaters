# Wave 12 — Pixel-proof coverage ledger

**What this is.** The entering-state ledger for the headless proof
system: every registered snap scene, the state it seeds, and which
proof kinds are actually applied to it — plus the two Wave 12
additions in flight and the honest gaps that remain. The integrator
reconciles this ledger at Wave 12 close-out; the entering state
below is exact as written.

**Entering state.** HEAD `3f04667` (Wave 11 close-out commit;
`docs/REDESIGN_PROGRESS.md` §6.6). Scene census at source:
**24 scenes** registered in `scenes()` in
`src/rune_sec/scenes.rs`, pinned by that module's own test
`scene_table_has_twentyfour_uniquely_named_scenes`. Proof is
produced by the manual-dispatch workflow
`.github/workflows/headless.yml` ("Headless Rune probe
(snap / tree)") and read at the headless debrief; the source gate's
check 8 (`check_coverage` in `scripts/phase_j_checks.py`) enforces
the scene ↔ workflow cross-reference in both directions.

**Evidence key.** Every row cites at least one of: a step in
`.github/workflows/headless.yml`, a scene in
`src/rune_sec/scenes.rs` (its doc comment and/or its numbered
layout fact in that module's `layout_assertions()` preamble), the
artifact file convention (`headless-out/tree-<scene>.json`,
`headless-out/snap-<scene>.png`, uploaded as the
`headless-probe-output` artifact), or a prior close-out
(§6.5 / §6.6 of `docs/REDESIGN_PROGRESS.md`).

**Proof-kind codes used in the table** (full definitions in §4):

- **T** — tree dump: the scene is named in a `for scene in …; do`
  loop of the workflow's "Tree dumps — plain (no display)" step
  and produces `tree-<scene>.json`.
- **S** — snap PNG: the workflow's "Snap renders — lavapipe
  attempt" step renders `snap-<scene>.png` for the scene.
- **O** — overflow scan: the tree dump is in the set scanned by
  `scripts/overflow_scan.py` at the debrief (§6.6: "overflow scan
  is CLEAN on all 27 trees" — the 24 scene trees plus the 3
  size-variant trees listed after the table).
- **—** — that proof kind is not applied to this scene.

## 1. The scene table — 24 entering scenes

All 24 scenes are in the plain tree loop, so **T and O apply to
every row**; the columns below therefore record T/O once here and
use the table for the per-scene facts that differ: the seeded
state, the snap, and the layout fact. Snap coverage is the uneven
column: **19 of 24 scenes are snapped; 5 are tree-only.**

| # | Scene | State seeded (one line, from the scene's doc comment in `scenes.rs`) | Snap | Layout fact |
|---|---|---|---|---|
| 1 | `default` | The demo project exactly as `EditorCore::new()` seeds it (frame 0, figure 0 selected, not playing); the scene body is a deliberate no-op, so `--scene default` equals no `--scene` at all. | `snap-default.png`, plus `snap-stage.png` (the `--crop stage` variant) and the 1280×720 snap below | Facts 1–6 (the base inventory every other fact references) |
| 2 | `frame-3` | Playhead on frame index 3 ("Frame 4/5") via `EditorCore::goto`. | — tree-only | Fact 7 (inventory identical to `default`) |
| 3 | `selected` | Figure 0 selected via `select_figure`, its `head` node written to the pub `selected_node` field, so selection handles exist in the drawn state. | — tree-only | Fact 7 |
| 4 | `playing` | Transport running via `chrome_view::toggle_play`. | — tree-only, deliberately: the scene's doc comment records that a `--snap` of a playing state is only deterministic at `--at 0`, and the workflow snaps it nowhere | Fact 7 |
| 5 | `export-open` | Export workspace via `workspaces::select_workspace`. | `snap-export-open.png` | Fact 8 (`export-panel` present, `figures-panel` absent) |
| 6 | `fx-selected` | Effects workspace; one particle event added at the stage centre via `EditorCore::particle_add_event` and selected, so the Particles detail block mounts. | `snap-fx-selected.png` | Fact 9 (`particles` + `particles-detail` holding `particles-kind-prev`) |
| 7 | `graph-duplicates` | Graph workspace; two `Channel` values for figure 0 with the same `RootX` target planted by direct write of the pub `project.channels` field (the module's named exception — no core op creates a channel with chosen keys). | — tree-only | Fact 10 (`graph-chan-1` exists — the second planted row) |
| 8 | `fx-split-mid` | Effects workspace; one `Burst` FX clip added via `EditorCore::add_fx_clip`, selected, playhead parked strictly inside the clip's span. | `snap-fx-split-mid.png` | Fact 11 (`fx-split` exists — mounted only with a clip selection) |
| 9 | `three-d` | Build3D workspace; live fig3 session via `EditorCore::enter_3d`, camera orbited yaw +35° / pitch +18° via `fig3_orbit`, joint 1 selected via `fig3_select_next`. | `snap-three-d.png` | Fact 12 (`panel-3d-preview` exists) |
| 10 | `loop-range` | Animate workspace; loop range over frames 1..=3 via `EditorCore::set_loop_range`, plus one planted `RootX` channel keyed flat at exactly the range's ends. | — tree-only | Fact 13 (`timeline-loop-row` exists, in its set state) |
| 11 | `three-d-stage` | The same end state as `three-d`, reached through the same ops; the separate name exists because its full-window snap is the pixel proof that the 3D figure is drawn on the MAIN STAGE while Build3D is active. | `snap-three-d-stage.png` | Fact 14 (same Build3D inventory as fact 12) |
| 12 | `palette-open` | Command palette open over the default landing via `command_palette::open_palette` (the Ctrl+K path). | `snap-palette-open.png` | Fact 15 (`command-palette` + `palette-query` exist) |
| 13 | `menu-open` | File menu dropdown open via `MenusState::toggle` with `menus::FILE`. | `snap-menu-open.png` | Fact 16 (`menu-file-dropdown`, `menu-scrim`, and the dropdown-only `export` item exist) |
| 14 | `settings-open` | Settings card expanded via `settings_popover::toggle_open`. | `snap-settings-open.png` | Fact 17 (`settings-card` exists; the `settings` shell exists under every scene, so only the card proves the flip) |
| 15 | `draw-open` | Draw workspace via `select_workspace`. The demo project carries no strokes, so the stroke inspector renders its empty state (see gap G3). | `snap-draw-open.png` | Fact 18 (`stroke-inspector` exists, `frames` absent) |
| 16 | `default-light` | The default shell in the LIGHT theme via `settings_popover::set_theme` (the settings card's own call). | `snap-default-light.png`, plus the 1280×720 light snap below | Fact 19 (inventory identical to `default` — theme is paint, not structure; the light rendering itself is a snap fact: background pixels in the light range, `bg` `#EDF0F5`) |
| 17 | `settings-open-light` | Settings card expanded in the light theme — `set_theme` then `toggle_open`, in the order a user reaches them. | `snap-settings-open-light.png` | Fact 20 (`settings-card` exists; inventory matches `settings-open`) |
| 18 | `home` | Home workspace via `select_workspace`. The probe harness wipes `./projects` and uses a fresh `HOME` per invocation (the `probe()` shell fn in `headless.yml`), so the library renders its EMPTY state (see gaps G1–G2). | `snap-home.png` (§6.6: "the Home workspace is now snapped … with an empty projects dir") | Fact 21 (`home-panel` exists, `figures-panel` absent) |
| 19 | `build-open` | Build3D workspace with NO fig3 session — deliberately no `enter_3d`; proves the gating mounts the panel in its no-session state, where `three-d` proves the session state. | `snap-build-open.png` | Fact 22 (`panel-3d` exists, `figures-panel` present) |
| 20 | `graph-open` | Graph workspace with no channels planted — the panel's empty state, where `graph-duplicates` proves the populated list. | `snap-graph-open.png` | Fact 23 (`graph-panel` exists, `figures-panel` present) |
| 21 | `effects-open` | Effects workspace with nothing added — FX browser and lanes in their empty states. | `snap-effects-open.png` | Fact 24 (`fx-browser` exists, `figures-panel` present) |
| 22 | `audio-open` | Audio workspace via `select_workspace`. | `snap-audio-open.png` | Fact 25 (`audio-panel` exists, `figures-panel` present) |
| 23 | `clips-populated` | Animate workspace; two movieclips built from the demo figure via `create_clip_from_figure` (the second renamed `Second clip`), edit session open on the first via `enter_clip`. The first scene ever to put a clip row in a headless tree. | `snap-clips-populated.png` | Fact 26 (`clip-dup-0` and `clip-dup-1` exist — two real clip rows) |
| 24 | `dopesheet-keys` | Animate workspace; real keyframes authored on two figures (figure 0 keyed at frames 0, 2, 4; figure 1 at frames 1, 3, via `delete_keyframe` / `add_figure` / `nudge_path_point` plus the scene's two documented direct-write exceptions), playhead parked on frame 2. | `snap-dopesheet-keys.png` (§6.5's pixel pass read this snap: two rows, diamonds on the authored keys) | Fact 27 (dopesheet lane rows/cells match the `stats::figure_keyframes` census exactly) |

**Size-variant probes** (window size is a CLI axis, not scene
state — the `scenes.rs` module docs record why no scene can reach
it). The workflow adds three tree dumps and two snaps at
1280×720 on top of the 24 + 22 above, which is what makes the
§6.5/§6.6 overflow denominator **27 trees**:

- `tree-default-1280.json` — `--scene default --size 1280x720`
- `tree-default-light-1280.json` — `--scene default-light --size 1280x720`
- `tree-three-d-1280.json` — `--scene three-d --size 1280x720`
- `snap-default-1280.png`, `snap-default-light-1280.png` — the
  matching snaps (there is no `three-d` snap at 1280×720).

**Snap count check.** The "Snap renders" step issues 22 snap
invocations: the 19 scene snaps in the table, the `--crop stage`
variant of `default`, and the two 1280×720 snaps. Together with
the 27 tree dumps, the rect probe and the live-MCP probe
transcript (§4), these are the whole artifact.

**Coverage enforcement.** Gate check 8
(`scripts/phase_j_checks.py`, `check_coverage`) parses the
`scenes()` vec and the workflow together and fails when: a
registered scene appears in NO tree loop (forward coverage), a
tree loop or `--scene` probe names a scene `scenes()` does not
register (reverse coverage), or the numbered layout-fact list in
`scenes.rs` falls out of sequence with `layout_assertions()`.
One known, report-only asymmetry the check itself prints: the
xvfb fallback loop in `headless.yml` lists 22 scenes — it omits
`default-light` and `settings-open-light`, which the plain loop
(the coverage contract) covers.

## 2. Wave 12 additions — in flight at launch

Neither scene exists at HEAD: `scenes()` still registers 24 and
the module's pin test still asserts 24. Both are Wave 12 squad D
work in flight (agents D2 and D4); the integrator lands the
scene, the workflow wiring and the new layout fact together, and
this section records the proof each WILL have when it lands —
reconciled against the closing tree at close-out.

| Scene (planned name) | Owner | State it will seed | Proof it will have |
|---|---|---|---|
| `home-populated` | D2 (in flight) | The Home workspace over a NON-empty projects dir: real saved project files present, so the library renders its project rows with thumbnails instead of the empty state `home` renders today. | **T** — must be added to the plain tree loop in `headless.yml` (gate check 8 forward coverage fails the gate until it is); **S** — `snap-home-populated.png` added to the "Snap renders" step and pixel-read at the debrief; **O** — its tree joins the overflow-scan set (denominator 27 → 28); a new numbered layout fact in `scenes.rs` asserting the populated row keys under `--scene home-populated`. |
| `strokes-populated` | D4 (in flight) | The Draw workspace with real strokes present and one selected, so the stroke inspector renders its populated rows instead of the empty state `draw-open` renders today. | **T** — same tree-loop requirement via gate check 8; **S** — `snap-strokes-populated.png` in the "Snap renders" step, pixel-read at the debrief; **O** — its tree joins the overflow-scan set (denominator → 29 once both additions land); a new numbered layout fact asserting the inspector's populated-row keys. |

Until both land, §6.6's closing census (scenes **24**) stands;
the closing census is expected to read **26**.

## 3. Honest gaps — unit-proven only, or unproven

Each gap names its evidence, its reason, and its owner — a Wave
12 item, or **UNOWNED** where no squad's remit covers it.

- **G1 — Populated Home rows are not pixel-proven.**
  Evidence: §6.5 ("Home project rows are still not pixel-proven
  (no populated-Home scene yet)") and §6.6 (the `home` snap
  renders "with an empty projects dir — rows and thumbnails of
  a populated Home are still not pixel-proven"). Reason: the
  only Home scene switches workspace over the probe's wiped
  `./projects`, so no row ever renders headlessly; the row
  behaviour is covered by unit tests only. **Owner: Wave 12 D2**
  (the `home-populated` scene, §2).
- **G2 — Home thumbnails are not proven in any state.**
  Evidence: §6.6 names thumbnails alongside rows as not
  pixel-proven; the thumbnail path itself exists in source
  (`src/rune_sec/home_panel.rs` documents the scan cache and
  `HomePanelState::fill_thumbnails` / `thumbnail_for`). Reason:
  a thumbnail can only render when a saved project file exists
  in the scanned dir, which no entering scene arranges — so
  thumbnails are unproven generally, not merely unproven in
  the populated case. **Owner: Wave 12 D2**, as part of the
  same scene (a populated-Home snap is the first thumbnail
  proof of any kind).
- **G3 — Stroke inspector populated rows have never been in a
  headless tree.** Evidence: layout fact 18 proves only that
  `stroke-inspector` mounts under `draw-open`; the demo project
  the scene seeds has no strokes (`Project::new_demo` builds
  the stroke vec empty — recorded in the round-4 notes of
  `src/rune_sec/snap_tests.rs`, whose stroke test therefore
  asserts empty-project behaviour). Reason: no scene seeds a
  stroke, so the inspector's per-stroke rows are exercised by
  unit tests only and have never faced the overflow scan.
  **Owner: Wave 12 D4** (the `strokes-populated` scene, §2).
- **G4 — The canvas-drawn state of the five tree-only scenes is
  pixel-unproven.** Evidence: the `layout_assertions()`
  preamble in `scenes.rs` states the tree dump carries keys,
  counts and rects only — state drawn inside the stage `Shape`
  (selection handles, the FX gizmo, loop edges, a parked
  playhead's styling) "can only be checked in the `--snap`
  PNG". The workflow snaps none of `frame-3`, `selected`,
  `playing`, `graph-duplicates` or `loop-range`. Reason:
  accepted scope, wave by wave — snaps were added where a
  wave's own surface needed pixels (facts 9, 11, 14 say so
  explicitly for their scenes). `playing` carries the extra
  recorded caveat that a playing snap is deterministic only
  at `--at 0`. **Owner: UNOWNED** — no Wave 12 item adds
  these snaps.
- **G5 — Sound is user-verified only.** Evidence: §6.6 ("sound
  remains user-verified only (no squad can listen to
  playback)"); `docs/QA_RUNE.md` carries the standing open row
  "Sound unverified on a real screen — the headless snap
  covers pixels, not audio". Reason: structural — no headless
  probe in `headless.yml` can listen; the proof is the user's
  Play test on a real machine, still outstanding. **Owner:
  UNOWNED** (the user's check, not a wave item).
- **G6 — 1280×720 coverage is three pairings, not a matrix.**
  Evidence: the size-variant probes listed in §1 — only
  `default`, `default-light` and `three-d` are dumped at
  1280×720, and only the first two are snapped there. Reason:
  window size is a CLI axis (per the `scenes.rs` module docs),
  so each small-window proof is an explicit extra invocation,
  and only the deepest/widest states were ever paired.
  **Owner: UNOWNED.**
- **G7 — The overflow scan is a debrief step, not a workflow
  step.** Evidence: `scripts/overflow_scan.py`'s own header —
  "it belongs to the headless debrief, not the source gate
  (the dumps are CI artifacts)" — and `headless.yml` contains
  no overflow step; the CLEAN verdicts in §6.5/§6.6 are the
  integrator's run over the downloaded artifact. Reason: the
  scan judges CI artifacts, which do not exist until the
  workflow finishes. Consequence: a headless run never
  self-reports overflow; skipping the debrief scan would
  silently un-prove all 27 trees. **Owner: the integrator at
  every close-out debrief** (standing practice, §6.5–§6.6).

## 4. Proof-kind definitions

**Tree dump (`--tree out.json`).** The binary applies the named
scene to the freshly initialised state and dumps the element
tree as JSON, rects included (the workflow's Q3 step, "Probe
tree JSON for element rects", greps each dump for rect fields
and the `stage` node's shape). A tree dump proves
**structure and geometry**: which elements mounted (workspace
gating — layout facts 8, 18, 21–25 are presence/absence pairs),
key counts (fact 5: one `frame-N` chip per project frame), and
exact element rects — the overflow scan's four rules and any
stacking claim are judged from these rects. It proves nothing
about paint: the `scenes.rs` preamble is explicit that
canvas-drawn state lives in the `Shape`, not the tree.

**Snap PNG (`--snap out.png`).** An offscreen render of the
whole window (or one `--crop`ped subtree) at `--at 0` on the
virtual clock, rendered under lavapipe/software Vulkan (the
workflow's Q2). A snap proves **drawn pixels** — the only
proof kind that can see poses, selection handles, gizmos,
theme colours and card styling. Two honesty limits: (a) most
snaps are proof *by reading* — the integrator pixel-reads them
at the debrief (§6.5's dopesheet read; the Wave 11 snaps copied
to `your_files/stykanimate-wave11-*.png`) — there is no
automated pixel-diff step in `headless.yml`; (b) where a pixel
assertion IS stated, it is stated in the fact itself (fact 19:
the `default-light` snap's background pixels sit in the light
range, `bg` `#EDF0F5`, where `default`'s sit in the dark
range). A snap file merely existing in the artifact is
presence, not a passed assertion.

**Overflow scan (`scripts/overflow_scan.py`).** A stdlib-python
scan over tree dumps (not snaps): Rule 1 content edges inside
the side columns, Rule 2 card bounds, Rule 3 window bounds,
Rule 4 the timeline card's vertical bounds (added Wave 10 for
the dopesheet lane). A run is CLEAN when every file reports
zero non-exempt hits. It proves **no element paints past its
card or the window** in the dumped states — the defect family
that survived Waves 1–8 in the un-scanned clips rows (§6.5,
and the `clips-populated` scene's doc comment) is exactly what
it exists to catch.

**Runtime presented rects (`src/rune_sec/snap_tests.rs`).** The
in-process `Runtime` tests are a different proof family from
the headless workflow: they drive the real shell with queued
pointer/key input and assert on the presented tree.
`Runtime::presented(key)` returns the element's **drawn** rect
`[x, y, w, h]` plus effective opacity (the module header of
`snap_tests.rs`). The Wave 10/11 lesson, recorded at §6.6
("two snap assertions were also made sound per the standing
presented-box doctrine"): a presented rect is a drawn box,
and a mid-session-mounted canvas can rest up to its own height
above its layout slot — so edge/displacement assertions
against presented rects are unsound, and exact stacking is
proven by centre order in the Runtime and by the exact rects
in the headless tree dump instead. Presented rects prove
presence, ordering and interaction outcomes; tree dumps prove
exact geometry.

**Live-MCP probe (not a scene proof).** The same workflow's Q4
step launches the real editor under Xvfb and drives its live
HTTP endpoint through the full JSON-RPC chain, writing the
assertion transcript `mcp-probe.txt` (§6.6: PASS end to end at
Wave 11 close). It proves the live control path, not layout;
it is listed here only so the artifact's contents are fully
accounted for in this ledger.

---

## 5. Close-out (integrator, 2026-11-09) — headless 42557913977

The run landed after this ledger was written; its
evidence closes the in-flight rows and three gaps:

- **Both Wave 12 scenes are in the run and proven.**
  Every probe in run 42557913977 exited 0 (the xvfb
  fallback step was skipped — the plain attempt missed
  nothing); 29 tree dumps were produced (26 scenes +
  the explicit size/light probes).
- **Overflow denominator is now 29 trees, scan CLEAN**
  (0 hits, `scripts/overflow_scan.py` over the run's
  artifact at this debrief — G7's standing arrangement).
- **G1 CLOSED** — the home-populated snap was pixel-read
  at this debrief: three real project rows (First Steps,
  Night Train, Harbour Lights) with metadata and
  Open/Duplicate/Rename/Delete, over a stage showing the
  selected project's figures.
- **G2 CLOSED** — the same snap shows rendered figure
  thumbnails inside all three Home rows.
- **G3 CLOSED** — the strokes-populated tree carries the
  stroke inspector's rows (including the new Scatter row
  keys) and its snap shows the inspector mounted under
  the Draw workspace with four stroke kinds on stage
  (plain, faded pressured, filled, spray).
- **G4, G5, G6 stand** as written (tree-only scenes'
  canvas state, sound user-verified only, 1280×720
  pairings not a matrix).
- New gap opened by this wave, recorded honestly:
  **library thumbnails are unit-proven only** — D5's
  library-populated scene was not built (a 27th scene
  was out of scope), so the library rows' new thumbnails
  have no snap yet.
