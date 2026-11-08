# S18 — Accessibility & Usability Audit (Phase 1)

Squad 18 scope: text contrast, hit targets, keyboard truth
and discipline, error-message quality, screen-reader
semantics, and beginner discoverability of the brief §8's
eight core tasks. `tab_visits_controls` stays OFF (standing
user decision) — this audit works around that fact and
never proposes enabling it.

## Method and evidence

- **Rendered ground truth:** the 13 headless tree dumps in
  `/tmp/hl-n3/headless-probe-output/tree-*.json` (Phase N
  final headless run). Every element carries its rendered
  sRGB `fill`, text `color`, `rect`, `role`, plus a full
  accessibility tree (roles, names, bounds, actions). All
  "rendered" numbers below were computed from those dumps:
  text contrast = the text element's rendered ink colour
  against its ancestor fills composited in linear space.
  4,062 text elements and 2,579 buttons were measured.
- **Token maths:** OKLCH → linear sRGB (standard Ottosson
  matrices) over the `rune::tok` theme values, compositing
  in linear space per the token file's own note. Token
  maths and rendered dumps agree wherever both exist
  (white-on-accent: 3.14 computed, 3.14 rendered; kbd chip:
  2.89 computed, 2.88 rendered), so the computed-only
  numbers can be trusted.
- **Honesty limit:** all 13 trees are the DARK theme
  (`dark_system: true`). No light-theme tree exists, so
  every light-theme number below is token-computed, not
  rendered. Closing that gap is item S18-13.
- Bars used: WCAG AA text 4.5:1 (no shell text qualifies
  as "large" — BODY is 13 pt, LABEL 12 pt, SECTION
  11.5 pt), non-text UI indicators 3:1, hit targets
  WCAG 2.5.8 AA floor 24×24 pt (AAA 2.5.5 is 44×44).

## 1. Contrast — what passes

The base reading experience is excellent and is not the
problem:

| Pair (rendered, dark) | Ratio |
| --- | --- |
| text on bg | 16.35:1 |
| text on card | 14.74:1 |
| text_secondary on bg | 7.51:1 |
| text_secondary on card | 6.77:1 |
| text on selected_fill (accent @ 16%) | 8.69:1 |
| Danger family, white on fill / hover / press | 5.03 / 6.14 / 7.55:1 |
| Waveform caption `0xA2AAB8` on card (dark) | 7.18:1 |
| Toast text (t.text on t.card) | 14.74:1 |

The Phase N danger deepening (0xC93A44 family) verifies
exactly as designed. Body and secondary text pass AA with
wide margins in both themes (light: 15.14–17.10 and
5.31–6.00).

## 2. Contrast — the failures

138 of 4,062 rendered text elements (3.4%) sit below
4.5:1. They cluster into four causes, worst first.

### 2.1 The kbd chip — 2.88:1 everywhere, 1.18:1 on the selected palette row

`design::kbd` renders shortcut hints as `text_secondary`
ink on a `control_off` chip fill. Rendered measurement
(dark): **2.88:1** — in the File menu ("Ctrl+S"), the
palette footer ("Enter", "Esc", "↑↓"), palette command
hints ("Space", "N", "I", "Left", "Right") and help rows.
This is real, must-read text (it teaches the keyboard
model) at roughly the contrast of the retired danger red
the project already fixed once.

On the command palette's SELECTED row the chip is drawn
over the accent fill and the same ink collapses to
**1.18:1** (rendered, "Space" hint) — effectively
invisible. Phase N's pixel pass noted this pair
qualitatively; it is now measured, and it is the worst
text contrast in the application.

### 2.2 White on accent — rendered 3.14:1, and the documented 4.19:1 does not reproduce

Every active state in the shell — workspace chips
("Animate · Alt+1"), active buttons ("Loop: on", "Onion",
"Pen", "Solid"…), active timeline frame numbers, menu
active items, graph channel chips — is white LABEL text
on `tok::accent(0, true)` = oklch(0.66, 0.17, 256).
Token maths gives 3.14:1; the rendered dumps give 3.14:1.
The project record carries this pair as a "documented
acceptance" at **4.19:1**, a number neither method
reproduces on the current tree. The acceptance may have
been measured against an older accent value; either way
the standing record overstates the shipping contrast by a
full point, and the pair fails the 4.5:1 text bar in
every active control in the app. This needs a fresh,
explicit decision with the true number (S18-3), not a
silent re-acceptance.

