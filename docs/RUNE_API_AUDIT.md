# RUNE_API_AUDIT.md — anomalyco/rune as the GPUI replacement

Phase A audit for the Rune migration. Source: `~/workspace/rune-src` at rev
`87caf2a3a2d9e3fb6172f5a62997748dbf25748e` (the pin candidate). Everything
below was **read from that source**; line citations are `file:line` inside the
rune repo unless a StykAnimate path is named. Items that could not be proven
from source are marked **UNPROVEN** — never assumed.

**Headline verdict:** Rune covers every editor need the GPUI app has, and two
things GPUI 0.2.2 never gave us: a first-class vector-shape canvas for the
stage, and a built-in headless render/inspect pipeline that makes real
screenshot QA possible in CI. The price is real: Rune has **no releases** —
apps pin `dev` revs and the API moves daily (see Q14).

---

## Q1 — App entry, window options, state/view model

- Entry is one call: `rune::run(AppSpec::new(title, size, init, view))`
  (`crates/rune/src/tooling.rs:183` `pub fn run`; `AppSpec::new` at
  `tooling.rs:45`). The smallest app is `examples/hello/src/main.rs:26-31`:
  a `State` struct, a free `fn view(s: &State, cx: &mut Cx<State>) -> El`,
  and `fn main() { rune::run(AppSpec { .. }) }`.
