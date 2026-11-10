# Phase O — Feel & Finish: close-out

> Phase O made the Phase N design kit *feel* professional:
> interaction states, motion consistency, feedback
> completeness, empty states everywhere, keyboard
> discipline, and the overflow repairs Phase N's pixel
> pass recorded. Charter: `docs/PHASE_O_CHARTER.md`.
> Plan of record for the overflow work:
> `docs/phase_o_plans/p05-overflow-widths.md`. The
> verification record for this phase is QA_RUNE §17.
>
> **Status: CLOSED** (2026-11-10), on the tree at
> `911cf77`. "Shipped" below means only what is in that
> tree, verified anchor by anchor at close-out; the CI
> position is stated once, in §10, and nowhere
> overstated.

## 1. What Phase O set out to do

The charter (written 2026-11-09, seeded from Phase N
§8's recorded remainders) is a feel-and-finish phase:
no new engine features, no new panels, no format work,
no re-styling. Its ten binding rules, in brief: kit
first (1); zero key loss against the 804-key Phase N
baseline (2); every interactive element carries rest /
hover / press / selected / muted states from the
existing token set, with inexpressible states named as
engine limits, never faked (3); motion only on the
`pair::SNAP` / `pair::SETTLE` spring pairs (4); width
budgets stated — left column content 276 pt, right
column content 316 pt — and nothing crossing a card
edge (5); no silent mutations (6); every bare empty
line gets the kit `empty_state` treatment (7); Esc
closes the topmost overlay and every shortcut shown in
the UI is real (8); copy is en-GB, numbers in mono
(9); every item names its proof (10).

Seed work: the settings language rows overflowing
their card plus a general card-overflow audit method;
the fx browser's bare "No presets saved yet" line; the
bare empty lines in project_tools, text_tool,
stroke_inspector, three_d_panel, camera_panel and the
two inspectors; and an adopt-or-retire decision on the
zero-consumer kit items `text_muted` and `toggle`.

Explicit non-goals, recorded here because they bound
every section below: `tab_visits_controls` stays OFF;
the two-"New project" behaviour is unchanged; the
accent is not darkened; no audio-path work.

## 2. How it actually executed — the honest history

Phase O did not run in the charter's wave shape
(planning → foundation → adoption → verification).
The original Phase O planning wave was stopped by the
user after exactly one plan landed (`p05`, the
overflow plan). Before the charter's own waves could
run, the UI/UX transformation mission (2026-11-08/09)
implemented the great majority of the charter's items
— its Wave 1 repaired the p05 overflow rows, Wave 2
the kit states and the zero-consumer decisions, Wave 4
the empty states — without calling them Phase O.

Phase O was therefore closed by audit, not by
construction. Wave P's Squad 1 (agents P1–P12,
2026-11-10, findings in `~/workspace/wave-p/findings/`)
took the charter item by item against the tree at
`0b75051`: most items verified already-landed, site by
site; the genuine remainders were fixed in that wave
(P4, P6, P9) or identified as stopped mid-work. The
user halted Wave P at 09:49 +06; the two Squad 1
slices that never filed (P5, P11) were finished and
verified by Wave Q (Q8, Q9), and P43's scan-set work
by Q12. Everything landed in the Wave P+Q integration
commits (`85bf9f9` core, `639ea47` MCP, `3d0d98c`
shell, `ce2b655` verification) plus the CI fix round
`911cf77`. REDESIGN_PROGRESS §6.11 is the wave-level
record; this document is the charter-level one.

Phase-O-attributable assignments, counted honestly:
12 Squad 1 agents (ten filed findings; P5 and P11
were halted without filing), 2 Wave Q finishers for
those slices (Q8, Q9), Q12 for the scan set, and this
close-out (Wave R, R11) — plus the 12 stopped
planning-wave agents of whom one plan (p05) survives.
The count is not padded to any ordered number.

## 3. Seed work — the overflow repairs (p05 items O-1…O-9)

The p05 plan measured 11+ overflowing rows from
headless run 42488647081 (head `ebdda27`) — a tree
that predates its own remedies. P1/P2/P3 verified
every named row in the tree; the position at close-out:

| Item | Defect (plan measurement) | Tree state at `911cf77` |
|---|---|---|
| O-1 settings language grid | 4 chips past the 316 pt content edge, 2 past the window (`settings-lang-sl` +51.0) | Rows carry the plan's `.flex_wrap()` **and** its named contingency `.w_full()` — the load-bearing half, since wrap cannot engage against an unbounded row width (`settings_popover.rs`, wrap pair on the language rows and, defensively, the workspace and speed grids). Keys verbatim (`settings-lang-grid`, `settings-lang-row-{i}`, `settings-lang-{code}`). In-file pins (`language_rows_overflow_unwrapped_and_wrap_inside_the_card`, :1820) prove all seven rows' one-line widths (340–443 pt) pack inside 316 and inside the 236 pt narrowest column; the widest chip (`Español (Latinoamérica)`, 181 pt modelled) never needs truncating. Q9 swept the whole settings surface: no other row crosses 316 pt |
| O-2 audit method | — (tooling) | `scripts/overflow_scan.py` implements p05 §2 rule for rule (content / card / window rules, the closed exemption list) and exceeds it where later defect families required (rule 4, the timeline card's vertical budget). Self-test green at close-out. Wave Q (Q12) made the proof set machine-readable: `SCAN_SET` names 13 required trees and fails a debrief that is missing one (§10) |
| O-3 layers row | ≥ +134 pt, reorder cluster past the card | The plan's exact two-line anatomy in `layer_row` (`layers_panel.rs:593`): dot/name/opacity on line 1, the reorder cluster (`layer-bottom/down/up/top-{fi}`, keys verbatim) on line 2 at ≈253 of 260 pt |
| O-4 draw cluster | `draw-brushes` 879 pt (+563; six of ten brushes unclickable), tools row +374.5, shapes +132.5, stamp params +42 | `.flex_wrap()` on brushes (:843), tools (:785) and shapes (:864) in `draw_tools.rs`; stamp params took the stronger form — an explicit three-row split (`draw-stamp-params` / `-2` / `-3`, each stepper group whole, each row inside even the 276 pt budget) after Wave 9 re-measurement showed no pairing fits |
| O-5 graph + keyframe rows | `graph-actions` +305.5 (`graph-clear` ended at x 1717.5), four more rows +2.8…+45 | Graph rows wrap (`graph_panel.rs:1961`, :2007). Keyframe presets wrapped at :1308 — **but `keyframe-actions` and `keyframe-clipboard` were the one genuine remainder in O-1…O-9**: the UI mission's Wave 1 commit message claimed all of O-1..O-9 and had not landed these two. P2's interrupted attempt added both `.flex_wrap()` calls (+11 lines, :1356, :1380, keys verbatim); verified line by line and kept |
| O-6 weather segments | `wx-kind` 434.3 pt (+118.3), `wx-seg-5` past the window | `.flex_wrap()` on the segment row (`scene_panel.rs:1071`), `wx-seg-{i}` keys verbatim |
| O-7 3D rows | posing strip +49, camera strip +37.5, bake row +9.5 | All three rows wrap (`three_d_panel.rs:917`, :1036, :1175), in-code comments citing the plan's own measurements |
| O-8 fx + playback rows | `fx-nudge` 343.5 pt (+27.5, `fx-dur-up` at the window edge); playback rows +17.6 / +36 | `fx-nudge` split into two lines under its one key — the plan's preferred anatomy (`fx_clips.rs` `nudge_row`, :1487); transport and loop rows wrap (`playback_controls.rs:673`, :715) |
| O-9 pose + camera rows | `pose-apply-all` ended x 1428.5; `cam-delete-key` ended x 1441.4 — past the window | The pose row no longer exists as packed: ops regrouped, Apply-to-all rides a `.flex_wrap()` icon row (`pose_tools.rs:1379`); the camera key row wraps (`camera_panel.rs:1232`) with the plan's measurement in its comment |

What remains for O-1…O-9 is proof at the artifact
level, not code: the §2 scan over a headless
dispatch's real tree dumps (§10). The UI-mission-era
dispatches scanned CLEAN over trees containing these
repairs minus the two keyframe rows; the completed
scan set has not yet had a dispatch to read.

## 4. Seed work — empty states (charter rule 7)

Every bare line named in Phase N §8 and the charter
seed renders the kit component
`design::empty_state(icon, message, t)`
(`design.rs:308`) — centred, width-wrapped (the
budget guarantee), glyph painted only when it
resolves in the curated 163-icon table, adopted copy
byte-identical to the bare line unless the message
itself names the filling action. P7 (panels A–M) and
P8 (N–Z + Home) audited every surface call-site by
call-site; all were landed by UI mission Wave 4:

- **camera_panel** — `empty_state("key", "No camera
  keys yet — Set key pins the camera in effect at this
  frame.")`; the filling controls (Paste key included)
  render above it, live, in the empty branch.
- **fx_browser** — `empty_state("floppy-disk", "No
  presets saved yet")` (the charter's named line) plus
  the filter state `"No effects match the filter"`
  (`magnifying-glass`).
- **Figure inspector** — `"No figures in the project."`
  (`shapes`); **segment inspector** — `"Select a
  segment on the stage to edit its geometry."`
  (`bezier-curve`). Neither panel owns its filling
  action, so neither carries a fake button — the
  charter's honesty rule applied, not an omission.
- **project_tools** — `"No saved projects in
  ./projects/ yet."` (`folder-open`), the core's own
  wording kept.
- **stroke_inspector** — `"No strokes on this
  frame."` (`paintbrush`), byte-identical.
- **text_tool** — both messages (`font`): `"No
  free-standing texts."`, `"No text segments on this
  figure."`
- **three_d_panel** — all three sub-section lines
  (`cube`), byte-identical, including the joints
  section's two-branch message (no session / node-less
  figure).
- **Home** (board addition beyond the charter list) —
  library and search states (`folder-open` /
  `magnifying-glass`), the library message naming its
  action ("use \"Save project\" to add one").

One imprecision recorded, not silently rewritten:
the figure inspector's message also renders for a
stale selection index in a non-empty project
(`figure_state` returns `None` for both). The copy is
a settled adoption and charter rule 2 keeps labels
verbatim unless a plan lists a change; none did.

## 5. Seed work — zero-consumer kit items (adopt or retire)

Decided on evidence by UI mission Wave 2; P12
verified both decisions stand in the tree and the
charter (written earlier) was the stale document:

- **`toggle` — RETIRED.** The kit's own docs record
  it (`design.rs:67`): the bare builder had zero
  production callers; every shipped toggle is a
  labelled `toggle_row` over the same `toggle_pill`
  visual. No `pub fn toggle` builder exists in the
  kit.
- **`text_muted` — KEPT**, as the kit's muted-text
  role (`design.rs:180`), with live consumers (the
  `v_splitter` grip glyph; `menus.rs` `dimmed_item`,
  :556) and a pinning test
  (`text_muted_is_secondary_at_sixty_percent`,
  `design.rs:967`). This supersedes QA_RUNE §16's
  "open — noted" for these two items: the decision
  the charter asked for exists and is documented in
  the kit itself.

## 6. Rule 3 — interaction states

**Buttons + rows (P4).** A per-builder census of the
kit families and every hand-rolled pressable found
the kit complete and exactly four call-site gaps,
all fixed in overlay setters only (+107 lines, zero
keys, no size or gap change anywhere):

- command-palette result rows had no press state →
  `.press(if on { accent } else { t.control_off })`
  (`command_palette.rs:2032`), the `btn` family's own
  press derivation.
- fx-lane rows had no press state → `.press(t.press)`
  (`fx_clips.rs:1378`), the kit row's token; the row's
  measured two-line layout was deliberately not
  migrated.
- scene background swatches had neither hover nor
  press → the kit swatch's overlay pair
  (`scene_panel.rs:1110`); a call-site fix, not a kit
  migration (this swatch is a ring slot, a different
  construction).
- the legacy (unmounted, census-baseline)
  `chrome_view::timeline_row` chips gained the same
  press derivation, keeping the retained surface
  honest at zero risk to live UI.

**Inputs + selects + steppers (P5, finished by
Q8).** The doctrine P5 built into the kit: a control
takes its inert form only where its press can only
no-op *and* is silent or wasteful; where the op
speaks a refusal at the bound, the press is feedback
and stays live. Landed: `StepForm` + `step_btn`, the
focus-state kit (`field_border` — accent only while
focused; `field_caret` — accent caret and selection,
fixing Rune's white-on-light invisibility),
`text_field`, `slider_track` (all in `design.rs`;
`step_btn` at :504). Q8 completed the adoption: the
14th and last bare text field (`proj-name`) now
builds through `design::text_field` — no bare
`text_input` remains outside the kit — and the
remaining stepper pairs took their computed forms
(layers and figures opacity at 0/100, particles
frame at 0, fx start/duration against the handlers'
own silent guards, segment length/thickness at the
ops' clamps, both camera zoom pairs at theirs —
including the panel pair, whose op snapshots *before*
clamping, so a limit press used to spend an undo
step on an identical key). Selects needed nothing:
cycle selects wrap (they can never no-op), and the
segmented/picker selects already carry the kit
active state. Two judged remainders, left live
deliberately under the doctrine: the stroke
inspector's `param_row` (its op speaks a specific
refusal at the ladder ends) and frame_ops' insert-N
stepper (MutedLive by design — a bound press heals a
stale stored count).

**Every other kit class (P12).** Audited builder by
builder: `tangent_preset_chip` (both branches carry
fill/hover/press), `swatch`, menu items and the
muted-live `dimmed_item`, `slider_track` (a
documented visual, not a control — its states belong
to its drag roots, P5's slice), workspace tabs and
chrome buttons (the buttons class). No class is left
stateless; no edit was owed.

**Focus — the named engine limit.** Rune's `El`
exposes no focus state for buttons or rows, and
keyboard focus exists only for text fields.
`tab_visits_controls` stays OFF (charter non-goal,
§11), so no button or row can be keyboard-focused
and no focus visual is expressible. Recorded as a
limit, not faked with a look-alike ring — rule 3's
own terms. The one focus visual the shell owns, the
text field's border, is driven by the same
`TextEdit::focused` flag the shortcut guard reads,
so visual and behaviour cannot disagree (Q8).
Field hover is likewise inexpressible in the pinned
rev without washing fills over glyphs; named in the
kit doc, not imitated.

## 7. Rule 4 — motion

P10's census is complete and its result is zero
conversions: the tree contains exactly **four**
`.transition(` call sites — menus, settings card,
command palette (all `pair::SNAP`), toasts
(`pair::SETTLE`) — and every other mounted element
rides the engine default, which is itself
`pair::SETTLE`. No ad-hoc spring, tween or easing
construction exists anywhere in app code. The 4 pt
rise at the three SNAP sites is Phase N's entrance
pattern, identical at all three.

Two named remainders, both charter-consistent:

- The toast *expiry* fade is linear (1.0 s,
  `FADE_OUT`) — a lifetime fade, not an entrance/exit
  curve; rule 4 preserves Phase N's toast behaviour,
  so converting it would violate the charter.
- The toggle knob teleports between its stops: the
  pinned Rune rev cannot animate a layout offset of
  a state-switched child. The kit doc already gates
  any future slide to SNAP/SETTLE. Not faked.

Timeline scrub carries no motion, correctly: direct
manipulation the charter's no-delay rule protects.

## 8. Rule 6 — feedback completeness

P6's census: 333 mutating core ops enumerated, every
panel-call and gesture cross-checked. The shell
already reported nearly everything; the one silent
class was the stage's own pose and camera drags,
which ended with whatever stale status preceded
them standing. `end_drag` now posts the family's
recap — `"Posed figure {n} at frame {f}."` /
`"Camera key moved at frame {f}."` (`editor.rs:524`,
:499) — measured against a baseline captured at
gesture start with the rotation dial's dead-zone
discipline, so a no-move press stays silent and
never claims an edit. Pinned by
`wave_p_end_drag_recaps_only_real_moves`
(`editor.rs:8103`). The fix adds no element and no
key; feedback rides the one existing channel
(`core.status` → status strip + `classify_status`
toast). Navigation, selection, gesture Move phases,
playback/mode toggles and settings changes are
intentionally silent, with reasons recorded in
P6's findings (the change is visible, or the state
is session/view state, not a project mutation).

P6 also observed that `stage_drag` ignored the
runtime's `cancelled()` flag (the graph gesture
rolls back; stage drags kept the dragged pose).
Gesture semantics rather than feedback — closed in
the same integration by Q2 under the stage-drag
snapshot discipline: a cancelled drag now undoes
its one gesture snapshot, guarded so a
snapshot-less press never pops an unrelated edit.

