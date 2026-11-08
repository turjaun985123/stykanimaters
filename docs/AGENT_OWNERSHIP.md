# Agent Ownership — Wave 15 (StykAnimate-RS 2.0 Transformation)

Status: **binding coordination matrix for Wave 15**.

Companion contracts:

- `docs/ULTIMATE_UI_ARCHITECTURE.md` — region map, fixed workspace
  matrices, menu/status/timeline contracts and preservation rules.
- `docs/WORKSPACE_DESIGN_SPEC.md` — Agent 02's visual specification.
- `docs/REDESIGN_PROGRESS.md` — Agent 03's living delivery record.

One agent owns a file, or one named region of a shared file. Nobody
edits outside their ownership to “help” another agent. If the correct
change crosses a boundary, the agent records the interface request
and routes it through Agent 01.

All agents follow the repository rule: **no local Cargo build,
check, test or clippy run**. GitHub Actions CI is the verification
environment after integration. No agent commits independently;
Agent 01 coordinates the integrated commit and CI loop.

---

## 1. The 20 agents

| # | Agent | Exclusive ownership | Deliverable | Depends on |
|---:|---|---|---|---|
| 01 | **Chief architect / orchestrator** | `docs/ULTIMATE_UI_ARCHITECTURE.md`, `docs/AGENT_OWNERSHIP.md`; integration review across all files, but no unilateral source rewrite | The binding architecture and this ownership matrix; interface arbitration; final integration order; rejection of matrix violations, dead controls and unowned edits | — |
| 02 | **Principal product designer / design spec** | `docs/WORKSPACE_DESIGN_SPEC.md` only | Visual hierarchy, region proportions, menu/dropdown treatment, status-strip treatment, workspace-by-workspace layout intent, small-window behaviour and screenshot acceptance criteria | 01 |
| 03 | **Progress / delivery tracker** | `docs/REDESIGN_PROGRESS.md` only | Living checklist for all 20 agents: ownership accepted, implementation reported, integration status, CI evidence, screenshot status and unresolved blockers. It records evidence; it does not invent completion | 01, all reporting agents |
| 04 | **Design-system / kit tokens** | `src/ui/kit.rs` only | Token values and any new semantic tokens required by the Wave 15 menu/status surfaces; existing token names remain stable unless Agent 01 approves a migration; spacing/type/radius scales remain the single source | 01, 02 |
| 05 | **Layout preferences** | `src/shell.rs` only | Extend `LayoutPrefs` with `timeline_collapsed: bool` (default `false`) while preserving the existing `workspace` and `dark` fields, file location, corrupt-file fallback and `*_in(dir)` test pattern; tests for default, round-trip and corrupt data | 01 |
| 06 | **Menu + status components** | New `src/ui/menu.rs`; new `src/ui/status_strip.rs`; module declarations for those two files in `src/ui/mod.rs` | Pure GPUI builders for the app-bar menu button/dropdown row and the status strip. Components receive plain data/child elements, use kit tokens, and contain no editor knowledge or listeners. Callers use `crate::ui::menu` / `crate::ui::status_strip`; no `src/lib.rs` alias change | 02, 04 |
| 07 | **Tool-groups gating** | `src/app.rs` — the tool-group band assembly in `render()`: the contiguous construction region containing the 24 `tool_group(...)` blocks, from Transport through Strokes | Wrap/branch the existing group construction so each workspace constructs exactly the group set in the architecture §3. Group interiors, labels, button IDs and listeners are not edited | 01, 04 |
| 08 | **App bar + menus + shell state** | `src/app.rs` — `EditorView` field declarations and constructor; `apply_workspace`; workspace/menu keyboard handling in `on_key_down`; render() app-bar, workspace-row and menu-dropdown assembly | Add/maintain shell fields (including the field declarations requested by Agents 10 and 17); restore layout prefs; implement one-open-menu state, dropdown rows using existing app-bar command buttons, workspace-gated panel toggles, frame/FPS chips and theme control; Alt+1..8 remains correct | 05, 06 |
| 09 | **Middle panels gating** | `src/app.rs` — render() panel-gate predicates and centre-stage insertion points: the `let *_panel = if ...` workspace predicates for graph/FX/audio/export/clips/colour/easing/weather/Home/Help, the figures-sidebar inclusion predicate, and the inspector inclusion predicate | Apply the architecture §4 matrix at construction/insertion time without clearing flags and without editing panel-builder internals. A disallowed panel builder is not invoked merely to be discarded | 01 |
| 10 | **Timeline collapse** | `src/app.rs` — bottom timeline assembly in `render()` (dopesheet strip, ruler/scrubber, frame-chip row and timecode assembly); the timeline collapse listener in that assembly | Real collapse/expand control; collapsed state omits the timeline body and leaves the agreed slim header; expanded state preserves all existing timeline behaviour and IDs; consumes the `timeline_collapsed` field declared by Agent 08 and persisted by Agent 05 | 05, 08 |
| 11 | **Status strip wiring** | `src/app.rs` — the new status-strip assembly at the end of `render()` only | Gather project name, current/total frame, FPS, evaluated zoom, selection label and current status/toast text from live core state; render Agent 06's component as the persistent status surface; remove/avoid duplication of the same plain status text in the old timeline row without changing toast semantics | 06, 08, 10 |
| 12 | **Contextual inspector** | `src/app.rs` — the inline inspector data/panel construction region in `render()`; `src/ui/inspector.rs` | Contextual figure/node inspector content and presentation for Animate/Build. Agent 09 owns only the outer workspace inclusion predicate; Agent 12 owns the data assembly and component. Existing inspector control IDs and stepper behaviour stay intact | 04, 09 |
| 13 | **FX categories** | `src/app.rs` — `fx_panel` builder internals only; `src/ui/clip_row.rs` if its presentation must change | Organise the existing FX kinds/presets/clips into truthful visual categories inside the FX panel using existing `fx_edit` data and operations. No new effect kind, no `vfx.rs` behaviour change, and no edit to the render() gate predicate (Agent 09) | 04, 09 |
| 14 | **Audio summary** | `src/app.rs` — `audio_panel` builder internals only; `src/ui/waveform.rs` if its presentation must change | A clearer audio panel summary/header and clip/waveform presentation using existing audio data and operations. No `audio.rs`/`audio_edit.rs` behaviour change, and no edit to the render() gate predicate (Agent 09) | 04, 09 |
| 15 | **Graph interpolation** | `src/app.rs` — the inline graph-panel construction region in `render()`, excluding Agent 09's outer workspace predicate; `easing_panel` builder internals; `src/ui/easing.rs`; `src/ui/graph_axis.rs` | Improve graph/easing presentation and interpolation controls using existing `graph.rs`/`graph_panel.rs` APIs. Hermite mathematics, tangent semantics and angle wrapping are not changed. Agent 09 owns only `graph_on && workspace == Graph` gating | 04, 09 |
| 16 | **Export categories** | `src/app.rs` — `export_panel` builder internals only | Present the existing export rows in truthful format categories while preserving all 15 `xc-*` IDs, labels, notes and editor operations. No `export.rs` behaviour or format change, and no edit to the render() gate predicate (Agent 09) | 04, 09 |
| 17 | **Performance caching** | `src/app.rs` — one bounded cache micro-region: cache fields are declared by Agent 08 from Agent 17's written spec; Agent 17 owns the private cache helper methods and the Home/FX data call sites that consume them. Conditional exclusive ownership of cache-helper APIs in `src/projects.rs` and `src/fx_edit.rs` only if Agent 01 approves the seam in advance | Cache project-scan and FX-preset-name reads that Wave 14's audit found running on every render, with documented invalidation after in-app changes and tests where a seam exists. No deterministic-render changes and no panel-content redesign | 08, 09, 13 |
| 18 | **Accessibility text floor** | Final pass, text-size lines only: `src/app.rs` chrome/panel text call sites and `src/ui/*.rs` text call sites, excluding `src/ui/kit.rs` and excluding Agent 06's new files until Agent 06 hands them over | Sequential final sweep enforcing the architecture's text floor: ordinary/status/menu text at least 11 px, main controls at the established scale, no new sub-11 informational text. This agent changes no layout, colour, listener, ID or text content | 02, 04, all UI region owners |
| 19 | **QA / visual-review authority** | `docs/QA_WAVE15.md` only; all source is read-only | Independent source audit: baseline ID inventory, workspace group matrix, panel matrix, dead-control check, token-literal check, timeline/status checks and the real-desktop screenshot checklist. QA may reject work but does not fix it | All implementation agents |
| 20 | **Product docs** | `README.md`, `docs/GAP-ANALYSIS.md`, `docs/COMPARISON.md` | Post-integration Wave 15 sections using only verified counts and CI results; explicitly separate implemented/CI-verified facts from pending screenshot acceptance. Does not own the architecture, design spec or progress record | 03, 19 |

