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
identically on both; only the session it edits differs.

## Tool count — 277, and how to check it

At the time of writing the registry holds **277 tools**:

| Source | Count | Where they are defined |
|---|---|---|
| Built-ins | 44 | `fn tool_*` constructors assembled in `all_tools()`, `src/mcp.rs` |
| Editing registry | 224 | `pub fn tools()`, `src/mcp_tools.rs` |
| File IO / export | 9 | `pub fn tools()`, `src/mcp_io.rs` |
| **Total** | **277** | `all_tools()` = built-ins, then `mcp_tools`, then `mcp_io` |

Runtime cross-check: call the `server_info` tool — it returns
`all_tools().len()` — or count the entries of a `tools/list`
response. Source cross-check: `scripts/mcp_census.py` re-derives
the whole surface from the three source files and fails on any
duplicate name, unregistered handler or orphan handler; the
pre-commit gate (check 9 of `scripts/phase_j_checks.py`) also
verifies that every registered tool is named in the catalogue
below and that the catalogue names no tool that does not exist.
The catalogue lists every tool by name, in registry order.

## Protocol (both transports)

- JSON-RPC 2.0. Methods: `initialize`, `ping`, `tools/list`,
  `tools/call`. Protocol version `2024-11-05`.
- `initialize` answers with `capabilities: { "tools": {} }` and
  `serverInfo: { "name": "stykanimate-rs", "version": <Cargo
  package version> }`.
- One message is one JSON object. **JSON-RPC batches are not
  supported**: a JSON array gets a `-32600` error.
