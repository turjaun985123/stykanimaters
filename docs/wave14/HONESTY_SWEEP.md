# Wave 14 — Honesty sweep (E6), pass 1

One pass, from source, not from agent reports, over
every text in the tree that **claims MCP capability**:
registry descriptions in `src/mcp_tools.rs` and the
built-ins in `src/mcp.rs`, the new Wave 14 handler
specs riding in the domain files, in-app texts (help
rows, settings card), and the wave's own docs. The
deep row-by-row audit of `docs/MCP.md` is E3's; rows
there are cited, not re-audited — except where a false
claim found in a registry description is mirrored in
MCP.md prose, which is named here so it cannot slip
between the two audits.

**Sweep state note.** This pass ran while the wave
was in flight (2026-11-09, mid-integration day). The
tree moved during the sweep itself: E4's help-row
edit, the B squad's `shell_tools_*.rs` handler files,
D1/D4's `live_mcp.rs` work and A10's ledger all landed
while earlier sections were being checked. Every
verdict below is dated by that reality — "as found"
means as found at this pass, and anything whose truth
depends on the integrator's splice is marked
PENDING-SPLICE, never TRUE. A pass 2 at close-out
must re-run the FALSE/STALE rows and the PENDING
list against the closing tree.

**Counts at this pass: 3 FALSE, 2 STALE, 27 TRUE
(sampled, table below), the rest PENDING-REPORT or
PENDING-SPLICE (listed at the end).**

---

## Findings — FALSE and STALE (integrator applies the corrections)

### F1 — FALSE: the Help row already promises `shell_*` tools that do not exist yet

| Claim | Location | Verdict |
|---|---|---|
| "the live-only shell_* tools drive the window itself — workspace, playback, view, selection, theme and panels (the full tool list is in docs/MCP.md)" | src/rune_sec/help_panel.rs:570 (row), repeated in the pin tests at :1158 and :1240 | **FALSE as found** |

Evidence: `grep -c "shell_" src/mcp_tools.rs` = **0**;
`src/shell_control.rs` does not exist; the B squad's
handler files (`src/shell_tools_state.rs`,
`_transport.rs`, `_view.rs`, `_selection.rs`,
`_settings.rs`, `_layout.rs`) are in the tree but
unregistered and unwired — there is no queue, no
drain change, no registry entry; `docs/MCP.md`
contains no `shell_*` tool for the row's "full tool
list" pointer to land on. E4's edit is honestly
scoped in its remit (the row is the wave's target
truth), but as the tree stands the app instructs
users that window-driving tools exist when no client
can reach one.

**Disposition — release-blocking, the Wave 13 F1
pattern exactly:** B2's plumbing + B3–B9's handlers +
the integrator's registry splice must ALL land, or
this row (and both pins) must be pulled back in the
same commit. If pulled back, the corrective text is
the entering row, restored verbatim:

> "Settings — on by default, started at launch; the
> Live MCP control toggle lets MCP clients and other
> programs on this computer drive this running editor
> at http://127.0.0.1:32117/mcp — no password, so turn
> it off here when you do not want that"

### F2 — FALSE: "no GUI surface deletes project files" (×2, one mirrored)

