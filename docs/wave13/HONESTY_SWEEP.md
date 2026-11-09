# Wave 13 — Honesty sweep (E10)

Two passes, both from source, not from agent reports:

- **Pass 1 — the entering tree.** Wave 12's additions
  re-verified with fresh eyes: every new key, button,
  command and tool traced to a real handler. Anything
  dead would be named here with file/line evidence.
- **Pass 2 — the Wave 13 contracts.** Every surface the
  roster (`~/workspace/wave13-roster.md`) promises,
  written as a checklist the integrator can tick off
  against the landed tree at close-out. Each row names
  where the handler must live; a row that cannot be
  ticked is a slip to fix or a claim to pull back —
  never a silent ship.

Sweep state note: this doc was written while the wave
was in flight. Pass 1's verdicts are about the entering
tree plus landed Wave 13 files as observed at sweep
time (each such observation is dated as "at sweep");
pass 2 is the closing contract regardless.

---

## Pass 1 — Wave 12's additions, re-verified

**Verdict: no dead controls found in Wave 12's
additions.** Every surface below resolves to a handler
that does real work. Two findings follow the table —
both are about *text* whose truth Wave 13 changes,
not about dead controls.

### 1.1 The six asset tools (B1 splice, Wave 12)

| Tool | Tool entry (src/mcp_tools.rs) | Handler (src/media.rs) | Body verified |
|---|---|---|---|
| `assets_list` | :5523, `handler: crate::media::tool_assets_list` | `tool_assets_list` :462 | Returns `{ok, count, assets}` from the live table |
| `asset_info` | :5529 | `tool_asset_info` :479 | Index-checked single entry |
| `asset_import` | :5538 | `tool_asset_import` :495 | Real validation: file must exist, ≤ `ASSET_IMPORT_MAX_BYTES` = 8 × 1024 × 1024 (src/media.rs:293), PNG magic + full decode + non-empty; `core.snapshot()` before the push — exactly one undo step |
| `asset_rename` | :5550 | `tool_asset_rename` :545 | Same-name rename is a no-op with `changed: false`, spends no snapshot |
| `asset_remove` | :5562 | `tool_asset_remove` :585 | Refuses with live counts from `asset_references` (src/media.rs:379 — segments, clip segments, sprites); on success snapshots once and remaps higher references via `remap_asset_refs` (:407) in the same step |
| `asset_attach` | :5571 | `tool_asset_attach` :623 | Writes `SegmentMedia::Image` on the addressed node |

Supporting read path: `get_node`'s `media` member
exists at src/figure_ops.rs:1413 (`null` /
`{"kind":"image","asset":N}` / `{"kind":"text",…}`),
and the strict e2e suite `tests/wave12_e2e_assets.rs`
drives all six tools through real dispatch.

### 1.2 The three palette asset commands (C5 spec, applied at integration)

| Command (src/rune_sec/command_palette.rs) | Shim | Resolves to |
|---|---|---|
| `attach-media-import` :1322 | `cmd_attach_media_import` :1028 | `EditorCore::attach_image_to_node` (src/editor.rs:2464) — the same op the panel's import button presses |
| `clear-node-media` :1323 | `cmd_clear_node_media` :1036 | `EditorCore::clear_node_media` (src/editor.rs:2519) — selection resolved before `snapshot()`, so a refusal spends no undo step |
| `cycle-attached-sprite` :1324 | `cmd_cycle_attached_sprite` :1046 | The panel's own composition: `inspector_segment::next_asset_target` (:654) over `attached_asset` (:632), then `attach_asset` (:687) — all three `pub` in src/rune_sec/inspector_segment.rs |

Palette count pin stands at 132 in-file; the gate
derives the count live (check 5).

### 1.3 Library thumbnails (D5, Wave 12)

- `library_thumb_prims` (src/rune_sec/library_panel.rs:1043)
  builds pieces from `paint::styled_segment_pieces`;
  `library_thumb_shape` (:1130) folds them into a canvas
  `Shape`.
