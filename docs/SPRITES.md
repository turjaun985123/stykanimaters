# SPRITES.md — the free-standing sprite surface

**Status:** written at Wave 13 launch (A10), against HEAD
`374e873` plus the Wave 13 working-tree edits present when the
doc was finalised. Squad A landed *while this doc was being
written*: the first pass found only A8 in the tree; a final
re-check found A1, A4, A6, A7 and A9 also present (all
uncommitted at that point), and §2, §4, §5.1, §7 and §8 were
updated to match. A2, A3 and A5 were still absent at the final
check. Every claim cites the source it rests on. Surfaces are
tagged:

- **[before W13]** — shipped before Wave 13; verified in source.
- **[W13, in tree]** — a Wave 13 addition found in source at
  the final check. "In tree" means the code exists in the
  working tree; it does **not** mean compiled, gated or
  CI-verified — that is the integrator's close-out.
- **[W13, contracted — integrator to confirm]** — named by the
  Wave 13 roster (`~/workspace/wave13-roster.md`, squad A) but
  **not found in source** at the final check. These are
  contracts for the integrator's splice, not facts about the
  tree. Do not cite them as shipped.

**Terminology.** This document is about *free-standing sprite
instances*: `Project.sprites`, images placed on the stage in
their own right. It is **not** about *segment media* — an image
attached to one figure segment (`SegmentMedia::Image` on a
`NodeDef`) — except where the two share machinery, in which case
the distinction is stated. The export panel and several code
comments use "sprite" for both; the model does not.

---

## 1. The model [before W13]

`src/model.rs`:

```rust
pub struct InstanceKey {            // model.rs:658
    pub frame: f32,
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub angle: f32,                 // degrees
}

pub struct SpriteInstance {         // model.rs:670
    pub asset: usize,               // index into Project.assets
    pub keys: Vec<InstanceKey>,
}
```

- A sprite is **only** an asset index plus a key track. There is
  no name, no opacity, no visibility flag, no z-order field and
  no per-instance size override on the instance itself
  (`src/model.rs:670-673`). The drawn size comes from the asset's
  decoded pixel dimensions times the key's `scale` (see §3).
- `Project.sprites: Vec<SpriteInstance>` carries
  `#[serde(default)]` (`src/model.rs:781`), so project files
  written before the field existed load with an empty list. The
  sibling list `Project.texts: Vec<TextInstance>` shares the
  same `InstanceKey` type (`src/model.rs:676-682, 782`).
- Assets are `Project.assets: Vec<ImageAsset>` — named PNG byte
  strings (`ImageAsset { name, png }`). An instance's `asset`
  index is meaningful only against that table; a dangling index
  is not an error anywhere — renderers skip it (§3, §5).
- Project JSON is the full-fidelity home: save/load serialises
  the whole `Project` with `serde_json::to_string_pretty`
  (`src/projects.rs:386, 364`), sprites and assets included.
  The MCP `import_file` description says the same of `.json`:
  "asset table, sprites and texts included, exactly as
  load_project" (`src/mcp_io.rs:941`).

### 1.1 Sampling: `InstanceKey::sample` [before W13]

`src/model.rs:686-716` (`impl InstanceKey`):

- Empty key list → `None` (the instance draws nothing).
- `frame_f` at or before the first key → the first key, verbatim.
- Between two keys → linear interpolation of `x`, `y`, `scale`
  and `angle` (plain lerp; angles are **not** wrapped, so a track
  authored 350° → 10° sweeps backwards through 180°).
- At or after the last key → the last key, verbatim.
- The walk assumes keys are in ascending frame order. Nothing in
  `sample` sorts them. The two writers that reorder keys keep
  them sorted: frame swap re-sorts instance keys after remapping
  (`src/frame_swap.rs:181-196`), and the Stick Nodes importer
  appends keys in frame order (§5, extra row). A hand-edited project file
  with unsorted keys samples unpredictably — no surface
  validates the order. (Wave 13 C6 is contracted to pin the
  sample edge cases in tests: roster squad C — integrator to
  confirm.)

### 1.2 What a key cannot express

Because the key is `{frame, x, y, scale, angle}` and nothing
else:

- **No opacity channel.** Sprite instances composite at alpha
  1.0, always (`src/media.rs`, `MediaItem::Image` for instances,
  §3). Segment media, by contrast, carries figure × style
  opacity. There is no way to fade a free-standing sprite.
- **Uniform scale only.** One `scale` multiplies both axes.
  The Stick Nodes importer collapses its source's non-uniform
  scale to `scale_x.abs()` (§5, extra row), so non-uniform scale is lost
  at import, not merely unrenderable.
- **No mirroring.** A negative scale is not a flip: the
  placement pass skips any sampled key with `scale <= 0.0`
  (`src/media.rs`, sprite loop in `media_items`).

---

## 2. Where sprites come from

**Entering Wave 13 [before W13]** there was exactly **one**
producer of sprite instances in the whole codebase, plus
hand-written project JSON:

1. **Stick Nodes animation import** (`.stknds` projects and
   `.nodemc` movieclips) — `src/nodes_anim.rs` (§5, extra row).
2. **Project JSON** — a `.json` / `.styk.json` file that already
   carries a `sprites` array loads verbatim (§1).

That is the entering truth the Wave 13 roster states, and
source confirms it: a census of `project.sprites` writes outside
tests found the importer (`src/nodes_anim.rs:2573, 2685`), the
asset-removal remapper (`src/media.rs:423`), frame swap
(`src/frame_swap.rs:190`) and the Wave 12 placeholder stage pass
reading — no editor op, no panel and no MCP tool created,
moved or deleted an instance.

**Wave 13 [W13, in tree]** adds the missing producer: the
engine ops in `src/sprite_ops.rs` (A1, §7) — `add_sprite`,
`remove_sprite`, `set_sprite_key`, `delete_sprite_key` as
`EditorCore` methods — and the MCP handlers over them (A4,
§7). What is still missing at the final check is the *shell*
half: the Sprites panel (A3) and the sprite stage prims (A2)
that would call the ops from the GUI. Until those land and
the registry splice wires A4's handlers, sprites are
creatable from code and (once spliced) from MCP — not yet
from the panel.

---

## 3. Placement: `media::media_items` [before W13]

`src/media.rs:125-230` is the single placement pass every
rendering surface shares. For a fractional frame `frame_f` it
returns `Vec<MediaItem>` in a fixed order:

1. Segment media, in figure order, then node order.
2. **Sprite instances**, in `Project.sprites` order.
3. Text instances, in `Project.texts` order.

Instances therefore draw **over** segment media and **under**
free-standing texts, on every surface that consumes the list
in order (§5).

For each sprite instance (`src/media.rs:171-199`):

- Sample the track at `frame_f` (§1.1); no sample → skip.
- `scale <= 0.0` → skip.
- Decode the asset's PNG dimensions via `png_dims`; a missing
  asset index or undecodable bytes → skip, silently.
- The quad is the image's **pixel size × key scale**, centred
  on `(key.x, key.y)`, rotated about the centre by `key.angle`
  (y-down rotation basis `u = (cos, sin)`, `v = (−sin, cos)`).
  Unlike segment media, a sprite's size does **not** depend on
  any segment length — only on the PNG and the key.
- The item is `MediaItem::Image { corners, asset, alpha: 1.0 }`.

`MediaItem` itself is defined at `src/media.rs:56-77`; its doc
records the alpha rule: segment media carry the figure × style
product (Wave 12, A6), "free-standing sprite instances carry
1.0 (an instance key has no opacity channel)".

### 3.1 Compositing (raster) [before W13]

