# Driving StykAnimate-RS from an AI agent (MCP)

StykAnimate-RS ships an MCP (Model Context Protocol) server core:
any MCP-capable AI client can create, pose, decorate, render and
export animations end-to-end. The same tool registry is reachable
over **two transports**:

1. **stdio** — the standalone `mcp_server` binary, holding its own
   private editing session.
2. **Live control** — an HTTP endpoint hosted by the running
   editor itself, executing against the project in the window.

Both transports route through the same JSON-RPC router
(`McpServer::handle` / `McpServer::handle_on` in `src/mcp.rs`) and
the same registry (`all_tools()` in `src/mcp.rs`). A tool behaves
identically on both; only the session it edits differs — with
one named exception: the 35 `shell_*` tools act on the GUI
shell; 31 of them take effect only through live control (on
stdio they answer an error result), and the 4 transport tools
execute on stdio against its own core.

## Tool count — 366, and how to check it

At the time of writing the registry holds **366 tools**:

| Source | Count | Where they are defined |
|---|---|---|
| Built-ins | 44 | `fn tool_*` constructors assembled in `all_tools()`, `src/mcp.rs` |
| Editing registry | 313 | `pub fn tools()`, `src/mcp_tools.rs` |
| File IO / export | 9 | `pub fn tools()`, `src/mcp_io.rs` |
| **Total** | **366** | `all_tools()` = built-ins, then `mcp_tools`, then `mcp_io` |

Runtime cross-check: call the `server_info` tool — it returns
`all_tools().len()` — or count the entries of a `tools/list`
response. Source cross-check: `scripts/mcp_census.py` re-derives
the whole surface from the three source files and fails on any
duplicate name, unregistered handler or orphan handler; the
pre-commit gate (check 9 of `scripts/phase_j_checks.py`) also
verifies that every registered tool is named in the catalogue
below and that the catalogue names no tool that does not exist.
35 of the registry tools are the **shell family** (`shell_*`): they drive the running editor's GUI shell — see the shell catalogue table below.

The catalogue lists every tool by name, in registry order.

## Protocol (both transports)

- JSON-RPC 2.0. Methods: `initialize`, `ping`, `tools/list`,
  `tools/call`. Protocol version `2024-11-05`.
- `initialize` answers with `capabilities: { "tools": {} }` and
  `serverInfo: { "name": "stykanimate-rs", "version": <Cargo
  package version> }`.
- One message is one JSON object. **JSON-RPC batches are not
  supported**: a JSON array gets a `-32600` error. Batch elements
  are never executed; an all-notification batch gets the same
  single `-32600`, not silence.
- Notifications (messages with no `id`) are never answered,
  and a notification also executes nothing — a `tools/call`
  notification mutates nothing on either transport — whatever
  the method — on stdio no response line is written; on
  the live endpoint the HTTP response is `202` with an empty body.
  `notifications/initialized` is accepted this way; it is not
  required before other calls.
- Protocol errors: `-32700` parse error (id `null`), `-32600`
  invalid request (not an object, or missing `method`), `-32601`
  unknown method, `-32602` bad `tools/call` params (missing
  `params` or missing tool `name`) **or an unknown tool name**:
  the error message is exactly `unknown tool: {name}`. An
  unknown tool name is the one call failure that never reaches
  a handler, so it is a protocol error, not a tool result.
- **Tool failures are not protocol errors.** A bad argument or
  an out-of-range index returns a normal `tools/call` result
  whose single text content block carries the error message and
  whose `isError` flag is `true`. Successful calls return the
  handler's JSON value serialised into that same
  text block, with `isError: false`.

## Transport 1 — stdio (`mcp_server` binary)

```sh
cargo build --release --bin mcp_server
./target/release/mcp_server        # speaks JSON-RPC on stdio
```

The binary names come from Cargo's auto-discovery (there are no
`[[bin]]` sections in `Cargo.toml`): `src/bin/mcp_server.rs`
builds as `mcp_server`, and the editor `src/main.rs` builds as
`stykanimate-rs`. The CI artifact `stykanimaters-linux-x86_64`
contains both binaries plus `SHA256SUMS.txt`. CI builds with
`cargo build --release --locked --bins`.

Framing and state:

- One JSON object per line on stdin; one response line per
  request on stdout, flushed immediately. Blank lines are
  skipped. Nothing but protocol messages goes to stdout;
  diagnostics go to stderr. The process serves until stdin EOF.
- The server holds **one `EditorCore` session in memory for the
  life of the process**. It starts holding the built-in demo
  project (`EditorCore::new()`); call `new_project`,
  `load_project` or `import_file` to replace it. Nothing is
  written to disk unless a save/export/render tool is called.
- This session is **private to the binary**: it is not the GUI's
  session. Edits made over stdio never reach an editor window,
  and the editor's unsaved work is invisible to it. To drive the
  project a user is looking at, use live control instead.

## Transport 2 — live control (HTTP, hosted by the editor)

```
POST http://127.0.0.1:32117/mcp
```

The editor binary (`stykanimate-rs`) hosts the endpoint
(`src/live_mcp.rs`, bound to `127.0.0.1` only, fixed port
`32117`). Requests are routed with
`McpServer::handle_on` against the editor's **live core**, so
tools edit the project in the window: the stage, timeline and
panels react, playback tools start real playback with audio, and
edits that the catalogue describes as "one undo step" land in the
editor's undo history.

Enabling it:

- The Settings card's **Live MCP control** toggle starts and
  stops the bridge, shows the endpoint URL, and persists the
  resulting state (`LayoutPrefs.live_mcp`).
- That preference **defaults to on** — including for prefs files
  written before the field existed — so a freshly started editor
  normally hosts the endpoint from launch. If the port is
  already taken (another editor instance), the bridge stays off
  and the toggle shows the real state; free the port and toggle
  it on.

How requests actually flow — read this before writing a client:

- The listener thread never touches the editor. It parses the
  request, queues it, and **blocks on the reply channel**. The
  editor's poll loop drains **up to 8 queued requests per tick**
  on the UI thread and sends each response back. So a reply
  arrives on the app's poll cadence — **poll-tick latency plus
  the tool's own runtime, never sub-frame immediacy** — and a
  long tool (a render, an export) holds its HTTP connection open
  until it finishes. Do not impose a short client timeout.
- Connections are accepted and served **sequentially**, one
  request per connection (`Connection: close`); there is no
  session id, no SSE stream, no server-initiated traffic.
- Identity probes: `GET /` and `GET /health` answer a small JSON
  document — `{"server": "stykanimate-rs", "version": ...,
  "transport": "live-http", "live": true, "endpoint":
  "http://127.0.0.1:32117/mcp", "auth": "none"}`. The stdio
  server has no HTTP listener, so a health answer also proves
  you are talking to the live editor.
- Framing limits: request head at most 16 KiB, body at most
  4 MiB (`413` with a JSON-RPC `-32600` body), the whole request
  must arrive within 30 s of accept. `Transfer-Encoding` is
  refused with `501` (send `Content-Length`); a malformed or
  conflicting `Content-Length` is `400`; a `POST` to any other
  path is `404` (as is a `GET` to any path but `/` and
  `/health`); non-`POST` methods other than the `GET` probes
  are `405`. A queued request the editor never answers is given
  up on after 600 s with `504` — note the edit may still land if
  the editor later drains it; only the reply is lost.

> **Security — no authentication.** Live control has **NO
> authentication in v1**. While it is on, **any program running
> as your user on this computer can call every MCP tool through
> this endpoint** — edit the project, save and load files,
> export, delete. The bind is loopback-only, so nothing off the
> machine can reach it, but local programs all can. Enable it
> only while you actually want an agent driving the editor, and
> turn it off when you are done.

### Queue, backpressure and timeouts (live transport)

All values below are constants or call sites in
`src/live_mcp.rs` unless another file is named.
as surveyed, the
behaviour described is the behaviour in the tree.

| Behaviour | Truth |
|---|---|
| Queue capacity | At most **8 parsed requests** wait for the UI thread (`MAX_QUEUED_REQUESTS = 8`, a bounded `sync_channel`). The bound matches the per-poll drain batch, so a healthy editor can clear a full queue in one poll. |
| Queue full | The listener refuses the new request at once with **HTTP 503** ("editor request queue is full") rather than blocking behind a queue that may never move (`handle_connection`, `TrySendError::Full` arm). |
| Queue disconnected | If the bridge is being torn down under a connection, the request is refused with **HTTP 500** ("editor not draining") (`TrySendError::Disconnected` arm). |
| Drain batch | The poll loop drains **up to 8 requests per tick**, strictly in receive (arrival) order; the drain never reads the message to prioritise — it cannot reorder by tool (`drain`; module docs, Wave 13 audit verdict 3). |
| Reply timeout | A queued request the editor never answers is given up on after **600 s** (`REPLY_TIMEOUT`): the client gets **HTTP 504** and the request is marked cancelled. |
| Cancelled requests | When the app later reaches a cancelled request in the queue, `drain` **discards it instead of executing it** — a stale edit must not land on whatever project is current by then (`discard_if_cancelled`, `drain`). A request already executing when the timeout fires cannot be recalled (module docs). Note the consequence: after a 504, the edit may still have happened if execution had already begun; only the reply is lost. |
| Bridge switched off mid-wait | The waiting listener notices the stop flag (it polls the reply channel every 25 ms), cancels the queued request so a later restart's drain cannot apply it out of context, and closes the connection without a response (`handle_connection` wait loop; `LiveBridge::stop`). |
| Socket I/O timeout | A single socket read or write may block at most **30 s** (`IO_TIMEOUT`). |
| Read deadline | The whole request (head + body) must arrive within **30 s** of accept (`READ_DEADLINE`); expiry is a clean close with no response — this is what closes a slow-dribble (slow-loris) connection that a per-read timeout alone would miss. |
| Write deadline | The whole response must be written within **60 s** in total (`WRITE_DEADLINE`, Wave 14 D8): the socket's write timeout is reset to the remaining budget before every chunk, so a slow reader cannot stretch the write indefinitely; a write that misses the deadline is abandoned (the peer is gone or wedged). |
| Head cap | Request head at most **16 KiB** (`MAX_HEAD`); larger is HTTP 400 ("headers too large"). |
| Body cap | Request body at most **4 MiB** (`MAX_BODY`); larger is **HTTP 413** whose body is a JSON-RPC error object (code `-32600`, id `null`) — see the error table. Responses are not capped. |
| Listener resilience | One connection can never kill the listener thread: connection handling is wrapped in `catch_unwind`, and a panicked serve drops that connection and accepts the next (`listener_loop`). |

The stdio transport's corresponding bound is its line cap:
32 MiB per line (`MAX_LINE_BYTES`,
`src/bin/mcp_server.rs`), answered `-32600` when exceeded;
there is no queue and no reply timeout — requests are
served one at a time, in arrival order, by the single
serving loop (`run`, `src/bin/mcp_server.rs`).


### Batch and notification semantics (both transports)

the
behaviour below is what the source does as surveyed.

