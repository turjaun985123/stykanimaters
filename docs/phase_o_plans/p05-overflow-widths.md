# P05 — Overflow & Widths

Charter rule 5: width budgets are stated, not hoped —
left column content **276 pt** (x 28..304), right column
content **316 pt** (x 1096..1412), cards themselves
300/340 (right edges 316/1424), window 1440. Nothing may
cross a card edge (the figures-panel lesson, Phase N
fix 5).

**Evidence base.** Everything below is measured, not
estimated: the integrator's Phase N headless run
**42488647081** tree dumps (all 13 scenes), scanned with
two prototype passes (§2 formalises them). The settings
remainder Phase N recorded is real — and it is not
alone. The same defect family (a row whose children sum
wider than its card; Rune does not clip, so the excess
paints past the card and past the window, unreachable by
the mouse) appears in **at least 11 rows across 9
files**. Worst single row: `draw-brushes`, +563 pt.

---

## 1. O-1 — Settings language grid repair (1 coder)

**File:** `src/rune_sec/settings_popover.rs` (grid
builder at :746–754; `LANG_COLUMNS` at :264).

**Measured defect** (tree-settings-open, content right
edge 1412): the grid chunks 28 languages 4-per-row by
count (`chunks(LANG_COLUMNS)`, gap 6) regardless of
width. Chip widths run 51.5–159.5 pt (`Español
(Latinoamérica)` 159.5, `Português (Brasil)` 123.5,
`Bahasa Indonesia` 121.0). Four chips cross the edge:

| chip | right edge | past content edge | past window (1440) |
|---|---|---|---|
| `settings-lang-sl` (row 5) | 1463.0 | **+51.0** | +23.0 |
| `settings-lang-he` (row 2) | 1450.5 | **+38.5** | +10.5 |
| `settings-lang-it` (row 3) | 1428.5 | +16.5 | — |
| `settings-lang-pl` (row 4) | 1427.0 | +15.0 | — |

**Fix (exact builder change).** One call added at
:748 — the row learns to wrap:

```rust
let mut lang_row = row()
    .key(format!("settings-lang-row-{i}"))
    .gap(6.0)
    .flex_wrap();
```

Rune measures each chip's real text width at layout,
so wrapping packs by true width on any machine/font —
no width table to maintain, and a 29th language (the
entries derive from `language_entries()`, :293) can
never re-break the card. Precedent: the Phase M
`flex_wrap` fix in frame_ops. All keys verbatim
(`settings-lang-grid`, `settings-lang-row-{i}`,
`settings-lang-{code}`); rows that wrap grow taller
(grid +≈29 pt per wrapped row, 4 rows affected ≈
+116 pt of scroll height — the settings card already
lives in the column's vertical scroll; acceptable and
stated).

**Contingency** (only if the post-fix scan still shows
a hit — e.g. wrap does not engage without a bounded
width): add `.w_full()` to the same row builder. If
both fail, fall back to `LANG_COLUMNS` 4 → 3 —
recorded here as fragile and NOT recommended: the
worst 3-row (de 68 + el 75.5 + es-419 159.5 + 12 gap
= **315.0 of 316**) fits by 1 pt at headless font
metrics and can re-break under real Indic/CJK fonts.

**Proof:** headless `settings-open` tree + the §2 scan
— zero language chips past 1412.5; integrator pixel
read of snap-settings-open (every language name fully
inside the card).

---

## 2. O-2 — The overflow audit method (1 coder, tooling only)

A repeatable scan the V wave runs over **all 15 scene
dumps** after every headless dispatch. Prototype
already written and run by this plan (results in §3);
the item turns it into a kept script,
`scripts/overflow_scan.py`, with this exact spec:

**Inputs.** `tree-*.json` from the headless artifact
(each element: `key`, `path`, `rect`). Paths encode
ancestry (`#0/main-row/side-left/figures-panel/…`), so
no app change is needed.

**Rules** (tolerance 0.5 pt everywhere):

