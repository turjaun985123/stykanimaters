# Wave 14 — acceptance checklist (E5)

The wave's executable close-out checklist, modelled on
`docs/wave13/ACCEPTANCE.md`. Every item is a checkable
assertion with its evidence source named: a test that must
exist and pass, a grep that must hit or miss, a count the
gate derives, or a document whose claim must trace to one
of those. A box is ticked only on that evidence — an
agent's report is a claim, and this list is how the
integrator catches a wrong one.

Scope, from the roster: **A** total parity — every
EditorCore op, palette command and panel action is
tool-reachable or on a named exclusion list; **B** shell
control — the live endpoint drives the GUI shell itself
(workspace, transport, view, selection, theme, panels);
**C** clip-def media end-to-end (Wave 13 E5's anchored
spec, `docs/wave13/CLIP_MEDIA.md` §6); **D** live
transport hardening; **E** proof and gates, including
the MCP-only full-project build.

**Entering baseline** (Wave 13 close,
`docs/REDESIGN_PROGRESS.md` §6.8): MCP **288 tools**
(235 registry + 9 io + 44 built-ins, per
`scripts/mcp_census.py`); **3,319 tests** (CI
42562084573 on `ac47b99`, GREEN — 1,337 lib + 1,911 bin
+ 71 integration); palette **134**; keys **940**, zero
lost; scenes **28**; help **61 rows**; layout facts
**31**; icons **163**; i18n **99 × 28**; gate **9/9**
(`scripts/phase_j_checks.py`). Gate note: if E2's parity
check (§5) lands as a tenth gate check, the closing gate
reads **10/10**; §7 records which.

**Exclusions guard** — the wave must NOT settle the
five standing user decisions: project-swap undo/redo
semantics, accent colours, MCP-save `write_recovery`
behaviour, the waveform badge, and the stored-name-
after-rename divergence. §6 makes each one a diff check.

---

## 1. Squad A — parity census closed

**The ledger (A1, `docs/wave14/PARITY_CENSUS.md`, NEW)**
- [ ] The census exists and rows every `pub` EditorCore
      op across the `*_ops.rs` files, `src/editor.rs`
      and `src/projects.rs`, all **134** palette
      commands, and the panel actions A1's method
      enumerates — each row marked COVERED (tool named)
      or GAP at launch.
- [ ] At close-out, **no row still reads GAP**: every
      launch gap is either flipped to COVERED with the
      shipped tool's name, or moved to the census's
      exclusion list with a named reason (shell-only →
      Squad B, dialog-bound → B9's refusal list, or a
      §6 standing exclusion). A row closed with neither
      is a fail.
- [ ] Completeness is re-derivable: the integrator
      re-greps the op surface (`pub fn` on `EditorCore`
      across the ops files) and the palette's command
      table, and every hit is a census row. An unrowed
      op or command fails the census, whoever wrote it.
- [ ] A10's palette section: all 134 commands mapped to
      a registry tool, a Squad B shell tool, or a named
      exclusion — the three sets partition the 134 with
      no command in two sets and none unmapped (count
      the rows).

**Handler fill (A2–A9, domain files + integrator splice)**
- [ ] A2 (`src/figure_ops.rs`, frame ops): every handler
      A2's spec names exists as a `fn tool_*` in the
      domain file (grep), and after the splice each is a
      `Tool` entry reachable through dispatch — proven
      by the domain's tests calling through the router,
      not the handler fn directly.
- [ ] A3 (`src/draw_ops.rs`): the nib / spray / scatter
      family is complete — each brush control the Draw
      panel exposes has a tool in the census's draw
      rows, and a test drives a scatter stroke through
      dispatch and reads the stroke back.
- [ ] A4 (`src/graph_keys.rs` / `src/graph.rs`): graph
      parity handlers exist for every graph-editor
      action the census rows; a test sets and reads a
      key's easing through dispatch.
- [ ] A5 (`src/fx_ops.rs` / `src/fx_edit.rs` /
      `src/particle_ops.rs`): FX and particle parity
      handlers exist per census rows, including the
      clip-def-adjacent FX clip family — spot-grep: for
      three census rows chosen at random, the named
      handler greps in production code and its `Tool`
      entry greps in the registry.
