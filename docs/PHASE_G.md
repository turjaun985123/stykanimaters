# PHASE_G.md — Rune Phase G: closing the named remainders

Snapshot: 2026-11-08, ~08:25 UTC, written **while the wave is
still landing** — other sections are being edited concurrently,
so every count in this file is a moving snapshot, not a final
figure. Re-run `python3 scripts/rune_inventory.py` for the
current count; the freshest run is recorded in §6.

## 1. Charter

Phase F closed every Phase E remainder it aimed at and went
CI-green (run `42461893898` on `c1d76cf`: **1,416 tests
passed — 742 lib + 664 bin + 10 integration, 0 failed**;
final inventory 475 element keys; tree 81,944 lines of Rust).
It also did something the earlier phases did not always do:
it wrote its own leftovers down, by name, in its completion
note (`docs/PHASE_F.md`, header) and in QA_RUNE §8.

**Phase G's charter starts from that list.** The wave is
organised remainder-first: the two stage helpers that landed
tested but undrawn (stroke highlight, FX gizmo) get the
selection state they were waiting for; the one figure-remap
case Phase F's census found but did not fix (reorder) gets
the same treatment duplicate/delete got; the playback
player's documented loop limit gets its wrap restart; the
audio ops that only ever acted on the *last* clip get indexed
forms and a panel selection to act on; the one panel whose
hand-audit stayed owed (project_tools) gets it; and the
workspace `blurb()` that Phase F shipped without a consumer
gets one. The rest of the wave is depth in the same places
Phase F went deep — direct manipulation, per-item surfaces
where panels still flatten lists, and controls the GPUI app
had that the Rune shell still lacks.

**"Perfect", defined honestly** (same definition as Phases
E and F): all *known* gaps closed, the full suite green in
CI, and the inventory preserved or grown. Phase G keeps
Phase F's added clause unchanged: nothing in this wave can
verify pixels or sound on the user's machine — see §5. The
user's screenshot-and-Play loop remains the only visual and
audio proof there is.

## 2. What Phase F left open — the named remainders

Evidence: `docs/PHASE_F.md`'s completion note and QA_RUNE
§8's rows, re-checked against the tree at snapshot time.

Left open, and assigned in this wave:

- **Stroke highlight not drawn.** Phase F §2 landed
  `highlight_geometry` (`src/rune_sec/stage_layers.rs:382`)
  as tested geometry, but nothing calls it in the stage
  pass: the shell had no selected-stroke state to feed it.
  The groundwork state now exists — `State::stroke_selected:
  Option<usize>` (`src/main.rs:99`, init `src/main.rs:167`).
