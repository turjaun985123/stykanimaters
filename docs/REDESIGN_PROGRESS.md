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

**Delivered** — TODO-AT-CLOSE: the integrator fills
this in at close-out with what actually landed,
squad by squad, against the remits above.

**Verification** — TODO-AT-CLOSE: run evidence only
(gate result, CI run id and test count, headless
run id, live-MCP probe and overflow verdicts, snap
list) goes here at close-out.

**Closing census** — TODO-AT-CLOSE: MCP, palette,
scenes, help, i18n, icons and keys as counted by
the gate on the closing tree, against the entering
baseline above.

**Honest remainders** — TODO-AT-CLOSE: what this
wave did not close, named at close-out.

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
