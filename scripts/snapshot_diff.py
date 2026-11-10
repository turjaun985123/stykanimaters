#!/usr/bin/env python3
"""Snapshot diff harness for headless --tree dumps (Roadmap PL-12).

Diffs two headless tree dumps element-by-element and reports
added / removed / moved / changed elements in a stable,
reviewable text format (or JSON). This is the tree-diff half
of PL-12's snapshot harness: per-scene baselines and budgets
are enforced by comparing a fresh headless artifact against a
reviewed baseline with this tool, at docs-close and in the
headless debrief -- never by raw pixel equality (refuted on
record) and never by eyeballing two 1,200-line JSON files.

Usage:
    python3 scripts/snapshot_diff.py OLD NEW [--eps PT] [--brief]
    python3 scripts/snapshot_diff.py OLD NEW --json
    python3 scripts/snapshot_diff.py --self-test

OLD and NEW are each either a single tree-*.json dump file or
a directory of dumps (the headless artifact layout). Two files
pair directly, even under different names (a mutated copy of a
baseline still diffs against it). Directory mode pairs scenes
by file name; a scene present on one side only is reported as
a scene-level add/remove and counts as a difference.

Exit codes: 0 = no differences, 1 = differences found,
2 = usage / IO / malformed-dump error.

What is compared, per element (the flat `elements` list -- the
counted surface the gates already use):
  identity   `path` (see matching below)
  geometry   `rect` [x, y, w, h]; position deltas beyond --eps
             (default 0.5 pt, the overflow scan's tolerance)
             are MOVED, size-only deltas are CHANGED rect.size
  attributes role, key, text (the label), help, fill, color,
             opacity, scale, blur, focusable, focused,
             focusVisible, inert, loopPhase -- a field that
             appears or disappears counts as a change
Scene level: element order (paint order) over the paths both
sides share, and the stable metadata fields title, viewport,
scale, dark_system, reduced_motion.

Matching: elements match by exact path first. Paths mix keyed
segments (app-bar/save) with positional `#n` ordinals for
unkeyed children, so inserting one unkeyed sibling renumbers
every following `#n` segment and the paths of all descendants.
For elements left unmatched, a second pass pairs them by the
signature (key, role, text) when that signature is unique
among the unmatched on BOTH sides; a pair found this way is
reported as MOVED (repathed), not as remove+add noise.
Signatures must be unique because real dumps contain duplicate
keys (tree-default.json carries view-fit, cam-zoom-in,
dur-dn/up and others twice -- menu + panel twins). Ambiguous
signatures fall back to plain added/removed. Children of a
repathed parent inherit the pairing by prefix cascade: an
unkeyed child (a button's icon/text) sitting at the
prefix-mapped path under a uniquely paired parent is the same
element, so whole subtrees repath together.

Dump-format quirks this tool lives with (measured on the
Wave AC closing artifact, 38 dumps):
- Fields are sparse: `key` on ~36% of elements, `text` on
  ~37%, `help` on ~19%, `color` only where an ink is set.
  Unkeyed, textless elements (groups, images) cannot be
  repath-matched; sibling churn around them surfaces as
  remove+add pairs. That noise is honest -- it is exactly
  where a reviewer's eye should go.
- The top-level fields t, anim_t, time_scale, animating and
  warnings change between any two runs of the same build and
  are excluded, or every diff would be noise.
- The parallel `accessibility` tree is NOT diffed: the flat
  `elements` list carries the same paths/rects plus the
  visual attributes, and is the surface the gates count.
- The root element's rect is the window. A viewport change
  (e.g. a -1280 variant against the 1440 baseline) honestly
  reports a metadata diff plus mass positional moves; pair
  like with like by file name in directory mode.
"""

import argparse
import json
import os
import sys
import tempfile
from collections import Counter

ATTRS = ("role", "key", "text", "help", "fill", "color",
         "opacity", "scale", "blur", "focusable", "focused",
         "focusVisible", "inert", "loopPhase")
