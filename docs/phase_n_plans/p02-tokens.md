# Phase N — P02: Design Tokens Spec

Scope: the mapping between the shell's design needs,
rune's token system (`rune::tok`, pinned rune rev,
`crates/rune/src/tok.rs`) and what the shell actually
does today (audited 2026-11-08 against the Phase M
tree, 733 keys). This plan feeds the F-wave kit
(`src/rune_sec/design.rs`, charter-fixed API) and
every A-wave adoption brief. No code changes here.

## 1. The token inventory (what rune gives us)

Spacing (`tok::space`): S1 4 · S2 8 · S3 12 · S6 24.
Nothing at 2, 6, 10, 16 or 20.

Radii (`tok::radius`): SM 6 · MD 8 · LG 12.
Nothing at 4 (chips) or 5.

Type (`tok::type_`, all `TextStyle`, Sans family):
BODY 13/400 · LABEL 12/500 · SECTION 11.5/600 ·
TITLE 13/600 · MONO_CAPTION 12/500 — **note:
MONO_CAPTION as defined does not set the Mono
family** (it is `TextStyle::new(12, 500)`, family
Sans). A mono readout must write
`type_::MONO_CAPTION.mono()` (the `.mono()` builder
exists on TextStyle) or accept sans. The kit's
`stepper_value` / `kbd` builders must do this
explicitly; the charter's "MONO_CAPTION" wording
alone does not produce mono digits.

Theme (`tok::Theme`, `tok::theme(dark)`) — 16
fields: `bg`, `card`, `card_border`, `text`,
`text_secondary`, `separator`, `control_track`,
`control_off`, `indicator`, `knob`, `hover`,
`press`, `shadow`, `shadow_soft`, `edge`,
`knob_shadow`. Both variants are fully specified
in tok.rs (dark: card oklch 0.232, text 0.955,
text_secondary 0.72; light: card white, text
0.225, text_secondary 0.50 — full table in tok.rs,
do not copy values into shell code).

Accent: `tok::accent(i, dark)` — 6 named hues,
index 0 = blue (dark L 0.66 / light L 0.60,
chroma 0.17, hue 256). The shell uses index 0 only.

Motion (`tok::motion`): springs exist; the shell
uses none and the charter takes motion out of
scope for Phase N. Not mapped further here.

## 2. Where the shell stands today (audit facts)

- **Fonts are the one real adoption.** `type_::`
  counts across the shell: LABEL ×269, TITLE ×24,
  BODY ×22, SECTION ×10, **MONO_CAPTION ×0**.
  Headings therefore render mostly in LABEL, not
  the SECTION the charter prescribes (figures_panel
  root: `text("Figures").font(type_::LABEL)`).
- **Spacing/radius tokens: zero use.** No file
  references `tok::space` or `tok::radius`. Every
  metric is a literal that happens to match (or
  miss) a token value — tallies in §3.
- **Theme fields are half-used.** As values in
  element styling: `t.text_secondary` ×232,
  `t.text` ×161, `t.card` ×50, `t.control_track`
  ×35, `t.control_off` ×28, `t.bg` ×14,
  `t.separator` ×3, `t.card_border` ×3.
  **`t.hover`, `t.press`, `t.indicator`, `t.knob`,
  `t.shadow`, `t.shadow_soft`, `t.edge`,
  `t.knob_shadow`: zero uses.** Buttons fake the
  hover/press states with `control_track` /
  `control_off` (chrome_view::btn, icon_btn,
  figures_panel::row_toggle all do this) — those
  are much stronger fills than the theme's
  intended hover (white α0.016 dark) and press
  (α0.03). `.border(` appears 8× in the whole
  shell, `.shadow(` 4×: panels are flat because
  almost nothing draws card_border or shadow.
