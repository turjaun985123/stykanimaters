# Stick Nodes animation containers — `.stknds` / `.nodemc` byte grammar spec

Derived from the original app's decoder source (no guessing):
`StykAnimate-App/src/sticknodes/decoder/project.mjs` (`openProject`,
`parseProjectPayload`, `parseMovieClipPayload`, `readInstance`, `readJoins`),
`decoder/project-layout.mjs` (`compilePoseLayout`, `readStyle`,
`readInstanceHeader`, `readProjectHeader`, `readMainSettings`,
`readProjectText`, `readCamera`, `readProjectTrailer`),
`decoder/binary.mjs` (`BinaryReader`), `decoder/decoder.mjs` (`parseNodes`,
embedded figure records — see `docs/NODES-SPEC.md`), `decoder/renderer.mjs`
(`layoutFigure`), `decoder/project.mjs` (`ProjectDocument.posedFigure`), and
`src/sticknodes/adapter.mjs` (the original's own conversion of a parsed
document into an editable timeline). Layout evidence in the original covers
loader profiles through 431/117; independently supplied projects cover
417/35, 425/100, 426/104 and 430/115 (`project.mjs` validation notes).
Untested version/build pairs are attempted with the same version-based
field rules and must still satisfy exact-EOF and reference checks.

## 0. What these files are

* `.stknds` — a full Stick Nodes **project**: a figure library, sound /
  sprite / movieclip libraries, a main timeline of keyframes, and an
  editor trailer.
* `.nodemc` — a standalone **movie clip**: its own local figure / sprite /
  sound libraries and a single clip timeline. The original treats it as
  "a single clip with a local reference namespace" (`project.mjs`).

Both embed `.nodes` figure definitions (grammar in `docs/NODES-SPEC.md`)
and per-frame **pose records** for every placed figure instance.

The original decoder dispatches on the *filename* (`.nodemc` →
`parseMovieClipPayload`, otherwise `parseProjectPayload`). The Rust
importer has no filename, so it attempts the `.stknds` grammar first and
then the `.nodemc` grammar; both require the payload to be consumed
**exactly to EOF** (the original fails with `TRAILING_PROJECT_BYTES` /
`TRAILING_MOVIECLIP_BYTES` otherwise), which makes a false dispatch
practically impossible.

## 1. Container / compression (`openProject`)

Payload ceiling 512 MiB. Exactly three wrappers exist:

* **9-byte prefix + gzip**: bytes `01 02 03 04 05 06 07 08 09`, then the
  gzip magic `1F 8B` at offset 9 (any other bytes there are
  `BAD_CONTAINER`). The payload is the gzip stream inflated from offset 9.
* **gzip**: `1F 8B` at offset 0 → inflate the whole input as gzip.
* **raw**: anything else is the payload itself.

There is **no** zlib form and **no** signature scan for these containers
(unlike standalone `.nodes`).

## 2. Primitive encodings (`BinaryReader`, big-endian throughout)

* All multi-byte values are **big-endian** (the reader wraps `DataView`
  with no little-endian flag). This includes the embedded `.nodes`
  figures: when embedded, `parseNodes` runs big-endian regardless of the
  standalone legacy little-endian fallback.
* `i8`/`i16`/`i32`/`f32`: 1/2/4/4 bytes. An `f32` must be finite.
* `flag`: one byte, must be `0` or `1` (the project reader fails with
  `NON_CANONICAL_BOOL` otherwise — stricter than the `.nodes` reader).
* `color`: 4 bytes in **[alpha, blue, green, red]** order.
* `string`: `i32` byte length (0..=1 MiB) followed by that many UTF-8
  bytes (invalid UTF-8 is an error).
* `count(max)`: an `i32` validated to `0..=max`.

Version gates below use `v` = the **container** version and `b` = the
**container** build, except inside embedded figures, which carry their
own version/build (figure build is read only when figure version ≥ 403,
mirroring `parseNodes`).

## 3. `.stknds` payload (`parseProjectPayload`)

```
i32  version                          (must be ≥ 1)
i32  build                            (only when version ≥ 401, else build = 1)
string name
project header        (§4)
figure library        (§6)   count i32; per figure: string name + embedded figure (§10)
sound library         (§7)   only when v ≥ 160: count; per sound: string name
sprite library        (§8)   only when v ≥ 400
movieclip library     (§9)   only when v ≥ 300
main timeline         (§12)  count; per frame: main frame (§12)
trailer               (§13)
EOF (exact)
```

