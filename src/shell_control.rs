//! Wave 14 — shell control: the channel that lets the
//! live MCP endpoint drive the GUI shell itself.
//!
//! The MCP registry (`stykanimate_rs::mcp_tools`) can only
//! hold handlers of one shape — `fn(&mut EditorCore,
//! &Value)` — because both transports share it. But part
//! of what a user does in the editor does not live in the
//! `EditorCore` at all: the active workspace, the view
//! layer's zoom/pan, the panels' selections, the theme
//! and settings, which project file the menus would open.
//! That state belongs to the shell (`crate::State` and its
//! panel states), which only the running GUI has.
//!
//! The shell tools — every name in [`SHELL_TOOL_NAMES`] —
//! are therefore registered in the shared registry with a
//! stub handler that refuses honestly ("needs the live
//! editor"), so `tools/list` is identical on both
//! transports and the stdio server never pretends it can
//! move a window it does not have. On the live endpoint
//! the call never reaches the stub: [`crate::live_mcp`]'s
//! drain peeks at each request (see [`peek`]) and hands
//! shell calls to this module's queue instead of the
//! router. The GUI's `poll` drains the queue (see
//! [`drain`]) on the UI thread, immediately after the core
//! drain, dispatching by name to the sibling handler
//! modules (`shell_tools_state`, `shell_tools_view`,
//! `shell_tools_selection`, `shell_tools_settings`,
//! `shell_tools_layout`, `shell_tools_file`) — each of
//! which performs exactly the writes the corresponding
//! GUI control performs.
//!
//! Design notes, for the next auditor:
//!
//! * **Granular names, not consolidated commands.** The
//!   design draft (docs/wave14/SHELL_CONTROL.md) proposed
//!   a handful of consolidated tools (`shell_transport`,
//!   `shell_select`, …) translated into a command enum.
//!   The squad's handler modules landed first with one
//!   tool per GUI control, each click-evidenced — finer
//!   parity, and the names clients see are the controls'
//!   own. This module therefore dispatches by NAME; there
//!   is no command enum. Transport needed no queue at all:
//!   playback state is `EditorCore` state, so
//!   `shell_play` / `shell_pause` / `shell_set_playhead` /
//!   `shell_get_transport` are ordinary registry tools
//!   with real handlers (`shell_tools_transport`, in the
//!   library crate) that also serve on stdio. There is no
//!   Stop tool: the GUI has no Stop control, and none was
//!   invented.
//! * **The reply slot travels with the request.** A
//!   queued [`ShellRequest`] carries the originating
//!   `LiveRequest`'s own reply channel and cancellation
//!   flag — the bridge already built exactly one result
//!   slot per request, and a second slot type would need
//!   a second waiter. The HTTP listener is blocked on
//!   that channel; sending there is what answers the
//!   client, with the same one-frame-of-poll latency a
//!   core call has.
//! * **Cancellation is checked twice.** The core drain
//!   discards cancelled requests before routing; a shell
//!   request is checked again here before executing, so
//!   a request whose client already received a 504 (or
//!   whose bridge was switched off mid-wait) never
//!   executes late against a different project.
//! * **No undo steps, no dirty marks.** Shell actions
//!   are session state — the GUI's own clicks on these
//!   controls take no undo step either (undo stores the
//!   `Project` only). The handlers say so in their own
//!   docs; this module adds nothing to that contract.
//! * **The queue is bounded** at [`MAX_SHELL_QUEUED`],
//!   the bridge's own bound: at most one drain batch of
//!   core calls can sit ahead of a shell call, and a full
//!   queue is answered with a refusal on the request's
//!   own channel — never a silent drop, never a block.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::live_mcp::LiveRequest;
use crate::State;