- [ ] A6 (`src/audio_ops.rs` / `src/audio_edit.rs`):
      the trim / fade / gain / normalise family is
      complete per census rows; `add_audio_clip`'s
      standing refusals (nonexistent file, non-WAV) are
      unchanged — their Wave 11 tests still pass
      unmodified.
- [ ] A7 (`src/fig3d.rs`): the Wave 11 fig3 tool set is
      completed per census rows — every 3D panel action
      (enter/exit, camera, rename, per census) has a
      tool or a named exclusion.
- [ ] A8 (`src/media.rs` / `src/sprite_ops.rs` /
      `src/projects.rs`): the residual rows — clipboard,
      onion-skin toggles, project housekeeping — are
      covered or excluded per census; no A8 handler
      duplicates a Wave 12/13 tool name (census: zero
      duplicates, §7).
- [ ] A9 (scene settings in `src/editor.rs` handlers):
      weather kind/cycle, the full camera-key set,
      onion-skin ranges and loop/easing defaults each
      have a tool per census rows; weather tools write
      through the same ops the Scene panel uses (the
      panel's dirty-marking pins from Wave 13 still
      pass — the tools must not bypass `mark_dirty`).

**Standing contract on every new A tool (with §5, E10)**
- [ ] Each mutating A tool spends exactly one undo step
      and each refusal spends nothing — asserted per
      domain by that domain's dispatch tests (the
      census rows name the test file for each family).

---

## 2. Squad B — shell control is live

**Design + plumbing (B1 `docs/wave14/SHELL_CONTROL.md`,
B2 `src/shell_control.rs`, NEW)**
- [ ] The design doc exists: the `ShellCommand` enum,
      the request queue, the drain site in the shell's
      poll loop, the result return path, the stdio
      behaviour and the security posture (loopback, no
      auth — the same posture the health document's
      `auth: "none"` states) are each named. Every tool
      name in its tool table is the name §2 checks
      against the registry — a table that disagrees
      with the shipped registry fails this item.
- [ ] `src/shell_control.rs` exists with the queue,
      command enum and result slots; the integrator's
      splice into `src/main.rs` / `src/live_mcp.rs` /
      `src/mcp.rs` follows B2's spec (the splice diff
      is the evidence — B2 itself touched no hot file).
- [ ] The drain is bounded like the core drain: a
      per-poll cap exists in the shell drain (grep the
      cap beside the drain fn) and timed-out shell
      requests are discarded, never executed late — a
      test pins the discard, mirroring the Wave 12/13
      core-queue pins.

**The shell tools (B3–B9 handlers, integrator splice)**
- [ ] Every tool in B1's table is a live registry
      entry: the closing census (§7) lists the shell
      family with zero unregistered handlers, and
      `tools/list` over the live endpoint contains
      each name (the headless probe, below, samples
      at least one; the integrator spot-calls three
      more by hand from the probe transcript pattern).
- [ ] B3: `set_workspace` switches the shell's workspace
      and `get_shell_state` reports it back — B10's
      Runtime test drives both through the real queue
      and asserts the shell state, not just the reply.
- [ ] B4: the transport tools (play / pause / stop /
      scrub, as B1 names them) move the shell's REAL
      playback state: a Runtime test asserts the
      playhead / playing flag in shell state after the
      call. A tool that only mutates the core's frame
      (the entering drain's reach) is a fail.
- [ ] B5: view tools change view zoom / fit / centre
      ONLY — a test asserts the camera keys and the
      undo depth are untouched by a view call (Wave 5
      doctrine: view navigation never authors camera).
- [ ] B6: selection tools write the shell selection
      fields, including `sprites_panel.selected`:
      a test selects a sprite through dispatch and
      asserts both the shell field and the stage's
      selected-sprite corner marks' input (the
      `StageOverlays` selection source).
