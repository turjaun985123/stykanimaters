# P05 — Buttons & Control States Spec

Phase N planning agent P5. Scope: every button and
button-like pressable in the Rune shell — the family
(`chrome_view::btn`, `icon_btn`, `icon_only_btn` and
their inert twins), every local helper that rebuilds
one, and every hand-rolled pressable row/chip/swatch
that behaves like a button. Method: source read of
`src/chrome_view.rs`, `src/rune_sec/icon_btn.rs`,
`src/rune_sec/frame_ops.rs` and a tree-wide grep for
`on_press` / `*btn*` helpers (2026-11-08, Phase M
baseline baf5441). No code was changed.

The charter (§4) already names the family and its
metrics. This spec (a) catalogues what actually
exists, state by state, (b) states the one normative
state table the F wave builds to, (c) gives the
migration table for every non-family button, and
(d) lists the tests that pin button behaviour.

---

## 1. The family today — full catalogue

### 1.1 `chrome_view::btn` — src/chrome_view.rs:38

The shell's text button. ~227 call sites tree-wide
(grep count, including indirect uses).

```rust
pub fn btn(key: &str, label: &str, active: bool, t: &Theme,
           f: impl Fn(&mut State) + 'static) -> El
```

| aspect | value |
|---|---|
| root | `el()` (single text child) |
| padding | px 10.0 / py 5.0 |
| radius | 6.0 |
| font | `type_::LABEL` (12/500) |
| rest fill | active → `tok::accent(0, true)`; else `t.card` |
| hover fill | active → accent; else `t.control_track` |
| press fill | active → accent; else `t.control_off` |
| ink | active → `Color::WHITE`; else `t.text` |
| handler | `.on_press(f)` on the root |
| states | normal, hover, press, active. **No disabled/inert form exists.** |

### 1.2 `icon_btn` — src/rune_sec/icon_btn.rs:155

~114 call sites. A `row()` root: glyph + label.

```rust
pub fn icon_btn(key: &str, icon: &str, label: &str, active: bool,
                t: &Theme, on_press: impl Fn(&mut State) + 'static) -> El
```

Identical box and fills to `btn` (the colour choice is
factored into `glyph_color` :85, `fill_color` :94,
`hover_color` :123, `press_color` :132 — all documented
as mirroring btn value for value). Adds: glyph
`ICON_SIZE` 13.0 (:68), gap `ICON_LABEL_GAP` 6.0 (:75),
`.center_y()`. Fallback rule: unknown icon name →
returns plain `btn(...)` — same key, label, handler.

### 1.3 `icon_only_btn` — src/rune_sec/icon_btn.rs:230

~35 call sites. Square, caller-fixed `size` × `size`,
**no padding** (the square contract), radius 6.0, same
three fills, glyph `glyph_size_for(size)` =
`min(13.0, size)` (:141), centred.

### 1.4 The inert twins — icon_btn.rs:199 / :267

~19 + ~5 call sites. Built in Phase I §11 to replace
exactly the local muted wrappers this spec retires.

| aspect | `icon_btn_inert` / `icon_only_btn_inert` |
|---|---|
| box | byte-identical metrics to the live twin |
| fill | `fill_color_inert(t)` = `t.card` (:118) |
| ink/glyph | `glyph_color_muted(t)` = `t.text_secondary` (:110) |
| hover / press fills | **none attached** |
| handler | **none — a plain element, not a control** |

`icon_btn_inert`'s unknown-icon fallback hand-rolls an
`el()` clone of btn's box in the inert colours
(icon_btn.rs:202-209) — remember this branch; §5 of
this spec promotes it to a named family member.

### 1.5 What the family does NOT have today

- **No text-only inert button.** A panel with a plain
  text button to disable must either keep it live,
  switch it to an icon button, or grow a local helper.
  This single gap is the root cause of `fbtn`'s muted
  branch, `kbtn` and `pbtn` (§2).
- **No danger member.** Two local copies exist (§2.4).
- **No compact member.** One local copy exists (§2.5).
- **No focus state.** No button builder anywhere in
  the shell sets a focus visual; keyboard handling is
  the shell's `on_key` routing and rune-kit text
  fields. Recorded as an observed gap; Phase N does
  not invent one (charter silent, Rune El usage in
  this codebase has no focus-fill precedent).
