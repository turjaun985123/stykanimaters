#!/usr/bin/env python3
"""Keyboard-focus census for headless --tree dumps (Roadmap A20-10,
focus half; Wave 2 seat W2-66).

Reads headless tree dumps and reports, per scene, the keyboard-focus
evidence the dump actually carries:

  * the FOCUSABLE set -- every element whose dump record carries
    `focusable: true`, in tree order. Under the standing D-2
    configuration (AppSpec::tab_visits_controls stays OFF and no
    builder adopts El::focusable(true)), Rune's Tab order is this
    set filtered to text inputs / on_key owners / list groups
    (runtime/input/focus.rs `focus_key`'s retain filter), sorted
    by (focus order, node index) -- every app element carries the
    default focus order, so tree order IS the derived Tab order.
    The census prints it labelled DERIVED: a dump proves the set;
    only a scripted --script run proves the walk.
  * the FOCUSED element (at most one) -- a state-pre-focused text
    field DOES surface here (measured: palette-open's query, Wave
    AC closing artifact), because the field builders wire the
    TextEdit focus flag into the element.
  * the focusVisible count -- measured 0 in every dump so far,
    including the one with a focused element: focus RINGS are not
    assertable from dumps (P14's doctrine, confirmed by data).

Usage:
    python3 scripts/focus_census.py DUMP.json|DIR [--json]
    python3 scripts/focus_census.py DIR --check [--expect FILE]
    python3 scripts/focus_census.py --self-test

DIR is a directory of tree-*.json dumps (the headless artifact
layout); a single file censuses one scene. Scene names derive
from file names: tree-<scene>.json -> <scene>.

--check asserts the D-2-off invariants over every dump:
  1. at most one focused element per dump;
  2. a focused element is also focusable;
  3. every focusable element has role "textbox" -- THE D-2
     tripwire: the day a button/slider/list becomes Tab-focusable
     (tab_visits_controls enabled, or a builder adopting
     .focusable(true) family-wide), this check fails loudly
     instead of the change slipping in unmeasured.
--expect FILE adds per-scene promises (JSON):
    { "scene": {"focused_key": "proj-name" | null,
                "focusable_keys": ["proj-name", ...]}, ... }
  "focused_key": null promises nothing is focused. A promised
  scene missing from the input is a failure -- an expectation is
  a claim about the artifact, and silence is not a pass.

Exit codes: 0 = report printed / all checks pass,
1 = check failures, 2 = usage / IO / malformed-dump error.

Stdlib only, like the rest of the gate toolkit.
"""

import json
import os
import sys
import tempfile

TEXTBOX_ROLE = "textbox"


def die(msg):
    print(f"focus_census: {msg}", file=sys.stderr)
    return 2


def load_dump(path):
    with open(path, "r", encoding="utf-8") as fh:
        data = json.load(fh)
    if not isinstance(data, dict) or not isinstance(data.get("elements"), list):
        raise ValueError(f"{path}: not a tree dump (no 'elements' list)")
    return data


def scene_name(path):
    base = os.path.basename(path)
    if base.startswith("tree-") and base.endswith(".json"):
        return base[len("tree-"):-len(".json")]
    return os.path.splitext(base)[0]


def census_dump(data):
    """The focus facts of one dump, in tree order."""
    focusable = []
    focused = []
    focus_visible = 0
    for el in data["elements"]:
        if not isinstance(el, dict):
            continue
        if el.get("focusVisible"):
            focus_visible += 1
        if el.get("focusable"):
            focusable.append(el)
        if el.get("focused"):
            focused.append(el)
    return {
        "elements": len(data["elements"]),
        "focusable": focusable,
        "focused": focused,
        "focus_visible": focus_visible,
    }


def el_label(el):
    key = el.get("key") or "-"
    return f"{key} [{el.get('role', '?')}] {el.get('path', '?')}"


def check_scene(name, facts, problems):
    foc = facts["focusable"]
    fone = facts["focused"]
    if len(fone) > 1:
        problems.append(
            f"{name}: {len(fone)} elements report focused (at most one may hold focus)"
        )
    for el in fone:
        if not el.get("focusable"):
            problems.append(f"{name}: focused element is not focusable: {el_label(el)}")
    for el in foc:
        if el.get("role") != TEXTBOX_ROLE:
            problems.append(
                f"{name}: non-textbox element is Tab-focusable "
                f"(D-2 tripwire): {el_label(el)}"
            )


