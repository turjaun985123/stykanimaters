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
//! (`server_info`, `get_state`, `set_frame`, plus the Wave-9
//! parity built-ins below — GUI operations that had no tool in
//! the other two registries: frame swap/range ops, loop range,
//! frame easing, figure move/delete, camera-key delete and
//! framing, camera follow, tween playback, motion-path
//! smoothing, named save, audio split/crossfade/mixdown,
//! particle add/clear and the 3D library, plus the Wave-10
//! round-2 built-ins: editor status, playback control,
//! selection get/set, figure keyframe jumps and delete, and
//! the transport key jumps),
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
    let mut tools = vec![
        tool_server_info(),
        tool_get_state(),
        tool_set_frame(),
        // Wave-9 parity built-ins (see the parity section below).
        tool_duplicate_frame(),
        tool_swap_frames(),
        tool_copy_frame(),
        tool_paste_frame(),
        tool_shift_frames(),
        tool_reverse_range(),
        tool_delete_range(),
        tool_copy_range(),
        tool_paste_range(),
        tool_set_loop_range(),
        tool_clear_loop_range(),
        tool_set_frame_easing(),
        tool_delete_figure(),
        tool_move_figure(),
        tool_delete_camera_key(),
        tool_frame_all_figures(),
        tool_centre_on_selection(),
        tool_save_named_project(),
        tool_audio_split(),
        tool_audio_crossfade(),
        tool_audio_mixdown(),
        tool_particle_add_event(),
        tool_clear_particles(),
        tool_fig3_library_list(),
        tool_fig3_load_from_library(),
        tool_fig3_save_to_library(),
        tool_smooth_path_roots(),
        tool_set_camera_follow(),
        tool_set_tween_playback(),
        // Wave-10 parity built-ins, round 2 (session state:
        // editor status, playback, selection, keyframe jumps).
        tool_get_editor_status(),
        tool_set_playback(),
        tool_get_selection(),
        tool_set_selection(),
        tool_goto_next_keyframe(),
        tool_goto_prev_keyframe(),
        tool_delete_keyframe(),
        tool_goto_next_key(),
        tool_goto_prev_key(),
    ];
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
// Wave-9 parity built-ins
//
// EditorCore operations the GUI exposes (panels, menus, command
// palette) that had no MCP tool in `mcp_tools` / `mcp_io`. Each
// handler mirrors the registry idioms of `mcp_tools`: explicit
// index arguments, the same argument-error wording, refusals
// surfaced as `Err(core.status)` where the operation reports them
// through the status line, and an `ok` object on success.
// ---------------------------------------------------------------------

/// Argument helper mirroring `mcp_tools::req_usize` (a JSON null
/// counts as absent; integral floats are accepted).
fn arg_usize(args: &Value, key: &str) -> Result<usize, String> {
    let Some(v) = args.get(key).filter(|v| !v.is_null()) else {
        return Err(format!("missing required argument '{key}'"));
    };
    if let Some(u) = v.as_u64() {
        return usize::try_from(u).map_err(|_| format!("argument '{key}' is too large"));
    }
    if let Some(f) = v.as_f64() {
        if f >= 0.0 && f.fract() == 0.0 && f <= usize::MAX as f64 {
            return Ok(f as usize);
        }
    }
    Err(format!("argument '{key}' must be a non-negative integer"))
}

/// Optional form of [`arg_usize`]: absent (or null) is `None`.
fn opt_usize(args: &Value, key: &str) -> Result<Option<usize>, String> {
    match args.get(key).filter(|v| !v.is_null()) {
        Some(_) => arg_usize(args, key).map(Some),
        None => Ok(None),
    }
}

fn arg_i32(args: &Value, key: &str) -> Result<i32, String> {
    let Some(v) = args.get(key).filter(|v| !v.is_null()) else {
        return Err(format!("missing required argument '{key}'"));
    };
    if let Some(i) = v.as_i64() {
        return i32::try_from(i).map_err(|_| format!("argument '{key}' is out of range"));
    }
    if let Some(f) = v.as_f64() {
        if f.fract() == 0.0 && f >= i32::MIN as f64 && f <= i32::MAX as f64 {
            return Ok(f as i32);
        }
    }
    Err(format!("argument '{key}' must be an integer"))
}

fn arg_f32(args: &Value, key: &str) -> Result<f32, String> {
    let Some(v) = args.get(key).filter(|v| !v.is_null()) else {
        return Err(format!("missing required argument '{key}'"));
    };
    v.as_f64()
        .map(|f| f as f32)
        .ok_or_else(|| format!("argument '{key}' must be a number"))
}

fn arg_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    let Some(v) = args.get(key).filter(|v| !v.is_null()) else {
        return Err(format!("missing required argument '{key}'"));
    };
    v.as_str()
        .ok_or_else(|| format!("argument '{key}' must be a string"))
}

fn check_frame(core: &crate::editor::EditorCore, frame: usize) -> Result<(), String> {
    if frame < core.project.frames.len() {
        Ok(())
    } else {
        Err(format!(
            "frame index {frame} out of range (project has {} frame(s))",
            core.project.frames.len()
        ))
    }
}

fn check_figure(core: &crate::editor::EditorCore, fi: usize) -> Result<(), String> {
    if fi < core.project.figures.len() {
        Ok(())
    } else {
        Err(format!(
            "figure index {fi} out of range (project has {} figure(s))",
            core.project.figures.len()
        ))
    }
}

fn int_schema(description: &str) -> Value {
    json!({ "type": "integer", "minimum": 0, "description": description })
}

fn tool_duplicate_frame() -> Tool {
    Tool {
        name: "duplicate_frame",
        description: "Duplicate the current frame in place (the timeline's Duplicate button): the copy becomes the current frame. One undo step.",
        schema: empty_args_schema(),
        handler: |core, _args| {
            core.duplicate_frame();
            Ok(json!({
                "ok": true,
                "current_frame": core.current,
                "frame_count": core.project.frames.len(),
                "status": core.status,
            }))
        },
    }
}

fn tool_swap_frames() -> Tool {
    Tool {
        name: "swap_frames",
        description: "Swap the CONTENT of two frames (0-based a and b): poses, durations and easings trade places and frame-pinned data (camera keys, particles, strokes, audio/FX starts, graph keys) follows its frame. Indices, playhead and loop range stay put. One undo step.",
        schema: json!({
            "type": "object",
            "properties": {
                "a": int_schema("First 0-based frame index"),
                "b": int_schema("Second 0-based frame index"),
            },
            "required": ["a", "b"],
            "additionalProperties": false,
        }),
        handler: |core, args| {
            let a = arg_usize(args, "a")?;
            let b = arg_usize(args, "b")?;
            if !crate::frame_swap::swap_frames(core, a, b) {
                return Err(core.status.clone());
            }
            Ok(json!({ "ok": true, "a": a, "b": b, "status": core.status }))
        },
    }
}

fn tool_copy_frame() -> Tool {
    Tool {
        name: "copy_frame",
        description: "Copy the current frame to the single-frame clipboard (the Edit menu's Copy frame). Not an undoable edit.",
        schema: empty_args_schema(),
        handler: |core, _args| {
            core.copy_frame();
            Ok(json!({ "ok": true, "frame": core.current, "status": core.status }))
        },
    }
}

fn tool_paste_frame() -> Tool {
    Tool {
        name: "paste_frame",
        description: "Paste the single-frame clipboard (see copy_frame) as a new frame after the current one, which becomes current. One undo step. Errors when nothing was copied or the copied frame's figure count no longer matches.",
        schema: empty_args_schema(),
        handler: |core, _args| {
            let before = core.project.frames.len();
            core.paste_frame();
            if core.project.frames.len() == before {
                return Err(core.status.clone());
            }
            Ok(json!({
                "ok": true,
                "current_frame": core.current,
                "frame_count": core.project.frames.len(),
                "status": core.status,
            }))
        },
    }
}

fn tool_shift_frames() -> Tool {
    Tool {
        name: "shift_frames",
        description: "Move a block of frames to a new position in the timeline (frame-range ops): 'count' frames starting at 'from' shift by 'delta' slots (negative moves earlier). One undo step.",
        schema: json!({
            "type": "object",
            "properties": {
                "from": int_schema("0-based index of the block's first frame"),
                "count": int_schema("How many frames the block holds (at least 1)"),
                "delta": { "type": "integer", "description": "Slots to move the block by (non-zero; negative = earlier)" },
            },
            "required": ["from", "count", "delta"],
            "additionalProperties": false,
        }),
        handler: |core, args| {
            let from = arg_usize(args, "from")?;
            let count = arg_usize(args, "count")?;
            let delta = arg_i32(args, "delta")?;
            let n = core.project.frames.len();
            if count == 0 {
                return Err("argument 'count' must be at least 1".to_string());
            }
            check_frame(core, from)?;
            if from + count > n {
                return Err(format!(
                    "frame range {from}..{} exceeds the timeline ({n} frame(s))",
                    from + count
                ));
            }
            if delta == 0 {
                return Err("argument 'delta' must be non-zero".to_string());
            }
            let target = from as i64 + delta as i64;
            if target < 0 || target as usize + count > n {
                return Err(format!(
                    "shifting frames {from}..{} by {delta} would leave the timeline",
                    from + count
                ));
            }
            if !core.shift_frames(from, count, delta) {
                return Err(core.status.clone());
            }
            Ok(json!({
                "ok": true,
                "frame_count": n,
                "status": core.status,
            }))
        },
    }
}

