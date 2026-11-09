# Wave 11 — MCP end-to-end: acceptance matrix (E10)

**Role of this document.** This is the acceptance auditor's
census for Wave 11 ("MCP and MCP control end to end, point
to point, not a single thing missed"). It was written from
source, independently, at wave launch — before any other
squad's report existed — and it is the checklist the
integrator closes the wave against (§7). Where another
squad's work lands in the tree *during* the wave, this
document records the state at census time and says so;
the checklist items are written so they can be re-run at
close-out to confirm the landing.

**Census snapshot:** 2026-11-09 ≈06:10 +06, working tree
as found (concurrent Wave 11 edits were already landing
in satellite files — see §2.4; the three registry files
`src/mcp.rs`, `src/mcp_tools.rs`, `src/mcp_io.rs` were
in their wave-start state).

**Scope rule obeyed:** no cargo, no git, no code edits.
Every classification below carries file:line evidence on
both sides. Absence claims name the grep that produced
them; the greps are listed in §1 so the integrator can
re-run every one.

---

## 1. Method (reproducible)

All counts in this document come from five mechanical
passes over the source. Re-run them; if your numbers
differ, the tree moved — that is itself a finding.

1. **MCP surface.** Registry tools: string literals in
   `Tool { name: "…"` entries in `src/mcp_tools.rs`
   (91) and `src/mcp_io.rs` (8). Built-ins: the
   `tool_*()` calls in the `all_tools()` vec literal in
   `src/mcp.rs` (41), each mapped to its `name:` field.
   Total 91 + 8 + 41 = **140**. (Matches the wave-start
   claim exactly — the claim survives re-derivation.)
2. **EditorCore surface (as tasked).** Every
   `pub fn` in the production part of `src/editor.rs`
   (the file up to its `#[cfg(test)] mod`, line ≈4586)
   inside `impl EditorCore`: **185**. Satellite
   `impl EditorCore` blocks and free handler functions
   in other `src/*.rs` files are censused separately in
   §2.4 — the editor.rs-only scope understates the true
   surface, and §2.4 quantifies by how much.
3. **GUI reach.** Every `core.<fn>(` call site in
   production `src/rune_sec/*.rs` (52 files, excluding
   `snap_tests.rs` and `scenes.rs`): **497 call sites,
   171 distinct core operations**. Plus a second pass
   for mutations that bypass EditorCore entirely —
   direct `core.project.<field> =` / `.push` / `.clear`
   writes in the same files (this pass is what finds
   stage text and the background colour, §4 G2/G7).
4. **MCP reach.** The same `core.<fn>(` extraction over
   `src/mcp.rs` + `src/mcp_tools.rs` + `src/mcp_io.rs`:
   **95 distinct core operations** called. Set algebra
   against pass 3 gives: 88 ops called by both, 83
   called by the GUI but never by an MCP file, 7 called
   by MCP but not directly by panels. **A GUI-only
   method name is only a *candidate* gap** — §3/§4 then
   check each candidate for a semantically equivalent
   tool under a different name (absolute-set tools
   covering GUI steppers/cyclers, generic io tools
   covering per-format editor methods, and so on).
   Only candidates with no equivalent are GAPs.
5. **Palette.** Every `Command { key: "…"` entry in
   `src/rune_sec/command_palette.rs`: **114 commands**
   in 15 groups (Transport 8, Frames 10, Figures 10,
   View 12, Audio 11, Segments 9, Draw 9, Effects 9,
   Workspaces 8, Camera 7, Projects 6, Export 6,
   Graph 5, Edit 2, 3D 2).

**Classification vocabulary.** EXPOSED = a tool reaches
the capability (directly, or an absolute-set tool covers
a GUI stepper). COMPOSITION = reachable by combining
existing tools; no single op. PARTIAL = a tool reaches
most of it, with a named residue. VIEW-ONLY = pure
presentation state; must never be a tool (argument in
§5). SESSION-ONLY = host/session state; architecturally
unreachable by tools today (argument in §5). GAP = no
tool reaches it and nothing is written. GAP-HANDLER =
no tool reaches it, but a finished handler function
already exists in a domain file, unwired (§2.4).
UNRESOLVED = could not be classified; what would
resolve it is stated.

**One structural fact that decides many rows.** The MCP
handler signature is `fn(&mut EditorCore, &Value)`
(`src/mcp.rs`, `pub struct Tool`). Handlers receive the
*engine core only* — never the shell `State`. Anything
that lives in `State` (workspace, theme, panel layout,
the live-MCP toggle itself, menu/scrim state, scroll
positions) **cannot** be a tool without changing that
signature and the live/stdio dispatch contract. That is
the principled line between SESSION-ONLY and GAP: a
GAP is always something an `EditorCore`-only handler
*could* do today.

---

## 2. The MCP surface as found

### 2.1 Composition and tools/list

`all_tools()` (`src/mcp.rs:70`) is the single registry:
41 built-ins constructed inline, then
`tools.extend(crate::mcp_tools::tools())` (91), then
`tools.extend(crate::mcp_io::tools())` (8).
`tools/list` is served from `all_tools()`
(`tools_list_result`, `src/mcp.rs`), and both
transports route through the same dispatch: the stdio
binary via `McpServer::handle`, the live GUI control
path via `McpServer::handle_on` → same `dispatch`
(`src/live_mcp.rs:332–346` drains into
`router.handle_on(core, &msg)`).

**Verdict (cross-check i): PASS — no discoverability
gap.** Built-ins are listed, not callable-but-unlisted.
A client connected either way receives all 140 tools.
There is exactly one registry; the two transports
cannot drift apart in what they offer.

### 2.2 Name collisions and near-duplicates (cross-check ii)

Exact-duplicate census over all 140 names
(`collections.Counter` over the three families):
**zero duplicates, 140 unique names.** No tool shadows
another; `tools/call` lookup (`src/mcp.rs`, first match
in `all_tools()`) is therefore unambiguous today.

Near-duplicates — not collisions, but drift risks the
close-out should not add to:

| Pair / cluster | Evidence | Verdict |
|---|---|---|
| `save_project` (io, explicit path) vs `save_named_project` (built-in, `./projects/` by slug) vs `new_project` (registry) | `src/mcp_io.rs` tools(); `src/mcp.rs` `tool_save_named_project` (its description explicitly disambiguates) | Distinct ops, confusable names. Acceptable while descriptions keep disambiguating. |
| `get_state` (built-in) vs `get_project_stats` (registry) vs `project_summary` (io) vs `get_editor_status` (built-in) | `src/mcp.rs`; `src/mcp_tools.rs`; `src/mcp_io.rs:666` | Four overlapping snapshot tools. Fields overlap (project name, figures, frames). Not the same op, but the reporting surface has grown by accretion — a client must read four descriptions to know which snapshot holds a field. Drift watch. |
| `layers` (registry) | `src/mcp_tools.rs:3299` — "Apply layer state to a figure: locked, hidden, opacity, solo" | The most opaque name on the surface: nothing in the name says *figure*. It is the sole exposure for figure hidden/locked/opacity/solo (§3), so renaming is out of scope mid-wave, but the name is a discoverability tax. |
| `set_frame` (built-in) | `src/mcp.rs` `tool_set_frame` | Means "move the playhead", not "set a frame property" — sits beside `add_frame` / `delete_frame` / `set_frame_duration(s)`. Surprising but documented in its description. |
| `goto_next_key` vs `goto_next_keyframe` (both built-ins, Wave 10) | `src/mcp.rs` `all_tools()` tail | Deliberate pair (transport marker key vs pose keyframe); descriptions distinguish them. Highest mis-selection risk on the surface for an LLM client. |
| `duplicate_frame` (built-in) vs `figure_duplicate` vs `fx_duplicate` vs `particle_duplicate_event` vs `duplicate_graph_channel` (registry) | both files | Three verb placements for one concept. Unambiguous, inconsistent. Do not add a fourth pattern. |
| `render` (io) vs `export_file` (io) | `src/mcp_io.rs:471` (render: png/gif/mp4/webm/apng/svg/spritesheet/jpeg) vs `src/mcp_io.rs:353` (export_file: piv2–5/stk/nodes/stknds + mp4_range/webm_range) | Both can produce an MP4 (full-length via `render`, ranged via `export_file`). Overlap is real; descriptions carry the split. |
| `audio_normalise_all` (registry, -ise) vs `audio_normalize` (editor, -ize) | `src/mcp_tools.rs`; `src/editor.rs:3174` | Spelling drift in one name only. Cosmetic; note and move on. |
| `tool_list_brushes` defined **twice** in the unwired handler set | `src/draw_ops.rs:1081` and `src/paint.rs:1138` | **Splice-time collision** — see §2.4. The integrator must pick one; splicing both is a duplicate implementation waiting to drift. |

### 2.3 docs/MCP.md (cross-check iii — pre-wave document)

Checked against the doc **as it stands at census time**
(401 lines; A8 is rewriting it in parallel — this
verdict is about the pre-wave doc and must be re-run
against the rewrite at close-out). Method: every
backticked snake_case token in `docs/MCP.md` compared
against the 140 tool names.

**Verdict: PASS both directions.** All 140 tools are
named in the doc (140/140); no phantom tools — the
remaining snake_case tokens are argument names
(`at_frame`, `start_frame`), format enum values
(`mp4_range`, `webm_range`), module names (`mcp_io`,
`mcp_tools`, `mcp_server`) and channel targets
(`root_x`, `root_y`), not tool claims. Caveat: a
backtick census cannot catch a tool described only in
prose without backticks; no instance was spotted, but
the method's limit is stated so the close-out re-check
knows what it proves.

### 2.4 The unwired handler set — the wave's biggest fact

Squad B's pattern (handlers written in domain files,
integrator splices registry entries) means the tree at
census time contains a large *written but unreachable*
MCP surface:

- **91** `pub fn tool_*` handler functions across 10
  domain files: `audio_ops.rs` (10), `draw_ops.rs`
  (16), `fig3d.rs` (13), `figure_ops.rs` (15),
  `frame_swap.rs` (4), `fx_ops.rs` (2), `graph_keys.rs`
  (16), `paint.rs` (3), `particle_ops.rs` (7),
  `style.rs` (5).
- Of these, exactly **2 are wired** — as the
  implementations behind registry tools:
  `draw_ops::tool_draw_stroke` and
  `draw_ops::tool_simplify_stroke` (referenced from
  `src/mcp_tools.rs`).
- The other **89 are referenced from no MCP file**
  (grep of each handler name across `src/mcp.rs`,
  `src/mcp_tools.rs`, `src/mcp_io.rs`,
  `src/live_mcp.rs`, `src/bin/mcp_server.rs`: zero
  hits). They are also called from no panel. Until
  spliced, they change nothing a client can do:
  `tools/list` still returns exactly 140.

This is **not** counted as 89 gaps in §4 — the handlers
exist, so the work is splicing + naming + testing, not
writing. But every capability they cover is, at census
time, a GAP at the protocol surface, and the checklist
(§7) treats "splice and prove listed" as a distinct
step from "already exposed". The duplicate
`tool_list_brushes` (§2.2) lives in this set.

