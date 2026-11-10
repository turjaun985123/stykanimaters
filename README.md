# StykAnimate-RS

An end-to-end Rust port of [StykAnimate](https://github.com/filippocalippo/StykAnimate-App)
(the free, open-source stick figure animation app), rebuilt with
[Rust](https://www.rust-lang.org) and [Rune](https://github.com/anomalyco/rune),
anomalyco's native GPU-rendered Rust UI framework. Rune has no
crates.io releases yet, so it is pinned by git revision
(`rev = "87caf2a3a2d9e3fb6172f5a62997748dbf25748e"` for both
`rune` and `rune-kit` in `Cargo.toml`). The editor was previously
built on GPUI 0.2.2; the GPUI shell was removed in full on
2026-11-08 and the migration record lives in
[docs/RUNE_MIGRATION.md](docs/RUNE_MIGRATION.md).
GPL-3.0-or-later, like the original.

It is a single native binary: a full stick-figure animation editor
(posing with FK + two-bone IK, timeline, camera, drawing, VFX,
audio, 3D posing sessions, movieclips), a CPU rasterizer that
exports PNG/GIF/MP4/WebM/APNG/JPEG/SVG/spritesheets with no GPU or
display needed, first-class Pivot and Stick Nodes format support,
and an MCP server that lets an AI agent drive the whole engine.

## Get it — the CI binary

Every push to `master` runs the **Build, Test & Binary** GitHub
Actions workflow: a cached release build, the full test suite in
the release profile, and a stripped binary package uploaded as
the `stykanimaters-linux-x86_64` artifact (both binaries —
`stykanimate-rs`, the editor, and `mcp_server` — plus
SHA256SUMS.txt). Download it from the repo's Actions tab and run
`./stykanimate-rs`. (The earlier GPUI editor was run and
screenshot-verified this way on a real Linux desktop; the Rune
shell has been run and screenshot-verified the same way
(2026-11-08) — the headless checks below are how it is
verified day to day.)

## Run it — and check it headlessly

```sh
./stykanimate-rs                 # the editor (1440×900 window, 1280×720 min)
```

Every Rune binary carries built-in headless tooling, so the UI
can be rendered and inspected with no display at all:

```sh
./stykanimate-rs --snap out.png   # render the UI offscreen to a PNG
./stykanimate-rs --tree out.json  # dump the element tree as JSON
# also: --scene NAME (named snapshot state), --script FILE
# (scripted input), --at SECONDS, --crop KEY (one element's rect)
```

Honest status: `--tree` and the in-process `Runtime`
interaction tests need no GPU; `--snap` renders through the
real GPU pipeline and needs a wgpu adapter — on a GPU-less
machine that means a software Vulkan driver — which is
what the headless CI workflow uses; its snapshot suite runs
green there.

## Build from source

> **Repo rule (see `AGENTS.md`): no local cargo builds without the
> owner's explicit approval for that run.** Local builds are heavy
> and the owner does not want them; GitHub Actions CI is the
> project's only sanctioned build/test environment, and fixes are
> diagnosed from CI logs. If you have approval, the commands are
> the usual ones:
>
> ```sh
> cargo build --release --locked --bins   # what CI builds
> cargo test --release --locked           # what CI tests
> cargo run --release                     # the editor
> ```
>
> The Rune dependencies are fetched from GitHub at the pinned
> revision on first build (network needed once; `--locked`
> keeps the resolution reproducible after that).

System libraries (Ubuntu/Debian) — this is the exact install line
from `.github/workflows/workflow.yml`, plus `ffmpeg` (used for
MP4/WebM export and audio playback):

```sh
sudo apt-get install -y --no-install-recommends \
  libfontconfig1-dev libxkbcommon-dev libxkbcommon-x11-dev \
  libwayland-dev libvulkan-dev libasound2-dev libudev-dev \
  cmake clang ffmpeg
```

Native file dialogs shell out to `zenity` or `kdialog` when
present and fall back to a folder-based flow when neither exists.

## What works today

- **Document model** (`src/model.rs`) — plain serializable data,
  separate from editor and UI state: figures are node/segment rigs
  (parent, fixed length, relative rest angle, head flag, IK bend
  side, per-node styles and media); a pose is a root position plus
  one relative angle per node, plus optional per-node **bends**
  (curved segments, rendered through an arc midpoint); a project
  is figures + frames + fps + stage size + camera keys, graph
  channels, FX clips, audio clips, sprites/texts, weather and
  subtitles (timed cues with an optional per-cue speaker
  credit and a project-wide subtitle style).
- **Posing** — forward kinematics for the whole rig; dragging a
  hand or foot runs two-bone IK (unreachable-target clamping,
  per-limb bend direction); dragging any other joint rotates its
  segment (FK aim); dragging the hips moves the figure; a mirror
  op flips the current pose (the 3D session mirrors poses too).
- **Editor UI** — a Rune shell (`src/main.rs` +
  `src/stage_view.rs` + `src/chrome_view.rs`, declared as
  modules of the binary; the library stays UI-free). What it
  has today: a stage canvas drawing **all
  figures** at the displayed frame — per-node colours and
  thickness, shape nodes, head rings, and onion skins with the
  previous shell's exact alpha falloff — with **drag posing**
  through the engine's own `begin_drag`/`drag_to`/`end_drag`
  ops (FK aim, two-bone IK, root move), camera/draw drag modes,
  and wheel zoom. Drawing and dragging share one
  `view_transform` whose screen↔stage roundtrip is
  unit-tested, so the figure tracks the cursor exactly. The
  app bar carries Save / Load / Undo / Redo / + Figure /
  Onion; the timeline is clickable numbered **frame chips**
  with Add / Duplicate / Delete frame; transport (prev /
  play-pause / next) plays through the core's `advance_play`
  (fps, per-frame durations and loop range honoured); the
  keyboard covers space, arrows, Ctrl+Z / Ctrl+Shift+Z /
  Ctrl+Y and Delete-segment; a status line shows frame, fps,
  zoom and the core status message. Everything shown calls a
  real engine op — there are no placeholder controls.
  The GPUI-era panels (workspaces, tool groups, graph, audio,
  FX, export centre, Home…) were deleted with GPUI and have
  been rebuilt section by section in `src/rune_sec/` —
  figures, segment/style inspectors, camera, scene, easing,
  graph, draw, strokes, FX, audio, clips, library, 3D,
  export, home, menus, command palette, settings, help,
  toasts, keyframes/layers, text, projects and stats panels,
  plus `Runtime` interaction tests and named `--snap` scenes
  (the rebuild record is §6 of docs/REDESIGN_PROGRESS.md;
  remaining gaps in docs/GAP-ANALYSIS.md).
- **Timeline depth** — per-frame durations and easing (per-frame
  override of the project easing), frame copy/paste, inbetween,
  block shift, range reverse, loop ranges, motion-path display
  with path smoothing, onion skin 1–3 in both directions, and a
  **graph engine** (Hermite channels for root X/Y, joint angles
  and segment lengths, with shortest-path angle wrap,
  Catmull-Rom auto-tangents, tangent presets — Smooth, EaseIn,
  EaseOut, EaseInOut, Linear — with preview curves, time-mirror
  and value-scale channel transforms, and a multi-channel
  overlay painted in the panel) applied live
  and editable in a visual graph panel. Motion statistics are
  computed alongside the project statistics.
- **Camera** — per-frame keys (centre, zoom **and rotation**) with
  interpolation; camera mode pans by dragging; a follow target can
  track a figure. The editor and every exporter render through
  the same camera, rotation included.
- **Figure building** — add/delete segments (children-first, root
  protected), length/thickness editing, per-segment colour
  override and styles (taper, caps, gradients, opacity), shape
  segments (lines, circles, regular polygons 3–6 sides), sprite
  and text segments (built-in 3×5 bitmap font for text), figure
  presets (humanoid, quadruped, mini), and a local figure library
  (`./library/*.json` plus built-ins).
- **Drawing** — Draw mode with an expanded brush library (preset
  brushes, brush/eraser tools, brush opacity and scatter, and a
  Spray brush whose strokes record a spray mark and render
  as deterministic dots — dots only, no polyline — in the
  editor and every exporter, all from one shared spray-dot
  source), per-point pressure widths, smoothing,
  Douglas-Peucker stroke simplification, nib shapes,
  serpentine fill and erasers; shape generators cover
  line, rectangle, ellipse, polygon, star and arrow, with
  stroke mirroring and editor stamp ops for shapes; strokes
  render in the editor and all exports.
- **VFX** (`src/vfx.rs`, `src/fx_edit.rs`) — 27 deterministic,
  seeded clip kinds (Burst, Sparkle, Smoke, Beam, Aura, Trail,
  Confetti, ShockRing, Ember, Fountain, Orbit, Lightning,
  Bubbles, Leaves, Meteor, Petals, Splash, Spiral, Fireworks,
  Aurora, Comet, Galaxy, Prism, Vortex, Glitter, Wave, Shard)
  anchored to joints, with an FX timeline engine
  (list/move/resize/duplicate/delete) over MCP,
  per-clip colour gradients (a start colour and an end
  colour), and FX clip presets saved on disk;
  plus continuous **weather** (rain, snow, fireflies, fog, storm)
  rendered in the rasterizer and every export. (The FX
  creation buttons and panels are Rune shell sections in
  `src/rune_sec/`.)
- **Audio** — clips placed at frames, each with a per-clip
  gain; playback mixes **all** clips audible at the playhead
  at their gains, synced via `ffplay`, and MP4/WebM exports
  mix every clip into the soundtrack at its frame offset and
  gain; per-clip normalise / split
  (including split at silences) / crossfade plus pitch shift
  and compressor (audio engine and editor ops; the audio
  panel and waveform view are Rune shell sections in
  `src/rune_sec/`), and a real mixdown export.
- **Movieclips** — capture a figure's timeline as a clip
  (the clip-session editing panel is
  `src/rune_sec/clips_panel.rs`); clip figures play their own
  animation inside any project with a selectable loop mode —
  **Loop**, **PingPong** or **Once**.
- **3D mode** (`src/fig3d.rs`) — a real 3D FK rig with Euler
  per-joint rotation, orbit camera and perspective projection,
  living in an editing session: pose A/B, animated bakes into
  timeline frames, one-click bake to a native 2D figure, a
  persistent 3D figure library (`./library3d`), a 3D pose
  library, and a two-bone IK solver with one-call arm/leg
  chain helpers.
- **Formats** — the grammar is ported from the original app's own
  codecs and cross-verified against them in both directions:
  - *Import:* `.piv` v2/v3/v4/v5 (v5 incl. image side tables,
    per-frame lengths and bends), `.stk`, Stick Nodes `.nodes`
    figures, and Stick Nodes animation containers
    `.nodemc`/`.stknds` (sprites/texts modelled as keyframed
    tracks with real pixel extraction; joins baked at import).
    Image assets import from PNG or JPEG files, detected by
    content rather than by file name.
  - *Export:* `.piv` v2/v3/v4/v5, `.stk`, `.nodes`, `.stknds`,
    plus PNG sequence, GIF, MP4, WebM, APNG, JPEG sequence, SVG
    and spritesheets. MP4/WebM can also be exported for a frame
    range (video-only), and APNG/JPEG sequence exports take a
    frame range too. The raster exports (PNG sequence, GIF,
    MP4, WebM, APNG, JPEG sequence and spritesheets) also
    accept an optional output scale — a true re-render at
    the scaled size, not a resample. The export bundles
    (the whole timeline in every format, in one call) accept
    a scale as well: it scales only the bundle's PNG sequence
    member, every other member is unaffected, and an invalid
    scale refuses the call before anything is written.
    MP4/WebM exports also have an optional encode-phase
    progress hook in the library: a caller passing a callback
    receives the encoder's own frame counts as the encode
    runs, ending with a single finished event for the video
    encode (an MP4's audio mux runs afterwards and can still
    fail the export). The hook is core-level only for now —
    neither the export panel nor the MCP server surfaces it.
  - *Subtitles:* cues import and export as SRT and WebVTT;
    speaker credits ride WebVTT voice tags (SRT has no speaker
    syntax, and an SRT export reports how many credits it
    could not write).
  - A cross-format fixture corpus (`tests/fixtures/`) generated
    with the original's encoders guards the importers.
- **Projects & shell** — named projects under `./projects/`
  with rename, duplicate and filter in the project library
  (`src/projects.rs`), a recovery snapshot and
  session-continuity helpers (`src/shell.rs`), native open/save
  dialogs, save/load as JSON (`project.styk.json`), and
  **28 languages** (the original's 27 + Bengali; 662 UI
  strings in each language's table) in `src/i18n.rs`.
  These library features survived the Rune
  migration untouched, and the Home panel and settings
  surfaces that present them are Rune shell sections in
  `src/rune_sec/`.

## Drive it from an AI agent (MCP)

The `mcp_server` binary speaks MCP (JSON-RPC 2.0 over stdio,
protocol `2024-11-05`) and exposes **381 tools**: project/figure/
pose editing, camera (with rotation), graph keys and tangent
presets, FX clip management and FX presets, weather (including
storm), audio, drawing (including polygon and star shapes
and stroke simplification),
3D sessions, movieclips,
per-frame bends, pose mirroring, graph tangent smoothing,
import/export in every supported format (including MP4/WebM
frame-range formats), and
rendering — including `render_frame_base64`, so an agent can see
the frame it just made, and `render_contact_sheet`. Full tool catalogue and the client
configuration snippet live in [docs/MCP.md](docs/MCP.md).

## Architecture notes

- The crate is a library (`src/lib.rs`) with **no UI
  dependencies**; the Rune editor (`src/main.rs`, with
  `stage_view.rs` / `chrome_view.rs` / `rune_sec/` as binary
  modules) and the MCP server (`src/bin/mcp_server.rs`)
  are two thin binaries over the same engine.
- `model.rs` has no UI dependencies; `editor.rs` holds editor
  state and every document operation; the Rune shell translates
  pointer events and buttons into calls.
- Exports are deterministic: same project → same bytes, in CI
  as on a desktop; the byte-determinism integration proof
  covers APNG and JPEG sequence exports and now also runs a
  rich project (storm weather, FX clips, spray strokes), not
  just the demo project.
- The UI framework is a **git dependency, not a crates.io
  one**: `rune` + `rune-kit` pinned at rev `87caf2a` (see
  above). The GPUI-era `[patch.crates-io]` vendored `xattr`
  copy was deleted with GPUI — nothing is patched now.

## Honest limits

- **No touch input.** Under GPUI 0.2.2 this was proven
  impossible — the toolkit had no touch API at all
  ([docs/TOUCH.md](docs/TOUCH.md)). The gesture recogniser and
  pinch-to-camera engine code are still written and tested;
  under Rune a `TouchPhase` type exists but touch-driven
  posing is **unproven** (not yet audited against the pinned
  rev), so for now a touchscreen arrives as one emulated
  mouse, as before.
- **`.piv` text side tables carry bitmap glyphs.** Text is
  written into v5 files with each bitmap pixel as a tiny square
  glyph outline (cross-verified with the original's codecs) and
  recovered on import — readable, but blocky by construction:
  the port has no vector font outlines to write. (Image side
  tables, per-frame lengths and bends round-trip too.)
- **3D is a session, not a document feature.** 3D figures are
  posed in the 3D mode and baked to native 2D figures; exports
  render the 2D result.
- **Depth gaps vs the original** (see
  [docs/GAP-ANALYSIS.md](docs/GAP-ANALYSIS.md)): the original's
  drawing and VFX systems are still larger, movieclips are flat
  pose lists (no nested clip timelines), some Stick Nodes
  instance data (styles/filters, smart-stretch, sounds) is
  consumed but not modelled, and the 28 localisations are
  machine-produced where the original's are human-maintained.
- **Visual checks: headless now, desktop still gold.**
  The GPUI editor could only be verified visually from CI
  artifacts run on a real desktop (this project's dev VM
  renders black screenshots), first verified 2026-11-08.
  The Rune shell can render itself offscreen (`--snap` PNG,
  `--tree` element dump) and be driven by in-process
  `Runtime` tests — `--snap` on the GPU-less CI runner
  (software Vulkan) is exercised by the headless workflow,
  and the shell has had its first real-desktop run
  (user-verified by screenshot and video, 2026-11-08).
  Desktop remains the gold check for what the headless
  suite cannot hear — sound above all.

## Licence

GPL-3.0-or-later, matching the original project.
