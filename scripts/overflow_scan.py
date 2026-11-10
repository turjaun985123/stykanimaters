#!/usr/bin/env python3
"""Overflow scan for headless --tree dumps (Phase O item O-2).

Method: docs/phase_o_plans/p05-overflow-widths.md §2, as run
by the S19 audit (docs/ui_mission/audit/s19-visual-qa.md §1).
Rune does not clip, so any element wider than its card
paints past the card (and possibly past the window) and the
excess controls cannot be clicked -- the side columns scroll
vertically only. This scan finds that defect family in the
CI headless tree dumps; it belongs to the headless debrief,
not the source gate (the dumps are CI artifacts).

Usage:
    python3 scripts/overflow_scan.py <tree.json> [tree.json ...]
    python3 scripts/overflow_scan.py --dir <dump-dir>
    python3 scripts/overflow_scan.py --scan-set
    python3 scripts/overflow_scan.py --self-test

--dir scans every tree-*.json in a dumps directory and
additionally checks the Wave Q scan set below: a required
tree absent from the directory is a failed pass (reported
MISSING), because the surfaces it carries were not proven.
--scan-set prints that set and exits.

Prints per-file hits, worst first, as
    (element key, parent card, overflow pt, rules fired)
and exits non-zero if any non-exempt hit is found in any
file. A run is CLEAN when every file reports zero hits.

Rules (tolerance 0.5 pt everywhere). The window width W is
read from the tree itself -- the root element's rect (#0)
is the window -- so trees dumped at any size are judged
against their own window. The column geometry generalises
from the reference calibration (W = 1440):

- The left column is anchored to the left edge and never
  moves: margin 16, width 300, card x-range [16, 316],
  content right edge 304 -- the same at every W.
- The right column is anchored to the right edge: width
  340, margin 16, card x-range [W-356, W-16], content
  right edge W-28 (1412 at W = 1440).
- Only the centre column absorbs a width change, and it
  carries no width budget (rules 1-2 apply inside the side
  columns only); the window rule applies to it, as to
  overlays.

At W = 1440 every derived value equals the original
hard-coded constant, so verdicts on 1440x900 trees are
unchanged (the self-test pins this).

1. Content rule. Every element under /side-left/ must have
   right edge <= 304.5; under /side-right/ <= (W-28) + 0.5 --
   except the card roots themselves (a card is 300/340 wide
   by design; its own right edge may reach 316 / W-16). Card
   root = the path segment directly under the column.
2. Card rule (defence in depth). Every descendant of a
   card root must lie within the card root's own x-range.
   (Rect-derived, so already width-agnostic.)
3. Window rule. No element anywhere may cross x < 0 or
   x > W (overlays included).
4. Timeline-card rule (Wave 10). The timeline card is
   centre-column chrome: an unkeyed card holding one
   horizontal scroll row (keyed `timeline`) plus, under
   it, the non-scrolling loop row. The card is found
   structurally -- it is the scroll row's parent element
   -- because Phase N deliberately left it unkeyed.
   - Every descendant of the card must lie within the
     card's own y-range. The scroll inside the card is
     horizontal only (scroll_x never moves y) and the
     centre column does not scroll vertically, so every
     y in the subtree is a painted y; Rune does not
     clip, so a row past the card's bottom edge paints
     over the status strip. This is the vertical budget
     the dopesheet lane is checked against: the lane
     adds one row per figure, so it is the first
     timeline content whose height grows with the
     project.
   - The card's non-scroll content -- the scroll row
     itself (its rect is the viewport, w_full inside
     the card's padding) and everything under the card
     but outside the scroll row (the loop row) -- must
     also lie within the card's x-range, the card rule
     (rule 2) extended to this one card.
   - The card root itself must fit the window
     vertically (rule 3 is horizontal only): a card
     pushed past the window's bottom edge is the same
     defect one level up.
   Strict descendants of the scroll row (the content
   column, ruler, chips, marker row, clips lane,
   dopesheet lane: headers and cells) get NO horizontal
   judgement: their rects are drawn positions under a
   live horizontal scroll offset, the dump carries no
   offset field this scan could correct by, and content
   wider than the viewport is a working scroll, not an
   overflow. The rule is inert when no `timeline`
   element exists (a zero-frame project: the card then
   holds only the kit empty state, no fixed-geometry
   content to judge).

Wave 12 audit -- the two populated scenes. Wave 12 adds
the first trees whose Home card and stroke inspector
carry live rows (scenes `home-populated` and
`strokes-populated`): project rows with 56 pt
thumbnails, metadata lines and wrapping button rows
under `home-panel`, and per-stroke rows (swatch,
truncated summary, Delete) under `stroke-inspector`.
No rule needed generalising, and none was weakened:

- Both panels are ordinary side-column card roots --
  in the Wave 11 dumps `home-panel` sits at
  #0/main-row/side-left/home-panel ([16, 157, 300, 49],
  the closed card) and `stroke-inspector` at
  #0/main-row/side-right/stroke-inspector
  ([1084, 786.5, 340, 501.13]) -- so rules 1-2 already
  judge every populated descendant horizontally,
  including a row escaping its card (the Wave 9 clips
  failure mode). Neither panel contains a scroll_x
  subtree, so no new exemption was needed.
- No rule assumes a single column, a fixed row
  height, or the empty-Home structure: the checks are
  per-element x-range tests, and populated lists only
  add vertical extent, which the side columns absorb
  by scrolling (the draw-open inspector card already
  ends past y=1287 in a 900 pt window, legitimately).
- The Home rows are a vertical list, not a grid --
  one thumbnail per row, inside the row -- so no
  tiling exception arises; the thumbnail is judged
  like any other element.

The self-test pins both shapes: conforming populated
trees (rows running below the window's bottom edge
included) scan clean, and one planted row escape per
scene is caught by the content and card rules.

Wave 13 audit -- the sprite surface and the saved
library rows. Wave 13 adds two more tree shapes:
sprites-panel rows (squad A's new panel) and the
library panel carrying SAVED entries (scene
`library-populated`, D1). Again no rule needed
generalising, and none was weakened:

- The sprites panel is an ordinary side-column card:
  one row per instance (asset name, key count, Delete)
  plus, for the selected instance, the per-key
  steppers (x, y, rotation, scale). Steppers and
  Delete buttons are fixed-width children ending at
  the content edge -- exactly what rules 1-2 judge.
  The panel has no horizontal-scroll subtree (its
  list scrolls vertically at most), so no exemption
  arises. The fixture's keys and row anatomy follow
  A3's shipped source (`sprites_panel.rs`, which
  landed during this audit); only the panel's mount
  (column, y) is modelled, on the stroke inspector's
  card geometry. That modelling choice dates from
  when the workspace gating and the `sprites-populated`
  scene were still in flight; the scene has since
  landed and headless.yml dumps its tree, so the
  debrief scan validates the shape against the real
  dump and the fixture's placement is corrected to
  the dump if they differ.
- The library rows' width anatomy is unchanged by
  the 44 pt thumbnails (Wave 12 D5): the thumb and
  the seg count are the first line's only fixed
  children (83 pt of its 236 pt) and the name
  truncates, so the Wave 10 verdicts stand. What the
  populated scene adds is the saved rows' button line
  (Insert / Rename / Delete) -- the fixed children
  whose 263 pt total caused the Wave 10 escape in
  this very panel. The fixture is modelled on the
  real Wave 12 dump geometry (card [16, 754, 300,
  329], rows 73 pt, thumb 44x44 at row x+8, count
  ending at 296, `lib-sort` and `library-collapse`
  ending exactly at the 304 edge).
- The library list rides a VERTICAL scroll (a
  ScrollSpec at a fixed 200 pt height): scrolled
  rows dump past the list's bottom and the window's,
  legitimately, but scroll_y moves no x, so rules
  1-2 judge every row exactly as they judge the
  un-scrolled Home list. SCROLL_SEGMENTS stays a
  horizontal-only closed list -- exempting the
  library list would blind the scan in the one panel
  where fixed row children have already escaped once.

The self-test pins both shapes the same way:
conforming trees (below-the-fold rows included) scan
clean, and one planted escape per shape -- a stepper
button, a saved row's Delete -- is caught by the
content and card rules.

Wave P audit -- the changed surfaces of Squads 1-2
against the full tree census. The headless harness
produces 36 trees: the 33 registered scenes at the
default 1440x900 plus default / default-light /
three-d at 1280x720 (headless.yml tree steps). This
wave changed no scene and no panel mount; its
surfaces ride existing trees, and every one of them
is judged by rules 1-3 (rule 4 for the timeline
card) exactly as it lands in the dump:

- O-1 settings language grid (settings_popover):
  trees settings-open, settings-open-light,
  settings-open-error. The grid's chips are
  descendants of the `settings-card` alias, which
  exempts only the card element itself -- the chips
  were always real hits and still are.
- O-3 layers rows (layers_panel, LEFT): every
  Animate tree (default, frame-3, selected, playing,
  loop-range, clips-populated, dopesheet-keys,
  library-populated, sprites-populated, the overlay
  scenes over Animate, and the 1280 default pair).
  The two-line row anatomy had no fixture; one is
  added below.
- O-4 draw cluster (Draw): draw-open,
  strokes-populated. O-5 graph + keyframe rows
  (Graph): graph-open, graph-duplicates (keyframe
  tools also ride every Animate tree). O-6 scene
  weather segments (Scene section): the Animate
  trees and effects-open. All three are ordinary
  side-column rows; rules 1-2 already judge them.
- O-7 three_d_panel wraps (Build3D): three-d,
  three-d-stage, three-d-1280, build-open.
- O-8 fx nudge split (Effects): fx-split-mid is the
  tree whose selection mounts the two-line
  `fx-nudge` anatomy; no fixture modelled it -- one
  is added below. The playback wraps (Playback
  section) ride the Animate trees and audio-open;
  loop-range carries the loop row's set state.
- O-9 pose row (Animate trees) and camera rows:
  the camera PANEL rides the Animate and Build3D
  trees, but no scene plants a camera key, so the
  key row (`cam-delete-key`) is in NO tree -- see
  the unproven list below.
- Empty states (P7/P8): the kit `empty_state`
  component (centred glyph + wrapped message) had
  no fixture; one is added below, modelled on the
  camera panel's no-keys state as rendered in the
  default tree, with the historical defect as its
  planted escape (the clips panel's unwrapped
  message measured 442 pt in a 316 pt card, +63
  past the edge -- design.rs). The states that DO
  render headlessly: camera (default), project
  list (default / export-open / home -- the probes
  wipe ./projects, so the list is empty), segment
  inspector (default), stroke inspector
  (draw-open), text tool (draw-open), 3D
  sub-sections (build-open), Home library (home),
  fx presets (effects-open).
- Interaction states (P4/P5): kit controls' REST
  and INERT geometry is in every tree and judged;
  hover / pressed / focus renditions are planted by
  no scene and appear in no dump, so no rect exists
  for the scan to judge (unproven list).
- Squad 2's fixes are core logic (bend permutation,
  missing-audio surfacing, format_version, export
  funnels): no side-column render surface. The one
  visible trace, P14's status text, rides the
  centre status strip, where only the window rule
  applies -- as it does to every element.

No rule needed generalising, and none was weakened;
the exemption list is unchanged. Named here so the
debrief does not mistake absence for coverage --
the surfaces NO tree carries, which this scan
therefore cannot judge: the camera key row (no
scene plants a camera key); the settings card in
the longest language (no scene sets a language --
P11's surface); the figure-inspector empty state
(every scene keeps the demo figures); the fx
filter empty state and the Home search-no-match
state (no scene types a query); hover / pressed /
focus renditions of kit controls.

The self-test pins the four newly modelled shapes
the same way as the earlier waves: conforming trees
scan clean at both window sizes, and one planted
escape per shape is caught at the measured
overflow.

Wave Q scan set -- the named proof set for the Wave P
surfaces (P43's pass, finished by Q12). The section 2
method scans EVERY dump a dispatch produces; the set
below (SCAN_SET in the code) is the subset the debrief
must be able to point at by name. The hand-offs that
named them: P2 (draw-open, graph-duplicates, default),
P3 (three-d, three-d-stage, fx-split-mid, loop-range,
default), P4 (palette-open, fx-selected, effects-open,
default). audio-open joins them as the Subtitles
panel's carrier (the panel mounts in the Audio
workspace -- P31, built by Wave Q's Q7; the O-8
playback wraps ride it too).

The subtitle lane trees are REQUIRED: `subtitles-populated`
and `subtitles-top` are registered scenes in the tree
(src/rune_sec/scenes.rs, planted by the P33 work) and
are the lane's only carriers, since the lane renders
only when a cue exists. They were PENDING until the
integrator applied IA-Q12-1 (Wave Q integration,
2026-11-10): headless.yml's tree loops now dump both,
so a CI artifact missing them fails the pass. Wave S
added a third lane tree on the same terms:
`subtitles-speaker` (S2's credited-cue scene) entered
the set as REQUIRED from the start, because IA-S2-1's
workflow dumps landed in the same integration -- the
Q12 precedent applied in one step instead of two.

Which rules judge the subtitle surfaces, for the
debrief: the stage draw is centre-column canvas paint
-- no element exists for it in any dump (scenes.rs
fact 32), so its proof is Q13's pixel census, not this
scan. The timeline lane rides inside the timeline
card, so rule 4 judges it (vertical fit of the card's
subtree; its cue blocks are scroll content,
horizontally exempt like the clips lane's spans). The
Subtitles panel is an ordinary right-column card in
the Audio workspace, judged by rules 1-2 in
audio-open.

Wave R maintenance (R17): the scan set still has 13
entries, all required and no pending entries. At the
R17 audit, R16 had registered no new headless scene
or tree in the shared tree: `scenes()` still listed
the same 31 names, and the harness cross-check still
parsed 34 produced tree dumps (the 31 default-size
scene trees plus the three 1280x720 variants named
above). No SCAN_SET addition or Integrator Action was
therefore owed by this maintenance pass.

Wave S maintenance (S16): S2 registered one new scene,
`subtitles-speaker`, so the counts the R17 note above
recorded have moved: the scan set now has 14 entries,
all required and still no pending entries; `scenes()`
lists 32 names, and the harness produces 35 tree dumps
(the 32 default-size scene trees plus the same three
1280x720 variants). IA-S2-1 was applied in full: both
headless.yml tree loops dump the new tree, the snap
step paints its full and stage-crop snaps for S2's
census leg, and the SCAN_SET entry is "required".

Wave T maintenance (T16): T12 registered one new
scene, `home-missing-audio`, so the counts the S16
note above recorded have moved again: the scan set
now has 15 entries, all required and still no
pending entries; `scenes()` lists 33 names, and the
harness produces 36 tree dumps (the 33 default-size
scene trees plus the same three 1280x720 variants).
IA-T12-1 was applied in full: both headless.yml tree
loops dump the new tree, the SCAN_SET entry is
"required", and headless.yml carries the marker-text
grep step this scan cannot make (the marker is
element text; the scan reads only path/rect/key).

Wave U maintenance (U12): U12 registered one new
scene, `home-scan-fallback`, so the counts the T16
note above recorded have moved again: the scan set
now has 16 entries, all required and still no
pending entries; `scenes()` lists 34 names, and the
harness produces 37 tree dumps (the 34 default-size
scene trees plus the same three 1280x720 variants).
IA-U12-1 was applied in full: both headless.yml tree
loops dump the new tree, the SCAN_SET entry is
"required", and headless.yml carries the
scan-fallback marker-text grep step beside the
missing-audio one -- the fallback line's proof is
its text (the marker plus the scan's defaulted
"0 frames" count, the discriminator against the
light-read line).

Wave V maintenance (V14): no new scene was
registered this wave (V6 builds subtitle-panel
controls; V7 adds snaps of existing scenes), so
the counts the U12 note above recorded stand:
the scan set still has 16 entries, all required
and still no pending entries; `scenes()` lists
34 names, and the harness produces 37 tree
dumps. One harness repair: the xvfb fallback
tree loop in headless.yml had drifted from the
plain loop -- it never named the Wave 7 light
pair (`default-light`, `settings-open-light`),
so a plain-attempt miss on either could never
be recovered by the fallback and its dump would
be absent without a word. The fallback list now
mirrors the plain list exactly. (The gate's
check 8 prints the per-loop split report-only
and names the plain loop the coverage contract;
that reading is unchanged -- the repair is to
the recovery set, not the contract.) The three
1280x720 size-variant probes remain plain-only,
as before: they are literal probes, not loop
scenes, and the scenes they pair with are
retried at the default size. V7 owns the snap
step's Home region this wave (snaps of
`home-populated` / `home-missing-audio` plus
the thumbnail probe's pixel gate, in
scripts/home_thumbnail_probe.py); the tree
loops this scan cross-checks already dump both
scenes, and V7's snap additions do not change
the produced tree set, so no SCAN_SET entry is
owed to V7.

Wave W maintenance (W14): no Wave W slice
registers a new scene (the wave wires progress
surfaces and hardens; its only headless-facing
slice is W7's probe-leg diagnosis), so the
counts the V14 note above recorded stand: the
scan set still has 16 entries, all required
and still no pending entries; `scenes()` lists
34 names, and the harness produces 37 tree
dumps. Both tree loops were re-verified
set-equal to the `scenes()` registry (and to
each other) at this pass, so V14's fallback-
mirror repair holds. The W7 boundary in
headless.yml this wave: W7 owns the Home
thumbnail probe step (and its snap capture
block for `home-missing-audio` in the snap
step); everything else in the workflow -- the
tree loops, the census and marker steps, the
live-MCP probe, upload and summary -- is this
pass's. W7's region reads tree dumps but
produces none, so no SCAN_SET entry is owed
to it. No IA from W1 or W6 naming workflow
wiring had been filed at this pass's final
check; any that lands later is the
integrator's to apply with this note's counts.

Wave X maintenance (X17): no Wave X slice
registers a new scene (the wave builds the
SpriteLoop GAP proposals -- states, bend-follow,
sprites, alpha export -- plus filed remainders;
none adds a `scenes()` entry), so the counts
the W14 note above recorded stand: the scan
set still has 16 entries, all required and
still no pending entries; `scenes()` lists
34 names, and the harness produces 37 tree
dumps. Both tree loops were re-verified
set-equal to the `scenes()` registry (and to
each other) at this pass, so V14's fallback-
mirror repair holds. The X10 boundary in
headless.yml this wave, drawn as W7/W14 drew
it: X10 owns the Home thumbnail probe step
(and its snap capture block for
`home-missing-audio` in the snap step) plus
the pair region of home_thumbnail_probe.py;
everything else in the workflow -- the tree
loops, the census and marker steps, the
live-MCP probe, upload and summary -- is this
pass's. X10's region reads tree dumps but
produces none, so no SCAN_SET entry is owed
to it. No IA from X3 or X10 naming workflow
wiring had been filed at this pass's final
check; any that lands later is the
integrator's to apply with this note's counts.

Wave Y maintenance (Y17): the audit at the
Wave X integration HEAD (d44098f) re-verified
the harness mechanics -- both tree loops parse
to 34 names, set-equal to the `scenes()`
registry and to each other, and the harness
produces 37 tree dumps -- and then audited
the Wave X surfaces for carriers, which X17's
pass could not do (X3/X5 had not yet filed).
Two additions, both REQUIRED from the start
on the S2/T12/U12 terms -- the trees were
already produced by both loops, and Y17
verified the surfaces in the Wave X closing
artifact (run 42784024319) before naming
them: `clips-populated`, the named-state
section's only carrier (clip-states-section
and its keying strip ride the populated clips
panel), and `sprites-populated`, the
dopesheet sprite lane's only carrier
(dope-sprite-row-* ride its two planted
instances). X5's SCAN_SET deferral to this
seat is discharged by the second addition.
The scan set now has 18 entries, all required
and still no pending entries; `scenes()`
still lists 34 names, and the harness still
produces 37 tree dumps. The alpha export
entries (X8) owe no entry: they are core /
MCP only, no panel surface exists (the panel
toggle is a flagged user decision, not
built), so no tree can carry them. One
coverage gap stands, filed as IA-Y17-2: no
scene plants a figure WITH state keys, so
the states section's state-row form (row +
clear button) is in no dump. A pending entry
was NOT added for it -- the pending side
stays empty by design (the self-test pins
it), and the S2 pattern names the tree
required in the same integration that lands
its scene.

The same audit's scan over that artifact is
NOT CLEAN, recorded plainly: 40 hits, one
family, in 20 of the 37 dumps -- X5's sprite
draw-order row. `sprite-z-front` (rect right
1426.0) escapes the right column's 1412.5
content edge by 14.0 pt and its own card's
right edge by 2.0 pt, and its unkeyed label
child (right 1416.0) by 4.0 pt; the row's
children sum wider than the 316 pt content
width, and Rune does not clip. The family
rides every dump whose right column mounts
the sprite panel's edit area, tree-default
.json among them -- so the required set's
clean contract fails on that dispatch.
Earlier waves' CLEAN verdicts stand (they
were measured on pre-X5 artifacts; the row
is new in Wave X), but the Wave X dispatch's
own CLEAN line was not artifact-measured --
Y15's audit reached the same 40-hit verdict
independently, and the scan is a debrief
instrument no CI step runs. No
exemption was added and no rule weakened:
the escape is the defect family this scan
exists for. The repair belongs to the panel
(src/rune_sec/sprites_panel.rs -- Wave Y's
Y14 seat) and is filed as IA-Y17-1; the two
entries above stay required either way,
since required governs presence, not
cleanliness.

The Y10 boundary in headless.yml this wave,
as X10 drew it against X17: Y10 owns the pair
region of scripts/home_thumbnail_probe.py
and the Home thumbnail probe step as invoked,
plus the snap step's `home-missing-audio`
capture block (per X17's statement of
record); everything else in the workflow --
the tree loops, the census and marker steps,
the live-MCP probe, upload and summary -- is
this pass's, and home_thumbnail_probe.py
itself is not touched by it. F7's live-MCP
probe repair (the step seeds the stored
live_mcp pref before launching) was verified
standing in the workflow at this pass, not
re-applied. No other Wave X finding names
workflow wiring in this region.

Checks considered and NOT added (they would
false-positive or judge what rects cannot show):

- Horizontal bounds on scrolled content (chips, clip
  spans, dope cells/headers): see rule 4 -- the right
  edge of scrolled content is scroll-dependent by
  definition, and so is the left edge once the
  viewport has scrolled.
- Cell pitch / alignment (a dope cell's x against
  frame x CHIP_PITCH): the pitch is a source constant
  (timeline_marks.rs); duplicating it here would
  create a second source of truth that can drift, and
  misalignment is an alignment defect, not a
  paint-past-bounds overflow -- the family this scan
  exists for. The in-process snap tests pin cell
  behaviour instead.
- Dope header width against its row slot: the slot
  width is not an element in the dump, so there is no
  rect to judge the header against; a too-wide header
  inside the scroll still paints within the content
  extent, and its failure mode is overlap, not
  overflow.
- A vertical window rule for all elements: the side
  columns scroll vertically, and below-the-fold
  content there legitimately draws past the window's
  bottom edge. Rule 4's vertical clause is sound only
  because the timeline card sits in no vertical
  scroll container.

Exemptions (closed list -- anything else is a hit):

- Card roots themselves (rule 1 only; see above).
- settings-toggle / settings-card: these two keys ARE the
  settings card (the collapsed strip and the expanded card
  root, nested one level under the `settings` wrapper), so
  their own right edge may reach W-16. Their descendants
  are NOT exempt -- the language chips inside
  settings-card are real hits (S19 §1 false-positive note).
- The timeline scroll subtree (any element whose path
  contains a `timeline` segment): it scrolls horizontally
  by design, so it is exempt from rules 1-3. Rule 4's
  vertical clause still applies inside it (see above).
  This is also the scan-time form of the general
  exemption "subtree rooted at an element whose
  builder sets scroll_x": at scan time the timeline is
  the only such subtree; a new one must be added here
  by key with a reason.
- Zero-size elements (w = 0 or h = 0: collapsed canvases,
  spacers).
- Vertical overflow in the side columns is never
  flagged: both side columns scroll vertically, so
  below-the-fold cards are legitimate. The one
  vertical judgement in the scan is rule 4's, inside
  the timeline card (see above).
"""

