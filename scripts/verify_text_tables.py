#!/usr/bin/env python3
"""Cross-verify .piv v5 TEXT side tables against the original app.

Wave 13: the port's v5 exporter now writes text side tables whose
glyph outlines are its 3x5 bitmap font drawn as tiny square paths
(one M/L/L/L/Z square per lit pixel). This script checks that
work against the ORIGINAL's Python codecs
(~/workspace/StykAnimate-App/codecs) in three ways:

1. FIXTURE: builds tests/fixtures/text-v5.piv with the original's
   own writer (scaffold + migrate + encode_piv_project) — the
   shared Tripod corpus scene with segment 2 as a text segment
   carrying "HI" — registers it in tests/fixtures/manifest.json,
   and re-reads it with the original's decoder, asserting the
   scene values and the text asset (content, glyph node counts,
   placements). The port's corpus tests then import this file.

2. ALGORITHM: mirrors the port's documented bitmap-glyph
   algorithm (font table transcribed from src/media.rs; glyph
   space: 1 unit = 1 bitmap pixel, cell 3x5 at the origin, unit i
   placed at x = 4i, placements only for visible glyphs) and
   round-trips it through the original's encode_side_tables /
   parse_side_tables, asserting content, glyph node counts
   (5 per lit pixel), placements and bounds.

3. RUST EXPORT (when present): /tmp/styk-export-v5-media.piv is
   written by the port's own test suite; when it exists, its
   payload is walked to the figure's side tables and parsed with
   the original's parse_side_tables, asserting the text entry is
   the bitmap encoding of "HI". Skipped (loudly) when absent —
   the file only exists after the Rust tests have run somewhere.

Usage: python3 scripts/verify_text_tables.py [--codecs PATH]
"""
from __future__ import annotations

import json
import os
import struct
import sys
import unicodedata
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FIXTURES = ROOT / "tests" / "fixtures"

_args = sys.argv[1:]
if "--codecs" in _args:
    CODECS = Path(_args[_args.index("--codecs") + 1])
else:
    CODECS = Path(
        os.environ.get(
            "STYK_ORIGINAL_CODECS",
            Path.home() / "workspace" / "StykAnimate-App" / "codecs",
        )
    )
sys.path.insert(0, str(CODECS))

from codec_bridge import scaffold  # noqa: E402
from piv_codec import decode_piv_bytes, encode_piv_project  # noqa: E402
from piv_migrate import _figure as migrate_figure  # noqa: E402
from piv_side_tables import encode_side_tables, parse_side_tables  # noqa: E402

