#!/usr/bin/env python3
"""Phase J+K+L+M pre-commit gate for StykAnimate-RS (integrator checks).

Run from the repo root:

    python3 scripts/phase_j_checks.py [base-ref]

Exit 0 = every check passes; exit 1 = failures are listed.
The script is READ-ONLY: it reads source files and runs only
read-only git commands (`git diff`, `git show`). It writes
nothing. Stdlib only.

What each check guards (and the Phase I failure behind it):

1. INVENTORY — the Rune element-key census (static / dynamic
   / total) via scripts/rune_inventory.py's own logic.
   Guards against silently losing UI element keys between
   phases: Phase H's count first *looked* like a drop
   (556 -> 480) only because the counter missed a new
   button-helper family; the real diff is keys, not vibes.

2. KEY DIFF — element keys present in the base ref but gone
   from the working tree, per changed src file. Guards the
   preservation rule every phase since the redesign has
   carried: no element key is ever lost without a decision.
   The optional base ref (default "HEAD") exists because
   the right comparison depends on when the integrator
   runs this: pre-commit against HEAD, or against HEAD~1
   once a wave commit exists.

3. STATE LITERAL CENSUS — every full `State { ... }`
   literal in src/ (test modules + main.rs `init`) must
   name every field of the current `State` struct, parsed
   live from src/main.rs. This is the Phase I section 23
   failure: the pose-library filter added
   `State::pose_library`, and the wiring of the new field
   into main.rs + all 18 full State literals was nearly
   missed at integration — the tree only compiled because
   the integrator caught a late detailed report. This
   census catches a missing field BEFORE CI does, and
   names the exact literals that lack it.

4. CALLER CENSUS — each Phase J+K+L+M lib op must have
   at least one production (non-test, non-defining-file)
   call site. Two entries are helpers whose only
   production caller lives in their own file; they are
   censused under a same-file rule (see SAME_FILE_OPS).
   This is the Phase I section 8 failure: the contact-
   sheet range export pair landed in export.rs with NO
   caller anywhere — finished-looking work that no user
   could ever reach. An op with no caller is unfinished
   work, and this check says so in those words (UNWIRED).

5. COUNT CLAIMS — the numbers the phase docs will print,
   read from source: palette COMMANDS entries, MCP
   registry tools, curated icons (ICON_NAMES), headless
   scenes. This is the Phase I docs failure family: a
   docs section reported files that were never written,
   and elsewhere a palette count was claimed as 98/100
   when the table held 97. Docs must match these numbers
   or the mismatch must be explained in the docs.

Phase K additions (same gate, wider net — the failure
modes above did not change, only the surface did):

- The caller census now also watches the Phase K lib
  ops (PHASE_K_OPS below), AND one Phase J op it never
  watched: `duplicate_channel_to_figure` landed in
  Phase J (graph_keys.rs) but was missing from the
  Phase J list, so its wiring was never verified by
  this gate. It is in the census now. Two Phase K
  entries are expected to read UNWIRED until the
  integrator wires them at integration
  (`audio_duplicate_clip_at` — the audio_panel button;
  `note_playback_speed` — the main.rs poll call): the
  flag mid-phase is the gate working, not a script bug,
  and neither is special-cased.
- The count claims stay read-from-source (no expected
  number is hard-coded anywhere in this script); the
  Phase K expectations the docs must match are palette
  103, MCP registry 84, icons 110, scenes 8. The scene
  count comes from the same live extraction of
  scenes.rs, so it reports 8 as soon as the two new
  scenes land.
- The State census covers Phase K's new
  `playback_speed` field automatically: the field list
  is parsed live from src/main.rs, so any field added
  to the struct is censused against every literal with
  no script change.

Phase L additions (same gate, wider net again):

- The caller census now also watches the Phase L lib
  ops (PHASE_L_OPS below), each with its caller
  verified in the tree at gate-writing time:
  `set_head_ring_scale` (editor.rs; the
  inspector_figure fig-ring steppers), `clip_duration_secs`
  (audio_ops.rs; audio_panel's clip_readout),
  `simplify_stroke_at` (draw_ops.rs; stroke_inspector),
  `set_key_interp` (graph_keys.rs; keyframe_tools).
  Unlike Phase K, NO Phase L entry is wired by the
  integrator: every section wires its own panel, so
  there are no by-design UNWIRED entries this phase —
  an UNWIRED Phase L line on a mid-phase tree simply
  means that op's caller section has not landed yet,
  and the flag is the gate working.
- The camera panel's key-delete fn is censused
  separately (SAME_FILE_OPS below):
  `delete_camera_key_at` (section 39, camera_panel.rs)
  is defined in the panel file and its production
  caller is the panel's key-row builder in the SAME
  file, which the cross-file rule above would report
  as UNWIRED forever. The same-file rule counts call
  sites in the defining file's production region
  (before its trailing `mod tests`, the definition
  site itself excluded).
- The count claims stay read-from-source; the Phase L
  numbers the docs must match are whatever this gate
  prints. On the final Phase L tree they read: scenes
  10 (two over Phase K's 8), MCP registry 87 (three
  over 84), curated icons 111 (one over 110), palette
  103 (unchanged).
- Phase L adds no State fields (Phase K's
  `playback_speed` was the last), so the State census
  needs no change: the live parse covers the struct
  exactly as it stands.

Phase M additions (same gate, wider net again):

- The caller census now also watches the Phase M lib
  ops (PHASE_M_OPS below). Caller files verified at
  gate-writing time: `swap_frame_with_neighbour`
  (frame_swap.rs; frame_ops.rs buttons — palette and
  menus entries are further callers as they land),
  `audio_set_muted_at` (audio_ops.rs; mcp_tools.rs,
  with the audio_panel caller as it lands),
  `shift_keys` (graph_keys.rs; graph_panel.rs),
  `fill_keys_in_range` (graph_keys.rs;
  keyframe_tools.rs), `stage_fig3_prims`
  (rune_sec/stage_fig3.rs; stage_view.rs). One entry
  is expected to read UNWIRED on a mid-phase tree:
  `frame_camera` (fig3d.rs) — its only production
  caller is three_d_panel's frame action, which had
  not landed when this gate was widened; the flag is
  the gate working, not a script bug.
- `swap_frames` (frame_swap.rs) is censused under
  the same-file rule (SAME_FILE_OPS below): its only
  production caller is `swap_frame_with_neighbour`
  in the same file, so the cross-file rule would
  report it UNWIRED forever even though every user
  path reaches it through the neighbour op — which
  is itself censused cross-file above.
- The count claims stay read-from-source (still no
  expected number hard-coded anywhere in this
  script); the Phase M numbers the docs must match
  are whatever this gate prints on the final tree.
  The expectations at gate-writing time are palette
  108 (five over Phase L's 103), MCP registry 92
  (five over 87; total 103 with the built-ins),
  curated icons 111 (unchanged), scenes 11 (one
  over 10) — mid-phase runs print lower palette /
  registry numbers until those sections land, which
  is the live read doing its job. (Integrator's
  correction at Phase M integration: the registry
  expectation was wrong — `fx_set_color2` already
  existed, so only four tools were added: registry
  91, total 102. The gate's own live read printed
  91 throughout.)
- The State census needs no change either way: the
  field list is parsed live from src/main.rs, so
  any field Phase M adds is censused against every
  literal automatically.

Phase N additions (same gate, new check 6):

- Check 6 is the DESIGN ADOPTION census (the P17
  verification plan section 5 / P20 section 2.2
  design): the Phase N restyle retires every panel's
  private layout/button dialect onto one design kit
  (src/rune_sec/design.rs) and one button family, and
  this check proves the adoption actually happened in
  source — the restyle-wave analogue of check 4's
  UNWIRED. It has four parts, all read from source:
  (a) a kit self-check — design.rs must define every
  charter-fixed constant (name AND value) and fn,
  with amendment extras reported, not required;
  (b) a per-file adoption census over the 31 panel
  files of the adoption commit a570d60 — the floor
  is >= 1 production-region `design::` reference per
  file (a file at zero is UNADOPTED), plus the
  panel_col roll: every adopted root is a
  design::panel_col card except files named in
  PANEL_COL_EXCEPTIONS with their charter reason;
  (c) a consumer census (report only — a kit fn with
  no external consumer mid-phase is the Phase K
  UNWIRED precedent, information for the docs, not
  a failure); (d) the retired-helper census — local
  definitions of the retired dialects (fbtn, kbtn,
  pbtn, pibtn, ctl, ictl, row_toggle, cap_chip,
  preset_chip, danger_btn, and the private
  section/section_label/section_header headings)
  must be zero across src/rune_sec/ outside the
  family homes, with two honest classifications
  that are NOT survivors: a definition whose body
  only delegates to the kit/family (the name kept
  as a call-site adapter — inspector_style's
  cap_chip/preset_chip, fx_browser's section) and
  entries in RETIRED_EXEMPTIONS with a named
  reason (graph_panel's preset_chip, a
  canvas-content control chip_btn cannot express).
  The foundation surfaces that predate the per-file
  floor (the family homes chrome_view.rs /
  icon_btn.rs, the pure-canvas stage_view.rs, and
  the F-wave overlays) are printed as a report-only
  census: several legitimately hold zero `design::`
  references because they ARE the family, mirror
  its tokens, or draw no chrome at all.
- Checks 1-5 are untouched by Phase N: the key
  censuses already knew the kit helpers (the P20
  section 2.1 regex repair landed with the
  foundation), the State census must read IDENTICAL
  to Phase M (43 fields — Phase N adds none), and
  the caller census guards the same ops.

UI mission Wave 9 additions (same gate, check 7):

- Check 7 is the LITERAL & EMPTY-STATE GUARD. (a) A
  census of production hex colour literals in
  src/rune_sec/ — both `0xRRGGBB` integer literals
  (underscore separators normalised) and "#RRGGBB"
  string literals — against the UI mission Wave 7
  colour census classification: the lawful sources
  are the token files (app_theme.rs, the single
  palette constructor; design.rs, the design-token
  file), and every other literal must fall inside a
  named exception in LITERAL_CONST_EXCEPTIONS /
  LITERAL_LINE_EXCEPTIONS below (colour-picker /
  palette DATA, preset DATA, project-content
  defaults, stage/canvas content colours,
  Fig3Palette::LEGACY, 24-bit channel masks). A
  literal outside every exception is UNCLASSIFIED
  and fails: new colour literals must either move
  into the token files or earn a named exception
  with a classification, never land silently.
  snap_tests.rs is excluded by name: main.rs
  declares it under #[cfg(test)], so the whole file
  is test code, not production. (The Wave 7 chrome
  danger-fill exception lives in src/chrome_view.rs,
  outside this check's src/rune_sec/ scope.)
  (b) Bare `text("No …")` / `text("Nothing…")`
  empty lines in src/rune_sec/ production code must
  stay ZERO — the Wave 6 sweep converted every one
  to the kit `design::empty_state`; there are no
  exceptions to this half.

Wave 10 additions (same gate, check 8):

- Check 8 is COVERAGE COMPLETENESS: the scene /
  headless cross-reference, in both directions,
  plus the layout-fact sequence. (a) Every scene
  registered in scenes.rs's `scenes()` must appear
  in at least one of the `for scene in …` tree
  loops in .github/workflows/headless.yml — the
  Wave 9 lesson: `export-open` / `fx-selected`
  existed as scenes for months but were in no loop,
  so their panels were never tree-dumped and an
  overflowing Export panel stayed invisible. (The
  per-loop split is printed report-only: the second
  loop is the xvfb fallback, which only retries
  scenes the plain attempt missed, so its list is
  a retry subset by design — the plain loop is the
  coverage contract.) (b) The layout facts
  documented above `layout_assertions()` in
  scenes.rs must be numbered 1..=N with no gaps or
  duplicates, `layout_assertions()` must hold
  exactly N strings (the vec is positional: string
  i IS fact i), and a string's `see fact N`
  citation, where present, must name its own
  position. (c) The reverse direction: every scene
  name headless.yml uses — tree-loop lists and
  literal `--scene NAME` probes alike — must be
  registered in `scenes()`; a stale name in the
  YAML makes the probe step produce nothing for
  that scene and say nothing about it. A failure
  under (a) on a mid-wave tree is the gate working
  (the Phase K UNWIRED precedent): the integrator
  resolves it by adding the scene to the loops in
  headless.yml, never by unregistering the scene
  or weakening this check.

Wave 11 additions (same gate, check 9):

- Check 9 is the MCP SURFACE CENSUS & INTEGRITY: the
  MCP tool surface can never silently shrink, shadow
  or drift from its docs again. All three tool
  families are extracted from source, in all_tools()
  assembly order: the built-ins (the `fn tool_*() ->
  Tool` constructors mcp.rs's `all_tools()` assembles,
  each of which must define exactly one tool name,
  with the defined and assembled constructor sets
  required to be equal and the two family `extend`
  calls required to be present), the editing registry
  (the `Tool { name:, handler: }` literals of
  mcp_tools.rs's `tools()`) and the IO tools (the
  same literal shape in mcp_io.rs's `tools()`). The
  check then proves: every registry/IO handler ident
  resolves to exactly one `fn` definition across the
  whole src/ production tree (handlers may live in
  domain files, not just their family's file); every
  tool name is unique across ALL families — dispatch
  (`tools_call_result`) takes the FIRST match in
  all_tools() order, so a cross-family collision
  makes the later tool unreachable while tools/list
  shows the name twice; and docs/MCP.md's catalogue
  lists exactly the surface set, in both directions,
  by name. Every extractor fails LOUDLY when its
  declaration pattern is not found — a reshaped
  registry must read as a broken census, never as
  a silent zero.

Wave 12 additions (same gate, NO new check — an audit
wave for the gate itself):

- The Wave 12 deltas are all absorbed by the existing
  live derivations; no check logic changed, and no
  Wave 11 number was hard-coded anywhere to update
  (audited: the only numerals in this file are the
  historical phase narratives above, check 6's design
  charter constants, and structural rules like check
  7's 6/8-digit hex test). (a) Scenes 24 -> 26:
  check 5 counts and check 8 cross-references the
  scenes() vec live, so home-populated and
  strokes-populated need no gate edit. D4's
  headless.yml loops landed first, so mid-wave
  check 8 fails in the REVERSE direction (the loops
  name scenes scenes() does not register yet) until
  D2 registers them; both directions go green
  together on the integrated tree, and a scene
  registered without loop coverage still fails in
  the forward direction. (b) The asset tools:
  check 9's handler census scans the production
  region of EVERY src/**/*.rs file — there is no
  file list that could omit a domain file — so
  B1's handlers in src/media.rs (tool_assets_list,
  tool_asset_info, tool_asset_import,
  tool_asset_rename, tool_asset_remove,
  tool_asset_attach; each verified defined exactly
  once, in media.rs's production region ahead of
  its trailing `mod tests`) resolve the moment the
  integrator splices their Tool literals into
  mcp_tools::tools() and adds their docs/MCP.md
  catalogue rows. (c) Palette and help: check 5's
  palette count is a live read of
  command_palette.rs — it printed 129 mid-wave, not
  Wave 11's 114, the moment the new COMMANDS entries
  landed, the derivation proving itself. Help rows
  are NOT censused by this gate and never have been:
  no expected help count exists anywhere in this
  script, so B9's 58 -> 61 delta can neither pass
  nor fail a check; that number lives in the docs
  only.
- Check 9 self-test (E1, on a disposable copy of the
  tree — the real tree was never planted): a
  `Tool { name: "assets_list", handler:
  media::tool_assets_list }` literal spliced into
  the copy's mcp_tools::tools() plus its catalogue
  row read registry 225 / surface 278 / handler
  integrity 234 with doc sync exact (PASS); deleting
  just the catalogue row then failed check 9 with
  "tool `assets_list` (registry family) is
  registered but missing from docs/MCP.md's
  catalogue — existing-but-undocumented". The copy
  was deleted afterwards.

Wave 13 additions (same gate, NO new check — upkeep
only, following the Wave 12 pattern):

- The Wave 13 deltas are likewise absorbed by the
  existing live derivations; no check logic changed
  and none of the expected numbers in this narrative
  is read by a check. (a) Sprite tools: A4's five
  handlers in src/media.rs enter the editing registry
  when the integrator splices their Tool literals
  into mcp_tools::tools() and adds their docs/MCP.md
  catalogue rows — the post-splice expectation is
  registry 230 -> 235. Check 9's handler census scans
  every src/**/*.rs production region, so a handler
  in a domain file resolves exactly as the Wave 12
  asset handlers did, and doc sync remains a live
  two-direction set comparison. (b) Scenes 26 -> 28:
  check 5 counts and check 8 cross-references the
  scenes() vec live, so library-populated and
  sprites-populated need no gate edit. D2's
  headless.yml loops landed before D1 registered
  the scenes, so a mid-wave check 8 failure in the
  REVERSE direction (the loops and --scene probes
  name scenes scenes() does not register yet) is
  the gate working; both directions go green
  together once D1 lands, and a scene registered
  without loop coverage still fails in the forward
  direction. (c) Palette 132 -> 134: check 5's count
  is a live read of command_palette.rs and follows
  E3's sprite commands the moment they land (E3
  shipped two, not three: `next-sprite` was dropped
  in its audit — no sprite selection existed for it
  to advance — so the wave's palette total is 134).
- The State census needs no change for the Sprites
  panel's new State field, and this was verified
  against the code rather than assumed: check 3 does
  NOT pin the entering 45 fields anywhere —
  state_fields() parses the field list live from
  src/main.rs's State struct and the census line
  prints len(fields) — so the field the integrator
  adds for A3's panel state is censused against
  every full State literal automatically, exactly
  as Phase K's playback_speed field was.
- Help rows stay uncensused, as they have always
  been: A8's Sprites/Project-assets rewrites keep
  the 61-row count in the docs only, and no expected
  help count exists anywhere in this script.
- Check 7 gains NO pre-registered exception this
  wave. The roster's D1 brief names the two new
  scenes but pins no exact hex literal for them, so
  there is nothing truthful to pin in advance: the
  Wave 12 scenes.rs line exception already covers
  the exact fixture literals it names if a new scene
  reuses them, and any genuinely new scenes.rs
  colour literal must earn its own named exception —
  with its exact literal — once the landed source
  shows it, never by guessing from a brief.

Wave 14 additions (same gate, check 10):

- Check 10 is PARITY: every command in the command
  palette must be accounted for against the MCP
  tool surface — covered by named tools that exist
  (PARITY_MAP), pending on a named tool a Wave 14
  squad has not spliced yet (PARITY_PENDING), or
  excluded with an argued reason (PARITY_EXCLUSIONS).
  The command set is derived live from
  command_palette.rs and the surface from check 9's
  own family extractors, so the two checks can
  never disagree about what the surface is. The
  tables are the accounting, not the semantics: a
  mapping asserts the named tools exist and are the
  ones A10's ledger (docs/wave14/PALETTE_PARITY.md)
  pairs with the command; it does not prove the
  tool's behaviour matches the command's, which is
  what the handlers' own tests are for. PENDING is
  a failure by design on a pre-integration tree
  (the Phase K UNWIRED precedent): the check going
  red before Squad B's shell tools and the last
  A-squad handlers are spliced, and green after, is
  the proof the check works. A pending entry whose
  tool has landed is reported RESOLVED and passes;
  the integrator moves it into PARITY_MAP at
  close-out so the pending table only ever names
  tools that do not exist yet. Table hygiene is
  enforced both ways: an entry naming a command
  the palette no longer has, a command in two
  tables, or a mapped tool that has left the
  surface all fail — parity can shrink only loudly.
  Seeding note: A1's PARITY_CENSUS.md and A10's
  PALETTE_PARITY.md had not landed when this check
  was written, so the tables were seeded from the
  palette shims' own bodies (each `cmd_*` shim names
  the core op it drives) and the handlers' source;
  the integrator reconciles them against A10's
  ledger at integration, and any disagreement is a
  finding, not a silent edit.

Focused Wave (post-Wave 14) note: that wave closed
census gaps that were never palette rows — G5 (the
library panel's per-row rename, a panel action with
no palette command) closed by the core tool
`figure_library_rename`; G6 (graph channel
selection), G7 (the style inspector's
recent-colours list) and the timeline-collapse
preference are shell state, likewise command-less,
closed by the shell tools
`shell_select_graph_channel`,
`shell_get_recent_colours` (read-only — G7's argued
outcome) and `shell_set_timeline`. The palette gained
no commands and no command changed coverage, so
the three tables below deliberately do not move:
a row keyed by anything but a real palette command
fails this check's own hygiene, and inventing one
would fabricate coverage. Those tools join the
surface this check derives live from check 9's
extractors; their accounting home is the
PARITY_CENSUS close-out addendum and check 9's
two-direction documentation sync.

Wave Q additions (same gate, checks 1/5/9/10 widened):

- Wave P landed the subtitle + sprite MCP modules and
  Wave Q wires them (Q1). Check 9's extractors — and
  check 10's surface derivation, which reuses them —
  now read TWO further families in all_tools()
  assembly order: subtitle io (the `Tool` literals of
  src/mcp_subtitle_io.rs's tools(), 4) and subtitle
  tools (src/mcp_subtitle_tools.rs's tools(), 5),
  each with the registry family's own verification.
  The two sprite handlers (src/mcp_sprite_tools.rs)
  take the domain-file path instead: P38 IA-2 splices
  their Tool literals into mcp_tools::tools(), so the
  registry reads 319 and their handlers resolve by
  leaf name like every earlier domain handler.
  scripts/mcp_census.py carries the same extension
  (P35 IA-2 is discharged in both scripts).
- Check 9 gains a surface FLOOR (MCP_SURFACE_FLOOR,
  ledgered at the constant): doc sync compares the
  surface to the doc, so a tool removed together with
  its catalogue row would pass silently; the floor
  pins the integrated truth — 381 = 44 built-ins +
  319 registry + 9 io + 4 subtitle io + 5 subtitle
  tools, recounted from the tree by Q5 on 2026-11-10.
  Until the sprite splice lands the surface reads 379
  and the floor line names the gap (the gate working,
  per the Phase K UNWIRED precedent).
- Check 5 prints the two new family counts beside the
  registry count, so the numbers the docs print are
  the gate's own reads end to end.
- Check 1 gains the key floor (KEY_TOTAL_FLOOR,
  ledgered at the constant): this gate's counter
  reads 940 total keys at HEAD 0b75051 (827 static +
  113 dynamic, 59 files) and 949 on the Wave Q tree
  (836 + 113, 60 files) — +9 static
  (home-recovery-discard, particles-seed-dn/up,
  subtitle-lane, the frames-* family in
  src/rune_sec/frame_ops.rs, step), zero dynamic
  change, zero lost (check 2 is the per-key proof).
  The "804 baseline" the Wave P board quotes does not
  reproduce under this counter — it reads 940 at the
  same HEAD — so the floor is the counter's own
  verified number, not the board's. Q7's subtitle
  panel had not landed at recount time; its keys can
  only raise the total above the floor.

Wave R note (R15, 2026-11-10): baseline recount on the
settled tree (HEAD 911cf77, clean) reads 979 total
keys (863 static + 116 dynamic, 61 files), 381 MCP
tools, 48 State fields / 22 full literals, 31 headless
scenes, 134 palette commands — gate PASS 10/10, census
16/16 PASS. KEY_TOTAL_FLOOR ratcheted 949 -> 979 at
the constant (ledger there). MCP_SURFACE_FLOOR stays
381 (== the recounted total). R15 re-runs both scripts
as Wave R slices land and ratchets only to verified
recounts; R15's findings carry the per-slice deltas.

Wave S note (S15, 2026-11-10): baseline recount on the
clean base (HEAD 85898b4, detached worktree) reads 980
total keys (864 static + 116 dynamic, 61 files), 381
MCP tools, 48 State fields / 22 full literals, 31
headless scenes, 134 palette commands, 163 icons —
gate PASS 10/10, census 16/16 PASS. The 980 is R15's
979 plus subtitle-speaker-field, added by Wave R's
integration after R15's recount; KEY_TOTAL_FLOOR
ratcheted 979 -> 980 at the constant (ledger there).
Final recount on the settled Wave S tree reads the
same 980 (no slice added or lost an element key), 381
tools, 32 headless scenes (S2's subtitles-speaker,
covered once S16's headless.yml IA landed), 134
palette commands — gate PASS 10/10, census 16/16 PASS.
The i18n catalogue (counted directly from i18n.rs,
not by this gate) reads 28 tables x 168 keys after
S1's export block (152 + 16, one contiguous block,
sequences identical, zero loss); IA-S14-1's 34
threed_* keys are integrator-side and not in the tree
— if applied, the catalogue becomes 202 and the
i18n.rs / i18n_wire.rs count lines must move with it
(IA-S15-1 in S15's findings).

Approximations are deliberate and documented inline:
block capture is brace counting over comment/string-
blanked source (the repo's established scan idiom), and
"test code" for the caller census means "at or after the
file's trailing `mod tests` declaration", which matches
this repo's test-modules-live-at-the-end layout (a bare
first-#[cfg(test)] cut was wrong: main.rs declares its
cfg(test) snap_tests module near the top).
"""

