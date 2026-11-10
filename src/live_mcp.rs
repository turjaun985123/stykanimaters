//! Live MCP control: drive the RUNNING editor from an MCP client.
//!
//! The standalone `mcp_server` binary owns a private
//! [`EditorCore`](stykanimate_rs::editor::EditorCore) — its edits
//! never reach the editor window the user is looking at, and the
//! user's live session (unsaved work, playhead, selection) is
//! invisible to it. This module closes that gap: the editor app
//! itself hosts a small MCP endpoint, and every request executes
//! against the app's own live core, on the UI thread, so the
//! stage, timeline and panels react as the client works.
//!
//! ## Transport
//!
//! Minimal HTTP/1.1 over TCP, loopback only: one `POST /mcp` with
//! a JSON-RPC 2.0 body gets one JSON-RPC response. The tool layer
//! is unchanged — requests are routed by
//! `stykanimate_rs::mcp::McpServer::handle_on` (the same dispatch
//! the stdio shell uses, parameterised over an external core).
//! Implemented with `std` only: connections are accepted and
//! served **sequentially** on one background thread, request
//! bodies are capped at 4 MiB, and `Transfer-Encoding: chunked`
//! is not supported (MCP clients send `Content-Length`).
//!
//! Two unauthenticated probes exist so a client can tell this
//! live endpoint apart from the stdio server (which has no HTTP
//! at all): `GET /` and `GET /health` answer a small JSON
//! identity document (server name/version, `"live": true`, the
//! bound port and enabled flag, and the registered tool count
//! — the same number the `server_info` tool reports, read from
//! the same registry). The document is built on the listener
//! thread from constants, atomics and the static registry
//! alone — no core, no queue, no bridge lock — so it answers
//! even while the app's drain is busy (see [`health_body`]).
//! The JSON-RPC `ping` method also works — it is answered by
//! the shared router, like every other method.
//!
//! Method parity with the stdio shell is by construction:
//! both transports hand the parsed message to the one
//! `McpServer` dispatch, so `initialize`, `ping`,
//! `tools/list`, `tools/call`, unknown methods (`-32601`),
//! unroutable requests (`-32600`), bad `tools/call` params
//! (`-32602`) and parse failures (`-32700`) produce identical
//! bodies. Two shapes deserve naming:
//!
//! - **Batch arrays** are not unrolled by the shared dispatch
//!   (it expects one object), so a batch gets the same single
//!   `-32600` error over HTTP as over stdio — an honest
//!   protocol error, never a hang or a dropped connection.
//! - **Notifications** (a JSON object with no `id` member)
//!   are answered `202 Accepted` with an empty body
//!   *immediately*, without queueing: dispatch checks for an
//!   `id` before it routes any method, so a notification is
//!   guaranteed to execute nothing and produce no response —
//!   the fast path is behaviour-identical, minus a pointless
//!   wait on the UI thread (and a pointless 504 risk if the
//!   app is wedged).
//!
//! ## Framing discipline (Wave 10 hardening)
//!
//! Malformed input is answered, never panicked on and never
//! waited on forever:
//!
//! - The whole request (head + body) must arrive within
//!   [`READ_DEADLINE`] of the accept — a per-read timeout alone
//!   would let a dribbling client hold the one listener thread
//!   indefinitely. Expiry is a clean close.
//! - A `Content-Length` header that is present but not a plain
//!   number, or duplicated with differing values, is HTTP 400.
//! - A `Content-Type` header that is present but is not
//!   `application/json` (parameters like `; charset=utf-8` are
//!   fine) is HTTP 415. An absent header is tolerated — the
//!   minimal hand-rolled clients this endpoint also serves do
//!   not always send one, and the body is parsed as JSON
//!   either way, so leniency costs nothing.
//! - `Transfer-Encoding` is refused with HTTP 501 rather than
//!   being silently misread as an empty body.
//! - An oversized body is refused with HTTP 413 whose body is
//!   a JSON-RPC error object (code `-32600`, id `null`) — the
//!   spec-shaped way to reject a request whose id was never
//!   read.
//! - A queued request whose reply never comes (the app wedged
//!   before draining) is given up on after [`REPLY_TIMEOUT`]
//!   with HTTP 504 and marked cancelled; if the app later
//!   drains it, the request is discarded rather than applied
//!   to whatever project happens to be current by then. A
//!   request already executing when the timeout fires cannot
//!   be recalled.
//! - Responses are written under a total [`WRITE_DEADLINE`],
//!   for the same reason reads have one: a per-write timeout
//!   alone lets a client that drains a byte at a time hold
//!   the one listener thread for as long as a response is
//!   large. Expiry mid-response is a plain close.
//! - A client that disconnects *after* its request has been
//!   queued does **not** cancel it. A disconnect is not a
//!   cancellation signal this transport can trust — it is
//!   indistinguishable from the half-close (`shutdown(Write)`
//!   once the request is sent) that simple clients use while
//!   still reading the reply — so the request executes and
//!   the dead reply write is silently dropped (see [`drain`]).
//!   Cancellation exists only where the client can be told
//!   what happened: the reply timeout (answered 504) and the
//!   bridge stop flag. A disconnect *before* the request is
//!   fully read is different — nothing was ever queued, so
//!   nothing executes.
//!
//! ## Synchronisation
//!
//! The listener thread never touches the editor core. It parses
//! the request, drops a [`LiveRequest`] into the queue and
//! **blocks on that request's reply channel**. The app's poll
//! loop calls [`drain`], which runs each queued request against
//! the live core and sends the response back. Replies therefore
//! arrive on the app's poll cadence — a client sees latency of
//! one poll tick plus the tool's own runtime, and a long tool
//! call (a render, say) holds its HTTP connection open until it
//! finishes. That is deliberate: tools must run on the UI thread
//! because that is where the live `State` lives.
//!
//! ## Wave 13 audit — sprite tools + dirty-seam write paths
//!
//! Wave 13 added five sprite tools to the registry (A4's
//! handlers in `media.rs`, spliced into `mcp_tools.rs` by
//! the integrator) and changed many write paths' undo /
//! dirty behaviour (Squad B's `mark_dirty()` seam). This
//! module was re-audited against both changes; nothing in
//! the transport needed to change. The three verdicts:
//!
//! 1. **Drain parity — SOUND.** [`drain`] never inspects
//!    the request message: it applies the cancellation
//!    gate, routes the message whole through
//!    `handle_on` against the live core, and attributes
//!    errors. A sprite mutation therefore lands on the
//!    app's own undo stack exactly like any other
//!    mutation (the one-snapshot-per-op discipline is the
//!    handler/op's contract, not this module's), and its
//!    errors surface by the one uniform rule in
//!    [`drain_attribution_error`]: a refusal that wrote
//!    `core.status` is toasted by the shell from the
//!    status line; a status-silent refusal or a protocol
//!    error is buffered for the `MCP: ` toast. The
//!    decision takes no tool name as input, so no family
//!    — sprites included — can drift from another. The
//!    Squad B seam changes WHICH core methods mark dirty,
//!    never how a request reaches the core.
//! 2. **Health count — DERIVED, follows automatically.**
//!    [`health_body`] reads
//!    `stykanimate_rs::mcp::all_tools().len()` fresh on
//!    every probe request; there is no count literal
//!    anywhere in this module. The sprite splice takes
//!    the registry 230 → 235 and the total 283 → 288, and
//!    the health document — and `server_info`, which
//!    reads the same source — report 288 with no change
//!    here. Pinned by
//!    `health_tool_count_is_the_served_tool_count`, which
//!    compares the health number against the `tools/list`
//!    the router actually serves, never against a literal.
//! 3. **Queue fairness — SOUND, with the settled bounds
//!    named.** The listener serves connections strictly
//!    sequentially; the queue is a FIFO `sync_channel`;
//!    [`drain`] processes strictly in receive order up to
//!    the shell's per-poll batch and cannot prioritise by
//!    tool — it never reads the message. A burst of sprite
//!    calls is therefore served in arrival order like any
//!    other burst. Two honest bounds, both Wave 10 / E6
//!    settlements rather than defects: a request whose
//!    tool runs long holds the one listener for its
//!    duration (true of a render today, and sprite ops
//!    are cheap core calls that do not worsen it), and a
//!    queue full of cancelled leftovers costs at most one
//!    drain batch of O(1) discards before live requests
//!    proceed.
//!
//! ## Security — read this before enabling
//!
//! The listener binds `127.0.0.1` only, so nothing off this
//! machine can reach it. On top of the bind, every request
//! must pass the Host/Origin gate ([`host_origin_allowed`]):
//! a loopback `Host` (`localhost` / `127.0.0.1` / `[::1]`)
//! and, when an `Origin` is sent, a loopback origin —
//! anything else is refused HTTP 403 before routing or
//! dispatch. That gate is what stops a *web page* from
//! driving the editor: a browser page on a foreign origin
//! either sends its own origin (refused) or, under DNS
//! rebinding, its own host name (also refused). The gate is
//! header **validation, not authentication**: any program
//! on this machine can simply send the loopback headers,
//! so it changes nothing about the warning below.
//! **There is NO authentication in v1:**
//! while live control is enabled, *any program running as your
//! user on this machine* can call every MCP tool — edit the
//! project, save and load files, export — through this endpoint.
//! Enable it only while you actually want an agent driving the
//! editor, and turn it off when you are done.
//!
//! ## Glue the integrator adds in `main.rs`
//!
//! This module is inert until wired (the integrator owns
//! `main.rs`; nothing here edits it):
//!
//! ```ignore
//! // 1. Declare the module with the other bin-crate modules:
//! mod live_mcp;
//!
//! // 2. Own a router in State (or a thread-local): a McpServer
//! //    used purely as a dispatcher — its own core stays unused
//! //    because handle_on takes the live core:
//! //        mcp_router: stykanimate_rs::mcp::McpServer,
//!
//! // 3. In poll(), after the usual playback/autosave work:
//! if live_mcp::is_enabled() {
//!     let n = live_mcp::drain(&mut s.core, &mut s.mcp_router, 4);
//!     if n > 0 {
//!         // invalidate / refresh bookkeeping, like any edit
//!     }
//! }
//!
//! // 4. From the settings toggle (persist the choice with the
//! //    layout prefs if desired):
//! let on = live_mcp::set_enabled(want_on);
//! // Show `live_mcp::endpoint()` in the toggle's note so the
//! // user can point their MCP client at it.
//! ```

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::mcp::McpServer;

/// The fixed loopback port the live endpoint listens on.
pub const PORT: u16 = 32117;

/// Serialises the tests that start, stop or port-hold
/// the singleton bridge — this module's two socket
/// tests and the Settings snap test
/// (`settings_live_mcp_toggle_shows_the_persisted_state_for_both_prefs_values`).
/// Cargo runs tests on parallel threads, and two bridge
/// tests racing present exactly like a busy port or a
/// vanished bridge (Wave 14 CI fix 6: the e2e's start
/// failed against the start-error test's occupier
/// listener). Hold the guard for the whole test.
#[cfg(test)]
pub static BRIDGE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Largest request body accepted: 4 MiB. Bigger bodies are
/// refused with HTTP 413 before a byte of them is read.
///
/// Why 4 MiB is the right size: a request is one JSON-RPC
/// message — a method name plus a small arguments object
/// (names, frame numbers, toggles). Project files are never
/// carried inline: the save/load/export tools take a `path`
/// and the file moves over the filesystem, not the wire. The
/// largest plausible inline payloads (a pasted pose or scene
/// fragment) are KiB-scale, so 4 MiB is orders of magnitude of
/// headroom — while still bounding the memory one connection
/// can make the listener allocate (connections are served
/// sequentially, so one buffer is all that is ever alive).
/// The cap is request-side only: responses are not capped,
/// because tool results (project summaries, frame data) can
/// legitimately be large.
const MAX_BODY: usize = 4 * 1024 * 1024;

/// Largest request head (request line + headers) accepted.
const MAX_HEAD: usize = 16 * 1024;

/// Maximum parsed requests waiting for the UI thread. This
/// matches the shell's current per-poll drain batch, so a
/// healthy editor can clear a full queue in one poll. The
/// listener normally has only one request in flight because
/// it blocks on that request's reply; the bound matters when
/// earlier requests timed out and were left cancelled in the
/// queue — without it, a wedged app could accumulate request
/// bodies (each up to [`MAX_BODY`]) without limit.
const MAX_QUEUED_REQUESTS: usize = 8;

/// How long a single socket read or write may block before
/// the listener gives up on it — the listener serves
/// connections one at a time, so a stuck client must not
/// starve every later one.
const IO_TIMEOUT: Duration = Duration::from_secs(30);