- **No disabled-with-handler state** — see the rule
  in §4, which names the one legitimate exception.

---

## 2. Every non-family button helper — catalogue

### 2.1 `fbtn` — src/rune_sec/frame_ops.rs:517 (16 sites, all in-file)

```rust
fn fbtn(key, label, active: bool, muted: bool, t, f) -> El
```

btn's exact box (px 10 / py 5 / r 6 / LABEL). Three
colour branches:

| branch | fill | hover | press | ink | handler |
|---|---|---|---|---|---|
| active | accent | accent | accent | WHITE | live |
| **muted** | `t.control_off` | `t.control_off` | `t.control_off` | `t.text_secondary` | **live** |
| normal | `t.card` | `t.control_track` | `t.control_off` | `t.text` | live |

The muted branch is a *live-but-muted* state: the
press still dispatches and the op answers with a
refusal status (e.g. shift-back on frame 0 →
"First frame: cannot shift it earlier."). It is NOT
the inert state: different fill (`control_off`, the
family's *press* colour, used as a resting fill) and
the handler survives.

### 2.2 `kbtn` — src/rune_sec/keyframe_tools.rs:1001 (3 sites)

```rust
fn kbtn(key, label, enabled: bool, t, f) -> El
```

Enabled: btn's inactive state exactly (card /
control_track / control_off, `t.text`). Disabled:
fill `t.control_off`, ink `t.text_secondary`, **no
hover/press, no handler**. No `active` state at all.
Same box metrics as btn.

### 2.3 `pbtn` — src/rune_sec/pose_tools.rs:1152 (9 sites)

Value-for-value the same builder as `kbtn` (enabled →
btn-inactive; disabled → `control_off` fill,
secondary ink, no handler), differing only in taking
`key: String` (its keys are `format!`-built). Two
files independently reinvented the same disabled
button — the clearest drift specimen in the shell.

Its sibling `pibtn` (pose_tools.rs:1193) is a pure
dispatch: live → `icon_btn`, inert → `icon_btn_inert`.
It is already family; the wrapper adds nothing.

### 2.4 `danger_btn` — TWO copies, different signatures

