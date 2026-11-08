# P18 — Hit-target & readability audit (Phase N planning)

Scope: every pressable, draggable or focusable control in
the Rune shell, measured against one house rule, plus a
readability pass over the type scale. No code changes in
this plan — it is the size census the F/A waves adopt
against.

## Method

Two sources, cross-checked:

1. **Builder arithmetic.** Text renders at a 1.25 line
   factor (measured, below): `type_::LABEL` (12 pt) lays
   out **15.0** high, `BODY`/`TITLE` (13 pt) **16.25**.
   A `px 10 / py 5` button is therefore
   5 + 15 + 5 = **25 high** before any content is added.
2. **Measured rects** from the Phase M headless tree
   dumps (run 42479090089, 1440×900 window), elements
   with `role: "button"` plus the keyed drag surfaces.
   Where a family appears in a tree, the measured number
   is quoted; where no headless scene reaches it
   (Audio/Export/Home workspaces), builder arithmetic is
   quoted and marked *(calc)*.

## The house rule (state it plainly)

- **24 px minimum** — the smaller dimension of a square
  target, the height of a wide one.
- **28 px comfortable** — where a control is a row, a
  primary action, or used repeatedly.

This is a **house rule, not a law**: it matches WCAG
2.5.8 (AA, 24 CSS px) at the floor and the charter's own
list-row/menu numbers at the comfortable tier, but the
charter deliberately fixes the button family at
`px 10 / py 5` (25 high) — density was chosen for the
button tier, comfort for rows. The audit therefore
judges each family against the tier the charter assigns
it, and flags only what falls below **24**, or below
its charter tier.

## Master size table

