#!/usr/bin/env python3
"""Print the MCP surface floor from its single source of truth.

The gate's `MCP_SURFACE_FLOOR` constant in
`scripts/phase_j_checks.py` is the one ledgered floor for the
MCP tool surface (see the ratchet ledger above that constant).
Consumers that need the number — above all the live-MCP probe
in `.github/workflows/headless.yml`, which carried a stale
hard-coded `TOOL_FLOOR = 370` while the surface stood at 392
(Roadmap F-10, RM11) — should derive it here instead of copying
it:

    python3 scripts/mcp_tool_floor.py            # prints e.g. 392

CI shape (Roadmap RM8 owns the workflow; see IA-RM11-1 in
`~/workspace/uiux-roadmap/findings/rm11.md`): a step runs this
script, exports the value as `MCP_TOOL_FLOOR`, and the probe
reads the environment instead of the hardcode.

Exit status: 0 with the integer on stdout; 1 (and nothing on
stdout) when the constant is missing or ambiguous — a silent
fallback floor would defeat the ratchet. Stdlib only.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

GATE = Path(__file__).resolve().parent / "phase_j_checks.py"


def main() -> int:
    text = GATE.read_text(encoding="utf-8")
    hits = re.findall(r"^MCP_SURFACE_FLOOR\s*=\s*(\d+)\s*$", text, re.M)
    if len(hits) != 1:
        print(f"mcp_tool_floor: expected exactly one MCP_SURFACE_FLOOR "
              f"assignment in {GATE.name}, found {len(hits)}",
              file=sys.stderr)
        return 1
    print(hits[0])
    return 0


if __name__ == "__main__":
    sys.exit(main())
