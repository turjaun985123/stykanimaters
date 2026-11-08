# Squad 02 Audit — Design System Architecture

> Phase 1 audit for the Ultimate UI/UX Mission. Read-only
> except this file. No cargo, no git (AGENTS.md rule).
> Method: source reads + grep census on the current tree;
> current source outranks every doc, including this
> mission's own brief.

## 0. The headline finding

**The app has no token file of its own.** Every colour,
type, spacing, radius and motion token the shell consumes
lives in the pinned Rune framework crate
(`rune::tok`, git rev `87caf2a`, source at
`~/workspace/rune-src/crates/rune/src/tok.rs`), not in
this repository. The brief's palette cannot be "adopted
in the theme" by editing a palette file here — there
isn't one. There are exactly three lawful adoption
paths, in order of preference:

1. **App-side theme construction.** `tok::Theme` is a
   struct with public fields, and `rune::color::hex`
   parses sRGB hex (already used by
   `chrome_view::danger_color`). `main.rs`'s `view()`
   can build `Theme { bg: …, card: …, ..tok::theme(dark) }`
   once, centrally, and every panel downstream inherits
   it unchanged. One function, one file, zero scatter.
2. **App-side token module** for everything `Theme` has
   no field for (semantic success/warning/error/info,
   elevated surface, muted text, stage accent) —
   extending `src/rune_sec/design.rs`, which is already
   the kit's home.
3. Editing the pinned Rune crate — **rejected**: it is
   an upstream git dependency shared by other apps;
   the mission rules say extend, don't fork.

Everything below is organised around that fact.

## 1. Token inventory — where tokens actually live

### 1.1 `rune::tok::Theme` (upstream, consumed via `&Theme`)

`main.rs::view` builds one theme per frame —
`tok::theme(!s.core.light_theme)` (`src/main.rs:559`) —
and passes `&t` down to every panel builder. That part
of the architecture is genuinely good: one construction
site, universal pass-down. The struct has **16 fields
and no others**:

`bg, card, card_border, text, text_secondary,
separator, control_track, control_off, indicator, knob,
hover, press, shadow, shadow_soft, edge, knob_shadow`.

Roles the brief's palette needs that **do not exist**
in the struct: secondary/chrome surface, elevated
surface, muted (third-level) text, success, warning,
error, info, and accent (accent is a separate function,
§1.3). Any brief adoption must add those app-side
(path 2), because the struct cannot grow here.

Values are OKLCH, not hex. The sRGB equivalents below
are computed with the standard OKLCH→sRGB conversion,
rounded to the nearest hex step; alpha tokens are given
as their effective colour composited over the card,
since an alpha hairline has no single hex.

#### Dark theme (`tok::theme(true)`)

| Token | Source value | ≈ sRGB |
|---|---|---|
| `bg` | oklch(0.185, 0.004, 265) | **#121314** |
| `card` | oklch(0.232, 0.005, 265) | **#1C1D20** |
| `card_border` | white α 0.05 | ≈ #27282B over card |
| `separator` | white α 0.055 | ≈ #28292C over card |
| `text` | oklch(0.955, 0.003, 265) | **#EFF0F2** |
| `text_secondary` | oklch(0.72, 0.008, 265) | **#A2A5AA** |
| `control_track` | white α 0.05 | ≈ #27282B over card |
| `control_off` | white α 0.085 | ≈ #2F3033 over card |
| `hover` | white α 0.016 | ≈ #202124 over card |
| `press` | white α 0.03 | ≈ #232427 over card |
| `indicator` | oklch(0.43, 0.006, 265) | #4E5053 |
| `knob` | oklch(0.985, 0, 0) | #FAFAFA |
| `shadow` / `shadow_soft` / `edge` / `knob_shadow` | black α 0.42 / 0.28 / 0.30 / 0.30 | — |