/// Total budget for writing one response, head and body
/// together. [`IO_TIMEOUT`] alone bounds each individual
/// write syscall, so a client that drains a byte at a time
/// — always inside the per-write timeout — could hold the
/// one listener thread for as long as the response is
/// large: the write-side twin of the slow-dribble read
/// [`READ_DEADLINE`] exists to close. Responses travel over
/// loopback to a client that asked for them, so 60 s is
/// generous for any realistic result size; expiry mid-
/// response is a plain close (the client is left with a
/// truncated body under an unsatisfied `Content-Length`,
/// which no HTTP client mistakes for a complete reply).
const WRITE_DEADLINE: Duration = Duration::from_secs(60);

/// Total budget for one request to arrive, from accept to
/// the last body byte. [`IO_TIMEOUT`] alone bounds each
/// individual read, so a client sending one byte every few
/// seconds would never trip it; this deadline is what
/// actually closes a slow-dribble (slow-loris) connection.
/// Expiry is a clean close with no response — the request
/// never became a request, so there is nothing to answer.
const READ_DEADLINE: Duration = Duration::from_secs(30);

/// How long a connection may wait for the UI thread to
/// answer a queued request before the listener gives up and
/// answers HTTP 504. Generous on purpose: the wait IS the
/// tool's runtime (a full export can take minutes), and this
/// bound exists only so a wedged app — one whose poll loop
/// never drains again — cannot hold the listener forever.
/// At the timeout the listener marks the request cancelled:
/// if the app recovers and reaches it in the queue, `drain`
/// discards it instead of applying a stale edit to whatever
/// project is current then. A request already executing
/// when the timeout fires cannot be recalled.
const REPLY_TIMEOUT: Duration = Duration::from_secs(600);

/// The URL an MCP client should be pointed at while live
/// control is enabled: `http://127.0.0.1:32117/mcp`.
pub fn endpoint() -> String {
    format!("http://127.0.0.1:{PORT}/mcp")
}

/// One parsed JSON-RPC message on its way to the UI thread,
/// with the channel its response must be sent back on.
/// `Some(value)` is the JSON-RPC response; `None` means the
/// message was a notification and gets no body (HTTP 202).
/// HTTP notifications are in fact answered by the listener
/// before queueing (see `handle_connection`), so a `None`
/// reaching the wait loop is only a defensive fallback —
/// the drain contract keeps it because `handle_on` can
/// still return `None` for a notification handed to it by
/// any other caller.
pub struct LiveRequest {
    pub msg: Value,
    pub reply: SyncSender<Option<Value>>,
    /// Set by the listener when it stops waiting for the
    /// reply (reply timeout, or the bridge is switched off).
    /// `drain` checks this before routing: a cancelled
    /// request must not execute later against a different
    /// project than the one its client was talking to.
    pub cancelled: Arc<AtomicBool>,
}

/// Discard a request the listener has already given up on,
/// returning the request unchanged when it is still live.
/// A discarded request gets the notification-shaped `None`
/// reply defensively; in practice its listener has already
/// left the wait loop (that is why it was cancelled), so the
/// send simply lands in the buffered reply channel and is
/// dropped with it.
fn discard_if_cancelled(req: LiveRequest) -> Option<LiveRequest> {
    if req.cancelled.load(Ordering::SeqCst) {
        let _ = req.reply.send(None);
        None
    } else {
        Some(req)
    }
}

/// The live bridge: the request queue the UI thread drains,
/// plus the listener thread that fills it.
///
/// Dropping the bridge stops it (see [`LiveBridge::stop`]).
pub struct LiveBridge {
    /// Queued requests from the listener thread. Only
    /// [`drain`] reads from this.
    requests: Receiver<LiveRequest>,
    /// Set to tell the listener thread to wind down: it checks
    /// the flag between accepts and while waiting for a reply.
    stop: Arc<AtomicBool>,
    /// The listener thread, joined on stop where feasible.
    thread: Option<JoinHandle<()>>,
}

impl LiveBridge {
    /// Bind the loopback listener and spawn the listener
    /// thread. Fails with the bind error (e.g. `AddrInUse`
    /// when another editor instance already holds the port) —
    /// no thread is spawned in that case.
    pub fn start() -> std::io::Result<LiveBridge> {
        let listener = TcpListener::bind(("127.0.0.1", PORT))?;
        // Non-blocking accept: the loop polls the stop flag
        // between accepts instead of parking inside accept()
        // forever, which is what makes a clean join possible.
        listener.set_nonblocking(true)?;
        let (tx, rx) = sync_channel::<LiveRequest>(MAX_QUEUED_REQUESTS);
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let thread = std::thread::spawn(move || {
            listener_loop(listener, tx, thread_stop);
        });
        Ok(LiveBridge {
            requests: rx,
            stop,
            thread: Some(thread),
        })
    }

    /// Stop the listener: raise the stop flag, nudge the
    /// accept loop with a throwaway connection, and join the
    /// thread. The join cannot hang in practice: the accept
    /// loop polls the flag every few milliseconds, and a
    /// connection in flight abandons its reply wait within one
    /// poll of the flag (its client sees the connection close
    /// without a response — the honest outcome for a bridge
    /// that was just switched off).
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Unblock a parked accept promptly (the loop also
        // polls, so this is a latency nicety, not a lifeline).
        let _ = TcpStream::connect(("127.0.0.1", PORT));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for LiveBridge {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The singleton bridge owned by the shell glue, plus the
/// enabled flag the settings toggle reads.
static BRIDGE: Mutex<Option<LiveBridge>> = Mutex::new(None);
static ENABLED: AtomicBool = AtomicBool::new(false);

/// Why the most recent bridge start failed, in the OS
/// error's own words (e.g. "Address already in use
/// (os error 98)") — or `None` when the last start
/// succeeded, the bridge has been stopped since, or no
/// start has been attempted in this process. Recorded by
/// [`set_enabled`], whose callers otherwise only ever see
/// the resulting bool: above all `main`'s `init`, which
/// starts the bridge at launch when the persisted
/// preference says so and has no surface of its own to
/// report through. Without this memory a busy port at
/// launch was invisible — the Settings card could show
/// the toggle off but never say why (Wave 14, D1; the
/// gap the Wave 11 audit named in `settings_popover`'s
/// module docs).
static LAST_START_ERROR: Mutex<Option<String>> = Mutex::new(None);

fn record_start_error(err: &std::io::Error) {
    *LAST_START_ERROR.lock().unwrap_or_else(|p| p.into_inner()) = Some(err.to_string());
}

fn clear_start_error() {
    *LAST_START_ERROR.lock().unwrap_or_else(|p| p.into_inner()) = None;
}

/// The reason the most recent bridge start failed, if it
/// did — see [`LAST_START_ERROR`]. Set on a failed start,
/// cleared on a successful start and on any stop. Cheap
/// enough to call from `view()` (one short lock, no I/O).
pub fn last_start_error() -> Option<String> {
    LAST_START_ERROR
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
}

/// The event-loop waker the app hands over at startup
/// (`AppSpec::setup` — the type is Rune's `Waker`, an
/// `Arc<dyn Fn() + Send + Sync>`; spelled structurally here so
/// this module stays std-only). The listener calls it after a
/// request lands in the queue, so the UI thread's next `poll`
/// — where [`drain`] runs — happens promptly instead of at
/// the next unrelated wake. The shell ALSO keeps a 100 ms
/// poll heartbeat (Wave 11, A3): the waker is additive, and
/// the heartbeat remains the proven floor until the waker has
/// a live track record.
static WAKER: Mutex<Option<std::sync::Arc<dyn Fn() + Send + Sync>>> = Mutex::new(None);

/// Store the event-loop waker (called once from the app's
/// setup hook).
pub fn set_waker(waker: std::sync::Arc<dyn Fn() + Send + Sync>) {
    *WAKER.lock().unwrap_or_else(|p| p.into_inner()) = Some(waker);
}

fn wake() {
    let slot = WAKER.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(waker) = slot.as_ref() {
        waker();
    }
}

/// Errors from drained requests that never wrote
/// `core.status` — protocol errors (an unknown tool, a
/// malformed call) and tool refusals whose handler returned
/// without touching the status line. Ordinary refusals DO
/// write the status, and the shell already toasts those from
/// the status line itself; buffering these separately is what
/// keeps them visible without double-toasting (Wave 11, A3
/// request B). Drained by [`take_drain_errors`]; capped
/// at [`MAX_DRAIN_ERRORS`] (Wave 14, D3).
static DRAIN_ERRORS: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Take the buffered drain errors (oldest first), clearing
/// the buffer. The shell calls this after each drain and
/// toasts each entry with the `MCP: ` attribution prefix.
pub fn take_drain_errors() -> Vec<String> {
    std::mem::take(&mut *DRAIN_ERRORS.lock().unwrap_or_else(|p| p.into_inner()))
}

/// Upper bound on buffered drain errors (Wave 14, D3).
/// In the shipped app the buffer can never hold more
/// than one drain batch: `drain` records at most one
/// error per processed request (≤ [`MAX_QUEUED_REQUESTS`]
/// per call) and the shell takes the buffer in the same
/// poll tick (src/main.rs). The cap is defence for the
/// static itself — a future drain caller that forgets
/// the take must not turn this into an unbounded buffer
/// growing by one string per request forever. When the
/// cap is reached the OLDEST errors are dropped: they
/// are toast texts, and the newest refusal is the one
/// the user still needs to see.
const MAX_DRAIN_ERRORS: usize = 64;

fn record_drain_error(text: String) {
    let mut errors = DRAIN_ERRORS.lock().unwrap_or_else(|p| p.into_inner());
    push_drain_error(&mut errors, text);
}

/// [`record_drain_error`]'s push, with the
/// [`MAX_DRAIN_ERRORS`] cap applied (oldest dropped
/// first). A free function over the buffer so the cap
/// is unit-testable without touching the shared static.
fn push_drain_error(errors: &mut Vec<String>, text: String) {
    if errors.len() >= MAX_DRAIN_ERRORS {
        let excess = errors.len() + 1 - MAX_DRAIN_ERRORS;
        errors.drain(..excess);
    }
    errors.push(text);
}

fn lock_bridge() -> std::sync::MutexGuard<'static, Option<LiveBridge>> {
    // A poisoned mutex here only means a previous holder
    // panicked; the bridge state is still coherent enough to
    // recover rather than wedge the toggle forever.
    BRIDGE.lock().unwrap_or_else(|p| p.into_inner())
}

/// Switch live control on or off, starting or stopping the
/// singleton bridge. Returns the RESULTING state: `true` only
/// if the bridge is actually listening (a failed bind — port
/// already taken — reports `false`, never a phantom on).
/// A failed start also records its OS error for
/// [`last_start_error`]; a successful start and any stop
/// clear the record.
pub fn set_enabled(on: bool) -> bool {
    if !on {
        // Stopping clears any recorded start error first —
        // even when the bridge is already off: once the user
        // has switched live control off, a stale failure
        // note must not outlive the stop.
        clear_start_error();
        if !is_enabled() {
            return false;
        }
        if let Some(mut bridge) = lock_bridge().take() {
            bridge.stop();
        }
        ENABLED.store(false, Ordering::SeqCst);
        return false;
    }
    if is_enabled() {
        return true;
    }
    match LiveBridge::start() {
        Ok(bridge) => {
            *lock_bridge() = Some(bridge);
            ENABLED.store(true, Ordering::SeqCst);
            clear_start_error();
            true
        }
        Err(err) => {
            // Remember WHY (Wave 14, D1): every caller of
            // this fn — above all `main`'s launch path —
            // only gets the bool back, so the reason would
            // vanish with this error value. This match arm
            // is the one failure point every start the app
            // ever makes passes through (`LiveBridge::start`
            // has no other caller).
            record_start_error(&err);
            false
        }
    }
}

/// Whether live control is currently on (the bridge is
/// listening). Cheap enough to call from `view()`.
pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::SeqCst)
}

