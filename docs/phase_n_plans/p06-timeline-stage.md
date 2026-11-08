# P06 — Timeline & Stage UX Spec (Phase N, planning wave)

Scope: charter §12 (status-strip ticks), §13 (timeline
card), §14 (stage radius + camera overlay pill), and the
transport polish that ties them together (§4 button
family applied to `chrome_view::transport_row`).
Files studied: `src/rune_sec/timeline_marks.rs`,
`src/chrome_view.rs`, `src/rune_sec/playback_controls.rs`,
`src/stage_view.rs`, `src/rune_sec/camera_panel.rs`
(`overlay()`), `src/rune_sec/status_strip.rs`, plus the
mounting code in `src/main.rs` (read-only — no Phase N
agent edits `main.rs`; its items are integrator wiring,
listed in §5).

Everything here is restyle/restructure only. Handlers,
labels, status strings and `tr()` keys move verbatim;
every element key survives byte-identical; no new
`State` fields; no new keys are needed anywhere in this
spec (the one candidate new key is explicitly rejected
in §1.2).

---

## 1. The timeline card — `timeline_marks.rs`

### 1.1 What exists today

`timeline_marks(s, t)` (timeline_marks.rs:798) returns
one of two shapes:

- **No clips and no loop content** — the bare scroll row
  `row().key("timeline").child(content).scroll_x().w_full()`,
  where `content` is an unkeyed `col().gap(2.0)` holding
  ruler → chips → marker row.
- **Otherwise** — an unkeyed `col().gap(2.0)` stack:
  the same scroll row, then the clips lane
  (`timeline-clips`, when spans exist), then the loop
  row (`timeline-loop-row`, when frame count > 0).

All of it sits directly on the window background in the
centre column (main.rs:717), between the transport row
and the status strip. There is no fill, no border, no
padding anywhere in the block.

### 1.2 The change

Wrap the whole block in ONE card, per charter §13:
fill `t.card`, border 1.0 `t.card_border`, radius 12
(`design::PANEL_RADIUS`), padding 8. Concretely, in
`timeline_marks()`:

- Build the card as an unkeyed `col()` — the same root
  shape the stack already returns in the clips/loop
  case, so the centre-column slot's root type stops
  alternating between row and col. **No new key.** A
  `timeline-card` key was considered and rejected: the
  gate diffs key sets, the card needs no finder (its
  children carry every key tests use), and today's
  stack root is already unkeyed.
- Apply `.fill(t.card).border(1.0, t.card_border)
  .radius(12.0).padding(8.0)` to that root col. Keep
  its internal `.gap(2.0)` — see §1.4 for why the
  charter's panel gap (8) does NOT apply inside this
  card.
- The early return disappears: with neither clips nor
  loop row, the card contains only the scroll row.
  `timeline` (scroll row), `timeline-ruler`,
  `timeline-chips`, `timeline-markers`,
  `timeline-clips`, `timeline-loop-row` and every
  `frame-{n}` / `marker-cam-{frame}` /
  `marker-graph-{frame}` / `loop-*` key stay exactly
  where they are, one level deeper.

### 1.3 Element-by-element

