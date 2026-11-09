//! StykAnimate-RS as an MCP (Model Context Protocol) server.
//!
//! Transport: JSON-RPC 2.0 over stdio, one JSON object per line.
//! Requests are read from stdin until EOF; every response is
//! written as a single line to stdout and flushed immediately.
//! Nothing but protocol messages may go to stdout — diagnostics
//! (this binary emits none by default) belong on stderr.
//!
//! All protocol logic lives in `stykanimate_rs::mcp::McpServer`;
//! this shell only owns the line framing and the top-level parse
//! error (`-32700`) that cannot be routed without a parsed message.
//!
//! Wave 11 (A5) robustness pass — the framing survives anything
//! a client can put on the wire:
//!
//! - A malformed line (bad JSON, invalid UTF-8) is answered
//!   with `-32700` and the server keeps serving; previously the
//!   `BufRead::lines()` iterator surfaced invalid UTF-8 as a
//!   read error and the loop BROKE — one bad byte killed the
//!   server. Lines are now read as raw bytes and decoded
//!   lossily, so bad UTF-8 degrades to a parse error.
//! - A line longer than [`MAX_LINE_BYTES`] is consumed through
//!   its newline (the stream stays in sync), answered with
//!   `-32600`, and never buffered whole — an unbounded line
//!   could previously grow memory until the process died.
//! - A panic inside a tool handler is caught and answered
//!   with `-32603` naming the request id, instead of unwinding
//!   out of `main` and killing the server with no response at
//!   all. (The panic may leave the session's edit half-applied;
//!   the error text says the dispatch panicked so the client
//!   can reload state rather than trust it.) Note this guard
//!   covers the stdio transport only — the shared dispatcher
//!   itself has no `catch_unwind`, so the live-control path
//!   needs the same guard at its own layer.
//! - EOF mid-line still processes the final unterminated line
//!   before exiting (the previous `lines()` semantics), and a
//!   client that connects and sends nothing gets silence until
//!   EOF: the server never speaks first.

use std::io::{BufRead, Write};
use std::panic::{catch_unwind, AssertUnwindSafe};

use stykanimate_rs::mcp::McpServer;

/// Largest request line the server will buffer, in bytes.
/// Requests carry tool arguments only — project files move by
/// path, and the biggest responses (base64 frame renders) flow
/// the other way — so 32 MiB is far above any legitimate
/// request while still bounding memory.
const MAX_LINE_BYTES: usize = 32 * 1024 * 1024;

/// One framed read from the input stream.
enum LineRead {
    /// A complete line (newline consumed). At EOF this is the
    /// final unterminated line, if any bytes were pending.
    Line(Vec<u8>),
    /// A line exceeded the byte cap; it was consumed through
    /// its newline (or EOF) without being buffered.
    Oversized,
    /// Clean end of stream with nothing pending.
    Eof,
}

/// Read one line from `input`, buffering at most `cap` bytes.
/// Bytes beyond the cap are consumed and dropped (the line is
/// reported [`LineRead::Oversized`]) so the stream stays framed
/// for the next line.
fn read_line_capped<R: BufRead>(input: &mut R, cap: usize) -> std::io::Result<LineRead> {
    let mut buf: Vec<u8> = Vec::new();
    let mut oversized = false;
    loop {
        let mut take = 0usize;
        let mut newline = false;
        let mut eof = false;
        {
            let chunk = input.fill_buf()?;
            if chunk.is_empty() {
                eof = true;
            } else {
                match chunk.iter().position(|b| *b == b'\n') {
                    Some(pos) => {
                        newline = true;
                        take = pos;
                    }
                    None => take = chunk.len(),
                }
                if !oversized {
                    if buf.len() + take > cap {
                        oversized = true;
                        buf = Vec::new(); // drop the partial line
                    } else {
                        buf.extend_from_slice(&chunk[..take]);
                    }
                }
            }
        }
        if eof {
            return Ok(if oversized {
                LineRead::Oversized
            } else if buf.is_empty() {
                LineRead::Eof
            } else {
                LineRead::Line(buf)
            });
        }
        input.consume(take + usize::from(newline));
        if newline {
            return Ok(if oversized {
                LineRead::Oversized
            } else {
                LineRead::Line(buf)
            });
        }
    }
}