`composite_media` (`src/media.rs:232` onwards) walks
`media_items` in order and, per image item, calls
`composite_image`: nearest-neighbour sampling through the
quad's affine map, alpha-blended over the destination at the
item's alpha (so sprites blend at full strength). PNGs are
decoded **on every call — no cache** (`src/media.rs:40-41`:
"fine for export-time compositing, not a per-frame editor hot
path").

`src/export.rs:744-766` gates the pass: `has_media` is true
when the project has sprites, texts, or any segment media
(the sprite/text half of that gate is itself a Wave 11 fix —
before it, a project whose only media was a free-standing
instance never composited at all). The gate converts the
rendered frame to RGBA, composites, and converts back.

---

## 4. The asset relationship [before W13]

Sprites are one of the three places an asset index can live.
The census lives in `src/media.rs` (the asset-table MCP
handlers' module doc, and `asset_references` at
`src/media.rs:371-389`):

| Reference kind | Where | Counted |
|---|---|---|
| Segment media on figure defs | `figures[].def.nodes[].media` | yes |
| Segment media on movieclip defs | `clips[].def.nodes[].media` | yes — stored and serialised, but **never rendered** (Wave 12 B8 finding; Wave 13 E5 audits it) |
| Sprite instances | `sprites[].asset` | yes |

Consequences, all shipped with the Wave 12 asset tools:

- `asset_remove` **refuses** while any of the three kinds
  references the target (`src/media.rs:579-583` region: the
  refusal counts segments + clip segments + sprites).
- When a removal does happen, references **above** the removed
  index are remapped down by one in the same undo step —
  sprite instances included (`remap_asset_refs`,
  `src/media.rs:398-440`).
- `assets_list` / `asset_info` report the per-asset reference
  counts, sprites included (`asset_json`,
  `src/media.rs:442-457`).
- `project_summary` reports `sprites: p.sprites.len()` among
  its counts (`src/mcp_tools.rs:427`).

The reciprocal read arrived with Wave 13: A4's
`tool_sprite_list` handler (`src/media.rs:725`) lists the
instances themselves over the A1 ops **[W13, in tree]** —
but the registry `Tool` entries are the integrator's splice,
so until that lands the handlers exist without being
reachable from a client. Before Wave 13 a client could learn
*that* an asset was referenced by 2 sprites, never *which*
instances, where they sit, or what their keys are — short of
loading the project JSON.

---

## 5. Per-format parity matrix

Every cell traces to a file read for this document. "Sprite"
means a free-standing `SpriteInstance` throughout; segment
media is noted only where a format treats the two differently.

| Aspect | Raster (PNG/GIF/MP4/WebM/APNG/JPEG/spritesheet/contact sheet) | SVG | Stage (Rune shell) | .piv (v2/v3/v4/v5) | .stk (Pivot figure) | .nodes (Stick Nodes figure) |
|---|---|---|---|---|---|---|
| **Renders sprites** | **Yes, real pixels.** `export.rs:750-766` gate → `media::composite_media` → `composite_image`, nearest-neighbour, alpha 1.0 | **Yes, real image.** `svg.rs:629` `append_media`: PNG embedded once per asset as a defs `<image>` (base64), each item a `<use>` with a placement matrix | **Placeholder only.** `stage_view.rs:1123-1146`: the item's exact quad, accent fill at 0.16 × alpha + outline at 0.7 × alpha; no bitmap (Rune `Shape` has no bitmap primitive) | **No.** No `.piv` writer reads `project.sprites` (`export_panel.rs:364-390` documents the drop; `pivot.rs` exporters take figures only) | **No.** `pivot.rs:1808` `export_stk(def, name)` exports one figure definition; the format has no project or instance concept | **No.** `nodes.rs:816-824`: the figure grammar is purely geometric, "media is dropped"; a figure file has no instance concept |
| **Draw order** | Media composites **over everything** in the frame (after figures, strokes, FX, weather): `export.rs:744` | Media **last**, after the particle discs: `svg.rs:567-573`. CAVEAT at launch: strokes emit *under* figures in SVG vs *over* in raster (Wave 13 C1 is contracted to fix that half; media stays last either way) | Placeholder sits **over the figures, under the 3D session figure and the FX/text/weather passes** (`stage_view.rs:1129-1134`) — a documented divergence from the raster | — | — | — |
| **Alpha / opacity** | 1.0, fixed — no opacity channel exists (§1.2) | No `opacity` attribute emitted for instances (alpha is 1.0, and `append_media` only writes the attribute below 1.0: `svg.rs:660-664`) | Fill/outline alphas are the placeholder's own (0.16 / 0.7 × item alpha = × 1.0) | — | — | — |
| **Text rendering note** (texts share the pass) | 3×5 bitmap glyph stamps (`media.rs:22-27`) | Real `<text>`, monospace; centre agrees, advance width cannot (`svg.rs:594-627`) | Glyph boxes via `stage_text::text_geometry` (`stage_text.rs:284`) | — | — | — |
| **Import creates sprites** | n/a | n/a | n/a | **No.** `.piv` import builds projects with `sprites: Vec::new()` (`pivot.rs:972-974, 1750-1752`); legacy image segments' bitmap bytes are skipped, "sprites are not imported as geometry" (`pivot.rs:19`) | **No.** `.stk` import adds one figure (`mcp_io.rs:941`); nothing else | **No** for the figure file. **Yes** for the animation containers — see the extra row below |
| **Export writes sprites back** | n/a (rendered, not stored) | n/a (rendered, not stored) | n/a | **No** — dropped silently by the writers; the Export panel warns first (§5.1) | **No** | **No** |
| **Project JSON (.styk.json)** | — the one full-fidelity format: sprites, keys and asset bytes round-trip via serde (`projects.rs:364, 386`; model round-trip tests `model.rs:2135-2200`) | | | | | |

**Extra row — `.stknds` / `.nodemc` animation containers**
(`src/nodes_anim.rs`, the files the roster's ".nodes" brief
item A5 actually audits):

| Direction | Verdict |
|---|---|
| Import | **Creates sprites.** Sprite-library images are extracted to PNG assets (`sprite_assets`, `nodes_anim.rs:1947`); main/`.nodemc` timeline sprite instances (types 2/3) become `SpriteInstance` key tracks (`build_instance_tracks`, `nodes_anim.rs:2045`); the project is assembled with `project.sprites = sprites_out` (`nodes_anim.rs:2573, 2685`; entry `import_nodes_animation`, `nodes_anim.rs:2766`) |
| Export | **Carries them [W13, A5 landed].** `export_nodes_animation` now writes the sprite library and per-frame type-2 sprite instances from the project model (`sprite_export_plan` / `write_sprite_instance` in `nodes_anim.rs`), and the round trip is pinned by tests. What still does not survive: group instances come back as plain member images (§5.2.3), keys at fractional frames or beyond the timeline are omitted, and texts are still written as none. |

### 5.1 The warnings that exist

- **Export panel, `.piv` [before W13]:** `piv_media_note` (key
  `xc-piv-media-note`, `src/rune_sec/export_panel.rs:355-390`)
  shows, when a project carries instances: *"Media note: no
  .piv version writes sprite or text instances — they are left
  out of .piv exports."* (plus a segment-media clause when both
  kinds are present).
