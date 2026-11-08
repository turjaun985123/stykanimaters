# GUI Audit — Wave 14 Phase 1 Baseline

Evidence document for the redesign. Every number below was **measured**
with `grep` / `wc` / scripted tallies — none are estimates.

**Measurement basis:** the committed tree at `b0f5578` (post-Wave-13
docs), extracted with `git show HEAD:…` and tallied from that snapshot.
The working tree was already changing under parallel Wave 14 agents
while this audit ran (e.g. `src/ui/kit.rs` grew mid-measurement), so
all figures are pinned to the `b0f5578` snapshot for reproducibility.
Re-running the same tallies against a later commit is how the redesign
phases should prove progress.

---

## 1. `src/app.rs` — size and control inventory

| Measure | Count | How measured |
|---|---|---|
| Total lines | **4,753** | `wc -l` |
| `render()` body alone | **2,173 lines** (lines 2167–4339) | fn boundaries |
| `btn(` textual occurrences | 221 | grep |
| — of which the `fn btn(` definition (line 2146) | 1 | |
| — of which a comment mention (line 2517) | 1 | |
| **`btn(...)` call sites** | **219** | 221 − def − comment |
| Call sites with a literal id written inline | 216 | regex on the call |
| Call sites with a dynamic/variable id | 3 | lines 952 (export-row helper), 1317 / 1332 (weather steppers) |
| **Distinct literal button-id patterns** | **216** | 193 plain + 23 tuple |
| — plain string ids (`btn("save", …)`) | 193 distinct, 193 calls | |
| — tuple ids (`btn(("fig-up", i), …)`) | 23 distinct patterns | audio-norm/split/xfade, clip-edit/loop, ease-set, fig-del/dn/eye/lock/op-dn/op-up/opac/solo/up, fx-del/dup/earlier/later/preset-apply/preset-save, home-dup/open |
| Additional non-`btn` `.id(...)` families | 5 | `frame`, `fig-sel`, `swatch`, `wx-seg` (×2 sites), `xc-tab` |
| **Tool groups** (`tool_group("…")`) | **24** | Transport, Figure, Frames, Segments, Figures, Draw, Camera, Playback, Effects, Library & Clips, Export, Projects, Import, View, Audio & Text, 3D, Scene, Audio Ops, Formats, Stage & Language, Effects 2, Home, Segment Style, Strokes |
| **Toggleable panels** (`show_*` fields) | **12** | see §2 |
| **EditorView fields** | **31** | struct tally, §2 |
| Panel builder methods (separate fns) | 6 | easing (726), fx (791), export (938), weather (1195), audio (1412), clips (1591) — Home, Graph and Inspector are built inline inside `render()` |

Two structural facts the redesign must confront:

- **219 buttons are rendered through one helper** (`btn()`, app.rs:2146)
  whose own styling hardcodes its colours as literals (see §3).
- The FX kind buttons alone are 27 of the 193 plain ids
  (`fxclip-*`, one per ClipKind), and the export surface is split
  across three groups (Export, Formats, plus the Export Centre
  panel) — the "Effects" / "Effects 2" group split is the same
  pattern.

## 2. EditorView state — 31 fields, 12 of them panel toggles

Fields (in declaration order): `core`, `canvas_bounds`,
`focus_handle`, `player`, **`show_home`**, `recognizer`, `clock`,
`pinch_cam`, `text_edit`, **`show_graph`**, `graph_channel`,
`graph_bounds`, `graph_drag`, `path_drag`, `graph_sel`,
`graph_tan_drag`, **`show_audio`**, **`show_clips`**, **`show_fx`**,
`toasts`, `last_status`, **`show_easing`**, **`show_help`**,
**`show_color`**, **`show_guides`**, **`show_weather`**,
**`show_export`**, `export_tab`, **`show_inspector`**, `range_sel`,
`audio_peaks_cache`.

- **12 independent `show_*` booleans** = 12 toggleable panels:
  home, graph, audio, clips, fx, easing, help, color, guides,
  weather, export, inspector. There is no workspace/mode concept —
  panels accumulate; mutual exclusion exists only ad hoc between a
  few of them (graph/audio/fx).
- The Export Centre's Media/Formats split is *not* a panel toggle:
  it is `export_tab: usize` with two local derived booleans
  (`show_media`, `show_formats` at app.rs:990–991) — a different
  mechanism from every other panel.
- `show_paths` is **not** an EditorView field — it lives on
  `EditorCore` (core.show_paths), a third place panel-like state
  can live.

## 3. Styling consistency — the scattered palette

### 3.1 The kit tokens (src/ui/kit.rs, 112 lines)

