# P15 — Motion & Presence Spec (Phase N, planning)

Researched 2026-11-08 against the pinned Rune source in
`~/workspace/rune-src` (rev 87caf2a, the rev the shell's
Cargo.toml pins). Every claim below cites the Rune file
it was read from. Shell claims cite `src/` in this repo.

**Headline finding, stated plainly:** the charter's
honest-limits note is right that Rune is not a
per-control motion *toolkit* — but it understates what
exists. Rune has a complete spring/transition system
(`El::transition`, `El::motion`, `Tx::motion`, preset
spring pairs in `tok::motion`). The shell uses **none**
of it today: a grep of `src/` finds zero `.transition(`,
`.motion(`, `.enter_delay(`, `.opacity_motion(` or
`.animate_layout(` calls. Everything the shell currently
"animates" is Rune's default behaviour. Phase N's safe
motion work is therefore small and surgical: three
additions (press scale on buttons, an enter transition
on menus/popovers, a slide on toasts), all zero-bounce,
all preserving every key and handler.

---

## 1. What the shell already gets for free (no code)

### 1.1 Default presence fade on every mount/unmount

Every element has a default transition,
`Transition::opacity()` — a **0.2 s enter fade and a
0.15 s exit fade** — applied when it mounts or is
removed *in an animated commit* (`El::on_press`,
`AppSpec::on_key`, `on_submit` commits animate by
default). Source: `crates/rune/src/el/motion.rs`,
`El::transition` doc (lines ~168–181).

This is already the shell's menu/toast/palette motion:

- Menus, the scrim and toasts are conditional **last
  root children** of the view (`src/main.rs:770–772`),
  so opening/closing a menu fades the dropdown in/out
  today, with no shell code.
- The command palette is a conditional child of the
  centre column (`src/main.rs:676–677`) — same default
  fade on open/close.
- Toasts are keyed rows (`toast-{index}`,
  `src/rune_sec/toasts.rs:370`) inside a pinned column;
  a toast mounting or expiring fades by the same
  default. (This is why Runtime tests since Phase F
  have needed bounded settle loops before asserting a
  removed element is *gone*.)
- Workspace switching (**Alt+1..8**) mounts/unmounts
  whole gated panels — they crossfade by the same
  default presence, for free.

### 1.2 Instant commits (the other default)

Commits from typing in a focused text field, focusing
an input on pointer-down, and drag moves are
**instant automatically**: elements removed in such a
commit unmount the same frame, no fade (same
`el/motion.rs` doc; `crates/rune/src/el/tx.rs`,
`Tx::instant` doc ~lines 410–425). Consequences the
shell already lives with, whether it knows it or not:

- Palette rows filtered while typing in the query
  field appear/disappear instantly — they cannot be
  made to fade without changing the commit type (see
  §3, cannot-do #2).
- The shell's two explicit `tx.instant()` calls are in
  drag-scrub paths and are correct as-is:
  `src/rune_sec/scene_panel.rs:389,421` (weather drags)
  and `src/rune_sec/layers_panel.rs:345` (opacity drag).

### 1.3 Layout and colour changes spring by default

When an animated commit changes layout or colours, the
change springs. The fallback spring when nothing is
set is `motion::DEFAULT` = `SETTLE`
(`crates/rune/src/runtime/frame.rs:213`;
`crates/rune-motion/src/tokens.rs:18,41` —
`Spring::new(0.3, 0.0)`). Colour changes (including a
hover highlight) ride the element's spring rather than
snapping (`crates/rune/src/el/motion.rs`,
`El::instant_changes` doc; `Tx::color_motion`,
`el/tx.rs` ~line 465). So the kit's hover/press fills
(charter items 4–5) already ease instead of cutting,
with nothing to wire.

### 1.4 Scroll: momentum and reveals are built in

- Scroll viewports (`El::scroll`, `El::scroll_x`,
  `El::scroll_xy` — `crates/rune/src/el/scroll.rs:15,45,61`)
  carry Rune's own wheel/trackpad handling and
  **momentum after a fling** (scroll_x doc, same file).
  The shell's column viewports (`src/main.rs:745,754`)
  and its horizontal strips (workspace chips
  `src/chrome_view.rs:206`, timeline
  `src/rune_sec/timeline_marks.rs:892`, brush strip,
  waveform) already get this.
- Programmatic scrolling exists as Tx requests
  (`Tx::scroll_to`, `scroll_to_bottom`,
  `scroll_into_view` — `crates/rune/src/el/tx.rs:49–54`
  and the scroll builders' docs). The shell does not
  call any of them today; Phase N does not need them
  (the Runtime tests' `Runtime::wheel` is test-side,
  not app behaviour).
- The stage's wheel-zoom is shell code, not a scroll
  viewport (`State::wheel_zoom` coalescing in
  `src/main.rs`) — Rune motion does not apply to it and
  Phase N must not try to spring it: camera zoom is
  direct manipulation.

### 1.5 Reduce Motion is respected automatically

Entrances degrade to a short fade, `press_scale` turns
off, matched flights fall back to plain transitions —
all decided by the runtime from the host's setting
(cited in the `El::transition`, `El::enter_delay` and
`El::press_scale` docs). The shell sets nothing on
`AppSpec` for motion (`src/main.rs:781` — title, size,
init, view only) and needs to set nothing.

---

## 2. Can do — the controllable surface (source-cited)

| # | Capability | Exact API | Source |
|---|---|---|---|
| C1 | Per-element enter/exit effect | `El::transition(Transition)` — constructors: `opacity()`, `offset(x, y)`, `scale(f)`, `scale_anchor`, `move_edge`, `slide()`, `reveal(edge)`, `blur(f)`, combined via `.combined(..)`, asymmetric via `Transition::asymmetric(enter, exit)` | `crates/rune-motion/src/transition.rs:186–275` |
| C2 | Tune a transition's speed/feel | `.springs(SpringPair)` on a Transition; pairs in `tok::motion` (re-exported from rune-motion tokens): INSTANT, SNAP (0.25/0.20), SETTLE (0.30/0.24), GLIDE (0.32/0.26), REFLOW (0.40/0.32) — all zero-bounce | `transition.rs:261`; `crates/rune-motion/src/tokens.rs:12–34,66–73`; `crates/rune/src/tok.rs:5–9` (`tok::motion`, plus the shell-only `LEAD` pair member) |
| C3 | Spring for a subtree's layout + colour changes | `El::motion(Spring)` (inherited); `El::token_motion(Spring)` for token-chosen springs | `crates/rune/src/el/motion.rs:89–100` |
| C4 | Per-commit spring override | `tx.motion(Spring)` / `tx.motion_curve(anim)` / `tx.color_motion(Spring)` inside a handler; `tx.animated()` to force animation where the runtime would go instant (not for drags — see §4) | `crates/rune/src/el/tx.rs:434–470` |
| C5 | Press feedback that springs back | `El::press_scale(0.96–0.97)` on the element carrying `on_press`; layout/hit-test keep the resting size; Reduce Motion disables it | `crates/rune/src/el/visuals.rs:296–306` |
| C6 | Hover/press colour overlays | `El::hover(Color)`, `El::press(Color)` (already the kit buttons' mechanism; their changes ride the element's spring per §1.3) | `crates/rune/src/el/visuals.rs:287–295` |
| C7 | Staggered entrances | `El::enter_delay(seconds)` + `Stagger::new(step)` (`stagger.delay(i, len)`) | `crates/rune/src/el/enter_delay.rs:36` and its doc example |
| C8 | Kill motion deliberately | `Spring::NONE` via `.motion(..)` (authored-instant mark) or `Transition::none()`; `tx.instant()` / `tx.settle()` per commit | `crates/rune-motion/src/spring.rs:141`; `el/motion.rs:89`; `el/tx.rs` |
| C9 | Shared-element flight between two mounts | `El::matched(id)` — the spring is `tok::motion::REFLOW` by default | `crates/rune/src/el/motion.rs` (`El::matched` doc) |
| C10 | Scroll edge fading | `El::scroll_edge_fade(width)` / `El::scroll_fade(ScrollFade)` on a viewport | `crates/rune/src/el/scroll.rs:83–98` |

Spring values are `Spring::new(duration_seconds, bounce)`
(`crates/rune-motion/src/spring.rs:30`). Bounce > 0
springs (POP 0.25/0.15, MORPH 0.3/0.16 —
`tokens.rs:27–31`) read as *authored overshoot*:
Rune's motion checks (`rune::motion_gate`, referenced
in `el/motion.rs`) treat them as deliberate only when
the element carries a `MotionIntent` mark
(`El::motion_intent`, `el/motion.rs:74`). **Phase N
rule: use zero-bounce pairs only (SNAP / SETTLE /
GLIDE); no `motion_intent` marks anywhere.** That
keeps every Phase N motion inside what the checker
accepts unmarked.

---

## 3. Cannot do — the honest list

1. **No theme-level motion token.** The `Theme` the
   shell styles from carries colour/geometry fields
   only; there is no spring or duration field on it,
   and `tok::motion` is a separate const module
   (`crates/rune/src/tok.rs:5–9`). Retuning the
   *default* presence fade (0.2 s / 0.15 s) globally is
   **not possible** — the only lever is a per-element
   `.transition(..)` override (C1/C2). Any Phase N plan
   that says "set the app-wide fade to X" is
   unimplementable; strike it.
2. **No presence animation inside instant commits.**
   Elements mounted/removed by typing, focus or drag
   commits unmount/mount the same frame (§1.2). Making
   palette-result rows fade while the user types would
   require the text-edit commit to animate, which the
   shell does not control (the instant behaviour is
   automatic in the runtime, not a shell setting).
   Typing-driven lists are instant, permanently.
3. **No scroll-momentum tuning.** `ScrollSpec` has
   exactly one field, `stick_bottom`
   (`crates/rune/src/el.rs:176–183`). Inertia,
   wheel-step size and glide curves are Rune-internal.
   The shell can add edge fading (C10) and nothing
   else about scroll feel.
4. **No authored looping/idle animation for controls.**
   There is no El builder for keyframed or looping
   decorative motion (no spinner/pulse builder in
   `el/`). Rune has an internal live-value system, but
   it is a data-binding mechanism, not a control-motion
   toolkit, and the shell binds nothing through it.
   Phase N adds no animated spinners, no pulsing
   indicators, no breathing highlights — progress and
   activity stay static or text-based, as today.
5. **No motion inside rune-kit text fields.** Caret
   blink, selection and IME behaviour are kit-internal
   (charter's honest limit, confirmed: the shell only
   ever styles field *frames*).
6. **Canvas content does not spring.** The stage and
   preview canvases are `Shape`s the shell rebuilds
   per frame; Rune's springs apply to elements, not to
   shape geometry. Camera moves, onion fades and the
   3D orbit stay exactly as direct as they are today.
   (Element-level: the stage *element* could carry a
   transition, but it never unmounts, so there is
   nothing to animate.)
7. **`matched` flights (C9) are not a Phase N tool.**
   They exist, but adopting one means restructuring a
   surface around a shared id — that is behaviour
   work, not styling, and no Phase N surface needs it.
   Recorded here so nobody "discovers" it mid-wave and
   improvises.

---

## 4. Safe improvements — the whole list (3 items)

Each item: one builder call at one site, zero-bounce,
no key changes, no handler changes, no State changes.
Everything not listed here was considered and rejected
(§5).

### M1 — Press scale on the kit button family

**Where:** `chrome_view::btn` and
`icon_btn::icon_btn` / `icon_only_btn` (the live
builders only — **not** the inert twins, which have no
press to feed back).
**Call:** `.press_scale(0.97)` on the root element
that carries `on_press`.
**Why safe:** one site per family restyles every
button in the app; layout and hit testing keep the
resting size, so no snap-test geometry moves; it
springs back on its own track; Reduce Motion turns it
off. This is the single highest-value tactile upgrade
available in Rune and it is one line per builder.
**Verify:** existing Runtime press tests unchanged
(they read state, not pixels); headless snap eyeball.

### M2 — Menus, popovers and the palette: fade + 4 pt rise

**Where:** the dropdown root in
`rune_sec::menus::menu_dropdown`, the settings popover
root (`rune_sec::settings_popover`), and the palette
card root (`rune_sec::command_palette::palette_view`).
**Call:**
```rust
.transition(
    Transition::offset(0.0, -4.0)
        .combined(Transition::opacity())
        .springs(tok::motion::SNAP),
)
```
(Start 4 pt above rest, spring to place on SNAP —
0.25 s in / 0.20 s out — while fading.) Import:
`rune::{Transition}` + `rune::tok` (already imported
in most of these files).
**Why safe:** these elements already mount/unmount in
animated (press/key) commits, so the transition
actually plays; keys and handlers untouched; the
scrim keeps the plain default fade (it is a dimmer,
not a surface). Popovers that open from a *typing*
commit would not animate — none of these three do.
**Verify:** the Phase F+ Runtime menu/palette tests
already settle-loop before absence assertions; the
same loops absorb the slightly longer SNAP exit.

### M3 — Toasts slide in from the right edge

**Where:** `toast_row` in `src/rune_sec/toasts.rs`
(the keyed `toast-{index}` row root).
**Call:**
```rust
.transition(
    Transition::offset(24.0, 0.0)
        .combined(Transition::opacity())
        .springs(tok::motion::SETTLE),
)
```
**Why safe:** toasts mount in poll commits, which the
shell's own history shows animate presence (their
exits already fade — §1.1); the row key, the Undo
action child and the dismiss handler are untouched.
Exit reuses the same transition in reverse (slide
back + fade), which matches the current fade-out
semantics, just directed.
**Verify:** toasts Runtime tests use settle loops for
absence; presence assertions read keys, not positions.

