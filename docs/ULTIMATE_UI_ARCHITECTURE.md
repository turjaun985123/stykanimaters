# Ultimate UI Architecture — StykAnimate-RS 2.0

Status: **Wave 15 binding architecture contract**.

Wave 14 established the design tokens, the eight-workspace selector,
layout persistence and a restyled shell. It did not remove the central
problem: all 24 tool groups remained instantiated in every workspace,
so changing workspace mainly changed which extra panel was open. The
result was still a button wall.

Wave 15 changes the rendering model, not just the styling:

> **`render()` constructs only the active workspace's tool groups and
> only the panels that workspace is allowed to display.**

This document fixes the region map, the workspace-to-groups mapping,
the workspace-to-panels mapping, the menu and status-strip contracts,
and the preservation rules. `docs/AGENT_OWNERSHIP.md` assigns every
part of the implementation to one owner.

The mapping in this document is fixed for Wave 15. An implementer who
believes a mapping entry is wrong must stop and raise it with the
orchestrator; they must not silently redesign the matrix while editing
a region.

---

## 1. Architectural decision

### 1.1 Gating is a render rule

Workspace gating lives in the GPUI presentation layer, principally in
`EditorView::render()` and the local panel-construction branches that
feed it.

It does **not** live in:

- the document model (`model.rs`);
- editor operations (`editor.rs`);
- format import/export;
- the deterministic renderers;
- the MCP server.

No project field records which tool groups exist. No editor operation
is removed or disabled merely because its button is in another
workspace. The same project and session remain loaded while the user
switches workspaces.

### 1.2 Gating hides; it never deletes

A hidden group or panel is simply not constructed in that render pass.
Its source code, button IDs, listeners, editor operations and panel
flag remain intact.

This distinction is mandatory:

- **Hidden:** not present in the current GPUI element tree.
- **Deleted:** source or behaviour removed — prohibited by the
  preservation contract in §8.
- **Disabled:** visible but inert — not a substitute for workspace
  gating.

When the user returns to a workspace, its groups are constructed
again from the same unchanged code paths.

### 1.3 Panel flags remain the state; workspace is the filter

The existing `EditorView` panel flags — `show_graph`, `show_fx`,
`show_audio`, `show_export`, `show_clips`, `show_color`,
`show_easing`, `show_weather`, `show_inspector`, `show_home` and
`show_help` — remain the panel state. Wave 15 does not rename them,
move them into the project, or make `apply_workspace` clear them just
to force the visual result.

For every panel, the effective render predicate is:

```text
rendered = panel_flag && workspace_allows_panel(workspace, panel)
```

For the figures sidebar, which has no dedicated `show_*` flag, the
predicate is workspace-only. For Home and Help, the workspace term is
always true.

Consequences:

1. A flag may be true in a workspace that does not allow its panel.
   The panel simply does not render there.
2. The flag is not rewritten merely because the panel is hidden.
3. Switching back to an allowed workspace makes the panel reappear
   if its flag is still true.
4. Existing panel toggles keep their current flag and mutual-
   exclusion behaviour when they are visible.
5. `apply_workspace` may continue to set flags and editor modes as
   it did in Wave 14, but those writes are not the gating mechanism.
   The render predicate is authoritative for what is on screen.

### 1.4 Build hidden surfaces lazily

A disallowed panel must not be built and then discarded. Its builder
or inline construction branch must be skipped. The same rule applies
to tool groups: the group container and its buttons are not
constructed at all in a workspace that does not include the group.

This is both a product rule — unrelated controls must not compete for
attention — and a performance rule: a render pass should not pay to
build surfaces it will not display.

---

## 2. Region map

The main window is a vertical stack with one central editing row.
The stage is the dominant surface in that row.

