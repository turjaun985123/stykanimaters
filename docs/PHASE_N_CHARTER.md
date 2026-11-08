# Phase N Charter — the UI/UX phase (100 agents)

Ordered 2026-11-08: *"Assign 100 agents some for planning
others will write code, main focus is making UI, UX
components and proper use and make it great looking app
as professional useful and very satisfying UX and UI."*

Phase N is a **visual and interaction-design phase**. It
changes how the shell looks and feels — panel cards,
headers, controls, lists, menus, feedback — and **no
engine behaviour**. Every op, handler and shortcut keeps
working exactly as before; only structure and styling
move.

## Why (the groundwork audit, 2026-11-08)

- Panels are **flat**: each is a bare `col()` with a
  hand-rolled text heading — no card fill, no border, no
  padding — so the side columns read as one long wall of
  controls floating on the window background.
- Headings are re-invented per file: at least five
  private `section()` / `section_label()` helpers with
  different looks (command_palette, easing_panel,
  export_panel, fx_browser, figure_add).
- Buttons have three implementations: `chrome_view::btn`,
  `icon_btn` (+ inert twins), and per-file `fbtn` copies
  with their own disabled handling (frame_ops).
- Steppers, toggles, sliders, list rows, swatches and
  segmented rows are rebuilt ad hoc in almost every
  panel, with drifting metrics.
- Rune already ships a token system the shell barely
  uses: `rune::tok` spacing (S1 4 / S2 8 / S3 12 /
  S6 24), radii (SM 6 / MD 8 / LG 12), fonts (BODY
  13/400, LABEL 12/500, SECTION 11.5/600, TITLE 13/600,
  MONO_CAPTION 12/500) and a `Theme` with `card`,
  `card_border`, `separator`, `control_track`,
  `control_off`, `hover`, `press`, `indicator`, `knob`,
  `shadow` fields. Phase N builds the shell's design
  language on these tokens instead of new literals.

## The design language — "card + hairline"

1. **Every panel is a card.** Root: fill `t.card`,
   border 1.0 `t.card_border`, radius 12 (LG), padding
   12 (S3), internal gap 8 (S2). The panel's root key
   does not change.
2. **One header.** Header row: optional 13px icon in
   `text_secondary`, title in `type_::SECTION` /
   `t.text`, meta/count in LABEL / `text_secondary`,
   trailing controls at the right. A hairline `divider`
   (1px `t.separator`) under the header.
3. **Section labels** inside panels: `type_::SECTION`,
   `text_secondary`, from the kit — never hand-rolled.
4. **One button family.** `chrome_view::btn`,
   `icon_btn`, `icon_only_btn` and their inert twins.
   Metrics: px 10 / py 5, radius 6 (SM). Active =
   accent fill + white text. Inert = muted, no handler.
   Local `fbtn` copies are retired onto these.
5. **List rows.** Min height 28, px 8, radius 8 (MD),
   hover `t.hover`; selected = accent at 16% alpha fill
   (+ accent border at 35% where a border is drawn).
6. **Steppers.** Label grows; − button; value in
   MONO_CAPTION, centred, min-width 34; + button.
7. **Toggles.** One pill switch from the kit: 28×16
   track (accent when on, `control_off` when off), 12px
   knob (`t.knob`) sliding by alignment. The existing
   control key stays on the pressable root.
8. **Sliders.** Drag logic stays where it lives; the
   track visual comes from the kit (track
   `control_track`, fill accent, knob dot at the value).
9. **Menus, popovers, palette.** Card fill, border,
   radius 12, padding 6; item rows 28 high, px 8,
   radius 6, hover `t.hover`; shortcut hints as `kbd`
   chips; separators are kit dividers.
10. **Empty states.** Kit `empty_state`: centred 20px
    icon + LABEL text, both `text_secondary`, py 12.
    No bare "nothing here" text rows.
11. **Kbd chips.** Kit `kbd`: px 5 / py 1, radius 4,
    fill `control_off`, MONO_CAPTION, `text_secondary`.
    Used by help rows, menu hints, palette footer.
12. **Status strip.** Segments separated by hairline
    ticks (1×10 `separator`); numbers in MONO_CAPTION
    where a segment is numeric.
