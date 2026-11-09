# Wave 14 — Palette-command parity ledger (A10)

**What this is.** The complete map of the command palette's
**134 commands** (`pub static COMMANDS`,
`src/rune_sec/command_palette.rs:1319-1473`; the count is
pinned by the in-file test at `command_palette.rs:2150`,
"palette should cover 134 commands") against the entering
MCP surface of **288 tools** (235 registry in
`src/mcp_tools.rs` + 44 built-ins in `src/mcp.rs` + 9 io
tools in `src/mcp_io.rs`). Every row traces the command's
`run` fn-pointer to what it actually calls — read from the
shim bodies in `command_palette.rs` (lines 308-1313) and
the three direct-fn entries — and names the MCP tool whose
handler was **verified in source** to reach the same
`EditorCore` op, or records exactly why no tool does.

**Verdict vocabulary.**

- **COVERED** — a tool handler calls the very op (or the
  identical underlying write) the shim calls. Handler
  file:line is the evidence.
- **COVERED-EQ** — no tool calls that op, but the
  command's full effect is reachable with existing tools
  (an absolute setter where the palette steps/cycles, a
  per-index form where the palette targets "last" or
  "selected"). Any semantic residue (undo-step
  differences, a panel-local side effect) is named in
  the row.
- **SHELL → B** — the shim touches shell `State` only
  (workspace, view flags, shell selections, transport
  flag); it belongs to Squad B's shell-control tools.
  The owning B item is named.
- **EXCLUSION-CANDIDATE** — a named exclusion is argued
  in one line.
- **GAP → A‹n›** — a core op with no tool; the gap sits
  inside that squad's roster domain, which owns the fix.
- **GAP → INTEGRATOR-OWNED** — a core op with no tool
  and no A2-A9 domain owns it; the integrator's splice
  list, per the A10 remit.

**Headline findings.**

1. **107 of 134 commands are already tool-covered**
   (61 COVERED + 46 COVERED-EQ). The palette is in far
   better shape than the entering assumption: Waves
   9-13's parity built-ins and per-domain handlers
   already reach nearly every shim target.
2. **Three handlers exist, are tested, and were never
   registered** — `tool_figure_toggle_hidden` /
   `tool_figure_toggle_lock` / `tool_figure_toggle_solo`
   (`src/figure_ops.rs:1401/1434/1466`, Tool-entry specs
   in the comments directly above each) and
   `tool_set_draw_mode` (`src/draw_ops.rs:1341`, spec at
   `:1326`). They are invisible to the census because
   the registry in `mcp_tools.rs` never spliced them.
   The three figure toggles are COVERED-EQ today via
   the `layers` tool; `draw-mode` has no equivalent and
   is a true GAP → A3 (splice-only fix).
3. **No palette command opens a native dialog.** Save,
   load, open and cycle all operate on `./projects`
   paths (`editor.rs:4787-4816`, `editor.rs:1940-1960`);
   `add-audio` scans `./audio` (`editor.rs:3528`);
   `attach-media-import` scans the import folder. The
   dialog-bound exclusion family the charter anticipated
   does not exist in this table — exactly one
   EXCLUSION-CANDIDATE remains (`toggle-live-mcp`).
4. **The only wholly untooled core ops behind palette
   commands are:** `onion` (a field flip, GAP → A9),
   `stamp_polygon` (GAP → A3), `cycle_project_file` /
   `open_project_file` (GAP → A8), and the style
   clipboard pair `copy_style` / `paste_style`
   (GAP → INTEGRATOR-OWNED — the clipboard is a
   panel-local `thread_local` in
   `src/rune_sec/inspector_style.rs:486-492`, and no
   Wave 14 squad owns the style domain).

## Summary counts

| Verdict | Count |
|---|---|
| COVERED (tool calls the same op) | 61 |
| COVERED-EQ (state-equivalent via other tools) | 46 |
| **Tool-covered subtotal** | **107** |
| SHELL → Squad B | 19 |
| EXCLUSION-CANDIDATE | 1 |
| GAP → A2 (splice pending; EQ-covered meanwhile) | 0 rows — see notes on rows 28, 29, 30 |
| GAP → A3 | 2 |
| GAP → A8 | 2 |
| GAP → A9 | 1 |
| GAP → INTEGRATOR-OWNED | 2 |
| **Total** | **134** |

Rows 28-30 (hide / solo / lock figure) are counted in
COVERED-EQ: their effect is reachable today through
`layers`. Their dedicated relative-form handlers are
nevertheless an A2 splice item, flagged in each row —
if the integrator prefers strict accounting, move those
3 rows from COVERED-EQ to GAP → A2 and the subtotal
becomes 104 covered / 3 A2 gaps; no other row changes.

## The table

Shim evidence is `command_palette.rs` unless another
file is named. "Op" is the `EditorCore` op or core-level
fn the shim ultimately calls.

### Transport

