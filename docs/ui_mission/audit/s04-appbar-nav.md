# Squad 04 Audit — App Bar & Navigation

Scope: `src/chrome_view.rs` (app bar, transport, timeline
chips, legacy status line) and `src/rune_sec/menus.rs`
(menu bar, dropdowns, scrim), plus the chrome stack they
sit in (`src/main.rs::view` root column) and the workspace
chip row directly beneath them (`workspaces::workspace_row`,
Squad 05's file — noted here only where it affects the
chrome stack). Read-only audit; every claim below is from
current source, not from the phase docs.

## 1. The File menu question — settled from source

The Phase N pixel pass photographed a File menu with
three items (Save / Load / Export PNG) and could not tell
whether that was the real definition or a scene artefact.

**It is the real definition.** The FILE arm of
`menus::menu_dropdown` builds, in order:

1. `save` — "Save" → `core.save()`
2. `load` — "Load" → `core.load()`
3. **Only if `shell::recents()` is non-empty:** a
   separator (`menu-file-sep`) plus up to five Open
   recent entries (`file-recent-0..4`, cap
   `MAX_RECENT_ITEMS = 5`)
4. `export` — "Export PNG" → `core.export_png()`

The headless scene state has no recents, so the snapshot
showed exactly the base menu: **three items, and no
separator anywhere in it**. With recents present the menu
grows to up to nine rows. There is no New, Open…,
Save As, Close or Exit item in the File menu, and no
grouping separator between the open/save cluster and
Export PNG. This is a genuine completeness defect
(D1 below), not a rendering bug.

## 2. Current anatomy

### 2.1 Chrome stack (main.rs `view()`, root column)

Four root children, in order, separated by the root's
12 pt gap and one kit hairline under the whole block:

1. `chrome_view::app_bar` — key `app-bar`
2. `menus::menu_bar` — key `menu-bar`
3. `workspaces::workspace_row` — key `workspace-row`
4. `design::divider` (Phase N charter §15 hairline)

Then the three-column main row, toasts, and — mounted
last, out of flow — `menus::menu_scrim` + `menu_dropdown`.

All three chrome rows use the same `btn`/`icon_btn`
styling (radius 6, px 10 / py 5, card fill). Nothing
visually distinguishes the "toolbar" row from the "menu"
row from the "workspace" row except their content.

### 2.2 App bar (`chrome_view::app_bar`)

One row, gap 8, `center_y`:

| Element | Key | Form | Action |
|---|---|---|---|
| Project name | *(unkeyed text)* | `type_::TITLE`, `t.text`, in a `grow(1.0)` row | — |
| Dirty dot | *(unkeyed)* | 8 pt accent disc, present iff `core.edits_since_autosave() > 0` | — |
| Save | `save` | `icon_btn`, floppy-disk + label | `core.save()` |
| Load | `load` | `icon_btn`, folder-open + label | `core.load()` |
| Undo | `undo` | `icon_btn`, arrow-rotate-left + label | `core.undo()` |
| Redo | `redo` | `icon_btn`, arrow-rotate-right + label | `core.redo()` |

Labels resolve through `i18n_wire::tr` (keys `save`,
`load`, `undo`, `redo`) in the core's current language.
`icon_btn` renders glyph + visible text label side by
side — the app bar is *not* icon-only. Phase F removed
the old + Figure and Onion buttons from this bar (they
duplicated panel controls); that deduplication is
correct and should not be reversed.

Not present anywhere in the app bar: application
wordmark/identity, New Project, settings access, theme
control, window-level actions.

### 2.3 Menu bar and dropdowns (`menus.rs`)

Triggers: four text `btn`s keyed `menu-file`,
`menu-edit`, `menu-view`, `menu-help` (labels File /
Edit / View / Help, `MENUS`/`MENU_KEYS` tables), accent
fill while their menu is open. The bar is a plain row —
nothing right-aligned, no spacer content; the rest of
the row's width is empty.

Dropdown mechanics (sound, keep): state-toggled anchored
column (`El::anchored(trigger, Side::Bottom,
Align::Start, 4.0)`), transparent full-window scrim
(`menu-scrim`) beneath it, close-on-select via the
`closing` wrapper, Esc closes (integrator `on_key`
wiring), trigger press toggles. Phase N surface: radius
12, padding 6, gap 2, `min_w(190.0)` for every menu,
fade-and-rise entrance on the SNAP spring pair. Items
are 28 high at px 8 with a fixed 18 pt leading icon slot
(`ICON_SLOT_W`) so labels share one left edge; shortcut
hints render as kit `kbd` chips (plain white 75% text on
the accent/active state).

Item inventory (key — label — condition):

**File** (`menu-file-dropdown`): `save`, `load`,
[`menu-file-sep` + `file-recent-{i}` × ≤5, only when
recents exist], `export` ("Export PNG").