- **Export panel, sprites [W13, in tree]:** A9's `sprite_note`
  (key `xc-sprite-note`, `src/rune_sec/export_panel.rs:417-451`)
  shows, when a project carries sprites: *"Sprite note: .stk
  and .nodes carry only the selected figure, so project sprite
  instances are left out of those two exports. .stknds carries
  the sprite tracks (on re-import, groups come back as plain
  images and text instances are not carried)."* (The note's
  `.stknds` clause was rewritten at integration: A9 wrote it
  against the pre-A5 exporter, which wrote the sprite library
  empty; A5's landed export carries the tracks.)
- **Scene panel:** the placed-media readout counts sprites and
  assets (`media_readout`, `src/rune_sec/scene_panel.rs:578-594`).
  Entering Wave 13, `media_note` said *"Sprites render in
  exports only — they can't be moved or edited in the app
  yet."*; A7 replaced it **[W13, in tree]** with: *"Sprites are
  edited in the Sprites panel — move, rotate, scale and
  keyframe them there."* (`src/rune_sec/scene_panel.rs:604-612`).
  The panel it names (A3) has since landed and is mounted
  in the Animate workspace, so the pointer is now simply
  true.
- **MCP `export_file` description** states the general rule:
  "The .piv/.stk/.nodes/.stknds formats are data interchange:
  they carry only what those formats hold, not the asset table,
  sprites, texts or audio." (`src/mcp_io.rs:953`.)

### 5.2 Known divergences and losses (all [before W13])

1. **Stage ≠ raster.** Same geometry (the placeholder quad *is*
   the composite quad), different pixels and different draw
   order (§5). A user cannot judge a sprite's real appearance
   from the stage.
2. **`.stknds` round trip losses, post-A5** (§5, extra
   row) — sprite tracks now round-trip; what is still lost:
   group structure (instances return as plain member
   images), text instances, and sprite keys at fractional
   frames or beyond the exported timeline.