fn tool_reverse_range() -> Tool {
    Tool {
        name: "reverse_range",
        description: "Reverse a block of frames in place (frame-range ops): 'count' frames starting at 'from' (clamped to the timeline end) play backwards. One undo step.",
        schema: json!({
            "type": "object",
            "properties": {
                "from": int_schema("0-based index of the block's first frame"),
                "count": int_schema("How many frames to reverse (at least 2)"),
            },
            "required": ["from", "count"],
            "additionalProperties": false,
        }),
        handler: |core, args| {
            let from = arg_usize(args, "from")?;
            let count = arg_usize(args, "count")?;
            check_frame(core, from)?;
            let effective = count.min(core.project.frames.len() - from);
            if effective < 2 {
                return Err(
                    "nothing to reverse: the range holds fewer than two frames".to_string(),
                );
            }
            core.reverse_range(from, count);
            Ok(json!({ "ok": true, "reversed": effective, "status": core.status }))
        },
    }
}

fn tool_delete_range() -> Tool {
    Tool {
        name: "delete_range",
        description: "Delete a block of frames (frame-range ops): 'count' frames starting at 'from' (clamped to the timeline end). Refuses to delete every frame — a project always keeps at least one. One undo step.",
        schema: json!({
            "type": "object",
            "properties": {
                "from": int_schema("0-based index of the block's first frame"),
                "count": int_schema("How many frames to delete (at least 1)"),
            },
            "required": ["from", "count"],
            "additionalProperties": false,
        }),
        handler: |core, args| {
            let from = arg_usize(args, "from")?;
            let count = arg_usize(args, "count")?;
            if count == 0 {
                return Err("argument 'count' must be at least 1".to_string());
            }
            check_frame(core, from)?;
            let n = core.project.frames.len();
            if count.min(n - from) == n {
                return Err(
                    "cannot delete every frame: a project always keeps at least one frame"
                        .to_string(),
                );
            }
            if !core.delete_range(from, count) {
                return Err(core.status.clone());
            }
            Ok(json!({
                "ok": true,
                "frame_count": core.project.frames.len(),
                "status": core.status,
            }))
        },
    }
}

fn tool_copy_range() -> Tool {
    Tool {
        name: "copy_range",
        description: "Copy a block of frames into the range clipboard (frame-range ops): 'count' frames starting at 'from' (clamped to the timeline end), ready for paste_range. Not an undoable edit.",
        schema: json!({
            "type": "object",
            "properties": {
                "from": int_schema("0-based index of the block's first frame"),
                "count": int_schema("How many frames to copy (at least 1)"),
            },
            "required": ["from", "count"],
            "additionalProperties": false,
        }),
        handler: |core, args| {
            let from = arg_usize(args, "from")?;
            let count = arg_usize(args, "count")?;
            if count == 0 {
                return Err("argument 'count' must be at least 1".to_string());
            }
            check_frame(core, from)?;
            let copied = count.min(core.project.frames.len() - from);
            if !core.copy_range(from, count) {
                return Err(core.status.clone());
            }
            Ok(json!({ "ok": true, "copied": copied, "status": core.status }))
        },
    }
}

fn tool_paste_range() -> Tool {
    Tool {
        name: "paste_range",
        description: "Insert the range clipboard's frames (see copy_range) at 0-based index 'at' (clamped to the timeline, so a large 'at' appends), shifting later frames. One undo step. Errors when the range clipboard is empty.",
        schema: json!({
            "type": "object",
            "properties": {
                "at": int_schema("0-based insertion index"),
            },
            "required": ["at"],
            "additionalProperties": false,
        }),
        handler: |core, args| {
            let at = arg_usize(args, "at")?;
            let status_before = core.status.clone();
            if !core.paste_range(at) {
                if core.status != status_before {
                    return Err(core.status.clone());
                }
                return Err(
                    "nothing to paste: the range clipboard is empty — call copy_range first"
                        .to_string(),
                );
            }
            Ok(json!({
                "ok": true,
                "frame_count": core.project.frames.len(),
                "status": core.status,
            }))
        },
    }
}

fn tool_set_loop_range() -> Tool {
    Tool {
        name: "set_loop_range",
        description: "Set the inclusive playback loop range [start, end] (0-based; the timeline's Set loop row). Endpoints are ordered and clamped to the timeline; start == end loops a single frame. Editor runtime state, like the playhead — not an undoable edit.",
        schema: json!({
            "type": "object",
            "properties": {
                "start": int_schema("0-based first frame of the loop"),
                "end": int_schema("0-based last frame of the loop (inclusive)"),
            },
            "required": ["start", "end"],
            "additionalProperties": false,
        }),
        handler: |core, args| {
            let start = arg_usize(args, "start")?;
            let end = arg_usize(args, "end")?;
            core.set_loop_range(start, end);
            let loop_range = match core.loop_range {
                Some((a, b)) => json!([a, b]),
                None => Value::Null,
            };
            Ok(json!({
                "ok": true,
                "loop_range": loop_range,
                "status": core.status,
            }))
        },
    }
}

fn tool_clear_loop_range() -> Tool {
    Tool {
        name: "clear_loop_range",
        description: "Clear the playback loop range (the timeline's loop Clear), so playback wraps at the timeline end again.",
        schema: empty_args_schema(),
        handler: |core, _args| {
            core.clear_loop_range();
            Ok(json!({
                "ok": true,
                "loop_range": Value::Null,
                "status": core.status,
            }))
        },
    }
}

fn tool_set_frame_easing() -> Tool {
    Tool {
        name: "set_frame_easing",
        description: "Set one frame's easing override (the easing panel's frame picker): \"linear\", \"smooth\", \"ease-in\" or \"ease-out\", or null for the project default. Applies to the current frame unless 'frame' names another. One undo step.",
        schema: json!({
            "type": "object",
            "properties": {
                "easing": {
                    "type": ["string", "null"],
                    "description": "\"linear\", \"smooth\", \"ease-in\", \"ease-out\", or null for the project default",
                },
                "frame": int_schema("Optional 0-based frame to apply to (moves the playhead there first)"),
            },
            "required": ["easing"],
            "additionalProperties": false,
        }),
        handler: |core, args| {
            let easing = match args.get("easing") {
                None => return Err("missing required argument 'easing'".to_string()),
                Some(Value::Null) => None,
                Some(v) => Some(match v.as_str() {
                    Some("linear") => crate::model::Easing::Linear,
                    Some("smooth") => crate::model::Easing::Smooth,
                    Some("ease-in") => crate::model::Easing::EaseIn,
                    Some("ease-out") => crate::model::Easing::EaseOut,
                    Some(other) => {
                        return Err(format!(
                            "unknown easing '{other}' (expected \"linear\", \"smooth\", \"ease-in\", \"ease-out\" or null)"
                        ));
                    }
                    None => {
                        return Err("argument 'easing' must be a string or null".to_string());
                    }
                }),
            };
            if let Some(frame) = opt_usize(args, "frame")? {
                check_frame(core, frame)?;
                core.goto(frame);
            }
            core.set_frame_easing(easing);
            Ok(json!({
                "ok": true,
                "frame": core.current,
                "easing": core.project.frames[core.current]
                    .easing
                    .map(|e| e.label()),
                "status": core.status,
            }))
        },
    }
}

fn tool_delete_figure() -> Tool {
    Tool {
        name: "delete_figure",
        description: "Delete a figure (0-based 'figure'; the figures panel's Delete): its pose rows go with it and every figure-indexed reference (particles, FX clips, graph channels, camera follow) is remapped. Refuses to delete the last figure. One undo step.",
        schema: json!({
            "type": "object",
            "properties": {
                "figure": int_schema("0-based index of the figure to delete"),
            },
            "required": ["figure"],
            "additionalProperties": false,
        }),
        handler: |core, args| {
            let fi = arg_usize(args, "figure")?;
            check_figure(core, fi)?;
            // EditorCore::delete_figure deletes the SELECTED
            // figure; select the target first, as the GUI does.
            core.selected_figure = fi;
            let before = core.project.figures.len();
            core.delete_figure();
            if core.project.figures.len() == before {
                return Err(core.status.clone());
            }
            Ok(json!({
                "ok": true,
                "figures": core.project.figures.len(),
                "status": core.status,
            }))
        },
    }
}

