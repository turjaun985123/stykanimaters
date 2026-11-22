#!/usr/bin/env python3
"""Systematic caller census for StykAnimate-RS (Wave 3,
IA-U3-89-2 — the successor instrument to gate check 4).

Gate check 4 (scripts/phase_j_checks.py) censuses a
HAND-MAINTAINED list of 28 lib ops (24 Phase J/K/L/M
census ops + 4 same-file ops). U3-89's blind spot B8
named the structural
defect: the wave's signature failure — built + unit-tested
+ never wired — struck at least ten times in Wave 3's
audit (U3-99's Ledger 2 verifies TWENTY cases), and NOT ONE
of those functions was on check 4's list. A census that
depends on the same memory whose failure it exists to
catch will keep missing exactly the failures that matter.

This script derives the watch list from the tree itself:

  * Every `fn` definition in `src/**/*.rs` is located on a
    masked copy of the source (comments / strings / chars /
    raw strings blanked, newlines preserved — the masking
    lexer is adapted from scripts/mcp_census.py).
  * Production region = everything except `#[cfg(test)]`
    modules (inline blocks, the trailing `mod tests`, and
    whole files declared as cfg(test) modules such as
    rune_sec/snap_tests.rs), `#[cfg(test)]`-attributed fns,
    and the `tests/` directory (never scanned — it is not
    under src/). Test-region call sites NEVER count as
    caller evidence: a fn called only by tests is exactly
    the disease this census exists to catch.
  * The FAIL-bearing universe is the set of `pub` fns
    (pub / pub(crate) / pub(super)) in production regions,
    minus the systematic exemption classes below. Private
    fns are counted report-only (the compiler's dead_code
    lint owns them; the disease class is pub API surface
    built for a consumer that never came).
  * A call site is a production occurrence of the fn's
    name token outside its definition site and outside
    `use` statements. Counting is NAME-based, like gate
    check 4 and mcp_census: same-named fns in different
    files share one caller pool, so a zero-caller verdict
    requires the NAME to have zero production occurrences
    anywhere. That biases the instrument toward false
    WIRED, never false UNWIRED — the safe direction for a
    tripwire (see LIMITATIONS).

EXEMPTION CLASSES (systematic — derived, not listed):

  * Trait declarations and trait-impl methods: dispatch
    goes through the trait; the name token need not appear
    at any call site (Display::fmt via format! is the
    canonical case). Fns inside a `trait { ... }` block or
    an `impl ... for ... { ... }` block are exempt.
  * `main` (bin entry points) and `#[no_mangle]` /
    `extern` fns (FFI surface): called from outside Rust.
  * Serde helper fns are NOT exempt — they are counted as
    referenced: `default = "..."`, `serialize_with`,
    `deserialize_with`, `with` attribute strings are read
    from the RAW source (masking blanks them) and recorded
    as production references.

THE KNOWN LEDGER (named — the PARITY_EXCLUSIONS pattern):

  Twenty cases from U3-99's Ledger 2 are seeded below in
  three classes, each with its ledger reference and the
  owning build slice that will land it:

  * KNOWN-ZERO — no production caller anywhere is the
    expected state today (the piece is unmounted /
    unwired). Verified condition: cross-file call sites
    == 0 AND same-file production call sites == 0.
  * KNOWN-INTERNAL — the placebo pattern: the fn is read
    inside its own file (its own settings section renders
    it) but has NO consumer outside the file. Verified
    condition: cross-file call sites == 0.
  * KNOWN-DOOR — the core is wired, but only from the MCP
    surface / its own file's handlers; no GUI file calls
    it, so a user cannot reach it. Verified condition:
    >= 1 production call site AND 0 call sites in GUI
    files (src/rune_sec/** except the headless-fixture
    module scenes.rs, plus main.rs / chrome_view.rs /
    stage_view.rs / graph_panel.rs).

  A ledger entry whose condition stops holding — a caller
  landed, the door opened, the fn was renamed, moved or
  deleted — reports STALE and FAILS the census. That is
  deliberate, and it is the gate's own ratchet philosophy
  (check 10's PARITY_PENDING): the ledger is edited in the
  same push as the fix, so the exception list can only
  shrink toward zero, never quietly rot.

  Two Ledger-2 entries are recorded NOT-CENSUSABLE and
  printed as such (never silently dropped): KeySel::clear
  (U-15 — the method name `clear` collides with ubiquitous
  std methods; a name-token census cannot isolate it) —
  and the non-fn entries (U-01a cache key, U-10 missing
  panel fields, U-17 migration reader, U-19 picker UI —
  the underlying ops ARE wired) are listed as out-of-scope
  notes in the ledger comments. One second-order limit is
  stated plainly: the census detects zero-caller HEADS.
  dialogs::open_file / save_file read WIRED because
  EditorCore::open_via_dialog / save_via_dialog call them —
  and those heads are themselves KNOWN-ZERO. Transitive
  deadness below a dead head is not detected.

LEGACY WATCH: gate check 4's 32 op names are embedded
below and must ALL read wired (cross-file or same-file)
under this census. The gate is green at HEAD, so any
disagreement indicts THIS instrument, not the tree — the
cross-check runs on every invocation.

VERDICTS / EXIT CODES (house style):
  RESULT: PASS       exit 0 — no new unwired fn, ledger
                     exact, legacy watch agrees.
  RESULT: FAIL       exit 1 — NEW-UNWIRED fn(s) and/or
                     STALE ledger entries and/or legacy
                     disagreement. Every failure names the
                     fn, its definition site, and the remedy.
  RESULT: INCOMPLETE exit 2 — the instrument itself could
                     not run (no sources, no definitions):
                     it refuses to print a verdict, like
                     mcp_census refuses a stale census.

Read-only, stdlib only, deterministic (entries sorted, no
timestamps, repo-relative paths). Run from anywhere:

    python3 scripts/caller_census.py [--root DIR]
    python3 scripts/caller_census.py --json
    python3 scripts/caller_census.py --self-test
"""

