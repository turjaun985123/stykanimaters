#!/usr/bin/env python3
"""Overflow scan for headless --tree dumps (Phase O item O-2).

Method: docs/phase_o_plans/p05-overflow-widths.md §2, as run
by the S19 audit (docs/ui_mission/audit/s19-visual-qa.md §1).
Rune does not clip, so any element wider than its card
paints past the card (and possibly past the window) and the
excess controls cannot be clicked -- the side columns scroll
vertically only. This scan finds that defect family in the
CI headless tree dumps; it belongs to the headless debrief,
not the source gate (the dumps are CI artifacts).

Usage:
    python3 scripts/overflow_scan.py <tree.json> [tree.json ...]

Prints per-file hits, worst first, as
    (element key, parent card, overflow pt, rules fired)
and exits non-zero if any non-exempt hit is found in any
file. A run is CLEAN when every file reports zero hits.

Rules (tolerance 0.5 pt everywhere; window 1440x900):

1. Content rule. Every element under /side-left/ must have
   right edge <= 304.5; under /side-right/ <= 1412.5 --
   except the card roots themselves (a card is 300/340 wide
   by design; its own right edge may reach 316/1424). Card
   root = the path segment directly under the column.
2. Card rule (defence in depth). Every descendant of a
   card root must lie within the card root's own x-range.
3. Window rule. No element anywhere may cross x < 0 or
   x > 1440 (overlays included).

Exemptions (closed list -- anything else is a hit):

- Card roots themselves (rule 1 only; see above).
- settings-toggle / settings-card: these two keys ARE the
  settings card (the collapsed strip and the expanded card
  root, nested one level under the `settings` wrapper), so
  their own right edge may reach 1424. Their descendants
  are NOT exempt -- the language chips inside
  settings-card are real hits (S19 §1 false-positive note).
- The timeline scroll subtree (any element whose path
  contains a `timeline` segment): it scrolls horizontally
  by design. This is also the scan-time form of the
  general exemption "subtree rooted at an element whose
  builder sets scroll_x": at scan time the timeline is
  the only such subtree; a new one must be added here by
  key with a reason.
- Zero-size elements (w = 0 or h = 0: collapsed canvases,
  spacers).
- Vertical overflow is never flagged: both side columns
  scroll vertically; below-the-fold cards are legitimate.

The centre column / stage subtree carries no width budget
(rules 1-2 only apply inside the side columns); the
window rule still applies to it, as to overlays.
"""

import json
import sys

TOL = 0.5
WINDOW_W = 1440.0
LEFT_CONTENT_EDGE = 304.0    # left column content x 28..304
RIGHT_CONTENT_EDGE = 1412.0  # right column content x 1096..1412
COLUMNS = {"side-left": LEFT_CONTENT_EDGE, "side-right": RIGHT_CONTENT_EDGE}

# Keys that ARE a card despite sitting one level below the
# column segment (see module docstring). Exempt from the
# content and card rules for the element itself only.
CARD_ALIAS_KEYS = {"settings-toggle", "settings-card"}

# Horizontal-scroll subtrees, by path segment (key). Closed
# list; see module docstring.
SCROLL_SEGMENTS = {"timeline"}


def segments(path):
    return [s for s in path.split("/") if s != ""]


def column_index(segs):
    """Index of the side-column segment in segs, or None."""
    for i, s in enumerate(segs):
        if s in COLUMNS:
            return i
    return None


def scan_file(path):
    with open(path) as fh:
        tree = json.load(fh)
    elements = tree.get("elements", [])

    # Card roots: path -> (key, x0, x1). A card root is the
    # element whose path ends at the segment directly under
    # a side column.
    card_roots = {}
    for el in elements:
        segs = segments(el.get("path", ""))
        ci = column_index(segs)
        if ci is not None and len(segs) == ci + 2:
            rect = el.get("rect") or [0, 0, 0, 0]
            card_roots[el["path"]] = (segs[-1], rect[0], rect[0] + rect[2])

    hits = []
    for el in elements:
        epath = el.get("path", "")
        rect = el.get("rect") or [0, 0, 0, 0]
        x, _y, w, h = rect
        right = x + w
        key = el.get("key")
        segs = segments(epath)

        # Exemptions that silence the element entirely.
        if w <= 0 or h <= 0:
            continue  # zero-size
        if any(s in SCROLL_SEGMENTS for s in segs):
            continue  # horizontal-scroll subtree (timeline)

        ci = column_index(segs)
        is_card_root = epath in card_roots
        parent_card = None
        card_range = None
        if ci is not None and len(segs) > ci + 1:
            root_path = "/".join(segs[: ci + 2])
            if root_path in card_roots:
                parent_card, cx0, cx1 = card_roots[root_path]
                card_range = (cx0, cx1)
            else:
                parent_card = segs[ci + 1]

        rules = []
        overflow = 0.0

        # Rule 1 -- content edge (not for card roots / aliases,
        # and not for the column element itself: len == ci + 1
        # means the element IS side-left/side-right, the
        # container, whose own right edge is 316/1424 by design).
        if (
            ci is not None
            and len(segs) > ci + 1
            and not is_card_root
            and key not in CARD_ALIAS_KEYS
        ):
            edge = COLUMNS[segs[ci]]
            if right > edge + TOL:
                rules.append("content")
                overflow = max(overflow, right - edge)

        # Rule 2 -- within the card root's own x-range.
        if card_range is not None and not is_card_root and key not in CARD_ALIAS_KEYS:
            cx0, cx1 = card_range
            excess = max(right - cx1, cx0 - x)
            if excess > TOL:
                rules.append("card")
                overflow = max(overflow, excess)

        # Rule 3 -- inside the window.
        excess = max(right - WINDOW_W, -x)
        if excess > TOL:
            rules.append("window")
            overflow = max(overflow, excess)

        if rules:
            label = key if key else "(unkeyed %s)" % segs[-1]
            hits.append(
                {
                    "key": label,
                    "card": parent_card if parent_card else "-",
                    "overflow": overflow,
                    "rules": "+".join(rules),
                    "right": right,
                }
            )

    hits.sort(key=lambda hit: -hit["overflow"])
    return hits


def main(argv):
    if len(argv) < 2:
        print(__doc__)
        return 2
    total = 0
    for path in argv[1:]:
        hits = scan_file(path)
        total += len(hits)
        print("%s: %d hit(s)" % (path, len(hits)))
        for hit in hits:
            print(
                "  %-28s card=%-18s +%.1f pt  [%s] right=%.1f"
                % (hit["key"], hit["card"], hit["overflow"], hit["rules"], hit["right"])
            )
    if total:
        print("OVERFLOW SCAN: %d hit(s) -- NOT CLEAN" % total)
        return 1
    print("OVERFLOW SCAN: CLEAN (0 hits)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
