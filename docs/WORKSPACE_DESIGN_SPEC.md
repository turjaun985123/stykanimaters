# Workspace Design Spec — StykAnimate-RS 2.0 (Wave 15)

**Status:** the per-workspace contract implementers build against in
Wave 15. It is grounded in the code as it stands after Wave 14
(CI-green, 893 tests): the 24 tool groups in `src/app.rs`, the
`EditorView` panel flags, the Wave 14 workspace presets
(`apply_workspace`, app.rs), the `src/ui/` component library and the
`src/ui/kit.rs` token set. Where this spec asks for something the
code does not have yet, it says so explicitly; nothing here invents
functionality.

**Relationship to Wave 14.** Wave 14 shipped the workspace
*selector*: switching workspaces applies panel/mode presets (graph /
FX / audio / export panels, draw mode, 3D session, inspector) and
persists the choice via `shell::LayoutPrefs`. It did **not** change
which tool groups are visible — all 24 groups render in every
workspace. **Wave 15's defining change is per-workspace group
filtering** (§2): the tool-group band renders only the groups
assigned to the active workspace. The button wall disappears
because most of it is simply not on screen for the current task —
not because it was deleted. Every group, button id and listener
from Wave 14 survives unchanged; filtering is a render-time
decision in one place (the band assembly in `render()`).

---

## 1. Shell invariants (all workspaces)

These regions exist in every workspace and never change identity:

| Region | Contents (existing) |
|---|---|
| App bar | `brand()`, File/Edit button groups, panel toggles (graph / audio / clips / FX / export / inspector / help), frame + FPS `info_chip`s |
| Workspace selector | 8 `workspace_segmented` segments, ids `("ws", i)`; Alt+1..Alt+8 |
| Tool-group band | **Filtered per §2** — the only region whose contents change with the workspace |
| Figures sidebar | `layer_row_shell` rows: colour dot, name (`fig-sel`), opacity %, up/down, lock, solo, opacity steppers, eye, delete; bulk visibility buttons |
| Stage column | Dark `C_BG` surround, Fit/Centre/1:1 viewport row, framed canvas with HUD strip inside the frame, optional guides overlay, hint row |
| Bottom bar | Dopesheet strip, timeline ruler + scrubber, frame chips (shift-click range), transport, timecode chip, status / toast area |
| Panels | Overlay/panel column driven by `show_*` flags (graph, FX, audio, clips, export, colour, weather, easing, inspector, Home, help) |

Rules:

- **Project and editor state are untouched by switching.** A
  workspace switch changes group visibility + the Wave 14 panel
  preset only. Poses, frames, selection, undo history, playback —
  all preserved.
- **The stage is the largest element in every workspace.** Panels
  open *beside or below* the stage column (graph below the canvas,
  the rest in the panel column); no workspace may cover the stage
  with controls. Home is the single exception: it replaces the
  stage area, as today, and is a panel — not a ninth workspace.
- **Mutual exclusion stays as implemented:** graph / audio / FX /
  clips / inspector close each other; colour, weather, easing and
  help are independent overlays. Workspace presets reuse the
  existing toggle bodies verbatim (Wave 14 rule).

---

## 2. Group visibility — THE Wave 15 mapping (fixed)

The band renders exactly these groups per workspace. Groups not
listed are not rendered (their buttons are not hidden-but-present;
the `tool_group(...)` call is skipped for this render). Group order
within the band follows the order listed here.

| Workspace | Groups shown |
|---|---|
| Animate | Transport, Figure, Frames, Segments, Figures, Camera, Playback, View, Library & Clips, Scene, Stage & Language, Home |
| Draw | Draw, Strokes, View, Stage & Language, Home |
| Build | Figure, Segments, Figures, Segment Style, Library & Clips, Stage & Language, Home |
| Graph | Frames, View, Stage & Language, Home |
| Effects | Effects, Effects 2, Scene, View, Stage & Language, Home |
| Audio | Audio & Text, Audio Ops, Transport, Playback, Stage & Language, Home |
| 3D | 3D, Camera, View, Stage & Language, Home |
| Export | Export, Formats, Projects, Import, View, Stage & Language, Home |

