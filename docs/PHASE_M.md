# Phase M — the 3D figure on the main stage, three model flags, frame swap, plus parity

Phase M opened on Phase L's remainder list: the
3D figure drawn only inside its panel (Phase
K §33's main-stage branch still unbuilt), and a
set of behaviours the model could not express —
an audio clip that could not be muted, a stroke
that could not be translucent, an FX clip that
could not be hidden. Around those, the phase
added frame swap with a full remap census, graph
key depth, an MCP parity batch that brings the
tool total level with the palette's Phase L
count, and a second pass of panel depth.

> **A note on provenance.** This document was
> written by the phase's docs section under the
> rule Phase J established: every row below was
> checked against the tree before it was written,
> and rows whose work is not in the tree at
> writing time are marked **per brief —
> integrator to verify** rather than described
> as done. The integrator re-verifies at
> integration time, as before. Section numbers
> are pinned by the files' own in-file numbering
> where a file states one (§8 audio_ops,
> §21 figures_panel, §38 these docs); the
> remaining numbers are this document's
> assignment of the charter's items to the
> verified work — every file:line anchor was
> grep-verified, but if the parent plan numbered
> those slots differently, only the numbers
> shift; the anchors stand. One count was
> flagged unresolved at this census (an MCP
> registry grep counting 92 against a
> completeness pin of 91); at integration it
> resolved in the pin's favour — the grep had
> also matched a non-registry occurrence, and
> the parity batch added four tools, not five,
> because `fx_set_color2` already existed.
> Registry 91, total 102 (§5).

## 1. What Phase M set out to close

From Phase L's remainder list (§7 of PHASE_L.md
and QA_RUNE §14):

- **The 3D figure on the main stage** — Phase L
  took the panel-canvas branch of Phase K §33's
  design decision and left the other branch
  explicit: the figure drawn inside the 3D
  panel's viewport only. Phase M builds the
  stage branch as a complement, not a
  replacement: a new stage-space geometry module
  (§1) and a stage pass behind an overlays flag
  (§2), with the panel preview unchanged.
- **Three model flags, all serde-defaulted** so
  old projects load unchanged: `AudioClip.muted`
  (§§5–8), `Stroke.opacity` (§§9–11) and
  `FxClipEntry.hidden` (§§12–13). Each follows
  the same shape — one field, the playback /
  render consumers honouring it, one panel
  surface, and (for mute) an MCP tool.
- **Frame swap** — frames could be inserted,
  duplicated and deleted, but two frames could
  not change places without hand-moving every
  collection pinned to a frame index. §14 owns
  the op and its remap census; §15 is its panel
  surface.
- **Graph depth** — a channel's keys could be
  deleted in a range (Phase K) but not shifted
  as a block, and a loop range could not be
  filled with keys. §§16–18.
- **MCP parity batch** — ops the shell had and
  MCP did not join the registry (§19):
  `set_key_interp`, `particle_set_kind`,
  `particle_set_color`, `audio_set_muted` —
  four new tools, not five, because the fifth
  candidate (`fx_set_color2`) already existed.
  Registry 87 → **91 entries**, total 98 →
  **102 tools**. The palette grows the other
  way, 103 → **108 commands** (§20).

And the new depth: a framing camera for the 3D
session, a direct per-frame SVG writer, a
timeline clips lane, settings speed chips, and
the small surfaces listed in §3.

## 2. Groundwork

None this phase beyond premise greps — stated
plainly, in the Phase K manner: every section
premise was grep-verified against the tree
before the sections were briefed. The phase's
one piece of new shell view state is the stage
pass's enable flag, which lives on
`StageOverlays` (§2), not in State. This docs
census ran while several sections were still
landing; the per-brief marks below are that
census's honest output, and §5 collects what
the integrator still owes.

## 3. The sections (charter census)