import json
import os
import re
import sys

TOL = 0.5
DEFAULT_WINDOW_W = 1440.0
DEFAULT_WINDOW_H = 900.0
# Left column: anchored left, fixed at every window width.
# Card x-range [16, 316]; content right edge 304.
LEFT_CONTENT_EDGE = 304.0
# Right column: anchored right. Card x-range [W-356, W-16];
# the content right edge sits 28 pt inside the window's
# right edge (1440 - 28 = 1412 at the reference width).
RIGHT_CONTENT_INSET = 28.0
COLUMN_SEGMENTS = {"side-left", "side-right"}

# Keys that ARE a card despite sitting one level below the
# column segment (see module docstring). Exempt from the
# content and card rules for the element itself only.
CARD_ALIAS_KEYS = {"settings-toggle", "settings-card"}

# Horizontal-scroll subtrees, by path segment (key). Closed
# list; see module docstring.
SCROLL_SEGMENTS = {"timeline"}

# The Wave Q scan set (see module docstring): the named
# proof set for the Wave P surfaces. Each entry is
# (tree dump basename, status, what the tree proves).
# "required" trees are produced by headless.yml today --
# a --dir pass that lacks one is INCOMPLETE, not clean.
# "pending" trees are named before the harness produces
# them; a --dir pass reports them PENDING without
# failing, and notes it when one shows up so the entry
# can be promoted. There are currently no pending
# entries: both Wave P subtitle trees were promoted after
# IA-Q12-1 landed in the workflow at Wave Q integration,
# and the speaker tree (Wave S) was added as required
# from the start -- IA-S2-1 landed its workflow dumps in
# the same integration. The Home missing-audio tree
# (Wave T) joined on the same terms -- IA-T12-1 landed
# its workflow dumps in the same integration. The Home
# scan-fallback tree (Wave U) joined on the same terms
# again -- IA-U12-1 landed its workflow dumps in the
# same integration. The clips-populated and sprites-populated
# trees (Wave Y) joined on the same terms again -- both were
# already produced by both loops, and Y17 verified the Wave X
# surfaces they carry in the Wave X closing artifact before
# naming them.
SCAN_SET = [
    ("tree-default.json", "required",
     "O-5 keyframe rows + O-6 wx-kind segments (P2); O-8 playback "
     "wraps + O-9 pose/camera rows (P3); Scene-panel bg-swatch "
     "state overlays, tree values (P4)"),
    ("tree-draw-open.json", "required",
     "O-4 draw cluster wraps (P2); stroke + text-tool empty "
     "states (Wave P audit)"),
    ("tree-graph-duplicates.json", "required",
     "O-5 graph rows (P2)"),
    ("tree-three-d.json", "required",
     "O-7 three_d_panel wraps (P3)"),
    ("tree-three-d-stage.json", "required",
     "O-7 wraps on the stage variant (P3)"),
    ("tree-fx-split-mid.json", "required",
     "O-8 two-line fx-nudge anatomy (P3)"),
    ("tree-loop-range.json", "required",
     "O-8 loop row in its set state (P3)"),
    ("tree-palette-open.json", "required",
     "command-palette result-row press overlays, tree values (P4)"),
    ("tree-fx-selected.json", "required",
     "fx clip-row press overlays, tree values (P4)"),
    ("tree-effects-open.json", "required",
     "P4's fx companion tree; O-6 weather segments + fx-presets "
     "empty state ride it (Wave P audit)"),
    ("tree-audio-open.json", "required",
     "Subtitles panel carrier (Audio workspace, P31/Q7); O-8 "
     "playback wraps ride it (Wave P audit)"),
    ("tree-subtitles-populated.json", "required",
     "subtitle lane in the timeline card (rule 4) with a cue "
     "live; dumped by headless.yml since IA-Q12-1 landed "
     "(Wave Q integration)"),
    ("tree-subtitles-top.json", "required",
     "same lane carrier under the Top style position (IA-Q12-1)"),
    ("tree-subtitles-speaker.json", "required",
     "same lane carrier with a credited cue (Wave S, IA-S2-1)"),
    ("tree-home-missing-audio.json", "required",
     "Home library open over a saved project whose audio file "
     "is missing — the row's meta line carries the missing-audio "
     "marker (Wave T, IA-T12-1)"),
    ("tree-home-scan-fallback.json", "required",
     "Home library open over a hand-trimmed project file (no "
     "frames key) whose audio file is missing — the row's light "
     "read refuses the file, so its meta line is the scan "
     "fallback carrying the missing-audio marker (Wave U, IA-U12-1)"),
    ("tree-clips-populated.json", "required",
     "clips panel populated: two-clip pool with loop-mode row and "
     "the Wave X named-state section (clip-states-section + keying "
     "strip, rules 1-2); the section's only carrier tree "
     "(Wave Y, Y17)"),
    ("tree-sprites-populated.json", "required",
     "Sprites panel populated rows + edit area and the Wave X "
     "dopesheet sprite lane (dope-sprite-row-*, rule 4) over two "
     "keyed instances; the lane's only carrier tree (Wave Y, Y17)"),
]