Notes for implementers:

- `Stage & Language` (stage size, theme toggle, language cycler)
  and `Home` (Home panel toggle) appear in all eight — they are
  application-level, not task-level, controls.
- `View` appears in seven (all but Build): guides, onion and
  display toggles are stage concerns; Build keeps its list short
  deliberately.
- The full 24-group inventory stays in the source in today's
  order; the filter is a per-workspace allow-list matched on the
  group label. Do not renumber, rename or reorder groups — the
  help panel's `group_rows()` (24 rows) and its tests describe the
  full inventory, not the visible subset, and stay valid.
- QA expectation: in Draw, the band shows 5 groups; in Animate,
  12. No workspace shows more than 12. If a screenshot shows all
  24, the filter is not wired.

---

## 3. The eight workspaces

For each: purpose, panels on entry (the Wave 14 preset, restated),
stage contents, inspector behaviour, and the user's top 3 tasks —
the tasks the layout must make effortless.

### 3.1 Animate — *pose figures and play the animation back*

- **Panels on entry:** inspector ON; graph / audio / FX / clips /
  export closed; draw mode off; 3D session exited.
- **Stage:** framed canvas with joint handles and head rings,
  onion skin (View group), motion paths when enabled
  (`core.show_paths`), HUD strip (mode / tool / camera / weather /
  loop / onion chips), guides overlay when toggled.
- **Inspector:** the selected figure + segment —
  `InspectorData { figure_name, figure_color, node_index,
  seg_count, length, thickness, angle_deg, is_origin }` with the
  length/thickness steppers and read-only gauges.
- **Top 3 tasks:** (1) pose the selected figure by dragging joints
  (FK aim / two-bone IK / root move); (2) scrub, play and
  frame-step the timeline while posing; (3) nudge a segment's
  length or thickness in the inspector and see it on stage.

### 3.2 Draw — *paint strokes on the current frame*

- **Panels on entry:** draw mode ON (`core.draw_mode`; canvas drag
  paints a stroke on the current frame), colour panel ON.
- **Stage:** the same canvas in draw mode; existing strokes render
  through the deterministic stroke path (rasterizer / SVG /
  canvas share one source, spray included). Brush feedback is the
  status line's draw-mode message plus the stroke as drawn.
- **Inspector:** figure/segment inspector remains available but is
  not the Draw property surface — the **colour panel** (16-swatch
  picker writing `NodeDef.color`) and the Strokes group (brush
  cycle, draw-tool toggle, simplify, mirror, stamp ops) are.
- **Top 3 tasks:** (1) paint a stroke with the active brush;
  (2) switch brush preset / eraser and keep painting; (3) clean up
  a frame's strokes (simplify, mirror, erase).

### 3.3 Build — *construct and style figures*

- **Panels on entry:** inspector ON, colour panel ON; graph /
  audio / FX / clips closed.
- **Stage:** canvas with the figure under construction; joint
  handles for posing the rest pose; the Figures sidebar for
  selecting which figure is being built.
- **Inspector:** the selected segment's length / thickness /
  angle — the core Build property surface, with the colour panel
  for segment colour.
- **Top 3 tasks:** (1) add and extend segments on a figure
  (Figure / Segments groups); (2) style segments — taper, cap,
  colour, gradient (Segment Style group + colour panel);
  (3) save the figure to the library and load / cycle library
  figures (Library & Clips group).

### 3.4 Graph — *shape motion with animation curves*

- **Panels on entry:** graph panel ON (audio / FX closed by
  mutual exclusion). The graph panel renders below the canvas:
  the channel curve canvas with draggable keys and tangent
  handles, overlay curves for the figure's other channels, the
  axis label row, and the tangent-preset buttons.
- **Stage:** canvas stays live above the graph — posing and curve
  edits are checked against each other without switching.
