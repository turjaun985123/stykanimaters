# MCP transport chapter — Wave 14 rewrite (D10)

> **What this file is.** The replacement transport chapter for
> `docs/MCP.md`, written by Wave 14 agent D10 for the integrator
> to merge. It does not edit `docs/MCP.md` itself. Every factual
> claim carries a source anchor in parentheses — a file, and a
> line or symbol where that helps. Sentences whose truth depends
> on a sibling agent's fix that had **not landed** when this
> chapter was surveyed are marked `[PENDING D#]` / `[PENDING B#]`;
> the integrator flips or deletes those markers at close-out,
> once the named work is in the tree.
>
> **Survey state (when written).** The tree was at the Wave 13
> close-out (`92c5d95`) plus early Wave 14 A-squad edits
> (`src/audio_ops.rs`, `src/fig3d.rs`, `src/fx_ops.rs`,
> `src/media.rs` modified). No `src/shell_control.rs` existed, no
> `docs/wave14/SHELL_CONTROL.md` existed, and no D-squad fix had
> landed (`git status` + `grep -rn "LAST_START_ERROR" src/` →
> zero hits). Everything below that describes today's transport
> is sourced from the code as surveyed; everything about the
> new shell-control channel is marked pending.

---

## Two transports, one router

StykAnimate-RS exposes the same MCP tool registry over two
transports (module docs, `src/mcp.rs:1-40`):

1. **stdio** — the standalone `mcp_server` binary, holding its
   own private editing session (`src/bin/mcp_server.rs:1-11`).
2. **Live control** — an HTTP endpoint hosted by the running
   editor itself, executing against the project in the window
   (`src/live_mcp.rs:1-27`).

Both transports hand each parsed message to the one shared
JSON-RPC router: `McpServer::handle` routes against the
server's own core, `McpServer::handle_on` routes against a
caller-supplied core, and both are thin delegates over the
private `dispatch` function, so the routing rules are identical
by construction (`src/mcp.rs:151-183`). The registry both serve
is `all_tools()` (`src/mcp.rs:65`); a tool behaves the same on
both transports — only the session it edits differs
(`src/live_mcp.rs:28-51`). The one systematic exception is the
shell-control tool family, which is live-only by design
(see "Shell tools" below) `[PENDING B1]`.

## Identity and version (both transports)

- Server name: `stykanimate-rs` (`initialize_result`,
  `src/mcp.rs:213-222`; also the `server_info` tool,
  `src/mcp.rs` `tool_server_info`).
- Server version: the Cargo package version, read at compile
  time via `env!("CARGO_PKG_VERSION")` (`src/mcp.rs:218`);
  the package version in `Cargo.toml` is `0.1.0`
  (`Cargo.toml:3`).
- Protocol version: `2024-11-05`, returned by `initialize`
  (`src/mcp.rs:215`).
- `initialize` capabilities: `{ "tools": {} }` — no other
  capability is advertised (`src/mcp.rs:216`).
- Supported methods: `initialize`, `ping`, `tools/list`,
  `tools/call` (`src/mcp.rs:202-211`). `ping` answers an empty
  result object (`src/mcp.rs:206`).
- `initialize` is **not** a gate: the router dispatches every
  method without checking whether `initialize` or
  `notifications/initialized` arrived first (`dispatch`,
  `src/mcp.rs:185-212`). `[PENDING D7]` — D7 re-verifies the
  stdio handshake against this shared code path; the parity
  claim itself follows from the shared `dispatch`, not from a
  second handshake implementation, because there is none.

## Transport 1 — stdio (`mcp_server` binary)

### How to run it

```sh
cargo build --release --bin mcp_server
./target/release/mcp_server        # speaks JSON-RPC on stdio
```

(The build command and the CI-artifact note are carried from
the existing chapter, `docs/MCP.md` "Transport 1"; the binary
names come from Cargo auto-discovery — there are no `[[bin]]`
sections in `Cargo.toml`, verified by `grep -n "bin"
Cargo.toml` returning no sections; `src/bin/mcp_server.rs`
builds as `mcp_server` and `src/main.rs` as `stykanimate-rs`.)

