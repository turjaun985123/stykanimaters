# P09 — Forms spec: steppers, sliders, text fields, swatches, segmented controls

Phase N planning (P wave). Scope: every form control in the
Rune shell — steppers, drag sliders, `TextEdit` fields,
colour swatch grids, and segmented / chip rows — censused
with `file:line`, mapped onto the charter's kit patterns,
with one value-readout rule and an explicit list of the
instances that must **keep bespoke visuals**.

Method: source census only (no build, per AGENTS.md).
Line numbers are against the Phase M final tree
(close-out `baf5441`). `design.rs` does **not** exist yet —
the F wave builds it to the charter's fixed API; this spec
consumes that API and names the two places the kit needs a
small extension (§2.2, §2.5).

Charter anchors: §4 one button family, §6 steppers,
§8 sliders, §11 kbd chips. The kit form surface the
charter fixes is small — `stepper_value(value, t)` and
`slider_track(frac, t)` in `design.rs`, plus `btn` /
`icon_btn` / `icon_only_btn` metrics (px 10 / py 5,
radius 6). Everything below is those pieces composed.

---

## 1. The readout rule (applies to every form below)

> **R-READOUT.** A value that a stepper, slider or
> segmented row exists to change is rendered **only**
> through `design::stepper_value`: `type_::MONO_CAPTION`
> with `.mono()` applied, colour `t.text`, centred,
> min-width 34. The string comes from the panel's existing
> formatter — **formatters never change in Phase N** —
> only the text element's font, colour, alignment and
> minimum width change.

Why `.mono()` must be explicit: `type_::MONO_CAPTION`
(`rune/src/tok.rs:142`) is only size 12 / weight 500;
rune's own code writes `type_::MONO_CAPTION.mono()`
(e.g. `rewind/scrub.rs:221`). The kit builder owns this;
consumers just call `stepper_value`.

Format table — the strings stay exactly as each
formatter produces them today:

| Kind | Format | Produced by |
|---|---|---|
| Integer | `"{:.0}"` or `"{}"` | segment length/thickness, stage W/H, weather amount |
| Percent | `"{}%"` / `"{:.0}%"` | figure opacity, layers opacity, camera zoom |
| Opacity (fraction) | whole percent via `opacity_text` / `opacity_label` | style opacity, stroke opacity |
| Degrees | `"{:.0}°"` (`format_deg`, `angle_label`) | segment angle/bend, camera angle |
| Hex colour | `"#{:06X}"` uppercase (`color_text`, bg header) | node colour, scene background |
| FPS | whole → int, else 1 decimal (`fps_label`, scene_panel.rs:497) | scene frame rate |
| Multiplier | trimmed `"1.5×"` (`head_ring_label`), `"{:.2}x"` (playback duration) | head ring, frame duration |
| Count with unit | `"{} frames"` | frame_ops Insert-N |
| Duration | `format_duration` via `duration_label` (frame_ops.rs:291), `"—"` when none | frame_ops main row |
| Missing value | `"—"` (em dash) | weather off, text size unloaded, duration none |
| Word value | enum wording (`taper_text`, kind names, `loop_label`) | taper, particle kind, FX kind |

Word values in the −/+ sandwich (particle kind, FX kind)
also go through `stepper_value` — position signals
"value", and one rule beats two. Word values that are
*standalone readouts* (readout rows, summary lines) are
not form values and stay `LABEL` — they belong to P10 /
panel adoption, not this spec.

Today **no** stepper value uses MONO_CAPTION: every one
is `type_::LABEL`. Colours split between `t.text` and
`t.text_secondary` with no pattern (see per-table notes).
R-READOUT removes that split for form values.

---

## 2. Kit form patterns

### 2.1 F-STEP — the stepper row (charter §6)

Canonical composition, in charter order:

```
row(gap 8, center_y)
├─ text(label)  LABEL, text_secondary, grow(1.0)
├─ btn(dn_key, "−", …)            chrome btn metrics
├─ design::stepper_value(value)  MONO_CAPTION.mono(), centred, min-w 34
└─ btn(up_key, "+", …)
```

- **Order conflict, flagged once.** Charter §6 lists
  label / − / value / +. The majority of existing rows
  are label / value / − / + (order B). Only
  `scene_panel::stepper_row` (:680) and
  `settings_popover::stage_axis_row` (:586) use charter
  order (A). Adopting the charter reorders ~15 rows —
  visual only; keys, handlers and formatters are
  untouched. **This spec targets charter order A
  everywhere.** If the integrator prefers B, decide once
  in the F wave; every census row below records the
  current order so either choice is executable.
- **Glyph rule.** Step buttons read `"−"` (U+2212) and
  `"+"`. Five families currently use ASCII `"-"`
  (stroke_inspector `param_row`, particles frame,
  layers per-row, fx start/duration, camera zoom).
  Unifying the glyph changes button *label text* only —
  not keys, handlers or status strings — and is listed
  per instance in §3.1 so the A wave does it knowingly.