| # | Key | Shim target | Verdict | MCP tool / owner | Note |
|---|---|---|---|---|---|
| 1 | `play` | core fields `playing`/`playhead` (shim :308) | COVERED-EQ | `set_playback` action `toggle` (built-in, `mcp.rs:1605`) | The tool flips the same `core.playing` flag; the shim additionally syncs `playhead = current` when starting — the tool does not. B4's shell transport supersedes this for the live GUI. |
| 2 | `prev` | `core.goto((current + n - 1) % n)` (:317) | COVERED | `set_frame` (built-in; handler calls `core.goto`, `mcp.rs:398-424`) | The shim wraps at frame 0; `set_frame` refuses out-of-range, so the client passes the wrapped index it computes from `get_state`. Same op reached. |
| 3 | `next` | `core.goto((current + 1) % n)` (:326) | COVERED | `set_frame` | As row 2. |
| 4 | `first-frame` | `core.goto(0)` (:334) | COVERED | `set_frame` with `frame: 0` | — |
| 5 | `restart-playback` | `core.goto(0)` + `playing = true` (:464) | COVERED | `set_playback` action `restart` (`mcp.rs:1639-1642`) | Byte-identical composition (goto, then the flag). |
| 6 | `toggle-loop-playback` | `s.loop_playback` flip (:519) | SHELL → B4 | Squad B transport tools | Shell `State` field only; no core state is touched. |

### Frames

| # | Key | Shim target | Verdict | MCP tool / owner | Note |
|---|---|---|---|---|---|
| 7 | `add-frame` | `core.add_frame()` (:1097) | COVERED | `add_frame` (`mcp_tools.rs:440`) | — |
| 8 | `dup-frame` | `core.duplicate_frame()` | COVERED | `duplicate_frame` (built-in; `mcp.rs:595`) | — |
| 9 | `del-frame` | `core.delete_frame()` (:1105) | COVERED | `delete_frame` (`mcp_tools.rs:463-474`: goto + `core.delete_frame()`) | Tool takes an explicit frame and restores the playhead — a superset of the palette's current-frame form. |
| 10 | `inbetween` | `core.insert_inbetween()` (:1109) | COVERED | `insert_inbetween` (`mcp_tools.rs:455`) | — |
| 11 | `copy-frame` | `core.copy_frame()` (:1113) | COVERED | `copy_frame` (built-in; `mcp.rs:644`) | — |
| 12 | `paste-frame` | `core.paste_frame()` (:1117) | COVERED | `paste_frame` (built-in; `mcp.rs:657`) | — |
| 13 | `fps-up` | `core.adjust_fps(1.0)` (:341) | COVERED-EQ | `set_project_fps` (`mcp_tools.rs:1733`) | The stepper (`editor.rs:2229`) only marks dirty; the deliberate setter snapshots. Reach the step as `get_state` fps + 1 → `set_project_fps`. Undo semantics differ, state does not. |
| 14 | `fps-down` | `core.adjust_fps(-1.0)` (:346) | COVERED-EQ | `set_project_fps` | As row 13. |

### Edit

| # | Key | Shim target | Verdict | MCP tool / owner | Note |
|---|---|---|---|---|---|
| 15 | `undo` | `core.undo()` (:1121) | COVERED | `undo` (`mcp_tools.rs:1916`) | The shim also clears `s.stroke_selected` — a shell selection side effect for B6 to mirror, not a core gap. |
| 16 | `redo` | `core.redo()` (:1126) | COVERED | `redo` (`mcp_tools.rs:1921`) | As row 15. |

### Projects

| # | Key | Shim target | Verdict | MCP tool / owner | Note |
|---|---|---|---|---|---|
| 17 | `save` | `core.save()` (:1131) — writes the session save path | COVERED-EQ | `save_project` (io, `mcp_io.rs:229`) | Same serialisation to an explicit path; the client passes the session's path. Not the named-project flow — that is row 19's tool. |
| 18 | `load` | `core.load()` (:1135) — reads the session save path | COVERED-EQ | `load_project` (io, `mcp_io.rs:260`) | As row 17. Project-swap undo semantics are a standing exclusion and are not touched by either side. |
| 19 | `save-named` | `core.save_named_project()` (:1139) | COVERED | `save_named_project` (built-in; `mcp.rs:1186`) | — |
| 20 | `cycle-project` | `core.cycle_project_file()` (:1143; `editor.rs:4787`) | GAP → A8 | — | The op only advances `core.project_index` through the cached `./projects` list and reports the name — no tool reads or moves that selector (`list_projects` lists files, not the selector). A8 owns project housekeeping. |
| 21 | `open-project` | `core.open_project_file()` (:1147; `editor.rs:4802`) | GAP → A8 | — | Opens `project_files[project_index]` — the cycled selection, not a path argument. `load_project` reaches the same end state only when the client already knows the path; the selector-driven open itself is untooled. A8 owns it (pairs with row 20). |
| 22 | `save-copy` | `project_tools::save_copy_pressed` → `projects::save_copy` (`project_tools.rs:983`) | COVERED | `save_project_copy` (`mcp_tools.rs:3068`, calls `projects::save_copy` with the same derived `"{name} copy"`) | Create-new semantics match the panel press exactly, including the second-press refusal. |

### Figures