- **Production call site verified**: the row builder
  calls `library_thumb_shape(def, THUMB_SIZE, t.text)`
  at :1277 from the live `s.core.library` in the same
  render pass, and keys the canvas
  `library-thumb-{i}` at :1290. No cache, no stored
  image — a thumbnail cannot be stale or another
  entry's figure by construction.
- Verdict: live, not test-only. (Pixel proof is a
  separate matter — Wave 12 closed with the thumbnails
  unit-proven only; the `library-populated` scene is
  Wave 13 D1's contract, pass 2 below.)

### 1.4 The segment Media section (B2, Wave 12)

All keys in src/rune_sec/inspector_segment.rs:

| Key | Line | Handler / nature |
|---|---|---|
| `seg-media-readout` | :1247 | Text line only (not a control); shows "No media attached" / "Sprite: {name}" / "Text: {content}" from live state |
| `seg-media-attach-{idx}` | :1261 | Button per asset (cap `ASSET_BUTTON_CAP` = 8) → `attach_asset(&mut s.core, idx)` |
| `seg-media-cycle` | :1270 | "Next sprite" → `next_asset_target` ∘ `attached_asset` → `attach_asset`. **Renders only when the table exceeds the 8-button cap** — conditional by design (the picker covers ≤ 8 directly); the palette's `cycle-attached-sprite` is the always-available path. Not a dead control, but the conditional render is recorded here so nobody "discovers" it later |
| `seg-media-import` | :1295 | → `s.core.attach_image_to_node()` |
| `seg-media-clear` | :1300 | → `s.core.clear_node_media()`; **renders only while media is attached** (`st.media.is_some()`), so it can never be a dead press |

### 1.5 Help rows' claims (help_panel.rs, Wave 12 B9 + Wave 13 A8)

| Row | Claim | Verdict at sweep |
|---|---|---|
| "Project assets" (:553) | Scene panel Placed-media readout counts assets in use; the six MCP tools manage the list; a referenced asset cannot be removed | **True.** Readout keys `scene-media-readout` / `scene-media-note` at src/rune_sec/scene_panel.rs:1043/:1048 over `media_readout` (:579); all six tools per §1.1; the refusal matches `tool_asset_remove`'s body, sprites included |
| "Attach asset to segment" (:557) | Segment inspector attaches/clears; MCP clients use `asset_attach` | **True.** §1.4 + §1.1 |
| "Sprites" (:561) | A Sprites panel exists (add/select/delete, keyframed transform steppers); the stage draws an outline and label; exports render the real image; MCP clients use `sprite_list`, `sprite_add`, `sprite_remove`, `sprite_set_key`, `sprite_delete_key` | **Forward claim — see Finding F1.** Exports rendering sprites is true since Wave 12 (media compositing + SVG emission). The panel, the stage outline/label and the five tools were NOT all in the tree at sweep time |

### 1.6 Pass-1 findings (for the integrator — not fixed here)

**F1 — Two landed texts currently point at a panel and
tools that have not landed yet.** A7's scene note
(src/rune_sec/scene_panel.rs, `media_note`: "Sprites
are edited in the Sprites panel — move, rotate, scale
and keyframe them there.") and A8's "Sprites" help row
(above) are both in the tree. At sweep time:

- A1 engine ops: **landed** — src/sprite_ops.rs
  (`add_sprite` :104, `remove_sprite` :129,
  `set_sprite_key` :149, `delete_sprite_key` :193).
- A4 MCP handlers: **landed** — src/media.rs
  (`tool_sprite_list` :725, `tool_sprite_add` :744,
  `tool_sprite_remove` :768, `tool_sprite_set_key`
  :794, `tool_sprite_delete_key` :851).
- A2 stage prims: **not landed** — src/stage_sprites.rs
  absent.
- A3 Sprites panel: **not landed** —
  src/rune_sec/sprites_panel.rs absent; no `sprite-*`
  panel keys exist.
- Registry splice: **not landed** — no `sprite_list`
  … `sprite_delete_key` Tool entries in
  src/mcp_tools.rs; the five tools are unreachable by
  MCP clients until the integrator splices them.
- E3 palette sprite commands: **not landed** — no
  `add-sprite` in src/rune_sec/command_palette.rs.

This is a mid-wave state, not a shipped lie — but it
makes A2 + A3 + the splice + E3 **release-blocking for
A7/A8's text**: if any of them slips, the scene note
and the help row must be pulled back in the same
commit, or the app ships instructions pointing at
controls that do not exist. The help row's stage claim
("(the stage draws an outline and label)") depends
specifically on A2 landing *and* being mounted into
the stage build by the integrator.

**F2 — `asset_remove`'s description goes stale the
moment the sprite splice lands.** src/mcp_tools.rs:5563
(and its mirror, docs/MCP.md:496) says "sprite
instances have no delete op, so an asset referenced
only by sprites stays refused until the project
changes". A4's `tool_sprite_remove` (src/media.rs:768)
is exactly that delete op. At splice time, update both
texts: the remedy becomes "remove the sprite instance
with sprite_remove, then asset_remove". Left as-is,
the description would instruct users that a solvable
refusal is unsolvable.

---

## Pass 2 — Wave 13 contracts: the integrator's close-out checklist

Tick each row against the landed tree. "Where it must
live" is the roster's assignment; "check" is the
cheapest source-level proof. Closing counts are at the
end — they are part of the contract.

### A — Sprite GUI surface

| # | Promised surface | Where it must live | Check |
|---|---|---|---|
| A1 | `add_sprite` / `remove_sprite` / `set_sprite_key` / `delete_sprite_key` on EditorCore; one snapshot per mutating op; refusals status-only | src/sprite_ops.rs (NEW) | The four `pub fn`s exist; each mutator calls `snapshot()` exactly once on its success path; in-file tests pass in CI |
| A2 | Sprite stage prims (quad outline, corner marks, name label) from `media_items` geometry; point-in-quad hit test | src/stage_sprites.rs (NEW) + integrator mount in src/stage_view.rs | File exists; prim + hit-test fns exist; `stage_view.rs` references the module (an unmounted prim builder is a dead surface — F1's stage claim rides on this) |
| A3 | Sprites panel: instance list (asset name, key count), add-from-assets picker, select, delete, per-key steppers (x, y, rotation, scale) at the current frame; keys in the `sprite-*` family | src/rune_sec/sprites_panel.rs (NEW) + integrator adds the panel State field in src/main.rs and mounts the panel (workspace per E8) | File exists; `sprite-` keys present in the panel builder; every button/stepper closure calls an A1 op or a panel-local fn that does; panel reachable in the shell (mount site greppable outside the panel file itself) |
| A4 | Five MCP handlers + five registered tools: `sprite_list`, `sprite_add`, `sprite_remove`, `sprite_set_key`, `sprite_delete_key` | Handlers src/media.rs; Tool entries src/mcp_tools.rs (integrator splice); catalogue rows docs/MCP.md | Handlers exist (landed at sweep: media.rs:725–:851+); `Tool { name: "sprite_…", handler: crate::media::tool_sprite_… }` entries present; gate check 9 green (registry 230 → **235**, total 283 → **288**) |
| A5 | .nodes sprite round-trip verdict (fixed, or pinned as-is with the truth stated) | src/nodes_anim.rs | In-file tests pin sprite export/import behaviour either way; the verdict is stated in the file's docs, not just implied by tests |
| A6 | Sprite name-label layout without duplicating text machinery | src/stage_text.rs | A label-boxes entry point for sprite quads exists and A2 consumes it (no second text-layout implementation in stage_sprites.rs) |
| A7 | Placed-media note points at the Sprites panel | src/rune_sec/scene_panel.rs (`media_note`) | Landed at sweep — **tick only if A3 is ticked** (F1) |
| A8 | "Sprites" help row = landed truth; row count still 61 | src/rune_sec/help_panel.rs | Landed at sweep — **tick only if A2, A3, A4 and E3 are ticked** (F1); in-file pin test updated to the same strings; features count still 61 rows |
| A9 | Per-format sprite honesty notes; key family `xc-sprite-note` | src/rune_sec/export_panel.rs | Keys present; .piv note states sprite instances are dropped; .stk/.nodes wording matches A5's verdict |
| A10 | Surface doc with the per-format parity matrix (raster / SVG / stage / .piv / .stk / .nodes) | docs/SPRITES.md (NEW) | File exists; matrix's stage cell says placeholder outline (not real image); .piv cell matches A9's note |

### B — Dirty-marking / snapshot coverage

| # | Promised surface | Where it must live | Check |
|---|---|---|---|
| B1 | Census of every non-snapshot mutator with a snapshot-vs-`mark_dirty` verdict and reason | docs/wave13/DIRTY_CENSUS.md (NEW) | File exists; figure rename/recolour, layer writes, clip angle/root, project_rename name write each have a named row |
| B2 | `mark_dirty()` seam; core-side census mutators fixed; `redo` gains undo's `selected_figure` clamp (parity bug fix, NOT swap semantics) | src/editor.rs | `pub fn mark_dirty` exists beside `edits_since_autosave` / `note_recovery_written`; redo body clamps `selected_figure` the way undo does (side-by-side read) |
| B3 | Figure rename/recolour MCP paths per census | src/figure_ops.rs | Each op snapshots once or calls `mark_dirty()` per its census row; read-back tests present |
| B4 | Clip mutator family per census (`clip_set_angle`, `clip_set_root`, …) | src/fx_ops.rs | Same check against B1's fx rows |
| B5 | `project_rename`'s name write marks dirty (the stored-name divergence itself stays — standing exclusion) | src/projects.rs | The name write reaches `mark_dirty()` or a snapshot; no stored-name read-modify-write change |
| B6 | Audio clip mutators per census | src/audio_ops.rs | Same check against B1's audio rows |
| B7 | Graph key mutators per census | src/graph_keys.rs | Same check against B1's graph rows |
| B8 | Stroke mutators per census | src/draw_ops.rs | Same check against B1's draw rows |
| B9 | Direct-write MCP handlers rerouted through ops or snapshot per census; **no registry changes from this file** | src/mcp_tools.rs | Diff discipline: no new/removed `Tool {` entries attributable to B9 (registry delta is exactly A4's +5) |
| B10 | E2E pins: fixed families report dirty / advance the autosave count; undo-step counts match the census; refusals spend nothing | tests/wave13_e2e_dirty.rs (NEW) | File exists; drives real MCP dispatch; any `WAVE13-GAP` markers flipped to strict assertions at close-out (the Wave 12 B10 pattern) |

### C — Fidelity remainders

| # | Promised surface | Where it must live | Check |
|---|---|---|---|
| C1 | SVG z-order: strokes emit OVER figures, media last; S10 SVG half: node `style.opacity` reaches heads/rings/polys as opacity attributes | src/svg.rs | Source order in the emitter: figures → strokes → media; head/ring/poly elements carry the opacity attribute; byte-change tests pin the new order |
| C2 | S10 raster half: node opacity composes on heads/rings/polys (no hard colour overwrite) | src/export.rs | The head/ring/poly pass multiplies/fades by node opacity; tests pin a 0.5-opacity node's head |
| C3 | Stage pressured strokes honour baked per-point widths via C5's generator | src/rune_sec/stage_layers.rs | Stage stroke construction calls `paint::pressured_stroke_pieces`; no local averaging re-implementation |
| C4 | Figure-card thumbnails rebuilt on `paint::styled_segment_pieces` (library parity); ring-prim colour doctrine unchanged | src/rune_sec/figures_panel.rs | Thumb geometry calls the shared generator; the documented ring-colour rule's comment still matches the code |
| C5 | `pressured_stroke_pieces`: shared per-segment (a, b, width) construction; averaging rule pinned against export.rs's | src/paint.rs | `pub fn pressured_stroke_pieces` exists; tests assert the endpoint-average widths export.rs uses |
| C6 | Serde pins for sprite/text round-trips; `InstanceKey::sample` edge tests (empty, single, before-first, after-last) | src/model.rs | Tests present; no model shape changes in the diff |
| C7 | Frame insert/delete vs sprite/text instance keys: audited, fixed if contained, verdict pinned either way | src/frame_swap.rs | A test or doc verdict names whether instance keys shift; if fixed, a shifting test proves it |
| C8 | Live-MCP drain attribution + queue fairness re-checked against sprite/dirty write paths; health body still derives its tool count | src/live_mcp.rs | Health body reads `all_tools().len()` (288 after the A4 splice), no hardcoded count |
| C9 | STAGE_STYLE close-out flips as a dated addendum; ACCEPTANCE S10/S21 status updated by pointer | docs/STAGE_STYLE.md, docs/wave12/ACCEPTANCE.md | Addendum dated and matches C1/C2/C3's landed code (integrator verifies before commit, per the roster) |
| C10 | Sprite MCP e2e (list/add/set_key/remove, undo contract, refusal shapes) | tests/wave13_e2e_sprites.rs (NEW) | File exists; `WAVE13-GAP` markers, if any remain, are named in the close-out as slips — none ship silently |

### D — Pixel-proof + scenes

| # | Promised surface | Where it must live | Check |
|---|---|---|---|
| D1 | Scenes `library-populated` + `sprites-populated`; scenes 26 → **28**; layout facts **30/31** | src/rune_sec/scenes.rs | Both names in `scenes()`; `layout_assertions()` holds 31 sequential strings; gate check 8 green |
| D2 | Both scenes in the plain tree loop; snaps `snap-library-populated.png`, `snap-sprites-populated.png`; xvfb loop per the standing subset rule | .github/workflows/headless.yml | Both scene names + both snap probe lines present; gate check 8 forward/reverse coverage green |
| D3 | Round 14 snap tests: sprite panel flow (add via panel keys → instance exists; delete → gone); library thumbs present in the library-populated state | src/rune_sec/snap_tests.rs | Tests drive the A3 panel's real keys (not core ops directly) for the panel-flow half |
| D4 | Library viewport follow-through for taller thumb rows (LIST_HEIGHT / scroll); **keys unchanged** | src/rune_sec/library_panel.rs | Gate check 2: zero keys lost in the file |
| D5 | Missing-file recents entries dimmed/inert, still visible | src/rune_sec/menus.rs | Missing-file entries render with the dimmed treatment; existing-file entries unchanged |
| D6 | Overflow-scan self-test fixtures for sprites-panel and library tree shapes | scripts/overflow_scan.py | Self-tests cover both new shapes; scan logic changed only if a fixture proved a blind spot (the change, if any, is named in the close-out) |
| D7 | Gate docstring census lines for Wave 13; counts still derived live | scripts/phase_j_checks.py | Gate runs 9/9 on the closing tree; no hardcoded count replaced a derived one |
| D8 | Census upkeep for the sprite tools | scripts/mcp_census.py | Header states the post-splice truth (registry 235, total 288) or derives it; census run agrees with gate check 9 |
| D9 | This wave's proof ledger | docs/wave13/PIXEL_PROOF.md (NEW) | File exists; library thumbnails listed as entering gap #1 and closed only with the D2 snap evidence |
| D10 | This wave's acceptance checklist with blank run-id slots for close-out | docs/wave13/ACCEPTANCE.md (NEW) | File exists; per-squad proof obligations mirror this table's rows |

### E — QA + parity

| # | Promised surface | Where it must live | Check |
|---|---|---|---|
| E1 | Dirty dot + autosave line read path matches the B-squad contract | src/rune_sec/status_strip.rs | The strip reads the same counters B2's seam advances; a pin test ties a `mark_dirty()` write to the strip's dirty state |
| E2 | Every GUI `after_save` path calls `note_recovery_written` | src/rune_sec/project_tools.rs (+ callers) | Grep every after-save arm: none reaches a saved state without the note call |
| E3 | Palette sprite commands forwarding to A1's ops; count pin 132 → **135** as scoped — **CORRECTED AT CLOSE-OUT: two commands landed (`add-sprite`, `remove-sprite`); `next-sprite` was dropped in E3's own audit (no sprite selection existed to advance), pin and gate read **134** | src/rune_sec/command_palette.rs | Two `Command {` entries + two shim fns calling sprite_ops paths; in-file pin reads 134 |
| E4 | Closure-only style controls (gradient swatches, RGB steppers, recent colours) extracted to named fns; panel behaviour byte-identical | src/rune_sec/inspector_style.rs | The named `pub fn`s exist; existing panel tests pass unchanged in CI |
| E5 | Movieclip-def media audit (stored + serialised, never rendered) with anchored fix spec and per-format verdicts | docs/wave13/CLIP_MEDIA.md (NEW) | File exists; names where rendering should sample clip-def media |
| E6 | Root-clone test oddity verdict: fixed if real and contained, else the truth pinned in the file's docs | src/shell.rs | The oddity is either gone with a test or explained in shell.rs docs — not left ambiguous |
| E7 | Toast attribution for sprite + dirty families (live-MCP mutations surface honest toasts) | src/rune_sec/toasts.rs | Sprite/dirty mutations through the live path produce attributed toasts; tests pin at least the sprite family |
| E8 | Sprites panel workspace gating decided and truthful (host workspace; per-workspace panel counts) | src/rune_sec/workspaces.rs | The gating table names the Sprites panel's workspace; counts match the mounted panels (A3's mount must agree) |
| E9 | §6.8 skeleton in the §6.7 shape | docs/REDESIGN_PROGRESS.md | Section exists with scope, entering counts, exclusions and TODO-AT-CLOSE blocks |
| E10 | This sweep | docs/wave13/HONESTY_SWEEP.md | This file; F1/F2 dispositions recorded at close-out below |

### Closing counts (all part of the contract)

| Measure | Entering | Closing |
|---|---|---|
| MCP total | 283 (230 + 9 + 44) | **288 (235 + 9 + 44)** |
| Palette commands | 132 | **134** (two sprite commands; see the E3 row) |
| Scenes | 26 | **28** |
| Layout facts | 29 | **31** |
| Help rows | 61 | **61** |
| Keys | 923, zero lost | **≥ 923, zero lost** (gate check 2) |
| Gate | 9/9 | **9/9** |

### Standing exclusions — tick that they were NOT touched

- [ ] No project-swap undo/redo semantics change
      (B2's redo `selected_figure` clamp is parity
      within one op, not swap semantics).
- [ ] No accent-colour change.
- [ ] No stored-name-after-rename divergence fix
      (B5 marks dirty only).
- [ ] No waveform-badge change.
- [ ] No MCP-save `write_recovery` change.

---

## Close-out dispositions (integrator fills in)

- F1: ☐ A2/A3/splice/E3 all landed, A7+A8 text stands
      as written · ☐ something slipped — text pulled
      back in commit: ______
- F2: ☐ `asset_remove` description + docs/MCP.md row
      updated at splice · ☐ carried as a named
      remainder: ______
- Pass-2 rows not ticked: ______ (each must appear in
  §6.8's honest remainders by name)