def required_trees():
    return [name for name, status, _ in SCAN_SET if status == "required"]


def pending_trees():
    return [name for name, status, _ in SCAN_SET if status == "pending"]


def set_gaps(present):
    """(missing required, absent pending) for a set of
    tree dump basenames found in a dumps directory."""
    missing = [n for n in required_trees() if n not in present]
    absent = [n for n in pending_trees() if n not in present]
    return missing, absent


def harness_tree_names():
    """Tree dump basenames .github/workflows/headless.yml
    produces, or None when the workflow is not readable
    (the scan also runs over downloaded artifacts, away
    from the checkout). Parsed from the workflow's --tree
    literals and its `for scene in ...` loops, not listed
    by hand, so the self-test's cross-check cannot drift
    from the workflow silently."""
    path = os.path.join(
        os.path.dirname(os.path.abspath(__file__)),
        "..", ".github", "workflows", "headless.yml",
    )
    try:
        with open(path) as fh:
            text = fh.read()
    except OSError:
        return None
    names = set()
    for m in re.finditer(r"--tree \"?headless-out/(tree-[a-z0-9-]+\.json)", text):
        names.add(m.group(1))
    for m in re.finditer(r"for scene in ([a-z0-9\- ]+); do", text):
        for scene in m.group(1).split():
            names.add("tree-%s.json" % scene)
    return names


def segments(path):
    return [s for s in path.split("/") if s != ""]


def column_index(segs):
    """Index of the side-column segment in segs, or None."""
    for i, s in enumerate(segs):
        if s in COLUMN_SEGMENTS:
            return i
    return None


def window_size(elements):
    """(width, height) of the window, from the tree root.

    The root is the element with the shallowest path (#0);
    its rect is the window. Each dimension falls back to
    the reference window (1440 x 900) when the root rect
    does not provide it, which reproduces the
    pre-generalisation width behaviour exactly.
    """
    root = None
    for el in elements:
        segs = segments(el.get("path", ""))
        if segs and (root is None or len(segs) < len(root[0])):
            root = (segs, el)
    win_w, win_h = DEFAULT_WINDOW_W, DEFAULT_WINDOW_H
    if root is not None:
        rect = root[1].get("rect") or [0, 0, 0, 0]
        if rect[2] > 0:
            win_w = float(rect[2])
        if len(rect) > 3 and rect[3] > 0:
            win_h = float(rect[3])
    return win_w, win_h


def window_width(elements):
    """Window width from the tree's root element."""
    return window_size(elements)[0]


def content_edges(win_w):
    """(left, right) content right-edges for window width W."""
    return LEFT_CONTENT_EDGE, win_w - RIGHT_CONTENT_INSET


