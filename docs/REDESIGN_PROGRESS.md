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
