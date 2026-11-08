# S20 — Integration & acceptance plan (mission Phases 2–10)

> Squad 20's deliverable for Phase 1: not a UI audit but
> the **integration plan** — how the other 19 squads'
> work lands without breaking the tree, and how every
> acceptance criterion in the mission brief (§11) is
> proven on a concrete surface. Built from the Phase N
> record (`docs/PHASE_N.md`), the gate
> (`scripts/phase_j_checks.py`, six checks), the
> headless workflow (`.github/workflows/headless.yml`)
> and `docs/QA_RUNE.md`'s structure. Where this plan and
> a squad plan disagree about *how work lands*, this
> plan governs; where they disagree about *what a
> surface should look like*, the squad plan governs.

## 1. The Phase N model being reused

Phase N integrated ~80 agents' work with zero keys
lost and three green CI verdicts. The mechanisms that
made that true, and which this mission reuses verbatim:

1. **Charter before code.** The design language and
   the kit API were fixed in writing (15 rules)
   before any agent edited source; amendments were
   ratified by the integrator and recorded, never
   improvised per file.
2. **Waves with hard gates between them.**
   Foundation (kit + family, one commit `b5aac0d`)
   → CI green → Adoption (32 files, four batches of
   eight, one agent per file, one commit `a570d60`)
   → CI green → Verification (scenes, Runtime tests,
   gate check 6, copy pass) → five fix commits → CI
   green → headless pixel pass. No wave started on a
   red tree.
3. **The gate is the contract.**
   `scripts/phase_j_checks.py` runs six checks:
   (1) key inventory, (2) per-file key diff vs a base
   ref — zero lost, (3) State-literal census (43
   fields × 20 full literals), (4) caller census
   (finished work with no production caller reads
   UNWIRED), (5) count claims read from source
   (palette / MCP / icons / scenes — docs must match
   the gate, never the other way round), (6) design
   adoption (kit self-check, per-file floor, retired-
   helper census with named exceptions only).
4. **Integrator-owned surfaces.** `src/main.rs`
   wiring, mount-order moves, census/gate script
   changes and workflow edits were never delegated.
   Agents flagged; the integrator landed.
5. **CI is the only compiler** (repo AGENTS.md). The
   loop is push → read the run's own logs → fix →
   push. A new push cancels the in-flight run
   (concurrency group), so the only verdict that
   counts is the **newest sha's completed run**.
6. **Headless discipline.** `headless.yml` is
   dispatch-only and fail-soft; every `probe()` wipes
   `./projects` and takes a fresh `HOME` per
   invocation, because layout prefs persist to the
   CWD-relative `./projects/.layout.json` and leaked
   the Draw workspace into `snap-default` for two
   runs before the cause was found. Proof is counted
   from the artifact's `.exit` files (23 probes: 13
   trees + 10 snaps), and **every snap is pixel-read
   by the integrator** — a green run alone is not
   visual proof (brief §11, final verification).
7. **Tests assert relations, not coordinates.**
   `Runtime::presented` returns the drawn box; for a
   mid-session-mounted canvas it rests up to its own
   height above the layout slot. Edge-abutment and
   displacement asserts are unsound; centre order,
   shared edges and presence are sound, and exact
   geometry is proven from the `--tree` dumps
   (Phase N: timeline 732..786, clips lane 788..798,
   loop row 800..825, exact 2 pt gaps).