- [ ] B7: theme + settings tools round-trip: set then
      read back through dispatch, with the value
      landing in `LayoutPrefs`-backed state (a test
      reloads prefs and sees it). No accent value is
      settable — §6's accent diff check covers the
      code; a shell tool exposing accent fails here.
- [ ] B8: panel tools collapse / expand a section and
      the timeline within the workspace gating rules —
      a test asserts a section hidden in the current
      workspace stays unreachable (the gating tables in
      `src/rune_sec/workspaces.rs` are the contract).
- [ ] B9: non-dialog shell actions (recent-project open
      and any B9 names) work through dispatch; every
      dialog-bound action returns an honest named
      refusal — a test calls each one and asserts the
      refusal names the dialog it would need. A silent
      no-op or a fake success is a fail.

**stdio refusal (B1/B2 contract)**
- [ ] Over stdio (`src/bin/mcp_server.rs`), calling any
      shell tool returns a clean refusal — an `isError`
      result or protocol error whose text states shell
      control needs the live editor — never a panic,
      never a queued-forever hang, never a success.
      Pinned by a test in B10's suite or the stdio
      dispatch tests; the refusal wording is quoted in
      `docs/MCP.md` (§5, E3) and the two agree.

**Headless live probe exercises a shell tool
(integrator MUST make this true)**
- [ ] The Q4 live probe in
      `.github/workflows/headless.yml` (the Python
      probe whose transcript is `mcp-probe.txt`) is
      extended to call at least one shell tool against
      the real editor — `get_shell_state` or B1's
      read-only equivalent — and asserts the reply
      reports live shell state (e.g. the workspace the
      probe's editor is actually in). The closing
      headless artifact's `mcp-probe.txt` contains the
      PASS line for that call (§7).
- [ ] The probe's `TOOL_FLOOR` (140 at launch) is
      raised past the closing total's floor or left
      with its comment updated — a floor the closing
      surface only just meets is a stale claim; the
      transcript's tools/list count equals the health
      count at the NEW total.

**Shell docs + tests (B10)**
- [ ] B10's Runtime-level shell-control suite exists
      and runs in CI (its binary/test names listed in
      the CI log — a suite that never executes is not
      proven, per the Wave 11 fail-fast lesson).
- [ ] The shell section of `docs/MCP.md` (B10's draft,
      E3's audit) documents every shell tool with its
      live-only nature stated; gate check 9's
      two-direction sync passes at the new total (§5).

---

## 3. Squad C — clip-def media renders end-to-end

Entering truth (Wave 13 E5, `docs/wave13/CLIP_MEDIA.md`):
clip-def media (`Project.clips[].def` node media) is
stored, serialised and census-counted but sampled by NO
renderer; the fix is the anchored spec, §6 of that doc —
one funnel change plus one raster gate. All items below
trace to it.

**The funnel (C1, `src/media.rs`)**
- [ ] `media_items` (`src/media.rs:131`) resolves node
      media per-node: the instance's own
      `fig.def.nodes[ni].media` first, else the clip
      def's node at the same index when `fig.clip`
      names a live clip (the spec §6.1 fallback — grep
      the `clips.get` read inside the figure loop).
- [ ] The spec §6.6 funnel tests exist and pass: clip-
      def-only image media yields exactly one
      `MediaItem::Image` at the segment quad with alpha
      = instance `fig.opacity` × CLIP-node
      `style.opacity`; clip-def text yields one
      `MediaItem::Text` with the clip payload and the
      `clip_node.color.unwrap_or(fig.color)` fallback.
- [ ] No double-draw: identical media on both defs
      yields exactly ONE item for the node, and
      differing media yields the INSTANCE's item
      (instance wins) — one test each.
- [ ] Divergence safety: clip def longer and shorter
      than the instance def — extra indices skipped,
      missing clip tolerated, no panic (spec §6.3);
      hidden figures stay skipped wholesale.

**Stage (C2)**
- [ ] A clip-sourced item produces the stage's media
      placeholder for the instance's segment: a test in
      the stage test family (C2 names it) builds the
      clip-def-only project and asserts the placeholder
      prim/outline appears where the figure-def path
      would put it. The placeholder honesty rule
      stands: no claim anywhere that the stage draws
      the real PNG (grep of C2's docs/comments for a
      bitmap claim is a fail).

**Raster (C3, `src/export.rs`)**
- [ ] The `has_media` gate (`src/export.rs:758` at
      launch) is extended exactly per spec §6.4: it
      reads `project.clips` def node media (grep hit
      inside the gate fn).
- [ ] A raster pixel test: a project whose ONLY media
      is clip-def media renders the image in
      `render_frame_at` — asserted on pixels (the
      media colour present in the frame), not on the
      funnel alone. This is the test that proves the
      gate extension, per spec §6.6.

**SVG (C4, `src/svg.rs`)**
- [ ] An SVG emission test: clip-def-only media appears
      in `write_frame_svg` output (the media element
      for the clip-sourced item is present in the
      emitted document, after the figure elements —
      media stays last, per the Wave 13 z-order pin).

**MCP parity (C5)**
- [ ] C5's audit verdict is written down (in the census
      or `src/media.rs` docs): which MCP path attaches
      media that the clip fallback renders — the clip
      session path or a filled gap — naming the tools.
      An unnamed path is a fail.
- [ ] The named path works end-to-end: C10's suite (§3,
      last item) attaches via MCP only and the export
      shows it. Any gap C5 filled has its handlers in
      the clip/media domain files and its `Tool`
      entries spliced (census, §7).

