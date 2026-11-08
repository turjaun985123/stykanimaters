# Wave 9 — Independent Acceptance Review (agent E10)

Adversarial review of `docs/ui_mission/ACCEPTANCE.md` against the
current source. Method: grep-level evidence only; no builds (repo
policy), no git writes. The five most load-bearing claims were
selected and each was attacked — the aim was to disprove, not to
confirm.

**Reading window: 2026-11-08T16:30:31Z – 16:32:44Z.** HEAD at
reading: `23abfb0` (acceptance doc) on `d1a548f` (mission-final
code). The working tree was dirty throughout with Wave 9 work in
flight (36 files modified by the final reading, including
`src/export.rs`); where the committed tree and the working tree
disagree, both readings are reported.

## Claim 1 — "Zero bare empty lines" (criterion 4; Wave 6-B)

**CONFIRMED.**

- `grep -rn 'text("No ' src/rune_sec/` → **0 hits**.
  `grep -rn 'text("Nothing' src/rune_sec/` → **0 hits**. A
  multi-line variant check (`text(` with the string on the next
  line) also returns 0.
- The panel empty messages that remain are all routed through the
  kit builder, e.g. `src/rune_sec/camera_panel.rs:844`
  `design::empty_state("key", "No camera keys yet — Set key pins
  the current view.")` and `src/rune_sec/easing_panel.rs:437`
  `design::empty_state("chart-line", "No graph channels on this
  figure yet — …")`. `empty_state(` has 31 call sites in
  `src/rune_sec/`.
- Every other `"No …"` / `"Nothing …"` literal in the tree is one
  of: a `core.status = …` assignment (the classified status/toast
  channel — a different surface, and the subject of Wave 6-C, not
  of this claim), a doc comment, or a test assertion. None is a
  bare panel `text(...)` line.

## Claim 2 — "Nothing lost": keys preserved, only the listed retirements (criterion 5)

**CONFIRMED**, with two extraction artefacts explained and one
wording nuance.

Retirements, verified against current source:

- `"loop-start"`, `"loop-end"`, `"stage-overlay-row"`: **0 hits**
  anywhere in `src/`.
- `"menu-edit-sep"`: the only occurrence in `src/` is
  `src/rune_sec/snap_tests.rs:3480`, `!keys.contains("menu-edit-sep")`
  — the test that pins its absence. The key is not mounted.
- `"loop-clear"` survives exactly where the review says:
  `src/rune_sec/timeline_marks.rs:1172`.
