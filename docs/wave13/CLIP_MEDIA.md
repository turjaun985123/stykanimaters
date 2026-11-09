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

---

## 7. Wave 14 landed addendum — **LANDED**

Wave 14, squad C (roster: `docs/wave14-roster.md`) is contracted
to implement §6 of this audit, plus the clip-def media MCP tools
(C5) that §6.7 left to a future wave. This addendum is the
close-out skeleton, written by C9 **before** integration. It
asserts nothing about the closing tree.

**Closed at the Wave 14 close-out (2026-11-09).** Every
anchor below was verified against the closing tree by the
integrator: all named test pins grep in their files and
passed in CI run **42571128696** (GREEN on `8f5bb54`,
3,519 tests, 0 failed — including the C7 suite
`tests/wave14_e2e_clip_media.rs` and the C10 suite
`tests/wave14_e2e_clip_media_mcp.rs`); the funnel fallback,
the `has_media` clips clause, the single stage pass and
the funnel-fed SVG emission all grep as claimed; gate
check 9 passes with the three clip tools counted.
§§1–5 stand as written: they are the entering record and
are **not** edited by this addendum; this addendum
supersedes §5's per-surface verdicts (only) for the
surfaces it names. W14-8 closes as **project thumbnails
only**: C8 argued the figure-card/library thumbnails
provenance-blind (they are built from figure defs) and
landed no change there — no doc may claim more.

### W14-1 — Enumeration: the per-node fallback exists in the funnel (C1)

- [x] **Claim.** `media::media_items` (`src/media.rs`) resolves
  each node's media per §6.1: the instance's own
  `fig.def.nodes[ni].media` first; else the clip def's
  `clips[ci].def.nodes[ni].media` when `fig.clip = Some(ci)`;
  at most **one** item per (figure, node).
- **Anchor (grep).** Entering this wave, `media_items`
  contained no `clips` read at all (§2). After C1,
  `grep -n "clips" src/media.rs` must hit inside the
  `media_items` figure loop — the fallback — and the function
  must still be the only sampling site (no second media pass
  elsewhere: `grep -rn "media_items" src/` lists the same
  consumers as §2's table, plus any C2/C8 additions C9 has
  recorded).
- **Anchor (tests).** The five pins of §6.6 exist in
  `src/media.rs`'s tests and pass in the closing CI run:
  clip-def-only image yields one item; clone case yields
  exactly one item; instance media wins; clip-def text yields
  a text item; longer/shorter clip defs skip without panic.
  C1's report names them; a missing name is a failed claim.
  (Re-dispatch note, C9: C1's pins are in the tree and named —
  `clip_def_image_media_renders_for_media_free_instance`
  (:2141), `clip_and_instance_media_never_double_draw`
  (:2174), `clip_def_text_media_uses_clip_payload_and_colour_fallback`
  (:2235), `clip_def_longer_and_shorter_than_instance_def_is_safe`
  (:2277) and `missing_clip_and_media_free_clip_def_change_nothing`
  (:2308); grep the names, then confirm they pass in CI.)

### W14-2 — Enumeration order: clip-sourced items take the figure's slot (C1)

- [x] **Claim.** No new pass and no new ordering rule (§6.2):
  clip-sourced items appear in the funnel's existing order —
  figure order, then node order, with sprite instances and
  text instances still appended after all segment media.
- **Anchor (grep).** The sprite/text append loops in
  `media_items` still follow the figure loop, and C1 added no
  second returned collection: `grep -n "pub fn media_items"
  src/media.rs` shows the same single-funnel signature as
  entering.
- **Anchor (test).** If C1 pinned order explicitly, its report
  names the test; if not, W14-1's one-item pins plus the
  unchanged consumer table (§2) are the evidence, and this
  claim is recorded as verified-by-structure, not by test.

### W14-3 — Alpha and colour provenance for clip-sourced items (C1)

- [x] **Claim.** A clip-sourced image item's alpha is the
  **instance's** `fig.opacity` × the **clip node's**
  `style.opacity` (each clamped 0..=1); a clip-sourced text
  item's colour is `clip_node.color.unwrap_or(fig.color)`
  (§6.2).
