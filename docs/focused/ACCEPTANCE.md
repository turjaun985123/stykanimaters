# Focused Wave (post-Wave 14) — acceptance checklist (E3)

The wave's executable close-out checklist, modelled on
`docs/wave14/ACCEPTANCE.md`. Every item is a checkable
assertion with its evidence source named: a test that must
exist and pass in the closing CI run, a grep that must hit
or miss, a count the gate or census derives, or a document
whose claim must trace to one of those. A box is ticked
only on that evidence — an agent's report is a claim, and
this list is how the integrator catches a wrong one.

Scope, from the roster (`~/workspace/focused-wave-roster.md`):
**A** G5 figure-library entry rename gets a lib home, a
panel wrapper and an MCP tool; **B** G6 graph channel
selection and G7 the recent-colours list are decided from
source (tool what is shell state, declare the rest
honestly); **C** `set_weather`'s defaults are reconciled
to the panel presets from one source; **D** the
`timeline_collapsed` / `sidebar_visible` layout prefs are
wired for real and `shell_set_timeline` becomes a real
control; **E** proof — gate, docs, probe, this checklist.
11 agents, five squads. This wave is **not** "Wave 15"
(`docs/QA_WAVE15.md` is a different, older wave).

**Entering baseline** (Wave 14 close,
`docs/REDESIGN_PROGRESS.md` §6.9): MCP **366 tools**
(313 registry + 9 io + 44 built-ins, per
`scripts/mcp_census.py`); **3,519 tests** (CI
**42571128696** on `8f5bb54`, GREEN — 1,432 lib + 8
mcp_server bin + 1,985 bin + 94 integration); palette
**134**; keys **940**, zero lost; scenes **29**; help
**61 rows**; layout facts **31**; icons **163**; i18n
**99 × 28**; gate **10/10** (`scripts/phase_j_checks.py`);
headless **42571630518** (all 60 probes exit 0, live-MCP
probe PASS, overflow CLEAN on 32 trees). HEAD at launch:
`4af7d85`, the Wave 14 close-out commit.

**Exclusions guard** — the wave must NOT settle the five
standing user decisions (project-swap undo/redo
semantics, accent colours, MCP-save `write_recovery`
behaviour, the waveform badge, the stored-name-after-
rename divergence) and must NOT tool G3
(`cycle_onion_frames`, a dead field — a tool for it would
be a fake control). §6 makes each one a diff check.

---

## 1. Squad A — G5 figure-library entry rename

**The lib home (A1, `src/library_ops.rs` NEW +
`src/lib.rs`)**
- [ ] `src/library_ops.rs` exists and `src/lib.rs`
      declares `pub mod library_ops;` (grep both).
- [ ] The rename logic lives in exactly one place:
      `EditorCore::rename_library_entry` in
      `src/library_ops.rs`. A grep for `std::fs::rename`
      over `src/library_ops.rs` and
      `src/rune_sec/library_panel.rs` hits only in
      `library_ops.rs` — the panel performs no file
      rename of its own.
- [ ] The helpers the op uses — `is_builtin`,
      `sanitize_stem`, `name_taken`, `library_file_for`
      — are defined in `src/library_ops.rs`; the rename
      path calls those, not a second copy. (Panel-side
      helpers retained for the panel's *delete* path are
      not this item; a helper that duplicates the
      *rename* logic is a fail.)
- [ ] Sanitising is pinned by the lib tests:
      `"My Hero"` → `my-hero`, case folded, surrounding
      whitespace trimmed, `/` and `\` become `-` (a typed
      name can never escape `./library`), blank input
      sanitises to the empty string.

**The refusal matrix — every rule the panel had,
verbatim (A1/A3)**
Each row: the named input, the exact outcome, and the
no-side-effects assertion (entry keeps its old name and
slot; no library file is created, moved or overwritten).
The MCP form returns the panel's status text as the
`Err`; the strings below are the panel's own wording.
- [ ] **Built-in:** renaming an entry whose name starts
      `Built-in: ` fails with `"<old>" is built in and
      can't be renamed.` — and no filesystem access
      happens first (the refusal precedes any path use).