import bisect
import glob
import json
import os
import re
import sys


# ---------------------------------------------------------------------
# Source handling: masking lexer + balanced spans.
# Adapted from scripts/mcp_census.py (the reference
# implementation): span location runs on a MASKED copy —
# same length, string/char/comment contents replaced by
# spaces, newlines preserved — while literal extraction
# (serde attribute strings) reads the RAW text. Line
# numbers survive masking.
# ---------------------------------------------------------------------

_CHAR_RE = re.compile(r"'(\\.|[^\\'])'")


def mask_source(src: str) -> str:
    out = list(src)
    i, n = 0, len(src)
    while i < n:
        ch = src[i]
        nxt = src[i + 1] if i + 1 < n else ""
        if ch == "/" and nxt == "/":
            j = src.find("\n", i)
            j = n if j < 0 else j
            for k in range(i, j):
                out[k] = " "
            i = j
        elif ch == "/" and nxt == "*":
            depth, j = 0, i
            while j < n:
                if src.startswith("/*", j):
                    depth += 1
                    j += 2
                elif src.startswith("*/", j):
                    depth -= 1
                    j += 2
                    if depth == 0:
                        break
                else:
                    j += 1
            for k in range(i, min(j, n)):
                if out[k] != "\n":
                    out[k] = " "
            i = j
        elif ch == "r" and (nxt == '"' or nxt == "#"):
            # Raw string: r"..." or r#*"..."#*
            j = i + 1
            hashes = 0
            while j < n and src[j] == "#":
                hashes += 1
                j += 1
            if j < n and src[j] == '"':
                closer = '"' + "#" * hashes
                end = src.find(closer, j + 1)
                end = n if end < 0 else end + len(closer)
                for k in range(i, end):
                    if out[k] != "\n":
                        out[k] = " "
                i = end
            else:
                i += 1
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
            m = _CHAR_RE.match(src, i)
            if m:
                for k in range(i, m.end()):
                    out[k] = " "
                i = m.end()
            else:
                i += 1  # a lifetime, not a char literal
        else:
            i += 1
    return "".join(out)


def brace_span(masked: str, open_brace: int) -> tuple[int, int]:
    """(start, end) offsets of the brace-balanced block
    whose `{` sits at `open_brace` in masked text."""
    depth = 0
    i = open_brace
    while i < len(masked):
        ch = masked[i]
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                return open_brace, i + 1
        i += 1
    raise ValueError("unbalanced braces")


# ---------------------------------------------------------------------
# Test regions. A file's production region is the file
# minus: (a) the trailing #[cfg(test)] mod tests region —
# gate check 4's convention, cut at the DECLARATION, not
# the first #[cfg(test)] anywhere (main.rs declares its
# cfg(test) snap_tests module near the top); (b) any
# #[cfg(test)] mod block, whatever its name; (c) any
# `mod tests { ... }` block. Whole cfg(test) FILE modules
# (declared `#[cfg(test)] mod name;` in a parent) are
# excluded file-wide by stem in build() below.
# ---------------------------------------------------------------------

_TRAILING_TESTS_RE = re.compile(
    r"(?m)^[ \t]*#\[cfg\(test\)\][ \t]*\n[ \t]*(?:pub[ \t]+)?mod[ \t]+tests\b"
)
_CFG_MOD_RE = re.compile(
    r"#\[cfg\(test\)\]\s*(?:pub\s+)?mod\s+[A-Za-z_]\w*\s*\{"
)
_MOD_TESTS_RE = re.compile(r"\bmod\s+tests\s*\{")
_MOD_DECL_RE = re.compile(r"\bmod\s+([A-Za-z_]\w*)\s*;")


def test_spans(masked: str) -> list[tuple[int, int]]:
    spans: list[tuple[int, int]] = []
    m = _TRAILING_TESTS_RE.search(masked)
    if m:
        spans.append((m.start(), len(masked)))
    for rx in (_CFG_MOD_RE, _MOD_TESTS_RE):
        for mm in rx.finditer(masked):
            try:
                spans.append(brace_span(masked, mm.end() - 1))
            except ValueError:
                pass
    return spans


def in_spans(spans: list[tuple[int, int]], offset: int) -> bool:
    return any(a <= offset < b for a, b in spans)