import glob
import os
import re
import subprocess
import sys

# ----------------------------------------------------------------
# Repo root (same convention as scripts/rune_inventory.py)
# ----------------------------------------------------------------


def repo_root() -> str:
    guess = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    if os.path.isfile(os.path.join(guess, "src", "main.rs")):
        return guess
    return os.getcwd()


ROOT = repo_root()

# ----------------------------------------------------------------
# Source blanking: replace comments / string / char literal
# contents with spaces (newlines and total length preserved),
# so brace counting and identifier scans never see punctuation
# that lives inside literals. The repo's established scan idiom.
# ----------------------------------------------------------------

_CHAR_LIT_RE = re.compile(r"'(\\.|[^'\\])'")


def blank_code(src: str) -> str:
    out = list(src)
    i, n = 0, len(src)
    while i < n:
        ch = src[i]
        two = src[i : i + 2]
        if two == "//":
            j = src.find("\n", i)
            j = n if j < 0 else j
            for k in range(i, j):
                out[k] = " "
            i = j
        elif two == "/*":
            depth = 1
            j = i + 2
            while j < n and depth:
                if src[j : j + 2] == "/*":
                    depth += 1
                    j += 2
                elif src[j : j + 2] == "*/":
                    depth -= 1
                    j += 2
                else:
                    j += 1
            for k in range(i, min(j, n)):
                if out[k] != "\n":
                    out[k] = " "
            i = j
        elif ch == '"':
            j = i + 1
            while j < n:
                if src[j] == "\\":
                    j += 2
                    continue
                if src[j] == '"':
                    j += 1
                    break
                j += 1
            for k in range(i, min(j, n)):
                if out[k] != "\n":
                    out[k] = " "
            i = j
        elif ch == "'":
            m = _CHAR_LIT_RE.match(src, i)
            if m:  # a real char literal, not a lifetime
                for k in range(i, m.end()):
                    out[k] = " "
                i = m.end()
            else:
                i += 1
        else:
            i += 1
    return "".join(out)


def read(rel: str) -> str:
    with open(os.path.join(ROOT, rel), encoding="utf-8") as fh:
        return fh.read()


def brace_block(blanked: str, open_idx: int) -> tuple[str, int]:
    """Given blanked source and the index of a '{', return
    (inner_text, index just past the matching '}')."""
    depth = 0
    i = open_idx
    while i < len(blanked):
        ch = blanked[i]
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                return blanked[open_idx + 1 : i], i + 1
        i += 1
    return blanked[open_idx + 1 :], len(blanked)


def bracket_block(blanked: str, open_idx: int) -> tuple[str, int]:
    depth = 0
    i = open_idx
    while i < len(blanked):
        ch = blanked[i]
        if ch == "[":
            depth += 1
        elif ch == "]":
            depth -= 1
            if depth == 0:
                return blanked[open_idx + 1 : i], i + 1
        i += 1
    return blanked[open_idx + 1 :], len(blanked)


def split_top_level(inner: str) -> list[str]:
    """Split a brace/bracket block's inner text at commas
    that sit at nesting depth 0 (all of {}, (), [] tracked),
    so each chunk is one entry."""
    chunks: list[str] = []
    depth = 0
    start = 0
    for i, ch in enumerate(inner):
        if ch in "{([":
            depth += 1
        elif ch in "})]":
            depth -= 1
        elif ch == "," and depth == 0:
            chunks.append(inner[start:i])
            start = i + 1
    tail = inner[start:]
    if tail.strip():
        chunks.append(tail)
    return chunks


_FIELD_START_RE = re.compile(r"^\s*(?:pub\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*(:|$)")


def entry_field(chunk: str) -> str | None:
    """The field an entry chunk assigns: `name: ...` (with
    an optional leading `pub`, for struct declarations), or
    the shorthand `name` alone (a whole chunk that is just
    an identifier). Returns None for anything else (`..`
    rest chunks are handled by the caller)."""
    m = _FIELD_START_RE.match(chunk)
    if not m:
        return None
    name, sep = m.group(1), m.group(2)
    if sep == ":":
        # Exclude path tails (`a::b`) and value-only chunks:
        # a field colon is a single ':' not followed by ':'.
        colon_idx = chunk.index(":", m.start(1))
        if chunk[colon_idx + 1 : colon_idx + 2] == ":":
            return None
        return name
    # Shorthand: the entire chunk must be just the identifier.
    if chunk.strip() == name:
        return name
    return None


# ----------------------------------------------------------------
# Check 1 — inventory (rune_inventory.py's logic)
# ----------------------------------------------------------------