## 9. Rules 8 + 9 — keyboard discipline, shortcut truth, copy

**Esc closes the topmost overlay (P9 + P12).** P12's
inventory: the command palette claims Esc first
(two arms, test-pinned), menus next — the dispatch
order *is* the charter's priority order. P9 found
and fixed the three real defects, all in
`src/main.rs`, with nine pins in
`key_routing_tests` (`main.rs:1450`):

- **D1 (high).** The Home rename field was missing
  from the `text_focused` guard, so stage shortcuts
  — Space, N, Delete, Ctrl+Z among them — fired
  mid-typing. It now scopes keys as the conjunction
  `s.home_rename.armed.is_some() &&
  s.home_rename.edit.focused` (`main.rs:830`),
  matching its three sibling rename sessions; the
  conjunction also neutralises the stale-focus-flag
  wart P9 observed in Home's cancel path.
- **D2.** Menu Esc sat *below* the text-field bail:
  with any field flagged focused, Esc died on the
  wrong scope and the menu stayed open. The Esc tier
  now sits immediately after the palette claim and
  before the bail, topmost first.
- **D3.** The two armed confirm gates (the shell's
  `confirm_action`; Home's per-row delete confirm)
  had no keyboard exit at all. The Esc tier disarms
  them without running the gated op — disarming is
  silent, matching the poll loop's project-swap
  clear of the same state.

No binding was removed, renamed or re-bound; the
binding census pin presses every documented binding
and asserts its effect (Ctrl+S enumerated but not
pressed — its arm writes `./projects`, which the
suite's no-pollution rule bars; the arm is unchanged
code, and its spelling is drift-guarded, below).

**The settings card** is a toggle-button card in the
column, not a scrim overlay; two independent audits
(P9's census, P12's sweep) read it as outside rule
8's overlay scope, and its open/close behaviour is
unchanged, as the charter's second sentence
provides. No Esc arm was added. If the user rules
it an overlay, the join is one arm beside the menu
arm — recorded here as an available decision, not
an open defect.

**Shortcuts shown in the UI are real (P12).** All
three display surfaces cross-checked against
`main.rs::on_key`: menu hints, the help panel's
shortcut rows, and the palette's `command_hint`
table — the last guarded by the drift test
`command_hints_name_only_real_bindings`
(`command_palette.rs:2975`), under which every hint
spelling must be a help-panel key label. The
charter's "Ctrl+S lesson" is closed on this tree:
Ctrl+S is a real binding with a real Shortcuts row,
and the historical documented exception to the
drift guard is retired.

Two observations stand, deliberately unchanged:
non-Esc keys still fire stage actions while a menu
is open (rule 8 names Esc only; swallowing the rest
would drop live bindings and change un-named
behaviour — a product decision before any fix), and
Enter does not confirm the armed gates (binding
Enter to a destructive confirm that can be armed
behind another scope risks an unseen destructive
run; the charter names no Enter there).

**Copy is en-GB; numbers are in mono (rule 9,
P12).** Every string literal in the shell UI was
swept for US forms: all hits are keys or identifiers
(`fx-color`, Font Awesome icon names) or already
en-GB copy; the i18n catalogue's keys are en-GB
too, and the MCP descriptions use "centred" /
"grey" (their `color` tokens are parameter names,
not copy). Mono numerics are structural rather than
per-panel: the kit's `stepper_value` renders
`MONO_CAPTION.mono()` and `kbd` does the same —
the pinned Rune rev's MONO_CAPTION token does not
set the Mono family, so the builders apply
`.mono()` on top (a recorded Phase N charter
amendment) — and every panel on the kit stepper
inherits the discipline.

## 10. Rules 1, 2, 10 — method, census, proof

**Kit first (1)** held throughout: every landed
visual is a kit form (`flex_wrap` rows, kit overlay
tokens, `step_btn`, `empty_state`) or a call-site
application of one; the only new kit surface the
phase produced is P5's state kit in `design.rs`.

**Zero key loss (2).** The charter's 804 baseline
was the Phase N close figure under an earlier
extractor calibration. Under the gate's current
counter the same tree at `0b75051` reads 940
(827 static + 113 dynamic), and the operative
no-loss instrument — gate check 2, key diff vs
HEAD — was green at every stage of the phase's
integration. Phase O's own items added zero keys
(the overflow wraps, P4's overlays and P9's routing
changes touch no key); P6's fix likewise. At
close-out the whole tree reads **979 total unique
keys (863 static + 116 dynamic)**, the additions
belonging to Wave P/Q's other themes (itemised in
REDESIGN_PROGRESS §6.11 and the gate's floor
ledger), not to this phase.