- Camera sub-claim ("all 28 camera keys survive"): literal-key
  extraction (the gate script's own `extract_keys`) on
  `src/rune_sec/camera_panel.rs` gives **28 keys at the mission
  baseline (`7fdd535`, Phase N close) and 28 at `d1a548f`, zero
  lost, zero gained**.

Unlisted-loss hunt (the adversarial part): the gate's
`extract_keys` applied to every `src/**/*.rs` at `7fdd535` vs
`d1a548f` reports LOST = `loop-end`, `loop-start`,
`stage-overlay-row`, `tog`:

- `tog` is **not a UI key loss**. At baseline it appears only at
  `src/rune_sec/design.rs:476-477`, inside the `design::toggle`
  builder's own unit test as a dummy key (`toggle("tog", true,
  …)`). Wave 2 retired the `design::toggle` builder itself (a
  recorded Wave 2 change), which took its test fixture with it.
  `tog` was never mounted in any rendered tree.
- `menu-edit-sep` does not appear in the diff because the gate's
  extractor never counted it on either side: at baseline it was
  mounted as `separator("menu-edit-sep", t)`
  (`menus.rs:785` at `7fdd535`), and `separator(` is not one of
  the extractor's keyed-builder patterns. Its retirement is
  nonetheless real (see above) — an extractor blind spot, not a
  doc error.
- Gained: +24 literal keys net (593 vs 573 by this extraction),
  consistent with the additive keys the waves recorded. (The
  doc's "809 distinct keys" comes from the fuller inventory with
  dynamic patterns; that census was not recounted here because
  the inventory tool reads the working tree, which Wave 9 has in
  flight. The load-bearing half of the claim — nothing lost
  beyond the listed changes — is what the diff above tests, and
  it holds.)

Wording nuance: the doc lists "`fig-duplicate`→`insp-fig-duplicate`
rekey" among "removals". In fact **no key was removed**: at
baseline two different buttons collided on the one key
`"fig-duplicate"` (`figures_panel.rs:683` and
`inspector_figure.rs:924` at `7fdd535`). Wave 1 rekeyed the
inspector's mount to `insp-fig-duplicate`
(`inspector_figure.rs:941` today); `fig-duplicate` itself
survives in the figures panel (`figures_panel.rs:719`). The
substance (nothing lost; the only changes are the listed ones)
is correct; a reader could misread the line as `fig-duplicate`
being gone. It is not.

## Claim 3 — Loop controls in exactly one home (criterion 5; Wave 5)

**CONFIRMED.**

- All six loop-range editing keys exist only in
  `src/rune_sec/timeline_marks.rs`: `loop-set`, `loop-clear`,
  `loop-a-dn`, `loop-a-up`, `loop-b-dn`, `loop-b-up`.
- `src/rune_sec/frame_ops.rs`: **zero** loop keys (the Wave 5
  duplicate block is gone, as recorded).
- `src/rune_sec/playback_controls.rs` retains only `pb-loop`
  (the loop-playback on/off toggle — transport behaviour, not
  range editing) and `pb-loop-start` / `pb-loop-end`, whose
  handlers (`playback_controls.rs:598-608`) call only
  `s.core.goto(a)` / `s.core.goto(b)` — playhead navigation;
  they never write the range.
- Every other `loop_range = …` / `set_loop_range(…)` write in UI
  files sits inside `#[cfg(test)]` modules
  (`camera_panel.rs:1694`, tests from line 1147;
  `keyframe_tools.rs:2008+`, tests from line 1325). No second
  production UI writer exists. (`scenes.rs`'s `"loop-range"` is
  a headless scene name, not a control; MCP loop tools are the
  API surface, not a UI home.)

## Claim 4 — Wheel zoom never writes the camera (Wave 5)

**CONFIRMED** — and provable at the type level, not just by
call-graph reading.

- There is exactly **one** `on_wheel` mount in the whole app:
  `src/main.rs:940`, routing to `stage_wheel` (`src/main.rs:386`).
- `stage_wheel` calls only `stage_view::view_zoom_at(&s.core,
  …)`. Its doc comment records the pre-Wave-5 behaviour
  (`camera_zoom_by` per step) as removed history.
- `pub fn view_zoom_at(core: &EditorCore, …)` (`src/stage_view.rs:295`)
  takes the core by **immutable** reference: it cannot upsert a
  camera key. It reads the core for the display transform and
  writes only `static VIEW_NAV: Cell<ViewNav>` (thread_local,
  `src/stage_view.rs:169-170`) via `set_view_nav`.
- The remaining production callers of `camera_zoom_by` are the
  Camera panel's authoring controls
  (`src/rune_sec/camera_panel.rs:222, 975, 979` — the 1:1 action
  and the zoom steppers), i.e. deliberate camera edits through
  the panel whose job that is — not the wheel.

## Claim 5 — Export copy matches the engine: range MP4 silent, WebM never has audio (criterion 4; Wave 4)

**STALE** — true at the acceptance commit; falsified for range
MP4 by engine work in flight in the working tree during this
reading. Panel copy not yet updated.

- Panel copy as found (unchanged from the mission-final tree):
  `src/rune_sec/export_panel.rs:254-255`,
  `RANGE_AUDIO_NOTE = "Range video exports are silent — audio is
  only in a full-timeline MP4 export."`, shown under key
  `xc-range-audio-note` when `range_audio_note_shown(...)`
  (line 1285). Module docs (lines 25-35, 249-253) state the same
  engine truth: full `export_mp4` muxes, `export_mp4_range` is
  video-only, WebM never muxes. The WebM format line (line 1404)
  says "video-only — WebM never includes audio".
- Engine at HEAD (`d1a548f`): matches — `export_mp4_range` was
  the plain video-only encode; the acceptance claim was correct
  when written.
- Engine in the working tree (read 2026-11-08T16:32:34Z;
  `git diff HEAD -- src/export.rs`, +31/−12): `export_mp4_range`
  is rewritten to mux the audio segments intersecting the range
  — new doc comment "AUDIO: the segments of the project's audio
  that intersect the range are muxed in, trimmed and offset to
  the range — see [`range_mux_plan`] / [`mux_audio_range`]", and
  the body now encodes to a silent temp file and calls
  `mux_audio_range(&plan, &silent, path)`. Against this tree,
  "Range video exports are silent" is **false for MP4 range
  exports** (it remains true for WebM: the same diff re-confirms
  `export_webm_range` as video-only, "no audio support at any
  range").
- Caveat on the in-flight state: as of the final reading
  (16:32:44Z) the called helpers — `range_mux_plan`,
  `mux_audio_range`, `range_window`, `RangeMuxPlan` — are
  **defined nowhere in `src/`** (only referenced at
  `src/export.rs:853-868`). The engine change is mid-landing and
  the tree as found would not compile. The verdict is therefore
  STALE rather than DISPROVED: the claim described the engine
  truthfully at acceptance, the engine is now being changed
  under it, and the moment that change lands, `RANGE_AUDIO_NOTE`,
  its `range_audio_note_shown` predicate and the export-panel
  module docs must be updated with it or criterion 4 ("no
  misleading copy") fails on this point.

## Summary

| # | Claim | Verdict |
|---|-------|---------|
| 1 | Zero bare empty lines | CONFIRMED |
| 2 | Keys preserved, only listed retirements | CONFIRMED (artefacts + one wording nuance noted) |
| 3 | Loop controls in exactly one home | CONFIRMED |
| 4 | Wheel zoom never writes camera | CONFIRMED (type-level proof) |
| 5 | Export copy matches engine audio truth | STALE (range-MP4 half; engine change in flight, copy not yet updated) |

No claim was found fabricated. Four of five survive a hostile
reading of the current source intact. The fifth is an honest
claim overtaken by parallel work in the same working tree — the
acceptance doc's Wave 4 statement and the standing remainder
"range video exports carry no audio" both need revisiting when
the range-audio engine change lands.
