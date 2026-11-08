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
                                  first argument (easing_panel idiom)
  - `key: "name"` / `key: format!("...")`  entry-data keys
                                  (figure_add's `Entry` table)

Known blind spots (enumerated by hand in docs/QA_RUNE.md instead):
positional dn/up key arguments to `stepper_row(...)` helpers, keys
held in plain local variables before being passed to a helper
(three_d_panel's `ictl(key, ...)` table, fx_clips' `swatch(key, ...)`),
and keys stored in data tables and passed by variable
(export_panel's `xc-*` rows). Note: the Phase M headline count
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
)
BTN_KEY_RE = re.compile(r'\b(?:' + _KEY_HELPERS + r')\(\s*"([^"]+)"')
BTN_DYN_RE = re.compile(r'\b(?:' + _KEY_HELPERS + r')\(\s*&?\s*format!\(\s*"([^"]+)"')
ENTRY_KEY_RE = re.compile(r'\bkey:\s*"([^"]+)"')
ENTRY_DYN_RE = re.compile(r'\bkey:\s*format!\(\s*"([^"]+)"')
STRING_RE = re.compile(r'"([^"]+)"')


def match_arm_keys(src: str) -> list[str]:
    """Keys chosen by `btn(match ... { ... => "key", ... }, ...)`:
    find `btn(` calls whose first argument is a `match`, then take
    the string literals inside the brace-balanced match block."""
    keys: list[str] = []
    for m in re.finditer(r'\b(?:' + _KEY_HELPERS + r')\(\s*match\b', src):
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
