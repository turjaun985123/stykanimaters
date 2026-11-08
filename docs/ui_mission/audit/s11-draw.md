# S11 — Drawing & Painting Workspace Audit (Phase 1)

**Scope:** `src/rune_sec/draw_tools.rs` (1,302 lines),
`src/rune_sec/stroke_inspector.rs` (2,946),
`src/rune_sec/text_tool.rs` (2,299), `src/paint.rs`
(2,153 — library only, **no UI surface**: it is the
brush/shape data the panels render; audited as data).
All three panels stack in the **right column**
(`src/main.rs:647–653`), so every row budget is the
right-column content width: **316 pt, x 1096..1412**;
card edge x 1424; window edge x 1440 (1440×900).

**Evidence:** current source + the measured tree dump
`tree-draw-open.json` from headless run 42488647081
(the same dump set the p05 plan measured). Every
number below is the dump's laid-out child extent
(first child left → last child right), not an
estimate. No cargo, no local build (AGENTS.md).

---

## 1. Overflow verdict — CONFIRMED, and wider than p05

p05's headline is **exactly right**: `draw-brushes`
content extent = **879.0 pt vs the 316 budget
(+563)**. The full Draw cluster in the draw-open
dump:

| row (key) | extent | over | state at rest (1440 pt) |
|---|---|---|---|
| `draw-brushes` | **879.0** | **+563.0** | brush-4 straddles the window edge (1403..1466); **brush-5…brush-9 lie entirely past x 1440** |
| `draw-tools-row` | **690.5** | **+374.5** | mirror-strokes straddles (1370..1470); **simplify, smooth, clear-drawing entirely past the window** (clear ends x 1786) |
| `draw-shapes` | **448.5** | **+132.5** | stamp-star crosses the card edge (1417..1481); **stamp-arrow entirely past the window** (1489..1544) |
| `draw-stamp-params` | **436.5** | **+120.5** | draw-rot-dn past the card edge; **the rotation value and draw-rot-up entirely past the window** — stamp rotation cannot be changed |
| `stroke-bulk-frame-row` *(new, not in p05)* | **331.9** | **+15.9** | stroke-delete-frame ends x 1427.9 — past the content edge, nibbling the card border zone |

Two corrections/refinements to p05's wording:

1. **"Unclickable" needs one qualification for the
   brushes row only.** `draw-brushes` is the one row
   that carries a mitigation: the builder wraps it in
   `.scroll_x().w_full()` (draw_tools.rs:772). The
   other three overflowing rows have **no** wrap and
   **no** scroll — their excess buttons (Simplify,
   Smooth, Clear drawing, Arrow stamp, rotation +)
   are painted past the window with no path to them
   at all. For the brushes, a horizontal scroll path
   exists in principle, but: the strip is 21 pt tall,
   carries no visible scroll affordance in the
   draw-open snap, and 6 of the 10 presets — the
   panel's primary control — sit off-canvas at rest.
   That is a P1 UX defect whether or not a determined
   user can scroll it. The repair (wrap) is the same.
2. **p05 underestimated stamp-params** (≈358 → the
   dump says 436.5) and **missed
   stroke-bulk-frame-row** (+15.9).

Everything else in the two panels fits, some of it
well: all four stroke-inspector stepper rows land
exactly on 316 (label grows, steppers right-align);
`brush-nib-row` is 235; every text-tool row fits
(`text-actions` 238.5, `text-color-row` 220,
nudge pad 148) — **the text tool is the healthy
reference implementation in this cluster**.

---

## 2. The repair design (per row, with arithmetic)

Uniform medicine, per the Phase O charter: wrap
first; two-line anatomy where a row has natural
clusters. Chip/button widths below are the dump's.

### draw-brushes → `.flex_wrap()`, drop `scroll_x()`
Chip widths 50/63/69/93/63/95/103/107.5/100.5/63,
gap 8. Wrap packs: L1 Pen+Pencil+Marker+Calligraphy
= 299 ✓ · L2 Brush+Paint Brush+Calligraphy L = 277 ✓
· L3 Chisel+Broad+Spray = 287 ✓. **3 lines**, height
21 → ≈79 pt (+58 of vertical scroll — the column
scrolls; acceptable and stated). A scroll container
does not wrap against its own content width, so
`scroll_x()` must go; keep `.w_full()`.

### draw-tools-row → `.flex_wrap()`
Widths: Draw 71.1, Eraser 77.6, Brush-cycle 101.5
(widest label, "Brush: Calligraphy L" — the label is
dynamic, which is itself an argument for wrap over
hand-split lines), Mirror 100.5, Simplify 87.1,
Smooth 86.8, Clear 117.9, gap 8. Wrap packs:
L1 Draw+Eraser+Cycle = 266.2 ✓ · L2 Mirror+Simplify+
Smooth = 290.4 ✓ · L3 Clear alone = 117.9 ✓.
3 lines (+66 pt). Bonus: the destructive Clear
lands alone on its own line — better separation
than it has today, for free.