| § | File | What landed |
|---|---|---|
| 1 | src/rune_sec/stage_fig3.rs | Stage-space 3D geometry (headline): `stage_fig3_prims(core)` (:91) — a pure function of the core; with no session it returns nothing, with a session the figure is evaluated once and projected with the **session camera through the project stage size**, so the prims land in the stage coordinates the stage's own camera transform expects. Every returned segment is clipped to the stage rect `[0, stage_w] × [0, stage_h]` through Phase L's own `fig3_preview::clip_segment_to_viewport` (:111) — the stage canvas does not clip either, and unclipped lines would paint over the shell exactly as the panel grid did before 2ba5e02. The ground offset follows the figure's feet through `fig3_preview::ground_y_of`, the Phase L integrator correction reused |
| 2 | src/stage_view.rs | The stage fig3 pass: `StageOverlays::fig3_stage` (:366, default `false` :388) gates a pass (:742) that draws §1's prims when the flag is set **and** a session exists (`core.fig3.is_some()`). The flag's caller is the integrator's wiring (§5): the `StageOverlays` literal in main.rs sets it to `workspace == Build3D && core.fig3.is_some()` |
| 3 | src/fig3d.rs | `frame_camera_points(points, fov_deg, aspect, yaw, pitch)` (:277) — the new point-cloud framer: centroid target, bounding-sphere distance with its documented headroom factor and clamps, `preview_camera()` fallbacks for degenerate input. The `_points` name exists because `frame_camera` (:363) already belonged to the older whole-figure framer, which is untouched |
| 4 | src/rune_sec/three_d_panel.rs | Preview framing + zoom (panel docs :84–95): a Frame button (`d3-frame`, :867, through `ictl`) running §3's camera on the session, and zoom steppers (`d3-zoom-dn` / `d3-zoom-up`) stepping the camera distance through the pure `zoom_distance`. The older `cam-frame-3d` control (:968) is untouched |
| 5 | src/audio.rs + src/model.rs | The three flags' fields, each serde-defaulted so old projects load unchanged: `AudioClip.muted` (audio.rs; `playback_plan` filters muted clips out at :110 — a muted clip never sounds directly and never joins a mix) · `Stroke.opacity` (model.rs :511, `#[serde(default = "default_stroke_opacity")]` :515, the field :516) · `FxClipEntry.hidden` (model.rs :712) |
| 6 | src/rune_sec/stage_fx.rs | Hidden FX clips draw nothing on the stage: an entry the user has hidden is skipped whole — no motes, no trail (:180–181). The skip is this layer's own; figure-hidden handling above it is unchanged |
| 7 | src/export.rs | The rasterizer honours two flags: per-stroke opacity applied at :424 ("Per-stroke opacity (Phase M), recorded from the brush"), and hidden FX entries skipped at :476, pinned by `hidden_fx_clip_renders_nothing` (:1743). Integrator addendum: the export **mux plan now excludes muted clips** (the section flagged that `mux_plan` lacked §5's filter; it mirrors `playback_plan`'s, with a regression test) |
| 8 | src/audio_ops.rs | Mute ops (file self-numbered §8): `audio_set_muted_at` (:209) + `audio_toggle_muted_at` — the indexed family gains its mute pair under the family's discipline: a no-change press takes **no snapshot**, so a repeated mute press cannot bury the undo stack in no-op steps (module docs :82–94) |
| 9 | src/editor.rs | Stroke opacity recording: `recorded_stroke_opacity` (:104–108) clamps the brush opacity to [0.0, 1.0] (a NaN brush opacity reads as opaque) and is what a newly recorded stroke carries, at both recording sites (:2599, :2825) |
| 10 | src/svg.rs | Two items: the `stroke-opacity` attribute for translucent strokes — `stroke_opacity_attr` (:22–32) emits the attribute only below opacity 1, clamped, so a fully opaque stroke's file is byte-identical to before; and the direct range export — the per-frame writer extracted as `write_frame_svg` (:61) with `export_svg_range` (:293) rendering each frame through it directly, no scratch directory (§22) |
| 11 | src/paint.rs | The translucent preset: appending an eleventh preset proved unsafe (panel tests pin the library at 10 with Spray last), so **Pencil (index 1) was converted to opacity 0.55** — the one preset besides Spray (0.5, mark-only) whose opacity now reaches `Stroke.opacity` through the recording sites. The `Brush::opacity` doc was rewritten to say the field is consumed |
| 12 | src/rune_sec/fx_clips.rs | Hide toggle: `toggle_selected_hidden` (:938) behind the `fx-hide` key (:1260), and the readout names the state — the strip/readout line appends ` · hidden` (:324). Hidden clips stay editable; the flag governs drawing (stage §6, export §7), not editing. The gizmo for a selected hidden clip still draws (§6's anchor path never samples particles — verified there, not a bug) |
| 13 | src/rune_sec/audio_panel.rs | Mute button: `audio-mute` (:833) through `icon_btn`, its label and glyph naming the action a press performs ("Mute" / volume glyph state) over §8's toggle, fed by the effective target like the panel's other per-clip ops; the muted readout line sits under the clip rows (module docs :42–70) |
| 14 | src/frame_swap.rs | Frame swap, engine half (new file): `swap_frames(core, a, b)` (:116) exchanges two frames' contents and remaps **every** frame-indexed collection — the file's census (module docs, each entry verified against model.rs / vfx.rs / graph.rs): `camera` keys, `particles`, `strokes`, audio `start_frame`s, `fx_clips` starts (swapped when equal to a/b exactly), graph `Key.frame`s, and `texts` / `sprites` instance keys; order invariants are restored by re-sorting the lists that rely on ascending order, not assumed. `swap_frame_with_neighbour` (:183) is the panel-shaped wrapper; refusals are status-only with no snapshot |
| 15 | src/rune_sec/frame_ops.rs | Swap buttons: `fop-swap-prev` (:607) / `fop-swap-next` (:626) as a live/inert icon pair gated by `swap_prev_enabled` (:240) / `swap_next_enabled` (:248) — at the timeline's ends there is no neighbour, so the button is inert rather than refusing. The palette commands (§20) and the Edit-menu item (§34's menus work) drive the same op |
| 16 | src/graph_keys.rs | Graph key depth, library half: `shift_keys(core, channel_index, delta)` (:491) moves every key of one channel by the same offset — the whole shift is refused, never clamped per key, so a channel can never be distorted by a partial move; `fill_keys_in_range` (:544) is its fill counterpart |
| 17 | src/rune_sec/graph_panel.rs | Shift buttons: `graph-shift-l` / `graph-shift-r` ("Shift -1" / "Shift +1", panel docs :281–290) in the channel block, the handler `shift_drawn_channel` resolving through the panel's drawn channel exactly as the canvas does, over §16's `shift_keys` |
| 18 | src/rune_sec/keyframe_tools.rs | Fill range: `key-fill-range` (grip-vertical glyph, module docs :188–199) in its own row under the clear-range row, over §16's `fill_keys_in_range`; the guard (`fill_range_enabled`) is a loop range set AND a channel resolved — the op owns the rest |
| 19 | src/mcp_tools.rs | MCP parity batch: four tools join the registry — `set_key_interp`, `particle_set_kind`, `particle_set_color`, `audio_set_muted` — each dispatching to the op the shell already had (Phase L §29's `set_key_interp`, Phase I's particle ops, §8's mute). The fifth briefed tool, `fx_set_color2`, **already existed** (handler + registry + tests from an earlier phase); re-adding it would have broken the uniqueness pin. Registry 87 → **91**, and the completeness pin was raised to match. Total (registry + 3 built-ins + 8 IO tools): **102** |
| 20 | src/rune_sec/command_palette.rs | Palette growth 103 → **108 commands**: `mute-audio-clip`, `swap-frame-prev`, `swap-frame-next`, `shift-channel-right`, `fill-keys-range`, each a shim onto the landed op (mute resolves through the audio panel's `effective_target`, the graph pair through the panel's `effective_channel_index`); the count pin reads 108 |
| 21 | src/rune_sec/figures_panel.rs | Duplicate button (file self-numbered §21): `fig-duplicate` (copy glyph, module docs :91–93) beside Recolour, duplicating the selected figure through the remapping duplicate op — until this phase the panel could add and delete but not duplicate |
| 22 | src/export.rs + src/svg.rs | SVG range: §10's direct writer landed in svg.rs, and **the integrator rewired `export::export_svg_range` onto it** — the scratch-dir body is deleted; the export.rs path is now a one-line delegation, and its range tests pass against either home |
| 23 | src/rune_sec/waveform_view.rs | Muted clips stay visible: in clip mode a muted clip's bars render at `MUTED_BAR_ALPHA` of their usual alpha (:173) and its span bracket dims by the same factor (:179); the caption names the state — "Clip {n} · muted" through `clip_caption` (:175) — while the unmuted caption is byte-identical to Phase I's |
| 24 | src/rune_sec/timeline_marks.rs | Timeline clips lane (`timeline-clips`): a content-width canvas row above the loop row, present only when the project carries clip spans — audio spans (accent) and FX spans (accent-2) laid out in the chips' geometry via pure `clip_spans` / `span_rect`, muted/hidden spans dimmed to the phase's 0.35 alpha; clip-free projects keep a byte-identical timeline |
| 25 | src/rune_sec/settings_popover.rs | Speed chips: a direct-select Playback speed row (module docs :113–119) — one chip per speed ("0.25×" … "2×", `speed_chip_label`), the chip for the current `State.playback_speed` active; the popover names the speed directly instead of only cycling it blind (the Phase K cycle remains in playback_controls and the View menu) |
| 26 | src/rune_sec/home_panel.rs | Home rows show file size: the row's metadata line gains the file's byte length through `human_size` (imported from project_tools, :198; the size line at :559) — the Phase J metadata (figures · frames · fps · modified) gains its missing member |
| 27 | src/rune_sec/text_tool.rs | Text z-order: **Bring to front** (`text-front`) moves the loaded instance to the END of the texts list and **Send to back** (`text-back`) to index 0 (module docs :184–186) — draw order is list order, so the pair is the whole feature, under the panel's one-snapshot discipline |
| 28 | src/rune_sec/clips_panel.rs | Insert selected clip: the per-row "Insert" now inserts **that row's** clip — `insert_clip_figure_at` (module docs :28–34, Phase M) parameterises the engine op's construction with the row's own index, where `insert_clip_figure` always inserted clips[0]. The clips vec is untouched, so no stored clip index shifts and neither the panel selection nor an open rename target needs repair |
| 29 | src/rune_sec/stats_view.rs | Flags block: `flag_totals` (module docs :16) counts the project's flag state (muted clips, hidden FX, translucent strokes among them) as a captioned block of muted rows (:426, :437) under the media totals |
| 30 | src/rune_sec/export_panel.rs | Range-audio warning: a note (key `xc-range-audio-note`, decision in `range_audio_note_shown`, module docs :25–33) states that range video exports carry no audio where the panel offers them — the full MP4 export muxes audio, the range path cannot, and until this phase nothing said so |
| 31 | src/rune_sec/status_strip.rs | Status-strip 3D segment: `fig3_segment(active)` composes a text-only "3D" segment after the speed segment while a session is live (no glyph, by the strip's conditional-segment convention); with no session the segment is absent and the Phase K pinned floor arithmetic is untouched |
| 32 | src/rune_sec/scenes.rs | Eleventh scene: `three-d-stage` (`scene_three_d_stage`, :322; table :309) reaches **exactly** the end state `three-d` reaches, plus the stage flag's state the scene exists to prove — the two scenes' sameness is deliberate, so a tree dump diff isolates the stage pass. Scene count 10 → **11**, pinned (:520) |
| 33 | src/rune_sec/help_panel.rs | Help refresh 48 → **54 rows**: six Features rows for the phase's surfaces (3D on the stage, mute, stroke opacity, frame swap, graph shift, hidden FX), placed so no earlier phase's pinned adjacency broke; the count pins read 54 |
| 34 | src/rune_sec/stroke_inspector.rs | Stroke opacity steppers: `stroke-opacity-row` under the width row, mirroring its discipline exactly — pure `opacity_after` (0.1 steps, clamped [0.05, 1.0]; the floor keeps a stroke faintly visible and selectable), `opacity_label` percentages, one snapshot per press. Spray selections show no row: their opacity lives on the mark, so a field stepper would be a fake control |
| 35 | scripts/phase_j_checks.py | The gate widened in place (Phase J+K+L+M): the Phase M caller census plus the same-file `swap_frames` entry; at integration the census target was corrected from `frame_camera` to `frame_camera_points` (the older name is a different op with its own caller). The integrator's run is §6 |
| 36 | src/rune_sec/snap_tests.rs | Runtime round 10: swap-button exchange, 3D-session stage state, settings speed chips (§35's three), plus the integrator's fourth — the clips lane appearing with a planted FX clip — for 39 tests in the file |
| 37 | src/rune_sec/stage_view.rs | (counted at §2 — the pass and the flag are one section's work) |
| 38 | docs/ | This document + QA_RUNE §15 |

## 4. Cross-section contracts

- **The stage 3D pass (§§1/2/3/32).** §1 owns
  which segments exist in stage space; §2 owns
  the gate (workspace flag AND live session);
  §3 owns the framing camera the panel's Frame
  button shares with the stage view. The
  projection is the bake paths' own pair
  (`fig3d::evaluate3` + `fig3d::project`), and
  the clip is Phase L's, reused rather than
  re-derived — one clipping rule for both
  canvases. §32's scene exists to prove the
  pass in a tree dump. The panel preview
  (Phase L §§1–3 of that phase) is untouched:
  the stage branch complements it, it does not
  replace it.
- **The three flags (§§5–13, 19, 23, 29).** One
  discipline, three times: the field is
  serde-defaulted (old files load with the flag
  off, behaviour identical to before); the
  consumers that must honour it are the ones
  that produce output (playback plan, stage,
  rasterizer, SVG); the panel surface names the
  state where it shows it (readout, caption,
  dimming) so a silenced or hidden thing never
  looks identical to a live one.
- **Frame swap (§§14/15).** §14 owns the census
  — the list of frame-indexed collections is
  written in its module docs and verified
  against the model files, so a future
  collection added to the model has one place
  that must grow. §15 owns only the press
  discipline (inert at the ends, never a
  refusal).
- **Counts are pinned in the files that own
  them**: scenes 11 (§32's test), palette 108
  (§20's pin), MCP registry 91 with its
  completeness pin raised to match (§19),
  help 54 (§33's pin), icons 111 (unchanged).
  A count in these documents can be checked
  against its pin in seconds — which is how
  the census's one mismatch (a grep count of
  92 against the pin's 91) was caught and
  resolved in the pin's favour (§5).

## 5. Integrator wiring (done at integration)

- **`StageOverlays::fig3_stage` — WIRED.** The
  main.rs literal now sets the flag to
  `workspace == Build3D && core.fig3.is_some()`,
  so the stage pass draws exactly in the 3D
  workspace while a session is live, and the
  `three-d-stage` scene (§32) exercises it.
- **Module declarations**: `pub mod frame_swap;`
  in lib.rs and `pub mod stage_fig3;` in
  rune_sec/mod.rs; eight `crate::frame_swap::` /
  `crate::audio_ops::` paths in frame_ops,
  menus and command_palette were corrected to
  the bin crate's `stykanimate_rs::` convention.
- **The literal sweep**: 60 `Stroke` /
  `AudioClip` / `FxClipEntry` literals across
  16 files gained the new field with its
  neutral default (verified by re-running the
  sweep to zero). Two sites got the semantic
  value instead: `paint::erase_at`'s split
  pieces now copy the source stroke's opacity,
  and the draw panel's stamp records the
  active brush's opacity like the editor's own
  stamp.
- **Stage stroke alpha**: stage_view's base
  Polyline/Filled conversions now apply the
  prims' carried opacity (§14's field); the
  highlight pass keeps its flat alpha by that
  section's pinned composition decision.
- **export.rs**: `mux_plan` excludes muted
  clips (new regression test) and
  `export_svg_range` delegates to §10's
  direct writer (§22).
- **MCP registry count resolved**: 91 entries,
  pin correct — the census grep had matched a
  non-registry occurrence, and the batch added
  four tools because `fx_set_color2` already
  existed. Total 102.
- **Gate census rename**: `frame_camera` →
  `frame_camera_points` (§35), so the census
  checks the op this phase added.
- Everything the docs census marked per brief
  landed before integration: palette 108,
  the Edit-menu swap/mute items, the timeline
  clips lane, the status 3D segment, help 54,
  the stroke opacity steppers, the gate
  widening and Runtime round 10.

## 6. Inventory snapshot

**INTEGRATOR (final)** — the Phase J+K+L+M
gate run on the integrated tree, base
9898e31: **PASS, all checks green**. Inventory
**733 unique element keys** (631 static + 102
dynamic), **zero lost** against Phase L's 704.
Caller census: every Phase M op wired
(`swap_frame_with_neighbour` ×5 call sites,
`shift_keys` / `fill_keys_in_range` ×2 each,
`audio_set_muted_at`, `frame_camera_points`,
`stage_fig3_prims` ×1 each; `swap_frames`
same-file ×2). State census unchanged (no new
State fields). Source-pinned counts: palette
**108** commands, MCP registry **91** (total
**102** tools), curated icons **111**, headless
scenes **11**, help **54** rows. CI verdict:
**GREEN — run 42478556459 on 6a9efd8: 2,451
tests passed (996 lib + 1,445 bin + 10
integration), 0 failed.** The road there took
five fix commits, each diagnosed from the
failed run's own log: two shorthand-literal
sweeps the gate's colon-field census could
not see (mcp_tools' `add_fx` literal, an
editor test helper, then the integration
tests in tests/export_determinism.rs), the
flag tests' missing `Stroke`/`FxClipEntry`
imports in model.rs's `audio_gain_tests`
module, a closure-lifetime error in
text_tool's z-order test, a brush-step test
premise §40's Pencil conversion invalidated,
and the clips-lane Runtime test learning
that a direct state write needs
`Runtime::invalidate()` before the view
re-runs. Fix 4 also carries a production
fix the swap test exposed: the Frames
section's main row never wrapped, so its
tail buttons (Swap included) sat clipped
past the side column's edge, unpressable —
the row now wraps (`flex_wrap`).

## 7. Honest remainders after Phase M

- Sound on a real screen is still the user's
  to confirm: the headless snap covers pixels,
  never audio, and every sync correction
  remains a bounded restart, not a
  sample-accurate clock. Phase M's mute flag
  gives the user a way to silence a clip; it
  does not change what verification the
  playback path still needs.
- Two "New project" results still disagree
  (panel blank vs Home demo) — the user's
  product call, untouched again.
- Range MP4/WebM exports still carry **no
  audio** — the range path renders video only.
  Phase M's change is that the panel now
  **says so** (§30) where the range exports
  are offered, instead of the silence being
  discoverable only by playing the file.
- `core.onion_frames` / `cycle_onion_frames`
  remain orphaned engine API, by the Phase G
  design (the shell's asymmetric onion depths
  cannot be expressed by the engine's single
  symmetric count).
- The status strip's Audio segment still has
  no glyph — Phase K's measured refusal stands
  (753.75 pt of floors against the strip's
  744 pt).
- MCP (102 tools) and the command palette
  (108 commands) are **different surfaces**;
  Phase M narrows the gap from opposite
  directions, and the sets are still not
  identical — parity of counts is not parity
  of sets, and nothing here claims it is.
- The 3D figure on the main stage is wired
  (§5): the flag is set for the Build3D
  workspace, so the figure draws on the stage
  there while a session is live. The panel
  preview remains the orbit-by-drag surface;
  the stage pass is a drawing, not a second
  editor for the 3D pose.
- Brush **scatter** for non-spray strokes is
  still model data only (a Wave 12 remainder):
  Phase M wired opacity end-to-end and left
  scatter exactly as it found it.
