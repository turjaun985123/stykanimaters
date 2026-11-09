# Wave 14 — Shell control design (B1)

**What this is.** The complete design the B squad builds
to so the live MCP endpoint can drive the GUI *shell* —
workspace, transport, view, shell-side selections, theme,
panels — and not only the `EditorCore`. Every type, field
and function named below was read from the entering tree
(Wave 13 close, `92c5d95`); file:line anchors are given
throughout, and the integrator's splice list is §7.

**Why the shell is unreachable today.** The live path is:
HTTP listener thread → `LiveRequest` queue → the shell's
`poll` → `live_mcp::drain(&mut s.core, router, 8)`
(`src/main.rs:520`) → `McpServer::handle_on(core, msg)`
(`src/mcp.rs:166`). Every registered `Tool` handler has
the signature `fn(&mut EditorCore, &Value) ->
Result<Value, String>` (`src/mcp.rs:53-60`), so a handler
can name the core and nothing else. The shell's state —
`State` (`src/main.rs:146`), the view navigation
thread-local in `stage_view`, the clips panel
thread-local in `clips_panel` — has no path in. This
design adds that path without changing the `Tool`
contract, the `McpServer` router, or either transport's
wire behaviour for existing tools.

**What is already core-reachable (not B-squad scope).**
Honesty about the entering surface: `set_playback`
(play/pause/toggle/restart/play_range, `src/mcp.rs:1605`),
`set_selection` (figure + node, `src/mcp.rs:1679`),
`set_frame` (playhead, via `EditorCore::goto`,
`src/editor.rs:996`) and `get_editor_status`
(`src/mcp.rs:1551`) already work over the live path,
because `poll` observes the core flags they flip. The
shell gap is exactly the state that lives *outside*
`EditorCore`, enumerated in §2 — no more, no less.

---

## 1. Where shell state actually lives (source census)