/// The live-only shell tools: the names the live drain
/// intercepts and this module dispatches. Every name here
/// MUST also be registered in `mcp_tools::tools()` with
/// the shared `shell_unavailable` stub handler (so the
/// stdio list matches and stdio calls refuse honestly),
/// and MUST have a dispatch arm in [`dispatch`]. The
/// gate's MCP census counts the registry side; the
/// shell chapter of docs/MCP.md lists the same names.
pub const SHELL_TOOL_NAMES: &[&str] = &[
    // shell_tools_state — workspace + shell snapshot.
    "shell_set_workspace",
    "shell_get_state",
    "shell_set_onion_depths",
    // shell_tools_view — the view layer (never the camera).
    "shell_view_zoom",
    "shell_view_fit",
    "shell_view_centre",
    "shell_view_reset",
    "shell_get_view",
    // shell_tools_selection — the panels' selections.
    "shell_select_figure",
    "shell_select_node",
    "shell_select_sprite",
    "shell_select_fx_clip",
    "shell_select_audio_clip",
    "shell_select_frame",
    "shell_select_stroke",
    "shell_select_particle",
    "shell_select_clip",
    "shell_select_graph_channel",
    "shell_get_selection",
    // shell_tools_settings — theme + the settings card.
    "shell_set_theme",
    "shell_get_settings",
    "shell_set_setting",
    // shell_tools_layout — sections + columns.
    "shell_set_section",
    "shell_set_column_widths",
    "shell_get_layout",
    "shell_set_timeline",
    // shell_tools_style — the style inspector's shell state.
    "shell_get_recent_colours",
    // shell_tools_file — menus, recents, project files.
    "shell_list_recents",
    "shell_open_recent",
    "shell_reload_project",
    "shell_recover_snapshot",
    "shell_duplicate_project_file",
    "shell_delete_project_file",
    "shell_set_sidebar",
];

/// Is `name` one of the live-only shell tools?
pub fn is_shell_tool(name: &str) -> bool {
    SHELL_TOOL_NAMES.contains(&name)
}

/// The shell queue's bound — the bridge's own
/// (`live_mcp`'s request queue also holds 8), so the two
/// drains have the same worst case per poll tick.
pub const MAX_SHELL_QUEUED: usize = 8;

/// One queued shell call, with everything its answer
/// needs. The `reply` channel is **the result slot**:
/// the originating [`LiveRequest`]'s own one-shot reply
/// channel, which the HTTP listener thread is blocked
/// on right now — sending the response there is what
/// answers the request.
pub struct ShellRequest {
    /// The JSON-RPC id, cloned from the message at peek
    /// time; echoed in the response.
    pub id: Value,
    /// The tool name (a member of [`SHELL_TOOL_NAMES`]).
    pub name: String,
    /// The call's `arguments` object (`{}` when absent).
    pub args: Value,
    /// The result slot (see the struct docs).
    pub reply: SyncSender<Option<Value>>,
    /// The originating request's cancellation flag,
    /// shared by reference: checked again before
    /// executing, exactly as `live_mcp`'s
    /// `discard_if_cancelled` checks it.
    pub cancelled: Arc<AtomicBool>,
}

/// The shell command queue: the live drain pushes
/// (through [`enqueue`], via [`peek`]), `poll` pops
/// (through [`drain`]). A plain `Mutex` + `VecDeque` —
/// the `BRIDGE` / `DRAIN_ERRORS` statics pattern —
/// because every critical section is a push or a pop,
/// and no producer ever needs to block: the listener's
/// blocking happens on the reply channel it already
/// holds.
static SHELL_QUEUE: Mutex<VecDeque<ShellRequest>> = Mutex::new(VecDeque::new());

/// Lock the queue, recovering a poisoned lock the house
/// way (`unwrap_or_else(|p| p.into_inner())`, as
/// `live_mcp`'s statics do): a panicked producer must
/// not wedge every later command.
fn lock_queue() -> std::sync::MutexGuard<'static, VecDeque<ShellRequest>> {
    SHELL_QUEUE.lock().unwrap_or_else(|p| p.into_inner())
}

/// Queue one shell request. Refuses when the queue is
/// full — and hands the request BACK with the refusal,
/// because the request's reply channel is a live HTTP
/// answer its caller still has to send on (the drain
/// answers a queue-full refusal itself, through
/// [`respond`], rather than the channel being dropped
/// into a client-side disconnect).
pub fn enqueue(req: ShellRequest) -> Result<(), (ShellRequest, String)> {
    let mut queue = lock_queue();
    if queue.len() >= MAX_SHELL_QUEUED {
        return Err((
            req,
            format!(
                "shell command queue is full ({MAX_SHELL_QUEUED} queued); \
                 the editor is not draining — try again in a moment"
            ),
        ));
    }
    queue.push_back(req);
    Ok(())
}

/// How many shell calls sit queued right now.
pub fn queued_shell_commands() -> usize {
    lock_queue().len()
}

