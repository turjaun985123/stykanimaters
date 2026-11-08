# Phase I — scrub-follow, stage bends, and depth

Phase I closed the functional remainders Phases G and H left
named, then spent its remaining sections on panel depth:
readouts, filters and shortcuts that make existing engine
data visible. It also expanded the icon set a second time
from the same Font Awesome download.

> **A note on provenance.** This document was written by
> the integrator after the phase's docs section reported
> files that were not on disk; every claim below was
> re-verified against the tree at integration time. The
> episode is recorded here rather than silently repaired,
> in the spirit of these documents.

## 1. What Phase I set out to close

From the Phase G/H remainder lists:

- **Audio scrub-follow** — seeking while playing did not
  re-sync the sound (Phase F §16's limit; Phase G §2 closed
  only the loop-wrap case).
- **Stage bends** — the stage drew segments straight;
  bends were visible only in exports (Phase G §19).
- **MCP parity** — the six indexed audio DSP ops from
  Phase H §37 had no MCP tools.
- **The inert icon button** — `icon_btn` had no muted
  variant, so panels kept local wrappers (`pibtn`, `ictl`)
  or lost their disabled tint (Phase H §7, §18).
- **Icon gaps** — `fig-scale-dn` stayed text ("compress"
  does not exist in the set) and the mirror buttons used
  "copy" for want of a flip glyph.
- **Palette gaps** — no workspace-jump commands, several
  shell toggles unreachable from Ctrl+K.

## 2. Icon expansion batch 2

`src/rune_sec/icon_data.rs`: 102 → **110 names**. Eight
glyphs extracted byte-for-byte from the same Font Awesome
Free 7.3.1 files in `assets/icons/fontawesome/solid/`:
`down-left-and-up-right-to-center` (the real shrink
glyph), `arrow-right-arrow-left`, `right-left`,
`left-right`, `up-down`, `bars`, `minimize`,
`up-down-left-right`. All eight are single-path, 512-high
— no anomalies. Adoptions: fig-scale-dn (§22), pose
mirror and 3D mirror (§§23/24, `arrow-right-arrow-left`).

## 3. The 40 sections