Related, same root: the menu's active-item shortcut hint
is white at 75% alpha on the accent — **2.60:1**
(computed), worse than the label it sits beside.

The palette's selected row (accent @ 0.85 over card,
white BODY title) renders at **3.58:1** in dark — and
**2.38:1 computed in the light theme**, where the card
underneath is white and lightens the blend. The most
important row in the keyboard user's main tool is the
light theme's worst full-text pair.

### 2.3 The light theme's dark-tuned constants

Several colours are fixed hexes chosen for dark and used
unchanged in the light theme:

| Pair (computed, light) | Ratio |
| --- | --- |
| Waveform caption `0xA2AAB8` on white card | 2.34:1 |
| Accent as ink on light bg (status-strip Play segment) | 2.78:1 |
| Toast severity glyphs on white card: Warn / Success / Info / Error | 2.16 / 2.63 / 2.81 / 3.91:1 |
| Toast severity bars on white card | 2.04–3.16:1 |
| Graph `GLYPH_MUTED` 0x707B8C on white card (decorative) | 4.28:1 |

The toast colours confess it in source: `Severity::color`
is documented "tuned for dark" and serves both themes.
Severity is carried by colour + glyph shape; in the
light theme the colour channel drops under the 3:1
non-text floor for three of four severities.

A structural contributor: the shell pins
`tok::accent(0, true)` — the DARK accent — in both themes
(`design::accent()`, `chrome_view::btn`, timeline chips,
status strip). The theme-correct light accent
oklch(0.60, 0.17, 256) would put white-on-accent at
3.99:1 and lift every accent-ink use on light surfaces.
The pin also means the light theme's accent is lighter
than its own theme intends.

### 2.4 Latent trap: `design::text_muted`

`text_muted` (text_secondary @ 0.6 alpha) still has zero
consumers. If a future panel adopts it, it lands at
4.46:1 in dark and **2.00:1 in light** — under every bar.
Retire it or guard it before someone uses it (S18-15).

Watch item, not a defect: `tok`'s `indicator`
(oklch 0.43) measures 2.30:1 on the dark bg. It is a Rune
kit control colour, not app text; noted so the mission
does not "discover" it later as an app bug.

## 3. Hit targets (rendered, 2,579 buttons)

The shell is a deliberately dense 25 pt interface:
`btn`/`icon_btn` render 25 high, `icon_only_btn` 26×26,
timeline chips 30×24, swatches 24×24, toggle rows 28.
That standard passes the 24 pt AA floor — barely, and
98.8% of buttons are under 28 high, so there is no slack.
Four classes fall UNDER the 24 floor:

| Target | Size | Instances |
| --- | --- | --- |
| `marker-graph-*` timeline graph-key markers | 30 × **14** | every graph key |
| `fig-sel-*` figure select cluster | 44.5 × **15** | every figure row |
| `fig-op-up/dn-*` opacity steppers | 20.5 × **21** | every figure row |
| `chip_btn` family (px 6 / py 3) | **21** high | settings, easing, style chips… |

142 button instances across the scenes render under
24 high; 26 under 24 wide. Two deserve plain words:

- **fig-sel (15 pt):** the figure row is 28+ tall, but
  only the name cluster inside it is pressable
  (`figures_panel.rs`, the `fig-sel-{fi}` row). The
  row's remaining height — thumbnail, opacity readout —
  does nothing. Selecting a character, a core task,
  means hitting a 15 pt band.
- **marker-graph (14 pt):** the timeline's graph-key
  markers are the smallest targets in the app and they
  are precision objects users drag/select repeatedly.

Positives: bare `design::toggle` (28×16) has zero
consumers — the 16 pt pill is only ever reached through
the 28 pt `toggle_row`. Steppers elsewhere
(`layer-op-dec/inc`) render 25–26 high, at standard.

## 4. Keyboard — truth, coverage, Esc

### 4.1 Shortcut truth: PASS, zero inventions

