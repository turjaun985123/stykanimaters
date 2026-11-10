#!/usr/bin/env python3
"""Headless subtitle census — a seeded subtitle cue, pixel-proven
on the Rune stage (Wave P, P33; finished Wave Q, Q13).

WHAT THIS PROVES
  The stage subtitle draw (src/rune_sec/stage_text.rs
  `subtitle_geometry`, layout from src/subtitles.rs) actually
  PAINTS the frame's live cues on the stage, at the anchor the
  project's subtitle style names. Unit pins prove the geometry
  maths; only pixels prove the draw reaches the screen. The
  inputs are three `--snap` renders of the demo project from
  the headless harness (.github/workflows/headless.yml), all at
  the same window size:

    snap-subtitles-populated.png   scene `subtitles-populated`:
                                   ONE cue "We walk home before
                                   dark." spanning the whole
                                   timeline, playhead parked on
                                   frame 2, DEFAULT style
                                   (Bottom, scale 1.0, text
                                   0xFFFFFF, box 0x000000).
    snap-subtitles-top.png         scene `subtitles-top`: the
                                   SAME state with exactly one
                                   field flipped — the style's
                                   position is Top.
    snap-default.png               scene `default`: the demo
                                   project as shipped, no cue.
    snap-subtitles-speaker.png     scene `subtitles-speaker`
                                   (Wave S, S2): ONE CREDITED
                                   cue — speaker "NOVA", text
                                   "The light is still on." —
                                   spanning the whole timeline,
                                   playhead parked on frame 2,
                                   DEFAULT style (Bottom). The
                                   speaker leg's input; required
                                   alongside either input set.

  If a full-window snap is missing, the stage crop of the same
  scene is accepted instead (snap-subtitles-populated-crop.png,
  snap-subtitles-top-crop.png, snap-subtitles-speaker-crop.png,
  and the workflow's existing snap-stage.png for `default`);
  all four inputs must then be crops. Mixed full/crop input
  sets are refused — the pixels would not be comparable.

THE DISCIPLINE (the Wave L subtitle census, carried over)
  * The subtitle band is located from the FRAME'S OWN stage
    rect: the `stage` element's rect in the same run's tree
    dump (tree-subtitles-populated.json), mapped through the
    stage view's letterboxed fit (src/stage_view.rs:
    fit = min(el_w/stage_w, el_h/stage_h), drawn stage centred
    in the element). No pixel row is ever hardcoded. In crop
    mode the crop IS the stage element, so the rect is the
    image itself and the same fit maths applies.
  * The load-bearing comparison is the ANCHOR PAIR
    (populated vs top): identical state, one style field
    apart, so every differing pixel is subtitle paint by
    construction. The pair must differ inside BOTH bands
    (the cue left the bottom band and arrived in the top
    band), the near-white text face must appear in each
    band in exactly the scene anchored there, and the stage
    between the bands must be PIXEL-IDENTICAL — the
    anti-vacuity leg: it proves the pair really does differ
    only by the bands, so the band deltas cannot be pose,
    theme or timing noise.
  * The baseline leg (populated vs default, bottom band) is
    supporting only: the default scene parks on frame 0 and
    the demo pose differs there (root +28 units and a raised
    arm by frame 2), so its band delta is pose-contaminated
    by design and is gated only on "clearly nonzero" — a
    zero would mean the seeded snap IS the default snap,
    i.e. the scene never took.
  * The speaker leg (speaker vs populated, Wave S S2): both
    scenes park frame 2 under the default Bottom style and
    differ only in the planted cue — the speaker scene's
    cue is credited (speaker NOVA) with its own text. The
    stage draws a credited cue's first line through
    `subtitles::subtitle_display_lines`, which prefixes
    `NOVA: ` onto it; every line is centred as a whole
    string, so the prefix widens the painted line by
    exactly its own advance (6 chars x 16 units = 96 stage
    units) over the same text uncredited. The leg measures
    the near-white face EXTENT (leftmost to rightmost
    speaker-only face pixel, XOR against the default snap
    so any figure paint cancels) inside the bottom band
    and gates it between the two analytic spans: a line
    drawn WITHOUT the prefix cannot exceed the bare
    text's run, so a span past the gate proves the prefix
    painted. Anti-vacuity, mirroring the anchor pair: the
    two snaps must be pixel-identical in the top band and
    mid-stage (same park, same anchor — cue content is
    the only difference) and must differ in the bottom
    band.
  * The lane leg (speaker vs populated, Wave T T11): the
    subtitle LANE's block for the credited cue, pixel-side.
    S3 pinned the lane's label composition (`block_label`:
    `Name: ` prefixed on a credited cue) at unit level and
    S2 proved the prefix on the stage; what neither proves
    is that the lane block in the credited scene actually
    PAINTS its composed label. Fact 34 (scenes.rs): the
    speaker tree's inventory is identical to the populated
    tree's, lane included — no view branches on cue
    content for structure, and a block's rect comes from
    the cue's frames alone (`cue_rect`), never its text.
    The leg reads the `subtitle-cue-0` block rect from BOTH
    scenes' tree dumps and asserts they agree (the block
    did not move or resize for the credited cue), then,
    inside that one rect in the full-window snaps, asserts
    the two labels' pixels DIFFER (the credited scene's
    block does not show the uncredited scene's label) and
    that the credited block carries label ink of its own
    (near-white text pixels — the block's fill is the dark
    `control_off`, the label is drawn in the theme's text
    colour, and nothing else near-white lives inside a cue
    block). Full-window inputs only: the `--crop stage`
    snaps are crops of the stage element, and the lane
    sits in the timeline strip below it, so in crop mode
    the leg is reported as not run rather than gated on
    pixels that cannot contain it.
  * Floors are sized from the measured quantity's analytic
    census (below), at the same ~1/3 ratio the Wave L census
    used to turn a measurement into a floor. This script
    PRINTS every measured value; after the first CI run the
    integrator ratchets the floors to ~1/3 of the measured
    values and the analytic derivation retires.

FLOOR DERIVATION (analytic ink census)
  Glyph grid 3x5 (stage_text.rs GLYPH_COLS/GLYPH_ROWS), cell
  = subtitle_font_size / 5 = 20.0 / 5 = 4 stage units, so one
  lit cell paints 16 unit^2 of text face. The planted cue has
  21 non-space glyphs; a 3x5 glyph lights ~6-9 cells, so the
  face is ~150 lit cells ~ 2,400 unit^2. The backing plates
  (1.5x cell, box colour, opaque — stage_text.rs) add at
  least the face's area again around the glyphs, all inside
  the band.
  Worst plausible fit: the stage element at the QA budget
  floor (docs/QA_RUNE.md S3: stage height >= 360 px at
  1280x720) gives fit = 360/720 = 0.5, so face >= 2,400 *
  0.25 = 600 px and band delta (face + plates) >= 1,200 px.
    DIFF_FLOOR = 200   (~1/3 of the face-only worst case,
                        plates ignored entirely)
    FACE_FLOOR = 100   (the near-white XOR is pure text
                        colour — no plate contribution — so
                        it takes the wider margin)
  At the harness's default 1440x900 window the drawn stage
  is far larger (fit ~0.7), so real measurements should land
  several times above both floors.

SPEAKER SPAN GATE (Wave S, S2)
  A line of n chars runs (4n - 1) cells at 4 units per cell
  (stage_text.rs's CHAR_ADVANCE layout: 3-wide glyphs, 4
  cells per char). The speaker scene's bare text (22
  chars) runs (88 - 1) * 4 = 348 units; its credited line
  (28 chars incl. the 6-char `NOVA: ` prefix) runs
  (112 - 1) * 4 = 444. A glyph's edge columns can be
  unlit (the text's last glyph `.` lights only its centre
  column), shaving a few units off the measured face
  extent under either accounting — at most ~8 units
  across both ends for these strings. The gate takes
  half the prefix widening as its margin, the same
  conservative fraction as the floors above:
    SPEAKER_SPAN_GATE = 348 + 48 = 396 units
                        (bare run + 1/2 the widening;
                        the expected measurement is
                        ~440, the bare ceiling 348)

LANE FLOORS (Wave T, T11 — analytic, same discipline)
  The seeded cue spans the whole timeline, so its lane
  block is the timeline's full width at the demo's frame
  count: 166 pt wide, 18 pt tall (tree-subtitles-
  populated.json in the harness artifacts), with the
  label inset 4 pt each side (subtitle_lane.rs's
  `px(4.0)`) — a 158 pt text run. At the lane's LABEL
  font the harness's populated snap measures ~6.3 pt of
  advance per char, so the uncredited 25-char label
  already spans ~158 pt: it saturates the text run, and
  the credited 28-char string (6-char `NOVA: ` prefix +
  22-char text) is LONGER than the run and truncates
  (the text element's `.truncate()`). Ink extent
  therefore cannot isolate the prefix in the lane — both
  labels end at the run's right edge — so the lane leg
  gates content difference and ink presence, and only
  REPORTS the two extents (the stage leg above owns the
  prefix-span proof, on a stage wide enough to hold the
  line). Ink census at 1x: the populated label paints
  ~400 near-white px inside its block (measured on the
  harness snap); two different strings of 25/28 chars
  disagree at nearly every glyph column, so the pair's
  differing pixels inside the block run several hundred.
    LANE_DIFF_FLOOR = 200   (the DIFF_FLOOR class)
    LANE_INK_FLOOR  = 100   (the FACE_FLOOR class,
                             ~1/4 of the measured ink)
  Block-rect agreement takes the stage rect's own
  tolerance: max drift <= 1.0 pt across the pair.

BAND GEOMETRY (stage units, src/subtitles.rs constants)
  SUBTITLE_EDGE_MARGIN = 16, SUBTITLE_BASE_LINE_HEIGHT = 24:
  one line's box spans [edge, edge + 40]. The glyph face sits
  inside it (centre at edge + 28 for line 0, +- font/2, plus
  the half-cell-low quirk stage_text.rs inherits from
  composite_text) and the plates reach ~2 units past the
  face. BAND_UNITS = 48 covers the box plus 8 units of slack
  on the inner side, and stays clear of the demo figure
  (root y = 470, feet well above 720 - 48 = 672).

INPUT GEOMETRY CONSTANTS
  The demo project's stage is [960, 720] (model.rs
  Project::new_demo). The three scenes all run the demo
  project, so the fit maths uses those numbers; if the demo
  stage size ever changes, this census fails its band gates
  loudly rather than passing on a wrong band.

USAGE
  python3 scripts/subtitle_census.py [DIR]   (default
      headless-out) — census over the harness outputs in DIR.
  python3 scripts/subtitle_census.py --self-test — synthetic
      PNG/geometry exercise; needs no harness outputs.

  Exit 0 = PASS, 1 = FAIL (missing inputs are FAIL, named:
  a proof that did not happen is not a pass). The transcript
  is written to DIR/subtitle-census.txt and appended to
  $GITHUB_STEP_SUMMARY when set, in the headless workflow's
  probe idiom. Stdlib only.
"""

