# P11 — Typography & Copy Spec

Phase N planning agent P11. Charter: `docs/PHASE_N_CHARTER.md`
(§2 header, §3 section labels, §6 steppers, §9 menus/palette,
§11 kbd chips, §12 status strip, §15 app bar). This spec is
the typography and copy contract the F and A waves execute.
**It changes no code.** Every rule below is either (a) a
restyle — same string, same key, same handler, new
`type_::` style or kit component — or (b) explicitly marked
FROZEN, meaning a known inconsistency Phase N must not touch
because the string is test-pinned, i18n-owned, or
status-semantics.

Method: production code only (everything before each file's
first `#[cfg(test)]`), `src/rune_sec/*.rs` + `src/chrome_view.rs`
+ `src/stage_view.rs` + `src/main.rs` for type; `src/*.rs`
additionally for status strings. Counts are grep/python
censuses run 2026-11-08 against the Phase M tree (733 keys).

---

## 1. The type scale (what exists)

From the pinned Rune rev (`rune-src/crates/rune/src/tok.rs:136-143`):

| Style | Size / weight | Family | Shell uses today |
|---|---|---|---|
| `type_::BODY` | 13 / 400 | Sans | 19 refs |
| `type_::LABEL` | 12 / 500 | Sans | **266 refs (84 % of all)** |
| `type_::SECTION` | 11.5 / 600 | Sans | 10 refs |
| `type_::TITLE` | 13 / 600 | Sans | 23 refs |
| `type_::MONO_CAPTION` | 12 / 500 | Sans (see quirk) | **0 refs** |

318 `type_::` references in production code across 39 files;
305 `.font(` call sites. LABEL is the shell's answer to
everything — field labels, readouts, panel headings, menu
items, chips, strip segments, hints — which is why the UI
reads flat: there is almost no size/weight contrast anywhere.

Three facts the adoption wave must know:

1. **MONO_CAPTION quirk (pinned rev).** In this Rune rev,
   `MONO_CAPTION` is `TextStyle::new(12.0, 500.0)` — *the same
   size, weight and family as LABEL*; the `.mono()` builder
   is not applied to it. Adopting MONO_CAPTION for numeric
   readouts (§2) is therefore a **semantic** change today —
   zero visual difference — and becomes visual only if Rune
   later differentiates it. Do not "fix" this locally with
   `.family(Family::Mono)` on readouts: the one place real
   mono glyphs are used today is help key caps
   (`help_panel.rs:436`, `LABEL.family(Family::Mono)`), and
   §5 rule C7 covers what happens to it.
2. **Implicit BODY.** Rune's `text()` default style *is*
   BODY 13/400 (`rune-src/crates/rune/src/el.rs:639`). Any
   `text(...)` chain that sets `.color(...)` but never
   `.font(...)` renders at BODY whether the author meant it
   or not. Seven such sites exist, all in
   `stats_view.rs` (§4D).
3. **One sanctioned raw style.** `timeline_marks.rs:559`
   `RULER_LABEL_STYLE = TextStyle::new(10.0, 500.0).lh(12.0)`
   is the only raw `TextStyle::new` in the shell. It is
   geometry-bound (the label must fit a 12 pt ruler strip
   and a fixed chip slot) and documented in its own comment.
   **Keep it.** It is the exception that proves the rule:
   no new raw `TextStyle::new` anywhere in Phase N.

Text inputs: `text_input(.., type_::BODY)` everywhere, in two
spellings — `BODY.lh(26.0)` at 10 sites, plain `BODY` at 4
(§4E). The style belongs to the field's frame, which the
charter leaves to panels; the *spelling* should be one.

---

## 2. Type assignment table (THE contract)

One role → one style → one colour → one owner. "Kit" means
the element comes from `design.rs` / `chrome_view` and panels
never restyle its text themselves.

