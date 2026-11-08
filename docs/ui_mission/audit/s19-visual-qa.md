# S19 — Visual QA & Performance Audit (Phase 1)

Squad 19's evidence base is the Phase N headless run
**42488647081** (final Phase N tree, commit ebdda27):
13 tree dumps + 10 snaps in the probe artifact, all 23
probes exit 0, every tree reporting `warnings: []`.
Method: an independent re-run of the content/window
rule scan over all 13 trees (the method the stopped
Phase O's p05 plan specified — its row table is
confirmed below and extended), a sibling-overlap scan,
cross-scene geometry comparison, pixel sampling of
every snap, and a pixel read of 8 of the 10 snaps.

**Evidence classes used throughout.** *Tree-proven* =
measured from element rects in the dumps. *Snap-proven*
= visible in a rendered snap. *Unverifiable* = this
evidence base cannot settle it, however plausible.
Nothing below is claimed from source reading alone.

**Headline counts.** Tree-proven: **22 rows overflow
their card's content edge** (one defect family; 3 rows
are new versus the p05 plan's table), plus **2 layout
anomalies** (palette-open centre compression;
menu-open +24 pt geometry shift). Snap-proven: **4
further visual defects** plus confirmations of 6
tree findings. Unverifiable: **6 classes**, led by
the light theme — see §6.

---

## 1. The overflow family (tree-proven, 22 rows)

Rune does not clip: a row wider than its card paints
past the card and past the window, and the excess
controls cannot be clicked (the columns scroll
vertically only). Overflow = worst child's right edge
minus the content edge (left 304, right 1412). All
rows from the p05 table re-measured and confirmed;
rows marked ★ are additions from this audit's scan.

| row (scene evidence) | overflow | past window? |
|---|---|---|
| `draw-brushes` — brushes 4–9 (draw-open) | **+563** | yes, to x 1975 |
| `draw-tools-row` — clear-drawing (draw-open) | +374.5 | yes |
| `graph-actions` — graph-clear (graph-duplicates) | +305.5 | yes, to x 1717 |
| ★ `stats-grid` — stat-duration / stat-root-travel (all Animate trees) | +153.5 | yes, to x 1565 |
| `layer-row-0` — layer-top-0 + unkeyed tail (Animate trees) | +134 (+186 tail) | no |
| `draw-shapes` — stamp-arrow (draw-open) | +132.5 | yes |
| `draw-stamp-params` — draw-rot-up (draw-open) | +120.5 | yes |
| `wx-kind` — wx-seg-5 (Animate + fx trees) | +118.3 | yes |
| settings language rows — settings-lang-sl (settings-open) | +51 | yes |
| `orbit` row — orbit-3d (three-d trees) | +49 | yes |
| `keyframe-presets` — key-preset-linear (Animate trees) | +45 | yes |
| `wx-kind` second segment — wx-seg-4 | +42.4 | yes |
| 3D camera row — cam-frame-3d (three-d trees) | +37.5 | yes |
| playback loop row — pb-loop-end + unkeyed child (loop-range) | +36 (+168 child) | yes |
| ★ segment bend row — seg-bend-zero (selected) | +34.5 | yes |
| `stats-grid` second pair — stat-strokes / stat-busiest | +35.5 | yes |
| camera key row — cam-delete-key (most trees) | +29.4 | yes |
| `fx-nudge` — fx-dur-up (fx-split-mid) | +27.5 | at edge (1439.5) |
| playback row — pb-next-key (Animate trees) | +17.6 | no |
| pose row — pose-apply-all (Animate trees) | +16.5 | no |
| ★ stroke bulk row — stroke-delete-frame (draw-open) | +15.9 | no |
| `graph-presets` — graph-preset-linear | +13.5 | no |
| `keyframe-actions` — key-auto | +11.5 | no |
| panel-3d bake row — bake-3d | +9.5 | no |
| `keyframe-clipboard` — key-clear-channel | +2.8 | no |

Notes on the additions:

- ★ **stats-grid is the worst *new* find.** The grid
  container itself is inside budget
  [1096, 3777, 316, 113], but its children are laid
  out as if the grid were far wider: stat-frames
  starts at x 1168.5 and stat-duration at x 1455.5 —
  the second column begins past the card edge. The
  stats panel sits at content y 3733 (≈ the 6th
  viewport of the right column), so no snap shows
  it; it is tree-proven only, and it affects every
  Animate-workspace tree.
- ★ **seg-bend-zero** (segment inspector, selected
  scene): the bend row's Zero button ends at
  x 1446.5 — off-window in the app's most-used
  inspector state.
- ★ **stroke-delete-frame** (stroke inspector bulk
  row): +15.9; the draw snap shows its label flush
  against the window edge.