Every shortcut shown anywhere in the UI was checked
against `src/main.rs::on_key` and
`command_palette::palette_handle_key`:

- Help panel: **24/24** true-key rows real (Ctrl+Z,
  Ctrl+Shift+Z, Ctrl+Y, Ctrl+S, Ctrl+K, palette
  Down/Up/Enter/Escape, Delete, Alt+1–8, C, O, Space,
  Left, Right, N, I).
- Menu hints: **4/4** real (Ctrl+S / Ctrl+Z /
  Ctrl+Shift+Z / Ctrl+K).
- Palette command hints: **18/18** real.
- Workspace chip labels ("Animate · Alt+1"…): all 8
  match the Alt+digit handler and the workspace order.
- A full sweep for `Ctrl+`/`Alt+` literals in view code
  found no other displayed shortcut. Nothing shown is
  fake; everything real is documented in Help.

### 4.2 Coverage: the palette is the bridge (by design)

With Tab-cycling off (standing decision), the rendered
trees show **1–5 focusable elements per scene out of
100–282 buttons** — essentially the text inputs. Panel
buttons, steppers, swatches and row actions are
pointer-only. The keyboard story that does exist is
real: 24 global shortcuts for the core loop plus the
command palette (Ctrl+K, 108 commands, fully driven by
↑/↓/Enter/Esc, with recents) — the palette is the
designed keyboard path to almost everything, and it
works. The residual gap, stated plainly: a control that
has neither a global shortcut nor a palette command is
unreachable by keyboard. A coverage census of that set
belongs to implementation planning, not this audit.

`focusVisible` is 0 in all 13 scenes — but the scenes are
pointer-scripted, so this is absence of evidence, not
proof of a missing focus ring. Rune supports focus
visibility; whether the shell shows it needs a
keyboard-driven scene to settle (S18-13 covers the
light-theme half; a focus scene is the other half).

### 4.3 Esc discipline: good, one ordering nit

- Palette handler takes first claim in `on_key` —
  topmost-first ordering is correct.
- Esc closes the palette ✓, an open menu ✓.
- Esc mid-drag cancels and restores (camera, timeline,
  graph handlers all treat a cancelled Up as undo of the
  drag) ✓ — this is professional-grade discipline.
- Nit: the menu-Esc branch sits AFTER the
  `text_focused` early return in `on_key`, so with a
  text field focused Esc never reaches an open menu.
  Rare (menus rarely coexist with field focus) but it is
  an ordering bug in the making (S18-12).
- Help and Settings are right-column panels, not
  overlays — no Esc expectation applies; noted so nobody
  files it as a defect later. Toasts are dismiss-button
  (26×26) + auto-expire (TTL) only; acceptable.

## 5. Screen-reader semantics — plumbing works, naming doesn't

Rune emits a real accessibility tree (roles, names,
bounds, click actions) and text-labelled buttons are
named correctly from their labels. But in the default
scene **47 of 229 buttons (21%) have no name at all**;
fx-split-mid has 63. The unnamed are structural:

- Every `icon_only_btn` — the builder takes NO label
  parameter, so fig-eye / fig-lock / fig-del,
  layer up/down, fx nudge/kind, camera prev/next and
  toast-dismiss can never be named without a builder
  change.
- Every colour swatch (fig-color-*, seg-color-*,
  text-color-*, fx-color-*) — a screen reader hears
  "button" for a colour choice.
- Sliders (`layers-opacity-slider`) — no name, no value.

The engine already provides the whole API —
`El::label()`, `.a11y_value()`, `.checked()`,
`.range_value()`, `.disabled()` in rune's `el/a11y.rs` —
and the shell uses it **nowhere** (0 call sites). Two
consequences beyond names: toggle-state buttons (Onion,
Loop, follow modes) expose no checked/pressed state, so
their state is invisible to assistive tech; and inert
buttons, which are correctly un-wired, are not marked
disabled either.

## 6. Error and feedback quality

The engine's status grammar is genuinely good — sampled
571 distinct status strings:

- Failures name the op and the reason: "Audio compress
  failed: output would overwrite the source."
- Refusals often name the remedy: "No dialog tool
  (zenity/kdialog) on this system — use Save project
  instead.", "3D library is empty — save a 3D figure
  first."