13. **Timeline block.** The timeline stack (ruler,
    chips, markers, clips lane, loop row) sits in one
    card: fill `t.card`, radius 12, padding 8. Chips
    keep their keys and geometry constants.
14. **Stage.** Radius 12; the camera overlay becomes a
    floating card pill (fill `t.card`, radius 999,
    px 10 / py 4, border `card_border`).
15. **App bar.** TITLE project name, dirty dot in the
    accent when the project is modified, hairline
    divider under the bar + workspace row.

Honest limits, stated once: Rune's presence/spring
system is not a per-control motion toolkit — Phase N
does not promise animation curves it cannot tune; the
win is hierarchy, spacing, states and consistency.
Text-field internals are rune-kit components; panels
style their frames, not the kit's editor core.

## The kit — `src/rune_sec/design.rs` (FIXED API)

One foundation agent builds this module; every
adoption agent imports `crate::rune_sec::design`.
Signatures are fixed here; the builder adapts them to
the real `El` API (`fill`, `radius`, `border(w, c)`,
`shadow(y, blur, c)`, `px`, `py` all exist in rune's
`el` builders) and reports any forced deviation in its
report, and the integrator re-checks consumers against
the landed file.

```rust
pub const PANEL_RADIUS: f32 = 12.0;
pub const PANEL_PAD: f32 = 12.0;
pub const PANEL_GAP: f32 = 8.0;
pub const ROW_RADIUS: f32 = 8.0;
pub const CONTROL_RADIUS: f32 = 6.0;

pub fn accent() -> Color;                    // tok::accent(0, true), matching chrome_view::btn
pub fn selected_fill() -> Color;             // accent at 0.16 alpha
pub fn panel_col(key: &str, t: &Theme) -> El;              // styled panel root (col)
pub fn header_row(key: &str) -> El;                        // styled header row shell
pub fn panel_title(icon: Option<&str>, title: &str, meta: Option<&str>, t: &Theme) -> El; // growing left cluster
pub fn section_label(label: &str, t: &Theme) -> El;
pub fn divider(t: &Theme) -> El;
pub fn kbd(label: &str, t: &Theme) -> El;
pub fn empty_state(icon: &str, message: &str, t: &Theme) -> El;
pub fn list_row(key: &str, selected: bool, t: &Theme) -> El;   // styled row shell (no handler)
pub fn toggle(key: &str, on: bool, t: &Theme, f: impl Fn(&mut State) + 'static) -> El;
pub fn toggle_row(key: &str, label: &str, on: bool, t: &Theme, f: impl Fn(&mut State) + 'static) -> El;
pub fn stepper_value(value: &str, t: &Theme) -> El;            // centred mono readout
pub fn slider_track(frac: f32, t: &Theme) -> El;               // visual track + fill + knob
```

## Amendments ratified after the planning wave (integrator, 2026-11-08)

The 20 planning reports (`docs/phase_n_plans/`) surfaced
gaps in the fixed API above and questions needing one
ruling each. These amendments are part of the charter;
where they extend the kit, the signatures below are as
fixed as the ones above.

**Kit extensions — `design.rs` also provides:**

```rust
pub fn divider_keyed(key: &str, t: &Theme) -> El;   // P7: menu separators are keyed (menu-file-sep, …)
pub fn row_shell(selected: bool, t: &Theme) -> El;  // P8: unkeyed list-row shell (text segment rows)
pub fn text_muted(t: &Theme) -> Color;              // P2: third text level = text_secondary at 0.6 alpha, never a new hex
pub fn swatch(key: &str, color: Color, selected: bool, t: &Theme, f: impl Fn(&mut State) + 'static) -> El; // P3/P18: one swatch construction, 24px
```

`stepper_value` and `kbd` apply `type_::MONO_CAPTION`
**and** `.mono()` — the token alone does not set the
Mono family in the pinned Rune rev (P9/P11).

**Button family additions (P5)** — the family grows so
the local dialects have somewhere to land:

- `chrome_view::btn_inert(key, label, t)` — inert text
  button: card fill, `text_secondary` ink, no handler,
  no hover/press.
- `chrome_view::btn_muted_live(key, label, active, t, f)`
  — muted visual with the handler KEPT (frame_ops'
  refusal-status semantics are designed behaviour).