Two scan hits were checked and **cleared as false
positives**: `settings-toggle` and `settings-card`
end at exactly x 1424 because they *are* the card
(the collapsed settings strip and the expanded card
root) — full-width by design, confirmed in the
settings snap.

## 2. Layout anomalies (tree-proven)

- **Palette-open compresses the centre to zero.**
  The command palette is in-flow in the centre
  column [328, 157, 540, 521], not an overlay: the
  stage collapses to height **0** at y 690, the
  timeline is pushed to y 792, and the status strip
  lands at y 893..**908 — 8 pt past the window
  bottom**. Snap-confirmed (§3): the strip's text is
  half-clipped and only the figure's head and
  shoulders peek out below the palette card. Every
  Ctrl+K costs the user the stage and the status
  strip's bottom half.
- **Menu-open shifts the centre +24 pt.** With the
  File menu open, main-row/centre height is 727
  (vs 703 in every other tree) and the stage grows
  485 → 509; everything below sits 24 pt lower. The
  snap shows no visual defect (the strip still
  fits), and the cause is not determinable from the
  dumps — recorded as an unexplained geometry
  variance for Squad 03/20; layouts that move when a
  menu opens are worth understanding before the
  shell is redesigned on top of them.
- Explained variances, **not** defects: fx-split-mid
  stage height 473 (the timeline clips lane takes
  12 pt — by design since Phase M); three-d and
  three-d-stage trees are geometrically identical
  (the scenes are the same state by design).

## 3. Snap-proven findings

Confirmations (tree defect visible in pixels):
settings language chips clipped mid-label at the
window edge ("Italia…", "Slovenš…", a Hebrew chip
cut in half); draw rows clipped ("Mirror s…", the
5th brush chip a sliver at the edge, the stamp
"Rot" stepper off-card); 3D "Orbi…" button cut at
the card edge; pose "Apply pose to all frames"
flush-cut at the card edge; palette-open status
strip half-clipped (§2).

New from the snaps:

1. **Settings guides text truncated mid-sentence.**
   "Stage guides: thirds grid + centre cross.
   Camera frame ar…" runs off the settings card's
   right edge — a wrapping failure in a plain text
   row, same family as §1 but in body copy.
2. **Tofu boxes for two language names.** In
   snap-settings-open, the Bengali (বাংলা) and
   Japanese (日本語) chips render as □□□□. This is
   snap-proven *for the probe's font stack*
   (lavapipe CI fonts); a desktop install with
   fuller fonts may render them. Flagged, not
   generalised: the mission should re-check on a
   real desktop, because a minimal Linux install
   would show exactly this.
3. **Palette kbd chip contrast.** The "Space" chip
   on the selected palette row is pale text on the
   accent fill — legible but washed (the documented
   4.19:1 white-on-accent acceptance; restated here
   because the mission brief asks for it to be
   re-examined, not because it changed).
4. **Status hint ellipsis.** The right-end hint
   truncates ("Drag a joint to…") in the default,
   settings, menu and 3D snaps, and fx-split-mid
   shows a whole status segment collapsed to "…".
   By-design ellipsis behaviour, cosmetically
   unfinished: the strip silently drops information
   at 1440 pt in the shell's most common states.

Cleared by pixel read: the **File dropdown** renders
cleanly (3 items — Save / Load / Export PNG — which
is the real File menu, not a scene artefact); the
**Effects browser** is fully inside its card in both
fx snaps (its "No presets saved yet" line is bare
text, a known Phase O seed item, not a layout
defect); the **stage crop** renders the figure and
onion ghost cleanly inside the rounded stage; the
**figures panel** (Phase N fix 5) holds — Recolour /
Duplicate / eye all inside the card in every snap.

## 4. Panel widths & alignment (tree-proven, clean)

The shell's skeleton does not wobble: `side-left`
[16, 157, 300, 703] and `side-right`
[1084, 157, 340, 703] are **identical in all 13
trees**; the stage is x 328 / w 744 in all 13; the
app bar and workspace row are byte-identical rects
across scenes; status-strip internals (15 pt
segments, centred 1×10 dividers) match between
default and palette trees. The sibling-overlap scan
found **no unintended overlaps** in any tree — the
only positive-area sibling intersections are slider
thumbs over their tracks (by design) and the File
dropdown over the rows beneath it (overlay by
design). Alignment debt is therefore concentrated
entirely in §1's row widths, not in the frame.

## 5. Element counts & growth risk (tree-proven)

| scene | elements | | scene | elements |
|---|---|---|---|---|
| default | 1,018 | | loop-range | 1,047 |
| frame-3 | 1,018 | | menu-open | 1,038 |
| playing | 1,018 | | palette-open | 1,082 |
| draw-open | 436 | | settings-open | 1,176 |
| graph-duplicates | 423 | | **selected** | **1,193** |
| three-d / -stage | 413 | | fx-split-mid | 674 |

