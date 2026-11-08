# Squad 17 Audit — Iconography & Micro-Interactions

Phase 1 audit, ULTIMATE UI/UX MISSION. Scope: the icon
system (`src/rune_sec/icon_data.rs`, `icons.rs`,
`icon_btn.rs`), the kit's interactive builders
(`src/rune_sec/design.rs`, `src/chrome_view.rs`), and
every local builder that defines its own interaction
states. Method: source reads + grep censuses on the
current tree; Rune capabilities checked against the
pinned rune-src (rev 87caf2a). No builds run (AGENTS.md).

## 1. Icon inventory

- **163 curated icons**, Font Awesome Free 7.3.1 solid,
  asserted by test (`icons.rs`:
  `every_curated_icon_parses_inside_its_view_box`).
  View-box height is always 512; widths vary 384–640,
  and `icon_el` sizes by height, so a glyph's on-screen
  width follows its aspect ratio.
- Resolution is total: the Phase N icon audit found
  zero unresolved names in production literals, and
  every builder degrades an unknown name gracefully
  (label-only or empty square) — no silent breakage
  path.
- **Doc defect (D9):** `icons.rs`'s module header still
  says "102 Font Awesome Free solid icons". The set is
  163. The icon system's own documentation is stale.
- **Unproven rendering (carried, not new):** `icons.rs`
  itself records that compound-path holes (rings,
  letterform counters) have never been pixel-verified
  through Rune's `fill_path`. Squad 19 should treat
  hole rendering as an open visual-QA item.

## 2. Icon sizing & alignment census

Kit-internal sizes are consistent: 13 in `icon_btn`
and `panel_title`, 12 in `chip_btn`, 20 in
`empty_state`, toast severity glyphs at 13, and every
literal `icon_only_btn` call site passes size **26**
(13 measured sites, no variance).

The drift is in **direct `icon_el` calls** in panels
(~19 sites): sizes in use are **11, 12, 13, 14, 16,
20** with no documented scale. The clearest symptom:
the same magnifying-glass search glyph is drawn at
**12** (home, fx browser), **13** (figures), **14**
(library, pose tools) and **16** (command palette) —
four sizes for one concept. The status strip's 11 pt
segment glyph is the only 11 in the app.

Alignment notes: in labelled buttons the aspect-width
variance is invisible (each button sizes to content).
In `icon_only_btn` squares the glyph is centred, so
wide icons (640) read optically larger than narrow
ones (384) at the same height — inherent to the
height-based sizing, acceptable, but worth one line
in the design-system spec so nobody "fixes" it per
panel.

## 3. State-coverage matrix (per builder family)

Legend: ✓ present · ✗ absent · — not applicable.
"Disabled" = a dedicated inert/muted form exists in
the family. Focus and Help are ✗ for **every** row —
see §5 and §6.

| Family | Rest | Hover | Press fill | Selected | Disabled form | press_scale |
|---|---|---|---|---|---|---|
| `chrome_view::btn` | ✓ | ✓ | ✓ | ✓ accent + white ink | ✓ `btn_inert`, `btn_muted_live` | ✓ 0.97 |
| `chrome_view::danger_btn` | ✓ | ✓ | ✓ (≈5:1 reds) | — always danger | ✗ none | ✓ |
| `icon_btn` (141 sites) | ✓ | ✓ | ✓ | ✓ | ✓ `icon_btn_inert` (26 sites) | ✓ |
| `icon_only_btn` (33 sites) | ✓ | ✓ | ✓ | ✓ | ✓ inert twin — but only **2** sites use it | ✓ |
| `chip_btn` (8 sites) | ✓ | ✓ | ✓ | ✓ | **✗ no inert chip exists** | ✓ |
| `danger_icon_btn` (1 site) | ✓ | ✓ | ✓ | — | ✗ none | ✓ |
| `design::list_row` / `row_shell` | ✓ transparent | ✓ `t.hover` overlay (unselected only) | **✗** | ✓ accent @ 0.16 | — | ✗ |
| `design::toggle` / `toggle_row` | ✓ pill | **✗** | **✗** | ✓ on/off is the state | ✗ no disabled toggle | ✗ |
| `design::swatch` (6 consumer files) | ✓ | **✗** | **✗** | ✓ 2 pt accent border | — | ✗ |
| `design::slider_track` | visual only — the caller's root owns the drag; no hover, press or drag emphasis of its own | | | | | |
| `menus` items (3 local builders) | ✓ | ✓ | ✓ | ✓ | — | ✗ (see I-10) |
| `command_palette` rows (local) | ✓ | ✓ | **✗** | ✓ accent @ 0.85 | — | ✗ |
| **timeline frame chips** (local, `timeline_marks`) | ✓ | ✓ | **✗** | ✓ current = accent | — | **✗** |
| `graph_panel::preset_chip` (local) | ✓ | ✓ | ✓ | — action | — | **✗** |
| `easing_panel::picker_cell` (local) | ✓ | ✓ | ✓ | ✓ | — | ✓ — the model exception: family states + a documented width reason for staying local |

