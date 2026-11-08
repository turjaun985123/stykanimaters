# Squad 05 Audit — Workspaces

**Scope:** the eight workspaces (Animate / Draw / Graph / Effects /
Audio / 3D / Export / Home), their panel gating, switching
affordances, active indicator, blurbs, and `Alt+1..8`.
**Sources read:** `src/rune_sec/workspaces.rs` (full),
`src/main.rs` (`view()` mount code lines 565–817, `on_key`
lines 466–558), `src/chrome_view.rs` (`btn` family),
`src/rune_sec/figure_add.rs`, `src/rune_sec/scenes.rs`,
`src/rune_sec/snap_tests.rs`, `src/rune_sec/help_panel.rs`,
`src/rune_sec/command_palette.rs`, `src/rune_sec/status_strip.rs`,
`src/rune_sec/menus.rs`.
**Method:** source audit only; no builds (AGENTS.md CI-only rule).
Where a claim needs a rendered measurement I say so instead of
asserting it.

---

## 1. How gating actually works (verified)

- `workspaces.rs` owns the model: a `Workspace` enum (8 variants),
  a `Section` enum (29 gated panels), one section list per
  workspace, and `section_visible(ws, section)` as the single
  gating question.
- `main.rs::view()` asks `vis(sec)` for **every** gated panel in
  both side columns. No gated panel is mounted unconditionally —
  the table and the view agree (checked panel by panel).
- Switching has three entry points, all funnelling into
  `select_workspace` (sets state + persists `LayoutPrefs.workspace`):
  the chip row, `Alt+1..8` in `on_key`, and the command palette's
  `go-animate` … `go-home` commands. Menus offer no workspace
  entries.
- Not gated, by documented design: app bar, menu bar, stage,
  camera overlay strip, transport, timeline, status strip,
  toasts, settings popover, help panel — and the **figure-add
  panel** (see defect D1).
- Stage behaviour is workspace-aware in exactly one place: the
  3D figure draws on the main stage only when
  `workspace == Build3D` and a fig3 session is live. Correct,
  no leak.

### Mounted panels per workspace (as `view()` actually orders them)

| Workspace | Left column | Right column (mount order) | Section count |
|---|---|---|---|
| Animate | Project, Figures, figure-add†, Library, Layers | Seg insp., Fig insp., Style, Pose, Keyframe, Playback, FrameOps, Camera, Onion, Clips, Stats | 16 |
| Draw | Figures, figure-add† | Draw tools, Stroke insp., Text | 4 |
| Graph | Figures, figure-add† | Graph, Keyframe, Easing | 4 |
| Effects | Figures, figure-add† | FX browser, FX clips, Particles, Scene | 5 |
| Audio | Figures, figure-add† | Playback, Audio, Waveform | 4 |
| 3D | Figures, figure-add† | 3D, Camera | 3 |
| Export | Project, figure-add† | Export, Stats | 3 |
| Home | Home, Project, figure-add† | Stats | 3 |

† figure-add is **not** a gated section — it mounts in every
workspace (D1). Settings popover and (when open) the help panel
also mount at the top of the right column in every workspace.
The centre column (stage, camera overlay strip, transport,
timeline, status strip) is identical in all eight workspaces.

---

## 2. Cross-cutting findings

### 2.1 Active indicator — good mechanism, plain presentation

- The active chip is the shared `chrome_view::btn` with
  `active = (s.workspace == ws)`: accent fill, white label,
  hover/press fills, `press_scale(0.97)`. Exactly one chip can
  be active (single `State.workspace` field), the choice
  persists across runs, and chips / help rows / palette
  commands all describe the same eight destinations in the
  same order. This part is genuinely solid.
- Weaknesses are presentational:
  - Chips are **text-only**. The shell has a curated 163-icon
    system (Phase H) used across ~30 panels; the primary
    navigation of the app uses none of it.
  - The shortcut hint is a plain-text suffix baked into the
    label (`"Animate · Alt+1"`). The kit already has a `kbd`
    chip (used in the command palette); the workspace row
    doesn't use it, so the hint reads as part of the name and
    makes every chip ~45 pt wider. Eight suffixed chips form
    the widest single row in the chrome; at the 1280 pt
    minimum window this row is the first candidate for
    crowding. (Estimate from label lengths — needs a tree
    dump at 1280 to confirm or clear; no 1280 workspace
    measurement exists yet.)
  - Inactive chips rest on `t.card` against the `t.bg` root —
    a low-contrast outline-free pill. Acceptable, but combined
    with text-only labels the row reads as a strip of near-
    identical grey buttons with one blue one; the *names*
    carry all the differentiation.
  - The status strip has **no workspace segment** — the chip
    row is the only place the current workspace is named.
    Fine while the row is always visible (it is), noted for
    completeness.

