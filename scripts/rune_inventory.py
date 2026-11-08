#!/usr/bin/env python3
"""Rune UI element-key inventory for StykAnimate-RS.

Extracts every element key defined by the Rune shell sources:

  - src/main.rs
  - src/stage_view.rs
  - src/chrome_view.rs
  - src/rune_sec/*.rs   (Phase D section files; the directory may
                         be empty or absent — both are fine)

Key definitions recognised:

  - `.key("name")`                static key on an element builder
  - `.key(format!("frame-{}"))`   dynamic key pattern (recorded with
                                  the format string's literal parts,
                                  e.g. `frame-{}`), one pattern entry
  - `btn("name", ...)`            the chrome `btn()` helper (and the
                                  section-local `fbtn`/`row_toggle`
                                  variants, plus Phase H's
                                  `icon_btn`/`icon_only_btn` and their
                                  inert twins, whose first argument is
                                  likewise the key). Phase N repair
                                  (2026-11-08): the helper list also
                                  covers the local dialects the old
                                  census was blind to (`ctl`/`ictl`,
                                  `kbtn`, `pbtn`/`pibtn`, `danger_btn`)
                                  and the Phase N family/kit helpers
                                  (`btn_inert`, `btn_muted_live`,
                                  `danger_icon_btn`, `chip_btn`, and
                                  design.rs's `panel_col`, `list_row`,
                                  `toggle`, `toggle_row`, `header_row`,
                                  `divider_keyed`, `swatch`) — keys
                                  passed to those helpers as string
                                  literals count exactly like `.key()`
  - `btn(&format!("..."), ...)`   dynamic key pattern via a helper
  - `btn(match ... { A => "k" })` keys chosen by a match on the
                                  first argument (easing_panel idiom);
                                  the same match form on `.key(...)`
                                  itself (easing_panel's picker cells)
  - `const NAME_KEY: &str = "k"`  keys carried by a named constant
                                  (Phase J §9 convention: the call
                                  sites name the constant, so the
                                  constant's value IS the key — the
                                  gate's diff extractor has counted
                                  these since Phase J; Wave 10 E3
                                  taught the inventory the same)
  - `const NAME_KEYS: [&str; N]`  literals inside a key table named
                                  with the `_KEYS` convention (menus'
                                  MENU_KEYS, export's RANGE_KEYS) —
                                  name-scoped on purpose, so label
                                  tables (MENUS and friends) are NOT
                                  swept in
  - `stepper_row(label, value, "dn", "up", ...)` — the two
                                  stepper_row helpers (inspector_
                                  segment, scene_panel) forward their
                                  3rd/4th arguments verbatim to the
                                  − / + buttons; Wave 10 E3 extracts
                                  those two positions with a
                                  balanced-argument scan
  - `key: "name"` / `key: format!("...")`  entry-data keys — in
                                  practice this catches more than
                                  figure_add's `Entry` table: the
                                  command palette's COMMANDS ids,
                                  fx_browser's clip-kind entries and
                                  stats_view's row keys are all
                                  `key:`-field tables whose ids the
                                  census has always counted

Known blind spots (enumerated by hand in docs/QA_RUNE.md instead):
keys held in plain local variables before being passed to a helper
(three_d_panel's `ictl(key, ...)` table, fx_clips' `swatch(key, ...)`
table and its variable-stem `format!` key, home_panel's let-bound
`home-open-{i}` / `home-dup-{i}` / `home-proj-rename-{i}` /
`home-proj-del-{i}` patterns, graph_panel's let-bound
`graph-ch-{}` pattern, menus' recents `item(&key, ...)`), and keys
stored in data tables that do not follow the `_KEY`/`_KEYS` naming
convention and are passed by variable. A general
`let x = format!(...)` extraction was considered and rejected:
most let-bound format strings are labels and status text, not
keys, so the pattern cannot be told apart from prose. Note:
the Phase M headline count
(733) was taken with the pre-repair census, which could not see
literal keys passed to `ctl`/`ictl`/`kbtn`/`pbtn`/`pibtn`/
`danger_btn` — those keys existed and were live in the headless
trees; the repaired census counts them, so post-repair totals
run higher than 733 without any key having been added.

Deliberately NOT extracted: key *lookups* such as
`cx.layout_size("stage")` — those reference a key defined elsewhere
and would double-count.

Output: the files scanned, then every unique key sorted (dynamic
patterns marked), then the unique count. Stdlib only; run with
`python3 scripts/rune_inventory.py` from anywhere.
"""

import glob
import os
import re
import sys

