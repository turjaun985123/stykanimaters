# QA Review — Wave 14 (Professional Redesign)

**Reviewer role:** independent QA (Agent 20 function). This review is
read-only: nothing in `src/` was modified by the reviewer. All checks
were run with `git`/`grep`/scripted extraction against the working
tree; no local cargo build or test was run (repo rule — CI is the
only build environment).

**Audit times (UTC, 2026-11-08):**

| Run | Time | Tree state |
|---|---|---|
| Baseline (HEAD) | 04:22 | clean; HEAD = `b0f5578` (post-Wave-13 docs) |
| Interim | 04:28 | Wave 14 diff at +1,872 / −141 across 24 files |
| Final | 04:33 | diff unchanged from interim — tree stable ≥ 5 min |

The final numbers below are from the 04:33 run. If further Wave 14
work lands after 04:33, this audit must be re-run at integration
(the extraction method is described under Check 1 so it can be
repeated exactly).

---

## Headline findings

1. **No existing control was lost. Zero HEAD button ids are missing
   from the redesigned `src/app.rs`.** A diff-level cross-check is
   even stronger: the redesign diff contains **zero removed
   `btn(`/`.id(` lines** — no existing button's id or listener line
   was touched at all.
2. **No dead controls.** The wave added exactly 4 new interactive
   elements (3 buttons + 1 clickable div family). Every one calls a
   real editor/view function, verified to the function definition.
3. **Token discipline improved in the component library:** raw
   `rgb(0x…)` literals in `src/ui/*.rs` outside `kit.rs` went
   **3 → 0**. `src/app.rs` is unchanged (19 → 19, all pre-existing).
4. **One duplicate-id flag was raised and cleared** (`wx-seg`,
   Check 4): two literal sites, disjoint index ranges forming one
   control's id set — the established tuple-id pattern, not a
   collision.
5. The new `src/ui/kit.rs` tokens match the redesign directive's
   suggested palette **exactly** (`C_BG 0x101216`, `C_SURFACE
   0x171A21`, `C_PANEL 0x1D222B`, `C_ELEVATED 0x252B36`, `C_ACCENT
   0x6C8CFF`, `C_ACCENT2 0x9B7CFF`, `C_INK 0xF1F3F7`, `C_MUTED
   0xA1A9B7`, `C_BORDER 0x303745`), plus spacing (`SP_*`),
   typography (`TEXT_*`) and radius (`RAD_*`) scales and both
   `Palette::DARK` and `Palette::LIGHT`.

---

## Check 1 — Button preservation (HEAD vs working tree)

**Method.** Element ids were extracted from `git show
HEAD:src/app.rs` and from the working-tree `src/app.rs` with the
same scripted extractor, covering every id form the file actually
uses:

- `btn("id", …)` — plain button ids
- `btn(("id", i), …)` — tuple button ids (incl. multiline calls)
- `xc_row("id", …)` — export-centre rows; the closure forwards the
  literal to `btn(id, …)`
- `stepper_row(…)` call args — the weather panel's dn/up ids are
  passed as literals to a local closure that forwards them to `btn`
- `.id(("id", i))` — clickable divs (frame chips, swatches, weather
  segments, export tabs, figure rows), tracked separately below

**Result — button ids:**

| Category | HEAD | Working tree | Missing | New |
|---|---|---|---|---|
| Plain `btn("…")` | 193 | 196 | 0 | 3 |
| Tuple `btn(("…", i))` | 23 | 23 | 0 | 0 |
| Export-centre `xc_row("…")` | 15 | 15 | 0 | 0 |
| Stepper-forwarded | 8 | 8 | 0 | 0 |
| **Union (unique ids)** | **235** | **238** | **0** | **3** |

**Missing HEAD ids: none.** Every one of the 235 ids present at
HEAD is present after the redesign.

**Clickable div ids (`.id(…)`), tracked alongside:**

| HEAD | Working tree | Missing | New |
|---|---|---|---|
| fig-sel, frame, swatch, wx-seg, xc-tab | fig-sel, frame, swatch, wx-seg, xc-tab, **ws** | none | `ws` |

**Diff-level cross-check.** `git diff src/app.rs` was scanned for
added/removed lines containing `btn(` or `.id(`. Added call sites:
exactly the 4 new controls of Check 2. Removed call sites: **0**.
The app.rs portion of the redesign is therefore styling, layout and
panel-organisation work around untouched control wiring.

---

## Check 2 — New controls and what they actually do

Every new interactive element added by the wave, with its listener
verified to a real function definition:

| New id | Kind | Listener calls | Target verified |
|---|---|---|---|
| `view-fit` | button | `view.core.frame_all_figures()` | `EditorCore::frame_all_figures` — `src/editor.rs:582` (new op added by this wave) |
| `view-centre` | button | `view.core.centre_on_selection()` | `EditorCore::centre_on_selection` — `src/editor.rs:629` (new op, with its own unit tests at `src/editor.rs:6375`+) |
| `view-actual` | button | reads `view.core.camera_here()`, then `view.core.camera_zoom_by(1.0 / zoom)` when zoom > 0 | `src/editor.rs:530` / `src/editor.rs:534` — a genuine 1:1 implementation, not a fixed-factor zoom |
| `("ws", i)` | clickable div (workspace selector) | `view.apply_workspace(i)` | `src/app.rs:319` — sets `self.workspace` and reconfigures the panel-visibility flags per workspace preset (Animate closes graph/audio/FX/clips/export, leaves draw/3D modes, etc.); `render()` consumes those flags, so the switch is functional |

The 8 workspace names are `Animate, Draw, Build, Graph, Effects,
Audio, 3D, Export` (`WORKSPACE_NAMES`, `src/app.rs:45`) — matching
the redesign directive's workspace list.

**Dead-control verdict: none found.** No new control's listener is
limited to `cx.notify()`, and no new flag is write-only:

- The three `view-*` buttons mutate the camera through editor ops.
- `apply_workspace` mutates view state that `render()` reads on the
  same frame.

**Components stay listener-free:** `on_click` appears in
`src/ui/*.rs` only inside doc comments (breadcrumb/tabs/toolbar
explain that callers attach clicks) — the component contract from
the folder wave (pure builders, caller attaches `.id`/`.on_click`)
is still honoured.

---

## Check 3 — Token discipline (raw `rgb(0x…)` literals)

| Location | HEAD | Working tree | Delta |
|---|---|---|---|
| `src/ui/*.rs` excluding `kit.rs` | 3 | **0** | **−3 (improved)** |
| `src/app.rs` | 19 | 19 | 0 |
| `src/ui/*.rs` excl. `kit.rs`, `rgba(0x…)` | 0 | 0 | 0 |
| `src/ui/kit.rs` itself, `rgb(0x…)` calls | 0 | 0 | 0 (tokens are `0x…` const values there, as intended) |

The 3 eliminated literals were identical cell-background values
(`0x2C313A`) in `ui/clip_row.rs`, `ui/color_field.rs` and
`ui/timecode.rs`; those components now consume kit tokens (the
palette value itself moved to the new `C_CELL`/`C_ELEVATED`
`0x252B36` as part of the palette redesign — a deliberate colour
change, not a discipline issue).

The 19 remaining `app.rs` literals are all pre-existing and were
not added by this wave: the `btn()` helper's own colours (the
helper predates the token migration and defines the values other
code mirrors), repeated `text_color` calls in panel code, and
canvas paint colours for joints/paths (`0xFFC65D`, `0xFF5D5D`,
`0x35C4D8`), which are stage-render constants rather than chrome
styling. Not a regression; migrating the `btn()` helper and panel
text colours onto the new `Palette` is legitimate follow-up work.

---

## Check 4 — Duplicate ids

Literal occurrence counts were taken for every id form in the
working-tree `src/app.rs`. Any id stem appearing more than once as
a literal was flagged for manual inspection:

| Form | Stems appearing > 1× | Verdict |
|---|---|---|
| Plain `btn("…")` | none | clean |
| Tuple `btn(("…", i))` | none | clean |
| `xc_row("…")` | none | clean |
| Div `.id(("…", i))` | `wx-seg` (2 sites) | **cleared — see below** |

**`wx-seg` clearance.** The two sites build one weather segmented
control in one panel function: site 1 hard-codes `.id(("wx-seg",
0usize))` for the "Off" segment; site 2 loops the five weather
kinds with `.id(("wx-seg", n + 1))` (indices 1–5). The index ranges
are disjoint and together form the single set `("wx-seg", 0..=5)`.
This is the established tuple-id pattern (same stem, unique index
per element) — there is no ElementId collision and no duplicated
control.

---

## Additional observations (context, not failures)

- `src/editor.rs` gained ~303 lines: the camera ops the new
  `view-*` buttons call (`frame_all_figures`,
  `centre_on_selection`, plus tests). These are additive editor
  operations; no existing op signatures changed (consistent with
  Check 1 finding every pre-existing control intact).