```text
┌──────────────────────────────────────────────────────────────────────┐
│ 1. SLIM APPLICATION / MENU BAR                                       │
│    brand · File/Edit/View/Help menus · contextual panel toggles       │
│    panel toggles · frame/FPS chips · theme                           │
├──────────────────────────────────────────────────────────────────────┤
│ 2. WORKSPACE NAVIGATION                                              │
│    Animate · Draw · Build · Graph · Effects · Audio · 3D · Export    │
├──────────────────────────────────────────────────────────────────────┤
│    Optional menu dropdown row (only while a menu is open)            │
├──────────────────────────────────────────────────────────────────────┤
│ 3. CONTEXTUAL TOOLBAR — GATED TOOL-GROUP BAND                        │
│    only the groups assigned to the active workspace                  │
├──────────────────────────────────────────────────────────────────────┤
│ 4. MAIN EDITING ENVIRONMENT                                          │
│                                                                      │
│  ┌───────────────┬──────────────────────────────┬─────────────────┐  │
│  │ Figures       │ Stage / active workspace     │ Contextual      │  │
│  │ sidebar       │ surface                      │ inspector       │  │
│  │ (Animate and  │                              │ (Animate and    │  │
│  │ Build only)   │ Workspace panels may stack   │ Build only)     │  │
│  │               │ below the stage in the       │                 │  │
│  │               │ centre column.               │                 │  │
│  └───────────────┴──────────────────────────────┴─────────────────┘  │
├──────────────────────────────────────────────────────────────────────┤
│ 5. TIMELINE                                                          │
│    all workspaces · expanded or collapsed                            │
├──────────────────────────────────────────────────────────────────────┤
│ 6. STATUS STRIP                                                      │
│    project · frame · FPS · zoom · selection · status                 │
└──────────────────────────────────────────────────────────────────────┘
```

### Region 1 — Slim application/menu bar

The app bar contains only global identity, menus, a small number of
workspace-relevant toggles, state chips and the theme control. It is
not a second copy of the tool-group band.

Required contents:

- StykAnimate brand.
- File, Edit, View and Help menu buttons.
- Workspace-gated panel toggles.
- Current-frame and FPS chips.
- Theme control.

Project name is carried by the status strip (§7); it must not be
duplicated as another large app-bar title unless the design-spec
agent defines a compact, non-duplicative treatment.

Menus are specified in §6.

### Region 2 — Workspace navigation

The existing eight workspace controls remain the navigation surface.
Their order and indices are fixed:

| Index | Workspace |
|---:|---|
| 0 | Animate |
| 1 | Draw |
| 2 | Build |
| 3 | Graph |
| 4 | Effects |
| 5 | Audio |
| 6 | 3D |
| 7 | Export |

Alt+1 through Alt+8 continue to select the corresponding workspace,
subject to the existing rule that text authoring captures the
keyboard first.

Switching workspace:

- preserves the project, undo history, current frame, selection,
  camera, editor modes and panel flags except for the explicit
  effects already performed by `apply_workspace`;
- closes any open app-bar menu;
- changes the groups constructed in Region 3;
- changes which already-flagged panels may render in Region 4;
- does not save, reload or duplicate the project.

### Region 3 — Contextual toolbar / gated groups band

This region replaces the permanent button wall. It contains the
existing labelled `tool_group` surfaces, but only the groups assigned
to the active workspace by §3.

The groups retain their existing labels, internal order, button IDs
and listeners. Wave 15 changes **which group elements are
constructed**, not what the surviving buttons do.

### Region 4 — Main editing environment

The centre stage or workspace surface is the largest element. The
left and right columns are conditional:

- Figures sidebar: Animate and Build only.
- Contextual inspector: Animate and Build only.
- Graph, Effects, Audio, Export, Clips, Colour, Easing and Weather
  panels: only as allowed by §4, using their existing placement in
  or below the centre column.
- Home: may replace or overlay the central content in any workspace
  under the existing Home layout mechanics.
- Help: may render in any workspace under the existing Help layout
  mechanics.

A panel being allowed does not force it open. Its flag must also be
true.

### Region 5 — Timeline

The timeline exists in every workspace and is independently
collapsible. See §5.

### Region 6 — Status strip

A new slim bottom row below the timeline gives persistent,
non-interactive session readouts and the current status message.
See §7.

---

## 3. Workspace → tool-groups contract

The application has exactly these 24 existing tool groups:

> Transport, Figure, Frames, Segments, Figures, Draw, Camera,
> Playback, Effects, Effects 2, Library & Clips, Export, Projects,
> Import, View, Audio & Text, 3D, Scene, Audio Ops, Formats,
> Stage & Language, Home, Segment Style, Strokes.

The labels above are the source labels used by `tool_group(...)` and
must stay synchronised with the help panel's group inventory.

### 3.1 Fixed mapping

| Workspace | Tool groups constructed by `render()` | Count |
|---|---|---:|
| **Animate** | Transport, Figure, Frames, Segments, Figures, Camera, Playback, View, Library & Clips, Scene, Stage & Language, Home | 12 |
| **Draw** | Draw, Strokes, View, Stage & Language, Home | 5 |
| **Build** | Figure, Segments, Figures, Segment Style, Library & Clips, Stage & Language, Home | 7 |
| **Graph** | Frames, View, Stage & Language, Home | 4 |
| **Effects** | Effects, Effects 2, Scene, View, Stage & Language, Home | 6 |
| **Audio** | Audio & Text, Audio Ops, Transport, Playback, Stage & Language, Home | 6 |
| **3D** | 3D, Camera, View, Stage & Language, Home | 5 |
| **Export** | Export, Formats, Projects, Import, View, Stage & Language, Home | 7 |

There are 52 workspace/group memberships. Every one of the 24 groups
appears in at least one workspace; no group is orphaned or deleted.

### 3.2 Interpretation rules

1. **Exact inclusion.** A workspace constructs the groups in its row
   and no others. “Useful”, “related” or “normally harmless” is not a
   reason to add a group.
2. **No empty shells.** A disallowed group does not render an empty
   labelled box, a heading by itself, or a disabled placeholder.
3. **No duplicate relocation.** A group is not copied into a menu or
   another panel to bypass the matrix. Existing app-bar commands are
   separate call sites with their own preserved IDs (see §6).
4. **Internal group content is frozen.** Button order, labels, IDs,
   listeners and editor operations inside a group do not change as
   part of gating.
5. **Common does not mean universal.** `Stage & Language` and `Home`
   occur in every row of the fixed matrix. Other apparent patterns do
   not override the table: for example, Build and Audio do **not**
   receive `View`, even though most other workspaces do.
6. **Group order follows the matrix row** unless the design spec
   gives a visual ordering that preserves exactly the same set. A
   reordering must not change membership.
7. **The gate is by workspace index/name at render time.** It must
   not be encoded by deleting branches for other workspaces or by
   changing panel flags on every frame.

### 3.3 What the user should experience

- **Animate** exposes posing, frame, camera, playback and scene
  controls, without drawing, audio-operation, 3D or export-format
  groups competing for space.
- **Draw** becomes a small drawing surface: Draw and Strokes only,
  plus the common View/language/Home groups.
- **Build** focuses on figure and segment construction and style.
- **Graph** removes nearly all of the button wall; the graph panel
  and Frames controls carry the workspace.
- **Effects** shows both existing FX groups and Scene, not the
  unrelated posing or export sets.
- **Audio** pairs the audio groups with Transport and Playback.
- **3D** shows the existing 3D group with Camera, not the general
  animation wall.
- **Export** gathers Export, Formats, Projects and Import while the
  export centre is gated to this workspace.

---

## 4. Workspace → panels contract

### 4.1 Fixed panel matrix