| UI role | Style | Colour | Owner / notes |
|---|---|---|---|
| App-bar project name | TITLE | `t.text` | chrome_view app_bar (already TITLE, `chrome_view.rs:90`); charter §15 adds the accent dirty dot beside it |
| **Panel header title** | **SECTION** | **`t.text`** | `design::panel_title` (charter §2). This *demotes* today's TITLE panel headings — §4B is the list. Distinguish from section labels by colour only |
| Panel header meta / count | LABEL | `text_secondary` | `design::panel_title` meta slot: `"{} clip(s)"`, `"12 of 30"` etc. |
| Section label inside a panel | SECTION | `text_secondary` | `design::section_label` (charter §3). All private helpers retire — §4A |
| Field label (beside a control) | LABEL | `text_secondary` | The label names the control; the value carries `t.text`. Today's majority practice (scene/inspector labels) |
| Stepper / cycler value | **MONO_CAPTION** | `t.text` | `design::stepper_value` (charter §6: centred, min-w 34). Today these are LABEL or BODY — §4C |
| Pure-numeric readout line (timecode, stage readout, zoom/angle, 3D coords, hex) | **MONO_CAPTION** | `t.text` | A line whose payload is numbers, dimensions or `#RRGGBB` |
| Prose readout line (words + numbers, e.g. FX clip readout, graph value line) | LABEL | `t.text` | Numbers inside a sentence do not make it a numeric readout |
| Button label | LABEL | white on active, `t.text`, muted inert | Inside `btn` / `icon_btn` / `fbtn`-successors only (charter §4); panels pass strings, never fonts |
| Menu title (File/Edit/View/Help) | LABEL | `t.text` | menus bar, already LABEL |
| Menu item label | LABEL | `t.text` | menus item rows |
| Menu / palette shortcut hint | **kbd chip** (MONO_CAPTION) | `text_secondary` on `control_off` | `design::kbd` (charter §9, §11) — replaces LABEL hint text, §4F |
| Palette command title | BODY | `t.text` (white on selected row) | Already BODY (`command_palette.rs:1597`) — keep |
| Palette group label (right side of row) | LABEL | `text_secondary` | Already LABEL — keep |
| Palette section heading ("Recent", "All commands") | SECTION | `text_secondary` | Via `design::section_label`; today LABEL (§4A) |
| Palette footer hint | LABEL words + **kbd chips** for the keys | `text_secondary` | Same words, split: `↑↓` `Enter` `Esc` become chips (charter §11) |
| List-row primary text | LABEL | `t.text` | Figures/library/layers rows already LABEL. Audio clip rows are BODY today — §4D |
| List-row meta / secondary text | LABEL | `text_secondary` | |
| Detail heading (inspector replaces panel: "Event 3 · Burst", "Graph — figure 2") | SECTION | `t.text` | It is a panel header in that state → `panel_title` treatment, §4B |
| Stat / totals cell value | TITLE | `t.text` | Keep (`stats_view.rs:366`). The one display-numeral role TITLE retains besides the app bar |
| Stat / totals row label + value | LABEL / MONO_CAPTION | secondary / `t.text` | Fixes the implicit-BODY rows, §4D |
| Timeline frame chip number | LABEL | `t.text` / white on current | Charter §13 keeps chip geometry; already LABEL (`timeline_marks.rs:851`) |
| Timeline ruler label | `RULER_LABEL_STYLE` (exception) | `text_secondary` | Do not touch (§1.3) |
| Timeline loop label / clips caption | LABEL | `t.text` | `timeline_marks.rs:937`, `waveform_view.rs:1100` — keep |
| Status-strip segment (words) | LABEL | `t.text` | `status_strip.rs` `segment()` — keep |
| Status-strip segment (numeric: timecode, "Frame 1/5", runtime, "1.5×", "Zoom 100%") | **MONO_CAPTION** | `t.text` | Charter §12. **Width-floor warning §4C** |
| Toast text | LABEL | `t.text` / severity colour | `toasts.rs:387` — keep |
| Workspace chip | LABEL | `t.text` / white on active | `workspaces.rs:468` — keep |
| Workspace blurb line | LABEL | `text_secondary` | `workspaces.rs:468` — keep |
| Empty state (whole-panel) | LABEL | `text_secondary` | `design::empty_state` (charter §10: 20px icon + LABEL). Text stays the panel's existing string |
| Inline hint / guidance ("Select a …", "uses the selected figure joint") | LABEL | `text_secondary` | Keep as text; not an `empty_state` unless it is the panel's only content |
| Text input content | BODY | (kit field) | One spelling: `BODY.lh(26.0)` for single-line panel fields — §4E |
| Help section title | SECTION | `t.text` | It heads a section inside the Help card → panel-header treatment; today TITLE (`help_panel.rs:465`) |
| Help description / shortcut action | LABEL | secondary / `t.text` | Already LABEL — keep |
| Help key cap | **kbd chip** | per `design::kbd` | Replaces local `key_cap` (§5 C7) |
| Confirmation headline ("Delete {name}?") | TITLE | `t.text` | Keep TITLE (`home_panel.rs:1027`): a prompt headline, not a panel header |
| Stage camera-overlay pill text | LABEL | `t.text` | Charter §14 pill; style only, overlay logic untouched |