**Census + refusal honesty (C6, `src/media.rs`)**
- [ ] `asset_references` / `remap_asset_refs` are
      UNCHANGED in behaviour (spec §6.5): the entering
      pins still pass unmodified —
      `asset_remove_refused_while_any_reference_kind_points_at_it`
      (:1683) and
      `asset_remove_remaps_clip_def_references_and_info_counts_them`
      (:1767). The refusal is now HONEST: the asset it
      protects renders (§3 items above) — that, not a
      census edit, is this item's change.
- [ ] C6's updated census table (the `media.rs` module
      docs' surface table) names the clip fallback as
      the sampling path for clip-def rows — grep the
      table row for `clip` + `media_items`.

**Session interplay + thumbnails (C7, C8)**
- [ ] C7's tests exist: clip media renders across the
      clip's frames (an item sampled at two clip-frame
      positions follows the clip pose), and one clip
      def reused by two instances renders on both.
- [ ] C8: thumbnails follow the funnel — a project
      whose only media is clip-def media produces a
      Home/library thumbnail containing it (test
      through `projects::thumbnail` or
      `render_frame_at`, per C8's spec note); surfaces
      the spec exempts are named in C8's report, not
      silently skipped.

**E2E (C10, `tests/`)**
- [ ] A `tests/wave14_e2e_*` clip-media suite exists
      and runs in CI (binary listed in the CI log):
      through real MCP dispatch only, it builds a
      clip carrying media, attaches/reads it back per
      C5's named path, and exports — asserting the
      media is present in the export per C3/C4's
      evidence forms.

**Docs flips (C9)**
- [ ] `docs/wave13/CLIP_MEDIA.md` gains a landed note
      (dated, Wave 14) flipping its verdict to the
      shipped behaviour, with the spec deviations — if
      any — named; `docs/MCP.md` rows for any C5 tool
      exist (§5 sync); any `docs/SPRITES.md` cross-ref
      C9 touched still greps true against source.

---

## 4. Squad D — transport hardening

**Busy-port fix (D1, `src/live_mcp.rs` + settings splice)**
- [ ] `LAST_START_ERROR` and `last_start_error()` exist
      in `src/live_mcp.rs` (grep hits — both ABSENT at
      launch): a failed bind records the error, a
      successful start clears it.
- [ ] A test proves it: with the port already bound, a
      start attempt leaves `last_start_error()` naming
      the failure (address-in-use in substance); after
      a successful start it reads empty/none.
- [ ] The settings note shows it: the settings card's
      Live MCP row renders the recorded start error
      when one exists (the integrator's splice of D1's
      spec into `src/rune_sec/settings_popover.rs`),
      pinned by a settings test — entering behaviour
      (a failed start invisible in the UI) misses on
      the closing tree.

**stdio ↔ live parity harness (D2, `tests/`)**
- [ ] The harness exists and runs in CI: ONE scripted
      tool sequence is driven against BOTH the stdio
      dispatch (`src/bin/mcp_server.rs` path) and the
      live-style dispatch, and the results are diffed —
      the suite asserts equality per step, with the
      shell-tool refusal (§2) as the only sanctioned
      divergence, named in the suite.
- [ ] The sequence covers at least: initialize,
      tools/list, one read, one mutation, one refusal,
      one unknown tool (−32602 both sides).

**Queue / backpressure (D3, `src/live_mcp.rs`, `src/mcp.rs`)**
- [ ] D3's audit is written against source: the bounded
      queue capacity and the timeout-discard rule are
      cited by constant/fn, and the Wave 12/13 pins for
      both still pass. Any gap D3 found is either fixed
      with a new pin or carried by name into §6.9's
      remainders — the audit claiming a gap the
      remainders don't carry is a fail.

**Health endpoint (D4, `src/live_mcp.rs`)**
- [ ] `health_body` still derives every field: `server`,
      `version`, `transport`, `live`, `endpoint`,
      `port`, `enabled`, `auth: "none"`, and `tools`
      read from `all_tools()` (`src/live_mcp.rs:974`
      at launch) — the entering pin
      `health_tool_count_is_the_served_tool_count`
      passes at the NEW total.
- [ ] The `shell_control` field lands truthfully or
      not at all: IF B's plumbing provides the atomic
      availability flag the entering comment block at
      `health_body` describes, the field exists, reads
      `true` when the shell drain is wired, and that
      comment block is rewritten (its "Deliberately
      ABSENT" wording **misses** on grep); IF the flag
      did not land, the field stays absent and §6.9
      carries it by name. A hardcoded `true` with no
      flag source is a fail.

**Robustness (D5, `src/mcp.rs` + tests)**
- [ ] The robustness suite exists and runs in CI: each
      of malformed JSON, wrong-typed arguments, an
      oversized payload, and an unknown method/tool
      yields a clean protocol error (−32602 for unknown
      tool, per the standing contract) with NO panic —
      one test per class, over the shared router so
      both transports inherit it.
- [ ] Any fix D5 made in `src/mcp.rs` is confined to
      error paths: the happy-path dispatch tests from
      Waves 11–13 pass unmodified.

**Semantics + handshake + lifecycle (D6, D7, D8)**
- [ ] D6's batch + notification audit is written: batch
      handling and the notification rule (no id → no
      result; HTTP 202 + empty body on live) match the
      implementation on BOTH transports, each claim
      citing the dispatch code or a pin.
- [ ] D7: the stdio binary's initialize handshake
      returns the same server name + version as the
      live `initialize` / health identity — a test or
      harness step (§4, D2) compares them.
- [ ] D8: lifecycle tests exist for the live transport:
      a client disconnecting mid-request and a partial
      write each leave the listener serving the next
      request (the test's second request answers).

**Settings UX truth (D9, `src/rune_sec/settings_popover.rs`)**
- [ ] Settings pins cover: the endpoint text (the real
      `live_mcp::endpoint()`), the D1 error note
      (§4, first item), and the toggle semantics — the
      toggle reads the bridge (`live_mcp::is_enabled`)
      and persists the RESULTING state to
      `LayoutPrefs.live_mcp`. Text the code contradicts
      is a fail.

**Transport chapter (D10, `docs/MCP.md`)**
- [ ] The transport chapter is rewritten to landed
      truth: ports, the busy-port behaviour + its
      settings note, queue bounds, the stdio/live
      divergence list (shell tools only) — each claim
      traces to a ticked item in this section or §2.

---

## 5. Squad E — proof & gates

**THE MCP-only build (E1, `tests/`)**
- [ ] A `tests/wave14_e2e_*` suite exists and runs in
      CI (binary listed in the CI log): it creates a
      complete project using ONLY MCP dispatch — no
      direct `EditorCore` calls in the build path (grep
      the suite: core construction is fixture setup
      only, every project mutation goes through the
      router).
- [ ] The build covers the charter's family list:
      figures, frames, keys, graph, draw, fx,
      particles, audio, sprites, assets, 3D, clips,
      camera, weather — the suite names one dispatch
      per family (a family reachable only by a direct
      call fails §1's census too).
- [ ] It then exports PNG, SVG and a frame range via
      MCP and asserts each output exists and is sane —
      non-empty, and for the PNG/SVG the suite's own
      content assertion (pixel/element present), not
      existence alone.

**Parity gate (E2, `scripts/phase_j_checks.py`)**
- [ ] The gate gains a parity check: every op/command
      in A1's census is tool-covered or on the named
      exclusion list, derived live from source + the
      census — no hardcoded tool list.
- [ ] Its failure mode is PROVEN on the entering tree:
      E2's report (or the check's docstring) records
      the check failing against the Wave 13 tree for
      exactly the launch GAP rows, and passing on the
      closing tree. A check that has never been seen to
      fail is not evidence.
- [ ] The closing gate run (§7) reads **10/10** with
      the new check, or **9/9** with the parity check
      run and passing as a named standalone — §7
      records which, and the gate docstring agrees.

**MCP.md audit (E3, `docs/MCP.md`)**
- [ ] Every shipped tool is documented with its
      arguments; gate check 9's two-direction sync
      passes at the closing total (docs ↔ registry,
      neither direction has an orphan).
- [ ] The shell section (§2) and transport chapter (§4)
      are inside the same sync — a shell tool
      documented in prose but missing from the synced
      tool list is a fail.

**Help row (E4, `src/rune_sec/help_panel.rs`)**
- [ ] The Help MCP row states the full-control truth:
      core parity AND the shell tools, with the
      live-only nature of shell control stated; the
      in-file pin test asserts the new strings; the
      panel still holds **61 rows** (a genuinely new
      row, if any, is named in E4's report and the
      count item in §7 updated to match).

**Built-ins + io census (E9)**
- [ ] All **44** built-ins and all **9** io tools are
      reachable on BOTH transports, or each exception
      is named with its reason in E9's census section
      (in `scripts/mcp_census.py` output or a wave doc)
      — the entering split (44 + 9) is re-derived, not
      assumed.

**Contract re-verification (E10)**
- [ ] Across ALL tools this wave adds (A, B, C
      families): one-undo-step per mutation, unknown
      tool → −32602 "unknown tool: {name}", finiteness
      enforced incl. f32-cast overflow, refusal
      messages quoting the offending index/frame/value
      — re-verified as ONE pass (a suite or a census
      section naming the per-family evidence), not
      assumed from the family items above.
- [ ] Shell tools' contract exception is explicit:
      shell tools spend NO undo step (they mutate shell
      state, not the project) — E10's pass states this
      and B10's tests pin it (a workspace switch leaves
      the undo depth unchanged).

**Wave docs (E6, E7, E8)**
- [ ] E6: `docs/wave14/HONESTY_SWEEP.md` exists — every
      key, button and tool this wave adds is traced to
      a real handler; any dead control found is NAMED
      in the sweep. A control the sweep certifies but
      the census cannot find (or vice versa) is a fail.
- [ ] E7: §6.9 skeleton exists in
      `docs/REDESIGN_PROGRESS.md` in the §6.8 shape:
      scope, entering counts (the baseline above), the
      exclusions stated, TODO-AT-CLOSE blocks —
      placeholders, not pre-written verdicts.
- [ ] E8: any UI the wave changed (the settings note
      line, shell-state readouts) has its overflow/snap
      upkeep: the changed surfaces appear in a headless
      tree or snap the closing run scans (§7), or E8
      names why no new surface exists.

---

## 6. Exclusions diff guard (integrator, on the wave diff)

- [ ] No project-swap undo/redo semantics change: load
      / `.piv` import / New / open clear-behaviour is
      behaviour-identical; the diff's undo/redo deltas
      are confined to new tools' own snapshot discipline
      (§1/§5 pins).
- [ ] No accent change: the diff touches no accent token
      or accent call-site colour (the upstream `#4392F7`
      carrier and the app token both untouched; a grep
      of the diff for accent hexes comes back empty),
      and no shell tool exposes an accent value (§2).
- [ ] No MCP-save `write_recovery` change: the MCP save
      paths' recovery behaviour is diff-empty.
- [ ] No waveform-badge change: the deleted-file
      waveform behaviour (stale waveform vs "(missing
      file)" badge) is diff-empty.
- [ ] No stored-name divergence fix: `project_rename`'s
      stored-name behaviour is diff-empty (dirty-marking
      around it, landed in Wave 13, is not this).

---

## 7. Integrator re-run list (close-out)

**Gate + censuses**
- [ ] `python3 scripts/phase_j_checks.py`, full
      foreground run read from the top: **10/10 PASS**
      (with E2's parity check) or **9/9 PASS** plus the
      parity check passing standalone — recorded here:
      ____________.
- [ ] `python3 scripts/mcp_census.py`: registry
      ______ (entering 235), io **9**, built-ins **44**,
      total ______ (entering 288), unregistered **0**,
      duplicates **0**; gate check 9 agrees and
      `docs/MCP.md` syncs both directions.
- [ ] Counts re-derived against the entering baseline:
      MCP 288 → ______; palette **134** (any change
      itself censused in §1); scenes **28**; layout
      facts **31**; help **61** (or E4's named delta);
      keys 940 → ______ with **zero lost**; icons 163;
      i18n 99 × 28; tests 3,319 → ______ (CI's exact
      figure, not a rounding).

**Spot-greps per squad (the census's claims, sampled)**
- [ ] A: three PARITY_CENSUS rows chosen at random —
      each named handler greps in its domain file and
      each named tool greps in the registry.
- [ ] B: `src/shell_control.rs` present; every B1-table
      name greps in the registry; the stdio refusal
      text greps in the dispatch path B1/B2 name.
- [ ] C: the `clips.get` fallback greps inside
      `media_items`; `clips` greps inside the
      `has_media` gate; the §6.6 test names grep in
      `src/media.rs`'s tests.
- [ ] D: `LAST_START_ERROR` greps in
      `src/live_mcp.rs` (absent at launch); the
      "Deliberately ABSENT" health comment is either
      rewritten (flag landed) or §6.9 carries the field
      as a named remainder.
- [ ] E: the MCP-only suite's build path greps clean
      of direct core mutation calls outside fixture
      setup.

**Run evidence (fill at close-out)**
- [ ] CI run ____________ on sha ____________:
      **______ tests passed, 0 failed** across all
      binaries — including the wave's new suites listed
      by name in the log (B10 shell-control, D2 parity
      harness, D5 robustness, C10 clip-media e2e, E1
      MCP-only build).
- [ ] Headless run ____________: every probe exit 0;
      live-MCP probe **PASS** with the health total
      reading the closing ______ AND the shell-tool
      PASS line present in `mcp-probe.txt` (§2);
      overflow scan **CLEAN on ______ trees**
      (denominator recorded); snaps pixel-read and
      copied to `your_files` as
      `stykanimate-wave14-*.png`.
- [ ] Docs finalised: §6.9 TODO-AT-CLOSE blocks filled
      from the evidence above; `docs/MCP.md` at the
      closing total; `docs/wave13/CLIP_MEDIA.md`'s
      landed note written; PARITY_CENSUS has no GAP
      rows left; the §8 verdict below written.
- [ ] Carried, unsettled, stated in the user report:
      sound remains user-verified only (B4 moves
      playback STATE; audibility is not proven by any
      item here); the five standing user decisions
      (project-swap undo, white-on-accent, two
      New-project results, C5 waveform, MCP-save
      `write_recovery`) untouched, plus the stored-name
      divergence.

---

## 8. Close-out verdict (integrator fills at close-out)

TODO-AT-CLOSE: what actually landed per squad against
§§1–5, the run evidence from §7, the closing census,
and the honest remainders — each remainder either
closed by a ticked box above or carried by name into
§6.9.
