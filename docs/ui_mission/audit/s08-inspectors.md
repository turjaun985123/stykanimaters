# Squad 08 Audit — Inspectors & Property Editing

Scope: the four property-editing panels of the Rune shell —
`src/rune_sec/inspector_figure.rs` (1,745 lines),
`src/rune_sec/inspector_segment.rs` (1,527),
`src/rune_sec/inspector_style.rs` (1,669; the brief's
"inspector.rs"), `src/rune_sec/stroke_inspector.rs` (2,946) —
against the Phase N kit in `src/rune_sec/design.rs` and the
button families in `src/chrome_view.rs` / `src/rune_sec/icon_btn.rs`.

Method: full read of all four view builders + their row
helpers, kit source, mount points in `src/main.rs`
(605–611, 650), workspace visibility in
`src/rune_sec/workspaces.rs`, and **measured geometry** from
the Phase N headless tree dumps (`tree-selected.json`,
`tree-default.json`, `tree-draw-open.json`, 1440×900, dark).
All four panels mount in the **right column**: card 340 pt,
content budget **316 pt** (x 1096..1412 in the dumps). No
local build was run (AGENTS.md); nothing here is claimed
from screenshots — tree rects only.

Already good, preserve: all four panels sit on
`design::panel_col` with hairline + divider; numeric values
already use the kit `stepper_value` mono readout almost
everywhere; swatches already collapsed onto `design::swatch`
(24×24, accent selected ring); handlers and keys were kept
verbatim through Phase N. The defects below are therefore
about *structure, duplication and reachability*, not about
basic styling.

---

## Defects

### D1 — CRITICAL: Segment inspector "Bend" row overflows past the window edge (measured)

`inspector_segment.rs` — the Bend row (label + stepper value
+ `−5°` `−1°` `+1°` `+5°` + `Straighten`, built inline at
the `seg-bend-*` keys) measures, from `tree-selected.json`:

- `seg-bend-m5` starts x=1176; `seg-bend-zero` ("Straighten")
  rect = [1367, …, **79.5 wide**] → row content ends at
  **x = 1446.5**.
- Card content edge is 1412, card edge 1424, **window edge
  1440**. The row overruns the 316 budget by **+34.5 pt**
  and its last button ends 6.5 pt *beyond the window* —
  the right end of "Straighten" is off-screen/unclickable
  at the reference size.
- The sibling Rotate row fits with **zero slack**:
  `seg-rot-p5` ends exactly at x=1412.
- Fixed-cost arithmetic (measured button widths: °-buttons
  38.5–41, Straighten 79.5, stepper value ≈44 incl. gaps):
  the Bend row's non-label content alone is ≈316 — the
  "Bend" label survives only because the row is allowed to
  bleed. Any wider font metric or locale makes Rotate
  overflow too.

### D2 — CRITICAL: One property, three incompatible colour editors; the same node's colour is editable in two stacked panels at once

Node colour is edited three different ways in the same
right-column stack (Animate shows Segment + Figure + Style
together, `main.rs:605–611`, `workspaces.rs:266–268`):

- **Segment inspector**: 6-swatch palette (`PALETTE: [u32; 6]`,
  `inspector_segment.rs:189`) + a "Cycle" button
  (`node-color`). No numeric entry.
- **Style panel**: RGB steppers (±8/press, `RGB_STEP`,
  `inspector_style.rs:284`), gradient swatches + gradient
  RGB steppers, recents, presets — but **no palette
  swatches at all**.
- **Figure inspector**: a *different* 16-swatch palette
  (`PALETTE: [u32; 16]`, `inspector_figure.rs:192`, chunked
  8+8) for figure colour, hex shown as a read-only readout.

Consequences: the two palettes disagree in size, contents
and order; a user who sets a segment's colour by swatch in
one panel must switch mental models to refine it in the
panel directly below; reaching an exact RGB value costs up
to 32 presses per channel at ±8; and **no panel anywhere
offers hex/text entry** — the figure panel even displays
`#RRGGBB` as an uneditable readout. Colour is the single
most-used property in a drawing tool; it is the least
unified surface in the app.

### D3 — HIGH: Brush "steppers" do not set values — they silently swap the whole preset

