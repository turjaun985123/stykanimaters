# PHASE_F.md — Rune Phase F: proven on screen, now polish and depth

**COMPLETED 2026-11-08.** Integrated in `1fd7741` plus three
CI fix rounds (`fa1a4ef` two borrow errors, `3eedbff` a
sign-convention test fix, `c1d76cf` a presence-fade settle in
the guides Runtime test). **CI run `42461893898` GREEN:
1,416 tests passed (742 lib + 664 bin + 10 integration), 0
failed.** Final inventory: **475 element keys** (402 static +
73 dynamic patterns); tree **81,944 lines of Rust** in src.
Two honest remainders against the table below: the §2 stroke
highlight and §3 FX gizmo landed as tested geometry helpers
without a live draw call — the shell has no stroke /
particle-event *selection* state for them to read, so wiring
waits on that state existing; and playback audio (§16) is
wired into `poll()` but hearing it is user-verified only —
CI has no speakers. Everything else in the table landed as
written.

Snapshot: 2026-11-08, ~07:56 UTC, written **while the wave is
still landing** — other sections are being edited concurrently,
so every count in this file is a moving snapshot, not a final
figure. Re-run `python3 scripts/rune_inventory.py` for the
current count; the freshest run is recorded in §6.

## 1. Charter

Phase E closed every known Phase D gap and went CI-green
(run `42459754303`, 1,218 tests). It still ended with the same
honest caveat as every Rune phase before it: no build had ever
been seen on a real screen.

That changed on 2026-11-08 at 13:47 +06, when the user
downloaded the Phase E build, ran it on their own Linux PC,
and sent back a full screenshot plus a short screen video.
**Phase F's charter starts from that evidence**: fix the two
defects the screenshot exposed, finish the remainders Phase E
documented but did not close (app-bar duplicates, silent
playback, the figure-remap engine gap, the remaining
hand-audit debt), and spend the rest of the wave on depth —
stage fidelity against the rasterizer, direct manipulation
where panels still only offer buttons, and the library / pose /
clip workflows the GPUI app had and the Rune shell still
flattens.

**"Perfect", defined honestly** (same definition as Phase E):
all *known* gaps closed, the full suite green in CI, and the
inventory preserved or grown. Phase F adds one new clause:
nothing in this wave can verify pixels or sound on the user's
machine — see §5. The screenshot loop that proved Phase E
remains the only visual proof there is.

## 2. What the real screen proved — and what it exposed

Evidence: the user's screenshot of the Phase E build
(2026-11-08 13:47 +06) and their 6-second screen video of the
Phase D build, both sent in chat. This is the **first
real-screen verification of the Rune shell**; QA_RUNE §4 row 6
("First user-PC screenshot review") is closable on the
strength of it.

Proven on screen:

- The window opens on the demo project; the workspace row
  renders with **Animate** active and only Animate's panels
  mounted — Phase E's workspace gating is real, not just
  state.
- The stage draws the figure with its head ring, red joint
  handles on the selected figure, and the green onion-skin
  ghost beside it.
- The Phase E surfaces are all visibly present: timeline
  chips (1–5), the camera rotation dial, Copy keys / Paste
  keys, the library search box, the per-figure card controls.
- In the video, **playback actually plays**: the playhead
  advances to frame 4/5, the pose changes, the transport
  button switches to Pause and the status line reads Playing.

Exposed by the same evidence — the two defects Phase F
opens with:

- **D1 — Status-strip text overlap.** At the right end of the
  bottom status line, the hint text is drawn over another
  segment. Visible in both the screenshot and the video, so
  it is not a one-frame artefact: the strip's segments do not
  make room for each other at real window widths.
- **D2 — A stray ".layout" row.** The left column's project
  area lists a ".layout" entry with an Open button — the
  shell's own settings file surfacing as an openable project.
  It should never appear in the project list.

## 3. The 40 sections

Rows whose files were seen landed or in flight at snapshot
time are stated plainly; rows marked *(brief)* are per the
wave brief and their files were still landing — correct them
against the files at integration time, as with Phase E.

