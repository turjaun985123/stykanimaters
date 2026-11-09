# Wave 13 — B1: The Dirty Census

**What this is.** A census of every operation in the library and the
binary that writes serialised project state **without** taking a
snapshot, ruled one by one: each row gets a verdict — `snapshot` or
`mark_dirty` — with the reason, and an anchored patch specification
the owning Wave 13 agent (or the integrator) can follow mechanically.

**Date / tree state.** Audited 2026-11-09 against the Wave 13
entering tree. Wave 13 runs in parallel: several owning agents
(B2, B3, B9) landed their fixes *while this census was being
written*, following the same Squad B contract. Each row therefore
records the **entering** behaviour ("Snapshots today" = at audit
start) and a **State at census close** note saying whether the
patch had already landed in the tree when this file was finished.
Rows marked OPEN are the integrator's remaining work list. Line
regions are as at census close; they will drift — the function
name is the durable anchor.

**The defect being censused (Wave 12 remainder E8).** Entering
Wave 13, `EditorCore::edit_count` advanced in exactly one place:
inside `snapshot()`. The dirty dot (chrome and status strip, both
reading `edits_since_autosave() > 0`) and the 25-edit autosave
cadence (`AUTOSAVE_EVERY`) therefore saw *only* snapshotting
edits. Any mutation that deliberately — or accidentally — skipped
the snapshot was invisible: a session could be changed, never
marked dirty, and slip past the autosave cadence entirely.

---

## 1. The mechanism, in one page

In `src/editor.rs`:

- `snapshot()` (entering lines ~259–270) serialises the whole
  `Project` to JSON, pushes it on the undo stack (capped at 60),
  clears redo, and increments `edit_count` by exactly 1. It is
  the house rule made code: **one user action = at most one
  snapshot = one undo step = one counted edit.**
- `edit_count` / `last_autosave_count` (entering lines ~48–53):
  `edits_since_autosave()` is their saturating difference;
  `note_recovery_written()` re-baselines after a save/autosave;
  `maybe_autosave()` fires at 25 edits.
- Entering test `autosave_counter_counts_snapshots_only`
  (editor.rs tests) pins the defect honestly: three snapshots
  count 3; `cycle_node_color` and `cycle_figure_opacity` count
  **nothing**, by design of the old code.

**The seam the census rules against** (Squad B contract; B2's
deliverable in `src/editor.rs`, landed during the census at
line ~2021, beside `edits_since_autosave`):

```rust
pub fn mark_dirty(&mut self) { self.edit_count += 1; }
```

`mark_dirty()` advances the counted-edit counter — dirty dot
and autosave cadence see the change — **without** pushing an
undo step. It is the honest completion of the sentence the old
code left half-said: "this write is not undoable" was allowed to
imply "this write does not exist". After Wave 13 the house rule
is: **a project mutation either snapshots exactly once or calls
`mark_dirty()` — never neither.** Callers mark only when a value
actually changed; refusals and no-ops spend nothing.

## 2. Verdict rules

- **`snapshot`** — the operation makes a discrete, serialised
  project-content change; comparable peer operations already
  take exactly one snapshot; and undoability does not break a
  documented interaction discipline. The snapshot goes **after**
  validation and no-op checks, **before** the first write.
- **`mark_dirty`** — the write is real serialised project state
  (so it must feed the dirty dot and the cadence), but an undo
  step would be wrong: the source deliberately treats the op as
  a non-undoable setting tweak, appearance cycler/stepper, or
  continuous/session pose tweak — or it is a primitive already
  used underneath another caller's single-snapshot umbrella.
  Mark **once per user action**, only on an actual change.
- **No row (covered)** — the function only reads; writes
  session/runtime state rather than `Project`; is a history or
  wholesale-swap operation; exports/saves without changing the
  in-memory project; or its caller takes the single snapshot
  for the same user action. §6 lists the audited covered
  families so the gap list is not mistaken for the whole
  mutation surface.

## 3. How I searched (completeness is checkable)

