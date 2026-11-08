# Squad 09 — Timeline Audit (ULTIMATE UI/UX Mission, Phase 1)

Scope: the timeline stack and its transport/playback
surfaces — `src/chrome_view.rs` (transport row, legacy
chips row), `src/rune_sec/timeline_marks.rs` (the live
timeline: ruler, chips, marker row, clips lane, loop
row, timeline card), `src/rune_sec/frame_ops.rs`
(Frames section), `src/rune_sec/playback_controls.rs`
(Playback panel). Presentation only: this audit
proposes **no change to frame-selection logic, drag
semantics, or any core op**. Items that would touch
the drag coordinate chain are flagged STOP for the
integrator rather than assigned.

Method: full source read of the four files' view
builders and module docs; cross-checks against
`src/main.rs` wiring (live timeline =
`timeline_marks::timeline_marks` under
`chrome_view::transport_row`, main.rs:747-748) and the
`loop-range` / `fx-split-mid` headless scenes. No local
builds (AGENTS.md); verification surfaces named per
item are CI/headless only.

## What is already good (do not redo)

- Ruler, chips and marker row share one geometry
  (`CHIP_W`/`CHIP_GAP`) inside one scrolled column, so
  they can never drift out of alignment with each
  other.
- Marker press targets are cell-sized (Phase N fix);
  the loop row sits outside the scroll viewport so its
  buttons never scroll away; the timecode readout and
  the Frames section's range readout (`range_label`)
  exist and are honest.
- Duration suffixes on chips (`3 ×2`), loop tinting
  and the selection outline are real, state-driven
  presentation — the bones are professional.

## Defects

### D1 — The clips lane does not scroll with the chips (P1, wrong information)

`timeline_marks.rs` module docs ("Clips lane") state
it plainly: the lane is content-width, drawn from
x = 0, and sits **outside** the scroll viewport. The
chips above it scroll. On any timeline longer than
the window, once the user scrolls right, every clip
bar sits under the **wrong frames** — the lane reads
as aligned and is not. A summary that silently
misaligns is worse than no summary.

### D2 — Overlapping clips hide each other on the single lane (P1)

One 10 pt lane (`CLIPS_H`), one row of bars:
`clip_spans` returns audio first, then FX, and
`clips_shape` paints in that order — an FX bar paints
**over** an audio bar on the same frames, and two
same-kind clips at the same frames fully overlap.
There is no stacking, offset, or overdraw warning.
Professional timelines give concurrent clips separate
lanes or an explicit overlap treatment.

### D3 — Loop controls live in three homes, with identical labels for different actions and duplicate element keys (P1, confusion)

- The timeline card's loop row (Set loop / Start − /
  Start + / End − / End + / Clear, keys `loop-set`,
  `loop-a-*`, `loop-b-*`, `loop-clear`).
- The Playback panel's loop-range row (keys
  `loop-start`, `loop-end`, `loop-clear` — **set**
  endpoint at playhead).
- The Frames section's loop endpoint buttons (keys
  `loop-start`, `loop-end` again — frame_ops.rs:747ff).
- Inside the Playback panel alone, "Loop start" /
  "Loop end" appear **twice with different meanings**:
  `pb-loop-start` / `pb-loop-end` *jump* to an endpoint;
  `loop-start` / `loop-end` *set* an endpoint.
- `loop-start`, `loop-end` and `loop-clear` keys are
  therefore mounted by two different panels in the
  same Animate workspace tree — duplicate keys, which
  makes tree-based QA (and any key-addressed lookup)
  ambiguous.

### D4 — Marker row: raw Unicode glyphs, one faint, no legend at the timeline (P2)