`stroke_inspector.rs:453` (`step_param`) + `:400`
(`step_index_by`): pressing +/− on Size, Opacity, Scatter
or Smoothing selects the *nearest library preset* whose
that-parameter value is larger/smaller, via
`apply_preset` (`:439`) — which changes **every other
parameter at once** (width, opacity, scatter, smoothing,
nib, and the brush name in the section header). The rows
are dressed exactly like the true value steppers in the
other inspectors (same label + `stepper_value` + −/+
construction, `param_row` at `:1330`), so the user cannot
tell that "Size +" will also change their opacity and nib.
This is a functional-honesty defect, not a styling one:
either the rows must edit a working copy of the brush's
values, or they must be presented as what they are
(preset stepping).

### D4 — HIGH: The Animate inspector stack is ~1,400 pt tall; no grouping or disclosure anywhere

Measured populated heights (tree dumps): Segment 494.75,
Figure 401.38, Style ≈ 500 populated (70.38 empty) — plus
Pose tools and 12 pt gaps — in a 900 pt window whose right
column also carries settings/help. The richest editor
(Style) sits two scrolls down. Inside the panels there is
no progressive disclosure to relieve this:

- **Segment** prints five read-only readout rows
  (Length / Thickness / Angle / Bend / Shape) and *then*
  repeats four of the same values as editable stepper /
  button rows — Bend's value appears **three times**
  (`seg-bend-readout` line, `readout_row("Bend", …)`, the
  Bend row's own stepper value). ≈5 rows of pure
  duplication ≈ 165 pt.
- **Figure** has exactly one section label ("Colour") —
  placed above the swatches, so the Opacity stepper,
  Scale, Head-ring, placement and Flip/Duplicate/Delete
  rows that follow all read as part of "Colour". The
  Head-ring row is a **project-wide** setting
  (`step_head_ring_scale`, `:739` — "not per-figure", its
  own doc says) living unlabelled inside a per-figure
  panel.
- **Style** shows everything at once: Taper, Cap,
  Gradient, 3 gradient channels (the panel's one good
  disclosure — rendered only when an end colour is set),
  Colour header + 3 channels, Recents, Opacity,
  Copy/Paste, Presets.
- **Stroke** mixes three scopes in one unsectioned flow:
  project-wide bulk ops at the top, project-wide stroke
  *selection* rows, per-stroke editors, then brush
  parameters, then the frame's stroke list — separated by
  a single section label ("Brush · {name}").

### D5 — HIGH: No-selection / empty states are bare text in all four panels; three different constructions

`grep -c empty_state` = **0** in all four files. The kit's
`design::empty_state` (glyph + message, charter §10) is
used by none of them:

- Figure, no figures: the whole card is one bare LABEL
  line, "No figures in the project." — **no header, no
  title** (`inspector_figure.rs:864`).
- Segment, no selection: same bare-line construction,
  "Select a segment on the stage to edit it." — again no
  header (`inspector_segment.rs:670`).
- Style, no selection: header **is** rendered, then a bare
  line, "Select a segment on the stage." — a third
  construction, and near-duplicate copy of Segment's
  message in the panel directly below it (`inspector_style.rs:806`).
- Stroke, empty frame: "No strokes on this frame." as a
  bare line under the strokes header (`stroke_inspector.rs:1707`)
  — the Phase O seed item, still open.

A new user in Animate meets two of these stacked bare
cards before they have selected anything.

### D6 — MEDIUM: Four stepper dialects; no kit stepper row; no sliders; no direct numeric entry

The kit exports `stepper_value` (the readout) but no
stepper *row*, so every panel hand-rolls its own — and
they differ:

1. `−`/`+` text `btn`s — figure opacity, segment
   length/thickness, style RGB + opacity, stroke
   `param_row`.
2. Chevron `icon_only_btn`s (26 pt) — stroke per-stroke
   Width / Opacity rows (`stroke_inspector.rs:1527`,
   `:1573`).
3. Multi-button clusters — segment Rotate / Bend
   (±1°/±5° + Straighten), figure Scale (icon buttons
   labelled "90%" / "110%", no readout of the current
   scale anywhere).
4. A lone cycle button — style Taper ("Step"), segment
   Shape ("Cycle shape"), segment Colour ("Cycle").

And across all four panels: `slider_track` uses = **0**
(the kit slider exists and is the established pattern in
`scene_panel.rs` / `layers_panel.rs`), and `text_input`
uses = **1** (the figure *name* field). Every numeric
property — lengths, angles, RGB, opacities — is
stepper-only. Step granularity for the same concept also
disagrees: Opacity steps 0.10 in Figure (`:182`) and
Stroke (`:922`) but 0.05 in Style (`:254`).

### D7 — MEDIUM: Destructive actions with no confirmation and no visual hierarchy of risk

`grep -c confirm` = **0** in all four files. One press
each, no confirm step: "Delete figure" (`fig-del`),
"Delete segment" (`del-seg`), "Delete all" strokes —
**project-wide, all frames** (`stroke-delete-all`,
`stroke_inspector.rs:1430`) — "Delete frame strokes",
"Clear frame" (`strokes-clear`), plus per-stroke Delete
rows. Mission §8 requires confirmations for destructive
operations; undo exists and mitigates, but the
project-wide stroke delete sits in the Draw panel's
second row, styled identically to its neighbours
(`icon_btn`, not even the danger family — figure/segment
deletes at least use `danger_btn` / `danger_icon_btn`).

### D8 — MEDIUM: Label typography splits two ways inside one column stack

Figure/Segment/Stroke field labels are `type_::LABEL` in
`t.text_secondary`; Style's field labels ("Taper", "Cap",
"R", "Gradient R", …) are `design::section_label` —
SECTION font, a heading style used as a field label
(`inspector_style.rs` `channel_row_impl:598`, taper/cap/
opacity/copy-paste/presets rows). In the stacked Animate
column the same job wears two type styles. Minor related
inconsistencies, same item family:

- Headers: Figure `panel_title(None, …, Some(name))`
  duplicates the name the Name field directly below
  already shows; Segment uses an icon ("shapes") + no
  meta; Style and Stroke use no icon.
- Style preset chips lost their colour square in Phase N
  (documented at `preset_chip:679`): presets are
  colour+cap pairs now identified by **name alone** — the
  user cannot see a preset's colour before applying it.
- Style gradient "Off" swatch renders in `control_off`
  fill (`gradient_swatch:699`) — reads as *disabled*, not
  as a value.
- Stroke stroke-list rows are unkeyed `row_shell`-less
  hand rows with a text "Delete" `btn`, unlike the kit
  `list_row` rows elsewhere; row click doesn't select the
  stroke (selection is only via the prev/next stepper
  row) — the list looks like the selection surface but
  isn't one.

### D9 — LOW: Silent no-op affordances in the Segment panel

`seg-nav-parent` / `seg-nav-child` are always live while
a node is selected; at the root or a leaf the press is a
documented *silent* no-op (`inspector_segment.rs:727`
comment). Same family: the Length and Bend rows are
*omitted* at the root while Thickness/Rotate stay — the
panel's shape changes with selection in a way nothing
explains. (Cheap fix direction in item I8.)

---

## Improvement items (each sized for ONE coder)

Ownership follows the mission rule: one primary owner per
file; kit work coordinated through Squad 02. Every item
preserves all existing keys/handlers unless it says so.

### I1 — Fix the Bend/Rotate row overflow (D1)
Owner: `inspector_segment.rs`. Split Bend into two lines
(stepper cluster on line 1 with the value; "Straighten" as
a full-row secondary button on line 2 — the figures-panel
fix-5 precedent), or drop to a −/+ stepper pair matching
Length/Thickness and move ±5° to a second row with
Straighten. Acceptance: measured from a fresh tree dump —
every `seg-bend-*` / `seg-rot-*` rect inside x ≤ 1412 with
≥ 8 pt slack; keys `seg-bend-m5/m1/p1/p5/zero`,
`seg-rot-*` all survive verbatim. Verification: headless
`--scene selected` tree + Squad 19 overflow scan.

