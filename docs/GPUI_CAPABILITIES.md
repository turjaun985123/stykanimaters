# GPUI 0.2.2 — Verified Capability Matrix (Wave 0, Agent 2)

**Method.** Every verdict below was proven by reading the crate source on
disk — no API is claimed from memory or from docs.rs. Crate root:

```
~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/gpui-0.2.2/
  Cargo.toml: name = "gpui", version = "0.2.2"   (matches the Cargo.lock pin)
```

Paths below are relative to that root's `src/` unless they start with
`examples/`. Line numbers are as read on 2026-11-08.

## Headline verdict

The standing claim in `docs/REDESIGN_ARCHITECTURE.md` / `docs/TOUCH.md`-era
notes — "GPUI 0.2.2 has **no** popover/menu/tooltip/dropdown primitive, no
splitter/drag support, and no text-input building blocks" — is **wrong on
all three counts**. The crate ships anchored floating layers, a real
hover-tooltip system, a full drag/drop event family, scrollable containers
with external scroll handles, virtualised lists, focus/tab machinery, a
complete `EntityInputHandler` text-input protocol (IME + clipboard), an
SVG element with an `AssetSource` hook, and a cursor set that includes
column/row resize. What the crate does **not** ship is *finished widgets*:
there is no prebuilt menu, dropdown, text field, or splitter component —
those are assembled from the primitives below (exactly what Zed does).
The one hard platform limit already proven elsewhere stands: **there is
no touch input** (see `docs/TOUCH.md`; not re-audited here).

## Summary table

| # | Primitive | Verdict | Crate path |
|---|---|---|---|
| 1 | `anchored()` floating layer | ✅ exists | `elements/anchored.rs:27` |
| 2 | `deferred()` out-of-flow render | ✅ exists | `elements/deferred.rs:7` |
| 3 | `.tooltip(...)` hover tooltips | ✅ exists | `elements/div.rs:1161` |
| 4 | `.on_drag` / `.on_drag_move` / `.on_drop` | ✅ exists | `elements/div.rs:499 / 282 / 462` |
| 5 | `overflow_x/y_scroll` + `.track_scroll` | ✅ exists | `elements/div.rs:1049–1081` |
| 6 | `uniform_list` / `list` (virtualised) | ✅ exists | `elements/uniform_list.rs:22`, `elements/list.rs:24` |
| 7 | `.occlude()` mouse occlusion | ✅ exists | `elements/div.rs:997` (+ `occlude_mouse` :575) |
| 8 | `.group()` / `.group_hover` / `.group_active` | ✅ exists | `elements/div.rs:601 / 680 / 1098` |
| 9 | `.focusable()` / `.tab_index()` / `FocusHandle::focus` | ✅ exists | `elements/div.rs:1043 / 637`, `window.rs:340` |
| 10 | `EntityInputHandler` + `Window::handle_input` (IME, clipboard) | ✅ exists | `input.rs:10`, `window.rs:3400` |
| 11 | `cx.global` / `Global` trait / `set_global` | ✅ exists | `global.rs:22`, `app.rs:1457 / 1497` |
| 12 | `svg()` + `AssetSource` + `Application::with_assets` | ✅ exists | `elements/svg.rs:18`, `assets.rs:13`, `app.rs:155` |
| 13 | `WindowOptions.window_bounds` (initial size) | ✅ exists | `platform.rs:1093` |
| 14 | `WindowOptions.window_min_size` | ✅ exists | `platform.rs:1127` |
| 15 | Window title via `TitlebarOptions.title` | ✅ exists (⚠️ see caveat) | `platform.rs:1250` |
| 16 | `Animation` / `with_animation` | ✅ exists | `elements/animation.rs:15 / 52` |
| 17 | `CursorStyle` incl. `ResizeColumn` / `ResizeRow` / `IBeam` | ✅ exists | `platform.rs:1404–1470` |
| 18 | `ClickEvent::modifiers()` (+ `position()`) | ✅ exists | `interactive.rs` (`impl ClickEvent`) |
| 19 | `text_ellipsis` / `truncate` / `line_clamp` | ✅ exists | `styled.rs:83 / 123 / 129` |
| 20 | `font_family` | ✅ exists | `styled.rs:616` |
| 21 | `Window::bounds()` (live window size) | ✅ exists | `window.rs:1688` |
| 22 | `on_children_prepainted` (element bounds after layout) | ✅ exists | `elements/div.rs:1243` |
| 23 | `canvas()` + `PathBuilder` (painted glyphs/paths) | ✅ exists | `elements/canvas.rs:10`, `path_builder.rs:25` |
| 24 | Prebuilt menu / dropdown / text-field / splitter **widgets** | ❌ absent (by design — assemble from 1–23) | — |
| 25 | Touch input | ❌ absent (proven in `docs/TOUCH.md`) | — |

