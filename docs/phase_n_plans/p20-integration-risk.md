# Phase N — P20: Integration & Risk Plan

The integrator's playbook for landing a 100-agent
restyle wave without breaking the shell it makes
prettier. Read with the charter
(`docs/PHASE_N_CHARTER.md`): Phase N changes
structure and styling only — no engine behaviour, no
new State fields, every element key byte-identical
(733 at the Phase M baseline, commit `baf5441`).

The core fact this plan is built on: **a restyle wave
fails differently from a feature wave.** Feature
waves break compilation and op wiring; restyle waves
break *geometry, visibility and census visibility* —
elements that still exist but moved, shrank, wrapped,
faded, or became invisible to the gate's regexes. The
Phase M fix history (§6 of `docs/PHASE_M.md`) is the
evidence base: five fix commits, of which only one
was a plain compile error in new code. The rest were
census blindness (shorthand literals), a stale test
premise, a clipped unpressable row, and a Runtime
that does not re-run the view for unmarked state
writes. Every one of those failure modes has a
Phase N analogue, named below.

---

## 1. Adoption order — what lands first, what lands last, and why

### 1.0 Integrator groundwork (before any agent's work lands)

Two edits only the integrator may make, both before
the F wave is integrated:

1. **`src/rune_sec/mod.rs`: add `pub mod design;`**
   (alphabetically, after `command_palette` /
   before `draw_tools` — the file is alphabetical).
   Charter rule: no agent touches mod.rs. Without the
   declaration pre-landed, the design.rs agent's file
   is dead code and every consumer fails to compile.
2. **Gate regex widening for kit helpers** (see §2.1).
   The charter schedules gate widening in the V wave,
   but the *minimal* regex extension must land with
   the F integration: from the first adoption commit
   onward, keys legitimately live inside
   `design::toggle("…", …)`-style calls that the
   current census cannot see, and every batch would
   report phantom key losses. V completes the gate
   (adoption census, §2.2); the regex fix is
   groundwork, not V scope creep.

`src/main.rs` needs **no changes at all** in Phase N,
and that is a rule, not an observation: the app-bar
work lives in `chrome_view.rs`, the timeline card
inside `timeline_marks.rs`'s own returned stack, the
stage chrome in `stage_view.rs`. Any agent whose
work seems to need main.rs has a blocker to report,
not an edit to make. The State census (§2.3) is the
tripwire that proves this held.

### 1.1 F wave — foundation, in this internal order

