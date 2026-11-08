# P19 — Render-cost review (Phase N planning)

Scope: what one full shell rebuild costs today, where the
element weight sits, and how much Phase N's card design
adds. This is a **build-cost review, not a profiler run.**

**Measured** (counted from real `--tree` dumps): every
element count in §1, the role mix, and which panels exist
in which workspace.
**Inferred** (read from the builder code, not timed):
everything in §3 about per-build work. No timings were
taken; rankings are by element count × per-element work
read from source, and say so where the evidence is thin.

Trees used (headless run 42479090089, final Phase M
tree): `tree-default.json` (Animate workspace, demo
project, nothing selected), `tree-three-d-stage.json`
(Build3D workspace + live 3D session), plus
`tree-fx-split-mid.json`, `tree-graph-duplicates.json`
and `tree-selected.json` for workspace/selection deltas.

## 1. Measured element counts

### Totals by tree

| Tree | Elements | Text | Group | Button | Image | Textbox |
| --- | --- | --- | --- | --- | --- | --- |
| default (Animate) | **942** | 373 | 263 | 229 | 73 | 4 |
| three-d-stage (Build3D) | **401** | 143 | 108 | 108 | 41 | 1 |
| fx-split-mid (Effects) | 611 | — | — | — | — | — |
| graph-duplicates (Graph) | 410 | — | — | — | — | — |
| selected (Animate + node selected) | **1,123** | — | — | — | — | — |

Only 381 of the default tree's 942 elements carry their
own key; the other 561 are unkeyed children (labels,
glyphs, row wrappers). Build cost follows the 942, not
the 733-key inventory.

### Regions (default tree)

| Region | Elements | Share |
| --- | --- | --- |
| main-row / side-right | 656 | 70% |
| main-row / side-left | 152 | 16% |
| main-row / centre | 87 | 9% |
| workspace row | 19 | 2% |
| app bar | 14 | 1.5% |
| menu bar | 9 | 1% |
| root + toasts slot | 3 | — |