class Source:
    def __init__(self, root: str, rel: str):
        self.rel = rel
        with open(os.path.join(root, rel), encoding="utf-8") as fh:
            self.raw = fh.read()
        self.masked = mask_source(self.raw)
        self._starts = [0]
        for m in re.finditer("\n", self.raw):
            self._starts.append(m.end())
        self.tspans = test_spans(self.masked)

    def line_of(self, offset: int) -> int:
        return bisect.bisect_right(self._starts, offset)

    def is_test_offset(self, offset: int) -> bool:
        return in_spans(self.tspans, offset)


# ---------------------------------------------------------------------
# File classes for the KNOWN-DOOR condition. GUI = the
# shell a user actually touches: rune_sec panels + the
# bin-crate stage/chrome files. scenes.rs is EXCLUDED —
# it is the headless fixture module; its plants are not a
# user door (U3-99 Ledger 2, U-09 makes exactly this
# distinction for chapter ops). MCP = the tool surface.
# ---------------------------------------------------------------------

def is_gui_file(rel: str) -> bool:
    if rel == "src/rune_sec/scenes.rs":
        return False
    return (
        rel.startswith("src/rune_sec/")
        or rel.startswith("src/stage_view/")
        or rel
        in (
            "src/main.rs",
            "src/chrome_view.rs",
            "src/stage_view.rs",
            "src/graph_panel.rs",
        )
    )


def is_mcp_file(rel: str) -> bool:
    base = os.path.basename(rel)
    return (
        (rel.startswith("src/") and base.startswith("mcp"))
        or rel.startswith("src/shell_tools_")
        or rel in ("src/live_mcp.rs", "src/bin/mcp_server.rs")
    )


# ---------------------------------------------------------------------
# Definition extraction
# ---------------------------------------------------------------------

_FN_RE = re.compile(r"\bfn\s+([A-Za-z_]\w*)\s*(?:<|\()")
_TRAIT_RE = re.compile(r"\btrait\s+[A-Za-z_]\w*[^;{]*\{")
_IMPL_RE = re.compile(r"\bimpl\b[^;{]*\{")
_USE_RE = re.compile(r"\buse\b[^;{}]*;")
_TOKEN_RE = re.compile(r"[A-Za-z_]\w*")
_SERDE_REF_RE = re.compile(
    r"(?:default|serialize_with|deserialize_with|with)\s*=\s*\""
    r"([A-Za-z_][\w:]*)\s*\""
)


def _block_spans(rx: re.Pattern, masked: str) -> list[tuple[int, int, str]]:
    out = []
    for m in rx.finditer(masked):
        try:
            a, b = brace_span(masked, m.end() - 1)
        except ValueError:
            continue
        out.append((a, b, masked[m.start() : m.end() - 1]))
    return out


def _prefix_of(masked: str, fn_start: int) -> str:
    """The declaration prefix immediately before `fn`:
    text after the last `;`, `{` or `}` in the 240 chars
    before it (visibility, async/const/unsafe, extern and
    attribute lines all live there)."""
    window = masked[max(0, fn_start - 240) : fn_start]
    cut = max(window.rfind(";"), window.rfind("{"), window.rfind("}"))
    return window[cut + 1 :]


def _attrs_of(masked: str, fn_start: int) -> list[str]:
    """Attribute lines (`#[...]`) in the contiguous block
    directly above the definition (doc comments are masked
    to spaces, so they read as blank lines and do not break
    the block scan; a real code line does)."""
    lines = masked[:fn_start].split("\n")
    attrs = []
    for line in reversed(lines[:-1][-10:]):
        s = line.strip()
        if s.startswith("#["):
            attrs.append(s)
        elif s == "":
            continue
        else:
            break
    return attrs


class Def:
    __slots__ = ("name", "file", "line", "offset", "pub", "kind")

    def __init__(self, name, file, line, offset, pub, kind):
        self.name = name
        self.file = file
        self.line = line
        self.offset = offset
        self.pub = pub
        # kind: "plain" (free / inherent-impl — censusable),
        # "trait-decl", "trait-impl", "main", "ffi"
        self.kind = kind


def extract_defs(src: Source) -> list[Def]:
    masked = src.masked
    trait_spans = _block_spans(_TRAIT_RE, masked)
    impl_spans = _block_spans(_IMPL_RE, masked)
    defs = []
    for m in _FN_RE.finditer(masked):
        name = m.group(1)
        name_off = m.start(1)
        if src.is_test_offset(m.start()):
            continue  # test-region definition: not production
        attrs = _attrs_of(masked, m.start())
        if any("cfg(test)" in a for a in attrs):
            continue  # test-only helper in a production file
        prefix = _prefix_of(masked, m.start())
        pub = bool(re.search(r"\bpub\b", prefix))
        kind = "plain"
        if any(a <= name_off < b for a, b, _h in trait_spans):
            kind = "trait-decl"
        elif any(
            a <= name_off < b and re.search(r"\bfor\b", h)
            for a, b, h in impl_spans
        ):
            kind = "trait-impl"
        elif name == "main":
            kind = "main"
        elif "extern" in prefix.split() or any(
            "no_mangle" in a for a in attrs
        ):
            kind = "ffi"
        defs.append(
            Def(name, src.rel, src.line_of(m.start()), name_off, pub, kind)
        )
    return defs