Non-interactive kit members (`panel_title`,
`section_label`, `kbd`, `empty_state`, `divider`) are
consistent and need no states.

## 4. Toggle, slider & stepper feedback

- **Toggles:** `toggle_row` is the used form (settings
  ×7, onion ×1); the bare `design::toggle` has **zero**
  production callers. The pill has no hover and no
  press feedback, and the knob **teleports** — no
  spring or transition is applied anywhere in the kit
  (the only `.springs(...)` calls in the shell are the
  four overlay entrances: palette/menus/settings SNAP,
  toasts SETTLE). Whether Rune can animate the knob's
  layout offset is unverified; what is certain is that
  nothing attempts it today.
- **Sliders:** `slider_track` is visual-only by
  design; the two consumers (scene weather, layers
  opacity) own their drag roots. Feedback during a
  drag is exactly "the knob follows the pointer" — no
  hover state, no enlarged/active knob, no value
  bubble. The wind slider's bipolar centre-fill is a
  documented local exception.
- **Steppers:** the readout is standardised
  (`stepper_value`, 13 consumer files, mono, fixed
  min-width so digits never shift the buttons). The
  −/+ buttons come from the button families and carry
  full states. This is the kit working as intended.
- **Group-hover reveal:** Rune supports it
  (`hover_group`, `hover_within`) and the app uses it
  **zero** times — dense rows (figures) show every
  action permanently. Recorded as an evaluated option
  (I-8), not a defect: hiding destructive actions
  behind hover hurts discoverability, which the
  mission brief ranks higher.

## 5. Tooltip verdict — SUPPORTED, ZERO ADOPTION

Checked against the pinned Rune source, not assumed:

- The **core `rune` crate** implements tooltips
  natively: `El::help(text)` (el.rs:1267),
  `.help_keys([...])` for shortcut hints,
  `.help_above()`, `.help_when_truncated(...)`. The
  runtime (`runtime/input/help.rs`) shows the tip
  after the hover delay, hides it on press / scroll /
  key / pointer-leave, and draws it itself off macOS
  (AppKit's native tooltips are the macOS path only).
  No app-side wiring, no rune-kit overlay, and no new
  dependency is needed — and the help text is carried
  in the `--tree` dump, so adoption is **headlessly
  verifiable**.
- **App usage: zero.** There is not one `.help(` call
  in the shell. All 33 `icon_only_btn` sites — figure
  eye / delete / opacity, toast dismiss, 3D and layer
  controls — are glyph-only buttons whose meaning a
  new user can only learn by pressing them. Labelled
  buttons are less hurt, but get no shortcut hints
  either (help_keys exists for exactly that).
- Adoption is cheap: the builders return `El`, and
  `El::help` chains onto the returned element, so
  call sites can adopt without any kit signature
  change. The kit work is a convention (which controls
  must carry help) plus help on the icon-only family
  first.

## 6. Focus state — absent app-wide

Rune supports keyboard focus (`focusable`,
`focus_highlight`) and Tab traversal
(`WindowSpec::tab_visits_controls`), but the default
is **off** (rune app.rs:137) and the app never sets
it and never calls a focus API. Result: no control in
the application has a visible focus state, ever.
Enabling Tab traversal has been a deferred product
decision since Phase N — this audit does not
re-litigate it, but the mission's accessibility
criteria cannot pass while the decision stays
unmade. Flagged for Squad 18 / the integrator as a
decision item, not a coding item.

## 7. Defects, ranked

- **D1 — The timeline frame chips have no press
  state.** `timeline_marks`' frame chip — the
  most-pressed control in an animation editor, every
  seek lands on one — defines fill + hover and stops:
  no `.press()` fill, no `press_scale`. The palette
  rows and kit `list_row` share the same missing-press
  pattern. **This is the worst state gap in the app.**
- **D2 — Zero tooltips** on 33 icon-only buttons and
  everywhere else (§5). The worst discoverability gap.
- **D3 — `swatch` gives no interaction feedback at
  all**: no hover, no press, no scale — the border
  change after a successful press is the only signal,
  across 6 consumer files of colour picking.
- **D4 — Toggles are inert-feeling**: no hover, no
  press feedback, teleporting knob, no disabled form.
- **D5 — App-bar Undo/Redo never dim.** They are
  always-live `icon_btn`s even with empty stacks,
  while panels carefully gate their ops with inert
  forms — and `EditorCore::undo_depth()` already
  exists, so the information is available. The two
  most standard gated controls in any editor are the
  ones that don't gate.
- **D6 — `chip_btn` has no inert twin** — the only
  button family without one, so a gated compact
  toggle has no in-family disabled form.
- **D7 — No visible focus state anywhere** (§6) —
  decision-blocked, recorded.
- **D8 — Icon size drift in direct `icon_el` calls**
  (§2): six sizes, one search glyph at four of them.
- **D9 — `icons.rs` header says 102 icons; the set
  is 163** (§1).
- **D10 — `graph_panel::preset_chip` lacks
  `press_scale`**; menu items also lack it, but menus
  are transient overlays — that one should be decided
  and documented, not silently aligned (I-10).

## 8. Work items (single-coder sized, kit first)

- **I-1 (kit convention + figures first):** adopt
  `El::help` — document the rule in `icon_btn.rs`
  (icon-only controls MUST carry help; labelled
  controls SHOULD carry `help_keys` where a shortcut
  exists), then apply to `icon_only_btn` /
  `icon_only_btn_inert` call sites, figures panel
  first, then layers / 3D / toasts. Proof: `--tree`
  dumps carry the help strings; gate census unchanged
  (help adds no keys).
- **I-2 (`timeline_marks.rs`):** frame chips gain the
  family press treatment — `.press(...)` fill +
  `.press_scale(PRESS_SCALE)`. One builder site.
- **I-3 (kit, `design.rs`):** `list_row` / `row_shell`
  gain a press overlay in `row_base`, and
  `command_palette`'s row takes the same press fill —
  one kit change + one local line class.
- **I-4 (kit, `design.rs`):** `swatch` gains hover
  and press overlays (Rune's hover/press are overlays
  over the fill, so the colour itself stays visible —
  use the theme's overlay tokens). Six consumer files
  inherit.
- **I-5 (kit, `design.rs`):** `toggle_pill` gains a
  hover overlay on the track; document the knob's
  teleport as a known limit pending a Rune
  layout-animation check. Do not fake a slide.
- **I-6 (`chrome_view.rs`):** app-bar Undo/Redo pick
  `icon_btn_inert` when the corresponding stack is
  empty (`undo_depth()` and its redo counterpart).
  Keys and handlers unchanged.
- **I-7 (kit, `icon_btn.rs`):** add `chip_btn_inert`
  for family symmetry (chip metrics, muted ink, no
  handler), with the family's construction tests.
- **I-8 (docs + call sites):** publish the icon size
  scale in the design-system spec (11 status strip /
  12 chip & inline / 13 standard / 16 palette hero /
  20 empty state) and converge the four search-glyph
  sizes onto it. Includes the hover_group evaluation
  note (§4) as a decided-and-deferred entry.
- **I-9 (`graph_panel.rs`):** `preset_chip` gains
  `press_scale` to match the family.
- **I-10 (docs):** decide menu-item `press_scale`
  (recommendation: NO — transient overlays; record
  the exception beside easing `picker_cell`'s), and
  fix the `icons.rs` "102 icons" header to 163.

## 9. What is already good (do not redo)

- The button family's state table is genuinely
  unified: one fill/hover/press/ink derivation shared
  by `btn`, `icon_btn`, `icon_only_btn`, `chip_btn`
  and both danger members, with the ≈5:1 danger reds
  measured, not guessed.
- Inert discipline is real: gated ops render inert
  forms with no handler at all (26 `icon_btn_inert`
  sites) instead of live buttons that refuse.
- `press_scale(0.97)` on every live kit builder,
  honouring the host's Reduce Motion setting.
- `inspector_style`'s old local chips verifiably
  delegate to `chip_btn` / `design::swatch` now —
  the Phase N absorption claims check out in source.