### 2.2 Alt+1..8 — correct, with two rough edges

- Bindings match chip order and the help panel's eight rows
  exactly (`help_panel.rs` lines 280–287, test-pinned).
  `on_key` resolves Alt+digit before the general key match.
- Rough edge 1: the palette key handler and the
  `text_focused` early-return run **before** the Alt branch,
  so Alt+1..8 silently do nothing while a text field is
  focused. Standard behaviour, but silent — no status hint.
- Rough edge 2: the digit is taken from the produced text
  character (`chars().next().to_digit(10)`). On layouts where
  Alt+digit produces a symbol rather than a digit character,
  the switch silently fails. Unverified against Rune's key
  events; logged as investigation item I7, not asserted.

### 2.3 Blurbes — disciplined, two content mismatches

All eight blurbs are one sentence, unique, end with a period,
and are pinned verbatim by tests — good discipline. Content
issues:

- **Animate's blurb under-reports its own workspace**: "…with
  the inspectors, camera, playback and frame tools" omits
  Clips (movieclips) and Stats, both mounted there.
- **3D's blurb hides the merge**: the GPUI Build workspace was
  folded into this one, but the chip says only "3D" and the
  blurb only "Pose the 3D figure and frame it with the
  camera." A user looking for figure *building* finds no
  "Build" anywhere; worse, a persisted GPUI-era
  `LayoutPrefs.workspace = "Build"` silently falls back to
  Animate (`from_name` fallback) with no notice.
