# Touch input: what GPUI 0.2.2 can and cannot do

**Verdict: GPUI 0.2.2 exposes no touch input API — on Linux or
any other platform.** There is no touch event type, no touch
variant in the input enum, and no `on_touch` handler registration
anywhere in the crate. Real multi-touch (per-contact pointer ids,
two-finger pinch from hardware) is therefore **not wireable**
against the pinned GPUI version, and this app does not pretend
otherwise. What exists instead is a complete, tested gesture
pipeline fed by the one pointer source GPUI can produce — the
mouse — with a single adapter seam where a future touch feed
would plug in.

Researched in the vendored crate source
(`~/.cargo/registry/src/index.crates.io-*/gpui-0.2.2/`, the exact
version `Cargo.toml` pins with `gpui = "=0.2.2"`). The same
research is recorded on `touch::PointerSource` in `src/touch.rs`.

## Evidence

- **No touch variant in the input enum.** `src/interactive.rs`
  (`PlatformInput`, line 547) has exactly nine variants:
  `KeyDown`, `KeyUp`, `ModifiersChanged`, `MouseDown`, `MouseUp`,
  `MouseMove`, `MouseExited`, `ScrollWheel`, `FileDrop`.
- **No `TouchEvent`** exists anywhere in the crate, and there is
  **no `on_touch`** (or equivalent) in the element handler set —
  `src/elements/div.rs` registers mouse down/up/move, scroll
  wheel and click handlers only.
- **Mouse events carry no pointer identity.** `MouseDownEvent`,
  `MouseUpEvent` and `MouseMoveEvent` have `button`, `position`,
  `modifiers`, `click_count` / `pressed_button` — nothing that
  could distinguish two simultaneous contacts, even in principle.
- **`TouchPhase` is not touch input.** The one touch-sounding
  type (`src/interactive.rs` line 81: `Started` / `Moved` /
  `Ended`) is used only as `ScrollWheelEvent::touch_phase`
  (line 374) — trackpad scroll phasing. The Wayland client stamps
  `TouchPhase::Moved` onto the scroll events it synthesises
  (`src/platform/linux/wayland/client.rs` lines 1794, 1806) and
  the X11 client uses `TouchPhase::default()` (line 2221).
- **Wayland never asks for touch.** The seat handling binds only
  the `Keyboard` and `Pointer` capabilities
  (`wayland/client.rs` lines 1154, 1169); nothing in the crate
  calls `get_touch`, so `wl_touch` events are never requested
  from the compositor.
- **X11 caps XInput below the touch version, on purpose.**
  `src/platform/linux/x11/client.rs` lines 335–336: *"Announce to
  X server that XInput up to 2.1 is supported. To increase this
  to 2.2 and beyond, support for touch events would need to be
  added."* Touch is an XInput 2.2 feature.
- **No pinch/magnify gesture events** exist in the public API
  either.

Consequence: on Linux, a touchscreen reaches a GPUI 0.2.2 app
only through compositor / X **pointer emulation** — a single
synthetic mouse. Taps and drags work exactly like mouse input;
a second finger produces nothing the app can observe as such.

## What the app does today

- Canvas mouse handlers (`on_mouse_down` / `on_mouse_move` /
  `on_mouse_up` on the stage in `src/app.rs`) convert window
  coordinates to stage coordinates and call
  `EditorView::feed_pointer` under `touch::MOUSE_POINTER_ID` (0).
- `feed_pointer` routes every event through **`touch::feed`**,
  the one adapter door, with `touch::ACTIVE_POINTER_SOURCE`
  (`PointerSource::Mouse`). The adapter normalises mouse events
  to id 0 — a mouse has no identity of its own — and passes
  `PointerEvent`s into the `touch::Recognizer`.
- The recogniser is fully multi-pointer and deterministic:
  drag with slop and full-delta catch-up, tap, long-press (via
  `on_tick`, driven from the 33 ms playback loop and at the head
  of each fed event), and two-pointer pinch with cumulative
  scale/pan. Gestures map to editor actions in `apply_gesture`:
  drag = pose dragging (hands/feet solve IK, hips move the
  figure), tap = select, long-press = select + a status note.
- **Pinch is implemented end to end except for the feed.**
  `PinchCamera` (app.rs) consumes `PinchStart` / `Pinch` /
  `PinchEnd`, captures the camera at pinch start and writes
  `baseline ∘ gesture` back as camera keys — zoom and pan from
  the recogniser's cumulative scale and centre travel. The
  recogniser side is pinned by unit tests in `src/touch.rs`
  (including the exact camera-math contract test). But with no
  multi-pointer event source in GPUI 0.2.2, no real two-finger
  gesture can reach it on this build. It is ready, tested at the
  recogniser level, and **idle on current hardware** — not a
  shipped touch feature.
- One timing limit, stated plainly: an idle, perfectly still
  pointer's long-press surfaces on the next event or playback
  tick rather than at exactly 500 ms, because `on_tick` is
  driven from those loops.

## What a future GPUI upgrade unlocks

If a later GPUI adds any of: a touch variant in `PlatformInput`,
an `on_touch`-style element handler, XInput 2.2 selection, or a
`wl_touch` binding — then multi-touch becomes a **wiring change
in this app, not a redesign**:

1. Translate each platform contact into a `touch::PointerEvent`
   with the contact's own id (constant from down to up).
2. Feed them through the same door:
   `touch::feed(&mut recognizer, PointerSource::Touch, ev)` —
   the adapter already passes touch ids through untouched
   (pinned by the `feed_from_touch_passes_contact_ids_through`
   test).
3. Point `ACTIVE_POINTER_SOURCE` at the real source; the test
   `pointer_source_today_is_the_gpui_mouse` in `src/touch.rs`
   pins today's reality and marks exactly where the change
   shows up.

The recogniser, `PinchCamera`, and the editor's gesture
application need no changes: they were built multi-pointer from
the start, and their behaviour is covered by tests that do not
depend on where events come from.