| Family | Builder (file:line) | Size (measured unless *(calc)*) | Verdict |
|---|---|---|---|
| Text button `btn` | `chrome_view.rs:38` (px 10, py 5, r 6) | h **25**; width = label + 20 (narrowest: `cam-zoom-out` 26 wide) | ✅ ≥24 · below 28 by charter design |
| Icon+label button `icon_btn` | `rune_sec/icon_btn.rs:155` (mirrors `btn`; glyph `ICON_SIZE` 13, `icon_btn.rs:66`) | h **25** | ✅ same tier as `btn` |
| Icon-only button `icon_only_btn` | `rune_sec/icon_btn.rs:230` | **26 × 26 at every production call site** — 33 sites parsed across 12 files, all pass `26.0` (chevron steppers, `fig-lock-{fi}`, `fig-eye-{fi}`, toast dismiss `toasts.rs:406`, library delete, fx dup/del…) | ✅ the most consistent family in the shell |
| Local `fbtn` copies (frame_ops etc.) | retired onto `btn` by the charter | h 25 *(calc)* | ✅ after retirement |
| Steppers (btn form: `−` / `+` pairs) | panels pass `btn`/`fbtn` with 1-char labels (scene, inspector, frame_ops, d3 zoom…) | **28.5 × 25** | ⚠ height is the button tier (25); charter §6 grows the *row* (value in MONO_CAPTION, min-w 34) — the buttons themselves stay 25 unless the kit stepper pads them |
| Figure-card mini toggles `row_toggle` | `rune_sec/figures_panel.rs:380` (px 6, py 3, r 5) | `fig-op-dn/up-{fi}` **20.5 × 21**, `fig-solo-{fi}` 36 × 21, `fig-opac-{fi}` 41 × 21 | ❌ **below 24 in both dimensions** (height 21; opacity steppers also 20.5 wide) |
| Figure select row `fig-sel-{fi}` | `rune_sec/figures_panel.rs` (~:532; press row, no padding, no height) | **~42 × 15** (width varies with siblings; height is the bare text line) | ❌ **the main per-figure target is 15 high** |
| Frame chips `frame-{n}` | `rune_sec/timeline_marks.rs:268` `CHIP_W 30 / CHIP_H 24 / CHIP_GAP 4`; legacy duplicate `chrome_view.rs:193` (`timeline_row`, unmounted) | **30 × 24** | ⚠ exactly at the 24 floor; height is **pinned** (see Pins) — do not grow |
| Timeline markers `marker-cam-{f}` / `marker-graph-{f}` | `rune_sec/timeline_marks.rs:868` (cell `CHIP_W × MARKER_H 14`, `:548`); press `el()` wraps only the glyph | **≈11 × 15** (measured `marker-graph-0`) inside a 30 × 14 cell | ❌ **smallest press target in the app** |
| Timeline ruler (drag-seek) | `rune_sec/timeline_marks.rs:812` (`on_drag` on the row; `RULER_H 12`, `:552`) | full-width × **12** | ❌ drag strip 12 high; **pinned** — mitigation only (see work list) |
| Timeline clips lane `timeline-clips` | `rune_sec/timeline_marks.rs:959` `CLIPS_H 10` | 10 high, **display only** (no handler) | — not a target |
| Loop-row / transport / app-bar / workspace chips | `btn` family | h **25** | ✅ button tier |
| Camera dial `cam-dial` (drag) | `rune_sec/camera_panel.rs:135` `DIAL_W 360 × DIAL_H 26` (`:138`); element is column-width × 26 | **26** drag height | ✅ ≥24; growing is test-pinned (see Pins) — leave |
| Sliders (layers opacity, weather amount/wind) | `rune_sec/layers_panel.rs:276` + `rune_sec/scene_panel.rs:784` (duplicated consts `SLIDER_W 200 / SLIDER_H 22 / KNOB 16`) | drag root **200 × 22**, knob Ø **16** | ❌ drag height 22 < 24; knob small. Drag maps **x only**, so growing the root height is safe (work list) |
| FX strip `fx-strip` (press/drag seek) | `rune_sec/fx_clips.rs:405` `LANE_H 14`; `strip_view` `:1099`, `on_drag` `:1106` | **14 per lane** | ❌ below floor; pinned by strip tests — optional growth, integrator call |
| Colour swatches — FX | `rune_sec/fx_clips.rs:990` `swatch()` size 18 | **18 × 18** (`fx-color-{i}-{hex}`, `fx-color2-{i}-{hex}`) | ❌ **worst swatch family** |
| Colour swatches — figure / segment | `rune_sec/inspector_figure.rs:827`, `rune_sec/inspector_segment.rs:658` (size 22) | **22 × 22** (`fig-color-{hex}`, `seg-color-{hex}`) | ❌ below 24 |
| Colour swatches — style recents / gradient | `rune_sec/inspector_style.rs:739` `recent_chip` (18 swatch + padding 2 → ~22 cell); `:698` `gradient_swatch` (18) | **≈22 cell / 18 swatch** | ❌ below 24 |
| Colour swatches — palettes with ring cells (scene bg, particles, text) | `rune_sec/scene_panel.rs:891`, `rune_sec/particles_panel.rs:535`, `rune_sec/text_tool.rs:999` (26 cell, 22 colour) | **26 × 26** | ✅ ≥24 |
| List rows — movieclips `clip-row-{i}` | `rune_sec/clips_panel.rs:651` (px 8, **py 4**) | h **23** *(calc: 4+15+4)* | ❌ **only list row below 24**; charter §5 tier is 28 |
| List rows — audio clips `audio-clip-{i}` | `rune_sec/audio_panel.rs:~716` (px 8, py 5) | h **25** | ⚠ ≥24, below charter §5's 28 |
| List rows — FX clips / graph channels / layers | full-width `btn`-style rows (`fx-clip-{i}`, `graph-chan-{i}`, `layer-row-{fi}`) | h **25–26** | ⚠ same: grow to 28 via kit `list_row` |
| List rows — library `library-row-{i}` | `rune_sec/library_panel.rs:785` (no own padding; height set by its `icon_btn`/`icon_only_btn` children) | h **25–26**, no whole-row press (actions are the row's buttons) | ✅ targets inside are compliant |
| List rows — home projects `home-row-{i}` | `rune_sec/home_panel.rs:~1050` (px 10, py 6, two-line body) | h **≈45** | ✅ comfortable |
| Figure card `fig-row-{fi}` | `rune_sec/figures_panel.rs` | **300 × 80** overall; line 1 is 44 (the `THUMB_SIZE 44` canvas, `:223`) | ✅ card is generous — its *children* are the problem (above) |
| Menu items | `rune_sec/menus.rs:338` `item_icon` (px 10, py 5) / plain items fall back to `btn` | h **25** | ⚠ charter §9 tier is **28** — grow py 5 → 6.5 in the menus adoption |
| Command palette rows `cmd-{key}` | `rune_sec/command_palette.rs:~1585` (`h(34)`, px 10, r 8) | h **34** | ✅✅ best rows in the app; the model for §5/§9 |
| Text fields (all `text_input`: project name, figure name, filters, search…) | e.g. `rune_sec/inspector_figure.rs:618` — kit field, `BODY`, **no frame, no padding of its own** | field **≈16.25 high**, frameless | ❌ focus/click target is one text line; charter allows framing the kit field (work list) |
| Toast actions / dismiss | `btn` (h 25) + `icon_only_btn` 26 (`toasts.rs:406`) | 25 / 26 | ✅ |
| Stage joint handles (drag) | engine `EditorCore::hit_test`, `src/editor.rs:289` — disc radius **13 stage units**; stroke pick `PICK_TOLERANCE 13` (`stage_layers.rs:620`) | Ø26 stage units ≈ **Ø19 screen px at fit zoom** (stage 960×720 in 744×522 → scale ≈0.73), Ø26 at 100% | ⚠ report-only: engine hit-testing, outside Phase N's no-behaviour-change rule |
| Graph key grab | `rune_sec/graph_panel.rs:335` `GRAB_RADIUS 10` | Ø **20** data-space px | ⚠ report-only: interaction constant with hit tests; not a styling change |
| Waveform / graph canvases (drag) | `waveform_view.rs` (fills panel), `graph_panel.rs:313` `GRAPH_H 170` | tall drag surfaces | ✅ |

### Worst offenders, ranked

1. **Timeline markers ≈11 × 15** — `timeline_marks.rs:868`.
   The pressable element is a bare `el()` around one
   glyph inside a 30 × 14 cell.
2. **Figure opacity steppers 20.5 × 21** —
   `row_toggle`, `figures_panel.rs:380` (call sites
   `fig-op-dn/up-{fi}`).
3. **Figure select row ~42 × 15** — `figures_panel.rs`
   (~:532). The single most-used per-figure press.
4. **FX colour swatches 18 × 18** — `fx_clips.rs:990`.
5. **Text fields ≈16.25, frameless** — every
   `text_input` (e.g. `inspector_figure.rs:618`).
6. **Ruler drag strip 12 high** — `timeline_marks.rs:552`
   (pinned; see below).
7. **Movieclip rows 23 high** — `clips_panel.rs:651`.
8. **Sliders 22 high, knob Ø16** — `layers_panel.rs:276`,
   `scene_panel.rs:784`.
9. **FX strip lanes 14** — `fx_clips.rs:405` (pinned-ish).
10. **`row_toggle` siblings 36–41 × 21** (`fig-solo`,
    `fig-opac`) — same helper as #2.

### Display-only small elements — NOT targets, do not grow

`layer-dot-{fi}` 10 × 10 (`layers_panel.rs:475`), the
figure-card colour dot 10 × 10 (`figures_panel.rs:537`),
`scene-bg-preview` 18 × 18 (`scene_panel.rs:1078`),
style channel previews 12 × 12 (`inspector_style.rs:574`),
the stroke colour chip 14 × 14
(`stroke_inspector.rs:1685`), status-strip ticks
1 × 12 (`status_strip.rs:820`), the clips lane (above).
None carries a handler; listing them here so an adoption
agent does not "fix" decoration.

## Charter compliance map

- **§4 buttons (px 10 / py 5, r 6) → 25 high.** Compliant
  with the house floor (24), below comfortable (28) —
  the charter's explicit choice. No adoption work;
  do not pad buttons locally.
- **§5 list rows min-height 28.** **Not yet true
  anywhere except the palette (34) and home (≈45).**
  Adoption must grow: movieclips 23 → 28, audio/FX/
  graph/layer rows 25–26 → 28. The kit `list_row`
  shell is the vehicle; heights come from the shell,
  not per-file py.
- **§6 steppers.** Value readout (MONO_CAPTION, centred,
  min-w 34) does not exist yet — values are plain LABEL
  text between buttons. Buttons stay 25 unless the kit
  stepper row pads the whole row to 28; recommend the
  kit row carries min-height 28 so a stepper row reads
  as one 28-high target band.
- **§7 toggles.** No pill toggle exists yet; toggle-like
  controls are `btn` chips (25) and `row_toggle` (21).
  The kit `toggle_row` must keep its key on a
  **row-height pressable root (≥28)** — the 28 × 16 pill
  is the *visual*, never the target.
- **§8 sliders.** Kit `slider_track` is visual only;
  adoption must also grow the **drag root** from
  `SLIDER_H 22` to ≥24 (28 preferred) in both slider
  homes. Safe: both drag handlers map x → frac and
  never read y; the track/knob stay centred by the
  existing `(SLIDER_H − 4) / 2` arithmetic if the
  constant itself grows.
- **§9 menus/popovers.** Menu items are 25; charter
  tier is 28 (item rows 28 high, px 8). Grow in the
  menus foundation work, not per panel. Palette rows
  (34) already exceed the tier — leave them.
- **§13 timeline card.** Chips keep geometry (30 × 24,
  floor-exact). The card's padding 8 adds 16 to the
  timeline stack — see Pins: this collides with the
  pinned stage bounds and needs one integrator
  decision, not per-agent improvisation.
- **§12 status strip.** Restyle only (ticks, mono
  numbers); the strip stays **15 high** — its height
  and floor arithmetic are pinned (below).
- **§14 stage overlay pill.** Overlay buttons inside
  stay `btn` (25); the pill (py 4 around them) is a
  container, not a target.

## Pinned geometry — what must not move (or only by integrator decision)

- **Timeline constants** — `CHIP_W 30 / CHIP_H 24 /
  CHIP_GAP 4` (`timeline_marks.rs:268-272`),
  `MARKER_H 14` (`:548`), `RULER_H 12` (`:552`),
  `CLIPS_H 10` (`:959`): consumed by `span_rect`,
  `timeline_width`, the clips-lane geometry and the
  ruler labels, and pinned by timeline_marks' in-file
  tests. The same 30 × 24 is hard-coded in the legacy
  `chrome_view::timeline_row` (`chrome_view.rs:193`,
  currently unmounted — keep the two in sync if either
  ever changes).
- **Centre-column layout** — stage rect
  **[328, 144, 744, 522]** and status strip
  **[328, 845, 744, 15]** at 1440×900, asserted by
  snap_tests' geometric tests (Phase J §37 set, incl.
  `status_strip_is_not_covered_by_the_right_column`,
  `snap_tests.rs:1873`) and the headless scene layout
  facts. Any growth in the timeline stack (card
  padding §13, a taller ruler/marker row) or the app
  bar/workspace rows (§15 hairlines) takes height from
  the stage and breaks these pins — re-pinning is an
  integrator decision taken once, with the new numbers
  stated in PHASE_N.md.