**Edit** (`menu-edit-dropdown`): `undo`, `redo`,
`menu-edit-sep`, `copy-frame`, `paste-frame`, then
conditional items on the offer-only-while-it-can-run
discipline: `edit-delete-stroke` (stroke selected),
`edit-delete-particle` (particle event selected),
`edit-swap-next` (a next frame exists), `edit-mute-clip`
(an audio target exists; label/icon name the action for
the clip's current mute state).

**View** (`menu-view-dropdown`): `onion` (toggle,
hint "O"), `view-guides`, `view-guides-frustum`,
`view-guides-safe` (guide toggles, active = flag),
`view-speed` (action: cycle playback speed), `menu-view-sep`,
`view-fit`, `view-centre`, `menu-view-sep2`,
`view-toggle-left`, `view-toggle-right` (sidebar toggles).

**Help** (`menu-help-dropdown`): `help` ("Shortcuts",
toggles the help panel), `menu-help-sep`, `menu-palette`
("Command palette", hint "Ctrl+K").

Shortcut hints exist for exactly five items (`save`
Ctrl+S, `undo` Ctrl+Z, `redo` Ctrl+Shift+Z, `onion` O,
`menu-palette` Ctrl+K) — each verified against
`main.rs::on_key` routing and pinned by the
`shortcut_hints_name_only_real_bindings` test. The other
22 items have no shell binding and correctly show no
hint. Icons: `menu_icon` maps 19 item keys (+ the
recent-prefix family) to Font Awesome glyphs; `view-fit`
and `view-centre` are deliberately unmapped.

### 2.4 Transport & timeline (`chrome_view.rs`)

`transport_row` (key `transport`): `prev` (chevron-left),
`play` (play/pause glyph + label, accent while playing),
`next` (chevron-right), a grow spacer, then `add-frame`
(plus), `dup-frame` (copy), `del-frame` (trash-can) —
all plain `icon_btn`s. `timeline_row` (key `timeline`):
one 30×24 chip per frame keyed `frame-{i+1}`, numbered,
accent on the current frame, horizontal scroll.
`status_text` is the legacy single-line status
("Frame n/N · F fps · Zoom P% · {status}"), deliberately
literal English (Phase E decision); the segmented
`status_strip` is the live status surface.

### 2.5 Workspace chip row (Squad 05's file, chrome-stack impact)

`workspace_row`: eight `btn` chips keyed per workspace,
labels of the form `"Animate · Alt+1"` — the shortcut is
baked into the label text run — above a truncating
secondary blurb line (`ws-blurb`). No glyphs on the
chips. The row is visually identical in weight to the
menu bar above it.

## 3. Defects (prioritised)

**D1 — File menu is three base items, ungrouped.**
Save / Load / Export PNG with no separator and no other
commands (§1). A professional File menu is where users
look for New, Open recent, Save As and export; here Open
recent exists but is invisible until a recent exists,
and Export PNG — the only export entry — sits ungrouped
directly under Load. (Save As and a file-dialog Open do
not exist as core ops — `core.save()` writes one fixed
path — so completing this menu is partly engine work,
not chrome work; see R1.)

**D2 — Undo/Redo never show a disabled state.**
`app_bar` renders both as live `icon_btn`s
unconditionally. The core exposes `undo_depth()` /
`redo_depth()` (`editor.rs`), and the button family
already has inert forms (`icon_btn_inert`), but the
chrome never consults the depths. Worse, `core.undo()`
on an empty stack is a *silent* no-op — it doesn't even
set a status. On a fresh project both buttons look fully
active and do nothing when pressed. Same for the Edit
menu's Undo/Redo items, which are always offered.

**D3 — Save gives no state feedback, and the dirty
signal measures the wrong save.** The dirty dot reads
`edits_since_autosave()` — edits the *autosave* hasn't
captured — not edits since the user's last explicit
Save. The `chrome_view` doc comment itself warns the two
signals must not be conflated. So the dot can read
"clean" while the on-disk save file is stale, and the
Save button itself never changes appearance. There is
no chrome surface that answers "is my work saved?".

**D4 — Key collisions between the app bar and open
menus.** `save`, `load`, `undo`, `redo` are carried by
*both* the app-bar buttons and the File/Edit menu items.
While a menu is open, the presented tree contains each
key twice, so any key-based tree lookup (headless
checks, tests, future automation) is ambiguous, and the
screen shows the same command twice in near-identical
presentation. (Fix is constrained — see R5.)

**D5 — The menu bar's right half is dead space; theme
and settings have no chrome presence.** The bar ends at
Help. The theme flip (sun/moon pair) exists only inside
the Settings card (`settings_popover.rs`), which is a
workspace-gated column section — a user in the Animate
workspace has no theme control on screen at all, and no
way to discover one from the chrome. Professional
creative apps put theme/settings at the header's right
edge.

**D6 — No application identity in the header.** The
app bar leads with the project name and nothing else;
the product name exists only in the OS window title
("StykAnimate RS"). First-run users get no brand anchor,
and the project name — the one piece of text that
identifies their work — carries the whole row's
identity alone.

**D7 — Project name cannot truncate and cannot be
edited from the header.** The name text has no
`.truncate()` and no width cap (contrast `ws-blurb`,
which truncates): a long project name grows at the
expense of the four action buttons at the row's right.
Renaming lives in the Project section's name field
(`project_tools`), with nothing in the header hinting
that it exists.

**D8 — Open menus don't switch on hover.** Triggers
respond to press only (`MenusState::toggle`). Once a
menu is open, moving the pointer across to a neighbouring
trigger does nothing — every professional menu bar
switches on hover in this state, and users will try it.
Whether Rune exposes a hover-*event* primitive (as
opposed to hover fills) must be verified in `rune-src`
before this is scheduled; if it doesn't, this is an
engine limit to record, not a defect to fake around.

**D9 — The transport's one destructive action is
unmarked.** `del-frame` uses the standard `icon_btn`
styling — same fill, same ink as Add and Duplicate —
although the shell has a measured danger family
(`danger_btn` / `danger_icon_btn`, `DANGER_FILL` ≈ 5:1)
that the figure/segment inspectors use for exactly this
purpose. Delete-frame is undoable, so no confirmation is
wanted; the issue is purely that the chrome's danger
language stops at the panels and never reaches the
transport.

**D10 — Three chrome rows, one visual weight.** App
bar, menu bar and workspace row are all radius-6 card-fill
button rows at the same type size, separated only by the
root's uniform 12 pt gap. There is no hierarchy: the
workspace switcher — a primary navigation control —
reads as a third menu row, and its chips bake the
Alt+N shortcut into the label text instead of using the
kit `kbd` chip the menus adopted in Phase N (Squad 05
owns the fix; flagged here because the stacking is a
chrome-level problem).

Minor: every dropdown inherits `min_w(190.0)` regardless
of content, so the two-item Help menu is mostly empty
width; item *height/spacing* is otherwise consistent.

## 4. Redesign items (one coder each, zero key loss)

All items keep every existing key, handler and label
byte-identical unless the item says otherwise. Keys
*added* by an item are new census entries (the census
rule is zero **loss**; additions are how Phase N grew
it), and each item names its proof.

**R1 — File menu grouping + recents discoverability**
(menus.rs). Add a separator before `export` (new key in
the existing naming pattern, e.g. `menu-file-sep2`) so
the menu reads as [Save, Load] / [recents…] / [Export
PNG]. Do **not** add New / Save As items: no real ops
exist for them today (`core.save()` has one fixed path;
the two-"New project" behaviours are a standing product
decision for the user, not this squad). Record both as
product/engine decisions in the mission defect log.
Proof: existing menu build tests + a new FILE-arm build
test; headless menu-open scene.

**R2 — Undo/Redo inert states in the app bar**
(chrome_view.rs). In `app_bar`, render `undo` via
`icon_btn_inert` when `core.undo_depth() == 0` and
`redo` likewise with `redo_depth()`; live `icon_btn`
otherwise. Keys, glyphs, labels and handlers unchanged
(the inert form drops the handler — the established
Phase I pattern, and strictly better than today's silent
no-op). Leave the Edit menu items live (menus
close-on-select makes a muted menu row a worse trade;
revisit only if R2's pattern proves out). Proof:
`dirty_dot`-style unit tests on depth gating + Runtime
snap test with a fresh core (both inert) and after an
edit (undo live).

**R3 — Honest saved-state in the app bar** (chrome_view.rs
+ editor.rs, integrator-approved because it touches the
core). Add an edits-since-explicit-save counter beside
the autosave counter, and drive a Save-button state from
it: muted-live when there is nothing unsaved, normal
when there is. The existing dot keeps its autosave
meaning (recovery signal) — do not repurpose it. If the
integrator rejects new core state, fall back to a
status-strip-only fix and record the app-bar limitation.
Proof: unit tests on the counter; snap test clean/dirty.

**R4 — Project-name truncation** (chrome_view.rs). Add
`.truncate()` to the name text (mirroring `ws-blurb`) so
long names ellipsise inside their grow row instead of
squeezing the action cluster. No key involved (the text
is unkeyed). Proof: snap test with a long project name;
headless tree shows the four buttons at their resting
rects.

**R5 — Menu-item key namespacing — DECISION ITEM, not a
coder item.** D4's clean fix (prefix menu items, e.g.
`menu-save`) changes existing keys and breaks the
zero-key-loss rule and the menu tests that pin them;
the alternative (scope all tree lookups under the
dropdown ancestor) keeps the keys and pushes discipline
onto every future check. Present both to the integrator;
do not implement without a ruling.

**R6 — Theme toggle at the app bar's right edge**
(chrome_view.rs; reuses settings_popover's theme fn).
Add one `icon_btn` after `redo` calling the Settings
section's existing theme setter on `core.light_theme`,
glyph sun/moon by current theme (the settings card's
own pair logic), label resolved through `tr()` (a new
i18n table key is an addition, coordinated with the
i18n owner). New button = new key (suggest `theme`),
zero loss. The Settings card keeps its pair — this is
the discoverable door, not a move. Proof: snap tests in
both themes; settings-scene parity check.

**R7 — Danger styling for `del-frame`** (chrome_view.rs).
Rebuild the transport's delete button on
`danger_icon_btn` (icon_btn.rs) with the same key,
glyph (`trash-can`), i18n label and handler. One
destructive language across chrome and panels. Proof:
existing transport build tests; pixel read of the
transport in the default scene.

**R8 — Hover-switch investigation** (menus.rs +
rune-src). Time-boxed spike: determine from `rune-src`
whether a hover-enter event exists on `El`. If yes,
implement switch-on-hover only while `menus.open` is
`Some` (press behaviour untouched, Esc/scrim untouched)
with Runtime tests. If no, close D8 as a recorded Rune
limit in QA_RUNE and stop. No fake substitutes (no
timers, no polling).

**R9 — Per-menu dropdown width** (menus.rs). Replace the
uniform `min_w(190.0)` with per-menu minima (File/Edit/
View keep 190; Help may size to content with a smaller
floor). Keys and anchoring unchanged. Low priority —
schedule only after R1–R7. Proof: menu-open scenes for
all four menus.

**Referral to Squad 05** (not an R-item here): workspace
chips should render the Alt+N hint as a kit `kbd` chip
and consider per-workspace glyphs, so the third chrome
row stops reading as a third menu row (D10). The chrome
stack's row differentiation (weights/gaps) should be
decided jointly by Squads 02/03/05 once R6 lands.

## 5. What must not change

- Every key in §2 (`app-bar`, `menu-bar`, `menu-scrim`,
  the four triggers and dropdowns, all item keys,
  `transport`, `timeline`, `frame-{i}`, separators) —
  verbatim.
- The hint honesty rule: no shortcut hint may appear
  before its binding exists (`shortcut_hint` + its
  pinning test are the model).
- The conditional-item discipline in Edit/File: items
  exist only while their action can run; no inert
  placeholder rows in menus.
- Close-on-select, scrim dismissal, Esc dismissal,
  anchored positioning, and the dropdown entrance.
- The i18n route for all chrome labels (`tr()`), and
  `status_text`'s pinned literal format.
- Phase F's deduplication: no + Figure / Onion buttons
  back in the app bar.

## 6. Squad report block

- **Files inspected:** `src/chrome_view.rs` (full),
  `src/rune_sec/menus.rs` (full), `src/main.rs` (view
  root + State wiring, targeted), `src/rune_sec/icon_btn.rs`
  (builder contract), `src/rune_sec/workspaces.rs`
  (`workspace_row`), `src/editor.rs` (`undo`/`redo`/
  `save`/`undo_depth` regions), `src/rune_sec/status_strip.rs`
  (`project_text`), `src/rune_sec/settings_popover.rs`
  (theme pair location), AGENTS.md.
- **Problems discovered:** D1–D10 above; File-menu
  question (§1) settled — the 3-item menu is the real
  base definition.
- **Changes implemented:** none — Phase 1 audit only.
  This document is the deliverable.
- **Components reused (in proposals):** `icon_btn_inert`,
  `danger_icon_btn`, kit `kbd`, kit `divider_keyed`,
  settings section's theme setter, `i18n_wire::tr`.
- **Components added:** none proposed beyond one new
  separator key (R1), one new button key (R6), one new
  i18n key (R6), and possibly one core counter (R3,
  integrator-gated).
- **Existing behaviour preserved:** §5 list.
- **Verification performed:** source-level — menu
  contents traced through `menu_dropdown`'s four arms;
  hint/icon maps checked against their pinning tests;
  undo/redo emptiness behaviour read in `editor.rs`;
  no builds run (AGENTS.md CI-only rule).
- **Known limitations of this audit:** no pixel evidence
  was produced by this squad (Squad 19 owns visual QA);
  D8's feasibility is unverified against `rune-src` by
  design (R8 is the spike).
- **Remaining defects after the R-items:** D5 partially
  (settings itself still lives in a gated card — a
  chrome settings door is a Squad 03/16 decision), D6
  (wordmark — a Squad 02/03 identity decision), D10
  (owned by Squad 05), D4 pending the R5 ruling.