Semantic validation the original applies (and the Rust importer mirrors
as errors): header FPS and every clip FPS must be in `1..=1000`; frame
sound indices must be `-1` or a valid sound index; instance asset
references must exist and sprite instance types must match the asset
kind; clip cursor indices on movieclip instances must be inside the
referenced clip.

## 4. Project header (`readProjectHeader`)

Record-style fields; a field marked *(gate, fallback)* is **absent from
the byte stream** when the gate is false (the fallback is semantic only).

```
flag   editorFlagD                    (b ≥ 16)
— when v ≥ 220:
  flag   editorFlagE                  (v ≥ 244)
  string projectCredit
  color  editorColor
i32    fps
i32    tweenCount                     (v ≥ 176, fallback 2)
flag   editorFlagB                    (v ≥ 127)
flag   editorFlagZ                    (v ≥ 113)
flag   editorFlagA
flag   editorFlagC                    (v ≥ 220)
flag   autoTween
flag   timelineFlagC                  (v ≥ 127)
flag   legacyFlag                     (140 ≤ v < 200)
i32    selectedFrame
i32    nextTextId                     (v ≥ 176)
i32    nextInstanceId                 (v ≥ 300)
i32    nextLibraryId                  (v ≥ 300)
— when v ≥ 400: count fontCount; fontCount × flag fontUsage
— order lists: (v ≥ 400 ? 3 : v ≥ 300 ? 2 : 0) lists, each:
    count n; n × i32
— when v ≥ 300: library metadata:
    count outer; per entry: i32 key, count inner; per inner:
    i32 innerKey, count pairs; per pair: i32, i32
— legacy background, only when 140 ≤ v < 400 and a flag byte is 1:
    flag legacyFit                    (v < 170)
    f32 scaleX, f32 scaleY            (v ≥ 170)
    count width (≤ 32768), count height (≤ 32768)
    width*height*3 bytes of RGB pixel data
```

## 5. Instance style block (`readStyle`)

Read inside every instance header (§11), immediately after `id` (and
after `scale` for figure instances).

```
f32  opacity                          (v ≥ 220, fallback 1)
f32  blur                             (v ≥ 220, fallback 0)
— when b ≥ 16: flag motionBlur, flag omnidirectionalBlur, i32 motionBlurAngle
— else when v ≥ 220: flag legacyHorizontalBlur, flag legacyVerticalBlur
f32  saturation                       (v ≥ 220, fallback 1)
i32  pixelation                       (b ≥ 16, fallback 0)
color tintColor                       (v ≥ 220)
f32  tintAmount                       (v ≥ 220, fallback 0)
f32  hueShift                         (v ≥ 303, fallback 0)
flag invert                           (v ≥ 220, fallback false)
f32  glow                             (v ≥ 230, fallback 0)
color glowColor                       (v ≥ 230)
f32  glowIntensity                    (v ≥ 242, fallback 1)
color outlineColor                    (b ≥ 32)
f32  outlineAlpha                     (b ≥ 32, fallback 0)
f32  outlineThickness                 (b ≥ 32, fallback 0)
color dropShadowColor                 (v ≥ 330)
f32  dropShadowAlpha                  (v ≥ 330, fallback 0)
i32  dropShadowDistance               (v ≥ 330, fallback 0)
i32  dropShadowAngle                  (v ≥ 330, fallback 0)
f32  dropShadowBlur                   (v ≥ 330, fallback 0)
```

## 6. Figure library

`count` figures; each is `string name` followed immediately by an
embedded `.nodes` figure (§10) whose byte length is whatever
`parseNodes` consumes — the next library entry starts at the figure's
end offset. A figure with error diagnostics aborts the import
(`INVALID_FIGURE_REFERENCES`).

## 7. Sound library

Names only (`count`; per sound `string name`). Sounds are **external
references** — no audio bytes exist in the file (the adapter retains the
names as metadata and warns that the external files are needed).

## 8. Sprite library (`.stknds`, v ≥ 400)

Per sprite: `u8 type` (0 = image, 1 = group), `string name`, then:

* group: `count n`; n × `i32` member indices (must reference images).
* image:
  ```
  flag alpha, flag flag1
  f32 pivotX, f32 pivotY, f32 field2, f32 field3
  flag samplingFlag                   (b ≥ 9; assumed true before)
  count width (≤ 32768), count height (≤ 32768)
  count pixelBytes                    (must equal width*height*(alpha ? 4 : 3))
  pixelBytes of raw pixel data        (RGB or RGBA, row-major)
  ```

## 9. Movieclip library (`.stknds`, v ≥ 300)

Per clip:

```
string name
count  frameCount
count  fps                            (count type; validated 1..=1000)
count  tweenCount
flag   autoTween (flags[0])
flag   flags[1]
i32    nextInstanceId
frameCount × clip frame (§11)
i32    libraryId
```

## 10. Embedded figures and pose records

Embedded figures use the `.nodes` grammar (`docs/NODES-SPEC.md`)
big-endian at the current offset. Per figure the container records:
node count, node ids, **connector node sequence indices** (nodes whose
parent marked them as connector children), polyfill count, and the
figure's own version/build.

Every figure instance in every frame is followed by **one pose record
per definition node** (in node sequence order). The record layout is
compiled from the **container** version/build by `compilePoseLayout`
(§14) — *not* from the figure's own version. Connector nodes use the
connector variant of the layout, which appends fields (§14). Pose
records are validated field-by-field by the original
(flags must be 0/1, floats finite).

## 11. Frames and instances

### Clip frame (`readClipFrame`) — used by movieclip libraries and `.nodemc`

```
f32  volume
f32  pan                              (v ≥ 310, else 0)
f32  pitch                            (v ≥ 310, else 1)
i32  soundIndex                       (-1 or a sound index)
count instanceCount; instances (§11.1, clip context)
joins (§11.2, always present)
```

### 11.1 Instance (`readInstance` + `readInstanceHeader`)

```
i32  type                             (only when v ≥ 400 in clip context,
                                       v ≥ 300 in main context; else 0)
i32  assetIndex                       (index into the type's library)
— header:
i32  id                               (persistent instance id)
f32  scale                            (type 0 only, before the style block)
style block (§5)
— type 0 (figure):
  f32 x, f32 y, color color
  flag isLocked (v ≥ 126), flag persistent (v ≥ 180)
  flag visibleInOnionSkin (b ≥ 15)
  flag tweenColors (v ≥ 230)
  tweenMode: i8 when b ≥ 78 else flag (v ≥ 230)
  flag scaleJoinedFigures (v ≥ 244)
  flag pushJoinedFigures (v ≥ 400), flag setStateOfJoins (v ≥ 400),
  i32 joinsState (v ≥ 400)
  flag useParentFilters (v ≥ 310)
  i32 lockedNodeId (v ≥ 248), f32 lockedNodeX (v ≥ 248), f32 lockedNodeY (v ≥ 248)
  — then the pose records (§10) for the referenced figure
— type 1 (movieclip; forbidden inside clip frames):
  flag useParentFilters (v ≥ 310)
  f32 scale, f32 x, f32 y
  i32 currentFrameIndex, i32 startFrameIndex
  i32 flipX, i32 flipY, i32 playMode
  f32 volumeScale
  flag playDuringDelay, flag isLocked, flag persistent
  i8  tweenMode (b ≥ 78; absent before)
  flag visibleInOnionSkin (b ≥ 15)
  f32 rotation, f32 secondaryRotation
— type 2 (sprite image) / type 3 (sprite group):
  flag useParentFilters               (always present)
  u8 legacySpriteByte0, u8 legacySpriteByte1   (b < 2)
  f32 scaleXYRatio, f32 scaleX, f32 scaleY
  — when b ≥ 110: for corners Tl, Tr, Bl, Br: f32 distortX, f32 distortY
  f32 pivotX, f32 pivotY, f32 x, f32 y, f32 rotation
  flag flipX, flag flipY, flag persistent
  i8 tweenMode (b ≥ 78; absent before)
  flag visibleInOnionSkin (b ≥ 15)
  i32 lockState, i32 smartStretchState
  — when smartStretchState ≠ 0: 9 × f32 smart-stretch parameters
  i32 groupFrame                      (type 3 only)
```

Type meanings: 0 figure, 1 movieclip, 2 sprite image, 3 sprite group.
Playback modes on type 1 (adapter): `playMode` 0 = single, 1 = hold,
anything else = loop; `currentFrameIndex`/`startFrameIndex` are the
saved clip cursors.