fn tool_move_figure() -> Tool {
    Tool {
        name: "move_figure",
        description: "Move a figure in the draw order (the figures/layers panels' up/down controls): 'direction' steps the figure later (positive, drawn on top) or earlier (negative), clamped at the ends; poses, bends and every figure-indexed reference are remapped. 'figure' defaults to the selected figure. One undo step.",
        schema: json!({
            "type": "object",
            "properties": {
                "direction": { "type": "integer", "description": "Steps to move (non-zero; positive = later/on top)" },
                "figure": int_schema("Optional 0-based figure to move (defaults to the selected figure)"),
            },
            "required": ["direction"],
            "additionalProperties": false,
        }),
        handler: |core, args| {
            if let Some(fi) = opt_usize(args, "figure")? {
                check_figure(core, fi)?;
                core.selected_figure = fi;
            }
            let direction = arg_i32(args, "direction")?;
            let before = core.selected_figure;
            core.move_figure_full(direction);
            Ok(json!({
                "ok": true,
                "figure": core.selected_figure,
                "moved": core.selected_figure != before,
                "status": core.status,
            }))
        },
    }
}

fn tool_delete_camera_key() -> Tool {
    Tool {
        name: "delete_camera_key",
        description: "Delete the camera key at a 0-based frame (the camera panel's per-key Delete). One undo step. Errors when the frame holds no camera key.",
        schema: json!({
            "type": "object",
            "properties": {
                "frame": int_schema("0-based frame whose camera key to delete"),
            },
            "required": ["frame"],
            "additionalProperties": false,
        }),
        handler: |core, args| {
            let frame = arg_usize(args, "frame")?;
            // The same semantics as the camera panel's
            // delete_camera_key_at helper: refuse without a
            // snapshot when the frame holds no key, otherwise
            // snapshot once and remove exactly that key.
            if !core.project.camera.iter().any(|k| k.frame == frame) {
                return Err(format!("No camera key at frame {}.", frame + 1));
            }
            core.snapshot();
            core.project.remove_camera_key(frame);
            core.status = format!("Camera key at frame {} deleted.", frame + 1);
            Ok(json!({ "ok": true, "frame": frame, "status": core.status }))
        },
    }
}

fn tool_frame_all_figures() -> Tool {
    Tool {
        name: "frame_all_figures",
        description: "Frame all visible figures with the camera at the current frame (the View menu's Frame all): upserts a camera key centred on the figures' bounds with a fitting zoom. One undo step.",
        schema: empty_args_schema(),
        handler: |core, _args| {
            if !core.project.figures.iter().any(|f| !f.hidden) {
                return Err("No figures to frame.".to_string());
            }
            core.frame_all_figures();
            let (x, y, zoom) = core.camera_here();
            Ok(json!({
                "ok": true,
                "camera": { "x": x, "y": y, "zoom": zoom },
                "status": core.status,
            }))
        },
    }
}

fn tool_centre_on_selection() -> Tool {
    Tool {
        name: "centre_on_selection",
        description: "Centre the camera on the selected figure's root at the current frame (the View menu's Centre on selection): upserts a camera key with the current zoom and angle, only the centre moved. One undo step.",
        schema: empty_args_schema(),
        handler: |core, _args| {
            if core.selected_figure >= core.project.figures.len() {
                return Err("No figures to centre on.".to_string());
            }
            core.centre_on_selection();
            let (x, y, zoom) = core.camera_here();
            Ok(json!({
                "ok": true,
                "figure": core.selected_figure,
                "camera": { "x": x, "y": y, "zoom": zoom },
                "status": core.status,
            }))
        },
    }
}

fn tool_save_named_project() -> Tool {
    Tool {
        name: "save_named_project",
        description: "Save the project under its own name in ./projects/ (the GUI's Save: the file name is the slugified project name, and the saved-projects list is refreshed). Distinct from save_project, which writes the JSON to an explicit path. Errors when the write fails.",
        schema: empty_args_schema(),
        handler: |core, _args| {
            if !core.save_named_project() {
                return Err(core.status.clone());
            }
            Ok(json!({
                "ok": true,
                "saved": true,
                "project": core.project.name,
                "status": core.status,
            }))
        },
    }
}

fn tool_audio_split() -> Tool {
    Tool {
        name: "audio_split",
        description: "Split an audio clip at a frame (the audio panel's Split): clip 'clip' (0-based) is cut at 0-based 'at_frame' into two clips. Errors when the clip does not exist or its file cannot be read.",
        schema: json!({
            "type": "object",
            "properties": {
                "clip": int_schema("0-based index of the audio clip"),
                "at_frame": int_schema("0-based frame to split at"),
            },
            "required": ["clip", "at_frame"],
            "additionalProperties": false,
        }),
        handler: |core, args| {
            let clip = arg_usize(args, "clip")?;
            let at_frame = arg_usize(args, "at_frame")?;
            if clip >= core.project.audio.len() {
                return Err(format!("Audio split failed: no clip {}.", clip + 1));
            }
            let before = core.project.audio.len();
            core.audio_split(clip, at_frame);
            if core.project.audio.len() == before {
                return Err(core.status.clone());
            }
            Ok(json!({
                "ok": true,
                "clips": core.project.audio.len(),
                "status": core.status,
            }))
        },
    }
}

fn tool_audio_crossfade() -> Tool {
    Tool {
        name: "audio_crossfade",
        description: "Crossfade an audio clip into the following one (the audio panel's Crossfade): clip 'clip' (0-based) blends with clip 'clip'+1. Errors when the clip has no following clip or a file cannot be read.",
        schema: json!({
            "type": "object",
            "properties": {
                "clip": int_schema("0-based index of the first audio clip"),
            },
            "required": ["clip"],
            "additionalProperties": false,
        }),
        handler: |core, args| {
            let clip = arg_usize(args, "clip")?;
            if clip + 1 >= core.project.audio.len() {
                return Err(format!(
                    "Audio crossfade failed: clip {} has no following clip.",
                    clip + 1
                ));
            }
            core.audio_crossfade(clip);
            if core.status.contains("failed") {
                return Err(core.status.clone());
            }
            Ok(json!({
                "ok": true,
                "clips": core.project.audio.len(),
                "status": core.status,
            }))
        },
    }
}

fn tool_audio_mixdown() -> Tool {
    Tool {
        name: "audio_mixdown",
        description: "Mix all audio clips down into a single mixed clip (the audio panel's Mixdown). Errors when the project has no audio clips or the mix fails.",
        schema: empty_args_schema(),
        handler: |core, _args| {
            if core.project.audio.is_empty() {
                return Err("No audio clips to mix.".to_string());
            }
            core.mixdown_audio();
            if core.status.contains("failed") {
                return Err(core.status.clone());
            }
            Ok(json!({
                "ok": true,
                "clips": core.project.audio.len(),
                "status": core.status,
            }))
        },
    }
}

fn tool_particle_add_event() -> Tool {
    Tool {
        name: "particle_add_event",
        description: "Add a particle event at a stage position on a frame (the Particles panel's add): 0-based 'frame', 'x'/'y' in stage coordinates. The event takes the panel's default kind and the selected figure's colour. One undo step.",
        schema: json!({
            "type": "object",
            "properties": {
                "frame": int_schema("0-based frame the event fires on"),
                "x": { "type": "number", "description": "Stage x coordinate" },
                "y": { "type": "number", "description": "Stage y coordinate" },
            },
            "required": ["frame", "x", "y"],
            "additionalProperties": false,
        }),
        handler: |core, args| {
            let frame = arg_usize(args, "frame")?;
            check_frame(core, frame)?;
            let x = arg_f32(args, "x")?;
            let y = arg_f32(args, "y")?;
            let index = core.particle_add_event(frame, x, y);
            Ok(json!({
                "ok": true,
                "index": index,
                "particles": core.project.particles.len(),
            }))
        },
    }
}

fn tool_clear_particles() -> Tool {
    Tool {
        name: "clear_particles",
        description: "Clear ALL particle events and FX clips (the command palette's Clear particles / the panel Clear). One undo step.",
        schema: empty_args_schema(),
        handler: |core, _args| {
            core.clear_particles();
            Ok(json!({ "ok": true, "status": core.status }))
        },
    }
}

fn tool_fig3_library_list() -> Tool {
    Tool {
        name: "fig3_library_list",
        description: "List the display names of every figure in the 3D library (the 3D panel's library), sorted; empty when the library holds nothing readable.",
        schema: empty_args_schema(),
        handler: |core, _args| {
            Ok(json!({ "ok": true, "figures": core.fig3_library_list() }))
        },
    }
}

