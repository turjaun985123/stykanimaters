# Squad 15 Audit — 3D & Camera

Phase 1 audit for the Ultimate UI/UX Mission. Scope:
`three_d_panel.rs` (1,747 lines), `fig3_preview.rs` (696),
`stage_fig3.rs` (347), `camera_panel.rs` (1,658),
`stage_camera.rs` (738) — all read in full at HEAD
(Phase N close + Phase O charter commit). Read-only
audit; no code changed.

Evidence base: source reads, plus the *measured* tree
dumps in `docs/phase_o_plans/p05-overflow-widths.md`
(O-7, O-9 cover two rows in these files — cited, not
re-planned), the Phase N headless snaps (`three-d`,
`three-d-stage` scenes exist and are pixel-proven), and
the kit in `design.rs` (`empty_state`, `list_row`,
`stepper_value`, `section_label` are the reference
forms). Mounting facts: both panels live in the RIGHT
column (`main.rs:629/638`, content budget 316 pt);
Build3D sections are [ThreeD, Figures, Camera]
(`workspaces.rs` BUILD3D_SECTIONS); the camera overlay
floats over the stage (`main.rs:745`); the on-stage 3D
figure is gated to the Build3D workspace
(`main.rs:722`).

## What already works — do not rebuild

- Every control in both panels calls a real engine op;
  nothing is faked. The 3D panel's readouts (joint
  position, camera orbit) recompute from the live
  session every build.
- `stage_camera.rs` IS wired (rotation in drawing and
  dragging, guides/frustum/safe-frames drawn in
  `stage_view.rs:535-565`) and its rotation parity with
  the export rasterizer is test-pinned.
- `stage_fig3.rs` clips every segment to the stage rect
  and is test-proven; preview and stage share colours,
  widths and the ground rule through `fig3_preview`.
- Camera key list rows already ride kit `list_row`
  shells with the current key marked once; zoom/angle
  readouts already use kit `stepper_value` mono.
- The dial's undo contract (one snapshot per gesture,
  Esc cancels) is correct and documented.

## Defects (severity order; ★ = top 3)

### ★ D1 — Controls run off the card and off the window (measured)

From p05's tree-dump measurements at 1440×900: the 3D
panel's posing row overflows the right-column content
edge by **+49 pt** (`orbit-3d`; the Phase N palette
snap already showed "Orbi…" cut), its camera row by
**+37.5 pt** (`cam-frame-3d`), and the camera panel's
key-action row is worse: **`cam-delete-key` ends at
x 1441.4 — past the window edge entirely** (+29.4 vs
content). In the 3D workspace, buttons the user needs
are clipped or unreachable. Fixes are already planned
as p05 items **O-7** (flex_wrap both 3D rows) and
**O-9** (wrap the camera key row) — Squad 15 endorses
both and adds no duplicate item.

### ★ D2 — Three "remove a camera key" paths, three behaviours, near-identical labels

`camera_panel.rs` offers three ways to remove a key:

| Control | Key | Acts on | Undoable | Side effect |
|---|---|---|---|---|
| "Clear key" (`cam-clear-key`) | `camera_clear_key` | current frame | **No** | — |
| Row "Clear" (`cam-key-clear-{f}`) | `clear_key_at` | that row's frame | **No** | **moves the playhead to that frame** |
| "Delete key" (`cam-delete-key`, trash-can) | `delete_camera_key_at` | current frame | Yes | — |

Nothing in the labels tells the user that two of the
three cannot be undone, or that the row button will
jump the playhead. This is exactly the
destructive-action clarity the mission brief (§8)
requires fixing. The undoable path is strictly better
(it also never moves the playhead), so the fix is
behavioural unification, not just copy. → Item I-6.

### ★ D3 — Bare empty lines where the app has a kit empty state

- `camera_panel.rs` `key_list`: `text("No camera keys
  yet.")` — a bare LABEL-font line, no glyph, no next
  action, while the Set key button that creates a key
  sits two rows above unexplained.
- `three_d_panel.rs`: three more bare lines —
  "Enter 3D to list the figure's joints.",
  "No saved 3D figures.", "No saved 3D poses."

The kit form exists (`design::empty_state(icon, msg)`)
and is already used by the audio, clips, figures and
graph panels; the glyphs needed (`key`, `cube`) are
both in the curated icon table. These four lines are
the mission brief's "never leave empty panels looking
unfinished" (§5G) in miniature. → Items I-1, I-2.

### D4 — 3D drawing colours bypass the theme entirely

