# Redesign Progress — StykAnimate-RS 2.0

The honest tracker for the redesign program across the user
directives: the Wave 14 directive (20-agent overhaul, 7 phases), the
2.0 directive (context-sensitive workspaces; the button wall must
go), and the 2026-11-08 UI/UX transformation mission (see §6).
Updated per wave. Statuses mean: **DONE** = implemented and
CI-verified; **IN PROGRESS** = being worked in the named wave;
**PENDING** = not started; **BLOCKED-BY-USER** = cannot be completed
from this environment (see the note). No status is granted on
intention alone.

Program references: `docs/REDESIGN_ARCHITECTURE.md` (strategy),
`docs/GUI_AUDIT.md` (measured baseline), `docs/QA_WAVE14.md`
(preservation audit), `docs/UI_COMPONENT_CONTRACTS.md` (component
inventory and gaps), `docs/ULTIMATE_UI_ARCHITECTURE.md`,
`docs/WORKSPACE_DESIGN_SPEC.md`, `docs/AGENT_OWNERSHIP.md` (2.0 wave),
`docs/PHASE_N_CHARTER.md` (Rune design kit), and — the current
verdict document — `docs/ui_mission/ACCEPTANCE.md` (UI mission).

---

## 1. Definition of Done (2.0 directive §17) — status

| # | Requirement | Status | Evidence (one line) |
|---|---|---|---|
| 1 | Old button-wall interface removed from the main editing experience | IN PROGRESS (Wave 15) | Wave 14 restyled the wall but left it fully visible (see §2); Wave 15 gates the tool-group band per workspace. |
| 2 | Coherent professional design system | DONE | Wave 14: directive palette adopted as kit token values + SP/TEXT/RAD scales; CI run 42448335711 green. |
| 3 | New shell implemented | IN PROGRESS (Wave 15) | Wave 14: slim app bar + workspace selector + persisted layout; Wave 15 adds the menu bar model and the slim status strip. |
| 4 | Workspace navigation changes the actual editing environment | IN PROGRESS (Wave 15) | Wave 14 workspaces were panel/mode presets only — all other controls stayed visible; Wave 15 makes switching hide non-relevant groups and panels. |
| 5 | Animation canvas dominant | IN PROGRESS (Wave 15) | Wave 14 gave the stage a dark surround + Fit/Centre/1:1; dominance depends on item 1 (the wall still takes the top of the window). |
| 6 | Timeline usable and resizable | IN PROGRESS (Wave 15) | Chips/ruler/scrubber/dopesheet exist and are tested; Wave 15 adds a collapse toggle. Drag-resize is not implemented (GPUI limit, §4). |
| 7 | Inspector contextual | IN PROGRESS (Wave 15) | The inspector already shows the selected segment's data; Wave 15 adds a selection-context section (figure/segment/mode). Full per-object-type inspectors (audio clip, effect, stroke) are not built. |
| 8 | Drawing workspace dedicated | IN PROGRESS (Wave 15) | Draw workspace preset enters draw mode (Wave 14); Wave 15 shows only Draw/colour groups in it. |
| 9 | Character builder dedicated | IN PROGRESS | Build workspace = inspector-focused preset (Wave 14). The figure/segment tool groups are always present in the model; a separate builder screen does not exist. |
| 10 | Graph editor improved | IN PROGRESS (Wave 15) | Graph panel with channel overlays + tangent preset previews landed in Wave 13; Wave 15 gates it to the Graph workspace. |
| 11 | Effects workspace dedicated | IN PROGRESS (Wave 15) | FX panel with presets (Waves 8/13); Wave 15 adds effect categories to the browser and gates the panel to Effects. |
| 12 | Audio workspace dedicated | IN PROGRESS (Wave 15) | Audio panel with waveform + ops exists; Wave 15 gates it to the Audio workspace. |
| 13 | 3D workspace dedicated | IN PROGRESS | 3D is a session (enter/exit ops) with its own mode; the 3D workspace preset enters it (Wave 14). No separate 3D screen layout beyond the session UI. |
| 14 | Export centre organized | DONE | Format tabs (Media/Formats) since the UI waves; Wave 14 added the themed shell, tab underline rule and loop-range note; all 15 export ops preserved (QA audit). |
| 15 | Home screen professional | IN PROGRESS | Project cards, stats, duplicate, recovery surface exist (Waves 3–14); search/sort/filter/delete-with-confirmation are not implemented in the panel. |
| 16 | Command palette works | PENDING | Does not exist. `projects::filter_projects` (a search backend) exists; there is no text-input component to drive a palette (see §4). |
| 17 | Keyboard shortcuts coherent | DONE | 27 documented shortcut rows in `ui_help` (19 audited against the handler + Alt+1..8 workspace switching, Wave 14), all implemented in the single key handler. |
| 18 | Panel layouts restored and persisted | DONE | `shell.rs` LayoutPrefs (workspace + theme) in `./projects/.layout.json`, restored at startup (Wave 14); restore-default = delete the file (documented in REDESIGN_ARCHITECTURE). Panel-level geometry is not persisted. |
| 19 | Existing functionality preserved | DONE | Wave 14 QA audit: all 235 button ids present vs HEAD, zero removed control lines in the diff; 893 tests green. |
| 20 | Existing project formats remain compatible | DONE | Redesign touches no format code; the cross-format fixture corpus (incl. the original-writer text-v5 fixture) passes in CI. |
| 21 | All visible controls work | DONE | Wave 14 QA audit: every new control (view-fit/centre/actual, workspace divs) traced to a real editor/view function; zero dead controls. Re-audited each wave. |
| 22 | Localization remains functional | DONE | 28 languages × 70 keys with invariant tests (Wave 14 added the 8 workspace keys; chips still English-labelled like all buttons — recorded in i18n.rs). |
| 23 | Sanctioned CI verification passes | IN PROGRESS (per wave) | Wave 14: run 42448335711 green (893 tests). Wave 15: pending its own run at integration. |
| 24 | Real desktop screenshots inspected | BLOCKED-BY-USER | See the note below the table. |
| 25 | Agent 20 (QA) approves the final product | IN PROGRESS | QA_WAVE14 approved Wave 14's scope with the visual item open; final approval requires items 1–24, including screenshots. |

**Count: DONE 8 · IN PROGRESS 15 · PENDING 1 · BLOCKED-BY-USER 1
(total 25).**

> **Why item 24 is blocked:** this environment's headless display
> renders the GPUI window as a black frame (proven repeatedly since
> Wave 1 — Xvfb + software Vulkan produces mean-RGB 0,0,0 captures).
> Real screenshots can only come from the user's own desktop, the
> same way the first GUI verification arrived on 2026-11-08.
> `docs/QA_WAVE14.md` carries the capture checklist (8 workspaces ×
> 1280×720 / 1366×768 / 1600×900 / 1920×1080 / 2560×1440 + light
> theme). Until those captures are reviewed, no visual claim in this
> document is stronger than "CI-verified behaviour".

---

## 2. Wave 14 — what it delivered, and what it did not

Delivered (CI run 42448335711 green on `ac41149`, 893 tests, tree
57,703 lines Rust):

- Phase-1 audit documents: `REDESIGN_ARCHITECTURE.md`,
  `AGENT_TASK_MATRIX.md`, `UI_COMPONENT_CONTRACTS.md`,
  `GUI_AUDIT.md` (measured baseline: 219 `btn(` call sites, 37
  scattered colour literals in app.rs, 12 toggleable panels).
- Design tokens: the directive palette as kit values (names frozen,
  so every consumer restyled at once), spacing/type/radius scales,
  `Palette` DARK + LIGHT, `panel_shell_themed`, button variants in
  `ui/toolbar.rs`, 16 colour literals in `src/ui/` tokenized (raw
  `rgb(0x…)` outside kit.rs: 3 → 0).
- Shell: 8 workspace presets under the app bar + Alt+1..8, each
  applying real pre-existing panel/mode state; workspace + theme
  persisted via `shell.rs` LayoutPrefs.
- Restyles: tool-group band container, timeline layered surfaces,
  sidebar/inspector metrics, canvas dark surround + Fit/Centre/1:1
  camera buttons (new editor framing ops), HUD inside the frame,
  Home launcher cards, export-centre loop-range note.
- Performance: keyframe map computed once per render (was 2×).
- Preservation proven by the QA audit (item 19/21 above).

**What Wave 14 did NOT do — stated plainly:** the button wall
stayed. All 24 tool groups remained visible in every workspace;
switching workspaces changed panels and modes but never hid the
other workspaces' controls. The 2.0 directive correctly identifies
this as the core failure, and it is what Wave 15 exists to fix.
Wave 14 also did not deliver: menus (the app bar is buttons, not
menus), a command palette, timeline resizing, per-object-type
inspectors, Home search/sort/delete, or any screenshot.

---

## 3. Wave 15 scope (this wave)

1. **Tool-wall gating** — the 24-group band renders only the active
   workspace's groups (per the mapping in
   `docs/WORKSPACE_DESIGN_SPEC.md`); a Show-all toggle restores the
   full band (persisted in LayoutPrefs).
2. **Panel gating** — open panels follow the workspace (graph only
   in Graph, audio only in Audio, FX only in Effects, colour in
   Animate/Draw/Build, export centre only in Export, weather in
   Animate/Effects, clips in Animate/Effects); Home and Help stay
   available everywhere.
3. **Menus** — a pure menu model (`src/ui/menu.rs`: File/Edit/View/
   Animation/Help) rendered as app-bar popover panels reusing the
   existing buttons; no command is removed or duplicated.
4. **Timeline collapse** — a persisted collapse toggle for the whole
   bottom timeline stack (transport remains reachable via the
   status strip and keyboard).
5. **Status strip** — the bottom bar becomes a slim strip: frame/
   timecode, FPS, play state, zoom, selection summary, save state —
   replacing the plain status text.