### I2 — One kit stepper row (D6, enables I3/I6)
Owner: Squad 02 → `design.rs`. Add
`stepper_row(key, label, value, dn_key, up_key, …)` —
today's dialect-1 construction (label grows, −,
`stepper_value`, +) as a kit builder, plus an optional
slider variant pairing `slider_track` with the same
readout. No panel changes in this item. Acceptance: kit
tests construct both variants on both themes; zero keys
invented inside the builder (callers pass them, per the
kit's standing rule).

### I3 — Adopt the kit stepper row in all four panels (D6)
Owners: one coder per file, after I2. Figure opacity +
head-ring, segment length/thickness, style RGB/gradient/
opacity, stroke `param_row` collapse onto I2's builder;
stroke Width/Opacity chevron rows convert to the same
−/+ form. Keys/handlers byte-identical. Acceptance: grep
shows no remaining hand-rolled stepper rows in the four
files; census stays 804.

### I4 — Unify colour editing (D2)
Owners: `inspector_style.rs` + `inspector_segment.rs`
(one coder, both files — the change is the relationship
between them). Decide node colour lives in **one** panel:
Style keeps RGB steppers + recents + presets and *gains*
the palette row; Segment's swatch row + "Cycle" button
are removed, replaced by a one-line summary row (current
colour chip + hex, pressing it is a no-op-free jump is
out of scope — simply omit). Both palettes merge to the
figure panel's 16-colour `PALETTE` as the single shared
constant (move it to `design.rs` or a shared module —
Squad 02 coordination). Figure colour gains the same
RGB-stepper trio under its palette for exact entry.
Acceptance: exactly one `PALETTE` definition in the
tree; the same 16 swatches in Figure and Style; segment
colour still settable end-to-end (Runtime test presses a
Style swatch and reads the node colour). Keys:
`seg-color-*` / `node-color` retirements must be listed
in the census delta and approved by the integrator —
this is the one item that intentionally changes the key
set.

### I5 — Make brush parameter rows honest (D3)
Owner: `stroke_inspector.rs` (+ engine consult). Two
acceptable outcomes, integrator picks one: (a) preferred
— steppers edit a working brush: add a core op that
sets the active brush's parameter directly (custom
brush state), rows keep their form and become truthful;
(b) fallback — relabel the block as preset stepping
(section label "Brush preset · {name}", rows gain a
"preset" affordance, and a hint line names that stepping
swaps presets). Acceptance: a Runtime test presses
"Size +" and asserts *only* size changed (a), or the
panel copy + section label state the preset semantics
(b). No silent middle state ships.

### I6 — Sliders for the continuous properties (D6)
Owners: per file, after I2's slider variant. Figure
opacity, style opacity, stroke brush opacity + selected
stroke opacity/width gain `slider_track` drags beside
their steppers (the scene/layers precedent: the caller's
root keeps key + `on_drag`, kit track is visual).
Acceptance: drag changes the value in a Runtime test for
one panel per file; steppers remain (keyboard/precision
path); no row exceeds budget (slider is fixed 200 pt +
stepper readout ≈ 250 < 316).