| Panel / surface | Existing state or source | Animate | Draw | Build | Graph | Effects | Audio | 3D | Export |
|---|---|:---:|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| Graph | `show_graph` | — | — | — | **Yes** | — | — | — | — |
| FX / Effects | `show_fx` | — | — | — | — | **Yes** | — | — | — |
| Audio | `show_audio` | — | — | — | — | — | **Yes** | — | — |
| Export centre | `show_export` | — | — | — | — | — | — | — | **Yes** |
| Movieclips | `show_clips` | **Yes** | — | — | — | — | — | — | — |
| Colour | `show_color` | **Yes** | **Yes** | **Yes** | — | — | — | — | — |
| Easing | `show_easing` | **Yes** | — | — | **Yes** | — | — | — | — |
| Weather | `show_weather` | **Yes** | — | — | — | **Yes** | — | — | — |
| Inspector | `show_inspector` | **Yes** | — | **Yes** | — | — | — | — | — |
| Figures sidebar | workspace-gated layout surface | **Yes** | — | **Yes** | — | — | — | — | — |
| Home | `show_home` | **Any** | **Any** | **Any** | **Any** | **Any** | **Any** | **Any** | **Any** |
| Help | `show_help` | **Any** | **Any** | **Any** | **Any** | **Any** | **Any** | **Any** | **Any** |
| Timeline | collapse state, §5 | **All** | **All** | **All** | **All** | **All** | **All** | **All** | **All** |

### 4.2 Panel rules

1. **Flag plus workspace.** Except for the figures sidebar, a panel
   renders only when its existing flag is true **and** the active
   workspace appears in its row.
2. **No flag clearing at the gate.** The render branch must not set
   a flag to false just because the active workspace disallows the
   panel.
3. **No construction when disallowed.** Panel data preparation,
   waveform/statistics work and builder calls specific to a hidden
   panel should be inside the allowed branch wherever the current
   code structure permits.
4. **Panel toggles follow the same matrix.** A panel toggle rendered
   in the app bar or menu is itself visible only in workspaces where
   that panel may render. The toggle's ID and listener remain
   unchanged when visible.
5. **Mutual exclusion is unchanged.** Where existing toggle handlers
   close graph/audio/FX/clips/inspector panels, they continue to do
   so. Gating is an additional render predicate, not a replacement
   state machine.
6. **Home and Help are global surfaces.** Their flags are sufficient
   in every workspace. This does not add the Home tool group to a
   workspace — it is already in every group row — and it does not
   create a new Help tool group.
7. **Inspector content is contextual, placement is gated.** In
   Animate and Build, the inspector continues to describe the
   selected figure/node using the existing inspector data path. In
   all other workspaces, the inspector column is absent even if
   `show_inspector` is true.
8. **Sidebar absence collapses the column.** In workspaces other
   than Animate and Build, no empty left rail, figure heading or
   disabled figure list is retained.

### 4.3 Stage and panel stacking

Allowed workspace panels retain their existing relationship to the
stage: they stack in or below the centre column rather than becoming
independent windows. Opening an allowed panel may reduce the stage's
available height, but an unrelated panel from another workspace may
never appear there.

The stage remains the visual centre because:

- the groups band is limited to the active task;
- the side columns exist only in Animate and Build;
- centre panels are limited to the active workspace;
- the timeline can be collapsed;
- the status strip is read-only and slim.

No fixed percentage size is promised by this architecture. GPUI
layout, window size, panel choice and timeline state all affect the
actual stage bounds; visual acceptance requires real desktop
screenshots, not a source-code assertion.

---

## 5. Timeline contract

The timeline is not workspace-gated. It is present in Animate, Draw,
Build, Graph, Effects, Audio, 3D and Export.

### 5.1 Expanded state

When expanded, the timeline retains its existing layers and
interactions:

- dopesheet strip;
- frame ruler and scrubber;
- frame chips, including range selection;
- timecode;
- keyframe/range presentation;
- existing click behaviour and IDs.

The timeline's data continues to come from the project and the
existing dopesheet/keyframe-map path. Workspace gating must not
create a second timeline model.

### 5.2 Collapsed state

Timeline collapse is a view state, persisted through the layout
preferences (§9). When collapsed:

- the dopesheet, ruler/scrubber and frame-chip body are not
  constructed;
- a slim timeline header remains visible;
- the header retains enough real context to identify the timeline
  and offers the expand control;
- the status strip remains a separate row below it.

When expanded again, the same timeline body returns. Collapsing does
not change the current frame, loop range, range selection, project
or playback state.

### 5.3 Collapse, not drag-resize

GPUI 0.2.2 does not provide the native splitter/docking model implied
by a freely draggable professional timeline. Wave 15 therefore uses
an explicit collapse/expand control. It must not simulate a drag
handle that does not actually resize the timeline.

