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
# clip_duration_secs — audio_panel.rs;
# simplify_stroke_at — stroke_inspector.rs;
# set_key_interp — keyframe_tools.rs.
PHASE_L_OPS = [
    "set_head_ring_scale",
    "clip_duration_secs",
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
        t > 0 and nf > 0,
        [f"static keys: {s}", f"dynamic patterns: {d}", f"total unique keys: {t}", f"files scanned: {nf}"],
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

    print()
    if failures:
        print(f"RESULT: FAIL — {failures} check(s) failed")
        return 1
    print("RESULT: PASS — all checks green")
    return 0


if __name__ == "__main__":
    sys.exit(main())
