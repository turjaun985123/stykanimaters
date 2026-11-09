//! Shell-control MCP handlers — workspace + shell state
//! (Wave 14, Squad B, agent B3). Declared from `main.rs`
//! by the integrator; bin-crate code, not part of the
//! engine library.
//!
//! The two handlers here drive and read the GUI shell
//! itself — [`crate::State`] — rather than the engine
//! core. That is why they cannot be ordinary registry
//! entries: the registry's handler type
//! (`stykanimate_rs::mcp::Tool::handler`) is
//! `fn(&mut EditorCore, &Value)`, and `State` lives in
//! this binary crate where the library cannot see it.
//! B2's shell-command plumbing routes live-endpoint
//! calls for these names through the shell command
//! queue, and its drain — running in the shell's poll
//! loop, on the UI thread, with `&mut State` in hand —
//! is what calls the handlers below.
//!
//! **Live-only.** These tools exist only on the live
//! HTTP endpoint hosted by the running editor. The
//! stdio server has no shell to drive, so it refuses
//! both names (B1's design, `docs/wave14/SHELL_CONTROL.md`):
//! an error result, never a silent no-op. The refusal
//! wording proposed for the stdio side is recorded in
//! each tool's entry spec below for B1/B2 to adopt
//! verbatim or amend in one place.
//!
//! **Undo.** Neither handler takes an undo step, and
//! that is the house contract kept, not broken: undo
//! snapshots store the `Project` only (the Wave 11
//! contract), and everything these tools touch is
//! shell state — the exact state a workspace-chip press
//! changes, which has never been an undo step either.
//! `shell_set_workspace` performs literally the chip
//! press's own action (`workspaces::select_workspace`),
//! persistence included.
//!
//! Integrator splice (one line, `src/main.rs` with the
//! other `mod` declarations):
//!
//! ```ignore
//! mod shell_tools_state;
//! ```

use serde_json::{json, Value};

use crate::rune_sec::workspaces::{self, Workspace};
use crate::stage_view;
use crate::State;

/// The workspace names `shell_set_workspace` accepts —
/// every [`Workspace`] under its persisted name
/// (`Workspace::name`: Animate, Draw, Graph, Effects,
/// Audio, 3D, Export, Home), joined for refusal text.
/// Derived from `Workspace::all()` so the list cannot
/// drift from the enum.
fn valid_workspace_names() -> String {
    Workspace::all()
        .iter()
        .map(|ws| ws.name())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Strictly parse a workspace name for the tool. The
/// shell's own `workspaces::from_name` is deliberately
/// lenient (an unknown persisted name falls back to
/// Animate), which is right for a prefs file and wrong
/// for a tool call: a misspelled name must be refused,
/// not silently landed on Animate. Exact, case-sensitive
/// match against the persisted names — `"3D"`, not
/// `"Build3D"` (the merged GPUI workspace has no name
/// of its own; see the `workspaces` module docs).
fn parse_workspace(name: &str) -> Result<Workspace, String> {
    Workspace::all()
        .into_iter()
        .find(|ws| ws.name() == name)
        .ok_or_else(|| {
            format!(
                "unknown workspace '{name}': valid workspaces are {}",
                valid_workspace_names()
            )
        })
}

/// A required string argument, in the registry handlers'
/// own wording (`mcp_tools.rs`'s `req_str`, which is
/// library-private and cannot be reused from here).
fn req_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    match args.get(key) {
        Some(v) => v
            .as_str()
            .ok_or_else(|| format!("argument '{key}' must be a string")),
        None => Err(format!("missing required argument '{key}'")),
    }
}

