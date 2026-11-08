# Squad 12 Audit — Graph & Keyframe Editing

Phase 1 audit, ULTIMATE UI/UX MISSION. Read-only source
audit; no code changed. All widths measured or derived
as stated; nothing here is claimed visually verified —
proof surfaces are named per item for the V wave.

## Files inspected

| File | Role |
|---|---|
| `src/rune_sec/graph_panel.rs` (3,180 lines) | Channel selector chips + channel list, curve canvas, interp segmented control, tangent-preset chips, transform actions |
| `src/rune_sec/keyframe_tools.rs` (2,449 lines) | Per-joint key controls: key/delete/auto, clipboard, range ops, interp cycle, nudge, stamp, tangent presets |
| `src/rune_sec/easing_panel.rs` (570 lines) | Frame tween, project tween (+ curve picker), graph tangent presets — three groups |
| `src/graph_keys.rs` (1,813 lines, `src/` not `rune_sec/`) | Engine key ops library (copy/paste, duplicate/delete channel, range delete/fill, shift, per-key interp, `channel_stats`). No UI of its own; its op surface is complete and is **not** a defect source. The panel-facing gap is exposure/wiring, in the panels below. |

Supporting reads: `src/chrome_view.rs` (`btn` metrics:
px 10 ×2 + LABEL text, py 5, radius 6), `src/rune_sec/
design.rs` (kit), `src/rune_sec/workspaces.rs`
(`GRAPH_SECTIONS = [Graph, Easing, KeyframeTools,
Figures]` — all three panels below mount **together**
in the right column of the Graph workspace),
`src/main.rs` (graph/keyframe/easing all mount in the
right column, x 1084..1424; card content **1096..1412,
budget 316 pt**), Rune `el/scroll.rs` (scroll
semantics), p05 plan (headless run 42488647081 tree
measurements, reused as the measured evidence base).

---

## 1. Overflow verdict — the p05 claim, verified

p05 reported five rows in this squad's files. Source
verification splits them into **two different defects**
that p05's dump scan could not tell apart:

### 1a. `graph-actions` — p05's "+305, unreachable" is
### wrong as stated; the real defect is a hidden-action
### scroll strip (P1, not P0)

The row is real and wide: 8 `btn`s (Key angle, Auto
tangents, Mirror time, Scale ×2, Scale ÷2, Reduce
keys, Bake, Clear), gap 4 ×7 = 28; measured row width
**621.5 pt = +305.5 over the 316 budget** (p05, tree
dump). BUT the builder wraps it:
`.child(actions.scroll_x().w_full())`
(graph_panel.rs, end of `graph_panel`). Per Rune's
`scroll_x` docs the row is a horizontal viewport:
content lays out at natural width, the viewport takes
the card's 316, excess is clipped and scrollable. The
tree dump reports unclipped layout rects, so
`graph-clear` *measures* at x …1717.5 while actually
sitting behind the strip's right edge. It does **not**
paint past the window and is not in the draw-brushes
unreachable class.

The defect that remains is still serious:

- **Clear is invisible by default.** The strip shows
  ≈ the first 4 buttons (Key angle … Scale ×2 ≈ 278 pt
  of 621.5); Scale ÷2, Reduce keys, Bake and Clear are
  off-strip with **no affordance**: `scroll_edge_fade`
  is used nowhere in `src/` (grep: zero hits), and
  Rune's own docs state a mostly-vertical mouse-wheel
  swipe over a horizontal strip is delegated to the
  vertical viewport around it — a plain-wheel mouse
  user can only reach Clear via Shift+wheel or a
  trackpad. The hidden button is also the panel's most
  destructive one (§4).
- The channel selector chips (`graph-channels`) use
  the same pattern (`.scroll_x().w_full()`): as a
  figure's channels grow, the active chip can sit
  off-strip with no visual cue.

**Repair (S12-01 below):** delete the scroll strip,
`.flex_wrap()` the row — the p05/Phase M medicine.
Packing at measured widths (btn widths sum 593.5):
line 1 = Key angle + Auto tangents + Mirror time ≈
278, line 2 = Scale ×2 + Scale ÷2 + Reduce keys +
Clear ≈ 290 — **2 lines, all 8 actions visible**, no
key/label/handler changes.