Client configuration (carried from `docs/MCP.md` "Connect a
client"):

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

### Framing and session

- One JSON object per line on stdin; one response line per
  request on stdout, flushed immediately; blank lines are
  skipped; nothing but protocol messages goes to stdout
  (`src/bin/mcp_server.rs:1-11`, `run` and `write_response`).
- The process serves until stdin EOF, and a final line with no
  trailing newline is still processed before exit
  (`read_line_capped`, `src/bin/mcp_server.rs`; pinned by its
  `eof_mid_line_still_processes_the_final_line` test).
- The server never speaks first: a client that connects and
  sends nothing gets silence until EOF (module docs,
  `src/bin/mcp_server.rs:38-40`).
- The server holds **one `EditorCore` session in memory for
  the life of the process**, starting from the built-in demo
  project (`McpServer::new`, `src/mcp.rs:145-149`;
  `EditorCore::new`). This session is **private to the
  binary**: it is not the GUI's session — edits made over
  stdio never reach an editor window, and the editor's
  unsaved work is invisible to it (module docs,
  `src/live_mcp.rs:3-10`; `docs/MCP.md` "Transport 1").
  Nothing is written to disk unless a save/export/render
  tool is called (tool contracts, `docs/MCP.md` catalogue).
- Robustness of the framing (Wave 11 pass, module docs
  `src/bin/mcp_server.rs:14-40`):
  - a malformed line or invalid UTF-8 is answered `-32700`
    and the server keeps serving — lines are read as raw
    bytes and decoded lossily, so bad UTF-8 degrades to a
    parse error instead of ending the session;
  - a line longer than 32 MiB (`MAX_LINE_BYTES`,
    `src/bin/mcp_server.rs`) is consumed through its
    newline, answered `-32600`, and never buffered whole;
    the stream stays in sync for the next line;
  - a panic inside dispatch is caught (`catch_unwind` in
    `run`) and answered `-32603` naming the request id
    (`panic_response`, `src/bin/mcp_server.rs`) — see the
    error table for how this differs from a handler panic.
  `[PENDING D5]` — D5's final robustness audit (wrong-typed
  arguments, huge payloads, unknown methods) may add cases
  to this list; the three bullets above are in the tree as
  surveyed.

## Transport 2 — live control (HTTP, hosted by the editor)

### Endpoint

```
POST http://127.0.0.1:32117/mcp
```

The port is fixed: `pub const PORT: u16 = 32117`
(`src/live_mcp.rs`), and `endpoint()` formats exactly the URL
above (`endpoint()`, `src/live_mcp.rs`; pinned by the
`endpoint_names_the_loopback_port` test). The listener binds
`127.0.0.1` only (`LiveBridge::start`,
`src/live_mcp.rs` — `TcpListener::bind(("127.0.0.1", PORT))`).

### How to run it

1. Run the editor (`stykanimate-rs`).
2. Live control starts with the editor when the persisted
   preference says so: `LayoutPrefs.live_mcp` **defaults to
   on**, including for prefs files written before the field
   existed (serde default `default_true`, `src/shell.rs`
   `LayoutPrefs`; `Default` impl sets `live_mcp: true`), and
   the shell's `init` attempts the start at launch
   (`src/main.rs` `init`, the `if layout.live_mcp` block).
3. The runtime switch is the Settings card's **Live MCP
   control** toggle (element key `settings-live-mcp`,
   `src/rune_sec/settings_popover.rs` builder): one press
   starts or stops the listener immediately, no restart
   (settings_popover module docs). The command palette has
   the same action as `toggle-live-mcp`
   (`src/rune_sec/command_palette.rs:1455`).
4. The toggle's state reads the bridge itself
   (`live_mcp::is_enabled`) — there is deliberately no
   mirrored boolean in shell state to drift from it
   (`toggle_live_mcp`, `src/rune_sec/settings_popover.rs`).
   A press persists the **resulting** state — not the wish —
   to `LayoutPrefs.live_mcp`, so a failed start is never
   remembered as on (`toggle_live_mcp`).