A naming warning for the splice: handler fn names are
not tool names. Several handlers will need tool names
chosen to avoid the near-duplicate patterns in §2.2
(e.g. `tool_audio_clip_normalise` → a per-clip
`audio_normalise` beside the existing
`audio_normalise_all`; `tool_graph_clear_channels` →
beside `delete_graph_channel`).

---

## 3. The universe and the matrix

### 3.0 Universe counts

| Universe | Count | Source |
|---|---|---|
| EditorCore `pub fn` in `editor.rs` (production) | **185** | §1 pass 2 |
| Satellite surface outside `editor.rs` | 86 `pub fn` in satellite `impl EditorCore` blocks across `audio_ops.rs` (30), `figure_ops.rs` (19), `fx_ops.rs` (13), `particle_ops.rs` (16), `audio_play.rs` (8); separately, 91 `pub fn tool_*` handler functions across 10 domain files (§2.4) — the two censuses overlap in method and membership; do not add them | §1 pass 2 note, §2.4 |
| Panel actions | 52 production files in `src/rune_sec/`; **497** `core.*()` call sites; **171** distinct core ops invoked; plus direct project-field mutations (pass 3b) | §1 pass 3 |
| Command palette commands | **114** | §1 pass 5 |
| MCP tools | **140** (91 + 8 + 41) | §1 pass 1 |

Set algebra (core ops): GUI∩MCP 88 · GUI-only 83 ·
MCP-only 7 · neither 47 (of the 185 editor.rs fns;
the residual is fns reached only via State wrappers,
satellite files, or genuinely unreached — each is
classified in §3.2–§3.9).

The 7 MCP-only ops are healthy, not drift:
`audio_set_gain_last` (the registry `audio_set_gain`
delegates to the per-clip "last" variant),
`create_clip_from_figure` (behind `clip_create`),
`delete_keyframe` / `goto_next_keyframe` /
`goto_prev_keyframe` (built-ins; panels reach them
through the dopesheet and keyframe tools' State
wrappers, not direct `core.` calls — an artefact of
the grep, verified in `src/rune_sec/dopesheet.rs` and
`src/rune_sec/keyframe_tools.rs`),
`mirror_figure_all_frames` (behind registry
`mirror_figure`), `particle_add_event` (behind the
built-in of the same name).

### 3.1 Frames & timeline

EXPOSED: `add_frame`, `duplicate_frame`,
`delete_frame`, `insert_inbetween`, frame durations
(`set_frame_duration`, `set_frame_durations`),
`set_frame_easing` / `cycle_frame_easing`
(`src/editor.rs:1940,2429,2452` → built-in
`set_frame_easing`), range ops (`shift_frames`,
`reverse_range`, `delete_range`, `copy_range`,
`paste_range` — all built-ins), `swap_frames`
(built-in), loop range (`set_loop_range` /
`clear_loop_range` built-ins), `goto` (built-in
`set_frame`), keyframe jumps and `delete_keyframe`
(built-ins), dopesheet press-to-seek (GUI) =
`set_selection` + `set_frame` (built-ins).
GAP-HANDLER: insert a run of blank frames
(`frame_swap.rs:360` `tool_insert_frames`), swap with
neighbour as one op (`frame_swap.rs:328`), frame/key
enumeration (`tool_list_frames`,
`tool_get_keyframes`, `frame_swap.rs:384,404`) —
today a client cannot even *list* frames with their
durations/easings except via `get_state`'s count.
PARTIAL: `advance` / `advance_play`
(`src/editor.rs:899,909`) are the GUI poll driver —
SESSION-ONLY as methods, but the intent is exposed
by built-in `set_playback` (play/pause/toggle/
restart/play_range, `src/mcp.rs` `tool_set_playback`).

### 3.2 Figures & poses