- Draw / Graph / Audio blurbs don't mention that the Figures
  panel rides along (harmless, but the pattern of "blurb =
  exact inventory" set by Animate's blurb is not kept).

### 2.4 Localisation gap

App-bar and transport labels resolve through `i18n_wire::tr`
(28 languages). Workspace chip labels and blurbs are hard-coded
English literals in `workspaces.rs` — the primary navigation
is the one chrome surface that never translates.

### 2.5 Visual-QA coverage gap

Of the 15 headless scenes, six exercise a non-default
workspace (draw-open, export-open, fx-selected, fx-split-mid,
graph-duplicates, three-d / three-d-stage). **Audio and Home
have no scene and no snapshot at all** — gating unit tests
reference them (`snap_tests.rs` ~3189/3207), but nothing has
ever rendered them. Any layout defect in those two workspaces
is currently invisible to QA.

---

## 3. Per-workspace verdicts

### Animate — GOOD (the reference workspace)
16 sections, everything an animator needs in one place, mount
order deliberate (inspectors → pose/keyframe → playback/frames
→ camera/onion → clips → stats). Only costs: the longest
right-column scroll of any workspace, and a blurb that omits
two of its panels (§2.3). No leaks in or out.

### Draw — UNDER-EQUIPPED for its task
Right column is exactly the three drawing panels — focused —
but drawing happens *on frames, layers and a scene*, and Draw
mounts none of: **Layers** (can't toggle layer visibility
while drawing), **Scene** (background/scene settings live in
Animate/Effects only), **Playback**, **FrameOps** (new frame
while drawing = keyboard `N` or the timeline only). The
workspace feels like a tool palette detached from its context.

### Graph — GOOD, one gap
Graph + Easing + Keyframe tools + Figures is the right set and
Graph correctly leads the column (Phase N ordering). Gap:
no **Playback** panel — checking a curve against playback
means the transport row only; the Playback panel's loop/speed
controls are Animate/Audio-only. Minor.

### Effects — GOOD
FX browser, FX clips, Particles, Scene, Figures — coherent.
Scene sits last in mount order (deliberate Phase N demotion so
it doesn't fill the first screen); in Effects specifically
that buries the scene controls under three FX panels. Minor.

### Audio — ADEQUATE, thin
Audio + Waveform + Playback is the right core. Two thin
spots: no **FrameOps** (aligning clips to frames is the
workspace's whole job, yet frame tools are absent), and no
**Clips** panel — movieclips exist only in Animate, so audio
work against movieclip timing relies on the timeline clips
lane alone. Also one of the two workspaces with zero visual
QA coverage (§2.5).

### 3D — LEAN AND CORRECT, BADLY NAMED
Three sections, fig3 on the main stage, camera alongside —
the tightest workspace and functionally right. Its problem is
identity, not content: the "Build" half of its brief is
invisible in the label, the blurb, and the persisted-name
fallback (§2.3).

### Export — COHERENT, with the leak
Export centre + Stats + Project tools is a sensible, calm
set. It suffers D1 like Home does: the figure-add panel sits
in its left column (see below), an authoring control in a
deliverable-focused workspace.

### Home — WEAKEST (see §4)
A workspace in name only: the centre still shows stage +
transport + timeline + camera strip, the right column holds a
lone Stats panel, and the left column carries the orphaned
figure-add panel (D1). Nothing about the shell says "project
management"; it reads as Animate with most panels removed.

---

## 4. Defects (ranked)

- **D1 — figure-add leaks into Home and Export (top defect).**
  `main.rs::view()` mounts `figure_add_panel` unconditionally
  in the left column. In Animate/Draw/Graph/Effects/Audio/3D
  it sits under the Figures panel it belongs to. In **Home**
  and **Export**, `Section::Figures` is gated OFF but
  figure-add still mounts — a "+ Add figure" creation panel
  with presets, orphaned from its parent panel, inside the
  project-management and export workspaces. This is exactly
  the "irrelevant control leaks into a task workspace" class
  the mission calls out, and it contradicts the module's own
  framing (figure-add is "part of the Figures area's chrome" —
  chrome for a panel that isn't there).
- **D2 — Home workspace has no identity.** Centre column and
  furniture are byte-identical to every other workspace;
  right column is Stats + the always-on settings/help. The
  "entrance to the studio" is a left column of three panels.
- **D3 — Draw missing its working context** (Layers, Scene;
  secondarily Playback/FrameOps). See verdict above.
- **D4 — "Build" discoverability loss.** Chip label, blurb
  and prefs fallback all erase Build; a legacy `"Build"`
  prefs value silently lands the user in Animate.
- **D5 — Audio + Home have never been rendered** by any
  scene or snapshot (§2.5).
- **D6 — Workspace row is untranslated** while neighbouring
  chrome localises (§2.4).
- **D7 — Shortcut suffix inflates the chip row**; plain-text
  suffix instead of the kit's `kbd` chip; 1280-width fit
  unverified (§2.1).
- **D8 — Camera overlay strip in Home/Export.** The centre
  camera strip mounts in all workspaces; in Home and Export
  camera framing is not part of the task. (Permanent-furniture
  design decision — flagged, not presumed wrong; needs an
  owner call. Lower than D1 because it is at least a
  considered, documented choice.)
- **D9 — Blurb/inventory mismatches** (Animate omits Clips
  and Stats). Copy-level, test-pinned, cheap to fix.

---

## 5. Improvement items (each sized for one coder)

- **I1 (fixes D1)** — Gate figure-add on the Figures section.
  Files: `src/main.rs` only. Wrap the `figure_add_panel`
  mount in `if vis(Sec::Figures)`. Acceptance: Home and
  Export left columns render without `figure-add`; the six
  figure workspaces unchanged; add/extend a snap test
  asserting `figure-add` absent in Home and Export trees.
  Keys/handlers untouched.
- **I2 (fixes D5)** — Add `audio-open` and `home-open`
  scenes. Files: `src/rune_sec/scenes.rs`,
  `src/rune_sec/snap_tests.rs`. Mirror `export-open`'s
  construction (`select_workspace` into Audio / Home on the
  default project). Acceptance: both scenes render headless,
  snap tests pin their key panels (waveform-card / home
  panel), bringing workspace scene coverage to 8/8.
- **I3 (fixes D3)** — Extend Draw's section list with Layers
  and Scene. Files: `src/rune_sec/workspaces.rs`
  (`DRAW_SECTIONS` + the gating spot-check test). Acceptance:
  updated `gating_spot_checks`; Draw left column gains
  Layers under Figures; Scene mounts in Draw's right column.
  (Playback/FrameOps for Draw deliberately excluded — one
  question per change; revisit after I3 lands.)
- **I4 (fixes D9, part of D4)** — Blurb corrections.
  Files: `src/rune_sec/workspaces.rs` (`blurb()` +
  `blurb_strings_are_pinned`). Animate's blurb gains clips;
  3D's blurb names figure building alongside posing the 3D
  figure. Acceptance: pinned-string test updated in the same
  commit; one sentence each, ending in a period (existing
  uniqueness test still green).
- **I5 (fixes D7, part of §2.1)** — `kbd`-style shortcut
  suffix for chips. Files: `src/rune_sec/workspaces.rs`
  (`workspace_row`), possibly `src/rune_sec/design.rs` if a
  shared chip-with-kbd builder belongs in the kit (Squad 02
  to own that call). Keep `label()`/keys/persistence
  byte-identical; the suffix becomes a distinct muted
  element rather than label text. Acceptance: chip keys
  `ws-*` unchanged; a 1280-wide tree dump shows the full row
  inside the root width (produces the missing measurement
  from §2.1 either way).
- **I6 (part of §2.1)** — Per-workspace chip icons. Files:
  `src/rune_sec/workspaces.rs` (`workspace_row`), icon table
  additions only if a needed glyph is missing from the
  curated set (verify against `icon_data.rs` first; e.g. a
  play/figure glyph for Animate, pencil for Draw, curve for
  Graph, sparkles for Effects, waveform for Audio, cube for
  3D, export arrow for Export, folder/house for Home).
  Acceptance: icon+label chips, active state unchanged
  (accent fill), no new icon style introduced. Coordinate
  with Squad 17 (iconography) before landing.
- **I7 (investigation, §2.2)** — Alt+digit layout probe.
  Files: none initially. Determine from Rune's key-event
  source (`~/workspace/rune-src`) what `Key::Text` carries
  for Alt+digit on non-US layouts and for numpad digits; if
  either can't reach `to_digit`, file the concrete fix
  (key-code-based match) as a follow-up item with the
  evidence attached. No code change in this item.
- **I8 (fixes D2, design-sized)** — Give Home a centre.
  Files: `src/main.rs` (centre column composition),
  `src/rune_sec/home_panel.rs` if the panel grows a wide
  form. Proposal for integrator approval, not a unilateral
  change: in Home only, the centre column leads with the
  project library (recent projects, recovery state) and the
  stage shrinks to a preview role or yields entirely.
  Acceptance: agreed design note first; Animate's centre
  untouched; Home scene (I2) updated to pin the new layout.
- **I9 (fixes D6)** — Localise the workspace row. Files:
  `src/rune_sec/workspaces.rs`, the i18n table source the
  wire reads. Chip labels and blurbs resolve through `tr()`
  like the app bar's, with English as the source strings.
  Acceptance: switching language re-labels the chips; the
  persisted workspace name stays the English `name()`
  (persistence must NOT localise — pin with a test).
- **I10 (owner call, D8)** — Decide the camera strip's
  workspace scope. No files until decided. Options: (a) keep
  everywhere (status quo, document the reason in
  `workspaces.rs`'s not-gated list); (b) gate it with the
  Camera section. Recommendation: (b) — it is a camera
  control, and Camera is already gated; Home/Export lose
  nothing they use.

---

## 6. What must not change (preserved behaviour)

- The eight workspace names, labels, keys (`ws-*`), chip
  order, and persisted strings — including `"3D"` for
  Build3D — are pinned by tests and by on-disk prefs.
- `section_visible` remains the single gating question;
  improvements above add sections to lists, never a second
  gating mechanism.
- Alt+1..8 numbering follows chip position; any chip-order
  change is a breaking change to three surfaces at once
  (chips, help, keys) and is out of scope.
- Animate stays the richest workspace and the default;
  `from_name`'s Animate fallback stays (I4 only fixes the
  *silence* around the Build case if the integrator wants a
  status note — optional, not required).

## 7. Squad 05 summary

**Worst workspace: Home** — gating exists but the workspace
has no identity of its own (D2), hosts the figure-add leak
(D1), and has never been rendered by QA (D5).
**Top defect: D1** — the ungated figure-add panel mounting in
Home and Export, orphaned from the Figures panel it belongs
to. One-line-class fix (I1), verified by tree assertions.