Note the near-zero chroma (0.003–0.008): the current
theme is a **neutral grey** system with a 265° whisper.
The brief's palette is a **blue-grey** system. That —
not lightness — is the main visual difference adoption
would make.

#### Light theme (`tok::theme(false)`)

| Token | Source value | ≈ sRGB |
|---|---|---|
| `bg` | oklch(0.958, 0.004, 265) | **#F0F1F4** |
| `card` | oklch(1.0, 0, 0) | **#FFFFFF** |
| `card_border` | black α 0.10 | ≈ #E6E6E6 over card |
| `separator` | black α 0.13 | ≈ #DEDEDE over card |
| `text` | oklch(0.225, 0.006, 265) | **#1A1C1F** |
| `text_secondary` | oklch(0.50, 0.01, 265) | **#606369** |
| `control_track` | oklch(0.2,·,265) α 0.13 | — |
| `control_off` | oklch(0.2,·,265) α 0.24 | — |
| `hover` | oklch(0.2,·,265) α 0.07 | ≈ #EFEFEF over card |
| `press` | oklch(0.2,·,265) α 0.12 | — |
| `indicator` | oklch(1.0, 0, 0) | #FFFFFF |
| `knob` | oklch(1.0, 0, 0) | #FFFFFF |
| `shadow` / `shadow_soft` / `edge` / `knob_shadow` | oklch(0.2,0.02,265) α 0.16 / 0.07 / 0.20 / 0.30 | — |

The light token set is complete *as a set* — all 16
fields defined, sensibly mirrored. The light theme's
problem is not missing tokens; it is app code that
ignores them (§4).

### 1.2 Type, space, radius, motion (`rune::tok`)

- **Type** (`tok::type_`): `BODY` 13/400, `LABEL` 12/500,
  `SECTION` 11.5/600, `TITLE` 13/600, `MONO_CAPTION`
  12/500. Five styles, no display/heading scale — the
  kit's discipline of "no giant headings" is enforced
  by absence.
- **Space** (`tok::space`): `S1` 4, `S2` 8, `S3` 12,
  `S6` 24. (No S4/S5 exist upstream.)
- **Radius** (`tok::radius`): `SM` 6, `MD` 8, `LG` 12.
- **Motion** (`tok::motion`, from `rune-motion`):
  springs `INSTANT` .1, `SNAP` .25, `SETTLE` .3,
  `REFLOW` .4, `GLIDE` .32, `MORPH` .3/bounce .16,
  `POP` .25/.15, `DRIFT` .85, `CAMERA` 1.2, each with an
  `*_EXIT` and a `pair::*` enter/exit pair; plus
  `CSS_ENTER` 150 ms / `CSS_EXIT` 110 ms curves and
  tok's own `LEAD` spring. A full, professional motion
  scale — of which the app uses two pairs (§5.4).

### 1.3 Accent (`tok::accent(i, dark)`)

Six accents by index (blue, purple, pink, orange,
green, graphite), OKLCH hue families at L 0.66 dark /
0.60 light. Index 0 (blue, hue 256) ≈ **#4392F7** dark /
#2F7FE3 light; index 1 (purple, hue 300) ≈ **#A473EE**
dark. The app calls `tok::accent(0, true)` — **dark
variant hard-coded** — from 13 files (§4.1).

### 1.4 The app kit (`src/rune_sec/design.rs`)

The Phase N kit is disciplined and is the right home
for mission foundation work:

- Constants mirror tok values under kit names:
  `PANEL_RADIUS` 12 (= LG), `PANEL_PAD` 12 (= S3),
  `PANEL_GAP` 8 (= S2), `ROW_RADIUS` 8 (= MD),
  `CONTROL_RADIUS` 6 (= SM).
- `accent()` wraps `tok::accent(0, true)`;
  `selected_fill()` = accent @ α 0.16;
  `text_muted(t)` = `t.text_secondary` @ α 0.6 — a
  derived third text level, the kit's one invented
  token, done correctly (derived, never a literal).
