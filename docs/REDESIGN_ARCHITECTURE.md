# Redesign Architecture — StykAnimate-RS UI/UX Overhaul

Status: Wave 14 (redesign program, Phase 1). This document is the
architecture map and the binding strategy for the redesign. The
ownership matrix that goes with it is `docs/AGENT_TASK_MATRIX.md`;
the component contracts are `docs/UI_COMPONENT_CONTRACTS.md`.

The redesign changes how the application **looks and is arranged**.
It does not change what the application **does**. Every engine,
format and behaviour listed under "Preservation contract" below
stays byte-compatible.

---

## 1. Repository architecture map

One library crate (`src/lib.rs`), two binaries built on it:

- `src/main.rs` — the GPUI editor application.
- `src/bin/mcp_server.rs` — the MCP server (JSON-RPC over stdio).

### 1.1 Document model (pure data, no UI knowledge)

| Module | Owns |
|---|---|
| `model.rs` | `Project` and everything in it: figure rigs (`FigureDef`/`NodeDef`), `Frame`/`Pose`, `Stroke`, `AudioClip`, FX clip entries, camera keys, weather, channels storage, serde for the whole document. |
| `style.rs` | Segment style helpers (colour lerp, caps, taper math) shared by renderers. |

### 1.2 Editor core (operations, no GPUI presentation)

| Module | Owns |
|---|---|
| `editor.rs` | `EditorCore`: every mutating operation on the project (posing, frames, draw recording, audio/FX ops, 3D session, library). Undo/redo as JSON project snapshots; autosave counter. |
| `graph.rs`, `graph_panel.rs` | Graph channel engine (Hermite keys, interp modes, tangent presets) and its view-model (axis fitting, overlay curves, preset previews). |
| `paint.rs`, `draw_ops.rs` | Stroke model: brushes, nib rendering geometry, shape generators, spray dots, simplification, project-level stroke ops. |
| `audio_edit.rs` | WAV read/write and all DSP (normalise, pitch, compress, echo, split, mix…). |
| `fx_edit.rs` | FX clip list operations and on-disk FX presets. |
| `vfx.rs` | Deterministic particle engine: 27 clip kinds + weather (incl. Storm), closed-form in frame time. |
| `fig3d.rs` | 3D rig engine: FK, Euler math, two-bone IK, pose library. |
| `layers.rs` | Figure layer operations (visibility, opacity, reorder). |
| `stats.rs` | Project statistics view-model (counts, root travel, busiest figure). |
| `projects.rs` | Project library on disk: scan, save, rename, duplicate, filter. |
| `text_edit.rs` | Segment text authoring session state. |
| `touch.rs` | Pointer gesture recogniser (engine only — see §5e). |

### 1.3 Formats (byte-level, fixture-verified)

| Module | Owns |
|---|---|
| `pivot.rs` | Pivot `.piv` v2–v5 import/export, `.stk`, v5 image **and** text side tables. |
| `nodes.rs`, `nodes_anim.rs` | Stick Nodes `.nodes` figures; `.nodemc`/`.stknds` animation import/export. |

### 1.4 Rendering and export (deterministic)

| Module | Owns |
|---|---|
| `export.rs` | CPU rasterizer (`render_frame_at`) and every file export (PNG/JPEG/GIF/APNG/MP4/WebM/SVG/spritesheet/contact sheet), audio mux planning. |
| `svg.rs` | SVG exporter (shares geometry sources with the rasterizer). |
| `media.rs` | Image/text segment compositing into rendered frames (3×5 bitmap font). |
| `audio.rs` | Live playback planning over `ffplay` (single-clip direct, multi-clip mix). |

### 1.5 Services

| Module | Owns |
|---|---|
| `shell.rs` | On-disk app state in `./projects/`: recovery snapshot, recents list, session-continuity queries. All functions follow a `name()` / `name_in(dir)` pattern so tests can use temp dirs. |
| `i18n.rs` | 28 language tables, one flat key set; every key present in every table (genuine translations; a few established loanwords). |
| `dialogs.rs` | Native file dialogs via zenity/kdialog with a folder-flow fallback. |

### 1.6 MCP (presentation-free)

`mcp.rs` (server + built-in tools), `mcp_tools.rs` (content
tools), `mcp_io.rs` (file/render tools) — 65 tools total. The MCP
server drives `EditorCore` directly and never touches the UI
layer. **The redesign does not touch this layer at all.**

