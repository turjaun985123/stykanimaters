# STAGE_STYLE.md — the NodeStyle surface matrix

**Status:** definitive for the tree at Wave 12 launch, HEAD `3f04667`.
Every claim below cites the source `file:line` it rests on; line numbers
are as of that HEAD. A clearly-marked **Wave 12 in flight** section
describes the flips the in-flight squads are scoped to make — those are
*planned*, not verified. The **Remainders** list at the end is the exact
set of divergent cells at launch; the integrator reconciles this doc
against it at close-out.

**Scope note.** `NodeStyle` is a per-*segment* style. In every surface
below it is read only for the stroke of the segment that ends at the
node — never for head rings or shape nodes (Circle/Polygon), which no
surface styles from `NodeStyle` at all (stage: `src/stage_view.rs:1115-1148`;
raster: `src/export.rs:346-352,412-425`; SVG: `src/svg.rs:313-345`).

## The model

`CapKind` — three variants, `Round` the default (`src/model.rs:89-95`):

```rust
pub enum CapKind { Round, Square, Point }   // #[default] Round
```

`NodeStyle` — four fields (`src/model.rs:98-106`), defaults
`taper 1.0, cap Round, color2 None, opacity 1.0`
(`src/model.rs:108-112`):

| Field | Type | Meaning (source comment) |
|---|---|---|
| `taper` | `f32` | "Width multiplier at the far end (1.0 = uniform, 0.2 = sharp taper)" — `src/model.rs:99-100` |
| `cap` | `CapKind` | End-cap rendering for the segment — `src/model.rs:87,101` |
| `color2` | `Option<u32>` | "Gradient end colour; `None` = solid" — `src/model.rs:102-103` |
| `opacity` | `f32` | "0.0..=1.0 opacity multiplier" — `src/model.rs:104-105` |

Stored per node as `NodeDef.style` (`src/model.rs:151`). The model's own
doc comment already names the entering asymmetry: "Per-segment rendering
style (wave 4 prep; **rendered by export/svg**)" (`src/model.rs:97`) —
the stage was not a listed renderer, and at launch still is not for
three of the four fields.

## The matrix (at launch, HEAD `3f04667`)

Verdicts: **Honoured** / **Partially** (the cell says exactly what is
missing) / **Ignored**.

