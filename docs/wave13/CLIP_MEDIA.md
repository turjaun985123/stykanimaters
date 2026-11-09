# Clip-def media audit — movieclip definitions carry segment media that no renderer samples

Wave 13, E5. Audit of the Wave 12 B8 finding: `Project.clips[].def`
nodes can carry `SegmentMedia` (image / text). It is stored,
serialised, counted as an asset reference, and remapped on asset
removal — but no renderer ever reads it. All anchors are
`file:line` on the entering Wave 13 tree.

## Verdict

**The B8 premise is confirmed, with one refinement.** No surface
samples `clips[ci].def.nodes[].media` — the single sampling funnel
(`media::media_items`) iterates `project.figures` only. The
refinement: clip-def media is not invisible in every reachable
state, because every clip-instance creation path **clones the
clip def into the instance's own def**, and the clone's media
*does* render, through the instance. What never renders is media
that exists **only** in the clip def — which is exactly the state
reached when the originating figure is deleted, or when a project
file carrying clip-def media is loaded and its instances were not
cloned from that def. In that state the media is invisible
everywhere **and still locks the asset**: `asset_remove` refuses
removal on a clip-def-only reference (see §3). The fix spec in §6
is written so a future wave can implement it without re-auditing.

## 1. The model: two defs per movieclip instance

- `ClipDef` (`src/model.rs:473-481`): `{ name, def: FigureDef,
  frames: Vec<Pose>, loop_mode }`. Its `def` is a full
  `FigureDef`, so every node carries `media: Option<SegmentMedia>`
  like any figure node.
- `FigureInstance` (`src/model.rs:418-435`): a figure becomes a
  movieclip instance via `clip: Option<usize>` — "its pose comes
  from `Project::clips[clip]` looping, not from the timeline
  frames".
- `Project::poses_at` (`src/model.rs:885-906`) clones the timeline
  poses, then `apply_clips` (`src/model.rs:943-962`) replaces the
  pose of each clip instance with the clip's sampled frame pose
  (loop / ping-pong / once via `clip_frame_index`). **Only the
  pose crosses from the clip to the instance.** Every renderer
  then evaluates the **instance's own `fig.def`** with that pose.
  The clip's `def` contributes nothing to any drawing except its
  frames' poses.
- Instance creation clones defs, never links them:
  - `create_clip_from_figure` (`src/editor.rs:4359`, def cloned at
    :4371, `ClipDef` pushed at :4384) and its selection sibling
    `make_clip_from_figure` (`src/editor.rs:4113`, push at :4139,
    clone at :4141):
    the new `ClipDef.def` is `fig.def.clone()`; the originating
    figure keeps its own def and is pointed at the clip.
  - `insert_clip_figure` (`src/editor.rs:4150`, clone at :4161) and
    `insert_clip_figure_at` (`src/editor.rs:4607`, clone at :4615): the new
    instance's def is `clip.def.clone()`, then
    `figures[idx].clip = Some(ci)`.
  - `.nodes` import (`src/nodes_anim.rs:2255-2260`): same shape —
    instance `def: clip_def.clone()`.
- Nothing ever re-syncs an instance's def from its clip def after
  creation. The clip editing session (`src/editor.rs:4167+`,
  `clip_positions` at :4338 = `evaluate(&clip.def, pose)`) mutates
  only `clips[ci]` (frames and poses; `clip_set_angle` at :4311
  validates against `clips[ci].def.nodes.len()` at :4317), and the stage
  has no clip-session awareness at all — `src/stage_view.rs`
  never references the session; it draws `project.figures`.

## 2. Where figure-def media IS sampled: the one funnel

`media::media_items(project, frame_f)` (`src/media.rs:123-233`)
is the only place segment media becomes geometry:

- It takes `project.poses_at(frame_f)` (so clip instances are
  placed by their **clip poses**), skips hidden figures, and for
  each figure evaluates `evaluate(&fig.def, pose)` and walks
  **`fig.def.nodes`** for `node.media` (`src/media.rs:126-185`).
  `project.clips` is never touched — there is no `clips` read in
  the function.
- Image items carry `alpha = fig.opacity × node.style.opacity`,
  both clamped (`src/media.rs:161-164`, the Wave 12 A6 rule);
  text items take `node.color.unwrap_or(fig.color)`.
- Sprite instances and text instances are appended after all
  segment media (figure order, then node order, is the segment
  order).

Every rendering surface consumes this funnel, so every surface
inherits the same verdict:

| Consumer | Anchor |
|---|---|
| Raster exporter | `export::render_frame_at` (`src/export.rs:292`) → `composite_media` (`src/export.rs:770`), gated by `has_media` (`src/export.rs:758-768`) |
| SVG exporter | `append_media` iterates `media_items` (`src/svg.rs:667`) |
| Stage (placeholders) | `stage_shape` media pass iterates `media_items` (`src/stage_view.rs:1128-1137`) |
| Project thumbnails | `projects::thumbnail` → `render_frame_at` (`src/projects.rs:592-596`) |