### 1.7 UI layer (the redesign's whole working surface)

| Piece | Owns |
|---|---|
| `src/app.rs` (~4,750 lines) | `EditorView`: the GPUI window. App bar, 24 labelled `tool_group` panels, figures sidebar, framed stage canvas, modal panels under the canvas, timeline bottom bar, keyboard/mouse routing. Contains the shared `btn()` helper. |
| `src/ui/` (39 component files + `mod.rs`) | Presentation-only component library: pure builders over plain data (kit, panel, rows, slider, badge, tabs, segmented, stepper, frame_chip, dope, scrubber, timeline_ruler, timecode, waveform, color/color_field/swatch_grid, inspector, hud, overlay, toast, dialog, help/kbd, file_row, stat_grid, sparkline, gauge, legend, node_row, layer_row, clip_row, easing, graph_axis, banner, breadcrumb, empty, progress_steps, toolbar). Exposed to the crate through flat `ui_*` aliases in `lib.rs`. |
| `src/ui/kit.rs` | **The design tokens** and shared chrome builders: `C_INK`, `C_MUTED`, `C_PANEL`, `C_BORDER`, `C_ACCENT`, `section_label`, `tool_group`, `group_row`, separators, `side_panel`, `top_bar`, `info_chip`, `brand`. |

---

## 2. State ownership

Three owners, three kinds of state. The redesign must not blur
these lines.

- **`Project` (document state)** — owned by `model.rs`, mutated
  only through `EditorCore` operations. This is the only state
  that is saved to disk as a project, and the only state undo
  snapshots cover.
- **`EditorCore` (session state)** — the live editing session:
  current frame, selection (figure/node), drag state, playhead,
  loop range, `camera_mode`, `draw_mode` + active brush/tool,
  3D session, `light_theme`, `status` line, undo/redo stacks,
  autosave counters. Runtime-only by design (e.g. `loop_range`
  and `solo` are deliberately not undoable and not saved).
- **`EditorView` (view state)** — window-only state in `app.rs`:
  which panels are open (`show_home`, `show_graph`, `show_audio`,
  `show_clips`, `show_fx`, `show_easing`, `show_help`,
  `show_color`, `show_guides`, `show_weather`, `show_export`,
  `show_inspector`), `export_tab`, timeline `range_sel`, graph
  drag/selection transients, the toast queue, canvas bounds,
  caches (waveform peaks). None of this is ever written into a
  project file.

Rules that follow:

1. Engine modules (§1.1–1.5) never import GPUI presentation.
   `app.rs` and `src/ui/*` are the only GPUI-facing code.
2. Components in `src/ui/` are pure builders over plain data:
   no editor knowledge, no listeners inside — the caller attaches
   `.id(...)` / `.on_click(...)` (the established swatch pattern).
3. A redesign change that needs new *behaviour* goes through an
   `EditorCore` op, not through view state reaching into the
   project. A redesign change that is purely *presentational*
   stays in `app.rs` / `src/ui/*`.

---

## 3. Redesign strategy

### (a) Design tokens are the single styling source

All chrome styling flows from `src/ui/kit.rs` constants, the kit
builders, and the one `btn()` helper in `app.rs`. Changing a
token's **value** restyles every consumer at once — that is the
mechanism of the visual overhaul, not a sweep of per-panel edits.

Current values → directive palette:

| Token | Now | Directive target |
|---|---|---|
| (app background) | implicit | `#101216` |
| `C_PANEL` | `0x22262F` | panel `#1D222B`, secondary bg `#171A21` |
| (elevated surface) | `0x2C313A` literals | `#252B36` |
| `C_BORDER` | `0x3A4150` | `#303745` |
| `C_ACCENT` | `0x3D5A99` | primary `#6C8CFF`, secondary `#9B7CFF` |
| `C_INK` | `0xE8E8EC` | `#F1F3F7` |
| `C_MUTED` | `0x9AA3B2` | `#A1A9B7` |

Work items this implies:

- Add the missing tokens (background, secondary background,
  elevated surface, secondary accent, error/success tones where
  components already hard-code them, e.g. toast/banner) to
  `kit.rs` — **one owner for that file** (see the task matrix).
- Sweep hard-coded hex literals out of `src/ui/*.rs` and the
  `app.rs` chrome into token references (several components
  already define local tone tables — `badge.rs`,
  `frame_chip.rs`, `layer_row.rs` — those become token-driven).
