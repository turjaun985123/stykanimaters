# Stick Nodes `.nodes` format — byte grammar spec

Derived from the original app's decoder source (no guessing):
`StykAnimate-App/src/sticknodes/decoder/decoder.mjs` (`parseNodes`, based on
sticknodes-rs, MIT), `decoder/binary.mjs`, `decoder/geometry.mjs`,
`decoder/renderer.mjs` (`layoutFigure`), `decoder/project.mjs`, and
`src/sticknodes/adapter.mjs` (conversion to native figures).

## 0. What a `.nodes` file is

A `.nodes` file is a **single figure definition** (a node tree with rest-pose
geometry). It has **no keyframes, poses or timeline**; opening one inserts an
editable figure into the scene. Animation lives in `.nodemc` (movie clips) and
`.stknds` (full projects), which embed `.nodes` figures — those containers are
out of scope for this spec.

Corpus-verified profiles in the original decoder: supplied native loader
431/117; corpus-backed figure profiles extend to 434/121. Untested
version/build pairs are attempted with the same version-based field rules.

## 1. Container / compression

* Input ceiling 32 MiB; decompressed payload ceiling 512 MiB.
* The payload may be raw, a zlib (deflate) stream, or gzip. The unwrapper
  scans the first 4096 bytes for a compression signature:
  * gzip: `1F 8B 08`
  * zlib: `(b & 0x0F) == 8 && (b >> 4) <= 7 && ((b<<8)+next) % 31 == 0 && (next & 0x20) == 0`
  * Each candidate offset is tried in order; the first stream that inflates
    successfully wins.
* Exception: a raw figure is recognised first and never mistaken for a
  compressed stream — big-endian i32 version at offset 0 is ≥ 160, there is
  no compression signature at offset 0, and the byte at the root-node offset
  (12 for version < 403, 16 for version ≥ 403) is `0xFF` (node type −1).
* Stick Nodes project files may instead carry a 9-byte prefix
  `01 02 03 04 05 06 07 08 09` followed by gzip — that is the `.stknds`
  container, not a standalone `.nodes`.

## 2. Endianness

* All multi-byte integers/floats use one byte order for the whole payload.
* Modern raw files (version ≥ 160) are **big-endian**.
* Legacy files (version < 160) only occur compressed, and their payload is
  **little-endian**. A standalone big-endian parse of a version < 160 payload
  is rejected (`LEGACY_ENDIAN`); the file-level decoder tries big-endian
  first, then little-endian, and uses the first that parses.

## 3. Primitive readers

* `i8`, `i16`, `i32`, `f32` in the payload byte order. `f32` must be finite.
* `flag`: one byte; any nonzero value is true (values > 1 are a diagnostic in
  the tracing reader, an error in the strict binary reader).
* `color`: 4 bytes in fixed order **[alpha, blue, green, red]** (ABGR),
  independent of payload endianness.
* `count`: `i32`, range-checked per call site (see limits in §8).
* Strings (length-prefixed UTF-8) exist in project containers but **not**
  in the `.nodes` figure grammar.

## 4. Header

| field | type | notes |
|---|---|---|
| version | i32 | ≥ 1; gates most of the layout below |
| build | i32 | only if version ≥ 403; ≥ 0 |
| scale | f32 | figure-wide scale factor |
| color | color | figure colour (used by nodes without their own colour) |

`modern(b)` below means `version >= 403 && build >= b`.

## 5. Node tree

Nodes are stored as a **preorder traversal**: a node's record is followed
immediately by the records of its children, in order. The first record must
be the root (type −1); no other record may be type −1. Each node has a
`draw_order_index` (i32, 0..1_000_000, unique across the figure) used as its
id everywhere (children lists, polyfills, connections).

Node types (i8): −1 root, 0 rounded-segment, 1 segment, 2 circle,
3 triangle, 4 filled-circle, 5 ellipse, 6 trapezoid, 7 polygon.

### 5.1 Connector block (conditional, read first)

If the parent flagged this child as a connector (see §5.3), the record
starts with:

| field | type | gate |
|---|---|---|
| local_x | f32 | |
| local_y | f32 | |
| percent | f32 | |
| percent_default | f32 | build ≥ 44 |
| value | f32 | |
| method | i32 | |
| reversed | flag | |
| smart_stretch_ancestral_value | f32 | build ≥ 65 |