- **Batches are not supported.** The shared router expects
  one JSON object; a JSON array is not an object, so a batch
  gets a single `-32600` error ("Invalid Request: expected
  a JSON-RPC object", id `null`) — over HTTP exactly as over
  stdio, never a hang or a dropped connection (`dispatch`,
  `src/mcp.rs:185-190`; module docs, `src/live_mcp.rs`).
- **A notification is a JSON object with no `id` member.**
  Notifications are never answered, whatever the method —
  including unknown ones (`dispatch`: the `id` check runs
  before method routing, `src/mcp.rs:192-195`).
- **An explicit `"id": null` is a request, not a
  notification**: it earns an answer carrying a `null` id
  (`is_notification`, `src/live_mcp.rs`, whose doc states
  this exclusion explicitly; same rule in `dispatch`).
- On **stdio**, a notification simply produces no response
  line (`run`, `src/bin/mcp_server.rs` — only `Some`
  responses are written).
- On **live HTTP**, a notification is answered **202 with
  an empty body by the listener itself, immediately,
  without queueing** (`handle_connection`, the
  `is_notification` fast path). This is behaviour-identical
  to queueing it — dispatch guarantees a notification
  executes nothing and earns no response — minus a pointless
  wait on the UI thread (and a pointless 504 risk if the
  app is wedged). A **non-object** message is *not*
  fast-pathed: it must still be queued so dispatch can
  answer it `-32600` (same source).
- `notifications/initialized` is accepted this way (silence
  / 202) and is **not required** before other calls
  (no initialisation gate in `dispatch`, `src/mcp.rs`).

### Error codes

the parse branch of `run`, `src/bin/mcp_server.rs` |
| `-32600` | Invalid Request — the message is not a JSON object (this includes batch arrays), or the `method` member is missing. Id is `null` for a non-object, the request's id otherwise. | `dispatch`, `src/mcp.rs:185-200` |
| `-32600` | Invalid Request — a stdio line over the 32 MiB cap ("line exceeds the …-byte limit", id `null`), or a live request body over the 4 MiB cap (delivered as HTTP 413, id `null`). | `run`, `src/bin/mcp_server.rs`; `body_too_large_body`, `src/live_mcp.rs` |
| `-32601` | Method not found — any method other than `initialize`, `ping`, `tools/list`, `tools/call` ("Method not found: {method}"). | `dispatch`, `src/mcp.rs:208-211` |
| `-32602` | Invalid params — a `tools/call` with no `params` object, or with no tool `name`. | `tools_call_result`, `src/mcp.rs:244-248` |
| `-32602` | Unknown tool — a `tools/call` naming a tool that is not registered. The message is exactly `unknown tool: {name}`. This is the one call failure that never reaches a handler, so it is a protocol error, not a tool result (standing contract). | `tools_call_result`, `src/mcp.rs:253-263` |
| `-32603` | Internal error — **stdio shell only**: a panic escaping dispatch is caught by the binary and answered `-32603` naming the request id ("Internal error: tool dispatch panicked: …"). | `panic_response` + `catch_unwind` in `run`, `src/bin/mcp_server.rs` |

#### Tool failures are not protocol errors

A bad argument, an out-of-range index or any other handler
refusal returns a normal `tools/call` **result** whose
single text content block carries the error message and
whose `isError` flag is `true`; a successful call returns
the handler's JSON value serialised into the same text
block with `isError: false` (`tool_result`,
`src/mcp.rs`). A **handler panic** is caught inside the
shared router itself and converted to exactly such an
`isError` result naming the tool ("tool panicked: {name}")
— on the live transport this is what stops a panicking
handler unwinding into the GUI's poll loop and taking the
editor down (`tools_call_result`, `src/mcp.rs:265-277`).
The stdio `-32603` net above is a second layer behind it,
for a panic outside a handler.

#### HTTP status codes (live transport only)

All routed JSON-RPC responses — including protocol errors —
are HTTP **200**; the error lives inside the JSON-RPC body
(`write_json`, `src/live_mcp.rs`). The other statuses are
transport-level, from `handle_connection`:

| Status | When |
|---|---|
| 200 | A routed request's JSON-RPC response; also a parse error body (`-32700`); also `GET /` and `GET /health`. |
| 202 | A notification (`POST /mcp` with no `id`) — empty body, answered without queueing. |
| 400 | Request head over 16 KiB ("headers too large"), or a `Content-Length` that is present but not a plain number, or duplicated with differing values ("bad content-length"). Identical duplicate `Content-Length` headers are tolerated. (`content_length_status`) |
| 404 | `GET` to any path but `/` and `/health`; `POST` to any path but `/mcp`. A query string never changes routing (`path_of`). |
| 405 | Any method other than `POST` and the `GET` probes. |
| 413 | Body over 4 MiB — body is the JSON-RPC `-32600` object described above. |
| 415 | A `Content-Type` header is present but is not `application/json` (parameters such as `; charset=utf-8` are fine). An **absent** header is tolerated. (`content_type_acceptable`) |
| 500 | The queue's receiver is gone (bridge being torn down), or a queued request was dropped without a reply. |
| 501 | Any `Transfer-Encoding` header — chunked bodies are not decoded; send `Content-Length`. |
| 503 | The request queue is full (8 waiting) — the editor has fallen a whole drain batch behind. |
| 504 | No reply from the editor within 600 s; the request is marked cancelled (see the queue table). |

the rows are sourced
from `handle_connection` as surveyed, and its early-return
contract is "every early return is a client or transport
failure that has already been answered (or needs no
answer); this function never panics on bad input".

## Connect a client

### stdio

```json
{
  "mcpServers": {
    "stykanimate": {
      "command": "/path/to/mcp_server",
      "args": []
    }
  }
}
```

Point `command` at the binary from the CI artifact or at
`target/release/mcp_server` from a source build.

### live control

For clients that configure an HTTP MCP endpoint by URL (some
clients also want `"type": "http"` next to the `url`):

```json
{
  "mcpServers": {
    "stykanimate-live": {
      "url": "http://127.0.0.1:32117/mcp"
    }
  }
}
```

The editor must be running with Live MCP control on (see above).
Because the transport is one POST per JSON-RPC message with the
response as the body, `curl` is the ground truth for debugging:

```sh
curl -s http://127.0.0.1:32117/health
curl -s -X POST http://127.0.0.1:32117/mcp \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'
```

## Catalogue

Argument rules that apply across the catalogue: indices
(`figure`, `frame`, `node`, `index`, `channel`, …) are 0-based;
colours are integers `0xRRGGBB`; angles are degrees unless a tool
says radians (`set_bend`); a tool that fails returns `isError:
true` with the reason as text and leaves the session usable.

### Built-ins — session, timeline and parity ops (44, `src/mcp.rs`)

| Tool | Purpose | Required | Optional |
|---|---|---|---|
| `server_info` | Identify this MCP server: name, version and how many tools are registered. | — | — |
| `get_state` | Snapshot of the current project: name, figures, frame count, current frame, fps, stage size, background colour (0xRRGGBB) and weather state (kind, amount, wind). | — | — |
| `set_frame` | Move the playhead to a 0-based frame index of the current project. | `frame` | — |
| `duplicate_frame` | Duplicate the current frame in place (the timeline's Duplicate button): the copy becomes the current frame. One undo step. | — | — |
| `swap_frames` | Swap the CONTENT of two frames (0-based a and b): poses, durations and easings trade places and frame-pinned data (camera keys, particles, strokes, audio/FX starts, graph keys) follows its frame. Indices, playhead and loop range stay put. One undo step. | `a`, `b` | — |
| `copy_frame` | Copy the current frame to the single-frame clipboard (the Edit menu's Copy frame). Not an undoable edit. Errors when the project has no frames. | — | — |
| `get_clipboard_state` | Read what the frame clipboards hold: 'frame' is null when the single-frame clipboard is empty, else the copied frame's figure count and duration; 'range_frames' is the range clipboard's frame count. A paste refuses when the clipboard is empty or the rig changed since the copy. Read-only; no undo step. | — | — |
| `paste_frame` | Paste the single-frame clipboard (see copy_frame) as a new frame after the current one, which becomes current. One undo step. Errors when nothing was copied or the copied frame's figure count no longer matches. | — | — |
| `shift_frames` | Move a block of frames to a new position in the timeline (frame-range ops): 'count' frames starting at 'from' shift by 'delta' slots (negative moves earlier). One undo step. | `from`, `count`, `delta` | — |
| `reverse_range` | Reverse a block of frames in place (frame-range ops): 'count' frames starting at 'from' (clamped to the timeline end) play backwards. One undo step. | `from`, `count` | — |
| `delete_range` | Delete a block of frames (frame-range ops): 'count' frames starting at 'from' (clamped to the timeline end). Refuses to delete every frame — a project always keeps at least one. One undo step. | `from`, `count` | — |
| `copy_range` | Copy a block of frames into the range clipboard (frame-range ops): 'count' frames starting at 'from' (clamped to the timeline end), ready for paste_range. Not an undoable edit. | `from`, `count` | — |
| `paste_range` | Insert the range clipboard's frames (see copy_range) at 0-based index 'at' (clamped to the timeline, so a large 'at' appends), shifting later frames. One undo step. Errors when the range clipboard is empty. | `at` | — |
| `set_loop_range` | Set the inclusive playback loop range [start, end] (0-based; the timeline's Set loop row). Endpoints are ordered and clamped to the timeline; start == end loops a single frame. Errors when the project has no frames. Editor runtime state, like the playhead — not an undoable edit. | `start`, `end` | — |
| `clear_loop_range` | Clear the playback loop range (the timeline's loop Clear), so playback wraps at the timeline end again. | — | — |
| `set_frame_easing` | Set one frame's easing override (the easing panel's frame picker): "linear", "smooth", "ease-in" or "ease-out", or null for the project default. Applies to the current frame unless 'frame' names another. One undo step. Errors when the project has no frames. | `easing` | `frame` |
| `delete_figure` | Delete a figure (0-based 'figure'; the figures panel's Delete): its pose rows go with it and every figure-indexed reference (particles, FX clips, graph channels, camera follow) is remapped. Refuses to delete the last figure. One undo step. | `figure` | — |
| `move_figure` | Move a figure in the draw order (the figures/layers panels' up/down controls): 'direction' steps the figure later (positive, drawn on top) or earlier (negative), clamped at the ends; poses, bends and every figure-indexed reference are remapped. 'figure' defaults to the selected figure. One undo step. | `direction` | `figure` |
| `delete_camera_key` | Delete the camera key at a 0-based frame (the camera panel's per-key Delete). One undo step. Errors when the frame holds no camera key. | `frame` | — |
| `get_camera` | Read the camera (the camera panel, as data): the interpolated camera {x, y, zoom} and raw angle at 'frame' (default: the current frame), the follow target (figure index or null), and every camera key sorted by frame — each verbatim with a 'stale' flag when its frame is at/past the project's frame count. Angles are raw as stored; compare modulo 360. Read-only. | — | `frame` |
| `set_camera_key_here` | Set a camera key at the current frame with the camera exactly as it stands (the camera panel's Set key). One undo step. | — | — |
| `hold_camera` | Hold the camera: stamp the camera in effect at the current frame onto every frame of a span (the camera panel's Hold). The span is 'from'..'to' (0-based, inclusive) when both are given, else the loop range when set, else the whole timeline. One undo step. Errors when the project has no frames. | — | `from`, `to` |
| `frame_all_figures` | Frame all visible figures with the camera at the current frame (the View menu's Frame all): upserts a camera key centred on the figures' bounds with a fitting zoom. One undo step. | — | — |
| `centre_on_selection` | Centre the camera on the selected figure's root at the current frame (the View menu's Centre on selection): upserts a camera key with the current zoom and angle, only the centre moved. One undo step. | — | — |
| `save_named_project` | Save the project under its own name in ./projects/ (the GUI's Save: the file name is the slugified project name, and the saved-projects list is refreshed). Distinct from save_project, which writes the JSON to an explicit path. Errors when the write fails. | — | — |
| `audio_split` | Split an audio clip at a frame (the audio panel's Split): clip 'index' (0-based; the legacy name 'clip' is also accepted) is cut at 0-based 'at_frame' into two clips. Errors when the clip does not exist or its file cannot be read. | `at_frame` | `index`, `clip` |
| `audio_crossfade` | Crossfade an audio clip into the following one (the audio panel's Crossfade): clip 'index' (0-based; the legacy name 'clip' is also accepted) blends with the following clip. Errors when the clip has no following clip or a file cannot be read. | — | `index`, `clip` |
| `audio_mixdown` | Mix all audio clips down to a single WAV file (out/mixdown.wav; the audio panel's Mixdown button runs the same operation). The project's clips are left in place — the mix is a rendered file, not a new clip. Errors when the project has no audio clips or the mix fails. | — | — |
| `particle_add_event` | Add a particle event at a stage position on a frame (the Particles panel's add): 0-based 'frame', 'x'/'y' in stage coordinates. The event takes the panel's default kind and the selected figure's colour. One undo step. | `frame`, `x`, `y` | — |
| `clear_particles` | Clear ALL particle events and FX clips (the command palette's Clear particles / the panel Clear). One undo step. | — | — |
| `fig3_library_list` | List the display names of every figure in the 3D library (the 3D panel's library), sorted; empty when the library holds nothing readable. | — | — |
| `fig3_load_from_library` | Load a 3D library figure by name (the 3D panel's library Load), replacing the current 3D session or starting one; an active session's camera is kept. Errors when no library figure has that name. | `name` | — |
| `fig3_save_to_library` | Save the current 3D session's figure and pose to the 3D library under the figure's name (the 3D panel's Save to library). Errors when no 3D session is active (see fig3_enter) or the write fails. | — | — |
| `smooth_path_roots` | Smooth a figure's motion path (the frame-ops Smooth path button): each interior frame's root for 'figure' (0-based) is pulled to the midpoint of its neighbours, bending the path without touching angles. Needs at least three frames. One undo step when anything moves. | `figure` | — |
| `set_camera_follow` | Set which figure the camera follows during playback (the camera panel's Follow toggle): 'figure' is a 0-based figure index, or null to turn following off. Stored in the project. One undo step when the follow target actually changes. | `figure` | — |
| `set_tween_playback` | Turn tweened (interpolated) playback on or off (the Tween playback toggle). Stored in the project. One undo step when the value actually changes. | `enabled` | — |
| `get_editor_status` | Read-only snapshot of the editor session (not the project file): playing state, current frame, fractional playhead, loop range (or null), the selected figure and node, undo/redo history depths with can_undo/can_redo, and the status line. EditorCore exposes no saved/dirty flag — the undo depth is the observable edit history. Complements get_state, which reports the project's shape. | — | — |
| `set_playback` | Drive playback, mirroring the transport buttons: 'play' / 'pause' / 'toggle' flip the same playing flag the chrome Play/Pause button flips; 'restart' goes to frame 0 and plays (the Restart button); 'play_range' goes to the loop range's start — frame 0 when no range is set — and plays (the Play range button). Over the live-control path the GUI's poll observes the flag, so playback and its audio start exactly as if the button had been pressed. | `action` | — |
| `get_selection` | Read the current selection: the selected figure's 0-based index and name (null if the index is somehow out of range), and the selected node index within that figure (null when no node is selected). | — | — |
| `set_selection` | Select a figure by 0-based index (the figures panel row press: EditorCore::select_figure, which also clears the node selection). Optionally also select a node of that figure by 0-based index, as a stage click on the figure does; omit 'node' (or pass null) to leave the node selection cleared. An out-of-range figure or node is an error and leaves the selection untouched. | `figure` | `node` |
| `goto_next_keyframe` | Move the playhead to a figure's next keyframe (EditorCore::goto_next_keyframe, the dopesheet's navigation): a figure's keyframes are the frames where its pose changes (stats::figure_keyframes). 'figure' is a 0-based figure index. An error when the figure has no later keyframe — the core's own refusal, carried on its status line. | `figure` | — |
| `goto_prev_keyframe` | Move the playhead to a figure's previous keyframe (EditorCore::goto_prev_keyframe, the dopesheet's navigation): a figure's keyframes are the frames where its pose changes (stats::figure_keyframes). 'figure' is a 0-based figure index. An error when the figure has no earlier keyframe — the core's own refusal, carried on its status line. | `figure` | — |
| `delete_keyframe` | Delete a figure's keyframe at a frame (EditorCore::delete_keyframe, the dopesheet's delete): the frame takes the previous frame's held pose, so the figure no longer changes there. 'figure' and 'frame' are 0-based indices. One undo step. The core's refusals — frame 0's base pose, a frame that is not a keyframe for the figure, a frame where the figure appears — are errors carrying its status line. | `figure`, `frame` | — |
| `goto_next_key` | Move the playhead to the next key strictly after the current frame (the transport's Next key button, pb-next-key): the keys are the sorted union of the camera keys and every graph channel's keys — not any one figure's pose keys (for those, goto_next_keyframe). Like the button, the jump only moves the playhead — playback state is untouched. An error when no key follows (the button renders inert in that state). | — | — |
| `goto_prev_key` | Move the playhead to the previous key strictly before the current frame (the transport's Prev key button, pb-prev-key): the keys are the sorted union of the camera keys and every graph channel's keys — not any one figure's pose keys (for those, goto_prev_keyframe). Like the button, the jump only moves the playhead — playback state is untouched. An error when no key precedes (the button renders inert in that state). | — | — |

### Editing registry (313, `src/mcp_tools.rs`)

| Tool | Purpose | Required | Optional |
|---|---|---|---|
| `new_project` | Replace the session project with a fresh one (a single default humanoid figure, one frame). | — | `name`, `fps`, `width`, `height` |
| `add_figure` | Add a figure of the given kind to the project; returns its index. | `kind` | `name`, `color` |
| `list_figures` | List all figures with index, name, colour, layer state and node count. | — | — |
| `figure_set_color` | Set a figure's colour (0xRRGGBB) — the colour its segments draw in unless a segment carries its own colour override. | `figure`, `color` | — |
| `figure_duplicate` | Duplicate a figure: the copy (same definition, colour and per-frame poses) is inserted right after the original and becomes the selected figure. Returns the new figure index. | `figure` | — |
| `get_project_stats` | Project summary counts: figures, frames, fps, duration in seconds, FX/audio clips, sprites, texts, weather kind (or null), movieclips and graph channels. | — | — |
| `add_frame` | Append a copy of the current frame after it and move to it. On a zero-frame project it recovers by creating the first frame instead of failing. | — | — |
| `insert_inbetween` | Insert a tween frame halfway between the current frame and the next. | — | — |
| `delete_frame` | Delete the frame at the given index. | `frame` | — |
| `set_frame_duration` | Set a frame's hold-time multiplier (1.0 = one beat at the project fps). | `frame`, `duration` | — |
| `adjust_frame_duration` | Adjust a frame's duration multiplier by 'factor' (the frame-ops panel's Dur −/+/×0.5/×2 buttons): the stored duration is multiplied and clamped to 0.25–8.0 by the op; the reply reports the duration read back, so a step past a bound truthfully reports the bound. The relative form of `set_frame_duration`. Applies to the current frame unless 'frame' names another (which moves the playhead there first). Not an undo step (timing-tweak panel parity); a real change counts for dirty/autosave. | `factor` | `frame` |
| `set_frame_durations` | Set hold-time multipliers for frames 0..n in one call (same positive-value rule as set_frame_duration; extra entries beyond the frame count are ignored). | `durations` | — |
| `set_project_easing` | Set the project's default easing for tweened playback: "linear", "smooth", "ease-in" or "ease-out" (the easing panel's preset buttons). A project-settings tweak — not an undo step. | `easing` | — |
| `get_pose` | Read a figure's pose in a frame: {root: [x, y], angles: [degrees per node]}. Angles are the STORED values: parent-relative (each relative to its parent segment) and unnormalised (725 means 725, not 5). For the solved world direction of one segment, use get_node. | `figure`, `frame` | — |
| `set_pose` | Write a figure's pose in a frame. 'angles' is either a full array (one angle per node, degrees) or an object mapping node index to degrees; 'root' is [x, y]. Angles are stored parent-relative and verbatim (no wrapping — 725 stays 725). One undo step. | `figure`, `frame` | `root`, `angles` |
| `mirror_pose` | Mirror the selected figure's pose in the current frame (left-right flip around its root). One undo step. | — | — |
| `mirror_figure` | Mirror a whole figure left-right in EVERY frame (the all-frames counterpart of mirror_pose): every frame's pose is flipped around the figure's root. One undo step. | `figure` | — |
| `set_node` | Edit a figure definition's segment: length (clamped to 4..=220), thickness (clamped to 1..=18), colour override, or shape ("line" \| "circle" \| "polygon" \| side count 3-12). Pass "color": null to CLEAR the colour override (an absent 'color' leaves it unchanged). One undo step. | `figure`, `node` | `length`, `thickness`, `color`, `shape` |
| `set_node_cap` | Set a segment's end cap directly: "round", "square" or "point" (the style panel's cap control). One undo step. | `figure`, `node`, `cap` | — |
| `set_node_color2` | Set a segment's gradient end colour (0xRRGGBB; the segment blends its own colour -> color2). Omit 'color2' (or pass null) to clear the gradient back to a solid colour. One undo step. | `figure`, `node` | `color2` |
| `set_node_opacity` | Set a segment's opacity (the style panel's opacity stepper). Opacity below 0 is refused; any other value is clamped to 0.05-1.0 and read back as stored. One undo step. | `figure`, `node`, `opacity` | — |
| `add_node` | Append a segment under the given parent node of a figure; returns the new node index. | `figure`, `parent`, `length` | `thickness` |
| `delete_node` | Delete a leaf segment of a figure (the root and segments with children are refused). | `figure`, `node` | — |
| `set_camera` | Upsert a camera key at a frame: centre (x, y), zoom factor (1.0 = neutral, must be positive), optional rotation angle in degrees — stored wrapped to -180..180, the panel's convention. One undo step. | `frame`, `x`, `y`, `zoom` | `angle` |
| `clear_camera_key` | Remove the camera key at the current frame WITHOUT an undo step (the camera panel's "Clear key (no undo)" button): counts for dirty/autosave only. For the undoable delete at any frame, use delete_camera_key. Errors when the current frame holds no camera key. | — | — |
| `camera_zoom_by` | Multiply the camera zoom at the current frame by a factor (the camera panel's zoom buttons): upserts a key there, zoom clamped to 0.3..=4.0, angle preserved. One undo step. | `factor` | — |
| `rotate_camera` | Rotate the camera at the current frame by a degree delta (the camera panel's rotate buttons): upserts a key with the pan/zoom in effect and the angle wrapped to -180..180. One undo step. | `degrees` | — |
| `add_fx_clip` | Add a timed particle FX clip anchored to a figure's node. start/duration are in frames; a start at or past the project's end is refused (park clips with fx_move instead, which deliberately allows past-end starts). 'color2' optionally sets a gradient end colour (particles blend colour -> color2). | `figure`, `node`, `kind`, `start`, `duration` | `color`, `color2`, `length` |
| `fx_list` | List all timed FX clips: index, figure, node, kind, start, duration, end frame, hidden, color, color2, seed, angle and length. | — | — |
| `fx_move` | Move an FX clip to a new start frame (clamped at 0). One undo step. | `index`, `start` | — |
| `fx_resize` | Set an FX clip's duration in frames (the stored duration is floored at 0.1 by the op). One undo step. | `index`, `duration` | — |
| `fx_duplicate` | Duplicate an FX clip: the copy is inserted right after it and starts where the original ends. One undo step. | `index` | — |
| `fx_delete` | Delete an FX clip by index. One undo step. | `index` | — |
| `fx_split_clip` | Split an FX clip into two at 'at_frame' (a frame coordinate inside the clip's span): the first part keeps the clip's start and the second starts at the split point, both on the same figure and node. Errors when the split point is not inside the clip. One undo step. | `index`, `at_frame` | — |
| `fx_set_color` | Set an FX clip's particle colour (0xRRGGBB). One undo step. | `index`, `color` | — |
| `fx_set_kind` | Set an FX clip's particle kind by name: burst, sparkle, smoke, beam, aura, trail, confetti, shock_ring, ember, fountain, orbit, lightning, bubbles, leaves, meteor, petals, splash, spiral, fireworks, aurora, comet, galaxy, prism, vortex, glitter, wave or shard (the names add_fx_clip accepts). One undo step. | `index`, `kind` | — |
| `fx_set_color2` | Set an FX clip's gradient end colour (0xRRGGBB; particles blend colour -> color2). Omit 'color2' (or pass null) to clear the gradient back to a solid colour. One undo step. | `index` | `color2` |
| `fx_set_anchor` | Re-anchor an FX clip to a figure's node — the joint its particles are emitted from. One undo step. | `index`, `figure`, `node` | — |
| `fx_preset_save` | Save the FX clip at 'index' as a reusable preset under 'name' (kind, duration, colour and motion are stored; the figure/node anchor is not). | `index`, `name` | — |
| `fx_preset_add` | Add a saved FX preset to the project at the given start frame: the preset's kind/duration/colour are kept, its start is overridden by 'frame', and the clip is anchored to the selected figure's first node. Errors if the preset does not exist. | `name`, `frame` | — |
| `particle_duplicate_event` | Duplicate a particle event (one of the project's timed particle bursts, listed in the stats' particle count): the copy is INSERTED immediately after the original (at index + 1, not appended to the end), events after it shift up one index, and the copy's index is returned. One undo step. | `index` | — |
| `particle_set_kind` | Set a particle event's kind by name: burst, sparkle or smoke (case-insensitive; the shapes the Particles panel cycles). One undo step. | `index`, `kind` | — |
| `particle_set_color` | Set a particle event's colour (0xRRGGBB) — the tint every particle of the event is drawn in. One undo step. | `index`, `color` | — |
| `set_weather` | Set continuous weather over the whole timeline, or turn it off. | `kind` | `amount`, `wind` |
| `set_weather_kind` | Set the scene weather kind (the Scene panel's kind picker): switching kinds keeps the tuned amount/wind; enabling from off applies the panel's per-kind preset. A project-settings tweak — not an undo step; a real change marks the project dirty. | `kind` | — |
| `cycle_weather` | Cycle the scene weather (the Scene group's Cycle weather button): off → rain → snow → fireflies → off, each kind's preset applied wholesale; fog and storm are not in the cycle (cycling from either turns weather off). Not an undo step; a real change marks the project dirty. | — | — |
| `set_weather_amount` | Set the weather's particle amount absolutely (the Scene panel's Amount stepper/slider), clamped to 0..=200. Errors when the weather is off. Not an undo step; a real change marks the project dirty. | `amount` | — |
| `set_weather_wind` | Set the weather's wind (horizontal drift) absolutely (the Scene panel's Wind stepper/slider), clamped to -200..=200. Errors when the weather is off. Not an undo step; a real change marks the project dirty. | `wind` | — |
| `get_onion_skin` | Read the onion-skin master switch ('enabled'). The per-side depths are GUI shell state, not core state, and are not reported; 'legacy_prev_frames' is the core's legacy count the stage no longer reads. Read-only. | — | — |
| `set_onion_skin` | Turn the onion-skin master switch on or off (the Onion panel's toggle). Session view state: no undo step, no dirty mark — exactly like the panel toggle. | `enabled` | — |
| `get_editor_settings` | Read the editor-settings surface in one call: weather, background, project easing, tween playback, camera follow, loop range, onion switch, fps, stage size and head-ring scale. Read-only. | — | — |
| `add_audio_clip` | Place an audio file on the timeline starting at a frame. MCP refuses a file that does not exist or is not a WAV — where the GUI panel would add the clip and badge it broken instead. One undo step. | `path`, `start_frame` | — |
| `audio_fade` | Apply a fade-in/fade-out to the LAST audio clip with the editor's fixed fade lengths (0.25 s in, 0.25 s out — not parameterised; use audio_clip_fade for per-clip lengths): a processed copy of its file is written and the clip repointed. One undo step. For a specific clip, prefer the indexed audio_clip_* family. | — | — |
| `audio_reverse` | Reverse the LAST audio clip: a processed copy of its file is written and the clip repointed (the editor's Reverse op). One undo step. For a specific clip, prefer the indexed audio_clip_* family. | — | — |
| `audio_set_gain` | Apply a gain to the LAST audio clip (the editor's gain op; 1.0 is unity, 0.5 halves the level; the stored gain is clamped and the reply reads it back). One undo step. For a specific clip, prefer the indexed audio_clip_* family. | `gain` | — |
| `audio_move_clip` | Move the LAST audio clip so it starts at 'start_frame' on the timeline. | `start_frame` | — |
| `audio_delete_clip` | Delete the audio clip at 'index' from the timeline (the clip's file on disk is not touched). | `index` | — |
| `audio_echo` | Apply a feedback echo (0.18s delay, 35% feedback) to the audio clip at 'index': a processed copy of its file is written and the clip repointed (the editor's Echo op). One undo step. | `index` | — |
| `audio_lowpass` | Apply a 1200 Hz low-pass filter to the audio clip at 'index': a processed copy of its file is written and the clip repointed (the editor's Lowpass op). One undo step. | `index` | — |
| `audio_trim_silence` | Trim leading and trailing silence from the audio clip at 'index': a processed copy of its file is written and the clip repointed (the editor's Trim silence op). One undo step when there is silence to trim. | `index` | — |
| `audio_pitch` | Pitch-shift the audio clip at 'index' up 2 semitones: a processed copy of its file is written and the clip repointed (the editor's Pitch op). One undo step. | `index` | — |
| `audio_compress` | Compress the audio clip at 'index' (threshold 0.5, ratio 4.0): a processed copy of its file is written and the clip repointed (the editor's Compress op). One undo step. | `index` | — |
| `audio_split_silences` | Split the audio clip at 'index' wherever silence of 0.25s or more falls: each part is written as its own file and becomes its own clip, and the result reports how many clips were added. One undo step when a split happens. | `index` | — |
| `audio_set_muted` | Mute or unmute the audio clip at 'index': a muted clip keeps its place on the timeline but is left out of playback and exports. One undo step. | `index`, `muted` | — |
| `audio_normalise_all` | Normalise every audio clip on the timeline to the same peak level: each clip's gain is set so its peak reaches 'target_peak' (in the range (0, 1], default 0.9). Reports how many clips were adjusted. | — | `target_peak` |
| `graph_key` | Add a graph-editor channel key: target is "root_x", "root_y", "angle:N", "length:N" or {"angle": node}; frame and value are numbers (degrees for angles). | `figure`, `target`, `frame`, `value` | — |
| `smooth_graph_tangents` | Smooth the tangents of every graph channel key (auto-tangent pass over all channels). One undo step. | — | — |
| `bake_graph` | Bake the graph channels into per-frame keys between frames 'from' and 'to' (inclusive), sampling every channel at each frame in the range. One undo step. | `from`, `to` | — |
| `graph_tangent_preset` | Apply an easing preset to the tangents of every key in the SELECTED figure's graph channels (other figures' channels are untouched — graph_tangent_preset_channel addresses any figure's channel by index): "smooth", "ease-in", "ease-out", "ease-inout" or "linear". One undo step. | `preset` | — |
| `mirror_channel_time` | Mirror one graph channel's keys in time around 'centre' (a frame coordinate; default: the timeline's middle frame), so that channel's animation plays backwards. Target addressing matches graph_key ("root_x", "root_y", "angle:N", "length:N" or {"angle": node}). Errors when the channel does not exist. One undo step. | `figure`, `target` | `centre` |
| `scale_channel_values` | Scale one graph channel's key values by 'factor' around 'pivot' (default 0): value' = pivot + (value - pivot) * factor. Target addressing matches graph_key. Errors when the channel does not exist. One undo step. | `figure`, `target`, `factor` | `pivot` |
| `graph_copy_keys` | Copy all keys of one graph channel, returned as key objects ({frame, value, in_tan, out_tan, interp}) to feed to graph_paste_keys. Target addressing matches graph_key. Errors when the channel does not exist. | `figure`, `target` | — |
| `graph_paste_keys` | Paste keys into a graph channel: the earliest pasted key lands at 'at_frame' and existing keys within half a frame of a pasted key are replaced. 'keys' is the array graph_copy_keys returns (per key: frame and value required, in_tan/out_tan default 0, interp "hermite"/"linear"/"step" default "hermite"). Target addressing matches graph_key; the channel must already exist. One undo step. | `figure`, `target`, `keys`, `at_frame` | — |
| `duplicate_graph_channel` | Duplicate a graph channel — addressed by its index in the project's channel list — onto another figure: the clone keeps every key and is appended to the channel list. Errors when the target figure already has a channel for the same target (the clone would be shadowed), is the channel's own figure, or lacks the node an angle/length target names. One undo step. | `channel`, `to_figure` | — |
| `delete_graph_channel` | Delete a graph channel by its index in the project's channel list. One undo step. | `channel` | — |
| `set_key_interp` | Set the interpolation mode of one key on a graph channel — addressed by the channel's index in the project's channel list and the key's index within it: "hermite" (smooth tangents), "linear" or "step". Frame, value and tangents are untouched. One undo step. | `channel`, `key`, `interp` | — |
| `graph_set_interp_at_playhead` | Set the interpolation mode ("hermite"/"linear"/"step") of the graph key nearest the current frame on the selected figure's angle channel for the selected joint (EditorCore::set_graph_key_interp — the graph panel interp control's fallback). set_key_interp addresses any key by index. One undo step when it sets. | `interp` | — |
| `set_project_fps` | Set the project's frame rate in frames per second (clamped to 1-60; the Scene panel's frame-rate stepper). One undo step when the rate changes. | `fps` | — |
| `set_stage_size` | Set the stage size in pixels (each axis clamped to 64-4096; the Scene panel's stage-size steppers). One undo step when the size changes. | `width`, `height` | — |
| `set_head_ring_scale` | Set the head-ring size as a multiple of the base ring radius (clamped to 0.5-2; applies to every figure's head ring in the stage and all exports). One undo step when the scale changes. | `scale` | — |
| `layers` | Apply layer state to a figure: locked, hidden, opacity (0.05-1.0), and solo (true solos this figure in the editor, false clears its solo). | `figure` | `locked`, `hidden`, `opacity`, `solo` |
| `figure_step_opacity` | Step a figure's opacity by 'delta' (the figures/layers panels' −/+ buttons): the target is rounded to whole percent and clamped to 0.05–1.0 by the op; the reply reports the stored opacity read back, so a step past a bound truthfully reports the bound. The relative form of the `layers` tool's opacity. Not an undo step (layers-family panel parity); a real change counts for dirty/autosave. | `delta` | `figure` |
| `figure_toggle_hidden` | Toggle a figure's hidden flag (the figures panel's hide button; the palette's Toggle figure hidden) — the relative form of the `layers` tool's 'hidden'. Not an undo step (layers-family panel parity); the flip counts for dirty/autosave. | — | `figure` |
| `figure_toggle_lock` | Toggle a figure's locked flag (the figures panel's lock button; the palette's Lock selected figure) — the relative form of the `layers` tool's 'locked'. Not an undo step (layers-family panel parity); the flip counts for dirty/autosave. | — | `figure` |
| `figure_toggle_solo` | Toggle a figure's solo state (the figures panel's solo button; the palette's Solo selected figure) — the relative form of the `layers` tool's 'solo'. Session state only: not an undo step and not a project edit. | — | `figure` |
| `nudge_path_point` | Nudge one figure's motion-path point: move the root of its pose at 'frame' by ('dx', 'dy') stage units — the stage's path-point drag as data; `figure_centre`/`figure_ground` compose this same primitive for their specific shifts. 'figure' defaults to the selected figure, 'frame' to the current frame. One undo step. | `dx`, `dy` | `figure`, `frame` |
| `undo` | Undo the last editor operation. | — | — |
| `redo` | Redo the last undone editor operation. | — | — |
| `draw_stroke` | Append a freehand stroke (polyline) to a frame's drawing layer. points is an array of [x, y] stage coordinates; color is 0xRRGGBB (default light grey), width in stage units (default 4, the editor's pen), scatter jitters the stroke's dabs by that radius in stage units (default 0 = clean line), opacity is the stroke's opacity 0-1 (default 1). One undo step. | `frame`, `points` | `color`, `width`, `scatter`, `opacity` |
| `draw_shape` | Append a geometric shape as a stroke: kind is "line" (2 points), "rect" (closed rectangle, 5 points) or "ellipse" (25 points), from corner (x1, y1) to corner (x2, y2) in stage coordinates. 'frame' defaults to the current frame. color is 0xRRGGBB (default light grey), width defaults to the active brush width; optional 'fill' (0xRRGGBB), 'opacity' (0-1) and 'scatter' (>= 0) mirror the stamp panel's state. One undo step. | `kind`, `x1`, `y1`, `x2`, `y2` | `frame`, `color`, `width`, `fill`, `opacity`, `scatter` |
| `draw_polygon` | Append a closed regular polygon as a stroke: centred at (cx, cy) with the given radius, 'sides' vertices (default 5, at least 3) plus the closing point, first vertex straight up unless 'rotation' (degrees) turns it. 'frame' defaults to the current frame. color is 0xRRGGBB (default light grey), width defaults to the active brush width; optional 'fill', 'opacity' and 'scatter' as in draw_shape. One undo step. | `cx`, `cy`, `radius` | `sides`, `frame`, `rotation`, `color`, `width`, `fill`, `opacity`, `scatter` |
| `draw_star` | Append a star outline as a stroke: centred at (cx, cy), outer radius 'radius', inner radius 26/60 of it (the stamp panel's ratio), 'points' spikes (default 5), first spike straight up unless 'rotation' (degrees) turns it. 'frame' defaults to the current frame. color is 0xRRGGBB (default light grey), width defaults to the active brush width; optional 'fill', 'opacity' and 'scatter' as in draw_shape. One undo step. | `cx`, `cy`, `radius` | `points`, `frame`, `rotation`, `color`, `width`, `fill`, `opacity`, `scatter` |
| `smooth_strokes` | Chaikin-smooth every stroke on a frame (one subdivision pass). Returns how many strokes changed. | `frame` | — |
| `smooth_stroke` | Chaikin-smooth ONE stroke by its project-wide index (see list_strokes) — the single-stroke form of smooth_strokes: 'subdiv' corner-cutting passes (default 1, at most 4), endpoints pinned, per-point widths resampled onto the new points. Returns smoothed: false — and takes no undo step — when the stroke has fewer than 2 points and cannot change. One undo step when it smooths. | `index` | `subdiv` |
| `erase_strokes` | Erase stroke points within radius of (x, y) on a frame, splitting strokes at the cut (draw_ops::erase semantics). Returns how many of the frame's strokes changed. | `frame`, `x`, `y`, `radius` | — |
| `simplify_strokes` | Simplify every stroke on a frame with Douglas-Peucker: interior points within 'epsilon' stage units of the path are dropped, endpoints kept. 'frame' defaults to the current frame. Returns how many strokes lost points. One undo step. | `epsilon` | `frame` |
| `delete_stroke` | Delete one stroke by its index in the project's stroke list (the order draw_stroke appends in). Returns the frame the stroke was on. One undo step. | `index` | — |
| `apply_nib` | Give every stroke on a frame a nib width profile: "round" (uniform), "square" or "calligraphy" (angle-dependent, angle in degrees, default 45). Returns the stroke count. | `frame`, `nib` | `angle` |
| `apply_nib_stroke` | Give ONE stroke, by its project-wide index (see list_strokes), a nib width profile: "round" (uniform), "square" or "calligraphy" (angle-dependent, angle in degrees, default 45) — the single-stroke form of apply_nib, computed from the stroke's own width as the nib size. Replaces any per-point widths the stroke carried. One undo step. | `index`, `nib` | `angle` |
| `fig3_enter` | Enter 3D mode: start a session posing the 3D humanoid with an orbit camera. Keeping an existing session as-is. Pose with fig3_select/fig3_rotate/fig3_orbit, then fig3_bake turns the pose into a normal 2D figure. | — | — |
| `fig3_select` | Select the 3D joint (node index) that fig3_rotate acts on. | `joint` | — |
| `fig3_rotate` | Rotate the selected 3D joint by degrees around an Euler axis: 0 = X, 1 = Y, 2 = Z. | `axis`, `degrees` | — |
| `fig3_orbit` | Orbit the 3D camera by yaw/pitch deltas in degrees (pitch clamps to +-89.9). | `yaw`, `pitch` | — |
| `fig3_bake` | Bake the current 3D pose into a new 2D figure in the project (held on all frames, like the editor's Bake 3D). Returns the new figure index. | — | — |
| `fig3_mark_b` | Mark the current 3D pose as pose B, the target fig3_bake_animation interpolates to (pose the figure, mark B, repose to A, then bake the animation). | — | — |
| `fig3_bake_animation` | Bake an animated 2D figure: interpolate the 3D session's current pose to the marked pose B over 'frames' frames (at least 2), extending the timeline if needed. Requires fig3_mark_b first. | `frames` | — |
| `clip_create` | Create a movieclip from a figure's poses across all frames and set the figure to loop it (the editor's Make clip). Returns the clip index and frame count. | `figure` | — |
| `clip_list` | List the project's movieclip definitions: index, name, frame count, node count, per-node segment media, and which figures loop each clip. | — | — |
| `set_clip_loop` | Set a movieclip's loop mode: with 'mode' ("loop", "ping-pong" or "once") the mode is set directly (one undo step; setting the current mode is a no-op); without it, the mode cycles one step: Loop -> PingPong -> Once -> Loop. The reply names the resulting mode; an out-of-range index is a no-op reported in the status. | `index` | `mode` |
| `set_bend` | Set one segment's bend (radians, clamped to +/-0.7) for a figure on a frame. Positive/negative bend arcs the segment to opposite sides; 0 is straight. Frame.bends rows/cols are extended with zeros as needed. Node 0 (the origin) has no segment to bend and is refused. The reply reports the stored bend in radians and degrees. One undo step. | `figure`, `frame`, `node`, `radians` | — |
| `figure_recolor` | Recolour a whole figure: write the colour (0xRRGGBB) as every segment's own colour override, under one undo step. Gradient end colours and the figure's base colour are left alone. | `figure`, `color` | — |
| `figure_rename` | Rename a figure (the name is trimmed; blank is refused). A plain field write like the inspector's Rename — not an undo step. | `figure`, `name` | — |
| `figure_scale` | Scale a figure's segment lengths by 'factor' (1.1 grows 10%, 0.9 shrinks): every parented segment's length is multiplied and clamped to [4, 220], as one undo step. Refuses when the factor is not a positive number, or when every segment already sits at the clamp in that direction. The reply's status reports the achieved scale. 'figure' defaults to the selected figure (which becomes the selection, as with delete_figure/move_figure). | `factor` | `figure` |
| `figure_centre` | Centre a figure on the stage at the current frame: shift its root so its drawn bounding box (head ring included) centres on the stage. Current frame only. One undo step. 'figure' defaults to the selected figure. | — | `figure` |
| `figure_ground` | Ground a figure at the current frame: shift its root down so its drawn bounding box's bottom rests exactly on the stage's bottom edge. Current frame only. One undo step. 'figure' defaults to the selected figure. | — | `figure` |
| `copy_pose_between` | Copy the current frame's pose of one figure onto another figure (same frame), as one undo step — the pose panel's Copy-pose-to. Refuses when either figure has no pose on the current frame, when source and target are the same figure, or when the two poses have different angle counts (different rig sizes). | `from_figure`, `to_figure` | — |
| `reset_pose` | Reset a figure's pose in a frame to its rest pose (root position kept), as one undo step — the pose panel's Reset pose. 'figure' defaults to the selected figure, 'frame' to the current frame. | — | `figure`, `frame` |
| `pose_apply_all_frames` | Write a pose onto a figure in EVERY frame, as one undo step — the pose panel's Apply-pose-to-all-frames with the pose supplied in the call. Frames whose pose row is missing or whose angle count differs from the supplied angles are skipped and counted in the reply. 'root' is optional: omit it to keep each frame's own root position. | `figure`, `angles` | `root` |
| `pose_library_list` | List the poses saved in the 2D pose library (./library/poses) by display name, sorted — the pose panel's library list. | — | — |
| `pose_library_save` | Save a figure's pose from a frame into the pose library under 'name' (default: the figure's own name — saving the same figure again overwrites its entry, as the pose panel's Save does). A file write, not an undo step. 'figure' defaults to the selected figure, 'frame' to the current frame. | — | `figure`, `frame`, `name` |
| `pose_library_apply` | Apply a saved pose-library pose to a figure in a frame, as one undo step — the pose panel's library Apply. With 'mirror': true the pose is mirrored (angles negated) as it is applied. Refuses when the entry cannot be loaded, the figure has no pose in the frame, or the saved pose's angle count does not match the figure's rig. 'figure' defaults to the selected figure, 'frame' to the current frame. | `name` | `figure`, `frame`, `mirror` |
| `pose_library_delete` | Delete a pose from the pose library (removes its file in ./library/poses). A file operation, not an undo step. | `name` | — |
| `pose_library_rename` | Rename a pose-library entry. Refuses a blank new name and any name another entry already owns (including a different display name that slugs to the same library file). Renaming to the entry's own name succeeds with renamed=false. A file operation, not an undo step. | `name`, `new_name` | — |
| `layers_set_all_visible` | Show or hide ALL figures at once (the figures panel's All / None buttons): visible=true shows every figure, visible=false hides every figure. Clears any active solo, exactly as the panel does, so the flags are the effective visibility. Returns how many figures changed state. | `visible` | — |
| `layers_invert_visibility` | Invert every figure's hidden flag (the figures panel's Invert button). Clears any active solo, exactly as the panel does. Returns how many figures changed state (always the figure count). | — | — |
| `list_strokes` | List strokes in the project (the stroke panel's list, as data): index in the project-wide stroke list, frame, point count, color (0xRRGGBB), width, per-point widths count, fill, spray mark, opacity and scatter. 'frame' filters to one frame; 'points': true includes each stroke's full polyline instead of its point count. | — | `frame`, `points` |
| `get_drawing_stats` | Drawing totals over the whole project (the Draw panel's readout, as data): stroke count, total points, summed polyline length in stage units, spray stroke count, and the current frame's stroke count. | — | — |
| `draw_arrow` | Append an arrow as a stroke: shaft from (x1, y1) to the tip (x2, y2) with a two-wing head of length 'head' (default 22, the editor's stamp). Geometry is paint::arrow_points, the Draw panel's Arrow stamp generator. 'frame' defaults to the current frame. color is 0xRRGGBB (default light grey), width defaults to the active brush width. One undo step. | `x1`, `y1`, `x2`, `y2` | `head`, `frame`, `color`, `width` |
| `set_stroke_style` | Edit one stroke's style by its project-wide index (see list_strokes): color (0xRRGGBB), width (0.5-40, the stroke panel's range; a per-point taper scales proportionally), opacity (0-1), scatter (>= 0), fill (0xRRGGBB) or clear_fill. At least one field is required. Opacity and scatter are refused on spray strokes — a spray stroke's opacity lives on its spray mark and it never renders through the scatter path. One undo step. | `index` | `color`, `width`, `opacity`, `scatter`, `fill`, `clear_fill` |
| `set_stroke_spray` | Edit one stroke's spray mark by its project-wide index (see list_strokes) — the field set_stroke_style cannot reach: 'radius' (> 0), 'opacity' (0-1) and/or 'seed' set the mark's fields, creating the mark on a plain stroke (defaults: radius 12, opacity 0.5, the deterministic begin_draw seed); 'clear_spray': true removes the mark instead, returning the stroke to a plain polyline. At least one field is required. Clearing a markless stroke reports changed: false and takes no undo step. One undo step when the mark changes. | `index` | `radius`, `opacity`, `seed`, `clear_spray` |
| `simplify_stroke` | Simplify ONE stroke by its project-wide index with Douglas-Peucker (the stroke panel's Simplify press): interior points within 'epsilon' stage units (default 2) are dropped, that point's own width survives. Returns simplified: false — and takes no undo step — when the stroke is already simple at that epsilon. One undo step when it simplifies. | `index` | `epsilon` |
| `fill_stroke` | Fill the most recent stroke on the CURRENT frame: its fill is set to its own colour and a scanline fill is appended as a companion stroke (EditorCore::fill_current_stroke). Errors when there is nothing fillable on the frame. One undo step. | — | — |
| `lasso_erase` | Erase strokes on the CURRENT frame that lie mostly (a strict majority of their points) inside the lasso polygon 'points' (at least 3 [x, y] points). Errors when nothing lies mostly inside. One undo step when strokes are removed. | `points` | — |
| `mirror_strokes` | Mirror every stroke on the CURRENT frame across the stage's vertical centre line (the Draw panel's Mirror strokes button). Only points change. Errors on a frame with no strokes. One undo step. | — | — |
| `mirror_all_strokes` | Mirror EVERY stroke in the project, on all frames, across the vertical line x = 'cx' (default: the stage's vertical centre) — the project-wide form of mirror_strokes. Only points change. Errors on a project with no strokes. One undo step. | — | `cx` |
| `clear_frame_strokes` | Remove every stroke on the current frame (the stroke list's Clear button). 'frame' may only name the current frame — the op is current-frame-only and errors otherwise rather than clearing the wrong frame. One undo step when the frame carried strokes. | — | `frame` |
| `clear_strokes_on_frame` | Remove every stroke on the NAMED frame (the frame-addressed form of clear_frame_strokes, whose core op is current-frame-only): 'frame' is required and may name any frame, including the current one. One undo step when the frame carried strokes. | `frame` | — |
| `delete_all_strokes` | Delete every stroke in the project, on all frames (the stroke panel's Delete all). One undo step for the whole deletion. | — | — |
| `recolor_strokes` | Recolour strokes to 'color' (0xRRGGBB, required): scope 'frame' recolours the current frame's strokes (the panel's Frame Recolour), scope 'all' recolours every stroke in the project (the panel's Recolour all — except the panel's target colour is the selected figure's, while here it is explicit). One undo step when strokes match. | `color` | `scope` |
| `list_brushes` | List the brush preset library (the Draw panel's brush chips): index, name, width, nib, nib angle, smoothing, opacity and scatter per preset, plus the active preset's index and name. | — | — |
| `get_brush` | Read the ACTIVE brush preset in full (the Draw panel's preset readouts, as data): index, name, width, nib, nib angle, smoothing, opacity and scatter, plus the active draw tool, the eraser radius the preset sizes (twice its width) and the draw-mode flag. Read-only. | — | — |
| `set_brush` | Select the active brush preset by 'name' (exact, then case-insensitive) or 'index' — the Draw panel's brush chips. Selection only; draws nothing. Subsequent interactive drawing, stamps and the eraser size use the selected preset. On the live transport this changes the editor's active brush. | — | `name`, `index` |
| `cycle_brush` | Cycle to the next brush preset in the library, wrapping (the Draw panel's brush-cycle button). Returns the new active preset. | — | — |
| `step_brush_param` | Step the active brush selection by one preset parameter (the stroke inspector's Preset size / opacity / scatter / smoothing steppers): selects the nearest library preset whose value for 'param' is strictly higher ('direction' 1) or lower (-1); the other parameters follow the chosen preset, because presets are bundles. Reports changed: false, with the panel's status text, when no preset steps that way. Session state only — not an undo step. | `param`, `direction` | — |
| `set_brush_nib` | Select the active brush preset by its nib (the stroke inspector's Preset nib segmented control): the first library preset carrying nib 'round', 'square' or 'calligraphy' becomes active. Session state only — not an undo step. | `nib` | — |
| `set_draw_tool` | Set the active draw tool: 'brush' or 'eraser' (the Draw panel's tool toggle, as an idempotent set). Affects subsequent interactive drawing and eraser sizing; draws nothing itself. | `tool` | — |
| `set_draw_mode` | Set draw mode on or off (the Draw panel's Draw toggle / the palette's draw-mode command, as an idempotent set): in draw mode, dragging on the canvas paints a stroke on the current frame instead of posing. Session state only — not an undo step. | `enabled` | — |
| `get_node_style` | Read a segment's complete style: colour override (null = inherits the figure colour), effective colour, gradient end colour, cap, taper and opacity. | `figure`, `node` | — |
| `cycle_node_taper` | Step a segment's taper one step (1.0 -> 0.6 -> 0.3 -> 0.0 -> 1.0), exactly the style panel's taper Step button. A taper set directly snaps to the next step down. One undo step. | `figure`, `node` | — |
| `set_node_taper` | Set a segment's taper directly (0.0-1.0; 1.0 = uniform width, lower = sharper point). Values outside 0.0-1.0 are refused, not clamped. One undo step. | `figure`, `node`, `taper` | — |
| `set_node_style` | Write a segment's complete style (colour, gradient end, cap, opacity) in exactly one undo step — the write half of the style panel's copy/paste; pair with get_node_style to copy a style between segments. Omit 'color2' (or pass null) to clear the gradient. Taper is not part of this write; use set_node_taper. Opacity below 0 is refused; any other value is clamped to 0.05-1.0 and read back as stored. | `figure`, `node`, `color`, `cap`, `opacity` | `color2` |
| `apply_style_preset` | Apply one of the style panel's six curated presets (Ink, Crimson, Ocean, Forest, Violet, Amber) to a segment: the preset's colour and cap are set, the segment's gradient end and opacity are left untouched. One undo step. | `figure`, `node`, `preset` | — |
| `list_texts` | List every text in the project: segment texts (figure, node, content, size) and free-standing text instances in draw order (index, content, size, colour, transform keys). | — | — |
| `set_segment_text` | Set (or replace) the text on a figure's segment — the text panel's Add text. Optional 'size' (glyph height) is clamped to 4-160; without it an existing size is preserved and a fresh text gets 28. Empty or control-only content is refused. The response reads back what actually stored (the text session drops control characters such as newlines). One undo step. | `figure`, `node`, `content` | `size` |
| `set_segment_text_size` | Set the glyph height of the text already on a segment (clamped to 4-160). A segment carrying no text is a no-op (changed: false). One undo step when the size changes. | `figure`, `node`, `size` | — |
| `clear_segment_text` | Remove the text from a segment. A segment carrying an image (or no media) is left untouched (changed: false). One undo step when a text is cleared. | `figure`, `node` | — |
| `add_text_instance` | Create a free-standing text instance (stage label) with one transform key. Defaults: size 28, colour white, key on the current frame at the stage centre. Note: the editor's text panel cannot create instances (they arrive via import) — this tool completes that side of the surface for MCP. One undo step. | `content` | `size`, `color`, `frame`, `x`, `y` |
| `update_text_instance` | Rewrite a text instance's content (the text panel's Update text): position keys, size and colour are untouched. Empty content is refused — an update never blanks a text; unchanged content is a no-op (changed: false). One undo step when the content changes. | `index`, `content` | — |
| `delete_text_instance` | Delete a free-standing text instance. One undo step. | `index` | — |
| `duplicate_text_instance` | Duplicate a text instance: the copy is inserted directly after the original, shifted +12/+12 stage units (the text panel's Duplicate). Returns the copy's index. One undo step. | `index` | — |
| `nudge_text_instance` | Nudge a text instance by (dx, dy) stage units: every transform key shifts by the same delta. An instance with no keys has no position to move (changed: false). One undo step when it moves. | `index`, `dx`, `dy` | — |
| `set_text_instance_size` | Set a text instance's glyph height (clamped to 4-160, the text panel's size range). Ok with changed: false when the stored size already equals the clamped request. One undo step when the size changes. | `index`, `size` | — |
| `rotate_text_instance` | Rotate a text instance by the given degrees: every transform key's angle gains the same amount (the text panel rotates in 15-degree steps). One undo step. | `index`, `degrees` | — |
| `set_text_instance_color` | Set a text instance's colour (0xRRGGBB). One undo step. | `index`, `color` | — |
| `move_text_instance` | Send a text instance to an end of the draw order: to_front true draws it on top of the other texts, false puts it behind them. Already at that end is a no-op (changed: false). One undo step when it moves. | `index`, `to_front` | — |
| `swap_frame_with_neighbour` | Swap the current frame with its 'previous' or 'next' neighbour (the timeline's frame-swap buttons): poses, durations, easings and bends trade places. One undo step; refuses at either end of the timeline. | `direction` | — |
| `insert_frames` | Insert 'count' blank frames immediately after the current frame (the timeline's batch insert; count is clamped to 60 by the op). The inserted frames copy the current frame's timing. One undo step. | `count` | — |
| `get_keyframes` | Read one figure's keyframe list — the exact data the dopesheet lane draws (a frame is a keyframe when the figure's pose there differs from the previous frame's). Read-only. | `figure` | — |
| `list_frames` | Read the timeline's per-frame timing table: every frame's duration multiplier and easing override, plus the playhead and loop range — the read counterpart of set_frame_duration / set_frame_easing. Read-only. | — | — |
| `fx_set_hidden` | Hide or show one FX clip (the FX panel's hide toggle): a hidden clip renders nowhere — stage, raster and SVG all skip it. One undo step. | `index`, `hidden` | — |
| `fx_set_seed` | Set an FX clip's particle random seed — the value `fx_list` reports; the same kind with a different seed draws a different particle pattern. One undo step. | `index`, `seed` | — |
| `fx_set_angle` | Set an FX clip's emission angle in degrees — the value `fx_list` reports; the directed kinds (beam, trail, fountain) emit along it. One undo step. | `index`, `angle` | — |
| `fx_set_length` | Set an FX clip's length in stage units — beam reach / trail span (the value `add_fx_clip`'s `length` sets at creation and `fx_list` reports). One undo step. | `index`, `length` | — |
| `fx_preset_list` | List the FX clip presets saved in ./fx-presets/ by name, sorted — the read side of fx_preset_save / fx_preset_add (the panel's preset list). Read-only. | — | — |
| `particle_list_events` | List every particle event in the project: index, frame, stage position, kind, colour and anchor (figure/node) — the read the Particles panel rows show. Read-only. | — | — |
| `particle_delete_event` | Delete the particle event at 'index' (the Particles panel's Delete). Events after it shift down one index — re-read with particle_list_events. One undo step. | `index` | — |
| `particle_set_frame` | Move the particle event at 'index' to 0-based 'frame', clamped to the project's last frame (the panel's frame stepper and To playhead). The reply reports the frame the event actually landed on. One undo step. | `index`, `frame` | — |
| `particle_nudge_frame` | Move the particle event at 'index' by signed 'delta' frames, clamped at both ends of the timeline. The reply reports the frame the event actually landed on. One undo step. | `index`, `delta` | — |
| `particle_set_position` | Move the particle event at 'index' to the stage point (x, y) — the point its effect renders from, independent of its anchor (the panel's nudge pad writes this in 4-unit steps). One undo step. | `index`, `x`, `y` | — |
| `particle_set_anchor` | Anchor the particle event at 'index' to figure 'figure', joint 'node' (the panel's Anchor to figure / Anchor to joint), or clear the anchor back to a free stage event by passing 'figure' as null or omitting it (the panel's Clear anchor). The figure/node are validated before anything changes. One undo step. | `index` | `figure`, `node` |
| `particle_add_anchored` | Add a particle event anchored to the SELECTED figure's selected joint at the current frame (the panel's anchored add). 'kind' is a particle kind name (default burst). Refuses when the selection cannot anchor an event. One undo step. | — | `kind` |
| `particle_set_seed` | Set a particle event's random seed — the value `particle_list_events` reports; the same kind with a different seed draws a different particle pattern. One undo step. | `index`, `seed` | — |
| `draw_spray` | Append a spray-mark stroke on a frame (the Spray brush's stroke kind — draw_stroke cannot create one): dots are scattered around 'points' within 'radius'. 'frame' and 'points' are required; color (0xRRGGBB), width, radius, opacity and seed default from the active brush preset. One undo step. | `frame`, `points` | `color`, `width`, `radius`, `opacity`, `seed` |
| `get_stroke_style` | Read one stroke's style back by its project-wide index (see list_strokes): colour, width, per-point widths, fill, spray mark, opacity and scatter — the read half of set_stroke_style. Read-only. | `index` | — |
| `audio_list_clips` | List every audio clip with its stored fields (index, path, start frame, gain, muted) and probed health (healthy / missing / unsupported / unreadable, with duration) — the read the audio panel's rows show. Read-only. | — | — |
| `audio_clip_info` | Read one audio clip's full facts by 'index': stored fields, probed health, duration in seconds and frames, peak level, and waveform peaks at 'buckets' buckets (default 64; an empty peaks array means the file could not be decoded — see health). Read-only. | `index` | `buckets` |
| `audio_clip_fade` | Fade the audio clip at 'index' (0.25 s in/out, the panel's per-clip Fade) — the indexed form the panel has; the older audio_fade tool always edits the LAST clip. A clip whose file cannot be read is an error, not ok. One undo step. | `index` | — |
| `audio_clip_reverse` | Reverse the audio clip at 'index' — the indexed form of audio_reverse (which always edits the last clip). The processed file is written beside the source and the clip repointed. One undo step. | `index` | — |
| `audio_clip_set_gain` | Set the audio clip at 'index' gain (clamped to 0.0-2.0 by the op; the reply reports the gain read back after clamping, so a request for 5.0 truthfully reports 2.0) — the indexed form of audio_set_gain. One undo step. | `index`, `gain` | — |
| `audio_clip_gain_step` | Step the audio clip at 'index' gain by 'delta' (the audio panel's −/+ gain buttons): the stored gain is clamped to 0.0-2.0 by the op and the reply reports the gain read back after clamping, so a step past the ceiling truthfully reports 2.0 — the relative form of audio_clip_set_gain. One undo step. | `index`, `delta` | — |
| `audio_clip_move` | Move the audio clip at 'index' so it starts at the absolute 0-based 'start_frame' (the reply reports the actual start). One undo step. | `index`, `start_frame` | — |
| `audio_clip_duplicate` | Duplicate the audio clip at 'index' (the panel's Duplicate button): the copy is inserted immediately after the original at the same start frame; the reply names its index. One undo step. | `index` | — |
| `audio_clip_normalise` | Normalise the audio clip at 'index' FILE to peak 0.9 (the panel's per-row Norm button): the processed copy is written beside the source and the clip repointed; the clip's gain field is untouched. Distinct from audio_normalise_all, which is gain-based. One undo step. | `index` | — |
| `audio_clip_trim` | Trim the audio clip at 'index' file to the range [start_secs, end_secs) — SECONDS, not frames: the clip's start frame advances by round(start_secs * fps), preserving the surviving audio's timeline position; the reply reports the new start frame. A range entirely beyond the file is refused. One undo step. | `index`, `start_secs`, `end_secs` | — |
| `audio_clip_toggle_muted` | Toggle the audio clip at 'index' mute flag (the panel toolbar's Mute toggle); the reply reports the flag's new value, read back from the clip. audio_set_muted is the absolute form. One undo step. | `index` | — |
| `fig3_exit` | Exit 3D mode (the panel's mode toggle / the palette's Exit 3D mode). Baked 2D figures stay in the project. | — | — |
| `fig3_get` | Read the full 3D session state: figure, pose, selection, camera (yaw both raw and wrapped to -180..180 as 'yaw_wrapped' — the panel displays the wrapped value) and library state. Without a session, returns ok with active: false. Read-only. | — | — |
| `fig3_select_next` | Cycle the selected 3D joint to the next node (the panel's Joint> button). | — | — |
| `fig3_cycle_library` | Load the next 3D library figure, cycling the library (a core op with no panel button; the panel loads by name instead — fig3_load_from_library). Refuses when the library is empty. | — | — |
| `fig3_camera_preset` | Apply a named 3D camera preset — 'front', 'side', 'three-quarter' or 'top' (the panel's Camera row): replaces the session camera. | `name` | — |
| `fig3_frame_camera` | Frame the 3D session figure on the stage (the panel's Frame camera button): replaces the camera with a front camera targeted on the figure's centroid, fitted to the stage's central 80%. | — | — |
| `fig3_frame_preview` | Frame the 3D session figure in the panel preview (the panel's Frame button): keeps the camera's yaw/pitch/FOV and writes back only the fitted target and distance, at the preview viewport's aspect. | — | — |
| `fig3_camera_zoom` | One preview-zoom step on the 3D camera distance: 'dir' +1 zooms in, -1 zooms out (x/divided by 1.25, clamped to 40..=4000 — the panel's zoom buttons). | `dir` | — |
| `fig3_ik` | Solve two-bone IK for one limb of the 3D figure toward its natural hang: 'limb' is arm_l, arm_r, leg_l or leg_r (the panel's IK buttons). | `limb` | — |
| `fig3_pose_mirror` | Mirror the 3D session pose across the YZ plane (the panel's Mirror pose button). An exact involution: calling it twice restores the pose. | — | — |
| `fig3_pose_library_list` | List the 3D pose library (the panel's Pose library section). No session needed. Read-only. | — | — |
| `fig3_pose_save` | Save the 3D session pose to the pose library (the panel's Save pose button). 'name' defaults to '<figure> pose'. A file write, not an undo step. | — | `name` |
| `fig3_pose_apply` | Apply a saved 3D pose-library entry to the session figure (the panel's pose Apply buttons). | `name` | — |
| `graph_list_channels` | List every graph channel in project order: index (the index the other graph tools take), figure, target, key count and value/frame stats — the graph panel's channel list and stats block, as data. Read-only. | — | — |
| `graph_get_channel` | Read one graph channel in full by its project-list 'channel' index: every key (frame, value, tangents, interp). With optional 'frame', the reply also carries the channel's evaluated value there (value_at) — the graph panel's value readout. Read-only. | `channel` | `frame` |
| `graph_key_interp` | Read the interpolation mode of the key at 'frame' on the channel at 'channel' index (nearest key within 0.5 frames — the graph panel's interp-control readout). 'interp' is null when no key sits there. Read-only. | `channel`, `frame` | — |
| `graph_delete_key` | Delete the one key at 'frame' from the graph channel at 'channel' index. One undo step. | `channel`, `frame` | — |
| `graph_move_key` | Move the key nearest 'from_frame' (within 0.5 frames) in the channel at 'channel' index to 'to_frame' — the graph canvas drag: tangents travel with the key, its value travels unless 'value' overrides it, and a key already sitting at the destination is replaced. One undo step. | `channel`, `from_frame`, `to_frame` | `value` |
| `graph_set_tangents` | Set both tangents of the key at 'frame' in the channel at 'channel' index (in_tan / out_tan are the Hermite slopes the graph canvas drag writes). One undo step. | `channel`, `frame`, `in_tan`, `out_tan` | — |
| `graph_shift_keys` | Shift EVERY key of the channel at 'channel' index by 'delta' frames (negative = earlier) — the graph panel's shift buttons at full range. Refused whole (never clamped) when delta is 0 or any key would land before frame 0. One undo step. | `channel`, `delta` | — |
| `graph_fill_keys` | Fill the keyless frames of the channel at 'channel' index inside [from, to] with copies of the channel's key at the CURRENT frame (set the playhead first with set_frame). Returns the inserted count. One undo step when it inserts. | `channel`, `from`, `to` | — |
| `graph_delete_keys_in_range` | Delete the keys of the channel at 'channel' index whose frames fall inside [from, to], both ends inclusive. Returns the removed count. One undo step. | `channel`, `from`, `to` | — |
| `graph_count_keys_in_range` | Count the keys of the channel at 'channel' index inside [from, to], both ends inclusive — the read-only pre-flight for graph_delete_keys_in_range. Read-only. | `channel`, `from`, `to` | — |
| `graph_clear_channel_keys` | Remove every key from the channel at 'channel' index (the channel itself stays, empty). One undo step. | `channel` | — |
| `graph_reduce_keys` | Reduce the channel at 'channel' index: drop interior keys whose removal moves the curve by at most 'tolerance' value units (default 0.5, the graph panel's Reduce) — endpoints always kept. One undo step. | `channel` | `tolerance` |
| `graph_mirror_channel` | Mirror the channel at 'channel' index in time around 'center' (or 'centre'; default: the timeline's middle frame) — the index form of mirror_channel_time, which can also name a duplicate channel. One undo step. | `channel` | `center`, `centre` |
| `graph_scale_channel` | Scale every key value of the channel at 'channel' index by 'factor' around 'pivot' (default 0) — the index form of scale_channel_values. One undo step. | `channel`, `factor` | `pivot` |
| `graph_tangent_preset_channel` | Apply a tangent preset ('smooth', 'ease-in', 'ease-out', 'ease-in-out', 'linear') to every key of the channel at 'channel' index — the index form of graph_tangent_preset. One undo step. | `channel`, `preset` | — |
| `graph_key_all_channels` | Key every graph channel at 'frame' (default: the current frame) with its evaluated value there — the keyframe tools' Key-all-channels. One undo step. | — | `frame` |
| `graph_key_angle` | Key the selected node's current pose angle into its graph channel at the current frame (the graph panel's Key angle button — EditorCore::key_graph_angle). One undo step. | — | — |
| `graph_clear_channels` | Remove ALL graph channels from the project (the graph panel's Clear all channels). One undo step. | — | — |
| `graph_paste_keys_at` | Paste keys into a graph channel addressed by its index in graph_list_channels (the by-index twin of graph_paste_keys — use it when two channels share a figure+target and the name form would resolve the first). The earliest pasted key lands at 'at_frame'; existing keys within half a frame of a pasted key are replaced. 'keys' is the array graph_copy_keys returns. One undo step. | `channel`, `keys`, `at_frame` | — |
| `fx_preset_delete` | Delete a saved FX clip preset by name (the file in ./fx-presets/). A missing preset is an error naming it. A file operation, not an undo step. | `name` | — |
| `figure_cycle_opacity` | Cycle a figure's opacity one step through the inspector's steps (1.0 -> 0.6 -> 0.3 -> 1.0). 'figure' defaults to the selected figure. Like the panel cycler, this takes NO undo snapshot. | — | `figure` |
| `get_figure` | Read one figure in full: name, colour (0xRRGGBB), hidden/locked flags, opacity, node count, movieclip link, solo state, and the drawn bounding box on the current frame (null when the figure has no pose there). Read-only. | `figure` | — |
| `get_node` | Read one node in full: definition (name, parent, length, thickness, colour override, shape, head flag), the segment style (colour override + effective colour, color2, cap, taper, opacity), the segment's attached media ('media': null, or {kind: 'image', asset} / {kind: 'text', content, size}), and pose facts at 'frame' (default: current) — the stored parent-relative angle in degrees, the solved world direction of the bone ending at this joint in degrees [0, 360) (null for a root node), and the frame's bend in radians and degrees. Note: get_pose returns stored parent-relative angles; the segment inspector shows this same world direction. Read-only. | `figure`, `node` | `frame` |
| `figure_library_list` | List the 2D figure library (definitions saved in ./library): index, name and node count per entry, plus the session's current library index. The library stores FigureDef only — an entry's colour is NOT stored; inserting assigns a palette colour. Read-only. | — | — |
| `figure_library_save` | Save a figure's definition to the 2D figure library under the figure's name (the library panel's Save). 'figure' defaults to the selected figure and selects it first. A file write, not an undo step. | — | `figure` |
| `figure_library_insert` | Insert a figure from the 2D figure library into the project (the library panel's Insert). Choose the entry with 'index' or 'name'; with neither, the session's current library index is used. The inserted figure gets a palette colour (the library stores the definition — including its node colours and styles — not the instance colour) and becomes the selection. One undo step. | — | `index`, `name` |
| `figure_library_cycle` | Cycle the 2D figure library selection to the next entry (the library panel's cycle). Selection only — inserts nothing. Errors when the library is empty. | — | — |
| `fig3_set_camera` | — / `distance`, `target`, `fov_deg` | `yaw`, `pitch`, `distance`, `target`, `fov_deg` | ✗ → §4 |
| `fig3_set_rotation` | Set a 3D joint's Euler rotation absolutely, in degrees: 'joint' (defaults to the selected joint) and any of 'x', 'y', 'z'; omitted axes keep their values. At least one axis is required. The absolute complement of fig3_rotate. No undo step (session pose). Requires an active 3D session. | — | `joint`, `x`, `y`, `z` |
| `fig3_set_root` | Set the 3D pose's root position from 'position' [x, y, z] — the value fig3_get reports as pose_root; bakes project from the root. No undo step (session pose). Requires an active 3D session. | `position` | — |
| `fig3_rename` | Rename the 3D session figure — the name a later fig3_save_to_library files it under. Trimmed; a blank name is refused. A session field write, not an undo step. Requires an active 3D session. | `name` | — |
| `clip_delete` | Delete a movieclip from the project (the clips panel's Delete; figures linked to it are unlinked and an edit session on it ends). One undo step. | `index` | — |
| `clip_rename` | Rename a movieclip (the clips panel's rename). One undo step when the name actually changes — the op snapshots only on a real change, and an active edit session on the clip follows the rename. Renaming to the current name (or a blank name, which keeps the current one) is a no-op reported with changed=false. | `index`, `name` | — |
| `clip_duplicate` | Duplicate a movieclip (deep copy, inserted after the original) and return the new clip's index. One undo step. | `index` | — |
| `clip_move` | Move a movieclip one slot 'up' or 'down' in the clip list (the clips panel's reorder). One undo step; moving past either end is an error. | `index`, `direction` | — |
| `clip_insert_figure` | Insert a NEW figure instance of a movieclip (the clips panel's Insert figure): built from the clip's definition, placed at the panel's standard spawn position, linked to the clip, and selected. 'index' is the CLIP index. One undo step. | `index` | — |
| `clip_enter` | Enter a movieclip edit session (the clips panel's Edit): subsequent clip_* session tools pose the clip's own frames. Session state only, not an undo step. | `index` | — |
| `clip_exit` | Exit the current movieclip edit session. A no-op when no session is active. Session state only. | — | — |
| `clip_goto` | Move the movieclip edit session's playhead to a frame of the clip (clamped to the clip's range). Requires an active session (clip_enter). | `frame` | — |
| `clip_add_frame` | Append a frame to the edited movieclip, copying the session's current frame pose. One undo step. Requires an active session (clip_enter). | — | — |
| `clip_delete_frame` | Delete the movieclip edit session's current frame. One undo step. Refuses the clip's last remaining frame. Requires an active session (clip_enter). | — | — |
| `clip_set_angle` | Set one joint angle (degrees, stored parent-relative) on the movieclip session's current frame. Like the core pose-tweak ops this takes NO undo snapshot (per-angle tweaks would flood the history); use clip_add_frame/undo boundaries deliberately. Requires an active session (clip_enter). | `node`, `degrees` | — |
| `clip_set_root` | Set the root position [x, y] of the movieclip session's current frame. No undo snapshot (pose-tweak rule, as clip_set_angle). Requires an active session (clip_enter). | `root` | — |
| `clip_get_pose` | Read the movieclip edit session's current frame: clip index and name, frame, root [x, y] and all joint angles (degrees, parent-relative). Read-only. Requires an active session (clip_enter). | — | — |
| `set_project_background` | Set the project's stage background colour (0xRRGGBB integer). Read it back via get_state's 'background'. One undo step. | `color` | — |
| `list_projects` | List the saved projects in ./projects (the Home panel's scan): name, path, figure and frame counts, fps and last-modified time per project. Read-only. | — | — |
| `save_project_copy` | Save a copy of the current project in ./projects (the projects panel's Save a copy): the copy's name is derived — '<project name> copy' — and stored in the copy. Create-new semantics: if that copy already exists the call FAILS rather than overwriting or suffix-walking (a second call therefore refuses, exactly like a second panel press). The session keeps editing the original project. | — | — |
| `duplicate_project` | Duplicate the current project's saved FILE on disk, walking the -copy / -copy-2 suffixes until a free name is found (the projects panel's Duplicate). The session keeps editing the original. Errors when the current project has no saved file. | — | — |
| `project_rename` | Rename the current project's saved file to 'new_name' (a target that already exists is refused, leaving both files untouched). The session project's name follows and the file selector is repointed at the renamed file; the name stored inside the file updates on the next save. Errors when the current project has no saved file. | `new_name` | — |
| `delete_project_file` | Delete a SAVED project file from ./projects — the Home panel's delete, via MCP. 'path' names the file exactly as list_projects reports it (its full path or its bare file name); anything not in that listing is refused, so the tool cannot be aimed outside the library. The currently open project, if this is its file, stays open in memory, exactly as in the panel. No undo step: a file operation, not a project edit. | `path` | — |
| `cycle_project_file` | Advance the project-file selector by one (the palette's Cycle project file); reports the newly selected file, opens nothing. No undo step. | — | — |
| `open_project_file` | Open the project file the selector currently names (the palette's Open project file; cycle_project_file moves the selector). A failed open keeps the current project and reports the failure in 'status'. No undo step (project-swap family, like load_project). | — | — |
| `attach_image_to_node` | Attach a sprite image to the SELECTED joint of the selected figure (the segment inspector's Attach image). Selection-scoped: steer the selection with set_selection first. The op takes the first PNG found in ./import — there is no path argument, matching the GUI op exactly. One undo step (inside the op). | — | — |
| `clear_node_media` | Clear the media (sprite) attached to the SELECTED joint of the selected figure (the segment inspector's Clear media). Selection-scoped: steer the selection with set_selection first. One undo step. | — | — |
| `assets_list` | List the project's image asset table: for each asset its index, name, stored byte size, decoded pixel width/height (null when the stored bytes do not decode) and live reference counts split by kind (figure segment images, movieclip-def segment images, sprite instances). Read-only. | — | — |
| `asset_info` | One asset's assets_list entry, addressed by its table index. Read-only. | `index` | — |
| `asset_import` | Import a PNG file from disk into the asset table (appended; its index is returned). The file must start with the PNG magic bytes, decode as an image and be at most 8388608 bytes (8 MiB) — anything else is refused before any change. The stored name defaults to the file's name; the optional 'name' argument overrides it. One undo step. (Unlike attach_image_to_node, this never touches ./import and attaches nothing.) | `path` | `name` |
| `asset_rename` | Rename the asset at 'index' to 'name' (trimmed; empty is refused). Renaming to the current name is a no-op that reports changed: false and spends no undo step; a real rename is one undo step. | `index`, `name` | — |
| `asset_remove` | Remove the asset at 'index' from the table. REFUSED, with the live counts, while anything still references it (a figure segment image, a movieclip-def segment image, or a sprite instance) — removal never orphans a reference; clear segment references first with clear_node_media or re-point them with asset_attach, and remove sprite instances with sprite_remove. When it does remove, every reference to a higher index is remapped down by one in the same undo step, so references keep naming the same image. One undo step. | `index` | — |
| `asset_attach` | Attach the EXISTING asset at index 'asset' to figure 'figure', node 'node' as that segment's image media, replacing whatever media the segment carried (the by-index counterpart of attach_image_to_node, which reads ./import and works on the selection). One undo step. | `figure`, `node`, `asset` | — |
| `clip_asset_attach` | Attach the EXISTING asset at index 'asset' to movieclip definition 'clip', node 'node' as that segment's image media, replacing whatever media the definition's segment carried (the clip-def counterpart of asset_attach, which writes a figure's own definition). The media renders on every instance of the clip whose own definition carries no media at that node — an instance's own media wins. One undo step. | `clip`, `node`, `asset` | — |
| `clip_clear_node_media` | Clear the media (image or text) attached to movieclip definition 'clip', node 'node' (the clip-def counterpart of clear_node_media, which is selection-scoped to a figure). Clearing a node that carries no media is reported, not an error. One undo step when media was actually cleared. | `clip`, `node` | — |
| `clip_get_node_media` | Read the segment media attached to movieclip definition 'clip', node 'node': null, or {kind: 'image', asset} / {kind: 'text', content, size} — the same shape get_node returns for a figure's node. Read-only. | `clip`, `node` | — |
| `sprite_list` | Every sprite instance in the project: index, asset index and asset name, key count, and every transform key (frame, x, y, scale, rotation in degrees). Read-only. | — | — |
| `sprite_add` | Place a new sprite instance of the image asset at 'asset': one key at frame 0 puts it at the stage centre, scale 1, no rotation. Returns the new instance's index. One undo step. | `asset` | — |
| `sprite_remove` | Remove the sprite instance at 'sprite'. Later instances shift down one index; nothing references an instance by index, so no remapping is owed. One undo step. | `sprite` | — |
| `sprite_set_key` | Write sprite 'sprite''s transform key at 'frame' (0-based, a whole frame number; clamped to the project's last frame): insert a new key or replace the one there. 'x', 'y', 'rotation' (degrees) and 'scale' are each optional — an omitted field takes the value the sprite samples at that frame (or x 0, y 0, rotation 0, scale 1 for a sprite with no keys). 'scale' must be greater than 0. One undo step. | `sprite`, `frame` | `x`, `y`, `scale`, `rotation` |
| `sprite_delete_key` | Delete sprite 'sprite''s key at 'frame' (0-based, a whole frame number; clamped to the project's last frame). A frame with no key is refused before any change, so no undo step is spent. Deleting the last remaining key is allowed — the instance then draws nothing. One undo step on success. | `sprite`, `frame` | — |
| `sprite_sample` | Read sprite 'sprite''s sampled transform at 'frame' (fractional frames allowed): the interpolated key between its authored keys, clamped outside the keyed range — the transform the stage and exports draw. Read-only; no undo step. | `sprite`, `frame` | — |

### Shell control — live endpoint only (35, `src/shell_tools_*.rs`, registered in `src/mcp_tools.rs`)

The shell family drives the running editor's window itself — see "Shell control — driving the window itself" below for how it works and the security posture. Thirty-one of the thirty-five are live-only in effect: registered on both transports, but on stdio each answers an `isError` result saying it needs the live editor. The four transport tools are the exception — playback state is editor-core state, so they execute on stdio too, against the stdio server's own core (nothing advances it there).

| Tool | Purpose | Required | Optional |
|---|---|---|---|
| `shell_set_workspace` | Switch the running editor's workspace by name: one of Animate, Draw, Graph, Effects, Audio, 3D, Export, Home (exact, case-sensitive). The workspace chip's own action, persistence included; an unknown name is refused, listing the valid set. Shell state only: no undo step. Live endpoint only. | `workspace` | — |
| `shell_get_state` | Read the running editor's shell state in one call: workspace; transport (playing/paused, frame, playhead, frame count, fps, loop, speed); view (view zoom/pan separate from the authored camera, plus the effective zoom); selections (figure, node, sprite, audio clip, FX clip, clip edit session); theme; collapsed panels. Read-only. Live endpoint only. | — | — |
| `shell_play` | Start playback in the live editor (the Play button): the playhead starts from the current frame; the shell poll advances it and starts playback audio, as for a GUI play press. Idempotent. Not an undo step; never marks dirty. On stdio the flag is set but nothing advances it. | — | — |
| `shell_pause` | Pause playback in the live editor (the Pause button): the playhead stays where it stopped; the shell poll stops playback audio. Idempotent. Not an undo step; never marks dirty. | — | — |
| `shell_set_playhead` | Move the playhead to a 0-based frame (a timeline click, `EditorCore::goto`): sets the current frame and snaps the fractional playhead onto it, playing or paused. Out-of-range frames are refused. The shell-family name for the set_frame built-in's move. Not an undo step; never marks dirty; selection untouched. | `frame` | — |
| `shell_get_transport` | Read the transport state: `playing`, current `frame`, fractional `playhead`, `frame_count`, project `fps` and `loop_range` (`[start, end]` inclusive or null). Read-only. The loop on/off toggle and playback speed are shell state and are not reported. | — | — |
| `shell_view_zoom` | View-only zoom: `factor` multiplies the view zoom; `percent` sets the effective (camera × view) zoom %. Exactly one of the two is required. Clamped to the view range 0.1×–8×; reply reports the resulting view state + `clamped`. No camera key, no undo step, no dirty mark. Live shell only. | — | `factor`, `percent` |
| `shell_view_fit` | Pill Fit: view zoom 1×, pan zero (the camera's own framing). No camera key, no undo step, no dirty mark. Live shell only. | — | — |
| `shell_view_centre` | Pill Centre: pan the view so the selected figure's root (or the given stage point) sits at the element centre; clears the pan when no stage is laid out yet (figure form only). No camera key, no undo step, no dirty mark. Live shell only. | — | `x`, `y` |
| `shell_view_reset` | Pill 1:1: view zoom = 1 / camera zoom (effective zoom 100%), pan unchanged; `clamped` reported when the view range prevents exactly 100%. No camera key, no undo step, no dirty mark. Live shell only. | — | — |
| `shell_get_view` | Read the view state: `view_zoom`, `pan`, `at_camera_framing`, `camera_zoom`, `effective_zoom`, `effective_percent`, `frame`, `stage_element_size`, `stage_size`. Read-only. Live shell only. | — | — |
| `shell_select_figure` | Select a figure in the GUI exactly as clicking its Figures-panel row (the node selection is cleared). Shell selection only: no undo step, never marks dirty. | `figure` | — |
| `shell_select_node` | Select a node of a figure in the GUI exactly as pressing that joint on the stage (figure and node selection both move). Shell selection only: no undo step, never marks dirty. | `figure`, `node` | — |
| `shell_select_sprite` | Select a sprite instance in the GUI exactly as clicking its Sprites-panel row. Shell selection only: no undo step, never marks dirty. | `sprite` | — |
| `shell_select_fx_clip` | Select an FX clip in the GUI exactly as pressing its FX-lane bar or panel row. Shell selection only: no undo step, never marks dirty. | `clip` | — |
| `shell_select_audio_clip` | Select an audio clip in the GUI exactly as pressing its Audio-panel row (the operations toolbar's target). Shell selection only: no undo step, never marks dirty. | `clip` | — |
| `shell_select_frame` | Move the GUI playhead to a frame exactly as a plain timeline-chip click (any range selection is cleared). Shell selection only: no undo step, never marks dirty. | `frame` | — |
| `shell_get_selection` | Read back every GUI selection (figure, node, sprite, FX clip, audio clip, playhead frame, timeline range) with the live counts. Read-only. | — | — |
| `shell_set_theme` | Set the editor theme outright ('dark' or 'light') — the Settings card's Dark/Light buttons as a tool. Applies immediately and persists via the shell's layout-prefs write-back; accent colours untouched. Live endpoint only. No undo step (shell state). | `theme` | — |
| `shell_get_settings` | Read every user-facing setting the Settings card exposes, from its real home: theme, live-MCP (on/off, endpoint, persisted preference, pending stop, last start outcome), language, default easing, guides flags, loop/speed, workspaces, stage size, autosave status. Read-only. Live endpoint only. | — | — |
| `shell_set_setting` | Set one Settings-card boolean/enum setting by name (theme, live_mcp, language, default_easing, guides, camera_frame, safe_frames, loop_playback, playback_speed). Persists exactly as the card persists it; session-only flags say so. All immediate, no restart; live_mcp=false is applied just after the reply is delivered. Non-settable names are refused with the reason. Live endpoint only. No undo step. | `setting`, `value` | — |
| `shell_set_section` | Collapse or expand a panel section (the header Hide/Show control). Only Figures and Library are collapsible; a section outside the active workspace is refused. No undo step; session-only. Live endpoint only. | `section`, `collapsed` | — |
| `shell_set_column_widths` | Set the side-column widths (the splitters, by target). At least one of `left` / `right` is required; clamped to 220–420 / 260–480 and persisted via the layout prefs; the reply reports the read-back widths. No undo step. Live endpoint only. | — | `left`, `right` |
| `shell_get_layout` | Read the live layout: active workspace, visible sections + collapse state, hidden sections, column visibility and widths, and the (currently unwired) timeline/sidebar prefs with their `wired` flags. Read-only. Live endpoint only. | — | — |
| `shell_set_onion_depths` | Set the onion-skin ghost depths in the live editor — 'prev' and/or 'next' (0-3 each), the Onion section's steppers by target value. Shell view state: no undo step, never marks the project dirty, session-only exactly like the steppers. Live endpoint only. | — | `prev`, `next` |
| `shell_select_stroke` | Select a stroke in the GUI — 'stroke' (0-based, an index into the project's stroke list, the index delete_stroke consumes) becomes the shell's stroke selection, the one the Edit menu's Delete stroke acts on. Shell selection only: no undo step, never marks dirty. Live endpoint only. | `stroke` | — |
| `shell_select_particle` | Select a particle event in the GUI (the Edit menu's selection). Shell selection only: no undo step, never marks dirty. Live endpoint only. | `particle` | — |
| `shell_select_clip` | Select a movieclip in the GUI — 'clip' (0-based, an index into the project's clips) becomes the clips panel's selection. Shell selection only: no undo step, never marks dirty. Live endpoint only. | `clip` | — |
| `shell_list_recents` | List the File menu's Open recent entries (at most five, most recent first): index, display name, resolved project file or `missing: true` when the file is gone. Read-only. Live endpoint only. | — | — |
| `shell_open_recent` | Open a recent project by its File menu index (0-based; see `shell_list_recents`): the menu's own press — scan resolution, the shell open path and its follow-ups (stale-solo guard, name re-sync, stroke selection cleared). Unresolved entries and out-of-range indices are refused, quoting the value. Replaces the session without a discard confirm, like the menu. Not an undo step. Live endpoint only. | `index` | — |
| `shell_reload_project` | Reload the current project from its saved file (the Project section's Reload from disk), discarding unsaved changes. Two-press gate: on a dirty session the first call arms (`armed: true`) and the second runs. Refused when the project has no saved file. Not an undo step. Live endpoint only. | — | — |
| `shell_recover_snapshot` | Recover the autosave snapshot (`./projects/.recovery.json`) as the current project (the Recover press), replacing the session. Two-press gate: on a dirty session the first call arms (`armed: true`) and the second runs. Refused when no snapshot exists. The recovery file is read, never written. Not an undo step. Live endpoint only. | — | — |
| `shell_duplicate_project_file` | Duplicate any saved project file from the Home library, addressed by its stored name: the copy lands beside the source as `<stem>-copy.json` (incrementing if taken), as the row's Duplicate button does it. The session is untouched. An unknown name is refused, quoting it. Not an undo step. Live endpoint only. | `name` | — |
| `shell_delete_project_file` | Delete any saved project file from the Home library, addressed by its stored name. Two calls over the panel's own arm state: the first arms (`armed: true`, no file touched), the second with `confirm: true` deletes — only while the arm for that name stands. Shell-owned files are refused by the shell's guard. Deleting the open project's file leaves the session's copy untouched. Not an undo step. Live endpoint only. | `name` | `confirm` |
| `shell_set_sidebar` | Show or hide a side column — the View menu's Figures sidebar (`side: "left"`) and Inspector sidebar (`side: "right"`) items, flipping the same shell fields. Contents and widths untouched (see `shell_get_layout` / `shell_set_column_widths`). An unknown side is refused, quoting it. Session state only: no undo step. Live endpoint only. | `side`, `visible` | — |

### File IO and export (9, `src/mcp_io.rs`)

| Tool | Purpose | Required | Optional |
|---|---|---|---|
| `save_project` | Save the current project as StykAnimate project JSON (.styk.json format) at the given path. The write is atomic (temp file + rename), so a failed save never truncates an existing file; like the editor's save, it restarts the session's dirty baseline. Parent directories are created. | `path` | — |
| `load_project` | Load a StykAnimate project JSON file, replacing the current session project, with the editor's open bookkeeping (selection and playhead reset to the start, dirty baseline restarted). Returns the project summary. | `path` | — |
| `import_file` | Import a file by extension: .piv (Pivot animation), .nodemc/.stknds (Stick Nodes animation) and .nodes (Stick Nodes figure) replace the project; .stk (Pivot figure) adds the figure to the current project; .json loads project JSON. Session bookkeeping matches the editor's own import for each format (the Stick Nodes and .stk imports are single undoable edits; .piv and .json restart the dirty baseline). Returns the project summary. | `path` | — |
| `export_file` | Export the project: 'piv2', 'piv3', 'piv4' or 'piv5' write the whole animation as .piv bytes; 'stk' writes a single figure as Pivot .stk bytes and 'nodes' writes it as a Stick Nodes .nodes figure (both take the 'figure' index, defaulting to the selected figure); 'stknds' writes the whole animation as a Stick Nodes project; 'mp4_range' and 'webm_range' render only frames 'from'..='to' (both required, integer frame indices) to a video file (needs ffmpeg). Parent directories are created. | `path`, `format` | `figure`, `from`, `to` |
| `render` | Render the animation. Formats: 'png' renders ONE frame (arg 'frame', default current; arg 'scale', default 1.0 = stage size) to a PNG file; 'gif', 'mp4' (needs ffmpeg), 'webm', 'apng' and 'spritesheet' render the whole timeline to the file at 'path'; 'svg', 'jpeg' and 'png_sequence' write one file per frame into the DIRECTORY at 'path' (created if needed). The sequence/animation formats 'gif', 'apng', 'spritesheet', 'svg', 'jpeg' and 'png_sequence' accept an optional frame range: args 'from' and 'to' (0-based, inclusive, given together, clamped to the timeline; echoed in the reply). Video ranges are export_file's mp4_range/webm_range. Parent directories are created for file targets. | `path`, `format` | `frame`, `from`, `to`, `scale` |
| `export_bundle` | Export a whole bundle of formats into the DIRECTORY at 'path' in one call (the export panel's two bundle buttons). kind 'images': a PNG sequence (png/ subdir), a JPEG sequence (jpeg/), one SVG per frame (svg/) and a spritesheet (spritesheet.png). kind 'video': '<project>.mp4' and '<project>.webm'. Bundles always cover the WHOLE timeline — they deliberately ignore frame ranges, matching the panel. Every member is attempted: the reply lists each member with ok + frame count, or ok:false + the failure's reason; member failures never fail the call itself. | `path`, `kind` | — |
| `render_frame_base64` | Render one frame (default: current) as a PNG at exact stage size and return it base64-encoded, so you can SEE the frame. Returns {frame, width, height, png_base64}. | — | `frame` |
| `render_contact_sheet` | Render a contact sheet (grid of frame thumbnails) as a PNG file at 'path'. Frame selection: 'frames' is an optional array of frame indices (out-of-range indices are clamped and reported in a note); or pass 'from'/'to' (0-based, inclusive, together) to draw the picks from that window; with neither, the default is up to 12 evenly spaced frames across the timeline — NOT one tile per frame (the export panel's sheet differs: it tiles every frame). 'frames' and 'from'/'to' together are an error. 'cols' is the grid column count (default 4, at least 1); 'thumb_width' is each thumbnail's width in pixels (default 240, 16..=1024). Returns {path, width, height, frames, cols, thumb_width}. Parent directories are created. | `path` | `frames`, `from`, `to`, `cols`, `thumb_width` |
| `project_summary` | Summarise the current project: name, figure names, frame count, fps, duration in seconds, stage size, whether it has audio, its weather setting, and the number of camera keys. | — | — |

## Shell control — driving the window itself

The shell tools (`shell_*`, 35 in the catalogue below) operate
the running editor's GUI shell: its workspace, its transport
as the user sees it, the view layer, the panels' selections,
the theme and settings, and the File menu's project actions.
They exist because that state does not live in the editor
core the other tools share — the workspace is a shell field,
the sprite selection is panel state, the view zoom is a
shell-side layer over the authored camera. A core handler,
which receives only the core, cannot reach any of it.

**How it works.** The registry's handler type cannot take
the shell state, so the two halves of a shell tool live in
different places:

1. In the shared registry (`src/mcp_tools.rs`) every
   live-only shell name is registered with a stub handler,
   `shell_unavailable`, so `tools/list` is identical on
   both transports.
2. On the **live endpoint**, a call for a shell name never
   reaches the stub: the bridge's drain peeks at each
   request (`shell_control::peek` in the editor binary)
   and routes shell calls to a bounded shell queue (8,
   the bridge's own bound) carrying the request's own
   reply channel and cancellation flag.
3. The editor's poll loop drains that queue immediately
   after the core drain, on the UI thread, dispatching by
   name to the handler modules (`src/shell_tools_*.rs`),
   each of which performs exactly the writes the
   corresponding GUI control performs. The reply is built
   by the same `tool_result` the registry uses, so a shell
   reply is byte-shaped like any other tool reply.

Consequences worth knowing as a client:

- **A frame of latency is normal** — the same poll
  heartbeat a core call already waits on.
- **Cancelled calls never execute late.** A request whose
  client already received a 504 (or whose bridge was
  switched off mid-wait) is discarded by the shell drain,
  exactly as the core drain discards it.
- **Shell actions are session state, not edits.** They
  take no undo step and do not dirty the project, beyond
  what the shell itself persists for its own layout (the
  active workspace, the theme, the column widths are
  layout preferences, and shell tools persist them
  exactly as the equivalent clicks do).
- **There is no Stop tool**, deliberately: the GUI has
  no Stop control. Pause plus `shell_set_playhead` to
  frame 0 is the composition, and it is named as such.
- **The transport four are core tools.** Playback state
  is editor-core state, so `shell_play`, `shell_pause`,
  `shell_set_playhead` and `shell_get_transport` have
  real registry handlers and execute on stdio too —
  against the stdio server's own core, where nothing
  advances the playhead.

**stdio behaviour.** A live-only shell call on stdio is
an `isError` tool result — never a `-32602`, which is
reserved for genuinely unknown names — with the one
canonical wording: "{name} needs the live editor:
connect to its live endpoint
http://127.0.0.1:32117/mcp instead."

**Security posture.** Shell control inherits the live
endpoint's posture exactly: loopback only, **no
password**. Anyone who can reach the endpoint can now
switch the workspace, press play and open recent
projects, not just edit the project. The Settings card
says so in plain words beside the toggle; turn Live MCP
control off there when you do not want it.

## Notes and deliberate limits

- **Undo rewinds the project, not the session.** History stores
  `Project` snapshots only: undoing an edit restores the project
  (poses, clips, strokes, …) but never the playhead or the
  selection, which stay where the user/agent left them. Every
  mutating tool is exactly one undo step unless its catalogue
  entry says otherwise (a few panel-parity exceptions — figure
  visibility/layer writes, figure rename, clip pose tweaks —
  mirror the GUI op they call, which does not snapshot either).
- **Node style is export-faithful, stage-approximate.** Segment
  cap, taper and opacity (`set_node_cap`, `set_node_taper`,
  `set_node_opacity`, `set_stroke_style`) are honoured by both
  exporters (raster and SVG) but the live stage still draws
  segments without cap/taper/opacity styling — style edits are
  fully visible in renders, approximate on the stage. Stage
  *text* has no such gap: text instances (`add_text_instance`
  and family) draw on the stage exactly as they export.
- **Selection-scoped tools steer the editor's selection.** A
  few tools act on the selected figure/node
  (`attach_image_to_node`, `set_brush` and the draw tools'
  brush state). On the live transport that is the GUI's own
  selection/brush — one core, by construction; steer it first
  with `set_selection`. On the stdio transport the same state
  exists but is invisible.
- **3D sessions draw on the stage only in the Build3D
  workspace.** A session started with `fig3_enter` over live
  control exists immediately, but the shell draws it on the
  main stage only while the Build3D workspace is active — a
  shell gate, not an MCP limitation.
- **FX kind names have two spellings.** `add_fx_clip` and the
  preset tools report a clip's kind with the enum's Debug
  spelling (`Burst`); `fx_list` reports snake_case (`burst`).
  Same kinds, both spellings accepted on input.
- **Legacy audio tools act on the LAST clip.** `audio_fade`,
  `audio_reverse`, `audio_set_gain` and `audio_move_clip`
  predate indexed addressing and always hit the last clip on
  the timeline; the `audio_clip_*` family addresses any clip
  by index. Both families are kept: scripts written against
  the legacy tools keep working.
- **Deleting project files is `delete_project_file`.**
  The Home panel's delete, via MCP: it names a file
  from the `list_projects` listing only, so it cannot
  be aimed outside ./projects. There is still no
  tool that deletes the *currently open* project as
  a project operation — the file operation is the
  whole surface, exactly as in the GUI.
- **`save_project_copy` takes no name argument.** The copy is
  named `<project name> copy`, exactly as the GUI derives it,
  and the stored name inside the copy matches.

## Shared vocabularies

- **FX kinds** (`add_fx_clip`, `fx_set_kind`): 27 names —
  `burst`, `sparkle`, `smoke`, `beam`, `aura`, `trail`,
  `confetti`, `shock_ring`, `ember`, `fountain`, `orbit`,
  `lightning`, `bubbles`, `leaves`, `meteor`, `petals`, `splash`,
  `spiral`, `fireworks`, `aurora`, `comet`, `galaxy`, `prism`,
  `vortex`, `glitter`, `wave`, `shard`. `parse_clip_kind` in
  `src/mcp_tools.rs` also accepts the aliases `shockring` and
  `shock-ring`. (Particle *events* are a smaller vocabulary:
  `particle_set_kind` accepts only `burst`, `sparkle`, `smoke`.)
- **Weather** (`set_weather`): `kind` is `rain`, `snow`,
  `fireflies`, `fog`, `storm` or `off`; optional `amount` /
  `wind`.
- **Render formats** (`render`): `png` (one frame), `gif`,
  `mp4` (needs ffmpeg), `webm`, `apng`, `spritesheet`, `svg`,
  `jpeg` and `png_sequence` (the last three write one file per
  frame into a directory). The sequence formats (`gif`, `apng`,
  `spritesheet`, `svg`, `jpeg`, `png_sequence`) also accept a
  `from`/`to` frame range; `png`, `mp4` and `webm` refuse
  ranges — video ranges are `export_file`'s `mp4_range` /
  `webm_range`.
- **Export formats** (`export_file`): `piv2`, `piv3`, `piv4`,
  `piv5`, `stk`, `nodes`, `stknds`, `mp4_range`, `webm_range`
  (the range formats require `from` and `to` and need ffmpeg).
- **Import** (`import_file`, by extension): `.piv`, `.nodemc` /
  `.stknds` and `.nodes` replace the project; `.stk` adds a
  figure to the current project; `.json` loads project JSON.
- **Graph targets** (`graph_key`, `mirror_channel_time`,
  `scale_channel_values`, `graph_copy_keys`,
  `graph_paste_keys`): `root_x`, `root_y`, `angle:N`,
  `length:N`, or an object `{"angle": node}`. When two
  channels share a figure+target, `graph_paste_keys_at`
  addresses the channel by its `graph_list_channels` index
  instead.

## Maintainers

This document is a contract, not a changelog: after any wave
that touches `src/mcp.rs`, `src/mcp_tools.rs` or `src/mcp_io.rs`,
re-derive the counts (the `server_info` total and the catalogue)
before trusting either. Do not document a tool that does not
exist in `all_tools()` yet.