| State | Home | Anchor |
|---|---|---|
| Active workspace | `State.workspace: rune_sec::workspaces::Workspace` | `src/main.rs:146` (struct), field `workspace` |
| Workspace switch fn | `workspaces::select_workspace(s, ws)` — sets the field + load-modify-saves `LayoutPrefs` | `src/rune_sec/workspaces.rs:304` |
| Playing flag | `EditorCore.playing` (core — but its *shell* accompaniments below are not) | `src/editor.rs:38` |
| Play/pause press | `chrome_view::toggle_play(s)` — syncs `core.playhead = core.current` when starting, then flips `core.playing` | `src/chrome_view.rs:211` |
| Loop-playback toggle | `State.loop_playback: bool` | `src/main.rs` (State field) |
| Playback speed | `State.playback_speed: f32`; GUI cycles `SPEEDS: [f32; 5] = [0.25, 0.5, 1.0, 1.5, 2.0]` via `cycle_playback_speed` | `src/rune_sec/playback_controls.rs:393, 440` |
| End-of-range stop | `playback_controls::should_stop_at_end(...)`, read by `poll` from `State.loop_playback` + core fields | `src/main.rs` (`poll`) |
| View navigation | `stage_view::ViewNav { zoom: f32, pan: [f32; 2] }` in a `thread_local!` `Cell`; `view_nav()` / `set_view_nav()` / `reset_view_nav()` / `view_zoom_by(f)` / `view_zoom_at(...)` / `view_centre_on(core, pt)`; zoom clamped `VIEW_ZOOM_MIN 0.1 ..= VIEW_ZOOM_MAX 8.0` | `src/stage_view.rs:143, 165-167, ~178-205, 295, 329` |
| Overlay Fit / Centre / 1:1 | Fit → `stage_view::reset_view_nav()`; Centre → `camera_panel::view_centre` (**private** fn); 1:1 → `camera_panel::view_actual` (**private** fn); steppers → `view_zoom_by(ZOOM_STEP)` / `view_zoom_by(1.0 / ZOOM_STEP)`, `ZOOM_STEP = 1.25` | `src/rune_sec/camera_panel.rs:229, 1237, 1255, 1312-1333` |
| Theme | `EditorCore.light_theme: bool` (core-carried); the settings buttons write it, and `poll` persists `LayoutPrefs.dark = !light_theme` on change | `src/editor.rs:69`; `src/rune_sec/settings_popover.rs:32-41`; `poll` theme block, `src/main.rs` |
| Figure / node selection | `EditorCore.selected_figure: usize`, `EditorCore.selected_node: Option<usize>`; op `EditorCore::select_figure(fi)` | `src/editor.rs:34-35, 2137` |
| Sprite selection | `State.sprites_panel.selected: Option<usize>` (`SpritesPanelState`) | `src/rune_sec/sprites_panel.rs:168, 173` |
| Movieclip selection | `ClipsPanelState.selected: Option<usize>` in the `CLIPS_PANEL` thread-local; accessors `panel_state()` / `update_panel_state(f)` are **private** fns | `src/rune_sec/clips_panel.rs:236, 254-255, 300, 305` |
| FX-clip selection | `State.fx_selected: Option<usize>` | `src/main.rs` (State field) |
| Audio-clip selection | `State.audio_panel.selected: Option<usize>` (`AudioPanelState`) | `src/rune_sec/audio_panel.rs:267, 277` |
| Stroke / particle selection | `State.stroke_selected: Option<usize>`, `State.particle_selected: Option<usize>` | `src/main.rs` (State fields) |
| Figures panel collapse | `State.figures_panel.collapsed: bool` (`FiguresPanelState`) — header Hide/Show writes it directly | `src/rune_sec/figures_panel.rs:214, 974` |
| Library panel collapse | `State.library_panel.collapsed: bool`; flip fn `library_panel::toggle_collapsed(s)` | `src/rune_sec/library_panel.rs:416, 922` |
| Column visibility | `State.show_left: bool`, `State.show_right: bool`; `view` mounts each column only while its flag is set | `src/main.rs:191-192, 1138, 1156` |
| Section mounting | Derived, not stored: `workspaces::section_visible(ws, section)` over the 30-variant `Section` enum — a pure function of the workspace | `src/rune_sec/workspaces.rs:143, 555` |
| Timeline collapse | `LayoutPrefs.timeline_collapsed: bool` — **persisted but dead**: it round-trips through `shell::load_layout` / `save_layout`, and *no shell code reads it* (grep over `src/`: only `src/shell.rs:409` and prefs tests in `workspaces.rs` name it). The timeline (`timeline_marks`) mounts unconditionally in `view`'s centre column | `src/shell.rs:402, 409`; `src/main.rs` (`view`) |
| Guides | `State.guides`, `State.guides_frustum`, `State.guides_safe: bool` | `src/main.rs` (State fields) |
| Live-MCP on/off | `live_mcp::is_enabled()` | `src/live_mcp.rs:462` |

Two consequences fall out of the census and shape the
whole design:

1. **There is no generic section-collapse mechanism.**
   Exactly two panels own a `collapsed` flag (Figures,
   Library). `Section` gates *mounting by workspace*,
   not collapse. A shell tool that claimed to collapse
   an arbitrary section would be a fake control, so
   §2's panel command names the two real panels and
   refuses everything else.
2. **The timeline-collapse prefs field is a discovered
   dead wire.** Writing it changes nothing on screen
   today. §2 therefore makes the timeline tool
   *conditional* on B8 wiring the missing reader; the
   tool is not registered until the wiring lands (§7,
   splice 3).

---

## 2. The `ShellCommand` enum

Defined in the new `src/shell_control.rs` (bin crate —
see §3 for why). One variant per user-meaningful shell
action; argument validation happens in `peek` (§4), so
a `ShellCommand` value is always executable.