Colour discipline that falls out of the table: **SECTION is
the only style whose meaning flips with colour** — `t.text`
= "this heads the panel", `text_secondary` = "this heads a
section inside it". No other style may appear in
`text_secondary` at header size, and TITLE never appears in
`text_secondary` at all.

---

## 3. Census summary (where each style lives today)

Per-style production totals: LABEL 266, TITLE 23, BODY 19,
SECTION 10, MONO_CAPTION 0.

- **LABEL** is heaviest in inspector_style (20),
  stroke_inspector (19), fx_clips (18), scene_panel (17),
  inspector_segment (14), inspector_figure (12),
  export_panel (11), three_d_panel (10) — i.e. exactly the
  dense inspector panels where §2 splits it into field
  label / stepper value / readout roles.
- **TITLE** sites: panel headings (audio_panel:687,
  easing_panel:306, export_panel:971, fx_browser:507,
  fx_clips:1186, help_panel:512, keyframe_tools:1115,
  library_panel:876, particles_panel:830,
  playback_controls:484, project_tools:730,
  scene_panel:958, three_d_panel:824), app bar
  (chrome_view:90), detail headings (graph_panel:1750,
  fx_clips:1280, particles_panel:579), list-item titles
  (home_panel:1070), a confirmation headline
  (home_panel:1027), a sentence-length section header
  (home_panel:1155), stat values (stats_view:366), the
  timecode readout (playback_controls:690), help section
  names (help_panel:465), and one glyph (the palette's
  "⌕" at command_palette:1559, `TITLE.lh(26.0)` — a 13 pt
  search glyph; leave it, it is an icon substitute, not
  text).
- **BODY** sites: text inputs (14, §4E), palette command
  titles (command_palette:1597), palette empty text
  (command_palette:1571), audio empty text
  (audio_panel:699) and audio clip-row labels
  (audio_panel:740), fx_clips cycler/stepper values
  (fx_clips:1324/1347/1370). Everything except inputs and
  palette titles is off-assignment — §4C/§4D.
- **SECTION** sites: the only correct section-label
  implementation (export_panel:858/874 `section()` /
  `section_with_icon()`), stats_view captions (366–496,
  six uses — the model pupil), onion_controls:120,
  clips_panel:971. SECTION is the rarest style and the
  charter's workhorse; Phase N inverts the LABEL/SECTION
  ratio for headings.
- **Private heading helpers** (all LABEL/secondary except
  export_panel's): command_palette.rs:1513
  `section_heading`, easing_panel.rs:267 `section`,
  export_panel.rs:855 `section` + :868 `section_with_icon`,
  figure_add.rs:154 `section_label`, fx_browser.rs:246
  `section` + :276 `section_with_icon`,
  home_panel.rs:946 `section_label`,
  playback_controls.rs:448 `section`,
  settings_popover.rs:551 `section_label`,
  three_d_panel.rs:688 `section_label`. Nine helpers, three
  names, two styles — all retire onto
  `design::section_label` / `design::panel_title`.

---

## 4. Off-style sites — the adoption work list

Restyle only. No string, key or handler changes anywhere in
this section. File:line is the Phase M tree position.

### 4A. Section labels in LABEL → `design::section_label` (SECTION, secondary)

Retire the helper, restyle its call sites:

- `src/rune_sec/command_palette.rs:1513` (`section_heading`
  fn) — calls :1577 "Recent", :1580 "All commands".
- `src/rune_sec/easing_panel.rs:267` (`section` fn).
- `src/rune_sec/figure_add.rs:154` (`section_label` fn) —
  calls :195 "Presets", :196 "Library", :199 "Library is
  empty" (that last one is an empty state wearing a section
  label's clothes — route it to `design::empty_state`,
  same string).
- `src/rune_sec/fx_browser.rs:246` (`section` fn) + :276
  (`section_with_icon` fn) — call :586 "Presets" and the
  five category headings.
- `src/rune_sec/home_panel.rs:946` (`section_label` fn).
- `src/rune_sec/playback_controls.rs:448` (`section` fn) —
  calls :488 "Transport", :532 "Loop playback", :608
  "Tween playback", :686 "Time".
- `src/rune_sec/settings_popover.rs:551` (`section_label`
  fn) — calls :684 "Theme", :720 "Language", :724
  "Stage size", :731 "Default easing", :834 "Playback",
  :872 "Startup workspace". (Its :676 "Settings" call is a
  panel title — §4B.)
