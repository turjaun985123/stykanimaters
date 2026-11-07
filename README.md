# StykAnimate-RS

An end-to-end Rust port of [StykAnimate](https://github.com/filippocalippo/StykAnimate-App)
(the free, open-source stick figure animation app), rebuilt with
[Rust](https://www.rust-lang.org) and [GPUI](https://www.gpui.rs)
(`gpui = "=0.2.2"` from crates.io). GPL-3.0-or-later, like the original.

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
SHA256SUMS). Download it from the repo's Actions tab and run
`./stykanimate-rs`. The editor has been run and screenshot-verified
this way on a real Linux desktop.

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
  channels, FX clips, audio clips, sprites/texts and weather.
- **Posing** — forward kinematics for the whole rig; dragging a
  hand or foot runs two-bone IK (unreachable-target clamping,
  per-limb bend direction); dragging any other joint rotates its
  segment (FK aim); dragging the hips moves the figure; a mirror
  op flips the current pose (the 3D session mirrors poses too).
- **Editor UI** (`src/app.rs`, `src/ui_kit.rs`, `src/ui_*.rs`) —
  redesigned chrome: a slim **app bar** (brand, File/Edit, panel
  toggles, frame/FPS chips) over labelled **tool-group panels**
  instead of a flat button wall. Panels for the keyframe graph,
  audio, FX clips, movieclips, colour picker, easing picker,
  inspector, export centre and help; a per-figure **dopesheet**
  above the timeline; a state HUD above the framed canvas; status
  messages arrive as a **toast stack**; the figures sidebar has
  colour dots, lock/solo/opacity controls and opacity steppers.
  Transport includes |< < > >| stepping and a loop range.
- **Timeline depth** — per-frame durations and easing (per-frame
  override of the project easing), frame copy/paste, inbetween,
  block shift, range reverse, loop ranges, motion-path display
  with path smoothing, onion skin 1–3 in both directions, and a
  **graph engine** (Hermite channels for root X/Y, joint angles
  and segment lengths, with shortest-path angle wrap and
  Catmull-Rom auto-tangents) applied live
  and editable in a visual graph panel.
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
  brushes, brush/eraser tools), per-point pressure widths, smoothing, nib
  shapes, serpentine fill and erasers; strokes render in the
  editor and all exports.
- **VFX** (`src/vfx.rs`, `src/fx_edit.rs`) — 21 deterministic,
  seeded clip kinds (Burst, Sparkle, Smoke, Beam, Aura, Trail,
  Confetti, ShockRing, Ember, Fountain, Orbit, Lightning,
  Bubbles, Leaves, Meteor, Petals, Splash, Spiral, Fireworks,
  Aurora, Comet) anchored to joints, with an FX timeline engine
  (list/move/resize/duplicate/delete) in the GUI and over MCP;
  plus continuous **weather** (rain, snow, fireflies, fog)
  rendered in the canvas and the rasterizer.
- **Audio** — clips placed at frames; playback mixes **all**
  clips audible at the playhead, synced via `ffplay`, and
  MP4/WebM exports mix every clip into the soundtrack at its
  frame offset; a waveform panel with per-clip normalise /
  split / crossfade and a real mixdown export.
- **Movieclips** — capture a figure's timeline as a clip, edit it
  in a dedicated clip session panel; clip figures play their own
  animation inside any project with a selectable loop mode —
  **Loop**, **PingPong** or **Once**.
- **3D mode** (`src/fig3d.rs`) — a real 3D FK rig with Euler
  per-joint rotation, orbit camera and perspective projection,
  living in an editing session: pose A/B, animated bakes into
  timeline frames, one-click bake to a native 2D figure, and a
  persistent 3D figure library (`./library3d`).
- **Formats** — the grammar is ported from the original app's own
  codecs and cross-verified against them in both directions:
  - *Import:* `.piv` v2/v3/v4/v5 (v5 incl. image side tables,
    per-frame lengths and bends), `.stk`, Stick Nodes `.nodes`
    figures, and Stick Nodes animation containers
    `.nodemc`/`.stknds` (sprites/texts modelled as keyframed
    tracks with real pixel extraction; joins baked at import).
  - *Export:* `.piv` v2/v3/v4/v5, `.stk`, `.nodes`, `.stknds`,
    plus PNG sequence, GIF, MP4, WebM, APNG, JPEG sequence, SVG
    and spritesheets.
  - A cross-format fixture corpus (`tests/fixtures/`) generated
    with the original's encoders guards the importers.
- **Projects & shell** — named projects under `./projects/`, a
  Home panel with recents, a recovery snapshot, native open/save
  dialogs, save/load as JSON (`project.styk.json`), light/dark
  themes, and **28 languages** (the original's 27 + Bengali)
  cycled from the toolbar.

## Drive it from an AI agent (MCP)

The `mcp_server` binary speaks MCP (JSON-RPC 2.0 over stdio,
protocol `2024-11-05`) and exposes **54 tools**: project/figure/
pose editing, camera (with rotation), graph keys, FX clip
management, weather, audio, drawing, 3D sessions, movieclips,
per-frame bends, pose mirroring, graph tangent smoothing,
import/export in every supported format, and
rendering — including `render_frame_base64`, so an agent can see
the frame it just made, and `render_contact_sheet`. Full tool catalogue and the client
configuration snippet live in [docs/MCP.md](docs/MCP.md).

## Architecture notes

- The crate is a library (`src/lib.rs`); the GPUI editor
  (`src/main.rs`) and the MCP server (`src/bin/mcp_server.rs`)
  are two thin binaries over the same engine.
- `model.rs` has no UI dependencies; `editor.rs` holds editor
  state and every document operation; the GPUI layer translates
  mouse events and buttons into calls.
- Exports are deterministic: same project → same bytes, in CI
  as on a desktop.
- `vendor/xattr` is a byte-identical copy of crates.io
  `xattr 0.2.3` with one patched constant: it referenced
  `libc::ENOATTR`, which current `libc` removed on Linux; the
  vendored copy maps it to `ENODATA` there. Wired through
  `[patch.crates-io]` in `Cargo.toml`.

## Honest limits

- **No touch input.** GPUI 0.2.2 has no touch API at all —
  proven from the crate source in [docs/TOUCH.md](docs/TOUCH.md).
  The gesture recogniser and pinch-to-camera code are written
  and tested but unreachable from hardware until GPUI ships
  touch events; a touchscreen arrives as one emulated mouse.
- **`.piv` text side tables cannot be written.** The format
  stores vector glyph outlines; this port's text is a 3×5 bitmap
  font with no outlines to write. (Image side tables, per-frame
  lengths and bends *do* round-trip.)
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
- **Visual checks happen on the user's machine.** This project's
  dev VM uses a virtual display with software Vulkan where the
  GUI renders black screenshots; the editor is verified visually
  from CI artifacts run on a real desktop (first verified
  2026-11-08).

## Licence

GPL-3.0-or-later, matching the original project.