`fig3_preview.rs` hard-codes packed literals:
`GRID_COLOR 0x3A4356`, `SHADOW_COLOR 0x000000` (pure
black), `SELECTED_COLOR 0x6D8CFF`. The preview canvas
*fill* follows the theme (`t.control_off` in
`preview_el`), but everything drawn on it — and, via
the shared constants, the on-stage 3D figure in
`stage_fig3.rs` — is dark-theme artwork in both
themes. The light theme gets a dark blue-grey grid and
a pure-black shadow ring; the selection colour
duplicates the accent as a literal instead of reading
`tok::accent`. Brief §7: no hard-coded dark-theme
colours inside panels; light theme gets equal
attention. → Item I-7.

### D5 — One-way rotation: posing can only turn +15°

The posing row offers RotX/RotY/RotZ **+15** and Orbit
**+15** only. Reaching −15° costs 23 presses. There is
also no readout of the selected joint's current
rotation (only its world position), so precise posing
is blind. Camera rotation in the same workspace has
±15° steppers and a dial; joint rotation has neither.
→ Items I-3, I-4.

### D6 — Two buttons named "Frame" with different semantics, six rows apart

Preview row: "Frame" (`d3-frame`) — frames the figure
in the *preview*, preserving yaw/pitch/FOV. Camera
row: "Frame camera" (`cam-frame-3d`) — replaces the
*whole* camera, framed for the *stage*. A user cannot
predict which framing a press will produce. → Item
I-5 (explicit copy change listed there).

### D7 — Inconsistent disabled semantics inside one panel

Without a session, the 3D panel's text controls are
**dimmed but live** (press fires; the op answers
"Enter 3D mode first.") while its icon controls are
**fully inert** (no handler at all). Two disabled
dialects, one panel — brief §5D asks for one button
language with appropriate states. (Each choice is
documented and deliberate; the inconsistency is the
defect.) → Item I-10 (kit-level, Squad 02/17 own the
builder; this panel is the first adopter).

### D8 — The stage's 3D grid ignores the panel's Grid toggle

`fig3_preview::preview_prims` honours
`grid_visible()`; `stage_fig3::stage_fig3_prims` calls
`push_grid` unconditionally — there is no flag
parameter. Toggling Grid off in the panel turns it off
in the preview and leaves it on on the main stage,
where the same figure is drawn larger. → Item I-8.

### D9 — Camera panel's first groups have no section labels

The panel opens with view (Fit/Centre/1:1), zoom,
rotation and key-action rows directly under the
divider, unlabelled, while "Keys" and "Follow" below
get kit `section_label`s. Grouping reads as one long
button stack — brief §5B (organised, labelled
sections). Relatedly, neither panel uses
`panel_title`'s `meta` slot, so neither header carries
its natural count (camera keys / joints). → Item I-9.

### D10 — The rotation dial sits on the panel's own colour

`angle_dial` fills its canvas with `t.card` — the same
fill as the `panel_col` card it sits on — so the dial
reads as a floating track, not a recessed control.
The 3D preview solved the same problem with
`t.control_off`. The dial also has no hairline border.
→ Item I-11.

### D11 — The stage camera overlay ignores the ambient theme

`camera_panel::overlay` takes only `&EditorCore` and
builds its own `tok::theme(true)` — the floating
Fit/Centre/zoom pill over the stage is always dark,
whatever theme the rest of the shell is in. →
Item I-12 (needs the `main.rs:745` call site —
integrator-owned file, authorisation required).

### D12 — Guide families are indistinguishable on stage

`stage_view.rs:539-565` draws guides, camera frustum
and safe frames all in one ink (`Color::WHITE` @ 0.22,
1.0 width) — "the same ink as the thirds grid" was a
Phase F choice. With all three toggles on, the user
sees three nested line systems and cannot tell what
the camera frames from what is title-safe. The
geometry functions are already separate
(`stage_camera.rs`), so only the draw pass needs
distinct inks. → Item I-13 (Squad 10's file; geometry
side needs no change).

### D13 — `stage_camera.rs` module docs describe a file that no longer exists

The header still says "nothing here is mounted yet"
and gives a six-step integrator wiring recipe —
all six steps were done phases ago (verified:
`main.rs:223/279`, `stage_view.rs:535+`). A coder
trusting the docs would re-wire a wired system.
Source outranks docs, but the doc is *in* the source.
→ Item I-14.

### D14 — Smaller organisation and polish defects

- **Joint list rows** are full-width text buttons with
  index-prefixed labels ("0 · root"); no depth indent,
  and selection shows only as the button's active
  fill — the camera key list in the same workspace
  shows the better pattern (`list_row` shell carries
  the mark). 15 rows also make the app's tallest
  panel taller. → Item I-15.