| Element (key) | Today | Phase N |
|---|---|---|
| Card root (unkeyed col) | stack col, gap 2, no styling; absent in the bare case | always present; fill `t.card`, border 1 `t.card_border`, radius 12, padding 8, gap 2 |
| Scroll row (`timeline`) | `scroll_x().w_full()` | unchanged |
| Content col (unkeyed) | gap 2: ruler, chips, markers | unchanged — it lives INSIDE the scroll viewport; see the drag warning in §6.1 |
| Ruler (`timeline-ruler`) | row, gap `CHIP_GAP`, `on_drag(ruler_drag)`; slots `CHIP_W × RULER_H`; labels `RULER_LABEL_STYLE` (10 pt/500, lh = `RULER_H`); tick `el().size(1, 5).fill(t.text_secondary)` | unchanged, every value |
| Chips row (`timeline-chips`) | row, gap `CHIP_GAP`, `on_drag(timeline_drag)` | unchanged |
| Chip (`frame-{n}`) | `size(CHIP_W, CHIP_H)`, radius 6, label `type_::LABEL`; fills: current = accent, in-loop = `t.card.mix(accent, 0.35)`, idle = `t.card`; hover: current = accent, else `t.control_track`; selection = `.outline(2.0, 1.0, ink)` | **idle fill `t.card` → `t.control_off`** (see below); everything else unchanged |
| Marker row (`timeline-markers`) | row, gap `CHIP_GAP`; cells `CHIP_W × MARKER_H`; glyphs `◆` accent / `●` `text_secondary` in transparent pressables (`marker-cam-{f}`, `marker-graph-{f}`) | unchanged |
| Clips lane (`timeline-clips`) | canvas, `w(timeline_width(n))`, `h(CLIPS_H)`, one `Shape` | unchanged (see §6.3 for the alignment note) |
| Loop row (`timeline-loop-row`) | row, gap `CHIP_GAP` (4), `center_y`; `btn` children `loop-set` / `loop-a-dn` / `loop-a-up` / `loop-b-dn` / `loop-b-up` / `loop-clear`; label `loop_label()` in LABEL / `t.text` | gap 4 → 8 (S2) — the only internal spacing change; buttons and label unchanged |

**The idle-chip fill is the one forced colour change.**
Today an idle chip is filled `t.card` and reads as a
chip because the window background behind it is darker
than `t.card`. Inside a `t.card` card it would be
invisible — fill identical to the ground. `t.control_off`
is the theme's resting-control tone and is the closest
token that still separates from `t.card`. The in-loop
tint `t.card.mix(accent, 0.35)` does NOT change: it is
visibly bluer than the card ground, so loop frames
still read as tinted. The current frame stays accent /
white text. Verify in the headless snap (§7) that idle
chips read as discrete chips on the card; if
`control_off` proves too close to `card` in the landed
theme, the fallback is `t.control_track` for idle fill
and `t.hover` for hover — decide from the snap, not by
eye in code.

### 1.4 What must NOT change in the timeline

- **Geometry constants, all of them**: `CHIP_W` 30.0,
  `CHIP_H` 24.0, `CHIP_GAP` 4.0, `RULER_H` 12.0,
  `MARKER_H` 14.0, `CLIPS_H` 10.0, `CLIP_BAR_H` 8.0,
  `EDGE_PX` 8.0. They are load-bearing far beyond
  looks: `frame_at_x`, `ruler_frame_at_x`, `edge_at`,
  `span_rect` and `timeline_width` all compute pointer
  mapping and clip spans from the same pitch. Changing
  a constant "for looks" silently re-maps every drag.
- **Chip radius 6.0** — already `CONTROL_RADIUS`; keep.
- **The two drag handlers and their placement**:
  `timeline_drag` on the `timeline-chips` row and
  `ruler_drag` on the `timeline-ruler` row, both inside
  the scrolled content col. Their `Drag` points are
  element-local content coordinates; the handlers'
  semantics (selection anchor/focus, click =
  release-on-anchor → `goto`, loop-edge sentinels
  −1.0/−2.0 in `d.data`, cancelled-drag rules) are
  specified in the module docs and are frozen.
- **Marker pressables**: the transparent `el()` wrappers
  add no fill, padding or radius today (module docs say
  so explicitly) — do not "style" them; any padding
  would shift the glyph inside its `CHIP_W × MARKER_H`
  cell and break the chip-for-chip alignment.
- **The internal gap of 2.0** between ruler/chips/
  markers and in the under-stack. Charter §1's panel
  gap (8) is for control panels; §13 specifies only
  fill / radius / padding for the timeline card. The
  marker row must sit directly under its chips; an
  8 pt gap would detach the markers from the frames
  they annotate. This is a deliberate, documented
  deviation from the generic card recipe.