| § | File | What landed |
|---|---|---|
| 1 | src/audio_play.rs | `playhead_seeked` + `sync_playback_audio_on_seek` — scrub-follow closed both ways (backward seeks were already restarted by the wrap path) |
| 2 | src/mcp_tools.rs | Six indexed DSP tools (`audio_echo` … `audio_split_silences`); registry 71 → 77, total 82 → 88 |
| 3 | src/figure_ops.rs | `recolor_figure` — one snapshot, every node's colour, color2 untouched |
| 4 | src/audio_ops.rs | `clip_peaks` — per-clip peaks via the panel's own loader path; gain NOT applied (the panel scales at draw) |
| 5 | src/graph_keys.rs | `channel_stats` + `ChannelStat` + `target_label` (matches the panel's casing string-for-string) |
| 6 | src/particle_ops.rs | `particle_set_kind` / `particle_set_color`, sibling discipline |
| 7 | src/fx_ops.rs | `fx_set_kind` + `FX_KIND_ORDER` + `fx_kind_cycle` (the browser's category order, mirrored as a const — the lib cannot import the shell) |
| 8 | src/export.rs | `export_contact_sheet` / `export_contact_sheet_range` — the range family is complete; the sheet tiles each authored frame once (no duration expansion), 4 cols × 240 px (the MCP tool's defaults) |
| 9 | src/stage_view.rs | Bend arcs on the stage: `segment_pieces` mirrors export's `bend_mid` construction; zero-bend output is prim-identical to before |
| 10 | src/rune_sec/icon_data.rs | Batch 2 above |
| 11 | src/rune_sec/icon_btn.rs | `icon_btn_inert` / `icon_only_btn_inert` / `glyph_color_muted` (= `text_secondary` — Rune's Theme has no `text_muted`; fill = `t.card`) |
| 12 | src/rune_sec/inspector_segment.rs | Topology readout (length/angle already existed — the branch-2 item landed instead) |
| 13 | src/rune_sec/playback_controls.rs | `pb-prev-key` / `pb-next-key` over the camera∪graph key union (graph rounding mirrors the graph seek) |
| 14 | src/rune_sec/timeline_marks.rs | Pressable markers (`marker-cam-{f}` / `marker-graph-{f}`) — glyphs wrapped, geometry untouched |
| 15 | src/rune_sec/fx_clips.rs | Strip click on empty space seeks; kind cycling via §7 (old fractional helper renamed `frame_frac_at_x`) |
| 16 | src/rune_sec/graph_panel.rs | Stats block under the channel list (`graph-stats`, drawn channel highlighted) |
| 17 | src/rune_sec/waveform_view.rs | Selected-clip waveform mode + `waveform-caption`; playhead/seek stay project-mapped |
| 18 | src/rune_sec/audio_panel.rs | `audio-readout` — the effective target's name, start and gain |
| 19 | src/rune_sec/particles_panel.rs | Kind cycling (all 17 ParticleKinds) + 8-colour swatch row for the selected event |
| 20 | src/rune_sec/figures_panel.rs | `fig-recolor` — paints the whole figure with the selected node's explicit colour (the ONLY recolour source: an uncoloured node's on-screen colour is a draw-time value with no stored source — documented in the file) |
| 21 | src/rune_sec/inspector_style.rs | Six style presets through `set_node_style_atomic` (CapKind has no Butt — Amber ships Round, documented) |
| 22 | src/rune_sec/inspector_figure.rs | `fig-measure` readout + fig-scale-dn's icon |
| 23 | src/rune_sec/pose_tools.rs | pibtn delegates to `icon_btn_inert` where it can (see §5), mirror glyph, pose-library filter with the figures count discipline |
| 24 | src/rune_sec/three_d_panel.rs | ictl deleted — live `icon_btn` / dim `icon_btn_inert`; mirror glyph |
| 25 | src/rune_sec/frame_ops.rs | Inert tint restored for all six icon buttons (copy-frame, paste-frame, range copy/paste/delete, insert-N) — each guard a pure fn with a truth-table test |
| 26 | src/rune_sec/keyframe_tools.rs | Inert tint restored for the seven stamp buttons via an `nudge_plan`-equivalent dry run (predicates mirror the refusal ladder rung for rung) |
| 27 | src/rune_sec/command_palette.rs | +13 commands: 8 workspace jumps + guides/frustum/safe-frame/left-panel/right-panel toggles — 84 → 97 (counted in the `COMMANDS` table; the file's own header agrees. An onion entry already existed, and no solo-clear was added) |
| 28 | src/rune_sec/status_strip.rs | Runtime inside the project segment (`Project::total_frames`, the mixer's own clock) |
| 29 | src/rune_sec/stats_view.rs | Media block: 4 rows (audio clips with their start-frame span — clip lengths live in the sound files, not the model; FX clips with summed duration; texts; strokes) |
| 30 | src/rune_sec/help_panel.rs | 24 → 30 rows — every claim verified against the tree (two briefed items corrected: no Alt+1..8 row existed; loop is transport/palette, not a key) |
| 31 | src/rune_sec/project_tools.rs | "Current file" line — `current_file_line` renders the `shell::last_opened_project` name, shown only on an exact match with `Project::name` (the match guard is the point: recents store names, not the open file) |
| 32 | src/rune_sec/draw_tools.rs | `draw-stats` readout (fields verified: `points: Vec<V2>`, `frame: i32`, `spray: Option<SprayMark>`) |
| 33 | src/rune_sec/stroke_inspector.rs | `stroke-measure` line (frame via the file's own `stroke_frame_of`) |
| 34 | src/rune_sec/scene_panel.rs | `scene-stage-readout` with gcd-reduced aspect (decimal fallback past 40) |
| 35 | src/rune_sec/layers_panel.rs | Show all (the figures footer's body, shared) + Solo selected via `toggle_solo` |
| 36 | src/rune_sec/text_tool.rs | Row previews now use the new stricter `text_preview` (14 chars, newlines flattened to spaces) for the load-button label and the Loaded line; the older 18-char `preview()` helper remains in place |
| 37 | src/rune_sec/library_panel.rs | File-stem matching added to the existing search + the missing count line (an integrator flag claimed "pen"→"pencil"; the tree already used "pencil" — nothing to change) |
| 38 | src/rune_sec/settings_popover.rs | Playback block: Loop toggle (inverse label idiom) + audio state line |
| 39 | src/rune_sec/snap_tests.rs | Round 6: 19 → 23 tests (palette jump, key jumps, marker seek, inert pose-paste) — see §5 |
| 40 | docs/ | This document + QA_RUNE §11 (rewritten by the integrator — see the provenance note) |

## 4. Cross-section contracts

- `clip_peaks` does not apply gain; the waveform panel
  scales by gain at draw time, as it always did (§§4/17).
- The FX kind order lives in the lib as `FX_KIND_ORDER`
  because the lib cannot import the shell's `fx_browser`;
  the two are pinned to agree by §7's coverage test (§7/§15).
- Inert buttons: fill `t.card`, muted foreground
  `text_secondary` — the same tone the local wrappers used,
  so migrated panels did not change shade (§11).

## 5. Integrator wiring (done at integration, not by a section)

- `main.rs` poll: `sync_playback_audio_on_seek` called
  beside the wrap sync (§1's other half).
- `export_panel.rs`: `ExportOp::ContactSheet` +
  `export_contact_action` + the `xc-contact` range row —
  §8's lib pair had **no caller** until this wiring
  (verified by grep at integration).
- §23's migration is partial by design: pose variant
  buttons (half/light/dark) carry an *active* tint in
  their inert form (`accent.alpha(0.35)`), which
  `icon_btn_inert` does not model, so `pibtn` keeps that
  one branch locally and delegates the rest.
- §39's pose test pins the inert contract end to end:
  `pose-paste` in its inert form (pose_tools' `pbtn`,
  no press handler attached) is present, and a tap
  changes nothing — status, undo depth, frame, joint
  positions and the empty clipboard all unchanged
  (green in CI run 42467099217). The shared builders
  (§§24–26) are handler-free by construction.

## 6. Inventory snapshot

Integrator's post-wiring run of
`python3 scripts/rune_inventory.py`:

- **612 unique keys** (521 static + 91 dynamic), vs 567
  at Phase H — and a key-set diff against the Phase H
  tree shows **zero keys lost**.

## 7. Honest remainders after Phase I

- Two "New project" results still disagree (panel blank
  vs Home demo) — the user's product call, untouched.
- Duplicate graph channels list twice and resolve to the
  first (now also true of their stats rows).
- The head ring radius is a constant 17.0 everywhere.
- `core.onion_frames` / `cycle_onion_frames` remain
  orphaned engine API (the shell keeps depths in State).
- The status strip's Audio segment still has no glyph
  (the Phase H width-maths refusal stands).
- Pose variant buttons keep a local inert branch (§5).
- Rune `fill_path` hole rendering is still not
  pixel-verified; Phase F–I pixels and sound still await
  the user's screenshot + Play test on a real screen.
- `headless.yml` is **proven now**: run 42467268661 on
  the green tree — all 6 probes exit 0 on llvmpipe
  (software Vulkan), 4 `--tree` dumps + 2 `--snap`
  PNGs. The default snap renders the full shell in
  pixels (icons, the Phase I measure/runtime lines,
  the inert Paste pose all read correctly). Two layout
  overlaps are visible in that render and are recorded
  here as new remainders: the Keyframes header's
  "No joint selected" note overlaps its Delete button,
  and at the snap's 2400×1350 the right column's
  bottom rows crowd the status strip's tail.
