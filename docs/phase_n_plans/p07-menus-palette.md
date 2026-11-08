# P07 — Menus, Popovers & Palette spec (Phase N, planning)

Scope: `src/rune_sec/menus.rs`, `src/rune_sec/command_palette.rs`,
`src/rune_sec/settings_popover.rs`, `src/rune_sec/help_panel.rs`
(the help panel *as a panel* — its copy belongs to the V-wave
help pass; this spec covers only its card, rows and key caps).

Charter basis: charter §9 (menus / popovers / palette recipe),
§10 (empty states), §11 (kbd chips), §1–§3 (card, header,
section labels), §5 (list rows), §6 (steppers), §7 (toggles).
All metrics below are the charter's; "today" values were read
from the files above on 2026-11-08 (Phase M tree, 733 keys).

**The popover recipe (charter §9), restated once:** surface =
`t.card` fill, 1.0 `t.card_border`, radius 12 (LG), padding 6;
item rows 28 high, px 8, radius 6, hover `t.hover`; shortcut
hints as `design::kbd` chips; separators are kit dividers.

Nothing in this spec changes a handler, a status string, an
i18n key, an offer condition or a shortcut. Handlers move
verbatim (charter rule). Where a kit builder cannot carry an
existing key, this spec says so and prescribes a keyed
wrapper instead — the gate diffs key sets, and a dropped key
is a failed phase, not a style choice.

---

## 1. Menus — `menus.rs`

### 1.1 What exists today

- `menu_bar`: `row` keyed `menu-bar`, gap 4; four `btn`
  triggers keyed `menu-file` / `menu-edit` / `menu-view` /
  `menu-help`, active while their menu is open.
- `menu_scrim`: keyed `menu-scrim`; an empty `el()` when
  closed, an absolute full-window transparent press target
  when open (press closes). Mounted as a root child
  immediately before the dropdown (see §5).
- `menu_dropdown`: `col` keyed `{menu}-dropdown`
  (`menu-file-dropdown` … `menu-help-dropdown`), `min_w`
  190, padding 4, gap 2, radius 8, fill `t.card`, border
  1.0 `t.card_border`, `items_stretch`, returned with
  `.anchored(MENU_KEYS[open], Side::Bottom, Align::Start,
  4.0)` as the outermost modifier.
- Items are built three ways, all px 10 / py 5 / radius 6
  with the button family's fills (inactive `t.card`, hover
  `control_track`, press `control_off`, active = accent +
  white): plain `chrome_view::btn`, `hinted_item` (label +
  grow spacer + right-aligned hint text), `item_icon`
  (12 pt glyph + label in an inner row, gap 6; hint the
  same). `item()` dispatches icon → hinted → plain.
- `separator(key, t)`: `el()` h 1, fill `t.separator`,
  keyed `menu-file-sep`, `menu-edit-sep`, `menu-view-sep`,
  `menu-view-sep2`, `menu-help-sep`.
- Shortcut hints exist for exactly five items and are
  pinned by the `shortcut_hints_name_only_real_bindings`
  test: `save` Ctrl+S, `undo` Ctrl+Z, `redo` Ctrl+Shift+Z,
  `onion` O, `menu-palette` Ctrl+K. Everything else is
  hint-less *by design* (no invented hints — the Delete
  key's cascade is explicitly not a per-item binding).
- Conditional items appear only while their action can
  run: `edit-delete-stroke`, `edit-delete-particle`
  (selection exists), `edit-swap-next` (a next frame
  exists), `edit-mute-clip` (an audio target exists; built
  via `item_icon` directly because its glyph is
  state-dependent — `mute_item_parts`), and the File ▸
  Open recent block (`file-recent-{i}`, ≤ 5, clock glyph,
  preceded by `menu-file-sep`; no block at all when there
  are no recents).

### 1.2 Recipe applied

**Dropdown surface** (`menu_dropdown`):
- radius 8 → **12**; padding 4 → **6**; gap stays 2.
  Fill and border are already the recipe's (`t.card`,
  1.0 `t.card_border`). `min_w` 190 stays.
- Do **not** swap the root for `design::panel_col`: its
  padding 12 / gap 8 is the *panel* recipe and is wrong
  for a menu. The dropdown keeps its own `col`; only the
  numbers move.
- `.anchored(trigger, Bottom, Start, 4.0)` stays the
  outermost call on the returned element, unchanged.

**Item rows** (`item_icon`, `hinted_item`; plain `btn`
items inherit chrome metrics — see the dependency note):
- Explicit height: add `.h(28.0)` to the `item_icon` and
  `hinted_item` roots (today py 5 + LABEL lands ≈ 27;
  the charter fixes the row at 28). `center_y` already
  set. px 10 → **8**. Radius stays 6. Fills unchanged
  (button family, charter §4).