META_FIELDS = ("title", "viewport", "scale", "dark_system",
               "reduced_motion")
DEFAULT_EPS = 0.5


class DiffError(Exception):
    """A dump cannot be read or is not a tree dump."""


# ----------------------------------------------------------------
# Loading / normalisation
# ----------------------------------------------------------------

def _record(el, where):
    if not isinstance(el, dict):
        raise DiffError(f"{where}: element is not an object")
    path = el.get("path")
    rect = el.get("rect")
    if not isinstance(path, str) or not path:
        raise DiffError(f"{where}: element lacks a string 'path'")
    if (not isinstance(rect, list) or len(rect) != 4
            or not all(isinstance(v, (int, float)) for v in rect)):
        raise DiffError(
            f"{where}: element {path!r} lacks a numeric rect[4]")
    attrs = {}
    for field in ATTRS:
        attrs[field] = el[field] if field in el else None
    return {"path": path,
            "rect": tuple(float(v) for v in rect),
            "attrs": attrs, "_present": frozenset(
                f for f in ATTRS if f in el)}


def load_dump(path):
    """Load one tree dump -> {meta, elements, order, count}."""
    try:
        with open(path, "r", encoding="utf-8") as fh:
            data = json.load(fh)
    except OSError as exc:
        raise DiffError(f"cannot read {path}: {exc}")
    except json.JSONDecodeError as exc:
        raise DiffError(f"{path}: not valid JSON: {exc}")
    if not isinstance(data, dict) \
            or not isinstance(data.get("elements"), list):
        raise DiffError(
            f"{path}: no top-level 'elements' list -- "
            "not a headless tree dump")
    elements = {}
    order = []
    for i, el in enumerate(data["elements"]):
        rec = _record(el, f"{path} element #{i}")
        if rec["path"] in elements:
            raise DiffError(
                f"{path}: duplicate element path {rec['path']!r}")
        elements[rec["path"]] = rec
        order.append(rec["path"])
    meta = {f: data[f] for f in META_FIELDS if f in data}
    return {"meta": meta, "elements": elements, "order": order,
            "count": len(order)}


def collect_scenes(path):
    """Map scene name -> dump file for a file or directory."""
    if os.path.isdir(path):
        scenes = {}
        for name in sorted(os.listdir(path)):
            if name.startswith("tree-") and name.endswith(".json"):
                scenes[name] = os.path.join(path, name)
        if not scenes:
            raise DiffError(
                f"{path}: directory holds no tree-*.json dumps")
        return scenes
    if os.path.isfile(path):
        return {os.path.basename(path): path}
    raise DiffError(f"{path}: no such file or directory")


# ----------------------------------------------------------------
# Diff engine
# ----------------------------------------------------------------

def _val_eq(old, new, eps):
    if isinstance(old, float) and isinstance(new, float):
        return abs(old - new) <= eps
    return old == new


def _attr_changes(old_rec, new_rec, eps):
    """Field-level changes between two matched records."""
    out = []
    for field in ATTRS:
        in_old = field in old_rec["_present"]
        in_new = field in new_rec["_present"]
        if not in_old and not in_new:
            continue
        if in_old != in_new:
            out.append((field,
                        old_rec["attrs"][field] if in_old else None,
                        new_rec["attrs"][field] if in_new else None,
                        "old" if not in_old else "new"))
        elif not _val_eq(old_rec["attrs"][field],
                         new_rec["attrs"][field], eps):
            out.append((field, old_rec["attrs"][field],
                        new_rec["attrs"][field], None))
    return out


def _signature(rec):
    """Repath-matching signature; None when unmatchable."""
    if "key" not in rec["_present"] or rec["attrs"]["key"] is None:
        return None
    text = rec["attrs"]["text"] if "text" in rec["_present"] else None
    return (rec["attrs"]["key"], rec["attrs"]["role"], text)


