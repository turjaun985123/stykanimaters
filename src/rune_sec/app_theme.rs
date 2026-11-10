//! App-side theme construction — the UI mission's palette
//! seam (Squad 02 audit `docs/ui_mission/audit/
//! s02-design-system.md`, foundation items F1/F2/F4/F5).
//! Declared from `rune_sec/mod.rs` only — bin-crate code,
//! not part of the engine library.
//!
//! The shell's colours live in the pinned Rune crate's
//! `tok::Theme` — a struct with exactly 16 `Color`
//! fields and no palette file in this repository. The
//! lawful adoption path (audit §0) is app-side
//! construction: [`app_theme`] starts from
//! `tok::theme(dark)` and overrides the fields the
//! mission brief names, in this one place, so every
//! panel downstream inherits the brief's palette
//! unchanged. `main.rs::view` is the single call site.
//!
//! # Dark mapping (brief → `Theme` field)
//!
//! | Brief role | Brief value | Field | Set to |
//! |---|---|---|---|
//! | Main background | `#101216` | `bg` | `#101216` |
//! | Panel surface | `#1D222B` | `card` | `#1D222B` |
//! | Primary text | `#F1F3F7` | `text` | `#F1F3F7` |
//! | Secondary text | `#A1A9B7` | `text_secondary` | `#A1A9B7` |
//! | Subtle border | `#303846` | `card_border` | `#303846` solid (see below) |
//! | Subtle border | `#303846` | `separator` | `#303846` @ α 0.55 (see below) |
//!
//! **The border mechanism exception.** Upstream,
//! `card_border`/`separator` are *alpha hairlines*
//! (white at α 0.05/0.055) whose effective colour
//! changes with whatever is behind them — there is no
//! solid border colour in the struct's design. The
//! brief's `#303846` is a solid. `card_border` takes
//! the brief value solid: a card edge sits on the app
//! background or between panels, the two adjacencies
//! the brief's palette pairs `#303846` with, so the
//! solid reads as specified in both. `separator` —
//! hairlines *inside* a card, which must stay
//! subordinate to the card edge — keeps the alpha
//! mechanism with the brief colour as its tint source:
//! composited over the `#1D222B` card it lands at
//! ≈ `#272E3A`, a dimmer sibling of the border.
//! Squad 19's pixel pass is the judge of both choices.
//!
//! Every other dark field (`control_track`,
//! `control_off`, `hover`, `press`, `indicator`,
//! `knob`, the four shadows) is a *mechanism* value the
//! brief does not name — white/black alpha overlays and
//! shadow depths — and is retained from upstream
//! verbatim.
//!
//! # Light mapping
//!
//! The brief names no light values (its palette is
//! dark-only). Per the audit's recommendation (§2),
//! the light overrides mirror the dark deltas — the
//! same blue-grey hue shift, lightness preserved —
//! rather than inventing a second palette:
//!
//! | Field | Upstream ≈ | Set to | Why |
//! |---|---|---|---|
//! | `bg` | `#F0F1F4` | `#EDF0F5` | same lightness, brief's blue-grey cast |
//! | `card` | `#FFFFFF` | retained | white panels are the brief's light direction |
//! | `text` | `#1A1C1F` | `#171A21` | blue-grey ink, matching the dark text family |
//! | `text_secondary` | `#606369` | `#5B6577` | the dark secondary's hue shift, mirrored |
//! | `card_border` | black α 0.10 | `#2A3242` @ α 0.14 | tinted hairline (≈ `#E1E4E9` over the white card) |
//! | `separator` | black α 0.13 | `#2A3242` @ α 0.19 | tinted hairline (≈ `#D6D9E0` over the white card) |
//!
//! Remaining light fields are mechanism values,
//! retained as in the dark theme.
//!
//! # Roles `Theme` cannot hold
//!
//! The brief names four roles with no field in the
//! struct. They are provided here as derived helpers
//! instead of being scattered as literals:
//!
//! * **Secondary (chrome) surface** `#171B22` —
//!   [`chrome_surface`]. Dark: the brief value.
//!   Light: derived midway between `bg` and `card`.
//! * **Elevated surface** `#252C37` — [`elevated`].
//!   Dark: the brief value. Light: the card stepped
//!   10% toward the background — the light ladder's
//!   third rung (see the Polish re-audit below);
//!   before RM25 the light value simply equalled
//!   `card`, so light elevation was carried by
//!   shadow and border alone.
//! * **Muted (third-level) text** `#737D8C` —
//!   [`muted_text`]. Dark: the brief value. Light:
//!   derived from `text_secondary` toward `bg`.
//!   (`design::text_muted` remains the alpha-derived
//!   sibling; it now derives from the overridden
//!   secondary, so the two agree in family.)
//! * **Accent** — accent is a function upstream
//!   (`tok::accent(i, dark)`), not a `Theme` field, and
//!   13 files hard-code its dark variant. [`accent`] /
//!   [`accent_on`] are the theme-aware successors; the
//!   call-site migration belongs to the adoption wave.
//!
//! # Semantic tokens, and the danger-fill exception
//!
//! [`success`], [`warning`] and [`error_ink`] are the
//! brief's semantic values (`#43C995`, `#F1B85B`,
//! `#EF6974`) as single definitions, ending the toast
//! dot/glyph drift the audit found in `toasts.rs`
//! (§1.5) once adoption points `Severity` here. The
//! `*_on` variants darken each tint for light-theme
//! surfaces (≈ 5:1 on the white card).
//!
//! **Exception, measured, do not "fix":** the brief's
//! error red is an *ink* — a text/glyph tint on a
//! surface. It is **not** the danger-button fill.
//! `#EF6974` under white label text measures ≈ 2.9:1,
//! and Phase N deliberately deepened the fill family
//! to `#C93A44`/`#B2323E`/`#9C2934`
//! (`chrome_view::DANGER_FILL` and siblings, ≈ 5:1 at
//! rest). Those fills stay exactly as measured; this
//! module's [`error_ink`] must never be substituted
//! for them.
//!
//! # Contrast audit (Wave 9)
//!
//! WCAG ratios computed with Rune's own colour
//! semantics (`hex` decodes sRGB to linear with the
//! standard EOTF, `Color::mix` interpolates in linear
//! light, authored hexes are the rendered values), so
//! the derived light muted below is its rendered
//! value (`#9BA0AA`), not a naive sRGB lerp. The
//! accent pair is excluded — the user's pending
//! decision, untouched here. Real-text pairs, all
//! clearing 4.5:1, no change: secondary on panel
//! 6.74 (dark) / 5.88 (light); secondary on elevated
//! 5.94 / 5.88; `success_on` on card 7.62 / 5.33;
//! `warning_on` on card 8.93 / 5.41; `error_on` on
//! card 5.28 / 5.43.
//!
//! Muted pairs: on panel 3.83 (dark) / 2.62 (light);
//! on background 4.50 (dark) / 2.30 (light). Muted is
//! nevertheless left unchanged: its only production
//! consumer is decorative — `graph_panel::glyph_muted`
//! (the channel rows' `chart-line` glyphs, documented
//! as pure decoration in that module). No real text
//! renders in [`muted_text`] in either theme (kit text
//! takes `text_secondary`; `design::text_muted`'s one
//! consumer is likewise a glyph, the splitter grip).
//! A genuine third level cannot be 4.5:1 text in
//! light anyway: clearing 4.5 on the white card would
//! need the light mix factor cut from 0.30 to ≈ 0.07,
//! i.e. muted ≈ secondary. If a future consumer ever
//! renders real text in muted, the dark clearing step
//! is a 0.056 linear mix toward white (`#818996`,
//! 4.51 on the card) — and light muted must not be
//! used for text at all.
//!
//! # Contrast re-audit (Wave 10)
//!
//! The Wave 9 audit above was recomputed from the
//! current token values: every published figure
//! reproduces exactly (secondary 6.74 / 5.88 and
//! 5.94 / 5.88; `success_on` 7.62 / 5.33;
//! `warning_on` 8.93 / 5.41; `error_on` 5.28 /
//! 5.43; muted 3.83 / 2.62 / 4.50 / 2.30; the
//! light muted still renders `#9BA0AA`; the 0.056
//! clearing step still lands `#818996` at 4.51).
//! No token value changed in between. Pairs the
//! first audit did not print, all clearing 4.5:
//! primary text on card 14.37 (dark) / 17.41
//! (light), on elevated 12.65 / 17.41, on the app
//! background 16.88 / 15.24; secondary on the
//! background 7.92 / 5.15. The danger fills were
//! re-verified too: white on `#C93A44` 5.03, on
//! `#B2323E` 6.14, on `#9C2934` 7.55 — the shell's
//! standing precedent is that a fill carrying
//! white text is held to ≈ 5:1.
//!
//! New or newly covered surfaces since Wave 9:
//!
//! * **Clips "Editing" marker** (Wave 9): the
//!   accent as LABEL text on the card — 5.19 dark
//!   / 5.06 light. Passes; the app accent as an
//!   *ink* is not the contrast problem (below).
//! * **Status strip** (the Wave 8 `strip_colors`
//!   seam; segments paint on the app background):
//!   text 16.88 / 15.24, secondary tail 7.92 /
//!   5.15 — and the play-state segment, accent
//!   text while playing: 6.10 dark / **4.43
//!   light**. The one new real-text miss in this
//!   re-audit, marginal (0.07 under) and only
//!   while playback runs; recorded, not fixed —
//!   any fix touches the accent decision below
//!   or the strip's colour seam, not this file's
//!   values alone.
//! * **Toasts**: severity bar and glyph measure
//!   on the *upstream* dark card (see the
//!   hardcode list below): Info 5.47, Success
//!   8.02, Warn 9.40, Error 5.56, body text
//!   14.74 — all clear, in both appearances,
//!   because the toast column never leaves the
//!   dark theme. The export recap's severity
//!   *text* takes the real theme and is the
//!   Wave 9 `*_on`-on-card pairs above.
//! * **Timeline marker row** (Wave 10): the
//!   current frame's cell is now accent-filled
//!   with white glyphs — the 3.14 pair below, as
//!   *glyphs* (non-text), clearing the 3:1
//!   non-text guidance by 0.14. Off the current
//!   frame the accent glyph on the card is 5.08
//!   dark / 3.14 light. The current chip's white
//!   frame *number* is real text at the same
//!   3.14 (pre-existing; part of the decision
//!   below). Loop-range chip fill (card mixed
//!   0.35 toward the accent, primary text on
//!   it): 5.91 / 13.26.
//! * **Tint surfaces** (primary text over an
//!   accent tint of the card): selected list
//!   rows (α 0.16) 8.69 / 15.51; the home
//!   recovery banner (α 0.18) 8.28 / 15.28.
//! * **Splitter grip** (Wave 9): `design::
//!   text_muted` (secondary at α 0.6) over the
//!   background — 5.15 dark / 1.94 light. A
//!   decorative glyph (the hairline carries the
//!   divider); the light grip falls under the
//!   3:1 non-text guidance and is recorded as
//!   an observation for the kit, not a text
//!   failure. Muted's consumer list is unchanged
//!   in kind: graph glyphs plus this grip, both
//!   decoration, still no real text.
//! * **Dopesheet lane** (Wave 10, in flight at
//!   this audit): figure names and readouts are
//!   primary / secondary text on the card
//!   (14.37 / 17.41 and 6.74 / 5.88 — covered,
//!   no new pair), and its keyframe diamonds
//!   are the upstream accent as a glyph: on the
//!   plain card 5.08 dark / 3.14 light, but on
//!   the selected-fill tint (the current-frame
//!   column and the selected figure's row)
//!   **3.07 dark / 2.80 light** — the light
//!   figure dips under the 3:1 non-text
//!   guidance, recorded for the lane's owner;
//!   the lane's own docs claim the accent
//!   "cannot vanish", which is 0.20 of ratio
//!   too optimistic in that one cell state.
//!
//! # Polish re-audit (Roadmap Wave 1, RM25 — PL-1/PL-2)
//!
//! The decision-free contrast fixes from the UI/UX
//! roadmap's PL-1, plus PL-2's light ladder. Ratios
//! recomputed with this module's own method (Rune's
//! colour semantics); the recomputation reproduces
//! every Wave 9/10 figure above before any candidate
//! is trusted. The accent itself is untouched — the
//! decision pack below stands as written.
//!
//! * **`kbd` chips (S18's 2.88).** The kit chip's
//!   ink rises from `text_secondary` to the primary
//!   text tone, fill unchanged: the pair now measures
//!   6.32 dark / 13.46 light on the chip over the
//!   card, 6.00 dark / 13.30 light over the elevated
//!   surface. Menus, help rows and the palette
//!   footer inherit from the one builder.
//! * **`kbd_on_accent`, adopted.** The palette's
//!   selected row now builds its hint chip through
//!   the on-accent variant (its first production
//!   call site). The variant's Wave 2 colours —
//!   white @ 0.75 ink over a white @ 0.16 fill —
//!   measure only 2.14 dark / 1.71 light on that
//!   row, so the builder takes a keycap treatment
//!   instead: white @ 0.85 fill, black @ 0.85 ink,
//!   measuring 5.12 dark / 5.15 light. The row's
//!   own white-on-accent pair is the decision
//!   below and does not move.
//! * **Muted's consumer record, corrected.** The
//!   Wave 9/10 claim "no real text renders in
//!   muted" stopped being true in Wave 13, when
//!   `menus::dimmed_item` began rendering an Open
//!   recent label — real, must-read text — in
//!   `design::text_muted` (≈ 3.8 dark / ≈ 2.0–2.6
//!   light). RM25 retires muted as that item's
//!   ink: the label takes `text_secondary`
//!   (6.74 dark / 5.88 light on the card), and the
//!   muted-live read is carried by the row's
//!   absent hover/press light-up, as the treatment
//!   always intended. Muted's production consumers
//!   are glyphs again — the graph channel rows'
//!   `glyph_muted` and the splitter grip — which
//!   is the only duty its ratios support.
//! * **Status-strip play segment, light (4.43).**
//!   Fixed at the strip's own seam, not the
//!   accent: in light the strip's playing ink is
//!   the brief accent mixed 0.15 toward black,
//!   measuring 5.00 on the app background. Dark
//!   is unchanged (6.10).
//! * **Dopesheet glyphs on the selected tint.**
//!   The Wave 10 figures (3.07 dark / 2.80 light)
//!   are the SINGLE tint (the current-frame cell
//!   or the selected row). Where both stack —
//!   the selected figure's current-frame cell —
//!   the same derivation gives 2.31 dark / 2.51
//!   light for the pose diamond and 2.32 dark for
//!   the state diamond: under the 3:1 non-text
//!   guidance in a state the lane hits constantly.
//!   On-tint glyphs now take [`accent_on_tint`] /
//!   [`secondary_accent_on_tint`]: the worst
//!   (double-tint) case measures 3.54 dark / 3.57
//!   light for the pose diamond and 3.55 dark /
//!   3.84 light for the state diamond. The
//!   channel dot on the double tint measures
//!   3.06 dark / 4.70 light in `text_secondary`
//!   and is left as it is, recorded here.
//! * **Splitter grip, light (1.94).** Fixed at
//!   the kit seam: in light the grip glyph takes
//!   the full secondary tone (5.15 on the
//!   background, clearing the 3:1 non-text
//!   guidance); dark keeps the muted role
//!   (5.15 there already).
//! * **The light surface ladder.** [`elevated`]
//!   in light is now the card mixed 0.10 toward
//!   the background, so the light ladder is
//!   monotonic by luminance — background 0.869,
//!   [`chrome_surface`] 0.935, elevated 0.987,
//!   card 1.000 — instead of ending in a degenerate
//!   elevated == card step. The pairs measured on
//!   the elevated surface hold (secondary 5.80,
//!   primary 17.19). Recorded plainly: at this
//!   audit `elevated` and `chrome_surface` have
//!   no production consumers yet — floating
//!   surfaces still fill with `card` — so the
//!   ladder is designed at this seam and its
//!   adoption by menus/popovers is a recorded
//!   remainder, not a rendered change.
//! * **Hard-coded themes retired.** The two
//!   offenders named in the Wave 10 check below
//!   are gone: `toasts_view` and the camera
//!   `overlay` now build on `app_theme` for the
//!   caller's appearance. Light toasts therefore
//!   render on the light card and receive the
//!   `*_on` severity variants (5.33 / 5.41 / 5.43
//!   on the card; body text 17.41) they never
//!   saw while the column was pinned dark.
//!
//! # The accent decision — evidence pack (no values changed)
//!
//! The standing question — white text on the
//! accent at 3.14:1, keep or darken — is usually
//! stated as if "the accent" were this module's
//! [`ACCENT_DARK`]. It is not. **Two accents
//! exist.** The fill that actually carries white
//! text is upstream's `tok::accent(0, true)` —
//! `oklch(0.66, 0.17, 256°)`, rendering as
//! `#4392F7`, luminance 0.2845, white on it
//! exactly 3.14 — hard-coded with the dark
//! variant, so the same fill serves **both**
//! appearances. This module's `#6C8CFF` (white
//! on it 3.07) carries no white text anywhere:
//! it works as ink, marker and dot (clips
//! marker 5.19, strip segment 6.10 dark, camera
//! dial marker, dirty dot), and its light twin
//! `#4666D8` already measures 5.06 against
//! white. Darkening this module's constants
//! alone would therefore change **nothing** for
//! button labels; a real fix replaces the
//! `tok::accent(0, true)` fills (or routes them
//! through an app-side fill token), which is a
//! call-site wave, not a value edit.
//!
//! Blast radius of the white-on-upstream-accent
//! pair, production sites at this audit: every
//! active `chrome_view::btn` label and
//! `icon_btn` glyph/label (rest, hover and
//! press fills alike); the menus' three active
//! row builders (label and hint in white); the
//! timeline's current-frame chip number and
//! the marker row's current-cell glyphs; the
//! command palette's selected row — white on
//! the accent at α 0.85 over the elevated
//! surface, measuring **3.55 dark / 2.38
//! light**, the worst instance in the shell.
//! Non-text riders on the same value: the kit
//! toggle pill (white knob), slider progress
//! fill, swatch selection ring, `selected_fill`
//! tint basis, graph-canvas handles, FX /
//! particle row dots, the scene swatch ring.
//!
//! Candidates, computed for a 4.5:1 white-text
//! target (foreground luminance ≤ 0.18333),
//! **none applied**: holding the upstream
//! hue/chroma (OKLCH C 0.17, h 256°) and
//! lowering lightness, the boundary is
//! L ≈ 0.571 (`#2576D9`, exactly 4.50); with
//! headroom for hex rounding, L 0.56 renders
//! `#2072D5` at 4.72 and L 0.55 renders
//! `#1C6FD2` at 4.93 (rounded hexes re-measured
//! 4.74 / 4.94). A linear scale of the rendered
//! `#4392F7` by k = 0.645 (`#3577CC`) sits
//! exactly on 4.50 with no rounding headroom
//! and is recorded only to be ruled out. If
//! the fills instead migrated to this module's
//! accent family, [`ACCENT_DARK`] scaled by
//! k = 0.611 renders `#556FCD` at 4.62 as a
//! rounded hex, and [`ACCENT_LIGHT`] needs no
//! candidate (5.06 already). One interaction
//! the decider should see: the palette row's
//! α 0.85 means even a compliant accent only
//! reaches ≈ 3.0–3.1 there in light (white
//! bleeds through the alpha) — darkening the
//! accent does not fix that row in light; its
//! alpha is a separate choice. The secondary
//! accents carry no white text (no production
//! consumer renders text on them); white on
//! them would be 3.12 dark / 4.81 light.
//!
//! # Hard-coded theme check (Wave 10)
//!
//! The Wave 8 surfaces still resolve through
//! this module: the status strip builds its
//! theme via `app_theme(!light_theme)` at both
//! call sites and `strip_colors` derives from
//! the passed theme; menus take the caller's
//! theme; no new `tok::theme(true)` crept into
//! production code in Waves 8–10. Every other
//! `tok::theme(…)` hit in `src/` is a
//! `#[cfg(test)]` module or a doc comment
//! (verified file by file). Two production
//! offenders remain, both Phase N-era, both
//! internally consistent (upstream text on
//! upstream card, 14.74) and therefore
//! appearance defects, not contrast failures:
//!
//! * `toasts.rs` (`toasts_view`): the whole
//!   toast column — card fill, body text and
//!   the theme fed to the severity helpers —
//!   is the upstream dark theme in both
//!   appearances, so light-theme toasts render
//!   as dark cards and the `*_on` light
//!   variants never reach them.
//! * `camera_panel.rs` (`overlay`): the stage
//!   camera pill likewise builds on
//!   `tok::theme(true)`; its module docs record
//!   the choice as deliberate (pill and buttons
//!   from one theme), but the buttons it must
//!   agree with take the caller's theme, so in
//!   light the pill stays dark.