# Wave Q key floor (Q5's recount, 2026-11-10): check 1's
# total may not fall below the verified Wave P+Q tree.
# This gate's own counter reads 940 total keys at HEAD
# 0b75051 (827 static + 113 dynamic, 59 files) and 949
# on the Wave Q tree (836 + 113, 60 files): +9 static,
# zero dynamic change, zero lost (check 2 proves the
# zero per key). The nine: home-recovery-discard (P42),
# particles-seed-dn / particles-seed-up (P37),
# subtitle-lane (P32), frames-arrange / frames-clipboard
# / frames-dur / frames-path (src/rune_sec/frame_ops.rs)
# and step. The "804 baseline" the Wave P board quotes
# does not reproduce under this counter — it reads 940
# at the same HEAD — so the floor is set to what this
# counter verifies, per Q5's brief. Q7's subtitle panel
# had not landed at recount time; its keys can only
# raise the total. Ratchet UP with a new ledger line
# when a wave's recount rises; never lower silently.
#
# Wave R ratchet (R15's baseline recount, 2026-11-10):
# on the settled Wave P+Q tree (HEAD 911cf77, clean)
# this counter reads 979 total (863 static + 116
# dynamic, 61 files) — Q5's predicted rise, realised
# once Q7's subtitle panel and the rest of the wave
# landed and were integrated. Floor raised 949 -> 979
# so Wave R starts from the integrated truth; any key
# loss in Wave R now fails check 1.
#
# Wave S ratchet (S15's recount, 2026-11-10): on the
# clean Wave R base (HEAD 85898b4) this counter reads
# 980 total (864 static + 116 dynamic, 61 files) —
# R15's 979 plus the one static key Wave R's
# integration itself added after R15's recount
# (subtitle-speaker-field, IA-R21-5). Floor raised
# 979 -> 980 so Wave S starts from the integrated
# truth. S15's final recount on the settled Wave S
# tree (HEAD 14bd9b3 + siblings' landings) reads the
# same 980: no Wave S slice added or lost an element
# key (each findings file says so, and check 2 proves
# zero loss per key), so the floor stands at 980.
# Wave V integration (F5): the recount on the final
# Wave V tree reads 983 (980 + V6's 3 dynamic subtitle
# stems), agreeing with V13/V20's measurement, so the
# floor is ratcheted 980 -> 983.
# Wave X ratchet (X16's baseline recount, 2026-11-10):
# on the clean Wave W integration HEAD (dccddbb,
# detached worktree) this counter reads 984 total
# (865 static + 119 dynamic, 61 files) — the 983 floor
# plus the one static key Wave W's integration counted
# (xc-progress, export_panel.rs 28 -> 29 by check 2).
# The other three W1 progress keys (xc-progress-working
# / -render / -encode) ride (key, line) tuple literals
# in job_lines and are NOT captured by this counter —
# F6's documented artefact; they exist and are pinned,
# they are simply outside the gate method, so the
# floor is set to what this counter verifies: 984.
# Floor raised 983 -> 984. Wave X's own key growth is
# PENDING arithmetic in x16.md, never floored here.
# Wave Y ratchet (Y16's recount, 2026-11-10): on the
# clean Wave X integration HEAD (d44098f, git archive
# scratch tree) this counter reads 995 total (871 static
# + 124 dynamic, 61 files) — the 984 floor plus Wave X's
# counted growth of 11 (zero lost proven by check 2 at
# Wave X integration). Tuple-carried keys remain outside
# this counter (F6's artefact, above). Floor raised
# 984 -> 995 so Wave Y starts from the integrated truth.
# Wave Z ratchet (Z13's baseline recount, 2026-11-10):
# on the clean Wave Y closing HEAD (5031700, detached
# git worktree) this counter reads 1017 total (889
# static + 128 dynamic, 61 files) — the 995 floor plus
# Wave Y's counted growth of 22 (zero lost proven by
# check 2 at Wave Y integration and again on this
# baseline). Tuple-carried keys remain outside this
# counter (F6's artefact, above). Floor raised
# 995 -> 1017 so Wave Z starts from the integrated truth.
KEY_TOTAL_FLOOR = 1017


def inventory_counts() -> tuple[int, int, int, int]:
    """(static, dynamic, total, files_scanned). Import the
    sibling script and reuse its regexes/file list; if the
    import fails, shell out and parse its summary lines."""
    scripts_dir = os.path.join(ROOT, "scripts")
    try:
        if scripts_dir not in sys.path:
            sys.path.insert(0, scripts_dir)
        import rune_inventory as inv

        files = [
            os.path.join(ROOT, "src", "main.rs"),
            os.path.join(ROOT, "src", "stage_view.rs"),
            os.path.join(ROOT, "src", "chrome_view.rs"),
        ]
        files += sorted(glob.glob(os.path.join(ROOT, "src", "rune_sec", "*.rs")))
        files = [f for f in files if os.path.isfile(f)]
        static_keys: set[str] = set()
        dynamic_keys: set[str] = set()
        for path in files:
            with open(path, encoding="utf-8") as fh:
                src = fh.read()
            static_keys |= set(inv.STATIC_KEY_RE.findall(src))
            static_keys |= set(inv.BTN_KEY_RE.findall(src))
            static_keys |= set(inv.ENTRY_KEY_RE.findall(src))
            static_keys |= set(inv.match_arm_keys(src))
            dynamic_keys |= set(inv.DYNAMIC_KEY_RE.findall(src))
            dynamic_keys |= set(inv.BTN_DYN_RE.findall(src))
            dynamic_keys |= set(inv.ENTRY_DYN_RE.findall(src))
        return len(static_keys), len(dynamic_keys), len(static_keys | dynamic_keys), len(files)
    except Exception:
        proc = subprocess.run(
            [sys.executable, os.path.join(scripts_dir, "rune_inventory.py")],
            capture_output=True,
            text=True,
            cwd=ROOT,
        )
        def grab(label: str) -> int:
            m = re.search(rf"{label}:\s+(\d+)", proc.stdout)
            return int(m.group(1)) if m else 0

        return (
            grab("Static keys"),
            grab("Dynamic patterns"),
            grab("Total unique keys"),
            proc.stdout.count("\n  src/"),
        )


# ----------------------------------------------------------------
# Check 2 — key diff vs a base ref
# ----------------------------------------------------------------

_DIFF_BTN_RE = re.compile(
    r"\b(?:btn|fbtn|icon_btn|icon_only_btn|row_toggle|icon_btn_inert|icon_only_btn_inert"
    r"|btn_inert|btn_muted_live|danger_btn|danger_icon_btn|chip_btn"
    r"|ctl|ictl|kbtn|pbtn|pibtn"
    # Wave 9: step_btn/edit_btn/v_splitter forward the key
    # verbatim as arg 1 (see rune_inventory._KEY_HELPERS).
    r"|step_btn|edit_btn|v_splitter"
    # Wave 10 (E3 + integration): mirror the inventory's
    # helper census — chip_btn_inert, tangent_preset_chip,
    # weather_slider, separator, item, group, plain_row,
    # range_row, bundle_row, stamp_step_btn, zorder_btn
    # all take the key as arg 1 and apply it verbatim
    # (verified at source; see rune_inventory._KEY_HELPERS).
    r"|chip_btn_inert|tangent_preset_chip|weather_slider"
    r"|separator|item|group"
    r"|plain_row|range_row|bundle_row"
    r"|stamp_step_btn|zorder_btn"
    r"|panel_col|list_row|toggle|toggle_row|header_row|divider_keyed|swatch)"
    r"\(\s*\"([^\"]+)\""
)
_DIFF_KEY_RE = re.compile(r"\.key\(\s*\"([^\"]+)\"")
# Keys carried by a named constant (Phase J §9: `key-delete`
# and `keyframe-actions` moved into `pub const …_KEY: &str`
# values, so the call sites name the constant, not the
# literal). The constant's value IS the key; count it on
# both sides of the diff.
_DIFF_CONST_KEY_RE = re.compile(r"const\s+\w+_KEY\s*:\s*&str\s*=\s*\"([^\"]+)\"")


def extract_keys(text: str) -> set[str]:
    return (
        set(_DIFF_BTN_RE.findall(text))
        | set(_DIFF_KEY_RE.findall(text))
        | set(_DIFF_CONST_KEY_RE.findall(text))
    )


def run_git(args: list[str]) -> tuple[int, str]:
    proc = subprocess.run(
        ["git", *args], capture_output=True, text=True, cwd=ROOT
    )
    return proc.returncode, proc.stdout


def check_key_diff(base: str) -> tuple[bool, list[str]]:
    lines: list[str] = []
    rc, out = run_git(["diff", "--name-only", base, "--", "src"])
    if rc != 0:
        return False, [f"git diff against {base!r} failed — is it a valid ref?"]
    changed = [
        p for p in out.splitlines() if p.startswith("src/") and p.endswith(".rs")
    ]
    if not changed:
        lines.append(f"no src files changed vs {base} — nothing to diff")
        return True, lines
    lost_total: list[str] = []
    for rel in changed:
        rc, base_text = run_git(["show", f"{base}:{rel}"])
        base_keys = extract_keys(base_text) if rc == 0 else set()
        full = os.path.join(ROOT, rel)
        tree_keys = extract_keys(open(full, encoding="utf-8").read()) if os.path.isfile(full) else set()
        lost = sorted(base_keys - tree_keys)
        if lost:
            lost_total.extend(f"{rel}: {k}" for k in lost)
            lines.append(f"{rel}: LOST {len(lost)}: {', '.join(lost)}")
        else:
            lines.append(f"{rel}: no keys lost ({len(base_keys)} in base, {len(tree_keys)} in tree)")
    if lost_total:
        lines.append(f"TOTAL LOST: {len(lost_total)} key(s)")
        return False, lines
    return True, lines


# ----------------------------------------------------------------
# Check 3 — State literal census
# ----------------------------------------------------------------

_STATE_LIT_RE = re.compile(r"(?<![A-Za-z0-9_])State\s*\{")


def state_fields() -> list[str]:
    blanked = blank_code(read("src/main.rs"))
    m = re.search(r"struct\s+State\s*\{", blanked)
    if not m:
        return []
    inner, _ = brace_block(blanked, blanked.index("{", m.start()))
    fields: list[str] = []
    for chunk in split_top_level(inner):
        name = entry_field(chunk)
        if name and name not in fields:
            fields.append(name)
    return fields


def find_state_literals() -> list[tuple[str, int, str]]:
    """(relpath, line, inner_text) for every full `State {`
    literal in src/**/*.rs, skipping the struct definition,
    enum declarations, and `-> State {` fn signatures (whose
    brace opens a fn body, not a literal)."""
    found: list[tuple[str, int, str]] = []
    paths = sorted(glob.glob(os.path.join(ROOT, "src", "**", "*.rs"), recursive=True))
    for full in paths:
        rel = os.path.relpath(full, ROOT)
        with open(full, encoding="utf-8") as fh:
            blanked = blank_code(fh.read())
        for m in _STATE_LIT_RE.finditer(blanked):
            line_start = blanked.rfind("\n", 0, m.start()) + 1
            line = blanked[line_start : blanked.find("\n", m.start())]
            if "struct" in line or "enum" in line or "->" in line:
                continue
            inner, _ = brace_block(blanked, blanked.index("{", m.start()))
            lineno = blanked.count("\n", 0, m.start()) + 1
            found.append((rel, lineno, inner))
    return found


def check_state_literals(fields: list[str]) -> tuple[bool, list[str]]:
    lines: list[str] = []
    literals = find_state_literals()
    if not fields:
        return False, ["could not parse the State struct's fields from src/main.rs"]
    if not literals:
        return False, ["no State literals found — the census itself is broken"]
    lines.append(f"{len(fields)} State fields; {len(literals)} full State literals found")
    missing: dict[str, list[str]] = {f: [] for f in fields}
    rest_blocks = 0
    for rel, lineno, inner in literals:
        chunks = split_top_level(inner)
        if any(c.strip().startswith("..") for c in chunks):
            rest_blocks += 1
            continue
        present = {entry_field(c) for c in chunks}
        present.discard(None)
        for f in fields:
            if f not in present:
                missing[f].append(f"{rel}:{lineno}")
    if rest_blocks:
        lines.append(f"({rest_blocks} literal(s) use a `..` rest pattern and were skipped)")
    ok = True
    for f in fields:
        absent = missing[f]
        tag = "all literals" if not absent else f"MISSING from {len(absent)}: {', '.join(absent)}"
        if absent:
            ok = False
        lines.append(f"  {f}: {tag}")
    pose = missing.get("pose_library", [])
    lines.append(
        "  pose_library (the Phase I near-miss): "
        + ("present in every literal" if not pose else f"MISSING from {len(pose)} literal(s)")
    )
    return ok, lines


# ----------------------------------------------------------------
# Check 4 — caller census for the Phase J+K+L+M lib ops
# ----------------------------------------------------------------

PHASE_J_OPS = [
    "fx_split_clip",
    "mirror_figure_all_frames",
    "normalise_all_clips",
    "particle_duplicate_event",
    "export_sheet_range",
    "export_svg_range",
    "sync_playback_audio_on_drift",
]

# Phase K additions. `duplicate_channel_to_figure` is a
# Phase J op (graph_keys.rs) that the Phase J list above
# never included — the gate did not watch it; it is
# censused from Phase K on. `audio_duplicate_clip_at`
# and `note_playback_speed` are wired by the integrator
# at integration (audio_panel button / main.rs poll), so
# on a pre-integration tree they correctly read UNWIRED.
PHASE_K_OPS = [
    "delete_channel",
    "delete_keys_in_range",
    "duplicate_channel_to_figure",
    "set_project_fps",
    "set_stage_size",
    "delete_project_file",
    "audio_duplicate_clip_at",
    "fx_clip_gizmo_geometry",
    "note_playback_speed",
]

# Phase L additions. Every Phase L section wires its
# own panel, so no entry here is integrator-wired and
# none is expected to read UNWIRED on the integrated
# tree; on a mid-phase tree an entry whose caller
# section has not landed yet reads UNWIRED, which is
# the gate working (the Phase K rule, restated in the
# header). Caller files verified at gate-writing time:
# set_head_ring_scale — inspector_figure.rs;
# clip_duration_secs — was audio_panel.rs; the UI
# mission's wave 4 replaced the panel's per-render
# decode with its cached probe, so the op's remaining
# production path is internal: clip_duration_frames
# (same file) wraps it, and timeline_marks calls that.
# It therefore moves to SAME_FILE_OPS below — still
# census-watched, classified by where its caller
# actually lives.
# simplify_stroke_at — stroke_inspector.rs;
# set_key_interp — keyframe_tools.rs.
PHASE_L_OPS = [
    "set_head_ring_scale",
    "simplify_stroke_at",
    "set_key_interp",
]

# Phase M additions. Caller files verified at
# gate-writing time: swap_frame_with_neighbour —
# frame_ops.rs (command_palette.rs / menus.rs entries
# are further callers as they land);
# audio_set_muted_at — mcp_tools.rs (audio_panel.rs
# as it lands); shift_keys — graph_panel.rs;
# fill_keys_in_range — keyframe_tools.rs;
# frame_camera_points — three_d_panel.rs (the Phase M
# point-cloud framer; plain `frame_camera` is the older
# figure-based search in the same file, a different op);
# stage_fig3_prims — stage_view.rs. `swap_frames`
# itself is NOT in this list: its only production
# caller is swap_frame_with_neighbour in its own
# file, so it is censused same-file below.
PHASE_M_OPS = [
    "swap_frame_with_neighbour",
    "audio_set_muted_at",
    "shift_keys",
    "fill_keys_in_range",
    "frame_camera_points",
    "stage_fig3_prims",
]

