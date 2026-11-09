# Wave 13 — acceptance checklist (D10)

The wave's executable close-out checklist, modelled on
`docs/wave12/ACCEPTANCE.md` §6. Every item is a checkable
assertion with its evidence source named: a test that must
exist and pass, a grep that must hit or miss, a count the
gate derives, or a document whose claim must trace to one
of those. A box is ticked only on that evidence — an
agent's report is a claim, and this list is how the
integrator catches a wrong one.

Scope, from the roster: **A** the sprite GUI surface
(B5's Wave 12 spec, the headline), **B** the
dirty-marking / snapshot-coverage fix (Wave 12's E8
remainder, now with its own contract), **C** the Wave 12
§6.7 fidelity remainders, **D** pixel-proof for the new
surfaces, **E** QA + parity.

**Entering baseline** (Wave 12 close, `docs/REDESIGN_PROGRESS.md`
§6.7): MCP **283 tools** (230 registry + 9 io + 44
built-ins); **3,156 tests** (CI 42557475828, GREEN);
keys **923**, zero lost; palette **132**; scenes **26**;
help **61 rows**; layout facts **29**; icons **163**;
i18n **99 × 28**; gate **9/9**.

**Exclusions guard** — the wave must NOT settle the
standing user decisions: no project-swap undo/redo
semantics change (B2's redo clamp is a parity bug fix,
not swap semantics), no accent-colour change, no
stored-name-after-rename divergence fix, no
waveform-badge call, no MCP-save `write_recovery`
change. §6 below makes each one a diff check.

---

## 1. Squad A — sprite GUI surface

**Engine ops (A1, `src/sprite_ops.rs`, NEW)**
- [ ] The file exists and defines `add_sprite`,
      `remove_sprite`, `set_sprite_key`,
      `delete_sprite_key` on `EditorCore` plus the
      sampling helpers (grep hits in production code,
      not only in tests).
- [ ] `add_sprite`'s contract pinned by test: the new
      instance references the given asset, sits at
      stage centre, and carries exactly one key, at
      frame 0 (`InstanceKey` shape: `src/model.rs:658`;
      `SpriteInstance { asset, keys }`: `src/model.rs:670`).
- [ ] **One-undo contract:** each mutating op spends
      exactly one snapshot — tests in `sprite_ops.rs`
      assert the undo depth moves by +1 per op and
      that one undo restores the prior `sprites` table.
- [ ] **Refusals are status-only:** bad asset index,
      bad sprite index and `delete_sprite_key` on a
      frame with no key each refuse with no snapshot
      spent and no state change (one test per refusal).

**Stage prims (A2, `src/stage_sprites.rs`, NEW)**
- [ ] Quad corners derive from the same sampling the
      raster uses — `media::media_items`
      (`src/media.rs:123`) over `InstanceKey::sample`
      (`src/model.rs:686`) — not a second sampling
      implementation (the fn's call graph is the
      evidence; a re-implemented lerp is a fail).
- [ ] The prim set is the honest placeholder family —
      outline + corner marks + name label — consistent
      with Wave 12's segment-media outlines. Rune
      `Shape` has no bitmap primitive: any claim, in
      code comments or docs, that the stage draws the
      real PNG is a fail.
- [ ] Hit-test geometry (`point-in-quad` or its shipped
      name) exists with tests: a point inside hits, a
      point outside misses, a corner/edge case is
      pinned explicitly.

**Panel (A3, `src/rune_sec/sprites_panel.rs`, NEW)**
- [ ] The panel mounts: the integrator-added `State`
      field exists, and the panel is reachable in the
      workspace E8 names (§5 cross-check — D1's
      sprites-populated tree must contain its keys).
- [ ] Every `sprite-*` key in the panel resolves to a
      real handler (gate key census + E10's sweep); the
      list rows show asset name and key count for each
      instance (asserted in the panel's tests or D3's
      Round 14).
- [ ] Add-from-assets, select and delete all route
      through A1's ops (panel-local state writes to
      `project.sprites` are a fail); the per-key
      steppers (x, y, rotation, scale) write at the
      current frame via `set_sprite_key`, one undo
      step per press.

**MCP tools (A4, `src/media.rs` + integrator splice)**
- [ ] Five handlers exist in `media.rs`:
      `tool_sprite_list`, `tool_sprite_add`,
      `tool_sprite_remove`, `tool_sprite_set_key`,
      `tool_sprite_delete_key` (grep hits).
- [ ] The splice landed: `sprite_list`, `sprite_add`,
      `sprite_remove`, `sprite_set_key`,
      `sprite_delete_key` are `Tool` entries in
      `mcp_tools::tools()`; the census (§7) reads
      registry **235**, total **288**, zero unregistered,
      and `docs/MCP.md` lists exactly the shipped set
      (gate check 9, both directions).
- [ ] Mutating sprite tools obey the standing contract:
      exactly one undo step each, refusals are `isError`
      results that spend nothing — proven end-to-end by
      C10 (§3), not only by handler unit tests.
- [ ] A4's `media_items` ordering audit is written
      down (in `media.rs` docs or `docs/SPRITES.md`):
      the item order `media_items` returns is stated
      against the raster composite order in
      `src/export.rs`, and any reorder A4 made is
      pinned by a media test.

**`.nodes` round-trip (A5, `src/nodes_anim.rs`)**
- [ ] The verdict is pinned by a test, whichever way it
      falls: either export writes sprite instances and
      import restores them byte-faithfully (round-trip
      test over a project carrying instances), or the
      exact loss is named in the test, in
      `docs/SPRITES.md`'s matrix and in A9's panel note.
      A round-trip claim without the test is a fail.

**Labels, notes, help (A6 / A7 / A9 / A8)**
- [ ] A6: sprite name labels ride the existing label
      machinery in `src/stage_text.rs` (label boxes for
      sprite quads); no duplicated text-layout code
      (grep: no second label-box constructor).
- [ ] A7: the Scene panel's placed-media note no longer
      says sprites are exports-only / cannot be edited
      — the entering wording **misses** on grep of
      `src/rune_sec/scene_panel.rs` — and the readout
      now points at the Sprites panel by name; the
      counts themselves are unchanged.
- [ ] A9: the `xc-sprite-note` key family exists in
      `src/rune_sec/export_panel.rs`; the `.piv` note
      states that sprite instances are dropped (Wave 12
      fact), and the `.stk` / `.nodes` notes match
      A5's pinned verdict word for word in substance.
- [ ] A8: the Help "Sprites" row describes the landed
      surface (Sprites panel home, keyframed transform,
      the `sprite_*` MCP tools); the entering
      "cannot be placed or edited in the app yet"
      wording **misses** on grep of
      `src/rune_sec/help_panel.rs`; the panel still
      holds **61 rows** and the in-file pin test asserts
      the new strings.

**Surface doc (A10, `docs/SPRITES.md`, NEW)**
- [ ] The doc exists and its per-format parity matrix
      has one row per surface — raster, SVG, stage,
      `.piv`, `.stk`, `.nodes` — each cell citing a
      test name or a code path. Spot-check rule for the
      integrator: pick any three cells at random, follow
      each citation, and it must land on a test or fn
      that exists (grep proves it). An uncited cell is
      a fail.

---

## 2. Squad B — dirty-marking / snapshot coverage

Entering truth (Wave 12 E8, source-proven): `edit_count`
bumps only inside `snapshot()` (`src/editor.rs:259`,
increment at :268), so whole mutator families never mark
the project dirty; the strip's own docs name the gap
(`src/rune_sec/status_strip.rs:778`). The wave's
contract: every project mutation either takes exactly
one snapshot (one undo step) or calls the new
`mark_dirty()` seam — dirty + autosave counting WITHOUT
an undo step — where undoability would be wrong. B1's
census rules per op.

**Census (B1, `docs/wave13/DIRTY_CENSUS.md`, NEW)**
- [ ] The census exists and rows, at minimum, every
      family named at launch: figure rename, figure
      recolour, layer locked / hidden / opacity writes,
      `clip_set_angle` / `clip_set_root` and the clip
      mutator family, `project_rename`'s name write,
      the audio clip mutators (B6), the graph key
      mutators (B7), the stroke mutators (B8), and the
      MCP handlers that write project state directly
      (B9). A family missing from the census is a fail.
- [ ] Completeness is re-derivable: the integrator
      greps the lib + bin for `EditorCore` / handler
      fns that write `Project` fields, and every hit is
      either snapshot-taking or a census row. Any
      unrowed mutator the grep finds fails the census,
      whoever wrote it.
- [ ] Every row carries a verdict — `snapshot` or
      `mark_dirty` — WITH its reason and an anchored
      patch spec (file + fn). A verdict-less row, or a
      reason that contradicts the one-undo house rule,
      is a fail.

**The seam (B2, `src/editor.rs`)**
- [ ] `pub fn mark_dirty` exists beside
      `edits_since_autosave` (`src/editor.rs:1905`) and
      `note_recovery_written`, and its semantics are
      pinned by test: after `mark_dirty()` on a clean
      core, `edits_since_autosave()` has advanced AND
      the undo depth is unchanged AND no history entry
      was created.
- [ ] Autosave cadence honours the seam: 25
      mark-only edits make autosave due
      (`autosave_due`, `src/editor.rs:1899`,
      `AUTOSAVE_EVERY` = 25) — unit test.
- [ ] **Redo clamp:** `redo` clamps `selected_figure`
      exactly as `undo` does — test drives undo-then-
      redo across a figure-count change and lands with
      `selected_figure` in range. This is the ONLY
      undo/redo behavioural change in the wave (§6
      diff check).

**Per-family fixes (B3–B9)**
- [ ] B3 (`src/figure_ops.rs`): figure rename and
      recolour follow their census verdicts; read-back
      tests show the stored value AND
      `edits_since_autosave()` advanced — on the
      entering tree both ops left it at 0.
- [ ] B4 (`src/fx_ops.rs`): `clip_set_angle`,
      `clip_set_root` and the rowed clip family follow
      the census; tests pin dirty + undo-step counts
      per verdict.
- [ ] B5 (`src/projects.rs`): `project_rename`'s name
      write marks per census (test). The stored-name
      divergence itself is NOT fixed — §6.
- [ ] B6 / B7 / B8 (`src/audio_ops.rs`,
      `src/graph_keys.rs`, `src/draw_ops.rs`): every
      mutator their census rows name follows its
      verdict, with tests in each file.
- [ ] B9 (`src/mcp_tools.rs`): handlers that wrote
      project state directly are rerouted through the
      core ops or snapshot per census; **no registry
      change from this file** — its tool count is 230
      before A4's splice and 235 after, with B9 adding
      none (census script is the evidence).

**End-to-end pins (B10, `tests/wave13_e2e_dirty.rs`, NEW)**
- [ ] The suite exists and runs in CI (the CI log lists
      its test binary — a suite that never executes is
      not proven, per the Wave 11 fail-fast lesson).
- [ ] Through real MCP dispatch: each fixed family
      reports dirty / advances the autosave-visible
      count; undo-step counts match the census verdicts
      exactly (snapshot rows +1 undo, `mark_dirty` rows
      +0); refusals spend nothing — no dirty, no undo,
      no state change.

---

## 3. Squad C — fidelity remainders

**S10, both exporters (C1 `src/svg.rs`, C2 `src/export.rs`)**
- [ ] SVG half: heads, rings and polys carry the node's
      `style.opacity` (composed with figure opacity by
      the rule C1's tests state) as opacity attributes —
      test with a node at opacity 0.5 on a figure at
      opacity 1.0 and on a faded figure.
- [ ] Raster half: the head/ring/poly pass
      (`src/export.rs:344–355` at launch) no longer
      hard-overwrites colour — node opacity composes
      with the background fade; a test renders a
      0.5-opacity node's head and asserts the pixel
      differs from the 1.0 render in the composed
      direction the test computes.
- [ ] S10 is closed only when BOTH boxes above are
      ticked; C9's addendum (§3, last item) must say
      exactly that, no more.

**SVG z-order (C1)**
- [ ] In `write_frame_svg` output (`src/svg.rs:107`),
      stroke elements emit AFTER the figure elements
      and media stays LAST (`append_media` call,
      `src/svg.rs:573` at launch, remains the final
      emission) — raster order. Pinned by an ordering
      test over a project carrying a figure, a stroke
      and a sprite: string-index assertions on the
      emitted document.
- [ ] Byte changes are deliberate: the SVG determinism
      tests pass with the new order, and no other
      element's bytes moved (the close-out diff of the
      determinism fixtures is the evidence).

**Stage pressured strokes (C5 `src/paint.rs`, C3 `src/rune_sec/stage_layers.rs`)**
- [ ] `paint::pressured_stroke_pieces` exists: the
      shared per-segment (a, b, width) construction
      for baked-width strokes.
- [ ] Its averaging rule is pinned AGAINST the
      raster's: one fixture with known per-point
      widths yields the same per-segment widths from
      `pressured_stroke_pieces` as `src/export.rs`'s
      pressured pass computes (endpoint average) — a
      test names both sides of the comparison.
- [ ] The stage uses it: `stage_layers.rs` builds
      pressured strokes from the shared pieces (grep
      hit in production code), and the pre-wave
      flat-width construction for pressured strokes is
      gone.
- [ ] Non-goal held: spray/scatter dab radius still
      uses the flat stroke width on every surface
      (Wave 12's standing note) — C5's tests or docs
      pin that nothing changed there.

**Figure-card thumbnails (C4, `src/rune_sec/figures_panel.rs`)**
- [ ] Thumbnails build through
      `paint::styled_segment_pieces` (grep hit in
      production code), at parity with the Wave 12
      library thumbnails; any local per-node
      construction it replaces is deleted, not left as
      a second path.
- [ ] The documented ring doctrine stands: ring prims
      still carry the figure colour, not the head
      node's override (the doc line and its test both
      survive).

**Support pins (C6 `src/model.rs`, C7 `src/frame_swap.rs`, C8 `src/live_mcp.rs`)**
- [ ] C6: serde round-trip pins for `sprites` and
      `texts` exist and pass; `InstanceKey::sample`
      edge tests cover all four cases by name — empty
      keys → `None`, single key → that key at any
      frame, before-first → first key, after-last →
      last key.
- [ ] C7: the frame insert/delete verdict for
      sprite/text instance keys is pinned either way —
      keys shift with their frames (test proves it) or
      the non-shift is documented in `frame_swap.rs`
      as house behaviour and carried into §6.8's
      remainders by name. No third outcome.
- [ ] C8: the live-MCP health body still DERIVES its
      tool count from the live surface — after the
      splice the headless live-MCP probe's transcript
      (§7) reports the new total (288), not a constant;
      drain attribution covers the new sprite/dirty
      write paths (test or probe line names them).

**Sprite e2e (C10, `tests/wave13_e2e_sprites.rs`, NEW)**
- [ ] The suite exists and runs in CI (binary listed
      in the CI log); through real dispatch it pins
      list / add / set_key / remove, the one-undo
      contract per mutating tool, and the refusal
      shapes. If `WAVE13-GAP` markers remain at
      close-out, each one names a tool the census
      proves unregistered — otherwise the markers are
      flipped strict per the Wave 12 B10 pattern.

**Docs (C9, `docs/STAGE_STYLE.md`)**
- [ ] A dated Wave 13 addendum records the C1/C2/C3
      flips and the S10 / S21 status lines, and every
      claim in it traces to a ticked box above; the
      ACCEPTANCE S10/S21 pointers agree with it.

---

## 4. Squad D — pixel-proof + scenes

**Scenes (D1, `src/rune_sec/scenes.rs`)**
- [ ] `scenes()` (`src/rune_sec/scenes.rs:988`) returns
      **28** scenes (gate check 5 derives the count):
      `library-populated` and `sprites-populated` are
      registered by those exact names.
- [ ] `layout_assertions()` (`src/rune_sec/scenes.rs:1245`)
      is sequential **1..=31** with no gaps or
      duplicates (gate check 8); fact **30** asserts
      the library-populated state's row/thumbnail keys,
      fact **31** the sprites-populated state's panel
      keys.
- [ ] `library-populated` seeds its entries in memory
      per D5's Wave 12 spec (a styled entry + a
      mini-copy), so the library renders real rows with
      real thumbnails without touching `./projects`.
