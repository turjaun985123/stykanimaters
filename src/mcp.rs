//! MCP (Model Context Protocol) server core for StykAnimate-RS.
//!
//! This module is the transport-agnostic half of the MCP server: it
//! owns the tool registry and the JSON-RPC 2.0 request router. The
//! binary in `src/bin/mcp_server.rs` is a thin stdio shell around it
//! (one JSON object per line, responses flushed per line).
//!
//! [`McpServer::handle`] is pure with respect to IO — it takes one
//! parsed JSON-RPC message and returns the response message, or
//! `None` for notifications, which JSON-RPC says must not be
//! answered. All editing state lives in an
//! [`crate::editor::EditorCore`]: the server's own wrapped core
//! when routing through `handle`, or a host-supplied core when
//! routing through [`McpServer::handle_on`] — the live-control
//! path, where the GUI hands over its session's core so every
//! tool edits the project the user is looking at.
//!
//! Supported methods: `initialize`, `ping`, `tools/list`,
//! `tools/call`. Unknown methods get a `-32601` error; unroutable
//! requests get `-32600`; bad `tools/call` params get `-32602`.
//! Tool-level failures are reported inside the result as
//! `isError: true`, per the MCP spec, not as protocol errors.
//!
//! Tools come from three places: the built-ins defined here
//! (`server_info`, `get_state`, `set_frame`),
//! [`crate::mcp_tools`] (editing operations) and [`crate::mcp_io`]
//! (file IO and export). [`all_tools`] is the single registry every
//! consumer goes through.

use serde_json::{json, Value};

/// One MCP tool: static metadata plus a handler that runs against
/// the shared editor core.
///
/// `schema` is the JSON Schema of the `arguments` object the tool
/// accepts (it is surfaced verbatim as `inputSchema` in
/// `tools/list`). Handlers return the tool's result value, which
/// the router serialises into the text content block of the
/// `tools/call` response; an `Err` string becomes an `isError`
/// result carrying that message.
pub struct Tool {
    pub name: &'static str,
    pub description: &'static str,
    /// JSON Schema for the arguments object.
    pub schema: serde_json::Value,
    pub handler:
        fn(&mut crate::editor::EditorCore, &serde_json::Value) -> Result<serde_json::Value, String>,
}

/// The full tool registry: this module's built-ins first, then the
/// editing tools from `mcp_tools` and the IO/export tools from
/// `mcp_io`.
pub fn all_tools() -> Vec<Tool> {
    let mut tools = vec![tool_server_info(), tool_get_state(), tool_set_frame()];
    tools.extend(crate::mcp_tools::tools());
    tools.extend(crate::mcp_io::tools());
    tools
}

/// The MCP server: one editor core plus the JSON-RPC router.
/// The wrapped core is the edit target for
/// [`McpServer::handle`]; [`McpServer::handle_on`] routes against
/// a caller-supplied core instead and leaves this one untouched.
pub struct McpServer {
    pub core: crate::editor::EditorCore,
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}

impl McpServer {
    pub fn new() -> Self {
        McpServer {
            core: crate::editor::EditorCore::new(),
        }
    }

    /// Route one parsed JSON-RPC message against this server's
    /// own core. Returns the response message, or `None` when the
    /// message is a notification (no `id` member) — notifications
    /// are never answered, known or not.
    ///
    /// A thin delegate over the shared router with
    /// `&mut self.core` as the edit target — the stdio binary's
    /// path, behaviour unchanged.
    pub fn handle(&mut self, msg: &serde_json::Value) -> Option<serde_json::Value> {
        Self::dispatch(&mut self.core, msg)
    }