- **Anchor (test).** §6.6 pin 1 asserts the alpha product with
  differing instance/clip-node factors (a test where both
  factors are 1.0 does **not** verify this claim); §6.6 pin 4
  asserts the text colour fallback. Both pass in CI.
  (C9 re-dispatch note: the pins are
  `clip_def_image_media_renders_for_media_free_instance`
  (media.rs:2141, alpha) and
  `clip_def_text_media_uses_clip_payload_and_colour_fallback`
  (media.rs:2235, colour).)

### W14-4 — Stage treatment: same placeholder pass, no bitmap, no new pass (C2)

- [x] **Claim.** Clip-sourced media reaches the stage through
  the existing `media_items` placeholder pass
  (`src/stage_view.rs`, entering anchor :1128-1137): the
  item's exact quad, accent fill and outline at the pass's
  standing alpha factors, item label as for figure-def media.
  Rune still draws no bitmap, and no clip-session media
  preview was invented (§6.5).
- **Anchor (grep).** `grep -n "media_items"
  src/rune_sec/stage_view.rs` (or `src/stage_view.rs`, per the
  closing tree) still shows the single media pass consuming
  the funnel; any clip-specific stage code C2 added must be
  named in C2's report and this claim reworded to match —
  an unnamed second path fails the claim as written.
  (C9 re-dispatch note: the pass is at `src/stage_view.rs:1144`
  under the "Image media placeholders (Wave 12, A6 §3b)"
  comment; it carries no clip-specific branch — clip-sourced
  image items arrive through the funnel and are placeholdered
  like figure-sourced ones, sprite instances still skipped to
  the Wave 13 sprite pass.)
- **Anchor (proof).** C7/C10 or a headless scene shows a
  clip-def-only image as a stage placeholder; the evidence
  named in C2's report is what the integrator cites here.

### W14-5 — Raster: real pixels, and the `has_media` gate extended (C3)

- [x] **Claim.** `render_frame_at` composites clip-sourced
  image media as real pixels, and a project whose **only**
  media is clip-def media no longer skips `composite_media`:
  `has_media` (`src/export.rs`, entering anchor :758-768)
  gained the clips clause of §6.4.
- **Anchor (grep).** `grep -n "clips" src/export.rs` hits
  inside `has_media` (the clause iterates
  `project.clips … def.nodes … media.is_some()` or an
  equivalent helper). (C9 re-dispatch note: as landed by C3,
  `has_media` is a `let` binding, not a fn — the gate is at
  `src/export.rs:768-776` under a "Wave 14 (C3)" comment at
  :761-767, and the clause reads
  `project.clips.iter().any(|c| c.def.nodes.iter().any(|n| n.media.is_some()))`.)
- **Anchor (test).** The §6.6 export pin exists in
  `src/export.rs`'s tests — a clip-def-only-media project
  renders the image through `render_frame_at` — and passes
  in the closing CI run. Project thumbnails (Home) inherit
  this via `projects::thumbnail` → `render_frame_at` with no
  separate claim (see W14-8). (Named:
  `clip_def_only_media_renders_into_rendered_frames`,
  export.rs:3966, over the `clip_only_project` fixture.)

### W14-6 — SVG: clip-sourced items emitted by `append_media` (C4)

- [x] **Claim.** SVG export emits clip-sourced media through
  the same `append_media` path as figure-sourced media
  (`src/svg.rs`, entering anchor :667): an image item as a
  `<use>` of the per-asset embedded PNG with its placement
  matrix (and the opacity attribute under the standing
  `< 1.0` rule), a text item as a `<text>` element; media
  still emits last.