| NodeStyle field | Stage (`src/stage_view.rs`) | Raster export (`src/export.rs`) | SVG export (`src/svg.rs`) | MCP read/write (`src/mcp_tools.rs`, `src/style.rs`, `src/figure_ops.rs`) |
|---|---|---|---|---|
| `taper` | **Ignored** — segments stroked at uniform `node.thickness` | **Honoured** — half-width interpolates near→far; `Point` forces far width 0 | **Partially** — honoured only on the gradient (polygon) path; a solid segment is a uniform-width `<line>` and taper is dropped | **Honoured** — read by `get_node_style`/`get_node`; written by `set_node_taper` (refuses out-of-range) and `cycle_node_taper` |
| `cap` | **Ignored** — every segment drawn `LineCap::Round` | **Honoured** — Square extends both ends by the local half-width; Round/Point fall out of the clamped distance test with the interpolated width | **Partially** — flat path maps Round→`round`, Square→`square`, Point→`butt` (flush, full width — not a point); on the gradient path the cap is dropped entirely (flat polygon ends; only Point's zero far width survives) | **Honoured** — `set_node_cap`, and inside `set_node_style`; read back by `get_node_style`/`get_node` |
| `color2` | **Honoured, approximated** — 6 flat-coloured steps per run instead of the exporters' continuous blend | **Honoured** — per-pixel `lerp_color(color_near, color_far, t)` | **Honoured** — a real `linearGradient` over a tapered polygon | **Honoured** — `set_node_color2` (absent/null clears); read by `get_node_style`/`get_node` |
| `opacity` | **Ignored** — `style.opacity` is never read by the stage's figure pass | **Honoured** — blended per pixel at `style.opacity × fig.opacity` | **Partially** — `style.opacity` emitted alone (`fill-opacity`/`stroke-opacity`); figure opacity is never composed (see §Figure opacity) | **Honoured, clamped** — `set_node_opacity`/`set_node_style` write through the editor op, which clamps to `0.05..=1.0`; reads return the stored value |

## Surface detail

### Stage — `src/stage_view.rs`

The figure pass reads exactly one `NodeStyle` field: `node.style.color2`,
passed to `segment_pieces` at `src/stage_view.rs:1091-1097`. A `grep` for
`style.` over the whole file returns only that call site (and one test at
`src/stage_view.rs:1954`) — `taper`, `cap` and `opacity` have no reader.

- **Taper / cap / opacity — ignored.** The pieces are stroked at the
  node's uniform thickness with a hardcoded round cap:
  `shape.line(..., node.thickness * s, LineCap::Round, color)`
  (`src/stage_view.rs:1100-1106`). The pass's own doc comment says the
  same: "segments in per-node colour at real thickness **with round
  caps**" (`src/stage_view.rs:883-887`). The style panel carries the
  identical audit note — the stage "never reads `style.cap`,
  `style.taper` or `style.opacity`"
  (`src/rune_sec/inspector_style.rs:122-138`).
- **color2 — honoured, approximated.** `segment_pieces`
  (`src/stage_view.rs:640-695`) splits a gradient segment into
  `GRADIENT_STEPS = 6` flat pieces (`src/stage_view.rs:584`), each
  coloured at its midpoint with the same `style::lerp_color` the
  exporters use (`src/stage_view.rs:589-595,602-611`). A bent segment
  splits each arc half again, with the t = 0.5 mid colour shared across
  the joint (`src/stage_view.rs:677-690`), mirroring the rasterizer's
  two-piece split. What is missing versus the exporters: the blend is
  stepped, not continuous, and the gradient's near colour is the
  *faded* figure colour (see §Figure opacity) while `color2` is used
  raw — "the near hex is exactly the colour the flat path draws …
  color2 is used raw, as export.rs uses it"
  (`src/stage_view.rs:1077-1090`).

### Raster export — `src/export.rs`

The reference surface: all four fields honoured, via `StyledSeg`
(`src/export.rs:66-76`) and its per-pixel coverage test
`styled_seg_color` (`src/export.rs:102-135`).

- Construction (`src/export.rs:356-372`): `half_far` is `0.0` when
  `cap == Point`, else `half * style.taper.max(0.0)`
  (`src/export.rs:359-363`); `color_far = style.color2.unwrap_or(color_hex)`
  (`src/export.rs:370`); `opacity = style.opacity * fig.opacity.clamp(0.0, 1.0)`
  (`src/export.rs:371`); `cap = style.cap` (`src/export.rs:372`).
- **Taper** interpolates the half-width along t
  (`src/export.rs:111-112`); **color2** interpolates the colour with
  `style::lerp_color` at the same t (`src/export.rs:129`).
- **Cap:** Square first extends both endpoints by their local
  half-widths along the segment direction (`src/export.rs:113-128`);
  Round and Point use the plain clamped distance test, Point tapering
  to zero width at the far end via `half_far = 0`
  (`src/export.rs:107-112,359-360`).
- **Opacity** is true alpha: covered pixels are blended with
  `style::blend_over(px, rgb(c), seg.opacity)` (`src/export.rs:606`).
- Bends: a bent segment becomes two `StyledSeg`s through
  `model::bend_mid`, mid width/colour at t = 0.5, the first piece's far
  cap forced `Round` so the arc joint stays smooth
  (`src/export.rs:378-410`).

### SVG export — `src/svg.rs`

The segment block is `src/svg.rs:243-311`. The module header already
lists "figure opacity, bends, z-order" as what still differs
(`src/svg.rs:10`).

- **Taper — partially.** `half_far` is computed exactly as the
  rasterizer's (`Point` → 0, else `half_near * style.taper.max(0.0)`,
  `src/svg.rs:249-255`), but it is only *used* on the gradient path,
  where the segment is emitted as a four-point polygon spanning
  `half_near`→`half_far` (`src/svg.rs:256-291`). With `color2 == None`
  the segment is a plain `<line>` at uniform `stroke-width`
  (`src/svg.rs:292-310`) and the taper is silently dropped.
- **Cap — partially.** On the flat path: `Round => "round"`,
  `Square => "square"`, `Point => "butt"` (`src/svg.rs:294-299`), with
  the source's own caveat: "A point is not expressible as a line cap;
  butt ends the stroke flush at the joint" (`src/svg.rs:296-298`) — so
  a Point segment keeps full width to the joint instead of narrowing
  to a tip. On the gradient path no cap is emitted at all: the polygon
  ends are perpendicular flat cuts for every cap kind; only Point's
  zero far width (`half_far = 0`) survives.