import json
import math
import os
import struct
import sys
import zlib

# ----------------------------------------------------------------
# Constants (derivations in the module docstring)
# ----------------------------------------------------------------

STAGE_W = 960.0   # Project::new_demo stage size (model.rs)
STAGE_H = 720.0
BAND_UNITS = 48.0  # edge margin 16 + line height 24 + 8 slack
DIFF_FLOOR = 200
FACE_FLOOR = 100
NEAR_WHITE = 224   # every channel >= this counts as near-white
MIN_STAGE_H_PX = 300  # drawn stage height below this proves nothing

SEEDED_BOTTOM_FULL = "snap-subtitles-populated.png"
SEEDED_TOP_FULL = "snap-subtitles-top.png"
BASELINE_FULL = "snap-default.png"
SEEDED_BOTTOM_CROP = "snap-subtitles-populated-crop.png"
SEEDED_TOP_CROP = "snap-subtitles-top-crop.png"
BASELINE_CROP = "snap-stage.png"
SEEDED_SPEAKER_FULL = "snap-subtitles-speaker.png"
SEEDED_SPEAKER_CROP = "snap-subtitles-speaker-crop.png"
TREE_SEEDED = "tree-subtitles-populated.json"
TREE_BASELINE = "tree-default.json"
TREE_SPEAKER = "tree-subtitles-speaker.json"