def diff_scene(old, new, eps):
    """Diff two loaded dumps -> scene diff dict."""
    meta = []
    for field in META_FIELDS:
        in_old = field in old["meta"]
        in_new = field in new["meta"]
        if not in_old and not in_new:
            continue
        if in_old != in_new or old["meta"].get(field) != new["meta"].get(field):
            meta.append((field, old["meta"].get(field),
                         new["meta"].get(field)))

    moved = []
    changed = []

    def compare(path, old_rec, new_rec, new_path=None):
        dx = new_rec["rect"][0] - old_rec["rect"][0]
        dy = new_rec["rect"][1] - old_rec["rect"][1]
        dw = new_rec["rect"][2] - old_rec["rect"][2]
        dh = new_rec["rect"][3] - old_rec["rect"][3]
        is_moved = abs(dx) > eps or abs(dy) > eps
        is_resized = abs(dw) > eps or abs(dh) > eps
        changes = _attr_changes(old_rec, new_rec, eps)
        if new_path is not None or is_moved:
            moved.append({
                "path": path, "new_path": new_path,
                "old_rect": old_rec["rect"],
                "new_rect": new_rec["rect"],
                "dx": dx, "dy": dy, "dw": dw, "dh": dh,
                "resized": is_resized,
                "repathed": new_path is not None,
                "attr_changes": len(changes)})
        elif is_resized:
            changes = [("rect.size", list(old_rec["rect"][2:]),
                        list(new_rec["rect"][2:]), None)] + changes
        if changes:
            changed.append({"path": new_path or path,
                            "changes": changes})

    old_only = {p: r for p, r in old["elements"].items()
                if p not in new["elements"]}
    new_only = {p: r for p, r in new["elements"].items()
                if p not in old["elements"]}

    for path, rec in old["elements"].items():
        if path in new["elements"]:
            compare(path, rec, new["elements"][path])

    # Repath pass: pair leftovers by unique (key, role, text).
    old_sigs = Counter(s for s in
                       (_signature(r) for r in old_only.values())
                       if s is not None)
    new_sigs = Counter(s for s in
                       (_signature(r) for r in new_only.values())
                       if s is not None)
    new_by_sig = {}
    for p, rec in new_only.items():
        sig = _signature(rec)
        if sig is not None:
            new_by_sig.setdefault(sig, []).append(p)
    pairs = []
    for path in sorted(old_only):
        sig = _signature(old_only[path])
        if sig is not None and old_sigs[sig] == 1 \
                and new_sigs.get(sig) == 1:
            pairs.append((path, new_by_sig[sig][0]))
    # Cascade: children of a repathed parent inherit the new
    # prefix. Unkeyed children (a button's icon/text) have no
    # signature of their own, but if the parent pairing is
    # unique, the child sitting at the prefix-mapped path in
    # the new tree is the same element. Shallow paths first so
    # nested repaths cascade down the subtree.
    queue = sorted(pairs, key=lambda pr: len(pr[0]))
    paired_old = {p for p, _n in pairs}
    paired_new = {n for _p, n in pairs}
    while queue:
        old_path, new_path = queue.pop(0)
        prefix = old_path + "/"
        for p in sorted(old_only):
            if p in paired_old or not p.startswith(prefix):
                continue
            cand = new_path + p[len(old_path):]
            if cand in new_only and cand not in paired_new:
                pairs.append((p, cand))
                paired_old.add(p)
                paired_new.add(cand)
                queue.append((p, cand))
    for old_path, new_path in pairs:
        compare(old_path, old_only.pop(old_path),
                new_only.pop(new_path), new_path=new_path)

    added = [new_only[p] for p in sorted(new_only)]
    removed = [old_only[p] for p in sorted(old_only)]
    moved.sort(key=lambda m: m["path"])
    changed.sort(key=lambda c: c["path"])

    common_old = [p for p in old["order"] if p in new["elements"]]
    common_new = [p for p in new["order"] if p in old["elements"]]
    order_changed = common_old != common_new

    total = (len(added) + len(removed) + len(moved) + len(changed)
             + len(meta) + (1 if order_changed else 0))
    return {"old_count": old["count"], "new_count": new["count"],
            "meta": meta, "added": added, "removed": removed,
            "moved": moved, "changed": changed,
            "order_changed": order_changed, "diffs": total}