5. The note under the toggle always shows the endpoint URL
   plus the no-password warning, in both states
   (`live_mcp_note()`, `src/rune_sec/settings_popover.rs`).

Client configuration (carried from `docs/MCP.md` "Connect a
client"):

```json
{
  "mcpServers": {
    "stykanimate-live": {
      "url": "http://127.0.0.1:32117/mcp"
    }
  }
}
```

`curl` is the ground truth for debugging, because the
transport is one POST per JSON-RPC message with the response
as the body:

```sh
curl -s http://127.0.0.1:32117/health
curl -s -X POST http://127.0.0.1:32117/mcp \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'
```

### How a request actually flows

- The listener thread never touches the editor. It parses the
  request, drops a `LiveRequest` into a bounded queue, and
  **blocks on that request's reply channel** (module docs,
  "Synchronisation", `src/live_mcp.rs`; `LiveRequest` struct).
- The editor's poll loop drains the queue on the UI thread:
  `live_mcp::drain(&mut s.core, &mut router, 8)` — **up to 8
  requests per poll tick** (`src/main.rs` `poll`, the
  `live_mcp::is_enabled()` block; `drain`,
  `src/live_mcp.rs`). Routing is
  `router.handle_on(core, &msg)` — the `McpServer` is only a
  dispatcher; the tools execute against the live core, never
  the router's own core (`drain` docs).
- So a reply arrives on the app's poll cadence — one poll
  tick plus the tool's own runtime, never sub-frame
  immediacy — and a long tool (a render, an export) holds
  its HTTP connection open until it finishes (module docs,
  `src/live_mcp.rs`). While the bridge is on, the shell also
  keeps a 100 ms poll heartbeat so a queued request drains
  even when the window is otherwise idle
  (`MCP_POLL_HEARTBEAT`, `src/main.rs:95`; `tx.wake_after` in
  `poll`), and the listener calls an event-loop waker when a
  request lands (`set_waker` / `wake`, `src/live_mcp.rs`).
  **Do not impose a short client timeout.**
- Connections are accepted and served **sequentially**, one
  request per connection, and every response carries
  `Connection: close` (`listener_loop` and `write_response`,
  `src/live_mcp.rs`). There is no session id, no SSE stream
  and no server-initiated traffic (the request handler serves
  exactly one request, then returns; `handle_connection`).
- Because requests run against the live core, the stage,
  timeline and panels react as the client works, playback
  tools start real playback with audio, and edits land in
  the editor's own undo history (`src/live_mcp.rs:1-27`;
  `docs/MCP.md` "Transport 2"). A drained request that swaps
  the project (an MCP load/import) also clears shell-side
  selection state that would otherwise point into the old
  project (`src/main.rs` `poll`, the `project_swaps` watch).
- Errors a drained request produces are surfaced to the user
  two ways, by one uniform rule that takes no tool name as
  input (`drain_attribution_error`, `src/live_mcp.rs`): a
  refusal that wrote `core.status` is toasted by the shell
  from the status line (with the `MCP: ` attribution prefix,
  `src/main.rs` `poll`); a protocol error or a status-silent
  refusal is buffered by the drain and toasted from
  `live_mcp::take_drain_errors()`.

## Health and identity probes (live transport only)

`GET /` and `GET /health` answer the same small JSON
document, with HTTP 200 and no queueing — no core access is
needed (`handle_connection` GET branch, `src/live_mcp.rs`).
`GET` to any other path is 404, as is a `POST` to any path
but `/mcp` (same function). The stdio server has no HTTP
listener at all, so a health answer also proves you are
talking to the live editor (module docs, `src/live_mcp.rs`).

The document is built by `health_body()` (`src/live_mcp.rs`)
and contains exactly these fields:

| Field | Value / meaning |
|---|---|
| `server` | `"stykanimate-rs"` — the same name `initialize` reports in `serverInfo`. |
| `version` | The Cargo package version (`env!("CARGO_PKG_VERSION")`) — the same version `initialize` reports. |
| `transport` | `"live-http"` — names this transport; the stdio transport serves no such document. |
| `live` | `true` — the endpoint is hosted by the running editor and executes against its live core. |
| `endpoint` | `"http://127.0.0.1:32117/mcp"` — the URL `POST` requests go to (from `endpoint()`). |
| `auth` | `"none"` — the v1 authentication posture, stated plainly (see "Auth posture"). |
| `tools` | The registered tool count, read fresh from `all_tools().len()` on every probe — the same number the `server_info` tool reports, from the same registry (`health_body`; module docs "Wave 13 audit", verdict 2). There is no count literal in the module, so the number cannot drift from the served registry; it is pinned against the router's own `tools/list` by the `health_tool_count_is_the_served_tool_count` test. |

`[PENDING D4]` — D4 audits health completeness/truth, and
if Squad B's shell-control channel lands, the health
document is expected to gain a shell-control awareness
flag; as surveyed, the seven fields above are the whole
document — do not read a shell capability into it yet.
`[PENDING B1]`

## Auth posture — none, loopback only (plain words)

