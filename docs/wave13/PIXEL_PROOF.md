# Wave 13 — Pixel-proof coverage ledger

**What this is.** The entering-state ledger for the headless
proof system as Wave 13 opens: the coverage Wave 12 closed
with, the two Wave 13 scene additions in flight and the proof
each must carry when it lands, and the honest gaps entering
this wave — gap #1 being the library thumbnails, unit-proven
in Wave 12, whose pixel proof this wave exists to land. The
integrator reconciles this ledger at Wave 13 close-out; the
entering state below is exact as written. Modelled on
`docs/wave12/PIXEL_PROOF.md`; where that ledger's §§1–2 give
the per-scene table for the 26 entering scenes, this ledger
records the deltas and cites it rather than duplicating the
table.

**Entering state.** HEAD at launch: the Wave 12 close-out
tree (`docs/REDESIGN_PROGRESS.md` §6.7, closing census and
Verification). Scene census at source: **26 scenes**
registered in `scenes()` in `src/rune_sec/scenes.rs` (§6.7
closing census: "scenes **26** (entering 24)"). Proof is
produced by the manual-dispatch workflow
`.github/workflows/headless.yml` ("Headless Rune probe
(snap / tree)") and read at the headless debrief; the source
gate's check 8 (`check_coverage` in
`scripts/phase_j_checks.py`) enforces the scene ↔ workflow
cross-reference in both directions (Wave 12 ledger, §1
"Coverage enforcement").
(Source: §6.7 closing census + Verification;
`docs/wave12/PIXEL_PROOF.md` §5.)

**Evidence key.** Every claim about the entering tree cites
one of: the Wave 12 ledger (`docs/wave12/PIXEL_PROOF.md`,
cited as "W12 §n"), §6.7 of `docs/REDESIGN_PROGRESS.md`
(cited as "§6.7"), a step in `.github/workflows/headless.yml`,
or a scene in `src/rune_sec/scenes.rs` (its doc comment
and/or its numbered layout fact in that module's
`layout_assertions()` preamble). The artifact file
convention is unchanged: `headless-out/tree-<scene>.json`,
`headless-out/snap-<scene>.png`, uploaded as the
`headless-probe-output` artifact.

**Proof-kind codes used in this ledger** (full definitions
in §4, carried from W12 §4):

- **T** — tree dump: the scene is named in a `for scene in …;
  do` loop of the workflow's "Tree dumps — plain (no display)"
  step and produces `tree-<scene>.json`.
- **S** — snap PNG: the workflow's "Snap renders — lavapipe
  attempt" step renders `snap-<scene>.png` for the scene.
- **O** — overflow scan: the tree dump is in the set scanned
  by `scripts/overflow_scan.py` at the debrief (§6.7: overflow
  CLEAN on all 29 trees — the 26 scene trees plus the 3
  size-variant trees listed in §1).
- **—** — that proof kind is not applied to this scene.

## 1. Entering coverage — 26 scenes, 29 trees, scan CLEAN

The full per-scene table for the entering 26 lives in the
Wave 12 ledger: §§1–2 give the 24 scenes Wave 12 entered
with (seeded state, snap, layout fact per row), and its §5
close-out records the two Wave 12 additions
(`home-populated`, `strokes-populated`) landing with T, S
and O each. This section records only the closing totals
the Wave 13 additions build on.
(Source per total: §6.7 Verification; W12 §5.)

- **Scenes: 26**, all in the plain tree loop, so **T and O
  apply to every scene** (gate check 8 forward coverage,
  W12 §1). The xvfb fallback loop retries only scenes the
  plain attempt missed and, by the standing subset rule,
  omits the two light-theme scenes (`default-light`,
  `settings-open-light`), which the plain loop — the
  coverage contract — covers (W12 §1, report-only asymmetry
  printed by the check itself).
- **Overflow denominator: 29 trees** — the 26 scene trees
  plus the three 1280×720 size-variant dumps
  (`tree-default-1280.json`, `tree-default-light-1280.json`,
  `tree-three-d-1280.json`; window size is a CLI axis, not
  scene state — W12 §1). At the Wave 12 close-out debrief
  the scan over headless run **42557913977**'s artifact was
  **CLEAN (0 hits) on all 29 trees** (§6.7 Verification;
  W12 §5). The same run's live-MCP probe was PASS
  end-to-end (not a scene proof — §4).
- **Snap coverage: 21 of 26 scenes snapped; 5 tree-only.**
  Wave 12 entered with 19 of 24 snapped (W12 §1) and its
  two additions landed snapped and pixel-read at the
  debrief (§6.7 Verification: the home-populated snap shows
  three real project rows with rendered thumbnails; the
  strokes-populated snap shows all four stroke kinds with
  the stroke inspector mounted). The five tree-only scenes
  are unchanged: `frame-3`, `selected`, `playing`,
  `graph-duplicates`, `loop-range` (see gap G2).