6. **Categories** — the FX browser gains category section headers
   (the 27 kinds grouped per the 2.0 directive's categories).
7. **Performance caching** — `projects::scan` (Home) and
   `fx_preset_names` (FX panel) stop hitting the filesystem on
   every render; both are cached in the view and refreshed on the
   actions that change them (GUI_AUDIT §performance findings).

Out of scope for Wave 15 (recorded, not silently dropped): command
palette (item 16), Home search/sort/delete (item 15 remainder),
drag-resize panels, interactive slider/text-input components,
per-object-type inspectors beyond the context section.

---

## 4. Standing open items

- **Real desktop screenshots** — BLOCKED-BY-USER (item 24). The
  single largest unverified area of the whole program: every visual
  decision since the first redesign wave is CI-verified but has
  never been seen on a real display by anyone except the user.
- **Light-theme per-component adoption** — the LIGHT palette exists
  and `panel_shell_themed` consumes it (export centre does); most
  components still read the flat dark constants, so the light theme
  today recolours the app-bar/root surfaces and themed shells, not
  every panel body. Adoption is incremental by design and tracked
  here until the last flat-dark panel migrates.
- **Interactive slider and text-input components** —
  `docs/UI_COMPONENT_CONTRACTS.md` records the gap: `slider_track`
  is a read-only gauge, the scrubber is display-only, and the
  library has no text/numeric input, checkbox or dropdown
  primitive. Editing is steppers and cyclers. This blocks the
  command palette (item 16) and the directive's numeric-input
  inspector rows; building these as custom GPUI components is a
  dedicated future item, not a by-product of restyle waves.
- **Drag-resize / dockable panels** — GPUI 0.2.2 has no splitter or
  docking primitives; panels are fixed-layout with collapse
  toggles and workspace presets. Implementing drag-resize means a
  custom pointer-tracked splitter (possible, unbuilt); recorded
  here so the limitation is a decision on record, not an oversight.
  (Touch input is separately impossible on GPUI 0.2.2 — see
  `docs/TOUCH.md` — and is not a redesign item.)

---

## 5. Wave 15 — record of the wave (summary; full entry in docs/GAP-ANALYSIS.md)

Section 3 above is this wave's scope as planned. What the wave
actually is, recorded at delivery time (CI verification happens
at integration; nothing here is claimed verified yet):

- **The verdict that drove it** — Wave 14 grouped the button
  wall but left all of it visible; the user rejected grouping
  as an answer. Wave 15 render-gates instead: the tool band
  carries 4–12 of the 24 groups per workspace, panels render
  only in their workspace, and the stage takes the freed
  space.
- **Shell** — the app bar is slimmed to File/Edit/View/Help
  dropdown menu rows reusing the existing command buttons
  (ids preserved), built on new menus and status components
  in `src/ui/`; the bottom bar is a status strip showing
  project, frame, fps, zoom, selection and playback state
  (all real editor state); the timeline collapse toggle
  persists with the layout preferences.
- **Context and categories** — the inspector shows a project
  summary when nothing is selected; FX kinds are grouped
  into five categories; the audio panel is sectioned with a
  real summary line; export rows are categorised (Images /
  Animation / Video / Project formats); the graph panel
  gains per-key Hermite/Linear/Step interpolation buttons.
- **Performance and palette** — the Home and FX per-render
  disk scans are now mtime-keyed caches (the GUI_AUDIT
  scorecard item); the kit palette is refined to the 2.0
  directive (accent `#6D8CFF`, error `#F16F78`, plus new
  hover, success, warning and tertiary-text tokens).
- **Preservation** — all 235 button ids audited in Wave 14
  remain present (checked in `docs/QA_WAVE15.md`); engines,
  formats and all 65 MCP tools are untouched.
- **Still open** — everything in section 4 stands, headed by
  real-desktop screenshots: the wall being down is proven in
  code, not yet seen on a display.

---

## 6. The UI/UX transformation mission (2026-11-08) — CLOSED

Sections 1–5 above describe the GPUI-era program. Between them and
this section, the shell was rebuilt on Rune (migration Phases A–H)
and Phase N put the whole Rune shell on one design kit
(`docs/PHASE_N_CHARTER.md`, "card + hairline"). The mission below
started from that baseline — an 804-key census (ACCEPTANCE.md,
criterion 5) — so the §1 table's statuses are historical, not
current. The mission's own verdict document is
`docs/ui_mission/ACCEPTANCE.md`; this section is the progress
record, and every figure in it traces to that review or to the
gate script (`scripts/phase_j_checks.py`), with the source named.

### 6.1 What the mission changed, wave by wave

(Condensed from ACCEPTANCE.md, "What the mission changed".)

- **Phase 1** — 20 squad audits (`docs/ui_mission/audit/s01..s20`).
  Headline finding: one defect family — controls overflowing their
  cards (22 rows tree-proven, worst +563 pt) — outranked every
  cosmetic item.
- **Wave 1** — all 22 overflow rows repaired across 16 files;
  `scripts/overflow_scan.py` kept as the standing guard.
- **Wave 2** — the brief's palette centralised in one app-side
  constructor (`src/rune_sec/app_theme.rs`); kit press/hover/inert
  states; `design::empty_state` learned to wrap (the root fix for
  two residual overflows).
- **Wave 3** — command palette anchored as an overlay (it had
  squeezed the stage to height 0); toasts docked inside the stage;
  status-strip yield ladder for narrow windows; menus deduplicated
  onto the app bar; undo/redo inert on empty stacks.
- **Wave 4** — Home project thumbnails + rename; discard
  confirmations on project-destroying actions; export notes made
  truthful about audio; audio scope split + clip-health badges;
  graph interpolation edits the channel the graph draws; tangent
  presets converged on one kit chip; camera Clear→Delete (undoable).
- **Wave 5** — clips lane moved inside the timeline scroll (one
  shared offset; overlapping clips stack into sub-lane rows); loop
  controls reduced to one home (the timeline row); stage boundary
  drawn inside the stage; **view zoom** — the mouse wheel now zooms
  the view instead of rewriting the project's camera track.
- **Live MCP** (user-inserted priority, between Waves 5 and 6) —
  the running editor itself is now drivable over MCP: the stdio
  server's dispatch was parameterised over an external core
  (`handle_on`), and a loopback-only HTTP bridge
  (127.0.0.1:32117, no auth in v1, settings toggle) feeds it the
  live editor core; end-to-end socket test in CI.
- **Wave 6** — tooltips by construction (`.help()` in every button
  builder; all 37 icon-only sites truthfully labelled); last bare
  empty states converted to the kit; `classify_status` refusal
  grammar (22 status strings Info→Warn); timeline chip press
  paint; waveform clip-health caption.
- **Wave 7** — light-theme scenes (`default-light`,
  `settings-open-light`) and 1280×720 headless probes; toast
  severities and the graph glyph unified onto the one palette;
  status-strip fit at 1280×720 proven from tree dumps.
- **Wave 8** — the two defects Wave 7's proof exposed: the status
  strip was invisible in the light theme (it had hard-coded the
  upstream dark theme — now a `strip_colors` seam on the app
  theme), and opening a menu grew the stage by 24 pt (menu layers
  now mount only while open, behind permanent in-flow spacers;
  closed geometry byte-identical, open geometry equal to it).

### 6.2 Final counts at mission close

| Figure | Value | Source |
|---|---|---|
| Tests | 2,584 (999 lib + 1,575 bin + 10 integration), 0 failed | ACCEPTANCE.md crit. 6; CI run 42499569292 |
| Headless probes | all exit 0; overflow scan CLEAN on every tree | ACCEPTANCE.md crit. 2/6; headless run 42499571269 |
| Distinct element keys | 809 (804 baseline + additive; retirements recorded in ACCEPTANCE.md crit. 5) | ACCEPTANCE.md crit. 5; gate check 2 per wave |
| Headless scenes | 17 (15 at Phase N close + 2 light-theme scenes, Wave 7) | `scenes()` in `src/rune_sec/scenes.rs` at HEAD; gate check 5 |
| Command palette commands | 108 | gate check 5 at mission close |
| MCP tools | 91 registry tools; 102 total incl. server built-ins | gate check 5 (registry); total per project record since Phase M |
| Curated icons | 163 | gate check 5 (`ICON_NAMES`) |
| Localisation | 78 keys × 28 languages, census unchanged | ACCEPTANCE.md crit. 10; gate-verified |
| Gate | 6/6 checks PASS | `scripts/phase_j_checks.py` |

Honest agent count: ≈68 assignments (20 auditors + ~48 coders)
against the brief's ordered 100 — recorded in ACCEPTANCE.md; the
count was never padded.

### 6.3 Remainders carried out of the mission

(From ACCEPTANCE.md, "Honest remainders".)

- White-on-accent contrast measures **3.14:1** as rendered; the
  brief's accent was kept as the user's latest word, and darkening
  it toward ~4.5:1 is a **user decision** (also §4's contrast
  thread, now measured rather than estimated).
- Panel resizing (no Rune splitter exists; a hand-built drag
  splitter is a feature build) and File-menu Save As (needs a core
  op that does not exist) remain **user decisions** — §4's
  drag-resize item carries into the Rune shell unchanged.
- The status strip's deepest coincidence state (audio + 1.5×
  speed + 3D) carries a ≈43 pt *estimated* residual at 1280×720;
  it cannot be produced headlessly, so the yield ladder was not
  tuned to an estimate.
- Home thumbnails are unit-proven but have no headless snap (no
  Home scene exists). Sound remains user-verified only.
- §4's light-theme adoption item is **closed** by this mission:
  panels read the app theme (Waves 2/7/8 retired the last private
  palettes — waveform, toasts, graph glyph, status strip).
- §1 item 16 (command palette) is **closed**: the palette exists
  and is overlay-anchored (Wave 3), 108 commands at close.
- §1 item 24's block is superseded for the Rune shell: the shell
  was screenshot-verified on the user's PC during the migration,
  and the mission's visual proof is the headless snap/tree suite,
  pixel-read per wave (ACCEPTANCE.md crit. 7).

### 6.4 Wave 9 — integrated 2026-11-08

A 50-agent improvement wave ordered by the user on 2026-11-08
(22:27 +06), after the mission close: five squads of ten (shell,
panels, timeline/stage/export, engine/formats, QA/tooling),
one owner per file, all fifty reports collected (five agents
lost to a startup rate-limit burst were re-dispatched).
Integrated by the session integrator and pushed as five
commits on top of the acceptance commit. Final gate: 7/7
(the wave added check 7, the literal & empty-state guard),
reading **827 total unique keys** (zero lost vs the mission
close; new keys are the two splitters, `save-copy-proj` and
`draw-stamp-params-3`), **112 palette commands**, **131 MCP
tools** (91 registry + 8 io + 32 built-ins in `mcp.rs`),
**23 headless scenes**, **96 i18n keys × 28 languages**,
help at 56 rows. Real defects the wave found and fixed
include clip rows overflowing their card by up to ~188 pt
(invisible for eight waves because no scene populated the
clips list — a `clips-populated` scene now covers them),
the Export panel's rows overflowing (the workspace was
never tree-dumped; it is in the headless loops now), the
waveform strip laying out 0 pt high in project mode, and
range MP4 exports gaining frame-accurate WAV audio. The
CI and headless verdicts for the wave's tree are recorded
in §3's table when they land; nothing here claims them
early.

### 6.5 Wave 10 — dopesheet + engine depth + panels round 2 (2026-11-09)

A follow-on improvement wave ordered by the user on
2026-11-09 (04:00 +06), in the same five-squad shape as
Wave 9: 50 agents, one owner per file, integrator wiring
the shared files. Its headline item is the **dopesheet
rebuild**: a Wave 9 audit found that no dopesheet exists
in the Rune shell — the GPUI shell's per-figure key rows
(`ui_dope::keyframe_map` / `dopesheet`) were lost in the
GPUI→Rune migration and never rebuilt, leaving only the
timeline's merged camera/graph marker row. The user
approved rebuilding it on 2026-11-08, and the wave builds
it as a lane inside the Animate timeline card: one row
per figure (selected first, capped at 4 + a "+N more"
line), keyframe diamonds from the surviving engine
helper `stats::figure_keyframes`, click a cell to go to
that frame and select the figure, sharing the frame
chips' scroll and pitch (the name gutter offsets the
lane's columns by a constant 124 pt — measured exactly
in the headless tree, disclosed in `docs/DOPESHEET.md`).
The research, the original app's sheet design, the gap
analysis and the landed-vs-planned table live in
`docs/DOPESHEET.md`. The engine grew the keyframe ops
the sheet implies (`delete_keyframe` = hold the previous
pose, `goto_next/prev_keyframe`, palette + MCP surface).

Also landed: `Stroke.scatter` wired end to end (model
field, `stroke_dabs` placements shared by the stage,
the raster exporter and the SVG exporter, MCP
`draw_stroke` arg — it was model-only since Phase M);
two engine panics closed (`add_frame` on a zero-frame
project; `copy_pose_between` across different rig
sizes) and a third found by the wave's own new test
during integration (onion skins on a figure absent
from a neighbouring frame); FX presets can no longer
silently overwrite a different preset; the dirty
baseline now resets on save/swap so "N edits since the
last recovery snapshot" means what it says; a clip edit
session can no longer be silently retargeted by
undo/redo; non-WAV range-export audio; live MCP
hardening (read deadline, /health identity); the
settings language rows' true overflow root cause
(flex_wrap without w_full — every row overflowed, worst
+127 pt) fixed; home/library row restructures; the
figures list bounded; the segment angle readout's
wrong convention fixed; gate check 8 (scene ↔ headless
coverage) and overflow-scanner rule 4 (timeline-card
bounds) added.

**Verification**: gate 8/8 PASS (zero keys lost vs the
Wave 9 commit; the recalibrated census reads 894 —
787 static + 107 dynamic patterns on the gate's
57-file basis). CI run 42531689025 GREEN on 6138daf:
**2,852 tests (1,079 lib + 1,763 bin + 10
integration)**, up from 2,683. Headless run
42532480879: all probes exit 0; **overflow scan CLEAN
on all 27 trees** — Wave 9's one hit (the project
autosave line at edits ≥ 1) is gone. Pixel pass on the
new snaps: the dopesheet lane reads cleanly (two rows,
diamonds on the authored keys, selected row + current
column tinted); stage measures 744×504 at 1440×900
(the lane's net cost, pinned in the snap tests) and
584×324 at 1280×720 — the lane caps at 4 rows, so the
small-window stage stays workable without a collapse
toggle. Inventory: palette 114, MCP 140 (91 registry +
8 io + 41 built-ins), scenes 24, help 57, i18n 99 × 28,
icons 163.

Honest remainders: the stage still ignores style
cap/taper/opacity that both exporters honour (scoped
as the Wave 11 "stage style fidelity" headline); the
dopesheet columns sit a constant 124 pt right of the
chips (the name gutter — pitch and scroll are shared,
the offset is constant); white-on-accent contrast is
still the user's open decision (the E7 evidence pack
in `app_theme.rs` shows the carrying value is upstream
`tok::accent` #4392F7 at 3.14:1 — darkening the app
token alone would fix nothing); sound remains
user-verified only; Home project rows are still not
pixel-proven (no populated-Home scene yet).