### draw-shapes → `.flex_wrap()`
Widths 63.5/63.4/76.5/86/64.1/55, gap 8. L1 Line+
Rect+Ellipse+Polygon = 313.4 ✓ · L2 Star+Arrow =
127.1 ✓. 2 lines (+33 pt).

### draw-stamp-params → explicit two rows (NOT wrap)
A wrap could split a stepper group (label / − /
value / +) across lines. The three groups measure
≈147 pt each (Sides ≈147, Size ≈147.5, Rot ≈147).
Row 1 = Sides + Size = 302.5 ✓ · Row 2 = Rot = 147 ✓.
`draw-stamp-params` key stays on row 1; row 2 takes
a new key `draw-stamp-params-2` (addition, not a
loss — census grows, nothing is renamed).

### stroke-bulk-frame-row → one listed copy change
The row is label ("Frame N", growing) + Frame colour
94.5 + Delete frame strokes 136.9. Shortening the
long button to **"Delete frame"** saves ≈35 pt →
extent ≈297 ✓, and matches its sibling's naming
pattern ("Recolour all" / "Delete all" / "Frame
colour" are all short). This is the audit's only
proposed label change in the cluster; it must be
listed in the implementing PR exactly as here.
Fallback if the copy change is refused: `flex_wrap()`
(delete wraps to line 2) — workable, uglier.

---

## 3. Coder items (one coder each)

**D-1 — Brushes wrap.** `draw_tools.rs` brushes
builder (:765–772): add `.flex_wrap()`, remove
`.scroll_x()`. Keys `brush-0…9` verbatim. Proof:
overflow scan on draw-open = zero; pixel read shows
all 10 presets visible at rest.

**D-2 — Tools row wrap.** `draw_tools.rs` `tools`
row (:695): add `.flex_wrap()`. All 7 keys verbatim.
Proof: scan zero; Clear drawing clickable at rest.

**D-3 — Shapes wrap + stamp-params split.**
`draw_tools.rs` `shapes` row (:790): `.flex_wrap()`;
`stamp_params` (:717–749): split into two rows per
§2 (new key `draw-stamp-params-2` on the second).
All stepper keys verbatim. Proof: scan zero;
draw-rot-up inside 1412.

**D-4 — Bulk-frame row fit.** `stroke_inspector.rs`
bulk frame row: label "Delete frame strokes" →
"Delete frame" (the §2 copy change), OR wrap if
refused. Keys verbatim. Proof: scan zero on
draw-open.

**D-5 — Show the drawing colour.** There is **no
colour picker anywhere in the Draw workspace**, and
nothing shows the current colour: new strokes
silently take the selected figure's colour
(draw_tools.rs:323; the core's `begin_draw` does the
same), and the inspector's two Recolour buttons
apply `bulk_target_color` (selected node's colour
over the figure's) **without showing which colour
that is** — the bulk rows deliberately carry no
swatch (stroke_inspector.rs module docs :322–325).
A user cannot answer "what colour will I draw /
recolour with?" without leaving the workspace.
Fix, copying the text tool's proven pattern
(`text-color-custom`, text_tool.rs:1031 — a
non-pressable 24 pt readout chip): (a) a colour
readout chip beside the Draw panel title's brush
meta; (b) one readout chip in each bulk row showing
`bulk_target_color`. Display only — choosing a
colour stays with the figure/node inspectors (a
real picker is an engine-facing feature, out of
this mission's UI scope; say so in the PR). Two
small items: **D-5a** draw_tools.rs, **D-5b**
stroke_inspector.rs. New keys are additions
(`draw-color-readout`, `stroke-recolor-swatch-all`,
`stroke-recolor-swatch-frame`).

**D-6 — Destructive presses have no confirmation
and no visible trace beyond the status line.**
Clear drawing (whole frame), Delete all (whole
project), Delete frame strokes: one press, no
confirm, anywhere (`grep confirm` in both files:
none). All three do take an undo snapshot, so they
are recoverable — but the user is never told that.
Minimum honest fix within existing machinery: route
these three presses' status strings through the
toast path with its Undo affordance (the shell's
existing toast/undo surface — coordinate with
Squad 17/20; if that surface cannot carry Undo for
these ops, escalate to a confirm popover rather
than shipping nothing). Files: draw_tools.rs,
stroke_inspector.rs, shell toast wiring. Acceptance:
no project-wide destructive press completes without
either a confirmation or a visibly offered Undo.

**D-7 — Bare empty lines → kit empty states.**
stroke_inspector's "No strokes on this frame."
(stroke list, Phase O charter seed) and text_tool's
"No free-standing texts." (text_tool.rs:1059) +
"No text segments on this figure." (:1197) are bare
LABEL text, not `design::empty_state`. Convert all
three (glyphs already curated: verify against
icon_data.rs at implementation; "shapes"/"font"
class names). 1 coder, both files.

**D-8 — Text edit controls look live when nothing
is loaded.** The nudge pad, rotate row, colour row
and instance-size steppers render identically
whether or not an instance is loaded; with nothing
loaded every press just writes a refusal to the
status line (text_tool.rs:1086–1188, "Built either
way"). Render them in the kit's muted/inert state
until `loaded_row.is_some()` (handlers unchanged —
this is presentation state only). 1 coder,
text_tool.rs. The conditional Update/z-order rows
already do the right thing — follow that pattern.

**D-9 — Eraser size is undiscoverable.**
The eraser's radius is twice the active brush's
width (editor.rs, per draw_tools module docs), but
the panel's width readout always says "Width: N px"
for the *brush*. When the eraser tool is active,
make the readout say so — e.g. "Eraser: 8 px
(2 × brush width)" — copy-only, in
`width_readout`'s call site (draw_tools.rs:780).
1 small item; pairs naturally with D-5a's coder.

---

## 4. Non-overflow defects (recorded, not all itemised)

- **Brush chips carry no distinguishing preview.**
  10 chips are icon+name only; the icon mapping
  collapses five presets to "pen" and four to
  "paintbrush" (draw_tools.rs `brush_icon`), and no
  chip shows its width/nib — the two facts that
  actually differentiate presets live in two small
  readout lines *below* the row. A width figure in
  the chip meta would fix the identification
  problem, but `chip_btn` has no meta slot — that is
  a kit change: **referred to Squad 02/17**, not a
  D-item.
- **Inspector "Brush size / opacity / scatter /
  smoothing" steppers do not set values** — they
  step the *preset selection* through the library
  to the nearest preset carrying a stepped value
  (stroke_inspector.rs `step_param`; honestly
  documented in the module docs). The labels promise
  continuous control the control doesn't have.
  Honest options are a rename ("Preset size" etc.)
  or real per-brush parameter storage (engine
  change — out of scope). **Referred to the
  integrator as a product decision**; not silently
  "fixed" here. Related standing fact: non-spray
  scatter remains model-only in the engine, so the
  Scatter stepper's promise is doubly indirect.
- **Grouping in the Draw panel mixes tools and
  actions.** One row holds two toggles (Draw mode,
  Eraser), a cycle button, and four frame-wide
  actions (Mirror — an action, not the
  mirror-while-drawing toggle its position implies;
  Simplify; Smooth; Clear). D-2's wrap separates
  Clear; a fuller regroup (tools line / actions
  line with a section label) is a design call for
  Squad 02 to ratify — the wrap arithmetic in §2
  already produces that grouping if the rows are
  split explicitly instead (L1 tools, L2 actions,
  L3 clear = the same packing).
- **Text tool has two "Size" controls with
  different targets** (new-text size in
  `text-actions` vs loaded-instance size in
  `text-instance-size`), 200 pt apart in the same
  card, distinguished only by the loaded label.
  D-8's inert state mitigates; a label prefix
  ("New text size" / the instance section already
  carries the loaded preview as its label) is a
  copy decision for the integrator.
- **paint.rs scatter caveat for the squad record:**
  `Brush.scatter` is consumed only via spray marks;
  the field doc says so plainly. No UI should imply
  otherwise (see the stepper item above).

## 5. What is good — preserve

- Text tool: every row fits, kit swatches with a
  custom-colour readout chip, conditional edit rows,
  kit list rows with real selection state. The model
  the rest of the cluster should converge to.
- Stroke inspector stepper anatomy (grow label,
  − / mono value / +) lands exactly on budget four
  times over; stroke rows carry a colour swatch
  readout, summary and per-row Delete.
- Draw panel's readouts are honest engineering:
  width/smoothing are readouts *because* the preset
  library makes them non-writable — documented in
  the module docs rather than faked with dead
  steppers. That honesty is the mission's own rule;
  keep it.

## 6. Summary for the integrator

- **Overflow verdict: CONFIRMED.** 5 rows, worst
  `draw-brushes` +563 (879 vs 316); three rows have
  controls entirely past the window with no scroll
  or wrap path at all (Clear drawing, Arrow stamp,
  stamp-rotation +). One qualification: brushes
  alone carry a `scroll_x` mitigation — hidden, not
  strictly unreachable.
- **Items: D-1…D-9** (D-5 split a/b → 10 coder
  slots), each one file-cluster, keys verbatim
  except the single listed copy change in D-4 and
  additive keys in D-3/D-5. Proof for D-1…D-4 is
  mechanical: the overflow scan over the next
  draw-open dump reads zero, plus a pixel read.
- **Referred out:** chip meta slot (Squad 02/17),
  preset-stepper semantics + scatter honesty
  (product decision), destructive-confirm wiring
  coordination (Squad 17/20, in D-6).
- Top defects in order: (1) the overflow family —
  primary controls unreachable; (2) no visible
  drawing colour anywhere in the workspace (D-5);
  (3) unconfirmed project-wide destructive presses
  (D-6); (4) live-looking dead text controls (D-8);
  (5) bare empty states (D-7).
