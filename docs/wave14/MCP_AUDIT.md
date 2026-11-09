# Wave 14 — docs/MCP.md full audit pack (E3)

Prepared by E3 for the integrator. **Nothing in this file is
merged**: `docs/MCP.md` is not edited by E3. Apply the punch
list below at the splice, in the order given (counts last,
once the registry is final), then delete or archive this
file per the wave's close-out habit.

**Freeze point:** 2026-11-09 04:50 UTC, working tree as the
squads left it (Wave 14 handlers written, **none spliced
yet** — `src/mcp_tools.rs` still holds the Wave 13 registry).
The tree was still moving while this audit ran (B6/B7/B8
files landed mid-audit); every count below is re-derived at
the freeze point and the method is stated so the integrator
can re-run it in seconds after the splice.

**Method.** (1) `python3 scripts/mcp_census.py` for the
callable surface. (2) A set diff of census names
(builtin/registry/io sections) against the first-cell names
of every table row in `docs/MCP.md`. (3) A sweep of every
`pub fn tool_*` / `exec_shell_*` definition in `src/`,
reconciled by hand against the census's unregistered list
(the census under-reports — see item D4). (4) Arg-read
sampling of handlers against their rows (§7).

---

## 0. Headline verdict

- **The entering catalogue is clean.** At the freeze point
  MCP.md documents exactly the 288 callable tools:
  **0 missing rows, 0 dead rows, 0 duplicated rows** for the
  current surface. Gate check 9's two-direction claim in the
  header is true today.
- **The punch list is therefore entirely forward-looking:**
  60 new tools land with this wave's splice and need rows
  (§2, §3), 1 existing row's args change (§4), 2 prose
  claims are falsified by a new tool (§5), the header counts
  and three section headings change (§1), and 4 spec-level
  defects need an integrator ruling while merging (§6).
- **One family has neither handler nor spec in the tree:**
  C5's clip-media tools (§6, item D5). Everything else the
  wave wrote carries a TOOL ENTRY SPEC.

**Punch-list size by kind:**

| Kind | Count | Where |
|---|---|---|
| Missing row (new registry tool) | 36 | §2 |
| Missing row (new shell tool) | 24 | §3 |
| Wrong args (existing row) | 1 (`fig3_set_camera`) | §4 |
| Stale prose falsified by a new tool | 2 | §5 |
| Count / heading corrections | 5 spots | §1 |
| Spec defects needing a ruling | 5 | §6 |
| Handler-without-spec gaps | 2 families | §6 D5–D6 |

---

## 1. Counts — corrected header (apply LAST, after the splice)

**Registration route (settles the count):** B1's design
(`docs/wave14/SHELL_CONTROL.md` §5, lines 525–532) rules
that the shell tools **are** registered in `all_tools()` —
one registry on both transports; on stdio their handlers
are the shared `shell_unavailable` stub answering an
`isError` result. Two squad specs contradict this in their
wording (B5: "live shell registry only, NOT the stdio
registry", `src/shell_tools_view.rs:192`; B6: "not
registrable in all_tools() as-is",
`src/shell_tools_selection.rs:267` and siblings) — B1's
design is the later, architectural word and B10's chapter
assumes it; the integrator splices all 24 shell tools into
`tools()` with stub handlers per B1 §5. If the integrator
instead keeps the shell family out of `tools()`, use the
alternative figures in the note under the table.

**Corrected counts (post-merge):**

| Family | Entering | Δ | Closing |
|---|---|---|---|
| Built-ins (`src/mcp.rs`) | 44 | — | **44** |
| Editing registry (`src/mcp_tools.rs`) | 235 | +36 core (§2) +24 shell (§3) | **295** |
| File IO / export (`src/mcp_io.rs`) | 9 | — | **9** |
| **Total (`all_tools()`)** | **288** | **+60** | **348** |

**Replacement text for MCP.md lines 18–27** (the
"## Tool count — 288, and how to check it" section):

> ## Tool count — 348, and how to check it
>
> At the time of writing the registry holds **348 tools**:
>
> | Source | Count | Where they are defined |
> |---|---|---|
> | Built-ins | 44 | `fn tool_*` constructors assembled in `all_tools()`, `src/mcp.rs` |
> | Editing registry | 295 | `pub fn tools()`, `src/mcp_tools.rs` |
> | File IO / export | 9 | `pub fn tools()`, `src/mcp_io.rs` |
> | **Total** | **348** | `all_tools()` = built-ins, then `mcp_tools`, then `mcp_io` |
>
> 24 of the registry tools are the **shell family**
> (`shell_*`): they drive the running editor's GUI shell
> and are live-endpoint only in effect — they are listed
> on both transports, but on stdio each answers an
> `isError` result saying it needs the live editor. See
> the shell catalogue table below.

