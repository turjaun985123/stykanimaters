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
  * CAPTURE SCALE (the integrator's fix after headless runs
    42759195547 and 42764348922 both concluded green while
    this census's transcript failed): tree rects are in
    window POINTS; the snaps are in capture PIXELS, and the
    harness captures at a device scale the tree dump itself
    declares (top-level "scale", 2.0 in the CI runs — snaps
    2880x1800 against a 1440x900-point window). Every rect
    read from a dump is converted ONCE, by the ratio
    snap_width / window_width (the dump's accessibility
    bounds; its "scale" field is the fallback), before any
    band or block is sampled. Sampling point coordinates
    in the pixel image reads the stage's upper-left
    quadrant and every leg reports zero — the failure
    signature of both runs above. The stage-agreement gate
    covers the SEEDED family (populated/speaker: same
    project, same lane-visible geometry); the default
    scene's stage element is taller (it carries no subtitle
    lane) and its rect is reported, never gated.
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

WAVE 2 EXPANSION (W2-79) — four legs over evidence the
original census never read. All four are FAIL-SOFT IN
THE INSTRUMENT'S OWN IDIOM: a leg whose evidence is
absent reports "not run" with the reason and gates
nothing (the lane leg's crop-mode precedent), and a
leg only fails when evidence is present AND contradicts
itself or the seeded plants. On artifacts predating
the Wave 2 surfaces (the Wave AC closing artifact is
the regression baseline) every pre-existing check's
verdict is unchanged and the new legs report not-run
or pass on the evidence that already exists (the
seeded lane labels, below).

  * BAND CLAIMS (W2-45): the stage subtitle pass now
    partitions each band between the bitmap prim stream
    and a shaped-tile element layer, per band, on the
    style's font claim (`subtitle_band_shaped`). The
    tree dumps carry the partition: a claimed band
    leaves a `stage-subtitle-line-{i}` element (under a
    `stage-subtitle-layer` container) in the scene's
    tree. This leg reads the three seeded trees and
    reports each scene's claim; when any tree carries
    the layer, the claims must AGREE across the seeded
    scenes (same default style, all Latin single-line
    cues — a claim is a property of the machine's fonts
    and the style, never of the anchor or the credit).
    The claim also recalibrates the speaker leg: the
    SPAN GATE above is a BITMAP derivation (4-unit
    cells). Under the shaped renderer the painted span
    is font metrics, and the same gate would fail an
    honest shaped line — so when the speaker band is
    claimed, the span gate is replaced by a
    same-renderer control: the credited line must span
    at least SHAPED_SPAN_RATIO x the populated cue's
    painted span, measured this run through the same
    face XOR (25 uncredited chars; the speaker text
    uncredited would run ~22/25 of it, credited ~28/25,
    so the ratio discriminates the prefix's presence
    with margin on both sides; the integrator ratchets
    the factor from the first shaped CI run, the
    standing floor discipline). The composition itself
    is then proven at the data level by the label leg
    below. DIFF_FLOOR / FACE_FLOOR stay in force under
    both renderers: they are presence floors with an
    order of magnitude of headroom, but their analytic
    census was bitmap ink — the first shaped run's
    measured values are the ratchet input, named here
    so a future floor failure is attributed to the
    renderer change before anything else.
  * LABEL + DIALOGUE LEG (W2-17, W2-12): the tree
    dumps carry the lane blocks' label TEXT (a text
    element under each block), which the pixel legs
    never read. The seeded labels are gated
    byte-for-byte against the scenes' known plants —
    populated: the raw cue text; speaker: the credited
    composition `NOVA: ` + text — pinning
    `block_label`'s composition at the data level (the
    pixel lane leg proves the labels DIFFER and carry
    ink; this proves WHAT they say). Where a tree
    carries W2-12's dialogue lane, each
    `dialogue-line-{i}` must sit exactly on its
    `subtitle-cue-{i}` block (line_rect IS cue_rect —
    the delegation, gated at the lane leg's own 1.0 pt
    tolerance) and read exactly as it (line_label IS
    block_label). Each `dialogue-chapter-{j}` segment
    must carry its title text; each line's geometric
    chapter (the first segment whose x-span holds the
    line's left edge — `chapter_index_at`'s
    first-in-episode-order rule read off the shared
    span algebra) is REPORTED, not gated. When a band
    of >= 3 segments lets one fill be identified as
    the odd one out, the odd segment must be the
    playhead's chapter: the segment first containing
    the parked frame's chip x (the parked frame is
    read from the lane header's `Frame n / m` text) —
    header, chip geometry and segment fill are three
    independent dump facts that must agree.
  * SRT ANNOUNCEMENT LEG (W2-17): SRT exports/imports
    announce their dropped speaker credits in the
    panel status sentence (`... speaker credit(s) not
    written (SRT has no speaker syntax).` on export,
    `... dropped ...` on import), and the MCP SRT
    export reply carries `speakers_dropped`, present
    only when above zero. No headless surface performs
    an SRT action today, so in current artifacts this
    leg finds nothing and reports not run — but it
    scans every tree dump's texts plus mcp-probe.txt
    when present, so the day a scene or the probe
    exercises SRT the announcement becomes gated
    evidence: an announced count is at least 1 (both
    surfaces only speak when credits were dropped);
    an EXPORT announcement in a seeded tree must
    equal that scene's planted credited-cue count
    (export is read-only — the dump's project is the
    exported project); the MCP field must be above
    zero whenever present. The import sentence's
    count is the REPLACED list's loss, whose ground
    truth is not in the post-import dump, so it is
    reported, not equated.
  * AUDIO-TREE LEG (W2-15, W2-16): tree-audio-open.json
    (an optional input — the Audio workspace's tree)
    carries two panel surfaces. The title-card
    section (W2-15): its chapter rows
    (`title-card-row-{i}`, labels `{n}. {title} —
    frames {a}–{b}`) must be numbered in episode
    order with increasing ranges, its preview caption
    IS the expectation line — reported verbatim, and
    in its "Shows for ... from frame ..." form the
    stated frame and the previewed title must match
    the row they belong to; when the dialogue band
    was also read, the two surfaces' chapter titles
    must agree title for title. The timing assistant
    (W2-16): its proposal lines and the selected
    cue's CPS readout are reported verbatim — CPS is
    REPORTED, NEVER JUDGED (decision D-9: no
    threshold verdict exists to gate on) — and the
    one structural fact is gated: the
    `{count} suggestion(s)` line must equal the
    number of proposal rows the block shows.

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

# Wave 2 expansion (W2-79; derivations in the module
# docstring's WAVE 2 EXPANSION section). Optional inputs:
# the top scene's tree (read for the band-claim census),
# the Audio workspace's tree (title-card + timing
# surfaces) and the MCP probe transcript (the SRT
# announcement scan). All are fail-soft: absent inputs
# report their leg not run.
TREE_TOP = "tree-subtitles-top.json"
TREE_AUDIO = "tree-audio-open.json"
MCP_PROBE_TXT = "mcp-probe.txt"

# The seeded scenes' planted lane labels (scenes.rs's
# plant_subtitle_cue_as calls): the populated cue is
# uncredited, so its block label is the raw text; the
# speaker cue is credited, so its label is the
# composition `NOVA: ` + text (block_label's shape).
SEEDED_LABEL_POPULATED = "We walk home before dark."
SEEDED_LABEL_SPEAKER = "NOVA: The light is still on."
# The seeded scenes' planted credited-cue counts — the
# SRT export announcement's ground truth per scene
# (export does not mutate the project it reads).
SEEDED_CREDITED = {TREE_SEEDED: 0, TREE_SPEAKER: 1}

# Shaped-route span control: the credited line must
# span at least this fraction of the populated cue's
# painted span in the same run (same renderer, same
# font size). Credited ~28/25 = 1.12, uncredited
# ~22/25 = 0.88; 0.95 sits between with margin.
SHAPED_SPAN_RATIO = 0.95

# Shaped-claim element keys (stage_text.rs, W2-45):
# the layer container is a counted literal, the line
# elements a variable family indexed by band.
SHAPED_LAYER_KEY = "stage-subtitle-layer"
SHAPED_LINE_STEM = "stage-subtitle-line-"

# Dialogue lane element keys (dialogue_lane.rs, W2-12).
DIALOGUE_LANE_KEY = "dialogue-lane"
DIALOGUE_LINE_STEM = "dialogue-line-"
DIALOGUE_CHAPTER_STEM = "dialogue-chapter-"

# Title-card section element keys (title_card_panel.rs,
# W2-15) and the chapter row label's shape:
# `{n}. {title} — frames {start+1}–{end}`.
TITLE_CARD_SECTION_KEY = "title-card-section"
TITLE_CARD_ROW_STEM = "title-card-row-"
TITLE_CARD_PREVIEW_KEY = "title-card-preview"

import re

SRT_EXPORT_ANNOUNCE_RE = re.compile(
    r"(\d+) speaker credit\(s\) not written "
    r"\(SRT has no speaker syntax\)")
SRT_IMPORT_ANNOUNCE_RE = re.compile(
    r"(\d+) speaker credit\(s\) dropped "
    r"\(SRT has no speaker syntax\)")
MCP_SPEAKERS_DROPPED_RE = re.compile(
    r'"speakers_dropped"\s*:\s*(\d+)')
TITLE_CARD_ROW_RE = re.compile(
    r"^(\d+)\.\s+(.*)\s+—\s+frames\s+(\d+)–(\d+)$")
TITLE_CARD_SHOWS_RE = re.compile(
    r"^Shows for ([\d.]+) s from frame (\d+)$")
TITLE_CARD_EYEBROW_RE = re.compile(r"^CHAPTER (\d+)$")
PARKED_FRAME_RE = re.compile(r"^Frame (\d+) / (\d+)$")
ASSIST_PROPOSAL_RE = re.compile(r"^Cue \d+ · .*chars/s$")
ASSIST_COUNT_RE = re.compile(r"^(\d+) suggestion\(s\)$")


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


def tree_scale(path, img_w):
    """The capture scale converting tree-dump POINTS to snap
    PIXELS: the snap's pixel width over the dump's window
    width (the accessibility root bounds — the measurement,
    not a claim). The dump's own top-level "scale" field is
    the fallback when it carries no bounds; 1.0 when it
    carries neither. Returns (scale, source) so the
    transcript names the contract it used."""
    try:
        with open(path) as fh:
            tree = json.load(fh)
    except Exception:
        return 1.0, "default (dump unreadable)"
    bounds = (tree.get("accessibility") or {}).get("bounds") or []
    if img_w and len(bounds) >= 4 and bounds[2] > 0:
        return img_w / float(bounds[2]), "snap width / window bounds"
    declared = tree.get("scale")
    if isinstance(declared, (int, float)) and declared > 0:
        return float(declared), "the dump's declared scale"
    return 1.0, "default (no bounds or scale in the dump)"


# ----------------------------------------------------------------
# Tree dumps: element and text readers (Wave 2 expansion)
# ----------------------------------------------------------------

def load_tree(path):
    """The parsed tree dump at `path`, or None when it is
    absent or unreadable. The expansion legs treat every
    dump beyond the original input set as optional, so a
    missing dump is a reported not-run, never an error."""
    try:
        with open(path) as fh:
            return json.load(fh)
    except Exception:
        return None


def tree_elements(tree):
    return (tree or {}).get("elements", []) or []


def elements_with_key(tree, key):
    """All elements carrying exactly `key` (a dump may in
    principle repeat a key; the census reads the first
    for geometry and all for families)."""
    return [el for el in tree_elements(tree) if el.get("key") == key]


def element_with_key(tree, key):
    els = elements_with_key(tree, key)
    return els[0] if els else None


def keyed_family(tree, stem):
    """{index: element} for the variable key family
    `stem{i}` (`dialogue-line-0`, `title-card-row-2`, ...)
    — the function-carried key class the lane block's
    `subtitle-cue-{i}` belongs to."""
    out = {}
    pat = re.compile("^" + re.escape(stem) + r"(\d+)$")
    for el in tree_elements(tree):
        m = pat.match(el.get("key") or "")
        if m:
            out[int(m.group(1))] = el
    return out


def element_rect(el):
    rect = (el or {}).get("rect") or []
    if len(rect) >= 4 and rect[2] > 0 and rect[3] > 0:
        return tuple(float(v) for v in rect[:4])
    return None


def descendant_text(tree, el):
    """The text an element presents: its own `text` when
    it carries one, else the first text-bearing
    descendant's, in dump order. A lane block's label is
    a child text element under the block's path; a
    chapter segment's title likewise."""
    if el is None:
        return None
    if el.get("text"):
        return el["text"]
    path = el.get("path") or ""
    if not path:
        return None
    prefix = path + "/"
    for other in tree_elements(tree):
        if (other.get("path") or "").startswith(prefix) \
                and other.get("text"):
            return other["text"]
    return None


def descendant_texts(tree, el):
    """Every text under an element (its own first), in
    dump order — the preview plate's lines (eyebrow,
    title, caption) are sibling text elements."""
    out = []
    if el is None:
        return out
    if el.get("text"):
        out.append(el["text"])
    path = el.get("path") or ""
    if path:
        prefix = path + "/"
        for other in tree_elements(tree):
            if (other.get("path") or "").startswith(prefix) \
                    and other.get("text"):
                out.append(other["text"])
    return out


def all_texts(tree):
    """Every text string a dump carries — the SRT
    announcement scan's haystack (the panel status
    sentence, when a scene performs an SRT action,
    lands in the dump as a text element)."""
    return [el["text"] for el in tree_elements(tree) if el.get("text")]


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
    # full mode; the image itself in crop mode. Full-mode
    # rects are window POINTS; the snaps are capture PIXELS
    # (see CAPTURE SCALE in the module docstring) — the
    # seeded rect is converted once, here, and every band
    # and block below samples pixel space.
    snap_scale = 1.0
    if mode == "full":
        rect = stage_rect_from_tree(os.path.join(directory, TREE_SEEDED))
        rect_base = stage_rect_from_tree(os.path.join(directory, TREE_BASELINE))
        rect_speaker = stage_rect_from_tree(os.path.join(directory, TREE_SPEAKER))
        snap_scale, scale_source = tree_scale(
            os.path.join(directory, TREE_SEEDED), img_w)
        report(f"capture scale {snap_scale:.3f} (from {scale_source}): "
               f"tree rects are window points, snap pixels are "
               f"{snap_scale:.3f}x points")
        if rect is not None and rect_speaker is not None:
            drift = max(abs(a - b) for a, b in zip(rect, rect_speaker))
            report(f"stage rect (seeded tree) {rect}; (speaker tree) "
                   f"{rect_speaker}; max drift {drift:.1f} pt")
            check(drift <= 1.0,
                  "stage rect agrees across the seeded tree dumps",
                  f"drift={drift:.1f}")
        if rect_base is not None:
            report(f"stage rect (default tree) {rect_base} — reported "
                   f"only: the default scene carries no subtitle lane, "
                   f"so its stage element is not the seeded geometry")
        if rect is None:
            rect = rect_base
        check(rect is not None, "stage rect located in a tree dump",
              f"no element keyed 'stage' with a rect in {TREE_SEEDED} "
              f"or {TREE_BASELINE}")
        if rect is None:
            return lines, failures
        rect = tuple(v * snap_scale for v in rect)
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
    report(f"stage rect (snap pixels) {rect}; fit {fit:.3f} px/unit; drawn height "
           f"{drawn_h:.0f} px; bottom band rows {bottom_r[2]}..{bottom_r[3]}, "
           f"top band rows {top_r[2]}..{top_r[3]}")
    check(drawn_h >= MIN_STAGE_H_PX, "drawn stage is tall enough to prove a band",
          f"drawn height {drawn_h:.0f} px < {MIN_STAGE_H_PX}")

    # ---- the band claims (Wave 2 expansion, W2-45):
    # which renderer painted each seeded scene's band is
    # recorded in the scene's own tree dump (a claimed
    # band leaves shaped-tile elements there). Read
    # before the pair legs: the speaker leg's span gate
    # depends on the speaker band's claim.
    claim_trees = {}
    for scene, tree_name in (("populated", TREE_SEEDED),
                             ("top", TREE_TOP),
                             ("speaker", TREE_SPEAKER)):
        claim_trees[scene] = load_tree(os.path.join(directory, tree_name))
    band_claims = {}
    for scene, tree in claim_trees.items():
        if tree is None:
            continue
        band_claims[scene] = {
            "lines": len(keyed_family(tree, SHAPED_LINE_STEM)),
            "layer": element_with_key(tree, SHAPED_LAYER_KEY) is not None,
        }
    any_claim = any(c["lines"] > 0 or c["layer"]
                    for c in band_claims.values())
    if not any_claim:
        report("band claims: no shaped layer in any seeded tree — "
               "the bands paint through the bitmap (Pixel) route, "
               "or the trees predate W2-45; bitmap derivations "
               "in force")
    else:
        for scene in ("populated", "top", "speaker"):
            c = band_claims.get(scene)
            if c is not None:
                report(f"band claims: {scene} tree carries the "
                       f"shaped layer (container "
                       f"{'present' if c['layer'] else 'ABSENT'}, "
                       f"{c['lines']} claimed line element(s))")
        counts = {c["lines"] for c in band_claims.values()}
        structure_ok = all(c["layer"] == (c["lines"] > 0)
                           for c in band_claims.values())
        check(structure_ok and len(counts) == 1,
              "shaped band claims agree across the seeded scenes",
              f"claims={band_claims} — same default style and "
              f"Latin single-line cues in every seeded scene, so "
              f"the per-band claim (a font/style property) must "
              f"be uniform, each claimed band under its layer "
              f"container")

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
    _pop_n, pop_min, pop_max = face_xor_extent(rows_bottom, rows_top,
                                               *bottom_r)
    pop_span_units = ((pop_max - pop_min + 1) / fit) if _pop_n else 0.0
    report(f"pair populated face span: {pop_span_units:.0f} stage "
           f"units (the same-renderer control for the shaped span "
           f"gate, Wave 2 expansion)")
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
    # The span gate's derivation is the bitmap renderer's
    # (4-unit cells). When the speaker band is claimed by
    # the shaped pipeline (W2-45), the painted span is
    # font metrics and the bitmap gate would fail an
    # honest line — gate instead on the same-renderer
    # control measured this run (Wave 2 expansion).
    speaker_claimed = band_claims.get("speaker", {}).get("lines", 0) > 0
    if speaker_claimed:
        report(f"speaker span gate: the bitmap gate "
               f"({SPEAKER_SPAN_GATE_UNITS:.0f} units) does not "
               f"apply — the speaker band is claimed by the shaped "
               f"pipeline, whose span is font metrics, not 4-unit "
               f"cells. Measured span {span_units:.0f} units; "
               f"gating on the same-renderer control instead "
               f"(populated span {pop_span_units:.0f} units)")
        check(span_units >= SHAPED_SPAN_RATIO * pop_span_units,
              f"credited line outspans the same-renderer control "
              f"under the shaped renderer (>= {SHAPED_SPAN_RATIO:.2f}x "
              f"the populated cue's painted span)",
              f"span={span_units:.0f} units, control span="
              f"{pop_span_units:.0f} units")
    else:
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
            # the speaker block can sit from it); converted
            # from points to capture pixels by the snap
            # scale, rounded outward and clamped to the
            # image, the band idiom.
            block_px = tuple(v * snap_scale for v in block_pop)
            bx0 = max(0, int(math.floor(block_px[0])))
            by0 = max(0, int(math.floor(block_px[1])))
            bx1 = min(img_w, int(math.ceil(block_px[0] + block_px[2])))
            by1 = min(img_h, int(math.ceil(block_px[1] + block_px[3])))
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

    # ---- the Wave 2 expansion legs (W2-79): tree-text
    # and optional-input evidence, each fail-soft (absent
    # evidence reports not-run and gates nothing).
    dialogue_leg(directory, check, report)
    srt_announcement_leg(directory, check, report)
    audio_tree_leg(directory, check, report)

    return lines, failures


# ----------------------------------------------------------------
# Wave 2 expansion legs (W2-79). Each takes the census's
# check/report closures so its lines join the one
# transcript; each is fail-soft — evidence absent means
# a reported not-run, evidence present and contradictory
# means a failed check.
# ----------------------------------------------------------------

def dialogue_leg(directory, check, report):
    """The seeded lane labels (W2-17's composition, read
    as dump text) and W2-12's dialogue lane: line blocks
    delegated to the subtitle lane's rects and labels,
    chapter segments carrying their titles, and the
    playhead's chapter agreeing across header text,
    chip geometry and segment fill."""
    for scene, tree_name, expected_label in (
            ("populated", TREE_SEEDED, SEEDED_LABEL_POPULATED),
            ("speaker", TREE_SPEAKER, SEEDED_LABEL_SPEAKER)):
        tree = load_tree(os.path.join(directory, tree_name))
        if tree is None:
            continue
        block = element_with_key(tree, LANE_BLOCK_KEY)
        if block is not None:
            label = descendant_text(tree, block)
            check(label == expected_label,
                  f"seeded lane label in the {scene} tree is the "
                  f"planted composition",
                  f"label={label!r}, planted={expected_label!r}")
        if element_with_key(tree, DIALOGUE_LANE_KEY) is None:
            report(f"dialogue leg ({scene}): not run — no dialogue "
                   f"lane in {tree_name} (pre-W2-12 tree, or the "
                   f"lane's data gate closed)")
            continue
        lines = keyed_family(tree, DIALOGUE_LINE_STEM)
        chapters = keyed_family(tree, DIALOGUE_CHAPTER_STEM)
        report(f"dialogue leg ({scene}): lane present with "
               f"{len(lines)} line block(s), {len(chapters)} "
               f"chapter segment(s)")
        for i in sorted(lines):
            cue_block = element_with_key(tree, f"subtitle-cue-{i}")
            if cue_block is None:
                check(False,
                      f"dialogue line {i} has its cue block in the "
                      f"{scene} tree",
                      "the dialogue lane binds lines to subtitle "
                      "lane blocks; the block is missing")
                continue
            lr, cr = element_rect(lines[i]), element_rect(cue_block)
            if lr is not None and cr is not None:
                drift = max(abs(a - b) for a, b in zip(lr, cr))
                check(drift <= LANE_RECT_DRIFT_MAX,
                      f"dialogue line {i} sits exactly on its cue "
                      f"block in the {scene} tree",
                      f"drift={drift:.1f} pt — line_rect delegates "
                      f"to cue_rect, so the two lanes cannot "
                      f"disagree about a line's window")
            lt = descendant_text(tree, lines[i])
            ct = descendant_text(tree, cue_block)
            check(lt == ct,
                  f"dialogue line {i} reads exactly as its cue "
                  f"block in the {scene} tree",
                  f"dialogue={lt!r}, lane={ct!r} — line_label "
                  f"delegates to block_label")
        if not chapters:
            report(f"dialogue leg ({scene}): no chapter band — "
                   f"no chapters planted in this scene")
            continue
        titles = {}
        missing = []
        for j in sorted(chapters):
            titles[j] = descendant_text(tree, chapters[j])
            if not titles[j]:
                missing.append(j)
        check(not missing,
              f"every dialogue chapter segment carries its title "
              f"in the {scene} tree",
              f"segments without a title text: {missing}")
        report(f"dialogue leg ({scene}): chapter titles "
               f"{[titles[j] for j in sorted(titles)]}")
        for i in sorted(lines):
            lr = element_rect(lines[i])
            if lr is None:
                continue
            holder = None
            for j in sorted(chapters):
                sr = element_rect(chapters[j])
                if sr is not None and sr[0] <= lr[0] < sr[0] + sr[2]:
                    holder = j
                    break
            report(f"dialogue leg ({scene}): line {i} starts in "
                   f"chapter "
                   f"{holder if holder is not None else 'a gap'} "
                   f"by geometry (reported, not gated: the "
                   f"first-in-episode-order rule read off the "
                   f"shared span algebra)")
        # The playhead's chapter: with >= 3 segments the
        # plain fill repeats, so a distinct fill identifies
        # the selected segment; it must be the first
        # segment holding the parked frame's chip x.
        parked = None
        for text in all_texts(tree):
            m = PARKED_FRAME_RE.match(text)
            if m:
                parked = int(m.group(1)) - 1
                break
        chip_x = None
        if parked is not None:
            for el in tree_elements(tree):
                if (el.get("path") or "").endswith(
                        f"/timeline-chips/frame-{parked}"):
                    r = element_rect(el)
                    if r is not None:
                        chip_x = r[0]
                    break
        if parked is None or chip_x is None or len(chapters) < 3:
            report(f"dialogue leg ({scene}): playhead-chapter "
                   f"agreement not run (parked frame "
                   f"{'unknown' if parked is None else parked}, "
                   f"chip x "
                   f"{'unknown' if chip_x is None else chip_x}, "
                   f"{len(chapters)} segment(s) — the fill "
                   f"discrimination needs a parked frame, its "
                   f"chip and >= 3 segments)")
            continue
        fills = {}
        for j in sorted(chapters):
            fills.setdefault(chapters[j].get("fill"), []).append(j)
        # The band knows two fills (plain, and the
        # playhead segment's selected fill), so with
        # >= 3 segments the selected one — if any — is
        # the singleton group.
        distinct = [j for group in fills.values() if len(group) == 1
                    for j in group] if len(fills) > 1 else []
        check(len(distinct) <= 1,
              f"at most one dialogue chapter segment wears a "
              f"distinct fill in the {scene} tree",
              f"distinct-filled segments: {distinct} — one "
              f"playhead has one chapter")
        holder = None
        for j in sorted(chapters):
            sr = element_rect(chapters[j])
            if sr is not None and sr[0] - 1.0 <= chip_x < sr[0] + sr[2] + 1.0:
                holder = j
                break
        if len(distinct) == 1:
            check(distinct[0] == holder,
                  f"the distinct-filled chapter segment is the "
                  f"playhead's chapter in the {scene} tree",
                  f"segment {distinct[0]} wears the distinct "
                  f"fill, but the parked frame {parked} (chip x "
                  f"{chip_x}) falls in segment {holder}")
        elif holder is not None:
            report(f"dialogue leg ({scene}): no segment wears a "
                   f"distinct fill, yet the parked frame "
                   f"{parked} falls in segment {holder} — "
                   f"reported, not gated (the selection fill "
                   f"may not discriminate in this theme)")


def srt_announcement_leg(directory, check, report):
    """W2-17's counted-and-announced SRT speaker loss.
    Scans every tree dump's texts and the MCP probe
    transcript for the two announcement sentences and
    the reply field; gates what has ground truth (see
    the docstring's SRT section)."""
    findings = []
    try:
        names = sorted(n for n in os.listdir(directory)
                       if n.startswith("tree-") and n.endswith(".json"))
    except Exception:
        names = []
    for name in names:
        tree = load_tree(os.path.join(directory, name))
        if tree is None:
            continue
        for text in all_texts(tree):
            for kind, rx in (("export", SRT_EXPORT_ANNOUNCE_RE),
                             ("import", SRT_IMPORT_ANNOUNCE_RE)):
                m = rx.search(text)
                if m:
                    findings.append((name, kind, int(m.group(1))))
    probe_findings = []
    probe_path = os.path.join(directory, MCP_PROBE_TXT)
    if os.path.isfile(probe_path):
        try:
            with open(probe_path) as fh:
                probe_text = fh.read()
            probe_findings = [int(v) for v in
                              MCP_SPEAKERS_DROPPED_RE.findall(probe_text)]
        except Exception:
            probe_findings = []
    if not findings and not probe_findings:
        report("SRT announcement leg: not run — no SRT export or "
               "import announcement in any tree dump and no "
               "speakers_dropped in the MCP probe transcript; no "
               "headless surface performs an SRT action today "
               "(IA-W2-79-2)")
        return
    for name, kind, n in findings:
        report(f"SRT announcement leg: {name} carries the {kind} "
               f"announcement with count {n}")
        check(n >= 1,
              f"SRT announcements only appear with a count of at "
              f"least 1 ({name})",
              f"count={n} — both surfaces write the sentence "
              f"only when credits were dropped")
        if kind == "export" and name in SEEDED_CREDITED:
            check(n == SEEDED_CREDITED[name],
                  f"SRT export announcement in {name} counts the "
                  f"scene's credited cues",
                  f"announced={n}, planted credited="
                  f"{SEEDED_CREDITED[name]} — export is "
                  f"read-only, so the count is the exported "
                  f"project's credited cues")
    for v in probe_findings:
        report(f"SRT announcement leg: the MCP probe transcript "
               f"carries speakers_dropped={v}")
        check(v >= 1,
              "MCP speakers_dropped is present only when above "
              "zero",
              f"value={v} — the reply gains the field only when "
              f"the SRT export dropped credits")


def audio_tree_leg(directory, check, report):
    """The Audio workspace tree's subtitle surfaces:
    W2-15's title-card section (row census + the
    preview's expectation caption) and W2-16's timing
    assistant (proposal lines reported — CPS never
    judged, D-9 — with its count line gated against
    the rows shown)."""
    tree = load_tree(os.path.join(directory, TREE_AUDIO))
    if tree is None:
        report(f"audio-tree leg: not run — {TREE_AUDIO} absent")
        return
    # ---- title cards (W2-15).
    section = element_with_key(tree, TITLE_CARD_SECTION_KEY)
    if section is None:
        report("title-card leg: not run — the audio tree carries "
               "no title-card section (pre-W2-15 tree, or the "
               "section did not mount)")
    else:
        rows = keyed_family(tree, TITLE_CARD_ROW_STEM)
        parsed = {}
        bad = []
        for i in sorted(rows):
            label = descendant_text(tree, rows[i])
            m = TITLE_CARD_ROW_RE.match(label or "")
            if m:
                parsed[i] = (int(m.group(1)), m.group(2),
                             int(m.group(3)), int(m.group(4)))
            else:
                bad.append(i)
        report(f"title-card leg: section present with "
               f"{len(rows)} chapter row(s)")
        if rows:
            ordered = (not bad
                       and [parsed[i][0] for i in sorted(parsed)]
                       == list(range(1, len(rows) + 1))
                       and all(parsed[i][2] <= parsed[i][3]
                               for i in parsed)
                       and all(parsed[a][2] < parsed[b][2]
                               for a, b in zip(sorted(parsed),
                                               sorted(parsed)[1:])))
            check(ordered,
                  "title-card row labels are numbered in episode "
                  "order with increasing frame ranges",
                  f"unparseable rows: {bad}, parsed: {parsed}")
        preview = element_with_key(tree, TITLE_CARD_PREVIEW_KEY)
        caption = None
        if preview is not None:
            for text in descendant_texts(tree, preview):
                if TITLE_CARD_SHOWS_RE.match(text) or \
                        text.startswith("Cards are off"):
                    caption = text
                    break
        if caption is not None:
            report(f"title-card leg: expectation caption: "
                   f"{caption!r}")
        if rows and preview is not None:
            check(caption is not None,
                  "title-card section carries its expectation "
                  "caption when chapters are planted",
                  "rows are present but the preview states no "
                  "expectation line")
            m = TITLE_CARD_SHOWS_RE.match(caption or "")
            if m:
                stated_frame = int(m.group(2))
                preview_texts = descendant_texts(tree, preview)
                eyebrow = next(
                    (int(TITLE_CARD_EYEBROW_RE.match(t).group(1))
                     for t in preview_texts
                     if TITLE_CARD_EYEBROW_RE.match(t)), None)
                titled = [i for i in parsed
                          if parsed[i][1] in preview_texts]
                check(bool(titled)
                      and any(parsed[i][2] == stated_frame
                              for i in titled)
                      and (eyebrow is None
                           or any(parsed[i][0] == eyebrow
                                  for i in titled)),
                      "title-card preview caption states the "
                      "selected chapter's first frame",
                      f"caption={caption!r}, eyebrow={eyebrow}, "
                      f"rows={parsed}")
        # Cross-surface, same tree: the dialogue band in
        # THIS tree (same project, same run) must title
        # its chapters exactly as the panel rows do.
        band = keyed_family(tree, DIALOGUE_CHAPTER_STEM)
        if band and parsed:
            band_titles = [descendant_text(tree, band[j])
                           for j in sorted(band)]
            row_titles = [parsed[i][1] for i in sorted(parsed)]
            check(band_titles == row_titles,
                  "chapter titles agree between the dialogue "
                  "band and the title-card rows",
                  f"band={band_titles}, rows={row_titles}")
    # ---- timing assistant + CPS readouts (W2-16, RM21).
    texts = all_texts(tree)
    proposals = [t for t in texts if ASSIST_PROPOSAL_RE.match(t)]
    cps_lines = [t for t in texts if "chars/s" in t]
    if not proposals and not cps_lines:
        report("timing-assistant leg: not run — the audio tree "
               "shows no proposal or CPS lines (the cue-less "
               "audio scene plants no cues)")
        return
    for t in cps_lines:
        report(f"timing-assistant leg: readout: {t!r} (CPS "
               f"reported, never judged — D-9)")
    count_line = next((t for t in texts if ASSIST_COUNT_RE.match(t)),
                      None)
    if count_line is not None:
        stated = int(ASSIST_COUNT_RE.match(count_line).group(1))
        check(stated == len(proposals),
              "timing-assistant count line matches the proposal "
              "rows shown",
              f"count line states {stated}, proposal rows "
              f"shown: {len(proposals)}")


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

        # Wave 2 expansion fixtures (W2-79): the dumps
        # carry each lane block's label as a text element
        # under the block; the dialogue lane delegates to
        # the same rects and labels; the speaker tree
        # additionally plants a chapter band, the
        # parked-frame header, the frame chips and an SRT
        # export announcement in its status text.
        BLOCK_PATH = "/root/timeline/subtitle-lane/subtitle-cue-0"

        def text_el(path, text):
            return {"path": path, "key": None, "text": text}

        def dialogue_els(label):
            return [
                {"path": "/root/timeline/dialogue-lane",
                 "key": "dialogue-lane", "rect": [120, 436, 166, 18]},
                {"path": "/root/timeline/dialogue-lane/dialogue-line-0",
                 "key": "dialogue-line-0", "rect": list(LANE_BLOCK)},
                text_el("/root/timeline/dialogue-lane/dialogue-line-0/#0",
                        label),
            ]

        def populated_tree():
            els = list(lane_tree["elements"]) + [
                text_el(BLOCK_PATH + "/#0",
                        "We walk home before dark."),
            ] + dialogue_els("We walk home before dark.")
            return {"elements": els}

        def speaker_tree(fills=("#101216ff", "#2b6cb0ff", "#101216ff"),
                         announce=1, dialogue_label=None):
            label = "NOVA: The light is still on."
            els = list(lane_tree["elements"]) + [
                text_el(BLOCK_PATH + "/#0", label),
                text_el("/root/status",
                        f"Exported 1 subtitle cue(s) to /tmp/x.srt "
                        f"— {announce} speaker credit(s) not written "
                        f"(SRT has no speaker syntax)."),
                text_el("/root/timeline/subtitle-lane/#0/#2",
                        "Frame 3 / 5"),
            ] + dialogue_els(dialogue_label or label)
            # The planted band: three segments over the
            # 5-frame timeline at origin 120, pitch 33.2
            # (the chips below); the parked frame is 2,
            # inside segment 1 — the one wearing the
            # distinct (selected) fill.
            spans = [(120.0, 33.2), (153.2, 66.4), (219.6, 66.4)]
            for j, (x, w) in enumerate(spans):
                els.append({
                    "path": "/root/timeline/dialogue-lane/"
                            f"dialogue-chapter-{j}",
                    "key": f"dialogue-chapter-{j}",
                    "rect": [x, 416, w, 18], "fill": fills[j]})
                els.append(text_el(
                    "/root/timeline/dialogue-lane/"
                    f"dialogue-chapter-{j}/#0",
                    ["Dawn", "Noon", "Dusk"][j]))
            for k in range(5):
                els.append({
                    "path": "/root/timeline/timeline-chips/"
                            f"frame-{k}",
                    "key": None,
                    "rect": [120.0 + 33.2 * k, 400, 30, 12]})
            return {"elements": els}

        def write_speaker_tree(**kw):
            with open(os.path.join(td, TREE_SPEAKER), "w") as fh:
                json.dump(speaker_tree(**kw), fh)

        with open(os.path.join(td, TREE_SEEDED), "w") as fh:
            json.dump(populated_tree(), fh)
        write_speaker_tree()

        # The honest set passes.
        _lines, fails = run_census(td)
        expect(not fails, f"census passes on the synthetic honest set "
                          f"(failures: {fails})")
        expect(any("planted composition" in l for l in _lines),
               "census gates the seeded lane labels as dump text")
        expect(any("carries the export announcement" in l
                   for l in _lines),
               "census reads the SRT announcement out of the "
               "speaker tree's status text")

        # A dialogue line whose label drops the credit
        # (the subtitle lane's keeps it) must fail the
        # delegation gate.
        write_speaker_tree(dialogue_label="The light is still on.")
        _lines, fails = run_census(td)
        expect(any("reads exactly as its cue block" in f
                   for f in fails),
               "census fails when the dialogue line's label drops "
               "the speaker credit")
        write_speaker_tree()

        # An SRT announcement counting more credits than
        # the scene planted must fail the export gate.
        write_speaker_tree(announce=2)
        _lines, fails = run_census(td)
        expect(any("SRT export announcement" in f for f in fails),
               "census fails when the SRT announcement overcounts "
               "the dropped credits")
        write_speaker_tree()

        # The distinct fill on the wrong chapter segment
        # must fail the playhead agreement.
        write_speaker_tree(fills=("#2b6cb0ff", "#101216ff",
                                  "#101216ff"))
        _lines, fails = run_census(td)
        expect(any("playhead's chapter" in f for f in fails),
               "census fails when the distinct fill sits on a "
               "chapter the playhead is not in")
        write_speaker_tree()

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

    # The capture-scale contract (the integrator's fix):
    # the same honest geometry at device scale 2.0 — trees
    # carry point rects plus the dump's "scale" and window
    # bounds, snaps are painted in doubled PIXELS. Under
    # the pre-fix contract (points sampled as pixels) every
    # leg below reads the wrong quadrant and reports zero,
    # so this set passing pins the conversion.
    with tempfile.TemporaryDirectory() as td2:
        PW, PH = W * 2, H * 2
        tree2 = {"scale": 2.0,
                 "accessibility": {"bounds": [0, 0, W, H]},
                 "elements": tree["elements"]}
        lane_tree2 = {"scale": 2.0,
                      "accessibility": {"bounds": [0, 0, W, H]},
                      "elements": lane_tree["elements"]}
        def lane_tree2_labelled(label):
            return {"scale": 2.0,
                    "accessibility": {"bounds": [0, 0, W, H]},
                    "elements": lane_tree2["elements"] + [
                        {"path": "/root/timeline/subtitle-lane/"
                                 "subtitle-cue-0/#0",
                         "key": None, "text": label}]}

        with open(os.path.join(td2, TREE_SEEDED), "w") as fh:
            json.dump(lane_tree2_labelled(
                "We walk home before dark."), fh)
        with open(os.path.join(td2, TREE_SPEAKER), "w") as fh:
            json.dump(lane_tree2_labelled(
                "NOVA: The light is still on."), fh)
        with open(os.path.join(td2, TREE_BASELINE), "w") as fh:
            json.dump(tree2, fh)

        _sx0, dy0p, _sx1, dy1p, fitp = drawn_stage(
            tuple(v * 2.0 for v in RECT))
        bandp = BAND_UNITS * fitp

        def blank2(fill=BG):
            return [bytes(fill) * PW for _ in range(PH)]

        def build2(bottom_cue, top_cue):
            rows = blank2()
            paint(rows, 660, 780, 360, 600, FIG)
            if bottom_cue:
                paint(rows, 440, 1000, int(dy1p - bandp) + 12,
                      int(dy1p) - 4, WHITE)
            if top_cue:
                paint(rows, 440, 1000, int(dy0p) + 4,
                      int(dy0p + bandp) - 12, WHITE)
            return rows

        def lane2(rows, x1):
            # LANE_BLOCK in pixels is (240, 832, 332, 36);
            # ink inset 8 px, vertically centred.
            paint(rows, 248, x1, 840, 860, WHITE)

        bottom2 = build2(True, False)
        lane2(bottom2, 480)   # the uncredited label's run
        top2 = build2(False, True)
        base2 = build2(False, False)
        speaker2 = build2(False, False)
        # The credited extent: 444 units at fitp = 444 px,
        # centred on the drawn stage's centre column (720).
        paint(speaker2, 498, 942, int(dy1p - bandp) + 12,
              int(dy1p) - 4, WHITE)
        lane2(speaker2, 564)  # the credited label's run
        _write_png(os.path.join(td2, SEEDED_BOTTOM_FULL),
                   PW, PH, bottom2, color=2)
        _write_png(os.path.join(td2, SEEDED_TOP_FULL),
                   PW, PH, top2, color=2)
        _write_png(os.path.join(td2, BASELINE_FULL),
                   PW, PH, base2, color=2)
        _write_png(os.path.join(td2, SEEDED_SPEAKER_FULL),
                   PW, PH, speaker2, color=2)
        lines2, fails2 = run_census(td2)
        expect(not fails2,
               f"census passes on the synthetic honest set at "
               f"capture scale 2 (failures: {fails2})")
        expect(any("capture scale 2.000" in l for l in lines2),
               "census reports the capture scale it converted by")

    # The shaped-claim recalibration (W2-79): the same
    # seeded scenes with their bands claimed by the
    # shaped pipeline. The painted spans are font-metric
    # scale (credited 320 units, under the bitmap gate's
    # 396), so the bitmap gate would fail them; with the
    # claim elements in the trees the census must pass
    # on the same-renderer control, and removing the
    # claim must restore the bitmap failure.
    with tempfile.TemporaryDirectory() as td3:
        def claim_tree(with_claim, label):
            els = [
                {"path": "/root", "key": "root",
                 "rect": [0, 0, W, H]},
                {"path": "/root/stage", "key": "stage",
                 "rect": [RECT[0], RECT[1], RECT[2], RECT[3]]},
                {"path": "/root/timeline/subtitle-lane/subtitle-cue-0",
                 "key": "subtitle-cue-0", "rect": list(LANE_BLOCK)},
                {"path": "/root/timeline/subtitle-lane/"
                         "subtitle-cue-0/#0",
                 "key": None, "text": label},
            ]
            if with_claim:
                els += [
                    {"path": "/root/stage/stage-subtitle-layer",
                     "key": "stage-subtitle-layer",
                     "rect": [RECT[0], RECT[1], RECT[2], RECT[3]]},
                    {"path": "/root/stage/stage-subtitle-layer/"
                             "stage-subtitle-line-0",
                     "key": "stage-subtitle-line-0",
                     "rect": [RECT[0], RECT[1], RECT[2], 24]},
                ]
            return {"elements": els}

        def write_claim_trees(pop_claim, spk_claim):
            with open(os.path.join(td3, TREE_SEEDED), "w") as fh:
                json.dump(claim_tree(pop_claim,
                                     "We walk home before dark."), fh)
            with open(os.path.join(td3, TREE_SPEAKER), "w") as fh:
                json.dump(claim_tree(spk_claim,
                                     "NOVA: The light is still on."),
                          fh)
            with open(os.path.join(td3, TREE_BASELINE), "w") as fh:
                json.dump(tree, fh)

        bottom3 = build(False, False)
        paint(bottom3, 220, 360, int(dy1 - band) + 6, int(dy1) - 2,
              WHITE)
        paint_lane_label(bottom3, 240)
        top3 = build(False, True)
        base3 = build(False, False)
        spk3 = build(False, False)
        paint(spk3, 250, 410, int(dy1 - band) + 6, int(dy1) - 2,
              WHITE)
        paint_lane_label(spk3, 282)
        _write_png(os.path.join(td3, SEEDED_BOTTOM_FULL), W, H,
                   to_rgba(bottom3), color=6)
        _write_png(os.path.join(td3, SEEDED_TOP_FULL), W, H,
                   to_rgba(top3), color=6)
        _write_png(os.path.join(td3, BASELINE_FULL), W, H,
                   base3, color=2)
        _write_png(os.path.join(td3, SEEDED_SPEAKER_FULL), W, H,
                   to_rgba(spk3), color=6)

        # Claims disagreeing across the scenes must fail:
        # the claim is a font/style property, uniform for
        # the machine the run happened on.
        write_claim_trees(True, False)
        _lines, fails = run_census(td3)
        expect(any("shaped band claims agree" in f for f in fails),
               "census fails when the seeded scenes disagree "
               "about the shaped claim")

        # Uniform claims: the shaped span gate passes on
        # the same-renderer control (320 >= 0.95 x 280).
        write_claim_trees(True, True)
        _lines, fails = run_census(td3)
        expect(not fails,
               f"census passes on the shaped-claimed set "
               f"(failures: {fails})")
        expect(any("same-renderer control" in l for l in _lines),
               "census reports the shaped span gate it applied")

        # The same snaps with no claim recorded fall back
        # to the bitmap span gate — and fail it, proving
        # the recalibration is what carried the shaped
        # set.
        write_claim_trees(False, False)
        _lines, fails = run_census(td3)
        expect(any("speaker prefix widens" in f for f in fails),
               "census fails the bitmap span gate on the "
               "shaped set when no claim is recorded")

    # The audio-tree leg exercised directly (W2-79): a
    # synthetic Audio workspace tree carrying the
    # title-card section and the timing assistant.
    with tempfile.TemporaryDirectory() as td4:
        sec = "/root/audio/title-card-section"
        els = [
            {"path": sec, "key": "title-card-section",
             "rect": [0, 0, 300, 200]},
            {"path": sec + "/title-card-row-0",
             "key": "title-card-row-0", "rect": [0, 0, 300, 18]},
            {"path": sec + "/title-card-row-0/#0", "key": None,
             "text": "1. Dawn — frames 1–24"},
            {"path": sec + "/title-card-row-1",
             "key": "title-card-row-1", "rect": [0, 20, 300, 18]},
            {"path": sec + "/title-card-row-1/#0", "key": None,
             "text": "2. Dusk — frames 25–48"},
            {"path": sec + "/title-card-preview",
             "key": "title-card-preview", "rect": [0, 44, 300, 60]},
            {"path": sec + "/title-card-preview/#0", "key": None,
             "text": "CHAPTER 1"},
            {"path": sec + "/title-card-preview/#1", "key": None,
             "text": "Dawn"},
            {"path": sec + "/title-card-preview/#2", "key": None,
             "text": "Shows for 2.5 s from frame 1"},
            {"path": "/root/audio/subtitle-assist", "key": None,
             "text": "Cue 1 · extend to 2.5 s · 24.0 → 17.0 chars/s"},
            {"path": "/root/audio/subtitle-assist-count",
             "key": None, "text": "1 suggestion(s)"},
        ]

        def audio_run(elements):
            with open(os.path.join(td4, TREE_AUDIO), "w") as fh:
                json.dump({"elements": elements}, fh)
            ls, fs = [], []

            def ck(ok, label, detail=""):
                if not ok:
                    fs.append(label)

            audio_tree_leg(td4, ck, lambda t: ls.append(t))
            return ls, fs

        _ls, fs = audio_run(els)
        expect(not fs,
               f"audio-tree leg passes on the honest section "
               f"(failures: {fs})")
        expect(any("expectation caption" in l for l in _ls),
               "audio-tree leg reports the title-card expectation "
               "caption")
        expect(any("never judged" in l for l in _ls),
               "audio-tree leg reports CPS without judging it")

        bad = [dict(e) for e in els]
        bad[8]["text"] = "Shows for 2.5 s from frame 9"
        _ls, fs = audio_run(bad)
        expect(any("first frame" in f for f in fs),
               "audio-tree leg fails when the caption states the "
               "wrong first frame")

        bad2 = [dict(e) for e in els]
        bad2[10]["text"] = "2 suggestion(s)"
        _ls, fs = audio_run(bad2)
        expect(any("count line matches" in f for f in fs),
               "audio-tree leg fails when the assist count line "
               "disagrees with the proposal rows")

    # The MCP speakers_dropped field, exercised directly.
    with tempfile.TemporaryDirectory() as td5:
        def srt_run(probe_text):
            p = os.path.join(td5, MCP_PROBE_TXT)
            if probe_text is None:
                if os.path.isfile(p):
                    os.remove(p)
            else:
                with open(p, "w") as fh:
                    fh.write(probe_text)
            ls, fs = [], []

            def ck(ok, label, detail=""):
                if not ok:
                    fs.append(label)

            srt_announcement_leg(td5, ck, lambda t: ls.append(t))
            return ls, fs

        _ls, fs = srt_run('{"speakers_dropped": 2}')
        expect(not fs, "SRT leg passes on a positive "
                       "speakers_dropped")
        _ls, fs = srt_run('{"speakers_dropped": 0}')
        expect(any("present only" in f for f in fs),
               "SRT leg fails on a zero speakers_dropped field")
        _ls, fs = srt_run(None)
        expect(not fs and any("not run" in l for l in _ls),
               "SRT leg reports not run when no surface speaks")

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