- [ ] **Empty:** a new name that sanitises to empty
      (`""`, `"   "`, or separators only) fails with
      `Type a new name for the library figure first.`
- [ ] **Same name:** a new name that sanitises to the
      entry's current stem succeeds with
      `renamed: false` and status `"<old>" already has
      that name.` — no file rename is attempted.
- [ ] **Name taken:** a stem another entry already
      carries fails with `A library figure named
      "<stem>" already exists.` The entry's own name is
      NOT a collision (the same-name row above wins).
- [ ] **Target file exists:** a stem whose
      `./library/<stem>.json` exists on disk but is not
      a loaded entry fails with `A file named
      "<stem>.json" already exists in the library
      folder.` — the existing file's bytes are
      unchanged after the call (no silent overwrite).
- [ ] **Filesystem failure:** renaming an entry whose
      backing file is missing fails with a status
      starting `Library rename failed for "<old>":`;
      the vec entry keeps its old name.
- [ ] **Out-of-range (tool form):**
      `figure_library_rename` with an index past the
      library end fails with `no library entry at index
      <n>`. (The panel wrapper's stale-index case is the
      panel's own contract — settled, silent — and is
      pinned by the pre-existing panel test
      `rename_saved_out_of_range_settles_silently`.)

**Happy path + reachability (A1/A3,
`tests/focused_e2e_library_rename.rs` NEW)**
- [ ] The e2e suite exists and drives the tool through
      MCP dispatch only (no direct handler calls in the
      build path): a rename returns
      `{ok: true, renamed: true, index, name, status}`,
      the entry keeps its slot under the new stem, the
      old `./library/<old>.json` is gone and
      `./library/<new>.json` exists with byte-identical
      contents.
- [ ] `figure_library_rename` appears exactly once in
      the registry (`src/mcp_tools.rs` `tools()`), its
      handler resolves to
      `library_ops::tool_figure_library_rename`, and
      gate check 9 / `scripts/mcp_census.py` count it
      with zero unregistered and zero duplicates (§7).
- [ ] It is a CORE tool, not a shell tool: it does not
      appear in `SHELL_TOOL_NAMES`, and a stdio dispatch
      reaches the handler (a refusal for a bad index
      names the index — it is not the shell tools'
      live-endpoint refusal sentence).

**Panel wrapper (A2, `src/rune_sec/library_panel.rs`)**
- [ ] `rename_saved` is a thin wrapper: its body calls
      the lib op and contains no refusal logic of its
      own (read the fn — one delegation, no `if` ladder
      re-deciding builtin/empty/collision).
- [ ] The pre-existing panel rename tests are present
      under their entering names and pass in the closing
      CI run: `rename_saved_refuses_builtins_without_touching_fs`,
      `rename_saved_out_of_range_settles_silently`,
      `rename_saved_rejects_empty_and_colliding_names`,
      `rename_saved_same_name_settles_without_fs`,
      `rename_saved_keeps_entry_when_file_rename_fails`.

**Bookkeeping — the file-operation contract**
- [ ] A successful rename, a same-name settle and every
      refusal spend **zero undo steps** and mark **no
      dirty edit**: the e2e suite asserts the undo
      depth and the dirty/edit counter are identical
      before and after each case. (This pins the
      landed source contract — a library rename is a
      file operation in the `pose_library_rename`
      class, and the panel rename it mirrors snapshots
      nothing. It deliberately supersedes the roster's
      A3 phrase "exactly-one-undo-step": a checklist
      that pinned both would be unpassable, and the
      integrator records the correction in §8 rather
      than silently ticking either.)

---

## 2. Squad B — G6/G7 shell-state decisions

**G6 — graph channel selection (B1,
`src/shell_tools_selection.rs`)**
The premise, checkable in the entering tree: the
selection is shell state — `State::graph_panel.selected_channel:
Option<usize>` (`src/main.rs`'s `State`,
`src/rune_sec/graph_panel.rs`), written by the panel's
own row presses and read by
`graph_panel::effective_channel_index`. It is the same
class as the sprite/stroke/particle/clip selections
Wave 14 tooled.
- [ ] `shell_select_graph_channel` exists in all four
      places a shell tool must: `SHELL_TOOL_NAMES`
      (`src/shell_control.rs`, exactly once), the
      registry stub (exactly once), the name dispatch,
      and a real handler in
      `src/shell_tools_selection.rs`.
