# PHASE_E.md — Rune Phase E: closing every Phase D gap

Snapshot: 2026-11-08, ~07:20 UTC, written **while the wave is
still landing** — other sections are being edited concurrently,
so every count in this file is a moving snapshot, not a final
figure. Re-run `python3 scripts/rune_inventory.py` for the
current count; the freshest run is recorded in §6.

## 1. Charter

Phase D put the whole GPUI-era feature surface back on Rune in
40 sections and went CI-green (run `42457752206`, 1,020 tests),
but it shipped with a written list of honest gaps: things the
sections had to leave out because **the engine had no operation**
for them, surfaces that were duplicated between the app bar and
the panels, labels that were still hardcoded English, playback
that started no audio, a timeline that could not show marks,
and a layout in which every section was scroll-reachable instead
of gated by workspace.

**Phase E's charter is to close every one of those known gaps**
— no new feature families. Where a gap existed because an engine
op was missing, Phase E adds the op to the engine (so the shell,
the MCP server and scripts share it) instead of growing another
shell-side workaround. Where a surface was duplicated, one home
survives. Where a label was hardcoded, it goes through the
28-language table.

**"Perfect", defined honestly:** all *known* gaps closed, the
full suite green in CI, and the inventory preserved or grown.
It does not mean the UI has been seen: the Rune shell has still
never been screenshot-verified on a real screen, and `--snap`
on the GPU-less CI runner is still unproven (software Vulkan).
The screenshot remains the user's move (§5).

## 2. The gap list Phase E closes

From `docs/QA_RUNE.md` §5 and the Phase D delivery report:

| # | Gap | Root cause |
|---|---|---|
| G1 | No audio gain / fade / reverse / delete-clip controls | no `EditorCore` ops existed |
| G2 | FX clip edits done inline in the panel (snapshot + free fns) | no `EditorCore` FX op family |
| G3 | Set-figure-colour / duplicate-figure were shell-local helpers | no engine ops |
| G4 | Graph "copy keys" not built | no key clipboard in the engine |
| G5 | Style panel could only cycle taper/cap/gradient | no direct-set ops |
| G6 | No per-stroke delete | no stroke-removal op |
| G7 | Stage did not draw weather/FX particles, camera rotation, guides, or text labels | stage_view rendered figures only |
| G8 | Timeline showed chips only — no loop/key/camera marks | chrome `timeline_row` was Phase C scope |
| G9 | All sections scroll-reachable; no workspace gating | Studio/2.0 design not yet applied to Rune layout |
| G10 | Duplicate controls: `add-figure` and `onion` in both app bar and panels | Phase D mounted both surfaces |
| G11 | Shell labels hardcoded English; 8 shell keys missing from the i18n table (`settings`, `frame`, `frames`, `figures`, `prev_frame`, `next_frame`, `fps`, `zoom`) | wiring layer only (Phase D §38) |
| G12 | Playback starts no audio | shell never started the mix |
| G13 | Guides toggle omitted from settings | no guides field/renderer existed |
| G14 | Help panel listed only 11 shortcuts; `Alt+1..8` workspace keys not wired | shell `on_key` scope |
| G15 | Status-strip vs playback-controls timecode disagreed (floor vs round) | two independent implementations |
| G16 | Project easing could only cycle | no direct setter |
| G17 | No record shown to the user after an export | export ops reported only via status line |

## 3. The 40 sections

Section numbers are taken from the file headers where the file
has landed; rows marked *(brief)* are per the wave brief and
their files were still landing at snapshot time.

