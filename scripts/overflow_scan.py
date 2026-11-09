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
4. Timeline-card rule (Wave 10). The timeline card is
   centre-column chrome: an unkeyed card holding one
   horizontal scroll row (keyed `timeline`) plus, under
   it, the non-scrolling loop row. The card is found
   structurally -- it is the scroll row's parent element
   -- because Phase N deliberately left it unkeyed.
   - Every descendant of the card must lie within the
     card's own y-range. The scroll inside the card is
     horizontal only (scroll_x never moves y) and the
     centre column does not scroll vertically, so every
     y in the subtree is a painted y; Rune does not
     clip, so a row past the card's bottom edge paints
     over the status strip. This is the vertical budget
     the dopesheet lane is checked against: the lane
     adds one row per figure, so it is the first
     timeline content whose height grows with the
     project.
   - The card's non-scroll content -- the scroll row
     itself (its rect is the viewport, w_full inside
     the card's padding) and everything under the card
     but outside the scroll row (the loop row) -- must
     also lie within the card's x-range, the card rule
     (rule 2) extended to this one card.
   - The card root itself must fit the window
     vertically (rule 3 is horizontal only): a card
     pushed past the window's bottom edge is the same
     defect one level up.
   Strict descendants of the scroll row (the content
   column, ruler, chips, marker row, clips lane,
   dopesheet lane: headers and cells) get NO horizontal
   judgement: their rects are drawn positions under a
   live horizontal scroll offset, the dump carries no
   offset field this scan could correct by, and content
   wider than the viewport is a working scroll, not an
   overflow. The rule is inert when no `timeline`
   element exists (a zero-frame project: the card then
   holds only the kit empty state, no fixed-geometry
   content to judge).

Wave 12 audit -- the two populated scenes. Wave 12 adds
the first trees whose Home card and stroke inspector
carry live rows (scenes `home-populated` and
`strokes-populated`): project rows with 56 pt
thumbnails, metadata lines and wrapping button rows
under `home-panel`, and per-stroke rows (swatch,
truncated summary, Delete) under `stroke-inspector`.
No rule needed generalising, and none was weakened:

- Both panels are ordinary side-column card roots --
  in the Wave 11 dumps `home-panel` sits at
  #0/main-row/side-left/home-panel ([16, 157, 300, 49],
  the closed card) and `stroke-inspector` at
  #0/main-row/side-right/stroke-inspector
  ([1084, 786.5, 340, 501.13]) -- so rules 1-2 already
  judge every populated descendant horizontally,
  including a row escaping its card (the Wave 9 clips
  failure mode). Neither panel contains a scroll_x
  subtree, so no new exemption was needed.
- No rule assumes a single column, a fixed row
  height, or the empty-Home structure: the checks are
  per-element x-range tests, and populated lists only
  add vertical extent, which the side columns absorb
  by scrolling (the draw-open inspector card already
  ends past y=1287 in a 900 pt window, legitimately).
- The Home rows are a vertical list, not a grid --
  one thumbnail per row, inside the row -- so no
  tiling exception arises; the thumbnail is judged
  like any other element.

The self-test pins both shapes: conforming populated
trees (rows running below the window's bottom edge
included) scan clean, and one planted row escape per
scene is caught by the content and card rules.

Wave 13 audit -- the sprite surface and the saved
library rows. Wave 13 adds two more tree shapes:
sprites-panel rows (squad A's new panel) and the
library panel carrying SAVED entries (scene
`library-populated`, D1). Again no rule needed
generalising, and none was weakened:

- The sprites panel is an ordinary side-column card:
  one row per instance (asset name, key count, Delete)
  plus, for the selected instance, the per-key
  steppers (x, y, rotation, scale). Steppers and
  Delete buttons are fixed-width children ending at
  the content edge -- exactly what rules 1-2 judge.
  The panel has no horizontal-scroll subtree (its
  list scrolls vertically at most), so no exemption
  arises. The fixture's keys and row anatomy follow
  A3's shipped source (`sprites_panel.rs`, which
  landed during this audit); only the panel's mount
  (column, y) is modelled, on the stroke inspector's
  card geometry, because the workspace gating and
  the `sprites-populated` scene are still in flight
  -- when the real tree lands, the debrief scan
  validates the shape against it and the fixture's
  placement is corrected to the dump if they differ.
- The library rows' width anatomy is unchanged by
  the 44 pt thumbnails (Wave 12 D5): the thumb and
  the seg count are the first line's only fixed
  children (83 pt of its 236 pt) and the name
  truncates, so the Wave 10 verdicts stand. What the
  populated scene adds is the saved rows' button line
  (Insert / Rename / Delete) -- the fixed children
  whose 263 pt total caused the Wave 10 escape in
  this very panel. The fixture is modelled on the
  real Wave 12 dump geometry (card [16, 754, 300,
  329], rows 73 pt, thumb 44x44 at row x+8, count
  ending at 296, `lib-sort` and `library-collapse`
  ending exactly at the 304 edge).
