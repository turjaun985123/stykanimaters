# Driving StykAnimate-RS from an AI agent (MCP)

StykAnimate-RS ships an MCP (Model Context Protocol) server: any
MCP-capable AI client can create, pose, decorate, render and export
animations end-to-end without the GUI.

## Run it

```sh
cargo build --release --bin mcp_server
./target/release/mcp_server        # speaks JSON-RPC on stdio
```

## Connect a client

Stdio MCP servers are configured the same way in most clients
(Claude Desktop, Claude Code, Cursor, …):

```json
{
  "mcpServers": {
    "stykanimate": {
      "command": "/path/to/StykAnimate-RS/target/release/mcp_server",
      "args": []
    }
  }
}
```

The server holds one editing session (one project) in memory.
Protocol: JSON-RPC 2.0, one message per line — `initialize`,
`notifications/initialized`, `ping`, `tools/list`, `tools/call`.
Protocol version `2024-11-05`.

## Tools (49)

Session: `server_info`, `get_state`, `set_frame`, `project_summary`.

Build: `new_project`, `add_figure` (humanoid/quadruped/mini),
`list_figures`, `add_frame`, `insert_inbetween`, `delete_frame`,
`set_frame_duration`, `get_pose`, `set_pose` (root + per-joint
angles), `set_node`, `add_node`, `delete_node`, `set_camera`
(x/y/zoom/**angle**), `add_fx_clip` (all 14 kinds: burst, sparkle,
smoke, beam, aura, trail, confetti, shock_ring, ember, fountain,
orbit, lightning, bubbles, leaves), `fx_list`, `fx_move`,
`fx_resize`, `fx_duplicate`, `fx_delete`, `set_weather`
(rain/snow/fireflies/off), `add_audio_clip`, `graph_key`,
`layers` (lock/hide/opacity/solo), `undo`, `redo`.

Draw (round 2): `draw_stroke`, `smooth_strokes`,
`erase_strokes`, `apply_nib` — the editor's stroke tools
(Chaikin smoothing, nib profiles, splitting eraser) over any
frame's strokes.

3D session (round 2): `fig3_enter`, `fig3_select`,
`fig3_rotate`, `fig3_orbit`, `fig3_mark_b`, `fig3_bake`
(single pose → native 2D figure), `fig3_bake_animation`
(interpolate between the current pose and the marked pose B
over N frames).

Movieclips + bends (round 2): `clip_create` (capture a
figure's timeline poses as a looping clip), `clip_list`,
`set_bend` (per-frame, per-node bend in radians — the same
`Frame.bends` the .piv v5 exporter writes).

Finish: `save_project`, `load_project`, `import_file`
(.piv/.stk/.nodes/.nodemc/.stknds/.json), `export_file`
(piv2/piv3/piv4/piv5/stk/nodes/stknds), `render`
(png/gif/mp4/webm/apng/svg/jpeg/spritesheet),
`render_frame_base64` — returns the frame as a PNG
the agent can look at, check, and iterate on.

All tools validate their arguments and return structured errors —
an agent can recover from a bad call without crashing the session.
A full scripted session (build → pose → weather → render → save →
export .piv, verified against the original's Python decoder) is
reproduced in the Wave 6 notes of docs/GAP-ANALYSIS.md.