### 1b. Four rows genuinely overflow — no scroll, no
### wrap, Rune does not clip (P0/P1)

| Row | File | Width | Over 316 | Ends at x | Class |
|---|---|---|---|---|---|
| `keyframe-presets` | keyframe_tools.rs | 361.0 | **+45.0** | **1457 — 17 pt past the window edge** | P0 |
| `graph-presets` | graph_panel.rs | 329.5 | +13.5 | 1425.5 — past the card edge (1424) | P1 |
| `keyframe-actions` | keyframe_tools.rs | 327.5 | +11.5 | 1423.5 — past content edge | P1 |
| `keyframe-clipboard` | keyframe_tools.rs | 318.8 | +2.8 | 1414.8 — past content edge | P1 |

Arithmetic: row width = last child's right edge −
1096 (p05 dump measurements; gaps: presets rows 4/6,
actions/clipboard 6). `graph-presets` chips are
content-sized columns (52 pt preview + px 4 ×2 →
≥60 each; "Ease in-out" label drives its chip to
≈74): 5 chips + 4 ×4 gap = 329.5. The other rows in
both panels fit: `graph-interp` is borderline (see
S12-06), easing picker strip is 4 ×72 + 3 ×6 =
**306 ≤ 316 ✓** (the source documents this fit),
easing preset rows ≈ 44 + 6 + btn ≤ ~180 ✓,
shift row ≈ 190 ✓, chan-list rows are `w_full` ✓.

Repairs: S12-02…S12-05, one coder each, `flex_wrap()`
first (Rune packs by measured width; keys verbatim),
two-line anatomy where a row has two natural clusters.

---

## 2. Defects — layout & structure

**D1. The Graph workspace's centrepiece lives in the
narrow column.** The curve canvas is `w_full × 170`
inside the 340 pt right column → **316 pt wide** for
editing curves that may span hundreds of frames
(`GraphView::auto_fit` compresses the whole channel
into 316 pt; `FALLBACK_SIZE` 720 is only a pre-layout
placeholder). The centre stage keeps showing the
animation stage in the Graph workspace. Cross-squad
item for Squads 03/05 (shell composition, `main.rs` —
outside S12 ownership): evaluate mounting the graph
panel in the centre column in the Graph workspace.
Recorded here, not sized for an S12 coder.

**D2. Nested unbounded scroll for the channel list.**
`chan_list` is a vertical `.scroll()` viewport with
`.min_h(0.0)` but **no bounded height**, inside a card
inside the column's vertical scroll. Rune's scroll
docs require a bounded height or the viewport grows
to content — so every added channel pushes the canvas,
interp row, presets and actions further down the
column, and the panel's working half sits below the
fold in a many-channel project. (S12-07.)

**D3. Three panels, one column, heavy repetition.**
Graph + Easing + Keyframes + Figures all stack in the
Graph workspace's right column — a very long scroll in
which the same operations recur (§3). Vertical cost
is dominated by duplication, not by unique controls.

---

## 3. Defects — duplication & scope (the squad's
## biggest UX problem after overflow)

