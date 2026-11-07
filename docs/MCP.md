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

## Tools (30)

Session: `server_info`, `get_state`, `set_frame`, `project_summary`.

Build: `new_project`, `add_figure` (humanoid/quadruped/mini),
`list_figures`, `add_frame`, `insert_inbetween`, `delete_frame`,
`set_frame_duration`, `get_pose`, `set_pose` (root + per-joint
angles), `set_node`, `add_node`, `delete_node`, `set_camera`
(x/y/zoom/**angle**), `add_fx_clip` (11 kinds), `set_weather`
(rain/snow/fireflies/off), `add_audio_clip`, `graph_key`,
`layers` (lock/hide/opacity/solo), `undo`, `redo`.

Finish: `save_project`, `load_project`, `import_file`
(.piv/.stk/.nodes/.nodemc/.stknds/.json), `export_file`
(piv3/piv5/stk), `render` (png/gif/mp4/webm/apng/svg/jpeg/
spritesheet), `render_frame_base64` — returns the frame as a PNG
the agent can look at, check, and iterate on.

All tools validate their arguments and return structured errors —
an agent can recover from a bad call without crashing the session.
A full scripted session (build → pose → weather → render → save →
export .piv, verified against the original's Python decoder) is
reproduced in the Wave 6 notes of docs/GAP-ANALYSIS.md.