- **Status-strip floors** — `status_strip.rs` segment
  floors are computed from `est_text_width`
  (LABEL_CHAR_W 8.0) at the 744 width (Phase F/K
  arithmetic, incl. the Audio-glyph refusal). Restyle
  (§12) must not widen segments.
- **Button mirroring** — `icon_btn`'s tests pin its
  metrics to `btn`'s (`icon_btn.rs` tests:
  `state_fills_mirror_the_kit_button` etc.); `ICON_SIZE
  13` is pinned there too. Kit work changes both
  builders together or neither.
- **FX strip** — `LANE_H 14` / `strip_height` are
  pinned by fx_clips' strip tests (`strip_windows` /
  `pack_lanes`); growing lanes re-pins them.
- **Camera dial** — `DIAL_W 360 / DIAL_H 26` are pinned
  by camera_panel's dial tests (`angle_at` /
  `marker_x` inverses, marker at `DIAL_H − 3`).
  26 ≥ 24: leave.
- **Slider constants** — `SLIDER_W / SLIDER_H / KNOB`
  are used by layers/scene slider tests (track-ends,
  travel = `SLIDER_W − KNOB`). Growing `SLIDER_H`
  only moves the vertical centre — those tests read
  x-travel; still, change both duplicated copies
  (`layers_panel.rs:276`, `scene_panel.rs:784`)
  together or the two sliders drift.
