# Phase N — P16 Workspace-by-Workspace UX Review

## Method and evidence

Sources, in order of authority:

- `src/rune_sec/workspaces.rs` — the gating table
  (`*_SECTIONS` lists, 29 sections over 8 workspaces)
  and the per-workspace blurbs.
- `src/main.rs` `view()` (line 554) — the single
  global mount order. Both columns are built in one
  fixed order and then gated per workspace, so a
  workspace's panel order is the global order filtered
  by its section list. **Panel-order problems are
  therefore global-order problems**, and every reorder
  below is checked against the other workspaces that
  share the panels.
- Headless tree dumps (`/tmp/headless-m/headless-probe-output/`,
  run 42479090089, 1440×900, scale 2.0 — rects below
  are logical points). Measured workspaces: **Animate**
  (`tree-default.json`, plus `tree-selected.json` for
  the with-selection state), **Graph**
  (`tree-graph-duplicates.json`), **Effects**
  (`tree-fx-split-mid.json`), **3D**
  (`tree-three-d-stage.json`, `tree-three-d.json`).
  **Draw, Audio, Export and Home have no headless
  scene** — their inventories are reconstructed from
  the gating table + mount order, with heights quoted
  only for panels measured in another workspace's
  dump. The V wave should add scenes for those four;
  today they are the only workspaces with no visual
  regression coverage at all.

Reading the numbers: the side columns are fixed
300 (left) / 340 (right) wide, visible band
y 144–860 (716 pt). Elements below y 860 are scroll
content, laid out at content offsets — so a panel
"starting at y 2457" needs ~1,750 pt of scrolling
before its header appears. "First screen" below
means the 716 pt visible without scrolling.

## Shared facts (all eight workspaces)

- Chrome above the columns: app bar 25 + menu bar 25
  + workspace row 42 (chips 25 + blurb line 15).
  The blurb is the only hierarchy cue the shell
  offers today, and it is one muted sentence.
- The centre column is identical everywhere and is
  **not** a problem: stage 522, camera overlay row
  25, transport 25, timeline 54 + loop row 25,
  status strip 15. Charter items 13/14 cover its
  restyle; this review does not re-open it.
- The **Settings toggle (25 pt) takes the first
  right-column slot in every workspace**, ahead of
  whatever the workspace is for. It is ungated by
  design, but it costs all eight workspaces their
  prime slot for a control used rarely. Flagged
  once here for the foundation wave
  (`settings_popover.rs`); not counted as a
  per-workspace fix below.
- Help (when open) mounts above Settings; the
  command palette replaces the stage area. Neither
  changes the workspace inventories below.
- Every panel is flat today (charter groundwork).
  This review assumes the F-wave card kit lands and
  concentrates on what cards alone do **not** fix:
  panel order, panel weight, and what is on the
  first screen.

---

## 1. Animate — the default, and the deepest scroll

**Primary task** (blurb): pose figures and build the
animation frame by frame.

**Panel inventory in mount order** (measured,
`tree-default.json`, demo project, nothing selected):

Left (300): Project tools `project-tools` y144
h145 · Figures `figures-panel` y301 h174 ·
figure-add (ungated) y487 h25 · Library
`library-panel` y524 h314 · Layers `layers-panel`
y850 h148. Left content ≈ 998 pt; only the Layers
header sliver (~10 pt) is cut.

Right (340): Settings 25 · Segment inspector
`inspector-segment` y181 h39 · Figure inspector
`inspector-figure` y232 h389 · Style
`inspector-style` y633 h38 · Pose tools
`pose-tools` y683 h139 · Keyframe tools
`keyframe-tools` y834 h307 · Camera
`camera-panel` y1153 h369 · Scene `scene-panel`
y1534 h770 · Onion `onion-controls` y2316 h128 ·
Playback `playback-controls` y2457 h452 · Frames
`frames` y2921 h263+ · Movieclips `clips-panel`
y3185 h93 · Stats `stats-panel` y3290 h535.
**Right content ≈ 3,682 pt ≈ 5.1 first screens.**

**Hierarchy problems**

- The two panels an animator touches every few
  seconds — **Playback and Frames — sit at scroll
  depths of ~2,300 pt and ~2,780 pt**, below Camera
  (369), Scene (770) and Onion (128), which are
  set-up-once panels. The first screen is all
  inspectors; the working loop (pose → next frame →
  play) requires constant scrolling to reach its
  controls.
