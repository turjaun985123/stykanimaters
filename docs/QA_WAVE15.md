# QA — Wave 15 (STYKANIMATE 2.0 GUI transformation)

Independent review of the Wave 15 working tree. Baseline: commit
`382dbce` (Wave 14 docs, the last commit before this wave). Audits
ran 05:28–05:30 UTC on 2026-11-08 against a tree verified stable
(identical `git diff` and `git status` checksums across a 200 s
interval, re-confirmed after the audits). Static audit only — no
cargo was run (repo rule); **CI is the compile verdict, and nothing
in this wave has compiled yet.**

## Headline findings

1. **Button preservation: PASS — zero missing ids.** All 240
   baseline id names extracted from `382dbce:src/app.rs` are
   present in the working tree's `src/app.rs`, each still wired
   through `btn(...)` (which attaches its listener by
   construction) or an `.id(...)` + `on_click` div. The File/Edit
   buttons that moved into menu dropdowns kept their ids, labels
   and listener bodies (spot-checked line by line, §1).
2. **Workspace gating: PASS — exact match.** The `ws_has` table
   in `src/app.rs` reproduces the contract table for all eight
   workspaces, group for group (§3). All ten panel gates also
   match the contract exactly. No tool group is orphaned: every
   one of the 24 groups renders in at least one workspace.
3. **New controls: 8 ids, all real.** Four menu buttons, the
   timeline collapse toggle, and three graph interpolation
   buttons — every one calls a real state change or editor op
   (§2). No dead controls were found.
4. **One integration blocker to clear before push:** the new
   components `src/ui/menu.rs` and `src/ui/status_bar.rs` exist
   and export exactly what `app.rs` calls, but they are **not yet
   registered** — no `pub mod menu;` / `pub mod status_bar;` in
   `src/ui/mod.rs`, no `ui_menu` / `ui_status_bar` aliases in
   `src/lib.rs`. As audited, `crate::ui_menu::*` references in
   `app.rs` cannot resolve. This is the established
   integration-owned step from earlier waves; it must land before
   the push or CI fails at build (§5).
5. **Two minor UX inconsistencies** (not contract violations):
   the Easing toggle button is visible in every workspace while
   its panel renders only in Animate/Graph, and the View menu
   opens empty outside Animate/Build (§3.3).

## 1. Button preservation

Method (repeatable): a regex extraction over the source of every
`btn("…")`, `btn(("…", …)`, `.id("…")`, `.id(("…", …)` and
`xc_row("…")` name — run identically on `git show
382dbce:src/app.rs` and on the working tree's `src/app.rs`, then
set-compared.

| Set | Baseline (382dbce) | Working tree | Delta |
|---|---|---|---|
| Plain ids | 196 | 204 | +8 new (§2) |
| Tuple ids | 29 | 29 | 0 |
| Export-centre `xc-` rows | 15 | 15 | 0 |
| **Unique names** | **240** | **248** | **+8, −0** |

(The Wave 14 QA counted 235 with a slightly different extraction;
the number that matters is the like-for-like diff, which is
**zero missing**.)

Unlike Wave 14 (whose diff contained zero removed `btn(` lines),
this wave's diff removes 167 `btn(`/`.id(` lines and adds 176 —
expected: buttons were *moved* (into menu dropdowns, into the
gated `tool_groups` vector), which appears as remove+add pairs.
Presence and wiring were therefore verified in the new locations
directly, not inferred from the diff.

Spot-check — the moved File/Edit buttons, now in the `menu_row`
dropdown (`src/app.rs`, menu match arms):

| Menu | Button | Listener calls |
|---|---|---|
| File (0) | `save` | `view.core.save()` |
| File (0) | `load` | `view.core.load()` |
| File (0) | `export` | `view.core.export_png()` |
| Edit (1) | `undo` | `view.core.undo()` |
| Edit (1) | `redo` | `view.core.redo()` |
| Edit (1) | `copy-frame` | `view.core.copy_frame()` |
| Edit (1) | `paste-frame` | `view.core.paste_frame()` |
| View (2) | `inspector` | toggles `show_inspector` (+ panel exclusions), gated to ws 0/2 |
| Help (3) | `help` | toggles `show_help` |

Labels use the same i18n keys (`tr("save")`, `tr("undo")`, …) as
the old inline bar buttons. Menu open/close is real state
(`EditorView.open_menu: Option<usize>`): a second click on the
menu button closes it, opening another menu replaces it, and Esc
closes it (key handler). A menu closes on selection only via
these paths — the dropdown buttons do not close the menu
themselves; noted as a behaviour fact for the screenshot pass,
not a defect claim.

## 2. New ids this wave

| Id | Kind | Listener does |
|---|---|---|
| `menu-file` | div + `on_click` | toggles `open_menu = Some(0)` / `None` — opens the File dropdown |
| `menu-edit` | div + `on_click` | toggles `open_menu = Some(1)` / `None` |
| `menu-view` | div + `on_click` | toggles `open_menu = Some(2)` / `None` |
| `menu-help` | div + `on_click` | toggles `open_menu = Some(3)` / `None` |
| `tl-collapse` | `btn(...)` | toggles `view.timeline_collapsed`; persists via `shell::load_layout` / `save_layout` (reload-stamp-save, preserving fields it does not own). Collapsed = dopesheet/ruler/scrubber omitted, chips row + status remain. State restored at startup from `LayoutPrefs.timeline_collapsed` |
| `graph-interp-herm` | `btn(...)` | `core.set_graph_key_interp(Interp::Hermite)` (editor.rs:1305, unit-tested) |
| `graph-interp-lin` | `btn(...)` | `core.set_graph_key_interp(Interp::Lin
...[truncated 7375 chars]