- The library list rides a VERTICAL scroll (a
  ScrollSpec at a fixed 200 pt height): scrolled
  rows dump past the list's bottom and the window's,
  legitimately, but scroll_y moves no x, so rules
  1-2 judge every row exactly as they judge the
  un-scrolled Home list. SCROLL_SEGMENTS stays a
  horizontal-only closed list -- exempting the
  library list would blind the scan in the one panel
  where fixed row children have already escaped once.

The self-test pins both shapes the same way:
conforming trees (below-the-fold rows included) scan
clean, and one planted escape per shape -- a stepper
button, a saved row's Delete -- is caught by the
content and card rules.

Checks considered and NOT added (they would
false-positive or judge what rects cannot show):

- Horizontal bounds on scrolled content (chips, clip
  spans, dope cells/headers): see rule 4 -- the right
  edge of scrolled content is scroll-dependent by
  definition, and so is the left edge once the
  viewport has scrolled.
- Cell pitch / alignment (a dope cell's x against
  frame x CHIP_PITCH): the pitch is a source constant
  (timeline_marks.rs); duplicating it here would
  create a second source of truth that can drift, and
  misalignment is an alignment defect, not a
  paint-past-bounds overflow -- the family this scan
  exists for. The in-process snap tests pin cell
  behaviour instead.
- Dope header width against its row slot: the slot
  width is not an element in the dump, so there is no
  rect to judge the header against; a too-wide header
  inside the scroll still paints within the content
  extent, and its failure mode is overlap, not
  overflow.
- A vertical window rule for all elements: the side
  columns scroll vertically, and below-the-fold
  content there legitimately draws past the window's
  bottom edge. Rule 4's vertical clause is sound only
  because the timeline card sits in no vertical
  scroll container.

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
  by design, so it is exempt from rules 1-3. Rule 4's
  vertical clause still applies inside it (see above).
  This is also the scan-time form of the general
  exemption "subtree rooted at an element whose
  builder sets scroll_x": at scan time the timeline is
  the only such subtree; a new one must be added here
  by key with a reason.
- Zero-size elements (w = 0 or h = 0: collapsed canvases,
  spacers).
- Vertical overflow in the side columns is never
  flagged: both side columns scroll vertically, so
  below-the-fold cards are legitimate. The one
  vertical judgement in the scan is rule 4's, inside
  the timeline card (see above).