# Same-file ops: the op's defining file is also its
# only production caller, so the cross-file rule in
# check_callers (which skips the defining file) would
# report UNWIRED forever. For these, call sites in the
# defining file's own production region count too —
# same test-region cut as everywhere else, with the
# definition site itself excluded. `delete_camera_key_at`
# is Phase L section 39's undoable camera-key delete:
# a panel-local helper in camera_panel.rs, defined
# there and called by the panel's own key-row builder
# there (the older `clear_key_at` in the same file is
# the non-undoable clear and is not this entry).
# `swap_frames` is Phase M's frame_swap.rs core op:
# defined there and called in production only by
# `swap_frame_with_neighbour` in the same file (its
# remaining callers are the file's own tests); every
# user path reaches it through the neighbour op,
# which is censused cross-file in PHASE_M_OPS above.
SAME_FILE_OPS = [
    "delete_camera_key_at",
    "swap_frames",
    # UI mission wave 4: internal step of the wired
    # clip_duration_frames (see PHASE_L_OPS note).
    "clip_duration_secs",
]

CENSUS_OPS = PHASE_J_OPS + PHASE_K_OPS + PHASE_L_OPS + PHASE_M_OPS


def check_callers() -> tuple[bool, list[str]]:
    lines: list[str] = []
    paths = sorted(glob.glob(os.path.join(ROOT, "src", "**", "*.rs"), recursive=True))
    blanked_by_file: dict[str, str] = {}
    for full in paths:
        rel = os.path.relpath(full, ROOT)
        with open(full, encoding="utf-8") as fh:
            blanked_by_file[rel] = blank_code(fh.read())
    ok = True

    def census_one(name: str, same_file_ok: bool) -> None:
        nonlocal ok
        def_re = re.compile(rf"fn\s+{name}\s*\(")
        defining = [rel for rel, b in blanked_by_file.items() if def_re.search(b)]
        call_re = re.compile(rf"\b{name}\b")
        callers: list[str] = []
        for rel, b in blanked_by_file.items():
            if rel in defining and not same_file_ok:
                continue
            # The test region starts at the file's trailing
            # `mod tests` declaration — NOT at the first
            # `#[cfg(test)]` anywhere: main.rs carries a
            # cfg(test) module *declaration* (snap_tests) at
            # the top, and cutting there hid every real call
            # site below it (Phase J integration fix).
            m_tests = re.search(
                r"(?m)^\s*#\[cfg\(test\)\]\s*\n\s*(?:pub\s+)?mod\s+tests\b", b
            )
            test_at = m_tests.start() if m_tests else -1
            # In the defining file (same-file ops only), the
            # definition itself is not a call site.
            def_spans = (
                [(m.start(), m.end()) for m in def_re.finditer(b)]
                if rel in defining
                else []
            )
            for m in call_re.finditer(b):
                if 0 <= test_at <= m.start():
                    continue  # inside the file's test region
                if any(a <= m.start() < z for a, z in def_spans):
                    continue  # the definition, not a call
                lineno = b.count("\n", 0, m.start()) + 1
                callers.append(f"{rel}:{lineno}")
        where = ", ".join(defining) if defining else "NOT DEFINED ANYWHERE"
        suffix = " (same-file caller allowed)" if same_file_ok else ""
        if not callers:
            ok = False
            lines.append(f"  {name} (defined in {where}){suffix}: UNWIRED — zero production callers")
        else:
            shown = ", ".join(callers[:6]) + ("…" if len(callers) > 6 else "")
            lines.append(f"  {name} (defined in {where}){suffix}: {len(callers)} production call site(s): {shown}")

    for name in CENSUS_OPS:
        census_one(name, same_file_ok=False)
    for name in SAME_FILE_OPS:
        census_one(name, same_file_ok=True)
    return ok, lines


# ----------------------------------------------------------------
# Check 5 — count claims (numbers docs must match)
# ----------------------------------------------------------------


def count_claims() -> list[tuple[str, int, str]]:
    claims: list[tuple[str, int, str]] = []

    palette = blank_code(read("src/rune_sec/command_palette.rs"))
    n = len(re.findall(r"Command\s*\{\s*key\s*:", palette))
    claims.append(("palette COMMANDS entries", n, 'count of `Command { key:` in command_palette.rs'))

    mcp_src = blank_code(read("src/mcp_tools.rs"))
    m = re.search(r"pub fn tools\s*\(\)[^{]*\{", mcp_src)
    if m:
        inner, _ = brace_block(mcp_src, mcp_src.index("{", m.start()))
        n = len(re.findall(r"Tool\s*\{\s*name\s*:", inner))
    else:
        n = 0
    claims.append(("MCP registry tools", n, '`Tool { name:` entries inside pub fn tools() in mcp_tools.rs'))

    # Wave Q: the two subtitle families' own counts,
    # read with the registry claim's idiom from their
    # own files — the docs print the family split
    # (registry 319 / io 9 / subtitle io 4 / subtitle
    # tools 5 beside the 44 built-ins), so the gate
    # prints every number the docs must match.
    for rel, label in (
        ("src/mcp_subtitle_tools.rs", "MCP subtitle tools"),
        ("src/mcp_subtitle_io.rs", "MCP subtitle IO tools"),
    ):
        sub_src = blank_code(read(rel))
        m = re.search(r"pub fn tools\s*\(\)[^{]*\{", sub_src)
        if m:
            inner, _ = brace_block(sub_src, sub_src.index("{", m.start()))
            n = len(re.findall(r"Tool\s*\{\s*name\s*:", inner))
        else:
            n = 0
        claims.append(
            (label, n, f"`Tool {{ name:` entries inside pub fn tools() in {rel.rsplit('/', 1)[1]}")
        )

    icon_raw = read("src/rune_sec/icon_data.rs")
    m = re.search(r"pub const ICON_NAMES[^=]*=\s*&\[", icon_raw)
    if m:
        # The slice's own `[` is the last char of the match;
        # searching from m.start() would find the `[` of the
        # `&[&str]` type annotation first and count nothing
        # (Phase J integration fix).
        open_idx = m.end() - 1
        depth = 0
        j = open_idx
        while j < len(icon_raw):
            if icon_raw[j] == "[":
                depth += 1
            elif icon_raw[j] == "]":
                depth -= 1
                if depth == 0:
                    break
            j += 1
        n = len(re.findall(r'"[^"]+"', icon_raw[open_idx:j]))
    else:
        n = 0
    claims.append(("curated icons (ICON_NAMES)", n, "string entries in the ICON_NAMES slice in icon_data.rs"))

    scenes_src = blank_code(read("src/rune_sec/scenes.rs"))
    m = re.search(r"pub fn scenes\s*\(\)[^{]*\{", scenes_src)
    n = 0
    if m:
        inner, _ = brace_block(scenes_src, scenes_src.index("{", m.start()))
        v = inner.find("vec!")
        if v >= 0:
            vec_inner, _ = bracket_block(inner, inner.index("[", v))
            depth = 0
            for ch in vec_inner:
                if ch in "{([":
                    if ch == "(" and depth == 0:
                        n += 1
                    depth += 1
                elif ch in "})]":
                    depth -= 1
    claims.append(("headless scenes", n, "tuple entries in the vec! returned by scenes() in scenes.rs"))
    return claims


# ----------------------------------------------------------------
# Check 6 — design adoption census (Phase N)
# ----------------------------------------------------------------

# The charter-fixed design kit (docs/PHASE_N_CHARTER.md):
# constants by name AND value, fns by name. Amendment
# extras the kit grew (text_muted, divider_keyed,
# row_shell, swatch, ...) are reported, not required.
CHARTER_CONSTS = {
    "PANEL_RADIUS": "12.0",
    "PANEL_PAD": "12.0",
    "PANEL_GAP": "8.0",
    "ROW_RADIUS": "8.0",
    "CONTROL_RADIUS": "6.0",
}
CHARTER_FNS = [
    "accent",
    "selected_fill",
    "panel_col",
    "header_row",
    "panel_title",
    "section_label",
    "divider",
    "kbd",
    "empty_state",
    "list_row",
    # "toggle" retired by the UI mission (wave 2): zero
    # consumers tree-wide, toggle_row is the used form.
    # Recorded in design.rs's module doc.
    "toggle_row",
    "stepper_value",
    "slider_track",
]
# Kit fns the charter allows to be consumed only by
# other kit fns: their consumer census runs tree-wide
# INCLUDING design.rs, and the printout marks it (the
# P17 section 5 item 2 exception).
KIT_INTERNAL_OK = {"accent", "selected_fill"}

# The 31 panel files of the adoption commit a570d60
# ("Phase N adoption: 32 panels onto the design kit").
# The floor (P20 section 2.2, task floor): >= 1
# production-region `design::` reference per file; a
# file at zero is UNADOPTED, the restyle analogue of
# check 4's UNWIRED.
ADOPTED_PANELS = [
    "audio_panel",
    "camera_panel",
    "clips_panel",
    "draw_tools",
    "easing_panel",
    "export_panel",
    "figure_add",
    "figures_panel",
    "frame_ops",
    "fx_browser",
    "fx_clips",
    "graph_panel",
    "help_panel",
    "home_panel",
    "inspector_figure",
    "inspector_segment",
    "inspector_style",
    "keyframe_tools",
    "layers_panel",
    "library_panel",
    "onion_controls",
    "particles_panel",
    "playback_controls",
    "pose_tools",
    "project_tools",
    "scene_panel",
    "stats_view",
    "stroke_inspector",
    "text_tool",
    "three_d_panel",
    "timeline_marks",
]

# Adopted files whose root is deliberately NOT a
# design::panel_col call, with the charter reason —
# the exemption idiom of SAME_FILE_OPS above.
PANEL_COL_EXCEPTIONS = {
    "timeline_marks": (
        "charter section 13: the timeline card is deliberately UNKEYED "
        "and hand-built (a plain col with design::PANEL_RADIUS / "
        "card fill / hairline); panel_col would force a key on it"
    ),
}

# Foundation surfaces that predate the per-file
# floor: censused and printed, never required. The
# family homes define the buttons the kit composes;
# stage_view is pure canvas; toasts / status_strip
# mirror kit tokens in local constructions.
FOUNDATION_SURFACES = [
    "src/chrome_view.rs",
    "src/stage_view.rs",
    "src/rune_sec/icon_btn.rs",
    "src/rune_sec/menus.rs",
    "src/rune_sec/command_palette.rs",
    "src/rune_sec/toasts.rs",
    "src/rune_sec/status_strip.rs",
    "src/rune_sec/settings_popover.rs",
    "src/rune_sec/workspaces.rs",
]

# The retired local dialects: definitions must be
# ZERO across src/rune_sec/ outside the family homes
# (design.rs itself and icon_btn.rs are excluded from
# the scan — design.rs DEFINES the kit section_label;
# chrome_view.rs, the danger_btn family home, lives
# outside src/rune_sec/ and is out of census scope).
RETIRED_HELPERS = [
    "fbtn",
    "kbtn",
    "pbtn",
    "pibtn",
    "ctl",
    "ictl",
    "row_toggle",
    "cap_chip",
    "preset_chip",
    "danger_btn",
    "section",
    "section_label",
    "section_header",
]
RETIRED_SCAN_EXCLUDE = {"design.rs", "icon_btn.rs"}
# A surviving definition is exempt only when it is a
# control the kit structurally cannot express, named
# here with its reason (the SAME_FILE_OPS precedent).
RETIRED_EXEMPTIONS = {
    ("graph_panel.rs", "preset_chip"): (
        "the chip carries its own drawn preview-curve canvas; "
        "chip_btn has no content slot (the easing-picker "
        "restyle-not-replace precedent)"
    ),
}
# A retired-name definition whose body delegates to
# the kit/family and constructs no element of its own
# is a call-site adapter, not a surviving dialect:
# it must CALL one of these and contain NONE of the
# construction tokens below.
_DELEGATION_CALLS = ("design::", "chip_btn(", "icon_btn(", "icon_only_btn(", "btn(")
_CONSTRUCTION_TOKENS = (
    "el()",
    "col()",
    "row()",
    "text(",
    ".fill(",
    ".radius(",
    ".hover(",
    ".press(",
    ".px(",
    ".py(",
    ".key(",
)

_TEST_MOD_RE = re.compile(r"(?m)^\s*#\[cfg\(test\)\]\s*\n\s*(?:pub\s+)?mod\s+tests\b")


def production_region(blanked: str) -> str:
    """The file's production region: everything before
    its trailing `mod tests` (check 4's cut)."""
    m = _TEST_MOD_RE.search(blanked)
    return blanked[: m.start()] if m else blanked