STATIC_KEY_RE = re.compile(r'\.key\(\s*"([^"]+)"\s*\)')
DYNAMIC_KEY_RE = re.compile(r'\.key\(\s*format!\(\s*"([^"]+)"')
_KEY_HELPERS = (
    r"btn|fbtn|row_toggle|icon_btn|icon_only_btn"
    r"|icon_btn_inert|icon_only_btn_inert"
    r"|btn_inert|btn_muted_live|danger_btn|danger_icon_btn|chip_btn"
    r"|ctl|ictl|kbtn|pbtn|pibtn"
    r"|panel_col|list_row|toggle|toggle_row|header_row|divider_keyed|swatch"
    # Wave 9 local dispatchers + kit splitter: step_btn
    # (onion_controls), edit_btn (text_tool) and v_splitter
    # (design) each take the key as arg 1 and forward it
    # verbatim to a listed builder — verified at source at
    # Wave 9 integration; without them 13 mounted keys were
    # invisible to the census.
    r"|step_btn|edit_btn|v_splitter"
    # Wave 10 (E3) calibration — each verified at source to
    # take the key as arg 1 and apply it verbatim:
    # chip_btn_inert (icon_btn: `.key(key)`; the inert twin
    # of the listed chip_btn, previously missing so its
    # keys were invisible), tangent_preset_chip (design:
    # `.key(key)`; carries graph/keyframe/easing preset
    # keys in literal, match and format! forms),
    # weather_slider (scene_panel: `el().key(key)`),
    # separator (menus: delegates to the listed
    # divider_keyed(key)), item (menus: dispatches to
    # plain_item/hinted_item/item_icon, all `.key(key)`),
    # group (library_panel: `col().key(key.to_string())`),
    # and export_panel's plain_row/range_row/bundle_row
    # (forward arg 1 to icon_btn/btn — their production
    # call sites pass the xc-* literals).
    r"|chip_btn_inert|tangent_preset_chip|weather_slider"
    r"|separator|item|group"
    r"|plain_row|range_row|bundle_row"
    # Wave 10 integration: two more local dispatchers,
    # verified at source to take the key as arg 1 and
    # forward it verbatim — stamp_step_btn (draw_tools:
    # forwards to step_btn/icon_btn idiom for the stamp
    # steppers) and zorder_btn (text_tool: the text
    # z-order pair). Without them the census misses
    # draw-sides-*/draw-size-* and text-front/text-back.
    r"|stamp_step_btn|zorder_btn"
)
BTN_KEY_RE = re.compile(r'\b(?:' + _KEY_HELPERS + r')\(\s*"([^"]+)"')
BTN_DYN_RE = re.compile(r'\b(?:' + _KEY_HELPERS + r')\(\s*&?\s*format!\(\s*"([^"]+)"')
ENTRY_KEY_RE = re.compile(r'\bkey:\s*"([^"]+)"')
ENTRY_DYN_RE = re.compile(r'\bkey:\s*format!\(\s*"([^"]+)"')
STRING_RE = re.compile(r'"([^"]+)"')
# Keys carried by a named constant (Phase J §9: `key-delete`
# and `keyframe-actions` moved into `pub const …_KEY: &str`
# values, so call sites name the constant, not the literal).
# The constant's value IS the key. This mirrors the gate's
# own _DIFF_CONST_KEY_RE in phase_j_checks.py, which has
# counted these on both sides of the key diff since Phase J
# — until Wave 10 the inventory was the blind extractor.
CONST_KEY_RE = re.compile(r'const\s+\w+_KEY\s*:\s*&str\s*=\s*"([^"]+)"')
# Key tables under the plural naming convention
# (`pub const MENU_KEYS: [&str; 4]`, `pub const RANGE_KEYS:
# [&str; 9]`): their literals are mounted keys, indexed at
# the call site. Name-scoped to `_KEYS` on purpose — label
# tables such as MENUS hold prose, not keys, and must not
# be swept in.
CONST_KEYS_RE = re.compile(r'const\s+\w+_KEYS\s*:\s*\[&str;\s*\d+\]\s*=\s*\[([^\]]*)\]')


def match_arm_keys(src: str) -> list[str]:
    """Keys chosen by `btn(match ... { ... => "key", ... }, ...)`:
    find helper calls whose first argument is a `match`, then take
    the string literals inside the brace-balanced match block.
    The same match form on `.key(...)` itself counts too
    (easing_panel's picker cells: `.key(match e { ... })`) —
    Wave 10 (E3): those four ease-pick-* keys were invisible
    before, because only helper-first matches were scanned."""
    keys: list[str] = []
    for m in re.finditer(
        r'\b(?:' + _KEY_HELPERS + r')\(\s*match\b|\.key\(\s*match\b', src
    ):
        brace = src.find("{", m.end())
        if brace < 0:
            continue
        depth = 0
        end = brace
        while end < len(src):
            ch = src[end]
            if ch == "{":
                depth += 1
            elif ch == "}":
                depth -= 1
                if depth == 0:
                    break
            end += 1
        keys.extend(STRING_RE.findall(src[brace:end]))
    return keys


