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
    python3 scripts/overflow_scan.py --self-test

Prints per-file hits, worst first, as
    (element key, parent card, overflow pt, rules fired)
and exits non-zero if any non-exempt hit is found in any
file. A run is CLEAN when every file reports zero hits.

Rules (tolerance 0.5 pt everywhere). The window width W is
read from the tree itself -- the root element's rect (#0)
is the window -- so trees dumped at any size are judged
against their own window. The column geometry generalises
from the reference calibration (W = 1440):

- The left column is anchored to the left edge and never
  moves: margin 16, width 300, card x-range [16, 316],
  content right edge 304 -- the same at every W.
- The right column is anchored to the right edge: width
  340, margin 16, card x-range [W-356, W-16], content
  right edge W-28 (1412 at W = 1440).
- Only the centre column absorbs a width change, and it
  carries no width budget (rules 1-2 apply inside the side
  columns only); the window rule applies to it, as to
  overlays.

At W = 1440 every derived value equals the original
hard-coded constant, so verdicts on 1440x900 trees are
unchanged (the self-test pins this).

1. Content rule. Every element under /side-left/ must have
   right edge <= 304.5; under /side-right/ <= (W-28) + 0.5 --
   except the card roots themselves (a card is 300/340 wide
   by design; its own right edge may reach 316 / W-16). Card
   root = the path segment directly under the column.
2. Card rule (defence in depth). Every descendant of a
   card root must lie within the card root's own x-range.
   (Rect-derived, so already width-agnostic.)
3. Window rule. No element anywhere may cross x < 0 or
   x > W (overlays included).

Exemptions (closed list -- anything else is a hit):

