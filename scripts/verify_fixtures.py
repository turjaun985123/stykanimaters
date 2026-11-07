#!/usr/bin/env python3
"""Verify the cross-format corpus against the ORIGINAL's decoders.

Decodes every fixture in tests/fixtures/ with the original app's own
codec code (~/workspace/StykAnimate-App/codecs) and asserts that each
value recorded in tests/fixtures/manifest.json is reproduced exactly:

  legacy .piv  -> piv_legacy.decode_legacy_payload
  modern .piv  -> piv_codec.decode_piv_bytes
  legacy .stk  -> codec_bridge.stk_document (+ pivot_decoder.decode_stk_file)

Prints PASS per fixture; exits non-zero on the first mismatch.

Run:  python3 scripts/gen_fixtures.py && python3 scripts/verify_fixtures.py
"""
from __future__ import annotations

import json
import os
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
FIXTURES = REPO / "tests" / "fixtures"

CODECS = Path(
    os.environ.get(
        "STYK_ORIGINAL_CODECS",
        Path.home() / "workspace" / "StykAnimate-App" / "codecs",
    )
)
sys.path.insert(0, str(CODECS))

from codec_bridge import stk_document  # noqa: E402
from piv_codec import decode_piv_bytes  # noqa: E402
from piv_legacy import decode_legacy_payload  # noqa: E402
from pivot_decoder import DecodeOptions, decode_stk_file  # noqa: E402

TOL = 1e-4
TRIPOD_LENGTHS = [50.0, 30.0, 16.0]


def check(cond, what):
    if not cond:
        raise AssertionError(what)


def close(a, b):
    return abs(float(a) - float(b)) <= TOL


def verify_piv(name, data, expect):
    if expect["format"] == "piv_v5_standalone":
        report = decode_piv_bytes(data)
    else:
        report = decode_legacy_payload(data)
    check(report["format"] == expect["format"],
          f"{name}: format {report['format']!r} != {expect['format']!r}")
    check([report["canvas_width"], report["canvas_height"]] == expect["canvas"],
          f"{name}: canvas {[report['canvas_width'], report['canvas_height']]} != {expect['canvas']}")
    check(report["fps"] == expect["fps"],
          f"{name}: fps {report['fps']} != {expect['fps']}")
    for want in expect["figures"]:
        fig = next((f for f in report["figure_types"] if f["name"] == want["name"]), None)
        check(fig is not None, f"{name}: figure {want['name']!r} not found in "
                               f"{[f['name'] for f in report['figure_types']]}")
        check(fig["segment_count"] == want["segments"],
              f"{name}: {want['name']} segment_count {fig['segment_count']} != {want['segments']}")
        lengths = [s["length"] for s in fig["segments"]]
        check(len(lengths) == len(TRIPOD_LENGTHS) and all(close(a, b) for a, b in zip(lengths, TRIPOD_LENGTHS)),
              f"{name}: {want['name']} segment lengths {lengths} != {TRIPOD_LENGTHS}")
    frames = report["keyframes"]
    check(len(frames) == expect["frames"],
          f"{name}: frame count {len(frames)} != {expect['frames']}")
    roots = [[o["root_x"], o["root_y"]] for o in frames[0]["objects"]]
    check(len(roots) == len(expect["frame0_roots"]) and
          all(close(a, b) for got, want in zip(roots, expect["frame0_roots"]) for a, b in zip(got, want)),
          f"{name}: frame-0 roots {roots} != {expect['frame0_roots']}")


def verify_stk(name, path, data, expect):
    report = decode_stk_file(path, options=DecodeOptions())
    check(report["format"] == expect["format"] and report["decode_status"] == "decoded",
          f"{name}: stk decode format/status = {report['format']}/{report['decode_status']}")
    doc = stk_document(path, data)
    for want in expect["figures"]:
        fig = next((f for f in doc["figure_types"] if f["name"] == want["name"]), None)
        check(fig is not None, f"{name}: figure {want['name']!r} not found")
        check(fig["segment_count"] == want["segments"],
              f"{name}: segment_count {fig['segment_count']} != {want['segments']}")
        lengths = [s["length"] for s in fig["segments"]]
        check(all(close(a, b) for a, b in zip(lengths, TRIPOD_LENGTHS)),
              f"{name}: segment lengths {lengths} != {TRIPOD_LENGTHS}")
        check(fig["segments"][2]["primitive_code"] == 1,
              f"{name}: segment 3 should be a circle (primitive 1)")


def main() -> int:
    manifest = json.loads((FIXTURES / "manifest.json").read_text())
    failures = 0
    for name, expect in manifest["fixtures"].items():
        path = FIXTURES / name
        try:
            data = path.read_bytes()
            if name.endswith(".stk"):
                verify_stk(name, path, data, expect)
            else:
                verify_piv(name, data, expect)
        except Exception as exc:  # noqa: BLE001 - report and continue
            failures += 1
            print(f"FAIL {name}: {exc}")
        else:
            print(f"PASS {name}")
    if failures:
        print(f"{failures} fixture(s) failed")
        return 1
    print(f"All {len(manifest['fixtures'])} fixtures verified against the original's decoders.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
