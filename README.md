# StykAnimate-RS

An end-to-end Rust port of [StykAnimate](https://github.com/filippocalippo/StykAnimate-App)
(the free, open-source stick figure animation app), rebuilt with
[Rust](https://www.rust-lang.org) and [GPUI](https://www.gpui.rs)
(`gpui = "=0.2.2"` from crates.io). GPL-3.0-or-later, like the original.

## Run it

```sh
cargo run            # debug build
cargo run --release  # what you actually want day to day
cargo test           # model + export tests (FK, IK, rasterizer)
```

Linux build dependencies (Ubuntu/Debian): `libfontconfig1-dev
libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libvulkan-dev
libasound2-dev libudev-dev libdrm-dev libgbm-dev libxcb1-dev
libx11-dev cmake clang pkg-config`.

## What works today

- **Document model** (`src/model.rs`) — plain serializable data, kept
  separate from editor and UI state like the original: figures are
  node/segment rigs (parent, fixed length, relative rest angle, head
  flag, IK bend side); a pose is a root position plus one relative
  angle per node; a project is figures + frames + fps + stage size.
- **Posing** — forward kinematics for the whole rig; dragging a hand
  or foot runs two-bone IK (with unreachable-target clamping and
  per-limb bend direction); dragging any other joint rotates its
  segment (FK aim); dragging the hips moves the figure.
- **GPUI editor** (`src/app.rs`, `src/editor.rs`) — canvas with joint
  handles and head rings, onion skin of the previous frame, frame
  timeline (click to jump), Play/Pause at the project fps, add /
  duplicate / delete frames, add figures (colour-cycled), undo
  snapshots, and a status line. A starter project with a 5-frame wave
  is loaded on first launch.
- **Figure builder** — select a joint, then add or delete segments
  (children-first, root protected), adjust segment length and
  thickness, and cycle a per-segment colour override. Definition
  edits stay consistent across every frame and are undoable.
- **Figure presets** — besides the humanoid: a two-segment-limb
  quadruped (spine, neck, head, tail, IK feet) and a compact
  7-node mini figure with FK-aimed limbs.
- **Inbetween** — inserts a tween frame halfway to the next frame;
  angles interpolate along the shortest circular path.
- **Camera** — per-frame camera keys (centre + zoom) with linear
  interpolation between keys. Camera mode pans by dragging the
  canvas; zoom buttons key the current frame. The editor view and
  both exporters render through the same camera.
- **Layers sidebar** — figures listed as layers: select, reorder
  (draw order), hide/show, delete. Plus redo, frame copy/paste,
  per-frame durations, editable fps, and keyboard shortcuts
  (arrows, space, N, I, O, C, Ctrl+Z / Ctrl+Shift+Z).
- **Tween playback** — optional interpolation between authored
  frames with linear / smooth / ease-in / ease-out easing, in the
  editor, GIF and MP4 exports.
- **Shapes** — segments can be lines, circles, or regular polygons
  (3-6 sides): shields, wheels, signs.
- **Particles** — deterministic burst / sparkle / smoke effects
  anchored to a joint on a chosen frame, rendered identically in
  the editor and every export.
- **Local figure library** — save figure definitions to
  `./library/*.json` and insert them into any project; three
  built-ins included.
- **More exports** — MP4 (H.264, via the system `ffmpeg`) and a
  spritesheet PNG join the PNG sequence and GIF.
- **Themes** — dark and light editor palettes.
- **Save / Load** — the project serializes to `project.styk.json`
  next to where you launched the app.
- **PNG + GIF export** (`src/export.rs`) — a CPU rasterizer (2x
  supersampled) writes `export/frame-0001.png`, `...` for every
  frame, or one looping `export/animation.gif` at the project fps;
  no GPU or display needed, so it also runs in CI.

## Architecture notes

- `model.rs` has no UI dependencies and is fully unit-tested.
- `editor.rs` holds editor state and every document operation; the
  GPUI layer only translates mouse events and buttons into calls.
- `vendor/xattr` is a byte-identical copy of crates.io `xattr 0.2.3`
  with one patched constant: it referenced `libc::ENOATTR`, which
  current `libc` removed on Linux; the vendored copy maps it to
  `ENODATA` there. Wired through `[patch.crates-io]` in `Cargo.toml`.

## Not yet ported (the road from here)

The original app is large; this is the working core loop, not the
whole feature list. Still to come, roughly in the original's shape:
`.piv` / `.stk` / Stick Nodes import (needs real fixture files to
reverse the binary layouts byte-for-byte — dropping a Pivot-made
`.stk` into this repo is the fastest way to unblock it), sprite
and text segments, a keyframe graph editor with layer folders and
property channels, particle clips with beams/auras, WebM/APNG/SVG
export, sounds, movieclips, 3D figures, localisation, and the
home/project library screen. See `docs/GAP-ANALYSIS.md` for the
full measured comparison.