1. Read the Wave 13 roster header + Squad B contract, and the
   `Project` definition (`src/model.rs`, entering lines
   ~727–793). Its serialised fields are the census checklist:
   `name`, `fps`, `stage`, `background`, `figures`, `frames`,
   `camera`, `tween_playback`, `easing`, `camera_follow`,
   `particles`, `strokes`, `audio`, `clips`, `fx_clips`,
   `assets`, `channels`, `sprites`, `texts`, `weather`,
   `head_ring_scale`.
2. Grepped all 97 `.rs` files under `src/` for: `snapshot()`
   occurrences per file; `edit_count` /
   `edits_since_autosave` / `last_autosave_count`; direct
   assignments into `project` / `core.project` /
   `self.project` / `s.core.project`; collection mutations
   (`push`, `pop`, `insert`, `remove`, `clear`, `retain`,
   `drain`, `swap`, `sort`, `extend`, `append`, `truncate`)
   on every `Project` table; nested field writes (`.name =`,
   `.color =`, `.locked =`, `.hidden =`, `.opacity =`,
   `.path =`, `.gain =`, `.start_frame =`, …); functions
   taking `&mut EditorCore` / `&mut Project` / `&mut self` in
   mutation-heavy modules; and comments saying "no snapshot",
   "without snapshotting", "not undoable", "dirty".
3. Ran a scratch static analyser over the production code
   (test modules excluded): it extracted function spans,
   flagged project-chain assignments and collection mutations,
   and recorded whether the body text contained `snapshot()`.
   ~750 candidate lines. The analyser is a net, not a proof:
   it has false positives (pure helpers, read-only rendering,
   caller-snapshot umbrellas) and false negatives (snapshots
   taken by a callee or a gesture-entry function).
4. **Manually read the body of every plausible candidate** and
   its production callers, and cross-checked the panels' own
   documented write disciplines. Every row below rests on a
   body I read, not on the analyser's flag.
5. Re-grepped at census close, because B2/B3/B9 were editing
   the same tree in parallel; the "state at close" notes and
   all line regions come from that final pass.

**Scope exclusions, stated:** test code and headless scene
fixtures (they assign project fields to *build* fixtures, not
as user operations); the Wave 13 A-squad sprite surface
(`sprite_ops.rs`, `stage_sprites.rs`, `sprites_panel.rs`),
which did not exist in the entering tree and is being built
under this same contract — the integrator should spot-check it
against §2 at splice time; import builders that construct a
fresh `Project` off-line (e.g. `nodes_anim.rs` conversion),
whose live-session boundary is the excluded project-swap path.

## 4. Premise corrections (the brief's entering assumptions, corrected)

1. **`project_rename`'s name write is not in `projects.rs`.**
   `projects::rename_project` (~line 494) is a file primitive
   with no `EditorCore`: it moves the file on disk and
   deliberately does not change the name stored inside the
   JSON. The serialised-state write is the *session* name
   assignment in `src/mcp_tools.rs::tool_project_rename`.
   The patch anchors there (row C5). The stored-name-after-
   rename read divergence is a standing excluded product
   decision — only the dirty marking is in scope.
2. **`clip_set_angle` / `clip_set_root` are not FX-clip ops.**
   They are movieclip-*session* pose tweaks in `src/editor.rs`;
   nothing in `src/fx_ops.rs` writes them. B4's in-tree audit
   (fx_ops.rs ~lines 54–99) confirms: the FX-clip family
   already complies (§6). Owner of the clip rows is B2.
