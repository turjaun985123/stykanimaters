# UI Component Contracts — src/ui/ inventory and rules

Phase 1 (Wave 14 redesign) audit of the component library:
what exists, the exact shape of every builder, and the rules
new components must follow. Measured from source on
2026-11-08; the library is 39 component files + `mod.rs`
(6,047 lines) under `src/ui/`, re-exported under the
historical flat names (`crate::ui_kit`, `crate::ui_toast`, …)
by aliases in `lib.rs`.

## The standing contract (verified, not assumed)

- **No component attaches a listener.** A grep for
  `on_click` / `on_mouse` / `on_key` / `track_focus` across
  all 39 files finds only doc comments *describing* the
  contract. Callers attach `.id(...)` and `.on_click(...)`
  to the returned `Div` — the "swatch pattern". Components
  also never set element ids.
- **Pure builders over plain data.** A component takes
  numbers, strings, small structs or slices, and returns a
  `Div`, a `Vec<Div>` (caller attaches per-item ids), or —
  for the four canvas components — an `impl IntoElement`
  that only paints.
- **Composed children travel by value.** Shells that wrap
  caller-built elements (`layer_row_shell`,
  `file_row`, `panel_shell`, `dialog_card`) take `Div` /
  `Vec<Div>` parameters, because GPUI elements are owned
  builders.
- **The one style (not behaviour) exception:**
  `frame_chip` sets its own `.hover(...)` background inside
  the component. It is presentation only; clicks still
  belong to the caller.
- **The one state exception:** `toast::ToastQueue` is a
  small state machine (push/tick/cap), not a builder;
  `toast_stack(&queue)` is its builder.

## Kit tokens (src/ui/kit.rs) — current vs directive

| Token | Current | Directive starting point |
|---|---|---|
| Ink (primary text) | `C_INK 0xE8E8EC` | #F1F3F7 |
| Muted (secondary text) | `C_MUTED 0x9AA3B2` | #A1A9B7 |
| Panel background | `C_PANEL 0x22262F` | #1D222B (panel) |
| Border | `C_BORDER 0x3A4150` | #303745 |
| Accent | `C_ACCENT 0x3D5A99` | #6C8CFF |