- **Keys** — growing any target changes no key; the
  Phase N gate diffs key sets (733 baseline). Every
  growth below is padding/size on the same keyed root.

## Adoption work list (file-scoped)

1. **`rune_sec/figures_panel.rs`** — retire `row_toggle`
   (`:380`) onto the kit stepper/toggle: `fig-op-dn/up`
   grow 20.5 × 21 → ≥24 wide × 28 row band; `fig-solo`,
   `fig-opac` likewise. Give `fig-sel-{fi}` a real
   target: stretch the press row to the card line's
   height (44) or set min-height 28 with py — the row
   already sits in a 44-high line, so stretching costs
   no layout height.
2. **`rune_sec/timeline_marks.rs`** — markers: put
   `.size(CHIP_W, MARKER_H)` on the pressable `el()`
   itself (`:868`) so the target becomes the full
   30 × 14 cell (≈2.7× the area) without touching
   `MARKER_H`. Ruler: **no growth** (pinned); the
   mitigation is that chips directly below seek on
   press — record the 12-high drag strip as an
   accepted, pin-driven exception in PHASE_N.md.
3. **`rune_sec/fx_clips.rs`** — `swatch()` 18 → 24
   (`:990`); both colour rows reflow (8 per row still
   fits 340: 8 × 24 + gaps < 316 inner width). Strip
   lanes: leave at 14 unless the integrator re-pins.