**D4. Tangent presets exist three times, on screen
simultaneously.** All call
`EditorCore::apply_graph_ease_preset` (selected
figure's channels):

- `graph_panel` `graph-presets`: 5 chips with drawn
  curve previews (best presentation).
- `keyframe_tools` `keyframe-presets`: the same 5 as
  bare text buttons.
- `easing_panel` "Graph keys — selected figure": the
  same 5 again as preview+button rows.

Plus: **Auto tangents twice** (`graph-smooth`,
`key-auto` — same `smooth_graph_tangents`), **Key
angle twice** (`graph-key`, `key-angle` — same
`key_graph_angle`), **per-key interp twice in two
dialects** (graph panel's direct Hermite/Linear/Step
segmented control vs keyframe's `key-interp` Cycle
button whose label names the current mode). The
mission's "no confusing duplicate controls" criterion
fails in this workspace more than anywhere else
audited so far. Easing panel also duplicates **within
itself**: the project tween is set by the picker strip
AND by 4 preview+button rows directly beneath it —
8 controls, one setting.

**D5. One panel, three different scopes — and the
interp control does not edit what the panel draws.**
graph_panel's own module docs state it honestly:

- Canvas, drags, Mirror/Scale/Reduce: the **drawn
  channel** (chip/list selection).
- Preset chips: **all channels of the selected
  figure**.
- Interp segmented control: the key nearest the
  playhead on **`Angle(selected_node)`** — a channel
  that may not be the drawn one at all (draw Root X,
  the interp row still edits the selected joint's
  angle key; its active state is read from that key
  too). The row label names the true target
  ("Interp · angle {ni} key"), which mitigates but
  does not fix the incoherence: a control sitting
  directly under a curve does not affect that curve.

The fix exists in the engine already:
`graph_keys::set_key_interp(core, channel_index,
frame, interp)` + `key_interp_at` (used by MCP and
by keyframe's cycle op). (S12-08.)

**D6. Stats block duplicates the channel list.** The
list rows already show figure/target + key count;
`graph-stats` then repeats figure/target/count per
channel as plain text lines, adding only the value
range. Two stacked per-channel lists double the
pre-canvas height (see D2). Merging the range into
the list row's trailing slot would halve it — but
`graph-stat-{i}` keys exist, so this is a key-loss
decision for the mission lead, not a coder's call.
Recorded as decision item S12-09.

---

## 4. Defects — destructive actions

**D7. "Clear" does not say what it clears, and
nothing destructive wears danger styling.**
`graph-clear` calls `core.clear_graph()` — it clears
**every graph channel in the project** — from a
button labelled just "Clear", in the same plain `btn`
style as "Bake", scroll-hidden (§1a). Its neighbour
in the next panel, "Clear channel keys", clears one
channel. Near-identical labels, project-wide vs
single-channel blast radius. `graph-chan-delete`
(deletes the drawn channel) is likewise a plain
full-width `btn`. The kit/chrome family already has
`danger_btn` + `DANGER_FILL` (Phase N, ≈5:1) — none
of this squad's destructive controls use it, and none
ask for confirmation. Items S12-10 (danger styling,
no behaviour change) + a copy decision ("Clear" →
"Clear all channels" is a label change — mission
lead's call, listed in S12-10 as optional part B).

---

## 5. Defects — curve readability

**D8. The canvas draws almost no reference frame.**
`graph_shape` draws: a zero line (only when the
fitted range crosses 0), the playhead line, the curve
(2 pt accent), key diamonds (amber, r 4.5), the
selected key's tangent handles, a playhead diamond.
Missing: any horizontal gridlines, frame ticks or
ruler, value labels for fit min/max, a border (the
canvas fills `t.card` — the same fill as the panel
card it sits on; only its radius distinguishes it).
A user cannot read a value off the curve except via
the single `graph-value` playhead line. Also:
hand-set tangents can bend the curve outside the
fitted range, where the canvas clips it silently
(module docs own this). Items S12-11 (gridlines at
fitted quartiles + fit-range labels as canvas text is
not available — labels via the existing value line /
header readout extended with "range {min}…{max}"),
S12-12 (canvas border via kit hairline colour so the
viewport reads as a viewport in both themes).

**D9. Theme literals on the canvas.** Key diamonds
are a hard-coded amber `0xFFC65D` in both themes, and
the curve/playhead use `tok::accent(0, true)` — the
dark-variant lightness — regardless of theme (an
app-wide idiom, 23 sites; flagged here for Squad 02
to settle centrally, per the mission's no-scattered-
literals rule). `design::accent()` exists and is
theme-derived in the easing panel already. (S12-12
includes the graph canvas switch if Squad 02 confirms
the token; otherwise it stays a Squad 02 item.)

---

## 6. Defects — selection, tangents, key controls

**D10. Channel selection is two systems.**
Chips pick a *target* (selected figure only, incl.
keyless pinned roots); list rows pick a channel
*index* (all figures, duplicates individually —
Phase J). Both stay live, with subtle resolution
rules (`effective_channel_index`) that the UI never
explains; pressing a chip silently clears a row pick.
This is powerful but undiscoverable. No removal
proposed (both do real work); item S12-13 adds the
missing affordance: the header readout already names
the drawn channel — extend it to name the figure when
the drawn channel belongs to another figure than the
panel title's ("Graph — figure {selected+1}" title
vs a row-picked channel of figure 3 is today's
contradiction: the title follows `selected_figure`,
which a row press does update via `select_figure` —
verified consistent; the remaining gap is duplicate
twins: two rows with identical labels, distinguished
only by position). S12-13: append the channel index
to duplicate rows' labels (data-derived, keys
verbatim).

**D11. Tangent editing is drag-only and
selection is implicit.** The "selected key" is always
the key nearest the playhead; its handles are the
only ones drawn/hit-tested. There is no numeric
readout of the selected key's frame/value/in/out
slopes anywhere, no per-key preset, and no way to
inspect a key without moving the playhead (click-seek)
or grabbing it. A user cannot verify a tangent value
the way they can verify everything else in this app
(mono numeric readouts). Item S12-14: extend the
`graph-value` line's family with a selected-key line
(frame, value, in/out slopes, interp — from
`nearest_key` + the key's fields, read-only display,
new key `graph-key-readout`; display only, no engine
change).

**D12. Two disabled dialects in keyframe_tools.**
Icon controls go truly inert (handler-free
`*_inert` twins, decided by `*_enabled` predicates);
the plain `btn` controls (Key angle here, Delete key
here, Auto tangents, 5 presets) stay live in every
state and land on status-only refusals. Key angle
here *must* stay live (it creates the channel), but
Delete key here already computes `keyed` for its
highlight — the same predicate could drive
`btn_muted_live`. One disabled language is a mission
§5D requirement. Item S12-15 (apply the family's
muted-live/inert forms where a predicate already
exists; guards/handlers/statuses unchanged).

**D13. Empty states are inconsistent.** The channel
list's empty line is a proper kit `empty_state`
(chart-line glyph + how-to) ✓. But the per-channel
"No graph keys on this channel yet — select a joint
on the stage, then Key angle." is a bare muted text
line — the module docs record this as a deliberate
Phase N ruling ("per-channel guidance, not a list
empty state"); the mission §5G and Phase O charter
rule 7 both supersede that ruling. The easing panel's
Graph keys group has **no** empty state at all: with
no channels on the selected figure its 5 buttons look
fully operational and only write a status refusal.
Keyframe tools with no joint selected is handled
well (dedicated note row, Phase J) ✓. Item S12-16.

---

## 8. Coder items (one coder each; keys/handlers
## verbatim unless an item says otherwise)

**S12-01 — graph-actions: kill the scroll strip, wrap.**
`graph_panel.rs` (`graph_panel`, actions row).
Remove `.scroll_x()`, add `.flex_wrap()` to the row.
Arithmetic (§1a): 593.5 of buttons + 28 gap = 621.5 →
2 wrapped lines ≈ 278 / 290, both ≤ 316. Same for
`graph-channels` selector row: `.flex_wrap()` instead
of `.scroll_x()` so the active chip is always visible.
Proof: `graph-duplicates` + default tree dumps pass
the p05 §2 scan (zero hits, no scroll exemption
needed); snap read shows all 8 actions + Clear.

**S12-02 — graph-presets wrap.** `graph_panel.rs`.
`.flex_wrap()` on `graph-presets` (329.5 → line 1 ≈
Smooth+Ease in+Ease out+Ease in-out ≈ 270, Linear
wraps). Do NOT shrink PREVIEW_W (52) — the previews
are the content. Proof: tree scan zero; snap read.

**S12-03 — keyframe-presets wrap.**
`keyframe_tools.rs`. `.flex_wrap()` on
`keyframe-presets` (361 → ≈280 + Linear). This row is
also a D4 duplicate — if S12-09's decision removes
it, this item is moot; wrap first (cheap, safe),
decide after. Proof: tree scan zero (row no longer
crosses x 1440).

**S12-04 — keyframe-actions two-line anatomy.**
`keyframe_tools.rs`. Split `ACTIONS_ROW_KEY` row:
line 1 = Key angle here + Delete key here
(≈110 + 6 + 118 = 234 ✓), line 2 = Auto tangents
(≈104 ✓). (flex_wrap would strand Auto tangents
alone anyway; the explicit split keeps the primary
pair together.) Keys verbatim. Proof: tree scan zero.

**S12-05 — keyframe-clipboard: Clear gets its own
row.** `keyframe_tools.rs`. Move
`key-clear-channel` out of `keyframe-clipboard` into
a solo row directly above `keyframe-clear-range`
(matching that row's and `keyframe-fill-range`'s
existing one-control pattern). Clipboard row becomes
Copy + Paste ≈ 76 + 6 + 80 = 162 ✓✓; the +2.8
overflow dies with margin to spare at any font
metric (a gap 6→4 squeeze would fit by only 1.2 pt —
rejected as fragile). Keys/handlers/guards verbatim.
Proof: tree scan zero.

**S12-06 — measure `graph-interp`, then fix if real.**
`graph_panel.rs`. The interp row (section_label +
Hermite/Linear/Step btns ≈ 59/53/42 + gaps 12) totals
≈ 294 with "Interp · angle 9 key" but ≈ 309–320 with
"Interp · no joint selected" or a two-digit node —
borderline vs 316, and the label text is state-
dependent. Coder: read the row's rect from the
default + graph-duplicates dumps; if any state
crosses 1412.5, shorten the label to "Interp" +
keep the target in the header readout (a listed copy
change) or wrap the row. Proof: dump measurement in
the PR description + scan zero in all states.

**S12-07 — bound the channel list height.**
`graph_panel.rs`. Give `chan_list`'s scroll viewport
a bounded height (`.max_h()` ≈ 5 rows ≈ 130 pt, kit
spacing) so the canvas stays in view as channels
grow; rows remain reachable by the list's own
scroll. Coordinate with S12-09 (if stats merge into
rows, row height changes — do S12-09's decision
first). Proof: a planted many-channel scene (extend
`graph-duplicates` pattern) — canvas rect y-position
identical with 2 and 12 channels.

**S12-08 — interp control edits the drawn channel.**
`graph_panel.rs`. Rewire `graph-interp-*` (keys,
labels verbatim) to `graph_keys::set_key_interp` on
the drawn channel's key nearest the playhead, active
state from `graph_keys::key_interp_at`; fall back to
today's `set_graph_key_interp` path when no channel
is drawn. Relabel the row with the drawn channel's
label (listed copy change: the label must stop
claiming "angle {ni}" when it edits Root X). Tests:
extend the panel's existing `current_interp` test
family for a Root X drawn channel. Proof: unit tests
+ graph-duplicates snap (interp highlight follows
the drawn channel).

**S12-09 — DECISION (mission lead): duplication.**
Decide the canonical surface for (a) tangent presets
— recommendation: graph_panel chips (only surface
with previews + drawn-channel context); remove
`keyframe-presets` row and easing's Graph keys group
OR keep them as deliberate mirrors with identical
preview-chip presentation; (b) the project-tween
picker strip vs its 4 duplicate rows in easing_panel
— recommendation: picker is canonical; (c) stats
block merge into list rows (D6). Every removal is a
key-loss exception requiring explicit approval per
mission §2 rule 8; this item produces the decision
record, not code.

**S12-10 — danger styling for destructive graph
controls.** `graph_panel.rs` + `keyframe_tools.rs`.
`graph-clear`, `graph-chan-delete`,
`key-clear-channel`, `key-clear-range` switch from
`btn`/`icon_btn` to the chrome family's `danger_btn`
/ `danger_icon_btn` (Phase N reds, already in the
family). Part B (optional, needs copy approval):
`graph-clear` label → "Clear all channels".
Handlers/guards verbatim. Proof: snap read — the
four controls read red; gate census unchanged
(part A).

**S12-11 — canvas reference frame.** `graph_panel.rs`
(`graph_shape`). Add horizontal gridlines at the
fitted range's quartiles (t.text at alpha ≈ 0.08)
and extend the header readout with the fitted range
("range {min:.0}…{max:.0}" — data from `view_for`'s
existing fit; listed copy addition, no new key).
No canvas text (Rune canvas has none here) — the
readout carries the numbers. Proof: snap read in
dark + light; existing graph tests green.

**S12-12 — canvas border + theme-correct curve
ink.** `graph_panel.rs`. Canvas gets a hairline
border in the kit's `card_border` so the viewport
reads as a surface on the card; curve/playhead ink
switches from `tok::accent(0, true)` to
`design::accent()` IF Squad 02 confirms the token
centrally (else this half defers to Squad 02). The
amber key diamonds stay (a deliberate, documented
highlight colour) but move to a named const with a
comment. Proof: snap read both themes.

**S12-13 — duplicate channel rows distinguishable.**
`graph_panel.rs` (`channel_rows`). When two rows
share figure+target, suffix the label with the
channel index ("Root X · #2"). Data-derived from
`channel_rows`' existing output; keys verbatim.
Proof: existing duplicate tests extended;
graph-duplicates snap shows two distinct labels.

**S12-14 — selected-key readout line.**
`graph_panel.rs`. New muted line under `graph-value`,
key `graph-key-readout`: "Key at frame {f}: {v:.1} ·
in {in_tan:.2} / out {out_tan:.2} · {interp}" for
the `nearest_key` of the drawn channel; absent when
no key/channel. Read-only display; mono numerals per
kit discipline. Proof: unit tests beside the
`value_line` test family.

**S12-15 — one disabled dialect in keyframe_tools.**
`keyframe_tools.rs`. Delete key here renders via
`btn_muted_live` gated on the existing `keyed`
predicate (its refusal status is already written by
`delete_key_here_core` — the muted-live contract);
audit Auto tangents the same way (live if no refusal
status exists to answer with — document the outcome
in the file header either way). Proof: existing
keyframe tests green; snap read both states.

**S12-16 — empty states completed.** `graph_panel.rs`
+ `easing_panel.rs`. (a) The per-channel "No graph
keys…" line becomes `design::empty_state` with the
`chart-line` glyph (same glyph as the list's empty
state; module-doc ruling updated to cite the mission
charter). (b) Easing's Graph keys group: when the
selected figure has no channels, the 5 rows are
replaced by one `empty_state` line ("No graph
channels on this figure yet — Key angle starts
one."), mirroring graph_panel's verified wording.
Proof: default-scene snap (fresh project has no
channels) shows both states as kit empty states.

---

## 9. Summary for the integrator

- **Overflow verdict:** p05's graph numbers are
  arithmetically right but mechanically mixed.
  `graph-actions` (+305.5) is a **scroll strip**, not
  an off-window row — p05's "unreachable" does not
  hold; the true defect is a hidden, unaffordanced,
  destructive action (S12-01). The squad's real
  off-window row is **`keyframe-presets` (+45, ends
  x 1457)** — P0. Three more rows cross their card
  (graph-presets +13.5, keyframe-actions +11.5,
  keyframe-clipboard +2.8). All repairs are one-coder
  items with keys verbatim; census stays 804 unless
  S12-09's decisions are approved.
- **Top defects after overflow:** (1) tangent presets
  ×3 + auto/key/interp ×2 duplication, all visible at
  once in one workspace (D4); (2) the interp control
  edits a different channel than the one drawn (D5 —
  fixable today via `graph_keys::set_key_interp`,
  S12-08); (3) project-wide "Clear" in plain styling,
  scroll-hidden (D7); (4) unbounded channel list
  pushing the canvas away (D2); (5) a curve canvas
  with no grid, no range labels and a same-fill
  boundary (D8).
- **Out-of-scope flags:** graph canvas in the centre
  column (D1 — Squads 03/05); `accent(0, true)` idiom
  (D9 — Squad 02); `src/graph_keys.rs` itself is
  sound — no items.
- **Item count: 16** (15 coder items + 1 decision
  item). Files touched: graph_panel.rs (9 items),
  keyframe_tools.rs (5), easing_panel.rs (2, one
  shared with S12-09).