| # | Key | Shim target | Verdict | MCP tool / owner | Note |
|---|---|---|---|---|---|
| 23 | `add-figure` | `core.add_figure()` (:1151) | COVERED | `add_figure` (`mcp_tools.rs:322-325`, `kind: "humanoid"`) | One tool covers rows 23-24 via its `kind` argument. |
| 24 | `add-quadruped` | `core.add_quadruped()` (:1155) | COVERED | `add_figure` with `kind: "quadruped"` (`mcp_tools.rs:326`) | — |
| 25 | `del-figure` | `core.delete_figure()` (:1159) | COVERED | `delete_figure` (built-in; `mcp.rs:999`) | Tool takes an explicit figure index; the palette deletes the selected figure. |
| 26 | `duplicate-figure` | `core.duplicate_figure(selected)` (:430) | COVERED | `figure_duplicate` (`mcp_tools.rs:390`) | — |
| 27 | `mirror-pose` | `core.mirror_selected_pose()` (:1167) | COVERED | `mirror_pose` (`mcp_tools.rs:1927`) | — |
| 28 | `hide-figure` | `core.toggle_figure_hidden()` (:1163) | COVERED-EQ | `layers` (`mcp_tools.rs:1862`, `hidden` arg) | **A2 splice pending:** the dedicated relative-form handler `tool_figure_toggle_hidden` exists and is tested at `figure_ops.rs:1401` (spec comment :1384) but was never registered. `layers` sets the flag absolutely — toggle = read `get_figure`, then set. |
| 29 | `solo-figure` | `core.toggle_solo(selected)` (:351) | COVERED-EQ | `layers` (`solo` arg, `mcp_tools.rs:1896-1902`) | **A2 splice pending:** `tool_figure_toggle_solo` at `figure_ops.rs:1466`, unregistered. Solo is session state (`core.solo`), which `layers` writes in its absolute form. |
| 30 | `lock-figure` | `core.toggle_figure_lock(selected)` (:357) | COVERED-EQ | `layers` (`locked` arg) | **A2 splice pending:** `tool_figure_toggle_lock` at `figure_ops.rs:1434`, unregistered. |
| 31 | `insert-library-figure` | `core.insert_library_figure()` (:1171) | COVERED | `figure_library_insert` (`figure_ops.rs:1509-1528`) | Tool resolves `index`/`name` into `core.library_index` first, then calls the same op. |
| 32 | `mirror-figure` | `core.mirror_figure_all_frames(selected)` (:648) | COVERED | `mirror_figure` (`mcp_tools.rs:1601`) | Phase J row. |

### Segments

| # | Key | Shim target | Verdict | MCP tool / owner | Note |
|---|---|---|---|---|---|
| 33 | `add-segment` | `core.add_segment()` (:1175) | COVERED | `add_node` (`mcp_tools.rs:811`) | The registry names it for the node it adds; the handler calls the same op. |
| 34 | `del-segment` | `core.delete_segment()` (:1179) | COVERED | `delete_node` (`mcp_tools.rs:828`) | — |
| 35 | `len-up` | `core.adjust_length(4.0)` (:363) | COVERED-EQ | `set_node` (`length`, `mcp_tools.rs:666`) with `get_node` | The adjust op (`editor.rs:731`) writes `node.length` clamped 4..220; `set_node` clamps identically (4..220). Step = read, +4, set. No tool calls `adjust_length`. |
| 36 | `len-down` | `core.adjust_length(-4.0)` (:368) | COVERED-EQ | `set_node` / `get_node` | As row 35. |
| 37 | `thick-up` | `core.adjust_thickness(1.0)` (:373) | COVERED-EQ | `set_node` (`thickness`) with `get_node` | Clamp ranges match (1..18 both sides). |
| 38 | `thick-down` | `core.adjust_thickness(-1.0)` (:378) | COVERED-EQ | `set_node` / `get_node` | As row 37. |
| 39 | `cycle-shape` | `core.cycle_shape()` (:1183; `editor.rs:779`) | COVERED-EQ | `set_node` (`shape`) with `get_node` | The cycle order (Line → Circle → Polygon 3..6 → Line) is fully enumerable from the `get_node` read; each step is one `set_node`. No tool calls `cycle_shape`. |
| 40 | `cycle-bend` | `core.cycle_bend()` (:1187; `editor.rs:1456`) | COVERED-EQ | `set_bend` (`mcp_tools.rs:2746`) with a bend read (`get_pose`) | The op steps the current frame's bend through [0, .35, .7, −.35, −.7]; `set_bend` writes any of those values at any frame for any figure/node — a superset. |
| 41 | `cycle-node-color` | `core.cycle_node_color()` (:1191; `editor.rs:797`) | COVERED-EQ | `set_node` (`color`) with `get_node` | The op steps through the fixed `NODE_COLORS` table; `set_node` sets any colour, including each table entry in turn. |
| 42 | `attach-media-import` | `core.attach_image_to_node()` (:1037) | COVERED | `attach_image_to_node` (`mcp_tools.rs:3162`) | Same op, including its import-folder scan — not a dialog. Squad C's clip-media work extends this family; the palette row itself is covered. |
| 43 | `clear-node-media` | `core.clear_node_media()` (:1045) | COVERED | `clear_node_media` (`mcp_tools.rs:3182`) | — |
| 44 | `cycle-attached-sprite` | `inspector_segment::attach_asset(core, next_asset_target(...))` (:1055; `inspector_segment.rs:687`) | COVERED-EQ | `asset_attach` (`media.rs:710`) with `get_node` + `assets_list` | `attach_asset` snapshot-writes `node.media = Image { asset }`; the tool writes the identical field with a snapshot. The cycle arithmetic (current + 1, wrapping) is client-side from the two reads. |
| 45 | `add-sprite` | `core.add_sprite(0)` (:1072) | COVERED | `sprite_add` (Wave 13, `media.rs` handlers) | The palette always places asset 0; the tool takes any asset index. |
| 46 | `remove-sprite` | `core.remove_sprite(last)` (:1086) | COVERED | `sprite_remove` | Tool takes the index explicitly; the palette's "last" is `sprite_list`'s final row. |