- **Accent is called with a hardcoded dark flag:**
  `tok::accent(0, true)` ×32 in 18 files
  (chrome_view, icon_btn, menus, settings_popover,
  status_strip, timeline_marks, fx_clips,
  graph_panel, inspector_style, scene_panel,
  layers_panel, camera_panel, easing_panel,
  figures_panel, frame_ops, home_panel,
  particles_panel, text_tool, command_palette).
  The one production theme construction is
  correct — main.rs:555 `tok::theme(!s.core
  .light_theme)` — but the accent ignores it.
- **Panel roots are bare cols.** figures_panel:
  `col().key("figures-panel").gap(6.0)` + a
  hand-rolled header row; settings_popover:
  `col().key("settings").gap(6.0)`. No fill, no
  border, no padding — the charter's diagnosis,
  confirmed.
- **Nine private heading helpers**, all different:
  easing_panel.rs:267 `section` (LABEL, bare text),
  export_panel.rs:855 `section` (SECTION in a
  gap-6 col) + `section_with_icon`, figure_add.rs
  :154 `section_label` (LABEL), fx_browser.rs:246
  `section`, home_panel.rs:946 `section_label`,
  playback_controls.rs:448 `section`,
  settings_popover.rs:551 `section_label`,
  three_d_panel.rs:688 `section_label`,
  command_palette.rs:1513 `section_heading`.

## 3. The mapping table

Design need → tok token → what the shell hardcodes
instead today (evidence counts are shell-wide,
production + test code as grepped; test fixtures
excluded where noted).