// TOOL ENTRY SPEC (for B2's live-endpoint registration
// and B10/E3's MCP.md pass — this handler's signature
// takes `&mut State`, so it is NOT a `mcp::Tool` entry;
// register it in the live shell-command table beside
// B2's other shell tools, under exactly this name,
// description and schema):
//
//   name: "shell_set_workspace"
//   description: "Switch the running editor's workspace
//     by name: one of Animate, Draw, Graph, Effects,
//     Audio, 3D, Export, Home (exact, case-sensitive;
//     '3D' is the merged Build/3D workspace). The switch
//     is the workspace chip's own action — it re-gates
//     which panels mount and persists to the layout
//     prefs. An unknown name is refused, listing the
//     valid set. Shell state only: no undo step. Live
//     endpoint only — the stdio server refuses this
//     tool."
//   schema: obj_schema(
//       json!({ "workspace": { "type": "string" } }),
//       &["workspace"],
//   )
//   handler: crate::shell_tools_state::tool_shell_set_workspace
//
//   stdio refusal (proposed wording for B1/B2):
//     "shell_set_workspace is live-only: it drives the
//     running editor's shell and has no meaning for the
//     stdio server — call it through the editor's live
//     endpoint instead."
//
//   MCP.md catalogue row (new "Shell control — live
//   endpoint only" table; columns Tool | Purpose |
//   Required | Optional):
//   | `shell_set_workspace` | Switch the running editor's workspace by name: one of Animate, Draw, Graph, Effects, Audio, 3D, Export, Home (exact, case-sensitive). The workspace chip's own action, persistence included; an unknown name is refused, listing the valid set. Shell state only: no undo step. Live endpoint only. | `workspace` | — |

/// MCP handler (live endpoint only): switch the shell's
/// workspace — `workspaces::select_workspace`, the exact
/// action the workspace chips dispatch, so the field
/// write and the `LayoutPrefs` persistence can never
/// diverge from a mouse switch. No undo step (shell
/// state; see the module docs).
pub fn tool_shell_set_workspace(s: &mut State, args: &Value) -> Result<Value, String> {
    let name = req_str(args, "workspace")?;
    let ws = parse_workspace(name)?;
    let previous = s.workspace.name();
    workspaces::select_workspace(s, ws);
    Ok(json!({
        "ok": true,
        "workspace": ws.name(),
        "previous": previous,
    }))
}

// TOOL ENTRY SPEC (same registration route as above):
//
//   name: "shell_get_state"
//   description: "Read the running editor's shell state
//     in one call: the active workspace; the transport
//     (playing or paused, current frame and fractional
//     playhead, frame count, fps, loop range and
//     playback toggles); the view (view-layer zoom and
//     pan, kept separate from the authored camera at the
//     current frame, plus the effective on-screen zoom
//     the two compose to); the selections (figure, node,
//     sprite, audio clip, FX clip, and the movieclip
//     edit session if one is open); the theme; and which
//     collapsible panels are collapsed. Read-only; no
//     undo step. Live endpoint only — the stdio server
//     refuses this tool."
//   schema: obj_schema(json!({}), &[])
//   handler: crate::shell_tools_state::tool_shell_get_state
//
//   stdio refusal (proposed wording for B1/B2):
//     "shell_get_state is live-only: it reads the
//     running editor's shell and has no meaning for the
//     stdio server — call it through the editor's live
//     endpoint instead."
//
//   MCP.md catalogue row:
//   | `shell_get_state` | Read the running editor's shell state in one call: workspace; transport (playing/paused, frame, playhead, frame count, fps, loop, speed); view (view zoom/pan separate from the authored camera, plus the effective zoom); selections (figure, node, sprite, audio clip, FX clip, clip edit session); theme; collapsed panels. Read-only. Live endpoint only. | — | — |