def check_expectations(name, facts, expect, problems):
    fone = facts["focused"]
    if "focused_key" in expect:
        want = expect["focused_key"]
        got = fone[0].get("key") if len(fone) == 1 else None
        if want is None:
            if fone:
                problems.append(
                    f"{name}: expected nothing focused, found {el_label(fone[0])}"
                )
        elif len(fone) != 1 or got != want:
            found = el_label(fone[0]) if fone else "nothing"
            problems.append(
                f"{name}: expected focused key {want!r}, found {found}"
            )
    if "focusable_keys" in expect:
        got_keys = [el.get("key") for el in facts["focusable"]]
        if got_keys != expect["focusable_keys"]:
            problems.append(
                f"{name}: focusable keys {got_keys} != expected {expect['focusable_keys']}"
            )


def collect_inputs(target):
    """[(scene, path)] for a dump file or a directory of dumps."""
    if os.path.isfile(target):
        return [(scene_name(target), target)]
    if os.path.isdir(target):
        out = []
        for base in sorted(os.listdir(target)):
            if base.startswith("tree-") and base.endswith(".json"):
                out.append((scene_name(base), os.path.join(target, base)))
        return out
    return None


def run(target, do_check, expect_path, as_json):
    inputs = collect_inputs(target)
    if inputs is None:
        return die(f"no such file or directory: {target}")
    if not inputs:
        return die(f"no tree-*.json dumps under {target}")
    expectations = {}
    if expect_path:
        try:
            with open(expect_path, "r", encoding="utf-8") as fh:
                expectations = json.load(fh)
        except (OSError, json.JSONDecodeError) as exc:
            return die(f"cannot read expectations {expect_path}: {exc}")
        if not isinstance(expectations, dict):
            return die(f"expectations {expect_path}: top level must be an object")
    scenes = {}
    problems = []
    seen = set()
    for name, path in inputs:
        try:
            data = load_dump(path)
        except (OSError, json.JSONDecodeError, ValueError) as exc:
            return die(str(exc))
        facts = census_dump(data)
        scenes[name] = facts
        seen.add(name)
        if do_check:
            check_scene(name, facts, problems)
            if name in expectations:
                check_expectations(name, facts, expectations[name], problems)
    if do_check:
        for name in expectations:
            if name not in seen:
                problems.append(
                    f"{name}: expected scene missing from the artifact "
                    f"(expectations are claims about the artifact)"
                )
    if as_json:
        report = {
            name: {
                "elements": f["elements"],
                "focusable": [
                    {"key": e.get("key"), "role": e.get("role"),
                     "path": e.get("path"), "rect": e.get("rect")}
                    for e in f["focusable"]
                ],
                "focused": [
                    {"key": e.get("key"), "role": e.get("role"),
                     "path": e.get("path"), "rect": e.get("rect")}
                    for e in f["focused"]
                ],
                "focus_visible": f["focus_visible"],
            }
            for name, f in sorted(scenes.items())
        }
        print(json.dumps({"scenes": report, "problems": problems}, indent=1))
    else:
        for name in sorted(scenes):
            f = scenes[name]
            print(f"== {name}: {f['elements']} elements, "
                  f"{len(f['focusable'])} focusable, "
                  f"{len(f['focused'])} focused, "
                  f"focusVisible {f['focus_visible']}")
            for i, el in enumerate(f["focusable"]):
                mark = " *FOCUSED*" if el.get("focused") else ""
                print(f"   tab[{i}] (derived) {el_label(el)} "
                      f"rect={el.get('rect')}{mark}")
            for el in f["focused"]:
                if not el.get("focusable"):
                    print(f"   focused (not focusable!): {el_label(el)}")
        if problems:
            print("FAILURES:")
            for p in problems:
                print(f"  - {p}")
        elif do_check:
            print("focus census: all checks PASS "
                  f"({len(scenes)} scenes, D-2-off invariants hold)")
    if do_check and problems:
        return 1
    return 0