| Design need | Token | Shell today |
|---|---|---|
| Panel card fill | `t.card` | Nothing — panel roots are bare `col()`s (figures_panel, settings_popover, all panels). `t.card` ×50 exists but on buttons/rows inside panels |
| Panel card border | `t.card_border` (1.0) | ×3 total; `.border(` ×8 shell-wide |
| Panel radius 12 | `radius::LG` / kit `PANEL_RADIUS` | `.radius(12.0)` ×1 in the whole shell |
| Panel padding 12 | `space::S3` / kit `PANEL_PAD` | No panel pads at all; main.rs columns use `.gap(12.0)` literal (main.rs view, side-left) |
| Panel internal gap 8 | `space::S2` / kit `PANEL_GAP` | `.gap(8.0)` ×139 by literal — right value, no token |
| Tight row gap 6 | **no token** (between S1/S2) | `.gap(6.0)` ×101 — the shell's de-facto second gap; also ICON_LABEL_GAP 6 (icon_btn.rs:72) and panel root gaps (figures_panel, settings_popover) |
| Hairline stacking gap 2 | **no token** | `.gap(2.0)` ×19 (timeline stack, marker rows) |
| Chip/inline gap 4 | `space::S1` | `.gap(4.0)` ×47 by literal |
| Window/column margin 16 | **no token** (no S4) | `.px(16.0)` ×3; layout margin 16 in main.rs view |
| Large separation 24 | `space::S6` | Rarely needed; no literal cluster |
| Button padding | kit constants (charter: px 10 / py 5) | `.px(10.0)` ×19, `.py(5.0)` ×20 — the button metrics already agree everywhere (chrome_view::btn, icon_btn.rs:169–171); they are literals, not tokens |
| Button radius 6 | `radius::SM` | `.radius(6.0)` ×41 by literal |
| List-row radius 8 | `radius::MD` | `.radius(8.0)` ×27 by literal, mixed uses |
| Kbd chip radius 4 | **no token** | `.radius(4.0)` ×9 scattered; no `kbd` anything exists (grep `kbd` = 0) |
| Chip radius 5 | **no token** | `.radius(5.0)` ×6 (figures_panel::row_toggle px6/py3/radius5 — a third button metric set) |
| Panel title | `type_::SECTION` + `t.text` | SECTION ×10 only; most panel headings use LABEL (figures_panel "Figures") |
| Section label in panel | `type_::SECTION` + `t.text_secondary` | Nine private helpers (§2), LABEL or SECTION inconsistently |
| Body text | `type_::BODY` + `t.text` | BODY ×22; most row text is LABEL |
| Row / control label | `type_::LABEL` | LABEL ×269 ✓ (the one settled mapping) |
| App-bar project name | `type_::TITLE` | ✓ already (chrome_view app_bar) |
| Numeric readout (stepper value, timecode, status numbers) | `type_::MONO_CAPTION` (+ `.mono()`, see §1) | ×0 — steppers show values in LABEL; status_strip segments in LABEL. Sole mono use in the shell: help_panel.rs:436 `type_::LABEL.family(Family::Mono)` |
| Primary text | `t.text` | ✓ ×161 |
| Secondary text (meta, counts, hints) | `t.text_secondary` | ✓ ×232 |
| Third text level (disabled, placeholders, faint meta) | **no token** | See §4.1 — the need is real and currently faked three ways |
| Header hairline / divider | `t.separator` (1px) | ×3 — dividers essentially don't exist yet |
| Button hover | `t.hover` | ×0 — faked with `t.control_track` (btn, icon_btn, row_toggle) |
| Button press | `t.press` | ×0 — faked with `t.control_off` |
| Inactive button fill | `t.card` | ✓ (btn/icon_btn already) |
| Toggle track off | `t.control_off` | No pill toggle exists; toggles are text buttons / row_toggle chips. `control_off` ×28 mostly as fake press fill |
| Slider track | `t.control_track` | ✓ partially — ×35 across audio/camera/clips/easing/graph panels; the kit `slider_track` standardises the visual |
| Toggle/slider knob | `t.knob` (+ `t.knob_shadow`, `t.edge`) | ×0 |
| Selected list row | kit `selected_fill()` (accent α0.16; border accent α0.35 where drawn) | Nothing — no α0.16 accent fill exists anywhere; selection is per-panel ad hoc (row fills, accent text, outlines) |
| Accent (active states, fills) | `tok::accent(0, dark)` via kit `accent()` | `tok::accent(0, true)` ×32 / 18 files — dark flag hardcoded (§5) |
| Accent as u32 (canvas/stage) | **no token** (tok is Color-typed) | `0x6D8CFF` re-declared per file: stage_view.rs:318 SELECTION_ACCENT, waveform_view.rs:198 WAVE_HEX, fx_clips.rs:418 STRIP_ACCENT, stage_fx.rs:253 GIZMO_COLOR, timeline_marks.rs:967 AUDIO_SPAN_RGB, fig3_preview.rs:88 SELECTED_COLOR |
| Popover/menu shadow | `t.shadow` / `t.shadow_soft` | `.shadow(` ×4 shell-wide; menus/palette are flat cards-in-name-only (`fill(t.card)` at command_palette.rs:1551, no border/shadow) |
| Empty-state icon + text | kit `empty_state` (icon 20px + LABEL, `text_secondary`, py 12 = S3) | Bare text rows today ("No exports yet" export_panel; assorted "No …" rows). Status strings ("No such clip." etc.) are engine status text — **not** empty states, do not restyle them |

## 4. Gaps — needs with no token