- [ ] `sprites-populated` places at least one sprite
      instance and mounts the Sprites panel in the
      workspace E8 names — the tree dump is the
      evidence, and it must agree with §5's gating box.

**Headless (D2, `.github/workflows/headless.yml`)**
- [ ] Both new scenes are in the plain tree loop; the
      snap probes are named exactly
      `snap-library-populated.png` and
      `snap-sprites-populated.png`; the xvfb fallback
      loop follows the standing subset rule (gate
      check 8 forward + reverse coverage green is the
      proof — no scene in the workflow is unregistered,
      no registered scene is missing).

**Snap tests (D3, `src/rune_sec/snap_tests.rs`)**
- [ ] Round 14 exists: the sprite panel flow test
      (add via panel keys → the instance exists in the
      core; delete via panel keys → gone) and the
      library-thumbnails-present assertion in the
      library-populated state, both passing in CI.

**Follow-through (D4 `src/rune_sec/library_panel.rs`, D5 `src/rune_sec/menus.rs`, D6 `scripts/overflow_scan.py`)**
- [ ] D4: the thumbnail-row viewport behaviour
      (LIST_HEIGHT / scroll follow) is adjusted with
      **keys unchanged** — the gate's key-diff against
      HEAD shows zero lost and zero renamed in
      `library_panel.rs` — and a test pins the follow
      behaviour.