def diff_run(old_arg, new_arg, eps):
    """Diff two files or two dump directories -> run result."""
    old_scenes = collect_scenes(old_arg)
    new_scenes = collect_scenes(new_arg)
    if os.path.isfile(old_arg) and os.path.isfile(new_arg):
        # Two explicitly named files are one pair, even when a
        # mutated copy carries a different file name; pair them
        # directly under the baseline's name.
        name = os.path.basename(old_arg)
        pairs = {name: (old_scenes[name],
                        new_scenes[os.path.basename(new_arg)])}
    else:
        pairs = {name: (old_scenes.get(name), new_scenes.get(name))
                 for name in set(old_scenes) | set(new_scenes)}
    scenes = {}
    totals = {"added": 0, "removed": 0, "moved": 0, "changed": 0,
              "scenes_added": 0, "scenes_removed": 0, "diffs": 0}
    for name in sorted(pairs):
        old_path, new_path = pairs[name]
        if old_path is None:
            dump = load_dump(new_path)
            scenes[name] = {"only": "new", "count": dump["count"]}
            totals["scenes_added"] += 1
            totals["diffs"] += 1
            continue
        if new_path is None:
            dump = load_dump(old_path)
            scenes[name] = {"only": "old", "count": dump["count"]}
            totals["scenes_removed"] += 1
            totals["diffs"] += 1
            continue
        sd = diff_scene(load_dump(old_path), load_dump(new_path), eps)
        scenes[name] = sd
        totals["added"] += len(sd["added"])
        totals["removed"] += len(sd["removed"])
        totals["moved"] += len(sd["moved"])
        totals["changed"] += len(sd["changed"])
        totals["diffs"] += sd["diffs"]
    return {"old": old_arg, "new": new_arg, "eps": eps,
            "scenes": scenes, "totals": totals,
            "clean": totals["diffs"] == 0}


# ----------------------------------------------------------------
# Rendering
# ----------------------------------------------------------------

def _fmt_num(v):
    return f"{v:.2f}"


def _fmt_rect(rect):
    return "[" + ", ".join(_fmt_num(v) for v in rect) + "]"


def _fmt_val(v):
    if isinstance(v, str):
        return repr(v)
    if isinstance(v, bool):
        return "true" if v else "false"
    if isinstance(v, float):
        return _fmt_num(v)
    if isinstance(v, (list, tuple)):
        return "[" + ", ".join(_fmt_val(x) for x in v) + "]"
    if v is None:
        return "null"
    return str(v)


def _describe(rec):
    parts = [rec["path"]]
    role = rec["attrs"].get("role")
    parts.append(f"[{role}]" if role is not None else "[?]")
    if "key" in rec["_present"]:
        parts.append(f"key={rec['attrs']['key']!r}")
    if "text" in rec["_present"]:
        parts.append(f"text={rec['attrs']['text']!r}")
    parts.append("rect " + _fmt_rect(rec["rect"]))
    return " ".join(parts)


def _change_text(field, old, new, absent):
    if absent == "old":
        return f"{field}: (absent) -> {_fmt_val(new)}"
    if absent == "new":
        return f"{field}: {_fmt_val(old)} -> (absent)"
    return f"{field}: {_fmt_val(old)} -> {_fmt_val(new)}"


