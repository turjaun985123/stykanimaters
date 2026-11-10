#!/usr/bin/env python3
"""Perf budget instrument for headless --tree artifacts (Roadmap PL-13).

Seat W2-72. Measures what can honestly be measured from a
headless artifact directory (the `headless-probe-output` layout:
`tree-<scene>.json` dumps + `snap-<scene>.log` runtime lines):

  * element counts per PANEL FAMILY, attributed from element
    paths (family = the first keyed, non-`#n`, path segment --
    the panel key -- rolled up through FAMILY_GROUPS);
  * dump bytes per scene (serialisation + IO cost proxy);
  * layout nodes / glyphs / instances from the Rune runtime
    line in each snap log (the runtime's own census);
  * FIRST-FRAME cpu ms from the same line ("frame cpu mean"
    over the probe's single presented frame).

What this is NOT: a steady-state frame-time profiler. The
headless probe presents one frame per scene, so the cpu figure
is a first-frame cost on CI-runner hardware under Xvfb -- it
includes one-time work (notably Home thumbnail decode) and says
nothing about the 60 fps steady state on user hardware. No
number in this tool is a steady-state frame time, and the tool
never invents one. A real frame-build p50 needs a multi-frame
probe (filed as IA-W2-72-1).

Budgets are ratchet-style, in the KEY_TOTAL_FLOOR culture:
derived from a reviewed baseline artifact as worst-observed
plus headroom (families +25%, scene totals +15%), then frozen
into a budgets JSON. Later artifacts are checked against the
frozen budgets; a family that outgrows its budget is reported
as an EXCEEDANCE with the numbers, never silently re-based.

Usage:
    python3 scripts/perf_budget.py ARTIFACT_DIR
        Measure ARTIFACT_DIR and derive budgets from it
        (headroom report; exceedances vs the derived budgets
        are impossible by construction and the report says so,
        but the external 16.7 ms single-frame reference IS
        checked and reported).
    python3 scripts/perf_budget.py ARTIFACT_DIR --budgets B.json
        Check ARTIFACT_DIR against frozen budgets B.json.
        Exit 1 on any exceedance.
    python3 scripts/perf_budget.py ARTIFACT_DIR --emit-budgets B.json
        Write the budgets derived from ARTIFACT_DIR to B.json.
    python3 scripts/perf_budget.py ARTIFACT_DIR --json
        Machine-readable report.
    python3 scripts/perf_budget.py --self-test

Exit codes: 0 = within budgets, 1 = exceedances found,
2 = usage / IO / malformed-artifact error.

Stdlib only.
"""

import json
import math
import os
import re
import sys

FRAME_MS_60 = 16.7  # one frame at 60 fps, the external reference
FAMILY_HEADROOM = 1.25
SCENE_HEADROOM = 1.15
CPU_HEADROOM = 1.25

# Panel keys (first keyed path segment) rolled into families.
# Anything unmapped keeps its own key as its family name -- the
# report lists unmapped keys so coverage is never overstated.
FAMILY_GROUPS = {
    "stage": {"stage-wrap", "stage-sprite-layer"},
    "timeline": {"timeline", "timeline-loop-row", "frames"},
    "transport": {"transport", "playback-controls"},
    "status": {"status-strip"},
    "inspector": {
        "inspector-figure", "inspector-segment", "inspector-style",
        "keyframe-tools", "pose-tools",
    },
    "scene-panel": {"scene-panel"},
    "sprites": {"sprite-panel"},
    "clips": {"clips-panel"},
    "figures": {"figures-panel"},
    "layers": {"layers-panel"},
    "library": {"library-panel"},
    "project": {"project-tools"},
    "camera": {"camera-panel"},
    "onion": {"onion-controls"},
    "stats": {"stats-panel"},
    "settings": {"settings"},
    "workspaces": {"workspace-chips"},
    "app-bar": {"save", "load", "undo", "redo", "menu-file",
                "menu-edit", "menu-view", "menu-help", "ws-blurb"},
    "home": {"home", "home-grid", "recent-list"},
    "menus": {"menu", "palette"},
    "subtitles": {"subtitle-panel"},
    "audio": {"audio-panel"},
    "graph": {"graph-panel"},
    "dialogue": {"dialogue-lane"},
    "build": {"rig-tree"},
}
_KEY_TO_FAMILY = {}
for _fam, _keys in FAMILY_GROUPS.items():
    for _k in _keys:
        _KEY_TO_FAMILY[_k] = _fam