# ---------------------------------------------------------------------------
# The 3x5 bitmap font, transcribed from src/media.rs glyph_rows
# (rows top-to-bottom, bit 2 = leftmost pixel). The Rust exporter
# carries the same table as bitmap_glyph_rows in src/pivot.rs.
# ---------------------------------------------------------------------------
FONT = {
    "A": [0b010, 0b101, 0b111, 0b101, 0b101], "B": [0b110, 0b101, 0b110, 0b101, 0b110],
    "C": [0b011, 0b100, 0b100, 0b100, 0b011], "D": [0b110, 0b101, 0b101, 0b101, 0b110],
    "E": [0b111, 0b100, 0b110, 0b100, 0b111], "F": [0b111, 0b100, 0b110, 0b100, 0b100],
    "G": [0b011, 0b100, 0b101, 0b101, 0b011], "H": [0b101, 0b101, 0b111, 0b101, 0b101],
    "I": [0b111, 0b010, 0b010, 0b010, 0b111], "J": [0b001, 0b001, 0b001, 0b101, 0b010],
    "K": [0b101, 0b101, 0b110, 0b101, 0b101], "L": [0b100, 0b100, 0b100, 0b100, 0b111],
    "M": [0b101, 0b111, 0b111, 0b101, 0b101], "N": [0b101, 0b111, 0b101, 0b101, 0b101],
    "O": [0b010, 0b101, 0b101, 0b101, 0b010], "P": [0b110, 0b101, 0b110, 0b100, 0b100],
    "Q": [0b010, 0b101, 0b101, 0b010, 0b001], "R": [0b110, 0b101, 0b110, 0b101, 0b101],
    "S": [0b011, 0b100, 0b010, 0b001, 0b110], "T": [0b111, 0b010, 0b010, 0b010, 0b010],
    "U": [0b101, 0b101, 0b101, 0b101, 0b111], "V": [0b101, 0b101, 0b101, 0b101, 0b010],
    "W": [0b101, 0b101, 0b101, 0b111, 0b101], "X": [0b101, 0b101, 0b010, 0b101, 0b101],
    "Y": [0b101, 0b101, 0b010, 0b010, 0b010], "Z": [0b111, 0b001, 0b010, 0b100, 0b111],
    "0": [0b111, 0b101, 0b101, 0b101, 0b111], "1": [0b010, 0b110, 0b010, 0b010, 0b111],
    "2": [0b110, 0b001, 0b010, 0b100, 0b111], "3": [0b110, 0b001, 0b010, 0b001, 0b110],
    "4": [0b101, 0b101, 0b111, 0b001, 0b001], "5": [0b111, 0b100, 0b110, 0b001, 0b110],
    "6": [0b011, 0b100, 0b110, 0b101, 0b010], "7": [0b111, 0b001, 0b001, 0b010, 0b010],
    "8": [0b010, 0b101, 0b010, 0b101, 0b010], "9": [0b010, 0b101, 0b011, 0b001, 0b110],
    " ": [0, 0, 0, 0, 0], ".": [0, 0, 0, 0, 0b010], ",": [0, 0, 0, 0b010, 0b100],
    "!": [0b010, 0b010, 0b010, 0, 0b010], "?": [0b110, 0b001, 0b010, 0, 0b010],
    "'": [0b010, 0b010, 0, 0, 0], '"': [0b101, 0b101, 0, 0, 0],
    "-": [0, 0, 0b111, 0, 0], ":": [0, 0b010, 0, 0b010, 0],
    ";": [0, 0b010, 0, 0b010, 0b100], "(": [0b001, 0b010, 0b010, 0b010, 0b001],
    ")": [0b100, 0b010, 0b010, 0b010, 0b100], "+": [0, 0b010, 0b111, 0b010, 0],
    "/": [0b001, 0b001, 0b010, 0b100, 0b100], "&": [0b010, 0b101, 0b010, 0b101, 0b011],
    "%": [0b101, 0b001, 0b010, 0b100, 0b101], "*": [0, 0b101, 0b010, 0b101, 0],
    "=": [0, 0b111, 0, 0b111, 0], "<": [0b001, 0b010, 0b100, 0b010, 0b001],
    ">": [0b100, 0b010, 0b001, 0b010, 0b100], "_": [0, 0, 0, 0, 0b111],
    "#": [0b101, 0b111, 0b101, 0b111, 0b101], "$": [0b011, 0b110, 0b010, 0b011, 0b110],
    "@": [0b010, 0b101, 0b111, 0b100, 0b011],
}
BLANK = [0, 0, 0, 0, 0]


def bitmap_rows_for_unit(unit: int) -> list[int]:
    """media.rs's fallback rules for one UTF-16 code unit:
    ASCII case folds up, control characters are blank, a unit
    that is not a scalar value (surrogate half) is '?', anything
    else unknown is '?'."""
    if 0xD800 <= unit <= 0xDFFF:
        return FONT["?"]
    ch = chr(unit)
    up = chr(ord(ch) - 32) if "a" <= ch <= "z" else ch
    if up in FONT:
        return FONT[up]
    if unicodedata.category(ch) == "Cc":
        return BLANK
    return FONT["?"]


def glyph_commands(rows: list[int]) -> list[dict]:
    cmds = []
    for row in range(5):
        for col in range(3):
            if (rows[row] >> (2 - col)) & 1:
                x, y = float(col), float(row)
                cmds += [
                    {"op": "M", "values": [x, y]},
                    {"op": "L", "values": [x + 1.0, y]},
                    {"op": "L", "values": [x + 1.0, y + 1.0]},
                    {"op": "L", "values": [x, y + 1.0]},
                    {"op": "Z", "values": []},
                ]
    return cmds