fn tool_fig3_load_from_library() -> Tool {
    Tool {
        name: "fig3_load_from_library",
        description: "Load a 3D library figure by name (the 3D panel's library Load), replacing the current 3D session or starting one; an active session's camera is kept. Errors when no library figure has that name.",
        schema: json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "Display name from fig3_library_list" },
            },
            "required": ["name"],
            "additionalProperties": false,
        }),
        handler: |core, args| {
            let name = arg_str(args, "name")?;
            if !core.fig3_library_list().iter().any(|n| n == name) {
                return Err(format!("No 3D library figure named '{name}'."));
            }
            core.fig3_load_from_library(name);
            Ok(json!({ "ok": true, "name": name, "status": core.status }))
        },
    }
}

fn tool_fig3_save_to_library() -> Tool {
    Tool {
        name: "fig3_save_to_library",
        description: "Save the current 3D session's figure and pose to the 3D library under the figure's name (the 3D panel's Save to library). Errors when no 3D session is active (see fig3_enter) or the write fails.",
        schema: empty_args_schema(),
        handler: |core, _args| {
            if core.fig3.is_none() {
                return Err("Enter 3D mode first.".to_string());
            }
            core.fig3_save_to_library();
            if core.status.starts_with("Could not save") {
                return Err(core.status.clone());
            }
            Ok(json!({ "ok": true, "status": core.status }))
        },
    }
}

fn tool_smooth_path_roots() -> Tool {
    Tool {
        name: "smooth_path_roots",
        description: "Smooth a figure's motion path (the frame-ops Smooth path button): each interior frame's root for 'figure' (0-based) is pulled to the midpoint of its neighbours, bending the path without touching angles. Needs at least three frames. One undo step when anything moves.",
        schema: json!({
            "type": "object",
            "properties": {
                "figure": int_schema("0-based index of the figure whose path to smooth"),
            },
            "required": ["figure"],
            "additionalProperties": false,
        }),
        handler: |core, args| {
            let fi = arg_usize(args, "figure")?;
            check_figure(core, fi)?;
            core.smooth_path_roots(fi);
            if core.status == "Nothing to smooth." {
                return Err(core.status.clone());
            }
            Ok(json!({ "ok": true, "figure": fi, "status": core.status }))
        },
    }
}

fn tool_set_camera_follow() -> Tool {
    Tool {
        name: "set_camera_follow",
        description: "Set which figure the camera follows during playback (the camera panel's Follow toggle): 'figure' is a 0-based figure index, or null to turn following off. Stored in the project.",
        schema: json!({
            "type": "object",
            "properties": {
                "figure": {
                    "type": ["integer", "null"],
                    "minimum": 0,
                    "description": "0-based figure to follow, or null for no follow",
                },
            },
            "required": ["figure"],
            "additionalProperties": false,
        }),
        handler: |core, args| {
            // The layers tool's precedent: project fields may be
            // set directly; the status strings are the toggle
            // operation's own.
            match args.get("figure") {
                None => Err("missing required argument 'figure'".to_string()),
                Some(Value::Null) => {
                    core.project.camera_follow = None;
                    core.status = "Camera follow off.".to_string();
                    Ok(json!({
                        "ok": true,
                        "follow": Value::Null,
                        "status": core.status,
                    }))
                }
                Some(_) => {
                    let fi = arg_usize(args, "figure")?;
                    check_figure(core, fi)?;
                    core.project.camera_follow = Some(fi);
                    core.status = format!(
                        "Camera follows {}.",
                        core.project.figures[fi].name
                    );
                    Ok(json!({
                        "ok": true,
                        "follow": fi,
                        "status": core.status,
                    }))
                }
            }
        },
    }
}

fn tool_set_tween_playback() -> Tool {
    Tool {
        name: "set_tween_playback",
        description: "Turn tweened (interpolated) playback on or off (the Tween playback toggle). Stored in the project.",
        schema: json!({
            "type": "object",
            "properties": {
                "enabled": { "type": "boolean", "description": "True for tweened playback, false for stepped" },
            },
            "required": ["enabled"],
            "additionalProperties": false,
        }),
        handler: |core, args| {
            let Some(v) = args.get("enabled").filter(|v| !v.is_null()) else {
                return Err("missing required argument 'enabled'".to_string());
            };
            let Some(enabled) = v.as_bool() else {
                return Err("argument 'enabled' must be a boolean".to_string());
            };
            if core.project.tween_playback != enabled {
                core.toggle_tween_playback();
            }
            Ok(json!({
                "ok": true,
                "enabled": core.project.tween_playback,
                "status": core.status,
            }))
        },
    }
}

// ---------------------------------------------------------------------
// Wave-10 parity built-ins (round 2)
//
// Session-state operations the GUI exposes that round 1 did not
// cover. `get_state` above reports the project's shape and
// `mcp_tools` carries `undo` / `redo` and `get_project_stats`;
// what no tool could do before this round was observe or drive
// the session state around the project: whether playback is
// running, where the fractional playhead sits, which figure and
// node are selected, how deep the undo history is, the
// transport's Prev/Next key jumps, and the dopesheet's
// per-figure keyframe navigation and delete (the
// `EditorCore` keyframe ops that landed with Wave 10's
// dopesheet). Each handler below
// mirrors the GUI operation it names — same core field or
// method, same semantics — against whichever core the router
// was handed, so over the live-control path they act on the
// session the user is looking at.
// ---------------------------------------------------------------------

fn tool_get_editor_status() -> Tool {
    Tool {
        name: "get_editor_status",
        description: "Read-only snapshot of the editor session (not the project file): playing state, current frame, fractional playhead, loop range (or null), the selected figure and node, undo/redo history depths with can_undo/can_redo, and the status line. EditorCore exposes no saved/dirty flag — the undo depth is the observable edit history. Complements get_state, which reports the project's shape.",
        schema: empty_args_schema(),
        handler: |core, _args| {
            let loop_range = match core.loop_range {
                Some((a, b)) => json!([a, b]),
                None => Value::Null,
            };
            Ok(json!({
                "playing": core.playing,
                "current_frame": core.current,
                "playhead": core.playhead,
                "loop_range": loop_range,
                "selected_figure": core.selected_figure,
                "selected_node": core.selected_node,
                "can_undo": core.undo_depth() > 0,
                "can_redo": core.redo_depth() > 0,
                "undo_depth": core.undo_depth(),
                "redo_depth": core.redo_depth(),
                "status": core.status,
            }))
        },
    }
}

fn tool_set_playback() -> Tool {
    Tool {
        name: "set_playback",
        description: "Drive playback, mirroring the transport buttons: 'play' / 'pause' / 'toggle' flip the same playing flag the chrome Play/Pause button flips; 'restart' goes to frame 0 and plays (the Restart button); 'play_range' goes to the loop range's start — frame 0 when no range is set — and plays (the Play range button). Over the live-control path the GUI's poll observes the flag, so playback and its audio start exactly as if the button had been pressed.",
        schema: json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["play", "pause", "toggle", "restart", "play_range"],
                    "description": "Which transport action to perform",
                },
            },
            "required": ["action"],
            "additionalProperties": false,
        }),
        handler: |core, args| {
            let action = arg_str(args, "action")?;
            if !matches!(action, "play" | "pause" | "toggle" | "restart" | "play_range") {
                return Err(format!(
                    "invalid 'action' argument: expected one of play, pause, toggle, restart, play_range; got \"{action}\""
                ));
            }
            if core.project.frames.is_empty() {
                return Err("the project has no frames to play".to_string());
            }
            match action {
                "play" => core.playing = true,
                "pause" => core.playing = false,
                "toggle" => core.playing = !core.playing,
                // The Restart / Play range composition the shell
                // shares (`start_playback_at` in
                // playback_controls): `goto` also resets the
                // fractional playhead, then the flag is set.
                "restart" => {
                    core.goto(0);
                    core.playing = true;
                }
                "play_range" => {
                    let start = core.loop_range.map(|(a, _)| a).unwrap_or(0);
                    core.goto(start);
                    core.playing = true;
                }
                _ => unreachable!("the action was validated above"),
            }
            Ok(json!({
                "ok": true,
                "playing": core.playing,
                "current_frame": core.current,
            }))
        },
    }
}

fn tool_get_selection() -> Tool {
    Tool {
        name: "get_selection",
        description: "Read the current selection: the selected figure's 0-based index and name (null if the index is somehow out of range), and the selected node index within that figure (null when no node is selected).",
        schema: empty_args_schema(),
        handler: |core, _args| {
            let figure_name = core
                .project
                .figures
                .get(core.selected_figure)
                .map(|f| f.name.clone());
            Ok(json!({
                "selected_figure": core.selected_figure,
                "figure_name": figure_name,
                "selected_node": core.selected_node,
            }))
        },
    }
}