def render_text(result, brief=False):
    lines = []
    lines.append("SNAPSHOT DIFF")
    lines.append(f"  old: {result['old']}")
    lines.append(f"  new: {result['new']}")
    lines.append(f"  eps: {result['eps']:.2f} pt")
    for name in sorted(result["scenes"]):
        sd = result["scenes"][name]
        if "only" in sd:
            side = "new" if sd["only"] == "new" else "old"
            lines.append(
                f"== {name} == ONLY IN {side.upper()} "
                f"({sd['count']} elements)")
            continue
        header = (f"== {name} == elements "
                  f"{sd['old_count']} -> {sd['new_count']}")
        if sd["diffs"] == 0:
            lines.append(header + " CLEAN")
            continue
        lines.append(header)
        lines.append(
            f"  added {len(sd['added'])}, removed "
            f"{len(sd['removed'])}, moved {len(sd['moved'])}, "
            f"changed {len(sd['changed'])}")
        for field, old, new in sd["meta"]:
            lines.append(
                f"  metadata {field}: {_fmt_val(old)}"
                f" -> {_fmt_val(new)}")
        if sd["order_changed"]:
            lines.append(
                "  element order changed (paint order over "
                "shared paths)")
        if brief:
            continue
        if sd["added"]:
            lines.append(f"ADDED ({len(sd['added'])}):")
            for rec in sd["added"]:
                lines.append("  + " + _describe(rec))
        if sd["removed"]:
            lines.append(f"REMOVED ({len(sd['removed'])}):")
            for rec in sd["removed"]:
                lines.append("  - " + _describe(rec))
        if sd["moved"]:
            lines.append(f"MOVED ({len(sd['moved'])}):")
            for m in sd["moved"]:
                head = f"  > {m['path']}"
                if m["repathed"]:
                    head += f" -> {m['new_path']} (repathed)"
                head += (f" rect [{_fmt_num(m['old_rect'][0])}, "
                         f"{_fmt_num(m['old_rect'][1])}] -> "
                         f"[{_fmt_num(m['new_rect'][0])}, "
                         f"{_fmt_num(m['new_rect'][1])}] "
                         f"(dx {m['dx']:+.2f}, dy {m['dy']:+.2f})")
                if m["resized"]:
                    head += (f"; size {_fmt_rect(m['old_rect'][2:])}"
                             f" -> {_fmt_rect(m['new_rect'][2:])}")
                if m["attr_changes"]:
                    head += f"; +{m['attr_changes']} attr change(s)"
                lines.append(head)
        if sd["changed"]:
            lines.append(f"CHANGED ({len(sd['changed'])}):")
            for c in sd["changed"]:
                parts = [_change_text(f, o, n, a)
                         for f, o, n, a in c["changes"]]
                lines.append(f"  ~ {c['path']} " + "; ".join(parts))
    t = result["totals"]
    verdict = "CLEAN" if result["clean"] else "DIFF FOUND"
    lines.append(
        f"TOTAL: {len(result['scenes'])} scene(s) | added "
        f"{t['added']}, removed {t['removed']}, moved "
        f"{t['moved']}, changed {t['changed']}, scenes "
        f"+{t['scenes_added']}/-{t['scenes_removed']} | {verdict}")
    return "\n".join(lines)


def _rec_json(rec):
    out = {"path": rec["path"], "rect": [round(v, 4) for v in rec["rect"]]}
    for field in ATTRS:
        if field in rec["_present"]:
            out[field] = rec["attrs"][field]
    return out


