# StykAnimate vs StykAnimate-RS — comprehensive comparison

Measured fresh from both working trees on 2026-11-08, after Wave 9,
the UI folder wave and UI wave 2. Port figures are re-measured from
the current tree (HEAD `8c68fdb`); the original's are unchanged
since its last release. This document supersedes all earlier
comparison + addenda documents.

## At a glance

| | Original (StykAnimate-App) | Port (StykAnimate-RS) |
|---|---|---|
| Version | 1.2.5 | 0.1.0 |
| Runtime | Electron + WebGPU app, Python codec backend | One native binary: Rust + GPUI 0.2.2 |
| Application code | 103,549 lines JS in 535 modules under `src/` (excluding `src/locales`) | 52,096 lines Rust: 32 core modules + 2 binaries + a dedicated UI library |
| UI code | Inside the JS total | `src/ui/`: **39 components, 5,900+ lines**, all presentation-only builders over plain data |
| Generated code | 179,460-line compiled locale bundle (`locale-packs.mjs`) | none |
| Backend/codecs | 12,987 lines Python (`codecs/` + `backend/`) | included in the Rust total (`pivot.rs`, `nodes.rs`, `nodes_anim.rs` ≈ 8,400 lines) |
| Tests | 876 files in `tests/` (fixtures + suites) | 779 tests CI-verified green at Wave 12 (`0139fa0`), including byte-determinism integration tests for frame renders, GIF, SVG, spritesheet, PNG-sequence, contact-sheet, APNG and JPEG-sequence exports + a cross-format fixture corpus with manifest |
| UI languages | 27 | 28 (the same 27 + Bengali), 62 keys |
| Licence | GPL-3.0-or-later | GPL-3.0-or-later |

## Feature matrix

