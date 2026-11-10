#!/usr/bin/env python3
"""Verdict aggregator for the headless probe artifact (Roadmap F-7).

The headless probe workflow (.github/workflows/headless.yml) is
fail-soft BY DESIGN: every probe step is continue-on-error so the
artifact always uploads, and each instrument records its strict
verdict only inside a transcript file. That contract has a known
failure mode, paid for three times on record: a green workflow
conclusion hiding a red transcript (Wave W's stale-premise MCP
probe), and a scan that was simply never run over an artifact
reported CLEAN (Wave X: the artifact held 40 overflow hits).

This aggregator is the strict reader those transcripts always
needed. It parses every verdict-bearing transcript in a probe
artifact directory, folds in the overflow scan (run here, over
the same directory), and emits ONE machine-readable verdict:

    <dir>/verdict.json   (or --out PATH)
    {"verdict": "PASS"|"FAIL", "checks": [{"name", "verdict",
     "detail"}, ...], ...}

and exits 0 iff the verdict is PASS. The rules are deliberately
unforgiving, because a binding net that passes vacuously is
worse than none:

  * A MISSING transcript is a FAIL, never a skip — the surface
    it proves was not proven.
  * UNPROVEN (the Home thumbnail probe's third verdict) is a
    FAIL here: unproven is not proven.
  * A FAIL assertion line anywhere in the MCP transcript fails
    the check even if a later line claims PASS — transcripts
    are read whole, not tail-grepped.
  * A recorded non-zero .exit beside a transcript fails the
    check even if the transcript reads PASS (the two records
    disagreeing is itself a defect).

Checks (transcript -> what PASS requires):
  subtitle-census.txt          last "SUBTITLE CENSUS:" line is PASS
  home-thumbnail-probe.txt     last "HOME THUMBNAIL PROBE:" line is PASS
  home-missing-audio-marker.txt  content is the PASS verdict line
  home-scan-fallback-marker.txt  content is the PASS verdict line
  mcp-probe.txt                last "MCP PROBE:" line is PASS, no FAIL lines
  overflow scan                overflow_scan.py --dir exits 0 (CLEAN,
                               scan set complete) — skipped by --skip-scan

(The rect probe is NOT a check: Q3 was answered long ago and its
text carries no verdict — it stays informational in the artifact.)

Usage:
    python3 scripts/verdict_aggregate.py <artifact-dir> [--out PATH]
                                         [--skip-scan]
    python3 scripts/verdict_aggregate.py --self-test

The --self-test builds synthetic transcript sets in a temp dir
and pins the aggregator's own behaviour: all-PASS passes; a FAIL
verdict, an UNPROVEN, a missing transcript, and a buried FAIL
assertion each fail. It is the anti-vacuity proof for the net
itself, and it runs in CI beside the other instruments'
self-tests.
"""

import json
import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))


# ---------------------------------------------------------------------
# Transcript readers. Each returns (ok: bool, detail: str).
# ---------------------------------------------------------------------

def _read(path):
    try:
        with open(path, encoding="utf-8", errors="replace") as fh:
            return fh.read()
    except OSError:
        return None


def _exit_file_blocks(dirpath, name):
    """A recorded non-zero exit beside a transcript fails the check."""
    path = os.path.join(dirpath, name)
    if not os.path.isfile(path):
        return None
    text = _read(path)
    if text is None:
        return f"{name} unreadable"
    text = text.strip()
    if text != "0":
        return f"{name} records exit {text!r}"
    return None


def _last_verdict_line(text, prefix):
    found = None
    for line in text.splitlines():
        if line.startswith(prefix):
            found = line.strip()
    return found


def check_subtitle_census(dirpath):
    text = _read(os.path.join(dirpath, "subtitle-census.txt"))
    if text is None:
        return False, "subtitle-census.txt missing — the census was not proven"
    for line in text.splitlines():
        if line.startswith("FAIL"):
            return False, f"census carries a FAIL line: {line.strip()[:120]}"
    verdict = _last_verdict_line(text, "SUBTITLE CENSUS:")
    if verdict is None:
        return False, "no SUBTITLE CENSUS verdict line in the transcript"
    if verdict != "SUBTITLE CENSUS: PASS":
        return False, f"census verdict: {verdict}"
    block = _exit_file_blocks(dirpath, "subtitle-census.exit")
    if block:
        return False, block
    return True, verdict