1. **A third text level.** Theme has exactly two
   text colours. The shell already reaches for a
   third: (a) icon_btn's inert glyph colour is
   `glyph_color_muted(t) = t.text_secondary` —
   disabled and secondary are the same level;
   (b) canvas-side, graph_panel.rs:322 declares
   `GLYPH_MUTED: u32 = 0x707B8C` (the retired 2.0
   palette's muted hex) because no muted token
   exists; (c) menu shortcut hints and panel meta
   compete with real secondary content at the
   same level. **Spec:** the kit derives it —
   `design` should expose a muted text colour as
   `t.text_secondary` at reduced alpha (the
   shell already uses `.alpha(…)` on colours,
   e.g. α0.55/0.75 sites in rune_sec) — proposed
   `text_muted(t) = t.text_secondary.alpha(0.6)`,
   F-wave builder to confirm the alpha against
   both themes and pin it in design.rs. Do NOT
   invent a new base colour.
2. **Gap 6 and gap 2.** The shell's two most-used
   non-token gaps (×101, ×19). S1 4 is too tight
   for icon+label and panel-root stacking; S2 8
   is the panel gap. **Spec:** adoption keeps 6.0
   only inside controls (icon/label gap, chip
   rows — name it in design.rs as a kit-local
   constant if the builder wants one) and moves
   panel-level stacking to S2 8 / S3 12. Gap 2
   stays a timeline-internal literal (chips
   geometry is charter-frozen).
3. **Button metrics (px 10 / py 5) have no space
   token** — they are control metrics, not
   spacing scale. Charter already fixes them as
   kit behaviour (btn/icon_btn keep their
   literals; adoption agents don't re-derive
   them). row_toggle's px6/py3/radius5 is the
   one divergent set — it retires onto the kit
   toggle/button family (figures_panel).
4. **Radius 4 (kbd, small chips) and 999 (stage
   camera pill)** have no token. Charter fixes
   both as kit literals (kbd radius 4; pill
   radius 999). Fine as design.rs constants.
5. **No S4/S5 (16/20).** Window margin is 16;
   empty-state icon is 20px; panel-to-panel
   separation in the columns is 12 (S3, main.rs
   literals). **Spec:** margin stays 16 as a
   main.rs layout literal (not a token claim);
   nothing in Phase N should "correct" it to 12
   or 24 — layout geometry is frozen by the
   Phase J column contract.
6. **MONO_CAPTION is not mono** (§1). Kit
   builders must apply `.mono()`.
7. **No u32 accent token.** Canvas code (stage,
   timeline, waveform, fx strip, fig3) works in
   u32 hex; tok is Color-typed. Six files
   re-declare 0x6D8CFF (§3). **Spec:** out of
   Phase N's element-styling scope — the
   adoption wave does not touch canvas colour
   constants. A future cleanup could add one
   shared `const ACCENT_HEX: u32` in stage_view
   (the SELECTION_ACCENT precedent), but that
   is engine-adjacent and not required for the
   visual pass. The retired-palette canvas
   hexes (waveform_view.rs:204 CAPTION_HEX
   0xA2AAB8, status_strip.rs:792 GLYPH_TEXT
   0xF1F3F7, graph_panel.rs:322 GLYPH_MUTED)
   are likewise noted, not adopted.
8. **No "danger/success/warning" tokens.** Theme
   has none; the shell uses content colours
   (inspector_figure.rs:808 / inspector_segment
   .rs:636 `hex_color(0xF16F78)` for a delete-ish
   red) and toast severity colours (Phase H
   literals). Charter doesn't ask for semantic
   colour tokens; toasts are an F-wave surface
   and keep their severity palette. Recorded
   here so nobody invents one mid-wave.

## 5. Light-theme notes

- The plumbing is one line and correct:
  main.rs:555 `tok::theme(!s.core.light_theme)`.
  Every `t.*` field has a light value in tok.rs,
  so any element styled purely from Theme fields
  is light-safe for free. **Adoption rule: a
  restyled element may only take colours from
  `t.*`, `design::accent()`, `design::selected
  _fill()` or Color::WHITE (on accent)** — that
  rule is what makes the light theme survive
  the phase.
- **The accent dark-flag is the one live
  defect.** `tok::accent(0, true)` (×32) pins
  the dark-tuned accent (L 0.66) in light mode
  too, where tok intends L 0.60. The charter
  fixes `design::accent()` as `tok::accent
  (0, true)` "matching chrome_view::btn" — i.e.
  Phase N **freezes today's behaviour**; all 32
  sites collapse into the one kit call, and
  flipping to a theme-aware accent afterwards
  becomes a one-line change. Two blockers to
  record for that future flip: Theme carries no
  `dark` flag to derive from, so design::accent
  cannot self-decide today; and headless snaps
  run the dark theme (light_theme defaults
  false), so a light-accent change is not
  pixel-verifiable in CI. F-wave: implement the
  charter signature verbatim; do not "fix" the
  flag inside adoption agents.
- **Hover/press swap is theme-asymmetric.**
  Moving btn/icon_btn hover from
  `t.control_track` → `t.hover` and press from
  `t.control_off` → `t.press` is nearly
  invisible in dark (α0.05→0.016, α0.085→0.03)
  and a bigger lightening in light (α0.13→0.07,
  α0.24→0.12). Both are the theme's intended
  values; the F wave should state the change in
  its report so the user's light-theme
  screenshot check (if any) isn't a surprise.
- **Canvas colours don't follow the theme** —
  stage/canvas u32 hexes (incl. GLYPH_TEXT
  0xF1F3F7, near-white, in status_strip canvas
  glyphs) are content-space colours against
  the project stage, not chrome. They read
  correctly on the dark stage in both themes.
  Out of scope; do not theme them.
- `Color::WHITE` label text on the accent
  (active buttons) is fine in both themes
  (light accent L 0.60 is mid-dark).

## 6. Adoption list per file

Ordered as the waves run. "Swap" = replace the
literal/helper with the kit or token named;
handlers, keys and status strings never move
(charter rules).

### F-wave (foundation) files first

- **design.rs (new)** — implement the charter
  API on §1–§4 of this spec: PANEL_* constants
  = LG/S3/S2 values; `accent()` =
  `tok::accent(0, true)` verbatim (see §5);
  `selected_fill()` = accent α0.16;
  `stepper_value`/`kbd` use
  `type_::MONO_CAPTION.mono()`; add the
  §4.1 muted derivation if the builder confirms
  it (report the alpha chosen).
- **chrome_view.rs** — btn: literals px10/py5/
  radius6 stay (they ARE the charter metrics)
  but hover/press swap `control_track`/
  `control_off` → `t.hover`/`t.press`; accent
  via `design::accent()`. app_bar: TITLE name
  ✓ already; add the charter's dirty dot +
  hairline under bar/workspace row
  (`divider(t)`).
- **icon_btn.rs** — same hover/press + accent
  swaps in icon_btn / icon_only_btn / inert
  twins (accent sites :96/:125/:134/:302);
  ICON_SIZE 13 and ICON_LABEL_GAP 6 stay as
  kit-true constants; inert glyph colour stays
  `text_secondary` unless design lands §4.1.
- **menus.rs** — dropdown root → card styling
  (fill `t.card`, border `card_border`, radius
  LG, padding 6); item rows 28 high / px 8 /
  radius SM / hover `t.hover`; shortcut hints →
  `design::kbd`; separators → `design::divider`;
  accent sites :232/:347 → kit accent.
- **command_palette.rs** — retire
  `section_heading` (:1513) onto
  `design::section_label`; card root gains
  border + shadow (`t.shadow_soft`); footer
  hints → kbd chips.
- **toasts.rs** — toast cards: fill `t.card`,
  border `card_border`, radius LG, shadow
  `t.shadow_soft`; severity colours unchanged.
- **status_strip.rs** — segments separated by
  hairline ticks (1×10 `t.separator`); numeric
  segments → MONO_CAPTION(.mono()); the
  floor-width maths (LABEL_CHAR_W) must be
  re-derived for the mono style by its agent
  and the Phase K floor test re-pinned — flag
  this as the riskiest F-wave change.
  GLYPH_TEXT canvas hex: untouched (§4.7).
- **settings_popover.rs** — root becomes
  `panel_col("settings")` + `panel_title`;
  retire `section_label` (:551); the inner
  "settings-card" col merges into the one card
  shell (key survives on the root).
- **workspaces.rs** — chips: active = kit
  accent fill; blurb line stays SECTION/
  secondary per kit header idiom.
- **stage_view.rs** — stage radius LG; camera
  overlay → floating card pill (fill `t.card`,
  radius 999, px 10 / py 4, border
  `card_border`). SELECTION_ACCENT and all
  canvas colours untouched.
- **main.rs** — column gaps 12.0 → S3-named
  (value unchanged); timeline stack wrapped
  in the charter's timeline card (fill
  `t.card`, radius LG, padding 8) around the
  existing timeline_marks mount (:717); root
  fill `t.bg` ✓ already (:763).