| § | File | What it does |
|---|---|---|
| 1 | `src/stage_view.rs` + `src/rune_sec/stage_layers.rs` | Stage gradients: segment gradient colours (node `color2`) drawn on the Rune stage the way the rasterizer and SVG draw them, not flat first-colour |
| 2 | `src/rune_sec/stage_layers.rs` | Stroke highlight geometry: the selected/authored stroke is visually distinguishable on stage (pure stage-space geometry + tests) |
| 3 | `src/rune_sec/stage_fx.rs` | FX gizmos: anchor/handles geometry for the selected FX clip on stage, mirroring the particle geometry the section already owns |
| 4 | `src/rune_sec/stage_camera.rs` | Camera frustum + safe-frame overlay geometry (frame border / safe area through the rotated transform) |
| 5 | `src/rune_sec/stage_text.rs` | Text backing plates: the plate behind text labels the rasterizer draws, as stage-space geometry |
| 6 | `src/rune_sec/status_strip.rs` | **Fixes D1**: segments size/truncate so no two texts share the same space at any window width |
| 7 | `src/rune_sec/home_panel.rs` | **Fixes D2**: the shell's own files (`.layout.json` and friends) never list as openable projects |
| 8 | `src/chrome_view.rs` | Chrome dedup: the app bar's duplicate `add-figure` and `onion` buttons are removed; the panel homes (§29/§30 of Phase E) become the single surfaces — the remainder of QA_RUNE §6 row 2 |
| 9 | `src/rune_sec/menus.rs` | Menu hints: shortcut hints on menu items, matching the bindings `on_key` actually wires |
| 10 | `src/rune_sec/workspaces.rs` | Workspace blurbs: a one-line description per workspace shown with the workspace row |
| 11 | `src/rune_sec/timeline_marks.rs` | Timeline loop drag: drag the loop-range edges on the timeline itself, not only via frame-ops state |
| 12 | `src/rune_sec/command_palette.rs` | Palette recents: recently run commands surface first in the palette |
| 13 | `src/rune_sec/graph_panel.rs` *(brief)* | Graph tangent drag: drag key tangent handles on the curve canvas, extending §30-of-Phase-E key dragging |
| 14 | `src/rune_sec/camera_panel.rs` | Camera key copy/paste: duplicate a camera key to another frame from the key list |
| 15 | `src/rune_sec/scene_panel.rs` | Stage-size presets: one-press stage dimensions alongside the existing steppers |
| 16 | `src/audio_play.rs` (**new lib module**) | Audio playback glue: `start_playback_audio` / `stop_playback_audio` / `playback_audio_active` on `EditorCore`, delegating to the engine's existing `audio::AudioPlayer` (ffplay; Wave 10) and the export's own mix. The player existed all along — the GPUI shell owned it, Phase B deleted the owner, and no Rune code has called it since. Shell wiring is the integrator's step (§4) |
| 17 | `src/rune_sec/playback_controls.rs` | Playback controls: the panel surface over the §16 glue (start/stop state shown honestly, incl. "nothing audible here") |
| 18 | `src/rune_sec/figures_panel.rs` *(brief)* | Figure thumbnails: a small rendered preview per figure card |
| 19 | `src/rune_sec/inspector_figure.rs` *(brief)* | Figure rename: rename the selected figure from its inspector |
| 20 | `src/rune_sec/inspector_segment.rs` | Segment angle steppers: direct angle entry/stepping for the selected segment |
| 21 | `src/rune_sec/inspector_style.rs` | Node RGB editor: per-channel colour editing for the selected node, over the Phase E direct setters |
| 22 | `src/rune_sec/pose_tools.rs` | Pose library: save/load named poses, extending copy/paste pose |
| 23 | `src/rune_sec/keyframe_tools.rs` *(brief)* | Keyframe nudge/clear: nudge the selected joint's keys by a frame; clear a frame range |
| 24 | `src/rune_sec/frame_ops.rs` | Frame duration stepper: direct per-frame duration control in the frame-ops surface |
| 25 | `src/rune_sec/draw_tools.rs` *(brief)* | Draw stamp controls: size/options for the stamp shape generators |
| 26 | `src/rune_sec/stroke_inspector.rs` | Stroke bulk ops: select/delete operations across strokes, over `delete_stroke_at` |
| 27 | `src/rune_sec/text_tool.rs` | Text list: the project's text instances listed and selectable, not only creatable |
| 28 | `src/rune_sec/fx_browser.rs` *(brief)* | FX search/recents: filter the 27-kind browser; recently used kinds first |
| 29 | `src/rune_sec/fx_clips.rs` *(brief)* | FX nudge: frame-nudge the selected FX clip from the panel |
| 30 | `src/rune_sec/clips_panel.rs` *(brief)* | Clip duplicate/rename for movieclips |
| 31 | `src/rune_sec/three_d_panel.rs` *(brief)* | 3D readouts: numeric joint/camera readouts in the 3D panel (also pays down its per-key hand-audit debt) |
| 32 | `src/rune_sec/export_panel.rs` *(brief)* | Export history: past exports listed under the Phase E recap line |
| 33 | `src/rune_sec/library_panel.rs` *(brief)* | Library rename: rename a saved library figure |
| 34 | `src/rune_sec/layers_panel.rs` *(brief)* | Layer opacity slider: drag control beside the existing steppers |
| 35 | `src/rune_sec/easing_panel.rs` *(brief)* | Easing picker: the project/figure easing choices as a picker over `set_project_easing` and friends |
| 36 | `src/rune_sec/toasts.rs` *(brief)* | Action toasts: toasts that can carry an action (e.g. Undo) — **expected to change the `ToastsState::push` / `toasts_view` signature; the integrator wires the call sites (§4)** |
| 37 | `src/figure_ops.rs` + engine *(brief)* | Figure remap fix: `duplicate_figure` / `delete_figure` remap FX-clip anchors and graph channels — the pre-existing engine gap Phase E documented and left open |
| 38 | `src/mcp_tools.rs` *(brief)* | MCP parity: the Phase E/F engine ops exposed as MCP tools in the existing naming convention |
| 39 | `src/rune_sec/snap_tests.rs` *(brief)* | Runtime tests round 3: the Phase F surfaces driven end-to-end headlessly (workspaces row interactions, loop drag, palette recents, dedup'd chrome) |
| 40 | `docs/PHASE_F.md` + `docs/QA_RUNE.md` §8 | This document |

## 4. Integration checklist (for the integrator)

Library declarations — new files the lib must declare:

- [ ] `pub mod audio_play;` in `src/lib.rs` (§16's file
      exists; at snapshot time lib.rs did **not** declare it)

Audio wiring (§16 — the wave's one conditional):

- [ ] §16's verdict is in and it is favourable: a real player
      exists (`audio::AudioPlayer`, ffplay-based since
      Wave 10), and §16 landed `start_playback_audio` /
      `stop_playback_audio` / `playback_audio_active` on
      `EditorCore`. What remains is shell wiring only.
- [ ] In `main.rs`: when playback starts (Play pressed /
      `core.playing` flips true), call
      `core.start_playback_audio()`; when it ends or pauses,
      call `core.stop_playback_audio()`. §16's convention:
      audio starts at `current / fps` (fps floored at 1);
      success writes nothing to `core.status`, so the toast
      feed stays quiet on success — keep it that way.
- [ ] If the landed method names differ from the three above,
      wire to the landed names and correct §3/§16's row.
- [ ] Do **not** claim playback audio works on the strength
      of CI: the suite can prove the calls dispatch, not that
      sound comes out. User-verified only (§5).

Toasts signature change (§36):

- [ ] Diff the landed `toasts.rs` against Phase E's
      `push(text, severity, now)` / `toasts_view(&state, now)`:
      if §36 changed either signature, update **every** call
      site in one pass — `main.rs` `poll()` (the status→toast
      feed) plus any section that pushes directly — and check
      the section tests that construct `ToastsState`.

State fields:

- [ ] Collect every `State` field the Phase F sections
      declare in their file headers (palette recents §12,
      export history §32, action toasts §36 are the likely
      ones) and add them all to `State` + `init()` in one
      pass, with the inits the sections specify.
- [ ] Mechanically patch the section-test `State { … }`
      literals with the new fields (the Phase D/E pattern —
      the literals end at `last_dark: true,`).

Chrome dedup (§8) fall-out:

- [ ] After the app-bar `add-figure` / `onion` buttons are
      removed, re-check `snap_tests` rounds 1–3 for taps on
      the removed keys; retarget them at the surviving panel
      controls. The inventory should drop by exactly these
      deliberate removals and nothing else.

Post-integration verification:

- [ ] `python3 scripts/rune_inventory.py` — count must be
      ≥ Phase E's 408 minus §8's two deliberate removals,
      plus Phase F's new keys
- [ ] Push; read the **Build, Test & Binary** run; fix from
      CI logs only (repo rule: no local cargo)
- [ ] Fill the CI column of the QA_RUNE.md §8 addendum and
      close QA_RUNE §4 row 6 on the strength of the user's
      2026-11-08 screenshot

## 5. Honest limits

- **Playback audio depends on §16 having found a real
  player — it did** (`audio::AudioPlayer` has existed since
  Wave 10; §16 glues it back). But "glued and green" is not
  "heard": CI has no speakers and no audio device. Whether
  sound comes out, in sync enough to be useful, is provable
  only by the user pressing Play on a project with clips.
  Until then the honest status is *wired, unverified*.
- **The player §16 revives has inherited limits**, documented
  in `audio_play.rs` itself: no sample-clock sync — if the
  visual playhead loops, the audio does not loop with it;
  scrubbing while playing does not move the audio. Closing
  those needs a clock-synced player; that is a different
  piece of work and Phase F does not claim it.
- **Nothing in this wave verifies pixels on a real screen.**
  The stage-fidelity sections (§1–§5) ship geometry and
  Runtime tests; gradients, gizmos, frustums and text plates
  will look right or wrong only in the user's next
  screenshot. The screenshot loop remains the proof, exactly
  as it was for Phase E.
- **`--snap` on GPU-less CI remains unproven** (software
  Vulkan); the `headless.yml` workflow added in Phase E is
  still dispatch-only and has never been run. The §3 budget
  check in QA_RUNE stays open.
- **Rune is pre-release** and pinned at rev `87caf2a`;
  nothing in Phase F changes the pin.
- Sections marked *(brief)* are recorded from the wave
  brief, not from landed headers, in this snapshot; their
  rows should be corrected against the files at integration
  time.

## 6. Inventory snapshot

`python3 scripts/rune_inventory.py`, last run 2026-11-08
07:56 UTC (mid-wave, files still landing — treat as a
floor, not a final count):

- **442 unique keys** (375 static + 67 dynamic patterns),
  vs 408 at Phase E green (run `42459754303`).

The count will move again before integration (§8's two
deliberate removals land in it too); the integrator's
post-wiring run is the number of record.
