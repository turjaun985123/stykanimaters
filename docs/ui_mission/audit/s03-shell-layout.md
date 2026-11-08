# S03 Audit — Application Shell & Window Layout

> Squad 03, Phase 1 of the Ultimate UI/UX Mission.
> Scope: `src/main.rs` (layout composition), `src/shell.rs`
> (layout persistence), with geometry evidence from the
> Phase N headless tree dumps and the overflow measurements
> in `docs/phase_o_plans/p05-overflow-widths.md`.
> Method: source audit + arithmetic from the layout code.
> No size other than 1440×900 has ever been rendered by CI
> (see defect D2) — per-size verdicts below are computed
> from the layout rules, and say so.

## 1. Current geometry (the reference size, 1440×900)

Window spec (`main.rs`, `fn main`): default size
**(1440, 900)**, `min_size: Some((1280.0, 720.0))`, no
`max_size`. The minimum is real: Rune passes
`spec.min_size` into the winit window attributes
(`rune-src/crates/rune/src/window/create.rs`,
`set_min_inner_size` in `window/float_on_open.rs`), so
the OS refuses a smaller window. Nothing below 1280×720
needs a verdict.

Root (`view` in `main.rs`): a full-window column,
padding 16, gap 12:

1. app bar (`chrome_view::app_bar`)
2. menu bar (`menus::menu_bar`)
3. workspace row (`workspaces::workspace_row`)
4. hairline divider (charter §15)
5. `main-row` — `grow(1.0)`, `min_h(0.0)`, gap 12
6. toasts, menu scrim, menu dropdown (overlay children)

The chrome block (1–4) measures ≈141 pt tall including
gaps: the Phase N headless tree puts the stage at
y = 157 (= 16 padding + 141).

`main-row` is three columns:

| Column | Width rule | Content |
|---|---|---|
| Left | scroll viewport, **fixed `w(300.0)`** | panel cards, content budget **276** (300 − 24 card padding) |
| Centre | `grow(1.0)`, `min_w(0.0)` | stage + transport + timeline + status |
| Right | scroll viewport, **fixed `w(340.0)`** | panel cards, content budget **316** (340 − 24) |

Centre width = W − 32 (root padding) − 300 − 340 − 24
(two gaps) = **W − 696**. At 1440: **744 pt** — matches
the measured tree exactly (stage rect [328, 157, 744,
485], Phase N).

Both side columns are **vertical scroll viewports**
(`ScrollSpec`), so panel stacks of any height are safe;
heights were never the problem (the Phase J comment in
`view` records this). The centre does **not** scroll:
the stage canvas is its only flexible element
(`grow(1.0)`, `min_h(0.0)`), and everything else in the
centre is fixed-height furniture:

- stage-overlay row (camera dial strip)
- transport row (`chrome_view::transport_row` — icon
  buttons around a `grow` spacer; ≈270 pt of fixed
  content, fits every centre width in scope)
- timeline card (chips `CHIP_H` 24, ruler `RULER_H` 12,
  markers `MARKER_H` 14, clips lane `CLIPS_H` 10;
  measured ≈104 pt with the loop row; frame chips live
  in a horizontal scroll viewport, so long frame counts
  scroll instead of overflowing)
- status strip, 15 pt tall

Centre furniture totals ≈235 pt; the stage takes the
rest. At 1440×900 the stage is **744 × 485 = 27.8 % of
the window area** (51.7 % of width, 53.9 % of height).

Visibility toggles: `State::show_left` / `show_right`
(View menu) unmount a column entirely; the centre then
absorbs its width too (one column hidden: centre =
W − 344; both hidden: W − 32).

Persistence (`src/shell.rs`, `LayoutPrefs` in
`./projects/.layout.json`): workspace, dark,
timeline_collapsed, sidebar_visible, palette_recents.
**No window size, no column widths** — the 300/340
widths are literals in `view()` and cannot be changed
by the user at all.

## 2. Per-size verdicts

Centre = W − 696 (both columns shown). Stage height ≈
(H − 185 chrome/main-row allowance) − 235 furniture.

| Size | Centre width | Stage ≈ | Stage share of window | Verdict |
|---|---|---|---|---|
| **1280×720** (the enforced minimum) | 584 | 584 × ~300 | ~19 % | **Works, tight, one real defect (D1)** |
| 1366×768 | 670 | 670 × ~348 | ~22 % | Works; D1 partially reachable |
| 1440×900 (reference) | 744 | 744 × 485 (measured) | 27.8 % | Works; verified by CI headless |
| 1600×900 | 904 | 904 × ~485 | ~31 % | Works |
| 1920×1080 | 1224 | 1224 × ~660 | ~39 % | Works well |
| 2560×1440 | 1864 | 1864 × ~1020 | ~52 % | Works; proportions skew (D5) |