    /// Route one parsed JSON-RPC message against an EXTERNAL
    /// editor core — the live-control path: a host that owns an
    /// [`EditorCore`](crate::editor::EditorCore) of its own (the
    /// GUI's live session) hands it over for the duration of the
    /// call, and every `tools/call` in the message runs against
    /// that core. `self` is only the dispatch host here: this
    /// server's own `core` is neither read nor mutated, so a host
    /// may keep one `McpServer` purely as a router.
    ///
    /// The routing itself — the notification rule, the method
    /// set, the error shapes — is exactly [`McpServer::handle`]'s.
    pub fn handle_on(
        &mut self,
        core: &mut crate::editor::EditorCore,
        msg: &serde_json::Value,
    ) -> Option<serde_json::Value> {
        Self::dispatch(core, msg)
    }

    /// The JSON-RPC router shared by [`McpServer::handle`] and
    /// [`McpServer::handle_on`]: everything below is
    /// parameterised over the `core` the caller chose as the
    /// edit target, never over `self`.
    fn dispatch(
        core: &mut crate::editor::EditorCore,
        msg: &serde_json::Value,
    ) -> Option<serde_json::Value> {
        if !msg.is_object() {
            return Some(error_response(
                Value::Null,
                -32600,
                "Invalid Request: expected a JSON-RPC object",
            ));
        }
        // Notifications have no id: never reply, whatever the method.
        let Some(id) = msg.get("id").cloned() else {
            return None;
        };
        let Some(method) = msg.get("method").and_then(Value::as_str) else {
            return Some(error_response(
                id,
                -32600,
                "Invalid Request: missing method",
            ));
        };
        match method {
            "initialize" => Some(success_response(id, Self::initialize_result())),
            "ping" => Some(success_response(id, json!({}))),
            "tools/list" => Some(success_response(id, Self::tools_list_result())),
            "tools/call" => Some(Self::tools_call_result(core, id, msg.get("params"))),
            other => Some(error_response(
                id,
                -32601,
                &format!("Method not found: {other}"),
            )),
        }
    }

    fn initialize_result() -> Value {
        json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": {
                "name": "stykanimate-rs",
                "version": env!("CARGO_PKG_VERSION"),
            },
        })
    }

    fn tools_list_result() -> Value {
        let tools: Vec<Value> = all_tools()
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "inputSchema": t.schema,
                })
            })
            .collect();
        json!({ "tools": tools })
    }

    fn tools_call_result(
        core: &mut crate::editor::EditorCore,
        id: Value,
        params: Option<&Value>,
    ) -> Value {
        let Some(params) = params else {
            return error_response(id, -32602, "Invalid params: missing params object");
        };
        let Some(name) = params.get("name").and_then(Value::as_str) else {
            return error_response(id, -32602, "Invalid params: missing tool name");
        };
        let empty = json!({});
        let args = params.get("arguments").unwrap_or(&empty);
        let tools = all_tools();
        let Some(tool) = tools.iter().find(|t| t.name == name) else {
            return tool_result(id, Err(format!("unknown tool: {name}")));
        };
        tool_result(id, (tool.handler)(core, args))
    }
}

fn success_response(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    })
}

/// Wrap a tool handler outcome as an MCP `tools/call` result: the
/// value JSON-serialised into a single text content block, with
/// `isError` distinguishing failure text from result text.
fn tool_result(id: Value, outcome: Result<Value, String>) -> Value {
    let (text, is_error) = match outcome {
        Ok(value) => (value.to_string(), false),
        Err(message) => (message, true),
    };
    success_response(
        id,
        json!({
            "content": [{ "type": "text", "text": text }],
            "isError": is_error,
        }),
    )
}

// ---------------------------------------------------------------------
// Built-in tools
// ---------------------------------------------------------------------

fn empty_args_schema() -> Value {
    json!({ "type": "object", "properties": {}, "additionalProperties": false })
}

fn tool_server_info() -> Tool {
    Tool {
        name: "server_info",
        description: "Identify this MCP server: name, version and how many tools are registered.",
        schema: empty_args_schema(),
        handler: |_core, _args| {
            Ok(json!({
                "name": "stykanimate-rs",
                "version": env!("CARGO_PKG_VERSION"),
                "tools": all_tools().len(),
            }))
        },
    }
}

