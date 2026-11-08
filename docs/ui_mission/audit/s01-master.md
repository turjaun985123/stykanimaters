# S01 — Master UI/UX Research & Code Audit

**Squad 01 deliverable for the ULTIMATE UI/UX MISSION, Phase 1.**
Method: current source only (read + grep, no build — AGENTS.md
CI-only rule). Docs (GUI_AUDIT, AGENT_OWNERSHIP, PHASE_N §8,
PHASE_O_CHARTER, phase_o_plans/p05) were used as leads; every
claim below is either re-verified against source or explicitly
marked as carried from a named document. Where the mission
brief and the shipped code disagree, **source wins** and the
disagreement is listed as a decision item, not silently fixed.

---

## 1. UI entry-point inventory

There is exactly **one** Rune application entry point and one
non-UI binary. Everything the user sees comes from these:

| Entry point | File | Role |
|---|---|---|
| `fn main()` / `AppSpec::new("StykAnimate RS", (1440.0, 900.0), init, view)` | `src/main.rs:809-817` | The only window. Fixed launch size 1440×900. |
| `fn view(s, cx)` | `src/main.rs:558` | Shell composition: app bar + workspace row + three columns + overlays. |
| `fn poll(s, t, tx)` | `src/main.rs:310` | Per-frame state pump (playback, toasts, autosave, status→toast classification). |
| `fn on_key(s, ev, tx)` | `src/main.rs:466` | Global keys, incl. Alt+1..8 workspace switching (:477). |
| `pub struct State` (43 fields) | `src/main.rs` | All shell/view state; census-guarded by the gate. |
| Native file choosers | `src/dialogs.rs` | **Not Rune**: shells out to `zenity`/`kdialog`. OS-themed, outside the design system by construction (see D2). |
| `src/bin/mcp_server.rs` | — | MCP server binary. **Not a UI entry point**; excluded from this mission except where MCP mirrors UI ops. |

### 1.1 Shell composition (src/main.rs `view()`, verified)

- **Chrome**: `chrome_view::app_bar` (project name, save/undo/redo,
  menus) + `workspaces::workspace_row` (8 workspace chips) +
  `chrome_view::transport_row` / `timeline_row`.
- **Left column** `col().key("side-left")` (:578), 300 pt card
  column, content x 28..304 (budget **276**).
- **Centre**: stage canvas (`stage_view.rs`, 1,321 lines) with
  overlay passes — `stage_layers`, `stage_fx`, `stage_text`,
  `stage_camera`, `stage_fig3` (fig3 pass only in Build3D,
  :722) — plus the timeline stack and a waveform card
  (`design::panel_col("waveform-card")`, :674).
- **Right column** `col().key("side-right")` (:599), 340 pt,
  content x 1096..1412 (budget **316**).