Missing tokens with no home yet: main background (#101216),
secondary background (#171A21), elevated surface (#252B36),
secondary accent (#9B7CFF). Kit has **no typography scale
and no spacing scale** — font sizes and paddings are inline
`px(...)` literals in every component. Cell/state colours
are re-declared per component instead of living in kit:
`0x2C313A` ("cell") appears as a private const or literal in
frame_chip, timeline_ruler, toolbar (`BG_IDLE`) and the
segmented docs; `0xB23A3A` (error) in both toast (`C_ERROR`)
and badge's tone table; the in-range blue `0x2E4470` in
frame_chip. A retheme today means hunting literals, not
editing one file.

## Inventory

Conventions column key: **V** = data by value, **R** = by
reference, **D** = composed `Div`/`Vec<Div>` children by
value. "Listeners: none" applies to every row (see the
standing contract) and is not repeated per row.

| Component (file) | Public builders / helpers (signatures) | Data | Tokens vs hardcoded |
|---|---|---|---|
| badge.rs | `tone_colors(Tone) -> (u32, u32)`; `badge(&str, Tone) -> Div`; `count_badge(usize) -> Div`; `status_dot(Tone) -> Div`; `tag(&str) -> Div` | V | Kit consts for Neutral/Accent; Success/Warn/Error colours are a local tone table (semantic data, named consts in the fn) |
| banner.rs | `banner(&str, Tone) -> Div`; `banner_plain(&str) -> Div`; `tone_for_status(&str) -> Tone` (pure) | V | Delegates colours to badge's `tone_colors`/`status_dot` |
| breadcrumb.rs | `crumb_text(&[String]) -> String` (pure); `breadcrumb(&[String]) -> Div`; `breadcrumb_owned(Vec<String>) -> Div`; const `CRUMB_MAX_CHARS = 24` | R + owned variant | Kit consts |
| clip_row.rs | `span(f32, f32) -> (f32, f32)` (pure); `clip_row(&ClipRowData) -> Div`; const `MIN_SPAN_FRAC` | R | Kit consts; clip colour comes from the data |
| color.rs | `swatch(u32, bool) -> Div`; const `PICKER_PALETTE: [u32; 16]` | V | The palette is data (16 literals by design); frame colours from kit |
| color_field.rs | `rgb_parts(u32) -> (u8, u8, u8)`; `hex_string(u32) -> String`; `channel_bar(u8) -> Div`; `color_readout(u32) -> Div` | V | Channel bars use semantic red/green/blue literals; text from kit |
| dialog.rs | `scrim() -> Div`; `dialog_card(&str, Div, Vec<Div>) -> Div`; `dialog(&str, Div, Vec<Div>) -> Div`; consts `SCRIM_COLOR` (RGBA-packed), `CARD_WIDTH = 420` | V + D | Kit consts + own scrim const |
| dope.rs | `keyframe_map(&Project) -> Vec<Vec<bool>>` (pure computation); `dopesheet(&[Vec<bool>], usize, &[String]) -> Div` | R | Kit consts only |
| easing.rs | `easing_name(Easing) -> &'static str`; `curve_canvas(Easing) -> impl IntoElement` (canvas; samples the engine's own `Easing::apply`) | V | Kit consts |
| empty.rs | `is_blank(&str) -> bool`; `empty_state(&str, &str, &str) -> Div`; `empty_line(&str) -> Div`; consts `DEFAULT_GLYPH`, `DEFAULT_TITLE` | V | Kit consts |
| file_row.rs | `display_name(&str) -> String`; `meta_line(usize, usize, i32) -> String`; `file_row(&FileRowData, Vec<Div>) -> Div`; const `MAX_NAME_CHARS = 40` | R + D | Kit consts |
| frame_chip.rs | `chip_colors(ChipState) -> (u32, u32)` (pure); `frame_chip(usize, ChipState) -> Div` (sets own hover style) | V | Private `C_CELL`/`C_RANGE` consts duplicate literals also used by timeline_ruler (see token note) |
| gauge.rs | `level_colors(f32) -> u32` (pure); `gauge(f32) -> Div`; `gauge_labeled(&str, f32) -> Div` | V | Threshold colours (green/amber/red) are local semantic literals |
| graph_axis.rs | `axis_ticks(f32, f32, usize) -> Vec<(f32, String)>` (pure); `axis_label_row(f32, f32, usize) -> Div` | V | Kit consts |
| help.rs | `shortcut_rows() -> Vec<(&'static str, &'static str)>`; `group_rows() -> Vec<(&'static str, &'static str)>`; `help_panel(&[(&str, &str)], &[(&str, &str)]) -> Div` | R | Kit consts; rows delegate key chips to kbd |
| hud.rs | `chip_specs(&HudState) -> Vec<(String, bool)>` (pure); `hud_strip(&HudState) -> Div` | R | Composes kit's `info_chip`/`top_bar` |
| inspector.rs | `inspector_panel(InspectorData) -> Div` (data-only panel shell; steppers stay in app.rs) | V (owned struct) | Kit consts via a glob import (`use crate::ui_kit::*`) — the only glob in the library's non-test code |
| kbd.rs | `combo_text(&[&str]) -> String`; `kbd(&str) -> Div`; `shortcut_row(&str, &str) -> Div` | V/R | Kit consts |
| kit.rs | Tokens above; `section_label(&str) -> Div`; `tool_group(&str) -> Div` (builder to fill); `group_row() -> Div`; `vsep()` / `hsep()`; `side_panel() -> Div`; `top_bar() -> Div`; `info_chip(String, bool) -> Div`; `brand() -> Div` | V | **Is** the token source |
| layer_row.rs | `row_bg(bool, bool) -> u32` (pure, RGBA-packed for `rgba()`); `layer_row_shell(bool, bool, Div, Vec<Div>) -> Div` | V + D | Kit consts |
| legend.rs | `total_count(&[LegendItem]) -> usize`; `legend_rows(&[LegendItem]) -> Div`; `legend_panel(&str, &[LegendItem]) -> Div` | R | Kit consts; item colours from data |
| node_row.rs | `length_label(f32) -> String`; `node_row(&NodeRowData, bool) -> Div` | R | Kit consts |
| overlay.rs | `safe_frame_rect(...) -> ...` (pure geometry); `stage_dimensions_label(f32, f32) -> String`; `overlay_canvas(f32, f32, bool, bool) -> impl IntoElement` (canvas, **no handlers**; documents that it paints the default camera and does not follow pan/zoom) | V | Kit consts |
| panel.rs | `panel_header(&PanelSpec) -> Div`; `panel_shell(&PanelSpec, Div) -> Div`; const `DEFAULT_PANEL_TITLE` | R + D | Kit consts; shell metrics match the pre-component inline panels |
| progress_steps.rs | `step_state(usize, usize) -> StepState`; `step_dot_color(StepState) -> u32`; `steps_row(&[&str], usize) -> Div` | V/R | Kit consts + local step colours |
| rows.rs | `clamp_label(&str, usize) -> String` (pure, `MAX_LABEL_CHARS = 24`); `labeled_row(&str, Div) -> Div`; `value_row(&str, String) -> Div`; `section_header(&str, Vec<Div>) -> Div` | V + D | Kit consts |
| scrubber.rs | `progress_frac(f32, usize) -> f32` (pure); `scrubber(f32, usize, &[f32]) -> Div` — display only; scrubbing interaction lives in app.rs (frame chips) | V/R | Kit consts |
| segmented.rs | `segment_bg(bool) -> u32` (pure); `segmented(&[&str], usize) -> Vec<Div>` (caller attaches per-segment ids/clicks) | V/R | Returns kit colours via `segment_bg`; the inactive literal `0x2C313A` is shared folklore, not a kit token |
| slider.rs | `clamp_fraction(f32, f32, f32) -> f32` (pure); `slider_track(f32) -> Div` — a **read-only gauge** (relative-width fill), not a draggable input | V | Kit consts |
| sparkline.rs | `normalise(&[f32]) -> Vec<f32>` (pure); `sparkline_canvas(Vec<f32>, bool) -> impl IntoElement` (canvas) | V | Accent/ink from kit |
| stat_grid.rs | `format_duration(f32) -> String`; `stat_cells(&ProjectStats) -> Vec<(String, String)>` (pure); `stat_grid(&ProjectStats) -> Div` | R | Kit consts |
| stepper.rs | `clamp_step(f32, f32, f32, f32) -> f32` (pure); `stepper_row(&str, String, Div, Div) -> Div` (minus/plus are caller-built buttons) | V + D | Kit consts |
| swatch_grid.rs | `swatch_grid(&[u32], Option<usize>) -> Vec<Div>`; `row_count(usize, usize) -> usize`; `grid_rows(&[u32], Option<usize>, usize) -> Vec<Div>`; `palette_grid(Option<usize>, usize) -> Vec<Div>` | R | Delegates cells to color::swatch |
| tabs.rs | `tab_bg(bool) -> u32`; `tab_fg(bool) -> u32` (pure); `tab_strip(&[&str], usize) -> Vec<Div>` (stores owned `String`s — borrowed `&str` in a `Div` fails E0521) | V/R | Kit consts |
| timecode.rs | `format_timecode(f32) -> String`; `timecode_label(usize, usize, f32) -> String`; `timecode_chip(usize, usize, f32) -> Div` | V | Kit consts (via info_chip-style chip) |
| timeline_ruler.rs | `tick_step(usize) -> usize` (pure); `ruler(usize, usize, Option<(usize, usize)>, &[bool]) -> Div` | V/R | Kit consts + own `C_CELL`/`C_LOOP` consts duplicating frame_chip's |
| toast.rs | `ToastQueue` (state: push with dedupe + TTL restart, `tick()`, cap `MAX_TOASTS = 3`, `DEFAULT_TTL_TICKS = 120`); `toast_stack(&ToastQueue) -> Div`; own const `C_ERROR` |
...[truncated 7375 chars]