---

## 2. `src/app.rs` region split

`src/app.rs` is the only heavily shared implementation file. Regions
are identified by function and anchor, not by line number, because
parallel edits move lines.

### 2.1 Frozen or separately owned micro-regions

| Region / anchor | Owner | Rule |
|---|---|---|
| `btn()` helper | **Frozen — no Wave 15 owner** | No agent restyles or rewrites it as a side effect. If a change becomes unavoidable, Agent 01 stops the line, assigns it explicitly and QA re-audits every button. |
| `EditorView` struct fields | Agent 08 | Agents 10 and 17 request new shell/cache fields in writing. Agent 08 adds the declarations so the struct has one editor. Initialised values follow the owning agent's spec. |
| `EditorView::new` / constructor | Agent 08 | Restores `LayoutPrefs` through Agent 05's API and mirrors `dark` into the single real state, `core.light_theme`. |
| `apply_workspace` | Agent 08 | Existing workspace/mode/flag effects stay coherent with the architecture. It is not converted into a per-render flag-clearing mechanism. |
| `on_key_down` | Agent 08 | Owns workspace shortcuts and menu-related keys. Text-authoring capture keeps priority. Other agents do not add key arms here. |
| `draw_data`, `paint_stage`, canvas pointer routing and free graph helper functions | **Frozen** | Wave 15 gating does not need to alter stage rendering, pointer routing or graph helper mathematics. |