def check_home_thumbnail(dirpath):
    text = _read(os.path.join(dirpath, "home-thumbnail-probe.txt"))
    if text is None:
        return False, "home-thumbnail-probe.txt missing — thumbnails were not proven"
    verdict = _last_verdict_line(text, "HOME THUMBNAIL PROBE:")
    if verdict is None:
        return False, "no HOME THUMBNAIL PROBE verdict line in the transcript"
    if verdict.startswith("HOME THUMBNAIL PROBE: UNPROVEN"):
        return False, "thumbnail probe UNPROVEN — unproven is not proven"
    if not verdict.startswith("HOME THUMBNAIL PROBE: PASS"):
        return False, f"thumbnail verdict: {verdict[:120]}"
    return True, verdict


def _check_marker(dirpath, filename, prefix):
    text = _read(os.path.join(dirpath, filename))
    if text is None:
        return False, f"{filename} missing — the marker was not proven"
    verdict = text.strip()
    if verdict == f"{prefix}: PASS":
        return True, verdict
    return False, f"{filename} records: {verdict[:120]!r}"


def check_missing_audio_marker(dirpath):
    return _check_marker(
        dirpath, "home-missing-audio-marker.txt", "HOME MISSING-AUDIO MARKER"
    )


def check_scan_fallback_marker(dirpath):
    return _check_marker(
        dirpath, "home-scan-fallback-marker.txt", "HOME SCAN-FALLBACK MARKER"
    )


def check_mcp_probe(dirpath):
    text = _read(os.path.join(dirpath, "mcp-probe.txt"))
    if text is None:
        return False, "mcp-probe.txt missing — the live endpoint was not proven"
    for line in text.splitlines():
        if line.startswith("FAIL"):
            return False, f"MCP transcript carries a FAIL line: {line.strip()[:120]}"
    verdict = _last_verdict_line(text, "MCP PROBE:")
    if verdict is None:
        return False, "no MCP PROBE verdict line in the transcript"
    if verdict != "MCP PROBE: PASS":
        return False, f"MCP verdict: {verdict[:120]}"
    block = _exit_file_blocks(dirpath, "mcp-probe.exit")
    if block:
        return False, block
    return True, verdict


def check_overflow_scan(dirpath):
    script = os.path.join(HERE, "overflow_scan.py")
    proc = subprocess.run(
        [sys.executable, script, "--dir", dirpath],
        capture_output=True,
        text=True,
    )
    out = proc.stdout + proc.stderr
    summary = None
    for line in out.splitlines():
        if line.startswith("OVERFLOW SCAN:"):
            summary = line.strip()
    if proc.returncode != 0:
        return False, summary or f"overflow_scan exited {proc.returncode}"
    if summary is None or "CLEAN" not in summary:
        return False, summary or "overflow scan printed no verdict line"
    return True, summary


CHECKS = [
    ("subtitle_census", check_subtitle_census),
    ("home_thumbnail_probe", check_home_thumbnail),
    ("home_missing_audio_marker", check_missing_audio_marker),
    ("home_scan_fallback_marker", check_scan_fallback_marker),
    ("mcp_probe", check_mcp_probe),
]


def aggregate(dirpath, skip_scan=False):
    """Fold every check into one verdict dict. Pure reporting:
    returns (verdict_dict, exit_code)."""
    results = []
    for name, fn in CHECKS:
        ok, detail = fn(dirpath)
        results.append(
            {"name": name, "verdict": "PASS" if ok else "FAIL", "detail": detail}
        )
    if not skip_scan:
        ok, detail = check_overflow_scan(dirpath)
        results.append(
            {"name": "overflow_scan", "verdict": "PASS" if ok else "FAIL",
             "detail": detail}
        )
    verdict = "PASS" if all(r["verdict"] == "PASS" for r in results) else "FAIL"
    doc = {
        "tool": "verdict_aggregate.py (Roadmap F-7)",
        "dir": dirpath,
        "verdict": verdict,
        "checks": results,
    }
    return doc, 0 if verdict == "PASS" else 1