- `loop_fill` formula, `chip_label` / `duration_suffix`
  text, `loop_label` wording ("Loop {a+1}–{b+1}", the
  status strip's wording — the two must stay in sync).
- Lane colours `AUDIO_SPAN_RGB` 0x6D8CFF /
  `FX_SPAN_RGB` 0xA184FF and `CLIP_DIM_ALPHA` 0.35.
- `chrome_view::timeline_row` (chrome_view.rs:184) is
  the unmounted Phase C fallback — main.rs mounts
  `timeline_marks` only. Leave it untouched; restyling
  dead code buys nothing and risks its unit tests.

---

## 2. Transport polish — `chrome_view::transport_row`

### 2.1 What exists today

`transport_row` (chrome_view.rs:114): one `row()`
keyed `transport`, `w_full`, gap 8, `center_y`:

`prev` (icon_btn, chevron-left) · `play` (icon_btn,
play/pause glyph swap, `active = core.playing`) ·
`next` (icon_btn, chevron-right) · spacer `el().grow(1.0)`
· `add-frame` (icon_btn, plus) · `dup-frame` (**plain
`btn`**) · `del-frame` (**plain `btn`**).

The buttons already meet charter §4 exactly — `btn`
and `icon_btn` share px 10 / py 5, radius 6, accent
active, card resting fill (icon_btn.rs documents the
value-for-value mirroring). There is nothing to
re-metric. The polish is family consistency:

### 2.2 The change

- Convert `dup-frame` and `del-frame` from `btn` to
  `icon_btn`, with glyphs `copy` and `trash-can`
  (both already in the icon catalogue — `copy` is used
  by frame_ops' `copy-frame`, `trash-can` by its
  `range-del`). Keys, `tr()` labels (`duplicate_frame`,
  `delete_frame`), `active = false` and the handler
  closures (`s.core.duplicate_frame()`,
  `s.core.delete_frame()`) move verbatim. Result: the
  whole transport speaks one button dialect, and the
  two frame-destructive/creative ops get the same
  glyph language as their Frames-section counterparts.
- Everything else stays: the spacer (the row's only
  flexible member), gap 8 (S2), the play button's
  accent-while-playing state, the glyph swap on
  `play`, and the wrap-around `goto` closures on
  `prev` / `next`.
- Do NOT wrap the transport in a card. Charter §13's
  card is the timeline stack only; the transport stays
  a bare row between the stage overlay row and the
  timeline card.
- Do NOT touch `app_bar` or `status_text` from this
  spec — charter §15 (TITLE name, dirty dot, hairline
  under the bar) belongs to the chrome/app-bar plan.
  Note for that plan: the strip's project segment
  already carries a ` ●` dirty suffix in text form
  (status_strip `project_text`); the app-bar dot is a
  separate element and must not be "reconciled" with
  it from here.

### 2.3 Playback panel — coordination only

`playback_controls.rs` is a separate adoption file,
but it is the transport's other half; the deltas its
agent should apply, for the record:

- Panel root (playback_controls.rs:478): `padding(10)`
  / `radius(8)` / `fill(t.card)` → the kit card:
  `design::panel_col("playback-controls", t)` —
  padding 12, radius 12, border added. The root key
  `playback-controls` does not change.
- Its private `section()` helper (LABEL /
  `text_secondary`) retires onto
  `design::section_label` — charter §3 names exactly
  this helper shape. Section TEXTS stay byte-identical
  (several are dynamic: "Frame rate — now {fps}",
  "Looping frames 2–4", the On/Off state lines).
- The `timecode` readout keeps its key; its font goes
  from `type_::TITLE` to `type_::MONO_CAPTION` —
  it is a pure `m:ss.f / m:ss.f` readout, the
  charter §12 numeric treatment applied in its home
  panel. Text from `timecode_label()` unchanged.
- `pb-loop`, `pb-speed` and `tween` stay active-state
  buttons (icon_btn / btn). Do NOT convert them to the
  kit toggle pill: their keys sit on labelled button
  roots with state text beside them, and the charter's
  toggle is a switch-row control, not a replacement
  for state buttons. (Same rule as the transport's
  `play`.)
- `fps-dn` / `fps-up` are NOT rebuilt as a kit stepper:
  the value they step lives in the section heading
  text ("Frame rate — now {}"), not between the
  buttons, and moving it would change pinned section
  strings. Leave the pair as plain `btn`s.

---

## 3. Stage radius + camera overlay pill

### 3.1 Stage radius — integrator item in `main.rs`

The stage canvas is built in main.rs:681-708:
`canvas(stage_shape(...)).key("stage").grow(1.0)
.w_full().min_h(0.0).fill(stage_fill).radius(8.0)`
with `on_drag(stage_drag)`,
`on_secondary_press_local(stage_secondary)` and
`on_wheel(stage_wheel)` attached.

Charter §14: radius 12. The change is exactly
`.radius(8.0)` → `.radius(12.0)` on that element.
Nothing else on the element moves:

- `cx.layout_size("stage")` (main.rs:560) reads the
  element's layout box — radius is paint, not layout,
  so the size the shape is drawn for is unchanged, and
  `ViewTransform` / `RotTransform` maths is untouched.
- The three handlers stay attached to the same
  element; drag points are element-local and do not
  know about the radius.
- `stage_shape` (stage_view.rs:530) itself needs NO
  change: it draws content (guides, onion, strokes,
  figures, fig3, FX, text, weather) and no background
  rect — the background is the element's own
  `.fill(stage_fill)`, which the radius clips.

**Corner risk (must verify, §7):** the Phase L lesson
stands — Rune canvas drawing is NOT clipped to the
element rect (the fig3 grid bleed). A Shape prim that
reaches a corner (a guide line, a long stroke) may
paint square across the rounded corner cutout. 12 pt
is a small radius and the stage's corner-reaching
prims are thin lines, so the expected artefact is at
most a hairline nick at a corner; but the stage snap
must be re-read at all four corners after the change.
If a real paint-over shows, the honest fallback is
radius 8 on the canvas with the charter deviation
reported — do NOT inset or re-clip the shape (that
would change drawing geometry shell-wide).

### 3.2 The overlay pill — `camera_panel::overlay()`

Today (camera_panel.rs:986): `overlay(core)` returns
`row().key("camera-overlay").gap(4.0).center_y()` with
`view-fit` (Fit) · `view-centre` (Centre) ·
`view-actual` (1:1) · `cam-zoom-out` (−) ·
`cam-zoom-label` (zoom_readout) · `cam-zoom-in` (+),
all plain `btn`s. main.rs:710-716 right-aligns it in
the unkeyed `stage-overlay-row` under the stage via a
growing spacer row.

Charter §14: the overlay becomes a floating card pill
— fill `t.card`, radius 999, px 10 / py 4, border
`t.card_border`. Concretely, on the `camera-overlay`
root row only:

- Add `.fill(t.card).border(1.0, t.card_border)
  .radius(999.0).px(10.0).py(4.0)`. Keep `.gap(4.0)
  .center_y()` and the key.
- The six children keep their keys, labels (the
  literals "Fit" / "Centre" / "1:1" / "−" / "+" are
  NOT `tr()` keys today — leave the strings alone)
  and handlers (`frame_all_figures`,
  `centre_on_selection`, `zoom_actual`,
  `camera_zoom_by(1.0 / ZOOM_STEP)` / `(ZOOM_STEP)`).
- `cam-zoom-label` becomes a real stepper value
  (charter §6): `zoom_readout` (camera_panel.rs:750)
  currently is `el().px(6.0).center()` + LABEL text,
  so the readout's width changes with the label
  ("50%" vs "100%") and the + button shifts on every
  zoom step. Replace its inner construction with
  `design::stepper_value(&zoom_label(core), t)`
  (MONO_CAPTION, centred, min-width 34) wrapped so the
  key `cam-zoom-label` stays on the element the tests
  find. Note `zoom_readout` has a second caller — the
  camera panel's own zoom row (camera_panel.rs:940) —
  it gets the same steadier readout; that is desired,
  one readout one look, but the camera panel's agent
  must be told.
- Buttons inside a pill sit on `t.card` while the
  pill itself is `t.card`: resting `btn` fill is
  `t.card`, so the pill's buttons go flat against it
  (hover `control_track` / press `control_off` still
  read). This matches the timeline-chip situation in
  reverse and is acceptable for a 6-control pill —
  the pill's border and shadow-less card fill carry
  the grouping. Do NOT invent a new button fill for
  the pill.
- "Floating": the pill stays in `stage-overlay-row`
  under the stage. A true on-stage float (re-parenting
  the pill over the canvas) is NOT in this spec: it
  would change the centre column's layout, the stage's
  height, `layout_size("stage")`, and every stage
  geometry pin, for a placement nuance. The charter's
  parenthetical defines the pill by its tokens
  (fill / radius / padding / border), all of which
  land here. If the integrator later wants the literal
  float, it is a main.rs restructure with its own
  verification round — flagged, not smuggled in.

**Theme trap (pre-existing, must not worsen):**
`overlay()` takes only `&EditorCore` and builds its
theme as `tok::theme(true)` — always dark — while the
shell view builds with `tok::theme(!s.core.light_theme)`.
Its buttons already render dark-theme `t.card` in a
light shell. The pill fill must come from the SAME
theme value the function already uses, so pill and
buttons can never disagree with each other. Threading
the real `&Theme` through means a signature change
(`overlay(core, t)`) plus the main.rs:715 call site —
that is integrator wiring; recommend it as an
optional integrator item, and if it is not taken, the
pill ships on `theme(true)` exactly like its buttons
do today. Either way, do not let the pill take the
shell theme while its buttons keep `theme(true)`.

---

## 4. Status-strip ticks — `status_strip.rs`

### 4.1 What exists today

`build_strip` (status_strip.rs:890) composes the row
keyed `status-strip` (`row().w_full().gap(8.0)
.center_y()`): the fixed segments in their pinned
order — project, frame, timecode, fps, zoom,
selection, undo, play — then the conditional block
(speed, 3D, audio, loop-with-icon, stroke, particle,
autosave), then the single flexible tail. Between
segments it inserts `divider()` (status_strip.rs:819):
`el().size(1.0, 12.0).min_w(1.0).fill(t.control_track)`.

### 4.2 The change (charter §12)

- **Ticks**: `divider()` becomes 1 × 10 filled with
  `t.separator` (from `size(1.0, 12.0)` /
  `t.control_track`). Keep `.min_w(1.0)` — the floor
  that stops width pressure shrinking the tick away
  is part of the Phase F discipline. If the landed
  `design::divider(t)` is exactly a 1×10 `separator`
  hairline in row context, delegate to it and delete
  the local fn; if the kit divider is a horizontal
  rule for column context, keep the local fn with the
  new metrics. Integrator checks against the landed
  design.rs.
- **Numeric segments in MONO_CAPTION**: exactly two
  segments qualify as numeric — their whole text is
  digits and punctuation:
  - the **timecode** segment (`timecode()` →
    "0:00.0"), and
  - the **speed** segment (`speed_segment()` →
    "0.25×" / "1.5×" / "2×"), when present.
  Give `segment()` a mono sibling (or a font
  parameter) used for those two call sites only.
  Frame ("Frame 1/5"), fps ("12 fps"), zoom
  ("Zoom 100%") and undo ("Undo 0") are word+number
  labels, not numeric segments — they stay
  `type_::LABEL`. The project segment (name + runtime
  + dirty dot), selection, play, and all other
  conditional segments stay LABEL. The tail stays
  LABEL / `text_secondary` / truncated.
- Row gap stays 8.0; the row's height is set by the
  text line (the strip measures 15 pt tall in the
  Phase J tree), so a 10 pt tick centred by
  `center_y` changes no bounds.

### 4.3 What must NOT change in the strip — the hard list

This module carries the heaviest pinned arithmetic in
the shell. The following are frozen:

- **`LABEL_CHAR_W` = 8.0 and `est_text_width()`** —
  every fixed segment's floor derives from them. The
  mono change is safe ONLY because a 12 pt mono
  advance (≈ 0.6 em ≈ 7.2 pt) still sits under the
  8.0 floor — the floor remains a conservative
  overestimate for the two mono segments. Do not
  "correct" the floor for mono; that reopens every
  width proof in the module docs.
- **`MAX_STATUS_CHARS` = 96 and `fit_hint()`** — the
  tail's backstop cap; untouched.
- **The tail's construction**: `grow(1.0)` +
  `.truncate()`, the row's ONLY flexible member.
  No other segment may grow, shrink-wrap differently,
  or truncate.
- **Segment order and composition**: the original
  eight, then speed → 3D → extras (audio, loop,
  stroke, particle) → autosave, then tail.
  `extra_segments`' emptiness on a fresh core is
  pinned by test.
- **The Phase K audio-glyph refusal.** The audio
  segment stays text-only. The refusal is arithmetic,
  not taste: `volume-high`'s 640-wide view box paints
  13.75 pt at 11 pt height; glyph + gap ≈ 17.75 pt
  charged against the strip leaves the tail under its
  160 pt minimum at the 744 pt strip width
  (593.75 + 160 = 753.75 > 744, pinned by
  `audio_glyph_width_recomputation_at_744pt`).
  Charter §12 does not ask for segment glyphs; the
  loop segment's `repeat` icon (`segment_with_icon`,
  floor unchanged, glyph absorbed by the floor's
  conservatism) remains the strip's only glyph. Do
  not add a glyph to the audio, speed, 3D, stroke or
  particle segments — the Phase M note records that
  even the 3D segment, which would fit, stays
  text-only for consistency with this refusal.