### 11.2 Joins (`readJoins`)

Present in clip frames always; in main frames only when v ≥ 174 (when
absent, **no count is read at all**).

```
count n; per join:
  i32 child                           (index into this frame's instances)
  i32 parentId                        (an instance *id*, not an index)
  i32 nodeId                          (target node id on the parent figure)
  f32 relativeRotation                (b ≥ 53)
```

### 12. Main frame (`readMainFrame`) — `.stknds` only

```
main settings:
  flag isTweened
  flag useImageBackground             (v ≥ 140, fallback true)
  flag useGradient (v ≥ 200), flag expandedGradient (v ≥ 200)
  flag stopSounds                     (v ≥ 160)
  flag isDelayed (v ≥ 200), f32 delaySeconds (v ≥ 200)
  flag slowMotion (v ≥ 310), i32 slowMotionTweenFrames (v ≥ 310)
  flag repeating (v ≥ 330), i32 repeatBackFrames (v ≥ 330),
  i32 repeatLoops (v ≥ 330), flag tweenWhenRepeating (v ≥ 330)
  flag cameraDuringDelay (v ≥ 310)
  flag easeIn (v ≥ 310), flag easeOut (v ≥ 310), i16 interpolation (v ≥ 310)
  color backgroundColor
  color backgroundGradientColor       (v ≥ 200)
  i32 soundIndex (v ≥ 160), f32 soundVolume (v ≥ 160)
  f32 soundPan (v ≥ 310), f32 soundPitch (v ≥ 310)
count instanceCount; instances (§11.1, main context)
joins (§11.2; present only when v ≥ 174)
— when v ≥ 150: count textCount; per text (§12.1)
camera (§12.2)
```

#### 12.1 Text (`readProjectText`)

```
i32 id (v ≥ 176), i32 fontIndex (v ≥ 244)
f32 x, f32 y, f32 scale
string text
color color
i32 align
f32 rotation (v ≥ 237), f32 opacity (v ≥ 237)
flag aboveBars (v ≥ 310), flag isLocked (v ≥ 310)
flag hasShadow (v ≥ 242), color shadowColor (v ≥ 242)
```

#### 12.2 Camera (`readCamera`)

```
f32 zoom (v ≥ 170, fallback 1), f32 x (v ≥ 170), f32 y (v ≥ 170)
f32 rotation                          (v ≥ 230)
— when v ≥ 310: flag wobbleXY, flag wobbleRotation, flag wobbleScale,
  i32 wobbleXYIntensity, f32 wobbleRotationIntensity, f32 wobbleSpeed
— else when v ≥ 230: flag legacyWobble, f32 legacyWobbleIntensity, f32 wobbleSpeed
flag widescreen (v ≥ 220), flag legacyFlag (220 ≤ v < 244)
— when v ≥ 237: count bindings; per binding:
  i32 instanceId, i32 nodeId, i32 kind (v ≥ 300), 9 × f32 parameters, flag enabled
— when v ≥ 310: count secondaryBindings; per binding:
  i32 instanceId, 8 × f32 parameters
```

