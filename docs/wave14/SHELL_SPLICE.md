# Wave 14 — shell control: integrator splice spec (B2)

The exact, minimal edits to the hot files that wire
`src/shell_control.rs` (B2, new) and
`src/shell_tools_state.rs` (B3, new) into the shell
and the MCP routing, per B1's design
(`docs/wave14/SHELL_CONTROL.md`, §§1–5 on disk when
this spec was written; its §§6–9 were still being
written — where they later disagree with this spec,
the design doc wins and the integrator reconciles).
Apply **S1–S5 together**: before S3, `shell_control.rs`
does not compile (it calls
`stykanimate_rs::mcp::tool_result`, which S3 makes
public), before S5 its Centre/Actual arms do not
compile (the camera fns are private), and before S1
neither new file is declared. This is the same
all-at-once pattern as Wave 13's registry splice.

Tool names (the `shell_` prefix is B3's, from its
landed file; `shell_scrub` is B1's, from
SHELL_CONTROL §2.1; the rest are B2's proposals for
B4–B8 — if a sibling file lands under a different
name, rename it in exactly two places: the
`SHELL_TOOL_NAMES` const in S3 and
`shell_control::SHELL_TOOL_NAMES` / `command_from_call`
in `src/shell_control.rs`):

| Tool | Family | Command(s) |
|---|---|---|
| `shell_set_workspace` | B3 | `SetWorkspace` → delegated to `shell_tools_state::tool_shell_set_workspace` |
| `shell_get_state` | B3 | `GetShellState` → delegated to `shell_tools_state::tool_shell_get_state` |
| `shell_transport` | B4 | `Transport` / `SetLoopPlayback` / `SetPlaybackSpeed` |
| `shell_scrub` | B4 | `Scrub` |
| `shell_set_view` | B5 | `View` |
| `shell_select` | B6 | `Select` |
| `shell_set_theme` | B7 | `SetTheme` |
| `shell_set_panels` | B8 | `SetPanelCollapsed` / `SetColumns` |

(`SetTimelineCollapsed` has NO tool: it is
conditional on B8 wiring the timeline-collapse view
reader, per B1 §2.1. The variant and executor arm
are wired in `shell_control.rs`; nothing produces
the command until the reader lands.)

---

## S1 — `src/main.rs`: declare the two new bin modules

Anchor (the module block at the top of the file):

```rust
mod chrome_view;
mod live_mcp;
mod rune_sec;
mod stage_sprites;
mod stage_view;
```

Replace with:

```rust
mod chrome_view;
mod live_mcp;
mod shell_control;
mod shell_tools_state;
mod rune_sec;
mod stage_sprites;
mod stage_view;
```

(The `shell_tools_state` line is B3's declaration;
it is listed here so both land in the one edit.
Nothing else in `main.rs` needs a new `use`: the
poll splice below names `shell_control` by path.)

## S2 — `src/main.rs`: drain the shell queue in `poll`

Anchor (inside `poll`, in the `if live_mcp::is_enabled()`
block):

```rust
        let drained = MCP_ROUTER.with(|router| {
            live_mcp::drain(&mut s.core, &mut router.borrow_mut(), 8)
        });
        if drained > 0 {
```

Replace with:

```rust
        let drained = MCP_ROUTER.with(|router| {
            live_mcp::drain(&mut s.core, &mut router.borrow_mut(), 8)
        });
        // Wave 14 (B2): the shell commands the drain
        // queued run HERE, still inside the drain
        // window — so the project-swap and status
        // checks below cover shell-driven changes
        // exactly like core-driven ones, a shell call
        // answers in the same tick it was queued, and
        // the swap-invalidation above the shell drain
        // means a shell selection in the same burst as
        // a project load applies to the NEW project
        // (B1 §3.3, the safe direction).
        let shell_drained = shell_control::drain(s);
        if drained + shell_drained > 0 {
```

Nothing else in `poll` changes: the heartbeat wake
already runs while the bridge is enabled, so queued
shell commands drain on the existing cadence, and
the status-attribution comparison below the anchor
still brackets every `core.status` write the tick
can make (the shell executor writes none itself —
it delegates workspace/snapshot to B3's handlers,
which write none either).

## S3 — `src/mcp.rs`: routing support (three edits)

### S3a — make `tool_result` public

Anchor:

```rust
/// Wrap a tool handler outcome as an MCP `tools/call` result: the
/// value JSON-serialised into a single text content block, with
/// `isError` distinguishing failure text from result text.
fn tool_result(id: Value, outcome: Result<Value, String>) -> Value {
```

