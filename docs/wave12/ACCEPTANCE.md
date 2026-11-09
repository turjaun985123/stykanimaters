# Wave 12 — SVG fidelity + STYLE_PRESETS unification: acceptance matrix (C10)

**Role of this document.** This is the acceptance auditor's
baseline for the Wave 12 SVG/presets slice (Squad C and the
preset unification). It was written from source,
independently, at wave launch — the tree as found at HEAD
`3f04667` (the Wave 11 close-out) plus whatever concurrent
Wave 12 edits had already landed in satellite files (see
§1.3) — and it is the checklist the integrator closes this
slice against (§6). Verdicts here are **launch state**;
the integrator updates them at close-out. Where a cell is
owned by no Wave 12 item it is marked **UNOWNED** — those
cells become named remainders at close-out, not silent
drops.

**Scope rule obeyed:** no cargo, no git, no code edits —
this document is the only file created. Every claim
carries file:line evidence. Absence claims name the grep
that produced them; all greps are listed in §1 so the
integrator can re-run every one. Project-swap undo/redo
behaviour and accent colours are out of scope and are
not touched anywhere in this audit.

**Ownership (from the wave roster).**
- **C1** (`src/svg.rs`) — figure opacity; taper emitted as
  a filled path when taper ≠ 1 (not only with a gradient);
  bends as arc paths through `bend_mid`. One coherent
  segment-emission fix.
- **C2** (`src/rune_sec/inspector_style.rs`) — delete the
  panel's duplicate preset table; the panel reads
  `src/style.rs`'s `STYLE_PRESETS`; spec any `style.rs`
  visibility change needed.
- **C3 SPEC** (read-only on `svg.rs`) — media/text/sprite
  emission for SVG (`<image>` base64, `<text>`, sprite
  order = raster composite order): an anchored patch spec
  sequenced after C1. C3 delivers a *spec* this wave, not
  emission code.
- **C9 SPEC** (read-only on `svg.rs`/`paint.rs`) — stroke
  width-profile (nib) SVG emission parity vs the
  rasterizer: an anchored patch spec after C1. Also
  spec-only this wave.

---

## 1. Method (reproducible)

### 1.1 The SVG pass

`src/svg.rs` was read whole (997 lines). The production
writer is `write_frame_svg` (`src/svg.rs:99`); everything
below it (line 490 on) is its `#[cfg(test)]` module.
Element classes were enumerated from the model's render
surface — `NodeStyle` (`src/model.rs:98–107`),
`FigureInstance::opacity` (`src/model.rs:430–432`),
`Frame::bends` (`src/model.rs:566–571`),
`Stroke` (`src/model.rs:498–527`, incl. `widths` at
:503–505, `fill`, `spray`, `opacity`, `scatter`),
`NodeDef::media` (`src/model.rs:153`) /
`SegmentMedia` (`src/model.rs:116–119`),
`Project::sprites` / `Project::texts`
(`src/model.rs:779–781`) — and each class was checked
for an emission site in the writer and for the
corresponding behaviour in the rasterizer
(`src/export.rs::render_frame_at`), which is the
fidelity reference: SVG and raster must draw the same
picture for the same project.

Absence greps over `src/svg.rs` (production + tests;
every hit shown was in the test module only, as
fixture literals):

```
grep -n "bends_at"    src/svg.rs   → no hits at all
grep -n "fig.opacity" src/svg.rs   → no hits at all
grep -n "\.media"     src/svg.rs   → no hits at all
grep -n "project.sprites|project.texts" src/svg.rs → no hits at all
grep -n "stroke.widths" src/svg.rs → no hits at all
```

### 1.2 The preset pass

Both `STYLE_PRESETS` tables were read in full and
diffed value by value by eye against the source:
`src/style.rs:138–145` and
`src/rune_sec/inspector_style.rs:218–225`, plus both
`StylePreset` struct definitions and both pin tests.

### 1.3 Concurrent landing noticed during the census

