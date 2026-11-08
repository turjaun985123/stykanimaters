# Phase N — the UI/UX phase: one design kit, every panel a card

Phase N is a **visual and interaction-design phase**.
It changed how the shell looks and feels — panel
cards, headers, controls, lists, menus, feedback —
and **no engine behaviour**: every op, handler,
shortcut and status string works exactly as before;
only structure and styling moved. It opened from
the groundwork audit's diagnosis (charter, "Why"):
panels were flat bare `col()`s with hand-rolled
text headings, at least five private section-label
helpers, three button implementations plus per-file
dialects, and steppers / toggles / sliders / list
rows / swatches rebuilt ad hoc in almost every
panel — while `rune::tok` already shipped the
spacing, radius, font and theme tokens the shell
barely used.

> **A note on provenance.** Written by the phase's
> docs section under the rule Phase J established:
> every claim below was checked against the tree,
> the gate or the git record before it was written.
> The verification wave was still landing when this
> census ran, so the final-CI and headless cells in
> §8 are marked **_integrator_** for the integrator
> to fill from the final run, as in Phase M
> (filled at close-out; see §7). Where
> a number comes from a plan rather than the tree,
> the plan is named.

## 1. What Phase N set out to do

The user's order (2026-11-08): *"Assign 100 agents
some for planning others will write code, main
focus is making UI, UX components and proper use
and make it great looking app as professional
useful and very satisfying UX and UI."*

The charter (`docs/PHASE_N_CHARTER.md`) fixed the
design language — **"card + hairline"**, 15 rules —
and a fixed kit API before any agent wrote code:

1. Every panel a card (fill `t.card`, border 1.0
   `t.card_border`, radius 12, padding 12, gap 8;
   root key unchanged). 2. One header (13px icon,
   SECTION title, LABEL meta, hairline divider
   under). 3. Section labels from the kit only.
4. One button family. 5. List rows (min-h 28,
   selected = accent @ 16%). 6. Steppers (label
   grows, −, centred mono value min-w 34, +).
7. One pill toggle (28×16). 8. Sliders keep their
   drag logic, take the kit track visual. 9. Menus /
   popovers / palette as cards with `kbd` hint
   chips. 10. Empty states from the kit. 11. `kbd`
   chips for help / menus / palette footer.
   12. Status strip: 1×10 separator ticks, mono
   numbers. 13. The timeline stack in one card.
   14. Stage radius 12; camera overlay a floating
   pill. 15. App bar: TITLE name, accent dirty dot,
   hairline under bar + workspace row.

The charter also states its honest limit up front:
Rune's presence/spring system is not a per-control
motion toolkit, so the phase promises hierarchy,
spacing, states and consistency — not animation
curves it cannot tune.

**The agent count, honestly.** The order said 100.
The waves as executed: **20 planning** (the 20 plan
files in `docs/phase_n_plans/`, 9,451 lines, are on
disk), **10 foundation** (F1–F10, §4), **32 adoption
+ 1 stage audit + 1 spillover = 34** (§5), and a
verification wave (§7) of a dozen-plus tasks, of
which at least ten left a trace in the tree or a
delivered audit report. The verifiable total is
therefore in the **high 70s, short of the ordered
100** — the shortfall is stated here rather than
padded with make-work, and the integration work
(charter, census repair, main.rs wiring, the CI
loop) was done by the integrator, not delegated.

## 2. Groundwork: plans, amendments, census repair

- **Planning wave (P01–P20).** Twenty audits/specs,
  one per surface or cross-cutting concern, each
  ending in a file-scoped work list. The findings
  that shaped the phase: P02 (the `tok` tokens had
  zero shell references; a third text level must be
  derived, never a new hex), P03 (7 button dialects,
  11 heading helpers, 10 stepper helpers), P05 (the
  root gap: `btn` had no inert twin), P06 (timeline
  card + stage rulings), P10 (**toast Severity was
  fully built but never used** — main.rs hardcoded
  `Severity::Info`), P12 (contrast, §9), P14
  (keyboard/focus inventory + the stale Delete help
  row), P16 (mount-order moves — Animate's right
  column ran ≈3,682pt of scroll with Playback
  buried), P17 (verification plan), P19 (default
  tree 942 elements; growth budget +8–11%), P20
  (integration risk; §2.1 below).