### A-wave panels — retire private helpers

Each of these deletes its private
section/section_label helper onto
`design::section_label` and takes `panel_col`
+ `panel_title` + `divider` as its root:
easing_panel.rs (:267), export_panel.rs (:855,
incl. `section_with_icon` → panel_title's icon
slot), figure_add.rs (:154), fx_browser.rs
(:246), home_panel.rs (:946),
playback_controls.rs (:448), three_d_panel.rs
(:688). (settings_popover and command_palette
are F-wave, above.)

### A-wave panels — card + metrics adoption

Every remaining panel takes the card shell
(panel_col/header/divider) and moves its
panel-level gaps to S2/S3 values; control-
internal 6.0 gaps and all keys stay. By
current metric density (gap+px+py+radius
calls), the heavy files: inspector_style.rs
(33), fx_clips.rs (26), scene_panel.rs (25),
particles_panel.rs (20), text_tool.rs (19),
home_panel.rs (19), camera_panel.rs (17),
inspector_segment.rs (16), graph_panel.rs
(16), figures_panel.rs (15 — plus row_toggle
retires onto the kit toggle/button family,
§4.3), pose_tools.rs (14), inspector_figure
.rs (14), clips_panel.rs (14), stroke_
inspector.rs (11), stats_view.rs (11),
library_panel.rs (10), keyframe_tools.rs
(10), project_tools.rs (9), playback_
controls.rs (9), audio_panel.rs, draw_tools
.rs, frame_ops.rs (its `fbtn` copy retires
onto btn/icon_btn per charter §4 — metrics
already match; the Phase M flex_wrap row
keeps wrapping), help_panel.rs (kbd chips
for shortcut rows; its one `.family(Family
::Mono)` use becomes MONO_CAPTION.mono()),
inspector_figure.rs / inspector_segment.rs
(hex_color(0xF16F78) is a content colour —
leave), layers_panel.rs, onion_controls.rs,
frame_ops.rs, fx_browser.rs (done above),
waveform_view.rs (element chrome only;
WAVE_HEX/CAPTION_HEX canvas hexes leave),
timeline_marks.rs (the lane/ruler/chips are
charter-frozen geometry — only the card
wrapper in main.rs changes), fig3_preview.rs
/ stage_fx.rs / stage_layers.rs (canvas —
no adoption; SELECTED_COLOR/GIZMO_COLOR
leave per §4.7).

### Explicitly not adopted

- All canvas/Shape colours (stage, timeline
  chips geometry, waveform, FX, fig3) — §4.7.
- Engine (lib) files — none touched by
  Phase N at all.
- Test-code `tok::theme(true)` fixtures —
  tests keep building dark themes directly;
  no migration needed or wanted.
- i18n strings, status strings, key names —
  frozen by charter rule.

## 7. Decisions the F wave must not re-open

1. accent() = `tok::accent(0, true)` in both
   themes (charter-fixed; §5 records why and
   what the future flip needs).
2. Hover/press come from `t.hover`/`t.press`,
   not the control_track/control_off fakes —
   even though dark-mode contrast drops; that
   subtlety is the theme's intent.
3. Third text level is a derived alpha of
   `text_secondary`, never a new hex.
4. MONO_CAPTION gets `.mono()` in the kit,
   not a tok patch (rune is a pinned
   dependency; the shell does not fork it).
5. Spacing off-scale values (6, 2, 10, 5, 16)
   stay literals inside the kit/controls that
   own them; only panel-level layout claims
   token names (S2/S3/LG/MD/SM).