### 2.2 Panel builder internals vs render gates

Panel work is deliberately split into **what a panel contains** and
**whether `render()` constructs it**.

| Panel surface | Builder/content owner | Render-gate owner |
|---|---|---|
| Graph | Agent 15 (inline graph construction + easing support) | Agent 09 |
| FX | Agent 13 (`fx_panel` internals) | Agent 09 |
| Audio | Agent 14 (`audio_panel` internals) | Agent 09 |
| Export centre | Agent 16 (`export_panel` internals) | Agent 09 |
| Movieclips | Frozen internals (`clips_panel`) | Agent 09 |
| Colour | Frozen internals (`color_panel`) | Agent 09 |
| Easing | Agent 15 (`easing_panel` internals) | Agent 09 |
| Weather | Frozen internals (`weather_panel`) | Agent 09 |
| Inspector | Agent 12 (inline data + component) | Agent 09 (outer inclusion only) |
| Figures sidebar | Frozen row/control internals | Agent 09 (workspace inclusion only) |
| Home | Frozen internals this wave | Agent 09 (flag-only; all workspaces allowed) |
| Help | Frozen internals this wave | Agent 09 (flag-only; all workspaces allowed) |

This split means a content agent must not “fix” visibility by
changing a workspace predicate, and Agent 09 must not redesign a
panel while changing its predicate.

### 2.3 `render()` assembly ownership

| Render segment | Owner | Boundary |
|---|---|---|
| Prologue values used only by the app bar/menu | Agent 08 | Does not own the shared `core` snapshot or unrelated panel data preparation. |
| App bar | Agent 08 | Brand, menu buttons, panel toggles, chips, theme. |
| Workspace selector row | Agent 08 | Existing `("ws", i)` family and `apply_workspace` calls. |
| Menu dropdown row | Agent 08, using Agent 06 components | Rendered only when a menu is open; contains existing app-bar command buttons, not new command implementations. |
| Tool-group band | Agent 07 | Only the branching/assembly around the 24 existing group blocks. No group interior edits. |
| Figures sidebar construction | Frozen internals; Agent 09 inclusion | Existing figure rows and all `fig-*` IDs stay unchanged. |
| Stage column and panel insertion | Agent 09 | Chooses which already-built/allowed panel values enter the stage column. Does not edit canvas paint or pointer routing. |
| Inspector construction | Agent 12 | The local inspector data/component block before Agent 09's inclusion point. |
| Home/Help insertion | Agent 09 | Both are allowed in every workspace when their flags are true. |
| Bottom timeline | Agent 10 | Dopesheet/ruler/scrubber/chips/timecode assembly and collapse behaviour. |
| Status strip | Agent 11 | Final bottom row only; consumes Agent 06's status component. |
| Toast presentation | Frozen, except Agent 11's no-duplicate-status boundary | Toasts remain notifications; the plain persistent status moves to the strip. |

### 2.4 Shared-state interfaces inside `app.rs`

- **Workspace:** `EditorView.workspace: usize` remains the single
  workspace value. Agents 07 and 09 read it; only Agent 08's
  workspace application path writes it.
- **Panel flags:** Existing toggle listeners and `apply_workspace`
  retain their established writes. Agents 07 and 09 do not clear
  flags to achieve gating.
- **Menu:** Agent 08 owns the open-menu field and its writes.
  Agent 06 supplies presentation only.