fn tool_set_selection() -> Tool {
    Tool {
        name: "set_selection",
        description: "Select a figure by 0-based index (the figures panel row press: EditorCore::select_figure, which also clears the node selection). Optionally also select a node of that figure by 0-based index, as a stage click on the figure does; omit 'node' (or pass null) to leave the node selection cleared. An out-of-range figure or node is an error and leaves the selection untouched.",
        schema: json!({
            "type": "object",
            "properties": {
                "figure": int_schema("0-based index of the figure to select"),
                "node": {
                    "type": ["integer", "null"],
                    "minimum": 0,
                    "description": "Optional 0-based node index within the figure, or null",
                },
            },
            "required": ["figure"],
            "additionalProperties": false,
        }),
        handler: |core, args| {
            let fi = arg_usize(args, "figure")?;
            check_figure(core, fi)?;
            // Validate the node before applying anything, so a
            // bad node cannot leave a half-moved selection.
            let node = match args.get("node") {
                None | Some(Value::Null) => None,
                Some(_) => {
                    let ni = arg_usize(args, "node")?;
                    let count = core.project.figures[fi].def.nodes.len();
                    if ni >= count {
                        return Err(format!(
                            "node index {ni} out of range (figure {fi} has {count} node(s))"
                        ));
                    }
                    Some(ni)
                }
            };
            core.select_figure(fi);
            if let Some(ni) = node {
                core.selected_node = Some(ni);
            }
            Ok(json!({
                "ok": true,
                "selected_figure": core.selected_figure,
                "selected_node": core.selected_node,
            }))
        },
    }
}

/// One graph key's timeline frame: `f32::round` (halves away
/// from zero), with out-of-timeline results dropped, never
/// clamped. A verbatim mirror of the shell's
/// `timeline_marks::graph_key_frame` — `rune_sec` is
/// binary-crate code the library cannot call, and the two must
/// agree or the MCP jumps would land off the markers the
/// timeline draws.
fn graph_key_frame(t: f32, count: usize) -> Option<usize> {
    if count == 0 || !t.is_finite() {
        return None;
    }
    let f = t.round();
    if f < 0.0 || f >= count as f32 {
        return None;
    }
    Some(f as usize)
}

/// Every key frame in the project, sorted ascending with
/// duplicates removed: the union of the camera keys and all
/// graph channel keys — the exact set the transport's
/// Prev/Next key buttons walk (`playback_controls::key_frames`
/// in the shell, mirrored here for the same reason as
/// [`graph_key_frame`]). Camera keys stored past the timeline
/// end are dropped, matching the timeline's markers.
fn key_frames_of(core: &crate::editor::EditorCore) -> Vec<usize> {
    let count = core.frame_count();
    let mut frames: Vec<usize> = Vec::new();
    for key in &core.project.camera {
        if key.frame < count {
            frames.push(key.frame);
        }
    }
    for channel in &core.project.channels {
        for key in &channel.keys {
            if let Some(f) = graph_key_frame(key.frame, count) {
                frames.push(f);
            }
        }
    }
    frames.sort_unstable();
    frames.dedup();
    frames
}

fn tool_goto_next_keyframe() -> Tool {
    Tool {
        name: "goto_next_keyframe",
        description: "Move the playhead to a figure's next keyframe (EditorCore::goto_next_keyframe, the dopesheet's navigation): a figure's keyframes are the frames where its pose changes (stats::figure_keyframes). 'figure' is a 0-based figure index. An error when the figure has no later keyframe — the core's own refusal, carried on its status line.",
        schema: json!({
            "type": "object",
            "properties": {
                "figure": int_schema("0-based index of the figure whose keyframes to walk"),
            },
            "required": ["figure"],
            "additionalProperties": false,
        }),
        handler: |core, args| {
            let fi = arg_usize(args, "figure")?;
            check_figure(core, fi)?;
            if !core.goto_next_keyframe(fi) {
                return Err(core.status.clone());
            }
            Ok(json!({
                "ok": true,
                "figure": fi,
                "current_frame": core.current,
                "key_frames": crate::stats::figure_keyframes(&core.project, fi),
                "status": core.status,
            }))
        },
    }
}

fn tool_goto_prev_keyframe() -> Tool {
    Tool {
        name: "goto_prev_keyframe",
        description: "Move the playhead to a figure's previous keyframe (EditorCore::goto_prev_keyframe, the dopesheet's navigation): a figure's keyframes are the frames where its pose changes (stats::figure_keyframes). 'figure' is a 0-based figure index. An error when the figure has no earlier keyframe — the core's own refusal, carried on its status line.",
        schema: json!({
            "type": "object",
            "properties": {
                "figure": int_schema("0-based index of the figure whose keyframes to walk"),
            },
            "required": ["figure"],
            "additionalProperties": false,
        }),
        handler: |core, args| {
            let fi = arg_usize(args, "figure")?;
            check_figure(core, fi)?;
            if !core.goto_prev_keyframe(fi) {
                return Err(core.status.clone());
            }
            Ok(json!({
                "ok": true,
                "figure": fi,
                "current_frame": core.current,
                "key_frames": crate::stats::figure_keyframes(&core.project, fi),
                "status": core.status,
            }))
        },
    }
}

fn tool_delete_keyframe() -> Tool {
    Tool {
        name: "delete_keyframe",
        description: "Delete a figure's keyframe at a frame (EditorCore::delete_keyframe, the dopesheet's delete): the frame takes the previous frame's held pose, so the figure no longer changes there. 'figure' and 'frame' are 0-based indices. One undo step. The core's refusals — frame 0's base pose, a frame that is not a keyframe for the figure, a frame where the figure appears — are errors carrying its status line.",
        schema: json!({
            "type": "object",
            "properties": {
                "figure": int_schema("0-based index of the figure whose keyframe to delete"),
                "frame": int_schema("0-based frame of the keyframe to delete"),
            },
            "required": ["figure", "frame"],
            "additionalProperties": false,
        }),
        handler: |core, args| {
            let fi = arg_usize(args, "figure")?;
            let frame = arg_usize(args, "frame")?;
            check_figure(core, fi)?;
            check_frame(core, frame)?;
            if !core.delete_keyframe(fi, frame) {
                return Err(core.status.clone());
            }
            Ok(json!({
                "ok": true,
                "figure": fi,
                "frame": frame,
                "key_frames": crate::stats::figure_keyframes(&core.project, fi),
                "status": core.status,
            }))
        },
    }
}

fn tool_goto_next_key() -> Tool {
    Tool {
        name: "goto_next_key",
        description: "Move the playhead to the next key strictly after the current frame (the transport's Next key button, pb-next-key): the keys are the sorted union of the camera keys and every graph channel's keys — not any one figure's pose keys (for those, goto_next_keyframe). Like the button, the jump only moves the playhead — playback state is untouched. An error when no key follows (the button renders inert in that state).",
        schema: empty_args_schema(),
        handler: |core, _args| {
            let keys = key_frames_of(core);
            let Some(target) = keys.iter().copied().filter(|k| *k > core.current).min() else {
                return Err(format!(
                    "no key after frame {} ({} key(s) in the project)",
                    core.current,
                    keys.len()
                ));
            };
            core.goto(target);
            Ok(json!({
                "ok": true,
                "current_frame": core.current,
                "key_frames": keys,
            }))
        },
    }
}