- **color2 — honoured.** A `linearGradient` (`userSpaceOnUse`, segment
  axis, near colour → `color2`) fills the tapered polygon
  (`src/svg.rs:263-267,285-290`).
- **Opacity — partially.** `style.opacity` alone is emitted:
  `fill-opacity` on the gradient polygon (`src/svg.rs:282-284`),
  `stroke-opacity` on the flat line (`src/svg.rs:301-303`), each only
  when `< 1.0`. It is never multiplied by figure opacity (see
  §Figure opacity).
- **Bends — ignored (style-adjacent).** The exporter draws the straight
  chord `a→b` (`src/svg.rs:246-248`); `bend_mid` has no call site in
  the file — the only "bend" in `src/svg.rs` is the module doc at
  `src/svg.rs:10`. Taper/gradient/cap therefore apply to the chord,
  not the arc the other two surfaces draw.

### MCP read/write — registry in `src/mcp_tools.rs`, handlers in
`src/mcp_tools.rs`, `src/style.rs`, `src/figure_ops.rs`

No style tool lives in `src/mcp.rs` (a search for `style`/`taper`
there returns nothing); the whole style surface is registry tools.
All four fields are readable and writable at launch:

| Field | Read | Write |
|---|---|---|
| `taper` | `get_node_style` (`src/mcp_tools.rs:4460-4470`, handler `src/style.rs:273-294`); also inside `get_node` (`src/figure_ops.rs:1360-1367`) | `set_node_taper` (`src/mcp_tools.rs:4484-4494`, handler `src/style.rs:316-341` — values outside `0.0..=1.0` are *refused, not clamped*, `src/style.rs:322-324`); `cycle_node_taper` (`src/mcp_tools.rs:4472-4481`, steps 1.0 → 0.6 → 0.3 → 0.0 → 1.0 via `src/editor.rs:2329-2344`) |
| `cap` | same two reads (`"cap"` in both replies) | `set_node_cap` (`src/mcp_tools.rs:3279-3290`, handler `src/mcp_tools.rs:713-736`); also inside `set_node_style` (below) |
| `color2` | same two reads (integer or `null`) | `set_node_color2` (`src/mcp_tools.rs:3292-3302`, handler `src/mcp_tools.rs:738-754` — absent/null clears the gradient); also inside `set_node_style` |
| `opacity` | same two reads | `set_node_opacity` (`src/mcp_tools.rs:3305-3315`, handler `src/mcp_tools.rs:756-775`); also inside `set_node_style` |

Two write-semantics caveats, both deliberate and documented at source:

- **`set_node_style` excludes taper.** The atomic whole-style write
  (`src/mcp_tools.rs:4497-4510`, handler `src/style.rs:352-386`) writes
  colour, `color2`, cap and opacity in one undo step through
  `EditorCore::set_node_style_atomic` (`src/editor.rs:2442-2467`) —
  exactly the style panel's copy/paste field set; "Taper is not part
  of this write; use set_node_taper" (`src/mcp_tools.rs:4498`, and
  `src/style.rs:343-351`). The reply still reports the untouched taper
  (`src/style.rs:378-385`).
- **Opacity writes clamp to `0.05..=1.0`.** Both opacity writers
  delegate to the editor op whose write clamps,
  `node.style.opacity = opacity.clamp(0.05, 1.0)`
  (`src/editor.rs:2425-2433`), "so a segment can never be made fully
  invisible by accident" (`src/editor.rs:2410-2412`). MCP therefore
  cannot store a segment opacity below 0.05, although the model field
  itself admits 0.0 (`src/model.rs:104-105`) and the rasterizer would
  honour it. Handlers read the stored value back so the caller sees
  the clamp (`src/mcp_tools.rs:767-772`, `src/style.rs:350-351`).

`get_node` (`src/figure_ops.rs:1304-1377`, registered
`src/mcp_tools.rs:5279-5289`) embeds `get_node_style`'s own read
(`src/figure_ops.rs:1321-1324`), so the two reads cannot drift.

## The faded-figure-colour rule, and where figure opacity composes

