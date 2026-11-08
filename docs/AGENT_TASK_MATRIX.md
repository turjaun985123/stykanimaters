# Agent Task Matrix — Wave 14 (Redesign Program, Phases 1–2 + Shell)

Binding companion to `docs/REDESIGN_ARCHITECTURE.md`. One owner
per file (or per named region of a shared file); nobody edits
outside their ownership. All agents: **no local cargo** (repo
`AGENTS.md` — CI is the only build/test environment), **no git
commits** (integration is a single coordinated commit + CI loop),
and the preservation contract (architecture doc §3d) applies to
every row: existing button ids and listeners stay byte-identical.

## The 20 agents

| # | Agent | Owns (exclusive) | Deliverable | Depends on |
|---|---|---|---|---|
| 1 | Principal architect / orchestrator | `docs/REDESIGN_ARCHITECTURE.md`, `docs/AGENT_TASK_MATRIX.md` | Architecture map, redesign strategy, this ownership matrix; integration review of all other rows | — |
| 2 | Component contracts | `docs/UI_COMPONENT_CONTRACTS.md` | Written contracts for the kit + component library: token list, builder signatures, the caller-attaches-id/listener rule, per-component data inputs | 1 |
| 3 | GUI audit | `docs/GUI_AUDIT.md` | Critical audit of the current GUI from source: layout inventory, inconsistency list, literal-colour census, prioritised findings the restyles must answer | 1 |
| 4 | Design tokens | `src/ui/kit.rs` **only**, plus the single `btn()` helper in `src/app.rs` as a carved-out micro-region (no other agent touches `btn()`) | Directive palette landed as token values; missing tokens added (backgrounds, elevated surface, secondary accent, tone colours); `btn()` reads tokens instead of literals | 1, 2 |
| 5 | Component variants | `src/ui/panel.rs`, `rows.rs`, `toolbar.rs`, `segmented.rs`, `tabs.rs`, `slider.rs`, `badge.rs` | Token-driven variants/restyle of the structural components (panel shell, rows, segmented control, tabs, slider, badges); signatures and behaviour unchanged | 4 |
| 6 | UI literal cleanup | `src/ui/banner.rs`, `dialog.rs`, `empty.rs`, `gauge.rs`, `legend.rs`, `hud.rs`, `overlay.rs`, `sparkline.rs`, `stat_grid.rs`, `breadcrumb.rs`, `progress_steps.rs`, `clip_row.rs`, `node_row.rs`, `easing.rs`, `graph_axis.rs`, `toast.rs`, `waveform.rs` | Every hard-coded hex in these files replaced by kit tokens (local tone tables become token-driven); zero visual API changes | 4 |
| 7 | Shell + workspaces | `src/app.rs` — render() root and app-bar region only (window frame, app bar, workspace selector, panel-flag switching) | Workspace selector in the app bar; switching sets the existing `show_*` flags / core modes per architecture §3b; layout record loaded at startup and saved on arrangement change via agent 8's API | 4, 8, 17 |
| 8 | Layout persistence | `src/shell.rs` only | Layout record: serde struct + `write_layout()` / `read_layout()` + `*_in(dir)` test variants, stored with recovery/recents in `./projects/`; defaults = today's layout; corrupt/missing → defaults | 1 |
| 9 | Tool-group restyle | `src/app.rs` — the 24 `tool_group(...)` blocks inside render() only (not `btn()`, not the app bar) | Groups re-arranged/restyled on the new tokens: order, spacing, captions; every button id/listener byte-identical; `ui::help::group_rows()` updated in the same change if group order/names move (coordinate via agent 16) | 4 |
| 10 | Timeline restyle | `src/app.rs` — bottom container region (dopesheet strip, timeline row, ruler/scrubber, status/toast area); `src/ui/frame_chip.rs`, `dope.rs`, `scrubber.rs`, `timeline_ruler.rs`, `timecode.rs` | Timeline hierarchy restyled on tokens: chips, ruler, scrubber, timecode, range highlight; chip ids and shift-click behaviour unchanged | 4 |
| 11 | Canvas region | `src/app.rs` — stage column region (HUD strip call site, `framed_canvas`, hint row) and the canvas paint code (`draw_data` / `paint_stage`) | Stage framing on tokens: workspace surround vs stage contrast, framed boundary, guides/HUD restyle; camera model and pointer routing untouched | 4 |
| 12 | Editor camera ops | `src/editor.rs` only | Engine-side viewport support the canvas needs (fit-to-scene / centre / actual-size style ops over the existing camera), each a tested `EditorCore` op; no UI code | 1 |
| 13 | Sidebar + inspector restyle | `src/app.rs` — figures sidebar column and inspector column regions; `src/ui/inspector.rs`, `layer_row.rs`, `stepper.rs`, `color.rs`, `color_field.rs`, `swatch_grid.rs` | Sidebar rows and inspector restyled on tokens (layer_row shell, stepper rows, colour picker); all per-figure button ids (`fig-up`, `fig-eye`, …) and swatch ids unchanged | 4 |
| 14 | Home cards | `src/app.rs` — Home panel region (`show_home` block in render()); `src/ui/file_row.rs` | Project rows become token-styled cards via `file_row` (name + meta line + Open/Copy actions); stats area restyled; no fake data — everything shown comes from `projects::scan` / `stats` as today | 4 |
| 15 | Export centre polish | `src/app.rs` — `export_panel` builder region only (consumes agent 5's `tabs`; does not edit it) | Export centre restyled on tokens: tab strip, category sections, per-format rows and guidance lines; all 15 `xc-` row ids and their ops unchanged; no new export options (only what exporters support) | 4, 5 |
| 16 | Help + shortcuts | `src/ui/help.rs`, `src/ui/kbd.rs` | Help panel restyled on tokens; shortcut rows (19) and group rows (24) kept in lockstep with the real handler and tool groups — counts change only if agent 9 changes groups, and then both move together | 4, 9 |
| 17 | i18n workspace keys | `src/i18n.rs` only | New keys for the workspace names and shell additions the selector needs, present in **all 28 tables** (genuine translations where established, the documented placeholder policy otherwise); no existing key renamed or dropped | 1 |
| 18 | Render performance | `src/export.rs` — `render_frame_at` hot path only | Measurement-first pass: profile the rasterizer path, remove only proven waste; **zero output change** — the byte-determinism suite is the acceptance test; findings reported even where no change is justified | 1 |
| 19 | QA review | No production files. `docs/REDESIGN_QA.md` | Independent review of rows 2–18: preservation-contract check (button-id inventory of `app.rs` before/after, diffed), token-leak grep (raw hex outside `kit.rs`), panel/flag coherence, honest open-items list (incl. screenshot verification still outstanding) | all |
| 20 | Docs | `README.md`, `docs/GAP-ANALYSIS.md`, `docs/COMPARISON.md` | Wave 14 sections written in the established style, framed as in-progress until CI is green; numbers (tests, tools, lines) only from verified sources | 19 |

## Shared-file rules

### `src/app.rs` (six owners, disjoint regions)

Regions are identified by anchor, not line number (lines shift
as agents work):

- **App bar + render() root** — agent 7.
- **`btn()` helper** — agent 4 (micro-region carve-out).
- **24 `tool_group(...)` blocks** — agent 9.
- **Bottom container** (dopesheet, timeline row, status) — agent 10.
- **Stage column + canvas paint** (`framed_canvas`, `draw_data`,
  `paint_stage`) — agent 11.
- **Sidebar + inspector columns** — agent 13.
- **Home panel block** — agent 14.
- **`export_panel` builder** — agent 15.
- **All other panel builders** (`easing_panel`, `fx_panel`,
  `color_panel`, `weather_panel`, `audio_panel`, `clips_panel`,
  graph panel block): **frozen this wave** — they restyle only
  by consuming the new token values; no agent edits them.

Rules: re-read your region before editing (parallel agents are
editing other regions of the same file); retry on anchor
mismatch, never force; if your change needs another region,
stop and route it through agent 1.

### `src/ui/kit.rs`

Exactly one owner (agent 4). Every other agent consumes tokens.
A needed new token is requested through agent 1, not added
locally — duplicate token definitions are rejected at review.

### `src/ui/*.rs`

Partitioned between agents 5, 6, 10, 13, 14, 16 in the table
above — the partition is exhaustive and non-overlapping.
`src/ui/mod.rs` changes (new modules) go through agent 1.

### Everything else

- Engine, format, export (except agent 18's fenced region),
  MCP, `src/lib.rs`, `src/main.rs`, `tests/`, `scripts/`:
  **not owned by this wave — read-only.**
- `src/editor.rs`: agent 12 only. `src/shell.rs`: agent 8
  only. `src/i18n.rs`: agent 17 only.

## Cross-cutting rules (all agents)

1. **No cargo locally, no commits.** CI verifies after
   integration (repo `AGENTS.md`).
2. **Ids and listeners are frozen** (architecture §3d). Styling
   values, arrangement and *additions* are the only allowed
   changes in UI code.
3. **Test-module trap:** in any file that imports `gpui::*`,
   test modules use exact-name imports only — never
   `use super::*;` (GPUI's `#[test]` proc-macro shadows the
   built-in and expands infinitely).
4. **No fake controls.** A button, tab or workspace that does
   nothing is a defect, not a placeholder (directive Rule 5).
5. **GPUI limits are law** (architecture §3e): no docking, no
   native dropdowns/popovers/tooltips, no touch. Proposals
   needing them are re-scoped, not faked.
6. **Report honestly:** line numbers, what was verified by
   reading/grep, and what only CI can decide. Screenshot
   verification stays an open item until a real desktop
   screenshot exists.