### Camera

| # | Key | Shim target | Verdict | MCP tool / owner | Note |
|---|---|---|---|---|---|
| 47 | `cam-zoom-in` | `core.camera_zoom_by(1.2)` (:383; `editor.rs:855`) | COVERED-EQ | `set_camera` with `get_camera` | The op snapshots and re-keys the current frame at zoom × factor (clamp 0.3..4.0). `get_camera` reads the interpolated zoom; `set_camera` writes the multiplied value as a key at that frame. No tool calls `camera_zoom_by`. A9's camera-key remit could add the relative form; the capability exists today. |
| 48 | `cam-zoom-out` | `core.camera_zoom_by(1.0 / 1.2)` (:388) | COVERED-EQ | `set_camera` with `get_camera` | As row 47. This is camera *authoring*, not view zoom — B5's view tools must not be used for it (Wave 5 doctrine). |
| 49 | `cam-frame-all` | `core.frame_all_figures()` (:1195) | COVERED | `frame_all_figures` (built-in, `mcp.rs` all_tools) | — |
| 50 | `cam-centre` | `core.centre_on_selection()` (:1199) | COVERED | `centre_on_selection` (built-in) | — |
| 51 | `cam-clear-key` | `core.camera_clear_key()` (:1203; `editor.rs:863`) | COVERED-EQ | `delete_camera_key` (built-in; `mcp.rs:976-1001`) | Both remove the key at a frame via `project.remove_camera_key`. One honest difference: the palette op deliberately takes **no undo snapshot** (the camera panel sells it as "Clear key (no undo)", `editor.rs:865-869` — it only marks dirty), while the tool snapshots (one undo step). Removing a key from MCP is *more* recoverable, never less capable. A9's camera-key set could add a no-snapshot form if exact parity of undo behaviour is ever wanted. |
| 52 | `cam-follow` | `core.toggle_camera_follow()` (:1207; `editor.rs:2373`) | COVERED-EQ | `set_camera_follow` (built-in; `mcp.rs:1419`) | The toggle flips `project.camera_follow` between the selected figure and `None`; the tool sets either outcome directly (`figure` index or `null`). |
| 53 | `cam-follow-clear` | `camera_panel::follow_clear(core)` (:509; `camera_panel.rs:723`) | COVERED-EQ | `set_camera_follow` with `figure: null` | `follow_clear` sets `camera_follow = None` (and repairs a stale index by the same write). The null form of the tool is the same write. |

### Graph

| # | Key | Shim target | Verdict | MCP tool / owner | Note |
|---|---|---|---|---|---|
| 54 | `key-graph-angle` | `core.key_graph_angle()` (:1211) | COVERED | `graph_key_angle` (`graph_keys.rs:1356-1363`) | — |
| 55 | `key-all-channels` | `keyframe_tools::key_all_here_core(core)` (:492; `keyframe_tools.rs:1094`) | COVERED-EQ | `graph_key_all_channels` (`graph_keys.rs:1334`) | The tool's `key_all_channels_at` is the documented library form of the same composition (one snapshot; every channel keyed at its evaluated value) — `key_all_here_core` itself lives in the shell crate and cannot be called from the library, which is why the twin exists (`graph_keys.rs:1309-1317`). |
| 56 | `clear-graph` | `core.clear_graph()` (:1215) | COVERED | `graph_clear_channels` (`graph_keys.rs:1371-1388`) | — |
| 57 | `shift-channel-right` | `graph_keys::shift_keys(core, channel, 1.0)` via the graph panel's effective channel (:776) | COVERED | `graph_shift_keys` (`graph_keys.rs:1097-1100`) | Phase M row. The palette resolves the channel from shell graph-panel state; the tool takes the channel index explicitly. |
| 58 | `fill-keys-range` | `graph_keys::fill_keys_in_range(core, channel, loop a, b)` (:797) | COVERED | `graph_fill_keys` (`graph_keys.rs:1111-1115`) | Phase M row. Tool takes `from`/`to` explicitly instead of reading `core.loop_range`. |

### View (core-state group)