- Phase M → Phase N default growth was 942 → 1,018
  (+8.1%); the heaviest state (selected) is already
  +17.2% over default, at 1,193 elements for a
  **5-frame, 1-figure demo project**.
- The count scales with project content: frame
  chips, figure rows, layer rows and their subtrees
  are instanced per item (381 keyed elements in
  default, 443 in selected). A 200-frame project
  multiplies the timeline subtree alone by ~40×
  over the demo. No frame-time or layout-time data
  exists in the artifacts, so any performance claim
  beyond "the tree grows linearly with content and
  the demo baseline grew 8% in one phase" is
  unverifiable here. Recommendation: the mission's
  QA phase should add a big-project scene (e.g.
  100 frames × 5 figures) to the probe set so
  growth becomes a measured number, not a worry.

## 6. Themes — the light-theme verdict

**The light theme has never been rendered in any
verification artifact. Verdict: visually
unverified — a gap, and itself a defect in the
verification system.**

- All 15 scenes in `scenes()` are dark: no scene
  function touches `light_theme` (the only "light"
  in scenes.rs is the word "flight").
- Both headless workflow loops iterate the same 13
  scenes; there is no light scene to probe.
- Pixel proof: all 10 snaps sample to background
  (18, 19, 20) = #121314 at every corner — dark,
  uniformly. There is not one light pixel of
  evidence in the set.
- The only light-theme coverage anywhere is one
  Runtime test (`settings_theme_buttons_flip_core_
  light_theme`) asserting the settings buttons flip
  the `core.light_theme` **boolean**. It proves the
  switch is wired; it says nothing about what the
  light theme looks like — contrast, borders, or
  whether any panel hardcodes dark colours (Squad
  02/18's audit territory, but only a light snap
  can prove it).
- The mission brief (§7) demands equal attention
  for both themes and the owner runs the app on a
  real desktop: until a `light` scene exists and
  is snapped, every light-theme statement in this
  mission is a guess. **This is the audit's top
  recommendation: add the scene before Phase 2
  token work begins, so the token delta is checked
  against both themes from the start.**

## 7. Scroll behaviour

Tree-proven: scrolling is load-bearing. In the
default tree the left column's content reaches
y 1204 (≈ 1.7 viewports of 703) and the right
column's reaches **y 4281 (≈ 6.1 viewports)**;
with settings open the right column reaches
y 5113 (≈ 7.3 viewports). Most Animate panels —
playback, camera, scene, stats — live below the
first fold, which is precisely why §1's deep
overflows (stats at y 3733) hide from snaps.

Unverifiable from static dumps: wheel response,
scroll clamping at the ends, scrollbar visibility
and hit area, and whether the horizontal-overflow
rows are reachable by any scroll gesture (the
columns scroll vertically; on the tree evidence the
§1 excess is unreachable, but a real pointer test
is the proof, and none exists in this base).

## 8. Coverage gaps in the evidence base itself

- **`export-open` has no artifact at all** — no
  tree, no snap. The Export workspace's open state
  has zero visual evidence. (It is in the 15-scene
  table but in neither workflow loop.)
- **`fx-selected` has a snap but no tree** — its
  layout cannot be measured, only viewed (it reads
  clean at scroll position 0).
- **One window size only.** All probes are
  1440×900; the workflow notes there is no
  window-size flag. The brief's other five sizes
  (1280×720 → 2560×1440) are unverifiable until
  the probe gains a size parameter. At 1280 wide
  the fixed 300/340 columns leave the centre
  ≈ 608 pt — every §1 overflow persists and the
  centre shrinks; that is arithmetic, not evidence.
- **One instant only.** Dumps are a single frame at
  rest: hover, press, focus, drag and animation
  states (the Phase O "feel" territory) have no
  evidence here at all.

## 9. Routing

- §1 overflow family → already planned as Phase O
  items O-1…O-9 (flex_wrap / two-line anatomy);
  add the three ★ rows to that plan. Owning squads:
  11 (draw), 12 (graph), 07 (layers), 08 (segment
  bend), 16 (stats), 15 (3D/camera), 13 (fx), 09
  (playback/pose).
- §2 palette compression → Squad 03/20: the palette
  as an overlay, or a centre layout that reserves
  its height without zeroing the stage.
- §6 light theme → Squad 19 + 02: one `light`
  scene + snap in the probe set before token work.
- §5 growth → Squad 19: one big-project scene in
  the probe set; report its element count beside
  default's in every future debrief.
- §8 export-open → Squad 16/19: add the scene to
  both workflow loops (one line each).

*Method note: the scan script was run fresh for
this audit over run 42488647081's dumps; p05's
numbers were treated as claims and re-measured —
all reproduced within 0.5 pt.*