use rune::tok::{self, Theme};
use rune::Color;

// ---- Brief palette anchors (dark) ----

/// Brief main background.
pub const DARK_BG: &str = "#101216";
/// Brief panel surface.
pub const DARK_CARD: &str = "#1D222B";
/// Brief primary text.
pub const DARK_TEXT: &str = "#F1F3F7";
/// Brief secondary text.
pub const DARK_TEXT_SECONDARY: &str = "#A1A9B7";
/// Brief subtle border (solid; see the module doc's
/// mechanism note for how the two border fields use
/// it).
pub const BRIEF_BORDER: &str = "#303846";
/// Separator alpha: the brief border tinted down over
/// the card, so in-card hairlines stay subordinate to
/// the card edge.
pub const DARK_SEPARATOR_ALPHA: f32 = 0.55;
/// Brief secondary (chrome) surface — no `Theme`
/// field; served by [`chrome_surface`].
pub const DARK_CHROME_SURFACE: &str = "#171B22";
/// Brief elevated surface — no `Theme` field; served
/// by [`elevated`].
pub const DARK_ELEVATED: &str = "#252C37";
/// Brief muted (third-level) text — no `Theme` field;
/// served by [`muted_text`].
pub const DARK_MUTED_TEXT: &str = "#737D8C";
/// Brief primary accent (dark).
pub const ACCENT_DARK: &str = "#6C8CFF";
/// Brief secondary accent (dark).
pub const SECONDARY_ACCENT_DARK: &str = "#9B7CFF";