# ----------------------------------------------------------------
# Self-test: synthetic dumps exercise every check both ways.
# ----------------------------------------------------------------

def _dump(elements):
    return {"elements": elements}


def _el(key, role, path, focusable=False, focused=False, fv=False):
    el = {"key": key, "role": role, "path": path, "rect": [0, 0, 10, 10],
          "focusable": focusable, "focused": focused, "focusVisible": fv}
    return el


def self_test():
    failures = []

    def expect(cond, label):
        print(("PASS " if cond else "FAIL ") + label)
        if not cond:
            failures.append(label)

    with tempfile.TemporaryDirectory() as td:
        def write(name, data):
            p = os.path.join(td, name)
            with open(p, "w", encoding="utf-8") as fh:
                json.dump(data, fh)
            return p

        clean = _dump([
            _el("proj-name", "textbox", "#0/a/proj-name", focusable=True),
            _el("play", "button", "#0/a/play"),
            _el("fig-name", "textbox", "#0/b/fig-name", focusable=True, focused=True),
        ])
        clean_dir = os.path.join(td, "clean")
        os.mkdir(clean_dir)
        with open(os.path.join(clean_dir, "tree-default.json"), "w") as fh:
            json.dump(clean, fh)
        exp = os.path.join(td, "exp.json")
        with open(exp, "w") as fh:
            json.dump({"default": {"focused_key": "fig-name",
                                   "focusable_keys": ["proj-name", "fig-name"]}}, fh)
        expect(run(clean_dir, True, exp, False) == 0,
               "clean set + matching expectations passes")

        btn = os.path.join(td, "btn")
        os.mkdir(btn)
        with open(os.path.join(btn, "tree-default.json"), "w") as fh:
            json.dump(_dump([_el("play", "button", "#0/play",
                                 focusable=True)]), fh)
        expect(run(btn, True, None, False) == 1,
               "focusable button trips the D-2 wire")

        two = os.path.join(td, "two")
        os.mkdir(two)
        with open(os.path.join(two, "tree-default.json"), "w") as fh:
            json.dump(_dump([
                _el("a", "textbox", "#0/a", focusable=True, focused=True),
                _el("b", "textbox", "#0/b", focusable=True, focused=True),
            ]), fh)
        expect(run(two, True, None, False) == 1, "two focused elements fail")

        ghost = write("tree-ghost.json", _dump([
            _el("a", "textbox", "#0/a", focusable=False, focused=True)]))
        expect(run(ghost, True, None, False) == 1,
               "focused-but-not-focusable fails")

        mismatch = os.path.join(td, "mm")
        os.mkdir(mismatch)
        with open(os.path.join(mismatch, "tree-default.json"), "w") as fh:
            json.dump(clean, fh)
        badexp = os.path.join(td, "badexp.json")
        with open(badexp, "w") as fh:
            json.dump({"default": {"focused_key": "proj-name"}}, fh)
        expect(run(mismatch, True, badexp, False) == 1,
               "expectation mismatch fails")
        with open(badexp, "w") as fh:
            json.dump({"nosuch": {"focused_key": None}}, fh)
        expect(run(mismatch, True, badexp, False) == 1,
               "promised-but-missing scene fails")

        expect(run(clean_dir, False, None, True) == 0,
               "report mode (json) exits 0 without --check")

    print("self-test:", "PASS" if not failures else f"{len(failures)} FAILURES")
    return 0 if not failures else 1


def main(argv):
    args = argv[1:]
    if "--self-test" in args:
        return self_test()
    do_check = "--check" in args
    as_json = "--json" in args
    expect_path = None
    positional = []
    i = 0
    while i < len(args):
        a = args[i]
        if a == "--expect":
            i += 1
            if i >= len(args):
                return die("--expect needs a file argument")
            expect_path = args[i]
        elif a in ("--check", "--json"):
            pass
        elif a.startswith("--"):
            return die(f"unknown flag {a}")
        else:
            positional.append(a)
        i += 1
    if len(positional) != 1:
        return die("usage: focus_census.py DUMP.json|DIR [--check] "
                   "[--expect FILE] [--json] | --self-test")
    return run(positional[0], do_check, expect_path, as_json)


if __name__ == "__main__":
    sys.exit(main(sys.argv))