def utf16_units(text: str) -> list[int]:
    blob = text.encode("utf-16-le", errors="surrogatepass")
    return [blob[i] | (blob[i + 1] << 8) for i in range(0, len(blob), 2)]


def build_bitmap_asset(text: str) -> tuple[dict, list[int], list[float]]:
    """The port's algorithm as an original-codec asset dict.
    Returns (asset, expected glyph node counts, expected visible
    placement x values)."""
    units = utf16_units(text)
    unique = list(dict.fromkeys(units))
    glyphs, node_counts = [], []
    for u in unique:
        cmds = glyph_commands(bitmap_rows_for_unit(u))
        glyphs.append({"commands": cmds, "utf16_code_unit": u})
        node_counts.append(len(cmds))
    placements, visible_x = [], []
    for i, u in enumerate(units):
        gi = unique.index(u)
        visible = bool(glyphs[gi]["commands"])
        placements.append({
            "x": 4.0 * i, "y": 0.0, "glyph_index": gi,
            "utf16_code_unit": u, "visible": visible,
        })
        if visible:
            visible_x.append(4.0 * i)
    asset = {
        "kind": "text", "asset_index": 0, "table_index": 0,
        "font_name": "Bitmap 3x5", "font_style": 0, "alignment": 0,
        "text": text, "glyphs": glyphs, "placements": placements,
    }
    return asset, node_counts, visible_x


def check_algorithm() -> None:
    # Covers: uppercase, lowercase fold, digit, space (invisible),
    # unknown '~' -> '?', non-ASCII 'é' -> '?', control character
    # (invisible), and the advance rule across all eight units.
    text = "Ab3 ~é\x01Z"
    asset, want_nodes, want_x = build_bitmap_asset(text)
    # Hand-pinned node counts (5 per lit pixel), independent of
    # the table above: A=10px, B=10px, '3'=7px, ' '=0, '?'=5px
    # twice, control=0, Z=9px.
    assert want_nodes == [50, 50, 35, 0, 25, 25, 0, 45], want_nodes
    segments = [{"segment_id": 2, "primitive_code": 5, "extra_u16": 0}]
    blob = encode_side_tables([asset], 0, segments)
    assets, end = parse_side_tables(blob, 0, segments)
    assert end == len(blob), (end, len(blob))
    back = assets[0]
    assert back["text"] == text, repr(back["text"])
    assert back["font_name"] == "Bitmap 3x5"
    assert [g["node_count"] for g in back["glyphs"]] == want_nodes
    assert len(back["placements"]) == len(utf16_units(text))
    visible_got = [p["x"] for p in back["placements"] if p["visible"]]
    assert visible_got == want_x, (visible_got, want_x)
    # Bounds: width = 4n - 1 units, height = 5 — the renderer's
    # own cell geometry, recovered by the original's bounds code.
    assert back["bounds"]["width"] == 4 * len(utf16_units(text)) - 1
    assert back["bounds"]["height"] == 5.0
    # First glyph ('A') starts with the square for pixel (1, 0).
    first = back["glyphs"][0]["commands"][:5]
    assert [c["op"] for c in first] == ["M", "L", "L", "L", "Z"]
    assert first[0]["values"] == [1.0, 0.0] and first[2]["values"] == [2.0, 1.0]
    print(f"  algorithm: bitmap table for {text!r} round-trips "
          f"({len(blob)} bytes, nodes {want_nodes})")


