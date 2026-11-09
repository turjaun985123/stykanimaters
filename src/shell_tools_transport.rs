//! Wave 14 (B4): MCP handlers for the shell's playback
//! transport — play, pause, playhead scrub and a
//! transport readout.
//!
//! Where the state actually lives (all read from source
//! at the Wave 13 closing tree, `92c5d95`):
//!
//! * The transport is **core state, not shell state**.
//!   `EditorCore::playing` (`src/editor.rs:38`),
//!   `EditorCore::current` (`:33`), the fractional
//!   `EditorCore::playhead` (`:63`) and
//!   `EditorCore::loop_range` (`:67`) are all public
//!   fields of the library core, so these handlers run
//!   against the live core through the ordinary MCP
//!   drain — no shell command channel is needed for
//!   anything in this file. The two transport-adjacent
//!   values that ARE shell state — `State::loop_playback`
//!   (the loop on/off toggle, `src/main.rs:226`) and
//!   `State::playback_speed` (`src/main.rs:230`) — are
//!   not readable from a core handler and are therefore
//!   NOT in `shell_get_transport`'s reply; if B1/B2's
//!   shell-state read lands, it can add them there.
//! * The shell's `poll` advances the playhead while
//!   `core.playing` (`src/main.rs`, `poll`: it calls
//!   `core.advance_play(dt * s.playback_speed)`), and
//!   its Phase F flip hook starts/stops playback audio
//!   whenever `core.playing` changes — the hook's own
//!   comment names "transport button, Space, command
//!   palette" as equivalent flip sources. An MCP flip
//!   is one more such source: on the live endpoint the
//!   sound follows exactly as for a button press.
//! * Loop behaviour: `advance_play` wraps at the
//!   timeline end, or inside `loop_range` when one is
//!   set (`src/editor.rs`, `advance_play`). The shell
//!   additionally stops at the end when its
//!   `loop_playback` flag is off
//!   (`playback_controls::should_stop_at_end`, consulted
//!   by `poll` before each advance) — a shell-side
//!   enforcement these handlers neither replicate nor
//!   override.
//!
//! Discipline: playhead moves and transport flips are
//! **session state, never project edits** — no undo
//! snapshot, no dirty mark. The precedent is in the
//! core itself: `loop_range` is documented "Runtime
//! only... undo snapshots cover the project, not
//! this", `goto` takes no snapshot, and the GUI's own
//! play/pause body (`chrome_view::toggle_play`,
//! `src/chrome_view.rs:211`) touches only `playing`
//! and `playhead`. These handlers mirror that exactly.
//!
//! **There is no `shell_stop`, deliberately.** The GUI
//! has no Stop control: the transport family is the
//! Play/Pause toggle, Prev/Next, Restart (go to frame
//! 0 AND play), Play range (go to the range start AND
//! play), the loop toggle and the speed cycle
//! (`chrome_view::transport_row`;
//! `rune_sec/playback_controls.rs` module doc). No GUI
//! control performs "pause + return to the loop
//! start", so inventing that composite for MCP would
//! create behaviour the editor itself does not have.
//! A caller that wants the playhead parked at the
//! range start composes `shell_pause` +
//! `shell_set_playhead` — the two real primitives.
//!
//! Transport honesty: on the **stdio** server there is
//! no shell and no poll loop, so `shell_play` sets the
//! same flag the GUI would — but nothing advances it
//! and no audio plays; the project state the flag
//! belongs to is real, the playback is a live-shell
//! phenomenon. The MCP.md rows say so.

use serde_json::{json, Value};

use crate::editor::EditorCore;

