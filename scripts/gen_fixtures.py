#!/usr/bin/env python3
"""Generate the cross-format test corpus in tests/fixtures/.

Every fixture encodes ONE shared scene (see below) in a different Pivot
container, so an importer can be checked format-by-format against the
same expected values (tests/fixtures/manifest.json).

Fixtures:
  legacy-v2.piv  hand-packed per codecs/piv_legacy_v2.py (decode_v2)
  legacy-v3.piv  hand-packed per codecs/piv_legacy_v2.py (decode_v2)
  legacy-v4.piv  hand-packed per codecs/piv_legacy_v4.py (decode_v4)
  modern-v5.piv  produced by the ORIGINAL's own encoder
                 (codec_bridge.scaffold + piv_migrate._figure +
                 piv_codec.encode_piv_project, the export_stk pattern)
  figure.stk     hand-packed legacy figure stream (Layout B of
                 pivot_decoder._parse_legacy_payload: [count][records];
                 the legacy STK container carries no name field)

Shared scene:
  canvas 800x600; one custom figure "Tripod", 3 segments
  (lengths 50/30/16, segment 3 is a circle), red colour override
  (COLORREF 0x0000FF), two keyframes:
    frame 0: root (111, 222), angles = definition angles
    frame 1: root (121, 222), segment 2 angle +0.5 rad

Format limits (see docs/CORPUS.md): v2 stores no frame interval, so
its fps is the format default 13; the .stk carries no canvas, fps,
frames or name (decoders report the file stem as the name).

Run:  python3 scripts/gen_fixtures.py
"""
from __future__ import annotations

import json
import os
import struct
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
FIXTURES = REPO / "tests" / "fixtures"

# The original app's codecs (sibling checkout). Override with
# STYK_ORIGINAL_CODECS if the layout differs.
CODECS = Path(
    os.environ.get(
        "STYK_ORIGINAL_CODECS",
        Path.home() / "workspace" / "StykAnimate-App" / "codecs",
    )
)
sys.path.insert(0, str(CODECS))

# ---------------------------------------------------------------- scene

CANVAS = (800, 600)
FIGURE_NAME = "Tripod"
# (parent_id, segment_id, length, absolute angle rad, thickness, primitive)
SEGMENTS = [
    (0, 1, 50.0, -1.5707963267948966, 6.0, 0),  # pole, points up
    (1, 2, 30.0, 0.0, 4.0, 0),                  # arm, points right
    (2, 3, 16.0, 1.5707963267948966, 4.0, 1),   # circle at the end
]
RED_COLORREF = 0x0000FF  # COLORREF 0x00BBGGRR with R=255
# (angles per segment, root) per keyframe
FRAMES = [
    ([-1.5707963267948966, 0.0, 1.5707963267948966], (111.0, 222.0)),
    ([-1.5707963267948966, 0.5, 1.5707963267948966], (121.0, 222.0)),
]
OBJECT_UID = 42


def segment_record(parent, sid, length, angle, thickness, primitive):
    """One 24-byte legacy segment record (piv_legacy_v2._segment)."""
    return struct.pack(
        "<BBHfdfBBH", parent, sid, 0, length, angle, thickness, primitive, 0, 0
    )


def figure_stream() -> bytes:
    """Legacy figure stream: count u8, records, str8 name."""
    out = bytearray()
    out += struct.pack("<B", len(SEGMENTS))
    for seg in SEGMENTS:
        out += segment_record(*seg)
    name = FIGURE_NAME.encode("cp1252")
    out += struct.pack("<B", len(name)) + name
    return bytes(out)


def legacy_object_v23(angles, root) -> bytes:
    out = bytearray()
    out += struct.pack("<B", 2)  # figure type id (slot 1 is the builtin)
    out += struct.pack("<f", 1.0)  # scale
    out += struct.pack("<I", RED_COLORREF)
    for a in angles:
        out += struct.pack("<d", a)
    out += struct.pack("<ff", root[0], root[1])
    return bytes(out)


# ---------------------------------------------------------------- v2

