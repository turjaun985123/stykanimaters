# Squad 13 Audit — Effects & Particles

Phase 1 audit for the ULTIMATE UI/UX mission. Read-only
source audit of the four squad files against the Phase N
design kit (`src/rune_sec/design.rs`), the mission brief
(§5 D/E/G/H, §7 themes, §8 UX), and each other — the
squad's own panels disagree with one another in several
places, and that internal inconsistency is itself a
finding. No code changed; no local build run (AGENTS.md
CI-only rule).

## Files inspected

| File | Lines | Role |
|---|---|---|
| `src/rune_sec/fx_browser.rs` | 1,223 | Kind browser (27 kinds, 5 categories), filter, Recent row, presets |
| `src/rune_sec/fx_clips.rs` | 2,496 | Clip lane rows, timeline strip canvas, clip inspector |
| `src/rune_sec/particles_panel.rs` | 1,412 | Particle-event list + detail block |
| `src/rune_sec/stage_fx.rs` | 800 | Stage-side FX/weather geometry + selection gizmo (the squad's only on-stage UI) |

Cross-checked: `design.rs` kit builders actually used
per file, `fx_edit.rs` preset API surface, `fx_ops.rs`
op surface, chrome_view `danger_btn` family existence.

## What is already good (do not redo)

- All four files are on the kit card root
  (`design::panel_col`), kit `panel_title` / `divider` /
  `section_label`, kit 24 pt `swatch` (fx_clips Phase N
  retired its local 18 pt construction).
- Every control is real: kind adds, preset apply/save,
  move/resize/split/duplicate/delete, hide toggle, kind
  cycling, re-anchor, particle add/duplicate/delete,
  anchor and nudge ops all call real `EditorCore` ops.
  No fake buttons anywhere in the squad.
- `particles_panel` empty state is the kit
  `design::empty_state` with a glyph — the squad's
  reference implementation for S13-3 below.
- The strip (fx_clips Phase G–M) is genuinely good:
  lane-packed bars in clip colours, selected bar
  brightened + outlined, hidden bars dimmed, press
  selects, press on empty seeks. The inspector Hide/Show
  toggle names the action it will take and keeps one key.
- Kind buttons carry Phase K usage counts
  ("Sparkle · 3") — in-use kinds are discoverable
  without leaving the browser.

## Defects (prioritised)

### D1 — Hidden clips are not identifiable in the clips lane (HIGH)

`ClipRowData` (fx_clips.rs:272) has **no `hidden`
field**, so a lane row renders identically whether its
clip is hidden or live: same colour dot (undimmed), same
text colours, no eye-off marker. The hidden state is
visible only as strip-bar dimming (`bar_alpha`) and as
a "· hidden" suffix in the inspector's readout for the
*selected* clip. A user who hid a clip yesterday cannot
tell which of twelve rows is suppressed without
selecting each one. Mission §5C/§8: state must be
readable at the list level.

### D2 — The FX inspector carries three overlapping control groups for the same two properties (HIGH)

For the selected clip, **Start** is driven by: the
Start stepper (−/+), the Earlier/Later buttons, *and*
the nudge row (−10 / chevron / chevron / +10) — three
groups, same op (`fx_move_clip`). **Duration** is
driven by: the Duration stepper, *and* the nudge row's
±0.1 s pair (same `fx_resize_clip`, different unit —
frames in one group, seconds in the other, with no
unit signalling beyond the button labels). Add the
To playhead / End at playhead pair and one property
has five controls. This is exactly the mission §8
"no confusing duplicate controls" failure, and it
makes the inspector ~9 rows tall before colours.

### D3 — The FX browser's empty lines are still bare text (HIGH — mission seed item)

fx_browser.rs:621 renders `No presets saved yet` as a
raw `text(...)` in `text_secondary`, and the
no-filter-match line ("No effects match the filter")
the same way. The module doc admits it: "the two muted
empty lines stay as they are — no glyph is assigned to
either state in any plan." The sibling particles panel
already uses kit `empty_state` with a glyph. Mission
§5G requires icon + message (+ next action where
supported) for every empty state. The fx lane's
"No FX clips in this project." (fx_clips.rs) is the
same bare-text pattern.

### D4 — Selection/active styling diverges inside the squad (MEDIUM)

Three different "selected row" treatments in one
workspace:

- fx lane rows: hand-rolled row, fill `t.control_track`
  when selected, plus an 8 pt accent dot
  (`tok::accent(0, true)`).
- particle rows: kit `design::list_row` (kit
  `selected_fill`, accent @ 0.16) — *plus* the same 8 pt
  accent dot on top, double-signalling.
- strip bars: brighten-toward-white + outline in
  `STRIP_ACCENT`.

`STRIP_ACCENT` (fx_clips.rs:432) and `GIZMO_COLOR`
(stage_fx.rs:253) are the **hard-coded literal
`0x6D8CFF`** — the legacy 2.0 accent — not the theme
token, and identical in dark and light themes. Mission
§3.2/§7: colors centralised in tokens, both themes
attended. (This is the documented legacy-accent split;
the mission is the occasion to close it deliberately,
not to scatter a second literal — the brief's proposed
`#6C8CFF` differs from both.)

### D5 — Kind identity and in-use state are weak in the browser (MEDIUM)

- Every kind button leads with its **category's**
  glyph, so all nine Particles kinds show the identical
  `shapes` glyph and all six Magic kinds the identical
  wand. The glyph carries zero per-kind information; 27
  buttons reduce to 5 repeated icons.
- A kind button's only "active/in-use" signal is the
  counted label suffix. Buttons pass `active = false`
  unconditionally — a kind the playhead is sitting in
  the middle of right now looks the same as an unused
  one, count aside.
- Add buttons are live even when they cannot succeed
  (no joint selected → the op refuses and only the
  status line explains). The panel itself gives no
  hint that a joint must be selected first; a first
  user clicks Burst, sees nothing happen on stage,
  and must notice a status line elsewhere on screen.
  (Documented as inherited GPUI behaviour — the
  mission is allowed to do better than the GPUI app.)

### D6 — Clip parameters angle / length / seed are displayed but not editable anywhere (MEDIUM)

The inspector footer prints `seed · angle · length`
read-only. Angle and length materially change how
Beam/Aura/etc. render, `fx_ops.rs` exposes **no**
set-angle/set-length op, and no other panel edits
them — the values are whatever creation or a preset
baked in. Configuration completeness (mission squad
13 charter: "improve effect configuration") requires
either real steppers (needs the small backend op) or
removing the impression that the footer is a control
surface.

### D7 — Preset lifecycle is incomplete (MEDIUM)

- Save exists only as "Save last clip as preset" in
  the browser (plus per-clip save on clip rows), with
  an auto-generated `<kind>-<n>` name and no naming
  UI — users cannot tell presets apart later except
  by filename.
- Apply buttons show the raw preset name as filename
  text, in plain `btn` rows chunked 3-per-row like
  kinds, with no glyph and no distinction from kind
  buttons.
- **No delete or rename exists at all** — verified:
  `fx_edit.rs` exposes only `save_clip_preset` /
  `load_clip_preset` / `list_clip_presets`. A mistyped
  or stale preset can never be removed from the app.

### D8 — Destructive actions: unstyled, unconfirmed, inconsistent (MEDIUM)

- fx inspector delete: icon-only `trash-can`
  (`icon_only_btn`), no danger styling.
- particles delete: labelled `icon_btn` "Delete event",
  also no danger styling.
- Neither uses the Phase N `danger_btn` /
  `danger_icon_btn` family that exists in chrome_view,
  and neither asks for confirmation. Undo snapshots
  exist for both ops, so the proportionate fix is
  danger styling + toast/undo reliance (mission §8:
  confirmations for destructive operations — a
  confirm step is the other honest option; one of the
  two must be chosen, currently neither signal exists).

### D9 — On-stage active-effect indication is a single unlabelled gizmo (MEDIUM)

stage_fx's gizmo (crosshair + dot ring) is drawn for
**one** selected event/clip only, always in the same
hard-coded accent, with no label, no kind/colour
coding, and **no difference for a hidden clip** (its
gizmo draws at full alpha by design). Consequences:
two clips anchored to nearby joints are
indistinguishable markers; a hidden clip's marker
looks live; and outside selection there is no stage
indication of which clips are active at the playhead
at all — the strip is the only place "what fires
when" is visible.

### D10 — Particles panel polish debt (LOW, several small tells)

- `panel_title(None, "Particles", …)` — the squad's
  only panel header with **no glyph** (fx panels use
  `wand-magic-sparkles`).
- The add button is `icon_btn("star", "+ Event")` — a
  leading "+" in the label *and* a glyph, and the
  label style ("+ Event") matches no other add button
  in the shell.
- The selected event's position is printed **twice**:
  fused into the detail summary line and again as the
  keyed `particles-pos` readout under the nudge pad.
- The nudge pad uses plain text arrows (`← ↑ ↓ →`)
  via `btn` while the rest of the shell (including
  this squad's fx nudge/kind steppers) uses icon
  chevrons.
- Three different actions — To playhead, Anchor to
  joint, Anchor to figure — all carry the same
  `crosshairs` glyph; the fx inspector does the same
  for To playhead / End at playhead. Glyph reuse this
  dense stops meaning anything (Squad 17 territory,
  flagged here because the instances are ours).
- Anchor-to-joint is present-or-absent by joint
  selection (documented presence pattern): the detail
  block's row composition changes as the user selects
  joints on stage — layout shift with no placeholder.

### D11 — Smaller items (LOW)

- Browser filter field has no clear (×) affordance;
  clearing means select-all + delete in the field.
- Recent row buttons are plain text `btn` while grid
  buttons are `icon_btn` — same action, two button
  languages in one panel.
- Inspector kind stepper cycles 27 kinds one chevron
  press at a time with no direct picker; cycling from
  Burst to Wave is up to 26 presses.
- fx inspector header is a raw `type_::TITLE` text
  line, not a kit header treatment, and the lane +
  inspector share one card with no divider between
  them — the inspector reads as more lane, not as a
  distinct editing surface.
- fx lane row colour dot does not reflect `color2`
  (gradient end) at all; gradient clips are
  indistinguishable from solid ones in the list.
- Width risk unverified: kind/preset rows chunk 3
  icon+label buttons per row; with counted labels
  ("Shock ring · 12") these are the widest buttons in
  the squad. Not measured here — route through the
  p05 overflow-scan method before claiming either way.

## Work items (each sized for ONE coder)

| # | Item | Files | Acceptance | Verification |
|---|---|---|---|---|
| S13-1 | Add `hidden` to `ClipRowData`; lane row dims dot + text and shows an `eye-slash` marker for hidden clips | fx_clips.rs | Hidden row visibly distinct at list level; keys/handlers unchanged | Runtime row-data test + headless tree on a scene with a hidden clip |
| S13-2 | Consolidate inspector Start/Duration controls: keep stepper + nudge row, retire Earlier/Later (or stepper — one decision, documented); unify duration unit display | fx_clips.rs | One control group per property; all keys it removes named in the commit (census delta explained, not lost) | Gate census + panel tests |
| S13-3 | Kit `empty_state` for "No presets saved yet" (glyph `floppy-disk`), "No effects match the filter" (`magnifying-glass`), and fx lane "No FX clips…" (`wand-magic-sparkles`) | fx_browser.rs, fx_clips.rs | All three lines glyph-led kit empty states; text unchanged | Tree check: lines render via empty_state construction |
| S13-4 | Move fx lane rows onto kit `design::list_row` (as particles rows already are); drop the duplicate accent dot in particles rows | fx_clips.rs, particles_panel.rs | One selected-row treatment across the squad | Headless snaps of both panels, selected row compared |
| S13-5 | Replace `STRIP_ACCENT` / `GIZMO_COLOR` literals with the theme accent token (one decision with Squad 02 on the legacy-split) | fx_clips.rs, stage_fx.rs | No `0x6D8CFF` literal left in squad files; both themes checked | Grep + light-theme snap |
| S13-6 | Browser affordance pass: in-panel hint line when no joint is selected ("Select a joint, then pick an effect"), filter clear button, Recent row onto `icon_btn` family | fx_browser.rs | Refused-add case is explained *in the panel*; keys added are new, none renamed | Scene/tree check for hint visibility by selection state |
| S13-7 | Angle/length steppers in the inspector + the two small `EditorCore` ops in fx_ops (snapshot + field write, fx_edit pattern) | fx_ops.rs, fx_clips.rs | Angle/length editable, undoable, status-fed; seed stays read-only (it is a hash seed, not a look control) | Op tests + inspector tests |
| S13-8 | Preset management: `fx_edit::delete_clip_preset` op + per-preset delete affordance in the browser; preset buttons get the `floppy-disk` glyph family treatment | fx_edit.rs, editor.rs, fx_browser.rs | A preset can be deleted from the UI; cache invalidates via dir mtime as saves do | fx_edit round-trip test (save→list→delete→list) |
| S13-9 | Danger styling for both delete buttons via chrome_view danger family; choose and document confirm-vs-undo policy once (with Squad 18) | fx_clips.rs, particles_panel.rs | Deletes read as destructive at a glance | Snap check |
| S13-10 | Gizmo differentiation: hidden clip's gizmo dimmed like its strip bar; gizmo tinted by the clip's own colour mixed toward accent | stage_fx.rs (+ stage_view consumer if needed) | Hidden vs live markers distinguishable on stage | stage_fx geometry tests (alpha/colour assertions) |
| S13-11 | Particles panel pass: header glyph (`star`), add-button label to "Event" (glyph carries the add affordance) or shell-standard wording, nudge pad onto chevron icon buttons, drop the duplicated unkeyed position text from the summary line | particles_panel.rs | Panel matches squad/shell idioms; `particles-pos` readout remains the one position display | Panel tests + snap |
| S13-12 | Kind picker for the inspector: pressing the kind label opens the existing browser-filter pattern (or a compact kind menu reusing `FX_KINDS`) instead of 27 chevron steps; chevrons stay for ±1 | fx_clips.rs (+ fx_browser table reuse) | Any kind reachable in ≤2 interactions | Interaction test on the picker path |
| S13-13 | Run the p05 overflow-scan over the Effects workspace scenes; fix or explicitly clear the kind/preset 3-per-row chunking at the 316 pt budget | fx_browser.rs (if needed) | Scan output attached to the item; no row crosses its card edge | overflow_scan.py over mission headless dumps |

## Existing behaviour preserved (constraints on every item)

- All element keys verbatim (`fxclip-*`, `fx-clip-{i}`,
  `fx-strip`, `fx-preset-apply-*`, `particle-row-{i}`,
  …). Any key an item retires (S13-2) must be named
  and census-accounted, never silently dropped.
- Handlers and ops unchanged except where an item
  explicitly adds one (S13-7, S13-8).
- The preset-name cache contract (dir-mtime keyed)
  must survive S13-8: deletes change dir mtime, so
  invalidation is automatic — the item's test proves
  it rather than assuming it.
- Strip geometry, lane packing, seek behaviour and
  the hidden-bar alpha rule are settled (Phases G–M);
  S13 items touch presentation, not geometry.
- The gizmo's no-frame-gating and live-anchor choices
  (stage_fx module docs) are deliberate; S13-10
  changes tint/alpha only.

## Known limitations of this audit

- Source-only: no headless snaps were re-rendered for
  this squad (Phase 1 audit rule — Squad 19 owns pixel
  verification). Width/overflow statements are flagged
  as unverified (S13-13) rather than asserted.
- Light theme was assessed by token usage in source
  (the `0x6D8CFF` literals are theme-independent by
  construction), not by viewing a light-theme build.
- Engine-side truth was checked for the two claims
  that needed it (preset API has no delete; fx_ops
  has no angle/length setter) and not beyond.