The original decoder itself notes that camera-binding parameter
*meanings* are not established ("typed and preserved, but their
behavioral meanings are not established"); §16 reflects that.

### 13. Trailer (`readProjectTrailer`)

```
— when v ≥ 237: count tweenLinks; per link:
  i32 startFrame, i32 endFrame, flag easeIn, flag easeOut,
  i16 interpolation (v ≥ 256)
— when v ≥ 251: viewport:
  i32 selectionMode, 5 × f32 parameters, i32 selectedAsset
  flags i (v ≥ 300), j (v ≥ 300), k, l, m (v ≥ 300), n (v ≥ 310),
        o (v ≥ 310), p, r/s/t/u (b ≥ 84)
  i8 mode (b ≥ 79)
  7 × color palette (b ≥ 84)
— when v ≥ 319: count editorPairs; per pair: i32, i32
EOF (exact)
```

## 14. Pose record layout (`compilePoseLayout(v, b, connector)`)

Fields in order; type widths: flag 1, i8 1, i16 2, i32 4, f32 4,
color 4. `local_angle` is the field the importer consumes (plus
`length`, `scale`, `use_segment_scale` for the length-flip rule, §15).

```
flag is_stretchy
flag is_floaty                        (b ≥ 48)
flag is_smart_stretch                 (v ≥ 248 and b < 22)
flag use_segment_color
flag use_circle_outline               (v ≥ 256)
flag circle_is_hollow                 (b ≥ 21)
flag use_gradient                     (v ≥ 176)
flag reverse_gradient                 (v ≥ 176)
i16  gradient_mode                    (b ≥ 20)
flag use_segment_scale
f32  scale
f32  length
i32  thickness
f32  local_angle
f32  default_local_angle              (v ≥ 259)
f32  default_angle                    (v ≥ 259)
color color
color gradient_color                  (v ≥ 176)
color circle_outline_color            (v ≥ 256)
flag use_polyfill_color               (v ≥ 230)
color polyfill_color                  (v ≥ 230)
i16  right_triangle_direction         (v ≥ 259)
i32  segment_curve_radius_and_default_curve_radius   (v ≥ 320)
flag curve_circulization              (b ≥ 20)
i16  segment_curve_polyfill_precision (b ≥ 21)
— when v ≥ 320:
  — when b < 36: f32 trapezoid_top_thickness_ratio
  — else:
    trapezoid_thickness_start         (i32 when b < 64, else f32)
    trapezoid_thickness_end           (i32 when b < 64, else f32)
    — when b = 36: flag use_trapezoid_thickness_start, flag use_trapezoid_thickness_end
    — else: flag trapezoid_is_rounded_start, flag trapezoid_is_rounded_end,
            f32 trapezoid_top_thickness_ratio
— when b ≥ 41:
  — when b ≤ 50: flag is_angle_locked, f32 legacy_angle_lock_offset
  — when 51 ≤ b ≤ 56: flag is_angle_locked, flag angle_lock_is_main_node,
                      f32 angle_lock_offset_minuend, f32 angle_lock_offset_subtrahend
  — when 57 ≤ b ≤ 66: flag legacy_angle_locked, flag legacy_angle_main,
                      f32 angle_lock_offset,
                      f32 legacy_angle_relative_start (b ≥ 63),
                      i8 legacy_angle_relative_multiplier (b ≥ 63)
  — when b ≥ 67: f32 angle_lock_offset
  flag is_drag_locked
  drag lock: i16 legacy_drag_lock_mode (b ≤ 45) else f32 drag_lock_angle
— connector records only:
  f32 connector_local_x, f32 connector_local_y,
  f32 connector_value, f32 connector_percent     (b ≥ 43)
  f32 connector_smart_stretch_ancestral_value    (b ≥ 65)
```

Reference sizes from the original: profile 417/35 normal record = 63
bytes (`PROFILE.nodePose`); 431/117 normal = 83 bytes, connector = 103
bytes (both confirmed by running `compilePoseLayout`).

## 15. Geometry / pose semantics (`layoutFigure`, `posedFigure`)

* A posed figure is the definition with each node's fields overridden
  by its pose record (`posedFigure`). Exceptions: **static** nodes keep
  their definition `length`, `thickness`, `scale`, `use_segment_scale`,
  colours and curve fields (a static limb is authored, not a world
  pin); a static node under a smart-stretch ancestor takes the pose
  `length`. The posed figure's `scale` and `color` are the
  **instance's** scale and color, replacing the definition's.
* Absolute angle Θ accumulates parent-first:
  `Θ(node) = Θ(parent) + local_angle(node)`, and the drawn segment
  vector is `(cos Θ, −sin Θ) · L` in y-down screen space, where
  `L = length · (use_segment_scale ? scale : 1) · figureScale` is
  **signed**: when `L < 0` (and the node is not a connector) the
  endpoint still lands on the signed vector, and the stored angle
  gains +180° before descendants accumulate from it.
* Special case (container build ≤ 53, from `posedFigure`): the pose
  `local_angle` of every direct child of the root is stored absolute;
  the root's pose angle is subtracted before accumulation.
* The root's local angle is *not* zeroed for posed figures (the
  build < 36 zeroing in `layoutFigure` applies only to unposed
  definitions).
* Connector nodes are solved toward their connection target by the
  native solver; the stored pose is preserved without recreating the
  dynamics (the original adapter takes the same stance). Angle-lock
  and smart-stretch solvers are likewise read, not simulated.