**Proof (10).** Per item, as the charter requires:
construction pins in both themes for the state
fixes (P4, Q8); nine routing pins for the keyboard
work (P9); the end-drag recap pin (P6); the
language-grid model pins (O-1); the drift test for
shortcut truth (P12). The local instruments at
`911cf77`, run at this close-out (python only, per
the CI-only rule): **gate PASS — all 10 checks
green** (`scripts/phase_j_checks.py`), MCP census
PASS, overflow-scan self-test PASS.

The one proof still owed at close-out was
artifact-level: the p05 §2 scan table over a real
headless dispatch. Q12 completed the scan set as a
machine-enforced contract (`SCAN_SET` in
`scripts/overflow_scan.py`: 13 required trees
spanning the repaired surfaces — default, draw-open,
graph-duplicates, three-d, three-d-stage,
fx-split-mid, loop-range, palette-open, fx-selected,
effects-open, audio-open, and the subtitle lane's
two trees, promoted to required when the workflow
gained them); the tool scans every dump a dispatch
produces, and a debrief missing a required tree
fails as INCOMPLETE instead of reading clean. The
settings scenes' dumps ride the same dispatch and
the same scan; they are not SCAN_SET-required
entries, so O-1's artifact proof rests on the
scan's output for those dumps plus its in-file
model pins (§3).

**That table is now written — the debt is
discharged (Wave S, S8) — refreshed against a
newer dispatch (Wave V, V19), upgraded to
the first 16/16-complete dispatch (Wave W,
W12), and upgraded again to the Wave W
integration dispatch (Wave X, X15), and
upgraded again to the Wave Y closing
dispatch (Wave Z, Z12); the Wave X
integration dispatch was checked in full and
not taken (Wave Y, Y15 — its scan reads NOT
CLEAN; see the upgrade notes below).** Original source:
**Headless Rune probe run `42759195547`**,
completed **SUCCESS** on
`59cb0bec36b80648c9f9fe0dfd7d30cd6266d5c3`
(`59cb0be`, the Wave R integrated tree — it
contains the integrated Wave P+Q tree `911cf77` as
an ancestor, so every Phase O repair is in it),
artifact `headless-probe-output` (204 files, 34
`tree-*.json` dumps); its scan exited 0 with
`OVERFLOW SCAN: CLEAN (0 hits, scan set complete)`
under the then-current 13-tree SCAN_SET. One
disclosure the original discharge did not make:
that run's `subtitle-census.txt` reads
`SUBTITLE CENSUS: FAIL (5 assertion(s) failed)`
inside the green workflow — the fail-soft pattern
first flagged on this very run (the census then
sampled tree points in capture pixels; §10's
refresh notes below carry the current state).
The overflow verdict itself was genuinely
measured and stands, but a citation is only as
strong as its weakest transcript, so the table
below is re-sourced.