**Baselines the mission starts from** (Phase N close,
all gate- or CI-verified): **804 unique keys** (699
static + 105 dynamic; repaired-census baseline 788 —
the census repair, not the tree, accounts for the
difference from Phase M's 733); **2,492 tests** green
(run 42488645714); palette **108** commands; MCP
**102** tools (registry 91); icons **163**; help
**54** rows; scenes **15**; State **43 fields / 20
literals** (the mission adds none — §3, rule F5);
default tree **1,018 elements** (growth budget
+8–11 % per the Phase N P19 precedent); stage rect
**[328, 157, 744, 485]** at 1440×900; columns
300 / 744 / 340.

## 2. Wave plan — mission Phases 2–10

Twelve waves. A wave lands as one commit (panel
batches: one commit per batch), gate-green against
the previous wave's commit as base ref, then CI
green, before the next wave starts.

| Wave | Mission phase | Squads | Scope (files) | Checkpoint |
|---|---|---|---|---|
| W0 | 1 (close) | S01, S20 | Audit docs only (`docs/ui_mission/`); no source edits. Gate run on the untouched tree records the 804 / 43-field / 1,018-element baseline. | CP0 |
| W1 | 2 — Design system | S02 (lead), S17, S18 | `design.rs`, theme tokens (`tok` consumers), `chrome_view.rs` family, `icon_btn.rs`, `icon_data.rs` (additions only, each verified single-path against `assets/icons/fontawesome/`). The brief's proposed palette (bg `#101216`, accent `#6C8CFF`, success `#43C995`, warning `#F1B85B`, error `#EF6974`, …) is adopted **centrally in the token layer after a delta audit against the current tokens** — never as literals scattered into panels. Any new key-bearing helper ships with both census regexes updated in the same commit (§4, R1). | CP1 |
| W2 | 3 — Shell | S03, S04, S05 | `main.rs` (integrator-owned, §3 F2), `chrome_view.rs`, `menus.rs`, `workspaces.rs`, `status_strip.rs`, `timeline_marks.rs` card geometry. Mount-order changes are integrator decisions, listed in the wave commit message. | CP2 |
| W3–W6 | 4 — Feature panels | S06–S08, S11–S16 | Four batches of ~8 panel files, one owner per file, standard brief (root → kit card, headers → kit, dialects retired onto the family, keys byte-identical, handlers verbatim, no State fields). Suggested grouping follows Phase N's: W3 leaf panels (home/project/export/stats/help…), W4 lists (figures/library/layers/clips/fx…), W5 inspectors & forms (segment/figure/style, camera, scene, pose, audio), W6 drag surfaces (draw, stroke, graph, keyframe, frame_ops, three_d). `stage_view.rs` is audit-only in this phase, as in Phase N. | CP3a–d |
| W7 | 5 — Stage & timeline | S09, S10 | `stage_view.rs`, `stage_fig3.rs`, `stage_camera.rs`, `timeline_marks.rs`, transport in `chrome_view.rs`. Geometry invariants proven from tree dumps (stage stays the grow child; timeline stack relations), never from `presented` rects. | CP4 |
| W8 | 6 — Micro-interactions | S17, S18 (+ squad owners for their files) | Cross-cutting states (hover/press/selected/inert), toasts/feedback, confirmations, and the empty-state completion set — including the bare lines Phase N §8 left by ruling (project_tools' project list, text_tool's two lines, stroke_inspector, three_d_panel's three lines, camera_panel, the inspectors' no-selection lines), each with the glyph decision its squad plan names. Small diffs only; per-file owner rule still applies. | CP5 |
| W9 | 7 — Responsive & themes | S19 (lead), S02 | Light-theme parity work in the token layer; the overflow programme (the Phase O p05 plan's measured rows — `draw-brushes` 879 pt vs the 316 budget, `draw-tools-row`, `graph-actions`, layers, weather segments, settings language chips — repaired by wrap/two-line anatomy per that plan, width budgets stated per row); new scenes/snaps W10 needs (§5). | CP6 |
| W10 | 8 — Verification | S19, S20 | New scenes + Runtime test round; gate check 7 **only if** a checkable invariant has emerged (the overflow tree-scan is the candidate — it runs over headless tree dumps in the debrief, not in the gate, because dumps are CI artifacts); `docs/QA_RUNE.md` **§17** opened in its established shape (gap-closure map + inventory snapshot + `_integrator_` cells for final CI/headless); mission doc `docs/UI_MISSION.md` drafted with every number taken from the gate's check-5 output. | CP7 |
| W11 | 9 — Pixel polish | S19, S20, squad owners on call | Headless re-dispatch on the integrated tree; integrator pixel-reads **every** snap against the per-scene checklist; fixes land as small named fix commits (the Phase N fixes 1–6 pattern), each followed by CI + a final headless run. | CP8 |
| W12 | 10 — Acceptance | S20 (Agents 96–100 roles) | §11 checklist signed line by line (§5 of this plan) with run IDs; honest remainders written into the mission doc; the honest agent count stated, not padded (Phase N precedent: ~80 of 100, said plainly). | CP9 |

## 3. File-conflict rules (binding on all waves)

- **F1 — One primary owner per file per wave.** The
  squad brief names the owner; nobody else edits that
  file in that wave. Cross-squad needs are filed as
  requests to the owner or deferred to a later wave.
- **F2 — Integrator-owned files.** `src/main.rs`,
  `src/rune_sec/scenes.rs`, `src/main.rs`'s test
  modules (`snap_tests.rs`), `scripts/phase_j_checks.py`,
  `scripts/rune_inventory.py`, `.github/workflows/*.yml`.
  Squads propose; the integrator lands. (Phase N:
  every mount-order move, the waveform-card wiring
  and both census repairs were integrator work.)
- **F3 — Kit-first, family homes are foundation-only.**
  `design.rs`, `chrome_view.rs`, `icon_btn.rs`,
  `icon_data.rs` change in W1 (and by integrator
  amendment afterwards, recorded in the wave commit).
  Panel waves consume the kit; they do not extend it
  locally. A new local helper that duplicates a kit
  or family builder is a check-6(d) survivor by
  definition.
- **F4 — No two waves touch the same file in flight.**
  Waves are sequential by construction (gate + CI
  between), so this binds *within* a wave: parallel
  agents in one wave take disjoint file sets, and
  W8's cross-cutting pass runs after W3–W7 have
  settled the files it touches.
- **F5 — No new State fields, no model changes.**
  Phase N added zero State fields by charter rule;
  the mission brief (§2 rules 12–13) preserves
  project compatibility, undo and recovery. Any
  exception needs an integrator ruling *before* the
  wave, because one field means editing all 20 full
  `State { … }` literals (gate check 3) — the Phase I
  §23 near-miss was exactly this.
- **F6 — Keys byte-identical.** Keys, handlers,
  labels and status strings stay verbatim unless a
  squad plan explicitly lists a copy change and the
  integrator ratifies it (the Phase N copy pass is
  the precedent: two files, census-neutral). Dynamic
  key templates are not merged, renamed or bound to
  `let` variables without a source-level census
  disproof attached to the wave report (§4, R2).
- **F7 — Restyle roots, never nest wrappers.** The
  element-growth budget (+8–11 % on the default tree)
  is a checkpoint number at CP7/CP8, measured from
  the headless tree dump.
- **F8 — Docs numbers come from the gate.** Any count
  a squad report prints (keys, commands, tools,
  icons, scenes, tests, elements) is either gate
  check-5 output, a CI log line, or a tree-dump
  measurement — named as such. Unverifiable counts
  are not printed.

## 4. Risks — census and CI

**R1 — A new key-bearing helper family blinds the
census (highest risk to the 804 baseline).** Both
censuses recognise keys only as literals passed to
*known* helpers. Phase H's count "fell" 556 → 480
purely because the counter missed a new family;
Phase N's repair found the Phase M tree actually
held 788 keys, not 733 — `ctl`/`ictl` (three_d),
`kbtn` (keyframe), `pbtn`/`pibtn` (pose) and
`danger_btn` keys had been invisible. If S02/S17
introduce a new button builder in W1 and any panel
adopts it before the regexes in
`scripts/rune_inventory.py` **and** the gate's
`_DIFF_BTN_RE` know it, real key loss becomes
invisible — or phantom loss blocks a clean wave.
*Mitigation:* helper freeze after W1; census changes
are integrator-only, land in the same commit as the
helper's first use, and are validated against the
pre-change tree, which must still read exactly 804.

**R2 — Template merges and variable-passed keys fake
(or hide) key loss.** Phase N's 808 → 804 delta was
two census artefacts, each disproven at source:
inspector_style merged two channel-row builders, so
one template text now renders a key set two texts
rendered before (−3 patterns, zero rendered keys
lost); pose_tools bound a `format!(…)` key to a
`let` before the `btn` call — a documented blind
spot (−1 pattern). W5's inspector wave is exactly
where builder merges tempt. *Mitigation:* gate
check 2 runs **per batch** (base = previous batch
commit), and every reported loss is disproven or
confirmed at source before the batch is accepted —
the standing rule: *a checker script is a claim
too.*

**R3 — Layout shifts break the fragile Runtime-test
class (highest risk to CI green).** Phase N's
foundation run failed on
`prev_next_key_jumps_between_camera_keys`: after the
Playback mount-order promotion, `scroll_into_view`
only scrolled down and `find()` reported centre
y = −564; the first fix rested on a wrong hypothesis
and was reverted before the diagnostic commit found
the cause. The timeline geometry test then took
three more rounds. W2 (shell/mount order) and W7
(timeline) will move the same furniture.
*Mitigation:* new and touched tests assert relations
(presence, centre order, shared edges) per §1.7;
exact geometry is proven from tree dumps; one
diagnostic commit is budgeted per wave, not treated
as a failure of discipline; a test is never
"fixed" by weakening it without tree-dump evidence
attached.

**R4 — A State field slips in mid-wave.** One field
= 20 literals + `init` to update, and the tree
compiles only if all are caught (gate check 3
exists because Phase I nearly missed this).
*Mitigation:* F5; check 3 runs at every checkpoint,
not just at integration.

**R5 — Headless evidence silently invalidated.**
If anyone edits `headless.yml`'s `probe()` and
drops the `./projects` wipe or fresh `HOME`, later
snaps can render the wrong workspace and still
"pass" (Phase N runs 42487139921 / 42488210880).
*Mitigation:* the workflow is integrator-owned (F2);
probe-count from `.exit` files (23 today; the count
changes only when W9/W10 add scenes, and the new
count is recorded at CP6/CP7); byte-identical snaps
across different scenes are treated as a leak
signal, not a pass.

**R6 — CI concurrency hides the verdict.** A push
cancels the in-flight run; a watched pipeline's exit
code is not a verdict. *Mitigation:* verdicts are
read only from the newest sha's completed run via
`gh run view` + `--log-failed`, and the run ID is
recorded beside every CI claim (Phase N §7 pattern).

**R7 — Proof surfaces that do not exist yet.** The
brief demands checks at 1280×720 → 2560×1440 (§6)
and equal light-theme attention (§7). Today the
headless snaps render only at the default 1440×900
(the audited Rune flag set has **no window-size
flag** — headless.yml Q3), and **no light-theme
scene or snap exists**. Acceptance lines depending
on those surfaces cannot be signed from existing
evidence. *Mitigation:* W9 must either add the
scenes/snaps (a light-theme scene is straightforward
— scene fns set state; size variants need a Rune
capability check first) or the acceptance record
names the gap as an honest remainder with the
substitute evidence used (tree-scan overflow checks
are size-independent for the side columns; the
user's own desktop captures at other sizes). What
must not happen is signing §6/§7 lines from the
1440×900 dark snaps alone.

**R8 — `main.rs` contention.** S03, S04, S09 and S20
all legitimately need shell wiring. *Mitigation:*
F2 — all `main.rs` edits are integrator-executed
from squad-supplied diffs, serialised, one per wave
at most.

## 5. Acceptance criteria → proof surfaces (§11 mapped)

Every §11 line signs against a named surface. "Pixel
read" always means the integrator's read of the
named snap from the final headless run, recorded in
`docs/QA_RUNE.md` §17 with the run ID.

### Visual quality

| Criterion | Proof surface |
|---|---|
| One coherent design language | Gate check 6 (kit self-check + per-file floor + retired-helper census) on the final tree; check 7 if W10 adds one |
| Professional application shell | Pixel read: `snap-default`; tree scan: stage rect + column widths in `tree-default.json` vs the §1 baselines |
| Attractive project dashboard | **Gap today:** no Home scene/snap exists. W9 adds a `home` scene + snap; until then this line cannot sign (R7) |
| Consistent panels | Check 6(b) panel_col roll (30 of 31 + named exceptions at baseline; the mission's roll printed at CP7) |
| Consistent buttons | Check 6(d) retired-helper census = zero survivors outside named exemptions |
| Consistent icons | Icon-resolution audit (the Phase N V-wave pattern: every glyph literal in production resolves in `icon_data.rs`); gate check 5 icon count |
| Proper typography | Source census of `type_::` uses per panel (squad S02 report, spot-checked at CP7) + pixel reads |
| Beautiful dark theme | Pixel reads: all 10 current snaps |
| Beautiful light theme | **Gap today:** no light scene/snap. W9 adds light variants of at least `default`, `draw-open`, `settings-open`; those pixel reads sign this line (R7) |
| Balanced visual hierarchy | Pixel reads across the per-workspace scene list (§6) |
| Professional timeline | Tree scan of `tree-fx-split-mid.json`: timeline / clips-lane / loop-row rects and gaps (Phase N values as the reference pattern); pixel read: `snap-fx-split-mid` |
| Attractive animation stage | Pixel reads: `snap-default`, `snap-stage` (crop), `snap-three-d-stage`; stage rect from tree scan |
| Properly designed dialogs | Pixel reads: `snap-palette-open`, `snap-menu-open`, `snap-settings-open` |
| No unfinished sections | Empty-state completion census (W8): every bare line named in Phase N §8 either converted or explicitly ruled, listed in the mission doc |

### UX quality

| Criterion | Proof surface |
|---|---|
| Clear navigation / intuitive workspaces | Runtime scene sweep (Phase N round-11 pattern: card roots present per workspace over all eight chips) + pixel reads per workspace (§6) |
| Consistent interactions | W8 state audit + check 6; spot pixel reads |
| Useful empty states | W8 census (above) + the scenes that render them (default scene's project list today) |
| Proper user feedback | Toast severity wired via `classify_status` (Phase N F7) — Runtime tests on status→toast mapping; no panel may hardcode a severity |
| Destructive-action confirmation | Runtime tests: home two-step delete (Phase N A3 precedent), `danger_btn`/`danger_icon_btn` census via check 6 |
| Readable controls | Contrast record: the standing documented acceptances (white-on-accent 4.19:1, short labels/icons only) re-measured if W1 changes tokens; any new acceptance is written down, not silent |
| Accessible keyboard workflows | Help-panel true-key audit (every Shortcuts row is a real binding — the Ctrl+S precedent); **standing decision:** `tab_visits_controls` stays OFF (Phase N §8 — it would let a focused button eat Space/Play and sliders eat frame-stepping). The brief's §8 keyboard line signs against the true-key audit + shortcut preservation (gate check 2), *not* against enabling tab traversal; if the user wants that revisited it is their call, flagged at CP9, not taken silently |
| No confusing duplicate controls | Caller census (check 4) + S01/S20 duplicate-control audit at CP7 |
| No decorative dead buttons | Check 4 (UNWIRED = unfinished) extended in spirit: every control in the per-workspace scene list traced to a handler at CP7 (§11 final verification) |

### Engineering quality

| Criterion | Proof surface |
|---|---|
| Features / handlers preserved | Gate checks 1–2: **804 keys, zero lost**, per-file diff vs the W0 baseline ref |
| Element-key requirements | Same as above; census-repair rule R1 for any new helper |
| No project-format regressions | F5 (no model/serde changes); the existing integration tests in the CI suite |
| Save/load, playback, undo/redo, export unbroken | Full CI suite green on the final sha (2,492 tests at baseline; the count may only grow), run ID recorded; **sound remains user-verified only** — the standing ask (download the build, press Play, drag the playhead) is restated at CP9, not claimed closed |
| No theme regressions | Light + dark snap sets pixel-read (W9 surfaces) |
| No duplicate UI builders | Check 6(d), zero survivors rule |
| No avoidable performance regressions | Default-tree element count from the final headless run vs the 1,018 baseline, inside the +8–11 % budget (F7) |
| GitHub Actions CI passes | Newest sha's completed "Build, Test & Binary" run: GREEN, ID recorded in QA_RUNE §17 (R6 rule) |

## 6. Per-workspace scene list (the review surface)

The 15 scenes (`scenes.rs`; gate check 5 counts them
live) and what each proves. Snaps exist for the 10
marked ●; trees for 13.

| Scene | Workspace / surface | Snap |
|---|---|---|
| `default` | Animate, cold state | ● |
| `frame-3` | Animate, mid-timeline | |
| `selected` | Animate, figure selected | |
| `playing` | Animate, playback state | |
| `loop-range` | Animate, loop range set | |
| `graph-duplicates` | Graph | |
| `fx-selected` | Effects — selects a *particle* event (FX browser + Particles detail) | ● |
| `fx-split-mid` | Effects — the only scene selecting an FX clip; timeline lane geometry | ● |
| `three-d` | 3D workspace | ● |
| `three-d-stage` | 3D figure on the main stage | ● |
| `draw-open` | Draw | ● |
| `palette-open` | Command palette overlay | ● |
| `menu-open` | File menu dropdown | ● |
| `settings-open` | Settings card (also the language-overflow evidence scene) | ● |
| `default` (crop) | Stage crop | ● (`snap-stage`) |

**Coverage gaps W9/W10 must close or the acceptance
record must name:** no dedicated **Build**-workspace
scene (the 3D scenes exercise the 3D session, not
the Build panel set as gated), no **Audio**-selected
scene/snap, no **Export** scene/snap, no **Home**
scene/snap, no **light-theme** variant of anything,
and no size other than 1440×900 (R7). Each added
scene changes the gate's check-5 scene count and the
headless probe count — both recorded at the
checkpoint where it lands, so the final numbers in
the mission doc reconcile.

## 7. Integrator checkpoints (summary)

- **CP0** — baseline gate output + census recorded
  (804 / 43 / 1,018) before any source edit.
- **CP1–CP6** — per wave: gate PASS vs the previous
  wave commit, CI GREEN (run ID), wave report filed
  with the §9 nine items (files inspected, problems,
  changes, components reused/added, behaviour
  preserved, verification, limitations, remaining
  defects).
- **CP7** — verification wave: QA_RUNE §17 opened,
  counts reconciled to gate output, duplicate/dead-
  control trace complete.
- **CP8** — headless final run: all probes exit 0
  (count as amended by W9/W10), every snap
  pixel-read, overflow scan over the tree dumps
  clean or its findings filed as fix commits and
  re-proven.
- **CP9** — acceptance: §5 tables signed line by
  line; honest remainders (expected candidates:
  sound user-verification, the two-"New project"
  disagreement, `tab_visits_controls`, any §6/§7
  surface W9 could not build) written into the
  mission doc; honest agent count stated.

*End of S20 plan.*