def main(argv):
    if len(argv) >= 2 and argv[1] == "--self-test":
        return self_test()
    if len(argv) < 2:
        print(__doc__)
        return 2
    dirpath = argv[1]
    out_path = None
    skip_scan = "--skip-scan" in argv
    if "--out" in argv:
        i = argv.index("--out")
        if i + 1 >= len(argv):
            print("--out needs a path")
            return 2
        out_path = argv[i + 1]
    if not os.path.isdir(dirpath):
        print(f"not a directory: {dirpath}")
        return 2
    doc, rc = aggregate(dirpath, skip_scan=skip_scan)
    if out_path is None:
        out_path = os.path.join(dirpath, "verdict.json")
    with open(out_path, "w", encoding="utf-8") as fh:
        json.dump(doc, fh, indent=2)
        fh.write("\n")
    lines = [f"verdict aggregate over {dirpath}:"]
    for r in doc["checks"]:
        lines.append(f"  [{r['verdict']}] {r['name']}: {r['detail']}")
    lines.append(f"VERDICT: {doc['verdict']} (machine-readable: {out_path})")
    text = "\n".join(lines)
    print(text)
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary:
        with open(summary, "a", encoding="utf-8") as fh:
            fh.write("\n### Binding verdict (F-7 aggregator)\n\n```\n" + text + "\n```\n")
    return rc


# ---------------------------------------------------------------------
# Self-test: synthetic transcript sets pinning the rules above.
# ---------------------------------------------------------------------

_GOOD = {
    "subtitle-census.txt": "PASS  a cue\nSUBTITLE CENSUS: PASS\n",
    "subtitle-census.exit": "0\n",
    "home-thumbnail-probe.txt": (
        "PASS  rows carry renders\n"
        "HOME THUMBNAIL PROBE: PASS (populated + missing-audio rows)\n"
    ),
    "home-missing-audio-marker.txt": "HOME MISSING-AUDIO MARKER: PASS\n",
    "home-scan-fallback-marker.txt": "HOME SCAN-FALLBACK MARKER: PASS\n",
    "mcp-probe.txt": "PASS  health answers\nMCP PROBE: PASS\n",
    "mcp-probe.exit": "0\n",
}


def _fixture(tmp, files):
    d = tempfile.mkdtemp(dir=tmp)
    for name, content in files.items():
        with open(os.path.join(d, name), "w", encoding="utf-8") as fh:
            fh.write(content)
    return d


def self_test():
    failures = []

    def expect(label, files, want_pass):
        with tempfile.TemporaryDirectory() as tmp:
            d = _fixture(tmp, files)
            doc, _ = aggregate(d, skip_scan=True)
            got_pass = doc["verdict"] == "PASS"
            if got_pass != want_pass:
                failures.append(
                    f"{label}: verdict {doc['verdict']}, wanted "
                    f"{'PASS' if want_pass else 'FAIL'}"
                )
            else:
                print(f"self-test PASS: {label} -> {doc['verdict']} (as wanted)")

    expect("all-PASS transcripts", dict(_GOOD), True)

    bad = dict(_GOOD)
    bad["mcp-probe.txt"] = "PASS  health answers\nMCP PROBE: FAIL (1 assertion(s) failed)\n"
    expect("an MCP FAIL verdict", bad, False)

    bad = dict(_GOOD)
    bad["home-thumbnail-probe.txt"] = "HOME THUMBNAIL PROBE: UNPROVEN\n"
    expect("an UNPROVEN thumbnail probe", bad, False)

    bad = dict(_GOOD)
    del bad["home-scan-fallback-marker.txt"]
    expect("a missing marker transcript", bad, False)

    bad = dict(_GOOD)
    bad["mcp-probe.txt"] = "FAIL  one buried assertion\nMCP PROBE: PASS\n"
    expect("a FAIL line buried under a PASS verdict", bad, False)

    bad = dict(_GOOD)
    bad["subtitle-census.exit"] = "1\n"
    expect("a non-zero census exit beside a PASS transcript", bad, False)

    if failures:
        for f in failures:
            print(f"self-test FAIL: {f}")
        return 1
    print("self-test PASS: verdict aggregator is not vacuous "
          "(1 pass shape, 5 fail shapes)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