- **The strip's hardcoded theme**: `build_strip`
  builds with `tok::theme(true)` and
  `segment_with_icon` paints its glyph with
  `GLYPH_TEXT` 0xF1F3F7, a constant chosen for that
  theme. Restyling must use the same `t` the builder
  already has; do not "fix" the theme source here.
- All label-producing fns (`project_text`,
  `frame_text`, `fps_text`, `zoom_text`,
  `selection_text`, `undo_text`, `play_text`,
  `audio_segment`, `loop_segment`, `stroke_segment`,
  `particle_segment`, `autosave_segment`,
  `speed_segment`, `fig3_segment`, `runtime_label`)
  — strings byte-identical. In particular the play
  segment keeps its inline construction (accent
  colour while playing, floored via `est_text_width`)
  and the dirty ` ●` stays a text suffix in
  `project_text`.

---

## 5. Integrator / main.rs items (no adoption agent touches these)

1. main.rs: stage canvas `.radius(8.0)` → `.radius(12.0)`
   (§3.1). One literal; corner verification in §7.
2. Optional: thread `&Theme` into
   `camera_panel::overlay(core, t)` and update the
   single call site (main.rs:715) — only if the theme
   consistency fix is taken (§3.2).
3. Nothing else in main.rs: the timeline card lives
   inside `timeline_marks()`, the transport inside
   `transport_row()`, the strip inside `build_strip()`
   — the centre column's child list, gaps (12.0) and
   order (stage, stage-overlay-row, transport,
   timeline, status strip) do not change.