| # | Key | Shim target | Verdict | MCP tool / owner | Note |
|---|---|---|---|---|---|
| 59 | `onion` | `core.onion = !core.onion` (:395) | GAP → A9 | — | No tool reads or writes `core.onion` (zero hits across `mcp.rs` / `mcp_tools.rs` / `mcp_io.rs`). A core field, not shell state — so not Squad B. A9's remit names onion-skin explicitly. |
| 60 | `draw-mode` | `core.draw_mode = !core.draw_mode` (:402) | GAP → A3 | — | **Handler written, never registered:** `tool_set_draw_mode` at `draw_ops.rs:1341` (tested, `:2887`) with its Tool-entry spec in the comment at `:1326`, yet `"set_draw_mode"` appears nowhere in `mcp_tools.rs`. Splice-only fix in A3's file domain. (Its state is *readable* today — `get_editor_status` reports `draw_mode`, `mcp.rs:1643` — but not writable.) |
| 61 | `tween-play` | `core.toggle_tween_playback()` (:1219) | COVERED-EQ | `set_tween_playback` (built-in; `mcp.rs:1551`) | The tool is the idempotent set form of the same project flag ("Stored in the project. One undo step when the value actually changes."). |
| 62 | `cycle-easing` | `core.cycle_easing()` (:1223; `editor.rs:2339`) | COVERED-EQ | `set_project_easing` (`mcp_tools.rs:534`) | The cycler steps `project.easing` to `.next()` and only marks dirty; the setter names any of the four easings and snapshots. A9 owns easing defaults if the relative form is ever wanted. |

### Draw

| # | Key | Shim target | Verdict | MCP tool / owner | Note |
|---|---|---|---|---|---|
| 63 | `cycle-brush` | `core.cycle_brush()` (:1227) | COVERED | `cycle_brush` (`draw_ops.rs:1186-1187`) | — |
| 64 | `draw-tool` | `core.toggle_draw_tool()` (:1231; `editor.rs:3515`) | COVERED-EQ | `set_draw_tool` (`draw_ops.rs:1222`) | Toggle outcomes are exactly the tool's two settable values (`brush` / `eraser`) on the same `core.draw_tool` field. |
| 65 | `clear-drawings` | `core.clear_frame_drawings()` (:1235) | COVERED | `clear_frame_strokes` (`draw_ops.rs:1001-1017`) | Registry name differs from the op; the handler calls it. |
| 66 | `mirror-strokes` | `core.mirror_strokes_x()` (:1239) | COVERED | `mirror_strokes` (`draw_ops.rs:983`) | — |
| 67 | `stamp-polygon` | `core.stamp_polygon()` (:1243; `editor.rs:3456`) | GAP → A3 | — | The op stamps a pentagon (radius 60, stage centre) via `stamp_points`: the **selected figure's colour** and a per-point `widths` vec. `draw_polygon` shares only the geometry fn — it defaults to light grey `0xE8E8EC`, writes empty `widths`, and demands explicit centre/radius (`mcp_tools.rs:2286`). Not reproducible exactly; a true gap in A3's draw domain. |
| 68 | `delete-last-stroke` | `core.delete_stroke_at(last)` (:439) | COVERED | `delete_stroke` (`mcp_tools.rs:2498`) | "Last" = final index of `list_strokes`. |
| 69 | `delete-stroke` | `core.delete_stroke_at(s.stroke_selected)` (:693) | COVERED | `delete_stroke` | Phase J row. The shim also clears the shell selection afterwards — B6 mirrors that; the document op is covered. |
| 70 | `stroke-select-next` | `stroke_inspector::step_stroke_selection(s, 1)` (:528) | SHELL → B6 | Squad B selection tools | Writes only `State::stroke_selected`. |
| 71 | `stroke-select-prev` | `stroke_inspector::step_stroke_selection(s, -1)` (:534) | SHELL → B6 | Squad B selection tools | As row 70. |

### Effects

| # | Key | Shim target | Verdict | MCP tool / owner | Note |
|---|---|---|---|---|---|
| 72 | `particle-burst` | `core.add_particle(0)` (:407) | COVERED | `particle_add_anchored` (`particle_ops.rs:632-678`) | The handler maps `kind: "burst"` to index 0 in the op's own kind table and calls `core.add_particle(0)`. |
| 73 | `particle-sparkle` | `core.add_particle(1)` (:412) | COVERED | `particle_add_anchored` with `kind: "sparkle"` | As row 72. |
| 74 | `particle-add-event` | `core.particle_add_event(current, stage centre)` (:478) | COVERED | `particle_add_event` (built-in; `mcp.rs:1371`) | Tool takes frame/x/y explicitly — the palette's centre-of-stage call is `x = stage[0]/2, y = stage[1]/2` from `get_state`. |
| 75 | `fx-clip-confetti` | `core.add_fx_clip(ClipKind::Confetti)` (:417; `editor.rs:2825`) | COVERED-EQ | `add_fx_clip` (`mcp_tools.rs:913`) | The tool does not call the op; it performs the documented "same construction as the editor's own add_fx_clip op" (`mcp_tools.rs:942`) — one snapshot, same `FxClipEntry` — with explicit figure/node/start/duration where the palette uses the selected node and op defaults. |
| 76 | `fx-clip-lightning` | `core.add_fx_clip(ClipKind::Lightning)` (:422) | COVERED-EQ | `add_fx_clip` with `kind: "lightning"` | As row 75. |
| 77 | `clear-particles` | `core.clear_particles()` (:1247; `editor.rs:2431`) | COVERED | `clear_particles` (built-in; `mcp.rs:1388`) | The op clears both `project.particles` and `project.fx_clips`; the tool is its parity twin. |
| 78 | `split-fx-clip` | `core.fx_split_clip(s.fx_selected, playhead)` (:629) | COVERED | `fx_split_clip` (`mcp_tools.rs:1501`) | Phase J row. Tool takes index + frame explicitly. |
| 79 | `duplicate-particle` | `core.particle_duplicate_event(s.particle_selected)` (:672) | COVERED | `particle_duplicate_event` (`mcp_tools.rs:1520`) | Phase J row. The shim also moves the shell selection to the copy — B6 mirrors that; the op is covered. |
| 80 | `delete-particle` | `core.particle_delete_event(s.particle_selected)` (:710) | COVERED | `particle_delete_event` (`particle_ops.rs:509-511`) | Phase J row. Selection-clearing side effect as row 69. |