**Also correct:**

1. MCP.md line 15 — "A tool behaves identically on both;
   only the session it edits differs." Now false for the
   shell family. Append: "— with one named exception: the
   24 `shell_*` tools act on the GUI shell and only take
   effect through live control (on stdio they answer an
   error result)."
2. Section heading "### Editing registry (235,
   `src/mcp_tools.rs`)" → "(295, `src/mcp_tools.rs`)".
3. New section heading before the §3 table:
   "### Shell control — live endpoint only (24,
   `src/shell_tools_*.rs`, registered in
   `src/mcp_tools.rs`)".
4. The catalogue-order sentence at line 36 ("The catalogue
   lists every tool by name, in registry order") stays
   true only if the shell table sits where the shell block
   sits in `tools()`; place the §3 table to match the
   splice position.

*Alternative if the shell family is NOT registered:*
registry 271, total 324 in `all_tools()`, plus 24
live-only shell tools documented separately (348 names in
the catalogue). E3 recommends against: it breaks B1 §5's
tools/list parity and D2's list-diff harness.

---

## 2. Missing rows — new registry tools (36)

All rows are the owning agent's own MCP.md ROW SPEC text,
verbatim except where marked ✎ (E3 normalisation). Place
each as its spec directs. Handler evidence is the `pub fn`
line; spec evidence is the TOOL ENTRY SPEC / ROW SPEC
comment immediately above it.

### A6 — audio (`src/audio_ops.rs`)

After the `audio_clip_set_gain` row (spec:
`src/audio_ops.rs:1427`):

| `audio_clip_gain_step` | Step the audio clip at 'index' gain by 'delta' (the audio panel's −/+ gain buttons): the stored gain is clamped to 0.0-2.0 by the op and the reply reports the gain read back after clamping, so a step past the ceiling truthfully reports 2.0 — the relative form of audio_clip_set_gain. One undo step. | `index`, `delta` | — |

Handler: `tool_audio_clip_gain_step`, `src/audio_ops.rs:1448`
(reads `index`, `delta` — verified against the handler body).

### A3 — draw (`src/draw_ops.rs`), draw-tools table

After `list_brushes` (spec `src/draw_ops.rs:1279`):

| `get_brush` | Read the ACTIVE brush preset in full (the Draw panel's preset readouts, as data): index, name, width, nib, nib angle, smoothing, opacity and scatter, plus the active draw tool, the eraser radius the preset sizes (twice its width) and the draw-mode flag. Read-only. | — | — |

After `set_draw_tool` (spec `src/draw_ops.rs:1331`):

| `set_draw_mode` | Set draw mode on or off (the Draw panel's Draw toggle / the palette's draw-mode command, as an idempotent set): in draw mode, dragging on the canvas paints a stroke on the current frame instead of posing. Session state only — not an undo step. | `enabled` | — |

After `cycle_brush` (spec `src/draw_ops.rs:1419`):

| `step_brush_param` | Step the active brush selection by one preset parameter (the stroke inspector's Preset size / opacity / scatter / smoothing steppers): selects the nearest library preset whose value for 'param' is strictly higher ('direction' 1) or lower (-1); the other parameters follow the chosen preset, because presets are bundles. Reports changed: false, with the panel's status text, when no preset steps that way. Session state only — not an undo step. | `param`, `direction` | — |

After `step_brush_param` (spec `src/draw_ops.rs:1519`):

| `set_brush_nib` | Select the active brush preset by its nib (the stroke inspector's Preset nib segmented control): the first library preset carrying nib 'round', 'square' or 'calligraphy' becomes active. Session state only — not an undo step. | `nib` | — |

After `apply_nib` (spec `src/draw_ops.rs:1579`):

| `apply_nib_stroke` | Give ONE stroke, by its project-wide index (see list_strokes), a nib width profile: "round" (uniform), "square" or "calligraphy" (angle-dependent, angle in degrees, default 45) — the single-stroke form of apply_nib, computed from the stroke's own width as the nib size. Replaces any per-point widths the stroke carried. One undo step. | `index`, `nib` | `angle` |

After `smooth_strokes` (spec `src/draw_ops.rs:1643`):

| `smooth_stroke` | Chaikin-smooth ONE stroke by its project-wide index (see list_strokes) — the single-stroke form of smooth_strokes: 'subdiv' corner-cutting passes (default 1, at most 4), endpoints pinned, per-point widths resampled onto the new points. Returns smoothed: false — and takes no undo step — when the stroke has fewer than 2 points and cannot change. One undo step when it smooths. | `index` | `subdiv` |

After `mirror_strokes` (spec `src/draw_ops.rs:1709`):

| `mirror_all_strokes` | Mirror EVERY stroke in the project, on all frames, across the vertical line x = 'cx' (default: the stage's vertical centre) — the project-wide form of mirror_strokes. Only points change. Errors on a project with no strokes. One undo step. | — | `cx` |

After `set_stroke_style` (spec `src/draw_ops.rs:1771`):

| `set_stroke_spray` | Edit one stroke's spray mark by its project-wide index (see list_strokes) — the field set_stroke_style cannot reach: 'radius' (> 0), 'opacity' (0-1) and/or 'seed' set the mark's fields, creating the mark on a plain stroke (defaults: radius 12, opacity 0.5, the deterministic begin_draw seed); 'clear_spray': true removes the mark instead, returning the stroke to a plain polyline. At least one field is required. Clearing a markless stroke reports changed: false and takes no undo step. One undo step when the mark changes. | `index` | `radius`, `opacity`, `seed`, `clear_spray` |

After `clear_frame_strokes` (spec `src/draw_ops.rs:1886`):

| `clear_strokes_on_frame` | Remove every stroke on the NAMED frame (the frame-addressed form of clear_frame_strokes, whose core op is current-frame-only): 'frame' is required and may name any frame, including the current one. One undo step when the frame carried strokes. | `frame` | — |

Handlers: `src/draw_ops.rs:1289, 1341, 1429, 1529, 1589,
1653, 1719, 1781, 1896` respectively.

### A9 — editor settings (`src/editor.rs`)

Beside `set_weather` (specs `src/editor.rs:5485, 5543,
5594, 5642, 5791, 5831, 5859`):

| `set_weather_kind` | Set the scene weather kind (the Scene panel's kind picker): switching kinds keeps the tuned amount/wind; enabling from off applies the panel's per-kind preset. A project-settings tweak — not an undo step; a real change marks the project dirty. | `kind` | — |
| `cycle_weather` | Cycle the scene weather (the Scene group's Cycle weather button): off → rain → snow → fireflies → off, each kind's preset applied wholesale; fog and storm are not in the cycle (cycling from either turns weather off). Not an undo step; a real change marks the project dirty. | — | — |
| `set_weather_amount` | Set the weather's particle amount absolutely (the Scene panel's Amount stepper/slider), clamped to 0..=200. Errors when the weather is off. Not an undo step; a real change marks the project dirty. | `amount` | — |
| `set_weather_wind` | Set the weather's wind (horizontal drift) absolutely (the Scene panel's Wind stepper/slider), clamped to -200..=200. Errors when the weather is off. Not an undo step; a real change marks the project dirty. | `wind` | — |
| `get_onion_skin` | Read the onion-skin master switch ('enabled'). The per-side depths are GUI shell state, not core state, and are not reported; 'legacy_prev_frames' is the core's legacy count the stage no longer reads. Read-only. | — | — |
| `set_onion_skin` | Turn the onion-skin master switch on or off (the Onion panel's toggle). Session view state: no undo step, no dirty mark — exactly like the panel toggle. | `enabled` | — |
| `get_editor_settings` | Read the editor-settings surface in one call: weather, background, project easing, tween playback, camera follow, loop range, onion switch, fps, stage size and head-ring scale. Read-only. | — | — |

Beside `set_camera` (specs `src/editor.rs:5678, 5718,
5758`):

| `clear_camera_key` | Remove the camera key at the current frame WITHOUT an undo step (the camera panel's "Clear key (no undo)" button): counts for dirty/autosave only. For the undoable delete at any frame, use delete_camera_key. Errors when the current frame holds no camera key. | — | — |
| `camera_zoom_by` | Multiply the camera zoom at the current frame by a factor (the camera panel's zoom buttons): upserts a key there, zoom clamped to 0.3..=4.0, angle preserved. One undo step. | `factor` | — |
| `rotate_camera` | Rotate the camera at the current frame by a degree delta (the camera panel's rotate buttons): upserts a key with the pan/zoom in effect and the angle wrapped to -180..180. One undo step. | `degrees` | — |

With the frame-clipboard tools (spec `src/editor.rs:6080`;
handler `tool_clipboard_state`, `src/editor.rs:6086`):

| `get_clipboard_state` | Read what the frame clipboards hold: 'frame' is null when the single-frame clipboard is empty, else the copied frame's figure count and duration; 'range_frames' is the range clipboard's frame count. A paste refuses when the clipboard is empty or the rig changed since the copy. Read-only; no undo step. | — | — |

### A2 — figure + frame (`src/figure_ops.rs`, `src/frame_swap.rs`)

In the figure/layers family (specs `src/figure_ops.rs:1355,
1394, 1430, 1462, 1496`):

| `figure_step_opacity` | Step a figure's opacity by 'delta' (the figures/layers panels' −/+ buttons): the target is rounded to whole percent and clamped to 0.05–1.0 by the op; the reply reports the stored opacity read back, so a step past a bound truthfully reports the bound. The relative form of the `layers` tool's opacity. Not an undo step (layers-family panel parity); a real change counts for dirty/autosave. | `delta` | `figure` |
| `figure_toggle_hidden` | Toggle a figure's hidden flag (the figures panel's hide button; the palette's Toggle figure hidden) — the relative form of the `layers` tool's 'hidden'. Not an undo step (layers-family panel parity); the flip counts for dirty/autosave. | — | `figure` |
| `figure_toggle_lock` | Toggle a figure's locked flag (the figures panel's lock button; the palette's Lock selected figure) — the relative form of the `layers` tool's 'locked'. Not an undo step (layers-family panel parity); the flip counts for dirty/autosave. | — | `figure` |
| `figure_toggle_solo` | Toggle a figure's solo state (the figures panel's solo button; the palette's Solo selected figure) — the relative form of the `layers` tool's 'solo'. Session state only: not an undo step and not a project edit. | — | `figure` |
| `nudge_path_point` | Nudge one figure's motion-path point: move the root of its pose at 'frame' by ('dx', 'dy') stage units — the stage's path-point drag as data; `figure_centre`/`figure_ground` compose this same primitive for their specific shifts. 'figure' defaults to the selected figure, 'frame' to the current frame. One undo step. | `dx`, `dy` | `figure`, `frame` |

Beside `set_frame_duration` (spec `src/frame_swap.rs:503`;
handler `tool_adjust_frame_duration`,
`src/frame_swap.rs:512`):

| `adjust_frame_duration` | Adjust a frame's duration multiplier by 'factor' (the frame-ops panel's Dur −/+/×0.5/×2 buttons): the stored duration is multiplied and clamped to 0.25–8.0 by the op; the reply reports the duration read back, so a step past a bound truthfully reports the bound. The relative form of `set_frame_duration`. Applies to the current frame unless 'frame' names another (which moves the playhead there first). Not an undo step (timing-tweak panel parity); a real change counts for dirty/autosave. | `factor` | `frame` |

### A5 — FX + particles (`src/fx_ops.rs`, `src/particle_ops.rs`)

Beside `fx_set_hidden` (specs `src/fx_ops.rs:658, 691,
724`; handlers `:676, 709, 742` — arg reads verified:
`index` + `seed`/`angle`/`length`):

| `fx_set_seed` | Set an FX clip's particle random seed — the value `fx_list` reports; the same kind with a different seed draws a different particle pattern. One undo step. | `index`, `seed` | — |
| `fx_set_angle` | Set an FX clip's emission angle in degrees — the value `fx_list` reports; the directed kinds (beam, trail, fountain) emit along it. One undo step. | `index`, `angle` | — |
| `fx_set_length` | Set an FX clip's length in stage units — beam reach / trail span (the value `add_fx_clip`'s `length` sets at creation and `fx_list` reports). One undo step. | `index`, `length` | — |

Beside `particle_add_anchored` (spec
`src/particle_ops.rs:722`; handler `:745`):

| `particle_set_seed` | Set a particle event's random seed — the value `particle_list_events` reports; the same kind with a different seed draws a different particle pattern. One undo step. | `index`, `seed` | — |

### A4 — graph (`src/graph_keys.rs`)

Beside `graph_get_channel` (spec `src/graph_keys.rs:1446`):

| `graph_key_interp` | Read the interpolation mode of the key at 'frame' on the channel at 'channel' index (nearest key within 0.5 frames — the graph panel's interp-control readout). 'interp' is null when no key sits there. Read-only. | `channel`, `frame` | — |

Beside `graph_delete_keys_in_range` (spec
`src/graph_keys.rs:1488`):

| `graph_count_keys_in_range` | Count the keys of the channel at 'channel' index inside [from, to], both ends inclusive — the read-only pre-flight for graph_delete_keys_in_range. Read-only. | `channel`, `from`, `to` | — |

Beside `set_key_interp` (spec `src/graph_keys.rs:1542`):

| `graph_set_interp_at_playhead` | Set the interpolation mode ("hermite"/"linear"/"step") of the graph key nearest the current frame on the selected figure's angle channel for the selected joint (EditorCore::set_graph_key_interp — the graph panel interp control's fallback). set_key_interp addresses any key by index. One undo step when it sets. | `interp` | — |

Handlers: `src/graph_keys.rs:1475, 1518, 1576`.

### A8 — media + projects (`src/media.rs`, `src/projects.rs`)

In the Wave 13 sprite block, beside `sprite_delete_key`
(spec `src/media.rs:986`; handler `tool_sprite_sample`,
`src/media.rs:1009` — reads `sprite`, `frame`, verified):

| `sprite_sample` | Read sprite 'sprite''s sampled transform at 'frame' (fractional frames allowed): the interpolated key between its authored keys, clamped outside the keyed range — the transform the stage and exports draw. Read-only; no undo step. | `sprite`, `frame` | — |

In the projects family, beside `project_rename` (spec
`src/projects.rs:670`; handler `tool_delete_project_file`,
`src/projects.rs:681` — reads `path`, verified):

| `delete_project_file` | Delete a SAVED project file from ./projects — the Home panel's delete, via MCP. 'path' names the file exactly as list_projects reports it (its full path or its bare file name); anything not in that listing is refused, so the tool cannot be aimed outside the library. The currently open project, if this is its file, stays open in memory, exactly as in the panel. No undo step: a file operation, not a project edit. | `path` | — |

---

## 3. Missing rows — shell family (24, NEW TABLE)

New table per §1 item 3. Rows are the agents' spec rows;
✎ marks E3 normalisations (B5's view specs were written in
a Tool | Args | Purpose layout — re-columned here into the
catalogue's Tool | Purpose | Required | Optional without
changing their words' substance; B8's widths row put
optional args in the Required column — corrected).

### B3 — state (`src/shell_tools_state.rs`)

Specs `src/shell_tools_state.rs:100` (workspace) and the
`shell_get_state` spec block following it; handlers take
`&mut State` (`tool_shell_set_workspace` :139,
`tool_shell_get_state` :296) and dispatch via B2's
shell-command channel with stub registry entries per B1 §5.

| `shell_set_workspace` | Switch the running editor's workspace by name: one of Animate, Draw, Graph, Effects, Audio, 3D, Export, Home (exact, case-sensitive). The workspace chip's own action, persistence included; an unknown name is refused, listing the valid set. Shell state only: no undo step. Live endpoint only. | `workspace` | — |
| `shell_get_state` | Read the running editor's shell state in one call: workspace; transport (playing/paused, frame, playhead, frame count, fps, loop, speed); view (view zoom/pan separate from the authored camera, plus the effective zoom); selections (figure, node, sprite, audio clip, FX clip, clip edit session); theme; collapsed panels. Read-only. Live endpoint only. | — | — |

### B4 — transport (`src/shell_tools_transport.rs`)

Specs/handlers at `:146/:155, :177/:187, :213/:228,
:254/:264`:

| `shell_play` | Start playback in the live editor (the Play button): the playhead starts from the current frame; the shell poll advances it and starts playback audio, as for a GUI play press. Idempotent. Not an undo step; never marks dirty. On stdio the flag is set but nothing advances it. ✎ (see D3) | — | — |
| `shell_pause` | Pause playback in the live editor (the Pause button): the playhead stays where it stopped; the shell poll stops playback audio. Idempotent. Not an undo step; never marks dirty. | — | — |
| `shell_set_playhead` | Move the playhead to a 0-based frame (a timeline click, `EditorCore::goto`): sets the current frame and snaps the fractional playhead onto it, playing or paused. Out-of-range frames are refused. The shell-family name for the set_frame built-in's move. Not an undo step; never marks dirty; selection untouched. | `frame` | — |
| `shell_get_transport` | Read the transport state: `playing`, current `frame`, fractional `playhead`, `frame_count`, project `fps` and `loop_range` (`[start, end]` inclusive or null). Read-only. The loop on/off toggle and playback speed are shell state and are not reported. | — | — |

### B5 — view (`src/shell_tools_view.rs`) ✎ re-columned

Specs/handlers at `:196/:204, :265/:273, :297/:305,
:359/:367, :394/:402`:

| `shell_view_zoom` | View-only zoom: `factor` multiplies the view zoom; `percent` sets the effective (camera × view) zoom %. Exactly one of the two is required. Clamped to the view range 0.1×–8×; reply reports the resulting view state + `clamped`. No camera key, no undo step, no dirty mark. Live shell only. | — | `factor`, `percent` |
| `shell_view_fit` | Pill Fit: view zoom 1×, pan zero (the camera's own framing). No camera key, no undo step, no dirty mark. Live shell only. | — | — |
| `shell_view_centre` | Pill Centre: pan the view so the selected figure's root (or the given stage point) sits at the element centre; clears the pan when no stage is laid out yet (figure form only). No camera key, no undo step, no dirty mark. Live shell only. | — | `x`, `y` |
| `shell_view_reset` | Pill 1:1: view zoom = 1 / camera zoom (effective zoom 100%), pan unchanged; `clamped` reported when the view range prevents exactly 100%. No camera key, no undo step, no dirty mark. Live shell only. | — | — |
| `shell_get_view` | Read the view state: `view_zoom`, `pan`, `at_camera_framing`, `camera_zoom`, `effective_zoom`, `effective_percent`, `frame`, `stage_element_size`, `stage_size`. Read-only. Live shell only. | — | — |

### B6 — selection (`src/shell_tools_selection.rs`)

Specs/handlers at `:200/:220, :232/:246, :262/:275,
:290/:302, :317/:329, :344/:361, :378/:392`:

| `shell_select_figure` | Select a figure in the GUI exactly as clicking its Figures-panel row (the node selection is cleared). Shell selection only: no undo step, never marks dirty. | `figure` | — |
| `shell_select_node` | Select a node of a figure in the GUI exactly as pressing that joint on the stage (figure and node selection both move). Shell selection only: no undo step, never marks dirty. | `figure`, `node` | — |
| `shell_select_sprite` | Select a sprite instance in the GUI exactly as clicking its Sprites-panel row. Shell selection only: no undo step, never marks dirty. | `sprite` | — |
| `shell_select_fx_clip` | Select an FX clip in the GUI exactly as pressing its FX-lane bar or panel row. Shell selection only: no undo step, never marks dirty. | `clip` | — |
| `shell_select_audio_clip` | Select an audio clip in the GUI exactly as pressing its Audio-panel row (the operations toolbar's target). Shell selection only: no undo step, never marks dirty. | `clip` | — |
| `shell_select_frame` | Move the GUI playhead to a frame exactly as a plain timeline-chip click (any range selection is cleared). Shell selection only: no undo step, never marks dirty. | `frame` | — |
| `shell_get_selection` | Read back every GUI selection (figure, node, sprite, FX clip, audio clip, playhead frame, timeline range) with the live counts. Read-only. | — | — |

### B8 — layout (`src/shell_tools_layout.rs`)

Specs/handlers at `:335/:358, :377/:393, :420/:430`:

| `shell_set_section` | Collapse or expand a panel section (the header Hide/Show control). Only Figures and Library are collapsible; a section outside the active workspace is refused. No undo step; session-only. Live endpoint only. | `section`, `collapsed` | — |
| `shell_set_column_widths` ✎ | Set the side-column widths (the splitters, by target). At least one of `left` / `right` is required; clamped to 220–420 / 260–480 and persisted via the layout prefs; the reply reports the read-back widths. No undo step. Live endpoint only. | — | `left`, `right` |
| `shell_get_layout` | Read the live layout: active workspace, visible sections + collapse state, hidden sections, column visibility and widths, and the (currently unwired) timeline/sidebar prefs with their `wired` flags. Read-only. Live endpoint only. | — | — |

### B7 — settings (`src/shell_tools_settings.rs`)

Specs at `:541, :579, :683`; executors
`exec_shell_set_theme` :553, `exec_shell_get_settings`
:591, `exec_shell_set_setting` :700 (State-taking, channel
dispatch as B3):

| `shell_set_theme` | Set the editor theme outright ('dark' or 'light') — the Settings card's Dark/Light buttons as a tool. Applies immediately and persists via the shell's layout-prefs write-back; accent colours untouched. Live endpoint only. No undo step (shell state). | `theme` | — |
| `shell_get_settings` | Read every user-facing setting the Settings card exposes, from its real home: theme, live-MCP (on/off, endpoint, persisted preference, pending stop, last start outcome), language, default easing, guides flags, loop/speed, workspaces, stage size, autosave status. Read-only. Live endpoint only. | — | — |
| `shell_set_setting` | Set one Settings-card boolean/enum setting by name (theme, live_mcp, language, default_easing, guides, camera_frame, safe_frames, loop_playback, playback_speed). Persists exactly as the card persists it; session-only flags say so. All immediate, no restart; live_mcp=false is applied just after the reply is delivered. Non-settable names are refused with the reason. Live endpoint only. No undo step. | `setting`, `value` | — |

---

## 4. Wrong args — 1 existing row

### `fig3_set_camera` (A7 amendment)

**Location:** MCP.md, editing-registry table, the
`fig3_set_camera` row. **Current text** lists Optional
`distance`, `target`, `fov_deg` only. **Evidence:** the
Wave 14 handler now also reads `yaw` and `pitch`
(`tool_fig3_set_camera`, `src/fig3d.rs:917` — arg reads
`yaw`, `pitch`, `distance`, `fov_deg`, `target` verified
in the body), and A7's registry spec widens the schema
likewise (`src/fig3d.rs:895–902`). The registry entry's
description text must be replaced with A7's spec text at
the same site. **Replacement row** (A7's ROW UPDATE SPEC,
`src/fig3d.rs:903` — amend, do not add a second row):

| `fig3_set_camera` | Set the 3D session camera absolutely: any of 'yaw', 'pitch' (clamped to +-89.9), 'distance' (clamped to 40..=4000), 'target' [x, y, z] and 'fov_deg'; omitted fields keep their values. At least one field is required. The absolute complement of fig3_orbit/fig3_camera_zoom. No undo step (session camera, like orbit/zoom). Requires an active 3D session. | — | `yaw`, `pitch`, `distance`, `target`, `fov_deg` |

No other existing row's args changed: the full
Wave 12/13 sample in §7 came back exact, and no other
squad spec amends an existing handler's arg set.

---

## 5. Stale prose — 2 claims falsified by `delete_project_file`

The classic rot, in prose rather than in a row's args:
Wave 14 A8's `delete_project_file` (§2) **is** "the Home
panel's delete, via MCP" (handler doc,
`src/projects.rs:672`) — so the GUI does delete project
files, and both of these MCP.md claims are now false.

1. **The `project_rename` row** ends: "Note there is
   deliberately no delete_project tool: no GUI surface
   deletes project files either." **Delete that sentence**
   from the row (the rest of the row is accurate), or
   replace with: "For deletion, see `delete_project_file`
   (the Home panel's delete)."
2. **Notes and deliberate limits**, the bullet "**There
   is no `delete_project` tool — deliberately.** No GUI
   surface deletes project files either (the projects
   panel can open, save, copy, duplicate and rename,
   never delete), so MCP has parity by omission. Delete
   files with your file manager." **Replace the whole
   bullet with:**

   > - **Deleting project files is `delete_project_file`.**
   >   The Home panel's delete, via MCP: it names a file
   >   from the `list_projects` listing only, so it cannot
   >   be aimed outside ./projects. There is still no
   >   tool that deletes the *currently open* project as
   >   a project operation — the file operation is the
   >   whole surface, exactly as in the GUI.

---

## 6. Spec defects & integrator rulings

- **D1 — Registration-route wording conflict (ruled in
  §1).** B5's spec says its tools are for the "live shell
  registry only, NOT the stdio registry"
  (`src/shell_tools_view.rs:192`); B6's says "not
  registrable in all_tools() as-is"
  (`src/shell_tools_selection.rs:267`); B4's and B8's say
  splice into `mcp_tools::tools()`. B1's design
  (`docs/wave14/SHELL_CONTROL.md` §5) settles it:
  register all 24 with `shell_unavailable` stub handlers.
  The B5/B6 spec comments stay wrong in the source until
  someone edits them — flagged here so the integrator
  does not "fix" the splice to match them.
- **D2 — B8 names B3's tools wrong.** The layout spec
  says "beside B3's `set_workspace` / `get_shell_state`"
  (`src/shell_tools_layout.rs:340`). No such names exist;
  B3's tools are `shell_set_workspace` /
  `shell_get_state`. Placement instruction only — the
  rows in §3 are unaffected.
- **D3 — `shell_play`'s stdio sentence.** B4's row says
  "On stdio the flag is set but nothing advances it"
  (spec `src/shell_tools_transport.rs:152`). Under B1 §5
  the stdio handler is the refusal stub, so on stdio
  nothing is set at all. Recommended row ending: "Live
  endpoint only — on stdio it answers the shell
  family's needs-the-live-editor error." The integrator
  picks one wording and applies it to all 24 shell rows'
  stdio claims consistently (B7's specs already use the
  refusal wording).
- **D4 — Census blind spots (for E2's gate work, not a
  doc defect).** `scripts/mcp_census.py` reported 53
  unregistered handlers at the freeze point; the true
  figure is 60. It misses handlers whose first parameter
  is not `&mut EditorCore`: `tool_shell_set_workspace`,
  `tool_shell_get_state` (`&mut State`),
  `tool_shell_get_selection` (`&ShellSelectionView`),
  the three `exec_shell_*` executors (`&mut State`), and
  `tool_clipboard_state` (fully-qualified
  `serde_json::Value` types, `src/editor.rs:6086`). Any
  gate built on the census alone will under-count the
  shell family by 7.
- **D5 — NO SPEC FOUND: C5 clip-media tools.** No handler
  and no TOOL ENTRY SPEC for clip-def media attach /
  clear / read exists anywhere in `src/` at the freeze
  point (the only `clip_media` hits are C7 test fixtures,
  `src/media.rs:2115, 2133, 2198`). C9's
  `docs/wave14/CLIP_MEDIA_ROWS.md` holds **proposed** rows
  for proposed names (`clip_asset_attach`,
  `clip_clear_node_media`, `clip_get_node_media`) —
  merge them only when C5's registration lands, renaming
  per C5's actual names as C9's §0 directs. Until then
  MCP.md must NOT gain these rows (the Maintainers
  section's rule: do not document a tool that does not
  exist).
- **D6 — NO SPEC FOUND: B9 file/menu shell actions.** No
  handler file and no specs for B9's remit (recent-project
  open etc.) exist in the tree at the freeze point. If B9
  lands after this audit, its rows are un-audited — the
  integrator collects them from B9's specs directly and
  re-runs the §0 diff before trusting the counts in §1
  (each B9 tool moves registry 295 and total 348 up by
  one).

---

## 7. Sample verification — existing rows vs handler arg reads

Every Wave 12/13 family, plus the Wave 14-adjacent
existing tools, sampled at the freeze point. "Reads" =
the arg names the handler body actually reads (helpers
followed where a read is delegated).

| Tool | Row (Required / Optional) | Handler reads | Verdict |
|---|---|---|---|
| `sprite_set_key` | `sprite`, `frame` / `x`, `y`, `scale`, `rotation` | same six (`src/media.rs`, `tool_sprite_set_key`) | ✓ exact |
| `sprite_delete_key` | `sprite`, `frame` / — | `sprite`, `frame` | ✓ |
| `sprite_add` | `asset` / — | `asset` | ✓ |
| `sprite_remove` | `sprite` / — | `sprite` | ✓ |
| `sprite_list` | — / — | none | ✓ |
| `asset_attach` | `figure`, `node`, `asset` / — | same three | ✓ |
| `asset_import` | `path` / `name` | `path`, `name` | ✓ |
| `asset_remove` | `index` / — | `index` | ✓ |
| `asset_rename` | `index`, `name` / — | `index`, `name` | ✓ |
| `audio_clip_set_gain` | `index`, `gain` / — | `index`, `gain` (`src/audio_ops.rs`) | ✓ |
| `add_fx_clip` | `figure`, `node`, `kind`, `start`, `duration` / `color`, `color2`, `length` | same eight (`src/mcp_tools.rs:913`) — no `seed`/`angle` at creation; the new `fx_set_*` tools are the write path | ✓ |
| `set_node` | `figure`, `node` / `length`, `thickness`, `color`, `shape` | same six | ✓ |
| `draw_stroke` | `frame`, `points` / `color`, `width`, `scatter`, `opacity` | same six (`src/draw_ops.rs`) | ✓ |
| `graph_get_channel` | `channel` / `frame` | `channel`, `frame` (`src/graph_keys.rs`) | ✓ |
| `graph_paste_keys` | `figure`, `target`, `keys`, `at_frame` / — | `figure`, `at_frame` directly; `target` via `parse_graph_target`, `keys` via `parse_keys` (`src/mcp_tools.rs:2163`) | ✓ (helper-mediated) |
| `set_weather` | `kind` / `amount`, `wind` | same three | ✓ |
| `particle_set_anchor` | `index` / `figure`, `node` | same three (`src/particle_ops.rs`) | ✓ |
| `clip_set_angle` | `node`, `degrees` / — | same two | ✓ |
| `pose_library_apply` | `name` / `figure`, `frame`, `mirror` | same four (`src/figure_ops.rs`) | ✓ |
| `layers` | `figure` / `locked`, `hidden`, `opacity`, `solo` | same five | ✓ |
| `new_project` | — / `name`, `fps`, `width`, `height` | same four | ✓ |
| `set_frame_duration` | `frame`, `duration` / — | same two | ✓ |
| `fig3_set_camera` | — / `distance`, `target`, `fov_deg` | `yaw`, `pitch`, `distance`, `target`, `fov_deg` | ✗ → §4 |

Header cross-check: MCP.md's stated counts (288 = 44 +
235 + 9, lines 18–27) match the census output at the
freeze point exactly; the §1 figures are the post-splice
correction. Section headings "(44, `src/mcp.rs`)" and
"(9, `src/mcp_io.rs`)" need no change.

## 8. False positives checked and cleared

- `tool_text` (`src/rune_sec/stroke_inspector.rs:788`) is
  a UI readout helper returning a `String`, not an MCP
  handler — no row owed.
- The six `name:` literals in `src/mcp_io.rs` beyond its 9
  tools (`mp4`, `webm`, `svg`, `png_sequence`,
  `jpeg_sequence`, `spritesheet`) are render-format
  descriptors, not tools — no rows owed (the `render` row
  covers the formats).
- `shell_state_json` / `settings_snapshot` /
  `view_state_json` are reply builders behind the §3
  read tools, not tools themselves.