- Notifications (messages with no `id`) are never answered,
  whatever the method — on stdio no response line is written; on
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
| `copy_frame` | Copy the current frame to the single-frame clipboard (the Edit menu's Copy frame). Not an undoable edit. | — | — |
| `paste_frame` | Paste the single-frame clipboard (see copy_frame) as a new frame after the current one, which becomes current. One undo step. Errors when nothing was copied or the copied frame's figure count no longer matches. | — | — |
| `shift_frames` | Move a block of frames to a new position in the timeline (frame-range ops): 'count' frames starting at 'from' shift by 'delta' slots (negative moves earlier). One undo step. | `from`, `count`, `delta` | — |
| `reverse_range` | Reverse a block of frames in place (frame-range ops): 'count' frames starting at 'from' (clamped to the timeline end) play backwards. One undo step. | `from`, `count` | — |
| `delete_range` | Delete a block of frames (frame-range ops): 'count' frames starting at 'from' (clamped to the timeline end). Refuses to delete every frame — a project always keeps at least one. One undo step. | `from`, `count` | — |
| `copy_range` | Copy a block of frames into the range clipboard (frame-range ops): 'count' frames starting at 'from' (clamped to the timeline end), ready for paste_range. Not an undoable edit. | `from`, `count` | — |
| `paste_range` | Insert the range clipboard's frames (see copy_range) at 0-based index 'at' (clamped to the timeline, so a large 'at' appends), shifting later frames. One undo step. Errors when the range clipboard is empty. | `at` | — |
| `set_loop_range` | Set the inclusive playback loop range [start, end] (0-based; the timeline's Set loop row). Endpoints are ordered and clamped to the timeline; start == end loops a single frame. Editor runtime state, like the playhead — not an undoable edit. | `start`, `end` | — |
| `clear_loop_range` | Clear the playback loop range (the timeline's loop Clear), so playback wraps at the timeline end again. | — | — |
| `set_frame_easing` | Set one frame's easing override (the easing panel's frame picker): "linear", "smooth", "ease-in" or "ease-out", or null for the project default. Applies to the current frame unless 'frame' names another. One undo step. | `easing` | `frame` |
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

### Editing registry (224, `src/mcp_tools.rs`)

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
| `set_frame_durations` | Set hold-time multipliers for frames 0..n in one call (same positive-value rule as set_frame_duration; extra entries beyond the frame count are ignored). | `durations` | — |
| `set_project_easing` | Set the project's default easing for tweened playback: "linear", "smooth", "ease-in" or "ease-out" (the easing panel's preset buttons). A project-settings tweak — not an undo step. | `easing` | — |
| `get_pose` | Read a figure's pose in a frame: {root: [x, y], angles: [degrees per node]}. Angles are the STORED values: parent-relative (each relative to its parent segment) and unnormalised (725 means 725, not 5). For the solved world direction of one segment, use get_node. | `figure`, `frame` | — |
| `set_pose` | Write a figure's pose in a frame. 'angles' is either a full array (one angle per node, degrees) or an object mapping node index to degrees; 'root' is [x, y]. Angles are stored parent-relative and verbatim (no wrapping — 725 stays 725). One undo step. | `figure`, `frame` | `root`, `angles` |
| `mirror_pose` | Mirror the selected figure's pose in the current frame (left-right flip around its root). One undo step. | — | — |
| `mirror_figure` | Mirror a whole figure left-right in EVERY frame (the all-frames counterpart of mirror_pose): every frame's pose is flipped around the figure's root. One undo step. | `figure` | — |
| `set_node` | Edit a figure definition's segment: length (clamped to 4..=220), thickness (clamped to 1..=18), colour override, or shape ("line" \| "circle" \| "polygon" \| side count 3-12). Pass "color": null to CLEAR the colour override (an absent 'color' leaves it unchanged). One undo step. | `figure`, `node` | `length`, `thickness`, `color`, `shape` |
| `set_node_cap` | Set a segment's end cap directly: "round", "square" or "point" (the style panel's cap control). One undo step. | `figure`, `node`, `cap` | — |
| `set_node_color2` | Set a segment's gradient end colour (0xRRGGBB; the segment blends its own colour -> color2). Omit 'color2' (or pass null) to clear the gradient back to a solid colour. One undo step. | `figure`, `node` | `color2` |
| `set_node_opacity` | Set a segment's opacity (the style panel's opacity stepper; clamped to 0.05-1.0 so a segment can never be made fully invisible). One undo step. | `figure`, `node`, `opacity` | — |
| `add_node` | Append a segment under the given parent node of a figure; returns the new node index. | `figure`, `parent`, `length` | `thickness` |
| `delete_node` | Delete a leaf segment of a figure (the root and segments with children are refused). | `figure`, `node` | — |
| `set_camera` | Upsert a camera key at a frame: centre (x, y), zoom factor (1.0 = neutral, must be positive), optional rotation angle in degrees — stored wrapped to -180..180, the panel's convention. One undo step. | `frame`, `x`, `y`, `zoom` | `angle` |
| `add_fx_clip` | Add a timed particle FX clip anchored to a figure's node. start/duration are in frames; a start at or past the project's end is refused (park clips with fx_move instead, which deliberately allows past-end starts). 'color2' optionally sets a gradient end colour (particles blend colour -> color2). | `figure`, `node`, `kind`, `start`, `duration` | `color`, `color2`, `length` |
| `fx_list` | List all timed FX clips: index, figure, node, kind, start, duration and end frame. | — | — |
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
| `set_project_fps` | Set the project's frame rate in frames per second (clamped to 1-60; the Scene panel's frame-rate stepper). One undo step when the rate changes. | `fps` | — |
| `set_stage_size` | Set the stage size in pixels (each axis clamped to 64-4096; the Scene panel's stage-size steppers). One undo step when the size changes. | `width`, `height` | — |
| `set_head_ring_scale` | Set the head-ring size as a multiple of the base ring radius (clamped to 0.5-2; applies to every figure's head ring in the stage and all exports). One undo step when the scale changes. | `scale` | — |
| `layers` | Apply layer state to a figure: locked, hidden, opacity (0.05-1.0), and solo (true solos this figure in the editor, false clears its solo). | `figure` | `locked`, `hidden`, `opacity`, `solo` |
| `undo` | Undo the last editor operation. | — | — |
| `redo` | Redo the last undone editor operation. | — | — |
| `draw_stroke` | Append a freehand stroke (polyline) to a frame's drawing layer. points is an array of [x, y] stage coordinates; color is 0xRRGGBB (default light grey), width in stage units (default 4, the editor's pen), scatter jitters the stroke's dabs by that radius in stage units (default 0 = clean line), opacity is the stroke's opacity 0-1 (default 1). One undo step. | `frame`, `points` | `color`, `width`, `scatter`, `opacity` |
| `draw_shape` | Append a geometric shape as a stroke: kind is "line" (2 points), "rect" (closed rectangle, 5 points) or "ellipse" (25 points), from corner (x1, y1) to corner (x2, y2) in stage coordinates. 'frame' defaults to the current frame. color is 0xRRGGBB (default light grey), width defaults to the active brush width; optional 'fill' (0xRRGGBB), 'opacity' (0-1) and 'scatter' (>= 0) mirror the stamp panel's state. One undo step. | `kind`, `x1`, `y1`, `x2`, `y2` | `frame`, `color`, `width`, `fill`, `opacity`, `scatter` |
| `draw_polygon` | Append a closed regular polygon as a stroke: centred at (cx, cy) with the given radius, 'sides' vertices (default 5, at least 3) plus the closing point, first vertex straight up unless 'rotation' (degrees) turns it. 'frame' defaults to the current frame. color is 0xRRGGBB (default light grey), width defaults to the active brush width; optional 'fill', 'opacity' and 'scatter' as in draw_shape. One undo step. | `cx`, `cy`, `radius` | `sides`, `frame`, `rotation`, `color`, `width`, `fill`, `opacity`, `scatter` |
| `draw_star` | Append a star outline as a stroke: centred at (cx, cy), outer radius 'radius', inner radius 26/60 of it (the stamp panel's ratio), 'points' spikes (default 5), first spike straight up unless 'rotation' (degrees) turns it. 'frame' defaults to the current frame. color is 0xRRGGBB (default light grey), width defaults to the active brush width; optional 'fill', 'opacity' and 'scatter' as in draw_shape. One undo step. | `cx`, `cy`, `radius` | `points`, `frame`, `rotation`, `color`, `width`, `fill`, `opacity`, `scatter` |
| `smooth_strokes` | Chaikin-smooth every stroke on a frame (one subdivision pass). Returns how many strokes changed. | `frame` | — |
| `erase_strokes` | Erase stroke points within radius of (x, y) on a frame, splitting strokes at the cut (draw_ops::erase semantics). Returns how many of the frame's strokes changed. | `frame`, `x`, `y`, `radius` | — |
| `simplify_strokes` | Simplify every stroke on a frame with Douglas-Peucker: interior points within 'epsilon' stage units of the path are dropped, endpoints kept. 'frame' defaults to the current frame. Returns how many strokes lost points. One undo step. | `epsilon` | `frame` |
| `delete_stroke` | Delete one stroke by its index in the project's stroke list (the order draw_stroke appends in). Returns the frame the stroke was on. One undo step. | `index` | — |
| `apply_nib` | Give every stroke on a frame a nib width profile: "round" (uniform), "square" or "calligraphy" (angle-dependent, angle in degrees, default 45). Returns the stroke count. | `frame`, `nib` | `angle` |
| `fig3_enter` | Enter 3D mode: start a session posing the 3D humanoid with an orbit camera. Keeping an existing session as-is. Pose with fig3_select/fig3_rotate/fig3_orbit, then fig3_bake turns the pose into a normal 2D figure. | — | — |
| `fig3_select` | Select the 3D joint (node index) that fig3_rotate acts on. | `joint` | — |
| `fig3_rotate` | Rotate the selected 3D joint by degrees around an Euler axis: 0 = X, 1 = Y, 2 = Z. | `axis`, `degrees` | — |
| `fig3_orbit` | Orbit the 3D camera by yaw/pitch deltas in degrees (pitch clamps to +-89.9). | `yaw`, `pitch` | — |
| `fig3_bake` | Bake the current 3D pose into a new 2D figure in the project (held on all frames, like the editor's Bake 3D). Returns the new figure index. | — | — |
| `fig3_mark_b` | Mark the current 3D pose as pose B, the target fig3_bake_animation interpolates to (pose the figure, mark B, repose to A, then bake the animation). | — | — |
| `fig3_bake_animation` | Bake an animated 2D figure: interpolate the 3D session's current pose to the marked pose B over 'frames' frames (at least 2), extending the timeline if needed. Requires fig3_mark_b first. | `frames` | — |
| `clip_create` | Create a movieclip from a figure's poses across all frames and set the figure to loop it (the editor's Make clip). Returns the clip index and frame count. | `figure` | — |
| `clip_list` | List the project's movieclip definitions: index, name, frame count, node count, and which figures loop each clip. | — | — |
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
| `simplify_stroke` | Simplify ONE stroke by its project-wide index with Douglas-Peucker (the stroke panel's Simplify press): interior points within 'epsilon' stage units (default 2) are dropped, that point's own width survives. Returns simplified: false — and takes no undo step — when the stroke is already simple at that epsilon. One undo step when it simplifies. | `index` | `epsilon` |
| `fill_stroke` | Fill the most recent stroke on the CURRENT frame: its fill is set to its own colour and a scanline fill is appended as a companion stroke (EditorCore::fill_current_stroke). Errors when there is nothing fillable on the frame. One undo step. | — | — |
| `lasso_erase` | Erase strokes on the CURRENT frame that lie mostly (a strict majority of their points) inside the lasso polygon 'points' (at least 3 [x, y] points). Errors when nothing lies mostly inside. One undo step when strokes are removed. | `points` | — |
| `mirror_strokes` | Mirror every stroke on the CURRENT frame across the stage's vertical centre line (the Draw panel's Mirror strokes button). Only points change. Errors on a frame with no strokes. One undo step. | — | — |
| `clear_frame_strokes` | Remove every stroke on the current frame (the stroke list's Clear button). 'frame' may only name the current frame — the op is current-frame-only and errors otherwise rather than clearing the wrong frame. One undo step when the frame carried strokes. | — | `frame` |
| `delete_all_strokes` | Delete every stroke in the project, on all frames (the stroke panel's Delete all). One undo step for the whole deletion. | — | — |
| `recolor_strokes` | Recolour strokes to 'color' (0xRRGGBB, required): scope 'frame' recolours the current frame's strokes (the panel's Frame Recolour), scope 'all' recolours every stroke in the project (the panel's Recolour all — except the panel's target colour is the selected figure's, while here it is explicit). One undo step when strokes match. | `color` | `scope` |
| `list_brushes` | List the brush preset library (the Draw panel's brush chips): index, name, width, nib, nib angle, smoothing, opacity and scatter per preset, plus the active preset's index and name. | — | — |
| `set_brush` | Select the active brush preset by 'name' (exact, then case-insensitive) or 'index' — the Draw panel's brush chips. Selection only; draws nothing. Subsequent interactive drawing, stamps and the eraser size use the selected preset. On the live transport this changes the editor's active brush. | — | `name`, `index` |
| `cycle_brush` | Cycle to the next brush preset in the library, wrapping (the Draw panel's brush-cycle button). Returns the new active preset. | — | — |
| `set_draw_tool` | Set the active draw tool: 'brush' or 'eraser' (the Draw panel's tool toggle, as an idempotent set). Affects subsequent interactive drawing and eraser sizing; draws nothing itself. | `tool` | — |
| `get_node_style` | Read a segment's complete style: colour override (null = inherits the figure colour), effective colour, gradient end colour, cap, taper and opacity. | `figure`, `node` | — |
| `cycle_node_taper` | Step a segment's taper one step (1.0 -> 0.6 -> 0.3 -> 0.0 -> 1.0), exactly the style panel's taper Step button. One undo step. | `figure`, `node` | — |
| `set_node_taper` | Set a segment's taper directly (0.0-1.0; 1.0 = uniform width, lower = sharper point). Values outside 0.0-1.0 are refused, not clamped. One undo step. | `figure`, `node`, `taper` | — |
| `set_node_style` | Write a segment's complete style (colour, gradient end, cap, opacity) in exactly one undo step — the write half of the style panel's copy/paste; pair with get_node_style to copy a style between segments. Omit 'color2' (or pass null) to clear the gradient. Taper is not part of this write; use set_node_taper. Opacity is clamped to 0.05-1.0 and read back as stored. | `figure`, `node`, `color`, `cap`, `opacity` | `color2` |
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
| `fx_preset_list` | List the FX clip presets saved in ./fx-presets/ by name, sorted — the read side of fx_preset_save / fx_preset_add (the panel's preset list). Read-only. | — | — |
| `particle_list_events` | List every particle event in the project: index, frame, stage position, kind, colour and anchor (figure/node) — the read the Particles panel rows show. Read-only. | — | — |
| `particle_delete_event` | Delete the particle event at 'index' (the Particles panel's Delete). Events after it shift down one index — re-read with particle_list_events. One undo step. | `index` | — |
| `particle_set_frame` | Move the particle event at 'index' to 0-based 'frame', clamped to the project's last frame (the panel's frame stepper and To playhead). The reply reports the frame the event actually landed on. One undo step. | `index`, `frame` | — |
| `particle_nudge_frame` | Move the particle event at 'index' by signed 'delta' frames, clamped at both ends of the timeline. The reply reports the frame the event actually landed on. One undo step. | `index`, `delta` | — |
| `particle_set_position` | Move the particle event at 'index' to the stage point (x, y) — the point its effect renders from, independent of its anchor (the panel's nudge pad writes this in 4-unit steps). One undo step. | `index`, `x`, `y` | — |
| `particle_set_anchor` | Anchor the particle event at 'index' to figure 'figure', joint 'node' (the panel's Anchor to figure / Anchor to joint), or clear the anchor back to a free stage event by passing 'figure' as null or omitting it (the panel's Clear anchor). The figure/node are validated before anything changes. One undo step. | `index` | `figure`, `node` |
| `particle_add_anchored` | Add a particle event anchored to the SELECTED figure's selected joint at the current frame (the panel's anchored add). 'kind' is a particle kind name (default burst). Refuses when the selection cannot anchor an event. One undo step. | — | `kind` |
| `draw_spray` | Append a spray-mark stroke on a frame (the Spray brush's stroke kind — draw_stroke cannot create one): dots are scattered around 'points' within 'radius'. 'frame' and 'points' are required; color (0xRRGGBB), width, radius, opacity and seed default from the active brush preset. One undo step. | `frame`, `points` | `color`, `width`, `radius`, `opacity`, `seed` |
| `get_stroke_style` | Read one stroke's style back by its project-wide index (see list_strokes): colour, width, per-point widths, fill, spray mark, opacity and scatter — the read half of set_stroke_style. Read-only. | `index` | — |
| `audio_list_clips` | List every audio clip with its stored fields (index, path, start frame, gain, muted) and probed health (healthy / missing / unsupported / unreadable, with duration) — the read the audio panel's rows show. Read-only. | — | — |
| `audio_clip_info` | Read one audio clip's full facts by 'index': stored fields, probed health, duration in seconds and frames, peak level, and waveform peaks at 'buckets' buckets (default 64; an empty peaks array means the file could not be decoded — see health). Read-only. | `index` | `buckets` |
| `audio_clip_fade` | Fade the audio clip at 'index' (0.25 s in/out, the panel's per-clip Fade) — the indexed form the panel has; the older audio_fade tool always edits the LAST clip. A clip whose file cannot be read is an error, not ok. One undo step. | `index` | — |
| `audio_clip_reverse` | Reverse the audio clip at 'index' — the indexed form of audio_reverse (which always edits the last clip). The processed file is written beside the source and the clip repointed. One undo step. | `index` | — |
| `audio_clip_set_gain` | Set the audio clip at 'index' gain (clamped to 0.0-2.0 by the op; the reply reports the gain read back after clamping, so a request for 5.0 truthfully reports 2.0) — the indexed form of audio_set_gain. One undo step. | `index`, `gain` | — |
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
| `graph_delete_key` | Delete the one key at 'frame' from the graph channel at 'channel' index. One undo step. | `channel`, `frame` | — |
| `graph_move_key` | Move the key nearest 'from_frame' (within 0.5 frames) in the channel at 'channel' index to 'to_frame' — the graph canvas drag: tangents travel with the key, its value travels unless 'value' overrides it, and a key already sitting at the destination is replaced. One undo step. | `channel`, `from_frame`, `to_frame` | `value` |
| `graph_set_tangents` | Set both tangents of the key at 'frame' in the channel at 'channel' index (in_tan / out_tan are the Hermite slopes the graph canvas drag writes). One undo step. | `channel`, `frame`, `in_tan`, `out_tan` | — |
| `graph_shift_keys` | Shift EVERY key of the channel at 'channel' index by 'delta' frames (negative = earlier) — the graph panel's shift buttons at full range. Refused whole (never clamped) when delta is 0 or any key would land before frame 0. One undo step. | `channel`, `delta` | — |
| `graph_fill_keys` | Fill the keyless frames of the channel at 'channel' index inside [from, to] with copies of the channel's key at the CURRENT frame (set the playhead first with set_frame). Returns the inserted count. One undo step when it inserts. | `channel`, `from`, `to` | — |
| `graph_delete_keys_in_range` | Delete the keys of the channel at 'channel' index whose frames fall inside [from, to], both ends inclusive. Returns the removed count. One undo step. | `channel`, `from`, `to` | — |
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
| `get_node` | Read one node in full: definition (name, parent, length, thickness, colour override, shape, head flag), the segment style (colour override + effective colour, color2, cap, taper, opacity), and pose facts at 'frame' (default: current) — the stored parent-relative angle in degrees, the solved world direction of the bone ending at this joint in degrees [0, 360) (null for a root node), and the frame's bend in radians and degrees. Note: get_pose returns stored parent-relative angles; the segment inspector shows this same world direction. Read-only. | `figure`, `node` | `frame` |
| `figure_library_list` | List the 2D figure library (definitions saved in ./library): index, name and node count per entry, plus the session's current library index. The library stores FigureDef only — an entry's colour is NOT stored; inserting assigns a palette colour. Read-only. | — | — |
| `figure_library_save` | Save a figure's definition to the 2D figure library under the figure's name (the library panel's Save). 'figure' defaults to the selected figure and selects it first. A file write, not an undo step. | — | `figure` |
| `figure_library_insert` | Insert a figure from the 2D figure library into the project (the library panel's Insert). Choose the entry with 'index' or 'name'; with neither, the session's current library index is used. The inserted figure gets a palette colour (definitions carry no colour) and becomes the selection. One undo step. | — | `index`, `name` |
| `figure_library_cycle` | Cycle the 2D figure library selection to the next entry (the library panel's cycle). Selection only — inserts nothing. Errors when the library is empty. | — | — |
| `fig3_set_camera` | Set the 3D session camera absolutely: any of 'distance' (clamped to 40..=4000), 'target' [x, y, z] and 'fov_deg'; omitted fields keep their values. At least one field is required. The absolute complement of fig3_orbit/fig3_camera_zoom. No undo step (session camera, like orbit/zoom). Requires an active 3D session. | — | `distance`, `target`, `fov_deg` |
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
| `project_rename` | Rename the current project's saved file to 'new_name' (a target that already exists is refused, leaving both files untouched). The session project's name follows and the file selector is repointed at the renamed file; the name stored inside the file updates on the next save. Errors when the current project has no saved file. Note there is deliberately no delete_project tool: no GUI surface deletes project files either. | `new_name` | — |
| `attach_image_to_node` | Attach a sprite image to the SELECTED joint of the selected figure (the segment inspector's Attach image). Selection-scoped: steer the selection with set_selection first. The op takes the first PNG found in ./import — there is no path argument, matching the GUI op exactly. One undo step (inside the op). | — | — |
| `clear_node_media` | Clear the media (sprite) attached to the SELECTED joint of the selected figure (the segment inspector's Clear media). Selection-scoped: steer the selection with set_selection first. One undo step. | — | — |

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
- **There is no `delete_project` tool — deliberately.** No GUI
  surface deletes project files either (the projects panel can
  open, save, copy, duplicate and rename, never delete), so MCP
  has parity by omission. Delete files with your file manager.
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
