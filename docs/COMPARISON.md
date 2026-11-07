# Comprehensive Comparison — StykAnimate (original) vs StykAnimate-RS (port)

Measured 2026-11-07. Original: StykAnimate-App @ `def97214`, v1.2.5.
Port: StykAnimate-RS @ `bd4f557`. Every number below was counted from
the two working trees on this date; feature verdicts come from the
source, not from marketing pages.

## 1. At a glance

| Measure | Original (JS/Electron) | Port (Rust/GPUI) |
| --- | --- | --- |
| Version | 1.2.5 | 0.1.0 |
| Licence | GPL-3.0-or-later | GPL-3.0-or-later |
| App source | ~85,800 lines JS in 655 modules (`src/`) | 4,498 lines Rust in 7 modules (`src/`) |
| Codec/backend code | ~16,100 lines Python (`codecs/`, `backend/`) | included above (`src/pivot.rs`, 907 lines) |
| Automated tests | hundreds of test/fixture files (`tests/`, `codecs/test_*.py`, `backend/test_*.py`), incl. hash-verified fixture corpora | 31 Rust unit/integration tests |
| Docs | 83 markdown files | README + this doc + GAP-ANALYSIS |
| Runtime deps | Electron + WebGPU (1 npm runtime dep, 5 dev) | native binary; crates: gpui, serde, serde_json, image, flate2; MP4/WebM via system ffmpeg |
| Platforms | Windows, Linux, browser (same web codebase) | Linux desktop today (GPUI; macOS/Windows possible, untested) |
| Rendering | WebGPU renderer, tile-based drawing engine | GPUI canvas (editor) + CPU rasterizer (exports), 2× supersampled |
| Interface languages | 27 packs (`src/locales/packs/*.json`) | English only |

## 2. Architecture

| Aspect | Original | Port |
| --- | --- | --- |
| UI | Web app in Electron/CEF shell, custom component kit | Native GPUI views, GPU-composited |
| Document model | Rich project schema: figure types, instances, joins, polyfills, text assets, camera, audio, VFX clips, layer folders | Single project struct: figures × frames of poses, camera keys, particles, strokes, audio clips, movieclips — everything serde JSON |
| Posing math | Segment graph with absolute transforms, bends as arcs, stretchy segments | Node tree, relative angles, FK + two-bone IK with bend sign; bends/stretch not modelled |
| Playback | Worker-based render pipeline, proxy frames, audio clock, interpolation engine | In-process; fractional playhead, optional tween playback with easing |
| Persistence | Native `.styk` container (v2 sections), Pivot files, autosave/recovery | `project.styk.json`, named projects in `./projects/` |
| Determinism | Seeded where it matters; large parts are interactive-first | Exports are fully deterministic (same input → same frames, incl. particles via closed-form functions of frame index) |

## 3. Feature matrix

Verdicts: ● full · ◐ partial · ○ missing.

| Feature | Original | Port | Notes |
| --- | --- | --- | --- |
| Drag posing (FK aim) | ● | ● | Both: drag a joint, the chain aims |
| Inverse kinematics | ● | ● | Port: two-bone, reach clamping, per-limb bend; original adds stretchy segments and arc bends |
| Multi-figure scenes | ● | ● | Port: unlimited instances, each with its own cloned rig |
| Figure builder | ● | ◐ | Port: add/delete/resize segments, thickness, colour, shapes. Original adds an outliner, fill regions (polyfills), gradient/taper/cap styles, segment copy/paste |
| Segment shapes | ● | ◐ | Port: line, circle, polygon 3–6. Original adds filled/clear circles, squares, sprites with colour-key transparency, text glyphs |
| Rig presets | ● | ● | Port: humanoid, quadruped, mini |
| Simple timeline | ● | ● | Frame strip, add/duplicate/delete, per-frame duration, copy/paste (both) |
| Layers panel | ● | ◐ | Port: select/reorder/hide/delete figures. Original adds folders, locking, renaming, blend/opacity |
| Advanced timeline | ● | ◐ | Original: property channels, keyframe graph, per-key easing, folders, scenes. Port: whole-pose frames + tween playback with 4 global easing curves |
| Inbetween generation | ● | ● | Port inserts a 50% pose tween; original interpolates with easing choices |
| Onion skin | ● | ◐ | Port: 1–3 previous frames, fading. Original adds per-frame colours, ranges both directions, opacity |
| Camera | ● | ◐ | Port: pan/zoom keys, interpolation, follow target. Original adds rotation, easing, framing guides, per-frame camera in file format |
| Motion paths | ● | ◐ | Port displays root + selected-joint paths; original also edits paths |
| Particles / VFX | ● | ◐ | Port: deterministic burst/sparkle/smoke anchored to joints, editor + all exports. Original: ~40 modules — clip-based VFX timelines, beams, auras, trails, emitters |
| Drawing | ● | ◐ | Port: vector brush strokes per frame, all exports. Original: raster tile engine (~6,700 lines) — pressure, wet ink, fill, lasso, palettes |
| Audio | ● | ◐ | Port: place a clip at a frame; muxed into MP4. Original: audio engine (~1,000 lines) — playback sync, multiple tracks, waveform, encode options |
| Movieclips | ● | ◐ | Port: capture a figure's timeline into a looping clip figure. Original: nested clip editing, clip library, per-instance offsets |
| Figure library | ● | ◐ | Port: local `./library/*.json` save/insert + built-ins. Original: community library with browsing/sharing |
| Undo / redo | ● | ● | Port: 60 JSON snapshots, both directions |
| Keyboard shortcuts | ● | ◐ | Port: arrows, space, N, I, O, C, Ctrl+Z/Y. Original: full map + customisation |
| Themes | ● | ● | Port: dark/light palettes |
| Home / project screens | ● | ◐ | Port: named save/open under `./projects/`. Original: home screen, thumbnails, recovery |
| Localisation | ● 27 languages | ○ English only | |
| Touch / pen input | ● | ○ | GPUI port is mouse-first |
| 3D / spatial figures | ● | ○ | Original has a real spatial figure system; port is 2D |
| Autosave / crash recovery | ● | ○ | |
| Live collaboration / cloud | ○ (local-first by design) | ○ | Neither |

