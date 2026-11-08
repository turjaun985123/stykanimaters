# P04 — Layout & Panel Shell Spec (Phase N, planning)

Scope: the shell geometry in `src/main.rs` `view()` and the
panel-card recipe from the charter applied to the four panel
archetypes. Studied in source: `src/main.rs`,
`src/rune_sec/figures_panel.rs`, `scene_panel.rs`,
`export_panel.rs`, `audio_panel.rs`, `three_d_panel.rs`,
plus `fig3_preview.rs` / `waveform_view.rs` for the
fixed-width canvases. No code changes in this document —
it is the contract the F/A waves execute against.

## 1. The shell as it stands (measured from source)

Root (`main.rs` `view()`, line 554):

- Root `col()`: `w_full / h_full`, fill `t.bg`,
  **padding 16**, **gap 12**. Children, in order:
  `app_bar` (chrome_view), `menu_bar`, `workspace_row`,
  `main_row`, then the overlay children (toasts, menu
  scrim, menu dropdown) last.
- `main_row`: `row()`, `w_full`, `grow(1.0)`,
  `min_h(0.0)`, **gap 12**.
- Left column: `col().key("side-left").gap(12.0)` inside
  `.scroll(...)` at **w 300**, `min_h(0.0)`. Mount order:
  Home, Project, Figures, FigureAdd (ungated), Library,
  Layers — each gated per `workspaces::section_visible`
  except FigureAdd.
- Right column: `col().key("side-right").gap(12.0)`
  inside `.scroll(...)` at **w 340**, `min_h(0.0)`.
  Mount order: Help (when open), Settings (ungated),
  then ~24 gated sections in workflow order (inspectors
  → pose/keyframe → camera → scene → onion → playback →
  frames → graph → easing → draw → stroke → text → FX
  browser/clips → particles → audio → **waveform** →
  clips → 3D → export → stats).
- Centre column: `col().key("centre").gap(12.0)
  .grow(1.0).min_w(0.0)`. Stack: command palette (when
  open), stage canvas (key `"stage"`, `grow(1.0)`,
  `w_full`, `min_h(0.0)`, fill = project background,
  **radius 8.0**), `stage-overlay-row` (spacer + camera
  overlay, right-aligned), `transport_row`,
  `timeline_marks`, `status_strip_state`.
- Window: default 1440×900, `min_size` 1280×720
  (`main()`).

Derived widths (the numbers every card decision hangs
on):

| Quantity | At 1440 | At 1280 (min) |
|---|---|---|
| Root content width (after pad 16 ×2) | 1408 | 1248 |
| Centre column width (1408 − 300 − 340 − 2×12 gaps) | **744** | **584** |
| Left card inner width (300 − 2×1 border − 2×12 pad) | **274** | 274 |
| Right card inner width (340 − 2×1 border − 2×12 pad) | **314** | 314 |

744 is the centre width the Phase F/K status-strip floor
arithmetic was proven against; 584 at minimum window is
already tight (see R2).

## 2. The panel-card recipe (charter §1–2, made exact)

Every panel root becomes `design::panel_col(key, t)`:

```
col().key(<existing panel key>)     // key byte-identical
     .w_full()
     .fill(t.card)
     .border(1.0, t.card_border)
     .radius(PANEL_RADIUS)          // 12
     .padding(PANEL_PAD)            // 12
     .gap(PANEL_GAP)                // 8
```

Rules:

- **The root key never changes** and the root stays a
  single `col` child of its column — no wrapper element
  around the card (a wrapper would add a tree level and
  break `layout_size` readers and presence matching for
  no visual gain).
- No panel sets outer margins. All inter-card spacing
  is the column gap (§5). All intra-card spacing is
  `PANEL_GAP` (8) — including header→divider and
  divider→body, which fall out of the col gap; do not
  add compensating spacers.
- Panels that are already proto-cards (scene, 3D — pad
  12 / fill `t.card` / radius 8 today) change only:
  radius 8 → 12, add the border, adopt the header
  (§3). Their internal layout does not move.