### 5.2 Node record (in order)

| # | field | type | gate |
|---|---|---|---|
| 1 | node_type | i8 | |
| 2 | draw_order_index (id) | i32 | |
| 3 | is_static | flag | |
| 4 | is_stretchy | flag | |
| 5 | is_floaty | flag | modern(48) |
| 6 | is_smart_stretch | flag | version ≥ 248 |
| 7 | do_not_apply_smart_stretch | flag | version ≥ 252 |
| 8 | smart_stretch_reset_impulse | flag | modern(50) |
| 9 | use_segment_color | flag | |
| 10 | use_circle_outline | flag | version ≥ 256 |
| 11 | circle_is_hollow | flag | modern(21) |
| 12 | use_gradient | flag | version ≥ 176 |
| 13 | reverse_gradient | flag | version ≥ 176 |
| 14 | gradient_mode | i16 | modern(20) |
| 15 | use_segment_scale | flag | |
| 16 | local_x | f32 | (see §7 — unused for figure layout) |
| 17 | local_y | f32 | |
| 18 | scale | f32 | per-node scale (applied iff use_segment_scale) |
| 19 | default_length | f32 | |
| 20 | length | f32 | segment length |
| 21 | default_thickness | i32 | |
| 22 | thickness | i32 | |
| 23 | segment_curve_radius_and_default_curve_radius | i32 | version ≥ 320 |
| 24 | curve_circulization | flag | modern(20) |
| 25 | segment_curve_polyfill_precision | i16 | modern(21) |
| 26 | half_arc | flag | version ≥ 256 |
| 27 | right_triangle_direction | i16 | version ≥ 256 |
| 28 | triangle_upside_down | flag | version ≥ 300 |
| 29 | trapezoid_thickness_start | i32 if build < 64 else f32 | modern(36) |
| 30 | trapezoid_thickness_end | i32 if build < 64 else f32 | modern(36) |
| 31 | reserved_build36_a, reserved_build36_b | i32 ×2 | modern(36) && build == 36 |
| 32 | use_trapezoid_thickness_start, use_trapezoid_thickness_end | flag ×2 | modern(36) && build == 36 |
| 33 | trapezoid_top_thickness_ratio | f32 | version ≥ 256 && build ≠ 36 |
| 34 | trapezoid_is_rounded_start | flag | modern(36) |
| 35 | trapezoid_is_rounded_end | flag | modern(36) |
| 36 | num_polygon_vertices | i16 | version ≥ 256 (default 5 when absent) |
| 37 | default_local_angle | f32 | version ≥ 248 |
| 38 | local_angle | f32 | degrees, relative to parent (§7) |
| 39 | default_angle | f32 | version ≥ 248 |
| 40 | color | color | |
| 41 | gradient_color | color | version ≥ 176 |
| 42 | circle_outline_color | color | version ≥ 256 |
| 43 | is_angle_locked | flag | modern(39) |
| 44 | reserved_angle_lock_39_50 | f32 | modern(39) && build ≤ 50 |
| 45 | angle_lock_is_main_node | flag | modern(51) |
| 46 | angle_lock_offset_minuend, angle_lock_offset_subtrahend | f32 ×2 | modern(51) && build ≤ 56 |
| 47 | angle_lock_offset | f32 | modern(57) |
| 48 | angle_lock_relative_start | f32 | modern(63) |
| 49 | angle_lock_stickfigure_start | f32 | modern(67) |
| 50 | angle_lock_relative_multiplier | i8 | modern(63) |
| 51 | is_drag_locked | i16 (build ≤ 40) else flag | modern(39) |
| 52 | reserved_drag_lock_41_45 | i16 | modern(41) && build ≤ 45 |
| 53 | drag_lock_angle | f32 | modern(46) |
| 54 | smart_stretch_multiplier | f32 | modern(41) |
| 55 | reserved_smart_stretch_41_45 | u8 | modern(41) && build ≤ 45 |
| 56 | child_count | count | max 10_000 |
| 57 | child_connector_flags | i32 × child_count | modern(38) |

### 5.3 Children