def build_fixture() -> bytes:
    """The shared Tripod scene with segment 2 as text, written by
    the original's full writer (same construction as
    gen_fixtures.build_v5)."""
    document = {"canvas_width": 800, "canvas_height": 600, "fps": 24}
    base = scaffold(document)
    mini = decode_piv_bytes(base)

    def box(x0, x1):
        return [
            {"op": "M", "values": [x0, 0.0]},
            {"op": "L", "values": [x1, 0.0]},
            {"op": "L", "values": [x1, 5.0]},
            {"op": "L", "values": [x0, 5.0]},
            {"op": "Z", "values": []},
        ]

    text_asset = {
        "kind": "text", "asset_index": 0, "table_index": 0,
        "font_name": "Arial", "font_style": 0, "alignment": 0,
        "text": "HI",
        "glyphs": [
            {"commands": box(0.0, 3.0), "utf16_code_unit": ord("H")},
            {"commands": box(1.0, 2.0), "utf16_code_unit": ord("I")},
        ],
        "placements": [
            {"x": 0.0, "y": 0.0, "glyph_index": 0, "utf16_code_unit": ord("H")},
            {"x": 4.0, "y": 0.0, "glyph_index": 1, "utf16_code_unit": ord("I")},
        ],
    }
    segments = [
        (0, 1, 50.0, -1.5707963267948966, 6.0, 0),
        (1, 2, 30.0, 0.0, 4.0, 5),  # segment 2 carries the text
        (2, 3, 16.0, 1.5707963267948966, 4.0, 1),
    ]
    legacy_fig = {
        "figure_type_id": 2, "name": "Tripod",
        "segments": [
            {
                "segment_id": sid, "parent_id": parent, "length": length,
                "angle_rad": angle, "thickness": thickness,
                "primitive_code": prim, "aux_code": 0, "flags": prim,
                "color_rgb": [255, 0, 0] if sid == 1 else [0, 255, 0] if sid == 2 else [0, 0, 255],
                "transparency": 0, "bend_rad": 0.0,
                "segment_kind": "circle" if prim == 1 else "line",
                "segment_type": "circle" if prim == 1 else "line",
                **({"extra_u16": 0, "extra_u8_a": 0, "extra_u8_b": 0} if prim == 5 else {}),
            }
            for (parent, sid, length, angle, thickness, prim) in segments
        ],
        "polyfills": [], "bends": [], "image_assets": [],
        "text_assets": [text_asset],
    }
    mini["figure_types"].append(migrate_figure(legacy_fig, 2))
    frames = [
        ([-1.5707963267948966, 0.0, 1.5707963267948966], (111.0, 222.0), 2),
        ([-1.5707963267948966, 0.5, 1.5707963267948966], (121.0, 222.0), 3),
    ]
    mini["keyframes"] = [
        {
            "keyframe_index": i, "duration_raw": dur,
            "duration_frames": dur, "inbetweens": dur - 1,
            "interpolation": "hold", "duration_semantics": "hold",
            "background_mix": 0, "background_id": 0, "background_id_b": None,
            "objects": [{
                "object_slot": 0, "unique_id": 42, "figure_type_id": 2,
                "scale": 1.0, "color_override_raw": 0x0000FF,
                "color_override_rgb": [255, 0, 0], "field28": 0,
                "image_text_flags": 0, "angles": list(angles),
                "root_x": root[0], "root_y": root[1],
                "len_flag": 0, "lengths": [], "bend_flag": 0,
                "bends": [], "join": None,
            }],
            "object_draw_order": [0],
            "camera": {"is_default": True, "x": 0.0, "y": 0.0,
                       "angle_rad": 0.0, "scale": 1.0},
        }
        for i, (angles, root, dur) in enumerate(frames)
    ]
    mini["fps"] = 24
    return encode_piv_project(base, mini, force_rebuild=True)


def check_fixture(data: bytes) -> None:
    report = decode_piv_bytes(data)
    assert report["format"] == "piv_v5_standalone"
    assert (report["canvas_width"], report["canvas_height"]) == (800, 600)
    assert report["fps"] == 24
    assert len(report["keyframes"]) == 2
    roots = [(o["root_x"], o["root_y"])
             for o in report["keyframes"][0]["objects"]]
    assert roots == [(111.0, 222.0)], roots
    fig = next(f for f in report["figure_types"] if f["name"] == "Tripod")
    prims = [(s["segment_id"], s["primitive_code"]) for s in fig["segments"]]
    assert prims == [(1, 0), (2, 5), (3, 1)], prims
    assert len(fig["text_assets"]) == 1
    asset = fig["text_assets"][0]
    assert asset["text"] == "HI" and asset["font_name"] == "Arial"
    assert [g["node_count"] for g in asset["glyphs"]] == [5, 5]
    assert [(p["x"], p["y"]) for p in asset["placements"]] == [(0.0, 0.0), (4.0, 0.0)]
    print(f"  fixture: text-v5.piv ({len(data)} bytes) decodes with the "
          "shared scene + text asset 'HI'")