1. **Content rule.** Every element whose path contains
   `/side-left/` must have right edge ≤ **304.5**;
   `/side-right/` ≤ **1412.5** — *except the card roots
   themselves* (a card is 300/340 wide by design; its
   own right edge may reach 316/1424). Card root = the
   path segment directly under `side-left`/`side-right`.
2. **Card rule (defence in depth).** Every descendant
   of a keyed card root must lie within the card root's
   own x-range (catches a card placed somewhere the
   column rule does not cover).
3. **Window rule.** No element anywhere may cross
   x < 0 or x > 1440 (overlays included; catches the
   `cam-delete-key` / `settings-lang-he` class that
   leaves the window entirely).

**Exemptions** (named, closed list — anything else is
a hit):

- The **timeline scroll subtree** (`timeline` key and
  descendants): it scrolls horizontally by design;
  chips/markers may exceed the viewport.
- **Centre column / stage subtree**: flexible width,
  not budgeted.
- **Zero-size elements** (w = 0 or h = 0: collapsed
  canvases, spacers).
- Elements in a **horizontal-scroll container**
  generally: any subtree rooted at an element whose
  builder sets `scroll_x` — at scan time this is the
  timeline only; a new one must be added to the list
  by name with a reason.
- Vertical overflow is **never** flagged: both side
  columns scroll vertically; below-the-fold cards are
  legitimate (Phase N's panel_card lesson).

**Output.** Per scene: hit list `(element key, parent
card, overflow pt)`, sorted worst-first; a run is
CLEAN when every scene reports zero non-exempt hits.
The V wave records the table in QA_RUNE §17. (This
cannot be a gate check — the gate runs on source; the
dumps are CI artifacts. It belongs to the headless
debrief, same status as the pixel read.)

**Proof of the method itself:** run against the
CURRENT dumps it must reproduce §3's table (it does —
the table below is its output, hand-classified);
after O-1/O-3…O-9 land, the same scan must read zero.

---

## 3. O-3…O-9 — Risky rows found (fix items, 1 coder each)

Arithmetic convention: *row width* = last child's right
edge − row's left edge (children + gaps as laid out);
*overflow* = row width − budget. All rows sit in
vertical-scroll columns, so horizontal excess is never
reachable by scrolling — the excess controls are
**unusable**, not merely ugly.

### O-3 — layers_panel: the layer row (LEFT, budget 276)

`layer-row-0` [28..304] carries: dot 10, name 46,
opacity stepper (− 26 / value / + 28.5), `layer-bottom-0`
77.25 (text btn), then `layer-down-0` at x 312.25
(already past the card), `layer-up-0` 346.25,
`layer-top-0` [380.25..438], plus a further unkeyed
child to ≈466. **Overflow ≥ +134** (last keyed child).
Treatment: the figures fix-5 anatomy — two lines.
Line 1 keeps dot/name/opacity stepper; line 2 takes
the reorder cluster (bottom/down/up/top, keys
verbatim). `flex_wrap()` on the row is the named
fallback. Proof: default tree scan + layers snap read.

### O-4 — draw cluster: three rows (RIGHT, budget 316)

| row | width | overflow |
|---|---|---|
| `draw-brushes` (10 brush chips) | **879** | **+563** |
| `draw-tools-row` (7 buttons) | 690.5 | +374.5 |
| `draw-shapes` (`stamp-arrow` last) | ≈448 | +132.5 |
| `draw-stamp-params` (`draw-rot-dn`) | ≈358 | +42 |

Brushes 4–9 end at x 1466…1975 — **past the window**;
six of ten brushes cannot be clicked at 1440 pt.
Treatment: `.flex_wrap()` on `draw-brushes` (10 chips
→ 3 lines ≈ 3×21 + gaps) and on `draw-shapes`;
`draw-tools-row` splits by meaning — mode/tool/cycle
on line 1, the four stroke ops (mirror/simplify/
smooth/clear) on line 2 — or wraps; stamp params wrap.
Proof: draw-open tree scan (zero hits) + snap read.

