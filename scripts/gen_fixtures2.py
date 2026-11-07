#!/usr/bin/env python3
"""Generate the Stick Nodes fixtures in tests/fixtures/ (wave 7).

Companion to gen_fixtures.py, which covers the Pivot corpus. These
two fixtures come from the ORIGINAL app's own test material, so the
port's Stick Nodes importers are checked against bytes the original
project itself produced:

  figure.nodes    the `modernNodesBase64` blob from the original's
                  tests/sticknodes-synthetic.json — a version 431 /
                  build 117 figure: root plus one rounded segment
                  (id 7, local angle 0, length 80, thickness 4).

  project.nodemc  a 431/117 movieclip built by the original test
                  suite's own `syntheticClip` helper
                  (tests/sticknodes-import.test.mjs), extracted
                  verbatim from that file and run under Node with
                  the figure above embedded as its library rig.
                  The clip is named "Synthetic clip", runs at
                  24 fps, and its single frame holds no instances,
                  so it imports as a project with zero figures.
                  Their decoder reports 389 bytes for this clip;
                  the length is asserted here.

Both fixtures are merged into the existing manifest.json (the Pivot
entries are preserved untouched).

Run:  python3 scripts/gen_fixtures2.py     (requires `node` on PATH)
"""
from __future__ import annotations

import base64
import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
FIXTURES = REPO / "tests" / "fixtures"

# The original app's checkout (sibling of this repo). Override with
# STYK_ORIGINAL_APP if the layout differs.
ORIGINAL = Path(
    os.environ.get(
        "STYK_ORIGINAL_APP",
        Path.home() / "workspace" / "StykAnimate-App",
    )
)
SYNTHETIC_JSON = ORIGINAL / "tests" / "sticknodes-synthetic.json"
IMPORT_TEST = ORIGINAL / "tests" / "sticknodes-import.test.mjs"

# Values the port's importers produce for these fixtures; asserted
# by the corpus tests in src/mcp_io.rs. The .nodes importer places
# the figure on a fixed 960x720 stage (nodes::import_nodes), root
# at its centre; nodes_anim uses the same stage constant.
CANVAS = [960, 720]


def build_nodes() -> bytes:
    data = json.loads(SYNTHETIC_JSON.read_text())
    return base64.b64decode(data["modernNodesBase64"])


def build_nodemc(nodes: bytes) -> bytes:
    """Run the original suite's syntheticClip helper under Node."""
    source = IMPORT_TEST.read_text()
    start = source.index("function syntheticClip")
    end = source.index("\n}", start) + 2
    helper = source[start:end]
    runner = (
        "const {readFileSync, writeFileSync} = require('fs');\n"
        + helper
        + "\nconst nodes = readFileSync(process.argv[2]);\n"
        + "writeFileSync(process.argv[3], syntheticClip(nodes));\n"
    )
    with tempfile.TemporaryDirectory() as td:
        td = Path(td)
        (td / "clip.cjs").write_text(runner)
        (td / "rig.nodes").write_bytes(nodes)
        out = td / "project.nodemc"
        subprocess.run(
            ["node", str(td / "clip.cjs"), str(td / "rig.nodes"), str(out)],
            check=True,
        )
        blob = out.read_bytes()
    # Their decoder reports 389 bytes for this exact construction
    # (see the port's nodes_anim tests); guard against drift.
    assert len(blob) == 389, f"syntheticClip produced {len(blob)} bytes, expected 389"
    return blob


def main() -> None:
    FIXTURES.mkdir(parents=True, exist_ok=True)
    nodes = build_nodes()
    nodemc = build_nodemc(nodes)
    built = {"figure.nodes": nodes, "project.nodemc": nodemc}
    for name, data in built.items():
        (FIXTURES / name).write_bytes(data)
        print(f"wrote tests/fixtures/{name} ({len(data)} bytes)")

    manifest_path = FIXTURES / "manifest.json"
    manifest = json.loads(manifest_path.read_text())
    manifest["fixtures"]["figure.nodes"] = {
        "format": "sticknodes_nodes",
        "canvas": CANVAS,
        "fps": 24,
        "figures": [{"name": "Stick Nodes figure", "segments": 2}],
        "frames": 1,
        "frame0_roots": [[480.0, 360.0]],
        "notes": (
            "The original suite's modernNodesBase64 corpus figure "
            "(431/117). The container stores no canvas or fps; the "
            "values above are the importing project's: a fixed "
            "960x720 stage, 24 fps, the figure at rest at the stage "
            "centre. Two segments (root + one child, id 7, length "
            "80, thickness 4); the imported definition has one "
            "extra origin node."
        ),
    }
    manifest["fixtures"]["project.nodemc"] = {
        "format": "sticknodes_nodemc",
        "canvas": CANVAS,
        "fps": 24,
        "figures": [],
        "frames": 1,
        "frame0_roots": [],
        "notes": (
            "Built by the original test suite's syntheticClip "
            "helper (431/117) around the figure.nodes rig, 389 "
            "bytes. The clip is named 'Synthetic clip'; its only "
            "frame holds no instances, so it imports with zero "
            "figures and no poses."
        ),
    }
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")
    print("updated tests/fixtures/manifest.json")


if __name__ == "__main__":
    sys.exit(main())