3. **Group sprites collapse.** A type-3 (group) instance
   resolves to a single library image — the member selected by
   `group_frame`, else the first member (`resolve_sprite_image`,
   `src/nodes_anim.rs:1973-1993`). The group structure does not
   survive into the model.
4. **Non-uniform scale collapses** to `scale_x.abs()` at import
   (`build_instance_tracks`, `src/nodes_anim.rs:2045` region);
   a negative source scale becomes positive (no mirroring).
5. **Image-change mid-track keeps the first image.** If a
   source instance's image changes after its first appearance,
   later keys are dropped rather than splitting the track
   (`build_instance_tracks` docs, `src/nodes_anim.rs:2045`).
6. **Joins bake into sprite keys.** A joined sprite instance's
   key position is the evaluated join root, not the authored
   instance position (`build_instance_tracks` — `join_roots`).
7. **Movieclip sprites vanish.** Sprite instances inside a
   `.nodemc` that converts to a `ClipDef` "simply are not
   carried into the `ClipDef`" (`clip_figure_instance` /
   `convert_clip`, `src/nodes_anim.rs:2117` region) — clip
   conversion is figures-only.
8. **Silent skips.** Missing asset, undecodable PNG, empty key
   list, non-positive scale: every surface skips the instance
   without a status, warning or log (§3). Nothing counts them.
9. **Frame operations do follow sprites.** Frame swap remaps
   and re-sorts sprite/text instance keys (`frame_swap.rs:33,
   181-196`) — sprites are inside the seven frame-pinned
   collections. (Insert/delete-frame shifting for instance
   keys is Wave 13 C7's audit — contracted, integrator to
   confirm.)

---

## 6. The pattern Wave 13 mirrors: text instances [before W13]

The roster directs squad A to mirror `TextInstance`'s surface,
and source shows why: texts are the same model shape with a
complete shell presence that sprites never got.

- `text_tool.rs` documents its own census: `TextInstance` has
  exactly four fields, and at the time of that audit **no editor
  op read or wrote `Project.texts`** (`src/rune_sec/text_tool.rs:41-54`)
  — the panel was built panel-locally on top of the vec.
- The panel lists instances as rows with real indices
  (`text_instances` → `TextInstanceRow`,
  `src/rune_sec/text_tool.rs:450-481`), selects, and edits them;
  the stage draws them through `stage_text::text_geometry`
  (`src/rune_sec/stage_text.rs:284-345`), which samples text
  keys with the same `InstanceKey::sample` sprites use.