- Builders (`panel_col`, `header_row`, `panel_title`,
  `section_label`, `divider`, `kbd`, `empty_state`,
  `list_row`, `toggle`, `toggle_row`, `stepper_value`,
  `slider_track`, `swatch`) consume `&Theme` for every
  colour. **Zero hex literals in design.rs production
  code.** The kit is not the problem.

### 1.5 App-side constants that act as tokens

| Where | What | Status |
|---|---|---|
| `chrome_view.rs:146-150` | `DANGER_FILL` 0xC93A44 / `DANGER_HOVER` 0xB2323E / `DANGER_PRESS` 0x9C2934 | Single home, imported by `icon_btn` — centralised, but hex, theme-independent, and *not* the brief's error |
| `toasts.rs` `Severity::color()` | Info oklch(.70,.14,250) ≈ #53A3F2; Success (.72,.15,155) ≈ #43C07A; Warn (.78,.15,75) ≈ #EFA831; Error (.68,.20,25) ≈ #FC5855 | Local to toasts; the shell's *only* semantic colours |
| `toasts.rs` `Severity::icon_color()` | Hex twins: 0x5C9BF5 / 0x35B56B / 0xE8A33D / 0xE5484D | **Drifted twins**: Success colour ≈ #43C07A vs icon 0x35B56B; Error ≈ #FC5855 vs icon 0xE5484D — dot and glyph on the same toast are visibly different colours |
| `stage_view.rs:318` + `stage_fx.rs` | `SELECTION_ACCENT` / `GIZMO_COLOR` = 0x6D8CFF, test-pinned equal | The legacy "stage accent" family (§3.3) |

## 2. Delta table — brief palette vs current tokens

The brief's palette has provenance: it is the Wave-14
redesign directive's palette (`docs/QA_WAVE14.md`,
`docs/GAP-ANALYSIS.md:573`), which the GPUI kit adopted,
then a 2.0 retune moved accent → #6D8CFF and error →
#F16F78 (`docs/GAP-ANALYSIS.md:683`,
`docs/WORKSPACE_DESIGN_SPEC.md:252-262`), and the Rune
migration then replaced the whole app palette with
Rune's upstream `tok` theme without a recorded palette
decision. So "current" below is a *third* palette,
neither the brief's nor the retune's.

**Count: 13 brief tokens compared — 0 exact matches,
3 near (≤ ~5 per channel), 7 material deltas,
3 roles missing entirely.**

| # | Brief token | Brief | Current equivalent | ≈ Current | Verdict |
|---|---|---|---|---|---|
| 1 | Main background | #101216 | `Theme.bg` dark | #121314 | **Near** — current +2/+1/−2; lightness equal, brief is bluer |
| 2 | Secondary background | #171B22 | — none — | (nearest: `card` #1C1D20, a different role) | **MISSING ROLE** — Theme has only bg + card |
| 3 | Panel surface | #1D222B | `Theme.card` dark | #1C1D20 | **Near** — blue channel −11; current is neutral, brief is blue-grey |
| 4 | Elevated surface | #252C37 | — none — | (nearest effective: `control_off` ≈ #2F3033) | **MISSING ROLE** |
| 5 | Subtle border | #303846 | `card_border` white α .05 | ≈ #27282B over card | **Mechanism delta** — alpha hairline, not a solid colour; reads darker/subtler and changes with what's behind it |
| 6 | Primary text | #F1F3F7 | `Theme.text` dark | #EFF0F2 | **Near** — −2/−3/−5 |
| 7 | Secondary text | #A1A9B7 | `Theme.text_secondary` dark | #A2A5AA | **Delta** — blue channel −13; current is greyer |
| 8 | Muted text | #737D8C | — none — | kit-derived `text_muted` ≈ #6C6F73 over card | **MISSING ROLE** — exists only as α 0.6 of secondary, and only where panels call the kit fn |
| 9 | Primary accent | #6C8CFF | `tok::accent(0, dark)` | #4392F7 | **LARGE DELTA** — current is a deeper, more saturated blue. Irony: the brief's accent ≈ the *stage's* legacy accent 0x6D8CFF (§3.3), so canvas selection already matches the brief while chrome does not |
| 10 | Secondary accent | #9B7CFF | `tok::accent(1, dark)` | #A473EE | **Delta** — current redder/darker; legacy 0xA184FF (timeline/FX spans) is closer to the brief |
| 11 | Success | #43C995 | `toasts Severity::Success` | ≈ #43C07A (dot) / 0x35B56B (glyph) | **Delta + not a token** — current is greener, brief is teal; exists only inside toasts.rs, in two drifted forms |
| 12 | Warning | #F1B85B | `toasts Severity::Warn` | ≈ #EFA831 (dot) / 0xE8A33D (glyph) | **Delta + not a token** — current is more orange |
| 13 | Error | #EF6974 | `toasts Severity::Error` ≈ #FC5855 / `DANGER_FILL` #C93A44 | two different reds | **Delta + split** — brief's value ≈ the retired 0xF16F78 family, which Phase N *measured* at ≈ 2.9:1 under white label text and deliberately deepened (chrome_view.rs:136-150). Adopting #EF6974 as a **fill** would regress a measured accessibility fix; as an on-dark text/glyph tint it is unobjectionable. The mission must split "error tint" from "danger fill" rather than unify them blindly |