- **Readouts off the numeric discipline:** Position
  and Orbit lines in the 3D panel are LABEL-font text,
  not the kit `stepper_value` mono the camera panel
  uses for the same kind of numbers. → Item I-5.
- **Pose-save name collisions:** poses save under the
  fixed name `"<figure> pose"`; saving twice silently
  reuses one name, so the library can never hold two
  poses of one figure from this panel. → Item I-16.
- **Preview canvas styling:** radius 6.0 (kit radii
  are 12 panel / 8 row) and no hairline border; the
  fixed `PREVIEW_W` 308 leaves only 8 pt of slack in
  the 316 budget — correct today, silently breakable
  by any padding change. → Item I-17.
- **Dead code smell (3D panel):** every text button is
  written twice behind `if dim { btn_muted_live } else
  { btn }` — ~40 duplicated call sites in
  `three_d_panel`, the same pattern in miniature in
  the camera rows of other panels. One kit helper
  (`btn_dim`) retires the whole class. → Item I-10.

## Work items (each sized for ONE coder)

**I-1 — Camera key-list empty state.**
`camera_panel.rs`, `key_list` only. Replace the bare
"No camera keys yet." text with
`design::empty_state("key", "No camera keys yet — Set
key pins the current view.", t)`. No key/handler
change (the line is unkeyed). Proof: default +
three-d tree dumps show the line's element replaced;
snap read.

**I-2 — 3D panel empty states.**
`three_d_panel.rs` only. The three bare lines become
`design::empty_state`: joints line with `cube`,
"No saved 3D figures." and "No saved 3D poses." with
`cube` (both glyphs verified in `icon_data.rs`).
Copy otherwise verbatim. Proof: three-d scene tree +
snap (no-session state is the default build).

**I-3 — Negative rotation buttons.**
`three_d_panel.rs` posing row. Add RotX/RotY/RotZ
−15 companions (new keys `rotx-neg-3d` etc. —
additions only, census-safe) calling
`fig3_rotate(axis, -15.0)`, and Orbit −15
(`orbit-neg-3d`). Land together with p05 O-7's
flex_wrap of this row so the wider row wraps instead
of overflowing. Proof: three-d tree scan within
budget; unit test on handler wiring follows the
panel's existing test patterns.

**I-4 — Selected-joint rotation readout.**
`three_d_panel.rs`. Add a "Rotation" line beside
Position reading the selected joint's stored Euler
angles from `st.pose` (verify the `Pose3` field in
`fig3d.rs` first), formatted through the existing
`fmt_fixed`. Pure helper + unit tests in the file's
existing style. No keys.

**I-5 — Frame disambiguation + mono readouts.**
`three_d_panel.rs`. Explicit copy change (charter
rule: listed here): `cam-frame-3d`'s label
"Frame camera" → "Frame on stage"; the preview's
"Frame" stays. Position and Orbit readout values move
into `design::stepper_value` (they are unkeyed text;
no census impact). Proof: three-d snap read shows
both labels and mono readouts.

**I-6 — Unify camera-key removal on the undoable path.**
`camera_panel.rs`. Change `cam-key-clear-{f}`'s
handler from `clear_key_at` (goto + non-undoable
clear) to `delete_camera_key_at(core, frame)` —
undoable, playhead stays. Label "Clear" → "Delete"
(copy change listed here) so the row matches what it
now does; `clear_key_at` and its tests are retired or
kept only if another caller exists (grep first).
`cam-clear-key` (the engine op, current frame) is
untouched. Proof: unit tests — delete at a non-current
frame leaves `core.current` unchanged and
`undo_depth` +1; CI green.

**I-7 — Theme the 3D drawing colours.**
`fig3_preview.rs`, `stage_fig3.rs`,
`three_d_panel.rs` (`preview_shape`), plus the
`stage_view.rs` fig3 pass (Squad 10 file — coordinate,
do not edit unilaterally). Introduce a small palette
built from `&Theme` at the two call sites (grid =
theme border-toned colour, shadow = theme text at low
alpha or a theme recess colour, selected =
`tok::accent`) and thread it into `preview_prims` /
`stage_fig3_prims` as a parameter, replacing the
packed literals. Both themes must be snapped.
Proof: three-d + three-d-stage snaps in dark AND
light; existing geometry tests unchanged (colours
only).

**I-8 — Stage 3D grid honours the Grid toggle.**
`stage_fig3.rs` + its `stage_view.rs` call site
(coordinate with Squad 10). Give
`stage_fig3_prims` a `show_grid: bool` parameter, the
exact shape `preview_prims` already has, and pass
`fig3_preview::grid_visible()` at the call site.
Proof: unit test (grid prims absent when false) +
three-d-stage snap with the toggle off.

