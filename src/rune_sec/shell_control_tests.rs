//! Wave 14 (B10, reconciled at integration) — shell
//! control contract tests against the LANDED design.
//!
//! Declared from `main.rs` beside the `snap_tests`
//! declaration:
//!
//! ```rust
//! #[cfg(test)]
//! #[path = "rune_sec/shell_control_tests.rs"]
//! mod shell_control_tests;
//! ```
//!
//! The first draft of this suite was written against the
//! design draft's assumed plumbing API (a command enum
//! with ticket/result slots, consolidated tool names and
//! a Stop action). The landed design differs, on the
//! evidence of the handler modules that shipped: granular
//! tool names dispatched by name through
//! `crate::shell_control`'s queue, transport as ordinary
//! core tools, and no Stop (the GUI has none). This suite
//! pins the landed contracts:
//!
//! * every shell name in `shell_control::SHELL_TOOL_NAMES`
//!   is registered in the shared registry (so `tools/list`
//!   matches on both transports), and the registry's
//!   shell-prefixed names are exactly those plus the four
//!   core transport tools;
//! * on the stdio router, a shell call is an `isError`
//!   tool result with the one canonical live-editor
//!   refusal — never a `-32602` (that code is reserved
//!   for genuinely unknown names, and an unregistered
//!   `shell_*` name still earns it);
//! * the live drain's peek takes over registered shell
//!   calls only — never notifications, never core tools,
//!   never unknown shell names.
//!
//! The end-to-end proof (a shell call over real HTTP
//! driving the running editor) lives in the headless
//! workflow's live-MCP probe, which runs against the
//! built binary; the queue's FIFO/bound/respond shapes
//! are pinned in `shell_control.rs`'s own tests.

use serde_json::{json, Value};

use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::mcp::McpServer;

use crate::live_mcp::LiveRequest;
use crate::shell_control::{is_shell_tool, peek, SHELL_TOOL_NAMES};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

fn call(server: &mut McpServer, core: &mut EditorCore, name: &str, args: Value) -> Value {
    let msg = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": { "name": name, "arguments": args },
    });
    server
        .handle_on(core, &msg)
        .expect("a tools/call with an id always answers")
}

#[test]
fn shell_names_and_registry_agree_exactly() {
    let tools = stykanimate_rs::mcp::all_tools();
    let registered: Vec<&str> = tools.iter().map(|t| t.name).collect();
    // Every live-only shell name is registered (with its
    // stub), so stdio clients see the same list.
    for name in SHELL_TOOL_NAMES {
        assert!(
            registered.contains(name),
            "shell tool {name} must be registered (tools/list parity)"
        );
        assert!(is_shell_tool(name));
    }
    // And the registry's shell_* family is exactly the
    // live-only set plus the four transport tools, which
    // are core tools with real handlers (transport state
    // is EditorCore state).
    let shell_registered: Vec<&str> = registered
        .iter()
        .copied()
        .filter(|n| n.starts_with("shell_"))
        .collect();
    let mut expected: Vec<&str> = SHELL_TOOL_NAMES.to_vec();
    expected.extend([
        "shell_play",
        "shell_pause",
        "shell_set_playhead",
        "shell_get_transport",
    ]);
    assert_eq!(shell_registered.len(), expected.len());
    for name in expected {
        assert!(shell_registered.contains(&name), "registry missing {name}");
    }
}

#[test]
fn stdio_refuses_shell_tools_with_the_canonical_wording() {
    let mut server = McpServer::new();
    let mut core = EditorCore::new();
    for name in ["shell_set_workspace", "shell_get_state", "shell_set_sidebar"] {
        let r = call(&mut server, &mut core, name, json!({}));
        assert!(
            r.get("error").is_none(),
            "a registered shell tool must not earn a protocol error: {r}"
        );
        assert_eq!(r["result"]["isError"], json!(true), "{name} on stdio refuses");
        let text = r["result"]["content"][0]["text"].as_str().unwrap();
        assert!(
            text.contains("needs the live editor")
                && text.contains("http://127.0.0.1:32117/mcp"),
            "{name} refusal must be the canonical live-editor wording, got: {text}"
        );
        assert!(text.contains(name), "the refusal names the tool: {text}");
    }
}

#[test]
fn unknown_shell_name_still_earns_minus_32602() {
    let mut server = McpServer::new();
    let mut core = EditorCore::new();
    let r = call(&mut server, &mut core, "shell_frobnicate", json!({}));
    assert_eq!(r["error"]["code"], json!(-32602), "{r}");
    assert!(
        r["error"]["message"]
            .as_str()
            .unwrap()
            .contains("unknown tool: shell_frobnicate")
    );
}

#[test]
fn transport_tools_are_real_core_tools_on_stdio() {
    // The transport family is EditorCore state, so on
    // stdio these EXECUTE against the server's own core —
    // the honest split the shell chapter documents.
    let mut server = McpServer::new();
    let mut core = EditorCore::new();
    let r = call(&mut server, &mut core, "shell_get_transport", json!({}));
    assert_eq!(r["result"]["isError"], json!(false), "{r}");
    let r = call(&mut server, &mut core, "shell_play", json!({}));
    assert_eq!(r["result"]["isError"], json!(false), "{r}");
    assert!(core.playing, "shell_play on stdio plays the server's own core");
}

fn live_req(msg: Value) -> LiveRequest {
    let (reply, _rx) = std::sync::mpsc::sync_channel(1);
    LiveRequest {
        msg,
        reply,
        cancelled: Arc::new(AtomicBool::new(false)),
    }
}

#[test]
fn peek_takes_only_registered_shell_calls() {
    let call = |name: &str| {
        json!({
            "jsonrpc": "2.0", "id": 9, "method": "tools/call",
            "params": { "name": name, "arguments": {} },
        })
    };
    assert!(peek(&live_req(call("shell_set_section"))).is_some());
    assert!(peek(&live_req(call("shell_get_layout"))).is_some());
    assert!(peek(&live_req(call("get_state"))).is_none(), "core tools route normally");
    assert!(
        peek(&live_req(call("shell_frobnicate"))).is_none(),
        "unknown shell names route normally (to their -32602)"
    );
    // A notification naming a shell tool is never taken:
    // notifications execute nothing.
    let note = json!({
        "jsonrpc": "2.0", "method": "tools/call",
        "params": { "name": "shell_set_section", "arguments": {} },
    });
    assert!(peek(&live_req(note)).is_none());
}