- [ ] D5: missing-file recents entries render dimmed
      and inert while present-file entries do not
      (menus.rs test pins both halves); entries stay
      visible — removal is a fail.
- [ ] D6: the overflow scanner's self-tests include
      fixtures for both new tree shapes (sprites-panel
      rows, library rows); scan logic changed ONLY if
      a fixture proved a blind spot, and the fixture +
      fix are named in D6's report either way.

**Gate + census upkeep (D7 `scripts/phase_j_checks.py`, D8 `scripts/mcp_census.py`)**
- [ ] D7: gate docstring/census lines updated for Wave
      13 with counts still derived live (no hardcoded
      totals); any NEW check fails loudly on the
      entering tree for the reason its docstring states
      — the integrator verifies by running it against
      `3f04667`-era behaviour conceptually: the check
      names what it catches.
- [ ] D8: the census header tells the truth after A4's
      splice (registry 235); the script's PASS line and
      the gate's check 9 agree.

**Proof ledger (D9, `docs/wave13/PIXEL_PROOF.md`, NEW)**
- [ ] The ledger exists: scene-table deltas (26 → 28),
      the snap list including both new snaps, and the
      gap list with **library thumbnails' pixel proof
      as gap #1 entering the wave**. At close-out the
      integrator's snap read either closes gap #1 or
      carries it by name into §6.8 — the ledger records
      which.