- The toast carries the full text even where the status
  strip's hint segment truncates (by design, capped).

Defects, all small:

1. **Five bare failures with no reason** — "Audio
   crossfade failed", "Audio normalise failed", "Audio
   split failed", "Audio trim failed", "FX preset apply
   failed" — against a family grammar where every sibling
   carries ": reason". A user learns something went
   wrong and nothing else.
2. **`classify_status` mis-severities.** Warn matches
   only `No `/`Nothing ` prefixes, so ≈13 refusal shapes
   classify as neutral Info: "Frame {} cannot be swapped
   with itself.", "Clip {} is already muted.", "A
   movieclip needs at least one frame.", "Keep at least
   one figure.", "Lasso needs at least 3 points; nothing
   erased.", "Every frame in the range already has a
   key.", "3D library is empty…", the frame_ops refusal
   "First frame: cannot shift it earlier." Severity is
   signalled by colour + icon, so a refusal dressed as
   Info misleads precisely the users who read the colour
   channel first.
3. **One false Warn:** "No audio clips needed
   normalising to peak 0.90." is a benign no-op outcome
   wearing the refusal colour.

## 7. Beginner discoverability — the brief §8's eight tasks

| Task | Findable? | Evidence |
| --- | --- | --- |
| Create a project | ✓ | Home workspace, "New project" button, blurb "Open, create and manage projects…" |
| Add a character | ✓ | Figures panel + add-figure in the default Animate view |
| Pose a character | ✓ | App starts in Animate with a figure on stage |
| Create frames | ✓ | Transport add-frame, timeline, N shortcut |
| Play animation | ✓ | Transport play, Space, status-strip play segment |
| Edit curves | ✓ | Graph workspace; blurb: "Edit the motion curves and easing…" |
| Add effects | ✓ | Effects workspace; blurb: "Add and edit particle and visual-effects clips…" |
| Export | ✓ | Export workspace; blurb: "Render and export the project to images, video and other formats." |

All eight workspace blurbs are task-first sentences shown
under the chips on every switch — this is the app's
orientation system and it is good. Help (54 rows,
including plain-language Features prose) is one menu
away; the palette makes every command searchable by
name. Home's empty state even teaches: "No saved
projects yet — use \"Save project\" to add one."

Residual gaps:

- First run lands in the full Animate workspace — 229
  buttons, no guided first step. Defensible for a pro
  tool; the cost falls on true beginners (the blurb line
  is the only orientation). No onboarding exists and
  none is faked — recorded, not scored as a defect.
- The project_tools list line is still the bare "No
  saved projects in ./projects/ yet." with no next
  action (Phase N remainder; home_panel's version of
  the same state does it properly) — S18-14.
- File menu's only export is "Export PNG" — a beginner
  hunting "Export video" in File finds nothing; the
  Export workspace is the real home. Honest but narrow;
  a menu pointer item is a product call, not an audit
  fix.
- Home is the LAST workspace chip (Alt+8) although it
  holds project creation — the §8 list's first task
  lives behind the last tab. Mitigated by project_tools
  in Animate's left column; recorded as a findability
  observation for Squad 05/06, not a defect claim.

Long-session comfort notes: Reduce Motion is honoured
(press_scale and transitions gate on the host setting;
trees record the flag), themes persist, and error text
is never colour-only (every toast pairs colour with a
severity glyph and full text). No text-size / UI-scale
setting exists in the settings card — users who need
larger text have no in-app lever; recorded as a gap for
the mission to weigh, since BODY/LABEL at 13/12 pt is
small for low-vision users.

## 8. Items (each sized for one coder)

Foundation / kit:

- **S18-1 — kbd ink.** `design::kbd`: raise chip ink so
  hint text clears 4.5:1 on the chip in both themes
  (primary text on a slightly deepened chip fill, or
  secondary ink on a stronger fill — one builder, every
  menu/palette/help surface inherits). Proof: tree-dump
  contrast re-measure + kit test.
- **S18-2 — kbd on selected rows.** A selected-row chip
  variant (light ink on transparent/accent-deep chip)
  adopted in `command_palette`'s selected row and the
  menu active item (replacing white@0.75 plain text,
  2.60:1). Proof: palette-open + menu-open trees.