- Selection state makes it worse: with a joint
  selected (`tree-selected.json`) the segment
  inspector grows 39 → 487 and Style 38 → 364,
  pushing everything down a further ~770 pt. The
  39 pt no-selection stub, meanwhile, spends the
  second slot of the column on a "no joint" note.
- Scene is the tallest panel in the app (770 pt)
  and is pure one-time setup: weather kind and
  sliders, fps, stage size + presets, background
  presets + three RGB steppers + swatches, stacked
  as one flat flow with no internal grouping.

**Highest-value fixes**

1. **Integrator — `src/main.rs`:** promote Playback
   and FrameOps in the global mount order to sit
   immediately after KeyframeTools (before Camera).
   Animate-only effect in practice: FrameOps exists
   in no other workspace; Playback also mounts in
   Audio, where it already precedes the Audio panel,
   so Audio's internal order is unchanged. Playback
   and Frames land at ~y1150 content offset — one
   scroll instead of three.
2. **`src/rune_sec/scene_panel.rs`:** adopt the kit
   card with one `section_label` per existing block
   (Weather / Frame rate / Stage / Background) and
   move `scene-stage-readout`'s text ("960×720 ·
   12 fps" shape) into the header meta slot. The
   panel stays 770 pt of controls but reads as four
   labelled groups instead of one wall; no handler
   or key changes.
3. **`src/rune_sec/inspector_figure.rs`** (389 pt)
   **+ `src/rune_sec/inspector_segment.rs`** (487 pt
   when open): group the existing rows under kit
   section labels (Joints / Head ring / Placement
   in the figure inspector — `fig-ring-row`,
   `fig-measure`, `fig-place-row` already delimit
   them) and put all −/+ pairs on the kit stepper
   (charter #6), so the two tallest Animate panels
   scan as grouped controls. The segment
   inspector's no-selection state becomes the kit
   `empty_state` instead of a bare note row.

## 2. Draw — a row pretending to be a panel

**Primary task:** draw freehand strokes, shapes and
text over the figures.

**Panel inventory in mount order** (reconstructed;
no dump): Left: Figures (174, measured) +
figure-add — content ends ~y355, leaving **~360 pt
of the left column empty**. Right: Settings ·
DrawTools (root key `draw-tools-row`) ·
StrokeInspector `stroke-inspector` · TextTool.

**Hierarchy problems**

- The workspace's primary panel is not panel-shaped:
  `draw_tools()` returns a **row** (`draw-tools-row`,
  draw_tools.rs:615), so brushes, shapes, stamp and
  mirror controls will sit as loose controls even
  after neighbouring panels become cards.
- Text — half the workspace's stated purpose — is
  the **last** panel, below the stroke inspector,
  which is contextual and empty until a stroke is
  selected. First-run Draw therefore leads with
  brushes, then an empty inspector, then text.
- The left column's emptiness vs the right column's
  load is the inverse of Animate's problem: the eye
  is pulled right while the stage's figures list
  sits alone on the left.

**Highest-value fixes**

1. **`src/rune_sec/draw_tools.rs`:** rebuild the
   root on `design::panel_col("draw-tools-row")`
   (root key unchanged, per charter rule 1), header
   meta = active brush name, and `section_label`s
   for the existing blocks: Brushes (`draw-brushes`)
   / Shapes / Stamp / Mirror.
2. **`src/rune_sec/stroke_inspector.rs`:** kit
   `empty_state` for the no-stroke-selected state;
   the width and opacity −/+ pairs onto the kit
   stepper row (the panel already has both steppers
   hand-rolled: `stroke-opacity-row` and the width
   row).
3. **Integrator — `src/main.rs`:** swap the
   TextTool and StrokeInspector mount slots.
   They are co-visible **only** in Draw, so no
   other workspace changes; Draw becomes brushes →
   text → contextual stroke inspector, matching
   the blurb's order ("strokes, shapes and text"
   notwithstanding, text is a create task, the
   inspector is an edit task).

## 3. Graph — the curve editor is third, and half off-screen

**Primary task:** edit the motion curves and easing
of the selected figure's keys.

**Panel inventory in mount order** (measured,
`tree-graph-duplicates.json`): Left: Figures +
figure-add. Right: Settings 25 · **Keyframe tools
y181 h307** · Graph `graph-panel` y500 h568
(channel list `graph-channel-list` y524 h141 ·
stats y673 · channel selector y713 · **canvas
`graph-canvas` y746 h170** · value readout y924 ·
interp y947 · presets y980 · actions y1043) ·
Easing `easing-panel` y1080 h683. Total ≈ 1,763 pt
≈ 2.5 screens.

