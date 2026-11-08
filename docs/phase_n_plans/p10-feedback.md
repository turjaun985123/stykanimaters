# Phase N — P10: Feedback spec (toasts, status, confirmations, empty & error states)

Planning agent P10. Scope: how the shell *talks back* —
toast cards, the status strip, destructive-action
confirmations, and every place a panel currently answers
with a bare text row. Sources studied: `toasts.rs`
(705 lines), `status_strip.rs` (1,740), the two-step
delete in `home_panel.rs`, `menus.rs` scrim/dropdown,
`export_panel.rs` recap + history, and the status→toast
path in `main.rs` `poll`. Charter items in force: §9
(menus/popovers), §10 (empty states), §12 (status
strip), §15 (app-bar dirty dot, adjacent). No code in
this document — it is the work list for the F-wave
toasts/status agents and the A-wave panel agents.

## 0. The one big finding

**Severity exists and is never used.** `toasts.rs`
defines four severities with colours and glyphs
(`Severity::color` toasts.rs:118, `icon` :137,
`icon_color` :152), but the shell's only push site
hardcodes Info:

- `src/main.rs:327-331` — `poll`: on any
  `core.status` change, `s.toasts.push(status,
  Severity::Info, now)`. Every failure, refusal and
  success in the app arrives as a blue info toast.

So today the feedback system has exactly one voice.
Most of this spec is giving it the other three —
**without changing a single status string** (charter:
handlers and strings move verbatim; classification
reads the strings, it does not rewrite them).

## 1. Toast anatomy upgrade — `src/rune_sec/toasts.rs`

Current card (`toast_row`, toasts.rs:371-413): row,
key `toast-{index}`, w/max_w 320, padding 12, gap 10,
radius 8, fill `t.card`, border 1 `t.card_border`,
opacity = `fade(now)`; children: severity glyph
(`icon_el_rgb`, 13 pt, tinted `icon_color()`), text
LABEL / `t.text`, `wrap_anywhere`, grow 1; optional
Undo `icon_btn` (`toast-undo`, newest live toast only,
when `undo_available`); trailing dismiss
`icon_only_btn` (`toast-dismiss-{index}`, "xmark",
26 pt). Stack (`toasts_view`, toasts.rs:331-368):
col key `toasts`, pin right 32 / bottom 48, max_w 320,
gap 12; Clear-all `icon_btn` (`toast-clear-all`,
offered at ≥2 live, `CLEAR_ALL_MIN` :95). Model rules
that **do not change**: `MAX_TOASTS` 4 (:90),
`TOAST_TTL` 5.0 s (:93), fade over the last 1.0 s
(`FADE_OUT` :98, `Toast::fade` :202), consecutive
duplicate refresh, oldest-drop at cap.

### 1.1 Target anatomy (per card, left → right)

| Part | Today | Target |
|---|---|---|
| Severity bar | none — severity is only the glyph's tint | **3 pt leading bar**, full card height, fill `Severity::color()`, radius 1.5. First child of the card row. |
| Card shell | radius 8, padding 12, gap 10 | radius **12** (LG — toasts are floating cards, charter §9 family), padding 12 (S3), gap **8** (S2) |
| Glyph | 13 pt FA glyph, severity tint | unchanged (name, size, tint all stay) |
| Text | LABEL, `t.text`, wrap-anywhere, grow | unchanged |
| Action slot | Undo `icon_btn` inline when newest + undoable | unchanged behaviour and key; it stays the *only* action — Phase N invents no new toast actions |
| Dismiss | `icon_only_btn` xmark, 26 pt | unchanged key/size; glyph colour follows `icon_btn` muted convention (already does) |

Severity-bar implementation note for the builder:
the card is a `row()` with `items_center` today;
the bar needs cross-axis stretch, so the card row
drops `items_center` for the bar's sake and the
glyph/text/action/dismiss cluster keeps its own
centring (wrap the existing content in an inner
`row().center_y()`), **or** — fallback if stretch
fights the El API — the bar becomes a left border
effect: card `border(1.0, severity.color at 0.35
alpha)` plus the existing tinted glyph. Builder
picks whichever the pinned Rune API supports
cleanly and reports the choice; the *visual
contract* is "severity is visible at the card's
leading edge before you read the glyph". Card keys
(`toast-{index}`) and both button keys are
byte-identical either way.