- **Icon alignment column.** Today an icon item's label
  starts 18 pt right of a plain item's label (12 glyph +
  6 gap), so labels zig-zag within one menu (File mixes
  all-icon items; Edit mixes icon items with the plain
  conditional ones). Prescription: every item root —
  icon, hinted and (via the menus-local builders) plain —
  reserves the same leading slot: a fixed 18-wide slot
  holding either the 12 pt glyph (centred-left) or an
  empty spacer of the same width. Concretely: `hinted_item`
  gains the slot (empty), `item_icon` keeps its glyph in
  the slot, and the two icon-less, hint-less items
  (`view-fit`, `view-centre`) are built by a menus-local
  plain builder with the empty slot rather than bare
  `btn` — same fills/padding as `btn`, handler wrapped in
  `closing(...)` exactly as `item()` does today. Labels
  then share one left edge in every menu.
- **Shortcut hints become kbd chips.** In `hinted_item`
  and `item_icon`, replace the right-edge hint `text`
  with `design::kbd(hint, t)` — except while the item is
  **active** (accent fill): a `control_off` chip on accent
  is low-contrast, so active items keep the current
  treatment (hint as text, white at 75% alpha). Inactive:
  chip. The five hint strings do not change; no new hints
  are added (the pinning test still passes — it tests
  `shortcut_hint`, which is untouched).
- **Selected (toggled-on) state** stays the button
  family's active style (accent fill, white label, white
  glyph): `onion`, `view-guides`, `view-guides-frustum`,
  `view-guides-safe`, `view-toggle-left`,
  `view-toggle-right`, `help`. No checkmark column is
  added — the accent fill is the charter's selected
  signal for buttons (§4), and the reserved icon slot
  (§ above) already keeps geometry stable between on and
  off items.
- **Separators keep their keys.** `design::divider(t)`
  takes no key, so a wholesale swap would drop five keys
  (`menu-file-sep`, `menu-edit-sep`, `menu-view-sep`,
  `menu-view-sep2`, `menu-help-sep`) and fail the gate.
  Prescription: keep the local `separator(key, t)` as a
  thin keyed wrapper restyled to the divider look — h 1,
  fill `t.separator` (both already true) — and give it
  vertical breathing room inside the gap-2 list:
  wrap as `col().key(key).py(4.0).child(design::divider(t))`
  or add `.my(4.0)` if the landed `El` API supports margin
  on the keyed element itself; either way the key stays on
  an element in the dropdown and the line reads as a kit
  divider. The Open-recent block keeps using the same
  wrapper (`menu-file-sep`).
- **Triggers** (`menu_bar`): unchanged. `btn` already
  carries the charter button metrics; active-while-open
  is the correct state. The bar itself is app-bar
  territory (charter §15, chrome plan), not this spec.

### 1.3 Keys at risk — menus

| Key(s) | Where | Risk |
|---|---|---|
| `menu-file-dropdown`, `menu-edit-dropdown`, `menu-view-dropdown`, `menu-help-dropdown` | dropdown root | Must stay on the root `col` that carries `.anchored(...)` |
| `menu-scrim` | scrim | Root returned by `menu_scrim`; see §5 |
| `menu-bar`, `menu-file/edit/view/help` | bar + triggers | Untouched, listed for completeness |
| `menu-file-sep`, `menu-edit-sep`, `menu-view-sep`, `menu-view-sep2`, `menu-help-sep` | separators | `design::divider` is unkeyed — keyed wrapper required (§1.2) |
| `save`, `load`, `export`, `undo`, `redo`, `copy-frame`, `paste-frame`, `view-fit`, `view-centre`, `onion`, `view-guides`, `view-guides-frustum`, `view-guides-safe`, `view-speed`, `view-toggle-left`, `view-toggle-right`, `help`, `menu-palette` | static items | Key stays on each item's pressable root (all three builders already do this) |
| `edit-delete-stroke`, `edit-delete-particle`, `edit-swap-next`, `edit-mute-clip` | conditional items | Same; offer conditions must not change |
| `file-recent-{i}` (dynamic, ≤ 5) | Open recent | Prefix-resolved clock glyph; keep the dynamic key format byte-identical |

### 1.4 Work list — menus.rs

1. `menu_dropdown`: radius 12, padding 6 (fill/border/
   min_w/anchor untouched).
2. `item_icon` + `hinted_item`: px 8, `.h(28.0)`; hint
   slot → `design::kbd` when inactive, white-75% text
   when active.
3. Add the fixed 18-wide leading slot to `hinted_item`;
   build `view-fit` / `view-centre` through a menus-local
   plain builder with the empty slot (fills, radius,
   `closing(...)` wrap identical to `btn` items today).
