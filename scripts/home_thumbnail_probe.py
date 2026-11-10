#!/usr/bin/env python3
"""Home thumbnail pixel probe for the headless snap (Wave Q, Q16).

The Home project rows have carried scan-time frame-0 thumbnails
since UI-mission Wave 4 (`HomePanelState::fill_thumbnails` ->
`render_thumbnail` -> `thumb_el` in src/rune_sec/home_panel.rs),
and the headless workflow has snapped the populated Home library
since Wave 12 (`--scene home-populated` ->
headless-out/snap-home-populated.png). What never existed is a
CHECK of those pixels: the snap step is fail-soft and only
uploads the PNG, so a thumbnail regression (rows rendering
text-only, images decoding to nothing, the fill never running
in snap mode) would be invisible. This probe is that check.

Method (the Wave L census discipline: locate the band from the
frame's own geometry, size the floors to the measured signal):

  * Row rects come from the SAME run's tree dump
    (tree-home-populated.json): elements keyed `home-row-{i}`,
    plus `home-totals` as the negative control. Nothing about
    the panel's layout is hard-coded except the row anatomy the
    source itself fixes: `design::list_row` pads its content by
    8 pt (`row_base`, src/rune_sec/design.rs) and the thumbnail
    leads the row at THUMB_W_PT = 56 pt wide, vertically
    centred (home_panel.rs `thumb_el`).
  * The probe strip for a row is therefore x in
    [row.x + 8, row.x + 8 + 56], y spanning the row inset by
    2 pt top/bottom. Tree points are scaled to PNG pixels by
    (png size / tree root rect size), so a 2x snap is judged
    in its own pixels.
  * The seeded projects are demo-derived (scenes.rs
    `scene_home_populated` saves the running demo project
    three times): stage 960x720, stage background 0x232830,
    figure colour 0xE8E8EC. A rendered thumbnail inside the
    strip must therefore show (a) the STAGE BACKGROUND colour
    over a large share of the strip -- the thumbnail is 42 pt
    tall (56 * 720/960) against a taller row, so even a
    perfect thumbnail covers only part of the strip; floor
    18% against a ~33% measured expectation -- and (b) INK:
    bright stroke pixels (brightest channel >= 100) from
    the figure's strokes, which at thumbnail scale are a
    thin minority; floor 15 px against a ~59+ measured
    expectation (see the Wave W note below). Text elsewhere
    in the row cannot fake either signal: the strip covers
    only the thumbnail's slot (the row's text starts at
    x = 100 pt, past the slot's 92 pt end -- the same run's
    tree dump shows it).
  * Negative control: the same strip over the `home-totals`
    footer (text only, never a thumbnail) must show almost
    no stage background (<= 6%). If the control trips, the
    probe's colour assumption is wrong for this theme and
    the verdict is UNPROVEN, not PASS -- a probe that cannot
    distinguish its control proves nothing.

Wave W (W7) re-derivation -- the Q16 colour premises were
fixture-specific and never held on a real snap. Every real
transcript on record (headless runs 42764348922,
42765217055, 42767540386) shows the identical control
trip, stage-bg share 0.672 against the 0.06 ceiling: the
probe had never issued a verdict on a real snap. Measured
on run 42767540386's artifacts, the render is right and
the premises were wrong, in two places:

  * The strip's non-thumbnail flat colour is the app
    theme's CARD background, #1D222B (29, 34, 43) --
    app_theme.rs pins `t.card.hex() == "#1d222bff"`, and
    the totals text sits on it (home_panel.rs renders
    `home-totals` as plain text on the card). Q16's
    fixture invented a panel colour (24, 26, 31) that
    sits (11, 14, 17) from the stage background; the real
    card sits (6, 6, 5) from it -- INSIDE the old +-12
    tolerance box, so the control strip's card area was
    counted as stage background. BG_TOL is now 2: less
    than half the 5-channel minimum distance to the
    nearest other flat colour the source fixes, so every
    pixel is classified by its nearest source colour and
    card/stage blends (midpoint distance 3) count as
    neither. Measured at tolerance 2 on the same
    artifacts: rows 0.333 / 0.337 / 0.339 stage-bg share
    (floor 0.18), control 0.008 (ceiling 0.06).
  * The ink predicate (every channel >= 190) measures
    ZERO on every real row: `render_thumbnail` shrinks
    the x2 supersampled frame to 112 px wide with a
    Triangle filter (home_panel.rs), so the figure's
    strokes are sub-pixel and their anti-aliased peak
    coverage never reaches 190 on all channels. Ink is
    now the brightest-channel predicate (>= 100: the
    strip's flats peak at 48 stage / 43 card / 76 for the
    current row's selected fill, so inside the slot only
    stroke pixels qualify). Measured on the same
    artifacts: 115 / 115 / 59 px per row (the demo-only
    row is the weakest) -- the 15 px floor stands, now
    at ~1/4 of the weakest measured signal.

  The self-test fixtures below paint the REAL card
  colour as their panel background, and a stage-washed
  fixture pins the control's trip path (UNPROVEN), so
  the fixture can no longer hide either conflation.

Wave V (V7) pair gate -- content fidelity across scenes.
  The workflow also snaps `home-missing-audio` (Wave V; the
  scene was previously tree-dumped only, so its row's
  thumbnail had no pixels to prove) and can pass both snaps
  to this probe (--also-snap / --also-tree). The seeded
  constructions make the four thumbnail strips a fidelity
  experiment:

    * home-populated's three saves are the demo project
      ("Harbour Lights"), then the same project plus one
      added figure saved twice more ("Night Train",
      "First Steps"; scenes.rs `scene_home_populated`).
      Among its three strips, exactly two must therefore
      be pixel-IDENTICAL and the third must DIFFER -- the
      added figure is red (editor.rs FIGURE_COLORS[1]) at
      stage x=390, hundreds of strip pixels.
    * home-missing-audio's single save is the demo project
      plus a missing audio clip (which does not render),
      so its strip must be pixel-identical to the
      populated snap's singleton strip -- the same content
      rendered by a separate process -- and differ from
      the pair.

  Row ORDER is never assumed (the scan sorts
  newest-first and save mtimes can tie): the gate works
  on the multiset of strips. Identical means 0 differing
  pixels -- the rasteriser's determinism is a proven
  property of this codebase (the subtitle census's
  pixel-identical mid-stage legs rest on it); different
  means >= DIFF_FLOOR differing pixels, ~1/4 of the
  expected added-figure signal. The missing-audio row
  also passes the same non-blank floors and totals
  control as the populated rows, against its own tree
  dump. If the four strips do not share one pixel size,
  the fixture geometry moved and the gate reports
  UNPROVEN rather than guessing.

Usage:
    python3 scripts/home_thumbnail_probe.py \
        headless-out/snap-home-populated.png \
        headless-out/tree-home-populated.json \
        [headless-out/home-thumbnail-probe.txt] \
        [--also-snap headless-out/snap-home-missing-audio.png \
         --also-tree headless-out/tree-home-missing-audio.json]
    python3 scripts/home_thumbnail_probe.py --self-test

Exit codes: 0 = PASS (thumbnails pixel-proven), 1 = FAIL
(thumbnails absent or indistinguishable), 2 = UNPROVEN
(inputs missing/unreadable, or the control invalidated the
colour assumptions). The transcript is printed and, when an
output path is given, written there in every case -- under
the workflow's fail-soft contract an UNPROVEN/FAIL transcript
is probe data, not a workflow failure.

Stdlib only (the headless runner has no PIL): the PNG reader
below handles the encoder shapes a wgpu readback produces
(8-bit, colour types 0/2/3/4/6, non-interlaced; 16-bit is
reduced to its high byte). Adam7 input is reported as
UNPROVEN rather than misread.
"""

