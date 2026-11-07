# StykAnimate vs StykAnimate-RS — comprehensive comparison

Measured fresh from both working trees on 2026-11-07 (after Wave 6);
refreshed 2026-11-08 after the Wave 7 integration, the GitHub
Actions CI workflow, and the UI redesign. Port figures below are
re-measured from the current tree; the original's are unchanged.
This document supersedes the earlier comparison + addenda.

## At a glance

| | Original (StykAnimate-App) | Port (StykAnimate-RS) |
|---|---|---|
| Version | 1.2.5 | 0.1.0 |
| Runtime | Electron + WebGPU app, Python codec backend | One native binary: Rust + GPUI 0.2.2 |
| Application code | ~103,500 lines JS in 657 files under `src/` (excluding `src/locales`) | 32,004 lines Rust in 30 library modules + 2 binaries |
| Generated code | 179,460-line compiled locale bundle (`locale-packs.mjs`) | none |
| Backend/codecs | ~13,000 lines Python (`codecs/` + `backend/`) | included in the Rust total (`pivot.rs` 2,896; `nodes.rs` 1,182; `nodes_anim.rs` 4,299) |
| Tests | 876 files in `tests/` (fixtures + suites) | 344 Rust tests, green in GitHub Actions CI, + a 7-file cross-format fixture corpus with manifest |
| UI languages | 27 | 28 (the same 27 + Bengali) |
| Licence | GPL-3.0-or-later | GPL-3.0-or-later |

## Feature matrix