### 6.6 Wave 11 — MCP end to end (2026-11-09)

A fifth improvement wave ordered by the user on
2026-11-09 (06:00 +06): "assign 50 agents to continue
improving projects mcp and mcp control end to end
point to point not miss a single things". Same shape
as Waves 9–10: 50 agents, five squads of ten, one
owner per file, the session integrator wiring the
shared files. The wave's single subject is **MCP
control, end to end**: every path by which an outside
client drives this editor — the protocol router, the
stdio binary, the live HTTP endpoint, the tool
registry, and the GUI surface that turns the live
endpoint on — audited and closed point to point, so
that nothing the editor can do is unreachable, and
nothing a tool claims is untrue.

The squads: **A, the live path** — `live_mcp.rs`
protocol and hardening, the `mcp.rs` dispatch and
built-in census, the `main.rs` poll drain and redraw,
prefs and port state, the stdio binary and
`mcp_io.rs`, the Settings toggle's truth, toast and
status attribution for MCP-driven edits, the
`docs/MCP.md` rebuild and the help rows. **B,
registry parity** — missing tools written by domain
(handlers in the domain files; the integrator splices
the `Tool` entries into `mcp_tools.rs::tools()`).
**C, panel parity** — ten panel owners auditing
panel↔MCP reachability, each producing a gap spec for
the integrator rather than editing the registry
itself. **D, end-to-end session tests** — ten
integration suites under `tests/wave11_e2e_*.rs`
driving the real dispatch through whole editing
sessions, with `WAVE11-GAP` markers wherever a session
step finds the surface wanting. **E, QA/tooling** —
a gate census of the MCP surface (check 9 and
`scripts/mcp_census.py`), a headless live-MCP probe,
snap coverage of MCP-visible state, an MCP-
reachability panic audit of the engine, and this
section.

Entering surface, counted at source at the wave's
start (not carried over from §6.5's prose):
`mcp::all_tools()` is the single registry every
consumer goes through, and it held **140 tools — 91
registry** (`Tool` entries in
`mcp_tools::tools()`), **8 io** (`Tool` entries in
`mcp_io::tools()`: save/load/import/export/render,
frame and contact-sheet rendering, project summary)
**and 41 built-ins** in `mcp.rs` itself (the 3 base
tools, 29 Wave 9 parity built-ins and 9 Wave 10
round-2 built-ins). The one named gap entering the
wave: **`FxInfo` was blind to `hidden`** — the FX
panel's Hide/Show toggle sets `FxClipEntry::hidden`,
the stage and exporters honour it, but the info
struct the MCP `fx_list` tool serves had no such
field, so no MCP client could read the flag (Squad B
owns the fix).

**Verification** (run evidence only, gathered at
close): the pre-commit gate passes **9/9** on the
closing tree, including the new check 9 MCP census —
**277 tools** (224 registry + 9 io + 44 built-ins),
zero duplicates, zero unregistered or orphan
handlers, and `docs/MCP.md`'s catalogue in exact
two-direction sync; **894 unique keys, zero lost**
vs the Wave 10 close. CI run **42548006376** on
`d3f77ba` is **GREEN: 3,033 tests passed, 0 failed**
(Wave 10 closed at 2,852). Getting there took 14
fix rounds after the first integration push — two
borrow errors, a `&&str` map, a missing `PartialEq`
derive, and the rest pre-wave tests reconciled with
this wave's deliberate contracts (the undo-step
ruling, `set_node` clamping, `add_audio_clip` file
validation, `clear_graph` snapshotting); two snap
assertions were also made sound per the standing
presented-box doctrine. One honest discovery from
those rounds: cargo's fail-fast had silently skipped
the D-suite binaries behind earlier failures in
several runs, so the undo/selection suite's first
real CI execution came late — its two wrong entering
expectations (add-figure node clearing, the f32
rest root) are now pinned to the true contracts.
Headless run **42548380654**: every probe exit 0;
the **live-MCP probe PASSES end to end** (health,
`tools/list` == health count, an `add_frame` mutation
visible in a fresh `get_state` and in the live undo
history); the **overflow scan is CLEAN on all 27
trees**. End-to-end suites: **10 suites, 39 tests**,
all green; the 8 remaining `WAVE11-GAP` tokens in the
tree are historical wording in comments (what *was*
missing), not open gaps. Closing census, as counted
by the gate: MCP **277** (224 + 9 + 44), palette
**114**, scenes **24**, help **58** (one row added),
i18n **99 × 28**, icons **163**, keys **894**. The
entering gap is closed: `fx_list` now serves the
full `FxInfo`, `hidden` included. Snaps from the
headless run are in `your_files/stykanimate-wave11-*.png`.

Honest remainders — standing items this wave does
**not** close (checked against the five squad remits
above; none of them covers these): the **stage style
fidelity** gap §6.5 named as a headline candidate —
the stage still ignores style cap/taper/opacity that
both exporters honour — is not an MCP item and no
squad's remit touches stage rendering, so it carries
over untouched (it is now also disclosed in
`docs/MCP.md`'s notes); **sound remains
user-verified only** (no squad can listen to
playback); the **Home workspace is now snapped**
(`snap-home`), but with an empty projects dir — **rows
and thumbnails of a populated Home are still not
pixel-proven**. Also standing, from §6.5: the
dopesheet columns' constant 124 pt gutter offset,
and white-on-accent contrast, which remains the
user's open decision.

Wave 11's own remainders, named at close: the
**sprites/assets subsystem has no MCP surface** and
was not audited this wave (E7 deferred it — no panel
audit, no tools); the **core project-swap contracts
are inconsistent** (load clears undo but not redo,
`.piv` import clears both, New/open clear neither) —
unifying them is a product call for the user, not a
fix a wave should smuggle in; the **`project_files`
cache is not refreshed when an MCP save creates a
file** (A5 I3 — the saved-projects list can lag until
the next GUI refresh); the **panel trims audio in
seconds where MCP's indexed trim also speaks
seconds** — verified consistent, but the panel's own
seconds-trim has no frame readout (cosmetic);
`set_loop_range` with an out-of-order range answers
Ok with a null range (cosmetic); the GUI's
follow/tween toggles are non-undoable session state
(mirrored by their tools, deliberately); MCP-driven
edits surface in the editor as toasts with **at most
one tick of latency**, and multiple drain errors in
one tick report the last status only; the **SVG
exporter still ignores figure opacity, draws bends
straight, loses taper without a gradient, and emits
no media/text/sprite content** (E8's design gaps,
documented, untouched); `STYLE_PRESETS` remains
duplicated between `style.rs` and the panel table,
guarded by a pin test rather than unified.

### 6.7 Wave 12 — stage style fidelity + Wave 11 remainders (2026-11-09)

A sixth improvement wave, launched 2026-11-09
(08:52 +06) from the proactive Wave 12 offer: the
user tapped "Launch Wave 12 — all 50 agents as
planned" rather than answering the two standing
decisions first. Same shape as Waves 9–11: 50
agents, five squads of ten, one owner per file, the
session integrator wiring the shared files (roster:
`~/workspace/wave12-roster.md`). The scope, as
offered: the **stage style fidelity** headline §6.6
carried over — the stage honouring the style
cap/taper/opacity both exporters already respect;
a **sprites/assets MCP surface** where none exists,
plus the `project_files` cache refresh Wave 11's A5
I3 named; **SVG exporter fidelity** (figure opacity,
bend arcs, taper without a gradient, media/text/
sprite emission) and the **`STYLE_PRESETS`
unification**; **pixel-proof coverage** — a
populated-Home scene and a strokes-populated scene,
so Home rows/thumbnails and the stroke inspector
are proven in headless trees, not just unit-tested;
and **QA/cosmetics** — `set_loop_range` erroring on
an out-of-order range instead of answering Ok with
a null range, the audio panel's seconds-trim
gaining a frame readout, and gate/census upkeep.

The squads: **A, stage style fidelity** — shared
styled-segment math in `paint.rs` (bend split,
gradient and taper lerps, caps, alpha), adopted by
`stage_view.rs` and refactored onto by `export.rs`;
one effective-style resolver in `style.rs`; the
strokes layer's single alpha model; segment text
and media following styled segments; an MCP truth
audit of node/style replies against what the stage
now draws; and the field × surface matrix in
`docs/STAGE_STYLE.md`. **B, sprites/assets MCP** —
asset handlers in `media.rs` (list, info, import,
remove, rename) with panel parity across the
inspector, scene and export panels; the
`projects.rs` refresh seam for the `project_files`
cache, wired into `editor.rs` save paths; asset
integrity across swaps and undo; `.piv` and Stick
Nodes asset truth; and an end-to-end asset session
suite. **C, SVG fidelity + presets** — `svg.rs`
figure opacity, taper emitted as a filled path,
bends as arc paths; the panel's duplicate preset
table deleted in favour of `style.rs`'s
`STYLE_PRESETS`; serde census pins in `model.rs`;
palette parity; and anchored specs for media/text/
sprite and stroke-width-profile SVG emission. **D,
pixel-proof coverage** — populated-Home correctness
in `home_panel.rs`; two new scenes (home-populated,
strokes-populated) taking scenes 24 → 26; the
stroke inspector populated and proven; both scenes
in the headless tree/snap loops; library and
figure-card thumbnail audits; save-time thumbnail
generation; and the coverage ledger in
`docs/wave12/PIXEL_PROOF.md`. **E, QA/tooling** —
gate check 8/9 and census upkeep for the new counts,
`set_loop_range` erroring on out-of-order ranges,
the audio trim's frame readout, snap coverage of
the new state, live-MCP swap invalidation, toast
attribution for the asset tools, overflow rules for
the new tree shapes, and this section.

Entering counts, as recorded at the Wave 11 close
(§6.6) and re-stated here so the closing census has
its baseline: MCP **277 tools** (224 registry + 9
io + 44 built-ins, per `scripts/mcp_census.py`);
**3,033 tests** (CI run **42548006376** on
`d3f77ba`, GREEN); **894 unique keys**, zero lost;
palette **114**; scenes **24**; help **58**; i18n
**99 × 28**; icons **163**; gate **9/9**
(`scripts/phase_j_checks.py` on the closing tree);
headless run **42548380654** (all probes 0, live-MCP
probe PASS, overflow CLEAN on 27 trees). HEAD at
launch: `3f04667`, the Wave 11 close-out commit.

**Delivered** — all five squads landed and were
integrated the same day (commits `ccecd85`..`1461623`,
plus CI fixes `4451172` and `5a8403f`). **A (stage
style fidelity):** `paint::styled_segment_pieces` is
now the single styled-segment construction — the
stage figure pass, the raster exporter and both
thumbnail renderers build from it; onion ghosts carry
styled pieces with per-piece alpha composed (never
replaced); segment-media carries figure × style
opacity into the compositor; the stage draws an
honest placeholder outline where a segment image
sits (Rune `Shape` has no bitmap primitive — real
PNG parity on stage is impossible, named as such).
**B (assets MCP):** six tools spliced (registry 224
→ 230): `assets_list`, `asset_info`, `asset_import`,
`asset_rename`, `asset_remove` (hybrid rule — refuse
while the target is referenced, remap references
above it), `asset_attach`; `get_node` gained its
`media` member; the segment inspector gained a real
Media section; `projects::refresh_project_files`
became a free seam wired into the MCP save paths,
the Home duplicate path and `save_via_dialog`;
`clear_node_media` resolves its selection before
spending an undo step; library inserts strip segment
images the library format cannot carry, and say so.
**C (SVG fidelity):** figure opacity per element,
bends arced through the bend midpoint, solid tapers
as quads, media emitted (images embedded once per
asset as PNG defs with placement matrices, text as
escaped centred `<text>`), pressured strokes as
per-segment lines at baked widths; `STYLE_PRESETS`
is one canonical table (the panel re-exports it —
the duplicate was value-identical, no live bug);
palette 114 → 132 (fifteen style commands plus
`attach-media-import`, `clear-node-media`,
`cycle-attached-sprite`). **D (pixel-proof):**
scenes 24 → 26 — `home-populated` seeds three real
project files through the app's own save path,
`strokes-populated` plants four stroke kinds on the
Draw workspace; library rows gained real per-entry
thumbnails (there was NO thumbnail before this wave
— a premise correction, recorded in
`library_panel.rs`); the File menu's recents now
resolve like Home's (a renamed project's entry was
dead). **E (QA):** live-MCP queue bounded with
cancellation — a request whose caller timed out is
discarded at drain instead of executing late against
a swapped project; `copy_frame` / `set_frame_easing`
refuse honestly on an empty project; audio readouts
carry frame spans beside seconds; snap tests gained
Round 13. Two further premise corrections stand as
recorded: `set_loop_range` endpoint ordering is
deliberate, test-pinned timeline behaviour (not a
bug to fix), and the Scene panel has no sprite
surface — B5's full sprite-GUI spec is the headline
remainder below, not built this wave.