def scan_tree(tree):
    elements = tree.get("elements", [])
    win_w, win_h = window_size(elements)
    left_edge, right_edge = content_edges(win_w)
    columns = {"side-left": left_edge, "side-right": right_edge}

    # Card roots: path -> (key, x0, x1). A card root is the
    # element whose path ends at the segment directly under
    # a side column.
    card_roots = {}
    for el in elements:
        segs = segments(el.get("path", ""))
        ci = column_index(segs)
        if ci is not None and len(segs) == ci + 2:
            rect = el.get("rect") or [0, 0, 0, 0]
            card_roots[el["path"]] = (segs[-1], rect[0], rect[0] + rect[2])

    # Timeline card (rule 4): the element keyed `timeline`
    # is the card's horizontal scroll row; the card itself
    # is deliberately unkeyed (Phase N), so the card root
    # is found structurally, as the scroll row's parent
    # element. Rect-derived, so width-agnostic like rule
    # 2. Absent on a zero-frame project (the card then
    # holds only the kit empty state) -- rule 4 stays
    # inert there.
    tl_row_path = None
    tl_card = None  # (path, x, y, right, bottom)
    for el in elements:
        if el.get("key") == "timeline":
            tl_row_path = el.get("path", "")
            break
    if tl_row_path and "/" in tl_row_path:
        card_path = tl_row_path.rsplit("/", 1)[0]
        for el in elements:
            if el.get("path") == card_path:
                rect = el.get("rect") or [0, 0, 0, 0]
                if rect[2] > 0 and rect[3] > 0:
                    tl_card = (
                        card_path,
                        rect[0],
                        rect[1],
                        rect[0] + rect[2],
                        rect[1] + rect[3],
                    )
                break

    hits = []
    for el in elements:
        epath = el.get("path", "")
        rect = el.get("rect") or [0, 0, 0, 0]
        x, y, w, h = rect
        right = x + w
        bottom = y + h
        key = el.get("key")
        segs = segments(epath)

        # Zero-size elements are silenced entirely
        # (collapsed canvases, spacers).
        if w <= 0 or h <= 0:
            continue  # zero-size
        # The horizontal-scroll subtree is exempt from
        # rules 1-3; rule 4's vertical clause still
        # applies inside it.
        in_scroll = any(s in SCROLL_SEGMENTS for s in segs)

        ci = column_index(segs)
        is_card_root = epath in card_roots
        parent_card = None
        card_range = None
        if ci is not None and len(segs) > ci + 1:
            root_path = "/".join(segs[: ci + 2])
            if root_path in card_roots:
                parent_card, cx0, cx1 = card_roots[root_path]
                card_range = (cx0, cx1)
            else:
                parent_card = segs[ci + 1]

        rules = []
        overflow = 0.0

        if not in_scroll:
            # Rule 1 -- content edge (not for card roots / aliases,
            # and not for the column element itself: len == ci + 1
            # means the element IS side-left/side-right, the
            # container, whose own right edge is 316 / W-16 by design).
            if (
                ci is not None
                and len(segs) > ci + 1
                and not is_card_root
                and key not in CARD_ALIAS_KEYS
            ):
                edge = columns[segs[ci]]
                if right > edge + TOL:
                    rules.append("content")
                    overflow = max(overflow, right - edge)

            # Rule 2 -- within the card root's own x-range.
            if card_range is not None and not is_card_root and key not in CARD_ALIAS_KEYS:
                cx0, cx1 = card_range
                excess = max(right - cx1, cx0 - x)
                if excess > TOL:
                    rules.append("card")
                    overflow = max(overflow, excess)

            # Rule 3 -- inside the window.
            excess = max(right - win_w, -x)
            if excess > TOL:
                rules.append("window")
                overflow = max(overflow, excess)

        # Rule 4 -- timeline card budget (see module
        # docstring). Applies to the card root and every
        # descendant; strict descendants of the scroll row
        # are judged vertically only.
        if tl_card is not None:
            cpath, cx, cy, cright, cbot = tl_card
            if epath == cpath:
                # The card root itself: vertical fit in
                # the window (rule 3 covers its horizontal
                # fit -- the card is not in the scroll
                # subtree).
                excess = max(-y, bottom - win_h)
                if excess > TOL:
                    rules.append("timeline-card")
                    overflow = max(overflow, excess)
            elif epath.startswith(cpath + "/"):
                excess = max(cy - y, bottom - cbot)
                if not epath.startswith(tl_row_path + "/"):
                    excess = max(excess, cx - x, right - cright)
                if excess > TOL:
                    rules.append("timeline-card")
                    overflow = max(overflow, excess)
                    if parent_card is None:
                        parent_card = "timeline-card"

        if rules:
            label = key if key else "(unkeyed %s)" % segs[-1]
            hits.append(
                {
                    "key": label,
                    "card": parent_card if parent_card else "-",
                    "overflow": overflow,
                    "rules": "+".join(rules),
                    "right": right,
                }
            )

    hits.sort(key=lambda hit: -hit["overflow"])
    return hits


def scan_file(path):
    with open(path) as fh:
        tree = json.load(fh)
    return scan_tree(tree)


# ------------------------------------------------------------------
# Self-test: synthetic fixtures proving (a) the generalised
# edges reduce EXACTLY to the original constants at W=1440,
# (b) a violation that only exists at 1280 is caught at 1280,
# (c) the timeline-card rule (rule 4) stays silent on a
# conforming card -- including scrolled content far past the
# window -- and catches each of its violation classes, and
# (d) the Wave 12 populated-scene fixtures (home-populated,
# strokes-populated) stay silent on conforming trees -- rows
# below the window's bottom edge included -- and catch one
# planted row escape per scene, (e) the Wave 13 fixtures
# (sprites-panel rows, library rows with 44 pt thumbnails
# and saved-row button lines) do the same for their shapes,
# and (f) the Wave P fixtures (layers two-line rows, the
# settings language grid under its alias card, the fx
# two-line nudge anatomy, the kit empty state) do the same
# for the surfaces Squads 1-2 changed, and (g) the Wave Q
# scan set: names unique and well-formed, the pending
# entries exactly none (both subtitle trees were promoted
# to required when IA-Q12-1 landed), the gap computation
# separating missing-required from absent-pending, and --
# when the checkout's headless.yml is readable -- every
# required tree harness-produced and no pending tree
# produced yet.
# ------------------------------------------------------------------

def _el(path, rect, key=None):
    el = {"path": path, "rect": rect}
    if key is not None:
        el["key"] = key
    return el


def _fixture(win_w, win_h):
    """A minimal two-column tree at window width win_w.

    Contains one conforming right-column chip ending exactly
    at the content edge, one right-column chip ending 18 pt
    past it (and 6 pt past its card), one left-column chip
    6 pt past the fixed left content edge, and one overlay
    crossing the window's right edge by 10 pt.
    """
    r_card_x = win_w - 356.0  # right card x0 (W-16-340)
    r_edge = win_w - 28.0     # right content right edge
    return {
        "elements": [
            _el("#0", [0.0, 0.0, win_w, win_h]),
            _el("#0/main-row", [16.0, 157.0, win_w - 32.0, 100.0]),
            # Left column (fixed geometry at every width).
            _el("#0/main-row/side-left", [16.0, 157.0, 300.0, 100.0]),
            _el("#0/main-row/side-left/card-l", [16.0, 157.0, 300.0, 100.0], "card-l"),
            _el("#0/main-row/side-left/card-l/left-chip",
                [100.0, 170.0, 210.0, 20.0], "left-chip"),  # right = 310
            # Right column (anchored to the right edge).
            _el("#0/main-row/side-right", [r_card_x, 157.0, 340.0, 100.0]),
            _el("#0/main-row/side-right/card-r", [r_card_x, 157.0, 340.0, 100.0], "card-r"),
            _el("#0/main-row/side-right/card-r/chip-ok",
                [r_card_x + 176.0, 170.0, 152.0, 20.0], "chip-ok"),  # right = edge
            _el("#0/main-row/side-right/card-r/chip-wide",
                [r_card_x + 176.0, 200.0, 170.0, 20.0], "chip-wide"),  # right = edge+18
            # Overlay crossing the window edge.
            _el("#0/overlay", [win_w - 80.0, 10.0, 90.0, 20.0], "overlay-wide"),  # right = W+10
        ]
    }


def _timeline_fixture(win_w, win_h, loop_dx=0.0, dope_dy=0.0,
                        timeline_dw=0.0, card_dy=0.0):
    """A minimal centre-column tree: stage + timeline card.

    Card geometry follows the real layout's anchors: the
    centre column starts at x=328 (16 margin + 300 left
    column + 12 gap) and its width ends 368 pt before the
    window's right edge (12 gap + 340 right column + 16
    margin), so card width = W-696 at any W; the card sits
    near the window's bottom. Inside the scroll row, a
    50-frame content column (1700 pt wide on the 34 pt
    frame pitch) with a far chip and a far dopesheet cell
    -- all far past the window's right edge, all
    legitimate scrolled content. The deltas plant one
    rule-4 violation each: loop_dx pushes the loop row's
    Clear button past the card's right edge, dope_dy drops
    a dopesheet row past the card's bottom, timeline_dw
    widens the scroll viewport itself past the card, and
    card_dy pushes the whole card past the window's
    bottom edge.
    """
    card_x = 328.0
    card_w = win_w - 696.0
    card_y = win_h - 201.0 + card_dy
    inner_x = card_x + 8.0
    inner_w = card_w - 16.0
    tl_y = card_y + 8.0
    lane_y = tl_y + 52.0
    base = "#0/main-row/centre/#3"
    return {
        "elements": [
            _el("#0", [0.0, 0.0, win_w, win_h]),
            _el("#0/main-row", [16.0, 141.0, win_w - 32.0, win_h - 157.0]),
            _el("#0/main-row/centre",
                [card_x, 141.0, card_w, win_h - 157.0], "centre"),
            _el("#0/main-row/centre/stage", [card_x, 157.0, card_w, 100.0], "stage"),
            # The (unkeyed) timeline card and its scroll row.
            _el(base, [card_x, card_y, card_w, 121.0]),
            _el(base + "/timeline",
                [inner_x, tl_y, inner_w + timeline_dw, 74.0], "timeline"),
            # Scrolled content: ruler/chips/lanes stack.
            _el(base + "/timeline/#0", [inner_x, tl_y, 1700.0, 74.0]),
            _el(base + "/timeline/#0/#1/frame-50",
                [inner_x + 49.0 * 34.0, tl_y + 26.0, 30.0, 24.0], "frame-50"),
            _el(base + "/timeline/#0/dopesheet-lane",
                [inner_x, lane_y, 1700.0, 18.0], "dopesheet-lane"),
            _el(base + "/timeline/#0/dopesheet-lane/dope-row-0",
                [inner_x, lane_y + dope_dy, 120.0, 18.0], "dope-row-0"),
            _el(base + "/timeline/#0/dopesheet-lane/dope-cell-0-49",
                [inner_x + 49.0 * 34.0, lane_y, 30.0, 18.0], "dope-cell-0-49"),
            # Non-scroll card content: the loop row.
            _el(base + "/timeline-loop-row",
                [inner_x, card_y + 86.0, inner_w, 27.0], "timeline-loop-row"),
            _el(base + "/timeline-loop-row/loop-clear",
                [inner_x + inner_w - 60.0 + loop_dx, card_y + 89.0, 60.0, 20.0],
                "loop-clear"),
        ]
    }