- **Panel gating**: `workspaces::section_visible(ws, sec)` (:575)
  decides which sections mount per workspace **at construction**
  (a gated-off panel's builder is never invoked). Help and a few
  sections are deliberately ungated (workspaces.rs docs).
- **Overlays**: `menus` dropdowns, `command_palette`,
  `settings_popover`, `help_panel`, `toasts` — all mounted from
  the shell, all kit-styled since Phase N.
- **Status**: `status_strip.rs` (1,806 lines) + `chrome_view::status_text`.

### 1.2 Section modules (src/rune_sec/, 51 files, ~57k lines)

Declared in `src/rune_sec/mod.rs` ("one file per feature area…
every control calls a real EditorCore op or writes the same
public core field… sections that found no op say so instead of
faking one"). Full file→squad map in §3. Largest surfaces:
`snap_tests.rs` 3,742 (test-only, not in mod.rs), `graph_panel.rs`
3,180, `stroke_inspector.rs` 2,946, `command_palette.rs` 2,615,
`fx_clips.rs` 2,496, `keyframe_tools.rs` 2,449, `text_tool.rs`
2,299, `pose_tools.rs` 2,127, `timeline_marks.rs` 2,113,
`home_panel.rs` 2,071.

Engine/UI boundary: panels call `src/editor.rs` (7,294 lines,
EditorCore ops). **No squad changes engine behaviour**
(mission rule); UI defects that trace into editor.rs are
reported as interface requests to the integrator.

### 1.3 Verification entry points (the mission's evidence base)

- **Gate**: `scripts/phase_j_checks.py` — 6 checks, incl.
  `check_key_diff` (key census vs base ref), `check_state_literals`,
  `check_callers`, `check_design_adoption` (Phase N check 6).
  Baseline: **804 keys, zero loss is a hard rule**.
- **Scenes**: `src/rune_sec/scenes.rs` — 15 named states:
  default, draw-open, export-open, frame-3, fx-selected,
  fx-split-mid, graph-duplicates, loop-range, menu-open,
  palette-open, playing, selected, settings-open, three-d,
  three-d-stage. All dark-theme, all 1440×900 (see D2/D3).
- **Headless workflow**: `.github/workflows/headless.yml`
  (dispatch-only) emits `--tree` JSON + `--snap` PNG per scene;
  probes wipe `./projects` per invocation (Phase N fix 6 —
  layout prefs are CWD-relative, `src/shell.rs`).
- **Snap tests**: `src/rune_sec/snap_tests.rs` (49 tests at
  Phase N close).

---

## 2. Shared builders — the design system as it actually is

| Layer | File | Contents (verified) |
|---|---|---|
| Kit | `src/rune_sec/design.rs` (489) | Consts PANEL_RADIUS 12 / PANEL_PAD 12 / PANEL_GAP 8 / ROW_RADIUS 8 / CONTROL_RADIUS 6; fns `panel_col`, `header_row`, `panel_title`, `section_label`, `divider(_keyed)`, `kbd`, `empty_state`, `list_row`, `row_shell`, `toggle`, `toggle_row`, `stepper_value`, `slider_track`, `swatch`; `accent()`, `selected_fill()` (accent @ 0.16), `text_muted()`. |
| Buttons | `src/chrome_view.rs` | `btn` (primary family), `btn_inert`, `btn_muted_live`, `danger_btn` (DANGER_FILL 0xC93A44 / HOVER 0xB2323E / PRESS 0x9C2934). |
| Icon buttons | `src/rune_sec/icon_btn.rs` (615) | `chip_btn`, `icon_btn`, `icon_only_btn`, `danger_icon_btn`; live builders carry PRESS_SCALE 0.97. |
| Icons | `src/rune_sec/icon_data.rs` + `icons.rs` | Curated Font Awesome Free 7.3.1 table (`ICON_NAMES`, :181; 163 names per Phase H/N record); `icon_data(name) -> Option<(viewbox, path)>`; unknown names degrade silently — the Phase N icon audit proved 0 unresolved of 220 literal sites. Never mix icon styles (mission §S17). |
| Theme | Rune `Theme` via `LayoutPrefs.dark` (`src/shell.rs:260`, persisted) | Every builder takes `t: &Theme`. Dark is the verified theme; light exists and is selectable but see D2. Palette in force is the Wave-14 2.0 set (bg #111318, chrome #171A21, panel #1D222B, accent #6D8CFF…). |
| Workspaces | `src/rune_sec/workspaces.rs` (722) | `Workspace` enum: Animate, Draw, Graph, Effects, Audio, Build3D ("3D"), Export, Home; keys `ws-*`; `Section` enum + `section_visible` gating table; per-workspace blurbs. |
| Menus | `src/rune_sec/menus.rs` (1,550) | App-bar menus; File menu is genuinely 3 items (Save/Load/Export PNG) per Phase N pixel pass — that is the definition, not a scene artefact. |
| Motion | Rune `tok::motion::pair` | Only springs SNAP / SETTLE exist as the sanctioned pair (Phase O charter rule 4). |

**Kit dead code (verified Phase N, carried):** `design::text_muted`
and `design::toggle` have zero consumers (`toggle_row` is the used
form). S02 decides adopt-or-retire on evidence.

---

## 3. Ownership map (mission squads → files)

One primary owner per file. `src/main.rs` is **integrator-only**
(mission coordination rule). Shared kit files route through S02.

| Squad | Owns (src/rune_sec/ unless noted) |
|---|---|
| 01 Master audit | `docs/ui_mission/**` (this file; roadmap skeleton §5) |
| 02 Design system | `design.rs`, `icon_btn.rs`, `icons.rs`, `icon_data.rs`, theme-token surface; kit parts of `chrome_view.rs` |
| 03 Shell & layout | `main.rs` regions **via integrator only**; `shell.rs` (LayoutPrefs) |
| 04 App bar & nav | `chrome_view.rs` (app_bar/transport), `menus.rs` |
| 05 Workspaces | `workspaces.rs`, gating table; per-workspace panels stay with their feature squads |
| 06 Home & projects | `home_panel.rs`, `project_tools.rs`, `src/projects.rs` (UI seams only) |
| 07 Figures | `figures_panel.rs`, `figure_add.rs`, `inspector_figure.rs`, `layers_panel.rs`, `pose_tools.rs` |
| 08 Inspectors | `inspector_segment.rs`, `inspector_style.rs`, `stroke_inspector.rs` (with S11), stepper/slider consumers |
| 09 Timeline | `timeline_marks.rs`, `frame_ops.rs`, `playback_controls.rs`, `onion_controls.rs`, timeline parts of `chrome_view.rs` |
| 10 Stage | `src/stage_view.rs`, `stage_layers.rs`, `stage_camera.rs`, `stage_text.rs` |
| 11 Draw | `draw_tools.rs`, `stroke_inspector.rs`, `text_tool.rs`, `src/paint.rs` (UI seams) |
| 12 Graph | `graph_panel.rs`, `keyframe_tools.rs`, `easing_panel.rs`, `src/graph_keys.rs` (UI seams) |
| 13 Effects | `fx_browser.rs`, `fx_clips.rs`, `particles_panel.rs`, `stage_fx.rs` |
| 14 Audio | `audio_panel.rs`, `clips_panel.rs`, `waveform_view.rs`, `src/audio_edit.rs`/`audio_ops.rs` (UI seams) |
| 15 3D & camera | `three_d_panel.rs`, `fig3_preview.rs`, `stage_fig3.rs`, `camera_panel.rs` |
| 16 Export & ops | `export_panel.rs`, `project_tools.rs` (shared with S06 — integrator arbitrates), `src/export.rs` (UI seams) |
| 17 Icons & micro-interactions | Icon census across all files (changes via S02 kit), `toasts.rs` states, press/hover surfaces |
| 18 Accessibility | Cross-file audit; fixes routed through owning squads; `help_panel.rs`, `settings_popover.rs` (with S04) |
| 19 Visual QA | `scenes.rs`, `snap_tests.rs`, `scripts/overflow_scan.py` (to be created, p05 §2), headless debriefs — **read-only on product code** |
| 20 Integration | Gate script, CI loop, conflict resolution, acceptance record |

Non-UI files (`editor.rs`, `model.rs`, `vfx.rs`, `fig3d.rs`,
`graph.rs`, etc.) are **read-only reference** for all squads.

---

## 4. Cross-cutting defect classes (prioritised)

Severity: **S1** = controls unreachable / function lost ·
**S2** = whole surface unverified or unusable for a class of
users · **S3** = consistency/professional-finish defect ·
**S4** = cosmetic or deferred product decision.

### D1 — Horizontal overflow: controls painted off-card and off-window — **S1**

Measured, not estimated (p05, from headless run 42488647081
tree dumps; Rune does not clip, columns scroll vertically only,
so horizontal excess is **unreachable by mouse**):

| Row | File | Overflow vs budget |
|---|---|---|
| `draw-brushes` (10 chips) | draw_tools.rs | **+563** (row 879 vs 316; last chip x 1975; **6 of 10 brushes unclickable**) |
| `draw-tools-row` | draw_tools.rs | +374.5 |
| `graph-actions` (`graph-clear` ends x 1717.5) | graph_panel.rs | +305.5 |
| `layer-row-0` cluster | layers_panel.rs | ≥ +134 |
| `draw-shapes` / `draw-stamp-params` | draw_tools.rs | +132.5 / +42 |
| `wx-kind` segments (`wx-seg-5` ends 1530) | scene_panel.rs | +118.3 |
| Settings language grid (4 chips; `settings-lang-sl` +51 past content edge, +23 past window) | settings_popover.rs | +51 worst chip |
| `orbit-3d` / `cam-frame-3d` | three_d_panel.rs | +49 / +37.5 |
| `fx-nudge`, playback rows (`pb-loop-end`) | fx_clips.rs, playback_controls.rs | +27.5 / +36 |
| `pose-apply-all`, `cam-delete-key` (ends 1441.4 — past the window) | pose_tools.rs, camera_panel.rs | +16.5 / +29.4 |
| `keyframe-presets`, `graph-presets`, `keyframe-actions`, `keyframe-clipboard` | keyframe_tools.rs, graph_panel.rs | +45 / +13.5 / +11.5 / +2.8 |

≥11 rows, 9 files, one defect family. The Phase N figures-panel
fix (b637864) proved the pattern and the cure: `flex_wrap()`
first (Rune measures real text widths; Phase M frame_ops
precedent), two-line anatomy where a row has two natural
clusters. p05 items O-1…O-9 are coder-sized and key-preserving.
The repeatable scan (p05 §2 → `scripts/overflow_scan.py`) does
not exist yet — S19 creates it; it belongs to the headless
debrief, not the gate (dumps are CI artefacts).

### D2 — Light theme is shipped but unverified — **S2**

`LayoutPrefs.dark` is a real persisted toggle and builders are
theme-parameterised, but: **zero** of the 15 headless scenes run
light (no theme variant in scenes.rs), no light snap has ever
been pixel-read, and Phase N's whole verification was dark
@1440×900. Mission §7 demands equal attention; today we cannot
even say which panels break. Compounding: native file dialogs
(`src/dialogs.rs`, zenity/kdialog) follow the OS theme, not the
app's — acceptable platform behaviour, but it must be *stated*,
not discovered by users. S19's first job is light-theme scene
coverage; fixes route to owning squads.

### D3 — One fixed window size; the §6 size matrix is untested — **S2**

`AppSpec` launches at exactly 1440×900; columns are fixed
300/744/340; there are no width breakpoints anywhere in the
shell. The mission's six sizes (1280×720 … 2560×1440) have
never been rendered. At 1280 wide the fixed columns alone
(300+340+gaps) leave ≈616 for the stage before chrome; whether
Rune rescales, scrolls, or clips is **unknown from source** —
S03/S19 must establish it empirically via headless probes at
other sizes before anyone promises responsive behaviour.
Mission §6 itself forbids faking resize: investigate a real
implementation or document the limitation.

### D4 — Colour literals scattered outside the token layer — **S3**

341 `0xRRGGBB` literals in panel/chrome/stage files outside
`design.rs`/`icon_data.rs` (grep census this audit):
`stage_layers.rs` 80, `fx_clips.rs` 55, `scene_panel.rs` 43,
`stage_view.rs` 30, `layers_panel.rs` 21, `particles_panel.rs`
13, `stage_text.rs` 11. **Most are content colours** (figure/
layer colours, weather, waveform, FX gizmos) — legitimate data,
not chrome. The defect is that chrome and content literals are
indistinguishable without classification, so theme work can
silently break content. Two named sub-items:

- **Legacy accent split (documented, Phase N §8):** canvas
  accent `0x6D8CFF` is re-declared per file — `stage_view::SELECTION_ACCENT`,
  `waveform_view::WAVE_HEX`, `fx_clips::STRIP_ACCENT`,
  `timeline_marks::AUDIO_SPAN_RGB`, `stage_fx::GIZMO_COLOR` —
  alongside the kit `accent()`. Recorded as a deliberate split;
  unification is a product decision for the integrator/owner,
  not a squad restyle.
- **Brief palette ≠ shipped palette (decision item):** mission
  §3.2 proposes bg #101216, accent #6C8CFF, success #43C995,
  warning #F1B85B, error #EF6974; the shipped 2.0 palette is
  bg #111318, accent #6D8CFF, success #55C99A, warning #E8B65D,
  error #F16F78 (and chrome danger reds 0xC93A44…). The brief
  itself says "design references… centralise, do not scatter".
  S02 produces a **delta audit**; adopting any value is one
  central token change with a named decision — never per-file
  edits, never a silent swap.

### D5 — Bare empty states remain, by Phase N ruling — **S3**

Kit `empty_state(icon, message)` exists but these lines are
still bare text (Phase N §8, spot-verified in source):
`project_tools.rs:820` "No saved projects in ./projects/ yet.",
`camera_panel.rs:821` "No camera keys yet.", text_tool's two
empty messages, stroke_inspector's "No strokes on this frame.",
three_d_panel's three sub-section lines ("Enter 3D to list the
figure's joints.", "No saved 3D figures.", "No saved 3D poses."),
segment/figure inspectors' no-selection lines, fx_browser's
"No presets saved yet" line (home_panel's empty line at :1255
already carries guidance text). Each needs a **glyph decision**
from the curated table (Phase O charter rule 7) — squads
propose, S02/S17 confirm the glyph exists, nobody invents one.

### D6 — Feedback completeness is unaudited — **S3**

Phase O charter rule 6 (no silent mutations) was never
executed — Phase O stopped in planning. The mechanism exists
(`poll` classifies `core.status` into toasts via
`toasts::classify_status`), but no census of mutating handlers
vs. their traces exists. S18/S06/S17 jointly own this audit
before any panel polish wave claims "proper feedback"
(mission §5H). Until then, feedback quality is **unknown**,
not bad — the audit must not assert defects it hasn't counted.

### D7 — Contrast acceptance on the accent — **S4 (documented)**

White on kit accent = **4.19:1** (Phase N §8): fails AA normal
text (4.5), passes AA large/non-text. Standing acceptance with
a usage rule — accent fills carry short labels/icons only,
never body text. S18 enforces the usage rule; darkening the
accent to chase the number is explicitly out of scope.

### D8 — Deferred product decisions (not defects; do not "fix") — **S4**

- Two "New project" results disagree (panel blank vs Home
  demo) — owner's product call, open since Phase M.
- `tab_visits_controls` stays OFF (Phase N §8): enabling it
  would let a focused button eat Space (Play conflict) and
  sliders eat Left/Right frame-stepping. Keyboard-focus
  visibility work (S18) must be designed inside this limit and
  name what Rune cannot express.
- Range MP4/WebM exports carry no audio (labelled in-panel
  since Phase M) — engine scope, not UI polish.
- Brush scatter for non-spray strokes is model-only (Wave 12
  remainder) — any Draw UI must not imply it works.
- Sound sync is user-verified only (bounded restarts, not
  sample-accurate) — S14/S20 must not claim playback-feedback
  verification beyond this.

### D9 — Element-tree growth needs a budget per wave — **S4**

Default tree: 942 (Phase M) → 1,018 elements (Phase N, +8.1%,
inside its budget). Restyle-by-nesting is the known failure
mode. Every adoption wave in this mission states its element
budget up front; S19 measures from the tree dumps.

---

## 5. Roadmap skeleton (recommended order)

Sequencing logic: **function before finish** (unreachable
controls outrank aesthetics), **foundation before adoption**
(kit first, or 20 squads invent 20 dialects), **verification
capacity before claims** (scenes/scans must exist before
waves that rely on them). Maps to mission phases 1–10.

| # | Step | Owner(s) | Mission phase | Depends on |
|---|---|---|---|---|
| R1 | **Overflow repairs** — p05 items O-1…O-9 verbatim (flex_wrap / two-line anatomy, keys untouched) | S07, S09, S11, S12, S13, S15, S18-area files per p05 | 4/6 pulled forward | — |
| R2 | **Overflow scan tooling** — `scripts/overflow_scan.py` per p05 §2; run over every headless dispatch from now on | S19 | 1/8 | R1 for the clean baseline |
| R3 | **Token delta audit** — classify all 341 literals chrome-vs-content; brief-vs-shipped palette delta; accent-split decision paper | S02 (+S01 review) | 2 | — |
| R4 | **Scene coverage expansion** — light-theme variants + first non-1440 size probes (feasibility of AppSpec sizes is itself the deliverable) | S19 with S03 | 7 pulled forward | — |
| R5 | **Kit foundation** — interaction-state completion (rest/hover/press/selected/muted per Phase O charter rule 3), adopt-or-retire `text_muted`/`toggle`, any token changes approved in R3 | S02, S17 | 2 | R3 |
| R6 | **Feedback census** — silent-mutation audit (D6); classify_status pattern gaps | S18, S06, S17 | 6 (audit half) | — |
| R7 | **Empty states** — glyph decisions + kit adoption for the D5 list | Owning squads, glyphs via S02/S17 | 6 | R5 (glyph/kit final) |
| R8 | **Shell & chrome wave** — app bar grouping, workspace row, status strip (inside D3's fixed-layout truth) | S03, S04, S05 | 3 | R5 |
| R9 | **Feature panel waves** — squads 06–16 adopt the kit per panel, workspace by workspace; element budgets stated (D9) | S06–S16 | 4 | R5, R1 |
| R10 | **Timeline & stage wave** — transport, marks, scrub feedback, stage framing (geometry/pointer behaviour frozen) | S09, S10 | 5 | R9 patterns |
| R11 | **Micro-interaction & feedback fixes** — R6 census fixes, hover/press consistency sweep | S17 + owners | 6 | R6, R9 |
| R12 | **Theme & size verification** — light parity fixes from R4 findings; size-matrix verdicts (real behaviour or documented limitation) | S19 + owners | 7 | R4, R9 |
| R13 | **Accessibility pass** — contrast usage rule (D7), hit targets, shortcut truth (every shortcut shown in UI must be a real binding), Esc discipline | S18 | 6/7 | R9 |
| R14 | **Integration & regression** — gate green (804 keys, 0 lost), full CI loop, headless all scenes + overflow scan clean | S20 | 8 | all |
| R15 | **Pixel QA & acceptance** — integrator pixel-reads every snap (dark + light), S19 defect report, S20 acceptance vs mission §11 checklist; honest remainders written down | S19, S20 | 9/10 | R14 |

Explicitly **not** in the roadmap: replacing the engine,
new panels/features, enabling `tab_visits_controls`, changing
the two-"New project" behaviour, darkening the accent,
project-thumbnail invention (mission §S06 forbids faking it —
implement only via a real rendering path or not at all).

---

## 6. Hard rules every squad inherits

1. CI-only verification (AGENTS.md). No local cargo, ever.
2. Zero key loss vs the **804** census; keys/handlers/labels
   verbatim unless a plan item names a copy change.
3. Kit first: new visuals land in S02's kit; per-panel
   one-offs need a named reason.
4. Width budgets stated before any row changes: left 276,
   right 316, nothing crosses a card edge (D1's lesson).
5. No fake controls, no static mockups, no feature removal
   without approval (mission §2).
6. Proof named per item: Runtime test, gate check, headless
   scene/snap, or integrator pixel read. A green CI alone is
   not proof of visual quality (mission §11).
7. Report honest counts and remainders; an unverified claim
   is a defect in the report itself.