- `src/rune_sec/three_d_panel.rs:688` (`section_label` fn).
- Already-correct reference, do not touch except to swap
  the local helper for the kit one with identical output:
  `src/rune_sec/export_panel.rs:855/868`, calls :1087
  "Project formats", :1160 "Bundles".

### 4B. Panel headings → `design::panel_title` (SECTION, `t.text`)

TITLE today — demote per charter §2:

- `src/rune_sec/audio_panel.rs:687` "Audio"
- `src/rune_sec/easing_panel.rs:306` "Easing"
- `src/rune_sec/export_panel.rs:971` "Export"
- `src/rune_sec/fx_browser.rs:507` "Effects"
- `src/rune_sec/fx_clips.rs:1186` "FX clips"
- `src/rune_sec/help_panel.rs:512` "Help"
- `src/rune_sec/keyframe_tools.rs:1115` "Keyframes"
- `src/rune_sec/library_panel.rs:876` "Library"
- `src/rune_sec/particles_panel.rs:830` "Particles"
- `src/rune_sec/playback_controls.rs:484` "Playback"
- `src/rune_sec/project_tools.rs:730` "Project"
- `src/rune_sec/scene_panel.rs:958` "Scene"
- `src/rune_sec/three_d_panel.rs:824` "3D"
- `src/rune_sec/home_panel.rs:1155` "Saved projects in
  ./projects" (a sentence doing a title's job — style
  change only, string frozen)
- Detail headings (same treatment): `graph_panel.rs:1750`
  "Graph — figure {}", `fx_clips.rs:1280` "{} · clip {}",
  `particles_panel.rs:579` "Event {} · {}".
- Help section names: `help_panel.rs:465` TITLE → SECTION
  `t.text`.

LABEL or SECTION-in-secondary today — promote colour to
`t.text` and route through `panel_title`:

- `src/rune_sec/figures_panel.rs:650` "Figures" (LABEL,
  `t.text` — style change only)
- `src/rune_sec/inspector_style.rs:956` "Style" (LABEL)
- `src/rune_sec/layers_panel.rs:562` "Layers" (LABEL,
  secondary)
- `src/rune_sec/draw_tools.rs:727` "Draw" (LABEL,
  secondary)
- `src/rune_sec/camera_panel.rs:916` "Camera" (LABEL,
  secondary, with 12 pt icon — becomes the charter §2
  header almost verbatim)
- `src/rune_sec/text_tool.rs:1224` "Text" (LABEL,
  secondary)
- `src/rune_sec/pose_tools.rs:1227` "Pose tools" (LABEL,
  secondary)
- `src/rune_sec/clips_panel.rs:970` "Movieclips" (already
  SECTION/`t.text` — kit swap only; its "{} clip(s)" meta
  at :976 goes to the meta slot)
- `src/rune_sec/onion_controls.rs:119` "Onion skin"
  (SECTION but secondary — colour change only)
- `src/rune_sec/settings_popover.rs:676` "Settings"
  (currently built by `section_label` — becomes the card's
  `panel_title`)

No heading exists — the card adoption adds `panel_title`
with the panel's established name (no new strings invented
beyond the name the workspace/status already uses):

- `src/rune_sec/inspector_segment.rs` (first text is the
  empty-state at :686)
- `src/rune_sec/inspector_figure.rs` (first text is the
  "Name" field label at :623)
- `src/rune_sec/stroke_inspector.rs` (first heading-like
  text is "All strokes", LABEL/`t.text` at :1345 — a
  list sub-head; it stays LABEL, the *panel* gets a title)
- `src/rune_sec/frame_ops.rs:575` "Frames" (LABEL,
  secondary, inline in the toolbar row — becomes the
  card header; the row keeps its buttons)

Keep TITLE, do not demote: app bar (chrome_view:90), stat
cell values (stats_view:366), delete confirmation
(home_panel:1027), palette search glyph
(command_palette:1559).

### 4C. Numeric values in LABEL/BODY/TITLE → MONO_CAPTION

Via `design::stepper_value` where the value sits between
−/+ buttons; direct `.font(type_::MONO_CAPTION)` for
standalone numeric lines:

- `src/rune_sec/settings_popover.rs:556` (`readout` fn —
  every settings stepper value flows through it)