fn tool_goto_prev_key() -> Tool {
    Tool {
        name: "goto_prev_key",
        description: "Move the playhead to the previous key strictly before the current frame (the transport's Prev key button, pb-prev-key): the keys are the sorted union of the camera keys and every graph channel's keys — not any one figure's pose keys (for those, goto_prev_keyframe). Like the button, the jump only moves the playhead — playback state is untouched. An error when no key precedes (the button renders inert in that state).",
        schema: empty_args_schema(),
        handler: |core, _args| {
            let keys = key_frames_of(core);
            let Some(target) = keys.iter().copied().filter(|k| *k < core.current).max() else {
                return Err(format!(
                    "no key before frame {} ({} key(s) in the project)",
                    core.current,
                    keys.len()
                ));
            };
            core.goto(target);
            Ok(json!({
                "ok": true,
                "current_frame": core.current,
                "key_frames": keys,
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

    // ---- Wave-9 parity built-ins ----

    #[test]
    fn wave9_parity_tools_are_all_registered() {
        let names: Vec<&str> = all_tools().iter().map(|t| t.name).collect();
        for expected in [
            "duplicate_frame",
            "swap_frames",
            "copy_frame",
            "paste_frame",
            "shift_frames",
            "reverse_range",
            "delete_range",
            "copy_range",
            "paste_range",
            "set_loop_range",
            "clear_loop_range",
            "set_frame_easing",
            "delete_figure",
            "move_figure",
            "delete_camera_key",
            "frame_all_figures",
            "centre_on_selection",
            "save_named_project",
            "audio_split",
            "audio_crossfade",
            "audio_mixdown",
            "particle_add_event",
            "clear_particles",
            "fig3_library_list",
            "fig3_load_from_library",
            "fig3_save_to_library",
            "smooth_path_roots",
            "set_camera_follow",
            "set_tween_playback",
        ] {
            assert!(names.contains(&expected), "missing parity tool {expected}");
        }
        // Pre-existing tools are untouched and still registered.
        for expected in ["add_figure", "set_camera", "layers", "save_project", "fig3_enter"] {
            assert!(names.contains(&expected), "lost pre-existing tool {expected}");
        }
    }

    #[test]
    fn duplicate_frame_and_swap_frames_dispatch() {
        let mut server = McpServer::new();
        let n0 = server.core.project.frames.len();
        let (out, is_error) = call_payload(&mut server, "duplicate_frame", json!({}));
        assert!(!is_error);
        assert_eq!(out["frame_count"].as_u64(), Some((n0 + 1) as u64));
        assert_eq!(server.core.project.frames.len(), n0 + 1);

        server.core.project.frames[0].duration = 2.0;
        server.core.project.frames[1].duration = 3.0;
        let playhead = server.core.current;
        let (out, is_error) =
            call_payload(&mut server, "swap_frames", json!({ "a": 0, "b": 1 }));
        assert!(!is_error);
        assert_eq!(out["a"], 0);
        assert_eq!(server.core.project.frames[0].duration, 3.0);
        assert_eq!(server.core.project.frames[1].duration, 2.0);
        assert_eq!(server.core.current, playhead, "content moves, not the playhead");

        let (_out, is_error) =
            call_payload(&mut server, "swap_frames", json!({ "a": 0, "b": 99 }));
        assert!(is_error, "out-of-range swap refuses");
    }

    #[test]
    fn frame_range_tools_dispatch() {
        let mut server = McpServer::new();
        let n0 = server.core.project.frames.len();

        // Range clipboard round trip.
        let (out, is_error) =
            call_payload(&mut server, "copy_range", json!({ "from": 0, "count": 2 }));
        assert!(!is_error);
        assert_eq!(out["copied"], 2);
        let (out, is_error) =
            call_payload(&mut server, "paste_range", json!({ "at": n0 }));
        assert!(!is_error);
        assert_eq!(out["frame_count"].as_u64(), Some((n0 + 2) as u64));

        // An empty range clipboard refuses on a fresh server.
        let mut fresh = McpServer::new();
        let (_out, is_error) = call_payload(&mut fresh, "paste_range", json!({ "at": 0 }));
        assert!(is_error, "paste with an empty range clipboard refuses");

        // Reverse is visible in the frame contents.
        server.core.project.frames[0].duration = 2.0;
        server.core.project.frames[1].duration = 3.0;
        let (_out, is_error) =
            call_payload(&mut server, "reverse_range", json!({ "from": 0, "count": 2 }));
        assert!(!is_error);
        assert_eq!(server.core.project.frames[0].duration, 3.0);
        assert_eq!(server.core.project.frames[1].duration, 2.0);

        // Shifting off the timeline edge refuses.
        let (_out, is_error) = call_payload(
            &mut server,
            "shift_frames",
            json!({ "from": 0, "count": 1, "delta": -1 }),
        );
        assert!(is_error, "shift past the start refuses");

        // Deleting everything refuses; deleting a subrange works.
        let all = server.core.project.frames.len();
        let (_out, is_error) = call_payload(
            &mut server,
            "delete_range",
            json!({ "from": 0, "count": all }),
        );
        assert!(is_error, "deleting every frame refuses");
        let (out, is_error) =
            call_payload(&mut server, "delete_range", json!({ "from": 0, "count": 1 }));
        assert!(!is_error);
        assert_eq!(out["frame_count"].as_u64(), Some((all - 1) as u64));

        // Single-frame clipboard round trip, and its empty refusal.
        let (_out, is_error) = call_payload(&mut server, "copy_frame", json!({}));
        assert!(!is_error);
        let before = server.core.project.frames.len();
        let (_out, is_error) = call_payload(&mut server, "paste_frame", json!({}));
        assert!(!is_error);
        assert_eq!(server.core.project.frames.len(), before + 1);
        let mut fresh = McpServer::new();
        let (_out, is_error) = call_payload(&mut fresh, "paste_frame", json!({}));
        assert!(is_error, "paste with an empty frame clipboard refuses");
    }

    #[test]
    fn loop_range_and_frame_easing_tools_dispatch() {
        let mut server = McpServer::new();
        // Endpoints are ordered by the operation itself.
        let (out, is_error) =
            call_payload(&mut server, "set_loop_range", json!({ "start": 2, "end": 0 }));
        assert!(!is_error);
        assert_eq!(out["loop_range"], json!([0, 2]));
        assert_eq!(server.core.loop_range, Some((0, 2)));
        let (out, is_error) = call_payload(&mut server, "clear_loop_range", json!({}));
        assert!(!is_error);
        assert_eq!(out["loop_range"], Value::Null);
        assert_eq!(server.core.loop_range, None);

        let (out, is_error) =
            call_payload(&mut server, "set_frame_easing", json!({ "easing": "smooth" }));
        assert!(!is_error);
        assert!(matches!(
            server.core.project.frames[server.core.current].easing,
            Some(crate::model::Easing::Smooth)
        ));
        assert_eq!(out["easing"], "smooth");
        // A named frame is applied to directly.
        let (_out, is_error) = call_payload(
            &mut server,
            "set_frame_easing",
            json!({ "frame": 0, "easing": "ease-in" }),
        );
        assert!(!is_error);
        assert!(matches!(
            server.core.project.frames[0].easing,
            Some(crate::model::Easing::EaseIn)
        ));
        // Null returns the frame to the project default.
        let (_out, is_error) =
            call_payload(&mut server, "set_frame_easing", json!({ "easing": null }));
        assert!(!is_error);
        assert_eq!(server.core.project.frames[server.core.current].easing, None);
        let (_out, is_error) =
            call_payload(&mut server, "set_frame_easing", json!({ "easing": "bounce" }));
        assert!(is_error, "an unknown easing name refuses");
    }

    #[test]
    fn figure_move_and_delete_tools_dispatch() {
        let mut server = McpServer::new();
        // The demo's single figure cannot be deleted.
        let (_out, is_error) =
            call_payload(&mut server, "delete_figure", json!({ "figure": 0 }));
        assert!(is_error, "the last figure is kept");
        let first_name = server.core.project.figures[0].name.clone();

        let (_out, is_error) =
            call_payload(&mut server, "add_figure", json!({ "kind": "mini" }));
        assert!(!is_error);
        assert_eq!(server.core.project.figures.len(), 2);

        // Move the added figure ahead of the original.
        let (out, is_error) = call_payload(
            &mut server,
            "move_figure",
            json!({ "figure": 1, "direction": -1 }),
        );
        assert!(!is_error);
        assert_eq!(out["figure"], 0);
        assert_eq!(out["moved"], true);
        assert_eq!(server.core.project.figures[1].name, first_name);

        let (out, is_error) =
            call_payload(&mut server, "delete_figure", json!({ "figure": 0 }));
        assert!(!is_error);
        assert_eq!(out["figures"], 1);
        assert_eq!(server.core.project.figures.len(), 1);
    }

    #[test]
    fn camera_parity_tools_dispatch() {
        let mut server = McpServer::new();
        let (_out, is_error) = call_payload(
            &mut server,
            "set_camera",
            json!({ "frame": 0, "x": 10.0, "y": 20.0, "zoom": 1.5 }),
        );
        assert!(!is_error);
        assert_eq!(server.core.project.camera.len(), 1);

        let (out, is_error) =
            call_payload(&mut server, "delete_camera_key", json!({ "frame": 0 }));
        assert!(!is_error);
        assert_eq!(out["frame"], 0);
        assert!(server.core.project.camera.is_empty());
        let (_out, is_error) =
            call_payload(&mut server, "delete_camera_key", json!({ "frame": 0 }));
        assert!(is_error, "deleting a key that is not there refuses");

        let (out, is_error) = call_payload(&mut server, "frame_all_figures", json!({}));
        assert!(!is_error);
        assert!(out["camera"]["zoom"].is_number());
        assert!(server
            .core
            .project
            .camera
            .iter()
            .any(|k| k.frame == server.core.current));

        let (_out, is_error) =
            call_payload(&mut server, "centre_on_selection", json!({}));
        assert!(!is_error);
    }

    #[test]
    fn save_named_project_dispatch_writes_the_named_file() {
        let mut server = McpServer::new();
        server.core.project.name = "MCP Parity Save Test".to_string();
        let (out, is_error) = call_payload(&mut server, "save_named_project", json!({}));
        assert!(!is_error);
        assert_eq!(out["saved"], true);
        // The op's slug: lowercase, non-alphanumerics become '-'.
        let path = std::path::Path::new("./projects/mcp-parity-save-test.json");
        assert!(path.exists(), "named save wrote {}", path.display());
        std::fs::remove_file(path).expect("clean up the named-save test file");
    }

    #[test]
    fn audio_particle_fig3_tools_dispatch_edges() {
        let mut server = McpServer::new();
        // The demo project has no audio clips: every audio parity
        // tool refuses cleanly instead of panicking.
        for (name, args) in [
            ("audio_mixdown", json!({})),
            ("audio_split", json!({ "clip": 0, "at_frame": 1 })),
            ("audio_crossfade", json!({ "clip": 0 })),
        ] {
            let (_out, is_error) = call_payload(&mut server, name, args);
            assert!(is_error, "{name} refuses with no clips");
        }

        let (out, is_error) =
            call_payload(&mut server, "particle_add_event", json!({ "frame": 0, "x": 100.0, "y": 50.0 }));
        assert!(!is_error);
        assert_eq!(out["index"], 0);
        assert_eq!(server.core.project.particles.len(), 1);
        let (_out, is_error) = call_payload(&mut server, "clear_particles", json!({}));
        assert!(!is_error);
        assert!(server.core.project.particles.is_empty());

        let (out, is_error) = call_payload(&mut server, "fig3_library_list", json!({}));
        assert!(!is_error);
        assert!(out["figures"].is_array());
        let (_out, is_error) = call_payload(
            &mut server,
            "fig3_load_from_library",
            json!({ "name": "no-such-figure" }),
        );
        assert!(is_error, "loading an absent library figure refuses");
        let (_out, is_error) =
            call_payload(&mut server, "fig3_save_to_library", json!({}));
        assert!(is_error, "saving without a 3D session refuses");
    }

    #[test]
    fn handle_on_swap_frames_moves_the_passed_core_only() {
        let mut server = McpServer::new();
        let mut external = crate::editor::EditorCore::new();
        external.project.frames[0].duration = 2.0;
        external.project.frames[1].duration = 3.0;
        let own_before = server.core.project.frames[0].duration;
        let (_out, is_error) = call_payload_on(
            &mut server,
            &mut external,
            "swap_frames",
            json!({ "a": 0, "b": 1 }),
        );
        assert!(!is_error);
        assert_eq!(external.project.frames[0].duration, 3.0);
        assert_eq!(external.project.frames[1].duration, 2.0);
        assert_eq!(
            server.core.project.frames[0].duration, own_before,
            "the server's own core is untouched on the live path"
        );
    }

    #[test]
    fn path_and_project_toggle_tools_dispatch() {
        let mut server = McpServer::new();
        // Smoothing a valid figure succeeds (changed or already
        // smooth); an invalid one refuses.
        let (_out, is_error) =
            call_payload(&mut server, "smooth_path_roots", json!({ "figure": 0 }));
        assert!(!is_error);
        let (_out, is_error) =
            call_payload(&mut server, "smooth_path_roots", json!({ "figure": 99 }));
        assert!(is_error, "an out-of-range figure refuses");

        let (out, is_error) =
            call_payload(&mut server, "set_camera_follow", json!({ "figure": 0 }));
        assert!(!is_error);
        assert_eq!(out["follow"], 0);
        assert_eq!(server.core.project.camera_follow, Some(0));
        let (out, is_error) =
            call_payload(&mut server, "set_camera_follow", json!({ "figure": null }));
        assert!(!is_error);
        assert_eq!(out["follow"], Value::Null);
        assert_eq!(server.core.project.camera_follow, None);

        let (out, is_error) =
            call_payload(&mut server, "set_tween_playback", json!({ "enabled": true }));
        assert!(!is_error);
        assert_eq!(out["enabled"], true);
        assert!(server.core.project.tween_playback);
        let (out, is_error) =
            call_payload(&mut server, "set_tween_playback", json!({ "enabled": false }));
        assert!(!is_error);
        assert_eq!(out["enabled"], false);
        assert!(!server.core.project.tween_playback);
    }

    #[test]
    fn wave10_editor_status_reports_session_state_via_handle_on() {
        let mut server = McpServer::new();
        let mut external = crate::editor::EditorCore::new();
        let (status, is_error) =
            call_payload_on(&mut server, &mut external, "get_editor_status", json!({}));
        assert!(!is_error);
        assert_eq!(status["playing"], false);
        assert_eq!(status["current_frame"], 0);
        assert_eq!(status["loop_range"], Value::Null);
        assert_eq!(status["selected_figure"], 0);
        assert_eq!(status["selected_node"], Value::Null);
        assert_eq!(status["can_undo"], false);
        assert_eq!(status["can_redo"], false);
        assert_eq!(status["undo_depth"], 0);
        // An undoable edit through the same route shows up in
        // the reported history depths.
        let (_out, is_error) =
            call_payload_on(&mut server, &mut external, "duplicate_frame", json!({}));
        assert!(!is_error);
        let (status, is_error) =
            call_payload_on(&mut server, &mut external, "get_editor_status", json!({}));
        assert!(!is_error);
        assert_eq!(status["can_undo"], true);
        assert_eq!(status["undo_depth"], 1);
        assert_eq!(status["current_frame"], external.current);
        // The server's own core was not the edit target.
        assert_eq!(server.core.undo_depth(), 0);
    }

    #[test]
    fn wave10_set_playback_drives_the_passed_core() {
        let mut server = McpServer::new();
        let mut external = crate::editor::EditorCore::new();
        assert!(
            external.project.frames.len() >= 4,
            "demo project needs several frames"
        );
        let (out, is_error) = call_payload_on(
            &mut server,
            &mut external,
            "set_playback",
            json!({ "action": "play" }),
        );
        assert!(!is_error);
        assert_eq!(out["playing"], true);
        assert!(external.playing);
        assert!(!server.core.playing, "the server's own core stays paused");

        // Toggle and pause flip the same flag.
        let (out, is_error) = call_payload_on(
            &mut server,
            &mut external,
            "set_playback",
            json!({ "action": "toggle" }),
        );
        assert!(!is_error);
        assert_eq!(out["playing"], false);
        let (out, is_error) = call_payload_on(
            &mut server,
            &mut external,
            "set_playback",
            json!({ "action": "toggle" }),
        );
        assert!(!is_error);
        assert_eq!(out["playing"], true);
        let (out, is_error) = call_payload_on(
            &mut server,
            &mut external,
            "set_playback",
            json!({ "action": "pause" }),
        );
        assert!(!is_error);
        assert_eq!(out["playing"], false);

        // Restart from mid-timeline lands on frame 0, playing.
        let (_out, is_error) =
            call_payload_on(&mut server, &mut external, "set_frame", json!({ "frame": 3 }));
        assert!(!is_error);
        let (out, is_error) = call_payload_on(
            &mut server,
            &mut external,
            "set_playback",
            json!({ "action": "restart" }),
        );
        assert!(!is_error);
        assert_eq!(out["current_frame"], 0);
        assert_eq!(out["playing"], true);
        assert_eq!(external.current, 0);

        // Play range: no range falls back to frame 0; with a
        // range it starts at the range's start.
        let (_out, is_error) = call_payload_on(
            &mut server,
            &mut external,
            "set_playback",
            json!({ "action": "pause" }),
        );
        assert!(!is_error);
        let (_out, is_error) =
            call_payload_on(&mut server, &mut external, "set_frame", json!({ "frame": 2 }));
        assert!(!is_error);
        let (out, is_error) = call_payload_on(
            &mut server,
            &mut external,
            "set_playback",
            json!({ "action": "play_range" }),
        );
        assert!(!is_error);
        assert_eq!(out["current_frame"], 0, "no range: play_range is restart");
        let (_out, is_error) = call_payload_on(
            &mut server,
            &mut external,
            "set_loop_range",
            json!({ "start": 1, "end": 3 }),
        );
        assert!(!is_error);
        let (out, is_error) = call_payload_on(
            &mut server,
            &mut external,
            "set_playback",
            json!({ "action": "play_range" }),
        );
        assert!(!is_error);
        assert_eq!(out["current_frame"], 1, "the range's start");
        assert_eq!(out["playing"], true);

        // An unknown action is an error and changes nothing.
        let (_out, is_error) = call_payload_on(
            &mut server,
            &mut external,
            "set_playback",
            json!({ "action": "rewind" }),
        );
        assert!(is_error);
        assert!(external.playing, "a refused action leaves playback alone");
        assert_eq!(external.current, 1);
    }

    #[test]
    fn wave10_selection_tools_round_trip_via_handle_on() {
        let mut server = McpServer::new();
        let mut external = crate::editor::EditorCore::new();
        let (sel, is_error) =
            call_payload_on(&mut server, &mut external, "get_selection", json!({}));
        assert!(!is_error);
        assert_eq!(sel["selected_figure"], 0);
        assert_eq!(sel["selected_node"], Value::Null);
        assert_eq!(
            sel["figure_name"],
            external.project.figures[0].name.as_str()
        );

        // A second figure through the same route, then select
        // it together with its root node.
        let (_out, is_error) = call_payload_on(
            &mut server,
            &mut external,
            "add_figure",
            json!({ "kind": "mini" }),
        );
        assert!(!is_error);
        let (out, is_error) = call_payload_on(
            &mut server,
            &mut external,
            "set_selection",
            json!({ "figure": 1, "node": 0 }),
        );
        assert!(!is_error);
        assert_eq!(out["selected_figure"], 1);
        assert_eq!(out["selected_node"], 0);
        assert_eq!(external.selected_figure, 1);
        assert_eq!(external.selected_node, Some(0));

        // Selecting a figure without a node clears the node
        // (select_figure's contract).
        let (out, is_error) = call_payload_on(
            &mut server,
            &mut external,
            "set_selection",
            json!({ "figure": 0 }),
        );
        assert!(!is_error);
        assert_eq!(out["selected_node"], Value::Null);
        assert_eq!(external.selected_node, None);

        // Out-of-range figure and node are errors and leave
        // the selection untouched.
        let (_out, is_error) = call_payload_on(
            &mut server,
            &mut external,
            "set_selection",
            json!({ "figure": 99 }),
        );
        assert!(is_error);
        let (_out, is_error) = call_payload_on(
            &mut server,
            &mut external,
            "set_selection",
            json!({ "figure": 0, "node": 9999 }),
        );
        assert!(is_error);
        assert_eq!(external.selected_figure, 0);
        assert_eq!(external.selected_node, None);
    }

    #[test]
    fn wave10_transport_key_jumps_walk_camera_and_graph_keys() {
        let mut server = McpServer::new();
        let mut external = crate::editor::EditorCore::new();
        // Precondition: the demo project starts with no camera
        // or graph keys, so both jumps have nowhere to go.
        assert!(
            external.project.camera.is_empty()
                && external
                    .project
                    .channels
                    .iter()
                    .all(|c| c.keys.is_empty()),
            "the demo project must start keyless for this test"
        );
        let (_out, is_error) =
            call_payload_on(&mut server, &mut external, "goto_next_key", json!({}));
        assert!(is_error, "no keys: next refuses");
        let (_out, is_error) =
            call_payload_on(&mut server, &mut external, "goto_prev_key", json!({}));
        assert!(is_error, "no keys: prev refuses");
        assert_eq!(external.current, 0, "refused jumps do not move");

        // A camera key at frame 3 and a graph key at frame 1,
        // both planted through the registry tools on the same
        // external core.
        let (_out, is_error) = call_payload_on(
            &mut server,
            &mut external,
            "set_camera",
            json!({ "frame": 3, "x": 400.0, "y": 300.0, "zoom": 1.0 }),
        );
        assert!(!is_error);
        let (_out, is_error) = call_payload_on(
            &mut server,
            &mut external,
            "graph_key",
            json!({ "figure": 0, "target": "root_x", "frame": 1.0, "value": 120.0 }),
        );
        assert!(!is_error);

        // From frame 0 the walk goes 1 (graph) then 3 (camera).
        let (out, is_error) =
            call_payload_on(&mut server, &mut external, "goto_next_key", json!({}));
        assert!(!is_error);
        assert_eq!(out["current_frame"], 1);
        assert_eq!(out["key_frames"], json!([1, 3]));
        let (out, is_error) =
            call_payload_on(&mut server, &mut external, "goto_next_key", json!({}));
        assert!(!is_error);
        assert_eq!(out["current_frame"], 3);
        // Standing on the last key, next refuses (strictly
        // after), and prev walks back to 1.
        let (_out, is_error) =
            call_payload_on(&mut server, &mut external, "goto_next_key", json!({}));
        assert!(is_error);
        assert_eq!(external.current, 3, "a refused jump does not move");
        let (out, is_error) =
            call_payload_on(&mut server, &mut external, "goto_prev_key", json!({}));
        assert!(!is_error);
        assert_eq!(out["current_frame"], 1);
        assert_eq!(external.current, 1);
        // The jumps never touch the playing flag.
        assert!(!external.playing);
    }

    #[test]
    fn wave10_figure_keyframe_tools_drive_the_editor_ops() {
        let mut server = McpServer::new();
        let mut external = crate::editor::EditorCore::new();
        // Fixture: rewrite figure 0's five demo poses as
        // A A B B C, so its keyframes are exactly [0, 2, 4]
        // (frame 0 holds a pose; frames 2 and 4 are where
        // the pose changes).
        assert_eq!(
            external.project.frames.len(),
            5,
            "the demo project has five frames"
        );
        assert!(
            external.project.frames.iter().all(|f| !f.poses.is_empty()),
            "the demo figure poses on every frame"
        );
        let a = external.project.frames[0].poses[0].clone();
        let mut b = a.clone();
        b.root[0] += 40.0;
        let mut c = a.clone();
        c.root[0] += 80.0;
        let staged = [a.clone(), a, b.clone(), b, c];
        for (frame, pose) in external.project.frames.iter_mut().zip(staged) {
            frame.poses[0] = pose;
        }
        assert_eq!(
            crate::stats::figure_keyframes(&external.project, 0),
            vec![0, 2, 4]
        );

        // Next walks 0 -> 2 -> 4, then refuses without moving.
        let (out, is_error) = call_payload_on(
            &mut server,
            &mut external,
            "goto_next_keyframe",
            json!({ "figure": 0 }),
        );
        assert!(!is_error);
        assert_eq!(out["current_frame"], 2);
        assert_eq!(out["key_frames"], json!([0, 2, 4]));
        let (out, is_error) = call_payload_on(
            &mut server,
            &mut external,
            "goto_next_keyframe",
            json!({ "figure": 0 }),
        );
        assert!(!is_error);
        assert_eq!(out["current_frame"], 4);
        let (_out, is_error) = call_payload_on(
            &mut server,
            &mut external,
            "goto_next_keyframe",
            json!({ "figure": 0 }),
        );
        assert!(is_error, "no later keyframe: the core refuses");
        assert_eq!(external.current, 4, "a refused jump does not move");

        // Prev walks back to 2.
        let (out, is_error) = call_payload_on(
            &mut server,
            &mut external,
            "goto_prev_keyframe",
            json!({ "figure": 0 }),
        );
        assert!(!is_error);
        assert_eq!(out["current_frame"], 2);

        // Deleting the keyframe at frame 2 holds frame 1's
        // pose there, so the change point moves to frame 3:
        // the keys become [0, 3, 4], at one undo step.
        let depth_before = external.undo_depth();
        let (out, is_error) = call_payload_on(
            &mut server,
            &mut external,
            "delete_keyframe",
            json!({ "figure": 0, "frame": 2 }),
        );
        assert!(!is_error);
        assert_eq!(out["key_frames"], json!([0, 3, 4]));
        assert_eq!(
            crate::stats::figure_keyframes(&external.project, 0),
            vec![0, 3, 4]
        );
        assert_eq!(external.undo_depth(), depth_before + 1, "one undo step");

        // The core's refusals surface as errors: the base
        // pose at frame 0, and frame 1, which is no keyframe.
        let (_out, is_error) = call_payload_on(
            &mut server,
            &mut external,
            "delete_keyframe",
            json!({ "figure": 0, "frame": 0 }),
        );
        assert!(is_error, "frame 0 is the base pose");
        let (_out, is_error) = call_payload_on(
            &mut server,
            &mut external,
            "delete_keyframe",
            json!({ "figure": 0, "frame": 1 }),
        );
        assert!(is_error, "frame 1 holds no keyframe for figure 0");

        // An out-of-range figure refuses across the family.
        let (_out, is_error) = call_payload_on(
            &mut server,
            &mut external,
            "goto_next_keyframe",
            json!({ "figure": 99 }),
        );
        assert!(is_error);
        let (_out, is_error) = call_payload_on(
            &mut server,
            &mut external,
            "delete_keyframe",
            json!({ "figure": 99, "frame": 2 }),
        );
        assert!(is_error);
    }
}
