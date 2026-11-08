# Phase O Charter — Feel & Finish

> Phase N gave the app one design kit (structure: card +
> hairline, one component family in `src/rune_sec/design.rs`).
> Phase O makes that kit *feel* professional: interaction
> states, motion consistency, feedback completeness, empty
> states everywhere, keyboard discipline, and the overflow
> repairs Phase N's pixel pass recorded. Same phase shape
> as N: planning wave → foundation → adoption → verification
> → integrator pixel pass.

## What Phase O is

- A **feel-and-finish** phase. The visual language is
  settled (Phase N tokens, radius, palette). Phase O does
  not re-restyle surfaces; it makes the existing surfaces
  respond, move, and report back like a professional tool.
- Seeded from Phase N §8's recorded remainders (below),
  not from a blank page.

## What Phase O is not

- No new engine features, no new panels, no format work.
- No enabling of `tab_visits_controls` (deferred product
  decision — the user's, standing).
- No change to the two-"New project" behaviour (user's
  product call, standing).
- No audio-path work; sound stays user-verified.
- No darkening of the accent to chase the 4.19:1 number
  (documented acceptance stands).

## Rules (binding on every plan and every coder)

1. **Kit first.** New interaction visuals land in the kit
   (`design.rs`, `chrome_view`, `icon_btn`) so all panels
   inherit them. A per-panel one-off needs a named reason
   in its plan.
2. **Zero key loss.** The repaired census baseline is
   **804 keys** (Phase N close). Keys, handlers and labels
   stay verbatim unless a plan item explicitly lists a
   copy change. The gate must pass all checks, plus any
   new check the verification wave adds.
3. **Every interactive element has its states designed:**
   rest / hover / press / selected / muted (inert) — using
   only the existing token set. Any state Rune cannot
   express is named in the plan as an engine limit, never
   faked with a look-alike.
4. **Motion discipline.** Only `tok::motion::pair::SNAP`
   and `pair::SETTLE` springs; entrance/exit behaviour for
   overlays and toasts follows Phase N's. No new durations
   or curves without a named reason; no animation that
   delays input.
5. **Width budgets are stated, not hoped.** Any item that
   adds or widens row content names its budget: left
   column content 276 pt, right column content 316 pt,
   centre flexible. Nothing may cross a card edge (the
   figures-panel lesson, Phase N fix 5).
6. **No silent mutations.** Every mutating action leaves a
   trace — a toast through `classify_status`, a status
   string, or an immediate visible state change. Plans
   list the silent mutations they find as defects.
7. **Empty states complete the set.** Every bare empty
   line Phase N §8 listed gets the kit `empty_state`
   treatment with a glyph that already exists in the
   curated 163-icon table — or the plan names the icon
   the foundation wave must add (Font Awesome Free 7.3.1,
   verified single-path).
8. **Keyboard discipline.** Esc closes the topmost
   overlay; overlay open/close behaviour otherwise
   unchanged except items a plan names. Shortcuts shown
   in the UI must be real (the Ctrl+S lesson).
9. **Copy is en-GB**, numbers in mono where the Phase N
   numeric discipline already applies.
10. **Proof is named per item.** Every plan item names
    how it will be proven: a Runtime test, a gate check,
    a headless scene/snap, or the integrator's pixel
    read. "Looks better" without a proof surface is not
    a plan item.

## Seed work (Phase N §8 remainders, verbatim scope)

- Settings language rows can run past the card's right
  edge at 1440 pt — repair + a general card-overflow
  audit method (tree-dump width scan).
- The fx browser's "No presets saved yet" line is bare
  text — kit empty state.
- Bare empty lines ruled out of Phase N scope, now in:
  project_tools' empty project-list line, text_tool's
  two empty messages, stroke_inspector's "No strokes on
  this frame.", three_d_panel's sub-section lines,
  camera_panel's "No camera keys yet.", the segment /
  figure inspectors' no-selection lines.
- Zero-consumer kit items from Phase N (`text_muted`
  unused, `toggle` unused — `toggle_row` is the used
  form): adopt or retire, decided on evidence.

## Waves

- **P — planning (12):** audits + plan docs in
  `docs/phase_o_plans/`, one per area, each item sized
  for one coder, with file/key/proof named.
- **F — foundation:** kit extensions the plans call for
  (interaction states, motion helpers, any icon
  additions), landed and gated before adoption starts.
- **A — adoption:** coders per file cluster, kit-first,
  keys verbatim.
- **V — verification:** scenes/tests for new states,
  gate check 7 if the plans define a checkable
  invariant, copy pass, docs (PHASE_O.md + QA_RUNE §17).
- **Integration:** the integrator merges, runs the CI
  loop to green, re-dispatches headless with the
  `./projects` wipe discipline, pixel-reads every snap,
  and reports the honest agent count.
