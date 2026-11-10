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

### 6.12 Wave R — parity + completeness burn-down: subtitles made whole, i18n wiring spreads (2026-11-10)

Ordered 2026-11-10 (10:29 +06) as a
re-issue of the standing continuous order:
"assign 20 agents continuous write code if
ci gets error then fix that". Twenty agents
on base `911cf77` (Wave P+Q plus its CI fix
round 1), one owner per file, integrator
surfaces via filed actions (board:
`~/workspace/wave-r/BOARD.md`; findings:
`~/workspace/wave-r/findings/`). Mid-wave
(10:36 +06) the user settled two standing
decisions with "Build both", and the wave
grew by two assignments: **R21** (the
subtitle `speaker` field) and **R22** (the
recovery content-comparison gate P42
recommended) — both formerly exclusions,
now approved work. Two honesty notes govern
everything below. First, this section was
written from the 18 findings filed at write
time (r1, r2, r3, r4, r5, r6, r8, r9, r10,
r11, r12, r13, r15, r16, r17, r18, r19,
r20); **r7, r21 and r22 were not yet filed**
and are named as such where their topics
arise — nothing is claimed for them beyond
what the tree itself shows. Second, every
status below is IMPLEMENTED (in tree,
uncommitted at write time), and **Wave R
has no CI verdict**: the base's own round-2
runs are the latest observed — headless
42755921970 **GREEN** on `911cf77`, build
42755902602 failed in the Test step on a
lib test-target compile error in Wave P's
missing-audio tests (imports in
`editor.rs`'s `wave11_tests`); the
integrator's fix for that rides with this
wave's integration push, not a separate
round.

**Subtitle surface parity (R1, R2, R3,
R20).** **R1 — the raster burn-in now lays
out through the shared helpers.** Q6's flag
is retired: the 6%-of-stage-height stamp
sizing is gone; the burn-in takes its font
size from `subtitle_font_size` and its line
centres from `subtitle_line_center_y`,
exactly as stage and SVG do, so all three
surfaces anchor a line's glyphs at the same
centre at the same size. The pin module's
bands were re-derived from the stamp maths
at the new size (centres 372/348/324 bottom,
28/52/76 top at H=400) — no assertion
weakened; the old values are kept in the
header as the pre-R1 record. **R2 — ONE
validation rule set, documented as a block
in `src/subtitles.rs`.** The audit's one
true inconsistency: the importers silently
*repaired* an inverted cue (end before
start) by clamping it into a fabricated
one-frame cue the file never contained,
where every other boundary refuses or
skips. Now an inverted block is a bad
block — skipped, and a file of only such
blocks refuses as zero yield — while a
non-inverted range that merely rounds onto
one frame keeps P23's one-frame clamp
(rounding is monotonic; collapse can only
mean equality). Overlaps are valid
everywhere; queries follow stored order
verbatim, and an invalid stored cue
(reachable only from a hand-edited file)
can never be live; eight pins cover the
settlement on both surfaces. **R3 —
P32's lane drag-to-move is built.** A cue
block drags in time (body preserves
duration; either edge resizes under the
timeline's own 8 pt edge rule) under
exactly one undo step per completed drag,
staged lazily at the first effective move,
cancelled drags rolling back through one
`undo()`; no re-sort mid-gesture, rows
pinned to the grab so the element path the
runtime captured cannot flip, an identity
guard abandoning the gesture if another
surface rewrites the cue mid-drag. No
`main.rs` action was needed — the gesture
record rides a `thread_local` (the
timeline chips' own precedent), so no
State field and no literal sweep. The lane
header now resolves through
`tr(s, "subtitles")` on R4's key. Fourteen
pins. **R20 — the shell↔MCP map, three
gaps closed.** The style was write-only
over MCP (no tool reported it, so a
faithful one-field change was impossible);
`subtitle_list` now carries the style, and
both replace tools (`subtitle_set_style`,
`subtitle_update`) now mirror the panel's
caller-side guard: a no-change call spends
no undo step and reports `changed: false`.
Census unchanged (no new tools); the io
module needed no behaviour change.

**i18n wiring spreads (R4, R5, R6).**
**R4 — the catalogue grows 99 → 151 keys
× 28 tables** (verified programmatically:
identical sequences, zero value==key, zero
loss): a 30-key subtitle block appended to
every table, plus IA-R6-1's 22 camera keys
applied at R6's own anchor (below).
Genuine translations in the eight
translated tables, English placeholders in
the other nineteen — the tables' own
convention. The Subtitles panel is wired
through `tr()` end to end with English
rendering byte-identical (pinned per
label); its status sentences stay English
by design — the shell localises no status
sentence anywhere, and several are pinned
verbatim. **R5 — the export panel wired
where the catalogue's English is
byte-identical to the label shown** (five
sites; three of Q14's dead keys —
`export_svg`, `spritesheet`, `export_webm`
— live again). The rest is named gaps, not
wired at the cost of changing English: six
section headings and eight buttons have no
key at all, and six keys exist whose values
are different strings from the panel's
labels (`export_png` = "Export PNG" vs the
button "PNG", and its siblings) — fixing
those is a catalogue decision, recorded in
r5.md. **R6 — the camera panel wired at
its two exact matches** (`camera_delete_key`,
`clear_key_no_undo`), and its IA-R6-1
proposed the 22 keys the panel actually
needs; R4 applied them (above). Wiring the
camera panel to those new keys is a later
pass — R6 did not pre-wire keys that did
not yet exist.

**Media and export completeness (R8, R9,
R10, R13).** **R8 — segment-text alpha,
P17's last remainder, settled WITHOUT a
model change.** The board's wording placed
a stored field on the model; R8's audit
found `MediaItem` lives in `src/media.rs`
(it has never lived in `model.rs`) and
that the alpha is an *evaluated* value —
the carrier node's already-stored
`style.opacity` — exactly like the image
variant's alpha beside it. A stored field
would have been unauthorable dead state
(no panel control or MCP argument writes
it): the fake-field pattern contract 5
exists to prevent. The settlement pins the
parity value as style opacity alone, NOT
the figure × style product images carry
(the stage's figure fade is a colour
blend; a product would manufacture a new
divergence). SVG emission landed in
`src/svg.rs` under the image arm's exact
rule; the stage needed no code (it is the
reference surface; its docs now state the
settlement instead of the closed
divergence); the raster threading is
IA-R8-1 (below) — **the tree does not
compile until the integrator applies it**,
stated plainly in r8.md. The legacy
guarantee rides the existing serde default
on `NodeDef.style`: a pre-style file loads
its carriers at opacity 1.0 and renders
byte-identically. **R9 — JPEG import.**
The `image` crate's jpeg feature was
already on; the only refusal was the MCP
funnel's PNG-magic gate. The gate now
accepts PNG signature or JPEG SOI by
content (a JPEG named `.png` imports; the
pin proves it), the stored-bytes model is
unchanged, and GIF stays refused
deliberately — widening the accepted set
is a scope decision, not a parity fix.
**R10 — export scale is a true re-render.**
The PNG-sequence and still-frame funnels
gained scale entries (0.5 / 1.0 / 2.0 and
any positive finite value): geometry is
re-rendered at `ss × scale`, never
resampled; at 1.0 the output is
byte-identical (the dimension rule reduces
to the historical truncation for every
stage). The audit also exposed that the
MCP single-PNG arm's existing `scale`
argument post-resized a 1.0 render — a
resample wearing a re-render's clothes;
IA-R10-1 rewires it onto the new funnel.
**R13 — frame ranges over MCP, unified
vocabulary, strict layer.** `render` and
`export_file` now accept `from_frame` /
`to_frame` alongside the Wave 11 `from` /
`to` (both spellings for a bound must
agree; a lone bound refuses), and a bound
past the timeline end is REFUSED at the
MCP layer before any file is written —
where the core exporters still clamp,
because their caller is the panel's
pre-clamped range state; the layer
settlement is documented in `mcp_io.rs`.
`export_bundle` ranges were NOT wired:
the panel deliberately ignores ranges for
bundles (its "Export all" label depends on
it) and the MCP tool mirrors that on
purpose — a disagreement reported per
contract 5, not silently settled.

**The verification ring (R11, R12,
R15–R19).** **R11 discharges P12's IA-1**:
`docs/PHASE_O.md` (new) and
`docs/QA_RUNE.md` §17 are written, every
anchor grep-verified against the tree —
except the IA's one CI-dependent half, the
p05 §2 scan table over a real headless
dispatch, which both documents record as
**open, owed to the first headless
debrief**, rather than fabricating it.
**R12 extends determinism to the two drawn
surfaces P47 did not cover**: SVG A-B-A
byte-identity including a live subtitle
cue and graph channels. The stage leg pins
the lib-side evaluated stream — everything
`stage_shape` is a pure function of —
because `stage_shape` itself is a
bin-crate item and Wave P+Q's CI round 1
proved that boundary in an error message;
the bin-side assembly order remains
pixel-proven by the headless census, and a
true in-bin pin is named as a remainder,
not faked. **R15** took the settled
baseline recounts (381 tools; 979 element
keys by the gate counter — 863 static +
116 dynamic; 48 State fields over 22 full
literals) and ratcheted `KEY_TOTAL_FLOOR`
949 → 979 with a ledger line (raise-only
discipline); `MCP_SURFACE_FLOOR` already
equalled the recount at 381. **R16**
needed no new scenes — layout facts assert
keys and counts, which label text cannot
move — but its audit found **facts 32/33
already false**: since Wave Q wired the
lane into the timeline, the
`subtitles-populated` tree carries the lane
over the default tree, contradicting fact
32's "identical inventory" text (the
overflow scan set had treated that tree as
the lane's carrier all along). Both facts
are corrected in `scenes.rs`; this is the
wave's one pre-existing-defect fix in a
document of record. **R17** confirms the
scan set stands at **13 required trees, 0
pending** (R16 added no tree) and corrects
four stale prose statements in
`scripts/overflow_scan.py`; self-test 84
PASS. **R18** wrote the wave's e2e suite
(14 tests): R1/R2/R9/R10 legs against the
landed shapes. **R19** extended P22's
round-trip discipline (7 tests), including
the load-verbatim leg: state the write
paths refuse (zero-length, inverted,
blank-text cues; an over-clamp stored
style) still loads byte-identically —
validation is edit-time and import-time
only, which matches R2's settlement
exactly.

**One contract collision, recorded for the
integrator.** R18's R8 leg and R19's tests
1–3 were written against the board's
literal R8 wording (a stored `alpha` field
on `SegmentMedia::Text` with a serde
default). R8 landed the evaluated-alpha
settlement above instead, with the
discrepancy reported in r8.md §2. Those
legs cannot compile against the landed
shape; the reconciliation is mechanical
and one-directional — rewrite them against
r8.md §5's contract (the item stream
carries the alpha; R19's test 4 and R8's
own pins already cover the evaluated path,
and legacy identity rides the
`NodeDef.style` default). No defect is
alleged in any of the three slices; it is
a board-wording casualty, named here so it
is fixed at integration rather than
discovered in a CI log.

**Integration ledger — every Integrator
Action filed across the findings.**
(1) **IA-R8-1** (`src/media.rs`, six parts,
exact code in r8.md): the `MediaItem::Text`
alpha field, both construction sites,
`composite_media` / `composite_text`
threading, two test patterns — REQUIRED
before the tree compiles. (2) The R18/R19
reconciliation to R8's contract, above.
(3) **IA-R9-1**: `attach_image_to_node`
accepts `jpg` / `jpeg` beside `png` in
`src/editor.rs`, with its status sentence,
plus one comment line in `media.rs`.
(4) **IA-R9-2**: the `asset_import`
description in `src/mcp_tools.rs` and its
`docs/MCP.md` row move together (exact text
in r9.md). (5) **IA-R9-3**: SVG export
embeds every asset as `data:image/png` —
a JPEG asset is mislabelled there; the
MIME must follow the bytes (exact fix in
r9.md; sequence against R8's `svg.rs`
work). (6) **IA-R10-1** (`src/mcp_io.rs`,
after R13's edits to the same file): the
single-PNG arm renders truly at scale, the
sequence arms accept `scale`, the
registry's render wording follows.
(7) **IA-R10-3**: on a non-1.0 scaled
frame, the subtitle burn-in still lays out
in stage space — feed R1's layout the
output dimensions so the burn-in scales
with the frame; until then R10's pins are
deliberately cue-free. (8) **IA-R13-1**:
`docs/MCP.md`'s `render` and `export_file`
Purpose rows (exact replacement rows in
r13.md). (9) **IA-R20-1**: `docs/MCP.md`
subtitle-tool prose for R20's three fixes
— as filed, r20.md ends mid-sentence at
its line 156, so the exact replacement
text must be recovered from the module
descriptions (or a completed r20.md)
before applying; the gate does not block
on it (its doc sync is name-set based).
(10) **IA-R1-1**: one comment-tense line in
`stage_text.rs` ("joins them via
IA-P26-1" is now past) — check against
R8's doc edits in the same file first.
(11) The integrator's own `editor.rs`
test-import fix from the base's CI round 2
rides this integration. (12) `i18n_wire.rs`'s
header still reads "99 keys" (stale since
Q14; now 151) — a header-truth tidy for
the integrator. Closed inside the wave:
**IA-R6-1**, applied by R4. No actions
were filed by R2, R3, R11, R12, R16 or R17.

**Verification — awaiting CI.** Counts in
this section were verified from the tree
at write time, not from findings' claims:
the census script re-run reads **381
tools** (44 + 319 + 9 + 4 + 5) — Wave R
added no tools, so README's count line
does not move and README is unchanged by
this pass; R4's programmatic audit reads
**151 i18n keys × 28**; the gate counter
reads **979 element keys** (zero lost;
no Wave R slice added any), with the floor
ratcheted to match by R15. R5, R6, R13,
R15 and R16 each report the gate at PASS
10/10 on the in-flight tree at their check
times; the integrator re-runs gate +
census on the settled tree before
committing, and CI (Build, Test & Binary +
a headless dispatch) is the proof of
everything above — including the first
measurement of the revised fact 32 and of
R1's re-derived burn-in bands.

**Honest remainders.** (1) Three findings
were outstanding at write time: **r7**
(the undo-contract census — its suite
`tests/wave_r_undo_census.rs` IS in the
tree; nothing beyond the file's presence
is claimed), and **r21 / r22** (the two
assignments the user's "Build both"
decision added mid-wave; `src/shell.rs`
carries an in-flight modification whose
attribution their findings will settle).
This section should be amended when they
file. (2) R5's export-panel gaps (headings
and buttons with no key; six keys whose
values differ from the labels) and R6's
camera-panel wiring pass await catalogue
decisions in a later wave; subtitle and
camera status sentences likewise stay
English by the shell's standing rule.
(3) An export-scale control in the panel:
the core is ready (R10) and no control was
invented — a product decision
(IA-R10-2). (4) Ranged `export_bundle`:
a reported disagreement between what the
core could do and what both surfaces
deliberately do; a user decision, panel
first. (5) The p05 §2 scan table remains
owed to the first headless debrief (R11).
(6) A bin-side `stage_shape` determinism
pin has no owner this wave (R12). (7) A
tall subtitle stack still walks off the
far edge on all three surfaces — the
stack guard is a cross-surface decision
(R1). (8) Two §6.11 remainders close
here: segment-text alpha (settled by R8,
without the media-model change §6.11
expected) and P12's IA-1 (discharged by
R11, scan table excepted). The standing
user decisions otherwise stand as §6.11
left them — project-swap undo,
white-on-accent contrast, the two
"New project" results, the waveform call;
the speaker field and the recovery
content-comparison gate left that list on
2026-11-10 by the user's own word and are
R21/R22 above. And sound remains
user-verified only.

### 6.13 Wave S — parity + proof burn-down 3: Wave R's remainders closed, scale completed, losses surfaced (2026-11-10)

Ordered 2026-11-10 (11:03 +06) as a
re-issue of the standing continuous order:
"assign 20 agents continuous write code if
ci gets error then fix that". Twenty
agents, one owner per file, integrator
surfaces via filed actions (board:
`~/workspace/wave-s/BOARD.md`; findings:
`~/workspace/wave-s/findings/`). The
board's base is the Wave R integrated
tree `59cb0be`; the working base is
`85898b4`, that tree plus its round-2 CI
fix. This section was written from all
nineteen sibling findings (s1–s16,
s18–s20; s17 is this section), each read
in full. Every status below is
IMPLEMENTED (in tree, uncommitted at
write time), and the CI frame is stated
once, exactly, here. **The Wave R tree
(`59cb0be`): headless 42759195547 GREEN.**
Build round 1 (42759195059) FAILED on
one bin-test compile error — a
`sorted.dedup()` on a `[u32; 8]` array in
a subtitle-panel palette pin, latent
since Wave Q because every earlier round
had died at the lib target first; fixed
in `85898b4` (the test collects into a
Vec; the assertion is unchanged). Build
round 2 (42759845639, on `85898b4`)
FAILED further along: everything
compiled, 1,562 lib tests passed, and
exactly one failed — R20's `set_style`
refusal test, whose own unwrap of
`serde_json::from_str` on a `1e999`
literal panicked because serde_json
refuses that literal itself ("number out
of range") before the handler under test
ever ran; the integrator fixed the test
in `14bd9b3` (committed alone, mid-wave:
the leg is now the always-reachable
1e300 f64→f32 overflow, same refusal
sentence, plus a guarded `1e999` leg).
Build round 3 (42760562852, on
`14bd9b3`) completed FAILED one step
deeper still: the lib (1,563/1,563) and
bin (2,102/2,102) suites were both fully
green — the R20 fix holds — and the sole
failure was an integration test,
`full_fixture_round_trip_is_lossless_per_section`
(`tests/wave_p_roundtrip.rs`), another
latent test-side typo: its non-vacuity
marker `"ünïcode-♪.ogg"` omitted the
fixture's `audio/` path prefix, so the
quoted substring could never occur in
the serialised project; fixture and
serialisation were always correct.
Fixed in `1d724a2`; build round 4
(42761122064, on `1d724a2`) is **in
flight at writing**. **Wave S itself
has no CI verdict**: nothing below is
called shipped, and its first verdict
comes on the integration push.

**Export-panel i18n closed (S1).** R5's
filed no-key gaps are closed: sixteen
keys appended as one block in all 28
tables — the six section headings
(Images / Animation / Video / Project
formats / Bundles / Recent exports), the
JPEG / APNG / Contact sheet buttons, the
`.nodes` / `.stknds` buttons, All
frames, the two bundle buttons, the
empty-state line and the header subtitle
— wired at sixteen sites through `tr()`
with English byte-identical (the pin's
pair list grows from R5's five to
twenty-one). The catalogue moves **152 →
168 keys × 28** (verified mechanically:
identical sequences, no value == key,
zero loss; genuine translations in the
eight translated tables). What S1 did
NOT wire is filed as seven
value-vs-label discrepancies, neither
side changed: `export_png` / `export_gif`
/ `export_mp4` ("Export PNG" in the
catalogue vs the button "PNG", and its
siblings), `export_stk`, `export_piv`
(one key, four versioned labels
".piv v2"–".piv v5"), `hud_loop` ("Loop"
vs "Loop range"), and `export_centre` —
a key with no call site anywhere in the
shell, retained under zero key loss
while the header line it half-covered is
now `export_subtitle`. S1 files the
three options (panel-shaped short keys
later; change catalogue values after a
consumer audit; change the panel labels
— a UI wording call) for whoever
decides. R5's class C stands as a
remainder: the panel's sentence-length
copy (guidance notes, recaps, the
history `row_line` / `age_label`
strings, `ExportOp::label()`'s format
names) remains English with no keys.

**The next panel wired (S14).** S14
audited every file under
`src/rune_sec/` for unwired
constructor-label literals against
existing `tr` sites and chose the worst:
**the 3D panel** — 34 wireable labels at
49 call sites, zero prior wiring
(frame_ops ≈26 and audio_panel ≈29 are
the recorded runners-up). All 34 are
wired through `tr()` with English
byte-identical — nine section captions,
the session toggle (its helper now takes
`&EditorCore`, the CamKeyRow::label
precedent), the view / limb / bake
buttons in both their dimmed and live
branches, the library controls and three
empty states — with handlers, layout and
the 29-key element set untouched.
Locale-invariant notation stays literal
by the R6 rule (the title `3D`, the `3/4`
preset, the `RotX +15` step labels, the
−/+ steppers). The 34 `threed_*` keys
themselves are **IA-S14-1** (below): the
panel's pin passes only once the
integrator inserts them, the established
Q7 pattern of wiring and keys landing
together. The panel's composed readouts
(`session_summary`, `joint_position_text`,
`camera_readout_text`) stay English —
they are format-template text, and
wiring them means placeholder template
keys, a future slice's call.

**Speaker proof completed (S2, S3,
S12).** Wave R shipped the speaker field
with converters pinned but no renderer
proof; this wave closes that on every
surface. **S2** seeds a new scene,
`subtitles-speaker` (registry
index 31; one credited cue, "The light
is still on.", speaker `"NOVA"`, default
Bottom style, playhead parked as in
`subtitles-populated`), with layout
**fact 34** keeping the scenes document
truthful (its tree's inventory is the
populated tree's; the lane is in it) —
scenes 31 → **32**. Its census leg
measures the prefix as line widening:
the credited line runs (4·28 − 1)·4 =
444 units against the bare text's 348,
and the gate is span ≥ 396 units — the
census's usual conservative fraction,
derived analytically like its floors
before it; the first headless run
prints the measured span (≈440
expected), and S2 names the derivation,
not the gate, as the thing to
re-examine if it lands far off. **S3**
gives the subtitle lane its block
labels: one composition site
(`block_label`, beside `block_shows_text`)
prefixes `Name: ` for credited cues,
display only — block geometry derives
from frames alone (pinned: a credited
cue's `cue_rect` equals its uncredited
twin's), the drag identity guard keeps
comparing the raw stored text, and
uncredited labels are byte-identical to
today's, multi-line bodies unflattened
exactly as the lane has always shown
them. **S12** pins the two lib surfaces
in five tests: the SVG `<text>` contents
of a credited two-line cue are exactly
`["MARA: First line", "Second line"]`
(prefix on the first line only, no added
line); the raster band's row extent is
exactly the uncredited render's while
its width grows by ≥ 48 px of the
nominal ~96; a WebVTT voice tag imported
through the core renders with the
prefix; and the SRT leg is the
non-regression — a credited cue's SRT
carries raw text and no name anywhere.
The stage's own pixel proof is S2's
census (the stage bands are bin-crate,
the R12 boundary), so each surface's
proof sits where it can actually run.

**Export scale completed (S4, S5), and
its burn-in pinned (S11).** R10's filed
next step is done: every remaining
export format now scales. **S4** adds
the six animation entries —
`export_gif[_range]_scaled`,
`export_apng[_range]_scaled`,
`export_sheet[_range]_scaled` — each
validating through `check_export_scale`
FIRST (before any clamp or filesystem
work, the PNG sequence's own order) and
each delegating to its unscaled
counterpart at exactly 1.0, so the 1.0
byte-identity is structural, not a
re-derivation; pins cover scaled
dimensions, tile-byte equality with the
scaled frame funnel, three-way 1.0 byte
identity and refusal of 0 / negative /
NaN / ∞ with no file left behind.
**S5** adds the four video entries
(`export_mp4[_range]_scaled`,
`export_webm[_range]_scaled`) on the same
conventions — the MP4 audio mux is
untouched; audio does not scale — and
routes the MCP `render` tool's `scale`
argument for gif / apng / spritesheet /
mp4 / webm (present-vs-absent parsing,
never defaulted into the unscaled call;
replies gain no field, and with `scale`
absent every call and reply is
byte-identical to pre-wave behaviour).
`export_file` was deliberately not given
a scale argument — it has none for any
format today, and its `mp4_range` /
`webm_range` remain the video-range
surface. **S11** supplies IA-R10-3's
missing pixel pin at last: four legs
deriving the scaled burn-in's geometry
from the shared constants (face 20·s
rows, edge offset 28·s, stack step 24·s)
and asserting both the derived ranges
and the relation to the same project's
measured 1.0 render — face doubling at
2.0 and halving at 0.5, the speaker
prefix scaling with its line (same rows,
~3.1× the column extent), and the stack
step doubling without the two faces
colliding. No defect was found by
inspection: the three `* scale` sites
stand in the tree exactly as the IA
prescribed. Standing remainders: the
contact sheet and the JPEG sequence
remain 1.0-only (the contact sheet's
writer is a different one, with its own
size notion — scaling it is a separate
design question); video container
dimensions are not ffprobe-pinned (CI
does not guarantee ffprobe, and the
codebase is written to tolerate its
absence); odd output dimensions at
fractional scales inherit libx264's
even-dimension rule exactly as an
odd-sized stage already does unscaled.

**The documented loss, surfaced (S9).**
Wave R documented that SRT export drops
cue speakers (the format has no speaker
syntax); the loss is now visible at the
moment it happens. The core gains a
counted export, `subtitles_to_srt_counted`,
returning the document and its
`speakers_dropped` count from the same
pass — `subtitles_to_srt` is a one-line
delegate, so the text output is
byte-identical by construction and no
existing caller or pin moved. The
counting rule is deliberately not the
WebVTT guard's: a stored name counts
when it is a real credit (non-empty
after trim), even one a voice tag could
not carry. Over MCP,
`subtitle_export_srt`'s reply gains
`speakers_dropped: N` **only when
positive** — additive, absent otherwise,
the rest of the reply byte-identical —
and the panel's SRT success line grows
the clause "— N speaker credit(s) not
written (SRT has no speaker syntax).",
its zero-drop and WebVTT lines
byte-identical to before. The VTT paths
are untouched. Import-side, the loss
still surfaces only as `changed: true`
(R21's settlement) — export-side was
this slice's board.

**Home's missing-audio badge (S7) and
the close-flush composition (S13).**
**S7** audited first: the badge was
ABSENT — Home's rows carried no audio
signal at all, and missing audio
surfaced only after opening (the P14
status note, the P15 export report, the
Audio panel's marked rows). It now
rides the panel's existing light read:
the same parse that counts figures and
frames deserialises only the `audio`
array and classifies it with the ONE
shared helper (`audio::missing_audio_clips`)
the open note, the export path and the
Audio panel already use, so the marker
can never disagree with what opening
the project would report; a non-zero
count closes the row's meta line with
"· N audio clip(s) missing", and at
zero the line is byte-identical to its
pre-wave shape. The scope is stated in
the module docs: light-read rows only
(the scan-fallback line has no audio
data, and a health claim there would be
least trustworthy), no full `Project`
load per row (the light read exists to
avoid exactly that), zero new element
keys. **S13** pins the P19 × R22
interaction that had no pin — the
flush form of the P42 spurious offer:
an edit made after a save and then
undone leaves `edit_count` standing
while the project is again exactly the
saved content, so the close-flush
writes a snapshot of the just-saved
project, strictly newer by mtime. Four
pins drive the real verdict function on
flush-shaped state (identical → no
offer, with the mtime-only verdict
asserted WOULD offer; differing →
offer; corrupted after close → offer,
fail-open; Discard deletes and flips
the verdict). The one link not claimed
is named precisely: the real flush
writes only through the CWD-relative
`./projects` and there is no
directory-parameterised flush entry, so
the single-call composition (real flush
→ verdict) belongs in the integration
binary `tests/wave_p_close_guard.rs`,
where P19 already solved the CWD
problem — filed as a test-only
follow-up in the ledger, not papered
over.

**Phase O's last proof, discharged
(S8).** The p05 §2 scan table owed
since R11 is written into
`docs/PHASE_O.md` §10 from the real
dispatch — headless run 42759195547's
artifact (204 files, 34 `tree-*.json`
dumps, on `59cb0be`): the scanner reads
**CLEAN — 0 non-exempt hits on all 34
trees, scan set complete** (all 13
then-required trees present, so the
debrief is complete by the
machine-enforced contract, not clean on
whatever happened to be there). Every
repaired row was re-measured in the
dumps: the worst settings chip now ends
at 1392.5 (pre-fix 1463.0, past the
window), `cam-delete-key` at 1193.9
(was 1441.4), `layer-row-0` exactly at
the 304.0 content edge, the wrapped
draw / graph / weather / fx rows ending
exactly at the 1412.0 content edge; one
absence is recorded, not smoothed —
`graph-actions` does not render in
tree-default (its proof tree is
graph-duplicates, as p05 named). The
table proves the 1440×900 and 1280×720
headless font metrics only; p05's own
caveat stands.

**The tall-stack question, answered
(S20).** The subtitle-stack off-stage
remainder carried since the Tauri line
("raster clips ~the 13th line, SVG off
its viewBox ~the 7th") **does not
reproduce on this tree — the verdict is
PARITY, and no production code was
changed**. All three surfaces place
every line at the same centre from the
shared helpers (the SVG writer restates
the centre formula inline, term-for-term
identical — verified constant by
constant, sanitisation included), so
the first clipped line is the same line
everywhere at any stage height. The
arithmetic is now pinned in
`src/subtitles.rs`'s tests: at the
default style a 200-unit stage holds 7
full lines + 1 partial (the 9th paints
nothing), 400 holds 16 + 0 (the
boundary case, (400 − 16) / 24 = 16
exactly), 540 holds 21 + 1, 720 holds
29 + 1, and the margin does not scale
with font scale (at scale 2.0 on 200:
3 full + 1 partial). What stands as a
remainder is the product question, not
a defect: no wrap, pagination or stack
cap exists on any surface, so beyond
the ceiling far lines are uniformly
unpaintable — whether an over-tall
stack should shrink, scroll or refuse
is a decision, not a parity fix.

**The verification ring (S6, S10, S15,
S16, S18, S19).** **S6** closes the
raster's determinism gap with five
A-B-A byte-identity legs — plain frame,
subtitled frame, credited cue,
segment-text alpha at 0.35, and the
subtitled frame at scale 2.0 and 0.5 —
each with a provably-different B render
interleaved; its header also corrects
the record on P47 (the older suite pins
raster REPEAT only, not the A-B-A its
successor's header credited it with).
No nondeterminism was found by
construction review, and no leg was
weakened. **S10** legs JPEG parity end
to end (four tests): import by content
from a file named `photo.bin`, stored
bytes verbatim through save → load with
dims intact, rendered pixels after the
round trip (JPEG centre within ±16 per
channel — a tolerance derived from the
codec, with the PNG control asserted
EXACT — and the SVG embed decoding back
to exactly the stored bytes under the
`image/jpeg` MIME), and the
magic-over-garbage refusal persisting
nothing. **S18** composes the wave's
features (four tests): credited cue +
range export at scale 2.0 (files
pixel-identical to the direct scaled
renders, differing from the control
only inside the scaled band, and only
on the cue's frames); JPEG asset +
credited cues + non-default style
round-tripping byte-identically through
both serialisation and the real file
path; the speaker surviving the
VTT → SRT → VTT flow with SRT as the
only lossy hop; and segment-text alpha
and a speaker subtitle composing in one
frame independently (disjoint pixel
populations; the blend count EXACTLY
equal between credited and uncredited
renders). **S19** applied the wave's
two docs actions (the ledger's (2) and
(3)) and swept every description-bearing
surface: exactly two tool descriptions
changed all wave (`render`,
`subtitle_export_srt`), both rows now
aligned verbatim, census and gate
check 9 green after the edits.
**S16** applied IA-S2-1 in full (below)
and records the sequencing as one
unit: S2's scene, the census leg, the
workflow dumps and the required
SCAN_SET entry land together or CI
fails by design. **S15** recounts —
its numbers are the verification
paragraph's, below — and records the
gate working as intended mid-wave:
check 8 went red with exactly the right
message while S2's scene existed before
S16's workflow half, and green again
when it landed.

**Integration ledger — every Integrator
Action filed across the findings.**
(1) **IA-S2-1** (S16 applies:
`.github/workflows/headless.yml` +
`scripts/overflow_scan.py`): both tree
loops gain `subtitles-speaker`, the
mirrored snap pair is added, and
`tree-subtitles-speaker.json` enters
SCAN_SET as **required** — CLOSED
inside the wave by S16; the set stands
at 14 entries, 14 required, 0 pending.
(2) **IA-S5-1** (`docs/MCP.md`'s
`render` row: one sentence inserted,
verbatim) — CLOSED inside the wave by
S19. (3) **IA-S9-1** (`docs/MCP.md`'s
`subtitle_export_srt` Purpose cell
replaced with the tool's new
description, verbatim) — CLOSED inside
the wave by S19. (4) **IA-S9-2**
(integrator applies): S9's panel status
pins, exact code in s9.md, inserted
into `src/rune_sec/subtitle_panel.rs`'s
test module after
`export_writes_the_document_and_import_replaces_once`
— S9 was barred from that test region
because the integrator's palette-pin
fix sits there. (5) **IA-S1-1** and
(6) **IA-S15-1** are the same edit seen
from two slices: `src/rune_sec/i18n_wire.rs`'s
header count moves off its stale 152 —
S15 supplies the exact replacement
text in both forms (202 if IA-S14-1
lands in the same integration, 168 if
it is deferred); apply once, with the
final count. (7) **IA-S14-1**
(integrator inserts into `src/i18n.rs`):
S14's 34 `threed_*` keys as one block
× 28 tables after S1's block, genuine
translations in the eight translated
tables per contract 6 — the catalogue
becomes **202 × 28**; S15's companion
note rides with it (`src/i18n.rs`'s own
module-doc line "168 keys" moves to 202
in the same commit). Until it lands,
S14's wiring resolves through the
catalogue fallback and its pin cannot
pass — expected, per the Q7 pattern.
(8) **IA-S8-1** (integrator applies):
the `docs/QA_RUNE.md` §17 row for the
p05 §2 scan table is replaced with the
closure row, exact text in s8.md.
(9) **IA-S20-1** (optional hygiene, zero
behaviour change): `src/svg.rs`'s
`append_subtitles` may retire its
inline restatement of the centre
formula and call
`subtitle_line_center_y` directly —
exact replacement in s20.md; the
writer's existing pins are the proof
that output is byte-identical. It is
not a divergence fix; it retires the
one place a future divergence could be
born. (10) **S13's follow-up**
(test-only, no code owed): the
one-call close-flush → verdict
composition pin recommended into
`tests/wave_p_close_guard.rs` (see the
S13 paragraph). No actions were filed
by S3, S4, S6, S7, S10, S11, S12 or
S18.

**Verification — awaiting CI.** Counts
are S15's, recounted on the settled
live tree (HEAD `14bd9b3` plus all
landings), not taken from siblings'
claims: the gate reads **PASS — all
checks green (10/10)**; the MCP census
reads **381 tools** (44 + 319 + 9 + 4 +
5), 16/16 checks PASS, and
`MCP_SURFACE_FLOOR` stays 381 (S5/S9
changed behaviour and descriptions,
never the surface); **element keys 980**
(864 static + 116 dynamic) with zero
loss per key against HEAD, and
`KEY_TOTAL_FLOOR` is ratcheted 979 →
980 to equal the verified count — the
+1 over R15's 979 is
`subtitle-speaker-field`, added by Wave
R's own integration (IA-R21-5) after
R15's recount, verified by S15 on a
clean base worktree. Palette commands
134; curated icons 163; headless scenes
**32**; State 48 fields over 22 full
literals. The i18n catalogue in tree
reads **168 keys × 28** — exactly S1's
+16 block over the 152 base, sequences
identical, base sequence preserved in
full — and becomes 202 when IA-S14-1
lands (labelled post-integration
arithmetic wherever it appears; S15
verified `grep -c threed_ src/i18n.rs`
= 0 in tree). SCAN_SET stands at 14
required / 0 pending with its
self-test green (S16). S19 re-ran the
census and the gate after its docs
edits: both green, check 9's doc sync
exact. CI — the integration push's
Build, Test & Binary plus a headless
dispatch — is the proof of everything
above, including S2's analytically
derived census gate and S11's derived
geometry.

**Honest remainders.** (1) The standing
user decisions stand exactly as §6.12
left them — white-on-accent contrast,
the panel splitter, File-menu Save As,
the two "New project" results,
`tab_visits_controls`, project-swap
undo, the skew channel, the whole-figure
pose transform, MCP `write_recovery`,
the waveform call; this wave touched
none of them, per its contract 9. (2)
**GIF import stays undecided**: the
funnel still refuses GIF deliberately;
widening the accepted set is a scope
decision for the user, not a parity
fix. (3) **A panel export-scale control
remains unbuilt by design** (IA-R10-2's
product decision): the core and MCP
scale surface is now complete — png,
png_sequence, gif, apng, spritesheet,
mp4, webm — and no control was invented
to front it. Ranged `export_bundle`
likewise stands: S5 confirms the
bundle's PNG member carries no scale
routing this wave and stays at 1.0.
(4) S1's seven value-vs-label
discrepancies stand for a catalogue
decision (the export-panel paragraph
above); the sentence-length class C
copy stands with them. (5) S14's
composed readouts stay English, and the
next unwired panels are named in
s14.md for a future slice (audio_panel
the natural next pick; frame_ops,
inspector_segment, fx_clips,
particles_panel behind it). (6) S13's
composition gap stands until the
follow-up pin in (10) lands: the flush
half of its pins is seamed at
`write_recovery_in`, the flush's own
dirty/clean behaviour is P19's pin, and
the verdict half is driven for real —
the unproven link is the composition
itself, stated, not hidden. (7) No
subtitle stack cap exists (the S20
paragraph): uniformly unpaintable past
the pinned ceiling is now the proven
behaviour, and changing it is a product
decision. (8) S10's ±16 JPEG tolerance
and S2's census span gate are both
calibrated by derivation, not by a run;
if CI contradicts either, the findings
name the number to revisit (the
tolerance; the span derivation) — and
S10's leg-1 verbatim-bytes assert
already isolates persistence from
decode either way. (9) S7's badge
deliberately does not ride the
scan-fallback meta line, and names a
count, not paths (the open verdict and
the Audio panel name paths already).
(10) §6.12 remainders closed here: R5's
export-panel no-key gaps (S1 — the
discrepancy half stands, per (4));
IA-R10-3's unpinned scaled burn-in
(S11); IA-R21-4's renderer proof (S2's
census for the stage, S12 for SVG +
raster, S3 for the lane's labels); the
p05 §2 scan table (S8 — the last item
R11 left open); the tall-stack
off-stage question (S20: parity, no
divergence to fix); and R10's filed
scaled-funnel next step (S4/S5). And
sound remains user-verified only.

### 6.14 Wave T — i18n completion, the scale matrix closed, proof deepening (2026-11-10)

Ordered 2026-11-10 (11:22 +06) as a
re-issue of the standing continuous
order — the sixth time the user has
issued it: "assign 20 agents continuous
write code if ci gets error then fix
that". Twenty agents, one owner per
file, integrator surfaces via filed
actions (board:
`~/workspace/wave-t/BOARD.md`; findings:
`~/workspace/wave-t/findings/`). The
board's base is the settled Wave S tree
— HEAD `e0eea5a` plus Wave S's
uncommitted landings plus the F2
finisher's IA package — and F2's files
(`src/i18n.rs`,
`src/rune_sec/i18n_wire.rs`,
`docs/QA_RUNE.md`, `src/svg.rs`,
`tests/wave_p_close_guard.rs`, the
subtitle panel's test region) were
frozen to this wave's agents. This
section was written from all nineteen
sibling findings (t1–t16, t18–t20; t17
is this section), each read in full, and
from the F2 findings
(`~/workspace/wave-s/findings/f2.md`).
Every status below is IMPLEMENTED (in
tree, uncommitted at write time), and
the CI frame is stated once, exactly,
here.

**The Wave S CI frame, completed.**
Rounds 1–3 are §6.13's record. Build
round 4 (42761122064, on `1d724a2`)
FAILED: five undo-census tests died in
setup because R7's hand-rolled 1×1 PNG
fixture carries a bad IDAT CRC — it
passed while asset import checked magic
only, and Wave R's decode-checked import
funnel (R9) rightly refuses it. Fixed in
`e0eea5a` (the fixture is now
encoder-made via the image crate, the
wave_r_e2e idiom; import strictness
unchanged). Build round 5 (42761838001,
on `e0eea5a`) FAILED one level deeper:
the failing target was again
`tests/wave_r_undo_census.rs`, which ran
19 tests with 15 passed and 4 failed —
`census_mutating_misc_spend_one_step`,
`census_mutating_fx_particles_audio_spend_one_step`,
`census_exceptions_spend_no_step_but_really_write`,
`census_figure_library_rename_spends_no_step`.
The integrator's fix (`7f9bce5`,
verified against the failed log at
writing) names four causes, all
test-side: the audio_split leg's stale
args (it passed no clip index; the
handler requires one), an exceptions
witness asserting absolute undo depth 0
after a setup whose clip_create
legitimately spends a step (now measured
relatively, like the fig3/figlib
witnesses), figlib legs hardcoding
'Styk' where the library names disk
entries by file stem and the save
lower-cases it to 'styk', and the
suite's legs racing on parallel threads
over the shared CWD-relative
`./library`, `./projects` and `./import`
stores (the suite is now serialised on a
static mutex, the wave_p_close_guard
precedent). Build round 6 (42762884625,
on `7f9bce5`) is **in flight at
writing**. The Wave S integration push
does not exist at writing: **the Wave S
tree stands as integrated in tree, push
pending integrator** — the runs above
are the pre-integration fix chain on
committed trees that carry none of the
Wave S landings (39 paths uncommitted at
writing, Wave T's landings among them).
Wave S's integration ledger itself is
closed by F2 (below). **Wave T itself
has no CI verdict**: nothing below is
called shipped, and its first verdict
comes on the integration push.

**Export class-C i18n wired (T1).** The
export panel's sentence-length copy —
R5's class C, which §6.13 left standing
— is wired where it is honestly
wireable: 35 new keys at 35 production
call sites, 26 plain + 9 templates
(`{from}`/`{to}`, `{count}`,
`{frames}`/`{width}`/`{height}`/`{fps}`,
`{name}`/`{size}`/`{age}`), English
byte-identical, filed as IA-T1-1. Of 58
inventoried user-visible strings, 23 are
filed, not forced: the 19 unique
range-action status literals and the 5
recap builders stay English because they
are machine-read — `export_failed` and
`toasts::classify_status` parse their
English shapes, so a translated failure
would be misread as success in every
non-English locale — and
`ExportOp::name()` values are slot data,
not label sites. The notation buttons
and the `Loop range` toggle are excluded
(the toggle is one of S1's seven
discrepancies, contract 8). The pins
pass only after IA-T1-1 lands (the Q7
pattern), and `export_centre` remains
without a call site, untouched.

**Audio panel wired (T2).** All 33 label
sites resolve through the catalogue: 31
constructor labels in the view + the
mute helper's 2. The exact recount
corrects S14's audit (31 + 2, not ≈26 +
2 — the panel_title meta, the name-field
placeholder and the icon-only Delete
clip help label were not itemised
there). `audio_clip_count` is the
block's one template (`{count}`, the
camera `follow_label` precedent). The
helper change — `mute_toggle_parts`
gains `&EditorCore` and returns a
resolved `String` label, the S14
`toggle_label` precedent — has a second
caller in `menus.rs`, filed as IA-T2-2
with four exact edits: **the tree does
not compile until IA-T2-2 lands in the
same integration pass as this wiring.**
Excluded: status sentences, composed
readouts, and `effects_settings_line()`
(a 144-char class-C sentence, named as
the first item if a class-C pass ever
covers this panel). menus.rs's own
Edit-menu labels stay English — not this
slice's file, not in the roster.

**Frame ops wired (T3).** 24 distinct
labels at 41 call sites, all inside the
panel's one render function; no helper
needed a signature change. The count
reconciles with S14's ≈26 exactly: the
difference is the two hold-multiplier
labels (`×0.5`, `×2`), classified as
notation here and left literal with the
stepper glyphs. The three dynamic
readouts (`range_label`,
`duration_label`, the Insert count) stay
English — template-key work, named for a
future pass. IA-T3-1 files the 24
`frame_*` keys; the pin passes only
after it lands.

**Segment inspector wired (T4).** The
sweep reproduces S14's count exactly: 24
label-constructor literals, becoming 22
distinct `segment_*` keys at 25 call
sites (Length/Thickness/Bend each serve
two sites through one key). One composed
label is wired in the same pass: the
populated header title becomes the
template `segment_title_node` (`Segment
{index}`, substituted via `.replace()`,
the camera precedent). Legacy lookalike
keys (`add_segment`, `shape`,
`delete_segment`) were deliberately not
reused — the wave discipline is
panel-prefixed keys. Degree steppers and
the readout content of
`shape_label`/`media_label`/`format_topology`
stay as classified in t4.md. IA-T4-1
files the block.

**FX clips wired (T5).** 28 distinct
labels at 30 sites — S14's 24 plus the
empty-state line, the no-selection
guidance line, the anchor hint and the
two gradient state labels, each itemised
in t5.md so the count is auditable. The
flagged kind-name case is settled with
reasons: the snake-case kind name is the
engine's serialised/MCP vocabulary,
never a literal in this file; the
display kind name is
`fx_browser::kind_label`'s output,
another module's function, used here
only in dynamic readouts — wiring it is
fx_browser's slice if ever ordered, and
T5 verified nothing parses its output
back, so no serialisation hazard blocks
that future slice. IA-T5-1 files the 28
`fx_*` keys, none a template.

**Home's scan-fallback line gains the
marker (T6).** §6.13 remainder (9) is
closed: the scan already holds the whole
parsed project at the moment it builds
its summary, so `ScanSummary` gains the
missing-audio count from the one shared
classification
(`audio::missing_audio_clips` — the diff
adds zero `.exists()` calls and exactly
one helper invocation), and
`ProjectInfo` gains `missing_audio:
Option<usize>`. The `Option` is the
honesty discipline made structural:
`Some(0)` is verified health, `None` is
unknown-not-zero for entries no scan
ever parsed, an unparseable file still
produces no row at all, and both
`Some(0)` and `None` leave the fallback
line byte-identical to its pre-Wave-T
shape — for opposite reasons, pinned
separately. The fallback marker inherits
the scan cache's staleness contract
(re-verified when the project file's
stamp moves), the same contract every
other scan field carries; the light-read
line remains render-time-fresh. No IA
filed.

**JPEG sequence takes scale (T7).**
`export_jpeg_sequence_scaled` + its
range sibling land in export.rs
mirroring the PNG-sequence scaled
entries: `check_export_scale` runs
before delegation, clamping and any
filesystem work; at exactly 1.0 each
entry delegates to its unscaled
counterpart (byte-identical by
construction); the scaled path is a true
re-render through `render_frame_scaled`
at the standing quality 90. The render
tool's `jpeg` arm gains the 4-way (range
× scale) routing in the png_sequence
shape, and the render description/schema
now list `jpeg` among the
scale-accepting formats. With this slice
the scale matrix is complete at the
core/MCP level: every raster render
format accepts `scale`; `svg` is the
only no-scale format, by nature. IA-T7-1
files the docs sentence for T18.
`export_file` has no JPEG-sequence
format and no scale argument for any
format — nothing to route there.

**Bundle scale: audited, validated,
pinned (T8).** The brief's premise had
already landed: Wave R's IA-R10-1 routed
`scale` in `tool_export_bundle` (the PNG
member already called the scaled entry;
schema and description already declared
it), and there is no bundle code in
export.rs at all — the bundle is
orchestrated entirely in the MCP
handler. The real gaps were two: no
up-front validation (a bad scale
surfaced only as an `ok:false` member
result after `create_dir_all`, leaving a
**partial bundle behind a successful
call**, and a non-number scale was
silently treated as absent), and no pins
anywhere for bundle-scale behaviour.
Both are closed in mcp_io.rs only: a
present scale is validated before any
write (non-number refuses in the MCP
helpers' voice; a number whose f32 cast
is not positive-finite refuses with
`check_export_scale`'s exact sentence —
`0`, negatives and `1e300` alike, both
bundle kinds), and three pins prove the
PNG member doubles at 2.0 with every
other member byte-identical, absent ==
explicit 1.0 byte-for-byte, and invalid
scales leaving not even the directory
behind. IA-T8-1 files the whole-row docs
replacement for T18. The JPEG member
stays 1.0 by the board's scoping —
routing it would be a cross-slice
dependency on T7's same-wave entry;
whole-bundle scaling is a follow-up if
wanted.

**SVG determinism combos (T9).** Three
A-B-A byte-identity legs in a new tests
file extend R12/S6's discipline to the
combined paths: (a) credited multi-line
cues — the `<text>` contents are exactly
`["MARA: First line", "Second line", "We
go now."]`, prefix on the first line
only, and B (speaker renamed) differs in
the prefix path specifically; (b) a JPEG
asset embed beside a PNG control — each
payload base64-decodes to the stored
bytes verbatim, and B's recolour is
proven local to the PNG asset; (c) both
+ rain + a changed Top/1.5 style — the
weather disc count tracks
`weather_particles` exactly at amounts
40 and 55 while the subtitle contents
stay equal across A and B. The audit
found no nondeterminism source (no map
iteration reaches the document; weather
is a pure function of seeded hashes) and
no leg was stopped or weakened.

**MCP render e2e through the real
registry (T10).** S18 proved credited
cue + range + scale at the core entry;
this suite drives the identical
combination through the layer S18 never
exercised — registry dispatch,
`tool_render`'s argument parsing and
legacy-spelling resolution, R13's
pre-write range validation, S5's scale
routing. Four legs: the framed-spelling
range render at 2.0 writes exactly
`frame-0002.png`…`frame-0005.png` at
960×800, byte-identical to the direct
scaled renders, with the "Ana: " prefix
widening proven on both ink extents
inside the scaled band and out-of-cue
frames byte-identical to the control;
the past-end refusal asserts R13's
sentence verbatim with nothing written;
the scale-0 refusal asserts
`check_export_scale`'s sentence in the
tool's `render to {path} failed:` voice,
nothing written; the legacy `from`/`to`
spelling drives the same scaled credited
render. Undo depth is unchanged across
every call.

**Lane speaker pixel proof (T11).** The
subtitle census gains the lane leg S3's
remainder named: on the speaker snap
pair, the `subtitle-cue-0` block rects
are read from BOTH tree dumps and must
agree within 1.0 pt before any pixel is
measured (geometry is frames-only, so a
label can never move the block). One
measured fact shapes the gates: on the
real populated snap the uncredited
25-char label already saturates the
block's 158 pt text run at ~6.3 pt/char,
so the credited 28-char string truncates
— ink extent cannot isolate the prefix
in the lane, unlike the stage leg. The
leg therefore gates label DIFFERENCE
(`diff_count` inside the block ≥
`LANE_DIFF_FLOOR = 200`) and label INK
(near-white ≥ `LANE_INK_FLOOR = 100`),
with both extents printed every run for
the first-run ratchet; the floors are
analytic, in S2's style, at ~1/3–1/4 of
the measured populated ink (≈400 px at
1×). Crop mode reports the leg as not
run, by design — the crop snaps are
`--crop stage`, and the lane lives below
the stage; the full-window set the
workflow's census step consumes carries
the leg as a hard gate. The self-test
extends 7 → 10 expects, all PASS,
including a negative where the geometry
gate stays green while the label gate
fails by name. T11's harness-scale
observation is carried under remainders
(6).

**Home missing-audio headless scene
(T12).** Scenes CAN seed disk state —
the `home-populated` precedent (real
save op + the save press's recents
follow-up) — and this scene extends it:
one project ("Vanished Take") saved with
one unmuted clip whose file exists
nowhere. The clip is a direct
`project.audio` write before the save,
the honest exception, justified in
t12.md: `add_audio_clip` takes its path
from the first file in `./audio/` and
adds nothing when there is none, so a
missing-file clip is unreachable through
ops — it is exactly the state a user
reaches by deleting a file after adding
it, which is the state S7's marker
exists to surface. The panel's own light
read, never the scene, produces the
count. Fact 35 is the layout table's
first TEXT fact (`home-proj-meta-0`'s
text ends with "· 1 audio clip missing"
— the marker is text, not structure),
the registry stands at 33 scenes, and a
unit pin proves the seeded row carries
the marker through the panel's own
scan/metadata/line path. IA-T12-1 files
the headless wiring for T16, including
the judgement call on a tree-text grep
step (the geometry scanner reads only
path/rect/key and cannot assert text) —
T16 applied it as proposed. The marker's
pixel proof is not in this slice: the
tree-text proof is the rendered string
itself, which a pixel leg is not. The
scan-fallback line cannot occur on this
scene by construction (the saved file
light-reads cleanly); its headless proof
would need a scene whose file defeats
the light read, which no honest fixture
currently produces.

**Undo-census extension for Wave S's
changes (T13).** The audit — every Wave
S finding grepped, both candidate
handlers cross-checked, S19's
description sweep as the second witness
— finds exactly two MCP-surface
behaviour changes in Wave S, on a
surface that stayed 381 tools all wave:
`subtitle_export_srt`'s reply shape
(S9's `speakers_dropped`, present only
when > 0; the tool was and stays
read-only) and `render`'s scale routing
widened to gif/apng/spritesheet/mp4/webm
(S5; reply shape unchanged). Eight
registry-level legs pin both: the SRT
reply both ways (field absent/present,
exact key sets, written file == the
plain export, 0 steps), a VTT control
leg (the field never appears there; the
voice tag does), gif + spritesheet
scaled renders at 0 steps, the combined
range+scale arm, and both refusals
(scale 0; range past end) at 0 steps
with nothing written. Deliberately not
pinned: S4's core entries (reached only
through `render`), `export_file` (no
scale argument exists), and the
mp4/webm/apng scaled arms — they rest on
S5's in-module pins, stated plainly in
t13.md, because a census leg must not
make the suite depend on ffmpeg's
presence in CI.

**README truth pass (T14).** Four edits,
each verified against the tree: the
checksum file is `SHA256SUMS.txt`, not
`SHA256SUMS` (the workflow writes the
former); the document-model sentence now
includes subtitles (timed cues, optional
per-cue speaker, project-wide style) —
the README had never mentioned subtitles
anywhere; image assets import from PNG
or JPEG by content, not file name (R9,
previously unstated); the export bullet
records the optional output scale as a
true re-render, and a new Formats
sub-bullet covers SRT/WebVTT
import/export with WebVTT voice-tag
speakers and the SRT export's
dropped-credit report. The scale list
deliberately excludes T7/T8's formats —
in flight at T14's read, not in the
settled tree; a future pass can extend
it once they integrate. Everything else
audited TRUE (the Rune pin, CI
behaviour, 27 FX kinds by name, 28
locales, MCP 381 + protocol version, the
headless flags). Two flags are reported,
not edited: no LICENSE/COPYING file
exists in the repo root (the
GPL-3.0-or-later claim rests on
Cargo.toml's `license` field — adding
the text file is a one-line future
slice), and the build-from-source
section's framing speaks to contributors
under this repo's CI-only discipline
rather than to end users (flagged, not
conflated, not rewritten). The verified
scene count (32 at T14's read, before
T12's scene) is recorded in t14.md.

**Gate/census maintenance (T15).** No
script edits — and that is the finding:
on a clean detached-worktree baseline at
HEAD `e0eea5a` and again on the settled
live tree, the gate reads PASS 10/10 and
the census 16/16, so both floors already
equal their verified counts
(`KEY_TOTAL_FLOOR` stays 980 — S15's
ratchet, re-verified here at clean HEAD
itself, the +1 over R15's 979 being
`subtitle-speaker-field` from Wave R's
own integration; `MCP_SURFACE_FLOOR`
stays 381). The forensic accounting is
T15's table: tools 381, palette 134,
icons 163, element keys 864 + 116 = 980
with zero loss per key across the whole
uncommitted stack, scenes 31 → 33 (S2's
speaker scene + T12's, layout facts
1..=35), State 48/22, census scan set
152 → 161 files (the +9 are Wave S's
five and Wave T's four new test files).
The mid-wave check-8 transient
(home-missing-audio registered but in no
tree loop) is diagnosed in t15.md as
neither side wrong — the instrument
caught exactly the blind spot it exists
for, and it resolved when T16's half
landed, the S15 precedent. T15's i18n
count: 202 in tree; the six Wave T IA
blocks are pending integrator-side
arithmetic (158 keys → 360), stated as
such and not made a floor.

**Overflow/headless maintenance (T16).**
IA-T12-1 applied in full: both tree
loops in headless.yml gain
`home-missing-audio`; SCAN_SET gains
`tree-home-missing-audio.json` as
**required** from the start (the Q12/S2
one-step precedent) and stands at 15
entries, 15 required, 0 pending; the
IA's proposed tree-text grep step was
added exactly as written (fail-soft,
verdict in
`headless-out/home-missing-audio-marker.txt`),
because the geometry scanner cannot
assert element text; the tree file joins
the Summary step's headline list; the
scanner's prose recount moves to 36
trees / 33 scenes. T16 also verified T11
needs no workflow change — the speaker
snap pair the lane leg consumes has been
dumped since IA-S2-1. The sequencing
warning stands in t16.md: T12's scene,
these dumps, the required SCAN_SET entry
and the gate's coverage check are one
unit and must land in the same
integration push.

**MCP docs sync (T18).** IA-T7-1 and
IA-T8-1 applied verbatim: the `render`
row's scale sentence now includes `jpeg`
(and only `svg` takes no scale); the
`export_bundle` row is replaced whole
with the source-verbatim form (the
member-content paragraph, the PNG-member
scale sentence with its
refuse-before-write clause, Optional
gains `scale`). Both rows byte-verified
against the sources post-edit. The drift
sweep found nothing else: Wave T has
exactly two MCP-surface writers (T7, T8,
both in mcp_io.rs), every other IA is a
key block or the menus call-site, and
the docs' "Render formats" note makes no
scale claim. Census stays 381; the
gate's check 9 doc-sync passes.

**Cross-feature e2e: scaled animated
exports with subtitles (T19).** Four
legs on a subtitled fixture close the
loop S4/S5/S11 opened: the GIF at 2.0
decodes frame-by-frame — frames outside
the cue's half-open range hold zero face
ink, the in-range face spans the derived
36..=44 rows at the doubled edge offset
against the 1.0 control, frame 2's face
rows equal frame 1's exactly, and a
third uncredited export proves the
prefix specifically (credited column
extent ≥ 2× uncredited); the APNG at 0.5
reads (240, 200) with 4 frames at the
wire level (acTL/fcTL chunk-walked) and
its first frame — the PNG default image
— is byte-equal to the direct 0.5
render, burn-in inside the compared
bytes; the spritesheet at 2.0 has every
tile byte-equal to the direct scaled
render of its frame, burn-in included on
the subtitled tiles; and S4's 1.0
byte-identity trios are repeated on this
fixture, where every frame carries the
credited burn-in. Decode limits are
stated, not hidden: GIF is
palette-quantised (geometry + relations
only, no byte claims), and APNG frames
2–4 carry no pixel assertion (no APNG
frame decoder in the test target — a
dependency question for the integrator,
stated in the suite header).

**Particles panel wired (T20).** The
sweep finds exactly S14's count — 24
label-constructor literals — splitting
into 16 wireable labels (wired, IA-T20-1
files the `particles_*` block) and 8
locale-invariant notation literals
(stepper `−`/`+`, the nudge pad's
arrows) left literal per the R6
precedent. Both view functions already
take `&State`, so no signature changed
anywhere. The kind names
(`burst`/`sparkle`/`smoke`) are excluded
with the brief's own warning honoured:
they are the export/MCP serialised
vocabulary, and translating them would
corrupt that surface. Composed readouts
(row labels, anchor text, the detail
title, the header count meta) stay
English — template-key work beyond a
label pass, the standing S14 remainder
for this panel.

**Wave S's ledger, closed by F2.** The
F2 finisher applied all six items of the
Wave S IA package on the Wave S tree
(base `1d724a2` + the wave's landings),
nothing stopped-on: (i) **IA-S14-1** —
S14's 34 `threed_*` keys as one block ×
28 tables, genuine translations in the
eight translated tables (only legitimate
English-identical values remain there:
`IK` in all eight, DE
`Position`/`Orbit`/`Arm L`/`Arm R`, FR
`Position`, each matching an in-table
precedent), placed immediately after the
`export_subtitle` entry — the end of
S1's export block, per the IA's own
wording (the tables' literal last key is
`subtitle_import_vtt`; the pre-edit
168-key sequence is preserved verbatim
around the insertion, parser-proven) —
the catalogue becoming **202 × 28**;
(ii) **IA-S15-1** — `i18n_wire.rs`'s
header and `src/i18n.rs`'s module-doc
line moved to 202 (S15's 202-variant
text verbatim); (iii) **IA-S8-1** — the
QA_RUNE §17 row replaced with S8's
closure row verbatim ("verified — table
in PHASE_O §10"); (iv) **IA-S9-2** —
S9's panel status pin inserted verbatim
into the subtitle panel's test module
after
`export_writes_the_document_and_import_replaces_once`;
(v) **IA-S20-1** — svg.rs's
`append_subtitles` retires its inline
centre-formula restatement for the
shared helpers, the STOP condition
checked first (no pin asserted the old
structure; output byte-identical by
construction); (vi) **S13's follow-up**
— the one-call close-flush → verdict
composition test landed in
`tests/wave_p_close_guard.rs`
(`close_flush_of_undone_edit_offers_nothing_at_next_launch`:
save + recents in a CWD temp dir, a core
made dirty the P42 way — counted edit,
mutate, undo — asserted serialised-equal
to the save with the counter standing at
1, the REAL `flush_recovery_on_close()`,
and the content gate's verdict
**false**). F2's verification: the
parser proof above, gate PASS 10/10
(element keys 981 by the gate's count at
that later settled state, floor ≥ 980
holding either way), census 381 PASS,
overflow and subtitle-census self-tests
PASS. F2's one remainder: no module-doc
paragraph for the threed block was added
to `src/i18n.rs` (IA-S15-1 specified
only the count line) — a one-minute
symmetry follow-up if the integrator
wants it. With F2, §6.13's ledger items
(4)–(10) are all discharged; items
(1)–(3) were closed inside Wave S
itself.

**Integration ledger — every Integrator
Action filed across Wave T's findings.**
(1) **IA-T1-1** (integrator inserts into
`src/i18n.rs`): T1's 35 export class-C
keys (26 plain + 9 templates) as one
block × 28 tables in roster order,
genuine translations in the eight
translated tables — the catalogue
becomes 237 on this block alone. (2)
**IA-T2-1**: T2's 33 `audio_*` keys (one
template, `audio_clip_count`). (3)
**IA-T2-2** (integrator applies to
`src/rune_sec/menus.rs`): the
`mute_item_parts` call-site adjustment
for `mute_toggle_parts`' new signature —
four exact edits in t2.md; **the tree
does not compile until this lands in the
same pass as T2's wiring**. (4)
**IA-T3-1**: T3's 24 `frame_*` keys (no
templates). (5) **IA-T4-1**: T4's 22
`segment_*` keys (one template,
`segment_title_node` — the `{index}`
placeholder and its braces survive every
table). (6) **IA-T5-1**: T5's 28 `fx_*`
keys (no templates). (7) **IA-T20-1**:
T20's 16 `particles_*` keys (no
templates). Items (1), (2), (4), (5),
(6), (7) total **158 keys**; with all
six blocks the catalogue becomes **360 ×
28** — T15's pending arithmetic,
labelled as such wherever it appears,
until the inserted tables are recounted.
The count docs (`src/i18n.rs` /
`i18n_wire.rs`) move to the settled
total in the same pass — the board's
integration step, not an IA. Until the
blocks land, the six panels' pins cannot
pass — expected, per the Q7 pattern. (8)
**IA-T7-1** (`docs/MCP.md`'s `render`
row: one sentence replaced) — CLOSED
inside the wave by T18. (9) **IA-T8-1**
(`docs/MCP.md`'s `export_bundle` row:
whole-row replacement) — CLOSED inside
the wave by T18. (10) **IA-T12-1** (T16
applies:
`.github/workflows/headless.yml` +
`scripts/overflow_scan.py`) — CLOSED
inside the wave by T16, including the
proposed tree-text grep step applied as
written; SCAN_SET stands at 15 entries,
15 required, 0 pending. No actions were
filed by T6, T9, T10, T11, T13 or T19.

**Verification — awaiting CI.** Counts
are T15's, recounted on the settled live
tree (HEAD `e0eea5a` plus all Wave
S/F2/Wave T landings) against a clean
detached-worktree baseline at HEAD, not
taken from siblings' claims: the gate
reads **PASS — all checks green
(10/10)**; the MCP census reads **381
tools** (44 + 319 + 9 + 4 + 5), 16/16
checks PASS, and both floors stay where
S15 put them — `MCP_SURFACE_FLOOR` 381,
`KEY_TOTAL_FLOOR` 980 (T15 edited
neither script; the floors equal the
verified counts). **Element keys 980**
(864 static + 116 dynamic), zero loss
per key against HEAD across the whole
uncommitted stack (the gate's check 2
over the full stack is the proof).
Palette commands 134; curated icons 163;
headless scenes **33** (layout facts
1..=35 sequential, 35 strings); State 48
fields over 22 full literals. The i18n
catalogue in tree reads **202 keys ×
28** — S1's 16 + F2's 34 over the 152
base, sequences identical ×28 — and
becomes **360 × 28** when the six Wave T
IA blocks land (pending arithmetic, per
T15; not verified from the tree and not
a floor). The census scan set reads 161
files (117 under src/, 44 under tests/;
the +9 over HEAD are Wave S's five and
Wave T's four test files). SCAN_SET
stands at 15 required / 0 pending with
its self-test green (T16), and the
subtitle census self-test reads PASS
(10/10) with T11's lane leg (T11). CI —
the integration push's Build, Test &
Binary plus a headless dispatch — is the
proof of everything above, including
T11's analytic lane floors, the F2
package's pins, and T2's wiring once
IA-T2-2 lands beside it.

**Honest remainders.** (1) The standing
user decisions stand exactly as §6.13
left them — white-on-accent contrast,
the panel splitter, File-menu Save As,
the two "New project" results,
`tab_visits_controls`, project-swap
undo, the skew channel, the whole-figure
pose transform, MCP `write_recovery`,
the waveform call; this wave touched
none of them, per its contract 8. (2)
S1's seven value-vs-label discrepancies
stand — neither side changed — and T1's
class-(c) classes stand with them: the
export panel's status sentences and
recap builders stay English because
their shapes are machine-read by the
toast classifier, and localising that
grammar is a shell-wide decision, not a
panel slice; the `Loop range` toggle is
untouched as one of the seven. (3) **GIF
import stays undecided**: the funnel
still refuses GIF deliberately; widening
the accepted set is a scope decision for
the user. (4) **A panel export-scale
control remains unbuilt by design**: the
core/MCP scale surface is now complete
for every raster render format, and no
control was invented to front it. The
bundle's JPEG member stays 1.0 (T8's
scoping; the docs now promise exactly
what the source does), so whole-bundle
scaling is a follow-up if the user wants
it. (5) No subtitle stack cap exists
(the S20 remainder): uniformly
unpaintable past the pinned ceiling
stays the proven behaviour, and changing
it is a product decision. (6) **Analytic
gates still await their first real
measurements**: S2's stage span gate
(§6.13 remainder (8)) has still never
run — no CI run has produced the speaker
snap — and T11's lane floors join it;
both print their measured values on the
first integration headless run and
ratchet to ~1/3 of measured then. T11
also records a harness observation the
integrator should read before that run:
run 42759195547's census transcript
itself FAILED — its snaps are 2880×1800
(2×) against 1440×900-point tree
geometry, so bands read at point rows
found zero diffs, and a 40 pt stage-rect
drift stands between the seeded and
default trees — while the census step's
fail-soft setting kept the workflow
green. The lane leg follows the script's
single existing contract (tree points
are snap pixels, as every leg assumes);
if the integration run's snaps again
come out 2×, all legs misread together
and the transcript shows it — the fix
belongs to the probe/workflow side, not
to one leg inventing a second coordinate
system. (7) T14's flags stand: no
LICENSE/COPYING file in the repo root (a
one-line future slice), and the README's
build-from-source framing — flagged, not
rewritten. The README's export-scale
list deliberately excludes the JPEG
sequence and the bundle PNG member until
T7/T8 integrate (T14's remainder). (8)
English residue by design: menus.rs's
own menu labels (T2), the audio panel's
`effects_settings_line()` class-C
sentence (T2), the composed readouts of
the segment, FX and particles panels
(T4/T5/T20), the export panel's
`export_centre` key still without a call
site (T1), and the FX display kind names
— fx_browser's surface if ever ordered
(T5). (9) Proof limits stated by their
slices: T19's APNG frames 2–4 are
wire-level only; T12's marker proof is
tree text, not pixels; T13's census legs
witness gif/spritesheet for the widened
render routing, the apng/mp4/webm arms
resting on S5's in-module pins; T10's
pixel assertions inherit S18's
calibrated bands. (10) §6.13 remainders
closed here: S7's scan-fallback gap (T6)
and the Home badge's missing headless
proof (T12, in tree-text form); S3's
lane pixel proof (T11); the scale
matrix's last two gaps — the JPEG
sequence (T7) and the bundle PNG
member's validation (T8); S14's named
next panels, all five wired (T2 audio,
T3 frame_ops, T4 inspector_segment, T5
fx_clips, T20 particles) plus the export
panel's class-C copy (T1); the undo
census extended over Wave S's two MCP
behaviour changes (T13); the README's
staleness (T14). And sound remains
user-verified only.

### 6.15 Wave U — i18n completion 2, the encode-progress contract, proof deepening 4 (2026-11-10)

Ordered 2026-11-10 (11:37 +06) as a
re-issue of the standing continuous
order — the seventh time the user has
issued it: "assign 20 agents continuous
write code if ci gets error then fix
that". Twenty agents, one owner per
file, integrator surfaces via filed
actions (board:
`~/workspace/wave-u/BOARD.md`; findings:
`~/workspace/wave-u/findings/`). The
board's base is the S+T integration HEAD
`d39fc10` (360 × 28, scenes 33, MCP
381); `src/i18n.rs` and
`src/rune_sec/i18n_wire.rs` were frozen
to this wave's agents, all key work
arriving via IA. This section was
written from eighteen sibling findings
(u1–u9, u11–u19; u20 is this section) —
**u10.md was not filed at the final
check and U10 is recorded as
outstanding below, not summarised** —
and from the CI frame read via `gh` at
that same check. Every Wave U status
below is IMPLEMENTED (in tree,
uncommitted at write time); Wave U has
no CI verdict. The S+T frame, by
contrast, is stated once, exactly, here
— including the parts a green workflow
conclusion would hide.

**The S+T integration ledger.** Six
commits on top of `7f9bce5`, in order:
(1) `6f0b737` — i18n: the catalogue
152 → 360 keys × 28 across the two
waves (S1's export gaps, the S14/F2
`threed_*` block, and **F3's IA package:
Wave T's six panel blocks — T1 35,
T2 33, T3 24, T4 22, T5 28, T20 16 =
158 keys — inserted as one package**,
genuine translations in the eight
translated tables, 11 template keys
keeping their slots in every table);
the panels wired in the `tr()` idiom;
**IA-T2-2** (the menus.rs call-site fix
for T2's `mute_toggle_parts` signature —
compile-blocking since Wave T, now
landed); count docs at 360. (2)
`00d1e05` — the scale matrix completed:
scaled GIF/APNG/spritesheet (S4) and
MP4/WebM (S5) entries with 1.0
byte-identical delegation; JPEG-sequence
scale + range sibling (T7), leaving
`svg` the only no-scale format, by
nature; `export_bundle`'s scale
validated before any write (T8); SRT
export counting dropped speaker credits
(S9); the svg subtitle helpers unified
(IA-S20-1). (3) `ecde74f` — Home: the
missing-audio marker on the light-read
meta line (S7) and on the scan-fallback
line from the scan's own verified count
(T6; `None` = unknown, never health);
the headless scenes `subtitles-speaker`
(S2) and `home-missing-audio` (T12);
S13's close-flush × recovery-gate
composition pin. (4) `50144d7` —
instruments: the gate floor ratcheted
to the verified 980 element keys (T15);
SCAN_SET at 15 required / 0 pending with
both new scene trees dumped by both
headless loops and the fail-soft
marker-text grep step (S16/T16,
IA-T12-1); the subtitle census's lane
speaker pixel leg (T11). (5) `ae69e07`
— proof suites: Wave S's raster A-B-A
determinism, cross-feature e2e, JPEG
parity, scaled burn-in and speaker
rendering proofs; Wave T's SVG
determinism combos, the MCP render e2e
through the real registry, the
undo-census extension for Wave S's two
MCP behaviour changes, and the scaled
GIF/APNG/sheet e2e with decoded burn-in.
(6) `d39fc10` — docs: PHASE_O §10's scan
table from run 42759195547's real
artifacts and QA_RUNE §17's closure row
(S8/F2); §6.13 + §6.14; docs/MCP.md's
render and export_bundle rows
source-verbatim (S19/T18); T14's README
truth pass. The integrator's own
instruments on the committed tree: gate
**PASS 10/10** and the subtitle-census
self-test **PASS** (U17's clean-worktree
recount at `d39fc10` independently
agrees: census 381, element keys 980,
i18n 360 × 28 identical sequences,
scenes 33, layout facts 1..=35).

**Round 6 closes the Wave R census
chain.** The six build rounds on the
pre-S tree, each failure test-side,
none in product code: round 1
(42759195059, on `59cb0be`) FAILED on a
bin-test compile error (a `dedup()` on
an array in the subtitle palette pin;
fixed `85898b4`); round 2 (42759845639,
on `85898b4`) FAILED on one lib test
(R20's `1e999` literal, which serde_json
refuses itself; fixed `14bd9b3`); round 3
(42760562852, on `14bd9b3`) FAILED on
one integration test (the round-trip
suite's non-vacuity marker missing its
`audio/` prefix; lib and bin green;
fixed `1d724a2`); round 4 (42761122064,
on `1d724a2`) FAILED on five census
tests (R7's hand-rolled PNG fixture's
bad IDAT CRC, rightly refused by R9's
decode-checked import; fixed `e0eea5a`,
encoder-made fixture); round 5
(42761838001, on `e0eea5a`) FAILED on
four census tests (the audio_split leg's
stale args, an absolute-depth witness
after a step-spending setup, figlib legs
hardcoding 'Styk' where the library's
file-stem name is 'styk', and parallel
races over the CWD-relative stores;
fixed `7f9bce5`, the suite serialised);
**round 6 (42762884625, on `7f9bce5`):
GREEN** — workflow conclusion success,
the chain closed.

**The S+T CI frame — read at the
transcript level, per the 42759195547
lesson.** Build round 1 on the
integrated tree (42764333258, on
`d39fc10`) **FAILED**: exactly one
compile error, E0308 at
`src/rune_sec/home_panel.rs:4440` — a
delete-confirm test seeds the panel's
metadata cache with the pre-S7 4-tuple,
but S7 widened the cache value with the
missing-audio count as its fifth
element; the lib target and every other
suite were unaffected, and no test ran
in that target. The fix is `b2f948f`
(the fixture's project has no audio, so
the fifth element is 0, the true count)
— **committed locally but ahead of
origin by one at this writing: no
round-2 run exists yet, and the S+T
build is not CI-green until one passes.**
Headless (42764348922, on `d39fc10`,
dispatched): workflow conclusion
**success** — and, exactly as in
42759195547, the conclusion alone would
have lied. The subtitle census
transcript (`subtitle-census.exit` = 1)
reads **FAIL — 10 assertions**: the
stage rect disagrees across the seeded
and default tree dumps (drift 40.0 —
heights 464 vs 504); every pixel leg
reads zero (both anchor-flip band
changes, both band text faces, the
credited face, the speaker span at 0
units against the ≥ 396 gate, the
credited-vs-uncredited band difference,
and T11's two lane pixel floors at
diff 0 / ink 0). The cause is the one
T11 recorded and U18 re-measured on this
run's artifacts: the snaps are 2880×1800
(2×) against 1440×900-point tree
geometry, so bands read at point rows
find nothing, compounded by the 40 pt
seeded-vs-default stage drift. What
passed in the same transcript is real:
all four snaps share one size, the
seeded bottom band differs from the
no-cue default (1736 ≥ 200), the
inter-band stage is pixel-identical
across both pairs, and **the lane rect
gate passes** (both dumps' blocks at
(336, 780, 166, 18), drift 0.0). The
marker transcript reads verbatim
**"HOME MISSING-AUDIO MARKER: PASS"** —
T12's scene and T16's grep step proven
end to end on their first real run: the
tree's `home-proj-meta-0` text ends with
"· 1 audio clip missing". The MCP probe
passes; the overflow scan over the run's
real artifacts is **CLEAN (0 hits, scan
set complete)** across all 15 required
trees (U18's run of the scanner over
the downloaded artifacts). Net S+T
frame: headless workflow green,
**census transcript red**, marker green,
build red on one test literal with its
fix unpushed. Nothing beyond that is
called shipped.

**Stroke inspector fully wired (U1).**
The last label-only panel gets the full
pass: 53 `stroke_*` keys — 31 plain
labels, 16 composed readouts as `{slot}`
templates, 6 plain readout fragments
(summary clauses, the shared `Off`
word) — at 51 literal `tr` sites (two
sites select among keys). Six helpers
gained `&EditorCore` (the T2 precedent;
grep-verified no caller outside the
file). One judgement call is recorded,
not hidden: the scatter press's status
sentence embeds the wired formatter's
`Off`, because forking the formatter to
keep one status hermetically English
would diverge the vocabulary; the
two-line hermetic fix is named in u1.md
if wanted. IA-U1-1 files the 53 keys
(413 × 28 from this block alone);
element keys 26 static, zero lost.

**Clips panel fully wired (U2).** 37
`clips_*` keys — 29 plain labels + 8
templates (header count, row detail,
span and usage shapes, all four session
label shapes) — at 41 sites, labels and
readouts in one pass per the brief. The
three readout helpers gained
`&EditorCore` in place: grep proved no
external callers, so unlike T2's mute
helper no cross-file IA is owed. The
panel's two status sentences stay
English (board rule); clip and figure
names flow through slots as data.
IA-U2-1 files the block (397 × 28
alone); element keys 25, set-identical.

**Menus wired (U3).** All 23 menu
labels — 4 bar triggers, 19 dropdown
items — resolve through the catalogue,
after a machine-read check done before
wiring (no code parses any of the 23;
other sections' identical strings are
their own literals, and the help panel
matches its own section data, never a
string the menu passes). The mute pair
is the slice's point: IA-T2-2's
signature work is **not disturbed** —
`mute_item_parts` still threads the core
to `mute_toggle_parts` for the glyph —
and its label now resolves through `tr`
under its own `menu_mute_clip` /
`menu_unmute_clip` keys, T2's exclusion
of the menus' own mute labels lifted by
the board because U3 owns the file.
Excluded: recent-project names (user
data), shortcut hints (chord notation),
and the open-recent not-found status
sentence — pinned verbatim and mirroring
the MCP file tools' identical error, so
translating only the menu's copy would
split the surfaces. IA-U3-1 files the
23 keys (383 × 28 alone).

**FX display names wired (U4).**
`fx_browser::kind_label` is core-aware:
the 27 known kinds resolve through
`fx_kind_*` keys, English byte-identical
to the pre-wiring humaniser. Every call
site in the tree was audited first — no
site parses, persists or serialises a
display name — and the old humaniser
survives as `kind_label_english`: the
filter's canonical English search alias
(the filter now matches the localised
label AND the English one) and the
fallback for names outside the 27-kind
vocabulary. The serialised vocabulary is
untouched (`fx_edit.rs` diff clean; the
27 `kind_name` values set-equal to the
table's). **IA-U4-2 is the wave's one
compile-blocking action**: kind_label's
new arity leaves exactly two old-arity
call sites, both in U8's fx_clips.rs,
with exact replacement text in u4.md —
the tree does not compile against the
new signature until the integrator (or
U8's landed file) carries both forms.
IA-U4-1 files the 27 keys.

**Audio readouts templated (U5).** T2's
filed remainder closes: 21 `audio_*`
keys convert every composed readout —
gain suffix/value, target lines,
readout base + duration + muted
segments, the row-label skeleton, the
frame-count pair, the summary pair, and
the health vocabulary (three plain words
+ two `{word}` wrappers via a new
resolver). One signature was deliberately
NOT changed: `health_word` has a second
consumer, `waveform_view::clip_health_
caption`, with no core in scope, so the
panel resolves through the catalogue and
a pin asserts catalogue English ==
`health_word` for all three states —
the two vocabularies cannot drift
silently, and the waveform strip's
caption stays English until a
waveform_view slice wires it, stated
plainly. `effects_settings_line()` stays
excluded as class-C copy; the English
summary's non-singularised "1 clips" is
preserved byte-identically (a locale's
template may pluralise its own grammar).
IA-U5-1 files the 21 keys.

**Frame-ops readouts templated (U6).**
T3's one filed remainder — the dynamic
readouts — closes with 6 template keys:
`Sel`/`Loop` in single and range forms
(four keys, shapes differ), the duration
value (`{value}×`, the two-decimal
number still formatted by the one
helper), and the Insert count composed
inline in the audio header-count idiom.
`format_duration` gained `&EditorCore`
(the T1 helper precedent; no caller
outside the file). Status sentences and
notation (the `—` empty marker, `×0.5` /
`×2`, stepper glyphs) stay as classified.
IA-U6-1 files the 6 keys (366 × 28
alone); element keys 41 occurrences /
25 distinct, multiset identical.

**Particles + segment readouts
templated (U7).** T20's and T4's
remainders close together: 21 keys —
10 `particles_*` (row label, anchor
summary/detail in their free/figure/
node shapes, detail title and fused
summary, pad position, the
`{count} event(s)` header meta) and 11
`segment_*` (the topology line composed
phrase-level from a line template plus
parent/root/child part keys, the bend
readout, and `media_label`'s five slot
states — including a stale-index
"Sprite: asset {n} (missing)" state the
code has and T4's findings had not
itemised). `shape_label`'s values stay
English per the board's standing
exclusion (model vocabulary); the
serialised particle kind names reach
the detail title only through the
`{kind}` slot, as data. IA-U7-1 files
both sub-blocks (381 × 28 alone).

**FX-clips readouts templated (U8).**
T5's exclusions close: 9 `fx_*` template
keys — window, readout + hidden suffix,
the lane count in two full templates
(word order and agreement inside one
string per form, not stem + suffix),
the row anchor, the inspector title /
summary / seed lines. Kind strings enter
only as `{kind}` slot values (the
serialised name in the title, U4's
display name in the readout); angle and
length slots take pre-formatted `{:.0}`
values, the rounding staying in code.
The pre-existing readout content tests
now double as the templates' composition
pins (the T1 precedent). IA-U8-1 files
the 9 keys (369 × 28 alone); U8's gate
run on the mid-wave tree read PASS
10/10, its file's key check included.

**3D readouts templated (U9).** S14's
remainder was exactly three readout
functions, and all three are wired with
6 `threed_*` keys: the no-session line,
the session summary template with the
pose-B state word keyed separately
(`marked` / `not marked`, translated as
a unit and substituted into the `{state}`
slot rather than baked into two
near-duplicate templates), the joint
position line, and the camera readout.
No signature changed — all three already
took `&EditorCore`; S14's 34 label keys
and 49 sites are byte-untouched (49 →
55 `threed_*` sites in total). The
52-character English camera-line pin
still pins the English extreme, which is
the locale the headless probes render.
IA-U9-1 files the 6 keys (366 × 28
alone).

**Catalogue hygiene audit (U10) —
OUTSTANDING.** No findings were filed
by the final check (U17's check,
earlier in the wave, records the same
absence), so the dead-key ledger — the
keys with zero `tr()` call sites,
`export_centre` the known example,
accounting for dynamically constructed
keys before calling any key dead — and
its IA-U10-1 wire-or-remove text are
not in hand. Nothing is recorded here
in its place: the audit, its per-key
recommendations, and any catalogue
removals it would cause remain owed,
and the integration arithmetic below
excludes it.

**Encode-phase progress: the contract
boarded, the core hook landed (U11).**
The audit's headline is plain: **the
Rune line at this base has no
export-progress plumbing at all** — no
callback, channel, event or shared state
anywhere in the crate — so there was
nothing to extend. What landed, in
`src/export.rs` only, is the minimal
phase-aware core hook: a public
`EncodeProgressEvent { encoded_frames,
total_frames, finished }` + callback
type beside the entries; eight new
`*_with_encode_progress` entries beside
the eight untouched existing ones (which
pass `None` and run the historical
ffmpeg invocation byte-for-byte); and a
whole-life drainer thread inside the one
process runner both encode funnels
share, reading ffmpeg's `-progress
pipe:1` stream. The contract, recorded
on the type: the count is the encoder's
own output-frame count (only `frame=`
lines are parsed — never `fps`,
`out_time_*` or stderr), so render and
encode phases cannot be conflated; the
total is the sampled frame count the
entries already return; intermediate
events are unfinished, strictly
increasing, clamped, with counts
unpinned (wall-clock business); exactly
one terminal event fires, after ffmpeg
exits successfully AND the stream drains
to EOF; a panicking observer loses one
event and can neither fail the export
nor kill the drainer (the pipe-fill
deadlock the whole-life read exists to
prevent). **`finished` closes the video
encode only** — an MP4's separate audio
mux runs afterwards and can still fail
the export, and no surface may present
`finished` as success. Five pins cover
the parser, the drain discipline, both
funnels' real encodes, and the
no-terminal-event failure path.
**IA-U11-1 wires nothing in this wave,
by specification**: the MCP handler
type is a bare fn pointer with no
progress token and no notification sink
on either transport, so a reply field
written after the encode would be fake
progress — the future transport slice's
mapping onto MCP `notifications/
progress` is fixed by the contract and
recorded in u11.md; the panel's wiring
is likewise specified only as a
background job over a cloned project
(a synchronous callback updates state
the blocked UI thread cannot paint until
the call returns). No tool description
changed; census stays 381.

**Scan-fallback headless proof (U12).**
Constructible after all — the
divergence T12's remainder named is a
content-level disagreement between
Home's two readers, and a real saved
file produces it once hand-trimmed:
the scan full-parses through `Project`'s
serde defaults (every field defaulted,
expressly so hand-trimmed files still
load), while the light read hard-requires
the top-level `figures`/`frames` keys
present and arrays. A file with its
`frames` key removed therefore yields a
scan summary (and T6's `Some(count)`)
while its light read returns `None` on
every read — the fallback arm taken
deterministically, no cache primed, no
panel state seeded. The scene
`home-scan-fallback` (34th, appended
last) seeds exactly as its sibling does
(real save op + recents follow-up, one
planted missing-file clip) and then
trims the saved file's `frames` key in
place; fact 36 is its text fact (the
fallback line's discriminator against
fact 35: no size segment, a defaulted
frame count of 0), and a unit pin proves
the row through both readers, the
composed line byte-equal to `meta_line`.
**IA-U12-1 was filed after U18's final
check**, so the pairing is the
integrator's to land: both tree loops +
`tree-home-scan-fallback.json` as
SCAN_SET's 16th required entry + the
marker grep step (which also asserts
the "· 0 frames ·" discriminator) in
ONE push with the scene already in tree
— U12's own gate run reads 9/10 with
exactly check 8 failing on the
registered-but-unlooped scene, the
expected transient (the S2/S16, T12/T16
precedent).

**Undo-census extension for Wave T
(U13).** The audit finds exactly two
MCP-surface behaviour changes in Wave T
(T13's discipline applied to its own
wave): `render`'s jpeg scale routing
(T7) and `export_bundle`'s
refuse-before-any-write validation (T8).
Seven registry-level legs pin both:
the full and ranged scaled JPEG renders
at 0 steps with exact reply key sets
and decoded dims; the scale refusals
(0, −2.5, the 1e300 f32-overflow) at
0 steps with the target directory never
created and `check_export_scale`'s
sentence asserted verbatim in the tool's
voice; the valid 2.0 bundle scaling
only its PNG member (dims doubled,
jpeg/svg/spritesheet members
byte-identical); absent == explicit 1.0
whole-tree; invalid scales refusing
both bundle kinds before any write; and
the string scale refusing in the
helpers' voice — the case that silently
rendered at 1.0 before Wave T. No
success-path video-bundle leg, stated
plainly: it would make the suite depend
on ffmpeg in CI; the video kind is
legged on the refusal path, which is
pure validation.

**Raster determinism on the full
project (U14).** Three A-B-A legs put
the whole population through the raster
funnel at once on the intact
`new_demo` project (five authored
frames, 960×720, frame 2): figures,
weather (rain 40, wind 30), a credited
two-line cue, a media sprite — at 1.0,
at 2.0 through the scaled funnel (B =
the speaker renamed, exercising the
composed prefix, not just geometry), and
the two public entries pinned against
each other at 1.0. Stated plainly in
u14.md: the demo ships NO bend data, so
the fixture authors it onto frame 2
(0.35 rad on every non-root node) — the
suite proves the bend machinery on
authored data, nothing about the demo
itself; four removal witnesses (bends,
weather, sprites, speaker) prove each
class actually reaches the rasterizer
before identity is demanded of it. The
source audit found no nondeterminism
evidence (no map iteration, no RNG but
the seeded `prand` hash, no wall clock,
no decode cache); FX clips and graph
channels are absent from the fixture and
outside the suite's claim.

**Cross-feature e2e, four legs (U15).**
(1) Scan-vs-light-read missing-audio
parity over five on-disk fixtures —
mixed (present + missing + muted-missing:
the muted clip is the drift witness),
healthy, silent, keyless (the `audio`
key removed by JSON surgery), and a
broken file both readers refuse — counts
agreeing everywhere, failure verdicts
included. The reachability limit is
stated openly in the suite header:
`read_metadata_counts` lives in the
binary's home_panel (`rune_sec` is not
in the library), so the leg performs its
exact computation from library types;
the bin-side wiring is pinned by
home_panel's in-crate tests. (2) The
scaled JPEG sequence carries the speaker
burn-in, decoded and measured (S11's
geometry; the credited extent ≥ 2× the
uncredited control). (3) Bundle refusal
atomicity through the real registry —
four invalid scales, directory never
created, undo depth 0, scratch dir empty
after all four. (4) JPEG-sequence 1.0
byte identity on the subtitled fixture,
non-vacuous (frame 1 decodes with face
ink present). No IAs; the scan cache's
freshness contract is not exercised, by
design (leg 1 scans each state once —
the parity question).

**README truth pass 2 (U16).** One
edit: the export bullet now claims what
T14 deliberately left unclaimed as
in-flight — JPEG sequence joins the
scale list, and the bundle sentence
lands in the docs row's exact scope
(PNG sequence member only, other members
unaffected, invalid scale refuses before
anything is written). The settled counts
were re-verified against the tree
rather than inherited: census 381;
i18n 360 × 28 with identical sequences
(a naive first parse reported 361/29 —
a test-fixture `static PARTIAL` artifact;
the delimited parse is the count of
record); scenes 33; 27 FX kinds by name;
5 weather kinds. T14's four flags are
carried forward unchanged (below), none
silently resolved.

**Gate/census maintenance (U17).** No
script edits — the finding, again, is
that none were owed: on a clean detached
worktree at `d39fc10` the gate reads
PASS 10/10 and the census 16/16, and
both floors already equal their verified
counts (`KEY_TOTAL_FLOOR` 980 =
864 static + 116 dynamic with zero loss
vs HEAD; `MCP_SURFACE_FLOOR` 381), so
there was no ratchet to make and none
was manufactured. The mid-wave live
recount also read 10/10 with no
transient — Wave U wiring touches no
gate-counted surface until the IA
blocks enter the frozen catalogue.
U17's pending arithmetic was partial by
timing (only U6's and U9's blocks filed
at its final run); the full pending
total is reconciled in the ledger below.
Recorded in u17.md: HEAD advanced to
`b2f948f` during its slice (the
integrator's S+T round-2 fix) without
changing any counted surface.

**Overflow/headless maintenance (U18).**
No edits to either owned file:
IA-U12-1 had not been filed at U18's
final check (it landed after), and the
brief's rule is to change nothing
without it — SCAN_SET stands at 15
required / 0 pending with its self-test
green and the workflow valid YAML.
U18's second job is the one this
section's S+T frame rests on: it
polled the dispatched headless run to
completion, downloaded the artifacts,
and read the transcripts directly —
census FAIL (10 assertions, quoted
verbatim in u18.md), the marker PASS,
and its own run of the overflow scanner
over the run's real trees: CLEAN,
0 hits, scan set complete. Its remainder
is filed, not fixed: the census step's
coordinate/scale contract (2880×1800
snaps vs point-space tree rects, plus
the 40 pt seeded/default stage drift)
belongs to `subtitle_census.py`/scenes
territory, which no Wave U slice owns —
until a rostered owner fixes it, every
pixel leg reads zero and the step keeps
failing soft behind a green workflow.

**MCP docs sync (U19).** No edit to
docs/MCP.md, and that is the verified
result: IA-U11-1 had not been filed at
U19's final check (it landed after,
specifying no description change), and
the in-tree evidence agreed — zero diff
across `src/mcp*.rs` / `src/shell_tools*.rs`
in the Wave U tree. The drift sweep
lifts all 381 source descriptions
against their catalogue rows: Wave T's
two changes (the only MCP description
changes across Waves T and U) are
script-verified character-identical to
their rows, including T8's
refuse-before-write sentence in the
`export_bundle` row. The sweep also
counts what is NOT drift: 86 of 381
rows stand in pre-existing condensed
form (the `shell_*` family the bulk),
predating both waves, with no IA
prescribing replacement — left
untouched, named here as a possible
future full-verbatim docs slice. Census
stays 381; check 9 doc-sync passes.

**Integration ledger — every Integrator
Action filed across Wave U's findings.**
(1) **IA-U1-1**: 53 `stroke_*` keys
(16 templates). (2) **IA-U2-1**: 37
`clips_*` keys (8 templates). (3)
**IA-U3-1**: 23 `menu_*` keys (no
templates). (4) **IA-U4-1**: 27
`fx_kind_*` keys (no templates); (5)
**IA-U4-2**: the two exact `kind_label`
call-site replacements in fx_clips.rs
(U8's file — discharged by the
integrator if U8's landed file does not
already carry the new-arity forms; the
tree does not compile until one side
lands them). (6) **IA-U5-1**: 21
`audio_*` readout keys (12 templates).
(7) **IA-U6-1**: 6 `frame_*` template
keys. (8) **IA-U7-1**: 21 keys
(10 `particles_*` + 11 `segment_*`,
templates flagged per key in u7.md).
(9) **IA-U8-1**: 9 `fx_*` template keys,
inserted after the IA-T5-1 block. (10)
**IA-U9-1**: 6 `threed_*` keys
(3 templates + the two state words +
the no-session line). Items (1)–(4) and
(6)–(10) total **203 keys**; with all
nine blocks the catalogue becomes
**563 × 28** — pending arithmetic,
labelled as such wherever it appears,
until the inserted tables are recounted
(U10's unknown removals, if it ever
files, would subtract). The count docs
move to the settled total in the same
pass, per the standing integration step.
Until the blocks land, every wired
panel's pin is red by the Q7 pattern —
expected. (11) **IA-U11-1**: no
insertion — the encode-progress wiring
specification (MCP: do not wire, no
description change, U19 owes nothing;
panel: background-job shape only),
recorded above. (12) **IA-U12-1** (the
integrator applies, U18's half having
closed before it filed): both headless
tree loops + the SCAN_SET entry +
the marker grep step + the prose
recount, in ONE push with U12's scene.
No actions were filed by U13, U14, U15,
U16, U17, U18 or U19.

**Verification — awaiting CI.** Wave
U's counts are U17's, verified on a
clean worktree at `d39fc10` and again
on the live tree, not taken from
siblings' claims: gate PASS 10/10,
census 381 (44 + 319 + 9 + 4 + 5),
element keys 980, palette 134, icons
163, i18n **360 × 28** in tree becoming
**563 × 28** when the nine IA blocks
land (pending arithmetic; not verified
from the tree and not a floor). Scenes
stand at **34** in tree with U12's
`home-scan-fallback` — and the gate's
check 8 reads it as registered but in
no tree loop until IA-U12-1 lands, the
one known red in the unintegrated tree,
by design. SCAN_SET stands at 15
required / 0 pending (16 once
IA-U12-1 lands). CI — the Wave U
integration push's Build, Test & Binary
plus a headless dispatch, transcripts
read — is the proof of everything
above, including U11's encode pins
(which need ffmpeg in CI, as their
sibling suites already assume) and the
first execution of every extended pin.

**Honest remainders.** (1) The standing
user decisions stand exactly as §6.14
left them — white-on-accent contrast
(3.14:1), the panel splitter, File-menu
Save As, the two disagreeing "New
project" results, `tab_visits_controls`
OFF, project-swap undo, the skew
channel, the whole-figure pose
transform, MCP `write_recovery`, the
waveform badge/call, S1's seven i18n
value-vs-label discrepancies (the
export "Loop range" button and the
PNG/GIF/MP4/.stk format labels among
them — neither side changed), the
subtitle-stack cap/wrap — and this wave
touched none of them. (2) **The S+T
build is not yet CI-green**: round 1
failed on one stale test literal and
its fix `b2f948f` stands committed but
unpushed at this writing (ahead of
origin by one); round 2 is owed before
anything on that tree is called
verified. (3) **The subtitle census's
coordinate contract is unfixed and now
twice-proven**: snaps at 2× against
point-space tree rects, plus the 40 pt
seeded/default stage drift, fail 10
assertions on the settled S+T tree
exactly as they silently did on
42759195547; S2's stage span gate and
T11's lane pixel floors therefore
remain **unratified** — their first real
measurements are zeros produced by the
misalignment, not by the app — and the
fix needs a rostered owner for
`scripts/subtitle_census.py` (and
possibly the scenes/probe side) before
the floors can be ratified or honestly
re-derived. The workflow stays green
throughout because the step is
fail-soft; read the transcript, never
the conclusion. (4) IA-U12-1's pairing
must land in one push with U12's scene
or headless and the gate go red by
design (remainder of record until the
integration push). (5) IA-U4-2's two
call sites: the tree does not compile
against the new `kind_label` until they
land. (6) U10's hygiene audit is
outstanding; `export_centre` still has
no call site, and any other dead keys
remain unledgered until it files. (7)
U11's follow-ups are specifications,
not work: the panel background-job
wiring and the MCP `notifications/
progress` transport slice are unboarded;
render-phase and audio-mux progress are
out of the contract's scope by design;
hooked encodes fail on a pre-1.0 ffmpeg
(the probe-vs-retry decision was
explicitly not taken). (8) English
residue by design: the export class-C
status/recap sentences (machine-read,
standing exclusion), status sentences
in every wired panel, the waveform
strip's health caption (U5), the help
panel's own section names and shortcut
table (U3), the open-recent not-found
sentence (U3), and the serialised
vocabularies everywhere. (9) GIF import
stays undecided (the funnel refuses it
deliberately); a panel export-scale
control remains unbuilt by design; the
bundle's JPEG member stays 1.0, so
whole-bundle scaling is a follow-up if
the user wants it. (10) T14's flags,
carried by U16 unchanged: no
LICENSE/COPYING file in the repo root
(U16 re-checked — both absent), the
README's build-from-source framing, and
the deep feature prose never
line-audited. (11) docs/MCP.md's 86
condensed rows stand as the catalogue's
standing form — a full-verbatim docs
slice is a possible future wave, not
drift. (12) §6.14 remainders closed
here: S14's last two audited panels
wired in full (U1 stroke_inspector,
U2 clips_panel); the composed readouts
§6.14 ¶(8) left standing — segment,
FX and particles (U7, U8), audio (U5),
frame_ops (U6), the 3D panel (U9);
menus.rs's own menu labels (U3); the
FX display kind names, fx_browser's
surface since T5 (U4); the README's
export-scale list, which ¶(7) left
excluding the JPEG sequence and the
bundle PNG member (U16); the
scan-fallback line's missing headless
proof, T12's named remainder (U12's
scene, in tree-text form, pending its
IA pairing); and the encode-phase
progress contract the Tauri line's
Wave N left boarded (U11, core side).
§6.14 ¶(8)'s other items stand: the
audio `effects_settings_line()` (U5's
exclusion) and `export_centre` (U10's
outstanding audit).
And sound remains user-verified only.

**Integrator's correction (Wave U
integration).** Two events postdate
the record above, and this note —
not a rewrite of it — supersedes the
points it names. (a) U10 DID complete
after U20's final check: its audit
accounts for all 360 keys as 282
live-direct, 7 live-dynamic, 12 WIRE,
7 RESERVED and 52 REMOVE. Its
IA-U10-1 is deferred to Wave V's V11
per the Wave V board, so none of its
removals or wirings are in this push
and the catalogue arithmetic stands
at 563 × 28 (360 + the 203 Wave U
keys of IA-U1-1 … IA-U9-1). (b) The
subtitle-census capture-scale fix and
the home_panel tuple fix were
committed and pushed as `b2f948f` +
`9d7dd0a`; S+T round 2 is build
42765201236 + headless 42765217055,
and the census was proven PASS
locally on run 42764348922's real
artifacts before that push. (c)
Accordingly, §6.15's "round 2 owed"
and "census unfixed" remainders are
superseded: the round-2 runs exist
and the census coordinate contract
is fixed; their verdicts belong to
the CI record, not to this section's
remainders.

### 6.16 Wave V — name fidelity, shell
i18n, subtitle style controls, proof
deepening 5 (2026-11-10)

Ordered 2026-11-10 (the standing
continuous order, re-issued for the
eighth time at 11:49 +06 and boarded
ahead so this wave launched the moment
Wave U closed): "assign 20 agents
continuous write code if ci gets error
then fix that". Twenty agents, one
owner per file, integrator surfaces via
filed actions (board:
`~/workspace/wave-v/BOARD.md`; findings:
`~/workspace/wave-v/findings/`). The
board's base is the Wave U integration
HEAD `1c7a70d` (563 × 28, scenes 34,
MCP 381); `src/i18n.rs` and
`src/rune_sec/i18n_wire.rs` were frozen
to this wave's agents, all key work
arriving via IA. This section was
written from all nineteen sibling
findings (v1–v16, v18–v20; v17 is this
section) and from the CI frame read
via `gh` at the final check, headless
transcripts read from the runs'
downloaded artifacts — never from
workflow conclusions alone, per the
42759195547 lesson. Every Wave V status
below is IMPLEMENTED (in tree,
uncommitted at write time); Wave V has
no CI verdict. The Wave U frame, by
contrast, is closed here, exactly,
including the round a green conclusion
would hide.

**The Wave U integration ledger.** Five
commits on top of `9d7dd0a`, in order:
(1) `9973ee6` — i18n: the catalogue
360 → **563 keys × 28** via **F4's IA
package: the nine Wave U blocks —
U1 53 `stroke_*`, U2 37 `clips_*`,
U3 23 `menu_*`, U4 27 `fx_kind_*`,
U5 21 `audio_*`, U6 6 `frame_*`,
U7 21 `particles_*`/`segment_*`,
U8 9 `fx_*`, U9 6 `threed_*` = 203 keys
— inserted as one run at the tail of
every table**, genuine translations in
the eight translated tables, every
template's slots kept in all 28; F4
verified each block mechanically
against its panel pin before inserting
(all nine match exactly) and applied
**IA-U4-2** (the two `kind_label`
call-site replacements in fx_clips.rs)
in the same slice; the panels wired in
the `tr()` idiom; count docs at 563.
(2) `c0fe4e5` — U11's encode-phase
export-progress core: the encoder's
own frame count from ffmpeg's
`-progress` stream, one terminal event,
the eight `*_with_encode_progress`
entries, the historical entries
delegating with `None`; MCP
deliberately not wired (IA-U11-1).
(3) `3480d1d` — U12's
`home-scan-fallback` scene (34th scene,
layout fact 36) with **IA-U12-1 applied
in full in the same push**: both
headless tree loops, SCAN_SET at 16
required / 0 pending, and the fail-soft
scan-fallback marker step. (4)
`d1ea862` — the three proof suites:
U13's registry undo-census extension,
U14's populated-project determinism
A-B-A at scales 1.0 and 2.0, U15's
cross-feature e2e (with its leg-1
caveat standing: the light read's
algorithm re-performed from library
types, the bin wiring pinned in-crate).
(5) `1c7a70d` — docs: §6.15 itself,
F4's integrator correction appended to
it, and U16's README truth pass 2. The
integrator's own instruments on the
committed tree before the push: gate
**PASS 10/10**, MCP census **381**,
overflow and subtitle-census
self-tests **PASS** (F4's verification,
reproduced independently by V13's
clean-worktree baseline at `1c7a70d`
and V20's rehearsal: element keys 980
with both floors already equal to their
verified counts, palette 134, icons
163, scenes 34, layout facts 1..=36).

**The Wave U CI frame — two rounds,
read at the transcript level.** Round 1
(on `1c7a70d`): build 42766879504
**FAILED** and headless 42766902475
**FAILED**, on a single shared cause —
**error E0308 at
`src/rune_sec/fx_clips.rs:452`**, the
sole `error[` in the build log.
IA-U4-2's first replacement text passed
`kind_label(core, …)` — which returns
`String` — where `str::replace` wants
`&str`; the neighbouring replaces in
the same chain borrow correctly, and
the second IA site (`text(kind_label
(…))`, which takes `String`) compiled.
The wave's python-only instruments
cannot see a Rust type error, so it
reached CI — the Q7-pattern pins were
never the issue; this was the applied
IA text itself, one character short.
In the build workflow nothing ran past
the release Build step (no test
executed). In the headless workflow the
probe binary never built, so no snaps
or tree dumps exist for the round: the
subtitle census transcript reads FAIL
(1 assertion — snap inputs present),
the MCP probe FAIL (1 assertion — the
/health answer, the app absent), the
Home thumbnail probe UNPROVEN (inputs
missing), the rect probe unanswered,
and neither marker file was produced.
Nothing in round 1 is evidence about
the app; it is evidence about one
borrow. The fix is `fbc3644` — one
character, `&kind_label(...)` — pushed
by the integrator. Round 2 (on
`fbc3644`): build 42767519353 **GREEN**
— 3,981 tests passed across targets,
zero failures, zero errors in the log;
headless 42767540386 **GREEN**, and its
transcripts, read from the downloaded
artifact, are the round's real verdict:
**SUBTITLE CENSUS: PASS** (exit 0;
19 PASS / 0 FAIL; capture scale 2.000
derived from the dump's own bounds;
speaker face 3086 px, extent 702 stage
units against the ≥ 396 gate; lane
diff 3612 / ink 1163 against floors
200 / 100) — S2's span gate and T11's
lane floors, unratified since Wave S
because their first measurements were
the misalignment's zeros (§6.15 ¶(3)),
are **ratified now** on the settled
tree, at the same numbers the census
fix's local proof produced on round 1
of S+T's artifacts. **HOME
MISSING-AUDIO MARKER: PASS** and **HOME
SCAN-FALLBACK MARKER: PASS** — the new
step green on its first real run:
U12's trimmed-file construction drives
Home's fallback arm in the real harness
exactly as its unit pin promised. **MCP
PROBE: PASS** (exit 0). The overflow
scan over the run's artifacts, run
locally on the download: **CLEAN
(0 hits, scan set complete)** across
the 16 required trees (37 dumps in the
artifact). One transcript does not
close: the legacy Home thumbnail
probe's control leg trips on this run's
populated snap — "control home-totals:
stage-bg share 0.672 over 2464px
(ceiling 0.06); control trips: the
strip colour premise does not hold for
this snap (theme/fixture drift?)" — and
issues no verdict; it is remainder (3)
below, and V7's reworked pair gate is
the rostered follow-up. Net Wave U
frame: round 1 red on one character in
an applied IA, round 2 **fully green
with every transcript read** — build,
census, both markers, MCP probe,
overflow. Wave U is verified.

**Figure-library name fidelity (V1).**
The defect CI round 5 of the Wave R
line proved is fixed at its root: the
2D figure library was the only one of
the app's four libraries that destroyed
the name at save (a bare `FigureDef`
has no name field; entries were named
by file stem). The save now writes the
house envelope the other three already
use — the def's JSON plus a stored
`"name"`, the file still parsing as a
bare def everywhere — and load prefers
the stored name, falling back to the
stem. The consumer audit found the rot
went further: rename and delete
resolved the backing file from the
entry's name, which breaks the moment
names are exact, so both now resolve
through a new `library_entry_file`
(verbatim file first, then the slug),
rename's same-name settle compares
stems, and a successful rename syncs
the stored name before the fs rename so
a reload can never resurrect the old
one; a display-collision refusal joins
the existing stem refusals. Migration
is stated plainly: nothing is renamed
or rewritten at load; legacy entries
keep their stem names until re-saved;
an older build reading a newer file
degrades to stem naming, no corruption.
In-module pins cover the round trip
(save "Styk" → listed "Styk" → insert
yields "Styk"); **IA-V1-1** files the
exact text restoring "Styk" in the
Wave R undo-census legs the defect once
forced to "styk".

**Subtitle readouts templated (V2).**
The subtitle panel's composed readouts
join the template programme: five
`{slot}` keys via **IA-V2-1** — the cue
row label (cue text substituted last,
so a cue reading `{start}` is never
re-scanned), the range line (skeleton
template composing the two word keys,
the U7 topology precedent), the detail
title, the scale stepper's value (the
`frame_duration_value` shape), and the
style colour line (brace-delimited slot
names so `{text}` can never match
inside `{text_hex}`). The extended pin
passes only after the IA lands (the Q7
pattern). The shared-file boundary with
V6 was held textually: V2's hunks are
the three helpers, the cue-row call
site, the detail title and exactly two
readout sites inside `style_block`; no
control, handler or swatch was touched.

**Home panel wired (V3).** The last
unwired panel gets the full audit
first: 19 label strings, 7 composed
readouts, 16 status sentences (left
English, board rule), notation and the
machine-read `format_modified`
classified with evidence. Landed: 31
keys via **IA-V3-1** — a `count_label_tr`
one/many helper in the audio panel's
shape serving both metadata skeletons,
the totals footer, the search count and
the missing-audio segment; the light
and scan meta-line templates (the scan
form honestly carries no `{size}` —
the scan never stats for a length);
the delete question and reopen label as
templates with the name substituted
last. **`missing_audio_segment` was
not forked** — it remains the single
shared composition, now resolving
through the catalogue internally, its
English byte-identical — so the
headless marker contracts, which read
the composed English line and passed
in Wave U round 2 above, are
unaffected by the wiring. Two existing-key
wirings needed no IA (the armed
"Discard changes?" forms and the
rename-line confirm, through
`discard_changes` / `rename`). Two
actions ride along: **IA-V3-2 is
compile-blocking** — three old-arity
call sites in scenes.rs's test module
(U12's pins) need `&s.core` under the
new signatures, exact text filed; and
the buttons-row Rename literal was
deliberately left for IA-U10-1's site
list (see V11), so the two slices
cannot double-wire it.

**Shell chrome audited (V4).** The
honest result is how small the surface
is: a scripted census of `src/main.rs`
(the board's `src/rune_sec/main.rs` is
the bin root `src/main.rs` — the path
note is in v4.md) finds 24 production
literals of which exactly **3 render**
— the waveform card's title and the two
splitter help labels — now wired
through three keys via **IA-V4-1**,
with a camera-shape pin. `src/shell.rs`
contains zero rendered strings (store
file names, JSON field names, serde
default fns — data, all classified).
The roster's four named areas were
located rather than assumed: workspace
selector, status strip and transport
chrome live in panel modules owned by
their own slices, not in chrome; the
app name and the persisted default
workspace name are filed as identity /
persistence data with evidence, not
wired. Element keys, keyboard-match
literals and format strings untouched.

**Help overlay wired (V5).** The help
surface is one file, `help_panel.rs`
(6 sections, 61 rows), located and
claimed before any edit. The wiring is
at the render layer, on the U5
precedent: the canonical data functions
keep returning English pairs
byte-identical — the command palette's
drift guard and the 61-row census pin
them — and `help_text_key` resolves a
canonical string to its catalogue key
at render. **IA-V5-1 files 109 keys**:
the header meta template, 6 section
titles, 6 descriptions, the Drawing
note, and the row-action families (61
rows, 60 distinct strings; 35 of 36
Features labels). Key-cap labels stay
canonical notation; section branching
and element keys stay keyed on the
English names. Two sites were
identified and deliberately NOT wired
here because they are U10's WIRE sites
for existing keys — the panel title
"Help" (`help`) and the Features label
"Live MCP control" (`live_mcp_control`)
— both carried into IA-V11-1's list
below, the second with V11's
recommendation against forcing it.

**Subtitle style controls (V6).** The
audit corrects the board's
parenthetical: `SubtitleStyle` has
exactly four fields and **no background
or opacity field exists** to build a
control for — `box_color` is the
backing colour, drawn as outline and
stamps, and no control was faked for a
field that is not there. Position and
scale controls were already complete.
The real gap was colour reach: both
colours were settable only to the
palette's 8 swatches from the panel,
while MCP accepts any `0xRRGGBB` — a
colour set over MCP showed in the
panel's own readout with no swatch
marked and no way to adjust it. Built,
in the shell's own channel-stepper
idiom: RGB channel rows under each
swatch row stepping one channel by 8
through the real `set_subtitle_style`
op — exactly one undo step per
effective press, nothing spent at a
clamp bound or on a bad index, no
panel-side state added — with pins for
the pure stepper, the off-palette
round trip (`0x123456` walking onto a
palette value), persistence through a
project JSON round trip, and
construction on both themes. Three
keys via **IA-V6-1** (`subtitle_red` /
`_green` / `_blue`); V20 attributes the
tree's +3 dynamic element-key patterns
to this slice's steppers.

**Home snap proof (V7).** The
UI-mission remainder — Home thumbnails
unit-proven but never snap-proven —
gets its capture and its gate: the
`home-missing-audio` scene's snap is
now taken in the lavapipe step (the
populated snap existed since Wave 12),
and the thumbnail probe gains a pair
gate (`--also-snap` / `--also-tree`,
wired into the existing probe step)
that treats the seeded constructions as
the fidelity experiment they are: among
the four strips, exactly two must be
pixel-identical and the others differ
(the populated set saves one project
twice and one once; the missing-audio
project differs only by a non-rendering
clip), decided on the multiset of
strips because scan order is
mtime-sorted and row order is never
assumed. Non-blank floors are Q16's
own, the legacy invocation is
byte-compatible, missing pair inputs
report UNPROVEN rather than failing,
and geometry disagreement reports
UNPROVEN rather than guessing. The
self-test passes five cases through the
real `analyse_pair`. No IA filed — the
snap region is V7's outright; the
boundary with V14 is stated in both
findings and the loop side already
agreed. First real execution is the
Wave V integration dispatch; remainder
(3) is the context it runs into.

**Registry subtitle round trip (V8).**
S18 proved the core level; this proves
the registry path in a new
`tests/wave_v_subtitle_registry_e2e.rs`
(3 tests, every call through
`mcp::all_tools()`). The VTT leg adds
three cues out of order (the sorted-
insert reply indices asserted: 0, 1,
then 0), sets a non-default style,
asserts `subtitle_list` byte-equal
(the uncredited cue's object carries no
`speaker` key), exports and asserts the
document itself (timing line, exactly
two `<v …>` voice tags), imports into a
fresh core and lands on cue-for-cue
equality with a byte-identical
re-export. The style claim is stated
plainly in the file: a VTT document
carries cues, not style, so the leg
asserts the two guarantees that exist —
style round-trips through the registry
surface in stored form, and import
replaces only the cue list (a style set
beforehand survives). The undo leg
witnesses depth across adds, the style
set, the no-op set (R20 parity), the
export (spends nothing, leaves
`core.status` untouched) and refusals;
the SRT leg asserts `speakers_dropped
== 2` at registry level for the first
time and the key's absence when zero.
fps 24.0 explicit on every core; cue
boundaries on exact half seconds. No
IAs.

**Bundle determinism (V9).** A-B-A over
`export_bundle` (images kind) through
the real registry on U14's populated
fixture class — bends authored, rain,
a credited two-line cue, an encoder-
made media sprite — in a new
`tests/wave_v_bundle_determinism.rs`
(2 tests): every reply asserted whole
(`succeeded == 4`, `failed == 0`), the
tree asserted to be exactly the 16
expected members with the credited
"MARA" literal inside the frame-0003
SVG before any identity is demanded,
then byte identity member by member
with P47/S6-style difference reporting.
The scaled leg adds the cross-scale
contract: scaled PNG members are
1920×1440 against 960×720 and every
non-PNG member is byte-identical across
scale classes — T8's PNG-only routing
proven on the populated fixture at
registry level. The video kind is
excluded plainly, on the board's own
ground: the encoders shell out to an
external ffmpeg whose presence and
version a library test cannot assume,
and the container carries encoder
metadata, so cross-run byte identity is
not a property the engine promises
there; remainder (8) carries it.

**Cross-feature e2e (V10).** A new
`tests/wave_v_e2e.rs` (6 tests), three
legs. (1) The Top-position raster
proof at core level, through
`render_frame` on the burn-in fixture
shape with every expectation derived at
run time from the shared helpers
(`subtitle_line_center_y` and the stack
offset rule — never a hardcoded band):
a single Top cue's cluster at the
helper line with zero face ink in the
bottom half; a two-cue stack reading
away from the top edge in cue order; a
multiline cue whose narrow first line
sits in the edge band and wide second
line one band in — the exact inverse of
the Bottom pin; and half-open liveness
on [2, 4) with frame 4 byte-identical
to the cueless control. (2) The JPEG
chain measured, never byte-compared:
encoder-made JPEG in a file named
`photo.bin` (content decides) →
`asset_import` → `sprite_add` → scaled
PNG sequence export, the instanced quad
located and sized in the decoded frames
against an unscaled control (coverage
ratio asserted in 3.2..=4.8). (3) The
name-fidelity leg, written against
V1's board contract while V1 ran
concurrently and saying so in its
header: on the pre-V1 tree its case
assertion fails on the defect itself;
against V1's landed fix it asserts
save "Styk" → listed "Styk", no "styk"
→ insert by that name.

**Catalogue hygiene wiring (V11).**
U10's WIRE list, executed: 10 of its
12 keys wired at 16 production sites
across 9 files — `settings` (2 sites),
`live_mcp_control` (the settings toggle
row), `figures`, `add_figure`,
`easing`, `discard_changes` (the
chrome app-bar site), `rename` (pose
library, figure library, figure
inspector), `go_to_start`, `go_to_end`
(both branches each), `tween_play`,
`clear_drawing` — all with existing
keys, English byte-identical, per-panel
pins in the Q7 idiom, and the element-
key set digest identical before and
after (149 pairs). The sites sitting
in V3's and V5's files were filed, not
edited, as **IA-V11-1**: the help
panel's title → the existing `help`
key; home_panel's buttons-row Rename →
`rename` (the same literal V3
deliberately left — the two findings
agree on the site); plus two same-key
literals V11 found that U10's ledger
had not catalogued. The integrator
reconciles IA-V11-1 against V3's
landed wiring at the apply pass: V3
already wired the rename-line confirm
and the armed discard forms to these
keys, so what remains of the IA is the
buttons-row site, the help title, and
whichever of V11's two extra sites
V3's pass did not reach. One item is
recommended against, in writing: the
help Features list's "Live MCP control"
label lives in a `(&'static str,
&'static str)` data table whose tests
read the literal as data — wiring it is
a shape change, not a swap, and stays
with a later slice (remainder (7)).

**Render-phase progress (V12).** U11
scoped the encode phase only — verified
in the tree, not just in u11.md — so
the board's first branch applied and
the render phase got its own boarded
type: `RenderProgressEvent` /
`RenderProgressCallback`, phase-pure
beside the encode pair so the phases
can never be conflated, wired into the
render loops of the two sequence
families (PNG and JPEG; full, range,
scaled, range-scaled — eight new
`*_with_render_progress` entries). The
one deliberate asymmetry with the
encode contract is stated in the type's
doc: sequence export is synchronous, so
delivery is pinned exactly — one event
per written frame from the caller's
thread, the final frame's event IS the
terminal, a successful export of N
frames delivers exactly N events, an
empty project's hooked export fires
exactly the `(0, 0)` terminal, and
refusals (invalid scale, inverted
range) fire nothing and write nothing.
**IA-V12-1** specifies the panel/MCP
wiring in IA-U11-1's terms and wires
nothing — no surface displays progress
yet, and nothing displays fake
progress. Named out of scope by
construction: the ffmpeg funnels'
temp-PNG loops (they feed the encode
phase) and GIF/APNG/spritesheet, whose
render and encode are interleaved in
single loops — a counter there would be
whole-export progress mislabelled as a
phase (remainder (6)).

**Gate/census maintenance (V13).**
Verification only, no edits — the
finding, again, is that none were owed:
on a clean detached worktree at
`1c7a70d` the gate reads PASS 10/10 and
the census 16/16 at **381**, both
floors already equal to their verified
counts (element keys **980** =
864 + 116; the ratchet T15 made is the
count), i18n recounted directly at
**563 × 28** identical sequences with
zero value==key, scenes 34, palette
134, icons 163, State 48 fields /
22 literals. The mid-wave live recount
also read 10/10 with no transient, and
its one delta is Wave V's own: element
keys **983** (+3 dynamic, V6's
steppers) — a pending delta, the floor
staying 980 until the integrator
recounts on the settled tree (V20's
ledger carries the attribution and the
ratchet arithmetic). V13 also records
HEAD advancing to `fbc3644` during its
slice without moving any counted
surface.

**Overflow/headless maintenance
(V14).** One real repair: the xvfb
fallback tree loop had drifted to 32
of the 34 scenes — it never named the
Wave 7 light pair (`default-light`,
`settings-open-light`) — so a plain-
attempt miss on either could never be
recovered by the fallback whose one job
is recovering missed dumps, and neither
tree is in SCAN_SET to flag the
absence. The fallback list now mirrors
the plain list exactly, with the reason
commented in the workflow; the gate's
coverage contract (the plain loop is
the contract, the fallback report-only)
is unchanged, and the three 1280×720
size-variant probes stay plain-only
deliberately, as every wave before.
SCAN_SET verified at 16 required /
0 pending at base and at final check;
no Wave V slice registers a scene.
`overflow_scan.py` gained only the
per-wave docstring maintenance note.
The V7 boundary is stated in both
findings and agreed: V7's scenes were
already in both loops; its snaps
produce no dumps; no IA from V6 or V7
named workflow wiring for V14 to apply.

**MCP docs sync (V15).** No edit to
docs/MCP.md, and that is the verified
result: census re-verified at **381**
(44 + 319 + 9 + 4 + 5, no duplicates),
the docs' count prose agreeing; IA-
U11-1 prescribes no description change
(it is a do-not-wire instruction), and
`git diff d39fc10..1c7a70d` over the
MCP sources is empty — U11 touched
`export.rs` only, so no description
could have drifted in the Wave U
integration. The drift sweep attributes
the rest: 296 rows character-identical,
85 differing — and the same sweep over
a pristine `d39fc10` tree yields the
byte-identical differing set, so all
85 are the pre-existing condensed form
(the `shell_*` bulk), standing docs
form since before Wave T, **zero new
drift**. If V12 had filed a description
IA it would have been applied verbatim;
it filed none (IA-V12-1 is a wiring
specification).

**README truth pass 3 (V16).** Two
edits: the shell bullet now states the
catalogue total Wave U settled (563 UI
strings per language table, 28
languages), and the export bullet gains
the encode-progress paragraph written
to IA-U11-1's exact scope — core-level
hook, the encoder's own counts, the
single finished event closing the video
encode while the MP4 audio mux can
still fail after it, and "neither the
export panel nor the MCP server
surfaces it" said in the README itself.
Counts re-verified from the tree rather
than inherited (census 381; i18n
563 × 28 by the delimited parse, the
564 raw tuple count and its
`status_ready` explanation recorded;
scenes 34 — the README makes no scene
claim). The name-fidelity claim was
deliberately NOT written: v1.md had
not landed at V16's final check, so
the README asserts nothing about
library name case; the claim waits for
V1 + CI. T14's flags are carried
forward again (remainder (10)).

**QA_RUNE re-audit (V18).** §17 closed
in Wave S; this is the full re-audit,
from a pristine archive snapshot so
concurrent edits could not contaminate
counts. QA_RUNE gains a re-audit
pointer at its header (§§1–5 are the
2026-11-08 snapshot; §18 is now the
standing-claims surface), the §4 rows
4–6 filled (headless assertions are
routine — 34 dumps per dispatch, facts
1..=36, SCAN_SET 16; the §3 rect
question resolved — dumps carry rects
and a top-level capture `scale`,
consumed by the overflow scan and the
subtitle census; the first user-PC
screenshot review marked DONE
2026-11-08), a supersession note after
§17 resolving its in-flight cells with
verdicts, and §18 itself: the verified
counts table, the post-§17 surfaces and
contracts, the CI frame through Wave U
round 2 (round-2 headless was still in
flight at V18's filing; this section's
frame above completes it from the
artifacts). The one genuinely new
finding is a counting artefact, filed
as its R1: structurally the catalogue
holds **564 distinct keys per table**
— the stated 563 is the line-wise
count, and the difference is exactly
`status_ready`, the catalogue's only
multi-line tuple, invisible to
line-wise counters (pre-existing: 360
vs 361 at the S+T base; F4 recorded the
observation at integration). No key is
lost, duplicated or desynchronised;
every count statement inherits the
artefact, and the remedy is the
integrator's (remainder (4)). §18.5
also files R2 (`help_panel.rs` prose
still says "54 rows" against its own
61/36 pins — addressed to the help
surface's owner), R3 (`subtitles.rs`'s
module doc still calls the speaker
prefix a filed IA no renderer draws —
stale since Wave R), R4 (the progress
wiring, boarded not built) and R5 (the
figure-library defect confirmed at the
audit base, V1's to close).

**PHASE_O table refresh (V19).** S8's
discharge table (PHASE_O §10; "§2" is
the p05 method it applies) is
re-sourced and re-verified first-hand:
the table now cites headless
**42765217055** (S+T round 2 on
`9d7dd0a`), downloaded and rescanned —
all 36 dumps at 0 non-exempt hits, all
34 of S8's rows reproducing exactly
(window + element counts identical),
both rows added since (subtitles-
speaker, home-missing-audio) CLEAN, and
the O-1…O-9 measurements unchanged to
the digit. **The refresh moves no
number.** What it adds is the
disclosure the original discharge
owed: S8's cited run 42759195547,
re-downloaded and read first-hand, ends
`SUBTITLE CENSUS: FAIL (5 assertions)`
inside its SUCCESS workflow — the
original table cited a census-red run
without saying so; its overflow verdict
was genuinely CLEAN under the then-13-
tree contract and stands, and §10 now
says all of this plainly and cites a
census-clean run (19 PASS / 0 FAIL;
marker PASS alongside). One provenance
wrinkle recorded in §10, not hidden:
the cited artifact satisfies 15 of the
current contract's 16 required trees —
it predates `home-scan-fallback`, so a
rescan under today's SCAN_SET reads
INCOMPLETE on exactly that tree. At
V19's final check the Wave U round-1
run had produced no dumps (the E0308
round) and round 2 was in flight; the
citation upgrades to 42767540386 —
census-clean, 16/16 complete — at the
next refresh.

**Integration-eve rehearsal (V20).**
No source ownership; every local
instrument run, repeatedly, on a moving
tree, with each number naming the tree
state it sampled: gate PASS 10/10 three
times (clean HEAD, mid-wave, final
tree — check 2 zero keys lost in every
changed file), MCP census PASS at 381
on HEAD and on the final tree, overflow
and subtitle-census self-tests PASS,
and the thumbnail probe's self-test
PASS including V7's extended pair cases
on the final tree. **No instrument
failed at any point; no real regression
was found.** The ledger reconciles the
two element-key extractors honestly
(the gate's ratcheted method reads 980
at HEAD and 983 on the final tree; the
standalone inventory reads 1014 / 1017
— a constant +34 method offset, not
drift), attributes the +3 dynamic
patterns to V6's channel steppers in
`subtitle_panel.rs` alone, and computes
the pending i18n arithmetic from the
findings as filed at its check: +120
across IA-V2-1/V4-1/V5-1/V6-1, minus
IA-U10-1's 52 = **631 × 28**, with
IA-V3-1 then still unfiled. V3 has
since filed (31 keys), so the
integrator's projection at this
section's write time is 563 + 151 − 52
= **662 × 28** — pending arithmetic,
labelled as such, until the inserted
tables are recounted; V13's ledger
computes the same 631 independently,
and the two ledgers agree to the key on
everything both measured.

**Integration ledger — every Integrator
Action filed across Wave V's findings.**
(1) **IA-V1-1**: exact replacement text
for `tests/wave_r_undo_census.rs`
restoring "Styk" in the legs the defect
forced to "styk" (+0 keys). (2)
**IA-V2-1**: 5 `subtitle_*` template
keys. (3) **IA-V3-1**: 31 `home_*` keys
(templates flagged per key in v3.md);
(4) **IA-V3-2**: compile-blocking —
three `&s.core` call-site updates in
scenes.rs's test module, exact text in
v3.md; the tree does not compile
against V3's new signatures until they
land. (5) **IA-V4-1**: 3 chrome keys.
(6) **IA-V5-1**: 109 `help_*` keys (one
template). (7) **IA-V6-1**: 3
`subtitle_red/green/blue` keys. Items
(2)–(7) total **151 new keys**. (8)
**IA-V11-1**: wiring only, all keys
already in the catalogue — the help
title → `help`, home_panel's
buttons-row Rename → `rename`, plus
V11's two extra same-key sites,
reconciled against V3's landed wiring
at the apply pass (see V11's
paragraph); its one negative
recommendation (the Features-list
"Live MCP control" literal) is NOT
applied. (9) **IA-V12-1**: no insertion
— the render/encode progress wiring
specification in IA-U11-1's terms
(panel background-job shape; MCP
`notifications/progress` transport),
recorded below as remainder (6). (10)
**IA-U10-1** (carried from Wave U,
deferred to this wave by the board):
Part 1 removes the 52 REMOVE keys ×28
with every companion edit U10
enumerated (SHELL_KEYS 31→25, NEW_KEYS
12→4 with its value assertions,
MISSION_KEYS 18→7, the DOPESHEET_KEYS
const/loop, the named value assertions,
count-doc updates) — **its companions
must land in the same push as the
removals or the i18n tests go red by
design**; Part 2's 12 WIRE sites were
V11's slice (landed in tree except
IA-V11-1's collisions, item (8)); the
7 RESERVED keys stay untouched. No
actions were filed by V7, V8, V9, V10,
V13, V14, V15, V16, V18, V19 or V20.

**Verification — awaiting CI.** Wave
V's counts are V13's and V20's,
verified on the clean integration HEAD
and again on the live tree, not taken
from siblings' claims: gate PASS 10/10,
census 381 (44 + 319 + 9 + 4 + 5),
element keys 980 settled at `1c7a70d`
and **983 in the Wave V tree** (+3
dynamic, V6's steppers; the floor stays
980 until the integrator recounts on
the settled tree and ratchets — it
passes as-is, 983 ≥ 980, and must not
be lowered), palette 134, icons 163,
scenes 34, SCAN_SET 16 required /
0 pending, i18n **563 × 28** in tree
becoming a projected **662 × 28** when
the 151 IA keys land and IA-U10-1's 52
removals are applied (pending
arithmetic; not verified from the tree
and not a floor — and see remainder
(4): the structural count runs one
higher than the line-wise figure at
every step of that arithmetic). CI —
the Wave V integration push's Build,
Test & Binary plus a headless dispatch,
transcripts read — is the proof of
everything above, including the first
execution of every extended pin, of
V7's pair gate against real snaps, and
of V14's repaired fallback loop.

**Honest remainders.** (1) The standing
user decisions stand exactly as §6.15
(as corrected) left them —
white-on-accent contrast (3.14:1), the
panel splitter, File-menu Save As, the
two disagreeing "New project" results,
`tab_visits_controls` OFF, project-swap
undo, the skew channel, the whole-
figure pose transform, MCP
`write_recovery`, the waveform
badge/call, S1's seven i18n
value-vs-label discrepancies (neither
side changed; U10's audit confirmed
zero `tr()` consumers for the six keys
and they stand RESERVED), the
subtitle-stack cap/wrap — and this wave
touched none of them. (2) GIF import
stays undecided (the funnel refuses it
deliberately); a panel export-scale
control remains unbuilt by design; the
bundle's JPEG member stays 1.0, so
whole-bundle scaling is a follow-up if
the user wants it. (3) **The Home
thumbnail probe's control premise trips
on current snaps**: on Wave U round 2's
artifact the legacy probe reads the
home-totals control strip at 0.672
stage-bg share against its 0.06 ceiling
and issues no verdict — whether the
strip moved, the theme drifted, or the
premise was always fixture-specific is
not established by any run yet. V7's
pair gate (in tree, self-test green)
inherits Q16's floors and that control
for its missing-audio leg, so its first
real run — the Wave V integration
dispatch — is where this premise is
answered; until that transcript reads
PASS with the pair sections present,
Home thumbnails are unit-proven and
snap-captured but not snap-proven.
(4) **The i18n count artefact (V18's
R1)**: the stated counts (563 now, 662
projected) are line-wise; the
structural count is one higher at every
step (564 / 663) because `status_ready`
is a multi-line tuple. No key is lost
or desynchronised. The integrator
settles it one way or the other at the
insertion pass — one-line reformat ×28
or restated counts — and every count
statement (`src/i18n.rs:13`,
`i18n_wire.rs`, this document, MEMORY)
should move together. (5) Doc-prose
staleness filed by V18, neither fixed
by its owner nor this wave's code:
`help_panel.rs`'s module prose says
"54 rows" against its own 61/36 pins
(R2 — the help surface's next slice;
V5 wired the render layer and did not
rewrite the prose), and `subtitles.rs`'s
module doc still describes the speaker
prefix as an undrawn filed IA (R3 —
stale since Wave R landed
`subtitle_display_lines`). (6) Progress
remains core-only by design: U11's
encode contract and V12's render
contract are landed and pinned, and no
surface reports either — the panel
background-job wiring and the MCP
`notifications/progress` transport are
boarded specifications (IA-U11-1,
IA-V12-1), not built work; the ffmpeg
funnels' temp-PNG render loops and the
interleaved GIF/APNG/spritesheet loops
have no phase-pure progress by
construction, and audio-mux progress
does not exist. (7) The help Features
list's "Live MCP control" label stays
English-canonical in its data table
(V11's recommendation, V5's
concurrence): wiring it is a data-shape
change with test consequences, owed to
a rostered slice, not the integration
pass. The settings-popover toggle row
of the same words IS wired (V11). (8)
Bundle determinism is proven for the
images kind only; the video kind's
cross-run byte identity is not a
property the engine promises (external
ffmpeg, container metadata) and stays
unlegged unless a future slice legs it
on looser terms with the reasoning
stated. (9) English residue by design:
status sentences in every wired panel
(incl. Home's 16 and the clips panel's
2), the export class-C status/recap
sentences (machine-read, standing
exclusion), the open-recent not-found
sentence, the waveform strip's health
caption, key-cap notation and shortcut
hints, the app name, and the serialised
vocabularies everywhere. (10) T14's
flags, carried by U16 and V16
unchanged: no LICENSE/COPYING file in
the repo root, the README's
build-from-source framing, and the deep
feature prose never line-audited. (11)
docs/MCP.md's 85 condensed rows stand
as the catalogue's standing form (V15
re-attributed them: zero new drift) — a
full-verbatim docs slice is a possible
future wave, not drift. (12) §6.15
remainders closed here: the subtitle
census's coordinate contract (fixed on
the S+T line; its floors ratified in
Wave U round 2, above); the S+T build's
owed round 2 (GREEN, 42765201236 /
42765217055); IA-U12-1's one-push
pairing (landed in `3480d1d`, its marker
PASS in round 2); IA-U4-2's call sites
(landed — and their one-character
defect is round 1's whole story, above);
U10's outstanding audit (filed, and its
WIRE half executed by V11; its REMOVE
half is IA-U10-1 at this wave's
integration); U11's unboarded
follow-ups (the render phase is V12;
the panel/transport wiring stays
boarded, remainder (6)); and the scan-
fallback line's headless proof (scene
in `3480d1d`, marker PASS in round 2).
And sound remains user-verified only.

**Integrator's note (F5, Wave V
integration).** The filed integrator
surfaces landed in one pass. Catalogue:
the five Wave V IA blocks (IA-V2-1,
IA-V3-1, IA-V4-1, IA-V5-1, IA-V6-1) added
**151 keys** in roster order to all 28
tables — genuine translations in BN /
ES / DE / FR / IT / PT_BR / RU /
ZH_HANS, English placeholders in the
other 19 (ES419 included), every
`{slot}` kept in every table — and
IA-U10-1 Part 1 removed the **52** dead
keys (1,456 lines), with every companion
edit (NEW_KEYS 12→4, MISSION_KEYS 18→7,
the DOPESHEET_KEYS const/loop and the
`dopesheet_keys_resolve_through_tr`
lines, the figures_layers / onion_skin
value assertions): **563 + 151 − 52 =
662 × 28** by the project's line-count
convention (the `status_ready` tuple
disclosure stands, unrenumbered). One
recorded deviation: SHELL_KEYS went
31 → 24, not U10's 31 → 25 — its §8
lists `onion_skin`'s SHELL_KEYS
membership as a reference, and with the
key removed from the tables, keeping it
listed would leave
`shell_key_coverage` reporting it
missing. The 7 RESERVED keys are
untouched. IA-V1-1 and IA-V1-2 applied
verbatim (the census's "Styk" legs and
the three figlib suites' listing
assertions; file-path/slug assertions
unchanged). IA-V3-2 applied (the three
`&s.core` call-arity fixes in
scenes.rs's pins). IA-V11-1 applied
reconciled against V3's landed wiring:
the help-panel title now resolves
through `help`, and the project-row
buttons' "Rename" through `rename`;
two filed sites were skipped as
already wired by V3 (the rename-line
confirm through `rename`, the open-row
armed prompt through
`discard_changes`), and the help
feature-list "Live MCP control" stays
unwired per V11's negative
recommendation (a `&'static str` data
table). Element keys recounted on the
final tree with the gate's extractor:
**983**, agreeing with V13/V20 —
KEY_TOTAL_FLOOR ratcheted 980 → 983.
Count docs updated (i18n.rs header,
i18n_wire.rs header, README languages
bullet): 662. Status: IMPLEMENTED
until CI.

### 6.17 Wave W — progress surfaces
land, live-MCP hardened, libraries
reconciled (2026-11-10)

Ordered 2026-11-10 (the standing
continuous order, re-issued for the
ninth time and boarded ahead so this
wave launched the moment Wave V's
integration pushed): "assign 20 agents
continuous write code if ci gets error
then fix that". Twenty agents, one
owner per file, integrator surfaces via
filed actions (board:
`~/workspace/wave-w/BOARD.md`; findings:
`~/workspace/wave-w/findings/`). The
board's base is the Wave V integration
HEAD `d5da9fd` (662 × 28, scenes 34,
MCP 381, element keys 983);
`src/i18n.rs` and
`src/rune_sec/i18n_wire.rs` were frozen
to this wave's agents, all key work
arriving via IA. This section was
written from all nineteen sibling
findings (w1–w16, w18–w20; w17 is this
section) and from the CI frame read via
`gh` at the final check, headless
transcripts read from the runs'
downloaded artifacts — never from
workflow conclusions alone, per the
42759195547 lesson. Every Wave W status
below is IMPLEMENTED (in tree,
uncommitted at write time); Wave W has
no CI verdict. The Wave V frame, by
contrast, is closed here, exactly,
including the round a green conclusion
would hide and the probe verdict that
is not the one the wave was promised.

**The Wave V integration ledger.** Six
commits on top of `fbc3644`, in order:
(1) `965d085` — figure-library name
fidelity (V1): the 2D figure library
was the only library that lost names
(pose/fig3 already store them); save
now writes the def JSON plus a
top-level name field with the exact
name, load prefers the stored name with
stem fallback for legacy files, the
on-disk slug never changes and the file
still parses as a bare FigureDef;
rename/delete resolved backing files
from display names and broke on
exact-name entries — both now resolve
through `library_entry_file` (verbatim
first, then slug), rename settles
same-name on stem comparison and syncs
the stored name so a reload cannot
resurrect the old one; migration:
nothing renamed/moved/rewritten at
load, a legacy entry gains its exact
name at next save; IA-V1-1 (census
"Styk" change-back) + IA-V1-2 (three
suites' listing assertions) applied
verbatim, slug/file-path assertions
untouched. (2) `db25b8b` — i18n
catalogue 563 → **662 × 28** (F5):
+151 keys (subtitle readouts V2 5,
home V3 31, chrome V4 3, help V5 109 at
the render layer over byte-identical
canonical data, subtitle RGB V6 3) and
IA-U10-1's 52-key removal with all
companions (NEW_KEYS 12→4, MISSION_KEYS
18→7, DOPESHEET_KEYS deleted;
SHELL_KEYS 31→24 — one beyond U10's
stated 25: `onion_skin` is itself a
removed key and keeping it listed fails
the coverage test, documented in f5.md,
reversible); RESERVED 7 byte-identical.
Also landed: the subtitle RGB channel
steppers (V6 — the one real style gap;
SubtitleStyle has no background/opacity
field and none was faked), V11's 16
hygiene wirings + IA-V11-1 reconciled
against V3 (help title + home Rename
applied; two sites already wired by V3
skipped; the "Live MCP control"
data-table item left per V11's negative
recommendation), IA-V3-2 scenes arity
fixes. (3) `c8582a4` — render-phase
export progress (V12): a phase-pure
RenderProgressEvent/RenderProgressCallback
beside U11's encode contract,
synchronous delivery pinned exactly
(one event per written frame, the final
frame's event the single terminal;
refusals fire nothing; a mid-loop
failure propagates with no terminal;
empty project fires exactly the (0,0)
terminal); eight
`*_with_render_progress` sequence
entries, plain entries delegate with
None; IA-V12-1: MCP not wired (no
sink), panel wiring boarded as the
background-job slice — Wave W's W1;
named remainder: the video funnels'
temp-PNG render loops adopt the
callback in Wave W (W2). (4) `2930922`
— headless + instruments: the Home
snap pair gate (V7 — snap-home-missing-audio
capture + a content-fidelity pair gate
in the thumbnail probe: populated's
three strips are exactly two identical
+ one different; missing-audio's strip
equals the singleton and differs from
the pair; missing inputs report
UNPROVEN, never a false fail), the xvfb
fallback loop repair (V14 — it listed
32 of 34 scenes; the two light scenes
could never be recovered by the
fallback), overflow docstring
maintenance, KEY_TOTAL_FLOOR ratcheted
980 → 983 (F5's recount, agreeing with
V13/V20: V6's +3 dynamic stems). (5)
`2e33c12` — three proof suites
(V8/V9/V10): registry-level subtitle
round-trip (WebVTT cues+speakers
byte-identical re-export, undo
discipline, SRT `speakers_dropped == 2`
at registry level; style asserted in
the two senses the registry guarantees
— VTT carries cues, not style); bundle
determinism A-B-A at 1.0 and 2.0 with
the PNG-only cross-scale contract
(video kind excluded on the board's
stated ground — ffmpeg wall-clock
metadata is not an engine property);
cross-feature e2e (Top-anchor raster
proof derived from the subtitle
helpers, JPEG-import → sprite →
scaled-export chain measured decoded,
name-fidelity leg live against V1's
fix + the legacy-library migration
guard). (6) `d5da9fd` — docs: §6.16
itself (V17 + F5's integrator note),
README truth pass 3 (V16), QA_RUNE §18
re-audit (V18), PHASE_O §2 table
re-sourced to run 42765217055 (V19).
The integrator's own instruments on the
committed tree before the push: gate
**PASS 10/10** (element keys 983 =
864 static + 119 dynamic, floor 983;
State 48/22; palette 134; MCP 381 =
44+319+9+4+5; icons 163; scenes 34;
layout facts 1..=36; docs/MCP.md in
sync), MCP census PASS.

**The Wave V CI frame — two rounds,
read at the transcript level.** Round 1
(on `d5da9fd`): headless 42771069640
**FAILED** at its release Build step —
**4 × error E0716** (temporary value
dropped while borrowed) in
`src/rune_sec/home_panel.rs`, at lines
3052 (`home_sort_name`), 3054
(`home_sort_recent`), 3071
(`home_empty_library`) and 3073
(`home_empty_search`): V3's Home wiring
passed `&i18n_wire::tr(&s.core, …)` —
`tr` returns a `String` — into
conditional branches feeding `btn` /
`empty_state`, which take `&str`. In
the build workflow nothing ran past the
Build step; build 42771042398 was
**CANCELLED** (superseded by the fix
push — no verdict exists for it, and
none is claimed). In the headless
workflow the probe binary never built,
so no snaps or tree dumps exist for the
round: its artifact is eight fail-soft
stub files — the subtitle census reads
FAIL (1 assertion — snap inputs
present), the Home thumbnail probe
UNPROVEN (inputs missing), the MCP
probe FAIL (1 assertion — GET
/health), the rect probe unanswered,
and neither Home marker file was
produced. Nothing in round 1 is
evidence about the app; it is evidence
about four borrows in one panel. The
fix is `2462a82` — "bind Home sort/empty
labels to locals (E0716)": the four
`tr()` Strings are bound to `let`
locals before the branches, one file,
18 insertions / 10 deletions. Round 2
(on `2462a82`): build 42771751700
**GREEN** — 4,024 tests passed across
54 targets, zero failures, zero errors
in the log; headless 42771786953
**GREEN**, and its transcripts, read
from the downloaded artifact (224
files), are the round's real verdict:
**SUBTITLE CENSUS: PASS** (19 PASS /
0 FAIL; capture scale 2.000 derived
from the dump's own bounds; speaker
face 3086 px, extent 702 stage units
against the ≥ 396 gate; lane diff 3612
/ ink 1163 against floors 200 / 100).
**HOME MISSING-AUDIO MARKER: PASS**
and **HOME SCAN-FALLBACK MARKER:
PASS**. **MCP PROBE: PASS** (the live
bridge answering on the machine, tool
count ≥ the 370 floor, the shell
workspace switch exercised end to end).
The overflow scan over the run's
artifacts, run locally on the download:
**CLEAN (0 hits, scan set complete)**
across the 16 required trees (37 dumps
in the artifact). One transcript does
not close, and it is the one this wave
was rostered to answer: the **Home
thumbnail probe reads UNPROVEN** — on
both its sections the legacy control
trips (home-totals stage-bg share
0.672 on the populated snap, 0.681 on
the missing-audio snap, against the
0.06 ceiling), strip extraction never
happens, and the fidelity gate reports
only "a row strip could not be
extracted". **The pair gate's first
real verdict is therefore UNPROVEN —
not PASS, not FAIL**: its identity
legs never executed against a real
snap. W7's diagnosis (below) answers
why, and answers it against this very
artifact set: the control's premise was
always fixture-specific, and the pair
gate's own identical-strip premise is
refuted by measurement on the round-2
predecessor's snaps. Net Wave V frame:
round 1 red on four borrows in an
applied IA wiring, round 2 **fully
green with every transcript read** —
build, census, both markers, MCP
probe, overflow — with the thumbnail
probe honestly UNPROVEN and its
redesign rostered, remainder (2).
Wave V is verified.

**Panel progress wiring (W1).**
IA-U11-1(b) and IA-V12-1(b) are
implemented as ONE background-job
mechanism in the export panel, exactly
as both IAs prescribe. The four formats
with a boarded progress contract —
PNG/JPEG sequences (render phase, V12)
and MP4/WebM (encode phase, U11), full
and range — now run on a worker thread
over a cloned `Project`; the two
phases' progress state lives in two
separate atomic slots, labelled
separately, never merged; the shell's
frame tick (`main.rs::poll`) polls the
job and re-wakes at 100 ms while one is
active; on completion the worker's
result is turned into the panel's
status and recap by the SAME wording
code the synchronous actions used, byte
for byte — including the MP4
missing-audio note composition.
Progress text never enters
`core.status`; every number shown comes
from a contract event — no estimated,
timed or interpolated progress anywhere.
While a job runs, every export press in
the panel is a no-op (today's
one-export-at-a-time semantics, without
the block); a worker that dies without
an outcome finishes as a failure
through the same per-op failure wording,
never silently dropped. The encode
terminal reads as "Encode finished"
only — never as export success, because
an MP4's audio mux can still run (and
fail) after it; the render terminal
needs no finished wording because the
outcome replaces the readout the same
tick. Formats with no boarded contract
(SVG, GIF, APNG, spritesheet, contact
sheet, project formats, both bundles)
keep the synchronous path unchanged.
Video jobs report the encode phase
only: at this base the hooked video
entries accept no render hook (W2's
adoption, below, makes the panel's
second slot light up additively).
**IA-W1-1** files the four catalogue
keys (`export_progress_working`,
`export_render_progress`,
`export_encode_progress`,
`export_encode_finished`); until the
integrator inserts them the readout
falls back to key text (the Q7 pattern)
and W1's pin says so. No cancel
affordance was built — the contracts
have no cancel path, and the panel
gains no fake button for it.

**Video funnels adopt the render
contract (W2).** The one render phase
V12 left unreported — the temp-PNG
render loops inside the MP4/WebM
funnels — now reports through V12's own
`RenderProgressCallback`, exactly as
its §4(c) remainder prescribed, without
a new type. Eight new
`*_with_progress` video entries carry
both hooks; every pre-existing video
entry keeps its exact signature and
delegates with `None`, so unhooked
behaviour is byte-identical. For video,
`total_frames` is the number of sampled
frame times — the temp PNGs the loop
will write — the same total the encode
phase reports against and the count the
export returns; the phases keep their
own event types, so the counts still
cannot be conflated. Refusals fire
nothing on either hook (including the
range entries' audio-plan refusal,
which precedes any render); a failed
encode after a completed render leaves
the render stream standing, each event
true when sent; `finished` closes the
render phase and is explicitly NOT
export success for video (encode and
mux still run). Six pins cover the
dual-hook streams, the scale-1.0
route-through, the failed-encode
boundary, the refused-range silence and
the panicking-observer containment.
Panel/MCP wiring unchanged by this
slice — the dual-hook entries are the
core surface W1/W3 consume. Its hunks
were re-verified against W6's
concurrent edits in the same file: the
only shared lines are the four range
entries' `render_progress` call-site
arguments; every audio line is W6's,
untouched.

**MCP progress transport (W3).** The
audit-first slice's design closes, and
it is built. The blocker U11 named is
real: `Tool.handler` is a bare fn
pointer with ~381 registrations, so
the design does not touch it. Instead
the router gains a transport-supplied
notification sink: a `tools/call`
carrying `_meta.progressToken` on a
streaming transport now streams
`notifications/progress` while the
export runs — encode counts for video,
render counts for sequences, translated
from U11/V12's core events one
notification per event, terminal last,
the reply byte-identical with and
without a token. Gating is opt-in twice
over (token present AND sink
installed); the sink is parked on
`EditorCore` for the handler invocation
and restored around the existing
`catch_unwind`, so even a panicking
handler cannot leave a dead call's sink
behind, and direct handler invocation
in tests (fresh core, sink `None`) is
unaffected by construction. The wire
form is exactly the 2024-11-05 shape
this server speaks — which has no phase
field, so the wire reports ONE phase
per call (video: encode; sequences:
render); the funnel render phase W2
threaded is deliberately not sent,
because unlabelled its counts would
reset the stream mid-call under one
token. Stdio is the streaming
transport: its sink writes through the
same locked writer as the responses
(one lock per line, no interleaving),
and `main` no longer holds a bare
StdoutLock across the run — the
deadlock the audit found (a notifier
blocking on the lock while the export
blocks on the drainer) is designed out.
The live HTTP bridge CANNOT stream
(one POST, one response, no second
channel) and never installs a sink, so
tokened calls there run exactly as
untokened ones; buffering notifications
to flush at the end would be the
fake-progress shape IA-U11-1 forbids.
`export_bundle` is deliberately NOT
wired: a bundle is four (or two)
separate exports under one request,
each member's counter starting at its
own 1 — one token carrying resetting
counts would run the client's bar
backwards; honest bundle progress needs
a core-level bundle progress type,
a future boarded slice. GIF/APNG/
spritesheet/SVG arms send nothing (no
phase contract exists). **IA-W3-1**
files the two description sentences
for docs/MCP.md (via W15's method,
applied by the integrator); the census
stays 381 — progress is a protocol-level
`_meta` convention, not a tool argument.

**Live-MCP default-off (W4).** The
audit found the transport's defences
sound and its default indefensible.
Bind (127.0.0.1:32117) and the
Host/Origin gate (loopback whitelist,
running before routing or body reads)
both verify SOUND, with pins. The gap
was the default state:
`LayoutPrefs::default()` had
`live_mcp: true`, the serde default was
`default_true`, the salvage path fell
back to on, and `main.rs` auto-started
the server — a fresh install launched
with unauthenticated live control
listening, opt-out rather than opt-in,
and any load-modify-save writer
cemented the `true` to disk. The fix
flips every default to off (serde
default → plain `#[serde(default)]`,
`Default` → false, salvage follows),
adds the pin
`live_mcp_defaults_off_until_explicitly_enabled`,
and corrects the two doc comments that
described the old behaviour. No auth
was added: the documented v1 design is
validation, not authentication, and a
loopback-origin page passes by design —
stated, not hidden. Migration is stated
plainly: none is possible — a stored
`true` is honoured, because the file
cannot distinguish a chosen enable from
a cemented default; one Settings toggle
writes `false`. **IA-W4-1** files the
`main.rs` launch-comment correction for
the integrator.

**Waveform health caption wired (W5).**
U5 localised the audio panel's health
surfaces but left
`waveform_view::clip_health_caption` on
canonical English — its helper had no
core in scope. W5's caller audit found
the constraint was the helper's own
signature, not its caller: the one
production caller already threads
`&s.core` into every sibling helper.
The caption now takes the core and
composes from the audio panel's shared,
already-public `health_segment` — one
health-word resolution path in the tree
(caption → `health_segment` →
`health_word_tr` → catalogue), no new
key, no edit to `audio_panel.rs`, the
`None`-for-healthy contract unchanged,
English byte-identical. **IA-W5-1**
files the two `audio_panel.rs` doc
sentences the wiring makes stale.
Adjacent observation, recorded as a
decision not an oversight:
`clip_caption`'s "muted" word in the
same file remains English-canonical —
outside U5's remainder and this slice's
brief; the catalogue's muted keys exist
if a future slice takes it.

**Range video audio completed (W6).**
The board's premise was half stale, and
the finding says so: range MP4 already
carried audio (Wave 9 WAV, Wave 10 any
ffmpeg-readable format, with the
refuse-before-writing unprobeable rule
and missing files named through
`Mp4ExportOutcome`). The genuinely open
half was WebM: video-only in every mode,
because the two mux endpoints hardcoded
`-c:a aac`, which WebM cannot carry.
The mux machinery is now
codec-generalised (`MP4_AUDIO_CODEC` /
`WEBM_AUDIO_CODEC = "libopus"`; the MP4
wrappers keep their exact behaviour and
every pre-existing AAC pin passes by
construction), and the two WebM range
entries mirror the MP4 range flow —
clamp, plan, refuse before writing
anything when a clip is unprobeable
(same error wording), otherwise encode
silent and mux the range's audio as
Opus. Full WebM stays video-only; only
the range entries changed. Six pins
cover the Opus argv shape, both mux
paths (direct and mixed-WAV), the
no-intersecting-clip silence, the
unprobeable refusal writing no file,
and the scaled entry. The known
limitation is stated plainly: the WebM
range entries keep their `Result<()>`
signature — there is no outcome type to
name missing clips through (MP4 range
has one); missing files are skipped
while unprobeable files refuse, so the
honesty-critical half is preserved, and
report parity is a separate API slice
(remainder (4)). The panel's guidance
note was classified before touching:
`RANGE_AUDIO_NOTE` is guidance copy,
not the machine-read class-C set, so
the const and its pin were updated —
while the class-C status/recap
sentences were NOT touched, and
therefore still say "video only" after
a range WebM export even though the
file now carries audio; that is a
consequence of the standing class-C
exclusion, flagged here rather than
edited around (remainder (5)).
**IA-W6-1** updates the VALUES of three
existing catalogue keys in all 28
tables (no keys added or removed; the
count stays 662 until W1's block lands);
until it lands, the panel's three
English pin literals fail — the known
Q7 pattern. W6 also flags the now-false
`export_file` description sentence in
`src/mcp_io.rs` ("'webm_range' never
carries audio…") and its neighbouring
comment for the integrator/W15, with
the docs/MCP.md row checked alongside.

**Legacy thumbnail probe diagnosed
(W7).** The premise was wrong; the
render is right. The legacy probe had
never issued a verdict on a real snap:
every real transcript on record —
42764348922, 42765217055, 42767540386,
and now round 2's 42771786953 above —
shows the identical control trip,
0.672 stage-bg share against the 0.06
ceiling. §6.16's open question ("the
strip moved, the theme drifted, or the
premise was always fixture-specific")
is answered: the premise was always
fixture-specific, in two independent
places, both invisible to the self-test
because Q16's fixture invented colours
the app never paints. Diagnosis 1: the
home-totals strip is 65.2 % the exact
colour (29, 34, 43) — #1D222B, the
theme's card background, which
`home_panel.rs` renders the totals line
on as plain text, exactly as the
control assumes — and the stage
background #232830 sits only (6, 6, 5)
from the card colour, so Q16's ±12
per-channel tolerance box swallowed the
card whole; the fixture's invented
panel colour sat outside even that box,
so it could never expose the
conflation. Diagnosis 2, which the
control trip masked: the ink predicate
(all channels ≥ 190) measures 0 px on
every real row — thumbnails are
Triangle-filter shrinks whose
anti-aliased strokes never reach 190 on
all channels, though the figure is
visibly there. The re-derivation moves
no floor or ceiling: `BG_TOL` 12 → 2
(under half the 5-channel minimum
distance between the two source-fixed
flat colours, so every pixel classifies
by its nearest source colour) and ink
becomes brightest-channel ≥ 100 (every
flat a strip can hold peaks below it,
and the slot is text-free by
construction). Measured on the real
artifacts: rows 0.333 / 0.337 / 0.339
(floor 0.18), control 0.008 (ceiling
0.06), ink 115 / 115 / 59 px (floor
15). After the fix the probe issues
**PASS on both real artifact sets** in
legacy mode (identical measurements —
the scenes are deterministic), the
self-test painters now use the real
card colour, and a new stage-washed
fixture pins the control's trip path to
UNPROVEN for the first time. The
flagged remainder is the section's most
important paragraph after the CI frame:
**V7's pair gate cannot pass as
designed, measured.** Its identity legs
demand pixel-identical strips (0
differing px); on 42767540386's
populated snap the identical-content
pair differs by 17,814 px at strip
level (≈17,452 of it the surround
alone — row 0 is the current project,
painted in the selected fill where the
others sit on card) and by 566 px even
inside the thumbnail band, MORE than
the different-content pair's 330 —
stroke-AA deltas between two renders of
identical frame-0 content exceed the
content signal (the added figure is
~15 red-ish px). At strip level and
thumb level the identical/different
distributions overlap and invert, so no
threshold on pixel-diff separates them,
and the cross-snap leg rests on the
same premise. The pair gate needs a
redesign around a signature the
thumbnails actually carry, with floors
derived from measured signals; it must
not be patched by loosening
`DIFF_FLOOR`, behind which there is no
separating value (remainder (2)).

**Progress contracts proven end to end
(W8).** A new suite,
`tests/wave_w_progress_e2e.rs`, drives
both contracts through the public
entries exactly as a consumer meets
them. Leg 1: the render contract's
exact stream through scaled and ranged
PNG/JPEG sequence entries, including a
ranged PNG at scale 1.0. Leg 2: the
encode contract via
`export_mp4_range_with_encode_progress`
— exactly one terminal, last, at (3,3);
intermediates strictly increasing,
their counts unpinned (wall-clock).
Leg 3 is the one that matters: phase
separation proven by measured
divergence, not assertion — a
held-frame fixture where render totals
count authored frames and encode totals
count sampled frames (ranged pair:
render 3 vs encode 4; full MP4 outcome
6 frames with no encode event carrying
total 5), so the two phases' counters
are shown to be different numbers about
different things. The registry
reachability question is answered at
this base: core entries only — the
registry `render` tool routes to the
plain entries and has no callback (the
IA-U11-1/IA-V12-1 design; W3's
transport, above, is what changes the
answer for tokened stdio calls). The
ffmpeg-on-PATH assumption is stated,
the same one the in-crate pins make. No
IAs filed.

**Pose/fig3 name paths fixed (W9).**
V1's defect class, generalised and
found in all three remaining libraries:
they list stored names from file
contents but resolved by
`<slug(name)>.json`, so an entry whose
file stem ≠ the slug of its stored name
(a hand-placed or renamed file) was
listed but could never be loaded,
applied, deleted or renamed. The 2D
figure library is coherent under V1 and
untouched. Broken and fixed: the 2D
pose library (load/delete/rename,
mirrored across `figure_ops.rs` and
`pose_tools.rs`), the 3D pose library
(load only, `fig3d.rs`), and the 3D
figure library (load + cycle, via
`fig3d::library_file_name`). The fix
pattern is one scan-based entry-file
resolver per library — the file whose
stored name equals the requested name,
matches sorted for determinism, else
the historical slug path — placed in
`src/fig3d.rs`, `src/editor.rs` (the
fig3 load call site only),
`src/figure_ops.rs` and
`src/rune_sec/pose_tools.rs`, with
pins. Save paths are unchanged, and no
element keys or catalogue keys moved.
W11's finding adds the panel-side
observation in the same class:
`library_panel.rs`'s `entry_file_name`
still returns the verbatim form, so the
panel's file-name search cannot match a
stored-name entry by its real slug file
name — W11's doc text now describes the
function honestly either way, and
whether the search should resolve
through `library_entry_file` is owned
by this audit's follow-up, not silently
decided by a docs slice.

**SpriteLoop reconciliation (W10).**
Audit only, against the RE record in
`~/workspace/spriteloop-re/`. Of the
GAP proposals' Tier-1+2 fourteen items:
**2 landed, 12 genuinely missing, 0
partial** under the strict sense.
Landed: #4 JPEG import (Wave R; one
recorded deviation — it stores the
original bytes and consumers decode by
content, no transcode-to-PNG, with one
residual: Pivot export embeds the JPEG
blob raw with unit anchors,
remainder (9)) and #5 export scale
(Wave R + S+T; the M-form re-render;
no panel control, which remains the
user's standing decision). Missing,
each verified at its definition site:
#1 animation states / StateKey
(FigureInstance still carries a single
`clip: Option<usize>`), #2 segment
media bend-follow (media still rides
the straight chord; text-on-chord is
deliberate and stays), #3 the sprite
small wins 0/4 (`sprite_at_point` still
uncalled; no `sprite_set_asset`,
z-reorder, or dopesheet sprite lane),
#6 joint limits (no min/max angle;
`ik_end` still hard-coded off on Stick
Nodes import), #7 transparent-alpha
sequence export (the funnel is
RGB-only), #8 sprite key enrichment
(InstanceKey still {frame, x, y, scale,
angle}; nodes_anim still discards
flip/scaleY/pivot), #9 editable setup
pose, #10 the bend graph channel and
4-piece arc (ChannelTarget still four
variants; the arc still two-piece),
#11 segment-media local transform, #12
layered-set import (no
`layered_import.rs`; "spriteloop"
appears nowhere in src/), #13 the
named-state panel (blocked on #1), #14
strip-warped media (blocked on #2).
Proposals P-1…P-11 plus P-4r (the Pivot
transcode rider) are filed with a
suggested order — P-1/P-2 first, then
P-3/P-4, then P-5…P-9, P-10/P-11 later.
The RE's rejected set stays rejected:
cutout rigs, full mesh, whole-figure
pose transform, and the skew channel
(the last needs the user's explicit OK
and has never had it).

**Doc-text corrections (W11).**
Comments and docs only, proven
mechanically: every added/removed line
in the three files is a doc line. (1)
`src/subtitles.rs` (V18's R3): the
module doc's "no renderer draws it yet"
speaker note was false at base —
`subtitle_display_lines` composes the
`Name: ` prefix and every surface draws
through it (stage, raster burn-in, SVG
writer, the timeline lane, the panel's
cue rows; the census's widening leg
measures the credited line on the real
stage). The bullet now says the
renderers DO draw it, identically
everywhere, that the prefix is display
composition only, and that on disk only
WebVTT represents the speaker itself.
(2) `src/rune_sec/help_panel.rs` (V18's
R2), with a judgement call stated for
the integrator: the three "54 rows"
sites are dated Phase M/N census
records and 54 was TRUE then — the
doc's counts form an arithmetic chain
(54 → 56 → 57 → 58 → 61) that later
paragraphs and the pin stand on, so
rewriting them would falsify the record
and break the chain (V5 likewise left
them). The correction is a closing
Wave W paragraph in the file's own
accretive convention: the panel now
holds 61 rows (10 Shortcuts + 8
Workspaces + 2 Posing + 5 Timeline +
36 Features), pinned by
`shortcut_rows_has_exactly_61_rows`,
and the 54-row paragraphs are named as
Phase M/N's figure. (3)
`src/rune_sec/library_panel.rs` (V1's
flag): the present-tense contract
statements were corrected in place to
the V1 rule (entries named from the
stored name, stem fallback, slug
unchanged, resolution via
`library_entry_file`), with the stale
line cites removed; the dated records
were examined and deliberately left,
each with its reason in the finding.
Element keys per file identical before
and after; the gate passed on the
mid-wave tree.

**PHASE_O citation upgraded (W12).**
PHASE_O §10's designated citation run
could not be the one the brief named:
42771069640 is the failed round-1
headless above, so under the brief's
own fallback the table now cites the
newest completed successful run under
the full 16/16 contract,
**42767540386** (`fbc3644`) — downloaded
first-hand for this pass: 221 files, 37
dumps, overflow **CLEAN (0 hits, scan
set complete)**, census **PASS 19/0**,
both Home markers **PASS**; all 36 of
V19's rows reproduce exactly, and one
row is added
(`tree-home-scan-fallback.json`,
1440×900, 333 elements, required,
CLEAN). O-1…O-9's measurements are
unchanged — the third consecutive
dispatch to produce them. §10 records
the 42771069640 failure in the table's
own text, so no one cites that run for
anything, and states the upgrade rule:
the citation moves to the first
successful Wave V-or-later dispatch,
with the pair-gate verdict read
alongside. (Round 2's 42771786953,
above, is that dispatch for the
overflow/census/marker figures; its
pair-gate verdict is UNPROVEN, so the
citation's thumbnail line stays exactly
as remainder (2) states until the
redesigned gate produces a verdict.)
The cited run's legacy thumbnail probe
still trips (0.672 vs 0.06) — that was
W7's slice, answered above. QA_RUNE
§17's alignment was left to W18.

**Gate/census maintenance (W13).**
Verification, not a manufactured
change: both owned scripts are
byte-identical to HEAD at final check.
On a clean detached worktree at the
base, W13 verified every post-Wave-V
truth independently — gate PASS 10/10;
element keys 983 (864 static + 119
dynamic) with `KEY_TOTAL_FLOOR` = 983
standing EXACTLY at the verified count,
so no ratchet is owed; MCP census 381
with `MCP_SURFACE_FLOOR` = 381 likewise
exact; the catalogue counted by an
independent parse at 662 × 28 by the
line convention (663 structural, the
`status_ready` tuple, disclosure kept),
identical ordered sequences, zero
duplicates, zero value-equals-key in
English; scenes 34 covered by both
headless loops in both directions;
palette 134; State 48/22. The live-tree
recount mid-wave also passed 10/10 with
no delta on any counted surface. Its
pending-IA arithmetic was written
before W1/W6 filed and is superseded by
the integrator-actions paragraph below;
its floor discipline stands as the
wave's method: verify on the settled
tree first, then set each floor to the
verified count — never before.

**Overflow/headless maintenance
(W14).** The audit found no drift, so
the pass files the standing note, not a
manufactured edit: both headless tree
loops parse to the registry's 34 scene
names, set-equal in both directions
(V14's fallback-mirror repair holds);
the produced tree set is 37 (34 loop
dumps + 3 size-variant literals);
SCAN_SET stands at 16 required / 0
pending, verified by importing the
module — no Wave W slice registers a
scene, and promoting without a
producing scene would make every
debrief read INCOMPLETE. The one change
is a +25-line docstring maintenance
note in `scripts/overflow_scan.py`
recording those counts and the W7
boundary (W7 owns the
snap-home-missing-audio capture block
and the thumbnail-probe step; its
region produces no tree dump, so it
owes no SCAN_SET entry — the V7/V14
carve-out, restated). `headless.yml`
itself is byte-unchanged. At W14's
check no IA-W1/IA-W6 workflow text
existed; W1 as filed needs none (its
wiring is in-panel), and W6 as filed
needs none (its IA is catalogue
values) — the counts in the note
(16/34/37) are the baseline any later
workflow IA is checked against.
Self-tests: overflow and subtitle
census both PASS; gate PASS on the
mid-wave tree.

**MCP docs sync (W15).** No edit was
made to docs/MCP.md, and the finding
proves there was none to make at its
check: the census verifies 381 with the
docs' own count prose agreeing; the
drift sweep (V15's method, reproduced
exactly) finds name sets identical,
296 rows character-identical and 85
differing — and the identical sweep
over a pristine `git archive 1c7a70d`
tree yields the byte-identical
differing set, while `git diff
1c7a70d..d5da9fd` over docs/MCP.md and
every MCP source file is empty, so all
85 are pre-existing condensed-form
rows: **zero new drift**. The docs
contain zero occurrences of "progress"
— correctly, while the MCP surface was
unwired at that base. W15's final check
predates W3's filing, so IA-W3-1's two
sentences were not in its sweep; they
are the integrator's to apply verbatim
(below), with the `render` /
`export_file` rows re-checked
character-for-character against the new
source text, per the standing U11/V12
remainder. W4 likewise owes the docs
nothing (no default-on claim about the
live server exists in them).

**README truth pass 4 (W16).** Three
hunks in README.md only. Name fidelity
is now CLAIMED — V1 landed, so the
bullet states the exact stored-name
rule including the migration (stem
fallback for legacy files, slug
unchanged, nothing rewritten at load)
where pass 3 flagged it unclaimed. The
subtitle bullet now states the panel
style controls as they are — position,
font scale, and text/box colour via
palette swatches or the RGB channel
steppers for any colour — and claims no
background/opacity control, because
none exists. The progress paragraph now
covers BOTH hooks with their phases
kept apart: render (per frame rendered
and written; its terminal closes the
whole export) and encode (the encoder's
own counts; "finished" closes only the
video encode, with the MP4 audio mux
after it) — and keeps the sentence that
neither the export panel nor the MCP
server surfaces them, true at the
pass's base. Counts re-verified against
the tree before editing (MCP 381 census
PASS, catalogue 662, scenes 34). Range
audio is NOT claimed: w6.md had not
landed at W16's final check, so the
README still calls range MP4/WebM
"(video-only)" — flag F6, remainder (5)
below; the LICENSE absence stays flag
F2, not an edit; and the
progress-surface wording is flagged
(F7) to be revisited now that W1/W3
have landed.

**QA_RUNE alignment (W18).** One file
edited: docs/QA_RUNE.md gains a §19
(Wave W alignment) in the doc's
audit-trail style, §18's text untouched
with named cells superseded. Counts
re-verified at `d5da9fd` from a pristine
archive snapshot (MCP 381; keys 983,
floor 983; catalogue 662 line / 663
structural; State 48/22; palette 134;
icons 163; scenes 34, facts 1..=36;
SCAN_SET 16/0; its snapshot gate reads
9/10 only because an archive has no git
ref for check 2 — the in-repo run on
the same tree is 10/10). **R1 is
CLOSED**: the line convention stands
with the disclosure kept, structural
663 named wherever the count is
load-bearing; no renumbering. **R5
stays OPEN, deliberately**: V1's fix
verifies in tree at `d5da9fd` and in
`2462a82`, but §18.5's own closing rule
requires the build carrying V1 to land
green, and at W18's final check round 2
was still in flight. That condition is
now met — round 2 (build 42771751700 +
headless 42771786953, above) is green
on the commit carrying V1 — so the
integrator (or the next docs pass)
closes R5 citing those runs; this
section records the evidence, QA_RUNE's
row is theirs to flip. §19 also aligns
§17's PHASE_O citation with W12's
re-sourcing (42765217055 standing, the
original citation's census-red-inside-
green disclosure kept), resolves
§18.3's open cell (Wave U round 2
green, transcripts verified), and its
remainders table re-owns R2/R3 to W11
(done, above), R4 to W1/W3 (done,
above) and R6 — the legacy probe trip —
to W7 (done, above; its pair-gate half
is remainder (2)).

**Library migration e2e (W19).** A new
suite, `tests/wave_w_figlib_e2e.rs`,
drives V1's migration contract end to
end against a hand-planted legacy
library mixing all three file shapes —
a bare pre-V1 def (stem fallback), a
def with stored `"name": "Styk"`, and a
bare def hand-placed under a verbatim
`Foo Bar.json` name — with fixture
honesty asserted in-test (the bare
text's top level carries no `"name"`
key; the stored-name file still parses
as a bare FigureDef). Listing is exact
and no entry is named "styk"; each
entry inserts by exact name through the
registry and the lossy "styk" refuses;
a fresh save lands in its slug file
with the exact stored name.
`library_entry_file`'s resolution order
is witnessed per shape (slug fallback,
verbatim wins, plain stem); both
rename collisions refuse with their
exact sentences (display collision —
only possible post-V1 — and file
collision) leaving names, files, stored
name and undo depth byte-identical; the
same-name settle answers `renamed:
false` (pre-V1 it hunted a nonexistent
file and failed); a real rename moves
the file and syncs the stored name; a
fresh reload shows no resurrection of
any earlier name. The delete surface
carries its caveat in the test's own
module doc: there is NO
`figure_library_delete` tool, and the
panel's `delete_saved` lives in the
binary integration tests cannot link —
so the delete legs pin the resolution
delete delegates to (`library_entry_file`,
whose panel wrapper is one delegation),
perform the removal it exists to aim,
and assert what a fresh load sees; the
panel fn's own refusals are pinned
in-crate. If a future slice moves
delete into the lib, these legs should
be re-pointed at it.

**Integration-eve rehearsal (W20).**
No source edited. Every instrument run
twice — on the clean base tree and on
the write-time tree (HEAD advanced to
`2462a82` mid-wave; 15/20 findings
filed at its snapshot) — and all PASS
on both: gate 10/10, census 381,
overflow and subtitle-census
self-tests, thumbnail-probe self-test.
The arithmetic ledger: element keys
983 on both snapshots (floor met
exactly, zero lost vs the wave base
across all changed src files); the
catalogue 662 × 28 in-tree, unchanged
(the frozen files byte-identical);
MCP 381 with docs in sync; scenes 34;
SCAN_SET 16/0; palette 134. One
measurement pair is recorded for the
integrator's eye: W1's file-local census
names new `xc-progress*` keys in its
readout block while the gate extractor
reads export_panel.rs at 28 = 28 (the
new keys are stem-constructed forms
inside already-counted patterns) — the
gate total is the ratcheted instrument
and it reads no net change. W20's
pending-IA itemisation predates W6's
filing; the complete set is the
paragraph below. Its honest caveat
stands: at its snapshot W6/W7 had not
landed, so nothing about them was
claimed either way; both have since
filed, and the integration gate run
after they land is the settling
measurement, as always.

**Integrator actions filed this
wave.** IA-W1-1: insert 4 catalogue
keys × 28 (`export_progress_working`,
`export_render_progress`,
`export_encode_progress`,
`export_encode_finished`) — projected
catalogue **662 + 4 = 666 × 28** (667
structural) once landed; W1's pin
passes only after insertion (Q7).
IA-W6-1: update the VALUES of 3
existing keys × 28
(`export_range_audio_note`,
`export_note_webm`, `export_loop_note`)
— no count change; W6's three panel pin
literals pass only after it lands (Q7).
IA-W3-1: append W3's two progress
sentences verbatim to the `render` and
`export_file` rows of docs/MCP.md (via
W15's method; +0 keys). IA-W4-1:
replace the `main.rs` launch-comment
parenthetical with the defaults-to-off
wording (+0 keys). IA-W5-1: correct the
two stale doc sentences in
`audio_panel.rs` (+0 keys). Plus W6's
flagged staleness: the `export_file`
description sentence in `src/mcp_io.rs`
claiming webm_range never carries audio
is now false, and its neighbouring
`missing_audio` comment with it — the
integrator corrects the source text and
the docs/MCP.md row together (the
`render` tool's "'webm' never carries
audio" sentence stays true: that format
is the full entry). Compile-blocking:
IA-W1-1 and IA-W6-1 are both Q7-pattern
pin gates, not compile errors; no IA
this wave blocks compilation. No
actions were filed by W2, W7, W8, W9,
W10, W11, W12, W13, W14, W17, W18, W19
or W20.

**Verification — awaiting CI.** Wave
W's counts are W13's and W20's,
verified on the base and on the
mid-wave tree, not taken from siblings'
claims: gate PASS 10/10, census 381
(44 + 319 + 9 + 4 + 5), element keys
**983** with the floor already standing
at the verified count, catalogue
**662 × 28** in tree becoming a
projected **666 × 28** when IA-W1-1's
four keys land (IA-W6-1 moves values
only), palette 134, icons 163, scenes
34, SCAN_SET 16 required / 0 pending,
State 48/22. Wave V's frame is closed
above (round 2 green, transcripts
read); Wave W's own proof is its
integration push's Build, Test &
Binary plus a headless dispatch,
transcripts read — including the first
execution of W1's job pins, W2/W6's
export pins, W3's transport pins,
W4's default pin, W8/W19's suites, and
the re-derived thumbnail probe against
the integration snaps.

**Honest remainders.** (1) The standing
user decisions stand exactly as §6.16
left them — white-on-accent contrast
(3.14:1), the panel splitter, File-menu
Save As, the two disagreeing "New
project" results, `tab_visits_controls`
OFF, project-swap undo, the skew
channel, the whole-figure pose
transform, MCP `write_recovery`, the
waveform badge/call, S1's seven i18n
value-vs-label discrepancies (RESERVED,
neither side changed), the
subtitle-stack cap/wrap, a panel
export-scale control (core/MCP only by
design) — and this wave touched none of
them. Sound remains user-verified only.
(2) **The Home thumbnail pair gate
needs redesign, not loosening.** Its
first real verdict is UNPROVEN (round
2, above: the legacy control blocked
strip extraction), and W7's
measurements on the predecessor's snaps
show its identical-strip premise is
refuted by the app itself: the current
project's row is painted in the
selected fill (≈17,452 px of surround
difference), and stroke-AA variance
between two renders of identical
frame-0 content (566 px in-band) exceeds
the different-content signal (330 px).
A follow-up slice must derive a
fidelity signature the thumbnails
actually carry — per-thumb statistics,
or the added figure's colour presence —
with floors from measured signals.
Until a redesigned gate produces a
transcript verdict, Home thumbnails
are unit-proven and snap-captured but
not snap-proven; the LEGACY leg, by
contrast, is now proven: W7's
re-derived probe PASSes on both real
artifact sets, and its first
integration transcript is owed by the
Wave W dispatch. (3) Live-MCP's stored
`true` cannot be migrated (W4): a
prefs file written by an old build may
still carry a cemented default-on that
the new build honours; the user-facing
remedy is the one Settings toggle that
writes `false`. The v1 posture —
validation, not authentication — stands
as documented design, not as a claim
that loopback control is authenticated.
(4) WebM range exports cannot name
their missing clips (W6): no outcome
type exists on that signature, and
adding one touches mcp_io.rs and the
panel — a separate API slice if the
user wants MP4's report parity.
Unprobeable clips already refuse on
both formats; missing files are the
only silent half, and only on WebM
range. (5) Two wording debts ride the
class-C exclusion and W16's flags: the
range WebM status/recap sentences still
say "video only" (class-C, machine-read
— product ruling only, exactly like
the Settings-card Esc and figure-
inspector items PHASE_O carries), and
the README still calls range exports
"(video-only)" (W16's F6) and still
says no surface reports progress (F7) —
both README sentences become false the
moment W6/W1/W3 integrate and CI
proves them, and the next README pass
owes both corrections with the
integration runs as their evidence.
(6) Bundle progress is unwired by
design (W3): a tokened bundle call
sends nothing rather than something
misleading; honest bundle progress
needs a core-level bundle progress type
(one total across members, allocated
before the first member runs) — a
future boarded slice, not a transport
patch. The live bridge likewise sends
no progress, by transport construction,
not by omission. (7) W10's SpriteLoop
proposals P-1…P-11 + P-4r are filed,
unbuilt and unrostered: 12 of the 14
audited GAP items are genuinely missing
from the app, and closing them is a
build wave of its own (suggested order
in w10.md), not integration fallout.
The two landed items carry their
residuals openly. (8) The panel's
library file-name search cannot match a
stored-name entry by its slug file name
(W9/W11): docs now describe
`entry_file_name` honestly; whether the
search resolves through
`library_entry_file` is a rostered
decision, not yet taken. (9) JPEG
import's residual (W10, item #4): Pivot
export embeds the imported JPEG blob
raw with unit anchors — P-4r is the
filed rider (transcode on that export
path), unbuilt. (10) `clip_caption`'s
"muted" remains English-canonical
(W5's observation): the catalogue's
muted keys exist; wiring it is a future
slice's, recorded so it reads as a
decision. (11) QA_RUNE's R5 row flips
on evidence now in hand (W18 + the
frame above): round 2 is green on
`2462a82`, which carries V1's fix — the
integrator closes the row citing
42771751700 / 42771786953 when the Wave
W slices integrate; until then the row
stands open with its evidence attached,
per §18.5's rule. (12) §6.16 remainders
closed here: the progress surfaces
(remainder (6) there) — panel (W1),
funnels (W2) and MCP transport (W3)
all built, core-only no longer; the
thumbnail probe's control premise
((3) there) — diagnosed and re-derived
by W7, the legacy leg proven on real
artifacts; the doc-prose staleness
((5) there) — W11 closed R2/R3 in the
docs' own conventions; and PHASE_O's
citation ((W12)) upgraded under its
own fallback rule. And the count
artefact (§6.16 (4)) stands settled as
disclosed: 662 line-wise, 663
structural, convention kept (W18's R1
closure).

### 6.18 Wave X — the SpriteLoop
build wave: states, bend-follow,
joint limits, alpha, sprite keys;
the Wave W frame closed
(2026-11-10)

Ordered 2026-11-10 (the standing
continuous order, re-issued once
more and boarded ahead so this wave
launched the moment Wave W's
integration pushed): "assign 20
agents continuous write code if ci
gets error then fix that". Twenty
agents, one owner per file,
integrator surfaces via filed
actions (board:
`~/workspace/wave-x/BOARD.md`;
findings:
`~/workspace/wave-x/findings/`).
The board's base is the Wave W
integration HEAD `dccddbb` (666 ×
28, scenes 34, MCP 381, element
keys 984 by the gate's method);
`src/i18n.rs` and
`src/rune_sec/i18n_wire.rs` were
frozen to this wave's agents, all
key work arriving via IA. This
section was written from all
nineteen sibling findings (x1–x19;
x20 is this section) and from the
CI frame read via `gh` at the
final check, headless transcripts
read from the runs' downloaded
artifacts — never from workflow
conclusions alone, per the
42759195547 lesson. Every Wave X
status below is IMPLEMENTED (in
tree, uncommitted at write time);
Wave X has no CI verdict. The
Wave W frame, by contrast, is
closed here, exactly — and it does
not close green: two red build
rounds, for two different reasons,
both recorded in full below. HEAD
advanced one commit mid-wave
(`aa598ec`, the Wave W round-2
fix, test code only), so no
slice's base moved in substance.

**The Wave W integration ledger.**
Four commits on top of `2462a82`,
in order: (1) `08cd835` — export
progress end to end: W1's panel
background job (worker thread over
a cloned `Project`, `State` gains
`export_job`, polled in
`main.rs::poll` before the live-MCP
drain, the two phases in separate
atomic slots with separate readout
lines, finish wording byte-identical
to the synchronous actions'), W2's
funnel adoption (eight dual-hook
`*_with_progress` video entries;
every pre-existing entry delegates
unhooked, byte-identical), W3's MCP
progress transport (a per-call
ProgressSink composed only when
the client passed
`_meta.progressToken` AND the
transport installed a
NotificationSink, parked on
`EditorCore` for the handler's
duration, panic-safe; the stdio
shell restructured to one shared
writer — the old StdoutLock would
have deadlocked against drainer
notifications; `export_bundle`
deliberately unwired, its counters
would run backwards under one
token), and W6's range audio riding
the same slice's export.rs (WebM
range exports mux the range's
audio as Opus; MP4 range already
carried audio; full WebM stays
video-only). (2) `04361bb` —
live-MCP explicit-enable (W4: the
persisted default flipped ON→OFF
in the serde default and the
`Default` impl, a stored `true`
still honoured, no migration —
the file cannot distinguish a
deliberate choice from a cemented
default; bind and the Host/Origin
gate were already sound), the
library name paths (W9: V1's
backing-file trap found in all
three unaudited libraries — 2D
poses, 3D poses, 3D figures — and
closed with one scan-based
entry-file resolver per library;
save unchanged), the waveform
health caption (W5: composed
through the shared `health_segment`
path, no new keys), and W11's doc
corrections (the subtitles.rs
speaker-prefix doc, the
help_panel 54-vs-61 census note,
the library_panel stem-naming
provenance; dated records
preserved verbatim). (3) `bb511d3`
— the F6 integrator pass: i18n
catalogue 662 → **666 × 28**
(IA-W1-1's four progress keys
inserted in all 28 tables, genuine
translations in 8 languages, slots
preserved; IA-W6-1's three value
updates; IA-W4-2's Live MCP help
row corrected at all four mirror
sites; the i18n_wire header count
updated). (4) `dccddbb` — probes,
docs, records: W7's re-derived
thumbnail probe, W12's PHASE_O §2
citation upgrade to run
42767540386, W14's overflow
maintenance note (16/34/37
re-verified, headless.yml
byte-unchanged), W15/F6's MCP.md
rows for W3's progress sentences
(census holds 381), W16's README
pass 4 (name fidelity and the
subtitle RGB steppers claimed;
range audio flagged pending W6),
§6.17 itself (W17) and QA_RUNE
§19 (W18: R1 closed; R5 left open
pending the Wave V build verdict).
The integrator's instruments on
the committed tree before the
push: gate **PASS 10/10** (element
keys **984** by the gate's method
= 865 static + 119 dynamic, floor
then standing at 983; State 49/22
— W1's `export_job` field landed;
palette 134; MCP 381 = 44+319+9+4+5;
icons 163; scenes 34; layout facts
1..=36; docs/MCP.md in sync),
catalogue 666 × 28 by the line
convention (667 structural, the
`status_ready` tuple, disclosure
kept), SCAN_SET 16 required / 0
pending. A fifth commit, the CI
fix `aa598ec`, belongs to the
frame below.

**The Wave W CI frame — two
rounds, read at the transcript
level, and not green.** Round 1
(on `dccddbb`): build 42774188570
**FAILED** at its Test step — the
lib test target only: **8 × error
E0425** in `src/export.rs`
(4105/4116, 4127/4137, 4149/4160,
4197/4207), where W6's four WebM
range pins call `temp_tone_wav(…)`
and `webm_has_opus(…)` — two
helpers that were referenced but
never defined anywhere in the
tree (the module's own fixture
writer is `temp_wav`, silent). The
Build step passed, every other
target compiled, the product code
was never implicated — and no lib
tests ran. Headless 42774207670
concluded **GREEN**, and its
transcripts, read from the
downloaded artifact (224 files),
are: **SUBTITLE CENSUS: PASS**
(19 PASS / 0 FAIL). **HOME
MISSING-AUDIO MARKER: PASS** and
**HOME SCAN-FALLBACK MARKER:
PASS**. The overflow scan over
the run's artifacts, run locally
on the download: **CLEAN (0 hits,
scan set complete)** across the
16 required trees (37 dumps). The
**Home thumbnail probe reads
UNPROVEN** — but its shape
changed decisively: the LEGACY
leg **PASSes in a CI transcript
for the first time** (control
home-totals stage-bg share 0.008
on the populated snap, 0.004 on
the missing-audio snap, against
the 0.06 ceiling that read 0.672 /
0.681 before W7's re-derivation;
rows 0.333 / 0.337 / 0.339 against
the 0.18 floor, ink 115 / 115 /
59 against the floor of 15 —
W7's locally measured numbers
reproduced digit for digit). The
pair gate then executed for the
first time and stopped at its
strip-geometry precondition —
"the four strips do not share one
pixel size: [(112, 238), (112,
239)]" — so its identity legs
issued no verdict: **not the FAIL
W7's older-artifact analysis
predicted**, and not a PASS; the
redesign it needs is X10's slice
below. And the **MCP PROBE reads
FAIL (1 assertion)** inside the
green conclusion: `GET /health`
never answered within 90 s, the
app log is empty, the bridge
simply never started. That is
W4's intended behaviour meeting a
probe whose clean-slate setup
assumed the old one — the step's
own comment states its premise
("the live-MCP pref defaults ON …
wiping ./projects … is what makes
the endpoint's presence a property
of the binary"). On a wiped
profile the flipped default means
no bridge, by design. Classified
here as **probe debt, not an app
regression** (remainder (3)); any
other reading would re-open the
security gap W4 closed. The fix
is `aa598ec` — "define the two
range_mux_tests helpers W6's pins
call": a real 440 Hz tone WAV
writer and the `A_OPUS` CodecID
scan, defined in `range_mux_tests`
beside `temp_wav`; no production
code touched. Round 2 (on
`aa598ec`): headless 42775545085
concluded GREEN with transcripts
**identical in every particular**
(the fix touches no binary code)
— census PASS, both markers PASS,
overflow CLEAN, thumbnail
UNPROVEN on the same precondition,
MCP probe FAIL on the same single
assertion. Build 42775511096
**FAILED** at its Test step,
differently: the lib target now
compiled and ran — **1,633
passed, 1 failed** — the failure
`mcp::tests::tokened_video_range_
streams_encode_progress`
(src/mcp.rs:3016): the tokened
MP4-range call emitted TWO
progress notifications, both
`(progress 2, total 2)`. The
2024-11-05 wire shape carries no
`finished` flag, so the encode
drainer's unfinished event at the
terminal count and the terminal
event itself arrive as the same
number twice, and the pin enforces
U11's strictly-increasing rule on
the wire, where the client sees
it. (At the core level the two
events differ by their flag; the
sequence sibling pin and the
both-gates pin passed.) The
remaining test targets did not
run. **Round 3 is owed**: coalesce
the duplicate in the transport
translation or in the drainer's
emission — the pin stands exactly
as written. Net Wave W frame: the
product code compiled in both
rounds; the headless frame is
green at the transcript level
except the pair gate (UNPROVEN,
redesigned below) and the MCP
probe (probe debt, above); but no
Build, Test & Binary run has
landed green on a commit carrying
Wave W, so **Wave W is not yet
VERIFIED** — round 1 was a
test-authoring slip in W6's pins,
round 2 is a real transport
contract violation that W3's own
pin caught. Both are recorded
here rather than smoothed over,
and Wave X's integration inherits
the same pin.

**Animation states, core (X1).**
GAP item #1's foundation lands in
model.rs only: `StateKey { frame,
clip, offset, speed }` (offset and
speed serde-defaulted, so a key
written as `{frame, clip}` is a
plain switch) and
`FigureInstance::states: Vec<
StateKey>` with `#[serde(default)]`
— every pre-Wave-X project loads
with an empty vec and behaves
byte-identically, the legacy path
being the old code in effect.
Resolution: the key in force is
the greatest key frame not past
`f` (a key takes over AT its
frame; ties go to the later vec
entry); local time is
`(f − key.frame) · speed + offset`
with corrupt speeds sanitised at
read (non-finite or non-positive
plays at 1.0, finite capped at
1e6); a dangling state clip skips
the figure exactly as a dangling
legacy `clip` does. Loop modes
stay a clip property; bends are
unaffected. Ten pins in a new
`state_key_tests` module, incl.
the headline (idle keyed at 0,
run keyed at 3, one figure playing
both across one timeline) and the
tween-follows-the-state-clock leg.
**IA-X1-1 is compile-blocking**:
the new field makes every
exhaustive `FigureInstance`
literal in the tree a hard error —
25 sites in 16 files each need
one `states: Vec::new(),` line
(the two inside model.rs are done;
the finisher's sweep is the W1
State-literal precedent).

**State ops bundle (X2).** GAP #1
Step 2, in editor.rs's clip-ops
region: `clip_compatible` (the
retarget guard the existing
copy-pose paths always had and
`apply_clips` never did — true
iff the clip's def matches the
figure's node count AND every
frame pose fits that count),
`assign_clip_to_figure` and
`unlink_figure` (the parameterised
link and its missing inverse;
refusals in the copy-pose voice,
no snapshot on refusal, the
already-linked case a silent
no-op), `set_state_key` /
`remove_state_key` (X1's authoring
remainder: upsert by frame, kept
sorted, NaN speed refused at
write), and
`create_clip_from_figure_range`
over the half-open `[start, end)`,
named "{fig} clip {start+1}-{end}"
so ranged clips are distinguishable
in the list X3's panel derives
from. Plus the coherence fix X1's
landing made necessary:
`delete_clip` / `move_clip`
remapped `fig.clip` but not
`fig.states` — keys would have
pointed at the wrong clips after
any delete or swap; both ops now
remap the keys in the same
snapshot. Six pins, one undo step
per mutation throughout.

**Named-state panel + tools
(X3).** GAP #13 in its no-new-field
form: clips_panel gains a states
section derived from
`FigureInstance::{clip, states}`
against the pool (per selected
figure: the base-link line, the
keys in timeline order with clear
buttons, and a click-to-key-at-
playhead strip over the pool,
the clip in force marked), all
writes routing through X2's ops —
one authoring path, rig guard
included — and `usage_text` /
`clip_list`'s `used_by` now count
state-key usage (the base-link-only
filter called state-keyed clips
unused). Seven registry tools
(`figure_list_states`,
`figure_set_state`,
`figure_clear_state`,
`figure_assign_clip`,
`figure_unlink_clip`,
`clip_compatible`,
`clip_create_range`), every
clip-addressing one accepting the
clip by index OR by name through
one shared resolver — **the census
moves 381 → 388 in tree** (registry
319 → 326). Element keys: +1 static
+ 3 dynamic stems, zero lost.
**IA-X3-1** files eight catalogue
keys for the panel's labels (the
panel's i18n pin passes once they
land, Q7 pattern).

**Bend-follow for image media
(X4).** GAP #2, and the wave's
second deliberate contract
reversal, on the record: segment
images now follow the bend arc
exactly as segment lines always
have. `MediaItem::Image` gains
`src_rect` (full = `[0,0,1,1]`);
`media_items` fetches `bends_at`
once per figure and, for a bent
node, emits two half-quads split
at `bend_mid`, each sampling its
half of the source; `composite_
image` gains the subrect mapping
(byte-identical at the full rect,
so unbent projects produce
identical item streams and
identical composites); SVG emits
the split as two clipped `<use>`s
under a new `mclip{n}` family
(the bare emission stands for
full-rect items). Text is
untouched — it stays on the chord
as one rigid label, its pin
standing; no test had ever pinned
image-on-chord, and the new pins
state the reversal in their doc
comments. The stage placeholder
needed no code: it draws the same
funnel's items, so bent images
draw two placeholders sharing the
bend edge by construction. Four
pins, incl. the two-tone
compositing proof (each half
samples its own source half; a
pixel below the old chord paints,
a pixel inside it no longer
does).

**Sprite small wins (X5).** GAP
#3, all four: `sprite_at_point`
— tested since Wave 13 and never
called — is wired into the stage
pointer path (`sprite_pick_at` in
stage_sprites.rs; main.rs's
stage_drag miss branch picks
joint → sprite → stroke, the
visual stacking order, and the
existing selection overlay lights
with no new drawing code); two
new ops, `set_sprite_asset`
(re-point an instance at another
asset keeping every key verbatim)
and `reorder_sprite` (z-order,
draw order being Vec order), each
one undo step with the family's
validated-no-op discipline; their
MCP handlers + TOOL ENTRY SPECs
in mcp_sprite_tools.rs (the P38
route — the registry splice is
one integrator action, **+2 tools
→ 390 projected**); and a sprite
lane in the dopesheet rendering
on its own condition so the
figure section's early returns
can no longer hide it. The
panel's new labels follow
sprites_panel.rs's literal-English
convention (the file was never
i18n-wired) — no catalogue IA
owed, remainder (9).

**Joint limits (X6).** GAP #6's
main half: `NodeDef` gains
`min_angle` / `max_angle:
Option<f32>`, both
`#[serde(default)]` (None =
unlimited = today's behaviour),
with `clamp_angle` semantics
stated exactly (both-None returns
the angle UNCHANGED, not even
normalised, so unlimited joints
are bit-identical everywhere;
min > max is the wrap arc, the
forbidden value taking the nearer
endpoint; a single bound clamps
one side; min == max pins the
joint). Enforcement at the three
funnels: `aim_node` (the solved
angle clamped before the write),
`apply_ik` (mid joint clamped
first and its position recomputed
so chain lengths are never
violated, then the end joint),
and graph Angle channels
(unlimited nodes keep the raw
channel value — the standing
no-re-wrap contract). Stored
poses are never rewritten: limits
bind solving, not data. The
humanoid preset gains anatomical
limits (elbows −10°/+160°, knees
−160°/+10°, signs derived from
the code's own IK geometry, the
rest pose inside its own limits);
quadruped/simple get none — their
flexion signs were not derived,
and a guessed preset is a fake
one. The segment inspector gains
the limit steppers + Clear
(`seg-lim-*` keys), one snapshot
per effective press. The 35-site
NodeDef literal sweep was
compile-required plumbing, done
in-slice. **IA-X6-1** files four
catalogue keys (`segment_limit_
min/max/off/clear`). No MCP tool:
parity is in the enforcement —
both surfaces funnel into the
same solvers.

**Import/export riders (X7).**
(a) The P-4 rider: Stick Nodes
import stops hard-coding
`ik_end: false`. The grammar
carries no such field (W10
verified), so `convert()` now
DERIVES it — a node no other node
parents to (a chain leaf, origin
excluded) imports as an end
effector, identically on every
import path through the funnel.
Heads and tails are leaves too
and derive ends as well; the
grammar cannot distinguish them,
the consequence is stated in the
finding and asserted in the
quadruped pin rather than left
implicit. Two pins that asserted
the old drop are reversed BY
CHOICE, on the record, and the
export loss list is corrected to
the residual truth (only a
non-leaf node's stored flag is
lost). (b) P-4r, W10's item-#4
residual: Pivot v5 export
transcodes non-PNG assets —
`piv_image_payload` passes PNG
blobs through verbatim with their
IHDR size (the existing verbatim
pin stands), decodes any other
blob by content and re-encodes
it as PNG with the REAL anchors
from the decoded size, and
refuses undecodable bytes by
name before anything is written.
`export_piv_v5` therefore returns
`Result<Vec<u8>, String>` now —
**IA-X7-1** files the two call-site
adaptations (editor.rs's
`export_v5_file`, mcp_io.rs's
`piv5` arm) for the integrator.
Three pins, incl. the
encoder-made-JPEG transcode proof
(no verbatim JPEG in the payload;
the embedded image decodes as PNG
at the source's size).

**Alpha sequence export (X8).**
GAP #7 / P-5: an RGBA variant of
the one scaled render funnel.
The drawable collection is
extracted verbatim into
`collect_frame_art` (shared, so
the RGB pass is byte-identical
and every byte-identity pin
stands); the alpha pass composites
the same coverage through a
premultiplied accumulator starting
at (0,0,0,0) — no background
fill — un-premultiplied once at
the end, so supersampled edges
carry true coverage alpha with no
dark fringe. Media and subtitle
tails run at parity: an alpha
sequence carries sprites, stage
texts and burned-in subtitles
like the RGB one; the ONLY
difference is the background.
Eight `export_png_alpha_sequence*`
entries + four spritesheet
entries ride the scale entries
(scaled alpha comes free; an
empty alpha sheet is a 1×1
TRANSPARENT pixel, this family's
blank). **IA-X8-1** exposes it on
the MCP `render` tool as an
optional `alpha` argument (+0
tools). Panel exposure is NOT
proposed — a toggle there is a
product decision for the user,
flagged, remainder (5). APNG
still renders opaque (the named
follow-up); GIF untouched.

**Sprite key enrichment (X9).**
GAP #8 / P-6 closes four proven
.stknds round-trip losses:
`InstanceKey` gains `opacity`
(default 1.0), `flip_x` / `flip_y`
(false), `scale_y` (0.0 — the
follow-`scale` sentinel, resolved
in exactly one place,
`eff_scale_y()`), and `pivot_x` /
`pivot_y` (0.5), every one
serde-defaulted so pre-enrichment
files load with today's behaviour.
The importer populates them from
the sites that used to discard
them (the sprite branch's scaleY,
pivot and flips; the style block's
opacity, now returned by
`read_style`); evaluation builds
the quad from the pivot with each
axis negated under its flip — a
mirror AROUND THE PIVOT — and at
the defaults the corners are
bit-identical to the old
±half-extent form (X5's pointer
wiring inherits the enriched
geometry for free, hit-testing
the same quads); export writes
scaleY = `eff_scale_y()`, the
key's pivot and flips, and the
key's opacity. `set_sprite_key`
now PRESERVES the enrichment of
the key it replaces (panel
transform stepping must not wipe
an imported sprite's flips), and
a new `set_sprite_key_full`
writes all eleven fields in one
undo step behind the MCP
surface. The 69-site literal
sweep was done in-slice;
**IA-X9-1** files the registry
schema/description replacement,
**IA-X9-2** asks the finisher to
re-sweep literals at integration,
**IA-X9-3** files two exact
NODES-ANIM-SPEC replacements.
Panel enrichment controls are a
follow-up (remainder (6));
movieclip-instance flips remain
discarded — a movieclip becomes
a figure+clip, which has no
InstanceKey.

**Pair-gate redesign (X10).**
§6.17's remainder (2) and the
Wave W frame above, answered.
The old gate was refuted twice
over, both measured: its
same-pixel-size premise is false
on every real set (the middle
populated row sits at a half-point
y, its strip 112×239 against the
others' 112×238 — "the fixture
geometry moved" was the gate
misreading its own fixture), and
raw-pixel identity inverts
(identical pair 566 differing px
vs different pair 330 — a
half-pixel phase shift resamples
every stroke's anti-aliasing).
The redesign measures a different
signature: each thumbnail's INK
MASK (W7's re-derived predicate),
in coordinates anchored on the
thumbnail band (the bounding box
of the strip's exact stage-bg
pixels — the surround contains
none, the totals control measures
0.000 exact), compared by
ink-coverage distance (ink pixels
of A with no ink of B within
Chebyshev distance 1, symmetrised).
Measured on all four real
artifact sets on record: identical
content = **exactly 0** (incl.
the cross-process pair), different
content = **48–51**; the floor is
12 (~¼ of the weakest different
signal), and a distance of 1–11
matches NEITHER construction and
FAILs as ambiguous — the gate
refuses to guess. `DIFF_FLOOR` is
retired. Run locally against the
downloaded sets, the redesigned
probe **PASSes on real snaps —
the first PASS the pair gate has
ever issued** — with identical
transcripts on 42771786953 and
42774207670; two new self-test
fixtures pin the reason to exist
(a resampled pair the old gate
fails and the new one PASSes at
distance 0; a near-miss pair at
distance 4 FAILing as ambiguous).
Its first CI execution is the
Wave X integration headless
(remainder (4)). No IA, no
workflow change — the existing
step already runs pair mode.

**Bundle progress core type
(X11).** W3's named future work,
built: export.rs gains
`BundleProgressEvent { completed_
frames, total_frames, finished }`
— the total is the sum of every
member's frame total in its own
reporting sense (render members:
authored frames; encode members:
sampled times, via the new
`sampled_frame_total`; contract-less
members: the frames they will
write), allocated BEFORE the first
member runs — and a `BundleProgress`
tracker translating each member's
phase events against the frames
banked when its callback was
taken: delivered counts are never
retracted (a failed member banks
only its reported peak, so the
stream cannot run backwards even
across a failure), and `finish()`
fires the single terminal only if
every member succeeded. Tokened
`export_bundle` calls now stream
it; every other call is
byte-identical (the plain entries
ARE the hooked entries with
`None`). SVG/spritesheet members
send no events of their own —
they have no phase contract and
none was invented; their frames
land at completion, so the stream
holds during them and never
overstates. The panel's bundles
stay synchronous (the core type
exists; panel wiring is a panel
slice). **IA-X11-1 is
compile-blocking**: a third
report tag, `ProgressPhase::
Bundle` + `ProgressReport::bundle`
in mcp.rs (tagging a bundle count
Encode or Render would corrupt
W3's never-merge discipline in
the type, even though this
protocol revision never puts the
phase on the wire). IA-X11-2's
docs sentence was applied by X18.

**WebM range outcome widening
(X12).** §6.17's remainder (4)
closed: all six WebM range
signatures return
`Result<Mp4ExportOutcome>` — the
MP4 range type, reused rather
than paralleled (its doc now says
so) — carrying `missing_audio`
out of the plan both bodies
already computed and discarded.
The registry's `webm_range` reply
flows it into the same
`missing_audio` field the
`mp4_range` branch fills (absent
when empty, so an all-present
export's reply is byte-identical
to before). In the panel, the
outcome is carried through the
job's result type but the finish
still speaks the byte-identical
class-C sentence — the standing
exclusion, stated plainly in the
finding: the user-visible
surfacing of this widening is
the MCP reply and the core API.
Five pins, incl. the panel finish
keeping its sentence WITH an
outcome naming a missing clip.
IA-X12-1 (the docs row) was
applied by X18.

**entry_matches search fix
(X13).** §6.17's remainder (8):
the library panel's file-name leg
matched the query against the
FABRICATED verbatim
`<display name>.json`, so wherever
display and slug diverge beyond
case the real file name could not
be searched ("Foo Bar" lives in
`foo-bar.json`; the leg searched
`Foo Bar.json`). The leg now
resolves through
`library_entry_file` — the same
display→file resolution delete
and rename use — factored as
`entry_matches_file` so both
directions are pinnable without
a library on disk. The verbatim
direction survives through the
resolver (a hand-placed entry
whose file IS its name still
matches by it); the one narrowing
is deliberate — the fabricated
form no longer matches when no
such file exists, since it names
no real file. Two new pins; the
pre-existing pins re-verified
against the new semantics by
reading. No IAs.

**Muted caption wiring (X14).**
§6.17's remainder (10) closed:
`clip_caption` / `waveform_
caption` take the core (the W5
precedent — the one production
caller's chain already threads
`&s.core` into every sibling
helper) and the muted segment
resolves through
`audio_muted_segment`, the exact
key and inline idiom the audio
panel's own readout uses; English
byte-identical, the unmuted
string untouched, no new key.
With this, the waveform strip has
no remaining English-canonical
caption surface: the health
caption (W5) and the muted
segment both resolve through the
catalogue.

**Citations + R5 closure (X15).**
QA_RUNE gains §19.7: **R5 is
CLOSED**, on §18.5's own rule —
the build carrying V1's fix
landed green (42771751700,
4,024 tests summed from the run's
own log, 0 failed; headless
42771786953 green on the same
commit; the fix verified standing
in the current tree). PHASE_O
§10's citation upgrades from
42767540386 to **42774207670**
(the Wave W integration headless
on `dccddbb`), re-verified by
W12's method first-hand: overflow
CLEAN (scan set complete), census
PASS 19/0, both markers PASS, 37
dumps; 36 of 37 per-tree counts
reproduce §10's table exactly and
one moves (`tree-audio-open.json`
408 → 462 — already 462 on the
predecessor's artifact, so the
move predates Wave W and is
stable); O-1…O-9 reproduce
exactly, the fourth consecutive
dispatch to do so. The pair
verdict is stated alongside, as
W12's rule requires: UNPROVEN,
its new shape recorded (legacy
legs PASS; the fidelity gate
stopped at its geometry
precondition).

**Gate/census maintenance
(X16).** Clean detached worktree
at `dccddbb`: gate PASS 10/10;
element keys **984** by the
gate's method (865 static + 119
dynamic) with zero lost;
`KEY_TOTAL_FLOOR` **ratcheted
983 → 984** with the ledger entry,
standing exactly at the verified
count; `MCP_SURFACE_FLOOR` 381 ==
verified, no ratchet owed;
catalogue 666 × 28 (667
structural) by independent parse.
The board's "expect 987" is
corrected on the record: it
assumed the gate counts all four
of W1's progress keys — it counts
one (`xc-progress`); the other
three ride `(key, line)` tuple
literals the extractor does not
capture. They exist, are pinned,
and render through the same
`.key()` call — a documented
artefact of the instrument, and
the floor is set to what the
counter verifies (remainder (7)).
Its pending-IA ledger was written
before x3/x10 filed; the complete
set is the integrator-actions
paragraph below.

**Overflow/headless maintenance
(X17).** The audit found no drift,
so the pass files the standing
note, not a manufactured edit:
both tree loops parse to the
registry's 34 names, set-equal in
both directions; the produced set
is 37; SCAN_SET 16 required / 0
pending, verified by importing
the module; `headless.yml`
byte-unchanged. The one change is
the +28-line docstring maintenance
note in `scripts/overflow_scan.py`
recording those counts and the
X10 boundary (X10 owns the
snap-home-missing-audio capture
block, the whole thumbnail-probe
step, and the probe's pair region;
its region produces no tree dump,
so it owes no SCAN_SET entry).
At X17's check no IA-X3/IA-X10
workflow text existed; X10 as
filed needs none. One thing its
audit did not flag is recorded
as remainder (3) below: the
live-MCP probe step's stale
premise.

**MCP docs sync (X18).** Run
against a moving census and
reconciled at filing: at base the
sweep reproduces W15 exactly (296
character-identical / 85 condensed,
the differing set byte-identical
to a pristine `d5da9fd` sweep —
zero new drift). During the slice
X3 registered its seven tools
in-tree (381 → 388) and two
source purposes moved; actions:
IA-X11-2 applied verbatim (the
`export_bundle` row MATCHES
source again); IA-X12-1 applied
in intent — its premise that the
`export_file` row is
source-verbatim did not hold (the
row was one of the 85 condensed),
so the row's Purpose cell was set
to the full current source
description, X12's clause included
verbatim, and the row now MATCHES;
X3's seven tools documented with
verbatim rows in registry order.
Final sweep: source 388,
catalogue 388, none missing or
extra; 304 identical / 84
condensed (the standing set minus
`export_file`). Filed for the
integrator: X5's two catalogue
rows verbatim (they ride the
registry splice), IA-X9-1's two
row updates, IA-X8-1's `render`
row sentence, and the count-prose
update — to be taken from a fresh
census at that pass (expected
390 / registry 328), not from
this arithmetic.

**README truth pass 5 (X19).**
Six hunks, README.md only. The
666 count replaces 662; range
audio is CLAIMED (pass 4's F6
resolved — both range formats
include the clips inside the
range, Opus in WebM, full WebM
video-only, an unprobeable clip
fails the export) and the Audio
bullet's overstatement corrected
with it; the "core-level only"
progress sentence is replaced
(F7 resolved) with both surfaces
and both deliberate non-wirings
(the bridge, the bundle) stated
as such; a new MCP-section
paragraph documents the live
bridge as it now is — loopback,
Host/Origin-validated, OFF by
default since W4, a stored choice
honoured, unauthenticated by
documented v1 design; and the
Figure building bullet gains
W9's name-path rule for the pose
and 3D libraries. Flags: F1, F3,
F4 carried; **F2 stands — no
LICENSE file exists in the repo
root** (the licence claim rests
on Cargo.toml's
`license = "GPL-3.0-or-later"`);
F8/F9 record X12's widening
(no README sentence needed) and
the class-C range-WebM status
wording lagging the file it
describes (a catalogue-side
decision, as W6 flagged it).

**Integrator actions filed this
wave.** Compile-blocking:
**IA-X1-1** (25 `FigureInstance`
literal sites in 16 files, one
`states: Vec::new(),` line each),
**IA-X11-1** (the `ProgressPhase::
Bundle` tag + `ProgressReport::
bundle` in src/mcp.rs — mcp_io's
bundle wiring already calls it),
**IA-X7-1** (`export_piv_v5`'s
new `Result` signature at its two
call sites). Catalogue:
**IA-X3-1** +8 keys (clips block)
and **IA-X6-1** +4 keys (segment
block) — projected catalogue
**666 + 12 = 678 × 28** (679
structural) once both land; the
owning panels' i18n pins pass
only after insertion (Q7).
Registry/docs: **X5's splice**
(two TOOL ENTRY SPECs into
`mcp_tools::tools()` + the
catalogue rows X18 filed verbatim)
— projected census **388 + 2 =
390**; **IA-X9-1** (sprite_set_key
schema/description replacement +
sprite_list's enumeration; +0
tools), **IA-X8-1** (the `render`
tool's optional `alpha` argument;
+0 tools) with X18's row updates
filed for both, and the MCP.md
count prose to be re-taken from a
fresh census at that pass.
**IA-X9-2** (finisher re-sweep of
InstanceKey literals — siblings
may have added fresh literals
against the old shape) and
**IA-X9-3** (two exact
NODES-ANIM-SPEC §17 replacements).
Already applied in-wave: IA-X11-2
and IA-X12-1 (by X18, above).
No actions were filed by X2, X4,
X10, X13, X14, X15, X16, X17,
X19 or X20.

**Rehearsal (X20) — every
instrument, two snapshots.**
Run on the near-settled tree and
again on the full Wave X tree at
the final check (HEAD `aa598ec`
plus all twenty slices'
uncommitted work): gate
`phase_j_checks.py` **PASS 10/10**
on both; `mcp_census.py` exit 0;
`overflow_scan.py --self-test`
all checks passed;
`subtitle_census.py --self-test`
PASS; `home_thumbnail_probe.py
--self-test` PASS (8 cases at the
final run — X10's two new fixtures
included). The arithmetic ledger
against the settled Wave W
counts: element keys **984** at
the settled base (X16's clean-
worktree verification and this
rehearsal's preliminary run
agree; floor ratcheted to 984 in
tree) and **995** on the final
tree (871 static + 124 dynamic) —
+11 counted additions across the
wave's panels, **zero lost**
(check 2); the catalogue **666 ×
28** in tree (independent parse:
28 tables × 667 structural keys,
identical ordered sequences, zero
duplicates; the four IA-W1-1 keys
present), **+12 pending** via
IA-X3-1/IA-X6-1 above; MCP **381**
settled, **388 in tree** (X3's
seven tools registered in-tree
and documented by X18 — gate
check 9's doc-sync passes against
388), **+2 pending** via X5's
splice; scenes **34**; SCAN_SET
**16 required / 0 pending**;
palette 134; icons 163; State
49/22. **Failure classification:
no instrument failure on the
final tree.** The wave's failures
live in the CI frame, and none is
pending-IA-shaped: round 1's
E0425 ×8 was a defect in the
wave's own test code (helpers
referenced, never defined — W6
filed no IA for them; fixed in
`aa598ec`); round 2's single
failing pin is a REAL product-code
contract violation in the MCP
transport (the duplicate (2,2)
notification — fix owed, round
3); the headless MCP probe's FAIL
is instrument debt from an
intended behaviour change (W4's
default flip), not a regression;
the thumbnail probe's UNPROVEN
was a gate precondition, since
redesigned in tree by X10. The
compile-blocking IAs above are
the integration's known first
work, in the wave's established
pattern — the tree as rehearsed
does not compile until IA-X1-1,
IA-X11-1 and IA-X7-1 land, which
is stated here so no one mistakes
the rehearsal's green instruments
(python-side) for a compile
verdict.

**Verification — awaiting CI.**
Wave X's counts are X16's
(baseline) and this rehearsal's
(final tree), not taken from
siblings' claims: gate PASS
10/10, census 388 in tree (44 +
326 + 9 + 4 + 5) becoming a
projected 390 at the splice,
element keys 995 by the gate's
method with the floor standing at
984, catalogue 666 × 28 in tree
becoming a projected 678 × 28
when IA-X3-1 + IA-X6-1 land,
palette 134, icons 163, scenes
34, SCAN_SET 16 required / 0
pending, State 49/22. Wave W's
frame is closed above (two red
build rounds, headless transcripts
read, round 3 owed); Wave X's own
proof is its integration push's
Build, Test & Binary plus a
headless dispatch, transcripts
read — including the first
execution of the state-key,
state-ops, bend-follow, sprite,
joint-limit, rider, alpha,
bundle-progress and outcome pins,
and the redesigned pair gate's
first CI transcript.

**Honest remainders.** (1) The
standing user decisions stand
exactly as §6.17 left them —
white-on-accent contrast
(3.14:1), the panel splitter,
File-menu Save As, the two
disagreeing "New project"
results, `tab_visits_controls`
OFF, project-swap undo, the skew
channel, the whole-figure pose
transform, MCP `write_recovery`,
the waveform badge/call, S1's
seven i18n value-vs-label
discrepancies (RESERVED, neither
side changed), the subtitle-stack
cap/wrap, a panel export-scale
control (core/MCP only by design)
— and this wave touched none of
them. The LICENSE file is still
absent from the repo root (X19's
F2; the claim rests on
Cargo.toml's licence field).
Sound remains user-verified only.
(2) **Wave W's build is red until
round 3.** The tokened range-video
progress stream emits the terminal
count twice (the wire shape has
no `finished` flag); the fix
belongs in the transport
translation or the drainer's
emission, and W3's pin stands
exactly as written. Until a
Build, Test & Binary run lands
green on a commit carrying Wave
W, Wave W stays IMPLEMENTED, not
VERIFIED — and Wave X's
integration run will execute the
same pin against the same path.
(3) **The live-MCP headless probe
assumes the default W4 removed.**
Its step launches the editor on a
wiped profile and waits for an
endpoint that, since the
explicit-enable flip, correctly
never starts; its own comment
still says the pref "defaults
ON". The repair is probe-side —
seed `./projects/.layout.json`
with a stored `live_mcp: true`
(the file W4's design honours),
or drive the Settings toggle —
and would double as the first
end-to-end proof of the
stored-true path. Owner: the
headless maintenance seat (the
step is in X17's stated region;
its audit this wave did not flag
the premise). No app change is
owed or wanted.
(4) **The pair gate's standing
transcript verdict is still
UNPROVEN.** X10's redesign is in
tree and PASSes on both real
artifact sets by local run
(coverage distance 0 identical /
48–51 different, floor 12), but
no CI transcript carries it yet;
the Wave X integration headless
owes the first one. Until then,
Home thumbnails are legacy-proven
and redesign-proven locally, not
snap-proven in CI.
(5) Alpha export has no panel
surface (X8): core + (via
IA-X8-1) MCP only. A panel alpha
toggle is a product decision for
the user — flagged, not built.
Alpha APNG is the named
follow-up; GIF carries no alpha
worth having.
(6) Sprite enrichment has no
panel controls (X9): the
round-trip closes through model /
import / export / MCP today;
panel steppers for opacity /
flip / scale_y / pivot are a
follow-up slice, and no fake
controls were added in their
place.
(7) The gate extractor's
tuple-carried-key blind spot
(X16): keys carried as
`(key, line)` tuples — W1's three
progress lines, X6's limit rows,
part of X3's state rows — are
undercounted by the ratcheted
instrument, so the counted total
is a floor on the truth.
Teaching check 1 a new pattern is
a deliberate recount + re-ratchet
for a future wave, not a
mid-wave change under every
slice's before/after census.
(8) Assign/unlink deliberately
never touch state keys (X2):
keys are authored and removed
only through their own ops, so
no op silently destroys another
op's authoring. A bulk
clear-all-keys press does not
exist; it composes per key, each
its own undo step, until a later
wave rosters a bulk op.
(9) The sprites panel's strings
remain literal English (X5) —
the file was never i18n-wired
and this wave followed its
convention rather than wiring
one file piecemeal; wiring it
wholesale is unrostered.
(10) §6.17 remainders closed
here: the WebM range outcome
((4) there) — X12 widened it,
its panel half still speaking
the class-C sentence by the
standing exclusion; the library
search ((8) there) — X13 fixed
it through the scan-based
resolver; the muted caption
((10) there) — X14 wired it,
closing the waveform strip's
last English-canonical caption;
R5 ((11) there) — X15 closed
the row citing the green round;
the Pivot JPEG residual ((9)
there) — X7(b) transcodes or
refuses by name; the README
half of (5) there — X19's pass
5 claims range audio, both
progress surfaces and the
live-bridge default (the class-C
in-app half stands, a product
ruling as before); the bundle
half of (6) there — the core
type exists and tokened bundle
calls stream (X11), the panel's
bundles remaining synchronous
by design; the probe half of
(2) there — the legacy leg is
transcript-proven and the gate
redesigned, its CI proof carried
at (4) above; and (7) there —
the SpriteLoop proposals P-1…P-6
+ P-4r are built this wave,
while P-7 (bend channel +
4-piece arc), P-8 (segment-media
local transform), P-9 (layered
import), P-10 (setup pose) and
P-11 (strip-warp, on X4's
subrect) remain genuinely
unbuilt — Wave Y's roster, per
W10's order. (3) there — the
stored-`true` migration
impossibility — stands exactly
as documented.

### 6.19 Wave Y — the SpriteLoop
build wave, second half: bend
channels, media transforms,
layered import, setup pose,
strip-warp; the Wave X frame
closed — with one correction
(2026-11-10)

Ordered 2026-11-10 (the standing
continuous order, re-issued once
more; the board was written ahead
so this wave launched the moment
Wave X closed): "assign 20 agents
continuous write code if ci gets
error then fix that". Twenty
agents, one owner per file,
integrator surfaces via filed
actions (board:
`~/workspace/wave-y/BOARD.md`;
findings:
`~/workspace/wave-y/findings/`).
The board's base is the Wave X
integration HEAD `d44098f` (MCP
390, i18n 678 × 28, element keys
995, State 49/22, scenes 34);
`src/i18n.rs` and
`src/rune_sec/i18n_wire.rs` were
frozen to this wave's agents, all
key work arriving via IA. This
section was written from all
nineteen sibling findings (y1–y19;
y20 is this section) and from the
CI frame re-verified first-hand
via `gh` and downloaded artifacts
at the final check — never from
workflow conclusions alone, per
the 42759195547 lesson. Every Wave
Y status below is IMPLEMENTED (in
tree, uncommitted at write time);
Wave Y has no CI verdict. The Wave
X frame, by contrast, is closed
here, exactly — both workflows
green in round 5 — with **one
correction to the wave's closing
summary**: the overflow scan over
the closing run's own artifacts
is NOT CLEAN, and §6.18's
remainders (3) and (4) both
retire inside this frame. The
correction is recorded in full
below, where it belongs: in the
frame, not in a footnote.

**The Wave X integration ledger.**
Five commits on top of `8fead5d`,
in order: (1) `a08e82c` — core:
X1's animation states (`StateKey
{ frame, clip, offset, speed }` on
`FigureInstance`, `apply_clips`
reclocking under an active key,
the legacy phase-locked path
verbatim otherwise), X2's ops
bundle (assign/unlink/set/remove
state keys, `create_clip_from_
figure_range`, and the coherence
fix — `delete_clip`/`move_clip`
remap `fig.states` in the same
snapshot), X6's joint limits
(`NodeDef` min/max angles,
`clamp_angle`, IA +4 keys), X9's
sprite-key enrichment (six
serde-defaulted `InstanceKey`
fields closing the four proven
.stknds round-trip losses). (2)
`3b983d8` — media/exports: X4's
bend-follow (`MediaItem::Image
src_rect`, two half-quads at
`bend_mid`; text stays on the
chord), X8's premultiplied RGBA
alpha funnel (PNG sequence +
spritesheet families, media and
subtitle tails at parity), X11's
bundle-progress core, X12's WebM
range outcome widening (all six
entries return `Mp4Export
Outcome`; the MCP reply names
`missing_audio`). (3) `540dd9c` —
MCP 390: X3's seven state tools
(registry 319 → 326), F7's splice
of X5's two sprite tools
(`sprite_set_asset`,
`sprite_reorder` → 390), IA-X8-1's
`alpha` argument on the `render`
tool, IA-X9-1's widened
`sprite_set_key` schema. (4)
`177e885` — panels: X3's
named-state panel section, X5's
sprite wins (stage click-pick,
asset picker, draw-order row,
dopesheet sprite lane), X14's
muted waveform caption. (5)
`7483e93` — docs/probes/CI: X10's
pair-gate redesign, F7's live-MCP
probe repair (the probe step seeds
the stored `live_mcp: true` its
premise requires), X15's citations
(QA_RUNE R5 closed; PHASE_O §2 →
42774207670), X18's docs sync,
X20's §6.18. Between the slices
and the rounds, F7 applied all 14
filed integrator actions (the
25-site `FigureInstance` literal
sweep, IA-X11-1's compile-blocking
`ProgressPhase::Bundle`, 12 i18n
keys → **678 × 28** measured, the
registry splice, IA-X7-1's two
`export_piv_v5` call sites, the
docs count refresh from a fresh
census) and measured the finished
tree: **MCP 390, i18n 678 × 28,
element keys 995 zero lost, State
49 fields / 22 literals, palette
134, scenes 34, SCAN_SET 16/0**;
gate PASS 10/10 on the git tree.

**The Wave W close-out frame**
(the two fix commits inside this
frame, re-verified from the runs).
Round 3 (`5ec229a`, the progress
sink suppressing non-advancing
reports): build 42776565547 FAILED
in a later target — lib green, then
two latent test-side pins the lib
target had been hiding: `health_
body_names_the_live_server` (a
racy two-read of the ENABLED flag)
and `loop_note_names_the_range_
one_based` (a stale pre-W6
"video-only" assert) — 2,149
passed / 2 failed in that target.
Headless 42776586689 concluded
success: census PASS, both Home
markers PASS, thumbnail UNPROVEN
(the old gate), **MCP PROBE: FAIL
(1 assertion)** inside the green
conclusion — the probe still
assumed the default W4 removed.
Round 4 (`8fead5d`: `health_body`
takes the flag as a parameter; the
loop-note assert updated): build
42777861274 **SUCCESS — 4,061
passed, 0 failed** (summed from
the run's log); headless
42777883168 SUCCESS with the same
transcripts as round 3 (census and
markers PASS; thumbnail UNPROVEN;
MCP PROBE FAIL — the repair was
already filed and landed with Wave
X's tree, see below). This
section's own overflow scan over
round 4's downloaded artifacts,
run with the `d44098f` scanner:
**CLEAN (0 hits, scan set
complete)** — the honest baseline
against which the correction below
is measured. Wave W is VERIFIED:
both workflows green on `8fead5d`,
every failure in its four rounds
test-side, never product code.

**The Wave X CI frame — five
rounds, every verdict read from
logs or artifacts, none from
conclusions.** Round 1 (HEAD
`7483e93`): headless 42778622657
SUCCESS, and its transcripts are
the frame's firsts — subtitle
census PASS, both Home markers
PASS, **HOME THUMBNAIL PROBE:
PASS** (X10's redesigned pair gate
live in CI for the first time: the
artifact transcript reads pairwise
ink-coverage distances 0 / 51 / 48
against the floor of 12, the
missing-audio thumbnail at
distance 0 from the singleton row
across processes; legacy legs at
W7's numbers, rows 0.333 / 0.337 /
0.339, ink 115 / 115 / 59), and
**MCP PROBE: PASS** (F7's
probe-side repair proven end to
end — §6.18 remainder (3) retires
here). Build 42778604118 FAILED at
compile: exactly two errors, both
in test targets — E0425 (a missing
`export_jpeg_sequence_with_
render_progress` import) and E0599
(itertools' `positions` called on
a `Windows` iterator in a pin).
Fixed test-only in `da4cd15`.
Round 2 (`da4cd15`): headless
42779624469 SUCCESS, transcripts
identical in every particular.
Build 42779603749 FAILED — lib
target 1,699 passed / 8 failed,
all eight test-side: four
`alpha_export_tests` pins (X8's
fixture figure sat off-canvas at
the shrunk stage size), three
`wave13_c6` wire-shape pins
(X9's new `InstanceKey` fields
changed the pinned JSON), and
`tool_table_is_complete_and_
unique` (the registry table count,
319 → 328 with X3's tools and
F7's splice). Fixed in `25c0ea9`
(fixture re-rooted; wire pins
updated to the real shape; table
count). Round 3 (`25c0ea9`):
headless 42780657922 SUCCESS, same
transcripts. Build 42780637045
FAILED — 1,706 passed / 1 failed:
`alpha_render_respects_figure_
opacity`, "a half-opacity figure
caps coverage near 128, got 223".
223 is the exact triple-overlap
composite 255 × (1 − (1 − 0.5)³):
the pin's premise (one layer of
coverage) was wrong where three
segments overlap, not the funnel.
Fixed in `5818061` — the pin
rewritten to assert the real
composite semantics; no product
code touched. Round 4 (`5818061`):
headless 42782279094 SUCCESS, same
transcripts. Build 42782257565
FAILED one target in: lib green
(1,707 passed / 0 failed) and
every other target green except
`tests/wave_r_undo_census.rs` —
`census_covers_every_tool_
exactly_once`, 18 passed / 1
failed: the Wave R census
classification still covered the
381 surface while the registry had
grown to 390 (its left set lacks
exactly X3's seven state tools +
`sprite_set_asset` /
`sprite_reorder`). Fixed in
`d44098f` — the census extended to
390: 209 mutating legs / 21
exceptions / 160 non-mutating.
Round 5 (`d44098f`): build
42784003685 **SUCCESS — 4,158
passed, 0 failed**, summed from
the run's own log over its 56
test-result lines; headless
42784024319 **SUCCESS** with the
full transcript set PASS for the
fifth consecutive dispatch
(census, both markers, thumbnail
probe with the pair gate, MCP
probe). All five rounds' failures
were test-side, never product
code — the same sentence §6.18
wrote for Wave W, now true of
Wave X. SpriteLoop GAP P-1…P-6 +
P-4r are landed and CI-verified.
Wave X is VERIFIED on both
workflows.

**The correction — the overflow
scan over Wave X's artifacts is
NOT CLEAN.** The scan is a debrief
instrument: no CI step runs it, so
no transcript in either workflow
carries its verdict, and the
wave's closing summary line
("overflow CLEAN") came from log
greps, which cannot see it. Run
the integrator's way — the
`d44098f` scanner over the
downloaded artifact tree dumps —
the verdict is: **40 hits, one
family, in 20 of 37 dumps, present
identically in round 1's artifacts
(42778622657) and round 5's
(42784024319)**, and CLEAN on Wave
W round 4's artifacts by the same
method (above), so the defect is
Wave X's, introduced and carried
through all five rounds while the
CI loop chased the build target.
The family is X5's sprite
Draw-order row (`sprite-z-row` in
`src/rune_sec/sprites_panel.rs`):
readout + `sprite-z-back` (109.5
pt) + `sprite-z-front` (98.5 pt) +
gaps sum wider than the card's
316 pt content width, and Rune
does not clip — `sprite-z-front`'s
right edge lands at 1426.0,
**+14.0 pt** past the 1412.5
content edge [content+card], its
unkeyed label child at 1416.0,
**+4.0 pt** [content]. The 20
affected dumps are every tree that
mounts the sprite edit area —
including SCAN_SET-required
`tree-default.json`, so the
standing "CLEAN on all SCAN_SET
trees" contract fails on this
dispatch; the other 17 are clean.
Three seats reached this verdict
independently this wave (Y15's
citation audit, Y17's coverage
audit, and this section's own
scan), with identical measurements
— and Y15's measurer was first
calibrated against the previous
citation's artifact, where the
same script reads CLEAN and every
O-1…O-9 edge reproduces, so the
NOT CLEAN is the artifact's fact,
not instrument drift. No exemption
was added, no rule weakened, no
threshold moved. The repair is
filed as IA-Y17-1 and has already
landed in this wave's tree: Y14
(the panel's owner seat) wrapped
the row (`.flex_wrap()` on the
bounded width, in the file's own
comment's words "an overflow
repair") — its proof is owed by
the first headless dispatch on
Wave Y's integrated tree, whose
artifact must scan CLEAN (0 hits,
scan set complete) under Y17's
grown 18-entry set before anyone
writes the word CLEAN about a
dispatch again. Until then the
honest sentence is: Wave X's
workflows are green; Wave X's
overflow contract is not yet
re-proven, and the PHASE_O §10
citation upgrade stays owed to
that first CLEAN-scanning dispatch
(Y15 withheld it, correctly).

**Wave Y, agent by agent** (from
their findings; every status
IMPLEMENTED until the Wave Y
integration CI):

**Y1 — P-7 bend channel (core).**
`ChannelTarget::Bend(usize)` in
graph.rs on the Length pattern —
serde `{"Bend":n}`, evaluated in
the same loop and phase as the
other targets, value stored raw
(the model keeps bends unclamped;
the ±0.7 discipline is the
inspector's, not the core's).
`ChannelResult` gains `bends`
rows seeded from `bends_at` before
the loop, so a project with no
bend channels evaluates
byte-identically to before — a pin
asserts it — and bends stay
render-only: no pose data anywhere
gained a field. Six pins. The
consumer routing was deliberately
not this slice's (Y2's follow-up
brief took it); IA-Y1-1 named the
five exhaustive-match arms the new
variant requires.

**Y2 — P-7 plumbing + 4-piece
arc + routing.** The graph panel
can author bend channels: Bend
pinned in the selector after
Angle, `key_bend` keying the value
in force at the playhead (the
channel's own value when one
exists, else the frame bend) in
one undo step, one new element key
(`graph-key-bend`). IA-Y1-1's five
arms landed verbatim, and the
consumers were routed from
`bends_at` to the channelled rows
— export's `collect_frame_art`
(plain and alpha funnels share
it), svg.rs, stage_view.rs — with
the no-channels path byte-identical
throughout. The arc upgrade:
paint.rs and svg.rs now draw a
bent segment as four arc quarters
(a flat bent segment is 4 pieces,
was 2; a varying one 16, was 8),
with cross-surface parity pins.
Two honest edges: segment-MEDIA
quads still follow frame bends
only (media.rs is Y9's file — the
funnel takes channelled rows in a
later pass), and MCP cannot yet
address a bend channel —
`gk_target_json` emits `"bend"`
but `parse_graph_target` does not
accept it; filed as IA-Y2-1 with
the exact fix.

**Y3 — P-8 media transform
(core).** `SegmentMedia::Image`
gains flat, individually
serde-defaulted fields — `offset
[f32; 2]` (stage units, in the
segment's own frame), `scale`,
`angle_offset` (degrees),
`flip` — each skipped on write
while it holds its identity
component, so an untransformed
project serialises byte-identically
to a pre-transform file (the
wave12 pin stands). Composition in
`media_items` only: the quad is
placed about the transformed
centre, the bend split rides on
it, flip reverses each item's
source-u range continuously across
the shared edge; at the identity
the emitted items equal the
pre-transform items bit for bit
(pinned). Sanitisation happens at
composition, never in storage —
stored projects are never
rewritten by rendering. The
mechanical sweep touched every
`SegmentMedia::Image` literal and
pattern in the tree (doc comments
untouched, per X9's lesson).

**Y4 — P-8 inspector + MCP
surface.** Written against Y3's
settled flat shape after its
nested-struct draft was overtaken
mid-slice (the slice was rewritten
in full; no nested-shape code
remains). The inspector gains
stepper controls for the four
fields (steppers only — exact
values are MCP's path, as with
bends and limits), and the media
MCP handlers accept and report the
transform, one undo step per edit.
Two IAs: IA-Y4-1 (the catalogue
keys, ×28) and IA-Y4-2 (registry +
`get_node` read parity — until it
lands, the figure-side read still
reports the two-field image shape
while the clip-side read reports
six; stated, not smoothed over).
The stale inspector module doc Y1
flagged ("why bends have no graph
override") is corrected in this
seat, its owner by Y1's note.

**Y5 — P-9 layered import
(core).** New `src/layered_
import.rs`: reads the documented
`spriteloop.import.json` package
plus a plain folder-of-images
fallback, all validation factored
through the shared `asset_from_
bytes` gate. The discipline is
refuse-before-any-write at package
scale: one corrupt part refuses
the whole import — no snapshot, no
asset, no media touched. Parts
attach to figure nodes by slug
match (part `id` or slugified name
against slugified node names) as
segment media; unmatched or
invisible parts degrade to free
sprites carrying the part's mapped
bounds, scale and opacity — no
part is ever dropped. Attached-part
opacity has no segment-media home,
so it is counted in the import
report (`attached_opacity_
dropped`), not silently absorbed;
no node, figure or pose is ever
created by an import — layers
dress the stick figure, per the
stickman rule. One undo step.
IA-Y5-1 is the lib.rs module line.

**Y6 — P-9 GUI + MCP surface.**
Palette command `import-layered-
set` (palette 134 → 135) resolving
the staged set under `./import/`
deterministically and refusing in
the house voice when nothing is
staged — no fake control — plus
the registry tool `import_
layered_set`, whose handler
verifies the effect from the
resulting state (asset and sprite
table deltas exactly the report's
counts; attached + sprites ==
parts) before reporting success.
IAs: IA-Y6-1 (lib.rs line),
IA-Y6-2 (the registry splice —
census 390 → **391** when it
lands), IA-Y6-3 (the docs row, via
Y18's sweep).

**Y7 — P-10 setup pose (core).**
`set_rest_from_pose` with the two
boarded modes: Rebase shifts
Frame.poses, rig-authored
ClipDef.frames and the figure's
Angle channels by the shortest-path
rest delta (tangents invariant —
a constant shift moves values, not
slopes); Absolute changes only the
rest angles. A clip rebases iff it
was authored on this figure's rig
— same node count, same parent
map, def rest exactly the old rest
— so other rigs' clips are
JSON-identical after (pinned); the
recorded caveat: two figures
sharing one identical rig share
that identity, honestly, because a
clip authored on either was
authored on the rig. StateKeys
ride the shift through the clip
store and are themselves never
touched (pinned both halves).
Refusals and the already-rest
no-op spend no snapshot; success
spends exactly one.

**Y8 — P-10 inspector setup
mode.** A runtime-only setup-mode
flag (never serialised; undo
stores Project only, so the mode
leaves no residue by construction):
while on, the whole stage draws
rest poses and stage drags edit
rest through the existing solvers,
write-back clamped through X6's
joint limits — rest data must
satisfy the rig's own limits;
frame poses stay unclamped
performance data, as always. The
root joint is not a setup drag
(a rest pose has no root position):
the press is consumed with its
reason posted, no snapshot. Three
IAs: IA-Y8-1 (editor.rs
`begin_drag` branch), IA-Y8-2
(stage_view.rs rest drawing),
IA-Y8-3 (the catalogue keys).
Stated remainders: playback in
setup mode shows rest poses (a
rigging mode is not a playback
mode), and the panel's Flip /
Centre / Ground stay current-frame
ops — gating them was not boarded,
and the choice is on the record.

**Y9 — P-11 strip-warped media.**
New `model::bend_arc_point` (the
arc point at fraction t, in
`bend_mid`'s own convention,
numerically stable as the bend →
0; t = 0.5 delegates to `bend_mid`
verbatim). The bend branch of
`media_items` now emits K strips,
K = round(|bend| / 0.5) clamped to
[2, 8]: spine points computed once,
strip edges sharing spine values
bit-for-bit, end edges the chord
quad's own corners, source tiling
exact — and at K = 2 the stream is
bit-identical to X4's two
half-quads, so every bend X4 pinned
(≤ 1.0 rad) emits the identical
stream and item counts change only
from 1.25 rad. The warp composes
with Y3's transform (spine points
ride the transformed centre; flip
reverses strip u ranges
continuously). NOT built, by
decision on the record: chain-
spanning media (one picture across
upper-arm + forearm) needs a span
model the GAP priced as its option
(b) — it awaits a user brief; the
spine-generic builder is the seam
left for it. paint.rs deliberately
untouched (Y2's file this wave).

**Y10 — pair-gate follow-through.**
X10's redesign left one hole: the
ink legs compare masks anchored on
each strip's own band, so a pair
whose bands sat at different
geometry with identical ink would
PASS on ink while the thumbnails
visibly differed in placement.
Y10 measured the constructions'
fixed geometry on the real
artifact sets, then added a second
leg to the pair region only: band
geometry agreement — origin and
size pairwise spreads within
`BAND_GEOM_TOL = 1` (the measured
legitimate variance of this
fixture; the FAIL direction is the
loud one, as with X10's ambiguous
band). Two new self-test fixtures
(a shifted pair member that the
old gate PASSes must now FAIL on
geometry; a near-bg singleton pins
the UNPROVEN branch, previously
unexercised): self-test green at
10 cases, and the strengthened
gate PASSes pair mode on both real
sets (42778622657 and 42774207670)
with the geometry line in the
transcript. No legacy predicate,
floor or ceiling moved.

**Y11 — states deepening.** The
audit proved two real gaps and
acquitted the third. (a) Panel
affordances: X3's states section
could key a clip only at offset 0 /
speed 1.0 and clear keys — the
only authoring path for a
non-default key was MCP, and the
strip's replace path silently
re-keyed over a key's offset/speed
with the defaults. Built: per-key
Offset and Speed stepper groups on
each state row, routed through
X2's `set_state_key` (one snapshot,
rig guard — the single authoring
path the strip and MCP share).
(b) Dopesheet visibility: state
keys were invisible in the lane —
a clip-driven figure showed one ◆
at frame 0 and no trace of its
switches, and the lane's empty
state could deny keys exist. Built:
state-key frames derived onto
`DopeRow`; state-only cells draw
◇ in the secondary accent, pose
keys keep ◆ byte-identically, a
frame holding both keeps ◆ plus a
secondary-accent border; the
gating widens so state keys alone
bring the rows up. (c) Pivot /
Stick Nodes round-trip of states:
NO gap — parity with the base
link, on the record. IA-Y11-1
files the two stepper labels
(×28, beside IA-X3-1's keys);
until insertion the panel renders
them via the Q7 `tr()` fallback,
never blank.

**Y12 — joint-limit surface
completion.** The audit found the
limits enforceable (X6) but
unpresettabile: the humanoid's
anatomical ranges existed only as
constructor literals. Built: the
values moved behind named consts
(`NodeDef::ELBOW_LIMITS` /
`KNEE_LIMITS`; `humanoid()` writes
through them, behaviour identical,
X6's pin standing), an inspector
preset op in X6's exact discipline
(validate → one snapshot → both
bounds → status; storing the
already-stored range is a silent
no-op) behind two buttons, and
graph readouts — `joint_limit_
note` appends "joint limits
{min}..{max}" to the stat row and
value line of a limited Angle
channel, inside the existing keyed
elements (no new element keys in
that file; the raw numbers stay
first — stored data is never
rewritten). IA-Y12-1 files the two
preset labels. MCP note, recorded:
no new tool — limit editing stays
panel-first, mirroring bend
editing, and enforcement parity
already holds through the shared
solvers.

**Y13 — alpha export completion.**
The audit: APNG was the one format
whose files are RGBA-typed yet
rendered through the opaque funnel
(X8's named follow-up); GIF's
1-bit transparency is a different
feature, not built, by the brief's
own scoping. Built: the alpha APNG
family in export.rs (+407/−0,
additions only) — alpha encoders
mirroring the opaque pair block
for block, entries mirroring the
opaque four including refusal
order and the 1.0 delegation, so
the opaque path is byte-identical
by construction (not one byte of
it edited) and a new pin asserts
the two families' bytes differ and
the opaque frames stay fully
opaque. IA-Y13-1 extends the
`render` tool's `alpha` argument
to the `"apng"` arm (the "exactly
two arms" comment becomes three;
the description sentence gains
APNG). Bundle members stay opaque
(no bundle alpha contract was
designed — X11's territory), and
the panel alpha toggle remains the
standing user decision, unbuilt.

**Y14 — sprite enrichment
completion.** X9's fields were
core/import/export/MCP-only; the
panel gains the full surface in
sprites_panel.rs, every write one
undo step through
`set_sprite_key_full` over the
panel's own `base_key`: opacity
stepper (0.05, whole-percent
readout), Scale Y stepper on the
EFFECTIVE value (stepping a
follow-`scale` key materialises the
axis only when the value actually
moves), a "Scale Y follows Scale"
toggle — the one affordance MCP
cannot mirror, since its `scale_y`
arg refuses the ≤ 0 sentinel by
design; the panel is where
follow-vs-explicit is authored,
recorded for parity bookkeeping —
Flip X / Flip Y toggles, Pivot X /
Y steppers, and the
`enrichment_readout` line. The
dopesheet gains the matching
readout row for the selected
sprite, reading the SAMPLE at the
current frame (what the stage
draws), never rounding a fractional
key onto a cell. This seat also
landed IA-Y17-1's repair on its
own file: the Draw-order row now
wraps at the card's bounded width
(`.flex_wrap()`) — the fix for the
overflow family in the correction
above; its acceptance is the first
CLEAN-scanning dispatch, still
owed at this record's close.

**Y15 — citations.** X15's method,
first-hand: build totals summed
from 42784003685's own log (4,158 /
0 — confirmed, not transcribed);
the headless artifact's transcripts
read from the files; and the
measurer calibrated against the
previous citation's artifact
(42774207670) before any number
was trusted — all 37 per-tree
counts and every O-1…O-9 edge
reproduce there, and the same
script reads that artifact CLEAN.
On the new artifact: O-1…O-9
reproduce exactly — **fifth
consecutive dispatch** — per-tree
counts moved upward in exactly the
20 sprite-panel trees, and
`tree-default.json`'s key diff is
exactly `sprite-z-row` /
`sprite-z-back` / `sprite-z-front`
added, none lost. PHASE_O §10's
provenance summary gains the
clause recording the Wave X
dispatch as NOT CLEAN-scanning;
the §10 citation upgrade itself is
**withheld** — it stays owed to
the first CLEAN-scanning dispatch
after Y14's repair, and §2's
citation stands at 42774207670
until then. QA_RUNE alignment
proceeds on the verified frame.

**Y16 — gate/census maintenance.**
Clean-archive baseline at
`d44098f` (the X16/F6 method — the
live tree carries this wave's
uncommitted work, so nothing was
counted there): every settled
count reproduces — keys **995**
(871 static + 124 dynamic, 61
files), MCP **390** (44 + 328 + 9
+ 4 + 5, census exit 0, docs sync
exact), i18n **678 × 28** by the
line-count convention (structural
parse 679 — the `status_ready`
four-line tuple artefact, disclosed
since F4, deliberately not
renumbered), State **49/22**,
palette **134**, icons **163**,
scenes **34**, SCAN_SET **16/0** at
base. The gate reads 9/10 on the
archive — check 2 (KEY DIFF) cannot
evaluate without `.git` and says
so; zero-lost was proven at
integration on the git tree (F7's
PASS 10/10), so the 9/10 is a
method artefact, classified as
such in the rehearsal below, not a
tree failure. Two floors the
integration pass had left behind
were ratcheted on self-verified
counts, with ledger lines:
`KEY_TOTAL_FLOOR` 984 → **995** and
`MCP_SURFACE_FLOOR` 381 → **390**.

**Y17 — overflow/headless
maintenance.** Ran the audit X17
could not: full-artifact debrief
scans of the Wave X dispatches —
and filed the headline in the
correction above (40 hits, one
family, `tree-default.json` among
the 20; IA-Y17-1 to the panel's
owner seat, since landed by Y14).
Maintenance proper: SCAN_SET grows
by the two carriers the Wave X
surfaces created — `tree-clips-
populated.json` (the states
section's only carrier) and
`tree-sprites-populated.json`
(the sprite lane's) — both
`required` from the start on the
established promotion terms: set
now **18 required / 0 pending**.
Required governs presence in a
debrief directory, not cleanliness
— the sprites entry stays required
while its tree carries the
IA-Y17-1 hits; that is the point of
naming it. headless.yml needed no
edit (loops verified set-equal at
34/37), and F7's live-MCP probe
repair was verified in place
(headless.yml:646-652 seeds the
stored pref with the corrected
comment), not re-applied.
IA-Y17-2 records the remaining
coverage gap: no scene plants a
figure WITH `states`, so the
state-row form appears in no dump;
a `pending` entry was deliberately
NOT added (the self-test pins the
pending side empty) — the scene
and its SCAN_SET promotion land
together when a scene is boarded.

**Y18 — MCP docs sync.** Zero new
drift: the escaped-pipe-aware sweep
reproduces the settled split
exactly — **304 identical / 86
condensed** — at base and on the
finished tree; the 86 are the
standing condensed-form rows, none
edited. One stale count the sweep
exposed was fixed: the Editing
registry section header said 319,
a pre-splice number — F7's splice
took the registry to 328 and the
file's own count table already
said so; the header now agrees
(328). Every Wave Y docs IA is
gated on an integrator code action
that has not landed yet (the
IA-Y6-2 splice, IA-Y13-1's arm,
IA-Y2-1's description spellings,
IA-Y4-2's parity), so none was
applied here — applying any now
would put the catalogue ahead of
the surface, the drift this seat
exists to prevent; each is filed
verbatim, in landing order, for
the integration pass. (An early
background census reading of
391/329 was not reproducible in
three foreground runs and is
disregarded on the record; the
surface at filing is 390.)

**Y19 — README truth pass 6.**
Wave X's landed GAP items are now
claimed, each verified against its
findings before a word was
written: animation states (panel
or MCP authoring, assign/unlink/
range-create, the retarget guard,
the delete/move remap), bend-follow
media (text on the chord, by
design, stated), the sprite wins,
enriched sprite keys (the
round-trip claimed through import /
editing / .stknds export), joint
limits (both solvers + graph
channels, mid-first IK clamp,
stored poses never rewritten), and
alpha export — claimed **for the
library and MCP only**; no panel
claim made or implied, the toggle
being the user's decision. One
pass-5 sentence was falsified by
Wave X and corrected: the README
no longer says `export_bundle`
sends no progress — X11's combined
count streams on stdio, and the
paragraph now says so (panel
bundles stay synchronous, called
out specifically). Flags carried,
re-checked, not edited: F1 (CI-only
framing), F2 (**no LICENSE file in
the repo root** — the licence claim
rests on Cargo.toml's
`GPL-3.0-or-later` alone; a flag
since T14, still the user's call),
F3, F4, F9 (the class-C range
sentences — the standing exclusion);
F10 is new: the absent panel alpha
toggle, recorded so the claim's
boundary stays visible.

**Integration-eve rehearsal** (on
the clean `git archive d44098f`
tree — the shared tree carries
this wave's uncommitted slices, so
nothing was counted there; Y16's
baseline and this section's own
runs agree). Settled counts versus
rehearsal: MCP **390** — census
exit 0, all PASS (44 builtin + 328
registry + 9 io + 4 subtitle_io +
5 subtitle), gate check 9 doc-sync
exact; i18n **678 × 28** —
independent structural parse reads
679 entries per table with
identical ordered sequences, the
difference being exactly the
`status_ready` tuple artefact
(convention kept, not renumbered);
element keys **995** by the gate's
method (871 + 124), zero lost per
integration check 2; State
**49 fields / 22 literals**;
palette **134**; icons **163**;
scenes **34**; SCAN_SET **16
required / 0 pending** at base
(18/0 once Y17's promotions land).
Script self-tests on the archived
tree: subtitle census PASS,
thumbnail probe PASS (10 cases
incl. X10's pair fixtures and
Y10's two new ones — the probe
script's self-test surface as
archived), overflow scan SELF-TEST
all checks passed. Failures and
anomalies, classified:
**(a) Pending-IA (expected
mid-wave state; none is a
regression).** The Wave Y
integration pass owes: IA-Y5-1 +
IA-Y6-1 (two lib.rs module lines),
IA-Y6-2 (the registry splice →
census 391) with IA-Y6-3's docs
row, IA-Y4-1 + IA-Y8-3 + IA-Y11-1 +
IA-Y12-1 (the catalogue keys —
measured at integration, never by
arithmetic), IA-Y4-2 (registry +
`get_node` parity), IA-Y8-1 +
IA-Y8-2 (setup-mode editor/stage
wiring), IA-Y13-1 (the APNG alpha
arm), IA-Y2-1 (the MCP bend-target
parser + its description
spellings), Y18's verbatim docs
rows in landing order, and the
count-prose refresh from a fresh
census. IA-Y1-1 was discharged
inside the wave (by Y2); IA-Y17-1
was discharged inside the wave (by
Y14) with its acceptance proof
still owed (see (b)). Until the
IAs land, the rehearsed Wave Y
tree does not compile — the green
python instruments are not a
compile verdict, exactly as §6.18
said of Wave X's eve.
**(b) Regression (one, Wave X's,
found by this wave's audits).**
The overflow family of the
correction above: a Wave X product
defect (X5's Draw-order row wider
than its card), present in every
Wave X dispatch's artifacts, in no
workflow's transcript because no
step runs the scan. It is not
pending-IA-shaped and was not
caused by anything in Wave Y.
Repair landed (Y14); classification
stays regression until a dispatch
on the integrated tree scans
CLEAN — that dispatch is also the
one that releases the PHASE_O §10
citation (Y15) and the next
"overflow CLEAN" sentence anyone
writes. **(c) Method artefact
(one).** Gate check 2 on an
archive tree (no `.git`) cannot
evaluate its diff and reports
9/10; on the git tree the same
check passed at integration. Not a
tree failure; recorded so the next
archive baseline is not mistaken
for one. No other instrument
failure on the rehearsed tree.

**Remainders.** (1) **The overflow
acceptance dispatch.** IA-Y17-1's
proof — a headless artifact set
scanning CLEAN (0 hits, scan set
complete) under the 18-entry
SCAN_SET — is owed by Wave Y's
integration run; the PHASE_O §10
citation upgrade rides on the same
dispatch (Y15). If the wrap does
not hold the row inside 316 pt at
both window widths, the row's
anatomy (readout + steppers +
gaps) must narrow — no exemption,
no threshold move. (2) **Bend
channels have two half-open
edges**: segment-media quads do
not yet follow them (media.rs
takes `bends_at` directly; the
funnel's channelled-rows routing
is a later pass, Y2 flagged it
rather than touching Y9's file),
and MCP cannot address a bend
target until IA-Y2-1 lands. Stage,
raster and SVG segment LINES
follow bend channels today. (3)
**Chain-spanning media** (Y9):
one picture across a two-segment
chain needs the span model (GAP
option (b)) and a user brief; the
spine-generic builder is the seam.
Not improvised. (4) **State-row
scene coverage** (IA-Y17-2): no
headless scene plants a figure
with `states`, so X3's state-row
form is in no tree dump; the scene
+ SCAN_SET promotion land together
when boarded — the pending side of
SCAN_SET stays empty by design
until then. (5) **The panel alpha
toggle** (X8/Y13): core + MCP
carry alpha for PNG sequence,
spritesheet and (via IA-Y13-1)
APNG; a panel toggle remains a
product decision for the user —
flagged since X8, claimed nowhere
(Y19's F10 keeps the boundary
visible). Bundle members stay
opaque until a bundle alpha
contract is designed. (6) **Setup
mode's stated edges** (Y8):
playback in setup mode shows rest
poses; Flip / Centre / Ground stay
current-frame ops; numeric
rest-angle steppers were not
boarded and were not built — no
fake controls. (7) **`get_node`
read parity** until IA-Y4-2 lands:
the figure-side read reports the
two-field image shape, the
clip-side six (Y4). (8) **Attached
layered-import opacity** (Y5) is
counted in the import report, not
stored — segment media has no
opacity home, and none was faked;
a transform-payload home for it is
a future design, not a field added
in passing. (9) §6.18 remainders
closed here: the live-MCP probe
premise ((3) there) — repaired by
F7, proven PASS in all five Wave X
headless transcripts; the pair
gate's standing UNPROVEN ((4)
there) — PASS live in all five,
and Y10 closed the geometry hole
X10's design left. Standing as
documented there: the stored-
`true` migration impossibility,
assign/unlink never touching state
keys (bulk clear composes per
key), the sprites panel's literal
English (Y5's and Y14's slices
followed the file's convention;
wiring it wholesale is unrostered),
and the gate extractor's
tuple-carried-key blind spot —
Y16's floor ratchet (984 → 995)
keeps the counted total a floor on
the truth, and teaching check 1 a
new pattern remains a deliberate
recount for a future wave. (10)
All standing user decisions from
§6.18's list stand untouched by
this wave: accent contrast, panel
splitter, File-menu Save As, the
two-"New project" results,
`tab_visits_controls` OFF,
project-swap undo, skew,
whole-figure pose transform, MCP
write_recovery, waveform badge,
GIF import, panel export-scale
control, subtitle-stack cap/wrap,
S1's seven i18n discrepancies, the
export class-C sentences
(machine-read — including the
range WebM "video only" sentence,
whose correction is a
catalogue-side user decision,
flagged since W6), serialised
vocabularies in English, the
Settings-card Esc / menu-open
non-Esc key leak (PHASE_O §9) and
the figure-inspector
stale-selection copy (PHASE_O §4)
pending a product ruling, and the
absent LICENSE file (Y19's F2 —
a flag, never an edit). Sound
remains user-verified only. P-7…
P-11 are built this wave (P-11's
chain half at (3) above); the
SpriteLoop GAP list W10 reconciled
is now exhausted except what user
briefs still own.


**Wave Y CI close (2026-11-10):** closed GREEN in round 7 on `a37f2aa` (build 42798005496 — 4,248 tests passed, 0 failed; headless 42798031914 SUCCESS with every transcript verified: SUBTITLE CENSUS PASS, THUMBNAIL PROBE PASS, MISSING-AUDIO + SCAN-FALLBACK markers PASS, MCP PROBE PASS, and the artifact overflow scan CLEAN — 0 hits, scan set complete, the second consecutive CLEAN dispatch). Rounds 1–6 failed only in test targets plus one real semantics fix: round 6's `InstanceKey::sample()` sentinel preservation (the dopesheet "(follows Scale)" readout), completed in round 7 by `base_key` materialising the sampled scale_y on write — readouts keep the follow sentinel, stored keys freeze the effective value, follow re-enters only via the explicit toggle. The PHASE_O §10 citation upgrade (owed to Wave Z's Z12) stands released by dispatch 42792705307 and confirmed by 42798031914.

### 6.20 Wave Z — closing the
half-open edges: bend-channel
media, the state-row scene,
sprite dragging, bundle alpha;
the Wave Y frame closed
(2026-11-10)

Ordered 2026-11-10 (the standing
continuous order; the board was
written while Wave Y's CI loop
ran, so this wave launched the
moment Wave Y closed): twenty
agents, one owner per file,
integrator surfaces via filed
actions (board:
`~/workspace/wave-z/BOARD.md`;
findings:
`~/workspace/wave-z/findings/`).
The board's base is the Wave Y
closing HEAD `5031700` (MCP 392,
i18n 692 × 28 as then counted —
**corrected to 693 × 28 by Z13
below**, element keys 1017,
palette 135, scenes 34,
SCAN_SET 18 required / 0
pending); `src/i18n.rs` and
`src/rune_sec/i18n_wire.rs` were
frozen to this wave's agents,
all key work arriving via IA.
This section was written from
all nineteen sibling findings
(z1–z19; z20 is this section —
Z13's arrived last, in the
record's final minutes, and its
i18n correction below is
carried in full) and from the
CI frame assembled by Z19 and
re-verified first-hand via `gh`,
downloaded artifacts and the
python instruments — never from
workflow conclusions alone, per
the 42759195547 lesson. Every Wave Z status
below is IMPLEMENTED (in tree,
uncommitted at write time);
Wave Z has no CI verdict. The
Wave Y frame, by contrast, is
closed here, exactly — both
workflows green in round 7,
overflow CLEAN twice running.

**The Wave Y integration
ledger.** Five commits on top
of `d44098f`, in order: (1)
`63211e7` — core: Y1's P-7 bend
channel (`ChannelTarget::Bend`,
render-only rows seeded from
`bends_at`) and Y7's P-10 setup
pose (`set_rest_from_pose`,
Rebase / Absolute). (2)
`a26156d` — Y3's P-8 media
transform (flat serde-defaulted
`offset` / `scale` /
`angle_offset` / `flip` on
`SegmentMedia::Image`, composed
in the one funnel), Y5's P-9
layered import core (new
`src/layered_import.rs`,
refuse-before-any-write), Y9's
P-11 strip warp (K strips over
`bend_arc_point`, K = 2
bit-identical to X4), Y13's
alpha APNG family. (3)
`e202293` — MCP 390 → **392**:
Y6's `import_layered_set`,
IA-Y7-1's `set_rest_pose`,
IA-Y2-1's bend graph target,
IA-Y13-1's APNG `alpha` arm.
(4) `feb5826` — panels: Y2's
bend plumbing + 4-piece arc,
Y8's setup mode, Y11's state
offset/speed steppers, Y12's
limit presets, Y14's sprite
enrichment surface + IA-Y17-1's
Draw-order wrap repair. (5)
`ca9dc89` — tests/docs/probes:
the undo-census legs for both
new tools, gate/probe upkeep,
the records. Between the slices
the finisher (F8) applied every
filed IA and measured the
finished tree: **MCP 392, i18n
692 × 28, element keys 1017
zero lost, palette 135, scenes
34, SCAN_SET 18/0**; gate PASS
10/10 on the git tree.

**The Wave Y CI frame — seven
rounds, every verdict read from
logs or artifacts (Z19's
assembly, anchors re-verified
first-hand by Z12, Z18 and this
section).** Round 1 (`ca9dc89`):
build 42789303086 FAILED on a
single **E0382** in
`src/layered_import.rs` (the
apply loop moved `part.asset`,
then borrowed `part`); headless
42789337805 FAILED at its build
step — its census / thumbnail /
MCP failure lines were proven
missing-input consequences (no
binary, no dumps), not defects.
Fixed in `c239d83`. Round 2:
build 42791998179 FAILED in the
**bin target only** — **E0599**
(`image_transform_sanitised`
never implemented; Y4's panel
assumed a Y3 API) + **E0503**
(`selected_figure` read while
`&mut s.core` was borrowed);
headless 42792034293 the same
missing-input pattern. Fixed in
`95e4a45` (the panel sanitises
inline, mirroring the funnel;
the figure read hoisted). Round
3: build 42792663877 FAILED in
the **lib TEST target** — 9
errors (duplicate `image_items`
test helper, the `json` helper's
missing `?Sized`, a match-by-
value pin); **headless
42792705307 SUCCESS with every
transcript verified: SUBTITLE
CENSUS PASS, HOME THUMBNAIL
PROBE PASS (incl. Y10's geometry
leg — band 112×83/84, spreads
within tolerance 1, pair
distances 0 / 51 / 48), both
Home markers PASS, MCP PROBE
PASS — and the artifact overflow
scan CLEAN (0 hits, scan set
complete): Y14's wrap repair
VERIFIED, the first CLEAN scan
since the Wave X regression,
releasing the PHASE_O §10
citation Z12 takes below.**
Fixed in `3f75e8b`. Round 4:
build 42793326988 FAILED on
exactly **1 of 1,755** lib tests
— the media-transform pin's
exact corners vs the funnel's
f32 dust (59.999996 / 50.000008
vs 60.0 / 50.0); headless
42793362928 SUCCESS. Fixed in
`1dcb085` (tolerance compare).
Round 5: build 42795203136
FAILED on **3 tests** (3,978
passed summed) — two stale
X4-era stage pins (2 flat / 8
gradient pieces expected vs
P-7's deliberate 4 / 16 at the
quarter points) and one REAL
semantics defect: the dopesheet
enrichment line lost its
"(follows Scale)" suffix because
`InstanceKey::sample()` resolved
`scale_y` to a number, erasing
the sentinel stored-key readouts
keep; headless 42795236749
SUCCESS. Round 6 (`3fde76d`:
`sample()` keeps the sentinel
when both endpoints follow; the
arc pins re-derived): build
42796321211 FAILED on **2 tests**
(3,979 passed summed) — the
write-side half of the same
thread: key authoring between
enriched keys now received the
sentinel (0.0) where the stored
key must carry the materialised
effective value (1.0); headless
42796352469 SUCCESS. Round 7
(`a37f2aa`: `base_key`
materialises the sampled
`scale_y` on write): build
42798005496 **SUCCESS — 4,248
passed, 0 failed**, summed from
the run's own log over its 56
test-result lines (re-summed
independently by Z18, Z19 and
this section); headless
42798031914 **SUCCESS** with the
full transcript set PASS and the
artifact overflow scan **CLEAN —
the second consecutive CLEAN
dispatch**. The frame's only
product-code fix commits after
integration are the r6/r7
sentinel pair; every other fix
is test-side or compile repair.
Wave Y is VERIFIED on both
workflows. The docs-close commit
`5031700` (this record's §6.19
close note, docs only) pushed
with build 42798924312 and
headless 42798957551 **still in
flight at this section's write
time**; no product behaviour
rides on that round, and its
verdicts belong to the next
record, not this one.

**Wave Z, agent by agent** (from
their findings; every status
IMPLEMENTED until the Wave Z
integration CI):

**Z1 — bend-channel media
routing.** §6.19 remainder (2)
closed on its media edge: the
one consumer that bypassed the
channelled evaluation was the
funnel's bends row (`media.rs`
took `bends_at` directly, under
a pre-P-7 comment that was no
longer true). One hunk in
`media_items` routes the row
through the `ChannelResult`
the funnel already computes —
Y2's consumer idiom verbatim —
so stage, raster and SVG follow
bend channels together with
zero consumer edits (the funnel
is the one stream); the
no-channels path is `bends_at`
byte-for-byte. Four pins: a
channel alone bends media with
no frame bends anywhere, a
channel keyed 0.0 overrides a
frame bend (override, never
additive), the channel value
drives Y9's strip count, and a
non-bend channel leaves the
stream identical. Text still
rides the chord (X4's rule,
unchanged). No IA, no keys, no
MCP change.

**Z2 — state-row headless scene
(IA-Y17-2 discharged).** New
scene `states-populated`
(registry index 34; 35 scenes):
clips-populated's exact state
plus ONE state key authored on
figure 0 through the real op
(`EditorCore::set_state_key`,
clip 1 at frame 0, in force at
the parked playhead) — no direct
`states` write. Layout fact 37
records `clip-state-row-0` +
`clip-state-clear-0` inside the
states section. Both headless
loops dump it, and SCAN_SET
names `tree-states-populated.
json` **required from the
start** — the set is now **19
required / 0 pending**, the
pending side empty by design.
Scene + loops + SCAN_SET landed
as one change (the S2 pattern);
no IA owed. Its proof is owed
by Wave Z's first dispatch: the
tree must be PRESENT (absence
reads INCOMPLETE, never clean)
and judged under rules 1–2.

**Z3 — stage sprite dragging.**
X5's remainder built, on X5's
own pick order (draw → camera →
joint → sprite → stroke — the
drag inherits the pick, so a
joint under a sprite still wins
the gesture; no priority was
re-decided). The gesture splits
like `begin_drag` / `drag_to`:
the grab records a `SpriteDrag`
in a `thread_local` (the
subtitle lane's precedent) with
the pre-drag key list; moves
write the panel's `base_key`
shifted by the pointer DELTA
(the sprite never jumps to the
cursor) live, with no snapshot;
a clean release restores the
pre-drag list and commits the
final key through
`set_sprite_key_full` — exactly
**one undo step per drag**,
enrichment verbatim (a
follow-`scale` base materialises
its `scale_y` on write, the
Wave Y r7 behaviour, pinned); a
cancel restores byte-for-byte
at zero undo cost; a click or a
returned excursion spends
nothing; identity loss mid-
gesture abandons. Seven pins.
x/y only — rotation/scale stay
panel/MCP routes; the
right-press path still does not
pick sprites (X5's remainder
stands).

**Z4 — setup-mode numeric rest
steppers.** §6.19 remainder (6)
closed: the Setup section gains
per-node rest-angle steppers in
`inspector_figure.rs` (one file,
+343/−1) — the house stepper
idiom at the segment panel's 5°
step, write-back through the
same helper the setup drags use
(`norm_deg` then X6's
`clamp_angle`), so a numeric
edit and a drag leave the rig in
the same state. One press = one
snapshot = one undo step; a
press the clamp makes a no-op
is silent (no write, no status,
no step); stored frame poses
are never touched (pinned
byte-identical). Every node
takes a row, the root included
(its rest angle orients the
rest stance — not X6's root
limit refusal, which is about
SOLVED angles). Six pins. No
i18n IA (node names are project
data; the steppers add zero
catalogue strings) and no MCP
tool (rest editing already
rides the panel surfaces).
Caveat on the record: the rows
mount only in setup mode and no
scene turns it on, so they
appear in no headless dump —
overflow exposure nil on
current scenes, by the ring
row's scan-proven anatomy.

**Z5 — layered-import opacity
home (§6.19 (8) closed).** The
audit verdict: the field
composes CLEANLY — segment
media have exactly ONE alpha
composition point (the funnel's
segment-image arm: figure ×
carrier style), so a third
factor multiplied there reaches
stage, raster, SVG and stage
sprites together, and no
surface has its own path to
conflict with. Built in Y3's
discipline: `SegmentMedia::
Image` gains `opacity: f32`,
serde-defaulted to 1.0 and
skipped at 1.0 — pre-opacity
files load fully opaque and an
all-opaque project serialises
byte-identically (the wave12
pin stands verbatim). The
funnel sanitises at composition
(non-finite reads as 1.0; stored
values never rewritten). The
attach surface gains parity:
`MediaTransformArgs.opacity`
(validated 0..=1 up front, a
refusal spends no snapshot) on
both attach tools, and
`segment_media_json` reports
`"opacity"` so no read hides
writable state. Layered import
now STORES the attached part's
opacity, and the report field
is renamed for honesty —
`attached_opacity_dropped` →
**`attached_opacity_stored`**
(a deliberate surface change,
IA-Z5-3): Y5's counted remainder
closes; nothing is counted-and-
lost any more. An 81-site
mechanical constructor sweep
(Y3's method; the post-sweep
checker reads 83 blocks with
opacity / 16 `..` patterns / 0
unhandled) plus five pins,
including the triple product
0.5 × 0.5 × 0.5 = 0.125 and the
importer storing 0.5 end to
end. Two IAs ride: **IA-Z5-1 is
COMPILE-REQUIRED** (the layered
tool's reply follows the rename
in `mcp_layered_tools.rs` — the
tree does not compile until it
lands), IA-Z5-2 the registry +
docs mirror for the attach
schemas and node-media shapes
(census stays 392). Stated
remainder: NO panel opacity
control — a stored opacity
survives transform edits via
the `..` write-back but cannot
be viewed or changed in the
inspector; that surface is a
future seat, not faked here.

**Z6 — bundle alpha contract.**
Designed first, then built, in
`src/mcp_io.rs` only: the
`export_bundle` tool gains an
optional `alpha` boolean (the
`render` tool's parsing idiom;
the census does not grow). In
kind `images`, exactly the two
members whose formats carry
per-pixel alpha honour it —
`png_sequence` (through the
(scale × alpha) matrix, bundle
progress translator threaded)
and `spritesheet` (the unscaled
alpha entry; T8's no-scale rule
stands) — while `jpeg_sequence`,
`svg` and both video members are
written by exactly today's
opaque calls, byte-identical;
`alpha: true` on kind `video`
is accepted, recorded, and
changes no byte. The manifest
is the reply: absent/false →
byte-identical to today;
true → a top-level `"alpha":
true` plus a per-member
`"alpha"` boolean (true only
where the member was actually
written with alpha — a JPEG
member's `false` is stated,
never silent). No manifest file
is written; panel bundles are
untouched and stay opaque (the
panel alpha toggle remains the
standing user decision). Five
pins. IA-Z6-1 (the docs row)
was applied verbatim by Z15;
IA-Z6-2 is this paragraph.

**Z7 — chain-spanning media:
DESIGN ONLY, as boarded.** The
deliverable is the design in
z7.md; no product code changed.
First, a correction to the
board's shorthand: Y9's
"spine-generic builder" is not
a named function — the strip
emission is INLINE in
`media_items`, spine-generic in
structure (it consumes only
spine, half-height, end corners,
flip, alpha, asset); a chain
build starts by extracting it.
The recommended span model:
option A — `span_next:
Option<usize>` on the existing
`SegmentMedia::Image` variant,
serde-defaulted and skipped
when `None` (a non-spanning
image serialises byte-identically
to today), authored on the
PROXIMAL node, validity derived
never trusted (a stale span
degrades to today's single-
segment picture, never to a
missing one). The two bends
compose as concatenated arc
samplings with the shared joint
once; strips tile u by
ARC-POSITION fraction so the
joint sits at L1/(L1+L2) of the
source; the joint edge keeps
segment 1's width vector
(option (i); the rotated-width
quality follow-up is named, not
taken). Conflict rule:
authoring refuses over a node
already carrying an image;
rendering degrades span-wins.
Migration is additive-only —
no version bump exists to ride
and none is recommended. The
build is priced as one L-sized
slice in six steps. **It does
not start until the user
answers the design's nine
questions** (chain length,
conflict rule, fit, elbow
width, transform frame, fade,
text, the kink-vs-straight use
case, panel affordance) — each
with the design's default
stated, so the user can accept
by exception.

**Z8 — panel bundle progress
job.** The panel bundle was the
last synchronous export (W1's
findings recorded it as "the
panel's remaining blocking
press"; X11 left the wiring to
a panel slice — this one). Both
panel bundles now run on W1's
background-job mechanism under
X11's core contract, all in
`export_panel.rs`: a press
spawns a worker over a cloned
Project; the tracker is
allocated from the members'
totals BEFORE the first member
runs (images `[frames.len(); 4]`,
video `[sampled_frame_total; 2]`
— the MCP handler's exact
construction); members run
through the same hooked entries
the ops call, at the ops' own
destinations; tracker events
write into a THIRD slot on the
job (the bundle slot, kept apart
from the two phase slots exactly
as the phases are kept apart);
the readout shows one bundle
line (`xc-progress-bundle` —
the slice's one new element key,
zero lost). The finish writes
`bundle_recap` to status AND
recap together, byte-identical
to the synchronous handler's last
word — no class-C sentence was
reworded anywhere (the standing
exclusion held by construction).
A worker that never reports
fails EVERY member in the recap
("Exported nothing…") — the
honest summary, pinned. No
`State` field was needed (the
slot lives in `ExportJob`), so
no literal sweep; main.rs
untouched. The panel alpha
question is not touched (the
worker calls the plain entries;
Z6 owns the contract question).
Eight new pins, workers only —
no pin runs a real export (the
W1 discipline). **IA-Z8-1**: one
catalogue key,
`export_bundle_progress`
("Exporting bundle {done} of
{total} frames") — until the
integrator inserts it (×28),
the wire falls back to the key
text (the Q7 pattern) and the
extended catalogue pin says so.
Stated remainders: no cancel
for a running bundle (no
contract offers one; no fake
button), and no per-member
readout lines (the combined
count is X11's design — per-
member truth lives in the
recap, as on the MCP side).

**Z9 — library state round-trip
audit.** One question, three
library families, per-field
verdicts — and exactly ONE
in-scope non-preservation, a
format boundary rather than a
drop: **joint limits are
PRESERVED verbatim everywhere
they are stored** (2D figure
library — NodeDef fields ride
the def's JSON; project library
— the whole Project, in BOTH
def copies, each figure's and
each clip's embedded one;
recovery snapshots by the same
mechanism), **pose libraries
correctly carry neither field**
and provably do not interfere
(over-limit angles save and
load bit-exact — limits bind
solving, not data — and a pose
apply leaves the target's
states and limits untouched),
and **figure-library STATES are
not stored at all**: the payload
is the def only, and a key's
`clip` index names a clip in
the SAVING project, which the
library does not carry — in
another project the same index
would be dangling or, worse,
silently wrong against a
different clip. Carrying clips
is the §6.7-class format change
already declined for referenced
PNGs, so the seat reported it
rather than forcing it (the
board's escape rule). No product
code changed; the build is four
files of pins (+284/−0) making
every verdict above — including
"an inserted figure starts
stateless" — a conscious,
pinned behaviour a future
format change must move
deliberately.

**Z10 — limit preset sweep
audit. REPORT; nothing built.**
The seat's rule was build-only-
if-exact, and the mapping is
not exact on any axis: the
preset words appear in NO node
name anywhere (humanoid names
the elbow-joint segments
"forearm L/R" and the knee
segments "shin L/R", applying
ranges by INDEX); names denote
segments while limits constrain
the joint at the segment's BASE,
so even the best case is an
inference; the constructors
disagree on vocabulary
("arm L" merged segments with
no elbow at all, quadruped
"hind lower" with no honest
anatomical word); and the
population the sweep exists for
carries purely mechanical names
(Pivot "segment {id}", Stick
Nodes "rounded-segment 12",
editor "segment {ni}") — a
sweep over those matches zero
nodes and reporting success
would be the partial-and-quiet
outcome the seat forbids.
Structural matching was checked
too and is also a guess
(quadruped hind lowers satisfy
the chain shape AND carry the
"elbow" bend sign). IA-Z10-1 is
deliberately UNISSUED — the
numbering gap is the decision,
on the record. The exact
precondition for a future
sweep is role METADATA (a
serde-defaulted joint-role
field written by the
constructors), a model change
beyond this seat; per-node
preset application (Y12) stands
as the honest route meanwhile.

**Z11 — bend-channel polish
audit.** Surface-by-surface
against Angle/Length: labels,
selector, authoring, every
generic graph edit, timeline
marks, MCP (IA-Y2-1) and
evaluation were verified AT
parity by reading, not assumed.
Five gaps were proven and
built. **G1:** the graph
readouts formatted radian bend
values as `{:.1}` — a 0.04 rad
bend printed "0.0", invisible;
bend now formats `"{v:.2} rad"`
in the header, value line and
stat range (every other target
byte-identical). **G2:** Reduce
DESTROYED bend channels — the
0.5 tolerance is half a DEGREE
in angle units but half a
RADIAN (≈71% of the max bend)
on a bend channel, dropping
essentially every interior key;
bend now reduces at
`REDUCE_TOLERANCE.to_radians()`.
**G3:** bake silently DROPPED
bend channels — `bake_graph_
channels` took only `.poses`
from the evaluation though
bends have a frame home; the
evaluated bend rows are now
written (figures no bend
channel targets keep their
tables byte-identical, pinned),
so a bend channel's motion
survives clearing the channels,
as the status always claimed.
**G4:** the segment inspector's
docs still asserted P-7's false
premise ("no graph channel can
override a bend") at three
sites — corrected, docs only
(the steppers deliberately keep
working on the STORED bend; the
channelled number's honest
surface is the Graph panel).
**G5:** the graph empty state
always advised "Key angle" —
for a Bend target it now names
"Key bend" (Root/Length honestly
name MCP `graph_key`). Eight
pins, no existing pin edited,
zero keys added or lost.
Equal-absence items were
reported, NOT built (dopesheet
marks no channel of any target;
the keyframe-tools section is
Angle-only by its own docs;
`auto_fit`'s generic fit for
bend is correct — a fixed ±0.7
fit would clip raw-stored
bends). **IA-Z11-1**
(comment-only): the MCP reduce
tool's "matches the panel"
comment is stale for Bend after
G2. One remainder flagged for
the board, not improvised:
Bend(0) channels can be authored
(selector + MCP) and are inert
at render — a refusal would
break `key_bend`'s mirror of
`key_graph_angle`'s shape.

**Z12 — citations: the §10
upgrade TAKEN.** The condition
Y15 withheld on is discharged
and verified first-hand in this
pass: the measurer was
calibrated against the previous
citation's artifact
(42774207670 — all 37 per-tree
counts and every O-1…O-9 edge
reproduce) before any number
was trusted; the closing
artifact (42798031914) scans
**CLEAN** under the current
script; `sprite-z-front` ends
at **1194.5** on both CLEAN
artifacts (was 1426.0, +14.0 pt,
on the Wave X frame) — Y14's
repair proven at artifact
level, not just in code. §10
now cites the Wave Y closing
dispatch: 22 per-tree counts
updated (all upward — the Wave
X+Y panel surface the cited run
predates), `tree-default.json`'s
diff exactly **21 keys added,
none lost**, O-1…O-9 unchanged
(sixth consecutive dispatch),
the clips + sprites carriers'
SCAN_SET flags corrected to
required (the contract stood at
18/0 on that frame). The first
CLEAN dispatch was round 3's
42792705307; per X15's precedent
the citation names the closing
one, and the two artifacts agree
on all 37 dumps, so the choice
moves no number. **The §10
citation debt is closed** — the
first citation since Wave X
with no withheld condition.
IA-Z12-1 (the QA_RUNE §19
pointer) is discharged by
Z18's §19.9, below.

**Z13 — gate/census
maintenance.** The baseline was
isolated properly: a detached
worktree of `5031700`, never the
shared tree (the X16/Y16
method). Every board expectation
verified exactly on the clean
tree — gate **PASS 10/10**,
keys **1017** (889 static + 128
dynamic, 61 files, zero lost),
State 49/22, scenes 34, MCP
**392** with doc sync exact,
palette 135, SCAN_SET 18/0, all
three script self-tests PASS —
and the two floors ratcheted on
those self-verified counts, with
ledger comments in the X16/Y16
style: **`KEY_TOTAL_FLOOR` 995
→ 1017, `MCP_SURFACE_FLOOR` 390
→ 392** (the edited script
re-run in the worktree passes
with both floors met exactly).
One genuine catch, the wave's
only count correction: **the
settled i18n count is 693 × 28,
not 692.** A trailing-comma-
tolerant structural parse reads
693 entries in each of the 28
tables (identical sequences,
keys unique, zero EN
value==key); the 692 figure is
an undercount by exactly the
`status_ready` key, whose tuple
is the file's only one formatted
across multiple lines with the
trailing comma INSIDE the
parens — every single-line pair
parser silently misses it (this
seat's first two parsers did;
§6.18's and §6.19's "structural
679 vs line-count 678" artefact
was this same tuple, read from
the other side). The tables
themselves are complete and
correct; only the count PROSE
is stale, and it lives in the
two FROZEN files — hence
**IA-Z13-1**: the integrator
corrects `src/i18n.rs`'s header
("exactly the same 692 keys" →
693) and `i18n_wire.rs:6`
("× 692" → 693); no table
content changes, and all prose
citing 692 ×28 for the settled
surface (the §6.19 record
included) should be read as
693. Z16's hygiene partition
(§ below) was built on its own
bounded parse of 692 and covers
those 692 keys; `status_ready`
itself is a live status-bar key
whose absence from the
partition is this same parser
artefact, recorded here rather
than silently renumbered in
another seat's paragraph.
Wave Z's own growth is NOT
floored by this seat — it is
the finisher's recount on the
integrated tree, per the
standing pattern.

**Z14 — overflow/headless
maintenance.** No edits — the
seat's product is the
independent verification that
Z2's promotion landed in
exactly the prescribed shape:
entry required from the start
with proving text, both loops
appended in mirror position,
header + counts prose extended,
NO pending entry added. The
battery, run at base and again
after the promotion: self-test
all checks passed both times;
`--scan-set` 18/0 → **19/0**;
per-loop cross-check stricter
than the script's own — scenes
35, both loops 35 and set-equal
to the registry and each other,
produced set **38 dumps**; and
the contract behaviour proven
against a pre-promotion artifact
(the closing frame's copy scans
0 hits with exactly one MISSING
line — the states tree —
verdict INCOMPLETE, exit 1: a
required tree absent is never
silently clean, and no other
tree regressed). Boundary
stated for the record: Z12 owns
citations, Z14 owns the
instrument and the set; Z2's
edits landed inside Z14's files
under Z2's own brief, and this
seat's role for them is
verification, not authorship.
Interim mid-wave gate failures
observed in this seat's window
(check 9 against the mid-splice
registry) are the moving-tree
artefact, not scan-set matters.

**Z15 — MCP docs sync.** The
escaped-pipe-aware sweep against
the settled census (392, zero
missing / phantom / misplaced
after the fixes; description
screen flagged zero rows) found
and fixed five real drifts in
`docs/MCP.md`, one of them
security-relevant: **the live-
MCP default was stated
backwards** — the doc said the
endpoint "defaults to on" /
"normally hosts from launch";
ground truth (`src/shell.rs`,
the W4 rule, the shipped help
copy) is OFF by default,
explicit-enable only. Also:
`get_clipboard_state` was
catalogued as a built-in (45
rows under a "(44)" header) —
it is a registry tool, moved
verbatim to its census
neighbour; the HTTP status
table lacked **403** (the
live transport's Host/Origin
DNS-rebinding refusal); the
error-codes table's header was
destroyed and its **-32700**
row lost — both restored from
the doc's own Protocol section
and the census; three splice
fragments repaired (joins and
capitalisation only). IA-Z6-1
applied verbatim. One IA
produced: **IA-Z15-1** — a stale
comment at
`src/rune_sec/help_panel.rs:306`
claims `live_mcp` "defaults to
`true`", contradicting the code;
comment-only, owed to the
integrator / that file's owner.
Recorded remainder: the sweep
ran against the base census
plus the findings filed by run
time; if a late seat adds a
tool or an argument, its docs
row rides its own IA and the
integrator re-runs this sweep
on the integrated tree.

**Z16 — i18n usage hygiene.**
The U10 pattern over 692 × 28,
and the headline is that the
catalogue is CLEAN: **zero keys
proposed for removal, zero
needing wiring.** The full
partition, machine-checked:
**531 live-direct** (a literal
inside a production `tr(` span,
span-matched — including all
twelve keys U10 filed as WIRE,
every one now wired), **154
live-dynamic** across five named
resolver mechanisms (7 workspace
keys via `Workspace::i18n_key()`,
4 menu titles via the indexed
`MENU_LABEL_KEYS`, 27 FX kind
labels via the name→key match,
8 Home count templates via
`count_label_tr`, 108 help texts
via the canonical-English
`help_text_key` lookup), **7
reserved** (six are S1's standing
user-decision discrepancies —
none has any `tr()` consumer,
refreshed audit — plus `fps`,
the documented Phase E literal),
**0 wire, 0 remove**. U10's 52
removed keys have not crept
back (parse-verified); zero
`value == key` cells and zero
`{slot}` mismatches across all
28 × 692 entries. The removal
IA list (IA-Z16-1…n) is empty
DELIBERATELY: deleting any of
the 161 non-span keys would
delete a key a resolver reads
by name, index or canonical
string — the exact failure the
no-grep-absence-alone rule
exists to prevent. The catalogue
stays as it is and the
integrator has NO i18n work
from this seat. (Count note:
this seat's bounded parse read
692; Z13's tolerant parse reads
693 — the `status_ready`
artefact, carried in Z13's
paragraph; the hygiene verdict
is unaffected, and IA-Z13-1
corrects the prose.) The post-T
English-placeholder translation
backlog is a translation matter,
not a usage defect — every such
key is wired or resolver-fed.

**Z17 — README truth pass 7.**
Eight edits to `README.md`
(+51/−18), every claim sourced
from its Y-finding, re-checked
against the landed tree, and
claimed on Wave Y's CLOSED CI
frame (round 7) — not on the
findings' word alone (the pass-6
discipline). Two stale counts
corrected: **390 → 392 tools**,
**678 → 692 UI strings**. New
claims landed: bend channels
(panel-keyed; stage / raster /
SVG follow; the arc at four
pieces), bend channels over MCP
(IA-Y2-1 discharged in F8 —
grep-verified, so §6.19
remainder (2)'s MCP edge does
not carry forward as owed),
media transforms, the strip
warp (two strips at gentle
bends, up to eight at strong),
layered import (including the
honest opacity limit, stated in
the README itself), setup pose
+ `set_rest_pose`, and alpha
APNG — claimed for the library
and MCP only, no panel claim
made or implied. Flags carried,
re-checked, none edited: F1
(CI-only framing), F2 (**no
LICENSE file** — the claim
rests on Cargo.toml's
`GPL-3.0-or-later` alone; still
the user's call), F3, F4, F9
(the class-C range sentences —
standing exclusion), F10 (the
absent panel alpha toggle, now
covering three formats).
Deliberately NOT claimed, each
tied to its Wave Z seat: bend-
channel media routing (Z1),
chain-spanning media (Z7,
design-only), layered-import
opacity storage (Z5), bundle
alpha (Z6), setup rest steppers
(Z4) — pass 8 revisits each
against its seat's CI verdict,
never its findings alone.

**Z18 — QA_RUNE alignment.**
One appended subsection,
**§19.9** (supersede-don't-
rewrite; no earlier text
touched; +49 lines): the Wave Y
closing frame aligned in QA's
idiom, every figure re-derived
first-hand — build totals
summed from the run's own log
(4,248 / 0), every headless
transcript read from the
downloaded artifact (census
PASS at 19 assertions, both
Home markers PASS, thumbnail
probe PASS with Y10's geometry
leg and distances 0 / 51 / 48,
MCP probe PASS), and the
overflow scan CLEAN as the
second consecutive. §19.8's
blocker is closed on GEOMETRY,
not just the verdict: the
sprite row still renders in
`tree-default.json` (it was not
hidden to pass) and
`sprite-z-front` sits on the
wrapped second line at right
edge 1194.5, against the
+14.0 pt breach Y15 measured.
§19.9 names this frame as the
basis for Z12's §10 upgrade
(IA-Z12-1 discharged) and
leaves §19.6's remaining rows
to their own waves' records.
(Relaunch note, for the honest
count: the first Z18 agent died
at startup on a rate limit and
produced nothing; this slice
started fresh.)

**Z19 — rehearsal support.**
The complete Wave Y CI frame
assembled from run records:
every round's build + headless
IDs, totals summed from
`test result:` lines (r4 1,754/1,
r5 3,978/3, r6 3,979/2, r7
**4,248/0** over 56 lines),
transcripts quoted from logs
and artifacts, and rounds 4–6's
artifacts downloaded fresh and
re-scanned (0 hits each). The
classification the ledger above
uses is this seat's: rounds 1–3
compile-only (no test target
ever executed), r4 test-side,
r5–r6 the one semantics thread
plus stale pins, r1–r2 headless
failures proven missing-input
from the logs. One method note
the next record-keeper needs:
the CURRENT scanner reads Wave
Y artifacts INCOMPLETE (1
required tree missing) — the
states tree entered SCAN_SET
only with Wave Z and cannot
exist in Wave Y dumps; the
comparable figure across
scanner versions is the hit
count, **0 in every Wave Y
dispatch that produced
artifacts (rounds 3–7)**.

**Z20 — this section** (§6.20
record + integration-eve
rehearsal, below).

**Integration-eve rehearsal**
(on the clean `git archive
5031700` tree — the shared tree
carries this wave's uncommitted
slices, so nothing was counted
there for the settled baseline;
Z16's and Z17's independent
runs agree). Settled counts
versus rehearsal: MCP **392** —
census exit 0, all PASS (44
builtin + 330 registry + 9 io +
4 subtitle_io + 5 subtitle),
gate check 9 doc-sync exact;
i18n **693 × 28** — Z13's
tolerant structural parse (693
entries per table; the 692
figures elsewhere, Z16's
partition and the frozen
headers included, are the
single-line-parser count that
misses the multi-line
`status_ready` tuple — Z13's
paragraph above carries the
method; IA-Z13-1 corrects the
prose); element keys
**1017** by the gate's method,
zero lost per F8's integration
check 2 on the git tree; palette
**135** (gate check 10 parity);
scenes **34** at base (35 on the
live tree with Z2's scene);
SCAN_SET **18 required / 0
pending** at base (19/0 on the
live tree after Z2's promotion,
Z14-verified). Script self-tests
on the archived tree: subtitle
census PASS, thumbnail probe
PASS (incl. Y10's geometry
fixtures), overflow scan
SELF-TEST all checks passed.
Failures and anomalies,
classified: **(a) Pending-IA
(expected mid-wave state; none
is a regression).** The Wave Z
integration pass owes:
**IA-Z5-1 (COMPILE-REQUIRED** —
the layered tool's reply field
follows Z5's report rename; the
tree does not compile until it
lands), IA-Z5-2 (the attach
schemas' + node-media shapes'
registry/docs mirror), IA-Z8-1
(the `export_bundle_progress`
catalogue key, ×28 — the
catalogue is measured again at
integration, never by
arithmetic), IA-Z11-1 and
IA-Z15-1 (stale comments, both
comment-only, no behaviour
depends on either), and
IA-Z13-1 (the 693 count prose
in the two frozen headers —
comment-only likewise). Z13's
floor ratchets themselves are
DONE in-tree (995 → 1017,
390 → 392, worktree-verified),
not owed. And the integrator
re-runs Z15's docs sweep on the
integrated tree if any late
seat moves the surface.
Discharged inside the wave, for
the record: IA-Z6-1 (by Z15),
IA-Z12-1 (by Z18), IA-Y17-2
(by Z2). Until the
integration pass lands, the
rehearsed Wave Z tree does not
compile — the green python
instruments are not a compile
verdict, exactly as §6.18 and
§6.19 said of their eves.
**(b) Regression (none open).**
The one regression standing at
§6.19's rehearsal — Wave X's
sprite Draw-order overflow
family — is CLOSED by this
frame: Y14's repair verified in
dispatches 42792705307 and
42798031914 (CLEAN twice, the
row present and wrapped, Z18's
geometry), and Z12's §10
citation now stands on the
closing dispatch. No Wave Z
audit found a new regression:
Z9 proved pin-coverage gaps,
not behaviour loss; Z10 built
nothing; Z14's contract check
isolated the pre-promotion
delta to exactly Z2's new tree.
**(c) Method artefacts (two).**
Gate check 2 on an archive tree
(no `.git`) cannot evaluate its
diff and reports 9/10; on the
git tree the same gate passed
10/10 at Wave Y integration
(F8) and in four mid-wave runs
reported by Z1, Z3, Z6 and Z14
over the shared tree. Not a
tree failure. And the current
scanner's INCOMPLETE on Wave Y
artifacts (Z19's annex): the
states tree postdates those
dumps; the hit count (0) is the
comparable figure. No other
instrument failure on the
rehearsed tree.

**Remainders.** (1) **The Wave Z
integration CI + its first
headless dispatch.** Everything
above is IMPLEMENTED until that
CI; the dispatch must produce
**38 dumps** with
`tree-states-populated.json`
among them, and the debrief scan
over that artifact must read
19/19 required present with its
hit verdict stated from the scan
output itself — never a
workflow-log grep. If the state
row overflows its card, the
repair belongs to the clips
panel's owner — no exemption, no
threshold move (the §6.19 (1)
discipline). (2) **The filed
IAs + three build remainders.**
The integration pass lands
IA-Z5-1 (compile-required),
IA-Z5-2, IA-Z8-1, IA-Z11-1,
IA-Z13-1 and IA-Z15-1 as
itemised in the rehearsal.
Riding with them: Z5's opacity
has NO panel control (stored
and MCP-writable, not
inspector-visible — a future
seat); Z8's bundles have no
cancel path (none exists in the
contracts; none faked); Z11's
Bend(0) channels are authorable
and inert at render (a refusal
is a board call, not an audit
fix), and its equal-absence
list (dopesheet channel marks,
keyframe-tools scope) is a
board call for ALL channel
targets, not a bend defect.
(3) **The docs-close round**
(5031700: build 42798924312,
headless 42798957551) was in
flight at this record's write
time; its verdicts — docs only,
no product behaviour — are
recorded by the next record,
not asserted here. (4)
**Chain-spanning media** (Z7):
the design is complete and the
seam verified; the build awaits
the user's answers to the
design's nine questions (Q1–Q9
in z7.md, defaults stated).
Nothing is improvised meanwhile.
(5) **Figure-library states**
(Z9): an inserted figure starts
stateless, now pinned as
deliberate; preserving states
requires the library to carry
the clips the keys index (or a
remap rule) — a format change
in the §6.7 class, awaiting a
user/integrator brief. (6)
**The limit preset sweep**
(Z10): no honest sweep exists
without role metadata; a
serde-defaulted joint-role
field on `NodeDef` is the exact
precondition — a model decision
for the user/integrator, not
built. Per-node presets (Y12)
stand. (7) **The panel alpha
toggle** remains the standing
user decision, unbuilt; bundle
alpha is now designed and built
MCP-side (Z6) with panel bundles
deliberately untouched, and Z6
also flags pre-existing drift
for the docs seat: the `render`
tool's SCHEMA `alpha`
description still omits APNG
although its description
sentence (Y13) includes it.
(8) **IA-Z15-1** — the stale
`live_mcp` "defaults to `true`"
comment at help_panel.rs:306:
comment-only, owed to the
integrator / file owner; the
behaviour and the docs (Z15)
are already correct. (9) All
standing user decisions from
§6.19's list stand untouched by
this wave: accent contrast,
panel splitter, File-menu Save
As, the two-"New project"
results, `tab_visits_controls`
OFF, project-swap undo, skew,
whole-figure pose transform,
MCP write_recovery, waveform
badge, GIF import, panel
export-scale control,
subtitle-stack cap/wrap, S1's
seven i18n discrepancies (six
surviving keys — Z16's audit
changes none of them), the
export class-C sentences
(machine-read), serialised
vocabularies in English, the
Settings-card Esc / menu-open
non-Esc key leak (PHASE_O §9)
and the figure-inspector
stale-selection copy (PHASE_O
§4) pending a product ruling,
the panel alpha toggle, and the
absent LICENSE file (Z17's F2 —
a flag, never an edit). Sound
remains user-verified only.
The SpriteLoop GAP list stays
exhausted except what user
briefs still own — chain-
spanning media (4) is now the
largest of those, fully designed
and priced.

**Wave Z CLOSED GREEN in CI round 4** (integration HEAD
`232f439`; build 42803905598 — 4,295 tests, 0 failed;
headless 42803945367 SUCCESS, transcripts verified:
SUBTITLE CENSUS PASS, HOME THUMBNAIL PROBE PASS, MCP
PROBE PASS, both Home markers present; artifact 38 dumps
with `tree-states-populated.json` among them, overflow
scan CLEAN — the fourth consecutive CLEAN dispatch, the
new state row inside its card). Rounds 1–3 failed only in
compile/test targets: r1 E0004 (the single-format worker's
match missed Z8's bundle kinds — the arms return a
dispatch error; bundle jobs run through their own
worker), r2 two test-module import gaps (graph types in
media's tests, GenericImageView in mcp_io's), r3 two
pre-existing media-shape pins that predate the stored
opacity field (updated to the IA-Z5-2 shape, `"opacity":
1.0`). No product behaviour changed in any fix round.
Settled counts at the close: MCP 392, i18n 694 ×28 (693
+ IA-Z8-1's key), element keys ≥ 1017 (floor), palette
135, scenes 35, SCAN_SET 19 required / 0 pending. Wave AA
(the panel catches up with the engine) was boarded
before this close and launches on this HEAD.

### 6.21 Wave AA — the panel
catches up with the engine:
inspector opacity, bundle
cancel, dopesheet channel
marks; the Wave Z frame closed
(2026-11-10)

Ordered 2026-11-10 (the standing
continuous order; the board was
written while Wave Z integrated,
so this wave launched the moment
Wave Z closed): twenty agents,
one owner per file, integrator
surfaces via filed actions
(board: `~/workspace/wave-aa/BOARD.md`;
findings:
`~/workspace/wave-aa/findings/`).
The board's base is the Wave Z
closing HEAD `0d32fa8` (MCP 392,
i18n 694 × 28 — Z13's corrected
693 plus IA-Z8-1's one key,
element keys at the 1017 floor,
palette 135, scenes 35,
SCAN_SET 19 required / 0
pending); `src/i18n.rs` and
`src/rune_sec/i18n_wire.rs` are
frozen to this wave's agents,
all key work arriving via IA.

**Record-time caveat.** This
section was first written in the
wave's first minutes, when no
sibling findings had landed;
before it closed, sixteen
sibling findings arrived (AA2–
AA12, AA14, AA15, AA17, AA18,
AA19) and their paragraphs
below were upgraded from board
summaries to findings digests,
with the frame corrected where
AA19's run-by-run assembly
superseded this seat's first
pass. The seats still out at
close (AA1, AA13, AA16) keep
their
**board summaries, marked as
such** — assignments and binding
constraints, not digests of work
done. Every Wave AA status is
IMPLEMENTED until Wave AA's
integration CI.

**The Wave Z integration
ledger.** Five slices on
`5031700`, in order: (1)
`fe1f183` — core: Z1's
bend-channel media routing, Z5's
segment-media opacity home
(serde-defaulted `opacity` on
`SegmentMedia::Image`, skipped
on write at 1.0 so opaque
projects serialise
byte-identically), Z11's bend
bake, Z9's library round-trip
pins. (2) `a245db1` — shell:
Z3's stage sprite dragging, Z4's
setup rest steppers, Z8's panel
bundle progress job (new element
key `xc-progress-bundle`), Z2's
states-populated scene, Z11's
bend readouts. (3) `4a4a47a` —
MCP: Z6's bundle alpha contract,
IA-Z5-1's layered-import report
rename (`attached_opacity_dropped`
→ `attached_opacity_stored`,
compile-required) and IA-Z5-2's
attach `opacity` argument,
IA-Z11-1's reduce comment. (4)
`7db7902` — i18n + infra:
IA-Z8-1's `export_bundle_progress`
key inserted into all 28 tables
beside `export_render_progress`,
IA-Z13-1's 693-count prose
corrections, floor ratchets to
1017 keys / 392 tools (Z13),
SCAN_SET 19 required / 0 pending
(Z2/Z14), IA-Z15-1's help comment.
(5) `f20a95a` — docs: Z17's
README truth pass 7, Z15's MCP
sync, Z12's PHASE_O §10 citation
upgrade, Z18's QA_RUNE §19.9,
Z20's §6.20 record. Gate PASS
10/10 and census clean at 392 on
the integrated tree, re-verified
by this seat on the clean
`0d32fa8` tree.

**The Wave Z CI frame — four
rounds plus two docs-close
rounds** (assembled from the
runs themselves by AA19 and
adopted here; this seat's
independent `gh` spot-checks
agree). **Round 1** (`f20a95a`):
build 42801198015 FAILURE — one
error, bin target only: E0004,
the single-format worker's match
missed Z8's bundle `JobKind`
variants; headless 42801233213
FAILURE at the same compile in
its own build step (no dumps
exist for it — a missing input,
not a probe defect). Fix
`6ebc4d6`: the bundle arms return
a dispatch error (bundle jobs
run through their own
plan-driven worker; the arm is
never reached). **Round 2**
(`6ebc4d6`): build 42802395799
FAILURE — 16 errors in the lib
*test* target only: Z1's media
tests used `Channel` /
`ChannelTarget` / `Key` without
the graph import, Z6's
bundle-alpha test called
`.dimensions()` without
`GenericImageView`. Fix
`2676132`: the two imports. The
round's headless (42802422399)
was **CANCELLED in its setup
step** when round 3 superseded
it — no headless verdict exists
for that tree, and none is
claimed (AA19's correction to
the interim record, adopted).
**Round 3** (`2676132`): build
42803084984 FAILURE — the lib
suite reported 1,771 passed / 2
failed (the step stops at the
first failing target; no
whole-run total exists), both
failures pre-existing media
tests pinning the node-media
JSON shape from before the
stored opacity field; headless
42803118202 SUCCESS — the first
green dispatch on a Wave Z tree,
38 dumps, the states tree's
first appearance. Fix `232f439`:
the two pins learn the IA-Z5-2
shape (`"opacity": 1.0`).
**Round 4** (`232f439`): build
42803905598 **SUCCESS — 4,295
tests, 0 failed** (log-summed);
headless 42803945367 **SUCCESS**,
transcripts verified (SUBTITLE
CENSUS PASS, HOME THUMBNAIL
PROBE PASS, MCP PROBE PASS, both
Home markers), 38 dumps with
`tree-states-populated.json`
among them, artifact overflow
scan CLEAN — the fourth
consecutive CLEAN dispatch.
**Docs-close rounds:** §6.20's
remainder (3) is settled — the
`5031700` pair both SUCCESS
(build 42798924312: 4,248 passed
/ 0 failed, identical to the Y
close as a docs-only delta
predicts; headless 42798957551:
all fail-soft PASS, 37 dumps —
the 38th is the states tree,
which lands exactly at round 3).
The `0d32fa8` close pair (build
42804787786, headless
42804827398) was in flight at
this record's write time; its
verdicts belong to the next
record. No product behaviour
changed in any fix round — every
failure sat in a compile or test
target, round by round.

**AA1 — inspector media-opacity
control: BUILT** (findings
digest, reconciled at
integration). §6.20 (2)'s panel
reach closed: the Media
section's transform block gains
a fifth stepper row — Opacity
(`seg-media-opacity-dn`/`-up`) —
viewing and editing Z5's stored
field through the new
`adjust_media_opacity` op
(validate → one snapshot →
mutate → status: exactly one
undo step per effective press,
clamp presses silent). The
sanitisation is the funnel's
own rule (`sanitise_media_opacity`:
non-finite → 1.0, clamped
0..=1), so the row shows exactly
what renders, and a changing
press repairs a corrupt store.
Deliberately NOT folded into
`MediaTransformState`: Reset
must spare a stored opacity
(Z5's preservation guarantee) —
a pin proves it. Nine new tests.
IA-AA1-1 landed at integration:
`segment_media_opacity` /
"Sprite opacity" inserted in
all 28 tables after
`segment_media_rotate` — the
catalogue is **695 ×28** from
this wave. This is the
stored-field control, **not**
the panel alpha toggle (a
standing user decision,
untouched).

**AA2 — panel bundle cancel:
AUDITED — NOT BUILT.** The
board's condition failed on the
evidence: the W1 mechanism is
one-way by design — both
channels run worker → UI, the
worker handle is discarded at
spawn, the core progress
callbacks return `()` and cannot
stop the member that is running
(or reach the hook-less SVG /
spritesheet members at all). The
one in-file improvisation (a
flag checked between members)
fails the board's own terms
three ways: it skips only
unstarted members, it has no
honest "cancelling…" state to
show under the status-written-
once rule, and a member skipped
at the user's request fits
neither bucket of the class-C
recap — writing it under
"failed:" would be a known
falsehood in the record. So no
control was wired and no string
filed; panel/MCP parity holds by
both sides having no cancel. A
real contract needs a core stop
signal, a class-C ruling on a
third recap bucket (the user's),
and a readout rule — filed as
remainders, not improvised.

**AA3 — dopesheet channel marks:
BUILT.** Z11's equal absence is
closed at the sheet: NO graph
channel of any target marked the
dopesheet (pose rows derive from
stored poses, and a channel key
writes no pose — a figure
animated purely from the Graph
panel left no trace). The new
derivation takes the frames
holding a key in ANY channel of
the figure — RootX, RootY,
Angle, Length, Bend at parity —
through `graph_key_frame`
imported from the marker row
(never re-typed, so sheet and
marker row cannot disagree on
which frame a key sits on;
fractional keys round, stale
channels mark nothing), and the
cell renders the marker row's
own `●` in its graph colour,
composing losslessly with the
pose/state glyphs (all three
kinds visible at once; lone-mark
cells byte-unchanged; no new
element keys — scripted 3/4 →
3/4). A channel-keyed but
unposed figure now gets rows
instead of the empty state. Six
pins. The mark is per-frame and
target-blind by design — the
sheet says a channel turns here;
naming targets stays the Graph
panel's job.

**AA4 — keyframe-tools channel
scope: the skip is BY DESIGN,
now documented and pinned.**
The audit cleared every other
surface first-hand: the
graph_keys library addresses any
channel (by index or by
(figure, target)), its MCP
handlers likewise, and the graph
panel's delete/shift act on the
DRAWN channel — any target. The
one skip is the keyframe-tools
SECTION, whose single-target ops
resolve (selected figure, Angle
(selected node)). That scope is
its Phase D charter, restated by
every later addition, ruled on
by the Wave 10 scope-honesty
audit, and stated in situ by its
own button ("Key angle here");
the generic reach sits adjacent
by construction (the Graph panel
mounts directly above it). The
considered widenings break real
semantics (one-channel clipboard,
per-channel collision rules,
traced-write-set honesty), so
nothing was widened: the module
docs of both files now carry the
scope rule and its audit trail,
and one pin proves the section's
ops never touch the joint's Bend
or Length channels. Scope
changes only by a future board
call.

**AA5 — Bend(0) authoring call:
BUILT.** §6.20 (2)'s call lands
as a refusal, and the refusal is
the codebase's own: frame bends
on the origin are already
refused everywhere with one
sentence ("The origin has no
segment to bend." — panel
`cycle_bend`, the inspector
steppers, MCP `set_bend`), and
the two bend-capable graph
doors were the leftover hole.
Panel `key_bend` and MCP
`tool_graph_key` now refuse
node 0 with that exact sentence
before any snapshot — no phantom
undo step, both `"bend:0"` forms
refused, `bend:1` untouched.
Saved projects cannot break:
no load/save path validates
channel targets, an existing
Bend(0) channel stays selectable,
movable, deletable and
transformable — only authoring
NEW keys stops. One pin per
door. Angle(0)/Length(0) are
equally inert and stay
authorable — a different board
call (remainder). IA-AA5-1
(docs sentence) was applied
verbatim by AA15.

**AA6 — render schema alpha
drift: DONE, one line.** §6.20
(7) closed: the schema now names
'apng' beside 'png_sequence' and
'spritesheet'. Two corrections
ride with it: the `render` tool
lives in `src/mcp_io.rs` (io
family), not `src/mcp_tools.rs`
as the board phrased it — the
edit went to the schema's actual
home, in the seat's declared
region spirit — and docs/MCP.md
needed no edit: its render row
already carried the APNG-inclusive
sentence (the docs were ahead of
the schema, not behind it). The
prose side was truthful all
along: `tool_render` dispatches
`apng` + alpha to the alpha
encoders. Census re-verified by
the seat: 392, 16/16 PASS.

**AA7 — states-populated
overflow verification: VERIFIED
CLEAN.** §6.20 (1)'s conditions
met in full on the closing
dispatch (42803945367), measured
from a fresh artifact download:
38 dumps, `tree-states-populated.json`
among them, the scan over the
artifact reading 0 hits on every
dump and "CLEAN (0 hits, scan
set complete)" — 19/19 required.
The state row sits inside its
card: the states section rests
12.0 inside the card on both
sides with 128.0 of card below,
its right edge 0.5 inside the
1412.5 scan edge; the row is
full-width flush with the
section by design (+0.00, never
past), the tightest stepper
holding 24.5 of headroom. The
clips-panel repair contingency
does not trigger; AA12 was free
to cite this frame.

**AA8 — sprite-drag stage polish
audit: one gap built, three
cleared.** The proven gap was
accidental drag: any pointer
travel, however small, committed
a real key — one or two units of
hand jitter spent an undo step
and, at a keyless frame,
materialised a stored key the
user never meant to author. The
gesture gains a press-relative
jitter gate (`DRAG_THRESHOLD`
2.0 stage units): below it
nothing is written at all (the
subtlety the subtitle lane's
lazy snapshot already practised);
crossing it applies the full
press-relative delta, no jump,
no subtraction drift. Two pins.
The other probes cleared on
evidence: fractional frames
cannot underlie a grab (the
fractional display exists only
while playing, and arming is
refused while playing); playback
starting mid-gesture leaves the
project coherent and no sibling
gesture guards it either, so a
sprite-only guard would diverge;
the overlap pick is taken once
at press and never re-picked, so
nothing can steal a gesture.

**AA9 — rest-stepper + drag
coherence audit: COHERENT, with
one seam filed as a decision.**
Every `rest_angle` writer was
enumerated tree-wide. Stepper
and Setup drag provably end at
the same composition
(`clamp_angle` after `norm_deg`,
the drag applying it inside the
solve and again, idempotently,
at write-back); panel set-rest
and MCP `set_rest_pose` are
structurally one op (one core
implementation, identical
validation and snapshot
accounting) — parity cannot
drift without a second
implementation being written.
The one real disagreement:
`set_rest_from_pose` normalises
but does NOT clamp, so a pose
captured after limits were
tightened stores a rest outside
its own arc, and the next
stepper press is direction-blind
until the value re-enters.
Unconsidered seam, not design;
the fix lives in model.rs and
its direction is a genuine
product call, so it is filed as
**IA-AA9-1 (DECISION)** —
remainder (8) below — not built
unilaterally. Nothing else
built; the seat touched no file.

**AA10 — bundle-job progress
truth audit: NO MISMATCH; the
one unpinned seam now pinned.**
The three surfaces tell the same
story by construction — one
shared core tracker
(`BundleProgress`: allocation
before the first member, clamped
monotonic advances, silent
banking, the terminal fired iff
every member succeeded), verbatim
field mappings at both hops
(panel slot, MCP wire), and
manifest alpha flags matching
the (scale × alpha) routing
member for member, recorded on
success and error objects alike.
Every candidate skew (member
order, banking divergence,
failed-member terminal, MP4's
double phase, alpha video) was
checked and cleared against the
X11/Z6/Z8 pins. The one seam no
pin covered — the manifest's
per-member FRAME counts, alpha
vs opaque — gains two pins
(images and video) in mcp_io's
tests; no production code
changed. One observation, not an
IA: the totals arithmetic exists
twice (core-inline and the
panel's helper), each side
independently pinned to the same
values, so a drift fails a pin
before it can desync the story.

**AA11 — media-opacity surface
sweep: CONSISTENT; the drop is
total; two doc sentences
built.** There is exactly one
media-shape emitter
(`segment_media_json`) and every
data surface routes through it —
`get_node`, `clip_get_node_media`,
`clip_list`, both attach replies,
the clear reply — with the
registry and docs enumerations
in step (IA-Z5-2 verified in
both files), attach replies
firing on opacity alone, and the
layered-import report carrying
`attached_opacity_stored` into
the MCP reply under its own key.
Raw reads report the stored
value unsanitised by house style
(the funnel sanitises at
render); the panel label names
identity, not payload — its
silence is its standing shape,
and the missing panel stepper is
AA1's build, not duplicated
here. The .stk/.piv drop was
verified total field by field:
the shared figure writer never
reads `node.media` at all, and
v5 — which carries the media's
identity and bytes — applies
NONE of the five payload fields
(nothing baked into blob alpha
or anchors). The two proven
gaps were documentary: the drops
were total in code but unstated
where a reader looks, so the
exporter docs gained the
sentences.

**AA12 — citations: the §10
upkeep — TAKEN.** PHASE_O §10's
citation now stands on the Wave
Z closing dispatch (42803945367),
by Z12's method with the
measurer calibrated first-hand
against the previously cited
artifact (every per-tree count
and O-1…O-9 edge reproduced
before the new frame was
trusted). The quietest upgrade
in the table's history: on the
new artifact **no shared number
moved** — all 37 shared trees
reproduce exactly, the O-1…O-9
streak reaches seven consecutive
dispatches, the z-row stands
(front 1194.5, subtree 1412.0),
and the `tree-default` key diff
is +0/−0. The table gains its
one growth row — the states
carrier (1306 elements, 0 hits,
required under the 19-tree set)
— and a corollary is recorded:
rescan the previously cited
artifact under the current
contract and it reads INCOMPLETE
for exactly one reason (it
predates the scene) — a
provenance gap, not a hit.
IA-AA12-1 filed for the QA
pointer; AA18's §19.10 carries
the matching fact.

**AA13 — gate/census
maintenance: DONE** (findings
digest, reconciled at
integration). Clean detached
worktree of `0d32fa8`: gate
PASS 10/10, census 392, every
count structurally verified —
element keys **1019** (889
static + 130 dynamic; the delta
vs `5031700` is exactly Z4's
two rest-stepper patterns, zero
lost; `xc-progress-bundle`
exists in source but rides
outside the counter as a
tuple-carried key, the standing
F6 artefact — AA17's 1018 was
the gate's check-1 line on the
moving tree before every seat
had landed), i18n **694 ×28**
at the baseline (695 after
IA-AA1-1), palette 135, scenes
35, SCAN_SET 19/0. Floor
ratchet landed: `KEY_TOTAL_FLOOR`
1017 → **1019**, proven in the
clean worktree before it was
applied to the live file;
`MCP_SURFACE_FLOOR` stays 392.
The header drift AA17 recorded
(693 against the gate's count)
was discharged at integration
with IA-AA16-2: both frozen-file
headers now read 695.

**AA14 — overflow/headless
maintenance: VERIFIED, no
edits.** The machinery survived
integration exactly to spec,
re-derived two ways (module
import and CLI, set-equal):
SCAN_SET **19 required / 0
pending**; `scenes()` 35; both
headless loops 35, set-equal to
the registry, to each other, and
each individually producing
every required tree; 35 loop
trees + 3 plain-loop-only 720p
literals = the **38 dumps** the
closing dispatch in fact
produced; self-test all green,
no stale counts in either owned
file. The boundary is stated for
the record: AA14 owns the
instrument and the set, AA7 owns
the artifact verdict, AA12 owns
the citation — a CLEAN line
cited downstream can be trusted
because the instrument was
checked, not assumed.

**AA15 — MCP docs sync: sweep
clean, one sentence applied.**
The escaped-pipe-aware sweep
against the settled census:
**392 doc rows, one per census
tool** — zero duplicates, zero
missing, zero phantom, zero
misplaced; the description
screen flagged no row; Z15's
Wave Z repairs all intact. The
Required / Optional columns were
re-derived from source for every
Wave Z changed row (Z15's
remainder, discharged): both
attach rows carry `opacity`,
`export_bundle` carries `scale` /
`alpha`, both media-shape
enumerations name `opacity`, and
the layered-import rename is
closed on the docs side (the
old name survives only as a
historical comment). IA-AA5-1
applied verbatim — the
`graph_key` row gains AA5's
bend:0 refusal sentence — and
AA6's schema-side fix needed no
doc action. No IAs produced; the
standing note stands (remainder
(10)): re-run this sweep on the
integrated tree before close.

**AA16 — i18n usage hygiene,
second pass: DONE, zero
removals** (findings digest,
reconciled at integration).
Structural parse of the
694-key baseline: 532 live-direct
(the 532nd is
`export_bundle_progress`, wired
at Wave Z integration), 154
live-dynamic, 7 reserved, **1
wire**, 0 remove. The wire key
is `status_ready` — a correction
to the record: it has never had
a consumer (zero `tr()`
credits, in no resolver
mechanism); its English value
is displayed constantly as the
core's initial status string,
rendered raw, so it classifies
WIRE under the standing bar,
not REMOVE — §6.20's implied
live-direct count for it does
not stand. IA-AA16-1 (wire it /
remove it / reserve it) was
decided at integration for the
seat's recommended option C:
**reserved as kept-by-decision**
— no code change; the user may
revisit. IA-AA16-2's header fix
landed with IA-AA1-1's (695).

**AA17 — README truth pass 8:
done, nine edits.** Every Wave Z
surface is now claimed on its
closed CI frame, each claim
sourced from its Z-finding and
re-checked against the landed
tree: the string count corrected
692 → **694** (gate check 4's own
structural count), bend-channel
media and bend polish in the
Timeline passage, sprite dragging
and rest steppers in Import /
Posing, stored layered opacity —
claimed for the attach tools and
layered import only, **no panel
claim** (AA1 is pass 9's claim) —
bundle alpha scoped to MCP with
panel bundles stated opaque, the
panel bundle jobs on W1's
mechanism, and the states scene's
coverage in Movieclips. All prior
flags carried verbatim (LICENSE
still absent — F2 stands on
Cargo.toml alone) and the
deliberate non-claims listed:
no panel opacity control, no
bundle cancel, no chain-spanning
media, no library states, no
limit sweep. No IAs.

**AA18 — QA_RUNE alignment:
§19.10 appended.** The §19 line
now stands against Wave Z's
closing frame, X15/Y15/Z18's
method, every figure re-derived
from the run log or the
downloaded artifact: 4,295
log-summed, 38 dumps with the
states dump genuinely populated,
every transcript PASS with its
load-bearing numbers, the scan
CLEAN as the **fourth
consecutive** — with the chain's
third link (42803118202)
re-verified first-hand in this
pass, links 1–2 standing on
Z12/Z18's verification.
**IA-Z12-1 is consumed and
discharged**: PHASE_O §10's
upgrade landed in Wave Z, §19.10
records it as fact, and
IA-AA12-1's matching pointer is
carried by the same entry —
§19.10 names this frame as the
newest CLEAN frame and records
§10's advance to it (taken by
AA12 in this wave). The §10
advance itself was AA12's call;
this seat took none of it.

**AA19 — rehearsal support: the
frame, from the runs.** Every
status, total and transcript in
the frame above was read from
the runs' own logs, nothing
transcribed from §6.20 or the
close note — and where the
interim record disagreed, the
runs won: the round-2 headless
was **cancelled in setup**, not
failed (the frame now says so).
Log-summed totals throughout:
4,248 (Y docs-close), 4,295 (Z
close); the 37 → 38 dump step
located exactly at round 3,
where the states tree first
exists; round 3's lib-suite-only
total stated as such rather than
inflated into a whole-run figure.
The `0d32fa8` docs-close pair
was still in flight at its write
time too — remainder (3) stands
for both records.

**AA20 — this section** (§6.21
record + integration-eve
rehearsal, below). First written
at launch from the board, the
git history and first-hand `gh` /
instrument checks, then revised
in place as sixteen sibling
findings landed — each upgraded
paragraph names its seat's own
verdict, and the three seats
still out remain marked board
summaries under the caveat
above, for the integrator to
reconcile (remainder (2)).

**Integration-eve rehearsal.**
Two readings, honestly labelled.
**Baseline** (this seat, on the
clean tree at `0d32fa8` before
any seat diff existed): gate
`phase_j_checks.py` **PASS
10/10**; `mcp_census.py` clean —
**MCP 392, palette 135**.
**Settled counts**, since
measured by sibling instruments
on this tree and mutually
consistent: element keys **1018**
(gate check 1, AA17 — the 1017
floor plus `xc-progress-bundle`),
i18n **694 × 28** (gate check 4,
AA17; AA14's SCAN_SET 19/0 and
scenes 35 re-derived two
independent ways), census **392**
(AA15 twice — clean base and
landed tree; AA6 once; this seat
and AA10 on the shared tree).
**Moving-tree readings** (the
Z14 caution applies — they
measure a tree other seats were
still editing): AA10's filing
read gate + census PASS over the
shared tree carrying every
in-flight AA diff; this seat's
closing census read likewise
clean. No count dropped against
the baseline in any reading.
Per-seat diff comparison: the
sixteen landed seats' diffs sit
in their declared files/regions
per their findings (AA2/AA9/AA14
deliberately empty; AA6 one line
in mcp_io.rs; AA15 one docs
sentence; AA5 two regions + two
pins), and AA15's census-side
sweep over the same tree found
zero missing/phantom/misplaced
rows — the comparison the
integrator must still re-run
seat by seat on the integrated
tree, against these counts, not
against expectations.
Classification rule for
integration, stated so it cannot
drift: a Wave AA failure is
**pending-IA** if it disappears
when the filed IAs are applied
exactly as filed — IA-AA5-1
already applied verbatim by AA15
is this class's discharged
example; the live cases are
AA1's inspector string reaching
`src/i18n.rs` + the wire through
the integrator, and AA16's
removals, which exist only as
IAs — and a **regression**
otherwise, including failures in
files no seat owned, count drops
against the settled counts
above, and any class-C sentence
altered. IA-AA9-1 is **neither**:
a decision IA (model.rs
semantics) that produces no
failure either way until someone
rules on it. At this record's
close there are **zero failures
to classify**: no integration
run exists yet. A paragraph
above marked "board summary" is
never evidence in the
integrator's comparison — the
seat's findings and diff are.

**Remainders.** (1) **The Wave AA
integration CI + its first
headless dispatch.** Everything
above is an assignment or a
baseline until that CI; the
dispatch must produce **38
dumps** with
`tree-states-populated.json`
among them, and the debrief scan
over that artifact must read
19/19 required present with its
hit verdict stated from the scan
output itself — never a
workflow-log grep. Overflow
repair, if any, belongs to the
owning panel seat — no exemption,
no threshold move. (2) **This
record's remaining board
summaries.** Sixteen seats were
reconciled against their landed
findings before this section
closed (AA19's log-summed totals
were adopted into the frame,
including its round-2
cancellation correction). The
three still marked "board
summary" (AA1, AA13, AA16) must
be reconciled by the integrator
when their findings land; where
a finding contradicts its
paragraph, the finding wins and
the discrepancy is recorded in
the next section, not silently
patched here. (3) **The
`0d32fa8` docs-close round**
(build 42804787786, headless
42804827398) was in flight at
this record's write time; its
verdicts — docs only, no product
behaviour — are recorded by the
next record, not asserted here.
(4) **Chain-spanning media**
(Z7): the design is complete and
the seam verified; the build
awaits the user's answers to the
design's nine questions (Q1–Q9
in z7.md, defaults stated).
Nothing is improvised meanwhile.
(5) **Figure-library states**
(Z9) and **the limit-preset
sweep** (Z10) still await user
briefs — the clips-in-library
format change and joint-role
metadata respectively. Per-node
presets (Y12) stand. (6) **The
panel alpha toggle** remains the
standing user decision, unbuilt;
AA1's stored-opacity control is
a different field and does not
prejudice it. Bundle alpha is
MCP-side (Z6) with panel bundles
progress-only (Z8), and **panel
bundle cancel is now settled as
not built** (AA2's audit): it
becomes buildable only with all
three of a core stop signal in
the progress contracts (threaded
through export.rs, svg, the
spritesheet entry and the MCP
driver), a class-C ruling adding
a cancelled bucket to the bundle
recap (the user's — the standing
exclusions forbid the seat from
rewording it), and a readout
rule for pressed-but-not-yet-
effective. Until then "no cancel
path exists in the contracts,
and no fake button was added"
remains the true sentence.
(7) All standing user decisions
from §6.20's list stand untouched
by this wave: accent contrast,
panel splitter, File-menu Save
As, the two-"New project"
results, `tab_visits_controls`
OFF, project-swap undo, skew,
whole-figure pose transform, MCP
write_recovery, waveform badge,
GIF import, panel export-scale
control, subtitle-stack cap/wrap,
S1's surviving i18n discrepancies,
the export class-C sentences
(machine-read), serialised
vocabularies in English, the
Settings-card Esc / menu-open
non-Esc key leak (PHASE_O §9) and
the figure-inspector
stale-selection copy (PHASE_O §4)
pending a product ruling, and the
absent LICENSE file (a flag,
never an edit). Sound remains
user-verified only. The SpriteLoop
GAP list stays exhausted except
what user briefs still own —
chain-spanning media (4) is the
largest of those, fully designed
and priced. (8) **IA-AA9-1 — the
set-rest clamp decision.** Whether
`Project::set_rest_from_pose`
clamps its capture through joint
limits is an integrator/user
ruling (model.rs): clamp and all
three rest writers share one rule,
at the cost of set-rest no longer
reproducing an out-of-arc pose
exactly; or keep norm-only as a
deliberate extension of "limits
bind solvers, not stores", with
the direction-blind stepper press
standing as its known symptom.
AA9's findings carry the one-site
diff and the pin for the YES
branch; no panel or MCP change is
needed either way. (9) **The
`src/i18n.rs` header prose.** The
header still reads "the same 693
keys" while the gate's structural
count is 694 — IA-Z13-1's prose
was written before IA-Z8-1's key
landed (AA17's observation). The
file is frozen to agents; the
one-line correction (693 → 694)
is the integrator's, alongside
any Wave AA key work. (10) **The
MCP catalogue's standing
verification debt** (AA15, after
Z15): the census + escaped-pipe
sweep must be re-run on the
integrated tree before the wave
closes, and the Required /
Optional columns — re-derived
this wave only for the changed
rows — still await an exhaustive
schema-by-schema pass over all
392 rows; neither is a defect,
both are how the catalogue stays
honest.
