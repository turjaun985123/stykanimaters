# The dopesheet — original design, the port's gap, and the Wave 10 lane

Status (2026-11-09, Wave 10 integrated): **the lane in §3
is LANDED** — `src/rune_sec/dopesheet.rs` exists, is wired
into the timeline card (Animate workspace), and is covered
by a scene, snap tests and a headless snap. Two facts the
brief did not state, recorded here so the document stays
truthful: the lane's frame columns are offset a constant
124 pt from the chips' columns by the 120 pt figure-name
gutter (pitch and shared scroll are exact; absolute
registration is shifted — judged acceptable at the
integration pixel pass, since each row's cells still sit
in the row's own frame order under the shared scroll);
and the lane's header label routes through the i18n
`dopesheet` key (all 28 languages).

## 1. What the original StykAnimate has

The original app's timeline *is* a dopesheet — it calls the
feature the **layer sheet** in code (the word "dopesheet"
appears in the original repo only in
`docs/research/animation-tools-research.md`, citing Spine's
dopesheet as design precedent). The implementation lives in
the clone at `~/workspace/StykAnimate-App`:

- **View:** `src/layer-timeline.mjs` — class `Timeline`,
  self-described as an "Adobe-style exposure grid". A canvas
  paints the grid; a DOM label column (220 px by default,
  drag-resizable 140–420) names the rows.
- **Data:** `src/layer-sheet.mjs` — the sheet is
  `report._styk.layerSheet.tracks`, one track per row id;
  each track holds `keys`, every key carrying a `tick`, an
  interpolation/easing, and an `empty` flag for blank spans
  (`sheetKeyAt(track, tick)` finds the key governing a tick).
- **Operations:** `src/editor-layer-sheet.mjs` — the
  `sheet*` editor methods behind the grid (below).

### Rows

One row per **track**, not per joint:

- figure layers and drawing layers — in the original's
  model, figures are *assigned to layers*
  (`sheetAssignFigures`, `layerForUid`), so a layer row is
  where a figure's keys live; several figures can share one
  layer row;
- pinned **Camera** and **Scene** rows;
- camera-motion lanes and VFX lanes (VFX keys are
  per-property channels, surfaced through the row's channel
  popover);
- folders, which group layer rows.

Per-segment animation exists in the original, but as a
separate sparse channel system (`src/animation-channels.mjs`
— targets of kind figure / segment / layer / camera /
scene) whose keys feed the sheet; the sheet grid itself has
no per-joint rows.

### What a row shows

From `_paintRow` in `src/layer-timeline.mjs`:

- an **exposure bar**: one rounded bar from each key to the
  next key, so a held pose reads as a single piece — neutral
  fill for a hold, violet for a tweened span, a dashed
  outline for a blank span; once a frame is ≥ 10 px wide,
  thin seams inside the bar count its frames;
- a **key dot** at each bar's start (ink; gold on the Camera
  row, violet on Scene; hollow for blanks and cel starts);
- drawing layers instead paint **cels**: a block per
  exposure with the drawing's thumbnail inside when wide
  enough;
- row geometry (from `src/timeline-rows.mjs`): 32 px rows
  and a 32 px ruler (28/24 at the short tier, 48/44 on
  touch); horizontal zoom is `pxPerTick`, 14 px per frame
  by default.

### Editing

The original's sheet is a full editor, not a display
(locale strings in `src/locales/en-features/layerTimeline.mjs`
name most of it):

- click/drag selects a tick range across one or more rows;
  dragging a selection **moves or copies frames** within and
  across tracks (`sheetMoveFrames` / `sheetTransferFrames`,
  with a replace-on-drop setting);
- an edge grip on the selected exposure drags its end —
  **extend/shrink exposure** (`sheetSetDuration`, which
  inserts or removes frames to match);
- a per-key **easing popover** (`keyframe-easing-popover`);
- context menus: copy / cut / paste frames, delete selected
  keys, remove frames, extend exposure, blank or copy
  drawing here;
- layer management in the label column: rename (double
  click), colour dot, visibility eye, lock, reorder,
  folders, filter and find (Ctrl+F);
- keyboard control via `src/timeline-hotkeys.mjs`, plus
  playhead scrubbing and a "N more rows" scroll affordance.

## 2. What the Rust port had, and what it lost

The port's own GPUI shell had a dopesheet, smaller than the
original's but real:

- `src/ui_dope.rs` (deleted with the GPUI shell) exposed
  `keyframe_map(&Project) -> Vec<Vec<bool>>` — a per-figure
  × per-frame boolean map — and `dopesheet(...)`, rendering
  **one row per figure with keyframe diamonds** as small
  rotated elements (contracts in
  `docs/UI_COMPONENT_CONTRACTS.md`; described in
  `docs/RUNE_API_AUDIT.md`, "Timeline"; audited in
  `docs/GUI_AUDIT.md` §6–7, which also noted the map was
  recomputed twice per render). It sat in the bottom bar
  beside the frame chips, ruler and scrubber, and later
  gained the Wave 15 `tl-collapse` toggle
  (`docs/QA_WAVE15.md`).