/// Run up to `max` queued live requests against the app's
/// LIVE core and send each response back to its waiting HTTP
/// connection. Call from the app's poll loop every tick; the
/// count returned is how many requests were processed (0 when
/// the bridge is off or the queue is empty).
///
/// Routing goes through `router.handle_on(core, &msg)` — the
/// `McpServer` is only a dispatcher here; the tools execute
/// against `core`, never against the router's own core.
///
/// Nothing in the queue is a project snapshot or any other
/// project-derived cache: each entry is one client's parsed
/// message, routed against the caller's live core at drain
/// time. That is also why a request the listener has given
/// up on is discarded here (see [`LiveRequest::cancelled`])
/// rather than executed — by the time a wedged app recovers,
/// a load or save through this same path may have replaced
/// the project the request was aimed at.
pub fn drain(core: &mut EditorCore, router: &mut McpServer, max: usize) -> usize {
    let guard = lock_bridge();
    let Some(bridge) = guard.as_ref() else {
        return 0;
    };
    let mut done = 0;
    while done < max {
        let req = match bridge.requests.try_recv() {
            Ok(req) => req,
            Err(_) => break, // empty, or the listener is gone
        };
        // A request whose client already got a 504 (or whose
        // bridge was switched off mid-wait) is discarded, not
        // routed: it was aimed at the project that was live
        // when it was sent, and that project may be gone.
        let Some(req) = discard_if_cancelled(req) else {
            done += 1;
            continue;
        };
        // Wave 14 (shell control): a call for a shell tool
        // is intercepted here and handed to the shell
        // queue — the router cannot execute it (its state
        // is the GUI shell's, not the core's), and its
        // registry entry is a stub that would refuse. The
        // shell drain in poll executes it against the live
        // State right after this drain and answers through
        // the request's own reply channel, which travels
        // with the queued request. This peek is the one
        // inspection the drain performs on a message: the
        // name test only, never the arguments (the
        // handlers own validation, against the live
        // shell). A queue-full refusal is answered here,
        // on the request's own channel, in the standard
        // tool-result shape — never a silent drop.
        if let Some(shell_req) = crate::shell_control::peek(&req) {
            if let Err((shell_req, why)) = crate::shell_control::enqueue(shell_req) {
                let _ = shell_req
                    .reply
                    .send(Some(crate::shell_control::respond(shell_req.id.clone(), Err(why))));
            }
            done += 1;
            continue;
        }
        // Snapshot the status line so an error that never
        // wrote it can be told apart from an ordinary refusal
        // (which writes the status and is toasted from there).
        let status_before = core.status.clone();
        let response = router.handle_on(core, &req.msg);
        if let Some(text) = drain_attribution_error(&status_before, &core.status, &response) {
            record_drain_error(text);
        }
        // A send failure only means the client hung up while
        // waiting — the edit still happened; nothing to undo.
        let _ = req.reply.send(response);
        done += 1;
    }
    done
}

/// The drain-attribution decision: given the status line
/// before and after one routed request and the router's
/// response, the error text (if any) that must be buffered
/// for the shell's `MCP: ` toast because it never reached
/// the status line.
///
/// Two response families qualify, and only while the status
/// line is untouched:
///
/// - a **protocol error** (`error.message` — an unknown
///   tool, a malformed call): protocol errors never write
///   the status, so without this buffer they would vanish;
/// - a **status-silent tool refusal** (a `tools/call`
///   result with `isError` set): most refusals write
///   `core.status` themselves and are toasted by the shell
///   from there; the ones that do not are caught here by
///   their first content text.
///
/// A refusal that DID write the status returns `None` —
/// surfacing it here too would double-toast. Successes,
/// notifications (no response) and text-less error results
/// also return `None`.
///
/// The decision's whole input surface is (status changed?,
/// response shape) — the request message is deliberately
/// NOT an input. That is the Wave 13 audit point: no tool
/// family can be surfaced differently from another, so the
/// wave's new sprite tools (`sprite_add`, `sprite_set_key`,
/// …) and the dirty-seam write paths of Squad B attribute
/// exactly like every call before them, with no per-tool
/// casing anywhere in this module.
fn drain_attribution_error(
    status_before: &str,
    status_after: &str,
    response: &Option<Value>,
) -> Option<String> {
    if status_after != status_before {
        return None;
    }
    let resp = response.as_ref()?;
    if let Some(msg) = resp
        .get("error")
        .and_then(|e| e.get("message"))
        .and_then(|m| m.as_str())
    {
        return Some(msg.to_string());
    }
    if resp
        .get("result")
        .and_then(|r| r.get("isError"))
        .and_then(|b| b.as_bool())
        == Some(true)
    {
        if let Some(text) = resp
            .pointer("/result/content/0/text")
            .and_then(|t| t.as_str())
        {
            return Some(text.to_string());
        }
    }
    None
}

// ---------------------------------------------------------------------
// Listener thread
// ---------------------------------------------------------------------

/// Accept connections one at a time until the stop flag is
/// raised. Sequential by design: it keeps ordering trivial and
/// the queue is the only hand-off to the UI thread.
fn listener_loop(listener: TcpListener, tx: SyncSender<LiveRequest>, stop: Arc<AtomicBool>) {
    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                // One connection must never be able to kill
                // the only listener thread: `handle_connection`
                // is written to answer malformed input rather
                // than panic (see its docs), and this catch is
                // the belt to those braces — an unforeseen panic
                // while serving one client drops that connection
                // and the loop accepts the next one.
                let served = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    handle_connection(stream, &tx, &stop);
                }));
                if served.is_err() {
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(_) => {
                // A transient accept failure should not kill
                // the bridge; back off briefly and keep going
                // unless we were told to stop.
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

/// Serve one connection: read one HTTP request, forward its
/// JSON-RPC body to the UI thread, wait for the reply, write
/// the HTTP response, close. Every early return is a client or
/// transport failure that has already been answered (or needs
/// no answer); this function never panics on bad input.
fn handle_connection(mut stream: TcpStream, tx: &SyncSender<LiveRequest>, stop: &Arc<AtomicBool>) {
    let _ = stream.set_read_timeout(Some(IO_TIMEOUT));
    let _ = stream.set_write_timeout(Some(IO_TIMEOUT));
    // The accept loop is non-blocking; accepted sockets are
    // blocking on some platforms and inherit the flag on
    // others — make the mode explicit before reading.
    let _ = stream.set_nonblocking(false);

    // The whole request must arrive within READ_DEADLINE of
    // the accept; checked before every blocking read below.
    let read_deadline = Instant::now() + READ_DEADLINE;

    // --- Read the head (request line + headers). ---
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 8192];
    let head_len = loop {
        if let Some(pos) = find_subslice(&buf, b"\r\n\r\n") {
            break pos + 4;
        }
        if buf.len() > MAX_HEAD {
            write_response(&mut stream, 400, "Bad Request", "text/plain", b"headers too large\n");
            return;
        }
        if Instant::now() >= read_deadline {
            return; // slow-dribble head; clean close
        }
        match stream.read(&mut chunk) {
            Ok(0) => return, // client went away mid-head
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(_) => return,
        }
    };
    // The terminator can arrive in the same chunk that pushes
    // the buffer past MAX_HEAD, so the cap is re-checked on
    // the head itself, not just on the accumulating buffer.
    if head_len > MAX_HEAD {
        write_response(&mut stream, 400, "Bad Request", "text/plain", b"headers too large\n");
        return;
    }
    let head = String::from_utf8_lossy(&buf[..head_len]).into_owned();

    // --- Host/Origin gate: before routing, before the body
    //     is read, before anything can be queued. A foreign
    //     Host (DNS rebinding) or a foreign Origin (a web
    //     page fetching across origins) is refused here, so
    //     no tool dispatch is reachable from a browser page.
    //     See `host_origin_allowed` for the rule — and for
    //     what it is not (authentication). ---
    if !host_origin_allowed(&head) {
        write_response(&mut stream, 403, "Forbidden", "text/plain", b"live endpoint serves loopback Host/Origin only\n");
        return;
    }

    // --- Route on method + path. ---
    let method = request_method(&head);
    let path = path_of(request_target(&head));
    if method == "GET" {
        // Health/identity probe: the one answer that proves
        // this port is the LIVE editor (the stdio server has
        // no HTTP listener at all). No core access, no queue.
        if path == "/" || path == "/health" {
            write_json(&mut stream, 200, &health_body());
        } else {
            write_response(&mut stream, 404, "Not Found", "text/plain", b"no such path\n");
        }
        return;
    }
    if method != "POST" {
        write_response(&mut stream, 405, "Method Not Allowed", "text/plain", b"POST only\n");
        return;
    }
    if path != "/mcp" {
        write_response(&mut stream, 404, "Not Found", "text/plain", b"POST to /mcp\n");
        return;
    }
    if has_transfer_encoding(&head) {
        // Chunked bodies are not decoded (module docs); say
        // so plainly instead of misreading the request as an
        // empty body and answering a misleading parse error.
        write_response(&mut stream, 501, "Not Implemented", "text/plain", b"chunked transfer encoding not supported; send Content-Length\n");
        return;
    }
    if !content_type_acceptable(&head) {
        // A JSON-RPC body under a non-JSON media type is a
        // client error, not something to guess at: MCP
        // clients send application/json. (An ABSENT header is
        // accepted — see the helper.)
        write_response(&mut stream, 415, "Unsupported Media Type", "text/plain", b"Content-Type must be application/json\n");
        return;
    }

    // --- Read exactly Content-Length bytes of body. A length
    //     header that is present but unusable (not a number,
    //     or duplicated with differing values) is a framing
    //     error — HTTP 400 — not a licence to guess 0. ---
    let content_length = match content_length_status(&head) {
        ContentLength::Absent => 0,
        ContentLength::Value(n) => n,
        ContentLength::Invalid => {
            write_response(&mut stream, 400, "Bad Request", "text/plain", b"bad content-length\n");
            return;
        }
    };
    if content_length > MAX_BODY {
        let body = body_too_large_body();
        write_response(&mut stream, 413, "Payload Too Large", "application/json", body.as_bytes());
        return;
    }
    let mut body: Vec<u8> = buf[head_len..].to_vec();
    while body.len() < content_length {
        if Instant::now() >= read_deadline {
            return; // slow-dribble body; clean close
        }
        match stream.read(&mut chunk) {
            Ok(0) => return, // truncated body; nothing to answer
            Ok(n) => body.extend_from_slice(&chunk[..n]),
            Err(_) => return,
        }
    }
    body.truncate(content_length);

    // --- Parse; a parse failure is the -32700 convention,
    //     answered as a normal JSON-RPC body (mirrors the
    //     stdio shell in src/bin/mcp_server.rs). ---
    let msg: Value = match serde_json::from_slice(&body) {
        Ok(msg) => msg,
        Err(err) => {
            let body = parse_error_body(&err);
            write_json(&mut stream, 200, &body);
            return;
        }
    };

    // --- Notifications are answered here, never queued. The
    //     shared dispatch checks for an `id` before routing
    //     any method (mcp.rs), so a notification is guaranteed
    //     to execute nothing and earn no response; queueing it
    //     would only make the client wait for the UI thread —
    //     and, if the app were wedged, wait out the whole
    //     REPLY_TIMEOUT for a message that needs no answer.
    //     202 with an empty body is the HTTP acknowledgement.
    if is_notification(&msg) {
        write_response(&mut stream, 202, "Accepted", "application/json", b"");
        return;
    }

    // --- Forward to the UI thread and block on the reply.
    //     This wait IS the synchronisation: the reply lands
    //     when poll() next drains the queue (see module docs).
    let (reply_tx, reply_rx) = std::sync::mpsc::sync_channel::<Option<Value>>(1);
    let cancelled = Arc::new(AtomicBool::new(false));
    let request = LiveRequest {
        msg,
        reply: reply_tx,
        cancelled: Arc::clone(&cancelled),
    };
    match tx.try_send(request) {
        Ok(()) => {}
        Err(TrySendError::Full(_)) => {
            // The queue is bounded (see MAX_QUEUED_REQUESTS):
            // a full queue means the app fell a whole drain
            // batch behind — stale cancelled requests not yet
            // discarded, or a wedged poll loop. Refuse plainly
            // rather than block the one listener thread behind
            // a queue that may never move.
            write_response(&mut stream, 503, "Service Unavailable", "text/plain", b"editor request queue is full\n");
            return;
        }
        Err(TrySendError::Disconnected(_)) => {
            // The queue's receiver is gone: the bridge is being
            // torn down under this connection.
            write_response(&mut stream, 500, "Internal Server Error", "text/plain", b"editor not draining\n");
            return;
        }
    }
    // The request is queued: nudge the event loop so poll
    // drains it promptly (the 100 ms heartbeat in the shell
    // remains as the floor).
    wake();
    let wait_start = Instant::now();
    loop {
        match reply_rx.recv_timeout(Duration::from_millis(25)) {
            Ok(Some(response)) => {
                write_json(&mut stream, 200, &response);
                return;
            }
            Ok(None) => {
                // Notification (defensive fallback — the
                // listener answers HTTP notifications before
                // queueing, above): JSON-RPC says never
                // answer, but the HTTP layer still needs a
                // status, so 202 with an empty body.
                write_response(&mut stream, 202, "Accepted", "application/json", b"");
                return;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if stop.load(Ordering::SeqCst) {
                    // Bridge switched off mid-request: cancel
                    // the queued request so a later restart's
                    // drain cannot apply it out of context.
                    cancelled.store(true, Ordering::SeqCst);
                    return;
                }
                if wait_start.elapsed() >= REPLY_TIMEOUT {
                    // The app is alive enough to keep us
                    // waiting but never drained: answer 504
                    // and free the listener. The request stays
                    // in the queue but is marked cancelled, so
                    // a later drain discards it instead of
                    // applying a stale edit (see drain).
                    cancelled.store(true, Ordering::SeqCst);
                    write_response(&mut stream, 504, "Gateway Timeout", "text/plain", b"editor did not answer in time\n");
                    return;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                // The request was dropped without a reply
                // (bridge stopped while it sat in the queue).
                write_response(&mut stream, 500, "Internal Server Error", "text/plain", b"request dropped\n");
                return;
            }
        }
    }
}

// ---------------------------------------------------------------------
// HTTP helpers (pure; the parsing ones are unit-tested below)
// ---------------------------------------------------------------------

/// The JSON-RPC parse-error body, byte-for-byte the convention
/// the stdio shell uses: id `null`, code `-32700`.
fn parse_error_body(err: &serde_json::Error) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": Value::Null,
        "error": { "code": -32700, "message": format!("Parse error: {err}") },
    })
}