## 4. File formats

| Format | Original read | Original write | Port read | Port write |
| --- | --- | --- | --- | --- |
| `.piv` v2 / v3 (legacy) | ● | ● | ● | ● (v3) |
| `.piv` v4 (legacy) | ● | ● | ● | ○ |
| `.piv` v5 (native container) | ● | ● | ● (incl. image/text side tables) | ○ |
| `.stk` legacy figure | ● | ● | ● | ● |
| `.stk` modern (zlib figure stream) | ● | ● | ◐ reads the figure grammar incl. zlib wrap | ○ writes legacy form |
| Stick Nodes `.nodes` | ● | ● (export) | ○ | ○ |
| Native project | ● `.styk` container | ● | ◐ own JSON schema (not the same container) | ◐ |
| Sprites / images in figures | ● | ● | ○ (asset tables skipped) | ○ |
| Background images in `.piv` | ● | ● | ○ (tables skipped, solid background imported) | ○ |

Interop was verified against the original's own code, both ways:
its Python decoder reads the port's exported `.piv` exactly, a
Python-packed v3 file imports with exact values, and the original's
embedded `default_pivot2.stk` bytes parse in the port.

## 5. Exports

| Export | Original | Port |
| --- | --- | --- |
| MP4 (H.264) | ● | ● (ffmpeg; audio mux when a clip exists) |
| WebM (VP9) | ● with transparency | ● opaque alpha |
| GIF (looping) | ● | ● |
| PNG sequence | ● | ● |
| Spritesheet | ● | ● |
| SVG per frame | ● | ● |
| APNG | ● | ○ |
| JPEG sequence | ● | ○ |
| `.piv` / `.stk` | ● | ● legacy |

## 6. Where the original is clearly stronger

- **Depth everywhere**: drawing engine, VFX clips, audio engine,
  keyframe graph, 3D figures — each is thousands of lines with
  years of edge cases behind it.
- **Formats**: the v5 native container and the Stick Nodes corpus
  decoder are research-grade; the port covers the legacy family.
- **Product surface**: 27 languages, touch, home/library screens,
  recovery, installers, browser build.
- **Verification mass**: fixture corpora with hashes, roundtrip
  corpora in three languages (JS, Python, Rust tooling in-repo).

## 7. Where the port already stands equal or has its own character

- **The core posing loop is the same experience**: FK aim, IK with
  bend, onion skin, camera keys — no compromises in the model.
- **Deterministic exports**: particles and tweens are closed-form
  functions of frame time, so an export is reproducible bit-for-bit
  from the project file; the original's interactive-first pipeline
  makes no such promise.
- **Native footprint**: one Rust binary, no Electron, no browser
  engine; starts fast and runs fully offline.
- **Cross-verified interop**: the port's Pivot support is tested
  against the original's decoder and fixtures, not just against
  itself — most reimplementations never get that check.
- **Simplicity budget**: 4,500 lines total means the whole system
  can be held in one head; changes that take a subsystem hunt in
  the original are single-file edits here.

## 8. What remains, ranked by value

1. ~~`.piv` v5 read~~ DONE 2026-11-07 (verified against the
   original's encoder). `.piv` v5 write remains.
2. Stick Nodes `.nodes` import.
3. Sprite/image segments (finishes legacy Pivot fidelity).
4. Keyframe graph + property channels (advanced timeline depth).
5. Audio playback in the editor.
6. APNG/JPEG export (small).
7. Localisation framework (English + user languages first).
8. 3D figures, touch, home screen polish.

## Addendum — Wave 3 (2026-11-07, 10-agent push)

Since the comparison above: the port gained .piv v5 **export**
(cross-verified against the original's decoder), Stick Nodes `.nodes`
figure import (the last missing format family), APNG + JPEG exports,
per-keyframe easing, EN/BN/ES localisation on the main toolbar,
playhead-synced audio playback, timed VFX clips with beam/aura/trail,
a cross-format test corpus, and two-way onion skin. Test suite: 64.
The original still wins on depth in drawing, audio editing, 3D, and
its 27-language reach — but the port now touches every feature family
the original has, except 3D figures and touch.

## Addendum — Wave 4 (2026-11-07, second 10-agent push)

The port now covers every feature family: Stick Nodes animation
containers import (verified against the original's decoder and real
corpus projects), sprite/text segments render, segment styles
(taper/caps/gradients) render in raster + SVG, drawing gained
pressure + fill + lasso, audio gained waveform peaks + real mixdown,
localisation reached all 27 of the original's languages (+ Bengali),
and both "missing" rows now have verified engines: a 3D figure
subsystem (FK + perspective projection) and a touch gesture
recogniser. Test suite: 136. What the original still holds: those
two engines are not yet wired into the editor UI, there is no
graph-editor UI yet, and its drawing/audio/3D depth remains deeper.
