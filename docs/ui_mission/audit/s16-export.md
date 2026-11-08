# Squad 16 Audit — Export & Project Operations (Phase 1)

Scope: `src/rune_sec/export_panel.rs`, `src/rune_sec/project_tools.rs`,
`src/projects.rs`, and the UI-facing surfaces of `src/export.rs` /
`src/editor.rs`'s export ops. Method: full source read of the two
panels and `projects.rs`; targeted reads of `export.rs` (export fn
signatures, ffmpeg paths, sequence writers) and `editor.rs` (op
status wordings, named save). Cross-checked against `toasts.rs`
(`classify_status`), `home_panel.rs` (the other project list),
`chrome_view.rs` (app-bar Save/Load), the Phase N headless tree
set, and `docs/phase_o_plans/p05-overflow-widths.md`.

## What already works — do not rebuild

- The range-toggle honesty machinery: `effective_text` states
  what the next export will actually cover in every
  toggle/range combination, including admitting the fallback
  when "Loop range" is set but no range exists; `loop_note`
  names the range 1-based. (One wording bug inside it: D1.)
- Bundle integrity: `plan_bundle` is the single source of
  truth for what runs and what the recap claims; bundles
  deliberately ignore the range toggle and their notes say so.
- Failure discipline: every op reports through `core.status`,
  failure wordings all contain "failed", `export_failed` /
  `note_single` / `bundle_recap` never claim a failed export
  as exported, and `toasts::classify_status` maps export
  failures to Error and successes to Success toasts.
- Project tools' §31 honesty: the "Current file" line and its
  meta line appear only on an exact recents-name match, so
  they can never name another project's file; Reload's
  success status names the discard outright; the Phase H
  `after_save` gate stopped failed saves planting phantom
  recents.
- `projects::rename_project` refuses to overwrite an existing
  file and `duplicate_project` increments its suffix — the
  library's careful half. (Its save half is not: D7.)

## Defects

### D1 — The audio notes lie about WebM (P1, honesty)

