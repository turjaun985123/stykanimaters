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
    18% against a ~40% expectation -- and (b) INK: near-white
    pixels (every channel >= 190) from the stick figure's
    strokes, which at thumbnail scale are a thin minority;
    floor 15 px against a ~50+ expectation. Text elsewhere in
    the row cannot fake either signal: the strip covers only
    the thumbnail's slot.
  * Negative control: the same strip over the `home-totals`
    footer (text only, never a thumbnail) must show almost
    no stage background (<= 6%). If the control trips, the
    probe's colour assumption is wrong for this theme and
    the verdict is UNPROVEN, not PASS -- a probe that cannot
    distinguish its control proves nothing.

Usage:
    python3 scripts/home_thumbnail_probe.py \
        headless-out/snap-home-populated.png \
        headless-out/tree-home-populated.json \
        [headless-out/home-thumbnail-probe.txt]
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
BG_TOL = 12                     # per-channel Chebyshev tolerance
INK_MIN = 190                   # figure 0xE8E8EC -> (232, 232, 236)
THUMB_W_PT = 56.0               # home_panel.rs THUMB_W_PT
ROW_PAD_PT = 8.0                # design::row_base px(8.0)

# --- Floors (Wave L discipline: ~1/3 of the expected signal) ---
ROW_BG_FLOOR = 0.18             # expected ~0.40
ROW_INK_FLOOR = 15              # expected ~50+
CONTROL_BG_CEIL = 0.06
ROW_KEYS = ("home-row-0", "home-row-1", "home-row-2")
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

def strip_stats(rows, png_w, png_h, rect, sx, sy):
    """(total, stage_bg_hits, ink_hits) over the row's thumb strip."""
    x0 = int((rect[0] + ROW_PAD_PT) * sx)
    x1 = int((rect[0] + ROW_PAD_PT + THUMB_W_PT) * sx)
    y0 = int((rect[1] + 2.0) * sy)
    y1 = int((rect[1] + rect[3] - 2.0) * sy)
    x0, y0 = max(0, x0), max(0, y0)
    x1, y1 = min(png_w, x1), min(png_h, y1)
    total = bg = ink = 0
    for y in range(y0, y1):
        row = rows[y]
        for x in range(x0, x1):
            r, g, b = row[x]
            total += 1
            if (abs(r - STAGE_BG[0]) <= BG_TOL and abs(g - STAGE_BG[1]) <= BG_TOL
                    and abs(b - STAGE_BG[2]) <= BG_TOL):
                bg += 1
            elif r >= INK_MIN and g >= INK_MIN and b >= INK_MIN:
                ink += 1
    return total, bg, ink


def analyse(png_bytes, tree):
    """Return (verdict, exit_code, lines)."""
    lines = []
    try:
        png_w, png_h, rows = decode_png(png_bytes)
    except (PngError, zlib.error, struct.error) as e:
        return "UNPROVEN", 2, [f"snap PNG unreadable: {e}"]
    elements = load_elements(tree)
    if not elements:
        return "UNPROVEN", 2, ["tree dump carries no elements"]
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
                                       "missing from the tree dump"]
    c_total, c_bg, _c_ink = strip_stats(rows, png_w, png_h, control_rect, sx, sy)
    if c_total == 0:
        return "UNPROVEN", 2, lines + ["control strip is empty (rect off-snap?)"]
    c_frac = c_bg / c_total
    lines.append(f"control {CONTROL_KEY}: stage-bg share {c_frac:.3f} "
                 f"over {c_total}px (ceiling {CONTROL_BG_CEIL})")
    if c_frac > CONTROL_BG_CEIL:
        lines.append("control trips: the strip colour premise does not "
                     "hold for this snap (theme/fixture drift?)")
        return "UNPROVEN", 2, lines

    failures = 0
    for key in ROW_KEYS:
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
        frac = bg / total
        ok = frac >= ROW_BG_FLOOR and ink >= ROW_INK_FLOOR
        lines.append(
            f"{'PASS ' if ok else 'FAIL '} {key}: stage-bg share {frac:.3f} "
            f"(floor {ROW_BG_FLOOR}), ink {ink}px (floor {ROW_INK_FLOOR}), "
            f"strip {total}px")
        if not ok:
            failures += 1
    if failures:
        lines.append(f"HOME THUMBNAIL PROBE: FAIL ({failures} row(s) "
                     "without a rendered thumbnail)")
        return "FAIL", 1, lines
    lines.append("HOME THUMBNAIL PROBE: PASS (all three Home rows carry a "
                 "rendered frame-0 thumbnail)")
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

    def painted(x, y):
        for k in ROW_KEYS:
            rx, ry, _rw, rh = rects[k]
            if rx + 8 <= x < rx + 8 + 56 and ry + 12 <= y < ry + 12 + 42:
                # figure ink: a near-white diagonal inside the thumb
                if abs((x - (rx + 8)) - (y - (ry + 12))) < 2:
                    return (232, 232, 236, 255)
                return STAGE_BG + (255,)
        return (24, 26, 31, 255)  # panel background

    verdict, code, lines = analyse(encode_png(W, H, painted), tree)
    assert verdict == "PASS" and code == 0, lines
    verdict2, code2, lines2 = analyse(
        encode_png(W, H, lambda x, y: (24, 26, 31, 255)), tree)
    assert verdict2 == "FAIL" and code2 == 1, lines2
    print("self-test PASS: painted fixture -> PASS, blank fixture -> FAIL")
    for line in lines:
        print("  " + line)
    return 0


def main(argv):
    if "--self-test" in argv:
        return self_test()
    if len(argv) < 3:
        print(__doc__)
        return 2
    snap_path, tree_path = argv[1], argv[2]
    out_path = argv[3] if len(argv) > 3 else None
    lines = []
    if not os.path.isfile(snap_path) or not os.path.isfile(tree_path):
        verdict, code = "UNPROVEN", 2
        lines.append(f"inputs missing (snap: {snap_path}, tree: {tree_path}) "
                     "-- the headless run produced no populated Home snap/tree")
        lines.append("HOME THUMBNAIL PROBE: UNPROVEN")
    else:
        try:
            tree = json.load(open(tree_path))
        except Exception as e:  # unreadable dump: data, not a crash
            verdict, code = "UNPROVEN", 2
            lines.append(f"tree dump unreadable: {e}")
            lines.append("HOME THUMBNAIL PROBE: UNPROVEN")
        else:
            verdict, code, lines = analyse(open(snap_path, "rb").read(), tree)
    text = "\n".join(lines) + "\n"
    print(text, end="")
    if out_path:
        os.makedirs(os.path.dirname(out_path) or ".", exist_ok=True)
        with open(out_path, "w") as fh:
            fh.write(text)
        summary = os.environ.get("GITHUB_STEP_SUMMARY")
        if summary:
            with open(summary, "a") as fh:
                fh.write("\n### Home thumbnail probe (Wave Q, Q16)\n\n```\n"
                         + text + "```\n")
    return code


if __name__ == "__main__":
    sys.exit(main(sys.argv))