def _home_populated_fixture(win_w, win_h, meta_dx=0.0):
    """A minimal populated-Home tree (Wave 12 scene
    `home-populated`): the `home-panel` card root in the
    left column, open, holding the inner `home-card`
    column with two project rows and the totals footer.

    Geometry follows the real layout (the Wave 11 `home`
    dump + home_panel.rs): card x=16, width 300, with
    12 pt padding, so content is x=28, width 276, right
    edge 304; each row is a `home-row-{i}` list row
    (w_full) leading with the 56 pt thumbnail (16:9
    stage aspect, so 31.5 pt tall) beside a text column
    (truncated name, wrapped `home-proj-meta-{i}` line,
    wrapping button row). Row 1 and the totals footer
    sit below the window's bottom edge -- legitimate,
    because the left column scrolls vertically; the
    scan must stay silent on them. meta_dx shifts row
    1's metadata line right: the planted escape (past
    the 304 content edge and the 316 card edge).
    """
    base = "#0/main-row/side-left/home-panel"
    return {
        "elements": [
            _el("#0", [0.0, 0.0, win_w, win_h]),
            _el("#0/main-row", [16.0, 141.0, win_w - 32.0, win_h - 157.0]),
            _el("#0/main-row/side-left", [16.0, 157.0, 300.0, 1000.0]),
            _el(base, [16.0, 157.0, 300.0, 900.0], "home-panel"),
            _el(base + "/home", [28.0, 169.0, 276.0, 25.0], "home"),
            _el(base + "/home-card", [28.0, 202.0, 276.0, 843.0], "home-card"),
            # Row 0: thumbnail + text column, all inside
            # the content edge.
            _el(base + "/home-card/home-row-0",
                [28.0, 210.0, 276.0, 66.0], "home-row-0"),
            _el(base + "/home-card/home-row-0/#0",
                [36.0, 218.0, 56.0, 31.5]),  # the thumbnail
            _el(base + "/home-card/home-row-0/#1/home-proj-meta-0",
                [100.0, 226.0, 196.0, 15.0], "home-proj-meta-0"),
            _el(base + "/home-card/home-row-0/#1/home-open-0",
                [100.0, 245.0, 86.0, 25.0], "home-open-0"),
            # Row 1: below the window's bottom edge (the
            # column scrolls); its metadata line is the
            # planted escape when meta_dx > 0.
            _el(base + "/home-card/home-row-1",
                [28.0, 940.0, 276.0, 66.0], "home-row-1"),
            _el(base + "/home-card/home-row-1/#0",
                [36.0, 948.0, 56.0, 31.5]),  # the thumbnail
            _el(base + "/home-card/home-row-1/#1/home-proj-meta-1",
                [100.0 + meta_dx, 956.0, 196.0, 15.0], "home-proj-meta-1"),
            _el(base + "/home-card/home-totals",
                [28.0, 1014.0, 276.0, 15.0], "home-totals"),
        ]
    }


def _strokes_populated_fixture(win_w, win_h, del_dx=0.0):
    """A minimal populated stroke-inspector tree (Wave 12
    scene `strokes-populated`): the `stroke-inspector`
    card root in the right column with two per-stroke
    rows on the current frame.

    Geometry follows the real layout (the Wave 11
    draw-open dump + stroke_inspector.rs): card
    x=W-356, width 340, with 12 pt padding, so content
    is x=W-344, width 316, right edge W-28; each row is
    an unkeyed row shell holding a 14 pt swatch, the
    truncated summary text and the row's Delete button
    (`stroke-del-{idx}`), which ends exactly at the
    content edge. The card and its second row run
    below the window's bottom edge -- legitimate,
    because the right column scrolls vertically (the
    real draw-open card ends past y=1287 in a 900 pt
    window). del_dx shifts row 0's Delete button
    right: the planted escape, the Wave 9 clips-row
    failure mode in this card.
    """
    card_x = win_w - 356.0
    cx = card_x + 12.0   # content x (W-344)
    edge = win_w - 28.0  # content right edge
    base = "#0/main-row/side-right/stroke-inspector"
    return {
        "elements": [
            _el("#0", [0.0, 0.0, win_w, win_h]),
            _el("#0/main-row", [16.0, 141.0, win_w - 32.0, win_h - 157.0]),
            _el("#0/main-row/side-right", [card_x, 157.0, 340.0, 1100.0]),
            _el(base, [card_x, 157.0, 340.0, 1050.0], "stroke-inspector"),
            _el(base + "/strokes-header",
                [cx, 600.0, 316.0, 25.0], "strokes-header"),
            # Row 0 (unkeyed shell, as in the panel).
            _el(base + "/#7", [cx, 633.0, 316.0, 33.0]),
            _el(base + "/#7/#0", [cx + 8.0, 642.0, 14.0, 14.0]),   # swatch
            _el(base + "/#7/#1", [cx + 30.0, 642.0, 180.0, 15.0]),  # summary
            _el(base + "/#7/stroke-del-0",
                [edge - 64.0 + del_dx, 637.0, 64.0, 25.0], "stroke-del-0"),
            # Row 1: below the window's bottom edge.
            _el(base + "/#8", [cx, 1000.0, 316.0, 33.0]),
            _el(base + "/#8/stroke-del-1",
                [edge - 64.0, 1004.0, 64.0, 25.0], "stroke-del-1"),
        ]
    }


def _sprites_populated_fixture(win_w, win_h, step_dx=0.0):
    """A minimal sprites-panel tree (Wave 13 scene
    `sprites-populated`): the `sprite-panel` card root
    in the right column holding the instance list, the
    add picker and the selected instance's edit rows.

    Keys and anatomy follow A3's shipped source
    (`sprites_panel.rs`, which landed during this
    audit): `sprite-list` rows keyed `sprite-row-{i}`
    (a `sprite-select-{i}` button carrying the asset
    name, an unkeyed key-count text, a `sprite-del-{i}`
    Delete), an add column `sprite-add` with per-asset
    `sprite-add-{a}` buttons, and the `sprite-edit`
    column's stepper rows (`sprite-rot-dn` /
    `sprite-rot-up`, `sprite-scale-dn` /
    `sprite-scale-up`, ...). The panel's mount -- which
    column, which y -- is still in flight with E8/D1,
    so placement is modelled on the stroke inspector's
    card geometry: card x=W-356, width 340, 12 pt
    padding, content x=W-344, width 316, right edge
    W-28. Each row's Delete and each stepper's up
    button end exactly at the content edge. Row 2
    sits below the window's bottom edge -- legitimate,
    the right column scrolls vertically. step_dx
    shifts the scale stepper's up button right: the
    planted escape (a fixed-width stepper child pushed
    past the card, the populated-row failure mode in
    stepper form).
    """
    card_x = win_w - 356.0
    cx = card_x + 12.0   # content x (W-344)
    edge = win_w - 28.0  # content right edge
    base = "#0/main-row/side-right/sprite-panel"
    return {
        "elements": [
            _el("#0", [0.0, 0.0, win_w, win_h]),
            _el("#0/main-row", [16.0, 141.0, win_w - 32.0, win_h - 157.0]),
            _el("#0/main-row/side-right", [card_x, 157.0, 340.0, 1100.0]),
            _el(base, [card_x, 300.0, 340.0, 460.0], "sprite-panel"),
            # Add picker: one per-asset button.
            _el(base + "/sprite-add",
                [cx, 312.0, 316.0, 25.0], "sprite-add"),
            _el(base + "/sprite-add/sprite-add-0",
                [cx, 312.0, 132.0, 25.0], "sprite-add-0"),
            # Row 0: select button (asset name), key
            # count, Delete at the edge.
            _el(base + "/sprite-list/sprite-row-0",
                [cx, 345.0, 316.0, 33.0], "sprite-row-0"),
            _el(base + "/sprite-list/sprite-row-0/sprite-select-0",
                [cx + 8.0, 349.0, 150.0, 25.0], "sprite-select-0"),
            _el(base + "/sprite-list/sprite-row-0/#1",
                [cx + 166.0, 354.0, 52.0, 15.0]),  # "3 keys"
            _el(base + "/sprite-list/sprite-row-0/sprite-del-0",
                [edge - 64.0, 349.0, 64.0, 25.0], "sprite-del-0"),
            # Row 1 (selected) and its edit rows:
            # rotation conforming, scale carrying the
            # planted escape when step_dx > 0.
            _el(base + "/sprite-list/sprite-row-1",
                [cx, 386.0, 316.0, 33.0], "sprite-row-1"),
            _el(base + "/sprite-list/sprite-row-1/sprite-select-1",
                [cx + 8.0, 390.0, 150.0, 25.0], "sprite-select-1"),
            _el(base + "/sprite-list/sprite-row-1/sprite-del-1",
                [edge - 64.0, 390.0, 64.0, 25.0], "sprite-del-1"),
            _el(base + "/sprite-edit/sprite-rot-row",
                [cx, 427.0, 316.0, 25.0], "sprite-rot-row"),
            _el(base + "/sprite-edit/sprite-rot-row/sprite-rot-dn",
                [edge - 56.0, 427.0, 26.0, 25.0], "sprite-rot-dn"),
            _el(base + "/sprite-edit/sprite-rot-row/sprite-rot-up",
                [edge - 26.0, 427.0, 26.0, 25.0], "sprite-rot-up"),
            _el(base + "/sprite-edit/sprite-scale-row",
                [cx, 460.0, 316.0, 25.0], "sprite-scale-row"),
            _el(base + "/sprite-edit/sprite-scale-row/sprite-scale-dn",
                [edge - 56.0, 460.0, 26.0, 25.0], "sprite-scale-dn"),
            _el(base + "/sprite-edit/sprite-scale-row/sprite-scale-up",
                [edge - 26.0 + step_dx, 460.0, 26.0, 25.0], "sprite-scale-up"),
            # Row 2: below the window's bottom edge.
            _el(base + "/sprite-list/sprite-row-2",
                [cx, 950.0, 316.0, 33.0], "sprite-row-2"),
            _el(base + "/sprite-list/sprite-row-2/sprite-del-2",
                [edge - 64.0, 954.0, 64.0, 25.0], "sprite-del-2"),
        ]
    }