**Verification** — gate **9/9 PASS** on the closing
tree (`scripts/phase_j_checks.py`, full foreground
run). CI run **42557475828** GREEN on `5a8403f`:
**3,156 tests passed, 0 failed** (1,256 lib + 1,838
bin + 62 integration across 16 binaries, incl. the
new 6-test asset e2e suite
`tests/wave12_e2e_assets.rs`, flipped strict against
the shipped reply shapes). Headless run
**42557913977**: every probe exit 0 (the xvfb
fallback step was skipped — the plain attempt missed
nothing), **live-MCP probe PASS** end-to-end,
**overflow scan CLEAN (0 hits) on all 29 trees**.
Both new scenes are pixel-proven: the
home-populated snap shows three real project rows
with rendered thumbnails; the strokes-populated snap
shows all four stroke kinds with the stroke
inspector mounted (snaps:
`stykanimate-wave12-{default,stage,home-populated,strokes-populated}.png`).

**Closing census** — MCP **283 tools** (230 registry
+ 9 io + 44 built-ins; entering 277); palette
**132** (entering 114); scenes **26** (entering 24);
help **61 rows** (entering 58); keys **923 total
(814 static + 109 dynamic patterns), zero lost**
(entering 894 gate-basis); layout facts **29**;
icons **163** (unchanged); i18n **99 × 28**
(unchanged); gate **9/9** (entering 9/9).

**Honest remainders** — (1) **No sprite GUI surface
exists**: sprite instances arrive only via `.nodes`
import and render in exports (plus stage placeholder
outlines); B5's full spec (engine ops + a sprites
panel) is written in `docs/wave12/` and is the
headline candidate for a future wave. (2) The
**snapshot-coverage gap** (E8): `edit_count` bumps
only in `snapshot()`, so figure rename/recolour,
layer writes, clip angle/root writes and the MCP
project-rename name write never mark dirty — the
dirty dot under-reports and the 25-edit autosave
cadence can miss real changes; the fix changes undo
semantics of 5+ tools and needs its own decision.
(3) SVG standing divergences: strokes draw UNDER
figures in SVG but over them in the raster (z-order),
SVG text is a font run where the raster stamps
bitmap glyphs, and node opacity does not reach
heads/rings/polys in raster or SVG. (4) Stage pressured
strokes keep the flat-width construction (SVG and
raster honour baked widths). (5) The figures panel's
own thumbnail ignores per-node styles (library
thumbnails now lead it in fidelity). (6) The library
format carries no image data, so inserts drop
segment images (with a status saying so). (7) D5's
library-populated scene was not built — library
thumbnails are unit-proven, not yet pixel-proven.
(8) `redo` lacks the `selected_figure` clamp `undo`
has (skipped by decision). (9) Sound remains
user-verified only.

Standing user decisions this wave carries but does
**not** settle — none of them is in any squad's
remit, and they are recorded here so they are not
lost: the **project-swap undo canonical behaviour**
(load clears undo but not redo, `.piv` import clears
both, New/open clear neither — a product call, not
a fix); **white-on-accent contrast** (the
white-text carrier is upstream rune `tok::accent`
**#4392F7 at 3.14:1** in both themes; darkening the
accent-filled call sites to ~**#2072D5 (4.72:1)**
fixes it, darkening the app token alone does not —
the user's open decision since the Wave 10
close-out); the **two "New project" results** that
still disagree; the **C5 waveform call** (a deleted
audio file's stale waveform vs a "(missing file)"
badge); and **sound remains user-verified only** —
no squad can listen to playback.

The wave works under two exclusions, stated plainly:
**no project-swap undo/redo semantics change**, and
**no accent colour change**. The user chose "launch
as planned" over "answer the two decisions first",
so both stay exactly as they are until the user
rules on them.

### 6.8 Wave 13 — sprite surface + Wave 12 remainders (2026-11-09)

A seventh improvement wave, launched 2026-11-09
(09:57 +06): the user tapped "Launch Wave 13 —
sprite surface + Wave 12 remainders". Same shape
as Waves 9–12: 50 agents, five squads of ten, one
owner per file, the session integrator wiring the
shared files (roster: `~/workspace/wave13-roster.md`).
The scope, as offered: the **sprite GUI surface**
headline §6.7 carried as its first remainder —
engine ops, stage prims, a Sprites panel and MCP
tools for the instances that today enter only via
`.nodes` import and cannot be placed or edited in
the app at all; the **dirty-marking fix** for
§6.7's snapshot-coverage gap, under a census
contract ruling every non-snapshot mutator; the
**fidelity remainders** — SVG strokes drawing
under figures where the raster paints them over,
node opacity never reaching heads/rings/polys in
either exporter, stage pressured strokes keeping
the flat-width construction, and the figures
panel's thumbnails ignoring per-node styles;
**pixel-proof coverage** — a library-populated
scene (the library thumbnails are unit-proven
only) and a sprites-populated scene, taking scenes
26 → 28; and **QA/parity** follow-through across
the strip, palette, toasts and workspace gating.

The squads: **A, sprite GUI surface** — engine ops
in a new `sprite_ops.rs` (add/remove an instance,
set/delete a transform key, exactly one snapshot
per mutation, status-only refusals); stage prims
and hit-testing in a new `stage_sprites.rs` (the
honest placeholder family — Rune `Shape` has no
bitmap primitive, consistent with Wave 12's
segment-media outlines); a new Sprites panel
mirroring the text tool's pattern; five MCP sprite
handlers in `media.rs` for the integrator's
registry splice, with a `media_items` ordering
audit against the raster composite order; a
`.nodes` sprite round-trip audit in
`nodes_anim.rs`; sprite name labels in
`stage_text.rs`; the Scene panel's placed-media
readout becoming the panel's entry point; help
rows rewritten to the landed truth; per-format
sprite honesty notes in the export panel; and the
surface doc `docs/SPRITES.md`. **B,
dirty-marking / snapshot coverage** — a census
(`docs/wave13/DIRTY_CENSUS.md`) ruling every
non-snapshot mutator in the lib + bin snapshot vs
the new `mark_dirty()` seam (dirty + autosave
counting without an undo step, where undoability
would be wrong); the seam in `editor.rs` beside
`edits_since_autosave`, plus the `redo`
`selected_figure` clamp as a parity bug fix;
per-op fixes across `figure_ops.rs`, `fx_ops.rs`,
`projects.rs`, `audio_ops.rs`, `graph_keys.rs`,
`draw_ops.rs` and the handlers in `mcp_tools.rs`
that write project state directly; and an
end-to-end suite pinning the contract through real
MCP dispatch. **C, fidelity remainders** — SVG
z-order (strokes over figures, media last) and
node `style.opacity` reaching SVG heads/rings/
polys; the raster half of node opacity in
`export.rs`; stage pressured strokes honouring
baked per-point widths via a new shared
`paint::pressured_stroke_pieces`; figure-card
thumbnails rebuilt on `styled_segment_pieces`;
serde pins and `InstanceKey::sample` edge tests
in `model.rs`; the frame insert/delete audit for
sprite/text instance keys in `frame_swap.rs`;
live-MCP hardening follow-through against the new
write paths; a `docs/STAGE_STYLE.md` close-out
addendum; and a sprite MCP end-to-end suite.
**D, pixel-proof + scenes** — library-populated
and sprites-populated scenes (scenes 26 → 28,
layout facts 30/31); both scenes in the headless
tree loops and snap probes; snap tests Round 14
(sprite panel flow, library thumbnails present);
library viewport follow-through for the taller
thumbnail rows; missing-file recents entries
dimmed in `menus.rs`; gate, census and
overflow-scan upkeep; and the wave's proof ledger
and acceptance checklist in `docs/wave13/`. **E,
QA + parity** — the status strip's dirty-dot and
autosave read path verified against the B
contract; the GUI save-path audit (every
`after_save` path calling `note_recovery_written`);
palette sprite commands (two landed of three
scoped — `next-sprite` dropped in E3's audit;
palette 132 → 134);
the style panel's closure-only controls extracted
to named fns in `inspector_style.rs`; the
movieclip-def media audit (`docs/wave13/CLIP_MEDIA.md`
— clip-def media is stored and serialised but
never rendered); the `shell.rs` root-clone test
oddity investigated from source; toast attribution
for the sprite and dirty families; workspace
gating for the Sprites panel; the final honesty
sweep (`docs/wave13/HONESTY_SWEEP.md`); and this
section.

Entering counts, as recorded at the Wave 12 close
(§6.7) and re-stated here so the closing census
has its baseline: MCP **283 tools** (230 registry
+ 9 io + 44 built-ins, per `scripts/mcp_census.py`);
**3,156 tests** (CI run **42557475828** on
`5a8403f`, GREEN — 1,256 lib + 1,838 bin + 62
integration); **923 unique keys** (814 static +
109 dynamic patterns), zero lost; palette **132**;
scenes **26**; help **61**; layout facts **29**;
i18n **99 × 28**; icons **163**; gate **9/9**
(`scripts/phase_j_checks.py` on the closing tree);
headless run **42557913977** (all probes 0,
live-MCP probe PASS, overflow CLEAN on 29 trees).
HEAD at launch: `374e873`, the Wave 12 close-out
commit.

**Delivered** — **A, sprite surface:** engine ops
(`src/sprite_ops.rs` — add/remove instance, set/delete
key; one snapshot per mutation, frame clamped to the
timeline), stage prims (`src/stage_sprites.rs`) painted
by a new `stage_view` pass (the generic placeholder
skips sprite items; fills/outlines at the family's
alpha factors, corner marks on the panel-selected
sprite, name labels via `stage_text::sprite_label_boxes`),
the Sprites panel (`src/rune_sec/sprites_panel.rs`,
Animate only, `Section::Sprites`), five MCP sprite
tools (registry 230 → 235), `.stknds` export now
carries the sprite library + type-2 instances
(round-trip pinned; groups return as member images,
texts still dropped). **B, dirty-marking:** the
`mark_dirty()` seam in `editor.rs` + a 31-row census
(`docs/wave13/DIRTY_CENSUS.md`) executed across the
core, the MCP handlers and the panels — including
the integrator rows (camera clear-key, clip pose
tweaks, the `begin_draw` snapshot a hand-drawn
stroke never had, `clear_frame_drawings` restructured
to own its snapshot, weather/stage/background panel
marks, slider drags marking once per gesture) and
the `redo` `selected_figure` clamp. **C, fidelity:**
SVG z-order now matches the raster (figures, strokes,
media last) and node opacity composes on heads /
rings / polygons in BOTH exporters (S10 + S21
closed); stage pressured strokes render per-segment;
figure-card thumbnails rebuilt on the shared styled
generator. **D, pixel-proof:** scenes 26 → 28
(library-populated, sprites-populated), layout facts
30/31, Round 14 snap tests, library list five-row
viewport. **E, QA/parity:** status-strip read path
pinned against the dirty contract; toast classifier
extended so MCP handler refusals (sprite + asset
import families) read Error, never Info; clip-def
media audit (`docs/wave13/CLIP_MEDIA.md` — stored,
counted, removal-blocking, but never rendered; fix
spec anchored for a future wave).