/// The oversized-body rejection, as a JSON-RPC error body:
/// the request's id was never read (that is the point of
/// rejecting early), so the id is `null` and the code is
/// `-32600` (Invalid Request). Serialised once per rejection;
/// the HTTP status stays 413.
fn body_too_large_body() -> String {
    json!({
        "jsonrpc": "2.0",
        "id": Value::Null,
        "error": {
            "code": -32600,
            "message": format!("Invalid Request: body exceeds the {MAX_BODY}-byte limit"),
        },
    })
    .to_string()
}

/// The health/identity document served by `GET /` and
/// `GET /health`. Name and version mirror `initialize`'s
/// `serverInfo` exactly, so a client sees one identity over
/// both transports; `transport`/`live` are the fields that
/// tell this endpoint apart from the stdio server, and
/// `auth` states the v1 security posture plainly, matching
/// the settings card's wording. `tools` is the registered
/// tool count, read from the same `all_tools()` registry
/// the `server_info` tool reports — knowable in this layer
/// because the registry is static data, no core needed.
/// `port` is the [`PORT`] constant the listener binds (the
/// same number `endpoint()` embeds), and `enabled` is the
/// [`is_enabled`] flag — set only after a successful bind,
/// so a health answer that exists at all reads it `true`
/// from the singleton bridge that served it.
///
/// This function runs on the listener thread and must stay
/// lock-free apart from atomics: it takes no core, no queue
/// and NOT the `BRIDGE` mutex — [`drain`] holds that mutex
/// across a whole batch, so a health build that locked it
/// could stall behind a long tool call, defeating the
/// probe's purpose (it is how a client tells a wedged app
/// from a dead one). Every field above is a constant, an
/// atomic read, or static registry data, so the document
/// answers even while the app's poll loop is busy.
///
/// The `shell_control` field (Wave 14, D4): D4 specced
/// it absent until Squad B's plumbing landed, on the
/// grounds that a runtime availability flag would need
/// a lock-free source readable here. As landed, no
/// flag is needed: the shell-control channel is
/// compiled into this binary and its drain runs in the
/// poll loop unconditionally, so availability is a
/// constant truth about the build — the field below
/// reports `true` outright, and
/// `health_body_names_the_live_server` pins it.
fn health_body() -> Value {
    json!({
        // Wave 14 (D7): the server's identity comes from
        // the one set of consts the router's initialize
        // answer and the server_info tool also read — a
        // literal here would be a second source waiting
        // to drift.
        "server": stykanimate_rs::mcp::SERVER_NAME,
        "version": stykanimate_rs::mcp::SERVER_VERSION,
        "transport": "live-http",
        "live": true,
        "endpoint": endpoint(),
        "port": PORT,
        "enabled": is_enabled(),
        // Wave 14: the shell-control channel is compiled
        // in and drained by poll unconditionally, so this
        // endpoint serves the shell_* tools — a constant
        // truth about this build (D4 specced the field
        // absent until the plumbing landed; it has).
        "shell_control": true,
        "auth": "none",
        "tools": stykanimate_rs::mcp::all_tools().len(),
    })
}

/// Write `bytes` to the socket under a TOTAL deadline (see
/// [`WRITE_DEADLINE`]): the socket's write timeout is reset
/// to the remaining budget before every chunk, so progress
/// cannot refinance the wait the way per-write timeouts
/// alone allow. Returns `false` when the bytes did not all
/// make it — peer gone, write error, or deadline spent.
/// Callers ignore that outcome in the usual socket way: a
/// client that cannot be written to is gone, and there is
/// no one left to report the failure to.
fn write_bounded(stream: &mut TcpStream, bytes: &[u8], deadline: Instant) -> bool {
    let mut written = 0;
    while written < bytes.len() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return false;
        }
        let _ = stream.set_write_timeout(Some(IO_TIMEOUT.min(remaining)));
        let end = (written + 64 * 1024).min(bytes.len());
        match stream.write(&bytes[written..end]) {
            Ok(0) => return false,
            Ok(n) => written += n,
            Err(_) => return false,
        }
    }
    true
}

/// Write one complete HTTP/1.1 response and let the socket
/// close with the stream. Write failures are ignored — the
/// client is already gone and there is no one to report to.
fn write_response(stream: &mut TcpStream, status: u16, reason: &str, content_type: &str, body: &[u8]) {
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let deadline = Instant::now() + WRITE_DEADLINE;
    if write_bounded(stream, head.as_bytes(), deadline) {
        let _ = write_bounded(stream, body, deadline);
    }
    let _ = stream.flush();
}

/// Write a JSON-RPC response value with HTTP 200 (all routed
/// responses, including protocol errors, are 200 — the error
/// lives inside the JSON-RPC body, as MCP's HTTP transport
/// prescribes).
fn write_json(stream: &mut TcpStream, status: u16, value: &Value) {
    let body = value.to_string();
    let head = format!(
        "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let deadline = Instant::now() + WRITE_DEADLINE;
    if write_bounded(stream, head.as_bytes(), deadline) {
        let _ = write_bounded(stream, body.as_bytes(), deadline);
    }
    let _ = stream.flush();
}

/// Split a raw request buffer at the head/body boundary
/// (`\r\n\r\n`). Returns `(head, body_prefix)` — the head
/// without the terminating blank line, and whatever body
/// bytes already arrived in the same read. `None` until the
/// terminator has arrived.
fn split_head_body(buf: &[u8]) -> Option<(&[u8], &[u8])> {
    let pos = find_subslice(buf, b"\r\n\r\n")?;
    Some((&buf[..pos], &buf[pos + 4..]))
}

/// The method token of the request line (`POST`, `GET`, …):
/// the first whitespace-delimited word of the head.
fn request_method(head: &str) -> &str {
    head.split_whitespace().next().unwrap_or("")
}

/// Parse the `Content-Length` header (case-insensitive name,
/// surrounding whitespace tolerated). `None` when the header
/// is absent or its value is not a plain number.
///
/// A thin view over [`content_length_status`] (which see for
/// the framing rules): both `Absent` and `Invalid` collapse
/// to `None` here. Production code making accept/refuse
/// decisions uses the status form, which keeps the two
/// apart; this shape is retained for the callers that only
/// ever needed the number.
fn parse_content_length(head: &str) -> Option<usize> {
    match content_length_status(head) {
        ContentLength::Value(n) => Some(n),
        ContentLength::Absent | ContentLength::Invalid => None,
    }
}

/// What the request's `Content-Length` framing actually
/// says — the three cases [`handle_connection`] must tell
/// apart, where [`parse_content_length`]'s `None` conflates
/// two of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContentLength {
    /// No header: a bodyless request (length 0).
    Absent,
    /// One usable length (or several headers that agree).
    Value(usize),
    /// A header is present but unusable: not a plain number,
    /// or duplicated with differing values — the classic
    /// request-smuggling shape, refused rather than guessed.
    Invalid,
}

/// Scan ALL `Content-Length` headers (case-insensitive) and
/// classify the framing. Identical duplicates are tolerated
/// (some clients emit them); differing ones are `Invalid`.
fn content_length_status(head: &str) -> ContentLength {
    let mut found: Option<usize> = None;
    for line in head.lines() {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if !name.trim().eq_ignore_ascii_case("content-length") {
            continue;
        }
        let Ok(n) = value.trim().parse::<usize>() else {
            return ContentLength::Invalid;
        };
        match found {
            None => found = Some(n),
            Some(prev) if prev == n => {}
            Some(_) => return ContentLength::Invalid,
        }
    }
    match found {
        None => ContentLength::Absent,
        Some(n) => ContentLength::Value(n),
    }
}

/// Whether the head carries any `Transfer-Encoding` header
/// (case-insensitive name). Any value counts: no transfer
/// encoding is decoded by this server, so presence alone is
/// enough to refuse the request rather than misframe it.
fn has_transfer_encoding(head: &str) -> bool {
    head.lines().any(|line| {
        line.split_once(':')
            .is_some_and(|(name, _)| name.trim().eq_ignore_ascii_case("transfer-encoding"))
    })
}

/// Whether a parsed message is a JSON-RPC notification,
/// mirroring the shared dispatch's rule exactly (mcp.rs):
/// a JSON **object** with no `id` member. Note the two
/// exclusions the rule implies — an explicit `"id": null`
/// is a request (it earns an answer, with a `null` id), and
/// a non-object (batch array, scalar) is not a notification
/// either: dispatch answers it `-32600`, so it must still
/// be queued, not fast-pathed.
fn is_notification(msg: &Value) -> bool {
    msg.is_object() && msg.get("id").is_none()
}

/// Whether the request's `Content-Type` framing is one this
/// server will parse a JSON-RPC body under. A present header
/// must name the `application/json` media type — parameters
/// (`; charset=utf-8`) and case are irrelevant. An ABSENT
/// header is accepted: the header is a claim about the body,
/// not the body itself, and minimal hand-rolled clients do
/// not always send one; the body is parsed as JSON either
/// way, so a missing claim costs nothing while a false one
/// is refused (HTTP 415) rather than silently parsed past.
fn content_type_acceptable(head: &str) -> bool {
    for line in head.lines() {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if !name.trim().eq_ignore_ascii_case("content-type") {
            continue;
        }
        let media = value.split(';').next().unwrap_or("").trim();
        return media.eq_ignore_ascii_case("application/json");
    }
    true
}

/// Every value sent for header `name` (case-insensitive
/// name, value trimmed), in the order sent. The scan is the
/// same whole-line one the other head helpers use; the
/// request line itself never matches (it carries no colon
/// in the name position of a real header line — and if a
/// malformed one did, the value checks below still apply
/// to whatever it yields).
fn header_values<'a>(head: &'a str, name: &str) -> Vec<&'a str> {
    head.lines()
        .filter_map(|line| line.split_once(':'))
        .filter(|(n, _)| n.trim().eq_ignore_ascii_case(name))
        .map(|(_, v)| v.trim())
        .collect()
}

/// Whether an authority (`host` or `host:port`, IPv6 in
/// brackets) names this machine: `localhost`, `127.0.0.1`
/// or `[::1]`, in any letter case, with an optional numeric
/// port. The port is not compared against [`PORT`]: the
/// request already arrived on this listener's socket, and
/// local clients legitimately send the bare host (the
/// test suite's own minimal client does).
///
/// Deliberately a whitelist of the spellings a local
/// client actually sends, NOT a resolver: a trailing dot
/// (`localhost.`), a shorthand (`127.1`), a name that
/// merely ends in a loopback spelling
/// (`127.0.0.1.evil.example`), a non-numeric port, or a
/// malformed bracket all refuse. Nothing here is ever
/// resolved through DNS — a name is loopback only if it
/// is spelled loopback.
fn authority_is_loopback(authority: &str) -> bool {
    fn port_ok(port: &str) -> bool {
        !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit())
    }
    let host = if let Some(rest) = authority.strip_prefix('[') {
        // Bracketed IPv6: `[::1]`, optionally `[::1]:port`.
        let Some((host, after)) = rest.split_once(']') else {
            return false;
        };
        if !after.is_empty() && !after.strip_prefix(':').is_some_and(port_ok) {
            return false;
        }
        host
    } else {
        match authority.split_once(':') {
            Some((host, port)) => {
                if !port_ok(port) {
                    return false;
                }
                host
            }
            None => authority,
        }
    };
    host.eq_ignore_ascii_case("localhost")
        || host == "127.0.0.1"
        || host.eq_ignore_ascii_case("::1")
}