// ---- Derived light anchors ----
// The brief names no light values; these mirror the
// dark deltas (audit §2): same blue-grey hue shift,
// lightness preserved. See the module doc's table.

/// Light background: upstream `#F0F1F4` shifted to the
/// brief's blue-grey cast at the same lightness.
pub const LIGHT_BG: &str = "#EDF0F5";
/// Light primary text: blue-grey ink.
pub const LIGHT_TEXT: &str = "#171A21";
/// Light secondary text.
pub const LIGHT_TEXT_SECONDARY: &str = "#5B6577";
/// Tint source for the light hairlines (the light ink
/// family), applied at alpha — the light border keeps
/// the upstream alpha mechanism.
pub const LIGHT_BORDER_TINT: &str = "#2A3242";
/// Light card-border alpha (upstream: black @ 0.10).
pub const LIGHT_BORDER_ALPHA: f32 = 0.14;
/// Light separator alpha (upstream: black @ 0.13).
pub const LIGHT_SEPARATOR_ALPHA: f32 = 0.19;
/// Light primary accent: the brief accent's hue
/// family darkened to ≈ 5:1 on the white card (the
/// brief value itself measures only ≈ 3:1 there).
pub const ACCENT_LIGHT: &str = "#4666D8";
/// Light secondary accent: the brief value's hue
/// family darkened to ≈ 4.8:1 on the white card.
pub const SECONDARY_ACCENT_LIGHT: &str = "#7A5AE0";