# Lane leg (Wave T, T11): the cue block's element key
# (subtitle_lane.rs's `block_key`, stored index 0 — both
# seeded scenes plant exactly one cue) and its floors
# (derivations in the module docstring).
LANE_BLOCK_KEY = "subtitle-cue-0"
LANE_RECT_DRIFT_MAX = 1.0
LANE_DIFF_FLOOR = 200
LANE_INK_FLOOR = 100

# Speaker leg (Wave S, S2): the credited cue's text runs
# (4*22 - 1) * 4 = 348 stage units bare; the `NOVA: `
# prefix (6 chars) widens the drawn line by 6 * 16 = 96
# units to 444. The gate sits halfway through the
# widening (derivation in the module docstring).
SPEAKER_BARE_RUN_UNITS = 348.0
SPEAKER_CREDITED_RUN_UNITS = 444.0
SPEAKER_SPAN_GATE_UNITS = 396.0


# ----------------------------------------------------------------
# PNG decoding (stdlib only)
# ----------------------------------------------------------------

class PngError(Exception):
    pass


def _paeth(a, b, c):
    p = a + b - c
    pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
    if pa <= pb and pa <= pc:
        return a
    if pb <= pc:
        return b
    return c


def read_png(path):
    """Decode a PNG to (width, height, rows) with rows a list
    of bytes of length 3*width (RGB, alpha dropped).

    Supports colour types 0/2/3/4/6 at bit depth 8 or 16,
    non-interlaced. Anything else raises PngError with the
    reason — a decoder limit must be loud, never a silent
    wrong image."""
    with open(path, "rb") as fh:
        data = fh.read()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise PngError("not a PNG (bad signature)")
    pos = 8
    ihdr = None
    plte = None
    idat = bytearray()
    while pos + 8 <= len(data):
        (length,) = struct.unpack(">I", data[pos:pos + 4])
        ctype = data[pos + 4:pos + 8]
        chunk = data[pos + 8:pos + 8 + length]
        if ctype == b"IHDR":
            ihdr = struct.unpack(">IIBBBBB", chunk)
        elif ctype == b"PLTE":
            plte = chunk
        elif ctype == b"IDAT":
            idat.extend(chunk)
        elif ctype == b"IEND":
            break
        pos += 12 + length
    if ihdr is None:
        raise PngError("no IHDR chunk")
    width, height, depth, color, _comp, _filt, interlace = ihdr
    if interlace != 0:
        raise PngError("interlaced PNG (Adam7) not supported")
    if depth not in (8, 16):
        raise PngError(f"bit depth {depth} not supported")
    channels = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}.get(color)
    if channels is None:
        raise PngError(f"colour type {color} not supported")
    if color == 3 and plte is None:
        raise PngError("palette PNG without a PLTE chunk")
    try:
        raw = zlib.decompress(bytes(idat))
    except zlib.error as e:
        raise PngError(f"IDAT decompress failed: {e}")
    sample_bytes = depth // 8
    bpp = channels * sample_bytes          # bytes per pixel, for filtering
    stride = width * bpp
    if len(raw) < height * (stride + 1):
        raise PngError("truncated image data")
    # Unfilter.
    rows = []
    prev = bytearray(stride)
    off = 0
    for _y in range(height):
        ftype = raw[off]
        line = bytearray(raw[off + 1:off + 1 + stride])
        off += stride + 1
        if ftype == 0:
            pass
        elif ftype == 1:  # Sub
            for i in range(bpp, stride):
                line[i] = (line[i] + line[i - bpp]) & 0xFF
        elif ftype == 2:  # Up
            for i in range(stride):
                line[i] = (line[i] + prev[i]) & 0xFF
        elif ftype == 3:  # Average
            for i in range(stride):
                left = line[i - bpp] if i >= bpp else 0
                line[i] = (line[i] + ((left + prev[i]) >> 1)) & 0xFF
        elif ftype == 4:  # Paeth
            for i in range(stride):
                a = line[i - bpp] if i >= bpp else 0
                b = prev[i]
                c = prev[i - bpp] if i >= bpp else 0
                line[i] = (line[i] + _paeth(a, b, c)) & 0xFF
        else:
            raise PngError(f"unknown filter type {ftype}")
        rows.append(line)
        prev = line
    # Normalise to 8-bit RGB rows.
    out = []
    if depth == 16:
        # High byte of each sample.
        rows = [r[0::2] for r in rows]
        # rows are now bytearray slices -> bytes; keep as bytes.
        rows = [bytes(r) for r in rows]
    if color == 2 and depth == 8:
        out = [bytes(r) for r in rows]
    elif color == 6:
        out = []
        for r in rows:
            ba = bytearray(r)
            del ba[3::4]  # drop alpha
            out.append(bytes(ba))
    elif color == 0:
        out = []
        for r in rows:
            rgb = bytearray(width * 3)
            rgb[0::3] = r
            rgb[1::3] = r
            rgb[2::3] = r
            out.append(bytes(rgb))
    elif color == 4:
        out = []
        for r in rows:
            grey = r[0::2]
            rgb = bytearray(width * 3)
            rgb[0::3] = grey
            rgb[1::3] = grey
            rgb[2::3] = grey
            out.append(bytes(rgb))
    elif color == 3:
        out = []
        for r in rows:
            rgb = bytearray(width * 3)
            for x in range(width):
                idx = r[x] * 3
                rgb[x * 3:x * 3 + 3] = plte[idx:idx + 3]
            out.append(bytes(rgb))
    else:  # colour type 2 at depth 16, post high-byte extraction
        out = [bytes(r) for r in rows]
    return width, height, out


