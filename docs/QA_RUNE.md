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