## 3. Header anatomy (charter §2, made exact)

One header block at the top of every card, two children
of the panel col:

1. `design::header_row(key)` containing:
   - `design::panel_title(icon, title, meta, t)` — the
     growing left cluster: optional **13px icon in
     `t.text_secondary`**, title in **`type_::SECTION` /
     `t.text`**, meta/count in **`type_::LABEL` /
     `t.text_secondary`**;
   - a spacer (`el().grow(1.0)`, the figures_panel
     idiom) where trailing controls exist;
   - the panel's existing trailing controls, keys and
     handlers byte-identical, in their current order.
2. `design::divider(t)` — 1px `t.separator`, `w_full`.

Header-row key: reuse the panel's existing header key
where one exists (`figures-panel-header`). Where the
header is a bare text today (scene, export, audio,
3D), the header row is **new structure**; give it the
key `<panel-root-key>-header` (e.g. `scene-panel-header`,
`export-panel-header`, `audio-panel-header`,
`panel-3d-header`) and **report it as an added key** in
the adoption report (charter: new keys only where the
brief allows — this spec is that allowance, for header
rows only).

Title font migration (all five studied panels differ
today; all land on SECTION per the charter):

| Panel | Title today | Meta today | Icon today |
|---|---|---|---|
| Figures | `text("Figures")` LABEL / `t.text` in keyed row `figures-panel-header` | `count_line(...)` LABEL / secondary, inline after title | none |
| Scene | bare `text("Scene")` TITLE, no row | none (first body row carries the weather summary) | none |
| Export | bare `text("Export")` TITLE + subtitle LABEL line "Export centre — every format in one place" | subtitle line doubles as meta | none (sections have icons, header doesn't) |
| Audio | `text("Audio")` TITLE in **unkeyed** row | `"Audio — {n} clip(s)"` LABEL / secondary after spacer | none |
| 3D | `text("3D")` TITLE, `grow(1.0)`, in unkeyed row | none | `icon_el("cube", 14.0, t.text)` |

Per-panel header rulings:

- **Figures**: header row already matches the anatomy;
  keep `figures-panel-header`, `fig-collapse`,
  `add-figure`, `fig-recolor`, `fig-duplicate` in
  order. Title LABEL → SECTION. Meta stays the
  `count_line` output (it already reads as a count).
- **Scene**: build the header row; title "Scene";
  no meta, no icon, no trailing controls today — the
  weather summary row stays as the first body row,
  untouched.
- **Export**: title "Export"; the subtitle line
  becomes the meta slot's neighbour — keep the
  subtitle as the first body line (it is descriptive
  copy, not a count) OR fold it into meta; this spec
  says **keep it as a body line** so no rendered
  string changes. No icon, no trailing controls.
- **Audio**: title "Audio". The meta string today is
  `"Audio — {n} clip(s)"`, which duplicates the title
  once the title exists beside it. **Flagged text
  change for integrator approval**: meta becomes
  `"{n} clip(s)"`. If not approved, keep the full
  string in the meta slot verbatim — do not silently
  edit it either way. It is a panel-built label, not a
  `core.status` string and not a `tr()` key.
- **3D**: icon stays `"cube"` but 14px/`t.text` →
  13px/`t.text_secondary`; title TITLE → SECTION and
  keeps `grow(1.0)` (the title cluster grows — same
  visual result as panel_title's growing cluster);
  trailing `mode-3d` toggle-button keeps its key,
  handler and position.
- Icons are **only** where a panel already has one
  (3D's cube). Do not invent header icons for the
  other panels in the adoption wave; the charter
  marks the icon optional.

## 4. The recipe per archetype

### 4.1 List panel — figures_panel, audio_panel
(analogy targets: library_panel, layers_panel,
clips_panel)

Before (measured):

- figures root: `col()` gap **6**, no fill / border /
  padding / radius. Rows: `figure_row` is a two-line
  `col`, px 6 / py 4, radius 6, selected fill
  `accent.alpha(0.22)`, hover `t.control_track`; row
  content includes the 44×44 rig thumbnail
  (`THUMB_SIZE = 44.0`, key `fig-thumb-{fi}`).
- audio root: `col()` gap **6**, no fill / border /
  padding. Clip rows: `row()` px 8 / py 5, radius 6,
  gap 6, selected fill `t.control_track`, unselected
  `t.card`. Empty state is a bare BODY text row:
  "No audio clips in this project."

After (the list-panel stack):

```
panel_col
├─ header_row (§3)
├─ divider
├─ [filter/search row — figures only, unchanged keys]
├─ list rows (kit list_row shell)
└─ [footer action row — figures only, unchanged keys]
```

- Root gap 6 → `PANEL_GAP` 8.
- Rows adopt charter §5 metrics: min-height 28,
  px 8, radius `ROW_RADIUS` 8, hover `t.hover`;
  selected = `design::selected_fill()` (accent 16%)
  replacing figures' 0.22 accent and audio's
  `control_track`. Existing row keys
  (`fig-row-{fi}`, `audio-clip-{i}`) and every inner
  control key survive; the row remains a shell — its
  press behaviour (audio row = selection carrier,
  figures select cluster) is unchanged.
- Figures rows keep their two-line structure and the
  44px thumbnail; only the shell metrics above move.
  Row height is thumbnail-driven (≈52), well over the
  28 minimum — the minimum binds only audio rows.
- Empty states become `design::empty_state` (charter
  §10): audio's bare text → `empty_state` with an
  icon from the panel's own glyph family (audio uses
  `volume-high` / `volume-xmark`, both in the curated
  table); figures' filter-no-match line ("No figures
  match …") → `empty_state("magnifying-glass", …)` —
  the glyph the filter row already uses. The empty
  *project* case for figures (zero figures) currently
  renders no line at all; leave that as is (no new
  element where none existed).
- Audio's `"Clips"` group label and summary line stay
  as body lines; the label restyles via the kit
  `section_label` (LABEL → SECTION per §6 below).

### 4.2 Form panel — scene_panel, export_panel
(analogy targets: inspector_*, camera_panel,
settings_popover, project_tools)

Before (measured):

- scene root: `col()` gap 8, **padding 12, fill
  `t.card`, radius 8, no border** — the proto-card.
  Header is the bare TITLE text. Body: weather row,
  `wx-kind` segmented row (gap 4), file-local
  `stepper_row`s, swatch grids (26×26 slots / 22×22
  cores, gap 4, radius 8/6), preset rows (gap 4).
- export root: `col()` gap **10**, no fill / border /
  padding. Header bare TITLE + subtitle. Body built
  from private `section()` / `section_with_icon()`
  helpers: a `col` gap 6 headed by a SECTION /
  secondary label (icon variant: 12px glyph + SECTION
  label in a gap-6 row) — the closest existing
  section idiom in the shell.

After:

```
panel_col
├─ header_row (§3)
├─ divider
├─ section_label (kit) + group rows, gap 8
├─ section_label (kit) + group rows, gap 8
└─ …
```

- Scene: radius 8 → 12, add border; everything below
  the new header is untouched — its rows already sit
  at gap 4/8 inside the panel gap-8 col. The local
  `stepper_row` retires onto the charter §6 stepper
  metrics (label grows; − btn; `stepper_value` mono
  readout, centred, min-width 34; + btn) — that is
  the scene adoption agent's file work; this spec
  fixes the target metrics only.
- Export: root gap 10 → 8. The private `section()` /
  `section_with_icon()` helpers are replaced by
  plain group cols headed by `design::section_label`
  (with the icon variant kept as a local row of
  12px glyph + kit label where the panel already
  uses one — frame_ops/fx_browser share that idiom).
  No divider between sections inside a form panel:
  the section label + gap carries the grouping
  (charter §3). All `xc-*` keys and rows unchanged.
- Swatch grids, preset rows and segmented rows keep
  their geometry; their radii (8/6) already match
  ROW/CONTROL radius tokens.

### 4.3 Canvas panel — three_d_panel (+ the waveform mount)
(analogy target: graph_panel)

Before (measured):

- 3D root: `col()` gap 8, **padding 12, fill
  `t.card`, radius 8, no border** — proto-card, with
  the best existing header row of any panel (§3).
  Body leads with the preview canvas:
  `panel-3d-preview`, **308×220 fixed**
  (`fig3_preview::PREVIEW_W/H = 308.0 / 220.0`), then
  control groups headed by a private `section_label()`
  that is LABEL-font (charter wants SECTION).
- Waveform is **not part of audio_panel**: main.rs
  mounts `waveform_view::waveform_el_state(s,
  [308.0, 56.0])` as its own right-column child
  (key `"waveform"`), gated as `Sec::Waveform`.

After:

```
panel_col
├─ header_row (§3)
├─ divider
├─ canvas block (exact pixel size, unchanged key)
└─ control groups (kit section_label)
```

- The canvas keeps its **exact** size and key. Do not
  wrap it in a second padded frame and do not round
  it expecting the drawing to follow: Rune canvas
  drawing is **not clipped to the element rect**
  (the Phase L lesson — the preview shape clips its
  own segments via `clip_segment_to_viewport`). The
  card is the frame; the canvas sits on the card fill
  directly.
- 3D: radius 8 → 12, add border, header per §3,
  private `section_label()` → kit `section_label`.
  Nothing else in the panel moves; the preview's
  308 width inside the card is the tight fit analysed
  in R1.
- Waveform: its card conversion is **shell work, not
  audio-panel work** (main.rs is integrator-owned;
  see R8). Recipe when the integrator does it: the
  waveform element gets the panel shell treatment as
  its own mini-card — `fill t.card`, border 1
  `card_border`, radius 12, **padding PANEL_PAD** —
  with the canvas still exactly 308 wide inside
  (314 inner, 6 slack, left-aligned as today). No
  caption or header is added; it reads as the audio
  card's visual sibling, which matches its gating
  (Audio workspace shows both).

### 4.4 Tool panel — the dense right-column clusters
(frame_ops, draw_tools, keyframe_tools, pose_tools,
onion_controls, playback_controls, stroke_inspector,
text_tool, fx_browser, fx_clips, particles_panel —
not individually studied; recipe fixed here so their
plans can cite it)

```
panel_col
├─ header_row (§3)
├─ divider
└─ button/control rows (one button family, charter §4)
```

- All local button helpers (frame_ops' `fbtn` is the
  named case) retire onto `chrome_view::btn` /
  `icon_btn` / `icon_only_btn` + inert twins at
  px 10 / py 5, radius 6 (charter §4).
- Rows must **wrap or scroll**, never overflow:
  frame_ops' main row already wraps (`flex_wrap`,
  the Phase M fix) — inside a card its wrap width
  drops from 340 to 314, so rows wrap roughly one
  button earlier; that is acceptable and needs no
  other change. graph_panel's `scroll_x` rows are
  the other accepted pattern. A tool panel must
  never set a fixed row width.
- Steppers/toggles/sliders inside tool panels follow
  charter §6–8 via the kit, keys on the pressable
  roots unchanged.

## 5. Column-level spacing rules

1. **Column gaps stay 12** (root col gap, both side
   cols' gap, main_row gap — all 12 = tok S3 today).
   Phase N does not retune them; card-to-card rhythm
   is exactly this gap, window-wide.
2. **Window inset stays root padding 16.** First and
   last cards sit flush against the column ends; the
   16 is the only outer margin in the shell.
3. **Inside a card, one gap value: 8** (`PANEL_GAP`).
   Header, divider and body are ordinary children of
   the panel col, so the rhythm header → divider →
   body is 8/8 with no special cases.
4. **Centre stack gap stays 12.** The timeline card
   (charter §13: fill `t.card`, radius 12, padding 8
   around the ruler/chips/markers/clips-lane/loop-row
   stack) and the stage are centre children; their
   internal padding is theirs (8 for timeline), the
   stack rhythm is unchanged.
5. **Stage radius 8 → 12** (charter §14) is a main.rs
   one-liner for the foundation/integrator list; the
   camera overlay becomes a floating pill (fill
   `t.card`, radius 999, px 10 / py 4, border
   `card_border`). Fallback if Rune overlay anchoring
   over the canvas proves awkward in the current tree
   shape: keep the existing `stage-overlay-row` mount
   and style the overlay element itself as the pill,
   right-aligned as today — same look, zero layout
   risk. The stage-chrome plan decides; both are
   specified here so neither blocks.
6. **App bar / workspace row**: charter §15 (TITLE
   name, accent dirty dot, hairline under bar +
   workspace row) is chrome work, but its layout
   cost lands here: two 1px dividers + their gaps
   come out of the vertical budget, i.e. out of the
   stage (R3).
7. Scroll viewports are unchanged: fixed widths
   300/340, `min_h(0.0)`, content clipped by the
   viewport. Cards are `w_full` children of the
   column col — a card never sets its own width.

## 6. Collapsed panels as cards

figures_panel is the shell's collapse precedent and
the only studied panel with collapse state
(`FiguresPanelState.collapsed`; Phase N adds no
state, so no other panel gains collapse in this
phase).

- Collapsed form = **card shell + header block,
  nothing else**: `panel_col` with the header row
  and the divider as its only children — exactly
  today's control flow (`if collapsed { return
  panel; }` after the header), with the shell and
  divider added. The divider **does** render when
  collapsed: it belongs to the header block, so a
  collapsed card reads as a titled card, not a
  stray bar.
- The toggle stays in the header's trailing cluster
  (`fig-collapse`, Show/Hide label swap and handler
  verbatim). No separate chevron element, no key
  changes.
- Collapsed card height ≈ 12 (pad) + ~26 (header
  row) + 8 (gap) + 1 (divider) + 12 (pad) ≈ **59** —
  it occupies one normal 12-gap slot in the column
  rhythm.
- Any future collapse (post–Phase N) follows this
  shape: shell + header + divider, early return,
  toggle in the header's trailing cluster.

## 7. Section labels inside panels (charter §3)

Every in-panel group label becomes
`design::section_label` — `type_::SECTION`,
`t.text_secondary`. Measured deltas: three_d_panel's
private `section_label()` is LABEL-font (→ SECTION);
audio's group labels are LABEL-font text rows
(→ SECTION via the kit); export's private helpers
already use SECTION and only change provenance;
figures has no group labels (its footer row is
actions, not a label). No label *text* changes.

## 8. Layout risks — read before adopting

- **R1 — The two 308-wide canvases (the tight fit).**
  Right-card inner width is **314**. The waveform
  canvas is exactly **308** wide
  (`waveform_el_state(s, [308.0, 56.0])`, mounted by
  main.rs) and the 3D preview exactly **308**
  (`PREVIEW_W`). Slack inside a standard card:
  **6pt**. Both fit — the 3D preview already lives
  at padding 12 today (316 inner, 8 slack; the
  border costs 2) — under three conditions:
  (a) the canvas is a **direct child** of the card —
  no card-in-card, no extra padded wrapper, ever;
  (b) nobody "helpfully" widens the canvas to
  `w_full`: the waveform shape is *built* for
  308 (`strip_columns(308.0) = 102` buckets) and
  its drag maths assumes the drawn size equals the
  passed `el_size` (waveform_view.rs module docs) —
  resizing the element without the shape desyncs
  seek; same class for the preview's clip viewport;
  (c) the scroll viewport hands the column the full
  340. **Verify (c) in the V wave**: read the
  headless `--tree` rects for `panel-3d` /
  `waveform` after adoption — if a scrollbar
  reservation ever eats into the 340, these two are
  the first elements to clip, and the fallback is a
  padding-0 shell (fill + border + radius only) for
  the waveform mount and the 3D panel's preview
  row, not a narrower canvas.
- **R2 — Centre width at minimum window.** At
  1280 wide the centre is 584; the status strip's
  floored segments were proven at 744 (Phase F) and
  re-argued at 744 in Phase K. Phase N must add
  **no horizontal padding to the centre stack**
  beyond the timeline card's 8 (which wraps a
  `scroll_x` row and is safe). If a centre element
  seems to want a card with padding 12, it doesn't
  — flag it instead.
- **R3 — Vertical budget / pinned layout facts.**
  Cards add ≈26 vertical per panel (pad 24 +
  border 2) + 1 divider + taller headers where
  TITLE→SECTION rows replace bare text; the app-bar
  and timeline changes add ~20 more. All of it is
  absorbed by scrolling columns and the `grow`ing
  stage, but the **pinned layout numbers move**:
  the Phase L stage bounds [328, 144, 744, 522] and
  the snap_tests geometric regressions will need
  deliberate re-pinning in the V wave — measure the
  new bounds from a headless snap, don't nudge
  assertions until they pass.
- **R4 — Scroll depth in Runtime tests.** Taller
  cards push right-column controls further down;
  `scroll_into_view`'s 24-wheel loop may run out on
  the deepest panels (frames, export). V wave:
  raise the loop bound if a test proves it, per
  test, with the reason recorded — the Phase G/K
  discipline.
- **R5 — Root-shape changes re-run presence.**
  Swapping a panel root from bare col to
  `panel_col` (same key, new fill/border/padding)
  and inserting header rows changes subtree shape;
  workspace switches will re-animate those panels
  once. Settle loops in Runtime tests already cover
  this class (Phase M's timeline-stack change was
  the same shape of event).
- **R6 — Layout-read keys are load-bearing.**
  `main.rs` reads `cx.layout_size("stage")` every
  frame; graph_panel reads layout via `cx` too;
  the 3D preview's drag uses element-local points
  at 308×220. All canvas keys (`stage`, `waveform`,
  `panel-3d-preview`, `fig-thumb-{fi}`) stay
  byte-identical and canvas elements keep their
  size-setting calls — restyle around them, never
  through them.
- **R7 — Left column is narrower than it looks.**
  Left-card inner is **274**. The studied left
  panel (figures) is fully fluid (grow filter row,
  `w_full` rows, 44px thumbs) and safe; any left
  panel with a fixed-width child > 274 breaks the
  day it becomes a card. The library/layers plans
  must census fixed widths against 274, not 300.
- **R8 — Ownership seams.** The waveform card and
  the timeline card wrap live in `main.rs` /
  `timeline_marks.rs` return shapes — foundation /
  integrator territory. The audio adoption agent
  must not try to absorb the waveform into
  audio_panel, and no adoption agent edits main.rs
  to reach a sibling mount.

## 9. Work list this spec feeds

- **F wave (design.rs consumers in chrome)**: app-bar
  divider + dirty dot (charter §15) — layout cost
  booked in R3.
- **Integrator / foundation (main.rs, one-line
  class)**: stage radius 8 → 12; camera overlay pill
  (§5.5, with the stated fallback); waveform
  mini-card (§4.3); timeline card wrap (charter §13,
  padding 8) at the `timeline_marks` mount.
- **A wave, per studied panel**:
  - figures_panel.rs — §4.1 + §6 (root shell, header
    font, row metrics, empty-state for no-match).
  - audio_panel.rs — §4.1 (root shell, header key +
    §3 meta ruling, row metrics, empty state).
    Waveform explicitly out of scope (R8).
  - scene_panel.rs — §4.2 (radius/border, header
    row, divider; stepper metrics target).
  - export_panel.rs — §4.2 (root shell + gap,
    header row, private section helpers → kit).
  - three_d_panel.rs — §4.3 (radius/border, header
    metrics, kit section labels; canvas untouched).
- **V wave**: headless-tree width verification for
  R1(c); layout re-pinning per R3; scroll-loop
  audit per R4.

## 10. Non-goals

No engine or handler changes; no new State fields;
no new keys beyond the header-row keys named in §3;
no text changes beyond the single flagged audio
meta (§3, integrator-approved or not at all); no
column width changes (300/340 are pinned by the
Phase J layout fix and this phase does not reopen
them).