// ---- Semantic tokens (brief values) ----

/// Brief success tint.
pub const SUCCESS_HEX: &str = "#43C995";
/// Brief warning tint.
pub const WARNING_HEX: &str = "#F1B85B";
/// Brief error *ink* — text/glyph tint only. Never a
/// button fill: see the module doc's danger-fill
/// exception (`chrome_view::DANGER_FILL` stays).
pub const ERROR_INK_HEX: &str = "#EF6974";
/// Success tint darkened for light-theme surfaces
/// (≈ 5.2:1 on the white card).
pub const SUCCESS_LIGHT_HEX: &str = "#177A52";
/// Warning tint darkened for light-theme surfaces
/// (≈ 5.3:1 on the white card).
pub const WARNING_LIGHT_HEX: &str = "#8F6108";
/// Error ink darkened for light-theme surfaces
/// (≈ 5.4:1 on the white card).
pub const ERROR_LIGHT_HEX: &str = "#C62F3B";

/// A fixed sRGB hex literal as a Rune colour, via the
/// shell's hex idiom (the same conversion
/// `chrome_view::danger_color` performs). The fallback
/// is unreachable — every literal in this module is a
/// valid 6-digit hex — and exists only because the
/// parser is fallible in general.
fn srgb(hex: &str) -> Color {
    rune::color::hex(hex).unwrap_or(Color::WHITE)
}