- Card roots themselves (rule 1 only; see above).
- settings-toggle / settings-card: these two keys ARE the
  settings card (the collapsed strip and the expanded card
  root, nested one level under the `settings` wrapper), so
  their own right edge may reach W-16. Their descendants
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
"""

import json
import sys

TOL = 0.5
DEFAULT_WINDOW_W = 1440.0
# Left column: anchored left, fixed at every window width.
# Card x-range [16, 316]; content right edge 304.
LEFT_CONTENT_EDGE = 304.0
# Right column: anchored right. Card x-range [W-356, W-16];
# the content right edge sits 28 pt inside the window's
# right edge (1440 - 28 = 1412 at the reference width).
RIGHT_CONTENT_INSET = 28.0
COLUMN_SEGMENTS = {"side-left", "side-right"}

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
        if s in COLUMN_SEGMENTS:
            return i
    return None


def window_width(elements):
    """Window width from the tree's root element.

    The root is the element with the shallowest path (#0);
    its rect is the window. Falls back to the reference
    width 1440 when no usable root rect exists, which
    reproduces the pre-generalisation behaviour exactly.
    """
    root = None
    for el in elements:
        segs = segments(el.get("path", ""))
        if segs and (root is None or len(segs) < len(root[0])):
            root = (segs, el)
    if root is not None:
        rect = root[1].get("rect") or [0, 0, 0, 0]
        if rect[2] > 0:
            return float(rect[2])
    return DEFAULT_WINDOW_W


def content_edges(win_w):
    """(left, right) content right-edges for window width W."""
    return LEFT_CONTENT_EDGE, win_w - RIGHT_CONTENT_INSET


def scan_tree(tree):
    elements = tree.get("elements", [])
    win_w = window_width(elements)
    left_edge, right_edge = content_edges(win_w)
    columns = {"side-left": left_edge, "side-right": right_edge}

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
        # container, whose own right edge is 316 / W-16 by design).
        if (
            ci is not None
            and len(segs) > ci + 1
            and not is_card_root
            and key not in CARD_ALIAS_KEYS
        ):
            edge = columns[segs[ci]]
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
        excess = max(right - win_w, -x)
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


def scan_file(path):
    with open(path) as fh:
        tree = json.load(fh)
    return scan_tree(tree)


# ------------------------------------------------------------------
# Self-test: synthetic fixtures proving (a) the generalised
# edges reduce EXACTLY to the original constants at W=1440 and
# (b) a violation that only exists at 1280 is caught at 1280.
# ------------------------------------------------------------------

def _el(path, rect, key=None):
    el = {"path": path, "rect": rect}
    if key is not None:
        el["key"] = key
    return el


def _fixture(win_w, win_h):
    """A minimal two-column tree at window width win_w.

    Contains one conforming right-column chip ending exactly
    at the content edge, one right-column chip ending 18 pt
    past it (and 6 pt past its card), one left-column chip
    6 pt past the fixed left content edge, and one overlay
    crossing the window's right edge by 10 pt.
    """
    r_card_x = win_w - 356.0  # right card x0 (W-16-340)
    r_edge = win_w - 28.0     # right content right edge
    return {
        "elements": [
            _el("#0", [0.0, 0.0, win_w, win_h]),
            _el("#0/main-row", [16.0, 157.0, win_w - 32.0, 100.0]),
            # Left column (fixed geometry at every width).
            _el("#0/main-row/side-left", [16.0, 157.0, 300.0, 100.0]),
            _el("#0/main-row/side-left/card-l", [16.0, 157.0, 300.0, 100.0], "card-l"),
            _el("#0/main-row/side-left/card-l/left-chip",
                [100.0, 170.0, 210.0, 20.0], "left-chip"),  # right = 310
            # Right column (anchored to the right edge).
            _el("#0/main-row/side-right", [r_card_x, 157.0, 340.0, 100.0]),
            _el("#0/main-row/side-right/card-r", [r_card_x, 157.0, 340.0, 100.0], "card-r"),
            _el("#0/main-row/side-right/card-r/chip-ok",
                [r_card_x + 176.0, 170.0, 152.0, 20.0], "chip-ok"),  # right = edge
            _el("#0/main-row/side-right/card-r/chip-wide",
                [r_card_x + 176.0, 200.0, 170.0, 20.0], "chip-wide"),  # right = edge+18
            # Overlay crossing the window edge.
            _el("#0/overlay", [win_w - 80.0, 10.0, 90.0, 20.0], "overlay-wide"),  # right = W+10
        ]
    }


def self_test():
    failures = []

    def check(cond, label):
        print("  %s %s" % ("PASS" if cond else "FAIL", label))
        if not cond:
            failures.append(label)

    # The generalised edges at the reference width are the
    # original constants, bit for bit.
    check(content_edges(1440.0) == (304.0, 1412.0),
          "edges at W=1440 are exactly (304, 1412)")
    check(content_edges(1280.0) == (304.0, 1252.0),
          "edges at W=1280 are (304, 1252): left fixed, right anchored")

    for win_w, win_h in ((1440.0, 900.0), (1280.0, 720.0)):
        hits = scan_tree(_fixture(win_w, win_h))
        by_key = {h["key"]: h for h in hits}
        tag = "W=%d" % win_w
        check("chip-ok" not in by_key,
              "%s: chip ending exactly at the content edge is clean" % tag)
        check("chip-wide" in by_key and "content" in by_key["chip-wide"]["rules"],
              "%s: chip 18 pt past the content edge is caught (content)" % tag)
        check("chip-wide" in by_key and "card" in by_key["chip-wide"]["rules"],
              "%s: chip 6 pt past its card is caught (card)" % tag)
        check(abs(by_key.get("chip-wide", {}).get("overflow", 0) - 18.0) < 1e-9,
              "%s: chip-wide overflow measures 18.0 pt" % tag)
        check("left-chip" in by_key and "content" in by_key["left-chip"]["rules"],
              "%s: left chip past the fixed 304 edge is caught" % tag)
        check("overlay-wide" in by_key and "window" in by_key["overlay-wide"]["rules"],
              "%s: overlay crossing the window edge is caught (window)" % tag)
        check(len(hits) == 3, "%s: exactly 3 hits (got %d)" % (tag, len(hits)))

    # The point of the generalisation: at W=1280 the wide
    # chip ends at x=1270, far inside the OLD fixed window
    # (1440) and old right edge (1412) -- the old constants
    # would have passed this tree. Prove the miss is real
    # by re-running the fixture's chip against the old edge.
    chip_right = 1280.0 - 28.0 + 18.0
    check(chip_right < 1412.0 and chip_right < 1440.0,
          "the 1280 violation (right=1270) was invisible to the old 1440 rules")

    # A rootless tree falls back to the reference width.
    check(window_width([]) == 1440.0, "empty tree falls back to W=1440")

    if failures:
        print("SELF-TEST: %d failure(s)" % len(failures))
        return 1
    print("SELF-TEST: all checks passed")
    return 0


def main(argv):
    if len(argv) < 2:
        print(__doc__)
        return 2
    if argv[1] == "--self-test":
        return self_test()
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