/// Pop up to `max` queued requests, oldest first. The
/// drain's pop step, factored out so the queue's FIFO
/// order and bound are testable without a `State`
/// (constructing one needs the whole shell `init`).
fn take_queued(max: usize) -> Vec<ShellRequest> {
    let mut queue = lock_queue();
    let n = max.min(queue.len());
    queue.drain(..n).collect()
}

/// Peek at one live request in the core drain: if it is
/// a `tools/call` for a shell tool, take it over as a
/// [`ShellRequest`] (the caller enqueues it and skips
/// the router); otherwise return `None` and the request
/// routes normally.
///
/// The test is deliberately shallow — object, method,
/// an `id` member, a registered shell name. Argument
/// validation belongs to the handlers at execute time
/// (they own the refusal wording, and the counts they
/// validate against only exist on the live core). Two
/// shapes stay with the router on purpose:
///
/// * a **notification** (no `id` member) — it must never
///   execute anything (the house rule, Wave 14 D6), so
///   it is never queued;
/// * an **unregistered `shell_*` name** — a typo'd shell
///   tool must earn the router's genuine `-32602`
///   "unknown tool" protocol error, not a queue slot.
pub fn peek(req: &LiveRequest) -> Option<ShellRequest> {
    let msg = &req.msg;
    if msg.get("method").and_then(Value::as_str) != Some("tools/call") {
        return None;
    }
    let id = msg.get("id")?.clone();
    let params = msg.get("params")?;
    let name = params.get("name").and_then(Value::as_str)?;
    if !is_shell_tool(name) {
        return None;
    }
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    Some(ShellRequest {
        id,
        name: name.to_string(),
        args,
        reply: req.reply.clone(),
        cancelled: Arc::clone(&req.cancelled),
    })
}

/// Build the JSON-RPC response for one shell outcome:
/// byte-identical to a registry tool's, because it IS
/// the registry's builder — `mcp::tool_result`, public
/// for exactly this call. Success and refusal are both
/// `result` objects with one text content block;
/// `isError` distinguishes them.
pub fn respond(id: Value, outcome: Result<Value, String>) -> Value {
    stykanimate_rs::mcp::tool_result(id, outcome)
}

/// Execute every queued shell call against the live
/// shell and answer each through its result slot.
/// Called by `main.rs`'s `poll` immediately after the
/// live core drain, on the same tick and thread, so a
/// shell call answers with the same latency as a core
/// call.
///
/// Per request: a cancelled request is skipped — never
/// executed — and answered with the defensive `None`
/// (its listener has already left the wait loop; that
/// is why it was cancelled). Any other request runs
/// under a `catch_unwind` net: a panic becomes an
/// `isError` outcome naming the tool, and the editor
/// survives. After the batch, any deferred bridge
/// action a settings call recorded (switching Live MCP
/// itself off or on) is applied — replies are already
/// sent by then, which is the ordering that makes
/// switching the endpoint off from the endpoint safe
/// (see `shell_tools_settings::apply_pending_bridge_actions`).
///
/// Returns how many calls actually executed (cancelled
/// skips do not count), so `poll` can treat a productive
/// shell drain like a productive core drain for redraw
/// purposes.
pub fn drain(state: &mut State) -> usize {
    // The queue is bounded at MAX_SHELL_QUEUED, so
    // draining it whole is draining at most that many.
    let batch = take_queued(usize::MAX);
    let mut ran = 0;
    for req in batch {
        if req.cancelled.load(Ordering::SeqCst) {
            let _ = req.reply.send(None);
            continue;
        }
        let name = req.name.clone();
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            dispatch(state, &req.name, &req.args)
        }));
        let outcome = match outcome {
            Ok(r) => r,
            Err(_) => Err(format!("tool panicked: {name}")),
        };
        let _ = req.reply.send(Some(respond(req.id, outcome)));
        ran += 1;
    }
    if ran > 0 {
        crate::shell_tools_settings::apply_pending_bridge_actions(state);
    }
    ran
}