/// A whole-frame argument, validated the way the
/// sprite handlers validate theirs: an unsigned or
/// non-negative integer, or a float with no fractional
/// part. Anything else (negative, fractional, wrong
/// type) is refused, quoting the value.
fn sht_req_frame(args: &Value, key: &str) -> Result<usize, String> {
    let raw = args
        .get(key)
        .ok_or_else(|| format!("missing required argument: {key}"))?;
    if let Some(v) = raw.as_u64() {
        return Ok(v as usize);
    }
    if let Some(v) = raw.as_i64() {
        if v >= 0 {
            return Ok(v as usize);
        }
        return Err(format!("argument '{key}' must not be negative (got {v})"));
    }
    if let Some(v) = raw.as_f64() {
        if v.fract() != 0.0 {
            return Err(format!(
                "argument '{key}' must be a whole frame number (got {v})"
            ));
        }
        if v < 0.0 {
            return Err(format!("argument '{key}' must not be negative (got {v})"));
        }
        return Ok(v as usize);
    }
    Err(format!(
        "argument '{key}' must be a whole frame number (got {raw})"
    ))
}

/// The transport fields every reply in this family
/// carries, read back from the core after the write so
/// a caller sees the state that actually resulted.
fn transport_json(core: &EditorCore) -> Value {
    json!({
        "playing": core.playing,
        "frame": core.current,
        "playhead": core.playhead,
        "frame_count": core.frame_count(),
        "fps": core.project.fps,
        "loop_range": core.loop_range.map(|(a, b)| json!([a, b])),
    })
}

/// Start playback — the play half of the GUI's
/// Play/Pause toggle (`chrome_view::toggle_play`):
/// starting from a parked playhead first snaps the
/// fractional `playhead` onto the current frame (so a
/// play press starts cleanly on the parked frame,
/// never mid-tween), then sets `playing`. Calling it
/// while already playing is a successful no-op that
/// does NOT re-snap the playhead (the toggle button
/// would pause in that situation; a set-style tool
/// must not). Audio is not started here: the shell
/// poll's flip hook observes the flag and starts it,
/// exactly as for the button. No undo step, no dirty
/// mark — session state only. On an empty project the
/// flag still flips, at parity with the GUI's Play
/// button (which is not frame-gated; its advance
/// simply no-ops until frames exist).
//
// TOOL ENTRY SPEC: for the integrator's registry splice
// in `mcp_tools::tools()`, in the shell family group:
//
//   Tool {
//       name: "shell_play",
//       description: "Start playback in the live editor (the Play button): the playhead starts from the current frame and the shell poll advances it and starts playback audio, exactly as for a GUI play press. Idempotent — calling it while already playing changes nothing. Not an undo step; does not mark the project dirty. On the stdio server the flag is set but nothing advances it.",
//       schema: obj_schema(json!({}), &[]),
//       handler: crate::shell_tools_transport::tool_shell_play,
//   }
//
// MCP.md ROW SPEC: for the shell transport table:
//
//   | `shell_play` | Start playback in the live editor (the Play button): the playhead starts from the current frame; the shell poll advances it and starts playback audio, as for a GUI play press. Idempotent. Not an undo step; never marks dirty. On stdio the flag is set but nothing advances it. | — | — |
pub fn tool_shell_play(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    if !core.playing {
        core.playhead = core.current as f32;
        core.playing = true;
    }
    Ok(transport_json(core))
}

/// Pause playback — the pause half of the GUI's
/// Play/Pause toggle: clears `playing` and leaves the
/// playhead exactly where it stopped (the fractional
/// `playhead` is NOT snapped to a frame boundary,
/// matching the toggle, which only snaps on start).
/// The shell poll's flip hook stops the audio.
/// Calling it while already paused is a successful
/// no-op. No undo step, no dirty mark.
//
// TOOL ENTRY SPEC: for the integrator's registry splice
// in `mcp_tools::tools()`, in the shell family group,
// after `shell_play`:
//
//   Tool {
//       name: "shell_pause",
//       description: "Pause playback in the live editor (the Pause button): the playhead stays exactly where it stopped, and the shell poll stops playback audio. Idempotent — calling it while already paused changes nothing. Not an undo step; does not mark the project dirty.",
//       schema: obj_schema(json!({}), &[]),
//       handler: crate::shell_tools_transport::tool_shell_pause,
//   }
//
// MCP.md ROW SPEC: for the shell transport table,
// after the `shell_play` row:
//
//   | `shell_pause` | Pause playback in the live editor (the Pause button): the playhead stays where it stopped; the shell poll stops playback audio. Idempotent. Not an undo step; never marks dirty. | — | — |
pub fn tool_shell_pause(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    core.playing = false;
    Ok(transport_json(core))
}

