# Phase N — P12: Colour & Contrast Audit

Scope: the **dark** theme only (the shell is dark-first:
`chrome_view::btn` and `icon_btn` both call
`tok::accent(0, true)` and the layout prefs default to
dark). Sources: `rune/src/tok.rs` `theme(true)` +
`accent(0, true)`, read 2026-11-08, and a census of
`src/` in StykAnimate-RS at the Phase M baseline.
No code was changed by this audit.

## Method (labelled approximation)

1. Each `oklch(L, C, h)` token was converted
   OKLCH → OKLab (a = C·cos h, b = C·sin h) → linear
   sRGB using the standard Ottosson matrices
   (l/m/s cubed, then the linear-sRGB matrix).
2. Relative luminance is the WCAG 2.x formula on the
   **linear** channels: `Y = 0.2126R + 0.7152G +
   0.0722B`. Contrast = `(Y_hi + 0.05) / (Y_lo +
   0.05)`.
3. Alpha tokens (`oklcha(1, 0, 0, α)` — white at low
   alpha) were composited **in linear light** over
   the stated base, matching tok.rs's own note that
   its alphas "are for linear-light blending". So a
   5% white border is *not* 5% in sRGB terms; the
   composited sRGB values below look brighter than a
   CSS intuition expects. That is correct, not an
   error.
4. sRGB hex values quoted are gamma-encoded from the
   same linear values, rounded to the nearest byte —
   approximations good to ±1–2 ratio points in the
   second decimal. Rune's renderer works in
   linear/OKLab space, so these WCAG numbers are a
   legibility yardstick, not a pixel prediction.
5. Hardcoded `0xRRGGBB` literals elsewhere in the
   shell are sRGB values; their ratios use the
   ordinary WCAG sRGB → linear conversion.

WCAG reference points used: AA normal text **4.5:1**,
AA large text / non-text (icons, control boundaries)
**3:1**, AAA normal text **7:1**. The shell's text
styles — BODY 13/400, LABEL 12/500, SECTION 11.5/600 —
all count as *normal* text (none reaches the
large-text size/weight threshold), so 4.5:1 is the
honest bar for text.

## The tokens, converted

| Token | OKLCH (dark) | ≈ sRGB | Luminance |
|---|---|---|---|
| `bg` | 0.185 / 0.004 / 265 | `#131214` | 0.0062 |
| `card` | 0.232 / 0.005 / 265 | `#1D1D1F` | 0.0123 |
| `text` | 0.955 / 0.003 / 265 | `#F0EFF2` | 0.8683 |
| `text_secondary` | 0.72 / 0.008 / 265 | `#A5A3A9` | 0.3696 |
| `accent(0)` | 0.66 / 0.17 / 256 | `#925EE9` | 0.2006 |
| `knob` | 0.985 / 0 / 0 | `#FAFAFA` | 0.9551 |
| `indicator` | 0.43 / 0.006 / 265 | `#504F53` | 0.0785 |
| white (button label) | — | `#FFFFFF` | 1.0000 |

Note the accent: OKLCH hue 256 at chroma 0.17 lands
as a vivid blue-violet (`#925EE9`-ish), **not** the
legacy 2.0 shell accent `0x6D8CFF`. Two different
"accent blues" already coexist in the app — see the
census section.

## Required pairs — results

| Pair | Ratio | Verdict |
|---|---|---|
| `text` on `bg` | **16.34 : 1** | ✅ AAA, large margin |
| `text` on `card` | **14.75 : 1** | ✅ AAA, large margin |
| `text_secondary` on `bg` | **7.46 : 1** | ✅ AAA (just over 7) |
| `text_secondary` on `card` | **6.74 : 1** | ✅ AA; ✗ AAA |
| white on `accent(0)` (active button / chip label) | **4.19 : 1** | ⚠️ **fails AA normal text (4.5)** by 0.31; passes AA large & non-text (3:1) |
| muted inert glyph on `card` | **6.74 : 1** | ✅ AA (see below) |
| `separator` on `card` | **1.87 : 1** | by design — hairline, see below |
| `separator` on `bg` | **1.97 : 1** | by design — hairline |
| `card_border` on `card` | **1.79 : 1** | by design — hairline |
| `card_border` on `bg` | **1.88 : 1** | by design — hairline |

### Reading the results

