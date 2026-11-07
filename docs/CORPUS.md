# Cross-Format Test Corpus — `tests/fixtures/`

One shared animation scene encoded in every Pivot container format, so
an importer can be tested format-by-format against the same expected
values. The expected values live in `tests/fixtures/manifest.json`;
`scripts/verify_fixtures.py` proves the original app's own decoders
reproduce every one of them from the fixture bytes.

## The shared scene

- Canvas 800×600.
- One custom figure, **Tripod**: 3 segments — lengths 50 / 30 / 16,
  thicknesses 6 / 4 / 4, absolute angles −π/2, 0, +π/2; segment 3 is a
  circle (primitive 1). (Every `.piv` additionally implies the format's
  built-in slot-1 figure, which is not part of the scene.)
- Red colour override (COLORREF `0x0000FF`).
- Two keyframes: frame 0 root (111, 222) with the definition angles;
  frame 1 root (121, 222) with segment 2 rotated +0.5 rad.

## Fixtures

| File | Format | How it was made |
| --- | --- | --- |
| `legacy-v2.piv` | Pivot v2 (179 B) | Hand-packed per `codecs/piv_legacy_v2.py`: version byte, `has_background = 0`, canvas, figure slots, frames with no durations and no background byte, no frame interval. |
| `legacy-v3.piv` | Pivot v3 (190 B) | Hand-packed per the same module: background-slots byte after the canvas, per-frame durations and background ids, 40 ms frame interval (→ 25 fps), optional loop byte. |
| `legacy-v4.piv` | Pivot v4 (212 B) | Hand-packed per `codecs/piv_legacy_v4.py`: u16 figure-slot count, per-object unique ids (42), per-frame draw-order array, 40 ms frame delay (→ 25 fps). |
| `modern-v5.piv` | Pivot v5 (115 B) | **Not** hand-packed: produced by the original's own encoder — `codec_bridge.scaffold` + `piv_migrate._figure` + `piv_codec.encode_piv_project`, the same pattern `codec_bridge.export_stk` uses. Zlib-wrapped, exactly as the encoder emits it. 24 fps. |
| `figure.stk` | Legacy stick figure (73 B) | Hand-packed legacy figure stream, Layout B of `pivot_decoder._parse_legacy_payload`: `[count u8][3 × 24-byte segment records]` and nothing else (1 + 3×24 = 73 bytes). |

## Format limits — where the scene is not represented exactly

- **v2 fps is 13, not a scene value.** The v2 container stores no
  frame interval; the format's decoders always report the native
  default of 13 fps. The manifest records 13.
- **`.stk` has no name, canvas, fps, or frames.** The legacy STK
  container is a bare figure stream. The original's decoder
  (`codec_bridge.stk_document`) reports the figure's name as the file
  stem — so the manifest expects the name `"figure"` for this fixture
  — and synthesises its own canvas/fps when building a document; the
  manifest therefore records `null` for canvas, fps, and frames. The
  geometry is the full Tripod (same lengths, angles, circle segment).
- **Frame durations differ per format** (v2 has none; the others use
  small hold durations). Durations are deliberately not part of the
  manifest contract.

## Regenerate and verify

```sh
python3 scripts/gen_fixtures.py       # rewrites tests/fixtures/*
python3 scripts/verify_fixtures.py    # decodes with the original's codecs
```

Both scripts need the original app's codecs, expected at
`~/workspace/StykAnimate-App/codecs` (override with the
`STYK_ORIGINAL_CODECS` environment variable). Verify prints one
`PASS` line per fixture and exits non-zero on any mismatch.