- Typography/spacing stay as the kit's established scale
  (10px uppercase section labels, 13px button text, 4–8px gaps,
  6–8px radii) unless the token pass changes them in the kit
  itself. One place, one change.
- The light theme (`EditorCore::light_theme`) is a token-set
  swap, not a second stylesheet: token consumers must read
  colours from the active set. Where today's code picks colours
  inline, the token pass is what makes the theme real.

### (b) Workspaces are presets over existing panels and modes

The shell gains a **workspace selector** in the app bar. A
workspace is not a new screen and not new functionality — it is
a named, one-click arrangement of the panels and editor modes
that already exist:

| Workspace | What it activates (all pre-existing) |
|---|---|
| Animate | Default shell: stage + figures sidebar + timeline; all modal panels closed; pose mode. |
| Draw | `EditorCore::draw_mode` on (Draw tool group is the working set); stage + sidebar. |
| Build | Figures / Segments / Library & Clips tool groups foregrounded; inspector open on the selected figure/node. |
| Graph | `show_graph` panel open (graph editor under the canvas). |
| Effects | `show_fx` panel open (FX timeline + presets). |
| Audio | `show_audio` panel open (waveform + clip ops). |
| 3D | The 3D tool group's session (editor 3D mode: orbit/pose/bake). |
| Export | `show_export` panel open (the export centre, tabbed All/Media/Formats). |

Semantics:

- The panel flags already implement mutual exclusion where it
  matters (graph/audio/clips/FX/inspector close each other;
  colour/easing/weather/help are independent overlays).
  Switching a workspace sets the same flags and core modes the
  existing buttons set — identical end state, one click instead
  of several.
- The Home screen (`show_home`) stays what it is: a panel that
  replaces the canvas area, reached from the existing Home
  controls, not a ninth workspace.
