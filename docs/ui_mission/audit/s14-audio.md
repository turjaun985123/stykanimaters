# Squad 14 Audit — Audio & Clips

ULTIMATE UI/UX MISSION, Phase 1. Scope: the audio
interface — `src/rune_sec/audio_panel.rs`,
`src/rune_sec/waveform_view.rs`,
`src/rune_sec/clips_panel.rs` (movieclips), and the
interface-facing surface of `src/audio_ops.rs` /
`src/audio_edit.rs` (status strings, parameters,
probes). Sound correctness itself is user-verified
only and out of scope; everything below is about what
the user sees, can find, and can control.

Method: full source read of the three panel files,
the op signatures and status strings of `audio_ops`,
the DSP signatures of `audio_edit`, the waveform mount
in `src/main.rs`, the Audio workspace definition in
`src/rune_sec/workspaces.rs`, and the scene table in
`src/rune_sec/scenes.rs`. No local build (AGENTS.md);
findings are source-proven, and the one visual claim
that needs pixels (D15) is stated as a gap, not a
verdict.

## What already works — do not rebuild

- Phase N kit adoption is complete in both panels:
  `panel_col` / `panel_title` / `divider` /
  `list_row` / `section_label` / `empty_state` /
  `stepper_value`, Font Awesome glyphs per control.
- Per-clip selection with indexed ops: since Phases
  G/H every DSP button aims at the selected clip
  (last-clip fallback), none is last-clip-only.
- Mute is a real model flag (`AudioClip.muted`) and is
  visible in rows (`(muted)` marker), the readout
  (`· muted`), the waveform (dimmed bars/bracket) and
  the timeline clips lane.
- The selected-clip readout shows name, start frame,
  gain, probed duration and mute state.
- The waveform has a peaks cache, a selected-clip
  mode with caption, a span bracket, all-clip boundary
  ticks, and clip-span-mapped scrubbing in clip mode.