_RUNE_LINE = re.compile(
    r"frame cpu mean ([\d.]+) ms.*?(\d+) instances")
_NODES_LINE = re.compile(r"layout nodes (\d+), glyphs (\d+)")
_GPU_LINE = re.compile(r"gpu init ([\d.]+) ms, first frame ([\d.]+) ms")


COLUMNS = {"main-row", "workspace-row", "centre", "side-left",
           "side-right", "split-left", "split-right"}


def family_of(path):
    """Panel family for an element path: the first keyed
    (non-`#n`) segment after the root and after any structural
    row/column segments (main-row / centre / side-left / ...),
    mapped via FAMILY_GROUPS; unmapped keys are their own
    family; a path with no keyed segment is 'shell-root'."""
    segs = [s for s in path.split("/")[1:] if s]
    while segs and segs[0] in COLUMNS:
        segs = segs[1:]
    for seg in segs:
        if not seg.startswith("#"):
            return _KEY_TO_FAMILY.get(seg, seg), seg
    return "shell-root", ""


def load_scene(tree_path, log_path):
    with open(tree_path, "r", encoding="utf-8") as fh:
        dump = json.load(fh)
    elements = dump.get("elements")
    if not isinstance(elements, list):
        raise ValueError(f"{tree_path}: no elements list")
    fams = {}
    unmapped = set()
    for el in elements:
        fam, key = family_of(str(el.get("path", "")))
        fams[fam] = fams.get(fam, 0) + 1
        if key and key not in _KEY_TO_FAMILY:
            unmapped.add(key)
    scene = {
        "elements": len(elements),
        "bytes": os.path.getsize(tree_path),
        "families": fams,
        "unmapped_keys": sorted(unmapped),
        "cpu_ms": None, "instances": None, "nodes": None,
        "glyphs": None, "gpu_init_ms": None, "first_frame_ms": None,
    }
    if log_path and os.path.exists(log_path):
        text = open(log_path, "r", encoding="utf-8",
                    errors="ignore").read()
        m = _RUNE_LINE.search(text)
        if m:
            scene["cpu_ms"] = float(m.group(1))
            scene["instances"] = int(m.group(2))
        m = _NODES_LINE.search(text)
        if m:
            scene["nodes"] = int(m.group(1))
            scene["glyphs"] = int(m.group(2))
        m = _GPU_LINE.search(text)
        if m:
            scene["gpu_init_ms"] = float(m.group(1))
            scene["first_frame_ms"] = float(m.group(2))
    return scene


def collect(artifact_dir):
    if not os.path.isdir(artifact_dir):
        raise ValueError(f"not a directory: {artifact_dir}")
    scenes = {}
    for name in sorted(os.listdir(artifact_dir)):
        if name.startswith("tree-") and name.endswith(".json"):
            scene_name = name[5:-5]
            log = os.path.join(artifact_dir, f"snap-{scene_name}.log")
            scenes[scene_name] = load_scene(
                os.path.join(artifact_dir, name), log)
    if not scenes:
        raise ValueError(f"no tree-*.json dumps in {artifact_dir}")
    return scenes


def _ceil_to(value, step):
    return int(math.ceil(value / step) * step)


def derive_budgets(scenes):
    """Ratchet budgets from a baseline artifact: worst observed
    per family / per scene, plus headroom, rounded up."""
    fam_worst = {}
    for sc in scenes.values():
        for fam, n in sc["families"].items():
            fam_worst[fam] = max(fam_worst.get(fam, 0), n)
    budgets = {
        "families": {
            fam: _ceil_to(n * FAMILY_HEADROOM, 10)
            for fam, n in sorted(fam_worst.items())
        },
        "scene_elements": {
            name: _ceil_to(sc["elements"] * SCENE_HEADROOM, 10)
            for name, sc in sorted(scenes.items())
        },
        "scene_cpu_ms": {
            name: round(sc["cpu_ms"] * CPU_HEADROOM, 2)
            for name, sc in sorted(scenes.items())
            if sc["cpu_ms"] is not None
        },
        "headroom": {"family": FAMILY_HEADROOM,
                     "scene": SCENE_HEADROOM, "cpu": CPU_HEADROOM},
    }
    return budgets