def check_design_adoption() -> tuple[bool, list[str]]:
    lines: list[str] = []
    ok = True

    # (a) Kit self-check: design.rs defines the
    # charter's constants (name and value) and fns.
    design_raw = read("src/rune_sec/design.rs")
    design_blanked = blank_code(design_raw)
    missing: list[str] = []
    for name, value in CHARTER_CONSTS.items():
        pat = rf"pub\s+const\s+{name}\s*:\s*f32\s*=\s*{re.escape(value)}\s*;"
        if not re.search(pat, design_raw):
            missing.append(f"const {name} = {value}")
    for name in CHARTER_FNS:
        if not re.search(rf"pub\s+fn\s+{name}\s*\(", design_blanked):
            missing.append(f"fn {name}")
    if missing:
        ok = False
        lines.append(f"  kit self-check: MISSING from design.rs: {', '.join(missing)}")
    else:
        lines.append(
            f"  kit self-check: all {len(CHARTER_CONSTS)} charter consts "
            f"(name + value) and {len(CHARTER_FNS)} charter fns defined in design.rs"
        )
    kit_fns = sorted(set(re.findall(r"pub\s+fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(", design_blanked)))
    extras = [n for n in kit_fns if n not in CHARTER_FNS]
    if extras:
        lines.append(f"  kit extras beyond the charter API (report only): {', '.join(extras)}")

    # Blanked production regions for every src file.
    paths = sorted(glob.glob(os.path.join(ROOT, "src", "**", "*.rs"), recursive=True))
    prod_by_rel: dict[str, str] = {}
    for full in paths:
        rel = os.path.relpath(full, ROOT)
        with open(full, encoding="utf-8") as fh:
            prod_by_rel[rel] = production_region(blank_code(fh.read()))

    # (b) Per-file adoption census over the 31 adopted
    # panels + the panel_col roll.
    lines.append(
        "  adoption floor: >= 1 production `design::` reference per adopted "
        "file (0 = UNADOPTED); panel roots via design::panel_col except "
        "named exceptions"
    )
    adopted_roots = 0
    for name in ADOPTED_PANELS:
        rel = f"src/rune_sec/{name}.rs"
        prod = prod_by_rel.get(rel)
        if prod is None:
            ok = False
            lines.append(f"  {name}: FILE MISSING")
            continue
        n_design = len(re.findall(r"\bdesign::", prod))
        n_col = len(re.findall(r"\bpanel_col\s*\(", prod))
        if n_design == 0:
            ok = False
            lines.append(f"  {name}: UNADOPTED — zero production design:: references")
            continue
        if n_col > 0:
            adopted_roots += 1
            root = f"panel_col x{n_col}"
        elif name in PANEL_COL_EXCEPTIONS:
            root = "panel_col exception (see below)"
        else:
            ok = False
            root = "NO panel_col ROOT AND NO NAMED EXCEPTION"
        lines.append(f"  {name}: design:: x{n_design}, {root}")
    for name, why in PANEL_COL_EXCEPTIONS.items():
        lines.append(f"  panel_col exception — {name}: {why}")
    lines.append(
        f"  panel_col adoption: {adopted_roots} of {len(ADOPTED_PANELS)} adopted "
        f"panel files ({len(PANEL_COL_EXCEPTIONS)} named exception(s))"
    )
    consumer_files = sorted(
        rel for rel, prod in prod_by_rel.items() if re.search(r"\bdesign::", prod)
    )
    lines.append(
        f"  design kit consumers: {len(consumer_files)} files "
        f"(distinct src files naming design:: in production code)"
    )
    for rel in FOUNDATION_SURFACES:
        prod = prod_by_rel.get(rel, "")
        n = len(re.findall(r"\bdesign::", prod))
        lines.append(f"  foundation surface (report only) {rel}: design:: x{n}")

    # (c) Consumer census per kit fn (report only).
    lines.append("  kit consumer census (report only; 0 external consumers is not a failure):")
    for name in kit_fns:
        call_re = re.compile(rf"design::{name}\s*\(")
        consumers = [
            rel
            for rel, prod in prod_by_rel.items()
            if rel != "src/rune_sec/design.rs" and call_re.search(prod)
        ]
        if name in KIT_INTERNAL_OK and re.search(rf"\b{name}\s*\(", prod_by_rel["src/rune_sec/design.rs"]):
            consumers.append("src/rune_sec/design.rs (internal, allowed)")
        tag = "ADOPTED" if consumers else "UNADOPTED (report only)"
        lines.append(f"    {name}: {tag} — {len(consumers)} consumer file(s)")

    # (d) Retired-helper census: definitions in
    # src/rune_sec/ production regions, outside the
    # family homes. A definition is a SURVIVOR (fail)
    # unless it only delegates to the kit/family or
    # carries a named exemption.
    lines.append("  retired-helper census (definitions in src/rune_sec/ production code):")
    for name in RETIRED_HELPERS:
        def_re = re.compile(rf"fn\s+{name}\s*\(")
        found: list[str] = []
        for rel, prod in prod_by_rel.items():
            if not rel.startswith("src/rune_sec/"):
                continue
            if os.path.basename(rel) in RETIRED_SCAN_EXCLUDE:
                continue
            for m in def_re.finditer(prod):
                lineno = prod.count("\n", 0, m.start()) + 1
                brace = prod.find("{", m.end())
                body, _ = brace_block(prod, brace) if brace >= 0 else ("", 0)
                base = os.path.basename(rel)
                if (base, name) in RETIRED_EXEMPTIONS:
                    found.append(f"{rel}:{lineno} — named exception: {RETIRED_EXEMPTIONS[(base, name)]}")
                elif any(c in body for c in _DELEGATION_CALLS) and not any(
                    t in body for t in _CONSTRUCTION_TOKENS
                ):
                    found.append(f"{rel}:{lineno} — delegating wrapper (name kept as a call-site adapter)")
                else:
                    ok = False
                    found.append(f"{rel}:{lineno} — SURVIVOR: a local {name} definition the charter retired")
        if found:
            lines.append(f"    fn {name}: {len(found)} definition(s): {'; '.join(found)}")
        else:
            lines.append(f"    fn {name}: 0 definitions")
    return ok, lines


# ----------------------------------------------------------------
# Check 7 — literal & empty-state guard (UI mission Wave 9)
# ----------------------------------------------------------------


def blank_comments(src: str) -> str:
    """blank_code's sibling: comment contents blanked,
    string / char literal contents PRESERVED (positions
    and newlines identical), so scans that need string
    contents — "#RRGGBB" literals, `text("No …")` —
    still never see text that lives in a comment."""
    out = list(src)
    i, n = 0, len(src)
    while i < n:
        ch = src[i]
        two = src[i : i + 2]
        if two == "//":
            j = src.find("\n", i)
            j = n if j < 0 else j
            for k in range(i, j):
                out[k] = " "
            i = j
        elif two == "/*":
            depth = 1
            j = i + 2
            while j < n and depth:
                if src[j : j + 2] == "/*":
                    depth += 1
                    j += 2
                elif src[j : j + 2] == "*/":
                    depth -= 1
                    j += 2
                else:
                    j += 1
            for k in range(i, min(j, n)):
                if out[k] != "\n":
                    out[k] = " "
            i = j
        elif ch == '"':
            j = i + 1
            while j < n:
                if src[j] == "\\":
                    j += 2
                    continue
                if src[j] == '"':
                    j += 1
                    break
                j += 1
            i = j
        elif ch == "'":
            m = _CHAR_LIT_RE.match(src, i)
            i = m.end() if m else i + 1
        else:
            i += 1
    return "".join(out)


# The lawful colour sources (Wave 7 census): literals
# here ARE the palette, not violations of it.
LITERAL_SOURCE_FILES = {
    "app_theme.rs": "the single app palette constructor (Wave 2 / Wave 7 census)",
    "design.rs": "the design-token file (holds no hex literals today)",
}
# Files under src/rune_sec/ that are not production
# code at all: main.rs declares snap_tests under
# #[cfg(test)], so every line of it is test code.
LITERAL_TEST_FILES = {
    "snap_tests.rs": "test-only module (#[cfg(test)] in main.rs) — not production code",
}

# Const-span exceptions: (file, const name) -> the
# Wave 7 classification. A hex literal is exempt when
# it sits inside the named const's declaration span
# (from the `const` keyword to the `;` after its `=`).
LITERAL_CONST_EXCEPTIONS = {
    ("clips_panel.rs", "FIGURE_COLORS"): "palette DATA (clip colour swatches)",
    ("fx_clips.rs", "SWATCHES"): "palette DATA (clip colour / end-colour picker)",
    ("fx_clips.rs", "STRIP_ACCENT"): "stage/canvas content colour (selected clip strip outline)",
    ("inspector_figure.rs", "PALETTE"): "palette DATA (figure colour picker)",
    ("inspector_segment.rs", "PALETTE"): "palette DATA (segment colour picker)",
    ("inspector_style.rs", "STYLE_PRESETS"): "preset DATA (named style presets applied to user strokes)",
    ("particles_panel.rs", "PARTICLE_PALETTE"): "palette DATA (particle colour picker)",
    ("subtitle_panel.rs", "SUBTITLE_PALETTE"): "palette DATA (subtitle caption colour picker; Wave Q integration)",
    ("text_tool.rs", "TEXT_PALETTE"): "palette DATA (text colour picker)",
    ("scene_panel.rs", "BACKGROUND_SWATCHES"): "palette DATA (stage background presets)",
    ("scene_panel.rs", "BG_PRESETS"): "palette DATA (stage background presets)",
    ("fig3_preview.rs", "GRID_COLOR"): "Fig3Palette::LEGACY constant (pre-theme 3D palette)",
    ("fig3_preview.rs", "SELECTED_COLOR"): "Fig3Palette::LEGACY constant (pre-theme 3D palette)",
    ("fig3_preview.rs", "SHADOW_COLOR"): "Fig3Palette::LEGACY constant (pre-theme 3D palette)",
    ("onion_controls.rs", "FUTURE_ONION_RGB"): "stage content colour (future-frame onion tint; Wave 7 census class)",
    ("stage_fx.rs", "GIZMO_COLOR"): "stage content colour (FX gizmo accent)",
    ("stage_text.rs", "BACKING_COLOR"): "stage content colour (text backing plate on the stage canvas)",
    ("timeline_marks.rs", "AUDIO_SPAN_RGB"): "stage/canvas content colour (timeline audio clip span)",
    ("timeline_marks.rs", "FX_SPAN_RGB"): "stage/canvas content colour (timeline FX clip span)",
}

# Line exceptions: (file, regex over the literal's raw
# source line) -> classification. Each regex pins the
# exact literal it exempts, so a NEW literal — even on
# an exempted line — still reads UNCLASSIFIED.
LITERAL_LINE_EXCEPTIONS = [
    (
        "graph_panel.rs",
        r"hex_color\(0xFFC65D\)",
        "stage content colour (graph key-handle amber; Wave 7 census class)",
    ),
    (
        "project_tools.rs",
        r"background:\s*0x232830",
        "project-content default (blank demo project background)",
    ),
    (
        "project_tools.rs",
        r"color:\s*0xE8E8EC",
        "project-content default (blank demo project figure colour)",
    ),
    (
        "inspector_style.rs",
        r"Some\(0x(?:FF_FFFF|22_2222)\)",
        "gradient swatch DATA (the two gradient end-colour values, matched and set)",
    ),
    (
        "inspector_style.rs",
        r"hex_color\(0x(?:FF_FFFF|22_2222)\)",
        "gradient swatch DATA (gradient_swatch paints each swatch with the exact value it sets)",
    ),
    (
        "icons.rs",
        r"&\s*0xFF_FFFF",
        "24-bit channel mask in icon_color (not a colour value)",
    ),
    (
        "scene_panel.rs",
        r"&\s*0xFF_FFFF",
        "24-bit channel mask in set_background (not a colour value)",
    ),
    (
        "text_tool.rs",
        r"&\s*0xFF_FFFF",
        "24-bit channel mask on a text instance colour (not a colour value)",
    ),
    (
        "scenes.rs",
        r"0x(?:33_66_CC|CC_55_33|FF_CC_66|22_88_55|77_33_AA)",
        "project-content default (Wave 12 populated-scene fixture figure/stroke colours)",
    ),
    (
        "scenes.rs",
        r"0x(?:FF0000|0000FF)",
        "project-content default (Wave 13 library-populated fixture: the styled def's red node and gradient end colour, replicated from library_panel's pin fixture)",
    ),
    (
        "library_panel.rs",
        r"unwrap_or\(0xFF_FFFF\)",
        "parse fallback in color_hex_u32 (white if the theme colour's hex fails to parse; not a drawn colour)",
    ),
]

_HEX_INT_RE = re.compile(r"0x[0-9A-Fa-f_]+")
_HEX_STR_RE = re.compile(r'"#[0-9A-Fa-f]{6}(?:[0-9A-Fa-f]{2})?"')
_BARE_NO_RE = re.compile(r'\btext\(\s*"(?:No |Nothing)')


def const_decl_span(blanked_prod: str, name: str) -> tuple[int, int] | None:
    """Span of `const NAME … = …;` in blanked production
    code: from the `const` keyword to the semicolon
    AFTER the `=` (a `[u32; 16]` type carries its own
    semicolon, so the first `;` is not the end)."""
    m = re.search(rf"\bconst\s+{name}\b", blanked_prod)
    if not m:
        return None
    eq = blanked_prod.find("=", m.end())
    if eq < 0:
        return None
    semi = blanked_prod.find(";", eq)
    return (m.start(), (semi + 1) if semi >= 0 else len(blanked_prod))


def check_literal_guard() -> tuple[bool, list[str]]:
    lines: list[str] = []
    ok = True
    paths = sorted(glob.glob(os.path.join(ROOT, "src", "rune_sec", "*.rs")))
    by_class: dict[str, int] = {}
    source_counts: dict[str, int] = {}
    unclassified: list[str] = []
    bare_hits: list[str] = []
    total = 0

    def note(cls: str) -> None:
        by_class[cls] = by_class.get(cls, 0) + 1

    for full in paths:
        rel = os.path.relpath(full, ROOT)
        base = os.path.basename(rel)
        with open(full, encoding="utf-8") as fh:
            raw = fh.read()
        raw_lines = raw.splitlines()
        blanked = blank_code(raw)
        uncommented = blank_comments(raw)
        prod_end = len(production_region(blanked))
        prod_blank = blanked[:prod_end]
        prod_uncommented = uncommented[:prod_end]

        # (b) Bare empty lines: production region, real
        # code only (the comment-blanked text keeps
        # strings, the code-blanked text proves the
        # `text(` at the match offset is code, not a
        # comment or string interior).
        if base not in LITERAL_TEST_FILES:
            for m in _BARE_NO_RE.finditer(prod_uncommented):
                if prod_blank[m.start()] != "t":
                    continue
                lineno = prod_uncommented.count("\n", 0, m.start()) + 1
                bare_hits.append(f"{rel}:{lineno}: {raw_lines[lineno - 1].strip()}")

        # (a) Hex colour literals in the production
        # region: integer forms in code-blanked text
        # (comments/strings cannot fake one), string
        # forms in comment-blanked text.
        occurrences: list[tuple[int, str]] = []
        for m in _HEX_INT_RE.finditer(prod_blank):
            digits = m.group(0)[2:].replace("_", "")
            if len(digits) in (6, 8) and all(
                c in "0123456789abcdefABCDEF" for c in digits
            ):
                occurrences.append((m.start(), m.group(0)))
        for m in _HEX_STR_RE.finditer(prod_uncommented):
            occurrences.append((m.start(), m.group(0)))
        occurrences.sort()
        if not occurrences:
            continue
        if base in LITERAL_TEST_FILES:
            lines.append(
                f"  {base}: {len(occurrences)} literal(s) — skipped: "
                f"{LITERAL_TEST_FILES[base]}"
            )
            continue
        if base in LITERAL_SOURCE_FILES:
            source_counts[base] = len(occurrences)
            total += len(occurrences)
            continue
        spans = [
            (const_decl_span(prod_blank, name), cls)
            for (f, name), cls in LITERAL_CONST_EXCEPTIONS.items()
            if f == base
        ]
        line_exceptions = [
            (re.compile(pat), cls)
            for f, pat, cls in LITERAL_LINE_EXCEPTIONS
            if f == base
        ]
        for at, literal in occurrences:
            total += 1
            cls = next((c for span, c in spans if span and span[0] <= at < span[1]), None)
            if cls is None:
                lineno = prod_blank.count("\n", 0, at) + 1
                line_text = raw_lines[lineno - 1] if lineno - 1 < len(raw_lines) else ""
                cls = next((c for pat, c in line_exceptions if pat.search(line_text)), None)
            if cls is None:
                lineno = prod_blank.count("\n", 0, at) + 1
                unclassified.append(f"{rel}:{lineno}: {literal} :: {raw_lines[lineno - 1].strip()}")
            else:
                note(cls)

    for base, why in LITERAL_SOURCE_FILES.items():
        if base in source_counts:
            lines.append(
                f"  lawful source {base}: {source_counts[base]} literal(s) — {why}"
            )
    lines.append(f"  production hex literals censused: {total}")
    for cls in sorted(by_class):
        lines.append(f"    exempt — {cls}: {by_class[cls]}")
    if unclassified:
        ok = False
        lines.append(
            f"  UNCLASSIFIED: {len(unclassified)} literal(s) outside the Wave 7 "
            "classification — move them into the token files or add a named exception:"
        )
        lines.extend(f"    {u}" for u in unclassified)
    else:
        lines.append("  UNCLASSIFIED: 0 — every literal is a token source or a named exception")
    if bare_hits:
        ok = False
        lines.append(
            f"  bare empty lines: {len(bare_hits)} FOUND — the Wave 6 sweep reached "
            "zero; empty states use design::empty_state:"
        )
        lines.extend(f"    {h}" for h in bare_hits)
    else:
        lines.append('  bare empty lines: 0 — no text("No …") / text("Nothing…") in production code')
    return ok, lines


