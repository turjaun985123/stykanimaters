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