3. **The figure-colour gap is `set_figure_color`, not
   recolour.** `figure_ops::recolor_figure` already snapshots
   exactly once before rewriting every node's colour. Do not
   "fix" it. (B10's e2e suite pins it as SNAPSHOT — correct.)
4. **`exit_clip` writes no project state.** It only drops the
   session (`self.clip_edit`) and sets status. The E8 wording
   that lists it beside the clip pose ops is loose; the
   uncensused writes are `clip_set_angle` / `clip_set_root`
   themselves (rows A19–A20).
5. **A test comment in the landed B2 code claims `begin_draw`
   "snapshots at the gesture start, as designed." It does
   not** — only the eraser branch does. The plain-brush branch
   is a genuine remaining gap (row A18), and the B2 test
   passes regardless because it re-baselines after drawing.
   The integrator should treat the comment as wrong until
   row A18 lands.

---

## 5. The census

### Squad A rows — `src/editor.rs` (owner: B2)

All rows: **Snapshots today: No** (entering tree).

| # | Op (anchor) | Writes | Verdict | Reason |
|---|---|---|---|---|
| A1 | `adjust_length` (~731) | Selected node's `length`, clamped [4, 220] | **mark_dirty** | Stepper primitive; deliberately non-undoable. `tool_figure_scale` already snapshots once and calls it repeatedly — a snapshot inside would break the one-step rule. Mark only when the clamped value differs. |
| A2 | `adjust_thickness` (~751) | Selected node's `thickness`, clamped [1, 18] | **mark_dirty** | Same stepper family as A1. |
| A3 | `cycle_shape` (~779) | Selected node's `Shape` (Line → Circle → Polygon → Line) | **mark_dirty** | Appearance cycler; the `edit_count` doc named this family as deliberately uncounted — the dirty half was the bug, not the undo half. A cycle always changes the value: mark unconditionally on the success path. |
| A4 | `cycle_node_color` (~797) | Selected node's `NodeDef.color` through `NODE_COLORS` | **mark_dirty** | As A3; named verbatim in the entering autosave test as a non-counting cycler. |
| A5 | `set_node_color` (~820) | Selected node's `NodeDef.color = Some(rgb)` via `write_node_color` | **mark_dirty** | Doc comment states the no-snapshot design (swatch/stepper presses must not flood undo). Patch at this public boundary only — the shared private writer is also used under `set_node_style_atomic`'s single snapshot and must not mark there. Mark only when the colour differs. |
| A6 | `camera_clear_key` (~863) | Removes the camera key at the current frame (`Project::remove_camera_key`) | **mark_dirty** | The camera panel sells this exact path as its labelled "Clear key (no undo)" button; the undoable sibling (`delete_camera_key_at`, MCP `delete_camera_key`) already snapshots and stays the undoable route. Making this one snapshot would falsify the button's label; leaving it uncounted was the bug. Mark only when a key was actually removed (the op already branches on that). **OPEN at census close.** |
| A7 | `toggle_figure_lock` (~1501) | Figure `locked` via `layers::toggle_lock` | **mark_dirty** | Layer-flag family: panel toggles, deliberately not undo steps. A toggle always changes the flag: mark on the success path only (out-of-range refusal marks nothing). |
| A8 | `cycle_figure_opacity` (~1514) | Figure `opacity` cycle 1.0 → 0.6 → 0.3 → 1.0 via `layers::cycle_opacity` | **mark_dirty** | Named in the entering autosave test as deliberately uncounted; same family as A7. |
| A9 | `step_figure_opacity` (~1533) | Figure `opacity` stepped via `layers::set_opacity` | **mark_dirty** | Its own doc states the no-snapshot design. Mark only when the stepped value differs from the stored one (a clamped no-move step marks nothing). |
| A10 | `toggle_figure_hidden` (~2119) | Selected figure's `hidden` flag, direct flip | **mark_dirty** | Layer-flag family, as A7. |
| A11 | `adjust_frame_duration` (~2209) | Current frame's `duration` × factor, clamped [0.25, 8] | **mark_dirty** | The playback panel's duration steppers are the settings-tweak family; the MCP duration setters (rows C3–C4) were ruled the same way by B9 for consistency. Mark when the clamped result differs. |
| A12 | `adjust_fps` (~2224) | `Project.fps`, clamped [1, 60] | **mark_dirty** | Stepper half of the fps pair — contrast `set_project_fps`, which refuses no-ops and snapshots (covered, §6). Mark on actual change only. |
| A13 | `cycle_easing` (~2334) | `Project.easing = easing.next()` | **mark_dirty** | Settings cycler; always changes: mark on the write. |
| A14 | `set_project_easing` (~2348) | `Project.easing` | **mark_dirty** | Doc comment rules project-settings tweaks "not undoable edits in this core"; `easing_panel.rs` repeats the ruling. Mark only when the value differs. |
| A15 | `toggle_tween_playback` (~2357) | `Project.tween_playback` flip | **mark_dirty** | Settings toggle family. **Interaction — see row D1:** the `mcp.rs` built-in pre-snapshots around this op, so the landed mark now double-counts that path until D1 is applied. |
| A16 | `toggle_camera_follow` (~2368) | `Project.camera_follow` (`None` ↔ selected figure) | **mark_dirty** | Settings toggle family. The MCP built-in `set_camera_follow` writes the field directly under its own snapshot and never calls this op, so no double-count there. |
| A17 | `clear_frame_drawings` (~3278) | `Project.strokes.retain(frame != current)` — deletes every stroke on the current frame | **snapshot** | **This row rules against the mark_dirty B2 landed during the census.** Evidence: every peer stroke-destruction op snapshots (`delete_stroke_at`, `fill_current_stroke`, `lasso_erase`, `mirror_strokes_x`), and the MCP handler `draw_ops::tool_clear_frame_strokes` already takes a snapshot itself before calling this op, "only when the frame actually carries strokes" — the surface contract is one undo step. With B2's mark also inside the op, the MCP path now counts **two** edits for one action. Patch in §7. **OPEN at census close.** |
| A18 | `begin_draw` plain-brush branch (~3136) | Pushes a new `Stroke` onto `Project.strokes`; `draw_to` appends its points/widths; `end_draw` smooths and applies the nib profile | **snapshot** | **Headline find.** A hand-drawn stroke is discrete project content, and every other stroke-creation path snapshots: the eraser branch of this same function, stamps (`push_stamp_stroke`), MCP `draw_stroke` / `draw_shape` / `draw_polygon` / `draw_star`. Entering Wave 13, a drawn stroke was neither undoable nor counted. Patch: snapshot **once**, in the plain branch, after the brush/colour are read and before the `strokes.push` — never in `draw_to` (per-move snapshots would flood the 60-deep stack). **OPEN at census close.** |
| A19 | `clip_set_angle` (~4311) | `Project.clips[ci].frames[fi].angles[node]` in the movieclip session (after `clip_fit_frame` normalisation) | **mark_dirty** | Session pose tweak: per-angle writes during clip editing would flood undo (the tool descriptions pin "NO undo snapshot (pose-tweak rule)" as deliberate). Mark after the session/range refusals, only when the stored angle differs. Do not mark inside `clip_fit_frame` — snapshotting structural ops (`clip_add_frame`) also call it. **OPEN at census close** (the MCP handlers delegate here, so the core is the single patch site). |
| A20 | `clip_set_root` (~4326) | `Project.clips[ci].frames[fi].root` in the movieclip session | **mark_dirty** | As A19. **OPEN at census close.** |

**State at census close (Squad A):** B2 landed `mark_dirty()`
(the seam itself, editor.rs ~2021) and the marks for A1–A5,
A7–A16, each guarded to real changes, with unit pins
(`layer_ops_mark_dirty_without_an_undo_step`,
`settings_and_tweak_ops_mark_dirty_on_real_changes_only`).
A17 landed as mark_dirty and is **overruled** by this census
(see §7). A6, A18, A19, A20 remain open.

### Squad B rows — `src/figure_ops.rs` (owner: B3)

All rows: **Snapshots today: No** (entering tree).

| # | Op (anchor) | Writes | Verdict | Reason |
|---|---|---|---|---|
| B1 | `EditorCore::set_figure_color` (~255) | `FigureInstance.color` | **mark_dirty** | Doc comment: a colour change "is not an undo step" by design, like `set_node_color`. Reached from the MCP `figure_set_color` handler and the GUI colour swatches, so this one patch site covers both surfaces. Mark only when the colour differs. **Landed at census close.** |
| B2 | `tool_figure_rename` (~618) | `Project.figures[fi].name` (trimmed; blank refused) | **mark_dirty** | The known E8 row. Names are "plain field writes everywhere" by documented design (the GUI half is row E1); the fix is the dirty half, not an undo step. Mark only when the trimmed name differs from the stored one. **Landed at census close.** |
| B3 | `tool_layers_set_all_visible` (~1237) | Every figure's `hidden` via `layers::set_all_visible`; also clears `core.solo` (session state — not censused) | **mark_dirty** | Bulk layer write, panel parity (the panel's All/None buttons do not snapshot). Mark **once per call**, only when at least one flag changed (the helper reports the change count). **Landed at census close.** |
| B4 | `tool_layers_invert_visibility` (~1271) | Every figure's `hidden` flipped via `layers::invert_visibility` | **mark_dirty** | As B3: once per call, only when at least one flag flipped. **Landed at census close.** |