def stepper_row_keys(src: str) -> list[str]:
    """The dn/up keys of `stepper_row(label, value, dn, up, ...)`
    calls. Both stepper_row helpers (inspector_segment's and
    scene_panel's — identical signatures, verified at source)
    forward arguments 3 and 4 verbatim to the row's − / +
    buttons (`btn(dn_key, "−", ...)` / `btn(up_key, "+", ...)`),
    so literal keys at those positions are mounted keys. The
    value argument is an arbitrary expression (it can be a
    `match` or a `format!` containing commas), so the argument
    list is split with a depth-balanced scan — the same
    discipline match_arm_keys uses — and only arguments that
    are exactly a string literal are taken. Non-literal dn/up
    arguments stay a blind spot, as do the fn definitions
    themselves (skipped via the `fn ` prefix)."""
    keys: list[str] = []
    for m in re.finditer(r'\bstepper_row\(', src):
        if src[max(0, m.start() - 3):m.start()] == "fn ":
            continue  # the definition, not a call
        args: list[str] = []
        depth = 0
        cur = ""
        i = m.end() - 1  # at the '('
        while i < len(src):
            ch = src[i]
            if ch in "([{":
                depth += 1
            elif ch in ")]}":
                depth -= 1
                if depth == 0:
                    break
            if ch == "," and depth == 1:
                args.append(cur)
                cur = ""
            elif depth >= 1 and not (depth == 1 and ch == "("):
                cur += ch
            i += 1
        args.append(cur)
        if len(args) >= 4:
            for arg in args[2:4]:
                lit = STRING_RE.fullmatch(arg.strip())
                if lit:
                    keys.append(lit.group(1))
    return keys


def repo_root() -> str:
    """The repo root is the parent of this script's directory;
    fall back to the current directory if that guess is wrong."""
    guess = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    if os.path.isfile(os.path.join(guess, "src", "main.rs")):
        return guess
    return os.getcwd()


def main() -> int:
    root = repo_root()
    files = [
        os.path.join(root, "src", "main.rs"),
        os.path.join(root, "src", "stage_view.rs"),
        os.path.join(root, "src", "chrome_view.rs"),
    ]
    files += sorted(glob.glob(os.path.join(root, "src", "rune_sec", "*.rs")))
    files = [f for f in files if os.path.isfile(f)]

    static_keys: set[str] = set()
    dynamic_keys: set[str] = set()
    origins: dict[str, list[str]] = {}

    def note(key: str, bucket: set[str], path: str) -> None:
        bucket.add(key)
        origins.setdefault(key, []).append(os.path.relpath(path, root))

    for path in files:
        with open(path, encoding="utf-8") as fh:
            src = fh.read()
        for key in STATIC_KEY_RE.findall(src):
            note(key, static_keys, path)
        for key in BTN_KEY_RE.findall(src):
            note(key, static_keys, path)
        for key in ENTRY_KEY_RE.findall(src):
            note(key, static_keys, path)
        for key in match_arm_keys(src):
            note(key, static_keys, path)
        for key in CONST_KEY_RE.findall(src):
            note(key, static_keys, path)
        for table in CONST_KEYS_RE.findall(src):
            for key in STRING_RE.findall(table):
                note(key, static_keys, path)
        for key in stepper_row_keys(src):
            note(key, static_keys, path)
        for pattern in DYNAMIC_KEY_RE.findall(src):
            note(pattern, dynamic_keys, path)
        for pattern in BTN_DYN_RE.findall(src):
            note(pattern, dynamic_keys, path)
        for pattern in ENTRY_DYN_RE.findall(src):
            note(pattern, dynamic_keys, path)

    print("Rune element-key inventory")
    print(f"Repo: {root}")
    print("Files scanned:")
    for path in files:
        print(f"  {os.path.relpath(path, root)}")
    sec_dir = os.path.join(root, "src", "rune_sec")
    if not glob.glob(os.path.join(sec_dir, "*.rs")):
        print("  (src/rune_sec/ has no .rs files yet)")
    print()
    print("Keys (unique, sorted):")
    for key in sorted(static_keys | dynamic_keys):
        marker = "  [dynamic pattern]" if key in dynamic_keys else ""
        where = ", ".join(sorted(set(origins[key])))
        print(f"  {key}  ({where}){marker}")
    print()
    print(f"Static keys:  {len(static_keys)}")
    print(f"Dynamic patterns: {len(dynamic_keys)}")
    print(f"Total unique keys: {len(static_keys | dynamic_keys)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