- **Timeline collapse:** Agent 05 owns persistence, Agent 08 owns
  the `EditorView` field declaration, Agent 10 owns the control and
  render behaviour.
- **Status:** Agent 11 reads the existing core status/toast state.
  No agent creates a second status string or save-state model.
- **Caches:** Agent 17 writes the cache specification before
  Agent 08 declares any fields. Cache helper implementations and
  their Home/FX call sites are then Agent 17's bounded micro-region,
  applied after the relevant panel/gating work to avoid simultaneous
  edits.

---

## 3. Other shared files

### 3.1 `src/ui/kit.rs`

Agent 04 is the only editor. Every other agent consumes token names.
A missing token is an interface request to Agent 01/04, not a reason
to add a local constant or hex literal in a panel.

### 3.2 `src/ui/mod.rs`

Agent 06 may add exactly the module declarations for `menu` and
`status_strip`. No other agent edits `src/ui/mod.rs` in Wave 15.

No `src/lib.rs` change is allocated. New components are referenced
by their full paths, `crate::ui::menu` and
`crate::ui::status_strip`, unless Agent 01 records a different
integration decision before Agent 06 starts.

### 3.3 `src/shell.rs`

Agent 05 is the only editor. It owns the `LayoutPrefs` shape and its
tests. Agent 08 consumes `load_layout()` / `save_layout()` and must
not duplicate layout JSON handling in `app.rs`.

### 3.4 Component files

| File(s) | Owner |
|---|---|
| `src/ui/menu.rs`, `src/ui/status_strip.rs` | Agent 06 |
| `src/ui/inspector.rs` | Agent 12 |
| `src/ui/clip_row.rs` | Agent 13, only if FX category presentation requires it |
| `src/ui/waveform.rs` | Agent 14, only if audio summary presentation requires it |
| `src/ui/easing.rs`, `src/ui/graph_axis.rs` | Agent 15 |
| All other existing `src/ui/*.rs` files | Frozen for content/layout; Agent 18 may make the final text-size-only pass after handover |

Components remain pure builders. A component owner does not add
listeners, `EditorCore` calls or project access inside `src/ui/*`.

### 3.5 Engine, formats, MCP and tests

No Wave 15 agent owns:

- `src/model.rs`, `src/editor.rs`, `src/export.rs`, `src/svg.rs`;
- `src/pivot.rs`, `src/nodes.rs`, `src/nodes_anim.rs`;
- `src/vfx.rs`, `src/audio.rs`, `src/audio_edit.rs`,
  `src/graph.rs`, `src/graph_panel.rs`, `src/fig3d.rs`;
- `src/mcp.rs`, `src/mcp_tools.rs`, `src/mcp_io.rs`,
  `src/bin/mcp_server.rs`;
- `src/main.rs`, `Cargo.toml`, `Cargo.lock`;
- existing tests or fixtures.

The only conditional exceptions are Agent 17's pre-approved cache
helper seams in `src/projects.rs` / `src/fx_edit.rs`. Those files are
otherwise read-only, and the exception does not permit behaviour
changes to project storage or FX preset formats.

If an agent believes an engine change is required, they stop, write
the reason in `docs/REDESIGN_PROGRESS.md` through Agent 03, and wait
for Agent 01 to reassign ownership. They do not make the change
inside a UI patch.

---

## 4. Fixed interface contracts between agents

### Agent 04 → all UI agents

- Token names and scales are stable during implementation.
- Any token addition lands before dependent component work is
  integrated.
- Palette changes are values in `kit.rs`, not per-file sweeps.

### Agent 05 → Agent 08/10

- `LayoutPrefs` keeps `workspace: String` and `dark: bool`.
- `LayoutPrefs` gains `timeline_collapsed: bool` with default
  `false`.
- Missing/corrupt data returns the full default record.
- Agent 08 restores the record; Agent 10 changes the timeline value
  only through the agreed save path used by workspace changes.

### Agent 06 → Agents 08/11

- Menu and status components are pure presentation.
- Agent 06 does not decide which commands a menu contains; Agent 08
  assembles existing command buttons into the dropdown.
- Agent 06 does not read editor state for the status strip;
  Agent 11 supplies the six required values as plain data/elements.

### Agent 08 → Agents 07/09/10/11/17

- Agent 08 owns shell field declarations and initialisation.
- Other agents state required fields and default values before
  Agent 08 integrates that part of the shell.
- Agent 08 does not edit the group band, panel predicates, timeline
  body or status-strip data assembly.

### Agent 09 → content agents (12–16)

- Agent 09 supplies the workspace predicate/insertion point.
- Content agents supply a panel value or builder whose internals
  work independently of which workspace allowed it.