def family_table(scenes):
    """Per-family aggregate: worst scene, mean per present
    scene, scene count."""
    agg = {}
    for name, sc in scenes.items():
        for fam, n in sc["families"].items():
            row = agg.setdefault(fam, {"worst": 0, "worst_scene": "",
                                       "total": 0, "scenes": 0})
            row["total"] += n
            row["scenes"] += 1
            if n > row["worst"]:
                row["worst"] = n
                row["worst_scene"] = name
    for row in agg.values():
        row["mean"] = round(row["total"] / row["scenes"], 1)
    return dict(sorted(agg.items(), key=lambda kv: -kv[1]["worst"]))


def check(scenes, budgets):
    """Exceedances of `scenes` against frozen `budgets`."""
    out = []
    agg = family_table(scenes)
    for fam, budget in budgets.get("families", {}).items():
        row = agg.get(fam)
        if row and row["worst"] > budget:
            out.append(
                f"family {fam}: worst {row['worst']} "
                f"({row['worst_scene']}) > budget {budget}")
    for name, budget in budgets.get("scene_elements", {}).items():
        sc = scenes.get(name)
        if sc and sc["elements"] > budget:
            out.append(
                f"scene {name}: {sc['elements']} elements "
                f"> budget {budget}")
    for name, budget in budgets.get("scene_cpu_ms", {}).items():
        sc = scenes.get(name)
        if sc and sc["cpu_ms"] is not None and sc["cpu_ms"] > budget:
            out.append(
                f"scene {name}: first-frame cpu {sc['cpu_ms']} ms "
                f"> budget {budget} ms")
    return out


def render_text(scenes, budgets, derived_from_self, exceedances):
    lines = []
    agg = family_table(scenes)
    lines.append("PERF BUDGET REPORT (PL-13, seat W2-72)")
    lines.append(
        "cost proxies: element counts per panel family + dump "
        "bytes; timing: FIRST-frame cpu ms from the Rune runtime "
        "line in each snap log (1 presented frame, CI runner, "
        "Xvfb -- includes one-time work; NOT a steady-state "
        "frame time).")
    lines.append("")
    lines.append("== Panel families (elements) ==")
    lines.append(f"{'family':<14} {'worst':>6} {'budget':>7} "
                 f"{'head':>6} {'mean':>6} {'scenes':>6}  worst scene")
    for fam, row in agg.items():
        budget = budgets["families"].get(fam, 0)
        head = (f"{100.0 * (budget - row['worst']) / budget:.0f}%"
                if budget else "  -")
        lines.append(f"{fam:<14} {row['worst']:>6} {budget:>7} "
                     f"{head:>6} {row['mean']:>6} {row['scenes']:>6}"
                     f"  {row['worst_scene']}")
    unmapped = sorted({k for sc in scenes.values()
                       for k in sc["unmapped_keys"]})
    if unmapped:
        lines.append(f"unmapped panel keys (own family): "
                     f"{', '.join(unmapped)}")
    lines.append("")
    lines.append("== Scenes ==")
    lines.append(f"{'scene':<26} {'elems':>6} {'budget':>7} "
                 f"{'KiB':>7} {'cpu ms':>8} {'glyphs':>7}")
    for name in sorted(scenes, key=lambda n: -scenes[n]["elements"]):
        sc = scenes[name]
        cpu = f"{sc['cpu_ms']:.2f}" if sc["cpu_ms"] is not None else "-"
        gly = sc["glyphs"] if sc["glyphs"] is not None else "-"
        lines.append(
            f"{name:<26} {sc['elements']:>6} "
            f"{budgets['scene_elements'].get(name, 0):>7} "
            f"{sc['bytes'] / 1024:>7.0f} {cpu:>8} {str(gly):>7}")
    lines.append("")
    lines.append(f"== External reference: {FRAME_MS_60} ms "
                 f"single-frame budget (60 fps) ==")
    over = [(n, sc["cpu_ms"]) for n, sc in scenes.items()
            if sc["cpu_ms"] is not None and sc["cpu_ms"] > FRAME_MS_60]
    if over:
        for name, cpu in sorted(over, key=lambda r: -r[1]):
            lines.append(f"OVER  {name}: first-frame cpu "
                         f"{cpu:.2f} ms (first frame incl. "
                         f"one-time work; see findings)")
    else:
        lines.append("no scene over the reference.")
    lines.append("")
    if derived_from_self:
        lines.append(
            "Budgets above were DERIVED from this artifact "
            "(worst + headroom): exceedances against them are "
            "impossible by construction. Freeze them "
            "(--emit-budgets) and check future artifacts with "
            "--budgets.")
    if exceedances:
        lines.append("EXCEEDANCES vs frozen budgets:")
        lines.extend(f"  {e}" for e in exceedances)
    elif not derived_from_self:
        lines.append("No exceedances vs frozen budgets.")
    return "\n".join(lines) + "\n"