# ----------------------------------------------------------------
# Check 8 — coverage completeness (Wave 10): scenes <->
# headless.yml in both directions + the layout-fact
# sequence. Regex over the two files, in this script's
# idiom: structure located on comment/string-blanked
# text (offsets identical to the raw text — blank_code
# preserves length), string contents read back from the
# raw text at the same offsets.
# ----------------------------------------------------------------

_FACT_ITEM_RE = re.compile(r"^\s*///\s+(\d+)\.(?:\s|$)")
_LOOP_RE = re.compile(r"for\s+scene\s+in\s+([^;]+?);\s*do")
_SCENE_ARG_RE = re.compile(r"--scene\s+([A-Za-z0-9][A-Za-z0-9_-]*)")
_SEE_FACT_RE = re.compile(r"see fact (\d+)")


def _fn_region(raw: str, blanked: str, sig_re: str) -> str:
    """The raw text of a fn body (signature's `{` to its
    matching `}`), located on the blanked copy; empty
    string when the signature is not found."""
    m = re.search(sig_re, blanked)
    if not m:
        return ""
    open_idx = blanked.index("{", m.start())
    _, end = brace_block(blanked, open_idx)
    return raw[open_idx:end]


def check_coverage() -> tuple[bool, list[str]]:
    lines: list[str] = []
    ok = True

    scenes_raw = read("src/rune_sec/scenes.rs")
    scenes_blanked = blank_code(scenes_raw)

    # Registered scenes: the ("name", fn) tuples of the
    # vec! scenes() returns — the same live extraction
    # check 5 counts, here with the names read back.
    scenes_region = _fn_region(scenes_raw, scenes_blanked, r"pub fn scenes\s*\(\)[^{]*\{")
    registered = re.findall(r'\(\s*"([^"]+)"\s*,', scenes_region)
    if not registered:
        return False, ["could not parse the scenes() vec in scenes.rs — the census itself is broken"]
    registered_set = set(registered)

    # headless.yml scene usage. Full-line comments are
    # dropped first: the workflow's header comment
    # documents `--scene NAME` in prose, which is not a
    # probe. Loop lists (the `for scene in …; do` lines)
    # and literal `--scene NAME` probe arguments are the
    # two real uses; the loops' own `--scene "$scene"`
    # is the variable form of the lists.
    yml_lines = [
        ln
        for ln in read(".github/workflows/headless.yml").splitlines()
        if not ln.lstrip().startswith("#")
    ]
    yml = "\n".join(yml_lines)
    loops = [m.group(1).split() for m in _LOOP_RE.finditer(yml)]
    if not loops:
        return False, ["no `for scene in …; do` tree loop found in headless.yml — the workflow changed shape; update this check"]
    loop_union: set[str] = set()
    for loop in loops:
        loop_union |= set(loop)
    direct = sorted(set(_SCENE_ARG_RE.findall(yml)))

    # (a) Forward coverage: every registered scene is
    # tree-dumped by at least one loop.
    uncovered = [name for name in registered if name not in loop_union]
    if uncovered:
        ok = False
        for name in uncovered:
            lines.append(
                f'  scene "{name}": registered in scenes() but in NO tree loop in '
                "headless.yml — its surfaces are never tree-dumped (the Wave 9 "
                "export-panel blind spot); the integrator adds it to the loops "
                "in .github/workflows/headless.yml"
            )
    else:
        lines.append(
            f"  forward coverage: all {len(registered)} registered scenes appear "
            "in the headless.yml tree loops"
        )
    for i, loop in enumerate(loops, 1):
        role = (
            "plain attempt — the coverage contract"
            if i == 1
            else "xvfb fallback — retries only scenes the plain attempt missed"
        )
        lines.append(f"  tree loop {i} ({role}): {len(loop)} scene names")
    # Report-only: scenes a later loop omits that an
    # earlier loop covers (today: the Wave 7 light pair
    # is absent from the fallback's retry list). Not a
    # failure — the fallback only runs for scenes whose
    # plain dump is missing, and the plain loop is the
    # contract (see the header).
    for i, loop in enumerate(loops, 1):
        if i == 1:
            continue
        subset_gap = [
            name for name in registered if name in loop_union and name not in set(loop)
        ]
        if subset_gap:
            lines.append(
                f"  tree loop {i} subset note (report only): does not list "
                f"{', '.join(subset_gap)} — covered by an earlier loop"
            )

    # (c) Reverse coverage: no stale names in the YAML.
    stale_loops = sorted(loop_union - registered_set)
    stale_direct = [name for name in direct if name not in registered_set]
    if stale_loops or stale_direct:
        ok = False
        for name in stale_loops:
            lines.append(
                f'  headless.yml tree loops name "{name}", which scenes() does '
                "not register — the probe produces nothing for it and says "
                "nothing (stale name: renamed scene or typo)"
            )
        for name in stale_direct:
            lines.append(
                f'  a --scene probe in headless.yml names "{name}", which '
                "scenes() does not register — the probe produces nothing for "
                "it and says nothing (stale name: renamed scene or typo)"
            )
    else:
        lines.append(
            "  reverse coverage: every scene name in headless.yml (tree loops "
            "+ --scene probes) is registered in scenes() — no stale names"
        )

    # (b) Layout facts: the numbered doc list above
    # layout_assertions() must run 1..=N in order; the
    # assertions vec is positional (string i is fact
    # i), so it must hold exactly N strings, and any
    # `see fact N` citation inside a string must name
    # that string's own position.
    raw_lines = scenes_raw.splitlines()
    start = next(
        (i for i, ln in enumerate(raw_lines) if "Layout facts a CI" in ln), None
    )
    stop = next(
        (
            i
            for i, ln in enumerate(raw_lines)
            if start is not None and i > start and "pub fn layout_assertions" in ln
        ),
        None,
    )
    if start is None or stop is None:
        ok = False
        lines.append("  layout facts: the fact list above layout_assertions() was not found in scenes.rs")
    else:
        facts: list[int] = []
        for ln in raw_lines[start:stop]:
            fm = _FACT_ITEM_RE.match(ln)
            if fm:
                facts.append(int(fm.group(1)))
        expected = list(range(1, len(facts) + 1))
        if facts != expected:
            ok = False
            missing = [n for n in range(1, max(facts, default=0) + 1) if n not in facts]
            dupes = sorted({n for n in facts if facts.count(n) > 1})
            detail = []
            if missing:
                detail.append(f"gap(s) at {missing}")
            if dupes:
                detail.append(f"duplicate(s) {dupes}")
            if not detail:
                detail.append("numbers out of order")
            lines.append(
                f"  layout facts: NOT sequential — found {facts} "
                f"({'; '.join(detail)}); renumber so the list runs 1..=N with no gaps or duplicates"
            )
        assertions_region = _fn_region(
            scenes_raw, scenes_blanked, r"pub fn layout_assertions\s*\(\)[^{]*\{"
        )
        assertions = re.findall(r'"((?:\\.|[^"\\])*)"', assertions_region)
        if len(assertions) != len(facts):
            ok = False
            lines.append(
                f"  layout facts: layout_assertions() holds {len(assertions)} strings "
                f"for {len(facts)} documented facts — the vec is positional "
                "(string i is fact i); add/remove the string with its fact"
            )
        miscited: list[str] = []
        for i, text in enumerate(assertions, 1):
            for cit in _SEE_FACT_RE.findall(text):
                if int(cit) != i:
                    miscited.append(f"string {i} cites fact {cit}")
        if miscited:
            ok = False
            lines.append(
                "  layout facts: miscited assertion string(s) — a `see fact N` "
                f"citation must name its own string's fact: {'; '.join(miscited)}"
            )
        if facts == expected and len(assertions) == len(facts) and not miscited:
            lines.append(
                f"  layout facts: 1..={len(facts)} sequential, no gaps or duplicates; "
                f"layout_assertions() holds {len(assertions)} strings and every "
                "`see fact N` citation names its own fact"
            )
    return ok, lines


# ----------------------------------------------------------------
# Check 9 — MCP surface census & integrity (Wave 11).
# Structure is located on comment/string-blanked text
# (check 8's idiom: offsets identical to the raw text),
# string contents (tool names) are read back from the
# raw text at the same offsets. Every extractor fails
# loudly when its declaration pattern is not found.
# ----------------------------------------------------------------

_MCP_NAME_RE = re.compile(r'\bname:\s*"([^"]+)"')
_MCP_HANDLER_RE = re.compile(
    r"\bhandler:\s*([A-Za-z_][A-Za-z0-9_]*(?:::[A-Za-z_][A-Za-z0-9_]*)*|\|)"
)
_MCP_CTOR_RE = re.compile(r"fn\s+(tool_[A-Za-z0-9_]*)\s*\(\s*\)\s*->\s*Tool\s*\{")
_MCP_DOC_ROW_RE = re.compile(r"^\|\s*`([a-z0-9_]+)`\s*\|", re.M)


def _mcp_fn_span(blanked: str, sig_re: str) -> tuple[int, int] | None:
    """(start, end) offsets of a fn body in blanked
    source: the signature's `{` to just past its
    matching `}`. None when the signature is absent."""
    m = re.search(sig_re, blanked)
    if not m:
        return None
    open_idx = blanked.index("{", m.start())
    _, end = brace_block(blanked, open_idx)
    return (open_idx, end)


def _mcp_tool_literals(
    raw: str, blanked: str, span: tuple[int, int], family: str
) -> tuple[list[tuple[str, str]], list[str]]:
    """One (name, handler) per `Tool { ... }` literal
    inside the span. A literal whose name or handler
    cannot be read is a census problem, never a
    skipped entry."""
    entries: list[tuple[str, str]] = []
    problems: list[str] = []
    region = blanked[span[0] : span[1]]
    for m in re.finditer(r"(?<![\w:])Tool\s*\{", region):
        open_idx = span[0] + region.index("{", m.start())
        _, end = brace_block(blanked, open_idx)
        lit_raw = raw[open_idx:end]
        lineno = raw.count("\n", 0, open_idx) + 1
        nm = _MCP_NAME_RE.search(lit_raw)
        hm = _MCP_HANDLER_RE.search(lit_raw)
        if nm is None:
            problems.append(
                f"  {family}: a Tool literal at line {lineno} has no readable "
                '`name: "..."` — the declaration pattern changed; update this check'
            )
        if hm is None:
            problems.append(
                f"  {family}: the Tool literal at line {lineno} has no readable "
                "`handler:` — the declaration pattern changed; update this check"
            )
        if nm is not None and hm is not None:
            entries.append((nm.group(1), hm.group(1)))
    if not entries and not problems:
        problems.append(
            f"  {family}: ZERO Tool literals found inside tools() — the "
            "declaration pattern changed; update this check (a silent zero "
            "is exactly what this census exists to prevent)"
        )
    return entries, problems


def _mcp_registry_family(
    rel: str, family: str
) -> tuple[list[tuple[str, str]], list[str]]:
    """The (name, handler) entries of a family's
    `pub fn tools()` (mcp_tools.rs / mcp_io.rs shape)."""
    raw = read(rel)
    blanked = blank_code(raw)
    span = _mcp_fn_span(blanked, r"pub fn tools\s*\(\)[^{]*\{")
    if span is None:
        return [], [
            f"  {family}: `pub fn tools()` not found in {rel} — the "
            "registration mechanism changed; update this check"
        ]
    return _mcp_tool_literals(raw, blanked, span, family)


def _mcp_builtins() -> tuple[list[str], list[str]]:
    """The built-in family (src/mcp.rs): tool names in
    all_tools() assembly order. Built-ins are not
    literals in a vec — each is a `fn tool_*() -> Tool`
    constructor whose body builds one Tool with a
    closure handler, and all_tools() assembles the
    constructors explicitly. The census therefore
    proves the constructor set all_tools() calls is
    exactly the set defined, and reads each name out
    of its constructor body."""
    problems: list[str] = []
    raw = read("src/mcp.rs")
    blanked = blank_code(raw)
    prod = production_region(blanked)
    prod_raw = raw[: len(prod)]
    ctors: dict[str, str] = {}
    for m in _MCP_CTOR_RE.finditer(prod):
        fn_name = m.group(1)
        open_idx = prod.index("{", m.start())
        _, end = brace_block(prod, open_idx)
        names = _MCP_NAME_RE.findall(prod_raw[open_idx:end])
        lineno = raw.count("\n", 0, m.start()) + 1
        if len(names) != 1:
            problems.append(
                f"  built-ins: constructor {fn_name} (src/mcp.rs:{lineno}) "
                f"defines {len(names)} tool names, expected exactly 1 — "
                "the constructor pattern changed; update this check"
            )
        elif fn_name in ctors:
            problems.append(
                f"  built-ins: constructor {fn_name} is defined more than once in src/mcp.rs"
            )
        else:
            ctors[fn_name] = names[0]
    span = _mcp_fn_span(blanked, r"pub fn all_tools\s*\(\)[^{]*\{")
    if span is None:
        problems.append(
            "  built-ins: `pub fn all_tools()` not found in src/mcp.rs — "
            "the assembly changed; update this check"
        )
        return [], problems
    body = blanked[span[0] : span[1]]
    # The assembly must still extend the other
    # families, or their tools silently leave the
    # surface while their own censuses stay green.
    # (Wave Q: the two subtitle extends join the two
    # original ones — Q1 landed both in all_tools().)
    for ext, fam in (
        ("mcp_tools::tools()", "registry"),
        ("mcp_io::tools()", "io"),
        ("mcp_subtitle_io::tools()", "subtitle io"),
        ("mcp_subtitle_tools::tools()", "subtitle tools"),
    ):
        if ext not in body:
            problems.append(
                f"  built-ins: all_tools() no longer extends {ext} — the "
                f"{fam} family would silently drop off the MCP surface"
            )
    called = re.findall(r"\b(tool_[A-Za-z0-9_]*)\s*\(\s*\)", body)
    seen: set[str] = set()
    for fn_name in called:
        if fn_name in seen:
            problems.append(
                f"  built-ins: all_tools() assembles {fn_name} more than once — "
                "its tool would appear twice in tools/list"
            )
        seen.add(fn_name)
    for fn_name in sorted(set(ctors) - set(called)):
        problems.append(
            f"  built-ins: constructor {fn_name} (tool `{ctors[fn_name]}`) is "
            "defined in src/mcp.rs but never assembled by all_tools() — "
            "the tool exists in source and is unreachable at runtime"
        )
    for fn_name in sorted(set(called) - set(ctors)):
        problems.append(
            f"  built-ins: all_tools() assembles {fn_name}(), which no "
            "`fn ... -> Tool` constructor defines in src/mcp.rs"
        )
    names = [ctors[fn_name] for fn_name in called if fn_name in ctors]
    return names, problems