def _library_populated_fixture(win_w, win_h, del_dx=0.0):
    """A minimal populated-library tree (Wave 13 scene
    `library-populated`): the `library-panel` card root
    in the left column with a built-in row and a saved
    row inside the vertically scrolled `library-list`.

    Geometry is the real Wave 12 dump's (tree-default):
    card [16, 754, 300, 329], content x=28, width 276,
    right edge 304; `library-collapse` and `lib-sort`
    end exactly at 304; each row is 73 pt tall (a 44 pt
    thumbnail line over a 25 pt button line), the
    `library-thumb-{i}` canvas 44x44 at row x+8, the
    seg count ending at 296 (the row body's right
    edge). The saved row -- the entry kind the Wave 12
    dumps never carried -- adds Rename and Delete to
    the button line. Rows run below the list's 200 pt
    viewport and the window's bottom edge: legitimate
    (vertical scroll moves no x). del_dx shifts the
    saved row's Delete button right, modelling the
    fixed button line outgrowing its budget -- the
    Wave 10 escape returning to its home panel.
    """
    base = "#0/main-row/side-left/library-panel"
    row0 = base + "/library-card/library-list/library-builtins/library-row-0"
    saved = base + "/library-card/library-list/library-saved/library-row-3"
    return {
        "elements": [
            _el("#0", [0.0, 0.0, win_w, win_h]),
            _el("#0/main-row", [16.0, 141.0, win_w - 32.0, win_h - 157.0]),
            _el("#0/main-row/side-left", [16.0, 157.0, 300.0, 1000.0]),
            _el(base, [16.0, 754.0, 300.0, 329.0], "library-panel"),
            _el(base + "/library-header",
                [28.0, 766.0, 276.0, 25.0], "library-header"),
            _el(base + "/library-header/library-collapse",
                [257.5, 766.0, 46.5, 25.0], "library-collapse"),  # right = 304
            _el(base + "/library-card",
                [28.0, 808.0, 276.0, 263.0], "library-card"),
            _el(base + "/library-card/library-save",
                [28.0, 808.0, 276.0, 25.0], "library-save"),
            _el(base + "/library-card/library-search-row",
                [28.0, 839.0, 276.0, 26.0], "library-search-row"),
            _el(base + "/library-card/library-search-row/library-search",
                [49.0, 839.0, 141.0, 26.0], "library-search"),
            _el(base + "/library-card/library-search-row/lib-sort",
                [198.0, 840.0, 106.0, 25.0], "lib-sort"),  # right = 304
            _el(base + "/library-card/library-list",
                [28.0, 871.0, 276.0, 200.0], "library-list"),
            _el(base + "/library-card/library-list/library-builtins",
                [28.0, 871.0, 276.0, 245.0], "library-builtins"),
            # Built-in row 0: thumbnail line + Insert.
            _el(row0, [28.0, 890.0, 276.0, 73.0], "library-row-0"),
            _el(row0 + "/#0/#0/library-thumb-0",
                [36.0, 890.0, 44.0, 44.0], "library-thumb-0"),
            _el(row0 + "/#0/#0/#1/#0",
                [88.0, 904.0, 165.0, 15.0]),   # entry name
            _el(row0 + "/#0/#0/#1/#1",
                [261.5, 904.0, 34.5, 15.0]),   # "N seg", right = 296
            _el(row0 + "/#0/#1/library-insert-0",
                [36.0, 938.0, 70.0, 25.0], "library-insert-0"),
            # Saved row, below the fold: the full
            # Insert / Rename / Delete button line.
            _el(saved, [28.0, 1141.0, 276.0, 73.0], "library-row-3"),
            _el(saved + "/#0/#0/library-thumb-3",
                [36.0, 1141.0, 44.0, 44.0], "library-thumb-3"),
            _el(saved + "/#0/#0/#1/#1",
                [261.5, 1155.0, 34.5, 15.0]),   # "N seg", right = 296
            _el(saved + "/#0/#1/library-insert-3",
                [36.0, 1189.0, 70.0, 25.0], "library-insert-3"),
            _el(saved + "/#0/#1/library-rename-3",
                [114.0, 1189.0, 86.0, 25.0], "library-rename-3"),
            _el(saved + "/#0/#1/library-delete-3",
                [208.0 + del_dx, 1189.0, 62.0, 25.0], "library-delete-3"),
        ]
    }


def _layers_rows_fixture(win_w, win_h, inc_dx=0.0):
    """A minimal layers-panel tree (Wave P, p05 O-3):
    the `layers-panel` card root in the LEFT column
    holding two-line `layer-row-{fi}` rows.

    Anatomy follows the shipped two-line restructure
    (layers_panel.rs `layer_row`): line 1 is the
    colour dot (`layer-dot-{fi}`), the name and the
    opacity stepper (`layer-op-dec-{fi}` / readout /
    `layer-op-inc-{fi}`); line 2 is the reorder
    cluster (`layer-bottom-`, `layer-down-`,
    `layer-up-`, `layer-top-{fi}`) with the
    joint-count readout trailing. Card x=16, width
    300, 12 pt padding, so content is x=28, width
    276, right edge 304; the row's own px(8) puts its
    inner right edge at 296, where the opacity +
    button ends. Row 1 sits below the window's bottom
    edge -- legitimate, the left column scrolls
    vertically. inc_dx shifts row 0's opacity +
    button right: the planted escape (the pre-fix
    single line measured +134 in this panel).
    """
    base = "#0/main-row/side-left/layers-panel"
    return {
        "elements": [
            _el("#0", [0.0, 0.0, win_w, win_h]),
            _el("#0/main-row", [16.0, 141.0, win_w - 32.0, win_h - 157.0]),
            _el("#0/main-row/side-left", [16.0, 157.0, 300.0, 1000.0]),
            _el(base, [16.0, 500.0, 300.0, 400.0], "layers-panel"),
            # Row 0, line 1: dot, name, opacity stepper
            # ending at the row's inner edge (296).
            _el(base + "/layer-row-0", [28.0, 512.0, 276.0, 58.0], "layer-row-0"),
            _el(base + "/layer-row-0/#0/layer-dot-0",
                [36.0, 518.0, 14.0, 14.0], "layer-dot-0"),
            _el(base + "/layer-row-0/#0/#1",
                [58.0, 518.0, 140.0, 15.0]),  # the layer name
            _el(base + "/layer-row-0/#0/layer-op-dec-0",
                [206.0, 514.0, 26.0, 25.0], "layer-op-dec-0"),
            _el(base + "/layer-row-0/#0/#2",
                [238.0, 519.0, 28.0, 15.0]),  # the opacity readout
            _el(base + "/layer-row-0/#0/layer-op-inc-0",
                [270.0 + inc_dx, 514.0, 26.0, 25.0], "layer-op-inc-0"),
            # Row 0, line 2: the reorder cluster and the
            # joint-count readout.
            _el(base + "/layer-row-0/#1/layer-bottom-0",
                [36.0, 545.0, 26.0, 25.0], "layer-bottom-0"),
            _el(base + "/layer-row-0/#1/layer-down-0",
                [66.0, 545.0, 26.0, 25.0], "layer-down-0"),
            _el(base + "/layer-row-0/#1/layer-up-0",
                [96.0, 545.0, 26.0, 25.0], "layer-up-0"),
            _el(base + "/layer-row-0/#1/layer-top-0",
                [126.0, 545.0, 26.0, 25.0], "layer-top-0"),
            _el(base + "/layer-row-0/#1/#4",
                [160.0, 550.0, 60.0, 15.0]),  # "6 joints"
            # Row 1: below the window's bottom edge.
            _el(base + "/layer-row-1", [28.0, 950.0, 276.0, 58.0], "layer-row-1"),
            _el(base + "/layer-row-1/#0/layer-op-inc-1",
                [270.0, 964.0, 26.0, 25.0], "layer-op-inc-1"),
        ]
    }


def _settings_language_fixture(win_w, win_h, chip_dx=0.0):
    """A minimal settings-card tree (Wave P, p05 O-1):
    the `settings` wrapper in the RIGHT column (the
    scan's card root for this column slot), the
    `settings-card` alias inside it, and the language
    grid's wrapped chip rows.

    Geometry follows the shipped card: wrapper
    x=W-356, width 340; content x=W-344, width 316,
    right edge W-28. Each `settings-lang-row-{i}`
    wraps, so every chip (`settings-lang-{code}`)
    ends inside the content edge -- the widest
    single chip in the real grid is 181 pt, so no
    chip needs the full row. The alias exempts the
    settings-card ELEMENT only; the chips are
    ordinary descendants and stay judgable. chip_dx
    shifts one chip right: the planted escape, the
    pre-fix class in which `settings-lang-he` left
    the window entirely.
    """
    card_x = win_w - 356.0
    cx = card_x + 12.0   # content x (W-344)
    edge = win_w - 28.0  # content right edge
    base = "#0/main-row/side-right/settings"
    return {
        "elements": [
            _el("#0", [0.0, 0.0, win_w, win_h]),
            _el("#0/main-row", [16.0, 141.0, win_w - 32.0, win_h - 157.0]),
            _el("#0/main-row/side-right", [card_x, 157.0, 340.0, 1100.0]),
            _el(base, [card_x, 157.0, 340.0, 700.0], "settings"),
            _el(base + "/settings-card",
                [card_x, 190.0, 340.0, 660.0], "settings-card"),
            _el(base + "/settings-card/settings-lang-grid",
                [cx, 400.0, 316.0, 56.0], "settings-lang-grid"),
            # Wrapped row 0: two chips, inside the edge.
            _el(base + "/settings-card/settings-lang-grid/settings-lang-row-0",
                [cx, 400.0, 316.0, 25.0], "settings-lang-row-0"),
            _el(base + "/settings-card/settings-lang-grid/settings-lang-row-0/settings-lang-en",
                [cx, 400.0, 68.0, 25.0], "settings-lang-en"),
            _el(base + "/settings-card/settings-lang-grid/settings-lang-row-0/settings-lang-de",
                [cx + 74.0, 400.0, 68.0, 25.0], "settings-lang-de"),
            # Wrapped row 1: the widest chip (181 pt)
            # alone, ending inside the edge; the planted
            # escape when chip_dx > 0.
            _el(base + "/settings-card/settings-lang-grid/settings-lang-row-1",
                [cx, 431.0, 316.0, 25.0], "settings-lang-row-1"),
            _el(base + "/settings-card/settings-lang-grid/settings-lang-row-1/settings-lang-he",
                [edge - 181.0 + chip_dx, 431.0, 181.0, 25.0], "settings-lang-he"),
        ]
    }


def _fx_nudge_fixture(win_w, win_h, dur_dx=0.0):
    """A minimal fx-inspector tree (Wave P, p05 O-8):
    the `fx-section` card root in the RIGHT column,
    the `fx-inspector` column and the two-line
    `fx-nudge` anatomy the split produced.

    Anatomy follows fx_clips.rs `nudge_row`: line 1
    is the nudge cluster (`fx-nudge-l10`, `fx-nudge-l`,
    the readout, `fx-nudge-r`, `fx-nudge-r10`); line
    2 is the duration pair (`fx-dur-dn`, the readout,
    `fx-dur-up`). Card x=W-356, width 340, content
    x=W-344, width 316, right edge W-28; `fx-dur-up`
    ends exactly at the content edge -- pre-split it
    sat at the window edge (343.5 pt in a 316 pt
    budget). dur_dx shifts it right: the planted
    escape, the split regressing to one line.
    """
    card_x = win_w - 356.0
    cx = card_x + 12.0   # content x (W-344)
    edge = win_w - 28.0  # content right edge
    base = "#0/main-row/side-right/fx-section"
    return {
        "elements": [
            _el("#0", [0.0, 0.0, win_w, win_h]),
            _el("#0/main-row", [16.0, 141.0, win_w - 32.0, win_h - 157.0]),
            _el("#0/main-row/side-right", [card_x, 157.0, 340.0, 1100.0]),
            _el(base, [card_x, 300.0, 340.0, 400.0], "fx-section"),
            _el(base + "/fx-inspector", [cx, 340.0, 316.0, 200.0], "fx-inspector"),
            _el(base + "/fx-inspector/fx-nudge",
                [cx, 380.0, 316.0, 58.0], "fx-nudge"),
            # Line 1: the nudge cluster.
            _el(base + "/fx-inspector/fx-nudge/#0/fx-nudge-l10",
                [cx, 380.0, 44.0, 25.0], "fx-nudge-l10"),
            _el(base + "/fx-inspector/fx-nudge/#0/fx-nudge-l",
                [cx + 50.0, 380.0, 26.0, 25.0], "fx-nudge-l"),
            _el(base + "/fx-inspector/fx-nudge/#0/#2",
                [cx + 82.0, 385.0, 90.0, 15.0]),  # the start readout
            _el(base + "/fx-inspector/fx-nudge/#0/fx-nudge-r",
                [cx + 178.0, 380.0, 26.0, 25.0], "fx-nudge-r"),
            _el(base + "/fx-inspector/fx-nudge/#0/fx-nudge-r10",
                [cx + 210.0, 380.0, 44.0, 25.0], "fx-nudge-r10"),
            # Line 2: the duration pair, + ending
            # exactly at the content edge.
            _el(base + "/fx-inspector/fx-nudge/#1/#0",
                [cx, 413.0, 40.0, 15.0]),  # the "Dur" label
            _el(base + "/fx-inspector/fx-nudge/#1/fx-dur-dn",
                [cx + 120.0, 408.0, 52.0, 25.0], "fx-dur-dn"),
            _el(base + "/fx-inspector/fx-nudge/#1/#2",
                [cx + 178.0, 413.0, 60.0, 15.0]),  # the duration readout
            _el(base + "/fx-inspector/fx-nudge/#1/fx-dur-up",
                [edge - 52.0 + dur_dx, 408.0, 52.0, 25.0], "fx-dur-up"),
        ]
    }