/// Whether an `Origin` header value is a loopback origin:
/// an `http`/`https` scheme over a loopback authority,
/// and nothing else. A serialized origin has no path,
/// query, fragment or userinfo, so any of those refuses —
/// as does the literal `null` browsers send for opaque
/// origins (sandboxed frames, `file:` pages): it carries
/// no authority to check, so it is not loopback.
fn origin_is_loopback(origin: &str) -> bool {
    let Some((scheme, authority)) = origin.split_once("://") else {
        return false;
    };
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return false;
    }
    if authority.chars().any(|c| matches!(c, '/' | '?' | '#' | '@')) {
        return false;
    }
    authority_is_loopback(authority)
}

/// The Host/Origin gate [`handle_connection`] applies to
/// every request before routing, before the body is read,
/// before anything can be queued:
///
/// - at least one `Host` header must be present, and
///   EVERY `Host` value must be a loopback authority
///   ([`authority_is_loopback`]). A missing Host refuses:
///   HTTP/1.1 requires the header and every real MCP
///   client sends it, so its absence is not a shape this
///   endpoint serves. A foreign Host is the DNS-rebinding
///   shape — a page whose domain was rebound to 127.0.0.1
///   still sends its own domain as Host.
/// - every `Origin` value, when the header is sent at
///   all, must be a loopback origin
///   ([`origin_is_loopback`]). Non-browser clients do not
///   send Origin; browsers always send it on cross-origin
///   fetches, which is exactly the traffic this refuses.
///
/// What this gate is NOT: it is header **validation, not
/// authentication**. Any program running on this machine
/// can send loopback headers — curl and every local MCP
/// client do — so the gate changes nothing for local
/// processes; the module docs' warning (any local program
/// can call every tool while live control is enabled)
/// stands in full. The gate's whole job is the browser
/// threat the loopback bind alone does not stop: a web
/// page the user happens to have open must not be able to
/// drive the editor, by its origin or by its host name.
fn host_origin_allowed(head: &str) -> bool {
    let hosts = header_values(head, "host");
    if hosts.is_empty() {
        return false;
    }
    if !hosts.iter().all(|h| authority_is_loopback(h)) {
        return false;
    }
    header_values(head, "origin")
        .iter()
        .all(|o| origin_is_loopback(o))
}

/// The request target of the request line (`/mcp`, `/`, …):
/// the second whitespace-delimited word of the head, or ""
/// when the request line has no second word.
fn request_target(head: &str) -> &str {
    head.split_whitespace().nth(1).unwrap_or("")
}

/// The path part of a request target: everything before an
/// optional `?query`. Routing compares paths, so a query
/// string never changes which handler a request reaches.
fn path_of(target: &str) -> &str {
    target.split('?').next().unwrap_or("")
}

/// First index of `needle` in `haystack`, if present.
fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