def build_v2() -> bytes:
    out = bytearray()
    out += struct.pack("<B", 2)  # version
    out += struct.pack("<B", 0)  # has_background = false
    out += struct.pack("<II", *CANVAS)
    out += struct.pack("<B", 3)  # figure slots: builtin + Tripod
    out += figure_stream()
    out += struct.pack("<I", len(FRAMES))
    for angles, root in FRAMES:
        # v2 frames: no duration, no background byte.
        out += struct.pack("<B", 1)  # object count
        out += legacy_object_v23(angles, root)
    # No frame interval in v2 (fps is the format default, 13), no tail.
    return bytes(out)


# ---------------------------------------------------------------- v3

def build_v3() -> bytes:
    out = bytearray()
    out += struct.pack("<B", 3)  # version
    out += struct.pack("<II", *CANVAS)
    out += struct.pack("<B", 1)  # background slots: default slot 0 only
    out += struct.pack("<B", 3)  # figure slots: builtin + Tripod
    out += figure_stream()
    out += struct.pack("<I", len(FRAMES))
    for i, (angles, root) in enumerate(FRAMES):
        out += struct.pack("<H", 2 if i == 0 else 1)  # duration
        out += struct.pack("<B", 0)  # background id
        out += struct.pack("<B", 1)  # object count
        out += legacy_object_v23(angles, root)
    out += struct.pack("<I", 40)  # frame interval ms -> 25 fps
    out += struct.pack("<B", 1)  # optional legacy loop flag
    return bytes(out)


# ---------------------------------------------------------------- v4

def build_v4() -> bytes:
    out = bytearray()
    out += struct.pack("<B", 4)  # version
    out += struct.pack("<II", *CANVAS)
    out += struct.pack("<B", 1)  # background slots: default slot 0 only
    out += struct.pack("<H", 3)  # figure slots (u16): builtin + Tripod
    out += figure_stream()
    out += struct.pack("<I", len(FRAMES))
    for i, (angles, root) in enumerate(FRAMES):
        out += struct.pack("<H", 3 if i == 0 else 1)  # duration (nonzero)
        out += struct.pack("<B", 0)  # background id
        out += struct.pack("<H", 1)  # object count (u16)
        out += struct.pack("<I", OBJECT_UID)  # unique id
        out += struct.pack("<H", 2)  # figure type id
        out += struct.pack("<f", 1.0)  # scale
        out += struct.pack("<I", RED_COLORREF)
        out += struct.pack("<B", 0)  # transparency (field28)
        for a in angles:
            out += struct.pack("<d", a)
        out += struct.pack("<ff", root[0], root[1])
        out += struct.pack("<B", 0)  # length flag: no per-frame lengths
        out += struct.pack("<B", 0)  # join flag: not joined
        out += struct.pack("<H", 0)  # draw order: object 0 first
    out += struct.pack("<I", 40)  # frame delay ms -> 25 fps
    return bytes(out)


# ---------------------------------------------------------------- stk

def build_stk() -> bytes:
    """Legacy .stk, Layout B of pivot_decoder._parse_legacy_payload:
    [count u8][segment records] and nothing else. The container has
    no name field; decoders report the file stem as the figure name.
    """
    out = bytearray()
    out += struct.pack("<B", len(SEGMENTS))
    for seg in SEGMENTS:
        out += segment_record(*seg)
    return bytes(out)


# ---------------------------------------------------------------- v5