### Audio

| # | Key | Shim target | Verdict | MCP tool / owner | Note |
|---|---|---|---|---|---|
| 81 | `add-audio` | `core.add_audio_clip()` (:1251; `editor.rs:3528`) | COVERED-EQ | `add_audio_clip` (`mcp_tools.rs:1237`) | The op loads the **first** audio file found in `./audio`; the tool requires an explicit `path` (and, since Wave 13, refuses nonexistent/non-WAV files). Passing that same first file reproduces the command exactly; the implicit-scan form itself is a folder convention, not a dialog. |
| 82 | `mixdown` | `core.mixdown_audio()` (:1255) | COVERED | `audio_mixdown` (built-in; `mcp.rs:1289`) | — |
| 83 | `audio-echo` | `core.audio_echo_last()` (:1259) | COVERED-EQ | `audio_echo` (`mcp_tools.rs:1399-1401`) | Palette targets the last clip via the `_last` wrapper; the tool calls the per-index `audio_echo_at(idx)` the wrapper itself resolves to. Last index from `audio_list_clips`. |
| 84 | `audio-lowpass` | `core.audio_lowpass_last()` (:1263) | COVERED-EQ | `audio_lowpass` (`mcp_tools.rs:1405-1407`) | As row 83 (`audio_lowpass_at`). |
| 85 | `audio-trim-silence` | `core.audio_trim_silence_last()` (:1267) | COVERED-EQ | `audio_trim_silence` (`mcp_tools.rs:1411-1413`) | As row 83 (`audio_trim_silence_at`). |
| 86 | `audio-split-silences` | `core.audio_split_at_silences_last()` (:1271) | COVERED-EQ | `audio_split_silences` (`mcp_tools.rs:1429-1431`) | As row 83 (`audio_split_silences_at`). |
| 87 | `fade-audio` | `core.audio_fade_last()` (:1275) | COVERED | `audio_fade` (`mcp_tools.rs:1327-1329`) | The tool calls the `_last` op itself. |
| 88 | `reverse-audio` | `core.audio_reverse_last()` (:1279) | COVERED | `audio_reverse` (`mcp_tools.rs:1333-1335`) | As row 87. |
| 89 | `delete-audio-clip` | `core.delete_audio_clip(last)` (:449) | COVERED | `audio_delete_clip` (`mcp_tools.rs:1380`) | Tool takes the index; "last" from `audio_list_clips`. |
| 90 | `normalise-audio` | `core.normalise_all_clips(0.9)` (:660) | COVERED | `audio_normalise_all` (`mcp_tools.rs:1589`) | Same op; the palette's fixed 0.9 peak is the tool's `target_peak: 0.9`. |
| 91 | `mute-audio-clip` | `audio_ops::audio_toggle_muted_at(core, audio_panel target)` (:740) | COVERED | `audio_clip_toggle_muted` (`audio_ops.rs:1393-1398`) | Phase M row. The palette resolves its target from shell audio-panel state; the tool takes the clip index. |

### Export

The palette export ops write to fixed paths under
`./export/` (`editor.rs:2992-3102`, `4833-4895`); the io
tools take an explicit path and run the same exporters.
Every row is therefore COVERED-EQ: identical output,
caller-chosen destination.

| # | Key | Shim target | Verdict | MCP tool / owner | Note |
|---|---|---|---|---|---|
| 92 | `export-png` | `core.export_png()` (:1283) — PNG sequence via `export_png_sequence` | COVERED-EQ | `render` (io) with `format: "png_sequence"` (enum at `mcp_io.rs:974`) | — |
| 93 | `export-gif` | `core.export_gif()` (:1287) | COVERED-EQ | `render` with `format: "gif"` | — |
| 94 | `export-mp4` | `core.export_mp4()` (:1291) | COVERED-EQ | `render` with `format: "mp4"` | Range exports are the separate `mp4_range` form of `export_file`. |
| 95 | `export-webm` | `core.export_webm()` (:1295) | COVERED-EQ | `render` with `format: "webm"` | — |
| 96 | `export-piv` | `core.export_piv()` (:1299) — `pivot::export_piv_v3` to `./export/animation.piv` | COVERED-EQ | `export_file` (io) with `format: "piv3"` (enum at `mcp_io.rs:958`) | The op is specifically the v3 writer; `piv3` is its format key. |
| 97 | `export-stk` | `core.export_stk()` (:1303) — selected figure to `./export/<name>.stk` | COVERED-EQ | `export_file` with `format: "stk"` | — |

### 3D

