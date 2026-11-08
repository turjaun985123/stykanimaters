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
//! ## Security — read this before enabling
//!
//! The listener binds `127.0.0.1` only, so nothing off this
//! machine can reach it. **There is NO authentication in v1:**
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
use std::sync::mpsc::{channel, Receiver, Sender, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use serde_json::{json, Value};

use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::mcp::McpServer;

/// The fixed loopback port the live endpoint listens on.
pub const PORT: u16 = 32117;

/// Largest request body accepted: 4 MiB. Bigger bodies are
/// refused with HTTP 413 before a byte of them is read.
const MAX_BODY: usize = 4 * 1024 * 1024;

/// Largest request head (request line + headers) accepted.
const MAX_HEAD: usize = 16 * 1024;

/// How long a single connection may take to deliver its
/// request / absorb its response before the listener gives up
/// on it — the listener serves connections one at a time, so a
/// stuck client must not starve every later one.
const IO_TIMEOUT: Duration = Duration::from_secs(30);

/// The URL an MCP client should be pointed at while live
/// control is enabled: `http://127.0.0.1:32117/mcp`.
pub fn endpoint() -> String {
    format!("http://127.0.0.1:{PORT}/mcp")
}

/// One parsed JSON-RPC message on its way to the UI thread,
/// with the channel its response must be sent back on.
/// `Some(value)` is the JSON-RPC response; `None` means the
/// message was a notification and gets no body (HTTP 202).
pub struct LiveRequest {
    pub msg: Value,
    pub reply: SyncSender<Option<Value>>,
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
        let (tx, rx) = channel::<LiveRequest>();
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
pub fn set_enabled(on: bool) -> bool {
    if on == is_enabled() {
        return on;
    }
    if on {
        match LiveBridge::start() {
            Ok(bridge) => {
                *lock_bridge() = Some(bridge);
                ENABLED.store(true, Ordering::SeqCst);
                true
            }
            Err(_) => false,
        }
    } else {
        if let Some(mut bridge) = lock_bridge().take() {
            bridge.stop();
        }
        ENABLED.store(false, Ordering::SeqCst);
        false
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
        let response = router.handle_on(core, &req.msg);
        // A send failure only means the client hung up while
        // waiting — the edit still happened; nothing to undo.
        let _ = req.reply.send(response);
        done += 1;
    }
    done
}

// ---------------------------------------------------------------------
// Listener thread
// ---------------------------------------------------------------------

/// Accept connections one at a time until the stop flag is
/// raised. Sequential by design: it keeps ordering trivial and
/// the queue is the only hand-off to the UI thread.
fn listener_loop(listener: TcpListener, tx: Sender<LiveRequest>, stop: Arc<AtomicBool>) {
    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => handle_connection(stream, &tx, &stop),
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
fn handle_connection(mut stream: TcpStream, tx: &Sender<LiveRequest>, stop: &Arc<AtomicBool>) {
    let _ = stream.set_read_timeout(Some(IO_TIMEOUT));
    let _ = stream.set_write_timeout(Some(IO_TIMEOUT));
    // The accept loop is non-blocking; accepted sockets are
    // blocking on some platforms and inherit the flag on
    // others — make the mode explicit before reading.
    let _ = stream.set_nonblocking(false);

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
        match stream.read(&mut chunk) {
            Ok(0) => return, // client went away mid-head
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(_) => return,
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_len]).into_owned();

    // --- Only POST carries JSON-RPC. ---
    if request_method(&head) != "POST" {
        write_response(&mut stream, 405, "Method Not Allowed", "text/plain", b"POST only\n");
        return;
    }

    // --- Read exactly Content-Length bytes of body. ---
    let content_length = parse_content_length(&head).unwrap_or(0);
    if content_length > MAX_BODY {
        write_response(&mut stream, 413, "Payload Too Large", "text/plain", b"body too large\n");
        return;
    }
    let mut body: Vec<u8> = buf[head_len..].to_vec();
    while body.len() < content_length {
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

    // --- Forward to the UI thread and block on the reply.
    //     This wait IS the synchronisation: the reply lands
    //     when poll() next drains the queue (see module docs).
    let (reply_tx, reply_rx) = std::sync::mpsc::sync_channel::<Option<Value>>(1);
    if tx.send(LiveRequest { msg, reply: reply_tx }).is_err() {
        // The queue's receiver is gone: the bridge is being
        // torn down under this connection.
        write_response(&mut stream, 500, "Internal Server Error", "text/plain", b"editor not draining\n");
        return;
    }
    loop {
        match reply_rx.recv_timeout(Duration::from_millis(25)) {
            Ok(Some(response)) => {
                write_json(&mut stream, 200, &response);
                return;
            }
            Ok(None) => {
                // Notification: JSON-RPC says never answer —
                // the HTTP layer still needs a status, so 202
                // with an empty body.
                write_response(&mut stream, 202, "Accepted", "application/json", b"");
                return;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if stop.load(Ordering::SeqCst) {
                    return; // bridge switched off mid-request
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

/// Write one complete HTTP/1.1 response and let the socket
/// close with the stream. Write failures are ignored — the
/// client is already gone and there is no one to report to.
fn write_response(stream: &mut TcpStream, status: u16, reason: &str, content_type: &str, body: &[u8]) {
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
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
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body.as_bytes());
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
fn parse_content_length(head: &str) -> Option<usize> {
    for line in head.lines() {
        let Some((name, value)) = line.split_once(':') else {
            continue; // the request line has no colon to split
        };
        if name.trim().eq_ignore_ascii_case("content-length") {
            return value.trim().parse::<usize>().ok();
        }
    }
    None
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

    // -----------------------------------------------------------------
    // End-to-end proof — the suite's ONLY socket-binding test.
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
    /// minimal MCP client would) and parse the response
    /// body. Every operation carries a timeout, so a broken
    /// bridge fails this test instead of hanging CI.
    fn post_json(msg: &Value) -> Result<Value, String> {
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
        if !raw.starts_with("HTTP/1.1 200") {
            return Err(format!(
                "unexpected response status: {}",
                raw.lines().next().unwrap_or("")
            ));
        }
        let (_, resp_body) = raw
            .split_once("\r\n\r\n")
            .ok_or("response had no header/body split")?;
        serde_json::from_str(resp_body).map_err(|e| format!("response was not JSON: {e}"))
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
        Ok(())
    }

    #[test]
    fn live_bridge_drives_an_external_core_over_http() {
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
}