- `audio_ops` status strings are exemplary: nearly
  every op and failure names itself in the status
  line ("Audio echo failed: …", "Echo applied to
  audio clip 2 (0.18s delay, 35% feedback): …").
- The movieclips panel has a genuinely good empty
  state and per-row Dup / Rename / Insert / Up / Dn /
  Del with careful index-remap discipline.

## Defects

### D1 (P1) — The Operations toolbar is one 17-control horizontal scroll strip, and it mixes scopes

`audio_panel`'s view builds a single `.scroll_x()`
row (`audio-ops`) holding, in order: Normalise, Echo,
Lowpass, Trim silence, Split silences, Pitch +2,
Compress, Fade, Reverse, Mute, Duplicate, Gain −,
gain value, Gain +, Earlier, Later, Mixdown. The
right column's content width is 316 pt, so only a few
controls are visible; the rest hide behind horizontal
scrolling with no affordance that more exists. Four
different kinds of thing are undifferentiated in the
strip: clip DSP effects, a state toggle (Mute), clip
management (Duplicate, Earlier/Later), a property
stepper (Gain), and — breaking the panel's own rule —
Mixdown, which is whole-project. Phase J deliberately
gave Normalise-all its own "Whole project" row
*because* project-scope ops do not belong in the
aimed toolbar; Mixdown never got the same treatment.

### D2 (P1) — Missing, unreadable and non-WAV clips are invisible in the UI

A clip whose file is missing renders exactly like a
healthy silent clip. `clip_duration_suffix` returns
`""` when `read_wav` fails; `audio_ops::clip_peaks`
and `clip_duration_secs` are WAV-only, so no
waveform, no duration, no span bracket, no boundary
ticks — and no badge, marker or warning anywhere.
This also hits MP3/OGG clips wholesale: the add flow
explicitly picks the first `.wav/.mp3/.ogg` in
`./audio/`, yet every visual probe in the app only
understands WAV, so a perfectly valid MP3 clip shows
no duration and no waveform and gives the user no
reason why. Professional tools mark missing media;
here "silent", "missing" and "unsupported format"
are indistinguishable.

### D3 (P1) — Every audio effect is a one-click black box with hard-coded parameters

Fade is fixed at 0.25 s in / 0.25 s out
(`audio_ops.rs` `FADE_IN_SECS` / `FADE_OUT_SECS`),
Echo at 0.18 s delay / 35 % feedback, Pitch at
exactly +2 semitones, Lowpass and Compress at fixed
internal values. The panel offers no parameter
control for any of them; the parameters are
learnable only from the after-the-fact status line.
The DSP layer is already parameterised
(`audio_edit::fade(data, in, out)`,
`echo(w, delay, decay)`, `pitch_shift(w, semitones)`,
`lowpass(w, cutoff)`, `compress(w, threshold,
ratio)`) — the indexed ops hard-code the arguments.
Audio property editing, beyond gain, effectively
does not exist.

### D4 (P1) — The panel re-decodes every WAV file on every render

`summary_line`, `clip_duration_suffix` (per row) and
`clip_readout` (via `audio_ops::clip_duration_secs`)
all call `audio_edit::read_wav` directly, per clip,
per view build — a full file decode just to print
text. During playback the shell rebuilds per frame,
so every clip file is decoded every frame.
`waveform_view` solved exactly this problem with its
path + length/mtime peaks cache (Phase E); the panel
and readout path never received the same treatment.

### D5 (P2) — With zero clips the toolbar stays fully live, and Mute is a silent no-op

When the project has no clips, the empty state shows
— and below it the full Operations section still
renders, all 17 buttons live (every `icon_btn` /
`btn` call passes the non-inert form). Most presses
produce an error status; `mute_target` produces
nothing at all — with no effective target it returns
without touching the status line, the one completely
silent press in the panel. The kit's inert/muted
button state is never used anywhere in this panel.

### D6 (P2) — No "now sounding" feedback during playback

`audio_edit::audible_at` already classifies which
clips are audible at the playhead; only the playback
engine (`src/audio.rs`) consumes it. No UI surface
does: no clip row highlights while its clip is
sounding, so during playback the user cannot see
which clips they are hearing (or which muted clip is
correctly silent). The only playback feedback in the
whole audio interface is the waveform playhead.

### D7 (P2) — Waveform colours are hard-coded literals, blind to the theme

`waveform_view.rs` draws with module constants —
`WAVE_HEX 0x6D8CFF`, `CENTRE_HEX 0x303744`,
`PLAYHEAD_HEX 0xFFC65D`, `CAPTION_HEX 0xA2AAB8` —
and `waveform_el_state` does not even take a
`&Theme`. In the light theme the caption (light grey
`#A2AAB8`) sits on a light card at poor contrast,
and the centre line (`#303744`, a dark-theme border
tone) is off-palette. The strip also cannot follow
any future token change. Coordination: Squad 02 owns
tokens; this squad flags the consumer.

### D8 (P2) — Waveform project mode never says what it is showing; the card has no empty state

In project mode the strip draws the LAST clip's peaks
stretched across the full width whatever that clip's
start frame or length (documented as deliberate),
with no caption naming the clip — project mode grows
no caption element at all. The bars are clip-time;
the playhead is project-time; only the span bracket
(when a clip is selected) hints at the relationship.
With no clips, or an unreadable last clip, the card
is a bare centre line: no empty state, no
explanation, in a card titled "Waveform".

### D9 (P3) — The toolbar's target is described twice, in two formats, neither complete

`clip_readout` (under the rows) and `target_line`
(under "Operations") both name the effective target:
"Clip 2: sting.wav · starts frame 30 · gain 0.50 ·
3.2s · muted" vs "Target: last clip — sting.wav
(clip 2 of 2)". The readout lacks the fallback
warning; the target line lacks gain, duration and
mute. Two lines doing one job, each missing what the
other has.

### D10 (P3) — The last-clip fallback target is not marked on its row

With nothing selected, every toolbar op targets the
last clip. The two text lines (D9) disclose this, but
the targeted ROW carries no visual mark — the kit
selected fill appears only after an explicit row
press. A new user who adds a second clip and presses
Reverse reverses a clip they never chose, with the
only warning in small secondary text.

### D11 (P3) — A clip's start frame can only be nudged one frame per press

Earlier/Later call `audio_move_at` with ±1 frame.
There is no start-frame stepper, field, or coarse
step: placing a clip 5 s into a 30 fps project takes
150 presses. The readout displays the start frame
but offers no way to set it. (Dragging on the
timeline clips lane exists as the coarse path, but
nothing in the Audio workspace points to it.)

### D12 (P3) — Gain is stepper-only: no slider, no numeric entry, no unity reset

The target clip's gain (clamped 0.0..=2.0 in
`audio_set_gain_at`) is editable only through
Gain − / Gain + buttons at ±0.1. No slider (the kit
has `slider_track`, unused here), no typed value, no
"reset to 1.00", no dB readout. Gain is the one
per-clip property with any control, in its coarsest
possible form.