The right column is the tree. It is also a scroll view
whose content runs far below the fold (the frames
section's laid-out y is ≈ 2,921 at the 900px viewport):
**off-screen panels are built in full on every rebuild —
there is no virtualisation or fold culling.**

### Panels (default tree, Animate)

| Panel (path under main-row) | Elements |
| --- | --- |
| side-right/scene-panel | **179** |
| side-right/stats-panel | 90 |
| side-right/camera-panel | 69 |
| side-right/frames section (unkeyed wrap: frames 47 + frames-this 10 + frames-insert 10) | 68 |
| side-right/inspector-figure | 67 |
| side-right/playback-controls | 61 |
| side-right/keyframe-tools | 55 |
| side-left/figures-panel | 52 |
| side-left/library-panel | 36 |
| side-left/layers-panel | 35 |
| centre/timeline block (timeline 30 + loop row 3 + wrap) | 34 |
| side-right/pose-tools | 26 |
| side-left/project-tools | 25 |
| side-right/onion-controls | 19 |
| centre/transport | 18 |
| centre/status-strip | 18 |
| centre/stage-overlay-row | 15 |
| side-right/clips-panel | 12 |
| settings / figure-add / inspector stubs | 2–4 each |
| centre/stage (single canvas element) | 1 |

### Panels (three-d-stage tree, Build3D)

| Panel | Elements |
| --- | --- |
| side-right/panel-3d | **130** |
| side-right/camera-panel | 69 |
| side-left/figures-panel | 52 |
| centre/timeline block | 34 |
| centre/status-strip | 20 |
| centre/transport | 18 |
| centre/stage-overlay-row | 15 |

### Workspace / selection deltas (measured)

- **Workspace gating works and is the main cost
  control**: Animate 942 vs Build3D 401 — gated-out
  panels build nothing (`main.rs:571` `section_visible`).
- Effects adds `fx-browser` **89** + `fx-section`
  (fx_clips) **129**; Graph adds `graph-panel` **76** +
  `easing-panel` **80**.
- **Selection is expensive**: selecting a node adds
  inspector-style **100** and inspector-segment **87**
  (selected tree 1,123 vs default 942, +181) — both
  panels exist only as stubs (3 + 2 elements) with no
  selection.

## 2. How build cost is paid (the model)

- `main.rs:554 view()` rebuilds the **entire** element
  tree from state. Rune re-runs it when state is marked
  changed; during playback `poll` advances the playhead
  and returns changed on every tick (`main.rs:392`,
  wake at `1/fps`, :416), so **playback = one full
  rebuild per frame** — 942+ elements re-created,
  re-laid-out (taffy) and diffed at 12–60 fps.
- Every key press, drag move and stepper tap pays the
  same full rebuild. There is no per-panel memoisation
  anywhere in the shell.
- Mitigations already in place (do not regress them):
  workspace gating (above); icon path parses cached
  in a thread-local (`icons.rs:56` SEG_CACHE — the
  `Shape` is still rebuilt per icon per build, 73
  image elements in the default tree); waveform peaks
  cached (below); home/library directory scans cached
  (below).

## 3. Build-cost hot spots, ranked (inferred from code)

1. **The full-rebuild model itself.** Any Phase N
   change that adds elements anywhere raises the cost
   of *every* interaction and *every playback frame*.
   This is why §4's growth estimate matters more than
   any single panel.
2. **scene-panel — 179 elements, the heaviest panel.**
   Inside it: `bg-swatches` 35 (16 backgrounds ×
   swatch + label), stage-preset row 15, bg-preset row
   13, weather-kind row 19, and **seven near-identical
   RGB/stepper sub-rows at 7 elements each** (bg R/G/B,
   fps row, stage-size row…). All rebuilt per build
   though they change rarely. It is visible in Animate,
   Effects and most workspaces, so it is paid almost
   always.
3. **figures_panel thumbnails — per-figure rig
   evaluation per build, uncached.**
   `thumb_shape` (`figures_panel.rs:357`) calls
   `thumb_prims` (:321), which re-solves the figure's
   pose and projects every segment + head ring, for
   **every figure, on every build**. Demo: 1 figure →
   panel is 52 elements. Cost scales O(figures ×
   joints) with allocations per prim; a 10-figure
   project pays 10× for a strip of 44×44 thumbnails
   that change only when a pose changes.
4. **Selection inspectors (+181 elements).**
   inspector-style (100) carries two 16-entry colour
   areas (node colour + gradient colour steppers/swatches)
   plus cap/width steppers; inspector-segment (87)
   carries per-joint steppers. Both rebuild fully per
   build while a selection exists — i.e. during all
   posing drags, the highest-frequency rebuild there is.
5. **fx_browser — 89 elements for 27 kinds**
   (`fx_browser.rs:497`; the 27-entry table at :164).
   Category headers + one button (icon + label = ~3
   elements) per kind + usage-count text, rebuilt per
   build whenever the Effects workspace is active,
   plus the filter pass over all 27 per build.
6. **Graph canvas + channel list** (`graph_panel.rs`):
   `graph_shape` (:1368) re-samples every channel's
   curve into a fresh `Shape` per build (point count
   scales with keys × channels), and the channel list
   rows rebuild with it. Graph workspace only, but
   graph editing is drag-heavy — the canvas rebuild
   rides every drag move.
7. **String formatting in always-visible builders.**
   `status_strip.rs` composes ~10 segments per build
   (segment strings + the width-fit pass over them);
   `stats_view::project_totals` (:205) walks every
   content vec and allocates a `Vec<(String, usize)>`
   plus a formatted row per total, per build (90
   elements of stats panel). Individually small;
   they are paid on *every* build including playback
   ticks. Whole-file `format!` counts overstate this
   (they include handlers/tests); the named builders
   are the real per-build sites.
8. **Waveform peaks — mitigated, with a sharp edge.**
   The view keeps a thread-local probe cache
   (`waveform_view.rs:441`, cap 8), so steady-state
   builds are cheap. But a cache miss calls
   `audio_ops::clip_peaks` (:160), which **reads and
   decodes the whole WAV from disk** and reduces it —
   per build until cached. Fine in practice; do not
   add per-build peak consumers outside the cache.
9. **Home / library lists — scan cost mitigated,
   element cost not.** Directory scans are cached
   (home: `scan_cache` keyed by dir mtime,
   `home_panel.rs:231`; row metadata behind a
   meta cache). What remains is linear element growth:
   every project/figure/pose file is a row subtree
   rebuilt per build while the Home workspace or
   library panel is up. Unbounded user directories =
   unbounded rebuild cost; no pagination exists.
10. **Timeline chips scale with authored frames.**
    30 elements for the demo's 5 frames (~6 per frame:
    chip + ruler tick + marker cell…). A 200-frame
    project pays ~1,200 timeline elements per build.
    Not a Phase N problem to solve, but Phase N's
    timeline card (§4) must not add per-chip wrappers.
