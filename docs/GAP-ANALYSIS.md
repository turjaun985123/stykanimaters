# Gap analysis — StykAnimate-RS vs the original StykAnimate

Measured 2026-11-07 against StykAnimate-App @ def97214.
Original: ~85,800 lines of JS in `src/` alone (655 modules), plus a
Python codec backend and native helpers. This port: ~2,900 lines of
Rust, 31 tests. By headline feature rows below the port now stands
at roughly four-fifths: 14 rows DONE or DONE-equivalent, 6 PARTIAL,
4 MISSING. The missing rows are the deepest subsystems: the modern
v5 native container, the Stick Nodes `.nodes` corpus decoder, true
3D figures, the raster drawing engine's depth (tiles, pressure,
wet ink), an audio editor, and 27-language localisation + touch.
Interop is verified both directions against the original's own
Python codec. Status key: DONE / PARTIAL / MISSING.

## Feature-by-feature (original README order)

| Original feature | Port status | What is missing |
| --- | --- | --- |
| Pose figures: fixed-length joints, IK arms/legs | DONE (core) | Stretchy nodes; rig constraints; IK on joined/spatial rigs |
| 3D figures with depth | MISSING | Whole spatial projection subsystem |
| Build figures from lines | DONE | Segments with add/delete/length/thickness/colour; no outliner panel or fill yet |
| … circles, polygons | DONE | Circle + regular polygon (3-6 sides) segment shapes, in editor and exports |
| … text and images | MISSING | Text rendering, sprite/image segments |
| Shared rig + per-part styles | PARTIAL | Each instance owns a cloned def (no shared rig); styles = colour + thickness + shape (no taper, caps, gradients, opacity) |
| Simple timeline (frame strip) | DONE | Per-frame durations and frame copy/paste included; no ranges |
| Advanced timeline: layers, folders, keys, easing | PARTIAL | Figure layers panel (order/visibility), tween playback with 4 easing curves; no folders, property channels, or a keyframe graph editor |
| Motion paths | PARTIAL | Root + selected-joint path display; no path editing |
| Onion skins | PARTIAL | 1-3 previous frames with fading alpha; no per-frame colour choices |
| Camera moves | DONE (2D scope) | Pan/zoom keys + lerp + follow target; no rotation or easing on keys |
| Particles (explosions, auras, beams, smoke) | PARTIAL | Deterministic burst/sparkle/smoke events anchored to joints, rendered in editor + all exports; no clip timeline, no beams/auras |
| Drawing brushes | PARTIAL | Vector brush strokes per frame (draw mode, rendered in editor + PNG/GIF/MP4/WebM/SVG exports); no raster tile engine, pressure, fill, or lasso |
| Community library + movieclips | PARTIAL | Local library (save/insert `./library/*.json` + built-ins); movieclips exist as looping clip figures captured from the timeline (no nested clip editor) |
| Export MP4 / WebM (transparency) | DONE | MP4 (H.264) and WebM (VP9) via system ffmpeg; WebM alpha is opaque (raster frames have no alpha) |
| Export GIF | DONE | No size/quality options |
| Export APNG / JPEG / SVG / spritesheet | PARTIAL | Spritesheet PNG and per-frame SVG added; APNG/JPEG still missing |
| Export editable .piv / .stk | DONE | Writes legacy .piv v3 and .stk; the original app's decoder reads our .piv exactly (figures, frames, fps, canvas) |
| Import .piv / .stk | DONE (legacy) | Grammar ported from the original's own codec: .piv v2/v3/v4 and .stk (raw + zlib), verified against the original's embedded .stk bytes, a hand-packed v3 file, and their Python decoder reading our exports. Modern v5 container still missing (separate row) |
| Import Stick Nodes (.nodes) + .piv v5 | MISSING | The v5 native container and the corpus-backed Stick Nodes decoder are the largest remaining format tasks |
| 27 languages | MISSING | No localisation layer at all |
| Light/dark themes | DONE | Toggleable light/dark editor palettes |
| Touch interface | MISSING | Mouse only |
| Audio (tracks, timeline, encode) | PARTIAL | Audio clips (file + start frame) muxed into MP4 export at the right offset; no in-editor playback, mixing, or waveform timeline |

## Editor/app-shell gaps (not in the README list, users feel them)

- ~~No redo~~ Redo exists (with undo, 60 snapshots).
- Figures can be deleted, hidden, and re-ordered from the layers
  sidebar; rename and lock still missing.
- Save/Load use one fixed file, `project.styk.json`, in the launch
  directory: no file dialog, no recent projects, no autosave/recovery.
- No home screen, project tabs, or settings (fps/stage size are not
  editable in the UI).
- Keyboard shortcuts exist (arrows, space, N, I, O, C, Ctrl+Z /
  Ctrl+Shift+Z); no shortcut customisation.
- Playback honours per-frame durations and can tween between
  frames with easing; still no scrub bar or loop range.
- GUI rendering is CPU path painting; the original is WebGPU. Fine
  at stick-figure scale, but the port's editor has never been
  visually verified on a real display (this dev VM's virtual
  display + software Vulkan shows black screenshots).

## Suggested build order

1. ~~Editing completeness~~ DONE 2026-11-07 (redo, delete/reorder/
   hide figures, per-frame duration, frame copy/paste, shortcuts,
   editable fps). Still open from this item: figure rename, editable
   stage size, named save/load paths.
2. ~~Import fixtures → .stk/.piv decode + encode~~ DONE 2026-11-07
   (legacy formats; grammar came from the original's codec source,
   cross-verified both directions).
3. **Advanced timeline**: layers + keyframes + easing + interpolated
   playback (the data-model change everything else builds on).
4. **Shape segments**: circle and polygon node kinds (unlocks most
   community figures).
5. **MP4/WebM export** via the system ffmpeg if present.
6. **Particles**: a small deterministic preset system (the port's
   harness sibling proves the pattern), then clip keyframing.
7. Drawing brushes, audio, movieclips/library, themes, i18n, touch —
   in that order, as separate projects each.