Covered sibling, named so nobody re-fixes it: `recolor_figure`
(~359) snapshots exactly once — see premise correction 3.
`tool_layers_cycle_opacity` delegates to core A8 and inherits
its mark. The pure helpers in `src/layers.rs`
(`set_all_visible`, `invert_visibility`, `toggle_lock`,
`set_opacity`, `cycle_opacity`, `move_figure`) take
`&mut Project` and cannot snapshot or mark; every row for
their writes sits at the `EditorCore`/handler boundary above,
and `EditorCore::move_figure` already snapshots.

### Squad C rows — `src/mcp_tools.rs` (owner: B9)

All rows: **Snapshots today: No** (entering tree).

| # | Op (anchor) | Writes | Verdict | Reason |
|---|---|---|---|---|
| C1 | `tool_layers` (~1862) | One figure's `locked`, `hidden` (direct field writes) and `opacity` (via `layers::set_opacity`); `solo` is session state | **mark_dirty** | The MCP face of the A7–A10 layer family; same deliberate no-undo discipline. Mark **once per call** when any of the three fields actually changed; identical values mark nothing. **Landed at census close.** |
| C2 | `tool_set_weather` (~1193) | `Project.weather` = `None` or `Some(Weather{…})` | **mark_dirty** | Project-settings write, same family as the GUI weather rows E3–E4. Mark only when the stored weather differs (including the off-transition). **Landed at census close.** |
| C3 | `tool_set_frame_duration` (~478) | `Project.frames[frame].duration` (direct write) | **mark_dirty** | Ruled with the core stepper A11 (the "timing tweak" family) for cross-surface consistency: one counted edit, no undo step. Mark only when the value differs. **Landed at census close.** |
| C4 | `tool_set_frame_durations` (~496) | `Project.frames[i].duration` for a validated prefix of the timeline | **mark_dirty** | As C3, but **once per call** when any applied value changed — never once per frame. **Landed at census close.** |
| C5 | `tool_project_rename` (~3105) | `Project.name` (session name; the file move itself is `projects::rename_project`) | **mark_dirty** | The known E8 row, at its corrected anchor (premise correction 1). No snapshot: undo would restore the old session name against a file that keeps the new one. Mark only when the trimmed name differs. The neighbouring `refresh_project_files` call must never touch the counters (it also runs after saves). **Landed at census close.** |
| C6 | `tool_apply_nib` (~2508) | Rewrites `widths` of **every** stroke on a frame via `draw_ops::apply_nib` | **snapshot** | A discrete batch edit of serialised content, and every sibling batch stroke tool snapshots (`tool_smooth_strokes`, `tool_erase_strokes`, `tool_simplify_strokes` all snapshot first). Entering, this one silently rewrote stroke geometry invisibly to both undo and dirty. Patch in §7. **OPEN at census close.** |