Camera `◆` (accent) and graph `●` (`t.text_secondary`)
are text glyphs, not the curated Font Awesome icon
system the rest of the shell uses; the graph dot in
secondary ink on the card ground is the faintest
signal in the timeline. The only legend ("◆ camera,
● graph") is a prose line inside the Help panel
(help_panel.rs:312) — nothing at the timeline itself
tells a new user what either glyph means.

### D5 — Chip labels can outgrow the fixed 30 pt chip (P2)

`chip_label` renders e.g. `12 ×0.25` (two-digit frame
+ longest suffix) in `type_::LABEL` inside a fixed
`CHIP_W` 30 pt chip, with no truncate and no smaller
suffix style. The label is wider than its chip on
exactly the frames (long holds on late frames) where
the information matters most.

### D6 — No playhead indicator on the ruler (P2)

The current frame is shown only by the chip's accent
fill. The ruler — the strip a user's eye naturally
tracks during playback — has no playhead tick, notch
or emphasised label for the current second, so during
playback the playhead must be followed as one moving
accent chip among loop-tinted neighbours.

### D7 — Loop edge grab zones are invisible (P2)

Loop edges are draggable within `EDGE_PX` (8 pt) of
the range's outer chip edges, but nothing on screen
marks those edges: the first/last loop chips look
like every other tinted chip. A real, useful gesture
is undiscoverable.

### D8 — Empty timeline renders a blank card (P3)

With zero frames the timeline card is returned
holding an empty scroll row alone — no empty state,
no hint. (The loop row and clips lane correctly
vanish; the card itself says nothing.) Mission §5G
requires useful empty states.

### D9 — Transport row has no position readout and silent wrap (P3)

`chrome_view::transport_row` is prev / play / next
plus add / duplicate / delete, icon-only. The frame
counter lives in the status strip and the timecode
in the Playback panel — neither is beside the
transport the user is pressing. Prev/Next **wrap**
at the ends (next on the last frame goes to frame 1)
with no visual distinction or cue.

### D10 — Frames section is an undifferentiated button wall (P3)

`frames_section` packs ~17 controls into one
flex-wrapped row (Inbetween, shifts, Reverse,
Reverse all, swaps, clipboards, loop endpoints,
duration steppers) mixing text buttons and icon
buttons (Reverse is text; Reverse all is an icon) —
one dialect break inside a single row, and grouping
only by wrap order. The range readout exists but is
buried in the wrap flow (frame_ops.rs:797).

### D11 — A dead second timeline implementation still ships (P3)

`chrome_view::timeline_row` (the original bare chips
row) has no live caller — main.rs mounts
`timeline_marks` — but it remains in the tree with
the same keys and near-identical geometry. Any
future chip fix can land in the wrong copy.

### D12 — Ruler has no tick hierarchy (P4)

Every non-second slot is the same 1×5 pt tick; at
60 fps there are 59 identical ticks between labels.
No 5-second (or 5-frame-group) major tick, so long
timelines have no intermediate landmarks.

### D13 — Selection outline on loop-tinted chips is low-contrast (P4)

A selected non-current chip gets an **accent**
outline over a fill that is already `t.card` mixed
0.35 toward that same accent — outline and fill are
neighbours in hue, so the selection read on
loop-range frames (the frames most often selected
for range ops) is the weakest in the row.

### D14 — Clip bars are identified by hue alone (P4)

Audio = primary accent `0x6D8CFF`, FX = secondary
`0xA184FF`: two blue-violet hues, similar lightness.
Kind is carried by colour only (unsafe for
colour-vision deficiency), bars carry no name or
kind mark, and a muted/hidden clip at 0.35 alpha can
read as a third "kind" rather than a state.

### D15 — Frame duration also has two homes (P4, inventory)

Playback panel `dur-dn`/`dur-up` (×0.5/×2) and the
Frames section's "This frame" row (×0.5/×2/Reset 1.0)
edit the same value. Noted for the integrator's
de-duplication decision; not assigned below.

## Work items (one coder each; presentation only)

- **T1 (D1)** — Move the clips lane inside the
  scrolled content column (below the marker row), so
  lane, ruler, chips and markers translate together.
  File: `timeline_marks.rs` (builder only — the lane
  has no handlers). ⚠ BEHAVIOUR-RISK FLAG: touches
  the card's element tree; the drag handlers' content
  coordinates are unaffected in principle (no padded
  element enters the scroll chain), but the
  integrator must re-run the timeline geometry tests
  + `fx-split-mid` tree check before accepting.
- **T2 (D2)** — Two sub-lanes inside the clips lane:
  audio on the upper half, FX on the lower half
  (kind-split rows within `clips_shape`/`CLIPS_H`
  growth is a layout-height change the stage
  absorbs). File: `timeline_marks.rs`. Acceptance:
  an audio+FX overlap scene shows both bars.
- **T3 (D3a)** — Relabel the Playback panel's jump
  buttons to "Go to start" / "Go to end" (keys
  `pb-loop-start`/`pb-loop-end` unchanged). File:
  `playback_controls.rs`. Copy only.
- **T4 (D3b)** — INTEGRATOR DECISION, not a coder
  item: resolve the duplicate `loop-start` /
  `loop-end` / `loop-clear` keys (Playback panel vs
  Frames section vs timeline row) and whether loop
  editing keeps two panel homes. Any rename moves
  the key census — decide once, record it, then one
  coder applies it.
- **T5 (D4)** — Add a one-line legend to the timeline
  card (in the loop row's empty state, or a muted
  caption under the marker row when any marker
  exists): "◆ camera key · ● graph key", and raise
  the graph glyph to `t.text`. File:
  `timeline_marks.rs`. No keys added (unkeyed text).
- **T6 (D5)** — Split the chip label: frame number in
  LABEL, duration suffix in MONO_CAPTION, so
  `12 ×0.25` fits the 30 pt chip. File:
  `timeline_marks.rs` (`chip_label` callers + tests
  updated). Proof: a scene/frame with duration 0.25
  at frame ≥ 10, pixel-read.
- **T7 (D6)** — Playhead mark on the ruler: the
  current frame's ruler slot gets an accent tick
  (and its second label in primary text). Builder
  only, `timeline_marks.rs` ruler loop. No handler
  change — the ruler drag is untouched.
- **T8 (D7)** — Loop edge caps: draw a 3 pt accent
  bar on the outer edge inside the first and last
  loop chips. Chip children only, `timeline_marks.rs`.
  ⚠ Chip content changes must not alter chip size or
  the row's drag element — caps are children of the
  chip element, not siblings.
- **T9 (D8)** — Empty state in the timeline card when
  `frame_count() == 0`: kit `empty_state` line
  ("No frames yet — add one to start animating.")
  inside the card in place of the empty scroll row's
  content. File: `timeline_marks.rs`. The "card is
  always returned" invariant stays.
- **T10 (D9)** — Read-only position readout in the
  transport row: `timecode_label`-format text
  (`Frame 3 / 24 · 0:07`) between Next and the
  spacer, unkeyed text element. File:
  `chrome_view.rs` + reuse of
  `playback_controls::timecode_label` (check its
  visibility; if private, move the pure fn — no
  logic change).
- **T11 (D10)** — Group the Frames section: three
  labelled clusters (Edit / Range / Duration) using
  the existing `section_label` + dividers, children
  and keys in the same order, wrap kept. File:
  `frame_ops.rs` builder only. ⚠ Order-sensitive
  Runtime tests must pass unchanged — keys and
  handlers byte-identical.
- **T12 (D11)** — INTEGRATOR DECISION: delete
  `chrome_view::timeline_row` (and note it in the
  phase doc) or mark it deprecated in its doc
  comment. No live caller exists; the census is
  built from live trees, so removal should be
  census-neutral — verify via the gate.
- **T13 (D12)** — Major ticks: every 5th second slot
  gets a taller (8 pt), primary-ink tick. Builder
  only, `timeline_marks.rs` (helper beside
  `ruler_label`).
- **T14 (D13)** — Selected outline ink on
  loop-tinted (non-current) chips: use `t.text`
  instead of accent. One branch in the chip builder,
  `timeline_marks.rs`. Pixel-proof in the
  `loop-range` scene plus a selection state.
- **T15 (D14)** — Kind mark for clip bars: FX bars
  drawn with a square-cap/shortened body vs audio's
  round capsule (shape-level difference in
  `clips_shape`), so kind survives greyscale. File:
  `timeline_marks.rs`. Coordinate with Squad 02/17
  before landing (design-system surface).

## Explicitly out of scope (per brief)

- Frame-selection logic, shift-click/drag selection
  semantics, loop edge drag maths, ruler seek maths
  — untouched. Items T1/T8 carry flags precisely
  because they stand near that machinery; if either
  cannot be done without moving a handler or a
  coordinate, it stops and returns to the integrator.
- No new controls, no fake buttons: every item above
  restyles, relabels, regroups or annotates existing
  live elements only.

## Verification surfaces (CI/headless only)

- Scenes: `loop-range` (loop tint, loop row, edges),
  `fx-split-mid` (lane position, clips stack), plus
  one new scene if T6/T9 land (duration suffix on a
  ≥10 frame; empty project).
- Gate: key census must stay 804 except for whatever
  T4's decision records.
- Pixel pass on the timeline card at 1440×900 after
  the wave, per mission §11 — source inspection alone
  is not visual proof.