### Explicitly not in the set

- **No `.motion(..)` on panel roots or columns.**
  A layout spring on a column makes every panel
  add/remove inside it reflow-spring — including
  panels appearing on workspace switch next to the
  stage. The default (SETTLE fallback) already eases
  genuine layout changes; adding inheritance risks
  motion in the stage's neighbourhood for zero user
  benefit.
- **No `enter_delay` staggers.** A stagger on workspace
  switch or panel mount would replay on every Alt+1..8
  press and read as lag within a day. C7 is real and
  Phase N should not use it.
- **No transitions on list rows that reorder during
  typing** (library/figure filters): those commits are
  instant (§3 #2), so the call would be dead code —
  worse than no call, because it promises motion the
  runtime will never play.
- **No `tx.animated()` in drag handlers.** Direct
  manipulation (stage posing, sliders, orbit, scrub)
  must stay 1:1 with the pointer. Rune's own guidance
  reserves `animated()` for rows a dragged row passes —
  the shell has no such surface in Phase N scope.

---

## 5. What Phase N coding agents must know (one paragraph)

Motion in this shell is **defaults plus three calls**.
If a brief asks for any other animation — a curve, a
duration, a stagger, a loop — the answer is in §3:
either it is already the default (§1), or it is not
controllable, and the agent should report that rather
than invent a mechanism. Springs named anywhere in
Phase N code or plans are zero-bounce token pairs
(`tok::motion::SNAP` / `SETTLE` / `GLIDE`) or nothing.