- **Charter amendments (ratified by the integrator
  after planning).** Kit extensions `divider_keyed`
  (P7: menu separators are keyed), `row_shell` (P8:
  unkeyed text segment rows), `text_muted` (P2:
  `text_secondary` @ 0.6 alpha), `swatch` (P3/P18:
  one 24px swatch); button-family additions
  `btn_inert`, `btn_muted_live`, `danger_btn` +
  named constants, `chip_btn`, `danger_icon_btn`
  (P5/P12); and the rulings: charter stepper order
  even where a panel differed; the wind slider
  keeps its bipolar visual; the palette's selected
  row keeps accent 0.85; the timeline card is
  **unkeyed**; status mono = timecode + speed only;
  toast severity wired via `classify_status`;
  mount-order moves are integrator-owned;
  `tab_visits_controls` stays OFF (§9); white on
  accent 4.19:1 accepted for short labels/icons
  only (§9); restyle roots, never nest wrappers.
- **Gate census repair (P20 §2.1 — the phase's most
  important repair).** Both key censuses recognised
  keys only as `.key("…")` literals or first
  arguments of known helpers — and the old helper
  lists were blind to live keys passed to `ctl` /
  `ictl` (three_d_panel), `kbtn` (keyframe_tools),
  `pbtn` / `pibtn` (pose_tools) and `danger_btn`
  (the inspectors). The Phase M headline "733 keys"
  was therefore an undercount. The repair extended
  both censuses with the existing dialects plus all
  Phase N helpers, and validated on the **unmodified
  Phase M tree: 788 unique keys (679 static + 109
  dynamic), a strict superset of the 733 — zero
  baseline keys lost by the repair**, +55 newly
  visible (the ~25 3D `ctl`/`ictl` keys, ~17 pose
  `pbtn`/`pibtn` keys, `del-seg`, `fig-del`,
  `key-copy`/`key-paste`, 6 test-only keys). Phase
  N's zero-lost baseline is 788, diffed with the
  repaired census applied symmetrically.

## 3. The kit — `src/rune_sec/design.rs`

One module, the charter API as fixed, **zero
deviations** (V-wave kit audit: 23/23 charter items
present, no signature drift). Constants
`PANEL_RADIUS 12.0`, `PANEL_PAD 12.0`,
`PANEL_GAP 8.0`, `ROW_RADIUS 8.0`,
`CONTROL_RADIUS 6.0` (design.rs:62–71); builders
`accent` (:77), `selected_fill` (:84, accent @
0.16), `text_muted` (:92), `panel_col` (:102),
`header_row` (:119), `panel_title` (:130),
`section_label` (:148), `divider` (:156),
`divider_keyed` (:163), `kbd` (:173),
`empty_state` (:192), `list_row` (:223),
`row_shell` (:230), `toggle` (:273), `toggle_row`
(:283), `stepper_value` (:305), `slider_track`
(:345), `swatch` (:381). No kit function invents a
key — the file's only `.key(` calls apply the
caller's key. Rune facts the kit encodes:
`.hover()`/`.press()` are overlays over the fill,
not replacement fills; `Color::alpha` replaces,
never multiplies; `MONO_CAPTION` alone does not
set the Mono family, so `kbd` and `stepper_value`
apply `.mono()` explicitly.

The family additions around it: `chrome_view`
gained `btn_inert` (src/chrome_view.rs:87),
`btn_muted_live` (:116 — muted visual, handler
KEPT, the frame_ops refusal semantics) and
`danger_btn` (:171) over the P12-verified deep
reds `DANGER_FILL 0xC93A44` / `DANGER_HOVER
0xB2323E` / `DANGER_PRESS 0x9C2934` (:146–150);
`icon_btn` gained `chip_btn` (:371) and
`danger_icon_btn` (:420, unknown icon →
`chrome_view::danger_btn`), and `PRESS_SCALE
0.97` (:101) on the live builders only — the
inert twins carry none (P15's M1, the phase's one
motion win alongside menu/toast entrances).