## 3. Where clip-def media IS honoured: the storage side

The asset layer treats clip-def media as fully real:

- **Serialisation**: `ClipDef` derives `Serialize`/`Deserialize`
  with the rest of the model; the native `.styk.json` save/load
  round-trips it (model round-trip test reads `back.clips[0]`,
  `src/model.rs:1904`).
- **Reference census**: `asset_references` (`src/media.rs:379-401`)
  counts `(figure segments, clip-def segments, sprite instances)`
  — the module doc calls this "the complete census: an asset
  index can only be stored in those three places", and the
  handlers' header repeats it (`src/media.rs:280-286`).
- **Remapping**: `remap_asset_refs` (`src/media.rs:403-435`)
  shifts clip-def references down on removal, alongside figure
  and sprite references.
- **Removal refusal**: `tool_asset_remove` (`src/media.rs:588-622`)
  refuses when `segments + clip_segments + sprites > 0`. A
  clip-def-only reference therefore **blocks asset removal for an
  image nothing draws** — the sharpest symptom of the gap.
  (The refusal test for exactly this case already exists:
  `src/media.rs:1550`.)

## 4. How clip-def media arises (provenance)

There is **no authoring path that writes clip-def media
directly**. Every production media writer targets figure defs:
`attach_image_to_node` (write at `src/editor.rs:2651`),
`clear_node_media` (`src/editor.rs:2659`), `tool_asset_attach`
(`src/media.rs:630` — `figures[fi].def.nodes[ni]` only), the text
writers (`src/text_edit.rs:168-174`). The segment inspector has
no clip-session awareness, and no MCP clip tool writes node
media. Clip-def media therefore reaches a project only by:

1. **Make-clip from a media-carrying figure** (§1): the
   `ClipDef` inherits the figure's media by clone. The
   originating figure keeps its own copy, which renders.
2. **Deleting the originating figure** (or every instance cloned
   while the media was present): the media now exists *only* in
   the clip def — invisible, but still counted (§3).
3. **Externally authored project files**: the serde model accepts
   clip-def media without question, so a hand-edited or
   third-party `.styk.json` can carry it with no instance ever
   having cloned it.

Not reachable via `.nodes` import (the grammar has no node-media
encoding — `src/nodes_anim.rs` module doc: "Figure nodes never
gain `SegmentMedia` from this importer") or `.piv` import (no
clips are created at all: `clips: Vec::new()`, `src/pivot.rs:970`).

## 5. Per-surface verdicts

| Surface | Clip-def media | Why |
|---|---|---|
| Raster export (PNG/GIF/MP4/…) | **Never renders** | Funnel reads figure defs only (§2); the `has_media` gate doesn't know clip defs either (`src/export.rs:758-768`) |
| SVG export | **Never renders** | Same funnel (`src/svg.rs:667`) |
| Stage | **Never renders** (no bitmap, no placeholder) | Same funnel (`src/stage_view.rs:1137`); stage draws placeholders for figure-def media only |
| Stage, clip editing session | **Never renders** | The stage has no session rendering; `clip_positions` (editor.rs:4338) is geometry for posing ops only, with no media sampling attached |
| Thumbnails (Home / library) | **Never renders** | `render_frame_at` funnel (`src/projects.rs:596`) |
| `.styk.json` (native save) | **Stored & restored** | Full `Project` serde (§3) |
| `.piv` v2/v3/v4 | **Dropped** | No movieclip concept in the format; writers iterate `project.figures` only (`export_piv_v5` at `src/pivot.rs:2122` and siblings) — the whole `clips` table is absent, media with it |
| `.piv` v5 | **Dropped** (clip-def media) | Same writer shape; v5 does carry **figure-def** segment media — a clip *instance's own* def media rides its figure, the `ClipDef`'s does not |
| `.stk` | **Dropped** | `export_stk` (`src/pivot.rs:1808`) exports a single `FigureDef`; segment media is not written (Wave 12 B6) and there is no clip table |
| `.nodes` (`.stknds` export) | **Dropped** | `export_nodes_animation` (`src/nodes_anim.rs:3253`); module doc: "texts/joins/clips written as absent"; the grammar has no node-media encoding in either direction |

Clone-shadowing note for every "never renders" row: if an
instance of the clip was created *after* the media landed on the
source def, the instance's cloned def carries an identical copy
and **that copy renders normally**. The gap is strictly
*clip-def-only* media.

## 6. Fix spec (anchored, for a future wave)

One funnel change fixes raster, SVG, stage placeholders and
thumbnails together. No renderer code changes are needed beyond
one gate.

### 6.1 The sampling rule — per-node fallback in `media_items`

In `media::media_items` (`src/media.rs:123`), inside the figure
loop, resolve the media for node index `ni` as:

1. the instance's own `fig.def.nodes[ni].media`, if `Some`;
2. else, if `fig.clip = Some(ci)` and `project.clips[ci]` exists
   and `clips[ci].def.nodes.get(ni)` has `media = Some(..)`, that
   media;
3. else nothing.

At most **one** item per (figure, node) — this is the load-bearing
choice, for three reasons:

- **No double-draw.** In the normal clone case both defs carry
  the *same* media; sampling both would composite the image
  twice (alpha applied twice) and stamp the text twice.
- **Instance wins** because `make_clip_from_figure` keeps the
  figure editable in its own right: media attached to the figure
  *after* the clip was made must not be shadowed by the clip's
  older copy.
- **Clip fills gaps** so media orphaned in the clip def (§4.2/4.3)
  becomes visible on every instance of that clip — which is the
  whole point of the fix.

### 6.2 Geometry, order and alpha

- **Geometry**: place the item on the instance's evaluated
  segment, exactly as today — `pos = evaluate(&fig.def, pose)`
  with the clip pose already applied by `poses_at`. The clip def
  contributes the media payload only; the instance def's
  topology (parent, length) governs placement, because those are
  the segments actually drawn. (When the defs are clones — the
  normal case — the topologies are identical anyway.)
- **Order**: unchanged. Clip-sourced items take their figure's
  existing slot in the funnel's order (figure order, then node
  order; sprites and texts still last). No new pass, no new
  ordering rule.
- **Alpha (images)**: the existing product, with the style taken
  from the node that *carries* the media: for clip-sourced media,
  `fig.opacity.clamp(0,1) × clip_node.style.opacity.clamp(0,1)` —
  mirroring `src/media.rs:161-164` with the clip def's node style
  substituted. The figure factor stays the **instance's**
  `fig.opacity` (the instance is what is placed and faded).
- **Text**: content/size from the clip def's node media; colour =
  `clip_node.color.unwrap_or(fig.color)` — the same fallback
  shape as the figure path.

### 6.3 Edge cases (all must be skip, never panic)

- `fig.clip` names a missing clip (`clips.get(ci)` is `None`):
  no fallback — `apply_clips` already tolerates this
  (`src/model.rs:946-948`); mirror it.
- Clip def has **more** nodes than the instance def: indices
  beyond the instance's evaluated positions have no segment to
  attach to — skip them.
- Clip def has **fewer** nodes: trailing instance nodes simply
  have no fallback.
- Dangling asset index in clip-sourced image media: already
  handled — `png_aspect` returns `None` and the item is skipped
  (`src/media.rs:148-150`).
- Hidden figures stay skipped wholesale, clip media included —
  the media belongs to the figure's slot.

### 6.4 The raster gate

`export.rs`'s `has_media` gate (`src/export.rs:758-768`) checks
sprites / texts / figure-def media only. Extend it with
`|| project.clips.iter().any(|c| c.def.nodes.iter().any(|n|
n.media.is_some()))` — otherwise a project whose *only* media is
clip-def media would still skip `composite_media` entirely and
the funnel fix would not reach the raster. (SVG and the stage
have no equivalent gate; `append_media` and the stage pass call
the funnel unconditionally.)

### 6.5 What NOT to change

- `asset_references` / `remap_asset_refs` / `tool_asset_remove`:
  already correct — once clip media renders, the refusal at
  `src/media.rs:593` is honest. The existing refusal test
  (`src/media.rs:1550`) stays as the pin.
- No change to `apply_clips`, pose handling, or def cloning at
  instance creation; the fix is sampling-side only.
- The clip editing session gets **no** media preview in this
  fix: the session has no media surface today (no stage
  rendering, no inspector support — §1), and inventing one is a
  separate item, not part of closing the render gap.

### 6.6 Tests to pin with the fix

In `src/media.rs` tests (the funnel's home):

1. Clip-def-only image media yields an item: figure is a clip
   instance whose own def is media-free; the clip def carries an
   image → one `MediaItem::Image` at the segment quad, alpha =
   instance `fig.opacity` × clip-node `style.opacity`.
2. Clone case: identical media on both defs → exactly **one**
   item for the node (no double-draw).
3. Instance media wins: differing media at the same index on
   both defs → the instance's item, unchanged.
4. Text variant: clip-def text media yields a `MediaItem::Text`
   with the clip node's content and the colour fallback of §6.2.
5. Divergence safety: clip def longer and shorter than the
   instance def — extra indices skipped, no panic (§6.3).

And in `src/export.rs` tests: a project whose only media is
clip-def media renders the image in `render_frame_at` (proves
the §6.4 gate extension, not just the funnel).

### 6.7 Follow-on, explicitly out of scope here

Authoring media *directly* on a clip def (a clip-session media
op, or an `asset_attach` clip variant) does not exist and is not
specced above; after this fix, clip-def media still only arises
by the §4 paths — but it renders, which is what makes the asset
census honest. If a future wave adds clip-def authoring, it
should write through the same per-node model and will need no
further renderer work.