# ----------------------------------------------------------------
# Tree dumps: the stage element's rect
# ----------------------------------------------------------------

def element_rect_from_tree(path, key):
    """(x, y, w, h) of the element keyed `key` in a --tree
    dump ({"elements": [{"key": ..., "rect": [x, y, w, h]}]}),
    or None if the dump or the element is absent."""
    try:
        with open(path) as fh:
            tree = json.load(fh)
    except Exception:
        return None
    for el in tree.get("elements", []):
        if el.get("key") == key:
            rect = el.get("rect") or []
            if len(rect) >= 4 and rect[2] > 0 and rect[3] > 0:
                return tuple(float(v) for v in rect[:4])
    return None


def stage_rect_from_tree(path):
    """(x, y, w, h) of the element keyed `stage` in a --tree
    dump — `element_rect_from_tree` at the stage's key."""
    return element_rect_from_tree(path, "stage")


# ----------------------------------------------------------------
# Pixel metrics (rows are RGB bytes, 3 bytes per pixel)
# ----------------------------------------------------------------

def _row_slice(row, x0, x1):
    return row[x0 * 3:x1 * 3]


def diff_count(rows_a, rows_b, x0, x1, y0, y1):
    """Pixels whose RGB differs between the two images inside
    the rectangle. Identical rows short-circuit, so a large
    agreeing region costs one bytes compare per row."""
    total = 0
    for y in range(y0, y1):
        a = _row_slice(rows_a[y], x0, x1)
        b = _row_slice(rows_b[y], x0, x1)
        if a == b:
            continue
        for i in range(0, len(a), 3):
            if a[i:i + 3] != b[i:i + 3]:
                total += 1
    return total


def _near_white(px):
    return px[0] >= NEAR_WHITE and px[1] >= NEAR_WHITE and px[2] >= NEAR_WHITE


def face_xor_count(rows_face, rows_other, x0, x1, y0, y1):
    """Pixels near-white in rows_face but NOT near-white at
    the same position in rows_other — the text-face census.
    In the anchor pair the figure (also near-white, 0xE8E8EC)
    sits at identical pixels in both images, so it cancels;
    only paint that exists under one anchor survives."""
    total = 0
    for y in range(y0, y1):
        a = _row_slice(rows_face[y], x0, x1)
        b = _row_slice(rows_other[y], x0, x1)
        if a == b:
            continue
        for i in range(0, len(a), 3):
            if _near_white(a[i:i + 3]) and not _near_white(b[i:i + 3]):
                total += 1
    return total


def face_xor_extent(rows_face, rows_other, x0, x1, y0, y1):
    """(count, min_x, max_x) of the face-XOR pixel set inside
    the rectangle — the same set `face_xor_count` counts,
    with its column extent (min_x/max_x are None when the
    set is empty). The extent is the speaker leg's ruler:
    a centred line's face spans its run width, so the
    prefix's widening shows up as extent, not just ink."""
    total = 0
    min_x = None
    max_x = None
    for y in range(y0, y1):
        a = _row_slice(rows_face[y], x0, x1)
        b = _row_slice(rows_other[y], x0, x1)
        if a == b:
            continue
        for i in range(0, len(a), 3):
            if _near_white(a[i:i + 3]) and not _near_white(b[i:i + 3]):
                total += 1
                x = x0 + i // 3
                if min_x is None or x < min_x:
                    min_x = x
                if max_x is None or x > max_x:
                    max_x = x
    return total, min_x, max_x


def near_white_extent(rows_img, x0, x1, y0, y1):
    """(count, min_x, max_x) of the near-white pixels of ONE
    image inside the rectangle (min_x/max_x are None when
    the set is empty). The lane leg's ink census: inside a
    cue block the only near-white paint is the label text
    (the fill is the theme's dark control colour), so the
    count is the label's presence and the extent its run."""
    total = 0
    min_x = None
    max_x = None
    for y in range(y0, y1):
        a = _row_slice(rows_img[y], x0, x1)
        for i in range(0, len(a), 3):
            if _near_white(a[i:i + 3]):
                total += 1
                x = x0 + i // 3
                if min_x is None or x < min_x:
                    min_x = x
                if max_x is None or x > max_x:
                    max_x = x
    return total, min_x, max_x


# ----------------------------------------------------------------
# Band geometry: stage rect -> drawn stage -> band rectangles
# ----------------------------------------------------------------

def drawn_stage(rect):
    """The drawn stage area (x0, y0, x1, y1, fit) inside an
    element rect, under stage_view.rs's letterboxed fit with
    the default camera (the demo has no camera keys, the
    scenes do not move the shell view): fit =
    min(el_w/STAGE_W, el_h/STAGE_H), drawn stage centred."""
    rx, ry, rw, rh = rect
    fit = min(rw / STAGE_W, rh / STAGE_H)
    dw, dh = STAGE_W * fit, STAGE_H * fit
    x0 = rx + (rw - dw) / 2.0
    y0 = ry + (rh - dh) / 2.0
    return x0, y0, x0 + dw, y0 + dh, fit


def band_rects(rect, img_w, img_h):
    """(bottom, top, mid) pixel rectangles (x0, x1, y0, y1),
    each BAND_UNITS of stage height against its edge, mid =
    the drawn stage between the bands. Rounded outward and
    clamped to the image."""
    dx0, dy0, dx1, dy1, fit = drawn_stage(rect)
    band_px = BAND_UNITS * fit

    def clamp_x(v0, v1):
        return max(0, int(math.floor(v0))), min(img_w, int(math.ceil(v1)))

    def clamp_y(v0, v1):
        return max(0, int(math.floor(v0))), min(img_h, int(math.ceil(v1)))

    x0, x1 = clamp_x(dx0, dx1)
    by0, by1 = clamp_y(dy1 - band_px, dy1)
    ty0, ty1 = clamp_y(dy0, dy0 + band_px)
    my0, my1 = clamp_y(dy0 + band_px, dy1 - band_px)
    return (x0, x1, by0, by1), (x0, x1, ty0, ty1), (x0, x1, my0, my1), fit