---

## 5. Squad E — QA + parity

**Strip read path (E1, `src/rune_sec/status_strip.rs`)**
- [ ] The dirty dot reads exactly
      `edits_since_autosave() > 0` (the doctrine at
      `status_strip.rs:912`, read at :953), and a test
      pins the strip's dirty state flipping after a
      mark_dirty-only edit (a figure rename) — a state
      the entering tree could not show.

**Recovery-note audit (E2, `src/rune_sec/project_tools.rs`)**
- [ ] Every `write_recovery` success path in the GUI
      save flow calls `note_recovery_written`: the two
      Wave 12 sites (`project_tools.rs` save-named
      path, `home_panel.rs` toggle-open path) plus
      every site E2's audit found, each named in E2's
      report; grep pairs each `write_recovery(` call
      with its note call. An unnoted success path is a
      fail.

**Palette (E3, `src/rune_sec/command_palette.rs`)**
- [ ] Palette is exactly **135** (132 + 3): commands
      `add-sprite`, `remove-sprite`, `next-sprite`
      exist, forward to A1's core ops (not panel
      closures — the fn-pointer targets are the
      evidence), and the in-file count pin reads 135;
      gate check 5 derives 135.

**Style-panel extraction (E4, `src/rune_sec/inspector_style.rs`)**
- [ ] The closure-only controls (gradient swatches,
      RGB steppers, recent colours) are named fns after
      E4, and panel behaviour is unchanged: the panel's
      pre-existing tests pass unmodified, and new pins
      cover the extracted fns. A behaviour change
      smuggled into the extraction is a fail.

