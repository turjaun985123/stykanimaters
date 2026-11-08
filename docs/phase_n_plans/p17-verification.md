# Phase N — P17: Verification plan (Runtime round 11, headless scenes, gate)

Planning agent P17. This plan tells the V-wave (verification)
agents exactly what to build, in what order, and what each
check can and cannot prove. Everything below was verified
against the Phase M tree (HEAD `baf5441`, 733 keys, 2,451
tests, 11 scenes, 39 snap tests).

## 0. The one fact that shapes this whole plan

A visual phase mostly changes things the Runtime **cannot
see**. Know the boundary before writing a single test:

| Surface | What it can assert | What it cannot |
|---|---|---|
| Runtime (`snap_tests.rs`) | element **keys present/absent**, presented **rect + opacity** (`rt.presented(key)`), findable centre (`rt.find(key)`), state fields after presses | fill, border, radius, colour, font, padding — none of these are in the element tree |
| `--tree` dump (headless) | keys + counts per scene (the `layout_assertions()` facts in scenes.rs) | same as Runtime — no style data |
| `--snap` PNG (headless) | **everything visual** — card fills, hairlines, overlap, clipping — but only by a human/agent **reading the pixels** | nothing automated; the integrator reads each snap |
| Gate (`phase_j_checks.py`) | key sets, State literals, caller census, source counts — including a new **design-adoption census** (§5) | runtime behaviour |

So round 11's job is narrow and real: prove the redesign
**kept every surface mounted, reachable and non-overlapping**
while its structure moved. The *look* is proven by the new
headless snaps (§4), read by the integrator. Any round-11
test that tries to assert a colour or a radius is a fake
test — do not write it.

## 1. Standing disciplines (from rounds 1–10, all still binding)

Copied from `src/rune_sec/snap_tests.rs` as it stands; the
round-11 agent inherits them verbatim:

- **Harness**: `editor()` builds `Runtime<State>` at
  `VIEWPORT (1440, 900)`, `DT = 1/60`; helpers `tap(rt, key)`
  (press-and-release the element's centre + one frame),
  `presented_keys(rt) -> BTreeSet<String>` (from
  `rt.elements()`), `press_key`, `type_char`,
  `scroll_into_view(rt, key)` (wheel the right column at
  (1255, 450) until the key's centre is above y≈850, then
  settle), and the geometry helpers `rects_intersect` /
  `rect_inside` (snap_tests.rs:1777–1790).
- **Shared prefs**: every test in the binary shares the
  on-disk layout prefs. Force the starting workspace by
  tapping its chip (`ws-animate` etc.) and assert
  `rt.state.workspace` before asserting anything else.
  Restore Animate at the end of workspace tests.
- **Presence fades**: an element leaving the tree stays
  "presented" for a few frames. Absence assertions need the
  bounded settle loop (≤30 frames, break when the key is
  gone); presence after a structural mount may need the
  same loop in reverse.
- **Direct writes need `rt.invalidate()`** (Phase M fix 5,
  the clips-lane lesson): the Runtime re-runs the view only
  when state is marked changed. A test that writes
  `rt.state.…` directly must call `rt.invalidate()` before
  running frames, or the view never rebuilds.
- **No local cargo** (repo AGENTS.md): CI is the only
  verdict. Verdicts come from `gh run view`, never from a
  watch pipeline's exit code.

## 2. Round 11 — the Runtime test list

Append to `src/rune_sec/snap_tests.rs` as **round 11**,
with the module-doc round note updated (rounds are
documented at the top of the file; round 10's note is the
model). All tests use only §1's harness. Expected file
total after round 11: **39 + 9 = 48 tests** (tests 1–9
below; 10–11 are conditional — count them in only if their
precondition lands).

Panel root keys cited below were verified in the panel
files on the Phase M tree: `figures-panel`
(figures_panel), `frames` (frame_ops), `stroke-inspector`
(stroke_inspector), `inspector-style` (inspector_style),
`audio-panel` (audio_panel), `fx-browser` (fx_browser),
`fx-clips` (fx_clips), `particles` + `particles-detail`
(particles_panel), `layers-panel` (layers_panel),
`library-panel` (library_panel), `home-panel`
(home_panel), `panel-3d` / `panel-3d-preview`
(three_d_panel), `export-panel` (export_panel),
`project-tools` (project_tools), `settings` /
`settings-card` (settings_popover), `command-palette` /
`palette-query` (command_palette), `menu-bar` /
`menu-scrim` / `menu-file-dropdown` (menus),
`timeline` / `timeline-loop-row` / `timeline-clips`
(timeline_marks), `stage`, `app-bar`, `transport`,
`centre` (main.rs / chrome_view).

### Test 1 — `animate_panel_card_roots_present`
Seed `ws-animate`. Assert the Animate set's panel roots
are all presented after the card restructure:
`figures-panel`, `frames`, plus (after
`scroll_into_view` where the column requires it) the
right column's Animate roots. Purpose: `design::panel_col`
replaces each panel's root `col()` — a builder mistake
loses the root key or the panel. The gate diffs key
*sets* statically; this test proves the roots still
**mount** in the live tree. Confirm the exact Animate
root list against `workspaces.rs`'s ANIMATE array at
write time; do not copy this paragraph's list blind.

### Test 2 — `panel_card_rects_stay_inside_their_columns`
Geometry, using `rt.presented(key) -> ([x,y,w,h],
opacity)` + `rect_inside`: the rect of `figures-panel`
lies inside the left column's x-range, and one
right-column panel's rect (e.g. `frames`) inside the
right column's. Card padding (S3 = 12) and borders must
not push a panel across a column edge or over the stage —
the Phase J overlap class, re-checked after the widest
structural change since. **Do not pin Phase M's numbers**
(300 / 744 / 340 columns, stage [328, 144, 744, 522]):
the timeline becomes a padded card in Phase N (§13 of
the charter), which can move the stage's height. Assert
*containment and non-overlap* (relations), and record
the new measured bounds in the test comment from the
first green run's headless tree (§4) — relations are the
durable assertion, numbers are a snapshot.

### Test 3 — `workspace_card_roots_sweep`
Table-driven sweep over all eight workspace chips
(`ws-animate`, `ws-draw`, `ws-graph`, `ws-effects`,
`ws-audio`, `ws-3d`, `ws-export`, `ws-home`): tap, settle,
assert 2–3 verified root keys present and one gated-out
root absent, per workspace. Anchors already proven in
earlier rounds: Draw → `draw-tools` present / `frames`
absent / `figures-panel` stays (round: workspace_gating
test); Export → `export-panel`; Build3D → `panel-3d` +
`panel-3d-preview`; Effects → `particles`. Fill the
remaining rows (Graph, Audio, Home) from `workspaces.rs`
+ the root keys above at write time. Restore Animate at
the end. This is the single highest-value round-11 test:
the adoption wave touches every panel's root, and this
sweep walks every workspace's roots in one test.

### Test 4 — `edit_menu_dropdown_renders_after_restyle`
Round 3's `file_menu_opens_lists_items_and_scrim_closes`
already proves the File menu. Round 11 adds the **Edit**
menu, whose dropdown carries a kit `divider` after the
restyle: tap `menu-edit`, settle one frame, assert
`rt.state.menus.open == Some(menus::EDIT)`,
`menu-edit-dropdown` presented, and `menu-edit-sep`
(the separator key, menus.rs) presented — the separator
is the element the charter's divider replaces, so its
survival is the restyle's proof. Close via `menu-scrim`
+ settle loop; assert the dropdown key is gone.

### Test 5 — `palette_open_card_renders_and_closes`
The palette becomes a kit card (charter §9). Open it the
real way — Ctrl+K through `press_key` (round 3's palette
test is the pattern) — settle, assert `command-palette`
and `palette-query` presented and
`rt.state.palette.open`. Close with Escape (the
`palette_handle_key` path) and, after the settle loop,
assert `command-palette` has left the presented set.
What this proves: the restyled overlay still mounts as a
centre-column child gated on `s.palette.open`
(main.rs:677) — the one structural fact the pixels can't
attribute. (Pixel check of the card itself: §4 snap.)

### Test 6 — `settings_card_renders_after_restyle`
Tap `settings-toggle` (after `scroll_into_view` — the
settings popover sits at the top of the right column,
main.rs:599, ungated), assert `settings-card`,
`settings-theme` and `settings-speed-row` presented and
`rt.state.settings.open`. Tap the toggle again; settle;
assert `settings-card` is gone but `settings` (the
always-mounted shell) remains. The card is the charter's
§9 popover surface in miniature: fill + border + radius
are snap-proven; mount/close is Runtime-proven here.

### Test 7 — `timeline_card_keeps_chips_marks_and_rows`
Charter §13 wraps the whole timeline stack in one card —
the riskiest restructure for key survival. In Animate:
assert `timeline`, `frame-1`…`frame-5` (exactly the demo
count), `timeline-markers`… (key verified in
timeline_marks) presented; tap `loop-set` (timeline
loop row's own control) and assert `timeline-loop-row`;
then plant one FX clip exactly as round 10's clips-lane
test does (direct `fx_clips.push` + `rt.invalidate()` +
settle loop) and assert `timeline-clips`. One test,
three sub-surfaces, because all three move together
into the card.

### Test 8 — `stage_and_shell_furniture_survive`
The app-bar / stage / status restyles (charter §12,
§14, §15) keep their keys by rule; prove the mounted
set: `app-bar`, `transport`, `stage`, `centre`,
`stage-overlay-row` (the camera overlay becomes a
floating pill — same row key, new styling), `menu-bar`,
`ws-animate`. Add the stage non-overlap relation from
test 2 if not already covered there: stage rect must
not intersect either side column's panel rects
(`rects_intersect`).

### Test 9 — `fx_browser_and_draw_roots_present`
Effects workspace: `fx-browser` + `fx-clips` presented
(the browser's category grid is the charter's list-row
showcase). Draw workspace: `stroke-inspector` presented
+ `figures-panel` stays. Both after chip taps + settle.
(Overlaps test 3's sweep deliberately: these two panels
carry the heaviest list/stepper adoption, so their
roots get a named test, not only a table row.)

### Conditional tests (only if the precondition lands)

10. **`empty_state_key_present`** — ONLY if an adoption
    brief gives a kit `empty_state` surface a key. The
    charter's kit signature carries no key parameter, so
    by default empty states are **snap-proven only**;
    do not invent a key to make this test writable
    (charter: new keys only where a brief allows).
11. **`help_panel_renders`** — `menus.help_open` is set
    by Help ▸ Shortcuts (`MenusState.help_open`,
    menus.rs:125). If the foundation wave's menus work
    keeps the help panel's current root key, add: tap
    `menu-help`, tap the Shortcuts item, assert the
    help root presented + `rt.state.menus.help_open`.
    Verify the help root key in help_panel.rs at write
    time — it was not censused for this plan.

### What round 11 must NOT do
- No colour/fill/radius/font assertions (impossible —
  see §0). No screenshot comparisons inside cargo tests.
- No new State fields, no new keys, no handler changes:
  round 11 is tests-only, in the one file, exactly like
  rounds 1–10.
- No re-pinning of layout numbers from Phase M (see
  test 2). Relations first; measured numbers only as
  comments.

## 3. New headless scenes (scenes.rs)

Four scenes, all **settable from State fields / pub ops**
— verified one by one below, because a scene fn is just
`fn(&mut State)` applied to `init()` state: no keyboard,
no presses, no clock. Each scene below names the exact
real op it calls, following the file's standing
discipline (scenes.rs module docs: "no scene invents
state the editor cannot reach").

| Scene | Setter (verified) | Why it is legal |
|---|---|---|
| `palette-open` | `command_palette::open_palette(s)` — pub fn, command_palette.rs:1401; the palette's own open path (called by `palette_handle_key`, :1453) | Sets `s.palette.open = true` (PaletteState, main.rs:72), clears query, selects 0, focuses the field. `main.rs:677` mounts `palette_view` in the centre column iff `s.palette.open`. |
| `menu-open` | `s.menus.toggle(menus::FILE)` — `MenusState::toggle` pub (menus.rs:131), `FILE` pub const (menus.rs:102), field `s.menus` (main.rs:71) | Exactly the File trigger's own handler (`menu_bar` wires `s.menus.toggle(index)`). Dropdown key `menu-file-dropdown` + `menu-scrim` mount as root children. |
| `settings-open` | `settings_popover::toggle_open(s)` — pub fn, settings_popover.rs:546 | The `settings-toggle` button's own handler calls it. Flips `s.settings.open` (main.rs:73); the popover is mounted ungated at the top of the right column (main.rs:599), so the default Animate landing shows `settings-card`. |
| `draw-open` | `workspaces::select_workspace(s, Workspace::Draw)` — the chips' / palette `go-*` op used by `export-open` & co. | No Draw scene exists yet (the 11 are: default, frame-3, selected, playing, export-open, fx-selected, graph-duplicates, fx-split-mid, three-d, loop-range, three-d-stage). Draw is the adoption wave's densest workspace (draw tools, stroke inspector, text tool) and has no pixel proof today. |

Rejected candidates, with reasons:
- *fx-browser scene*: no open-state exists to set —
  `FxBrowserState` is only `{ query: TextEdit, recent:
  Vec<ClipKind> }` (fx_browser.rs:311). The browser is a
  panel gated to Effects, already reached by
  `fx-selected` / `fx-split-mid`. A dedicated scene
  would duplicate those trees. The browser's restyle is
  pixel-proven by adding a snap of the EXISTING
  `fx-selected` scene (§4), not a new scene.
- *help-open scene*: `menus.help_open` is settable
  (pub bool), but the Help menu must be open first for
  the state to be reachable in the real shell; a scene
  setting both fields is legal (`s.menus.open =
  Some(HELP); s.menus.help_open = true`) yet proves
  little the menu-open snap doesn't. Skip; revisit only
  if the help restyle needs its own snap.

### Registration + facts (the V-wave scenes agent's list)
1. Append the four fns to scenes.rs in the file's
   doc-comment style (each doc names the op it calls
   and the proof it feeds), and append to `scenes()` —
   **order is contract**: the table test pins positions
   0–10 today; append at 11–14 (`palette-open`,
   `menu-open`, `settings-open`, `draw-open`) and update
   `scene_table_has_eleven_uniquely_named_scenes` →
   fifteen, with the new tail positions pinned.
2. One unit test per scene in scenes.rs (the file's own
   pattern): palette-open → `s.palette.open` true and
   query empty; menu-open → `s.menus.open ==
   Some(FILE)`; settings-open → `s.settings.open` true;
   draw-open → `s.workspace == Workspace::Draw`.
3. Layout facts 15–18 for `layout_assertions()`:
   - **15** (`palette-open`): `command-palette` exists
     and `palette-query` exists (the overlay mounts iff
     the open flag is set — a tree without them means
     the gate or the view branch regressed).
   - **16** (`menu-open`): `menu-file-dropdown` exists,
     `menu-scrim` exists, and `export` (the
     dropdown-only File item) exists.
   - **17** (`settings-open`): `settings-card` exists
     (the `settings` shell exists under every scene;
     the card is the open state's proof).
   - **18** (`draw-open`): `stroke-inspector` exists
     and `frames` does NOT (Draw gates FrameOps out —
     the inverse of fact 8's shape).
4. Gate count claims (§5) read scenes live, so the
   printed count becomes **15** automatically; QA §16
   and PHASE_N.md must print 15.

## 4. Headless workflow + the integrator's snap list

`.github/workflows/headless.yml` is dispatch-only and
fail-soft; every probe records a `.exit` file and the
run's conclusion proves nothing by itself — the
integrator reads the artifact (the Phase I–M
discipline). Changes for the V-wave workflow agent:

**Tree loops** (both the plain step and the xvfb
fallback list — they must stay identical): append
`palette-open menu-open settings-open draw-open` to the
existing nine (`default frame-3 selected playing
graph-duplicates fx-split-mid three-d loop-range
three-d-stage`) → **13 tree dumps**.

**Snap step**: keep the existing four (default, stage
crop, three-d, three-d-stage) and add four, same
`--at 0` discipline:
- `snap-palette-open.png` (`--scene palette-open`)
- `snap-menu-open.png` (`--scene menu-open`)
- `snap-settings-open.png` (`--scene settings-open`)
- `snap-draw-open.png` (`--scene draw-open`)
- plus `snap-fx-selected.png` (`--scene fx-selected`) —
  the FX browser's restyle proof without a new scene
  (§3). Total snaps: **9**.

**The integrator's pixel checklist** (read every snap;
this is the phase's actual visual verdict):
- *default*: every panel reads as a card — fill distinct
  from the window, 1px border, rounded corners; headers
  aligned across panels; no text clipped by card padding;
  the app bar's hairline under bar + workspace row;
  status strip segments separated by ticks.
- *stage crop*: stage radius reads clean; the camera
  overlay is a floating pill, not a bare row.
- *palette-open*: the palette is a centred card, radius
  12, item rows even, kbd chips in the footer/hints;
  nothing of the centre column bleeds through its edge.
- *menu-open*: the File dropdown is a card under its
  trigger — not clipped by the window edge, items 28px
  even, the scrim state invisible but the dropdown
  shadowed/bordered as a surface.
- *settings-open*: the settings card inside the right
  column — toggle pills read as pills (track + knob),
  speed chips even, no row overflows the card's padding.
- *draw-open*: Draw panels carded like the rest; brush
  list rows selected-state reads (accent 16% fill);
  steppers' mono values centred.
- *fx-selected*: FX browser category rows + recent row
  consistent with the kit; the Particles detail card
  matches the other panels' header shape.
- *three-d / three-d-stage*: regression only — the Phase
  L/M proofs (grid contained in the panel; 3D figure on
  the main stage, clipped to the stage rect) must still
  hold after the stage/timeline restyles.
- Any snap where a card's content touches the window
  edge, a dropdown is clipped, or two surfaces overlap
  is a **fail** the integrator sends back — the Phase J
  overlap precedent (fixed in code, re-proven in pixels).

Also record in the run's reading: the new stage/timeline
geometry (stage rect from `tree-default.json`) — it
feeds round 11 test 2's comment and QA §16. If the
timeline card changed the stage height, that is
expected, not a regression; a changed stage **width**
or column width IS one (charter changes padding inside
surfaces, not the 300 / 744 / 340 column contract).

## 5. Gate additions — design adoption census (check 6)

`scripts/phase_j_checks.py` gains a **Check 6 — DESIGN
ADOPTION**, in the script's established idioms
(`blank_code` blanking, brace-block capture, the
test-region cut at the trailing `mod tests`, counts
printed for docs to match — no hard-coded expectations
except the kit API list itself, which the charter
fixes). The V-wave gate agent implements:

1. **Kit surface check.** Parse `src/rune_sec/design.rs`
   for its `pub fn` / `pub const` names. The charter's
   fixed API must all be present: consts `PANEL_RADIUS`,
   `PANEL_PAD`, `PANEL_GAP`, `ROW_RADIUS`,
   `CONTROL_RADIUS`; fns `accent`, `selected_fill`,
   `panel_col`, `header_row`, `panel_title`,
   `section_label`, `divider`, `kbd`, `empty_state`,
   `list_row`, `toggle`, `toggle_row`, `stepper_value`,
   `slider_track`. A missing name = FAIL (the foundation
   deviated from the charter without the plan knowing).
   Extra names = report only.
2. **Consumer census.** For each kit fn, count
   production call sites outside design.rs (check 4's
   machinery, same-file rule OFF — a kit fn used only
   inside design.rs is an unadopted helper by
   definition, except the documented internal ones:
   `accent` / `selected_fill` may be consumed by other
   kit fns — census them across the whole tree
   including design.rs and mark the difference in the
   printout). Print per-fn consumer-file lists, ADOPTED
   (≥1 external consumer file) vs UNADOPTED. UNADOPTED
   is a **report, not a failure**, mid-phase — the
   adoption wave lands file by file (the Phase K
   UNWIRED precedent).
3. **Panel adoption roll.** The headline number:
   distinct panel files calling `design::panel_col`,
   printed as `panel_col adoption: N of M panel files`,
   with the non-adopting files listed by name. M = the
   panel builder files under `src/rune_sec/` that
   main.rs mounts (the §2 root-key files + chrome_view /
   stage surfaces the F-wave owns, listed in the script
   as a named set the integrator maintains). At
   close-out this becomes a hard FAIL when N < M —
   flip the flag in the same commit that lands the
   last adoption section, not before.
4. **`fbtn` retirement census.** Count `fn fbtn`
   definitions across `src/` (blanked source). Charter
   §4 retires local copies onto the one button family:
   print the count + defining files. Baseline on the
   Phase M tree: frame_ops carries one; census the
   others at F-wave start and print the baseline in
   QA §16. Target at close-out: the only remaining
   button builders are `chrome_view::btn` and the
   icon_btn family — FAIL at close-out if a `fn fbtn`
   survives outside a file the integrator has explicitly
   exempted in the script (with the reason in a comment,
   the SAME_FILE_OPS precedent).
5. **Private section-helper census.** Count
   `fn section(` / `fn section_label(` definitions
   outside design.rs. The charter's audit found ≥5
   (command_palette, easing_panel, export_panel,
   fx_browser, figure_add). Print count + files; target
   0 outside design.rs at close-out (same close-out
   flag discipline as item 3).
6. **Count-claims extension.** Check 5 already reads
   scenes live (11 → 15 needs no script change). Add
   one printed line: `design kit consumers: K files`
   (distinct files importing `crate::rune_sec::design`
   or naming `design::`), for PHASE_N.md / QA §16 to
   quote next to the adoption roll.

Non-goals for check 6: no colour/literal policing
(`.radius(12.0)` hand literals are legal where the kit
doesn't reach — the palette already had radius 12
before the kit existed), no style assertions of any
kind. The census proves **use**, not taste; taste is
§4's snaps.

## 6. Execution order for the V wave

1. **Scenes agent** (§3) — independent of the design
   kit; can land first. Scenes + facts + unit tests in
   scenes.rs only.
2. **Workflow agent** (§4) — headless.yml lists only;
   lands with or after the scenes (it names them).
3. **Gate agent** (§5) — check 6, report-mode flags as
   specified; lands any time after design.rs exists
   (needs the real kit file to parse).
4. **Round-11 agent** (§2) — lands LAST among the four:
   its tests assert the adopted tree, so it is written
   against the post-adoption panels (root keys are
   charter-frozen, so an early draft is possible, but
   the geometry relations in test 2 want the final
   layout).
5. **Integrator**: gate run (checks 1–6) → push → CI
   green → dispatch headless → read all 9 snaps against
   the §4 checklist + all 13 `.exit` files → QA §16 +
   PHASE_N.md quote gate counts (733 keys or the new
   total with every addition named, scenes 15, adoption
   roll N of M).

## 7. Honest limits of this plan

- Round 11 proves structure survived, not that the app
  looks better. If the snaps show an ugly-but-intact
  shell, CI is green and the phase still failed its
  point — the §4 pixel read is the verdict, and this
  plan deliberately gives it teeth (named fail
  conditions, Phase J precedent).
- Geometry: Phase N *will* move some rects (timeline
  card padding, panel card padding). Round 11 asserts
  relations, so legitimate moves pass; but the
  integrator must still eyeball the new stage size in
  the tree dump — a stage that quietly lost 100px to
  padding passes every relation and is still wrong.
- The adoption census counts call sites, not correct
  use: a panel can call `panel_col` and still hand-roll
  its header. Census + snaps together cover it; neither
  does alone.
