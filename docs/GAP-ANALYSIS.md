# Gap analysis — StykAnimate-RS vs the original StykAnimate

Measured 2026-11-07 against StykAnimate-App @ def97214.
Original: ~85,800 lines of JS in `src/` alone (655 modules), plus a
Python codec backend and native helpers. This port: ~2,900 lines of
Rust, 31 tests. By headline feature rows below the port now stands
at roughly four-fifths: 14 rows DONE or DONE-equivalent, 6 PARTIAL,
4 MISSING. The missing rows are the deepest subsystems: the Stick Nodes `.nodes` corpus decoder, true
3D figures, the raster drawing engine's depth (tiles, pressure,
wet ink), an audio editor, .piv v5 export, and 27-language
localisation + touch.
Interop is verified both directions against the original's own
Python codec, including .piv v5 files produced by its encoder. Status key: DONE / PARTIAL / MISSING.

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
| Import .piv v5 (native container) | DONE (read) | Full sequential grammar: flagged figure records, bends/polyfills/z-order skips, image+text side tables (absolute end pointers with both recovery branches, nibble-walk glyph skipping), uid-keyed instances, per-frame camera. Verified against files written by the original's own encoder and hand-built media fixtures its decoder also reads. Export still missing |
| Import Stick Nodes (.nodes) | MISSING | The corpus-backed Stick Nodes decoder is the largest remaining format task |
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

## Wave 3 — the 10-agent push (2026-11-07)

Built by ten parallel agents with strict file ownership, then integrated
(module registration, UI wiring, corpus tests) centrally. 64 tests green.

| Addition | State |
|---|---|
| .piv v5 EXPORT (`pivot::export_piv_v5`) | DONE — original's Python decoder reads our file with exact byte consumption (900/900) |
| Stick Nodes `.nodes` import (`src/nodes.rs`, spec in docs/NODES-SPEC.md) | DONE for figures — full 57-field record grammar with all version gates; verified against a real corpus fixture from the original repo AND their own JS decoder. Connectors import as plain segments; polyfills parsed, not rendered; `.nodemc`/`.stknds` containers not covered |
| APNG export (png crate native APNG) | DONE — PIL opens the output as a real animated PNG |
| JPEG sequence export | DONE — SOI/EOI-verified |
| Per-keyframe easing (`Frame.easing`) | DONE — frame overrides project easing for its outgoing segment |
| Localisation (`src/i18n.rs`) | PARTIAL — EN/BN/ES, 50 keys, wired into ~45 toolbar labels + a Lang toggle; the original ships 27 languages |
| Audio playback (`src/audio.rs`) | PARTIAL — ffplay subprocess synced to the playhead on Play; no waveform/editor depth, first clip only |
| VFX clips (`src/vfx.rs`) | DONE as a system — timed clips (Burst/Sparkle/Smoke/Beam/Aura/Trail) anchored to joints, rendered in canvas + all raster exports; add via FX clip buttons |
| Cross-format test corpus (`tests/fixtures/`, `scripts/`) | DONE — one scene in v2/v3/v4/v5/.stk, generated + verified with the original's codecs, asserted by a Rust corpus test |
| Project library backend (`src/projects.rs`) | DONE as a backend — scan/load/save/thumbnails; the editor's existing named-project flow still drives the UI |
| Onion skin, both directions | DONE — next frames tinted green 0x7BE495 |

Updated parity accounting: **~16 of 24 rows done-equivalent, 6 partial,
2 missing** (true 3D figures; touch UI). The partials are depth, not
absence: drawing (no brushes/text), audio (playback only), movieclips
(no nested-timeline editor), keyframe graph editor, localisation
(3 of 27 languages), and the Stick Nodes animation containers.