# ----------------------------------------------------------------
# The census
# ----------------------------------------------------------------

def run_census(directory):
    """Returns (lines, failures): the transcript and the list
    of failed assertion labels. Pure reporting — the caller
    owns exit codes and file output."""
    lines = []
    failures = []

    def note(text):
        lines.append(text)

    def check(ok, label, detail=""):
        note(("PASS  " if ok else "FAIL  ") + label +
             ("" if ok else f"  [{detail}]"))
        if not ok:
            failures.append(label)

    def report(text):
        note("  ..  " + text)

    # ---- input selection: full set preferred, crop set as
    # fallback; never mixed.
    def present(name):
        return os.path.isfile(os.path.join(directory, name))

    mode = None
    names = {}
    if all(present(n) for n in (SEEDED_BOTTOM_FULL, SEEDED_TOP_FULL, BASELINE_FULL)):
        mode = "full"
        names = {"bottom": SEEDED_BOTTOM_FULL, "top": SEEDED_TOP_FULL,
                 "base": BASELINE_FULL}
    elif all(present(n) for n in (SEEDED_BOTTOM_CROP, SEEDED_TOP_CROP, BASELINE_CROP)):
        mode = "crop"
        names = {"bottom": SEEDED_BOTTOM_CROP, "top": SEEDED_TOP_CROP,
                 "base": BASELINE_CROP}
    if mode is None:
        check(False, "snap inputs present (full set or crop set)",
              "need snap-subtitles-populated.png + snap-subtitles-top.png "
              "+ snap-default.png, or the three --crop stage equivalents; "
              f"dir={directory}")
        return lines, failures
    # The speaker leg's input belongs to the same mode's
    # set (never mixed, like the rest); its absence is a
    # failed proof, named — Wave S (S2).
    speaker_name = SEEDED_SPEAKER_FULL if mode == "full" else SEEDED_SPEAKER_CROP
    if not present(speaker_name):
        check(False, "speaker snap input present",
              f"need {speaker_name} (scene subtitles-speaker) alongside "
              f"the {mode} set; dir={directory}")
        return lines, failures
    names["speaker"] = speaker_name
    report(f"input mode: {mode} ({names['bottom']}, {names['top']}, "
           f"{names['base']}, {names['speaker']})")

    images = {}
    for tag, name in names.items():
        try:
            images[tag] = read_png(os.path.join(directory, name))
        except Exception as e:
            check(False, f"{name} decodes as a PNG", repr(e))
            return lines, failures
    (wb, hb, rows_bottom) = images["bottom"]
    (wt, ht, rows_top) = images["top"]
    (wd, hd, rows_base) = images["base"]
    (ws, hs, rows_speaker) = images["speaker"]
    check((wb, hb) == (wt, ht) == (wd, hd) == (ws, hs),
          "all four snaps share one image size",
          f"bottom={wb}x{hb} top={wt}x{ht} base={wd}x{hd} speaker={ws}x{hs}")
    if failures:
        return lines, failures
    img_w, img_h = wb, hb

    # ---- the stage rect: from the frame's own tree dump in
    # full mode; the image itself in crop mode.
    if mode == "full":
        rect = stage_rect_from_tree(os.path.join(directory, TREE_SEEDED))
        rect_base = stage_rect_from_tree(os.path.join(directory, TREE_BASELINE))
        if rect is not None and rect_base is not None:
            drift = max(abs(a - b) for a, b in zip(rect, rect_base))
            report(f"stage rect (seeded tree) {rect}; (default tree) "
                   f"{rect_base}; max drift {drift:.1f} px")
            check(drift <= 1.0, "stage rect agrees across the two tree dumps",
                  f"drift={drift:.1f}")
        elif rect is None:
            rect = rect_base
        check(rect is not None, "stage rect located in a tree dump",
              f"no element keyed 'stage' with a rect in {TREE_SEEDED} "
              f"or {TREE_BASELINE}")
        if rect is None:
            return lines, failures
    else:
        rect = (0.0, 0.0, float(img_w), float(img_h))
        report("crop mode: the stage rect is the crop itself")
        tree_rect = stage_rect_from_tree(os.path.join(directory, TREE_SEEDED))
        if tree_rect is not None:
            report(f"tree stage rect {tree_rect} vs crop {img_w}x{img_h} "
                   "(cross-check only)")

    bottom_r, top_r, mid_r, fit = band_rects(rect, img_w, img_h)
    _dx0, dy0, _dx1, dy1, _f = drawn_stage(rect)
    drawn_h = dy1 - dy0
    report(f"stage rect {rect}; fit {fit:.3f} px/unit; drawn height "
           f"{drawn_h:.0f} px; bottom band rows {bottom_r[2]}..{bottom_r[3]}, "
           f"top band rows {top_r[2]}..{top_r[3]}")
    check(drawn_h >= MIN_STAGE_H_PX, "drawn stage is tall enough to prove a band",
          f"drawn height {drawn_h:.0f} px < {MIN_STAGE_H_PX}")

    # ---- the anchor pair (load-bearing).
    d_bottom = diff_count(rows_bottom, rows_top, *bottom_r)
    d_top = diff_count(rows_bottom, rows_top, *top_r)
    d_mid = diff_count(rows_bottom, rows_top, *mid_r)
    f_bottom = face_xor_count(rows_bottom, rows_top, *bottom_r)
    f_top = face_xor_count(rows_top, rows_bottom, *top_r)
    report(f"pair populated-vs-top: diff bottom band {d_bottom}, "
           f"diff top band {d_top}, diff mid-stage {d_mid}")
    report(f"pair text face: bottom-only near-white {f_bottom}, "
           f"top-only near-white {f_top}")
    check(d_bottom >= DIFF_FLOOR,
          f"bottom band changes when the anchor flips to Top (>= {DIFF_FLOOR})",
          f"diff={d_bottom}")
    check(d_top >= DIFF_FLOOR,
          f"top band changes when the anchor flips to Top (>= {DIFF_FLOOR})",
          f"diff={d_top}")
    check(f_bottom >= FACE_FLOOR,
          f"text face present in the bottom band under Bottom (>= {FACE_FLOOR})",
          f"face={f_bottom}")
    check(f_top >= FACE_FLOOR,
          f"text face present in the top band under Top (>= {FACE_FLOOR})",
          f"face={f_top}")
    check(d_mid == 0,
          "stage between the bands is pixel-identical across the anchor pair",
          f"diff={d_mid}")

    # ---- the baseline leg (supporting).
    d_base_bottom = diff_count(rows_bottom, rows_base, *bottom_r)
    f_base_bottom = face_xor_count(rows_bottom, rows_base, *bottom_r)
    report(f"baseline populated-vs-default: diff bottom band "
           f"{d_base_bottom} (pose-contaminated by design), text face "
           f"bottom {f_base_bottom} (reported, not gated: the demo "
          "figure is near-white and moves between frames 0 and 2)")
    check(d_base_bottom >= DIFF_FLOOR,
          f"seeded bottom band differs from the no-cue default (>= {DIFF_FLOOR})",
          f"diff={d_base_bottom}")

    # ---- the speaker leg (Wave S, S2): the credited cue's
    # prefix, proven as line widening against the
    # uncredited control scene (same park, same anchor,
    # only the cue differs).
    d_sp_bottom = diff_count(rows_speaker, rows_bottom, *bottom_r)
    d_sp_top = diff_count(rows_speaker, rows_bottom, *top_r)
    d_sp_mid = diff_count(rows_speaker, rows_bottom, *mid_r)
    f_speaker = face_xor_count(rows_speaker, rows_base, *bottom_r)
    face_n, face_min, face_max = face_xor_extent(
        rows_speaker, rows_base, *bottom_r)
    span_units = ((face_max - face_min + 1) / fit) if face_n else 0.0
    report(f"speaker pair speaker-vs-populated: diff bottom band "
           f"{d_sp_bottom}, diff top band {d_sp_top}, diff mid-stage "
           f"{d_sp_mid}")
    report(f"speaker face: bottom-band near-white {f_speaker} "
           f"(vs default); face extent {face_n} px over columns "
           f"{face_min}..{face_max} = {span_units:.0f} stage units "
           f"(bare text runs {SPEAKER_BARE_RUN_UNITS:.0f}, credited "
           f"line {SPEAKER_CREDITED_RUN_UNITS:.0f})")
    check(f_speaker >= FACE_FLOOR,
          f"credited cue's text face present in the bottom band "
          f"(>= {FACE_FLOOR})",
          f"face={f_speaker}")
    check(span_units >= SPEAKER_SPAN_GATE_UNITS,
          f"speaker prefix widens the credited line past the bare-text "
          f"span (>= {SPEAKER_SPAN_GATE_UNITS:.0f} units; bare run "
          f"{SPEAKER_BARE_RUN_UNITS:.0f})",
          f"span={span_units:.0f} units")
    check(d_sp_bottom >= DIFF_FLOOR,
          f"credited bottom band differs from the uncredited control "
          f"(>= {DIFF_FLOOR})",
          f"diff={d_sp_bottom}")
    check(d_sp_top == 0,
          "top band is pixel-identical across the speaker pair",
          f"diff={d_sp_top}")
    check(d_sp_mid == 0,
          "stage between the bands is pixel-identical across the speaker pair",
          f"diff={d_sp_mid}")

    # ---- the lane leg (Wave T, T11): the credited cue's
    # lane block paints its prefixed label, in the same
    # block at the same place as the uncredited control's.
    # Full-window inputs only — the stage crops do not
    # contain the timeline strip (see the module docstring).
    if mode == "crop":
        report("lane speaker leg: not run in crop mode — the "
               "stage crops do not contain the subtitle lane; "
               "the full-window set carries this leg")
    else:
        block_pop = element_rect_from_tree(
            os.path.join(directory, TREE_SEEDED), LANE_BLOCK_KEY)
        block_spk = element_rect_from_tree(
            os.path.join(directory, TREE_SPEAKER), LANE_BLOCK_KEY)
        check(block_pop is not None and block_spk is not None,
              "lane block rect located in both subtitle tree dumps",
              f"need an element keyed '{LANE_BLOCK_KEY}' with a rect "
              f"in {TREE_SEEDED} and {TREE_SPEAKER}; "
              f"populated={'missing' if block_pop is None else 'found'}, "
              f"speaker={'missing' if block_spk is None else 'found'}")
        if block_pop is not None and block_spk is not None:
            drift = max(abs(a - b) for a, b in zip(block_pop, block_spk))
            report(f"lane block rect (populated tree) {block_pop}; "
                   f"(speaker tree) {block_spk}; max drift {drift:.1f} px")
            check(drift <= LANE_RECT_DRIFT_MAX,
                  "lane block geometry is identical across the speaker pair",
                  f"drift={drift:.1f}")
            # The populated dump's rect is the canonical
            # region (the drift check above bounds how far
            # the speaker block can sit from it); rounded
            # outward and clamped to the image, the band
            # idiom.
            bx0 = max(0, int(math.floor(block_pop[0])))
            by0 = max(0, int(math.floor(block_pop[1])))
            bx1 = min(img_w, int(math.ceil(block_pop[0] + block_pop[2])))
            by1 = min(img_h, int(math.ceil(block_pop[1] + block_pop[3])))
            lane_diff = diff_count(rows_speaker, rows_bottom,
                                   bx0, bx1, by0, by1)
            ink_spk, ink_spk_min, ink_spk_max = near_white_extent(
                rows_speaker, bx0, bx1, by0, by1)
            ink_pop, ink_pop_min, ink_pop_max = near_white_extent(
                rows_bottom, bx0, bx1, by0, by1)
            report(f"lane pair speaker-vs-populated: diff inside the "
                   f"block {lane_diff}; label ink speaker {ink_spk} px "
                   f"over columns {ink_spk_min}..{ink_spk_max}, "
                   f"populated {ink_pop} px over columns "
                   f"{ink_pop_min}..{ink_pop_max} (extents reported, "
                   f"not gated: both labels saturate the block's "
                   f"text run — see LANE FLOORS in the docstring)")
            check(lane_diff >= LANE_DIFF_FLOOR,
                  f"lane label differs from the uncredited block's "
                  f"label (>= {LANE_DIFF_FLOOR})",
                  f"diff={lane_diff}")
            check(ink_spk >= LANE_INK_FLOOR,
                  f"credited lane block carries its label ink "
                  f"(>= {LANE_INK_FLOOR})",
                  f"ink={ink_spk}")

    return lines, failures