- **S18-4 — theme-correct accent.** Stop pinning
  `accent(0, true)`: thread the theme through
  `design::accent` and the `chrome_view::btn` family,
  timeline chips and the status play segment so light
  uses oklch(0.60, …). Dark rendering unchanged.
- **S18-8 — `icon_only_btn` names.** Add a label
  parameter wired to Rune's `El::label()` in
  `icon_btn.rs`; inert twin likewise.
- **S18-9 — chip height floor.** `chip_btn` to a 24 pt
  minimum height (padding or min_h) in `icon_btn.rs`.

Panel adoption (one file cluster each):

- **S18-5 — waveform caption.** `waveform_view.rs`:
  replace fixed `CAPTION_HEX` with the theme's
  secondary text colour (7.18:1 dark today; fixes the
  2.34:1 light failure).
- **S18-6 — toast severity per theme.** `toasts.rs`:
  `Severity::color`/`icon_color` take the dark flag;
  light variants deepened to ≥3:1 on a white card.
- **S18-7a — marker targets.** `timeline_marks.rs`:
  graph-key marker hit height 14 → 24 (visual mark may
  stay slim; the pressable grows).
- **S18-7b — figure row target.** `figures_panel.rs`:
  make the whole list row the select pressable (or grow
  `fig-sel` to the row's full height); opacity steppers
  to ≥24 in both dimensions.
- **S18-10 — names & states adoption.** After S18-8:
  name the icon-only clusters (figures, layers, fx,
  camera, toast dismiss), give swatches colour names,
  `range_value` on the kit sliders, `checked()` on
  toggle-state buttons (Onion, Loop, follow modes).
- **S18-14 — project list empty state.**
  `project_tools.rs`: the bare list line becomes the
  kit `empty_state` with home_panel's next-action
  wording.
- **S18-15 — retire or guard `text_muted`.**
  `design.rs`: remove it, or document the 2.00:1 light
  figure at the fn and gate adoption on a contrast
  test. Tiny, prevents a future defect.

Engine strings / classification (editor side):

- **S18-11a — classifier grammar.** `toasts.rs`
  `classify_status`: extend Warn to the refusal grammar
  (contains "cannot", "already", "needs at least",
  "Keep at least", " is empty"); special-case the
  normalise no-op out of Warn. Tests pin each shape.
- **S18-11b — bare failure reasons.** The five bare
  "… failed" strings in audio_ops / fx_edit gain their
  ": reason" suffixes, matching the family grammar.

Integrator / verification:

- **S18-3 — re-decide white-on-accent with the true
  number.** The record says 4.19:1 accepted; the tree
  says 3.14:1. Integrator + final acceptance must
  either (a) deepen the ACTIVE-state fill so white
  clears an agreed bar, or (b) re-accept at the
  measured 3.14:1 and correct PHASE_N.md / QA_RUNE.md.
  Not a coder item until decided — flagged here because
  every squad's active states inherit it.
- **S18-12 — Esc ordering.** `main.rs::on_key` (an
  integrator-owned file): move the menu-Esc branch
  above the `text_focused` early return.
- **S18-13 — light + focus verification scenes.** Add a
  light-theme scene and a keyboard-focus scene to
  `scenes.rs` and the headless probe set, so light
  contrast and focus visibility become rendered facts
  instead of computed/absent ones.

## 9. Headline numbers for the mission

- Worst contrast pair: **kbd hint on the palette's
  selected row — 1.18:1** (rendered). Worst general
  text pair: **kbd chips — 2.88:1** (rendered, dark).
  Worst full-size text pair: **white on accent —
  3.14:1 rendered**, against a documented acceptance of
  4.19:1 that does not reproduce.
- Worst hit target: **`marker-graph-*` — 30 × 14 pt**;
  runner-up `fig-sel-*` at 15 pt high on a core task.
- Strongest areas (do not let redesign churn break
  them): base text contrast (14.7–16.4:1), the danger
  family (5.0–7.6:1), shortcut truth (46/46 shown
  shortcuts real), the error grammar's reason-giving
  shape, and the eight workspace blurbs.
