# Squad 07 Audit — Character & Figure Management

**Scope:** `figures_panel.rs`, `figure_add.rs`,
`inspector_figure.rs`, `layers_panel.rs`, `pose_tools.rs`
(all `src/rune_sec/`). Left column budget **276 pt**
content (card 300, x 28..304); right column budget
**316 pt** (card 340, x 1096..1412); window 1440×900.

**Method / evidence.** Source read of all five files
plus the kit (`design.rs`, `icon_btn.rs`, `chrome_view.rs`),
the mount order in `main.rs` (left: figures → figure-add
→ layers; right: inspector-figure → pose-tools), the
workspace gating in `workspaces.rs` (Animate shows
Figures + Layers + InspectorFigure + PoseTools at once),
and **measured rects from the Phase N final headless tree
dump** (run 42488647081, `tree-default.json`) — widths
below are measured, not estimated, wherever a rect is
quoted. Cross-reference: the Phase O plan
`docs/phase_o_plans/p05-overflow-widths.md` (items O-3
and O-9 already own two rows below; this audit adopts
their treatments and does not re-plan them).

**Standing constraint honoured:** the Phase N figures
fix (Recolour/Duplicate on their own header-actions row,
measured in-card at 91.6/91.9 pt) is correct and is not
touched by any item here. No item re-crowds the header.

---

## What is already good (do not rebuild)

- Figures rows: live rig thumbnails, kit list-row
  anatomy, filter with kit empty state, bulk footer —
  Phase N quality throughout.
