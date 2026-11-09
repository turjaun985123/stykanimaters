# Wave 14 — clip media row texts for integrator merge (C9)

Prepared by C9 for the integrator. **Nothing in this file is
merged yet**: `docs/MCP.md` and `src/rune_sec/help_panel.rs`
are not edited by C9. The integrator merges the texts below
once C5's tools are registered, then deletes or archives this
file per the wave's close-out habit.

Sources: the anchored spec in `docs/wave13/CLIP_MEDIA.md`
(§6 the render fix, §6.7 the authoring gap this wave fills),
the roster's C5 remit ("clip media attach/clear/read tool
parity complete"), and the entering MCP.md rows for the
figure-def counterparts (`asset_attach`, `clear_node_media`,
`get_node`) whose wording these rows mirror.

## 0. Tool names — PROPOSED, C5's registration wins; status **PENDING-C5**

C5's spec was not in the tree when this file was written, so
the names below follow the house conventions (the `clip_*`
family prefix of `clip_list`/`clip_duplicate`/…, crossed with
the segment-media names `asset_attach` / `clear_node_media` /
`get_node`):

**Re-dispatch check (C9, later in the wave): C5's handlers
and TOOL ENTRY SPECs are still NOT in the tree** —
`grep -rn "clip_asset_attach" src/` returns nothing, so these
rows remain **PENDING-C5**: the row texts stand, but nothing
here is mergeable until C5's registration exists for the
integrator to check them against. One change since this file
was first written: the proposed names are no longer C9's
alone — C10's e2e suite adopted them as the wave's contract
(`tests/wave14_e2e_clip_media_mcp.rs:50-52`, citing this
section as its source). A C5 rename therefore has to be
applied in **three** places: C5's registry entries, these
rows, and C10's funnels — the integrator renames all three
or none.

| Proposed name | Counterpart | Args |
|---|---|---|
| `clip_asset_attach` | `asset_attach` (figure, node, asset) | `clip`, `node`, `asset` |
| `clip_clear_node_media` | `clear_node_media` (selection-scoped there; by index here — a clip def has no selection to scope to) | `clip`, `node` |
| `clip_get_node_media` | the `media` field of `get_node` | `clip`, `node` |

If C5 registers different names, the integrator renames the
rows — the row **texts** stand as written. If C5 instead folds
the read into `clip_list` (per-node media in its reply), drop
the `clip_get_node_media` row and append this clause to the
existing `clip_list` row's Purpose cell instead: *"; each
definition's per-node segment media is included (null, or
{kind: 'image', asset} / {kind: 'text', content, size} per
node)"* — and record the choice in CLIP_MEDIA.md §7, W14-9.

Two scope notes the rows encode, both from parity with the
figure side:

- **Attach is image-only.** `asset_attach` attaches image
  media; no MCP tool attaches *text* media to a figure node
  either (text segment media is written by the GUI text
  path), so parity does not require a clip text-attach tool.
  The clear and read tools cover either kind, exactly as
  `clear_node_media` and `get_node` do.
- **Counting.** Three new registry tools, if all three land
  as proposed: registry 235 → 238, total 288 → 291. E3/D10
  own MCP.md's count statements — the integrator updates
  them from the gate's census on the closing tree, not from
  this file.

## 1. MCP.md rows (merge next to `asset_attach`)

Placement: directly after the `asset_attach` row in the
registry table (entering `docs/MCP.md:497`), with the
asset/sprite family — these are the clip-def counterparts of
the segment-media tools, and a reader comparing the two
families should find them side by side. Format matches the
table's four columns (Tool | Purpose | Required | Optional).

