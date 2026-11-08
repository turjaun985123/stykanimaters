# Phase N — P3: Component Duplication Census

Scope: every locally-defined builder/helper in the shell
(`src/chrome_view.rs`, `src/stage_view.rs`,
`src/rune_sec/*.rs`) that creates a button, section
heading, label/readout, stepper, slider, toggle, list
row, swatch, chip, divider or empty state — what it
costs today, how it differs from the charter's kit
(`src/rune_sec/design.rs`, fixed API in
`docs/PHASE_N_CHARTER.md`), and what replaces it.

Method: all 156 production `-> El` functions in the
shell were extracted and read, plus name-based sweeps
(`fbtn`, `kbtn`, `pbtn`, `ctl`, `*_row`, `swatch`,
`chip`, `section*`, `separator`, `divider`…) and
string sweeps for inline empty states. Test modules
are excluded. Panel *roots* (e.g. `audio_panel`,
`panel`, `*_section`) are not primitives — they appear
in the per-file checklist as `panel_col`/`panel_title`
adoption sites, not in the tables.

## Headline findings

1. **Buttons: 3 canonical + 7 local dialects.**
   `chrome_view::btn` + the `icon_btn` family are the
   charter family. On top of them sit `fbtn`
   (frame_ops), `kbtn` (keyframe_tools), `pbtn`/`pibtn`
   (pose_tools), `ctl`/`ictl` (three_d_panel),
   `row_toggle` (figures_panel) and two `danger_btn`
   copies — differing in exactly one dimension each:
   *how a disabled/dim button behaves* (see table A).
   That one dimension is the whole duplication.
2. **Section headings: 11 helpers, 3 different looks.**
   Two use `type_::SECTION` (export ×2), eight use
   `type_::LABEL` in `text_secondary`, one is a 22px
   row (palette). The charter look is `SECTION` —
   so most panels' headings will *change size* on
   adoption; that is intended, and it is the single
   most visible change in Phase N.
3. **Steppers: 10 row helpers, 3 value placements.**
   Label-left/value-right (inspectors), label/value
   sandwiched by −/+ (scene, stroke), and label-less
   inline rows (onion, draw, settings). The kit only
   standardises the value (`stepper_value`), so every
   row is rebuilt as label + `btn(−)` +
   `stepper_value` + `btn(+)`.
4. **Toggles: zero pill switches exist.** Every
   toggle in the app is an active-state `btn`/
   `icon_btn` (onion, guides ×3, loop, follow,
   fx-hide, row toggles…). `design::toggle` /
   `toggle_row` are net-new at ~15 call-site groups —
   the biggest single UX change in the phase.
5. **Sliders: exactly 2 visual copies** (scene
   weather, layers opacity) with near-identical
   hand-drawn track/fill/knob, plus the camera dial
   as a custom third. Both copies map onto
   `slider_track(frac)` for the visual; drag logic
   stays local per the charter.
6. **Swatches: 5 implementations, 4 sizes**
   (22², 18², 26-slot/22-chip, 12² mini) and **3
   different "current" markers** (white 2px border,
   white outline ring, accent frame/fill). The kit
   has *no* swatch function — recorded as kit gap
   G1 below; adoption normalises the marker onto
   `selected_fill()`/accent until F-wave decides.
7. **Dividers: 2 helpers, neither is the charter
   hairline.** menus' is keyed + `t.separator` (closest
   to kit), status_strip's is a 1×12 vertical tick in
   `control_track`. Panels have *no* header divider
   at all today — kit `divider` under every header is
   net-new structure.
8. **Empty states: no helper exists.** At least 10
   bare `LABEL`/`text_secondary` text lines (table H)
   become `empty_state(icon, message, t)`.
9. **Kbd: 1 helper with the wrong recipe**
   (help `key_cap`: card fill + border + `text`
   colour), and menu hints are bare text, not chips.
   Both become kit `kbd`.

## Kit gaps found (for the F wave, not blockers)

- **G1 — no swatch/chip primitive** in the design.rs
  API. Five swatch builders + four chip builders have
  nothing to retire *onto*; census recommendation:
  F-wave adds `swatch(key, rgb, current, t, f)` and a
  segmented-chip shell, or adoption keeps local
  builders restyled to shared metrics (22², radius 6,
  accent frame via `selected_fill()`).
- **G2 — no danger-button variant.** Two identical
  `danger_btn` copies hard-code `#F16F78`/`#D85560`
  literals (not theme fields). Consolidate into one
  shared builder in icon_btn.rs or design.rs during
  F-wave; do not leave two copies.
- **G3 — no readout-row / menu-item / format-row
  primitive.** `readout_row` ×2, menu item rows ×3
  and export format rows ×3 are structural
  duplicates whose only kit support is metrics
  (charter §5/§9) + `section_label`/`kbd` parts.

---

## Table A — Buttons

Canonical family (KEEP — these *are* the charter §4
family; F-wave aligns their metrics, files do not
replace them):

| Helper | File:line | Signature | Notes |
|---|---|---|---|
| `btn` | src/chrome_view.rs:38 | `pub fn btn(key: &str, label: &str, active: bool, t: &Theme, f: impl Fn(&mut State) + 'static) -> El` | px10/py5, r6; active = accent fill + white; inactive fill `t.card`, hover `control_track`, press `control_off`; LABEL font. Baseline for all comparisons below. |
| `icon_btn` | src/rune_sec/icon_btn.rs:155 | `pub fn icon_btn(key: &str, icon: &str, label: &str, active: bool, t: &Theme, on_press: impl Fn(&mut State) + 'static) -> El` | `btn` metrics + 13px glyph, `ICON_LABEL_GAP`; unknown icon degrades to `btn`. |
| `icon_btn_inert` | src/rune_sec/icon_btn.rs:199 | `pub fn icon_btn_inert(key: &str, icon: &str, label: &str, t: &Theme) -> El` | Same box; fill `card` (inert), ink `text_secondary`, **no handler/hover/press**. The inert twin every local copy re-invents. |
| `icon_only_btn` | src/rune_sec/icon_btn.rs:230 | `pub fn icon_only_btn(key: &str, icon: &str, size: f32, active: bool, t: &Theme, on_press: impl Fn(&mut State) + 'static) -> El` | Square `size`², no padding, glyph centred. |
| `icon_only_btn_inert` | src/rune_sec/icon_btn.rs:267 | `pub fn icon_only_btn_inert(key: &str, icon: &str, size: f32, t: &Theme) -> El` | Inert twin of the above. |
| `icon_el` / `icon_el_rgb` | src/rune_sec/icons.rs:139 / :148 | `pub fn icon_el(name: &str, size: f32, color: Color) -> El` / `(name, size, rgb: u32)` | Glyph only, not a button. Used by `panel_title`, `empty_state`, menu items. KEEP. |

Local copies (RETIRE):

