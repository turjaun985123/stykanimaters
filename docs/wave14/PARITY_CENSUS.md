# Wave 14 — MCP parity census (A1)

The wave ledger: every capability the editor exposes, and whether an MCP
tool reaches it today. Every later Wave 14 claim traces to a row here.

**Snapshot.** Taken against the working tree on 2026-11-09 (~04:50 +06),
mid-wave: sibling agents' handler files are present but unregistered, and
the tree was still moving (editor.rs, draw_ops.rs, fx_ops.rs, media.rs,
projects.rs, mcp.rs modified; src/shell_tools_*.rs untracked and not yet
declared in lib.rs). Counts must be re-derived at integration (E2's gate
does exactly that); the classifications below are grep-verified against
this snapshot.

**Method.** Registered tools were extracted from the `Tool` literals in
src/mcp.rs (44 built-ins), src/mcp_tools.rs (235 registry) and
src/mcp_io.rs (9 io) = **288 tools**, matching the entering census.
A capability counts as COVERED only when a call graph from a registered
handler reaches it (handler bodies followed transitively through
src/*.rs production code, test modules excluded). Name-collision false
positives were hunted and removed (generic names and the rune_sec UI
tree are excluded from the graph). Every non-covered row was then read
by hand and classified EQUIV / HANDLER-READY / GAP / SHELL / INTERNAL /
DIALOG / EXCLUDED with its evidence.

**Status legend.**
- `COVERED` — a registered tool's handler reaches the fn.
- `EQUIV` — not reached literally, but a registered tool delivers the
  same capability in explicit-value form (a setter for a cycler/stepper,
  an index form for a *_last form, a format name for a fixed-path export).
- `HANDLER-READY` — a `tool_*` handler (or B7 `exec_shell_*` executor)
  exists in a domain file but has no Tool entry yet: integrator splice,
  not new design work.
- `HANDLER` — the row is itself a registered MCP handler fn.
- `GAP` — no tool, no equivalent, no handler. The owning squad is named.
- `SHELL` — shell-side state or machinery (State fields, transport tick,
  playback audio engine); Squad B's remit, not a core-op gap.
- `INTERNAL` — helper/read-model/render fn; not a user capability.
- `DIALOG` — bound to a native file dialog; B9 owes an honest refusal.
- `EXCLUDED` — charter exclusion (standing user decision); do not touch.

**Headline totals.**

| Family | Counted | Covered (incl. EQUIV + registered handlers) | Handler-ready | Gap rows | Shell | Internal/Dialog/Excluded |
|---|---|---|---|---|---|---|
| 1. Core & domain pub fns | 543 | 449 | 55 | 3 | 13 | 23 |
| 2. Palette commands | 134 | 106 | 24 | 4 | — | — |
| 3. Panel actions | 87 | 53 | 23 | 8 | 3 | — |
| **Total** | **764** | **608** | **102** | **15** | **16** | **23** |

Family 1 arithmetic: 543 = COVERED 297 + HANDLER 104 + EQUIV 48 +
HANDLER-READY 55 + GAP 3 + SHELL 13 + INTERNAL 20 + DIALOG 2 + EXCLUDED 1.
"Covered" above folds HANDLER rows (they are the tool surface itself) and
EQUIV rows into the 449. Family 3 counts the curated action rows in §3.

**The true gaps — 15 rows over 7 distinct mechanisms** (the wave's real work list):

| # | Capability | Where | Owner |
|---|---|---|---|
| G1 | cycle_project_file | src/editor.rs (Family 1) + palette cycle-project | B9 |
| G2 | open_project_file (recent/Home open) | src/editor.rs + palette open-project + Home/project panels | B9 |
| G3 | cycle_onion_frames (onion depth stepping) | src/editor.rs | A9 |
| G4 | stroke-select-next / stroke-select-prev | palette + stroke_inspector::step_stroke_selection | B6 |
| G5 | figure-library entry rename (rename_saved) | src/rune_sec/library_panel.rs | A8 |
| G6 | graph channel selection state (graph_panel) | src/rune_sec/graph_panel.rs | B6 |
| G7 | style recent-colours list (panel-local) | src/rune_sec/inspector_style.rs | B8 |

(G1+G2 appear in two families each; G4 is two
palette rows over one mechanism. Distinct mechanisms: 7.)

---

## Family 1 — EditorCore methods & domain pub fns

Every `pub fn` in src/editor.rs, src/*_ops.rs, src/projects.rs,
src/media.rs, src/sprite_ops.rs, src/fig3d.rs, src/graph_keys.rs, plus
the adjacent domain files those ops live with (fx_edit.rs, audio_edit.rs,
audio_play.rs, graph.rs, frame_swap.rs). 543 fns.

### src/editor.rs

| Capability | Where | MCP tool / status | Owner |
|---|---|---|---|
| `new` | src/editor.rs:166 | INTERNAL — Constructor; project creation is the `new_project` tool. | — |
| `frame_count` | src/editor.rs:249 | COVERED (reached from 16 registered tools) | — |
| `positions` | src/editor.rs:254 | COVERED (reached from 7 registered tools) | — |
| `snapshot` | src/editor.rs:259 | COVERED (reached from 185 registered tools) | — |
| `redo` | src/editor.rs:272 | COVERED (reached from 6 registered tools) | — |
| `undo` | src/editor.rs:298 | COVERED (reached from 6 registered tools) | — |
| `undo_depth` | src/editor.rs:343 | COVERED (reached from 10 registered tools) | — |
| `redo_depth` | src/editor.rs:348 | COVERED (reached from 10 registered tools) | — |
| `hit_test` | src/editor.rs:353 | COVERED (reached from 5 registered tools) | — |
| `begin_drag` | src/editor.rs:366 | COVERED (reached from 5 registered tools) | — |
| `begin_camera_drag` | src/editor.rs:399 | COVERED (reached from 5 registered tools) | — |
| `drag_to` | src/editor.rs:405 | COVERED (reached from 5 registered tools) | — |
| `end_drag` | src/editor.rs:436 | COVERED (reached from 5 registered tools) | — |
| `add_frame` | src/editor.rs:441 | COVERED (reached from 11 registered tools) | — |
| `duplicate_frame` | src/editor.rs:477 | COVERED (reached from 10 registered tools) | — |
| `delete_frame` | src/editor.rs:482 | COVERED (reached from 6 registered tools) | — |
| `delete_keyframe` | src/editor.rs:520 | COVERED (reached from 5 registered tools) | — |
| `next_keyframe` | src/editor.rs:562 | COVERED (reached from 5 registered tools) | — |
| `prev_keyframe` | src/editor.rs:570 | COVERED (reached from 5 registered tools) | — |
| `goto_next_keyframe` | src/editor.rs:580 | COVERED (reached from 5 registered tools) | — |
| `goto_prev_keyframe` | src/editor.rs:597 | COVERED (reached from 5 registered tools) | — |
| `add_figure` | src/editor.rs:611 | COVERED (reached from 1 registered tool) | — |
| `insert_inbetween` | src/editor.rs:623 | COVERED (reached from 1 registered tool) | — |
| `add_segment` | src/editor.rs:647 | COVERED (reached from 1 registered tool) | — |
| `delete_segment` | src/editor.rs:687 | COVERED (reached from 1 registered tool) | — |
| `adjust_length` | src/editor.rs:731 | COVERED (reached from 1 registered tool) | — |
| `adjust_thickness` | src/editor.rs:751 | EQUIV — `set_node` — Explicit thickness arg (clamp 1..=18, same as the op). | — |
| `cycle_shape` | src/editor.rs:779 | EQUIV — `set_node` — Explicit shape arg incl. polygon side count. | — |
| `cycle_node_color` | src/editor.rs:797 | EQUIV — `set_node / set_node_style` — Cycle replaced by explicit colour write. | — |
| `set_node_color` | src/editor.rs:820 | EQUIV — `set_node_style` — Writes colour in one undo step. | — |
| `camera_here` | src/editor.rs:851 | COVERED (reached from 12 registered tools) | — |
| `camera_zoom_by` | src/editor.rs:855 | HANDLER-READY — Reached by unregistered handler(s): tool_camera_zoom_by. | A9 |
| `camera_clear_key` | src/editor.rs:863 | HANDLER-READY — Reached by unregistered handler(s): tool_clear_camera_key. | A9 |
| `frame_all_figures` | src/editor.rs:908 | COVERED (reached from 5 registered tools) | — |
| `centre_on_selection` | src/editor.rs:955 | COVERED (reached from 5 registered tools) | — |
| `add_quadruped` | src/editor.rs:972 | COVERED (reached from 1 registered tool) | — |
| `add_simple` | src/editor.rs:984 | COVERED (reached from 1 registered tool) | — |
| `goto` | src/editor.rs:996 | COVERED (reached from 17 registered tools) | — |
| `advance` | src/editor.rs:1003 | SHELL — Per-tick transport advance; driven by the shell poll, not a tool surface. | B4 |
| `advance_play` | src/editor.rs:1013 | SHELL — Per-tick playback advance incl. audio sync hooks. | B4 |
| `display_frame` | src/editor.rs:1049 | COVERED (reached from 8 registered tools) | — |
| `set_loop_range` | src/editor.rs:1064 | COVERED (reached from 5 registered tools) | — |
| `clear_loop_range` | src/editor.rs:1078 | COVERED (reached from 5 registered tools) | — |
| `shift_frames` | src/editor.rs:1117 | COVERED (reached from 4 registered tools) | — |
| `reverse_range` | src/editor.rs:1160 | COVERED (reached from 5 registered tools) | — |
| `delete_range` | src/editor.rs:1228 | COVERED (reached from 4 registered tools) | — |
| `copy_range` | src/editor.rs:1254 | COVERED (reached from 5 registered tools) | — |
| `paste_range` | src/editor.rs:1291 | COVERED (reached from 5 registered tools) | — |
| `nudge_path_point` | src/editor.rs:1333 | COVERED (reached from 2 registered tools) | — |
| `smooth_path_roots` | src/editor.rs:1361 | COVERED (reached from 5 registered tools) | — |
| `positions_f` | src/editor.rs:1399 | COVERED (reached from 8 registered tools) | — |
| `move_figure_full` | src/editor.rs:1415 | COVERED (reached from 5 registered tools) | — |
| `rotate_camera` | src/editor.rs:1439 | COVERED (reached from 5 registered tools) | — |
| `cycle_bend` | src/editor.rs:1456 | EQUIV — `set_bend` — Explicit bend write. | — |
| `toggle_figure_lock` | src/editor.rs:1506 | HANDLER-READY — Reached by unregistered handler(s): tool_figure_toggle_lock. | A2 |
| `cycle_figure_opacity` | src/editor.rs:1519 | COVERED (reached from 1 registered tool) | — |
| `step_figure_opacity` | src/editor.rs:1538 | HANDLER-READY — Reached by unregistered handler(s): tool_figure_step_opacity. | A2 |
| `toggle_solo` | src/editor.rs:1556 | HANDLER-READY — Reached by unregistered handler(s): tool_figure_toggle_solo. | A2 |
| `key_graph_angle` | src/editor.rs:1568 | COVERED (reached from 1 registered tool) | — |
| `clear_graph` | src/editor.rs:1598 | COVERED (reached from 1 registered tool) | — |
| `smooth_graph_tangents` | src/editor.rs:1618 | COVERED (reached from 1 registered tool) | — |
| `apply_graph_ease_preset` | src/editor.rs:1641 | COVERED (reached from 1 registered tool) | — |
| `set_graph_key_interp` | src/editor.rs:1683 | HANDLER-READY — Reached by unregistered handler(s): tool_graph_set_interp_at_playhead. | A4 |
| `bake_graph_channels` | src/editor.rs:1738 | COVERED (reached from 1 registered tool) | — |
| `mirror_selected_pose` | src/editor.rs:1783 | COVERED (reached from 1 registered tool) | — |
| `copy_pose_between` | src/editor.rs:1816 | COVERED (reached from 1 registered tool) | — |
| `open_via_dialog` | src/editor.rs:1869 | DIALOG — Native file dialog; no tool may open one. B9 named refusal. | B9 |
| `save_via_dialog` | src/editor.rs:1901 | DIALOG — Native file dialog; save_project/save_named_project cover path-explicit saves. | B9 |
| `save_path` | src/editor.rs:1934 | COVERED (reached from 9 registered tools) | — |
| `save` | src/editor.rs:1940 | COVERED (reached from 5 registered tools) | — |
| `load` | src/editor.rs:1951 | COVERED (reached from 4 registered tools) | — |
| `edits_since_autosave` | src/editor.rs:1983 | COVERED (reached from 10 registered tools) | — |
| `note_recovery_written` | src/editor.rs:1996 | EXCLUDED — Charter exclusion: MCP-save write_recovery behaviour is a standing user decision. | — |
| `mark_dirty` | src/editor.rs:2026 | COVERED (reached from 14 registered tools) | — |
| `maybe_autosave` | src/editor.rs:2041 | INTERNAL — Shell tick bookkeeping; not a user capability. | — |
| `delete_figure` | src/editor.rs:2053 | COVERED (reached from 5 registered tools) | — |
| `move_figure` | src/editor.rs:2089 | COVERED (reached from 5 registered tools) | — |
| `toggle_figure_hidden` | src/editor.rs:2124 | HANDLER-READY — Reached by unregistered handler(s): tool_figure_toggle_hidden. | A2 |
| `select_figure` | src/editor.rs:2137 | COVERED (reached from 10 registered tools) | — |
| `copy_frame` | src/editor.rs:2164 | COVERED (reached from 5 registered tools) | — |
| `paste_frame` | src/editor.rs:2178 | COVERED (reached from 5 registered tools) | — |
| `adjust_frame_duration` | src/editor.rs:2214 | HANDLER-READY — Reached by unregistered handler(s): tool_adjust_frame_duration. | A2 |
| `adjust_fps` | src/editor.rs:2229 | EQUIV — `set_project_fps` — Explicit fps write. | — |
| `set_project_fps` | src/editor.rs:2263 | COVERED (reached from 1 registered tool) | — |
| `set_stage_size` | src/editor.rs:2284 | COVERED (reached from 1 registered tool) | — |
| `set_head_ring_scale` | src/editor.rs:2318 | COVERED (reached from 1 registered tool) | — |
| `cycle_easing` | src/editor.rs:2339 | EQUIV — `set_project_easing` — Explicit easing write. | — |
| `set_project_easing` | src/editor.rs:2353 | COVERED (reached from 1 registered tool) | — |
| `toggle_tween_playback` | src/editor.rs:2362 | EQUIV — `set_tween_playback` — Explicit bool set supersedes the toggle. | — |
| `toggle_camera_follow` | src/editor.rs:2373 | EQUIV — `set_camera_follow` — Explicit figure/null set supersedes the toggle. | — |
| `add_particle` | src/editor.rs:2401 | COVERED (reached from 1 registered tool) | — |
| `clear_particles` | src/editor.rs:2431 | COVERED (reached from 5 registered tools) | — |
| `import_nodes_anim_file` | src/editor.rs:2440 | EQUIV — `import_file` — format stknds. | — |
| `cycle_node_cap` | src/editor.rs:2467 | EQUIV — `set_node_cap` — Explicit cap write. | — |
| `cycle_node_taper` | src/editor.rs:2478 | COVERED (reached from 1 registered tool) | — |
| `cycle_node_gradient` | src/editor.rs:2494 | EQUIV — `set_node_style` — Gradient end written via color2. | — |
| `set_node_cap` | src/editor.rs:2515 | COVERED (reached from 1 registered tool) | — |
| `set_node_color2` | src/editor.rs:2536 | COVERED (reached from 1 registered tool) | — |
| `set_node_opacity` | src/editor.rs:2562 | COVERED (reached from 1 registered tool) | — |
| `set_node_style_atomic` | src/editor.rs:2591 | COVERED (reached from 2 registered tools) | — |
| `attach_image_to_node` | src/editor.rs:2609 | COVERED (reached from 1 registered tool) | — |
| `clear_node_media` | src/editor.rs:2664 | COVERED (reached from 2 registered tools) | — |
| `mixdown_audio` | src/editor.rs:2687 | COVERED (reached from 5 registered tools) | — |
| `cycle_lang` | src/editor.rs:2726 | HANDLER-READY — Language patch in shell_tools_settings.rs:364 (B7 executor form). | B7 |
| `export_v5_file` | src/editor.rs:2735 | EQUIV — `export_file` — format piv5; the op only adds a fixed CWD path. | — |
| `export_apng_file` | src/editor.rs:2745 | EQUIV — `render` — format apng. | — |
| `export_jpeg_dir` | src/editor.rs:2752 | EQUIV — `render` — format jpeg. | — |
| `import_nodes_file` | src/editor.rs:2759 | EQUIV — `import_file` — format nodes. | — |
| `cycle_frame_easing` | src/editor.rs:2780 | EQUIV — `set_frame_easing` — Explicit easing write. | — |
| `set_frame_easing` | src/editor.rs:2803 | COVERED (reached from 5 registered tools) | — |
| `frame_easing_label` | src/editor.rs:2816 | INTERNAL — Read-only label helper; values surface via list_frames. | — |
| `add_fx_clip` | src/editor.rs:2825 | EQUIV — `add_fx_clip` — Registry tool of the same name is parameterised (mcp_tools.rs:913); this fn is the palette/panel convenience form. | — |
| `fx_save_preset` | src/editor.rs:2864 | EQUIV — `fx_preset_save` — Handler implements the same save (never-overwrite suffix rule). | — |
| `fx_apply_preset` | src/editor.rs:2903 | EQUIV — `fx_preset_add` — Loads a preset clip at a frame. | — |
| `fx_preset_names` | src/editor.rs:2924 | EQUIV — `fx_preset_list` — Read behind the list tool. | — |
| `cycle_library` | src/editor.rs:2931 | COVERED (reached from 1 registered tool) | — |
| `insert_library_figure` | src/editor.rs:2938 | COVERED (reached from 1 registered tool) | — |
| `save_figure_to_library` | src/editor.rs:2974 | COVERED (reached from 1 registered tool) | — |
| `export_gif` | src/editor.rs:2992 | COVERED (reached from 1 registered tool) | — |
| `import_piv` | src/editor.rs:3025 | COVERED (reached from 1 registered tool) | — |
| `import_stk` | src/editor.rs:3046 | COVERED (reached from 1 registered tool) | — |
| `export_piv` | src/editor.rs:3065 | EQUIV — `export_file` — The op writes piv v3; format piv3. | — |
| `export_stk` | src/editor.rs:3077 | COVERED (reached from 1 registered tool) | — |
| `export_piv2` | src/editor.rs:3090 | EQUIV — `export_file` — format piv2. | — |
| `export_piv4` | src/editor.rs:3102 | EQUIV — `export_file` — format piv4. | — |
| `export_nodes` | src/editor.rs:3114 | EQUIV — `export_file` — format nodes. | — |
| `export_stknds` | src/editor.rs:3127 | EQUIV — `export_file` — format stknds. | — |
| `begin_draw` | src/editor.rs:3141 | COVERED (reached from 5 registered tools) | — |
| `draw_to` | src/editor.rs:3213 | COVERED (reached from 5 registered tools) | — |
| `end_draw` | src/editor.rs:3248 | COVERED (reached from 5 registered tools) | — |
| `delete_stroke_at` | src/editor.rs:3282 | COVERED (reached from 1 registered tool) | — |
| `clear_frame_drawings` | src/editor.rs:3292 | COVERED (reached from 1 registered tool) | — |
| `fill_current_stroke` | src/editor.rs:3313 | COVERED (reached from 1 registered tool) | — |
| `lasso_erase` | src/editor.rs:3367 | COVERED (reached from 1 registered tool) | — |
| `mirror_strokes_x` | src/editor.rs:3400 | COVERED (reached from 1 registered tool) | — |
| `stamp_polygon` | src/editor.rs:3456 | EQUIV — `draw_polygon` — Selected-node wrapper over the draw_ops stamp the tool calls. | — |
| `stamp_star` | src/editor.rs:3465 | EQUIV — `draw_star` — As stamp_polygon. | — |
| `stamp_arrow` | src/editor.rs:3475 | EQUIV — `draw_arrow` — As stamp_polygon. | — |
| `active_brush` | src/editor.rs:3486 | COVERED (reached from 12 registered tools) | — |
| `cycle_brush` | src/editor.rs:3500 | COVERED (reached from 1 registered tool) | — |
| `draw_tool_is_eraser` | src/editor.rs:3510 | HANDLER-READY — Reached by unregistered handler(s): tool_get_brush. | A3 |
| `toggle_draw_tool` | src/editor.rs:3515 | EQUIV — `set_draw_tool` — Explicit tool set supersedes the toggle. | — |
| `add_audio_clip` | src/editor.rs:3528 | EQUIV — `add_audio_clip` — Registry tool reimplements the op inline with the same WAV validation + status voice (mcp_tools.rs:1237). | — |
| `audio_normalize` | src/editor.rs:3566 | COVERED (reached from 1 registered tool) | — |
| `audio_trim` | src/editor.rs:3611 | COVERED (reached from 1 registered tool) | — |
| `audio_split` | src/editor.rs:3673 | COVERED (reached from 5 registered tools) | — |
| `audio_crossfade` | src/editor.rs:3747 | COVERED (reached from 5 registered tools) | — |
| `audio_echo_last` | src/editor.rs:3838 | EQUIV — `audio_echo` — Tool calls the parameterised audio_echo_at(idx); pass the last index for the *_last form. | — |
| `audio_lowpass_last` | src/editor.rs:3874 | EQUIV — `audio_lowpass` — Via audio_lowpass_at. | — |
| `audio_trim_silence_last` | src/editor.rs:3916 | EQUIV — `audio_trim_silence` — Via audio_trim_silence_at. | — |
| `audio_pitch_last` | src/editor.rs:3965 | EQUIV — `audio_pitch` — Via audio_pitch_at. | — |
| `audio_compress_last` | src/editor.rs:4006 | EQUIV — `audio_compress` — Via audio_compress_at. | — |
| `audio_split_at_silences_last` | src/editor.rs:4059 | EQUIV — `audio_split_silences` — Via audio_split_silences_at. | — |
| `make_clip_from_figure` | src/editor.rs:4131 | EQUIV — `clip_create` — Handler calls the parameterised create_clip_from_figure. | — |
| `insert_clip_figure` | src/editor.rs:4168 | EQUIV — `clip_insert_figure` — Registry tool of the matching name. | — |
| `enter_clip` | src/editor.rs:4197 | COVERED (reached from 1 registered tool) | — |
| `exit_clip` | src/editor.rs:4218 | COVERED (reached from 1 registered tool) | — |
| `clip_add_frame` | src/editor.rs:4269 | COVERED (reached from 1 registered tool) | — |
| `clip_delete_frame` | src/editor.rs:4292 | COVERED (reached from 1 registered tool) | — |
| `clip_goto` | src/editor.rs:4313 | COVERED (reached from 1 registered tool) | — |
| `clip_set_angle` | src/editor.rs:4329 | COVERED (reached from 1 registered tool) | — |
| `clip_set_root` | src/editor.rs:4351 | COVERED (reached from 1 registered tool) | — |
| `clip_positions` | src/editor.rs:4368 | INTERNAL — Read helper for the clips panel; clip_list / clip_get_pose surface the data. | — |
| `create_clip_from_figure` | src/editor.rs:4389 | COVERED (reached from 1 registered tool) | — |
| `cycle_clip_loop` | src/editor.rs:4432 | COVERED (reached from 1 registered tool) | — |
| `set_clip_loop_mode` | src/editor.rs:4460 | COVERED (reached from 1 registered tool) | — |
| `hold_camera` | src/editor.rs:4487 | COVERED (reached from 5 registered tools) | — |
| `duplicate_clip` | src/editor.rs:4519 | COVERED (reached from 2 registered tools) | — |
| `rename_clip` | src/editor.rs:4542 | COVERED (reached from 1 registered tool) | — |
| `delete_clip` | src/editor.rs:4573 | COVERED (reached from 2 registered tools) | — |
| `move_clip` | src/editor.rs:4594 | COVERED (reached from 2 registered tools) | — |
| `insert_clip_figure_at` | src/editor.rs:4637 | COVERED (reached from 1 registered tool) | — |
| `save_named_project` | src/editor.rs:4665 | COVERED (reached from 5 registered tools) | — |
| `note_project_swapped` | src/editor.rs:4681 | COVERED (reached from 8 registered tools) | — |
| `project_swaps` | src/editor.rs:4696 | SHELL — Home swap-list session state; opening a file is load_project. | B9 |
| `note_project_replaced` | src/editor.rs:4704 | COVERED (reached from 2 registered tools) | — |
| `clear_history` | src/editor.rs:4713 | COVERED (reached from 1 registered tool) | — |
| `note_project_loaded` | src/editor.rs:4724 | COVERED (reached from 3 registered tools) | — |
| `cycle_project_file` | src/editor.rs:4787 | GAP — Home/palette project cycling has no tool and no handler. | B9 |
| `open_project_file` | src/editor.rs:4802 | GAP — Recent-project open (Home rows, File recents, project panel) has no tool; load_project needs a path the shell list owns. | B9 |
| `cycle_onion_frames` | src/editor.rs:4828 | GAP — Onion depth stepper on the core; A9s set_onion_skin toggles only the master switch. | A9 |
| `export_mp4` | src/editor.rs:4833 | COVERED (reached from 2 registered tools) | — |
| `export_webm` | src/editor.rs:4844 | COVERED (reached from 2 registered tools) | — |
| `export_svg` | src/editor.rs:4855 | COVERED (reached from 2 registered tools) | — |
| `export_sheet` | src/editor.rs:4866 | EQUIV — `render` — format spritesheet. | — |
| `export_png` | src/editor.rs:4877 | EQUIV — `render` — format png; the op only adds a fixed CWD path. | — |
| `enter_3d` | src/editor.rs:4892 | COVERED (reached from 1 registered tool) | — |
| `exit_3d` | src/editor.rs:4911 | COVERED (reached from 1 registered tool) | — |
| `fig3_select_next` | src/editor.rs:4920 | COVERED (reached from 1 registered tool) | — |
| `fig3_rotate` | src/editor.rs:4933 | COVERED (reached from 1 registered tool) | — |
| `fig3_orbit` | src/editor.rs:4957 | COVERED (reached from 1 registered tool) | — |
| `fig3_bake` | src/editor.rs:5050 | COVERED (reached from 1 registered tool) | — |
| `fig3_mark_pose_b` | src/editor.rs:5085 | COVERED (reached from 1 registered tool) | — |
| `fig3_save_to_library` | src/editor.rs:5106 | COVERED (reached from 5 registered tools) | — |
| `fig3_load_from_library` | src/editor.rs:5128 | COVERED (reached from 6 registered tools) | — |
| `fig3_library_list` | src/editor.rs:5156 | COVERED (reached from 18 registered tools) | — |
| `fig3_cycle_library` | src/editor.rs:5165 | COVERED (reached from 1 registered tool) | — |
| `fig3_bake_animation` | src/editor.rs:5194 | COVERED (reached from 1 registered tool) | — |
| `tool_set_weather_kind` | src/editor.rs:5500 | HANDLER-READY — Handler written (src/editor.rs:5500); no Tool entry yet — integrator splice. | A9 |
| `tool_cycle_weather` | src/editor.rs:5554 | HANDLER-READY — Handler written (src/editor.rs:5554); no Tool entry yet — integrator splice. | A9 |
| `tool_set_weather_amount` | src/editor.rs:5611 | HANDLER-READY — Handler written (src/editor.rs:5611); no Tool entry yet — integrator splice. | A9 |
| `tool_set_weather_wind` | src/editor.rs:5652 | HANDLER-READY — Handler written (src/editor.rs:5652); no Tool entry yet — integrator splice. | A9 |
| `tool_clear_camera_key` | src/editor.rs:5693 | HANDLER-READY — Handler written (src/editor.rs:5693); no Tool entry yet — integrator splice. | A9 |
| `tool_camera_zoom_by` | src/editor.rs:5732 | HANDLER-READY — Handler written (src/editor.rs:5732); no Tool entry yet — integrator splice. | A9 |
| `tool_rotate_camera` | src/editor.rs:5771 | HANDLER-READY — Handler written (src/editor.rs:5771); no Tool entry yet — integrator splice. | A9 |
| `tool_get_onion_skin` | src/editor.rs:5811 | HANDLER-READY — Handler written (src/editor.rs:5811); no Tool entry yet — integrator splice. | A9 |
| `tool_set_onion_skin` | src/editor.rs:5843 | HANDLER-READY — Handler written (src/editor.rs:5843); no Tool entry yet — integrator splice. | A9 |
| `tool_get_editor_settings` | src/editor.rs:5872 | HANDLER-READY — Handler written (src/editor.rs:5872); no Tool entry yet — integrator splice. | A9 |
| `tool_clipboard_state` | src/editor.rs:6086 | HANDLER-READY — Handler written (src/editor.rs:6086); no Tool entry yet — integrator splice. | A9 |
### src/figure_ops.rs

| Capability | Where | MCP tool / status | Owner |
|---|---|---|---|
| `remap_figures_after_insert` | src/figure_ops.rs:74 | COVERED (reached from 1 registered tool) | — |
| `remap_figures_after_remove` | src/figure_ops.rs:126 | COVERED (reached from 5 registered tools) | — |
| `remap_index_after_move` | src/figure_ops.rs:173 | COVERED (reached from 5 registered tools) | — |
| `remap_figures_after_move` | src/figure_ops.rs:206 | COVERED (reached from 5 registered tools) | — |
| `set_figure_color` | src/figure_ops.rs:255 | COVERED (reached from 1 registered tool) | — |
| `duplicate_figure` | src/figure_ops.rs:286 | COVERED (reached from 1 registered tool) | — |
| `recolor_figure` | src/figure_ops.rs:359 | COVERED (reached from 1 registered tool) | — |
| `mirror_figure_all_frames` | src/figure_ops.rs:402 | COVERED (reached from 1 registered tool) | — |
| `tool_figure_recolor` | src/figure_ops.rs:589 | registered as `figure_recolor` | — |
| `tool_figure_rename` | src/figure_ops.rs:618 | registered as `figure_rename` | — |
| `tool_figure_scale` | src/figure_ops.rs:649 | registered as `figure_scale` | — |
| `tool_figure_centre` | src/figure_ops.rs:781 | registered as `figure_centre` | — |
| `tool_figure_ground` | src/figure_ops.rs:798 | registered as `figure_ground` | — |
| `tool_copy_pose_between` | src/figure_ops.rs:813 | registered as `copy_pose_between` | — |
| `tool_reset_pose` | src/figure_ops.rs:856 | registered as `reset_pose` | — |
| `tool_pose_apply_all_frames` | src/figure_ops.rs:888 | registered as `pose_apply_all_frames` | — |
| `tool_pose_library_list` | src/figure_ops.rs:1036 | registered as `pose_library_list` | — |
| `tool_pose_library_save` | src/figure_ops.rs:1046 | registered as `pose_library_save` | — |
| `tool_pose_library_apply` | src/figure_ops.rs:1089 | registered as `pose_library_apply` | — |
| `tool_pose_library_delete` | src/figure_ops.rs:1142 | registered as `pose_library_delete` | — |
| `tool_pose_library_rename` | src/figure_ops.rs:1162 | registered as `pose_library_rename` | — |
| `tool_layers_set_all_visible` | src/figure_ops.rs:1237 | registered as `layers_set_all_visible` | — |
| `tool_layers_invert_visibility` | src/figure_ops.rs:1271 | registered as `layers_invert_visibility` | — |
| `tool_figure_cycle_opacity` | src/figure_ops.rs:1295 | registered as `figure_cycle_opacity` | — |
| `tool_figure_step_opacity` | src/figure_ops.rs:1359 | HANDLER-READY — Handler written (src/figure_ops.rs:1359); no Tool entry yet — integrator splice. | A2 |
| `tool_figure_toggle_hidden` | src/figure_ops.rs:1401 | HANDLER-READY — Handler written (src/figure_ops.rs:1401); no Tool entry yet — integrator splice. | A2 |
| `tool_figure_toggle_lock` | src/figure_ops.rs:1434 | HANDLER-READY — Handler written (src/figure_ops.rs:1434); no Tool entry yet — integrator splice. | A2 |
| `tool_figure_toggle_solo` | src/figure_ops.rs:1466 | HANDLER-READY — Handler written (src/figure_ops.rs:1466); no Tool entry yet — integrator splice. | A2 |
| `tool_nudge_path_point` | src/figure_ops.rs:1503 | HANDLER-READY — Handler written (src/figure_ops.rs:1503); no Tool entry yet — integrator splice. | A2 |
| `tool_get_figure` | src/figure_ops.rs:1543 | registered as `get_figure` | — |
| `tool_get_node` | src/figure_ops.rs:1575 | registered as `get_node` | — |
| `tool_figure_library_list` | src/figure_ops.rs:1671 | registered as `figure_library_list` | — |
| `tool_figure_library_save` | src/figure_ops.rs:1701 | registered as `figure_library_save` | — |
| `tool_figure_library_insert` | src/figure_ops.rs:1734 | registered as `figure_library_insert` | — |
| `tool_figure_library_cycle` | src/figure_ops.rs:1770 | registered as `figure_library_cycle` | — |
### src/draw_ops.rs

| Capability | Where | MCP tool / status | Owner |
|---|---|---|---|
| `stroke_count` | src/draw_ops.rs:60 | COVERED (reached from 12 registered tools) | — |
| `smooth_stroke` | src/draw_ops.rs:136 | COVERED (reached from 6 registered tools) | — |
| `smooth_frame` | src/draw_ops.rs:157 | COVERED (reached from 1 registered tool) | — |
| `apply_nib` | src/draw_ops.rs:185 | COVERED (reached from 6 registered tools) | — |
| `erase` | src/draw_ops.rs:202 | COVERED (reached from 6 registered tools) | — |
| `mirror_strokes_all` | src/draw_ops.rs:248 | HANDLER-READY — Reached by unregistered handler(s): tool_mirror_all_strokes. | A3 |
| `simplify_strokes` | src/draw_ops.rs:274 | COVERED (reached from 1 registered tool) | — |
| `douglas_peucker_indices` | src/draw_ops.rs:342 | COVERED (reached from 1 registered tool) | — |
| `simplify_stroke_at` | src/draw_ops.rs:409 | COVERED (reached from 1 registered tool) | — |
| `tool_list_strokes` | src/draw_ops.rs:637 | registered as `list_strokes` | — |
| `tool_get_drawing_stats` | src/draw_ops.rs:661 | registered as `get_drawing_stats` | — |
| `tool_draw_stroke` | src/draw_ops.rs:703 | registered as `draw_stroke` | — |
| `tool_draw_arrow` | src/draw_ops.rs:757 | registered as `draw_arrow` | — |
| `tool_set_stroke_style` | src/draw_ops.rs:818 | registered as `set_stroke_style` | — |
| `tool_simplify_stroke` | src/draw_ops.rs:923 | registered as `simplify_stroke` | — |
| `tool_fill_stroke` | src/draw_ops.rs:964 | registered as `fill_stroke` | — |
| `tool_lasso_erase` | src/draw_ops.rs:985 | registered as `lasso_erase` | — |
| `tool_mirror_strokes` | src/draw_ops.rs:1009 | registered as `mirror_strokes` | — |
| `tool_clear_frame_strokes` | src/draw_ops.rs:1028 | registered as `clear_frame_strokes` | — |
| `tool_delete_all_strokes` | src/draw_ops.rs:1059 | registered as `delete_all_strokes` | — |
| `tool_recolor_strokes` | src/draw_ops.rs:1086 | registered as `recolor_strokes` | — |
| `tool_list_brushes` | src/draw_ops.rs:1142 | registered as `list_brushes` | — |
| `tool_set_brush` | src/draw_ops.rs:1179 | registered as `set_brush` | — |
| `tool_cycle_brush` | src/draw_ops.rs:1213 | registered as `cycle_brush` | — |
| `tool_set_draw_tool` | src/draw_ops.rs:1228 | registered as `set_draw_tool` | — |
| `tool_get_brush` | src/draw_ops.rs:1289 | HANDLER-READY — Handler written (src/draw_ops.rs:1289); no Tool entry yet — integrator splice. | A3 |
| `tool_set_draw_mode` | src/draw_ops.rs:1341 | HANDLER-READY — Handler written (src/draw_ops.rs:1341); no Tool entry yet — integrator splice. | A3 |
| `tool_step_brush_param` | src/draw_ops.rs:1429 | HANDLER-READY — Handler written (src/draw_ops.rs:1429); no Tool entry yet — integrator splice. | A3 |
| `tool_set_brush_nib` | src/draw_ops.rs:1529 | HANDLER-READY — Handler written (src/draw_ops.rs:1529); no Tool entry yet — integrator splice. | A3 |
| `tool_apply_nib_stroke` | src/draw_ops.rs:1589 | HANDLER-READY — Handler written (src/draw_ops.rs:1589); no Tool entry yet — integrator splice. | A3 |
| `tool_smooth_stroke` | src/draw_ops.rs:1653 | HANDLER-READY — Handler written (src/draw_ops.rs:1653); no Tool entry yet — integrator splice. | A3 |
| `tool_mirror_all_strokes` | src/draw_ops.rs:1719 | HANDLER-READY — Handler written (src/draw_ops.rs:1719); no Tool entry yet — integrator splice. | A3 |
| `tool_set_stroke_spray` | src/draw_ops.rs:1781 | HANDLER-READY — Handler written (src/draw_ops.rs:1781); no Tool entry yet — integrator splice. | A3 |
| `tool_clear_strokes_on_frame` | src/draw_ops.rs:1896 | HANDLER-READY — Handler written (src/draw_ops.rs:1896); no Tool entry yet — integrator splice. | A3 |
### src/fx_ops.rs

| Capability | Where | MCP tool / status | Owner |
|---|---|---|---|
| `fx_kind_cycle` | src/fx_ops.rs:191 | EQUIV — `fx_set_kind` — Explicit kind write. | — |
| `fx_move_clip` | src/fx_ops.rs:206 | EQUIV — `fx_move` — fx_move mirrors this op exactly (absolute start; fx_ops comment in mcp_tools.rs:994). | — |
| `fx_resize_clip` | src/fx_ops.rs:227 | COVERED (reached from 1 registered tool) | — |
| `fx_split_clip` | src/fx_ops.rs:263 | COVERED (reached from 1 registered tool) | — |
| `fx_duplicate_clip` | src/fx_ops.rs:307 | COVERED (reached from 1 registered tool) | — |
| `fx_delete_clip` | src/fx_ops.rs:326 | COVERED (reached from 1 registered tool) | — |
| `fx_set_color` | src/fx_ops.rs:342 | COVERED (reached from 1 registered tool) | — |
| `fx_set_color2` | src/fx_ops.rs:356 | COVERED (reached from 1 registered tool) | — |
| `fx_set_anchor` | src/fx_ops.rs:378 | COVERED (reached from 1 registered tool) | — |
| `fx_set_kind` | src/fx_ops.rs:404 | COVERED (reached from 1 registered tool) | — |
| `fx_set_hidden` | src/fx_ops.rs:431 | COVERED (reached from 1 registered tool) | — |
| `fx_set_seed` | src/fx_ops.rs:456 | HANDLER-READY — Reached by unregistered handler(s): tool_fx_set_seed. | A5 |
| `fx_set_angle` | src/fx_ops.rs:477 | HANDLER-READY — Reached by unregistered handler(s): tool_fx_set_angle. | A5 |
| `fx_set_length` | src/fx_ops.rs:500 | HANDLER-READY — Reached by unregistered handler(s): tool_fx_set_length. | A5 |
| `tool_fx_set_hidden` | src/fx_ops.rs:597 | registered as `fx_set_hidden` | — |
| `tool_fx_preset_list` | src/fx_ops.rs:619 | registered as `fx_preset_list` | — |
| `tool_fx_preset_delete` | src/fx_ops.rs:633 | registered as `fx_preset_delete` | — |
| `tool_fx_set_seed` | src/fx_ops.rs:676 | HANDLER-READY — Handler written (src/fx_ops.rs:676); no Tool entry yet — integrator splice. | A5 |
| `tool_fx_set_angle` | src/fx_ops.rs:709 | HANDLER-READY — Handler written (src/fx_ops.rs:709); no Tool entry yet — integrator splice. | A5 |
| `tool_fx_set_length` | src/fx_ops.rs:742 | HANDLER-READY — Handler written (src/fx_ops.rs:742); no Tool entry yet — integrator splice. | A5 |
### src/fx_edit.rs

| Capability | Where | MCP tool / status | Owner |
|---|---|---|---|
| `kind_name` | src/fx_edit.rs:63 | COVERED (reached from 13 registered tools) | — |
| `list` | src/fx_edit.rs:96 | COVERED (reached from 6 registered tools) | — |
| `move_clip` | src/fx_edit.rs:121 | COVERED (reached from 2 registered tools) | — |
| `resize_clip` | src/fx_edit.rs:132 | COVERED (reached from 1 registered tool) | — |
| `duplicate_clip` | src/fx_edit.rs:146 | COVERED (reached from 2 registered tools) | — |
| `delete_clip` | src/fx_edit.rs:157 | COVERED (reached from 2 registered tools) | — |
| `clips_at` | src/fx_edit.rs:169 | INTERNAL — Read helper (fx clips at a frame). | — |
| `save_clip_preset` | src/fx_edit.rs:350 | COVERED (reached from 1 registered tool) | — |
| `load_clip_preset` | src/fx_edit.rs:360 | COVERED (reached from 1 registered tool) | — |
| `list_clip_presets` | src/fx_edit.rs:366 | COVERED (reached from 1 registered tool) | — |
| `delete_clip_preset` | src/fx_edit.rs:392 | COVERED (reached from 1 registered tool) | — |
### src/particle_ops.rs

| Capability | Where | MCP tool / status | Owner |
|---|---|---|---|
| `particle_add_event` | src/particle_ops.rs:91 | COVERED (reached from 5 registered tools) | — |
| `particle_delete_event` | src/particle_ops.rs:124 | COVERED (reached from 1 registered tool) | — |
| `particle_duplicate_event` | src/particle_ops.rs:151 | COVERED (reached from 1 registered tool) | — |
| `particle_set_frame` | src/particle_ops.rs:177 | COVERED (reached from 1 registered tool) | — |
| `particle_nudge_frame` | src/particle_ops.rs:205 | COVERED (reached from 1 registered tool) | — |
| `particle_set_anchor` | src/particle_ops.rs:240 | COVERED (reached from 1 registered tool) | — |
| `particle_set_position` | src/particle_ops.rs:274 | COVERED (reached from 1 registered tool) | — |
| `particle_set_kind` | src/particle_ops.rs:297 | COVERED (reached from 1 registered tool) | — |
| `particle_set_color` | src/particle_ops.rs:319 | COVERED (reached from 1 registered tool) | — |
| `particle_set_seed` | src/particle_ops.rs:348 | HANDLER-READY — Reached by unregistered handler(s): tool_particle_set_seed. | A5 |
| `tool_particle_list_events` | src/particle_ops.rs:524 | registered as `particle_list_events` | — |
| `tool_particle_delete_event` | src/particle_ops.rs:542 | registered as `particle_delete_event` | — |
| `tool_particle_set_frame` | src/particle_ops.rs:560 | registered as `particle_set_frame` | — |
| `tool_particle_nudge_frame` | src/particle_ops.rs:577 | registered as `particle_nudge_frame` | — |
| `tool_particle_set_position` | src/particle_ops.rs:595 | registered as `particle_set_position` | — |
| `tool_particle_set_anchor` | src/particle_ops.rs:623 | registered as `particle_set_anchor` | — |
| `tool_particle_add_anchored` | src/particle_ops.rs:665 | registered as `particle_add_anchored` | — |
| `tool_particle_set_seed` | src/particle_ops.rs:745 | HANDLER-READY — Handler written (src/particle_ops.rs:745); no Tool entry yet — integrator splice. | A5 |
### src/audio_ops.rs

| Capability | Where | MCP tool / status | Owner |
|---|---|---|---|
| `clip_peaks` | src/audio_ops.rs:198 | COVERED (reached from 2 registered tools) | — |
| `clip_duration_secs` | src/audio_ops.rs:226 | EQUIV — `audio_clip_info` — Read; durations surface in the info reply. | — |
| `clip_duration_frames` | src/audio_ops.rs:242 | EQUIV — `audio_clip_info` — Read; as above. | — |
| `audio_set_muted_at` | src/audio_ops.rs:267 | COVERED (reached from 2 registered tools) | — |
| `audio_toggle_muted_at` | src/audio_ops.rs:298 | COVERED (reached from 1 registered tool) | — |
| `normalised_gain` | src/audio_ops.rs:347 | COVERED (reached from 1 registered tool) | — |
| `audio_fade_at` | src/audio_ops.rs:372 | COVERED (reached from 2 registered tools) | — |
| `audio_fade_last` | src/audio_ops.rs:408 | COVERED (reached from 1 registered tool) | — |
| `audio_reverse_at` | src/audio_ops.rs:422 | COVERED (reached from 2 registered tools) | — |
| `audio_reverse_last` | src/audio_ops.rs:461 | COVERED (reached from 1 registered tool) | — |
| `audio_set_gain_at` | src/audio_ops.rs:477 | COVERED (reached from 2 registered tools) | — |
| `audio_set_gain_last` | src/audio_ops.rs:495 | COVERED (reached from 1 registered tool) | — |
| `audio_clip_gain_step_at` | src/audio_ops.rs:509 | HANDLER-READY — Reached by unregistered handler(s): tool_audio_clip_gain_step. | A6 |
| `audio_clip_gain_step` | src/audio_ops.rs:522 | EQUIV — `audio_clip_set_gain` — Explicit gain write; the _at stepper is handler-ready via tool_audio_clip_gain_step (A6). | A6 |
| `normalise_all_clips` | src/audio_ops.rs:552 | COVERED (reached from 1 registered tool) | — |
| `delete_audio_clip_at` | src/audio_ops.rs:583 | COVERED (reached from 1 registered tool) | — |
| `delete_audio_clip` | src/audio_ops.rs:597 | COVERED (reached from 1 registered tool) | — |
| `audio_duplicate_clip_at` | src/audio_ops.rs:616 | COVERED (reached from 1 registered tool) | — |
| `audio_move_at` | src/audio_ops.rs:635 | COVERED (reached from 2 registered tools) | — |
| `audio_move_last` | src/audio_ops.rs:654 | COVERED (reached from 1 registered tool) | — |
| `audio_echo_at` | src/audio_ops.rs:683 | COVERED (reached from 1 registered tool) | — |
| `audio_lowpass_at` | src/audio_ops.rs:723 | COVERED (reached from 1 registered tool) | — |
| `audio_trim_silence_at` | src/audio_ops.rs:768 | COVERED (reached from 1 registered tool) | — |
| `audio_pitch_at` | src/audio_ops.rs:822 | COVERED (reached from 1 registered tool) | — |
| `audio_compress_at` | src/audio_ops.rs:868 | COVERED (reached from 1 registered tool) | — |
| `audio_split_silences_at` | src/audio_ops.rs:926 | COVERED (reached from 1 registered tool) | — |
| `tool_audio_list_clips` | src/audio_ops.rs:1238 | registered as `audio_list_clips` | — |
| `tool_audio_clip_info` | src/audio_ops.rs:1254 | registered as `audio_clip_info` | — |
| `tool_audio_clip_fade` | src/audio_ops.rs:1271 | registered as `audio_clip_fade` | — |
| `tool_audio_clip_reverse` | src/audio_ops.rs:1280 | registered as `audio_clip_reverse` | — |
| `tool_audio_clip_set_gain` | src/audio_ops.rs:1292 | registered as `audio_clip_set_gain` | — |
| `tool_audio_clip_move` | src/audio_ops.rs:1312 | registered as `audio_clip_move` | — |
| `tool_audio_clip_duplicate` | src/audio_ops.rs:1331 | registered as `audio_clip_duplicate` | — |
| `tool_audio_clip_normalise` | src/audio_ops.rs:1351 | registered as `audio_clip_normalise` | — |
| `tool_audio_clip_trim` | src/audio_ops.rs:1365 | registered as `audio_clip_trim` | — |
| `tool_audio_clip_toggle_muted` | src/audio_ops.rs:1393 | registered as `audio_clip_toggle_muted` | — |
| `tool_audio_clip_gain_step` | src/audio_ops.rs:1448 | HANDLER-READY — Handler written (src/audio_ops.rs:1448); no Tool entry yet — integrator splice. | A6 |
### src/audio_edit.rs

| Capability | Where | MCP tool / status | Owner |
|---|---|---|---|
| `frames` | src/audio_edit.rs:22 | COVERED (reached from 45 registered tools) | — |
| `duration_secs` | src/audio_edit.rs:31 | COVERED (reached from 7 registered tools) | — |
| `read_wav` | src/audio_edit.rs:41 | COVERED (reached from 24 registered tools) | — |
| `write_wav` | src/audio_edit.rs:108 | COVERED (reached from 21 registered tools) | — |
| `peak` | src/audio_edit.rs:136 | COVERED (reached from 3 registered tools) | — |
| `rms` | src/audio_edit.rs:142 | INTERNAL — DSP helper, no user-facing capability. | — |
| `normalize` | src/audio_edit.rs:157 | COVERED (reached from 1 registered tool) | — |
| `fade` | src/audio_edit.rs:183 | COVERED (reached from 2 registered tools) | — |
| `gain` | src/audio_edit.rs:220 | COVERED (reached from 4 registered tools) | — |
| `reverse` | src/audio_edit.rs:232 | COVERED (reached from 10 registered tools) | — |
| `echo` | src/audio_edit.rs:263 | COVERED (reached from 5 registered tools) | — |
| `lowpass` | src/audio_edit.rs:289 | COVERED (reached from 1 registered tool) | — |
| `trim_silence` | src/audio_edit.rs:329 | COVERED (reached from 1 registered tool) | — |
| `split_at_silences` | src/audio_edit.rs:370 | COVERED (reached from 1 registered tool) | — |
| `pitch_shift` | src/audio_edit.rs:424 | COVERED (reached from 1 registered tool) | — |
| `compress` | src/audio_edit.rs:468 | COVERED (reached from 1 registered tool) | — |
| `trim` | src/audio_edit.rs:506 | COVERED (reached from 21 registered tools) | — |
| `split_wav` | src/audio_edit.rs:526 | COVERED (reached from 5 registered tools) | — |
| `peaks` | src/audio_edit.rs:544 | COVERED (reached from 2 registered tools) | — |
| `mix` | src/audio_edit.rs:627 | COVERED (reached from 8 registered tools) | — |
| `crossfade_mix` | src/audio_edit.rs:699 | COVERED (reached from 5 registered tools) | — |
| `audible_at` | src/audio_edit.rs:757 | INTERNAL — Read helper for waveform/sync rendering. | — |
| `split_clip` | src/audio_edit.rs:792 | COVERED (reached from 5 registered tools) | — |
### src/audio_play.rs

| Capability | Where | MCP tool / status | Owner |
|---|---|---|---|
| `playhead_seeked` | src/audio_play.rs:277 | SHELL — Playback-engine sync hook, fired by the shell on seek. | B4 |
| `drift_frames` | src/audio_play.rs:297 | INTERNAL — Playback drift maths helper. | — |
| `drift_exceeded` | src/audio_play.rs:304 | INTERNAL — Playback drift maths helper. | — |
| `expected_frame` | src/audio_play.rs:319 | INTERNAL — Playback position maths helper. | — |
| `expected_frame_walk` | src/audio_play.rs:362 | INTERNAL — Playback position maths helper. | — |
| `walk_wraps` | src/audio_play.rs:479 | INTERNAL — Playback position maths helper. | — |
| `start_playback_audio` | src/audio_play.rs:652 | SHELL — Audio engine start; fires from the shell when playing flips true (set_playback / B4 shell_play). | B4 |
| `stop_playback_audio` | src/audio_play.rs:685 | SHELL — Audio engine stop; shell-driven. | B4 |
| `note_playback_speed` | src/audio_play.rs:708 | SHELL — Engine note-on from the shell speed control; B7 SettingPatch::PlaybackSpeed is handler-ready. | B4 |
| `sync_playback_audio_on_wrap` | src/audio_play.rs:744 | SHELL — Engine sync hook, shell-driven. | B4 |
| `sync_playback_audio_on_seek` | src/audio_play.rs:788 | SHELL — Engine sync hook, shell-driven. | B4 |
| `sync_playback_audio_on_drift` | src/audio_play.rs:825 | SHELL — Engine sync hook, shell-driven. | B4 |
| `sync_playback_audio_on_full_loop` | src/audio_play.rs:889 | SHELL — Engine sync hook, shell-driven. | B4 |
| `playback_audio_active` | src/audio_play.rs:909 | SHELL — Engine state read used by the settings card; get_editor_status is the MCP read surface. | B4 |
### src/projects.rs

| Capability | Where | MCP tool / status | Owner |
|---|---|---|---|
| `scan` | src/projects.rs:218 | COVERED (reached from 1 registered tool) | — |
| `list_project_files` | src/projects.rs:283 | COVERED (reached from 11 registered tools) | — |
| `refresh_project_files` | src/projects.rs:348 | COVERED (reached from 4 registered tools) | — |
| `load` | src/projects.rs:386 | COVERED (reached from 4 registered tools) | — |
| `save` | src/projects.rs:399 | COVERED (reached from 5 registered tools) | — |
| `save_copy` | src/projects.rs:427 | COVERED (reached from 1 registered tool) | — |
| `rename_project` | src/projects.rs:495 | COVERED (reached from 1 registered tool) | — |
| `duplicate_project` | src/projects.rs:534 | COVERED (reached from 1 registered tool) | — |
| `filter_projects` | src/projects.rs:582 | INTERNAL — Pure filter helper behind the Home list (list_projects covers the data). | — |
| `thumbnail` | src/projects.rs:593 | SHELL — Home row thumbnail renderer (shell-side pixels). | B9 |
| `tool_delete_project_file` | src/projects.rs:681 | HANDLER-READY — Handler written (src/projects.rs:681); no Tool entry yet — integrator splice. | A8 |
### src/media.rs

| Capability | Where | MCP tool / status | Owner |
|---|---|---|---|
| `png_dims` | src/media.rs:107 | COVERED (reached from 5 registered tools) | — |
| `media_items` | src/media.rs:131 | COVERED (reached from 5 registered tools) | — |
| `composite_media` | src/media.rs:272 | COVERED (reached from 5 registered tools) | — |
| `tool_assets_list` | src/media.rs:549 | registered as `assets_list` | — |
| `tool_asset_info` | src/media.rs:566 | registered as `asset_info` | — |
| `tool_asset_import` | src/media.rs:582 | registered as `asset_import` | — |
| `tool_asset_rename` | src/media.rs:632 | registered as `asset_rename` | — |
| `tool_asset_remove` | src/media.rs:672 | registered as `asset_remove` | — |
| `tool_asset_attach` | src/media.rs:710 | registered as `asset_attach` | — |
| `tool_sprite_list` | src/media.rs:812 | registered as `sprite_list` | — |
| `tool_sprite_add` | src/media.rs:831 | registered as `sprite_add` | — |
| `tool_sprite_remove` | src/media.rs:855 | registered as `sprite_remove` | — |
| `tool_sprite_set_key` | src/media.rs:881 | registered as `sprite_set_key` | — |
| `tool_sprite_delete_key` | src/media.rs:950 | registered as `sprite_delete_key` | — |
| `tool_sprite_sample` | src/media.rs:1009 | HANDLER-READY — Handler written (src/media.rs:1009); no Tool entry yet — integrator splice. | A8 |
### src/sprite_ops.rs

| Capability | Where | MCP tool / status | Owner |
|---|---|---|---|
| `add_sprite` | src/sprite_ops.rs:104 | COVERED (reached from 1 registered tool) | — |
| `remove_sprite` | src/sprite_ops.rs:129 | COVERED (reached from 1 registered tool) | — |
| `set_sprite_key` | src/sprite_ops.rs:149 | COVERED (reached from 1 registered tool) | — |
| `delete_sprite_key` | src/sprite_ops.rs:193 | COVERED (reached from 1 registered tool) | — |
| `sprite_sample` | src/sprite_ops.rs:232 | HANDLER-READY — Reached by unregistered handler(s): tool_sprite_sample. | A8 |
| `sprite_key_at` | src/sprite_ops.rs:243 | INTERNAL — Read helper; sprite_list returns all keys and tool_sprite_sample (A8, handler-ready) covers sampling. | — |
### src/fig3d.rs

| Capability | Where | MCP tool / status | Owner |
|---|---|---|---|
| `rest` | src/fig3d.rs:71 | COVERED (reached from 1 registered tool) | — |
| `default_camera` | src/fig3d.rs:121 | COVERED (reached from 3 registered tools) | — |
| `camera_preset` | src/fig3d.rs:149 | COVERED (reached from 1 registered tool) | — |
| `grid_line_points` | src/fig3d.rs:185 | INTERNAL — 3D preview render geometry. | — |
| `preview_camera` | src/fig3d.rs:225 | COVERED (reached from 1 registered tool) | — |
| `frame_camera_points` | src/fig3d.rs:277 | COVERED (reached from 1 registered tool) | — |
| `frame_camera` | src/fig3d.rs:363 | COVERED (reached from 1 registered tool) | — |
| `mat3_identity` | src/fig3d.rs:430 | COVERED (reached from 1 registered tool) | — |
| `mat3_mul` | src/fig3d.rs:435 | COVERED (reached from 14 registered tools) | — |
| `mat3_apply` | src/fig3d.rs:445 | COVERED (reached from 14 registered tools) | — |
| `mat3_from_euler` | src/fig3d.rs:455 | COVERED (reached from 14 registered tools) | — |
| `mat3_to_euler` | src/fig3d.rs:499 | COVERED (reached from 1 registered tool) | — |
| `evaluate3` | src/fig3d.rs:559 | COVERED (reached from 14 registered tools) | — |
| `solve_two_bone_ik` | src/fig3d.rs:676 | COVERED (reached from 1 registered tool) | — |
| `solve_arm_ik` | src/fig3d.rs:814 | COVERED (reached from 1 registered tool) | — |
| `solve_leg_ik` | src/fig3d.rs:838 | COVERED (reached from 1 registered tool) | — |
| `tool_fig3_set_camera` | src/fig3d.rs:914 | registered as `fig3_set_camera` | — |
| `tool_fig3_rename` | src/fig3d.rs:972 | registered as `fig3_rename` | — |
| `project` | src/fig3d.rs:1304 | COVERED (reached from 13 registered tools) | — |
| `draw_order` | src/fig3d.rs:1336 | INTERNAL — 3D render ordering helper. | — |
| `humanoid3` | src/fig3d.rs:1361 | COVERED (reached from 1 registered tool) | — |
| `segments2d` | src/fig3d.rs:1399 | INTERNAL — 3D→2D projection helper behind fig3_get / bake. | — |
| `segment_shades` | src/fig3d.rs:1432 | INTERNAL — 3D preview shading helper. | — |
| `bake_nodes` | src/fig3d.rs:1494 | COVERED (reached from 2 registered tools) | — |
| `lerp_pose` | src/fig3d.rs:1541 | COVERED (reached from 1 registered tool) | — |
| `mirror_pose` | src/fig3d.rs:1651 | COVERED (reached from 1 registered tool) | — |
| `bake_range_staged` | src/fig3d.rs:1676 | COVERED (reached from 1 registered tool) | — |
| `bake_range` | src/fig3d.rs:1697 | EQUIV — `fig3_bake_animation` — Wrapper over bake_range_staged, which the tool calls. | — |
| `library_file_name` | src/fig3d.rs:1711 | COVERED (reached from 7 registered tools) | — |
| `save_library_figure` | src/fig3d.rs:1754 | COVERED (reached from 5 registered tools) | — |
| `load_library_figure` | src/fig3d.rs:1776 | COVERED (reached from 6 registered tools) | — |
| `scan_library` | src/fig3d.rs:1785 | COVERED (reached from 18 registered tools) | — |
| `save_pose_library` | src/fig3d.rs:1853 | COVERED (reached from 1 registered tool) | — |
| `load_pose_library` | src/fig3d.rs:1871 | COVERED (reached from 1 registered tool) | — |
| `scan_pose_library` | src/fig3d.rs:1904 | COVERED (reached from 13 registered tools) | — |
| `tool_fig3_exit` | src/fig3d.rs:2061 | registered as `fig3_exit` | — |
| `tool_fig3_get` | src/fig3d.rs:2070 | registered as `fig3_get` | — |
| `tool_fig3_select_next` | src/fig3d.rs:2077 | registered as `fig3_select_next` | — |
| `tool_fig3_cycle_library` | src/fig3d.rs:2093 | registered as `fig3_cycle_library` | — |
| `tool_fig3_camera_preset` | src/fig3d.rs:2107 | registered as `fig3_camera_preset` | — |
| `tool_fig3_frame_camera` | src/fig3d.rs:2130 | registered as `fig3_frame_camera` | — |
| `tool_fig3_frame_preview` | src/fig3d.rs:2149 | registered as `fig3_frame_preview` | — |
| `tool_fig3_camera_zoom` | src/fig3d.rs:2177 | registered as `fig3_camera_zoom` | — |
| `tool_fig3_ik` | src/fig3d.rs:2218 | registered as `fig3_ik` | — |
| `tool_fig3_pose_mirror` | src/fig3d.rs:2278 | registered as `fig3_pose_mirror` | — |
| `tool_fig3_pose_library_list` | src/fig3d.rs:2293 | registered as `fig3_pose_library_list` | — |
| `tool_fig3_pose_save` | src/fig3d.rs:2305 | registered as `fig3_pose_save` | — |
| `tool_fig3_pose_apply` | src/fig3d.rs:2340 | registered as `fig3_pose_apply` | — |
### src/graph_keys.rs

| Capability | Where | MCP tool / status | Owner |
|---|---|---|---|
| `channel_index_of` | src/graph_keys.rs:113 | COVERED (reached from 1 registered tool) | — |
| `copy_keys` | src/graph_keys.rs:125 | COVERED (reached from 1 registered tool) | — |
| `paste_keys` | src/graph_keys.rs:140 | COVERED (reached from 2 registered tools) | — |
| `duplicate_channel_to_figure` | src/graph_keys.rs:209 | COVERED (reached from 1 registered tool) | — |
| `delete_channel` | src/graph_keys.rs:300 | COVERED (reached from 1 registered tool) | — |
| `keys_in_range` | src/graph_keys.rs:325 | HANDLER-READY — Range read implemented by A4s handler (graph_keys.rs:1518). | A4 |
| `delete_keys_in_range` | src/graph_keys.rs:356 | EQUIV — `graph_delete_keys_in_range` — The tool calls the sibling delete_keys_in_range_at; this fn is a duplicate implementation A4 should reconcile. | A4 |
| `set_key_interp` | src/graph_keys.rs:410 | COVERED (reached from 1 registered tool) | — |
| `key_interp_at` | src/graph_keys.rs:458 | HANDLER-READY — Reached by unregistered handler(s): tool_graph_key_interp. | A4 |
| `shift_keys` | src/graph_keys.rs:491 | COVERED (reached from 1 registered tool) | — |
| `fill_keys_in_range` | src/graph_keys.rs:544 | COVERED (reached from 1 registered tool) | — |
| `target_label` | src/graph_keys.rs:617 | COVERED (reached from 10 registered tools) | — |
| `channel_stats` | src/graph_keys.rs:665 | COVERED (reached from 1 registered tool) | — |
| `tool_graph_list_channels` | src/graph_keys.rs:894 | registered as `graph_list_channels` | — |
| `tool_graph_get_channel` | src/graph_keys.rs:928 | registered as `graph_get_channel` | — |
| `remove_key_at` | src/graph_keys.rs:954 | COVERED (reached from 1 registered tool) | — |
| `tool_graph_delete_key` | src/graph_keys.rs:971 | registered as `graph_delete_key` | — |
| `move_key_at` | src/graph_keys.rs:995 | COVERED (reached from 1 registered tool) | — |
| `tool_graph_move_key` | src/graph_keys.rs:1033 | registered as `graph_move_key` | — |
| `set_key_tangents_at` | src/graph_keys.rs:1055 | COVERED (reached from 1 registered tool) | — |
| `tool_graph_set_tangents` | src/graph_keys.rs:1082 | registered as `graph_set_tangents` | — |
| `tool_graph_shift_keys` | src/graph_keys.rs:1097 | registered as `graph_shift_keys` | — |
| `tool_graph_fill_keys` | src/graph_keys.rs:1111 | registered as `graph_fill_keys` | — |
| `delete_keys_in_range_at` | src/graph_keys.rs:1135 | COVERED (reached from 1 registered tool) | — |
| `tool_graph_delete_keys_in_range` | src/graph_keys.rs:1163 | registered as `graph_delete_keys_in_range` | — |
| `clear_channel_keys_at` | src/graph_keys.rs:1191 | COVERED (reached from 1 registered tool) | — |
| `tool_graph_clear_channel_keys` | src/graph_keys.rs:1208 | registered as `graph_clear_channel_keys` | — |
| `tool_graph_reduce_keys` | src/graph_keys.rs:1224 | registered as `graph_reduce_keys` | — |
| `tool_graph_mirror_channel` | src/graph_keys.rs:1245 | registered as `graph_mirror_channel` | — |
| `tool_graph_scale_channel` | src/graph_keys.rs:1267 | registered as `graph_scale_channel` | — |
| `tool_graph_tangent_preset_channel` | src/graph_keys.rs:1286 | registered as `graph_tangent_preset_channel` | — |
| `key_all_channels_at` | src/graph_keys.rs:1319 | COVERED (reached from 1 registered tool) | — |
| `tool_graph_key_all_channels` | src/graph_keys.rs:1334 | registered as `graph_key_all_channels` | — |
| `tool_graph_key_angle` | src/graph_keys.rs:1356 | registered as `graph_key_angle` | — |
| `tool_graph_clear_channels` | src/graph_keys.rs:1371 | registered as `graph_clear_channels` | — |
| `tool_graph_key_interp` | src/graph_keys.rs:1475 | HANDLER-READY — Handler written (src/graph_keys.rs:1475); no Tool entry yet — integrator splice. | A4 |
| `tool_graph_count_keys_in_range` | src/graph_keys.rs:1518 | HANDLER-READY — Handler written (src/graph_keys.rs:1518); no Tool entry yet — integrator splice. | A4 |
| `tool_graph_set_interp_at_playhead` | src/graph_keys.rs:1576 | HANDLER-READY — Handler written (src/graph_keys.rs:1576); no Tool entry yet — integrator splice. | A4 |
### src/graph.rs

| Capability | Where | MCP tool / status | Owner |
|---|---|---|---|
| `flat` | src/graph.rs:80 | COVERED (reached from 3 registered tools) | — |
| `with_interp` | src/graph.rs:92 | INTERNAL — Channel builder helper. | — |
| `sort_keys` | src/graph.rs:110 | COVERED (reached from 18 registered tools) | — |
| `add_key` | src/graph.rs:132 | COVERED (reached from 3 registered tools) | — |
| `remove_key` | src/graph.rs:144 | COVERED (reached from 3 registered tools) | — |
| `move_key` | src/graph.rs:159 | COVERED (reached from 1 registered tool) | — |
| `set_tangents` | src/graph.rs:180 | COVERED (reached from 1 registered tool) | — |
| `value_at` | src/graph.rs:214 | COVERED (reached from 12 registered tools) | — |
| `auto_tangents` | src/graph.rs:305 | COVERED (reached from 3 registered tools) | — |
| `smooth_channel` | src/graph.rs:348 | COVERED (reached from 1 registered tool) | — |
| `apply_tangent_preset` | src/graph.rs:426 | COVERED (reached from 2 registered tools) | — |
| `reduce_keys` | src/graph.rs:526 | COVERED (reached from 1 registered tool) | — |
| `mirror_keys_time` | src/graph.rs:615 | COVERED (reached from 2 registered tools) | — |
| `scale_key_values` | src/graph.rs:665 | COVERED (reached from 2 registered tools) | — |
| `channel_mut` | src/graph.rs:686 | INTERNAL — Accessor used by the graph ops themselves. | — |
| `apply_channels` | src/graph.rs:711 | COVERED (reached from 9 registered tools) | — |
### src/frame_swap.rs

| Capability | Where | MCP tool / status | Owner |
|---|---|---|---|
| `swap_frames` | src/frame_swap.rs:206 | COVERED (reached from 6 registered tools) | — |
| `swap_frame_with_neighbour` | src/frame_swap.rs:283 | COVERED (reached from 1 registered tool) | — |
| `insert_frames_after_current` | src/frame_swap.rs:405 | COVERED (reached from 1 registered tool) | — |
| `tool_swap_frame_with_neighbour` | src/frame_swap.rs:439 | registered as `swap_frame_with_neighbour` | — |
| `tool_insert_frames` | src/frame_swap.rs:471 | registered as `insert_frames` | — |
| `tool_adjust_frame_duration` | src/frame_swap.rs:512 | HANDLER-READY — Handler written (src/frame_swap.rs:512); no Tool entry yet — integrator splice. | A2 |
| `tool_get_keyframes` | src/frame_swap.rs:548 | registered as `get_keyframes` | — |
| `tool_list_frames` | src/frame_swap.rs:568 | registered as `list_frames` | — |

---

## Family 2 — the 134 palette commands

Every `Command` in src/rune_sec/command_palette.rs, classified by what
its run fn actually does (read from source, not from the title).

| Command | Title | Where | MCP tool / status | Owner |
|---|---|---|---|---|
| `play` | Play / Pause (Transport) | src/rune_sec/command_palette.rs:1320 | HANDLER-READY — `tool_shell_play / tool_shell_pause` — Core flag also settable today via set_playback; the shell handlers (shell_tools_transport.rs:155,187) add the real GUI transport. | B4 |
| `prev` | Previous frame (Transport) | src/rune_sec/command_palette.rs:1321 | COVERED — via `audio_trim_silence, delete_frame` | — |
| `next` | Next frame (Transport) | src/rune_sec/command_palette.rs:1322 | COVERED — via `audio_trim_silence, delete_frame` | — |
| `first-frame` | Go to first frame (Transport) | src/rune_sec/command_palette.rs:1323 | COVERED — via `audio_trim_silence, delete_frame` | — |
| `restart-playback` | Restart playback (Transport) | src/rune_sec/command_palette.rs:1324 | EQUIV — `set_playback` — action restart; shell form handler-ready via tool_shell_play + tool_shell_set_playhead. | B4 |
| `toggle-loop-playback` | Toggle loop playback (Transport) | src/rune_sec/command_palette.rs:1325 | HANDLER-READY — `exec_shell_set_setting` — SettingPatch::LoopPlayback (shell_tools_settings.rs:391) flips State::loop_playback (main.rs:226). | B7 |
| `add-frame` | Add frame (Frames) | src/rune_sec/command_palette.rs:1327 | COVERED — via `add_frame, audio_trim_silence` | — |
| `dup-frame` | Duplicate frame (Frames) | src/rune_sec/command_palette.rs:1328 | COVERED — via `audio_trim_silence, delete_range` | — |
| `del-frame` | Delete frame (Frames) | src/rune_sec/command_palette.rs:1329 | COVERED — via `delete_frame, export_bundle` | — |
| `inbetween` | Insert inbetween (Frames) | src/rune_sec/command_palette.rs:1330 | COVERED — via `insert_inbetween` | — |
| `copy-frame` | Copy frame (Frames) | src/rune_sec/command_palette.rs:1331 | COVERED — via `audio_trim_silence, copy_frame` | — |
| `paste-frame` | Paste frame (Frames) | src/rune_sec/command_palette.rs:1332 | COVERED — via `audio_trim_silence, delete_range` | — |
| `fps-up` | Increase FPS (Frames) | src/rune_sec/command_palette.rs:1333 | EQUIV — `set_project_fps` — Stepper over the explicit write. | — |
| `fps-down` | Decrease FPS (Frames) | src/rune_sec/command_palette.rs:1334 | EQUIV — `set_project_fps` — As fps-up. | — |
| `undo` | Undo (Edit) | src/rune_sec/command_palette.rs:1336 | COVERED — via `export_bundle, export_file` | — |
| `redo` | Redo (Edit) | src/rune_sec/command_palette.rs:1337 | COVERED — via `export_bundle, export_file` | — |
| `save` | Save project (Projects) | src/rune_sec/command_palette.rs:1339 | COVERED — via `export_bundle, export_file` | — |
| `load` | Load project (Projects) | src/rune_sec/command_palette.rs:1340 | COVERED — via `audio_trim_silence, delete_range` | — |
| `save-named` | Save named project (Projects) | src/rune_sec/command_palette.rs:1341 | COVERED — via `audio_trim_silence, delete_range` | — |
| `cycle-project` | Cycle project file (Projects) | src/rune_sec/command_palette.rs:1342 | GAP — Calls cycle_project_file — no tool, no handler. | B9 |
| `open-project` | Open project file (Projects) | src/rune_sec/command_palette.rs:1343 | GAP — Calls open_project_file — no tool, no handler. | B9 |
| `add-figure` | Add figure (Figures) | src/rune_sec/command_palette.rs:1345 | COVERED — via `add_figure` | — |
| `add-quadruped` | Add quadruped figure (Figures) | src/rune_sec/command_palette.rs:1346 | COVERED — via `add_figure` | — |
| `del-figure` | Delete figure (Figures) | src/rune_sec/command_palette.rs:1347 | COVERED — via `audio_trim_silence, delete_figure` | — |
| `duplicate-figure` | Duplicate figure (Figures) | src/rune_sec/command_palette.rs:1348 | COVERED — via `figure_duplicate` | — |
| `hide-figure` | Toggle figure hidden (Figures) | src/rune_sec/command_palette.rs:1349 | EQUIV — `layers` — locked/hidden/opacity/solo in one call; toggle handlers also handler-ready (A2). | A2 |
| `solo-figure` | Solo selected figure (Figures) | src/rune_sec/command_palette.rs:1350 | EQUIV — `layers` — As hide-figure. | A2 |
| `lock-figure` | Lock selected figure (Figures) | src/rune_sec/command_palette.rs:1351 | EQUIV — `layers` — As hide-figure. | A2 |
| `mirror-pose` | Mirror selected pose (Figures) | src/rune_sec/command_palette.rs:1352 | COVERED — via `mirror_pose` | — |
| `insert-library-figure` | Insert library figure (Figures) | src/rune_sec/command_palette.rs:1353 | COVERED — via `figure_library_insert` | — |
| `add-segment` | Add segment (Segments) | src/rune_sec/command_palette.rs:1355 | COVERED — via `add_node` | — |
| `del-segment` | Delete segment (Segments) | src/rune_sec/command_palette.rs:1356 | COVERED — via `delete_node` | — |
| `len-up` | Lengthen segment (Segments) | src/rune_sec/command_palette.rs:1357 | COVERED — via `figure_scale` | — |
| `len-down` | Shorten segment (Segments) | src/rune_sec/command_palette.rs:1358 | COVERED — via `figure_scale` | — |
| `thick-up` | Thicken segment (Segments) | src/rune_sec/command_palette.rs:1359 | EQUIV — `set_node` — Explicit thickness. | — |
| `thick-down` | Thin segment (Segments) | src/rune_sec/command_palette.rs:1360 | EQUIV — `set_node` — Explicit thickness. | — |
| `cycle-shape` | Cycle segment shape (Segments) | src/rune_sec/command_palette.rs:1361 | EQUIV — `set_node` — Explicit shape. | — |
| `cycle-bend` | Cycle segment bend (Segments) | src/rune_sec/command_palette.rs:1362 | EQUIV — `set_bend` — Explicit bend. | — |
| `cycle-node-color` | Cycle segment colour (Segments) | src/rune_sec/command_palette.rs:1363 | EQUIV — `set_node` — Explicit colour override. | — |
| `attach-media-import` | Attach sprite from import folder (Segments) | src/rune_sec/command_palette.rs:1364 | COVERED — via `attach_image_to_node` | — |
| `clear-node-media` | Clear segment media (Segments) | src/rune_sec/command_palette.rs:1365 | COVERED — via `clear_node_media, clear_segment_text` | — |
| `cycle-attached-sprite` | Attach next sprite in table (Segments) | src/rune_sec/command_palette.rs:1366 | EQUIV — `attach_image_to_node` — Cycle = attach the next asset index explicitly. | — |
| `add-sprite` | Add sprite from assets (Segments) | src/rune_sec/command_palette.rs:1367 | COVERED — via `sprite_add` | — |
| `remove-sprite` | Remove last sprite (Segments) | src/rune_sec/command_palette.rs:1368 | COVERED — via `sprite_remove` | — |
| `cam-zoom-in` | Camera zoom in (Camera) | src/rune_sec/command_palette.rs:1370 | HANDLER-READY — `tool_camera_zoom_by` — A9 handler (editor.rs:5732); set_camera also writes zoom explicitly today. | A9 |
| `cam-zoom-out` | Camera zoom out (Camera) | src/rune_sec/command_palette.rs:1371 | HANDLER-READY — `tool_camera_zoom_by` — As cam-zoom-in. | A9 |
| `cam-frame-all` | Frame all figures (Camera) | src/rune_sec/command_palette.rs:1372 | COVERED — via `audio_trim_silence, delete_range` | — |
| `cam-centre` | Centre on selection (Camera) | src/rune_sec/command_palette.rs:1373 | COVERED — via `audio_trim_silence, centre_on_selection` | — |
| `cam-clear-key` | Clear camera key (Camera) | src/rune_sec/command_palette.rs:1374 | HANDLER-READY — `tool_clear_camera_key` — A9 handler (editor.rs:5693); delete_camera_key covers the effect today (with undo; the op is the panels no-undo form). | A9 |
| `cam-follow` | Toggle camera follow (Camera) | src/rune_sec/command_palette.rs:1375 | EQUIV — `set_camera_follow` — Explicit set supersedes toggle. | — |
| `cam-follow-clear` | Camera follow: clear (Camera) | src/rune_sec/command_palette.rs:1376 | EQUIV — `set_camera_follow` — figure: null clears (camera_panel::follow_clear effect). | — |
| `key-graph-angle` | Key graph angle (Graph) | src/rune_sec/command_palette.rs:1378 | COVERED — via `graph_key_angle` | — |
| `key-all-channels` | Key all channels here (Graph) | src/rune_sec/command_palette.rs:1379 | EQUIV — `graph_key_all_channels` — Registry tool keys all channels at the playhead. | — |
| `clear-graph` | Clear graph curves (Graph) | src/rune_sec/command_palette.rs:1380 | COVERED — via `graph_clear_channels` | — |
| `onion` | Toggle onion skin (View) | src/rune_sec/command_palette.rs:1382 | HANDLER-READY — `tool_set_onion_skin` — A9 handler (editor.rs:5843) toggles core.onion. | A9 |
| `draw-mode` | Toggle draw mode (View) | src/rune_sec/command_palette.rs:1383 | HANDLER-READY — `tool_set_draw_mode` — A3 handler (draw_ops.rs:1341) writes core.draw_mode. | A3 |
| `tween-play` | Toggle tween playback (View) | src/rune_sec/command_palette.rs:1384 | EQUIV — `set_tween_playback` — Explicit bool set. | — |
| `cycle-easing` | Cycle easing (View) | src/rune_sec/command_palette.rs:1385 | EQUIV — `set_project_easing` — Explicit easing. | — |
| `cycle-brush` | Cycle brush (Draw) | src/rune_sec/command_palette.rs:1387 | COVERED — via `cycle_brush` | — |
| `draw-tool` | Toggle brush / eraser (Draw) | src/rune_sec/command_palette.rs:1388 | EQUIV — `set_draw_tool` — Explicit tool set. | — |
| `clear-drawings` | Clear frame drawings (Draw) | src/rune_sec/command_palette.rs:1389 | COVERED — via `clear_frame_strokes` | — |
| `mirror-strokes` | Mirror strokes horizontally (Draw) | src/rune_sec/command_palette.rs:1390 | COVERED — via `mirror_strokes` | — |
| `stamp-polygon` | Stamp polygon (Draw) | src/rune_sec/command_palette.rs:1391 | EQUIV — `draw_polygon` — Stamp via the draw tool family. | — |
| `delete-last-stroke` | Delete last stroke (Draw) | src/rune_sec/command_palette.rs:1392 | COVERED — via `delete_stroke` | — |
| `stroke-select-next` | Select next stroke (Draw) | src/rune_sec/command_palette.rs:1393 | GAP — step_stroke_selection (stroke_inspector.rs) — shell_tools_selection has no stroke variant. | B6 |
| `stroke-select-prev` | Select previous stroke (Draw) | src/rune_sec/command_palette.rs:1394 | GAP — As stroke-select-next. | B6 |
| `particle-burst` | Add particle: Burst (Effects) | src/rune_sec/command_palette.rs:1396 | COVERED — via `particle_add_anchored` | — |
| `particle-sparkle` | Add particle: Sparkle (Effects) | src/rune_sec/command_palette.rs:1397 | COVERED — via `particle_add_anchored` | — |
| `particle-add-event` | Add particle event (Effects) | src/rune_sec/command_palette.rs:1398 | COVERED — via `audio_trim_silence, delete_range` | — |
| `fx-clip-confetti` | Add FX clip: Confetti (Effects) | src/rune_sec/command_palette.rs:1399 | EQUIV — `add_fx_clip` — kind confetti. | — |
| `fx-clip-lightning` | Add FX clip: Lightning (Effects) | src/rune_sec/command_palette.rs:1400 | EQUIV — `add_fx_clip` — kind lightning. | — |
| `clear-particles` | Clear particles and FX (Effects) | src/rune_sec/command_palette.rs:1401 | COVERED — via `audio_trim_silence, clear_particles` | — |
| `add-audio` | Add audio clip (Audio) | src/rune_sec/command_palette.rs:1403 | EQUIV — `add_audio_clip` — Path-explicit form of the picker flow. | — |
| `mixdown` | Mix down audio (Audio) | src/rune_sec/command_palette.rs:1404 | COVERED — via `audio_mixdown, audio_trim_silence` | — |
| `audio-echo` | Audio: echo last clip (Audio) | src/rune_sec/command_palette.rs:1405 | EQUIV — `audio_echo` — Index form of *_last (last index = the palette target). | — |
| `audio-lowpass` | Audio: lowpass last clip (Audio) | src/rune_sec/command_palette.rs:1406 | EQUIV — `audio_lowpass` — As audio-echo. | — |
| `audio-trim-silence` | Audio: trim silence (Audio) | src/rune_sec/command_palette.rs:1407 | EQUIV — `audio_trim_silence` — As audio-echo. | — |
| `audio-split-silences` | Audio: split at silences (Audio) | src/rune_sec/command_palette.rs:1408 | EQUIV — `audio_split_silences` — As audio-echo. | — |
| `fade-audio` | Audio: fade last clip (Audio) | src/rune_sec/command_palette.rs:1409 | COVERED — via `audio_fade` | — |
| `reverse-audio` | Audio: reverse last clip (Audio) | src/rune_sec/command_palette.rs:1410 | COVERED — via `audio_reverse` | — |
| `delete-audio-clip` | Audio: delete last clip (Audio) | src/rune_sec/command_palette.rs:1411 | COVERED — via `audio_delete_clip` | — |
| `export-png` | Export PNG sequence (Export) | src/rune_sec/command_palette.rs:1413 | EQUIV — `render` — format png. | — |
| `export-gif` | Export GIF (Export) | src/rune_sec/command_palette.rs:1414 | COVERED — via `render` | — |
| `export-mp4` | Export MP4 (Export) | src/rune_sec/command_palette.rs:1415 | COVERED — via `export_bundle, render` | — |
| `export-webm` | Export WebM (Export) | src/rune_sec/command_palette.rs:1416 | COVERED — via `export_bundle, render` | — |
| `export-piv` | Export .piv (Export) | src/rune_sec/command_palette.rs:1417 | EQUIV — `export_file` — format piv3 (the op writes v3). | — |
| `export-stk` | Export .stk (Export) | src/rune_sec/command_palette.rs:1418 | COVERED — via `export_file` | — |
| `enter-3d` | Enter 3D mode (3D) | src/rune_sec/command_palette.rs:1420 | COVERED — via `fig3_enter` | — |
| `exit-3d` | Exit 3D mode (3D) | src/rune_sec/command_palette.rs:1421 | COVERED — via `fig3_exit` | — |
| `go-animate` | Go to Animate workspace (Workspaces) | src/rune_sec/command_palette.rs:1424 | HANDLER-READY — `tool_shell_set_workspace` — B3 handler (shell_tools_state.rs:139). | B3 |
| `go-draw` | Go to Draw workspace (Workspaces) | src/rune_sec/command_palette.rs:1425 | HANDLER-READY — `tool_shell_set_workspace` — As go-animate. | B3 |
| `go-graph` | Go to Graph workspace (Workspaces) | src/rune_sec/command_palette.rs:1426 | HANDLER-READY — `tool_shell_set_workspace` — As go-animate. | B3 |
| `go-effects` | Go to Effects workspace (Workspaces) | src/rune_sec/command_palette.rs:1427 | HANDLER-READY — `tool_shell_set_workspace` — As go-animate. | B3 |
| `go-audio` | Go to Audio workspace (Workspaces) | src/rune_sec/command_palette.rs:1428 | HANDLER-READY — `tool_shell_set_workspace` — As go-animate. | B3 |
| `go-build3d` | Go to 3D workspace (Workspaces) | src/rune_sec/command_palette.rs:1429 | HANDLER-READY — `tool_shell_set_workspace` — As go-animate. | B3 |
| `go-export` | Go to Export workspace (Workspaces) | src/rune_sec/command_palette.rs:1430 | HANDLER-READY — `tool_shell_set_workspace` — As go-animate. | B3 |
| `go-home` | Go to Home workspace (Workspaces) | src/rune_sec/command_palette.rs:1431 | HANDLER-READY — `tool_shell_set_workspace` — As go-animate. | B3 |
| `toggle-guides` | Toggle stage guides (View) | src/rune_sec/command_palette.rs:1433 | HANDLER-READY — `exec_shell_set_setting` — SettingPatch::Guides (shell_tools_settings.rs:382). | B7 |
| `toggle-frustum` | Toggle camera frame guide (View) | src/rune_sec/command_palette.rs:1434 | HANDLER-READY — `exec_shell_set_setting` — SettingPatch::CameraFrame (shell_tools_settings.rs:385). | B7 |
| `toggle-safe-frame` | Toggle safe frames guide (View) | src/rune_sec/command_palette.rs:1435 | HANDLER-READY — `exec_shell_set_setting` — SettingPatch::SafeFrames (shell_tools_settings.rs:388). | B7 |
| `toggle-left-panel` | Toggle left panel (View) | src/rune_sec/command_palette.rs:1436 | HANDLER-READY — `tool_shell_set_section` — B8 handler (shell_tools_layout.rs:358). | B8 |
| `toggle-right-panel` | Toggle right panel (View) | src/rune_sec/command_palette.rs:1437 | HANDLER-READY — `tool_shell_set_section` — As toggle-left-panel. | B8 |
| `split-fx-clip` | Split FX clip at playhead (Effects) | src/rune_sec/command_palette.rs:1440 | COVERED — via `fx_split_clip` | — |
| `mirror-figure` | Mirror figure (all frames) (Figures) | src/rune_sec/command_palette.rs:1441 | COVERED — via `mirror_figure` | — |
| `normalise-audio` | Normalise audio levels (Audio) | src/rune_sec/command_palette.rs:1442 | COVERED — via `audio_normalise_all` | — |
| `duplicate-particle` | Duplicate particle event (Effects) | src/rune_sec/command_palette.rs:1443 | COVERED — via `particle_duplicate_event` | — |
| `delete-stroke` | Delete selected stroke (Draw) | src/rune_sec/command_palette.rs:1444 | COVERED — via `delete_stroke` | — |
| `delete-particle` | Delete selected particle event (Effects) | src/rune_sec/command_palette.rs:1445 | COVERED — via `particle_delete_event` | — |
| `mute-audio-clip` | Toggle mute on the selected audio clip (Audio) | src/rune_sec/command_palette.rs:1448 | COVERED — via `audio_clip_toggle_muted` | — |
| `swap-frame-prev` | Swap frame with previous (Frames) | src/rune_sec/command_palette.rs:1449 | COVERED — via `swap_frame_with_neighbour` | — |
| `swap-frame-next` | Swap frame with next (Frames) | src/rune_sec/command_palette.rs:1450 | COVERED — via `swap_frame_with_neighbour` | — |
| `shift-channel-right` | Shift graph channel keys +1 frame (Graph) | src/rune_sec/command_palette.rs:1451 | COVERED — via `graph_shift_keys` | — |
| `fill-keys-range` | Fill keys across the loop range (Graph) | src/rune_sec/command_palette.rs:1452 | COVERED — via `graph_fill_keys` | — |
| `toggle-live-mcp` | Toggle Live MCP (View) | src/rune_sec/command_palette.rs:1455 | HANDLER-READY — `exec_shell_set_setting` — SettingPatch::LiveMcp (shell_tools_settings.rs:358) + start/stop bridge (shell_tools_settings.rs:190). | B7 |
| `theme-light` | Switch to light theme (View) | src/rune_sec/command_palette.rs:1456 | HANDLER-READY — `exec_shell_set_theme` — B7 executor (shell_tools_settings.rs:553). | B7 |
| `theme-dark` | Switch to dark theme (View) | src/rune_sec/command_palette.rs:1457 | HANDLER-READY — `exec_shell_set_theme` — As theme-light. | B7 |
| `save-copy` | Save a copy of the project (Projects) | src/rune_sec/command_palette.rs:1458 | EQUIV — `save_project_copy` — Registry tool calls projects::save_copy. | — |
| `prev-keyframe` | Go to previous keyframe (Transport) | src/rune_sec/command_palette.rs:1461 | COVERED — via `audio_trim_silence, delete_frame` | — |
| `next-keyframe` | Go to next keyframe (Transport) | src/rune_sec/command_palette.rs:1462 | COVERED — via `audio_trim_silence, delete_frame` | — |
| `open-style-inspector` | Open style inspector (Style) | src/rune_sec/command_palette.rs:1471 | HANDLER-READY — `tool_shell_set_workspace + tool_shell_set_section` — Workspace switch + right column show. | B8 |
| `style-preset-ink` | Style preset: Ink (Style) | src/rune_sec/command_palette.rs:1472 | EQUIV — `apply_style_preset` — Preset by name/index. | — |
| `style-preset-crimson` | Style preset: Crimson (Style) | src/rune_sec/command_palette.rs:1473 | EQUIV — `apply_style_preset` — As style-preset-ink. | — |
| `style-preset-ocean` | Style preset: Ocean (Style) | src/rune_sec/command_palette.rs:1474 | EQUIV — `apply_style_preset` — As style-preset-ink. | — |
| `style-preset-forest` | Style preset: Forest (Style) | src/rune_sec/command_palette.rs:1475 | EQUIV — `apply_style_preset` — As style-preset-ink. | — |
| `style-preset-violet` | Style preset: Violet (Style) | src/rune_sec/command_palette.rs:1476 | EQUIV — `apply_style_preset` — As style-preset-ink. | — |
| `style-preset-amber` | Style preset: Amber (Style) | src/rune_sec/command_palette.rs:1477 | EQUIV — `apply_style_preset` — As style-preset-ink. | — |
| `style-cap-round` | Set segment cap: Round (Style) | src/rune_sec/command_palette.rs:1478 | COVERED — via `set_node_cap` | — |
| `style-cap-square` | Set segment cap: Square (Style) | src/rune_sec/command_palette.rs:1479 | COVERED — via `set_node_cap` | — |
| `style-cap-point` | Set segment cap: Point (Style) | src/rune_sec/command_palette.rs:1480 | COVERED — via `set_node_cap` | — |
| `cycle-taper` | Cycle segment taper (Style) | src/rune_sec/command_palette.rs:1481 | COVERED — via `cycle_node_taper` | — |
| `style-opacity-up` | Increase segment opacity (Style) | src/rune_sec/command_palette.rs:1482 | EQUIV — `set_node_opacity` — Stepper over the explicit write. | — |
| `style-opacity-down` | Decrease segment opacity (Style) | src/rune_sec/command_palette.rs:1483 | EQUIV — `set_node_opacity` — As style-opacity-up. | — |
| `copy-style` | Copy segment style (Style) | src/rune_sec/command_palette.rs:1484 | EQUIV — `get_node_style` — The documented copy half of the style copy/paste pair. | — |
| `paste-style` | Paste segment style (Style) | src/rune_sec/command_palette.rs:1485 | EQUIV — `set_node_style` — The documented write half of the pair. | — |

---

## Family 3 — panel-only actions

State-mutating actions that live in src/rune_sec/*.rs (button handlers,
steppers, drags, panel-local helpers the panels call). Pure builders,
read-only readouts and geometry fns are not actions and are excluded;
every excluded file was still read (design.rs, scenes.rs, snap_tests.rs,
status_strip.rs and toasts.rs contain no mutating actions).
87 action rows.

| Panel | Action | Where | Status | MCP tool / evidence | Owner |
|---|---|---|---|---|---|
| audio_panel.rs | Add clip (picker path) | src/rune_sec/audio_panel.rs:865 | EQUIV | add_audio_clip (path-explicit) | — |
| audio_panel.rs | Gain stepper on target clip | src/rune_sec/audio_panel.rs:1011 | EQUIV | audio_clip_set_gain (explicit); stepper handler-ready via tool_audio_clip_gain_step | A6 |
| audio_panel.rs | Echo/lowpass/trim/split/pitch/compress on target | src/rune_sec/audio_panel.rs:1034 | EQUIV | audio_echo / audio_lowpass / audio_trim_silence / audio_split_silences / audio_pitch / audio_compress (index forms) | — |
| audio_panel.rs | Fade/reverse/move/duplicate/normalise/delete row clip | src/rune_sec/audio_panel.rs:945 | COVERED | audio_clip_fade / audio_clip_reverse / audio_clip_move / audio_clip_duplicate / audio_clip_normalise / audio_delete_clip | — |
| audio_panel.rs | Crossfade / mixdown / split buttons | src/rune_sec/audio_panel.rs:1144 | COVERED | audio_crossfade / audio_mixdown / audio_split | — |
| camera_panel.rs | Zoom stepper (authored camera zoom) | src/rune_sec/camera_panel.rs:302 | HANDLER-READY | tool_camera_zoom_by (editor.rs:5732); set_camera writes zoom today | A9 |
| camera_panel.rs | Set key here / Hold / delete key at frame | src/rune_sec/camera_panel.rs:319 | COVERED | set_camera_key_here / hold_camera / delete_camera_key | — |
| camera_panel.rs | Clear key (no-undo form) | src/rune_sec/camera_panel.rs:1130 | HANDLER-READY | tool_clear_camera_key (editor.rs:5693) | A9 |
| camera_panel.rs | Paste key here (panel clipboard) | src/rune_sec/camera_panel.rs:541 | EQUIV | get_camera + set_camera; the clipboard itself is panel-local state | B6 |
| camera_panel.rs | Follow select / clear / cycle | src/rune_sec/camera_panel.rs:701 | EQUIV | set_camera_follow (incl. null = follow_clear) | — |
| camera_panel.rs | Camera dial drag | src/rune_sec/camera_panel.rs:822 | SHELL | Pointer gesture; effect via set_camera / rotate (tool_rotate_camera handler-ready, A9) | B5 |
| camera_panel.rs | View zoom steppers (view only) | src/rune_sec/camera_panel.rs:1237 | HANDLER-READY | tool_shell_view_zoom (shell_tools_view.rs:204) | B5 |
| clips_panel.rs | Loop mode / duplicate / rename / delete / move clip def | src/rune_sec/clips_panel.rs:341 | COVERED | set_clip_loop / clip_duplicate / clip_rename / clip_delete / clip_move | — |
| clips_panel.rs | Enter clip, session prev/next, add/delete session frame | src/rune_sec/clips_panel.rs:741 | COVERED | clip_enter / clip_goto / clip_add_frame / clip_delete_frame / clip_exit | — |
| clips_panel.rs | Insert figure into clip | src/rune_sec/clips_panel.rs:516 | COVERED | clip_insert_figure | — |
| dopesheet.rs | Row press (goto frame + select figure) | src/rune_sec/dopesheet.rs:429 | COVERED | set_frame / set_selection; shell form handler-ready (tool_shell_select_frame) | B6 |
| draw_tools.rs | Brush width/smoothing/scatter steppers | src/rune_sec/draw_tools.rs:279 | HANDLER-READY | tool_step_brush_param (draw_ops.rs:1429); set_brush covers preset choice today | A3 |
| draw_tools.rs | Nib set / apply nib to stroke | src/rune_sec/draw_tools.rs:425 | HANDLER-READY | tool_set_brush_nib / tool_apply_nib_stroke (draw_ops.rs:1529,1589); apply_nib covers nib-to-frame today | A3 |
| draw_tools.rs | Spray settings | src/rune_sec/draw_tools.rs:352 | HANDLER-READY | tool_set_stroke_spray (draw_ops.rs:1781) | A3 |
| draw_tools.rs | Stamp shape push | src/rune_sec/draw_tools.rs:425 | EQUIV | draw_shape / draw_polygon / draw_star / draw_arrow | — |
| draw_tools.rs | Simplify current frame | src/rune_sec/draw_tools.rs:642 | COVERED | simplify_strokes | — |
| draw_tools.rs | Clear strokes on frame | src/rune_sec/draw_tools.rs:769 | HANDLER-READY | tool_clear_strokes_on_frame (draw_ops.rs:1896); clear_frame_strokes covers the current frame today | A3 |
| easing_panel.rs | Project/frame easing presets | src/rune_sec/easing_panel.rs:171 | COVERED | set_project_easing / set_frame_easing | — |
| export_panel.rs | Run export (all formats + ranges) | src/rune_sec/export_panel.rs:674 | EQUIV | render / export_file (the panel fns are fixed-path forms) | — |
| export_panel.rs | Export bundle | src/rune_sec/export_panel.rs:696 | COVERED | export_bundle | — |
| figure_add.rs | Add figure entry (humanoid/quadruped/simple/library) | src/rune_sec/figure_add.rs:155 | COVERED | add_figure / figure_library_insert | — |
| figures_panel.rs | Row hide/lock/solo toggles | src/rune_sec/figures_panel.rs:759 | EQUIV | layers; direct toggles handler-ready (tool_figure_toggle_*) | A2 |
| figures_panel.rs | Row opacity stepper / cycle | src/rune_sec/figures_panel.rs:660 | HANDLER-READY | tool_figure_step_opacity (figure_ops.rs:1359); layers writes opacity today | A2 |
| figures_panel.rs | All / None / Invert visibility | src/rune_sec/figures_panel.rs:986 | COVERED | layers_set_all_visible / layers_invert_visibility | — |
| figures_panel.rs | Move/recolour/duplicate/delete row | src/rune_sec/figures_panel.rs:650 | COVERED | move_figure / figure_recolor / figure_duplicate / delete_figure | — |
| frame_ops.rs | Reverse/copy/paste/shift range buttons | src/rune_sec/frame_ops.rs:259 | COVERED | reverse_range / copy_range / paste_range / shift_frames (built-ins) | — |
| frame_ops.rs | Insert frames after current | src/rune_sec/frame_ops.rs:490 | COVERED | insert_frames | — |
| frame_ops.rs | Frame duration stepper | src/rune_sec/frame_ops.rs:547 | HANDLER-READY | tool_adjust_frame_duration (frame_swap.rs:512); set_frame_duration covers explicit values | A2 |
| fx_browser.rs | Save/apply FX preset, add preset clip | src/rune_sec/fx_browser.rs:671 | EQUIV | fx_preset_save / fx_preset_add / add_fx_clip | — |
| fx_clips.rs | Nudge start / resize / duplicate / delete / colours / anchor / kind / split | src/rune_sec/fx_clips.rs:793 | COVERED | fx_move / fx_resize / fx_duplicate / fx_delete / fx_set_color / fx_set_color2 / fx_set_anchor / fx_set_kind / fx_split_clip | — |
| fx_clips.rs | Toggle clip hidden | src/rune_sec/fx_clips.rs:1097 | COVERED | fx_set_hidden | — |
| graph_panel.rs | Channel selection | src/rune_sec/graph_panel.rs:871 | GAP | Panel-local selection state; shell_tools_selection has no graph-channel variant | B6 |
| graph_panel.rs | Set interp at playhead / drawn interp | src/rune_sec/graph_panel.rs:1110 | HANDLER-READY | tool_graph_set_interp_at_playhead / tool_graph_key_interp (graph_keys.rs:1475,1576); set_key_interp covers per-key writes | A4 |
| graph_panel.rs | Mirror/scale/reduce drags, key drag | src/rune_sec/graph_panel.rs:1187 | COVERED | graph_mirror_channel / graph_scale_channel / graph_reduce_keys / graph_move_key | — |
| home_panel.rs | Open project row | src/rune_sec/home_panel.rs:1663 | GAP | open_project_file — no tool, no handler | B9 |
| home_panel.rs | New project | src/rune_sec/home_panel.rs:1967 | COVERED | new_project | — |
| home_panel.rs | Recover snapshot | src/rune_sec/home_panel.rs:1947 | SHELL | Opens the recovery file shell-side; load_project covers path-explicit opens | B9 |
| home_panel.rs | Rename project file row | src/rune_sec/home_panel.rs:1808 | EQUIV | project_rename | — |
| inspector_figure.rs | Rename figure | src/rune_sec/inspector_figure.rs:541 | COVERED | figure_rename | — |
| inspector_figure.rs | Opacity stepper | src/rune_sec/inspector_figure.rs:838 | HANDLER-READY | tool_figure_step_opacity (figure_ops.rs:1359) | A2 |
| inspector_figure.rs | Head-ring scale stepper | src/rune_sec/inspector_figure.rs:900 | COVERED | set_head_ring_scale | — |
| inspector_figure.rs | Nudge figure root (path point) | src/rune_sec/inspector_figure.rs:438 | HANDLER-READY | tool_nudge_path_point (figure_ops.rs:1503) | A2 |
| inspector_segment.rs | Rotate/bend steppers on selected segment | src/rune_sec/inspector_segment.rs:393 | COVERED | set_pose / set_node / set_bend (explicit forms of the same writes) | — |
| inspector_segment.rs | Attach asset / clear media | src/rune_sec/inspector_segment.rs:687 | COVERED | attach_image_to_node / asset_attach / clear_node_media | — |
| inspector_style.rs | Colour/gradient/cap/opacity steppers + swatches | src/rune_sec/inspector_style.rs:328 | COVERED | set_node_style / set_node_color2 / set_node_cap / set_node_opacity | — |
| inspector_style.rs | Copy/paste style | src/rune_sec/inspector_style.rs:530 | EQUIV | get_node_style + set_node_style (the documented pair) | — |
| inspector_style.rs | Recent-colours list | src/rune_sec/inspector_style.rs:669 | GAP | Panel-local list state; applying a colour is set_node_style, the list itself is shell state | B8 |
| keyframe_tools.rs | Key/delete/nudge/clear/copy key at playhead | src/rune_sec/keyframe_tools.rs:656 | COVERED | graph_key / delete_keyframe / graph_move_key / graph_clear_channel_keys / graph_copy_keys | — |
| keyframe_tools.rs | Key all channels here | src/rune_sec/keyframe_tools.rs:1094 | EQUIV | graph_key_all_channels | — |
| layers_panel.rs | Move/jump layer, solo, opacity | src/rune_sec/layers_panel.rs:508 | EQUIV | move_figure / layers | — |
| library_panel.rs | Insert / save current figure | src/rune_sec/library_panel.rs:761 | COVERED | figure_library_insert / figure_library_save | — |
| library_panel.rs | Rename library entry (rename_saved) | src/rune_sec/library_panel.rs:945 | GAP | No figure_library_rename tool (pose_library_rename exists for poses only) | A8 |
| menus.rs | File menu recents | src/rune_sec/menus.rs:840 | GAP | Same open_project_file mechanism as G2 | B9 |
| menus.rs | Delete selected stroke/particle | src/rune_sec/menus.rs:735 | COVERED | delete_stroke / particle_delete_event (by index) | — |
| onion_controls.rs | Onion toggle | src/rune_sec/onion_controls.rs:204 | HANDLER-READY | tool_set_onion_skin (editor.rs:5843) | A9 |
| onion_controls.rs | Onion prev/next depth steppers | src/rune_sec/onion_controls.rs:204 | GAP | State.onion_ui depths; core half is cycle_onion_frames (G3) | A9 |
| particles_panel.rs | Add/delete/duplicate/step/anchor/nudge events | src/rune_sec/particles_panel.rs:326 | COVERED | particle_add_event / particle_delete_event / particle_duplicate_event / particle_set_frame / particle_set_anchor / particle_set_position / particle_set_kind / particle_set_color | — |
| playback_controls.rs | Play/pause/restart/range buttons | src/rune_sec/playback_controls.rs:504 | HANDLER-READY | tool_shell_play / tool_shell_pause (shell_tools_transport.rs:155,187); set_playback flips the core flag today | B4 |
| playback_controls.rs | Playback speed stepper | src/rune_sec/playback_controls.rs:603 | HANDLER-READY | SettingPatch::PlaybackSpeed via exec_shell_set_setting (shell_tools_settings.rs:394) | B7 |
| pose_tools.rs | Copy pose / write pose / apply to all / reset | src/rune_sec/pose_tools.rs:491 | COVERED | copy_pose_between / set_pose / pose_apply_all_frames | — |
| pose_tools.rs | Pose library save/apply/delete/rename/list | src/rune_sec/pose_tools.rs:710 | COVERED | pose_library_save / pose_library_apply / pose_library_delete / pose_library_rename / pose_library_list | — |
| project_tools.rs | New / save named / save a copy | src/rune_sec/project_tools.rs:613 | COVERED | new_project / save_named_project / save_project_copy | — |
| project_tools.rs | Open row / reload from disk | src/rune_sec/project_tools.rs:701 | GAP | open_project_file mechanism (G2) | B9 |
| project_tools.rs | Apply project name | src/rune_sec/project_tools.rs:586 | EQUIV | project_rename | — |
| scene_panel.rs | Weather kind/cycle/amount/wind + drags | src/rune_sec/scene_panel.rs:255 | COVERED | set_weather; finer-grained handlers handler-ready (tool_set_weather_kind/amount/wind, tool_cycle_weather) | A9 |
| scene_panel.rs | Stage size steppers/presets, background, fps step | src/rune_sec/scene_panel.rs:508 | COVERED | set_stage_size / set_project_background / set_project_fps | — |
| settings_popover.rs | Theme light/dark | src/rune_sec/settings_popover.rs:393 | HANDLER-READY | exec_shell_set_theme (shell_tools_settings.rs:553) | B7 |
| settings_popover.rs | Language cycle | src/rune_sec/settings_popover.rs:647 | HANDLER-READY | SettingPatch::Language via exec_shell_set_setting | B7 |
| settings_popover.rs | Guides / frustum / safe-frame toggles | src/rune_sec/settings_popover.rs:488 | HANDLER-READY | SettingPatch::Guides / CameraFrame / SafeFrames | B7 |
| settings_popover.rs | Loop playback toggle | src/rune_sec/settings_popover.rs:564 | HANDLER-READY | SettingPatch::LoopPlayback | B7 |
| settings_popover.rs | Live-MCP toggle | src/rune_sec/settings_popover.rs:647 | HANDLER-READY | SettingPatch::LiveMcp + bridge start/stop (shell_tools_settings.rs:190) | B7 |
| settings_popover.rs | Default easing cycle | src/rune_sec/settings_popover.rs:873 | HANDLER-READY | SettingPatch::DefaultEasing; set_project_easing covers the project value today | B7 |
| sprites_panel.rs | Add/delete instance, transform steppers, key add/delete | src/rune_sec/sprites_panel.rs:413 | COVERED | sprite_add / sprite_remove / sprite_set_key / sprite_delete_key | — |
| stage_view.rs / stage_layers.rs | Stage pointer drags (pose, draw, camera) | src/stage_view.rs | SHELL | Gesture machinery; every effect is covered (set_pose, set_node, draw_stroke, set_camera). Selection side handler-ready (B6) | B6 |
| stroke_inspector.rs | Width/opacity/scatter steppers on selected stroke | src/rune_sec/stroke_inspector.rs:1175 | EQUIV | set_stroke_style (by stroke index) | — |
| stroke_inspector.rs | Simplify/recolour/delete selected, frame and all | src/rune_sec/stroke_inspector.rs:1450 | COVERED | simplify_stroke / recolor_strokes / delete_stroke / delete_all_strokes / clear_frame_strokes | — |
| stroke_inspector.rs | Step stroke selection | src/rune_sec/stroke_inspector.rs:788 | GAP | step_stroke_selection — same mechanism as G4 | B6 |
| text_tool.rs | Add/update/move/nudge/resize/rotate/colour/delete/duplicate text | src/rune_sec/text_tool.rs:488 | COVERED | add_text_instance / update_text_instance / move_text_instance / nudge_text_instance / set_text_instance_size / rotate_text_instance / set_text_instance_color / delete_text_instance / duplicate_text_instance | — |
| three_d_panel.rs | All 3D panel actions incl. preview orbit drag | src/rune_sec/three_d_panel.rs:740 | COVERED | fig3_* tool family (25 tools) | — |
| timeline_marks.rs | Loop edge drags, Set loop / Clear buttons | src/rune_sec/timeline_marks.rs:602 | COVERED | set_loop_range / clear_loop_range | — |
| timeline_marks.rs | Ruler scrub drag | src/rune_sec/timeline_marks.rs:1030 | HANDLER-READY | tool_shell_set_playhead (shell_tools_transport.rs:228); set_frame covers the core move today | B4 |
| workspaces.rs | Workspace switching | src/rune_sec/workspaces.rs | HANDLER-READY | tool_shell_set_workspace (shell_tools_state.rs:139) | B3 |

---

## Handler-ready register (integrator splice list)

58 `tool_*` handler fns exist in domain files with no Tool entry, plus
B7's `exec_shell_*` executors. Registering them is splice work only:

- **A2** (figure_ops.rs, frame_swap.rs): tool_figure_step_opacity,
  tool_figure_toggle_hidden, tool_figure_toggle_lock,
  tool_figure_toggle_solo, tool_nudge_path_point,
  tool_adjust_frame_duration.
- **A3** (draw_ops.rs): tool_get_brush, tool_set_draw_mode,
  tool_step_brush_param, tool_set_brush_nib, tool_apply_nib_stroke,
  tool_smooth_stroke, tool_mirror_all_strokes (covers
  draw_ops::mirror_strokes_all), tool_set_stroke_spray,
  tool_clear_strokes_on_frame.
- **A4** (graph_keys.rs): tool_graph_key_interp,
  tool_graph_count_keys_in_range, tool_graph_set_interp_at_playhead.
- **A5** (fx_ops.rs, particle_ops.rs): tool_fx_set_seed,
  tool_fx_set_angle, tool_fx_set_length (over the new fx_set_seed /
  fx_set_angle / fx_set_length ops), tool_particle_set_seed.
- **A6** (audio_ops.rs): tool_audio_clip_gain_step.
- **A8** (media.rs, projects.rs): tool_sprite_sample,
  tool_delete_project_file.
- **A9** (editor.rs): tool_set_weather_kind, tool_cycle_weather,
  tool_set_weather_amount, tool_set_weather_wind, tool_clear_camera_key,
  tool_camera_zoom_by, tool_rotate_camera, tool_get_onion_skin,
  tool_set_onion_skin, tool_get_editor_settings, tool_clipboard_state.
  Note: the four weather handlers duplicate set_weather's surface —
  A9/integrator should register them as the fine-grained family or
  fold them, not both blindly.
- **B3** (shell_tools_state.rs): tool_shell_set_workspace,
  tool_shell_get_state.
- **B4** (shell_tools_transport.rs): tool_shell_play, tool_shell_pause,
  tool_shell_set_playhead, tool_shell_get_transport.
- **B5** (shell_tools_view.rs): tool_shell_view_zoom,
  tool_shell_view_fit, tool_shell_view_centre, tool_shell_view_reset,
  tool_shell_get_view.
- **B6** (shell_tools_selection.rs): tool_shell_select_figure,
  tool_shell_select_node, tool_shell_select_sprite,
  tool_shell_select_fx_clip, tool_shell_select_audio_clip,
  tool_shell_select_frame, tool_shell_get_selection.
- **B7** (shell_tools_settings.rs, executor form):
  exec_shell_set_theme, exec_shell_get_settings, exec_shell_set_setting
  (SettingPatch: Theme, LiveMcp, Language, DefaultEasing, Guides,
  CameraFrame, SafeFrames, LoopPlayback, PlaybackSpeed).
- **B8** (shell_tools_layout.rs): tool_shell_set_section,
  tool_shell_set_column_widths, tool_shell_get_layout.

Not a handler: dialogs.rs:89 `tool_runs` is a program-existence probe
for the native dialog helper — the name is a coincidence, recorded so
nobody "registers" it.

The shell_tools_* files are not yet declared in lib.rs and their
handlers take `&mut State` (not `&mut EditorCore`), so they can only be
served by the live endpoint through B2's ShellCommand queue; on stdio
they must refuse honestly (B1/B10).

---

## Gap ledger by owner

| Owner | Items |
|---|---|
| A9 | G3 cycle_onion_frames + onion depth steppers (Family 3). Extend tool_set_onion_skin or add a depth arg; onion_ui depths are State-side, so the shell half needs B plumbing. |
| A8 | G5 figure-library entry rename. Add figure_library_rename beside figure_library_save (pose_library_rename is the pattern). |
| B6 | G4 stroke selection stepping; G6 graph channel selection. Extend shell_tools_selection with stroke + graph-channel variants. |
| B8 | G7 style recent-colours list (panel-local state; expose read/write or declare it session-only in MCP.md). |
| B9 | G1+G2 project cycling + recent/Home open; DIALOG rows open_via_dialog / save_via_dialog owe named refusals, not tools. |
| B4 | SHELL rows: transport advance + the audio_play engine hooks — no tools wanted; the census records them so "full control" claims do not silently include the tick machinery. |
| A4 | Reconcile graph_keys::delete_keys_in_range vs delete_keys_in_range_at (two implementations of one capability; the tool uses the latter). |
| A5/A6/A2/A3 | No true gaps; their remaining rows are handler-ready splices above. |
| C | No Family 1–3 gaps: media.rs is 15/15 covered or handler-ready. Clip-def media rendering is C's charter work (docs/wave13/CLIP_MEDIA.md), tracked there, not a parity gap in this census. |

## Exclusions honoured

No row in this census asks for: project-swap undo/redo semantics,
accent colours, MCP-save write_recovery (note_recovery_written is
marked EXCLUDED), the waveform badge, or the stored-name divergence.

---

## Wave 14 close-out addendum (integrator, 2026-11-09)

This census is the wave's LAUNCH ledger: its GAP rows record
what was true when it was taken and are not rewritten. Their
dispositions at close-out, each enforced from source by gate
check 10 (PARITY) rather than by this table:

- **G1/G2 (project-file cycling/opening)** — CLOSED:
  `cycle_project_file` and `open_project_file` landed as core
  registry tools over the existing EditorCore ops (project-swap
  undo semantics untouched, per the standing exclusion).
- **G3 (`cycle_onion_frames`)** — DELIBERATELY UNTOOLED: the
  legacy op writes a field the stage no longer reads, so a
  tool for it would be a fake control. The real depths are
  tooled by `shell_set_onion_depths`.
- **G4 (stroke selection stepping)** — CLOSED:
  `shell_select_stroke` landed (with `shell_select_particle`
  and `shell_select_clip`); gate check 10 maps the palette's
  stroke-select commands to it plus `shell_get_selection`.
- **G5 (figure-library entry rename)** — REMAINS OPEN, named:
  the panel's rename is file-backed shell logic with no lib
  op beneath it; pose-side rename has a tool, library-entry
  rename does not. A future wave must first give the shell
  logic a lib home.
- **G6 (graph channel selection)** — REMAINS OPEN, named:
  channel selection is GUI cursor state; the graph tools
  address channels by explicit index, which is the same
  reach by a different handle.
- **Recent-colours list (inspector_style)** — panel-local
  list state; applying a colour is `set_node_style`. The
  list itself stays shell state by the G6 argument.

Gate check 10's PARITY_PENDING list is empty on the closing
tree: every one of the 134 palette commands maps to at least
one of 94 distinct tools or to a named exclusion above.

---

## Focused Wave addendum (post-Wave 14, 2026-11-09) — G5/G6/G7 CLOSED

The three remainders named above were closed by the Focused
Wave (11 agents, 5 squads; acceptance in
`docs/focused/ACCEPTANCE.md`):

- **G5 — CLOSED by `figure_library_rename`.** The rename
  logic moved to a lib home, `EditorCore::rename_library_entry`
  in `src/library_ops.rs` (the panel's `rename_saved` is now
  a one-line wrapper; the panel's pure helpers delegate to
  the lib copies). The tool is index-addressed
  (`{index, new_name}`), mirrors the panel's refusal strings
  verbatim, and — like `pose_library_rename` — is a file
  operation spending NO undo step and no dirty mark.
  MCP-only e2e: `tests/focused_e2e_library_rename.rs`.
- **G6 — CLOSED by `shell_select_graph_channel`** (live
  endpoint only): calls the graph panel's own
  `select_channel`, so the pick, target sync and figure
  switch are the row press exactly; it is the only handle
  that can name the second of two duplicate
  `(figure, target)` channels. `shell_get_selection` now
  also reports `graph_channel` (the pick) and
  `graph_channel_drawn` (what the panel actually draws).
- **G7 — CLOSED by `shell_get_recent_colours`** (live
  endpoint only, read-only): the style panel's session
  Recent list (thread-local, cap 8, not persisted). No
  write tool by design: applying a colour is
  `set_node_style`; a push tool would fabricate history.
  Honest limit: colours applied through MCP are not
  recorded in the list — only the panel's handlers note
  colours.

Also in the Focused Wave: the layout prefs D squad found
dead were wired for real — `timeline_collapsed` (new
`shell_set_timeline`; the timeline card renders
frame-chips-only while collapsed) and `sidebar_visible`
(the left column's persisted mirror; `sidebar_pref_wired`
now reports true) — and `set_weather`'s omitted
amount/wind now read the panel's one preset table
(`editor::eds_kind_preset`). Closing surface: **370
tools** (registry 317 + io 9 + built-ins 44); palette
parity unchanged (134 commands, all mapped — these gaps
were never palette commands, so gate check 10's tables
deliberately do not move).