def _result_json(result):
    scenes = {}
    for name, sd in result["scenes"].items():
        if "only" in sd:
            scenes[name] = {"only": sd["only"], "count": sd["count"]}
            continue
        scenes[name] = {
            "old_count": sd["old_count"],
            "new_count": sd["new_count"],
            "meta": [{"field": f, "old": o, "new": n}
                     for f, o, n in sd["meta"]],
            "order_changed": sd["order_changed"],
            "added": [_rec_json(r) for r in sd["added"]],
            "removed": [_rec_json(r) for r in sd["removed"]],
            "moved": [{
                "path": m["path"], "new_path": m["new_path"],
                "old_rect": [round(v, 4) for v in m["old_rect"]],
                "new_rect": [round(v, 4) for v in m["new_rect"]],
                "dx": round(m["dx"], 4), "dy": round(m["dy"], 4),
                "resized": m["resized"], "repathed": m["repathed"],
                "attr_changes": m["attr_changes"],
            } for m in sd["moved"]],
            "changed": [{
                "path": c["path"],
                "changes": [{"field": f, "old": o, "new": n,
                             "absent": a}
                            for f, o, n, a in c["changes"]],
            } for c in sd["changed"]],
            "diffs": sd["diffs"],
        }
    return {"old": result["old"], "new": result["new"],
            "eps": result["eps"], "scenes": scenes,
            "totals": result["totals"], "clean": result["clean"]}


# ----------------------------------------------------------------
# Self-test (synthetic dumps, in the real format)
# ----------------------------------------------------------------

def _el(path, rect, **kw):
    el = {"path": path, "rect": rect, "blur": 0.0,
          "focusVisible": False, "focusable": False,
          "focused": False, "inert": False, "loopPhase": None,
          "opacity": 1.0, "scale": 1.0}
    el.update(kw)
    return el


def _fixture(mutated=False, swapped=False):
    els = [
        _el("#0", [0, 0, 1440, 900], role="group",
            fill="#101216ff"),
        _el("#0/app-bar", [16, 16, 1408, 25], role="group",
            fill="#00000000"),
        _el("#0/app-bar/save", [1126.5, 16, 65.88, 25],
            role="button", key="save", help="Save",
            fill="#1d222bff"),
        _el("#0/app-bar/save/#0", [1136.5, 22, 11.38, 13],
            role="image", fill="#00000000"),
        _el("#0/app-bar/save/#1", [1153.88, 21, 28.5, 15],
            role="text", text="Save", color="#f1f3f7ff",
            fill="#00000000"),
        _el("#0/panel/#5/save-proj", [97.75, 296.38, 108.88, 25],
            role="button", key="save-proj", help="Save project",
            fill="#1d222bff"),
        _el("#0/panel/#5/save-proj/#1",
            [125.13, 301.38, 71.5, 15], role="text",
            text="Save project", color="#f1f3f7ff",
            fill="#00000000"),
        _el("#0/panel/jitter", [10, 10, 50, 20], role="group",
            fill="#111111ff"),
        _el("#0/panel/wide", [10, 40, 50, 20], role="group",
            fill="#222222ff"),
        _el("#0/panel/doomed", [10, 70, 50, 20], role="button",
            key="doomed", fill="#333333ff"),
        _el("#0/x/dup", [10, 100, 50, 20], role="button",
            key="dup", text="Dup", fill="#444444ff"),
    ]
    if mutated:
        out = []
        for el in els:
            el = dict(el)
            p = el["path"]
            if p == "#0/panel/doomed":
                continue  # removed
            if p.startswith("#0/app-bar/save"):
                el["rect"] = [el["rect"][0] + 12.0] + el["rect"][1:]
            if p == "#0/app-bar/save/#1":
                el["text"] = "Save!"  # label change
            if p.startswith("#0/panel/#5/save-proj"):
                el["path"] = p.replace("/#5/", "/#9/")  # repath
            if p == "#0/panel/jitter":
                el["rect"] = [el["rect"][0] + 0.2] + el["rect"][1:]
                el["fill"] = "#999999ff"  # sub-eps move + fill change
            if p == "#0/panel/wide":
                el["rect"] = el["rect"][:2] + [64.0, 20]  # resize only
            if p == "#0/x/dup":
                el["path"] = "#0/y/dup"  # ambiguous: two new dups
            out.append(el)
        out.append(_el("#0/z/dup", [10, 130, 50, 20],
                       role="button", key="dup", text="Dup",
                       fill="#444444ff"))
        out.append(_el("#0/panel/fresh", [10, 160, 50, 20],
                       role="button", key="fresh",
                       fill="#555555ff"))
        els = out
    if swapped:
        els = list(els)
        els[2], els[3] = els[3], els[2]  # paint-order swap
    return {"accessibility": {"role": "window", "children": []},
            "anim_t": 0.0, "animating": False, "dark_system": True,
            "elements": els, "reduced_motion": False, "scale": 2.0,
            "t": 1.0, "time_scale": 1.0, "title": "StykAnimate RS",
            "viewport": [1280.0, 800.0] if mutated
            else [1440.0, 900.0],
            "warnings": []}