- `chrome_view::danger_btn(key, label, t, f)` — one
  danger button with named colour constants; the fill
  follows P12's measured fix (white on `0xF16F78`
  measures 2.88:1 and fails; use the deeper red P12
  verifies at ≈5:1).
- `icon_btn::chip_btn(key, icon: Option<&str>, label, active, t, f)`
  — compact chip absorbing `row_toggle` / `cap_chip` /
  `preset_chip` / `picker_cell`.
- `icon_btn::danger_icon_btn(key, icon, label, active, t, f)`
  — unifies the two drifted `danger_btn` copies
  (figure: key/label; segment: key/icon/label).

**Rulings:**

- **Stepper order is the charter order** (label grows,
  −, value, +) even where a panel's majority order
  differs today (P9 D1).
- **The wind slider keeps its bipolar centre-fill
  visual** — `slider_track` is unipolar only; the wind
  slider is a documented exception, not a defect
  (P9 D2 / P3).
- **The palette's selected row keeps its accent 0.85
  fill** — transient launcher focus is not persistent
  list selection (P7).
- **The timeline card is unkeyed**; idle chip fill
  becomes `t.control_off` so chips read on the card
  (P6). The card wraps OUTSIDE the horizontal scroll
  row — never between the scroll row and its content.
- **Status strip mono applies to the timecode and
  speed segments only**; segment dividers become
  1×10 `t.separator` (P6/P10). The Phase K width-floor
  arithmetic and the audio-glyph refusal stand.
- **Toast severity is wired**: `classify_status()` in
  toasts.rs (pure, pattern-matched on real engine
  status strings) + one integrator-owned line in
  main.rs `poll` (P10). No status string changes.
- **Mount-order moves are integrator-owned** main.rs
  child-order changes, done at foundation integration
  (P16): Scene demoted below the Particles/Audio block;
  Playback + FrameOps promoted above Camera; Graph
  before KeyframeTools; ThreeD before Camera. The
  stage radius 8→12 (P6) and the waveform card wrap
  (P4) are likewise integrator items. No agent edits
  main.rs.
- **`AppSpec::tab_visits_controls` stays OFF** — a
  deferred product decision; enabling it makes Space
  press a focused Play button and arrows feed sliders
  instead of frame-stepping (P14).
- **White on the kit accent measures 4.19:1** — a
  documented acceptance: accent fills carry short
  labels and icons only, never body text (P12).
- **Restyle roots, never nest wrappers** — card roots
  replace panel roots; the element-growth budget for
  the whole phase is +8–11% on the default tree (P19).
- The stale Delete help row ("selected segment") may
  be corrected in the V-wave copy pass — it is
  documentation text, not a status string (P14).

## Rules for every Phase N agent

- **One file.** Your brief names it. No main.rs, no
  mod.rs, no other agent's file, no engine (lib) files
  except where a brief explicitly names one.
- **No cargo, no git, no commits.** Verification is
  CI-only (the repo's standing rule).
- **Every element key survives byte-identical** (733
  keyed elements at the Phase M baseline; the gate
  diffs sets). New keys only where your brief allows;
  report every one.
- **Handlers move verbatim.** Restructure and restyle
  the element tree; never change what a press, drag or
  edit does, never change a status string, never
  rename a `tr()` i18n key.
- **No new State fields.** If a visual state seems to
  need one, derive it from existing state or report
  the blocker — do not invent state.
- Read this charter and, for adoption work, your
  area's plan in `docs/phase_n_plans/` first.
- Report: the visual changes (before → after in one
  line each), keys added (none expected), tests added,
  and anything you could not do, honestly.

## Waves

- **P — planning (20 agents):** audits + specs in
  `docs/phase_n_plans/`, each scoped to one surface or
  one cross-cutting concern, each ending in a concrete,
  file-scoped work list the coding wave can execute.
- **F — foundation (10 agents):** `design.rs` and the
  shared surfaces (chrome buttons, icon buttons,
  menus, palette, toasts, status strip, settings,
  workspace chips, stage chrome). Integrated and
  pushed by the integrator before adoption starts.
- **A — adoption (~50 agents):** one panel file each;
  adopt the card shell, header, kit controls; retire
  local helpers onto the kit.
- **V — verification & docs (~20 agents):** Runtime
  round-11 visual tests, new headless UI scenes, gate
  widening, help/copy pass, PHASE_N.md + QA §16.

Total: 100 agents.