- `src/rune_sec/scene_panel.rs:694` (stepper value), :713,
  :738 (`fps_label` readout), :770 (stage-size stepper
  value), :1024, :1061/:1066 (stage steppers), :1084
  (background `#RRGGBB` readout), and the `stage_readout`
  line itself ("960×720 · 4:3 · 12 fps",
  `scene_panel.rs:521`)
- `src/rune_sec/camera_panel.rs:755` (`zoom_label`), :764
  (`angle_label`)
- `src/rune_sec/stroke_inspector.rs:1490` (width value
  `{w:.1}`), :1533 (`opacity_label`)
- `src/rune_sec/inspector_segment.rs:588` and
  `src/rune_sec/inspector_figure.rs:644` (`readout_row` —
  value side only; the label side stays LABEL/secondary.
  Hex colour values in these rows are the clearest win)
- `src/rune_sec/three_d_panel.rs:653` (readout fn), :944
  and :985 (joint/camera readout lines, "x {} · y {} · z {}"
  format at :603)
- `src/rune_sec/fx_clips.rs:1324` (kind cycler value,
  BODY), :1347 (start `frame_label`, BODY), :1370
  (duration `frame_label`, BODY)
- `src/rune_sec/playback_controls.rs:690` (timecode
  readout, TITLE → MONO_CAPTION)