/// The `-32603` response for a tool dispatch that panicked,
/// carrying the request's id and the panic message when the
/// payload is a string (the usual `panic!("…")` shapes).
fn panic_response(id: serde_json::Value, payload: &(dyn std::any::Any + Send)) -> serde_json::Value {
    let detail = payload
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown panic".to_string());
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": -32603,
            "message": format!("Internal error: tool dispatch panicked: {detail}"),
        },
    })
}

/// Write one response line and flush. Returns false when the
/// client went away — nothing more can be served.
fn write_response<W: Write>(out: &mut W, response: &serde_json::Value) -> bool {
    writeln!(out, "{response}").is_ok() && out.flush().is_ok()
}

/// Serve requests from `input` until EOF, writing responses to
/// `out`. Split from `main` so the framing is testable on
/// in-memory streams.
fn run<R: BufRead, W: Write>(server: &mut McpServer, input: &mut R, out: &mut W, cap: usize) {
    loop {
        let bytes = match read_line_capped(input, cap) {
            Ok(LineRead::Line(bytes)) => bytes,
            Ok(LineRead::Oversized) => {
                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": serde_json::Value::Null,
                    "error": {
                        "code": -32600,
                        "message": format!("Invalid Request: line exceeds the {cap}-byte limit"),
                    },
                });
                if !write_response(out, &response) {
                    break;
                }
                continue;
            }
            Ok(LineRead::Eof) => break,
            Err(err) => {
                eprintln!("mcp_server: stdin read error: {err}");
                break;
            }
        };
        // Lossy decode: invalid UTF-8 becomes U+FFFD and fails
        // JSON parsing below (-32700), instead of ending the
        // session the way the old `lines()` error path did.
        let text = String::from_utf8_lossy(&bytes);
        let line = text.trim();
        if line.is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<serde_json::Value>(line) {
            Ok(msg) => {
                let id = msg.get("id").cloned().unwrap_or(serde_json::Value::Null);
                match catch_unwind(AssertUnwindSafe(|| server.handle(&msg))) {
                    Ok(response) => response,
                    Err(payload) => Some(panic_response(id, &*payload)),
                }
            }
            Err(err) => Some(serde_json::json!({
                "jsonrpc": "2.0",
                "id": serde_json::Value::Null,
                "error": { "code": -32700, "message": format!("Parse error: {err}") },
            })),
        };
        if let Some(response) = response {
            if !write_response(out, &response) {
                // Client went away; nothing more to serve.
                break;
            }
        }
    }
}

fn main() {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut input = stdin.lock();
    let mut out = stdout.lock();
    let mut server = McpServer::new();
    run(&mut server, &mut input, &mut out, MAX_LINE_BYTES);
}