# Wave Q surface floor (Q5's recount, 2026-11-10 — the
# derivation is ledgered in this script's header, Wave Q
# section): the MCP surface may not silently shrink
# below the integrated Wave P+Q truth, even if
# docs/MCP.md were edited down to match a smaller
# surface (doc sync alone cannot see that). 381 = 44
# built-ins + 319 registry (317 at the Wave P opening +
# sprite_move_key / sprite_delete_key_at, P38 IA-2's
# splice into mcp_tools::tools()) + 9 io + 4 subtitle
# io (mcp_subtitle_io::tools()) + 5 subtitle tools
# (mcp_subtitle_tools::tools()). Until the sprite
# splice lands the surface reads 379 and this floor
# names the gap. Ratchet UP with a new ledger line when
# a wave adds tools; lower it only for a named,
# deliberate removal.
# Wave Y ratchet (Y16's recount, 2026-11-10): on the
# clean Wave X integration HEAD (d44098f, git archive
# scratch tree) the surface reads 390 = 44 built-ins +
# 328 registry (319 + X3's seven state tools + X5's
# sprite_set_asset / sprite_reorder splice) + 9 io +
# 4 subtitle io + 5 subtitle tools, doc sync exact.
# Floor raised 381 -> 390.
# Wave Z ratchet (Z13's baseline recount, 2026-11-10):
# on the clean Wave Y closing HEAD (5031700, detached
# git worktree) the surface reads 392 = 44 built-ins +
# 330 registry (328 + import_layered_set + set_rest_pose,
# Wave Y's two additions) + 9 io + 4 subtitle io + 5
# subtitle tools, doc sync exact (check 9 here and
# scripts/mcp_census.py both PASS on that baseline).
# Floor raised 390 -> 392.
MCP_SURFACE_FLOOR = 392


def check_mcp_surface() -> tuple[bool, list[str]]:
    lines: list[str] = []
    ok = True

    def fail(msg: str) -> None:
        nonlocal ok
        ok = False
        lines.append(msg)

    builtin_names, problems = _mcp_builtins()
    for p in problems:
        fail(p)
    registry, problems = _mcp_registry_family("src/mcp_tools.rs", "registry (src/mcp_tools.rs)")
    for p in problems:
        fail(p)
    io_tools, problems = _mcp_registry_family("src/mcp_io.rs", "io (src/mcp_io.rs)")
    for p in problems:
        fail(p)
    subtitle_io, problems = _mcp_registry_family(
        "src/mcp_subtitle_io.rs", "subtitle io (src/mcp_subtitle_io.rs)"
    )
    for p in problems:
        fail(p)
    subtitle_tools, problems = _mcp_registry_family(
        "src/mcp_subtitle_tools.rs", "subtitle tools (src/mcp_subtitle_tools.rs)"
    )
    for p in problems:
        fail(p)

    lines.append(
        f"  family census (all_tools() assembly order): built-ins {len(builtin_names)} "
        f"(constructors in src/mcp.rs), registry {len(registry)} "
        f"(Tool literals in mcp_tools::tools()), io {len(io_tools)} "
        f"(Tool literals in mcp_io::tools()), subtitle io {len(subtitle_io)} "
        f"(Tool literals in mcp_subtitle_io::tools()), subtitle tools "
        f"{len(subtitle_tools)} (Tool literals in mcp_subtitle_tools::tools())"
    )

    # Handler integrity: every registry / IO handler
    # must resolve to exactly one fn definition in
    # the src/ production tree (the handler may live
    # in a domain file after Wave 11 — the census is
    # tree-wide, not per-file). Built-ins carry
    # closure handlers inside their constructors, so
    # their resolution is the constructor census
    # above.
    paths = sorted(glob.glob(os.path.join(ROOT, "src", "**", "*.rs"), recursive=True))
    prod_by_rel: dict[str, str] = {}
    for full in paths:
        rel = os.path.relpath(full, ROOT)
        with open(full, encoding="utf-8") as fh:
            prod_by_rel[rel] = production_region(blank_code(fh.read()))

    def def_sites(ident: str) -> list[str]:
        pat = re.compile(rf"fn\s+{re.escape(ident)}\s*\(")
        sites: list[str] = []
        for rel, prod in prod_by_rel.items():
            for m in pat.finditer(prod):
                lineno = prod.count("\n", 0, m.start()) + 1
                sites.append(f"{rel}:{lineno}")
        # Wave 14: the 31 live-only shell stubs are stamped by
        # the `shell_unavailable_fn!` macro in src/mcp_tools.rs
        # (capturing closures do not coerce to the registry's
        # fn-pointer handler type). Each invocation
        # `shell_unavailable_fn!(<ident>, "<tool>");` expands to
        # exactly one `fn <ident>(...)` definition, so an
        # invocation counts as that handler's definition site;
        # the exactly-one-definition invariant is unchanged.
        mpat = re.compile(
            rf"shell_unavailable_fn!\(\s*{re.escape(ident)}\s*,"
        )
        for rel, prod in prod_by_rel.items():
            for m in mpat.finditer(prod):
                lineno = prod.count("\n", 0, m.start()) + 1
                sites.append(f"{rel}:{lineno} (shell_unavailable_fn! stamp)")
        return sites

    resolved = 0
    for family, entries in (
        ("registry", registry),
        ("io", io_tools),
        ("subtitle io", subtitle_io),
        ("subtitle tools", subtitle_tools),
    ):
        for name, handler in entries:
            if handler == "|":
                fail(
                    f"  {family}: tool `{name}` has a closure handler — "
                    "registry/IO handlers must be named fns so this census "
                    "can resolve them (closures are the built-ins' shape, "
                    "censused via their constructors)"
                )
                continue
            leaf = handler.split("::")[-1]
            sites = def_sites(leaf)
            if len(sites) == 1:
                resolved += 1
            elif not sites:
                fail(
                    f"  {family}: tool `{name}` names handler `{handler}`, "
                    "which no `fn` in src/ defines — the tool is registered "
                    "but its handler does not exist"
                )
            else:
                fail(
                    f"  {family}: tool `{name}` names handler `{handler}`, "
                    f"defined {len(sites)} times ({', '.join(sites)}) — a "
                    "handler must resolve to exactly one definition"
                )
    if resolved:
        lines.append(
            f"  handler integrity: {resolved} registry/io/subtitle handlers each resolve "
            "to exactly one fn definition in src/"
        )

    # Name uniqueness across ALL families, in
    # assembly order. Dispatch is first-match
    # (tools_call_result's iter().find over
    # all_tools()): a name claimed by two families
    # makes the later family's tool unreachable,
    # and tools/list would carry the name twice.
    families: list[tuple[str, list[str]]] = [
        ("built-ins", builtin_names),
        ("registry", [n for n, _ in registry]),
        ("io", [n for n, _ in io_tools]),
        ("subtitle io", [n for n, _ in subtitle_io]),
        ("subtitle tools", [n for n, _ in subtitle_tools]),
    ]
    owner: dict[str, str] = {}
    surface: list[str] = []
    for fam, names in families:
        for name in names:
            if name in owner:
                if owner[name] == fam:
                    fail(
                        f"  uniqueness: tool `{name}` is registered twice in "
                        f"the {fam} family — tools/list would carry it twice"
                    )
                else:
                    fail(
                        f"  uniqueness: tool `{name}` is claimed by both "
                        f"{owner[name]} and {fam} — dispatch takes the first "
                        f"match in all_tools() order, so the {fam} tool is "
                        "unreachable and tools/list would carry the name twice"
                    )
            else:
                owner[name] = fam
            surface.append(name)
    surface_set = set(surface)
    lines.append(
        f"  surface: {len(surface)} tools, {len(surface_set)} unique names "
        "(built-ins, then registry, then io, then subtitle io, then "
        "subtitle tools — the all_tools() order)"
    )

    # Surface floor (Wave Q): doc sync below compares
    # the surface against the doc, so a tool removed
    # TOGETHER with its catalogue row would pass it —
    # the floor is what makes the surface unable to
    # shrink silently (check 9's founding purpose).
    if len(surface) < MCP_SURFACE_FLOOR:
        fail(
            f"  surface floor: {len(surface)} tools is below the "
            f"floor of {MCP_SURFACE_FLOOR} (ledger at MCP_SURFACE_FLOOR) — "
            "a tool has left the surface, or a splice the floor "
            "counts has not landed yet"
        )
    else:
        lines.append(
            f"  surface floor: {len(surface)} tools >= the floor "
            f"of {MCP_SURFACE_FLOOR} (ledger at MCP_SURFACE_FLOOR)"
        )

    # Doc sync: docs/MCP.md's catalogue (the tables
    # under `## Catalogue`, first cell of each row)
    # must list exactly the surface set.
    doc_path = os.path.join(ROOT, "docs", "MCP.md")
    if not os.path.isfile(doc_path):
        fail("  doc sync: docs/MCP.md is missing — the MCP contract doc is gone")
        return ok, lines
    doc = read("docs/MCP.md")
    if "## Catalogue" not in doc:
        fail(
            "  doc sync: docs/MCP.md has no `## Catalogue` section — the "
            "catalogue pattern changed; update this check"
        )
        return ok, lines
    catalogue = doc.split("## Catalogue", 1)[1].split("\n## ", 1)[0]
    doc_names = _MCP_DOC_ROW_RE.findall(catalogue)
    if not doc_names:
        fail(
            "  doc sync: the catalogue in docs/MCP.md lists ZERO tool rows "
            "(`| `name` |` table rows) — the catalogue format changed; "
            "update this check (a silent zero is what this census prevents)"
        )
        return ok, lines
    doc_set = set(doc_names)
    doc_dupes = sorted(n for n in doc_set if doc_names.count(n) > 1)
    for name in doc_dupes:
        fail(f"  doc sync: docs/MCP.md's catalogue lists `{name}` more than once")
    for name in sorted(doc_set - surface_set):
        fail(
            f"  doc sync: docs/MCP.md's catalogue documents `{name}`, which "
            "no family registers — documented-but-nonexistent (the doc's own "
            "rule: do not document a tool that does not exist in all_tools())"
        )
    for name in sorted(surface_set - doc_set):
        fail(
            f"  doc sync: tool `{name}` ({owner.get(name, '?')} family) is "
            "registered but missing from docs/MCP.md's catalogue — "
            "existing-but-undocumented"
        )
    if doc_set == surface_set and not doc_dupes:
        lines.append(
            f"  doc sync: docs/MCP.md's catalogue lists exactly the "
            f"{len(doc_set)} surface tools — no documented-but-nonexistent, "
            "no existing-but-undocumented"
        )
    return ok, lines


# ----------------------------------------------------------------
# Check 10 — parity (Wave 14): every palette command is
# covered by MCP tools that exist, pending on a named
# tool a Wave 14 squad has not spliced yet, or excluded
# with an argued reason. The command set is derived
# from command_palette.rs (raw text — command keys are
# string literals, check 9's idiom) and the surface
# from check 9's family extractors, reused verbatim.
# ----------------------------------------------------------------