EXPOSED: add/duplicate/delete/move figure
(registry `add_figure`, `figure_duplicate`;
built-ins `delete_figure`, `move_figure` — the
built-in uses `move_figure_full`,
`src/editor.rs:1287`; the plainer `move_figure`
at :1875 is its superseded variant, not a gap),
colour (`figure_set_color`; editor `recolor_figure`
in `figure_ops.rs` is the same capability),
hidden/locked/opacity/solo (registry `layers`,
`src/mcp_tools.rs:3299` and handler at :1728 —
this one tool discharges `toggle_figure_hidden`
:1904, `toggle_figure_lock` :1378, `toggle_solo`
:1413, `cycle_figure_opacity` :1387,
`step_figure_opacity` :1399), segments
(`set_node`, `add_node`, `delete_node` — `set_node`
carries length/thickness/colour/shape, discharging
`adjust_length` :680, `adjust_thickness` :689,
`cycle_shape` :708, `set_node_color` :735), poses
(`get_pose` / `set_pose` — root + angles,
`src/mcp_tools.rs` descriptions at :2645/:2657 —
discharging `nudge_path_point` :1205 by COMPOSITION
and `copy_pose_between` :1660 by COMPOSITION),
mirroring (`mirror_pose`, `mirror_figure`),
`insert_inbetween`, quadruped/simple adders are
GUI figure-add paths whose result (a figure) is
reachable via `add_figure` + `set_pose` composition
— PARTIAL (no single "add quadruped" op; the rig
topology differs, so composition is laborious —
noted, not counted a gap).
GAP-HANDLER: figure rename, uniform scale, centre,
ground (`figure_ops.rs:587,615,747,764`), atomic
`tool_copy_pose_between` (`figure_ops.rs:779` —
the two-call composition above has no rig-size
guard parity with the panel op, Wave 10 D7),
`tool_reset_pose` (:822), pose-apply-to-all-frames
(:854), **the pose library** (list/save/apply/
delete/rename, `figure_ops.rs:1002–1128` — the GUI
pose library has no MCP surface at all today),
layers set-all-visible / invert-visibility
(`figure_ops.rs:1198,1215` — the panel's All/None/
Invert buttons; `layers` only addresses one figure).
GAP: **the whole-figure library** —
`insert_library_figure` (`src/editor.rs:2587`),
`save_figure_to_library` (:2602), `cycle_library`
(:2580), driven from `library_panel.rs` /
`figure_add.rs`. Grep for `library` across
`src/mcp_tools.rs` + `src/mcp_io.rs`: zero hits;
no `tool_*` handler exists in any domain file
(the `tool_pose_library_*` set above is poses, not
figures; `tool_fig3_*_library` is the 3D library,
which *is* exposed via built-ins). See G4.

### 3.3 Node style (cap / taper / colour2 / opacity / media)