/// The shell's theme: `tok::theme(dark)` with the
/// mission brief's palette applied centrally (see the
/// module doc's mapping tables). The single theme
/// construction site — `main.rs::view` calls this
/// instead of `tok::theme`.
pub fn app_theme(dark: bool) -> Theme {
    let base = tok::theme(dark);
    if dark {
        Theme {
            bg: srgb(DARK_BG),
            card: srgb(DARK_CARD),
            text: srgb(DARK_TEXT),
            text_secondary: srgb(DARK_TEXT_SECONDARY),
            card_border: srgb(BRIEF_BORDER),
            separator: srgb(BRIEF_BORDER).alpha(DARK_SEPARATOR_ALPHA),
            ..base
        }
    } else {
        Theme {
            bg: srgb(LIGHT_BG),
            // card retained: upstream's pure white is
            // the brief's light panel direction.
            text: srgb(LIGHT_TEXT),
            text_secondary: srgb(LIGHT_TEXT_SECONDARY),
            card_border: srgb(LIGHT_BORDER_TINT).alpha(LIGHT_BORDER_ALPHA),
            separator: srgb(LIGHT_BORDER_TINT).alpha(LIGHT_SEPARATOR_ALPHA),
            ..base
        }
    }
}

/// Whether a theme is the dark appearance. `Theme`
/// carries no dark flag, so this reads the one field
/// whose lightness defines the appearance: the
/// background's luminance (dark ≈ 0.005, light
/// ≈ 0.85 — the 0.5 threshold sits far from both, for
/// upstream and overridden backgrounds alike).
pub fn is_dark(t: &Theme) -> bool {
    t.bg.luminance() < 0.5
}