- **FX gizmo not drawn.** Same story for Phase F §3's
  gizmo geometry in `stage_fx.rs`: it indexes
  `Project::particles` *events*, and no panel listed or
  selected those events. Groundwork: `State::particle_selected:
  Option<usize>` (`src/main.rs:103`, init `src/main.rs:168`),
  and a new engine surface, `src/particle_ops.rs` (landed;
  lib registration is the integrator's step, §4).
- **`move_figure` remaps nothing.** Phase F §37 closed the
  duplicate/delete remap gap and its census found the
  adjacent one: `EditorCore::move_figure`
  (`src/editor.rs:1655`) swaps the figure and its per-frame
  poses and nothing else — particle anchors, FX-clip
  entries, graph channels and `camera_follow` keep pointing
  at the old indices. Still true at snapshot time (the
  function body is unchanged).
- **`blurb()` has no consumer.** Phase F §10 added
  `Workspace::blurb()` (`src/rune_sec/workspaces.rs:182`,
  with `blurb_line` at `:411`); nothing renders it.
- **Playback audio does not loop.** `src/audio_play.rs`'s
  own module docs carry the inherited limits: the audio
  does not restart when the visual playhead wraps, and
  scrubbing while playing does not move the audio. The
  wrap half is this wave's §2; the module docs at snapshot
  time already describe the sync method that closes it
  (`src/audio_play.rs:31–74`). Scrub-follow stays open
  (`src/audio_play.rs:64`) — Phase G does not claim it.
- **Audio ops act on the last clip only.** The panel
  surface Phase E shipped drove the `_last` wrappers in
  `src/audio_ops.rs`. The indexed family now exists in
  that file (`audio_fade_at` `:83`, `audio_reverse_at`
  `:133`, `audio_set_gain_at` `:188`,
  `delete_audio_clip_at` `:245`, `audio_move_at` `:267`);
  what was missing is a per-clip selection for them to
  act on (§9).
- **project_tools hand-audit still owed.** QA_RUNE §8
  closed the audit debt for the five files Phase F
  reworked and named the exception: project_tools has
  never had its per-key hand pass. §12 pays it.
- **One guides flag, three overlays.** Phase F §4 drew
  the stage guides, the camera frustum and the safe
  frames under the single `State::guides` flag (one
  `if guides` block, `src/stage_view.rs:400–416`).
  Groundwork splits it: `guides_frustum` / `guides_safe`
  (`src/main.rs:106–109`, both init `true` at `:169–170`,
  so today's rendering is preserved until §13's toggles
  land).

Not assigned, and still open after this wave unless the
user says otherwise: **`--snap` on GPU-less CI.** The
`headless.yml` workflow is still `workflow_dispatch`-only
(`.github/workflows/headless.yml:48`, `:77–78`) and has
never been run; no Phase G section changes that. See §5.

## 3. The 40 sections

Rows whose files were seen landed or in flight at snapshot
time are stated plainly; rows marked *(brief)* are per the
wave brief and their files were still landing — correct them
against the files at integration time, as with Phases E
and F.

| § | File | What it does |
|---|---|---|
| 1 | `src/figure_ops.rs` + engine *(brief)* | Reorder remap: swapping two figures remaps every figure-indexed reference (particles, FX-clip entries, graph channels, camera_follow) — the `move_figure` gap §2 names; the `editor.rs` call is the integrator's wiring (§4) |
| 2 | `src/audio_play.rs` | Loop-wrap restart: when the visual playhead wraps, the audio pass restarts with it, per the sync method the module docs describe (`:31–74`); the `poll()` call is the integrator's step (§4). Scrub-follow explicitly not claimed |
| 3 | `src/audio_ops.rs` | Indexed clips: the `_at(index)` op family (fade/reverse/gain/delete/move) as the primary surface, with the `_last` wrappers kept for existing callers |
| 4 | `src/particle_ops.rs` (**new lib module**) | Particle-event ops: the engine surface over `Project::particles` events (the list the FX gizmo indexes) — landed at snapshot time; `pub mod particle_ops;` in lib.rs is the integrator's step (§4) |
| 5 | `src/rune_sec/stage_layers.rs` *(brief)* | Stroke picking: hit-testing over the stage's stroke geometry, writing `State::stroke_selected` — the selection source Phase F §2's highlight was waiting for |
| 6 | `src/stage_view.rs` *(brief)* | Selection drawing + guide split: the stage pass draws the stroke highlight (§5's selection) and the FX gizmo (§8's selection), and draws guides / frustum / safe frames under their separate State flags instead of the one `guides` block at `:400–416` |
| 7 | `src/rune_sec/stroke_inspector.rs` *(brief)* | Selection UI: the selected stroke shown and operable in the inspector, two-way with §5's stage picking |
| 8 | `src/rune_sec/particles_panel.rs` (**new**) | Particles panel: the project's particle events listed and selectable, writing `State::particle_selected` — the selection source the FX gizmo was waiting for. File landed at snapshot time; mod declaration, `Section` gating and mount are the integrator's steps (§4) |
| 9 | `src/rune_sec/audio_panel.rs` *(brief)* | Per-clip selection: clips listed and selectable, so §3's indexed ops act on the chosen clip instead of always the last one |
| 10 | `src/rune_sec/waveform_view.rs` *(brief)* | Waveform playhead + seek: the playhead drawn on the waveform; clicking seeks. The integrator switches the `main.rs` call site (`:503`, fixed size `[308.0, 56.0]` at snapshot time) to the state-ful form (§4) |
| 11 | `src/rune_sec/workspaces.rs` *(brief)* | Blurb consumer: the workspace row shows the active workspace's `blurb()` line — the consumer Phase F §10 shipped without |
| 12 | `src/rune_sec/project_tools.rs` *(brief)* | Hand-audit: the per-key audit pass QA_RUNE §8 left owed for this file, plus anything it surfaces |
| 13 | `src/rune_sec/settings_popover.rs` *(brief)* | Guide toggles + startup workspace: separate switches for the frustum / safe-frame overlays (over the `guides_frustum` / `guides_safe` groundwork), and the workspace the shell starts in |
| 14 | `src/rune_sec/camera_panel.rs` *(brief)* | Camera follow picker: choose which figure the camera follows (writes `Project::camera_follow`) from the panel, instead of the field being import-only |
| 15 | `src/rune_sec/timeline_marks.rs` *(brief)* | Timeline key markers: graph/camera keys marked on the timeline itself |
| 16 | `src/rune_sec/graph_panel.rs` *(brief)* | Channel list: the project's graph channels listed and selectable in the panel, not only reachable through the selected joint |
| 17 | `src/rune_sec/figures_panel.rs` *(brief)* | Figures filter: filter the figure list by name |
| 18 | `src/rune_sec/layers_panel.rs` *(brief)* | Jump reorder: move a figure to the top/bottom of the order in one press, over the same remap path as §1 |
| 19 | `src/rune_sec/inspector_segment.rs` *(brief)* | Bend steppers: direct stepping of the selected segment's bend, beside Phase F's angle steppers |
| 20 | `src/rune_sec/inspector_style.rs` *(brief)* | Style copy/paste: copy the selected node's style and paste it onto another node |
| 21 | `src/rune_sec/inspector_figure.rs` *(brief)* | Figure scale: scale the selected figure's proportions from its inspector |
| 22 | `src/rune_sec/pose_tools.rs` *(brief)* | Pose library delete/rename/mirror: manage the on-disk poses Phase F §22 created, and mirror a pose on apply |
| 23 | `src/rune_sec/keyframe_tools.rs` *(brief)* | Key-all + copy-to-frame: key every channel of the selected joint at once; copy a frame's keys to another frame |
| 24 | `src/rune_sec/frame_ops.rs` *(brief)* | Insert N frames: insert a chosen number of frames at the playhead, extending the single-frame insert |
| 25 | `src/rune_sec/onion_controls.rs` *(brief)* | Onion depths: how many ghost frames show before/after, over the `State::onion_ui` groundwork (`src/main.rs:111`) — the panel surface for depths the engine's fixed falloff previously decided |
| 26 | `src/rune_sec/playback_controls.rs` *(brief)* | Loop toggle: playback loops or stops at the end, over the `State::loop_playback` field the integrator adds (§4) — until now `advance_play` looped unconditionally and no off switch existed |
| 27 | `src/rune_sec/status_strip.rs` *(brief)* | Status segments: the strip's segments revisited (order/content) on top of Phase F §6's no-overlap sizing; the integrator switches the `main.rs` call site (`:545`) to the state-ful form (§4) |
| 28 | `src/rune_sec/toasts.rs` *(brief)* | Toast dismissal: dismiss a toast before its TTL expires, extending Phase F §36's action toasts |
| 29 | `src/rune_sec/command_palette.rs` *(brief)* | Palette Phase G commands: this wave's new surfaces reachable from the palette, in the existing command-table convention |
| 30 | `src/rune_sec/help_panel.rs` *(brief)* | Shortcut audit: the help panel's shortcut list checked line-by-line against the bindings `on_key` actually wires |
| 31 | `src/rune_sec/menus.rs` *(brief)* | Menu guide items: the View menu gains the frustum / safe-frame toggles beside its existing guides item, calling the same writers §13 uses |
| 32 | `src/rune_sec/home_panel.rs` *(brief)* | Reopen-last: reopen the most recent project from Home in one press |
| 33 | `src/rune_sec/stats_view.rs` *(brief)* | Stats totals: project totals (frames/figures/clips/events) in the stats surface |
| 34 | `src/rune_sec/fx_clips.rs` *(brief)* | FX clip strip: the project's FX clips as a strip/list surface, extending §29-of-Phase-F's nudge row |
| 35 | `src/rune_sec/clips_panel.rs` *(brief)* | Movieclip delete/reorder: the two list ops Phase F §30's duplicate/rename left out |
| 36 | `src/rune_sec/three_d_panel.rs` *(brief)* | 3D pose library UI: the panel surface over fig3d's on-disk pose library (engine side landed in Wave 12), matching §22's 2D library |
| 37 | `src/rune_sec/scene_panel.rs` *(brief)* | Custom background: a user-chosen stage background colour beyond the preset list |
| 38 | `src/rune_sec/text_tool.rs` *(brief)* | Text instance edit: edit an existing text instance's content/size from Phase F §27's list, not only load-and-restamp |
| 39 | `src/rune_sec/snap_tests.rs` *(brief)* | Runtime tests round 4: the Phase G surfaces driven end-to-end headlessly (selection flow, particles panel, loop toggle, guide toggles) |
| 40 | `docs/PHASE_G.md` + `docs/QA_RUNE.md` §9 | This document |

## 4. Integration checklist (for the integrator)

State groundwork — already done before the wave landed
(verified at snapshot time; do not redo):

- [x] `src/main.rs` `State` carries `stroke_selected`
      (`:99`), `particle_selected` (`:103`),
      `guides_frustum` (`:106`), `guides_safe` (`:109`)
      and `onion_ui` (`:111`), with inits at `:167–171`
      (frustum/safe default `true`, preserving Phase F
      rendering)

State still to add:

- [ ] `loop_playback: bool` on `State` + `init()` (§26's
      loop toggle reads/writes it; absent from main.rs at
      snapshot time). Pick the default that preserves
      today's behaviour — `advance_play` loops
      unconditionally now, so the field should init `true`
- [ ] Mechanically patch the section-test `State { … }`
      literals with any new fields (the Phase D/E/F
      pattern), and collect any further fields the landed
      sections declare in their file headers in the same
      pass

Library declarations:

- [ ] `pub mod particle_ops;` in `src/lib.rs` (§4's file
      exists; at snapshot time lib.rs declares `audio_ops`
      at `:8` and `audio_play` at `:9` but not
      `particle_ops`)

Rune section wiring:

- [ ] Declare `pub mod particles_panel;` in
      `src/rune_sec/mod.rs` (§8's file exists; undeclared
      at snapshot time)
- [ ] Add a `Section::Particles` variant to the `Section`
      enum (`src/rune_sec/workspaces.rs:89`; 28 variants
      at snapshot time, none for particles), gate it to
      its workspace in `section_visible`, and mount
      `particles_panel` in `main.rs`'s view alongside the
      other right-column sections

Engine wiring:

- [ ] In `src/editor.rs` `move_figure` (`:1655`): after
      the figures/poses swap, call §1's reorder-remap so
      particles, FX-clip entries, graph channels and
      `camera_follow` follow the swap. §18's jump reorder
      should land on the same path, not a second one
- [ ] If §1's landed helper name differs from the brief,
      wire to the landed name and correct §3's §1 row

Shell wiring (`src/main.rs`):

- [ ] `poll()`: call §2's audio sync so a playhead wrap
      restarts the audio pass; and when `loop_playback`
      is false, stop playback at the end of the timeline
      (and stop the audio with it, through the existing
      playing-flip hook) instead of wrapping
- [ ] Switch the status-strip call site (`:545`,
      `status_strip(core)` at snapshot time) to §27's
      state-ful form, and the waveform call site (`:503`,
      `waveform_el(core, [308.0, 56.0])`) to §10's
      state-ful form — exact names from the landed files
- [ ] `text_focused()`: add every new `TextEdit` the
      wave's sections declare (figures filter §17 and
      scene custom background §37 are the likely ones),
      following the existing field list's pattern, so
      typing in them doesn't trigger global shortcuts