### 1.2 Severity classification (new, toasts.rs)

Add one pure function next to `Severity`:

```rust
pub fn classify_status(status: &str) -> Severity
```

Rules, evaluated in this order, derived from real
status strings in the tree (grep-verified
2026-11-08):

1. **Error** — contains `" failed"` (the engine's
   failure shape: `"Audio fade failed: …"`,
   `"Audio compress failed: output would overwrite
   the source."`, export/DSP failures follow it).
2. **Warn** — starts with `"No "` or `"Nothing "`
   (refusals: the op did not happen, nothing is
   broken — `"No FX clip selected to split."`,
   `"No camera key on this frame."`, `"No saved
   file to reload."`).
3. **Success** — starts with `"Exported "` or
   `"Saved "` (`"Exported 5 PNG frames to
   ./export"`, `"Exported out/animation.apng."`).
4. **Info** — everything else, including
   `"Undone."` / `"Redone."` (neutral facts).

Wiring (integrator, `main.rs:330-331`): the push
site passes `classify_status(&s.last_status)`
instead of the hardcoded `Severity::Info`. That is
the only main.rs line this spec asks for, and it is
integrator-owned as in every prior phase.

Tests the F-wave agent adds (unit, in toasts.rs):
one real string per rule from the table above, the
ordering traps (a `"No … failed"`-shaped string is
Error, not Warn — rule order matters; an
`"Exported …"` string never matches `" failed"`),
empty string → Info (poll never pushes empty, but
the function must not panic). Existing model tests
(cap, dedupe, TTL boundary, dismiss, clear-all)
must pass untouched — classification is additive.

### 1.3 Stack treatment

- Gap 12 → **8** (S2) so the stack reads as one
  component.
- Clear-all stays a plain `icon_btn` *below* the
  cards (never card-styled — it is a stack control,
  not a message); add `items_end` alignment on the
  column so it hugs the right edge like the cards.
- Pin (32 right / 48 bottom), width 320, cap, TTL:
  unchanged. The charter's motion honesty applies —
  the existing opacity fade is the only motion;
  no slide/spring is promised.

## 2. Status strip treatment — `src/rune_sec/status_strip.rs`

What exists (and is good): fixed segments floored by
`est_text_width` (LABEL_CHAR_W 8.0, :712) so nothing
overlaps — the Phase F D1 fix; hairline dividers
already (`divider` :819, 1×12, fill
`t.control_track`); the tail is the single flexible
segment: `fit_hint(core.status, MAX_STATUS_CHARS
96)` (:733), LABEL / `text_secondary`, grow +
truncate (build_strip tail, :986-991). Strip root:
row key `status-strip`, gap 8 (:993).

Charter §12 asks for two changes and no more:

1. **Divider metrics**: 1×12 `control_track` →
   **1×10 `t.separator`** (charter's exact tick).
   One-line change in `divider` (:819-821); no floor
   math depends on the divider's height, and its
   1 pt width floor is untouched.
2. **Numeric segments in MONO_CAPTION**: the
   segments whose content is a number or clock —
   `timecode` (:402), the fps segment
   (`"{} fps"`, build_strip), `zoom_text` (:513),
   `frame_text` (:507, "Frame 1/5"), `undo_text`
   (:484, "Undo 0"), `runtime_label` (:440) where
   shown, `speed_segment` (:645, "0.25×" style),
   `autosave_segment` (:607, "Autosaved 5m ago").
   Project, selection, play, loop, audio, stroke,
   particle, fig3 and the status tail stay LABEL.

**Width-floor caution (read before touching)**:
floors are *estimated* from `LABEL_CHAR_W` and are
pinned by tests, including the Phase K width proof
that the Audio segment cannot afford a glyph inside
the 744 pt centre column (floors 512 + audio 64 +
glyph 17.75 + tail-min 160 = 753.75 > 744). Mono at
12 pt has a uniform advance; before switching a
segment's font, the agent re-derives that segment's
painted width at MONO_CAPTION metrics and confirms
the LABEL-based floor still covers it — if a floor
would under-cover, the segment keeps LABEL and the
agent reports it. The strip must remain
overlap-proof first, mono second. `segment()` /
`segment_with_icon()` (:782 / :805) gain a font
parameter defaulting to LABEL so only the listed
segments change; no label string changes, so
`est_text_width` inputs are identical.

Adjacent, already charter-compliant: the dirty
marker lives in `project_text` (:467, " ●" suffix)
— charter §15's app-bar dirty dot is the chrome
agent's job; the strip keeps its own.

## 3. Confirm-flow pattern — the two-step delete is the template

The pattern as it exists in `home_panel.rs`, and
the spec every destructive surface follows:

- **Arm, don't act.** First press only records
  intent: `HomePanelState.confirm_delete:
  Option<PathBuf>` (:252); `arm_delete` (:746)
  sets it. **Nothing touches the disk until
  confirm** (module docs :71-78).
- **Confirm in place.** The armed row re-renders
  under the *same row key* (`home-row-{i}`) as a
  confirm line — `"Delete {name}?"` in TITLE,
  `home-proj-del-confirm` ("Delete file") and
  `home-proj-del-cancel` ("Keep") — so the list
  does not jump (builder `project_row_confirm`
  :996-1047; `confirm_pending` :433).
- **Dispatch is separate and total.**
  `dispatch_delete_confirm` (:778) performs the
  delete, clears the arm either way; `cancel_delete`
  (:755) clears without acting.
- Styling upgrade for Phase N (home_panel agent):
  the confirm row's fill `t.bg` (:1018) →
  `selected_fill()` (accent 16%) so the armed state
  reads as *selected-for-danger* in the kit
  language, and the confirm button becomes the
  family's active-style button (accent fill, white
  text — `btn(.., active = true)` semantics);
  "Keep" stays a plain button. Keys, labels,
  handlers byte-identical.