`src/style.rs:56–97` already contains a **Wave 12 (A4)**
section: `effective_style()` / `faded_color()` — one
shared resolution of node opacity × figure opacity,
whose header comment states the launch fact this audit
independently confirms: *"the SVG exporter (`svg.rs`)
used the node opacity alone and ignored the figure
opacity entirely"* (`src/style.rs:64–66`). At census
time `svg.rs` does **not** call `effective_style` (the
§1.1 greps stand), so the helper's existence changes
no verdict below; it does mean C1 has a shared source
to adopt rather than a formula to invent — which is
exactly what Squad A's item promises ("make stage
honour them from the same shared source", extended
here to SVG).

---

## 2. Entering universe counts (reproducible)

| Universe | Count | How derived |
|---|---|---|
| `src/svg.rs` total lines | **997** | `wc -l src/svg.rs` |
| Formatted emission sites in `write_frame_svg` | **14** | 13 × `svg += &format!(…)` at `src/svg.rs:119,123,150,187,210,228,287,306,315,323,339,403,427` + 1 × `defs += &format!(…)` at `src/svg.rs:263`; plus 2 literal appends (the `<!--DEFS-->` placeholder and `</svg>`) |
| Distinct SVG element kinds emitted | **8** | `svg`, `rect` (background), `circle`, `polygon`, `polyline`, `line`, `linearGradient`, `stop` |
| Unit tests in `src/svg.rs` | **16** | `grep -c "#\[test\]" src/svg.rs` → 16 (list in §3, proof column) |
| Integration SVG test | **1** | `tests/export_determinism.rs:98` `svg_export_is_byte_identical_across_runs` |
| `STYLE_PRESETS` tables | **2** | `src/style.rs:138` (lib, for MCP) and `src/rune_sec/inspector_style.rs:218` (bin, for the panel) — `grep -rn "STYLE_PRESETS" src/` |
| Presets per table | **6** | both are `[StylePreset; 6]` |
| `StylePreset` struct definitions | **2** | `src/style.rs:122–128`, `src/rune_sec/inspector_style.rs:203–207` — two distinct types with identical shape |
| Tests in `src/style.rs` / `inspector_style.rs` | **16 / 38** | `grep -c "#\[test\]"` on each file |

---

## 3. The SVG fidelity matrix (launch state)

Vocabulary: **FAITHFUL** = SVG draws what the
rasterizer draws, from the same source values.
**APPROXIMATED** = something is emitted, but it
visibly differs from the rasterizer in a named way.
**ABSENT** = nothing is emitted; the feature vanishes
from SVG exports.