**Clip-media audit (E5, `docs/wave13/CLIP_MEDIA.md`, NEW)**
- [ ] The audit exists and states the entering fact
      with its source: movieclip-def media
      (`clips[].def`) is stored and serialised but
      NEVER rendered (Wave 12 B8); it names the
      sampling point where rendering should read it,
      gives per-format verdicts, and is a SPEC — any
      claim in it that rendering was fixed this wave
      is a fail.

**Shell oddity (E6, `src/shell.rs`)**
- [ ] Resolved one of two checkable ways: the
      root-clone tests pass with the cause named in
      E6's report, OR the file's docs pin why they fail
      and that the failure pre-dates this wave. The
      integrator checks the CI log's `shell` test
      results against whichever claim E6 made.

**Toasts (E7, `src/rune_sec/toasts.rs`)**
- [ ] Sprite and dirty-family mutations through live
      MCP surface honest toasts, per the Wave 12 E7
      pattern: a test drives an MCP sprite add and a
      mark_dirty-only edit and asserts the toast
      family each earns (and that the dirty-only edit
      earns no mutation toast it didn't make).

**Workspace gating (E8, `src/rune_sec/workspaces.rs`)**
- [ ] The Sprites panel's host workspace is named in
      `workspaces.rs` and its gating test; per-workspace
      panel counts are updated truthfully — exactly one
      workspace's count grows by the panel, unless
      E8's report states a different arrangement and
      D1's tree (§4) agrees with it.

**Progress doc (E9, `docs/REDESIGN_PROGRESS.md`)**
- [ ] §6.8 skeleton exists in the §6.7 shape: scope,
      entering counts (the baseline above), the two
      exclusions stated, and TODO-AT-CLOSE blocks for
      Delivered / Verification / Closing census /
      Honest remainders — placeholders, not pre-written
      verdicts.

**Honesty sweep (E10, `docs/wave13/HONESTY_SWEEP.md`, NEW)**
- [ ] Every key, button and tool this wave adds —
      `sprite-*` keys, the five `sprite_*` tools, the
      three palette commands, the `xc-sprite-note`
      family — is traced in the sweep to a real
      handler; any dead control found is NAMED in the
      sweep. A control the sweep certifies but the
      gate census cannot find (or vice versa) is a
      fail.

---

## 6. Exclusions diff guard (integrator, on the wave diff)

- [ ] No project-swap undo/redo semantics change: the
      diff's only undo/redo behavioural delta is B2's
      redo `selected_figure` clamp; load / `.piv`
      import / New / open clear-behaviour is byte-
      identical in behaviour (their fns' diffs are
      empty or comment-only).
- [ ] No accent change: the diff touches no accent
      token or accent call-site colour (the upstream
      `#4392F7` carrier and the app token are both
      untouched; a grep of the diff for accent hexes
      comes back empty).
- [ ] No stored-name divergence fix, no waveform
      badge, no MCP-save `write_recovery` change: each
      named behaviour's diff is empty.

---

## 7. Integrator re-run list (close-out)

**Gate + censuses**
- [ ] `python3 scripts/phase_j_checks.py`, full
      foreground run read from the top: **9/9 PASS**
      (or 10/10 if D7's new check landed — its failure
      mode on the entering tree recorded either way).
- [ ] `python3 scripts/mcp_census.py`: registry
      **235**, io **9**, built-ins **44**, total
      **288**, unregistered **0**; gate check 9 agrees
      and `docs/MCP.md` syncs both directions.
- [ ] Counts re-derived against the entering baseline:
      MCP 283 → **288**; palette 132 → **135**; scenes
      26 → **28**; layout facts 29 → **31**; help
      **61 → 61**; keys 923 → ______ with **zero
      lost**; icons 163; i18n 99 × 28; tests 3,156 →
      ______ (CI's exact figure, not a rounding).

**Absence greps that must flip**
- [ ] `src/sprite_ops.rs`, `src/stage_sprites.rs`,
      `src/rune_sec/sprites_panel.rs`: absent at
      launch, present now.
- [ ] `mark_dirty` hits in `src/editor.rs` and in each
      B-family file its census rows name.
- [ ] `pressured_stroke_pieces` hits in `src/paint.rs`
      (definition) and `src/rune_sec/stage_layers.rs`
      (production use).
- [ ] `styled_segment_pieces` hits in
      `src/rune_sec/figures_panel.rs` production code.
- [ ] The entering wordings MISS: scene_panel's
      exports-only sprite note; help_panel's "cannot
      be placed or edited" Sprites wording.

**Greps that must stay absent**
- [ ] No second styled-segment or pressured-stroke
      construction reintroduced in `figures_panel.rs`,
      `library_panel.rs` or `stage_layers.rs` (one
      generator per construction, as Wave 12 left it).
- [ ] No bitmap-sprite rendering claim for the stage
      anywhere in `docs/SPRITES.md` or the panel code.

**Run evidence (fill at close-out)**
- [ ] CI run ____________ on sha ____________:
      **______ tests passed, 0 failed** across all
      binaries — including `wave13_e2e_dirty` and
      `wave13_e2e_sprites` listed by name in the log.
- [ ] Headless run ____________: every probe exit 0;
      live-MCP probe **PASS** with the health total
      reading **288**; overflow scan **CLEAN on ______
      trees** (denominator recorded, expected 31);
      snaps pixel-read: `snap-library-populated.png`,
      `snap-sprites-populated.png`, plus the standard
      set, copied to `your_files` as
      `stykanimate-wave13-*.png`.
- [ ] Docs finalised: §6.8 TODO-AT-CLOSE blocks filled
      from the evidence above; `docs/SPRITES.md`'s
      matrix matches shipped behaviour; `docs/MCP.md`
      at 288; C9's STAGE_STYLE addendum verified
      against the C1/C2/C3 landings; the §8 verdict
      below written.
- [ ] Carried, unsettled, stated in the user report:
      sound remains user-verified only; the five
      standing user decisions (project-swap undo,
      white-on-accent, two New-project results, C5
      waveform, MCP-save `write_recovery`) untouched.

---

## 8. Close-out verdict (integrator fills at close-out)

TODO-AT-CLOSE: what actually landed per squad against
§§1–5, the run evidence from §7, the closing census,
and the honest remainders — each remainder either
closed by a ticked box above or carried by name into
§6.8.