def extract_sites(
    sources: dict[str, Source], def_names: set[str], def_offsets: set
) -> dict[str, list[tuple[str, int]]]:
    """Production occurrences of defined names: name ->
    [(file, line)]. Definition sites and `use` spans are
    excluded; test regions never count."""
    sites: dict[str, list[tuple[str, int]]] = {}
    for rel, src in sorted(sources.items()):
        masked = src.masked
        use_spans = [(m.start(), m.end()) for m in _USE_RE.finditer(masked)]
        for m in _TOKEN_RE.finditer(masked):
            tok = m.group(0)
            if tok not in def_names:
                continue
            off = m.start()
            if (rel, off) in def_offsets:
                continue  # a definition site, not a call
            if src.is_test_offset(off):
                continue  # test-region evidence does not count
            if in_spans(use_spans, off):
                continue  # an import, not a call
            sites.setdefault(tok, []).append((rel, src.line_of(off)))
        # Serde helper references live in attribute STRINGS,
        # which masking blanks — read them from the raw text
        # (offsets align: masking preserves length).
        for m in _SERDE_REF_RE.finditer(src.raw):
            name = m.group(1).split("::")[-1]
            if name not in def_names:
                continue
            if src.is_test_offset(m.start()):
                continue
            sites.setdefault(name, []).append(
                (rel, src.line_of(m.start()))
            )
    return sites


# ---------------------------------------------------------------------
# The KNOWN ledger — seeded from U3-99 Ledger 2 (Wave 3).
# Every entry: (fn name, defining file, class, ledger ref,
# owning build slice, one-line argument). Classes:
#   "zero"     — cross == 0 and same == 0 expected
#   "internal" — cross == 0 expected (same-file use is the
#                placebo pattern itself)
#   "door"     — >= 1 production caller, 0 GUI callers
# An entry whose condition stops holding is STALE and
# fails the census: edit this ledger in the same push
# that lands the fix (the PARITY_PENDING ratchet).
#
# Out-of-scope Ledger-2 entries (not fns — recorded so
# they are not silently dropped): U-01a SPRITE_BITMAPS is
# a cache, not a fn; U-10 is missing panel FIELDS (the
# squash core is MCP-wired); U-17's migration reader is
# not a discrete fn; U-19's legal_parents / reparent_node
# ARE wired (editor.rs same-file + rig_tree) — the missing
# piece is a picker UI, not a call site.
# ---------------------------------------------------------------------