def build_v5() -> bytes:
    """A real v5 file from the original's own encoder."""
    from codec_bridge import scaffold
    from piv_codec import decode_piv_bytes, encode_piv_project
    from piv_migrate import _figure as migrate_figure

    document = {"canvas_width": CANVAS[0], "canvas_height": CANVAS[1], "fps": 24}
    base = scaffold(document)
    mini = decode_piv_bytes(base)

    legacy_fig = {
        "figure_type_id": 2,
        "name": FIGURE_NAME,
        "segments": [
            {
                "segment_id": sid,
                "parent_id": parent,
                "length": length,
                "angle_rad": angle,
                "thickness": thickness,
                "primitive_code": prim,
                "aux_code": 0,
                "flags": prim,
                "color_rgb": [255, 0, 0] if sid == 1 else [0, 255, 0] if sid == 2 else [0, 0, 255],
                "transparency": 0,
                "bend_rad": 0.0,
                "segment_kind": "circle" if prim == 1 else "line",
                "segment_type": "circle" if prim == 1 else "line",
            }
            for (parent, sid, length, angle, thickness, prim) in SEGMENTS
        ],
        "polyfills": [],
        "bends": [],
        "image_assets": [],
        "text_assets": [],
    }
    mini["figure_types"].append(migrate_figure(legacy_fig, 2))

    def keyframe(index, duration, angles, root):
        return {
            "keyframe_index": index,
            "duration_raw": duration,
            "duration_frames": duration,
            "inbetweens": duration - 1,
            "interpolation": "hold",
            "duration_semantics": "hold",
            "background_mix": 0,
            "background_id": 0,
            "background_id_b": None,
            "objects": [
                {
                    "object_slot": 0,
                    "unique_id": OBJECT_UID,
                    "figure_type_id": 2,
                    "scale": 1.0,
                    "color_override_raw": RED_COLORREF,
                    "color_override_rgb": [255, 0, 0],
                    "field28": 0,
                    "angles": list(angles),
                    "root_x": root[0],
                    "root_y": root[1],
                    "len_flag": 0,
                    "lengths": [],
                    "bend_flag": 0,
                    "bends": [],
                    "join": None,
                }
            ],
            "object_draw_order": [0],
            "camera": {"is_default": True, "x": 0.0, "y": 0.0, "angle_rad": 0.0, "scale": 1.0},
        }

    mini["keyframes"] = [
        keyframe(0, 2, FRAMES[0][0], FRAMES[0][1]),
        keyframe(1, 3, FRAMES[1][0], FRAMES[1][1]),
    ]
    mini["fps"] = 24
    return encode_piv_project(base, mini, force_rebuild=True)


# ---------------------------------------------------------------- main

def main() -> None:
    FIXTURES.mkdir(parents=True, exist_ok=True)
    built = {
        "legacy-v2.piv": build_v2(),
        "legacy-v3.piv": build_v3(),
        "legacy-v4.piv": build_v4(),
        "modern-v5.piv": build_v5(),
        "figure.stk": build_stk(),
    }
    for name, data in built.items():
        (FIXTURES / name).write_bytes(data)
        print(f"wrote tests/fixtures/{name} ({len(data)} bytes)")

    tripod = [{"name": FIGURE_NAME, "segments": len(SEGMENTS)}]
    frame0_roots = [[FRAMES[0][1][0], FRAMES[0][1][1]]]
    manifest = {
        "scene": (
            "One shared scene in every container: canvas 800x600, the "
            "3-segment 'Tripod' figure (lengths 50/30/16, segment 3 a "
            "circle), red colour override, frame 0 root (111, 222), "
            "frame 1 root (121, 222). 'figures' lists the custom "
            "figures only; every .piv also implies the format's "
            "built-in slot-1 figure."
        ),
        "fixtures": {
            "legacy-v2.piv": {
                "format": "piv_v2_legacy_exact",
                "canvas": list(CANVAS),
                "fps": 13,  # v2 stores no interval; 13 is the format default
                "figures": tripod,
                "frames": len(FRAMES),
                "frame0_roots": frame0_roots,
            },
            "legacy-v3.piv": {
                "format": "piv_v3_legacy_exact",
                "canvas": list(CANVAS),
                "fps": 25,  # 40 ms frame interval
                "figures": tripod,
                "frames": len(FRAMES),
                "frame0_roots": frame0_roots,
            },
            "legacy-v4.piv": {
                "format": "piv_v4_exact",
                "canvas": list(CANVAS),
                "fps": 25,  # 40 ms frame delay
                "figures": tripod,
                "frames": len(FRAMES),
                "frame0_roots": frame0_roots,
            },
            "modern-v5.piv": {
                "format": "piv_v5_standalone",
                "canvas": list(CANVAS),
                "fps": 24,
                "figures": tripod,
                "frames": len(FRAMES),
                "frame0_roots": frame0_roots,
            },
            "figure.stk": {
                "format": "legacy_stk",
                "canvas": None,
                "fps": None,
                "figures": [{"name": "figure", "segments": len(SEGMENTS)}],
                "frames": None,
                "frame0_roots": [],
                "notes": (
                    "The legacy STK container stores no name, canvas, "
                    "fps or frames. The original's decoder reports the "
                    "figure name as the file stem ('figure'); the "
                    "geometry is the same Tripod (lengths 50/30/16, "
                    "segment 3 a circle)."
                ),
            },
        },
    }
    (FIXTURES / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print("wrote tests/fixtures/manifest.json")


if __name__ == "__main__":
    main()