| Helper | File:line | Signature | How it differs from the kit family | Replacement |
|---|---|---|---|---|
| `fbtn` | src/rune_sec/frame_ops.rs:517 | `fn fbtn(key: &str, label: &str, active: bool, muted: bool, t: &Theme, f: impl Fn(&mut State) + 'static) -> El` | Same box metrics as `btn`, but `muted` is a *third* state: fill/hover/press all `control_off`, ink `text_secondary`, **handler stays wired** (press lands on the op's refusal status). The only button whose disabled form still fires. | Live+active → `btn`; muted → the muted-but-wired case has no kit twin: F-wave decision — either add `btn_muted_wired` beside `btn`, or accept `icon_btn_inert`-style inert (changes refusal-status behaviour — **flagged: behaviour-sensitive, integrator must rule**). Do NOT silently make them inert. |
| `kbtn` | src/rune_sec/keyframe_tools.rs:1001 | `fn kbtn(key: &str, label: &str, enabled: bool, t: &Theme, f: impl Fn(&mut State) + 'static) -> El` | `btn` box; disabled = `control_off` fill + secondary ink + **no handler**, but fill differs from `icon_btn_inert` (`control_off` vs `card`) and there is no icon slot. | `btn` when enabled; a text-only inert twin is missing from the family — use `icon_btn_inert` only where a glyph exists, else F-wave adds `btn_inert(key,label,t)` (same box, inert fills). Census recommendation: add `btn_inert` in F. |
| `pbtn` | src/rune_sec/pose_tools.rs:1152 | `fn pbtn(key: String, label: &str, enabled: bool, t: &Theme, f: impl Fn(&mut State) + 'static) -> El` | Byte-for-byte the `kbtn` recipe (same fills, same no-handler disabled) with an owned `String` key. Two files, one idea, two copies. | Same as `kbtn`: `btn` / new `btn_inert`. |
| `pibtn` | src/rune_sec/pose_tools.rs:1193 | `fn pibtn(key: String, icon: &str, label: &str, enabled: bool, t: &Theme, f: impl Fn(&mut State) + 'static) -> El` | Already a pure dispatch: live → `icon_btn`, disabled → `icon_btn_inert`. No visual duplication left. | DELETE the wrapper; call `icon_btn`/`icon_btn_inert` at the 7 call sites (or keep as a 6-line adapter — census prefers delete). |
| `ctl` | src/rune_sec/three_d_panel.rs:633 | `fn ctl(key: &str, label: &str, active: bool, dim: bool, t: &Theme, f: impl Fn(&mut State) + 'static) -> El` | Live branch already delegates to `btn`. `dim` branch is `fbtn`'s recipe again (all-`control_off`, secondary ink, **handler wired**). | Live → `btn` directly; dim → same ruling as `fbtn` muted (behaviour-sensitive). |
| `ictl` | src/rune_sec/three_d_panel.rs:673 | `fn ictl(key: &str, icon: &str, label: &str, active: bool, dim: bool, t: &Theme, f: impl Fn(&mut State) + 'static) -> El` | Pure dispatch: live → `icon_btn`, dim → `icon_btn_inert`. | DELETE wrapper; call the family at call sites. |
| `danger_btn` | src/rune_sec/inspector_figure.rs:807 | `fn danger_btn(key: &str, label: &str, f: impl Fn(&mut State) + 'static) -> El` | `btn` box in hard-coded error red `#F16F78`, hover/press `#D85560`, white label; no theme field, no icon. | Kit gap G2: one shared danger builder (F-wave, in design.rs or icon_btn.rs) with the red taken from a token; both copies retire onto it. |
| `danger_btn` | src/rune_sec/inspector_segment.rs:635 | `fn danger_btn(key: &str, icon: &str, label: &str, f: impl Fn(&mut State) + 'static) -> El` | Identical to the figure copy plus an optional 12px white glyph before the label. Proves the two copies already drifted (one grew an icon). | Same G2 builder, icon parameter kept. |
| `row_toggle` | src/rune_sec/figures_panel.rs:380 | `fn row_toggle(key: String, label: &str, active: bool, t: &Theme, f: impl Fn(&mut State) + 'static) -> El` | `btn` recipe at *different metrics*: px6/py3, radius 5 — the only button that breaks charter §4 metrics, deliberately ("five fit a sidebar row"). Used for lock/solo/eye mini-toggles in figure rows. | Where the control is a true on/off: kit `toggle` (key stays on the pressable root). Where space forbids the pill: `icon_only_btn` at the charter square size. The px6/py3/r5 metrics do not survive. |
| `picker_cell` | src/rune_sec/easing_panel.rs:210 | `fn picker_cell(e: Easing, samples: &[f32], active: bool, t: &Theme) -> El` | A button rebuilt from `el()`: px4/py5, r6, `btn` fills, canvas+label column inside. Content is panel-specific (curve preview), chrome is not. | Keep the content; rebuild the shell on `btn`-family metrics via a keyed `el()` only if F-wave's chip shell (G1) doesn't cover it. At minimum align px to 10/8 and active frame to `selected_fill()` conventions. |
| `preset_chip` (graph) | src/rune_sec/graph_panel.rs:1444 | `fn preset_chip(preset: TangentPreset, label: &str, key: &str, t: &Theme) -> El` | Card-like button: padding 4, gap 2, r6, `card` fill, canvas over label, no active state (presets are actions). | G1 chip shell or `btn`-metrics `el()`; keep canvas+label content. |
| `cap_chip` | src/rune_sec/inspector_style.rs:645 | `fn cap_chip(cap: CapKind, current: bool, t: &Theme) -> El` | Segmented-control chip: px10/py5 r6, accent when current, **no `press` fill** (hover only) — the one chip that forgot a state. | Segmented row of `btn(..., current, ...)`; gains the missing press state for free. |
| `preset_chip` (style) | src/rune_sec/inspector_style.rs:668 | `fn preset_chip(index: usize, preset: StylePreset, t: &Theme) -> El` | px8/py5 r6 `card` fill, 12² swatch + name inside; no press fill, no active state. | G1 chip shell / `icon_btn`-shaped row with the swatch as leading element; align px to 10. |
| `hinted_item` | src/rune_sec/menus.rs:224 | `fn hinted_item(key: &str, label: &str, hint: &str, active: bool, t: &Theme, f: impl Fn(&mut State) + 'static) -> El` | Menu item rebuilt as a row at `btn` metrics (px10/py5, r6, gap 12) with the hint as bare LABEL text (white 75% on active). Charter §9 wants item rows 28 high, px8, r6, hover `t.hover`, hints as `kbd` chips. | Rebuild to charter §9 item metrics; hint child → `kbd(hint, t)`. Handler/`closing()` wrapper unchanged. |
| `item_icon` | src/rune_sec/menus.rs:338 | `fn item_icon(key: &str, icon: &str, label: &str, active: bool, hint: Option<&'static str>, t: &Theme, f: impl Fn(&mut State) + 'static) -> El` | `hinted_item` + 12px leading glyph, gap 12/6. Same §9 drift, same bare-text hint. | Same §9 rebuild; glyph via `icon_el` (already), hint → `kbd`. |
| `item` | src/rune_sec/menus.rs:385 | `fn item(key: &str, label: &str, active: bool, t: &Theme, f: impl Fn(&mut State) + 'static) -> El` | Dispatcher over the two rows above + `btn`. Not duplication itself — it *routes* duplication. | Keep dispatcher; its three branches land on the §9 item row + `btn` + `kbd`. |
| `language_button` / `workspace_button` | src/rune_sec/settings_popover.rs:563 / :574 | `fn language_button(entry: LangEntry, current: Lang, t: &Theme) -> El` / `fn workspace_button(entry: WorkspaceEntry, current: Workspace, t: &Theme) -> El` | Thin adapters that already call `btn` with `active = (entry == current)`. No visual duplication. | KEEP as data adapters (or inline); no change beyond what `btn` itself gains in F. |
| `entry_button` | src/rune_sec/figure_add.rs:160 | `fn entry_button(entry: Entry, t: &Theme) -> El` | Adapter over `btn` (dismiss-on-pick). | KEEP adapter. |

---

## Table B — Section headings & panel headers

The kit replacement for every row is
`design::section_label(label, t)` (SECTION font,
`text_secondary`) for in-panel sections, and
`panel_title(icon, title, meta, t)` inside
`header_row(key)` + `divider(t)` for panel headers.
None of the existing helpers takes an icon+meta —
that structure is net-new from the kit.

| Helper | File:line | Signature | How it differs |
|---|---|---|---|
| `section` | src/rune_sec/easing_panel.rs:267 | `fn section(label: String, t: &Theme) -> El` | Bare text, LABEL, secondary. Owned `String` param. |
| `section` | src/rune_sec/playback_controls.rs:448 | `fn section(label: String, t: &Theme) -> El` | Identical copy of easing's (its doc even says "same helper shape as the other panels"). |
| `section` | src/rune_sec/fx_browser.rs:246 | `fn section(label: &str, t: &Theme) -> El` | Same look, `&str` param — three copies, two signatures. |
| `section` | src/rune_sec/export_panel.rs:855 | `fn section(title: &str, t: &Theme) -> El` | **Different look**: a `col()` (gap 6) whose heading is SECTION font — the only LABEL-free copy, and it returns a container, not a label, so callers append rows into it. Adoption must split: `section_label` + caller's own `col`. |
| `section_with_icon` | src/rune_sec/export_panel.rs:868 | `fn section_with_icon(icon: &str, title: &str, t: &Theme) -> El` | SECTION + 12px glyph in a gap-6 row, inside the same col-returning shape. Closest existing thing to `panel_title`, but glyph is 12 (kit: 13), no meta slot, no divider. |
| `section_with_icon` | src/rune_sec/fx_browser.rs:276 | `fn section_with_icon(icon: Option<&str>, label: &str, t: &Theme) -> El` | Same name, different signature (`Option` icon, falls back to `section`) and LABEL font. Two files, one name, two contracts — the census's clearest drift specimen. |
| `section_label` | src/rune_sec/figure_add.rs:154 | `fn section_label(t: &Theme, label: &str) -> El` | Bare LABEL/secondary text; **argument order flipped** (`t` first) vs the other four `section_label`s' name-mates. |
| `section_label` | src/rune_sec/home_panel.rs:946 | `fn section_label(t: &Theme, label: &str) -> El` | Identical copy of figure_add's. |
| `section_label` | src/rune_sec/settings_popover.rs:551 | `fn section_label(t: &Theme, label: &str) -> El` | Identical copy again (third). |
| `section_label` | src/rune_sec/three_d_panel.rs:688 | `fn section_label(text_: &str, t: &Theme) -> El` | Same look, opposite argument order to the three above (`label` first, param named `text_` to dodge the `text` builder). |
| `section_heading` | src/rune_sec/command_palette.rs:1513 | `fn section_heading(label: &str, t: &Theme) -> El` | A 22px-high row, px10, LABEL/secondary — the only heading with box metrics (it lives in a menu-like list). Becomes `section_label` content inside a §9-metric row, or stays a palette-specific row shell around kit label. |
| `group_label` | src/rune_sec/library_panel.rs:747 | `fn group_label(t: &Theme, label: &str) -> El` | Bare LABEL/secondary text; sixth name for the same idea (`group_` vs `section_`). |
| `help_section` header (inline) | src/rune_sec/help_panel.rs:455 | inside `fn help_section(name: &str, description: &str, t: &Theme) -> El` | Header is TITLE font + 12px glyph — a *panel-title* look, not a section label; its sections are top-level. Map header → `panel_title`, keep rows below. |
| Inline SECTION texts (no helper) | clips_panel.rs (1 site), onion_controls.rs (1), stats_view.rs (6) | `text(...).font(type_::SECTION).color(t.text_secondary)` at call sites | Already the charter font/colour, but hand-placed per site with no shared row/divider structure. Replace the text with `section_label` at each site (stats' six captions: Project stats / totals / Media / Flags / Figures + cell captions). |
| Inline LABEL headings (no helper) | camera_panel.rs (`key_list` "Keys", `follow_block` "Follow"), fx_clips.rs `fx_section`, graph_panel.rs, audio_panel.rs, draw_tools.rs, particles_panel.rs, project_tools.rs, scene_panel.rs, stroke_inspector.rs, text_tool.rs, waveform headers | bare `text(...).font(type_::LABEL).color(t.text_secondary)` | The majority pattern: headings that never even got a helper. All → `section_label`. Per-file sites are in the checklist. |

---

## Table C — Labels & readouts

No kit readout primitive exists; the kit parts are
`section_label` (the label side) and
`stepper_value` (a centred MONO_CAPTION value,
min-width 34). Rows below are rebuilt from those
parts; where the value is not a stepper value, it
stays plain text beside a `section_label`.

| Helper | File:line | Signature | How it differs | Replacement |
|---|---|---|---|---|
| `readout_row` | src/rune_sec/inspector_figure.rs:644 | `fn readout_row(label: &str, value: String, t: &Theme) -> El` | Row w_full, gap 8: LABEL secondary label grown, LABEL value in `text`. | Label → `section_label`; value stays text (or `stepper_value` where numeric). One shared row shell is a G3 candidate. |
| `readout_row` | src/rune_sec/inspector_segment.rs:588 | `fn readout_row(label: &str, value: String, t: &Theme) -> El` | **Byte-identical copy** of the figure one. | Same; the two copies collapse to one build. |
| `readout` | src/rune_sec/settings_popover.rs:556 | `fn readout(t: &Theme, value: String) -> El` | A bare value text (LABEL, `text`) meant to sit *between* stepper buttons — a stepper value that isn't centred, isn't mono, has no min-width. | `stepper_value(&value, t)` — this is the exact control the kit function describes. |
| `zoom_readout` | src/rune_sec/camera_panel.rs:750 | `fn zoom_readout(core: &EditorCore, t: &Theme) -> El` | Keyed chip: `el()` px6, centred LABEL text; no fill/radius — a readout pretending to be a chip. | Value → `stepper_value`; keep key `cam-zoom-label` on the wrapper. |
| `angle_readout` | src/rune_sec/camera_panel.rs:759 | `fn angle_readout(core: &EditorCore, t: &Theme) -> El` | Identical copy for the angle. | Same. |
| `segment` | src/rune_sec/status_strip.rs:781 | `fn segment(label: String, t: &Theme) -> El` | LABEL text floored at `est_text_width` (anti-shrink floor) — strip-specific, charter §12 keeps the floor but wants hairline ticks between segments and MONO_CAPTION for numeric segments. | Keep builder; switch numeric segments' font to MONO_CAPTION and separators to kit-aligned tick (see Table F). |
| `segment_with_icon` | src/rune_sec/status_strip.rs:806 | `fn segment_with_icon(label: String, icon: &str, t: &Theme) -> El` | `segment` + 11px glyph, gap 4, same floor (glyph width absorbed by the floor's conservatism — documented). | Keep; it is the strip's `panel_title`-in-miniature and has no kit peer. |
| `stat_cell_el` | src/rune_sec/stats_view.rs:359 | `fn stat_cell_el(cell: &StatCell, t: &Theme) -> El` | Already a card: `card` fill, r8, padding 10, TITLE value over SECTION caption. Radius is 8, charter card radius is 12. | Keep structure; radius → `PANEL_RADIUS` at adoption; caption → `section_label` styling is already equivalent. |
| `total_row_el` | src/rune_sec/stats_view.rs:376 | `fn total_row_el(label: &str, count: usize, t: &Theme) -> El` | Label + count, gap 8, **no fonts set at all** — inherits defaults, unlike every sibling row. | Fonts aligned at adoption (label secondary, count `text`, mono for the number per §12 spirit). |
| `media_row_el` | src/rune_sec/stats_view.rs:402 | `fn media_row_el(label: &str, value: &str, t: &Theme) -> El` | Both sides secondary, no fonts set. | Same alignment. |
| `flag_row_el` | src/rune_sec/stats_view.rs:429 | `fn flag_row_el(index: usize, label: &str, value: &str, t: &Theme) -> El` | `media_row_el` copy with an index key. Three row shapes for one idea. | The three stats rows collapse to one local row build at adoption (G3). |
| `clip_row_label` | src/rune_sec/audio_panel.rs:354 | `pub fn clip_row_label(clip: &AudioClip) -> String` | String builder, not an element — listed so adoption does not mistake it for a row builder. The actual clip *rows* are inline in `audio_panel` (see checklist). | No change. |

---

## Table D — Steppers

Kit mapping for every row: label part →
`section_label`; − / + → `btn` (or `icon_only_btn`
chevrons where the file already uses them); value →
`stepper_value` (centred, MONO_CAPTION, min-w 34).
Today **no** stepper value in the app is mono or
min-width-floored, and value placement splits 3 ways.

| Helper | File:line | Signature | Layout today | Differs how |
|---|---|---|---|---|
| `stepper_row` | src/rune_sec/inspector_segment.rs:604 | `fn stepper_row(label: &str, value: String, dn_key: &str, up_key: &str, t: &Theme, dn: impl Fn(&mut State) + 'static, up: impl Fn(&mut State) + 'static) -> El` | label(grow) · value · − · + | Value *before* the buttons, LABEL font, right-aligned by the grow. |
| `stepper_row` | src/rune_sec/scene_panel.rs:680 | `fn stepper_row(label: &str, value: String, dn_key: &'static str, up_key: &'static str, t: &Theme, dn: impl Fn(&mut State) + 'static, up: impl Fn(&mut State) + 'static) -> El` | label · − · value · + | Same name, same idea, **value between the buttons** and no grow — the two `stepper_row`s disagree with each other. Keys are `&'static str` here, `&str` there. |
| `opacity_stepper_row` | src/rune_sec/inspector_figure.rs:661 | `fn opacity_stepper_row(st: &InspectorFigureState, t: &Theme) -> El` | label(grow) · % value · − · + | Segment layout with hard-coded "Opacity" + keys. |
| `ring_stepper_row` | src/rune_sec/inspector_figure.rs:766 | `fn ring_stepper_row(core: &EditorCore, t: &Theme) -> El` | label(grow) · − · + (value inside the label text) | The value is *in the label string* ("Head ring 1.5×") — no separate value element to make mono without restructuring. |
| `scale_stepper_row` | src/rune_sec/inspector_figure.rs:726 | `fn scale_stepper_row(t: &Theme) -> El` | label(grow) · `icon_btn` 90% · `icon_btn` 110% | Not a −/+ stepper at all: two labelled actions. Keep as action row; only panel adoption applies. |
| `bg_channel_row` | src/rune_sec/scene_panel.rs:704 | `fn bg_channel_row(channel: usize, label: &str, value: u8, t: &Theme) -> El` | letter · value(grow) · − · + | Segment layout; label in `text` (not secondary). |
| `fps_row` | src/rune_sec/scene_panel.rs:730 | `fn fps_row(core: &EditorCore, t: &Theme) -> El` | label · value(grow) · − · + | Copy of `bg_channel_row`'s shape with its own keys. |
| `stage_axis_row` | src/rune_sec/scene_panel.rs:758 | `fn stage_axis_row(axis: usize, label: &str, value: f32, t: &Theme) -> El` | label · value(grow) · − · + | Third copy of the same shape in one file. |
| `stage_axis_row` | src/rune_sec/settings_popover.rs:586 | `fn stage_axis_row(s: &State, t: &Theme, axis: usize, heading: &str) -> El` | heading · − · **readout** · + | Same name as scene's, different signature *and* the sandwich layout; its value is the `readout` of Table C. The two files will collide in any shared-stepper future — kit adoption removes both. |
| `channel_row` | src/rune_sec/inspector_style.rs:566 | `fn channel_row(channel: usize, label: &str, value: u8, t: &Theme) -> El` | label · 12² channel swatch · value(grow) · − · + | Stepper + mini-swatch hybrid; keep the swatch, kit the rest. |
| `gradient_channel_row` | src/rune_sec/inspector_style.rs:606 | `fn gradient_channel_row(channel: usize, label: &str, value: u8, t: &Theme) -> El` | Identical to `channel_row` with a `g` key infix | A copy made for a key prefix. One builder with a stem parameter at adoption. |
| `param_row` | src/rune_sec/stroke_inspector.rs:1296 | `fn param_row(label: &str, value: String, dec_key: &'static str, inc_key: &'static str, param: Param, t: &Theme) -> El` | label · value(grow) · − · + | Segment layout; label in `text`, value secondary (the only row that swaps the two colours). |
| Inline steppers (no helper) | onion_controls.rs (Prev/Next depth rows), draw_tools.rs (stamp sides/size/rotation rows), frame_ops.rs (duration + insert-count rows), stroke_inspector.rs (width + opacity rows), fx_clips.rs `nudge_row` (icon-only nudge pad), three_d_panel.rs (zoom steppers), particles_panel.rs (nudge pad in `detail_block`) | `row(btn(−), text, btn(+))` variants | Every conceivable ordering occurs across these sites. | All rebuilt on the one kit pattern at adoption; sites enumerated per file in the checklist. |

---

## Table E — Sliders, toggles, list rows

### Sliders → `design::slider_track(frac, t)` (visual only)

| Helper | File:line | Signature | Differs how |
|---|---|---|---|
| `weather_slider` | src/rune_sec/scene_panel.rs:795 | `fn weather_slider(key: &'static str, frac: f32, fill_from: f32, centre_tick: bool, t: &Theme, handler: impl Fn(&mut State, &mut Drag, &mut Tx) + 'static) -> El` | Hand-drawn: 200×22 box, 4px track in `control_off`, accent fill from an arbitrary origin (`fill_from` — bipolar wind), optional centre tick, 16px knob in `t.text`. Kit `slider_track` is unipolar (fill from 0) and its knob is `t.knob`, not `t.text`. The bipolar/centre-tick variant is a real difference: adoption = `slider_track` for Amount; Wind needs either a kit extension (fill-from-centre) or keeps its fill drawing over a kit track. **Flagged for F-wave ruling.** |
| `opacity_slider` | src/rune_sec/layers_panel.rs:363 | `fn opacity_slider(frac: f32, t: &Theme) -> El` | Same drawing recipe as `weather_slider` (same constants via its own copies: track `control_off`, accent fill, knob `t.text`), drag handler attached inside (`opacity_drag`), key `layers-opacity-slider` on the root. | Visual → `slider_track(frac, t)` inside the existing keyed, drag-carrying root; `opacity_drag` stays. |
| `angle_dial` (+ `dial_shape`) | src/rune_sec/camera_panel.rs:734 (shape :701) | `fn angle_dial(core: &EditorCore, t: &Theme) -> El` | A canvas drag *dial*, not a track slider: card fill, r6, full width. | Keep custom (it is a different control); only its frame aligns to panel card metrics. Not a `slider_track` candidate. |
| Graph canvas / timeline drag surfaces | graph_panel.rs, timeline_marks.rs | — | Drag canvases, not sliders. | No action. |

### Toggles → `design::toggle(key, on, t, f)` / `toggle_row(key, label, on, t, f)`

**No toggle helper exists anywhere in the shell.**
Every on/off control is an active-state button.
Adoption is per call site (keys stay on the new
pressable root, per charter §7):

| Call-site group | File | Current form |
|---|---|---|
| `onion-toggle` | onion_controls.rs | `btn` with `active = core.onion` |
| `settings-guides`, `-frustum`, `-safe` | settings_popover.rs | `icon_btn` active-state (+ children rows) |
| `settings-loop` | settings_popover.rs | `icon_btn` active-state |
| `pb-loop` | playback_controls.rs | `btn` active-state |
| `cam-follow` | camera_panel.rs `follow_block` | `btn` active-state |
| `fx-hide` | fx_clips.rs | `icon_btn` eye/eye-slash swap — a *glyph-swap* toggle, the one site where the pill loses information (hidden state is named by the icon); F-wave ruling: keep icon toggle or pair pill + label via `toggle_row` |
| Figure-row lock/solo/eye | figures_panel.rs `figure_row` | `row_toggle` (Table A) |
| Layer-row visibility/solo | layers_panel.rs `layer_row` + footer | `btn`/`row_toggle`-style actives |
| Guides / frustum / safe (menu items) | menus.rs | active menu items — menus keep active items, not pills (charter §9) |
| `view-guides` etc. (View menu) | menus.rs | same — no change beyond §9 metrics |
| Stroke/particle selection toggles | stroke_inspector.rs, particles_panel.rs | active `btn`s in selection rows |

### List rows → `design::list_row(key, selected, t)` shell

Charter §5: min-h 28, px8, r8, hover `t.hover`;
selected = `selected_fill()` (accent 16%) + accent
border 35% where bordered. **No existing row uses
`selected_fill()` or `t.hover`** — selected today
means `control_track` fill, `card` fill, or accent
full-strength, depending on the file.

| Helper | File:line | Signature | Selected treatment today |
|---|---|---|---|
| `clip_row` | src/rune_sec/clips_panel.rs:651 | `fn clip_row(s: &State, t: &Theme, index: usize, selected: bool) -> El` | px8/py4, r6, selected = `control_track` fill; height under 28 with its buttons inside. Row shell → `list_row`; the inner Dup/Rename/Insert/Del buttons stay family buttons. |
| `figure_row` | src/rune_sec/figures_panel.rs:498 | `fn figure_row(r: &FigureRow, core: &EditorCore, t: &Theme) -> El` | Two-line card-ish row with thumbnail canvas, built inline (no shell helper); selection shown on the name/dot, not a row fill. Adopt `list_row` as the outer shell; thumbnail + `row_toggle` cluster migrate per Tables A/E. |
| `layer_row` | src/rune_sec/layers_panel.rs:466 | `fn layer_row(r: &LayerRow, t: &Theme) -> El` | gap-6 row, no fill/hover/radius at all; selection lives on the name `btn`. Shell → `list_row`. |
| `entry_row` | src/rune_sec/library_panel.rs:785 | `fn entry_row(entry: &LibraryRow, s: &State, t: &Theme) -> El` | Row with name + insert/rename/delete family buttons; no selected state (hover-less). Shell → `list_row(selected=false)`. |
| `group` | src/rune_sec/library_panel.rs:838 | `fn group(key: &str, label: &str, rows: &[LibraryRow], s: &State, t: &Theme) -> El` | Container: `group_label` + rows. Label → `section_label`; rows → `entry_row` above. |
| `project_row` / `project_row_confirm` | src/rune_sec/home_panel.rs:987 / :1006 | `fn project_row(info: &ProjectInfo, i: usize, t: &Theme, meta: Option<(usize, usize, u64, SystemTime)>) -> El` (+ `confirming: bool` twin) | One builder with a confirm-state twin (the confirm row reuses the same chrome so the list doesn't jump). Shell → `list_row`; the confirm variant keeps its two danger/keep buttons (danger per G2). |
| Audio clip rows (inline) | src/rune_sec/audio_panel.rs:717 (in `audio_panel`) | `row()` built per clip inside the panel root | No helper at all: name label + per-clip buttons assembled inline. Extract onto `list_row` shell at adoption (selected = the panel's `selected` clip). |
| `toast_row` | src/rune_sec/toasts.rs:368 | `fn toast_row(toast: &Toast, index: usize, t: &Theme, now: f32, action: bool, undo_label: &str) -> El` | Already a charter-style card: padding 12, r8, `card` fill + `card_border`, severity glyph, fade. Radius 8 vs card 12 is the only drift. | F-wave (toasts are foundation): radius → 12, rest already conforms. |
| Rename-editor rows ×3 | clips_panel.rs:759 `rename_row`, library_panel.rs:756 `rename_editor_row`, pose_tools.rs:1114 `rename_editor_row` | all `fn(…, t: &Theme) -> El` | Three copies of "text field + Apply/Cancel family buttons under the renamed row", two sharing a name. Not a kit primitive; adoption just re-skins the buttons inside and the row gap (4 vs 8 drift). | Note for G3. |
| Format rows ×3 | export_panel.rs:889 `plain_row`, :921 `range_row`, :942 `bundle_row` | `fn(key, icon, label, note, t, …) -> El` | Button + muted note rows, gap 8; `plain_row` is the only one with an icon-optional branch. | One local row build at adoption; button → family (already), note stays LABEL/secondary. G3. |
| `preset_row` + `preview` | easing_panel.rs:189 / :259 | `fn preset_row(preview: El, button: El) -> El`, `fn preview(samples: &[f32], color: Color, t: &Theme) -> El` | Preview chip: canvas on `card` fill, r4. Radius is the only drift (chips elsewhere are r6). | Align radius; content stays. |

---

## Table F — Swatches, chips, dividers, kbd

### Swatches (kit gap G1 — normalise marker + metrics)

| Helper | File:line | Signature | Size / current marker |
|---|---|---|---|
| `swatch` | src/rune_sec/inspector_segment.rs:658 | `fn swatch(rgb: u32, selected: bool) -> El` | 22², r6; current = **white 2px border**; press → `set_node_color`. |
| `swatch` | src/rune_sec/inspector_figure.rs:827 | `fn swatch(rgb: u32, selected: bool) -> El` | 22², r6; current = **white 2px border**; press → `set_figure_color`. Identical builder, different key prefix (`seg-color-` / `fig-color-`) and different target op — one builder with key+handler params replaces both. |
| `swatch` | src/rune_sec/fx_clips.rs:990 | `fn swatch(key: String, hex: u32, current: bool, f: impl Fn(&mut State) + 'static) -> El` | 18², r5; current = **white outline ring** (`outline(2,1)`); handler passed in — the most general of the three, and the smallest. |
| `swatch_rows` | src/rune_sec/fx_clips.rs:1010 | `fn swatch_rows(clip_index: usize, key_stem: &str, current: Option<u32>, mut make: impl FnMut(u32) -> Box<dyn Fn(&mut State)>) -> El` | Grid wrapper (chunks of 8) over the fx `swatch`. Keeps its shape; inner swatch normalises. |
| `gradient_swatch` | src/rune_sec/inspector_style.rs:698 | `fn gradient_swatch(state: usize, current: bool, t: &Theme) -> El` | 18² chip in a padded (2px) r6 cell; current = **accent fill behind the chip**. |
| `recent_chip` | src/rune_sec/inspector_style.rs:739 | `fn recent_chip(index: usize, color: u32, current: bool, t: &Theme) -> El` | Built "exactly like a gradient swatch" (its own doc) — a deliberate copy with a different press op. The two collapse to one builder at adoption. |
| Colour cells (inline) | text_tool.rs `text_color_row` (:987) and particles_panel.rs `detail_block` (:517) | 26² slot (r8), accent or `t.bg` fill, 22² chip (r6) centred | **The same construction in two files** — and a fourth current-marker (accent slot fill). This pair is the strongest candidate for the G1 kit swatch: 26/22 metrics already agree across both. |
| Channel mini-swatches (inline) | inspector_style.rs `channel_row` / `gradient_channel_row` | 12², r3, non-pressable | Display-only; no adoption beyond staying aligned with the rows of Table D. |
| Background swatches (inline) | scene_panel.rs (16-swatch grid in `scene_panel`) | pressable colour cells built inline | Same G1 normalisation; no helper exists to retire. |
| Onion legend dot (inline) | onion_controls.rs | 10², r5 single dot | Display-only. No action. |

### Chips

| Helper / site | File:line | Differs how | Replacement |
|---|---|---|---|
| `cap_chip`, `preset_chip` (style), `preset_chip` (graph), `picker_cell` | Table A | Four chip shells, four metric sets | G1 segmented/chip shell or `btn` |
| Timeline chips (inline) | timeline_marks.rs `timeline_marks` (:798) | CHIP_W×CHIP_H, r6, accent current / `card` idle / loop mix fill; range outline | Charter §13: keys + geometry constants **do not change**; only the surrounding stack gains the card. No helper swap. |
| Workspace chips | workspaces.rs `workspace_row` (:447) | Already plain `btn` call sites (label `Name · Alt+n`) | Nothing to retire; the row under them gains the app-bar hairline (charter §15) at the mount, not here. |
| Speed chips | settings_popover.rs `speed_row` (:620), playback_controls.rs | Already `btn` call sites | Nothing to retire. |
| `stat_cell_el` | stats_view.rs:359 | See Table C | Radius alignment only. |

### Dividers → `design::divider(t)`

| Helper | File:line | Signature | Differs how |
|---|---|---|---|
| `separator` | src/rune_sec/menus.rs:603 | `fn separator(key: &str, t: &Theme) -> El` | `el()` h1, fill `t.separator`, **keyed** (keys are per-menu `…-sep-…`). Kit `divider` is unkeyed — adoption must keep the key on a wrapper or the F-wave divider takes a key; **flagged: key-preservation detail for the menus agent**. Otherwise an exact match for the kit hairline. |
| `divider` | src/rune_sec/status_strip.rs:819 | `fn divider(t: &Theme) -> El` | Vertical tick 1×12, fill `control_track` (not `separator`), `min_w` floor 1. Charter §12 wants 1×10 `separator` ticks. | Restyle in place (fill → `t.separator`, height → 10) or swap to a vertical kit tick if F-wave provides one; the floor stays. |
| (none) | every panel | — | No panel draws a header divider today. | Net-new: `divider(t)` under every `header_row` at adoption. |

### Kbd → `design::kbd(label, t)`

| Helper / site | File:line | Differs how |
|---|---|---|
| `key_cap` | src/rune_sec/help_panel.rs:427 `fn key_cap(key: &str, t: &Theme) -> El` | px6/py2, r4, fill `t.card` + 1px `control_track` border, LABEL-in-Mono-family, colour `t.text`. Kit `kbd`: px5/py1, r4, fill `control_off`, MONO_CAPTION, `text_secondary`, no border mentioned. Every metric differs except radius. → replace outright; `shortcut_row` (:442) keeps its shape around it. |
| Menu hints (inline text) | menus.rs `hinted_item` / `item_icon` | Hints are bare LABEL text (secondary / white-75%). Charter §9/§11: hints are `kbd` chips. → hint child becomes `kbd(hint, t)` (active-state colour handling is the menus agent's detail). |
| Palette footer / hints | command_palette.rs `palette_view` | Any hint text in the palette chrome follows §9: card + `kbd`. |

---

## Table G — Empty states → `design::empty_state(icon, message, t)`

No helper exists. Every site below is a bare
`text(...).font(type_::LABEL).color(t.text_secondary)`
line (or a row/col wrapping one). Adoption replaces
the line with `empty_state(icon, message, t)`
(centred 20px icon + LABEL, py12) — message strings
and any keys on the wrapper stay byte-identical.

| Site | File:line | Message / context |
|---|---|---|
| `key_list` empty branch | camera_panel.rs:811 | "No camera keys yet." (nav/clipboard rows stay) |
| `loop_mode_row` no-selection branch | clips_panel.rs:806 | "Select a clip to set its loop mode." |
| `clip_edit_section` no-selection branch | clips_panel.rs:937 | "Select a clip to edit its frames." |
| Panel hint | inspector_segment.rs:686 | "Select a segment on the stage to edit it." |
| Panel hint | inspector_style.rs:805 | "Select a segment on the stage." |
| Panel hint | inspector_figure.rs:859 | "No figures in the project." |
| `detail_block` no-selection branch | particles_panel.rs:520 | "Select a particle event to edit it." |
| Filter no-match line | figures_panel.rs:716 | `No figures match "{query}".` (dynamic message) |
| Saved-projects empty line | project_tools.rs:822 | "No saved projects in ./projects/ yet." |
| Stroke list empty line | stroke_inspector.rs:1672 | "No strokes on this frame." |
| Help section notes | help_panel.rs `help_section` (:455) via `section_note` | Explanatory note text for sections with no shortcut rows — closest existing thing to an empty state; adopt `empty_state` only if the copy fits the icon+line shape, else keep as note under the new header. |
| No-joint note row | keyframe_tools.rs `no_joint_note_row` (:1059) | A full-width truncating muted row — a *note*, not an empty state; keep, restyle to panel metrics. Listed so it is not swept up by mistake. |

---

## Table H — Panel roots & composite builders (adoption targets, not primitives)

These builders assemble panels/sections. They are
listed once here so the checklist can reference
them: at adoption each root becomes
`panel_col(key, t)` + `header_row` + `panel_title`
+ `divider`, with its existing children re-homed.
Roots: chrome_view `app_bar` (:81, + §15 dirty dot &
hairline), `transport_row` (:114); camera_panel
`panel` (:905), `overlay` (:986, → charter §14 pill);
clips_panel `clips_panel` (:952), `clip_edit_section`
(:875); draw_tools `draw_tools` (:610); easing_panel
`easing_panel` (:276); export_panel `export_panel`
(:967); figure_add `figure_add_panel` (:174);
figures_panel `figures_panel` (:634); frame_ops
`frames_section` (:554); fx_browser `fx_browser`
(:497); fx_clips `fx_section` (:1538), `fx_inspector`
(:1266), `strip_view` (:1097), `fx_lane` (:1113);
graph_panel `graph_panel` (:1468); help_panel
`help_panel` (:493); home_panel `home_panel`
(:1099), `recovery_banner` (:954 — already an
accent-alpha banner, keep shape); inspector_figure
`inspector_figure` (:849); inspector_segment
`inspector_segment` (:676); inspector_style
`style_panel` (:795); keyframe_tools
`keyframe_section` (:1097); layers_panel `panel`
(:552); library_panel `library_panel` (:852);
menus `menu_bar` (:610), `menu_dropdown` (:646 —
§9 card), `menu_scrim` (:628 — unchanged);
onion_controls `onion_controls` (:112);
particles_panel `particles_panel` (:759),
`detail_block` (:517); playback_controls
`playback_controls` (:465); pose_tools `pose_tools`
(:1210); project_tools `project_section` (:717);
scene_panel `scene_panel` (:856); settings_popover
`settings_popover` (:645 — §9 card); stats_view
`stats_panel` (:485); status_strip `status_strip`
(:837) / `status_strip_state` (:858) / `build_strip`
(:889); stroke_inspector `stroke_panel` (:1329);
text_tool `text_tool` (:1031); three_d_panel
`three_d_panel` (:804), `preview_el` (:793);
timeline_marks `timeline_marks` (:798, → §13 card at
mount); toasts `toasts_view` (:312); waveform
`waveform_el_state` (:1067 — canvas, frame only);
workspaces `workspace_row` (:447); command_palette
`palette_view` (:1528, → §9 card); audio_panel
`audio_panel` (:674).

No UI builders exist in: stage_view.rs (geometry +
`hex_color` only — its stage element is assembled in
main.rs), fig3_preview.rs, stage_camera.rs,
stage_fig3.rs, stage_fx.rs, stage_layers.rs,
stage_text.rs (Shape geometry — charter §14 touches
the stage *frame* in main.rs/stage mount, not these),
icons.rs beyond `icon_el` (kept), icon_data.rs (data),
i18n_wire.rs (strings), scenes.rs / snap_tests.rs
(test/scenario code, out of shell scope).

---

## Per-file adoption checklist

Legend: **RETIRE** = delete helper, call kit/family ·
**RESTYLE** = keep builder, align metrics · **ADOPT**
= wrap root in `panel_col`/`panel_title`/`divider` ·
**SITES** = inline call sites to convert.

- **src/chrome_view.rs** — `btn` KEEP (F-wave owns
  its metrics). `app_bar`: ADOPT §15 (TITLE name,
  accent dirty dot, hairline under bar). `transport_row`:
  family buttons already; no change. `timeline_row`:
  legacy row — confirm mount status before touching
  (timeline_marks is the live timeline).
- **src/stage_view.rs** — no helpers. No action
  (stage frame/radius is a mount-site job, §14).
- **src/rune_sec/audio_panel.rs** — ADOPT root.
  Inline clip rows (:717) → `list_row` shells
  (selected clip lit). Inline steppers/readouts →
  Table D pattern. Headings → `section_label`.
  `clip_row_label` unchanged.
- **src/rune_sec/camera_panel.rs** — ADOPT `panel`.
  `zoom_readout`/`angle_readout` → `stepper_value`
  values (keys kept). `key_list`: heading →
  `section_label`; empty line → `empty_state`;
  per-key rows stay `btn` rows. `follow_block`:
  heading → `section_label`; `cam-follow` → `toggle_row`
  candidate (label "Follow"). `angle_dial` RESTYLE
  frame only. `overlay` → §14 floating pill.
- **src/rune_sec/clips_panel.rs** — ADOPT root.
  `clip_row` → `list_row` shell. `loop_mode_row`:
  segmented `btn`s stay; no-selection branch →
  `empty_state`. `rename_row` RESTYLE (G3).
  `clip_edit_section` hint → `empty_state`.
- **src/rune_sec/command_palette.rs** — `palette_view`
  → §9 card (r12, pad 6). `section_heading` →
  `section_label` inside §9 row metrics. Result rows
  → §9 item metrics (28 high, px8, hover); any footer
  hints → `kbd`.
- **src/rune_sec/draw_tools.rs** — ADOPT root (its
  controls already use the family). Inline stamp
  steppers (sides/size/rotation) → Table D pattern
  with `stepper_value`. Headings → `section_label`.
- **src/rune_sec/easing_panel.rs** — RETIRE
  `section` → `section_label`. `picker_cell` /
  `preset_row` / `preview` RESTYLE (G1, radius).
  ADOPT root.
- **src/rune_sec/export_panel.rs** — RETIRE
  `section` (split col from label) and
  `section_with_icon` → `section_label` /
  `panel_title` parts. `plain_row`/`range_row`/
  `bundle_row` → one local row build (G3). ADOPT root.
- **src/rune_sec/fig3_preview.rs** — no UI helpers
  (Shape geometry). No action.
- **src/rune_sec/figure_add.rs** — RETIRE
  `section_label` → kit. `entry_button` KEEP. ADOPT
  root (it is a popover-style card — §9 metrics).
- **src/rune_sec/figures_panel.rs** — RETIRE
  `row_toggle` → `toggle` / `icon_only_btn` (ruling
  in Table A). `figure_row` → `list_row` shell.
  Filter no-match line → `empty_state`. ADOPT root.
- **src/rune_sec/frame_ops.rs** — RETIRE `fbtn`
  (muted-wired ruling needed, Table A). Inline
  duration/insert steppers → Table D pattern.
  ADOPT `frames_section` root. (Row already wraps —
  Phase M `flex_wrap` stays.)
- **src/rune_sec/fx_browser.rs** — RETIRE `section`
  + `section_with_icon` → `section_label` (icon
  variant → `panel_title`-style row or dropped to
  plain label; category icons are decoration the
  kit header carries at panel level only — agent
  to keep category glyphs via `icon_el` beside
  `section_label` if the look must survive).
  ADOPT root.
- **src/rune_sec/fx_clips.rs** — `swatch`/
  `swatch_rows` RESTYLE per G1 (marker →
  `selected_fill()` convention). `fx-hide` toggle
  ruling (Table E). `nudge_row` → Table D pattern
  alignment. Headings in `fx_section`/`fx_inspector`
  → `section_label`. ADOPT roots; `strip_view`
  canvas frame aligns to card.
- **src/rune_sec/graph_panel.rs** — `preset_chip`
  RESTYLE (G1). Channel rows / interp segmented
  controls: already family buttons — align only.
  Headings → `section_label`. ADOPT root.
- **src/rune_sec/help_panel.rs** — RETIRE `key_cap`
  → `kbd`. `help_section` header → `panel_title`
  parts. `shortcut_row` KEEP shape. ADOPT root.
- **src/rune_sec/home_panel.rs** — RETIRE
  `section_label` → kit. `project_row(_confirm)` →
  `list_row` shell. `recovery_banner` KEEP shape.
  ADOPT root.
- **src/rune_sec/i18n_wire.rs** — no UI. No action.
- **src/rune_sec/icon_btn.rs** — family KEEP
  (F-wave). Host the new `btn_inert` (for `kbtn`/
  `pbtn`) and the G2 danger builder if F-wave
  places them here.
- **src/rune_sec/icon_data.rs** — data. No action.
- **src/rune_sec/icons.rs** — `icon_el`/`icon_el_rgb`
  KEEP. No action.
- **src/rune_sec/inspector_figure.rs** — RETIRE
  `danger_btn` → G2 builder; `swatch` → G1 builder
  (shared with segment's). `readout_row` → Table C
  parts. `opacity_stepper_row`/`ring_stepper_row`
  → Table D pattern (`ring` value must leave the
  label string). `scale_stepper_row`/`place_row`/
  `name_field_row` KEEP content, family buttons.
  Empty line → `empty_state`. ADOPT root.
- **src/rune_sec/inspector_segment.rs** — RETIRE
  `danger_btn` → G2; `swatch` → G1 (same builder as
  figure's). `stepper_row` → Table D pattern.
  `readout_row` → Table C parts. Hint →
  `empty_state`. ADOPT root.
- **src/rune_sec/inspector_style.rs** — MERGE
  `channel_row` + `gradient_channel_row` into one
  stem-parameterised builder on the Table D
  pattern. `cap_chip` → `btn` segmented.
  `preset_chip` RESTYLE (G1). MERGE
  `gradient_swatch` + `recent_chip` into one
  builder (G1). `recents_row` KEEP shape. Hint →
  `empty_state`. ADOPT root.
- **src/rune_sec/keyframe_tools.rs** — RETIRE
  `kbtn` → `btn` / `btn_inert`. `no_joint_note_row`
  KEEP (restyle only). ADOPT `keyframe_section`.
- **src/rune_sec/layers_panel.rs** — `opacity_slider`
  visual → `slider_track` inside its keyed root.
  `layer_row` → `list_row` shell. Footer toggles →
  Table E rulings. ADOPT root.
- **src/rune_sec/library_panel.rs** — RETIRE
  `group_label` → `section_label`. `entry_row` →
  `list_row` shell. `group` KEEP as container over
  kit parts. `rename_editor_row` RESTYLE (G3).
  ADOPT root.
- **src/rune_sec/menus.rs** — `hinted_item` /
  `item_icon` → §9 item metrics + `kbd` hints;
  `item` dispatcher KEEP. `separator` → kit
  `divider` with key preserved on a wrapper
  (flagged, Table F). `menu_dropdown` → §9 card.
  `menu_bar` triggers stay `btn`.
- **src/rune_sec/onion_controls.rs** — heading →
  `section_label` (it is already SECTION — swap
  the call, not the look). `onion-toggle` →
  `toggle_row("Onion", …)` or `toggle` + label.
  Prev/Next rows → Table D pattern. Legend dot
  KEEP. ADOPT root.
- **src/rune_sec/particles_panel.rs** — Colour
  cells in `detail_block` → G1 swatch (26/22
  metrics shared with text_tool). No-selection
  branch → `empty_state`. Nudge pad → Table D
  alignment. Headings → `section_label`. ADOPT root.
- **src/rune_sec/playback_controls.rs** — RETIRE
  `section` → `section_label`. `pb-loop` → toggle
  ruling (Table E). Speed cycle stays `btn`.
  ADOPT root.
- **src/rune_sec/pose_tools.rs** — RETIRE `pbtn`
  → `btn`/`btn_inert`; DELETE `pibtn` wrapper
  (call sites direct). `rename_editor_row`
  RESTYLE (G3). Library rows → `list_row` shells
  where they are rows. ADOPT root.
- **src/rune_sec/project_tools.rs** — Empty line
  → `empty_state`. Headings → `section_label`.
  ADOPT `project_section`.
- **src/rune_sec/scene_panel.rs** — MERGE the
  three stepper copies (`stepper_row`,
  `bg_channel_row`/`fps_row`/`stage_axis_row`
  shapes) onto the Table D pattern.
  `weather_slider`: Amount → `slider_track`;
  Wind bipolar fill = F-wave ruling (Table E).
  Background swatch grid → G1 metrics. Headings
  → `section_label`. ADOPT root.
- **src/rune_sec/settings_popover.rs** — RETIRE
  `section_label` → kit; `readout` →
  `stepper_value`. `stage_axis_row` → Table D
  pattern. Guides/loop toggles → `toggle` /
  `toggle_row` (keys stay). `language_button` /
  `workspace_button` / `speed_row` KEEP (family
  adapters). Card → §9 metrics.
- **src/rune_sec/stage_camera.rs / stage_fig3.rs /
  stage_fx.rs / stage_layers.rs / stage_text.rs** —
  Shape geometry only. No action.
- **src/rune_sec/stats_view.rs** — Six SECTION
  captions → `section_label`. `stat_cell_el`
  radius → 12. `total_row_el` / `media_row_el` /
  `flag_row_el` → one font-aligned row build
  (G3). ADOPT root.
- **src/rune_sec/status_strip.rs** — `divider`
  RESTYLE to §12 tick (1×10, `separator`).
  `segment` numeric fonts → MONO_CAPTION.
  `segment_with_icon` KEEP. Strip root gains no
  card (it is the §12 strip, not a panel).
- **src/rune_sec/stroke_inspector.rs** —
  `param_row` → Table D pattern (and un-swap its
  label/value colours). Width/opacity rows →
  same pattern. Selection-row toggles → Table E.
  Empty line → `empty_state`. Headings →
  `section_label`. ADOPT root.
- **src/rune_sec/text_tool.rs** — `text_color_row`
  cells → G1 swatch (shares 26/22 metrics with
  particles). Front/back + nudge rows align to
  family/Table D. Headings → `section_label`.
  ADOPT root.
- **src/rune_sec/three_d_panel.rs** — RETIRE
  `ctl` (live → `btn`; dim → `fbtn` ruling) and
  DELETE `ictl` wrapper. RETIRE `section_label`
  → kit. Zoom steppers → Table D pattern.
  `preview_el` canvas frame → card metrics.
  ADOPT root.
- **src/rune_sec/timeline_marks.rs** — No helper
  swaps (charter §13 protects chip geometry/keys).
  The stack (ruler/chips/markers/clips lane/loop
  row) gains the §13 card at the mount (main.rs /
  this builder's root). Clips lane unchanged.
- **src/rune_sec/toasts.rs** — `toast_row` radius
  8 → 12 (F-wave). Rest already conforms.
- **src/rune_sec/waveform_view.rs** — canvas only;
  its frame/caption align at audio adoption.
  No helper action.
- **src/rune_sec/workspaces.rs** — `workspace_row`
  KEEP (family chips); hairline under the row is
  a mount/app-bar job (§15).
- **src/rune_sec/scenes.rs, snap_tests.rs, mod.rs** —
  out of scope (scenarios/tests/module index).

## Counts

- Production `-> El` builders scanned: **156**.
- Primitive helpers in the tables: **~85**.
- Button dialects to retire: **7** (+2 already-pure
  dispatch wrappers to delete).
- Heading helpers to retire: **11** (+ inline sites
  in ~14 files).
- Stepper row helpers to rebuild: **10** (+ inline
  sites in 7 files).
- Slider visuals to replace: **2** (+1 bipolar
  ruling). Toggle helpers existing: **0**;
  toggle call-site groups: **~10**.
- Swatch builders: **5** (+2 identical inline
  constructions). List-row shells: **6** (+1
  inline). Divider helpers: **2**. Kbd
  helpers: **1**. Empty-state helpers: **0**;
  empty-state sites: **11**.
- Kit gaps for F-wave: **G1** swatch/chip,
  **G2** danger button, **G3** readout/menu/format
  row shells; plus two rulings — muted-but-wired
  buttons (`fbtn`/`ctl`) and the bipolar slider.