- [ ] The handler writes the real field: a test builds
      shell state with a known channel list, calls the
      tool through the shell dispatch, and asserts
      `state.graph_panel.selected_channel == Some(<n>)`
      — the field itself, not only the reply.
- [ ] Validation precedes any write: an index at or
      past the live channel count
      (`core.project.graph.channels.len()`) is refused,
      the refusal quotes the offending index, and
      `selected_channel` is unchanged after the refusal
      (test-asserted).
- [ ] `shell_get_selection` reports the field: its
      reply carries the graph channel selection
      (the raw `selected_channel` value, `null` when
      unset) — a test sets it via
      `shell_select_graph_channel`, reads it back via
      `shell_get_selection`, and gets the same value.
- [ ] No side effects beyond the field: the call spends
      no undo step, marks no dirty edit and mutates no
      project data (test-asserted against undo depth /
      edit counter / a project read-back).
- [ ] On stdio the tool refuses with the canonical
      live-endpoint sentence every shell tool uses —
      never a success, never a panic (the shell-family
      stdio pin from Wave 14 still passes).
- [ ] A no-tool outcome is acceptable ONLY if B1's
      report cites a source fact contradicting the
      premise above; in that case the argued alternative
      must be recorded in `docs/MCP.md` and in the
      PARITY_CENSUS close-out addendum (§5), and this
      section's boxes are struck through with the
      reason — not ticked, not silently dropped.

**G7 — the recent-colours list (B2,
`src/rune_sec/inspector_style.rs` tracing +
`src/shell_tools_style.rs` if a tool lands)**
The storage facts the decision must name (all greppable
in `inspector_style.rs`): the list is a panel-local
`thread_local`, most-recent-first, deduplicated by
`push_recent`, capped at 8, written by
`note_recent_color` from the style panel's own colour
writes, and consumed by the panel's `recents_row`.
Applying a colour from the list is already tooled —
it is `set_node_style`'s effect via
`apply_recent_color`. Exactly ONE of the two outcomes
below closes this section:
- [ ] **Outcome 1 — read tool landed:**
      `shell_get_recent_colours` exists in
      `SHELL_TOOL_NAMES`, the registry stub, the
      dispatch and a handler (in the B2-named file);
      a test records at least two colours through the
      panel's own write path (`note_recent_color`) and
      asserts the reply lists them most-recent-first,
      deduplicated, in the representation
      `docs/MCP.md` documents for the tool.
- [ ] **Outcome 2 — session-only declared:** no
      `shell_get_recent_colours` (or any recent-colours
      tool) appears in `SHELL_TOOL_NAMES` or the
      registry (grep misses), AND the session-only
      declaration is written in `docs/MCP.md` with its
      reason — panel-local `thread_local` state, not
      `State`-held, not persisted; the capability (use
      a recent colour) is already reachable via
      `set_node_style` — AND the same disposition is
      recorded in the PARITY_CENSUS close-out addendum.
      Silence in either document is a fail.
- [ ] **No fake write, either way:** no tool claims to
      set the recent-colours list (grep the registry
      for a `shell_set_recent_colours`-style name —
      miss), unless a landed write demonstrably changes
      what the panel's `recents_row` renders, proven
      by a test that writes through the tool and reads
      the list through the panel's own `recent_colors()`.
- [ ] The read (Outcome 1) spends no undo step and
      marks no dirty edit (test-asserted or true by
      construction — a handler whose signature takes no
      mutable core/state handle).

---

## 3. Squad C — weather defaults reconciliation

**(C1, `src/mcp_tools.rs` `tool_set_weather` +
`src/editor.rs` `eds_kind_preset`)**
- [ ] One source: `tool_set_weather` takes its omitted
      amount/wind defaults from
      `crate::editor::eds_kind_preset`. A read of the
      `tool_set_weather` body shows the call, and no
      per-kind default literals of its own (no local
      match table returning 90/20, 140/30 or any other
      amount/wind pair).
