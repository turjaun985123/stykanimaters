# QA — Rune Phase D (GPUI→Rune migration verification)

Snapshot: 2026-11-08, taken while the Phase D section files were
still landing in `src/rune_sec/`. Method: **static reading only** —
no cargo was run (repo rule, `AGENTS.md`); CI is the compile
verdict, and the integrator fills the run ids in §4. Where this
doc says a control "calls" an op, the handler body was read in
the file cited. Where a file landed after the hand-audit pass,
it is marked PRESENT–NOT-AUDITED rather than described from
assumption.

**Re-audit pointer (Wave V, V18, 2026-11-10):** §§1–5 above
are the Phase D snapshot and are kept verbatim as the audit
trail — their present-tense claims ("the Phase C shell is
the only wired UI", "NOT YET RUN") describe 2026-11-08, not
the current tree. §§6–17 are per-phase closure maps whose
counts and CI cells are true as of their own phase. For
standing claims about the settled tree, read **§18**: a
full re-audit against the Wave U integration HEAD
(`1c7a70d`) that re-verifies every count, surface and
contract this doc carries, fills §4's open rows, and
supersedes §17's in-flight CI column.

## Headline findings

1. **The Phase C shell is the only wired UI.** `src/main.rs`
   (212 lines) declares exactly two modules — `chrome_view` and
   `stage_view`. **No `src/rune_sec/*` file is declared anywhere**
   (no `mod rune_sec`, no `mod` lines in `main.rs`/`lib.rs`), so
   none of the section files (35+ and still landing during
   this audit) are compiled into the binary yet, and none of
   their controls are reachable at runtime.
2. **Sections assume a richer `State` than the shell has.**
   Section handlers read/write fields that do not exist on the
   shell's `State { core, last_poll, wheel_zoom }` — e.g.
   `s.figure_add.open` (figure_add.rs:150), `s.figures_panel.
   collapsed` (figures_panel.rs:203), `s.graph_panel.target`
   (graph_panel.rs), `s.frame_ops` (frame_ops.rs), a
   `panel_state()` in clips_panel.rs. Integration must extend
   `State`, declare the modules, and mount the panels — that is
   the single remaining step between "written" and "live".
3. **At the control level, every GPUI-era family now has Rune
   controls written** (§2). At the binary level, only the
   Phase C subset is live: 17 keys (16 static + the `frame-{}`
   pattern) out of 357 script-extracted keys across the tree
   at the audit's final run (§1.3).
4. **Duplicate surfaces exist and need an integration
   decision:** `add-figure` and the onion toggle are built both
   in `chrome_view` (keys `add-figure`, `onion`) and in
   `figures_panel` / `onion_controls` (keys `add-figure`,
   `onion-toggle`, `onion-frames-dn/up`). Same ops, two homes —
   fine once mounted deliberately, confusing if both mount.
5. **Headless layout verification is designed but not yet
   runnable** (§3): `rune_sec/scenes.rs` defines four snap
   scenes and seven `--tree` assertions, but `AppSpec.scenes`
   is not registered in `main.rs`, and whether the `--tree`
   dump carries element rects (needed for the stage-height
   budget) is unverified — the scenes author asserts keys and
   counts only.

---

## 1. Command inventory

### 1.1 Phase C shell — WIRED (in the binary)

#### `src/main.rs` — stage + input + playback clock

| Control | Key | Calls |
|---|---|---|
| Stage canvas drag (pose mode) | `stage` | `core.begin_drag(p)` / `drag_to(p)` / `end_drag()` (hit-test inside `begin_drag`) |
| Stage canvas drag (camera mode) | `stage` | `core.begin_camera_drag(p)` / `drag_to` / `end_drag` |
| Stage canvas drag (draw mode) | `stage` | `core.begin_draw(p)` / `draw_to(p)` / `end_draw()` |
| Stage canvas wheel | `stage` | `core.camera_zoom_by(1.1^steps)` (sub-steps accumulated in `State::wheel_zoom`) |
| Playback poll | — | `core.advance_play(dt)` + `tx.wake_after(1/fps)` while `core.playing` |
| Key: Space | — | `chrome_view::toggle_play` (flips `core.playing`, syncs `core.playhead`) |
| Key: Left / Right | — | `step_frame` → `core.goto((cur ± 1) mod n)` |
| Key: Ctrl+Z / Ctrl+Shift+Z / Ctrl+Y | — | `core.undo()` / `core.redo()` |
| Key: Delete | — | `core.delete_segment()` |

#### `src/stage_view.rs` — no controls

Pure geometry + `Shape` building (`view_transform`, `to_screen`,
`to_stage`, `ngon`, `stage_shape`, onion-skin falloff). Unit
tests cover the transform roundtrip and `ngon`.

#### `src/chrome_view.rs` — app bar / transport / timeline / status

The `btn(key, label, active, theme, f)` helper: first argument
is the element key; every button below is built through it.

| Key | Label | Calls |
|---|---|---|
| `save` | Save | `core.save()` |
| `load` | Load | `core.load()` |
| `undo` | Undo | `core.undo()` |
| `redo` | Redo | `core.redo()` |
| `add-figure` | + Figure | `core.add_figure()` |
| `onion` | Onion | flips `core.onion` (field; no op exists — GPUI parity) |
| `prev` | Prev | `core.goto((cur + n − 1) % n)` |
| `play` | Play/Pause | `toggle_play` (flips `core.playing`; **audio is not started** — noted in the code as a later phase) |
| `next` | Next | `core.goto((cur + 1) % n)` |
| `add-frame` | + Frame | `core.add_frame()` |
| `dup-frame` | Duplicate | `core.duplicate_frame()` |
| `del-frame` | Delete frame | `core.delete_frame()` |
| `frame-{}` (pattern, 1-based) | chip number | `core.goto(i)` |
| `app-bar` / `transport` / `timeline` | row containers | — (structural keys for `--tree`) |
| status text (no key) | — | reads `core.current`, `frame_count()`, `project.fps`, `camera_here()`, `core.status` |

### 1.2 Section files — WRITTEN, NOT WIRED (`src/rune_sec/`)

Every file below exists and was read for this table; none is
compiled yet (finding 1). "Calls" names the `EditorCore` op or
core-field write in the handler body.

#### camera_panel.rs — `panel()` + stage `overlay()`

| Key | Calls |
|---|---|
| `view-fit` | `core.frame_all_figures()` |
| `view-centre` | `core.centre_on_selection()` |
| `view-actual` | zoom-to-1:1 via the file's `actual_factor` + `camera_zoom_by` path |
| `cam-zoom-out` / `cam-zoom-in` | `core.camera_zoom_by(1/ZOOM_STEP)` / `(ZOOM_STEP)` |
| `cam-rot-left` / `cam-rot-right` | `core.rotate_camera(∓ROTATE_STEP)` (±15°) |
| `cam-set-key` | `set_key_here` handler → core camera-key op |
| `cam-clear-key` | `core.camera_clear_key()` |
| `cam-follow` | `core.toggle_camera_follow()` |
| `cam-zoom-label`, `cam-angle-label` | readouts (`camera_here()` / angle) |
| `camera-panel`, `camera-overlay` | containers |

#### easing_panel.rs — `easing_panel()`

| Key | Calls |
|---|---|
| `ease-frame-default` | `core.set_frame_easing(None)` |
| `ease-frame-linear` / `-smooth` / `-ease-in` / `-ease-out` (match-arm keys) | `core.set_frame_easing(Some(e))` |
| `ease-project-cycle` | `core.cycle_easing()` |
| `ease-graph-smooth` / `-ease-in` / `-ease-out` / `-ease-in-out` / `-linear` (match-arm keys) | `core.apply_graph_ease_preset(p)` |
| curve previews | pure: `easing_curve` / `tangent_curve` → `Shape` (unit-tested endpoints) |

#### figure_add.rs — `figure_add_panel()`

| Key | Calls |
|---|---|
| `figure-add-toggle` | `toggle_open` — flips `s.figure_add.open` (shell state, **field missing from shell `State`**) |
| `fig-preset-humanoid` (entry data) | `apply_entry` → `core.add_figure()` |
| `fig-preset-quadruped` (entry data) | `apply_entry` → `core.add_quadruped()` |
| `fig-preset-mini` (entry data) | `apply_entry` → `core.add_simple()` |
| `fig-library-{i}` (entry data, pattern) | `apply_entry` → sets `core.library_index`, `core.insert_library_figure()` (bounds-checked in the section) |
| `figure-add`, `figure-add-card`, `figure-add-library`, `figure-add-presets` | containers |

#### figures_panel.rs — `figures_panel()`

| Key | Calls |
|---|---|
| `fig-collapse` | flips `s.figures_panel.collapsed` (shell state, **missing field**) |
| `add-figure` | `core.add_figure()` (duplicate of chrome's — finding 4) |
| `fig-sel-{fi}` | `core.select_figure(fi)` |
| `fig-lock-{fi}` | `core.toggle_figure_lock(fi)` |
| `fig-solo-{fi}` | `core.toggle_solo(fi)` |
| `fig-op-dn-{fi}` / `fig-op-up-{fi}` | `core.step_figure_opacity(fi, ∓0.1)` |
| `fig-eye-{fi}` | `core.select_figure(fi)` then `core.toggle_figure_hidden()` |
| `fig-vis-all` / `fig-vis-none` | `layers::set_all_visible(&mut core.project, true/false)`, clears `core.solo`, sets `core.status` |
| `fig-vis-inv` | `layers::invert_visibility(&mut core.project)`, clears `core.solo`, sets `core.status` |
| `fig-row-{fi}`, `figures-panel`, `figures-panel-header`, `figures-panel-footer` | containers |

#### layers_panel.rs — `panel()`

| Key | Calls |
|---|---|
| `layer-name-{fi}` | `core.select_figure(fi)` |
| `layer-op-dec-{fi}` / `layer-op-inc-{fi}` | `core.step_figure_opacity(fi, ∓OPACITY_STEP)` |
| `layer-down-{fi}` / `layer-up-{fi}` | `move_layer` → `core.select_figure(fi)` + `core.move_figure_full(dir)` |
| `layer-row-{fi}`, `layers-panel` | containers |

#### onion_controls.rs

| Key | Calls |
|---|---|
| `onion-toggle` | flips `core.onion` (duplicate surface of chrome `onion` — finding 4) |
| `onion-frames-down` / `onion-frames-up` | `core.onion_frames = stepped_onion_frames(core.onion_frames, ∓1)` (field write, clamped in the section) |

#### inspector_segment.rs — `inspector_segment()`

| Key | Calls |
|---|---|
| `insp-len-dn` / `insp-len-up` (stepper args — script blind spot) | `core.adjust_length(∓LENGTH_STEP)` |
| `insp-th-dn` / `insp-th-up` (stepper args — script blind spot) | `core.adjust_thickness(∓THICKNESS_STEP)` |
| `cycle-bend` | `core.cycle_bend()` |
| `node-shape` | `core.cycle_shape()` |
| `seg-color-{rgb:06x}` (swatch pattern) | `core.set_node_color(rgb)` |
| `node-color` | `core.cycle_node_color()` |
| `add-seg` | `core.add_segment()` |
| `del-seg` | `core.delete_segment()` |
| `inspector-segment` | container; readouts (Length/Thickness/Angle/Bend/Shape) are display-only |

#### inspector_style.rs — `style_panel()`

| Key | Calls |
|---|---|
| `style-taper` | `core.cycle_node_taper()` |
| `style-cap` (row) | `core.cycle_node_cap()` |
| `style-gradient` (row) | `core.cycle_node_gradient()` |
| `inspector-style`, `style-cap-row`, `style-gradient-row`, `style-taper-row` | containers |

#### scene_panel.rs — `scene_panel()`

| Key | Calls |
|---|---|
| `wx-seg-{}` (kind picker pattern) | `set_weather_kind` → writes `core.project.weather` (+ status) |
| `weather` | `cycle_weather` → cycles `core.project.weather` presets (+ status) |
| `wx-amt-dn` / `wx-amt-up` (stepper args) | `step_weather_amount` → weather amount field |
| `wx-wind-dn` / `wx-wind-up` (stepper args) | `step_weather_wind` → weather wind field |
| `stage-w-dn` / `stage-w-up` (stepper args) | `adjust_stage` → `core.project.stage[0]` |
| `stage-h-dn` / `stage-h-up` (stepper args) | `adjust_stage` → `core.project.stage[1]` |
| `bg-swatch-{}` (pattern) | `set_background` → `core.project.background = rgb` (+ status) |
| `scene-panel`, `weather-panel`, `wx-kind`, `bg-swatches` | containers |

#### waveform_view.rs — display only

`waveform_el()` → canvas keyed `waveform`; `waveform_shape`
draws clip peaks + playhead from the core. **No input
handlers** — it is a view, not a control surface (audio ops
live in audio_panel.rs).

#### frame_ops.rs

| Key | Calls |
|---|---|
| `inbetween` | `core.insert_inbetween()` |
| `shift-back` / `shift-fwd` | `core.shift_frames(cur, 1, ∓1)` (with fallback handling in the handler) |
| `reverse-range` | `core.reverse_range(from, count)` (args from the section's `reverse_args`) |
| `smooth-path` | `core.smooth_path_roots(selected_figure)` |
| `copy-frame` | `core.copy_frame()` |
| `paste-frame` | `core.paste_frame()` |
| `range-copy` | `core.copy_range(from, count)` (args use `s.frame_ops` — **missing shell field**) |
| `range-paste` | `core.paste_range(at)` path in handler |

Buttons render disabled from the section's `can_*` guards —
the GPUI grey-out behaviour, preserved.

#### keyframe_tools.rs

| Key | Calls |
|---|---|
| `key-angle` | `core.key_graph_angle()` |
| `key-delete` | deletes the graph key at the playhead (handler in file) |
| `key-auto` | `core.smooth_graph_tangents()` |
| `key-preset-{suffix}` (pattern) | graph ease preset at the key (`apply_graph_ease_preset` family) |

#### draw_tools.rs

| Key | Calls |
|---|---|
| `draw-mode` | toggles `core.draw_mode` (field) |
| `draw-tool` | tool cycle/select (handler in file) |
| `brush-cycle` | cycles `core.active_brush()` selection |
| `mirror-strokes` | `core.mirror_strokes_x()` |
| `simplify-strokes` | `simplify_current_frame` → draw-ops simplify on the current frame |
| `smooth-strokes` | `smooth_current_frame` → draw-ops smooth on the current frame |
| `clear-drawing` | `core.clear_frame_drawings()` |
| `draw-shape-line` / `draw-shape-rect` / `draw-shape-ellipse` | `draw_shape_button` → core shape-stroke ops |
| `stamp-poly` / `stamp-star` / `stamp-arrow` | `core.stamp_polygon()` / `stamp_star()` / `stamp_arrow()` |

#### graph_panel.rs

| Key | Calls |
|---|---|
| `graph-ch-{}` (channel pattern) | selects graph channel/target (`s.graph_panel.target` — **missing shell field**) |
| `graph-interp-hermite` / `-linear` / `-step` | `core.set_graph_key_interp(Interp::…)` |
| `graph-key` | `core.key_graph_angle()` |
| `graph-smooth` | `core.smooth_graph_tangents()` |
| `graph-mirror` | `apply_mirror` → core mirror-time on the effective target |
| `graph-scale-up` / `graph-scale-down` | `apply_scale` → core scale key values ×2 / ÷2 |
| `graph-reduce` | `apply_reduce` → core reduce-keys on the target |
| `graph-bake` | `core.bake_graph_channels(0, last)` |
| curve canvas press | `core.apply_graph_ease_preset(preset)` (per-key rows) |

#### audio_panel.rs

| Key | Calls |
|---|---|
| `audio-norm-{i}` (per clip) | `core.audio_normalize(i)` |
| `audio-split-{i}` (per clip) | clip split op at index (handler in file) |
| `audio-xfade-{i}` (per clip) | clip crossfade op at index (handler in file) |
| `audio-norm-last` | `normalize_last` → last-clip normalise |
| `audio-echo` | `core.audio_echo_last()` |
| `audio-lowpass` | `core.audio_lowpass_last()` |
| `audio-trimsil` | `core.audio_trim_silence_last()` |
| `audio-split-sil` | `core.audio_split_at_silences_last()` |
| `audio-pitch` | `core.audio_pitch_last()` |
| `audio-comp` | `core.audio_compress_last()` |
| `mixdown` | `core.mixdown_audio()` |
| `add-audio` | `add_clip` → core add-clip path (file-dialog backed op) |

#### clips_panel.rs

| Key | Calls |
|---|---|
| `make-clip` | `core.make_clip_from_figure()` |
| `insert-clip` | `core.insert_clip_figure()` |
| `clip-loop-loop` / `clip-loop-pingpong` / `clip-loop-once` | sets the selected clip's loop mode (via the section's `panel_state()` — **missing shell state**) |

#### library_panel.rs

| Key | Calls |
|---|---|
| `library-insert-{i}` (pattern) | `insert_at` → library figure insert at index |
| `library-collapse` | panel collapse (shell state) |
| `library-save` | save current figure to the library (handler in file) |

#### export_panel.rs

Rows are built from a data table of `(key, label, op)` and
dispatched through one handler (`op(&mut s.core)`), so the
keys below are table literals — the inventory script's
documented blind spot; enumerated here from the file.

| Key family | Calls |
|---|---|
| `xc-png`, `xc-jpeg`, `xc-svg` | image exports (current frame) |
| `xc-sheet`, `xc-gif`, `xc-apng` | animation exports |
| `xc-mp4`, `xc-webm` | video exports |
| `xc-v2`, `xc-v3`, `xc-v4`, `xc-v5`, `xc-stk`, `xc-nodes`, `xc-stknds` | project-format exports |
| `xc-range-toggle`, `xc-range-all`, `xc-range-loop` | frame-range selection for the export ops (shell state) |
| — | one literal `xc-bogus` exists in the file (test/fixture string, **not** a control — flagged so nobody counts it) |

#### three_d_panel.rs — helpers + `three_d_panel()`

Keys are held in local variables and passed to a local
button helper (script blind spot); the section's pure
dispatchers were read: `apply_camera_preset(core, name)`,
`apply_frame_camera(core)`, `apply_pose(core, name)`,
`apply_ik(core, limb)` (arm/leg chains), plus keys including
`mode-3d`, `panel-3d`, `orbit-3d`, `rotx-3d`, `joint-3d`,
`mark-b-3d`, `bake-3d`, `bake-anim-3d`, `cam-front-3d`,
`cam-side-3d`, `cam-top-3d`, `cam-three-quarter-3d`,
`cam-frame-3d`, `ik-arm-l-3d`, `ik-arm-r-3d`, `ik-leg-l-3d`,
`ik-leg-r-3d`. Full per-key op mapping: NOT hand-audited —
the integrator should verify each key's handler at wiring
time.

#### help_panel.rs — data, no controls

Ports the GPUI help data: `shortcut_rows()`, `section_rows()`,
`group_rows()`, `section_note()`; renders section containers
keyed `help-section-{}`. No EditorCore ops — it is the help
text surface; mounting is part of wiring.

#### i18n_wire.rs — no controls

Wiring data only: `lang_of(core)`, `tr(core, key)`,
`language_options()` (from `Lang::all()/code()/label()`), and
`shell_key_coverage()` / `missing_shell_keys()` splitting its
`SHELL_KEYS` list against the live i18n table. It deliberately
does **not** add keys to `i18n.rs`; missing shell keys are the
translation pass's work list.

#### scenes.rs — CI scenes (not UI)

`scenes()` returns four named state mutators for headless
runs: `default`, `frame-3`, `selected`, `playing`;
`layout_assertions()` lists the seven `--tree` facts in §3.
Not registered in `AppSpec` yet (finding 5).

#### status_strip.rs / toasts.rs / stats_view.rs — read in part

- `status_strip.rs`: a section-built status bar keyed
  `status-strip` — overlaps `chrome_view::status_text`
  (finding 4 class: pick one at wiring).
- `toasts.rs`: overlay container `toasts`, items
  `toast-{index}` — display/dismiss surface for `core.status`
  history.
- `stats_view.rs`: `stats-panel` / `stats-grid`, cells
  `stat-{}` — project statistics readouts (display only).

#### PRESENT–NOT-AUDITED files

These landed during this audit and are inventoried by the
script (§1.3) but their handlers were **not** hand-read;
their rows must be completed at integration, not assumed:

`command_palette.rs`, `fx_browser.rs`, `fx_clips.rs`,
`menus.rs`, `playback_controls.rs` (keys include `timecode`,
`tween`), `pose_tools.rs`, `project_tools.rs`,
`settings_popover.rs`, `snap_tests.rs`, `text_tool.rs`
(keys include `text-input`, `text-actions`, `text-list`,
`text-seg`, `text-size-up`, `text-size-down`, `text-tool`).

### 1.3 Mechanical inventory (scripts/rune_inventory.py)

`python3 scripts/rune_inventory.py` (stdlib only) extracts
element keys from `src/main.rs`, `src/stage_view.rs`,
`src/chrome_view.rs` and `src/rune_sec/*.rs` — patterns:
`.key("…")`, `.key(format!("…"))`, `btn("…")` / `fbtn` /
`row_toggle` first arguments (incl. `&format!` dynamic keys),
`btn(match …)` arm literals, and `key:` entry-data fields.
Documented blind spots (covered by hand in §1.2):
`stepper_row` dn/up positional keys, variable-held keys
(three_d_panel), and data-table keys (export_panel).

Runs during this audit (tree still landing between runs):
**337 unique keys** (291 static + 46 patterns, 39 files),
rising minutes later to **357 unique keys** (309 static +
48 patterns, 41 files). The number moves as sections land —
rerun the script for the current count; do not quote any
figure here as final.

---

## 2. Carry-over checklist vs the GPUI-era 248-id inventory

Baseline: **248 unique control ids** in the final GPUI tree
(`docs/QA_WAVE15.md` §1: 240 at `382dbce` — 196 plain + 29
tuple + 15 `xc-` — plus 8 added by Wave 15, zero lost). The
GPUI organisation was 24 tool groups across 8 workspaces.
Status below is **controls written** (all in `src/rune_sec/`,
none wired yet) vs **live in the binary** (Phase C shell only).

| Family | GPUI-era source | Rune controls written | Live in binary | Notes |
|---|---|---|---|---|
| Transport | Transport + Playback groups | ✅ chrome transport + playback_controls.rs | ✅ (prev/play/next) | Loop/tween readouts in sections; audio start on Play still missing (chrome comment) |
| Frames | Frames group, `copy-frame`/`paste-frame` | ✅ chrome chips/add/dup/del + frame_ops.rs (inbetween, shift, reverse, smooth, copy/paste frame+range) | ✅ partial (chips + add/dup/del) | Range ops written, not live |
| Figures | Figure/Figures groups, `fig-*` (13 ids) | ✅ figures_panel + figure_add + layers_panel | ⚠️ `add-figure` only | Sidebar rows, presets, library, layers all written |
| Segments | Segments + Segment Style groups | ✅ inspector_segment + inspector_style | ⚠️ Delete key only | Steppers/cycles/swatches written |
| Camera | Camera group, `cam-*` (5 ids) + view ops | ✅ camera_panel (fit/centre/1:1, zoom, rotate, keys, follow) | ⚠️ wheel zoom + camera drag | Panel + overlay written |
| Draw | Draw + Strokes groups | ✅ draw_tools + stage draw drag + text_tool.rs | ⚠️ drag path only (no UI sets `draw_mode`) | Brush/shapes/stamps written |
| FX | Effects + Effects 2 groups, `fxclip-*` (27 ids) | ✅ fx_browser.rs + fx_clips.rs | ❌ | Present, not hand-audited |
| Audio | Audio & Text + Audio Ops groups, `audio-*` (10 ids) | ✅ audio_panel (12 controls) + waveform_view | ❌ | Per-clip + last-clip ops + mixdown written |
| 3D | 3D group | ✅ three_d_panel | ❌ | Dispatchers read; per-key map pending |
| Export | Export + Formats groups, 15 `xc-` rows | ✅ export_panel (16 format keys + 3 range keys) | ❌ | Data-table dispatch; `xc-bogus` is a fixture literal |
| Projects | Projects + Import + Home groups, `home-*` | ⚠️ project_tools.rs present; shell save/load live | ⚠️ save/load only | Home/project-browser/recovery surface not hand-audited |
| Settings | Stage & Language group | ⚠️ settings_popover.rs + scene stage steppers + i18n_wire language data | ❌ | Language picker options exist as data; no mounted picker verified |
| Help | Help panel + `menu-help` | ✅ help_panel (shortcut/section/group rows) | ❌ | Data ported; mounting pending |
| Graph (extra) | Graph workspace, `graph-*` (10 ids) + interp buttons | ✅ graph_panel + keyframe_tools + easing_panel | ❌ | Beyond the 13 named families; included for completeness |
| Clips/Library (extra) | Library & Clips group, `clip-*` (7 ids) | ✅ clips_panel + library_panel | ❌ | Loop modes + make/insert written |
| Scene (extra) | Scene group (weather, stage, bg) | ✅ scene_panel | ❌ | Weather kind/amount/wind, stage size, bg swatches |
| Menus/Palette (extra) | Wave 15 `menu-*` (4 ids) | ✅ menus.rs + command_palette.rs | ❌ | Present, not hand-audited |

**Bottom line:** no GPUI family is without a Rune
counterpart on disk; the carry-over risk is no longer
*missing controls* but *unwired controls* — integration
(§5, gap 1) is what converts this table's "written" column
into the "live" column.

---

## 3. Layout-budget check via `--tree`

Budget (Studio directive): **stage height ≥ 360 px at
1280×720**, the window minimum (`AppSpec.min_size` in
`main.rs` is exactly `(1280.0, 720.0)`; initial size
1440×900).

### Method

1. Take the CI-built binary (no local builds — `AGENTS.md`).
2. Run headless tree dumps per scene:
   `./stykanimate-rs --tree tree-default.json --scene default`
   and likewise for `frame-3`, `selected`, `playing`
   (requires gap 1: `scenes.rs` registered in
   `AppSpec.scenes`).
3. Assert the seven facts from `rune_sec/scenes.rs ::
   layout_assertions()` — key/count facts the tree dump is
   known to carry:
   - exactly one element keyed `stage`;
   - elements keyed `app-bar`, `transport`, `timeline` exist;
   - the count of `frame-N` elements equals the project frame
     count (5 in the demo project: `frame-1`..=`frame-5`),
     each exactly once;
   - all facts identical under every scene.
4. **Stage-height assertion (the budget itself):** if the
   `--tree` JSON carries element rects, read the `stage`
   element's rect at the 1280×720 size and assert
   `height ≥ 360`. Whether rects are in the dump is
   **UNVERIFIED** — the scenes author asserts keys/counts
   only. Fallbacks, in order:
   a. `--snap snap.png --crop stage` at 1280×720 and assert
      the cropped PNG's height ≥ 360 (needs a wgpu adapter —
      the software-Vulkan question from
      `docs/RUNE_API_AUDIT.md` Q8, still open for GPU-less CI);
   b. arithmetic bound from the shell layout: stage height =
      720 − 2×16 root padding − 4×12 gaps − (app-bar +
      transport + timeline + status heights), with the chrome
      heights measured once from a `--snap` at any size.
5. Record the measured stage height per size (1280×720,
   1440×900) in §4 when the check first runs.

### Status

**NOT YET RUN.** Blocked on: scenes/wiring (gap 1), the
first real `--tree` dump (rect question), and the CI
headless adapter question. The check is designed and the
assertions are written in-repo (`scenes.rs`); nothing here
has been measured, and no stage height is claimed.

---

## 4. CI evidence (integrator fills)

| # | What | Commit | CI run | Result |
|---|---|---|---|---|
| 1 | Phase B: GPUI removed, Rune shell lands | `6f03dd4` | `42455193634` | ✅ GREEN — 701 tests (691 unit + 10 integration) |
| 2 | Phase C: interactive shell (stage/timeline/transport) | `7083701` | `42456311759` | ✅ GREEN — 708 tests (691 unit + 7 bin + 10 integration) |
| 3 | Phase D: `rune_sec` wired into the binary (mod decls, `State` fields, panel mounting, `AppSpec.scenes`) | `9f26d44` + fixes `75116de`, `66f4e46` | `42457752206` | ✅ GREEN — 1,020 tests (691 lib + 319 bin + 10 integration); 362 element keys inventoried |
| 4 | Headless `--tree` assertions (§3, facts 1–7) in CI | through `9d7dd0a` | `42765217055` (latest of a routine series) | ✅ RUNNING — the probe now dumps all 34 scenes per dispatch; the original seven facts grew into layout facts 1..=36, the overflow SCAN_SET (16 required trees) and the fail-soft subtitle-census + Home-marker transcripts, which are read, not just concluded (§18) |
| 5 | Headless `--snap` stage-height measurement (§3, step 4) | — | `42467268661` (first proof) | ✅ SUPERSEDED — the §3 rect question resolved itself: `--tree` dumps DO carry element rects plus a top-level capture `scale`, and two instruments (`overflow_scan.py`, `subtitle_census.py`) read the stage rect from them (seeded family 744×464 pt at the 1440×900 harness window; Phase N's tree-default stage measured 485 pt). The literal one-off 1280×720 ≥ 360 assertion was never run as designed; the budget is instead guarded on every headless run by the layout facts + overflow scan (§18) |
| 6 | First user-PC screenshot review of the Rune shell | — | — | ✅ DONE 2026-11-08 — the user ran the Phase E build on their PC (screenshot + screen video, §8's opening event); two defects it exposed were fixed in Phase F |

---

## 5. Gaps list

1. **Nothing in `src/rune_sec/` is wired.** No module
   declarations in `main.rs`/`lib.rs`; shell `State` lacks
   the section fields (`figure_add`, `figures_panel`,
   `graph_panel`, `frame_ops`, clips `panel_state()`, …);
   no panel is mounted in `view`; `AppSpec.scenes` does not
   register `scenes.rs`. Until this lands, the binary's live
   inventory is the Phase C subset only (§1.1).
2. **Duplicate control surfaces** to resolve at wiring:
   `onion` (chrome) vs `onion-toggle` (onion_controls);
   `add-figure` in both chrome and figures_panel;
   `status_text` (chrome) vs `status_strip.rs`. Same ops —
   the integrator picks one mount per function.
3. **Not hand-audited sections** (§1.2): command_palette,
   fx_browser, fx_clips, menus, playback_controls,
   pose_tools, project_tools, settings_popover, snap_tests,
   text_tool — plus the per-key op map of three_d_panel.
   Their controls are script-inventoried only.
4. **`--tree` rect availability unverified; `--snap` on
   GPU-less CI unproven** (software Vulkan). The §3 budget
   check cannot close until one of the two paths is proven.
5. **i18n is data-ready, not wired:** chrome labels are
   hardcoded English; `i18n_wire` lists shell keys missing
   from the 28-language table (`missing_shell_keys()`) as
   the translation pass's work list; no mounted language
   picker has been verified.
6. **Playback still starts no audio** (chrome
   `toggle_play` comment: audio start is a later phase) —
   the GPUI app started audio with playback; parity gap.
7. **Inventory script blind spots** (documented in the
   script): `stepper_row` dn/up positional keys,
   variable-held keys (three_d_panel), data-table keys
   (export_panel). §1.2 covers them by hand; a future
   script version should parse the `stepper_row` call
   shape.
8. **`xc-bogus` literal in export_panel.rs** — a test
   fixture string that key-shaped greps will keep
   surfacing; harmless, flagged so it is never counted
   as a control.
9. **Docs debt:** `README.md` still describes the GPUI
   build (architecture + touch sections). Not a code gap;
   update when the Rune shell is wired.

---

## 6. Phase E addendum — gap closure map

Appended 2026-11-08, mid-wave; CI column and the two
corrections below filled by the integrator after the green
run `42459754303` (commit `4035db6`): **1,218 tests passed,
0 failed** (733 lib + 475 bin + 10 integration). Phase E's charter is to close
the gaps listed in §5 above plus the Phase D delivery gaps
that §5 did not enumerate (audio engine ops, FX/figure/graph
engine ops, stage layers, timeline marks, workspace gating,
playback audio, timecode unification). Full charter and the
40-section table: `docs/PHASE_E.md`.

| §5 gap | Closing section(s) | Status | CI |
|---|---|---|---|
| 1. Nothing in `src/rune_sec/` wired | Closed by Phase D integration (`9f26d44`); Phase E builds on the wired shell | ✅ closed (Phase D) | run `42457752206` GREEN |
| 2. Duplicate surfaces: `onion` vs `onion-toggle`; `add-figure` ×2; `status_text` vs `status_strip` | figures_panel / onion_controls / status_strip sections | **still partially open (integrator correction):** the status surface is single (status_strip), but the app bar still carries its quick `add-figure` and `onion` buttons alongside the panel homes — the removal this row promised did not happen in Phase E | run `42459754303` GREEN |
| 3. Not hand-audited sections (command_palette, fx_browser, fx_clips, menus, playback_controls, pose_tools, project_tools, settings_popover, snap_tests, text_tool; three_d_panel per-key map) | Phase E upgrades touch fx_clips (§16), menus (§24), playback_controls (§20), settings_popover (§21), command_palette (§25) with in-file tests; the remaining files' hand-audit is still owed and is **not** claimed closed by this wave | partially closed — audit debt for fx_browser, pose_tools, project_tools, snap_tests, text_tool, three_d_panel per-key map remains open | run `42459754303` GREEN |
| 4. `--tree` rect availability unverified; `--snap` on GPU-less CI unproven | §28 scenes (one per workspace) gives `--tree` more to assert; the rect question and the software-Vulkan question themselves are **not** closable by section code | open — unchanged, restated honestly in PHASE_E.md §5 | — |
| 5. i18n data-ready, not wired; shell keys missing from the table | §27 i18n pass: 8 missing shell keys (`settings`, `frame`, `frames`, `figures`, `prev_frame`, `next_frame`, `fps`, `zoom`) added; labels routed through `i18n_wire::tr` | written | run `42459754303` GREEN |
| 6. Playback starts no audio | — | **still open (integrator correction):** no Phase E section wired audio start into the shell's Play; `playback_controls` contains no audio call. The engine still mixes audio into exports only | — |
| 7. Inventory script blind spots (stepper_row keys, variable-held keys, data-table keys) | Not a Phase E section; §1.2-style hand coverage continues. The script was re-run for Phase E (§7 below) without error | open — documented, unchanged | — |
| 8. `xc-bogus` literal in export_panel.rs | Fixture string, still present by design; still not a control | unchanged — not a gap | — |
| 9. Docs debt: README described the GPUI build | Closed outside Phase E — README was rewritten for Rune in Phase D (§39 of that wave) | ✅ closed (Phase D) | — |

Phase D gaps not enumerated in §5, closed by Phase E:

| Gap | Closing section(s) | Status | CI |
|---|---|---|---|
| No audio gain/fade/reverse/delete-clip ops (buttons honestly absent in Phase D) | §8 `audio_ops.rs` (engine) + §15 audio_panel buttons | written | run `42459754303` GREEN |
| FX clip edits inline in the panel | §9 `fx_ops.rs` (engine) + §16 fx_clips dispatch | written | run `42459754303` GREEN |
| Figure colour/duplicate as shell-local helpers | §10 `figure_ops.rs` (engine) + §17 inspector_figure dispatch | written | run `42459754303` GREEN |
| Graph copy-keys not built | §11 `graph_keys.rs` (engine) + §12/§18 panel surface, `key_clipboard` state | written | run `42459754303` GREEN |
| Style panel cycle-only; no direct setters | §7 editor ops + §13 inspector_style | written | run `42459754303` GREEN |
| No per-stroke delete | §7 `delete_stroke_at` + §14 stroke_inspector | written | run `42459754303` GREEN |
| Stage drew figures only (no weather/FX/rotation/guides/text) | §2 stage layers + §3 stage_fx + §4 stage_camera + §5 stage_text | written | run `42459754303` GREEN |
| Timeline without loop/key/camera marks | §6 timeline_marks (replaces chrome `timeline_row`) | written | run `42459754303` GREEN |
| No workspace gating in the Rune layout | §1 workspaces + §24 menus View entries + `Alt+1..8` in `on_key` | written | run `42459754303` GREEN |
| Guides toggle omitted (no guides existed) | §4 guides geometry + §21 settings toggle + `guides` state | written | run `42459754303` GREEN |
| Timecode floor-vs-round disagreement | §20/§22 single implementation | written | run `42459754303` GREEN |
| Project easing cycle-only | §7 `set_project_easing` + §19 easing_panel | written | run `42459754303` GREEN |
| No post-export record for the user | §26 export recap (`export_recap` state) | written | run `42459754303` GREEN |
| Help panel missing restored shortcuts | §23 help_panel | written | run `42459754303` GREEN |
| Command palette missing the new ops | §25 command_palette (+5 commands) | written | run `42459754303` GREEN |

## 7. Phase E inventory snapshot

`python3 scripts/rune_inventory.py` re-run 2026-11-08,
07:21 UTC, mid-wave (files still landing — treat as a
floor, not a final count): **395 unique keys** (338 static
+ 57 dynamic patterns), up from 362 at Phase D green
(run `42457752206`). Post-wiring count of record:
**408 unique keys** (348 static + 60 dynamic patterns) at
Phase E green (`42459754303`).

---

## 8. Phase F addendum — gap closure map

Appended 2026-11-08, mid-wave (sections still landing;
statuses are a snapshot — the integrator updates the CI
column after the green run). Phase F's charter and the
40-section table: `docs/PHASE_F.md`.

**The event that opens this phase:** on 2026-11-08 the user
ran the Phase E build on their own PC and sent a screenshot
+ screen video — the first real-screen verification of the
Rune shell (§4 row 6 is closable on its strength). The
screenshot proved workspaces + gating, the stage with onion
skins, and the Phase E surfaces on a real display; the video
showed playback actually playing. The same evidence exposed
two defects, which lead this map.

| Phase E remainder | Closing section(s) | Status | CI |
|---|---|---|---|
| **D1 — status-strip text overlap** at real window widths (new; from the user's screenshot + video) | §6 status_strip fix | written | run `42461893898` GREEN — 1,416 tests (742 lib + 664 bin + 10 integration) |
| **D2 — stray ".layout" row** in the project list (new; from the user's screenshot) | §7 home_panel fix | written | run `42461893898` GREEN — 1,416 tests (742 lib + 664 bin + 10 integration) |
| App-bar duplicate `add-figure` / `onion` buttons still present (§6 row 2 remainder) | §8 chrome_view dedup | written | run `42461893898` GREEN — 1,416 tests (742 lib + 664 bin + 10 integration) |
| Playback starts no audio (§6 row 6, still open after Phase E) | §16 `audio_play.rs` glue (new lib module: `start/stop_playback_audio`, `playback_audio_active` over the Wave 10 `audio::AudioPlayer`) + §17 playback_controls surface | wired — the integrator hooked `poll()` in main.rs to the playing flip (start on play, stop on pause, from every play/pause source), and **hearing it is user-verified only**: CI cannot prove sound. Player limits inherited (no loop sync, no scrub-follow) — see PHASE_F §5 | run `42461893898` GREEN — 1,416 tests (742 lib + 664 bin + 10 integration) |
| `duplicate_figure` / `delete_figure` do not remap FX-clip anchors or graph channels (pre-existing engine gap, documented in Phase E) | §37 figure remap fix | written | run `42461893898` GREEN — 1,416 tests (742 lib + 664 bin + 10 integration) |
| Hand-audit debt (§6 row 3 remainder): fx_browser, pose_tools, project_tools, snap_tests, text_tool, three_d_panel per-key map | Phase F sections rework fx_browser (§28), pose_tools (§22), text_tool (§27), three_d_panel (§31), snap_tests (§39) — each rework is a fresh read of that file; **project_tools has no Phase F section and its audit stays owed**. Not claimed fully closed | partially closed — project_tools audit + any per-key maps the reworks don't reach remain open | run `42461893898` GREEN — 1,416 tests (742 lib + 664 bin + 10 integration) |
| Stage fidelity vs the rasterizer: Rune stage drew flat first-colours (no gradients), no selection highlight for strokes, no FX gizmos, no frustum/safe frames, no text backing plates | §1 stage gradients, §2 stroke highlight, §3 FX gizmos, §4 frustum/safe frames, §5 text plates | written — geometry + tests only; **pixels unverified until the user's next screenshot** | run `42461893898` GREEN — 1,416 tests (742 lib + 664 bin + 10 integration) |
| MCP parity: engine ops added in Phase E (and Phase F §37) not all exposed as MCP tools | §38 mcp_tools parity pass | written | run `42461893898` GREEN — 1,416 tests (742 lib + 664 bin + 10 integration) |
| `--tree` rect availability unverified; `--snap` on GPU-less CI unproven (§5 gap 4) | Not closable by Phase F section code; `headless.yml` (Phase E) is still dispatch-only and has never been run | open — unchanged | — |
| Inventory script blind spots (§5 gap 7) | Not a Phase F section; §1.2-style hand coverage continues. Mid-wave inventory: **442 keys** (375 static + 67 dynamic) at 07:56 UTC vs 408 at Phase E green — floor, not final | open — documented, unchanged | — |

Phase F depth work with no Phase E gap behind it (new
surface, recorded so the inventory growth is explained):
workspace blurbs (§10), timeline loop drag (§11), palette
recents (§12), graph tangent drag (§13), camera key
copy/paste (§14), stage-size presets (§15), figure
thumbnails (§18), figure rename (§19), segment angle
steppers (§20), node RGB editor (§21), pose library (§22),
keyframe nudge/clear (§23), frame duration stepper (§24),
draw stamp controls (§25), stroke bulk ops (§26), text
list (§27), FX search/recents (§28), FX nudge (§29), clip
duplicate/rename (§30), 3D readouts (§31), export history
(§32), library rename (§33), layer opacity slider (§34),
easing picker (§35), action toasts (§36), menu hints (§9),
Runtime tests round 3 (§39).

## 9. Phase G addendum — gap closure map

Appended 2026-11-08, mid-wave (sections still landing;
statuses are a snapshot — the integrator updates the CI
column after the green run). Phase G's charter and the
40-section table: `docs/PHASE_G.md`.

**The remainders that open this phase are Phase F's own.**
Phase F went green (run `42461893898`, 1,416 tests) and
named its leftovers in its completion note: two stage
helpers landed tested but undrawn for want of a selection
state, one remap case its census found but did not fix, a
`blurb()` with no consumer, the audio player's documented
loop limit, and the one hand-audit still owed. This map
tracks exactly those, plus the two structural limits Phase
F's sections exposed along the way (last-clip-only audio
ops, the single guides flag).

| Phase F remainder | Closing section(s) | Status | CI |
|---|---|---|---|
| Stroke highlight geometry landed tested but **not drawn** — no stroke selection state existed for it to read (§8 stage-fidelity row) | §5 stage_layers stroke picking (writes `State::stroke_selected`, groundwork in `src/main.rs:99`) + §6 stage_view selection drawing + §7 stroke_inspector selection UI | wired | run `42463908847` GREEN — 1,646 tests (771 lib + 865 bin + 10 integration) |
| FX gizmo geometry landed tested but **not drawn** — no panel listed or selected `Project::particles` events (§8 stage-fidelity row) | §4 `particle_ops.rs` (new lib module; registration is the integrator's step) + §6 stage_view selection drawing + §8 particles_panel (new; writes `State::particle_selected`, groundwork in `src/main.rs:103`) | wired | run `42463908847` GREEN — 1,646 tests (771 lib + 865 bin + 10 integration) |
| `move_figure` (reorder) remaps nothing — particles, FX-clip entries, graph channels and `camera_follow` keep stale indices after a swap (`src/editor.rs:1655`; found by Phase F §37's census, not fixed there) | §1 figure_ops reorder remap (the `editor.rs` call is integrator wiring) + §18 layers jump reorder on the same path | wired | run `42463908847` GREEN — 1,646 tests (771 lib + 865 bin + 10 integration) |
| `Workspace::blurb()` has no consumer (Phase F §10 shipped the text only) | §11 workspaces blurb consumer (blurb line shown with the workspace row) | wired | run `42463908847` GREEN — 1,646 tests (771 lib + 865 bin + 10 integration) |
| Playback audio loop limit: the audio pass does not restart when the visual playhead wraps (documented in `src/audio_play.rs`; scrub-follow listed there too and **stays open** — not claimed by Phase G) | §2 audio_play loop-wrap restart (the `poll()` wrap-sync call is integrator wiring) | wired | run `42463908847` GREEN — 1,646 tests (771 lib + 865 bin + 10 integration) |
| Audio ops act on the **last clip only** — the panel surface drove the `_last` wrappers in `src/audio_ops.rs` | §3 audio_ops indexed clips (the `_at(index)` family: `audio_fade_at` `:83`, `audio_reverse_at` `:133`, `audio_set_gain_at` `:188`, `delete_audio_clip_at` `:245`, `audio_move_at` `:267`) + §9 audio_panel per-clip selection | wired | run `42463908847` GREEN — 1,646 tests (771 lib + 865 bin + 10 integration) |
| project_tools hand-audit still owed (§8 hand-audit row: the one file with no Phase F section) | §12 project_tools hand-audit | wired | run `42463908847` GREEN — 1,646 tests (771 lib + 865 bin + 10 integration) |
| `--snap` on GPU-less CI unproven (§8 row, unchanged there) | Not a Phase G section; `headless.yml` is still `workflow_dispatch`-only (`.github/workflows/headless.yml:48`, `:77–78`) and has never been run | open — unchanged | — |

Phase G depth work with no Phase F remainder behind it
(new surface, recorded so the inventory growth is
explained): settings guide toggles + startup workspace
(§13), camera follow picker (§14), timeline key markers
(§15), graph channel list (§16), figures filter (§17),
bend steppers (§19), style copy/paste (§20), figure scale
(§21), pose library delete/rename/mirror (§22), key-all +
copy-to-frame (§23), insert N frames (§24), onion depths
(§25, over the `State::onion_ui` groundwork), loop toggle
(§26), status strip segments (§27), toast dismissal
(§28), palette Phase G commands (§29), help shortcut
audit (§30), menu guide items (§31), home reopen-last
(§32), stats totals (§33), FX clip strip (§34), movieclip
delete/reorder (§35), 3D pose library UI (§36), scene
custom background (§37), text instance edit (§38),
waveform playhead + seek (§10), Runtime tests round 4
(§39).


## 10. Phase H addendum — gap closure map

Appended 2026-11-08, mid-wave (sections still landing;
statuses are a snapshot — the integrator updates the CI
column after the green run). Phase H's charter and the
40-section table: `docs/PHASE_H.md`.

**The remainders that open this phase are Phase G's own.**
Phase G's sections named their limits as they shipped:
§29 documented two palette commands it could not wire,
§9 and §20 recorded the audio and style caveats in their
own files, and §12's audit left two integrator items
behind. This map tracks exactly those. The phase's other
half — the Font Awesome icon adoption (§§1–35 of
PHASE_H.md: the 7.3.1 set in `assets/icons/fontawesome/`,
the curated 102-icon table in `src/rune_sec/icon_data.rs`,
the `icon_path` → `icons` → `icon_btn` pipeline) — has no
Phase G remainder behind it; it is new surface, recorded
in PHASE_H.md §3. One honesty note travels with it: Rune
`fill_path` compound-path (hole) rendering is not
pixel-verified — `src/rune_sec/icons.rs` says so in its
own header, the curated set was chosen solid so most
glyphs read either way, and the first proof is the user's
screenshot.

| Phase G remainder | Closing section(s) | Status | CI |
|---|---|---|---|
| Palette dispatch cannot reach shell state — `Command.run: fn(&mut EditorCore)`, so the loop-playback toggle and stroke step-selection commands were **documented skips** in §29 | §36 command_palette dispatch migration (State-level `run`) + the two skipped commands landing with it | wired | run `42465077559` GREEN — 1,697 tests (802 lib + 885 bin + 10 integration) |
| Six audio DSP ops act on the **last clip only** — Echo, Lowpass, Trim silence, Split silences, Pitch, Compress never got indexed forms (Phase G §3/§9 indexed fade/reverse/gain/move/delete only) | §37 audio_ops indexed DSP ops (the `_at(index)` family for the six) + audio_panel aiming them at the selected clip | wired | run `42465077559` GREEN — 1,697 tests (802 lib + 885 bin + 10 integration) |
| Style paste is **three undo steps**, not one — §20's documented caveat: `set_node_color` does not snapshot, `set_node_cap`/`set_node_color2`/`set_node_opacity` each do | §38 `set_node_style_atomic` (engine setter, one snapshot) + inspector_style paste routed through it | wired | run `42465077559` GREEN — 1,697 tests (802 lib + 885 bin + 10 integration) |
| `save_named_project` returns `()` — a failed save still writes a recovery snapshot and a phantom recent (§12 integrator item 3) | §38 `save_named_project` returns `bool`; the project_tools handler branches on it | wired | run `42465077559` GREEN — 1,697 tests (802 lib + 885 bin + 10 integration) |
| Waveform carries **two coordinate mappings** — the original `waveform_el` maps peaks/playhead into the last clip's span, Phase G §10's `waveform_el_state` maps them onto the project; the builders coexist | §27 waveform_view: the clip-mapped builder was **removed** (grep-proven zero callers), leaving the project mapping as the only one | wired | run `42465077559` GREEN — 1,697 tests (802 lib + 885 bin + 10 integration) |
| Project-panel **key-coverage test** owed — §12's audit map cannot be asserted from the panel file (Rune's `El` has no public tree-walk); §12 integrator item 4 assigns it to the Runtime suite | §39 snap_tests round 5 (presented key set for the project section asserted against the audit map) | wired | run `42465077559` GREEN — 1,697 tests (802 lib + 885 bin + 10 integration) |
| Playback audio **scrub-follow** (named open in `src/audio_play.rs` since Phase F; Phase G §2 closed the wrap case only) | Not a Phase H section | open — unchanged | — |
| Stage draws segments straight — Phase G §19's bend steppers edit values the stage does not render (bends are export-visible only) | Not a Phase H section | open — unchanged | — |
| `--snap` on GPU-less CI unproven (§9 row, unchanged there) | Not a Phase H section; `headless.yml` is still `workflow_dispatch`-only and has never been run | open — unchanged | — |

Phase H depth work with no Phase G remainder behind it:
the icon adoption itself (§§1–35 — the lib path parser,
the icon builder and house buttons, then panel-by-panel
glyph adoption across the shell's chrome and panels),
recorded section by section in PHASE_H.md §3. Two
boundaries of that work are stated there and repeated
here so they are not lost: MCP does not gain §37's
indexed DSP ops in this wave (parity is a later pass),
and the command palette's rows stay text — §36 changes
its dispatch, not its rendering.

## 11. Phase I — targeted remainders

| Remainder | Closing section(s) | Status | CI |
|---|---|---|---|
| Playback audio **scrub-follow** open since Phase F (Phase G §2 closed the wrap case only) | §1 audio_play (`playhead_seeked` + `sync_playback_audio_on_seek`; backward seeks already restarted via the wrap path) + integrator poll wiring | wired | 42467099217 |
| Stage draws segments **straight** — bends visible only in exports (Phase G §19) | §9 stage_view bend arcs (`segment_pieces` mirrors export's `bend_mid` construction; zero-bend prim-identical) | wired | 42467099217 |
| **MCP parity** for the indexed DSP ops missing (Phase H §37 landed the ops, no tools) | §2 mcp_tools (six tools; registry 71 → 77, total 82 → 88) | wired | 42467099217 |
| `icon_btn` has **no inert variant** — local wrappers (`pibtn`, `ictl`) and lost tints (frame_ops, keyframe_tools) | §11 icon_btn inert variants + §24 ictl deleted + §25/§26 tints restored; §23 pose_tools delegates except its active-tinted variant branch (documented partial) | wired | 42467099217 |
| Icon gaps: **fig-scale-dn text**, **mirror buttons on "copy"** ("compress" absent from the FA set) | §10 icon_data batch 2 (102 → 110) + §22 scale-dn + §§23/24 mirror glyphs | wired | 42467099217 |
| **Contact sheet** is whole-timeline only — the range family had one hole left | §8 export range pair + integrator's `xc-contact` row (the pair had no caller until the wiring — grep-verified) | wired | 42467099217 |
| Palette cannot jump **workspaces** or reach several shell toggles | §27 command_palette (+13 commands, 84 → 97) | wired | 42467099217 |
| **headless.yml never dispatched** — `--snap` on software Vulkan unproven since Phase E | Integrator dispatch during Phase I integration (run 42467268661 on the green tree) | **proven**: all 6 probes exit 0 on llvmpipe — 4 `--tree` scene dumps + 2 `--snap` PNGs; the default snap renders the full Phase I shell (icons, fig-measure, status runtime all visible in pixels) | 42467268661 |
| Two "New project" results disagree (panel blank vs Home demo) | Not a Phase I section — user's product call | open — unchanged | — |
| Rune `fill_path` hole rendering not pixel-verified; Phase F–I pixels + sound unverified on a real screen | Needs the user's screenshot + Play test | narrowed — the headless snap (run 42467268661) renders the shell and its solid icons correctly in pixels; sound and a real-screen screenshot are still the user's to confirm | — |

## 12. Phase J — targeted remainders

| Remainder | Closing section(s) | Status | CI |
|---|---|---|---|
| **Overlap 1** — the Keyframes header's "No joint selected" note paints across its Delete button (headless snap, run 42467268661) | §9 keyframe_tools (note in its own truncating row, `keyframe-no-joint-note`; Delete keeps `key-delete`, wording byte-identical) | wired — **pixel-proven**: headless run 42468895995 tree dump puts the note row (y 823.3, h 15) fully above the actions row (y 865.3) — no intersection | 42468754625 |
| **Overlap 2** — at 2400×1350 the right column's bottom rows crowd the status strip's tail (same snap) | §10 main.rs layout (root cause: all three columns carried `.grow(1.0)` — equal thirds starved the centre and the strip's floored tail painted over the right column; side columns now fixed 300 / 340, centre takes the rest) | wired — **pixel-proven**: headless run 42468895995 tree dump measures side-left 300, side-right 340, centre/strip 744; the strip ends at x 1072, 12 pt before the right column starts (1084) | 42468754625 |
| Playback audio **drift within one pass** — wrap/seek re-syncs fire on jumps only (named open in `src/audio_play.rs` since Phase F) | §1 audio_play drift sync (`DRIFT_TOLERANCE_FRAMES` 2.5; playback clock per start) + §10 poll wiring (wrap → seek → drift) | wired | 42468754625 |
| **Range family incomplete** — spritesheet and SVG whole-timeline only (Phase I §8 closed the contact sheet only) | §2 export range pair (`export_sheet_range`, `export_svg_range`) + §15 export_panel rows (`RANGE_KEYS` 7 → 9; range variants write to the full ops' destinations) | wired | 42468754625 |
| **Duplicate graph channels** list twice and resolve to the first (Phase I §7) | §16 graph_panel (selection by row index — `GraphPanelState.selected_channel`; a picked row draws and edits its own twin) | wired | 42468754625 |
| **Library Enter-to-insert** — Phase I recorded a filtered-index wrong-figure quirk | §25 library_panel (audit found the quirk never existed in code: no `on_submit` was wired, Enter inserted nothing; the new submit path resolves through the rows' real-index mapping from the start) | wired | 42468754625 |
| Two "New project" results disagree (panel blank vs Home demo) | Not a Phase J section — user's product call | open — unchanged | — |
| Head ring radius is a constant 17.0 everywhere | Not a Phase J section | open — unchanged | — |
| `core.onion_frames` / `cycle_onion_frames` orphaned engine API | Stays deliberately: the shell keeps asymmetric onion depths in State (`onion_ui.prev` / `onion_ui.next`, Phase G §25), which the engine's single symmetric count cannot express — wiring the shell to it would lose a feature | open — by design | — |
| Status strip's Audio segment has no glyph (Phase H width-maths refusal) | Not a Phase J section | open — unchanged | — |
| Sound unverified on a real screen — the headless snap covers pixels, not audio; Phase J's drift sync is a bounded restart (2.5-frame tolerance), not a sample-accurate clock | Needs the user's Play test on a real screen | open — unchanged | — |

## 13. Phase K — targeted remainders

| Remainder | Closing section(s) | Status | CI |
|---|---|---|---|
| Playback audio **drift prediction was duration-blind** — Phase J's sync charged every frame at the project rate, so a long duration-multiplier frame could trip the 2.5-frame tolerance on its own (named in PHASE_J §7) | §1 audio_play (`expected_frame_walk`: each frame costs its real `duration / fps`, speed-scaled, folding at loop/timeline end) + §2 main.rs poll (speed noted before the scaled advance) | wired | run `42471213022` GREEN — 2,153 tests (907 lib + 1,236 bin + 10 integration) |
| **Graph channels could not be deleted at all** — duplicable since Phase J, removable never | §3 graph_keys (`delete_channel`, one snapshot) + §4 graph_panel (`graph-chan-delete`; twin-aware via the effective index, pick repaired by `selection_after_delete`) + §5 mcp_tools (`delete_graph_channel`) + §31 keyframe_tools (range clear over `delete_keys_in_range`) | wired | run `42471213022` GREEN — 2,153 tests (907 lib + 1,236 bin + 10 integration) |
| **Project files could not be deleted** — the Home panel listed and measured them, nothing more | §14 home_panel (two-step arm/confirm, caches invalidated) + §15 shell (`delete_project_file` — refuses shell-owned names, missing path is `NotFound`) | wired | run `42471213022` GREEN — 2,153 tests (907 lib + 1,236 bin + 10 integration) |
| **Head-ring radius was a 17.0 literal in five copies** (rasterizer, SVG, stage, thumbnails, inspector bounds) | §6 model (`HEAD_RING_RADIUS`, model.rs:22) + §7 export (:300) + §8 svg (:177) + §34 stage_view (:669) + §10 figures_panel re-export (:214) + §11 inspector_figure re-export (:300) — one source, value unchanged. stage_fx's `GIZMO_RING_R` is a different ring (a particle anchor, not a head) and stays its own constant | wired | run `42471213022` GREEN — 2,153 tests (907 lib + 1,236 bin + 10 integration) |
| **Palette search recorded as substring-only** | §17 command_palette audit — **premise disproven**: `fuzzy_score` has matched by case-insensitive subsequence with quality scoring since Phase D, over title, key AND group; the proposed tiering would have broken group-only queries ("vew"). No behaviour changed; the facts are pinned by two tests | audited — nothing to close | run `42471213022` GREEN — 2,153 tests (907 lib + 1,236 bin + 10 integration) |
| Status strip's **Audio segment glyph** — Phase H's refusal rested on the squeezed ~461 pt layout; Phase J fixed the widths | §16 status_strip — **re-examined, refusal stands on current numbers**: at the strip's real 744 pt the glyph totals 593.75 pt of floors + the tail's 160 pt minimum = 753.75 > 744 (short by 9.75 pt; without the glyph 736 ≤ 744). The arithmetic is pinned by `audio_glyph_width_recomputation_at_744pt`, so the question re-opens loudly if a label, the icon or the width changes | re-examined — text-only stands | run `42471213022` GREEN — 2,153 tests (907 lib + 1,236 bin + 10 integration) |
| Sound unverified on a real screen — the headless snap covers pixels, not audio; the drift correction is a bounded restart, not a sample-accurate clock | Needs the user's Play test on a real screen | open — unchanged | — |
| Two "New project" results disagree (panel blank vs Home demo) | Not a Phase K section — user's product call | open — unchanged | — |
| `core.onion_frames` / `cycle_onion_frames` orphaned engine API | Stays deliberately: the shell keeps asymmetric onion depths in State (`onion_ui.prev` / `onion_ui.next`, Phase G §25), which the engine's single symmetric count cannot express | open — by design | — |
| **Full-loop-in-one-tick miss** — a loop completed inside one poll tick, landing on the previous frame (or one past it), is indistinguishable from no movement, so wrap/seek syncs never fire | Named since Phase I; no Phase K section closes it | open — unchanged | — |
| MCP (95 tools) vs palette (103 commands) are different surfaces | Parity noted, not closed — the sets overlap but are not identical | open — noted | — |

## 14. Phase L — targeted remainders

| Remainder | Closing section(s) | Status | CI |
|---|---|---|---|
| **Phase K §33's stop: no 3D preview existed**, so the ground grid had nowhere to draw — the panel's readouts were text, `fig3d::project` fed only the bake paths, and the design decision (main stage vs panel canvas) was owed | §3 fig3d (`grid_line_points` :183, `preview_camera` :223) + §1 fig3_preview (`preview_prims` :137; grid flag :106/:111) + §2 three_d_panel (canvas `panel-3d-preview` :571, drag-orbit via `orbit_degrees` :594, `d3-grid-toggle` :723) + §30 icon_data (`border-all`; table 111) + §27 scenes (`three-d`) — **panel-canvas branch taken**. Integrator correction: the engine defines no ground plane, so the preview offsets the grid and the ground shadow to the figure's lowest evaluated joint (under its feet) instead of leaving them on the y = 0 plane through a session figure's hips. The first three-d headless snap (run 42474407850) then showed the grid's projected lines painting past the panel onto the shell (the canvas does not clip its drawing); fixed by clipping every preview segment to the viewport inside `preview_prims` (2ba5e02, with a regression test), and the re-dispatch (run 42475260622) shows the grid contained in the panel | wired | CI run 42475046641 GREEN on 2ba5e02 — 2,297 tests (947 lib + 1,340 bin + 10 integration) |
| **Head-ring radius centralised but constant** — Phase K made 17.0 a single source; the ring could not be resized | §4 model (`head_ring_scale` :741, `head_ring_radius()` :806, sanitised at read) + §7 editor (`set_head_ring_scale` :1816, one snapshot) + consumers §8 export (:302), §9 svg (:177), §10 stage_view (:671), §11 inspector_figure (steppers :717; bbox :347), §12 figures_panel (fit :270/:316) + §13 mcp_tools (`set_head_ring_scale`, registry :3120) | wired | CI run 42475046641 GREEN on 2ba5e02 — 2,297 tests (947 lib + 1,340 bin + 10 integration) |
| **Full-loop-in-one-tick miss** — a loop completed inside one poll tick, landing on the previous frame (or one past it), was indistinguishable from no movement, so wrap/seek syncs never fired (named since Phase I) | §14 audio_play (`walk_wraps`; `sync_playback_audio_on_full_loop`; decision `full_loop_detected`) + integrator wiring (the poll call in main.rs beside the wrap/seek syncs, with the tick's previous frame) | wired | CI run 42475046641 GREEN on 2ba5e02 — 2,297 tests (947 lib + 1,340 bin + 10 integration) |
| **Palette recents died with the process** — the recents list was shell memory only | §15 shell (`LayoutPrefs::palette_recents` :275; `palette_recents` / `save_palette_recents` :333) + §16 command_palette (merge on open :1154, write-back via `record_recent`) | wired | CI run 42475046641 GREEN on 2ba5e02 — 2,297 tests (947 lib + 1,340 bin + 10 integration) |
| Graph channels could be deleted (Phase K) but a key's **interp mode** was reachable only at the playhead, on one channel | §29 graph_keys (`set_key_interp` / `key_interp_at` by index) + §32 keyframe_tools (`key-interp` cycling Hermite → Linear → Step) | wired | CI run 42475046641 GREEN on 2ba5e02 — 2,297 tests (947 lib + 1,340 bin + 10 integration) |
| MCP total 95 → **98 tools** (registry 84 → 87: the settings family, incl. §13's head-ring tool); palette 103 commands unchanged; scenes 8 → 10; help 42 → 48 rows; icons 110 → 111 | §§13/16/27/28/30 | wired | CI run 42475046641 GREEN on 2ba5e02 — 2,297 tests (947 lib + 1,340 bin + 10 integration) |
| Status strip's **Audio segment glyph** refusal (Phase K §16: 753.75 pt > 744 pt, test-pinned) | Not a Phase L section — §31's speed segment faces the same arithmetic and fits it by existing only off 1.0× (below) | open — unchanged | — |
| **Status-strip speed segment** — the strip shows no playback-speed segment | §31 status_strip (`speed_segment`: `None` at exactly 1.0× and for non-finite speeds, else `"{x}×"`; composed by `status_strip_state` after the play segment) — landed after the docs census; the integrator verified the section in the tree and rewrote this row. The default strip is byte-identical, so Phase K's pinned floors stand | wired | CI run 42475046641 GREEN on 2ba5e02 — 2,297 tests (947 lib + 1,340 bin + 10 integration) |
| **Runtime test round 9** | §37 snap_tests (31 → 35 tests: pb-speed cycle, fig-ring-up, timeline loop row set/clear, ws-3d preview presence) — landed after the docs census; the integrator verified the round in the tree and rewrote this row | wired | CI run 42475046641 GREEN on 2ba5e02 — 2,297 tests (947 lib + 1,340 bin + 10 integration) |
| **3D figure on the main stage** — Phase K §33's other design branch | Not a Phase L section: the preview is panel-canvas only (§§1/2/3); the stage branch remains unbuilt | open — unbuilt | — |
| Sound unverified on a real screen — the headless snap covers pixels, not audio; the sync corrections are bounded restarts, not a sample-accurate clock | Needs the user's Play test on a real screen | open — unchanged | — |
| Two "New project" results disagree (panel blank vs Home demo) | Not a Phase L section — user's product call | open — unchanged | — |
| `core.onion_frames` / `cycle_onion_frames` orphaned engine API | Stays deliberately: the shell keeps asymmetric onion depths in State (`onion_ui.prev` / `onion_ui.next`, Phase G §25), which the engine's single symmetric count cannot express | open — by design | — |
| MCP (98 tools) vs palette (103 commands) are different surfaces | Parity noted, not closed — the sets overlap but are not identical | open — noted | — |

## 15. Phase M — targeted remainders

| Remainder | Closing section(s) | Status | CI |
|---|---|---|---|
| **3D figure on the main stage** — Phase K §33's other design branch, open since Phase L (the preview was panel-canvas only) | §1 stage_fig3 (`stage_fig3_prims` :91 — session camera through the project stage size, ground at the figure's feet via `fig3_preview::ground_y_of`, every segment clipped to the stage rect through Phase L's `clip_segment_to_viewport`) + §2 stage_view (`StageOverlays::fig3_stage` :366, default false :388; the pass at :742 requires the flag AND a live session) + §3 fig3d (`frame_camera_points` :277) + §32 scenes (`three-d-stage`) — **stage branch taken as a complement; the panel preview is unchanged**. The flag's caller was the integrator's wiring (PHASE_M §5): main.rs now sets it for the Build3D workspace with a live session | wired | CI 42478556459 ✓ |
| **Audio clips could not be muted** — a clip could be gained, faded and normalised, never silenced without deleting it | §5 audio (`AudioClip.muted`, serde-defaulted; `playback_plan` filters muted clips at :110 — never sounds directly, never joins a mix) + §8 audio_ops (`audio_set_muted_at` :209 + `audio_toggle_muted_at`; a no-change press takes no snapshot) + §13 audio_panel (`audio-mute` :833, label/glyph naming the press's action; muted readout under the clip rows) + §23 waveform_view (muted bars at `MUTED_BAR_ALPHA` :173, bracket dimmed :179, caption "Clip {n} · muted" :175) + §19 mcp_tools (`audio_set_muted`) + integrator: the export `mux_plan` excludes muted clips too (regression test in export.rs) | wired | CI 42478556459 ✓ |
| **Strokes could not be translucent** — brush opacity was model data the pipeline did not apply | §5 model (`Stroke.opacity` :516, `#[serde(default = "default_stroke_opacity")]` :515) + §9 editor (`recorded_stroke_opacity` :104, clamped [0.0, 1.0], NaN reads opaque; recording sites :2599/:2825) + §7 export (rasterizer applies it :424) + §10 svg (`stroke_opacity_attr` :32 — attribute only below 1, so opaque strokes' files are byte-identical) stage_view applies the StrokePrim opacity in its base Polyline/Filled conversions. The stroke_inspector steppers landed (§34 of PHASE_M: `stroke-opacity-row`, [0.05, 1.0] clamp, spray refused), and the translucent preset is Pencil converted to 0.55 (§11 — appending an eleventh preset was unsafe; Spray's 0.5 stays mark-only) | wired | CI 42478556459 ✓ |
| **FX clips could not be hidden** — a clip could be recoloured, moved and split, never taken out of the render while staying in the project | §5 model (`FxClipEntry.hidden` :712, serde-defaulted) + §6 stage_fx (hidden entry skipped whole — no motes, no trail :180) + §7 export (skip at :476, pinned by `hidden_fx_clip_renders_nothing` :1743) + §12 fx_clips (`fx-hide` toggle via `toggle_selected_hidden` :938; readout appends ` · hidden` :324; hidden clips stay editable). The gizmo still draws for a selected hidden clip — verified in §6: the anchor path never samples particles | wired | CI 42478556459 ✓ |
| **Two frames could not change places** — insert/duplicate/delete existed, but swapping meant hand-moving every frame-pinned collection | §14 frame_swap (`swap_frames` :116 + `swap_frame_with_neighbour` :183; the remap census in the module docs — camera keys, particles, strokes, audio starts, fx starts (exact-frame), graph keys, text/sprite instance keys — with ascending-order lists re-sorted after the move) + §15 frame_ops (`fop-swap-prev` :607 / `fop-swap-next` :626, live/inert pair gated by `swap_prev_enabled` :240 / `swap_next_enabled` :248) + §20 palette commands + the Edit-menu swap item (menus) | wired | CI 42478556459 ✓ |
| **Graph keys could not shift as a block, and a loop range could not be filled** — keys moved one at a time or not at all | §16 graph_keys (`shift_keys` :491 — the whole shift refused, never clamped per key; `fill_keys_in_range` :544) + §17 graph_panel (`graph-shift-l` / `graph-shift-r` over the drawn channel) + §18 keyframe_tools (`key-fill-range`, guard: loop range set AND a channel resolved) | wired | CI 42478556459 ✓ |
| **MCP parity batch** — shell ops with no MCP surface (per-key interp, particle kind, particle colour, audio mute) | §19 mcp_tools (`set_key_interp`, `particle_set_kind`, `particle_set_color`, `audio_set_muted` — each dispatching to the op the shell already had). The fifth briefed tool, `fx_set_color2`, already existed, so four tools were added: registry 87 → **91** (completeness pin raised to match; the census grep's 92 matched a non-registry occurrence), total **102** (registry + 3 built-ins + 8 IO). Scenes 10 → 11 (§32's pin) | wired | CI 42478556459 ✓ |
| **No framing camera for the 3D session** — the preview camera framed the rest pose only; a posed figure could sit outside the panel | §3 fig3d (`frame_camera_points` :277 — the new point-cloud framer; `frame_camera` :363 is the older figure framer, untouched) + §4 three_d_panel (`d3-frame` :867; zoom steppers `d3-zoom-dn` / `d3-zoom-up` through the pure `zoom_distance`) | wired | CI 42478556459 ✓ |
| **SVG range export rendered the whole timeline into a scratch dir and copied the range out** (Phase J §2's noted follow-up) | §10 svg (per-frame writer extracted: `write_frame_svg` :61; direct `export_svg_range` :293 in svg.rs — byte-identical files, no scratch dir). §22: the integrator rewired export.rs's `export_svg_range` onto the direct writer — a one-line delegation, the scratch-dir body deleted | wired | CI 42478556459 ✓ |
| **Palette growth 103 → 108 commands** | §20 command_palette (`mute-audio-clip`, `swap-frame-prev`, `swap-frame-next`, `shift-channel-right`, `fill-keys-range` — shims onto the landed ops; the count pin reads 108) | wired | CI 42478556459 ✓ |
| **Timeline clips lane** (`timeline-clips`) — the timeline showed frames, markers and the loop row, not the project's clip layout | §24 timeline_marks (lane row above the loop row, present only when clip spans exist; pure `clip_spans` / `span_rect` in the chips' geometry; muted/hidden spans dimmed to 0.35) | wired | CI 42478556459 ✓ |
| **Playback speed was cycle-only in the settings popover** — the chips existed in playback_controls and the View menu, not where settings live | §25 settings_popover (direct-select speed row :113–119 — one chip per speed, `speed_chip_label`, the current speed's chip active) | wired | CI 42478556459 ✓ |
| **Home rows did not show file size** — the Phase J metadata line had figures, frames, fps and modified time, not size | §26 home_panel (the row line gains the file's byte length through project_tools' `human_size`, imported :198; size line :559) | wired | CI 42478556459 ✓ |
| **Text instances could not change draw order** — texts drew in list order with no way to raise or lower one | §27 text_tool (`text-front` moves the loaded instance to the END of the list, `text-back` to index 0 :184–186; one snapshot per press) | wired | CI 42478556459 ✓ |
| **The figures panel could add and delete but not duplicate** | §21 figures_panel (`fig-duplicate`, copy glyph, beside Recolour :91–93 — through the remapping duplicate op) | wired | CI 42478556459 ✓ |
| **"Insert clip" always inserted clips[0]** — a per-row insert of the selected clip did not exist | §28 clips_panel (`insert_clip_figure_at` — the engine construction parameterised with the row's own index; the clips vec untouched, so no stored index shifts and neither selection nor rename target needs repair) | wired | CI 42478556459 ✓ |
| **Stats showed media totals but no flag state** | §29 stats_view (Flags block: `flag_totals` — muted clips, hidden FX, translucent strokes counted as captioned muted rows :426/:437) | wired | CI 42478556459 ✓ |
| **Range video exports carry no audio, and nothing said so** | §30 export_panel (`xc-range-audio-note`, decision in `range_audio_note_shown` :25–33 — the note states it where the range exports are offered; the full MP4 export still muxes audio) | wired | CI 42478556459 ✓ |
| **Status-strip 3D segment** — the strip names project, frame, speed and selection, not a live 3D session | §31 status_strip (`fig3_segment` — text-only "3D" after the speed segment while a session is live; absent otherwise, Phase K floor arithmetic untouched) | wired | CI 42478556459 ✓ |
| **Help refresh 48 → 54 rows** | §33 help_panel (six Features rows for the phase's surfaces; the count pins read 54) | wired | CI 42478556459 ✓ |
| Sound unverified on a real screen — the headless snap covers pixels, not audio; the sync corrections are bounded restarts, not a sample-accurate clock | Needs the user's Play test on a real screen | open — unchanged | — |
| Two "New project" results disagree (panel blank vs Home demo) | Not a Phase M section — user's product call | open — unchanged | — |
| Range MP4/WebM exports carry no audio (the range path renders video only) | Not closed by Phase M — the panel now warns (§30 above); closing it means muxing range audio, a separate piece of work | open — warned in panel | — |
| `core.onion_frames` / `cycle_onion_frames` orphaned engine API | Stays deliberately: the shell keeps asymmetric onion depths in State (`onion_ui.prev` / `onion_ui.next`, Phase G §25), which the engine's single symmetric count cannot express | open — by design | — |
| Status strip's **Audio segment glyph** refusal (Phase K §16: 753.75 pt > 744 pt, test-pinned) | Not a Phase M section — the measured refusal stands | open — unchanged | — |
| MCP (102 tools) vs palette (108 commands) are different surfaces | The gap narrowed from opposite directions this phase; the sets overlap but are not identical — parity of counts is not parity of sets | open — noted | — |

## 16. Phase N — targeted remainders

| Remainder | Closing section(s) | Status | CI |
|---|---|---|---|
| **Panels were flat** — every panel a bare `col()` with a hand-rolled heading: no card fill, no border, no padding; at least five private `section()` / `section_label()` helpers with different looks (charter groundwork audit) | F-wave foundation: NEW `src/rune_sec/design.rs` — the "card + hairline" kit built on the `rune::tok` tokens the shell barely used (`panel_col` :102, `header_row` :119, `panel_title` :130, `section_label` :148, `divider` :156 + `divider_keyed` :163, `kbd` :173, `empty_state` :192, `list_row` :223 + `row_shell` :230, `toggle` :273 + `toggle_row` :283, `stepper_value` :305, `slider_track` :345, `swatch` :381, `text_muted` :92; consts PANEL_RADIUS 12 / PANEL_PAD 12 / PANEL_GAP 8 / ROW_RADIUS 8 / CONTROL_RADIUS 6). Charter-fixed API, verified item-for-item against the landed file by the kit audit | wired | CI 42482498164 ✓ |
| **Three button implementations** — `chrome_view::btn`, the icon_btn family, and per-file dialects (frame_ops' `fbtn`, keyframe_tools' `kbtn`, pose_tools' `pbtn`/`pibtn`, three_d_panel's `ctl`/`ictl`, figures_panel's `row_toggle`) plus two drifted local `danger_btn` copies, so muted/inert handling drifted per file | F-wave family additions (charter amendments, P5): `chrome_view::btn_inert` :87 (no handler), `btn_muted_live` :116 (muted visual, handler KEPT — refusal statuses still fire), `danger_btn` :171 on the named consts `DANGER_FILL 0xC93A44` :146 / `DANGER_HOVER 0xB2323E` / `DANGER_PRESS 0x9C2934` (P12's measured fix: white on the old 0xF16F78 measures 2.88:1, the deepened red ≈5:1); `icon_btn::chip_btn` :371 + `danger_icon_btn` :420; `PRESS_SCALE 0.97` (icon_btn.rs:101, chrome btn likewise) on the live builders only (P15 M1 — the inert twins carry none) | wired | CI 42482498164 ✓ |
| **Shared surfaces predated the kit** — menus, palette, settings, workspace chips and the status strip each had their own chrome; the palette border was a WHITE.alpha(0.10) literal | F-wave restyles onto the kit: menu separators are `design::divider_keyed` (menus.rs:686 — the separators are keyed), dropdowns/palette/settings enter on `pair::SNAP` springs (menus.rs:751, command_palette.rs:1581, settings_popover.rs:692; P15 M2), toasts slide on `pair::SETTLE` (toasts.rs:490; M3); the palette's border is `t.card_border` and its selected row keeps the accent 0.85 fill (P7 ruling — launcher focus is not list selection); status-strip numbers take `numeric_font()` (status_strip.rs:804, MONO_CAPTION + `.mono()`) for the timecode and speed segments only, dividers are 1×10 `t.separator` ticks, and the Phase K width floors stand untouched | wired | CI 42482498164 ✓ |
| **Toast severity was built but never used** — `toasts::Severity` had Info/Success/Warn/Error styling since Phase F, but main.rs `poll` pushed every toast as `Severity::Info` | F7 toasts: pure `classify_status(status)` (toasts.rs:198 — Error = contains " failed", Warn = "No "/"Nothing " refusals, Success = "Exported "/"Saved ", else Info; patterns grep-verified against real engine strings) + the one integrator poll line (main.rs:333, `toasts::classify_status(&s.last_status)`). No status string changed | wired | CI 42482498164 ✓ |
| **Mount order buried the working panels** (P16) — Animate's right column ran ≈3,682 pt (Playback and Frames under Camera + the 770 pt Scene panel); Effects' first screen was all Scene panel, the FX browser below the fold; Build3D's 2D Camera panel preceded the 3D panel | Integrator-owned main.rs child-order moves at foundation integration (no agent edits main.rs): Graph (main.rs:623) before KeyframeTools (:626); ThreeD (:629), Playback (:632) and FrameOps (:635) promoted above Camera (:638); Scene (:695) demoted below the Particles (:662) / Audio (:665) block, above Export | wired | CI 42482498164 ✓ |
| **The waveform was a bare canvas, the stage radius was 8, and no hairline separated the chrome rows** (charter §§13–15) | Integrator items: the waveform mounts in `design::panel_col("waveform-card", …)` (main.rs:674) with a `wave-square` panel_title + divider, the canvas kept at its exact [308.0, 56.0] inside the card's 314 pt inner width (P4 fit ruling); the stage canvas radius is 12.0 (main.rs:733); the root column carries an unkeyed `design::divider` under the workspace row (main.rs:802); the app bar gained the accent dirty dot on `edits_since_autosave() > 0` | wired | CI 42482498164 ✓ |
| **The curated icon table lacked the glyphs the plans assigned** — P13's Tier B conversions and the layers Top/Bottom jumps had no curated names to call | F4 icon_data: 111 → **163** names (52 added, each verified present as an official Font Awesome Free 7.3.1 asset with a single path; the count pin reads 163 at icon_data.rs:356) | wired | CI 42482498164 ✓ |
| **31 panel files still hand-rolled their roots, headers, steppers, rows and swatches** after the foundation landed | A-wave adoption (commit a570d60): every adopted root is a `panel_col` card with a kit header + divider; steppers in charter order (label grows, −, `stepper_value`, +; the "−" is U+2212); list rows on `list_row`/`row_shell` with the kit selected fill (accent 0.16 — figures_panel's 0.22 retired); swatches on `design::swatch` (24 px); empty states on `empty_state` where a plan assigned a glyph; the local dialects deleted (census in the gate row below). Judgment calls recorded in the phase docs: easing picker cells and graph preset chips keep their drawn-canvas content (restyle-not-replace — `chip_btn` has no content slot); style preset chips lost their 12×12 colour squares to `chip_btn` (the preset names state their colours); scene's wind slider keeps its bipolar visual (ratified exception) while its SLIDER_H copy grew 22→28 with layers_panel's. stage_view audited — pure canvas geometry, no element chrome, file untouched; fig3_preview likewise geometry-only, its card is three_d_panel's | wired | CI 42484494999 ✓ |
| **Census artefact: the key total moved 808 → 804 across the adoption** — read naively, four keys lost | Not a loss, source-verified both halves: inspector_style's two channel-row builders (templates `style-{lower}-…` and `style-{stem}-…`) merged onto the surviving `{stem}` template, which renders the identical keys (style-r-dn, style-g-dn, …); and `pose-copy-to-{}` is now bound to a `let` variable before the btn call — a documented census blind spot (variable-passed keys), the format string and rendered key byte-identical. The gate's per-file diff vs the foundation tree: zero keys lost in every file | noted | CI 42484494999 ✓ |
| **Draw, Audio, Export and Home workspaces had no headless scenes**; the overlay surfaces (palette, menus, settings) had no pixel proof at all (P17) | V-wave scenes: `scenes()` 11 → **15** — `palette-open` (via the pub `command_palette::open_palette`), `menu-open` (via `MenusState::toggle(FILE)`), `settings-open` (via the pub `settings_popover::toggle_open`), `draw-open` (via `select_workspace(Draw)`), appended at table positions 11–14 (pinned scenes.rs:648–651) with layout facts 15–18. An fx-browser scene was rejected (no open state exists); the browser is pixel-proven by the fx-selected / fx-split-mid snaps instead | wired 
| **Runtime test round 11** — rounds 1–10 predate the card restructure; nothing proved the adopted surfaces still mount, stay reachable and do not overlap | V-wave snap_tests: 39 → **49** tests (ten new: `animate_panel_card_roots_present`, `panel_card_rects_stay_inside_their_columns`, `workspace_card_roots_sweep`, `edit_menu_dropdown_renders_after_restyle`, `palette_open_card_renders_and_closes`, `settings_card_renders_after_restyle`, `timeline_card_keeps_chips_marks_and_rows`, `stage_and_shell_furniture_survive`, `fx_browser_and_draw_roots_present`, `help_panel_renders_via_help_menu` — P17's nine firm plus the help conditional). Geometry is asserted as relations (containment, ordering, non-overlap), never new absolute pins: the stage height is the one number the phase provably moves — measured 522 → **485 pt** on the final headless tree-default (stage rect [328, 157, 744, 485]: the §15 hairline + root spacing ≈13 pt and the timeline card ≈18 pt, absorbed by the stage as the centre column's grow child). The second conditional (an empty-state key test) was not written — the kit `empty_state` carries no key and P17 forbade inventing one; empty states are snap-proven | wired | CI 42488645714 ✓; headless 42488647081 ✓ |
| **The gate could not see adoption** — checks 1–5 diff key sets and wiring, but a panel could keep every key and never touch the kit | V-wave gate **check 6 — DESIGN ADOPTION** (scripts/phase_j_checks.py): kit self-check (all 5 charter consts by name + value, all 14 charter fns in design.rs); adoption census — **panel_col in 30 of 31 adopted files**, timeline_marks the named exception (charter §13: its card is deliberately UNKEYED and hand-built from `design::PANEL_RADIUS`; `panel_col` would force a key), design-kit consumers 36 files; retired-helper census ZERO for `fbtn`/`kbtn`/`pbtn`/`pibtn`/`ctl`/`ictl`/`row_toggle`/`danger_btn`/`section_label` definitions in rune_sec production code — the survivors are delegating wrappers whose bodies call the kit (inspector_style `cap_chip` :660, `preset_chip` :679; fx_browser `section` :271) plus graph_panel `preset_chip` :1470, a named exception (the chip carries its own drawn preview-curve canvas; `chip_btn` has no content slot). Local gate run: PASS, all six checks | wired | CI 42488645714 ✓; headless 42488647081 ✓ |
| **Help copy was stale in two places** (P14) — the Delete row said "selected segment" though the key has been a stroke → particle → segment cascade since Phase J; Ctrl+S (a real binding) hid in the Features prose, and the palette's drift guard carried a documented exception for exactly that | V-wave copy pass: the Delete row reads "Delete the selected stroke, particle event or segment" (help_panel.rs:277); `("Ctrl+S", "Quick save")` is a Shortcuts kbd row (help_panel.rs:271) and its Features prose row is deleted — the census is unchanged at **54** (pin help_panel.rs:592; 25 true-key rows / 29 Features rows); the drift guard's exception is retired (command_palette.rs:2253) now that every hint names a real Shortcuts label | wired | CI 42488645714 ✓; headless 42488647081 ✓ |
| **Layers Top/Bottom jumps stayed text** — the one P13 conversion the A-wave left behind, because `angles-up`/`angles-down` were uncurated at its census time | V-wave spillover: both are `icon_btn`s now (layers_panel.rs:537 `angles-down`, :559 `angles-up`; glyphs curated by F4), keys/labels/handlers verbatim. The panel's Phase H doc paragraph still states the old premise (angles glyphs uncurated) — a stale doc line, named here rather than silently rewritten | wired | CI 42488645714 ✓; headless 42488647081 ✓ |
| **Counts at Phase N close** — keys **804** by the repaired census (699 static + 105 dynamic; the Phase M tree under the same repaired census is **788** — 679 + 109, re-measured on baf5441 — so zero keys lost across the phase; the foundation's additions are the `waveform-card` key plus test/branch keys the census counts symmetrically), palette **108** commands (pin command_palette.rs:1712), MCP **102** total (registry **91**, pin mcp_tools.rs:3557), icons **163**, scenes **15**, help **54** rows | Gate checks 1–6 + the pins cited | wired | CI run 42488645714 GREEN on ebdda27 — 2,492 tests (996 lib + 1,486 bin + 10 integration); headless run 42488647081, all 23 probes exit 0, snaps pixel-read by the integrator |
| **White on the kit accent measures 4.19:1** — fails WCAG AA for normal text by 0.31 at 12 px (P12, the one token-level contrast finding) | Documented acceptance, not a silent miss: accent fills carry short labels and icons only (active buttons, chips, selected rows), never body text; text on card measures 14.75:1 and text_secondary 6.74:1 | open — accepted | — |
| **`AppSpec::tab_visits_controls` stays OFF** — Rune supports Tab-to-controls focus; the shell never enables it | Deferred product decision (P14): enabling it would make Space press a focused Play button (the Play conflict) and Left/Right feed focused sliders instead of frame-stepping. main.rs never sets it; Rune's default is false | open — deferred | — |
| **Legacy accent split** — the stage/FX drawing family uses the Wave-14 2.0 constants (`SELECTION_ACCENT 0x6D8CFF`, stage_view.rs:318, pinned equal to `stage_fx::GIZMO_COLOR` :253; `0xA184FF` in the timeline/FX span colours and palettes), not the `tok` accent | A documented split (P12), left exactly as documented by the stage_view audit: unifying canvas colours with the token accent is a product decision, not a restyle | open — noted | — |
| **Two kit items have zero production consumers** — `design::text_muted` (design.rs:92) and `design::toggle` (design.rs:273) | Gate check 6 reports both UNADOPTED (report only). text_muted: the one candidate consumer (the workspace blurb) deliberately did not take it — it would dim the blurb, not lateral-move it. toggle: every shipped toggle uses `toggle_row`, whose pill is the same construction; the bare `toggle` fn is exercised only by design.rs's own test. The API stays as chartered | open — noted | — |
| Sound unverified on a real screen — the headless snaps cover pixels, not audio; the sync corrections are bounded restarts, not a sample-accurate clock | Needs the user's Play test on a real screen | open — unchanged | — |
| Two "New project" results disagree (panel blank vs Home demo) | Not a Phase N section — user's product call | open — unchanged | — |
| Range MP4/WebM exports carry no audio (the range path renders video only) | Not closed by Phase N — the panel warns (Phase M §30); closing it means muxing range audio, a separate piece of work | open — warned in panel | — |
| `core.onion_frames` / `cycle_onion_frames` orphaned engine API | Stays deliberately: the shell keeps asymmetric onion depths in State (`onion_ui.prev` / `onion_ui.next`, Phase G §25), which the engine's single symmetric count cannot express | open — by design | — |
| Status strip's **Audio segment glyph** refusal (Phase K §16: 753.75 pt > 744 pt, test-pinned) | Not a Phase N section — the Phase N status restyle kept the Phase K floors by charter ruling, so the measured refusal stands | open — unchanged | — |
| MCP (102 tools) vs palette (108 commands) are different surfaces | Unchanged by Phase N — the sets overlap but are not identical; parity of counts is not parity of sets | open — noted | — |

## 17. Phase O — targeted remainders

| Remainder | Closing section(s) | Status | CI |
|---|---|---|---|
| **Settings language rows ran past the card's right edge** (Phase N §8's named remainder; p05 O-1: four chips past the 316 pt content edge, `settings-lang-sl` +51.0 and `settings-lang-he` +38.5 past the 1440 window) | PHASE_O §3 (O-1): the rows carry the plan's `.flex_wrap()` AND its named contingency `.w_full()` — the load-bearing half, since wrap cannot engage against an unbounded row width (`settings_popover.rs`; the workspace and speed grids carry the same defensive pair). Keys verbatim (`settings-lang-grid`, `settings-lang-row-{i}`, `settings-lang-{code}`). Model pins (`language_rows_overflow_unwrapped_and_wrap_inside_the_card`, :1820) prove all seven rows' one-line widths (340–443 pt) pack inside 316 and inside the 236 pt narrowest column; the widest chip (Español (Latinoamérica), 181 pt modelled) never truncates. Q9's whole-surface sweep: no other settings row crosses 316 pt | verified in tree | in flight on 911cf77 |
| **No general card-overflow audit method existed** — Phase N's overflow was found by hand, one pixel pass at a time (p05 O-2) | PHASE_O §3 (O-2): `scripts/overflow_scan.py` implements p05 §2 rule for rule (content 304.5/1412.5, card, window; closed exemption list) and exceeds it where later defect families required (rule 4, timeline vertical budget). Wave Q (Q12) made the proof set a machine-enforced contract: `SCAN_SET` names 13 required trees (default, draw-open, graph-duplicates, three-d, three-d-stage, fx-split-mid, loop-range, palette-open, fx-selected, effects-open, audio-open + the subtitle pair); `--dir` scans every dump present and fails INCOMPLETE when a required tree is absent. Self-test PASS at 911cf77 | wired | in flight on 911cf77 |
| **Layers layer row overflowed ≥ +134 pt** — the reorder cluster sat past the card, unreachable (p05 O-3) | PHASE_O §3 (O-3): the plan's two-line anatomy in `layer_row` (`layers_panel.rs:593`) — dot/name/opacity stepper on line 1, `layer-bottom/down/up/top-{fi}` (keys verbatim) on line 2 at ≈253 of 260 pt; inert twins at the list ends came with the same restructure | verified in tree | in flight on 911cf77 |
| **Draw cluster: three rows + stamp params past the card** — `draw-brushes` 879 pt (+563, six of ten brushes unclickable), tools row +374.5, shapes +132.5, stamp params +42 (p05 O-4) | PHASE_O §3 (O-4): `.flex_wrap()` on tools (:785), brushes (:843) and shapes (:864) in `draw_tools.rs`; stamp params took the stronger explicit three-row split (`draw-stamp-params`/`-2`/`-3`, each stepper group whole, each row inside even 276 pt) after Wave 9 re-measurement showed no pairing fits | verified in tree | in flight on 911cf77 |
| **Graph + keyframe rows past the card** — `graph-actions` +305.5 (`graph-clear` ended x 1717.5), `keyframe-presets` +45, `graph-presets` +13.5, `keyframe-actions` +11.5, `keyframe-clipboard` +2.8 (p05 O-5) | PHASE_O §3 (O-5): graph rows wrap (`graph_panel.rs:1961`, :2007), presets wrap (`keyframe_tools.rs:1308`). `keyframe-actions` (:1356) and `keyframe-clipboard` (:1380) were the one genuine remainder in O-1…O-9 — the UI mission Wave 1 commit message claimed all nine items and had not landed these two; P2's wraps (+11 lines, keys verbatim) are in the tree, reviewed line by line | wired | in flight on 911cf77 |
| **Weather-kind segments ran 434.3 pt** — `wx-seg-5` ended past the window (p05 O-6) | PHASE_O §3 (O-6): `.flex_wrap()` on the segment row (`scene_panel.rs:1071`), `wx-seg-{i}` keys verbatim | verified in tree | in flight on 911cf77 |
| **3D posing / camera / bake rows ran wide** — +49 / +37.5 / +9.5 (p05 O-7) | PHASE_O §3 (O-7): all three rows wrap (`three_d_panel.rs:917`, :1036, :1175) with the plan's measurements in their comments; the plan's unkeyed "+39 sibling" is one of these three rows under the mission audit's attribution | verified in tree | in flight on 911cf77 |
| **`fx-nudge` ran 343.5 pt with `fx-dur-up` at the window edge; playback transport/loop rows +17.6 / +36** (p05 O-8) | PHASE_O §3 (O-8): `fx-nudge` split into two lines under its one key — the plan's preferred anatomy (`fx_clips.rs` `nudge_row`, :1487); transport and loop rows wrap (`playback_controls.rs:673`, :715) | verified in tree | in flight on 911cf77 |
| **Pose row packed past the edge; `cam-delete-key` ended past the window** — x 1428.5 / x 1441.4 (p05 O-9) | PHASE_O §3 (O-9): the packed pose row no longer exists — ops regrouped, Apply-to-all on a `.flex_wrap()` icon row (`pose_tools.rs:1379`, keys verbatim); the camera key row wraps (`camera_panel.rs:1232`) with the plan's measurement in its comment | verified in tree | in flight on 911cf77 |
| **Bare empty lines** — the fx browser's "No presets saved yet", project_tools' project-list line, text_tool's two messages, stroke_inspector's "No strokes on this frame.", three_d_panel's three sub-section lines, camera_panel's "No camera keys yet.", both inspectors' no-selection lines (Phase N §8 + charter seed) | PHASE_O §4: every named surface renders the kit `design::empty_state` (`design.rs:308`) — glyph from the curated table, message wrapped inside the card budget, adopted copy byte-identical unless the message names its action (camera, segment inspector, Home library). P7/P8 audited A–M, N–Z and Home call-site by call-site (UI mission Wave 4 adoptions); panels that do not own their filling action carry no fake button. One imprecision stands verbatim under rule 2: the figure inspector's message also renders for a stale selection in a non-empty project | verified in tree | in flight on 911cf77 |
| **Two kit items had zero production consumers** — `design::text_muted` and `design::toggle` (§16 carried both as open — noted; the Phase O charter seed demanded adopt-or-retire on evidence) | PHASE_O §5: **toggle RETIRED** (the kit's docs record it, `design.rs:67` — zero callers; every shipped toggle is a `toggle_row` over the same `toggle_pill`); **text_muted KEPT** as the kit's muted-text role (`design.rs:180`; consumers: `v_splitter` grip, `menus.rs` `dimmed_item` :556; pin `text_muted_is_secondary_at_sixty_percent`, `design.rs:967`). Decided by UI mission Wave 2, verified standing by P12 — supersedes §16's open entry for these two items | closed — decided | in flight on 911cf77 |
| **Interaction states: buttons + rows** — charter rule 3 demanded rest/hover/press/selected/muted on every element from the existing tokens | PHASE_O §6 (P4): the kit families were complete; exactly four call-site gaps, fixed in overlay setters only (+107 lines, zero keys) — palette result rows gain the btn press derivation (`command_palette.rs:2032`), fx-lane rows gain `.press(t.press)` (`fx_clips.rs:1378`), scene bg swatches gain the kit overlay pair (`scene_panel.rs:1110`), the legacy unmounted `timeline_row` chips gain press. Construction pins in both themes for each | wired | in flight on 911cf77 |
| **Interaction states: inputs + selects + steppers** — the charter's second state class had no disabled-state doctrine at all | PHASE_O §6 (P5, finished by Wave Q's Q8): the kit gains the doctrine — `StepForm` + `step_btn`, `field_border` (accent only while focused), `field_caret` (accent caret/selection; Rune's white default is invisible on the light theme), `text_field`, `slider_track` (`design.rs`; `step_btn` :504). Q8 completed the adoption: `proj-name` is the 14th field on `design::text_field` (no bare `text_input` remains outside the kit) and the remaining stepper pairs take computed forms against their ops' own clamps/guards (layers/figures opacity, particles frame, fx start/duration, segment length/thickness, both camera zoom pairs — the panel pair's op snapshots before clamping, so a limit press spent an undo step on an identical key). Judged remainders left live under the doctrine: stroke `param_row` (its op speaks a refusal at the ladder ends), frame_ops insert-N (MutedLive by design — a bound press heals a stale count) | wired | in flight on 911cf77 |
| **Interaction states: every other kit class** — the charter's remainder after buttons/rows and inputs/selects/steppers | PHASE_O §6 (P12): audited builder by builder — `tangent_preset_chip` (both branches), `swatch`, menu items + `dimmed_item`, `slider_track` (a documented visual, not a control; its states belong to its drag roots), workspace tabs and chrome buttons (the buttons class). No class left stateless; no edit owed | verified in tree | in flight on 911cf77 |
| **Motion consistency** — charter rule 4 allows only the `pair::SNAP` / `pair::SETTLE` springs; Phase N's entrance/exit behaviour preserved | PHASE_O §7 (P10): exactly four `.transition(` call sites in the tree (menus, settings card, command palette — SNAP; toasts — SETTLE) and every other element rides the engine default, itself SETTLE; zero ad-hoc springs/tweens/easings in app code; zero conversions needed or made. Named, not faked: the toast expiry fade is linear by Phase N design (the charter preserves it), and the toggle knob teleports (the pinned Rune rev cannot animate a layout offset; the kit doc gates any future slide to SNAP/SETTLE). Timeline scrub is motionless by the charter's no-delay rule | verified in tree | in flight on 911cf77 |
| **Silent mutations: stage pose + camera drags ended without a word** — every other gesture family posts a recap at gesture end; the stage's own drags left the stale status standing (charter rule 6) | PHASE_O §8 (P6): a 333-op transitive census found this the one silent class. `end_drag` now posts `"Posed figure {n} at frame {f}."` / `"Camera key moved at frame {f}."` (`editor.rs:524`, :499) measured against a gesture-start baseline with the dial's dead-zone discipline — a no-move press stays silent; pin `wave_p_end_drag_recaps_only_real_moves` (`editor.rs:8103`). No element, no key, no new channel. P6's sibling observation (stage_drag ignored `cancelled()`) was closed in the same integration by Q2: a cancelled drag undoes its one gesture snapshot, guarded against snapshot-less presses | wired | in flight on 911cf77 |
| **Keyboard discipline: three routing defects** — charter rule 8 (Esc closes the topmost overlay; keys never leak to the stage mid-typing) | PHASE_O §9 (P9): all in `src/main.rs`, nine pins in `key_routing_tests` (:1450). **D1 (high):** the Home rename field was missing from `text_focused` — Space/N/Delete/Ctrl+Z fired mid-typing; now the armed-conjunction at :830, matching its sibling rename sessions and neutralising Home's stale-focus-flag wart. **D2:** menu Esc sat below the text-field bail and could die on the wrong scope; the Esc tier now sits after the palette claim, before the bail, topmost first. **D3:** the two armed confirm gates had no keyboard exit; Esc now disarms them without running the gated op. No binding removed, renamed or re-bound (census pin; Ctrl+S enumerated, not pressed — its arm writes ./projects) | wired | in flight on 911cf77 |
| **Shortcuts shown in the UI must be real** — charter rule 8, second half (the Ctrl+S lesson) | PHASE_O §9 (P12): menu hints, help-panel rows and the palette's `command_hint` table all cross-checked against `main.rs::on_key`; the palette table is guarded by the drift test `command_hints_name_only_real_bindings` (`command_palette.rs:2975`) — every hint spelling must be a help-panel key label. Ctrl+S is a real binding with a real Shortcuts row; the historical documented exception is retired. No lying hint exists on any surface | verified in tree | in flight on 911cf77 |
| **Copy en-GB; numbers in mono** — charter rule 9 | PHASE_O §9 (P12 item 5): every shell string literal swept for US forms — all hits are keys/identifiers or already en-GB; MCP descriptions likewise ("centred"/"grey"; their `color` tokens are parameter names). Mono is structural: the kit `stepper_value` and `kbd` builders apply `.mono()` over `MONO_CAPTION` (the pinned rev's token does not set the Mono family — a recorded Phase N charter amendment), so every kit stepper inherits it | verified in tree | in flight on 911cf77 |
| **The p05 §2 scan table over real headless dumps** — the charter's named artifact-level proof for O-1…O-9 (P12 IA-1's second half) | PHASE_O §10 (Wave S, S8): the table is written from Headless Rune probe run `42759195547`'s artifact (34 tree dumps on `59cb0be`, the Wave R tree that contains the integrated Wave P+Q tree) — `scripts/overflow_scan.py --dir` reads **CLEAN: 0 non-exempt hits on all 34 trees, scan set complete** (all 13 SCAN_SET-required trees present). Every repaired row of p05 §3 re-measured in the dumps: worst settings chip now ends 1392.5 (pre-fix 1463.0, past the window), `cam-delete-key` 1193.9 (was 1441.4), `layer-row-0` exactly at the 304.0 content edge, the wrapped draw/graph/weather/fx rows ending exactly at the 1412.0 content edge with no descendant past it | verified — table in PHASE_O §10 | headless run `42759195547` SUCCESS on `59cb0be` |
| **White on the kit accent** — 4.19:1 in §16's measurement; the UI mission acceptance review measured **3.14:1** as rendered and records that the older figure does not reproduce | Charter exclusion, restated by PHASE_O §11: the accent is not darkened; accent fills carry short labels and icons only, never body text. Not relitigated by Phase O | open — user's decision | — |
| **`AppSpec::tab_visits_controls` stays OFF** (§16, unchanged) | Charter non-goal (PHASE_O §1, §11): the Play / frame-stepping conflicts stand as recorded in PHASE_N §8. Its Phase O consequence — Rune's `El` exposes no button focus state, so no focus visual is expressible — is recorded as an engine limit (PHASE_O §6), not faked | open — deferred | — |
| **Two "New project" results disagree** (panel blank vs Home demo) (§16, unchanged) | Charter non-goal: the user's standing product call; Phase O changed nothing | open — unchanged | — |
| **Legacy accent split** — stage/FX canvas colours vs the token accent (Phase N §8 remainder the charter did not adopt) | PHASE_O §11: unification is a product decision, not a restyle; left exactly as documented | open — noted | — |
| **The settings card and Esc** — is the settings card an "overlay" under rule 8? (P12 IA-2) | PHASE_O §9: two independent audits (P9's census, P12's sweep) read it as a toggle-button card in the column, not a scrim overlay; its open/close behaviour is unchanged per the charter's second sentence, and no Esc arm was added. If the user rules it an overlay, the join is one arm beside the menu arm | open — noted (reading recorded) | — |
| **Menu-open key leak (non-Esc)** — stage bindings still fire while a menu is open; the menu scope claims only Esc (P9 observation O1) | PHASE_O §9: rule 8 names Esc only and preserves other overlay behaviour; swallowing the rest would drop live bindings. Needs a product decision before any fix | open — noted | — |
| **Range MP4/WebM exports carry no audio** (§16, unchanged) | The charter excludes audio-path work; the panel warns (Phase M §30), so the feedback half is closed. Muxing range audio is a separate piece of work | open — warned in panel | — |
| Sound unverified on a real screen (§16, unchanged) | Needs the user's Play test on a real screen; no Phase O item touches the audio path | open — unchanged | — |
| **Counts at Phase O close** — keys **979** total unique by the gate's current counter (863 static + 116 dynamic), floor 949; the charter's 804 baseline was the Phase N figure under an earlier extractor calibration (the same tree at 0b75051 reads 940 under the current counter — 827 + 113) | Gate checks 1–2 (inventory + key diff vs HEAD) green at every integration stage; Phase O's own items added zero keys — the growth to 979 belongs to Wave P/Q's other themes (itemised in REDESIGN_PROGRESS §6.11). Icons 163, scenes 31, palette 134 — none moved by this phase. Local instruments at 911cf77, run at close-out (python only): gate PASS 10/10, MCP census PASS (381 — not a Phase O surface), overflow self-test PASS | verified locally | in flight on 911cf77 |
| **CI evidence for this section** | The last completed master CI run before Wave R was on `0b75051` (Build, Test & Binary, SUCCESS — observed via `gh` during Wave Q's docs pass). The integrated Wave P+Q tree's CI on `911cf77` is in flight; no verdict is claimed anywhere in this section, and every status above is stated as in-tree / locally gated, which is what the evidence supports | — | in flight |

**§17 CI column — superseded (Wave V re-audit, §18):**
every "in flight on 911cf77" cell above has since resolved.
On `911cf77` itself: headless `42755921970` GREEN; build
`42755902602` FAILED in the Test step (a lib test-target
compile error in Wave P's missing-audio tests — test-side,
fixed on the Wave R line). The §17 items' substance was
then re-proven by later headless runs (§18's CI frame):
the overflow scan reads CLEAN over the real artifacts of
runs `42759195547`, `42764348922` and `42765217055`, and
§17's counts row (keys 979 / floor 949, scenes 31) is
superseded by §18's settled counts.

## 18. Wave V re-audit — current-state verification

Appended 2026-11-10 (Wave V, V18). **Audit base: HEAD
`1c7a70d`** (the Wave U integration), read from a pristine
`git archive` snapshot so concurrent Wave V edits could
not contaminate the counts; CI state via `gh` at the
final check. Method as always in this doc: static reading
+ the python instruments — no cargo (repo rule). The one
gate check that needs a git ref (check 2, key diff vs
HEAD) cannot run in an archive snapshot and is the
snapshot run's only non-pass; the integrator's in-repo
run on the same committed tree reported PASS 10/10.

### 18.1 Standing counts — re-verified

| Claim | Settled value at `1c7a70d` | Evidence |
|---|---|---|
| MCP surface | **381 tools** (44 builtin + 319 registry + 9 io + 4 subtitle_io + 5 subtitle) | `mcp_census.py` totals line on the snapshot; gate checks 9–10 PASS (handler integrity: 337 handlers resolve 1:1; docs/MCP.md catalogue in exact sync) |
| Element keys | **980 total** (864 static + 116 dynamic), gate floor 980 | gate check 1 on the snapshot. §17's 979 / floor 949 is superseded (+1 key; the floor ratcheted to the verified total) |
| Shell `State` | 48 fields, 22 full literals, every field in every literal | gate check 3 |
| Palette commands | **134** | pin `command_palette.rs:2156`; gate checks 5 + 10 derive 134 from source. §§13–16's 95–108 figures are their phases' closes, not the current count |
| Curated icons | **163** | pin `icon_data.rs:355` (`curated_set_has_163_names`); gate check 5. Unchanged since Phase N |
| Headless scenes | **34** (layout facts 1..=36) | scene table in `scenes.rs` (`home-scan-fallback` is the 34th); gate checks 5 + 8 (all 34 names appear in the headless.yml tree loops) |
| Overflow SCAN_SET | **16 required / 0 pending** | direct parse of `overflow_scan.py`'s SCAN_SET; §17's 13 grew by the speaker tree (Wave S) and the two Home trees (Waves T/U) |
| i18n catalogue | 28 tables, **identical key sequences in all 28**, no duplicates | this audit's structural parse. Stated count **563** is the project's line-wise convention; the tuple-true count is **564** — the difference is exactly one entry per table, `status_ready` (`src/i18n.rs:445`), the catalogue's only multi-line tuple, which line-wise counters miss. No key is lost, duplicated or desynchronised; the artefact is remainder **R1** |
| Help rows | **61 total, 36 of them Features** | pins `help_panel.rs:752` (`shortcut_rows().len() == 61`) and `:805` (Features == 36). §16's "54 rows" was the Phase N close; the +7 reconciles exactly with the UI-mission Waves 9–12 refreshes (+2/+1/+1/+3) traced in the pin's own comment. `help_panel.rs`'s module prose still says 54 — remainder **R2** |

### 18.2 Surfaces and contracts landed since §17 — verified in tree

§§1–17 predate the entire subtitle subsystem and the
Wave R–U feature set; the standing record for them is
here.

- **Subtitles (Waves P–T).** Core in `src/subtitles.rs`:
  cues with half-open `[start, end)` liveness, style model,
  the `speaker` field (`:118`, `Option<String>`, serde
  default; normalised by `normalise_speaker`), the display
  prefix composed by `subtitle_display_lines` (`:431` —
  `"{speaker}: {first}"`), SRT import/export with the
  counted `speakers_dropped` (`:543`), WebVTT `<v>` voice
  tags round-tripping the speaker. Shell: subtitle panel +
  timeline lane; MCP: 5 subtitle tools + 4 subtitle IO
  tools (in the 381 above). Pixel proof: the subtitle
  census (below). One stale doc line inside the file —
  remainder **R3**.
- **Subtitle census capture-scale contract (S+T CI
  fix, `9d7dd0a`).** Tree dumps are window POINTS; snaps
  are capture PIXELS at the dump-declared scale (2.0 in
  CI: 2880×1800 snaps vs a 1440×900-point window).
  `subtitle_census.py`'s `tree_scale` (`:463`) derives
  the scale as snap width / dump window bounds (the
  dump's `scale` field is the fallback), converts the
  seeded stage rect and the lane block ONCE into pixel
  space (`:728`), and gates stage agreement across the
  seeded family only — the default scene's stage element
  is legitimately 40 pt taller (no subtitle lane) and is
  reported, not gated. Floors unchanged; the self-test
  carries a scale-2 honest set pinning the conversion
  (PASS on the snapshot). Proven on real artifacts: the
  `42764348922` transcript failed 10 assertions under the
  old contract; after the fix the same artifacts read
  PASS locally, and run `42765217055`'s transcript is
  PASS (19/0) in CI. **Lesson, restated for this doc:
  a headless workflow's conclusion is never the verdict —
  its fail-soft transcripts are.**
- **Export scale matrix + JPEG import (Waves R/T).**
  Every raster format takes a scale (SVG excepted by
  nature), JPEG sequence included; `export_bundle`
  validates scale before any write; JPEG imports by
  magic through the decode-checked funnel. Proof suites
  in tree (`tests/wave_u_undo_census.rs`,
  `wave_u_determinism.rs`, `wave_u_e2e.rs`).
- **Encode-phase export progress (Wave U, U11).**
  `EncodeProgressEvent` (`src/export.rs:1407`) +
  `EncodeProgressCallback` (`:1425`): the count is the
  ENCODER's own, from ffmpeg's `-progress` stream —
  never the render counter; strictly increasing
  intermediates, exactly one terminal event after
  successful exit + drain, failures keep the historical
  error voice with no terminal. Eight
  `*_with_encode_progress` entries; the eight historical
  entries delegate with `None` (byte-identical
  invocation). MCP is deliberately NOT wired — zero
  `EncodeProgress` references across `src/mcp*.rs`
  (IA-U11-1: the handler type has no notification sink;
  a post-hoc field would be fake progress). Panel wiring
  is remainder **R4**.
- **Home missing-audio + scan fallback (Waves S–U).**
  Both Home read paths (scan full-parse and light read)
  surface missing-audio counts; the fallback arm is now
  headless-proven by scene 34, `home-scan-fallback` (a
  project file trimmed of its `frames` key full-parses
  through serde defaults while the light read's gate
  refuses it). Marker steps in `headless.yml` (both tree
  loops carry all 34 scenes; markers at `:539ff`);
  transcripts PASS in runs `42764348922` and
  `42765217055`.
- **§3's open questions are closed.** `--tree` dumps
  carry element rects (consumed by the overflow scan
  and the subtitle census on every run); `--snap` on
  software Vulkan has been proven since Phase I (run
  `42467268661`) and is routine; §4 rows 4–6 are filled
  above. `headless.yml` remains `workflow_dispatch`-only
  by design (its header states why) and is dispatched
  manually on every integration.

### 18.3 CI frame at this audit's final check

| Tree | Build, Test & Binary | Headless Rune probe | Note |
|---|---|---|---|
| `911cf77` (Wave P+Q) | `42755902602` FAILED (Test step, test-target compile error) | `42755921970` GREEN | §17's "in flight" — resolved as above |
| `7f9bce5` (Wave R line) | round 6 `42762884625` GREEN | GREEN | all six R-line rounds failed test-side only, never product code |
| `9d7dd0a` (S+T close) | `42765201236` GREEN | `42765217055` GREEN | transcripts verified on the artifacts: census PASS 19/0, Home marker PASS, overflow CLEAN |
| `1c7a70d` (Wave U; this audit's base) | round 1 `42766879504` FAILED | round 1 `42766902475` FAILED | sole error E0308 at `fx_clips.rs:452` — IA-U4-2's replacement text missed a borrow (`kind_label` returns `String`; `str::replace` wants `&str`). Integrator fix `fbc3644` (one character); round 2 build `42767519353` **GREEN**; round 2 headless `42767540386` still in flight at filing — its transcripts are unread, so no headless verdict is claimed for the Wave U tree yet |

### 18.4 Carried open items (unchanged, not re-litigated)

The standing user decisions and by-design entries from
§§11–17 stand exactly as recorded: white-on-accent
contrast (measured 3.14:1 as rendered; the user's call),
`tab_visits_controls` OFF, the two-"New project"
disagreement, range MP4/WebM exports carrying no audio
(the panel warns), sound verifiable only by the user,
`core.onion_frames` orphaned by design, the status-strip
Audio-glyph refusal, the settings-card Esc reading, the
menu-open non-Esc leak (a product decision), and MCP
(381) vs palette (134) remaining distinct surfaces that
merely overlap.

### 18.5 Remainders needing a slice

| # | Remainder | Evidence | Owner |
|---|---|---|---|
| R1 | i18n stated count (563) undercounts the catalogue by one in every table: `status_ready` is written as a multi-line tuple (`src/i18n.rs:445`), invisible to the line-wise counters the docs and headers (`i18n.rs:13`, `i18n_wire.rs:6`) rely on. Fix is mechanical — reformat the tuple to one line in all 28 tables (counts then read 564 everywhere) or restate the count as 564 | this audit's parse: one-line entries 563 vs 564 tuples per table; difference exactly `status_ready`; sequences identical ×28 | integrator (i18n.rs is frozen for Wave V agents) |
| R2 | `help_panel.rs` module prose still states "54 rows" (`:197`, `:220`, `:234`) while its own pins assert 61 total / 36 Features (`:752`, `:805`) | grep + pins above | V5 (owns the help surface this wave) — prose only |
| R3 | `src/subtitles.rs` module doc (`:30–36`) says the speaker "is metadata… No renderer draws it yet (the prefix display is a filed IA…)" — stale since Wave R: `subtitle_display_lines` (`:431`) composes the prefix for the renderers and the panel carries the speaker field | doc text vs `:431` + the census's widening leg | subtitles owner / integrator — doc text only, no behaviour change |
| R4 | Encode-progress has no consumer: the panel wiring needs a background-job slice (worker thread + shared state + tick polling — panel exports block the UI thread), and MCP transport needs a `notifications/progress` mapping. Both are boarded feature work, deliberately not improvised (IA-U11-1) | `src/export.rs:1407ff` contract; zero MCP references | a future panel/transport slice (V12 covers the render-phase extension this wave, not the wiring) |
| R5 | Figure-library name case-loss: a figure saved as "Styk" is written to `styk.json` and listed/inserted as "styk" — CONFIRMED present at this audit's base (`src/editor.rs:3136` lower-cases the library file name; the slug path `src/figure_ops.rs:965` likewise) | greps above; the defect was first proven by Wave R-line CI round 5 | V1 owns the fix this wave (round-trip fidelity + on-disk migration behaviour); QA closes this row only when V1's CI lands |
