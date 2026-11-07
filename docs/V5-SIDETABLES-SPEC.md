# .piv v5 — Image/Text Side Tables, Figure Flags, and Trailer: Byte-Level Spec

Source of truth: `StykAnimate-App/codecs/piv_side_tables.py` (complete) and
`piv_codec.py` (`_figure`, `_keyframes`, `_decode`, `Reader`). All integers are
little-endian. All offsets are positions in the **whole decompressed payload**
(source file is either raw — first byte in 1..5 — or a single zlib stream over
the payload). Grammar notation: `name: type — notes`.

Relevant framing inside a v5 payload: `version: u8 = 5`, `canvas_width: u32`,
`canvas_height: u32`, backgrounds (u16 slot count incl. implicit white slot 0;
each further slot: kind u8 — 0/1: u64 absolute blob end + blob; 2: color u32;
3/4: color_a u32, color_b u32, x1/y1/x2/y2 f32×4 — then name string8),
`figure_slots: u16` (≥ 2; slot 0 reserved, slot 1 is the implicit built-in
figure, stored figures follow for ids `2 .. figure_slots-1`), figures,
keyframes, `fps: u32`, trailer (see §5).

A `string8` (figure/background names) is: `length: u8`, then `length` bytes
decoded as cp1252. This is **not** the string type used in side tables (§2).

---

## 1. Image table entry (`_read_image`) — skip-precise

Fields in order:

- `anchor_start.x: i32`, `anchor_start.y: i32`, `anchor_end.x: i32`,
  `anchor_end.y: i32` — 16 bytes total. Source anchor segment of the image;
  render scale = world endpoint distance / anchor distance. Values may be
  any i32 (not validated for finiteness).
- `absolute_end: u64` — **absolute stream offset** at which the encoded image
  blob ends. It is NOT a byte length. Let `pointer` = offset of this u64
  field itself, and `image_start` = `pointer + 8` (first blob byte).
- `encoded_data: bytes[absolute_end - image_start]` — the raw encoded image
  (PNG/JPEG/BMP/GIF sniffed by magic for metadata only; sniffing never
  affects offsets). A skipping parser simply sets `pos = absolute_end`.

Branches (checked in this order):

- **Normal:** `absolute_end` in range → blob is `data[image_start:absolute_end]`.
  Skipping = seek to `absolute_end`. (Error if `absolute_end < image_start`
  or beyond the data.)
- **`unpatched_end` recovery:** if `absolute_end == pointer` exactly (a
  failed native save left the end placeholder pointing at its own field),
  the blob is empty (0 bytes) and the entry consumes exactly the 16 anchor
  bytes + 8 pointer bytes — do NOT rewind; final position is `image_start`.
  Decoders flag this entry (`image_record_recovery`) and treat the image
  as missing/transparent.
- **`omitted_end` recovery:** only when `allow_missing_end` is true for this
  entry (see §3). If `absolute_end > len(data)` AND the two bytes at
  `pointer` are `0x00 0x00`, then the u64 field was never written: the bytes
  at `pointer` are actually the figure's following fields (polyfill count
  u16 = 0) and the byte at `pointer + 2` is the figure-name string8 length,
  followed by the name bytes. If that name is non-empty, fits, and all bytes
  are printable (`>= 32`, `!= 127`), the entry consumes **nothing at all**:
  rewind `pos = pointer` so the rest of the figure parses from there.
  Otherwise fall through to the normal branch (which will fail on the
  out-of-range end).
- An empty blob in the normal branch is legal on disk; decoders substitute
  a 1×1 transparent PNG and mark `missing_image_data` (decode-time
  canonicalisation only — the byte layout is just a zero-length blob).

Skip recipe: read 4×i32, read u64 `end`; if `end == pos - 8` → done (empty);
else if recovery conditions → `pos -= 8`; else `pos = end` (validate
`image_start <= end <= len(data)`).

## 2. Text table entry (`_read_text`) — skip-precise

### String type (confirmed against `_write_string`)

- `length: u32` — count of **UTF-16 code units** (not bytes, not Unicode
  code points; a surrogate pair counts as 2 units and each unit is indexed
  separately, Delphi-style).
- `data: bytes[length * 2]` — UTF-16LE, decoded with surrogatepass.
- Writer emits exactly `u32(len(utf16_bytes) // 2)` + the UTF-16LE bytes.
  Read and write sides agree.

### Fields in order

- `font_name: string` — as above.
- `font_style: u8` — bit 0 bold, bit 1 italic, bit 2 underline, bit 3
  strikeout. Other bits unused by the reader.
- `alignment: u32` — opaque to the parser.
- `content: string` — the text; its code-unit list drives glyph mapping
  and placements below.
- `glyph_count: u8` — MUST equal the number of **unique** UTF-16 code
  units in `content`, in first-appearance order (error otherwise).
  Glyph *j* belongs to the *j*-th unique unit.