4. **`rune_sec/inspector_figure.rs` /
   `rune_sec/inspector_segment.rs`** — swatch size
   22 → 24 (`:827` / `:658`); frame the name field
   (below).
5. **`rune_sec/inspector_style.rs`** — `recent_chip`
   cell → 24 (swatch 20 + padding 2, or swatch 18 +
   padding 3); `gradient_swatch` visual 18 → 20 inside
   its (already pressable) cell so the cell reaches 24.
6. **`rune_sec/clips_panel.rs`** — rows onto kit
   `list_row` (23 → 28). **`rune_sec/audio_panel.rs`**,
   **`rune_sec/fx_clips.rs`** (clip rows),
   **`rune_sec/graph_panel.rs`** (channel rows),
   **`rune_sec/layers_panel.rs`** (layer rows) — same
   25 → 28 via the kit shell.
7. **`rune_sec/menus.rs`** (foundation wave) — item
   rows 25 → 28 (py 5 → 6.5 or an explicit h 28),
   matching charter §9; `item_icon` and the `btn`
   fallback path must grow together so item heights
   never alternate inside one menu.
8. **Sliders** — `rune_sec/layers_panel.rs:363` and
   `rune_sec/scene_panel.rs:795`: drag root height
   22 → 28 when the kit `slider_track` lands; knob
   visual may stay 16 (charter §8 fixes the visual),
   the *root* is the target.
9. **Text fields** (every panel that mounts a
   `text_input` — project_tools, inspector_figure,
   figures filter, library filter, palette query,
   home search, fx/text fields): wrap the field in a
   framed row — px 8, py 6, 1 px `card_border`,
   radius 6 — lifting the focus target from ≈16.25 to
   ≈28–30 without touching the kit editor (charter:
   panels style frames, not the kit core). One shared
   frame helper in the foundation wave, adopted per
   panel; keys stay on the field element itself.