Why no size hard-breaks the shell: the side columns are
fixed-width and vertically scrolling, the centre is the
single absorber in both axes, and the stage is the
single flexible element inside it. All variance lands
on the stage — which is the correct design — so the
failure modes are *squeezes and overflows of fixed-width
content inside the centre*, not collapsed layout.

### 1280×720 in detail (worst size)

- Stage ≈ 584 × 300: the project stage (e.g. 960×540)
  fits at ~55 % scale. Usable, not comfortable; the
  centre furniture (235 pt) consumes ~44 % of the
  main-row height. The persisted timeline-collapse
  option is the user's only relief.
- **The status strip does not fit** — see D1.
- Panel-internal row overflows (D3) are exactly as bad
  as at 1440: they live in fixed-width columns and
  never heal with window size.

### 2560×1440 in detail

- Nothing breaks. The stage takes ~52 % of the window.
- The side columns stay 300/340 — 12 % / 13 % of the
  width — so panels look comparatively narrow and the
  chrome rows span a very wide window. A 4:3-ish project
  stage letterboxes inside the wide stage card with
  large flat margins. Cosmetic only (D5).

## 3. Panel resizing — the honest limitation

**The app has no panel resizing, and Rune ships no
splitter.** Evidence:

- Column widths are hard-coded literals (`.w(300.0)` /
  `.w(340.0)` in `view()`); `LayoutPrefs` has no width
  fields (§1).
- Searching `rune-src/crates` (rune core + rune-kit) for
  a splitter / split-pane component finds none; the only
  `resize` hits are rune-kit's table-column internals,
  which this app does not use.
- Rune's layout is taffy flexbox (`rune-taffy`): widths
  are whatever the element's style says. Nothing in the
  framework resizes a side panel on its own.

What *is* possible, stated plainly so nobody later
claims it was impossible or that it already exists:
`El` supports `on_drag` (the stage canvas uses it), so
a hand-built drag divider — a thin element whose drag
writes a width into `State`, persisted as new
`LayoutPrefs` fields — is buildable from framework
primitives. It is a **feature build, not a setting**:
one coder for the splitter + width state in `main.rs`
(integrator-owned, see C3), one for persistence and
tests in `shell.rs`. Until that is scheduled and built,
the correct product statement is: *side panels are
fixed-width; columns can be shown or hidden from the
View menu.*

## 4. Defects

### D1 — HIGH — Status strip overflows at the app's own minimum size

`status_strip.rs` pins its own arithmetic: default
fixed segment floors total **512 pt**; the audio
segment adds 64 (**576**); the Phase L speed segment
adds up to 32; the Phase M 3D segment ~24 more; the
row's 8 pt gaps and dividers between ~9 segments are
uncharged in that accounting (~64–80 pt). Worst case
≈ **690 pt of floored content in a 584 pt strip** at
1280 wide. Floors are `min_w` — the layout cannot
shrink them. The flexible tail (hint text) truncates
to zero first; beyond that, floored segments paint
past the strip's right edge — the exact failure Phase J
fixed at the old ~461 pt centre, returning at the size
the app itself declares its minimum. Even the plain
default state (512 + gaps ≈ 580) is borderline at 584.
At 1440 the documented worst default+audio case has
8 pt to spare (576 + 160 tail target = 736 ≤ 744), so
this is invisible in every existing screenshot.
Reachable at 1366 too (670 centre) whenever audio +
speed + 3D segments coincide.

### D2 — HIGH — No automated evidence at any size except 1440×900

Scenes render at the AppSpec size; every CI headless
probe and every pixel verdict in Phases J–N is
1440×900. The mission's other five sizes have **zero**
tree/snap coverage — D1 survived precisely because of
this. Rune's headless runner accepts a size override
(`crates/rune/src/tooling/headless.rs` reads
`app_options.min_size`/size limits), but the workflow
never uses it.

### D3 — HIGH (cross-squad) — Panel-internal rows overflow at every size

Because the columns are fixed, rows measured by the
p05 audit at 1440 are broken identically at all sizes
(source: `docs/phase_o_plans/p05-overflow-widths.md`,
from tree dumps, not estimates): `draw-brushes` 879 pt
vs the 316 budget (**+563; the last chips end at
x ≈ 1975 — six of ten brushes unclickable even at
1440**), `draw-tools-row` +374.5, `graph-actions`
+305.5 (`graph-clear` ends at x 1717.5, off-window at
1440), layers row +134, scene weather segments +118.3,
settings language chips +51 past the card edge. Shell
squad's interest: these are the strongest argument
*against* treating column width as the fix — widening
the right column to fit an 879 pt row is not a layout
option; the rows must wrap/split (p05 items O-3…O-9,
already sized for single coders). Flagged here so the
mission roadmap keeps them as the top defect family;
ownership stays with S11/S12/S07 and the p05 plan.