- `glyphs: glyph_path[glyph_count]` — see below.
- `placements: per code unit of content` (all units, duplicates
  included), in content order:
  - if the unit's glyph is **visible** (`node_count > 0`):
    `x: f32`, `y: f32` — character offset; must be finite.
  - if the glyph is **invisible** (`node_count == 0`, e.g. space):
    **zero bytes are stored for this character.**

There is **no colour field and no font-size field** anywhere in a text
asset. Colour comes from the owning segment/object; vertical scale is
`object.scale`, horizontal scale is world endpoint distance / text
bounds width.

### Glyph path (`_read_path`)

- `node_count: u32` — total expanded node count of the outline.
- `opcodes: packed nibble stream` — commands are packed **two per byte,
  low nibble first**. Opcode values: 0=`M`, 1=`L`, 2=`C`, 3=`Z`, 4=`V`,
  5=`H` (values > 5 are an error). Node expansion: `C` counts as **3**
  nodes, every other opcode counts as **1**. Bytes are consumed until
  the cumulative expansion reaches `node_count`; the final cumulative
  value must equal `node_count` exactly (a cubic that would cross the
  boundary is an error), and the unused high nibble of the last byte is
  ignored. There is no separate opcode-count field: **a skipping parser
  must simulate the expansion nibble-by-nibble to find where the opcode
  bytes end.**
- `coordinates: f32 stream` — stored **after all opcode bytes**, one
  group per command in command order, value counts: `M`=2, `L`=2,
  `C`=6, `Z`=0, `V`=1, `H`=1. All values must be finite.

Skip size for one glyph = `4 + opcode_bytes(simulation required) +
4 * (sum of per-command value counts)` bytes.

Bounds/SVG paths in the decoded model are derived data (bounds include
cubic control points); nothing extra is stored.

## 3. `parse_side_tables` — which tables exist, and their order

- Invoked from `_figure` **only if at least one segment has
  `primitive_code` 2 (image) or 5 (text)**, immediately after the figure's
  bend list and before the polyfill count. If neither primitive occurs,
  no side-table bytes exist at all.
- Table presence is decided **per kind from the segment set**, not from
  any stored flag/count: image table iff some segment has prim 2; text
  table iff some segment has prim 5.
- Fixed order: **image table first, then text table** (regardless of
  segment order in the figure).
- Each present table: `table_count: u16`, then `table_count` entries of
  that kind (§1 / §2). An absent kind consumes **zero bytes** — not even
  a count.
- `allow_missing_image_end` is passed down per entry and is true **only**
  for the **last** image entry, **only** when no text table exists in the
  same figure, and **only** when the caller allowed it (figure flags:
  bit 16 set AND bit 64 clear — see §4).
- Each prim-2/prim-5 segment binds to an asset via its `extra_u16`
  (the u16 of the segment's `extra` triple: `extra_u16: u16`,
  `extra_u8_a: u8` — reverse endpoints / +π rotation, `extra_u8_b: u8` —
  baseline reflection, XORed with the frame object's `image_text_flags`
  byte at render). Indices are zero-based and **separate per kind**;
  an out-of-range reference is an error.

## 4. Figure flags byte (first byte of every stored `_figure`)

One `flags: u8` precedes `segment_count: u16`
(`0 < count <= 30000`). All bits describe **omitted/defaulted fields for
the whole figure** — they change how every following field is read:

- **bit 0 (value 1) — primitives omitted:** no `primitive_code: u8` per
  segment; every segment's primitive is 0 (line).
- **bit 1 (value 2) — aux omitted:** no `aux_code: u8` per segment;
  every segment's aux is 0 (so no segment is static: aux bit 0 is the
  static flag).
- **bit 2 (value 4) — colours omitted:** no `color_rgb: u8×3` per
  segment; every segment's colour is `[0, 0, 0]`.
- **bit 3 (value 8) — transparency omitted:** no `transparency: u8` per
  segment; every segment's transparency is 0.
- **bit 4 (value 16) — implicit z-order:** the `segment_z: u16[count]`
  and `polyfill_z: u16[polyfill_count]` arrays after the polyfills are
  **absent**; z-order is identity (`segment i → z i`,
  `polyfill j → z count + j`). Side effect: this bit (together with bit
  64 being clear) is what enables the image `omitted_end` recovery (§1, §3).
- **bit 5 (value 32) — implicit segment ids:** no `segment_id: u16` per
  segment; ids are `index + 1` in record order. (`parent_id: u16` is
  always present.)
- **bit 6 (value 64) — global style block present:** after polyfills and
  the z-order arrays (if any), before the figure name:
  `u28: u32` (its bit pattern reinterpreted as f32 = outline thickness),
  `rgb: u8×3`, `u30: u8`
...[truncated 3391 chars]