Exactly **5 colour constants** exist in the kit at baseline:

| Token | Value |
|---|---|
| `C_INK` | `0xE8E8EC` |
| `C_MUTED` | `0x9AA3B2` |
| `C_PANEL` | `0x22262F` |
| `C_BORDER` | `0x3A4150` |
| `C_ACCENT` | `0x3D5A99` |

There is **no token** for: the button cell background, any status
colour (success/warn/error), the 3D accent, the onion-skin colours,
or the light theme. `C_ERROR: 0xB23A3A` exists but lives in
`ui/toast.rs:21`, not the kit. The kit has no spacing, radius or
typography tokens at all.

### 3.2 Hardcoded colour literals in app.rs

- `rgb(0x…)` call sites: **19**, with **8 distinct values**.
- `rgba(0x…)` call sites: **6** (0x9AA3B2 ×3, 0xFF5D5D ×2, 0xFFC65D ×1).
- All 6-digit hex literals anywhere in the file (any argument
  position): **37 occurrences, 15 distinct values.**

**The scattered palette** — every distinct hex value used in
app.rs, grouped:

*Token values re-typed as literals instead of referenced*
(duplication, not divergence):

| Literal | = token | Occurrences |
|---|---|---|
| `0xE8E8EC` | C_INK | 7 |
| `0x9AA3B2` | C_MUTED | 7 (+3 rgba) |
| `0x3A4150` | C_BORDER | 2 |
| `0x3D5A99` | C_ACCENT | 1 |

Meanwhile the tokens themselves are barely referenced in app.rs:
`C_MUTED` ×17, `C_BORDER` ×10, `C_PANEL` ×8, `C_INK` ×3, and
**`C_ACCENT` ×0** — the accent colour reaches app.rs almost only
through kit helper functions, never by name.

*Values with no token at all* (the real scatter):

| Literal | Occurrences | Used for |
|---|---|---|
| `0x2C313A` | 1 (in app.rs) | button/chip cell background — the single most-used surface colour in the app has no name |
| `0x35C4D8` | 4 | cyan 3D-mode accent |
| `0xFFC65D` | 4 (+1 rgba) | amber (weather/key highlights) |
| `0xFF5D5D` | 3 (+2 rgba) | red (record/danger accents) |
| `0x7BE495` | 2 | onion-skin next-frame green |
| `0xE9EBEF`, `0xD9DCE2`, `0xD0D4DB`, `0x191C21`, `0x21252D`, `0x1D2129` | 1 each | light-theme / stage greys, one site each |

The emblem: `btn()` itself (app.rs:2146–2165) sets
`bg active = rgb(0x3D5A99)`, `bg idle = rgb(0x2C313A)`,
`hover = rgb(0x3A4150)`, `text = rgb(0xE8E8EC)` — three token
values re-typed as literals plus one untokenised value, in the
one function every button flows through.

### 3.3 src/ui/*.rs outside kit.rs (40 files, 6,047 lines total)

- `rgb(0x…)` call sites: only **3**, all the same value `0x2C313A`
  (clip_row.rs, color_field.rs, timecode.rs).