| # | Key | Shim target | Verdict | MCP tool / owner | Note |
|---|---|---|---|---|---|
| 98 | `enter-3d` | `core.enter_3d()` (:1307) | COVERED | `fig3_enter` (`mcp_tools.rs:2547-2548`) | — |
| 99 | `exit-3d` | `core.exit_3d()` (:1311) | COVERED | `fig3_exit` (`fig3d.rs:2061-2063`) | — |

### Workspaces (Phase I)

All eight call `workspaces::select_workspace(s, ...)`
(:551-595) — a pure shell `State` write (with the chip
press's own persistence). No core state is touched.

| # | Key | Shim target | Verdict | MCP tool / owner | Note |
|---|---|---|---|---|---|
| 100 | `go-animate` | `select_workspace(Animate)` | SHELL → B3 | `set_workspace` (B3) | — |
| 101 | `go-draw` | `select_workspace(Draw)` | SHELL → B3 | `set_workspace` | — |
| 102 | `go-graph` | `select_workspace(Graph)` | SHELL → B3 | `set_workspace` | — |
| 103 | `go-effects` | `select_workspace(Effects)` | SHELL → B3 | `set_workspace` | — |
| 104 | `go-audio` | `select_workspace(Audio)` | SHELL → B3 | `set_workspace` | — |
| 105 | `go-build3d` | `select_workspace(Build3D)` | SHELL → B3 | `set_workspace` | — |
| 106 | `go-export` | `select_workspace(Export)` | SHELL → B3 | `set_workspace` | — |
| 107 | `go-home` | `select_workspace(Home)` | SHELL → B3 | `set_workspace` | — |

### View toggles (Phase I + Wave 9)

| # | Key | Shim target | Verdict | MCP tool / owner | Note |
|---|---|---|---|---|---|
| 108 | `toggle-guides` | `settings_popover::toggle_guides` — `s.guides` flip (`settings_popover.rs:488`) | SHELL → B7/B8 | Squad B theme/settings + panel tools | Direct fn-pointer entry (no shim). Pure shell flag. |
| 109 | `toggle-frustum` | `settings_popover::toggle_guides_frustum` — `s.guides_frustum` flip (`settings_popover.rs:499`) | SHELL → B7/B8 | Squad B | As row 108. |
| 110 | `toggle-safe-frame` | `settings_popover::toggle_guides_safe` — `s.guides_safe` flip (`settings_popover.rs:507`) | SHELL → B7/B8 | Squad B | As row 108. |
| 111 | `toggle-left-panel` | `s.show_left` flip (:600) | SHELL → B8 | Squad B panel tools | — |
| 112 | `toggle-right-panel` | `s.show_right` flip (:606) | SHELL → B8 | Squad B panel tools | — |
| 113 | `toggle-live-mcp` | `settings_popover::toggle_live_mcp` — `live_mcp::set_enabled(!is_enabled())` (`settings_popover.rs:647`) | EXCLUSION-CANDIDATE | — (fallback: B7) | **Argument:** a tool that switches off the very endpoint serving it strands its own session mid-conversation, and over stdio the toggle is meaningless (no listener exists) — the one palette action whose target is the MCP transport itself rather than the editor. If the integrator disagrees, it lands naturally as a B7 settings tool with B1's honest-refusal treatment for stdio. |
| 114 | `theme-light` | `settings_popover::set_theme(core, true)` (:824; `settings_popover.rs:393`) | SHELL → B7 | Squad B theme tools | Storage is `core.light_theme`, but the target is the settings fn and B7's remit names the theme toggle explicitly — Squad B owns it. |
| 115 | `theme-dark` | `settings_popover::set_theme(core, false)` (:831) | SHELL → B7 | Squad B theme tools | As row 114. |

### Frames, continued (Phase M)

| # | Key | Shim target | Verdict | MCP tool / owner | Note |
|---|---|---|---|---|---|
| 116 | `swap-frame-prev` | `frame_swap::swap_frame_with_neighbour(core, -1)` (:758) | COVERED | `swap_frame_with_neighbour` (`frame_swap.rs:432-438`) | — |
| 117 | `swap-frame-next` | `frame_swap::swap_frame_with_neighbour(core, 1)` (:764) | COVERED | `swap_frame_with_neighbour` | — |

### Style (Wave 12)

| # | Key | Shim target | Verdict | MCP tool / owner | Note |
|---|---|---|---|---|---|
| 118 | `open-style-inspector` | `select_workspace(Animate)` + `s.show_right = true` (:908) | SHELL → B8 | Squad B panel tools | Pure view navigation; no document state. B8's panel-visibility tools subsume it (workspace + right column). |
| 119 | `style-preset-ink` | `inspector_style::apply_preset(core, STYLE_PRESETS[0])` via `apply_style_preset_at` (:932, :925) | COVERED-EQ | `apply_style_preset` (`style.rs:456`) | `apply_preset` itself is `preset_application` + `core.set_node_style_atomic(...)` (`inspector_style.rs:560-565`); the tool performs the identical atomic write from the same canonical table. The one residue: `apply_preset` also records the colour in the panel's recent-colours list — panel-local state, not document state. |
| 120 | `style-preset-crimson` | `apply_preset(STYLE_PRESETS[1])` (:936) | COVERED-EQ | `apply_style_preset` with `preset: "Crimson"` | As row 119. |
| 121 | `style-preset-ocean` | `apply_preset(STYLE_PRESETS[2])` (:940) | COVERED-EQ | `apply_style_preset` with `preset: "Ocean"` | As row 119. |
| 122 | `style-preset-forest` | `apply_preset(STYLE_PRESETS[3])` (:944) | COVERED-EQ | `apply_style_preset` with `preset: "Forest"` | As row 119. |
| 123 | `style-preset-violet` | `apply_preset(STYLE_PRESETS[4])` (:948) | COVERED-EQ | `apply_style_preset` with `preset: "Violet"` | As row 119. |
| 124 | `style-preset-amber` | `apply_preset(STYLE_PRESETS[5])` (:952) | COVERED-EQ | `apply_style_preset` with `preset: "Amber"` | As row 119. |
| 125 | `style-cap-round` | `core.set_node_cap(CapKind::Round)` (:962) | COVERED | `set_node_cap` (`mcp_tools.rs:745`) | — |
| 126 | `style-cap-square` | `core.set_node_cap(CapKind::Square)` (:969) | COVERED | `set_node_cap` with `cap: "square"` | — |
| 127 | `style-cap-point` | `core.set_node_cap(CapKind::Point)` (:976) | COVERED | `set_node_cap` with `cap: "point"` | — |
| 128 | `cycle-taper` | `core.cycle_node_taper()` (:984) | COVERED | `cycle_node_taper` (`style.rs:360-366`) | The tool selects the addressed node, then calls the very op. |
| 129 | `style-opacity-up` | `inspector_style::step_opacity(core, 1)` via `step_opacity_guarded` (:1005, :995) | COVERED-EQ | `set_node_opacity` (`mcp_tools.rs:782`) with `get_node_style` | `step_opacity` is `set_node_opacity(opacity_stepped(current, dir))` (`inspector_style.rs:328-332`) — the ±0.05 step is client arithmetic on the read value; the write op is the same. |
| 130 | `style-opacity-down` | `inspector_style::step_opacity(core, -1)` (:1011) | COVERED-EQ | `set_node_opacity` with `get_node_style` | As row 129. |
| 131 | `copy-style` | `inspector_style::copy_style(core)` (:1019; `inspector_style.rs:574`) | GAP → INTEGRATOR-OWNED | — | Captures the selected node's style into the panel-local clipboard (`store_clipboard`, a `thread_local` — `inspector_style.rs:486-492`). No tool can hold or set that clipboard: it lives in the shell process's panel module, outside `EditorCore`, and no Wave 14 squad owns the style domain. An MCP client can *emulate* a copy with `get_node_style`, but the clipboard itself — and therefore row 132's palette-faithful paste — is unreachable. |
| 132 | `paste-style` | `inspector_style::paste_style(core)` (:1029; `inspector_style.rs:590`) | GAP → INTEGRATOR-OWNED | — | Applies the clipboard via the same atomic setter the tools use, so the *document effect* is emulatable (`get_node_style` on the source → `set_node_style` on the target); the command as such depends on row 131's untooled clipboard. The pair is one integrator item: expose the style clipboard (or copy/paste as a pair of tools) in `style.rs`/`inspector_style.rs` terms. |

### Transport, continued (Wave 10)

| # | Key | Shim target | Verdict | MCP tool / owner | Note |
|---|---|---|---|---|---|
| 133 | `prev-keyframe` | `playback_controls::prev_key(current, stats::figure_keyframes(project, selected))` → `core.goto(frame)` (:852) | COVERED | `goto_prev_keyframe` (built-in; `mcp.rs:1803`) | Same keyframe basis (the selected figure's pose-change frames, `stats::figure_keyframes`); the tool takes the figure index explicitly. Not to be confused with `goto_prev_key` — the camera/graph key union — which is a different jump. |
| 134 | `next-keyframe` | `playback_controls::next_key(...)` → `core.goto(frame)` (:867) | COVERED | `goto_next_keyframe` (built-in; `mcp.rs:1774`) | As row 133. |

## Integrator-owned gap list (the splice queue from this ledger)

1. **Style clipboard pair** (rows 131-132): `copy_style` /
   `paste_style` — no squad domain; needs a tool pair or a
   core-level clipboard. Handler home: `src/style.rs`
   (which already hosts the style tool handlers).
2. **Registry splices for handlers that already exist**
   (not gaps in capability, but the integrator's splice
   step must not lose them): A2's three figure toggles
   (`figure_ops.rs:1401/1434/1466`, rows 28-30 — EQ-covered
   meanwhile) and A3's `set_draw_mode`
   (`draw_ops.rs:1341`, row 60 — a true gap until spliced).

## Method note

Every COVERED verdict was verified by reading the tool
handler's body and finding the op call (or, for the
built-ins, the closure in `src/mcp.rs`); a tool merely
*sharing a name* with a command was never counted —
`add_node`/`delete_node` (rows 33-34), `clear_frame_strokes`
(row 65) and `audio_delete_clip` (row 89) are the rows
where the registry name and the op name differ, and each
was confirmed at the call site. EQ verdicts name the
exact read tool a client needs to compute the palette's
implicit argument (selected / last / current), so A1's
census can re-derive every row mechanically.