/// Move the playhead to a whole frame — the timeline
/// click's own path, `EditorCore::goto` (the frame
/// strip's press handler calls exactly this,
/// `rune_sec/timeline_marks.rs`): sets `current` and
/// snaps `playhead` onto the frame. Works whether or
/// not playback is running (a click during playback
/// seeks, and the poll's seek sync re-aligns the
/// audio). Out-of-range frames are refused up front,
/// quoting the frame and the frame count — a click
/// cannot produce one, and silently clamping would
/// hide a caller bug; the `set_frame` built-in refuses
/// the same way. No undo step, no dirty mark: `goto`
/// takes no snapshot and selection is untouched (a
/// timeline click does not change the figure/node
/// selection either).
//
// TOOL ENTRY SPEC: for the integrator's registry splice
// in `mcp_tools::tools()`, in the shell family group,
// after `shell_pause`:
//
//   Tool {
//       name: "shell_set_playhead",
//       description: "Move the playhead to a 0-based frame (a timeline click): sets the current frame and snaps the fractional playhead onto it, playing or paused. Out-of-range frames are refused. The shell-family name for the same move the set_frame built-in makes. Not an undo step; does not mark the project dirty; selection is untouched.",
//       schema: obj_schema(
//           json!({
//               "frame": { "type": "integer" }
//           }),
//           &["frame"],
//       ),
//       handler: crate::shell_tools_transport::tool_shell_set_playhead,
//   }
//
// MCP.md ROW SPEC: for the shell transport table,
// after the `shell_pause` row:
//
//   | `shell_set_playhead` | Move the playhead to a 0-based frame (a timeline click, `EditorCore::goto`): sets the current frame and snaps the fractional playhead onto it, playing or paused. Out-of-range frames are refused. The shell-family name for the set_frame built-in's move. Not an undo step; never marks dirty; selection untouched. | `frame` | — |
pub fn tool_shell_set_playhead(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let frame = sht_req_frame(args, "frame")?;
    let count = core.frame_count();
    if frame >= count {
        return Err(format!(
            "frame {frame} is out of range: the project has {count} frame(s)"
        ));
    }
    core.goto(frame);
    Ok(transport_json(core))
}