import json
import os
import struct
import sys
import zlib

# --- The seeded fixture's facts (source-cited above) -----------
STAGE_BG = (0x23, 0x28, 0x30)   # Project::new_demo background
# Per-channel Chebyshev tolerance for "is stage background".
# The nearest other flat colour the source fixes is the
# theme's card background #1D222B (29, 34, 43), a distance
# of (6, 6, 5); the tolerance must stay under half the
# 5-channel minimum (2.5) so classification is by nearest
# source colour -- 2 (Wave W re-derivation, see docstring;
# the original 12 swallowed the card colour whole).
BG_TOL = 2
# Ink = a bright stroke pixel: brightest channel >= this.
# Figure 0xE8E8EC anti-aliased by the thumbnail's Triangle
# shrink peaks far below full value; 100 clears every flat
# the strip can hold (stage 48, card 43, selected fill 76)
# while the weakest real row measures 59 px of it.
INK_MIN = 100
THUMB_W_PT = 56.0               # home_panel.rs THUMB_W_PT
ROW_PAD_PT = 8.0                # design::row_base px(8.0)

# --- Floors (Wave L discipline: ~1/3 of the expected signal) ---
ROW_BG_FLOOR = 0.18             # measured ~0.33 on real snaps
ROW_INK_FLOOR = 15              # measured ~59+ on real snaps
CONTROL_BG_CEIL = 0.06
DIFF_FLOOR = 25                 # differing px: same vs added-figure content
ROW_KEYS = ("home-row-0", "home-row-1", "home-row-2")
MISSING_ROW_KEYS = ("home-row-0",)
CONTROL_KEY = "home-totals"