def self_test():
    failures = []

    def check(cond, label):
        print(("ok   " if cond else "FAIL ") + label)
        if not cond:
            failures.append(label)

    with tempfile.TemporaryDirectory() as td:
        base_p = os.path.join(td, "tree-base.json")
        mut_p = os.path.join(td, "tree-mut.json")
        swap_p = os.path.join(td, "tree-swap.json")
        for path, fx in ((base_p, _fixture()),
                         (mut_p, _fixture(mutated=True)),
                         (swap_p, _fixture(swapped=True))):
            with open(path, "w", encoding="utf-8") as fh:
                json.dump(fx, fh)

        base = load_dump(base_p)
        mut = load_dump(mut_p)

        same = diff_scene(base, load_dump(base_p), DEFAULT_EPS)
        check(same["diffs"] == 0, "identical dumps are CLEAN")

        sd = diff_scene(base, mut, DEFAULT_EPS)
        added_paths = {r["path"] for r in sd["added"]}
        removed_paths = {r["path"] for r in sd["removed"]}
        moved_paths = {m["path"] for m in sd["moved"]}
        changed = {c["path"]: c["changes"] for c in sd["changed"]}

        check(added_paths == {"#0/panel/fresh", "#0/y/dup",
                              "#0/z/dup"},
              f"added set exact (got {sorted(added_paths)})")
        check(removed_paths == {"#0/panel/doomed", "#0/x/dup"},
              f"removed set exact (got {sorted(removed_paths)})")
        save_moves = {m["path"]: m for m in sd["moved"]
                      if m["path"].startswith("#0/app-bar/save")}
        check(len(save_moves) == 3 and all(
            abs(m["dx"] - 12.0) < 1e-9 for m in save_moves.values()),
            "save subtree reported MOVED with dx +12.00")
        check("#0/app-bar/save/#1" in changed and any(
            f == "text" and o == "Save" and n == "Save!"
            for f, o, n, _a in changed["#0/app-bar/save/#1"]),
            "label change 'Save' -> 'Save!' caught")
        repathed = {m["path"]: m for m in sd["moved"] if m["repathed"]}
        check(set(repathed) == {"#0/panel/#5/save-proj",
                                "#0/panel/#5/save-proj/#1"}
              and all(m["new_path"] ==
                      m["path"].replace("/#5/", "/#9/")
                      for m in repathed.values()),
            "keyed subtree under shifted ordinal is repathed, "
            "not added/removed")
        check("#0/x/dup" not in moved_paths
              and "#0/y/dup" not in moved_paths,
              "ambiguous duplicate-key signature falls back to "
              "added/removed")
        check("#0/panel/jitter" not in moved_paths
              and "#0/panel/jitter" in changed and any(
                  f == "fill" for f, _o, _n, _a
                  in changed["#0/panel/jitter"]),
              "sub-eps jitter ignored; its fill change caught")
        check("#0/panel/wide" not in moved_paths
              and any(f == "rect.size" for f, _o, _n, _a
                      in changed.get("#0/panel/wide", [])),
              "size-only change is CHANGED rect.size, not MOVED")
        check(any(f == "viewport" for f, _o, _n in sd["meta"]),
              "viewport metadata change caught")
        check(not sd["order_changed"],
              "no false paint-order alarm on the mutation")

        swap = diff_scene(base, load_dump(swap_p), DEFAULT_EPS)
        check(swap["order_changed"] and swap["diffs"] == 1,
              "paint-order swap detected, nothing else fires")

        strict = diff_scene(base, mut, 0.0)
        check("#0/panel/jitter" in
              {m["path"] for m in strict["moved"]},
              "--eps 0 reports the 0.2 pt jitter as MOVED")

        # Directory mode + end-to-end exit codes.
        d_old = os.path.join(td, "old")
        d_new = os.path.join(td, "new")
        os.makedirs(d_old)
        os.makedirs(d_new)
        for d, fx in ((d_old, _fixture()), (d_new, _fixture())):
            with open(os.path.join(d, "tree-same.json"), "w",
                      encoding="utf-8") as fh:
                json.dump(fx, fh)
        with open(os.path.join(d_new, "tree-extra.json"), "w",
                  encoding="utf-8") as fh:
            json.dump(_fixture(), fh)
        run = diff_run(d_old, d_new, DEFAULT_EPS)
        check(run["totals"]["scenes_added"] == 1
              and run["totals"]["diffs"] == 1
              and not run["clean"],
              "directory mode pairs scenes; extra scene counts")
        renamed = diff_run(base_p, mut_p, DEFAULT_EPS)
        check(len(renamed["scenes"]) == 1
              and renamed["totals"]["scenes_added"] == 0
              and renamed["totals"]["diffs"] > 0,
              "two files pair directly despite different names")
        import contextlib
        import io

        def quiet_main(argv):
            with contextlib.redirect_stdout(io.StringIO()), \
                    contextlib.redirect_stderr(io.StringIO()):
                return main(argv)

        check(quiet_main([base_p, base_p]) == 0,
              "main() exits 0 on identical files")
        check(quiet_main([base_p, mut_p]) == 1,
              "main() exits 1 when differences exist")
        check(quiet_main([os.path.join(td, "nope.json"),
                          base_p]) == 2,
              "main() exits 2 on a missing file")
        bad_p = os.path.join(td, "tree-bad.json")
        with open(bad_p, "w", encoding="utf-8") as fh:
            json.dump({"nope": True}, fh)
        check(quiet_main([bad_p, base_p]) == 2,
              "main() exits 2 on a malformed dump")

    if failures:
        print(f"SELF-TEST FAILED: {len(failures)} check(s)")
        return 1
    print("SELF-TEST PASSED")
    return 0