**Verification** — Gate **9/9 PASS** on the closing
tree (`scripts/phase_j_checks.py`). **CI run
42562084573 GREEN on `ac47b99`: 3,319 tests passed,
0 failed** (1,337 lib + 1,911 bin + 71 integration;
three fix rounds: a `Shape::Line` variant path, the
sprite handler tests' one-frame fixture, one
presence-settle assert). **Headless run 42562380603
SUCCESS: all 58 probes exit 0, live-MCP probe PASS,
overflow scan CLEAN (0 hits) on all 31 trees.**
Snaps: `stykanimate-wave13-{default,library-populated,sprites-populated}.png`
in the user's files — the sprites snap shows both
seeded instances as labelled placeholder quads on
stage at their keyed rotations; the library rows and
thumbnails are proven in the tree dump and the
Round 14 Runtime test (in the PNG they sit below
the fold).

**Closing census** — MCP **288** (235 registry + 9
io + 44 built-ins); palette **134** (E3 shipped two
sprite commands, not three — `next-sprite` was
dropped in its audit because no sprite selection
existed to advance; the skeleton above said 135 and
is corrected here); scenes **28**; help **61**;
layout facts **31**; icons **163**; keys **940**
(827 static + 113 dynamic), zero lost; State 46
fields in 22 full literals.

**Honest remainders** — the clip-def media fix
(E5's anchored spec) is the headline future-wave
candidate, unbuilt. Sprite keys can only be authored
within the timeline (A1's clamped op contract,
owner-pinned; imported out-of-range keys still
render and survive). The stage still cannot draw
real sprite pixels (Rune has no bitmap primitive) —
placeholders are geometry-exact, appearance is not.
Library thumbnails remain unit- + tree-proven, not
visible in a snap (below the fold). Sound is still
user-verified only. The five standing user decisions
below are unchanged.

Standing user decisions this wave carries but does
**not** settle — none of them is in any squad's
remit, and they are recorded here so they are not
lost: the **project-swap undo canonical behaviour**
(load clears undo but not redo, `.piv` import clears
both, New/open clear neither — a product call, not
a fix); **white-on-accent contrast** (the
white-text carrier is upstream rune `tok::accent`
**#4392F7 at 3.14:1** in both themes; darkening the
accent-filled call sites to ~**#2072D5 (4.72:1)**
fixes it, darkening the app token alone does not —
the user's open decision since the Wave 10
close-out); the **two "New project" results** that
still disagree; the **C5 waveform call** (a deleted
audio file's stale waveform vs a "(missing file)"
badge); and **sound remains user-verified only** —
no squad can listen to playback.

The wave works under five exclusions, stated
plainly: **no project-swap undo/redo semantics
change**, **no accent colour change**, **no
stored-name-after-rename divergence fix**, **no
waveform-badge change**, and **no MCP-save
`write_recovery` change**. (The B squad's `redo`
`selected_figure` clamp is a parity bug fix inside
its remit, not a swap-semantics change.) The user
chose "launch as planned" over "answer the
decisions first", so all five stay exactly as they
are until the user rules on them.

### 6.9 Wave 14 — complete MCP full control (2026-11-09)

An eighth improvement wave, launched 2026-11-09
(10:43 +06): the user tapped "Assign next 50
agents for complete full control mcp". Same shape
as Waves 9–13: 50 agents, five squads of ten, one
owner per file, the session integrator wiring the
shared files (roster: `~/workspace/wave14-roster.md`).
The charter: **MCP can do EVERYTHING the editor
can do — no capability reachable only by mouse.**
Three thrusts: **total parity** — every
`EditorCore` op, every one of the 134 palette
commands and every panel action is tool-reachable
or sits on a named, argued exclusion list, with
the parity census enforced by the gate rather
than resting on reports; **shell control** — the
live endpoint can also drive the GUI shell itself
(workspace, transport, view, selection, theme,
panels), which today it cannot touch at all (the
drain reaches `EditorCore` only); and **clip-def
media end-to-end**, per the anchored spec in
`docs/wave13/CLIP_MEDIA.md` — attachable via MCP
and actually rendering on every surface. Around
the thrusts: **transport hardening** for the live
endpoint and the stdio server (including the
known busy-port invisibility gap), and an
**MCP-only proof build** — a complete project
created and exported using nothing but MCP calls.

The squads: **A, parity census & fill** — the
census (`docs/wave14/PARITY_CENSUS.md`) enumerating
every pub `EditorCore` op (all `*_ops.rs`,
`editor.rs`, `projects.rs`), all 134 palette
commands and all panel actions, each marked
tool-covered or GAP, as the wave's ledger; the
missing `tool_*` handlers written in their domain
files — figure and frame ops, the draw family's
nib/spray/scatter completion, graph-editor parity,
FX + particle parity, the audio clip
trim/fade/gain/normalise family, the 3D figure
set, media/sprite/project residuals, and the
editor-settings family (scene weather, the full
camera-key set, onion-skin, loop/easing defaults)
— with `Tool` entries prepared for the
integrator's registry splice; and palette parity
mapping all 134 commands to a tool, to Squad B's
shell list, or to a named exclusion. **B, shell
control** — the design (`docs/wave14/SHELL_CONTROL.md`:
a `ShellCommand` channel — command enum, request
queue, drain in the main poll loop, result return
path, an honest refusal on stdio, and the standing
security posture stated plainly: loopback, no
auth, same as today); plumbing in a new
`src/shell_control.rs` with exact splice specs
for the hot files (no hot-file edits); workspace
and shell-state tools; transport tools driving
the shell's real playback state; view tools
(view navigation only — never camera authoring,
the Wave 5 doctrine); selection tools writing
the shell's selection fields (including
`sprites_panel.selected`); theme and
`LayoutPrefs`-backed settings tools; panel tools
(section collapse/expand, timeline collapse,
visibility per the workspace rules); non-dialog
file/menu actions, with honest named refusals for
the dialog-bound ones; Runtime-level shell-control
tests; and the shell section of `docs/MCP.md`.
**C, clip-def media end-to-end** — clip-def media
joins the `media_items` composite in `media.rs`
per the anchored spec, draws on the stage, renders
in the raster exporter and is emitted by the SVG
exporter; MCP clip-media attach/clear/read parity
audited against the Wave 11 clip tools and filled;
`asset_remove`'s reference census updated now
that clip media renders; movieclip session/timeline
interplay tests; thumbnails where the spec requires
them; the doc flips (`CLIP_MEDIA.md` marked landed,
`docs/MCP.md` rows); and a clip-media end-to-end
suite driven and exported via MCP only. **D, live
transport hardening** — the busy-port fix
(`live_mcp.rs` gains a `LAST_START_ERROR` static
and `last_start_error()` per the recorded spec,
with the settings-note splice spec for the
integrator); a stdio↔live parity harness diffing
one identical scripted sequence across both
transports; the queue/backpressure audit against
the Wave 12/13 bounded-queue + timeout-discard
discipline; health-endpoint completeness and truth
(tool count derived from `all_tools`, a
shell-control awareness flag if Squad B lands one);
robustness — malformed JSON, wrong types, huge
payloads and unknown methods answered with clean
protocol errors and zero panics; batch +
notification semantics audited against JSON-RPC
on both transports; stdio handshake parity with
live; connection lifecycle (disconnect mid-request,
partial writes, slow clients); settings-UX truth
pins; and `docs/MCP.md`'s transport chapter
rewritten to the landed truth. **E, proof &
gates** — the MCP-only build (an end-to-end suite
creating a complete project using ONLY MCP calls
— figures, frames, keys, graph, draw, fx,
particles, audio, sprites, assets, 3D, clips,
camera, weather — then exporting PNG + SVG + a
range via MCP and asserting the outputs exist and
are sane); the parity gate extended so every op
and command is covered or on the named exclusion
list, its failure mode proven on the entering tree
first; the `docs/MCP.md` full audit (every tool
documented with args, two-direction sync under
gate check 9); the MCP help row updated to the
full-control truth; the wave's acceptance
checklist and honesty sweep in `docs/wave14/`;
overflow/snap upkeep for any UI the wave changes;
the built-ins (44) and io (9) census on both
transports; contract re-verification across ALL
new tools (exactly one undo step per mutation,
−32602 for unknown tools, finiteness on every
float argument, refusals quoting the offending
value); and this section.

Entering counts, as recorded at the Wave 13 close
(§6.8) and re-stated here so the closing census
has its baseline: MCP **288 tools** (235 registry
+ 9 io + 44 built-ins, per `scripts/mcp_census.py`);
**3,319 tests** (CI run **42562084573** on
`ac47b99`, GREEN — 1,337 lib + 1,911 bin + 71
integration; the docs close-out commit's CI run
**42562837588** also GREEN); **940 unique keys**
(827 static + 113 dynamic patterns), zero lost;
palette **134**; scenes **28**; help **61**;
layout facts **31**; icons **163**; gate **9/9**
(`scripts/phase_j_checks.py` on the closing tree);
headless run **42562380603** (all 58 probes 0,
live-MCP probe PASS, overflow CLEAN on 31 trees).
HEAD at launch: `92c5d95`, the Wave 13 close-out
commit.

**Delivered** — all three charter thrusts landed.
**Parity (A):** the census
(`docs/wave14/PARITY_CENSUS.md`, 764 capabilities
classified) drove 47 new core registry tools —
figure/frame residuals, the draw family's
nib/spray/scatter completion, graph selection-state
reads/writes and channel reads, FX + particle
completion, the audio gain step, fig3 rotation and
root placement (plus the fig3 camera yaw/pitch fix),
scene weather, the full camera-key set, onion-skin
master and depths, clipboard state, sprite frame
sampling, and project-file delete/cycle/open; its
close-out addendum records each true-gap mechanism's
disposition. **Shell control (B):** 31 live-only
`shell_*` tools behind the new `src/shell_control.rs`
peek/queue/drain (bounded 8, drained in the main poll
loop after replies are sent), dispatched by name to
`shell_tools_*` handler modules — workspace + shell
state, view navigation, selection (frame, figure,
node, sprite, stroke, particle, clip), theme,
settings (with the Live-MCP-off deferred-intent
bridge: a setting that would kill the very endpoint
serving the call applies only after the reply is
delivered), layout (sections and timeline per the
workspace rules), and the File menu's non-dialog
actions — plus 4 transport tools implemented as
ordinary core handlers, because playback is core
state. Two design deviations are recorded in
`docs/wave14/ACCEPTANCE.md` §8: granular tool names
won over the ticket design, and there is **no Stop
tool** — the GUI has none. On stdio every shell tool
refuses with one canonical sentence naming the live
endpoint. **Clip-def media (C):** the
`media_items` per-node fallback (instance wins,
clip def fills gaps, alpha = instance × carrier,
colour falls back to the figure), the stage-text
carrier fix, the raster `has_media` clips clause,
funnel-fed SVG, three MCP tools
(`clip_asset_attach`, `clip_clear_node_media`,
`clip_get_node_media`), and
`docs/wave13/CLIP_MEDIA.md` §7 flipped to LANDED
with every anchor re-verified. **Transport (D):**
`LAST_START_ERROR` + `last_start_error()` with the
Settings start-failure note (the busy port is no
longer invisible), a 60 s connection write
deadline, health gains `port`, `enabled` and
`shell_control: true`, one server-identity const
set, the `arg_f32` finiteness fix, and the
stdio↔live parity harness (no divergences found).
**Proof (E):** the MCP-only build suite (a complete
project created and exported through MCP calls
only), gate check 10 (PARITY), the MCP audit and
this section. `docs/MCP.md` documents all 366
tools; the gate counts the 31 macro-stamped shell
stubs as the real handler definitions they expand
to, and `scripts/mcp_census.py` carries the same
rule.

**Verification** — gate **10/10 PASS**
(`scripts/phase_j_checks.py`, full foreground run
on the closing tree, read from the top). CI run
**42571128696** GREEN on `8f5bb54`: **3,519 tests
passed, 0 failed** (1,432 lib + 8 mcp_server bin +
1,985 bin + 94 integration), after 9 CI fix rounds
— every one in test or plumbing detail (spec-escape
literals, an mpsc path, test-module imports, two
float-tolerance reconciliations, the three-test
bridge-port serialisation, two e2e payload-shape
corrections, one cross-id comparison), each
diagnosed from the failing run's own logs; no
production behaviour was reversed by any fix.
Headless run **42571630518** SUCCESS: all 60
probes exit 0; the live-MCP probe **PASS**
end-to-end, its new step 8 driving
`shell_get_state`, `shell_set_workspace` (the
switch visible in a fresh read, then switched
back), `shell_select_frame` and
`shell_get_transport` over HTTP against the real
running editor; overflow scan **CLEAN on 32
trees**. Snaps pixel-read and copied to
`your_files/stykanimate-wave14-*.png` — the new
`settings-open-error` scene shows the D1 note
verbatim: "Live MCP did not start: Address already
in use (os error 98). Turn the toggle on to
retry." 