"""

import json
import sys

TOL = 0.5
DEFAULT_WINDOW_W = 1440.0
DEFAULT_WINDOW_H = 900.0
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


def window_size(elements):
    """(width, height) of the window, from the tree root.

    The root is the element with the shallowest path (#0);
    its rect is the window. Each dimension falls back to
    the reference window (1440 x 900) when the root rect
    does not provide it, which reproduces the
    pre-generalisation width behaviour exactly.
    """
    root = None
    for el in elements:
        segs = segments(el.get("path", ""))
        if segs and (root is None or len(segs) < len(root[0])):
            root = (segs, el)
    win_w, win_h = DEFAULT_WINDOW_W, DEFAULT_WINDOW_H
    if root is not None:
        rect = root[1].get("rect") or [0, 0, 0, 0]
        if rect[2] > 0:
            win_w = float(rect[2])
        if len(rect) > 3 and rect[3] > 0:
            win_h = float(rect[3])
    return win_w, win_h


def window_width(elements):
    """Window width from the tree's root element."""
    return window_size(elements)[0]


def content_edges(win_w):
    """(left, right) content right-edges for window width W."""
    return LEFT_CONTENT_EDGE, win_w - RIGHT_CONTENT_INSET


def scan_tree(tree):
    elements = tree.get("elements", [])
    win_w, win_h = window_size(elements)
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

    # Timeline card (rule 4): the element keyed `timeline`
    # is the card's horizontal scroll row; the card itself
    # is deliberately unkeyed (Phase N), so the card root
    # is found structurally, as the scroll row's parent
    # element. Rect-derived, so width-agnostic like rule
    # 2. Absent on a zero-frame project (the card then
    # holds only the kit empty state) -- rule 4 stays
    # inert there.
    tl_row_path = None
    tl_card = None  # (path, x, y, right, bottom)
    for el in elements:
        if el.get("key") == "timeline":
            tl_row_path = el.get("path", "")
            break
    if tl_row_path and "/" in tl_row_path:
        card_path = tl_row_path.rsplit("/", 1)[0]
        for el in elements:
            if el.get("path") == card_path:
                rect = el.get("rect") or [0, 0, 0, 0]
                if rect[2] > 0 and rect[3] > 0:
                    tl_card = (
                        card_path,
                        rect[0],
                        rect[1],
                        rect[0] + rect[2],
                        rect[1] + rect[3],
                    )
                break

    hits = []
    for el in elements:
        epath = el.get("path", "")
        rect = el.get("rect") or [0, 0, 0, 0]
        x, y, w, h = rect
        right = x + w
        bottom = y + h
        key = el.get("key")
        segs = segments(epath)

        # Zero-size elements are silenced entirely
        # (collapsed canvases, spacers).
        if w <= 0 or h <= 0:
            continue  # zero-size
        # The horizontal-scroll subtree is exempt from
        # rules 1-3; rule 4's vertical clause still
        # applies inside it.
        in_scroll = any(s in SCROLL_SEGMENTS for s in segs)

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

        if not in_scroll:
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

        # Rule 4 -- timeline card budget (see module
        # docstring). Applies to the card root and every
        # descendant; strict descendants of the scroll row
        # are judged vertically only.
        if tl_card is not None:
            cpath, cx, cy, cright, cbot = tl_card
            if epath == cpath:
                # The card root itself: vertical fit in
                # the window (rule 3 covers its horizontal
                # fit -- the card is not in the scroll
                # subtree).
                excess = max(-y, bottom - win_h)
                if excess > TOL:
                    rules.append("timeline-card")
                    overflow = max(overflow, excess)
            elif epath.startswith(cpath + "/"):
                excess = max(cy - y, bottom - cbot)
                if not epath.startswith(tl_row_path + "/"):
                    excess = max(excess, cx - x, right - cright)
                if excess > TOL:
                    rules.append("timeline-card")
                    overflow = max(overflow, excess)
                    if parent_card is None:
                        parent_card = "timeline-card"

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
# edges reduce EXACTLY to the original constants at W=1440,
# (b) a violation that only exists at 1280 is caught at 1280,
# (c) the timeline-card rule (rule 4) stays silent on a
# conforming card -- including scrolled content far past the
# window -- and catches each of its violation classes, and
# (d) the Wave 12 populated-scene fixtures (home-populated,
# strokes-populated) stay silent on conforming trees -- rows
# below the window's bottom edge included -- and catch one
# planted row escape per scene, and (e) the Wave 13 fixtures
# (sprites-panel rows, library rows with 44 pt thumbnails
# and saved-row button lines) do the same for their shapes.
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


def _timeline_fixture(win_w, win_h, loop_dx=0.0, dope_dy=0.0,
                        timeline_dw=0.0, card_dy=0.0):
    """A minimal centre-column tree: stage + timeline card.

    Card geometry follows the real layout's anchors: the
    centre column starts at x=328 (16 margin + 300 left
    column + 12 gap) and its width ends 368 pt before the
    window's right edge (12 gap + 340 right column + 16
    margin), so card width = W-696 at any W; the card sits
    near the window's bottom. Inside the scroll row, a
    50-frame content column (1700 pt wide on the 34 pt
    frame pitch) with a far chip and a far dopesheet cell
    -- all far past the window's right edge, all
    legitimate scrolled content. The deltas plant one
    rule-4 violation each: loop_dx pushes the loop row's
    Clear button past the card's right edge, dope_dy drops
    a dopesheet row past the card's bottom, timeline_dw
    widens the scroll viewport itself past the card, and
    card_dy pushes the whole card past the window's
    bottom edge.
    """
    card_x = 328.0
    card_w = win_w - 696.0
    card_y = win_h - 201.0 + card_dy
    inner_x = card_x + 8.0
    inner_w = card_w - 16.0
    tl_y = card_y + 8.0
    lane_y = tl_y + 52.0
    base = "#0/main-row/centre/#3"
    return {
        "elements": [
            _el("#0", [0.0, 0.0, win_w, win_h]),
            _el("#0/main-row", [16.0, 141.0, win_w - 32.0, win_h - 157.0]),
            _el("#0/main-row/centre",
                [card_x, 141.0, card_w, win_h - 157.0], "centre"),
            _el("#0/main-row/centre/stage", [card_x, 157.0, card_w, 100.0], "stage"),
            # The (unkeyed) timeline card and its scroll row.
            _el(base, [card_x, card_y, card_w, 121.0]),
            _el(base + "/timeline",
                [inner_x, tl_y, inner_w + timeline_dw, 74.0], "timeline"),
            # Scrolled content: ruler/chips/lanes stack.
            _el(base + "/timeline/#0", [inner_x, tl_y, 1700.0, 74.0]),
            _el(base + "/timeline/#0/#1/frame-50",
                [inner_x + 49.0 * 34.0, tl_y + 26.0, 30.0, 24.0], "frame-50"),
            _el(base + "/timeline/#0/dopesheet-lane",
                [inner_x, lane_y, 1700.0, 18.0], "dopesheet-lane"),
            _el(base + "/timeline/#0/dopesheet-lane/dope-row-0",
                [inner_x, lane_y + dope_dy, 120.0, 18.0], "dope-row-0"),
            _el(base + "/timeline/#0/dopesheet-lane/dope-cell-0-49",
                [inner_x + 49.0 * 34.0, lane_y, 30.0, 18.0], "dope-cell-0-49"),
            # Non-scroll card content: the loop row.
            _el(base + "/timeline-loop-row",
                [inner_x, card_y + 86.0, inner_w, 27.0], "timeline-loop-row"),
            _el(base + "/timeline-loop-row/loop-clear",
                [inner_x + inner_w - 60.0 + loop_dx, card_y + 89.0, 60.0, 20.0],
                "loop-clear"),
        ]
    }


def _home_populated_fixture(win_w, win_h, meta_dx=0.0):
    """A minimal populated-Home tree (Wave 12 scene
    `home-populated`): the `home-panel` card root in the
    left column, open, holding the inner `home-card`
    column with two project rows and the totals footer.

    Geometry follows the real layout (the Wave 11 `home`
    dump + home_panel.rs): card x=16, width 300, with
    12 pt padding, so content is x=28, width 276, right
    edge 304; each row is a `home-row-{i}` list row
    (w_full) leading with the 56 pt thumbnail (16:9
    stage aspect, so 31.5 pt tall) beside a text column
    (truncated name, wrapped `home-proj-meta-{i}` line,
    wrapping button row). Row 1 and the totals footer
    sit below the window's bottom edge -- legitimate,
    because the left column scrolls vertically; the
    scan must stay silent on them. meta_dx shifts row
    1's metadata line right: the planted escape (past
    the 304 content edge and the 316 card edge).
    """
    base = "#0/main-row/side-left/home-panel"
    return {
        "elements": [
            _el("#0", [0.0, 0.0, win_w, win_h]),
            _el("#0/main-row", [16.0, 141.0, win_w - 32.0, win_h - 157.0]),
            _el("#0/main-row/side-left", [16.0, 157.0, 300.0, 1000.0]),
            _el(base, [16.0, 157.0, 300.0, 900.0], "home-panel"),
            _el(base + "/home", [28.0, 169.0, 276.0, 25.0], "home"),
            _el(base + "/home-card", [28.0, 202.0, 276.0, 843.0], "home-card"),
            # Row 0: thumbnail + text column, all inside
            # the content edge.
            _el(base + "/home-card/home-row-0",
                [28.0, 210.0, 276.0, 66.0], "home-row-0"),
            _el(base + "/home-card/home-row-0/#0",
                [36.0, 218.0, 56.0, 31.5]),  # the thumbnail
            _el(base + "/home-card/home-row-0/#1/home-proj-meta-0",
                [100.0, 226.0, 196.0, 15.0], "home-proj-meta-0"),
            _el(base + "/home-card/home-row-0/#1/home-open-0",
                [100.0, 245.0, 86.0, 25.0], "home-open-0"),
            # Row 1: below the window's bottom edge (the
            # column scrolls); its metadata line is the
            # planted escape when meta_dx > 0.
            _el(base + "/home-card/home-row-1",
                [28.0, 940.0, 276.0, 66.0], "home-row-1"),
            _el(base + "/home-card/home-row-1/#0",
                [36.0, 948.0, 56.0, 31.5]),  # the thumbnail
            _el(base + "/home-card/home-row-1/#1/home-proj-meta-1",
                [100.0 + meta_dx, 956.0, 196.0, 15.0], "home-proj-meta-1"),
            _el(base + "/home-card/home-totals",
                [28.0, 1014.0, 276.0, 15.0], "home-totals"),
        ]
    }


def _strokes_populated_fixture(win_w, win_h, del_dx=0.0):
    """A minimal populated stroke-inspector tree (Wave 12
    scene `strokes-populated`): the `stroke-inspector`
    card root in the right column with two per-stroke
    rows on the current frame.

    Geometry follows the real layout (the Wave 11
    draw-open dump + stroke_inspector.rs): card
    x=W-356, width 340, with 12 pt padding, so content
    is x=W-344, width 316, right edge W-28; each row is
    an unkeyed row shell holding a 14 pt swatch, the
    truncated summary text and the row's Delete button
    (`stroke-del-{idx}`), which ends exactly at the
    content edge. The card and its second row run
    below the window's bottom edge -- legitimate,
    because the right column scrolls vertically (the
    real draw-open card ends past y=1287 in a 900 pt
    window). del_dx shifts row 0's Delete button
    right: the planted escape, the Wave 9 clips-row
    failure mode in this card.
    """
    card_x = win_w - 356.0
    cx = card_x + 12.0   # content x (W-344)
    edge = win_w - 28.0  # content right edge
    base = "#0/main-row/side-right/stroke-inspector"
    return {
        "elements": [
            _el("#0", [0.0, 0.0, win_w, win_h]),
            _el("#0/main-row", [16.0, 141.0, win_w - 32.0, win_h - 157.0]),
            _el("#0/main-row/side-right", [card_x, 157.0, 340.0, 1100.0]),
            _el(base, [card_x, 157.0, 340.0, 1050.0], "stroke-inspector"),
            _el(base + "/strokes-header",
                [cx, 600.0, 316.0, 25.0], "strokes-header"),
            # Row 0 (unkeyed shell, as in the panel).
            _el(base + "/#7", [cx, 633.0, 316.0, 33.0]),
            _el(base + "/#7/#0", [cx + 8.0, 642.0, 14.0, 14.0]),   # swatch
            _el(base + "/#7/#1", [cx + 30.0, 642.0, 180.0, 15.0]),  # summary
            _el(base + "/#7/stroke-del-0",
                [edge - 64.0 + del_dx, 637.0, 64.0, 25.0], "stroke-del-0"),
            # Row 1: below the window's bottom edge.
            _el(base + "/#8", [cx, 1000.0, 316.0, 33.0]),
            _el(base + "/#8/stroke-del-1",
                [edge - 64.0, 1004.0, 64.0, 25.0], "stroke-del-1"),
        ]
    }


def _sprites_populated_fixture(win_w, win_h, step_dx=0.0):
    """A minimal sprites-panel tree (Wave 13 scene
    `sprites-populated`): the `sprite-panel` card root
    in the right column holding the instance list, the
    add picker and the selected instance's edit rows.

    Keys and anatomy follow A3's shipped source
    (`sprites_panel.rs`, which landed during this
    audit): `sprite-list` rows keyed `sprite-row-{i}`
    (a `sprite-select-{i}` button carrying the asset
    name, an unkeyed key-count text, a `sprite-del-{i}`
    Delete), an add column `sprite-add` with per-asset
    `sprite-add-{a}` buttons, and the `sprite-edit`
    column's stepper rows (`sprite-rot-dn` /
    `sprite-rot-up`, `sprite-scale-dn` /
    `sprite-scale-up`, ...). The panel's mount -- which
    column, which y -- is still in flight with E8/D1,
    so placement is modelled on the stroke inspector's
    card geometry: card x=W-356, width 340, 12 pt
    padding, content x=W-344, width 316, right edge
    W-28. Each row's Delete and each stepper's up
    button end exactly at the content edge. Row 2
    sits below the window's bottom edge -- legitimate,
    the right column scrolls vertically. step_dx
    shifts the scale stepper's up button right: the
    planted escape (a fixed-width stepper child pushed
    past the card, the populated-row failure mode in
    stepper form).
    """
    card_x = win_w - 356.0
    cx = card_x + 12.0   # content x (W-344)
    edge = win_w - 28.0  # content right edge
    base = "#0/main-row/side-right/sprite-panel"
    return {
        "elements": [
            _el("#0", [0.0, 0.0, win_w, win_h]),
            _el("#0/main-row", [16.0, 141.0, win_w - 32.0, win_h - 157.0]),
            _el("#0/main-row/side-right", [card_x, 157.0, 340.0, 1100.0]),
            _el(base, [card_x, 300.0, 340.0, 460.0], "sprite-panel"),
            # Add picker: one per-asset button.
            _el(base + "/sprite-add",
                [cx, 312.0, 316.0, 25.0], "sprite-add"),
            _el(base + "/sprite-add/sprite-add-0",
                [cx, 312.0, 132.0, 25.0], "sprite-add-0"),
            # Row 0: select button (asset name), key
            # count, Delete at the edge.
            _el(base + "/sprite-list/sprite-row-0",
                [cx, 345.0, 316.0, 33.0], "sprite-row-0"),
            _el(base + "/sprite-list/sprite-row-0/sprite-select-0",
                [cx + 8.0, 349.0, 150.0, 25.0], "sprite-select-0"),
            _el(base + "/sprite-list/sprite-row-0/#1",
                [cx + 166.0, 354.0, 52.0, 15.0]),  # "3 keys"
            _el(base + "/sprite-list/sprite-row-0/sprite-del-0",
                [edge - 64.0, 349.0, 64.0, 25.0], "sprite-del-0"),
            # Row 1 (selected) and its edit rows:
            # rotation conforming, scale carrying the
            # planted escape when step_dx > 0.
            _el(base + "/sprite-list/sprite-row-1",
                [cx, 386.0, 316.0, 33.0], "sprite-row-1"),
            _el(base + "/sprite-list/sprite-row-1/sprite-select-1",
                [cx + 8.0, 390.0, 150.0, 25.0], "sprite-select-1"),
            _el(base + "/sprite-list/sprite-row-1/sprite-del-1",
                [edge - 64.0, 390.0, 64.0, 25.0], "sprite-del-1"),
            _el(base + "/sprite-edit/sprite-rot-row",
                [cx, 427.0, 316.0, 25.0], "sprite-rot-row"),
            _el(base + "/sprite-edit/sprite-rot-row/sprite-rot-dn",
                [edge - 56.0, 427.0, 26.0, 25.0], "sprite-rot-dn"),
            _el(base + "/sprite-edit/sprite-rot-row/sprite-rot-up",
                [edge - 26.0, 427.0, 26.0, 25.0], "sprite-rot-up"),
            _el(base + "/sprite-edit/sprite-scale-row",
                [cx, 460.0, 316.0, 25.0], "sprite-scale-row"),
            _el(base + "/sprite-edit/sprite-scale-row/sprite-scale-dn",
                [edge - 56.0, 460.0, 26.0, 25.0], "sprite-scale-dn"),
            _el(base + "/sprite-edit/sprite-scale-row/sprite-scale-up",
                [edge - 26.0 + step_dx, 460.0, 26.0, 25.0], "sprite-scale-up"),
            # Row 2: below the window's bottom edge.
            _el(base + "/sprite-list/sprite-row-2",
                [cx, 950.0, 316.0, 33.0], "sprite-row-2"),
            _el(base + "/sprite-list/sprite-row-2/sprite-del-2",
                [edge - 64.0, 954.0, 64.0, 25.0], "sprite-del-2"),
        ]
    }


def _library_populated_fixture(win_w, win_h, del_dx=0.0):
    """A minimal populated-library tree (Wave 13 scene
    `library-populated`): the `library-panel` card root
    in the left column with a built-in row and a saved
    row inside the vertically scrolled `library-list`.

    Geometry is the real Wave 12 dump's (tree-default):
    card [16, 754, 300, 329], content x=28, width 276,
    right edge 304; `library-collapse` and `lib-sort`
    end exactly at 304; each row is 73 pt tall (a 44 pt
    thumbnail line over a 25 pt button line), the
    `library-thumb-{i}` canvas 44x44 at row x+8, the
    seg count ending at 296 (the row body's right
    edge). The saved row -- the entry kind the Wave 12
    dumps never carried -- adds Rename and Delete to
    the button line. Rows run below the list's 200 pt
    viewport and the window's bottom edge: legitimate
    (vertical scroll moves no x). del_dx shifts the
    saved row's Delete button right, modelling the
    fixed button line outgrowing its budget -- the
    Wave 10 escape returning to its home panel.
    """
    base = "#0/main-row/side-left/library-panel"
    row0 = base + "/library-card/library-list/library-builtins/library-row-0"
    saved = base + "/library-card/library-list/library-saved/library-row-3"
    return {
        "elements": [
            _el("#0", [0.0, 0.0, win_w, win_h]),
            _el("#0/main-row", [16.0, 141.0, win_w - 32.0, win_h - 157.0]),
            _el("#0/main-row/side-left", [16.0, 157.0, 300.0, 1000.0]),
            _el(base, [16.0, 754.0, 300.0, 329.0], "library-panel"),
            _el(base + "/library-header",
                [28.0, 766.0, 276.0, 25.0], "library-header"),
            _el(base + "/library-header/library-collapse",
                [257.5, 766.0, 46.5, 25.0], "library-collapse"),  # right = 304
            _el(base + "/library-card",
                [28.0, 808.0, 276.0, 263.0], "library-card"),
            _el(base + "/library-card/library-save",
                [28.0, 808.0, 276.0, 25.0], "library-save"),
            _el(base + "/library-card/library-search-row",
                [28.0, 839.0, 276.0, 26.0], "library-search-row"),
            _el(base + "/library-card/library-search-row/library-search",
                [49.0, 839.0, 141.0, 26.0], "library-search"),
            _el(base + "/library-card/library-search-row/lib-sort",
                [198.0, 840.0, 106.0, 25.0], "lib-sort"),  # right = 304
            _el(base + "/library-card/library-list",
                [28.0, 871.0, 276.0, 200.0], "library-list"),
            _el(base + "/library-card/library-list/library-builtins",
                [28.0, 871.0, 276.0, 245.0], "library-builtins"),
            # Built-in row 0: thumbnail line + Insert.
            _el(row0, [28.0, 890.0, 276.0, 73.0], "library-row-0"),
            _el(row0 + "/#0/#0/library-thumb-0",
                [36.0, 890.0, 44.0, 44.0], "library-thumb-0"),
            _el(row0 + "/#0/#0/#1/#0",
                [88.0, 904.0, 165.0, 15.0]),   # entry name
            _el(row0 + "/#0/#0/#1/#1",
                [261.5, 904.0, 34.5, 15.0]),   # "N seg", right = 296
            _el(row0 + "/#0/#1/library-insert-0",
                [36.0, 938.0, 70.0, 25.0], "library-insert-0"),
            # Saved row, below the fold: the full
            # Insert / Rename / Delete button line.
            _el(saved, [28.0, 1141.0, 276.0, 73.0], "library-row-3"),
            _el(saved + "/#0/#0/library-thumb-3",
                [36.0, 1141.0, 44.0, 44.0], "library-thumb-3"),
            _el(saved + "/#0/#0/#1/#1",
                [261.5, 1155.0, 34.5, 15.0]),   # "N seg", right = 296
            _el(saved + "/#0/#1/library-insert-3",
                [36.0, 1189.0, 70.0, 25.0], "library-insert-3"),
            _el(saved + "/#0/#1/library-rename-3",
                [114.0, 1189.0, 86.0, 25.0], "library-rename-3"),
            _el(saved + "/#0/#1/library-delete-3",
                [208.0 + del_dx, 1189.0, 62.0, 25.0], "library-delete-3"),
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
    check(window_size([]) == (1440.0, 900.0),
          "empty tree falls back to the 1440x900 reference window")

    # Rule 4: a conforming timeline card is silent at both
    # window sizes -- including its scrolled content, whose
    # right edges (past x=2000) no rule may judge.
    for win_w, win_h in ((1440.0, 900.0), (1280.0, 720.0)):
        hits = scan_tree(_timeline_fixture(win_w, win_h))
        by_key = {h["key"]: h for h in hits}
        tag = "timeline W=%d" % win_w
        check(len(hits) == 0,
              "%s: conforming card is clean (got %d hit(s))" % (tag, len(hits)))
        check("frame-50" not in by_key,
              "%s: a scrolled chip past the window edge is not flagged" % tag)
        check("dope-cell-0-49" not in by_key,
              "%s: a scrolled dope cell past the window edge is not flagged" % tag)

    # Rule 4 violations, one planted defect per run.
    hits = scan_tree(_timeline_fixture(1440.0, 900.0, loop_dx=18.0))
    by_key = {h["key"]: h for h in hits}
    check(len(hits) == 1 and "loop-clear" in by_key,
          "loop row button 10 pt past the card's right edge is caught")
    check("timeline-card" in by_key.get("loop-clear", {}).get("rules", ""),
          "loop row hit names the timeline-card rule")
    check(abs(by_key.get("loop-clear", {}).get("overflow", 0) - 10.0) < 1e-9,
          "loop row overflow measures 10.0 pt")
    check(by_key.get("loop-clear", {}).get("card") == "timeline-card",
          "loop row hit is attributed to the timeline card")

    hits = scan_tree(_timeline_fixture(1280.0, 720.0, loop_dx=18.0))
    by_key = {h["key"]: h for h in hits}
    check(len(hits) == 1
          and abs(by_key.get("loop-clear", {}).get("overflow", 0) - 10.0) < 1e-9,
          "the same loop row violation is caught at W=1280 (card is rect-derived)")

    hits = scan_tree(_timeline_fixture(1440.0, 900.0, dope_dy=52.0))
    by_key = {h["key"]: h for h in hits}
    check(len(hits) == 1 and "dope-row-0" in by_key,
          "dopesheet row 9 pt past the card's bottom edge is caught (vertical clause)")
    check("timeline-card" in by_key.get("dope-row-0", {}).get("rules", "")
          and abs(by_key.get("dope-row-0", {}).get("overflow", 0) - 9.0) < 1e-9,
          "dopesheet row overflow measures 9.0 pt")

    hits = scan_tree(_timeline_fixture(1440.0, 900.0, timeline_dw=24.0))
    by_key = {h["key"]: h for h in hits}
    check(len(hits) == 1 and "timeline" in by_key,
          "scroll viewport itself 16 pt wider than its card is caught")
    check(abs(by_key.get("timeline", {}).get("overflow", 0) - 16.0) < 1e-9,
          "scroll viewport overflow measures 16.0 pt")

    hits = scan_tree(_timeline_fixture(1440.0, 900.0, card_dy=100.0))
    by_key = {h["key"]: h for h in hits}
    check(len(hits) == 1 and "(unkeyed #3)" in by_key,
          "card root pushed 20 pt past the window's bottom edge is caught")
    check("timeline-card" in by_key.get("(unkeyed #3)", {}).get("rules", "")
          and abs(by_key.get("(unkeyed #3)", {}).get("overflow", 0) - 20.0) < 1e-9,
          "card root overflow measures 20.0 pt")

    # Wave 12: the populated scenes. Conforming trees
    # are silent at both window sizes -- including rows
    # below the window's bottom edge (the side columns
    # scroll vertically) and a Delete button ending
    # exactly at the content edge.
    for win_w, win_h in ((1440.0, 900.0), (1280.0, 720.0)):
        hits = scan_tree(_home_populated_fixture(win_w, win_h))
        check(len(hits) == 0,
              "home-populated W=%d: conforming populated Home is clean (got %d hit(s))"
              % (win_w, len(hits)))
        hits = scan_tree(_strokes_populated_fixture(win_w, win_h))
        check(len(hits) == 0,
              "strokes-populated W=%d: conforming populated inspector is clean (got %d hit(s))"
              % (win_w, len(hits)))

    # One planted row escape per scene: caught by the
    # content and card rules, with the measured overflow.
    hits = scan_tree(_home_populated_fixture(1440.0, 900.0, meta_dx=40.0))
    by_key = {h["key"]: h for h in hits}
    check(len(hits) == 1 and "home-proj-meta-1" in by_key,
          "home-populated: metadata line pushed past the card is caught")
    check("content" in by_key.get("home-proj-meta-1", {}).get("rules", "")
          and "card" in by_key.get("home-proj-meta-1", {}).get("rules", ""),
          "home-populated: the escape names the content and card rules")
    check(abs(by_key.get("home-proj-meta-1", {}).get("overflow", 0) - 32.0) < 1e-9,
          "home-populated: overflow measures 32.0 pt")

    hits = scan_tree(_strokes_populated_fixture(1440.0, 900.0, del_dx=18.0))
    by_key = {h["key"]: h for h in hits}
    check(len(hits) == 1 and "stroke-del-0" in by_key,
          "strokes-populated: Delete button pushed past the card is caught")
    check("content" in by_key.get("stroke-del-0", {}).get("rules", "")
          and "card" in by_key.get("stroke-del-0", {}).get("rules", ""),
          "strokes-populated: the escape names the content and card rules")
    check(abs(by_key.get("stroke-del-0", {}).get("overflow", 0) - 18.0) < 1e-9,
          "strokes-populated: overflow measures 18.0 pt")

    # Wave 13: the sprite surface and the saved library
    # rows. Conforming trees are silent at both window
    # sizes -- including the below-the-fold rows and the
    # edge-exact children (Delete and stepper buttons at
    # the content edge, lib-sort and the seg counts
    # inside theirs).
    for win_w, win_h in ((1440.0, 900.0), (1280.0, 720.0)):
        hits = scan_tree(_sprites_populated_fixture(win_w, win_h))
        check(len(hits) == 0,
              "sprites-populated W=%d: conforming sprites panel is clean (got %d hit(s))"
              % (win_w, len(hits)))
        hits = scan_tree(_library_populated_fixture(win_w, win_h))
        check(len(hits) == 0,
              "library-populated W=%d: conforming populated library is clean (got %d hit(s))"
              % (win_w, len(hits)))

    # One planted escape per shape: caught by the
    # content and card rules, with the measured overflow.
    hits = scan_tree(_sprites_populated_fixture(1440.0, 900.0, step_dx=24.0))
    by_key = {h["key"]: h for h in hits}
    check(len(hits) == 1 and "sprite-scale-up" in by_key,
          "sprites-populated: stepper up button pushed past the card is caught")
    check("content" in by_key.get("sprite-scale-up", {}).get("rules", "")
          and "card" in by_key.get("sprite-scale-up", {}).get("rules", ""),
          "sprites-populated: the escape names the content and card rules")
    check(abs(by_key.get("sprite-scale-up", {}).get("overflow", 0) - 24.0) < 1e-9,
          "sprites-populated: overflow measures 24.0 pt")

    hits = scan_tree(_library_populated_fixture(1440.0, 900.0, del_dx=50.0))
    by_key = {h["key"]: h for h in hits}
    check(len(hits) == 1 and "library-delete-3" in by_key,
          "library-populated: saved row's Delete pushed past the card is caught")
    check("content" in by_key.get("library-delete-3", {}).get("rules", "")
          and "card" in by_key.get("library-delete-3", {}).get("rules", ""),
          "library-populated: the escape names the content and card rules")
    check(abs(by_key.get("library-delete-3", {}).get("overflow", 0) - 16.0) < 1e-9,
          "library-populated: overflow measures 16.0 pt")

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