def check_rust_export() -> None:
    """Parse the port's own exported media file, when a Rust test
    run has produced it, with the original's side-table parser."""
    path = Path("/tmp/styk-export-v5-media.piv")
    if not path.exists():
        print("  rust export: SKIP (/tmp/styk-export-v5-media.piv absent; "
              "it is written by the port's Rust tests)")
        return
    payload = zlib.decompress(path.read_bytes())
    pos = 0

    def take(fmt):
        nonlocal pos
        v = struct.unpack_from("<" + fmt, payload, pos)
        pos += struct.calcsize("<" + fmt)
        return v

    assert take("B")[0] == 5
    take("II")
    bg = take("H")[0]
    for _ in range(bg - 1):
        assert take("B")[0] == 2  # solid colour slot
        take("I")
        pos += payload[pos] + 1  # name str8
    take("H")  # figure slots
    flags = take("B")[0]
    assert flags == 144, flags
    count = take("H")[0]
    segments = []
    for i in range(count):
        take("H")  # parent
        take("H")  # id
        take("f")  # length
        take("d")  # angle
        take("f")  # thickness
        prim = take("B")[0]
        take("B")  # aux
        pos += 3  # rgb
        take("B")  # transparency
        extra = 0
        if prim in (2, 5):
            extra = take("H")[0]
            pos += 2
        segments.append({"segment_id": i + 1, "primitive_code": prim,
                         "extra_u16": extra})
    bends = take("H")[0]
    pos += bends * 10  # (u16 index, f64 value) per record bend
    assets, _end = parse_side_tables(payload, pos, segments)
    text = [a for a in assets if a["kind"] == "text"]
    assert len(text) == 1, assets
    asset = text[0]
    assert asset["text"] == "HI", asset["text"]
    assert asset["font_name"] == "Bitmap 3x5"
    # Bitmap node counts: H = 11 lit pixels, I = 9 -> 55 / 45.
    assert [g["node_count"] for g in asset["glyphs"]] == [55, 45]
    assert [(p["x"], p["y"]) for p in asset["placements"]] == [(0.0, 0.0), (4.0, 0.0)]
    print("  rust export: /tmp/styk-export-v5-media.piv text table parses "
          "with the original's parser (content 'HI', bitmap glyphs)")


def main() -> int:
    print("text side tables vs the original's codecs:")
    check_algorithm()

    data = build_fixture()
    check_fixture(data)
    FIXTURES.mkdir(parents=True, exist_ok=True)
    (FIXTURES / "text-v5.piv").write_bytes(data)
    manifest_path = FIXTURES / "manifest.json"
    manifest = json.loads(manifest_path.read_text())
    manifest["fixtures"]["text-v5.piv"] = {
        "format": "piv_v5_standalone",
        "canvas": [800, 600],
        "fps": 24,
        "figures": [{"name": "Tripod", "segments": 3}],
        "frames": 2,
        "frame0_roots": [[111, 222]],
        "notes": ("Shared Tripod scene; segment 2 is a TEXT segment "
                  "(primitive 5, extra 0) whose side table carries "
                  "'HI' in box-outline glyphs, font 'Arial'. Written "
                  "by the original's encoder via "
                  "scripts/verify_text_tables.py. The port's importer "
                  "recovers the text onto segment 2 (size 5*30/7)."),
    }
    # Match the committed manifest's formatting (indent 2, key
    # order preserved) so re-running this script stays a no-op
    # beyond the one entry.
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")
    print("  fixture written: tests/fixtures/text-v5.piv + manifest entry")

    check_rust_export()
    print("text table verification passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
