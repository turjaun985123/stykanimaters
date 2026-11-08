# Gap analysis — StykAnimate-RS vs the original StykAnimate

Measured 2026-11-07 against StykAnimate-App @ def97214.
Original: ~85,800 lines of JS in `src/` alone (655 modules), plus a
Python codec backend and native helpers. This port: ~7,600 lines of
Rust, 64 tests. By headline feature rows below the port now stands
at roughly two-thirds done-equivalent: 16 rows DONE or DONE-equivalent, 6 PARTIAL,
2 MISSING. The missing rows are the deepest subsystems: true
3D figures and touch UI. The partials are depth, not absence:
drawing (no brushes/fill), audio (playback only), movieclips (no
nested-timeline editor), keyframe graph, localisation (3 of 27
languages), and Stick Nodes animation containers.
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
  at stick-figure scale. (Updated 2026-11-08: the editor HAS now
  been visually verified on a real display — the user ran the
  CI-built binary on their own Linux PC and sent a screenshot of
  the editor open with the demo project loaded. This dev VM's
  virtual display + software Vulkan still shows black screenshots,
  so visual checks happen on the user's machine.)

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

## Wave 4 — the second 10-agent push (2026-11-07)

Central prep added model fields first (NodeStyle, SegmentMedia,
Project.assets, Stroke.widths/fill) so agents never fought over
model.rs. Ten agents, strict file ownership, central integration.
136 tests green.

| Addition | State |
|---|---|
| Stick Nodes animation (.nodemc/.stknds) import (`src/nodes_anim.rs`, spec docs/NODES-ANIM-SPEC.md) | DONE for the modelled subset — verified against the original's decoder, its synthetic corpus, and three real corpus projects with exact values. Sprites/texts/joins/camera records are parsed and length-validated but not modelled (spec §17) |
| Text + sprite segments (`src/media.rs`) | DONE — sprites aspect-correct on segments, text via a built-in 3x5 bitmap font (documented charset); composited in the rasterizer, attach via "Img seg" (text authoring UI still to come) |
| Keyframe graph engine (`src/graph.rs`) | ENGINE DONE — Hermite channels (root/angle/length) with shortest-path angle wrap; no graph-editor UI or project storage yet |
| Drawing depth (`src/paint.rs` + editor) | DONE — pressure widths recorded AND rendered per-point, serpentine fill, lasso erase |
| Audio editor backend (`src/audio_edit.rs`) | DONE as a backend — WAV peaks, real mixdown (Mixdown button writes out/mixdown.wav), audible_at drives multi-clip-aware playback (first audible clip; simultaneous playback still single-stream) |
| Localisation | DONE for the UI string set — 28 languages (the original's 27 + Bengali), all 50 keys, Lang button cycles all |
| Segment styles (`src/style.rs` + rasterizer + SVG) | DONE — taper, caps, gradients, opacity rendered in raster + SVG; Seg cap / Taper / Gradient buttons edit the selected segment |
| App shell (`src/shell.rs` + app.rs) | DONE — Home panel with project list + recents, recovery snapshot/restore, stage size buttons. Native file dialogs still not present (folder-based flow remains) |
| 3D figures (`src/fig3d.rs`) | SLICE DONE — real 3D FK rig + perspective projection + painter order, unit-verified; not yet integrated into the editor/canvas |
| Touch (`src/touch.rs`) | ENGINE DONE — gesture recogniser fully tested; GPUI touch wiring still to come |

Updated parity accounting: every feature family in the original is
now present in some verified form. Remaining gaps are integration
and depth: 3D in the editor, touch wiring, graph-editor UI, text
authoring, native file dialogs, sprite/joins modelling for Stick
Nodes animation files, and the original's drawing/audio depth.

## Wave 5 — integration (2026-11-07)

Central prep added Project.channels/sprites/texts/weather and
FigureInstance.locked/opacity. Ten agents then landed: touch wired
into the canvas (gesture-driven posing); a 3D editor mode with
bake-to-2D (projected joints match within 0.5 px); graph channel
editing ops + a panel view-model; a text-authoring state machine,
wired to the keyboard; native file dialogs via zenity/kdialog
(folder-flow fallback); Stick Nodes sprite/text instances modelled
(real pixel extraction) and rendered; drawing round 2 (Chaikin
smoothing, resampling, nib shapes, splitting eraser); continuous
weather (rain/snow/fireflies) + confetti and shock-ring clips;
audio depth (peak/RMS, normalise, trim, split, crossfade); and a
layers engine (solo/lock/opacity/move with full reference
remapping). Integration wired all of it into the UI: sidebar
lock/solo/opacity buttons, weather and 3D controls, graph key
authoring with live application in positions_f, locked figures
refusing drags, solo-aware canvas, per-figure opacity in canvas
and rasterizer. 232 tests green.

## Wave 6 — MCP + depth (2026-11-07)

Prep: the crate became a library (src/lib.rs) so the GUI binary
and the new MCP server binary share one engine; CameraKey gained
an angle field + camera_angle_at. Ten agents: MCP transport/core
(mcp.rs, JSON-RPC stdio, handle() pure and tested), 20 MCP content
tools, 7 MCP render/format tools (incl. render_frame_base64);
the visual graph panel (add/drag/delete keys, undo via a press
snapshot); audio editor ops (normalise/trim/split-with-real-tail/
crossfade); Stick Nodes joins baked at import per the original
decoder's semantics (no rotation inheritance exists upstream);
draw_ops (smoothing/nib/erase over projects); .piv v5 export
deepening (per-frame lengths from graph Length channels + image
side tables landed and cross-verified 40/40 against the original's
decoder; bends and text side tables proven impossible from the
model's data and documented); VFX depth (Ember/Fountain/Orbit +
real motion trails for Trail clips); camera rotation rendered in
rasterizer + SVG with Pivot's sign convention, byte-identical at
0 degrees, and .piv camera angles now round-trip. Integration
also wired Cam rot / Smooth / audio buttons. End-to-end proof: a
scripted MCP session over the real binary built a project, posed
it, added rain + confetti + a graph key + a rotating camera,
rendered PNG/GIF, read a frame back as base64, saved JSON and
exported a .piv the original's decoder accepts. 289 tests green.

## Wave 7 + 8 — formats round 2, bends, timeline depth, CI, UI redesign (2026-11-07/08)

Wave 7 landed as WIP (commit 121c0e3, stopped before integration),
was integrated and fixed through the new CI loop, and Wave 8 is
wiring the last pieces. State of the items:

| Addition | State |
|---|---|
| FX timeline engine (`src/fx_edit.rs`) | DONE — pure ops over `Project.fx_clips`: list (FxInfo), move, resize, duplicate, delete, clips_at |
| Clip kinds Lightning / Bubbles / Leaves (`src/vfx.rs`) | DONE — 14 clip kinds total; deterministic like the rest (bolt flicker is a pure function of frame time; bubbles pop in their last 10%; leaves flutter on individual cycles) |
| Brush library (`src/paint.rs` + editor) | DONE — `Brush` presets from `brush_library()`, a `DrawTool { Brush, Eraser }` in the editor with a clamped `brush_index`; the default Pen path leaves prior draw behaviour unchanged |
| Format exports round 2 | DONE with documented losses — `export_piv_v2` / `export_piv_v4` (media segments written as plain segments; v4 also drops camera keys, joins (flag 0), transparency and bends; v2's u8 counts cap it at 253 figures, v4's counts are u16), `export_nodes_figure` (thickness rounded to i32, negative lengths as magnitude + 180° flip, names/styles/media dropped, ellipses as circles), `export_nodes_animation` (.stknds writer: plain posed figures — joins written as none, clip library empty, default camera records; sounds/weather/FX/strokes/channels/easing/bends/stage size have no slots and are dropped; fps rounded to an integer, durations round up to whole ticks) |
| Per-frame bends round-trip | DONE — model `Frame.bends` (prep d922318) + `Project::bends_at` + `model::bend_mid`; .piv v5 export writes `bend_flag` + f64 arrays from `Frame.bends` when they differ from frame 0, import reads them back into the model; the rasterizer draws bent segments as two pieces through the arc midpoint (zero-bend projects render byte-identically); editor `cycle_bend()` steps 0 → +0.35 → +0.7 → −0.35 → −0.7 → 0 rad; MCP `set_bend` tool |
| Timeline depth (`src/editor.rs`) | DONE — `loop_range` (playback wraps inside it), `shift_frames` (block move), `reverse_range` (poses/bends/durations/easings reverse together), `nudge_path_point` + `smooth_path_roots` (motion-path editing) |
| Movieclip editing session (`src/editor.rs`) | DONE — `ClipEditSession` (enter/exit, clip frame ops) + `create_clip_from_figure(fi)` capturing the figure's poses across the timeline |
| 3D depth (`src/fig3d.rs` + editor) | DONE for this slice — `lerp_pose`, `bake_range`/`bake_range_staged`, a marked pose B and `fig3_bake_animation(frames)` interpolating between poses into new timeline frames; 3D figures persist in a library under `./library3d` |
| MCP round 2 | DONE — 14 new tools (44 total): `draw_stroke`, `smooth_strokes`, `erase_strokes`, `apply_nib`, `fig3_enter`/`fig3_select`/`fig3_rotate`/`fig3_orbit`/`fig3_mark_b`/`fig3_bake`/`fig3_bake_animation`, `clip_create`/`clip_list`, `set_bend`; `export_file` gained piv2/piv4/nodes/stknds |
| Corpus round 2 | DONE — `tests/fixtures/` gained `figure.nodes` and `project.nodemc` (generated with the original's encoders where possible, `scripts/gen_fixtures2.py`) + manifest entries + Rust corpus-test dispatch for both formats |
| Touch on GPUI 0.2.2 — platform verdict | RESEARCHED, BLOCKED UPSTREAM — `src/touch.rs` documents the check of the vendored crate source: `PlatformInput` has no touch/pointer variant, mouse events carry no pointer identity, the X11 client announces XInput 2.1 (touch needs 2.2), and Wayland binds no `wl_touch`. A touchscreen can only arrive as a single emulated mouse pointer. The recogniser and the editor's pinch handling are already multi-pointer end to end, so real touch is a wiring change the day GPUI dispatches touch events — until then there is nothing honest to wire |
| CI (Wave 8 infrastructure) | DONE — `.github/workflows/workflow.yml`: cached release build + full test suite + stripped binary artifact on every push. Per the repo AGENTS.md, CI is the only build environment (no local builds without the owner's approval); fixes are diagnosed from CI logs |
| UI redesign (Wave 8) | DONE — the flat ~110-button toolbar became an app bar (brand, File/Edit, panel toggles, frame/FPS chips) + labelled tool-group panels, built on a shared kit (`src/ui_kit.rs`: `tool_group`, `section_label`, `group_row`, `top_bar`, `side_panel`, `info_chip`, `brand`); sidebar rows gained figure colour dots; the canvas is framed with a permanent hint line; timeline and graph/audio panels restyled. Every button kept its id, label and listener |

**GUI visually verified (2026-11-08).** The user downloaded the
CI artifact, ran the editor on their own Linux PC and sent a
screenshot: the app opens, the demo project loads, the selected
figure shows its joint handles, the timeline shows its frames.
The long-standing "never seen on a real display" caveat is
closed. (The same screenshot showed how dense the old toolbar
was — the direct motivation for the UI redesign above.)

**Test state:** 344 tests green in CI (run for dd2d785: "344
passed; 0 failed"). The three failures that run shook out of
the Wave 7 WIP are worth remembering: the corpus test dispatched
the new Stick Nodes fixtures to the Pivot importer, the v4
lengths test assumed a two-figure demo project (it has one),
and the MCP clip test expected a 2-frame clip from a timeline
its own animated bake had already grown to 4.

## What remains (honest list, 2026-11-08)

- **Drawing depth.** The original's raster engine is still much
  deeper: tiles, on-canvas text, flood-fill at scale, a far
  larger brush system. The port has a solid vector-stroke subset.
- **VFX volume.** 21 deterministic clip kinds + weather vs the
  original's ~6,600-line system. (The MCP kind gap is closed:
  `add_fx_clip` accepts all 14 kinds and `fx_list`/`fx_move`/
  `fx_resize`/`fx_duplicate`/`fx_delete` manage clips.)
- **.piv text side tables** — closed in Wave 13: the bitmap
  font's lit pixels are written as tiny square glyph outlines
  in the format's own encoding (machine cross-verified with
  the original's Python codecs), and text is recovered on
  import. Limit: glyphs are the port's blocky bitmap shapes,
  not real font outlines. (The v5 image side tables and
  per-frame bends round-trip too.)
- **Movieclip depth.** Clips are flat pose lists: no nested clip
  timelines, and sprite/text tracks inside Stick Nodes clips are
  not converted into ClipDefs.
- **3D depth.** 3D exists as an editing session that bakes to
  native 2D figures; there is no persistent 3D in the document
  model and no 3D rendering in exports.
- **Real multi-touch** — blocked on GPUI (see the verdict
  above), not on this codebase.
- **Stick Nodes leftovers** (spec §17): instance style/filter
  data, smart-stretch, connectors and angle locks are consumed
  or approximated, camera records and sounds are parsed but not
  modelled, and ZIP-packaged projects are out of scope.
- **Localisation polish.** 28 languages of machine-produced
  strings vs the original's 27 human-maintained ones.

## Wave 9 (delivered, 2026-11-08 — CI run 42403754079 green, 421 tests)

The current wave, landed in the working tree and pending CI
verification (per the repo rule, CI is the first build/test any
of this gets). Themes:

- **Stabiliser** — stroke stabilisation for drawing (smoothing
  live input before it becomes a stroke).
- **4 new VFX kinds** — four more deterministic clip kinds on
  top of the existing 14 in `src/vfx.rs`.
- **Audio DSP** — deeper audio processing on top of the
  peak/RMS/normalise/trim/split/crossfade set.
- **Graph interpolation modes** — more interpolation options
  for graph channels beyond the Hermite default.
- **3D shading** — shaded rendering for the 3D posing session's
  preview (3D remains a session that bakes to 2D).
- **Stats** — project/session statistics surfaced in the UI.
- **Contact-sheet render** — a contact-sheet export (frames as
  a single tiled image) alongside the spritesheet.
- **Pivot probe** — a diagnostic read of `.piv` files (header/
  structure inspection without a full import).
- **Range ops + UI** — more timeline range operations, wired
  into the redesigned tool groups.
- **Weather panel** — a dedicated panel for the weather
  controls (rain/snow/fireflies) instead of toolbar buttons.
- **MCP round 3** — more MCP tools on top of the current 49.

Nothing in this section is CI-verified yet; statuses move to
the per-wave tables above once the run is green.

## Wave 10 (delivered, 2026-11-08 — CI run 42409358708 green, 622 tests)

Twenty code agents, aimed at the top remaining gaps listed in
docs/COMPARISON.md. Everything below was implemented in Wave 10
and verified at integration per the repo rule (CI is the first
build/test any of this gets): after three fix commits (a dangling
#[cfg(test)] in export.rs, a stale i18n test, a wrong buffer-size
assumption in the determinism test), CI run 42409358708 passed
with 622 tests green.

- **Live audio mixing** — playback now mixes ALL clips audible
  at the playhead; the first-audible-clip-only, single-stream
  playback noted in the Wave 4 section is gone.
- **Export audio mixing** — MP4/WebM exports mix every audio
  clip into the soundtrack at its frame offset, instead of
  muxing only the first clip.
- **Fog weather** — a fourth weather kind (Fog) beside rain,
  snow and fireflies: model variant, rasterizer and editor
  canvas rendering, and a Fog segment in the weather panel's
  segmented control.
- **Brush library round 3** — three new preset brushes in
  `brush_library()` (`src/paint.rs`).
- **3 new VFX kinds** — Fireworks, Aurora, Comet: 21
  deterministic clip kinds total (was 18). MCP's
  `parse_clip_kind` covers all 21.
- **Graph auto-tangents** — Catmull-Rom auto-tangents for
  graph channels: engine support plus an editor op that
  smooths a channel's tangents in one action.
- **Pose mirroring** — an editor op that mirrors the current
  pose, plus the matching 3D pose mirror in `src/fig3d.rs`.
- **Movieclip loop modes** — clips gain a loop mode in the
  model (Loop / PingPong / Once) with an editor op to cycle
  it; clips are still flat pose lists (no nested timelines).
- **.nodes export fidelity** — writer improvements scoped to
  what the format grammar can express; the exact wins are the
  implementing agent's to report, and the documented losses
  in the Wave 7+8 table stand until integration confirms
  otherwise.
- **MCP round 4** — three new tools (`mirror_pose`,
  `smooth_graph_tangents`, `render_contact_sheet`): 54 tools
  total.
- **Localisation** — the 12 newest UI keys (added in Wave 9)
  are now genuinely translated into German, French, Italian,
  Portuguese (pt-BR), Russian and Chinese (zh-Hans); they were
  English-filled placeholders in those languages. (Japanese,
  Korean, Hindi and Urdu have no tables in this project — or
  in the original — so there was nothing to translate into;
  adding them would be a structural change.) The remaining
  languages still carry the English strings for those 12 keys.
- **UI wiring** — the Guides overlay gains a toggle; timeline
  chips render through `ui::frame_chip`, inspector steppers
  through `ui::stepper_row`, and the Home project list
  through `ui::file_row`; the HUD gains a pure `chip_specs`
  seam so its chip layout is testable without GPUI.
- **Export determinism tests** — integration tests asserting
  byte-identical exports for identical projects, guarding
  the determinism claim the architecture notes make.

Nothing in this section is CI-verified yet; statuses move to
the per-wave tables above once the run is green, and
docs/COMPARISON.md is refreshed after that run.

## Wave 11 (delivered 2026-11-08, CI run 42427232939 green on 78f1665, 689 tests)

Twenty code agents, aimed at the top remaining gaps listed in
docs/COMPARISON.md. Two CI fix commits followed the wave commit
(a GifDecoder BufRead bound in the range-export test, and a
rect-shape test that expected counter-clockwise winding where
the implementation's clockwise rotation is correct). Both
investigations resolved: .stknds camera implemented both ways,
segment gradients audited as already implemented.

- **Drawing shape tools** — line, rectangle and ellipse stroke
  generators in `src/paint.rs`, so exact shapes can be drawn
  instead of only freehand strokes, plus an MCP `draw_shape`
  tool exposing them to agents.
- **3 new VFX kinds + full GUI coverage** — Galaxy, Prism,
  Vortex: 24 deterministic clip kinds total (was 21). The GUI
  also gains creation buttons for ALL 24 kinds; previously
  only 10 kinds had buttons and the rest were MCP-only.
- **Audio FX** — echo, lowpass and silence-trim join the audio
  engine's processing set, with editor ops and buttons in the
  audio panel beside normalise/split/crossfade.
- **Editor depth** — graph channels can be baked to frames
  (channels become real keyframed poses), and a pose can be
  copied from one figure to another.
- **Movieclip loop-mode button** — the clips panel gains a
  loop-mode control; the Wave 10 `cycle_clip_loop` op finally
  has a button.
- **3D camera presets + auto-framing** — preset camera views
  for the 3D session, plus `frame_camera` auto-framing of the
  current figure.
- **Graph key reduction** — `reduce_keys` drops graph keys
  that do not change the channel's curve beyond a tolerance,
  simplifying over-keyed channels.
- **Frame-range exports** — GIF and PNG-sequence exports can
  be limited to a frame range instead of the whole timeline.
- **.stknds camera round-trip (investigation — outcome:
  implemented)** — the camera grammar proved out from the
  original's decoder, adapter and renderer sources, so
  `.stknds` camera records are now imported (mapped to
  CameraKeys via the adapter's own transform) and exported
  (evaluated camera baked to per-frame records). Only
  bindings and procedural wobble remain unmodelled, as
  upstream.
- **Segment gradient audit (investigation — outcome:
  already implemented)** — segment gradients (`color2`)
  already render along the segment axis in the rasterizer
  and as real linearGradient defs in SVG; no production
  change was needed. Test coverage was extended (far-end
  sampling + a byte-identity twin test).
- **Localisation, round 2** — the 12 newest UI keys (added in
  Wave 9, translated into 6 languages in Wave 10) are now
  translated into the remaining 19 languages, so all 28
  languages carry real translations for these keys.
- **MCP round 5** — three new tools: 57 tools total.
- **Determinism + timecode** — the byte-determinism proof
  (Wave 10) extends to PNG sequences and contact sheets; the
  timeline bar gains a timecode chip.

Nothing in this section is CI-verified yet; statuses move to
the per-wave tables above once the run is green, and
docs/COMPARISON.md is refreshed after that run.

## Wave 12 (delivered 2026-11-08, CI run 42442484756 green on 0139fa0, 779 tests)

Twenty code agents, aimed at the top remaining gaps listed in
docs/COMPARISON.md — depth in the five systems rather than new
feature families. Everything below is implemented in Wave 12;
CI verification happens at integration (per the repo rule, CI
is the first build/test any of this gets). Two CI fix commits
followed (a test closure lifetime in projects.rs, and a wrong
slug literal in the fig3d pose-library test); run 42442484756
is green with 779 tests.

- **Brush model depth** — the brush model gains opacity and
  scatter, a Spray brush joins the library, and spray offsets
  are deterministic (seeded, like every other effect in the
  project). Honest limit: opacity/scatter are model data for
  now — the stroke recorder and renderers do not apply them
  yet, so Spray currently draws like a wide round pen;
  wiring them into Stroke is a follow-up.
- **Shape generators round 2** — polygon, star and arrow
  stroke generators join line/rectangle/ellipse, plus stroke
  mirroring across `src/paint.rs`, `src/draw_ops.rs` and the
  editor.
- **3 new VFX kinds + GUI buttons** — Glitter, Wave, Shard:
  27 deterministic clip kinds total (was 24), with GUI
  creation buttons for the new kinds.
- **Storm weather** — a fifth weather kind (Storm) beside
  rain, snow, fireflies and fog.
- **FX clip presets on disk** — FX clip presets can be saved
  to and loaded from disk, not just created in place.
- **Audio pitch shift + compressor** — pitch shift and a
  compressor join the audio engine's processing set, with
  editor ops and buttons in the audio panel.
- **Graph tangent presets + overlay** — tangent presets
  (Smooth, EaseIn, EaseOut, EaseInOut, Linear) for graph
  keys, plus a multi-channel overlay view-model for the
  graph panel.
- **Editor stamp ops for shapes** — the editor gains stamp
  operations that place the shape generators' output as
  strokes in one action.
- **MP4/WebM frame-range exports** — MP4 and WebM exports
  can be limited to a frame range, like the Wave 11 GIF/PNG
  ranges. Range exports are video-only; this is documented
  in the exporter.
- **3D pose library + two-bone IK** — a pose library for
  the 3D session, plus two-bone IK in the 3D rig. The first
  IK agent proved the blocker precisely: the crate had no
  matrix→Euler extraction. A follow-up agent added
  `mat3_to_euler` (with a documented gimbal-lock convention)
  and the full solver on top of it; the solver's tests were
  replayed in a Python simulation with zero error before
  landing. CI is still the first Rust compile, as always.
- **MCP round 6** — new tools including `draw_polygon`,
  `draw_star`, `graph_tangent_preset` and FX preset tools;
  storm and the three new kinds join `set_weather` /
  `add_fx_clip`; MP4/WebM frame-range formats join the
  export formats: 62 tools total.
- **Determinism, round 3** — the byte-determinism proof
  extends to APNG and JPEG sequence exports.
- **Project library management** — the project library
  gains rename, duplicate and filter operations.
- **Motion statistics** — motion statistics are computed
  and surfaced alongside the existing project statistics.

Nothing in this section is CI-verified yet; statuses move to
the per-wave tables above once the run is green, and
docs/COMPARISON.md is refreshed after that run.

## Wave 13 (in progress, 2026-11-08)

Twenty code agents, aimed at the loose ends earlier waves
left documented — the spray render wiring, backend features
that never reached the GUI, and the unfinished range-export
family — plus one long-standing investigation. Everything
below is implemented in Wave 13; CI verification happens at
integration (per the repo rule, CI is the first build/test
any of this gets), so no item here is claimed verified yet.

- **Spray wiring, end to end** — the Wave 12 brush model
  gains its missing half: `Stroke` records a spray mark
  (`SprayMark`) when a scatter brush draws, and the
  rasterizer, the SVG exporter and the editor canvas all
  render spray strokes as deterministic dots computed from
  one shared `spray_dots` source. Spray strokes render
  dots only — no polyline is drawn through them. The
  Wave 12 "model data only" limit is closed.
- **Stroke simplification** — Douglas-Peucker
  simplification for freehand strokes in `src/paint.rs`,
  exposed over project strokes in `src/draw_ops.rs` and
  over MCP.
- **FX clip colour gradients** — FX clips gain an end
  colour (`color2`) alongside the existing colour, so a
  clip's particles render as a colour gradient.
- **Per-clip audio gain, end to end** — audio clips carry
  a gain in the model; the live playback plan and the
  MP4/WebM export mux both honour it, so the gain set in
  the panel is the gain that plays and exports.
- **Audio split at silences** — the audio engine computes
  a clip's non-silent ranges, and an editor op splits a
  clip at its silences into separate clips.
- **Graph transforms** — graph channels gain time-mirror
  and value-scale transforms.
- **FX preset GUI buttons** — the Wave 12 on-disk FX clip
  presets gain save/apply buttons in the FX panel;
  presets are no longer reachable only over MCP.
- **Graph overlay wired in** — the Wave 12 multi-channel
  overlay view-model is painted in the graph panel.
- **Graph preset preview curves** — the tangent preset
  buttons gain preview curves, so a preset's shape is
  visible before it is applied.
- **3D IK conveniences** — one-call arm and leg chain
  helpers over the Wave 12 two-bone IK solver in
  `src/fig3d.rs`, so posing code names a limb instead of
  raw node indices.
- **.piv v5 text side tables — INVESTIGATION SUCCEEDED**:
  the grammar proved fully determined by the original's
  source; text tables are written with bitmap-pixel glyph
  outlines and recovered on import. The encoding algorithm
  round-trips through the original's Python codecs and a
  fixture produced by the original's own writer decodes
  exactly (scripts/verify_text_tables.py). Rust-side tests
  are CI-pending like the rest of this section.
- **APNG/JPEG frame-range exports** — APNG and JPEG
  sequence exports can be limited to a frame range,
  completing the range family (GIF/PNG in Wave 11,
  MP4/WebM in Wave 12).
- **MCP round 7** — three new tools, including stroke
  simplification: 65 tools total.
- **Shell session continuity** — session-continuity
  helpers in the shell layer, restoring session state
  across editor relaunches.
- **Determinism, round 4** — the byte-determinism proof
  extends from the demo project to a rich project (storm
  weather, FX clips and spray strokes).
- **Home panel** — the Home panel gains a duplicate
  button per saved project (the Wave 12 project-library
  backend, wired in) and a most-active-figure line from
  the motion statistics.

Nothing in this section is CI-verified yet; statuses move
to the per-wave tables above once the run is green, and
docs/COMPARISON.md is refreshed after that run.