## 4. Foundation wave (F1–F10)

Commit `b5aac0d` (36 files, +11,594/−257: charter,
20 plans, the census-repaired scripts, the kit,
the F files and the integrator's main.rs wiring).

| § | File | What landed |
|---|---|---|
| F1 | src/rune_sec/design.rs | The kit (§3): full charter API + amendments, zero deviations; the pill toggle's track is its pressable root (28×16, knob inset 2, x=2 off / x=14 on); `slider_track` standardises on `control_track` rail + `t.knob` knob |
| F2 | src/chrome_view.rs | `btn` gains only `.press_scale(0.97)`; `btn_inert`, `btn_muted_live`, danger constants + `danger_btn` (§3); app bar: project name in a growing cluster + an unkeyed 8×8 accent **dirty dot** when `edits_since_autosave() > 0`; transport Duplicate/Delete frame converted to `icon_btn` (`copy` / `trash-can`), keys/labels/handlers identical |
| F3 | src/rune_sec/icon_btn.rs | `PRESS_SCALE` on the live builders; `chip_btn` (row_toggle's own metrics: px 6 / py 3 / radius 5, 12px glyph) absorbing the chip dialects; `danger_icon_btn` unifying the two drifted danger copies |
| F4 | src/rune_sec/icon_data.rs | **52 curated icons added → 163.** Extraction proven against disk first: all 111 pre-existing entries matched their `assets/icons/fontawesome/solid/*.svg` byte-for-byte, then all 163 re-verified (width = viewBox, `d` = the single path verbatim) |
| F5 | src/rune_sec/menus.rs | Dropdown card radius 8→12, padding 4→6; M2 entrance (`Transition::offset(0,-4)` + opacity on `pair::SNAP`); fixed 18-wide leading icon slot; hints as `design::kbd`; the five keyed separators via `divider_keyed` with the **same keys**; `shortcut_hint` still exactly 5 entries |
| F6 | src/rune_sec/command_palette.rs | Border literal `WHITE.alpha(0.10)` → `t.card_border`; selected row keeps accent 0.85 (charter ruling); local heading retired to `section_label`; empty state via kit; footer prose → interleaved `kbd` chips; a 17-entry `command_hint` table with a drift-guard test against `help_panel::shortcut_rows()` (one documented exception at landing — Ctrl+S — retired by the V-wave copy pass, §7) |
| F7 | src/rune_sec/toasts.rs | `classify_status()` (:198): pure severity from real engine strings — Error = contains " failed", Warn = "No "/"Nothing " refusals, Success = "Exported "/"Saved ", else Info — closing P10's never-used-Severity finding; cards radius 8→12 with a 3pt severity bar; stack gap 12→8; M3 entrance (24pt slide + fade on `pair::SETTLE`) |
| F8 | src/rune_sec/status_strip.rs | Only three changes: dividers 1×12 `control_track` → **1×10 `t.separator`** (:849); mono (MONO_CAPTION + `.mono()`) on the timecode and speed segments only; `GLYPH_TEXT 0xF1F3F7` deleted for `t.text`. Phase K's floors and the audio-glyph refusal untouched |
| F9 | src/rune_sec/settings_popover.rs | Root → `panel_col("settings-card")`; header → `panel_title` (gear) + divider; local `section_label(t, label)` deleted for the kit's `(label, t)` at all sites; stage steppers take `stepper_value`; the four toggle rows take `toggle_row` with the same keys; M2 entrance on the card |
| F10 | src/rune_sec/workspaces.rs | Verified already-consistent on 3 of 4 items with evidence (chips already family `btn` with `ws-*` keys and "· Alt+n" labels; blurb already `text_secondary` — `text_muted` deliberately NOT applied, it would dim rather than move). One change: chips row gap 4.0 → `PANEL_GAP` (8.0) |

**Integrator wiring (foundation, main.rs).**
(1) `poll` pushes toasts with
`toasts::classify_status(&s.last_status)`
(main.rs:333) instead of the hardcoded
`Severity::Info`. (2) P16's four mount-order moves
in the right column: Graph before KeyframeTools;
ThreeD + Playback + FrameOps promoted above
Camera; Scene demoted below the
Particles/Audio/Waveform/Clips block. (3) The
waveform card wrap: `panel_col("waveform-card")`
(main.rs:674) + `panel_title` (wave-square) +
divider + the canvas at its exact [308, 56] —
**the phase's one new production key**, the
waveform being the one canvas main.rs mounts
directly. (4) Stage canvas radius 8.0 → 12.0
(main.rs:733). (5) Charter §15's hairline: a
`design::divider` between the workspace row and
the main row. Plus `pub mod design;` in
rune_sec/mod.rs and the icons pin 111 → 163.

**Foundation CI loop.** `b5aac0d` → run
42481461461 FAILED on one Runtime test
(`prev_next_key_jumps_between_camera_keys`). The
first fix (116af6a, a toast-expiry workaround)
rested on a wrong hypothesis and was reverted; a
one-off diagnostic commit (84ddb37) produced the
evidence — pb-next-key's centre sat at **y = −564**
with toasts empty: after the Playback promotion,
`scroll_into_view` only scrolled down, and `find()`
returns the drawn rect's centre, so an element
above the scroll offset reports a negative y the
old check read as "in view". Fix `5dfed00` made
the helper bidirectional → **run 42482498164
GREEN: 2,475 tests**. Census after foundation:
**808 unique keys** (699 static + 109 dynamic) —
the +20 over 788 is `waveform-card` plus ~19
test/branch keys in the new foundation files (the
census counts test code symmetrically).

## 5. Adoption wave (A1–A4 + audit + spillover)

32 panel files adopted in four batches of eight,
one agent per file, under the standard brief:
root → `panel_col` (existing key), header →
`panel_title` + `divider`, local labels →
`section_label`, dialects retired onto the family
(muted-but-live → `btn_muted_live` with the
handler kept; inert → `btn_inert` / inert twins),
list rows → `list_row` / `row_shell`, steppers →
charter order with `stepper_value`, swatches →
`design::swatch`, empty states → `empty_state`;
every key byte-identical, handlers verbatim, no
new State fields. A verified "already consistent"
beat churn. Commit `a570d60` (31 files,
+2,644/−1,910); gate PASS with **zero keys lost**;
**CI run 42484494999 GREEN: 2,478 tests** (996 lib
+ 1,472 bin + 10 integration).

- **A1 (leaf panels):** figure_add, onion_controls
  (steppers reordered to charter order, U+2212
  "−"), easing_panel, stats_view (stat cells now
  sit on `t.bg` wells inside the card),
  project_tools, help_panel (true-key rows take
  `kbd`; the 30 Features rows deliberately do NOT
  — a chip would claim a binding that doesn't
  exist), playback_controls, export_panel (verified
  mechanically: the sorted multiset of every quoted
  string literal in the file is identical to HEAD).
- **A2 (lists):** fx_browser, particles_panel,
  layers_panel (its `SLIDER_H` copy 22 → 28),
  clips_panel (**1-based `clip-row-{n}` keys
  preserved, not "fixed"**), figures_panel,
  library_panel, fx_clips (swatches 18×18 → kit
  24px; strip geometry untouched), text_tool
  (unkeyed segment rows → `row_shell`; the loaded
  instance's row now carries the selected fill —
  previously unmarked).
- **A3 (inspectors/forms):** inspector_segment
  (`del-seg` → `danger_icon_btn`, local danger
  copy deleted), inspector_figure (`fig-del` →
  `chrome_view::danger_btn`), inspector_style,
  camera_panel (overlay → the charter §14 floating
  pill), scene_panel, pose_tools (`pbtn`/`pibtn`
  deleted onto `btn`/`btn_inert` + icon twins),
  home_panel (rows now mark the currently open
  project, derived from existing state; two-step
  delete's confirm → `danger_btn`), audio_panel.
- **A4 (drag surfaces + timeline):** draw_tools
  (the bare row became a real card), stroke_inspector,
  graph_panel, keyframe_tools (`kbtn` → `btn` /
  `btn_inert` per-site), frame_ops (`fbtn` →
  `btn_muted_live` / `btn_inert` per-site; the
  Phase M **`flex_wrap` production fix verified
  intact**), three_d_panel (`ctl`/`ictl` deleted
  onto the family; preview canvas still exactly
  308×220 as a direct card child), timeline_marks
  (the unkeyed timeline card of charter rule 13 —
  §6's named exception). **stage_view: audit
  only** —
  verified pure canvas, no panel chrome, file
  untouched. **Spillover:** inspector_figure's
  colour swatches (left by A3's brief) converted
  to `design::swatch`.

**Ratified deviations** (each flagged by its agent,
decided by the integrator, recorded here):

- **Easing picker cells: restyle-not-replace.**
  Each cell carries a 64×38 sampled-curve canvas
  (a test-pinned feature) and `chip_btn` has no
  content slot; the shell was aligned instead
  (`design::accent()`, `CONTROL_RADIUS`,
  `press_scale`), padding kept at px 4 for width.
- **Graph preset chips: the same class** —
  `graph_panel`'s `preset_chip` carries its own
  drawn preview-curve canvas; it survives as a
  named exemption in gate check 6 (§7).
- **Scene wind slider: bipolar exception**
  (charter amendment). The unipolar amount slider
  takes `design::slider_track` (scene_panel.rs:829);
  the wind slider keeps its centre-fill visual with
  kit colours. Both `SLIDER_H` copies (scene_panel
  :791, layers_panel :305) grew 22 → 28 together.
- **inspector_style preset chips lost their colour
  squares.** Converted to `chip_btn`, which has no
  content slot, so the 12×12 colour square beside
  each preset name is gone; the preset names
  identify them. The two local builders survive
  only as one-line delegating wrappers
  (`cap_chip` :660, `preset_chip` :679).
- **Audio icons follow P13, not the brief's
  summary.** Where the commissioning brief's
  parenthetical disagreed with P13's keyed tables,
  the agent followed the plan: `shuffle` →
  `audio-xfade-{i}`, `arrow-trend-up` →
  `audio-pitch`, `wave-square` → `audio-split-sil`,
  `volume-high` → normalise (`audio-norm-last`,
  `audio-norm-{i}`), plus `filter` → lowpass and
  `compress` → comp — all verified curated.
- **figures_panel rows keep their two-line `col`
  anatomy** with the kit's row styling applied to
  the root rather than nesting `list_row` inside —
  the charter's "restyle roots, never nest
  wrappers" applied literally.
- **help_panel section headers use `panel_title`,
  not `section_label`** (P11's type table: titles
  take SECTION / `t.text` + glyph).
- **frame_ops' `frames` key moved** from the inner
  row onto the card root — the key set is
  unchanged and presence assertions are unaffected,
  but `find("frames")` now returns the card rect.
- **three_d_panel zoom steppers take no
  `stepper_value`** — there is no readout element at
  that site; the distance lives in the Orbit
  readout line.

**The 808 → 804 census artefact.** Post-adoption
the inventory reads 804 unique keys (699 static +
105 dynamic). The gate's per-file diff against the
foundation HEAD reports **zero keys lost in every
file**; the −4 is two census artefacts, each
disproven at source level (the Phase H/K lesson: a
counter is a claim too). (1) inspector_style
merged its two channel-row builders into one
`channel_row_impl` — the base carried both a
`style-{lower}-…` template set and a
`style-{stem}-…` set rendering the same keys
(`style-r-dn`, …); one template text now renders
them all (−3 patterns). (2) pose_tools now binds
`format!("pose-copy-to-{}", …)` to a `let key`
variable before the `btn` call — a documented
census blind spot (variable-passed keys, the same
class as three_d_panel's table keys); the format
string and the rendered key are byte-identical
(−1 pattern). **Zero rendered keys were lost.**

## 6. Verification wave (V)

- **Scenes 11 → 15** (scenes.rs): `palette-open`
  (via the pub `command_palette::open_palette`),
  `menu-open` (`MenusState::toggle(FILE)`),
  `settings-open` (`settings_popover::toggle_open`),
  `draw-open` (`select_workspace(Draw)`) — appended
  at table positions 11–14, with layout facts 15–18
  and one unit test each; the table test now pins
  fifteen. headless.yml runs 13 tree scenes in both
  loops and **10 snaps** (the existing four plus
  palette-open, menu-open, settings-open, draw-open
  and two Effects snaps: `fx-selected` — the scene
  so named selects a *particle* event and shows the
  FX browser + Particles detail — and
  `fx-split-mid`, the only scene that selects an FX
  clip).
- **Runtime round 11** (snap_tests.rs): P17's list
  — card roots present in Animate, card rects
  contained in their columns (relations, not
  re-pinned Phase M numbers), a workspace card-root
  sweep over all eight chips, Edit-menu dropdown
  after restyle, palette/settings cards render and
  close, the timeline card keeping chips/marks/rows,
  shell furniture survival. The file stands at 49
  `#[test]`s at this census (39 at Phase M's close:
  the 9 firm tests + the help-panel conditional,
  whose precondition the copy pass landed) —
  confirmed against the final CI log: run
  42488645714 on `ebdda27`, bin 1,486 passed /
  0 failed, round 11's 49 tests among them. The stage (the centre column's grow
  element) absorbs the timeline card's +18pt, and
  round 11 asserts relations rather than re-pinned
  Phase M numbers, per P17.
- **Gate check 6 — DESIGN ADOPTION**
  (scripts/phase_j_checks.py, per P17 §5 / P20
  §2.2): the kit self-check (5 consts by name **and**
  value, 14 charter fns; extras `divider_keyed`,
  `row_shell`, `swatch`, `text_muted` reported); a
  per-file adoption floor (≥ 1 production
  `design::` reference in each of the 31 adopted
  files; observed counts 1–17) with the panel_col
  roll — **30 of 31**; a kit consumer census
  (report-only); a retired-helper census (13 names:
  `fbtn`, `kbtn`, `pbtn`, `pibtn`, `ctl`, `ictl`,
  `row_toggle`, `cap_chip`, `preset_chip`,
  `danger_btn`, `section`, `section_label`,
  `section_header` — zero surviving definitions
  outside the family homes, modulo the named items
  below); and the count-claims line `design kit
  consumers: 36 files`. The items P20 §2.2 requires
  named in this document:
  - **timeline_marks panel_col exception**
    (`PANEL_COL_EXCEPTIONS`): charter §13 designs
    the timeline card deliberately UNKEYED and
    hand-built (a plain col with
    `design::PANEL_RADIUS`, card fill, hairline);
    `panel_col` would force a key on it. It is the
    one adopted file with no `panel_col` call.
  - **graph_panel preset_chip exemption**
    (`RETIRED_EXEMPTIONS`): the chip carries its own
    drawn preview-curve canvas; `chip_btn` has no
    content slot (the easing-picker precedent).
  - **Delegating wrappers, not survivors:**
    inspector_style's `cap_chip` (:660) and
    `preset_chip` (:679) are single delegations to
    `chip_btn`; fx_browser's `section` (:271) is
    literally `design::section_label(label, t)`.
    Check 6 classifies a definition as delegating
    only when its body calls the kit/family and
    constructs no element of its own.
  - **Zero-consumer kit items** (consumer census +
    docs audit agree): `text_muted` has no
    production consumer (F10 deliberately did not
    apply it to the workspace blurb); `toggle` has
    no `design::toggle(` call site — every landed
    toggle uses `toggle_row` (settings ×4) or a
    btn-form toggle, the pill visual living through
    `toggle_row`'s shared construction; `PANEL_PAD`
    is referenced only inside design.rs, by
    `panel_col` itself. All three stay in the kit
    as chartered API; they are named here so the
    docs do not claim adoption the tree doesn't show.
- **Copy pass** (help_panel.rs + the one exception
  site in command_palette.rs): the stale Delete row
  now reads "Delete the selected stroke, particle
  event or segment" (the Phase J cascade, P14 H2);
  Ctrl+S moved from Features prose to a Shortcuts
  true-key row (P14 H1) — census unchanged at 54
  (Shortcuts 9 → 10, Features 30 → 29) — which
  retired F6's drift-guard exception: all 17
  palette hints are now first-column help labels.
- **Layers spillover** (layers_panel.rs): Bottom/Top
  jumps converted to `icon_btn` with the foundation's
  `angles-down` / `angles-up` glyphs — closing the
  Phase H paragraph's "glyphs not curated" premise;
  that file's two stale doc comments were corrected
  in the same pass (module doc + `layer_row` doc).
  The same pass converted the panel's hand-drawn
  opacity slider to `design::slider_track`: the
  V-wave audit had found its rail/knob colours
  drifted from the kit (`control_off` rail, `text`
  knob — the scene panel's Amount slider, its
  shape-twin, already used the kit), so the two
  unipolar sliders now share one construction.
- **Read-only audits:** stage_view (§5);
  waveform_view + fig3_preview (3/3 verified, no
  discrepancies — the waveform's only element
  builder is its canvas plus the clip caption;
  fig3_preview is pure geometry with the Phase L
  viewport clip and ground offset intact); the
  design kit itself (§3).

## 7. Inventory snapshot

Gate run on the V-wave tree (this census; base
`a570d60`): **PASS — all six checks green**.
Inventory **804 unique element keys** (699 static
+ 105 dynamic), **zero lost** (§5's artefact
account stands). State census identical to Phase M
(**43 fields, 20 literals** — the phase added no
State fields, by charter rule). Source-pinned
counts: palette **108** commands, MCP registry
**91** (total **102** tools — unchanged; different
surfaces, as Phase M noted), curated icons
**163**, headless scenes **15**, help **54** rows,
design kit consumers **36 files**, panel_col
adoption **30 of 31** (one named exception).

CI verdicts: foundation **GREEN — run 42482498164
on `5dfed00`, 2,475 tests**; adoption **GREEN —
run 42484494999 on `a570d60`, 2,478 tests (996 lib
+ 1,472 bin + 10 integration), 0 failed**. Final
integration CI: **GREEN — run 42488645714 on
`ebdda27`, 2,492 tests (996 lib + 1,486 bin + 10
integration), 0 failed**. The loop from the V tree
(`a9f4013`) took five fix commits: a sweep-test
compile deref, three rounds on the timeline test's
geometry relations (edge abutment and displacement
are unsound for a mid-session-mounted canvas —
`Runtime::presented` returns the drawn box, which
rests up to its own height above the layout slot;
the test now asserts presence, centre order and the
shared left edge, and the layout truth is proven by
the headless tree dump: timeline 732..786,
timeline-clips 788..798, loop row 800..825, exact
2 pt gaps), the figures-panel overflow fix (§8),
and two headless probe-isolation fixes (§8).
Headless re-dispatch: **run 42488647081 — all 23
probes exit 0 (13 trees + 10 snaps), every snap
pixel-read by the integrator against P17's
checklist**: cards read as cards with hairlines,
columns 300/744/340 hold, the fig3 grid stays
inside the rounded stage on the three-d-stage
snap, palette kbd chips legible on the accent
0.85 selected row, the File dropdown and the
settings card unclipped. Default tree: **1,018
elements** (Phase M 942, **+8.1%** — inside P19's
+8–11% budget); stage rect **[328, 157, 744,
485]** (Phase M height 522; −13 hairline/root
spacing, −18 timeline card, −6 root spacing).

## 8. Honest remainders after Phase N

- **White on the kit accent measures 4.19:1**
  (P12) — it fails AA normal text (4.5) by 0.31
  and passes AA large / non-text. Documented
  acceptance, with the usage rule the charter
  recorded: accent fills carry short labels and
  icons only, never body text. The token was not
  darkened to chase the number.
- **`tab_visits_controls` stays OFF** — a deferred
  product decision (P14): enabling it would let a
  focused button eat Space (the Play conflict) and
  sliders eat Left/Right frame-stepping. The field
  is set nowhere in main.rs, so Rune's default
  stands; the decision is the user's to revisit.
- **The legacy accent split is documented, not
  unified.** Stage/timeline drawing still uses the
  Wave-14 2.0 constants (`SELECTION_ACCENT
  0x6D8CFF`, shared with stage_fx's `GIZMO_COLOR`,
  and `0xA184FF`) alongside the token accent —
  P12 records them as a deliberate split, and
  unification is a product decision, not a restyle.
  Phase N changed neither constant.
- **Sound is still the user's to confirm.** The
  phase touched toast/status surfaces fed by
  status strings, never the audio path; every sync
  correction remains a bounded restart, not a
  sample-accurate clock (Phase M remainder, open).
- **Two "New project" results still disagree**
  (panel blank vs Home demo) — the user's product
  call, untouched again.
- **Range MP4/WebM exports still carry no audio**
  — Phase M made the panel say so; Phase N did not
  change any export path.
- **Brush scatter for non-spray strokes is still
  model data only** (Wave 12 remainder): Phase M
  wired opacity end-to-end, Phase N touched
  neither.
- **Some empty lines stay bare, by ruling.** No
  plan assigns these panels an empty-state glyph,
  and the wave's rule was never to invent one:
  project_tools' empty project-list line,
  text_tool's two empty messages, stroke_inspector's
  "No strokes on this frame.", three_d_panel's
  sub-section lines ("Enter 3D to list the figure's
  joints.", "No saved 3D figures.", "No saved 3D
  poses.") — and, under the same ruling,
  camera_panel's "No camera keys yet." and the
  segment/figure inspectors' no-selection lines.
  They are follow-up candidates with a glyph
  decision attached, not oversights.
- **Element growth, measured at close-out:**
  1,018 elements on the final default tree vs
  Phase M's 942 — **+8.1%**, inside P19's +8–11%
  budget (adoption restyled roots instead of
  nesting wrappers, as budgeted).
- **The look is pixel-proven** (integrator read of
  the §7 snaps, run 42488647081): cards, hairlines,
  radii, kit steppers/swatches/toggles and the
  danger Delete all read as designed. The pixel
  pass also caught three things the gate cannot
  see. (1) **Figures-panel overflow — FIXED**
  (fix 5, `b637864`): the header row measured
  ~127 pt wider than the card (fig-duplicate
  entirely off-card at x 320..412; fig-eye past
  the row edge at 299.5..325.5 — the white
  half-disc in the snaps); Recolour + Duplicate
  now sit on their own actions row and the eye
  rides row line 2; the final tree reads
  fig-duplicate x 125.6..217.5, fig-eye-0
  238..264. Pre-existing since Phase M, found by
  this phase's pixel pass. (2) **Headless probes
  leaked prefs between processes — FIXED**
  (fixes 4 + 6): layout prefs persist to
  `./projects/.layout.json` (CWD-relative,
  shell.rs), so snap-default inherited the Draw
  workspace from the draw-open probe and rendered
  byte-identical to snap-draw-open in runs
  42487139921 and 42488210880; probe() now wipes
  `./projects` (and takes a fresh HOME) per
  invocation. (3) Two minor pre-existing items
  left as remainders: the settings language rows
  can run past the card's right edge at 1440 pt
  (the last chip in a row clips at the window
  edge), and the fx browser's "No presets saved
  yet" line is bare text rather than the kit
  empty_state — neither was in any plan's scope.