- [ ] The defaults, pinned per kind by a test that
      calls `set_weather` through dispatch with only
      `kind` given and asserts BOTH the reply and the
      stored `core.project.weather`:
      rain **90 / 25**, snow **90 / 12**,
      fireflies **60 / 4**, fog **60 / 0**,
      storm **140 / 30** (amount / wind).
- [ ] Explicit arguments still win individually: a test
      passes only `amount` and gets the preset wind,
      and only `wind` and gets the preset amount; the
      amount clamp (0..=200) is unchanged (a test
      clamps an over-range amount exactly as before).
- [ ] Unchanged behaviour around the change: `kind:
      "off"` still clears the weather, an unknown kind
      still refuses quoting the kind, and the
      dirty-marking contract still holds — a real
      change marks exactly one edit, re-setting the
      identical weather marks none, and no call in the
      sequence spends an undo step (the entering tests
      `set_weather_marks_dirty_on_change_and_on_off`
      and neighbours pass unmodified in CI).
- [ ] The comment truth: the `eds_kind_preset` doc no
      longer describes two differing tables — a grep
      over `src/editor.rs` / `src/mcp_tools.rs` for the
      entering claim (the tool carrying its own,
      different defaults) finds it only in past-tense
      "used to" wording, and `docs/MCP.md`'s
      `set_weather` row states the same five
      amount/wind pairs the test pins.

---

## 4. Squad D — layout wiring

**D1 — the prefs become real (`src/main.rs` + the
render sites)**
- [ ] Startup applies `timeline_collapsed`: with the
      layout prefs file holding
      `"timeline_collapsed": true`, a fresh shell's
      rendered timeline is the collapsed form; with
      `false`, the full timeline (a Runtime test or a
      headless tree asserts the rendered difference —
      a state-field-only assertion is not this item).
- [ ] Startup applies `sidebar_visible`: with
      `"sidebar_visible": false`, the left column is
      not rendered at startup (`State::show_left`
      seeded false); with `true`, it is. (The pref is
      the LEFT sidebar only — `src/shell.rs`'s field
      doc — and no item here wires a right-sidebar
      pref that does not exist.)
- [ ] Writes go back: changing the left sidebar's
      visibility through its existing GUI controls
      (View menu / palette toggles) updates
      `sidebar_visible` in the persisted prefs — a
      test toggles, then asserts a fresh
      `load_layout()` reads the new value. Changing
      the timeline's collapsed state through its
      control path (the D2 tool, and any GUI toggle
      D1 wires) likewise persists `timeline_collapsed`.
- [ ] No other layout behaviour moves: column widths,
      workspace, dark and `live_mcp` seeding read and
      persist exactly as entering (their existing
      tests pass unmodified), and a prefs file written
      before this wave still loads with the same
      defaults (`timeline_collapsed: false`,
      `sidebar_visible: true`).

**D2 — `shell_set_timeline` is a real control
(`src/shell_tools_layout.rs`)**
- [ ] `shell_set_timeline` exists in all four places:
      `SHELL_TOOL_NAMES` (exactly once), the registry
      stub (exactly once), the name dispatch, and a
      real handler `tool_shell_set_timeline` in
      `src/shell_tools_layout.rs`.
- [ ] Round-trip through the handler: calling it with
      `{"collapsed": true}` updates the live timeline
      state AND persists the pref — a fresh
      `load_layout()` reads `timeline_collapsed ==
      true`; calling it with `{"collapsed": false}`
      reverses both (test-asserted).
- [ ] `shell_get_layout` tells the new truth: its
      reply carries `timeline_collapsed` matching the
      live state with `timeline_collapse_wired: true`,
      and `sidebar_pref_wired: true` — a grep of
      `src/shell_tools_layout.rs` finds no
      `"timeline_collapse_wired": false` and no
      `"sidebar_pref_wired": false` literal.
- [ ] The stale words are gone: the module header no
      longer presents timeline collapse as EXCLUDED /
      not provided, and the `shell_get_layout` tool
      description (registry draft in the file + the
      spliced entry) no longer says "currently false"
      / "does not read them yet" (grep both phrases —
      miss).