def finish(lines, failures, directory):
    verdict = ("SUBTITLE CENSUS: PASS" if not failures else
               f"SUBTITLE CENSUS: FAIL ({len(failures)} assertion(s) failed)")
    lines.append(verdict)
    text = "\n".join(lines) + "\n"
    print(text, end="", flush=True)
    if directory is not None:
        os.makedirs(directory, exist_ok=True)
        with open(os.path.join(directory, "subtitle-census.txt"), "w") as fh:
            fh.write(text)
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary:
        with open(summary, "a") as fh:
            fh.write("\n### Subtitle census (P33)\n\n```\n" + text + "```\n")
    return 0 if not failures else 1


# ----------------------------------------------------------------
# Self-test (synthetic; no harness outputs needed)
# ----------------------------------------------------------------

def _write_png(path, width, height, rows, color=6, filters=None):
    """Minimal PNG writer for the self-test. rows: RGB(A)
    bytes per row at the target colour type. filters: per-row
    filter types to APPLY (the decoder must undo them)."""
    bpp = 4 if color == 6 else 3
    raw = bytearray()
    prev = bytes(width * bpp)
    for y in range(height):
        ftype = (filters or {}).get(y, 0)
        line = rows[y]
        if ftype == 0:
            enc = bytes(line)
        elif ftype == 1:  # Sub
            enc = bytes((line[i] - (line[i - bpp] if i >= bpp else 0)) & 0xFF
                        for i in range(len(line)))
        elif ftype == 2:  # Up
            enc = bytes((line[i] - prev[i]) & 0xFF for i in range(len(line)))
        else:
            raise ValueError(f"self-test writer: filter {ftype} unimplemented")
        raw.append(ftype)
        raw.extend(enc)
        prev = bytes(line)

    def chunk(ctype, payload):
        out = struct.pack(">I", len(payload)) + ctype + payload
        out += struct.pack(">I", zlib.crc32(ctype + payload) & 0xFFFFFFFF)
        return out

    ihdr = struct.pack(">IIBBBBB", width, height, 8, color, 0, 0, 0)
    png = (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) +
           chunk(b"IDAT", zlib.compress(bytes(raw))) + chunk(b"IEND", b""))
    with open(path, "wb") as fh:
        fh.write(png)