### I7 — Kit empty states in all four panels (D5)
Owners: per file (four small items, may be one coder).
Figure/Segment/Style/Stroke empty branches adopt
`design::empty_state` with glyphs from the curated table
(figure → "shapes" family, segment → "circle-nodes"-style
existing name — coder verifies via `icons::has_icon`;
stroke → the draw glyph already used by Draw tools).
Unify the construction: header always rendered (Style's
form wins), empty state below the divider; Segment and
Style messages differentiated (Segment: "Select a segment
on the stage to edit its geometry." / Style: "…to edit
its style."). Acceptance: `grep -c empty_state` ≥ 1 per
file; headless snaps of default + draw-open scenes
pixel-read by Squad 19.

### I8 — Segment panel de-duplication + disclosure (D4, D9)
Owner: `inspector_segment.rs`. Delete the five
`readout_row`s whose values the edit rows already show
(Length, Thickness, Angle, Bend); keep Shape as a
readout (it has no value-bearing edit row) and keep the
topology + bend header lines. Convert nav buttons to the
muted/inert treatment at root/leaf (the `btn_inert`
family already exists in `chrome_view`) with the status
line naming why ("Already at the root joint.") —
`select_parent`/`select_first_child` learn to report.
Acceptance: populated panel height ≤ ~340 measured;
Bend's value appears exactly once; Runtime test presses
Parent at root and reads the status.

### I9 — Figure panel grouping (D4)
Owner: `inspector_figure.rs`. Real sections via
`section_label`: "Colour" (palette + hex readout),
"Appearance" (opacity, scale, head-ring — head-ring
label gains "(all figures)" to state its project scope),
"Placement" (centre/ground + measure line), actions row
last, unlabelled. Header meta drops the duplicated name
(the Name field below carries it). No key/handler
changes. Acceptance: tree dump shows the four labels in
order; panel height within ±10 pt of today's 401.

### I10 — Style panel label + preset/gradient fixes (D8)
Owner: `inspector_style.rs`. Field labels move from
`section_label` to the LABEL/`text_secondary` form the
other three panels use (a tiny local `field_label`
helper matching `readout_row`'s label, or the kit's if
Squad 02 adds one). Preset chips regain a colour square:
extend via `chip_btn`'s icon slot only if Squad 02
agrees, else render the 12×12 square as a leading
sibling inside the row — colour visible before applying.
Gradient "Off" swatch gets the `card_border` hairline
treatment so it reads as a value, not a disabled slot.
Acceptance: type-style grep audit clean (no
`section_label` as field label in the file); preset row
rects inside budget (3-per-row grid retained).

### I11 — Destructive-action confirmation pass (D7)
Owners: per file. Project-wide deletes (`stroke-delete-all`,
`strokes-clear`, `stroke-delete-frame`) and figure/segment
deletes gain a two-press confirm state (press once →
button turns danger-filled "Confirm delete?", press
again within the same panel state → executes; any other
press resets) — the cheapest pattern that needs no new
overlay system; if Squad 17/18 standardise a confirm
dialog, adopt that instead. Acceptance: Runtime tests —
first press deletes nothing and flips the label, second
press deletes; census gains no keys (state lives in the
existing edit-state structs / State fields with
`#[serde(default)]` if persisted — prefer view-local).

### I12 — Stroke panel sectioning + list rows (D4, D8)
Owner: `stroke_inspector.rs`. Three `section_label`s:
"All strokes" block stays top but gains the label
"Project" scope framing via section labels "Selection",
"Brush · {name}" (exists), "Frame {n} strokes" (the
existing strokes header becomes the section label).
Stroke-list rows adopt `design::list_row` with the row
itself pressable to select that stroke (new handler on
the row root; per-row Delete keeps its key), selected
row filled with `selected_fill`. Acceptance: clicking a
list row moves `stroke_selected` in a Runtime test;
rows render inside budget; `stroke-del-{idx}` keys
unchanged.

### I13 — Opacity step consistency (D6)
Owners: `inspector_style.rs` (one-line constant) with
Squad 02 sign-off. Style `OPACITY_STEP` 0.05 → 0.10 to
match Figure/Stroke/Layers, *or* document the finer step
as deliberate in the panel (a "%" readout precision
note). Acceptance: the three panels' opacity constants
agree, or the difference is written down in
`docs/QA_RUNE.md`. Trivial; listed so it is decided, not
drifted into.

---

## Verification notes for Squad 19 / 20

- Re-run the overflow scan (Phase O p05 method) over a
  fresh `selected` scene after I1: the Bend row is the
  scan's best regression case (it must flag the *old*
  tree and pass the new one).
- The `selected`, `default` and `draw-open` scenes
  exercise all four panels' empty and populated forms;
  I7/I8 change heights — re-measure the right column's
  total and re-check the 1440×900 first screen.
- Census watch: only I4 proposes key retirements
  (`seg-color-*`, `node-color`); everything else is
  zero-key-loss by construction.

## Known limitations of this audit

- Geometry is from the Phase N headless dumps (dark,
  1440×900) — the populated Style height is estimated
  from its row inventory, not measured; light theme and
  other window sizes were not measured for these panels
  (mission Phase 7 owns that gap).
- Whether a direct-set brush op (I5a) is engine-cheap
  was not determined here — `paint.rs` / `editor.rs`
  belong to other squads' scopes; I5 is written so the
  fallback (I5b) is shippable without them.
