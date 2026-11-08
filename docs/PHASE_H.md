# PHASE_H.md — Rune Phase H: icons, and the structural remainders

Snapshot: 2026-11-08, ~08:40 UTC, written **while the wave is
still landing** — other sections are being edited concurrently,
so every count in this file is a moving snapshot, not a final
figure. Re-run `python3 scripts/rune_inventory.py` for the
current count; the freshest run is recorded in §6.

## 1. Charter

Phase H has two halves.

**The headline is icons.** Until this wave, every control in
the Rune shell was a text run: the shell's button —
`chrome_view::btn` (`src/chrome_view.rs:37`) — renders a
single text run and has no icon slot (the fact is documented
in `src/rune_sec/icon_btn.rs`'s header, which exists because
of it). Phase H adopts **Font Awesome Free 7.3.1** as the
shell's icon source and gives the panels glyph buttons:

- The complete Free set was downloaded into
  `assets/icons/fontawesome/` (commit `362b7a7`):
  **2,884 files** — `LICENSE.txt` plus the `solid/`,
  `regular/` and `brands/` SVG trees (2,885 files under
  `assets/icons/` in total, counting this file's companion
  `assets/icons/README.md`). The icons are **CC BY 4.0**;
  the attribution lives in `assets/icons/README.md` and
  the licence text in `assets/icons/fontawesome/LICENSE.txt`.
  The full set stays on disk as the source of truth for
  picking more icons later.
- A curated subset — **102 icons** — was generated into
  `src/rune_sec/icon_data.rs` by the integrator as wave
  groundwork (commit `bc809bc`): a `(view_box_width,
  path_data)` table behind `icon_data(name)`, view-box
  height always 512, path data verbatim Font Awesome.
- The rendering pipeline is three pieces: a lib SVG path
  parser (`src/icon_path.rs`, §1 — arcs converted to
  cubics) → `src/rune_sec/icons.rs` (§2), which turns
  parsed segments into a Rune `Path` and fills it via
  `Shape::fill_path` under the icon's real `view_box`
  (`src/rune_sec/icons.rs:111–112`), with the parse cached
  thread-locally → `src/rune_sec/icon_btn.rs` (§3), the
  house icon buttons (`icon_btn` / `icon_only_btn`),
  styled value-for-value like `btn` and falling back to
  the plain text button when a name is unknown.
- §§4–35 then adopt icons panel by panel (§3).

**The second half is remainder-first, as in Phases F and
G.** Phase G's sections wrote their own limits down — in
§29's documented palette skips, §9's and §20's panel notes,
and §12's integrator items. Phase H takes the structural
ones: the palette's dispatch signature (§36), the six
audio DSP ops still acting on the last clip only (§37),
the non-atomic style paste and the save op that cannot
report failure (§38), the waveform's two coexisting
coordinate mappings (§29), and the project-panel key
coverage test §12 said belongs in the Runtime suite (§39).

**The honest unknown, stated up front:** how Rune's
`fill_path` renders compound paths — glyphs whose outline
has holes (rings with counters, letterforms) — is **not
pixel-verified**. `src/rune_sec/icons.rs` says so in its
own header. The curated set was chosen as solid-style
icons so most glyphs read either way, but hole rendering
is unproven until the user's first screenshot of this
build. **"Perfect", defined honestly** (same definition
as Phases E–G): all *known* gaps closed, the full suite
green in CI, the inventory preserved or grown — and
nothing in this wave verifies pixels or sound on the
user's machine; the screenshot-and-Play loop remains the
only visual and audio proof there is.

## 2. What Phase G left open — the named remainders

Evidence: `docs/PHASE_G.md` (§2, §5) and the Phase G
sections' own notes, re-checked against the tree at
snapshot time.

Left open, and assigned in this wave:

- **The palette cannot reach shell state.** Phase G §29
  shipped four new commands and documented two skips with
  one root cause: the command table dispatches
  `run: fn(&mut EditorCore)`, and the loop-playback toggle
  and the stroke step-selection are `State`-level — no
  command can move them. §36 migrates the dispatch and
  lands the two skipped commands.
- **Six audio DSP ops act on the last clip only.** Phase G
  §3/§9 indexed fade, reverse, gain, move and delete and
  gave the panel a per-clip selection; Echo, Lowpass,
  Trim silence, Split silences, Pitch and Compress stayed
  last-clip-only because no indexed engine forms exist.
  §37 adds them.