Figure-level opacity (`Figure.opacity`) is **not** applied the same
way by any two surfaces, and on the stage it is not alpha at all.

**Stage — pre-fade toward the background, floor 0.05, overrides exempt.**
The figure colour is blended toward the project background *before*
drawing: `faded = lerp_color(project.background, fig.color,
fig.opacity.clamp(0.05, 1.0))` (`src/stage_view.rs:1064-1068`), with
the source's rationale: "Canvas fade for layer opacity (the rasterizer
uses true alpha for segments; the canvas blends toward the bg — the
GPUI stage's decision)" (`src/stage_view.rs:1061-1063`). Consequences:

- The faded colour is the segment flat colour **and** the gradient's
  near colour (`src/stage_view.rs:1069-1074,1091-1097`).
- A per-node colour override is **not** faded: `node_color` returns the
  override raw, else the faded base (`src/stage_view.rs:1070-1074`).
  Shape nodes and head rings draw through the same `node_color`/`base`
  (`src/stage_view.rs:1122,1126,1140-1148`).
- `color2` is used raw (`src/stage_view.rs:1088-1090`).
- `NodeStyle.opacity` plays no part anywhere in this.

**Raster — true alpha for segments, fade for the rest, floor 0.0.**
Segments keep the raw colour (`color_near = node.color.unwrap_or(fig.color)`,
`src/export.rs:345,369`) and blend per pixel at
`style.opacity × fig.opacity` (`src/export.rs:371,606`) — over whatever
is actually beneath, not just the background. Heads, rings and
polygons "have no alpha plumbing, so they fade toward the background"
with `fig.opacity.clamp(0.0, 1.0)` (`src/export.rs:346-352`) and ignore
`NodeStyle.opacity` (they are not segments).

**SVG — figure opacity composes nowhere.** `fig.opacity` has no reader
in `src/svg.rs` (its only mention is the module doc's difference list,
`src/svg.rs:10`). Segment colour is raw (`src/svg.rs:243`), segment
alpha is `style.opacity` alone (`src/svg.rs:282,301`), and head rings /
shape nodes are emitted with no opacity attribute at all
(`src/svg.rs:313-345`).

Net effect at launch: the same figure at `opacity 0.5` reads as a
background-blended solid on the stage, as true 50 % alpha segments
plus background-faded heads in a raster export, and as fully opaque
in an SVG export.

## CapKind name mapping

One enum, four spellings of its variants across the surfaces:

| Model variant (`src/model.rs:89-95`) | Editor status string | MCP wire (in and out) | Panel label | SVG `stroke-linecap` (flat path only) |
|---|---|---|---|---|
| `Round` | `Segment cap: Round` | `"round"` | `Round` | `"round"` |
| `Square` | `Segment cap: Square` | `"square"` | `Square` | `"square"` |
| `Point` | `Segment cap: Point` | `"point"` | `Point` | `"butt"` — approximation, see matrix |

Evidence:

- **Status string** is the Rust `Debug` name verbatim:
  `format!("Segment cap: {:?}", node.style.cap)` (`src/editor.rs:2377`,
  in `write_node_cap`, `src/editor.rs:2373-2382`).
- **MCP in:** exact lowercase match in both parsers — anything else is
  refused with `unknown cap '{other}' (expected "round", "square" or
  "point")` (`src/mcp_tools.rs:717-726`; `src/style.rs:245-254`). The
  registry schemas pin the same enum (`src/mcp_tools.rs:3285,4505`).
- **MCP out:** `format!("{:?}", cap).to_lowercase()` in the registry
  handler (`src/mcp_tools.rs:733`) and the same construction in
  `sty_cap_name` (`src/style.rs:239-241`).
- **Panel label:** `cap_label` returns the Title-case variant name
  unchanged (`src/rune_sec/inspector_style.rs:243-248`); display/cycle
  order is Round → Square → Point (`CAP_ORDER`,
  `src/rune_sec/inspector_style.rs:201`; `cycle_cap`,
  `src/style.rs:11-17`).
- **SVG:** the mapping at `src/svg.rs:294-299` — the only surface where
  a variant changes *name*, because SVG has no point cap.

Related label: taper has no enum; the panel/status label is
`Taper NN%` from `taper_label` (`src/style.rs:21-23`).

## Wave 12 in flight

