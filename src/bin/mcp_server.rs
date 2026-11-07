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

use std::io::{BufRead, Write};

use stykanimate_rs::mcp::McpServer;

fn main() {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut lines = stdin.lock().lines();
    let mut out = stdout.lock();
    let mut server = McpServer::new();

    while let Some(line) = lines.next() {
        let line = match line {
            Ok(line) => line,
            Err(err) => {
                eprintln!("mcp_server: stdin read error: {err}");
                break;
            }
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<serde_json::Value>(line) {
            Ok(msg) => server.handle(&msg),
            Err(err) => Some(serde_json::json!({
                "jsonrpc": "2.0",
                "id": serde_json::Value::Null,
                "error": { "code": -32700, "message": format!("Parse error: {err}") },
            })),
        };
        if let Some(response) = response {
            if writeln!(out, "{response}").is_err() || out.flush().is_err() {
                // Client went away; nothing more to serve.
                break;
            }
        }
    }
}