KNOWN = [
    # --- KNOWN-ZERO: built + tested, never called --------
    ("stage_sprite_layers", "src/stage_view.rs", "zero", "U-01",
     "IA-U3-09-3", "sprite layer builder; its shell mount never landed"),
    ("gizmo_hud_line", "src/stage_view.rs", "zero", "U-02",
     "IA-U3-25-1", "gizmo drag HUD line; call site IA-W2-07-1 never landed"),
    ("set_snap_config", "src/stage_view.rs", "zero", "U-14",
     "IA-U3-27-*", "snap configuration setter; snap has no toggle or caller"),
    ("overlay_curves", "src/graph_panel.rs", "zero", "U-03",
     "IA-U3-35-1 family", "graph multi-channel overlay; zero consumers"),
    ("composite_title_card", "src/title_cards.rs", "zero", "U-04",
     "IA-U3-68-1", "title-card export funnel entry; never wired at either funnel tail"),
    ("is_due", "src/autosave.rs", "zero", "U-05",
     "IA-U3-69-1", "AutosavePolicy::is_due; maybe_autosave still uses hard-coded AUTOSAVE_EVERY"),
    ("write_recovery_keeping", "src/autosave.rs", "zero", "U-05",
     "IA-U3-69-1", "policy recovery writer; engine still uses the single-file writer"),
    ("read_recovery_fallback", "src/autosave.rs", "zero", "U-05",
     "IA-U3-69-1", "policy recovery reader; no production reader exists"),
    ("open_via_dialog", "src/editor.rs", "zero", "U-07",
     "IA-U3-64-1", "native-dialog open head; zero call sites (dialogs::* below it read wired through it)"),
    ("save_via_dialog", "src/editor.rs", "zero", "U-07",
     "IA-U3-64-1", "native-dialog save head; zero call sites"),
    ("beat_marker_next", "src/beat_markers.rs", "zero", "U-11",
     "IA-U3-36-1 family", "sequential beat navigation; tests only"),
    ("beat_marker_prev", "src/beat_markers.rs", "zero", "U-11",
     "IA-U3-36-1 family", "sequential beat navigation; tests only"),
    ("clip_positions", "src/editor.rs", "zero", "U-13",
     "IA-U3-58-2", "clip edit-session positions; the GUI session is blind without it"),
    ("slice_sheet_grid", "src/export.rs", "zero", "U-18",
     "IA-U3-47-*", "foreign spritesheet slicer; tests only, no GUI path"),
    ("chrome_surface", "src/rune_sec/app_theme.rs", "zero", "U-16",
     "IA-U3-15-2 / IA-U3-19-3 (DECISION-FLAGGED)", "elevation ladder rung; zero production consumers"),
    ("elevated", "src/rune_sec/app_theme.rs", "zero", "U-16",
     "IA-U3-15-1", "elevation ladder rung; zero production consumers"),
    ("bounded_value_row", "src/rune_sec/design.rs", "zero", "U-20",
     "IA-U3-49-*", "design kit row builder; zero production consumers"),
    # --- KNOWN-ZERO: delegating public entry points kept
    # by IA-U3-29-1 (Build Batch 1, B1-03): production now
    # enters via the _with_positions variants under the
    # stage pass-context; each original delegates with a
    # fresh-evaluation closure and remains its module's
    # tested public API (its own suite pins behaviour
    # through it). Ledgered by the integrator at B1
    # integration; a production caller makes them STALE.
    ("text_geometry", "src/rune_sec/stage_text.rs", "zero", "B1-03",
     "IA-U3-29-1", "delegating entry; production uses text_geometry_with_positions under StagePass"),
    ("fx_geometry", "src/rune_sec/stage_fx.rs", "zero", "B1-03",
     "IA-U3-29-1", "delegating entry; production uses fx_geometry_with_positions under StagePass"),
    ("gizmo_geometry", "src/rune_sec/stage_fx.rs", "zero", "B1-03",
     "IA-U3-29-1", "delegating entry; production uses gizmo_geometry_with_positions under StagePass"),
    ("draw_gizmo", "src/stage_gizmo.rs", "zero", "B1-03",
     "IA-U3-29-1", "delegating entry; production uses draw_gizmo_with under StagePass"),
    ("draw_navigator", "src/stage_view/navigator.rs", "zero", "B1-03",
     "IA-U3-29-1", "delegating entry; production uses draw_navigator_with_positions under StagePass"),
    # --- KNOWN-INTERNAL: read only inside its own file ---
    ("export_defaults", "src/rune_sec/settings_popover.rs", "internal", "U-06",
     "IA-U3-66-1", "Settings export-defaults record; rendered by its own section, consumed by no panel"),
    # --- KNOWN-DOOR: wired, but only from MCP / own file -
    ("chapter_add", "src/editor.rs", "door", "U-09",
     "IA-U3-54-1", "chapter authoring op; MCP handlers only, no GUI path"),
    ("chapter_rename", "src/editor.rs", "door", "U-09",
     "IA-U3-54-1", "chapter authoring op; MCP handlers only, no GUI path"),
    ("chapter_set_range", "src/editor.rs", "door", "U-09",
     "IA-U3-54-1", "chapter authoring op; MCP handlers only, no GUI path"),
    ("chapter_move", "src/editor.rs", "door", "U-09",
     "IA-U3-54-1", "chapter authoring op; MCP handlers only, no GUI path"),
    ("fx_set_seed", "src/fx_ops.rs", "door", "U-12",
     "IA-U3-51-1", "FX aim op; called only by fx_ops.rs's own MCP handlers"),
    ("fx_set_angle", "src/fx_ops.rs", "door", "U-12",
     "IA-U3-51-1", "FX aim op; a browser-added Beam fires at 0 degrees forever"),
    ("fx_set_length", "src/fx_ops.rs", "door", "U-12",
     "IA-U3-51-1", "FX aim op; called only by fx_ops.rs's own MCP handlers"),
    ("clip_set_angle", "src/editor.rs", "door", "U-13",
     "IA-U3-58-2", "clip pose op; MCP-only, the GUI session is hand-less"),
    ("clip_set_root", "src/editor.rs", "door", "U-13",
     "IA-U3-58-2", "clip pose op; MCP-only, the GUI session is hand-less"),
]

# Ledger-2 entries a name-token census cannot isolate.
# Printed on every run; verified by their owning slices'
# own tests, not by this instrument.
NOT_CENSUSABLE = [
    ("clear (KeySel::clear)", "src/rune_sec/key_selection.rs", "U-15",
     "IA-U3-34-1 inside IA-U3-86-1",
     "the method name `clear` collides with ubiquitous std methods "
     "(Vec/HashMap/String); no name-based census can isolate it"),
]

# Gate check 4's hand-maintained watch list (phase_j_checks.py
# PHASE_J_OPS + PHASE_K_OPS + PHASE_L_OPS + PHASE_M_OPS +
# SAME_FILE_OPS), embedded as the cross-check: every name
# must read wired here, or this instrument disagrees with
# a green gate and the disagreement is reported as a
# failure of the census, not of the tree.
LEGACY_WATCH = [
    "fx_split_clip", "mirror_figure_all_frames", "normalise_all_clips",
    "particle_duplicate_event", "export_svg_range",
    "sync_playback_audio_on_drift",
    "delete_channel", "delete_keys_in_range",
    "duplicate_channel_to_figure", "set_project_fps", "set_stage_size",
    "delete_project_file", "audio_duplicate_clip_at",
    "fx_clip_gizmo_geometry", "note_playback_speed",
    "set_head_ring_scale", "simplify_stroke_at", "set_key_interp",
    "swap_frame_with_neighbour", "audio_set_muted_at", "shift_keys",
    "fill_keys_in_range", "frame_camera_points", "stage_fig3_prims",
    "delete_camera_key_at", "swap_frames", "export_sheet_range",
    "clip_duration_secs",
]