All of 1–23 are re-exported through `elements/mod.rs` (`pub use
anchored::*; pub use deferred::*;` etc.) and the `gpui::` root / prelude,
so `use gpui::*;` — which `src/app.rs` already does — reaches them.

---

## Detail per primitive

### 1–2. Floating layers: `anchored()` + `deferred()`

`elements/anchored.rs`:

```rust
pub fn anchored() -> Anchored                       // :27
impl Anchored {
    pub fn anchor(mut self, anchor: Corner) -> Self             // :40
    pub fn position(mut self, anchor: Point<Pixels>) -> Self    // :47
    pub fn position_mode(mut self, mode: AnchoredPositionMode) -> Self // :62
    pub fn snap_to_window(mut self) -> Self                     // :68
    pub fn snap_to_window_with_margin(mut self, edges: impl Into<Edges<Pixels>>) -> Self // :74
}
```

`elements/deferred.rs`:

```rust
pub fn deferred(child: impl IntoElement) -> Deferred            // :7
impl Deferred { pub fn with_priority(mut self, priority: usize) -> Self } // :25
```

**Usage pattern** (the crate's own tooltip system is built on this pair;
this is the canonical popover recipe):

```rust
deferred(
    anchored()
        .position(trigger_bottom_left_point)
        .anchor(Corner::TopLeft)
        .snap_to_window_with_margin(px(8.))
        .child(popover_body),
).with_priority(1)
```

**Caveats.** `anchored()` positions relative to a *point you supply* —
there is no "anchor to this element" handle; the trigger's bounds must be
captured (see §A). `snap_to_window*` flips/clamps the child so it stays
inside the window — edge awareness is built in. Deferred children render
after (above) the normal tree and do not participate in the parent's
layout — a popover built this way **cannot push content down**, which is
exactly the Wave 15 menu-row defect (S3) fixed at the primitive level.

### 3. Tooltips

`elements/div.rs:1161` (fluent) and `:538` (imperative `Interactivity`):

```rust
fn tooltip(mut self, build_tooltip: impl Fn(&mut Window, &mut App) -> AnyView + 'static) -> Self
```

Doc comment (div.rs, above :1161): *"Use the given callback to construct
a new tooltip view when the mouse hovers over this element. The tooltip
itself is also hoverable and won't disappear when the user moves the
mouse into it."* The tooltip view is any `Render` view the app supplies,
so a tooltip can contain text + a kbd chip. **Caveat:** the builder runs
per hover target; keep tooltip views cheap. Delay behaviour is internal
to the window's tooltip state — no delay constant is exposed in the
public signature (see UNPROVEN).

### 4. Drag and drop

`elements/div.rs`:

```rust
pub fn on_drag<T, W>(&mut self, value: T,
    constructor: impl Fn(&T, Point<Pixels>, &mut Window, &mut App) -> Entity<W> + 'static) // :499
pub fn on_drag_move<T>(...)        // :282 — events while a drag of type T is in flight
pub fn on_drop<T: 'static>(&mut self, listener: impl Fn(&T, &mut Window, &mut App) + 'static) // :462
```