/// The brief's primary accent for a theme's
/// appearance: `#6C8CFF` in dark, the darkened
/// [`ACCENT_LIGHT`] in light. The theme-aware
/// successor to the 13 hard-coded `tok::accent(0,
/// true)` call sites (audit §4.1); those sites
/// migrate in the adoption wave.
pub fn accent(t: &Theme) -> Color {
    accent_on(is_dark(t))
}

/// [`accent`] for a caller that already knows the
/// appearance flag (the form `main.rs`-side state
/// naturally holds).
pub fn accent_on(dark: bool) -> Color {
    if dark {
        srgb(ACCENT_DARK)
    } else {
        srgb(ACCENT_LIGHT)
    }
}

/// The brief's secondary accent (`#9B7CFF`) for a
/// theme's appearance, with the same light darkening
/// as [`accent`].
pub fn secondary_accent(t: &Theme) -> Color {
    if is_dark(t) {
        srgb(SECONDARY_ACCENT_DARK)
    } else {
        srgb(SECONDARY_ACCENT_LIGHT)
    }
}

/// How far the on-tint glyph variants lift toward
/// white in the dark appearance (see
/// [`accent_on_tint`]).
pub const TINT_GLYPH_DARK_LIFT: f32 = 0.25;

/// How far the on-tint accent variant deepens toward
/// black in the light appearance (see
/// [`accent_on_tint`]).
pub const TINT_GLYPH_LIGHT_DEEPEN: f32 = 0.35;

/// The upstream accent adjusted for glyph duty on
/// the selected tint (the accent at α 0.16 over the
/// card — the dopesheet's current-frame cells and
/// selected rows). The plain accent measures 3.07
/// dark / 2.80 light on a single tint layer and
/// 2.31 / 2.51 where two layers stack, dipping
/// under the 3:1 non-text guidance; this variant
/// lifts toward white in dark and deepens toward
/// black in light so the worst (stacked) case
/// measures 3.54 dark / 3.57 light, and the single
/// layer 4.72 / 3.98 (module doc, Polish re-audit).
/// Off-tint glyphs keep the plain accent — this is
/// a local correction at the tint seam, not an
/// accent change (the accent decision is the
/// module doc's standing evidence pack, untouched).
pub fn accent_on_tint(t: &Theme) -> Color {
    let base = tok::accent(0, true);
    if is_dark(t) {
        base.mix(Color::WHITE, TINT_GLYPH_DARK_LIFT)
    } else {
        base.mix(Color::BLACK, TINT_GLYPH_LIGHT_DEEPEN)
    }
}

/// [`secondary_accent`] adjusted for glyph duty on
/// the selected tint, the companion of
/// [`accent_on_tint`]: the secondary accent measures
/// 2.32 on the stacked tint in dark (the state
/// diamond's coincidence border case), so dark
/// lifts it by the same factor; in light it already
/// measures 4.29 single / 3.84 stacked and is
/// returned unchanged.
pub fn secondary_accent_on_tint(t: &Theme) -> Color {
    let base = secondary_accent(t);
    if is_dark(t) {
        base.mix(Color::WHITE, TINT_GLYPH_DARK_LIFT)
    } else {
        base
    }
}