/// Read the transport state: whether playback is
/// running, the current frame, the fractional
/// playhead, the frame count, the project's fps and
/// the loop range (`[start, end]`, inclusive, or
/// null). Read-only. The shell's loop on/off toggle
/// and playback speed are shell `State` fields, not
/// core state, and are not in this reply — see the
/// module doc.
//
// TOOL ENTRY SPEC: for the integrator's registry splice
// in `mcp_tools::tools()`, in the shell family group,
// after `shell_set_playhead`:
//
//   Tool {
//       name: "shell_get_transport",
//       description: "Read the playback transport state: playing, current frame, fractional playhead, frame count, project fps and the loop range ([start, end] inclusive, or null). Read-only. The loop on/off toggle and playback speed are shell state and are not reported here.",
//       schema: obj_schema(json!({}), &[]),
//       handler: crate::shell_tools_transport::tool_shell_get_transport,
//   }
//
// MCP.md ROW SPEC: for the shell transport table,
// after the `shell_set_playhead` row:
//
//   | `shell_get_transport` | Read the transport state: `playing`, current `frame`, fractional `playhead`, `frame_count`, project `fps` and `loop_range` (`[start, end]` inclusive or null). Read-only. The loop on/off toggle and playback speed are shell state and are not reported. | — | `playing`, `frame`, `playhead`, `frame_count`, `fps`, `loop_range` |
pub fn tool_shell_get_transport(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    Ok(transport_json(core))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn core() -> EditorCore {
        // The demo project: 5 frames (Project::new_demo).
        EditorCore::new()
    }

    #[test]
    fn play_snaps_the_playhead_onto_the_parked_frame_and_sets_the_flag() {
        let mut c = core();
        c.goto(2);
        c.playhead = 2.75; // parked mid-tween
        let undo_before = c.undo_depth();
        let out = tool_shell_play(&mut c, &json!({})).expect("play succeeds");
        assert!(c.playing);
        assert_eq!(c.playhead, 2.0, "the start snaps onto the parked frame");
        assert_eq!(out["playing"], json!(true));
        assert_eq!(out["frame"], json!(2));
        assert_eq!(c.undo_depth(), undo_before, "play is not an undo step");
    }

    #[test]
    fn play_while_playing_is_a_no_op_that_keeps_the_playhead() {
        let mut c = core();
        c.playing = true;
        c.playhead = 1.5;
        let out = tool_shell_play(&mut c, &json!({})).expect("play succeeds");
        assert!(c.playing);
        assert_eq!(c.playhead, 1.5, "no re-snap while already playing");
        assert_eq!(out["playhead"], json!(1.5));
    }

    #[test]
    fn pause_clears_the_flag_and_leaves_the_playhead_mid_frame() {
        let mut c = core();
        c.playing = true;
        c.playhead = 3.25;
        let undo_before = c.undo_depth();
        let out = tool_shell_pause(&mut c, &json!({})).expect("pause succeeds");
        assert!(!c.playing);
        assert_eq!(c.playhead, 3.25, "pause never snaps the playhead");
        assert_eq!(out["playing"], json!(false));
        assert_eq!(c.undo_depth(), undo_before);
        // Idempotent: a second pause succeeds and changes nothing.
        tool_shell_pause(&mut c, &json!({})).expect("second pause succeeds");
        assert!(!c.playing);
        assert_eq!(c.playhead, 3.25);
    }

    #[test]
    fn set_playhead_goes_through_goto_without_an_undo_step() {
        let mut c = core();
        let undo_before = c.undo_depth();
        let out = tool_shell_set_playhead(&mut c, &json!({ "frame": 3 }))
            .expect("scrub succeeds");
        assert_eq!(c.current, 3);
        assert_eq!(c.playhead, 3.0);
        assert_eq!(out["frame"], json!(3));
        assert_eq!(c.undo_depth(), undo_before, "a scrub is not an undo step");
    }

    #[test]
    fn set_playhead_refuses_out_of_range_and_fractional_frames() {
        let mut c = core();
        let n = c.frame_count();
        let err = tool_shell_set_playhead(&mut c, &json!({ "frame": n }))
            .expect_err("one past the end must be refused");
        assert!(
            err.contains(&format!("frame {n} is out of range")),
            "err: {err}"
        );
        let err = tool_shell_set_playhead(&mut c, &json!({ "frame": 1.5 }))
            .expect_err("a fractional frame must be refused");
        assert!(err.contains("whole frame number"), "err: {err}");
        let err = tool_shell_set_playhead(&mut c, &json!({ "frame": -1 }))
            .expect_err("a negative frame must be refused");
        assert!(err.contains("must not be negative"), "err: {err}");
        assert_eq!(c.current, 0, "refusals never move the playhead");
    }

    #[test]
    fn get_transport_reports_the_core_fields_and_loop_range() {
        let mut c = core();
        c.goto(1);
        let out = tool_shell_get_transport(&mut c, &json!({})).expect("read succeeds");
        assert_eq!(out["playing"], json!(false));
        assert_eq!(out["frame"], json!(1));
        assert_eq!(out["frame_count"], json!(c.frame_count()));
        assert_eq!(out["fps"], json!(c.project.fps));
        assert_eq!(out["loop_range"], Value::Null);
        c.loop_range = Some((1, 3));
        let out = tool_shell_get_transport(&mut c, &json!({})).expect("read succeeds");
        assert_eq!(out["loop_range"], json!([1, 3]));
    }
}