Replace the last line with:

```rust
/// Public for the shell-control plumbing
/// (`src/shell_control.rs`, editor-binary side): its
/// `respond` builds a shell command's response with
/// this same shape, so a shell tool's reply bytes are
/// identical to a registry tool's.
pub fn tool_result(id: Value, outcome: Result<Value, String>) -> Value {
```

### S3b — the shell name test + stubs, after `all_tools`

Anchor: the end of `all_tools` —

```rust
    tools.extend(crate::mcp_tools::tools());
    tools.extend(crate::mcp_io::tools());
    tools
}
```

Insert after that closing brace:

```rust
/// The shell-control tool names (Wave 14, Squad B):
/// tools that drive the running editor's GUI shell
/// rather than its core. The registry carries them as
/// refusal stubs (see [`shell_tools`]) so the stdio
/// server — which has no shell — refuses them
/// honestly; the live endpoint never runs the stubs,
/// because `live_mcp::drain` offers each message to
/// `shell_control::peek` first and routes the names
/// to the shell command queue instead. This list and
/// `shell_control::SHELL_TOOL_NAMES` are the one
/// deliberate duplication in the design: the registry
/// lives in the library, the queue in the binary, and
/// neither can call the other. (Registry entries ⇔
/// this list ⇔ `ShellCommand` variants is the
/// three-way correspondence the wave's gate pins.)
pub const SHELL_TOOL_NAMES: &[&str] = &[
    "shell_set_workspace",
    "shell_get_state",
    "shell_transport",
    "shell_scrub",
    "shell_set_view",
    "shell_select",
    "shell_set_theme",
    "shell_set_panels",
];

/// Is `name` a shell-control tool (see
/// [`SHELL_TOOL_NAMES`])?
pub fn is_shell_tool(name: &str) -> bool {
    SHELL_TOOL_NAMES.contains(&name)
}

/// The shell tools' registry entries: metadata plus
/// a refusal. The handler is never the live path
/// (the live drain intercepts the names first); it
/// exists for the stdio server, whose clients deserve
/// a refusal that names the tool and the way to reach
/// it — an `isError` result, never a silent no-op,
/// and never `-32602` (that code is reserved for
/// names ABSENT from `all_tools()`; a shell tool is
/// present, well-formed, and callable in the right
/// environment). `tools/list` keeps listing the tools
/// because they do exist, on the other transport.
///
/// The refusal handlers are one macro-stamped fn per
/// tool, because `Tool::handler` is a plain `fn`
/// pointer: a closure interpolating the tool's name
/// would capture, and capturing closures do not
/// coerce to `fn`. The wording is the design's
/// (SHELL_CONTROL §5), one place per name.
macro_rules! shell_stub_fn {
    ($fn_name:ident, $tool:literal) => {
        fn $fn_name(
            _core: &mut crate::editor::EditorCore,
            _args: &Value,
        ) -> Result<Value, String> {
            Err(concat!(
                $tool,
                " needs the live editor: shell tools drive the running GUI's \
                 shell (workspace, transport, view, panels), and the stdio \
                 server has no GUI shell. Start the editor and connect to \
                 its live endpoint http://127.0.0.1:32117/mcp instead."
            )
            .to_string())
        }
    };
}
shell_stub_fn!(shell_refusal_set_workspace, "shell_set_workspace");
shell_stub_fn!(shell_refusal_get_state, "shell_get_state");
shell_stub_fn!(shell_refusal_transport, "shell_transport");
shell_stub_fn!(shell_refusal_scrub, "shell_scrub");
shell_stub_fn!(shell_refusal_set_view, "shell_set_view");
shell_stub_fn!(shell_refusal_select, "shell_select");
shell_stub_fn!(shell_refusal_set_theme, "shell_set_theme");
shell_stub_fn!(shell_refusal_set_panels, "shell_set_panels");

fn shell_tools() -> Vec<Tool> {
    fn stub(
        name: &'static str,
        description: &'static str,
        schema: Value,
        handler: fn(&mut crate::editor::EditorCore, &Value) -> Result<Value, String>,
    ) -> Tool {
        Tool {
            name,
            description,
            schema,
            handler,
        }
    }
    vec![
        stub(
            "shell_set_workspace",
            "Switch the running editor's workspace by name: one of Animate, Draw, Graph, Effects, Audio, 3D, Export, Home (exact, case-sensitive; '3D' is the merged Build/3D workspace). The switch is the workspace chip's own action — it re-gates which panels mount and persists to the layout prefs. An unknown name is refused, listing the valid set. Shell state only: no undo step. Live endpoint only — the stdio server refuses this tool.",
            json!({
                "type": "object",
                "properties": {
                    "workspace": {
                        "type": "string",
                        "enum": ["Animate", "Draw", "Graph", "Effects", "Audio", "3D", "Export", "Home"],
                        "description": "Workspace name, exact and case-sensitive",
                    },
                },
                "required": ["workspace"],
                "additionalProperties": false,
            }),
            shell_refusal_set_workspace,
        ),
        stub(
            "shell_get_state",
            "Read the running editor's shell state in one call: the active workspace; the transport (playing or paused, current frame and fractional playhead, frame count, fps, loop range and playback toggles); the view (view-layer zoom and pan, kept separate from the authored camera at the current frame, plus the effective on-screen zoom the two compose to); the selections (figure, node, sprite, audio clip, FX clip, and the movieclip edit session if one is open); the theme; and which collapsible panels are collapsed. Read-only; no undo step. Live endpoint only — the stdio server refuses this tool.",
            json!({ "type": "object", "properties": {}, "additionalProperties": false }),
            shell_refusal_get_state,
        ),
        stub(
            "shell_transport",
            "Drive the running editor's transport: play, pause, toggle and stop mirror the chrome transport's own presses (stop is the pause press plus a return to frame 0 — the GUI has no Stop button); set_loop writes the shell-side loop-playback toggle and set_speed the playback speed, which must be one of the transport cycle's speeds (0.25, 0.5, 1.0, 1.5, 2.0). Shell state only: no undo step. Live endpoint only — the stdio server refuses this tool.",
            json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["play", "pause", "toggle", "stop", "set_loop", "set_speed"],
                        "description": "Which transport action to perform",
                    },
                    "loop": { "type": "boolean", "description": "Loop-playback toggle (action: set_loop)" },
                    "speed": {
                        "type": "number",
                        "enum": [0.25, 0.5, 1.0, 1.5, 2.0],
                        "description": "Playback speed multiplier (action: set_speed)",
                    },
                },
                "required": ["action"],
                "additionalProperties": false,
            }),
            shell_refusal_transport,
        ),
        stub(
            "shell_scrub",
            "Move the running editor's playhead to a 0-based frame index — the transport family's scrub, range-checked against the live timeline (an out-of-range frame is refused, quoting the frame and the count). The set_frame built-in moves the same playhead; this tool exists so a client driving the transport family gets transport-shaped semantics in one place. Navigation only: no undo step. Live endpoint only — the stdio server refuses this tool.",
            json!({
                "type": "object",
                "properties": {
                    "frame": {
                        "type": "integer",
                        "minimum": 0,
                        "description": "0-based frame index to jump to",
                    },
                },
                "required": ["frame"],
                "additionalProperties": false,
            }),
            shell_refusal_scrub,
        ),
        stub(
            "shell_set_view",
            "Move the running editor's view layer — the camera overlay's own actions: zoom_by (a positive factor), zoom_in / zoom_out (one overlay stepper step), fit (restore the camera's own framing), centre, and actual (the 1:1 view). View navigation only (the Wave 5 doctrine): the authored camera track is project data and is never written by this tool. Shell state only: no undo step. Live endpoint only — the stdio server refuses this tool.",
            json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["zoom_by", "zoom_in", "zoom_out", "fit", "centre", "actual"],
                        "description": "Which view action to perform",
                    },
                    "factor": { "type": "number", "exclusiveMinimum": 0, "description": "Zoom factor (action: zoom_by)" },
                },
                "required": ["action"],
                "additionalProperties": false,
            }),
            shell_refusal_set_view,
        ),
        stub(
            "shell_select",
            "Write one of the running editor's selections: kind figure (index required), node, sprite, clip, fx, audio, stroke or particle (index, or null / omitted to clear). Every index is validated against the live project before the write, so a refused call changes nothing. The figure, node and sprite selections are also reachable through the core set_selection tool; the rest live in shell state only this tool can reach. (Clip selection waits on the clips panel's pub setter and refuses until it lands.) Shell state only: no undo step. Live endpoint only — the stdio server refuses this tool.",
            json!({
                "type": "object",
                "properties": {
                    "kind": {
                        "type": "string",
                        "enum": ["figure", "node", "sprite", "clip", "fx", "audio", "stroke", "particle"],
                        "description": "Which selection to write",
                    },
                    "index": {
                        "description": "0-based index, or null to clear (not for kind figure, where it is required)",
                        "type": ["integer", "null"],
                        "minimum": 0,
                    },
                },
                "required": ["kind"],
                "additionalProperties": false,
            }),
            shell_refusal_select,
        ),
        stub(
            "shell_set_theme",
            "Set the running editor's theme: dark (true) or light (false). The write is the same core field the Settings popover's theme buttons write, and the shell persists it to the layout prefs on the same tick. Shell state only: no undo step. Live endpoint only — the stdio server refuses this tool.",
            json!({
                "type": "object",
                "properties": {
                    "dark": { "type": "boolean", "description": "True for the dark theme, false for light" },
                },
                "required": ["dark"],
                "additionalProperties": false,
            }),
            shell_refusal_set_theme,
        ),
        stub(
            "shell_set_panels",
            "Set the running editor's panel visibility, one command per call: either a collapsible panel (panel: figures or library, with collapsed) or the column visibility (left and/or right). There is no generic section collapse in this shell — exactly two panels own a collapsed flag, and this tool names only those two. Shell state only: no undo step. Live endpoint only — the stdio server refuses this tool.",
            json!({
                "type": "object",
                "properties": {
                    "panel": {
                        "type": "string",
                        "enum": ["figures", "library"],
                        "description": "A collapsible panel (requires collapsed)",
                    },
                    "collapsed": { "type": "boolean", "description": "The named panel's collapsed flag (requires panel)" },
                    "left": { "type": "boolean", "description": "Left column visible" },
                    "right": { "type": "boolean", "description": "Right column visible" },
                },
                "additionalProperties": false,
            }),
            shell_refusal_set_panels,
        ),
    ]
}
```