Live control has **no authentication in v1**. There is no
password, token or pairing step (module docs, "Security",
`src/live_mcp.rs`; `health_body`'s `"auth": "none"` field).
While the toggle is on, **any program running as your user
on this computer can call every MCP tool through the
endpoint** — edit the project, save and load files, export,
delete. The listener binds `127.0.0.1` only, so nothing off
the machine can reach it; local programs all can (same
source). The settings card says the same thing in plain
words under the toggle: "while on, any program on this
computer can drive the editor. No password in this version."
(`live_mcp_note()`, `src/rune_sec/settings_popover.rs`).
Enable it only while you actually want an agent driving the
editor, and turn it off when you are done.

The stdio transport has no listener and no auth concept of
its own: whoever can launch the binary and write to its
stdin is its client (framing, `src/bin/mcp_server.rs`).

`[PENDING B1]` — Squad B's design keeps this exact posture
for the new shell-control channel (same endpoint, same
loopback bind, no new credential), per the Wave 14 roster's
B1 remit; the design document itself had not landed when
this chapter was surveyed.

## Queue, backpressure and timeouts (live transport)

All values below are constants or call sites in
`src/live_mcp.rs` unless another file is named.
`[PENDING D3]` — D3's audit re-verifies this section against the
source and fills any gap it finds; as surveyed, the
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
| Head cap | Request head at most **16 KiB** (`MAX_HEAD`); larger is HTTP 400 ("headers too large"). |
| Body cap | Request body at most **4 MiB** (`MAX_BODY`); larger is **HTTP 413** whose body is a JSON-RPC error object (code `-32600`, id `null`) — see the error table. Responses are not capped. |
| Listener resilience | One connection can never kill the listener thread: connection handling is wrapped in `catch_unwind`, and a panicked serve drops that connection and accepts the next (`listener_loop`). |

The stdio transport's corresponding bound is its line cap:
32 MiB per line (`MAX_LINE_BYTES`,
`src/bin/mcp_server.rs`), answered `-32600` when exceeded;
there is no queue and no reply timeout — requests are
served one at a time, in arrival order, by the single
serving loop (`run`, `src/bin/mcp_server.rs`).
`[PENDING D2]` — D2's parity harness runs one identical scripted
sequence against both transports and diffs the results;
its verdict had not landed when this chapter was surveyed.

## Batch and notification semantics (both transports)

`[PENDING D6]` — D6's final audit pins this section against
the JSON-RPC specification on both transports; the
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

## Error codes

`[PENDING D5]` — D5 owns the final robustness pass over
this table (malformed JSON, wrong types, huge payloads,
unknown methods → clean protocol errors, zero panics). The
table below is complete against the source as surveyed.

### JSON-RPC protocol errors (the `error` object in the reply)

| Code | Name / when | Source |
|---|---|---|
| `-32700` | Parse error — the message body is not valid JSON. Id is `null`. Both transports build the identical body ("Parse error: …"). | `parse_error_body`, `src/live_mcp.rs`; the parse branch of `run`, `src/bin/mcp_server.rs` |
| `-32600` | Invalid Request — the message is not a JSON object (this includes batch arrays), or the `method` member is missing. Id is `null` for a non-object, the request's id otherwise. | `dispatch`, `src/mcp.rs:185-200` |
| `-32600` | Invalid Request — a stdio line over the 32 MiB cap ("line exceeds the …-byte limit", id `null`), or a live request body over the 4 MiB cap (delivered as HTTP 413, id `null`). | `run`, `src/bin/mcp_server.rs`; `body_too_large_body`, `src/live_mcp.rs` |
| `-32601` | Method not found — any method other than `initialize`, `ping`, `tools/list`, `tools/call` ("Method not found: {method}"). | `dispatch`, `src/mcp.rs:208-211` |
| `-32602` | Invalid params — a `tools/call` with no `params` object, or with no tool `name`. | `tools_call_result`, `src/mcp.rs:244-248` |
| `-32602` | Unknown tool — a `tools/call` naming a tool that is not registered. The message is exactly `unknown tool: {name}`. This is the one call failure that never reaches a handler, so it is a protocol error, not a tool result (standing contract). | `tools_call_result`, `src/mcp.rs:253-263` |
| `-32603` | Internal error — **stdio shell only**: a panic escaping dispatch is caught by the binary and answered `-32603` naming the request id ("Internal error: tool dispatch panicked: …"). | `panic_response` + `catch_unwind` in `run`, `src/bin/mcp_server.rs` |

### Tool failures are not protocol errors

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

### HTTP status codes (live transport only)

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

`[PENDING D8]` — D8 audits the connection-lifecycle rows
above (disconnect mid-request, partial writes, slow
clients) against the implementation; the rows are sourced
from `handle_connection` as surveyed, and its early-return
contract is "every early return is a client or transport
failure that has already been answered (or needs no
answer); this function never panics on bad input".

## Shell tools — the live-only rule

**The rule (Squad B's design):** tools that drive the GUI
shell itself — workspace, playback transport as the user
sees it, view, shell selection, theme, panels — execute
**only over live control**. Over stdio they refuse honestly
rather than pretending to affect a window that is not
there. `[PENDING B1]` — the design document
(`docs/wave14/SHELL_CONTROL.md`: command enum, request
queue, drain in the main poll loop, result return path)
had not landed when this chapter was surveyed, and neither
had the plumbing (`src/shell_control.rs`) `[PENDING B2]`,
so no shell-tool names, arguments or refusal wording are
stated here — inventing them would be exactly the claim
without a source this chapter exists to avoid.

What is true today, from the source:

- The live drain hands the router an `EditorCore` and
  nothing else: `drain(core, router, max)`
  (`src/live_mcp.rs`) and its one call site
  (`src/main.rs` `poll`) pass `&mut s.core` — the shell's
  wider `State` (workspace, panels, theme, view) is not
  reachable from any tool today, on either transport.
- Tools that *look* like shell control in the current
  registry act on the **core**, not the shell chrome:
  `set_playback` flips the same playing flag the chrome's
  Play/Pause button flips, and over live control the GUI's
  poll observes that flag — which is why it works live —
  while over stdio it starts playback only inside the
  binary's private session (built-in `set_playback`,
  `src/mcp.rs`; catalogue row, `docs/MCP.md`). The same
  applies to `get_editor_status`, `get_selection` and
  `set_selection`: core state, readable/writable on both
  transports, against whichever session the transport owns
  (built-ins, `src/mcp.rs`).
- When the shell channel lands, its commands will be
  drained in the same poll loop as core requests, and the
  live-only refusal on stdio is the designed behaviour —
  the integrator flips this whole section to the landed
  names and wording at close-out. `[PENDING B1]`
  `[PENDING B3]` `[PENDING B4]` `[PENDING B5]`
  `[PENDING B6]` `[PENDING B7]` `[PENDING B8]`
  `[PENDING B9]`

## Troubleshooting

- **The editor is running but nothing answers on the
  port.** Check the Settings card's Live MCP toggle: its
  state is the bridge's real state, not the preference
  (`live_mcp::is_enabled`). If the persisted preference was
  off, the bridge never started at launch; flip the toggle
  on — the start is immediate, no restart
  (`src/rune_sec/settings_popover.rs`, Live MCP section).
- **Port busy — "Live MCP did not start".** Another
  program — in practice, another editor instance — already
  holds port 32117, so the bind failed and `set_enabled`
  reported `false`, never a phantom on (`set_enabled`,
  `src/live_mcp.rs`). What you can see today: the toggle
  sits off; the Settings card shows a persistent note —
  "Live MCP did not start — port 32117 may be in use. Turn
  it off and on to retry." (settings_popover builder,
  reading `shell::live_mcp_start_outcome()`); and pressing
  the toggle while the port is still held puts "Live MCP
  could not start — port 32117 is in use (another editor
  instance?)." on the status line (`toggle_live_mcp`). The
  start record itself is session state, deliberately not
  persisted (`LiveMcpStartOutcome` docs, `src/shell.rs`).
  Fix: close the other instance (or whatever holds the
  port), then toggle off and on. `[PENDING D1]` — D1 gives
  the bridge its own memory of the failure
  (`LAST_START_ERROR` / `last_start_error()` in
  `live_mcp.rs`), so the settings note can quote the actual
  bind error instead of the shell-side outcome record
  above; as surveyed, the bridge still swallows the bind
  error at the start call (`LiveMcpStartOutcome` docs:
  "the bridge module's start API reports failure as a bare
  `false` and keeps no reason — the bind error is
  swallowed"). `[PENDING D9]` — D9 pins the final settings
  wording in tests.
- **You turned it off and a client call hung, then the
  connection closed with no response.** That is the honest
  outcome for a bridge switched off mid-request: the
  queued request is cancelled (so it cannot be applied
  later, out of context) and the connection closes
  (`LiveBridge::stop`; wait loop in `handle_connection`).
  Turn the toggle back on and re-send.
- **A call got HTTP 504 after ten minutes.** The editor
  never drained the request (wedged, or stuck in a long
  modal operation). The request was marked cancelled and
  will be discarded, not executed late — but if execution
  had already begun, the edit may still have landed; only
  the reply is lost (see the queue table). Check the
  project state (e.g. `get_state`) before retrying a
  mutation.
- **A call got HTTP 503.** Eight requests are already
  waiting for the UI thread. Wait and retry; do not open
  parallel connections to get ahead — connections are
  served sequentially anyway.
- **The toggle is on but the editor restarted and the
  bridge is off.** The preference lives in
  **`./projects/.layout.json`**, field **`live_mcp`**
  (`LAYOUT_FILE`, `load_layout` / `save_layout`,
  `src/shell.rs` — the path is relative to the editor's
  working directory). A failed start persists the
  *resulting* state, so after a port-busy launch the file
  may legitimately say `false` even though you never
  toggled it off — flip the toggle on once the port is
  free and the file follows (`toggle_live_mcp`). A prefs
  file with no `live_mcp` field loads as **on** (serde
  default); a corrupt file is salvaged field-by-field
  rather than discarded (`load_layout_in`,
  `src/shell.rs`).
- **`curl` to `/health` answers, but your client cannot
  connect.** You are talking to the live editor (stdio has
  no HTTP listener). Point the client at the `endpoint`
  value from the health document, with `POST` and
  `Content-Type: application/json`; a `POST` to any other
  path is 404 and any `Transfer-Encoding: chunked` client
  is refused 501 (error table above).
- **stdio works but the user sees no change in their
  window.** Expected: the stdio server edits its own
  private session. Use live control to drive the project
  the user is looking at (module docs, `src/live_mcp.rs`).