| Order | File(s) | Why here |
|---|---|---|
| F1 | `rune_sec/design.rs` (new) | Everything imports it. Lands alone, first, so its real signatures (vs the charter's fixed API) are known before any consumer is written against them. Integrator diffs the landed file against the charter API and circulates deviations *before* F2 starts. |
| F2 | `rune_sec/icon_btn.rs` + `chrome_view.rs` | The one button family (charter §4). Every panel consumes these; their metrics (px 10 / py 5, radius 6) shift every button rect in the app. Land once, early, so all later geometry surprises are attributable to one change. The inert twins must keep **identical box metrics** to the live buttons — that contract already exists in icon_btn.rs and is load-bearing for tests that tap a button before and after it flips live/inert. |
| F3 | `rune_sec/workspaces.rs` + `rune_sec/status_strip.rs` | Workspace chips gate every panel mount (a chip restyle that breaks a `ws-*` tap breaks *every* Runtime test's seeding). Status strip carries the Phase K floor arithmetic (fixed segments floored by `est_text_width`, one grow+truncate tail absorber); tick separators and MONO_CAPTION numbers change those widths, so this lands early and its floors are re-derived, not assumed (§3, risk 6). |
| F4 | `rune_sec/menus.rs`, `rune_sec/command_palette.rs`, `rune_sec/toasts.rs`, `rune_sec/settings_popover.rs` | Overlay surfaces. They float above the columns, so their restyle cannot move column geometry; but they are press-tested (menu open/close settle loops, palette selection) and presence-fade-sensitive, so they land as one batch with their Runtime tests watched closely. |
| F5 | `rune_sec/timeline_marks.rs` | The timeline card (charter §13). Moves the whole under-timeline stack (ruler, chips, markers, clips lane, loop row) into a padded card — the centre column's vertical budget changes here. |
| F6 | `stage_view.rs` (+ `chrome_view.rs` transport/app-bar pieces if split) | **Last in F.** Stage radius 12 + floating camera pill touch the most pinned geometry in the repo: stage bounds [328, 144, 744, 522] at 1440×900, the fig3 pass, camera overlay hit-testing, right-press stage selection. Everything else must be stable before this moves. |

F is integrated and pushed as its own commit(s) and
taken **CI-green before the A wave starts** — the
charter says so, and the reason is attribution: if
adoption lands on a foundation that is itself red,
nobody can tell whose change broke a test.

### 1.2 A wave — adoption batches, ordered by blast radius

One file per agent (charter rule). Integrate in
batches; run the full gate after each batch; never
let two batches sit unintegrated (the Wave 11
lesson: finished work left unintegrated for hours is
indistinguishable from lost work).

- **A1 — leaf panels first** (text/steppers only, no
  drag surfaces, few coordinate-sensitive test taps):
  `help_panel`, `stats_view`, `project_tools`,
  `figure_add`, `easing_panel`, `onion_controls`,
  `scene_panel`. These prove the card/header pattern
  at the lowest risk and shake out kit API friction
  while the cost of a kit change is still small.
- **A2 — list panels**: `figures_panel`,
  `library_panel`, `home_panel`, `fx_browser`,
  `fx_clips`, `clips_panel`, `audio_panel`,
  `particles_panel`, `layers_panel`. The `list_row`
  shell and selected-fill land here. `home_panel`
  goes **last in A2**: it reads the filesystem
  (project scan + metadata cache), carries the
  Phase F ".layout" filter, and its rows are the
  tallest in the left column.
- **A3 — inspector/stepper panels**:
  `inspector_figure`, `inspector_segment`,
  `inspector_style`, `camera_panel`, `pose_tools`,
  `draw_tools`, `stroke_inspector`, `text_tool`,
  `three_d_panel`. Stepper metrics (`stepper_value`
  min-width 34, MONO_CAPTION) change many small rects
  at once. `three_d_panel` goes last in A3: its
  preview canvas is asserted by key in Runtime and
  headless probes, and its orbit drag reads
  element-local coordinates.
- **A4 — drag surfaces and the most test-coupled
  panels, last**: `waveform_view`, `graph_panel`,
  `export_panel`, `playback_controls`, `frame_ops`,
  `keyframe_tools`. Reasons: waveform seek and graph
  dragging map pointer x to values through
  element-local geometry (padding shifts break the
  mapping, risk 8); `frame_ops` carries the Phase M
  `flex_wrap` row inside what becomes a padded card
  (the wrap must be re-proven inside the card's
  narrower inner width); `keyframe_tools` carries
  the Phase J overlap fix, whose note/actions rows
  are geometrically pinned by regression tests.

### 1.3 Files that are NOT adopted (and must not appear in adoption diffs)

- `rune_sec/icon_data.rs` — data table, no elements.
- `rune_sec/i18n_wire.rs` — string helpers, no UI.
- `rune_sec/scenes.rs` — headless scene builders
  (state setup, no panels). New UI scenes are V work.
- `rune_sec/stage_camera.rs`, `stage_layers.rs`,
  `stage_fx.rs`, `stage_fig3.rs`, `stage_text.rs`,
  `fig3_preview.rs` — pure geometry: they return
  prims/shapes, they build no `El` trees. Their
  consumer is `stage_view.rs` (F6). An adoption diff
  in any of these is out of charter.
- `src/main.rs`, `src/stage_view.rs` is F6 (above);
  all lib (engine) files — untouched, full stop.

### 1.4 V wave — verification & docs

Runtime round 11 (`snap_tests.rs`, mounted from
main.rs under `#[cfg(test)]` — declared there, not
in mod.rs), new headless UI scenes (`scenes.rs` +
`headless.yml` probe/snap steps, integrator commits
those), gate completion (§2.2), `docs/PHASE_N.md` +
QA_RUNE §16. Docs numbers come only from the gate's
live reads on the final tree (Phase I lesson: a docs
report once described files that did not exist —
the integrator verifies every claimed file on disk).

---

## 2. Gate additions for Phase N

The existing gate (`scripts/phase_j_checks.py`,
checks 1–5) keeps running unchanged in structure.
Phase N adds three things: a regex repair (§2.1), a
new check 6 (§2.2), and frozen expectations (§2.3).
The V-wave gate agent owns the script edits; §2.1's
regex repair is pulled forward to F integration
(§1.0) — the rest lands in V.

### 2.1 Regex repair — the census must see kit calls (do FIRST)

Both key censuses recognise a key only when it is:

- a `.key("…")` / `.key(format!("…"))` literal, or
- the first string argument of a known helper:
  `btn | fbtn | row_toggle | icon_btn | icon_only_btn`
  (inventory: `scripts/rune_inventory.py`
  `BTN_KEY_RE` / `BTN_DYN_RE`; diff:
  `phase_j_checks.py` `_DIFF_BTN_RE`, which also
  knows the inert twins), or
- a `key: "…"` entry field, or a `*_KEY` const value
  (diff only).

Phase N moves keys into new call shapes:

```rust
design::panel_col("panel-camera", t)          // was: col().key("panel-camera")
design::list_row("fig-row-3", sel, t)         // keyed row shell
design::toggle("onion-toggle", on, t, f)      // was: row_toggle("onion-toggle", …)
design::toggle_row("settings-loop", …)        // keyed pressable root (charter §7)
design::header_row("…")                        // if an adopter keys a header
```

None of these callee names is in either regex.
Consequences if unrepaired, in both directions:

- **False loss**: the key diff reports the key LOST
  and the inventory total drops — the Phase H
  artefact exactly (556 → 480 that was really a
  counter gap), except this time it fires on *every
  adoption batch*.
- **False safety**: an agent could delete a key and
  the diff would not notice its absence from the
  tree side either, if the base side is also
  re-expressed — the diff is per-file set difference,
  so both sides must be extracted with the same,
  complete regex family.

**Repair**: add the design helpers that take a key
as first argument — `panel_col`, `header_row`,
`list_row`, `toggle`, `toggle_row` — to
`BTN_KEY_RE` / `BTN_DYN_RE` (inventory) and
`_DIFF_BTN_RE` (gate), static and `format!` dynamic
forms. Also confirm the inventory regexes cover
`icon_btn_inert` / `icon_only_btn_inert` first-arg
keys (the diff regex already does; the inventory
`BTN_KEY_RE` list does not name them — verify and
align). The repair commit must be validated by
re-running the gate on the **unmodified Phase M
tree**: totals must still read exactly 733 (631
static + 102 dynamic). A regex change that moves
the baseline is a bug in the repair, not a discovery.

One more extraction hazard, specific to restyle
diffs: keys passed as *variables* (`let k = …;
design::toggle(k, …)`) or built by helper fns in
the panel file are invisible to every regex. The
charter's "handlers move verbatim" rule mostly
prevents this; the adoption census (§2.2) plus the
key diff together catch the rest — a key that
vanishes from extraction while its panel was
adopted is flagged for manual review, never waved
through.

### 2.2 New check 6 — DESIGN ADOPTION CENSUS

Modelled on check 4 (caller census): for each file
that must adopt the kit, count `design::`
references in the file's **production region**
(before its trailing `mod tests`, same cut as
check 4) and require the file-specific minimum.
A file at zero is UNADOPTED — the restyle-wave
analogue of check 4's UNWIRED.

**Required (≥ 1 production `design::` reference;
panel files additionally ≥ 1 of `panel_col` /
`panel_title`):**

- Foundation: `chrome_view.rs`, `rune_sec/icon_btn.rs`,
  `rune_sec/menus.rs`, `rune_sec/command_palette.rs`,
  `rune_sec/toasts.rs`, `rune_sec/status_strip.rs`,
  `rune_sec/settings_popover.rs`, `rune_sec/workspaces.rs`,
  `rune_sec/timeline_marks.rs`, `stage_view.rs`.
- Adoption panels (all of `src/rune_sec/`):
  `audio_panel`, `camera_panel`, `clips_panel`,
  `draw_tools`, `easing_panel`, `export_panel`,
  `figure_add`, `figures_panel`, `frame_ops`,
  `fx_browser`, `fx_clips`, `graph_panel`,
  `help_panel`, `home_panel`, `inspector_figure`,
  `inspector_segment`, `inspector_style`,
  `keyframe_tools`, `layers_panel`, `library_panel`,
  `onion_controls`, `particles_panel`,
  `playback_controls`, `pose_tools`, `project_tools`,
  `scene_panel`, `stats_view`, `stroke_inspector`,
  `text_tool`, `three_d_panel`, `waveform_view`.

**Exempt (must NOT be required; a `design::`
reference here is suspicious, not virtuous):**
`design.rs` itself (it *is* the kit; census it for
the charter constants/API instead — see below),
`icon_data.rs`, `i18n_wire.rs`, `scenes.rs`,
`snap_tests.rs`, and the geometry producers
`stage_camera.rs`, `stage_layers.rs`, `stage_fx.rs`,
`stage_fig3.rs`, `stage_text.rs`, `fig3_preview.rs`.

**Retired-helper census (same check, second half):**
the private helpers the charter retires must be
gone from adopted files' production regions:

- `fn fbtn` — `frame_ops.rs` (the one local button
  copy; its disabled handling moves onto
  `icon_btn_inert` / the btn family's inert form).
- `fn section(` / `fn section_label(` /
  `fn section_header(` — known instances today:
  `easing_panel`, `export_panel`, `fx_browser`,
  `playback_controls` (`fn section(`);
  `figure_add`, `home_panel`, `settings_popover`,
  `three_d_panel` (`fn section_label(`);
  `command_palette` per the charter audit. The
  census greps the pattern repo-wide in
  `src/rune_sec/*.rs` production regions, so
  instances the audit missed are caught too.
  Any survivor needs a named exception in
  PHASE_N.md, or it is a fail.

**Kit self-check (same check, third half):**
`design.rs` must define every charter-fixed item:
constants `PANEL_RADIUS 12.0`, `PANEL_PAD 12.0`,
`PANEL_GAP 8.0`, `ROW_RADIUS 8.0`,
`CONTROL_RADIUS 6.0`, and fns `accent`,
`selected_fill`, `panel_col`, `header_row`,
`panel_title`, `section_label`, `divider`, `kbd`,
`empty_state`, `list_row`, `toggle`, `toggle_row`,
`stepper_value`, `slider_track`. Census by name,
like check 4's op census — a consumer written
against a charter fn the builder renamed is a
compile error CI will find, but the census finds
it before the push.

### 2.3 Frozen expectations — what must NOT move

- **Key diff base = `baf5441`** (Phase M close-out,
  the final Phase M tree). Not HEAD: the diff must
  cover exactly Phase N. **Zero lost** is the only
  passing result; every addition is listed in
  PHASE_N.md with its brief's authority (charter:
  new keys only where a brief allows — expected
  additions: any new row/header keys design helpers
  introduce, round-11 test scenes' keys are not
  element keys and do not count).
- **Inventory total ≥ 733**, and the static/dynamic
  split is reported; a *drop* in either is
  investigated as a §2.1 extraction gap before it
  is believed as a loss.
- **State census IDENTICAL to Phase M**: 43 fields,
  20 full literals, every field present in every
  literal. Phase N adds no State fields (charter),
  so any change in these numbers — a field more, a
  literal more, a field renamed — is a charter
  breach, not a new baseline. Check 3 passes
  *unchanged* or the phase stops.
- **Caller census unchanged**: all Phase J+K+L+M
  ops (check 4, incl. same-file `swap_frames` /
  `delete_camera_key_at`) still wired. A restyle
  that restructures a panel can orphan a call
  (e.g. `stage_fig3_prims` has exactly one call
  site, in stage_view.rs — the F6 file).
- **Count claims frozen**: palette **108**, MCP
  registry **91** (total 102), icons **111**. None
  of these files is adopted for content; any drift
  means a restyle diff wandered. Scenes: **11** at
  phase start; V may add UI scenes — the final
  number is whatever the gate's live read prints,
  and PHASE_N.md must match it.

---

## 3. Top-10 integration risks for a restyle wave

1. **Census blindness to kit call shapes.**
   Covered in §2.1 — listed here because it is the
   likeliest failure of the whole phase: not broken
   code, but a gate crying wolf on every batch
   (phantom LOST keys, falling inventory) until
   nobody trusts it, followed by a real loss waved
   through. Mitigation: regex repair lands and is
   baseline-validated (733 exactly) *before* F2.

2. **`find()`-centre taps land on the wrong thing.**
   Runtime `tap()` presses the centre of the keyed
   element's drawn rect. Cards add padding and
   borders; list rows gain hover/selection shells;
   headers insert new rows above controls. A
   control's centre can move onto a child that does
   not forward the press, or the keyed root can
   become a *shell* (`list_row` carries no handler,
   charter §5) whose centre sits between its inner
   buttons. Phase M fix 4 is the precedent: the
   press missed entirely and the op simply never
   ran. Mitigation: round-11 tests re-tap every
   control whose root was restructured, not just
   whose handler moved; failures are diagnosed as
   "press never arrived" (check status/undo depth)
   before "op is broken".

3. **`scroll_into_view` assumptions silently expire.**
   The helper wheels at a fixed point (1255, 450) —
   the right column — up to 24 times, breaking when
   the key's centre y < 850, then settles ≤ 90
   frames for opacity. Card padding (12 top + 12
   bottom per panel), dividers and headers make
   both columns substantially taller: deep controls
   (frame_ops' wrapped row inside a card,
   stroke_inspector, keyframe_tools) sit further
   down than any previous phase put them. If 24
   wheel steps no longer suffice, the failure looks
   like risk 2 (a missed tap) with a different
   cause. The fixed wheel point also presumes the
   right column's scroll viewport still covers
   (1255, 450) after the app-bar/workspace rows
   change height. Mitigation: V wave re-verifies
   every key any test scrolls to; if the cap is the
   problem, raise the helper's iteration cap in
   snap_tests (test-only change) and record it —
   do not shrink panels to fit the helper.

4. **Presence fades on restructured subtrees.**
   Rune keeps unmounted elements presented during
   fade-out and fades restructured subtrees in.
   Wrapping a panel's children in a card, a header
   row, or `list_row` shells creates new unkeyed
   intermediate nodes; on every workspace switch
   those subtrees re-enter. Consequences seen in
   earlier phases (F/G): absence assertions need
   the ≤ 30-frame settle loop; *presence*
   assertions right after a workspace tap can fire
   while the panel is still fading (presented but
   below full opacity — `tap` during the fade can
   also miss, since presses test through the
   presented tree). Duplicated keys can exist
   transiently (exiting copy + entering copy), so
   `find()` may return the exiting element's rect.
   Mitigation: settle loops both directions in new
   tests; never assert on the same frame as a
   workspace switch.

5. **Direct state writes without `invalidate()`.**
   Phase M's clips-lane test failed twice on this:
   the Runtime re-runs the view only when state is
   marked changed, so a test that writes `rt.state`
   directly (planting a selection to see the
   selected fill, planting clips, forcing a toggle)
   must call `rt.invalidate()`. Restyle tests are
   *more* likely to plant state than feature tests
   were, because visual states (selected, on/off,
   muted, hidden) are exactly what they check.
   Mitigation: every round-11 test that writes
   state directly calls `invalidate()` before the
   frame loop — put it in the round's brief, not in
   a post-mortem.

6. **Pinned layout budgets move.**
   Three hard numbers from earlier phases:
   columns 300 / 744 / 340; stage bounds
   [328, 144, 744, 522]; status-strip floor
   arithmetic (Phase K: floors sum 512 + audio 64,
   + 17.75 for a glyph = 593.75 ≤ 736 usable — the
   reason the Audio segment has no glyph). The
   timeline card's padding (§13) eats centre-column
   height; tick separators and MONO_CAPTION change
   status segment widths (the tail absorber's
   `fit_hint` cap and `est_text_width`'s 8.0/char
   assumption are tuned to the old font). Any of
   these moving fails the Phase J geometric
   regression tests. That failure is *information*:
   either the change is an intended re-baseline
   (document the new numbers in PHASE_N.md and
   update the pinning test in the same commit, with
   the reason) or it is drift (fix the panel).
   Never just widen a pin to make red go away.

7. **Live/inert metric drift in the button family.**
   The shell leans on live and inert button twins
   having identical box metrics, so a control that
   flips state (Swap at the last frame, Mute with
   no clip) does not jump. If the kit restyle
   changes metrics in `icon_btn` but not
   `icon_btn_inert` (or `btn` vs its disabled form,
   or `toggle` on vs off track alignment), every
   state flip shifts layout — and tests that find()
   a button, flip its state, and tap again will
   tap stale coordinates. Mitigation: the F2
   agent's report must state the metric table for
   live and inert forms side by side; round-11
   includes one flip-and-tap-again test per twin
   pair that changed.

8. **Drag-coordinate origins shift under padding.**
   Several panels map pointer position to a value
   using element-local coordinates: waveform seek
   and span bracket, layers opacity slider, scene
   weather sliders, graph canvas keys and tangent
   handles, timeline ruler drag-seek and loop-edge
   dragging (`EDGE_PX 8.0`), fig3 preview orbit
   (travel-since-press via a thread_local last
   position), stage drags (hit_test r = 13). The
   charter keeps drag logic where it lives and
   makes `slider_track` visual-only — the risk is
   an adopter wrapping a drag surface in a padded
   container, or moving the surface inside the card
   such that the local origin the maths assumes is
   no longer the element's origin. Symptom: every
   drag is off by a constant (the padding), which
   looks like an engine bug and is not.
   Mitigation: A4 goes last (§1.2); each A4 brief
   names its drag surfaces and forbids padding
   between the pressable element and its drawing;
   round-11 replays one drag per surface.

9. **Large mechanical diffs resurrect Phase M's
   patch failure modes.** Restyle edits are wide,
   repetitive, and tempting to script: the recorded
   traps are (a) regex/literal patchers matching
   inside format strings and char literals —
   brace-counting must run over comment/string-
   blanked source; (b) double-insertion when two
   anchors resolve to the same block, needing
   dedupe + a duplicate-collapse pass; (c)
   shorthand struct literals invisible to
   colon-field scans (Phase M fixes 1–2: two whole
   CI rounds). Phase N touches no struct literals
   by charter — but it rewrites hundreds of builder
   chains, where the analogue is a `.child(` inserted
   at the wrong nesting level, silently reparenting
   a subtree (compiles fine, looks wrong, may move
   a key under a gated parent). Mitigation: one
   agent per file, no scripted cross-file rewrites
   by the integrator; per-file diffs reviewed for
   reparenting before commit; key diff per batch
   (§2) catches keys that moved under a workspace
   gate because per-file extraction still finds
   them — but a key moved to a *different file*
   shows as lost+added, which the per-file diff
   reports as a loss in the origin file. Treat that
   report as a prompt to verify the move was
   intended, never as noise.

10. **Headless evidence goes stale quietly.**
    `--tree` dumps gain wrapper nodes everywhere
    (cards, headers, dividers); probes that grep
    for keys survive, probes that count nodes or
    match paths do not. `--snap` images change
    completely by design, so the Phase L/M
    pixel-proof habit must be re-applied fresh:
    stage containment (the fig3 grid stayed inside
    the stage — now check it against a *rounded*
    stage rect: canvas prims clip to the rect, and
    corners may paint square over the radius),
    no clipped button rows (the Frames card is the
    specific re-check — wrapped row, narrower
    card), columns still 300/744/340 or the
    documented re-baseline from risk 6. A headless
    run that is "all exit 0" proves the app runs;
    only reading the snaps proves the restyle
    landed. Mitigation: close-out checklist (§4)
    makes snap-reading a named step with named
    snaps, not a vibe.

---

## 4. Close-out checklist

In order; the phase is not done until every box is
ticked on the final tree.

- [ ] **Gate PASS on the final tree**, base
      `baf5441`: inventory ≥ 733 with the split
      reported; key diff **zero lost** (additions
      listed in PHASE_N.md); State census identical
      to Phase M (43 fields / 20 literals); caller
      census all wired; count claims read
      palette 108 / MCP registry 91 / icons 111 /
      scenes = the live number docs will print.
- [ ] **Check 6 PASS**: every required file (§2.2)
      at or above its `design::` minimum; retired
      helpers (`fn fbtn`, `fn section(`, `fn
      section_label(`) absent from production
      regions or named as exceptions in PHASE_N.md;
      `design.rs` contains the full charter API.
- [ ] **CI green on the final sha** — verdict from
      `gh run view`, never the watch pipeline's
      exit code (Phase K lesson). Test totals
      recorded from that run's log (lib + bin +
      integration), not from section reports.
- [ ] **Foundation was CI-green before adoption
      started** (F commit(s) named in PHASE_N.md);
      adoption integrated in batches A1–A4, no
      batch left unintegrated overnight.
- [ ] **Headless re-dispatch on the final tree**:
      every probe exit 0, including any new V-wave
      UI scenes; tree dumps spot-checked for the
      panel keys the phase touched most
      (`panel-3d-preview`, `timeline-clips`,
      `timeline-loop-row`).
- [ ] **Snaps READ by the integrator** (not just
      produced): default, three-d-stage, and any
      new UI scene snap — checking stage
      containment at the rounded corners, the
      Frames card's wrapped row fully pressable-
      looking inside the card, no text clipped in
      the status strip's tail, columns at
      300/744/340 or the documented re-baseline.
      Save to `your_files/` as
      `stykanimate-phase-n-*.png`.
- [ ] **Round-11 Runtime tests** green inside the
      CI total above, including: one flip-and-tap
      test per changed live/inert twin (risk 7),
      one drag replay per A4 surface (risk 8),
      settle loops on restructured panels (risk 4),
      `invalidate()` after every direct write
      (risk 5).
- [ ] **Docs**: `docs/PHASE_N.md` written from the
      tree (every claimed file verified on disk —
      Phase I lesson), QA_RUNE §16 cells filled
      with the green run id + final counts, fix
      loop (if any) summarised with each failure's
      actual cause from its run's log.
- [ ] **MEMORY.md** Phase N entry appended.
- [ ] **Final report** to the user with a snap
      attached: what the shell looks like now,
      the counts, any re-baselined layout numbers
      with reasons, honest remainders — and the
      standing sound ask, brief (download the
      green run's build, press Play, drag the
      playhead; still unverifiable from here).