The collapse control is a new, real control. Its listener changes
only the collapse state (and persists it through the agreed layout
path); it is not a decorative chevron.

---

## 6. Application menus contract

### 6.1 Menus are custom dropdown rows

GPUI 0.2.2 has no native application-menu, dropdown, popover or
tooltip primitive for this shell. Wave 15 menus are therefore custom
GPUI elements:

- File, Edit, View and Help are real menu buttons in the app bar.
- At most one menu is open at a time.
- An open menu renders as a dropdown row/panel directly associated
  with the app bar.
- Choosing a command closes the menu after invoking the command.
- Switching workspace closes the menu.
- The open-menu value is transient view state and is not persisted
  as a layout preference.

The implementation must not describe these as native OS menus.

### 6.2 Menus reuse existing commands

Menu rows reuse the existing app-bar command buttons and their
operations. They do not introduce parallel implementations of Save,
Load, Undo or other commands.

The logical families are:

| Menu | Contents |
|---|---|
| **File** | Existing app-bar file commands, such as save/load/export entry points, with their existing IDs and listeners. |
| **Edit** | Existing app-bar edit commands, such as undo/redo and frame copy/paste, with their existing IDs and listeners. |
| **View** | Existing panel toggles and view controls, filtered by the panel matrix in §4; theme remains the separately specified app-bar control. |
| **Help** | The existing Help entry point and shortcut-reference surface; no invented About, update or web actions. |

“Reuse” means the same operation and ID, not a visually similar
button wired to a duplicate code path. A command must not be
instantiated twice in the same visible render tree merely because
both a menu and another surface are open; the app-bar/menu owner is
responsible for choosing the single app-bar render branch for those
existing app-bar commands. Tool-group buttons are separate preserved
call sites and are governed by §3.

### 6.3 Slim app-bar rules

- Panel toggles in the app bar follow §4. A Graph toggle is not shown
  in Animate, an FX toggle is not shown in Audio, and so on.
- Frame and FPS chips remain visible because they describe the
  session in every workspace.
- The theme control remains visible in every workspace and continues
  to operate on the single existing theme state,
  `EditorCore::light_theme`.
- The app bar must not accumulate one visible toggle for every panel
  in every workspace. That would recreate the button wall at a
  smaller scale.

---

## 7. Status-strip contract

The status strip is a new bottom region below the timeline. It is
always present, in every workspace and in both timeline states.

### 7.1 Required fields

From left to right, the strip presents:

1. **Project name** — the current project's actual name.
2. **Frame** — current frame and total frame count.
3. **FPS** — the project's actual frames-per-second value.
4. **Zoom** — the evaluated camera zoom at the current frame.
5. **Selection** — the selected figure, and the selected node where
   one is selected; otherwise a truthful no-selection label.
6. **Status** — the existing editor status message, with the
   established toast treatment taking precedence where the current
   UI already gives active toasts priority.

All values are derived from the current `EditorCore`/`Project`
state. The strip contains readouts, not editable fields and not
buttons disguised as labels.

### 7.2 No duplicate status surface

The strip becomes the persistent home of the plain status message.
The implementation must not render the same plain status string in
both the old timeline row and the new strip in the same frame.
Transient toast notifications may retain their existing notification
presentation; they are not a second persistent status field.

### 7.3 No fabricated save state

The required status field is the editor's real status text. The
strip must not invent “Saved”, “Synced” or a timestamp that the
application does not actually track. Autosave/recovery behaviour is
unchanged.

---

## 8. Preservation contract

### 8.1 Button and control inventory

The preservation baseline is the Wave 14 QA inventory in
`docs/QA_WAVE14.md`:

- all **235 unique pre-Wave-14 HEAD button IDs** survive;
- the Wave 14 additions also survive: `view-fit`, `view-centre`,
  `view-actual` and the clickable workspace family `("ws", i)`;
- export-centre `xc-*`, tuple IDs, stepper-forwarded IDs and clickable
  `.id(...)` families are included in the audit method, not treated
  as disposable because they are not plain `btn("...")` calls.