Previous source: **Headless Rune probe run
`42765217055`**, completed **SUCCESS** on
`9d7dd0a6e0853ce6fd6f0eef3c2ceee9a96bbd85`
(`9d7dd0a`, the Waves S+T tree carrying the
capture-scale census fix), artifact
`headless-probe-output` (217 files, 36
`tree-*.json` dumps). Its `subtitle-census.txt`
reads `SUBTITLE CENSUS: PASS` (19 PASS, 0 FAIL)
and its Home missing-audio marker reads `PASS` —
both read from the artifact itself, not from the
workflow conclusion. Method: exactly p05 §2 as
kept — `python3 scripts/overflow_scan.py --dir
<artifact dir>` (content / card / window rules,
0.5 pt tolerance, the closed exemption list).
Every one of the 36 dumps reads 0 non-exempt
hits, and every per-tree figure and per-item
measurement below was re-measured in these dumps:
each reproduces the original discharge's value
exactly, so the refresh moves no number — it
adds two rows, two SCAN_SET flags, and the
census-clean citation.

Previous source: **Headless Rune probe run
`42767540386`**, completed **SUCCESS** on
`fbc36448ac6c280ffbda893d187e4416eea84c36`
(`fbc3644`, the Wave U integrated tree plus the
one-character fix for the E0308 the refresh
notes below record), artifact
`headless-probe-output` (221 files, 37
`tree-*.json` dumps). This is the first
successful dispatch under the 16-tree SCAN_SET
contract: its scan reads `OVERFLOW SCAN: CLEAN
(0 hits, scan set complete)` — no INCOMPLETE
provenance gap — its `subtitle-census.txt`
reads `SUBTITLE CENSUS: PASS` (19 PASS, 0 FAIL),
and both Home markers read `PASS`, the
scan-fallback marker on its first real run —
all read from the artifact itself, not from
the workflow conclusion. Every figure below
was re-measured in these dumps: the 36
reproduce exactly for the third consecutive
dispatch, and the table gains the one row the
refresh notes owed, `tree-home-scan-fallback.json`,
required and CLEAN.

Previous source: **Headless Rune probe run
`42774207670`**, completed **SUCCESS** on
`dccddbb7d76dc038ea34622d101c6e632d69fa61`
(`dccddbb`, the Wave W integrated tree),
artifact `headless-probe-output` (224 files,
37 `tree-*.json` dumps). Its scan reads
`OVERFLOW SCAN: CLEAN (0 hits, scan set
complete)` — the 16/16 contract again — its
`subtitle-census.txt` reads `SUBTITLE CENSUS:
PASS` (19 PASS, 0 FAIL), and both Home markers
read `PASS` — all read from the artifact
itself, not from the workflow conclusion.
Every figure below was re-measured in these
dumps: 36 of the 37 counts reproduce exactly;
the one move is `tree-audio-open.json`,
408 → **462** elements (it already read 462 on
the Wave V round-2 artifact `42771786953`, so
the move predates Wave W and is stable across
both dispatches). Every O-1…O-9 measurement
reproduces exactly — the fourth consecutive
dispatch to do so.

Current source: **Headless Rune probe run
`42798031914`**, completed **SUCCESS** on
`a37f2aa03368e9272e10692449f901e662e4dfb5`
(`a37f2aa`, the Wave Y integrated tree at its
CI close; the build beside it, `42798005496`,
completed **SUCCESS** with test totals summed
from its own log at **4,248 passed, 0
failed**), artifact `headless-probe-output`
(224 files, 37 `tree-*.json` dumps). Its scan
reads `OVERFLOW SCAN: CLEAN (0 hits, scan set
complete)` — the contract now standing at 18
required trees (the Wave X clips carrier and
the Wave Y sprites carrier promoted since
the last citation) — its
`subtitle-census.txt` reads
`SUBTITLE CENSUS: PASS` (19 PASS, 0 FAIL),
both Home markers read `PASS`, its Home
thumbnail probe reads
`HOME THUMBNAIL PROBE: PASS` (pairwise
ink-coverage distances 0 / 51 / 48; the band
geometry leg within tolerance at 112×83/84),
and its live-MCP probe reads
`MCP PROBE: PASS` — all read from the
artifact itself, not from the workflow
conclusion. Every figure below was
re-measured in these dumps: 15 of the 37
counts reproduce exactly; 22 moved, all
upward (the Wave X+Y panel surface; the
upgrade notes carry the list). Every
O-1…O-9 measurement reproduces exactly —
the sixth consecutive dispatch to do so.
Per §2's output shape each
scene's hit list is empty, so the table records
the verdict per scene:

| Scene tree | Window | Elements | SCAN_SET | Non-exempt hits | Verdict |
|---|---|---|---|---|---|
| tree-audio-open.json | 1440×900 | 462 | required | 0 | CLEAN |
| tree-build-open.json | 1440×900 | 376 | — | 0 | CLEAN |
| tree-clips-populated.json | 1440×900 | 1287 | required | 0 | CLEAN |
| tree-default-1280.json | 1280×720 | 1215 | — | 0 | CLEAN |
| tree-default-light-1280.json | 1280×720 | 1215 | — | 0 | CLEAN |
| tree-default-light.json | 1440×900 | 1215 | — | 0 | CLEAN |
| tree-default.json | 1440×900 | 1215 | required | 0 | CLEAN |
| tree-dopesheet-keys.json | 1440×900 | 1292 | — | 0 | CLEAN |
| tree-draw-open.json | 1440×900 | 461 | required | 0 | CLEAN |
| tree-effects-open.json | 1440×900 | 562 | required | 0 | CLEAN |
| tree-export-open.json | 1440×900 | 408 | — | 0 | CLEAN |
| tree-frame-3.json | 1440×900 | 1215 | — | 0 | CLEAN |
| tree-fx-selected.json | 1440×900 | 643 | required | 0 | CLEAN |
| tree-fx-split-mid.json | 1440×900 | 698 | required | 0 | CLEAN |
| tree-graph-duplicates.json | 1440×900 | 448 | required | 0 | CLEAN |
| tree-graph-open.json | 1440×900 | 409 | — | 0 | CLEAN |
| tree-home-populated.json | 1440×900 | 375 | — | 0 | CLEAN |
| tree-home.json | 1440×900 | 291 | — | 0 | CLEAN |
| tree-home-missing-audio.json | 1440×900 | 334 | required | 0 | CLEAN |
| tree-home-scan-fallback.json | 1440×900 | 333 | required | 0 | CLEAN |
| tree-library-populated.json | 1440×900 | 1214 | — | 0 | CLEAN |
| tree-loop-range.json | 1440×900 | 1250 | required | 0 | CLEAN |
| tree-menu-open.json | 1440×900 | 1223 | — | 0 | CLEAN |
| tree-palette-open.json | 1440×900 | 1279 | required | 0 | CLEAN |
| tree-playing.json | 1440×900 | 1215 | — | 0 | CLEAN |
| tree-selected.json | 1440×900 | 1409 | — | 0 | CLEAN |
| tree-settings-open-error.json | 1440×900 | 1380 | — | 0 | CLEAN |
| tree-settings-open-light.json | 1440×900 | 1379 | — | 0 | CLEAN |
| tree-settings-open.json | 1440×900 | 1379 | — | 0 | CLEAN |
| tree-sprites-populated.json | 1440×900 | 1262 | required | 0 | CLEAN |
| tree-strokes-populated.json | 1440×900 | 514 | — | 0 | CLEAN |
| tree-subtitles-populated.json | 1440×900 | 1223 | required | 0 | CLEAN |
| tree-subtitles-speaker.json | 1440×900 | 1223 | required | 0 | CLEAN |
| tree-subtitles-top.json | 1440×900 | 1223 | required | 0 | CLEAN |
| tree-three-d-1280.json | 1280×720 | 433 | — | 0 | CLEAN |
| tree-three-d-stage.json | 1440×900 | 433 | required | 0 | CLEAN |
| tree-three-d.json | 1440×900 | 433 | required | 0 | CLEAN |

Per-item evidence — the repaired rows p05 §3
named, measured in these dumps (the right edge of
the row's subtree: the row element and everything
under its path; the budget is the §2 content edge
including the 0.5 pt tolerance):

| Item | Repaired row(s) | Tree(s) | p05 §3 (pre-fix) | Measured now | Budget | Verdict |
|---|---|---|---|---|---|---|
| O-1 | settings language chips (28 per tree) | settings-open, settings-open-light, settings-open-error | worst chip `settings-lang-sl` at 1463.0 (+51.0 past the content edge, +23.0 past the window); he 1450.5, it 1428.5, pl 1427.0 | worst chip in each tree is `settings-lang-fr` at **1392.5**; the four named offenders now end at 1185.5 (sl), 1148.0 (he), 1158.0 (it), 1150.5 (pl) | 1412.5 | CLEAN — 0 hits |
| O-3 | `layer-row-0` | default | overflow ≥ +134 (children reaching ≈466) | subtree ends **304.0** — the row box is exactly the 276 pt content width (x 28..304) | 304.5 (left) | CLEAN — 0 hits |
| O-4 | `draw-brushes`, `draw-tools-row`, `draw-shapes`, `draw-stamp-params`, `draw-stamp-params-2`, `draw-stamp-params-3` | draw-open | +563 / +374.5 / +132.5 / +42 | every subtree ends **1412.0** | 1412.5 | CLEAN — 0 hits |
| O-5 | `graph-actions`, `graph-presets`, `keyframe-presets`, `keyframe-actions`, `keyframe-clipboard` | graph-duplicates (`keyframe-actions` also in default) | +305.5 / +13.5 / +45 / +11.5 / +2.8 | every subtree ends **1412.0** | 1412.5 | CLEAN — 0 hits |
| O-6 | `wx-kind` | default, effects-open | +118.3 (`wx-seg-5` ended 1530.3, past the window) | ends **1412.0** in both trees | 1412.5 | CLEAN — 0 hits |
| O-7 | `orbit-3d`, `cam-frame-3d` | three-d, three-d-stage | +49 / +37.5 | **1168.0** / **1198.5** in both trees | 1412.5 | CLEAN — 0 hits |
| O-8 | `fx-nudge`; `pb-next-key`; `pb-loop-end` | fx-split-mid; audio-open (also default); loop-range | +27.5 / +17.6 / +36 | **1412.0** / **1166.5** / **1354.0** | 1412.5 | CLEAN — 0 hits |
| O-9 | `pose-apply-all`; `cam-delete-key` | default | +16.5 (ended 1428.5); +29.4 (ended 1441.4, past the window) | **1272.4** / **1193.9** | 1412.5 | CLEAN — 0 hits |