10. **No work** — `icon_only_btn` (26 everywhere),
    palette rows (34), home rows (≈45), camera dial
    (26), toast controls, transport/app-bar/workspace
    rows (25, charter tier). Record them as compliant
    so adoption agents do not churn them.

## Readability

### Type scale in use (census over `src/rune_sec` + `src/*.rs`)

| Style | Spec | Uses | Where |
|---|---|---|---|
| `type_::LABEL` | 12 / 500 | **269** | the workhorse: buttons, rows, values, hints |
| `type_::TITLE` | 13 / 600 | 24 | project name, home rows |
| `type_::BODY` | 13 / 400 | 22 | palette rows, field text |
| `type_::SECTION` | 11.5 / 600 | **10** | almost no heading uses the heading style — the charter §2/§3 header work is what fixes this |
| `type_::MONO_CAPTION` | 12 / 500 | **0** | not used anywhere yet; charter §6/§11/§12 introduce it (stepper values, kbd chips, status numbers) |
| `RULER_LABEL_STYLE` | **10 / 500**, lh 12 | 1 | `timeline_marks.rs:559` — **the smallest text in the app** (ruler time labels) |

Findings:

- **One sub-11.5 text exists**: the 10 pt ruler labels.
  They sit inside the pinned `RULER_H 12` line
  (`.lh(RULER_H)`), so the font can rise to at most
  11 before the line box must grow — and the line box
  is pinned. Leave at 10, or take the re-pin decision
  with the §13 timeline card. Everything else is
  ≥11.5.
- **Line height is uniformly 1.25×** (12 → 15,
  13 → 16.25, measured) — no cramped or double-spaced
  outliers in the shell.
- **Contrast** (dark theme, from rune `tok` theme
  values; OKLCH-lightness → luminance approximations,
  ±10%): primary text on card ≈ **14.7 : 1**;
  `text_secondary` on card ≈ **6.8 : 1**, on window bg
  ≈ **7.5 : 1** — all comfortably over the 4.5 : 1
  AA line for 12 pt text. Light theme: secondary on
  card ≈ 6.0 : 1, on bg ≈ 5.3 : 1 — also passes.
  Inert/muted controls use `text_secondary`
  (`glyph_color_muted`), so disabled-looking controls
  stay legible.
- **The one tight contrast spot is white label text on
  the accent fill** (active buttons, selected rows,
  chips): ≈ **3.1–3.6 : 1** for the blues in use
  (`tok::accent(0)` / shell accent `0x6D8CFF`). That
  passes the 3 : 1 line for large text and UI
  components but not 4.5 : 1 for 12 pt text. This is a
  **token-level** choice inside rune's `tok` and the
  shell accent — no panel agent can or should fix it
  locally; flagged here for the foundation/integrator
  to accept or adjust once, centrally.
- **Density risk, not size**: the figure card packs
  eight controls into line 1 of a 300-wide card
  (thumb, select, %, lock, solo, −, +, eye) — with
  the steppers at 20.5 wide and 6-ish gaps, mis-presses
  are likelier there than anywhere else. Work-list
  item 1 (grow + stretch) is the fix; no other row in
  the shell packs controls that tightly.
- **Numbers are not tabular anywhere** (MONO_CAPTION
  unused): timers, percentages and stepper values in
  proportional LABEL shift width as they change — the
  charter §6/§12 mono adoption fixes the visible
  cases (stepper values, status numbers, timecode).

## Bottom line for the waves

The shell's shared buttons are uniformly at the floor
(25–26) and need no rescue; the failures are all in
**local, pre-kit helpers** — `row_toggle`, bare press
rows, unframed fields, hand-sized swatches — i.e.
exactly what the charter's kit replaces. Grow rows to
their charter tiers (28) through the kit shells, grow
swatches to 24 at their single size literals, stretch
`fig-sel`, fill the marker cells, frame the fields —
and leave the pinned timeline/status geometry alone
unless the integrator takes the one re-pin decision
for the §13 timeline card.