- MCP text tools author stage texts (the `export.rs:744-749`
  gate comment names them: "Load-bearing now that MCP's text
  tools author stage texts").

Sprites entering Wave 13 have the model, the placement pass
and the stage placeholder — and none of the rest of that list.

---

## 7. The Wave 13 surface (roster squad A) — contract status

The roster's A-squad contracts, each marked with what source
showed at the final check. Six of the nine code items were in
the tree; **A2, A3 and A5 were not**, and two in-tree items
(A7's note, A8's help row) point at the missing A3 panel by
name — the integrator must reconcile those pointers against
what actually lands, or the UI will direct users to a panel
that does not exist.

| Item | Contract (roster) | Status at final check |
|---|---|---|
| **A1** `src/sprite_ops.rs` (NEW) | Engine ops on `EditorCore`: `add_sprite` (asset → instance at stage centre, one key at frame 0), `remove_sprite`, `set_sprite_key` (frame, x/y/rotation/scale), `delete_sprite_key`, sampling helpers; exactly one snapshot per mutating op; refusals status-only | **[W13, in tree].** `add_sprite` (`sprite_ops.rs:104`), `remove_sprite` (`:129`), `set_sprite_key` (`:149`), `delete_sprite_key` (`:193`) — as `EditorCore` methods. Verified against the contract: bounds-checked asset (bounds only, no decode — the module doc argues the renderers skip bad bytes), one key at frame 0 at stage centre, scale 1.0, angle 0.0; `set_sprite_key` replaces a same-frame key or inserts sorted (keeping §1.1's order assumption), clamps frame to the last frame, refuses non-finite values; `delete_sprite_key` may delete the last key, leaving an instance that renders nothing; every refusal is status-only with **no** snapshot, every success snapshots exactly once (the module doc, `sprite_ops.rs:1-60`, states the same rules) |
| **A2** `src/stage_sprites.rs` (NEW) | Stage prims from the sampled transform: quad corners via `media_items` geometry, outline + corner marks + name-label prim spec; point-in-quad hit-test geometry | **[W13, in tree].** Landed after this table's snapshot: prims derive from `media::media_items` verbatim; the stage's generic placeholder loop skips sprite items and a dedicated pass paints fill/outline/corner marks (corner marks on the panel-selected sprite via `StageOverlays.sprite_selected`), with labels laid out by A6's fn inside the quad. Pixel-proven in the sprites-populated snap. (Label placement reconciled at integration: A2's spec decides WHICH sprites + names, A6's in-quad layout places the glyphs.) |
| **A3** `src/rune_sec/sprites_panel.rs` (NEW) | Sprites panel: instance list (asset name, key count), add-from-assets picker, select, delete, per-key steppers (x, y, rotation, scale) at the current frame; panel-local state (integrator adds the shell `State` field); keys `sprite-*` | **[W13, in tree].** Landed after this table's snapshot and spliced by the integrator: `State.sprites_panel` field, mounted in the Animate right column after Scene (`Section::Sprites`), rows/steppers/key add-delete all live; the Round 14 Runtime test drives add + delete through real taps. |
| **A4** `src/media.rs` handlers | MCP handlers `tool_sprite_list` / `tool_sprite_add` / `tool_sprite_remove` / `tool_sprite_set_key` / `tool_sprite_delete_key` (handlers only; registry `Tool` entries are the integrator's splice), plus a `media_items` ordering audit against the raster composite order | **[W13, in tree] — handlers only.** All five handlers exist (`media.rs:725, 744, 768, 794, 851`), thin wrappers over A1's ops (module doc `media.rs:50-51`; refusals surface `core.status` as the error). The registry splice landed at integration: five `Tool` entries in `mcp_tools.rs` (registry 230 → 235, total 288), so the tools are reachable under exactly the names A8's help text uses. |
| **A5** `src/nodes_anim.rs` | Sprite round-trip audit for the animation containers: does export write instances back, does import restore them byte-faithfully; fix what's broken, pin the verdict in tests either way | **[W13, in tree].** Landed after this table's snapshot: `.stknds` export now writes the sprite library (unique assets, first-use order) + type-2 instance records; the round-trip is pinned in tests. Documented losses: groups return as member images, fractional/out-of-timeline keys are omitted, texts still dropped. |
| **A6** `src/rune_sec/stage_text.rs` | Extend the label machinery so A2's prims can carry asset names without duplicating text layout (label boxes for sprite quads) | **[W13, in tree].** `sprite_label_boxes(corners, name, color)` (`stage_text.rs:304` onwards): the asset name centred on the quad, running along its top edge, glyph height fitted to the quad (run-length cap `5 · top_len / (4n − 1)` cells, plus a quad-height fraction cap), fully opaque glyph boxes; empty/blank names and degenerate quads yield no prims. It builds on [before W13] `label_boxes` / `label_boxes_alpha` (`stage_text.rs:166-176`) rather than duplicating layout. Its consumer (A2's stage pass, via the integrator's `stage_view` wiring) landed with the wave — the labels render on stage. |
| **A7** `src/rune_sec/scene_panel.rs` | Placed-media readout becomes the entry point: counts stay, the "exports only / can't be edited" note is replaced by a pointer to the Sprites panel | **[W13, in tree].** Counts unchanged; `media_note` now returns *"Sprites are edited in the Sprites panel — move, rotate, scale and keyframe them there."* (`scene_panel.rs:604-612`), module doc updated (`scene_panel.rs:143-154`). The panel it names (A3) landed with the wave. |
| **A8** `src/rune_sec/help_panel.rs` | "Sprites" row rewritten to the landed truth (panel home, keyframed transform, MCP tools); in-file pins updated; still 61 rows | **[W13, in tree]** (the first squad-A edit to appear). The row now reads: *"Sprites panel — free-floating images from the project's assets, each placed with keyframed position, rotation and scale: add an instance from the panel, select it, and step its transform keys at the current frame (the stage draws an outline and label; exports render the real image). MCP clients use sprite_list, sprite_add, sprite_remove, sprite_set_key and sprite_delete_key. An image fixed to a segment is attached from the segment inspector instead"* (`help_panel.rs:561-562`), module doc updated (`help_panel.rs:350-362`), test pins rewritten (`help_panel.rs:1275-1308`). Both surfaces it describes (A3's panel, A2's stage outline + label) landed with the wave, so the row reads true. |
| **A9** `src/rune_sec/export_panel.rs` | Sprite honesty notes per format under an `xc-sprite-note` key family (extending the Wave 12 `piv_media_note` pattern) | **[W13, in tree — text corrected at integration].** `sprite_note`, key `xc-sprite-note`: the launch text claimed `.stknds` writes its library empty, which A5's fix made false before the wave closed; the integrator rewrote the note to the landed truth (*".stk and .nodes carry only the selected figure and omit sprites; .stknds carries its sprite library and instances"*) and updated its pin test in the same commit. (Key is the singular `xc-sprite-note`, not a family.) |
| **A10** this doc | — | — |

Related Wave 13 contracts outside squad A that touch sprites:
**C6** (`model.rs` serde pins + `sample` edge tests), **C7**
(`frame_swap.rs` insert/delete shifting audit), **C10**
(`tests/wave13_e2e_sprites.rs`, sprite MCP e2e with WAVE13-GAP
funnel helpers if the registry splice lags), **D1/D2**
(`sprites-populated` scene + headless snap), **D3** (snap-test
Round 14 sprite panel flow), **E3** (palette commands
`add-sprite` / `remove-sprite` / `next-sprite`), **E8**
(workspace gating for the panel). This doc did not audit
those files; their status is the integrator's to confirm at
close-out.

---

## 8. Stage placeholder honesty

The stage's sprite rendering, stated as plainly as the code
states it:

- Rune's `Shape` has **no bitmap primitive** — the stage
  *cannot* draw the PNG (`stage_view.rs:1123-1128`, named
  "the hard limit A6 named R1" in Wave 12). This is a
  toolkit limit, not a missing feature flag.
- What the stage draws instead is **geometry-exact**: the
  same quad `media_items` computed for the export, filled
  with the selection accent at 0.16 × alpha and stroked at
  0.7 × alpha, 1.0 width, round caps (`stage_view.rs:1137-1146`).
  Position, size and rotation on stage are therefore true;
  appearance is not.
- The pass is **not sprite-specific**: it iterates every
  `MediaItem::Image` — segment media and sprite instances
  alike — with no label, no name, no selection state and no
  hit-testing. There is still no way to click a sprite, tell
  two overlapping sprites apart, or learn which asset a quad
  belongs to from the stage.
- Wave 13 halves that gap on paper: A6's label plumbing
  **is in the tree** (`sprite_label_boxes`, §7) — but its
  only caller would be A2's sprite stage pass, which is
  **not**. So at the final check the labels are computed by
  no one and drawn nowhere, and the honest summary for users
  remains: real pixels in exports; a geometry-exact outline
  on stage; editing via the A1 ops (and, once spliced, the
  A4 tools) — not from the stage and not yet from a panel.

---

## 9. Summary

- **Model:** an asset index + a linear-interpolated
  `{frame, x, y, scale, angle}` key track. No opacity, no
  name, no non-uniform scale. [before W13]
- **Producer:** Stick Nodes animation import, or project
  JSON. Nothing else. [before W13]
- **Rendering:** one placement pass (`media_items`) feeding
  real pixels in raster and SVG, and a geometry-exact
  placeholder outline on stage. [before W13]
- **Interchange:** `.piv`, `.stk` and `.nodes` figures drop
  sprites entirely; `.stknds` imports them and (since A5)
  exports the sprite tracks back — groups return as plain
  member images and texts are not carried; project JSON
  round-trips them fully.
  [before W13]
- **Wave 13:** squad A landed in the tree in pieces — the
  engine ops (A1), the MCP handlers (A4, registry splice
  still the integrator's), the label plumbing (A6), the
  Scene pointer (A7), the help row (A8) and the export
  note (A9). Still **contracted, integrator to confirm**:
  the stage prims (A2), the Sprites panel itself (A3) and
  the `.nodes` round-trip fix (A5). Two in-tree texts (A7,
  A8) already point users at A3's panel — reconciling
  that at splice is the wave's sharpest honesty item.