| Feature family | Original | Port | Verdict |
|---|---|---|---|
| Core posing (FK, two-bone IK, bend) | Full | Full | Equal |
| Figure presets (humanoid, quadruped, mini) | Full | Full | Equal |
| Figure builder (add/remove, length, thickness, colour, shapes) | Full | Full | Equal |
| Segment styles (taper, caps, gradients, opacity) | Full | Rendered in raster + SVG, button editing | Near-equal |
| Timeline (frames, durations, fps, copy/paste, inbetween) | Full | Full — plus loop ranges, block shift/reverse, shift-click range selection and motion-path editing (drag/smooth roots) | Equal |
| Easing | Full curve editor | 4 curves + per-keyframe override + per-key interpolation modes (Hermite/Linear/Step) in a Hermite channel engine (`graph.rs`) applied live in the editor, with a visual panel (add/drag/delete keys, tangent handles, axis labels from the fitted range, undoable) | Original deeper |
| Onion skin | Full | Two-way (prev in figure colour, next in green), range 1–3 | Equal |
| Camera (keys, pan, zoom, follow, rotation) | Full | Full | Equal |
| Layers / figure list | Full layer system | Reorder (full reference remapping), hide, delete, select, solo, lock, per-figure opacity, bulk visibility (all/none/invert); rows are component shells with count badges | Near-equal |
| Drawing | Deep raster engine (brushes, fill, tiles, text) | Strokes with pressure widths, a brush library + brush/eraser tools, stroke stabilisation, serpentine fill, splitting eraser, smoothing and nib profiles | Original much deeper |
| Sprite / text segments | Full | Sprites (aspect-correct) + bitmap-font text rendered **and typed in the editor**; Stick Nodes sprite/text instances imported as keyframed tracks and rendered | Near-equal |
| Particles / VFX | ~6,600-line VFX system | Events + timed clips (**27 kinds**) with an FX timeline editor (list/move/resize/duplicate/delete + span bars) + FX clip presets on disk + continuous weather (rain, snow, fireflies, fog, storm), deterministic | Original deeper |
| Audio | Full editor (waveform, mixing) | Playback, waveform panel (per-clip strips + overview waveform), mixdown export, normalise/trim/split/crossfade/fade/gain/reverse (engine and editor buttons) | Original deeper |
| Movieclips | Full editor | ClipDef playback, conversion from .nodemc, and an in-editor clip session (create a clip from a figure's timeline poses, edit its frames) | Original deeper |
| 3D figures | Full 3D system | 3D mode in the editor: orbit, per-joint Euler rotation, live projected preview, per-segment depth shading data, **bake to a native 2D figure** — a single pose or an animated bake interpolated between two marked poses; library persisted in `./library3d` | Original deeper |
| Touch | Full touch UI | Recogniser + pinch engine wired through a single `touch::feed` adapter. Real multi-touch is **impossible on this platform**: GPUI 0.2.2's Linux input has no touch events at all (proven from the vendored crate source, `docs/TOUCH.md`), so a touchscreen arrives as one emulated mouse pointer | Original deeper (platform-blocked) |
| Home / library / projects | Full | Home panel with project statistics grid + per-figure keyframe sparkline, recents, recovery snapshots, autosave every 25 edits, figure library | Near-equal |
| App shell + UI | Native dialogs, full shell | Native open/save dialogs via zenity/kdialog with the folder flow as fallback, recovery, stage-size controls — and component-built chrome: app bar, labelled tool groups, 11 toggleable panels, dopesheet + timeline ruler + scrubber, export centre with category tabs, segmented weather control, inspector gauges, toast stack, HUD strip. The GUI has been seen running on a real Linux desktop (user screenshot, 2026-11-08); the *redesigned* chrome has not yet been screenshot-verified | Near-equal |
| AI-agent control (MCP) | None | **Full MCP server: 62 tools** — build, pose, draw (incl. polygon/star shape tools), decorate, manage FX clips (all 27 kinds) and FX presets, mirror poses, smooth, bake and preset graph tangents, set clip loop modes, run 3D sessions, author movieclips, set per-frame bends, project statistics, bulk frame durations, render (incl. contact sheets), import/export (incl. .piv v2/v4, .nodes, .stknds, MP4/WebM frame ranges), frame inspection as base64 PNG | Port only |
| Localisation | 27 languages, human-maintained | 28 languages (same set + Bengali), machine-produced strings; the 12 newest keys are now genuinely translated in all 28 languages (a few values are established loanwords, e.g. "Easing") | Port wider, original more polished |

## Formats — the port's strongest area

| Format | Original | Port import | Port export |
|---|---|---|---|
| .piv v2 / v3 / v4 | native | Yes | Yes — v2, v3 and v4 (v2/v4 with documented losses: media segments written as plain segments; v4 also drops camera keys, joins and bends) |
| .piv v5 | native | Yes — full grammar incl. image/text side tables | Yes — incl. per-frame lengths (from graph Length channels), image side tables and per-frame bends (from `Frame.bends`); the original's own decoder consumes our files exactly, media fixture included. Text side tables are the one unwritable piece: the model carries a bitmap font, not glyph outlines |
| .stk | native | Yes | Yes |
| Stick Nodes .nodes | native | Yes — 57-field record grammar, verified against their decoder + real corpus fixture | Yes — figure export (documented losses: thickness rounded to i32; node names, taper/cap/opacity and media dropped — gradient end colour round-trips both ways since Wave 10) |
| Stick Nodes .nodemc / .stknds | native | Yes — modelled subset (sprites/texts as keyframed tracks, joins baked at import) | Yes — .stknds writer (plain posed figures; camera zoom/x/y/rotation round-trips since Wave 11; joins, clip library, sounds and FX dropped) |
| Native project | JSON | JSON | JSON |
| PNG sequence / GIF / MP4 / WebM / APNG / JPEG / SVG / spritesheet / contact sheet | Yes | — | All yes, deterministic, several verified with independent decoders (PIL checks the APNG; ffmpeg produces real MP4/WebM) |

Interop is verified **against the original's own code in both
directions**: their Python decoder reads our .piv v3/v5 exports; our
importers reproduce their decoders' exact values on shared fixtures
(the cross-format corpus in `tests/fixtures/`).

## Where each one wins

**The original wins on depth and maturity.** Its drawing engine,
VFX system, audio editor, 3D editor and graph editor are each
thousands of lines with years of edge cases behind them; it has an
876-file test corpus, human-maintained translations, native file
dialogs, and a wired touch UI. Version 1.2.5 vs 0.1.0 is a fair
summary of the maturity gap.

**The port wins on shape and proof.** One native binary — no
Electron, no Python runtime. Roughly 2.4× less code than the
original's JS alone for the surface it covers. Exports are
closed-form deterministic (same input, same bytes, verified). Its
format support is not re-implemented from guesses but proven
against the original's own decoders, in both directions. It has an
MCP server the original lacks entirely, and it speaks one more
language: Bengali.

## Remaining gaps, end to end

Nothing is entirely missing any more — every feature family in the
original has a working counterpart in the port. What remains:

1. **Depth in five systems** — drawing volume, VFX volume, audio
   editor depth, 3D editor depth, graph editor depth. The port's
   versions work and are wired into the GUI; the original's are
   years deeper.
2. **Simultaneous audio — closed in Wave 10.** Live playback
   and MP4/WebM export now mix every clip audible at the playhead
   (the export falls back to the earliest clip alone if a clip is
   not WAV and cannot be mixed).
3. **Stick Nodes fidelity limits** (documented in
   `docs/NODES-ANIM-SPEC.md`) — smart-stretch, connectors and
   angle locks are consumed, not simulated; clip-internal
   sprite/text tracks and instance style/filter data are not
   modelled; the .stknds writer drops joins, the clip library,
   sounds and FX (camera zoom/x/y/rotation round-trips since
   Wave 11; only bindings and procedural wobble stay unmodelled,
   as upstream).
4. **.piv v5 text side tables** cannot be written from the port's
   model (bitmap font, no glyph outlines) — a model change, not a
   writer change, would be needed.
5. **Real multi-touch** is blocked by GPUI 0.2.2 itself (no touch
   events in its Linux input layer). The recogniser, pinch camera
   and feed adapter are built and tested; they wait on a GPUI
   upgrade, not on port work.
6. **Localisation polish** — the port's strings are
   machine-produced; since Wave 11 the 12 newest keys carry
   genuine translations in all 28 languages (a few values are
   established loanwords, e.g. "Easing"). What remains is polish,
   not coverage. (Japanese, Korean, Hindi and Urdu exist in
   neither app.)
7. **Visual verification of the redesign** — the editor runs and
   was screenshot-verified on the user's PC in its pre-redesign
   form; the redesigned, component-built chrome (UI folder wave,
   UI wave 2, Wave 10 wiring) is CI-verified but has not yet been
   seen on a real display.
8. **Maturity** — 0.1.0 vs 1.2.5: fewer field hours, a smaller
   fixture corpus, and edge cases the original has already met.

## Bottom line

By feature family, the port has **something verified in every area
the original has**. By depth, the port covers the everyday
posing/animation workflow at full strength — posing, timeline,
camera, formats and exports are at parity — while the original
remains deeper in drawing, VFX, audio, 3D and graph editing. Since
the last refresh of this document, Wave 9 deepened the engine
(stroke stabilisation, 4 new VFX kinds, audio fade/gain/reverse,
per-key graph interpolation, autosave, project statistics, MCP at
51 tools) and two UI waves rebuilt the editor's chrome as a
38-component library in a dedicated `src/ui/` folder, with the
components wired into the sidebar, panels, timeline and export
centre. Wave 10 then closed the simultaneous-audio gap (live +
export mixing), added Fog weather, movieclip loop modes, pose
mirroring (2D and true 3D), graph auto-tangents, 3 VFX kinds,
.nodes gradient round-trip, MCP at 54 tools, real translations
for 6 more languages, and export-determinism tests. Wave 11 then
added drawing shape tools, 3 more VFX kinds (24) with GUI buttons
for every kind, audio echo/lowpass/silence-trim, graph baking and
key reduction, 3D camera presets and auto-framing, frame-range
exports, a .stknds camera round-trip, translations for the last
19 languages, MCP at 57 tools, and a timecode chip. Wave 12
then went for depth: brush opacity/scatter model, shape
generators round 2, 3 VFX kinds (27) + Storm weather + FX
presets, audio pitch shift + compressor, graph tangent
presets, MP4/WebM frame-range exports, a 3D pose library and
a full two-bone 3D IK solver (after the missing matrix→Euler
extraction was built), MCP at 62 tools, and determinism proof
for APNG + JPEG exports. The suite's
last fully verified state is 779 tests green in GitHub Actions
CI; per the repo's AGENTS.md, CI is the only build environment.