A note on reading the wrapped rows: a
`flex_wrap()` row's own box spans the full card
content width by design, so its subtree ends
exactly at the content edge (1412.0 against the
1412.5 tolerance edge) — the scanner's hit test
(no descendant past the tolerance edge, none
outside its card, none past the window) is the
verdict, and it reads zero on every row. The rows
that do not fill their card (O-7, O-8's playback
pair, O-9, O-1's chips) end with real headroom, as
measured above. QA_RUNE §17's matching row closes
on this same evidence.

**Refresh notes (Wave V, V19).** Three things
moved around the table even though no number in
it did. (1) *The scan set grew.* SCAN_SET stood
at 13 required trees for the original discharge
and 15 at the refresh dispatch — the subtitle
lane's speaker tree (Wave S) and the Home
missing-audio tree (Wave T), both rows above,
both CLEAN — and it stands at 16 in the current
contract (Wave U, IA-U12-1:
`tree-home-scan-fallback.json`). A rescan of the
refresh artifact under the current contract
therefore reads `OVERFLOW SCAN: scan set
INCOMPLETE`, for exactly one reason: the refresh
dispatch predates the Wave U scene and cannot
contain its tree. That is a provenance gap, not
a hit — every dump present reads 0. (2) *The
current tree's own dispatch has not yet produced
an artifact.* The first headless run on the
Wave U integrated tree, `42766902475`
(`1c7a70d`), concluded **FAILURE** at its own
"Build (release, all binaries)" step —
`error[E0308]` at `src/rune_sec/fx_clips.rs:452`
(expected `&str`, found `String`, at the
IA-U4-2 `kind_label` replace site) — so every
scene step was skipped and its artifact carries
only fail-soft stubs (its stub census reads FAIL
for absent snap inputs: nothing ran). Fixing
that is the integrator's work, not this table's;
it is recorded here so no one cites that run for
anything. (3) *What the next successful dispatch
owes:* the `tree-home-scan-fallback.json` row
(the 37th dump), a 16/16 complete scan, and a
census transcript read alongside the conclusion —
this table's citation upgrades to that run when
it lands.

**Upgrade notes (Wave W, W12).** The refresh
notes' item (3) is discharged: the next
successful dispatch landed — `42767540386`,
above — carrying the 37th dump, a 16/16
complete scan, and a census transcript read
alongside (PASS), so the table's citation
stands upgraded to it and the scan-fallback
row is in. No number moved: every per-tree
figure and every O-1…O-9 measurement
reproduces on the new artifact exactly, the
third consecutive dispatch to do so. The
citation does not move to the Wave V
integration dispatch: `42771069640` (on
`d5da9fd`) concluded **FAILURE** at its build
step — `error[E0716]`, temporary value dropped
while borrowed, at
`src/rune_sec/home_panel.rs:3052` — so no scene
ran, its artifact carries only fail-soft stubs,
and the Home snap pair gate's first transcript
verdict does not exist yet. It is recorded here
so no one cites that run for anything; the
citation upgrades to the first successful
Wave V-or-later dispatch, the pair gate's
verdict stated alongside, when the integrator's
fix round lands it. One further transcript
from the current citation's artifact, stated
for the record: its legacy thumbnail probe
still trips its control leg (`home-totals`
stage-bg share 0.672 against the 0.06 ceiling —
"premise does not hold"); diagnosing that leg
is Wave W's W7 slice, not this table's.

**Upgrade notes (Wave X, X15).** W12's
commitment is discharged: the citation upgrades
to `42774207670` (above), the first successful
Wave V-or-later dispatch against which this
table was re-verified end to end. (The Wave V
fix round's own dispatch, `42771786953` on
`2462a82`, also completed SUCCESS and its
artifact was read at the Wave V close; this
upgrade takes the newer dispatch, on the
Wave W integrated tree, as the standing
source.) One number moved —
`tree-audio-open.json`, 408 → 462, recorded in
the current-source paragraph and in the table;
every other figure and every O-1…O-9
measurement reproduces. The pair gate's
verdict, stated alongside as W12 required: on
this artifact the legacy thumbnail legs PASS
on real snaps for the first time — W7's
re-derived predicates read control shares
0.008 / 0.004 against the 0.06 ceiling, every
row over its floors — but the fidelity gate
stopped at its strip-geometry precondition:
the four strips do not share one pixel size
((112, 238) and (112, 239) px — "the fixture
geometry moved"), so the probe's verdict is
`HOME THUMBNAIL PROBE: UNPROVEN` and the
identity legs issued no verdict. No threshold
was loosened to reach this reading; the gate's
redesign is Wave X's X10 slice.

**Upgrade notes (Wave Y, Y15).** The Wave X
integration dispatch — Headless Rune probe run
`42784024319`, completed **SUCCESS** on
`d44098f` (the Wave X integrated tree),
artifact `headless-probe-output` (224 files,
37 `tree-*.json` dumps) — was checked end to
end by X15's method, and the citation does
**not** upgrade to it. What verified on its
artifact, read first-hand: its
`subtitle-census.txt` reads
`SUBTITLE CENSUS: PASS` (19 PASS, 0 FAIL) and
both Home markers read `PASS`; and two
transcripts pass live for the first time. The
Home thumbnail probe reads
`HOME THUMBNAIL PROBE: PASS` — the pair gate
live at last under X10's redesign (populated
pairwise ink-coverage distances 0 / 51 / 48
against the different-content floor of 12;
the missing-audio thumbnail at distance 0
from the singleton row and 48 from the pair;
the legacy legs at control shares 0.008 /
0.004, every row over its floors). The
live-MCP probe reads `MCP PROBE: PASS`
(exit 0) — its first pass since W4 flipped
the live-MCP default to explicit-enable, on
F7's probe-side repair (the probe's HOME now
seeds the stored `true` it exercises). The
build beside it, `42784003685` on the same
commit, completed **SUCCESS**, its test
totals summed from its own log at **4,158
passed, 0 failed**. What did not verify:
`scripts/overflow_scan.py --dir` over the
artifact reads `OVERFLOW SCAN: 40 hit(s) --
NOT CLEAN`. The hits are one family in one
card, identical in 20 dumps: X5's new
Draw-order row in the sprite panel
(`sprite-z-row`, Wave X) — its
`sprite-z-front` button ends at right edge
1426.0, **+14.0 pt [content+card]** past the
1412.5 tolerance edge, and the button's
unkeyed child ends at 1416.0 (+4.0 pt
[content]). SCAN_SET-required
`tree-default.json` is among the 20, so the
16/16 contract does not hold on this dispatch
and the table above stays sourced from
`42774207670` — the same script over that
artifact still reads CLEAN, re-verified in
this pass. Nothing in the family touches a
Phase O row: every O-1…O-9 measurement,
re-measured in the new dumps, reproduces
exactly — the **fifth** consecutive dispatch
to do so. Per-tree element counts did move,
all upward, in exactly the 20 dumps that
carry the hit family (the scenes whose sprite
panel renders its edit rows): the default
family 1161 → 1169, subtitles family
1169 → 1177, settings family +8 each,
dopesheet-keys, library-populated, loop-range,
menu-open and palette-open +8 each,
clips-populated 1224 → 1241, selected
1334 → 1358, sprites-populated 1174 → 1213 —
Wave X's new panel surface (the
`tree-default.json` key diff is exactly
`sprite-z-row` / `sprite-z-back` /
`sprite-z-front` added, none lost); the other
17 counts reproduce. Those numbers are
recorded here, not in the table: the table
keeps the counts of the run it cites, and the
next upgrade re-measures from its own
artifact, as every generation has. The z-row
now owes what every repaired row before it
paid — a wrap or a narrower anatomy — and the
citation upgrades to the first later dispatch
whose scan reads CLEAN with these transcripts
beside it.