| § | File | Closes | What it does |
|---|---|---|---|
| 1 | `src/rune_sec/workspaces.rs` | G9, G14 | The 8 workspaces (Animate/Draw/Build/Graph/Effects/Audio/3D/Export) as shell state; the gating table the integrator applies in `view()` |
| 2 | `src/stage_view.rs` (stage layers) *(brief)* | G7 | Stage composes the layer stack: figures + weather/FX + camera/guides + text in one transform |
| 3 | `src/rune_sec/stage_fx.rs` | G7 | Pure stage-space geometry for the weather + FX particle layers — draws exactly what exports render |
| 4 | `src/rune_sec/stage_camera.rs` | G7, G13 | Camera rotation applied to the stage transform + `guides_geometry` for the guides overlay |
| 5 | `src/rune_sec/stage_text.rs` | G7 | Stage-space geometry for text/media labels the rasterizer would draw |
| 6 | `src/rune_sec/timeline_marks.rs` | G8 | Drop-in replacement for chrome `timeline_row`: same chips/keys plus loop-range, keyframe and camera-key marks |
| 7 | `src/editor.rs` (new ops) | G5, G6 | Direct-set ops (taper/cap/gradient), `delete_stroke_at`, and the other single-step ops panels previously composed by hand |
| 8 | `src/audio_ops.rs` (new lib module) | G1 | `EditorCore` audio ops: fade, reverse, per-clip gain, clip delete/move — snapshot, repointing and status handled in the engine |
| 9 | `src/fx_ops.rs` (new lib module) | G2 | `EditorCore` FX-clip op family: move, resize, duplicate, delete, colours, anchor — the undo snapshot lives here, not in the panel |
| 10 | `src/figure_ops.rs` (new lib module) | G3 | Figure colour-set and duplicate promoted from shell helpers into the engine |
| 11 | `src/graph_keys.rs` (new lib module) | G4 | Key clipboard: `copy_keys` hands back an owned `Vec<Key>`; `paste_keys` rebases onto a target frame/channel |
| 12 | `src/rune_sec/graph_panel.rs` (upgrade) *(brief)* | G4 | Copy/paste-keys controls dispatching to `graph_keys` via the shell's `key_clipboard` state |
| 13 | `src/rune_sec/inspector_style.rs` (upgrade) | G5 | Style chips call the section-7 direct setters instead of cycling |
| 14 | `src/rune_sec/stroke_inspector.rs` (upgrade) | G6 | Per-stroke rows gain delete, dispatching to `delete_stroke_at` |
| 15 | `src/rune_sec/audio_panel.rs` (upgrade) | G1 | The buttons Phase D had to omit (fade/reverse/gain/delete) now exist, dispatching to `audio_ops` |
| 16 | `src/rune_sec/fx_clips.rs` (upgrade) | G2 | Handlers dispatch to `fx_ops`; the file's inline snapshot code is deleted |
| 17 | `src/rune_sec/inspector_figure.rs` (upgrade) | G3 | Colour/duplicate dispatch to `figure_ops`; shell-local helpers deleted |
| 18 | `src/rune_sec/keyframe_tools.rs` (upgrade) | G4 | Key copy/paste surface over the same clipboard |
| 19 | `src/rune_sec/easing_panel.rs` (upgrade) | G16 | Project easing set directly via `EditorCore::set_project_easing` |
| 20 | `src/rune_sec/playback_controls.rs` (upgrade) *(brief)* | G12, G15 | Play starts the audio mix; timecode unified with the status strip |
| 21 | `src/rune_sec/settings_popover.rs` (upgrade) | G13 | Guides toggle exists now that section 4 provides the geometry and the shell a `guides` field |
| 22 | `src/rune_sec/status_strip.rs` (upgrade) | G15 | Single timecode implementation shared with playback controls |
| 23 | `src/rune_sec/help_panel.rs` (upgrade) | G14 | Shortcut rows restored for every binding the shell now actually wires, incl. `Alt+1..8` |
| 24 | `src/rune_sec/menus.rs` (upgrade) | G9, G10 | View menu gains workspace + panel-visibility entries; duplicate entries pruned |
| 25 | `src/rune_sec/command_palette.rs` (upgrade) | G1–G6 | Five new commands over the new engine ops (all Phase E keys resolve — asserted in-file) |
| 26 | `src/rune_sec/export_panel.rs` (upgrade) *(brief)* | G17 | Export recap: after an export the panel shows what was written (op, path, frames) from `export_recap` state |
| 27 | i18n pass — `src/i18n.rs` + shell wiring *(brief)* | G11 | The 8 missing shell keys added to the table; chrome/panel labels routed through `i18n_wire::tr` |
| 28 | `src/rune_sec/scenes.rs` (upgrade) *(brief)* | G9 | One scene per workspace for `--tree`/`--snap` checks of the gated layout |
| 29 | `src/rune_sec/figures_panel.rs` (completion) | G10 | The panel becomes the single home of `add-figure`; the app-bar duplicate is removed at integration |
| 30 | `src/rune_sec/onion_controls.rs` (completion) *(brief)* | G10 | The single home of the onion toggle; the app-bar duplicate is removed at integration |
| 31–39 | Remaining panel/section completions per brief *(brief)* | — | Per-section gap closures in the same pattern (engine op first, panel dispatch second, duplicate removed third); see each file's header as it lands |
| 40 | `docs/PHASE_E.md` + `docs/QA_RUNE.md` addendum + inventory re-run | — | This document |