---

## 6. Press / drag risk register

### 6.1 Timeline drags (highest risk in this spec)

`timeline_drag` and `ruler_drag` read `Drag.x` as a
content coordinate because their handlers sit on rows
INSIDE the `scroll_x` viewport's content col — the
element actually translated by scrolling. The card
wraps OUTSIDE the scroll row, so its 8 pt padding and
1 pt border never enter the coordinate chain: drag x
is unchanged, `frame_at_x` / `ruler_frame_at_x` /
`edge_at` keep mapping exactly as today. **The one
fatal restructuring would be inserting the card (or
any padded element) between the scroll row and the
content col, or moving a handler onto the card /
scroll row** — either shifts every drag by the
padding and mis-maps clicks, selections and loop-edge
grabs (EDGE_PX is only 8 pt; an 8 pt shift makes the
start-edge zone land a full chip-edge away). The
work list in §7 states the wrap point explicitly;
reviewers should diff for it.

Scroll behaviour side-effect, acceptable and intended:
the scroll viewport's width shrinks by 18 pt
(padding 16 + border 2), so a long timeline scrolls
fractionally sooner. The chips' pitch is untouched, so
scroll offset ↔ frame mapping is unchanged. The clips
lane (outside the viewport, reads from frame 0 — a
documented Phase M caveat) and the chips share the
card's content-left edge, so their mutual x-alignment
is preserved exactly.