- `src/rune_sec/status_strip.rs:783` (`segment` fn), :813,
  :910, :987 — **numeric segments only** (timecode,
  frame, runtime, speed, zoom). Word segments ("Audio
  on", "3D", project name, figure name) stay LABEL.
  ⚠ **Width floors:** the strip's segment floors are
  computed by `est_text_width` from `LABEL_CHAR_W = 8.0`
  (`status_strip.rs:712`), a constant the Phase F/K
  overlap proofs are pinned to. In the pinned Rune rev
  MONO_CAPTION has LABEL's metrics, so floors stay valid
  *today*; the status-strip adoption agent must re-check
  the floor arithmetic in its report and must not change
  `LABEL_CHAR_W` silently.

### 4D. Implicit BODY and row-style drift

- `src/rune_sec/stats_view.rs:380/381` (`total_row_el`),
  :406/407 (`media_row_el`), :433/434 (`flag_row_el`),
  :467 (per-figure line) — colour-only `text()` chains
  rendering at implicit BODY 13 inside a block whose
  captions are SECTION 11.5. Assign: label LABEL
  secondary, count/value MONO_CAPTION `t.text` (totals,
  media, flags); the per-figure prose line LABEL `t.text`.
- `src/rune_sec/audio_panel.rs:740` — audio clip-row
  primary text in BODY; every other list row in the shell
  (figures_panel, library_panel:791, layers_panel) is
  LABEL. → LABEL.
- `src/rune_sec/audio_panel.rs:699` — "No audio clips in
  this project." in BODY; all other panels' empty texts
  are LABEL/secondary. → `design::empty_state` (LABEL).
- `src/rune_sec/command_palette.rs:1571` — "No commands
  match" in BODY. → `design::empty_state` inside the
  palette card (LABEL).

### 4E. Text-input spelling

Adopt `type_::BODY.lh(26.0)` at the four plain-BODY sites,
matching the other ten:

- `src/rune_sec/audio_panel.rs:906`
- `src/rune_sec/clips_panel.rs:773`
- `src/rune_sec/inspector_figure.rs:625`
- `src/rune_sec/project_tools.rs:754`

### 4F. Shortcut hints in LABEL → `design::kbd`

- `src/rune_sec/menus.rs:246/251` (item builder hint),
  :365/369 (item row hint) — "Ctrl+S" etc. become kbd
  chips; the hint *strings* come from `shortcut_hint`
  unchanged.
- `src/rune_sec/command_palette.rs:1618` — footer line
  "↑↓ navigate · Enter run · Esc close": same words, the
  three key names become chips.
- `src/rune_sec/help_panel.rs:431` (`key_cap` fn) — retire
  onto `design::kbd`; see rule C7 for the mono caveat.

---

## 5. Casing rules

### The evidence

Casing in the shell is already unusually disciplined —
Phase N's job is to *name* the rule so adoption doesn't
break it, not to fix it:

- Multi-word button labels extracted from
  `btn` / `icon_btn` / `fbtn` call sites: **113 of 113
  sentence case** ("Swap with next", "Copy frame",
  "Normalise all", "Trim silence", "Delete particle
  event"). Zero Title-Case buttons.
- Menu items (menus.rs): sentence case ("Export PNG",
  "Delete stroke", "Swap frame with next", "Mute audio
  clip").
- Command palette titles (108 commands,
  command_palette.rs:960+): sentence-case verb phrases
  ("Go to first frame", "Toggle loop playback", "Save
  named project"); group names are single capitalised
  nouns ("Transport", "Frames", "Workspaces").
- Section headings at call sites: sentence case
  ("Project formats", "Loop playback", "Stage size",
  "Default easing", "Startup workspace", "All commands").
- No ALL-CAPS visible text anywhere in the shell. (The
  one ALL-CAPS string in the codebase, the i18n value
  `figures_layers = "FIGURES (layers)"`, is never rendered
  by the Rune shell — and i18n values are frozen anyway,
  §7.)

### The rules

- **C1. Sentence case for everything multi-word**: buttons,
  menu items, palette titles, panel titles, section
  labels, field labels, hints, empty states. Capitalise
  the first word only, plus proper nouns and acronyms.
- **C2. Single-word labels take an initial capital**
  ("Reverse", "Duplicate", "Fit", "Centre"). A single
  word is sentence case degenerate — never ALL CAPS,
  never lowercase.
- **C3. Acronyms and format names keep their canonical
  form** inside sentence-case strings: FPS, FX, 3D, PNG,
  GIF, MP4, SVG, WebM, MP4 ("Export MP4 sequence",
  "Increase FPS"). "WebM" and "3D" are not "Webm"/"3d".
- **C4. A referenced control name keeps the control's own
  casing** when prose points at it: "…then Key angle
  starts the first one." (graph_panel.rs:1533/1798 — "Key
  angle" is a button's name, not a casing error).
- **C5. Symbols, glyphs and pure numbers are exempt**
  ("−", "+", "×2", "90%", "-15°", "1:1", chip numbers).
- **C6. en-GB spelling is the house spelling** in visible
  copy: Normalise (audio_panel:821/886, palette :1077 —
  zero "Normalize"), Colour (7 heading sites — zero
  visible "Color"), Centre (camera_panel:928/995,
  inspector_figure:797, menus:849 — zero "Center").
  Adoption must not "correct" these, and any *new* string
  a plan explicitly authorises follows en-GB.
- **C7. Kbd chips**: key names render as written on the
  key or in `shortcut_hint` ("Ctrl+S", "Ctrl+Shift+Z",
  "O", "Enter", "Esc", "↑↓"). The kit `kbd` uses
  MONO_CAPTION per charter §11; in the pinned rev that is
  metrically LABEL (§1.1), so help's caps lose their
  current `Family::Mono` glyphs (help_panel:436) when
  they move to the kit. That is a *visible* change the
  foundation agent must call out in its report — it is
  not a licence for panels to keep private mono caps.

### Frozen casing deviations (record, do not fix)

- "Movieclips" as one word (clips_panel:970) vs the
  engine's "Movieclip" in status strings — one word is
  the panel's established name; leave both.
- Terse abbreviations on buttons: "Norm"
  (audio_panel:744, beside full "Normalise" at :821),
  "Dur -" / "Dur +" (frame_ops:731/734). Expanding them
  changes pinned strings and button widths; frozen.
- "uses the selected figure joint" (fx_clips:1518) — the
  shell's only lower-case-start visible text, a trailing
  hint fragment beside its button. Style (LABEL
  secondary) already matches §2; casing frozen.
- "FX clips" (fx_clips:1186) vs "FX clip" singular in
  readouts — number, not casing; frozen.

---

## 6. Number & format rules

Formats are owned by helper functions, several test-pinned.
Adoption restyles the *rendering* (usually to MONO_CAPTION)
and never re-derives a format inline.

- **N1. Timecode is `M:SS.t`** — minutes uncapped,
  two-digit seconds, one tenth, rounded once half-away
  from zero. Two implementations exist and already agree:
  `playback_controls::format_timecode`
  (playback_controls.rs:418) and `status_strip::timecode`
  (status_strip.rs:402). The strip's total-runtime label
  is the `M:SS` no-tenths variant (`runtime_label`,
  status_strip.rs:446). Never write a third formatter.
- **N2. Frame counts in prose are 1-based** ("Frame 1/5",
  "starts frame 3", loop "Loop 2–4" with an en dash,
  status_strip.rs:569). Internal 0-based values never
  reach text.
- **N3. Percentages are whole**: `{:.0}%` ("Zoom 100%",
  "90%"/"110%" scale buttons, opacity readouts). No
  decimal percentages anywhere; do not introduce one.
- **N4. Multipliers use ×, trimmed**: "×2", "×0.5",
  "Speed 1.5×" (`playback_controls::speed_label`:316 —
  two decimals trimmed), duration "{:.2}x" in engine
  status. The letter "x" as a multiplier is wrong; as a
  *dimension separator* it currently appears in status
  strings ("Stage size: {} x {}", scene_panel.rs:477,
  settings_popover.rs:317) while the readout line uses
  "×" (`stage_readout`, scene_panel.rs:528: "960×720").
  Both are frozen strings (status semantics / pinned
  readout); the rule binds only the *style* (§4C) — flag
  the ×/x split as a known wart for a future copy phase,
  do not unify it in Phase N.
- **N5. Rates use `fps_label`** (scene_panel.rs:497):
  whole rates as integers ("12"), fractional at one
  decimal ("23.5") — including its pinned quirk
  29.97 → "30.0". The word is lower-case "fps" inside
  readout lines ("12 fps") and "FPS" only as a label /
  in palette titles ("Increase FPS", playback "FPS −/+"
  buttons, playback_controls:582/585).
- **N6. Decimal precision by quantity**: angles whole
  degrees with ° ("-15°", "{:.0}°"); lengths/positions
  `{:.0}` in readouts, `{:.1}` for stroke width
  (stroke_inspector:1490) and graph values
  (graph_panel:759); opacity as a whole-percent label
  (`opacity_label`, stroke_inspector.rs, pinned "58%"
  rounding). Reuse each panel's existing label helper —
  precision is part of its contract.
- **N7. Counts**: plain integers in headers/meta via the
  established lazy-plural forms "{} clip(s)" /
  "{} event(s)" (audio_panel:690, clips_panel:976,
  fx_clips:1188, particles_panel:832) and
  `figures_panel::count_line` ("{total}", or "{shown} of
  {total} figures" when filtered, figures_panel.rs:214;
  library_panel.rs:350 is a second copy whose unfiltered
  form differs — "{total} figures"). Thousands separators
  exist only in stats (`stats_view::format_count`:69,
  comma-grouped). Rules: header meta keeps the "(s)"
  convention (it is test-pinned in status strings too,
  §7); do not add separators outside stats; do not
  "correct" the two `count_line` copies in Phase N —
  flag for the dedup plan (P03) instead.
- **N8. Byte sizes have two live formatters and they
  disagree**: `project_tools::human_size` (project_tools
  .rs:318 — *decimal* KB/MB, no GB) vs
  `export_panel::format_size` (export_panel.rs:709 —
  *binary* KiB/MiB/GiB). The same file can read different
  sizes in Home rows and Export history. **Phase N does
  not pick a winner** (both are pinned by their own
  tests; changing either changes visible strings).
  Recorded here so no adoption agent "harmonises" them
  by accident; the choice belongs to a copy phase with
  the user's say.

---

## 7. Status-line microcopy (tone spec)

Census: 744 status-string literals in production code
(engine + panels). 609 (82 %) end with a full stop; 722
start with a capital; **zero** end with "?" or "!".

The established grammar — adoption must preserve it
exactly, because charter rules freeze status strings:

- **S1. Confirmation = past tense, full stop.**
  "Figure duplicated." "Deleted project file {name}."
  "Exported {n} PNG frames … to {}." The user is told
  what *happened*, never congratulated.
- **S2. Refusal = the missing thing first.** Two shapes:
  "No …" ("No such figure.", "No camera key before this
  frame.", "Nothing to redo.") and "Select … first, then
  …" for a missing selection ("Select a joint first, then
  Delete key.", keyframe_tools.rs:458). 123 strings start
  "No ", 30 with "Select", 13 with "Nothing".
- **S3. Failure = "{Op} failed: {reason}".** 165 strings
  contain "failed" ("Audio fade failed: {e}", "Export
  failed: {e}", "Paste pose failed: copied pose has {}
  angles…"). The reason follows the colon, unpunctuated
  engine text and all.
- **S4. Readout status = "Label: value."** 189 strings
  match the colon-readout shape ("Weather: off.",
  "Stage size: {} x {}.", "Stroke {} opacity {}.").
- **S5. No exclamation, no questions, no "please", no
  emoji, no second person imperatives outside the
  "Select … first" shape.** Tone is a lab notebook, not
  a coach. Any Phase N agent tempted to "friendlify" a
  string is out of scope by charter ("never change a
  status string").
- **S6. Known deviation, frozen:** the no-full-stop
  minority clusters in engine file ops ("Saved to {}",
  "Opened {}", editor.rs:1540/1566) and audio DSP
  failures ("Audio fade failed: {e}", audio_ops.rs —
  16 of its strings). They are engine strings with
  pinned tests; the strip *renders* them, Phase N does
  not edit them.
- **S7. Rendering is typography's only stake here:** the
  strip shows `core.status` in the tail segment (LABEL,
  `t.text`, truncating). Under §2 it stays LABEL — the
  status is prose, not a numeric segment, even when the
  string contains numbers.

---

## 8. i18n flags — labels that are also keys

The table has 78 keys × 28 languages (`src/i18n.rs`, EN
table at :260). The shell's `tr()` surface is far smaller
than the key list suggests:

- **Actually translated at runtime (the ONLY `tr()` call
  sites in production):** `chrome_view.rs` app bar —
  `save`, `load`, `undo`, `redo` (:94–103); transport —
  `play`/`pause` (:116), `prev_frame` (:125),
  `next_frame` (:146), `add_frame` (:160),
  `duplicate_frame` (:167), `delete_frame` (:174); and
  the toast Undo action (`toasts.rs:324`, `undo`).
  **These twelve keys' strings are owned by the i18n
  table.** Phase N may restyle the buttons; it must not
  touch the label expressions at all — not even to
  "match" a hardcoded twin elsewhere.
- **`SHELL_KEYS`** (`i18n_wire.rs:96`, 30 keys) is the
  shell's *intended* surface; coverage is complete, but
  most of its keys are **not** wired to `tr()` — the same
  words are hardcoded literals at other sites. Phase N
  must not wire them up (that changes language
  behaviour — an engine-visible change, out of charter)
  and must not rename keys to fit new copy.
- **Hardcoded twins — labels whose text equals an i18n
  English value but are literals (flagged, frozen):**
  - "Frames" — `frame_ops.rs:575` (key `frames`)
  - "Inbetween" — `frame_ops.rs:579` (key `inbetween`);
    also "Copy frame" / "Paste frame" icon_btns in the
    same file (keys `copy_frame` / `paste_frame`)
  - "Figures" — `figures_panel.rs:650` (key `figures`)
  - "Settings" — `settings_popover.rs:676` (key
    `settings`)
  - "Help" — `help_panel.rs:512` (key `help`)
  - "Easing" — `easing_panel.rs:306` (key `easing`)
  - "Export" — `export_panel.rs:971` (cf. `ws_export`)
  - "Audio" — `audio_panel.rs:687` (key `ws_audio`);
    "Effects" — `fx_browser.rs:507` (key `ws_effects`);
    workspace chips themselves are literals in
    `workspaces.rs:137` `label()` — including "3D" for
    Build3D, where the table has *both* `ws_build`
    ("Build") and `ws_3d` ("3D")
  - "Frame {}/{}" and "Zoom {}%" — `status_strip.rs:508/
    515` compose their own strings; `frame` / `zoom` are
    SHELL_KEYS but the strip never calls `tr()`.
  - "Onion skin" — `onion_controls.rs:119` (key
    `onion_skin`); its toggle button reads "Onion".
- **Rules for adoption:**
  - I1. Never rename, add or remove a `tr()` key; never
    edit an i18n value in any language.
  - I2. Never convert a hardcoded twin to `tr()` or a
    `tr()` label to a literal. Restyle the element
    around the label expression, byte-identical.
  - I3. When a panel_title/section_label adoption touches
    a twin site (several are in §4B), the string
    argument moves verbatim — including "Saved projects
    in ./projects" and "Movieclips".
  - I4. Menu labels "Save"/"Load"/"Undo"/"Redo"
    (menus.rs:663+) are *separate literals* from the app
    bar's `tr()` versions that happen to match in
    English. They do not translate today. Frozen as-is;
    the discrepancy is recorded, not repaired.

---

## 9. What this spec does not decide

- Whether panel titles *should* be SECTION (charter §2
  says so; this spec executes it). If the foundation
  wave's `panel_title` lands TITLE instead, §4B's colour
  and routing rules still apply unchanged — only the
  style column flips. The integrator owns that call.
- The byte-size formatter split (§6 N8) and the ×/x
  split (§6 N4): recorded, deferred to a copy phase.
- New strings: Phase N invents none. Every text an
  adoption agent renders already exists in the Phase M
  tree; §4B's heading-less panels reuse the name the
  shell already uses for that panel elsewhere (status,
  workspace blurb, or palette group).