/// The shell-state snapshot `shell_get_state` returns,
/// factored out of the handler so B10's tests (and any
/// future shell surface) read the exact same shape.
///
/// Every value is read from the live field it names —
/// no cached or derived copies beyond the two marked
/// compositions:
///
/// - `transport.state` is `core.playing` rendered as
///   words ("playing" / "paused").
/// - `view.effective_zoom` is the one composed figure:
///   `stage_view::view_zoom_factor`, the authored camera
///   zoom at the current frame multiplied by the view
///   zoom — the number the camera overlay shows. The
///   two factors are also reported separately
///   (`view.view_zoom`, `view.authored_camera.zoom`) so
///   the view-vs-authored distinction is never blurred:
///   the view layer is session-only navigation (Wave 5
///   doctrine) and is never written into the project,
///   while the authored camera is project data.
///
/// Two honest gaps, recorded rather than papered over:
///
/// - The movieclips panel's *selection* is not here:
///   it lives in a thread-local inside
///   `rune_sec::clips_panel` behind a private accessor,
///   unreachable from outside that module. What is
///   reachable is the core's clip *edit session*
///   (`core.clip_edit`), reported as
///   `selections.clip_edit`. Reading the panel selection
///   needs a pub accessor in `clips_panel.rs` (an
///   integrator/B6 follow-up, not this file's edit).
/// - `collapsed.timeline` is the persisted
///   `LayoutPrefs.timeline_collapsed` value, read fresh
///   via `load_layout()` the way the shell itself reads
///   layout prefs; no `State` field carries it. The two
///   panel flags beside it are live `State` fields.
pub fn shell_state_json(s: &State) -> Value {
    let core = &s.core;
    let nav = stage_view::view_nav();
    let (cam_x, cam_y, cam_zoom) = core.camera_here();
    json!({
        "ok": true,
        "workspace": s.workspace.name(),
        "transport": {
            "playing": core.playing,
            "state": if core.playing { "playing" } else { "paused" },
            // The playhead frame, 0-based (the core's own
            // numbering) beside the 1-based figure the
            // status strip shows ("Frame 1 / 5").
            "frame": core.current,
            "frame_display": core.current + 1,
            // The fractional transport position, in
            // frames (`core.playhead`, the field `poll`
            // advances while playing).
            "playhead": core.playhead,
            "frame_count": core.project.frames.len(),
            "fps": core.project.fps,
            "loop_range": core.loop_range.map(|(a, b)| json!([a, b])),
            "loop_playback": s.loop_playback,
            "playback_speed": s.playback_speed,
        },
        "view": {
            // The view layer only (stage_view::ViewNav,
            // thread-local session state): a multiplier
            // over the authored camera, never project data.
            "view_zoom": nav.zoom,
            "view_pan": [nav.pan[0], nav.pan[1]],
            "view_is_identity": nav.zoom == 1.0 && nav.pan == [0.0, 0.0],
            // The authored camera at the current frame —
            // project data (`Project::camera_at`).
            "authored_camera": {
                "x": cam_x,
                "y": cam_y,
                "zoom": cam_zoom,
            },
            // Authored zoom × view zoom: the on-screen
            // magnification, as the camera overlay shows.
            "effective_zoom": stage_view::view_zoom_factor(core),
        },
        "selections": {
            // An index, not an Option: the core always has
            // a selected figure slot. The name rides along
            // (null if the index ever dangles).
            "figure": core.selected_figure,
            "figure_name": core
                .project
                .figures
                .get(core.selected_figure)
                .map(|f| f.name.as_str()),
            "node": core.selected_node,
            "sprite": s.sprites_panel.selected,
            "audio_clip": s.audio_panel.selected,
            "fx_clip": s.fx_selected,
            // The clips panel's selection (Wave 14: made
            // reachable by clips_panel::clip_selection,
            // the accessor shell_select_clip's wave added;
            // before that the thread-local state had no
            // reader outside its module).
            "clip_panel": crate::rune_sec::clips_panel::clip_selection(),
            // The movieclip edit session, when one is open.
            "clip_edit": core.clip_edit.as_ref().map(|sess| json!({
                "clip": sess.clip,
                "frame": sess.frame,
                "name": sess.name,
            })),
        },
        // The onion-skin ghost depths (shell state —
        // State::onion_ui; the core's master switch is
        // the get_onion_skin tool's field).
        "onion": {
            "prev": s.onion_ui.prev,
            "next": s.onion_ui.next,
        },
        "theme": if core.light_theme { "light" } else { "dark" },
        "light_theme": core.light_theme,
        "collapsed": {
            "figures_panel": s.figures_panel.collapsed,
            "library_panel": s.library_panel.collapsed,
            "timeline": stykanimate_rs::shell::load_layout().timeline_collapsed,
        },
    })
}