# ---------------------------------------------------------------------
# Census assembly
# ---------------------------------------------------------------------

class CensusError(Exception):
    pass


def find_test_file_stems(sources: dict[str, Source]) -> set[str]:
    """Stems of files declared as `#[cfg(test)] mod name;`
    in some parent (snap_tests, shell_control_tests):
    those files are test modules file-wide."""
    stems = set()
    for src in sources.values():
        for m in _MOD_DECL_RE.finditer(src.masked):
            before = src.masked[max(0, m.start() - 240) : m.start()]
            if "#[cfg(test)]" in before:
                stems.add(m.group(1))
    return stems


def run_census(root: str, known=KNOWN, legacy=LEGACY_WATCH) -> dict:
    paths = sorted(
        os.path.relpath(p, root)
        for p in glob.glob(os.path.join(root, "src", "**", "*.rs"),
                           recursive=True)
    )
    if not paths:
        raise CensusError(f"no src/**/*.rs under {root} — wrong root?")
    sources = {rel: Source(root, rel) for rel in paths}
    test_stems = find_test_file_stems(sources)
    test_files = {
        rel
        for rel in paths
        if os.path.splitext(os.path.basename(rel))[0] in test_stems
    }
    prod_sources = {r: s for r, s in sources.items() if r not in test_files}

    all_defs: list[Def] = []
    for src in prod_sources.values():
        all_defs += extract_defs(src)
    if not all_defs:
        raise CensusError("zero production definitions extracted — "
                          "the extraction patterns no longer match the tree")
    def_names = {d.name for d in all_defs}
    def_offsets = {(d.file, d.offset) for d in all_defs}
    sites = extract_sites(prod_sources, def_names, def_offsets)

    # Aggregate per name.
    by_name: dict[str, dict] = {}
    for d in all_defs:
        e = by_name.setdefault(
            d.name,
            {"defs": [], "def_files": set(), "censusable_files": set()},
        )
        e["defs"].append(d)
        e["def_files"].add(d.file)
        if d.pub and d.kind == "plain":
            e["censusable_files"].add(d.file)
    for name, e in by_name.items():
        ss = sites.get(name, [])
        e["cross"] = [s for s in ss if s[0] not in e["def_files"]]
        e["same"] = [s for s in ss if s[0] in e["def_files"]]
        e["gui"] = [s for s in ss if is_gui_file(s[0])]
        e["mcp"] = [s for s in ss if is_mcp_file(s[0])]
        e["total"] = len(ss)

    def def_in_file(name: str, rel: str) -> Def | None:
        for d in by_name.get(name, {}).get("defs", []):
            if d.file == rel and d.pub and d.kind == "plain":
                return d
        return None

    failures: list[str] = []

    # --- Legacy watch (cross-check vs gate check 4) ------
    legacy_rows = []
    for name in legacy:
        e = by_name.get(name)
        if e is None or not e["defs"]:
            legacy_rows.append((name, "MISSING — not defined in production"))
            failures.append(
                f"legacy watch: {name} is not defined anywhere in "
                "production — this census disagrees with gate check 4"
            )
        elif e["total"] == 0:
            legacy_rows.append((name, "UNWIRED under this census"))
            failures.append(
                f"legacy watch: {name} reads UNWIRED here but WIRED in "
                "gate check 4 — the instruments disagree; distrust this "
                "census until the difference is explained"
            )
        else:
            legacy_rows.append(
                (name, f"wired ({e['total']} production call site(s))")
            )

    # --- KNOWN ledger verification -----------------------
    known_rows = []
    known_names = set()
    for name, rel, cls, ref, owner, note in known:
        known_names.add(name)
        d = def_in_file(name, rel)
        e = by_name.get(name)
        if d is None:
            if e and e["defs"]:
                where = ", ".join(
                    sorted({f"{x.file}:{x.line}" for x in e["defs"]})
                )
                why = f"moved or redeclared — defined at {where}, not {rel}"
            else:
                why = f"gone — no production definition of {name} anywhere"
            known_rows.append((name, rel, cls, ref, "STALE", why))
            failures.append(
                f"KNOWN ledger STALE: {name} ({ref}, {owner}) — {why}. "
                "Edit the ledger in caller_census.py in the same push "
                "as the change."
            )
            continue
        cross, same, gui, total = (
            len(e["cross"]), len(e["same"]), len(e["gui"]), e["total"],
        )
        if cls == "zero":
            holds = cross == 0 and same == 0
            state = f"cross-file {cross}, same-file {same}"
        elif cls == "internal":
            holds = cross == 0
            state = f"cross-file {cross}, same-file {same}"
        else:  # door
            holds = total >= 1 and gui == 0
            state = f"call sites {total}, GUI call sites {gui}"
        if holds:
            known_rows.append((name, rel, cls, ref, "KNOWN", state))
        else:
            known_rows.append((name, rel, cls, ref, "STALE", state))
            failures.append(
                f"KNOWN ledger STALE: {name} ({ref}, {owner}) no longer "
                f"matches its class '{cls}' ({state}) — the landing it "
                "waits for may have happened. Verify, then edit the "
                "ledger in caller_census.py."
            )

    # --- The systematic verdict ----------------------------
    new_unwired = []
    same_file_only = []
    exempt_counts = {"trait-decl": 0, "trait-impl": 0, "main": 0, "ffi": 0}
    n_censusable = 0
    n_private_defs = 0
    for name, e in sorted(by_name.items()):
        kinds = {d.kind for d in e["defs"]}
        for k in exempt_counts:
            if kinds == {k}:
                exempt_counts[k] += 1
        if not e["censusable_files"]:
            if any(not d.pub for d in e["defs"]):
                n_private_defs += 1
            continue
        n_censusable += 1
        if name in known_names:
            continue
        if e["total"] == 0:
            new_unwired.append(e)
            failures.append(
                f"NEW-UNWIRED: {name} — pub fn defined at "
                + ", ".join(
                    sorted({f"{d.file}:{d.line}" for d in e["defs"]})
                )
                + " has ZERO production call sites and is not on the "
                "KNOWN ledger. Wire it, or ledger it in "
                "caller_census.py with a one-line argument."
            )
        elif not e["cross"]:
            same_file_only.append(e)

    verdict = "FAIL" if failures else "PASS"
    return {
        "root": root,
        "files_scanned": len(paths),
        "test_files_excluded": sorted(test_files),
        "production_defs": len(all_defs),
        "distinct_names": len(by_name),
        "censusable_names": n_censusable,
        "private_only_names": n_private_defs,
        "exempt_counts": exempt_counts,
        "legacy": legacy_rows,
        "known": known_rows,
        "new_unwired": [
            {
                "name": e["defs"][0].name,
                "defs": sorted({f"{d.file}:{d.line}" for d in e["defs"]}),
            }
            for e in new_unwired
        ],
        "same_file_only": [
            {
                "name": e["defs"][0].name,
                "defs": sorted({f"{d.file}:{d.line}" for d in e["defs"]}),
                "same_file_sites": len(e["same"]),
            }
            for e in same_file_only
        ],
        "failures": failures,
        "verdict": verdict,
    }