EXPOSED: `set_node_cap` (discharges
`cycle_node_cap` :2157), `set_node_color2`
(gradient end; discharges `cycle_node_gradient`
:2184), `set_node_opacity` — all registry, handlers
calling `src/editor.rs:2205,2226,2252`.
GAP-HANDLER: **taper** — `cycle_node_taper`
(`src/editor.rs:2168`, `inspector_style.rs`) has no
tool: grep `taper` across `src/mcp.rs`,
`src/mcp_tools.rs`, `src/mcp_io.rs` returns nothing
on the tool side. Handlers written:
`style.rs:296` `tool_cycle_node_taper`, `:316`
`tool_set_node_taper`, `:352` `tool_set_node_style`
(the atomic cap+taper+colour2+opacity write the
panel's `set_node_style_atomic`, `src/editor.rs:
2281`, performs — today MCP needs up to three calls
and still cannot set taper), `:273`
`tool_get_node_style`, `:392`
`tool_apply_style_preset`.
GAP: **node media** — `attach_image_to_node`
(`src/editor.rs:2299`) and `clear_node_media`
(:2330). Grep `media|attach|image` across the three
MCP files: zero tool-side hits; no `tool_*` handler
exists. See G6.

### 3.4 Camera & graph

EXPOSED: camera keys (`set_camera` — centre, zoom,
**and rotation angle**, `src/mcp_tools.rs:2769`,
discharging `rotate_camera` `src/editor.rs:1311`
and `camera_zoom_by` :756), `delete_camera_key`
(built-in, discharging `camera_clear_key` :764),
`frame_all_figures`, `centre_on_selection`
(built-ins), `set_camera_follow` (built-in —
writes the field directly with its own snapshot,
discharging `toggle_camera_follow` :2071),
graph keys (`graph_key`, discharging
`key_graph_angle` :1425), `set_key_interp`
(discharging `set_graph_key_interp` :1527),
`smooth_graph_tangents`, `bake_graph`,
`graph_tangent_preset`, `mirror_channel_time`,
`scale_channel_values`, `graph_copy_keys`,
`graph_paste_keys`, `duplicate_graph_channel`,
`delete_graph_channel` (all registry).
GAP-HANDLER: the graph *enumeration and key-level*
set — `graph_keys.rs:894` `tool_graph_list_channels`,
`:928` `tool_graph_get_channel`, `:971`
`tool_graph_delete_key`, `:1033`
`tool_graph_move_key`, `:1082`
`tool_graph_set_tangents`, `:1097`
`tool_graph_shift_keys`, `:1111`
`tool_graph_fill_keys` (palette `fill-keys-range`),
`:1163` `tool_graph_delete_keys_in_range`, `:1208`
`tool_graph_clear_channel_keys`, `:1224`
`tool_graph_reduce_keys`, `:1245`
`tool_graph_mirror_channel`, `:1267`
`tool_graph_scale_channel`, `:1286`
`tool_graph_tangent_preset_channel`, `:1334`
`tool_graph_key_all_channels`, `:1356`
`tool_graph_key_angle`, `:1371`
`tool_graph_clear_channels`.
PARTIAL until spliced: `clear_graph`
(`src/editor.rs:1450` — clears **all** channels)
is reachable only by COMPOSITION (loop
`delete_graph_channel` at index 0); the one-call
handler above closes it. Note the panel also edits
graph keys by direct field writes
(`src/rune_sec/graph_panel.rs`, 32 direct-write
sites in pass 3b) — all of that capability is the
graph_keys.rs handler set's reason to exist.

### 3.5 FX, particles, weather

EXPOSED: FX clips (`add_fx_clip`, `fx_list`,
`fx_move`, `fx_resize`, `fx_duplicate`, `fx_delete`,
`fx_split_clip`, `fx_set_color`, `fx_set_kind`,
`fx_set_color2`, `fx_set_anchor` — registry;
`add_fx_clip` the tool discharges editor
`add_fx_clip` :2474 by construction in the handler),
FX presets (`fx_preset_save` discharging
`fx_save_preset` :2513; `fx_preset_add` discharging
`fx_apply_preset` :2552), weather (`set_weather` —
kind + amount + wind, `src/mcp_tools.rs:1089–
1114`, fully discharging the scene panel's weather
writes), particles (`particle_add_event` built-in,
`clear_particles` built-in, registry
`particle_duplicate_event`, `particle_set_kind`,
`particle_set_color`).
GAP-HANDLER: **particle event editing** — the
Particles panel's per-event ops
(`particle_ops.rs`: `particle_delete_event`,
`particle_set_frame`, `particle_set_anchor`,
`particle_set_position`) have no tools; handlers
written at `particle_ops.rs:491` (list), `:509`
(delete), `:527` (set frame), `:544` (nudge frame),
`:562` (set position), `:590` (set anchor), `:632`
(add anchored). Today a client can add/duplicate/
recolour an event but cannot move, retime, anchor
or delete a single one — only `clear_particles`
(all of them) exists.
GAP-HANDLER: **FX preset enumeration** —
`fx_preset_names` (`src/editor.rs:2573`) has no
tool; `fx_preset_add` errors when the preset does
not exist, yet a client cannot list the names that
do. Handler written: `fx_ops.rs:461`
`tool_fx_preset_list`.
GAP (in-flight fix, verify at close-out): **the
entering known gap — `FxInfo.hidden`.** During the
census the field landed in `src/fx_edit.rs:20–32`
("Wave 11: this field closes the known gap where
the MCP `fx_list` tool was blind to the flag"), but
grep `"hidden"` in `src/mcp_tools.rs` still shows
only *figure* hidden (lines 346, 1731, 1750) — the
`fx_list` output wiring and the setter
(`fx_ops.rs:439` `tool_fx_set_hidden`, unwired)
had not reached the registry at census time. See
G8.

### 3.6 Audio

EXPOSED: `add_audio_clip` (registry — discharges
editor `add_audio_clip` :3136 via the audio_ops
path), per-clip `audio_fade`, `audio_reverse`,
`audio_set_gain`, `audio_move_clip`,
`audio_delete_clip`, `audio_echo`, `audio_lowpass`,
`audio_trim_silence`, `audio_pitch`,
`audio_compress`, `audio_split_silences`,
`audio_set_muted` (registry — these discharge the
panel's `*_at` / `*_last` satellite ops in
`audio_ops.rs`), built-ins `audio_split`,
`audio_crossfade`, `audio_mixdown` (discharging
`mixdown_audio` :2341), `audio_normalise_all`
(registry).
GAP-HANDLER: the per-clip *management* set —
today a client cannot list clips or read one
clip's state except by side effect, cannot trim a
clip to a range (`audio_trim`, `src/editor.rs:
3219` — distinct from trim-silence), cannot
duplicate a clip (`audio_duplicate_clip_at`,
`audio_ops.rs:578`), and cannot normalise **one**
clip (`audio_normalize`, `src/editor.rs:3174`, is
per-clip; the only tool normalises all).
Handlers written for all of it:
`audio_ops.rs:1189` (list), `:1205` (info), `:1222`
(fade), `:1231` (reverse), `:1243` (set gain),
`:1263` (move), `:1282` (duplicate), `:1302`
(normalise one), `:1316` (trim), `:1344` (toggle
muted). Several duplicate already-exposed ops
(fade/reverse/gain/move/mute) — at splice time
these must become the *same* tools or be dropped,
not parallel duplicates (see §2.2 discipline).
SESSION-ONLY: the playback audio engine
(`audio_play.rs`: start/stop/sync on wrap/seek/
drift, `playback_audio_active`) — the GUI's
real-time output path. `set_playback` exposes the
transport intent; the sync machinery itself is
host behaviour, not project state.

### 3.7 Draw & strokes

EXPOSED: `draw_stroke` (registry → `draw_ops.rs:
643`, scatter arg included), `draw_shape`
(line/rect/ellipse only), `draw_polygon`,
`draw_star`, `smooth_strokes`, `erase_strokes`
(radius erase, `draw_ops` semantics),
`simplify_strokes`, `delete_stroke`,
`apply_nib`. The gesture session
(`begin_draw`/`draw_to`/`end_draw`,
`src/editor.rs:2769,2832,2867`) is SESSION-ONLY as
a *gesture* and discharged atomically by
`draw_stroke`.
GAP-HANDLER: nearly the whole panel's second
tier — `draw_ops.rs:577` list strokes, `:601`
drawing stats, `:697` **arrow** (`stamp_arrow`,
`src/editor.rs:3083` — `draw_shape` has no arrow
kind, so this is a genuine capability gap today),
`:758` set stroke style, `:904` fill stroke
(discharging `fill_current_stroke` :2921),
`:925` lasso erase (discharging `lasso_erase`
:2975 — the radius `erase_strokes` is not a
lasso), `:949` mirror strokes (discharging
`mirror_strokes_x` :3008), `:968` clear frame
strokes (discharging `clear_frame_drawings`
:2911), `:998` delete all strokes, `:1025`
recolor strokes, `:1081` list brushes, `:1118`
set brush, `:1152` cycle brush, `:1167` set draw
tool; plus `paint.rs:1174` draw spray, `:1245`
get stroke style, and the duplicate
`paint.rs:1138` list brushes (§2.2). The brush /
draw-tool *session* state (`active_brush` :3094,
`cycle_brush` :3108, `toggle_draw_tool` :3123,
`draw_tool_is_eraser` :3118) is GUI session state
— but Squad B has written handlers that treat it
as settable; classification GAP-HANDLER with a
note: at splice time, decide deliberately whether
MCP may steer the *user's live* brush (live path)
or whether these tools only make sense on the
stdio core. That decision is the integrator's;
the audit flags it rather than burying it.

### 3.8 Movieclips

EXPOSED: `clip_create` (registry — handler calls
`create_clip_from_figure`, `src/editor.rs:3968`),
`clip_list`, `set_clip_loop` (discharging
`cycle_clip_loop` :4011).
GAP (no handler anywhere): **the entire clip
editing session** — `enter_clip` (:3788),
`exit_clip` (:3809), `clip_add_frame` (:3860),
`clip_delete_frame` (:3883), `clip_goto` (:3904),
`clip_set_angle` (:3920), `clip_set_root` (:3935),
`clip_positions` (:3947, the session's readback),
`insert_clip_figure` (:3759), and
`make_clip_from_figure` (:3737) as distinct from
the create path. Grep `pub fn tool_` across
`src/*.rs` filtered to clip: only the audio-clip
handlers match — no movieclip handler exists.
The clips panel (`src/rune_sec/clips_panel.rs`)
is a full editor-within-the-editor; an automating
client can create a clip and then do nothing with
its insides. See G3. (Whether clip *deletion*
exists even in the GUI could not be confirmed from
the call census — no `delete_clip`-shaped core op
exists in `editor.rs`; marked UNRESOLVED in §6.)

### 3.9 Projects, files, export, 3D, misc

EXPOSED: io `save_project` / `load_project`
(discharging `save` :1776, `save_path` :1770,
`load` :1787), built-in `save_named_project`,
io `import_file` (piv/stk/nodes/nodemc/stknds/
json — discharging `import_piv` :2653,
`import_stk` :2674, `import_nodes_file` :2408,
`import_nodes_anim_file` :2130), io
`export_file` (piv2/3/4/5, stk, nodes, stknds,
mp4_range/webm_range — discharging
`export_piv` :2693, `export_piv2` :2718,
`export_piv4` :2730, `export_v5_file` :2384,
`export_nodes` :2742, `export_stknds` :2755),
io `render` (png/gif/mp4/webm/apng/svg/
spritesheet/jpeg — discharging `export_mp4`
:4147, `export_webm` :4158, `export_svg` :4169,
`export_sheet` :4180, `export_png` :4191,
`export_gif` :2620, `export_apng_file` :2394,
`export_jpeg_dir` :2401), `render_frame_base64`,
`render_contact_sheet`, `project_summary`.
PARTIAL: `.piv` import — `src/mcp_io.rs:176–181`
documents that MCP `.piv` import leaves the
previous project's undo history in place where
the editor's `import_piv` clears it. Named in
the io module's own comments; carried here so it
is not lost.
GAP: **saved-project management** —
`open_project_file` (`src/editor.rs:4116`) and
`cycle_project_file` (:4101) are reachable from
Home/project tools; a client can open a project
only if it already knows an exact path
(`load_project`). There is no tool to **list**
the saved projects in `./projects/`
(`projects::scan` is never called from any MCP
file — grep, zero hits), and none to rename or
delete a saved project (the Home panel's rename
flow has no MCP counterpart). No `tool_*`
handler exists in `projects.rs`. See G5.
SESSION-ONLY: `open_via_dialog` (:1713) /
`save_via_dialog` (:1745) — native OS dialogs;
the io tools' explicit paths are the correct MCP
shape. `maybe_autosave` (:1834) — engine
housekeeping driven by the shell poll.
`note_project_swapped` (:4057) — internal
bookkeeping, correctly not a tool.
3D — EXPOSED: `fig3_enter` (registry, discharging
`enter_3d` :4206), `fig3_select` (discharging
`fig3_select_next` :4234 by absolute index),
`fig3_rotate`, `fig3_orbit`, `fig3_bake`,
`fig3_mark_b`, `fig3_bake_animation` (registry),
3D library list/load/save (built-ins).
GAP-HANDLER: `exit_3d` (:4225) — **there is no
fig3 exit tool** (grep `fig3_exit|exit_3d` in the
MCP files: zero); handlers also written for get
session, select-next, library cycle, camera
presets/frame/zoom, IK, pose mirror and the 3D
pose library (`fig3d.rs:1899–2178`).
Project settings — EXPOSED: `set_project_fps`
(discharging `adjust_fps` :1946 stepping by
absolute set), `set_stage_size`,
`set_head_ring_scale`, `set_project_easing`
(discharging `cycle_easing` :2049),
`set_tween_playback` / `set_camera_follow`
(built-ins discharging the `toggle_*` ops :2063,
:2071), undo/redo (registry).
GAP: **stage background colour** — the scene
panel's `set_background` (`src/rune_sec/
scene_panel.rs:635`) writes `project.background`
directly; grep `background` across the MCP files
finds only a test struct literal
(`src/mcp_tools.rs:269`). No tool sets it, and
`get_state` does not report it (its field list:
project, figures, frame_count, current_frame,
fps, stage, weather — `src/mcp.rs`
`tool_get_state`). See G7.
GAP: **stage text** — see G2; classified here for
the misc row: `Project::texts`
(`src/model.rs:781`) is mutated only by
`src/rune_sec/text_tool.rs` via direct writes +
`core.snapshot()` (sites at :493–:758 and
beyond); no EditorCore op, no tool, no handler,
and no readback (`get_state` / `project_summary`
do not mention texts — grep `text` in the three
MCP files returns no tool).

---

## 4. The GAP list, ordered by user impact

Ordered by what an animator automating the editor
hits first. "Handler?" says whether the fix is
splice-work or write-work.

**G1 — The unwired 89 (meta-gap).** Handler: YES
(§2.4). Until the splice lands, every GAP-HANDLER
row in §3 is a live gap: particle retime/move/
delete, graph channel enumeration and key-level
edits, per-clip audio list/info/trim/duplicate/
single-normalise, node taper and atomic style,
figure rename/scale/centre/ground, the pose
library, FX preset listing, fig3 exit and camera
set, draw second tier (list/arrow/fill/lasso/
mirror/clear/recolour/brush/spray), frame
insert/list. This is the single highest-impact
item in the wave: the code largely exists; the
*registry* is what's missing. Splice discipline:
one tool per capability — where an unwired
handler duplicates an exposed tool (audio
fade/reverse/gain/move/mute), keep one.

**G2 — Stage text: total gap.** Handler: NO.
Project content (`Project::texts`) that a client
cannot create, edit, move, read or delete. The
GUI text tool is the only surface. An episode
with captions/titles cannot be automated at all.

**G3 — Movieclip session editing.** Handler: NO.
Create/list/loop exist; everything inside the
clip (enter, frames, angles, roots, exit) is
unreachable (§3.8). Clips are a headline feature
of the editor; automation stops at their door.

**G4 — Whole-figure library.** Handler: NO.
Insert/save/cycle library figures unreachable
(§3.2). (Pose library is G1; 3D library is
exposed — the 2D figure library is the odd one
out.)

**G5 — Saved-project enumeration & management.**
Handler: NO. No list of `./projects/`, no
open-by-name, no rename/delete (§3.9). A client
must know exact paths — the Home screen's basic
browsing is GUI-only.

**G6 — Node media (image attach/clear).**
Handler: NO. `attach_image_to_node` /
`clear_node_media` unreachable (§3.3).

**G7 — Stage background colour.** Handler: NO.
Cannot be set or even read via MCP (§3.9).
Small, but it is project content on every frame
of every export.

**G8 — `fx_list` hidden-flag blindness.**
Handler: PARTIAL (field landed in `fx_edit.rs`
during census; setter handler `fx_ops.rs:439`
unwired; `fx_list` serialisation unverified —
§3.5). The wave's entering known gap; it is
only closed when a client's `fx_list` result
shows `hidden` and a tool can set it.

**PARTIALs carried (not full gaps):**
`clear_graph` by composition until G1 lands;
quadruped/simple figure adds by laborious
composition (§3.2); `.piv` import undo-history
divergence (§3.9); `get_state` reporting omits
background, texts and weather parameters (kind +
amount + wind are returned by `set_weather` but
`get_state` reports weather as bool + kind only);
`edits_since_autosave` (`src/editor.rs:1819`)
not surfaced by `get_editor_status` (which does
report undo/redo depths and the status line —
the autosave counter itself is the residue).

---

## 5. VIEW-ONLY and SESSION-ONLY — the deliberate exclusions

These were audited and are **not** gaps. The
argument in every case is one of two shapes:
(a) the state is presentation, not project —
exposing it would let a client rearrange the
user's screen, which is the user's business; or
(b) the handler signature (`&mut EditorCore`,
§1) makes it architecturally unreachable without
a dispatch redesign nobody should smuggle into a
parity wave.

VIEW-ONLY: workspace switching (palette `go-*` ×8;
`workspaces.rs`), panel visibility
(`toggle-left-panel`, `toggle-right-panel`),
stage guides (`toggle-guides`, `toggle-frustum`,
`toggle-safe-frame`; `stage_camera.rs` view nav
and the camera pill's *view* zoom — distinct from
camera *keys*, which are exposed), theme
(`theme-light` / `theme-dark`), the GUI language
(`cycle_lang`, `src/editor.rs:2375`), onion-skin
configuration (`cycle_onion_frames` :4142 and
`onion_controls.rs` — onion *rendering* reads the
project; the skin count is a viewing aid),
figure thumbnails / list filtering / scroll
state in panels, menu and scrim state, toast
dismissal, the dopesheet lane's own collapse/
scroll (its *data* is exposed via the keyframe
ops).

SESSION-ONLY: pointer gestures (`begin_drag`
:335, `drag_to` :374, `end_drag` :405,
`begin_camera_drag` :368, `hit_test` :322 — the
absolute setters `set_pose` / `set_node` /
`set_camera` are the MCP shape of the same
intent); the draw gesture triple (§3.7);
playback's poll driver and audio sync (§3.1,
§3.6); native file dialogs (§3.9);
`toggle-live-mcp` (palette) — the toggle for the
very server a live client is connected through;
it lives in shell prefs, and a tool that can kill
its own transport is a footgun, not a feature;
`new` (:129, the constructor — `new_project` the
tool is its surface); `snapshot` / `undo` /
`redo` internals (the *ops* are exposed as
registry `undo` / `redo`; the snapshot stack
itself is engine plumbing); `positions` (:233),
`positions_f` (:1271), `display_frame` (:945),
`clip_positions` excepted via G3 — pure
render-path queries whose authored-state
counterparts (`get_pose`, `get_state`) are the
tools. See §6 for the one query that stays
UNRESOLVED.

---

## 6. UNRESOLVED

1. **Tweened display pose readback.**
   `positions_f` / `display_frame` return the
   *interpolated* pose at a fractional frame.
   No tool returns it (`get_pose` returns the
   authored pose at an integer frame). Is that a
   gap? It is read-only derived state — but so is
   `get_project_stats`. Resolves with: a product
   call on whether automation clients need
   display poses (e.g. for render verification),
   or a `render_frame_base64` screenshot being
   deemed sufficient (it exists — arguably the
   display pose's honest tool *is* the render).
2. **Movieclip deletion.** No `delete_clip`-
   shaped op was found in `editor.rs` or the
   clips panel's call census. Either clips
   cannot be deleted anywhere (a GUI gap, out of
   this wave's scope) or deletion rides another
   op the census grep did not attribute.
   Resolves with: a targeted read of
   `clips_panel.rs` handlers by the clips owner.
3. **fx_list hidden serialisation post-splice.**
   §3.5/G8: re-run the `"hidden"` grep on
   `src/mcp_tools.rs` *and* read the `fx_list`
   handler's output construction at close-out;
   the field's presence in `fx_edit.rs` does not
   by itself prove the tool emits it.
4. **Satellite-fn granularity.** The 86 satellite
   `pub fn`s (§3.0) were classified at domain
   granularity, not one row each; the panel-only
   helpers among them (`*_at` / `*_last` variants)
   are discharged by the per-clip tools as a
   class. If the integrator wants per-fn rows,
   the §1 greps reproduce the list in minutes.
5. **Brush/draw-tool steering on the live path**
   (§3.7): whether spliced brush tools should
   affect the user's live GUI session or only a
   stdio core is a policy call for the
   integrator/live-path owner, flagged here so
   the splice does not decide it by accident.

---

## 7. Integrator checklist

Close-out must resolve each item or carry it
over *explicitly* in the wave's close-out doc —
silence is not a carry-over.

**The splice (G1)**
- [ ] All 89 unwired handlers (§2.4) either
      spliced into a registry with a chosen tool
      name, or named here as deliberately dropped
      (with the reason).
- [ ] `tool_list_brushes` duplication resolved:
      exactly one of `draw_ops.rs:1081` /
      `paint.rs:1138` survives as the
      implementation.
- [ ] No parallel duplicates: unwired handlers
      covering already-exposed ops (audio fade /
      reverse / gain / move / mute) are the same
      tool as the existing one, not a second one.
- [ ] Post-splice census re-run (§1 pass 1):
      `tools/list` count and names recorded in
      the close-out; zero exact-duplicate names
      (§1 Counter check).
- [ ] Brush/draw-tool live-steering policy
      decided and written down (§6.5).

**Written-this-wave gaps**
- [ ] G2 stage text: tools for create / edit /
      move / delete / list stage texts, or an
      explicit carry-over.
- [ ] G3 movieclip session: enter/exit, clip
      frame add/delete/goto, clip angle/root
      editing — or an explicit carry-over.
- [ ] G4 figure library: insert / save (and
      list) library figures — or carry-over.
- [ ] G5 projects: list saved projects;
      open-by-name; rename/delete — or
      carry-over.
- [ ] G6 node media: attach / clear — or
      carry-over.
- [ ] G7 background: set + report (tool and/or
      `get_state` field) — or carry-over.
- [ ] G8 fx hidden: `fx_list` output verified to
      include `hidden` (§6.3 method) and the
      setter tool listed.

**Cross-checks to re-run at close-out**
- [ ] §2.1: `tools/list` still served from the
      single `all_tools()`; stdio and live
      surfaces identical.
- [ ] §2.2: Counter check — zero duplicate tool
      names across the (grown) surface; no new
      naming pattern introduced.
- [ ] §2.3: docs/MCP.md census re-run against
      **A8's rewritten doc** (this audit checked
      the pre-wave doc): every listed tool
      exists, every tool is listed.
- [ ] §3.0 set algebra re-run: GUI-only core-op
      count after the wave recorded; every
      remaining GUI-only op is VIEW-ONLY,
      SESSION-ONLY, COMPOSITION or PARTIAL with
      its §3/§5 argument, or appears above as a
      carried-over gap.
- [ ] PARTIALs (§4 tail): each either closed or
      carried over by name — `clear_graph`
      one-call op, `.piv` undo-history
      divergence, `get_state` reporting omissions.
- [ ] UNRESOLVED §6 items 1–2: product call /
      clips-owner read recorded, or carried
      over by name.