### O-5 — graph_panel + keyframe_tools rows (RIGHT)

| row | width | overflow |
|---|---|---|
| `graph-actions` (8 buttons) | 621.5 | +305.5 |
| `keyframe-presets` (5 chips) | 361 | +45 |
| `graph-presets` (`graph-preset-linear`) | 329.5 | +13.5 |
| `keyframe-actions` (3 buttons) | 327.5 | +11.5 |
| `keyframe-clipboard` (`key-clear-channel`) | 318.8 | +2.8 |

`graph-clear` ends at x 1717.5 — unreachable.
Treatment: `.flex_wrap()` on all five rows (actions
wraps to ~3 lines; the two ≈+3/+11 rows could instead
shed gap, but wrap is uniform and future-proof).
Proof: graph-duplicates + default tree scans.

### O-6 — scene_panel: weather-kind segments (RIGHT)

`wx-kind`: 6 segments 58.6/64/71/86.6/62.1/71.9 + 5×4
gaps = **434.3, overflow +118.3**; `wx-seg-5` ends at
1530.3 (past the window). Treatment: `.flex_wrap()`
on the segment row (wraps 4+2, segment keys
`wx-seg-{i}` verbatim), or rebuild as two rows of
three — wrap preferred (one call, no new keys).
Proof: default tree scan + scene snap read.

### O-7 — three_d_panel: orbit/camera rows (RIGHT)

From the three-d dumps: `orbit-3d` **+49**, its row's
unkeyed sibling +39, `cam-frame-3d` **+37.5** — the
Joint/RotX/RotY/RotZ/Orbit strip and the camera view
strip (Front/Side/¾/Top/Frame…) both run wide (the
Phase N palette snap already showed "Orbi…" cut).
Treatment: `.flex_wrap()` on both rows. Proof:
three-d + three-d-stage tree scans.

### O-8 — fx detail + playback rows (RIGHT)

`fx-nudge`: label chips + nudge cluster + duration
stepper = **343.5, +27.5** (`fx-dur-up` ends 1439.5 —
at the window edge). Playback panel: `pb-next-key`
+17.6, `pb-loop-end` +36 past their rows. Treatment:
split `fx-nudge` into two lines (nudge cluster /
duration stepper) or wrap; wrap the two playback
rows. Proof: fx-split-mid + loop-range tree scans.

### O-9 — pose_tools + camera_panel rows (RIGHT)

`pose-apply-all` ends 1428.5 (**+16.5**) — the pose
row packs Mirror pose / Mirror figure / Copy / Paste
/ Apply-to-all into 316. `cam-delete-key` ends
**1441.4 — past the window edge** (+29.4 vs content).
Treatment: wrap both rows (pose row wraps Apply to a
second line; camera key row wraps its delete).
Proof: default tree scan (both panels are in the
Animate right column) + pixel read.

---

## 4. Summary for the integrator

- **Items: 9** (O-1 language fix; O-2 audit tooling;
  O-3…O-9 row repairs) — each sized for one coder, no
  item touches another's files except O-2 (no app
  code at all).
- **Uniform medicine:** `flex_wrap()` first (one call
  per row, keys verbatim, Rune measures real widths),
  two-line anatomy where a row has two natural
  clusters (layers, draw-tools, fx-nudge). No label,
  key or handler changes anywhere in this plan —
  census stays 804.
- **Worst risky row:** `draw-brushes` at **+563 pt**
  (row width 879 vs 316 budget; last chip ends at
  x 1975, 535 pt past the window; 6 of 10 brushes
  unclickable at 1440 pt). Runner-up: `draw-tools-row`
  +374.5 and `graph-actions` +305.5 in the same
  unreachable class.
- Every fix is proven the same way: the §2 scan over
  the next headless dispatch reads zero non-exempt
  hits, and the integrator pixel-reads the affected
  snaps (settings-open, draw-open, three-d,
  fx-split-mid, default).