# ------------------------------------------------------------------
# Minimal PNG reader (stdlib).

class PngError(Exception):
    pass


def decode_png(data):
    """Return (width, height, rows) with rows[y][x] = (r, g, b)."""
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise PngError("not a PNG")
    pos = 8
    ihdr = None
    plte = None
    idat = bytearray()
    while pos + 8 <= len(data):
        (length,) = struct.unpack(">I", data[pos:pos + 4])
        ctype = data[pos + 4:pos + 8]
        chunk = data[pos + 8:pos + 8 + length]
        pos += 12 + length
        if ctype == b"IHDR":
            ihdr = struct.unpack(">IIBBBBB", chunk)
        elif ctype == b"PLTE":
            plte = [tuple(chunk[i:i + 3]) for i in range(0, len(chunk) - 2, 3)]
        elif ctype == b"IDAT":
            idat.extend(chunk)
        elif ctype == b"IEND":
            break
    if ihdr is None:
        raise PngError("no IHDR")
    w, h, depth, colour, _comp, _filt, interlace = ihdr
    if interlace != 0:
        raise PngError("interlaced PNG (Adam7) not supported")
    if depth not in (8, 16):
        raise PngError(f"unsupported bit depth {depth}")
    channels = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}[colour]
    step = 2 if depth == 16 else 1
    bpp = channels * step          # bytes per pixel (filter unit)
    stride = w * bpp
    raw = zlib.decompress(bytes(idat))
    if len(raw) < h * (stride + 1):
        raise PngError("truncated image data")
    rows = []
    prev = bytearray(stride)
    off = 0
    for _y in range(h):
        ftype = raw[off]
        line = bytearray(raw[off + 1:off + 1 + stride])
        off += stride + 1
        if ftype == 1:
            for i in range(bpp, stride):
                line[i] = (line[i] + line[i - bpp]) & 0xFF
        elif ftype == 2:
            for i in range(stride):
                line[i] = (line[i] + prev[i]) & 0xFF
        elif ftype == 3:
            for i in range(stride):
                left = line[i - bpp] if i >= bpp else 0
                line[i] = (line[i] + ((left + prev[i]) >> 1)) & 0xFF
        elif ftype == 4:
            for i in range(stride):
                a = line[i - bpp] if i >= bpp else 0
                b = prev[i]
                c = prev[i - bpp] if i >= bpp else 0
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                pred = a if (pa <= pb and pa <= pc) else (b if pb <= pc else c)
                line[i] = (line[i] + pred) & 0xFF
        elif ftype != 0:
            raise PngError(f"unknown filter {ftype}")
        row = []
        for x in range(w):
            base = x * bpp
            px = line[base:base + bpp:step]
            if colour == 3:
                row.append(plte[px[0]] if plte and px[0] < len(plte) else (0, 0, 0))
            elif colour == 0:
                row.append((px[0], px[0], px[0]))
            else:
                row.append((px[0], px[1], px[2]))
        rows.append(row)
        prev = line
    return w, h, rows


# ------------------------------------------------------------------
# Tree dump: mirror scripts/overflow_scan.py's reading of it --
# {"elements": [{"path", "key", "rect": [x, y, w, h]}, ...]},
# the shallowest path is the window root.