- **Layout facts: 29**, sequential (gate check 8, §6.7
  closing census). The Wave 13 additions continue the
  sequence at 30 and 31 (§2).

## 2. Wave 13 additions — in flight at launch

Neither scene exists at HEAD: `scenes()` still registers 26
and the module's pin test still asserts 26. Both are Wave 13
squad D work in flight (agents D1 and D2, with D3's Runtime
pins and D6's scan fixtures as companion proof); the
integrator lands each scene, its workflow wiring and its
layout fact together, and this section records the proof
each WILL have when it lands — reconciled against the
closing tree at close-out.

| Scene (planned name) | Owner | State it will seed | Proof it will have |
|---|---|---|---|
| `library-populated` | D1 (in flight) | The figure-library panel over a NON-empty library: entries seeded in memory per Wave 12 D5's spec (a styled entry and a mini-copy entry), so the library rows render their per-entry thumbnails instead of the empty state the entering scenes render. | **T** — must be added to the plain tree loop in `headless.yml` (gate check 8 forward coverage fails the gate until it is); **S** — `snap-library-populated.png` added to the "Snap renders" step (D2) and pixel-read at the debrief; **O** — its tree joins the overflow-scan set (denominator 29 → 30), with D6's self-test fixtures covering the library-row tree shape; a new numbered layout fact (**30**) in `scenes.rs` asserting the populated row keys under `--scene library-populated`. Companion Runtime proof: D3's Round 14 asserts library thumbnails present in this state (`snap_tests.rs`). |
| `sprites-populated` | D1 (in flight) | Sprite instances placed in the project with the Sprites panel mounted (Wave 13 squad A's new surface), so the panel renders its populated rows — instance list, key counts — instead of having no mounted state in any scene. | **T** — same tree-loop requirement via gate check 8; **S** — `snap-sprites-populated.png` in the "Snap renders" step (D2), pixel-read at the debrief; **O** — its tree joins the overflow-scan set (denominator → **31** once both additions land), with D6's self-test fixtures covering the sprites-panel-row tree shape; a new numbered layout fact (**31**) asserting the panel's populated-row keys under `--scene sprites-populated`. Companion Runtime proof: D3's Round 14 drives the panel flow (add → instance exists; delete → gone). |

Both scenes also join the xvfb fallback loop under the
standing subset rule of §1 (the rule omits only the two
light-theme scenes; neither addition is one).

Until both land, §6.7's closing census (scenes **26**,
layout facts **29**) stands; the closing census is expected
to read scenes **28**, layout facts **31**, overflow
denominator **31 trees**.

## 3. Honest gaps — unit-proven only, or unproven

Each gap names its evidence, its reason, and its owner — a
Wave 13 item, or **UNOWNED** where no squad's remit covers
it. G1 is this wave's headline gap; G2–G4 are the standing
gaps carried from the Wave 12 ledger, restated without
embellishment (their Wave 12 numbers are given so the two
ledgers can be read together).

- **G1 — Library thumbnails are unit-proven only; no snap
  exists.** Evidence: §6.7 Honest remainders (7) ("D5's
  library-populated scene was not built — library
  thumbnails are unit-proven, not yet pixel-proven") and
  W12 §5's closing item ("library thumbnails are
  unit-proven only — D5's library-populated scene was not
  built (a 27th scene was out of scope), so the library
  rows' new thumbnails have no snap yet"). Reason: Wave 12
  D5 built the real per-entry thumbnail renderer (library
  rows had NO thumbnail before that wave — §6.7 Delivered,
  squad D) and its tests pin the prims, but no entering
  scene seeds a non-empty library, so no headless state
  ever renders a library row, thumbnail included. **Owner:
  Wave 13 D1 + D2** (the `library-populated` scene and its
  workflow wiring, §2), with D3's Round 14 pin and D6's
  library-row scan fixtures as companion proof. This gap
  closes at this wave's debrief if the snap is pixel-read
  there, the way W12 §5 closed its G1–G3.
- **G2 — The canvas-drawn state of the five tree-only
  scenes is pixel-unproven** (Wave 12 G4). Evidence: the
  `layout_assertions()` preamble in `scenes.rs` states the
  tree dump carries keys, counts and rects only — state
  drawn inside the stage `Shape` (selection handles, the
  FX gizmo, loop edges, a parked playhead's styling) "can
  only be checked in the `--snap` PNG". The workflow snaps
  none of `frame-3`, `selected`, `playing`,
  `graph-duplicates` or `loop-range`. Reason: accepted
  scope, wave by wave — snaps were added where a wave's
  own surface needed pixels. `playing` carries the extra
  recorded caveat that a playing snap is deterministic
  only at `--at 0`. W12 §5: "G4, G5, G6 stand as written."
  **Owner: UNOWNED** — no Wave 13 item adds these snaps.
- **G3 — Sound is user-verified only** (Wave 12 G5).
  Evidence: §6.7 Honest remainders (9) ("Sound remains
  user-verified only") and §6.7's standing-decisions
  paragraph (the same words, carried as a user decision
  the wave does not settle); W12 §3 G5 cites
  `docs/QA_RUNE.md`'s standing open row "Sound unverified
  on a real screen — the headless snap covers pixels, not
  audio". Reason: structural — no headless probe in
  `headless.yml` can listen; the proof is the user's Play
  test on a real machine, still outstanding. **Owner:
  UNOWNED** (the user's check, not a wave item).
- **G4 — 1280×720 coverage is three pairings, not a
  matrix** (Wave 12 G6). Evidence: the size-variant probes
  listed in §1 — only `default`, `default-light` and
  `three-d` are dumped at 1280×720, and only the first two
  are snapped there. Reason: window size is a CLI axis
  (per the `scenes.rs` module docs), so each small-window
  proof is an explicit extra invocation, and only the
  deepest/widest states were ever paired. W12 §5: stands
  as written. **Owner: UNOWNED.**

**Standing arrangement (not a gap): the overflow scan is a
debrief step, not a workflow step** (Wave 12 G7). Evidence:
`scripts/overflow_scan.py`'s own header — "it belongs to
the headless debrief, not the source gate (the dumps are
CI artifacts)" — and `headless.yml` contains no overflow
step; the CLEAN verdict in §6.7 is the integrator's run
over the downloaded artifact. Consequence, unchanged: a
headless run never self-reports overflow; skipping the
debrief scan would silently un-prove all trees in the set
(29 entering, 31 at this wave's close). **Owner: the
integrator at every close-out debrief** (standing practice,
§6.5–§6.7).

## 4. Proof-kind definitions

Carried from W12 §4 unchanged in substance; the Wave 12
close-out (§6.7 Verification, W12 §5) exercised every kind
below on the entering tree.

**Tree dump (`--tree out.json`).** The binary applies the
named scene to the freshly initialised state and dumps the
element tree as JSON, rects included (the workflow's Q3
step, "Probe tree JSON for element rects", greps each dump
for rect fields and the `stage` node's shape). A tree dump
proves **structure and geometry**: which elements mounted
(workspace gating — layout facts are presence/absence
pairs), key counts, and exact element rects — the overflow
scan's rules and any stacking claim are judged from these
rects. It proves nothing about paint: the `scenes.rs`
preamble is explicit that canvas-drawn state lives in the
`Shape`, not the tree.

**Snap PNG (`--snap out.png`).** An offscreen render of the
whole window (or one `--crop`ped subtree) at `--at 0` on
the virtual clock, rendered under lavapipe/software Vulkan
(the workflow's Q2). A snap proves **drawn pixels** — the
only proof kind that can see poses, selection handles,
gizmos, theme colours and card styling. Two honesty limits:
(a) most snaps are proof *by reading* — the integrator
pixel-reads them at the debrief (W12 §5's reads of the
home-populated and strokes-populated snaps) — there is no
automated pixel-diff step in `headless.yml`; (b) where a
pixel assertion IS stated, it is stated in the fact itself
(fact 19: the `default-light` snap's background pixels sit
in the light range, `bg` `#EDF0F5`, where `default`'s sit
in the dark range). A snap file merely existing in the
artifact is presence, not a passed assertion.

**Overflow scan (`scripts/overflow_scan.py`).** A
stdlib-python scan over tree dumps (not snaps): Rule 1
content edges inside the side columns, Rule 2 card bounds,
Rule 3 window bounds, Rule 4 the timeline card's vertical
bounds (added Wave 10 for the dopesheet lane). A run is
CLEAN when every file reports zero non-exempt hits. It
proves **no element paints past its card or the window**
in the dumped states — the defect family that survived
Waves 1–8 in the un-scanned clips rows is exactly what it
exists to catch.

**Runtime presented rects (`src/rune_sec/snap_tests.rs`).**
The in-process `Runtime` tests are a different proof family
from the headless workflow: they drive the real shell with
queued pointer/key input and assert on the presented tree.
`Runtime::presented(key)` returns the element's **drawn**
rect `[x, y, w, h]` plus effective opacity. The standing
presented-box doctrine (§6.6, restated at W12 §4): a
presented rect is a drawn box, and a mid-session-mounted
canvas can rest up to its own height above its layout slot
— so edge/displacement assertions against presented rects
are unsound, and exact stacking is proven by centre order
in the Runtime and by the exact rects in the headless tree
dump instead. Presented rects prove presence, ordering and
interaction outcomes; tree dumps prove exact geometry.
Wave 13's companion pins (D3's Round 14) belong to this
family.

**Live-MCP probe (not a scene proof).** The workflow's Q4
step launches the real editor under Xvfb and drives its
live HTTP endpoint through the full JSON-RPC chain, writing
the assertion transcript `mcp-probe.txt` (§6.7: PASS end to
end at Wave 12 close). It proves the live control path, not
layout; it is listed here only so the artifact's contents
are fully accounted for in this ledger.
