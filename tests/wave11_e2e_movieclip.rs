//! Integration tests: the movieclip lifecycle driven END TO END
//! through the real MCP dispatch — Wave 11, squad D, session 7
//! ("movieclips, MCP only").
//!
//! The client here is the actual `mcp_server` binary
//! (`src/bin/mcp_server.rs`), spawned as a subprocess and spoken
//! to in newline-delimited JSON-RPC 2.0 over its stdin/stdout —
//! byte for byte what an external MCP client sends. No editor
//! API is touched: if a step cannot be done with `tools/call`
//! messages, it cannot be done in this file, which is exactly
//! the property under test. (It is also the only route available
//! to `tests/`: the crate's integration tests link no
//! dev-dependencies, so there is no `serde_json` here — the wire
//! text is the interface, parsed with small substring helpers.)
//!
//! The surface, verified at source (src/mcp.rs `all_tools` =
//! built-ins + src/mcp_tools.rs registry + src/mcp_io.rs):
//! entering Wave 11 the whole movieclip vocabulary was three
//! tools — `clip_create` (build a clip from a figure's poses
//! across ALL frames; there is no frame-range variant),
//! `clip_list` and `set_clip_loop` — plus the generic
//! `undo`/`redo`, and the editor core's full clip edit session
//! (`enter_clip`/`exit_clip`, clip frame ops, the Wave 10
//! name-carrying `ClipEditSession`) was reachable through NONE
//! of it. The Wave 11 splice landed the rest: `clip_delete`,
//! `clip_rename`, `clip_duplicate`, `clip_move`,
//! `clip_insert_figure`, and the session family (`clip_enter`,
//! `clip_exit`, `clip_goto`, `clip_add_frame`,
//! `clip_delete_frame`, `clip_set_angle`, `clip_set_root`,
//! `clip_get_pose`). The tests below began as `wave11_gap_*`
//! absence probes written against the correct behaviour; they
//! pass unchanged in substance now that the tools exist (the
//! candidate-name probes find the landed names).

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};

// ---------------------------------------------------------------------
// The stdio MCP client (std only)
// ---------------------------------------------------------------------

/// One spawned `mcp_server` process spoken to in request/response
/// lock-step: the server answers every request with exactly one
/// line and flushes, so a blocking `read_line` per request is the
/// whole synchronisation story.
struct McpClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
    next_id: u64,
}

/// A `tools/call` outcome, pre-digested from the raw wire line:
/// the unescaped text payload of the first content block, and the
/// two failure channels kept distinct — a tool that ran and
/// failed (`isError: true`, message in `text`) versus a protocol
/// error (`error` object, e.g. -32602 "unknown tool"), which is
/// how a MISSING tool announces itself.
struct CallOutcome {
    raw: String,
    text: String,
    is_error: bool,
    protocol_error: Option<String>,
}