After a node's record, its `child_count` child subtrees follow in preorder.
Child *j*'s connector block (§5.1) is present iff
`child_connector_flags[j] != 0`; without modern(38) there are no flags and
no child is a connector.

## 6. Footers (after the tree)

* **Polyfills** (version ≥ 230): count i32 (max 10_000); per polyfill:
  anchorId i32, color, use_polyfill_color flag, attached_count (max 10_000),
  attachedIds i32 × attached_count.
* **Connections** (modern(38)): count i32 (max 10_000); per connection:
  sourceId i32, targetId i32. A connection makes the source node's
  connector anchor to the target node's endpoint (§7).
* Trailing bytes after the footers are tolerated by the original decoder
  (reported as a warning, parse marked partial).

## 7. Geometry semantics (`layoutFigure` in renderer.mjs)

* Coordinates: **Y down** on screen; stored angles are **degrees in a Y-up
  convention**. A node's absolute angle is
  `Θ(node) = Θ(parent) + local_angle(node)`, and its endpoint is
  `start + (cos Θ°, −sin Θ°) · L`. So stored 0° = right, 90° = up,
  −90° = down on screen.
* The root's start and end are both (0, 0); its length is ignored. Its
  local angle is forced to 0 when build < 36.
* `L = length · (use_segment_scale ? scale : 1) · figure.scale`.
  Drawn thickness = `|thickness| · |(use_segment_scale ? scale : 1) · figure.scale|`.
  If L < 0 the segment points the opposite way (absolute angle += 180, and
  descendants accumulate from the flipped angle).
* `local_x` / `local_y` do **not** affect figure layout (they are only used
  by cached pose records in project containers).
* Connector nodes: the endpoint is driven by the connection target's
  endpoint and the connector fraction (percent/value/method, reversed) —
  a constraint, not plain FK.
* Angle-locked and smart-stretch/floaty nodes carry solver settings; the
  stored `local_angle` in a figure file is the authored rest pose. The
  original importer preserves stored poses and does not recreate the
  procedural dynamics.

## 8. Limits (original decoder)

Input 32 MiB, expanded payload 512 MiB, 10_000 nodes, depth 1_024,
10_000 polyfills, 10_000 connections, ids ≤ 1_000_000.

## 9. Conversion to the StykAnimate-RS model (`src/nodes.rs`)

Model conventions (`src/model.rs`): angles are relative degrees,
`dir(d) = (sin d, cos d)` in Y-down screen space (0° = down, positive
clockwise); a pose is a root position plus one relative angle per node;
node 0 is a synthetic origin whose absolute angle is 0.

* Absolute-angle mapping: Stick Nodes screen direction for absolute
  angle Θ is `(cos Θ, −sin Θ)`; solving `(sin d, cos d) = (cos Θ, −sin Θ)`
  gives **`d = Θ + 90`**.
* The Stick Nodes root becomes model node 1 (length 0) with
  `rest_angle = norm_deg(Θ_root + 90)`; every other node's
  `rest_angle = norm_deg(local_angle)` plus the 180° flip when its
  effective length is negative (the flip accumulates into descendants
  exactly as in `layoutFigure`). Effective lengths/scales are baked in
  per §7.
* Colours: node colour = node.color if use_segment_color, else the
  figure header colour (kept on the instance; node colour `None`).
  ABGR bytes → `0xRRGGBB` (alpha dropped, as in the pivot importer).
* Shape mapping: circle / filled-circle → `Shape::Circle`;
  triangle → `Polygon(3)`; polygon → `Polygon(num_polygon_vertices)`
  (default 5, clamped 3..=255); ellipse → `Circle` (approximation);
  segment / rounded-segment / trapezoid → `Line` (approximation).
  Note the model's circle is centred on the node endpoint with radius
  length/2, while Stick Nodes centres it on the segment midpoint with
  radius (L+T)/2 — shape *placement* is approximate.
* Polyfills and connections are parsed (byte-exact) but not modelled;
  connector nodes import as ordinary segments using their stored
  local angle/length, and angle-lock solver fields are consumed but not
  simulated — mirroring the original importer's "stored poses preserved,
  dynamics not recreated" stance.
* The imported project has one figure, one rest frame (fps 24, as the
  original adapter uses for standalone `.nodes`), stage 960×720, figure
  root placed at the stage centre.