Wave 15's new timeline-collapse and menu controls are additions.
They must be recorded by QA with the real functions their listeners
call.

### 8.2 Gating is not permission to remove

No agent may:

- delete a tool group because it is hidden in most workspaces;
- delete or rename a button ID to make a workspace branch compile;
- replace a listener with `cx.notify()` or a status-only stub;
- remove a panel builder because its workspace gate is narrow;
- change an editor operation's semantics to fit a layout branch;
- weaken or delete a test to accommodate gating;
- claim a hidden control was “removed from the product” — it is
  available in its assigned workspace.

A source-level ID inventory must show no missing baseline IDs after
Wave 15. A runtime/workspace inventory must additionally show that
each ID is constructed only in surfaces allowed by this contract.

### 8.3 Engine and format preservation

Wave 15 is a UI-layer programme. It preserves, without redesign:

- project serialization and named-project storage;
- Pivot `.piv` and `.stk` import/export;
- Stick Nodes `.nodes`, `.nodemc` and `.stknds` support;
- FK, two-bone IK, bends and figure evaluation;
- timeline interpolation, graph channels and easing;
- camera animation and motion paths;
- onion skin;
- drawing, brushes and deterministic stroke rendering;
- VFX and weather rendering;
- audio processing, playback planning and export muxing;
- movieclips;
- 3D posing sessions and baking;
- PNG, JPEG, GIF, APNG, MP4, WebM, SVG, spritesheet and contact-sheet
  exports;
- autosave and recovery;
- all MCP tools and the MCP server's separation from the GUI;
- all 28 language tables.

If an implementation appears to require an engine change, the agent
stops and routes the issue through the orchestrator. A layout goal
does not authorise an engine rewrite.

---

## 9. Layout-state contract

Layout state remains app state, not project state.

### 9.1 Existing persisted values

Wave 14's `shell::LayoutPrefs` persists:

- `workspace` — the active workspace name;
- `dark` — the persisted mirror of the single real theme state,
  `EditorCore::light_theme`.

The constructor restores the theme from that mirror; workspace
application writes the mirror back. There must not become a second
theme boolean owned by the view.

### 9.2 Wave 15 addition

The layout record gains one value:

- `timeline_collapsed: bool`, default `false`.

Missing or corrupt layout data continues to produce defaults:
Animate workspace, dark theme, expanded timeline.

Panel flags are not added to the layout record by Wave 15. They keep
their existing session behaviour: they start closed unless existing
startup/workspace code sets them.

The open app-bar menu is not persisted.

### 9.3 Restore-default meaning

Restoring defaults means applying the default layout record —
Animate, dark, timeline expanded — through the existing layout
persistence path. It does not reset the project, camera, selection,
undo history or editor session.

---

## 10. Design-system contract

All Wave 15 surfaces consume `src/ui/kit.rs` tokens and the existing
component contracts. Implementers must not scatter the new palette as
hex literals through `app.rs` or component files.

The kit owner is the only agent who changes token values or adds
tokens. If a menu, status or panel component needs a semantic colour
that does not exist, it requests the token through the orchestrator
rather than defining a local duplicate.

Component rules remain:

- components are pure builders over plain data;
- callers attach IDs and listeners;
- text and labels passed into a component are owned values where the
  existing component pattern requires them;
- no component reaches into `EditorCore` or the project;
- test modules in files that import `gpui::*` use exact-name imports,
  never `use super::*;`.

The status strip and menu dropdown are presentation components under
the same rule. Their app-specific data assembly and listeners live in
`app.rs`.

### Accessibility floor

Wave 15 adopts this minimum for ordinary UI text:

- main controls: the established 13 px button scale;
- status, menu and secondary readouts: at least 11 px;
- no new informational text below 11 px;
- the existing 10 px uppercase section-label treatment may remain
  only for non-essential group captions already using it; it must not
  be introduced for status, menu commands or instructions.

Colour must not be the only signal for workspace, panel or timeline
state; the visible label or selection treatment must also identify
the state.

---

## 11. GPUI 0.2.2 limits — binding interpretations