def _fixture_scenes():
    def sc(elements, fams, cpu):
        return {"elements": elements, "bytes": elements * 900,
                "families": fams, "unmapped_keys": [],
                "cpu_ms": cpu, "instances": elements * 8,
                "nodes": elements, "glyphs": elements // 2,
                "gpu_init_ms": 1.0, "first_frame_ms": cpu}
    return {
        "alpha": sc(100, {"stage": 40, "timeline": 60}, 10.0),
        "beta": sc(200, {"stage": 150, "inspector": 50}, 20.0),
    }


def self_test():
    fails = []

    def expect(cond, label):
        if not cond:
            fails.append(label)

    fam, key = family_of("#0/centre/#2/timeline/#0/dope-cell-3")
    expect(fam == "timeline" and key == "timeline",
           "timeline attribution through #n segments")
    fam, key = family_of("#0/side-right/inspector-figure/#1")
    expect(fam == "inspector", "inspector group mapping")
    fam, _ = family_of("#0")
    expect(fam == "shell-root", "root path family")
    fam, key = family_of("#0/side-left/mystery-panel/#0")
    expect(fam == "mystery-panel" and key == "mystery-panel",
           "unmapped key keeps its own family")
    scenes = _fixture_scenes()
    budgets = derive_budgets(scenes)
    expect(budgets["families"]["stage"] == 190,
           f"stage budget 150*1.25->190, got "
           f"{budgets['families'].get('stage')}")
    expect(budgets["scene_elements"]["beta"] == 230,
           "scene budget 200*1.15->230")
    expect(check(scenes, budgets) == [],
           "baseline within its own derived budgets")
    grown = _fixture_scenes()
    grown["beta"]["families"]["stage"] = 500
    grown["beta"]["elements"] = 900
    exc = check(grown, budgets)
    expect(any("family stage" in e for e in exc),
           "grown family flagged")
    expect(any("scene beta" in e and "elements" in e for e in exc),
           "grown scene flagged")
    text = render_text(scenes, budgets, True, [])
    expect("timeline" in text and "DERIVED" in text,
           "report renders with derivation note")
    if fails:
        print("SELF-TEST FAIL:")
        for f in fails:
            print(f"  - {f}")
        return 1
    print("SELF-TEST PASS (10 checks)")
    return 0


def main(argv):
    args = []
    skip_next = False
    for a in argv:
        if skip_next:
            skip_next = False
            continue
        if a in ("--budgets", "--emit-budgets"):
            skip_next = True
            continue
        if not a.startswith("--"):
            args.append(a)
    if "--self-test" in argv:
        return self_test()
    if len(args) != 1:
        print(__doc__)
        return 2
    try:
        scenes = collect(args[0])
    except (ValueError, OSError, json.JSONDecodeError) as exc:
        print(f"error: {exc}")
        return 2
    if "--emit-budgets" in argv:
        idx = argv.index("--emit-budgets")
        if idx + 1 >= len(argv):
            print("error: --emit-budgets needs a path")
            return 2
        with open(argv[idx + 1], "w", encoding="utf-8") as fh:
            json.dump(derive_budgets(scenes), fh, indent=1)
        print(f"budgets written to {argv[idx + 1]}")
        return 0
    derived = True
    budgets = derive_budgets(scenes)
    exceedances = []
    if "--budgets" in argv:
        idx = argv.index("--budgets")
        if idx + 1 >= len(argv):
            print("error: --budgets needs a path")
            return 2
        with open(argv[idx + 1], "r", encoding="utf-8") as fh:
            budgets = json.load(fh)
        derived = False
        exceedances = check(scenes, budgets)
    if "--json" in argv:
        print(json.dumps({
            "families": family_table(scenes),
            "scenes": {n: {k: v for k, v in sc.items()
                           if k != "families" and
                           k != "unmapped_keys"}
                       for n, sc in scenes.items()},
            "budgets": budgets,
            "exceedances": exceedances,
            "derived_from_self": derived,
        }, indent=1))
    else:
        sys.stdout.write(render_text(scenes, budgets, derived,
                                     exceedances))
    return 1 if exceedances else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