- src/rune_sec/inspector_figure.rs:807 —
  `danger_btn(key, label, f)`: **no `Theme` parameter
  at all**; colours are hard-coded hex literals
  `0xF16F78` (the 2.0 palette's C_ERROR) and
  `0xD85560` for hover/press.
- src/rune_sec/inspector_segment.rs:635 —
  `danger_btn(key, icon, label, f)`: same hex pair,
  plus a leading glyph at **12.0** (family glyph is
  13.0) in a row with gap 6.0. Its doc comment states
  the danger styling is deliberate and must not be
  swapped for standard chrome.

Both: btn box metrics, fill always the red (an
always-"active" look), ink WHITE, no inactive form,
no inert form, one call site each (Delete figure /
Delete segment).

### 2.5 `row_toggle` — src/rune_sec/figures_panel.rs:~383 (5 sites)

A compact toggle: px **6.0** / py **3.0** / radius
**5.0**, otherwise btn's state table verbatim
(accent when active). Exists because five toggles
must fit one sidebar row — the only place the
family's metrics were judged too wide.

### 2.6 Menu rows — src/rune_sec/menus.rs:224 / ~340

`hinted_item` (:224) and `item_icon` (:340) are btn
clones rebuilt as rows: same px 10 / py 5 / r 6 and
the same fill triad, but gap **12.0**, a growing
spacer, and a trailing shortcut hint (LABEL,
`t.text_secondary`; `Color::WHITE.alpha(0.75)` while
active). `item_icon` leads with the glyph at **12.0**
(family: 13.0), inner gap 6.0. The whole row is the
hit area, hint included. These are menu items, not
panel buttons — the charter gives them their own
metrics (§9: rows 28 high, px 8, radius 6, `kbd`
hint chips), so they migrate to the kit menu row,
not to `icon_btn` (§5).

### 2.7 Segmented / picker chips — three more btn clones

- `cap_chip` — src/rune_sec/inspector_style.rs:~643.
  btn's box and fills **except no `.press()` fill at
  all** — a press gives no pressed feedback. Keys
  `style-cap-*`.
- `preset_chip` — inspector_style.rs:~665. px **8.0**
  variant, leading 12×12 colour swatch (r 3), also
  **no press fill**. Keys `style-preset-{i}`.
- `picker_cell` — src/rune_sec/easing_panel.rs:~203.
  btn's fill triad (press fill present), px **4.0**,
  content is a canvas-over-label column. Keys
  `ease-pick-*`.

### 2.8 Pressable list rows (button behaviour, row shape)

- command palette command row —
  src/rune_sec/command_palette.rs:1594: h **34.0**,
  px 10, r **8**, font BODY; selected fill =
  `accent.alpha(0.85)` (neither solid accent nor the
  charter's 16% selected fill), hover `control_track`
  or the same 0.85, **no press fill**.
- audio clip row — src/rune_sec/audio_panel.rs:~722:
  px 8 / py 5 / r 6, BODY; selected fill =
  `t.control_track`, hover swaps card↔`control_off`
  by selection, **no press fill**.
- movieclip row — src/rune_sec/clips_panel.rs:~652:
  px 8 / py **4** / r 6; selected `control_track`,
  hover `control_track`, **no press fill**; the
  handler writes thread-local panel state (Phase
  G §35) — handlers move verbatim, as always.
- figure card — src/rune_sec/figures_panel.rs:536:
  the whole figure row is a press target
  (`select_figure`).

Three different "selected row" fills exist today
(accent@0.85, control_track, card.mix(accent, 0.35)
on timeline loop frames) — the charter §5 fixes one:
accent at 16% (`design::selected_fill()`).

### 2.9 Swatches — a pressable class of their own

Not family buttons, but pressables with a selected
state, and their metrics have drifted furthest:

| site | size | radius | selected marker |
|---|---|---|---|
| inspector_figure.rs:~824 `swatch` | 22×22 | 6 | `border(2.0, WHITE)` |
| inspector_segment.rs:~657 `swatch` | 22×22 | 6 | `border(2.0, WHITE)` |
| fx_clips.rs:~1004 `swatch` | 18×18 | 5 | `outline(2.0, 1.0, WHITE)` |
| inspector_style.rs gradient swatch | 18×18 | — | accent frame |
| scene_panel.rs:895 / :923 bg swatch | 26 slot + 22 core | 8 / 6 | accent fill behind core |
| particles_panel.rs:539, text_tool.rs:1003 | colour wells | — | panel-local |

All use hover = press = own fill (no state change on
hover/press). A white selected border is invisible
on a white swatch — a live defect the unification in
§5 fixes by moving the marker to the accent.

### 2.10 Timeline frame chips — timeline_marks.rs:842

Fixed geometry: `CHIP_W` 30.0 × `CHIP_H` 24.0
(:268/:270), radius 6, current = accent/WHITE,
loop-range = `t.card.mix(accent, 0.35)`, range-
selected = `outline(2.0, 1.0, WHITE on current /
accent otherwise)`; hover only (no press fill —
presses are the row's `on_drag` gesture, not per-chip
`on_press`). The marker cells (:866) are transparent
pressable glyph wrappers, not buttons. **Charter
§13 pins chip keys and geometry constants** — this
class stays hand-built in timeline_marks (§5).

### 2.11 Already family — no action

`workspaces.rs` chips (plain `btn`, :452),
`playback_controls.rs` (17 `btn`/`icon_btn` sites),
`camera_panel::overlay` (:986, five `btn`s — the
container becomes the charter §14 floating pill in
the stage foundation work, the buttons stay family),
`settings_popover` (`btn` throughout; its private
`language_button` :563 / `workspace_button` :574 are
one-line `btn` dispatches, not re-implementations),
`figure_add::entry_button` (:160, a `btn` dispatch
with dismiss-on-pick), `draw_tools`, `home_panel`,
`layers_panel`, `library_panel`, `toasts` — all
route through `btn` / `icon_btn` / `icon_only_btn`
already (~380 family call sites in total vs ~50
behind local helpers).

---

## 3. Inconsistencies — the numbered list

1. **Three different disabled visuals.** Inert twins:
   `t.card` fill + secondary ink. `kbtn`/`pbtn`
   disabled: `t.control_off` fill + secondary ink.
   `fbtn` muted: `t.control_off` everywhere + a live
   handler. `control_off` is the family's *press*
   colour — a resting disabled button currently reads
   as permanently pressed in two of the three forms.
2. **Two opposite disabled semantics.** Handler
   dropped (`kbtn`, `pbtn`, inert twins) vs handler
   kept to deliver a refusal status (`fbtn` muted).
   Both are legitimate; today nothing names or
   separates them.
3. **`btn` has no inert twin** — the gap that caused
   1–2. (`icon_btn_inert`'s fallback branch already
   contains the exact element, unnamed.)
4. **Missing press feedback.** `cap_chip`,
   `preset_chip`, the palette row and both list rows
   attach no `.press()` fill; the rest of the shell
   does. Pressing them feels dead.
5. **Glyph size drift.** Family 13.0; menu items and
   segment `danger_btn` 12.0.
6. **Padding/radius drift.** Family px10/py5/r6;
   `row_toggle` 6/3/5; `preset_chip` px8; `picker_cell`
   px4; palette row px10/r8/h34; list rows py4–5.
7. **Selected-row fill drift** (§2.8): accent@0.85 vs
   control_track vs card.mix(accent, 0.35).
8. **Danger colours bypass the theme.** Hex literals
   in two files, one helper without a `Theme`
   parameter; the pair is duplicated verbatim.
9. **Swatch selected-marker drift** (§2.9), including
   the invisible white-on-white case.
10. **Duplicate implementations of the same builder.**
    `kbtn` ≡ `pbtn`; `danger_btn` ×2; menu rows ≅
    `icon_btn` + hint. Each duplicate is a place a
    future restyle silently misses.

---

## 4. The disabled-state rule (normative)

> **A control whose op cannot run from the current
> state is rendered in the inert visual — resting
> `t.card` fill, `t.text_secondary` ink/glyph, no
> hover fill, no press fill — in exactly two forms:**
>
> - **Inert (default): no press handler at all.**
>   Use when the op has nothing to say: paste with an
>   empty clipboard, a 3D op outside a session, swap
>   at the timeline's end. This is the inert twins'
>   existing discipline (Phase I §11) and becomes the
>   family's.
> - **Muted-live (exception): the handler stays, and
>   the press's only effect is a refusal status.**
>   Use only where that refusal is the designed
>   feedback and a test pins the status string
>   (frame_ops' shift/duration gates are the standing
>   example). The visual is byte-identical to inert —
>   a user cannot tell the two apart until the status
>   line answers.
>
> **Never** a third look, never `control_off` as a
> resting fill, never a live handler behind full
> normal styling (a button that looks armed and only
> refuses), and never a handler that silently no-ops.

Migration test for every gated button: *does the
current code drop the handler, or keep it for a
pinned refusal status?* The answer picks the form;
the visual does not change either way.

---

## 5. The one button family — the spec

All members live in `src/chrome_view.rs` (text forms)
and `src/rune_sec/icon_btn.rs` (icon forms), styled
from the shared state table. F-wave foundation work;
adoption agents only call these.

### 5.1 State table (normative, all members)

| state | fill | ink / glyph | hover | press | handler |
|---|---|---|---|---|---|
| normal | `t.card` | `t.text` | `t.control_track` | `t.control_off` | yes |
| active (toggle on) | `tok::accent(0, true)` = `design::accent()` | WHITE | accent | accent | yes |
| inert | `t.card` | `t.text_secondary` | — | — | **no** |
| muted-live | as inert | as inert | — | — | yes (refusal status only) |
| danger | `0xF16F78` | WHITE | `0xD85560` | `0xD85560` | yes |

The danger pair becomes two named constants in
chrome_view.rs (`DANGER`, `DANGER_DEEP`) — the values
do not change, the duplication and the theme bypass
do (the helpers take `t: &Theme` for signature
uniformity even though today's pair is theme-free).

### 5.2 Members

| member | where | signature delta vs today | metrics |
|---|---|---|---|
| `btn` | chrome_view.rs:38 | unchanged | px10 py5 r6, LABEL |
| `btn_inert` | chrome_view.rs (**new**) | extracted from `icon_btn_inert`'s fallback branch (icon_btn.rs:202-209) | btn box, inert state |
| `btn_muted_live` | chrome_view.rs (**new**, thin) | `btn_inert`'s styling + `.on_press(f)` | as inert |
| `icon_btn` | icon_btn.rs:155 | unchanged | btn box + glyph 13, gap 6 |
| `icon_btn_inert` | icon_btn.rs:199 | fallback branch delegates to `btn_inert` | as today |
| `icon_only_btn` | icon_btn.rs:230 | unchanged | size², r6, glyph min(13, size) |
| `icon_only_btn_inert` | icon_btn.rs:267 | unchanged | as today |
| `danger_btn` | chrome_view.rs (**new**) | figure copy's shape + `t` param | btn box, danger state |
| `danger_icon_btn` | chrome_view.rs (**new**) | segment copy; glyph 12 → **13**, gap 6 | btn box, danger state |
| `chip_btn` | chrome_view.rs (**new**) | content-flexible compact toggle (see 5.3) | px6 py3 r5, LABEL |

`icon_btn` with an unknown icon already degrades to
`btn`; with `btn_inert` named, `icon_btn_inert`
degrades to it instead of a private clone — one
implementation of the inert text box.

### 5.3 `chip_btn` — the compact content chip

One new member absorbs `row_toggle`, `cap_chip`,
`preset_chip` and `picker_cell`:

```rust
pub fn chip_btn(key: &str, active: bool, t: &Theme,
                f: impl Fn(&mut State) + 'static,
                content: El) -> El
```

- Metrics px 6 / py 3 / radius 5 (row_toggle's, the
  only compact metrics in the shell), state table
  §5.1 (including the press fill cap/preset chips
  lack today).
- Content is caller-built: a text run (row_toggle,
  cap_chip), a swatch + label row (preset_chip), a
  canvas + label column (picker_cell). Ink colour for
  text content follows the state table — callers pass
  content already coloured via the family's
  `glyph_color(active, t)` (icon_btn.rs:85, re-export
  or mirror in chrome_view).
- `picker_cell`'s px 4 → 6 is the one metric change;
  its canvas (`PICK_W`/`PICK_H`) and keys do not move.

### 5.4 Explicit non-members (stay local, restyled by their own plans)

- **Menu rows** (`hinted_item`, `item_icon`) → the
  kit menu row of charter §9 (28 high, px 8, r 6,
  hover `t.hover`, hint as `design::kbd`). Owned by
  the menus foundation agent, not this family —
  a menu item is a list row with a hint, not a
  button with a label.
- **List rows** (palette command row, audio clip row,
  clip row, figure card) → `design::list_row`
  shells; selected fill becomes
  `design::selected_fill()` (accent 16%), retiring
  inconsistency 7. The palette row's height follows
  charter §9 (28) at adoption; its BODY font stays.
- **Swatches** → one swatch discipline, kept
  per-panel (the wells carry panel colour data):
  22×22, radius 6, hover = press = own fill,
  selected = `outline(2.0, 1.0, accent)` — visible
  on every well colour, retiring the white-border
  forms (inconsistency 9). Scene bg swatches drop
  the nested 26/22 construction for the single well.
- **Timeline frame chips + marker cells** → stay in
  timeline_marks (charter §13 pins their geometry);
  no migration, no metric change.
- **Stepper −/+ buttons** → already `btn` /
  `icon_only_btn` at every site; the readout between
  them becomes `design::stepper_value` under the
  stepper spec (P-plan for steppers), not this one.

---

## 6. Migration table — every non-family button

Handlers, keys and status strings move verbatim in
every row (charter rule). "Form" = which §4 form the
gated sites take.

| # | site (file:line) | sites | → family member | notes |
|---|---|---|---|---|
| 1 | frame_ops.rs:517 `fbtn` | 16 | live → `btn`; muted → `btn_muted_live` | muted sites keep handlers: their refusal statuses are the designed feedback (shift/duration gates). Fill changes `control_off` → `t.card` (§4). |
| 2 | keyframe_tools.rs:1001 `kbtn` | 3 | enabled → `btn`; disabled → `btn_inert` | kbtn drops the handler when disabled → inert form. Fill changes `control_off` → `t.card`. |
| 3 | pose_tools.rs:1152 `pbtn` | 9 | enabled → `btn`; disabled → `btn_inert` | key is `String` at call sites; pass `&key`. Same fill change as #2. |
| 4 | pose_tools.rs:1193 `pibtn` | ~7 | **delete the wrapper**; sites call `icon_btn` / `icon_btn_inert` directly | already a pure dispatch; gates unchanged. |
| 5 | three_d_panel.rs:673 `ictl` | in-file | **delete the wrapper**; sites call `icon_btn` / `icon_btn_inert` directly | pure dispatch incl. `active` passthrough; `dim` gates unchanged. Its pinning test (§7) is rewritten to the direct calls. |
| 6 | inspector_figure.rs:807 `danger_btn` | 1 | chrome `danger_btn` | gains `t: &Theme` param; hex literals become the family constants; look unchanged. |
| 7 | inspector_segment.rs:635 `danger_btn` | 1 | chrome `danger_icon_btn` | glyph 12 → 13; gap already 6. Deliberate danger styling preserved (its doc comment's demand). |
| 8 | figures_panel.rs:~383 `row_toggle` | 5 | `chip_btn` (text content) | metrics already chip metrics; only the builder changes. |
| 9 | inspector_style.rs:~643 `cap_chip` | 3 kinds | `btn` | a pure clone minus press fill; `btn` restores press feedback. Keys `style-cap-*` unchanged. |
| 10 | inspector_style.rs:~665 `preset_chip` | per preset | `chip_btn` (swatch + label content) | px 8 → chip 6/3/5; gains press fill. Keys `style-preset-{i}` unchanged. |
| 11 | easing_panel.rs:~203 `picker_cell` | 4 | `chip_btn` (canvas + label content) | px 4 → 6; canvas size and `ease-pick-*` keys unchanged. |
| 12 | menus.rs:224 `hinted_item`, :~340 `item_icon` | all menu items | kit menu row (charter §9) — **not** the button family | see §5.4; hint text becomes a `design::kbd` chip. Owned by the menus foundation agent. |
| 13 | command_palette.rs:1594 command row | per result | `design::list_row` + `selected_fill()` | accent@0.85 solid → 16% selected fill; height per charter §9. |
| 14 | audio_panel.rs:~722 clip row | per clip | `design::list_row` | selected `control_track` → `selected_fill()`; gains press fill via the shell. |
| 15 | clips_panel.rs:~652 clip row | per clip | `design::list_row` | py 4 → list-row metrics; thread-local handler verbatim. |
| 16 | figures_panel.rs:536 figure card | per figure | `design::list_row` | press = `select_figure`, unchanged. |
| 17 | swatches: inspector_figure.rs:~824, inspector_segment.rs:~657, fx_clips.rs:~1004, inspector_style.rs gradient swatch, scene_panel.rs:895/:923, particles_panel.rs:539, text_tool.rs:1003 | per colour | one swatch discipline (§5.4) | 18 → 22 where smaller; selected marker → accent outline everywhere; scene bg drops the nested core. |
| 18 | graph_panel.rs:1459 ease-preset pressable | per preset | `btn` (verify content at adoption) | hand-rolled `on_press` in a btn-shaped row; if it carries non-text content, `chip_btn` instead. |
| 19 | timeline_marks.rs:842 frame chip, :866 marker cells | per frame | **no migration** | charter §13 pins chip geometry/keys. |

Out of scope, verified family-clean: chrome_view
transport + app bar, workspaces chips,
playback_controls, settings_popover, figure_add,
camera overlay buttons, draw_tools, home/library/
layers panels, toasts (all `btn`/`icon_btn` already).

---

## 7. Tests that pin button behaviour — do not break

A restyle may change fills/metrics only where no
test pins the old value; every entry below is a hard
constraint. (CI is the only runner — the F/A agents
check this list before touching a builder.)

### 7.1 Family unit tests — src/rune_sec/icon_btn.rs tests (:288–385)

These pin the state table numerically:

- `glyph_color_matches_the_kit_label_choice` —
  ink = WHITE on active, `t.text` otherwise.
- `state_fills_mirror_the_kit_button` — fill/hover/
  press = accent/accent/accent on active; `t.card` /
  `t.control_track` / `t.control_off` otherwise.
  **Any change to §5.1's normal/active rows must
  deliberately rewrite this test.**
- `muted_foreground_is_the_theme_secondary_text` —
  `glyph_color_muted` = `t.text_secondary` and ≠ the
  live ink: pins the inert ink the disabled rule
  adopts.
- `inert_fill_is_the_live_resting_fill` —
  `fill_color_inert` = `t.card` = `fill_color(false)`:
  pins inert fill = card (the §4 choice over
  `control_off`).
- `use_icon_follows_the_catalogue`,
  `glyph_size_is_icon_size_capped_by_the_button`,
  `builders_construct_on_both_fallback_branches`,
  `inert_builders_construct_on_both_fallback_branches`
  — fallback + square-contract behaviour.

### 7.2 Runtime press tests — src/rune_sec/snap_tests.rs

39 tests, **109 `tap(...)` calls**; a tap resolves the
element by key and presses its centre, so a restyle
must keep (a) every key byte-identical, (b) every
button's centre inside its clip (the Phase M
frames-row lesson: a clipped button is an
unpressable button), and (c) live/inert swaps
presenting the **same key in both forms**. The
button-critical tests:

- `transport_shows_icon_buttons` (:1361) — the `play`
  key is ONE element in both states; the press flips
  `core.playing`; the active flag is the only change.
- `swap_next_button_exchanges_frame_contents`
  (Phase M round 10) — `fop-swap-next` live at
  frame 0, inert at the last frame, same key; the
  frames row must stay unclipped (the flex_wrap fix).
- `pose_panel_inert_apply_is_present_without_a_pose`
  (:1702) — the inert form is *presented* (findable)
  with no pose: inert twins must remain in the tree.
- `add_frame_button_and_chip_navigation` (:516),
  `play_advances_the_playhead_on_the_virtual_clock`,
  `playback_advances_and_pauses` — transport/chip
  presses (`play` tapped 6× across the file).
- `file_menu_opens_lists_items_and_scrim_closes`
  (:648) — menu items (incl. hinted/icon rows) press
  through their whole row.
- `command_palette_ctrl_k_types_and_enter_runs_undo`
  (:711) and the palette tests (:1279, :1486) —
  palette rows run their command on press/Enter.
- `settings_theme_buttons_flip_core_light_theme`
  (:770) — `settings-theme-dark/light` are `btn`s
  whose press flips `core.light`.
- `settings_speed_chips_set_playback_speed`
  (round 10) — settings speed chips are pressable
  `btn`s setting the speed.
- `workspace_chip_switches_the_workspace_field`
  (:790) — `ws-*` chips (32 `ws-animate` taps across
  the file seed the workspace discipline).
- `keyframes_note_and_delete_button_do_not_overlap`
  (:1791) + `status_strip_is_not_covered_by_the_right_column`
  (:1873) — **geometry** assertions on element rects:
  metric changes in the keyframe/frames area must
  re-check these.
- `timeline_marker_press_seeks` (:1661),
  `particles_panel_add_selects_the_new_event`
  (:1204), `pose_mirror_all_button_negates_every_angle_and_keeps_roots`,
  `fx_split_button_cuts_the_selected_clip_at_the_playhead`,
  `pb_speed_button_cycles_the_shell_playback_speed`,
  `fig_centre_button_centres_the_figure_on_the_stage`,
  `fig_ring_up_button_raises_the_head_ring_scale`,
  `timeline_clips_lane_appears_with_a_planted_clip`,
  `build3d_workspace_shows_fig3_stage_state`,
  `guides_toggle_flips_state` + `guide_split_toggles_flip_independently`
  (settings guide toggles are active-state `btn`s),
  `view_menu_toggles_hide_columns`,
  `stroke_selection_row_reports_empty_project`.

### 7.3 Panel unit tests touching buttons

- three_d_panel.rs:1518 `ictl_builds_live_and_inert_forms`
  — pins ictl's two branches; migration row 5
  rewrites it onto the direct family calls.
- three_d_panel.rs:1453
  `mirror_button_glyph_is_the_left_right_swap` and
  :1534 `frame_button_glyph_is_in_the_icon_table` —
  glyph-name pins for ictl call sites.
- keyframe_tools.rs:2409
  `no_joint_note_gets_its_own_row_not_the_delete_buttons_row`
  + the key constants `NO_JOINT_NOTE_ROW_KEY`,
  `ACTIONS_ROW_KEY`, `DELETE_KEY_BUTTON_KEY`
  (keyframe_tools.rs:~1010) — `key-delete` keeps its
  key and its own row under any restyle.
- menus.rs:1032
  `all_dropdowns_build_with_hinted_items_and_guides_states`
  — menu builders construct in all guide states.
- stroke_inspector.rs:2368
  `selection_presses_step_wrap_and_clear_as_view_state_only`
  and :2423 `selection_presses_on_an_empty_project_only_report`
  — handler-level pins behind the stroke steppers
  (icon_only_btn sites); restyle-safe, listed so the
  adoption agent knows the presses are covered.
- camera_panel.rs:1274
  `prev_next_buttons_move_the_playhead_or_report` —
  handler-level pin for the camera key steppers.
- pose_tools.rs — the `APPLY_MIRRORED_ICON` pin
  (pose_tools.rs:~1180): pibtn's mirrored-apply glyph
  stays `arrow-right-arrow-left` through migration
  row 4.
- frame_ops.rs tests around the `can_*` gates and
  refusal statuses — the evidence base for migration
  row 1's muted-live classification; if a status is
  pinned, the site stays muted-live, never inert.
- The 733-key gate (`scripts/phase_j_checks.py`,
  widened per phase) diffs key **sets**: no migration
  row may add, drop or rename a key. `chip_btn`,
  `btn_inert`, `danger_*` reuse the caller's existing
  key — they introduce no keys of their own.

### 7.4 Headless probes

The `--tree` scene dumps assert key presence in the
real binary (Phase M: 11 scenes, incl. `three-d`,
`three-d-stage`, `loop-range`). Workspace chips
(`ws-*`), frame chips (`frame-*`) and panel keys must
survive every restyle byte-identical.

---

## 8. Work list for the coding waves (file-scoped)

**F wave (foundation, chrome/icon owners):**
1. chrome_view.rs: add `btn_inert`, `btn_muted_live`,
   `danger_btn`, `danger_icon_btn`, `chip_btn` +
   `DANGER`/`DANGER_DEEP` consts, per §5. Unit tests
   mirroring icon_btn's (state-table pins for the new
   members).
2. icon_btn.rs: `icon_btn_inert` fallback delegates
   to `btn_inert`; keep all §7.1 tests green
   (they pin the shared values, so this is a pure
   refactor under them).

**A wave (one row of §6 per panel agent):** rows
1–11 and 13–18, each in its owning file, handlers
and keys verbatim; row 12 belongs to the menus
foundation agent.

**V wave:** extend §7.1-style pins to the new members
if F didn't; re-run the key gate; the snap_tests in
§7.2 are the regression net — no new button tests
are needed unless a migration changes a metric a
geometry test measures (§7.2's two rect tests).

## 9. Honest limits

- Element trees are not readable from bin-crate unit
  tests (Phase G §12 finding): metric parity between
  live and inert twins is structural (same builder
  calls), asserted by construction tests, not by
  measuring laid-out boxes. The Runtime tests in
  §7.2 are the real geometry net.
- Call-site counts in §1–2 are grep counts
  (approximations ±a few): they size the migration,
  they are not an inventory. The key gate is.
- `graph_panel.rs:1551` (a second hand-rolled
  `on_press`) was not read line-by-line for this
  spec; row 18's sibling site should be classified
  (list row vs chip) by its adoption agent before
  migrating.
