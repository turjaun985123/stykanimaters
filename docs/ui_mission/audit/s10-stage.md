# Squad 10 Audit — Animation Stage & Viewport

Scope: `src/stage_view.rs`, `src/rune_sec/stage_camera.rs`,
`src/rune_sec/stage_fx.rs`, `src/rune_sec/stage_fig3.rs`, plus the
stage's mounting in `src/main.rs` and its one control surface,
`camera_panel::overlay`. Presentation and controls only — the
coordinate design (`ViewTransform` / `RotTransform`, proven
export-parity) is correct and must not be touched.

Measured baseline: at 1440×900 the stage element is **744×485**
(Phase N headless tree, `stage` key). Everything below is read
from current source, not the phase docs.

## What is already right (do not redo)

- One transform for draw and drag (`RotTransform::for_display` /
  `for_drag`), rotation parity with the rasterizer pinned by tests.
- Draw order mirrors export: guides → onion → stroke highlight →
  strokes → figures → fig3 → FX → text → weather → gizmos last.
- Selection markers (stroke highlight, particle/FX gizmos) share
  one accent (`SELECTION_ACCENT` = `GIZMO_COLOR` = `0x6D8CFF`).
- fig3 prims are clipped to the stage rect (`stage_fig3.rs`),
  because the Rune canvas does not clip its own drawing.

## Defects

### D1 — No stage boundary by default; letterbox is invisible (P0 presentation)
`main.rs:566` fills the *whole canvas element* with
`project.background`. `stage_shape` draws no stage rectangle of
its own — the only stage outline is the guides border, and guides
default **off** (`StageOverlays::default`, shell `State.guides`).
Result: stage area and letterbox are one flat colour; the user
cannot see what an export will contain, and off-stage content
looks identical to on-stage content. A professional viewport
always shows its frame.
**Item S10-1 (one coder, `stage_view.rs` + `main.rs`):** draw a
permanent stage frame in `stage_shape` — the stage rect outline
in a theme-derived ink at low alpha, plus a dimmed/hatched-free
letterbox treatment achieved by filling the element with a
chrome colour and the stage rect with `project.background`
inside the shape (element fill becomes the surroundings colour).
Acceptance: default snap (guides off) shows a readable frame;
guides-off stage snap pixel-diff confined to the frame/surround;
existing stage tests unchanged; keys unchanged (`stage`).

### D2 — 2D passes are not clipped to the stage rect; fig3 is (P1)
`stage_fig3` clips every segment to `[0, stage_w] × [0, stage_h]`
precisely because unclipped lines paint over the shell. The 2D
figure, stroke, FX and weather passes in `stage_shape` have no
equivalent clip: a figure dragged off-stage draws across the
full element, over the letterbox. Inconsistent with D1's frame
and with fig3's own rule.
**Item S10-2 (one coder, `stage_view.rs`):** clip figure/stroke/
FX/weather screen prims to the stage rect's screen footprint
(the rotated rect under `RotTransform`), reusing the
`clip_segment_to_viewport` approach; discs/glyph boxes clip or
cull by centre+radius. Acceptance: off-stage figure snap shows
nothing outside the frame; on-stage snaps byte-identical;
roundtrip/drag code untouched.

### D3 — All guides share one ink; frustum ≡ border at rest (P1)
Thirds, centre cross, frustum and both safe frames all draw in
`Color::WHITE.alpha(0.22)`, 1 pt (`stage_shape` guides pass).
At the default camera the frustum *is* the stage border, so two
systems overdraw invisibly; when zoomed, frustum vs thirds vs
action-safe vs title-safe are indistinguishable, and none is
labelled. White ink also vanishes on light project backgrounds.
**Item S10-3 (one coder, `stage_view.rs` + `stage_camera.rs`):**
give each guide family its own treatment from central tokens —
frustum = accent at higher alpha, safe frames = warning-tinted
dashed-equivalent (segmented lines; Rune has no dash, so emit
gapped segments in the geometry fns), thirds/centre stay faint —
and pick ink polarity from `project.background` luminance.
Acceptance: `guides_geometry`/`frustum_geometry`/
`safe_frame_geometry` tests updated for the segmented safe
frames; a guides-on snap at zoom 2 shows three visually distinct
families; no new colour literals outside the token source.

### D4 — Viewport zoom is the authored camera; no view-only navigation (P0 UX, decision needed)
`stage_wheel` (`main.rs:288`) and every zoom control call
`core.camera_zoom_by`, which upserts a **camera key** — scrolling
the wheel rewrites the animation's camera track, and there is no
way to inspect a detail without editing the film. Likewise
"Fit" (`view-fit`) actually runs `frame_all_figures()` — it
frames the *figures*, not the stage, so its label misleads.
This squad may not change engine behaviour unilaterally; the
item is scoped as a shell-side view transform multiplier.
**Item S10-4 (one coder, `main.rs` + `stage_view.rs`, integrator
approval required):** add a shell-only `view_zoom`/`view_pan`
multiplier composed into `view_transform` at the stage call
site (draw + drag + wheel all read it; engine camera untouched),
wheel/buttons drive the view layer, and a "Reset view" appears
when the view differs from the camera. Relabel or re-behaviour
`view-fit` to fit the *stage* (key unchanged). Acceptance:
wheel-zoom then save → project file has no new camera key
(test); drag still lands on the grabbed joint at view_zoom 2
(Runtime test); camera keys still drive the stage when the view
is reset. If the integrator rejects a view layer, the fallback
item is copy-only: rename the strip's group to make "these
controls edit the camera" explicit.