- A content agent must not rely on being constructed in a disallowed
  workspace for side effects.

### Agent 10 → Agent 11

- Timeline collapse changes only the timeline region.
- The status strip remains present and unchanged in both timeline
  states.
- Agent 10 does not move the persistent status message; Agent 11
  owns that migration.

### Agent 17 → Agents 09/13

- FX panel content work (Agent 13) and FX data caching (Agent 17)
  are sequential at the preset-name call site, not simultaneous.
- Agent 09's gate decides whether the FX panel is constructed;
  Agent 17's cache decides how a constructed panel obtains its
  preset names without repeated disk reads.
- Cache invalidation triggers are documented before integration.

### Agent 18 → all UI agents

- The accessibility pass runs last among UI implementation agents.
- It may change only `text_size(...)` values (and a directly
  associated test expectation if a component test pins a size).
- It must not use the pass to restyle, reorder or rewrite another
  agent's region.

### Agent 19 → Agent 01/03/20

- QA reports defects with file/anchor evidence and the contract
  clause violated.
- QA does not patch source.
- Agent 03 records QA status in the progress document; Agent 20
  does not publish completion language stronger than QA/CI support.

---

## 5. Integration order

Integration follows dependency order, not the order in which agents
happen to report.

1. **Contracts:** Agent 01 architecture/ownership; Agent 02 design
   spec.
2. **Foundations:** Agent 04 tokens; Agent 05 layout prefs;
   Agent 06 menu/status components.
3. **Shell state:** Agent 08 app bar/menus and requested field
   declarations.
4. **Gating:** Agent 07 groups; Agent 09 panel predicates/insertion.
5. **Workspace surfaces:** Agent 10 timeline; Agent 11 status strip;
   Agents 12–16 panel content.
6. **Performance:** Agent 17 cache micro-region, after Home/FX call
   sites are stable.
7. **Accessibility:** Agent 18 text-size-only final pass.
8. **Review:** Agent 19 QA audit; Agent 03 updates progress from
   evidence.
9. **Publication:** Agent 01 integrates and pushes; CI is read and
   fixed through the normal loop; Agent 20 updates product docs from
   the final verified state.

No later stage may overwrite an earlier agent's region to make its
own stage compile. Compile problems at a boundary go back to the
two owners and Agent 01.

---

## 6. Completion and reporting rules

Each agent's report must state:

- files and, for `app.rs`, the exact named region changed;
- the architecture clause implemented;
- what was verified by source reading or grep;
- what remains for CI to decide;
- any interface request or blocker, named by owning agent.

The following claims are prohibited without the specified evidence:

| Claim | Required evidence |
|---|---|
| “Button preserved” | QA's source inventory against the Wave 14 baseline |
| “Workspace is gated” | The exact group/panel set for that workspace compared with the architecture matrix |
| “Control works” | Listener traced to a real editor/view function consumed by render or an operation |
| “Faster” | A described repeated operation removed or cached, plus its invalidation rule |
| “CI green” | The GitHub Actions run ID and result |
| “Visually complete” | Real desktop screenshots reviewed against Agent 02's criteria |

Agent 03's progress document must keep implementation, integration,
CI and visual acceptance as separate states. An agent finishing
their code does not mark the wave complete.

---

## 7. Conflict protocol

1. Re-read the current region before editing; parallel work may
   have landed.
2. If an anchor no longer matches, retry against the new text. Do
   not force an old patch over another agent's change.
3. If two agents need the same line or predicate, stop that hunk and
   notify Agent 01 with both intended diffs.
4. Agent 01 assigns a single writer for the contested line and
   records the decision in `docs/REDESIGN_PROGRESS.md` through
   Agent 03.
5. Never resolve a conflict by deleting the other agent's control,
   panel, test or documentation.
6. Never run local Cargo to “see who is right.” CI verifies the
   integrated tree only.

---

## 8. Wave 15 ownership summary

The decisive split is:

- **Agent 07 removes the button wall** by gating group construction.
- **Agent 09 removes cross-workspace panels** by gating panel
  construction/insertion.
- **Agent 08 makes the shell slim and navigable** with menus and
  workspace-gated toggles.
- **Agents 10–11 give the bottom of the window a stable structure**:
  collapsible timeline above a persistent status strip.
- **Agents 12–16 improve only the workspace surfaces they own**,
  without reopening the global visibility problem.
- **Agents 17–18 address render cost and text legibility** after the
  structural work is stable.
- **Agents 19–20 and Agent 03 prove and record the result**; they do
  not decorate a claim into existence.