The following are not unfinished aspirations; they are the agreed
implementable forms for Wave 15:

| Directive desire | GPUI reality | Wave 15 form |
|---|---|---|
| Native menus | No native menu primitive in the app toolkit | Custom app-bar menu buttons with a dropdown row |
| Popovers/tooltips | No general native popover/tooltip primitive | Existing panels, help surface and inline labels |
| Docking panels | No native dock/splitter system | Fixed regions, conditional columns and workspace gating |
| Drag-resizable timeline | No agreed splitter implementation | Timeline collapse/expand |
| Touch-first panels | GPUI 0.2.2 has no touch input path for this app | Mouse/keyboard interactions only |
| Separate workspace windows | Not the existing shell model | One shell whose constructed regions change by workspace |

No agent may fake the left-hand column with a non-functional
divider, a decorative resize handle, an empty dock area or a menu
that opens an unrelated panel without identifying it as that panel.

---

## 12. Performance contract

Workspace gating is the first performance improvement: hidden groups
and panels are not constructed.

The separate Wave 15 caching work addresses known render-path disk
access identified by the Wave 14 audit:

- project-library scanning while Home is open;
- FX preset-name reading while the FX panel is open.

Caching must preserve truthfulness:

1. A cached list must be invalidated or refreshed after an in-app
   operation that creates, renames, duplicates, deletes, saves or
   otherwise changes the underlying entries.
2. A cache must not fabricate entries or retain a deleted entry as
   if it still existed.
3. The caching owner documents the invalidation trigger for each
   cache and covers it with tests where a testable seam exists.
4. Gating must not be used to claim a cache works: a panel hidden by
   workspace is simply not rendered; when it becomes visible, its
   data must be correct.

No performance change may alter deterministic rendering output.

---

## 13. Verification contract

Wave 15 is not accepted from code inspection alone.

### 13.1 Source and behaviour evidence

QA must verify:

1. **ID preservation:** the Wave 14 baseline inventory is still
   present in source; no baseline ID is missing.
2. **Group matrix:** for each of the eight workspaces, the group set
   constructed by the render branch equals §3 exactly — no extras,
   no omissions.
3. **Panel matrix:** for representative true/false flag combinations,
   the panel construction predicate equals §4.
4. **No dead controls:** every new menu, workspace, panel-toggle and
   timeline-collapse control calls a real function or changes real
   view state consumed by `render()`.
5. **Flag persistence across workspaces:** setting a panel flag,
   switching to a disallowed workspace and back does not clear the
   flag or lose the panel.
6. **Timeline:** the timeline renders in every workspace; collapse
   removes only its body; expand restores it without changing frame
   or selection.
7. **Status strip:** every field is derived from live project/core
   state, and the plain status text is not duplicated.
8. **CI:** the sanctioned GitHub Actions workflow passes. No local
   Cargo build or test is part of Wave 15 verification unless the
   repository owner separately approves that exact run.

### 13.2 Visual evidence

CI cannot prove that the stage is dominant or that the interface no
longer looks like a dashboard. Real desktop screenshots remain a
required acceptance item, covering:

- all eight workspaces;
- Home and Help;
- expanded and collapsed timeline;
- an open File/Edit/View/Help menu;
- light and dark themes;
- the agreed desktop window sizes, including a small laptop size and
  a large desktop size.

Until those screenshots are reviewed, the correct status is
**“implemented and CI-verified; visual acceptance pending”**, not
“visually complete.”

---

## 14. Non-goals for Wave 15

Wave 15 does not attempt to:

- rewrite `app.rs` into a new framework;
- create eight separate applications or windows;
- replace the project model or editor operations;
- invent unsupported timeline tracks, export formats, effects or
  figure properties;
- add a command palette as a substitute for the specified menus;
- introduce drag docking or a fake splitter;
- localise every existing English button label as a side effect of
  gating;
- solve every panel's internal information architecture in the same
  change as the shell gating.

Those limits do not reduce the central requirement: after Wave 15,
the default editing experience must no longer present the entire
24-group control wall in every workspace.