- **Style paste is three undo steps.** Phase G §20's
  documented caveat: its setters split — `set_node_color`
  does not snapshot; `set_node_cap`, `set_node_color2`
  and `set_node_opacity` each do — so one paste lands as
  three undo entries and no panel-side composition can
  make it atomic. §38 adds the engine-side atomic setter
  (`set_node_style_atomic`).
- **A failed save still writes a recovery snapshot and a
  recent entry.** Phase G §12's integrator item 3:
  `save_named_project` returns `()`, so the panel handler
  runs its recovery-write + `touch_recent` even when the
  save failed — a phantom name in recents with no file
  behind it. §38 gives the op a `bool` return and the
  handler branches on it.
- **The waveform has two coordinate mappings.** Phase G
  §10 kept the existing `waveform_el` builder on its
  clip-span mapping (peaks and playhead mapped into the
  last clip's span) and added `waveform_el_state` on a
  project mapping, because marker and press must agree;
  the two mappings coexisting is the leftover. §29
  unifies them as part of its audio-surface pass.
- **No key-coverage test for the project panel.** Phase G
  §12's integrator item 4: its audit map cannot be
  asserted from the panel file — Rune's `El` exposes no
  public tree-walk — so emitted-key coverage belongs in a
  Runtime test in `snap_tests.rs`. §39 writes it, as part
  of Runtime tests round 5.

Not assigned, and still open after this wave: see §5.

## 3. The 40 sections

Rows whose files were seen landed at snapshot time are
stated plainly; rows marked *(brief)* are per the wave
brief and their files were still landing — correct them
against the files at integration time, as with Phases E,
F and G. For the panel rows (§§4–35), "landed" means the
file already imports `icon_btn` / `icon_only_btn` at
snapshot time (grep evidence); that was true of 20 files
(the 19 panels marked plainly below, plus §4's
`chrome_view.rs`).

| § | File | What it does |
|---|---|---|
| 1 | `src/icon_path.rs` (**new lib module**) *(brief)* | SVG path parser: Font Awesome path data → segments, arcs converted to cubics. Not yet landed at snapshot time — `src/rune_sec/icons.rs:48` already imports `stykanimate_rs::icon_path::{parse_path, Seg}` (and `path_bbox` at `:142`) from it; the file and its lib.rs registration are the integrator's step (§4) |
| 2 | `src/rune_sec/icons.rs` | Icon builder: name → `icon_data` lookup → parsed segments → Rune `Path` → `Shape::fill_path` under the icon's `view_box` (`:111–112`); `icon_el` sizes the canvas by the icon's real aspect; parses cached thread-locally; unknown names yield `None`/an empty spacer, never a panic. Its header carries the fill-rule caveat (§1) |
| 3 | `src/rune_sec/icon_btn.rs` | House icon buttons: `icon_btn` (glyph + label) and `icon_only_btn`, mirroring `chrome_view::btn`'s padding/radius/fills/label colour value for value, the glyph taking the label's colour in each state; unknown icon names fall back to the plain `btn` — same key, same label, same handler |
| 4 | `src/chrome_view.rs` | Icons for the shell chrome this file owns (app-bar and transport buttons), over §3's builders |
| 5 | `src/rune_sec/home_panel.rs` | Icons for the Home panel's actions (incl. Phase G §32's reopen-last) |
| 6 | `src/rune_sec/project_tools.rs` | Icons for the project panel's actions (new/save/recover/open rows) |
| 7 | `src/rune_sec/figures_panel.rs` | Icons for the figures panel (row actions, filter row) |
| 8 | `src/rune_sec/figure_add.rs` *(brief)* | Icons for the add-figure panel's preset/blank choices |
| 9 | `src/rune_sec/library_panel.rs` | Icons for the library panel (save/load/rename/delete rows) |
| 10 | `src/rune_sec/layers_panel.rs` | Icons for the layers panel (visibility/lock/reorder/opacity) |
| 11 | `src/rune_sec/inspector_segment.rs` | Icons for the segment inspector (angle/bend steppers, node actions) |
| 12 | `src/rune_sec/inspector_figure.rs` | Icons for the figure inspector (rename, scale, colour) |
| 13 | `src/rune_sec/inspector_style.rs` | Icons for the style inspector (colour steppers, copy/paste style) |
| 14 | `src/rune_sec/pose_tools.rs` *(brief)* | Icons for the pose tools and pose library rows (apply/mirror/delete/rename) |
| 15 | `src/rune_sec/keyframe_tools.rs` | Icons for the keyframe tools (nudge, key-all, copy-to-frame) |
| 16 | `src/rune_sec/camera_panel.rs` | Icons for the camera panel (key list, follow block, rotation dial row) |
| 17 | `src/rune_sec/scene_panel.rs` | Icons for the scene panel (weather, stage presets, background steppers) |
| 18 | `src/rune_sec/onion_controls.rs` *(brief)* | Icons for the onion-skin toggle and depth steppers |
| 19 | `src/rune_sec/playback_controls.rs` | Icons for the transport-adjacent playback controls (restart, play-range, loop toggle) |
| 20 | `src/rune_sec/frame_ops.rs` | Icons for the frame ops (insert-N, duration row, range ops) |
| 21 | `src/rune_sec/graph_panel.rs` *(brief)* | Icons for the graph panel (channel list, interp, tangent actions) |
| 22 | `src/rune_sec/easing_panel.rs` *(brief)* | Icons for the easing curve picker |
| 23 | `src/rune_sec/draw_tools.rs` | Icons for the draw tools (brushes, stamps, shape tools) |
| 24 | `src/rune_sec/stroke_inspector.rs` | Icons for the stroke inspector (selection steppers, bulk ops) |
| 25 | `src/rune_sec/text_tool.rs` *(brief)* | Icons for the text tool (instance rows, nudge pad, resize) |
| 26 | `src/rune_sec/fx_browser.rs` *(brief)* | Icons for the FX browser (kind categories, filter, recents) |
| 27 | `src/rune_sec/fx_clips.rs` | Icons for the FX clips panel (nudge row, strip, clip rows) |
| 28 | `src/rune_sec/particles_panel.rs` | Icons for the particles panel (add, anchor, frame steppers) |
| 29 | `src/rune_sec/audio_panel.rs` + `waveform_view.rs` *(brief)* | Icons for the audio panel's per-clip rows and op buttons; and the waveform cleanup from §2 — one coordinate mapping for playhead and seek, retiring the clip-span/project split Phase G §10 left |
| 30 | `src/rune_sec/clips_panel.rs` | Icons for the movieclip panel (duplicate/rename/delete/reorder rows) |
| 31 | `src/rune_sec/three_d_panel.rs` *(brief)* | Icons for the 3D panel (session, camera, pose library rows) |
| 32 | `src/rune_sec/export_panel.rs` | Icons for the export panel (format bundles, history rows) |
| 33 | `src/rune_sec/stats_view.rs` + `status_strip.rs` *(brief)* | Icons for the stats surface and the status strip's segments where a glyph replaces a text label |
| 34 | `src/rune_sec/settings_popover.rs` + `help_panel.rs` *(brief)* | Icons for the settings card (guide toggles, startup workspace) and the help panel |
| 35 | `src/rune_sec/menus.rs` + `workspaces.rs` + `timeline_marks.rs` + `toasts.rs` *(brief)* | Icons for the remaining chrome surfaces: menu items, workspace chips, timeline marks row, toast actions (undo/dismiss) |
| 36 | `src/rune_sec/command_palette.rs` *(brief)* | Palette dispatch migration: `Command.run` moves from `fn(&mut EditorCore)` to State-level dispatch, unblocking the two commands Phase G §29 documented as skipped — toggle loop playback and stroke step-selection — which land with it |
| 37 | `src/audio_ops.rs` *(brief)* | Indexed DSP ops: Echo, Lowpass, Trim silence, Split silences, Pitch and Compress gain `_at(index)` forms in the Phase G §3 convention (status-only refusal, one snapshot on success), and the audio panel's buttons aim at the selected clip through them |
| 38 | `src/editor.rs` + `src/audio_ops.rs` neighbours *(brief)* | Two engine honest-signal fixes: `save_named_project` returns `bool` so a failed save writes no recovery snapshot and no recent (Phase G §12 item 3); `set_node_style_atomic` applies a full node style under one snapshot, making Phase G §20's style paste a single undo step |
| 39 | `src/rune_sec/snap_tests.rs` *(brief)* | Runtime tests round 5: this wave's surfaces driven headlessly — icon buttons press their real handlers, the migrated palette runs the loop/stroke commands, and the project panel's presented key set is asserted against Phase G §12's audit map (its integrator item 4) |
| 40 | `docs/PHASE_H.md` + `docs/QA_RUNE.md` §10 | This document |