# ----------------------------------------------------------------
# Entry point
# ----------------------------------------------------------------

def main(argv=None):
    parser = argparse.ArgumentParser(
        prog="snapshot_diff.py",
        description="Diff two headless tree dumps (files or "
                    "dump directories) element-by-element.")
    parser.add_argument("old", nargs="?", help="baseline dump/dir")
    parser.add_argument("new", nargs="?", help="candidate dump/dir")
    parser.add_argument("--eps", type=float, default=DEFAULT_EPS,
                        help="positional tolerance in pt "
                             "(default 0.5)")
    parser.add_argument("--json", action="store_true",
                        help="emit the diff as JSON")
    parser.add_argument("--brief", action="store_true",
                        help="per-scene summaries only, no "
                             "element lines")
    parser.add_argument("--self-test", action="store_true",
                        help="run the synthetic-fixture self-test")
    args = parser.parse_args(argv)

    if args.self_test:
        return self_test()
    if not args.old or not args.new:
        parser.print_usage(sys.stderr)
        return 2
    try:
        result = diff_run(args.old, args.new, args.eps)
    except DiffError as exc:
        print(f"snapshot_diff: {exc}", file=sys.stderr)
        return 2
    if args.json:
        print(json.dumps(_result_json(result), indent=2,
                         sort_keys=True))
    else:
        print(render_text(result, brief=args.brief))
    return 0 if result["clean"] else 1


if __name__ == "__main__":
    sys.exit(main())