impl McpClient {
    fn spawn() -> McpClient {
        let mut child = Command::new(env!("CARGO_BIN_EXE_mcp_server"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn the mcp_server binary");
        let stdin = child.stdin.take().expect("server stdin");
        let stdout = BufReader::new(child.stdout.take().expect("server stdout"));
        McpClient {
            child,
            stdin,
            stdout,
            next_id: 0,
        }
    }

    /// Send one JSON-RPC request line, read its one response line.
    fn request(&mut self, body: &str) -> String {
        self.next_id += 1;
        let line = format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":{},\"method\":{body}}}",
            self.next_id
        );
        self.stdin
            .write_all(line.as_bytes())
            .and_then(|()| self.stdin.write_all(b"\n"))
            .and_then(|()| self.stdin.flush())
            .expect("write request to the server");
        let mut response = String::new();
        let n = self
            .stdout
            .read_line(&mut response)
            .expect("read response from the server");
        assert!(
            n > 0,
            "the server closed its stdout without answering request {line}"
        );
        response
    }

    /// A raw method call (initialize / tools/list).
    fn method(&mut self, method: &str) -> String {
        self.request(&format!("\"{method}\""))
    }

    /// One `tools/call` for `tool` with a hand-written JSON
    /// arguments object (all argument sets in this file are plain
    /// ASCII with no characters needing escapes).
    fn call(&mut self, tool: &str, arguments: &str) -> CallOutcome {
        let raw = self.request(&format!(
            "\"tools/call\",\"params\":{{\"name\":\"{tool}\",\"arguments\":{arguments}}}"
        ));
        let protocol_error = extract_after(&raw, "\"message\":\"")
            .filter(|_| raw.contains("\"error\":{"))
            .map(|m| m.to_string());
        CallOutcome {
            text: extract_text_payload(&raw).unwrap_or_default(),
            is_error: raw.contains("\"isError\":true"),
            protocol_error,
            raw,
        }
    }

    /// The registered tool names, from a live `tools/list`.
    fn tool_names(&mut self) -> Vec<String> {
        let raw = self.method("tools/list");
        let mut names = Vec::new();
        let mut rest = raw.as_str();
        while let Some(pos) = rest.find("\"name\":\"") {
            let tail = &rest[pos + "\"name\":\"".len()..];
            let end = tail.find('"').expect("tool name terminates");
            names.push(tail[..end].to_string());
            rest = &tail[end..];
        }
        names
    }

    /// Pick the first candidate name that is actually registered
    /// — the gap tests probe for capabilities, not for one exact
    /// spelling, so whichever name a future tool lands under is
    /// the one they exercise.
    fn pick_tool(&mut self, candidates: &[&str]) -> Option<String> {
        let names = self.tool_names();
        candidates
            .iter()
            .find(|c| names.iter().any(|n| n.as_str() == **c))
            .map(|c| c.to_string())
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        // Closing stdin is the server's EOF: it exits on its own.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The unescaped string value following `marker` in a JSON text
/// (handles the `\"` and `\\` escapes serde_json emits).
fn extract_after<'a>(text: &'a str, marker: &str) -> Option<&'a str> {
    let start = text.find(marker)? + marker.len();
    let tail = &text[start..];
    let mut end = tail.len();
    let mut escaped = false;
    for (i, ch) in tail.char_indices() {
        if escaped {
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            end = i;
            break;
        }
    }
    Some(&tail[..end])
}

/// The `content[0].text` payload of a tools/call response,
/// unescaped back into the handler's own JSON (or error string).
fn extract_text_payload(raw: &str) -> Option<String> {
    let escaped = extract_after(raw, "\"text\":\"")?;
    let mut out = String::with_capacity(escaped.len());
    let mut chars = escaped.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    Some(out)
}

/// The integer value of `"key":N` in a JSON text, if present.
fn json_num(text: &str, key: &str) -> Option<i64> {
    let marker = format!("\"{key}\":");
    let start = text.find(&marker)? + marker.len();
    let tail = &text[start..];
    let digits: String = tail
        .chars()
        .skip_while(|c| *c == ' ')
        .take_while(|c| c.is_ascii_digit() || *c == '-')
        .collect();
    digits.parse().ok()
}

/// A clip_list call digested to (count, first clip's text) — the
/// session's clip oracle, since it is the only clip readback
/// MCP has.
fn clip_list(client: &mut McpClient) -> (i64, String) {
    let out = client.call("clip_list", "{}");
    assert!(
        !out.is_error && out.protocol_error.is_none(),
        "clip_list itself failed: {}",
        out.raw
    );
    let count = json_num(&out.text, "count").expect("clip_list reports a count");
    (count, out.text)
}

/// The opening moves every session below shares: handshake,
/// then a fresh named project via MCP (1 figure "Styk", 1 frame).
fn fresh_project(client: &mut McpClient) {
    let init = client.method("initialize");
    assert!(
        init.contains("\"protocolVersion\":\"2024-11-05\"")
            && init.contains("\"name\":\"stykanimate-rs\""),
        "initialize did not identify the server: {init}"
    );
    let out = client.call("new_project", "{\"name\":\"Wave11 Clip Session\"}");
    assert!(!out.is_error, "new_project failed: {}", out.text);
    assert_eq!(
        json_num(&out.text, "frames"),
        Some(1),
        "a fresh MCP project starts on one frame: {}",
        out.text
    );
}

/// Build the session's one clip: grow the fresh project to four
/// frames, differentiate frame 2's pose, then `clip_create` from
/// figure 0 — the only creation path MCP has (the figure's poses
/// across ALL frames; a frame RANGE cannot be selected).
/// Returns the clip_create payload.
fn build_clip(client: &mut McpClient) -> String {
    fresh_project(client);
    for want in [2, 3, 4] {
        let out = client.call("add_frame", "{}");
        assert!(!out.is_error, "add_frame failed: {}", out.text);
        assert_eq!(
            json_num(&out.text, "frames"),
            Some(want),
            "add_frame should report {want} frames: {}",
            out.text
        );
    }
    // Make frame 2 (0-based) a different pose from the rest, so
    // the clip's frames are real captured poses, not four copies.
    // (set_pose answers with the pose it wrote, root + angles.)
    let out = client.call(
        "set_pose",
        "{\"figure\":0,\"frame\":2,\"angles\":{\"2\":45}}",
    );
    assert!(!out.is_error, "set_pose failed: {}", out.text);
    assert!(
        out.text.contains("\"angles_changed\":1") && out.text.contains("45.0"),
        "set_pose should echo the written pose: {}",
        out.text
    );
    let out = client.call("clip_create", "{\"figure\":0}");
    assert!(
        !out.is_error && out.protocol_error.is_none(),
        "clip_create failed: {}",
        out.raw
    );
    out.text
}

// ---------------------------------------------------------------------
// The session that works today, proven end to end
// ---------------------------------------------------------------------

#[test]
fn movieclip_lifecycle_via_stdio_mcp() {
    let mut client = McpClient::spawn();

    // -- Tools surface: the three clip tools and the history
    //    tools this session needs are registered.
    let names = client.tool_names();
    for want in [
        "new_project",
        "add_frame",
        "set_pose",
        "clip_create",
        "clip_list",
        "set_clip_loop",
        "undo",
        "redo",
    ] {
        assert!(
            names.iter().any(|n| n.as_str() == want),
            "tools/list is missing {want:?}: {names:?}"
        );
    }

    // -- Create: new project -> 4 frames -> clip from figure 0.
    //    The clip takes the figure's poses across all frames and
    //    the figure is set looping it ("Styk clip" — the name is
    //    "<figure> clip", built by the core op).
    let created = build_clip(&mut client);
    assert!(
        created.contains("\"clip\":0"),
        "the first clip is index 0: {created}"
    );
    assert!(
        created.contains("\"name\":\"Styk clip\""),
        "the clip is named after its figure: {created}"
    );
    assert!(
        created.contains("\"frames\":4"),
        "the clip captured all four frames: {created}"
    );
    assert!(
        created.contains("\"loops_on_figure\":true"),
        "figure 0 loops the clip it was made from: {created}"
    );

    // -- List and verify.
    let (count, list) = clip_list(&mut client);
    assert_eq!(count, 1, "one clip after one create: {list}");
    assert!(
        list.contains("\"name\":\"Styk clip\"")
            && list.contains("\"frames\":4")
            && list.contains("\"used_by\":[0]"),
        "clip_list describes the created clip: {list}"
    );
    let stats = client.call("get_project_stats", "{}");
    assert_eq!(
        json_num(&stats.text, "frames"),
        Some(4),
        "the main timeline still has its four frames: {}",
        stats.text
    );

    // -- Loop mode cycles one step per call (Loop -> PingPong).
    let out = client.call("set_clip_loop", "{\"index\":0}");
    assert!(!out.is_error, "set_clip_loop failed: {}", out.text);
    assert!(
        out.text.contains("ping-pong"),
        "one cycle step lands on ping-pong, named in the status: {}",
        out.text
    );
    // An out-of-range index is a reported no-op, not an error —
    // and it must not have created or altered anything.
    let out = client.call("set_clip_loop", "{\"index\":9}");
    assert!(
        !out.is_error && out.text.contains("No such clip."),
        "an out-of-range loop cycle is a reported no-op: {}",
        out.text
    );
    let (count, _) = clip_list(&mut client);
    assert_eq!(count, 1, "the no-op left the clip alone");

    // -- Negative: a clip from a figure that does not exist is a
    //    tool error naming the range problem. (A clip from an
    //    EMPTY RANGE — the brief's other negative — cannot even
    //    be expressed via MCP: creation is figure-based across
    //    all frames, and no MCP path produces a zero-frame
    //    project. The bad-figure error is the nearest reachable
    //    refusal, and clip_create on a 1-frame project simply
    //    makes a 1-frame clip.)
    let out = client.call("clip_create", "{\"figure\":12}");
    assert!(
        out.is_error && out.text.contains("out of range"),
        "clip_create on a missing figure refuses: {}",
        out.raw
    );
    let (count, _) = clip_list(&mut client);
    assert_eq!(count, 1, "the refused create added nothing");

    // -- Undo/redo of the create itself, step by step.
    //    First undo reverts the loop cycle (the last snapshot);
    //    the clip survives it.
    let out = client.call("undo", "{}");
    assert!(
        out.text.contains("Undone."),
        "undo reports itself: {}",
        out.text
    );
    let (count, _) = clip_list(&mut client);
    assert_eq!(count, 1, "undoing the loop cycle keeps the clip");
    // Second undo reverts clip_create. (Undo is no longer the
    // only removal path — clip_delete landed in Wave 11 and is
    // proven in its own test below — but the create's undo is
    // part of the lifecycle contract.)
    let out = client.call("undo", "{}");
    assert!(out.text.contains("Undone."), "undo: {}", out.text);
    let (count, _) = clip_list(&mut client);
    assert_eq!(count, 0, "undoing clip_create removes the clip");
    // Redo restores it, same index, still looping on figure 0.
    let out = client.call("redo", "{}");
    assert!(out.text.contains("Redone.") || out.text.contains("\"ok\":true"), "redo: {}", out.text);
    let (count, list) = clip_list(&mut client);
    assert_eq!(count, 1, "redo restores the clip");
    assert!(
        list.contains("\"used_by\":[0]"),
        "the restored clip is still figure 0's loop: {list}"
    );
    // And undo removes it again, leaving the walk below a
    // clip-free timeline.
    let out = client.call("undo", "{}");
    assert!(out.text.contains("Undone."), "undo: {}", out.text);
    let (count, _) = clip_list(&mut client);
    assert_eq!(count, 0, "the clip is gone again before the walk");

    // -- The full walk back: undo every remaining snapshot until
    //    the server says there is nothing left. The state must
    //    end exactly at the fresh project (1 frame, 0 clips) —
    //    new_project itself takes no snapshot, so the walk can
    //    never fall through into the server's boot project.
    //    (Counts only: the frame/clip counts are the chain's
    //    reliable observables. Wave 11 note: set_pose takes a
    //    snapshot now — deliberately, for data safety — so the
    //    walk has one more step than it did entering the wave:
    //    the pose edit IS part of the chain, between the last
    //    add_frame and clip_create.)
    let mut steps = 0;
    let mut frames = 4;
    loop {
        steps += 1;
        assert!(steps <= 12, "the undo walk did not terminate");
        let out = client.call("undo", "{}");
        if out.text.contains("Nothing to undo.") {
            break;
        }
        assert!(
            out.text.contains("Undone."),
            "undo step {steps} reports itself: {}",
            out.text
        );
        frames = json_num(&out.text, "frames").expect("undo reports the frame count");
        assert!(frames >= 1, "the walk never drops below one frame");
    }
    assert_eq!(frames, 1, "the walk ends on the fresh project's frame");
    assert!(steps >= 4, "the walk undid real steps ({steps})");
    let (count, _) = clip_list(&mut client);
    assert_eq!(count, 0, "no clip survives the full walk");
    let state = client.call("get_state", "{}");
    assert!(
        state.text.contains("\"frame_count\":1")
            && state.text.contains("\"project\":\"Wave11 Clip Session\""),
        "the terminal state is the fresh project itself: {}",
        state.text
    );

    // -- No phantom state: with the clip undone away, addressing
    //    clip 0 is the same reported no-op as any missing clip.
    let out = client.call("set_clip_loop", "{\"index\":0}");
    assert!(
        !out.is_error && out.text.contains("No such clip."),
        "no phantom clip after the walk: {}",
        out.text
    );
}

// ---------------------------------------------------------------------
// Landed-surface tests — written as WAVE11-GAP probes against
// CORRECT behaviour while the capabilities had no MCP path;
// each passed unchanged (via its candidate-name probe) the day
// the tool landed in the Wave 11 splice.
// ---------------------------------------------------------------------

#[test]
fn clip_rename_via_mcp() {
    let mut client = McpClient::spawn();
    build_clip(&mut client);

    // Clip rename: the editor renames clips in the clips
    // panel (and the Wave 10 session repair depends on clip
    // NAMES); MCP renames them too, via clip_rename.
    let tool = client
        .pick_tool(&["clip_rename", "set_clip_name", "rename_clip"])
        .expect("a clip rename tool is registered");
    let out = client.call(&tool, "{\"index\":0,\"name\":\"Chase\"}");
    assert!(
        !out.is_error && out.protocol_error.is_none(),
        "renaming clip 0 should succeed: {}",
        out.raw
    );
    let (_, list) = clip_list(&mut client);
    assert!(
        list.contains("\"name\":\"Chase\""),
        "clip_list shows the new name: {list}"
    );

    // The duplicate-name negative (rename a second clip onto an
    // existing name) is equally unreachable today — it lives
    // behind the same missing tool. Once rename exists it
    // belongs here: create clip 2 from a second figure, rename
    // it to "Chase", and assert a tool-level refusal that
    // leaves both clips' names untouched.
}

#[test]
fn clip_delete_via_mcp() {
    let mut client = McpClient::spawn();
    build_clip(&mut client);

    // Clip delete: a real delete (not undo-of-creation, which
    // is unavailable once any later edit exists).
    let tool = client
        .pick_tool(&["clip_delete", "delete_clip", "clip_remove", "remove_clip"])
        .expect("a clip delete tool is registered");
    let out = client.call(&tool, "{\"index\":0}");
    assert!(
        !out.is_error && out.protocol_error.is_none(),
        "deleting clip 0 should succeed: {}",
        out.raw
    );
    let (count, _) = clip_list(&mut client);
    assert_eq!(count, 0, "the deleted clip is gone from clip_list");
    // Figure 0's loop assignment must not dangle onto a clip
    // that no longer exists.
    let (_, list) = clip_list(&mut client);
    assert!(
        !list.contains("\"used_by\":[0]"),
        "no clip claims figure 0 after the delete: {list}"
    );
}

#[test]
fn clip_session_enter_edit_exit() {
    let mut client = McpClient::spawn();
    let created = build_clip(&mut client);
    assert!(created.contains("\"frames\":4"), "setup clip: {created}");

    // Clip session reachability — the headline gap of this
    // session entering Wave 11, closed by the splice. The
    // editor core's clip edit session (enter_clip / exit_clip,
    // clip frame ops, and the Wave 10 invariant
    // repair_clip_session_after_history, which keeps a session
    // bound to its clip across undo/redo) is driven here end
    // to end: enter, aim a frame op INTO the clip (the
    // session's own clip_set_angle — set_pose / add_frame
    // always hit the main timeline), read the clip frame back
    // with clip_get_pose, and prove the main timeline is
    // untouched.
    let enter = client
        .pick_tool(&["clip_enter", "enter_clip", "open_clip", "clip_edit_enter"])
        .expect("a clip session enter tool is registered");

    // Everything below is the session as it SHOULD run the day
    // the enter/exit tools land (exit probed the same way).
    let out = client.call(&enter, "{\"index\":0}");
    assert!(
        !out.is_error && out.protocol_error.is_none(),
        "entering clip 0's session should succeed: {}",
        out.raw
    );
    // Edit a frame INSIDE the clip (the session's own angle
    // op targets the open session's current frame), read it
    // back through the clip pose read, then prove the main
    // timeline is untouched: its frame-2 pose keeps the
    // 45-degree angle build_clip set, and its frame count is
    // unchanged.
    let out = client.call("clip_set_angle", "{\"node\":3,\"degrees\":30}");
    assert!(!out.is_error, "in-session clip_set_angle: {}", out.text);
    let clip_pose = client.call("clip_get_pose", "{}");
    assert!(
        !clip_pose.is_error
            && clip_pose.text.contains("\"frame\":0")
            && clip_pose.text.contains("30.0"),
        "the in-clip edit reads back through clip_get_pose: {}",
        clip_pose.text
    );
    let stats = client.call("get_project_stats", "{}");
    assert_eq!(
        json_num(&stats.text, "frames"),
        Some(4),
        "the main timeline keeps its four frames during the session"
    );
    let main_pose = client.call("get_pose", "{\"figure\":0,\"frame\":2}");
    assert!(
        main_pose.text.contains("45.0"),
        "the main timeline's frame 2 is untouched by the in-clip edit: {}",
        main_pose.text
    );
    // Exit, then undo with no session open is ordinary history;
    // the no-retarget property itself (undo WHILE a session is
    // open) belongs here once enter exists: reopen the session,
    // undo the clip's rename, and assert the session reports
    // itself closed rather than editing the renamed clip.
    let exit = client
        .pick_tool(&["clip_exit", "exit_clip", "close_clip"])
        .expect("a clip session exit tool is registered");
    let out = client.call(&exit, "{}");
    assert!(
        !out.is_error && out.protocol_error.is_none(),
        "exiting the session should succeed: {}",
        out.raw
    );
}

#[test]
fn clip_session_enter_nonexistent() {
    let mut client = McpClient::spawn();
    fresh_project(&mut client);

    // Entering a clip that does not exist is a tool-level
    // refusal (isError, naming the missing clip).
    let enter = client
        .pick_tool(&["clip_enter", "enter_clip", "open_clip", "clip_edit_enter"])
        .expect("a clip session enter tool is registered");
    let out = client.call(&enter, "{\"index\":99}");
    assert!(
        out.is_error && out.protocol_error.is_none(),
        "entering a nonexistent clip is a tool error, not a protocol error \
         and not a success: {}",
        out.raw
    );
}