### Squad D row — `src/mcp.rs` (owner: integrator — no B-squad owner)

| # | Op (anchor) | Writes | Verdict | Reason |
|---|---|---|---|---|
| D1 | `tool_set_tween_playback` handler (~1505–1515) | `Project.tween_playback`, composed as `core.snapshot()` + `core.toggle_tween_playback()` | **snapshot (keep) — fix the composition** | Wave 11 deliberately made the MCP call one undo step by pre-snapshotting around the non-undoable core toggle. Now that A15 marks inside the toggle, this path counts **two** edits for one undo step. Fix per §7: mirror the neighbouring `set_camera_follow` built-in, which writes the field directly under its own snapshot and never calls the toggle. **OPEN at census close.** |

### Squad E rows — Rune panels, `src/rune_sec/` (owner: integrator — no B-squad owner)

All rows: **Snapshots today: No** (entering tree), and none had
landed at census close. These panels document their own
"no snapshot, on purpose" discipline in their module docs;
the census does not disturb the undo half of that discipline —
it rules that the writes must count.

| # | Op (anchor) | Writes | Verdict | Reason |
|---|---|---|---|---|
| E1 | `inspector_figure::rename_selected_figure` (~541) | `Project.figures[fi].name` (direct write; no core op exists) | **mark_dirty** | The GUI half of B2 — same write, same verdict, or the dirty signal depends on which surface renamed the figure. Mark only when the cleaned name differs from the stored one; blank/stale refusals mark nothing. |
| E2 | `camera_panel::follow_selected` (~701), `follow_clear` (~717), `follow_cycle` (~730) | `Project.camera_follow` (direct writes, bypassing core A16) | **mark_dirty** | Same field and family as A16, reached without the core op. Mark only on actual change (`follow_clear` on an already-`None` follow marks nothing; a stale index being repaired *is* a change). The panel's key-delete helper `delete_camera_key_at` already snapshots — covered, §6. `write_angle_here` is covered by the dial gesture's own single snapshot. |
| E3 | `scene_panel::set_weather_kind` (~255), `cycle_weather` (~287), `step_weather_amount` (~307), `step_weather_wind` (~322) | `Project.weather` (kind swap / wholesale preset cycle / amount & wind steppers) | **mark_dirty** | The module doc's own words: "No undo snapshots, on purpose: weather writes were never snapshotted." Settings family, as C2. Mark on actual change only (a stepper clamped to the same value, or any press with weather off, marks nothing). |
| E4 | `scene_panel::set_weather_amount` (~371), `set_weather_wind` (~387), via `amount_drag` (~406) / `wind_drag` (~438) | `Project.weather.amount` / `.wind` during slider drags | **mark_dirty — once per gesture** | The setters are called per Move event; marking per call would inflate the counter dozens of times for one user action. Follow the rotation-dial discipline already in the tree: the drag handlers already stash the press value in `Drag::data` — at `Up`, when not cancelled and the final value differs from the press value, mark **once**. A cancelled drag restores the press value and marks nothing. |
| E5 | `scene_panel::adjust_stage` (~471), `set_stage_preset` (~506) | `Project.stage` (per-axis steppers; wholesale preset write) | **mark_dirty** | Panel bypasses the snapshotting core setter `set_stage_size` (covered, §6) — the asymmetry is recorded here honestly: the panel's stepper/preset discipline is the settings-tweak family, and presets/steppers firing an undo step per press would diverge from every sibling panel. Mark on actual change only (a fully-clamped step marks nothing). |
| E6 | `settings_popover::apply_stage_step` (~452) | `Project.stage` via `stepped_stage_size` | **mark_dirty** | The second panel writing the same field directly; same ruling as E5 so the count does not depend on which popover stepped the stage. Mark on actual change only. |
| E7 | `scene_panel::set_background` (~696), reached also by `step_background_channel` (~743) | `Project.background` (swatches, presets, RGB steppers) | **mark_dirty** | Asymmetry recorded: the MCP sibling `tool_set_project_background` snapshots (a deliberate one-call edit). The panel's swatch/stepper discipline is the appearance-tweak family (cf. A5/B1), so the panel marks instead. Patch inside `set_background` — compare before writing — so swatches, presets and the channel steppers are all covered by one site. Mark on actual change only. |