- [ ] Argument discipline matches the family: a
      missing `collapsed` refuses with
      `missing required argument 'collapsed'`, a
      non-boolean refuses with `must be a boolean`,
      and a refusal changes neither the live state nor
      the persisted pref (test-asserted).
- [ ] No undo step, no dirty mark: the tool spends
      neither (test-asserted), matching every other
      shell layout control.
- [ ] On stdio the tool refuses with the canonical
      live-endpoint sentence (the shell-family stdio
      pin still passes).

---

## 5. Squad E — proof & gates

**Gate (E1, `scripts/phase_j_checks.py`)**
- [ ] Check 10 (PARITY) accounts for the wave's tools
      honestly: any focused tool that answers an
      existing palette command is named in
      `PARITY_MAP` for that command; `PARITY_PENDING`
      is empty at close; and every entry left in
      `PARITY_EXCLUSIONS` carries an argued
      no-capability-loss reason. No palette command is
      dropped from the accounting to make the numbers
      fit (the check's own derivation count is
      unchanged in method).
- [ ] The G5/G6/G7 dispositions are recorded where a
      future census will find them: the
      `docs/wave14/PARITY_CENSUS.md` close-out addendum
      (or a focused-wave addendum it points to) states
      G5 CLOSED by `figure_library_rename`, G6 CLOSED
      by `shell_select_graph_channel` (or the §2
      argued alternative), and G7's §2 outcome —
      matching what actually landed, verified against
      the registry, not the reports.
- [ ] Full gate on the closing tree: **10/10 PASS**
      (recorded in §7), including check 9 (registry ↔
      `docs/MCP.md` two-direction sync at the closing
      total) and check 10.

**Docs (E2, `docs/MCP.md`)**
- [ ] One row per landed tool, with its arguments:
      `figure_library_rename` (core table),
      `shell_select_graph_channel`,
      `shell_set_timeline`, and
      `shell_get_recent_colours` iff §2 Outcome 1
      landed. Gate check 9's sync is the proof; a row
      whose text disagrees with the handler's actual
      behaviour (defaults, refusals, undo bookkeeping)
      fails this item even if the sync passes.