Post-integration verification:

- [ ] `python3 scripts/rune_inventory.py` — count must be
      ≥ Phase F's 475 plus Phase G's new keys; no
      deliberate removals are planned in this wave, so
      any drop is a regression to explain
- [ ] Push; read the **Build, Test & Binary** run; fix
      from CI logs only (repo rule: no local cargo)
- [ ] Fill the CI column of the QA_RUNE.md §9 addendum

## 5. Honest limits

- **Playback audio is still *wired, unverified*.** Phase G
  closes the loop-wrap limit in code (§2), but CI has no
  speakers and no audio device: whether sound comes out,
  restarts on the wrap, and stays in sync enough to be
  useful is provable only by the user pressing Play on a
  project with clips. One user Play-press now verifies
  Phase F's wiring and Phase G's wrap restart together.
- **Scrub-follow remains open, by design.** Seeking or
  scrubbing while playing still does not move the audio
  (`src/audio_play.rs:64` says so in its own docs). §2
  closes the wrap case only; a clock-synced player is a
  different piece of work and Phase G does not claim it.
- **Nothing in this wave verifies pixels on a real
  screen.** The selection drawing (§6), the split guide
  overlays (§6/§13), the waveform playhead (§10) and the
  particles panel (§8) ship geometry, panels and Runtime
  tests; they will look right or wrong only in the user's
  next screenshot. The screenshot loop remains the proof,
  exactly as it was for Phases E and F.
- **`--snap` on GPU-less CI remains unproven** (software
  Vulkan); the `headless.yml` workflow added in Phase E
  is still dispatch-only and has never been run. The §3
  budget check in QA_RUNE stays open. Phase G does not
  touch this.
- **Rune is pre-release** and pinned at rev `87caf2a`;
  nothing in Phase G changes the pin.
- Sections marked *(brief)* are recorded from the wave
  brief, not from landed headers, in this snapshot; their
  rows should be corrected against the files at
  integration time.

## 6. Inventory snapshot

`python3 scripts/rune_inventory.py`, last run 2026-11-08
08:25 UTC (mid-wave, files still landing — treat as a
floor, not a final count):

- **526 unique keys** (448 static + 78 dynamic patterns),
  vs 475 at Phase F green (run `42461893898`).

The count will move again before integration as the
remaining sections land; the integrator's post-wiring run
is the number of record.