### 6.2 Loop-edge and marker presses

Loop-edge grabs are decided by `edge_at` on the chips
row's drag (§6.1 covers the coordinates). Marker
pressables are siblings of the chips row, not children
— card wrapping does not route their presses through
the drag handler, same as today. The loop row's
buttons gain 4 pt more separation (gap 8); their keys
and handlers are untouched, and the row sits outside
the scroll viewport as before, so its buttons never
scroll away.

### 6.3 Stage presses and drags

The stage's `on_drag` (pose / draw / camera modes),
`on_secondary_press_local` (figure pick) and
`on_wheel` (zoom coalescing through `State::wheel_zoom`)
all consume element-local points from the canvas
element itself. Radius is not layout: no point moves.
The fig3 stage pass and every other `stage_shape`
pass draw in the same element space as before. The
only stage risk is the corner paint question in §3.1,
which is visual, not interactive.

### 6.4 Presence / layout shifts to re-pin

- The timeline block grows by ≈ 18 pt of height
  (padding + border). The stage is the centre
  column's `grow(1.0)` member and absorbs it: stage
  height shrinks by the same amount, and the
  transport / timeline / strip all move down ≈ 18 pt.
- The stage WIDTH pin (744 pt, snap_tests
  `stage_width_pins_the_phase_j_column_split`) is a
  width assertion — unaffected, must keep passing
  as-is. The status-strip overlap test reads bounds
  live (`presented("status-strip")`) — no absolute
  pin to update, but it must be re-run, since the
  strip's y moves.