**I-9 — Camera panel group labels + header meta.**
`camera_panel.rs`, `three_d_panel.rs`. Add
`section_label("View")` above the Fit row and
`section_label("Key")` above the Set key row in the
camera panel; pass `meta` to both panels'
`panel_title` (camera: `"{n} keys"` from `key_rows`
len; 3D: `"{n} joints"` in session, `None` out).
Proof: default/three-d snaps; no key changes (labels
and meta are unkeyed).

**I-10 — One dimmed-button helper; one disabled dialect.**
Kit item — Squad 02 owns `chrome_view.rs` /
`icon_btn.rs`; Squad 15 supplies the census. Add
`chrome_view::btn_dim(key, label, active, dim, t,
handler)` collapsing the if/else pairs, and a
muted-live icon variant so the 3D panel's icon
controls adopt the same dimmed-but-live dialect as
its text controls (the refusal message is the more
useful behaviour). First adopter: `three_d_panel.rs`
(~40 sites collapse). Keys/labels/handlers verbatim
at every site. Proof: gate census 804 intact; three-d
tree identical key set.

**I-11 — Dial recess.**
`camera_panel.rs`, `angle_dial` only: fill
`t.control_off` + `border(1.0, t.card_border)`, radius
stays 6. Proof: default snap read — dial reads as a
recessed control like the 3D preview.

**I-12 — Overlay follows the ambient theme.**
`camera_panel.rs` (`overlay` gains a `&Theme`
parameter) + `main.rs:745` call site — **integrator
authorisation required** (main.rs). Proof: light-theme
snap of the stage shows a light pill; dark unchanged.

**I-13 — Distinguish the guide families.**
`stage_view.rs` draw pass only (Squad 10 owns the
file; `stage_camera.rs` geometry unchanged): frustum
in the accent at higher alpha, safe frames in the
existing white at lower alpha, thirds unchanged.
Proof: a rotated-camera scene snap with all three
toggles on, pixel-read.

**I-14 — Refresh `stage_camera.rs` module docs.**
Docs-only, that file: replace the stale "nothing is
mounted yet" header and wiring recipe with the wired
reality (call sites in `main.rs` / `stage_view.rs`),
keeping the sign-convention proof section verbatim —
it is the load-bearing part. Proof: grep — no
remaining "not mounted" claims; CI (doc comments only,
but the gate still runs).

**I-15 — Joint rows on kit list-row shells.**
`three_d_panel.rs` joint list: each row rides
`design::list_row("joint-row-3d-{i}", selected, t)`
(the camera key list's pattern — the shell carries
the selection mark), the button inside drops its own
active fill, and the label gains depth indentation
from the node's parent chain. Keys
`joint-sel-3d-{i}` and the `select_joint` handler
verbatim. Proof: three-d tree (shell keys added,
button keys intact) + snap read.

**I-16 — Pose-save names stop colliding.**
`three_d_panel.rs`: `pose_save_name` gains a
uniqueness step over `fig3d::scan_pose_library()` —
`"<figure> pose"`, then `"<figure> pose 2"`, … Pure
helper, unit-tested in the file's style. The status
line already reports the name saved under, so the
user sees the distinct name with no further UI.
Proof: unit tests (empty library, one collision, two
collisions).

**I-17 — Preview canvas kit styling.**
`three_d_panel.rs`, `preview_el` only: radius
6.0 → 8.0 (kit ROW_RADIUS), add
`border(1.0, t.card_border)`. Also add a
`debug_assert`/test pinning `PREVIEW_W ≤ 316 -
2*PANEL_PAD` budget arithmetic so a future padding
change fails loudly instead of clipping silently.
Proof: three-d snap + the new test in CI.

## Cross-references

- p05 **O-7 / O-9** own the overflow repairs in these
  files (D1) — do not duplicate; I-3 must land with
  or after O-7.
- Squad 03 / integrator: in Build3D the right column
  stacks the app's tallest panel (3D: preview + 15
  joint rows + five control groups + two libraries)
  directly above the full camera panel, so the camera
  controls start below the fold at 900 pt. Panel-side
  relief is I-15; column-order relief is the
  integrator's call, not a panel coder's.
- Squad 19: light-theme snaps of `three-d` /
  `three-d-stage` do not exist yet — I-7's proof
  needs them added to the headless set.
- Observation, not a defect: key rows show a key's
  *stored* angle, and a key created by zooming
  honestly stores 0° (engine behaviour, documented in
  the file). The row is truthful; changing it is an
  engine decision, out of this squad's scope.