Census note: `all_tools()` grows by eight
(288 → **296**). The health endpoint's tool count
follows automatically (it reads `all_tools()`).
Wherever the gate or `scripts/mcp_census.py` pins
the built-in family at 44 or the total at 288,
those pins move +8 — E2/E9 own the recount; this
splice only names the delta.

### S3c — register the stubs in `all_tools`

Anchor:

```rust
    tools.extend(crate::mcp_tools::tools());
```

Insert before it:

```rust
    tools.extend(shell_tools());
```

## S4 — `src/live_mcp.rs`: the peek in `drain`

Not "nothing" — one contained insertion in `drain`,
plus the module-doc paragraph (S4b). The argument,
briefly: `drain` is the only point in the program
where a queued message, its reply channel, and
(via the S2 call in `poll`) the live `State` meet.
`mcp.rs` cannot intercept — its dispatch is
parameterised over `EditorCore` and serves the
stdio server too, which must reach the stubs.
`main.rs` cannot intercept — the request queue is
private to the bridge. So the offer lives here, and
it is deliberately shallow: [`peek`] decides, this
code only moves channels.

### S4a — the insertion in `drain`

Anchor (inside `drain`'s loop):

```rust
        let Some(req) = discard_if_cancelled(req) else {
            done += 1;
            continue;
        };
        // Snapshot the status line so an error that never
        // wrote it can be told apart from an ordinary refusal
        // (which writes the status and is toasted from there).
```

Insert between the `discard_if_cancelled` block and
the `// Snapshot the status line` comment:

```rust
        // Wave 14 (B2): a shell tool call is not routed
        // through `handle_on` — its target is the shell
        // (`State`), not the core. `shell_control::peek`
        // decides: NotShell falls through to the routing
        // below untouched; Refuse is answered here, in
        // the shared response shape; Command moves the
        // request's reply channel + cancellation flag
        // into the shell queue, whose drain (called by
        // `poll` immediately after this one) executes it
        // against the live `State` and answers on that
        // channel. Shell requests skip the attribution
        // snapshot below: their errors ride inside the
        // tool result itself, and the executor never
        // writes `core.status`, so there is nothing to
        // attribute. Cancellation already ran above: a
        // cancelled shell request never enqueues.
        match crate::shell_control::peek(&req.msg) {
            crate::shell_control::Peek::NotShell => {}
            crate::shell_control::Peek::Refuse(refusal) => {
                let id = req.msg.get("id").cloned().unwrap_or(Value::Null);
                let _ = req.reply.send(Some(crate::shell_control::respond(
                    id,
                    Err(refusal),
                )));
                done += 1;
                continue;
            }
            crate::shell_control::Peek::Command(command) => {
                let request = crate::shell_control::ShellRequest {
                    id: req.msg.get("id").cloned().unwrap_or(Value::Null),
                    command,
                    reply: req.reply,
                    cancelled: req.cancelled,
                };
                if let Err((request, refusal)) =
                    crate::shell_control::enqueue(request)
                {
                    // Full queue: the request came back —
                    // answer its refusal on its own
                    // channel rather than dropping it
                    // into a disconnect.
                    let _ = request.reply.send(Some(crate::shell_control::respond(
                        request.id,
                        Err(refusal),
                    )));
                }
                done += 1;
                continue;
            }
        }
```

(One borrow note for the applier: `peek` borrows
`req.msg` only for the call — the returned `Peek`
owns its payload — so moving `req.reply` /
`req.cancelled` in the `Command` arm is sound, and
each arm either continues or falls through with
`req` whole.)

### S4b — module doc paragraph

Append to the `## Synchronisation` section of the
module docs:

```rust
//! Wave 14 adds one routing exception inside
//! [`drain`]: a `tools/call` for a shell tool (the
//! names in `stykanimate_rs::mcp::SHELL_TOOL_NAMES`)
//! is offered to `shell_control::peek` before
//! routing. Commands move onto the shell command
//! queue with the request's own reply channel as
//! their result slot; `poll`'s shell drain executes
//! them against the live `State` in the same tick,
//! so the reply a client waits on is still produced
//! on this module's cadence — one queue-hop later,
//! through the shared `mcp::tool_result` shape.
```

## S5 — `src/rune_sec/camera_panel.rs`: the two view presses go `pub(crate)`

`shell_control`'s Centre/Actual arms call the
overlay's own fns (B1 §2.1, its splice 7) — exact
GUI behaviour, no re-implementation. Both are
private today. One word each:

Anchor (`src/rune_sec/camera_panel.rs:1237`):

```rust
fn view_actual(s: &mut State) {
```

Replace with:

```rust
pub(crate) fn view_actual(s: &mut State) {
```

Anchor (`src/rune_sec/camera_panel.rs:1255`):

```rust
fn view_centre(s: &mut State) {
```

Replace with:

```rust
pub(crate) fn view_centre(s: &mut State) {
```

## S6 — the files that need nothing

- **`src/lib.rs` — nothing.** `shell_control` must
  NOT be a library module: `State` is defined in the
  binary crate root (`src/main.rs:146`) and the
  library has no access to it, by construction —
  that asymmetry is what makes the shell tools
  live-only in the first place. The queue, the
  command enum and the executor are all bin-side;
  the library's whole share of the design is S3
  (the name list, the stubs, one `pub`).
- **`src/bin/mcp_server.rs` — nothing.** The stdio
  server's shell behaviour comes entirely from S3's
  registry stubs, reached through its existing
  `server.handle(...)` path: a shell name answers
  with the stub's `isError` refusal. No stdio-side
  code can or should grow a shell.
- **`src/rune_sec/mod.rs` — nothing.**
  `shell_control` is a crate-root module like
  `stage_view`, not a `rune_sec` section.
- **`Cargo.toml` — nothing.** `serde_json` is
  already a package dependency (`Cargo.toml`
  declares `serde_json = "1"`), and both new bin
  modules use it the way `shell_tools_state.rs`
  already does.

## Known follow-on splices (not B2's, named so they are not lost)

- **B6**: the `clips_panel` pub setter over
  `update_panel_state` (B1 splice 8). Until it
  lands, `shell_select` kind `clip` refuses at
  execute time with that reason stated.
- **B8**: the timeline-collapse view reader (B1
  splice 3). Until it lands, `SetTimelineCollapsed`
  has no producing tool.
- **B1 §§6–9** (the tool table, its own splice
  list, gates) were still being written when this
  spec was finalised; the integrator reconciles
  any naming differences there first, per this
  spec's header.

## Post-splice checks for the integrator

1. Gate check 9 / census: totals move as named in
   S3b (288 → 296, if B4–B8 land under exactly the
   eight names tabled here).
2. The live-MCP headless probe (D-squad extends it)
   should exercise at least `shell_get_state` and
   `shell_set_workspace` end-to-end over HTTP, and
   one shell name over stdio expecting the refusal.
3. `shell_control.rs`'s unit tests cover `peek`,
   the queue, the translator and `respond`; the
   executor arms are covered by B10's Runtime tests.