Semantics: `on_drag` starts a typed drag carrying a value `T` and builds
a *drag preview view* via the constructor; any element can observe the
drag with `on_drag_move` and receive the drop with `on_drop`.
`examples/drag_drop.rs` is a complete worked example. **This is
payload drag-and-drop** (reorder, drag-into). For *continuous* drags
(splitter handles, slider thumbs, scrubbing) the cheaper pattern the app
already uses — `on_mouse_down` on the handle + `on_mouse_move` /
`on_mouse_up` routed on a parent — remains available and is what the
existing canvas input does today; `on_drag_move` also works if the drag
is started with `on_drag`. **Caveat:** one `on_drag` per element
(`debug_assert` in source: "calling on_drag more than once on the same
element is not supported").

### 5. Scrolling

`elements/div.rs:1049–1081` (`Styled`-style fluent methods on `Div`):

```rust
fn overflow_scroll(mut self) -> Self        // :1049
fn overflow_x_scroll(mut self) -> Self      // :1056
fn overflow_y_scroll(mut self) -> Self      // :1062
fn track_scroll(mut self, scroll_handle: &ScrollHandle) -> Self // :1077
```

`ScrollHandle` is an external, cloneable handle: app code can read the
offset and scroll programmatically, and several containers can share one
handle. `examples/scrollable.rs` demonstrates it. **Caveat:** scrollbar
*styling* is the app's own div overlaid using the handle's offset —
GPUI draws no styled native scrollbar widget for you (⚠️ partial as a
"scrollbar component", ✅ as a mechanism).

### 6. Virtualised lists

```rust
pub fn uniform_list<R>(...) -> UniformList   // elements/uniform_list.rs:22
pub fn list(...) -> List                     // elements/list.rs:24
```

`uniform_list` renders only visible rows of uniform height (processor:
`Fn(Range<usize>, &mut Window, &mut App) -> Vec<E>`); `list` supports
variable heights with a `ListState`. `examples/uniform_list.rs` is a
worked example. Directly applicable to the Home project list and long
clip/figure lists.

### 7. Occlusion

```rust
fn occlude(mut self) -> Self                       // div.rs:997
pub fn occlude_mouse(&mut self)                    // div.rs:575 (Interactivity)
```

An occluding element blocks mouse events from reaching elements painted
beneath it — the mechanism a modal dialog / popover backdrop uses to eat
outside clicks. Combined with §1–2 this gives real outside-click dismiss.

### 8. Groups

```rust
fn group(mut self, group: impl Into<SharedString>) -> Self   // div.rs:601
fn group_hover(...)                                          // div.rs:680
fn group_active(...)                                         // div.rs:1098
```

Parent marks `.group("name")`; children style themselves with
`group_hover("name", |s| ...)` / `group_active(...)`. This is how
row-hover reveals (eye/lock/solo on figure rows) are done without state.

### 9. Focus and tab order

```rust
fn focusable(mut self) -> Self        // div.rs:1043
fn tab_index(mut self, index: isize) -> Self   // div.rs:637
impl FocusHandle { pub fn focus(&self, window: &mut Window) } // window.rs:340
pub fn focus(&mut self, handle: &FocusHandle)  // Window, window.rs:1386
```

Plus the existing key-context/keymap machinery the app already uses.
`examples/tab_stop.rs` demonstrates tab order. A command palette / text
field holds a `FocusHandle` and calls `window.focus(&handle)`.

### 10. Text input — `EntityInputHandler`

`src/input.rs:10`:

```rust
pub trait EntityInputHandler: 'static + Sized {
    fn text_for_range(&mut self, range: Range<usize>, adjusted_range: &mut Option<Range<usize>>, window: &mut Window, cx: &mut Context<Self>) -> Option<String>;
    fn selected_text_range(&mut self, ignore_disabled_input: bool, window: &mut Window, cx: &mut Context<Self>) -> Option<UTF16Selection>;
    fn marked_text_range(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<Range<usize>>;
    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>);
    fn replace_text_in_range(&mut self, range: Option<Range<usize>>, text: &str, window: &mut Window, cx: &mut Context<Self>);
    fn replace_and_mark_text_in_range(&mut self, range: Option<Range<usize>>, new_text: &str, new_selected_range: Option<Range<usize>>, window: &mut Window, cx: &mut Context<Self>);
    fn bounds_for_range(&mut self, range_utf16: Range<usize>, element_bounds: Bounds<Pixels>, window: &mut Window, cx: &mut Context<Self>) -> Option<Bounds<Pixels>>;
    fn character_index_for_point(&mut self, point: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) -> Option<usize>;
    // …
}
```

Wiring, from `examples/input.rs` (a complete single-line text field,
~570 lines, `struct TextInput` at :33, handler impl at :261):

```rust
// in the field view's Render:
window.handle_input(&focus_handle, ElementInputHandler::new(bounds, entity), cx)  // examples/input.rs:540
// Window::handle_input — window.rs:3400:
//   pub fn handle_input(&mut self, focus_handle: &FocusHandle,
//       input_handler: impl InputHandler, cx: &mut App)
```

Clipboard is app-level: `App::write_to_clipboard(&self, item:
ClipboardItem)` (`app.rs:1041`) and `read_from_clipboard(&self) ->
Option<ClipboardItem>` (`app.rs:1053`). IME composition is the
`marked_text_range` / `replace_and_mark_text_in_range` pair — i.e. IME
is *protocol-supported*, not automatic.

**Context for this repo:** our own `src/text_edit.rs` (368 lines) does
**not** use any of this — it is a UI-agnostic state machine fed by
printable characters from the app's key handler (no IME, no clipboard,
no selection rendering protocol). A Studio text field should implement
`EntityInputHandler` on a small stateful view, reusing the example's
shape; `text_edit.rs` stays as the commit/cancel session logic behind it.