/// The secondary (chrome) surface: between the app
/// background and the panel card — app bar, status
/// strip and other shell furniture. `Theme` has no
/// field for it. Dark: the brief's `#171B22`. Light:
/// derived midway between `bg` and `card`.
pub fn chrome_surface(t: &Theme) -> Color {
    if is_dark(t) {
        srgb(DARK_CHROME_SURFACE)
    } else {
        t.bg.mix(t.card, 0.5)
    }
}

/// How far the light elevated surface steps from
/// the card toward the background (Roadmap Wave 1,
/// RM25): enough to separate a floating surface from
/// the panels under it, small enough that the pairs
/// measured on the surface hold (secondary 5.80,
/// primary 17.19 — see the module doc's Polish
/// re-audit).
pub const LIGHT_ELEVATED_BG_MIX: f32 = 0.10;

/// The elevated surface: menus, popovers and dialogs
/// raised above the panel card. `Theme` has no field
/// for it. Dark: the brief's `#252C37`. Light: the
/// card stepped [`LIGHT_ELEVATED_BG_MIX`] toward the
/// background — the third rung of the light ladder
/// (background → chrome → elevated → card by
/// luminance; see the module doc's Polish re-audit).
/// Before RM25 the light value equalled the card, so
/// light elevation rested on shadow and border
/// alone.
pub fn elevated(t: &Theme) -> Color {
    if is_dark(t) {
        srgb(DARK_ELEVATED)
    } else {
        t.card.mix(t.bg, LIGHT_ELEVATED_BG_MIX)
    }
}

/// Muted (third-level) text, below even
/// `text_secondary`: captions, hints, disabled-adjacent
/// labels — non-essential text only. `Theme` has no
/// field for it. Dark: the brief's `#737D8C`. Light:
/// `text_secondary` mixed toward `bg`, a greyer step
/// down at the same family.
pub fn muted_text(t: &Theme) -> Color {
    if is_dark(t) {
        srgb(DARK_MUTED_TEXT)
    } else {
        t.text_secondary.mix(t.bg, 0.3)
    }
}

/// The brief's success tint (`#43C995`).
pub fn success() -> Color {
    srgb(SUCCESS_HEX)
}

/// The brief's warning tint (`#F1B85B`).
pub fn warning() -> Color {
    srgb(WARNING_HEX)
}

/// The brief's error ink (`#EF6974`) — a text/glyph
/// tint on a surface, **never** a fill behind white
/// text (see the module doc's measured exception;
/// the danger fill stays `chrome_view::DANGER_FILL`).
pub fn error_ink() -> Color {
    srgb(ERROR_INK_HEX)
}

/// [`success`] for a theme's appearance: the brief
/// value in dark, darkened to ≈ 5:1 on the white card
/// in light.
pub fn success_on(t: &Theme) -> Color {
    if is_dark(t) {
        success()
    } else {
        srgb(SUCCESS_LIGHT_HEX)
    }
}

/// [`warning`] for a theme's appearance.
pub fn warning_on(t: &Theme) -> Color {
    if is_dark(t) {
        warning()
    } else {
        srgb(WARNING_LIGHT_HEX)
    }
}