def _self_test():
    import tempfile

    failures = []

    def expect(ok, label):
        print(("PASS  " if ok else "FAIL  ") + label)
        if not ok:
            failures.append(label)

    W, H = 720, 450
    RECT = (60.0, 45.0, 600.0, 360.0)  # stage element in the fake window
    BG = (0x23, 0x28, 0x30)            # demo background 0x232830
    WHITE = (0xFF, 0xFF, 0xFF)
    FIG = (0xE8, 0xE8, 0xEC)           # the near-white demo figure

    dx0, dy0, dx1, dy1, fit = drawn_stage(RECT)
    band = BAND_UNITS * fit

    def blank(fill=BG):
        return [bytes(fill) * W for _ in range(H)]

    def paint(rows, x0, x1, y0, y1, color):
        seg = bytes(color) * (x1 - x0)
        for y in range(y0, y1):
            row = bytearray(rows[y])
            row[x0 * 3:x1 * 3] = seg
            rows[y] = bytes(row)

    def build(bottom_cue, top_cue, stray_mid=False):
        rows = blank()
        # A near-white "figure" mid-stage, identical in every
        # image: the pair's mid-stage and face XOR must cancel it.
        paint(rows, 330, 390, 180, 300, FIG)
        if bottom_cue:
            paint(rows, 220, 500, int(dy1 - band) + 6, int(dy1) - 2, WHITE)
        if top_cue:
            paint(rows, 220, 500, int(dy0) + 2, int(dy0 + band) - 6, WHITE)
        if stray_mid:
            # Inside the drawn stage's mid region (drawn x
            # starts at 120 in this fixture): the pair's
            # mid-stage identity gate must catch exactly this.
            paint(rows, 150, 151, 200, 201, WHITE)
        return rows

    def to_rgba(rows):
        out = []
        for r in rows:
            ba = bytearray(W * 4)
            ba[0::4] = r[0::3]
            ba[1::4] = r[1::3]
            ba[2::4] = r[2::3]
            ba[3::4] = b"\xFF" * W
            out.append(bytes(ba))
        return out

    # The lane block's fixture rect: below the drawn stage
    # (which ends at y=405 in this fixture), inside the
    # window — where the timeline strip sits in the real
    # snaps. Both subtitle trees carry it at the same
    # rect (fact 34); the default tree has no lane.
    LANE_BLOCK = [120, 416, 166, 18]

    def paint_lane_label(rows, x1):
        # Label ink inside the block: a white bar inset
        # 4 px from the block's left edge (the lane's
        # px(4.0)), vertically centred in the 18-tall
        # block. The block's fill is not modelled — the
        # leg reads only the rect, the near-white ink and
        # the pair diff, and the fixture models exactly
        # those.
        paint(rows, 124, x1, 420, 430, WHITE)

    with tempfile.TemporaryDirectory() as td:
        tree = {"elements": [
            {"path": "/root", "key": "root", "rect": [0, 0, W, H]},
            {"path": "/root/stage", "key": "stage",
             "rect": [RECT[0], RECT[1], RECT[2], RECT[3]]},
        ]}
        lane_tree = {"elements": tree["elements"] + [
            {"path": "/root/timeline/subtitle-lane/subtitle-cue-0",
             "key": "subtitle-cue-0", "rect": list(LANE_BLOCK)},
        ]}
        with open(os.path.join(td, TREE_SEEDED), "w") as fh:
            json.dump(lane_tree, fh)
        with open(os.path.join(td, TREE_SPEAKER), "w") as fh:
            json.dump(lane_tree, fh)
        with open(os.path.join(td, TREE_BASELINE), "w") as fh:
            json.dump(tree, fh)

        bottom = build(True, False)
        paint_lane_label(bottom, 240)  # the uncredited label's run
        top = build(False, True)
        base = build(False, False)
        # Seeded images as RGBA with Sub/Up filters mixed in;
        # baseline as plain RGB — the decoder must handle both.
        _write_png(os.path.join(td, SEEDED_BOTTOM_FULL), W, H,
                   to_rgba(bottom), color=6,
                   filters={0: 1, 1: 2, 5: 1, 100: 2})
        _write_png(os.path.join(td, SEEDED_TOP_FULL), W, H,
                   to_rgba(top), color=6, filters={3: 2})
        _write_png(os.path.join(td, BASELINE_FULL), W, H,
                   base, color=2)

        # The speaker scene's snap: a bottom-band face bar
        # centred on the drawn stage's centre column (360),
        # spanning the CREDITED extent (444 units at fit
        # 0.5 = 222 px, less the last glyph's unlit edge
        # column: 220 px painted). The bare text alone
        # would span 172 px (274..446) — the negative below.
        def speaker_rows(bar_x0, bar_x1):
            rows = build(False, False)
            paint(rows, bar_x0, bar_x1,
                  int(dy1 - band) + 6, int(dy1) - 2, WHITE)
            return rows

        def honest_speaker():
            rows = speaker_rows(250, 470)
            paint_lane_label(rows, 282)  # the credited label's run
            return rows

        _write_png(os.path.join(td, SEEDED_SPEAKER_FULL), W, H,
                   to_rgba(honest_speaker()), color=6)

        # Decoder round-trip.
        w2, h2, dec = read_png(os.path.join(td, SEEDED_BOTTOM_FULL))
        expect((w2, h2) == (W, H) and dec == bottom,
               "decoder: RGBA + Sub/Up filters round-trips")
        w3, h3, dec3 = read_png(os.path.join(td, BASELINE_FULL))
        expect((w3, h3) == (W, H) and dec3 == base,
               "decoder: RGB filter-0 round-trips")

        # The honest set passes.
        _lines, fails = run_census(td)
        expect(not fails, f"census passes on the synthetic honest set "
                          f"(failures: {fails})")

        # A speaker snap whose band face spans only the
        # BARE text's width (the prefix never painted)
        # must fail the widening gate, and only that gate.
        _write_png(os.path.join(td, SEEDED_SPEAKER_FULL), W, H,
                   to_rgba(speaker_rows(274, 446)), color=6)
        _lines, fails = run_census(td)
        expect(any("speaker prefix" in f for f in fails),
               "census fails when the credited line spans only the bare text")

        # A missing speaker snap is a failed proof, named.
        os.remove(os.path.join(td, SEEDED_SPEAKER_FULL))
        _lines, fails = run_census(td)
        expect(any("speaker snap input" in f for f in fails),
               "census fails when the speaker snap is missing")

        # Restore the honest speaker snap for the
        # remaining negatives.
        _write_png(os.path.join(td, SEEDED_SPEAKER_FULL), W, H,
                   to_rgba(honest_speaker()), color=6)

        # A speaker snap whose lane block shows the
        # UNCREDITED label (same ink run as the control —
        # the lane never composed the credit) must fail
        # the lane label gate, while the block geometry
        # gate stays green: the block is right, the label
        # is wrong.
        spk = speaker_rows(250, 470)
        paint_lane_label(spk, 240)
        _write_png(os.path.join(td, SEEDED_SPEAKER_FULL), W, H,
                   to_rgba(spk), color=6)
        _lines, fails = run_census(td)
        expect(any("lane label differs" in f for f in fails),
               "census fails when the credited lane label matches "
               "the uncredited one")
        expect(not any("lane block geometry" in f for f in fails),
               "lane geometry gate stays green when only the label is wrong")

        # A missing speaker tree dump is a failed lane
        # proof, named.
        os.remove(os.path.join(td, TREE_SPEAKER))
        _lines, fails = run_census(td)
        expect(any("lane block rect" in f for f in fails),
               "census fails when the speaker tree dump is missing")

        # Restore the tree and the honest speaker snap
        # for the remaining negatives (they rewrite the
        # Top snap).
        with open(os.path.join(td, TREE_SPEAKER), "w") as fh:
            json.dump(lane_tree, fh)
        _write_png(os.path.join(td, SEEDED_SPEAKER_FULL), W, H,
                   to_rgba(honest_speaker()), color=6)

        # The anchor flip missing its top cue must fail.
        _write_png(os.path.join(td, SEEDED_TOP_FULL), W, H,
                   to_rgba(build(False, False)), color=6)
        _lines, fails = run_census(td)
        expect(bool(fails), "census fails when the Top scene paints no top cue")

        # A stray mid-stage pixel in one of the pair must fail.
        _write_png(os.path.join(td, SEEDED_TOP_FULL), W, H,
                   to_rgba(build(False, True, stray_mid=True)), color=6)
        _lines, fails = run_census(td)
        expect(any("pixel-identical" in f for f in fails),
               "census fails on a mid-stage difference in the pair")

    print("SELF-TEST: " + ("PASS" if not failures else
                           f"FAIL ({len(failures)})"))
    return 0 if not failures else 1


def main(argv):
    if "--self-test" in argv:
        return _self_test()
    directory = argv[1] if len(argv) > 1 else "headless-out"
    lines, failures = run_census(directory)
    return finish(lines, failures, directory)


if __name__ == "__main__":
    sys.exit(main(sys.argv))