def load_elements(tree):
    if isinstance(tree, dict):
        return tree.get("elements", [])
    return []


def window_size(elements):
    root = None
    for el in elements:
        path = el.get("path", "")
        depth = len([s for s in path.split("/") if s])
        if depth and (root is None or depth < root[0]):
            root = (depth, el)
    win_w, win_h = 1440.0, 900.0
    if root is not None:
        rect = root[1].get("rect") or [0, 0, 0, 0]
        if len(rect) > 2 and rect[2] > 0:
            win_w = float(rect[2])
        if len(rect) > 3 and rect[3] > 0:
            win_h = float(rect[3])
    return win_w, win_h


def rect_of(elements, key):
    for el in elements:
        if el.get("key") == key:
            rect = el.get("rect")
            if rect and len(rect) >= 4:
                return [float(v) for v in rect[:4]]
    return None


# ------------------------------------------------------------------
# The measurement.

def strip_bounds(png_w, png_h, rect, sx, sy):
    """The row's thumbnail strip in PNG pixels: (x0, y0, x1, y1)."""
    x0 = int((rect[0] + ROW_PAD_PT) * sx)
    x1 = int((rect[0] + ROW_PAD_PT + THUMB_W_PT) * sx)
    y0 = int((rect[1] + 2.0) * sy)
    y1 = int((rect[1] + rect[3] - 2.0) * sy)
    x0, y0 = max(0, x0), max(0, y0)
    x1, y1 = min(png_w, x1), min(png_h, y1)
    return x0, y0, x1, y1


def strip_stats(rows, png_w, png_h, rect, sx, sy):
    """(total, stage_bg_hits, ink_hits) over the row's thumb strip."""
    x0, y0, x1, y1 = strip_bounds(png_w, png_h, rect, sx, sy)
    total = bg = ink = 0
    for y in range(y0, y1):
        row = rows[y]
        for x in range(x0, x1):
            r, g, b = row[x]
            total += 1
            if (abs(r - STAGE_BG[0]) <= BG_TOL and abs(g - STAGE_BG[1]) <= BG_TOL
                    and abs(b - STAGE_BG[2]) <= BG_TOL):
                bg += 1
            elif max(r, g, b) >= INK_MIN:
                ink += 1
    return total, bg, ink


def strip_pixels(rows, png_w, png_h, rect, sx, sy):
    """(w, h, flat [(r, g, b), ...]) over the strip, or None if empty."""
    x0, y0, x1, y1 = strip_bounds(png_w, png_h, rect, sx, sy)
    if x1 <= x0 or y1 <= y0:
        return None
    pixels = []
    for y in range(y0, y1):
        pixels.extend(rows[y][x0:x1])
    return x1 - x0, y1 - y0, pixels