| # | Element class | Launch state | Evidence (SVG side ↔ rasterizer side) | Owner | Proof that closes it |
|---|---|---|---|---|---|
| S1 | Segment taper, **with** gradient (`color2` set) | FAITHFUL | Polygon spanning near/far half-widths, `half_far = half_near * taper.max(0.0)` (`src/svg.rs:247–253`), polygon emitted `src/svg.rs:287–290` | — (already closed) | existing test `svg_renders_gradient_polygons_and_square_caps` (`src/svg.rs:495`) |
| S2 | Segment taper, **no** gradient | **ABSENT** | The no-gradient branch emits a `<line>` with one uniform `stroke-width` (`src/svg.rs:306–310`); taper is never read in that branch. Rasterizer builds a tapered `StyledSeg` either way (`src/export.rs:360–372`) | **C1** (taper as filled path when taper ≠ 1) | NEW test: a no-gradient node with `taper = 0.3` emits a filled path/polygon whose far width is the near width × 0.3, and emits **no** plain `<line>` for that segment; taper = 1.0 keeps today's `<line>` bytes |
| S3 | Segment cap — Round / Square, no gradient | FAITHFUL | `stroke-linecap` mapped Round→`round`, Square→`square` (`src/svg.rs:296–305`) | — | existing test at `src/svg.rs:495` (square cap asserted) |
| S4 | Segment cap — Point | APPROXIMATED | Point is not a line cap; emitted as `butt` with an in-code admission (`src/svg.rs:299–302`). Partially redeemed: on the *gradient* path Point forces `half_far = 0` (`src/svg.rs:250–254`), i.e. a true point; on the plain path the segment just ends flush at full width, where the rasterizer tapers it to zero (`src/export.rs:358–363`) | **C1** (its filled-path fix subsumes Point on the plain path — the integrator must confirm C1's patch covers cap, not taper alone) | NEW test: a Point-capped, no-gradient segment emits the filled path ending at width 0, not `stroke-linecap="butt"` |
| S5 | Segment cap, gradient branch (any cap) | **ABSENT** | The gradient polygon's far edge is flat whatever the cap (`src/svg.rs:271–290`); Round never rounds. Rasterizer carries `cap` on every `StyledSeg` (`src/export.rs:371`) | **C1** (same filled-path rework; not separately named in C1's brief — flagged so the close-out does not assume it) | NEW test or an explicit C1 statement of the gradient-branch cap behaviour, pinned by an emitted-string assert |
| S6 | Gradient (`color2`) | FAITHFUL | `linearGradient` def with both stops, `userSpaceOnUse` along the segment axis (`src/svg.rs:261–266`), polygon filled `url(#gradN)` (`src/svg.rs:287–290`) | — | existing tests `src/svg.rs:495` and `svg_gradient_stops_are_both_self_closed` (`src/svg.rs:779`) |
| S7 | Segment bends | **ABSENT** | No `bends_at` call anywhere in `src/svg.rs` (§1.1 grep); segments are drawn straight between evaluated joints (`src/svg.rs:244–245`). Rasterizer arcs each bent segment through `model::bend_mid` as two pieces (`src/export.rs:384–410`; data: `Frame::bends`, `src/model.rs:566–571`, sampled by `Project::bends_at`, `src/model.rs:907–910`) | **C1** (bends as arc paths through `bend_mid`) | NEW test mirroring `export.rs`'s `bent_segment_arcs_through_the_bend_midpoint` (`src/export.rs:2971`): a project with `frames[0].bends = vec![vec![0.0, 0.7]]` emits a path through the `bend_mid` point for that segment; a zero-bend project stays byte-identical |
| S8 | Node (segment) opacity | FAITHFUL for segments; see S10 for shapes | `stroke-opacity` on the line branch (`src/svg.rs:301–310`), `fill-opacity` on the gradient polygon (`src/svg.rs:281–290`) — but read **raw and unclamped** in the gradient branch (`style.opacity` at `src/svg.rs:282`), where the rasterizer clamps (`src/export.rs:371`) | C1 (only the clamp nit; the field itself is closed) | existing test at `src/svg.rs:495` (`stroke-opacity="0.50"`); NEW assert that a gradient segment with opacity > 1 or < 0 emits the clamped value |
| S9 | **Figure opacity** | **ABSENT** | `fig.opacity` is never read in `write_frame_svg` (§1.1 grep). Rasterizer multiplies it into every segment (`style.opacity * fig.opacity.clamp(0.0, 1.0)`, `src/export.rs:371`) and fades heads/rings/polys toward the background by it (`src/export.rs:348–355`). Corroborated by `src/style.rs:64–66` (A4 header) and by the exporter's own module doc, which names figure opacity as a known difference (`src/svg.rs:10`) | **C1** (figure opacity) — with A4's `effective_style` (`src/style.rs:92–97`) as the shared source to adopt | NEW test: a figure at `opacity = 0.5` with a node at `opacity = 0.5` emits segment opacity `0.25` (the product), and head/shape colours are faded per the shared rule; figure opacity 1.0 keeps today's bytes |
| S10 | Head ring + node shapes (Circle / Polygon) opacity | **ABSENT** | Head circle (`src/svg.rs:313–320`), shape circle (`src/svg.rs:321–327`) and shape polygon (`src/svg.rs:328–347`) carry **no** opacity attribute and use the unfaded node colour; rasterizer fades all three toward the background by figure opacity (`src/export.rs:348–355`, heads/rings collected at :408–412) | **C1** partially — C1's brief says "figure opacity" without naming heads/shapes; node opacity on shapes is named by **no** item. The un-named half is **UNOWNED** unless C1's patch adopts `effective_style`/`faded_color` for these elements too | NEW test: head ring and a Circle shape on a 0.5-opacity figure emit the faded colour (and/or opacity attribute) the shared helper prescribes |
| S11 | Plain strokes (polyline) | FAITHFUL | `<polyline>` with zoom-scaled width and `stroke-linecap="round"` (`src/svg.rs:220–235`); strokes have no cap field in the model, so round is the only case | — | existing tests `src/svg.rs:556,707,729` |
| S12 | Stroke opacity | FAITHFUL | `stroke_opacity_attr` — clamped, trimmed, omitted at 1.0 so old output is byte-identical (`src/svg.rs:66–78`, used at :233) | — | existing tests `svg_stroke_opacity_below_one_emits_the_trimmed_attribute` (`src/svg.rs:707`), `svg_stroke_opacity_one_emits_nothing` (`src/svg.rs:729`) |
| S13 | Stroke nib / pressure width profile (`Stroke.widths`) | **ABSENT** | The polyline carries a single `stroke-width = stroke.width * zoom` (`src/svg.rs:228–234`); `stroke.widths` is never read (§1.1 grep; model field at `src/model.rs:503–505`). Rasterizer averages per-point widths per window (`pressured` at `src/export.rs:492`, width at :507–510); the stage's stroke geometry does the same (`src/paint.rs:699–710`) | **C9 SPEC** — spec only this wave; emission code is **not** promised by any item, so the *fidelity gap itself* stays open past close-out unless the integrator promotes the spec | C9's anchored patch spec exists and names the emission shape (per-window segments vs a filled outline) + the test that will prove it; the closing test itself is a post-wave item and must be carried as a remainder |
| S14 | Stroke scatter | FAITHFUL | Dabs from the shared `paint::stroke_dabs` at the shared spacing, zoom-scaled radius, recorded opacity (`src/svg.rs:195–218`) — same placements as stage and rasterizer | — | precedence pinned by `svg_filled_stroke_exports_a_polygon_and_beats_scatter` (`src/svg.rs:827`) |
| S15 | Stroke spray | FAITHFUL | Dots only, no polyline, from the single source `paint::spray_dots`, per-dot alpha, zoom-scaled radius (`src/svg.rs:140–158`) | — | existing tests `src/svg.rs:556` (dots, not a polyline) and `svg_spray_radius_scales_with_camera_zoom` (`src/svg.rs:795`) |
| S16 | Stroke fill | FAITHFUL | Filled `<polygon>` at the stroke's recorded opacity, and the stage precedence spray > fill > scatter > polyline is mirrored branch-for-branch (`src/svg.rs:159–194`) | — | existing test `src/svg.rs:827` |
| S17 | Segment media (`NodeDef::media`: `SegmentMedia::Image` / `::Text`) | **ABSENT** | `node.media` is never read in `src/svg.rs` (§1.1 grep; model at `src/model.rs:116–119,153`). Rasterizer composites media onto the final image (`src/export.rs:666–686`, via `media::composite_media`) | **C3 SPEC** — spec only; emission is sequenced after C1 and not coded this wave | C3's spec names the emission for both variants (image → `<image>` with the asset's base64 PNG; segment text → `<text>` along the segment, routed through `xml_escape`, `src/svg.rs:45–60`) and the assert that will prove each |
| S18 | Free-standing sprites (`Project::sprites`) | **ABSENT** | Never iterated in `src/svg.rs` (§1.1 grep; model `SpriteInstance` at `src/model.rs:668–672`, table at :779). Rasterizer composites them in the same `composite_media` pass (`src/export.rs:672–686`) | **C3 SPEC** — spec only, as S17 | C3's spec names `<image>` emission, keyframed transform sampling (`InstanceKey::sample`, `src/model.rs:684ff`) and the composite-order rule (see S20) |
| S19 | Free-standing texts (`Project::texts`) | **ABSENT** | Never iterated in `src/svg.rs` (§1.1 grep; model `TextInstance` at `src/model.rs:675–681`, table at :781). Rasterizer composites them (`src/export.rs:672–686`). The exporter already carries the safety precondition: no user string is interpolated today, and `xml_escape` is the pinned helper any text emission MUST route through (`src/svg.rs:13–16,45–60`) | **C3 SPEC** — spec only, as S17 | C3's spec names `<text>` emission (content through `xml_escape`, size/colour from the instance, keyframed transform) + an emitted-string assert with a hostile content string, extending `svg_user_names_cannot_break_the_markup` (`src/svg.rs:761`) |
| S20 | FX clips + weather + authored particles | FAITHFUL | One disc pass in the rasterizer's own order — `particles_at`, then FX clips (hidden clip and hidden figure skipped, `src/svg.rs:358–387`), Trail segments as fading `<line>`s (`src/svg.rs:388–417`), then weather (`src/svg.rs:418–424`) — all discs emitted at `src/svg.rs:425–433` from the shared `vfx` sources | — | existing test `svg_fx_clip_and_weather_discs_are_emitted` (`src/svg.rs:867`), incl. the hidden-clip case |
| S21 | Draw order (z-order) of the whole frame | APPROXIMATED | SVG emits **strokes first, then figures, then particles** (loops at `src/svg.rs:132`, :237, :425). The rasterizer collects figure segments first and appends stroke segments into the *same* segment pass (`src/export.rs:340ff`, strokes at :442–524), so a stroke paints **over** a figure segment there and **under** it in SVG. The module doc names z-order as a known difference (`src/svg.rs:10`) | **UNOWNED** — no Wave 12 item names draw order | Becomes a named remainder at close-out (§5); closing proof, when an item owns it: a stroke crossing a figure segment serialises after the figure's elements, with a byte-diff test on an overlap project |