- The status strip's own width is untouched (the
  card is a sibling above it, same column width), so
  the Phase K 744 pt floor arithmetic and its pinning
  test are unaffected by §4: the tick's width is
  still 1.0 floored, its fill/height are paint.
- Runtime tests that `find()` timeline keys get their
  centres from the drawn tree — keys and relative
  order are unchanged, so press paths keep working;
  any test that wheel-scrolls the right column to
  reach timeline-adjacent controls is unaffected
  (the timeline is in the centre column).

---

## 7. Work list for the coding wave

File-scoped, in order. (Ownership per the charter's
waves: items 1–2 and 4 are adoption-file work; item 3
splits between the camera panel's file and the
integrator's main.rs items in §5.)

1. **`src/rune_sec/timeline_marks.rs`** — card wrap
   per §1.2 (root col always returned; fill / border /
   radius 12 / padding 8; gap stays 2.0); idle chip
   fill → `t.control_off` (§1.3); loop row gap
   `CHIP_GAP` → 8.0. Nothing else in the file: no
   constant, handler, label or shape changes.
2. **`src/chrome_view.rs`** — `dup-frame` /
   `del-frame` → `icon_btn` with `copy` / `trash-can`
   (§2.2). `btn`, `app_bar`, `transport_row`'s other
   children, `timeline_row`, `status_text` untouched.
3. **`src/rune_sec/camera_panel.rs`** — pill styling
   on the `camera-overlay` root (fill / border /
   radius 999 / px 10 / py 4) and `zoom_readout` →
   `design::stepper_value` under the existing
   `cam-zoom-label` key (§3.2). Plus the §5.2
   integrator item if taken.
4. **`src/rune_sec/status_strip.rs`** — `divider()` →
   1×10 `t.separator` (or delegate to
   `design::divider` if it matches, §4.2); mono
   variant of `segment()` applied at the timecode and
   speed call sites only. Nothing in §4.3 moves.
5. **Coordination** — `playback_controls.rs` deltas
   in §2.3 go to that file's adoption agent; the
   `zoom_readout` change (item 3) is announced to the
   camera panel's agent, whose panel zoom row shares
   the helper.

## 8. Verification (V wave)

- CI green on the full suite (standing rule: CI is
  the only build/test environment).
- Gate: element-key set diff vs the Phase M baseline
  (733) — this spec adds and removes NO keys.
- Headless: re-dispatch and read (a) the default snap
  — idle chips visible on the timeline card, card
  hairline clean; (b) the stage snap — all four stage
  corners checked for Shape paint-over at radius 12
  (§3.1); (c) a clips-present scene — lane present
  inside the card, aligned with the chips at scroll 0.
- Runtime round-11 candidates: timeline drag still
  seeks/selects after the card wrap (press on a chip →
  `goto`; drag → `range_sel`; loop-edge drag at
  `EDGE_PX`), ruler drag still scrubs, marker press
  still seeks, transport `dup-frame` / `del-frame`
  presses still duplicate/delete, overlay zoom
  steppers still step and `cam-zoom-label`'s centre
  no longer moves between 50% and 100%.