def measure_snap(png_bytes, tree, row_keys):
    """The control + non-blank measurement for ONE Home snap.

    Return (verdict, exit_code, lines, strips, failures):
    `lines` carries no final verdict line (callers compose
    it); `strips` maps each measured row key to its
    strip_pixels result for the pair gate.
    """
    lines = []
    strips = {}
    try:
        png_w, png_h, rows = decode_png(png_bytes)
    except (PngError, zlib.error, struct.error) as e:
        return "UNPROVEN", 2, [f"snap PNG unreadable: {e}"], strips, 0
    elements = load_elements(tree)
    if not elements:
        return "UNPROVEN", 2, ["tree dump carries no elements"], strips, 0
    win_w, win_h = window_size(elements)
    sx, sy = png_w / win_w, png_h / win_h
    lines.append(f"snap {png_w}x{png_h}px, window {win_w:.0f}x{win_h:.0f}pt, "
                 f"scale {sx:.2f}x{sy:.2f}")

    # Negative control first: if the totals strip reads as
    # stage background, the colour premise is broken and no
    # row verdict can be trusted.
    control_rect = rect_of(elements, CONTROL_KEY)
    if control_rect is None:
        return "UNPROVEN", 2, lines + [f"control element {CONTROL_KEY!r} "
                                       "missing from the tree dump"], strips, 0
    c_total, c_bg, _c_ink = strip_stats(rows, png_w, png_h, control_rect, sx, sy)
    if c_total == 0:
        return ("UNPROVEN", 2,
                lines + ["control strip is empty (rect off-snap?)"],
                strips, 0)
    c_frac = c_bg / c_total
    lines.append(f"control {CONTROL_KEY}: stage-bg share {c_frac:.3f} "
                 f"over {c_total}px (ceiling {CONTROL_BG_CEIL})")
    if c_frac > CONTROL_BG_CEIL:
        lines.append("control trips: the strip colour premise does not "
                     "hold for this snap (theme/fixture drift?)")
        return "UNPROVEN", 2, lines, strips, 0

    failures = 0
    for key in row_keys:
        rect = rect_of(elements, key)
        if rect is None:
            lines.append(f"FAIL  {key}: element missing from the tree dump")
            failures += 1
            continue
        total, bg, ink = strip_stats(rows, png_w, png_h, rect, sx, sy)
        if total == 0:
            lines.append(f"FAIL  {key}: thumbnail strip is empty (rect off-snap?)")
            failures += 1
            continue
        strips[key] = strip_pixels(rows, png_w, png_h, rect, sx, sy)
        frac = bg / total
        ok = frac >= ROW_BG_FLOOR and ink >= ROW_INK_FLOOR
        lines.append(
            f"{'PASS ' if ok else 'FAIL '} {key}: stage-bg share {frac:.3f} "
            f"(floor {ROW_BG_FLOOR}), ink {ink}px (floor {ROW_INK_FLOOR}), "
            f"strip {total}px")
        if not ok:
            failures += 1
    if failures:
        return "FAIL", 1, lines, strips, failures
    return "PASS", 0, lines, strips, 0


def analyse(png_bytes, tree):
    """Return (verdict, exit_code, lines) — the populated snap alone."""
    verdict, code, lines, _strips, failures = measure_snap(
        png_bytes, tree, ROW_KEYS)
    if verdict == "FAIL":
        lines.append(f"HOME THUMBNAIL PROBE: FAIL ({failures} row(s) "
                     "without a rendered thumbnail)")
    elif verdict == "PASS":
        lines.append("HOME THUMBNAIL PROBE: PASS (all three Home rows carry a "
                     "rendered frame-0 thumbnail)")
    return verdict, code, lines


def pixel_diff(a, b):
    """Differing-pixel count between two same-size strips."""
    return sum(1 for pa, pb in zip(a[2], b[2]) if pa != pb)