### D5 — Camera controls sit below the stage in a dead row (P1)
The Fit/Centre/1:1/zoom pill (`camera-overlay`) is not overlaid
on the stage at all: `main.rs:740` puts it in a full-width
`stage-overlay-row` under the canvas, right-aligned over an
empty grow spacer. It costs stage height (part of why the stage
is 485 and not ~510 tall), reads as a stray floating pill, and
is visually disconnected from the viewport it controls.
**Item S10-5 (one coder, `main.rs` + `camera_panel.rs`):** move
the pill into the stage's bottom-right as a true overlay if
Rune stacking at this rev supports it (verify in rune-src
first); otherwise dock it into the transport row's right end.
Keys verbatim (`view-fit`, `view-centre`, `view-actual`,
`cam-zoom-out`, `cam-zoom-in`, `camera-overlay`). Acceptance:
stage element height grows by the reclaimed row; tree dump
shows the pill inside/right of the stage region; census holds.

### D6 — Joint handles use off-system colours and scale with zoom (P1)
Handles are hard-coded GPUI-era hex — red `0xFF5D5D`, selected
amber `0xFFC65D` (`stage_shape`) — not the 2.0 tokens and not
the shared `SELECTION_ACCENT` the other three markers use. Red
handles disappear against red figures/backgrounds. Radius is
`(5 | 8) * s.max(0.6)`: handles grow/shrink with zoom instead of
holding a constant screen size like every professional editor.
**Item S10-6 (one coder, `stage_view.rs`):** handles take the
selection language — unselected = figure-agnostic neutral with
a dark outline ring, selected = `SELECTION_ACCENT`; radius
constant in element points (divide by `s`, clamped), outline
drawn as a slightly larger dark ngon under each handle.
Acceptance: snap at zoom 0.5 and zoom 2 shows same-size handles;
colour literals gone from the handle block; hit-test radii in
the core untouched (visual only).

### D7 — Weather/FX draw over the joint handles (P2)
Handles are emitted inside the figure pass; fig3, FX, text and
weather then draw over them. Only the two gizmos were given the
"always findable" last slot. In rain/storm scenes the selected
figure's handles — the thing being dragged — can be buried.
**Item S10-7 (one coder, `stage_view.rs`):** collect handle
prims during the figure pass and emit them in the final overlay
slot with the gizmos (same pass order, handles before gizmos).
Acceptance: storm-scene snap shows handles over weather; draw
order for all content passes unchanged (existing order tests /
snap diffs confined to handle pixels).

### D8 — Stage text renders as solid glyph boxes (P1, honesty)
`stage_text::TextPrim::GlyphBox` is drawn as a filled polygon:
on-stage text is unreadable blocks, while exports render real
text. The stage therefore misrepresents any scene with labels.
**Item S10-8 (one coder, `stage_text.rs` + `stage_view.rs`,
feasibility-gated):** if Rune canvas text is available at this
rev, draw the string at the box's origin/size; if not, replace
the solid fill with an outline + first-line bar treatment and
record the limitation in `docs/QA_RUNE.md` — do not leave solid
blocks implying rendered text. Acceptance: text-scene snap
shows legible or clearly-placeholder text; export path
untouched either way.

### D9 — No hover or drag feedback on the canvas (P2)
There is no hover highlight for the joint under the pointer, no
live coordinate/angle readout while dragging, and no visual
difference between "grabbed" and "hovering". Selection happens
only on press (`stage_drag`, `stage_secondary`).
**Item S10-9 (one coder, `main.rs` + `stage_view.rs`,
feasibility-gated on Rune pointer-move events at this rev):**
hover ring on the hit-tested joint (reuse `hit_test` through
`RotTransform::for_drag`), and while dragging, the status strip
shows the joint's stage coordinates (status segment already
exists — text only). If pointer-move is unavailable, ship the
drag readout alone and record the hover limit. Acceptance:
Runtime test for the drag readout string; no change to drag
maths or undo snapshots.

### D10 — Guides controls are buried; readout frame mismatch (P2)
The three guide toggles live only in the settings popover, far
from the stage they affect, and the zoom % readout
(`camera_panel::overlay`) reads the *current* frame's camera
while the stage draws `display_frame()` — during tween playback
the number and the picture disagree.
**Item S10-10 (one coder, `camera_panel.rs`):** add a guides
toggle chip to the camera pill (new key `view-guides`, flips the
existing `State.guides`; sub-toggles stay in settings), and
sample the readout from `camera_at(display_frame())`.
Acceptance: pill width stays within the centre column at 1280
(tree check); playback snap test pins readout = display zoom.

### D11 — Onion skins ignore segment thickness (P2, minor)
Skins draw every segment at a flat `5.0 * s` regardless of the
node's real thickness, so a thick-limbed figure's ghost is a
thin stick — the ghost misstates the pose's mass. Two alpha
steps only (0x4D / 0x26) regardless of depth setting.
**Item S10-11 (one coder, `stage_view.rs`):** carry each node's
thickness through `onion_skins` (its segment builder already
walks the def) and stroke skins at node thickness; keep the
colour/falloff contract and its tests' intent. Acceptance:
existing onion tests updated only where thickness is asserted;
ghost snap matches figure proportions.

## Ownership & sequencing

- `stage_view.rs`: S10-1, 2, 3, 6, 7, 11 — one owner, sequence
  1 → 2 → 7 → 6 → 3 → 11 to minimise snap churn.
- `main.rs`: S10-4, 5, 9 — integrator-owned file; these three
  items need integrator scheduling, not squad edits.
- `camera_panel.rs`: S10-5 (shared), S10-10.
- Dependencies: D1 before D2/D5 (frame defines the clip and
  the overlay anchor); Squad 02 tokens before S10-3/S10-6.
- Verification for every item: CI (GitHub Actions only),
  headless scene/snap pixel read, key census vs the 804
  baseline (only planned addition: `view-guides` in S10-10).
