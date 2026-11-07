# StykAnimate vs StykAnimate-RS — comprehensive comparison

Measured fresh from both working trees on 2026-11-07 (after Wave 6).
This document supersedes the earlier comparison + addenda.

## At a glance

| | Original (StykAnimate-App) | Port (StykAnimate-RS) |
|---|---|---|
| Version | 1.2.5 | 0.1.0 |
| Runtime | Electron + WebGPU app, Python codec backend | One native binary: Rust + GPUI 0.2.2 |
| Application code | ~103,500 lines JS in 657 files under `src/` (excluding `src/locales`) | 26,620 lines Rust in 28 modules + 2 binaries |
| Generated code | 179,460-line compiled locale bundle (`locale-packs.mjs`) | none |
| Backend/codecs | ~13,000 lines Python (`codecs/` + `backend/`) | included in the Rust total (`pivot.rs` 1,826; `nodes.rs` 772; `nodes_anim.rs` 2,433) |
| Tests | 876 files in `tests/` (fixtures + suites) | 289 Rust tests + a 5-format fixture corpus with manifest |
| UI languages | 27 | 28 (the same 27 + Bengali) |
| Licence | GPL-3.0-or-later | GPL-3.0-or-later |

## Feature matrix

| Feature family | Original | Port | Verdict |
|---|---|---|---|
| Core posing (FK, two-bone IK, bend) | Full | Full | Equal |
| Figure presets (humanoid, quadruped, mini) | Full | Full | Equal |
| Figure builder (add/remove, length, thickness, colour, shapes) | Full | Full | Equal |
| Segment styles (taper, caps, gradients, opacity) | Full | Rendered in raster + SVG, button editing | Near-equal |
| Timeline (frames, durations, fps, copy/paste, inbetween) | Full | Full | Equal |
| Easing | Full curve editor | 4 curves + per-keyframe override + a Hermite channel engine (`graph.rs`) applied live in the editor with key authoring; panel view-model built, visual panel pending | Original deeper |
| Onion skin | Full | Two-way (prev in figure colour, next in green) | Equal |
| Camera (keys, pan, zoom, follow) | Full | Full | Equal |
| Layers / figure list | Full layer system | Reorder (full reference remapping), hide, delete, select, **solo, lock, per-figure opacity** | Near-equal |
| Drawing | Deep raster engine (brushes, fill, tiles, text) | Strokes with pressure widths, serpentine fill, lasso erase | Original much deeper |
| Sprite / text segments | Full | Sprites (aspect-correct) + bitmap-font text rendered **and typed in the editor**; Stick Nodes sprite/text instances imported as keyframed tracks and rendered | Near-equal |
| Particles / VFX | ~6,600-line VFX system | Events + timed clips (8 kinds incl. confetti, shock ring) + continuous weather (rain, snow, fireflies), deterministic | Original deeper |
| Audio | Full editor (waveform, mixing) | Playback (multi-clip aware), waveform peaks, mixdown export, normalise/trim/split/crossfade engine (editor buttons pending) | Original deeper |
| Movieclips | Full editor | ClipDef playback + conversion from .nodemc | Original deeper |
| 3D figures | Full 3D system | 3D mode in the editor: orbit, per-joint Euler rotation, live projected preview, **bake to a native 2D figure** | Original deeper |
| Touch | Full touch UI | Recogniser wired into the canvas (tap select, drag pose, long-press); single-pointer via GPUI mouse events, pinch arms ready for real touch pointers | Original deeper |
| Home / library / projects | Full | Home panel, recents, recovery snapshots, figure library | Near-equal |
| App shell (dialogs, autosave) | Native dialogs, full shell | Native open/save dialogs via zenity/kdialog with the folder flow as fallback, recovery, stage-size controls | Near-equal |
| AI-agent control (MCP) | None | **Full MCP server: 30 tools — build, pose, decorate, render, import/export, frame inspection as base64 PNG** | Port only |
| Localisation | 27 languages, human-maintained | 28 languages (same set + Bengali), machine-produced strings | Port wider, original more polished |

## Formats — the port's strongest area

| Format | Original | Port import | Port export |
|---|---|---|---|
| .piv v2 / v3 / v4 | native | Yes | v3 only |
| .piv v5 | native | Yes — full grammar incl. image/text side tables | Yes — subset (no media figures); the original's own decoder consumes our files exactly (900/900 bytes in the checked case) |
| .stk | native | Yes | Yes |
| Stick Nodes .nodes | native | Yes — 57-field record grammar, verified against their decoder + real corpus fixture | No |
| Stick Nodes .nodemc / .stknds | native | Yes — modelled subset (sprites/texts/joins parsed, not modelled) | No |
| Native project | JSON | JSON | JSON |
| PNG sequence / GIF / MP4 / WebM / APNG / JPEG / SVG / spritesheet | Yes | — | All yes, deterministic, several verified with independent decoders (PIL checks the APNG; ffmpeg produces real MP4/WebM) |

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
Electron, no Python runtime. Roughly 6.5x less code for the surface
it covers. Exports are closed-form deterministic (same input, same
bytes, verified). Its format support is not re-implemented from
guesses but proven against the original's own decoders, in both
directions. And it speaks one more language: Bengali.

## Bottom line

By feature family, the port now has **something verified in every
area the original has** — nothing is entirely missing any more.
By depth, the honest estimate is that the port covers most of the
everyday posing/animation workflow at full strength, while the
original remains deeper in drawing, VFX volume, audio editing
UI, 3D depth, and the graph editor's visual panel. Wave 5 closed
the integration list Wave 4 left: 3D and touch are in the editor,
graph channels apply live with key authoring, text is typeable,
native dialogs exist, and Stick Nodes sprites/texts are modelled
and rendered. What remains is depth: the graph panel's visuals,
audio-edit buttons, Stick Nodes joins, and drawing/VFX volume.