**Hierarchy problems**

- The workspace exists for the curve canvas, yet
  the canvas is the **third panel's fifth element**:
  it starts at y746 and its lower half is below the
  fold (746+170 = 916 > 860) on entry. Keyframe
  tools — a supporting panel — lead the workspace
  because the global mount order puts the
  KeyframeTools slot before the Graph slot.
- Inside the graph panel itself, the 141 pt channel
  list and its duplicate/delete/shift rows precede
  the canvas; the channel selector
  (`graph-channels`) sits *below* the stats block
  it relates to.
- Easing (683 pt) puts the curve picker
  (`ease-picker`) at its very bottom (y1340,
  ~66 pt tall), ~260 pt into the panel — the one
  visual control in a panel of steppers and lists.

**Highest-value fixes**

1. **Integrator — `src/main.rs`:** mount Graph
   before KeyframeTools. Co-visible check:
   KeyframeTools also mounts in Animate, where
   Graph is gated out, so Animate is untouched.
   The canvas rises to ~y420 — fully on the first
   screen.
2. **`src/rune_sec/graph_panel.rs`:** reorder
   children inside the panel (handlers verbatim):
   header (meta = `graph-value` readout text) →
   canvas → channel selector strip → channel list
   as kit `list_row`s (28 pt) → stats as a caption
   row. The panel's work surface leads; the list
   becomes scannable rows instead of ad-hoc rows.
3. **`src/rune_sec/easing_panel.rs`:** kit card +
   section labels (Project easing / Curve picker /
   Presets) and move the `ease-picker` block
   directly under the project-easing block whose
   curve it draws, instead of after the preset
   lists.

## 4. Effects — the worst first screen in the app

**Primary task:** add and edit particle and
visual-effects clips anchored to the scene.

**Panel inventory in mount order** (measured,
`tree-fx-split-mid.json`): Left: Figures +
figure-add. Right: Settings 25 · **Scene
`scene-panel` y181 h770** · FX browser
`fx-browser` y963 h627 (filter row y997,
preset-save y1555) · FX clips `fx-section` y1602
h637 (lane/strip y1602, clip list y1644, inspector
`fx-inspector` y1679 h560) · Particles
`particles` y2252 h68+. Total ≈ 2,320 pt ≈ 3.2
screens.

**Hierarchy problems**

- **Not one pixel of the workspace's primary panel
  is on the first screen.** Scene — gated in "for
  the scene they play in" — fills the entire first
  screen (y181–951), and the FX browser starts
  819 pt down the column. A user entering Effects
  to add an effect first meets stage-size steppers
  and background swatches.
- The FX browser itself is 627 pt: a filter row,
  then 27 kind buttons in 5 categories with recents,
  all at the same visual weight — finding one kind
  is a visual search through a wall.
- The FX clips section (637 pt) buries its 560 pt
  inspector (kind / split / playhead / visibility /
  nudge / gradient rows) below the lane and list
  with no grouping; the readout (`fx-readout`)
  that summarises the selected clip sits inside
  the inspector at y1727 instead of heading it.

**Highest-value fixes**

1. **Integrator — `src/main.rs`:** demote Scene in
   the global mount order to after the
   Particles/Audio block. Effects: the FX browser
   becomes the first panel (y963 → y181). Animate:
   Scene drops below Playback/Frames/Movieclips,
   which §1 already wants. Draw/Graph/Audio/3D/
   Export/Home: Scene is gated out or (3D) absent —
   no change. This single reorder fixes the worst
   first screen in the app.
2. **`src/rune_sec/fx_browser.rs`:** kit card;
   filter row moves into the header's trailing
   slot; one `section_label` per category with the
   kind buttons as selectable rows using
   `selected_fill()` for the active kind; recents
   directly under the header. Same buttons, same
   keys — the 27-kind wall becomes five labelled
   groups.
3. **`src/rune_sec/fx_clips.rs`:** kit card with
   the `fx-readout` string as the header meta;
   inspector blocks grouped under section labels
   (Kind / Timing / Visibility / Colour). The lane
   and clip list stay on top — they are the
   section's timeline and inventory.

## 5. Audio — the waveform has no home

**Primary task:** place sound clips and line them
up against the animation.

**Panel inventory in mount order** (reconstructed;
no dump): Left: Figures (174, measured) +
figure-add. Right: Settings · Playback
`playback-controls` (452, measured in Animate) ·
Audio `audio-panel` · Waveform — a bare canvas
mounted by `main.rs` at a fixed [308, 56] size
(`waveform_el_state`), with no card, header or
panel root of its own.