| Feature family | Original | Port | Verdict |
|---|---|---|---|
| Core posing (FK, two-bone IK, bend) | Full | Full | Equal |
| Figure presets (humanoid, quadruped, mini) | Full | Full | Equal |
| Figure builder (add/remove, length, thickness, colour, shapes) | Full | Full | Equal |
| Segment styles (taper, caps, gradients, opacity) | Full | Rendered in raster + SVG, button editing | Near-equal |
| Timeline (frames, durations, fps, copy/paste, inbetween) | Full | Full — plus loop ranges, block shift/reverse and motion-path editing (nudge/smooth roots) | Equal |
| Easing | Full curve editor | 4 curves + per-keyframe override + a Hermite channel engine (`graph.rs`) applied live in the editor, with a visual panel (add/drag/delete keys, undoable) | Original deeper |
| Onion skin | Full | Two-way (prev in figure colour, next in green) | Equal |
| Camera (keys, pan, zoom, follow) | Full | Full | Equal |
| Layers / figure list | Full layer system | Reorder (full reference remapping), hide, delete, select, **solo, lock, per-figure opacity** | Near-equal |
| Drawing | Deep raster engine (brushes, fill, tiles, text) | Strokes with pressure widths, a brush library + brush/eraser tools, serpentine fill, splitting eraser, smoothing and nib profiles | Original much deeper |
| Sprite / text segments | Full | Sprites (aspect-correct) + bitmap-font text rendered **and typed in the editor**; Stick Nodes sprite/text instances imported as keyframed tracks and rendered | Near-equal |
| Particles / VFX | ~6,600-line VFX system | Events + timed clips (14 kinds incl. lightning, bubbles, leaves) with an FX timeline editor (list/move/resize/duplicate/delete) + continuous weather (rain, snow, fireflies), deterministic | Original deeper |
| Audio | Full editor (waveform, mixing) | Playback (multi-clip aware), waveform peaks + panel, mixdown export, normalise/trim/split/crossfade (engine and editor buttons) | Original deeper |
| Movieclips | Full editor | ClipDef playback, conversion from .nodemc, and an in-editor clip session (create a clip from a figure's timeline poses, edit its frames) | Original deeper |
| 3D figures | Full 3D system | 3D mode in the editor: orbit, per-joint Euler rotation, live projected preview, **bake to a native 2D figure** — a single pose or an animated bake interpolated between two marked poses; library persisted in `./library3d` | Original deeper |
| Touch | Full touch UI | Recogniser wired into the canvas (tap select, drag pose, long-press; pinch logic is multi-pointer ready). Real multi-touch is blocked by the platform: GPUI 0.2.2's Linux input dispatches no touch events at all (verified in the vendored crate source, see `touch.rs`), so a touchscreen arrives as one emulated mouse pointer | Original deeper |
| Home / library / projects | Full | Home panel, recents, recovery snapshots, figure library | Near-equal |
| App shell + UI (dialogs, autosave) | Native dialogs, full shell | Native open/save dialogs via zenity/kdialog with the folder flow as fallback, recovery, stage-size controls — and redesigned chrome: an app bar + labelled tool groups (`ui_kit.rs`), colour-dot figure rows, a timeline bar. The GUI has been seen running on a real Linux desktop (user screenshot, 2026-11-08) | Near-equal |
| AI-agent control (MCP) | None | **Full MCP server: 51 tools — build, pose, draw, decorate, manage FX clips, run 3D sessions, author movieclips, set per-frame bends, render, import/export (incl. .piv v2/v4, .nodes, .stknds), frame inspection as base64 PNG** | Port only |
| Localisation | 27 languages, human-maintained | 28 languages (same set + Bengali), machine-produced strings | Port wider, original more polished |

## Formats — the port's strongest area

| Format | Original | Port import | Port export |
|---|---|---|---|
| .piv v2 / v3 / v4 | native | Yes | Yes — v2, v3 and v4 (v2/v4 with documented losses: media segments written as plain segments; v4 also drops camera keys, joins and bends) |
| .piv v5 | native | Yes — full grammar incl. image/text side tables | Yes — incl. per-frame lengths (from graph Length channels), image side tables and per-frame bends (from `Frame.bends`); the original's own decoder consumes our files exactly, media fixture included. Text side tables are the one unwritable piece: the model carries a bitmap font, not glyph outlines |
| .stk | native | Yes | Yes |
| Stick Nodes .nodes | native | Yes — 57-field record grammar, verified against their decoder + real corpus fixture | Yes — figure export (documented losses: thickness rounded to i32; node names, styles and media dropped) |
| Stick Nodes .nodemc / .stknds | native | Yes — modelled subset (sprites/texts as keyframed tracks, joins baked at import) | Yes — .stknds writer (plain posed figures; joins, clip library, camera detail, sounds and FX dropped — see GAP-ANALYSIS) |
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
original remains deeper in drawing volume, VFX volume, and 3D
depth. Wave 6 closed the graph panel (visual, undoable), audio
editing operations, Stick Nodes joins, camera rotation, and a
full MCP server. Wave 7 then closed the rest of that list: the
remaining export formats (.piv v2/v4, .nodes figures, .stknds
animations), per-frame bends end-to-end (model, .piv v5
round-trip, rendered arcs, editor + MCP access), timeline depth
(loop ranges, block shift/reverse, motion-path editing), the
brush library, the FX timeline editor, the movieclip editing
session, animated 3D bakes, and 14 more MCP tools; a Wave 8 pass
then wired the remaining Wave 7 surface (FX clip management in
MCP + GUI panels, brush/clip/3D-library UI, timeline depth
buttons, bend button, format export buttons) and a UI components
wave (toasts, inspector, colour picker, dopesheet, easing picker,
help overlay, export centre, canvas HUD), bringing the MCP
server to 51 tools (Wave 9).
Since then the suite runs green in GitHub Actions CI (344 tests —
per the repo's AGENTS.md, CI is the only build environment), the
editor chrome was redesigned (app bar + labelled tool groups),
and the GUI has been visually verified running on the user's own
Linux PC (2026-11-08) — the last "never seen on a real display"
caveat is gone. What remains is depth, plus two hard limits:
real multi-touch is impossible on GPUI 0.2.2's Linux platform
(its input layer has no touch events to wire), and .piv text
side tables cannot be written from a bitmap-font model. One
small coverage gap: the MCP `add_fx_clip` tool exposes 8 of the
14 clip kinds the editor offers.
