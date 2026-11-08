# Phase J — the two overlaps, the process fix, and depth

Phase J opened by fixing what Phase I's own report named:
the two layout overlaps visible in the headless snap and
the two process failures behind them (a docs section that
reported files it never wrote, and a State field whose
wiring was nearly missed). Around those fixes it closed
audio's last named gap, finished the export range family,
and resolved two Phase I data quirks — duplicate graph
channels and the library's Enter-to-insert — before
spending its remaining sections on panel depth.

> **A note on provenance.** This document was written by
> the phase's docs section, but under the rule Phase I
> taught: every row below was checked against the tree
> before it was written, and rows whose work is not in
> the tree at writing time are marked
> **planned — not landed** rather than described as
> done. The integrator re-verifies at integration time,
> as before.

## 1. What Phase J set out to close

From Phase I's report and remainder list:

- **Overlap 1** — the Keyframes header's "No joint
  selected" note painted across its Delete button
  (visible in the headless snap, run 42467268661).
- **Overlap 2** — at 2400×1350 the right column's bottom
  rows crowded the status strip's tail.
- **The process gap** — Phase I's docs section reported
  deliverables that were never on disk, and §23's
  `pose_library` State field was caught only by a late
  detailed report. Section 39's pre-commit gate exists
  so both failure shapes are checked mechanically.
- **Audio drift** — wrap and seek re-syncs fire on
  jumps; slow drift *within* one pass had no correction
  (named in `src/audio_play.rs` since Phase F).
- **The range family's last holes** — spritesheet and
  SVG were whole-timeline only (Phase I §8 closed the
  contact sheet; these two remained).
- **Duplicate graph channels** — listed twice, resolved
  to the first (Phase I §7); a picked row had no
  identity of its own.
- **Library Enter-to-insert** — Phase I recorded a
  wrong-figure quirk; §25 found the truth in the code
  (see §3).
- **Delete key** — with a stroke or particle selected,
  Delete still removed a segment; the selection the
  user was looking at was not the thing deleted.

## 2. The overlap fixes (charter item a)

**§9 — keyframe_tools.** Root cause, proven from the
Phase I snap's geometry: the no-joint note and the
actions row shared a layout row, and Rune text does not
truncate, so the note's painted line ran across the
Delete button. The fix is structural, not cosmetic: the
note is now its own full-width row (key
`keyframe-no-joint-note`, constant
`NO_JOINT_NOTE_ROW_KEY`) holding one truncating muted
text — the status strip's tail-segment discipline —
above the actions row (`keyframe-actions`). The wording
is byte-identical ("No joint selected"); Delete keeps
its key (`key-delete`). Nothing else in the panel moved.

**§10 — main.rs layout.** Root cause, found by the
section in the shell's own layout code: all three
columns carried `.grow(1.0)`, flattening them to equal
thirds (~461 pt each at 1440 wide). The centre — stage,
timeline, status strip — was starved to a third of the
window, and the strip's floored segments (the Phase F
width floors doing their job) summed wider than the
strip, so its tail painted rightwards over the right
column's bottom rows. The heights were never the
problem. The fix: side viewports keep their fixed
widths (300 / 340), the centre takes the rest. The same
section wired §1's drift sync into `poll` and gave the
Delete key its priority order (§1 item c below).

Pixel proof for both fixes is the integrator's headless
re-dispatch — see §7.

## 3. The 40 sections