def render_text(data: dict) -> str:
    lines = []
    lines.append("Caller census — StykAnimate-RS (IA-U3-89-2)")
    lines.append(
        f"Scanned {data['files_scanned']} files under src/ "
        f"({len(data['test_files_excluded'])} cfg(test) file module(s) "
        f"excluded: {', '.join(data['test_files_excluded']) or 'none'}); "
        f"{data['production_defs']} production fn definitions, "
        f"{data['distinct_names']} distinct names."
    )
    lines.append(
        f"Censusable pub fns: {data['censusable_names']}; "
        f"private-only names (report-only): {data['private_only_names']}; "
        "exempt by class: "
        + ", ".join(
            f"{k} {v}" for k, v in data["exempt_counts"].items()
        )
    )
    lines.append("")
    lines.append(f"[legacy watch — gate check 4 cross-check] "
                 f"({len(data['legacy'])} ops)")
    bad = [r for r in data["legacy"] if "wired (" not in r[1]]
    for name, state in data["legacy"]:
        lines.append(f"  {name}: {state}")
    lines.append(
        "  legacy summary: "
        + (f"{len(data['legacy'])}/{len(data['legacy'])} wired"
           if not bad else f"{len(bad)} DISAGREE — see failures")
    )
    lines.append("")
    lines.append(f"[known ledger] ({len(data['known'])} entries, "
                 "U3-99 Ledger 2)")
    for name, rel, cls, ref, status, state in data["known"]:
        lines.append(f"  {status:6} {name} ({rel}, {cls}, {ref}): {state}")
    for name, rel, ref, _owner, why in NOT_CENSUSABLE:
        lines.append(f"  NOT-CENSUSABLE {name} ({rel}, {ref}): {why}")
    lines.append("")
    lines.append(f"[new unwired] ({len(data['new_unwired'])}) — "
                 "pub fns with ZERO production call sites, not ledgered")
    for row in data["new_unwired"]:
        lines.append(f"  UNWIRED {row['name']}  def {', '.join(row['defs'])}")
    lines.append("")
    lines.append(f"[wired same-file only] ({len(data['same_file_only'])}) "
                 "— report-only: no cross-file production caller")
    for row in data["same_file_only"]:
        lines.append(
            f"  {row['name']}  def {', '.join(row['defs'])}  "
            f"({row['same_file_sites']} same-file call site(s))"
        )
    lines.append("")
    if data["failures"]:
        lines.append("[failures]")
        for f in data["failures"]:
            lines.append(f"  FAIL {f}")
        lines.append("")
    lines.append(f"RESULT: {data['verdict']}")
    return "\n".join(lines) + "\n"


# ---------------------------------------------------------------------
# Self-test: synthetic trees exercising every verdict path.
# ---------------------------------------------------------------------