| Tool | Purpose | Required | Optional |
|---|---|---|---|
| `clip_asset_attach` | Attach the EXISTING asset at index 'asset' to movieclip definition 'clip', node 'node' as that segment's image media, replacing whatever media the definition's segment carried (the clip-def counterpart of asset_attach, which writes a figure's own definition). The media renders on every instance of the clip whose own definition carries no media at that node — an instance's own media wins. One undo step. | `clip`, `node`, `asset` | — |
| `clip_clear_node_media` | Clear the media (image or text) attached to movieclip definition 'clip', node 'node' (the clip-def counterpart of clear_node_media, which is selection-scoped to a figure). Clearing a node that carries no media is reported, not an error. One undo step when media was actually cleared. | `clip`, `node` | — |
| `clip_get_node_media` | Read the segment media attached to movieclip definition 'clip', node 'node': null, or {kind: 'image', asset} / {kind: 'text', content, size} — the same shape get_node returns for a figure's node. Read-only. | `clip`, `node` | — |

Wording discipline behind the rows, for the integrator's
check against C5's handlers (a mismatch is C5's or this
file's bug — resolve it before merging):

- "One undo step" follows the standing contract (every
  mutating tool = exactly one undo step). If C5's clear
  spends a step even when nothing was cleared, the row's
  conditional phrasing ("when media was actually cleared")
  is wrong and must be corrected to match — the handler's
  behaviour is the truth, the row follows it.
- "Replacing whatever media the segment carried" mirrors
  `asset_attach`'s entering wording deliberately.
- The fallback sentence in `clip_asset_attach` ("an
  instance's own media wins") states the §6.1 sampling rule
  — it is only true once C1 lands, which is why this file
  merges at integration, not before.

## 2. Help panel — verdict and replacement row text

**Verdict: YES, one row changes truthfully — "Project
assets" (a Features row in `src/rune_sec/help_panel.rs`,
entering text at :554).** The other two media rows were
checked and need no change (reasons below). The replacement
**edits an existing row's text only — the row count stays
61** (`shortcut_rows_has_exactly_61_rows` untouched), and no
row is added or removed.

Why "Project assets" must change: entering, it reads *"…an
asset a segment or sprite still uses cannot be removed,
detach it first"*. Before this wave that advice was
incomplete in a way a user could hit: a movieclip-def-only
reference also blocked `asset_remove` (CLIP_MEDIA.md §3),
yet no detach path for it existed anywhere — not in the
panel, not in MCP. After C5, the detach path exists and has
names; a row that sends the user to "detach it first"
without naming the clip tools is no longer the whole truth.
The replacement also makes the row's census complete by
naming the movieclip-definition reference kind explicitly.

**Replacement text** (label unchanged; merge into the
"Project assets" row — with C5's final tool names
substituted if they differ from §0's proposals):

> **Project assets** — "Scene panel — a Placed media readout counts the project's assets in use; the asset list itself is managed with the MCP tools assets_list, asset_info, asset_import, asset_rename, asset_attach and asset_remove — an asset a figure segment, a movieclip definition's segment or a sprite still uses cannot be removed, detach it first (a movieclip definition's segment media is read with clip_get_node_media and detached with clip_clear_node_media)"

Checked, no change:

- **"Attach asset to segment"** (entering :558) — *"Segment
  inspector — attach an image from the project's assets to
  the selected segment, or clear the segment's image; MCP
  clients use asset_attach."* Scoped to the segment
  inspector and a figure's selected segment; both halves
  stay exactly true after this wave. The clip-def path is
  MCP-only (no inspector surface — CLIP_MEDIA.md §6.5), so
  adding it here would misplace it; it is covered by the
  "Project assets" replacement instead.
- **"Sprites"** (entering :561) — about free-floating sprite
  instances; its closing sentence ("An image fixed to a
  segment is attached from the segment inspector instead")
  concerns figure segments and stays true. Clip-def media
  is not a sprite-instance topic.
- There is **no movieclip feature row** in the Help panel to
  update (the only "clip" rows are the audio and FX clip
  rows, which this wave does not touch), and adding one is
  E4's call under its own remit, not C9's — this file
  proposes none.