```rust
pub enum ShellCommand {
    /// Read-only snapshot of the shell state (§2.1).
    GetShellState,
    /// workspaces::select_workspace(s, ws)
    SetWorkspace(Workspace),
    /// chrome_view::toggle_play compositions (§2.2).
    Transport(TransportAction),
    /// s.loop_playback = b
    SetLoopPlayback(bool),
    /// s.playback_speed = v, v ∈ SPEEDS
    SetPlaybackSpeed(f32),
    /// s.core.goto(frame), range-checked in peek
    Scrub { frame: usize },
    /// The view layer only — never a camera op.
    View(ViewCommand),
    /// One selection write (§2.3).
    Select(SelectTarget),
    /// s.core.light_theme = !dark (poll persists)
    SetTheme { dark: bool },
    /// figures_panel.collapsed / library_panel.collapsed
    SetPanelCollapsed { panel: CollapsiblePanel, collapsed: bool },
    /// s.show_left / s.show_right (None = leave)
    SetColumns { left: Option<bool>, right: Option<bool> },
    /// CONDITIONAL — ships only with splice 3 (§1, §7).
    SetTimelineCollapsed(bool),
}

pub enum TransportAction { Play, Pause, Toggle, Stop }

pub enum ViewCommand {
    ZoomBy { factor: f32 },   // stage_view::view_zoom_by
    ZoomIn,                   // view_zoom_by(ZOOM_STEP)
    ZoomOut,                  // view_zoom_by(1.0 / ZOOM_STEP)
    Fit,                      // stage_view::reset_view_nav()
    Centre,                   // camera_panel::view_centre(s)
    Actual,                   // camera_panel::view_actual(s) — the 1:1 press
}

pub enum CollapsiblePanel { Figures, Library }

pub enum SelectTarget {
    Figure(usize),            // core.select_figure(i)
    Node(Option<usize>),      // core.selected_node, figure-relative
    Sprite(Option<usize>),    // s.sprites_panel.selected
    Clip(Option<usize>),      // CLIPS_PANEL via B6's new pub setter
    Fx(Option<usize>),        // s.fx_selected
    Audio(Option<usize>),     // s.audio_panel.selected
    Stroke(Option<usize>),    // s.stroke_selected
    Particle(Option<usize>),  // s.particle_selected
}
```

### 2.1 Execution semantics (exact writes)

`execute(s: &mut State, cmd: &ShellCommand) ->
Result<Value, String>` performs, per variant:

- **GetShellState** — no writes. Returns the §2.4
  snapshot.
- **SetWorkspace(ws)** — calls
  `rune_sec::workspaces::select_workspace(s, ws)`
  verbatim: the same fn the workspace chips and the
  Alt+1..8 handler (`src/main.rs`, `on_key`) call, so
  field write + `LayoutPrefs` persistence + field
  preservation are inherited, not re-implemented.
- **Transport(Play)** — if `!s.core.playing`,
  `chrome_view::toggle_play(s)` (which first syncs
  `core.playhead` to `core.current`, the press
  semantics at `src/chrome_view.rs:211`); no-op
  success if already playing. **Pause** — the mirror.
  **Toggle** — `toggle_play(s)` unconditionally.
  **Stop** — the GUI has *no* Stop button; the design
  defines it as pause + return to frame 0:
  `if s.core.playing { toggle_play(s) }` then
  `s.core.goto(0)` (`goto` also resets `playhead`,
  `src/editor.rs:996-1001`). Playback audio follows
  automatically: `poll`'s `was_playing` flip hook
  starts/stops it for every source, the shell's own
  one-place design. Empty project (zero frames):
  refusal, `"the project has no frames to play"` —
  `set_playback`'s existing refusal text
  (`src/mcp.rs:1629`), reused word-for-word.
- **SetLoopPlayback(b)** — `s.loop_playback = b`. This
  is the `pb-loop` toggle's field; `poll` reads it in
  `should_stop_at_end`. No core write, no snapshot.
- **SetPlaybackSpeed(v)** — `s.playback_speed = v`.
  `peek` requires `v` to equal one of
  `playback_controls::SPEEDS` (`[0.25, 0.5, 1.0, 1.5,
  2.0]`, `src/rune_sec/playback_controls.rs:393`):
  parity means MCP reaches exactly the states the
  GUI's cycle reaches, and the refusal lists the set.
  (Finiteness is still checked first — the standing
  contract — even though set membership subsumes it.)