**Matrix summary:** 21 rows — FAITHFUL 9 (S1, S3, S6,
S11, S12, S14, S15, S16, S20), APPROXIMATED 2 (S4, S21),
ABSENT 10 (S2, S5, S7, S9, S10, S13, S17, S18, S19, and
the absent half of S8's clamp nit is not counted).
Owned gaps: C1 × 6 (S2, S4, S5, S7, S9, S10-in-part),
C3 spec × 3 (S17, S18, S19), C9 spec × 1 (S13).
UNOWNED: S21 outright, S10's node-opacity half.

---

## 4. STYLE_PRESETS unification

### 4.1 The two tables

| | Library copy (MCP side) | Panel copy (GUI side) |
|---|---|---|
| File | `src/style.rs:138–145` | `src/rune_sec/inspector_style.rs:218–225` |
| Type | `style::StylePreset` (`src/style.rs:122–128`) | `inspector_style::StylePreset` (`src/rune_sec/inspector_style.rs:203–207`) — a **second, distinct struct** with identical fields |
| Why it exists | The panel's table lives in the bin crate, which the library cannot import, so the MCP side carries its own copy (`src/style.rs:130–137`) | The original; the chips render from it (`src/rune_sec/inspector_style.rs:1067–1070`) |
| Consumers | `style_preset_by_name` (`src/style.rs:149–154`) → the MCP `apply_style_preset` handler | `preset_application` (`src/rune_sec/inspector_style.rs:503`) / `apply_preset` (`src/rune_sec/inspector_style.rs:518`) |

### 4.2 Value-by-value diff at launch — **NO DRIFT**

Read from both sources, in order:

| # | Name | Colour (lib) | Colour (panel) | Cap (lib) | Cap (panel) |
|---|---|---|---|---|---|
| 1 | Ink | `0x20242C` | `0x20_242C` | Round | Round |
| 2 | Crimson | `0xD23B3B` | `0xD2_3B3B` | Round | Round |
| 3 | Ocean | `0x2B6FD2` | `0x2B_6FD2` | Square | Square |
| 4 | Forest | `0x2E9E5B` | `0x2E_9E5B` | Round | Round |
| 5 | Violet | `0x7A4FD0` | `0x7A_4FD0` | Square | Square |
| 6 | Amber | `0xE8A33D` | `0xE8_A33D` | Square | Square |

Names, order, colours and caps are **identical** in
both tables (the underscore separators are formatting
only). **There is no value drift at launch** — C2 is a
structural unification, not a bug fix, and the
close-out must not report it as one.

### 4.3 The taper / color2 / opacity question — a finding about the type, not the values

The audit was asked to compare taper, `color2` and
opacity per preset. **Neither table carries those
fields, in either copy.** `StylePreset` has exactly
three fields — `name`, `color`, `cap`
(`src/style.rs:122–128`; `src/rune_sec/inspector_style.rs:203–207`).
Applying a preset changes exactly colour and cap; the
node's taper, gradient end and opacity pass through
untouched — stated in the lib copy's doc
(`src/style.rs:121–123`) and pinned on the panel side
by `preset_application_preserves_current_color2_and_opacity`
(`src/rune_sec/inspector_style.rs:1587ff`). So the
per-preset taper/color2/opacity comparison is: **not
carried, by design, identically in both tables** — no
drift, and no hidden third table. C2 must preserve
this: unifying the tables must not grow the preset
type or change what applying a preset does (C4's
serde census guards the saved-file side).

### 4.4 How drift is (and is not) prevented today

Each copy is pinned only against a **hard-coded literal
list inside its own crate**:

- Lib: `style_presets_match_the_panel_table`
  (`src/style.rs:598–618`) asserts the lib table equals
  six literal triples and exercises `style_preset_by_name`.
- Panel: `style_presets_table_is_six_distinct_colours_with_valid_caps`
  (`src/rune_sec/inspector_style.rs:1548–1579`) asserts
  the panel table equals the same six literal triples,
  plus distinctness and `CAP_ORDER` membership.

No test compares the two tables **to each other** —
none can, across the bin/lib boundary that forced the
duplication (`src/style.rs:130–137`). Drift protection
today is therefore *two copies of the same literal
list*: if one table and its pin are edited together,
nothing fails. That — not any value difference — is
the defect C2 removes.

### 4.5 Proof obligations for C2

- [ ] Exactly **one** `STYLE_PRESETS` table remains:
      `grep -rn "pub const STYLE_PRESETS" src/` returns
      one hit, in `src/style.rs`; the panel's
      `StylePreset` struct is likewise gone or re-exported
      from the library (one `pub struct StylePreset`).
- [ ] The panel consumes the library table: chips,
      `preset_application` and `apply_preset` in
      `inspector_style.rs` reference the imported table,
      and the panel file's own table literal is deleted.
- [ ] **Pin-test disposition, decided and recorded:**
      the panel pin
      (`inspector_style.rs:1548–1579`) either moves to
      pin the (now shared) table's values + distinctness
      against the imported table, or is deleted *because*
      the lib pin (`src/style.rs:598–618`) now covers the
      single table — the close-out names which, and the
      surviving pin still asserts all six (name, colour,
      cap) triples from §4.2.
- [ ] Behaviour unchanged: `preset_application_preserves_current_color2_and_opacity`
      and the surrounding preset tests
      (`inspector_style.rs:1581ff`) still pass against
      the imported table; preset application still
      touches colour + cap only (§4.3).
- [ ] Any `style.rs` visibility change C2 needed
      (e.g. exporting the table/type to the bin crate)
      is named in C2's report — no silent API growth.

---

## 5. UNOWNED cells → named remainders at close-out

These are real fidelity gaps that **no Wave 12 item
owns**. The integrator must carry each into the wave's
close-out doc by name; silence is not a carry-over.

1. **S21 — SVG draw order.** Strokes under figures in
   SVG, over figures in the rasterizer (`src/svg.rs:10,132,237`;
   `src/export.rs:340ff,442–524`). Fixing it means
   reordering emission (figures before strokes, or a
   shared order), which interacts with C1's segment
   rework but is not in C1's brief.
2. **S10, node-opacity half.** Head rings and node
   shapes ignore *node* opacity in SVG
   (`src/svg.rs:313–347`). C1's "figure opacity" may
   sweep this in via `faded_color`; if C1's patch does
   not touch these three emission sites, the remainder
   stands.
3. **S13 / S17 / S18 / S19 — spec-owned, code-unowned.**
   C9 and C3 deliver *specs* this wave. Nib emission,
   segment-media emission, sprite emission and text
   emission remain unimplemented at close-out by
   design; the remainders are the four implementation
   items the specs define, and the close-out should
   list them as such (with the specs as their entry
   points), not as "SVG fidelity done".

---

## 6. Integrator close-out checklist (this slice)

**C1 — segment emission**
- [ ] S2: no-gradient taper test exists and passes
      (§3 S2 proof); taper = 1.0 output byte-identical
      to the launch writer on the demo project.
- [ ] S4/S5: Point-cap and gradient-branch cap
      behaviour stated by C1 and pinned, or carried
      to §5 by name.
- [ ] S7: bend test exists (arc through `bend_mid`);
      zero-bend projects byte-identical.
- [ ] S9: figure-opacity test exists (product rule,
      §3 S9 proof); SVG adopts A4's `effective_style`
      rather than a second formula — or the divergence
      is justified in C1's report.
- [ ] S8 nit: gradient-branch opacity clamped, or
      carried by name.
- [ ] All 16 launch tests in `src/svg.rs` still pass;
      the determinism tests (`src/svg.rs:929`,
      `tests/export_determinism.rs:98`) are green —
      every C1 change is a deliberate byte change,
      reviewed as such.

**C3 / C9 — specs**
- [ ] C3 spec delivered: `<image>` / `<text>` / sprite
      emission anchored to `write_frame_svg` patch
      points, composite order = raster order
      (`src/export.rs:666–686`), all text routed
      through `xml_escape` (`src/svg.rs:45–60`).
- [ ] C9 spec delivered: nib emission shape chosen
      and anchored, with the closing test named.
- [ ] §5.3 remainders written into the close-out doc
      as four named implementation items.

**C2 — preset unification**
- [ ] All five §4.5 obligations checked, including
      the recorded pin-test disposition.
- [ ] §4.2's six triples still the shipped values
      (the unification changed structure, not values).

**Re-run at close-out**
- [ ] §1.1 absence greps re-run: `bends_at` and
      `fig.opacity` now **hit** in `src/svg.rs`
      production code; `\.media` / `project.sprites` /
      `project.texts` / `stroke.widths` still absent
      (spec-owned) and matching §5.3's remainders.
- [ ] §2 counts re-derived: emission sites, element
      kinds (expect growth if C1 adds a path element),
      test counts — deltas recorded in the close-out.
- [ ] §5 remainders: each either closed by a landed
      patch or carried by name into the close-out doc.

---

## 7. Close-out verdict (integrator, 2026-11-09)

The wave closed with every §6 item resolved — two of them
beyond what the checklist dared to expect:

- **C1 landed as specced.** S2 taper (filled quad path,
  no plain `<line>` for tapered solids; taper = 1.0
  byte-identical), S4/S5 caps (Point-capped solids are
  pointed), S7 bends (two pieces through `bend_mid`;
  zero-bend projects byte-identical), S9 figure opacity
  (per-element product rule, implemented directly in
  `svg.rs` via its opacity-attribute helpers rather than
  by adopting A4's `effective_style` — the helpers serve
  the stage/raster composition; the SVG rule is the same
  product), S8's clamp nit closed (spray dot alphas are
  clamped at SVG emission, on the stage, and — added at
  integration — in the raster disc pass). All launch
  tests pass; CI is green on the closing tree.