- `src/shell.rs` gained ~135 lines and `EditorView` restores the
  workspace from a persisted layout (`src/app.rs:72`, `:252`) —
  layout persistence exists in code; its on-disk behaviour is for
  CI/manual testing, not this static audit.
- `src/i18n.rs` gained ~268 lines — new UI strings were added
  through the existing 28-language infrastructure rather than
  hard-coded, per the directive's localisation rule (spot-checked:
  the diff adds keys; full per-language completeness is the i18n
  test suite's job in CI).
- Phase-1 deliverable docs exist as new files:
  `docs/REDESIGN_ARCHITECTURE.md`, `docs/AGENT_TASK_MATRIX.md`,
  `docs/UI_COMPONENT_CONTRACTS.md`, `docs/GUI_AUDIT.md`.

## Open risks

1. **Nothing in this wave has compiled yet.** Per the repo rule,
   CI is the first build. This audit proves id/wiring preservation
   statically; it cannot prove the redesign diff type-checks. The
   integration CI run is the verdict, as with every wave.
2. **Static id audit scope.** GPUI ElementId collisions across
   *different panels rendered simultaneously* can't be fully
   excluded by literal counting (e.g. the same tuple stem used in
   two panels with overlapping index ranges). Within `app.rs` the
   only repeated stem is the cleared `wx-seg` case, but runtime
   behaviour is the real test.
3. **Point-in-time snapshot.** The tree was stable from 04:28 to
   04:33 UTC. Work landing after the final run invalidates the
   counts above until re-audited.
4. **Light theme consumption.** `Palette::LIGHT` exists in
   `kit.rs`, but this audit did not trace which panels consume the
   active palette vs the `C_*` dark constants directly. If panels
   read `C_*` constants, a light-theme switch would restyle only
   some surfaces — check in the screenshot pass (below).
5. **Pre-existing raw literals in `app.rs`** (19) remain; they are
   not wave regressions but they are unfinished token migration.

---

## User screenshot checklist (required to close Wave 14 visual QA)

Compilation is not proof of a successful redesign (directive
Rule 9). Screenshots cannot be produced on this VM or in CI:
headless renders of this GPUI app come out black here — an
established project fact since before the redesign. **The
screenshots must come from the user's real desktop**, running the
CI artifact from the Wave 14 integration run.

### Window sizes to capture

Capture the main window at each of these sizes (the directive's
Part IV matrix):

- [ ] 1280 × 720
- [ ] 1366 × 768
- [ ] 1600 × 900
- [ ] 1920 × 1080
- [ ] 2560 × 1440

At minimum, capture every workspace (below) at 1920 × 1080, and
the Animate workspace at all five sizes — small sizes are where
panel collapse behaviour matters most.

### Workspaces to capture (all 8)

- [ ] **Animate** — stage centred, timeline + dopesheet readable,
      figure sidebar proportions sane
- [ ] **Draw** — brush/shape controls reachable, canvas not
      crowded out by tool panels
- [ ] **Build** — figure/segment controls and library visible
- [ ] **Graph** — curve panel height sufficient, axis labels
      legible, overlay curves visible but faint
- [ ] **Effects** — FX panel clip rows + preset buttons not
      clipped
- [ ] **Audio** — waveform strip and per-clip buttons visible
- [ ] **3D** — 3D controls grouped; session state clear
- [ ] **Export** — export centre tabs (All/Media/Formats) and all
      format rows reachable without overlap

Plus:

- [ ] **Home / project launcher** (first screen)
- [ ] **Light theme** (if a theme switch exists in the build) —
      at least Animate + Home; watch for surfaces that stayed dark
      (Open risk 4)

### What to look for in each shot

- [ ] No clipped or overlapping controls; no button text cut off
- [ ] No text below the 11px floor (kit `TEXT_XS`); status and
      hint lines still readable
- [ ] Panel proportions balanced — the stage remains the visual
      centre and stays usable at 1280 × 720 (secondary panels
      collapse instead of crushing the stage)
- [ ] Consistent button heights, spacing and borders across
      panels — the app should read as one product, not panels from
      different apps
- [ ] Active/inactive states obvious (selected figure, active
      workspace tab, toggled panels)
- [ ] The three new canvas buttons (Fit / Centre / 1:1) visible
      near the stage and the workspace selector row complete
- [ ] Weather segmented control shows all six segments
      (Off/Rain/Snow/Fireflies/Fog/Storm) without wrapping badly
- [ ] Toast/status area does not cover the timeline
- [ ] Long translated labels (switch language to German or
      Bengali for one shot) do not overflow their buttons

**Until these screenshots are reviewed, Wave 14's visual
acceptance criteria remain open** — regardless of CI test results.