def _empty_state_fixture(win_w, win_h, spill=False):
    """A minimal kit empty-state tree (Wave P, P7/P8):
    the `camera-panel` card root in the RIGHT column
    rendering design::empty_state for its empty key
    list -- the state the default tree carries.

    Anatomy follows design.rs `empty_state`: a
    w_full centred column with 12 pt of vertical air,
    the 20 pt glyph, and the message WRAPPED at the
    content width (two centred lines here). Card
    x=W-356, width 340, content x=W-344, width 316,
    right edge W-28. With spill=True the message is
    instead ONE unwrapped 442 pt line, centred, so
    it spills 63 pt past BOTH sides of the content
    box -- the historical defect this component's
    `.wrap().w_full()` exists to prevent (the clips
    panel's bare line, measured in the mission
    audits; see design.rs).
    """
    card_x = win_w - 356.0
    cx = card_x + 12.0   # content x (W-344)
    base = "#0/main-row/side-right/camera-panel"
    elements = [
        _el("#0", [0.0, 0.0, win_w, win_h]),
        _el("#0/main-row", [16.0, 141.0, win_w - 32.0, win_h - 157.0]),
        _el("#0/main-row/side-right", [card_x, 157.0, 340.0, 1100.0]),
        _el(base, [card_x, 157.0, 340.0, 300.0], "camera-panel"),
        _el(base + "/camera-header", [cx, 169.0, 316.0, 25.0], "camera-header"),
        # The empty-state column and its centred glyph.
        _el(base + "/#4", [cx, 202.0, 316.0, 76.0]),
        _el(base + "/#4/#0", [cx + 148.0, 214.0, 20.0, 20.0]),  # the glyph
    ]
    if spill:
        elements.append(
            _el(base + "/#4/empty-msg", [cx - 63.0, 242.0, 442.0, 15.0]))
    else:
        elements.append(
            _el(base + "/#4/#1", [cx + 38.0, 242.0, 240.0, 15.0]))  # line 1
        elements.append(
            _el(base + "/#4/#2", [cx + 58.0, 259.0, 200.0, 15.0]))  # line 2
    return {"elements": elements}