def analyse_pair(pop_bytes, pop_tree, miss_bytes, miss_tree):
    """Return (verdict, exit_code, lines) — the Wave V pair gate.

    Both snaps' rows must carry rendered thumbnails (the
    measure_snap floors + control, per snap), and the four
    strips must satisfy the seeded constructions' fidelity
    pattern (see the module docstring): among the populated
    strips exactly one identical pair plus a differing
    singleton, and the missing-audio strip pixel-identical
    to that singleton.
    """
    lines = []
    sections = []
    total_failures = 0

    lines.append("[home-populated]")
    v, _c, ls, pop_strips, f = measure_snap(pop_bytes, pop_tree, ROW_KEYS)
    lines.extend(ls)
    sections.append(v)
    total_failures += f

    lines.append("[home-missing-audio]")
    v, _c, ls, miss_strips, f = measure_snap(
        miss_bytes, miss_tree, MISSING_ROW_KEYS)
    lines.extend(ls)
    sections.append(v)
    total_failures += f

    lines.append("[thumbnail fidelity gate]")
    pop = [pop_strips.get(k) for k in ROW_KEYS]
    miss = miss_strips.get(MISSING_ROW_KEYS[0])
    gate = None  # None = PASS, else (verdict, reason lines)
    if any(s is None for s in pop) or miss is None:
        gate = ("UNPROVEN", ["a row strip could not be extracted "
                             "(see the sections above)"])
    else:
        sizes = {(s[0], s[1]) for s in pop} | {(miss[0], miss[1])}
        if len(sizes) != 1:
            gate = ("UNPROVEN",
                    [f"the four strips do not share one pixel size: "
                     f"{sorted(sizes)} — the fixture geometry moved"])
    if gate is None:
        d01 = pixel_diff(pop[0], pop[1])
        d02 = pixel_diff(pop[0], pop[2])
        d12 = pixel_diff(pop[1], pop[2])
        lines.append(f"populated pairwise differing pixels: "
                     f"rows 0-1: {d01}, rows 0-2: {d02}, rows 1-2: {d12} "
                     f"(identical = 0, different-content floor {DIFF_FLOOR})")
        pairs = {(0, 1): d01, (0, 2): d02, (1, 2): d12}
        identical = [ij for ij, d in pairs.items() if d == 0]
        cross_ok = all(d >= DIFF_FLOOR
                       for ij, d in pairs.items() if ij not in identical)
        if len(identical) != 1 or not cross_ok:
            gate = ("FAIL",
                    ["the populated strips are not one identical pair plus "
                     "a differing singleton — the added-figure construction "
                     "no longer reads in the thumbnails"])
        else:
            (i, j) = identical[0]
            k = ({0, 1, 2} - {i, j}).pop()
            lines.append(f"PASS  populated rows {i} and {j} share a thumbnail "
                         f"pixel-for-pixel; row {k} differs")
            dm_single = pixel_diff(miss, pop[k])
            dm_pair = min(pixel_diff(miss, pop[i]), pixel_diff(miss, pop[j]))
            lines.append(f"missing-audio strip vs the singleton row {k}: "
                         f"{dm_single} differing px; vs the paired rows: "
                         f"{dm_pair} differing px")
            if dm_single == 0 and dm_pair >= DIFF_FLOOR:
                lines.append("PASS  the missing-audio project's thumbnail is "
                             "the demo project's thumbnail, rendered by a "
                             "separate process, and differs from the "
                             "added-figure pair")
                gate = ("PASS", [])
            else:
                gate = ("FAIL",
                        ["the missing-audio strip does not match the "
                         "populated singleton strip — either the demo "
                         "content changed in one scene or thumbnails are "
                         "not content-faithful"])
    gate_verdict, gate_lines = gate
    lines.extend(("FAIL  " if gate_verdict == "FAIL" else "") + line
                 for line in gate_lines)
    sections.append(gate_verdict)
    if gate_verdict == "FAIL":
        total_failures += 1

    if "FAIL" in sections:
        lines.append(f"HOME THUMBNAIL PROBE: FAIL ({total_failures} "
                     "assertion(s) failed across the two Home snaps)")
        return "FAIL", 1, lines
    if "UNPROVEN" in sections:
        lines.append("HOME THUMBNAIL PROBE: UNPROVEN")
        return "UNPROVEN", 2, lines
    lines.append("HOME THUMBNAIL PROBE: PASS (populated + missing-audio rows "
                 "carry rendered thumbnails, and the thumbnails are "
                 "content-faithful across the two scenes)")
    return "PASS", 0, lines


# ------------------------------------------------------------------
# Self-test: synthetic snap + tree through the real decoder and
# the real analysis, both polarities.

def encode_png(w, h, painter):
    def chunk(ctype, payload):
        out = struct.pack(">I", len(payload)) + ctype + payload
        return out + struct.pack(">I", zlib.crc32(ctype + payload) & 0xFFFFFFFF)
    raw = bytearray()
    for y in range(h):
        raw.append(0)
        for x in range(w):
            raw.extend(painter(x, y))
    ihdr = struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr)
            + chunk(b"IDAT", zlib.compress(bytes(raw)))
            + chunk(b"IEND", b""))