# command -> the tools that cover it. Every named tool
# must exist in the derived surface. Read-modify-write
# pairs are listed where the command is a relative
# step (lengthen, thicken, zoom, the audio "last clip"
# family): the read tool supplies the current value
# the step starts from. Selection-relative commands
# name the tool that takes the index explicitly —
# the GUI selection itself is shell state (Squad B).
PARITY_MAP: dict[str, list[str]] = {
    # Transport
    "play": ["set_playback"],
    "prev": ["set_frame"],
    "next": ["set_frame"],
    "first-frame": ["set_frame"],
    "restart-playback": ["set_playback"],
    "prev-keyframe": ["goto_prev_keyframe"],
    "next-keyframe": ["goto_next_keyframe"],
    # Frames
    "add-frame": ["add_frame"],
    "dup-frame": ["duplicate_frame"],
    "del-frame": ["delete_frame"],
    "inbetween": ["insert_inbetween"],
    "copy-frame": ["copy_frame"],
    "paste-frame": ["paste_frame"],
    "fps-up": ["set_project_fps"],
    "fps-down": ["set_project_fps"],
    "swap-frame-prev": ["swap_frame_with_neighbour"],
    "swap-frame-next": ["swap_frame_with_neighbour"],
    # Edit
    "undo": ["undo"],
    "redo": ["redo"],
    # Projects
    "save": ["save_project"],
    "load": ["load_project"],
    "save-named": ["save_named_project"],
    "save-copy": ["save_project_copy"],
    "cycle-project": ["list_projects", "load_project"],
    "open-project": ["load_project"],
    # Wave Y (Y6): the layered-set import command and
    # its registry tool are the same importer call.
    "import-layered-set": ["import_layered_set"],
    # Figures (hide / solo / lock are the `layers`
    # tool's figure flags — its handler writes the
    # same fields the toggle ops write)
    "add-figure": ["add_figure"],
    "add-quadruped": ["add_figure"],  # the tool's "quadruped" kind
    "del-figure": ["delete_figure"],
    "duplicate-figure": ["figure_duplicate"],
    "hide-figure": ["layers"],
    "solo-figure": ["layers"],
    "lock-figure": ["layers"],
    "mirror-pose": ["mirror_pose"],
    "mirror-figure": ["mirror_figure"],
    "insert-library-figure": ["figure_library_insert"],
    # Segments (segments are nodes: add_node's handler
    # calls the same add_segment op the shim calls)
    "add-segment": ["add_node"],
    "del-segment": ["delete_node"],
    "len-up": ["get_node", "set_node"],
    "len-down": ["get_node", "set_node"],
    "thick-up": ["get_node", "set_node"],
    "thick-down": ["get_node", "set_node"],
    "cycle-shape": ["get_node", "set_node"],
    "cycle-bend": ["set_bend"],
    "cycle-node-color": ["get_node", "set_node"],
    "attach-media-import": ["attach_image_to_node"],
    "clear-node-media": ["clear_node_media"],
    "cycle-attached-sprite": ["assets_list", "attach_image_to_node"],
    "add-sprite": ["sprite_add"],
    "remove-sprite": ["sprite_list", "sprite_remove"],
    # Camera (zoom-by is get + set at the current
    # frame; follow-clear is set_camera_follow null)
    "cam-zoom-in": ["get_camera", "set_camera"],
    "cam-zoom-out": ["get_camera", "set_camera"],
    "cam-frame-all": ["frame_all_figures"],
    "cam-centre": ["centre_on_selection"],
    "cam-clear-key": ["delete_camera_key"],
    "cam-follow": ["set_camera_follow"],
    "cam-follow-clear": ["set_camera_follow"],
    # Graph
    "key-graph-angle": ["graph_key_angle"],
    "key-all-channels": ["graph_key_all_channels"],
    "clear-graph": ["graph_clear_channels", "graph_clear_channel_keys"],
    "shift-channel-right": ["graph_shift_keys"],
    "fill-keys-range": ["graph_fill_keys"],
    # View (core-backed only; shell-backed view
    # commands are in PARITY_PENDING below)
    "tween-play": ["set_tween_playback"],
    "cycle-easing": ["set_project_easing"],
    # Draw
    "cycle-brush": ["cycle_brush"],
    "draw-tool": ["set_draw_tool"],
    "clear-drawings": ["clear_frame_strokes"],
    "mirror-strokes": ["mirror_strokes"],
    "stamp-polygon": ["draw_polygon"],
    "delete-last-stroke": ["list_strokes", "delete_stroke"],
    "delete-stroke": ["delete_stroke"],
    # Effects (burst / sparkle are particle_add_event's
    # anchored sibling: kind + figure/node anchor)
    "particle-burst": ["particle_add_anchored"],
    "particle-sparkle": ["particle_add_anchored"],
    "particle-add-event": ["particle_add_event"],
    "fx-clip-confetti": ["add_fx_clip"],
    "fx-clip-lightning": ["add_fx_clip"],
    "clear-particles": ["clear_particles"],
    "split-fx-clip": ["fx_split_clip"],
    "duplicate-particle": ["particle_duplicate_event"],
    "delete-particle": ["particle_delete_event"],
    # Audio (the "last clip" commands: list supplies
    # the last index, the op tool takes it explicitly)
    "add-audio": ["add_audio_clip"],
    "mixdown": ["audio_mixdown"],
    "audio-echo": ["audio_list_clips", "audio_echo"],
    "audio-lowpass": ["audio_list_clips", "audio_lowpass"],
    "audio-trim-silence": ["audio_list_clips", "audio_trim_silence"],
    "audio-split-silences": ["audio_list_clips", "audio_split_silences"],
    "fade-audio": ["audio_list_clips", "audio_fade"],
    "reverse-audio": ["audio_list_clips", "audio_reverse"],
    "delete-audio-clip": ["audio_list_clips", "audio_delete_clip"],
    "normalise-audio": ["audio_normalise_all"],
    "mute-audio-clip": ["audio_clip_toggle_muted"],
    # Export (one format-arg tool covers all six)
    "export-png": ["export_file"],
    "export-gif": ["export_file"],
    "export-mp4": ["export_file"],
    "export-webm": ["export_file"],
    "export-piv": ["export_file"],
    "export-stk": ["export_file"],
    # 3D
    "enter-3d": ["fig3_enter"],
    "exit-3d": ["fig3_exit"],
    # Style (copy/paste style is a style read applied
    # to another node: get_node_style + set_node_style)
    "style-preset-ink": ["apply_style_preset"],
    "style-preset-crimson": ["apply_style_preset"],
    "style-preset-ocean": ["apply_style_preset"],
    "style-preset-forest": ["apply_style_preset"],
    "style-preset-violet": ["apply_style_preset"],
    "style-preset-amber": ["apply_style_preset"],
    "style-cap-round": ["set_node_cap"],
    "style-cap-square": ["set_node_cap"],
    "style-cap-point": ["set_node_cap"],
    "cycle-taper": ["cycle_node_taper"],
    "style-opacity-up": ["get_node_style", "set_node_opacity"],
    "style-opacity-down": ["get_node_style", "set_node_opacity"],
    "copy-style": ["get_node_style"],
    "paste-style": ["set_node_style"],
    # Wave 14 — shell control + the last core settings
    # handlers (moved from PARITY_PENDING at the wave's
    # integration, under the LANDED tool names).
    "go-animate": ["shell_set_workspace"],
    "go-draw": ["shell_set_workspace"],
    "go-graph": ["shell_set_workspace"],
    "go-effects": ["shell_set_workspace"],
    "go-audio": ["shell_set_workspace"],
    "go-build3d": ["shell_set_workspace"],
    "go-export": ["shell_set_workspace"],
    "go-home": ["shell_set_workspace"],
    # The command selects Animate and forces the right
    # column open: workspace + sidebar, one each.
    "open-style-inspector": ["shell_set_workspace", "shell_set_sidebar"],
    "toggle-loop-playback": ["shell_set_setting"],
    "theme-light": ["shell_set_theme"],
    "theme-dark": ["shell_set_theme"],
    "toggle-guides": ["shell_set_setting"],
    "toggle-frustum": ["shell_set_setting"],
    "toggle-safe-frame": ["shell_set_setting"],
    "toggle-live-mcp": ["shell_set_setting"],
    "toggle-left-panel": ["shell_set_sidebar"],
    "toggle-right-panel": ["shell_set_sidebar"],
    "onion": ["set_onion_skin"],
    "draw-mode": ["set_draw_mode"],
    # Stroke selection stepping: the shell selection is
    # settable by explicit index and readable back, so
    # stepping composes (read, select ±1) — covered, no
    # longer an exclusion.
    "stroke-select-next": ["shell_get_selection", "shell_select_stroke"],
    "stroke-select-prev": ["shell_get_selection", "shell_select_stroke"],
}

# command -> (tool that will cover it, owning squad).
# PENDING IS A FAILURE while the named tool is absent
# from the surface: these are a wave's splices that
# have not landed yet. When a tool lands, its entries
# read RESOLVED and pass; the integrator then moves
# them into PARITY_MAP above, under the landed names.
PARITY_PENDING: dict[str, tuple[str, str]] = {
    # EMPTY at the Wave 14 close: all twenty entering
    # entries resolved at integration and moved into
    # PARITY_MAP above under their landed names
    # (shell_set_workspace, shell_set_setting,
    # shell_set_theme, shell_set_sidebar, set_onion_skin,
    # set_draw_mode). A future wave's unlanded splices
    # belong here, failing until they land.
}

# command -> why no tool covers it, by design. An
# exclusion must argue that no capability is lost:
# the command's effect is a GUI-local cursor whose
# target every tool already takes explicitly.
PARITY_EXCLUSIONS: dict[str, str] = {
    # EMPTY at the Wave 14 close: the two entering
    # exclusions (stroke-select-next/prev) became
    # covered when shell_select_stroke landed — the
    # stepping argument only held while the stroke
    # selection had no tool at all. An exclusion must
    # argue that no capability is lost; nothing here
    # currently needs that argument.
    # Still EMPTY after the Focused Wave (post-Wave
    # 14): its G5/G6/G7 items were weighed for this
    # table and do not belong in it — they are panel
    # and shell capabilities with no palette command
    # (see the Focused Wave note in this script's
    # header), so there is no command key to exclude;
    # their dispositions live in the PARITY_CENSUS
    # close-out addendum.
}

_PALETTE_CMD_RE = re.compile(r'Command\s*\{\s*key:\s*"([^"]+)"')


def _palette_commands() -> tuple[list[str], list[str]]:
    """The palette's command keys, in table order,
    read from the RAW command_palette.rs (the keys
    are string literals, which blanking erases).
    Duplicate keys are census problems, never
    silently deduplicated."""
    raw = read("src/rune_sec/command_palette.rs")
    keys = _PALETTE_CMD_RE.findall(raw)
    problems: list[str] = []
    if not keys:
        problems.append(
            "  palette derivation: ZERO `Command { key:` entries found in "
            "src/rune_sec/command_palette.rs — the declaration pattern "
            "changed; update this check (a silent zero is exactly what "
            "this census exists to prevent)"
        )
    seen: set[str] = set()
    for key in keys:
        if key in seen:
            problems.append(
                f"  palette derivation: command key `{key}` appears more "
                "than once in the COMMANDS table — the palette itself "
                "would list it twice"
            )
        seen.add(key)
    return keys, problems


def check_parity() -> tuple[bool, list[str]]:
    lines: list[str] = []
    ok = True

    def fail(msg: str) -> None:
        nonlocal ok
        ok = False
        lines.append(msg)

    # The surface, from check 9's own extractors, so
    # the two checks can never disagree about it.
    builtin_names, problems = _mcp_builtins()
    for p in problems:
        fail(p.replace("built-ins:", "surface (via check 9's extractors):", 1))
    registry, problems = _mcp_registry_family("src/mcp_tools.rs", "registry (src/mcp_tools.rs)")
    for p in problems:
        fail(p)
    io_tools, problems = _mcp_registry_family("src/mcp_io.rs", "io (src/mcp_io.rs)")
    for p in problems:
        fail(p)
    subtitle_io, problems = _mcp_registry_family(
        "src/mcp_subtitle_io.rs", "subtitle io (src/mcp_subtitle_io.rs)"
    )
    for p in problems:
        fail(p)
    subtitle_tools, problems = _mcp_registry_family(
        "src/mcp_subtitle_tools.rs", "subtitle tools (src/mcp_subtitle_tools.rs)"
    )
    for p in problems:
        fail(p)
    surface = (
        set(builtin_names)
        | {n for n, _ in registry}
        | {n for n, _ in io_tools}
        | {n for n, _ in subtitle_io}
        | {n for n, _ in subtitle_tools}
    )

    commands, problems = _palette_commands()
    for p in problems:
        fail(p)
    command_set = set(commands)
    lines.append(
        f"  derived from source: {len(commands)} palette commands "
        f"(command_palette.rs), {len(surface)} MCP tools (check 9's "
        "family extractors: built-ins + registry + io + subtitle io "
        "+ subtitle tools)"
    )
    if not command_set or not surface:
        lines.append("  parity cannot be assessed on an empty derivation — see above")
        return ok, lines

    # Table hygiene, both directions: an entry naming
    # a command the palette no longer has is a stale
    # table; a command in two tables is ambiguous
    # accounting. Both fail, by name.
    tables = (
        ("PARITY_MAP", set(PARITY_MAP)),
        ("PARITY_PENDING", set(PARITY_PENDING)),
        ("PARITY_EXCLUSIONS", set(PARITY_EXCLUSIONS)),
    )
    for (name_a, set_a), (name_b, set_b) in (
        (tables[0], tables[1]),
        (tables[0], tables[2]),
        (tables[1], tables[2]),
    ):
        for cmd in sorted(set_a & set_b):
            fail(
                f"  hygiene: command `{cmd}` is in both {name_a} and "
                f"{name_b} — a command is covered, pending or excluded, "
                "never two of those"
            )
    for name, keys_in_table in tables:
        for cmd in sorted(keys_in_table - command_set):
            fail(
                f"  hygiene: {name} names command `{cmd}`, which the "
                "palette does not have — the table is stale; remove the "
                "entry or restore the command"
            )

    # Coverage: every command is in exactly one table
    # (the overlap failures above cover the 'two'
    # case; this is the 'none' case).
    accounted = set(PARITY_MAP) | set(PARITY_PENDING) | set(PARITY_EXCLUSIONS)
    for cmd in commands:
        if cmd not in accounted:
            fail(
                f"  coverage: palette command `{cmd}` is in no parity "
                "table — map it to the tools that cover it, pend it on "
                "the tool that will, or exclude it with an argued reason"
            )

    # Mapped coverage: every tool a mapping names must
    # exist in the surface. A tool that leaves the
    # surface takes its commands' coverage with it,
    # and this is where that gets said.
    referenced: set[str] = set()
    for cmd, tools in PARITY_MAP.items():
        if cmd not in command_set:
            continue  # already failed as stale above
        if not tools:
            fail(f"  mapped: command `{cmd}` maps to an EMPTY tool list — a mapping must name its tools")
        for tool in tools:
            referenced.add(tool)
            if tool not in surface:
                fail(
                    f"  mapped: command `{cmd}` is mapped to tool "
                    f"`{tool}`, which no MCP family registers — the "
                    "coverage is claimed but the tool does not exist"
                )
    if referenced:
        lines.append(
            f"  mapped: {len(PARITY_MAP)} commands covered by "
            f"{len(referenced)} distinct tools, all present in the surface"
        )

    # Pending: a failure per command until its tool
    # lands; RESOLVED (and passing) once it has.
    pending_open = 0
    for cmd in commands:
        if cmd not in PARITY_PENDING:
            continue
        tool, owner = PARITY_PENDING[cmd]
        if tool in surface:
            lines.append(
                f"  pending RESOLVED: command `{cmd}` — tool `{tool}` "
                f"({owner}) is now in the surface; move this entry to "
                "PARITY_MAP at close-out"
            )
        else:
            pending_open += 1
            fail(
                f"  pending: command `{cmd}` waits on tool `{tool}` "
                f"({owner}), which no MCP family registers yet — the "
                "command is not MCP-reachable until that splice lands"
            )
    if pending_open == 0 and PARITY_PENDING:
        lines.append(
            f"  pending: all {len(PARITY_PENDING)} pending entries resolved "
            "(their tools are in the surface)"
        )

    if PARITY_EXCLUSIONS:
        lines.append(
            f"  excluded: {len(PARITY_EXCLUSIONS)} commands excluded with "
            "argued reasons (see PARITY_EXCLUSIONS in this script)"
        )
    return ok, lines


# ----------------------------------------------------------------
# Main
# ----------------------------------------------------------------


def main() -> int:
    base = sys.argv[1] if len(sys.argv) > 1 else "HEAD"
    failures = 0

    def report(title: str, ok: bool, lines: list[str]) -> None:
        nonlocal failures
        print(f"[{'PASS' if ok else 'FAIL'}] {title}")
        for line in lines:
            print(f"       {line}")
        if not ok:
            failures += 1

    print("Phase J+K+L+M pre-commit checks")
    print(f"Repo: {ROOT}")
    print(f"Key-diff base ref: {base}")
    print()

    s, d, t, nf = inventory_counts()
    report(
        "1. INVENTORY",
        t >= KEY_TOTAL_FLOOR and nf > 0,
        [
            f"static keys: {s}",
            f"dynamic patterns: {d}",
            f"total unique keys: {t}",
            f"files scanned: {nf}",
            f"floor: >= {KEY_TOTAL_FLOOR} total unique keys (Wave Q ledger at KEY_TOTAL_FLOOR)",
        ],
    )

    ok, lines = check_key_diff(base)
    report(f"2. KEY DIFF (vs {base})", ok, lines)

    fields = state_fields()
    ok, lines = check_state_literals(fields)
    report("3. STATE LITERAL CENSUS", ok, lines)

    ok, lines = check_callers()
    report("4. CALLER CENSUS (Phase J+K+L+M lib ops)", ok, lines)

    claims = count_claims()
    claim_lines = [f"{label}: {n}  ({how})" for label, n, how in claims]
    report(
        "5. COUNT CLAIMS (docs must match these)",
        all(n > 0 for _, n, _ in claims),
        claim_lines,
    )

    ok, lines = check_design_adoption()
    report("6. DESIGN ADOPTION (Phase N design-kit census)", ok, lines)

    ok, lines = check_literal_guard()
    report("7. LITERAL & EMPTY-STATE GUARD (UI mission Wave 9)", ok, lines)

    ok, lines = check_coverage()
    report("8. COVERAGE (scenes <-> headless.yml; layout-fact sequence)", ok, lines)

    ok, lines = check_mcp_surface()
    report("9. MCP SURFACE CENSUS & INTEGRITY (Wave 11)", ok, lines)

    ok, lines = check_parity()
    report("10. PARITY (palette commands <-> MCP tools, Wave 14)", ok, lines)

    print()
    if failures:
        print(f"RESULT: FAIL — {failures} check(s) failed")
        return 1
    print("RESULT: PASS — all checks green")
    return 0


if __name__ == "__main__":
    sys.exit(main())