**Upgrade notes (Wave Z, Z12).** Y15's
condition is discharged. Y14's `flex_wrap`
repair of the sprite Draw-order row landed in
Wave Y, and the first later dispatch whose
scan reads CLEAN is Headless Rune probe run
`42792705307` (Wave Y round 3, on `95e4a45`) —
verified first-hand in this pass: its scan
reads `OVERFLOW SCAN: CLEAN (0 hits, scan set
complete)` and its census, thumbnail probe
and MCP probe transcripts all read PASS. Per
X15's precedent the citation takes the newer
dispatch as the standing source: the Wave Y
closing dispatch `42798031914`, above, whose
per-tree counts are identical to
`42792705307`'s on all 37 dumps. The defect
itself, closed: `sprite-z-front`, which ended
at 1426.0 (+14.0 pt [content+card]) on the
Wave X frame, ends at **1194.5** on both
CLEAN dispatches, and the z-row's subtree
ends at 1412.0 — a wrapped `flex_wrap` row
filling its card, the anatomy every repaired
row in this table carries. What verified on
the closing artifact, read first-hand: scan
CLEAN under the contract as it now stands —
18 required trees, the Wave X clips carrier
(`tree-clips-populated.json`) and the Wave Y
sprites carrier (`tree-sprites-populated.json`)
promoted since the last citation, both rows'
SCAN_SET flags updated in the table;
`SUBTITLE CENSUS: PASS` (19 PASS, 0 FAIL);
both Home markers `PASS`; the thumbnail
probe PASS live again (pairwise distances
0 / 51 / 48 against the floor of 12; legacy
legs at control shares 0.008 / 0.004, rows
0.333/0.337/0.339, ink 115/115/59; the band
geometry leg spreads within tolerance 1 at
112×83/84); `MCP PROBE: PASS`. The build
beside it, `42798005496` on the same commit,
completed **SUCCESS**, its test totals summed
from its own log at **4,248 passed, 0
failed**. The measurer was calibrated before
any number was trusted (X15's discipline):
run against the previous citation's artifact
(`42774207670`, downloaded fresh), all 37
per-tree counts and every O-1…O-9 subtree
right edge reproduce the table exactly, so
the movements below are the new artifact's
facts. Per-tree counts moved in 22 trees, all
upward — Wave X+Y panel surface the cited
run predates: the default family
(default, default-1280, default-light,
default-light-1280, frame-3, playing)
1161 → 1215; the subtitles family
1169 → 1223; the settings family 1325 → 1379
(error tree 1326 → 1380); dopesheet-keys,
library-populated, loop-range, menu-open and
palette-open +54 each; clips-populated
1224 → 1287; selected 1334 → 1409;
sprites-populated 1174 → 1262; the graph
pair +2 each. The other 15 reproduce. The
`tree-default.json` key diff against the
previously cited artifact is exactly 21 keys
added, none lost — the sprite edit rows
(z / flip / follow / opacity / pivot /
scale-y) and `fig-setup-mode`. Every
O-1…O-9 measurement reproduces exactly —
the **sixth** consecutive dispatch to do so —
so the per-item table's measured column
stands unchanged.

**CI, stated once and exactly.** The last completed
master CI run before Wave R was on `0b75051`
(Build, Test & Binary, SUCCESS, observed via `gh`
during Wave Q). The integrated Wave P+Q tree's CI
(`911cf77`) is in flight; this close-out claims no
verdict for it. Everything above is therefore
stated as *in the tree and locally gated*, which is
what the evidence supports. (Postscript, Wave S:
the headless dispatch that statement waited for
has since landed and succeeded — run `42759195547`
on `59cb0be` — and its scan table, written above,
is the artifact-level proof this section owed.)

## 11. Honest remainders

Standing user decisions — named, not relitigated:

- **Accent contrast.** White on the accent fill
  measures 3.14:1 as rendered (UI mission acceptance
  review; the older 4.19:1 figure does not
  reproduce). The charter excludes darkening the
  accent, and Phase O did not touch it. Reaching
  ~4.5:1 remains the user's decision.
- **`tab_visits_controls`** stays OFF — the user's
  deferred product decision (the Play / frame-step
  conflicts are recorded in PHASE_N §8). Its
  consequence for this phase — no keyboard focus
  state for buttons — is §6's named engine limit.
- **Two-"New project" results disagree** (panel
  blank vs Home demo) — the user's standing product
  call; unchanged by this phase.
- **Legacy accent split** (stage/timeline canvas
  colours vs the token accent) — Phase N §8
  remainder the charter did not adopt; unification
  is a product decision, not a restyle.

Engine and scope limits, recorded:

- **Toggle knob slide** awaits a Rune rev that
  animates layout offsets (§7); **toast expiry
  fade** stays linear by the charter's own Phase N
  clause (§7).
- **Range MP4/WebM exports carry no audio** — the
  charter excludes audio-path work; the export
  panel states it outright (Phase M §30), so the
  feedback half is closed and the muxing half is a
  separate piece of work.
- **Sound is user-verified only**, as it has been
  since Phase F; no Phase O item could change that.
- **Settings-card Esc** (§9) and the **menu-open
  non-Esc key leak** (§9) stand as recorded
  readings/observations: no code is owed under the
  charter as written; either changes only on a
  product ruling.
- **The figure-inspector stale-selection copy**
  (§4) stands verbatim under rule 2.