* Instance placement: the figure root sits at the instance's `(x, y)`
  in stage coordinates (y-down, no axis flip — the `1080 − y` flip in
  the adapter is a quirk of that app's Pivot-canvas document, not of
  this format).
* Frame timing (adapter `durations`): each stored frame lasts
  `hold + tween` ticks, where `hold = isDelayed ? max(1,
  ceil(delaySeconds · fps)) : 1` and, for every frame except the last,
  `tween = autoTween && isTweened ? (slowMotion ?
  slowMotionTweenFrames : tweenCount) : 0`. Clip frames have no
  per-frame settings: hold is 1 and the clip's own
  fps/tweenCount/autoTween apply.

## 16. `.nodemc` payload (`parseMovieClipPayload`)

```
i32  version
i32  build                            (only when version ≥ 402, else build = 1)
string name
count  frameCount
count  figureCount
count  spriteCount                    (only when v ≥ 400, else 0)
count  soundCount                     (always read — unlike .stknds)
count  thumbnailCount, count thumbnailWidth, count thumbnailHeight
thumbnailCount*thumbnailWidth*thumbnailHeight*4 bytes of RGBA thumbnails
settings: i32 fps, i32 tweenCount, flag autoTween, flag clipFlag,
          i32 nextInstanceId
figure library: per figure: string name + embedded figure (§10)
sprite library: per sprite:
  u8 type, string name, i32 sourceLibraryId,
  then the §8 group/image bodies (group members are *source* library
  ids, resolved against sourceLibraryId, not local indices)
sound library: per sound: string name
frameCount × clip frame (§11)
EOF (exact)
```

Note the two differences from the `.stknds` sprite library: entries
carry a `sourceLibraryId`, and group membership is expressed in those
source ids.

## 17. What the Rust importer models — and what it does not

`import_nodes_animation` in `src/nodes_anim.rs` parses both containers
byte-exactly (same gates, same validation strictness, exact EOF) and
maps the result onto `crate::model::Project`:

**Modelled**
* Project name, FPS (header / clip settings), `tween_playback` from
  the autoTween flag, frame durations from the §15 tick rule.
* Figure library → `FigureDef`s via `crate::nodes::import_nodes_figure`
  (identical conversion to the `.nodes` path), with a local conversion
  fallback for embedded figures older than version 160, whose
  standalone unwrap path cannot be reused.
* Every placed **figure instance** becomes a project figure slot,
  keyed by (type, instance id) in first-appearance order; a frame
  without the instance holds its previous pose. Instance scale is
  baked into the slot's def lengths/thicknesses from its first
  appearance (the definition's own scale is divided back out, since a
  posed figure's scale *is* the instance scale, §15); the instance
  color becomes the slot color.
* Pose records → model poses using §15: parent-first Θ accumulation,
  the signed-length +180° flip, the build ≤ 53 root-child adjustment,
  and the model convention `modelAngle = Θ + 90` already established
  by the `.nodes` importer. Per-frame length/thickness changes are
  *not* representable in `model::Pose` and are dropped (the flip sign
  still uses the pose's length/scale).
* Movieclips whose every frame holds exactly one figure instance of
  the same figure (and no joins) become `model::ClipDef`s; main
  timeline instances referencing them become clip instances, with the
  clip's pose roots offset so the clip's first-frame root lands on the
  placement point (first placement wins).
* A `.nodemc` becomes a timeline project of its own frames (not a
  clip), so it stays editable.
* `.stknds` background: the first main frame's `backgroundColor`
  becomes the project background.

**Parsed byte-exactly but not modelled** (mirroring the original
adapter's own "stored poses preserved, dynamics not recreated" stance)
* Sprite instances and sprite pixel data, main-frame texts, sounds
  (external references only — there are no bytes to import), joins,
  camera records and bindings (semantics unestablished upstream,
  §12.2), style/filter blocks, thumbnails, trailer editor state,
  movieclip instance playback cursors / play modes / rotation / flips
  (the model's clip playback loops from the clip start), and instance
  scale changes after an instance's first frame.
* Multi-figure or joined movieclips are not converted to `ClipDef`s;
  main-timeline instances referencing them are skipped.

**Known limits**
* Container dispatch is by parse attempt (`.stknds`, then `.nodemc`),
  not by filename.
* ZIP archives of Stick Nodes files (handled by the adapter via
  `archive.mjs`) are out of scope.