- **Primary and secondary text are safe.** Every text
  pairing Phase N will produce from the kit —
  panel titles, section labels, meta text, kbd chips,
  empty states, status segments — sits at 6.7:1 or
  better on both surfaces. No token change is needed
  or requested for text.
- **The muted inert glyph is `text_secondary` on
  `card`, by construction**: `icon_btn.rs`
  `glyph_color_muted(t) = t.text_secondary` and
  `fill_color_inert(t) = t.card`. So the inert pair
  *is* the 6.74:1 row above. Two consequences:
  (a) inert controls are **not** low-contrast by
  colour — they would pass AA even if they weren't
  exempt as disabled controls under WCAG; (b) what
  actually distinguishes an inert button from a live
  inactive one is *not* contrast (14.75 vs 6.74 is
  clear, but the fill is identical `card`), it's the
  absence of hover/press response. Phase N adoption
  must preserve that distinction — don't mute inert
  glyphs any further, and don't "fix" them up to
  `text`, or live/inert become indistinguishable.
- **White on accent is the audit's one token-level
  finding.** 4.19:1 is close, but at LABEL 12/500 it
  is formally under AA for normal text. The charter
  fixes `design::accent()` to `tok::accent(0, true)`,
  and the accent is Rune's token, not the shell's to
  retune — so this is a **documented acceptance, not
  a fix item**: active buttons/chips carry short
  labels and usually an icon as well; never put a
  long or critical string *only* in white-on-accent;
  never render accent-coloured *text* on bg/card as
  the sole carrier of meaning at small sizes without
  checking it (accent on card ≈ 3.4:1 by the same
  math — fine for a glyph or a large heading, not
  for 12px body text). The Phase N verification wave
  should re-state this in QA §16 so it stays a known,
  accepted property rather than a surprise.
- **Hairlines are subtle on purpose — do not demand
  AA for them.** `separator` (~1.9:1) and
  `card_border` (~1.8:1) are 1px decorative/structural
  lines, not text and not the sole boundary of an
  interactive control (controls carry their own fill
  changes on hover/press). A related watch item, not
  a blocker: `card` vs `bg` fill alone is only
  **1.11:1**, so the Phase N "card + hairline" look
  genuinely depends on the border plus the
  hover/press deltas (`hover` on card 1.25:1, `press`
  1.48:1, `control_track` 1.79:1, `control_off`
  2.35:1 — all composited). On a dim or low-quality
  monitor the card edge will be faint. That is the
  framework's aesthetic, consistent across Rune apps;
  Phase N should not thicken or brighten borders
  locally. If users report "panels blend into the
  background", the answer is a Rune-token issue to
  raise upstream, not a per-panel patch.
- **Kit composites check out.** Charter's selected
  list-row recipe (accent at 16% over `card`, accent
  border at 35%): the fill computes to ≈ `#432F69`,
  with `text` on it at **9.94:1** and
  `text_secondary` on it at **4.54:1** — both pass
  AA. The toggle recipe: `knob` (`#FAFAFA`) on
  accent = **4.01:1**, knob on `control_off`-over-
  card = **6.87:1** — both pass the 3:1 non-text bar
  comfortably.

## Census: hardcoded hex colours in `src/`

Pattern `0x[0-9A-Fa-f]{6}` over all of `src/`:
**697 occurrences**. Split at each file's first
`#[cfg(test)]` (an approximation — a few files
interleave, so read the split as ±a few percent):

| Bucket | Occurrences | Distinct values |
|---|---|---|
| Test code (fixtures, sentinels, expected values) | ≈ 542 | ≈ 63 |
| Production code | ≈ 155 | 58 |

### Class 1 — test fixtures: KEEP, not chrome (≈542)

Sentinel and sample colours (`0x112233`, `0x123456`,
`0x445566`, `0xABCDEF`, `0xFF0000`, `0x00FF00`, …)
exist to prove round-trips and rendering, not to be
seen. Phase N must not "tokenise" them; the gate's
key diff and CI are the only checks they answer to.

### Class 2 — data colours: KEEP (≈120 of the production 155)

Colours that *are the document*, or defaults for
document data. Tokenising them would change what
users' projects contain:

- Palette / swatch arrays: `inspector_figure.rs`
  16-colour figure palette (`0xE8E8EC, 0xFF5D5D,
  0x5DB2FF, 0x7BE495, 0xFFC65D, 0xC792FF, …`);
  `scene_panel.rs` background presets (Slate
  `0x232830`, Midnight, Deep blue, …, Paper, White)
  and gradient presets; `particles_panel.rs` /
  `text_tool.rs` colour palettes (incl. `0xA184FF`
  violet as a *data* swatch).
- Figure colour defaults: `FIGURE_COLORS`
  (`editor.rs`, duplicated in `mcp_io.rs` — the
  duplication is a pre-existing fact, not Phase N's
  to fix).
- Project defaults: demo/new-project background
  `0x232830` and figure colour `0xE8E8EC`
  (`model.rs`, `nodes.rs`, `nodes_anim.rs`,
  `mcp_tools.rs` defaults for `opt_color`).
- Engine/VFX/audio data colours in `vfx.rs`,
  `fx_ops.rs`, `fx_clips.rs`, `export.rs`, `svg.rs`,
  `paint.rs`, `media.rs` — clip-kind colours, render
  colours, format fixtures. These render the user's
  animation; they are content, not UI.

### Class 3 — UI chrome candidates (≈35): the Phase N work list

Hardcoded colours doing *interface* work. None is a
contrast emergency (ratios vs the stage/card below),
but each is a place where the shell's look can drift
from the token theme:

1. **`status_strip.rs` `GLYPH_TEXT = 0xF1F3F7`** —
   the old 2.0 "primary text" hex, ≈ theme `text`
   (`#F0EFF2`, 15.2:1 on card). Pure duplication:
   switch to `t.text`. *Highest-value, lowest-risk
   item in this class.*
2. **Legacy accent hexes**: `SELECTION_ACCENT`
   (`stage_view.rs`), `GIZMO_COLOR` (`stage_fx.rs`),
   `STRIP_ACCENT` (`fx_clips.rs`), `WAVE_HEX`
   (`waveform_view.rs`), `AUDIO_SPAN_RGB` /
   `FX_SPAN_RGB` (`timeline_marks.rs`),
   `SELECTED_COLOR` (`fig3_preview.rs`) — all
   `0x6D8CFF` / `0xA184FF`, the Wave-14 2.0 palette,
   **not** the Rune token accent (`#925EE9`-ish).
   On the (dark, project-chosen) stage background
   these read at ≈ 5.5–5.8:1 vs card-like darks —
   legible. But buttons will say "accent" in violet
   while canvas selection says it in sky blue.
   Recommendation: **Phase N documents the split,
   does not silently unify.** The canvas colours are
   drawn over *user* stage backgrounds (data), where
   the violet token accent is not guaranteed the
   better choice; unification is a product decision
   for the user, not a restyle. If the user later
   says unify, these six constants are the whole
   surface.
3. **Stage selection/onion colours** (`stage_view.rs`,
   `stage_layers.rs`): handle red `0xFF5D5D`
   (5.6:1 vs card), selected amber `0xFFC65D`
   (10.8:1), onion green `0x7BE495` (10.7:1) — all
   strongly legible on dark stages; they are
   canvas-convention colours (red = handle is a
   cross-editor convention). Keep; note only.
4. **Toast severity colours** (`toasts.rs`):
   info `0x5C9BF5` 6.0:1, success `0x35B56B` 6.4:1,
   warn `0xE8A33D` 7.8:1, error `0xE5484D` 4.3:1 —
   all vs card, all pass the 3:1 non-text bar for
   the *glyph*; toast text itself is `t.text`.
   Keep; when the foundation wave cards the toasts,
   these four constants should move into the toast
   module's top as named severity tokens (same
   values), not scatter further.
5. **`inspector_figure.rs` `danger_btn`** — the one
   chrome colour with a real contrast problem:
   white LABEL text on fill `0xF16F78` ≈ **2.9:1**,
   under even the 3:1 floor; hover/press darkens to
   `0xD85560` (≈ 3.9:1) — i.e. the button is *more*
   legible while pressed than at rest, which is
   backwards. When the foundation wave retires local
   button copies onto the kit family (charter §4),
   the danger variant needs a decision: dark text on
   the light red, or a deeper red fill with white
   text (≥ 4.5:1 needs roughly `0xC93A44`-depth —
   illustrat
...[truncated 3391 chars]