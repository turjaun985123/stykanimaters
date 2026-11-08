# P14 — Keyboard & Focus UX Audit (Phase N, planning)

Scope: every keyboard binding the shell actually routes,
every surface that could tell the user about it (menus,
help panel, command palette, workspace chips, buttons),
and what Rune's focus model can and cannot give the
shell in Phase N. All findings are from source:
`src/main.rs` (`on_key`, `text_focused`),
`src/rune_sec/menus.rs` (`shortcut_hint`),
`src/rune_sec/help_panel.rs` (`section_rows`),
`src/rune_sec/command_palette.rs` (`COMMANDS`,
`palette_handle_key`, the view), and the pinned Rune
tree at `~/workspace/rune-src` (cited per claim).

Charter anchors: §9 (menus/popovers/palette get `kbd`
shortcut chips), §11 (`design::kbd` spec — px 5 / py 1,
radius 4, fill `control_off`, MONO_CAPTION,
`text_secondary`), §5 list-row metrics, and the
standing honesty rule the shell already applies to
hints: **a hint names only a real binding** (menus.rs
`shortcut_hint` doc, pinned by the
`shortcut_hints_name_only_real_bindings` test). This
plan adds no invented hints anywhere.

---

## 1. The complete live binding inventory

From `main.rs::on_key` (line 462). Routing inside the
function, in order:

1. `command_palette::palette_handle_key` gets first
   claim (Ctrl+K opens; while open it consumes
   Down/Up/Enter/Escape — command_palette.rs:1450).
2. `text_focused(s)` guard (main.rs:432) — if any of
   the **14 named text fields** is focused (palette
   query, project name, audio clip name, Home search,
   library search + rename, text-tool field, figure
   name, clip rename, FX browser query, figures
   filter, pose rename, pose-library filter),
   `on_key` returns `false` and every binding below
   is dead until the field blurs.
3. Alt+digit, Ctrl chords, then the match:

| Binding | Action | Routed at |
|---|---|---|
| Ctrl+K | Open command palette | palette_handle_key:1452 |
| Ctrl+Z | Undo | on_key, `is_command('z')` |
| Ctrl+Shift+Z | Redo | on_key, z + shift |
| Ctrl+Y | Redo | on_key, `is_command('y')` |
| Ctrl+S | Quick save (= app-bar Save press) | on_key, `is_command('s')` |
| Alt+1 … Alt+8 | Switch workspace (Animate…Home) | on_key, alt + digit 1–8 |
| Space | Play / pause (no modifiers) | match, `Key::Text(" ")` |
| Left / Right | Previous / next frame (wraps) | match, `Key::Left/Right` |
| Delete | Priority cascade: selected **stroke** → selected **particle event** → selected **segment** | match, `Key::Delete` |
| C | Toggle camera mode | match, text "c", no modifiers |
| O | Toggle onion skin | match, text "o", no modifiers |
| N | Add frame after current | match, text "n", no modifiers |
| I | Insert inbetween | match, text "i", no modifiers |
| Escape | Close the open menu (palette Escape is the palette's own) | match, `Key::Escape` + `menus.open` |

Palette-open only (command_palette.rs:1458–1502):
Down/Up move the selection (wrapping), Enter runs it,
Escape closes; the query field's own handler
(`palette_field_key`) takes Down/Up/Escape and Enter
falls through to the field's `on_submit`.

That is the whole keyboard surface: **16 shell
bindings (counting Alt+1..8 as one family) + 4
palette-internal keys.** There are no others — no
menu mnemonics, no F-keys, no Ctrl+O/Ctrl+N, no
Home/End frame jumps. Anything not in this table
must not be hinted, in any Phase N surface.

## 2. Discoverability audit — binding by surface

Surfaces checked: menu hints (`menus::shortcut_hint`,
exactly 5 entries: save/undo/redo/onion/menu-palette),
help rows (`help_panel::section_rows`, 54 rows:
9 Shortcuts + 8 Workspaces + 2 Posing + 5 Timeline +
0 Drawing + 30 Features), palette rows + footer,
workspace chips, transport/app-bar buttons. The shell
uses **no tooltips anywhere** (`grep tooltip src/` =
0 hits), so tooltips are not an existing hint vehicle
and this plan does not propose them.

| Binding | Menu hint | Help row | Palette | Elsewhere | Verdict |
|---|---|---|---|---|---|
| Ctrl+K | ✓ Help ▸ Command palette | ✓ Shortcuts | — | — | **Covered** |
| Ctrl+Z | ✓ Edit ▸ Undo | ✓ Shortcuts | ✗ (Undo command row shows no key) | — | Menu/help only |
| Ctrl+Shift+Z | ✓ Edit ▸ Redo | ✓ Shortcuts | ✗ (Redo row) | — | Menu/help only |
| Ctrl+Y | — (no second Redo item exists) | ✓ Shortcuts | ✗ | — | Help only — acceptable, one hint per item rule |
| Ctrl+S | ✓ File ▸ Save | ⚠ **not a Shortcuts row** — buried as a Features row whose label is "Quick save", not the key | ✗ (Save project row) | — | **Gap H1** |
| Alt+1..8 | — | ✓ Workspaces (all 8) | ✗ (go-* rows ×8) | ✓ chip labels "Animate · Alt+1" + ws-blurb line | **Best covered family** |
| Space | — (no transport menu) | ✓ Timeline | ✗ (Play / Pause row) | ✗ Play button carries no hint | **Gap D1** |
| Left / Right | — | ✓ Timeline | ✗ (Previous/Next frame rows) | ✗ transport buttons carry no hint | **Gap D1** |
| Delete | ✗ **deliberately** (see §3) | ⚠ row exists but text is **stale**: "Delete the selected segment" (help_panel.rs:234) — the key has been a stroke→particle→segment cascade since Phase J | — | — | **Gap H2 (copy honesty)** |
| C | — (no menu item for camera mode) | ✓ Posing | — (no palette command) | ✗ stage mode buttons carry no hint | **Gap D2** |
| O | ✓ View ▸ Onion skin ("O") | ✓ Posing | ✗ (Toggle onion skin row) | — | Menu/help only |
| N | — | ✓ Timeline | ✗ (Add frame row) | ✗ "+ Frame" button carries no hint | **Gap D1** |
| I | — | ✓ Timeline | ✗ (Insert inbetween row) | ✗ Frame Ops "Inbetween" button carries no hint | **Gap D1** |
| Escape | — | ✓ Shortcuts | ✓ footer text | — | Covered |
| Palette ↑/↓/Enter/Esc | — | ✓ Shortcuts (4 rows) | ⚠ footer exists as one plain-text line, not chips | — | **Gap P2 (styling)** |

### The gaps, named

- **P1 — the palette hides the keys it duplicates
  (biggest win).** 17 of the 108 commands run exactly
  a key-bound action and show no key: `play` (Space),
  `prev` (Left), `next` (Right), `add-frame` (N),
  `inbetween` (I), `undo` (Ctrl+Z), `redo`
  (Ctrl+Shift+Z), `save` (Ctrl+S), `onion` (O),
  `go-animate`…`go-home` (Alt+1…Alt+8). The result
  row (command_palette.rs view) renders title (grow)
  + group label only; the `Command` struct
  (command_palette.rs:219) has no hint field
  (`key`/`title`/`group`/`run` only). The palette is
  the shell's discovery surface — a user who finds
  "Add frame" there should leave knowing `N` exists.
- **P2 — palette footer is prose, not chips.** Footer
  (command_palette.rs:1617) is one LABEL text
  "↑↓ navigate · Enter run · Esc close". Charter §11
  names the palette footer as a `kbd` consumer.
- **D1 — transport and frame buttons carry no hint.**
  Space, Left, Right, N, I are help-only. The buttons
  themselves (Play, Previous/Next frame, + Frame,
  Inbetween) show nothing. Phase N's vehicle is the
  `kbd` chip; §4 below scopes where chips fit on
  buttons and where they don't.
- **D2 — C (camera mode) has no second surface.** No
  menu item, no palette command; only the Posing
  help row and the unlabelled stage mode toggle.
- **H1 — Ctrl+S is missing from the Shortcuts
  section.** It is menu-hinted and real, but a user
  reading the help panel's Shortcuts table never
  sees it as a key; it sits in Features as
  ("Quick save", "Ctrl+S — the app-bar Save
  button's save, on the keyboard").
- **H2 — the Delete help row understates the key.**
  "Delete the selected segment" has been wrong in
  scope since the Phase J priority cascade landed.
  The row should name the cascade. (Copy change —
  routed in §5, not a styling change.)
- **H3 — the text-field caveat is documented
  nowhere.** Every single-letter binding, Space,
  and the arrows all stop working while any of the
  14 text fields is focused, and nothing on screen
  says so. A user typing in the library search who
  presses Space gets a space in the field, not
  playback — correct behaviour, invisible rule.
  The help panel already has a mechanism for this
  (`section_note`, currently used only by Drawing).
- **M1 — menus are pointer-only to open and to
  navigate.** Triggers (`menu-file`…`menu-help`,
  menus.rs:113) are plain buttons; no key opens a
  menu, and an open dropdown's items are plain
  buttons with no arrow-key navigation, no
  type-select, no focus group (the dropdown's scrim
  even sets `.role("none")`, menus.rs:634). Escape
  closes (on_key). Rune *can* do roving menu focus
  (§6) but wiring it is behaviour, not restyle —
  scoped as an optional F-wave item in §5 with its
  hazards named, not silently assumed.

## 3. What must NOT get a chip (documented refusals)

`menus::shortcut_hint`'s doc comment records two
deliberate decisions. Phase N keeps both:

- **Edit ▸ Delete stroke / Delete particle event get
  no "Delete" hint.** The key is bound to the
  priority cascade, not to either item's single
  action; a chip on the stroke item would claim the
  key targets strokes when, with no stroke selected,
  it deletes something else. The items stay
  mouse-only doors onto the cascade's first two
  branches.
- **Edit ▸ Swap frame with next and Mute/Unmute get
  no hint** — no global key is bound to either action
  alone (they are palette-reachable, key-less).
- Everything else in the menus (Load, Export PNG,
  Copy/Paste frame, guides, Playback speed, Fit view,
  Centre on selection, sidebar toggles, Shortcuts)
  has no binding and gets no hint. The table stays at
  5 entries. Phase N work on menus is **chip styling
  of the existing hints only** (charter §9).

Likewise in help: the 30 **Features rows are not
shortcut rows** — their first column names a feature,
not a key. They must not be rendered as `kbd` chips
(a chip would claim a binding). Only the 24 true-key
rows (9 Shortcuts + 8 Workspaces + 2 Posing +
5 Timeline) get chips. The one exception inside
Features is "Quick save", whose *description* names
Ctrl+S; the fix is H1 (a real Shortcuts row), not a
chip on the Features row.

## 4. Where `kbd` chips go (charter §9 + §11)

`design::kbd` per the charter signature:
`kbd(label, t)` — px 5 / py 1, radius 4, fill
`control_off`, MONO_CAPTION, `text_secondary`.

1. **Menus (F wave, menus.rs).** `hinted_item`
   (menus.rs:224) and the `item_icon` hint path
   (menus.rs:394, 770) render the hint today as
   plain secondary text inside the item row. Swap
   that text element for `design::kbd(hint)`. Same 5
   hints, same keys, same handlers; the hint stays
   inside the pressable row. Dropdown chrome moves
   to charter §9 metrics in the same pass (card fill,
   `card_border`, radius 12, padding 6; items 28
   high, px 8, radius 6).
2. **Help rows (A wave, help_panel.rs).** The
   private `key_cap` (help_panel.rs:427 — px 6 / py 2,
   radius 4, fill `t.card`, border `control_track`,
   LABEL-in-Mono, `t.text`) is a pre-kit kbd in all
   but name. Retire it onto `design::kbd` in
   `shortcut_row` (help_panel.rs:442). One deviation
   to report, not hide: `key_cap` is bordered and
   full-ink; the charter chip is borderless
   `control_off` at secondary ink — long labels like
   "Ctrl+Shift+Z" stay legible at MONO_CAPTION 12px,
   so adopt the charter spec as-is.
3. **Palette footer (F wave, command_palette.rs).**
   Replace the single prose line with: `kbd("↑↓")`
   "navigate" · `kbd("Enter")` "run" · `kbd("Esc")`
   "close", LABEL secondary text between chips, in
   the existing 26px footer row (footer row key is
   unkeyed today — keep it unkeyed).
4. **Palette result rows (F wave, command_palette.rs)
   — the P1 fix.** Add a trailing `design::kbd` after
   the group label for the 17 key-bound commands.
   Data: a new private table in command_palette.rs,
   `command_hint(key: &str) -> Option<&'static str>`,
   keyed by *command* key (the spellings mirror
   `help_panel::section_rows` exactly — "Space",
   "Left", "Right", "N", "I", "O", "Ctrl+Z",
   "Ctrl+Shift+Z", "Ctrl+S", "Alt+1"…"Alt+8"). It
   must be a separate table from `menus::shortcut_hint`
   (different key namespace: menu item keys ≠ command
   keys), so add the drift guard as a unit test in
   command_palette.rs: every `command_hint` value
   appears as a key label in
   `help_panel::shortcut_rows()`. Row layout: title
   grows, group label, then chip; on the selected
   (accent-filled) row the chip keeps its
   `control_off` fill — report contrast in the F
   report; if it reads poorly, the row's chip may
   take the white-at-alpha treatment `hinted_item`
   already uses for active items (menus.rs:222).
   Charter §9 says palette item rows are 28 high;
   rows are 34 today (command_palette.rs view) —
   adopt 28 in the same pass and report if the
   BODY-font title + chip crowd.
5. **Workspace chips — leave the labels alone.**
   Chips already carry "· Alt+n" as label text
   (workspaces.rs:424) and the `ws-blurb` line
   repeats the active workspace's blurb. Splitting
   the suffix into a `kbd` chip is cosmetic churn on
   a surface that is already covered, and chip label
   strings are pinned by workspaces tests. Not in
   scope; recorded as considered-and-rejected.
6. **Buttons — one scoped exception.** Do **not**
   add chips to panel buttons generally (rows would
   grow everywhere; menus + help + palette already
   cover those bindings). The exception is the
   transport cluster in chrome_view.rs, where D1
   lives: Play (Space), Previous (Left), Next
   (Right) are the app's most-used buttons and their
   keys are otherwise help-only. Proposal for the F
   chrome agent: a `kbd` chip inside each of the
   three transport buttons, after the label/icon,
   using the kit chip at reduced prominence
   (`text_secondary` on `control_off` already is).
   "+ Frame" (N) and Frame Ops "Inbetween" (I) stay
   unchipped — they are covered once P1 lands (the
   palette teaches N and I at the moment of
   discovery). If the chrome agent finds the chips
   crowd the transport row at 1280px min width,
   report it and drop to Play-only.

## 5. Per-file work list (coding waves)

**F — foundation**

- `src/rune_sec/design.rs` — build `kbd()` to the
  charter signature. Plus one proposed addition,
  flagged for integrator approval because the
  charter's fixed API has no focus entry: ring
  constants for §6 adoption —
  `pub const FOCUS_RING_W: f32 = 2.0;`
  `pub const FOCUS_RING_OFF: f32 = 2.0;`
  with `accent()` as the ring colour (so every
  adopted ring matches the design language instead
  of mixing with Rune's system-colour default ring —
  see §6). No other focus helpers; the builders are
  one-liners at the call site.
- `src/rune_sec/menus.rs` — hint slot in
  `hinted_item`/`item_icon` → `design::kbd`;
  dropdown + item metrics → charter §9. **No table
  changes**: `shortcut_hint` stays 5 entries, its
  doc and its pinning tests stand.
- `src/rune_sec/command_palette.rs` — footer chips
  (§4.3); `command_hint` table + row chips (§4.4)
  + the drift-guard test; panel border literal
  `Color::WHITE.alpha(0.10)` → `t.card_border`;
  row height 34 → 28 (charter §9) with a report
  note. Handlers, fuzzy matching, recents: verbatim.
- `src/chrome_view.rs` — transport `kbd` chips
  (§4.6). App-bar Save/Load/Undo/Redo get **no**
  chips (menu-hinted already).
- `src/rune_sec/icon_btn.rs` + `src/chrome_view.rs`
  (btn family) — focus adoption, wave 2 only, gated
  on the integrator decision in §6: live variants
  gain `.focusable(true)` + `.focus_ring(FOCUS_RING_W,
  FOCUS_RING_OFF, accent())`; inert twins gain
  nothing (they are not pressable; Rune's default
  FocusSpec is unfocusable, and `focusable`'s doc
  requires disabled controls to stay unfocused).

**A — adoption**

- `src/rune_sec/help_panel.rs` — `key_cap` deleted,
  `shortcut_row` → `design::kbd`; panel root →
  `design::panel_col` (today hand-rolled: fill
  `t.card`, border `control_track`, radius 8 —
  charter wants `card_border` + radius 12); section
  headers → kit `panel_title`/`section_label`.
  Features rows keep plain-text first columns (§3).
- Per-panel agents — **no keyboard work** beyond
  not breaking keys: panel roots, headers, rows and
  controls restyle only. Two named exceptions where
  a drag slider can gain Rune's slider keyboard
  (`on_step`, §6) as an *additional* handler, drag
  untouched, each needing its own brief line:
  - `src/rune_sec/layers_panel.rs` — opacity slider:
    `on_step` delta → the existing opacity step the
    drag/steppers already use, clamped [0.05, 1.0].
  - `src/rune_sec/scene_panel.rs` — weather amount/
    wind sliders, same pattern, if the panel agent's
    brief includes it; otherwise defer. No other
    sliders adopt `on_step` in Phase N.

**Integrator-only (no agent brief — charter: no
main.rs edits by agents)**

- `src/main.rs` AppSpec (line 776): decide
  `tab_visits_controls(true)` — see §6. This one
  flag is the gate on whether any adopted
  `.focusable(true)` button is reachable by Tab.
  Recommended: set it, together with the round-11
  behaviour pins below, in the F integration, not
  during A.

**V — verification & docs**

- `src/rune_sec/help_panel.rs` copy riders (do in
  the V help pass, not the A restyle, so the 54-row
  census moves once): H1 — add
  `("Ctrl+S", "Quick save")` to the Shortcuts
  section and delete the Features "Quick save" row
  (net 54 rows, census unchanged, text de-duplicated
  — the `no_row_text_duplicates_another` test
  requires the Features row to go); H2 — Delete row
  text → `("Delete", "Delete the selected stroke,
  particle event or segment")` (spot-check test at
  help_panel.rs:938 pins the old pair — update it in
  the same commit); H3 — add a `section_note` for
  the Shortcuts section or a panel-level note:
  "Single-letter keys, Space and the arrows are
  shortcuts only while no text field is focused."
  (exact wording is the V agent's, the fact is
  main.rs:432).
- `src/rune_sec/snap_tests.rs` round 11 — focus
  behaviour pins (see §6 for what is and isn't
  assertable): (a) Tab from the stage reaches the
  palette-independent first field in document order;
  (b) tap-focus the layers opacity slider, press
  Right — opacity steps, **frame does not**;
  (c) the Space pin: with a transport button
  click-focused, Space's effect is whatever the F
  integration decided (§6 hazard 1) — pin the
  decided behaviour, whichever way it lands;
  (d) palette rows for `undo`/`add-frame`/`go-home`
  present their chips (key presence of the chip
  element is not keyed — assert via the row's
  element subtree or accept a `find` on a new
  unkeyed chip is impossible and instead unit-test
  `command_hint` exhaustively in command_palette.rs;
  the honest split is: table by unit test, footer
  by tree dump in a headless scene).

## 6. Focus — what Rune can and cannot do (source-cited)

### Current shell state

**The shell uses none of Rune's focus builders.**
`grep -rn "focusable|tab_index|focus_group|
focus_ring|focus_highlight|default_button|on_step"
src/` returns zero hits. The only focusable elements
in the app are text inputs: the runtime's
`can_focus` (rune-src `crates/rune/src/runtime/
input/focus.rs`) accepts a node when
`focus.enabled || input.access.is_some()`, and only
kit/core text fields carry input access. Practical
consequences today:

- Tab cycles through text fields only. No button,
  stepper, slider, chip or stage element can hold
  keyboard focus.
- No focus ring ever draws on a control (there is
  nothing focusable to draw it on). The palette's
  query field is the one place focus is visible —
  caret + field frame — because it is a text input.
- Space and Enter always reach `main.rs::on_key`,
  because no focused element can consume them first.
  Today's global shortcuts are, in part, an accident
  of there being no focus system in play — which is
  why adoption below is staged and hazard-first.

### What Rune offers (pinned tree, all verified)

From `crates/rune/src/el.rs` (El builders) and
`crates/rune/src/el/focus.rs` (the `FocusSpec` they
fill):

- `.focusable(bool)` (el.rs:1065) — opt into
  keyboard + pointer focus; the doc adds "Disabled
  controls must set this false."
- `.tab_index(i32)` (el.rs) — positive values sort
  before document order, 0 is document order,
  negative skips Tab (element still click-focusable).
- `.focus_group()` (el.rs:1079) — arrow-key roving
  among focusable descendants, type-select by label
  prefix, Home/End jump; the *group* is one Tab stop
  (runtime `tab_order`, focus.rs:304, collapses a
  group to its focused/selected/first member).
- `.focus_selected(bool)`, `.reveal_selected(bool)`,
  `.focus_follows()` — group bookkeeping: which
  member is the stop, scroll-reveal, selection
  follows focus.
- `.focus_highlight(color)` (el.rs:1112) — a menu/
  list row's highlight painted while focused from
  keyboard **or** pointer (pointer hover focuses the
  row, NSMenu-style); the doc says use it *instead
  of* `hover` and `focus_ring` on such rows.
- `.focus_ring(w, offset, color)` (el.rs:1153),
  `.focus_within_ring(...)` (el.rs:1163, CSS
  `:focus-within` analogue), `.no_focus_ring()`
  (el.rs:1188).
- **A default ring exists**: per the `no_focus_ring`
  doc, a focusable element focused from the keyboard
  draws a default ring when neither it nor a child
  sets one — 3pt wide in `Cx::focus_color()`, just
  outside the border box, following corner radii.
  `Cx::focus_color()` (crates/rune/src/runtime/
  cx.rs:202) is the platform colour: AppKit's
  `keyboardFocusIndicatorColor`, "the accent at 50%
  elsewhere". `Cx::focus_ring(w, offset, color)`
  (cx.rs:161) lets a view override the default.
- `.default_button()` (el.rs:1123) — Return presses
  it from anywhere (unless the focused element takes
  Return itself); with several, the last shown wins.
- `.on_step(f)` / `.on_step_limit(f)` (el.rs:1132,
  1142) — NSSlider keyboard: Left/Down −1, Right/Up
  +1, PageDown/PageUp ∓10, Home/End to limits; makes
  the element focusable and gives it role `slider`.
- `.focus_return(key)` (el.rs) — focus returns to a
  keyed trigger when the element is removed (the
  palette/menu opener pattern).
- `.modal(click_outside, dismiss)` (el.rs) — traps
  Tab, background inert, Escape requests dismissal.
- **Focus-visible semantics** (focus.rs, `navigates`
  doc): the ring shows after *navigation* keys (Tab,
  arrows, Home/End, PageUp/PageDown) — "as CSS
  `:focus-visible`"; typing or clicking does not by
  itself raise the ring. Type-select raises it when
  it moves focus.
- **Tab order computation** (focus.rs:304): all
  `can_focus` nodes with `order >= 0`, groups
  collapsed to one stop, sorted by (positive order
  first, then document order).

### What Rune will NOT do for us (the honest limits)

- **Buttons are not Tab stops by default, even when
  focusable.** `focus_key`'s Tab branch (focus.rs,
  the `tab_order` retain): without Full Keyboard
  Access, Tab visits only text inputs, elements with
  their own `focus.key` handler, and list-role
  groups — "A control that owns its keyboard is a
  list of its own." The escape hatch is
  `AppSpec::tab_visits_controls(bool)`
  (crates/rune/src/app.rs:229; default `false`,
  app.rs:137; applied to the Runtime at window
  creation, desktop/window.rs:139). The shell's
  AppSpec (main.rs:776–782) does not set it. So:
  `.focusable(true)` alone buys click-focus and a
  ring after click-focus navigation, **not** a Tab
  order. The flag flip is one line but it is in
  main.rs — integrator territory, gated in §5.
- **No per-control focus styling in the Theme.**
  Rune's `Theme`/`tok` carry no focus colour; the
  default ring is the *system* colour via `Cx`,
  which panel builders (they receive `&Theme`, not
  `&Cx`) cannot read. Hence §5's proposal: explicit
  `.focus_ring(2.0, 2.0, design::accent())` at
  adoption sites. Mixing both (some controls
  explicit accent ring, others falling back to the
  system-colour default) is the failure mode to
  avoid — adoption must be family-wide per surface.
- **Focus navigation eats keys before the app sees
  them.** Key routing is 9 steps (docs/
  RUNE_API_AUDIT.md Q4): IME → default-button
  Return → focused field → **focus navigation** →
  window `on_key` → copy keys → scroll keys.
  `main.rs::on_key` is step 5. Three concrete
  hazards, in the order Phase N would meet them:
  1. **Space.** "A focused button presses on
     Space" (el.rs, `default_button` doc). The day
     any button can hold focus, Space stops being
     reliably Play/Pause — after clicking Play with
     the mouse, focus sits on it and Space re-presses
     it (harmless-looking), but after clicking
     **+ Frame**, Space would re-add frames instead
     of playing. This is standard desktop behaviour
     and still a behaviour change against the
     charter's "no handler changes" line in spirit:
     the F integration must choose — (a) accept it
     (document in help: Space presses the focused
     button), or (b) keep buttons click-focusable
     but out of Space's way is **not** an option
     Rune offers per-element (no "focusable but
     doesn't take Space" builder exists). This plan
     recommends (a) + the round-11 pin (§5 V), with
     transport buttons adopted **last**, after the
     pin exists. Deciding is the integrator's, at F
     integration, before A-wave panels adopt.
  2. **Arrows.** A focused `on_step` slider consumes
     Left/Right/PageUp/PageDown/Home/End
     (focus.rs `focus_key`: step handling precedes
     group handling, which precedes the app). Left/
     Right frame-stepping dies while a slider holds
     focus — correct NSSlider behaviour, and the
     reason §5 scopes `on_step` to two sliders and
     asks the help pass to note it. A `focus_group`
     with horizontal roving (segmented rows,
     workspace chips) likewise consumes Left/Right
     while focused: **do not focus-group the
     workspace chips or any segmented control whose
     neighbours are frame-step keys** in Phase N.
  3. **Escape.** If menus or the palette adopted
     `.modal(..)`, Escape would dismiss at the focus
     step and `on_key`'s menu-close branch would
     never run — two owners for one key. Phase N
     does **not** adopt `modal()` for dropdowns or
     the palette; their existing dismissal (scrim
     press, Escape in `on_key`/`palette_handle_key`)
     stays the only path.
- **Rings are not assertable in Runtime tests.**
  `Runtime::elements()`/`presented()` report rect +
  opacity, not ring state, and headless scenes
  cannot pre-focus a control (scenes drive state,
  not focus). So focus *visuals* verify by: unit
  tests on builders (the element carries
  focusable + ring spec — inspectable in the tree
  the builder returns only indirectly), behaviour
  pins (§5 V: keys cause the focused control's
  action, not the global one), and one human/
  headless screenshot pass. Anyone promising a
  pixel-pinned ring test is overpromising; the
  honest V deliverable is the behaviour pins plus a
  snap of the palette (whose field focus is real
  today) and, post-adoption, a clicked-then-Tabbed
  Runtime sequence asserted on *which element the
  next key acted on*.
- **Exiting elements cannot hold focus** (runtime
  `reachable` excludes exiting subtrees), and
  workspace-gated panels unmount on switch — focus
  does not survive a workspace switch. No Phase N
  design may promise focus restoration across
  workspaces; `focus_return` covers popups within
  one mounted tree only.

### Recommended adoption order (if the integrator takes §6 on)

1. F: kit `kbd` + menu/palette chips (no focus
   change at all — pure discoverability, zero
   routing risk).
2. F integration decision: `tab_visits_controls`
   + Space behaviour (a) — recorded in PHASE_N.md.
3. F: btn/icon_btn family focusable + explicit
   accent ring (family-wide, one commit).
4. A: list rows that are already pressable rows
   (figures, library, home, clips) get
   `focus_highlight` treatment only where the panel
   plan (P08) already restyles them — optional per
   panel, never groups in wave 1.
5. A exceptions: the two `on_step` sliders (§5).
6. V: behaviour pins + help copy (H1–H3) + a note
   in help that Tab now moves through controls
   (wording in the V pass, once the behaviour is
   pinned and true).

## 6. Summary for the integrator

- Discoverability gaps, ranked: **P1** palette rows
  hide 17 real keys (fix: `command_hint` + chips) >
  **D1** transport keys help-only (fix: 3 chips in
  chrome) > **H1/H2/H3** help copy (Ctrl+S row,
  Delete cascade text, focus caveat) > **P2** palette
  footer chips > **D2** C key (covered by help;
  no further surface proposed) > **M1** menu
  keyboard open/navigation (real gap, Rune-capable
  via `focus_group` + role `menu`, but behaviour
  wiring with routing hazards — propose as a
  post-Phase-N scoped item, not a restyle).
- The hint honesty discipline (menus.rs) is a
  feature: Phase N styles the 5 real menu hints and
  24 real help keys, adds 17 real palette hints, and
  invents none.
- Focus is a **staged, gated** adoption: Rune's
  model is complete (focusable, tab order, groups,
  rings, slider keys, default button — all cited
  above), but the shell uses none of it, Tab won't
  reach buttons without the integrator's
  `tab_visits_controls` line in main.rs, and Space/
  arrows change meaning the moment controls can
  hold focus. The plan's order (§6) puts every
  zero-risk chip first and the focus switch behind
  an explicit decision + behaviour pins.