**Closing census** — MCP **366 tools** (313
registry + 9 io + 44 built-ins; entering 288;
`scripts/mcp_census.py` and gate check 9 agree,
unregistered 0, duplicates 0); palette **134**
(unchanged — coverage changed, not the command
set; gate check 10 maps all 134 via 94 distinct
tools with PARITY_PENDING empty); scenes **29**
(+settings-open-error); help **61** (rows
rewritten to the full-control truth, count
unchanged); keys **940** (827 static + 113
dynamic), **zero lost**; icons **163**; i18n
**99 × 28**; layout facts **31** (the
start-failure note adds no layout fact); gate
**10/10** with the new check 10.

**Honest remainders** — E9's
`docs/wave14/` doc pack never reported (49 of 50
agents reported; the integrator authored the
`docs/MCP.md` shell chapter against the landed
design in its place, and B10's chapter draft —
written against the superseded ticket design —
was not used). Two parity items stay untooled
with their arguments recorded in
`docs/wave14/PARITY_CENSUS.md`'s close-out
addendum: figure-library entry rename (G5 — the
panel rename is file-backed shell logic with no
lib op beneath it) and graph channel selection
(G6 — GUI cursor state; the graph tools address
channels by explicit index). The legacy
`cycle_onion_frames` op (G3) is deliberately
untooled: it writes a field the stage no longer
reads, so a tool for it would be a fake control;
the real depths are `shell_set_onion_depths`.
The pre-existing `tool_set_weather` /
`tool_clear_weather` defaults still differ from
the panel's presets (disclosed by A9, untouched).
D10's transport chapter remains in
`docs/wave14/` as the survey record; its
substance is merged into `docs/MCP.md`. Sound
remains user-verified only, as in every wave.

Standing user decisions this wave carries but does
**not** settle — none of them is in any squad's
remit, and they are recorded here so they are not
lost: the **project-swap undo canonical behaviour**
(load clears undo but not redo, `.piv` import clears
both, New/open clear neither — a product call, not
a fix); **white-on-accent contrast** (the
white-text carrier is upstream rune `tok::accent`
**#4392F7 at 3.14:1** in both themes; darkening the
accent-filled call sites to ~**#2072D5 (4.72:1)**
fixes it, darkening the app token alone does not —
the user's open decision since the Wave 10
close-out); the **two "New project" results** that
still disagree; the **C5 waveform call** (a deleted
audio file's stale waveform vs a "(missing file)"
badge); and **sound remains user-verified only** —
no squad can listen to playback.

The wave works under five exclusions, stated
plainly: **no project-swap undo/redo semantics
change**, **no accent colour change**, **no
stored-name-after-rename divergence fix**, **no
waveform-badge change**, and **no MCP-save
`write_recovery` change**. One exclusion guard is
new in kind this wave and is stated explicitly:
**shell control must not become a side door
around any of them** — a shell tool that loads,
creates or swaps projects inherits the standing
undo/redo behaviour exactly as the GUI path has
it, a theme tool changes no accent value, and no
shell or parity tool may alter what an MCP save
writes beyond what the editor itself writes. The
user chose "launch as planned" over "answer the
decisions first", so all five stay exactly as they
are until the user rules on them.

### 6.10 Focused Wave — Wave 14's named remainders (2026-11-09)

A focused follow-on wave, launched 2026-11-09
(12:49 +06): the user tapped "Launch the focused
wave" on the proposal prepared from Wave 14's
close-out. It is deliberately **not** called
Wave 15 — `docs/QA_WAVE15.md` names a different,
older wave (the 2026-11-08 2.0 workspace-gating
wave) — and it is deliberately small: **11 agents,
five squads**, one owner per file, the session
integrator wiring the shared files (roster:
`~/workspace/focused-wave-roster.md`). The scope
is exactly Wave 14's named remainders and nothing
else — padding it to a round number would be
dishonest about the agent count. The charter:
**close the four remainders Wave 14 carried by
name** — the figure-library entry rename (G5),
the graph channel selection (G6) and
recent-colours list (G7) shell-state decisions,
the weather-defaults divergence, and the unwired
layout prefs — with the same discipline as every
wave: CI-only verification, the gate deriving its
claims from source, and no fake controls.

The squads: **A, G5 library rename** — the
panel's file-backed rename logic
(`library_panel.rs::rename_saved`) moves into a
lib `EditorCore` op in a new `src/library_ops.rs`
(the `pose_library_rename` pattern), preserving
every refusal rule verbatim (built-in, empty
stem, same name, name taken, target file exists,
fs error) and the status-message voice; the panel
becomes a thin wrapper; `tool_figure_library_rename`
makes it a core MCP tool; an MCP-dispatch-only
e2e suite pins the happy path and each refusal.
**B, G6/G7 shell-state decisions** — graph
channel selection is traced to its storage
(`State::graph_panel.selected_channel`, shell
state of the same class Wave 14 tooled for
sprite/stroke/particle/clip) and lands as
`shell_select_graph_channel` with
`shell_get_selection` extended to report it; the
recent-colours list is decided from the code —
a read tool (`shell_get_recent_colours`) if the
list is shell state, else a written session-only
declaration for the gate's exclusion record; no
fake writes either way (applying a colour is
already `set_node_style`). **C, weather
defaults** — `tool_set_weather`'s drifted
defaults (storm 140/30, everything else 90/20)
are replaced by the panel presets from ONE
source (`eds_kind_preset`: rain 90/25, snow
90/12, fireflies 60/4, fog 60/0, storm 140/30);
tests pin each kind's default through the tool;
the editor comment that recorded the divergence
is updated to the new truth. **D, layout
wiring** — `LayoutPrefs.timeline_collapsed` and
`sidebar_visible` become real: startup applies
them as it already applies dark / `live_mcp`,
the UI toggles write them back, rendering reads
the wired state; `shell_set_timeline` — excluded
in Wave 14 as a fake control while its prefs
wire was dead — lands as a real control, and
`shell_get_layout`'s `timeline_collapse_wired` /
`sidebar_pref_wired` flags flip to true, proven
by Runtime tests that round-trip the prefs into
the rendered timeline. **E, proof & close** —
gate check 10's parity map gains the landed
tools and the G5/G6/G7 dispositions leave the
exclusion record only where a tool actually
landed; `docs/MCP.md` rows for every new tool
plus the weather and layout truths; the
acceptance checklist (`docs/focused/ACCEPTANCE.md`);
the headless live-MCP probe gains a
`shell_set_timeline` step if D lands it; and
this section.

Entering counts, as recorded at the Wave 14
close (§6.9) and re-stated here so the closing
census has its baseline: MCP **366 tools** (313
registry + 9 io + 44 built-ins, per
`scripts/mcp_census.py`); **3,519 tests** (CI run
**42571128696** on `8f5bb54`, GREEN — 1,432 lib +
8 mcp_server bin + 1,985 bin + 94 integration;
the Wave 14 docs close-out commit's CI run
**42572280451** also GREEN); **940 unique keys**
(827 static + 113 dynamic patterns), zero lost;
palette **134**; scenes **29**; help **61**;
layout facts **31**; icons **163**; i18n
**99 × 28**; gate **10/10**
(`scripts/phase_j_checks.py` on the closing
tree); headless run **42571630518** (all 60
probes 0, live-MCP probe PASS including the
shell steps, overflow CLEAN on 32 trees). HEAD
at launch: `4af7d85`, the Wave 14 close-out
commit.