### D13 (P3) — Movieclip rows are overloaded, and the header "Insert clip" inserts a different clip than you think

`clips_panel::clip_row` packs the name, a
span + usage text, the loop-mode label and six
buttons (Dup, Rename, Insert, Up, Dn, Del) into the
316 pt column. `usage_text` joins ALL playing
figures' names with no bound, so a well-used clip's
row grows arbitrarily wide. Worse, the header
"Insert clip" button calls
`EditorCore::insert_clip_figure`, which always
inserts the FIRST clip — even when another clip is
selected — while the per-row Insert (Phase M)
inserts its own clip. Two controls with the same
name doing different things.

### D14 (P3) — The loop-mode control is detached from the clip it edits, and can cost two undo steps

The Loop / Ping-pong / Once segmented row sits below
the rename row, edits the panel selection with no
clip name in its label ("Loop mode:" is a
`section_label` used inside a control row), and
`apply_loop_mode` reaches the target by cycling
through `cycle_clip_loop` — up to two presses, each
one snapshot, so one apparent change can take two
undo steps. The undo cost is engine behaviour and is
flagged, not silently changeable; the placement and
labelling are panel defects.

### D15 (P3) — No headless scene has ever shown a populated audio workspace

`scenes.rs` defines 15 scenes; none adds an audio
clip, and the demo project carries none. The
`snap_tests` workspace assertion for `ws-audio`
checks only that `audio-panel`, `waveform-card` and
`playback-controls` mount — with empty content. The
populated audio UI (rows, live toolbar, real peaks,
clip mode, span bracket, muted rendering) has never
been pixel-verified. Coordination: Squad 19.

### D16 (P4) — The audio empty state does not teach the add flow

"No audio clips in this project." — full stop. It
never mentions the name field below or the
`./audio/` folder convention; that teaching lives
only in the field's placeholder, which disappears
once the user starts typing. The movieclips empty
state ("\"Make clip\" builds one from the selected
figure.") is the pattern to match.

### D17 (P4) — Per-row audio ops are an inconsistent subset, with three names for one op

Rows offer Norm / Split@cur / Xfade / Delete but no
per-row Mute, Duplicate or gain — the three things
most often wanted per clip — forcing select-then-
toolbar for mute and duplicate even though indexed
ops for both exist (`audio_set_muted_at`,
`audio_duplicate_clip_at`). And normalise appears
under three names in one panel: row "Norm", toolbar
"Normalise", project row "Normalise all".

## Work items (each sized for one coder)

- **I-1 — Regroup the operations toolbar.**
  `audio_panel.rs` view only. Split `audio-ops` into
  labelled groups (Effects / Clip / Position & gain),
  move Mixdown into the "Whole project" row beside
  Normalise all. Every key, label and handler
  verbatim. Acceptance: key census unchanged;
  `ws-audio` snap test still mounts; groups visible
  without horizontal scrolling at 316 pt where the
  kit allows wrapping. Fixes D1.
- **I-2 — Missing/unreadable clip indicator.** Row
  label and readout gain a clear "(missing file)"
  treatment driven by a probe result (not by empty
  peaks, which cannot tell silence from absence);
  waveform caption states it in clip mode. One new
  pure helper in `audio_ops` + panel rendering.
  Fixes D2 (WAV scope; MP3/OGG waveform support is
  an engine feature, separately flagged).
- **I-3 — Cached clip-metadata probe.** Extend the
  waveform peaks-cache pattern (path + length +
  mtime) in `audio_ops` to serve duration and peak;
  route `summary_line`, `clip_duration_suffix` and
  `clip_readout` through it. No view changes.
  Acceptance: existing panel tests pass; no
  `read_wav` call remains in `audio_panel.rs`.
  Fixes D4; enables I-2 and I-6.
- **I-4 — Fade and Echo parameter steppers.** Panel
  steppers for fade length and echo delay/decay
  backed by parameterised indexed ops (the
  `audio_edit` functions already take the values).
  Two half-items (fade, echo) if one coder's plate
  fills. Fixes D3 for the two most-used effects;
  Pitch/Lowpass/Compress parameters follow the same
  pattern as a later item.
- **I-5 — Empty-project toolbar discipline.** With
  no clips, the Operations section renders in the
  kit inert state (or collapses to the target line);
  `mute_target` gains the standard no-clips status
  line. Fixes D5.
- **I-6 — Now-sounding row marker.** Audio rows mark
  the clips `audible_at` reports at the current
  frame (a small glyph or row affordance, muted
  clips excluded by construction). Uses I-3's
  durations. Fixes D6.
- **I-7 — Waveform theme tokens.** `waveform_el_state`
  takes `&Theme`; bars, centre line, playhead and
  caption derive from theme tokens instead of hex
  constants (main.rs mount updated by the
  integrator). Acceptance: light-theme snap shows a
  legible caption. Fixes D7.
- **I-8 — Waveform project-mode caption + empty
  state.** Project mode names the clip whose peaks
  are drawn ("Last clip: {name}" until a truer
  multi-clip strip exists); the card gains a kit
  empty state when nothing can be drawn. Fixes D8.
- **I-9 — One target line.** Merge `target_line`
  into `clip_readout`'s format (keeping the
  fallback wording), delete the second line, keep
  the `audio-readout` key. Fixes D9.