// ---------------------------------------------------------------------
// Tests — pure parsing helpers only; no sockets are bound here.
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_HEAD: &str = "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:32117\r\nContent-Type: application/json\r\nContent-Length: 42\r\n\r\n";

    #[test]
    fn endpoint_names_the_loopback_port() {
        assert_eq!(PORT, 32117);
        assert_eq!(endpoint(), "http://127.0.0.1:32117/mcp");
    }

    #[test]
    fn split_head_body_finds_the_blank_line() {
        let raw = format!("{SAMPLE_HEAD}{{\"jsonrpc\":\"2.0\"}}");
        let (head, body) = split_head_body(raw.as_bytes()).expect("terminator present");
        assert!(head.starts_with(b"POST /mcp"));
        assert!(!head.ends_with(b"\r\n\r\n"), "terminator is not part of the head");
        assert_eq!(body, b"{\"jsonrpc\":\"2.0\"}");
    }

    #[test]
    fn split_head_body_waits_for_the_full_terminator() {
        // Partial head, and a head ending in a lone CRLF (one
        // header's end, not the blank line): both incomplete.
        assert!(split_head_body(b"POST /mcp HTTP/1.1\r\nHost: x").is_none());
        assert!(split_head_body(b"POST /mcp HTTP/1.1\r\nHost: x\r\n").is_none());
        assert!(split_head_body(b"").is_none());
    }

    #[test]
    fn request_method_reads_the_request_line() {
        assert_eq!(request_method(SAMPLE_HEAD), "POST");
        assert_eq!(request_method("GET /mcp HTTP/1.1\r\n\r\n"), "GET");
        assert_eq!(request_method(""), "");
    }

    #[test]
    fn content_length_parses_case_insensitively() {
        assert_eq!(parse_content_length(SAMPLE_HEAD), Some(42));
        assert_eq!(parse_content_length("POST / HTTP/1.1\r\ncontent-length: 7"), Some(7));
        assert_eq!(parse_content_length("POST / HTTP/1.1\r\nCONTENT-LENGTH:   123  "), Some(123));
    }

    #[test]
    fn content_length_absent_or_garbled_is_none() {
        assert_eq!(parse_content_length("POST /mcp HTTP/1.1\r\nHost: x"), None);
        assert_eq!(parse_content_length("POST / HTTP/1.1\r\nContent-Length: abc"), None);
        assert_eq!(parse_content_length("POST / HTTP/1.1\r\nContent-Length: "), None);
        // A colon in the request line must not be mistaken for
        // a header (request lines have no colon in practice,
        // but the parser only trusts known header names).
        assert_eq!(parse_content_length("POST http://x HTTP/1.1"), None);
    }

    #[test]
    fn parse_error_body_matches_the_stdio_convention() {
        let err = serde_json::from_slice::<Value>(b"{not json").unwrap_err();
        let body = parse_error_body(&err);
        assert_eq!(body["jsonrpc"], "2.0");
        assert_eq!(body["id"], Value::Null);
        assert_eq!(body["error"]["code"], -32700);
        assert!(
            body["error"]["message"].as_str().unwrap().starts_with("Parse error: "),
            "message mirrors the stdio shell's wording"
        );
    }

    #[test]
    fn body_cap_is_four_mib() {
        assert_eq!(MAX_BODY, 4 * 1024 * 1024);
    }

    #[test]
    fn request_queue_is_bounded_to_one_drain_batch() {
        // The shell drains at most 8 requests per poll
        // (main.rs); the queue bound matches that batch so a
        // healthy editor clears a full queue in one poll,
        // while a wedged one cannot accumulate requests
        // without limit.
        assert_eq!(MAX_QUEUED_REQUESTS, 8);
    }

    #[test]
    fn cancelled_request_is_discarded_before_routing() {
        let (reply_tx, reply_rx) = std::sync::mpsc::sync_channel::<Option<Value>>(1);
        let req = LiveRequest {
            msg: json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" }),
            reply: reply_tx,
            cancelled: Arc::new(AtomicBool::new(true)),
        };
        assert!(
            discard_if_cancelled(req).is_none(),
            "a request the listener gave up on must not reach the router"
        );
        // The defensive None reply is delivered so nothing
        // can wait on this request ever again.
        assert_eq!(reply_rx.recv(), Ok(None));
    }

    #[test]
    fn live_request_passes_the_drain_gate_untouched() {
        let (reply_tx, reply_rx) = std::sync::mpsc::sync_channel::<Option<Value>>(1);
        let req = LiveRequest {
            msg: json!({ "jsonrpc": "2.0", "id": 2, "method": "ping" }),
            reply: reply_tx,
            cancelled: Arc::new(AtomicBool::new(false)),
        };
        let req = discard_if_cancelled(req).expect("a live request passes through");
        assert_eq!(req.msg["id"], json!(2));
        assert!(
            reply_rx.try_recv().is_err(),
            "no reply is fabricated for a request still awaiting routing"
        );
    }

    #[test]
    fn drain_error_buffer_is_capped_oldest_first() {
        // Wave 14 (D3): the drain-error static is the one
        // buffer in the queue path with no intrinsic bound
        // of its own — the cap keeps the newest refusals
        // and drops the oldest toast texts.
        let mut errors: Vec<String> = Vec::new();
        for i in 0..MAX_DRAIN_ERRORS + 5 {
            push_drain_error(&mut errors, format!("err {i}"));
        }
        assert_eq!(errors.len(), MAX_DRAIN_ERRORS);
        assert_eq!(
            errors.first().map(String::as_str),
            Some("err 5"),
            "the five oldest were dropped"
        );
        let newest = format!("err {}", MAX_DRAIN_ERRORS + 4);
        assert_eq!(
            errors.last().map(String::as_str),
            Some(newest.as_str()),
            "the newest refusal survives the cap"
        );
    }

    #[test]
    fn full_queue_refuses_the_client_with_503() {
        // Wave 14 (D3): what happens AT the queue bound,
        // pinned end to end — a client whose request
        // cannot be queued is refused with HTTP 503
        // immediately: not queued, not blocked behind the
        // wedged queue, not panicked on. Driven through
        // the real `handle_connection` on an ephemeral
        // loopback port (NOT the bridge port, and no
        // singleton state), so the test is parallel-safe.
        use std::io::{Read, Write};
        let (tx, _rx) = sync_channel::<LiveRequest>(MAX_QUEUED_REQUESTS);
        for _ in 0..MAX_QUEUED_REQUESTS {
            let (reply_tx, _reply_rx) =
                std::sync::mpsc::sync_channel::<Option<Value>>(1);
            tx.try_send(LiveRequest {
                msg: json!({ "jsonrpc": "2.0", "id": 0, "method": "ping" }),
                reply: reply_tx,
                cancelled: Arc::new(AtomicBool::new(true)),
            })
            .expect("the queue takes requests up to its bound");
        }
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind ephemeral port");
        let addr = listener.local_addr().expect("ephemeral addr");
        let stop = Arc::new(AtomicBool::new(false));
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept one connection");
            handle_connection(stream, &tx, &stop);
        });
        let mut client = TcpStream::connect(addr).expect("connect");
        client
            .set_read_timeout(Some(Duration::from_secs(10)))
            .expect("client read timeout");
        let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" }).to_string();
        let request = format!(
            "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\n\
             Content-Type: application/json\r\nContent-Length: {}\r\n\
             Connection: close\r\n\r\n{body}",
            body.len()
        );
        client.write_all(request.as_bytes()).expect("write request");
        let mut raw = String::new();
        client.read_to_string(&mut raw).expect("read response");
        server.join().expect("server thread");
        assert!(
            raw.starts_with("HTTP/1.1 503"),
            "a full queue refuses plainly, got: {}",
            raw.lines().next().unwrap_or("")
        );
        assert!(
            raw.contains("editor request queue is full"),
            "the refusal says why: {raw}"
        );
    }

    #[test]
    fn request_target_reads_the_second_word() {
        assert_eq!(request_target(SAMPLE_HEAD), "/mcp");
        assert_eq!(request_target("GET /health HTTP/1.1\r\n\r\n"), "/health");
        assert_eq!(request_target("POST"), "");
        assert_eq!(request_target(""), "");
    }

    #[test]
    fn path_of_drops_any_query_string() {
        assert_eq!(path_of("/mcp"), "/mcp");
        assert_eq!(path_of("/health?verbose=1"), "/health");
        assert_eq!(path_of("/"), "/");
        assert_eq!(path_of(""), "");
    }

    #[test]
    fn content_length_status_separates_absent_value_and_invalid() {
        assert_eq!(content_length_status(SAMPLE_HEAD), ContentLength::Value(42));
        assert_eq!(
            content_length_status("POST /mcp HTTP/1.1\r\nHost: x"),
            ContentLength::Absent
        );
        // Present but not a plain number — including a
        // negative length and an empty value — is Invalid,
        // never silently 0.
        assert_eq!(
            content_length_status("POST / HTTP/1.1\r\nContent-Length: abc"),
            ContentLength::Invalid
        );
        assert_eq!(
            content_length_status("POST / HTTP/1.1\r\nContent-Length: -5"),
            ContentLength::Invalid
        );
        assert_eq!(
            content_length_status("POST / HTTP/1.1\r\nContent-Length: "),
            ContentLength::Invalid
        );
    }

    #[test]
    fn content_length_status_tolerates_agreeing_duplicates_only() {
        assert_eq!(
            content_length_status("POST / HTTP/1.1\r\nContent-Length: 7\r\ncontent-length: 7"),
            ContentLength::Value(7)
        );
        assert_eq!(
            content_length_status("POST / HTTP/1.1\r\nContent-Length: 7\r\nContent-Length: 8"),
            ContentLength::Invalid
        );
    }

    #[test]
    fn transfer_encoding_is_detected_case_insensitively() {
        assert!(has_transfer_encoding(
            "POST /mcp HTTP/1.1\r\nTransfer-Encoding: chunked"
        ));
        assert!(has_transfer_encoding(
            "POST /mcp HTTP/1.1\r\ntransfer-encoding: identity"
        ));
        assert!(!has_transfer_encoding(SAMPLE_HEAD));
    }

    #[test]
    fn health_body_names_the_live_server() {
        let body = health_body();
        assert_eq!(body["server"], "stykanimate-rs");
        assert!(body["version"].as_str().is_some_and(|v| !v.is_empty()));
        assert_eq!(body["transport"], "live-http");
        assert_eq!(body["live"], true);
        assert_eq!(body["endpoint"], endpoint());
        // Wave 14 (D4): endpoint state, from the same
        // sources the listener itself uses — the PORT
        // constant it binds and the ENABLED atomic that is
        // set only after a successful bind. `enabled` is
        // pinned by self-consistency, not a fixed value:
        // unit tests never start the bridge, and the field's
        // contract is that it reports the flag truthfully,
        // whatever the flag currently reads.
        assert_eq!(body["port"], json!(PORT), "health names the bound port");
        assert_eq!(
            body["enabled"],
            json!(is_enabled()),
            "health reports the enabled flag, not a hard-coded state"
        );
        assert_eq!(body["auth"], "none");
        // Wave 14 (D4, landed at integration): shell
        // control exists in this build (the shell queue
        // drains in poll), so the field is present and
        // true. D4's original pin held it absent until
        // the plumbing landed — this pin is that
        // landing, made deliberate.
        assert_eq!(
            body["shell_control"],
            json!(true),
            "shell control is compiled in and drained by poll"
        );
        // The tool count is the registry's own number — the
        // same value the `server_info` tool reports — not a
        // literal that can drift as tools are added.
        assert_eq!(
            body["tools"],
            json!(stykanimate_rs::mcp::all_tools().len()),
            "health names the live tool count"
        );
        assert!(body["tools"].as_u64().is_some_and(|n| n > 0));
    }

    // -----------------------------------------------------------------
    // Wave 13 audit pins — drain attribution + health count.
    //
    // The sprite tools and the Squad B dirty-seam paths land
    // outside this module; what this module must guarantee
    // is that (1) a drained request's error surfaces by the
    // one uniform rule whatever tool produced it, and (2)
    // the health document's tool count is the served
    // surface's count, derived — never a literal.
    // -----------------------------------------------------------------

    #[test]
    fn drain_attribution_surfaces_protocol_errors_that_never_wrote_status() {
        // The shape dispatch builds for an unroutable call
        // (an unknown tool name earns -32602): no result,
        // the problem named in `error.message`. Protocol
        // errors never write the status line, so the drain
        // buffer is their only route to the user.
        let resp = Some(json!({
            "jsonrpc": "2.0",
            "id": 5,
            "error": { "code": -32602, "message": "unknown tool: sprite_frobnicate" },
        }));
        assert_eq!(
            drain_attribution_error("Ready", "Ready", &resp).as_deref(),
            Some("unknown tool: sprite_frobnicate"),
            "a status-silent protocol error is buffered for the MCP toast"
        );
    }

    #[test]
    fn drain_attribution_surfaces_status_silent_tool_refusals() {
        // The shape a `tools/call` refusal takes when its
        // handler returned an error WITHOUT writing
        // core.status (the sprite handlers' refusal shape is
        // this same dispatch shape — dispatch builds it
        // uniformly for every tool): isError plus the
        // refusal text as the first content block.
        let resp = Some(json!({
            "jsonrpc": "2.0",
            "id": 6,
            "result": {
                "isError": true,
                "content": [{ "type": "text", "text": "no sprite at index 7" }],
            },
        }));
        assert_eq!(
            drain_attribution_error("Ready", "Ready", &resp).as_deref(),
            Some("no sprite at index 7"),
            "a status-silent refusal is buffered with its own text"
        );
    }

    #[test]
    fn drain_attribution_leaves_status_writing_refusals_to_the_shell() {
        // The SAME refusal shape, but the handler wrote the
        // status line (the ordinary refusal): the shell
        // toasts from the status itself, so the drain buffer
        // must stay out of it — no double toast. This is the
        // half of the rule that keeps behaviour identical
        // for the write paths Squad B re-routed: whether an
        // op writes status or not, exactly one surface
        // reports its refusal.
        let resp = Some(json!({
            "jsonrpc": "2.0",
            "id": 7,
            "result": {
                "isError": true,
                "content": [{ "type": "text", "text": "Select a sprite first." }],
            },
        }));
        assert_eq!(
            drain_attribution_error("Ready", "Select a sprite first.", &resp),
            None,
            "a status-writing refusal is the shell's toast, not the drain buffer's"
        );
        // A protocol error alongside a changed status
        // cannot occur (protocol errors never touch the
        // core), but the rule stays total: changed status
        // always means hands off.
        let err = Some(json!({
            "jsonrpc": "2.0",
            "id": 8,
            "error": { "code": -32601, "message": "Method not found: x" },
        }));
        assert_eq!(drain_attribution_error("Ready", "Busy", &err), None);
    }

    #[test]
    fn drain_attribution_ignores_successes_notifications_and_textless_errors() {
        // A successful call — sprite or otherwise — has
        // nothing to attribute.
        let ok = Some(json!({
            "jsonrpc": "2.0",
            "id": 9,
            "result": { "isError": false, "content": [{ "type": "text", "text": "{}" }] },
        }));
        assert_eq!(drain_attribution_error("Ready", "Ready", &ok), None);
        // A notification earns no response at all (None):
        // nothing to attribute.
        assert_eq!(drain_attribution_error("Ready", "Ready", &None), None);
        // An isError result whose first content block is not
        // text carries no attributable string; the caller
        // still gets the full result over HTTP — only the
        // toast has nothing to say.
        let textless = Some(json!({
            "jsonrpc": "2.0",
            "id": 10,
            "result": { "isError": true, "content": [] },
        }));
        assert_eq!(drain_attribution_error("Ready", "Ready", &textless), None);
    }

    #[test]
    fn health_tool_count_is_the_served_tool_count() {
        // The Wave 13 pin for the health document: its
        // `tools` number must equal the number of tools a
        // `tools/list` through the REAL dispatch actually
        // serves — so when the integrator's splice adds the
        // five sprite registry entries, the health body
        // follows with no edit here. No count literal is
        // asserted anywhere: the registry is the only
        // source, read three ways that must agree.
        let mut core = EditorCore::new();
        let mut router = McpServer::new();
        let resp = router
            .handle_on(
                &mut core,
                &json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {} }),
            )
            .expect("tools/list is always answered");
        let served = resp["result"]["tools"]
            .as_array()
            .expect("tools/list carries a tools array")
            .len();
        assert!(served > 0);
        assert_eq!(
            health_body()["tools"],
            json!(served),
            "health reports the served surface, not a stored number"
        );
        assert_eq!(served, stykanimate_rs::mcp::all_tools().len());
    }

    #[test]
    fn is_notification_mirrors_the_dispatch_rule() {
        // Object without an `id` member: the only notification.
        assert!(is_notification(&json!({ "jsonrpc": "2.0", "method": "ping" })));
        assert!(is_notification(&json!({ "method": "notifications/initialized" })));
        // An explicit null id is a REQUEST — dispatch answers
        // it (with id null) on both transports.
        assert!(!is_notification(&json!({ "id": null, "method": "ping" })));
        assert!(!is_notification(&json!({ "id": 0, "method": "ping" })));
        assert!(!is_notification(&json!({ "id": "abc", "method": "ping" })));
        // Non-objects earn a -32600 from dispatch, so they are
        // requests to be queued, not notifications.
        assert!(!is_notification(&json!([{ "id": 1, "method": "ping" }])));
        assert!(!is_notification(&json!(42)));
        assert!(!is_notification(&json!("ping")));
        assert!(!is_notification(&Value::Null));
    }

    #[test]
    fn is_notification_covers_erroring_and_batch_shapes() {
        // Wave 14 (D6): the HTTP fast path's two edges.
        //
        // Erroring notifications are still notifications:
        // the listener answers them 202 with an empty body
        // BEFORE queueing (handle_connection), exactly like
        // well-formed ones — an unknown method, an unknown
        // tool, a tools/call with no params, even an object
        // with no method at all. Nothing is queued, so
        // nothing can execute, and no error body is ever
        // produced for them (the dispatch-level half of
        // this truth is pinned in mcp.rs's D6 tests, and
        // the 202 itself over the wire by the e2e below).
        assert!(is_notification(&json!({ "jsonrpc": "2.0", "method": "no/such" })));
        assert!(is_notification(&json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": { "name": "does_not_exist", "arguments": {} },
        })));
        assert!(is_notification(&json!({ "jsonrpc": "2.0", "method": "tools/call" })));
        assert!(is_notification(&json!({ "jsonrpc": "2.0" })));
        // Batch arrays are NEVER notifications — not even
        // an all-notification batch, and not the empty
        // batch: the array shape is refused by dispatch
        // with a single -32600, so it must be queued and
        // answered (HTTP 200 + that error body, as the
        // e2e pins), never fast-pathed to a silent 202.
        assert!(!is_notification(&json!([
            { "jsonrpc": "2.0", "method": "ping" },
            { "jsonrpc": "2.0", "method": "notifications/initialized" },
        ])));
        assert!(!is_notification(&json!([])));
    }

    #[test]
    fn content_type_acceptance_is_json_or_absent() {
        assert!(content_type_acceptable(SAMPLE_HEAD));
        assert!(content_type_acceptable(
            "POST /mcp HTTP/1.1\r\nContent-Type: application/json; charset=utf-8"
        ));
        assert!(content_type_acceptable(
            "POST /mcp HTTP/1.1\r\ncontent-type: Application/JSON"
        ));
        // Absent is tolerated (minimal clients); a false claim
        // is not.
        assert!(content_type_acceptable("POST /mcp HTTP/1.1\r\nHost: x"));
        assert!(!content_type_acceptable(
            "POST /mcp HTTP/1.1\r\nContent-Type: text/plain"
        ));
        assert!(!content_type_acceptable(
            "POST /mcp HTTP/1.1\r\nContent-Type: text/json"
        ));
        assert!(!content_type_acceptable(
            "POST /mcp HTTP/1.1\r\nContent-Type: application/json2"
        ));
    }

    /// Build a request head from an optional Host value and
    /// an optional Origin value, for the gate matrix below.
    fn gated_head(host: Option<&str>, origin: Option<&str>) -> String {
        let mut head = String::from("POST /mcp HTTP/1.1\r\n");
        if let Some(h) = host {
            head.push_str(&format!("Host: {h}\r\n"));
        }
        if let Some(o) = origin {
            head.push_str(&format!("Origin: {o}\r\n"));
        }
        head.push_str("Content-Type: application/json\r\nContent-Length: 2\r\n\r\n");
        head
    }

    #[test]
    fn host_origin_gate_allows_loopback_spellings() {
        // The spellings local clients actually send: bare
        // host and host:port, IPv4, name and IPv6, any case,
        // header name in any case.
        for host in [
            "127.0.0.1",
            "127.0.0.1:32117",
            "localhost",
            "localhost:32117",
            "LOCALHOST",
            "LocalHost:8080",
            "[::1]",
            "[::1]:32117",
        ] {
            assert!(
                host_origin_allowed(&gated_head(Some(host), None)),
                "loopback Host {host} must pass the gate"
            );
        }
        assert!(host_origin_allowed(
            "POST /mcp HTTP/1.1\r\nhost: 127.0.0.1\r\n\r\n"
        ));
        // The head the module's own sample carries passes.
        assert!(host_origin_allowed(SAMPLE_HEAD));
        // Absent Origin is the non-browser case: allowed.
        // A loopback Origin (a local page served from this
        // machine) is allowed on any port, over https too.
        for origin in [
            "http://127.0.0.1:32117",
            "http://localhost:8080",
            "https://[::1]:32117",
            "http://LOCALHOST",
        ] {
            assert!(
                host_origin_allowed(&gated_head(Some("127.0.0.1"), Some(origin))),
                "loopback Origin {origin} must pass the gate"
            );
        }
    }

    #[test]
    fn host_origin_gate_refuses_foreign_and_missing() {
        // Missing Host: HTTP/1.1 requires it and every real
        // client sends it — absence refuses.
        assert!(!host_origin_allowed(&gated_head(None, None)));
        assert!(!host_origin_allowed("GET /health HTTP/1.1\r\n\r\n"));
        // Foreign Host spellings, including the ones that
        // merely CONTAIN a loopback spelling: a whitelist,
        // not a substring or resolver check.
        for host in [
            "evil.example",
            "evil.example:32117",
            "127.0.0.1.evil.example",
            "localhost.evil.example",
            "localhost.",
            "127.1",
            "0.0.0.0",
            "[::2]",
            "[::1",
            "::1",
            "127.0.0.1:abc",
            "127.0.0.1:",
            "",
            "user@127.0.0.1",
        ] {
            assert!(
                !host_origin_allowed(&gated_head(Some(host), None)),
                "Host {host:?} must refuse at the gate"
            );
        }
        // A second, foreign Host header poisons an otherwise
        // loopback request (every value must pass).
        assert!(!host_origin_allowed(
            "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nHost: evil.example\r\n\r\n"
        ));
        // Foreign Origin refuses even behind a loopback
        // Host: this is the cross-origin web-page shape.
        for origin in [
            "https://evil.example",
            "http://evil.example:32117",
            "http://127.0.0.1.evil.example",
            "null",
            "file://127.0.0.1",
            "http://user@127.0.0.1",
            "http://127.0.0.1/some/path",
            "127.0.0.1",
            "",
        ] {
            assert!(
                !host_origin_allowed(&gated_head(Some("localhost"), Some(origin))),
                "Origin {origin:?} must refuse at the gate"
            );
        }
    }

    #[test]
    fn foreign_host_is_refused_403_before_anything_is_queued() {
        // End to end through the real handler: the refusal
        // is an HTTP 403 and the request never reaches the
        // queue (no tool dispatch is reachable behind it).
        let (port, rx, server) = spawn_one_shot_server();
        let mut client = connect_client(port);
        let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" }).to_string();
        let request = format!(
            "POST /mcp HTTP/1.1\r\nHost: evil.example\r\n\
             Content-Type: application/json\r\nContent-Length: {}\r\n\
             Connection: close\r\n\r\n{body}",
            body.len()
        );
        client.write_all(request.as_bytes()).expect("request write");
        let (status, _) = read_one_response(&mut client);
        assert_eq!(status, 403, "a foreign Host is refused 403");
        server.join().expect("the server finishes after refusing");
        assert!(
            rx.try_recv().is_err(),
            "a gated request must never be queued for dispatch"
        );
    }

    #[test]
    fn foreign_origin_is_refused_403_before_anything_is_queued() {
        // The DNS-rebinding twin: loopback Host (the socket
        // really is local) but a web page's Origin — refused
        // the same way, before queueing.
        let (port, rx, server) = spawn_one_shot_server();
        let mut client = connect_client(port);
        let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" }).to_string();
        let request = format!(
            "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\n\
             Origin: https://evil.example\r\n\
             Content-Type: application/json\r\nContent-Length: {}\r\n\
             Connection: close\r\n\r\n{body}",
            body.len()
        );
        client.write_all(request.as_bytes()).expect("request write");
        let (status, _) = read_one_response(&mut client);
        assert_eq!(status, 403, "a foreign Origin is refused 403");
        server.join().expect("the server finishes after refusing");
        assert!(
            rx.try_recv().is_err(),
            "a gated request must never be queued for dispatch"
        );
    }

    #[test]
    fn too_large_body_is_a_json_rpc_error_with_null_id() {
        let body: Value = serde_json::from_str(&body_too_large_body()).expect("valid JSON");
        assert_eq!(body["jsonrpc"], "2.0");
        assert_eq!(body["id"], Value::Null);
        assert_eq!(body["error"]["code"], -32600);
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .contains(&MAX_BODY.to_string()),
            "the message names the limit it enforces"
        );
    }

    #[test]
    fn waits_are_bounded_and_ordered() {
        // The read deadline is a real bound (not zero, not
        // effectively infinite), and the reply wait must far
        // outlast it: a slow REQUEST is a framing problem,
        // while a slow REPLY is a tool doing real work.
        assert!(READ_DEADLINE >= Duration::from_secs(5));
        assert!(READ_DEADLINE <= Duration::from_secs(120));
        assert!(REPLY_TIMEOUT > READ_DEADLINE);
    }

    // -----------------------------------------------------------------
    // Wave 14 (D8) — connection lifecycle pins.
    //
    // The pure tests pin the write deadline; the socket
    // tests run the REAL `handle_connection` on an
    // ephemeral port against a private request queue this
    // test drains by hand. Nothing here touches the
    // singleton bridge or the real PORT, so these run
    // alongside the socket e2e below without interference.
    // -----------------------------------------------------------------

    #[test]
    fn write_deadline_is_a_total_bound_not_a_per_write_one() {
        // At least one full per-write timeout, so a healthy
        // slow-ish loopback client is never cut off early;
        // far below the reply wait, so a trickle reader
        // frees the listener long before its request would
        // have timed out waiting.
        assert!(WRITE_DEADLINE >= IO_TIMEOUT);
        assert!(WRITE_DEADLINE <= Duration::from_secs(300));
        assert!(WRITE_DEADLINE < REPLY_TIMEOUT);
    }

    /// A connected pair of streams over an ephemeral
    /// loopback listener: `(client_end, server_end)`.
    fn stream_pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("ephemeral bind");
        let port = listener.local_addr().expect("local addr").port();
        let client = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        let (server, _) = listener.accept().expect("accept");
        (client, server)
    }

    #[test]
    fn write_bounded_gives_up_on_an_expired_deadline() {
        let (mut writer, mut reader) = stream_pair();
        let past = Instant::now() - Duration::from_secs(1);
        assert!(
            !write_bounded(&mut writer, b"hello", past),
            "an already-spent deadline must refuse the write, not start it"
        );
        // And nothing reached the peer.
        reader
            .set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        let mut byte = [0u8; 1];
        assert!(
            !matches!(reader.read(&mut byte), Ok(1)),
            "no byte may be written under an expired deadline"
        );
    }

    #[test]
    fn write_bounded_reports_a_dead_peer_without_panicking() {
        let (mut writer, reader) = stream_pair();
        drop(reader); // the peer is gone before a byte is written
        let mut failed = false;
        for _ in 0..100 {
            let deadline = Instant::now() + WRITE_DEADLINE;
            if !write_bounded(&mut writer, &[0u8; 8192], deadline) {
                failed = true;
                break;
            }
        }
        assert!(failed, "writes to a closed peer must fail, never hang or panic");
    }

    /// Spawn a one-shot server on an ephemeral port: accept
    /// ONE connection and serve it with the real
    /// `handle_connection` against a fresh queue. Returns
    /// the port, the queue receiver the test drains by
    /// hand, and the server thread (joined by each test —
    /// a server that does not finish is the failure these
    /// tests exist to catch).
    fn spawn_one_shot_server() -> (
        u16,
        std::sync::mpsc::Receiver<LiveRequest>,
        std::thread::JoinHandle<()>,
    ) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("ephemeral bind");
        let port = listener.local_addr().expect("local addr").port();
        let (tx, rx) = sync_channel::<LiveRequest>(MAX_QUEUED_REQUESTS);
        let stop = Arc::new(AtomicBool::new(false));
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept one connection");
            handle_connection(stream, &tx, &stop);
        });
        (port, rx, server)
    }

    fn connect_client(port: u16) -> TcpStream {
        let stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("client read timeout");
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .expect("client write timeout");
        stream
    }

    fn post_bytes(msg: &Value) -> Vec<u8> {
        let body = msg.to_string();
        format!(
            "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .into_bytes()
    }

    /// Read exactly one HTTP response from `client` (head,
    /// then `Content-Length` body bytes) and return its
    /// status and body.
    fn read_one_response(client: &mut TcpStream) -> (u16, String) {
        let mut raw: Vec<u8> = Vec::new();
        let mut byte = [0u8; 1];
        let head_len = loop {
            if let Some((head, _)) = split_head_body(&raw) {
                break head.len() + 4;
            }
            let n = client.read(&mut byte).expect("read response head");
            assert_eq!(n, 1, "connection closed inside the response head");
            raw.push(byte[0]);
        };
        let head = String::from_utf8_lossy(&raw[..head_len]).into_owned();
        let status: u16 = head
            .split_whitespace()
            .nth(1)
            .and_then(|t| t.parse().ok())
            .expect("response has a status");
        let len = parse_content_length(&head).expect("response carries Content-Length");
        let mut body: Vec<u8> = raw[head_len..].to_vec();
        assert!(body.len() <= len, "more body bytes arrived than were framed");
        let mut rest = vec![0u8; len - body.len()];
        client.read_exact(&mut rest).expect("read response body");
        body.extend_from_slice(&rest);
        (status, String::from_utf8_lossy(&body).into_owned())
    }

    #[test]
    fn disconnect_mid_head_queues_nothing_and_frees_the_server() {
        let (port, rx, server) = spawn_one_shot_server();
        let mut client = connect_client(port);
        client
            .write_all(b"POST /mcp HTTP/1.1\r\nContent-Len")
            .expect("partial head write");
        drop(client); // gone mid-head: the terminator never arrives
        server
            .join()
            .expect("the server must let go of a client that went away mid-head");
        assert!(
            rx.try_recv().is_err(),
            "a request that never fully arrived must not be queued"
        );
    }

    #[test]
    fn truncated_body_queues_nothing_and_frees_the_server() {
        let (port, rx, server) = spawn_one_shot_server();
        let mut client = connect_client(port);
        // A complete head promising 100 body bytes, then a
        // tenth of the body, then a disconnect: the framing
        // fails, so nothing may be queued or executed. (The
        // Host is a real loopback one so the request passes
        // the Host/Origin gate and it is the BODY framing
        // this test pins that refuses it.)
        client
            .write_all(
                b"POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: 100\r\n\r\n{\"jsonrpc\"",
            )
            .expect("head + partial body write");
        drop(client);
        server
            .join()
            .expect("the server must let go of a truncated body");
        assert!(
            rx.try_recv().is_err(),
            "a truncated body must not be queued"
        );
    }

    #[test]
    fn disconnect_after_sending_does_not_cancel_the_queued_request() {
        // The Wave 14 (D8) ruling, pinned: a client that
        // hangs up AFTER its request was queued does not
        // cancel it. A disconnect is indistinguishable from
        // a legitimate half-close (a client that shuts down
        // its write side and keeps reading), so treating it
        // as cancellation would break real clients; the
        // transport's cancellations are only the ones the
        // client can be told about — the reply timeout's 504
        // and the bridge stop. The edit executes, and the
        // reply write to the dead socket is dropped quietly.
        let (port, rx, server) = spawn_one_shot_server();
        let mut core = EditorCore::new();
        let mut router = McpServer::new();
        assert!(core.project.frames.len() >= 2 && core.current == 0);
        let mut client = connect_client(port);
        let msg = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": "set_frame", "arguments": { "frame": 1 } },
        });
        client.write_all(&post_bytes(&msg)).expect("request write");
        drop(client); // the client is gone before the app drains
        let req = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("a fully sent request is queued even though its client left");
        let req = discard_if_cancelled(req)
            .expect("a disconnect is not a cancellation: the request stays live");
        let response = router.handle_on(&mut core, &req.msg);
        assert_eq!(
            core.current, 1,
            "the mutation executes even though its client hung up while waiting"
        );
        // The reply lands in the wait loop, whose write to
        // the dead socket fails and is ignored.
        let _ = req.reply.send(response);
        server
            .join()
            .expect("the listener must survive writing a reply to a dead client");
    }

    #[test]
    fn one_connection_serves_exactly_one_request() {
        // Keep-alive is not offered: every response carries
        // `Connection: close` and the socket is closed after
        // the first exchange, so a second request on the
        // same connection gets no answer — the contract
        // pipelining clients must see, pinned.
        let (port, rx, server) = spawn_one_shot_server();
        let mut core = EditorCore::new();
        let mut router = McpServer::new();
        let mut client = connect_client(port);
        let ping = json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" });
        client.write_all(&post_bytes(&ping)).expect("first request");
        let req = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("first request queued");
        let req = discard_if_cancelled(req).expect("first request is live");
        let response = router.handle_on(&mut core, &req.msg);
        let _ = req.reply.send(response);
        let (status, body) = read_one_response(&mut client);
        assert_eq!(status, 200);
        let resp: Value = serde_json::from_str(&body).expect("first response is JSON");
        assert_eq!(resp["result"], json!({}), "the first request was answered");
        // Second request on the SAME connection: never
        // queued, never answered; the read side sees the
        // close the first response promised.
        let _ = client.write_all(&post_bytes(&ping));
        let queued = rx.recv_timeout(Duration::from_millis(300));
        assert!(queued.is_err(), "a second request on one connection is not served");
        let mut byte = [0u8; 1];
        match client.read(&mut byte) {
            Ok(0) | Err(_) => {} // FIN or reset: the promised close
            Ok(n) => panic!("second response byte(s) on a closed connection: {n}"),
        }
        server
            .join()
            .expect("the server finishes after its one request");
    }

    // -----------------------------------------------------------------
    // End-to-end proof — the suite's end-to-end socket test.
    //
    // Everything above tests pure helpers; this one binds
    // the real port, speaks real HTTP from a client thread,
    // and drives a real external EditorCore by draining the
    // queue exactly the way the app's poll loop does. The
    // bridge singleton is process-global, so the run is
    // wrapped to guarantee cleanup (bridge stopped) on every
    // path — an enabled bridge or bound port must never leak
    // into the rest of the suite.
    // -----------------------------------------------------------------

    /// POST one JSON-RPC message to the live endpoint over a
    /// raw socket (the HTTP request built by hand, like a
    /// minimal MCP client would) and return the response's
    /// HTTP status code and raw body. Every operation
    /// carries a timeout, so a broken bridge fails this test
    /// instead of hanging CI.
    fn post_raw(msg: &Value) -> Result<(u16, String), String> {
        use std::io::{Read, Write};
        let body = msg.to_string();
        let mut stream = TcpStream::connect(("127.0.0.1", PORT))
            .map_err(|e| format!("connect: {e}"))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .map_err(|e| format!("set read timeout: {e}"))?;
        stream
            .set_write_timeout(Some(Duration::from_secs(10)))
            .map_err(|e| format!("set write timeout: {e}"))?;
        let request = format!(
            "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1:{PORT}\r\n\
             Content-Type: application/json\r\nContent-Length: {}\r\n\
             Connection: close\r\n\r\n{body}",
            body.len()
        );
        stream
            .write_all(request.as_bytes())
            .map_err(|e| format!("write: {e}"))?;
        let mut raw = String::new();
        stream
            .read_to_string(&mut raw)
            .map_err(|e| format!("read: {e}"))?;
        let status: u16 = raw
            .split_whitespace()
            .nth(1)
            .and_then(|token| token.parse().ok())
            .ok_or_else(|| format!("response had no status line: {}", raw.lines().next().unwrap_or("")))?;
        let (_, resp_body) = raw
            .split_once("\r\n\r\n")
            .ok_or("response had no header/body split")?;
        Ok((status, resp_body.to_string()))
    }

    /// [`post_raw`] specialised to the normal JSON-RPC case:
    /// HTTP 200 with a JSON response body.
    fn post_json(msg: &Value) -> Result<Value, String> {
        let (status, resp_body) = post_raw(msg)?;
        if status != 200 {
            return Err(format!("unexpected response status: {status}"));
        }
        serde_json::from_str(&resp_body).map_err(|e| format!("response was not JSON: {e}"))
    }

    /// Run one request/response exchange: the client thread
    /// POSTs and blocks on the reply while THIS thread
    /// drains the queue against `core` — the app's poll
    /// loop, mimicked. Bounded by `deadline`; on timeout
    /// the client thread is left to expire on its own
    /// socket timeouts (the caller's cleanup stops the
    /// bridge, which closes its wait early).
    fn exchange(
        core: &mut EditorCore,
        router: &mut McpServer,
        msg: Value,
        deadline: Duration,
    ) -> Result<Value, String> {
        let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
        let client = std::thread::spawn(move || {
            let res = post_json(&msg);
            let _ = done_tx.send(());
            res
        });
        let start = std::time::Instant::now();
        while done_rx.try_recv().is_err() {
            if start.elapsed() > deadline {
                return Err("client exchange timed out".to_string());
            }
            drain(core, router, 8);
            std::thread::sleep(Duration::from_millis(2));
        }
        client
            .join()
            .map_err(|_| "client thread panicked".to_string())?
    }

    /// [`exchange`] over [`post_raw`]: for the exchanges
    /// whose expected outcome is an HTTP status rather than
    /// a JSON-RPC body (the notification's 202).
    fn exchange_raw(
        core: &mut EditorCore,
        router: &mut McpServer,
        msg: Value,
        deadline: Duration,
    ) -> Result<(u16, String), String> {
        let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
        let client = std::thread::spawn(move || {
            let res = post_raw(&msg);
            let _ = done_tx.send(());
            res
        });
        let start = std::time::Instant::now();
        while done_rx.try_recv().is_err() {
            if start.elapsed() > deadline {
                return Err("client exchange timed out".to_string());
            }
            drain(core, router, 8);
            std::thread::sleep(Duration::from_millis(2));
        }
        client
            .join()
            .map_err(|_| "client thread panicked".to_string())?
    }

    /// The e2e run proper, returning its verdict so the
    /// test wrapper can stop the bridge before surfacing
    /// any failure.
    fn run_live_bridge_e2e() -> Result<(), String> {
        // Start from a known state: another test's `init`
        // may already have enabled the singleton (harmless —
        // set_enabled is idempotent), and a stop/start also
        // proves the toggle path the settings card drives.
        let _ = set_enabled(false);
        if !set_enabled(true) {
            return Err("bridge did not start — port 32117 is already in use".to_string());
        }
        // The test's own core + router, standing in for the
        // app's LIVE core and poll's thread-local router.
        let mut core = EditorCore::new();
        let mut router = McpServer::new();
        if core.project.frames.len() < 2 {
            return Err("the demo project needs at least two frames for the playhead proof".to_string());
        }
        if core.current != 0 {
            return Err("a fresh core must start on frame 0".to_string());
        }

        // 1. `initialize` over the wire names the server.
        let init = exchange(
            &mut core,
            &mut router,
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": { "name": "live-mcp-e2e", "version": "0" },
                },
            }),
            Duration::from_secs(10),
        )?;
        let name = init
            .pointer("/result/serverInfo/name")
            .and_then(Value::as_str)
            .unwrap_or("");
        if name != "stykanimate-rs" {
            return Err(format!(
                "initialize named the server {name:?}, expected \"stykanimate-rs\""
            ));
        }

        // 2. `tools/call set_frame` must move THE TEST's
        //    core — the whole point of live control.
        let call = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": { "name": "set_frame", "arguments": { "frame": 1 } },
        });
        let budget = std::time::Instant::now() + Duration::from_secs(30);
        loop {
            let resp = exchange(&mut core, &mut router, call.clone(), Duration::from_secs(10))?;
            if let Some(err) = resp.get("error") {
                return Err(format!("set_frame returned a JSON-RPC error: {err}"));
            }
            let is_error = resp
                .pointer("/result/isError")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if is_error {
                return Err(format!("set_frame reported a tool error: {resp}"));
            }
            if core.current == 1 {
                break;
            }
            // The reply was fine but THIS core did not move:
            // a snap test's Runtime drained the request
            // against ITS core (snap tests install the real
            // `init` + `poll`, and test threads run in
            // parallel). In the app there is exactly one
            // State and one poll, so this cannot happen —
            // retry until this test's own drain wins a round.
            if std::time::Instant::now() > budget {
                return Err("set_frame never landed on the test's own core within 30 s".to_string());
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        // 3. The router's private core must be untouched:
        //    `handle_on` aimed every call at the external
        //    core, never at the dispatcher's own.
        if router.core.current != 0 {
            return Err("the router's own core moved — dispatch hit the wrong core".to_string());
        }

        // 4. Protocol parity over the wire — the same shapes
        //    the stdio shell produces, because both transports
        //    share the one dispatch:
        //    a string id is echoed verbatim, and `ping`
        //    answers an empty result object.
        let ping = exchange(
            &mut core,
            &mut router,
            json!({ "jsonrpc": "2.0", "id": "abc-1", "method": "ping" }),
            Duration::from_secs(10),
        )?;
        if ping["id"] != json!("abc-1") || ping["result"] != json!({}) {
            return Err(format!("ping over HTTP diverged from dispatch: {ping}"));
        }
        //    An explicit null id is a request, not a
        //    notification: it must be answered, with id null.
        let null_id = exchange(
            &mut core,
            &mut router,
            json!({ "jsonrpc": "2.0", "id": null, "method": "ping" }),
            Duration::from_secs(10),
        )?;
        if null_id["id"] != Value::Null || null_id["result"] != json!({}) {
            return Err(format!("null-id request was not answered as a request: {null_id}"));
        }
        //    An unknown method earns -32601, same as stdio.
        let unknown = exchange(
            &mut core,
            &mut router,
            json!({ "jsonrpc": "2.0", "id": 7, "method": "no/such" }),
            Duration::from_secs(10),
        )?;
        if unknown.pointer("/error/code").and_then(Value::as_i64) != Some(-32601) {
            return Err(format!("unknown method did not earn -32601: {unknown}"));
        }
        //    A batch array is not unrolled by dispatch: one
        //    -32600 object with a null id — an honest protocol
        //    error over HTTP, never a hang or a dropped socket.
        let batch = exchange(
            &mut core,
            &mut router,
            json!([
                { "jsonrpc": "2.0", "id": 9, "method": "ping" },
                { "jsonrpc": "2.0", "id": 10, "method": "ping" },
            ]),
            Duration::from_secs(10),
        )?;
        if batch.is_array()
            || batch.pointer("/error/code").and_then(Value::as_i64) != Some(-32600)
            || batch["id"] != Value::Null
        {
            return Err(format!("batch was not the single -32600 dispatch gives stdio: {batch}"));
        }
        //    A notification earns no JSON-RPC body at all:
        //    HTTP 202 with an empty body.
        let (status, body) = exchange_raw(
            &mut core,
            &mut router,
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            Duration::from_secs(10),
        )?;
        if status != 202 || !body.is_empty() {
            return Err(format!(
                "notification earned HTTP {status} with body {body:?}, expected 202 and empty"
            ));
        }
        Ok(())
    }

    #[test]
    fn live_bridge_drives_an_external_core_over_http() {
        let _bridge_guard =
            super::BRIDGE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let verdict = std::panic::catch_unwind(run_live_bridge_e2e);
        // Cleanup on EVERY path (failure and panic alike),
        // before the verdict is surfaced.
        let _ = set_enabled(false);
        assert!(!is_enabled(), "cleanup must leave the bridge off");
        match verdict {
            Ok(Ok(())) => {}
            Ok(Err(msg)) => panic!("live bridge e2e: {msg}"),
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    // -----------------------------------------------------------------
    // Start-failure memory (Wave 14, D1) — the suite's second
    // socket-binding test, and a short one: it binds the real
    // port only to hold it hostage (the busy-port case), never
    // to speak HTTP. Same singleton discipline as the e2e
    // above: known state first, cleanup on every path.
    // -----------------------------------------------------------------

    /// The run proper, returning its verdict so the test
    /// wrapper can stop the bridge before surfacing any
    /// failure (the e2e's shape, for the same reason).
    fn run_start_error_recording() -> Result<(), String> {
        // Known state first (the e2e opens the same way):
        // bridge off — and a stop must already have cleared
        // whatever a previous run recorded.
        let _ = set_enabled(false);
        if let Some(err) = last_start_error() {
            return Err(format!("a stop left a stale start error behind: {err:?}"));
        }
        // Hold the port hostage with a listener of our own —
        // the second-editor-instance case the launch path
        // hits, forced the honest way.
        let occupier = TcpListener::bind(("127.0.0.1", PORT))
            .map_err(|e| format!("the test could not occupy the port itself: {e}"))?;
        if set_enabled(true) {
            return Err("a start against an occupied port reported success".to_string());
        }
        if is_enabled() {
            return Err("the bridge claims to be on after a failed start".to_string());
        }
        let recorded = last_start_error()
            .ok_or_else(|| "a failed start recorded no error".to_string())?;
        if !recorded.contains("in use") {
            return Err(format!(
                "the recorded error does not name the cause: {recorded:?}"
            ));
        }
        // A stop clears the record even though nothing was
        // running — the note must not outlive the user's
        // decision to switch off.
        let _ = set_enabled(false);
        if let Some(err) = last_start_error() {
            return Err(format!("a stop left the recorded error behind: {err:?}"));
        }
        // Free the port and retry: the start succeeds and
        // the record clears with it.
        drop(occupier);
        if !set_enabled(true) {
            return Err("the retry failed even with the port free".to_string());
        }
        if let Some(err) = last_start_error() {
            return Err(format!(
                "a successful start left the old error behind: {err:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn failed_start_records_its_error_and_success_or_stop_clears_it() {
        let _bridge_guard =
            super::BRIDGE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let verdict = std::panic::catch_unwind(run_start_error_recording);
        // Cleanup on EVERY path (failure and panic alike),
        // before the verdict is surfaced.
        let _ = set_enabled(false);
        assert!(!is_enabled(), "cleanup must leave the bridge off");
        match verdict {
            Ok(Ok(())) => {}
            Ok(Err(msg)) => panic!("start-error recording: {msg}"),
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
}