def self_test():
    W, H = 360, 300
    # Rows at x=16 (card), widths 300; thumb strips start at +8.
    rects = {"home-row-0": [16, 20, 300, 80], "home-row-1": [16, 106, 300, 80],
             "home-row-2": [16, 192, 300, 80], "home-totals": [16, 278, 300, 18]}
    elements = [{"path": "#0", "key": None, "rect": [0, 0, W, H]}]
    for i, (k, r) in enumerate(rects.items()):
        elements.append({"path": f"#0/panel/{k}", "key": k, "rect": r})
    tree = {"elements": elements}

    # The panel background is the app theme's real card
    # colour #1D222B -- the Wave W lesson: the invented
    # (24, 26, 31) this fixture used before sits farther
    # from the stage background than the real card does,
    # and hid the tolerance conflation for three waves.
    CARD = (29, 34, 43, 255)

    def painted(x, y):
        for k in ROW_KEYS:
            rx, ry, _rw, rh = rects[k]
            if rx + 8 <= x < rx + 8 + 56 and ry + 12 <= y < ry + 12 + 42:
                # figure ink: a near-white diagonal inside the thumb
                if abs((x - (rx + 8)) - (y - (ry + 12))) < 2:
                    return (232, 232, 236, 255)
                return STAGE_BG + (255,)
        return CARD

    verdict, code, lines = analyse(encode_png(W, H, painted), tree)
    assert verdict == "PASS" and code == 0, lines
    verdict2, code2, lines2 = analyse(
        encode_png(W, H, lambda x, y: CARD), tree)
    assert verdict2 == "FAIL" and code2 == 1, lines2
    print("self-test PASS: painted fixture -> PASS, blank fixture -> FAIL")
    for line in lines:
        print("  " + line)

    # A snap washed in stage background (the panel never
    # rendered; the stage shows through everywhere,
    # control included) must trip the control and issue
    # no verdict -- the control's protective purpose,
    # pinned for the first time.
    verdict3, code3, lines3 = analyse(
        encode_png(W, H, lambda x, y: STAGE_BG + (255,)), tree)
    assert verdict3 == "UNPROVEN" and code3 == 2, lines3
    print("self-test PASS: stage-washed fixture -> UNPROVEN (control trips)")

    # --- Pair gate (Wave V, V7): synthetic versions of the
    # two scenes' constructions. Kind A = the demo thumb
    # (stage bg + ink diagonal), kind B = A plus a red
    # block (the added figure), kind C = a third distinct
    # thumb (stage bg + an ink bar, no diagonal).
    miss_rects = {"home-row-0": [16, 20, 300, 80],
                  "home-totals": [16, 278, 300, 18]}
    miss_elements = [{"path": "#0", "key": None, "rect": [0, 0, W, H]}]
    for k, r in miss_rects.items():
        miss_elements.append({"path": f"#0/panel/{k}", "key": k, "rect": r})
    miss_tree = {"elements": miss_elements}

    def make_painter(row_rects, kinds):
        def paint(x, y):
            for k, r in row_rects.items():
                rx, ry = r[0], r[1]
                if not (rx + 8 <= x < rx + 8 + 56 and ry + 2 <= y < ry + 78):
                    continue
                kind = kinds.get(k)
                if kind is None or not (ry + 12 <= y < ry + 12 + 42):
                    return CARD
                if kind == "B" and rx + 36 <= x < rx + 52 \
                        and ry + 20 <= y < ry + 44:
                    return (255, 93, 93, 255)
                if kind == "C":
                    if ry + 30 <= y < ry + 32:
                        return (232, 232, 236, 255)
                    return STAGE_BG + (255,)
                if abs((x - (rx + 8)) - (y - (ry + 12))) < 2:
                    return (232, 232, 236, 255)
                return STAGE_BG + (255,)
            return CARD
        return paint

    pop_png = encode_png(W, H, make_painter(
        rects, {"home-row-0": "B", "home-row-1": "B", "home-row-2": "A"}))
    miss_png = encode_png(W, H, make_painter(
        miss_rects, {"home-row-0": "A"}))
    verdict, code, lines = analyse_pair(pop_png, tree, miss_png, miss_tree)
    assert verdict == "PASS" and code == 0, lines
    print("self-test PASS: pair fixture (pair B + singleton A, missing A) "
          "-> PASS")
    for line in lines:
        print("  " + line)

    # The missing-audio thumb carrying the ADDED figure's
    # content (kind B) must fail the fidelity gate.
    miss_b = encode_png(W, H, make_painter(
        miss_rects, {"home-row-0": "B"}))
    verdict, code, lines = analyse_pair(pop_png, tree, miss_b, miss_tree)
    assert verdict == "FAIL" and code == 1, lines
    print("self-test PASS: missing-audio thumb = added-figure content "
          "-> FAIL")

    # Three distinct populated thumbs: no identical pair,
    # the construction is gone -> FAIL.
    pop_abc = encode_png(W, H, make_painter(
        rects, {"home-row-0": "A", "home-row-1": "B", "home-row-2": "C"}))
    verdict, code, lines = analyse_pair(pop_abc, tree, miss_png, miss_tree)
    assert verdict == "FAIL" and code == 1, lines
    print("self-test PASS: three distinct populated thumbs -> FAIL")

    # A blank missing-audio snap fails the non-blank floors.
    verdict, code, lines = analyse_pair(
        pop_png, tree,
        encode_png(W, H, lambda x, y: CARD), miss_tree)
    assert verdict == "FAIL" and code == 1, lines
    print("self-test PASS: blank missing-audio fixture -> FAIL")
    return 0


