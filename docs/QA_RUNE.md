# QA — Rune Phase D (GPUI→Rune migration verification)

Snapshot: 2026-11-08, taken while the Phase D section files were
still landing in `src/rune_sec/`. Method: **static reading only** —
no cargo was run (repo rule, `AGENTS.md`); CI is the compile
verdict, and the integrator fills the run ids in §4. Where this
doc says a control "calls" an op, the handler body was read in
the file cited. Where a file landed after the hand-audit pass,
it is marked PRESENT–NOT-AUDITED rather than described from
assumption.

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
| 4 | Headless `--tree` assertions (§3, facts 1–7) in CI | _TBD_ | _TBD_ | _TBD_ |
| 5 | Headless `--snap` stage-height measurement (§3, step 4) | _TBD_ | _TBD_ | _TBD_ |
| 6 | First user-PC screenshot review of the Rune shell | — | — | _TBD — user_ |

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