### 11. Globals

```rust
pub trait Global: 'static { … }                                  // global.rs:22
impl App { pub fn global<G: Global>(&self) -> &G }               // app.rs:1457
impl App { pub fn try_global<G: Global>(&self) -> Option<&G> }   // app.rs:1466
impl App { pub fn set_global<G: Global>(&mut self, global: G) }  // app.rs:1497
```

This is the supported backing for the redesign's `kit::pal()` plan: a
`Global` (or an atomic mirroring one) read from any render. Note
`global()` panics if unset — use `try_global` or set it at startup.

### 12. SVG icons + `AssetSource`

```rust
pub fn svg() -> Svg                                        // elements/svg.rs:18
impl Svg { pub fn path(mut self, path: impl Into<SharedString>) -> Self } // svg.rs:28
pub trait AssetSource: 'static + Send + Sync {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>>;
    fn list(&self, path: &str) -> Result<Vec<SharedString>>;   // assets.rs:13
}
impl Application { pub fn with_assets(self, asset_source: impl AssetSource) -> Self } // app.rs:155
```

`examples/svg/svg.rs` is the worked example: a hand-written `Assets`
struct whose `load` serves bytes from a static table (`:10–15`), wired
with `Application::new().with_assets(Assets { … })` (`:72`). SVG
rendering itself is `svg_renderer.rs` (path fills/strokes rasterised by
the crate; tint via the element's text colour in Zed's usage pattern).
No `rust-embed` dependency is required — a `match` on path strings
returning `include_bytes!(...)` satisfies the trait.

### 13–15. `WindowOptions` — the integrator's main.rs facts

The struct lives in `platform.rs` (not `window.rs`). Quoted fields:

```rust
pub struct WindowOptions {
    pub window_bounds: Option<WindowBounds>,        // :1093
    pub titlebar: Option<TitlebarOptions>,          // :1096
    pub focus: bool,                                // :1099
    pub show: bool,                                 // :1102
    pub kind: WindowKind,                           // :1105
    pub is_movable: bool,                           // :1108
    pub is_resizable: bool,                         // :1111
    pub is_minimizable: bool,                       // :1114
    pub display_id: Option<DisplayId>,              // :1118
    pub window_background: WindowBackgroundAppearance, // :1121
    pub app_id: Option<String>,                     // :1124
    pub window_min_size: Option<Size<Pixels>>,      // :1127
    pub window_decorations: Option<WindowDecorations>, // :1131
    pub tabbing_identifier: Option<String>,         // :1134
}

pub enum WindowBounds {                              // platform.rs:1188
    Windowed(Bounds<Pixels>),
    Maximized(Bounds<Pixels>),
    Fullscreen(Bounds<Pixels>),
}

pub struct TitlebarOptions {
    pub title: Option<SharedString>,                 // platform.rs:1250
    pub appears_transparent: bool,                   // :1254
    pub traffic_light_position: Option<Point<Pixels>>, // :1257
}
```