- [ ] The `set_weather` row states the reconciled
      defaults (§3's five pairs) as the panel presets.
- [ ] The `shell_get_layout` row no longer calls the
      timeline/sidebar prefs "currently unwired".
- [ ] G7's outcome is written in `docs/MCP.md` per §2
      (tool row for Outcome 1, session-only note for
      Outcome 2).

**Probe (E3, headless live MCP probe)**
- [ ] If `shell_set_timeline` landed (§4), the Q4
      live probe in `.github/workflows/headless.yml`
      exercises it against the real editor, in the
      probe's existing `check()` pattern: a
      `shell_set_timeline {"collapsed": true}` call
      answers without error, a following
      `shell_get_layout` shows `timeline_collapsed`
      true AND `timeline_collapse_wired` true, and the
      probe then sets it back to `false` (the app is
      left as it was found). The closing headless
      artifact's `mcp-probe.txt` contains the PASS
      lines for all three (§7). If the handler landed
      after this checklist was written, adding this
      step is integrator homework BEFORE the closing
      headless run — a headless PASS without the
      timeline lines does not tick this box.
- [ ] The probe transcript's health count and
      tools/list count agree with each other and with
      the closing census total (§7) — `TOOL_FLOOR`
      raised if the closing total requires it, or its
      comment updated to the new floor's derivation.

**Suites execute (the fail-fast lesson)**
- [ ] The closing CI log shows each new suite by name
      with its own `test result: ok` line — the
      library-rename e2e (`tests/focused_e2e_library_rename.rs`),
      and the shell selection / layout / weather test
      groups in their owners' files. A green total
      that never names a suite does not prove it ran
      (cargo test fail-fast silently skips later
      binaries behind an earlier failure).

---

## 6. Exclusions diff guard (integrator, on the wave diff)

- [ ] No project-swap undo/redo semantics change: the
      load / `.piv` import / New / open clear-behaviour
      is behaviour-identical; the diff's undo deltas are
      confined to the new tools' own bookkeeping as
      pinned in §1/§2/§4 (the rename's zero-step file-op
      contract included — it is not a side door into
      project undo).
- [ ] No accent change: the diff touches no accent
      token or accent call-site colour (upstream
      `#4392F7` carrier and the app token untouched; a
      grep of the diff for accent hexes comes back
      empty). §2's recent-colours read, if landed,
      exposes the style panel's colour history only —
      no accent value is readable or settable through
      any new tool.
- [ ] No MCP-save `write_recovery` change: the MCP save
      paths' recovery behaviour is diff-empty. The
      library rename writes library files only — no
      project file and no recovery file is written by
      any §1 path (the e2e suite's fixture directory
      listing before/after is the evidence).
- [ ] No waveform-badge change: the deleted-file
      waveform behaviour (stale waveform vs "(missing
      file)" badge) is diff-empty.
- [ ] No stored-name divergence fix: `project_rename`'s
      stored-name behaviour is diff-empty, and the §1
      rename provably does not touch it — the e2e
      suite asserts the project name is unchanged by a
      library rename.
- [ ] G3 stays untooled: no tool named
      `cycle_onion_frames` (or a shell alias for it) is
      registered, and no new handler calls
      `EditorCore::cycle_onion_frames` — grep the
      registry and `SHELL_TOOL_NAMES` (miss) and the
      wave diff for the op name (only pre-existing
      mentions).
- [ ] No side doors: every new shell tool writes only
      the shell state or prefs its name claims (§2's
      graph field, §4's two layout prefs, §2's read) —
      none loads, creates or swaps a project, changes
      a theme/accent value, or alters what an MCP save
      writes.

---

## 7. Integrator re-run list (close-out)

**Gate + censuses**
- [ ] `python3 scripts/phase_j_checks.py`, full
      foreground run read from the top: **10/10 PASS**
      — recorded here: ____________.
- [ ] `python3 scripts/mcp_census.py`: registry
      ______ (entering 313), io **9**, built-ins **44**,
      total ______ (entering 366), unregistered **0**,
      duplicates **0**; gate check 9 agrees and
      `docs/MCP.md` syncs both directions. The total
      must equal 366 + (tools actually landed:
      `figure_library_rename`, `shell_select_graph_channel`,
      `shell_set_timeline`, and `shell_get_recent_colours`
      iff §2 Outcome 1) — any other delta is explained
      here by name: ____________.
- [ ] Counts re-derived against the entering baseline:
      MCP 366 → ______; palette **134** (any change
      itself censused); scenes **29** (any new scene
      named); layout facts **31** (any change named);
      help **61** (or the named delta); keys 940 →
      ______ with **zero lost**; icons 163; i18n
      99 × 28; tests 3,519 → ______ (CI's exact
      figure, not a rounding).

**Spot-greps per squad (the claims, sampled)**
- [ ] A: `std::fs::rename` hits only in
      `src/library_ops.rs` for the rename path;
      `tool_figure_library_rename` greps in
      `src/library_ops.rs` and its name greps exactly
      once in the registry; `rename_saved`'s body is
      the one-line delegation.
- [ ] B: each landed shell name greps exactly once in
      `SHELL_TOOL_NAMES` and once as a registry stub;
      `selected_channel` greps in the
      `shell_tools_selection.rs` handler path; for G7,
      exactly one of §2's two outcome artifacts exists
      (the handler, or the session-only texts in
      `docs/MCP.md` + the census addendum).
- [ ] C: `eds_kind_preset` greps inside
      `tool_set_weather`; the five-pair default table
      greps in the C1 test; no second default table
      greps in `src/mcp_tools.rs`.
- [ ] D: `timeline_collapsed` and `sidebar_visible`
      each grep as READERS in `src/main.rs` (not only
      in `src/shell.rs`'s load/save); the two
      `"…_wired": true` literals grep in
      `src/shell_tools_layout.rs` and the `false`
      literals miss; `tool_shell_set_timeline` greps
      in the handler file.
- [ ] E: every §5 doc row greps in `docs/MCP.md`; the
      probe's timeline step greps in
      `.github/workflows/headless.yml` iff §4 landed;
      `tests/focused_e2e_library_rename.rs` exists.

**Run evidence (fill at close-out)**
- [ ] CI run ____________ on sha ____________:
      **______ tests passed, 0 failed** across all
      binaries — including the wave's new suites listed
      by name in the log (§5: the library-rename e2e,
      the shell selection/layout groups, the weather
      defaults test).
- [ ] Headless run ____________: every probe exit 0;
      live-MCP probe **PASS** with the health total
      reading the closing ______ AND, iff §4 landed,
      the `shell_set_timeline` / `shell_get_layout`
      wired PASS lines present in `mcp-probe.txt` (§5);
      overflow scan **CLEAN on ______ trees**
      (denominator recorded); snaps pixel-read and
      copied to `your_files` as
      `stykanimate-focused-*.png` (or the wave's chosen
      prefix, recorded here).
- [ ] Docs finalised: §6.10 TODO-AT-CLOSE blocks filled
      from the evidence above; `docs/MCP.md` at the
      closing total; the PARITY_CENSUS addendum carries
      the G5/G6/G7 dispositions (§5); the §8 verdict
      below written.
- [ ] Carried, unsettled, stated in the user report:
      sound remains user-verified only (no item in
      this wave touches playback audio); the five
      standing user decisions (project-swap undo,
      white-on-accent, two New-project results, C5
      waveform, MCP-save `write_recovery`) untouched,
      plus the stored-name divergence; G3 untooled by
      design (§6).

---

## 8. Close-out verdict (integrator fills at close-out)

**ACCEPTED** (2026-11-09, integrator close-out).

Per squad, against §§1–5: **A** — `figure_library_rename`
landed over `EditorCore::rename_library_entry`
(`src/library_ops.rs`); the panel is a wrapper and its
helpers delegate to the lib copies; the MCP-only e2e
suite pins the happy path, all four refusals as isError
results, and the bookkeeping contract. **Bookkeeping
correction, recorded as instructed:** the roster's A3
phrase "exactly-one-undo-step" was wrong — a library
rename is a file operation spending **zero** undo steps
and no dirty mark (the `pose_library_rename` precedent);
§1 is ticked against the zero-step contract, and the
suite proves Undo does not revert a rename.
**B** — `shell_select_graph_channel` landed (the
panel's own `select_channel`); `shell_get_selection`
reports the pick and the drawn channel. **G7 outcome,
by name: (a) — the read tool landed**
(`shell_get_recent_colours`, session-only, cap 8);
the no-write-tool reasoning is in the handler's module
docs and §2's branch (a) boxes are the ticked ones.
**C** — one weather defaults source
(`editor::eds_kind_preset`), five kinds pinned through
the tool. **D** — both prefs wired
(`State.frame_ops.timeline_collapsed`; `sidebar_visible`
as the left column's mirror), `shell_set_timeline`
landed, both `…_wired` flags report true; the two
argued deviations (sub-state field, no GUI toggle
invented) are recorded in §6.10's Delivered.
**E** — check 10's tables deliberately unmoved with
the disposition recorded (§5's accounting rule); MCP.md
rows + counts exact (gate check 9 derives them);
probe step 9 landed in the headless workflow and
passed live.

Run evidence (§7): gate **10/10**; **CI 42578449523
GREEN, 3,539 tests, 0 failed** (1,437 lib + 8
mcp_server + 1,997 bin + 97 integration; A3's suite
appears with its own `test result: ok` line);
**headless 42579538663 SUCCESS** — all probes 0,
MCP PROBE PASS incl. step 9, overflow CLEAN on 32
trees. Closing census: **MCP 370** (registry 317 +
io 9 + built-ins 44), shell 38, palette 134, keys
940 zero lost, scenes 29, help 61.

Remainders, each carried by name into §6.10's Honest
remainders: the shared-layout-file test race (two
snap tests normalise), `shell_get_state`'s one-tick
timeline lag, MCP-applied colours not entering the
Recent list, sound user-verified only. No §1–§5 box
stands unticked without one of those names.