/// MCP handler (live endpoint only): read the shell
/// state — see [`shell_state_json`] for the shape and
/// the field-by-field provenance. Read-only; no undo
/// step.
pub fn tool_shell_get_state(s: &State, _args: &Value) -> Result<Value, String> {
    Ok(shell_state_json(s))
}

// TOOL ENTRY SPEC: shell_set_onion_depths
//   name: "shell_set_onion_depths"
//   description: "Set the onion-skin ghost depths in the live editor — 'prev' and/or 'next' (0-3 each), the Onion section's steppers by target value. Shell view state: no undo step, never marks the project dirty, session-only exactly like the steppers. Live endpoint only."
//   schema: obj_schema(json!({ "prev": { "type": "integer" }, "next": { "type": "integer" } }), &[])
//   handler: crate::shell_tools_state::tool_shell_set_onion_depths
//     (&mut State — dispatched by the shell-control
//     drain, not registrable in all_tools()).
//   MCP.md row:
//   | `shell_set_onion_depths` | Set the onion-skin ghost depths (the Onion steppers, by target; 0–3 each). Session-only; no undo step. Live endpoint only. | — | `prev`, `next` |
/// Set the onion-skin ghost depths — the writes the
/// Onion section's −/+ steppers make to
/// `State::onion_ui.prev` / `.next` (Wave 14 integrator
/// addition: A9's parity fill covered the core's master
/// switch, but the per-side depths are shell state and
/// had no tool). Values are absolute targets clamped to
/// the panel's own range by validation: an out-of-range
/// depth is refused with the range quoted, exactly as
/// the steppers simply stop at 0 and 3
/// (`onion_controls::MAX_ONION_DEPTH`). Session state:
/// no undo step, no dirty mark, nothing persisted —
/// the steppers' own discipline.
pub fn tool_shell_set_onion_depths(s: &mut State, args: &Value) -> Result<Value, String> {
    fn depth_arg(args: &Value, key: &str) -> Result<Option<u8>, String> {
        let Some(v) = args.get(key).filter(|v| !v.is_null()) else {
            return Ok(None);
        };
        let n = v.as_u64().ok_or_else(|| {
            format!("argument '{key}' must be an integer between 0 and 3, got {v}")
        })?;
        if n > 3 {
            return Err(format!(
                "argument '{key}' must be between 0 and 3 (the onion depth range), got {n}"
            ));
        }
        Ok(Some(n as u8))
    }
    let prev = depth_arg(args, "prev")?;
    let next = depth_arg(args, "next")?;
    if prev.is_none() && next.is_none() {
        return Err("argument 'prev' or 'next' is required (give at least one depth)".into());
    }
    if let Some(p) = prev {
        s.onion_ui.prev = p;
    }
    if let Some(n) = next {
        s.onion_ui.next = n;
    }
    Ok(json!({
        "ok": true,
        "prev": s.onion_ui.prev,
        "next": s.onion_ui.next,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_workspace_accepts_every_persisted_name() {
        for ws in Workspace::all() {
            assert_eq!(parse_workspace(ws.name()), Ok(ws));
        }
    }

    #[test]
    fn parse_workspace_accepts_3d_under_its_persisted_name() {
        assert_eq!(parse_workspace("3D"), Ok(Workspace::Build3D));
    }

    #[test]
    fn parse_workspace_refuses_unknown_names_and_lists_the_valid_set() {
        for bad in ["", "Build", "Build3D", "animate", "3d", "Animat"] {
            let err = parse_workspace(bad).expect_err("must refuse");
            assert!(
                err.contains(&format!("'{bad}'")),
                "the refusal quotes the offending name: {err}"
            );
            assert!(
                err.contains("Animate") && err.contains("3D") && err.contains("Home"),
                "the refusal lists the valid set: {err}"
            );
        }
    }
}