| § | File | What landed |
|---|---|---|
| 1 | src/audio_play.rs | Drift correction: every start records a playback clock (wall instant + start frame); `drift_frames` + `DRIFT_TOLERANCE_FRAMES` (2.5 frames); `sync_playback_audio_on_drift` restarts the mix when the playhead wanders past tolerance. Poll wiring landed with §10 (below) |
| 2 | src/export.rs | `export_sheet_range` (:1318) + `export_svg_range` (:1533) — the range family is now complete: every format with a range form has one |
| 3 | src/figure_ops.rs | `mirror_figure_all_frames(fi)` (:382) — whole-figure mirror over every frame, one snapshot |
| 4 | src/graph_keys.rs | `duplicate_channel_to_figure` — clones a channel (target + keys verbatim) onto another figure; refuses a same-target destination (the clone would be shadowed by first-match resolution), a missing rig node, the channel's own figure. **No caller yet — see §5** |
| 5 | src/particle_ops.rs | `particle_duplicate_event(index) -> Option<usize>` (:150), sibling discipline (one snapshot, status refusals) |
| 6 | src/fx_ops.rs | `fx_split_clip(index, at_frame)` (:190) — one clip becomes two at the cut frame |
| 7 | src/audio_ops.rs | `normalised_gain` (pure) + `normalise_all_clips(target_peak) -> usize` (:400) — project-wide normalise over the Phase H indexed family |
| 8 | src/mcp_tools.rs | MCP parity for the phase's new ops: `fx_split_clip`, `particle_duplicate_event`, `audio_normalise_all`, `mirror_figure` — registry 77 → 81 (the file's count test pins 81), total 88 → 92 |
| 9 | src/rune_sec/keyframe_tools.rs | Overlap 1 fix (§2 above): note in its own truncating row (`keyframe-no-joint-note`); Delete untouched |
| 10 | src/main.rs | Overlap 2 fix (§2 above) + Delete priority (selected stroke → selected particle → segment, selection cleared either way) + poll's drift-sync call, after the wrap and seek syncs |
| 11 | src/rune_sec/timeline_marks.rs | `timeline-ruler` — a time ruler above the chips; labels at whole seconds only, counted at the project rate (rounded, floored at 1 fps) |
| 12 | src/rune_sec/menus.rs | File ▸ Open recent: up to five entries keyed `file-recent-{i}`, resolved by exact stored name against the project files; a vanished file and an empty recents list both behave (no block at all when there are no recents) |
| 13 | src/rune_sec/scenes.rs | Two new headless scenes in the scene table: `export-open` (Export workspace) and `fx-selected` (Effects workspace with a selected FX element) — snap/tree coverage for workspaces Phase I's four scenes never showed |
| 14 | src/rune_sec/command_palette.rs | +6 commands over the phase's new capabilities — `split-fx-clip`, `mirror-figure`, `normalise-audio`, `duplicate-particle`, `delete-stroke`, `delete-particle` — 97 → 103 (the file's count test pins 103) |
| 15 | src/rune_sec/export_panel.rs | Spritesheet + SVG join the range family in the panel: `RANGE_KEYS` 7 → 9 (`xc-svg`, `xc-sheet`); range variants write to the full ops' own destinations (`./export/svg`, `./export/spritesheet.png`) |
| 16 | src/rune_sec/graph_panel.rs | Duplicate channels resolve by **row index**: `GraphPanelState.selected_channel: Option<usize>` (panel state — not a new shell State field); a picked row draws and edits its own twin, target-only behaviour resumes when the pick clears. The Phase I "resolve to first" remainder is closed for selection |
| 17 | src/rune_sec/fx_clips.rs | Split button (`fx-split`, scissors glyph, in `fx-split-row`): cuts the selected clip at the playhead via §6 |
| 18 | src/rune_sec/audio_panel.rs | Normalise-all row (`audio-normalise`): every clip in the project via §7, not just the selected one |
| 19 | src/rune_sec/particles_panel.rs | Duplicate button (`particles-duplicate`) for the selected event, via §5 |
| 20 | src/rune_sec/pose_tools.rs | Two whole-timeline tools: Mirror figure (all frames) (`pose-mirror-all`, via §3) + `apply_pose_to_all_frames` — the clipboard pose pasted onto every frame |
| 21 | src/rune_sec/inspector_segment.rs | Bend in degrees: the steppers now read −5°/−1°/+1°/+5°, a `seg-bend-readout` line shows the selected segment's bend in whole degrees, and `straighten_selected` zeroes a bend in one undo step |
| 22 | src/rune_sec/inspector_style.rs | Recent colours: a panel-local `thread_local` list (most recent first, no duplicates, cap 8 — `RECENT_COLORS_CAP`), recorded from the panel's own apply sites only |
| 23 | src/rune_sec/stroke_inspector.rs | Frame-scoped bulk row under the project-wide one: recolour/delete this frame's strokes only (one snapshot; statuses name the frame, 1-based; a strokeless frame takes no snapshot) |
| 24 | src/rune_sec/text_tool.rs | Write-back editing: a loaded text's content can be edited and written back with the Update button (`text-update`) — rows were load-only before |
| 25 | src/rune_sec/library_panel.rs | Enter-to-insert, honestly reframed: the Phase I "filtered index" quirk never existed in code — the search field wired no `on_submit` at all, so Enter inserted nothing. The submit path is new here and resolves through the rows' real-index mapping (`enter_insert_index` / `insert_at`) from the start, so the wrong-figure insert can never happen |
| 26 | src/rune_sec/home_panel.rs | Row metadata: each project row shows figures/frames/modified from a *light* read of the file's top-level arrays (never a full parse), cached in `HomePanelState::metadata_for` |
| 27 | src/rune_sec/project_tools.rs | `proj-current-meta` — "{size} · modified {time}" under the Phase I current-file line, gated on that line's own exact-match decision so the two can never disagree about which file the session is on |
| 28 | src/rune_sec/status_strip.rs | Autosave segment: `autosave_segment(modified, now)` (pure) renders "Autosaved just now / {n}m ago / {n}h ago"; the segment vanishes when no snapshot exists. **Its call site needs one lib accessor the tree does not have yet — see §5** |
| 29 | src/rune_sec/help_panel.rs | 30 → 34 rows (the file's count test pins 34) — the phase's new features documented in the panel's own format |
| 30 | src/rune_sec/layers_panel.rs | Colour dot per layer row (`layer_dot_color`: the figure's root-node colour, engine resolution), so layers are distinguishable at a glance |
| 31 | src/rune_sec/waveform_view.rs | Clip-mode seek: in selected-clip mode the strip maps x onto the clip's own span (the clip-span counterpart of `frame_at_x`), so dragging in clip mode seeks within the clip's frames, not the project's |
| 32 | src/rune_sec/camera_panel.rs | Hold camera (`cam-hold`): one camera key written on **every** frame of a span (`hold_span` — the loop range) in exactly one snapshot |
| 33 | src/rune_sec/frame_ops.rs | Reverse all (`frames-reverse-all`) beside Reverse: always the whole timeline; the loop-range rule stays with the range pair. Guard `reverse_all_enabled` (below two frames it renders inert) |
| 34 | src/rune_sec/stage_layers.rs | Stroke vertex dots: one `StrokePrim::Dots` per vertex of the selected stroke's **base** points (never the spray scatter), radius `VERTEX_HALF` — the picking convention of Phase G §5, made visible |
| 35 | src/rune_sec/draw_tools.rs | Brush smoothing readout (`draw-smooth-readout`): the active preset's `paint::Brush::smoothing` shown as a readout (smoothing itself is a preset field; the panel now says what it is) |
| 36 | src/rune_sec/inspector_figure.rs | Centre / ground figure: `figure_bbox` (evaluated joints, head-flagged points grown by `HEAD_RING_R` 17.0 on all sides), `centre_delta` / `ground_delta`, both actions through `nudge_path_point` — exactly one snapshot, **current frame only** — as `fig-centre` / `fig-ground` in the `fig-place-row` under the scale row. Landed after this document's final census; the row was rewritten against the tree at integration |
| 37 | src/rune_sec/snap_tests.rs | Runtime round 7: 23 → 27 tests. Two **geometric** regressions for the snap's overlaps — `keyframes_note_and_delete_button_do_not_overlap` (asserts on the note *text's* drawn rect via `Runtime::elements`, not just its row) and `status_strip_is_not_covered_by_the_right_column` (the strip's right edge against the `side-right` column's left edge at the 1440×900 Runtime viewport) — plus `timeline_ruler_shows_the_zero_label` (§11) and `palette_mirror_figure_command_exists_and_runs` (§§3/14). Landed after this document's first census; the row was rewritten against the tree |
| 38 | src/rune_sec/fx_browser.rs | Filter matches category names (`category_matches`): a query naming a category ("energy", "nat") shows that category's whole kind list, unioned with the kind matches |
| 39 | scripts/phase_j_checks.py | The pre-commit gate **landed** (586 lines, stdlib only, read-only): five checks — inventory, key diff against a base ref, State-literal census (every full `State {…}` literal must name every field of the current struct), caller census for new lib fns (an op with no production call site is reported UNWIRED), and count claims read from source (palette / MCP registry / icons / scenes). Each check's header names the Phase I failure behind it. It landed after this document's first census; the row was rewritten against the tree — and its caller census is exactly what flags §5's first two items mechanically |
| 40 | docs/ | This document + QA_RUNE §12 |

## 4. Cross-section contracts

- **Poll sync order** (main.rs): wrap sync, then seek
  sync, then drift sync (§§1/10). The three are mutually
  exclusive by construction — a wrap is `cur < prev`, a
  seek is `cur > prev + 1`, drift is the fractional
  playhead wandering within a pass — so the order can
  never double-restart the mix.
- **Range exports write where the full ops write**
  (§§2/15): the range variants of sheet and SVG use the
  full ops' destinations, so the recap and history
  lines name the same paths either way — the Phase I
  contact-sheet convention, kept.
- **FX kind order** is unchanged: §6's split and §17's
  button inherit `FX_KIND_ORDER` (§7 of Phase I); a
  split clip's second half takes the first half's kind.
- **No new shell State fields.** Phase J's state lives
  where it belongs: §16's pick in `GraphPanelState`,
  §22's recents in a `thread_local`, §26's cache in
  `HomePanelState`. Verified against main.rs: its diff
  carries §10's fixes and §1's poll call, and no new
  `State` field — the Phase I §23 trap did not recur.
- **Counts are pinned in the files that own them**:
  palette 103 (§14's test), MCP registry 81 (§8's
  test), help 34 rows (§29's test). A count in these
  documents can be checked against its pin in seconds.

## 5. Integrator wiring (owed at integration, found by census)

- **`shell::recovery_modified()` did not exist.**
  §28's `status_strip_state` calls it
  (status_strip.rs:657) to feed `autosave_segment`, but
  shell.rs defined no such accessor — it had a private
  `file_mtime` (shell.rs:151) and the `RECOVERY_FILE`
  constant. **Done at integration**: the public wrapper
  (mtime of `./projects/.recovery.json`, `None` when
  absent) is in shell.rs beside `file_mtime`. This is
  precisely the caller-census catch §39's gate is
  meant to make mechanical.
- **§4's `duplicate_channel_to_figure` had no caller.**
  The lib op landed with its refusal discipline, but
  no panel button called it (§16 resolved duplicates
  by index instead). **Done at integration**: the
  graph panel's `graph-chan-duplicate` button
  ("Duplicate channel to next figure", shown while a
  channel is drawn) calls it via the panel's
  `duplicate_channel_to_next` helper — twin-aware
  through §16's effective index, one snapshot through
  the op, two tests in graph_panel.
- **§34's vertex dots needed their draw call.**
  `stroke_points_geometry` landed tested but, like
  its Phase F predecessors, had no consumer.
  **Done at integration**, exactly as §34's report
  specified: in stage_view's highlight pass, after
  the glow loop, the dots are recoloured to the
  selection accent and drawn through the same
  `highlight_prim` path.
- **§36 landed after this document's final census**
  (§§37/39 landed after it too, and their rows in §3
  were rewritten against the tree; §36's row was
  rewritten the same way at integration). Nothing
  from the 40 sections is outstanding.

## 6. Inventory snapshot

Post-wiring, at integration: `scripts/rune_inventory.py`
reports **641 unique keys (547 static + 94 dynamic)**,
up from Phase I's 612 — and the §39 gate's key diff
against the Phase I tree passes with **zero keys lost**
(the two apparent losses, `key-delete` and
`keyframe-actions`, are §9's named constants carrying
the same values — the gate now counts `…_KEY` constant
values on both sides). The gate's full run is green:
all 20 full `State {…}` literals name all 42 fields,
every Phase J lib op has production callers, and the
source counts read palette **103**, MCP registry
**81**, icons **110**, scenes **6**.

## 7. Honest remainders after Phase J

- The two overlaps are fixed (§§9/10) and now
  **pixel-proven**: `headless.yml` re-dispatched on
  the green tree (run 42468895995, all 6 probes exit
  0) — the tree dump measures the columns at exactly
  300 / 744 / 340, the status strip ending 12 pt
  before the right column starts, and the Keyframes
  note row fully above the actions row; the rendered
  snap shows the wide stage, the ruler's "0:00" label
  and the strip contained in the centre column. One
  trade-off the narrower left column introduced: the
  Figures header's "Recolour figure" button label
  clipped at 300 pt wide — shortened to "Recolour"
  at integration (key and behaviour unchanged).
- Two "New project" results still disagree (panel
  blank vs Home demo) — the user's product call,
  untouched again.
- The head ring radius is a constant 17.0 everywhere.
- `core.onion_frames` / `cycle_onion_frames` remain
  orphaned engine API. They stay deliberately: the
  shell keeps *asymmetric* onion depths in State
  (`onion_ui.prev` / `onion_ui.next`, Phase G §25),
  which the engine's single symmetric count cannot
  express — wiring the shell to it would lose a
  feature, so the orphan is the honest state.
- The status strip's Audio segment still has no glyph
  (the Phase H width-maths refusal stands).
- Pose variant buttons keep their local inert branch
  (Phase I §5).
- Sound on a real screen is still the user's to
  confirm: the headless snap covers pixels, never
  audio, and drift (§1) is a bounded restart, not a
  sample-accurate clock — within one pass, picture
  and sound can still part by up to the 2.5-frame
  tolerance before the mix re-syncs.