- The Rune migration audit specced carrying it across
  ("one dopesheet row per figure with keyframe diamonds",
  `docs/RUNE_API_AUDIT.md`), but the Rune shell never
  rebuilt it. **The current state, honestly:**

  - `src/rune_sec/timeline_marks.rs` gives the timeline
    card a single merged **marker row** (key
    `timeline-markers`, `MARKER_H` = 14 pt) under the frame
    chips: a `◆` where a frame carries a camera key, a `●`
    where any graph channel has a key (fractional graph
    times rounded by `graph_key_frame`), each glyph
    pressable to seek (`core.goto`). It shows camera and
    graph keys only — **no per-figure pose keyframes, no
    rows, no spans**.
  - The engine helper survived the migration:
    `stats::figure_keyframes(&Project, fi) -> Vec<usize>`
    in `src/stats.rs` returns the frames where figure
    `fi`'s pose changes (frame 0 counts when the figure is
    posed there; change is `pose_changed` — root moved past
    `ROOT_EPS` or any joint angle past `ANGLE_EPS`; a frame
    where the figure is missing is never a key, and its
    reappearance is). Its doc comment still cites the
    deleted `ui_dope.rs` rule. In the current tree it has
    **no callers** outside `stats.rs`'s own tests — in the
    GPUI shell it fed a Home-panel sparkline
    (`docs/GUI_AUDIT.md` §7.1); the Rune stats view does
    not use it.
  - The name survived too: `src/i18n.rs` carries a
    `dopesheet` string ("Dopesheet") in all 28 language
    tables, currently unused by any panel.

So the gap is precise: the data (per-figure keyframes) and
the name exist; the per-figure visual surface does not. A
Wave 9 audit surfaced the loss, and the user approved
rebuilding the panel on 2026-11-08.

## 3. The Wave 10 lane — LANDED

Deliberately a lane, not the original's full sheet: the
brief restores the per-figure overview and navigation the
port lost, inside the timeline the Rune shell already has.
It does **not** attempt the original's exposure bars, drag
editing, easing popover or layer system — the port's model
has no layers; figures are the top-level objects
(`Project.figures`, poses positional per frame), so one row
per figure is the faithful mapping to this model.

As briefed for Wave 10:

- **Placement:** a lane inside the Animate workspace's
  timeline card, in the same stack as the frame chips and
  the marker row in `src/rune_sec/timeline_marks.rs`.
- **Rows:** one per figure, **capped at 4 rows** — a taller
  stack would take stage height the timeline card does not
  have; figures past the cap are not shown (the cap is a
  display limit, not a data limit).
- **Marks:** a keyframe diamond in every cell whose frame
  is in `stats::figure_keyframes(project, fi)` for that
  row's figure — the same pose-change rule the GPUI
  dopesheet's `keyframe_map` used, now consumed as frame
  indices instead of a boolean map.
- **Geometry:** cells reuse the chips' pitch (`CHIP_W` =
  30 pt, `CHIP_GAP` = 4 pt in `timeline_marks.rs`), and the
  lane lives **inside the same scrolled column** as the
  chips, so rows, chips and markers share one horizontal
  scroll and a diamond always sits under its frame chip.
- **Interaction:** clicking a cell seeks the playhead to
  that frame and selects that row's figure (`core.goto`
  plus the shell's figure selection — the marker row's
  press-to-seek is the precedent). No drag editing, no key
  creation or deletion from the lane in this wave.

### Landed vs planned

| Piece | State at writing |
|---|---|
| Engine data (`stats::figure_keyframes`) | Landed (pre-Wave 10; unused) |
| i18n `dopesheet` string, 28 languages | Landed (pre-Wave 10; unused) |
| Original + GPUI designs (research above) | Done — this document |
| `src/rune_sec/dopesheet.rs` lane module | **Landed** — Wave 10 (A1) |
| Wiring into the timeline card | **Landed** — Animate-gated append in `timeline_marks`, integration |
| Scene / snap / tree coverage for the lane | **Landed** — scene `dopesheet-keys`, snap tests, headless tree + snap |

Deliberately NOT in the lane (unchanged from the plan):
exposure bars, drag editing, key creation/deletion from
the lane, and any layer concept. Pose-key deletion lives
in the engine (`EditorCore::delete_keyframe`, Wave 10 A3)
and is reachable via MCP; a lane context action is a
future-wave candidate.