- **Inspector:** not part of the Graph preset; the graph panel
  itself is the property surface (selected channel + key state
  lives in the view's `graph_sel`).
- **Top 3 tasks:** (1) pick a channel and drag a key in time /
  value; (2) reshape a segment with the tangent handles or a
  tangent preset (Smooth / EaseIn / EaseOut / EaseInOut / Linear);
  (3) mirror or scale a channel's keys to reuse motion.

### 3.5 Effects — *place particle effects and set the weather*

- **Panels on entry:** FX panel ON (graph / audio closed). The FX
  panel lists clips as `clip_row` span bars with per-clip
  earlier / later / duplicate / delete, the kind buttons for all
  27 `ClipKind`s, and the preset save / apply section.
- **Stage:** canvas with live FX particles (clips and weather
  render through the same `clip_particles` / `weather_particles`
  path as exports — what you see is what renders).
- **Inspector:** not part of the preset; clip properties are
  edited in the FX panel (start, figure/node attachment, colour
  and `color2` gradient end via the add-clip flow and MCP parity).
- **Top 3 tasks:** (1) add an FX clip at the playhead on the
  selected figure; (2) retime / move / duplicate an existing clip;
  (3) set scene weather (Scene group: kind segmented control —
  Off / Rain / Snow / Fireflies / Fog / Storm — amount and wind).

### 3.6 Audio — *sync sound to the animation*

- **Panels on entry:** audio panel ON (graph / FX closed). The
  panel shows the waveform overview strip (peaks of the first
  clip), the clip list with per-clip normalise / split / crossfade
  buttons, and the panel-level ops: echo, lowpass, trim-silence,
  pitch, compress, split-at-silences, mixdown.
- **Stage:** canvas + Transport / Playback groups for
  play-through sync checks; live playback mixes all audible clips
  (`play_project`), honouring per-clip gain.
- **Inspector:** not part of the preset; clip state (path, start
  frame, gain) is surfaced in the panel rows and the status line.
- **Top 3 tasks:** (1) place an audio clip so it starts on the
  right frame; (2) trim / split / normalise a clip to fit the
  scene; (3) mix down or export with the mix the panel describes.

### 3.7 3D — *pose in 3D, bake to a 2D figure*

- **Panels on entry:** 3D session entered (`core.enter_3d()`,
  self-guarding — an active session is kept). No panel preset;
  the 3D tool group is the working set.
- **Stage:** the canvas shows the 3D session's preview (the
  fig3d figure projected through its camera) alongside the 2D
  project; orbit / joint-rotate interactions run through the
  session ops.
- **Inspector:** figure/segment inspector remains available for
  the 2D side; 3D joint state is session state, shown via the
  stage preview and status, not the inspector.
- **Top 3 tasks:** (1) rotate joints and orbit the camera to
  build a pose; (2) mark pose A / pose B and bake the session to
  a native 2D figure; (3) save / load figures in the 3D library
  and solve arm / leg IK to reach a target.
- **Honesty bound (from the Wave 8/14 docs):** 3D is a posing
  *session* that bakes into 2D figures. The workspace must not
  present it as a 3D animation-document pipeline.

### 3.8 Export — *ship the animation in any supported format*

- **Panels on entry:** export centre ON (`panel_shell_themed`),
  graph / audio / FX / clips closed. The centre has the
  All / Media / Formats tab strip (with the tab-underline rule),
  the 15 `xc-` export rows calling the existing `EditorCore`
  export ops, and the loop-range info line when a loop range is
  set.
- **Stage:** canvas remains visible with the current frame — the
  framing check before exporting. Fit / Centre / 1:1 stay
  available (View group).