// ---------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// Drive `run` over an in-memory stream and return the
    /// response lines it produced.
    fn serve(input: &[u8], cap: usize) -> Vec<String> {
        let mut server = McpServer::new();
        let mut reader = Cursor::new(input.to_vec());
        let mut out: Vec<u8> = Vec::new();
        run(&mut server, &mut reader, &mut out, cap);
        String::from_utf8(out)
            .expect("responses are valid UTF-8")
            .lines()
            .map(str::to_string)
            .collect()
    }

    fn ping(id: u64) -> String {
        format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"method\":\"ping\"}}")
    }

    #[test]
    fn malformed_lines_are_answered_and_the_server_survives() {
        let input = format!("this is not json\n\n{}\n", ping(7));
        let responses = serve(input.as_bytes(), MAX_LINE_BYTES);
        assert_eq!(responses.len(), 2, "empty line skipped, two answers: {responses:?}");
        let parse_error: serde_json::Value = serde_json::from_str(&responses[0]).unwrap();
        assert_eq!(parse_error["error"]["code"], -32700);
        let pong: serde_json::Value = serde_json::from_str(&responses[1]).unwrap();
        assert_eq!(pong["id"], 7, "the request after the garbage still gets served");
    }

    #[test]
    fn invalid_utf8_is_a_parse_error_not_the_end_of_the_session() {
        let mut input = vec![0xFF, 0xFE, b'\n'];
        input.extend_from_slice(ping(3).as_bytes());
        input.push(b'\n');
        let responses = serve(&input, MAX_LINE_BYTES);
        assert_eq!(responses.len(), 2);
        let first: serde_json::Value = serde_json::from_str(&responses[0]).unwrap();
        assert_eq!(first["error"]["code"], -32700);
        let second: serde_json::Value = serde_json::from_str(&responses[1]).unwrap();
        assert_eq!(second["id"], 3);
    }

    #[test]
    fn eof_mid_line_still_processes_the_final_line() {
        // No trailing newline: the partial line at EOF is a
        // request, not a truncation to drop.
        let responses = serve(ping(9).as_bytes(), MAX_LINE_BYTES);
        assert_eq!(responses.len(), 1);
        let pong: serde_json::Value = serde_json::from_str(&responses[0]).unwrap();
        assert_eq!(pong["id"], 9);
    }

    #[test]
    fn oversized_line_is_refused_and_the_stream_resyncs() {
        let mut input = vec![b'x'; 64];
        input.push(b'\n');
        input.extend_from_slice(ping(5).as_bytes());
        input.push(b'\n');
        let responses = serve(&input, 48);
        assert_eq!(responses.len(), 2, "{responses:?}");
        let refusal: serde_json::Value = serde_json::from_str(&responses[0]).unwrap();
        assert_eq!(refusal["error"]["code"], -32600);
        let pong: serde_json::Value = serde_json::from_str(&responses[1]).unwrap();
        assert_eq!(pong["id"], 5, "the request after the oversized line still parses");
    }

    #[test]
    fn a_tools_call_runs_end_to_end_over_stdio_framing() {
        let call = "{\"jsonrpc\":\"2.0\",\"id\":11,\"method\":\"tools/call\",\
                    \"params\":{\"name\":\"project_summary\",\"arguments\":{}}}";
        let responses = serve(format!("{call}\n").as_bytes(), MAX_LINE_BYTES);
        assert_eq!(responses.len(), 1);
        let resp: serde_json::Value = serde_json::from_str(&responses[0]).unwrap();
        assert_eq!(resp["id"], 11);
        assert_eq!(resp["result"]["isError"], false);
    }

    #[test]
    fn panic_response_names_the_id_and_the_panic() {
        let payload = catch_unwind(|| panic!("boom in a tool")).unwrap_err();
        let resp = panic_response(serde_json::json!(42), &*payload);
        assert_eq!(resp["id"], 42);
        assert_eq!(resp["error"]["code"], -32603);
        assert!(
            resp["error"]["message"].as_str().unwrap().contains("boom in a tool"),
            "{resp}"
        );
    }

    #[test]
    fn read_line_capped_reports_clean_eof() {
        let mut empty = Cursor::new(Vec::new());
        assert!(matches!(
            read_line_capped(&mut empty, 8).unwrap(),
            LineRead::Eof
        ));
        let mut one = Cursor::new(b"ab\ncd".to_vec());
        match read_line_capped(&mut one, 8).unwrap() {
            LineRead::Line(bytes) => assert_eq!(bytes, b"ab"),
            _ => panic!("expected the first line"),
        }
        match read_line_capped(&mut one, 8).unwrap() {
            LineRead::Line(bytes) => assert_eq!(bytes, b"cd", "unterminated tail at EOF"),
            _ => panic!("expected the tail line"),
        }
        assert!(matches!(
            read_line_capped(&mut one, 8).unwrap(),
            LineRead::Eof
        ));
    }
}