fn tool_get_state() -> Tool {
    Tool {
        name: "get_state",
        description: "Snapshot of the current project: name, figures, frame count, current frame, fps, stage size and weather state.",
        schema: empty_args_schema(),
        handler: |core, _args| {
            let project = &core.project;
            let figures: Vec<Value> = project
                .figures
                .iter()
                .map(|f| {
                    json!({
                        "name": f.name,
                        "hidden": f.hidden,
                        "locked": f.locked,
                        "opacity": f.opacity,
                    })
                })
                .collect();
            let weather_kind = project.weather.as_ref().map(|w| match w.kind {
                crate::vfx::WeatherKind::Rain => "rain",
                crate::vfx::WeatherKind::Snow => "snow",
                crate::vfx::WeatherKind::Fireflies => "fireflies",
                crate::vfx::WeatherKind::Fog => "fog",
                crate::vfx::WeatherKind::Storm => "storm",
            });
            Ok(json!({
                "project": project.name,
                "figures": figures,
                "frame_count": project.frames.len(),
                "current_frame": core.current,
                "fps": project.fps,
                "stage": project.stage,
                "weather": project.weather.is_some(),
                "weather_kind": weather_kind,
            }))
        },
    }
}

fn tool_set_frame() -> Tool {
    Tool {
        name: "set_frame",
        description: "Move the playhead to a 0-based frame index of the current project.",
        schema: json!({
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
        handler: |core, args| {
            let raw = args
                .get("frame")
                .ok_or_else(|| "missing required argument: frame".to_string())?;
            let frame = raw
                .as_u64()
                .or_else(|| raw.as_i64().filter(|v| *v >= 0).map(|v| v as u64))
                .or_else(|| {
                    raw.as_f64()
                        .filter(|v| *v >= 0.0 && v.fract() == 0.0)
                        .map(|v| v as u64)
                })
                .ok_or_else(|| {
                    format!("invalid 'frame' argument: expected a non-negative integer, got {raw}")
                })?;
            let count = core.project.frames.len();
            if frame as usize >= count {
                return Err(format!(
                    "frame {frame} is out of range: the project has {count} frame(s)"
                ));
            }
            core.goto(frame as usize);
            Ok(json!({
                "current_frame": core.current,
                "frame_count": count,
            }))
        },
    }
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn request(id: u64, method: &str, params: Value) -> Value {
        json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
    }

    /// Extract the parsed JSON document a `tools/call` result
    /// carries in its single text content block.
    fn call_payload(server: &mut McpServer, name: &str, args: Value) -> (Value, bool) {
        let resp = server
            .handle(&request(1, "tools/call", json!({ "name": name, "arguments": args })))
            .expect("tools/call is a request and must be answered");
        let result = &resp["result"];
        let text = result["content"][0]["text"]
            .as_str()
            .expect("content block carries text");
        let is_error = result["isError"].as_bool().unwrap_or(false);
        (
            serde_json::from_str(text).unwrap_or(Value::String(text.to_string())),
            is_error,
        )
    }

    #[test]
    fn initialize_handshake_shape() {
        let mut server = McpServer::new();
        let resp = server
            .handle(&request(1, "initialize", json!({})))
            .expect("initialize must be answered");
        assert_eq!(resp["jsonrpc"], "2.0");
        assert_eq!(resp["id"], 1);
        let result = &resp["result"];
        assert_eq!(result["protocolVersion"], "2024-11-05");
        assert_eq!(result["serverInfo"]["name"], "stykanimate-rs");
        assert!(result["serverInfo"]["version"].is_string());
        assert!(result["capabilities"]["tools"].is_object());
    }

    #[test]
    fn notifications_get_no_reply() {
        let mut server = McpServer::new();
        let msg = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
        assert!(server.handle(&msg).is_none());
        let unknown = json!({ "jsonrpc": "2.0", "method": "notifications/whatever" });
        assert!(server.handle(&unknown).is_none());
    }

    #[test]
    fn ping_returns_empty_result() {
        let mut server = McpServer::new();
        let resp = server
            .handle(&request(7, "ping", json!({})))
            .expect("ping must be answered");
        assert_eq!(resp["id"], 7);
        assert_eq!(resp["result"], json!({}));
    }

    #[test]
    fn tools_list_contains_builtins_and_registered_tools() {
        let mut server = McpServer::new();
        let resp = server
            .handle(&request(2, "tools/list", json!({})))
            .expect("tools/list must be answered");
        let tools = resp["result"]["tools"]
            .as_array()
            .expect("tools is an array");
        assert_eq!(tools.len(), all_tools().len());
        for expected in ["server_info", "get_state", "set_frame"] {
            assert!(
                tools.iter().any(|t| t["name"] == expected),
                "tools/list is missing built-in {expected}"
            );
        }
        for tool in tools {
            assert!(tool["name"].is_string());
            assert!(tool["description"].is_string());
            assert!(tool["inputSchema"].is_object());
        }
        // server_info agrees with the registry size.
        let (info, is_error) = call_payload(&mut server, "server_info", json!({}));
        assert!(!is_error);
        assert_eq!(info["name"], "stykanimate-rs");
        assert_eq!(info["tools"].as_u64(), Some(tools.len() as u64));
    }

    #[test]
    fn get_state_reports_the_demo_project() {
        let mut server = McpServer::new();
        let (state, is_error) = call_payload(&mut server, "get_state", json!({}));
        assert!(!is_error);
        assert_eq!(state["project"], server.core.project.name.as_str());
        assert_eq!(
            state["frame_count"].as_u64(),
            Some(server.core.project.frames.len() as u64)
        );
        assert_eq!(state["current_frame"], 0);
        assert_eq!(
            state["fps"].as_f64(),
            Some(server.core.project.fps as f64)
        );
        assert_eq!(state["stage"], json!(server.core.project.stage));
        assert_eq!(state["weather"], false);
        let figures = state["figures"].as_array().expect("figures array");
        assert_eq!(figures.len(), server.core.project.figures.len());
        assert!(!figures.is_empty(), "demo project has a figure");
        assert_eq!(figures[0]["name"], server.core.project.figures[0].name.as_str());
    }

    #[test]
    fn set_frame_moves_the_playhead_and_validates() {
        let mut server = McpServer::new();
        let count = server.core.project.frames.len();
        assert!(count >= 2, "demo project needs multiple frames");
        let (out, is_error) = call_payload(&mut server, "set_frame", json!({ "frame": 1 }));
        assert!(!is_error);
        assert_eq!(out["current_frame"], 1);
        assert_eq!(server.core.current, 1);

        let (_out, is_error) =
            call_payload(&mut server, "set_frame", json!({ "frame": count }));
        assert!(is_error, "one past the last frame is out of range");
        assert_eq!(server.core.current, 1, "failed set_frame must not move");

        let (_out, is_error) = call_payload(&mut server, "set_frame", json!({}));
        assert!(is_error, "missing frame argument is an error");

        let (_out, is_error) = call_payload(&mut server, "set_frame", json!({ "frame": -1 }));
        assert!(is_error, "negative frame is an error");
    }

    #[test]
    fn unknown_tool_is_an_error_result_not_a_protocol_error() {
        let mut server = McpServer::new();
        let resp = server
            .handle(&request(
                3,
                "tools/call",
                json!({ "name": "does_not_exist", "arguments": {} }),
            ))
            .expect("tools/call must be answered");
        assert!(resp.get("error").is_none());
        assert_eq!(resp["result"]["isError"], true);
        let text = resp["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("does_not_exist"));
    }

    #[test]
    fn unknown_method_is_method_not_found() {
        let mut server = McpServer::new();
        let resp = server
            .handle(&request(9, "resources/list", json!({})))
            .expect("unknown methods with an id must be answered");
        assert_eq!(resp["error"]["code"], -32601);
        assert_eq!(resp["id"], 9);
    }

    #[test]
    fn non_object_message_is_invalid_request() {
        let mut server = McpServer::new();
        let resp = server.handle(&json!([1, 2, 3])).expect("answered");
        assert_eq!(resp["error"]["code"], -32600);
    }

    /// Same as [`call_payload`], but routed through
    /// [`McpServer::handle_on`] against a caller-owned core.
    fn call_payload_on(
        server: &mut McpServer,
        core: &mut crate::editor::EditorCore,
        name: &str,
        args: Value,
    ) -> (Value, bool) {
        let resp = server
            .handle_on(
                core,
                &request(1, "tools/call", json!({ "name": name, "arguments": args })),
            )
            .expect("tools/call is a request and must be answered");
        let result = &resp["result"];
        let text = result["content"][0]["text"]
            .as_str()
            .expect("content block carries text");
        let is_error = result["isError"].as_bool().unwrap_or(false);
        (
            serde_json::from_str(text).unwrap_or(Value::String(text.to_string())),
            is_error,
        )
    }

    #[test]
    fn handle_on_set_frame_moves_the_passed_core_only() {
        let mut server = McpServer::new();
        let mut external = crate::editor::EditorCore::new();
        assert!(
            external.project.frames.len() >= 2,
            "demo project needs multiple frames"
        );
        let (out, is_error) =
            call_payload_on(&mut server, &mut external, "set_frame", json!({ "frame": 1 }));
        assert!(!is_error);
        assert_eq!(out["current_frame"], 1);
        assert_eq!(external.current, 1, "the passed core moved");
        assert_eq!(server.core.current, 0, "the server's own core must not move");
    }

    #[test]
    fn handle_on_content_tool_mutates_the_passed_core_only() {
        let mut server = McpServer::new();
        let mut external = crate::editor::EditorCore::new();
        let own_figures = server.core.project.figures.len();
        let ext_figures = external.project.figures.len();
        let (_out, is_error) = call_payload_on(
            &mut server,
            &mut external,
            "add_figure",
            json!({ "kind": "mini" }),
        );
        assert!(!is_error);
        assert_eq!(
            external.project.figures.len(),
            ext_figures + 1,
            "the figure landed on the passed core"
        );
        assert_eq!(
            server.core.project.figures.len(),
            own_figures,
            "the server's own core gained nothing"
        );
        // The change is visible through the same external route…
        let (state, is_error) =
            call_payload_on(&mut server, &mut external, "get_state", json!({}));
        assert!(!is_error);
        assert_eq!(
            state["figures"].as_array().expect("figures array").len(),
            ext_figures + 1
        );
        // …but not through the server's own core.
        let (own_state, is_error) = call_payload(&mut server, "get_state", json!({}));
        assert!(!is_error);
        assert_eq!(
            own_state["figures"].as_array().expect("figures array").len(),
            own_figures
        );
    }

    #[test]
    fn handle_on_keeps_the_notification_rule_and_routing() {
        let mut server = McpServer::new();
        let mut external = crate::editor::EditorCore::new();
        let note = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
        assert!(server.handle_on(&mut external, &note).is_none());
        let resp = server
            .handle_on(&mut external, &request(5, "initialize", json!({})))
            .expect("initialize must be answered");
        assert_eq!(resp["result"]["protocolVersion"], "2024-11-05");
        let resp = server
            .handle_on(&mut external, &request(6, "resources/list", json!({})))
            .expect("unknown methods with an id must be answered");
        assert_eq!(resp["error"]["code"], -32601);
    }
}