Light-theme side of the same table: the brief names no
light values at all (its palette is dark-only, as was
Wave-14's — GAP-ANALYSIS records the light pair as
"token-complete, not yet applied"). Current light
tokens (§1.1) are a coherent neutral set; if the dark
adoption proceeds via app-side construction (path 1),
a matching light override set must be authored in the
same function — the brief gives no values, so Squad 02
recommends deriving them by mirroring the dark deltas
(same hue shift, lightness preserved), not inventing a
second palette.

## 3. Literal-leak census

Method: every `0xRRGGBB` literal in `src/`, split at
each file's `#[cfg(test)]` boundary.

- **704 literals total; 547 (78%) are in test code** —
  fixtures, expected values, demo data. Not leaks.
- **157 are in production code**: 111 in
  `src/rune_sec/*.rs`, 46 in `src/*.rs`.
- Of the 111 in `rune_sec`, **≈ 83 are domain data** —
  user-content colour palettes (figure colours, stage
  background presets, particle/text swatch palettes).
  These are *content*, not chrome: a figure's red is
  data in the project file's terms, and tokenising it
  away would be wrong. Their sin is duplication (§3.2),
  not existence.
- **≈ 28 production literals in `rune_sec` are genuine
  chrome/semantic leaks**, plus 4 in `chrome_view.rs`
  (the DANGER family + references). Concentrated in
  five places (§3.1–3.4).

Other literal forms are clean: no `Color::rgb`/raw
`oklch` constructions in panel production code outside
toasts' severity fn (6 `oklch` mentions, all toasts);
`Color::WHITE` (59 production uses) is mostly
white-label-on-accent/danger, a legitimate constant;
`hex_color()` — the u32→Color bridge every palette
needs — is defined **twice** (`stage_view.rs:46`
pub, `waveform_view.rs:237` private copy).

### 3.1 Worst leak: `waveform_view.rs` is theme-blind

`src/rune_sec/waveform_view.rs:198-204` hard-codes the
*frozen GPUI-era palette* as file constants and the
file never takes a `&Theme` at all (it imports only
`type_`):