- Pose tools: the app's best gating discipline — ops
  that would no-op render **inert** (muted, no handler),
  never fake. (S07-5 asks the figure/layer rows to learn
  this panel's own habit.)
- Pose "Copy pose to" row uses `scroll_x` — the one row
  in scope that handles overflow by design.
- Inspector: charter-order steppers, kit swatches
  (234 pt of 316 — fits), danger button for Delete,
  honest measured readouts.
- Figure-add: kit empty state for an empty library;
  dismiss-on-pick; bounds-checked library dispatch.

---

## Defects

### S07-1 — HIGH — Figure names are crushed to ≈28 pt in the figures list

Measured (`fig-row-0`, inner width 260): thumbnail 44 +
opacity readout ≈32 + lock 26 + solo chip 36 + steppers
20.5 + 20.5 + six gaps ×6 = **215 pt of fixed controls**,
leaving the dot+name cluster (`fig-sel-0`) **44.5 pt** —
dot 10 + gap 6 → **≈28 pt for the name text itself**
(~4 characters at LABEL size). Every real name ("Hero",
"Woman", any duplicate's "… copy") is clipped in the
panel whose entire job is identifying figures. The same
crowding makes each row **80 pt tall** (44 thumb on
line 1 + 26 line 2 + gaps/padding), so a five-figure
cast costs ≈440 pt of column and pushes Layers below
the fold.

**Fix item (1 coder, `figures_panel.rs` only, all keys
verbatim):** move the two opacity steppers
(`fig-op-dn-{fi}` / `fig-op-up-{fi}`) from line 1 to
line 2, and restructure the row root from a `col` to a
`row` whose thumbnail spans both lines (col of the two
control lines beside it).

- Line 1 after: fixed = 215 − 41 (steppers) − 12 (two
  gaps) = 162 → name cluster = 260 − 162 = **98 pt** →
  name text ≈ **82 pt** (~13 characters). ✔
- Line 2 after: current cluster ends at 296 having used
  169 pt (up/dn/opac/eye/del + gaps); adding the two
  steppers + 2 gaps = +53 → **222 ≤ 260**. ✔
- Row height after: max(44, 26+2+26) + 8 padding =
  **62 pt** (from 80, −22%). ✔
- Selection, hover, handlers, status strings unchanged.
  Proof: headless default tree — `fig-sel-0` ≥ 90 pt,
  row height ≤ 64; pixel read of the figures card.

### S07-2 — HIGH — Layers row overflows the card by ≥134 pt; three controls off-card

Measured (`layer-row-0`): `layer-bottom-0` already ends
at 304.25 (the content edge); `layer-down-0` spans
312..338, `layer-up-0` 346..372, `layer-top-0` 380..438,
and the joint-count text runs to ≈466 — all painted past
the card (edge 316) into the centre gutter/stage area.
Reorder-up and Top are effectively unusable where they
sit. **This is P05 item O-3** — this audit adopts its
treatment (two-line anatomy: dot/name/steppers on
line 1; bottom/down/up/top cluster on line 2, keys
verbatim) and adds S07-5b/S07-6 below, which must ride
the same restructure. Budget check for the O-3 line 2:
Bottom 77 + Down 26 + Top 58 + Up 26 + 3 gaps ×6 = 205;
with the S07-4 lock toggle (+26 + 6 gap) = **237 ≤ 260**. ✔

### S07-3 — HIGH — Duplicate element key `fig-duplicate` mounted twice in the Animate workspace

`figures_panel.rs` (header actions row, `icon_btn`) and
`inspector_figure.rs:924` (actions row, plain `btn`)
both emit key **`fig-duplicate`**, and `workspaces.rs`
shows both panels simultaneously in Animate (and the
other workspaces that include both sections). Headless
`--tree` lookups and any key-addressed state assume
unique keys — the figure-add tests state this outright.
The two buttons also duplicate each other functionally
(same op, same selection), one with a glyph, one
without — two visual dialects for one action.

**Fix item (1 coder, `inspector_figure.rs`):** rekey
the inspector's button to **`insp-fig-duplicate`** — the
inspector's own convention already prefixes its steppers
`insp-fop-dn/up`; `fig-duplicate`/`fig-del` there are
GPUI-legacy roots. This is a key *rename*, not a loss:
the gate census goes 804 → 805 distinct keys and the
integrator records the rename in PHASE docs. Handler,
label and op unchanged. (The figures-panel
`fig-duplicate` keeps the root key — it was the Phase M
original.) Deliberately *not* proposed: removing either
button — Duplicate in both the list header and the
inspector matches how opacity steppers legitimately
appear in both contexts.

### S07-4 — MEDIUM — The lock affordance points at a panel that has no lock control

`EditorCore::begin_drag` refuses a locked figure with
the status **"Figure is locked — unlock it in the
layers list."** (`src/editor.rs:305`) — but
`layers_panel.rs` rows carry **no lock toggle at all**;
the only lock control in the app is `fig-lock-{fi}` on
the figures panel rows. A user who follows the app's
own instruction finds nothing to unlock with.
Additionally, the lock guards only stage dragging:
a locked figure can still be deleted, reordered and
re-opacity'd from its own figures row (the row controls
never consult `locked`).

**Fix items:**
- **(a) (1 coder, `layers_panel.rs`, rides S07-2's
  two-line restructure):** add the lock toggle to the
  layers row line 2 — `icon_only_btn`
  (`layer-lock-{fi}`, `lock`/`lock-open`, 26 pt, active
  = locked, handler `toggle_figure_lock(fi)` verbatim
  from the figures row). Line-2 total 237 ≤ 260 (see
  S07-2). The layers list then matches the engine's own
  directions, and the state is visible where draw-order
  work happens.
- **(b) Cross-squad flag (engine, not S07 files):** for
  Squad 20 / integrator — decide whether `delete_figure`
  and `move_figure_full` should refuse on a locked
  figure the way `begin_drag` does. Panel-side, S07
  does not fake a guard the engine doesn't have.

### S07-5 — MEDIUM — Reorder buttons never disable at the ends of the list

`LayerRow.can_move_up` / `can_move_down` are computed
and unit-tested — and **never read by the builder**.
The back figure's Down button and the front figure's Up
button render fully live; pressing one silently selects
the figure and no-ops (figures panel `fig-up-{fi}` /
`fig-dn-{fi}` behave the same). Pose tools, in the same
column family, renders genuinely-unavailable ops inert
— so the app speaks two disabled-state languages.

**Fix items (1 coder each, keys verbatim,
`icon_only_btn_inert` already exists in the kit):**
- **(a) `figures_panel.rs`:** `fig-up-{fi}` inert when
  `fi` is the front figure, `fig-dn-{fi}` inert when
  `fi == 0` (the row data already carries `index`;
  the panel knows the total).
- **(b) `layers_panel.rs`:** wire the existing
  `can_move_up` / `can_move_down` fields to the
  down/up buttons and `jump_target(..) == None` to
  Bottom/Top, as part of the S07-2 restructure.

### S07-6 — MEDIUM — Layers rows are blind to hidden and locked state

`LayerRow` has no `hidden`/`locked` fields. A hidden
figure's layer row is pixel-identical to a shown one —
name not dimmed, dot not dimmed — in the panel that
owns draw order and ships visibility bulk ops in its
footer. The figures panel dims a hidden figure's name
(`text_secondary`); the layers panel, looking at the
same figure, says nothing.

**Fix item (1 coder, `layers_panel.rs`, rides S07-2):**
add `hidden`/`locked` to `LayerRow` (pure derivation,
no `State` change); when hidden, draw the name in
`t.text_secondary` and the dot at the figure colour
(the dot already reads muted by contrast) — zero width
cost. Locked state becomes visible via S07-4a's toggle.

### S07-7 — MEDIUM — Two competing "add figure" buttons in the same column

The figures header's **"+ Figure"** instantly adds a
humanoid; directly below it, a separate card's
**"+ Add figure"** opens the preset/library chooser.
A beginner (mission §8: "where to add a character"
must be obvious) cannot predict which button does
what, and the chooser — the one with quadrupeds, minis
and the library — reads as the secondary one.

**Fix item (1 coder, `figures_panel.rs`, label-only):**
relabel the header button **"+ Humanoid"** (its exact
behaviour: `add_figure()` *is* the humanoid preset,
`figure_add.rs` module docs) and leave the chooser's
"+ Add figure" as the general entry. Keys (`add-figure`,
`figure-add-toggle`), handlers and behaviour unchanged
— the two listed label changes are the whole item.
Alternative for the integrator's call: make `add-figure`
open the chooser instead — a behaviour change, **not**
recommended under the mission's preserve-behaviour rule.

### S07-8 — MEDIUM — The app's only irreversible delete has the plainest styling

Pose-library Delete (`pose_tools.rs`) removes a file
from disk with `std::fs::remove_file` — **not an undo
step, not recoverable** — yet renders as a plain
`icon_btn` labelled "Del", identical in weight to the
Rename button beside it. Meanwhile figure deletion is
danger-styled in the inspector (`danger_btn`) but a
plain 26 pt icon in the figures rows (`fig-del-{fi}`),
sitting in a cluster of harmless toggles (eye beside
it) where a misclick costs an undo-or-lose figure.

**Fix items:**
- **(a) (1 coder, `pose_tools.rs`):** `pose-lib-del-{i}`
  → `danger_icon_btn` (kit, icon_btn.rs:420 — same
  call shape as `icon_btn`, danger fills). Width delta:
  "Del" with glyph ≈ 57 pt either way; the entry's
  line 1 (name + Rename ≈85 + Del ≈57 + gaps) stays
  far inside 316. ✔
- **(b) Kit gap for Squad 02/17:** there is no
  *icon-only* danger button in the kit
  (`danger_icon_btn` is labelled). Propose
  `danger_icon_only_btn` (26 pt, `danger_*_color`
  fills, mirrors `icon_only_btn`); S07 adopts it for
  `fig-del-{fi}` once it exists. Until then the row
  delete keeps its current styling — noted, not faked.

### S07-9 — MEDIUM — Figure inspector: bare-text empty state; hidden/locked shown but not editable

- With no figures, `inspector_figure.rs` renders one
  bare `text("No figures in the project.")` — not the
  kit `empty_state`, no glyph, no next action. (Phase O
  charter seed list already names the inspectors'
  no-selection lines.)
- The panel *reports* hidden/locked in a read-only
  "State" readout row, but the property inspector —
  the place a user goes to change properties — offers
  no way to change either; both toggles live only on
  the figures list rows.

**Fix items (1 coder, `inspector_figure.rs`):**
- **(a)** Empty case → `design::empty_state("shapes",
  "No figures in the project.", t)` (glyph verified in
  use at `figure_add.rs`). Zero width risk (316 card).
- **(b)** Replace the conditional "State" readout with
  two kit `toggle_row`s — Hidden (`toggle_figure_hidden`
  after ensuring selection, the figures-row handler
  shape) and Locked (`toggle_figure_lock(fi)`) — always
  rendered, so the property is discoverable before it
  is ever set. `toggle_row` is the kit's used form
  (design.rs); rows are full-width 316. ✔
- Observation (no item): the heading leads with
  "Figure {n}" and demotes the name to meta, while every
  other surface leads with the name. Worth the
  integrator's eye during Phase 4; reversing title/meta
  is a one-line change if approved.

### S07-10 — LOW — Inspector swatch grid shows no selection for off-palette colours

The selected ring appears only when `st.color` exactly
equals one of the 16 `PALETTE` entries. After a
Recolour (which paints with an arbitrary *node* colour)
or a library figure with a custom colour, **no swatch
reads as current** — the grid silently disagrees with
the "Colour #RRGGBB" readout above it.

**Fix item (1 coder, `inspector_figure.rs`):** when
`st.color ∉ PALETTE`, prepend the current colour as a
17th swatch (selected ring, key `fig-color-current`,
press = re-apply `set_figure_color` with the same
value, i.e. a no-op write through the real op). Grid
arithmetic: first row becomes 9 swatches = 9×24 + 8×6
= **264 ≤ 316**. ✔

### S07-11 — LOW — Figure-add panel duplicates its presets and lists ragged buttons

- The Library list's first three entries are
  "Built-in: Humanoid / Quadruped / Mini" — the **same
  three figures** the Presets row directly above already
  offers. One card, two doors to the same three ops.
- Library entries are content-sized `btn`s stacked in a
  column, so the list is a ragged left edge of
  different-width buttons, unlike every other list in
  the app.

**Fix items (1 coder, `figure_add.rs`):**
- **(a)** `library_entries` skips the three built-ins
  (they are `core.library[0..3]` by `load_library`'s
  contract, pinned by this file's own tests) — entries
  keep their **real** library indices in
  `EntryKind::Library(i)`, so dispatch and keys
  (`fig-library-{i}`) stay correct; user library files
  are unaffected. If the library then has no user
  entries, the section shows the kit empty state that
  already exists. Test update: `library_entries_mirror_
  the_core_library` becomes "mirrors the user entries".
- **(b)** Library entry buttons take `.w_full()` so the
  list reads as a list (276 budget; labels left-aligned
  by the button's own row layout). Preset row unchanged
  (measured fit: ≈216 ≤ 276).

### S07-12 — LOW — Pose library vanishes when empty; filter glyph size drifts

- The whole "Pose library" section (header, filter,
  list) renders only `if !saved.is_empty()`. A new user
  never learns the library exists or that **Save pose**
  is what populates it — the save button writes to a
  list they cannot see.
- The filter's search glyph is 14 pt here, 13 pt in
  the figures filter (the kit's Phase N search size).

**Fix items (1 coder, `pose_tools.rs`):**
- **(a)** Always render the section header; when empty,
  render `design::empty_state("shapes", "No saved poses
  yet — Save pose stores the current pose here.", t)`
  (en-GB copy finalised by the copy pass). No width
  risk.
- **(b)** Filter glyph 14 → 13 pt to match
  `figures_panel.rs`.

### S07-13 — LOW — Cross-panel duplication & parity gaps (recorded, small items)

- **Opacity has four controls** for one value: figures
  row steppers, layers row steppers, layers slider,
  inspector stepper — plus the figures row's cryptic
  `fig-opac-{fi}` cycle button ("opac", cycles
  100→60→30), a fifth. The steppers/slider are
  defensible per-context duplication; the cycle button
  is the odd one (undiscoverable label, overlaps its
  own row's steppers). No removal proposed — zero key
  loss is binding — but the integrator should note it
  as accepted redundancy or approve a relabel.
- **Mirror pose exists twice** in the right column:
  inspector `fig-flip` ("Flip horizontal") and pose
  tools `pose-mirror` ("Mirror pose") call the same op
  (`mirror_selected_pose`) under two names. One
  terminology should win in the copy pass ("Mirror" —
  the pose panel's word, and the op's own name).
- **Layers footer parity:** figures footer offers All /
  None / Invert; layers footer only Show all / Solo
  selected. Adding None + Invert to layers: measured
  footer now 68.5 + 97.5 + gap 6 = 172; + None ≈51 +
  Invert ≈53 + 2 gaps = **288 > 276** ✗ — does not fit
  one line; only worth doing as a two-row footer.
  Recorded as a parity note, not a sized item.
- **Pose row 2 overflow (+16.5, `pose-apply-all` past
  the content edge)** is P05 item O-9 (wrap the row) —
  S07 concurs; no duplicate item.
- **Figures count meta** is a bare number ("3") with no
  unit when unfiltered (`count_line` is pinned
  byte-identical by tests). "3 figures" would read
  better; it is a listed copy change + test update if
  the integrator wants it. Not sized here.

---

## Item index (for Phase 4 assignment)

| Item | File | Size | Depends on |
|---|---|---|---|
| S07-1 figures row restructure | figures_panel.rs | 1 coder | — |
| S07-2 = P05 O-3 layers two-line row | layers_panel.rs | 1 coder | — |
| S07-3 rekey inspector duplicate | inspector_figure.rs | 1 coder | integrator census note |
| S07-4a layers lock toggle | layers_panel.rs | 1 coder | S07-2 |
| S07-5a figures inert ends | figures_panel.rs | 1 coder | — |
| S07-5b layers inert ends | layers_panel.rs | 1 coder | S07-2 |
| S07-6 layers hidden dimming | layers_panel.rs | 1 coder | S07-2 |
| S07-7 "+ Humanoid" relabel | figures_panel.rs | 1 coder | — |
| S07-8a pose-lib-del danger | pose_tools.rs | 1 coder | — |
| S07-8b kit danger icon-only | icon_btn.rs (Squad 02/17) | 1 coder | then S07 adopts |
| S07-9a inspector empty state | inspector_figure.rs | 1 coder | — |
| S07-9b inspector hidden/locked toggles | inspector_figure.rs | 1 coder | — |
| S07-10 current-colour swatch | inspector_figure.rs | 1 coder | — |
| S07-11a library de-duplication | figure_add.rs | 1 coder | — |
| S07-11b library buttons full width | figure_add.rs | 1 coder | — |
| S07-12a pose library empty state | pose_tools.rs | 1 coder | — |
| S07-12b filter glyph 13 pt | pose_tools.rs | trivial | — |

**Verification for every item:** keys/handlers census
(gate, 804 baseline — S07-3's rename recorded), CI green,
and a headless tree dump proving the quoted rects
(`fig-sel-0` width, `layer-row-0` containment, row
heights) plus the integrator's pixel read. No item
claims visual success from source alone.