/// [`error_ink`] for a theme's appearance.
pub fn error_on(t: &Theme) -> Color {
    if is_dark(t) {
        error_ink()
    } else {
        srgb(ERROR_LIGHT_HEX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_theme_carries_the_brief_palette() {
        let t = app_theme(true);
        assert_eq!(t.bg.hex(), "#101216ff");
        assert_eq!(t.card.hex(), "#1d222bff");
        assert_eq!(t.text.hex(), "#f1f3f7ff");
        assert_eq!(t.text_secondary.hex(), "#a1a9b7ff");
        assert_eq!(t.card_border.hex(), "#303846ff");
        // Separator: the brief border at the separator
        // alpha — tint source exact, alpha preserved.
        let sep = t.separator;
        let sep_hex = sep.hex();
        assert_eq!(&sep_hex[..7], "#303846");
        assert!((sep.a - DARK_SEPARATOR_ALPHA).abs() < 1e-6);
    }

    #[test]
    fn dark_theme_retains_upstream_mechanism_fields() {
        let t = app_theme(true);
        let up = tok::theme(true);
        assert_eq!(t.hover, up.hover);
        assert_eq!(t.press, up.press);
        assert_eq!(t.control_off, up.control_off);
        assert_eq!(t.knob, up.knob);
        assert_eq!(t.shadow, up.shadow);
    }

    #[test]
    fn light_theme_mirrors_the_dark_deltas() {
        let t = app_theme(false);
        assert_eq!(t.bg.hex(), "#edf0f5ff");
        assert_eq!(t.text.hex(), "#171a21ff");
        assert_eq!(t.text_secondary.hex(), "#5b6577ff");
        // Card retained from upstream (pure white).
        assert_eq!(t.card, tok::theme(false).card);
        // Hairlines keep the alpha mechanism, tinted.
        assert!((t.card_border.a - LIGHT_BORDER_ALPHA).abs() < 1e-6);
        assert!((t.separator.a - LIGHT_SEPARATOR_ALPHA).abs() < 1e-6);
    }

    #[test]
    fn accent_follows_the_appearance() {
        assert_eq!(accent(&app_theme(true)).hex(), "#6c8cffff");
        assert_eq!(accent(&app_theme(false)).hex(), "#4666d8ff");
        // Also correct on raw upstream themes, so
        // not-yet-migrated callers get the right value.
        assert_eq!(accent(&tok::theme(true)).hex(), "#6c8cffff");
        assert_eq!(accent(&tok::theme(false)).hex(), "#4666d8ff");
        assert_eq!(secondary_accent(&app_theme(true)).hex(), "#9b7cffff");
    }

    #[test]
    fn missing_roles_resolve_to_the_brief_values_in_dark() {
        let t = app_theme(true);
        assert_eq!(chrome_surface(&t).hex(), "#171b22ff");
        assert_eq!(elevated(&t).hex(), "#252c37ff");
        assert_eq!(muted_text(&t).hex(), "#737d8cff");
        let light = app_theme(false);
        // RM25: the light elevated surface is its own
        // ladder rung now, no longer the card itself.
        assert_eq!(
            elevated(&light),
            light.card.mix(light.bg, LIGHT_ELEVATED_BG_MIX)
        );
        assert_ne!(elevated(&light), light.card);
    }

    /// WCAG ratio between two opaque colours, Rune
    /// semantics (luminance over linear values).
    fn ratio(fg: Color, bg: Color) -> f32 {
        let (a, b) = (fg.luminance(), bg.luminance());
        let (hi, lo) = if a >= b { (a, b) } else { (b, a) };
        (hi + 0.05) / (lo + 0.05)
    }

    /// The selected tint as `layers` coats of the
    /// accent at α 0.16 over the card (one coat: a
    /// current-frame cell or a selected row; two:
    /// the selected row's current-frame cell).
    fn tint(t: &Theme, layers: usize) -> Color {
        let mut c = t.card;
        for _ in 0..layers {
            c = c.mix(tok::accent(0, true), 0.16);
        }
        c
    }

    #[test]
    fn light_surface_ladder_is_monotonic() {
        // RM25 (PL-2): bg < chrome < elevated < card
        // by luminance in the light appearance — the
        // ladder the Wave 10 record found degenerate
        // (elevated == card).
        let t = app_theme(false);
        assert!(t.bg.luminance() < chrome_surface(&t).luminance());
        assert!(chrome_surface(&t).luminance() < elevated(&t).luminance());
        assert!(elevated(&t).luminance() < t.card.luminance());
        // The pairs measured on the light elevated
        // surface still clear the text bar.
        assert!(ratio(t.text, elevated(&t)) >= 4.5);
        assert!(ratio(t.text_secondary, elevated(&t)) >= 4.5);
    }

    #[test]
    fn on_tint_glyph_pairs_clear_the_non_text_bar() {
        // RM25 (PL-1): the derivation behind the
        // module doc's dopesheet figures — the plain
        // accent reproduces the Wave 10 audit on one
        // tint layer (3.07 dark / 2.80 light) and
        // dips under 3:1 where two layers stack,
        // while the on-tint variants clear 3:1 with
        // headroom in every case.
        for dark in [true, false] {
            let t = app_theme(dark);
            let plain = tok::accent(0, true);
            let single = tint(&t, 1);
            let stacked = tint(&t, 2);
            let audit = if dark { 3.07 } else { 2.80 };
            assert!(
                (ratio(plain, single) - audit).abs() < 0.03,
                "plain accent on single tint, dark={dark}: {}",
                ratio(plain, single)
            );
            assert!(ratio(plain, stacked) < 3.0, "stacked plain dips, dark={dark}");
            assert!(ratio(accent_on_tint(&t), single) >= 3.4, "dark={dark}");
            assert!(ratio(accent_on_tint(&t), stacked) >= 3.4, "dark={dark}");
            assert!(ratio(secondary_accent_on_tint(&t), single) >= 3.4, "dark={dark}");
            assert!(ratio(secondary_accent_on_tint(&t), stacked) >= 3.4, "dark={dark}");
        }
        // The light secondary needs no adjustment and
        // gets none.
        let light = app_theme(false);
        assert_eq!(
            secondary_accent_on_tint(&light),
            secondary_accent(&light)
        );
    }

    #[test]
    fn semantic_tokens_are_the_brief_values() {
        assert_eq!(success().hex(), "#43c995ff");
        assert_eq!(warning().hex(), "#f1b85bff");
        assert_eq!(error_ink().hex(), "#ef6974ff");
        let light = app_theme(false);
        assert_eq!(success_on(&light).hex(), "#177a52ff");
        assert_eq!(warning_on(&light).hex(), "#8f6108ff");
        assert_eq!(error_on(&light).hex(), "#c62f3bff");
        let dark = app_theme(true);
        assert_eq!(success_on(&dark), success());
        assert_eq!(error_on(&dark), error_ink());
    }
}