11. **Stage shape** (`stage_view` + `stage_layers` /
    `stage_fx` / `stage_fig3`): one canvas element in
    the tree, but its `Shape` prim count scales with
    figures × joints + strokes + FX particles, rebuilt
    per build. Element count says "1"; the geometry
    build is real work per playback frame. Listed for
    honesty — Phase N does not touch it.

## 4. What Phase N adds — element growth estimate

Estimates per charter item, against the default tree.
"Per panel" assumes adoption restyles the existing
root `col()` into the card (charter §1) rather than
nesting a new wrapper inside it — see §5.

| Charter item | Delta | Basis |
| --- | --- | --- |
| Panel card root (fill/border/radius/pad) | **+0** | Style on the existing root element |
| Header row + divider + header icon + meta text | **+3 to +5 / panel** | Divider +1 (new), icon image +1 (some panels have one already), meta text +1, header wrapper +0–1 where a heading row already exists, +2 where it doesn't |
| Section labels from kit | +0 | Replaces hand-rolled text rows 1:1 |
| Stepper value readout (`stepper_value` wrapper) | **+1 / stepper** | A group wrapping the existing value text |
| Toggle pill replacing toggle buttons | **0 to +1 / toggle** | Button (icon+text ≈ 2–3 el.) → track group + knob (2–3 el.) |
| List-row shell | +0 | Restyles the existing row root |
| Empty states | +1 to +2 / empty panel | Icon + text (2) replacing bare text (1); only when empty |
| Status-strip hairline ticks | **+~9** | One 1×10 tick between ~10 segments |
| Timeline card wrapper | **+1** | One card col around the existing stack |
| Stage camera pill | +0 to +1 | Restyle of the existing overlay row |
| App-bar divider + dirty dot | **+2** | Divider +1, dot +1 (only when modified) |
| Kbd chips in menus / palette / help | +1 / hint, **0 in these trees** | Menus and palette are closed in both measured trees; help lives in Home workspace. Cost lands only while open |
| Menu/popover card shells | +0 to +1 / surface | Restructure of existing roots |

**Total for the default tree:** ~10 panels visible in
Animate × (+3–5 header) ≈ +35–50; steppers visible
across scene/inspector/frames/playback ≈ 25–35 × +1;
status ticks +9; timeline card +1; app bar +2;
toggles ≈ +0–10. **≈ +75 to +105 elements, i.e. the
942-element tree grows to roughly 1,020–1,050
(+8–11%).** The three-d-stage tree grows less in
absolute terms (~+40–60, +10–15% — fewer panels, same
per-panel header cost). The selected tree grows most
(two extra inspector panels take headers + stepper
wrappers): ~+110–140.

Where growth could **balloon if adoption is careless**
— the numbers adoption agents should be held to:

- Wrapping the card *inside* the existing root instead
  of restyling it: +1 col per panel **and** a second
  header row in panels that already have one → up to
  +3 extra per panel beyond the estimate.
- A divider under every *section*, not just the
  header: scene-panel alone has ~7 sections → +7
  there, ~+30–40 across the right column.
- `stepper_value` wrappers on the scene panel's RGB
  steppers are already counted; adding wrappers
  *around each stepper row* as well doubles that line.
- Kbd chips are free while menus are closed; a
  **persistent** footer/palette hint bar would not
  be. The charter does not ask for one — don't add it.
- Per-chip timeline wrappers (forbidden by §3.10
  arithmetic): +1 × frames on the one structure that
  already scales linearly.

## 5. Guidance for the adoption wave

1. Restyle roots, don't nest: the card is the panel's
   existing root with fill/border/radius/padding.
2. One divider per panel (under the header). Section
   separation comes from labels + gap, not hairlines.
3. Keep canvas counts flat: no new per-frame canvases;
   thumbnails and the graph/waveform canvases stay
   exactly as many as today.
4. Don't add per-item wrappers in scaling lists
   (timeline chips, library/home rows, fx kinds,
   swatch grids) — style the row that exists.
5. Anything that makes a §3 hot spot build *more* per
   build (extra formatting in status/stats builders,
   new uncached probes) is a regression even if the
   element count is unchanged — flag it in the report.
6. Re-measure after foundation lands: one new
   `--tree` dump of the default scene diffed against
   this section's table tells the integrator whether
   the wave stayed inside the +8–11% envelope.

*Method note: counts were taken by walking the
`elements[].path` key segments of the dumps in
`/tmp/headless-m/headless-probe-output/`; unkeyed
subtrees were identified by their keyed children
(frames section, timeline block). No code was timed
and no local build was run.*
