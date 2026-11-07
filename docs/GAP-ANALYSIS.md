# Gap analysis — StykAnimate-RS vs the original StykAnimate

Measured 2026-11-07 against StykAnimate-App @ def97214.
Original: ~85,800 lines of JS in `src/` alone (655 modules), plus a
Python codec backend and native helpers. This port: ~1,700 lines of
Rust, 13 tests, 2 commits. By code volume the port is ~2%; by
headline features it covers the core posing loop and parts of
several others. Status key: DONE / PARTIAL / MISSING.

## Feature-by-feature (original README order)

| Original feature | Port status | What is missing |
| --- | --- | --- |
| Pose figures: fixed-length joints, IK arms/legs | DONE (core) | Stretchy nodes; rig constraints; IK on joined/spatial rigs |
| 3D figures with depth | MISSING | Whole spatial projection subsystem |
| Build figures from lines | PARTIAL | Only straight segments; no outliner, no fill, no multi-select of nodes |
| … circles, polygons | MISSING | Shape segment kinds in the model |
| … text and images | MISSING | Text rendering, sprite/image segments |
| Shared rig + per-part styles | PARTIAL | Each instance owns a cloned def (no shared rig); styles = colour + thickness only (no taper, caps, gradients, opacity) |
| Simple timeline (frame strip) | DONE | No per-frame duration, no frame copy/paste, no ranges |
| Advanced timeline: layers, folders, keys, easing | MISSING | Layers, layer folders, property channels, keyframe easing curves, tween playback |
| Motion paths | MISSING | Path display and editing for roots/joints |
| Onion skins | PARTIAL | Previous frame only; no multi-frame range, colours, or opacity control |
| Camera moves | PARTIAL | Pan/zoom keys + lerp exist; no camera follow target, no rotation, no easing, no camera preview framing guides |
| Particles (explosions, auras, beams, smoke) | MISSING | Entire clipped, keyframed VFX system (~40 modules in the original) |
| Drawing brushes | MISSING | Entire raster drawing subsystem (~40 modules) |
| Community library + movieclips | MISSING | Library browsing/insertion; movieclips (nested animated figures) entirely absent |
| Export MP4 / WebM (transparency) | MISSING | Needs an encoder route (ffmpeg sidecar or native) |
| Export GIF | DONE | No size/quality options |
| Export APNG / JPEG / SVG / spritesheet | MISSING | Only PNG sequence + GIF |
| Export editable .piv / .stk | MISSING | Blocked on the same format work as import |
| Import .piv / .stk | MISSING | Not guessed: needs a real Pivot-made fixture file to reverse the binary layout byte-for-byte, then a decoder + tests against the fixture |
| Import Stick Nodes (.nodes) | MISSING | Original's decoder is a research-grade corpus-backed reader; largest single import task |
| 27 languages | MISSING | No localisation layer at all |
| Light/dark themes | MISSING | One hard-coded dark palette |
| Touch interface | MISSING | Mouse only |
| Audio (tracks, timeline, encode) | MISSING | Original has a full audio engine; port has no sound at all |

## Editor/app-shell gaps (not in the README list, users feel them)

- No **redo** (undo only, 60 snapshots).
- **Figures cannot be deleted**, renamed, hidden, locked, or
  re-ordered (no z-order control); no figure list panel.
- Save/Load use one fixed file, `project.styk.json`, in the launch
  directory: no file dialog, no recent projects, no autosave/recovery.
- No home screen, project tabs, or settings (fps/stage size are not
  editable in the UI).
- No keyboard shortcuts.
- Playback plays raw frames only: no scrubbing, no loop range, no
  interpolated playback between keys (there are no keys yet — see
  advanced timeline).
- GUI rendering is CPU path painting; the original is WebGPU. Fine
  at stick-figure scale, but the port's editor has never been
  visually verified on a real display (this dev VM's virtual
  display + software Vulkan shows black screenshots).

## Suggested build order

1. **Editing completeness** (small, high value): redo, delete/rename
   figure, z-order, per-frame duration, frame copy/paste, keyboard
   shortcuts, editable fps/stage, named save/load paths.
2. **Import fixtures → .stk/.piv decode + encode**, tested against
   real files. This is the original's headline promise.
3. **Advanced timeline**: layers + keyframes + easing + interpolated
   playback (the data-model change everything else builds on).
4. **Shape segments**: circle and polygon node kinds (unlocks most
   community figures).
5. **MP4/WebM export** via the system ffmpeg if present.
6. **Particles**: a small deterministic preset system (the port's
   harness sibling proves the pattern), then clip keyframing.
7. Drawing brushes, audio, movieclips/library, themes, i18n, touch —
   in that order, as separate projects each.