4. `separator`: restyle as keyed divider wrapper
   (key preserved, divider child, py 4 breathing room).
5. No changes to `menu_icon`, `shortcut_hint`,
   `mute_item_parts`, `recent_items`, any handler, or any
   offer condition. Existing tests
   (`shortcut_hints_name_only_real_bindings`,
   `menu_icons_name_real_glyphs…`, dropdown builders)
   must pass unmodified.

---

## 2. Command palette — `command_palette.rs`

### 2.1 What exists today

- `PaletteState { open, query: TextEdit, selected,
  recent }`; `palette_handle_key` routes Ctrl+K (open),
  ↑/↓ (wrap), Enter (run), Esc (close) at app level; the
  field's own handler covers ↑/↓/Esc and Enter via
  `on_submit`. Selection indexes result rows only —
  headings are never selectable.
- `palette_view` root: `col` keyed `command-palette`,
  w 540, radius 12, `clip_rounded(12.0)`, fill `t.card`,
  border 1.0 `Color::WHITE.alpha(0.10)`.
- Header row h 52, px 16, gap 10: a `⌕` text glyph
  (TITLE, `text_secondary`) + the query field keyed
  `palette-query` (kit `text_input`, BODY lh 26,
  placeholder "Type a command…", `on_input_change`
  resets `selected` to 0).
- Results: `col` padding 6, gap 2. `section_heading`:
  row h 22, px 10, LABEL `text_secondary` ("Recent" /
  "All commands"). Result rows keyed `cmd-{key}`:
  h 34, px 10, radius 8, gap 8, title BODY growing left,
  group LABEL right; selected row = accent at 0.85 alpha
  fill + white text; unselected = transparent fill,
  hover `control_track`.
- Empty results: a separator line + row h 36, px 16,
  "No commands match" in BODY `text_secondary`.
- Footer: row h 26, px 16, one LABEL `text_secondary`
  string: "↑↓ navigate · Enter run · Esc close". The
  footer renders in both the results and empty states.
- Mounting: **inline**, as the centre column's first
  child while `s.palette.open` (main.rs) — not a root
  overlay. See §5.

### 2.2 Recipe applied

- **Border token.** Border `Color::WHITE.alpha(0.10)` →
  `t.card_border` (the one literal in the shell's
  surfaces that bypasses the theme token). Radius 12,
  `clip_rounded(12.0)`, fill `t.card`, width 540: already
  the recipe; unchanged.
- **Result rows.** px 10 → **8** (title text then starts
  at 6 padding + 8 = 14 from the panel edge). Height 34
  stays — charter §5 sets a *minimum* of 28 for list
  rows; the launcher's 34 is a deliberate, denser-than-
  minimum choice and shrinking it would change the
  palette's feel for no consistency gain (panel list rows
  elsewhere adopt 28+; the palette is not a panel list).
  Radius 8 (= `ROW_RADIUS`) stays. Gap 8 stays.
- **Selected row — documented deviation.** Charter §5's
  selected style (accent at 16% alpha) is written for
  persistent selection in panel lists. The palette's
  highlight is transient keyboard focus in a launcher;
  every professional launcher (Spotlight, Raycast, VS
  Code) uses a strong highlight there. Prescription:
  keep accent at 0.85 alpha + white title/group text
  exactly as today. This is the spec's one deliberate
  deviation from a charter number; it is visual only,
  and the alternative (16% fill) remains a one-line
  change if the F-wave agent's review disagrees.
- **No per-command icons.** The 108 commands have no
  icon map and inventing one is out of scope (and would
  collide with the menus' curated map discipline).
  Rows keep title-left / group-right; group label stays
  LABEL `text_secondary` (white on the selected row).
- **Section headings.** Retire the local
  `section_heading` onto `design::section_label(label,
  t)` inside the existing row shell: h 22 stays, px 10 →
  **8** so heading text aligns with row titles (14 from
  the panel edge). Headings stay unkeyed, unpressable
  and outside the selection indexing — `display_commands`
  and the `recent_rows` split are untouched.
- **Search header.** Keep h 52 / px 16 / gap 10 and the
  field exactly as is (charter: text-field internals are
  rune-kit; style the frame, not the editor). One visual
  swap, optional but recommended: the `⌕` text glyph →
  `icons::icon_el("magnifying-glass", 16.0,
  t.text_secondary)` — the glyph the menus already use
  for this meaning, in the curated set. The text node is
  unkeyed; no key risk either way. Placeholder text and
  all field handlers unchanged.
- **Empty state (charter §10).** Replace the
  separator + h-36 text row with `design::empty_state(
  "magnifying-glass", "No commands match", t)`: centred
  20 pt icon + LABEL text, both `text_secondary`, py 12.
  The message string stays byte-identical. Keep the
  foot
...[truncated 14737 chars]