- **Variants** (all still F-STEP):
  - **F-STEP-CHEV** — `icon_only_btn` 26px chevrons
    instead of −/+ buttons, for cycle/step axes where
    direction is spatial: vertical chevrons for
    width/opacity (stroke_inspector), horizontal for
    kind cycling (particles, fx). Keep the chevrons;
    only the value becomes `stepper_value`.
  - **F-STEP-MULTI** — multi-step clusters
    (−5°/−1°/+1°/+5°, −15°/+15°, −10/+10, −0.1s/+0.1s).
    These are action clusters, not steppers: they keep
    `btn` metrics and their degree/second labels; the
    shared readout beside them becomes `stepper_value`.
  - **F-STEP-LABELLED** — the step is in the button
    label itself ("Dur −", "Size −", "×0.5", "90%").
    Keep the labels (they are the control's meaning);
    the adjacent readout becomes `stepper_value`.
  - **Not steppers:** cyclers with one Step button
    (style taper, `style-taper`), nudge pads
    (←↑↓→ in particles_panel / text_tool), and
    prev/next pagers (stroke selection, camera key
    nav). They take the button-family pass only.

### 2.2 F-SLIDER — drag slider (charter §8)

```
el(key, size 200×22).on_drag(<existing handler, verbatim>)
└─ design::slider_track(frac, t)   visual only
```

- Drag logic, keys, sizes and the `Drag::data`
  press-value / cancel-restore discipline stay exactly
  where they live. Only the three hand-drawn children
  (track / fill / knob) are replaced by the kit visual.
- **Colour change, stated plainly:** today's sliders
  draw the track in `t.control_off` and the knob in
  `t.text`. Charter §8 fixes the kit track as
  `control_track`, fill accent, knob `t.knob`. Adoption
  therefore changes track and knob colour. That is the
  charter's decision, recorded here so it isn't
  rediscovered as a regression.
- Geometry is already unified: scene_panel
  (`SLIDER_W 200 / SLIDER_H 22 / KNOB 16`, :782–785) and
  layers_panel (:273–280) share it by design.
- **Kit extension needed — bipolar origin.** The wind
  slider fills from the centre (`fill_from = 0.5`) and
  draws a centre tick (scene_panel.rs:795, :831). The
  fixed `slider_track(frac, t)` signature cannot express
  either. F wave: either add an optional origin/tick to
  the kit, or the wind slider keeps its local visual as
  **F-SLIDER-BI** (a named bespoke variant, §6). Do not
  silently drop the centre tick — it is the zero mark
  of a −200..+25 range.

### 2.3 F-FIELD — text fields

Rune-kit owns the editor core (charter, honest limits);
panels style the **frame**. One frame for all fields,
derived from the two framed instances that already
exist (project_tools' wrapper, text_tool's field):

```
frame:  fill t.card, border 1.0 t.card_border,
        radius 6 (CONTROL_RADIUS), px 8, py 4,
        text type_::BODY, colour t.text,
        placeholder t.text_secondary, single_line, grow(1.0)
row:    gap 8, center_y
```

Three sub-variants, differing only in furniture:

| Variant | Furniture | Used by |
|---|---|---|
| F-FIELD-SEARCH | leading `magnifying-glass` icon at 13px in `text_secondary`; no submit button; typing filters live | home / library / figures / fx / pose-library searches |
| F-FIELD-NAME | trailing Apply/Rename/Set-name `icon_btn`; Enter (`on_submit`) applies too where it does today | project name, figure rename, clip rename, library rename, pose rename, audio name |
| F-FIELD-QUERY | no frame of its own — the field sits in the palette's header row (the palette card *is* the frame, charter §9); keep the 52px header, px 16 | command palette only |

Current drift this removes: icon size 12 vs 14 across
search rows; `BODY.lh(26.0)` on some fields and plain
`BODY` on others (clip rename, figure rename, project
name, audio name); nine of thirteen fields completely
bare (no fill, no border — the text floats on the panel
background); project_tools' field already framed but by
a wrapper `el` (`proj-name-field`, h 32) instead of the
field itself — adoption moves the same metrics onto the
field frame and keeps the wrapper key.

Placeholders, `on_edit` / `on_submit` / `on_input_change`
/ `on_input_key` wiring and the poll-driven sync
(figure rename, project name) are behaviour — verbatim.

### 2.4 F-SWATCH — colour swatch grids

Two dialects exist. **Dialect A (slot)** is canonical —
it is the majority (scene bg, scene presets, particles,
text) and reads better at panel width:

```
slot: el(key).size(26,26).radius(8)
        .fill(current ? accent : t.bg).center()
core:   el.size(22,22).radius(6).fill(colour)
grid:   row gap 4, chunks of 8 per row
```

**Dialect B (direct)** — a bare square whose selection
is a white border/outline or an accent-filled padded
cell — is retired onto A everywhere in §3.4. Selection
semantics do not change (exact colour equality, as
today); only the mark changes: white border → accent
slot.

Exceptions that are **not** swatch controls and stay
as-is: 12×12 channel preview squares in
inspector_style's stepper rows (they are readouts of
the channel's own value), the 18×18 `scene-bg-preview`
chip, `text-color-custom` (a readout of an off-palette
colour), and the 14×14 colour chips in stroke list
rows. All four are P10/list-row territory.

### 2.5 F-SEG — segmented and chip rows

```
row(gap 4, center_y)          // the group
└─ btn / icon_btn per segment, active = the value
   active: accent fill + white text (btn's active state)
   inactive: card fill, t.text
```

- One gap: **4**. Today clips loop uses 8, settings
  speed and playback presets use 6, cap / weather /
  workspace rows use 4. The group is one control; its
  internal gap is not panel gap.
- Segments keep individual radius 6 (charter §4
  metrics). Rune has no group-clipping container in
  use anywhere in the shell; do not invent joined
  edges in this phase.
- Labelled groups keep their label as a plain `LABEL /
  text_secondary` text before the group ("Loop mode:",
  "Cap", "Nib"). The workspace row's `· Alt+n` suffix
  is a separate decision (§5, item D3).

---

## 3. Census

### 3.1 Steppers

Order key: **A** = label/−/value/+, **B** = label/value/−/+,
**C** = −/value/+ (no label). Value font today is LABEL
everywhere; colour noted where it is `text_secondary`.

#### inspector_segment.rs — helper `stepper_row` :604 (order B)

| Row | Keys | Builder line | Value format |
|---|---|---|---|
| Length | `insp-len-dn` / `insp-len-up` | :760 (root node: row omitted) | `{:.0}` |
| Thickness | `insp-th-dn` / `insp-th-up` | :771 | `{:.0}` |
| Rotate (F-STEP-MULTI) | `seg-rot-m5/m1/p1/p5` | row :782 | `format_deg` |
| Bend (F-STEP-MULTI) | `seg-bend-m5/m1/p1/p5` + `seg-bend-zero` | row :820 | `format_deg` |

#### inspector_style.rs — three local shapes

| Row | Keys | Builder line | Notes |
|---|---|---|---|
| Colour R/G/B | `style-{r,g,b}-dn/up` | `channel_row` :559, btns :589/:592 | order B; value `text_secondary` + **grows**; 12px channel preview square (readout, §2.4) |
| Gradient R/G/B | `style-g{ r,g,b }-dn/up` (stems gr/gg/gb) | `gradient_channel_row` :597, btns :633/:636 | same shape; rendered only while a gradient end is set |
| Opacity | `style-opacity-dn/up` | row :924, btns :938/:941 | order B; value `opacity_text` (%), `text_secondary` + grows |
| Taper | `style-taper` "Step" | row :815 | **cycler, not a stepper** — button family only; value `taper_text` stays a word readout |

#### inspector_figure.rs

| Row | Keys | Builder line | Notes |
|---|---|---|---|
| Opacity | `insp-fop-dn/up` | `opacity_stepper_row` :661, btns :678/:681 | order B; value `"{}%"`, `t.text` |
| Head ring | `fig-ring-dn/up` | `ring_stepper_row` :766, btns :778/:781 | order B; the *label itself* carries the value (`head_ring_label` → "Head ring 1.5×") — adoption splits label/value: label "Head ring", value `stepper_value("1.5×")`; the formatter stays |
| Scale | `fig-scale-dn/up` ("90%" / "110%") | `scale_stepper_row` :726 | F-STEP-LABELLED (icon_btn pair, no value) — button family only |

#### scene_panel.rs — helper `stepper_row` :680 (order A) + three B-order rows

| Row | Keys | Builder line | Value format |
|---|---|---|---|
| Amount (weather) | `wx-amt-dn/up` | :974 | `{:.0}`, `"—"` when off |
| Wind (weather) | `wx-wind-dn/up` | :994 | `"{}"` int, `"—"` when off |
| Stage Width | `stage-w-dn/up` | :1029 | `"{}"` int (±40 steps) |
| Stage Height | `stage-h-dn/up` | :1038 | `"{}"` int |
| Background R/G/B | `scene-bg-{r,g,b}-dn/up` | `bg_channel_row` :698, btns :717/:720 | order B; value u8, `text_secondary` + grows |
| Frame rate | `scene-fps-dn/up` | `fps_row` :725, btns :742/:745 | order B; `fps_label` |
| Stage W/H (fine) | `scene-stage-{w,h}-dn/up` | `stage_axis_row` :750, keys :760/:762 | order B; `"{}"` int (±10 steps). Duplicates the coarse Width/Height rows above it — a pre-existing panel redundancy, **not** Phase N's to merge |

Also: settings_popover.rs `stage_axis_row` :586
(btns :600/:606, private `readout` :556) — order A,
value `"{}"` int. Same pattern, fifth file; adopt with
the rest.

#### stroke_inspector.rs — three shapes in one panel

| Row | Keys | Builder line | Notes |
|---|---|---|---|
| Brush Size / Opacity / Scatter / Smoothing | `brush-{size,opacity,scatter,smoothing}-{dec,inc}` | `param_row` :1296, uses :1590–1650 | order B; ASCII `"-"` glyph (→ F-STEP glyph rule); values `width_text` / `opacity_text` / `scatter_text` / `smoothing_text` |
| Selected stroke Width | `stroke-width-dn/up` | row :1475, icon_only :1495/:1505 | F-STEP-CHEV (chevron-down/up, 26px); value `{:.1}` |
| Selected stroke Opacity | `stroke-opacity-dn/up` | row :1517, icon_only :1539/:1549 | F-STEP-CHEV; value `opacity_label` (%); row absent for spray strokes (gate stays) |
| Stroke selection pager | `stroke-sel-prev/next` | row :1430 | **pager, not a stepper** — button family only |

#### frame_ops.rs — no −/+ stepper exists; three labelled families

| Control | Keys | Builder line | Notes |
|---|---|---|---|
| Duration (main row) | `dur-dn` / `dur-up` ("Dur −" / "Dur +") | :731/:734 | F-STEP-LABELLED via local `fbtn` (retires onto `icon_btn`/`btn` per charter §4); readout is the `"Dur {}"` text at :728 — becomes `stepper_value` |
| This-frame duration | `fop-dur-half` (×0.5) / `fop-dur-double` (×2) / `fop-dur-reset` | row `frames-this` :748, btns :763–777 | F-STEP-LABELLED; `duration_label` readout at :758 becomes `stepper_value` |
| Insert-N count | `fop-ins-n-dn` / `fop-ins-n-up` ("-" / "+") | row `frames-insert` :780, btns :795/:810 | the panel's one true −/+ stepper (order B); value `"{} frames"` between the buttons; ASCII `"-"` glyph |

Note: frame_ops' keys `dur-dn`/`dur-up` collide in
*name* with playback_controls' `dur-dn`/`dur-up`
(:672) — different panels, pre-existing, both survive
byte-identical; the gate diffs key sets per tree, and
only one panel renders at a time. Recorded so nobody
"fixes" it in this phase.

#### layers_panel.rs

| Row | Keys | Builder line | Notes |
|---|---|---|---|
| Per-row figure opacity | `layer-op-dec-{fi}` / `layer-op-inc-{fi}` | `layer_row` :470, btns ~:505/:517 | order C-with-name (name button, spacer, −, %, +); ASCII `"-"`; value `"{}%"`, `t.text` |
| Selected opacity readout | — (readout for the slider) | row `layers-opacity-row` :570, slider :581 | value `opacity_label` → `stepper_value` beside the F-SLIDER |

#### particles_panel.rs — detail block

| Row | Keys | Builder line | Notes |
|---|---|---|---|
| Kind | `particles-kind-prev/next` | row :580, icon_only :592/:607 | F-STEP-CHEV (chevron-left/right); value is a word (`kind_name`, BODY font today — the panel's one BODY value) |
| Frame | `particles-frame-dn/up` | row :625, btns :638/:647 | order B; ASCII `"-"`/`"+"` labels ("-" / "+"); value `frame + 1` int, BODY today |

#### fx_clips.rs — inspector + nudge row

| Row | Keys | Builder line | Notes |
|---|---|---|---|
| Kind | `fx-kind-prev/next` | `fx-kind-row` :1311 | F-STEP-CHEV; word value |
| Start | `fx-start-dec-{i}` / `fx-start-inc-{i}` | :1342/:1350 | order B; ASCII glyphs; value = frame int |
| Duration | `fx-dur-dec-{i}` / `fx-dur-inc-{i}` | :1365/:1373 | order B; ASCII glyphs |
| Nudge | `fx-nudge-l10` (−10) / `fx-nudge-l` / `fx-nudge-r` / `fx-nudge-r10` (+10) | `nudge_row` :1207 | F-STEP-MULTI (mixed text + chevron icon_only) |
| Duration quick | `fx-dur-dn` (−0.1s) / `fx-dur-up` (+0.1s) | in `fx_inspector` :1266 block | F-STEP-MULTI (second-valued steps) |

#### camera_panel.rs

| Row | Keys | Builder line | Notes |
|---|---|---|---|
| Zoom | `cam-zoom-out` / `cam-zoom-in` ("-" / "+") | panel row :933, btns :937/:941 | order C; the readout is a **chip element** (`cam-zoom-label` :748, px 6, centred text) — becomes `stepper_value` inside the same keyed chip |
| Rotation | `cam-rot-left` / `cam-rot-right` ("-15°" / "+15°") | row :946, btns :949/:953 | F-STEP-MULTI; readout chip `cam-angle-label` :758 (`angle_label` → `"{:.0}°"`) |
| Stage overlay cluster | same keys, second builder | `overlay` :979 | the same zoom stepper + chips built again for the stage overlay (charter §14 makes it a floating pill) — both builders adopt together or the two copies drift |

Adjacent, same pattern, other owners' files (adopt if
their briefs say so; listed so the pattern is counted
once): timeline_marks loop steppers `loop-{a,b}-{dn,up}`
(:938–947), onion_controls `onion-{prev,next}-{dn,up}`
(:130–154), three_d_panel zoom steppers
`d3-zoom-{dn,up}`.

### 3.2 Sliders

| Instance | Key | File:line | Handler | Pattern |
|---|---|---|---|---|
| Weather Amount | `wx-amt-slider` | scene_panel.rs:986 (builder `weather_slider` :795) | `amount_drag` :369 | F-SLIDER, unipolar (fill from 0.0) |
| Weather Wind | `wx-wind-slider` | scene_panel.rs:1006 | `wind_drag` :405 | **F-SLIDER-BI bespoke variant** — fill from centre 0.5, centre tick (:831); see §2.2 / §6 |
| Figure opacity | `layers-opacity-slider` | layers_panel.rs:581 (builder `opacity_slider` :363) | `opacity_drag` :323 | F-SLIDER, unipolar |
| Camera angle dial | `cam-dial` | camera_panel.rs:734 (`dial_shape` :701) | `dial_drag` :688 → `dial_gesture` :644 | **BESPOKE — §6.1** |

Premise correction: **particles_panel has no slider.**
Its detail block is steppers + swatches only (§3.1,
§3.4); the `control_track` hits in that file (:791,
:796) are list-row shape strokes, not controls. The
brief's "fx/particles sliders" resolves to: fx_clips'
strip (below) and nothing in particles.

Not sliders — drag surfaces that must **not** be
converted to `slider_track` (they are canvases whose
drag means select / seek / orbit / draw, not "set a
fraction"): fx_clips strip `fx-strip`
(fx_clips.rs:1097, `strip_press` :970 — press selects a
clip bar or seeks), waveform seek (waveform_view.rs:
1095/:1106), graph canvas (graph_panel.rs:1778), 3D
preview orbit (three_d_panel.rs:799), stage drag.

### 3.3 Text fields — all 13 `text_input` sites

State field line = where the `TextEdit` lives. "Bare" =
no fill / border / radius on the field today.

| # | Field | Key | File:line (builder) | State | Font today | Frame today | Variant |
|---|---|---|---|---|---|---|---|
| 1 | Home search | `home-search` | home_panel.rs:1174 | `HomePanelState.search` :219 | BODY.lh(26) | bare; icon 12 at :1171 | SEARCH |
| 2 | Library search | `library-search` | library_panel.rs:914 | `LibraryPanelState.search` :254 | BODY.lh(26) | bare; icon 14 at :911 | SEARCH |
| 3 | Library rename | `library-rename` | library_panel.rs:762 | `LibraryPanelState.rename` :259 | BODY.lh(26) | bare; Apply/Cancel icon_btns | NAME |
| 4 | Figures filter | `figures-filter` | figures_panel.rs:698 | filter state :131 | BODY.lh(26) | bare; icon 14 at :695 | SEARCH |
| 5 | Text content | `text-input` | text_tool.rs:1229 | `TextToolState.field` | BODY.lh(26) | **framed**: fill card, radius 6, px 8, py 4, w_full | NAME (content entry; the F-FIELD frame's source) |
| 6 | Project name | `proj-name` (wrapper `proj-name-field`) | project_tools.rs:754 (wrapper :746) | `State.name_edit` | BODY | **framed by wrapper**: h 32, px 8, radius 6, fill card, border card_border | NAME |
| 7 | Clip rename | `clip-rename-field` | clips_panel.rs:773 | `ClipRenameState.field` :179 | BODY | bare; Apply/Cancel | NAME |
| 8 | Figure rename | `fig-name` | inspector_figure.rs:625 | `InspectorFigureEditState.name` :470 | BODY | bare; Rename icon_btn; poll-synced | NAME |
| 9 | Palette query | `palette-query` | command_palette.rs:1533 | `PaletteState.query` :1361 | BODY.lh(26) | bare inside 52px header row (px 16, ⌕ glyph) | QUERY |
| 10 | Audio clip name | `audio-name` | audio_panel.rs:906 | `AudioPanelState.name` :110 | BODY | bare; Add clip icon_btn; long placeholder | NAME |
| 11 | FX filter | `fx-filter` | fx_browser.rs:518 | `FxBrowserState.query` :315 | BODY.lh(26) | bare; icon 12 at :515 | SEARCH |
| 12 | Pose rename | `pose-rename` | pose_tools.rs:1120 | `PoseRenameState.field` | BODY.lh(26) | bare; Apply/Cancel | NAME |
| 13 | Pose library filter | `pose-lib-filter` | pose_tools.rs:1343 | `PoseLibraryState.filter` | BODY.lh(26) | bare; icon 14 at :1340 | SEARCH |

All 13 are `.single_line()`, `.color(t.text)`, with
`t.text_secondary` placeholders where a placeholder
exists (figure rename has none). `on_submit` exists on
#2 (Enter inserts), #5, #6, #7, #8, #10, #12 — and not
on #3 (Apply button only). That asymmetry is behaviour;
Phase N does not add or remove submits.

### 3.4 Swatch grids

| Grid | File:line (builder / data) | Count | Geometry today | Selected mark today | Target |
|---|---|---|---|---|---|
| Segment colour | inspector_segment.rs `swatch` :662, row :858; `PALETTE` :173 | 6 | 22×22, radius 6, key `seg-color-{rgb:06x}` | white border 2px | F-SWATCH (A) |
| Figure colour | inspector_figure.rs `swatch` :827, rows :888 (chunks of 8); `PALETTE` :178 | 16 | 22×22, radius 6, key `fig-color-{rgb:06x}` | white border 2px | F-SWATCH (A) |
| Style gradient states | inspector_style.rs `gradient_swatch` :698, row :847 | 3 (incl. Off = `control_off` fill) | 18×18 core radius 4 in 2px-pad cell radius 6, key `style-gradient-{state}` | accent-filled cell | F-SWATCH (A); Off keeps its `control_off` core — it is a *value*, not an empty slot |
| Style recent colours | inspector_style.rs `recent_chip` :740, `recents_row` :765 | ≤8, dynamic | same 18+cell construction, key `style-recent-{i}` | accent-filled cell | F-SWATCH (A) |
| Scene background | scene_panel.rs :883; `BACKGROUND_SWATCHES` :597 | 16 (named) | **dialect A already**: 26 slot radius 8 / 22 core radius 6, key `bg-swatch-{name}`, chunks of 8 | accent slot | canonical — no change beyond grid gap audit |
| Scene bg presets | scene_panel.rs :912; `BG_PRESETS` :634 | 6 (named) | dialect A, key `scene-bg-preset-{i}` | accent slot | canonical; note the names are data-only — chips show colour alone today and stay that way (labels are P10's call, not forms) |
| Particle colour | particles_panel.rs :526; `PARTICLE_PALETTE` :140 | 8 | dialect A, key `particles-color-{ci}` | accent slot | canonical |
| Text colour | text_tool.rs `text_color_row` :983; `TEXT_PALETTE` :242 | 8 (+ custom readout chip) | dialect A, key `text-color-{ci}` | accent slot | canonical |
| FX clip colour + end colour | fx_clips.rs `swatch` :987, `swatch_rows` :1001; `SWATCHES` :172; rows :1468 / :1501 | 16 ×2 | 18×18, radius 5, keys `fx-color-{i}-{hex:06x}` / `fx-color2-…` | white **outline** (2,1) | F-SWATCH (A) — the outline dialect's last user |

Palette identity note: `PARTICLE_PALETTE` and
`TEXT_PALETTE` are the same 8 colours (test-pinned
equal in both files) — one language already; the other
palettes (6 / 16 / 16 / 16) are panel-local by design
and Phase N does not merge them.

### 3.5 Segmented and chip rows

| Group | File:line | Keys | Segments | Gap today | Active rendering today |
|---|---|---|---|---|---|
| Clip loop mode | clips_panel.rs `loop_mode_row` :806, btns :825/:836/:847 | `clip-loop-loop` / `clip-loop-pingpong` / `clip-loop-once` | Loop · Ping-pong · Once (+ "Loop mode:" label) | 8 | `btn` active |
| Playback FPS presets | playback_controls.rs preset row :596, btns :600 | `preset_key(p)` per `FPS_PRESETS` | fps values | 6 | `btn` active (`is_active_preset`) |
| Settings speed chips | settings_popover.rs `speed_row` :620, btns :624 | `settings-speed-{i}` ×5 | `SPEEDS` labels "0.25×"…"2×" | 6 | `btn` active (`active_speed_index`) |
| Workspace chips | workspaces.rs `workspace_row` :447, btns :451 | `ws-*` ×8 | label + "· Alt+n" suffix | 4 | `btn` active |
| Style cap | inspector_style.rs `cap_chip` :645, row :831 | `style-cap-{round,square,point}` | Round · Square · Point | 4 | **custom el** (accent / card fill, hover accent / control_track) — retires onto `btn` active/inactive, same keys |
| Weather kind | scene_panel.rs kind row :863 | `wx-seg-{i}` ×6 (Off + 5 kinds) | icon_btn with kind icons | 4 | `icon_btn` active |
| Brush nib | stroke_inspector.rs nib row :1613 | `nib_key(nib)` ×3 | Round · Square · Calligraphy (labels per `nib_label`) | 8 | `btn` active |
| Stage size presets | scene_panel.rs :938 (chunks of 3) | `scene-stage-{w}x{h}` ×6 | size labels | 4 | `btn` active (`stage_matches_preset`) — a preset **grid**, wraps 3×2; F-SEG metrics, grid layout stays |
| Settings language / startup-workspace | settings_popover.rs `language_button` :561 / `workspace_button` :573 | per-entry keys | full grids | 6 | `btn` active — picker grids, same treatment as stage presets |

Premise correction: **playback_controls has no
direct-select speed segmented row.** Speed there is the
`pb-speed` **cycler** (icon_btn, gauge glyph,
playback_controls.rs:545) over the `SPEEDS` table
(:288); the direct-select chips are in settings_popover
(above). The segmented row in playback_controls is the
FPS preset row. Both are censused; neither changes
behaviour.

Toggle-shaped relatives (`pb-loop`, `tween`,
`cam-follow`, loop-start/end in two panels) are charter
§7 toggles, not F-SEG — they take the kit `toggle` /
`toggle_row` from the charter, owned by the toggle
spec; listed here only so nobody double-adopts them.

---

## 4. Drift summary (what Phase N actually fixes)

| Axis | States found | Rule |
|---|---|---|
| Stepper child order | A (2 helpers) vs B (~15 rows) vs C (3) | charter order A (§2.1) |
| Step glyph | "−" (U+2212) vs ASCII "-" in 5 families | "−" / "+" |
| Value font | LABEL everywhere | `stepper_value` (MONO_CAPTION.mono()) |
| Value colour | `t.text` vs `text_secondary`, no pattern; two values *grow* (style rows) | `t.text`, centred, min-w 34 |
| Value font size outlier | BODY (particles kind + frame) | MONO_CAPTION via kit |
| Slider track / knob | `control_off` / `t.text` in both sliders | kit: `control_track` / `t.knob` (charter §8) |
| Search icon size | 12 (home, fx) vs 14 (library, figures, poses) | 13 (kit `ICON_SIZE`) |
| Field line height | BODY.lh(26) vs BODY, split by file | one frame, BODY (frame sets the height) |
| Field framing | framed ×2, bare ×11 | F-FIELD frame on all |
| Swatch geometry | 26-slot A vs 22 B vs 18 B vs 18+cell | dialect A (26/22, radius 8/6, gap 4) |
| Swatch selected mark | accent slot vs white border vs white outline vs accent cell | accent slot |
| Segmented gap | 4 / 6 / 8 | 4 |
| Segmented builder | `btn` vs `icon_btn` vs custom el (cap) | button family (charter §4) |

## 5. Decisions the F wave / integrator must take (once)

- **D1 — Stepper order.** Charter A vs majority B
  (§2.1). Spec assumes A. Either way, one order
  everywhere; the census records both per row.
- **D2 — Bipolar slider.** Extend `slider_track` with
  an origin/tick, or bless F-SLIDER-BI as a permanent
  local variant for wind (§2.2). Do not drop the tick.
- **D3 — Workspace chip suffix.** The `· Alt+n` suffix
  is plain text inside the `btn` label because `btn`
  renders one text run (workspaces.rs:426, documented
  in-file). Charter §11 wants kbd chips for hints.
  Options: leave the suffix (labels are composed
  locally at :451, not persisted — but snap tests pin
  the composed label), or F wave extends the chip to
  carry a `kbd` element. **Recommendation: leave it in
  Phase N**; the suffix is quiet, and touching it risks
  the pinned label tests for a marginal gain.
- **D4 — Head-ring label split** (inspector_figure
  :766): the value lives inside the label string.
  Splitting label/value is a text-structure change
  (formatter unchanged). Recommended: split — it is
  the only stepper whose value cannot take
  `stepper_value` as built.

## 6. Bespoke visuals — keep, and why

### 6.1 Camera angle dial (`cam-dial`, camera_panel.rs:734) — KEEP BESPOKE

The dial is not a slider with a different skin:

- Its domain is **absolute angle** −180°..+180° with
  five fixed ticks (taller at 0°) and an accent
  **diamond** marker (`dial_shape` :701, view box
  360×26). `slider_track(frac)` has no ticks, no
  absolute scale, and a dot knob.
- Drag maps pointer x to an **absolute target angle**
  (`angle_at`), not a fraction of a range the user
  thinks of linearly — and the gesture has its own
  undo contract (one snapshot per gesture, 0.01° dead
  zone, cancel restores via undo; `dial_gesture`
  :635). Charter §8 itself says drag logic stays
  where it lives; here the *visual is* the logic's
  scale. Converting it would delete information.
- It already speaks the token language where it
  should: track `t.control_track`, ticks
  `t.text_secondary`, marker accent. Phase N's only
  dial change: the readout chips beside it
  (`cam-angle-label`, `cam-zoom-label`) take
  `stepper_value`, in both builders (panel :905,
  overlay :979).

### 6.2 Wind slider (F-SLIDER-BI) — keep the centre-origin visual

See §2.2 / D2. Bipolar fill + centre zero tick are
meaning, not decoration.

### 6.3 Everything else bespoke-looking is not

The FX strip, waveform, graph canvas and 3D orbit are
canvases (§3.2), not form controls in disguise — they
are out of this spec's scope entirely, and no kit
pattern in the charter describes them.

## 7. Work list for the A wave (file-scoped)

| File | Adopt |
|---|---|
| inspector_segment.rs | `stepper_row` → F-STEP (one helper fixes 2 rows); Rotate/Bend readouts → `stepper_value`; swatch → F-SWATCH |
| inspector_style.rs | `channel_row` + `gradient_channel_row` + opacity row → F-STEP; `cap_chip` → `btn` F-SEG; gradient/recent swatches → F-SWATCH; taper row untouched beyond buttons |
| inspector_figure.rs | opacity + ring rows → F-STEP (ring per D4); swatch → F-SWATCH; `fig-name` → F-FIELD-NAME |
| scene_panel.rs | `stepper_row` already order A — values → `stepper_value` only; bg/fps/stage-axis rows → F-STEP; `weather_slider` → F-SLIDER (amt) / F-SLIDER-BI (wind); kind row → F-SEG gap stays 4; stage presets F-SEG metrics; bg swatches already canonical |
| stroke_inspector.rs | `param_row` → F-STEP (glyph fix); width/opacity rows → F-STEP-CHEV values; nib row → F-SEG gap 4 |
| frame_ops.rs | Insert-N → F-STEP; duration readouts → `stepper_value`; `fbtn` family retires per charter §4 (foundation's job, this file is its biggest consumer) |
| layers_panel.rs | per-row steppers glyph + value; slider → F-SLIDER; slider readout → `stepper_value` |
| camera_panel.rs | zoom row value chip → `stepper_value` (both builders); rotation F-STEP-MULTI readout; **dial untouched** (§6.1) |
| particles_panel.rs | kind → F-STEP-CHEV, frame → F-STEP (glyph + value font); swatches already canonical |
| fx_clips.rs | kind/start/duration → F-STEP family; swatches → F-SWATCH (outline dialect retired) |
| clips_panel.rs | loop row → F-SEG gap 4; rename field → F-FIELD-NAME |
| playback_controls.rs | FPS preset row → F-SEG gap 4; duration stepper readout → `stepper_value`; `pb-speed` cycler untouched |
| settings_popover.rs | speed row → F-SEG gap 4; stage rows → F-STEP values (`readout` :556 retires onto `stepper_value`) |
| workspaces.rs | chips → F-SEG metrics (gap already 4); suffix per D3 |
| home_panel.rs | search → F-FIELD-SEARCH (icon 13) |
| library_panel.rs | search → F-FIELD-SEARCH; rename → F-FIELD-NAME |
| figures_panel.rs | filter → F-FIELD-SEARCH |
| fx_browser.rs | filter → F-FIELD-SEARCH |
| pose_tools.rs | filter → F-FIELD-SEARCH; rename → F-FIELD-NAME |
| text_tool.rs | field already framed — align to F-FIELD metrics; size rows (F-STEP-LABELLED) readouts → `stepper_value` |
| project_tools.rs | wrapper frame metrics → F-FIELD on the field; wrapper key `proj-name-field` survives |
| audio_panel.rs | name field → F-FIELD-NAME |
| command_palette.rs | QUERY variant — header row only, no field frame (§2.3) |

## 8. Verification notes for the V wave

- Every key in §3 must survive byte-identical; the
  census tables are the checklist. New keys: **none**
  in this spec (D4's split reuses `fig-ring-dn/up`;
  the ring row already has key `fig-ring-row`).
- Glyph changes (§2.1) touch button labels in
  stroke_inspector, particles_panel, layers_panel,
  fx_clips, camera_panel — grep those files' tests for
  label assertions before pushing.
- Runtime round-11 candidates: one press per F-STEP
  family (value changes, key set unchanged), one
  F-SLIDER drag per slider (value + release status
  unchanged), F-FIELD typing in one SEARCH and one
  NAME field (filter/rename behaviour unchanged).
- Headless tree probes can assert the frame on
  `home-search` / `fig-name` once fields carry
  fill/border (visible in `--tree` vis fields).