- `rgba(0x…)` call sites: **0**.
- But bare hex literals are widespread as *values* (returned from
  `*_colors()` helpers, consts, palette data). Styling-relevant
  scatter, excluding pure data (the colour picker's 16-swatch
  palette in color.rs, test fixtures in swatch_grid.rs):
  - `0x2C313A` (untokenised cell bg) appears in **10 ui files**:
    badge, clip_row, color_field, dope, frame_chip, node_row,
    progress_steps, scrubber, segmented, tabs, timecode,
    timeline_ruler, toolbar (13 counting multi-use files once each
    — it is the library's de-facto second background, unnamed).
  - `0x2E4470` — in-range selection highlight: frame_chip.rs and
    timeline_ruler.rs each hardcode it independently.
  - Status colours re-invented per component: badge.rs
    (0x2E7D4F success, 0x8A6D1F warn, 0xB23A3A error),
    gauge.rs repeats 0x8A6D1F / 0xB23A3A as its own consts,
    toast.rs defines 0xB23A3A a third time as `C_ERROR`,
    progress_steps.rs repeats 0x2E7D4F.
  - `0x5A2C2C` — toolbar.rs danger background, used nowhere else.

## 4. Typography — no type scale

`text_size(px(N))` call sites at baseline:

| File scope | Calls | Distinct sizes | Tally |
|---|---|---|---|
| app.rs | 29 | 3 | 12.0 ×22, 11.0 ×4, 13.0 ×3 |
| src/ui/*.rs (incl. kit) | 53 | 6 | 11.0 ×21, 12.0 ×20, 10.0 ×5, 13.0 ×5, 15.0 ×1, 32.0 ×1 |
| **Total** | **82** | **6** | 10 / 11 / 12 / 13 / 15 / 32 |

Notes:

- app.rs and the library disagree about the default: app.rs text
  is mostly 12, the library's is mostly 11. Buttons are 13 (the
  `btn()` helper), body text is 11 or 12 depending on which side
  of the app.rs/ui boundary it was written.
- The kit's own components use 15 (brand), 12 (chips) and 10
  (section labels) — three more ad-hoc steps.
- 10.0 appears only in micro-label contexts (dope.rs, clip_row.rs,
  graph_axis.rs, timeline_ruler.rs, kit section labels); 32.0 is a
  single empty-state glyph in empty.rs. Nothing names any of
  these roles (caption/body/title) — every site picks a number.

## 5. Spacing and radius — an undocumented de-facto scale

Method-call tallies in app.rs (`.gap(px(N))`, `.px(px(N))`, etc.):

| Method | Calls | Values (tally) |
|---|---|---|
| `.gap` | 31 | **6.0 ×21**, 4.0 ×6, 8.0 ×2, 10.0 ×1, 0.0 ×1 |
| `.px` (h-padding) | 14 | **8.0 ×12**, 10.0 ×1, 6.0 ×1 |
| `.py` (v-padding) | 13 | 3.0 ×6, **4.0 ×6**, 6.0 ×1 |
| `.p` (all padding) | 12 | **8.0 ×10**, 6.0 ×1, 16.0 ×1 |
| `.w` / `.h` with px | 4 / 3 | 180 ×2, 168, 26, 170, 10 ×2 |
| `.rounded` | 12 | 8.0 ×9, 6.0 ×1, 12.0 ×1, 999.0 ×1 |

A scale *almost* exists — gap is 6 in 21 of 31 sites, padding is
8 in 22 of 26 sites — but it is nowhere written down, and the
exceptions are unsystematic: gap also takes 0, 4, 8, 10; vertical
padding splits between 3 and 4 for visually similar rows; radius
takes 6 (buttons), 8 (panels/chips), 12 (home panel) and 999
(pills) with the choice made per call site. The btn() helper uses
px 10 / py 6 / rounded 6, which matches **none** of the app-level
padding tallies (8 / 3–4 / 8) — buttons are visibly a different
density from the rows around them.

## 6. Known visual debts (from project history)

1. **Dense toolbar — the only real screenshot.** The one time the
   GUI was seen on a real display (user's own Linux PC screenshot,
   2026-11-08, pre-redesign editor) the toolbar was ~110 small
   buttons in many wrapped rows. The redesign that followed
   regrouped them into the 24 tool groups counted in §1, but the
   fundamental density is unchanged: 219 `btn()` call sites still
   exist and most groups render simultaneously.
2. **The redesigned UI has never been screenshot-verified.**
   Everything since the regrouping (tool groups, all component
   waves, Waves 9–13 wiring) is verified only by CI compile+tests.
   docs/COMPARISON.md states this openly ("the *redesigned* chrome
   has not yet been screenshot-verified"). There is no screenshot
   harness in the repo; visual QA currently depends on the user
   downloading a CI artifact and sending a screenshot back.
3. **Guides overlay does not follow the camera.**
   `src/ui/overlay.rs` documents it in its own source: the overlay
   paints "under the default camera" (overlay.rs:111) and
   replicates `paint_stage`'s transform "with the default camera"
   (overlay.rs:150). With camera pan/zoom/rotation active, the
   safe-frame and cross guides are drawn for a camera the user is
   not looking through.
4. **Timeline duplication: chips vs ruler vs scrubber vs
   dopesheet.** The bottom bar builds **four** representations of
   the same timeline in every render: one `frame_chip` element
   **per frame** (loop in render, app.rs ~2625), the
   `ui_timeline_ruler::ruler` (frame numbers + key dots), the
   `ui_scrubber::scrubber` (playhead track), and the
   `ui_dope::dopesheet` (per-figure key rows). The ruler and the
   chips encode the same keyframe union twice in different
   shapes; nothing collapses at large frame counts.

## 7. Performance observations (source-level)

`render()` is one 2,173-line function. GPUI calls it on every
`cx.notify()` and on every playback tick (33 ms timer during
playback), so everything below runs at up to ~30×/s while playing.
Call-site counts are per render pass unless marked otherwise.

### 7.1 Recomputed in render() itself

| Computation | Call sites in render() | Conditional? | Cost driver |
|---|---|---|---|
| `ui_dope::keyframe_map(&project)` | **2** (app.rs:2610 for the chips, app.rs:2666 for the dopesheet) | No — every render | O(figures × frames) pose comparisons; the same full map computed **twice** per render. (A comment at 2610 even says the ruler uses "its own keyframe_map call" — the duplication is structural, not accidental.) |
| `graph_panel::overlay_curves(project, fig, 96)` | 1 (app.rs:2261) | Only when the graph panel data is built | Samples every channel of the figure at 96 points |
| `stats::project_stats(&project)` | 1 | Only inside the Home panel block | Whole-project aggregation |
| `stats::busiest_figure` + `stats::root_travel` | 1 + 1 | Home panel only | Each walks all frames again, separately from project_stats |
| `stats::figure_keyframes(project, fi)` | 1 call site, **inside a per-figure loop** | Home panel only, ≥2 figures | One full per-figure scan per figure, for the sparkline |
| `projects::scan("projects")` | 1 | Home panel only | **Filesystem directory scan + per-file parse on every render while Home is open** |
| `hud_strip(...)` / chip data | 1 | No | Trivial |
| `dopesheet(...)`, `ruler(...)`, `scrubber(...)` | 1 each | No — every render | Element construction proportional to frames × figures |
| Frame chips | 1 call site **in a loop over all frames** | No | One element + one chip per frame, unbounded |
| `draw_data()` | 1 (see §7.2) | No | The full stage snapshot |
| `toasts.tick()`, `core.maybe_autosave()` | 1 each | No | Cheap; autosave is counter-gated |

### 7.2 Recomputed in draw_data() (called once per render)

| Computation | Call sites | Notes |
|---|---|---|
| `positions_f(...)` | 2 | Full FK evaluation of every figure |
| `vfx::weather_particles(...)` | 1 | Closed-form, proportional to weather amount (Storm caps at 360) |
| `vfx::clip_particles(...)` | 1 | Per active FX clip |
| `camera_at(...)` | 1 | Camera interpolation |
| `paint::spray_dots(...)` | 1 call site, per spray stroke | 6 dots per stroke point |

### 7.3 Panel builders (called from render when their panel is open)

| Computation | Call sites | Notes |
|---|---|---|
| `core.fx_preset_names()` | 1 (fx_panel, app.rs:906) | **Reads the fx-presets directory from disk on every render while the FX panel is open** |
| `audio_edit::read_wav(...)` + `ui_waveform::downsample_peaks(...)` | 1 each (audio_panel) | Mitigated: peaks are memoised in `EditorView.audio_peaks_cache` (checked at app.rs:2411) — the one existing example of render-path caching in the app |
| Weather panel matches on `WeatherKind` | 25 matches in the panel region | Label/summary recomputed per render; trivial cost, but the weather *summary string* is rebuilt in three places (panel, HUD state, cycle handler) |

### 7.4 What this means for the redesign

- The only cache in the whole render path is the audio-peaks
  cache. Everything else — keyframe maps (×2), stats, directory
  scans, overlay sampling — is recomputed from scratch per
  render, and render fires on every interaction and every
  playback tick.
- Any redesign that makes panels *more* live (more panels open,
  richer timeline) multiplies §7.1's unconditional rows. The
  baseline to beat: **keyframe_map call sites per render: 2 →
  target ≤1 (or cached); filesystem scans per render: 2
  (projects::scan + fx_preset_names) → target 0.**

## 8. Baseline scorecard (for later phases to re-measure)

| Metric | Baseline (b0f5578) |
|---|---|
| app.rs lines | 4,753 |
| render() lines | 2,173 |
| btn() call sites | 219 |
| Distinct button ids | 216 literal + 5 `.id()` families |
| Tool groups | 24 |
| Toggleable panels | 12 (independent booleans) |
| EditorView fields | 31 |
| Kit colour tokens | 5 |
| Distinct hex values in app.rs | 15 (37 occurrences) |
| Token values re-typed as literals in app.rs | 17 occurrences (0xE8E8EC ×7, 0x9AA3B2 ×7, 0x3A4150 ×2, 0x3D5A99 ×1) |
| rgb(0x…) sites in ui/ ex-kit | 3 (all 0x2C313A) |
| text_size distinct values | 6 (10/11/12/13/15/32) across 82 sites |
| gap values in app.rs | 5 distinct (0/4/6/8/10), mode 6 |
| rounded values in app.rs | 4 distinct (6/8/12/999) |
| keyframe_map calls per render | 2 |
| Filesystem scans per render (panels open) | 2 |
| Screenshot-verified workspaces | 0 of the redesigned UI (1 pre-redesign screenshot exists) |