- No workspace may expose a control that does not already work.
  If a directive workspace has no backing functionality, it is
  not added (per the directive's own no-fake-features rule).

### (c) Layout preferences persist via `shell.rs`

Workspace choice and panel arrangement are view state (§2), so
they persist with the app's other on-disk state — not in project
files:

- A new layout record in `shell.rs`, following its established
  pattern: a small serde struct, `write_layout()` /
  `read_layout()` plus `*_in(dir)` variants for tests, stored
  alongside recovery/recents in `./projects/`.
- Contents: active workspace, sidebar visibility, inspector
  visibility, and the independent overlay flags worth restoring
  (guides). Nothing else — document state never migrates here.
- Missing or corrupt file → built-in defaults (today's layout).
  "Restore default layout" = delete the record and apply
  defaults; it needs no special engine support.
- `EditorView` loads the record at startup and writes it when
  the arrangement changes (debounced by the same render-pass
  observation the toast feed uses, not on every frame).

### (d) Preservation contract — ids and listeners are frozen

The redesign is **styling, arrangement and addition — never
removal**:

- Every existing button id and click listener in `app.rs` stays
  **byte-identical**: same id (including tuple ids like
  `("fig-up", fi)`), same listener body, same `EditorCore` op
  behind it. Buttons may move between groups and change visual
  style; they may not disappear or change behaviour.
- All 24 tool groups survive as groups (their order and the
  shell arrangement around them may change). Note the coupling:
  `ui::help::group_rows()` mirrors the group list and a test
  pins its count — group renames/reorders must update the help
  rows in the same change.
- All existing panels, the sidebar rows and their per-figure
  buttons, the timeline chips, the export centre's 15 `xc-`
  rows, and the keyboard shortcut set (19 rows in
  `ui::help::shortcut_rows()`, defined by the single
  `on_key_down` handler) are preserved.
- Engine preservation is absolute: project formats, `.piv`
  v2–v5 / `.stk`, `.nodes` / `.nodemc` / `.stknds`, FK/IK math,
  interpolation, graph engine, camera model, drawing, VFX,
  audio, movieclips, 3D, every export format, the MCP server,
  recovery/autosave, and deterministic rendering are untouched
  by this program. The fixture corpus and the determinism
  suite are the tripwires.
- Verification of the contract: an id inventory (grep of
  `btn("` / `.id((` patterns in `app.rs`) taken before the wave
  and diffed after; CI green including the determinism and
  corpus tests.

### (e) GPUI 0.2.2 limits — stated honestly

The directive asks for a dockable, popover-rich desktop shell.
GPUI 0.2.2 (pinned, `gpui = "=0.2.2"`) does not provide several
of the primitives that vision assumes. What is and is not
implementable:

- **No native drag-docking or splitter framework.** Panels
  cannot be dragged between docks. The implementable form of
  the directive's flexible layout is: panel collapse/expand,
  visibility toggles, workspace presets (§3b) and persisted
  arrangement (§3c). Hand-rolled drag-resizing of the sidebar
  is possible mouse math, but it is not part of this wave.
- **No dropdown, popover, menu or tooltip primitives.** Every
  such element in this app is — and remains — custom `Div`s
  with manually routed clicks (the existing panels and the
  dialog scrim already work this way). New "menus" are panels,
  styled by the same kit.
- **No touch input at all** (proven from the GPUI 0.2.2 crate
  source; see `docs/TOUCH.md`). The gesture recogniser and
  pinch camera exist and are tested but unreachable from
  hardware; the redesign does not change that.
- **Styling is builder-level, not CSS.** No stylesheet cascade,
  no pseudo-classes beyond GPUI's hover/active builder states,
  per-element single border colour, no per-side border colours.
  Shadows and blur are limited; the design must read as premium
  through colour, spacing and typography, not effects.
- **Canvas elements receive no pointer events** unless handlers
  are attached — overlays (guides, HUD) are safe to layer over
  the stage for this reason, and all pointer routing stays in
  the root handlers keyed by position/bounds.
- **One window, one view.** Workspaces are arrangements of the
  single `EditorView`, not separate windows or routes.

Any redesign proposal that needs a primitive from the "no"
list is rejected at review and re-scoped to the implementable
form above.

---

## 4. Phase plan

Mapped to the directive's phases, in wave-sized slices:

- **Wave 14 (this wave) = Phases 1 + 2 + shell groundwork.**
  Audit and architecture (this document, the task matrix,
  component contracts, GUI audit); the token pass in `kit.rs`
  and literal cleanup in `src/ui/*`; component variant work on
  the existing library; the app-shell changes — workspace
  selector + switching, persisted layout via `shell.rs`;
  restyles of the tool groups, timeline, sidebar/inspector,
  canvas framing, Home cards and the export centre **using the
  new tokens only**; new i18n keys for the workspace names;
  QA review of the whole wave (id inventory, preservation
  contract) and docs.
- **Later wave = Phase 3 depth.** Canvas interaction polish
  (fit/centre/actual-size backed by editor camera ops),
  rigging-UX feedback in the canvas paint layer, timeline and
  graph depth — all on top of the shell and tokens from
  Wave 14, still styling/arrangement/addition only.
- **Later wave = Phase 4 depth.** Draw / Effects / Audio / 3D
  workspace depth: richer panels inside the workspaces Wave 14
  establishes, consuming the component library.
- **Later wave = Phase 5 depth.** Home/project-launcher depth,
  command discovery, export-centre settings depth (only
  options the exporters actually support).
- **Later wave = Phase 6 + 7.** Performance pass (render-path
  measurements first, no speculative optimisation),
  accessibility/localisation polish, full integration review.
- **Standing item — real visual verification.** CI green is
  necessary, not sufficient (directive Rule 9). The redesigned
  UI must be screenshot-reviewed from the CI artifact on a real
  desktop, as the pre-redesign editor was. Until that happens,
  every wave report lists visual verification as **open** —
  this is gap #7 in `docs/COMPARISON.md` and the redesign does
  not close it by assertion.

---

## 5. Coordination rules (summary)

Full ownership is in `docs/AGENT_TASK_MATRIX.md`. The load-
bearing rules:

1. `src/ui/kit.rs` has exactly **one owner** per wave; everyone
   else consumes tokens.
2. `src/app.rs` is edited by **disjoint regions only** (panel
   builders, tool groups, sidebar, canvas, timeline, home,
   export centre are separate regions with named anchors);
   two agents never edit the same region in one wave.
3. No agent runs cargo locally; GitHub Actions CI is the only
   build/test environment (repo `AGENTS.md`). Agents do not
   commit — integration is one coordinated commit-and-CI loop.
4. New test modules in files that import `gpui::*` use
   exact-name imports only, never `use super::*;` (the GPUI
   `#[test]` proc-macro trap).
5. Any conflict between a redesign idea and §3(d) or §3(e) is
   resolved in favour of the contract and the platform facts.