- **Inspector:** not part of the preset; output destinations are
  stated per row in the centre (e.g. `./export/`, `out/` for
  v5/APNG/JPEG — the panel's existing note stands).
- **Top 3 tasks:** (1) export the finished animation (MP4 / GIF);
  (2) export a frame range, PNG sequence, contact sheet or
  spritesheet; (3) import a `.piv` / `.nodes` / `.stknds` file
  (Import group) and export it back out (Formats group).

---

## 4. Palette — 2.0 directive colours vs Wave 14 tokens

One token change restyles every consumer (Wave 14 rule). Wave 15
retunes **values**, adds **three** tokens, and renames **nothing**.
`Palette::LIGHT` is unchanged this wave (the directive is
dark-first; light adoption remains phased, as documented in
Wave 14).

| Directive colour | Hex | Kit token | Wave 14 value | Wave 15 action |
|---|---|---|---|---|
| Main background | #111318 | `C_BG` | 0x101216 | Retune → 0x111318 |
| Application chrome | #171A21 | `C_SURFACE` | 0x171A21 | Keep |
| Panel background | #1D222B | `C_PANEL` | 0x1D222B | Keep |
| Elevated surface | #252B35 | `C_ELEVATED` | 0x252B36 | Retune → 0x252B35 |
| (chip / cell idle) | — | `C_CELL` | 0x252B36 | Align to elevated → 0x252B35 |
| Hover surface | #303846 | **`C_HOVER` (new)** | — (btn() hardcodes 0x3A4150) | Add token; `btn()` hover + component hovers consume it |
| Primary text | #F1F3F7 | `C_INK` | 0xF1F3F7 | Keep |
| Secondary text | #A2AAB8 | `C_MUTED` | 0xA1A9B7 | Retune → 0xA2AAB8 |
| Muted (faint) text | #707B8C | **`C_FAINT` (new)** | — (ad-hoc greys in app.rs) | Add token for hints / disabled-adjacent text |
| Borders | #303744 | `C_BORDER` | 0x303745 | Retune → 0x303744 |
| Primary accent | #6D8CFF | `C_ACCENT` | 0x6C8CFF | Retune → 0x6D8CFF |
| Secondary accent | #A184FF | `C_ACCENT2` | 0x9B7CFF | Retune → 0xA184FF (largest shift) |
| Success | #55C99A | **`C_SUCCESS` (new)** | — (badge local 0x2E7D4F) | Add token; badge Success + gauge success consume it |
| Warning | #E8B65D | **`C_WARNING` (new)** | — (badge local 0x8A6D1F) | Add token; badge Warn + gauge warn consume it |
| Error | #F16F78 | `C_ERROR` | 0xB23A3A | Retune → 0xF16F78 (bright coral replaces deep red) |

Rules:

- The three new tokens land in `kit.rs` and in the `Palette`
  struct (`hover`, `faint`, `success`, `warning` fields) in the
  same change; `Palette::DARK` is built from the flat consts, as
  today, so struct and consts cannot drift.
- Tests that pin token values (kit's own table, badge /
  frame_chip / layer_row / progress_steps expectations) are
  updated to the new literals **in the same commit** — Wave 14's
  CI saga (9 stale pins) is the lesson.
- Deliberate non-token colours stay literal and documented:
  badge's data tones migrate to `C_SUCCESS`/`C_WARNING` (above);
  dialog scrim (RGBA-packed), frame-chip range blue 0x2E4470,
  colour-picker palette (user data), weather/HUD data colours.
- No other hex literal may be introduced anywhere in `src/ui/`
  (the Wave 14 end state was 0 outside kit.rs; it stays 0).

---

## 5. Typography floor

The `TEXT_*` scale (XS 11 / SM 12 / MD 13 / LG 15 / XL 18) is the
only source of text sizes in new or touched code.

| Level | Token | Size | Used for |
|---|---|---|---|
| Floor | — | **10 px minimum, anywhere** | Nothing renders below 10 px |
| Section labels | (kit `section_label`) | 10–11 px, uppercase, `C_MUTED` | Group and panel headers |
| Secondary text | `TEXT_XS` | 11 px | Hints, row meta, chip-adjacent notes |
| Small | `TEXT_SM` | 12 px | Chips, badges, status-adjacent text |
| Body | `TEXT_MD` | 13 px | Buttons (`btn()`), panel body, list rows — the default reading size |
| Heading | `TEXT_LG` | 15 px | Panel titles, brand suffix contexts |
| Display | `TEXT_XL` | 18 px | Dialog headings, Home title |

- The current `section_label` renders at 10 px — at the floor,
  compliant, but it is the *only* element allowed at 10. Any new
  caption style uses 11.
- Long translated labels (28 languages) must truncate or wrap per
  the existing component contracts (e.g. `rows::clamp_label`),
  never shrink below the floor to fit.

---

## 6. Progressive disclosure — three levels

| Level | What | Where | Rule |
|---|---|---|---|
| **1 — Workspace tools** | The active workspace's groups (§2), transport, workspace selector, stage, timeline | Always visible | Everything at Level 1 must be usable without opening anything |
| **2 — Contextual panels** | Graph, FX, audio, clips, export, colour, weather, easing, inspector, Home, help | Opened by workspace preset or its toggle; mutual exclusion as implemented | A panel shows properties for the current selection/task only — the inspector's `InspectorData` is the model: figure + selected segment, nothing else |
| **3 — Advanced sections** | Inside Level 2 panels: FX presets, graph overlay curves + tangent presets, audio panel-level ops (pitch / compress / echo), export Formats tab, Segment Style group in Build | Reached by a tab, section or toggle *inside* the panel | Advanced controls never appear at Level 1; opening a workspace never opens a Level 3 section directly |

Disclosure is by **workspace → panel → section**, in that order.
A control's level is determined by how often the median user
needs it during that workspace's top-3 tasks (§3): needed every
session → Level 1; needed when an object is selected → Level 2;
needed for finishing work → Level 3.

---

## 7. Reject-by-design list

Implementations violating any of these are rejected at review,
however good they look:

1. **All-groups visibility.** Any render path that shows groups
   outside the active workspace's §2 list — including "show all"
   fallback modes, debug layouts left enabled, or the band
   ignoring `self.workspace`.
2. **Fake menus.** Menu-like chrome (File/Edit dropdowns,
   context menus, popovers) rendered without working items.
   GPUI 0.2.2 has no menu primitives; until a menu is real, the
   app bar keeps its working button groups. A labelled element
   that opens nothing is a defect, not a placeholder.
3. **Decorative controls.** Any button, chip, slider or toggle
   whose handler does not change editor/view state or call an
   editor op — including purely visual "selected" states on
   non-controls, disabled-looking buttons that are actually
   inert, and progress indicators not fed by real work.
4. **Fake data.** Project statistics, thumbnails, effect or
   weather states not computed from the actual project (the
   Home panel's real-scan rule from Wave 14 stands).
5. **Dead workspaces.** A workspace whose preset sets state
   nothing reads, or whose §2 group list is empty of task
   controls. Every workspace must pass its §3 top-3 tasks using
   only Level 1 + its own panels.
6. **Token bypass.** New hex literals in `src/ui/` outside
   `kit.rs`, or new text sizes outside the `TEXT_*` scale.
7. **Preservation breaks.** Any removed/renamed button id,
   group, panel or listener relative to the Wave 14 tree — the
   QA id-inventory diff (QA_WAVE14.md, Check 1 method) is re-run
   and must show additions only.

---

## 8. Verification for this spec

- **CI** (GitHub Actions, the repo's only sanctioned build)
  green: full test suite + binary packaging.
- **Group-filter audit:** scripted count of rendered
  `tool_group` labels per workspace against §2 (Animate 12,
  Draw 5, Build 7, Graph 4, Effects 6, Audio 6, 3D 5, Export 7).
- **Id inventory:** additions only vs the Wave 14 tree.
- **Screenshots:** still the user's move — the redesigned UI
  has never been screenshot-verified on a real display
  (checklist: docs/QA_WAVE14.md). Wave 15 adds one checklist
  line per workspace: *the band shows only this workspace's
  groups from §2.*