`export::export_webm` never muxes audio — full or range
(`src/export.rs:926`; `export_webm_range`'s own docs:
"VIDEO ONLY, like `export_webm` itself: no audio is muxed —
the WebM export path has no audio support"). The panel copy
says otherwise:

- `RANGE_AUDIO_NOTE` (`export_panel.rs`): "Range video
  exports are silent — audio is only in full-timeline
  MP4/WebM." Full-timeline WebM has no audio either.
- `loop_note`'s "(MP4/WebM range exports are video-only)"
  carries the same implication.
- The WebM row note ("WebM video (VP9) — ./export/
  animation.webm") says nothing about audio, while the MP4
  row's note carefully explains its audio behaviour — so
  the one format that is *always* silent is the one with no
  warning anywhere.

A user who exports a full WebM expecting the project's
sound gets silence after the UI implied the opposite. This
is the exact defect class the panel's conditional audio
note (Phase M) was built to prevent, with the polarity
reversed.

### D2 — New / Open / Reload / Recover discard unsaved work with no confirmation (P1, data loss)

`project_tools::new_project`'s docs state it plainly: the
project "is replaced in place … with **no confirmation
prompt and no undo snapshot**, so unsaved work is dropped
the moment this is pressed". The same instant-swap applies
to every selector-row Open, to Recover, and to Reload —
Reload at least names the discard in its success status,
after it has happened. Meanwhile the *less* destructive
action in this app, deleting one saved file in Home, has a
two-step arm/confirm (`HomePanelState::confirm_delete`,
Phase K §15). The app owns a confirmation idiom and uses
it for the file, but not for the live session. Mission
brief §8 requires confirmations for destructive
operations; this is the largest gap against it in S16's
scope.

### D3 — "Recent exports" misrepresents reality in both directions (P1/P2)

The history lists the top level of `./export/` and `out/`,
newest first, capped at 8 (`recent_exports_cached`,
`HISTORY_LIMIT`).

- **Flooded by sequences:** PNG frames are written
  *directly into `./export/`* as `frame-0001.png` …
  (`export.rs` `export_png_sequence_range`; `editor.rs`
  `export_png` targets `./export` itself, not a subdir).
  One 30-frame PNG export fills all 8 rows with individual
  frame files and pushes every meaningful export out of
  the list.
- **Blind to two formats:** JPEG writes into `out/jpeg/`
  and SVG into `./export/svg/` — subdirectories the scan
  deliberately does not enter — so JPEG and SVG exports
  *never appear in the history at all*, not even as a
  directory entry.

The module docs acknowledge the subdir exclusion as a
design choice; combined with the PNG-in-root fact it
produces a list that is wrong in both directions.

### D4 — Exports block the UI thread; no busy state, no progress, ffmpeg errors swallowed (P2)

Every export handler runs its op synchronously on the UI
thread. `ffmpeg_encode_times` renders every frame to PNG
and then runs ffmpeg to completion before returning;
ffmpeg's stderr is discarded (`Stdio::null`), so the only
error surface is "ffmpeg exited with status …" / "could
not run ffmpeg: …" in the status line afterwards. There is
no pre-flight ffmpeg check: a user without ffmpeg learns
of it only by pressing MP4/WebM and failing. For a long
project the window simply freezes for the whole encode
with zero feedback — indistinguishable from a hang. No
fake progress is proposed (see I10): the honest options
are a real worker-thread execution with a busy state, or
explicitly documenting the freeze. Currently neither
exists.

### D5 — No output settings exist, and the panel never states what it will produce (P2)

The brief asks to improve resolution and output settings;
the audit fact is that there are none to improve:

- Every export renders at `project.stage` size
  (`render_frame_at`) — no scale/resolution control in any
  export fn signature in `export.rs`.
- JPEG quality is hard-coded 90; MP4 is libx264 with
  default rate control; WebM is VP9 at a fixed `-b:v 2M`.
- Destinations and file names are fixed (`animation.mp4`,
  `animation.webm`, `animation.gif`, …) and every export
  **silently overwrites** the previous export of the same
  format.
- The panel shows frame counts (via `effective_text`) but
  never the output dimensions or fps, so a user cannot
  predict the deliverable's resolution from the Export
  workspace at all — resolution is set far away, as a
  stage-size project property.

Smallest honest increments are I5 (state the output) and,
as flagged engine work, real settings later. Inventing
controls the engine cannot honour is out of bounds.

### D6 — The in-panel recap is visually flat and mode-blind (P2)

The `xc-recap` line renders success and failure in the
same `text_secondary` — the toast carries the severity,
the panel's own persistent record does not. And
`note_single` builds the recap from `op.name()` +
`op.dest()` only: after a *range* export the recap claims
the plain full destination with no frame-range or
video-only marker, so the one surface that persists after
the toast fades understates what actually happened (the
transient status line does say it). A failed recap
("MP4 export failed") also drops the reason the status
line had.

### D7 — Named save overwrites by slug with no collision guard (P2)

`EditorCore::save_named_project_to` (`editor.rs:3818`)
writes `./projects/<slug>.json` unconditionally. Two
project names that slug alike — "My Film" and "my-film"
both → `my-film` — silently overwrite each other's files;
the save reports success either way. `projects::save` has
the same unconditional write. The library's rename path
refuses exactly this; the save path does not check at all.

### D8 — Corrupt project files vanish silently (P2)

`projects::scan` (`projects.rs:39`): "Files that cannot be
read or parsed as a `Project` are skipped silently." A
damaged `<name>.json` therefore disappears from Home and
from every count with no error anywhere — indistinguishable
from deletion. The user has no way to learn a save is
broken until they go looking for it. (Fix is shared with
Squad 06: scan's contract + both list surfaces.)

### D9 — Two project lists, unequal capability; the near one is the poor one (P3)

Home (Squad 06) offers stored-name rows with metadata,
search, sort, rename, duplicate, and confirmed delete.
The Project panel in the Animate-side column offers
file-stem labels with an Open button only — no metadata,
no rename/duplicate/delete, list order is the core's file
order rather than newest-first. Its one state cue is also
misleading by design (and documented as such): the accent
on a row's Open button tracks `project_index`, the
palette's *cycle cursor*, which `cycle-project` moves
without opening anything — so a row can look "current"
when it is merely the cursor's position. A user working
in Animate gets the weak list; the capable one is a
workspace switch away.

### D10 — Zero-frame projects "export" successfully (P3)

`effective_text` already knows the truth ("The project
has no frames to export.") but every button stays live,
and the sequence paths return `Ok(0)`: the status then
reads "Exported 0 PNG frames …" — a Success-classified
toast for an export that wrote nothing. The kit has no
disabled state yet (Squad 02/17 dependency); the interim
honest behaviour is a refusal with a Warn status, the
shape every other refusal in the engine uses.

### D11 — Copy and census nits (P3)

- The module header says "the same 15 formats"; the panel
  renders 16 `xc-*` rows (9 media + 7 project formats)
  plus 2 bundle rows.
- The subtitle "Export centre — every format in one
  place" repeats the "Export" title directly above it.
- A partially failed bundle's recap contains " failed",
  so `classify_status` shows an Error toast even when
  most formats exported — acceptable, but worth a pinned
  test so the behaviour is a decision, not an accident.

### D12 — The export panel's rows have never been width-measured (P3, evidence gap)

The panel lives in the right column (`main.rs:698`;
316 pt content budget) and every row pairs a button with a
long note text — the bundle notes run ~150 characters.
The Phase N headless artifact set contains 13 tree dumps
(default, draw-open, frame-3, fx-split-mid,
graph-duplicates, loop-range, menu-open, palette-open,
playing, selected, settings-open, three-d, three-d-stage)
— **no export-open tree**, although the `export-open`
scene exists in `scenes.rs`. The p05 overflow scan
therefore never saw these rows. Whether the notes wrap,
truncate, or push past the card is currently unmeasured,
not known-good.

## Work items (each sized for one coder)

- **I1 (S)** — Fix the WebM audio copy: `RANGE_AUDIO_NOTE`,
  `loop_note`, and the WebM row note state that WebM is
  always video-only; keep the MP4 wording untouched; pin
  with the panel's existing string tests. Files:
  `export_panel.rs` only. Fixes D1.
- **I2 (M)** — Unsaved-work confirmation for New, selector
  Open, Reload, Recover in `project_tools.rs`, reusing
  Home's arm/confirm idiom (arm on first press when the
  session is dirty, confirm line naming the discard).
  Needs one new panel-state field — `State` is
  integrator-owned and the 43-field census moves, so this
  item is integrator-gated. Fixes D2.
- **I3 (M)** — Repair Recent exports: collapse a PNG
  sequence into one row ("frame-*.png · N files") and
  represent the JPEG/SVG sequence directories as single
  rows with their dir mtimes, inside the existing
  cache/scan shape. Files: `export_panel.rs` history half
  + its tests. Fixes D3.
- **I4 (S)** — Recap truth and severity: colour/glyph the
  `xc-recap` line by success/failure (reuse the toast
  severity palette), and make `note_single` mode-aware —
  range exports recap the frame range and video-only
  where true. Files: `export_panel.rs`. Fixes D6.
- **I5 (S)** — Output summary line under the toggle:
  "Output: {frames} frames · {w}×{h} · {fps} fps" driven
  by the same state `effective_text` reads, updating with
  the range toggle. Files: `export_panel.rs`. Addresses
  D5's information half.
- **I6 (M)** — Slug-collision guard on named save: when
  the target file exists and its stored name differs from
  the current project name, refuse with a Warn status
  naming the collision (or arm/confirm per I2's idiom —
  integrator picks one). Files: `editor.rs`
  (`save_named_project_to`), `projects.rs` (`save`).
  Engine-adjacent: integrator approval. Fixes D7.
- **I7 (M)** — Surface unreadable project files:
  `projects::scan` additionally returns the skipped
  `.json` paths; Home and the Project panel each render
  one muted "N unreadable project files" line (names on
  the Home surface). Files: `projects.rs`, `home_panel.rs`
  (S06 coordination), `project_tools.rs`. Fixes D8.
- **I8 (S)** — Zero-frame refusal: all export handlers
  refuse with "Nothing to export — the project has no
  frames." (Warn class) when `frames` is empty, replacing
  the false "Exported 0 …" successes. Files:
  `export_panel.rs` (+ op-level guard optional).
  Fixes D10.
- **I9 (S)** — Add the `export-open` tree + snap probes to
  the headless workflow and run the p05 overflow scan over
  the export panel; fix any row that crosses the card.
  Files: `.github/workflows/headless.yml`, then
  `export_panel.rs` if the scan convicts a row.
  Closes D12.
- **I10 (L, integrator/architecture — not a panel item)**
  — Decide the export-execution model: move encodes onto
  a worker thread with a real busy state and completion
  status (the only honest "progress" available), plus an
  ffmpeg pre-flight check surfaced in the Video section
  when the binary is missing. Until decided, D4 stands as
  a documented limitation in `docs/PHASE_O`-successor
  notes — the freeze must at least stop being silent in
  the docs.

## Cross-squad notes

- **S04 (app bar):** two save systems coexist — app-bar
  Save/Load work on the working file `project.styk.json`;
  the Project panel's "Save project" writes
  `./projects/<slug>.json`. Nothing in either surface
  explains the difference; a beginner cannot tell which
  Save persists a named project. Whoever owns the final
  copy pass should name both destinations at the point of
  use.
- **S06 (Home):** D8 and D9 are shared surfaces; I7 needs
  S06 coordination, and D9's end-state (one list model,
  two presentations) is an integration decision, not two
  independent fixes.
- **S02/S17:** D10's proper fix (a real muted/inert button
  state in the kit) is kit work; I8 is the interim
  handler-level honesty fix and should not wait for it.

## Audit limitations

No build or run was performed (CI-only rule); all claims
are from source. Row-width behaviour in D12 is stated as
unmeasured because it is — no export tree dump exists in
the artifacts on disk. Whether ffmpeg exists on any given
user machine is unknowable from here; D4 is about the
absence of a pre-flight surface, not about any specific
machine.