**Delivered** — all four remainders closed.
**A (G5):** `figure_library_rename` landed —
`EditorCore::rename_library_entry` +
`tool_figure_library_rename` in the new
`src/library_ops.rs`; the panel's `rename_saved`
is a one-line wrapper and its four pure helpers
now delegate to the lib copies (the panel const
is an alias of the lib's), so one implementation
exists. The e2e suite
(`tests/focused_e2e_library_rename.rs`, 3 tests)
drives it through MCP dispatch only. One
bookkeeping correction, recorded per the
acceptance's instruction: a library rename
spends **zero** undo steps and no dirty mark —
it is a file operation, the
`pose_library_rename` precedent — so the
roster's "exactly-one-undo-step" shorthand for
A3 was wrong and the suite pins the true
contract (undo depth unchanged; Undo does not
revert a rename). **B (G6/G7):**
`shell_select_graph_channel` landed, calling
the graph panel's own `select_channel` (pick +
target sync + figure switch are the row press
exactly; it is the only handle that can name
the second of two duplicate channels);
`shell_get_selection` reports `graph_channel`
and `graph_channel_drawn`. G7's outcome, by
name: **(a) — the read tool landed**:
`shell_get_recent_colours` reads the style
panel's session Recent list (cap 8, most
recent first); no write tool, by design —
applying a colour is `set_node_style`, and a
push tool would fabricate history. **C:**
`tool_set_weather` now reads
`editor::eds_kind_preset` for omitted
amount/wind — one defaults table for panel
and tool (rain 90/25, snow 90/12, fireflies
60/4, fog 60/0, storm 140/30), pinned by a
tool-level test over all five kinds.
**D:** both prefs wired. The timeline flag
lives in the timeline's own sub-state
(`State.frame_ops.timeline_collapsed`, seeded
in `init`, mirrored back by poll's
load-modify-save); the collapsed card renders
frame-chips-only with the chips' keys and
geometry untouched. `sidebar_visible` is the
**left column's** persisted mirror
(`show_left` seeds from it, flips persist
back); `show_right` has no pref in any era
and stays session-only. `shell_set_timeline`
landed; `shell_get_layout` reports
`timeline_collapse_wired: true` and
`sidebar_pref_wired: true`. Two deviations
from the charter text, argued: the flag is a
sub-state field, not a top-level `State`
field (the ~30 literal `State` constructions
made a new field the expensive option, and
D2's handler was repointed to the real field
at integration); and **no GUI timeline
toggle exists** in the Rune shell (the GPUI
one was lost in the rewrite) — none was
invented; the writers are the startup pref
and the shell tool. **E:** gate check 10's
tables deliberately did **not** move — G5/G6/
G7 were never palette commands, so there were
no rows to move and inventing keys would have
fabricated coverage (E1's premise correction);
the dispositions live in the gate's comments
and the `docs/wave14/PARITY_CENSUS.md`
Focused-Wave addendum. `docs/MCP.md` carries
every new row and the corrected counts; the
headless probe gained step 9
(`shell_set_timeline` round-trip) in
`.github/workflows/headless.yml` — the probe
is inline there, not a scripts/ file — and
its `TOOL_FLOOR` rose 366 → 370.

**Verification** — gate **10/10 PASS** on
the closing tree (`scripts/phase_j_checks.py`,
full foreground run; check 9 derives registry
317 / surface 370 with exact doc sync, check
10 parity unchanged at 134 commands via 94
tools). **CI 42578449523 GREEN** on `d311b58`:
**3,539 tests, 0 failed** (1,437 lib + 8
mcp_server bin + 1,997 bin + 97 integration;
Wave 14: 3,519 — the +3 integration tests are
A3's suite, and the fail-fast lesson held:
each suite's own `test result: ok` line is in
the log). Three fix rounds, all test-side:
the registry count pin (313 → 317) and the
two gating snap tests normalised for the
shared-layout-file race D1's wiring made live
(see Honest remainders). **Headless
42579538663 SUCCESS**: all probes exit 0,
**MCP PROBE: PASS** including step 9's three
assertions (`shell_set_timeline` collapses on
the live shell, `shell_get_layout` shows it
collapsed and wired, expand restores), health
tool count ≥ floor 370, **overflow CLEAN on
32 trees**, 27 snaps (copied to the user's
files as `stykanimate-focused-wave-*.png`).

**Closing census** — MCP **370 tools**
(registry **317** + io 9 + built-ins 44; shell
tools **38**, `SHELL_TOOL_NAMES` 34), against
366/313/35 entering — the delta is exactly the
four landed tools, each named above. Palette
**134** (unchanged, deliberately — see E);
keys **940, zero lost** (no element keys were
added or removed; D1's collapsed form reuses
the chips' existing keys); scenes **29**;
help **61**; layout facts **31**; icons
**163**; i18n **99 × 28** — all gate-derived
on the closing tree.

**Honest remainders** — (1) Wiring the prefs
into `init` made the bin test binary's shared
layout file a live input: two snap tests now
normalise the pref-seeded fields after boot
(the race is documented in the tests; a
per-test layout file would remove it, at
larger churn). (2) `shell_get_state` reads
the timeline pref from disk, so it can trail
a `shell_set_timeline` write by one tick;
`shell_get_layout` reads the live field and
is always current. (3) Colours applied
through MCP tools are not recorded in the
Recent list — only the style panel's own
handlers note colours (B2's stated limit,
in the tool's description). (4) Sound
remains user-verified only, as below.

Standing user decisions this wave carries but
does **not** settle — none of them is in any
squad's remit, and they are recorded here so
they are not lost: the **project-swap undo
canonical behaviour** (load clears undo but not
redo, `.piv` import clears both, New/open clear
neither — a product call, not a fix);
**white-on-accent contrast** (the white-text
carrier is upstream rune `tok::accent` **#4392F7
at 3.14:1** in both themes; darkening the
accent-filled call sites to ~**#2072D5 (4.72:1)**
fixes it, darkening the app token alone does
not — the user's open decision since the Wave 10
close-out); the **two "New project" results**
that still disagree; the **C5 waveform call** (a
deleted audio file's stale waveform vs a
"(missing file)" badge); and **sound remains
user-verified only** — no squad can listen to
playback, and nothing in this wave touches
playback audio.

The wave works under six exclusions, stated
plainly: **no project-swap undo/redo semantics
change**, **no accent colour change**, **no
stored-name-after-rename divergence fix**, **no
waveform-badge change**, **no MCP-save
`write_recovery` change**, and **no tool for G3
`cycle_onion_frames`** — the legacy op writes a
field the stage no longer reads, so a tool for
it would be a fake control; the real depths
remain `shell_set_onion_depths`. The Wave 14
side-door guard stands unchanged: **shell
control must not become a side door around any
of them** — a new shell tool writes only the
shell state or prefs its name claims, a theme or
style tool changes no accent value, and no shell
or parity tool may alter what an MCP save writes
beyond what the editor itself writes. The five
standing decisions stay exactly as they are
until the user rules on them.

### 6.11 Wave P — the Rune line's own wave: Phase O close-out, core soundness, subtitles arrive (2026-11-10)

Ordered 2026-11-10 (09:34 +06): "run nxt wave
using 50 agent improve master" — hours after
master was restored to this line (`0b75051`,
the Focused Wave close-out), with the Tauri
line parked on branch `tauri` — untouched by
this wave (the user deleted that branch
mid-wave, on the same day; nothing in this
wave depended on it). Fifty agents, five squads, one
owner per file, the session integrator wiring
the shared files (board:
`~/workspace/wave-p/BOARD.md`; per-agent
findings: `~/workspace/wave-p/findings/`). The
themes: close the Phase O "feel & finish"
charter on this shell; land the core soundness
fixes the Tauri line proved against this
shared ancestry (bend permutation,
missing-audio surfaces, `format_version`);
bring subtitles to the Rune line; and audit
MCP/shell parity. Two honesty notes govern
everything below. First, this section was
written late in the wave from the findings
then on disk, and was brought up to date in
Wave Q's docs pass (Q11) from all 40 findings
now filed — p1, p2, p3, p4, p6, p7, p8, p9,
p10, p12, p13, p14, p15, p16, p17, p18, p19,
p20, p21, p22, p23, p24, p25, p26, p27, p29,
p30, p32, p34, p35, p36, p37, p38, p39, p40,
p41, p42, p45, p46, p47. The ten never filed
(p5, p11, p28, p31, p33, p43, p44, p48, p49,
p50) are named as such where their topics
arise; nothing is claimed for them beyond
what the tree itself shows. The user halted
the wave at 09:49 +06, before integration;
Wave Q (20 agents, ordered the same morning)
finishes the stopped slices and performs the
wiring this wave filed as integrator actions
— this update is part of that pass. Second,
the wave's work sits in the working tree,
uncommitted: every status below is
IMPLEMENTED (in tree) or VERIFIED
ALREADY-LANDED (an audit outcome), and **CI
is awaiting integration** — the only completed
master run observed via `gh` is
**42749543075** (Build, Test & Binary,
SUCCESS), which covers HEAD `0b75051` itself,
not Wave P.

**Phase O feel & finish (Squad 1) — mostly
already here, and now proven so.** The squad's
recurring result is that the charter's
premises predate their own remedies. The p05
overflow plan (O-1..O-9) measured a tree from
before the UI mission's repairs: P1 (O-1..O-3),
P2 (O-4..O-6) and P3 (O-7..O-9) verified every
named row in the tree site by site — the
settings language grid wraps with its
load-bearing `.w_full()`, the layers row has
its two-line anatomy, the draw / graph /
scene / 3D / FX / playback / pose / camera
rows all wrap or split inside the 316 pt
budget — with **zero code changes** except
O-5's one genuine remainder: `keyframe-actions`
and `keyframe-clipboard` had no wrap at HEAD
(the Wave 1 commit message overclaimed them);
the two `.flex_wrap()` calls (+11 lines,
`src/rune_sec/keyframe_tools.rs`) are in the
tree. The §2 scan tool
(`scripts/overflow_scan.py`) is itself verified
at spec, self-test green. What remains for
O-1..O-9 is proof, not code: the §2 scan over
the next headless dispatch's dumps. P43's
close-out pass was stopped at the halt with
partial edits to the script in the tree; the
extended scan (the scene trees P2/P3/P4
handed off, plus the subtitle lane's) is
Wave Q's Q12. P7 and P8 audited every Phase N
bare line (panels A–M, N–Z, plus Home): all
already render the kit's
`design::empty_state`, landed by UI mission
Wave 4 — audit and proof only, no code. P10's
motion census found every animated surface
already on the charter's two spring pairs
(`pair::SNAP` / `pair::SETTLE` — four explicit
`.transition(` sites plus the engine default);
zero conversions; the toggle knob's teleport
(the pinned Rune rev cannot animate a layout
offset) and the toast expiry's linear fade
(Phase N behaviour, charter-preserved) are
named, not faked. P12's charter sweep closed
the unowned remainder: the zero-consumer kit
items were already decided (`toggle` retired,
`text_muted` kept as the kit's muted-text
role), the remaining kit classes carry their
states, the Esc overlay order (palette, then
menus) is the dispatch's own order and is
test-pinned, every shortcut hint shown in the
UI names a real binding, and copy is en-GB
with mono numerics via the kit stepper. Its
two open items are integrator actions,
recorded in the remainders below. The rest of
the squad has since filed. **P4 —
interaction states:** four surfaces lacked a
state the charter requires — command-palette
result rows and fx-lane rows had no press
state, scene background swatches no hover or
press, the legacy timeline chips no press;
all four fixed in the overlay surfaces only
(+107 lines, zero keys). Focus remains an
engine limit, named not faked: the Rune `El`
has no button focus state to draw. **P6 —
feedback completeness:** a transitive census
(333 mutating core ops) found the shell
already reports nearly everything; the one
silent class was stage pose and camera drags
ending without a word — `end_drag` now posts
the family's recap on a real move only, and
stays silent on a no-move press. **P9 — Esc
discipline:** three defects in the shell's
key routing, all fixed in `src/main.rs` with
nine pins — the Home rename field was missing
from the text-focus guard, so stage shortcuts
fired mid-typing; menu Esc sat below the
text-field bail and could leave a menu open;
and the two armed confirm gates had no
keyboard exit at all, where the new Esc tier
now disarms them topmost-first. Zero key
loss. Findings for P5 (inputs / selects /
steppers) and P11 (the settings-language
remainder) were never filed — both slices
were stopped at the halt and are finished in
Wave Q (Q8, Q9).

**Core soundness (Squad 2).** **P13 — the
bend corruption is fixed.** `layers::move_figure`
permuted figures and poses but never
`frame.bends`, so any layer reorder silently
re-paired every figure with another figure's
bend rows — the Tauri Wave J bug class,
present in this ancestry. The fix pads sparse
bend tables to the figure count (an absent row
and an empty row are semantically identical:
both read as straight) and permutes the rows
with the figures; five pins cover full,
sparse, multi-step, bend-free and
undo-restore cases. One adjacent hazard was
reported with it (outside P13's surface):
`EditorCore::move_figure` (editor.rs) swapped
bend rows only when both indices held rows,
so a single-step swap across the
padded-prefix boundary could strand a row.
**P46 has since fixed it** with the same
pad-then-swap remedy, and its regression
suite (`tests/wave_p_e2e.rs`, seven tests)
pins bend pairing through both move ops at
every boundary shape. **P14 —
missing audio now surfaces at open.** Clips
were never dropped at load; they were
silently dead at the consumers. One shared
query (`audio::missing_audio_clips` — unmuted
clips whose files are absent, in clip order)
now feeds a runtime `EditorCore::missing_audio`
report, refreshed at every project swap,
undo/redo and converter import, and the three
load statuses name the missing files (a clean
load's status is byte-identical to before).
The one definition dispute — whether a muted
clip's missing file is reported — was settled
by the parity rule: playback and the export
mix both treat a muted clip's absence as no
loss, so it is not reported; **P15 — the
export side, since filed and completed:**
missing files now surface on MP4 export too
— the full and range outcomes carry them
(`Mp4ExportOutcome::missing_audio`, on the
same shared query), the editor status and
the export panel's range arm speak them, and
the MCP replies gain a `missing_audio` field
only when non-empty, so an all-present export
answers byte-identically to before. **P16 + P23 — `format_version`.** The
field is in the tree (`src/model.rs`:
`format_version: u32`, serde default
`FORMAT_VERSION_CURRENT` = 1, beside its doc
rule — no loader-side version check; fields
are absorbed by serde defaults, so legacy
files load as current, a stamp of 0 loads
untouched, and a newer stamp still loads).
P16's seven behaviour pins are written
against the contract; the module they waited
on (P23's, below) has since landed in the
tree, so the pins compile against it — CI is
the proof. **P20 — figure-library rename
pinned, no divergence:** this tree's rename
takes zero undo steps by design, the Tauri
settlement; five new pins cover the refusal
set and the op level, and the one deliberate
asymmetry (MCP refuses a missing entry; the
core op settles silently for a stale slot) is
pinned on both sides as documented behaviour.
**P22 — round-trip hardening:** save→load
pins over a fixture exercising all 24
top-level `Project` fields, now including the
subtitle fields; **no lossy field found** —
the load path is a bare `from_str` with no
transform on it, and the suite's per-section
diff format is the evidence mechanism if CI
ever disagrees. **P21 — media parity: SPLIT
verdict, STOP taken.** Media *images*
(segment images + sprites) are settled: one
evaluated stream (`media::media_items`) feeds
stage, raster and SVG, and no channel-driven
visibility / opacity / colour targets exist
on this tree for any surface to miss.
Segment *text* diverges: the stage re-derives
it via `stage_text::text_geometry` from the
channelled evaluation (`positions_f`), so
under any graph channel the stage's text sits
on the channelled segment while both exports
place it on the un-channelled one; the stage
also applies carrier style opacity (the
exported `MediaItem::Text` has no alpha
field) and honours solo (exports read
`fig.hidden` only). Per the parity rule P21
changed no code; the divergence was routed
to P17 (the export-funnel question — the same
two-evaluations root). **P17 has since
settled it:** one rule at each funnel head —
when `project.channels` is non-empty, poses
and defs come from `graph::apply_channels`,
else the raw path runs verbatim — applied in
`render_frame_at`, `write_frame_svg` and
`media_items` (the single media funnel, so
segment text rides the channelled segment on
every surface now; the stage's own segment
images and trail lines, which sat on the raw
segment too, are fixed by the same swap).
Channel-less projects render byte-identically;
pins cover the raster, SVG and media forms.
What remains open from P21's audit is
narrower: segment-text *alpha* parity —
`MediaItem::Text` has no alpha field, so that
one is a media-model change, not a funnel
fix. **P18 — zero-length parity:** the
audit's one settled cell (a plain zero-length
segment paints a disc of radius t/2 in the
near colour, on all three surfaces) is pinned
in `tests/wave_p_zero_length.rs` and the
stage tests; every other cell disagrees
across raster / SVG / stage, so per the
parity rule no code changed and the
disagreement matrix is tabled in p18.md for
a settlement decision — with its adjacent
finding that polygon *fill* parity is broken
at every length on this tree (the stage
fills polygons; both exporters stroke the
outline), so the zero-length polygon cell
should be settled with polygons generally,
not alone. **P19 — the window-close guard:**
the bypass is proven, and structural — the
pinned Rune framework closes unconditionally
after its `on_close` hook and offers no veto,
so the fail-closed settlement is
loss-lessness, not refusal:
`EditorCore::flush_recovery_on_close` (in the
tree) writes the recovery snapshot
immediately whenever counted edits stand
outside it, closing the 25-edit autosave
window a close could otherwise drop. The
hook registration itself is IA-P19-1 (exact
code in p19.md), applied by Wave Q's Q2; the
pin file (`tests/wave_p_close_guard.rs`)
carries one routing pin that is red until
that wiring lands, deliberately, so the
bypass cannot be integrated away silently.

**Subtitles arrive (Squad 3) — built against
contract 1, sequenced on P23.** The contract
shapes are in the tree where P23 has landed:
`Project::subtitles` + `Project::subtitle_style`
(`src/model.rs`, serde defaults reproducing
the plain default look) beside
`format_version`. Around them, written
against the contract's exact API: **P24** —
SRT discipline pins (8 tests: frame-exact
round trips at 24 fps, on-grid and off-grid —
frame 7 exports as `00:00:00,292`; bad blocks
skipped; zero yield refuses; the parser
returns file order and the caller sorts; no
error wording asserted, by design). **P25** —
WebVTT pins (5 tests: a missing header
refuses whole-file even for parseable cues;
STYLE blocks are skipped, not mined; SRT and
VTT imports of the same cues agree; the empty
project exports exactly `"WEBVTT\n\n"` and
refuses re-import). **P27** — SVG emission
landed in `src/svg.rs` (`append_subtitles`:
stacked `<text>`, styled, painted last, stage
coordinates, cue-free output byte-identical),
with 7 pins fixing the bands — 572/548
bottom, 28/52 top on an 800×600 stage at
scale 1.0. **P30** — the MCP file-IO module
(`src/mcp_subtitle_io.rs`):
`subtitle_export_srt` / `subtitle_export_vtt`
/ `subtitle_import_srt` /
`subtitle_import_vtt`, path-based; imports
parse before any mutation and report
`changed: false` with no snapshot when the
file matches the current cues; census delta
+4 (370 → 374 from P30 alone), with exact IAs
filed for the `lib.rs` / `mcp.rs` /
`docs/MCP.md` / gate wiring. The honest
centre of the squad was sequencing, and it
has since resolved in the tree: **P23
landed** — `src/subtitles.rs` (the cue and
style types, `sort_subtitle_cues`,
`live_subtitle_cues`, the SRT/VTT converters
under the contract's refusal discipline, and
the shared layout helpers the other surfaces
consume) plus the EditorCore cue ops in
`src/editor.rs` (add / update / delete /
style — one undo step each, refusals spending
nothing). What remained outstanding at the
halt was registration — the `lib.rs` module
line and the `all_tools()` extends — which
Wave Q's Q1 performs. Around the core, the
rest of the squad landed before the halt:
**P26** — raster burn-in at the tail of
`render_frame_at`, riding the one media
funnel (so every raster format inherits it),
full style colours honoured, six pins.
**P29** — the five cue/style MCP tools
(`src/mcp_subtitle_tools.rs`; handlers never
snapshot — the core ops spend the step),
taking the census delta to +9 combined with
P30. **P32** — the timeline lane
(`src/rune_sec/subtitle_lane.rs`: cue blocks
on the clips lane's own span geometry,
sub-row packing for overlaps, click-to-select
seeking the cue start), its shell wiring
filed as IAs for Wave Q. **P34** — the e2e
suite's legs 1–3 (legacy load, core round
trip, export presence), with leg 4 (the MCP
round trip) filed as a drop-in test for
wiring time. Not landed in this wave: P28's
stage draw (partial edits in `stage_text.rs`
/ `stage_view.rs` at the halt; Wave Q's Q6
completes it), P31's Audio-workspace panel
(Wave Q's Q7 builds it) and P33's headless
pixel proof (Wave Q's Q13). Two parity notes
ride with the squad: the layout maths landed
inline in `svg.rs` and `export.rs` before
P23's helpers existed (IA-P27-1 / IA-P26-1
swap in the shared helpers, output unchanged
— Wave Q's Q17), so raster↔SVG band parity
is pinned-by-construction until CI compares
them; and P47's determinism suite (Squad 5:
repeat-render, A-B-A and interleave
byte-identity pins over the raster path, the
render path audited clean of clocks, RNG and
parallelism by inspection) records that its
subtitle sensitivity leg waited on P23 + P26
— both now in the tree; the leg itself is
Wave Q follow-through (Q20).

**MCP and shell parity (Squad 4).** **P36 —
camera parity holds; no gap proven, nothing
built.** Every GUI camera capability on this
tree has an MCP twin, including the stale-key
class the Tauri line tripped on: value writes
are timeline-bounded on both surfaces (the
MCP `check_frame` refusal matches GUI
reachability exactly), while per-key delete
at a past-end frame is reachable — and
unbounded — on both. The one nuance is
documented, not a gap: `set_camera_follow`
snapshots when the value changes where the
GUI writes snapshot-free (referred to P41's
undo sample). **P39 — the live-MCP
Host/Origin gate is built**
(`src/live_mcp.rs`). It was absent at HEAD:
the loopback bind stopped remote machines
but not a local browser page (a foreign
`Origin` on a cross-origin fetch, or DNS
rebinding with a foreign `Host`). Now
`host_origin_allowed` runs before routing,
before the body is read, before anything is
queued — every `Host` value must be a
loopback authority and every present `Origin`
a loopback http/https origin, else HTTP 403.
It is validation, not authentication: no
token exists, and the module docs keep the
standing no-auth warning in full. **P40 —
the unknown-tool contract already holds**
(Wave 11 ancestry; both transports share the
one dispatch): `-32602`, `unknown tool:
{name}`, no `result` member. P40 added the
one pin at full contract strength — near-miss
names plus the byte-exact message, with an
unmoved-state witness. The rest of the squad has since filed.
**P35 — the census is true:** `docs/MCP.md`'s
370 verified three ways (the census script,
the in-tree completeness pin, and a
schema-level diff of every row — 369 of 370
exact; the one garbled row, `fig3_set_camera`,
fixed in the doc). Its IA-2 names the trap
the new modules set: the census and gate
extractors read three source files and are
blind to modules with their own `tools()`, so
the extractor extension must land with the
wiring (Wave Q's Q4/Q5). **P37 — particle
parity:** every stored `ParticleEvent` field
was panel-editable except one — `seed` had
an op and an MCP tool but no panel control;
a seed stepper now sits in the detail block
(+2 keys, wrapping the full u32 range, one
undo step per press), and three MCP
descriptions that named panel controls which
do not exist were filed as exact-text IAs.
**P38 — the sprite clamped-delete hazard
exists here in its worst form:** the MCP
`sprite_delete_key` read, deleted and
verified at the *clamped* frame while its
success reply reported the *requested* one —
naming frame 999 deleted frame 4's key and
reported "frame 999". Past-end deletes now
refuse at both layers, spending nothing, and
two exact-frame tools join the registry
(`sprite_move_key`, `sprite_delete_key_at`,
`src/mcp_sprite_tools.rs`; census +2).
**P41 — the undo-contract sample:** 13 of 14
sampled tools compliant and pinned; one
violation — `figure_set_color` mutates with
zero undo steps and declares no exception,
where its sibling `figure_recolor` spends
one. Its disposition (declare the exception
or add the step) is Wave Q's Q18, taken from
the panel caller's shape. **P42 — session
recovery:** the spurious offer is confirmed,
with its mechanism — a clean GUI save
refreshes the snapshot *after* writing the
file, and the offer condition is pure mtime
with no content comparison, so a
byte-identical snapshot is offered as unsaved
work; the previously caller-less
`clear_recovery` is now wired to a Discard
button (+1 key, `home-recovery-discard`). The
content-comparison gate itself is
recommended in p42.md, not built — it is a
user decision, carried into the remainders.

**Docs (P45).** This section, and a README
correction pass limited to stale claims about
*this* line: the MCP tool count (65 → **370**,
the branch's own census in `docs/MCP.md`),
the Phase C / Phase D framing of the editor
UI and the "returns with the Phase D
sections" asides (the `rune_sec` sections —
FX, audio/waveform, movieclips, Home,
settings — are in the tree), and the
real-desktop status (the Rune shell was run
and screenshot-verified on a real Linux
desktop on 2026-11-08; `--snap` is exercised
by the headless workflow on the CI runner).
The README deliberately says nothing about
Wave P's subtitles or fixes: none of it is
integrated or CI-proven yet. (Wave Q's docs
pass — Q11 — later re-verified every claim in
this section against the tree and the 19
findings filed after the first write, and
updated the README's tool count to the
in-tree census once Q1's wiring and Q4's
census package landed there: **381**.)

**Verification — awaiting CI.** Observed via
`gh`: the latest completed run
on master is **42749543075** (Build, Test &
Binary) — **SUCCESS**, on `0b75051`, the
entering HEAD. No run covers Wave P: the
wave's changes are uncommitted in the working
tree, and integration (Wave Q) + push are the
integrator's next steps. The entering tree's
own record stands as the baseline: gate
**10/10 PASS** (`scripts/phase_j_checks.py`,
re-run locally by P1 and P2 during the wave —
Python, no compiling; census **827 static +
113 dynamic = 940 unique keys**, zero lost),
CI 42578449523 and headless 42579538663 green
at `0b75051`. Every Wave P test count and
census figure awaits the integration CI; the
filed deltas (MCP +4 from P30, +5 from P29,
+2 from P38 = **381** expected) move the
`docs/MCP.md` counts and gate floors only
through the integrator's recount — Q4's
census package has since landed in
`docs/MCP.md` at 381, and the script
ratchets are Wave Q's Q5.

**Honest remainders.** (1) Ten
findings were never filed: p5, p11, p28,
p31, p33, p43, p44, p48, p49, p50. Their
slices were stopped at the halt; Wave Q
finishes them (the stage draw, the subtitles
panel, the interaction-state and
settings-language remainders, the headless
proofs, the gate ratchet, the i18n wiring
audit, toolbar density, Home thumbnails).
(2) The subtitles stack is landed but not
yet whole: the panel, the stage draw's
completion, the lane/panel shell wiring and
the registry registration are Wave Q work,
and several Squad 3 suites are
contract-written, not behaviour-measured —
their first CI run is the measurement.
(3) P18's zero-length disagreement matrix,
and the polygon-fill divergence beneath it,
await a settlement decision; the one agreed
cell is pinned. Segment-text alpha parity
(P21) likewise awaits a media-model
decision. (4) P12's IA-1: `docs/PHASE_O.md`
and QA_RUNE §17 belong to close-out, after
the headless dispatch — they cannot be
written truthfully mid-wave, and this
section does not pretend to be them. P12's
IA-2 (an Esc arm for the settings card) was
answered by P9's census in code: the card is
a toggle panel, not a scrim overlay, and the
Esc tier treats it as such. (5) Two Wave Q
dispositions ride with the integration:
P41's F1 (declare `figure_set_color`'s
zero-step exception or make it spend one —
Q18) and P34's leg 4 (Q19). (6) P14's
follow-up — per-clip missing-file warnings
in the audio panel, consuming the shared
query — is Wave Q's Q20, together with
P47's subtitle determinism leg. (7) The
standing user decisions from §6.10 are
untouched and still open: project-swap undo
semantics, white-on-accent contrast, the two
"New project" results, the C5 waveform call;
to them Wave P adds one more — P42's
recommended recovery content-comparison
gate, which would end the spurious
byte-identical recovery offers. And sound
remains user-verified only.