**The decision rule for every other surface**
(this is the spec's normative part):

- An op that is **undoable** (it snapshots —
  figure/frame/clip/stroke deletes, clears) gets
  **no confirm**: one press, and the toast's Undo
  (§1) is the recovery path. Adding confirms there
  would train users to ignore them.
- An op that is **not undoable** (it touches the
  disk or destroys data outside the undo stack —
  project-file delete today; library-entry file
  delete in `library_panel.rs` is the same class)
  gets the two-step pattern above, with
  panel-local arm state that already exists or the
  panel's existing state struct — **no new shell
  State fields** (charter rule).
- A-wave agents: when your panel has a
  disk-destructive press that fires on one click,
  flag it in your report with file:line rather
  than inventing a confirm yourself — the
  integrator schedules it as a follow-up so the
  pattern lands once, consistently.

## 4. Scrim & dismissal — `menus.rs`, palette, popovers

Grounded behaviour (menus.rs:624-640, main.rs
mount order):

- `menu_scrim` (menus.rs:628): transparent
  full-window press target, key `menu-scrim`,
  rendered only while a menu is open; a press
  calls `s.menus.close()`. Mounted as a root child
  at main.rs:771, immediately before
  `menu_dropdown` (:772) and **after**
  `toasts_view` (:770) — so the z-order is
  dropdown > scrim > toasts: while a menu is open,
  toast buttons (Undo, dismiss) are *not*
  pressable; the first click anywhere closes the
  menu instead. That is the intended modal-lite
  contract — **Phase N keeps it exactly**: scrim
  stays transparent (no dim tint — Rune has no
  backdrop treatment to promise, charter honesty
  note), key and mount order unchanged.
- The dropdown itself (menus.rs:653-663: padding
  4, radius 8, fill `t.card`, border
  `t.card_border`) moves to the charter §9 metrics
  — padding 6, radius 12, item rows 28 high — but
  that is the menus agent's brief; this spec only
  pins the *feedback* invariants: opening/closing
  changes no state besides `MenusState.open`, and
  Escape-close (main.rs `on_key`) keeps working.
- Command palette (`palette_view`,
  command_palette.rs:1528): card metrics are
  already charter-shaped (radius 12, card fill,
  separator hairline :1563); its no-results row
  ("No commands match", :1571) is an empty state —
  see §5, row 17. The palette has no scrim; its
  dismissal is Escape/select. No change specified.
- Settings popover: same family; no scrim exists
  and none is added — dismissal behaviour is the
  settings agent's surface, out of P10 scope.

## 5. Bare-text feedback → `empty_state` / toast-consistent

Every row below is a place a panel answers with a
bare `text(...)` today. Treatment key: **E** =
replace with kit `empty_state(icon, message, t)`
(centred 20 pt icon + LABEL, `text_secondary`,
py 12 — charter §10; the message string is
*unchanged*, only the container changes);
**P** = prompt state (nothing selected yet) —
same `empty_state` treatment, since these blocks
replace the whole editor area they precede;
**N** = inline notice — keep in place, restyle to
the toast language (severity-tinted icon + LABEL
row, no card). Icons named are all present in
`icon_data.rs` (verified 2026-11-08).

| # | file:line | today | → | icon |
|---|---|---|---|---|
| 1 | audio_panel.rs:698 | "No audio clips in this project." | E | music |
| 2 | camera_panel.rs:811 | "No camera keys yet." | E | camera |
| 3 | clips_panel.rs:995 | "No movieclips in this project. …" | E | film |
| 4 | clips_panel.rs:809 | "Select a clip to set its loop mode." | P | film |
| 5 | clips_panel.rs:937 | "Select a clip to edit its frames." | P | film |
| 6 | fx_clips.rs:1122 | "No FX clips in this project." | E | wand-magic-sparkles |
| 7 | fx_clips.rs:1210 | "Select an FX clip to edit it." | P | wand-magic-sparkles |
| 8 | fx_browser.rs:555 | "No effects match the filter" | E (no-match) | magnifying-glass |
| 9 | fx_browser.rs:590 | "No presets saved yet" | E | wand-magic-sparkles |
| 10 | graph_panel.rs:1533 | "No graph channels yet — select a joint…" | E | chart-line |
| 11 | graph_panel.rs:1798 | "No graph keys on this channel yet — …" | E | chart-line |
| 12 | particles_panel.rs:765 | "No particle events yet — they arrive with imports, or add one." | E | wand-magic-sparkles |
| 13 | particles_panel.rs:520 | "Select a particle event to edit it." | P | wand-magic-sparkles |
| 14 | inspector_figure.rs:859 | "No figures in the project." | E | shapes |
| 15 | inspector_segment.rs:686 | "Select a segment on the stage to edit it." | P | shapes |
| 16 | inspector_style.rs:805 | "Select a segment on the stage." | P | shapes |
| 17 | command_palette.rs:1571 | "No commands match" (36 pt row) | E, inside the palette card; row height becomes the empty_state's own py 12 | magnifying-glass |
| 18 | home_panel.rs:1211-1219 | empty library rendered via `section_label` with "No saved projects yet — …" / "No projects match the search." | E (both strings kept; icon differs: folder-open for empty library, magnifying-glass for no-match) | folder-open |
| 19 | project_tools.rs:822 | "No saved projects in ./projects/ yet." | E | folder-open |
| 20 | library_panel.rs:942 | "No library figures match \"…\"." (text via helper) | E (no-match) | magnifying-glass |
| 21 | figures_panel.rs:716 | "No figures match \"…\"." | E (no-match) | magnifying-glass |
| 22 | pose_tools.rs:1429 | "No poses match \"…\"." | E (no-match) | magnifying-glass |
| 23 | stroke_inspector.rs:1672 | "No strokes on this frame." | E | pen |
| 24 | text_tool.rs:1051 | "No free-standing texts." | E | font |
| 25 | text_tool.rs:1201 | "No text segments on this figure." | E | font |
| 26 | three_d_panel.rs:1045 | "No saved 3D figures." | E | cube |
| 27 | three_d_panel.rs:109 |
...[truncated 7373 chars]