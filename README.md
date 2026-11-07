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
- **Save / Load** — the project serializes to `project.styk.json`
  next to where you launched the app.
- **PNG export** (`src/export.rs`) — a CPU rasterizer (2x
  supersampled) writes `export/frame-0001.png`, `...` for every
  frame; no GPU or display needed, so it also runs in CI.

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
`.piv` / `.stk` / Stick Nodes import, the figure builder (custom
rigs, shapes, text, images), layers and the advanced keyframe
timeline with easing, camera moves and follow, particles, GIF/MP4/
WebM export, multiple colour/style tools per segment, sounds, and
the home/project library screen.