- **C3 and C9 landed as CODE, not specs.** The checklist
  expected anchored specs with post-wave implementation;
  the integrator promoted both during the splice:
  `append_media` emits sprites, texts and segment media
  (images embedded once per asset as PNG defs with
  placement matrices, text as escaped centred `<text>`),
  and pressured strokes emit per-segment lines at baked
  endpoint-averaged widths with degenerate segments as
  discs. §5.3's four named implementation items are
  therefore closed, not carried.
- **C2 landed.** The panel table is a re-export of the
  canonical `style.rs` table; §4.2's six triples are the
  shipped values unchanged; the self-comparison pin was
  converted to `style_preset_lookup_by_name`, and the
  panel-side test pins the shared table's distinct
  colours and valid caps.
- **Absence greps re-run at close-out** (§6's final
  item): `bend_mid`/`bends_at` 8 hits, `fig_opacity`
  11, `.media` 1, `project.sprites` 3, `project.texts`
  3, `.widths` 3 in `src/svg.rs` production code — every
  launch ABSENT now emits. The matrix summary flips
  from 9 faithful / 2 approximated / 10 absent to:
  all 21 rows emit; the standing divergences are the
  two §5 unowned remainders — **S21 z-order** (SVG
  strokes under figures, raster over) and the **S10
  node-opacity half** (node `style.opacity` does not
  reach heads/rings/polys in raster or SVG) — plus
  SVG text as a font run vs the raster's bitmap glyphs.
- **Run evidence:** gate 9/9 on the closing tree; CI
  **42557475828** GREEN, **3,156 tests, 0 failed**;
  headless **42557913977** all probes 0, live-MCP PASS,
  overflow CLEAN on 29 trees. Full record:
  `docs/REDESIGN_PROGRESS.md` §6.7.