- `AppSpec<S>` fields (`tooling.rs:17-40`): `title: &'static str`,
  `size: (f32, f32)`, `min_size: Option<(f32, f32)>` — "the smallest window
  the user can resize to … (None: 420×400)" (`tooling.rs:20-21`) — `init:
  fn() -> S`, `view: fn(&S, &mut Cx<S>) -> El`, `scenes` (named snapshot
  states), `fonts`/`families`, and optional hooks `setup`, `poll`, `on_key`.
  Literals end with `..AppSpec::new(..)` so new fields don't break apps
  (`tooling.rs:43-44`).
- The window title can change at runtime: `Tx::set_title(..)`
  (crates/rune/README.md, "App lifetime"). For apps that outlive one window
  there is `rune::App` with a builder (`.keep_running()`, `.windowless()`,
  `.window_options(..)`, `AppHandle` for cross-thread updates) — the editor
  does not need it; plain `rune::run` suffices.
- **Update model:** handlers (`.on_press(|s: &mut S| ..)`) are *queued and
  run between frames with `&mut S`; then the view re-runs and the diff
  retargets property tracks* (crates/rune/README.md, "View API"). Hover and
  press never re-run the view — they only retarget tracks. So: mutate state
  in a handler → next frame's `view(state)` reflects it. This maps 1:1 onto
  our `EditorCore` ops called from listeners today.

## Q2 — Custom 2D drawing (the stage) — THE critical question

**There is a real vector canvas, and it is data, not a paint callback.**

- `rune::canvas(shape: Shape) -> El` (`crates/rune/src/el.rs:717`) and
  `El::shape(Shape)` paint a `Shape` inside an element's box.
- `Shape` builder API (`crates/rune/src/shape.rs`): `Shape::new()` (:57),
  `.stroke(points, width, color)` polyline (:62), `.line(a, b, width, cap,
  color)` (:89), `.fill(points, color)` simple polygon, ear-clipped (:94),
  `.stroke_path(&Path, width, cap, color)` (:103), `.fill_path(&Path,
  color)` (:111), `.view_box(w, h)` to draw in your own coordinates
  stretched to the element (:121). `Path` supports `move_to` (:273),
  `line_to` (:284), `quad_to` (:289), `cubic_to` (:304), `close` (:320) —
  quadratic/cubic Béziers are flattened by the framework.
- `examples/shapes/src/main.rs` is the proof app: a 160-point filled
  waveform envelope + polyline stroke (:36-61), 24 thick round-cap meter
  lines (:63-77), Bézier paths and a concave star fill (:79-101) — all
  rebuilt from state every frame (`poll` advances `s.t`, :158-161).
- **No circle primitive exists in `Shape`** (full `pub fn` list read —
  there is no `circle`/`arc`). Joint circles are either (a) a small filled
  polygon (16–24 points — trivially generated), or (b) an `el()` with
  `radius = size/2`, the idiom the examples use for knobs/dots
  (`examples/hello/src/main.rs:15`, `examples/todo/src/main.rs:23`).
- **Performance model:** the renderer is "one instanced uber-quad pipeline
  and one draw call per frame"; a stroke segment is *one analytic capsule
  instance* (`KIND_SEGMENT`), fills are triangles antialiased on edges, and
  "a shape can change every frame" because shapes bypass the glyph atlas
  (crates/rune/README.md, "Renderer"). Cost scales with segment count, not
  element count. The crate's own pinch benchmark: ~2,000 cards / ~6,000
  elements at 2× costs 13.7 ms CPU/frame when handlers re-run the view and
  2.0 ms with `tx.live_only()` (README, "Canvas handlers"). A stage Shape
  of a few hundred segments is far below either figure.
- **Caveat:** text cannot be drawn *inside* a Shape — labels are separate
  `text()` elements positioned over/around the canvas (the shapes example
  labels its canvases exactly this way, `examples/shapes/src/main.rs:116`).
- **Caveat 2:** "Translucent strokes blend twice where segments overlap at
  a joint; fills do not overlap" (README, "Renderer"). Use round caps and
  don't stack translucent segments at joints if colour purity matters.

**Stick-figure verdict:** one `canvas()` element for the whole stage (or one
per figure) whose `Shape` is built from the solved rig each commit: ~13
thick round-cap `line()`s for segments, small filled polygons for joints,
`Path` for motion paths/accessories. That is *less* element churn than the
GPUI canvas produced, and camera pan/zoom can ride `Live` values with
`tx.live_only()` so drags skip view re-runs entirely (Q3).

## Q3 — Pointer input

- `.on_press(..)`, `.on_double_press(..)`, `.on_drag(|s, drag, tx| ..)`
  with a `Drag` carrying `x, y` in the element's **own layout points**,
  `dx, dy` travel in window points, `start_x, start_y`, `w, h` captured at
  press, `phase` (Down/Move/Up), `clicks`, `cancelled()`, and a per-gesture
  scratch `data` field (README, "Canvas handlers" coordinate table;
  a complete worked drag — a slider — is `examples/settings/src/main.rs`,
  `speed_control`, `on_drag` with `Phase::Down/Move/Up` and cancel-restore).
- Pointer capture is built in ("hit-testing on presented rects, pointer
  capture", README "Input"); press fires on release inside the element;
  scrolling under a held press cancels it.
- `.on_pointer_move_local(..)` gives local points; `.on_pointer_move(..)`
  gives window points (`PointerEvent` also carries `window_x, window_y`).
- **Right-click:** `.on_secondary_press(|s, point, tx| ..)` receives
  window-point coordinates (also ctrl-click on macOS) without invoking
  primary actions; `.on_secondary_press_local` gives local points too
  (`crates/rune/src/el.rs:992` and `:1007`).
- **Wheel/pinch on a canvas:** `.on_wheel(|s, e: WheelEvent, tx| -> bool)`
  hears deltas *before* any scroll view, with local pointer position,
  mods, phase, momentum; returning true captures the rest of the gesture.
  `.on_magnify` (pinch; on Linux/Windows a Ctrl+wheel pinch is routed to
  it), `.pointer_buttons([PointerButton::Middle])` for middle-button drags
  (README, "Canvas input"; event types re-exported at
  `crates/rune/src/lib.rs:144-148`).
- Hover: `.on_hover` / `.on_hover_change`, plus hover/press overlay fills
  as pure view properties (`.hover(..)`, `.press(..)` in examples/settings).
- Cursors: `.cursor(Cursor::Pointer)` and the system set, plus custom PNG
  cursors with hot spots (README, "Cursors") — the GPUI app never had
  resize/text cursors; Rune has them (`Cursor` re-exported at lib.rs).

## Q4 — Keyboard and focus

- App-level: `AppSpec::on_key: Option<fn(&mut S, &KeyEvent, &mut Tx) ->
  bool>` (`tooling.rs:39`). Element-level: `El::on_key` (`el.rs:983`),
  `El::on_input_key` for fields (`el.rs:969`), `El::on_key_up`.
- Routing is fully specified in 9 steps (README, "Key routing"): IME
  composition first, default-button Return, focused field, focus
  navigation, window `on_key` (with `tx.focused()` naming the taker),
  selection copy keys, then scroll keys.
- Shortcuts: `Chord::parse("mod+shift+t")` resolves to ⌘ on macOS / Ctrl
  elsewhere, layout-aware via `tx.key_base()` and `tx.physical_key()`;
  `Keybind` supports per-platform alternatives (`lib.rs:149-154`).
  `KeyEvent` carries the typed character; `Mods::cmd` is the platform
  primary modifier.
- Focus model: `.focusable(true)`, `.tab_index(n)`, `.focus_group()` with
  arrow-key roving + type-select, `.focus_ring(w, offset, color)` shown
  for keyboard only, `.modal(..)` traps Tab and restores the opener
  (README, "Focus and modals"). This is strictly more than GPUI gave us.

## Q5 — Text input

Two layers, both real:

- **Core:** `text_input(&edit, style)` bound to a `TextEdit` value in app
  state via `.on_edit(..)`; `.on_submit(..)` on Enter. The editor provides
  "caret, selection, word and line motion, select all, copy, cut and
  paste, undo, and IME preedit through winit" (README, "Text input";
  module `crates/rune/src/edit.rs`, `TextEdit` re-exported at lib.rs).
  Key routing step 1 gives Return/Escape to an active IME composition
  before anything else.
- **Kit:** `TextField`, `TextInput`, `TextArea` components
  (`crates/rune-kit/src/lib.rs`, `pub use text_input::{TextArea,
  TextField, TextInput}`).
- Honest note: PLAN.md risk #1 names desktop IME on three platforms as the
  framework's own top risk area (PLAN.md §9.1) — it is implemented from the
  native protocols, but this is the area to test first on Linux.

## Q6 — Images (export thumbnails/previews)

- `ImageCache` (`crates/rune/src/image.rs`): `.load(bytes)` (:176),
  `.svg(bytes, size_pt, scale)` (:189, resvg-backed), `.file(path, ..)`
  (:211), `.rgba(width, height, bytes)` (:227) — the last is exactly our
  case: our own rasterizer's RGBA frames.
- Display: an `Image` exposes `.pixels() -> &Pixels` (`image.rs:51`);
  elements take `.pixels(&Pixels)` (`crates/rune/src/el/visuals.rs:196`)
  or `.pixels_fit(&Pixels, SurfaceFit)` (:204) with aspect handling
  (`aspect_fit`, `el/layout.rs:34`). The renderer has an RGBA image kind
  in the same single draw call (README, "Renderer").
- So Home project cards and export previews can show real rendered frames
  produced by the existing deterministic rasterizer — no new decode stack
  (PNG/JPEG via the `image` crate dep, `crates/rune/Cargo.toml`).

## Q7 — Timers and playback (30 fps)

Three mechanisms, all verified:

- **`AppSpec::poll`** — "Called at the start of every frame with seconds
  since start (virtual when headless); true when state changed"
  (`tooling.rs:33-35`). `examples/shapes` drives continuous animation
  exactly this way (`poll` sets `s.t = t`, returns true).
- **`tx.wake_after(Duration)`** — schedules a poll on the app clock; "the
  shell sleeps until the earliest deadline without a timer thread"
  (README, "App hooks"). A playback loop = `wake_after(33 ms)` + poll
  advancing the frame counter; `AppHandle::update` from a thread also
  wakes the loop.
- **`.looping(LoopSpec)`** for periodic motion (springs/tweens in
  `rune-motion`) — for animated decoration, not for the transport clock.
- The view *can* depend on time: poll-fed state is ordinary state, so
  `view` reads the current time/frame from `S`. Headless runs use a
  virtual clock, so time-dependent views stay deterministic in tests.
- Note the idle contract: "an idle window draws nothing" — playback must
  actively request frames via poll/wake, which suits a transport that
  starts and stops.

## Q8 — Headless render-to-PNG and tree inspection (our QA strategy)

**This is built into every Rune binary** via `crates/rune/src/tooling/`:

- CLI flags parsed in `tooling/args.rs`: `--snap out.png` (:129) "render
  offscreen (no window) at --at seconds" (usage text, args.rs:~236),
  `--tree out.json` (:168) "element tree with animated values",
  `--scene NAME` (applies an `AppSpec::scenes` fn first), `--script
  FILE` (:143) driving scripted input (clicks, `double-click`, `type`,
  `key`, `paste`, `scroll`, `fling` — README, "Tooling"), `--at SECONDS`,
  `--frames/--strip` for frame sequences, `--crop KEY` to snapshot one
  element's rect, `--scale`.
- Mechanism (`tooling/headless.rs`): `pub fn headless(..)` (:25) runs an
  "offscreen deterministic frame loop" on a virtual-clock `Timeline`
  (:507-557), rendering through the real GPU renderer into an offscreen
  target — `Gpu::try_with_features(..)` (:139), `Offscreen::with_format`
  (:144) — and writes PNGs (:203-209). Detection: `rune::is_headless()`
  (lib.rs). `RUNE_HEADLESS_WAIT` env delays start for profilers
  (headless.rs, top of `headless_configured`).
- **In-process equivalent for unit tests:** `Runtime::new(state, view,
  size, env)`, `rt.frame(dt)`, `rt.pointer_down/up/move`, `rt.key(..)`,
  `rt.find(key) -> (x, y)`, `rt.presented(key) -> rect`,
  `rt.elements() -> Vec<ElementInfo>` (`runtime/elements.rs:196,379,396`;
  worked example: the test inside `examples/hello/src/main.rs:36-54`,
  which taps the switch and asserts the knob's animated travel). This path
  exercises view+layout+input **without a GPU**.
- CI design consequence: logic/layout assertions run GPU-free in
  `cargo test` via `Runtime`; pixel snapshots via `--snap` need a wgpu
  adapter — on a GPU-less Linux runner that means a software Vulkan
  driver (lavapipe). **UNPROVEN** on our GitHub runner until Phase B runs
  it; the fallback (snapshots on the user's PC, tree/logic checks in CI)
  still beats today's GPUI situation, where headless frames were pure
  black and *nothing* visual was verifiable anywhere but a human's screen.

## Q9 — Layout

- Layout is **Taffy 0.15** (CSS flexbox + grid + block) behind the
  `rune-taffy` adapter: "Keeps Taffy's Flexbox, CSS Grid, Block,
  out-of-flow positioning, and intrinsic sizing" (`crates/rune-taffy/src/
  lib.rs:3`); the rune crate depends on taffy with features `flexbox,
  grid, block_layout, content_size` (`crates/rune/Cargo.toml`).
- Builders: `row()`, `col()`, `el()` with `.gap/.padding/.grow/.shrink/
  .w_full/.h_full/.min_h(..)` etc.; out-of-flow via `.absolute(left, top)`
  (`crates/rune/src/el/layout.rs:257`), `.pin(l, t, r, b)` (:263), grid
  placement via `.grid_cell(column)` (:252).
- CSS gotchas are documented in the crate README ("Layout"): `.grow` is
  `flex: g 1 0` (basis 0 — a grown scroll viewport in an auto-height
  column collapses; bound an ancestor or use `.min_h`), flex items
  default to content-min size (put `.min_h(0.0)` down the chain to a
  scroll viewport), and `.virtualize` requires `.growth(..)`.
- Verdict for the editor shell: app bar (row), side panels (fixed-width
  columns + one grown centre), timeline rows, inspector property rows —
  all plain flex; the stage is a grown centre element holding one canvas.
  Nothing in the Studio layout needs more than this.

## Q10 — Overlays, menus, popovers, dialogs, tooltips

Both a core layer and kit components exist:

- **Core:** a popup-menu module (`rune::PopupMenu`, `PopupAnchor`,
  `popup_menu_supported()` — lib.rs re-exports), native OS menus
  (`rune::MenuItem`, `native_menu` module), separate overlay *windows*
  (`OverlaySpec`, `ConfiguredOverlay`, `run_windows` — exercised by
  `examples/overlay/src/main.rs`, which is about multi-display OS
  overlays, not in-window popovers), in-window anchored overlays
  (`El::anchored(key, ..)` follows an anchor's drawn rect, README), and
  `.modal(click_outside, close)` which traps Tab, handles Escape, makes
  the background inert and restores the opener (README, "Focus and
  modals"). Tooltips are `El::help` — "a tooltip anchors itself to the
  element as drawn" (README, coordinate-spaces note).
- **Kit:** `Menu` + `MenuEntry`/`MenuItem` + `context_menu`,
  `Popover`, `Tooltip` (+ `HoverCard`), `Dialog`, `Combobox`, `Select`,
  `Toast`/`ToastRegion`, `PopupList` (`crates/rune-kit/src/lib.rs`
  re-exports: menu :108-111, overlay :114-122, dialog :76, toast,
  select :134, combobox :73-75).
- This directly replaces the Wave 15 "menus are a full-width row" defect:
  menus are floating, anchored, dismiss-on-outside-click layers.

## Q11 — Scrolling

- `.scroll(ScrollSpec { stick_bottom })` on any element makes it a
  viewport with a runtime-owned fractional offset: wheel 1:1, fling
  decay, follow-the-end (README, "Scrolling"; used in
  `examples/todo/src/main.rs`, the list). `.scroll_x()` / `.scroll_xy()`
  for horizontal/both axes with AppKit-style predominant-axis gestures
  (README, "Horizontal scrolling").
- Programmatic control is by element key through `Tx`: `scroll_into_view
  (key)`, `scroll_to(key, x, y)`, `scroll_to_with(key, x, y, spring)`,
  `scroll_to_instant`, `scroll_to_x/y`, `scroll_page(key, pages)`,
  `scroll_center(..)` (README, "Naming elements"; CHANGELOG dev notes).
- Visible scrollbars: kit `ScrollBar` component + style
  (`rune-kit/src/lib.rs`, `pub use scroll_bar::{ScrollBar, ScrollBarStyle,
  scroll_bar}`), plus system scroll-bar reporting (`rune::ScrollBars`).
- Virtualisation for long lists: `.virtualize(..)` growth stacks
  (README, "Virtual rows") — relevant to the projects list and clip lanes
  if they ever grow large; not needed at current scale.

## Q12 — rune-i18n: what it is, and whether we use it

- `crates/rune-i18n/src/lib.rs` (crate docs): message catalogs as flat
  JSON per locale, turned into embedded blobs by a **build-script codegen**
  (feature `codegen`); keys are compile-time-checked `const fn` lookups
  (a typo fails the build); `I18n` holds the locale in an atomic with
  source-locale fallback; `{{name}}` templates; CLDR plurals and number
  formatting from ICU4X data; `system_languages()` + `negotiate()` match
  the OS's languages to supported locales.
- **Recommendation: ignore it for this migration.** StykAnimate already
  has a working 28-language flat table (`src/i18n.rs`) with an invariant
  test (every key in every table). rune-i18n would replace a solved
  problem with a build.rs step + ICU4X dependencies, buying plural rules
  the editor's strings barely use. The `tr()` call sites don't care which
  table backs them. Revisit only if plural-correct strings become a real
  requirement.

## Q13 — Dependency: exact lines, pin form, heavy deps, toolchain

- **Rune is not consumed from crates.io.** Its workspace version is
  `0.0.1` and the project states: "Rune has no releases yet; consumers
  pin `dev` revs" (`CHANGELOG.md`, preamble); "Apps pin `dev` revs"
  (rune repo `AGENTS.md`). In-repo examples use path deps
  (`examples/hello/Cargo.toml`: `rune = { path = "../../crates/rune" }`).
  For StykAnimate the form is a git pin at the audited rev:

  ```toml
  rune     = { git = "https://github.com/anomalyco/rune", rev = "87caf2a3a2d9e3fb6172f5a62997748dbf25748e" }
  rune-kit = { git = "https://github.com/anomalyco/rune", rev = "87caf2a3a2d9e3fb6172f5a62997748dbf25748e" }
  ```

  (rune-kit depends on `rune` with `default-features = false`
  internally — `crates/rune-kit/Cargo.toml` — so the app controls the
  core feature set through its own `rune` line.)
- **The framework's own dependency weight** (`crates/rune/Cargo.toml`):
  **wgpu ~30** (Vulkan/DX12/Metal; `custom` backend on Apple), **winit
  0.30.13** (x11 + wayland on Linux), **taffy 0.15**, **parley 0.12 +
  swash 0.2 + fontique 0.12** (text: Parley layout, swash rasterisation,
  embedded Inter 4.1; fontconfig is dlopen'd at runtime, so no build
  headers), `image` (png/jpeg), optional **resvg** (feature `svg`, on by
  default), **accesskit** (feature `accessibility`, default), on Linux:
  wayland-client/protocols, x11rb, xkbcommon, and optional zbus+tokio
  (feature `dbus`, default — desktop portal services). **No Skia, no
  tiny-skia**: rendering is one custom wgpu uber-shader; glyphs come from
  an R8 atlas. Compare GPUI 0.2.2, which also rides wgpu/winit-class
  deps — the weight is comparable, the surface is not.
- **Toolchain:** the rune repo pins `channel = "1.99.1"` with clippy +
  rustfmt (`rust-toolchain.toml`) — the same rustc this machine and CI
  already use for StykAnimate. Workspace edition is **2024**
  (`Cargo.toml`, `[workspace.package] edition = "2024"`); our crate is
  edition 2021 — a git-pinned 2024-edition *dependency* builds fine under
  a 1.99 toolchain regardless of our own edition (**verified by reading**
  the edition fields; the compile itself is CI's to confirm in Phase B).

## Q14 — Risks

- **API stability is the dominant risk.** No releases; `dev` is
  fast-forward-only and apps pin revs precisely because a rewrite would
  break them (rune repo `AGENTS.md`, "Changes"). The git log shows
  multiple commits per day (2026-11-07/08: motion, menus, hover, diff,
  Luau…), and the changelog's stated purpose is listing "changes that can
  move an app's output without a code change". Mitigation: pin the exact
  rev; treat upgrades as deliberate, changelog-read events — the same
  discipline as our GPUI `=0.2.2` pin, except the pinned thing moves
  faster. Types apps build as literals (incl. `AppSpec`, `ScrollSpec`)
  can gain breaking fields; the repo documents that list (AGENTS.md).
- **macOS-first centre of gravity.** The default Apple path is a custom
  Metal backend; many features are macOS-only or macOS-led (native menu
  details, Dock reopen, `on_open_url`, smart magnify). Linux is a real
  target — winit x11/wayland, wgpu Vulkan, D-Bus portals, AT-SPI — but
  some behaviours differ by shell: closing destroys the window on
  Wayland, activation without a token is best-effort, the window title
  lives in client-side decorations (winit `wayland-csd-adwaita-notitle`
  feature). PLAN.md risk #1 (IME/text quality per platform) and risk #5
  (platform feel drift) are the authors' own warnings.
- **Text on Linux:** font smoothing defaults to `Off` (= antialiased,
  browser-like) on Linux and web; `On` (CoreText-style dilation) is
  arithmetic on outlines and works everywhere (README, "Text"). Good —
  but our users will see the Linux default, not the macOS screenshots.
- **Web target:** experimental M5 (PLAN.md §13, WEB.md). **Irrelevant**
  to this migration — StykAnimate is a desktop app; noted and dropped.
- **Headless pixels need a GPU adapter object** (Q8) — CI viability with
  a software Vulkan driver is UNPROVEN until Phase B tries it.
- Counterweight: the framework is the UI toolkit of a shipping product
  family (OpenCode), is developed in the open with an unusually thorough
  in-repo verification culture (goldens, traces, perf gates — PLAN.md
  §6), and its AGENTS.md enforces file-size/folder discipline similar to
  what our waves converged on.

---

## MIGRATION DESIGN — StykAnimate on Rune

### What does NOT change

The engine freeze survives the migration untouched: `model.rs`,
`editor.rs` (EditorCore + all ops), formats (`pivot.rs`, `nodes*.rs`),
exporters, audio, VFX, fig3d, graph, paint, MCP server (65 tools), i18n
table, determinism suite. None of them depend on GPUI today — only
`src/app*`, `src/ui/*`, `src/main.rs` and the `gpui` dependency line do.
That is the whole blast radius.

### Entry (replaces `src/main.rs`, 11 lines)

```rust
fn main() {
    rune::run(AppSpec::new("StykAnimate RS", (1440.0, 900.0),
        || EditorState::new(), view));
}
```

with `AppSpec { min_size: Some((1280.0, 720.0)), scenes: &[…],
poll: Some(poll), on_key: Some(on_key), .. }`. (Window title is the
`AppSpec` title; runtime changes via `Tx::set_title`.) The Studio
directive's main.rs asks — initial 1440×900, min 1280×720, real title —
are one struct literal here, not a research project.

### State

`struct EditorState { core: EditorCore, view-state… }` — EditorCore is
embedded, not rewritten. Every GPUI listener body today calls exactly one
core op; in Rune the same body hangs off `.on_press(|s: &mut
EditorState| s.core.<op>())`. The 248-id preservation inventory carries
over as a **248-command inventory**: same ids as element keys
(`.key("save")` etc.), same ops, verified by the same grep discipline.

### Stage rendering (per Q2)

- One grown centre element holding `canvas(stage_shape(&core))`, where
  `stage_shape` builds ONE `Shape` per commit from the solved poses:
  segments as round-cap lines at their real thickness/colour, joints as
  small filled polygons, heads as stroked polygon circles, motion paths
  as thin `Path` strokes, onion skins as alpha-blended duplicates.
- Camera: pan/zoom live in `Live` values on a wrapper element
  (`live_offset`/`live_scale`); canvas handlers (`on_wheel`,
  `on_magnify`, `on_drag` with `tx.live_only()`) move the camera without
  re-running the view — the 2.0 ms/frame path from the crate's own
  benchmark instead of 13.7 ms.
- Posing drags: `on_drag` on the canvas gives local (unzoomed) points —
  exactly the `to_stage` conversion the GPUI code does by hand today.
  Hit-testing joints stays our own math against solved poses, as now.
- HUD/overlays: absolutely-positioned elements (`.pin`/`.absolute`)
  inside the stage container — viewport controls top-right, mode chips
  bottom-left — handler-free where inert (`.inert()`), so they never eat
  pointer events meant for the canvas.

### Timeline

Elements, not canvas: a header row (transport buttons, timecode text,
FPS field), a ruler row of frame cells (keyed `frame-{n}`), and one
dopesheet row per figure with keyframe diamonds as small rotated
elements. Shift-click range selection keeps its current op; the ruler
seek is `on_drag` on the ruler element. The Wave 15 `tl-collapse`
behaviour is a boolean in state; height animation comes free from
springs if wanted. Loop band = an absolutely-positioned tinted element
behind the cells.

### Panels, menus, inspector

- App bar: one 40px row — brand, kit `Menu`s (File/Edit/View/Help) as
  anchored popovers, workspace segmented control (kit
  `SegmentedControl`), right cluster. Menus float; the layout never
  moves (the Wave 15 defect is gone by construction).
- Figures sidebar: kit `Sidebar`/`SidebarRow` shell or plain rows —
  colour dot (small rounded el), name, eye/lock/solo icon buttons,
  opacity via a hand-rolled drag field (below).
- Inspector: tabbed property rows; numeric values use a **scrub field
  built on `on_drag`** — the pattern is proven by `examples/settings`'
  speed slider (drag with grab-offset, Escape cancels and restores,
  `tx.instant()` while scrubbing). There is **no ready-made Slider in
  rune-kit's public inventory** (full `pub use` list read) — this is the
  one control we build ourselves, once, as a reusable builder.
- Command palette: kit `Combobox`/`PopupList` + a text field over our
  own static command table (the 248 commands) — the piece GPUI blocked
  for want of a text-input component is unblocked by Q5.

### Where the 41 GPUI `src/ui` components' jobs go

- **Kit covers:** Button/IconButton, Badge, Card, Checkbox, Switch,
  Radio, SegmentedControl, Select, Combobox, Menu/context menus,
  Popover, Tooltip/HoverCard, Dialog, Toast, TextField/TextInput/
  TextArea, List, Table, Tree, ScrollBar, Progress/Spinner, Accordion/
  Collapsible, Sidebar/TabStrip/Toolbar/SplitView shells, Keybind
  display, shortcut recorder. That is the large majority of our 41.
- **Tokens:** `rune::tok` (OKLCH dark/light themes, space/radius/type/
  motion scales) + kit's `Kit`/`Palette` theming replace our `kit.rs`;
  our 2.0 palette values port as a `Theme::from_json` theme (themes are
  data in Rune) or as `tok`-style constants.
- **Ours to write (small, pure builders as today):** stage canvas
  (above), timeline ruler/dopesheet, graph editor (a canvas Shape of
  curves + draggable key diamonds as elements), waveform (one canvas
  Shape — trivially), colour swatch grid (rounded els), FX kind browser
  (grid of buttons), easing curve previews (canvas Shape).
- **Stateful-widget tier:** Rune's model needs no GPUI-style entity
  tier — text fields are state-in-`S` (`TextEdit`), drags are handler
  state (`Drag::data`), scroll offsets are runtime-owned. The Studio
  plan's `src/ui/widgets/` exception tier evaporates.

### Headless CI design (the verification upgrade)

1. `cargo test` gains `Runtime`-driven tests: build the real editor
   view, `find()` controls by key, tap/type/drag, assert state — the
   preservation inventory becomes *executable*, not grep-only.
2. The binary gains `--snap` scenes for free from `AppSpec::scenes`:
   one scene per workspace + menu-open + inspector-selected states;
   CI renders PNGs and a human (or a pixel-diff against goldens)
   reviews them. `--tree` JSON asserts layout facts mechanically —
   e.g. stage height ≥ 360 at 1280×720, status strip present — turning
   the Studio layout budget into CI assertions instead of hopes.
3. Caveat carried from Q8: pixel snapping needs a wgpu adapter in CI
   (software Vulkan) — Phase B must prove or route around this; the
   `Runtime` tests (item 1) need no GPU and already exceed everything
   CI could check in the GPUI app.

### What Rune cannot do that the GPUI app did — honest list

Verified gaps are few; most former "GPUI can't" items invert (Rune
*adds* popovers, tooltips, text input, drag, cursors):

- **No circle/arc primitive in `Shape`** — workaround is trivial
  (polygon or rounded el), but it is a workaround. (Verified, Q2.)
- **No text inside a canvas Shape** — labels are elements; for a stage
  with many text labels this means more elements than a paint-callback
  API would need. Our stage has few labels. (Verified, Q2.)
- **No kit Slider** — hand-roll once from the settings example's proven
  `on_drag` pattern. (Verified by reading rune-kit's full export list.)
- **Translucent stroke joints blend twice** — a rendering caveat for
  onion skins/ghosts drawn as strokes; fills avoid it. (Verified, Q2.)
- **Touch input:** Rune exports a `TouchPhase` type (lib.rs), but
  touch-driven posing was **UNPROVEN** in this audit — GPUI had none at
  all, so nothing is lost; not investigated further per scope.
- **The GPUI app's one real advantage: it exists.** 58,767 lines with
  912 green tests run on GPUI today. The migration re-skins the shell
  (`src/app*` + `src/ui/*` ≈ the top layer only), but every screen must
  be rebuilt and re-verified — hence the phase plan, and hence keeping
  the GPUI build green on its own branch until the Rune shell reaches
  the 248-command inventory in full.

---

## Report (master format)

1. **Files changed:** created `docs/RUNE_API_AUDIT.md` (this file) —
   the only file touched. Rune source and all other StykAnimate files
   untouched; no cargo run, no build, no commit.
2. **Public API added/changed:** none (audit only). The dependency lines
   proposed for Phase B are quoted in Q13.
3. **Ids:** none added/moved/removed — the audit instead defines the
   migration's preservation unit: the existing 248-id inventory becomes
   a 248-command/key inventory under Rune.
4. **Tests added:** none. The audit designs the future test surface
   (Q8 / Migration Design): `Runtime`-driven interaction tests +
   `--snap`/`--tree` CI checks.
5. **Verified by reading:** app entry/AppSpec/hooks (tooling.rs,
   examples/hello, settings, todo, shapes, overlay — all read in full);
   Shape/Path API (shape.rs export list + shapes example); pointer,
   keyboard, focus, scroll, overlay systems (crates/rune/README.md,
   which documents behaviour against the same source, cross-checked
   against el.rs handler signatures); image stack (image.rs, pixels.rs,
   el/visuals.rs); headless mechanism (tooling/args.rs + headless.rs);
   kit inventory (rune-kit/src/lib.rs full export list + Cargo.toml);
   rune-i18n (crate docs); dependency weight + toolchain (crates/rune/
   Cargo.toml, workspace Cargo.toml, rust-toolchain.toml); stability
   signals (CHANGELOG.md preamble, AGENTS.md, git log, PLAN.md §9/§13).
6. **Only CI can decide:** that the git-pinned rune + rune-kit rev
   compiles into our crate (edition-2024 dependency under our
   toolchain); whether `--snap` pixel rendering works on the GitHub
   runner's software Vulkan stack; real frame times with a full stage.
7. **Only a screenshot can decide:** Linux text weight (smoothing Off
   default), Wayland vs X11 window chrome/title behaviour, and whether
   the Rune shell *feels* like the Studio target at 1280×720 — the
   first two are explicitly flagged in Q14.
8. **Requests to other agents:** Phase B should (a) prove the pin
   compiles with a hello-world Rune shell behind the existing binary
   name before any screen is ported, (b) run one `--snap` on CI to
   settle the software-Vulkan question early, (c) test Linux IME in a
   text field before the command palette is designed around it.
9. **Open risks:** API churn on a pinned-but-fast-moving `dev` rev
   (Q14); the migration's true cost is rebuilding every screen in the
   shell layer — the engine is safe, the shell is a rewrite; Rune's
   macOS-first polish means Linux rough edges will be found by us, not
   by their screenshots.