def _self_test() -> int:
    import tempfile

    failures = []

    def expect(ok, label):
        print(("PASS  " if ok else "FAIL  ") + label)
        if not ok:
            failures.append(label)

    OPS = """
pub fn wired_op() {}
pub fn lonely_op() {}
pub fn known_dead() {}
pub fn internal_step() {}
pub fn helper() { internal_step(); }
pub fn test_only() {}
pub fn default_flag() -> bool { true }
pub trait Greeter { fn greet(&self); }
pub struct Thing;
impl Greeter for Thing { fn greet(&self) {} }
#[derive(Default)]
pub struct Cfg { #[serde(default = "default_flag")] pub flag: bool }
#[cfg(test)]
mod tests {
    #[test]
    fn t() { test_only(); known_dead(); }
}
"""
    PANEL = ("pub fn panel() { crate::ops::wired_op(); "
             "crate::ops::helper(); }\n")
    MAIN = "fn main() { panel(); }\n"
    ledger = [
        ("known_dead", "src/ops.rs", "zero", "TEST", "TEST", "fixture"),
    ]
    legacy = ["wired_op", "internal_step"]

    def make_tree(extra_ops: str = "", extra_panel: str = ""):
        td = tempfile.mkdtemp()
        os.makedirs(os.path.join(td, "src"))
        with open(os.path.join(td, "src", "ops.rs"), "w") as fh:
            fh.write(OPS + extra_ops)
        with open(os.path.join(td, "src", "panel.rs"), "w") as fh:
            fh.write(PANEL + extra_panel)
        with open(os.path.join(td, "src", "main.rs"), "w") as fh:
            fh.write(MAIN)
        return td

    # Tree A: the honest mixed tree — FAIL on exactly the
    # two unledgered zero-caller fns.
    data = run_census(make_tree(), known=ledger, legacy=legacy)
    names = {r["name"] for r in data["new_unwired"]}
    expect(data["verdict"] == "FAIL", "tree A verdict is FAIL")
    expect(
        names == {"lonely_op", "test_only"},
        f"tree A new-unwired is exactly lonely_op + test_only "
        f"(test-region callers do not count) — got {sorted(names)}",
    )
    expect(
        data["known"][0][4] == "KNOWN",
        "tree A known_dead reads KNOWN (its test callers are not evidence)",
    )
    expect(
        all("wired (" in s for _n, s in data["legacy"]),
        "tree A legacy watch agrees (wired_op cross-file, internal_step same-file)",
    )
    sfo = {r["name"] for r in data["same_file_only"]}
    expect(
        "internal_step" in sfo and "default_flag" in sfo,
        "tree A same-file-only holds internal_step and the "
        "serde-referenced default_flag",
    )
    expect(
        "greet" not in names and "main" not in names,
        "tree A trait-impl greet and main are exempt, never unwired",
    )

    # Tree B: the known fn gains a production caller and a
    # ghost joins the ledger — both must read STALE.
    ledger_b = ledger + [
        ("ghost_fn", "src/ops.rs", "zero", "TEST", "TEST", "fixture"),
    ]
    data_b = run_census(
        make_tree(extra_panel="pub fn p2() { crate::ops::known_dead(); }\n"),
        known=ledger_b,
        legacy=legacy,
    )
    stale = {r[0] for r in data_b["known"] if r[4] == "STALE"}
    expect(
        stale == {"known_dead", "ghost_fn"},
        f"tree B landed-caller + ghost entries read STALE — got {sorted(stale)}",
    )

    # Tree C: no unwired, ledger exact — PASS.
    ops_c = OPS.replace("pub fn lonely_op() {}\n", "").replace(
        "pub fn test_only() {}\n", ""
    ).replace(" test_only();", "")
    td = tempfile.mkdtemp()
    os.makedirs(os.path.join(td, "src"))
    with open(os.path.join(td, "src", "ops.rs"), "w") as fh:
        fh.write(ops_c)
    with open(os.path.join(td, "src", "panel.rs"), "w") as fh:
        fh.write(PANEL)
    with open(os.path.join(td, "src", "main.rs"), "w") as fh:
        fh.write(MAIN)
    data_c = run_census(td, known=ledger, legacy=legacy)
    expect(
        data_c["verdict"] == "PASS" and not data_c["new_unwired"],
        "tree C clean tree reads PASS",
    )

    print("SELF-TEST: " + ("PASS" if not failures
                           else f"FAIL ({len(failures)})"))
    return 0 if not failures else 1


# ---------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------

def repo_root() -> str:
    guess = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    if os.path.isfile(os.path.join(guess, "src", "main.rs")):
        return guess
    return os.getcwd()


def main(argv: list[str]) -> int:
    if "--self-test" in argv:
        return _self_test()
    root = repo_root()
    if "--root" in argv:
        root = argv[argv.index("--root") + 1]
    try:
        data = run_census(root)
    except (CensusError, ValueError, OSError) as exc:
        print(f"caller_census: REFUSING to print a census: {exc}",
              file=sys.stderr)
        print("RESULT: INCOMPLETE")
        return 2
    if "--json" in argv:
        print(json.dumps(data, indent=2))
    else:
        sys.stdout.write(render_text(data))
    return 0 if data["verdict"] == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