/// Dispatch one shell call by name to its handler
/// module. Every arm performs exactly the writes the
/// corresponding GUI control performs; the handlers own
/// argument validation and refusal wording.
fn dispatch(state: &mut State, name: &str, args: &Value) -> Result<Value, String> {
    use crate::shell_tools_selection as sel;
    match name {
        // -- shell_tools_state --------------------------------
        "shell_set_workspace" => crate::shell_tools_state::tool_shell_set_workspace(state, args),
        "shell_get_state" => crate::shell_tools_state::tool_shell_get_state(state, args),
        "shell_set_onion_depths" => {
            crate::shell_tools_state::tool_shell_set_onion_depths(state, args)
        }
        // -- shell_tools_view (the view layer only) -----------
        n if crate::shell_tools_view::VIEW_TOOL_NAMES.contains(&n) => {
            crate::shell_tools_view::dispatch_view_tool(n, &mut state.core, args)
                .unwrap_or_else(|| Err(format!("unknown view tool: {n}")))
        }
        // -- shell_tools_selection ----------------------------
        "shell_select_figure" => sel::tool_shell_select_figure(&mut state.core, args),
        "shell_select_node" => sel::tool_shell_select_node(&mut state.core, args),
        "shell_select_clip" => sel::tool_shell_select_clip(&mut state.core, args),
        "shell_select_sprite"
        | "shell_select_fx_clip"
        | "shell_select_audio_clip"
        | "shell_select_frame"
        | "shell_select_stroke"
        | "shell_select_particle"
        | "shell_select_graph_channel" => {
            let mut targets = sel::ShellSelectionTargets {
                sprite_selected: &mut state.sprites_panel.selected,
                fx_selected: &mut state.fx_selected,
                audio_selected: &mut state.audio_panel.selected,
                frame_range_sel: &mut state.frame_ops.range_sel,
                stroke_selected: &mut state.stroke_selected,
                particle_selected: &mut state.particle_selected,
                graph_panel: &mut state.graph_panel,
            };
            match name {
                "shell_select_sprite" => {
                    sel::tool_shell_select_sprite(&mut state.core, &mut targets, args)
                }
                "shell_select_fx_clip" => {
                    sel::tool_shell_select_fx_clip(&mut state.core, &mut targets, args)
                }
                "shell_select_audio_clip" => {
                    sel::tool_shell_select_audio_clip(&mut state.core, &mut targets, args)
                }
                "shell_select_frame" => {
                    sel::tool_shell_select_frame(&mut state.core, &mut targets, args)
                }
                "shell_select_stroke" => {
                    sel::tool_shell_select_stroke(&mut state.core, &mut targets, args)
                }
                "shell_select_graph_channel" => {
                    sel::tool_shell_select_graph_channel(&mut state.core, &mut targets, args)
                }
                _ => sel::tool_shell_select_particle(&mut state.core, &mut targets, args),
            }
        }
        "shell_get_selection" => {
            let view = sel::ShellSelectionView {
                sprite_selected: &state.sprites_panel.selected,
                fx_selected: &state.fx_selected,
                audio_selected: &state.audio_panel.selected,
                frame_range_sel: &state.frame_ops.range_sel,
                stroke_selected: &state.stroke_selected,
                particle_selected: &state.particle_selected,
                graph_panel: &state.graph_panel,
            };
            sel::tool_shell_get_selection(&state.core, &view, args)
        }
        // -- shell_tools_settings -----------------------------
        "shell_set_theme" => crate::shell_tools_settings::exec_shell_set_theme(state, args),
        "shell_get_settings" => crate::shell_tools_settings::exec_shell_get_settings(state),
        "shell_set_setting" => crate::shell_tools_settings::exec_shell_set_setting(state, args),
        // -- shell_tools_layout -------------------------------
        "shell_set_section" => crate::shell_tools_layout::tool_shell_set_section(state, args),
        "shell_set_column_widths" => {
            crate::shell_tools_layout::tool_shell_set_column_widths(state, args)
        }
        "shell_get_layout" => crate::shell_tools_layout::tool_shell_get_layout(state, args),
        "shell_set_timeline" => crate::shell_tools_layout::tool_shell_set_timeline(state, args),
        // -- shell_tools_style --------------------------------
        "shell_get_recent_colours" => {
            crate::shell_tools_style::tool_shell_get_recent_colours(state, args)
        }
        // -- shell_tools_file ---------------------------------
        "shell_list_recents" => crate::shell_tools_file::tool_shell_list_recents(state, args),
        "shell_open_recent" => crate::shell_tools_file::tool_shell_open_recent(state, args),
        "shell_reload_project" => crate::shell_tools_file::tool_shell_reload_project(state, args),
        "shell_recover_snapshot" => {
            crate::shell_tools_file::tool_shell_recover_snapshot(state, args)
        }
        "shell_duplicate_project_file" => {
            crate::shell_tools_file::tool_shell_duplicate_project_file(state, args)
        }
        "shell_delete_project_file" => {
            crate::shell_tools_file::tool_shell_delete_project_file(state, args)
        }
        "shell_set_sidebar" => crate::shell_tools_file::tool_shell_set_sidebar(state, args),
        // is_shell_tool gates peek, so this arm is only a
        // defence against the name list and this table
        // drifting apart.
        _ => Err(format!("unknown tool: {name}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::mpsc::sync_channel;

    fn live_req(msg: Value) -> LiveRequest {
        let (reply, _rx) = sync_channel(1);
        LiveRequest {
            msg,
            reply,
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    fn call_msg(name: &str, args: Value) -> Value {
        json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "tools/call",
            "params": { "name": name, "arguments": args },
        })
    }

    #[test]
    fn peek_takes_over_registered_shell_calls() {
        let req = live_req(call_msg("shell_set_workspace", json!({ "workspace": "Draw" })));
        let shell = peek(&req).expect("a registered shell call is taken over");
        assert_eq!(shell.name, "shell_set_workspace");
        assert_eq!(shell.id, json!(7));
        assert_eq!(shell.args, json!({ "workspace": "Draw" }));
    }

    #[test]
    fn peek_leaves_core_calls_and_unknown_shell_names_with_the_router() {
        // A core tool routes normally.
        assert!(peek(&live_req(call_msg("get_state", json!({})))).is_none());
        // An unregistered shell_* name routes normally too,
        // so the router answers its genuine -32602.
        assert!(peek(&live_req(call_msg("shell_frobnicate", json!({})))).is_none());
        assert!(!is_shell_tool("shell_frobnicate"));
        // A non-call method is not a shell takeover.
        let list = live_req(json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }));
        assert!(peek(&list).is_none());
    }

    #[test]
    fn peek_never_takes_a_notification() {
        // No `id` member: a notification must execute
        // nothing (house rule), so it is never queued —
        // even when it names a shell tool.
        let note = live_req(json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": { "name": "shell_set_workspace", "arguments": { "workspace": "Draw" } },
        }));
        assert!(peek(&note).is_none());
        // An explicit null id IS a request (the member is
        // present) and is taken over like any other.
        let null_id = live_req(json!({
            "jsonrpc": "2.0",
            "id": null,
            "method": "tools/call",
            "params": { "name": "shell_set_workspace", "arguments": { "workspace": "Draw" } },
        }));
        assert!(peek(&null_id).is_some());
    }

    #[test]
    fn every_view_tool_name_is_a_shell_tool() {
        for name in crate::shell_tools_view::VIEW_TOOL_NAMES {
            assert!(is_shell_tool(name), "{name} must route to the shell queue");
        }
    }

    #[test]
    fn queue_is_fifo_and_bounded() {
        // Drain whatever earlier tests left, then prove
        // order + bound on a clean queue.
        let _ = take_queued(usize::MAX);
        let mk = |name: &str| {
            let (reply, _rx) = sync_channel(1);
            ShellRequest {
                id: json!(1),
                name: name.to_string(),
                args: json!({}),
                reply,
                cancelled: Arc::new(AtomicBool::new(false)),
            }
        };
        for i in 0..MAX_SHELL_QUEUED {
            assert!(enqueue(mk(&format!("shell_t{i}"))).is_ok());
        }
        let (back, why) = enqueue(mk("shell_overflow")).unwrap_err();
        assert_eq!(back.name, "shell_overflow", "the refused request is handed back");
        assert!(why.contains("queue is full"), "why: {why}");
        let names: Vec<String> = take_queued(usize::MAX)
            .into_iter()
            .map(|r| r.name)
            .collect();
        assert_eq!(names.len(), MAX_SHELL_QUEUED);
        assert_eq!(names[0], "shell_t0", "oldest first");
        assert_eq!(names[MAX_SHELL_QUEUED - 1], "shell_t7");
    }

    #[test]
    fn respond_uses_the_registry_tool_result_shape() {
        let ok = respond(json!(3), Ok(json!({ "workspace": "Draw" })));
        assert_eq!(ok["id"], json!(3));
        assert!(ok.get("error").is_none());
        assert_eq!(ok["result"]["isError"], json!(false));
        let err = respond(json!(4), Err("no such workspace".to_string()));
        assert_eq!(err["result"]["isError"], json!(true));
        assert!(
            err["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("no such workspace")
        );
    }
}