| Claim | Location | Verdict |
|---|---|---|
| "Note there is deliberately no delete_project tool: no GUI surface deletes project files either." | src/mcp_tools.rs:5578 (`project_rename` description) | **FALSE** |
| "There is no `delete_project` tool — deliberately. No GUI surface deletes project files either (the projects panel can open, save, copy, duplicate and rename, never delete), so MCP has parity by omission. Delete files with your file manager." | docs/MCP.md:557-560 (E3's file — cited, corrective text supplied) | **FALSE** |

Evidence, both clauses: (1) the GUI **does** delete
project files — the Home panel's two-step
`home-proj-del-{i}` → confirm flow over
`shell::delete_project_file`
(src/rune_sec/home_panel.rs:93-98 module doc, call
site :1749; help_panel.rs's own module doc states
the same at src/rune_sec/help_panel.rs:112). (2) An
MCP handler for exactly that delete **exists**:
`tool_delete_project_file` (src/projects.rs:681),
written with its registry-entry spec at
src/projects.rs:655-668 — only the splice is missing
(see F5). The description's rationale is therefore
wrong in the entering tree already, and it now reads
as an argument against the very splice Wave 14's A8
remit calls for.

**Corrective text (integrator applies):**
`project_rename`'s description, final sentence
deleted — the description ends at "…has no saved
file." For docs/MCP.md (E3 applies): replace the
bullet at :557-560 with —

> - **`delete_project_file` deletes a saved project
>   file** from `./projects`, the Home panel's delete
>   over MCP: the argument must name a file
>   `list_projects` reports, shell-owned files are
>   refused, and the open project (if it is that file)
>   stays open in memory, exactly as in the panel.

(wording tracks the handler's own spec,
src/projects.rs:655-668 — E3 reconciles at merge.)

### F3 — STALE: `fig3_set_camera`'s registry description no longer names the args its handler reads

| Claim | Location | Verdict |
|---|---|---|
| "Set the 3D session camera absolutely: any of 'distance' (clamped to 40..=4000), 'target' [x, y, z] and 'fov_deg'; omitted fields keep their values…" | src/mcp_tools.rs:5415 (description + schema) | **STALE** |

Evidence: A7's fix is landed in src/fig3d.rs —
`tool_fig3_set_camera` (:914) now reads `yaw`
(:916) and `pitch` (:917) as well, pitch clamped to
±89.9°. The registry entry names neither, and its
schema has no `yaw`/`pitch` properties, so a client
reading the catalogue cannot discover the two args
the handler accepts. (The entering description was
the worse half of the same mismatch — it promised
"absolutely" while yaw/pitch were unreachable at
all; A7's handler fix is verified, the description
is the half that remains.)

**Corrective text:** A7 already wrote it — the TOOL
ENTRY UPDATE SPEC at src/fig3d.rs:889-912 (description
gaining `'yaw', 'pitch' (clamped to +-89.9)`, schema
properties `yaw`/`pitch`, all five args optional).
The integrator applies that spec at splice; no new
wording is needed from this sweep.

### F4 — STALE: the settings card still guesses at the start-failure cause D1 just made knowable

| Claim | Location | Verdict |
|---|---|---|
| "Live MCP did not start — port {} may be in use. Turn it off and on to retry." | src/rune_sec/settings_popover.rs:963 (persistent failure note) | **STALE** (hedged, not false) |

Evidence: D1 landed — `live_mcp::last_start_error()`
(src/live_mcp.rs:441, backed by `LAST_START_ERROR`
at :427) now records the real bind error, set on a
failed start (:430) and cleared on success (:434).
Nothing in the settings card reads it: the note
still prints a guessed cause ("may be in use") next
to a toggle whose bridge knows the actual error.
That is precisely the busy-port invisibility gap D1
was chartered to close; the settings half is D1's
splice spec + D9's pins, both PENDING at this pass.

**Corrective text (integrator applies at the D1/D9
splice):** the note renders the recorded error when
one exists, e.g. `format!("Live MCP did not start —
{}. Turn it off and on to retry.", err)` with
`err = crate::live_mcp::last_start_error()`, falling
back to today's text only when no error is recorded.

### F5 — No false claim, but an orphan: `delete_project_file`'s splice has been pending since Wave 12

`tool_delete_project_file` (src/projects.rs:681) and
its TOOL ENTRY SPEC (:655-668) entered in the Wave
12 B commit and were never spliced: no registry
entry (`grep delete_project_file src/mcp_tools.rs`
= 0 hits), no MCP.md row. No text claims the tool
exists, so nothing here is FALSE — but the handler
is unreachable dead surface, the gate's census
cannot see it (check 9 counts the registry), and F2's
description actively argues it should not exist.
Wave 14's A8 remit (projects.rs residual parity) and
E9's built-ins census must account for it by name:
**splice it in this wave, or record it as a named
remainder at close-out — do not let it ride a third
wave unremarked.**

---

## Verified claims — TRUE as found (sampled, evidence-named)

### Registry descriptions and built-ins (entering surface)

| # | Claim | Location | Evidence |
|---|---|---|---|
| 1 | `asset_remove` refusal remedy: "remove sprite instances with sprite_remove" | src/mcp_tools.rs:5638 | `tool_sprite_remove` registered (Wave 13 splice) and handled at src/media.rs (sprite family); this is Wave 13's F2 disposition, verified applied |
| 2 | `assets_list` reports "live reference counts split by kind (figure segment images, movieclip-def segment images, sprite instances)" | src/mcp_tools.rs:5599 | `asset_references` counts all three families (src/media.rs, incl. the clip-def row in its table at :434) |
| 3 | `set_tween_playback`: "One undo step when the value actually changes" | src/mcp.rs:1555 | Handler snapshots exactly once, inside the changed branch only (src/mcp.rs:1572-1582); Wave 13's double-count fix is in the body as commented |
| 4 | `set_playback`: "Over the live-control path the GUI's poll observes the flag, so playback and its audio start exactly as if the button had been pressed" | src/mcp.rs:1675 | Handler flips `core.playing` / `goto` on the same core the live drain serves (src/mcp.rs:1690-1723); the claim is scoped to the live path and makes no stdio promise |
| 5 | Brush-select description: "On the live transport this changes the editor's active brush" | src/mcp_tools.rs:4521 | Scoped to the live transport in the sentence itself; no claim about stdio |
| 6 | Description-wide arg audit: no description names an arg its schema lacks | All 237 entry blocks in src/mcp_tools.rs (scripted scan) | 9 flags raised, every one resolves to an arg VALUE or a reply field on a read: scope values 'frame'/'all' (`recolor_strokes`, props `color`/`scope`), 'brush'/'eraser' (`set_draw_tool` values of `tool`), 'yaw_wrapped' (`fig3_get` reply field), 'background' (`set_project_background` — `get_state`'s reply field) |

### New Wave 14 handler specs (in domain files; registry splice pending — verdicts are about the spec text vs its own handler)

| # | Claim | Location | Evidence |
|---|---|---|---|
| 7 | `fx_set_seed` / `fx_set_angle` / `fx_set_length`: "One undo step"; args `index` + the named value | Specs at src/fx_ops.rs:656-660, :689-693, :722-726 | Handlers pre-check the index, then call the op (src/fx_ops.rs:676, :709, :742); each op snapshots exactly once after its guard (ops at :456/:461, :477/:482, :500/:505) |
| 8 | `particle_set_seed`: "One undo step", refusal spends nothing | Spec at src/particle_ops.rs (handler doc) | Handler pre-checks index + seed range before the op; `particle_set_seed` op snapshots once (src/particle_ops.rs:348, snapshot at :356) |
| 9 | `audio_clip_gain_step`: "One undo step", reply reports the clamped gain | Spec at src/audio_ops.rs:1424-1446 | Handler validates, delegates to `audio_clip_gain_step_at` (:509) → `audio_set_gain_at` (the snapshotting setter), reads `gain` back from the clip (:1448-1462) |
| 10 | `sprite_sample`: "Read-only; no undo step", fractional frames allowed | Spec at src/media.rs (handler :1009) | Handler only calls the `sprite_sample` read; no snapshot in its body |
| 11 | `graph_key_interp`, `graph_count_keys_in_range`: "Read-only: no snapshot, no status write" | src/graph_keys.rs:1475, :1518 | Handler bodies contain no `snapshot()` and no status write; the count reads the addressed channel in place, exactly as the doc argues |
| 12 | `graph_set_interp_at_playhead`: "One undo step when it sets", status-only no-ops spend nothing | Spec at src/graph_keys.rs:1576 | Routes through `EditorCore::set_graph_key_interp`, whose no-op paths are status-only per the op contract the doc cites |
| 13 | A3 draw family: session tools claim "not an undo step" (`set_draw_mode`, `step_brush_param`, `set_brush_nib`); stroke tools claim one step only when a change happens (`apply_nib_stroke`, `smooth_stroke`, `mirror_all_strokes`, `set_stroke_spray`, `clear_strokes_on_frame`) | Specs at src/draw_ops.rs:1327, :1414, :1515, :1573, :1638, :1705, :1763, :1882 | Bodies match: `tool_set_stroke_spray` snapshots once per mutating path and spends none on the clear-a-markless-stroke reply (:1785-1890 read in full); `tool_clear_strokes_on_frame` snapshots only when the frame carries strokes (:1890-1921); `tool_apply_nib_stroke` validates fully before its single snapshot (:1588-1613) |
| 14 | `clear_camera_key`: "without an undo step… counts for dirty/autosave"; `camera_zoom_by` / `rotate_camera`: "One undo step" | Specs in src/editor.rs (A9 block) | `camera_clear_key` calls `mark_dirty()`, no snapshot (src/editor.rs:863-869); the handler refuses a keyless frame before the op (src/editor.rs:5693-5698) |
| 15 | Figure family: toggles + `figure_step_opacity` claim "Not an undo step… counts for dirty/autosave"; `figure_toggle_solo` claims "Session state only" | Specs in src/figure_ops.rs (A2 block) | `toggle_figure_hidden` → op with `mark_dirty()`, no snapshot (src/editor.rs:2124-2133); `step_figure_opacity` → `mark_dirty()` only (src/editor.rs:1538-1548); handlers at src/figure_ops.rs:1359, :1401 delegate without snapshotting |

### Shell-tool specs, in-app texts, transport, wave docs

| # | Claim | Location | Evidence |
|---|---|---|---|
| 16 | Shell specs uniformly claim "Live endpoint only" + "no undo step / never marks the project dirty" for selection, view, layout and transport tools | src/shell_tools_selection.rs:208-379, src/shell_tools_view.rs:195-396, src/shell_tools_layout.rs:344-421, src/shell_tools_transport.rs:147-255 | Internally consistent and honest about stdio (e.g. `shell_play`: "On the stdio server the flag is set but nothing advances it", shell_tools_transport.rs:147); `shell_get_layout` even discloses two prefs as "`wired`… currently false: those prefs round-trip but the shell does not read them yet" (:421). Verdict TRUE **as specs**; the tools themselves are PENDING-SPLICE (F1) |
| 17 | Help row: "MCP stdio server… works on its own copy of a project and never sees or drives this editor window" | src/rune_sec/help_panel.rs:565-567 | The stdio binary (`src/bin/mcp_server.rs`) owns its own core; D7's landed handshake test asserts only identity parity, no window path |
| 18 | Help rows: "Project assets" (six asset tools named; referenced asset cannot be removed) and "Sprites" (five sprite tools named; panel/stage/export claims) | src/rune_sec/help_panel.rs:554, :562 | All eleven tools registered with handlers as §1 verifies; the panel, stage outline/label and export rendering landed in Wave 13 and were re-verified at its close-out |
| 19 | Settings note: endpoint + "No password in this version" | src/rune_sec/settings_popover.rs:684-689 | Matches `live_mcp`'s posture (loopback, no auth — the design's stated contract, B1's SHELL_CONTROL.md keeps it) |
| 20 | Live health body derives its tool count | src/live_mcp.rs:1030 | `"tools": stykanimate_rs::mcp::all_tools().len()` — derived, not hardcoded; D4's test asserts body == served count (:1687) |
| 21 | Health makes **no** shell-control claim yet | src/live_mcp.rs (D4 block) | The `shell_control` field is deliberately ABSENT, pinned by a test ("no shell_control field until shell control has a truth source") — an honest omission, recorded here so pass 2 flips it when B lands |
| 22 | CLIP_MEDIA §7 addendum asserts nothing about the closing tree | docs/wave13/CLIP_MEDIA.md §7 | Marked **PENDING-INTEGRATION** in its own header, claims phrased as checkable anchors — honest as found |
| 23 | SPRITES.md clip-def cell hedged to the §7 flip | docs/SPRITES.md (C9 edit) | Cell now reads "never rendered… this cell is only true until that flip" — honest; C1's funnel fallback itself is in src/media.rs (:141-155, tests :2141, :2235) |
| 24 | B10's shell chapter + C9's clip-media rows declare their own merge status | docs/wave14/MCP_SHELL_CHAPTER.md (merge note), docs/wave14/CLIP_MEDIA_ROWS.md §0 | Both state plainly they are unmerged texts whose names the integrator/E3 reconcile — no shipped-truth claim |
| 25 | §6.9 keeps TODO-AT-CLOSE blocks; ACCEPTANCE carries the true entering baseline with closing counts left blank | docs/REDESIGN_PROGRESS.md §6.9; docs/wave14/ACCEPTANCE.md:23-27 | Baseline reads 288 (235 + 9 + 44), 3,319 tests, palette 134, keys 940 — all match the Wave 13 close; no delivered claim made |
| 26 | A10's palette ledger + B1's design doc frame themselves as map/design, with the entering surface stated as 288 | docs/wave14/PALETTE_PARITY.md, docs/wave14/SHELL_CONTROL.md | SHELL_CONTROL.md's "Why the shell is unreachable today" (§0) matches source: handlers take `&mut EditorCore` only (src/mcp.rs:53-60), drain at src/main.rs:520 |
| 27 | `delete_project_file` handler's own spec: "No undo step: a file operation, not a project edit" | src/projects.rs:655-668 (spec), handler :681 | Handler performs the guarded file delete + selector refresh, no snapshot — spec TRUE; the tool's reachability is F5 |

---

## PENDING list (not in the tree at this pass — pass 2 sweeps them on arrival)

**PENDING-REPORT (agent work not yet landed):**

- A1 `docs/wave14/PARITY_CENSUS.md` — absent; every
  later parity claim traces to it, so its numbers
  get a full re-check on arrival.
- C2 (stage), C3 (raster), C4 (SVG) clip-media
  surfaces — src/stage_view.rs, src/export.rs,
  src/svg.rs untouched at this pass; any text
  claiming clip media *renders* anywhere but the
  `media_items` funnel is unverifiable until they
  land (C1's funnel change alone does not prove a
  surface consumes it per the spec).
- C5 clip-media MCP handlers — none in src/media.rs
  yet (C9's proposed names in CLIP_MEDIA_ROWS.md are
  explicitly provisional).
- C7/C8/C10 clip tests + thumbnails — only
  tests/wave14_e2e_clip_media.rs present.
- D2 stdio↔live parity harness, D3 queue audit
  write-up, D5 robustness, D6 batch/notification,
  D8 lifecycle — src/mcp.rs/src/live_mcp.rs diffs at
  this pass cover D1/D4/D7 only.
- D9 settings pins + the F4 splice.
- E2 parity gate check, E3 the MCP.md deep audit
  (incl. F2's mirror), E9 built-ins census (incl.
  F5's accounting), E10 contract re-verification.
- A10's remaining rows beyond the landed ledger
  sections; E1's MCP-only build is present
  (tests/wave14_e2e_mcp_only_build.rs) but its
  claims are CI's to prove, not this sweep's.

**PENDING-SPLICE (handlers/specs in the tree, truth
waits on the integrator):**

- ALL Wave 14 registry entries: the 19+ landed
  `tool_*` handlers (A2/A3/A4/A5/A6/A8/A9 families)
  and every B-squad `shell_*` handler — zero are in
  `src/mcp_tools.rs` yet, so the catalogue still
  reads 288 and every spec verdict above is a
  spec-vs-handler verdict, not a shipped-truth one.
- F3's fig3_set_camera entry update (spec written,
  src/fig3d.rs:889-912).
- F5's `delete_project_file` entry (spec written,
  src/projects.rs:655-668).
- B2's `src/shell_control.rs` plumbing — absent;
  without it the shell handler files are dead code
  and F1's help row stays FALSE.
- D1's settings-note splice (F4).

---

## Close-out dispositions (integrator fills in)

- F1: ☐ B squad + splice landed, help row stands ·
      ☐ B slipped — row + pins pulled back to the
      entering text above, in commit: ______
- F2: ☐ description + MCP.md bullet corrected ·
      ☐ carried as a named remainder: ______
- F3: ☐ A7's entry-update spec applied at splice ·
      ☐ remainder: ______
- F4: ☐ settings note renders `last_start_error()` ·
      ☐ remainder: ______
- F5: ☐ `delete_project_file` spliced (registry +
      MCP.md row) · ☐ named remainder in §6.9: ______
- Pass 2: ☐ run against the closing tree; every
      PENDING row above swept or carried by name
      into §6.9's honest remainders.