- `WAVE_HEX` 0x6D8CFF (legacy accent, not tok accent)
- `CENTRE_HEX` 0x303744 (the retired 2.0 border colour)
- `PLAYHEAD_HEX` 0xFFC65D (amber, exists in no token)
- `CAPTION_HEX` 0xA2AAB8 (the retired 2.0 secondary
  text, vs current `text_secondary` #A2A5AA)

In light theme the waveform still draws dark-theme
colours, and #A2AAB8 caption text on a white card is
the weakest text contrast in the shell. This is the
only panel surface with **zero** token consumption.

### 3.2 Duplicated domain palettes (one value set, many homes)

- The 6-colour figure palette `[E8E8EC, FF5D5D, 5DB2FF,
  7BE495, FFC65D, C792FF]` exists **verbatim twice**:
  `editor.rs:102` (`FIGURE_COLORS`) and
  `clips_panel.rs:465` (`FIGURE_COLORS`).
- A near-twin in `inspector_segment.rs:189` (`PALETTE`,
  last entry 0x232830 instead of C792FF ordering) and a
  16-colour superset in `inspector_figure.rs:193-194`.
- The 8-colour palette `[FF5D5D, FF9F43, FFC65D, 7BE495,
  57C7E3, A184FF, FFFFFF, 000000]` exists **verbatim
  twice**: `particles_panel.rs:142-149` and
  `text_tool.rs:265-272`.
- `scene_panel.rs` carries **two stage-background
  preset lists in one file that have drifted apart**:
  the list at :613-630 (Sky 0x87B5D6, Forest 0x1E4634,
  Paper 0xF2EAD8) vs the list at :650-655 (Sky
  0x87B5D9, Forest 0x1E3327, Paper 0xF2EDE4) — same
  preset names, different colours, both live.

### 3.3 The legacy accent family (0x6D8CFF / 0xA184FF)

Seven production sites use the Wave-14-retune accent
instead of `tok::accent`: `stage_view::SELECTION_ACCENT`,
`stage_fx::GIZMO_COLOR` (test-pinned equal),
`fx_clips::STRIP_ACCENT` (:432), `timeline_marks`
(:1032 `AUDIO_SPAN_RGB` 0x6D8CFF, :1036 `FX_SPAN_RGB`
0xA184FF), `waveform_view::WAVE_HEX`,
`fig3_preview::SELECTED_COLOR` (:88). `docs/QA_RUNE.md`
records this as a *documented split* — canvas colours
were deliberately left as product data. It is not an
accident to clean up in passing; it is a decision the
mission must re-take explicitly, especially as the
brief's accent (#6C8CFF) would make the two accents
nearly identical and the split pointless.

### 3.4 Small chrome leaks

- `graph_panel.rs:348` `GLYPH_MUTED` 0x707B8C — the
  retired 2.0 muted-text hex, one site; the kit's
  `text_muted()` is its token successor.
- `fig3_preview.rs:78` `GRID_COLOR` 0x3A4356 — a
  blue-grey grid colour in neither palette.
- `onion_controls.rs:197` `FUTURE_ONION_RGB` 0x7BE495
  + `stage_view.rs:168` — onion-skin green as a raw
  literal (domain-adjacent; a named const exists in
  one place, a bare literal in the other).
- `project_tools.rs:166-169` — new-project defaults
  (background 0x232830, figure 0xE8E8EC) duplicate
  values owned by `editor.rs` / `scene_panel.rs`
  preset data.

## 4. Light theme audit

The token set is complete (§1.1); consumption is not.
Every light-theme defect found is an app-side
dark assumption:

1. **Accent is dark-pinned.** `tok::accent(0, true)` —
   the `true` is literal — at production sites in 13
   files, including `design::accent()` itself and
   `chrome_view::btn`. In light mode buttons, selected
   rows, toggles and the dirty dot all render the dark
   accent (L 0.66 ≈ #4392F7) instead of the light one
   (L 0.60 ≈ #2F7FE3). Visually survivable (the dark
   accent is *lighter*, so on white it is the weaker
   contrast of the two) — exactly backwards for
   legibility.
2. **Toasts are dark-pinned.** `toasts_view(s, now)`
   takes no theme; internally `tok::theme(true)`
   (toasts.rs:368) — its doc comment still claims "the
   shell is dark-only today", which stopped being true
   when `main.rs` wired `light_theme`. In light mode,
   toast cards render with the dark card/text tokens.
3. **Waveform is theme-blind** (§3.1) — worst in light.
4. **Danger reds are theme-independent** — defensible
   (measured for white text on red), but in light mode
   there is no lighter-weight error *tint* for text
   use; error text borrows the toast severity colour,
   which is dark-tuned.
5. **Stage/canvas colours** (background presets, figure
   colours, fig3 grid/shadow) ignore theme — correctly:
   they are project content, and a light UI around a
   dark stage is normal for animation tools. Recorded
   here so the mission does not "fix" them by mistake.
   (`fig3_preview::SHADOW_COLOR` 0x000000 with alpha
   handling is fine in both.)

Net: light theme = complete tokens, three dark-pinned
consumers (accent path, toasts, waveform). All three
are single-coder fixes (F2, F6, F3 below).

## 5. Typography, spacing, radii, motion adherence

### 5.1 Typography — excellent

183 production `type_::*` references in `rune_sec`
(LABEL 167, BODY 16, MONO_CAPTION 11, TITLE 5,
SECTION 2 — SECTION reads low only because the kit's
`panel_title`/`section_label` wrap it). Exactly **one**
raw `TextStyle::new` in production:
`timeline_marks.rs`' 10 pt/500 ruler numerals — an
off-scale size (the scale has no 10), justified as
microtype but worth a named token if the mission adds
one. The kit's `.mono()` amendment (MONO_CAPTION does
not set the Mono family upstream) is applied in
`kbd`/`stepper_value`. No other font-size literals.

### 5.2 Spacing — convergent, but never by reference

`tok::space` is referenced **zero** times in the app.
Yet the numeric gaps panels actually use land on the
same scale: `gap(8)` ×134, `gap(6)` ×81, `gap(4)` ×46,
`gap(2)` ×18, `gap(12)` ×2 (production, `rune_sec`).
Adherence is by convention, not by token — it works
today because Phase N's kit set the example, and it
will drift the first time a panel author guesses.
Notable: **6 is the second most used gap and is not a
tok step** (4/8/12/24) — a de-facto half-step the
system should either adopt (app-side `SPACE_HALF_STEP`
in the kit) or migrate to 4/8 deliberately.

### 5.3 Radii — same pattern

`tok::radius` referenced zero times; literals match
its values anyway: `radius(6)` ×19 (SM, via kit
CONTROL_RADIUS and chrome btn), `radius(8)` ×5 (MD),
`radius(12)` ×2 (LG). Off-scale literals: 4 (kbd chip,
dirty dot — small-element radii, reasonable), 5, 2,
3, 1.5 (canvas-adjacent hairline radii), 999 (pills —
an idiom, fine). Verdict: values adhere, references
don't; the kit constants are the de-facto tokens.

### 5.4 Motion — total adherence, thin coverage

No app-local spring or duration literals anywhere.
The only presence springs in the shell are
`pair::SETTLE` (toasts) and `pair::SNAP` (menus,
settings popover, command palette) — all from the
token pairs, per the Phase O charter's discipline.
`press_scale(0.97)` at 6 sites (chrome btn family,
icon_btn family). The other pairs (REFLOW, GLIDE,
MORPH, POP, DRIFT) and `tok::motion::LEAD` are unused
— available headroom, not a defect. If the mission
adds motion, it must come from these pairs; there is
no literal-motion leak to clean.

## 6. Foundation changes (sized for single coders)

Ordered by dependency. Each is one coder, one primary
file cluster, no key/handler changes anywhere.

- **F1 — Central theme adoption point.** Add
  `design::app_theme(dark: bool) -> Theme` (or a
  `theme.rs` beside it): starts from `tok::theme(dark)`
  and overrides fields via `rune::color::hex` **if and
  when** the mission ratifies brief values; `main.rs`
  calls it instead of `tok::theme`. Even with zero
  overrides ratified, this is the seam every later
  retune needs. *Files: design.rs (or new theme.rs),
  main.rs (one line).*
- **F2 — Theme-aware accent.** `design::accent_on(dark)`
  + migrate the 13 `tok::accent(0, true)` files to take
  the dark flag from state; `chrome_view::btn` and
  `design::accent()` first (they define what panels
  copy). Fixes light-theme accent (§4.1) regardless of
  any palette decision. *Files: design.rs,
  chrome_view.rs, then one coder per panel cluster for
  the remaining call sites.*
- **F3 — Waveform theme-isation.** `waveform_view`
  takes `&Theme`; WAVE/CENTRE/CAPTION colours derive
  from theme + accent; PLAYHEAD amber becomes a named
  semantic in the F5 module. Retire the private
  `hex_color` copy onto `stage_view::hex_color`.
  *Files: waveform_view.rs + its caller.*
- **F4 — Semantic token module.** Extend design.rs (or
  F1's theme.rs) with `success/warning/error/info`
  as (Color, hex-twin) pairs generated from **one**
  definition each, ending the toast dot/glyph drift;
  toasts' `Severity::color/icon_color` delegate to it.
  The mission's brief values, if ratified, land here —
  as tints; `DANGER_*` fills stay as measured.
  *Files: design.rs, toasts.rs, chrome_view.rs (docs
  pointer only).*
- **F5 — Missing-role tokens.** App-side
  `elevated(t)` / `chrome_surface(t)` /
  `text_muted` promotion: define the secondary-surface
  and elevated roles as theme-derived values in the
  kit (e.g. elevated = card lightened via control_off
  composite) so Squad 03/04 work has tokens to consume
  instead of inventing fills. *Files: design.rs only.*
- **F6 — Toasts take the theme.** `toasts_view(s, t,
  now)`; delete the `tok::theme(true)` pin and the
  stale "dark-only" comment. *Files: toasts.rs,
  main.rs (one call site).*
- **F7 — Domain palette dedupe.** One home per palette:
  figure colours in `editor.rs` (clips_panel imports);
  the 8-colour swatch set in one `rune_sec` const
  (particles + text_tool import); scene background
  presets merged into a single list in scene_panel
  with the drifted values reconciled (product picks
  Sky/Forest/Paper values). *Files: one coder per
  palette pair; scene_panel merge is its own item.*
- **F8 — Legacy stage accent decision + execution.**
  Mission-level decision (Squad 20 ratifies): keep
  0x6D8CFF as a named `STAGE_ACCENT` in one home, or
  migrate the 7 sites to the (possibly re-ratified)
  accent. Execution is one coder once decided; the
  test pinning SELECTION_ACCENT == GIZMO_COLOR moves
  or retires with it. *Files: stage_view, stage_fx,
  fx_clips, timeline_marks, fig3_preview.*
- **F9 — Spacing/radius by reference.** Kit re-exports
  (`SPACE_*`, plus existing radius consts) and a
  mechanical migration of the top literal clusters —
  deliberately last and optional; values do not change,
  only references. Includes ruling on the gap-6
  half-step: adopt it as a named kit constant.
  *Files: design.rs + one coder per panel cluster.*

## 7. What Squad 02 could not verify

- Rendered truth: this audit is source + computed
  colour conversion. Whether #4392F7-vs-#6C8CFF and the
  neutral-vs-blue-grey shift *feel* like the brief
  intends is Squad 19's pixel work, on real captures.
- The OKLCH→sRGB figures are standard-conversion
  approximations; Rune's renderer composites in
  linear light (per tok's own docs), so on-screen
  values of alpha tokens vary with background — the
  effective hexes given are over-card estimates.
- Light theme was audited at the token/consumer level
  only; no light-theme screenshot exists in the Phase N
  evidence set (dark @ 1440×900 only) — a gap the
  mission already carries.