> **Planned, not verified.** This section describes the Wave 12 scope
> as launched; nothing here is true of the tree until the integrator
> verifies it at close-out and updates the matrix above.

- **Squad A — stage style fidelity (the deferred headline).** Scoped
  to make the stage honour the three fields it ignores, from the same
  shared construction the exporters use. Expected flips:
  Stage × `taper` Ignored → Honoured; Stage × `cap` Ignored →
  Honoured; Stage × `opacity` Ignored → Honoured. Whether the
  6-step gradient approximation and the faded-near-colour rule are
  replaced by the shared source is A's design call — the matrix cell
  for `color2` and §Figure opacity must be re-read against the landed
  code, not assumed.
- **Squad C — SVG exporter fidelity.** Scoped to the SVG caveats:
  taper without a gradient, cap on the gradient path and Point's
  `butt` approximation, figure opacity composition, bent segments
  drawn straight, and unifying the duplicated `STYLE_PRESETS` tables
  (library copy `src/style.rs:79-86`, panel copy
  `src/rune_sec/inspector_style.rs:219-224` — the library copy's own
  doc names the panel table as the one it was "copied verbatim" from,
  `src/style.rs:65-77`). Expected flips: the four SVG cells marked
  Partially/Ignored above → Honoured.
- **MCP cells:** no flip is in flight — the MCP surface already reads
  and writes all four fields at launch. (Squad B's in-flight item,
  the saved-projects list not refreshing after an MCP save, is a
  project-list cache matter, not a `NodeStyle` cell, and is tracked
  outside this doc.)

## Remainders — every divergent cell at launch

Each item is tagged with the Wave 12 squad item that owns it
(**A** = stage, **C** = SVG). There are no UNOWNED cells at launch:
every divergence below has an owner in flight.

1. **Stage × taper — Ignored.** Uniform thickness regardless of
   `style.taper` (`src/stage_view.rs:1100-1106`). **Owner: A.**
2. **Stage × cap — Ignored.** Hardcoded `LineCap::Round` for every
   segment (`src/stage_view.rs:1103`). **Owner: A.**
3. **Stage × opacity — Ignored.** `style.opacity` has no reader in
   `src/stage_view.rs`. **Owner: A.**
4. **Stage × color2 — Partially.** Stepped 6-piece approximation of
   the exporters' continuous blend (`src/stage_view.rs:584,640-695`).
   **Owner: A.**
5. **Stage figure-opacity composition — divergent.** Background fade
   with a `0.05` floor and unfaded per-node overrides
   (`src/stage_view.rs:1064-1074`), versus the rasterizer's true
   alpha at a `0.0` floor for segments (`src/export.rs:371`).
   **Owner: A.**
6. **SVG × taper (no `color2`) — Ignored.** Solid segments emit a
   uniform-width `<line>` (`src/svg.rs:292-310`). **Owner: C.**
7. **SVG × cap (`color2` present) — Ignored.** Gradient segments emit
   a polygon with flat perpendicular ends for all cap kinds; only
   Point's zero far width survives (`src/svg.rs:256-291`).
   **Owner: C.**
8. **SVG × cap `Point` (no `color2`) — Partially.** Emitted as
   `stroke-linecap="butt"` — flush and full-width, not a point
   (`src/svg.rs:296-299`). **Owner: C.**
9. **SVG × opacity — Partially.** `style.opacity` honoured alone;
   figure opacity never composed, for segments, heads, rings or
   polygons (`src/svg.rs:10,282,301,313-345`). **Owner: C.**
10. **SVG × bends — Ignored (style-adjacent).** Bent segments drawn
    as the straight chord, so taper/gradient/cap apply to the chord
    (`src/svg.rs:10,246-248`). **Owner: C.**
11. **`STYLE_PRESETS` duplicated.** Two tables that must be edited
    together — library `src/style.rs:79-86` and panel
    `src/rune_sec/inspector_style.rs:219-224`, currently held in sync
    only by a pin test (`src/style.rs:65-77`). **Owner: C.**

Explicitly **not** remainders (checked, and consistent or deliberate
at launch): MCP coverage of all four fields (above); the
`set_node_style` taper exclusion and the `0.05..=1.0` opacity write
clamp (deliberate, panel-parity, documented at source); head rings
and shape nodes ignoring `NodeStyle` on all four surfaces alike
(scope note, first section).