- **Scrub{frame}** — `s.core.goto(frame)`. `goto`
  itself silently ignores an out-of-range frame
  (`src/editor.rs:997`), so `peek` refuses
  `frame >= frame_count` with the offending frame and
  the count quoted, per the standing refusal contract.
  Note the deliberate overlap: the `set_frame`
  built-in already moves the core playhead; `shell_
  scrub` exists so a client driving the *transport*
  family gets range-checked, transport-shaped
  semantics in one place. Both write the same field
  the same way.
- **View(cmd)** — view layer only (Wave 5 doctrine:
  view navigation never authors camera keys, takes no
  snapshot, bumps no edit counter). ZoomBy →
  `stage_view::view_zoom_by(factor)` (factor finite
  and > 0, checked in `peek`; the clamp to
  `0.1..=8.0` is `set_view_nav`'s own). ZoomIn/Out →
  `view_zoom_by(ZOOM_STEP)` / `view_zoom_by(1.0 /
  ZOOM_STEP)` with `camera_panel::ZOOM_STEP = 1.25` —
  the overlay steppers' exact calls
  (`src/rune_sec/camera_panel.rs:1329-1333`). Fit →
  `stage_view::reset_view_nav()` — the overlay's
  `view-fit` handler verbatim (note: the overlay's
  "Fit" *is* reset-to-camera-framing in this shell;
  the design names what the button does, not what
  other editors mean by fit). Centre →
  `camera_panel::view_centre(s)`; Actual →
  `camera_panel::view_actual(s)`. Both fns are private
  today (`src/rune_sec/camera_panel.rs:1255, 1237`);
  splice 7 (§7) makes them `pub(crate)` — one word
  each, exact GUI behaviour, no re-implementation.
- **Select(t)** — validated in `peek` against the
  live counts where the count is core-known (figure,
  node, sprite, fx, audio, particle) and at execute
  time where it is not (clip, stroke — see below);
  an out-of-range index refuses with the index and
  the count quoted, and leaves every selection
  untouched. Figure → `s.core.select_figure(i)`
  (`src/editor.rs:2137`; clears the node selection,
  exactly the `set_selection` built-in's semantics).
  Node → `s.core.selected_node = v` after checking
  `v < s.core.project.figures[s.core.selected_figure]
  .def.nodes.len()`. Sprite → `s.sprites_panel.
  selected = v`. Clip → B6's new pub setter in
  `clips_panel.rs` over `update_panel_state`
  (splice 8). Fx → `s.fx_selected = v`. Audio →
  `s.audio_panel.selected = v`. Stroke →
  `s.stroke_selected = v`; Particle →
  `s.particle_selected = v`. `None` clears, and
  clearing is always in range. Clip and stroke counts
  are not cheaply knowable at `peek` time (peek sees
  the message, not the project), so for those two the
  executor re-validates against the project it finds
  and refuses the same way — validation is never
  skipped, only staged.
- **SetTheme{dark}** — `s.core.light_theme = !dark`.
  That single field write is the whole GUI mechanism:
  the settings theme buttons write it
  (`src/rune_sec/settings_popover.rs:32-41`), the
  view themes from it, and `poll` persists
  `LayoutPrefs.dark` when it changes. Nothing else
  to wire.
- **SetPanelCollapsed{panel, collapsed}** — Figures →
  `s.figures_panel.collapsed = collapsed`; Library →
  `s.library_panel.collapsed = collapsed` (the field
  `library_panel::toggle_collapsed` flips). Any
  other panel name is refused in `peek`, naming the
  two supported panels (§1, consequence 1).
- **SetColumns{left, right}** — `s.show_left` /
  `s.show_right` written for each `Some` side.
- **SetTimelineCollapsed(b)** — *conditional*: writes
  `LayoutPrefs.timeline_collapsed` by load-modify-
  save (the `prefs_with_workspace` pattern,
  `src/rune_sec/workspaces.rs:~291`) **and** only
  ships with splice 3, the missing view reader. If
  B8's wiring slips, this variant and its tool are
  omitted — never registered as a prefs-only no-op.

### 2.2 The undo / dirty contract for shell tools

Shell commands are **session actions, not project
edits**: `execute` takes no undo snapshot, calls no
`mark_dirty`, and bumps no edit counter — the same
class as a workspace-chip press, which has never
snapshotted. The standing "every mutating tool =
exactly one undo step" contract binds *project*-
mutating tools; shell tools are exempt **by kind**,
and each tool's `docs/MCP.md` row must say so ("shell
tool — drives the GUI session only; no undo step").
`Scrub` is navigation, already exempt in the core
surface (`set_frame`). `SetTheme` / `SetWorkspace` /
`SetTimelineCollapsed` persist to `LayoutPrefs` —
the GUI's own prefs file, written by the GUI's own
fns — never to the project file.

### 2.3 The snapshot (`shell_get_state` result)

One JSON object, read in a single `execute` pass:

```json
{
  "workspace": "Animate",            // Workspace::name()
  "workspace_key": "ws-animate",     // Workspace::key()
  "sections_visible": ["Figures", "…"],  // Section::all() filtered by section_visible(workspace, ·)
  "playing": false,                  // core.playing
  "current_frame": 0,                // core.current
  "playhead": 0.0,                   // core.playhead
  "frame_count": 5,                  // core.frame_count()
  "loop_playback": true,             // State
  "playback_speed": 1.0,             // State
  "loop_range": [0, 4],              // core.loop_range or null
  "theme": { "dark": true },         // !core.light_theme
  "view": {                          // stage_view::view_nav() + view_zoom_factor(core)
    "zoom": 1.0, "pan": [0.0, 0.0], "effective_zoom": 1.0
  },
  "selection": {
    "figure": 0, "node": null,       // core
    "sprite": null,                  // sprites_panel.selected
    "clip": null,                    // clips panel_state().selected (via B6's pub read)
    "fx": null, "audio": null,       // State / audio_panel
    "stroke": null, "particle": null // State
  },
  "panels": {
    "figures_collapsed": false, "library_collapsed": false,
    "show_left": true, "show_right": true,
    "timeline_collapsed": false      // load_layout().timeline_collapsed
  },
  "guides": { "guides": false, "frustum": true, "safe": true },
  "live_mcp": true                   // live_mcp::is_enabled()
}
```

It complements, never duplicates, the core snapshots:
project shape stays `get_state`'s job, core session
facts stay `get_editor_status`'s; this is the *shell*
half neither can see.

---

## 3. The channel (`src/shell_control.rs`)

### 3.1 Placement: bin crate, beside `live_mcp`

`shell_control.rs` is a new **bin-crate** module
(`mod shell_control;` next to `mod live_mcp;` in
`src/main.rs`). It cannot live in the library: the
executor names `crate::State`, `crate::stage_view`,
`crate::chrome_view` and `crate::rune_sec::*`, all
bin items the library has no access to. `live_mcp.rs`
itself is the precedent — bin-side glue over the
library's `McpServer`. No `lib.rs` change, no
`rune_sec/mod.rs` change, **no new `State` field**
(the State census stays untouched; shell plumbing
already lives outside `State` — the `MCP_ROUTER`
thread-local in `src/main.rs` is the standing
example, and this channel goes one better: a static
in its own module).

### 3.2 The queue and the result slots

```rust
pub struct ShellRequest {
    /// The JSON-RPC id, cloned from the message at peek
    /// time; echoed in the response.
    pub id: Value,
    /// Validated, ready to execute.
    pub command: ShellCommand,
    /// THE RESULT SLOT: the originating LiveRequest's
    /// own reply channel (src/live_mcp.rs, LiveRequest
    /// ::reply: SyncSender<Option<Value>>). The HTTP
    /// listener thread is blocked on its peer right
    /// now; sending Some(response) here is what answers
    /// the HTTP request.
    pub reply: SyncSender<Option<Value>>,
    /// The originating LiveRequest's cancellation flag,
    /// shared by reference. Checked before executing,
    /// exactly as live_mcp's discard_if_cancelled does:
    /// a request whose client already got a 504 must
    /// not execute late against a different project.
    pub cancelled: Arc<AtomicBool>,
}

static SHELL_QUEUE: Mutex<VecDeque<ShellRequest>>
    = Mutex::new(VecDeque::new());

/// Side-queue bound. The bridge queue feeding it is
/// itself bounded at 8 (live_mcp::MAX_QUEUED_REQUESTS),
/// so this can only fill if poll stalls; the bound
/// keeps a wedged shell from accumulating commands.
pub const MAX_SHELL_QUEUED: usize = 8;
```

All synchronisation is `std`: one `Mutex<VecDeque<_>>`
(the `DRAIN_ERRORS` / `BRIDGE` statics pattern in
`src/live_mcp.rs:375, 411`) plus the per-request
`sync_channel(1)` the bridge already created. No new
thread is spawned, ever: the listener thread enqueues
(indirectly, via `drain`), the UI thread executes.

### 3.3 The flow, tick by tick

1. A client POSTs a `tools/call` for a `shell_*` tool
   to `http://127.0.0.1:32117/mcp`. The listener parses
   it into a `LiveRequest { msg, reply, cancelled }`
   and queues it on the bridge, then blocks on the
   reply channel (its budget is `REPLY_TIMEOUT`,
   600 s — a one-tick delay is invisible against it).
2. `poll` runs `live_mcp::drain(&mut s.core, router,
   8)` as today. Inside the drain loop, each dequeued
   request is offered to `shell_control::peek(&msg)`
   **before** `router.handle_on` (splice 2, §7):
   - `Peek::NotShell` → routed exactly as today.
   - `Peek::Refuse(text)` → the drain sends the §5
     refusal response on `req.reply` at once; nothing
     is queued, nothing executes.
   - `Peek::Command(cmd)` → `shell_control::enqueue`
     moves the request's `reply` + `cancelled` and the
     cloned `id` into a `ShellRequest` on
     `SHELL_QUEUE`. (Queue full → immediate refusal
     response, `"shell command queue is full"`.)
3. Immediately after the core drain, still inside
   `poll`'s `is_enabled` block, `shell_control::
   drain(s)` runs (splice 1, §7): pop front; skip +
   defensive `None` reply if `cancelled`; else
   `catch_unwind(execute)` and send the §3.4 response
   on the slot. Returns the count; `poll` sets
   `redraw = true` when > 0, mirroring the core drain.
4. The listener wakes with the response and writes
   the HTTP 200. The redraw the same tick is what the
   client would see if it were watching the window:
   workspace switched, playhead moved, theme flipped.

**Ordering.** Within one tick, core calls drained in
step 2 execute before shell commands enqueued in the
same batch; shell commands execute in arrival order
among themselves. A mixed burst is therefore
reordered at tick granularity (≤ 8 requests) — the
same batching the core drain already imposes, stated
here so no one promises stricter ordering. One
interplay to document in MCP.md: `poll`'s existing
project-swap invalidation (it clears
`stroke_selected` / `particle_selected` /
`fx_selected` / `confirm_action` when the core drain
swapped the project, `src/main.rs`, `poll`) runs
*before* the shell drain, so a `shell_select` in the
same burst as a project load applies to the **new**
project — the safe direction.

**Why not execute inside the handler?** The obvious
alternative — the registry handler itself enqueueing
and blocking on a result slot — deadlocks on the
live path: the handler runs *inside* `handle_on`,
which runs *inside* the core drain on the UI thread,
so the shell drain later in the same `poll` could
never run to answer it. Equally, a handler that
touched shell state through a thread-local would be
executing against whichever `State` the *router's*
thread sees — the stdio server has none. Interception
before routing is the only shape that is correct on
both transports, and it keeps `McpServer` and the
`Tool` signature untouched.

### 3.4 Response construction

`shell_control::respond(id, outcome) -> Value` builds
byte-identical shapes to `mcp.rs`'s own (the private
`tool_result` / `error_response` fns in
`src/mcp.rs`): success and handler-refusal are
`result` objects with one text content block and
`isError` false/true; the text is the outcome JSON
serialised (success) or the refusal string (error).
`execute` is wrapped in `catch_unwind` — the shell
branch gets the same panic net `tools_call_result`
gives handlers (`src/mcp.rs`, Wave 11 A5), answering
`isError` `"tool panicked: {name}"` rather than
unwinding into the Rune event loop.

---

## 4. `peek`: translation and validation

```rust
pub enum Peek {
    NotShell,
    Refuse(String),
    Command(ShellCommand),
}

pub fn peek(msg: &Value) -> Peek
```

`peek` returns `NotShell` unless **all** hold: the
message is an object, `method == "tools/call"`, it
carries an `id` (an id-less shell call is a
notification: `dispatch` never answers those, and
the stub it would reach mutates nothing — leaving it
to the normal path preserves the notification rule
exactly), and `params.name` is a string starting
with `shell_`. Otherwise it translates `params.
arguments` into a `ShellCommand`:

- Unknown `shell_*` name (prefix matches, no such
  tool) → `Refuse` with §5's unavailable text? **No**
  — an unregistered name must keep the one true
  unknown-tool behaviour. `peek` therefore checks the
  name against the **registered shell tool table**
  (§6): a `shell_*` name the registry does not carry
  returns `NotShell`, flows into `handle_on`, and
  earns the standard `-32602 "unknown tool: {name}"`
  there. The prefix gets a request *examined*; only
  a registered name gets it *intercepted*. E2/E10's
  gate pins the three-way correspondence — registry
  entries ⇔ peek arms ⇔ `ShellCommand` variants —
  so the table cannot silently drift.
- Bad arguments (wrong type, non-finite float,
  fractional frame, unknown workspace / panel /
  select-kind string, speed outside `SPEEDS`,
  out-of-range index where the count is core-known
  via… — see the staging note in §2.1) →
  `Refuse(text)` with the offending value quoted, in
  the house refusal style (the `mcp.rs` arg helpers'
  wording is the model). Refusals are `isError`
  results, never protocol errors: the call was
  well-formed, the tool exists, the *arguments* fail
  — the same distinction `tools_call_result` draws.

Workspace strings parse through
`workspaces::from_name` semantics but must **not**
inherit its silent fallback: `from_name` maps an
unknown string to `Workspace::Animate`
(`src/rune_sec/workspaces.rs:~276`), which would
make `{"workspace": "Anmate"}` a silent success.
`peek` matches against `Workspace::all()` names
itself and refuses an unmatched string, listing the
eight names (`"3D"` included for `Build3D`).

---

## 5. Dispatch: live vs stdio, and the exact error shapes

**One registry, both transports.** `tools/list` is
`all_tools()` on both paths (`src/mcp.rs:65`), so the
shell tools appear in the stdio server's list too.
That is deliberate: D2's parity harness can then diff
the two lists clean, and a client can discover the
shell family and its live-only nature from the list
alone (each shell tool's description says so).

**Live** — §3.3: intercepted in the drain, executed
against the real `State`, answered on the request's
own reply channel. The registry handler never runs.

**Stdio** — `src/bin/mcp_server.rs`'s `run` loop
calls `server.handle(&msg)` (`src/bin/mcp_server.rs:
180`), which reaches `tools_call_result`, finds the
registered tool, and runs its handler — which for
every shell tool is the shared stub:

```rust
fn shell_unavailable(_core: &mut EditorCore, _args: &Value)
    -> Result<Value, String>
{
    Err(SHELL_UNAVAILABLE_TEXT.to_string())  // name interpolated
}
```

answering an **`isError` tool result** (the
`tool_result` shape), text:

> `shell_set_workspace needs the live editor: shell
> tools drive the running GUI's shell (workspace,
> transport, view, panels), and the stdio server has
> no GUI shell. Start the editor and connect to its
> live endpoint http://127.0.0.1:32117/mcp instead.`

**Why `isError`, and why never `-32602`.** `-32602`
with the literal text `unknown tool: {name}` is the
standing protocol error for a name **absent from
`all_tools()`** (`src/mcp.rs:253-263`) — and a shell
tool is present, well-formed, and callable in the
right environment. Claiming "unknown" would (a) lie,
(b) break the contract D5/E10 verify, and (c) collide
with the shell to
...[truncated 9113 chars]