def self_test():
    failures = []

    def check(cond, label):
        print("  %s %s" % ("PASS" if cond else "FAIL", label))
        if not cond:
            failures.append(label)

    # The generalised edges at the reference width are the
    # original constants, bit for bit.
    check(content_edges(1440.0) == (304.0, 1412.0),
          "edges at W=1440 are exactly (304, 1412)")
    check(content_edges(1280.0) == (304.0, 1252.0),
          "edges at W=1280 are (304, 1252): left fixed, right anchored")

    for win_w, win_h in ((1440.0, 900.0), (1280.0, 720.0)):
        hits = scan_tree(_fixture(win_w, win_h))
        by_key = {h["key"]: h for h in hits}
        tag = "W=%d" % win_w
        check("chip-ok" not in by_key,
              "%s: chip ending exactly at the content edge is clean" % tag)
        check("chip-wide" in by_key and "content" in by_key["chip-wide"]["rules"],
              "%s: chip 18 pt past the content edge is caught (content)" % tag)
        check("chip-wide" in by_key and "card" in by_key["chip-wide"]["rules"],
              "%s: chip 6 pt past its card is caught (card)" % tag)
        check(abs(by_key.get("chip-wide", {}).get("overflow", 0) - 18.0) < 1e-9,
              "%s: chip-wide overflow measures 18.0 pt" % tag)
        check("left-chip" in by_key and "content" in by_key["left-chip"]["rules"],
              "%s: left chip past the fixed 304 edge is caught" % tag)
        check("overlay-wide" in by_key and "window" in by_key["overlay-wide"]["rules"],
              "%s: overlay crossing the window edge is caught (window)" % tag)
        check(len(hits) == 3, "%s: exactly 3 hits (got %d)" % (tag, len(hits)))

    # The point of the generalisation: at W=1280 the wide
    # chip ends at x=1270, far inside the OLD fixed window
    # (1440) and old right edge (1412) -- the old constants
    # would have passed this tree. Prove the miss is real
    # by re-running the fixture's chip against the old edge.
    chip_right = 1280.0 - 28.0 + 18.0
    check(chip_right < 1412.0 and chip_right < 1440.0,
          "the 1280 violation (right=1270) was invisible to the old 1440 rules")

    # A rootless tree falls back to the reference width.
    check(window_width([]) == 1440.0, "empty tree falls back to W=1440")
    check(window_size([]) == (1440.0, 900.0),
          "empty tree falls back to the 1440x900 reference window")

    # Rule 4: a conforming timeline card is silent at both
    # window sizes -- including its scrolled content, whose
    # right edges (past x=2000) no rule may judge.
    for win_w, win_h in ((1440.0, 900.0), (1280.0, 720.0)):
        hits = scan_tree(_timeline_fixture(win_w, win_h))
        by_key = {h["key"]: h for h in hits}
        tag = "timeline W=%d" % win_w
        check(len(hits) == 0,
              "%s: conforming card is clean (got %d hit(s))" % (tag, len(hits)))
        check("frame-50" not in by_key,
              "%s: a scrolled chip past the window edge is not flagged" % tag)
        check("dope-cell-0-49" not in by_key,
              "%s: a scrolled dope cell past the window edge is not flagged" % tag)

    # Rule 4 violations, one planted defect per run.
    hits = scan_tree(_timeline_fixture(1440.0, 900.0, loop_dx=18.0))
    by_key = {h["key"]: h for h in hits}
    check(len(hits) == 1 and "loop-clear" in by_key,
          "loop row button 10 pt past the card's right edge is caught")
    check("timeline-card" in by_key.get("loop-clear", {}).get("rules", ""),
          "loop row hit names the timeline-card rule")
    check(abs(by_key.get("loop-clear", {}).get("overflow", 0) - 10.0) < 1e-9,
          "loop row overflow measures 10.0 pt")
    check(by_key.get("loop-clear", {}).get("card") == "timeline-card",
          "loop row hit is attributed to the timeline card")

    hits = scan_tree(_timeline_fixture(1280.0, 720.0, loop_dx=18.0))
    by_key = {h["key"]: h for h in hits}
    check(len(hits) == 1
          and abs(by_key.get("loop-clear", {}).get("overflow", 0) - 10.0) < 1e-9,
          "the same loop row violation is caught at W=1280 (card is rect-derived)")

    hits = scan_tree(_timeline_fixture(1440.0, 900.0, dope_dy=52.0))
    by_key = {h["key"]: h for h in hits}
    check(len(hits) == 1 and "dope-row-0" in by_key,
          "dopesheet row 9 pt past the card's bottom edge is caught (vertical clause)")
    check("timeline-card" in by_key.get("dope-row-0", {}).get("rules", "")
          and abs(by_key.get("dope-row-0", {}).get("overflow", 0) - 9.0) < 1e-9,
          "dopesheet row overflow measures 9.0 pt")

    hits = scan_tree(_timeline_fixture(1440.0, 900.0, timeline_dw=24.0))
    by_key = {h["key"]: h for h in hits}
    check(len(hits) == 1 and "timeline" in by_key,
          "scroll viewport itself 16 pt wider than its card is caught")
    check(abs(by_key.get("timeline", {}).get("overflow", 0) - 16.0) < 1e-9,
          "scroll viewport overflow measures 16.0 pt")

    hits = scan_tree(_timeline_fixture(1440.0, 900.0, card_dy=100.0))
    by_key = {h["key"]: h for h in hits}
    check(len(hits) == 1 and "(unkeyed #3)" in by_key,
          "card root pushed 20 pt past the window's bottom edge is caught")
    check("timeline-card" in by_key.get("(unkeyed #3)", {}).get("rules", "")
          and abs(by_key.get("(unkeyed #3)", {}).get("overflow", 0) - 20.0) < 1e-9,
          "card root overflow measures 20.0 pt")

    # Wave 12: the populated scenes. Conforming trees
    # are silent at both window sizes -- including rows
    # below the window's bottom edge (the side columns
    # scroll vertically) and a Delete button ending
    # exactly at the content edge.
    for win_w, win_h in ((1440.0, 900.0), (1280.0, 720.0)):
        hits = scan_tree(_home_populated_fixture(win_w, win_h))
        check(len(hits) == 0,
              "home-populated W=%d: conforming populated Home is clean (got %d hit(s))"
              % (win_w, len(hits)))
        hits = scan_tree(_strokes_populated_fixture(win_w, win_h))
        check(len(hits) == 0,
              "strokes-populated W=%d: conforming populated inspector is clean (got %d hit(s))"
              % (win_w, len(hits)))

    # One planted row escape per scene: caught by the
    # content and card rules, with the measured overflow.
    hits = scan_tree(_home_populated_fixture(1440.0, 900.0, meta_dx=40.0))
    by_key = {h["key"]: h for h in hits}
    check(len(hits) == 1 and "home-proj-meta-1" in by_key,
          "home-populated: metadata line pushed past the card is caught")
    check("content" in by_key.get("home-proj-meta-1", {}).get("rules", "")
          and "card" in by_key.get("home-proj-meta-1", {}).get("rules", ""),
          "home-populated: the escape names the content and card rules")
    check(abs(by_key.get("home-proj-meta-1", {}).get("overflow", 0) - 32.0) < 1e-9,
          "home-populated: overflow measures 32.0 pt")

    hits = scan_tree(_strokes_populated_fixture(1440.0, 900.0, del_dx=18.0))
    by_key = {h["key"]: h for h in hits}
    check(len(hits) == 1 and "stroke-del-0" in by_key,
          "strokes-populated: Delete button pushed past the card is caught")
    check("content" in by_key.get("stroke-del-0", {}).get("rules", "")
          and "card" in by_key.get("stroke-del-0", {}).get("rules", ""),
          "strokes-populated: the escape names the content and card rules")
    check(abs(by_key.get("stroke-del-0", {}).get("overflow", 0) - 18.0) < 1e-9,
          "strokes-populated: overflow measures 18.0 pt")

    # Wave 13: the sprite surface and the saved library
    # rows. Conforming trees are silent at both window
    # sizes -- including the below-the-fold rows and the
    # edge-exact children (Delete and stepper buttons at
    # the content edge, lib-sort and the seg counts
    # inside theirs).
    for win_w, win_h in ((1440.0, 900.0), (1280.0, 720.0)):
        hits = scan_tree(_sprites_populated_fixture(win_w, win_h))
        check(len(hits) == 0,
              "sprites-populated W=%d: conforming sprites panel is clean (got %d hit(s))"
              % (win_w, len(hits)))
        hits = scan_tree(_library_populated_fixture(win_w, win_h))
        check(len(hits) == 0,
              "library-populated W=%d: conforming populated library is clean (got %d hit(s))"
              % (win_w, len(hits)))

    # One planted escape per shape: caught by the
    # content and card rules, with the measured overflow.
    hits = scan_tree(_sprites_populated_fixture(1440.0, 900.0, step_dx=24.0))
    by_key = {h["key"]: h for h in hits}
    check(len(hits) == 1 and "sprite-scale-up" in by_key,
          "sprites-populated: stepper up button pushed past the card is caught")
    check("content" in by_key.get("sprite-scale-up", {}).get("rules", "")
          and "card" in by_key.get("sprite-scale-up", {}).get("rules", ""),
          "sprites-populated: the escape names the content and card rules")
    check(abs(by_key.get("sprite-scale-up", {}).get("overflow", 0) - 24.0) < 1e-9,
          "sprites-populated: overflow measures 24.0 pt")

    hits = scan_tree(_library_populated_fixture(1440.0, 900.0, del_dx=50.0))
    by_key = {h["key"]: h for h in hits}
    check(len(hits) == 1 and "library-delete-3" in by_key,
          "library-populated: saved row's Delete pushed past the card is caught")
    check("content" in by_key.get("library-delete-3", {}).get("rules", "")
          and "card" in by_key.get("library-delete-3", {}).get("rules", ""),
          "library-populated: the escape names the content and card rules")
    check(abs(by_key.get("library-delete-3", {}).get("overflow", 0) - 16.0) < 1e-9,
          "library-populated: overflow measures 16.0 pt")

    # Wave P: the surfaces Squads 1-2 changed. Conforming
    # trees are silent at both window sizes -- including
    # the layers row below the window's bottom edge and
    # the edge-exact children (the opacity + button at
    # the layers row's inner edge, fx-dur-up at the
    # content edge, the settings card alias at W-16).
    for win_w, win_h in ((1440.0, 900.0), (1280.0, 720.0)):
        hits = scan_tree(_layers_rows_fixture(win_w, win_h))
        check(len(hits) == 0,
              "wave-p layers W=%d: conforming two-line rows are clean (got %d hit(s))"
              % (win_w, len(hits)))
        hits = scan_tree(_settings_language_fixture(win_w, win_h))
        check(len(hits) == 0,
              "wave-p settings-lang W=%d: conforming wrapped grid is clean (got %d hit(s))"
              % (win_w, len(hits)))
        hits = scan_tree(_fx_nudge_fixture(win_w, win_h))
        check(len(hits) == 0,
              "wave-p fx-nudge W=%d: conforming two-line nudge is clean (got %d hit(s))"
              % (win_w, len(hits)))
        hits = scan_tree(_empty_state_fixture(win_w, win_h))
        check(len(hits) == 0,
              "wave-p empty-state W=%d: conforming kit empty state is clean (got %d hit(s))"
              % (win_w, len(hits)))

    # One planted escape per shape, caught at the
    # measured overflow.
    hits = scan_tree(_layers_rows_fixture(1440.0, 900.0, inc_dx=30.0))
    by_key = {h["key"]: h for h in hits}
    check(len(hits) == 1 and "layer-op-inc-0" in by_key,
          "wave-p layers: opacity + button pushed past the card is caught")
    check("content" in by_key.get("layer-op-inc-0", {}).get("rules", "")
          and "card" in by_key.get("layer-op-inc-0", {}).get("rules", ""),
          "wave-p layers: the escape names the content and card rules")
    check(abs(by_key.get("layer-op-inc-0", {}).get("overflow", 0) - 22.0) < 1e-9,
          "wave-p layers: overflow measures 22.0 pt")

    hits = scan_tree(_settings_language_fixture(1440.0, 900.0, chip_dx=41.0))
    by_key = {h["key"]: h for h in hits}
    check(len(hits) == 1 and "settings-lang-he" in by_key,
          "wave-p settings-lang: chip pushed past the window is caught")
    check("content" in by_key.get("settings-lang-he", {}).get("rules", "")
          and "card" in by_key.get("settings-lang-he", {}).get("rules", "")
          and "window" in by_key.get("settings-lang-he", {}).get("rules", ""),
          "wave-p settings-lang: the escape names the content, card and window rules")
    check(abs(by_key.get("settings-lang-he", {}).get("overflow", 0) - 41.0) < 1e-9,
          "wave-p settings-lang: overflow measures 41.0 pt")

    hits = scan_tree(_fx_nudge_fixture(1440.0, 900.0, dur_dx=18.0))
    by_key = {h["key"]: h for h in hits}
    check(len(hits) == 1 and "fx-dur-up" in by_key,
          "wave-p fx-nudge: duration + button pushed past the card is caught")
    check("content" in by_key.get("fx-dur-up", {}).get("rules", "")
          and "card" in by_key.get("fx-dur-up", {}).get("rules", ""),
          "wave-p fx-nudge: the escape names the content and card rules")
    check(abs(by_key.get("fx-dur-up", {}).get("overflow", 0) - 18.0) < 1e-9,
          "wave-p fx-nudge: overflow measures 18.0 pt")

    hits = scan_tree(_empty_state_fixture(1440.0, 900.0, spill=True))
    by_key = {h["key"]: h for h in hits}
    check(len(hits) == 1 and "(unkeyed empty-msg)" in by_key,
          "wave-p empty-state: unwrapped 442 pt message is caught")
    check("content" in by_key.get("(unkeyed empty-msg)", {}).get("rules", "")
          and "card" in by_key.get("(unkeyed empty-msg)", {}).get("rules", ""),
          "wave-p empty-state: the spill names the content and card rules")
    check(abs(by_key.get("(unkeyed empty-msg)", {}).get("overflow", 0) - 63.0) < 1e-9,
          "wave-p empty-state: overflow measures 63.0 pt (the historical measurement)")

    # Wave Q: the named scan set (see SCAN_SET). The set
    # is only as good as its bookkeeping: a duplicated or
    # misnamed entry would let a debrief claim coverage
    # it does not have.
    names = [n for n, _, _ in SCAN_SET]
    check(len(names) == len(set(names)),
          "scan set: tree names are unique")
    check(all(n.startswith("tree-") and n.endswith(".json") for n in names),
          "scan set: every entry is a tree-*.json dump name")
    # IA-Q12-1 landed at Wave Q integration (2026-11-10):
    # headless.yml dumps both subtitle trees, so both
    # SCAN_SET entries were promoted to "required" and the
    # pending set is now empty by design.
    check(pending_trees() == [],
          "scan set: pending entries are exactly none (IA-Q12-1 promoted both subtitle trees)")
    missing, absent = set_gaps(set(names))
    check(missing == [] and absent == [],
          "scan set: a directory holding every entry has no gaps")
    missing, absent = set_gaps(set())
    check(missing == required_trees() and absent == pending_trees(),
          "scan set: an empty directory misses every required tree")
    missing, absent = set_gaps(set(required_trees()))
    check(missing == [] and absent == pending_trees(),
          "scan set: pending absence is not a required gap")

    # Cross-check against the harness itself when the
    # checkout is at hand: a required tree the workflow
    # never produces would fail every debrief forever,
    # and a "pending" tree the workflow already produces
    # is a promotion waiting to happen.
    produced = harness_tree_names()
    if produced is not None:
        check(all(n in produced for n in required_trees()),
              "scan set: every required tree is produced by headless.yml")
        check(all(n not in produced for n in pending_trees()),
              "scan set: pending trees are not yet produced by headless.yml")

    if failures:
        print("SELF-TEST: %d failure(s)" % len(failures))
        return 1
    print("SELF-TEST: all checks passed")
    return 0


def print_scan_set():
    for name, status, what in SCAN_SET:
        print("%-32s %-8s %s" % (name, status, what))
    return 0


def scan_dir(dump_dir):
    """Scan every tree-*.json in a dumps directory, then
    check the Wave Q scan set against what was found."""
    import glob

    files = sorted(glob.glob(os.path.join(dump_dir, "tree-*.json")))
    if not files:
        print("OVERFLOW SCAN: no tree-*.json dumps in %s" % dump_dir)
        return 2
    total = 0
    for path in files:
        hits = scan_file(path)
        total += len(hits)
        print("%s: %d hit(s)" % (path, len(hits)))
        for hit in hits:
            print(
                "  %-28s card=%-18s +%.1f pt  [%s] right=%.1f"
                % (hit["key"], hit["card"], hit["overflow"], hit["rules"], hit["right"])
            )
    present = {os.path.basename(p) for p in files}
    proves = {name: what for name, _, what in SCAN_SET}
    missing, absent_pending = set_gaps(present)
    for name in missing:
        print("MISSING (required) %s -- %s" % (name, proves[name]))
    for name in absent_pending:
        print("PENDING %s -- %s" % (name, proves[name]))
    for name in pending_trees():
        if name in present:
            print(
                "NOTE %s present -- promote its SCAN_SET entry to required"
                % name
            )
    if total:
        print("OVERFLOW SCAN: %d hit(s) -- NOT CLEAN" % total)
    if missing:
        print(
            "OVERFLOW SCAN: scan set INCOMPLETE (%d required tree(s) missing)"
            % len(missing)
        )
    if total or missing:
        return 1
    print("OVERFLOW SCAN: CLEAN (0 hits, scan set complete)")
    return 0


def main(argv):
    if len(argv) < 2:
        print(__doc__)
        return 2
    if argv[1] == "--self-test":
        return self_test()
    if argv[1] == "--scan-set":
        return print_scan_set()
    if argv[1] == "--dir":
        if len(argv) < 3:
            print(__doc__)
            return 2
        return scan_dir(argv[2])
    total = 0
    for path in argv[1:]:
        hits = scan_file(path)
        total += len(hits)
        print("%s: %d hit(s)" % (path, len(hits)))
        for hit in hits:
            print(
                "  %-28s card=%-18s +%.1f pt  [%s] right=%.1f"
                % (hit["key"], hit["card"], hit["overflow"], hit["rules"], hit["right"])
            )
    if total:
        print("OVERFLOW SCAN: %d hit(s) -- NOT CLEAN" % total)
        return 1
    print("OVERFLOW SCAN: CLEAN (0 hits)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