- **Anchor (grep).** `grep -n "media_items" src/svg.rs`
  shows `append_media` still consuming the funnel, and
  C4 added no clip-specific emission branch (any such branch
  must be named in C4's report and this claim reworded).
- **Anchor (test).** A test in `src/svg.rs`'s tests builds a
  project with clip-def-only media and asserts the emitted
  document contains the image `<use>` (and/or `<text>` for
  the text variant); C4's report names it; it passes in CI.
  (C9 re-dispatch note: C4's pin is
  `svg_clip_def_image_emits_def_use_with_matrix_after_strokes`
  (svg.rs:1986) under a "Wave 14 (C4)" section comment at
  :1895 stating `append_media` has no clip-specific branch.)

### W14-7 — Asset census: behaviour unchanged, refusal now honest (C6)

- [x] **Claim.** `asset_references`, `remap_asset_refs` and
  `tool_asset_remove` behave exactly as entering (§6.5):
  clip-def references still count, still remap, and still
  refuse removal — and the refusal is now honest, because
  the media renders (W14-1…W14-6) and can be detached
  (W14-9).
- **Anchor (grep + test).** `grep -n "fn asset_references"
  src/media.rs` and the entering refusal pin (entering
  `src/media.rs:1550`, the clip-def-only refusal test) both
  present; the pin passes unmodified in the closing CI run.
  C6's report must state whether any census code changed at
  all — the spec says none should; an unreported change
  fails this claim. (C9 re-dispatch note: C6's definitive
  census now documents `asset_references` at
  `src/media.rs:413-465`, including the forward rule for new
  writers, and C6 reported **no logic change**; its new pin
  `asset_remove_remaps_clip_def_references_and_info_counts_them`
  (media.rs:1767) must also pass in CI.)

### W14-8 — Thumbnails: exactly the surfaces C8 names, no broader claim (C8)

- [x] **Claim.** Project/Home thumbnails carry clip-def
  media (a consequence of W14-5, via `render_frame_at`).
  Figure-card and library thumbnails carry clip-def media
  **only if** C8's report says its remit required it and
  names the code that does it — those thumbs are built from
  figure defs, and §6 promised nothing for them.
- **Anchor.** C8's report, quoted here at close-out with the
  surfaces it names. If C8 landed no thumb change, this
  claim closes as: project thumbnails only — and any doc
  (including `docs/SPRITES.md`) claiming more is wrong.

### W14-9 — MCP attach / clear / read parity for clip-def media (C5)

- [x] **Claim.** An MCP client can attach an existing asset
  as a clip definition's node image media, clear a clip
  definition's node media (either kind), and read it back —
  the clip-def counterparts of `asset_attach`,
  `clear_node_media` and `get_node`'s media field. Mutating
  tools take exactly one undo step; the read is read-only;
  refusals quote the offending clip/node/asset index; the
  read returns the media in `get_node`'s shape (`null`, or
  `{kind: 'image', asset}` / `{kind: 'text', content,
  size}`).
- **Anchor (grep).** The registered names from C5's report
  hit in `src/mcp_tools.rs` (registry entries) with handlers
  in the clip/media domain files; the proposed names in
  `docs/wave14/CLIP_MEDIA_ROWS.md` (`clip_asset_attach`,
  `clip_clear_node_media`, `clip_get_node_media`) are what
  the integrator greps for first — a renamed tool is fine,
  an unrecorded one is not.
- **Anchor (docs + gate).** The MCP.md rows in
  `docs/wave14/CLIP_MEDIA_ROWS.md` are merged into
  `docs/MCP.md` next to `asset_attach`, and gate check 9
  (registry ↔ MCP.md two-direction sync) passes on the
  closing tree with the new tools counted.

### W14-10 — End-to-end: a clip-media project built and exported via MCP only (C10, C7)

- [x] **Claim.** The wave's e2e suite creates a project
  whose clip-def media is attached via MCP, and exports it
  via MCP, asserting the media is present in the outputs;
  C7's session/timeline pins (clip media sampled across
  frames, one clip def reused by several instances) pass
  alongside.
- **Anchor (test).** The test file(s) named in C10's and
  C7's reports under `tests/` (and `src/media.rs` for C7's
  unit pins) pass in the closing CI run. The clip editing
  session still has **no** media preview (§6.5) — if any
  session media surface appears, it is named here as a
  scope addition, not silently absorbed.