- **I-10 — Fallback target row affordance.** When
  the selection is empty/stale and clips exist, the
  effective target's row renders with a distinct
  muted-selected treatment so the toolbar's aim is
  visible on the row itself. Visual only; selection
  semantics unchanged. Fixes D10.
- **I-11 — Start-frame steppers.** In the readout
  area for the effective target: −10 / −1 / +1 / +10
  frame steppers over `audio_move_at`. Fixes D11.
- **I-12 — Gain slider + unity reset.** Kit
  `slider_track` for the target gain beside the
  existing stepper (keys unchanged), plus a
  "1.00" reset button calling `audio_set_gain_at`.
  Fixes D12.
- **I-13 — Movieclip row diet + honest Insert.**
  Truncate `usage_text` (first two figure names +
  "+n more"); rename the header button to state
  what it does ("Insert first clip") OR make it
  insert the selected clip — integrator decides;
  the label fix is the minimum. Fixes D13.
- **I-14 — Loop-mode placement and label.** Move
  the segmented row directly under the clip list and
  label it with the selected clip's name; document
  the two-undo-step cost in `docs/PHASE_O.md` /
  mission docs for an engine decision. Fixes D14's
  panel half.
- **I-15 — Populated audio scene.** New headless
  scene: generate a small WAV fixture, add two clips
  (one muted, one selected), open the Audio
  workspace; extend `snap_tests` so the populated
  panel + waveform are pixel-verified. With Squad
  19. Fixes D15.
- **I-16 — Empty-state copy.** Audio empty state
  gains the add-flow hint, matching the movieclips
  pattern. Fixes D16.
- **I-17 — Per-row mute toggle.** Icon-only volume
  button per audio row over `audio_set_muted_at`
  (the row's `(muted)` marker and the toolbar
  toggle already read the same flag). Fixes D17's
  mute half; naming unification rides I-1.

## Coordination notes

- Squad 02: D7 needs theme tokens for canvas
  colours; D1's grouping should reuse kit section
  anatomy, not invent panel-local chrome.
- Squad 09 (timeline): D11's coarse path is the
  timeline clips lane; any in-panel fix should not
  duplicate lane behaviour beyond steppers.
- Squad 17: glyphs needed — a warning/missing-file
  glyph for I-2 and a sounding indicator for I-6
  must come from the curated icon table.
- Squad 19: I-15 is the audio surface's first real
  visual verification; light-theme waveform (I-7)
  needs a light snap in the same pass.
- Standing constraints honoured throughout: zero
  key loss vs the 804 census, no fake controls
  (every proposed control maps to an existing op),
  CI-only verification.