---

## 6. Audited and covered — not gaps

So the gap list is not mistaken for the whole mutation
surface, the families checked and found compliant:

- **FX clips (`fx_ops.rs`)** — B4's module audit (~lines
  54–99) verified per op: `fx_move_clip`, `fx_resize_clip`,
  `fx_split_clip`, `fx_duplicate_clip`, `fx_delete_clip`,
  `fx_set_color`, `fx_set_color2`, `fx_set_anchor`,
  `fx_set_kind`, `fx_set_hidden` each snapshot exactly once
  after refusal checks. Its pure helpers in `fx_edit.rs` are
  covered by those callers.
- **Audio (`audio_ops.rs` + the editor's audio family)** —
  every `*_at` op snapshots on success (mute, fade, reverse,
  gain set/step, normalise — one snapshot for the bulk op —
  delete, duplicate, move, echo, lowpass, trim-silence,
  pitch, compress, split); the `*_last` wrappers delegate.
  MCP audio handlers delegate or snapshot (`tool_add_audio_clip`
  snapshots before its push).
- **Particles (`particle_ops.rs`)** — the core ops snapshot;
  `tool_particle_add_anchored` delegates to `add_particle`.
- **Graph (`graph_keys.rs`, `editor::clear_graph`,
  `graph_panel`)** — the ops snapshot once per success;
  the panel's key/tangent drags are covered by `drag_down`'s
  single gesture snapshot; `tool_graph_clear_channels`
  delegates to the snapshotting `clear_graph` (B7's in-tree
  truth note corrects an older contrary claim).
- **Text (`text_edit.rs`)** — the instance and segment ops
  snapshot once per applied change; both `TextEdit::commit`
  callers (library and `text_tool.rs`) snapshot before
  committing.
- **Assets (`media.rs`, Wave 12)** — import/rename/remove/
  attach handlers each snapshot once on success;
  `remap_asset_refs` runs under the remove handler's step.
- **Drawing helpers (`draw_ops.rs`)** — the pure
  `&mut Project` helpers (`erase`, `smooth_frame`,
  `simplify_strokes`, …) are covered at their call sites:
  the MCP handlers snapshot first, the eraser gesture
  snapshots in `begin_draw`, the panel simplify/stamp
  helpers snapshot. The one uncovered drawing path is the
  plain-brush gesture itself (row A18).
- **Gestures generally** — pose/camera `drag_to` writes are
  covered by the single snapshot in `begin_drag` /
  `begin_camera_drag`; the camera rotation dial composes
  its own one-snapshot-per-gesture discipline.
- **Settings setters with snapshot discipline** —
  `set_project_fps`, `set_stage_size`, `set_head_ring_scale`
  refuse no-ops and snapshot once on a real change.
- **Style** — `set_node_style_atomic`, `set_node_cap`,
  `set_node_color2`, `set_node_opacity` snapshot once; the
  private `write_node_*` helpers are not separate gaps.
- **Built-in MCP settings (`mcp.rs`)** — `set_camera_follow`
  and `delete_camera_key` snapshot on actual change at the
  handler (the pattern D1's fix copies).
- **GUI project rename** — `project_tools::apply_project_name`
  snapshots when the name actually changes (Wave 10). The
  asymmetry with figure rename is exactly why B2/E1 exist.
- **History and swaps** — `undo`/`redo`, load/open/import/
  new/recover and `note_project_swapped` are baseline
  operations, not mutators; project-swap undo semantics are
  a standing user decision, excluded from this wave.
- **File primitives (`projects.rs`)** — `rename_project`,
  `duplicate_project`, `save_copy` move/copy files and hold
  no `EditorCore`; `refresh_project_files` touches only the
  cached selector list and must never touch the counters.
- **Session-only state** — selection, playhead, loop range,
  solo, drag state, panel state, clipboard: not serialised
  `Project` state, wherever it is written.

## 7. Patch specifications for the OPEN rows

Each spec: anchor (function + line region at census close),
exact placement, and the no-op rule. One user action must end
up counting **exactly one** edit — never zero, never two.

- **A6 — `camera_clear_key`** (`src/editor.rs` ~863).
  The body already branches on `remove_camera_key`'s bool:
  add `self.mark_dirty();` inside the `true` branch, before
  the status write. The no-key branch marks nothing.
  Owner: B2 file; integrator to land.
- **A17 — `clear_frame_drawings`** (`src/editor.rs` ~3278) —
  **overrules the landed mark.** (1) In the core op: count
  the current frame's strokes first; if zero, keep today's
  status-only behaviour and spend nothing; if non-zero,
  `self.snapshot()` once, then retain. Remove B2's
  `mark_dirty()` call. (2) In
  `draw_ops::tool_clear_frame_strokes` (~996–1020): remove
  the handler's own `core.snapshot()` — the op now owns the
  step — keeping its frame checks and reply shape.
  Net effect on every surface: one undo step, one counted
  edit, zero for an empty frame. B2's unit test
  `settings_and_tweak_ops_mark_dirty_on_real_changes_only`
  pins the old mark_dirty behaviour for this op and must be
  updated to the snapshot deltas (undo depth +1,
  edits +1).
- **A18 — `begin_draw`** (`src/editor.rs` ~3136). In the
  plain-brush branch only, after `active_brush()` / colour
  are read and immediately before
  `self.project.strokes.push(...)`: insert
  `self.snapshot();`. The eraser branch already snapshots —
  do not add a second one there. No change to `draw_to` /
  `end_draw`. Also correct the B2 test comment that claims
  this already happens (premise correction 5).
- **A19/A20 — `clip_set_angle` / `clip_set_root`**
  (`src/editor.rs` ~4311 / ~4326). After the existing
  refusals (no session, bad joint) and after
  `clip_fit_frame` (A19), compare the stored value with the
  incoming one; write and `self.mark_dirty()` only when
  different. No snapshot in either op, and none in
  `clip_fit_frame`. Owner: B2 file; integrator to land.
  (B10's e2e pins already expect exactly these deltas:
  edits +1, undo depth unchanged.)
- **C6 — `tool_apply_nib`** (`src/mcp_tools.rs` ~2508).
  Count the frame's strokes first; if zero, return the
  current `count: 0` reply spending nothing; otherwise
  `core.snapshot()` once, then run the existing
  `apply_nib` loop. Owner: B9 file; integrator to land.
- **D1 — `tool_set_tween_playback`** (`src/mcp.rs`
  ~1505–1515). Replace `core.snapshot();
  core.toggle_tween_playback();` with the
  `set_camera_follow` pattern in the same file: when the
  requested value differs, `core.snapshot()` then write
  `core.project.tween_playback = enabled;` directly, and
  set the status the toggle would have set. When it does
  not differ, spend nothing (as today). Owner: integrator.
- **E1 — `rename_selected_figure`**
  (`src/rune_sec/inspector_figure.rs` ~541). After the
  blank/stale refusals: if `fig.name != name`, write and
  `core.mark_dirty()`; if identical, keep the status reply
  and mark nothing. (`EditorCore::mark_dirty` is `pub`;
  the bin already calls core methods throughout.)
- **E2 — camera follow** (`src/rune_sec/camera_panel.rs`
  ~701/717/730). In each fn, compare
  `core.project.camera_follow` with the value about to be
  written; write + `core.mark_dirty()` only on a real
  change. `follow_selected`'s no-valid-selection refusal
  marks nothing, as today.
- **E3 — scene weather discrete ops**
  (`src/rune_sec/scene_panel.rs` ~255/287/307/322). In
  `set_weather_kind` and `cycle_weather`, compare the new
  `Option<Weather>` with the stored one and mark on
  difference. In the steppers, mark only when the clamped
  result differs from the stored amount/wind; the
  weather-off branch marks nothing.
- **E4 — weather sliders** (`src/rune_sec/scene_panel.rs`
  ~371/387 setters; ~406/438 drags). Do **not** mark in the
  setters. In `amount_drag` / `wind_drag`, `Phase::Up`:
  if not cancelled and the current stored value differs
  from the press value already stashed in `d.data`,
  `core.mark_dirty()` once. Cancelled drags (which restore
  the press value) mark nothing.
- **E5 — scene stage** (`src/rune_sec/scene_panel.rs`
  ~471/506). In `adjust_stage`, capture the stage before
  clamping; mark when either axis changed. In
  `set_stage_preset`, mark when `[w, h]` differs from the
  stored stage.
- **E6 — popover stage step**
  (`src/rune_sec/settings_popover.rs` ~452). In
  `apply_stage_step`, compare `stepped_stage_size(...)`
  with the stored stage; write + mark only on difference.
- **E7 — scene background** (`src/rune_sec/scene_panel.rs`
  ~696). In `set_background`, compare `rgb & 0xFF_FFFF`
  with the stored background; write + mark only on
  difference. This one site covers swatches, presets, and
  `step_background_channel`.

## 8. What this census does not claim

- It does not re-audit whether the *undo* half of any
  covered op is correct — only whether the write counts.
- The Wave 13 sprite surface (A squad) and any code landed
  after census close are outside the audit; §3 states the
  spot-check the integrator owes them.
- Two verdicts are judgement calls the integrator may
  revisit with the user, both flagged in their rows: A6
  (mark_dirty rests on the panel's own "no undo" label)
  and E5/E7 (panel-vs-MCP asymmetry on stage/background,
  recorded rather than silently harmonised). A17 is not a
  judgement call: the double-count is arithmetic.