## 4. Integration checklist (for the integrator)

Rune section wiring:

- [ ] Declare `pub mod icon_data;`, `pub mod icons;` and
      `pub mod icon_btn;` in `src/rune_sec/mod.rs` (§§2–3's
      files and the integrator's `icon_data.rs` have
      landed; none of the three is declared at snapshot
      time, while ~20 landed files already import
      `icon_btn`)

Library declarations:

- [ ] Register `pub mod icon_path;` in `src/lib.rs`, and
      confirm §1's `src/icon_path.rs` has landed —
      `src/rune_sec/icons.rs:48` imports `parse_path`,
      `Seg` and `path_bbox` from `stykanimate_rs::icon_path`;
      at snapshot time neither the file nor the
      declaration exists

Shell wiring (`src/main.rs`):

- [ ] Wire any call-site fallout from §36's palette
      dispatch migration (the `Command.run` signature
      change) exactly as §36's landed header prescribes

State:

- [ ] State literal fallout: **none expected** — Phase H
      adds no `State` fields (the palette migration moves
      a function-pointer type, not state). If a landed
      section declares a field in its header anyway,
      patch the section-test `State { … }` literals in
      the usual mechanical pass

Post-integration verification:

- [ ] `python3 scripts/rune_inventory.py` — count must be
      ≥ Phase G's final 556; icon adoption re-keys
      nothing deliberately (an icon button keeps its
      text button's key), so any drop against 556 is a
      regression to explain
- [ ] Push; read the **Build, Test & Binary** run; fix
      from CI logs only (repo rule: no local cargo)
- [ ] Fill the CI column of the QA_RUNE.md §10 addendum

## 5. Honest limits

- **Icon holes are unproven.** Compound-path rendering
  under Rune's `fill_path` has never been seen on screen
  (§1). If nested subpaths fill solid instead of cutting
  out, ring- and counter-style glyphs will read as blobs
  in the user's first screenshot of this build; the fix
  then is icon choice or a fill-rule change, and neither
  is in this wave.
- **Playback audio scrub-follow remains open.**
  Inherited from Phase F, named again by Phase G §2:
  seeking or scrubbing while playing still does not move
  the audio. No Phase H section claims it; a
  clock-synced player is a different piece of work.
- **Stage bend rendering remains open.** Phase G §19's
  steppers edit segment bends, but the stage still draws
  segments straight — bends are visible in exports only.
  No Phase H section teaches the stage `bend_mid`.
- **MCP parity for §37's indexed DSP ops is not in this
  wave.** The new `_at` forms land in the engine and the
  panel; the MCP registry does not gain them here, so
  the server keeps the last-clip surface until a later
  parity pass (the Phase F §38 pattern).
- **Palette rows stay text.** §36 migrates the palette's
  dispatch, not its rendering: command rows gain no icon
  column in this wave, even though the panels they mirror
  now have glyphs.
- **`--snap` on GPU-less CI remains unproven** (software
  Vulkan); the `headless.yml` workflow added in Phase E
  is still dispatch-only and has never been run. Phase H
  does not touch this.
- **Nothing in this wave verifies pixels or sound on a
  real screen.** The icon panels, the waveform mapping
  and the palette commands ship code and Runtime tests;
  how they look — icons above all — is provable only in
  the user's next screenshot, and the Phase G/H audio
  behaviour only by a Play press. The screenshot loop
  remains the proof, exactly as it was for Phases E–G.
- **Rune is pre-release** and pinned at rev `87caf2a`;
  nothing in Phase H changes the pin.
- Sections marked *(brief)* are recorded from the wave
  brief, not from landed headers, in this snapshot; their
  rows should be corrected against the files at
  integration time.

## 6. Inventory snapshot

`python3 scripts/rune_inventory.py`, last run 2026-11-08
~08:40 UTC (mid-wave, files still landing — treat as a
moving snapshot, not a final count):

- **488 unique keys** (420 static + 68 dynamic patterns),
  vs 556 at Phase G's integration.

The count sits *below* Phase G's final figure at snapshot
time, with panel files being edited concurrently; whether
it recovers as the remaining sections land is exactly
what the integrator's post-wiring run (§4) exists to
answer — a persisting drop is a regression to explain,
per the checklist. The integrator's run is the number of
record.

**Integrator's post-wiring run (the number of record):**
the mid-wave dip was a counting artefact, not a loss —
the script only recognised `btn`/`fbtn`/`row_toggle` as
key-carrying helpers, so every button converted to
`icon_btn`/`icon_only_btn` vanished from the count while
keeping its key in the app. With the script taught the
two new helpers, the integrated tree inventories **567
unique keys (480 static + 87 dynamic)** — a key-set diff
against Phase G's tree shows **zero keys lost** and 11
new (clip-rename-cancel, the three palette commands,
icon_btn's test fixtures, and keys the script can now
see through icon_btn).