## 4. Integration checklist (for the integrator)

State fields the sections expect on `State` (main.rs) — add all
of them in one pass, with the inits shown:

- [ ] `workspace` — current workspace id (workspaces.rs §1);
      drives the `view()` gating table
- [ ] `guides: bool` — guides overlay on/off (stage_camera §4,
      settings §21)
- [ ] `show_left: bool` / `show_right: bool` — panel-column
      visibility (menus §24 View entries)
- [ ] `key_clipboard` — graph key clipboard storage
      (graph_keys §11; graph_panel §12, keyframe_tools §18)
- [ ] `export_recap` — last-export record for the export panel
      (§26)

New library modules — add the `pub mod` lines to `src/lib.rs`
(the files exist; at snapshot time lib.rs did **not** declare
them yet):

- [ ] `pub mod audio_ops;`
- [ ] `pub mod fx_ops;`
- [ ] `pub mod figure_ops;`
- [ ] `pub mod graph_keys;`

Engine ops the sections dispatch to (verify each exists before
wiring the panel that calls it):

- [ ] `editor.rs` §7 ops: direct taper/cap/gradient setters,
      `delete_stroke_at`, `set_project_easing`
- [ ] `audio_ops`: fade / reverse / gain / delete / move on
      `EditorCore`
- [ ] `fx_ops`: move / resize / duplicate / delete / colours /
      anchor on `EditorCore`
- [ ] `figure_ops`: set colour, duplicate figure
- [ ] `graph_keys::copy_keys` / `paste_keys`

Shell wiring in `main.rs` / `stage_view.rs`:

- [ ] `stage_view` composes the stage layers in order:
      `stage_fx` (weather/FX particles) → figures →
      `stage_camera` (rotation + guides overlay) →
      `stage_text` (labels)
- [ ] Replace chrome `timeline_row` with
      `timeline_marks::…` (§6) — same chip keys, marks added
- [ ] `on_key`: `Alt+1..8` switch `State.workspace` (§1, §23);
      keep the palette-first and text-focus guards ahead of it
- [ ] `view()`: gate the left/right columns by `workspace`
      (workspaces.rs table) and by `show_left` / `show_right`
- [ ] Remove the duplicated app-bar `add-figure` and `onion`
      controls (single homes: §29, §30) — keep the section keys
- [ ] Playback: start the audio mix when play starts (§20)
- [ ] `AppSpec.scenes`: register the per-workspace scenes (§28)

Post-integration verification:

- [ ] `python3 scripts/rune_inventory.py` — count must be
      ≥ Phase D's 362 minus the two deliberate duplicate
      removals, plus Phase E's new keys
- [ ] Push; read the **Build, Test & Binary** run; fix from CI
      logs only (repo rule: no local cargo)
- [ ] Fill the CI column of the QA_RUNE.md Phase E addendum and
      §4 of this file's repo record with the green run id

## 5. Honest limits

- **"Perfect" = all known gaps closed + CI green.** It is not a
  claim about looks. No Rune build has been screenshot-verified
  on a real screen; the user's download-and-screenshot check is
  still the only visual verdict, and it is still outstanding.
- **`--snap` on GPU-less CI remains unproven** (needs a software
  Vulkan adapter). The `Runtime` interaction tests and `--tree`
  dumps are the headless verification that exists; pixel checks
  wait on an adapter or a real machine.
- **IME on Linux** is the framework authors' own named top risk
  and is still unexercised in a real field.
- **Rune is pre-release** and pinned at rev `87caf2a`; nothing
  in Phase E changes the pin or the upgrade procedure in
  `docs/RUNE_MIGRATION.md` §8.
- Sections 31–39 are recorded from the wave brief, not from
  landed headers, in this snapshot; their rows should be
  corrected against the files at integration time.

## 6. Inventory snapshot

`python3 scripts/rune_inventory.py`, last run 2026-11-08
07:21 UTC (mid-wave, files still landing — the count rose
from 387 to 395 within the final minute of writing this
file):

- **395 unique keys** (338 static + 57 dynamic patterns),
  vs 362 at Phase D green.

The count will move again before integration; the integrator's
post-wiring run is the number of record.