### D4 — MEDIUM — Toasts resize the stage

`toasts_view` is a flow child of the root column,
after the `grow(1.0)` main-row. When a toast appears
it takes vertical space and the main row — stage
included — shrinks; when it expires the stage grows
back. Every status change (saves, exports, errors)
therefore makes the canvas jump. Professional tools
overlay notifications; they do not re-layout the
document. (Menu scrim/dropdown share the same flow
pattern but are zero-height overlays in practice.)

### D5 — LOW — Large-size proportions

At 1920+ the fixed columns read narrow against the
window, and non-16:9 project stages letterbox inside
an ever-wider stage card. No functional defect;
recorded so Phase 7's theme/size pass looks at it
knowingly rather than re-discovering it.

### D6 — LOW — Small-height squeeze has one relief valve, poorly surfaced

At 720 high the centre furniture is ~44 % of the
main row. Timeline collapse exists and persists
(`LayoutPrefs.timeline_collapsed`) but the toggle's
discoverability is a chrome/UX question (S04/S18),
noted here because the shell owns the geometry it
relieves.

## 5. Changes sized for single coders

> ⚠ **Every change touching `src/main.rs` needs
> integrator coordination** (mission rule: one owner
> per file; `main.rs` is integrator-owned). Items are
> sized for one coder *after* the integrator assigns
> the file.

- **C1 — D1 fix (1 coder, INTEGRATOR coordination:
  `main.rs` view + `status_strip.rs` owner).**
  Small-centre strip behaviour: below a centre width
  of ~680, the strip sheds its lowest-priority fixed
  segments (selection, then undo count) before the
  tail is consumed, keeping project/frame/play always
  visible. Segment floors and the pinned arithmetic
  test in `status_strip.rs` must be updated together —
  the test exists precisely to force this.
  Acceptance: strip fits 584 with audio+speed+3D
  segments live, proven by the pinned test re-run at
  the new width + a 1280×720 tree dump (needs C2).
- **C2 — D2 fix (1 coder, tooling only:
  `.github/workflows/headless.yml`, maybe `scenes.rs`).**
  Add a minimum-size probe pass at 1280×720 (tree +
  default/palette/settings snaps) using Rune headless's
  size override, with the `./projects` wipe discipline
  the Phase N probes already follow. No app code
  changes. Acceptance: CI headless artifact contains
  `*-1280` trees; the overflow scan (p05 O-2) runs
  over them.
- **C3 — D4 fix (1 coder, INTEGRATOR: `main.rs` +
  `toasts.rs`).** Take toasts out of the root flow:
  overlay them over the main row's bottom-right (Rune
  overlay/presence primitives, as menus already do),
  or reserve a fixed-height toast lane so the stage
  never resizes. Acceptance: stage rect identical
  with zero, one and three live toasts in a headless
  tree comparison.
- **C4 — D3 (not re-planned here).** Execute p05
  items O-3…O-9 under their owning squads. Shell squad
  reviews the before/after overflow scan.
- **C5 — Panel resizing (feature; needs a product
  decision before any coder is assigned).** Drag
  splitter per §3: coder A — splitter element + width
  state in `main.rs` (INTEGRATOR), coder B —
  `LayoutPrefs` width fields + round-trip tests in
  `shell.rs`. Do not start as part of a polish pass;
  it changes the shell's geometry contract (budgets
  276/316 become ranges, and every panel audit that
  assumes them must be re-based).

## 6. What the shell squad did NOT find

- No size at which the three-column structure itself
  collapses, overlaps, or loses a column: fixed
  columns + one absorber is robust from 1280 to 2560.
- No vertical overflow risk in the side columns
  (scroll viewports, bounded by the `grow` main row,
  `min_h(0.0)` — the Phase J analysis still holds).
- No horizontal timeline risk: chips scroll.
- Transport row fits the narrowest centre (584) with
  room to spare.

## Squad deliverable checklist

1. Files inspected: `src/main.rs`, `src/shell.rs`,
   `src/chrome_view.rs` (transport), `src/rune_sec/
   timeline_marks.rs`, `src/rune_sec/status_strip.rs`,
   `docs/phase_o_plans/p05-overflow-widths.md`,
   rune-src window/headless/taffy sources.
2. Problems discovered: D1–D6 above.
3. Changes implemented: none (Phase 1 is audit-only).
4. Components reused / added: none.
5. Behaviour preserved: n/a — no code touched.
6. Verification: arithmetic cross-checked against the
   Phase N measured tree (744 centre, stage [328, 157,
   744, 485]) and the status strip's own pinned floor
   accounting; per-size numbers are computed, not
   rendered — D2 is the plan to close that gap.
7. Known limitations: this audit did not render any
   non-1440 size (no local builds allowed; CI scenes
   are 1440-only until C2).
8. Remaining defects: D1–D6, owned as marked.