**Hierarchy problems**

- The visual centre of audio work — the waveform —
  is the only "panel" in the shell that is not a
  panel at all: no header, no caption of its own
  (the `waveform-caption` text lives inside the
  canvas element's builder), no card boundary. It
  will look broken next to the F-wave cards around
  it.
- Playback (452 pt — timecode, speed, loop and
  keyframe-nav rows) leads the workspace ahead of
  the clip list. Hearing a clip against the
  animation matters, but the primary task is
  placing clips; 452 pt of transport-adjacent
  controls precede the first clip row.
- Clip rows in the audio panel carry state
  (selected, muted, gain) that the hand-rolled
  rows express inconsistently — mute is a caption
  suffix, selection is a highlight elsewhere.

**Highest-value fixes**

1. **`src/rune_sec/waveform_view.rs`:** return the
   waveform wrapped in `design::panel_col` with a
   header whose meta is the existing
   `waveform-caption` string; the canvas keeps its
   exact size and drag behaviour. (The mount site
   in `main.rs` does not change — it mounts the
   returned element.)
2. **`src/rune_sec/audio_panel.rs`:** clip rows
   onto kit `list_row` (selected = `selected_fill`,
   muted = dimmed text), DSP/edit buttons grouped
   under one section label, and `audio-readout`
   promoted to the header meta.
3. **`src/rune_sec/playback_controls.rs`:** compress
   the 452 pt: timecode becomes the header meta in
   MONO_CAPTION (charter #12 style), the five speed
   chips become one segmented row, loop controls
   become a kit `toggle_row`. Benefits Audio
   (where it leads) and Animate (after the §1
   reorder it sits higher there too).

## 6. 3D — right panels, wrong order, one giant panel

**Primary task:** pose the 3D figure and frame it
with the camera.

**Panel inventory in mount order** (measured,
`tree-three-d-stage.json`): Left: Figures y144
h174 · figure-add y330 h25 — content ends y355,
**361 pt of the column empty**. Right: Settings
25 · Camera `camera-panel` y181 h369 · 3D
`panel-3d` y562 h1141 (preview `panel-3d-preview`
y607 h220; the remaining ~900 pt is joint steppers,
pose library and bake controls). Total ≈ 1,559 pt
≈ 2.2 screens. The status strip gains the "3D"
segment and the stage draws the figure on its grid
(Phase M) — the centre is fine.

**Hierarchy problems**

- The 2D **Camera panel (369 pt) precedes the 3D
  panel in the workspace whose camera is the 3D
  session camera**. Its dial, key list and follow
  block push the 3D preview to y607; without it the
  preview would sit at ~y230 with the first pose
  rows visible under it. (Camera's Follow block is
  the only part a 3D user needs, and it is the
  block furthest down the camera panel.)
- The 3D panel at 1,141 pt is the second-tallest
  panel in the app and reads as one undifferentiated
  flow: preview, joint readouts, rotate steppers,
  pose library, bake — no internal landmarks.
- The left column is 50% empty while the right
  column scrolls 2.2 screens — the starkest
  column imbalance of any workspace.

**Highest-value fixes**

1. **Integrator — `src/main.rs`:** mount ThreeD
   before Camera. Camera and ThreeD are co-visible
   **only** in Build3D, so this is a single-
   workspace change with zero side effects. The
   preview rises to ~y230.
2. **`src/rune_sec/three_d_panel.rs`:** kit card +
   section labels for the existing blocks
   (Preview / Joint / Pose library / Bake); the
   grid toggle, Frame button and zoom steppers
   (Phase M) form one control row directly under
   the preview canvas; the selected-joint readout
   becomes the Preview block's header meta.
3. **Observation, no adoption task:** the empty
   left column could host the 3D pose library, but
   the library lives inside `three_d_panel` today;
   splitting it out is a gating change
   (`workspaces.rs`), outside the charter's
   restyle-only scope. Recorded for a future phase.

## 7. Export — the one workspace that already leads with its task

**Primary task:** render and export the project to
images, video and other formats.

**Panel inventory in mount order** (reconstructed;
no dump): Left: Project tools (145, measured) +
figure-add (ungated — a "+ Figure" control in the
export workspace is noise, but ungated by design;
see the note below). Right: Settings · Export
`export-panel` · Stats `stats-panel` (535,
measured).

**Hierarchy problems**

- Export is the only workspace whose primary panel
  leads the right column — the order is right; the
  problem is **inside** the panel: ~15 export
  operations (images, video, data formats, range
  variants) presented as a button wall, with the
  range toggle and the "range exports are silent"
  note (`xc-range-audio-note`) competing at the
  same weight as the primary full-timeline export.
  Nothing marks the one action most users came
  for (MP4 full export).
- Stats at 535 pt is a dense readout block
  (grid + 9 total rows + media + flags + figures)
  serving as the pre-export summary, but it reads
  as raw telemetry, not a summary.
- Recent-export history (`xc-history`) is a plain
  list under the buttons — the fastest re-export
  path is the least visible element in the panel.

**Highest-value fixes**

1. **`src/rune_sec/export_panel.rs`:** kit card
   with section labels per existing category
   (Video / Images / Formats / Range); the
   full-timeline MP4/WebM buttons keep the accent
   treatment as the panel's primary action;
   `xc-range-audio-note` becomes a muted note row
   under the Range label; history entries become
   kit `list_row`s directly under the header.
2. **`src/rune_sec/stats_view.rs`:** totals, media
   and flags as compact MONO_CAPTION readout rows
   under section labels (Totals / Media / Flags),
   cutting the panel's visual weight without
   removing a single number. Shared with Home (§8)
   — one file, two workspaces fixed.
3. **Note for the foundation wave:**
   `figure_add_panel` is ungated, so Export (and
   Home) show a figure-creation toggle in their
   left columns. `workspaces.rs` documents the
   choice; if it is revisited, Export is the
   workspace that gains most. No adoption change
   proposed here.

## 8. Home — the welcome screen reads as telemetry

**Primary task:** open, create and manage
projects, and check project stats.

**Panel inventory in mount order** (reconstructed;
no dump): Left: Home `home_panel` (project list) ·
Project tools (145, measured) · figure-add.
Right: Settings · Stats (535, measured) — the
**only** right-column panel.

**Hierarchy problems**

- The first screen a returning user sees pairs a
  hand-rolled project list (name + meta line +
  Open + two-step delete per row) with 535 pt of
  project statistics on the right. The stats —
  which describe the *currently open* project —
  outweigh the project list that is the screen's
  actual purpose.
- The two-step delete is a safety pattern whose
  armed state is expressed only by button text;
  nothing visually distinguishes "Delete" from
  "Press again to really delete" at a glance.
- The totals footer (`home-totals`) sits at the
  bottom of the scroll; the newest-first list
  (Phase K sort) has no selected/current row
  treatment for the open project.

**Highest-value fixes**

1. **`src/rune_sec/home_panel.rs`:** project rows
   onto kit `list_row` — meta line (figures ·
   frames · fps · size · modified, Phase M) as the
   row's secondary text; the armed-delete row gets
   an error-tinted fill so the second press reads
   as dangerous before it lands; `home-totals`
   moves to the header meta; kit `empty_state`
   when no projects exist. All keys (`home-proj-*`,
   `home-proj-del-*`, `home-open-*`) unchanged.
2. **`src/rune_sec/project_tools.rs`:** kit card +
   header with `proj-current-meta` as the header
   meta. This one file leads the left column in
   Animate and Export and follows Home here — one
   adoption, three workspaces.
3. **`src/rune_sec/stats_view.rs`:** the shared
   compact-readout adoption from §7. In Home it is
   the whole right column, so its weight *is* the
   workspace's balance problem.

---

## Priority summary for the integrator

The four mount-order changes are one-slot moves in
`src/main.rs` `view()` with the largest first-screen
gains in the phase, each verified above against
every workspace that shares the affected panels:

| # | Move | Workspaces affected | Gain |
|---|------|--------------------|------|
| 1 | Scene demoted below Particles/Audio block | Effects, Animate | FX browser: y963 → y181; Animate Scene below the working loop |
| 2 | Playback + FrameOps promoted above Camera | Animate (Audio unchanged) | The pose→play loop's controls: ~2,300 pt → ~1,000 pt scroll depth |
| 3 | Graph mounted before KeyframeTools | Graph only | Curve canvas fully on the first screen |
| 4 | ThreeD mounted before Camera | 3D only | 3D preview: y607 → ~y230 |

Everything else in this review is per-file adoption
work on the charter's card kit and needs no
integrator decision. Two items are explicitly
**not** proposed, with reasons: moving the curve
editor or waveform into the centre column (a
layout change beyond the charter's restyle scope —
the centre is shared furniture in all eight
workspaces), and re-gating sections between
workspaces (the gating table is product behaviour,
not styling).