def load_tree(path):
    """(tree, None) or (None, error line) — unreadable is data."""
    try:
        return json.load(open(path)), None
    except Exception as e:
        return None, f"tree dump unreadable: {e}"


def main(argv):
    if "--self-test" in argv:
        return self_test()
    args = argv[1:]
    also_snap = also_tree = None
    positional = []
    i = 0
    while i < len(args):
        if args[i] == "--also-snap" and i + 1 < len(args):
            also_snap = args[i + 1]
            i += 2
        elif args[i] == "--also-tree" and i + 1 < len(args):
            also_tree = args[i + 1]
            i += 2
        else:
            positional.append(args[i])
            i += 1
    if len(positional) < 2:
        print(__doc__)
        return 2
    snap_path, tree_path = positional[0], positional[1]
    out_path = positional[2] if len(positional) > 2 else None
    pair_requested = also_snap is not None or also_tree is not None
    lines = []
    if not os.path.isfile(snap_path) or not os.path.isfile(tree_path):
        verdict, code = "UNPROVEN", 2
        lines.append(f"inputs missing (snap: {snap_path}, tree: {tree_path}) "
                     "-- the headless run produced no populated Home snap/tree")
        lines.append("HOME THUMBNAIL PROBE: UNPROVEN")
    elif pair_requested and (also_snap is None or also_tree is None):
        verdict, code = "UNPROVEN", 2
        lines.append("--also-snap and --also-tree must be given together")
        lines.append("HOME THUMBNAIL PROBE: UNPROVEN")
    else:
        tree, err = load_tree(tree_path)
        if err is not None:
            verdict, code = "UNPROVEN", 2
            lines.append(err)
            lines.append("HOME THUMBNAIL PROBE: UNPROVEN")
        elif not pair_requested:
            verdict, code, lines = analyse(open(snap_path, "rb").read(), tree)
        elif not os.path.isfile(also_snap) or not os.path.isfile(also_tree):
            # The populated half still runs; the pair gate is
            # the unproven part (the snap step is fail-soft, so
            # a missing missing-audio snap is probe data).
            verdict, code, lines = analyse(open(snap_path, "rb").read(), tree)
            lines.append(f"pair inputs missing (snap: {also_snap}, "
                         f"tree: {also_tree}) -- the headless run produced "
                         "no missing-audio Home snap/tree")
            if verdict != "FAIL":
                verdict, code = "UNPROVEN", 2
                lines.append("HOME THUMBNAIL PROBE: UNPROVEN")
        else:
            miss_tree, err = load_tree(also_tree)
            if err is not None:
                verdict, code = "UNPROVEN", 2
                lines.append(f"missing-audio {err}")
                lines.append("HOME THUMBNAIL PROBE: UNPROVEN")
            else:
                verdict, code, lines = analyse_pair(
                    open(snap_path, "rb").read(), tree,
                    open(also_snap, "rb").read(), miss_tree)
    text = "\n".join(lines) + "\n"
    print(text, end="")
    if out_path:
        os.makedirs(os.path.dirname(out_path) or ".", exist_ok=True)
        with open(out_path, "w") as fh:
            fh.write(text)
        summary = os.environ.get("GITHUB_STEP_SUMMARY")
        if summary:
            with open(summary, "a") as fh:
                fh.write("\n### Home thumbnail probe (Wave Q, Q16; pair "
                         "gate Wave V, V7)\n\n```\n" + text + "```\n")
    return code


if __name__ == "__main__":
    sys.exit(main(sys.argv))