So for Agent 1's `main.rs` step: **initial size** =
`window_bounds: Some(WindowBounds::Windowed(Bounds::centered(None,
size(px(1440.), px(900.)), cx)))` (the `Bounds::centered` helper is
used in the crate's own `examples/window.rs`); **minimum size** =
`window_min_size: Some(size(px(1280.), px(720.)))`; **title** =
`titlebar: Some(TitlebarOptions { title: Some("StykAnimate RS".into()),
..Default::default() })`. ⚠️ Caveat: on Linux the titlebar title is
honoured by the platform window (X11/Wayland); with a *custom* titlebar
(`appears_transparent`) the app draws its own title text and the field
is moot. Whether the packaged binary's window actually shows the title
on the user's desktop is a screenshot/CI-artifact question, not a source
question.

### 16. Animation

```rust
pub struct Animation { … }                            // elements/animation.rs:15
impl Animation { pub fn new(duration: Duration) -> Self }  // :28
pub trait AnimationExt {
    fn with_animation(…)                              // :54
    fn with_animations(…)                             // :72
}
```

`examples/animation.rs` demonstrates easing-driven element animation.
Enough for 80–140 ms popover/hover transitions; state-swap remains the
fallback where an animated value has no element to hang off.

### 17. Cursors

`platform.rs:1404+`, `pub enum CursorStyle`:

```
Arrow, IBeam, Crosshair, ClosedHand, OpenHand, PointingHand,
ResizeLeft, ResizeRight, ResizeLeftRight, ResizeUp, ResizeDown,
ResizeUpDown, ResizeUpLeftDownRight, ResizeUpRightDownLeft,
ResizeColumn (:1466), ResizeRow (:1470), …
```

Applied via the `Styled` macro methods (`gpui_macros::cursor_style_methods!()`,
`styled.rs:31`) — e.g. `.cursor(CursorStyle::IBeam)`
(`examples/input.rs:571`), `.cursor_pointer()` (already used in our
`src/app.rs` menu code). Splitter handles get `ResizeColumn` /
`ResizeRow`; text fields get `IBeam`. The style field underneath is
`Style::mouse_cursor: Option<CursorStyle>` (`style.rs:258`).

### 18. Click modifiers

`ClickEvent` (in `interactive.rs`) exposes `modifiers()` and
`position()` accessors (`impl ClickEvent`, `pub fn modifiers(&self) ->
Modifiers`); `Modifiers` carries `control / alt / shift / platform /
function` bools. Our app already depends on `ev.modifiers().shift` for
shift-click frame ranges — consistent with this source.

### 19–20. Text styling

`styled.rs`: `fn text_ellipsis(mut self) -> Self` (:83),
`fn truncate(mut self) -> Self` (:123), `fn line_clamp(mut self, lines:
usize) -> Self` (:129), `fn font_family(mut self, family_name: impl
Into<SharedString>) -> Self` (:616). Enough for truncated panel labels
and a mono font family on numeric fields (the family must exist on the
user's system — GPUI bundles no fonts; see UNPROVEN).

### 21–22. Learning sizes at runtime

```rust
impl Window { pub fn bounds(&self) -> Bounds<Pixels> }   // window.rs:1688
pub fn on_children_prepainted(                          // div.rs:1243
    &mut self, listener: impl Fn(&[Bounds<Pixels>], &mut Window, &mut App) + 'static)
```

`window.bounds()` gives the live window size *during render* — layout
decisions (how many toolbar sections fit) can be computed from it every
frame with no subscription needed, since a resize repaints.
`on_children_prepainted` reports each child's laid-out `Bounds` after
prepaint — the way to learn a trigger's exact position for §1 anchoring
(store it in view state, use it next frame / on open).

### 23. Painted paths (icon alternative)

```rust
pub fn canvas<T>(…) -> Canvas<T>                 // elements/canvas.rs:10
pub struct PathBuilder { … }                     // path_builder.rs:25
impl PathBuilder { pub fn build(self) -> Result<Path<Pixels>, Error> } // :244
```

This is the mechanism our stage already paints with. A glyph set can be
drawn as filled paths on a `canvas()` with no asset plumbing at all.

### 24–25. What is genuinely absent

- **No prebuilt widgets.** There is no `menu`, `select`, `text_field`,
  `splitter`, or `dialog` component type anywhere in the crate (the
  `elements/` folder is the complete list: anchored, animation, canvas,
  deferred, div, image_cache, img, list, surface, svg, text,
  uniform_list). Widgets are assembled per-app from §§1–23 — planned
  work for Agents 5–7, not a blocker.
- **No touch input** — proven previously from this same source
  (`docs/TOUCH.md`): `PlatformInput` has no touch variant. Unchanged.

---

## Design question A — toolbar overflow: how does app code learn its width?

**Evidence.** Three mechanisms exist, in increasing precision:

1. **`window.bounds()` during render** (`window.rs:1688`) — the toolbar's
   available width is the window width minus the known fixed insets
   (panels have token widths). Zero plumbing; recomputed every render;
   resize triggers a repaint, so the computation is always current.
2. **`on_children_prepainted`** (`div.rs:1243`) — after a frame is laid
   out, the toolbar container learns each section's real `Bounds`;
   overflow decisions can then be *exact* (render N sections, measure,
   move the rest into `More ▾` next frame). One frame of lag on resize.
3. **Per-breakpoint section lists** — pure function of (1): choose a
   section set from width bands (≥1600 / 1366 / 1280). Deterministic,
   unit-testable, no feedback loop.

**Recommendation.** Ship (3) on top of (1) for Wave 2 — section lists
keyed off `window.bounds().size.width` bands, computed in the toolbar
builder as a pure function (unit-testable without a window). Add (2)
only if band edges prove visibly wrong in the Wave 2 screenshot
checkpoint. Do **not** attempt CSS-style continuous measurement; GPUI
has no layout-constraint feedback within a single frame, and (2)'s
one-frame lag is the honest price.

## Design question B — icons: `svg()` + `AssetSource` vs painted paths

**Evidence.** The app today has **no** `AssetSource`:
`src/main.rs` is 11 lines — `Application::new().run(|cx| cx.open_window(
WindowOptions::default(), …))`. The SVG route needs exactly:

- one `AssetSource` impl in `main.rs` (a `match` over path strings →
  `include_bytes!("../assets/icons/…")`, following `examples/svg/svg.rs:10–15`),
- `.with_assets(…)` on the `Application` builder (`app.rs:155`),
- `.svg` files under `assets/icons/`, rendered with `svg().path("…")`.

The painted-path route needs: a `PathBuilder` per glyph drawn inside
`canvas()` — no main.rs change, but every glyph is hand-plotted Rust
code, stroke/fill behaviour is ours to keep consistent, and there is no
designer-editable file.

**Recommendation.** **Adopt `svg()` + `AssetSource`.** The wiring cost is
one small struct in `main.rs` (Agent 1 owns it, Part 6) plus files on
disk; the per-glyph cost is writing standard SVG path data (the same
data every icon set ships), tintable via text colour, resizable via the
element's size. Painted paths remain the right tool for *stage* art
(handles, paths, previews) where geometry is computed, not authored.
Risk to note: SVG feature coverage in `svg_renderer.rs` is the crate's
own subset (fills, strokes, transforms — not filters/masks); keep icons
to plain paths and fills, which is the professional icon style anyway.

## Recommended implementation (summary for Agents 5–8)

| Studio need | Build it from | Owner |
|---|---|---|
| Popover / dropdown menu | `deferred(anchored().position(p).snap_to_window_with_margin(..))` + `occlude()` backdrop + stored trigger bounds from `on_children_prepainted` | Agent 6 |
| Tooltip | `.tooltip(|w, cx| view)` on the trigger | Agent 6 |
| Text field / palette input | Stateful view + `EntityInputHandler` + `window.handle_input` (shape of `examples/input.rs`) | Agent 7 |
| Numeric scrub field | Mouse down/move/up routing (existing pattern) + `CursorStyle::ResizeLeftRight` hover + click-to-type via the text field | Agent 7 |
| Slider / splitter | Mouse routing or `on_drag`/`on_drag_move`; `ResizeColumn`/`ResizeRow` cursors; persist via `LayoutPrefs` | Agent 7/8 |
| Scroll regions | `overflow_y_scroll()` + `track_scroll(&handle)`; custom thin scrollbar div from the handle | Agent 7 |
| Long lists | `uniform_list` | Agents 13/16 |
| Row hover reveals | `.group(name)` + `group_hover` | Agent 12 |
| Theme global (`pal()`) | `Global` impl set at startup, or the atomic mirror in the master prompt — both are source-supported | Agent 4 |
| Icons | `svg()` + `AssetSource` (see §B) | Agent 5 |
| Window setup | `WindowOptions { window_bounds, window_min_size, titlebar.title }` (§13–15) | Agent 1 |
| Toolbar overflow | Width bands from `window.bounds()` (§A) | Agent 9 |
| Micro-animation | `with_animation` (durations from kit tokens) | Agent 4/5 |

## UNPROVEN — not established from source, do not design against these

1. **Tooltip delay duration** — the hover delay is internal window
   state; no public constant/signature was found in this pass. The
   master prompt's "400 ms delay" figure is **unproven**.
2. **Whether the packaged binary shows `TitlebarOptions.title`** on the
   user's Linux desktop (X11 vs Wayland, server-side vs client-side
   decorations) — only running the CI artifact can decide.
3. **Performance of `deferred`/`anchored` overlays at stage repaint
   rates** — source shows the mechanism, not its frame cost in this app.
4. **A bundled monospace font** — `font_family` accepts any name, but
   GPUI bundles no fonts; which mono families exist on the user's
   machine is a runtime question. (Fallback stacks are a design choice,
   not a crate guarantee.)
5. **`svg_renderer.rs` subset coverage** beyond plain paths/fills —
   gradients, masks, filters inside icon SVGs were not audited file by
   file; keep icons to paths + fills (per §B).
6. **`list` (variable-height) state persistence across frames** — the
   `ListState` API was located but not read end to end; `uniform_list`
   (read fully) is the safe default.
