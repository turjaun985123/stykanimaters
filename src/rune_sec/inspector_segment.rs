//! Segment inspector panel (Rune migration Phase D, section
//! 3/40). Declared from `main.rs` only (as
//! `mod rune_sec::inspector_segment` or the section aggregator's
//! re-export) — bin-crate code, not part of the engine library.
//!
//! Shown when a node is selected: real readouts from
//! `EditorCore` state (index, name, length, thickness, world
//! angle at the current frame, current-frame bend, shape,
//! colour) plus the editing controls, each wired to exactly
//! one real core op — the same ops the GPUI inspector's
//! buttons called, under the same element keys where one
//! existed:
//!
//! * Length −/+ → `adjust_length(∓4.0)` (editor.rs:466) —
//!   keys `insp-len-dn` / `insp-len-up`; the origin has no
//!   length (the op no-ops on it), so the row is omitted for
//!   the root, exactly as the GPUI inspector omitted it.
//! * Thickness −/+ → `adjust_thickness(∓1.0)` (editor.rs:475)
//!   — keys `insp-th-dn` / `insp-th-up`.
//! * Rotate −5°/−1°/+1°/+5° → `rotate_selected` (this
//!   module) — keys `seg-rot-m5` / `seg-rot-m1` /
//!   `seg-rot-p1` / `seg-rot-p5`. The core has no
//!   node-rotate op: a node's angle at a frame IS its pose
//!   angle (`frames[current].poses[fi].angles[ni]`, degrees
//!   — the value `key_graph_angle` keys and drag posing
//!   solves into), so the steppers write the current
//!   frame's pose directly, in the core ops' own pattern
//!   (validate → `snapshot()` → mutate → status, as
//!   `mirror_pose` does): one press = exactly one undo
//!   step. Pose semantics, not a graph key — an `Angle(ni)`
//!   graph channel still overrides the sampled pose at
//!   render (`positions_f`, editor.rs:1060). The stored
//!   angle is not normalised (the engine keeps unbounded
//!   degrees); the Angle readout normalises for display
//!   and shows one decimal via `format_deg` — the same
//!   formatter the Rotate row's value uses.
//! * Bend −5°/−1°/+1°/+5° → `bend_selected` (this
//!   module) — keys `seg-bend-m5` / `seg-bend-m1` /
//!   `seg-bend-p1` / `seg-bend-p5`, the fine-grained
//!   sibling of Cycle bend below. Bends live in
//!   `Frame.bends[fi][ni]` (model.rs:531), stored in
//!   **radians** in a SPARSE table parallel to `poses`:
//!   a missing figure row or a short node row reads as
//!   0.0 (straight), so the write grows the table first,
//!   exactly as `cycle_bend` grows it (editor.rs:1164).
//!   The model defines no hard clamp; the only range the
//!   engine itself ever writes is `cycle_bend`'s cycle,
//!   whose extremes are ±0.7 rad (≈40.1°), so the
//!   steppers clamp to ±`BEND_LIMIT_RAD` — an imported
//!   off-range bend is pulled to the limit on the first
//!   press, the same snap-toward-the-model spirit as the
//!   cycle snapping off-cycle values to a step. Bends
//!   are RENDER-ONLY: they never move a joint
//!   (`model::evaluate` and `positions_f` never see
//!   them); renderers fetch the interpolated
//!   `Project::bends_at` and arc the drawn segment
//!   through `model::bend_mid`. Since Wave Y (P-7) a
//!   graph channel CAN override a bend —
//!   `ChannelTarget::Bend(ni)` — exactly as an
//!   `Angle(ni)` channel overrides the pose angle the
//!   Rotate bullet describes: the steppers and this
//!   panel's readouts work on the STORED bend (the
//!   authoring value), while the rendered bend is the
//!   channelled evaluation (`apply_channels`' bend
//!   rows) whenever a Bend channel exists for the
//!   figure. (This bullet claimed the opposite until
//!   Wave Z's Z11 audit — the premise ended with P-7.)
//!   The origin has no parent
//!   segment and cannot bend (`cycle_bend` refuses it),
//!   so the row is omitted for the root, like the Length
//!   row. Steps are 1°/5° for the same reason the Rotate
//!   steps are: the panel speaks degrees for bends
//!   everywhere (the Bend readout, `cycle_bend`'s own
//!   status), while `bend_selected` takes the delta in
//!   the storage unit, radians — mirroring
//!   `rotate_selected`, whose degrees ARE the storage
//!   unit for angles.
//! * Bend readout + Straighten → `bend_degrees` /
//!   `straighten_selected` (this module) — the readout
//!   is one muted line under the header, beside the
//!   Index line (key `seg-bend-readout`): the selected
//!   segment's bend in whole degrees via
//!   `format_bend_readout` (negatives show their sign;
//!   a straight segment reads "Bend 0°").
//!   `bend_degrees` performs the same stored-value
//!   read as `segment_state`'s Bend readout — the
//!   current frame's `Frame.bends` entry,
//!   sparse-missing as 0 — NOT the renderers'
//!   interpolated `bends_at`, so the line can never
//!   disagree with the Bend readout row or the stepper
//!   row below it. Straighten (key `seg-bend-zero`,
//!   on the Bend stepper row's second line) calls
//!   `bend_selected`
//!   with the exact negation of the stored radians, so
//!   the sum lands on bit-exact 0.0 — no degrees round
//!   trip, which can be an ulp off. At bend 0 the
//!   press is a silent no-op rather than an undo step:
//!   `bend_selected` takes its snapshot unconditionally
//!   once its guards pass, so even a zero delta would
//!   snapshot — the guard lives in
//!   `straighten_selected`, not in the shared op.
//! * Joint limits (Wave X, P-4) → `adjust_joint_limit` /
//!   `clear_joint_limits` (this module) — keys
//!   `seg-lim-min-dn` / `seg-lim-min-up` /
//!   `seg-lim-max-dn` / `seg-lim-max-up` / `seg-lim-clear`.
//!   Two F-STEP rows step the selected node's
//!   `NodeDef::min_angle` / `max_angle` (degrees, 5°
//!   steps) — the joint limits the model's solvers
//!   (`aim_node` / `apply_ik`) and the graph Angle
//!   channels enforce on the node's RELATIVE angle.
//!   The core has no limit op, so the writes follow
//!   `attach_asset`'s pattern (validate → one
//!   `snapshot()` → mutate → status): one press is
//!   exactly one undo step, refusals are status-only,
//!   and a press that changes nothing (stepping at
//!   the ±180° hard clamp, clearing an unlimited
//!   node) is a silent no-op. An open side reads "Off"
//!   (`format_limit_bound`) and steps in from 0°. The
//!   rows are omitted for the origin, like Length and
//!   Bend: its angle is never solved, so a limit on it
//!   could never bind anything, and the ops refuse it.
//!   Clear limits renders only while a limit is set.
//!   A preset row (`seg-lim-preset-elbow` /
//!   `seg-lim-preset-knee`, Wave Y Y12) applies the
//!   model's anatomical ranges (`NodeDef::ELBOW_LIMITS`
//!   / `NodeDef::KNEE_LIMITS` — the humanoid
//!   constructor's own values, one source in the model)
//!   to the selected node in one press:
//!   `apply_joint_limit_preset` sets BOTH bounds under
//!   one snapshot, with the same refusal discipline,
//!   so a range the constructor ships is never 34
//!   stepper presses away on a figure built another
//!   way. Stored poses are never rewritten by a limit —
//!   enforcement binds solving, not data.
//! * Limit arc display (Roadmap Wave 2, W2-27) →
//!   `format_limit_arc` / `toggle_limit_arcs` (this
//!   module), under the preset row: while the node
//!   carries a limit, a readout line (key
//!   `seg-lim-arc-readout`) names the allowed/blocked
//!   sweep from the model's own partition
//!   (`model::limit_spans` — the arc `clamp_angle`
//!   enforces, so the line and the stage drawing agree
//!   by construction), and a kit toggle row (key
//!   `seg-lim-arc-toggle`, literal label "Limit arc on
//!   stage" — the i18n adoption is IA-W2-27-3) flips
//!   `State::limit_arcs`, the shell flag the stage's
//!   limit-arc pass reads (IA-W2-27-1). Display only:
//!   neither control writes the project, and no
//!   set-rest clamp is built — that stays the standing
//!   IA-AA9-1 user decision.
//! * Rig properties (Roadmap Wave 2, W2-26 — RM19 R6)
//!   — keys `seg-node-name` / `seg-node-head` /
//!   `seg-node-ik-end` / `seg-bend-side-neg` /
//!   `seg-bend-side-pos`, mounted directly under the
//!   header cluster: the constructor-only `NodeDef`
//!   properties (name, head, IK end, bend side) become
//!   editable. The name field is a `TextEdit` session
//!   homed in [`SegmentNumberState`] (`name_edit`),
//!   seeded by [`sync_segment_name_field`] from the
//!   poll sync ([`sync_number_edits`]); Enter submits
//!   to the core's `set_node_name` (the trim and the
//!   empty refusal live there). Head / IK end are kit
//!   toggle rows onto `set_node_head` /
//!   `set_node_ik_end` — the IK chain refusal is the
//!   core's standing sentence — and Bend side is a
//!   −/+ pair onto `set_node_bend_side`, writing
//!   `NodeDef.bend`'s ±1.0 sign: NOT the frame bend
//!   the Bend stepper row below steps. Every setter
//!   is validate-first in the core: exactly one
//!   `snapshot_labeled` per real change, a no-change
//!   write spends nothing, locked figures refuse in
//!   the standing sentence. The four labels are
//!   literals this wave (the i18n splice RM19 priced
//!   as IA-RM19-8 is filed as IA-W2-26-2).
//! * Cycle bend → `cycle_bend()` (editor.rs:1106) — key
//!   `cycle-bend`. There is no ± bend op in the core; bend
//!   steps 0 → +0.35 → +0.7 → −0.35 → −0.7 rad per frame.
//! * Cycle shape → `cycle_shape()` (editor.rs:494) — key
//!   `node-shape` (line → circle → triangle → square →
//!   pentagon → hexagon → line).
//! * Colour swatches → `set_node_color(rgb)` (editor.rs:521)
//!   — keys `seg-color-{rrggbb}`; the palette mirrors the
//!   core's private `NODE_COLORS` cycle (editor.rs:484).
//! * Cycle colour → `cycle_node_color()` (editor.rs:506) —
//!   key `node-color`; the only op that can return a segment
//!   to the figure colour (`set_node_color` always writes
//!   `Some(rgb)`), so it stays as the "default" path.
//! * Media (Wave 12) — keys `seg-media-readout`,
//!   `seg-media-attach-{i}`, `seg-media-cycle`,
//!   `seg-media-import`, `seg-media-clear`. Before this
//!   wave the panel had NO media surface at all: the
//!   only attach path in the app was the core's
//!   `attach_image_to_node`, which takes the first PNG
//!   in ./import, appends it as a NEW asset and attaches
//!   that — no way to pick an asset the project already
//!   holds — and `clear_node_media` had no panel caller
//!   at all (only MCP reached it, while its own tool
//!   description still called it "the segment
//!   inspector's Clear media", a GPUI-era leftover).
//!   Now a Media section closes the section list: the
//!   readout names the attached media (`media_label` —
//!   the sprite's asset name, the text label's content,
//!   or "No media attached"); one button per asset in
//!   `project.assets` (the fx_browser wrapping-row
//!   idiom; the attached asset's button renders active)
//!   attaches THAT asset via `attach_asset` (this
//!   module — the core has no attach-by-index op, so
//!   the write follows `rotate_selected`'s pattern:
//!   validate → one `snapshot()` → write → status, and
//!   re-attaching the already-attached asset is a
//!   silent no-op, like Straighten at bend 0). The row
//!   is capped at `ASSET_BUTTON_CAP` buttons; past the
//!   cap a `seg-media-cycle` "Next sprite" button
//!   reaches every remaining asset by cycling the
//!   attachment. "Attach from import" keeps the core's
//!   import-folder op one press away (and is the only
//!   attach path while the project holds no assets),
//!   and Clear — rendered only while media is attached
//!   — calls `clear_node_media` directly. Text labels
//!   are READ here but EDITED in the text tool panel
//!   (its `SegTextEdit` owns content and size); the
//!   model has ONE media slot per segment, so attaching
//!   a sprite replaces a label, and `attach_asset`'s
//!   status says so when it happens.
//! * Sprite transform (Wave Y, P-8) — keys
//!   `seg-media-offx-dn/up`, `seg-media-offy-dn/up`,
//!   `seg-media-scale-dn/up`, `seg-media-rot-dn/up`,
//!   `seg-media-flip`, `seg-media-reset`. The Media
//!   section grows a transform block under the picker,
//!   rendered only while the segment carries a sprite:
//!   offset X/Y steppers (stage units), a scale
//!   stepper, a rotation stepper (degrees — the
//!   panel's one angle language) and a Flip toggle
//!   edit the `SegmentMedia::Image` local transform
//!   Y3 composed into the `media_items` funnel, so the
//!   stage, raster and SVG all follow the same values.
//!   The writes are this module's own ops
//!   (`adjust_media_offset` / `adjust_media_scale` /
//!   `adjust_media_rotate` / `toggle_media_flip` /
//!   `reset_media_transform`), in `attach_asset`'s
//!   pattern — validate → ONE `snapshot()` → mutate →
//!   status — because the core has no by-index media
//!   op; a press that lands on the stored value (a
//!   step at a clamp) is a silent no-op, and Reset —
//!   rendered only while the transform differs from
//!   the defaults — restores them in one step. The
//!   MCP twins are `asset_attach` / `clip_asset_attach`
//!   with their optional transform args (media.rs).
//! * Sprite opacity (Wave AA, AA1) — keys
//!   `seg-media-opacity-dn/up`. Z5 gave the image
//!   payload a stored `opacity` of its own — the
//!   funnel multiplies it into the item alpha and the
//!   attach tools write it — but the panel could
//!   neither see nor change it: the transform
//!   write-back preserved it through its `..` and
//!   nothing else reached it. The transform block
//!   gains a fifth stepper row for it
//!   (`adjust_media_opacity`, in
//!   `adjust_media_offset`'s discipline — validate →
//!   ONE `snapshot()` → mutate → status), at the
//!   app's one opacity step (0.05 — the style and
//!   sprite panels' step) with whole-percent rounding
//!   and the funnel's 0..=1 clamp; the readout is the
//!   whole-percent form (`format_media_opacity`, the
//!   figure/style panels' display). The value the
//!   row shows and steps from is the stored value
//!   sanitised by the funnel's own rule
//!   (`sanitise_media_opacity`: non-finite → 1.0,
//!   clamped), so a hand-edited corrupt value reads
//!   as what renders and a changing press repairs it.
//!   The stepper moves ONLY the opacity — the
//!   transform's `..` preservation now runs in both
//!   directions. The MCP twin is the `opacity` arg on
//!   `asset_attach` / `clip_asset_attach` (media.rs).
//!   This is the media payload's own opacity — NOT
//!   the export alpha toggle, which stays a standing
//!   user decision, unbuilt.
//! * Clip-def fallback media (Wave AC, AC5) — key
//!   `seg-media-from-clip`. A movieclip instance whose
//!   node carries no media renders the clip def's
//!   media at that index (media.rs, Wave 14 C1), but
//!   this section read only the node's own slot, so
//!   such a segment announced "No media attached"
//!   while a sprite drew on it, and the media's
//!   values were visible nowhere in the panel.
//!   `segment_state` now also resolves the fallback
//!   with the funnel's own rule (`clip_media`):
//!   while it is live the readout names the clip's
//!   media, a provenance line names the clip it
//!   comes from, and an Image payload's transform +
//!   opacity appear as READ-ONLY readout rows (the
//!   writable block's labels and formatters, no
//!   presses). The writable surface is unchanged and
//!   keeps its meaning: the picker, import, steppers
//!   and Clear write the node's own slot — attaching
//!   over a fallback is the funnel's documented
//!   override (the instance's media wins), never a
//!   silent edit of the clip's store, which is
//!   edited through the MCP clip doors
//!   (`clip_get_node_media` / `clip_asset_attach` /
//!   `clip_clear_node_media`). The two new labels
//!   ride IA-AC5-1.
//! * + Segment → `add_segment()` (editor.rs:402) — key
//!   `add-seg`.
//! * Delete segment → `delete_segment()` (editor.rs:434) —
//!   key `del-seg`, danger-styled; the op itself refuses the
//!   root and non-leaves with a status message.
//! * Index line → `segment_topology` (this module) — one
//!   muted line under the header giving the node's place
//!   in the figure tree: its own index, its parent's
//!   index (or "root") and how many children hang off
//!   it. Read-only. Length and world angle already have
//!   their own readout rows (see
//!   `InspectorSegmentState`), so this adds the topology
//!   the header's bare "Segment N" does not show.
//! * Joint navigation → `select_parent` /
//!   `select_first_child` (this module; Phase K) —
//!   keys `seg-nav-parent` / `seg-nav-child` in a row
//!   under the readout lines: the Index line's topology
//!   as buttons. Parent moves the selection to the
//!   node's parent; Child to its FIRST child (the
//!   lowest-index node naming it as parent — see
//!   `parent_of` / `first_child_of`). Selection is not
//!   an undoable edit — shell selection changes are
//!   bare `selected_node` field writes (home_panel
//!   clears the field the same way) — so the presses
//!   take no snapshot and set no status, and at the
//!   root (no parent) or a leaf (no child) a press is
//!   a silent no-op. (The audit behind this addition:
//!   length steppers already exist above —
//!   `insp-len-dn` / `insp-len-up` over
//!   `adjust_length`, editor.rs:489 — so the panel's
//!   missing segment control was navigation, not
//!   length.)
//!
//! When nothing is selected the panel renders its
//! header over the kit's empty state ("Select a segment
//! on the stage to edit its geometry."; UI mission
//! Wave 4, audit s08 I7 — the header-always form,
//! replacing the bare line this panel used to be, and
//! worded against the style panel's "…its style." so
//! the two stacked cards no longer near-duplicate).
//!
//! Phase H icon pass: the header title leads with the
//! `shapes` glyph (`icons::icon_el`, rendered only when
//! the icon set has it), Cycle bend renders through
//! `icon_btn` with the `repeat` glyph, and Delete
//! segment keeps its danger styling with the `trash-can`
//! glyph inside it — element keys and labels unchanged
//! everywhere. Every stepper (Length, Thickness,
//! Rotate, Bend) deliberately stays text: compactness
//! is their point.
//!
//! Roadmap Wave 1 (F-3, RM4): the four primary geometry
//! rows — Length, Thickness, Rotate, Bend — carry the
//! design kit's number field between their existing
//! step buttons (new keys `insp-len-value`,
//! `insp-th-value`, `seg-rot-value`, `seg-bend-value`):
//! click to type, drag to scrub. Length/Thickness ride
//! RM3's core setters (`set_length` / `set_thickness`,
//! quiet halves for scrub); Rotate/Bend gain exact-set
//! siblings here (`set_angle_selected` /
//! `set_bend_selected`, one snapshot per commit) with
//! silent write halves under one lazy snapshot per
//! scrub ([`ScrubGesture`]). The Rotate field reads the
//! STORED pose angle — the quantity its buttons step —
//! not the world angle of the summary readout (see
//! [`stored_angle_deg`]). Session state lives in
//! [`SegmentNumberState`] on `State` (IA-RM4-2); the
//! Escape-revert sync is [`sync_number_edits`]
//! (IA-RM4-1); the fields' help / invalid copy rides
//! IA-RM3-1's keys. The joint-limit rows (an open side
//! is "Off" — no honest number) and the media-transform
//! rows (delta-only ops, no exact setters yet) keep
//! their steppers; see RM4's findings for the follow-on
//! recipe.
//!
//! Phase N adoption: the root is the kit card
//! (`design::panel_col`), the header is
//! `design::panel_title` (the `shapes` glyph rides
//! inside it, still guarded by the kit's own `has_icon`
//! check) over a `design::divider`, the steppers are
//! F-STEP (charter order — label, −, `design::
//! stepper_value`, +; the Rotate/Bend multi-step
//! clusters keep their buttons and only their readouts
//! become `stepper_value`), Delete segment is the kit
//! `danger_icon_btn` (this file's local hex-literal
//! copy is deleted), the colour swatches are
//! `design::swatch`, and the "Colour" heading is a kit
//! `section_label`. Keys, handlers and status strings
//! are unchanged.
//!
//! UI mission Wave 9 convergence: the header title now
//! follows the figure inspector's exact construction —
//! `panel_title`'s title is "Segment {node}" and the
//! node name rides in the meta slot, where this panel
//! used to bake both into one title string with a " · "
//! separator. Same information, same order, same glyph;
//! the name now renders in the kit's
//! LABEL/`text_secondary` meta style like the figure
//! panel's name does. Wave 10 round 2: the meta copy is
//! fitted with the figure inspector's own header-name
//! budget, because the model's name is unbounded while
//! the header row is not; `InspectorSegmentState` still
//! carries the full name.
//!
//! Localisation (Wave T, T4): every user-visible label
//! in the panel — the header titles, the empty state,
//! the joint-navigation buttons, the readout and
//! stepper row labels, the section headings and the
//! media controls — resolves through `i18n_wire::tr`
//! under the `segment_*` keys (IA-T4-1; the keys land
//! in `i18n.rs` at integration, until then `tr` falls
//! back to the key itself). English output is
//! byte-identical to the pre-wiring literals, pinned by
//! `segment_i18n_keys_exist_and_match_the_english_labels`.
//! The populated header title is the one template
//! (`segment_title_node`, "Segment {index}", the camera
//! `follow_label` precedent). Locale-invariant notation
//! stays literal by design (the R6 precedent): the
//! degree steppers (−5°/−1°/+1°/+5°) and the −/+
//! stepper glyphs. Readout VALUES and composed readout
//! lines stay English by the same standing remainder
//! as the 3D panel's composed readouts: the shape
//! names (`shape_label`), the media readout
//! (`media_label`), the Index line (`format_topology`)
//! and the Bend readout line (`format_bend_readout`)
//! are dynamic content, not label arguments.
//!
//! Localisation (Wave U, U7): those composed readout
//! lines now resolve through template keys (IA-U7-1),
//! the camera `follow_label` idiom — the helpers take
//! `&EditorCore` first, exactly like `follow_label`:
//! `media_label` (the `segment_media_*` keys, one per
//! slot state), `format_topology` (the
//! `segment_topology*` keys — the line template
//! composes the parent/root and child/children parts,
//! each composed from its own key) and
//! `format_bend_readout` (`segment_bend_readout`,
//! "Bend {degrees}°"). English output is byte-identical
//! to the pre-wiring strings, pinned by the extended
//! `segment_i18n_keys_exist_and_match_the_english_labels`
//! and the helpers' own tests — all of which resolve
//! only once IA-U7-1 lands (the Q7 pattern). The shape
//! names stay English by standing rule: `shape_label`'s
//! values are model vocabulary, not labels.

use rune::tok::{type_, Theme};
use rune::{row, text, Color, El, TextEdit, Tx};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::{angle_of, limit_spans, ImageAsset, NodeDef, SegmentMedia, Shape};

use crate::chrome_view::btn;
use crate::rune_sec::design;
use crate::rune_sec::i18n_wire;
use crate::rune_sec::icon_btn::{danger_icon_btn, icon_btn};
use crate::stage_view::hex_color;
use crate::State;

/// Step sizes — the GPUI inspector's steps (its stepper
/// handlers called `adjust_length(±4.0)` /
/// `adjust_thickness(±1.0)`).
pub const LENGTH_STEP: f32 = 4.0;
pub const THICKNESS_STEP: f32 = 1.0;

/// The segment colour palette: the colour entries of the
/// core's `NODE_COLORS` cycle (editor.rs:484), in cycle
/// order. (`NODE_COLORS` also has a leading `None` — the
/// figure colour — which `set_node_color` cannot write; the
/// panel's Cycle colour button reaches it.)
pub const PALETTE: [u32; 6] = [0xFF5D5D, 0x5DB2FF, 0x7BE495, 0xFFC65D, 0xC792FF, 0x232830];

/// The panel-side view of a segment sprite's local
/// transform (Wave Y, P-8): the four flat fields of
/// `SegmentMedia::Image`'s payload (Y3's shape),
/// carried as one value so the stepper rows and
/// their ops pass it around whole. `IDENTITY` is
/// the payload's serde defaults — exactly how every
/// pre-P-8 project renders: no offset, natural size,
/// chord-aligned, unflipped.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MediaTransformState {
    /// Translation of the image centre in stage
    /// units, in the segment's own frame: `[along
    /// the segment direction, along its normal]`.
    pub offset: [f32; 2],
    /// Uniform multiplier on the fitted quad.
    pub scale: f32,
    /// Extra rotation about the image centre, degrees.
    pub angle_offset: f32,
    /// Mirror the image along the segment axis.
    pub flip: bool,
}

impl MediaTransformState {
    /// The identity transform — the payload defaults.
    pub const IDENTITY: MediaTransformState = MediaTransformState {
        offset: [0.0, 0.0],
        scale: 1.0,
        angle_offset: 0.0,
        flip: false,
    };

    /// True when every field sits at its default (the
    /// Reset button renders only while this is false).
    pub fn is_identity(&self) -> bool {
        *self == MediaTransformState::IDENTITY
    }
}

/// Step for the media offset steppers, stage units.
pub const MEDIA_OFFSET_STEP: f32 = 2.0;
/// Hard clamp for either media offset axis, stage
/// units — far past any stage the app draws, so the
/// clamp only ever catches a runaway press sequence.
pub const MEDIA_OFFSET_LIMIT: f32 = 1000.0;
/// Step for the media scale stepper.
pub const MEDIA_SCALE_STEP: f32 = 0.1;
/// The media scale's clamp range: below 0.1 the
/// sprite all but vanishes, above 8 it swamps the
/// stage; the StepForms mirror these bounds exactly.
pub const MEDIA_SCALE_MIN: f32 = 0.1;
pub const MEDIA_SCALE_MAX: f32 = 8.0;
/// Step for the media rotation stepper, degrees —
/// the Rotate/Bend rows' coarse step (LIMIT_STEP's
/// language), and the stored offset is hard-clamped
/// to ±180° like a joint limit.
pub const MEDIA_ROTATE_STEP: f32 = 5.0;
/// Step for the media opacity stepper (Wave AA,
/// AA1) — the app's one opacity step: the style
/// panel's and the sprite panel's `OPACITY_STEP`.
pub const MEDIA_OPACITY_STEP: f32 = 0.05;

/// Media the selected node renders through the
/// movieclip fallback (Wave AC, AC5): the figure is
/// a clip instance, its node's own media slot is
/// empty, and the clip def's node at the same index
/// carries media — the payload every surface renders
/// (media.rs, Wave 14 C1). Carried so the Media
/// section can present it READ-ONLY: the section's
/// presses write the node's own slot, never the
/// clip's store, so this state feeds text readouts
/// only — no stepper, no button.
#[derive(Clone, Debug, PartialEq)]
pub struct ClipMediaState {
    /// The clip's name, fitted like the media
    /// label's asset names — the provenance line's
    /// `{name}`.
    pub clip_name: String,
    /// `media_label` over the clip node's media —
    /// the same readout text the node's own media
    /// would produce, so the readout names what
    /// renders.
    pub label: String,
    /// The clip payload's local transform, when it
    /// is `SegmentMedia::Image` — the values behind
    /// the read-only transform rows. `None` for a
    /// text label.
    pub transform: Option<MediaTransformState>,
    /// The clip payload's stored opacity, sanitised
    /// by the funnel's rule — `Some` exactly when
    /// `transform` is.
    pub opacity: Option<f32>,
}

/// Derived view-state for the panel: everything the builder
/// renders, computed once from the core so the builder stays
/// a pure function of it (and so the readout math is unit
/// testable without elements).
#[derive(Clone, Debug, PartialEq)]
pub struct InspectorSegmentState {
    /// Selected figure index.
    pub figure: usize,
    /// Selected node index within the figure's definition.
    pub node: usize,
    /// Node name from the definition.
    pub name: String,
    /// True for the figure's origin node (no parent segment).
    pub is_root: bool,
    /// The node's head flag (`NodeDef::head`) — the
    /// value behind the Head toggle (Roadmap Wave 2,
    /// W2-26, RM19 R6).
    pub head: bool,
    /// The node's IK-end flag (`NodeDef::ik_end`) — the
    /// value behind the IK end toggle (W2-26).
    pub ik_end: bool,
    /// The node's preferred IK bend side
    /// (`NodeDef::bend`, a ±1.0 sign by convention) —
    /// the value behind the Bend side −/+ pair (W2-26).
    /// NOT the current frame's bend (that is
    /// `bend_deg`, the Bend readout's quantity).
    pub bend_side: f32,
    /// Segment length (definition units).
    pub length: f32,
    /// Segment thickness (definition units).
    pub thickness: f32,
    /// World direction of the segment at the current frame,
    /// degrees in `[0, 360)`, in the model's own angle
    /// convention (`model::angle_of`: 0° down, 90° right,
    /// 180° up, 270° left) — from the solved positions, so
    /// it agrees with the pose angles the stage solver
    /// writes. For the root (no segment) it is the node's
    /// pose angle at the frame.
    ///
    /// Wave 11 (C10) precision note: this is the WORLD
    /// angle, not the stored pose angle. Pose angles are
    /// parent-relative (`model::evaluate` accumulates
    /// them down the tree), so for a node deeper than a
    /// root child this readout differs from
    /// `frames[current].poses[fi].angles[ni]` by the
    /// ancestors' accumulated angle — and that stored
    /// relative value, raw and unnormalised, is what
    /// MCP's `get_pose` returns. The two surfaces also
    /// differ when an `Angle(ni)` graph channel overrides
    /// the sampled pose: this readout is graph-aware
    /// (`positions_f`), `get_pose` is not. Same
    /// convention, same unit — different quantity.
    pub angle_deg: f32,
    /// Current frame's bend for this node, degrees (0 when
    /// the frame stores no bend for it — the same read
    /// `cycle_bend` performs).
    pub bend_deg: f32,
    /// Joint limits on the node's relative angle, degrees,
    /// read from the definition (`NodeDef::min_angle`) —
    /// the values behind the Min angle stepper row
    /// (Wave X, P-4). `None` = that side is open.
    pub min_angle: Option<f32>,
    /// The upper joint limit (`NodeDef::max_angle`); see
    /// `min_angle`.
    pub max_angle: Option<f32>,
    /// The node's own colour, or `None` for the figure colour.
    pub color: Option<u32>,
    /// Human label of the node's shape ("Line", "Circle",
    /// "Triangle", …).
    pub shape_label: String,
    /// The node's attached media, cloned from the
    /// definition (Wave 12) — the raw value behind
    /// `media_label`, carried so the builder stays a
    /// pure function of this state.
    pub media: Option<SegmentMedia>,
    /// The attached sprite's asset index, when `media`
    /// is `SegmentMedia::Image` — the picker button that
    /// renders active. `None` for a text label or no
    /// media.
    pub attached_asset: Option<usize>,
    /// The attached sprite's local transform (Wave Y,
    /// P-8), when `media` is `SegmentMedia::Image` —
    /// the values behind the Media section's transform
    /// stepper rows. `None` for a text label or no
    /// media (the rows render only for a sprite).
    pub media_transform: Option<MediaTransformState>,
    /// The attached sprite's stored payload opacity
    /// (Wave Z, Z5), sanitised by the funnel's rule —
    /// the value behind the Media section's Opacity
    /// stepper row (Wave AA, AA1). `Some` exactly when
    /// `media_transform` is (both read the sprite
    /// payload); `None` for a text label or no media.
    pub media_opacity: Option<f32>,
    /// The clip-def fallback media this node renders
    /// (Wave AC, AC5), resolved with the funnel's own
    /// rule: `Some` only while the node's own media
    /// slot is empty AND the figure's clip def carries
    /// media at this node index. Read-only
    /// presentation state — see `ClipMediaState`.
    pub clip_media: Option<ClipMediaState>,
    /// Readout text for the Media section: the attached
    /// sprite's asset name, the text label's content, or
    /// "No media attached" (see `media_label`).
    pub media_label: String,
    /// Names of the project's assets (`project.assets`),
    /// in asset order — the sprite picker's buttons.
    /// Names only: the PNG bytes stay in the model.
    pub asset_names: Vec<String>,
}

/// Wrap an angle in degrees into `[0, 360)`.
pub fn norm_deg_360(a: f32) -> f32 {
    let r = a % 360.0;
    if r < 0.0 {
        r + 360.0
    } else {
        r
    }
}

/// Format an angle in degrees with one decimal place. The
/// single formatter behind the Angle readout and the Rotate
/// row's value, so the two can never disagree.
pub fn format_deg(deg: f32) -> String {
    format!("{deg:.1}°")
}

/// Maximum characters a whole-number display value may
/// occupy before the display switches to compact
/// scientific notation. Model values are not bounded by
/// the panel's own steppers: imports and the MCP
/// `set_node` / `set_bend` tools can store lengths,
/// thicknesses and bends far outside the stepper ranges,
/// and a plain `text` value does not shrink to fit its
/// row. Twelve characters leaves every ordinary value
/// byte-identical to the historical `{:.0}` form while
/// keeping even the four-button Bend row inside the
/// 316 pt content budget.
pub const DISPLAY_NUMBER_MAX_CHARS: usize = 12;

/// Format a model value as a whole number for a readout
/// or stepper, switching to one-decimal scientific
/// notation only when the plain decimal form would be
/// too wide for the panel (see
/// [`DISPLAY_NUMBER_MAX_CHARS`]). Non-finite values keep
/// their short Rust forms (`inf`, `NaN`).
pub fn format_whole_number(value: f32) -> String {
    let plain = format!("{value:.0}");
    if plain.chars().count() <= DISPLAY_NUMBER_MAX_CHARS {
        plain
    } else {
        format!("{value:.1e}")
    }
}

/// Format a bend in degrees for the Bend readout and
/// stepper: exactly [`format_deg`] for ordinary values,
/// compact scientific notation for an imported or
/// MCP-written bend whose plain form would overflow the
/// row. (The Angle readout needs no such guard: its
/// value is normalised into `[0, 360)` first.)
pub fn format_deg_display(deg: f32) -> String {
    let plain = format_deg(deg);
    if plain.chars().count() <= DISPLAY_NUMBER_MAX_CHARS {
        plain
    } else {
        format!("{deg:.1e}°")
    }
}

/// Fit a node name for the panel header's meta slot,
/// using the figure inspector's own header budget and
/// word-boundary rule. The full name stays in the model
/// and in [`InspectorSegmentState`]; only this display
/// copy is shortened, exactly as the figure panel's
/// header treats the figure name.
pub fn fit_segment_header_name(name: &str) -> String {
    crate::rune_sec::inspector_figure::fit_header_name(name)
}

/// Rotate the selected node's angle at the current frame by
/// `delta_deg` degrees (see the module docs for why this is
/// a pose write and what its undo semantics are). Refusals
/// are status-only — no snapshot, no edit — exactly like
/// the core ops' guards.
pub fn rotate_selected(core: &mut EditorCore, delta_deg: f32) {
    let Some(ni) = core.selected_node else {
        core.status = "Select a segment first, then rotate it.".into();
        return;
    };
    let fi = core.selected_figure;
    let has_angle = core
        .project
        .frames
        .get(core.current)
        .and_then(|f| f.poses.get(fi))
        .and_then(|p| p.angles.get(ni))
        .is_some();
    if !has_angle {
        core.status = format!(
            "Figure {} has no pose angle for segment {} on frame {}.",
            fi + 1,
            ni,
            core.current + 1
        );
        return;
    }
    core.snapshot_labeled("Rotate segment");
    let pose = &mut core.project.frames[core.current].poses[fi];
    pose.angles[ni] += delta_deg;
    let new = pose.angles[ni];
    core.status = format!(
        "Segment {} angle {} on frame {}.",
        ni,
        format_deg(new),
        core.current + 1
    );
}

/// The bend limit, in radians: the extreme of
/// `cycle_bend`'s cycle (editor.rs:1143, steps
/// 0 / ±0.35 / ±0.7). The model stores bends unclamped,
/// but ±0.7 rad (≈40.1°) is the largest magnitude the
/// engine itself ever writes, so the steppers clamp
/// there — see the module docs.
pub const BEND_LIMIT_RAD: f32 = 0.7;

/// Bend the selected node's segment at the current frame
/// by `delta_rad` radians — the storage unit of
/// `Frame.bends` (see the module docs for the storage,
/// the clamp, and the Bend channel that — since Wave Y
/// P-7, like the Angle channel for poses — overrides
/// this stored value at render). Same discipline as
/// `rotate_selected`:
/// validate → ONE `snapshot()` → write → status, so one
/// press is exactly one undo step; refusals are
/// status-only. The bend table is sparse, so the write
/// grows it first, in `cycle_bend`'s own pattern
/// (editor.rs:1164): outer vec to the figure, inner vec
/// to the figure's node count, zero-filled.
pub fn bend_selected(core: &mut EditorCore, delta_rad: f32) {
    let Some(ni) = core.selected_node else {
        core.status = "Select a segment first, then bend it.".into();
        return;
    };
    if ni == 0 {
        // Mirrors cycle_bend's refusal (editor.rs:1135):
        // the origin has no parent segment to arc.
        core.status = "The origin has no segment to bend.".into();
        return;
    }
    let fi = core.selected_figure;
    let nodes = match core.project.figures.get(fi) {
        Some(fig) if ni < fig.def.nodes.len() => fig.def.nodes.len(),
        _ => {
            core.status = format!(
                "Figure {} has no segment {} to bend.",
                fi + 1,
                ni
            );
            return;
        }
    };
    if core.current >= core.project.frames.len() {
        core.status = format!("No frame {} to bend on.", core.current + 1);
        return;
    }
    let cur = core.project.frames[core.current]
        .bends
        .get(fi)
        .and_then(|b| b.get(ni))
        .copied()
        .unwrap_or(0.0);
    let new = (cur + delta_rad).clamp(-BEND_LIMIT_RAD, BEND_LIMIT_RAD);
    core.snapshot_labeled("Bend segment");
    let frame = &mut core.project.frames[core.current];
    if frame.bends.len() <= fi {
        frame.bends.resize(fi + 1, Vec::new());
    }
    if frame.bends[fi].len() < nodes {
        frame.bends[fi].resize(nodes, 0.0);
    }
    frame.bends[fi][ni] = new;
    core.status = format!(
        "Segment {} bend {} on frame {}.",
        ni,
        format_deg(new.to_degrees()),
        core.current + 1
    );
}

/// The selected node's bend at the current frame, in
/// degrees, or `None` when no (valid) node is selected —
/// the same defensive indexing as `segment_state`, so a
/// stale selection reads as "nothing selected" instead
/// of panicking. The read is the stored-value read the
/// panel already uses for its Bend readout (the current
/// frame's `Frame.bends` entry, sparse-missing as 0.0),
/// NOT the renderers' interpolated
/// `Project::bends_at`: the steppers and `cycle_bend`
/// also work on the stored value, so every bend surface
/// in the panel agrees. Nor is it the channelled value:
/// since Wave Y (P-7) a `Bend(ni)` graph channel
/// overrides the stored bend at render (see the module
/// docs) — the channelled number is the Graph panel's
/// readout, while this stored read stays the value the
/// steppers step FROM, so the panel never disagrees
/// with its own writes. The origin qualifies (its
/// stored bend is definitionally 0 — nothing can write
/// it). This is the value behind the panel's
/// `seg-bend-readout` line.
pub fn bend_degrees(core: &EditorCore) -> Option<f32> {
    let ni = core.selected_node?;
    let fi = core.selected_figure;
    let fig = core.project.figures.get(fi)?;
    if fig.def.nodes.get(ni).is_none() {
        return None;
    }
    Some(
        core.project
            .frames
            .get(core.current)
            .and_then(|f| f.bends.get(fi))
            .and_then(|b| b.get(ni))
            .copied()
            .unwrap_or(0.0)
            .to_degrees(),
    )
}

// ----------------------------------------------------------------
// F-3 number fields (Roadmap Wave 1, RM4)
// ----------------------------------------------------------------

/// The selected node's STORED bend at the current frame,
/// in radians — [`bend_degrees`] without the degree
/// conversion: the quantity `bend_selected` steps and
/// the bend number field edits. `None` exactly when
/// `bend_degrees` is (no valid node selected).
pub fn stored_bend_rad(core: &EditorCore) -> Option<f32> {
    bend_degrees(core).map(|d| d.to_radians())
}

/// The selected node's STORED pose angle at the current
/// frame, in degrees — the quantity `rotate_selected`
/// steps (`frames[current].poses[fi].angles[ni]`, raw).
/// This is deliberately NOT `InspectorSegmentState::
/// angle_deg`: that readout is the WORLD angle from the
/// solved positions (its own docs carry the Wave 11
/// precision note), while the Rotate row's buttons have
/// always stepped the stored relative angle — so the
/// row's number field reads and writes the stored
/// value, the quantity its own presses move. (For a
/// root child the two agree; deeper in the tree they
/// differ by the ancestors' accumulated angle, and the
/// panel's "Angle" readout line above keeps the world
/// read.) `None` when no node is selected or the frame
/// stores no pose angle for it — the field then does
/// not mount and the row keeps its plain readout.
pub fn stored_angle_deg(core: &EditorCore) -> Option<f32> {
    let ni = core.selected_node?;
    let fi = core.selected_figure;
    core.project
        .frames
        .get(core.current)?
        .poses
        .get(fi)?
        .angles
        .get(ni)
        .copied()
}

/// The (figure, node) an angle write may target, with
/// `rotate_selected`'s two refusal texts posted when it
/// may not — the shared validation of the angle writers
/// below.
fn angle_write_target(core: &mut EditorCore) -> Option<(usize, usize)> {
    let Some(ni) = core.selected_node else {
        core.status = "Select a segment first, then rotate it.".into();
        return None;
    };
    let fi = core.selected_figure;
    let has_angle = core
        .project
        .frames
        .get(core.current)
        .and_then(|f| f.poses.get(fi))
        .and_then(|p| p.angles.get(ni))
        .is_some();
    if !has_angle {
        core.status = format!(
            "Figure {} has no pose angle for segment {} on frame {}.",
            fi + 1,
            ni,
            core.current + 1
        );
        return None;
    }
    Some((fi, ni))
}

/// Set the selected node's stored pose angle to exactly
/// `deg` degrees (Roadmap Wave 1, F-3): the exact-set
/// sibling of `rotate_selected` — same validation and
/// refusals, ONE snapshot, the op family's status
/// wording. The number field's typed commit.
pub fn set_angle_selected(core: &mut EditorCore, deg: f32) {
    let Some((fi, ni)) = angle_write_target(core) else {
        return;
    };
    core.snapshot_labeled("Set segment angle");
    core.project.frames[core.current].poses[fi].angles[ni] = deg;
    core.status = format!(
        "Segment {} angle {} on frame {}.",
        ni,
        format_deg(deg),
        core.current + 1
    );
}

/// The silent half of [`set_angle_selected`]: the write
/// with no snapshot and no status — the number field's
/// scrub write, under [`ScrubGesture`] bookkeeping.
/// `false` when the target is invalid (nothing written).
pub fn write_angle_quiet(core: &mut EditorCore, deg: f32) -> bool {
    let Some(ni) = core.selected_node else {
        return false;
    };
    let fi = core.selected_figure;
    let Some(angle) = core
        .project
        .frames
        .get_mut(core.current)
        .and_then(|f| f.poses.get_mut(fi))
        .and_then(|p| p.angles.get_mut(ni))
    else {
        return false;
    };
    *angle = deg;
    true
}

/// The (figure, node, node-count) a bend write may
/// target, with `bend_selected`'s refusal texts posted
/// when it may not — the shared validation of the bend
/// writers below.
fn bend_write_target(core: &mut EditorCore) -> Option<(usize, usize, usize)> {
    let Some(ni) = core.selected_node else {
        core.status = "Select a segment first, then bend it.".into();
        return None;
    };
    if ni == 0 {
        core.status = "The origin has no segment to bend.".into();
        return None;
    }
    let fi = core.selected_figure;
    let nodes = match core.project.figures.get(fi) {
        Some(fig) if ni < fig.def.nodes.len() => fig.def.nodes.len(),
        _ => {
            core.status = format!(
                "Figure {} has no segment {} to bend.",
                fi + 1,
                ni
            );
            return None;
        }
    };
    if core.current >= core.project.frames.len() {
        core.status = format!("No frame {} to bend on.", core.current + 1);
        return None;
    }
    Some((fi, ni, nodes))
}

/// Grow the current frame's bend table to cover
/// (`fi`, `ni`) — `bend_selected`'s sparse-table growth,
/// in `cycle_bend`'s pattern — and store `rad` clamped
/// to the panel's bend range. Returns the stored value.
fn bend_write(core: &mut EditorCore, fi: usize, ni: usize, nodes: usize, rad: f32) -> f32 {
    let new = rad.clamp(-BEND_LIMIT_RAD, BEND_LIMIT_RAD);
    let frame = &mut core.project.frames[core.current];
    if frame.bends.len() <= fi {
        frame.bends.resize(fi + 1, Vec::new());
    }
    if frame.bends[fi].len() < nodes {
        frame.bends[fi].resize(nodes, 0.0);
    }
    frame.bends[fi][ni] = new;
    new
}

/// Set the selected node's stored bend to exactly
/// `rad` radians (Roadmap Wave 1, F-3): the exact-set
/// sibling of `bend_selected` — same validation and
/// refusals, ONE snapshot, the op family's status
/// wording. The number field's typed commit.
pub fn set_bend_selected(core: &mut EditorCore, rad: f32) {
    let Some((fi, ni, nodes)) = bend_write_target(core) else {
        return;
    };
    core.snapshot_labeled("Set segment bend");
    let new = bend_write(core, fi, ni, nodes, rad);
    core.status = format!(
        "Segment {} bend {} on frame {}.",
        ni,
        format_deg(new.to_degrees()),
        core.current + 1
    );
}

/// The silent half of [`set_bend_selected`]: the write
/// with no snapshot and no status — the number field's
/// scrub write, under [`ScrubGesture`] bookkeeping.
/// `false` when the target is invalid (nothing written).
pub fn write_bend_quiet(core: &mut EditorCore, rad: f32) -> bool {
    let Some(ni) = core.selected_node else {
        return false;
    };
    if ni == 0 {
        return false;
    }
    let fi = core.selected_figure;
    let nodes = match core.project.figures.get(fi) {
        Some(fig) if ni < fig.def.nodes.len() => fig.def.nodes.len(),
        _ => return false,
    };
    if core.current >= core.project.frames.len() {
        return false;
    }
    bend_write(core, fi, ni, nodes, rad);
    true
}

/// One number-field scrub gesture's undo bookkeeping —
/// the camera panel's `ScrubGesture` discipline, kept
/// per panel (the panels duplicate this idiom rather
/// than share a type): the Rotate/Bend families
/// snapshot per press, so a scrub takes ONE snapshot,
/// lazily, at its first real change, and writes
/// field-level after that. `press` is the value the
/// gesture started from; the kit's cancel path writes
/// exactly that value once more, which seals the
/// gesture (see the camera panel's `scrub_write` for
/// the full contract — identical here).
#[derive(Clone, Copy, Debug, Default)]
pub struct ScrubGesture {
    /// A snapshot has been taken for the in-flight scrub.
    pub open: bool,
    /// The value the in-flight scrub started from.
    pub press: f32,
}

/// A number field's silent mid-scrub write under
/// `gesture`'s bookkeeping (see [`ScrubGesture`]).
fn scrub_write(
    gesture: &mut ScrubGesture,
    core: &mut EditorCore,
    value: f32,
    current: f32,
    write: fn(&mut EditorCore, f32) -> bool,
    label: &str,
) {
    if gesture.open && value == gesture.press {
        write(core, value);
        gesture.open = false;
        return;
    }
    if !gesture.open {
        if value == current {
            return;
        }
        core.snapshot_labeled(label);
        gesture.press = current;
        gesture.open = true;
    }
    write(core, value);
}

/// A number field's scrub release: the final write and
/// the one status recap. A gesture that changed nothing
/// leaves no trace (no write, no recap).
fn scrub_finish(
    gesture: &mut ScrubGesture,
    core: &mut EditorCore,
    value: f32,
    current: f32,
    write: fn(&mut EditorCore, f32) -> bool,
    status: String,
    label: &str,
) {
    if gesture.open {
        write(core, value);
    } else if value != current {
        core.snapshot_labeled(label);
        write(core, value);
    } else {
        return;
    }
    gesture.open = false;
    core.status = status;
}

/// The adopted fields' element keys, as consts: the
/// kit number field takes its key as an argument rather
/// than through `.key()`, and the gate's key-diff reads
/// `*_KEY` consts — the keys stay diff-visible in their
/// carried form (the AC1 const-carried pattern).
pub const LEN_FIELD_KEY: &str = "insp-len-value";
/// See [`LEN_FIELD_KEY`].
pub const TH_FIELD_KEY: &str = "insp-th-value";
/// See [`LEN_FIELD_KEY`].
pub const ROT_FIELD_KEY: &str = "seg-rot-value";
/// See [`LEN_FIELD_KEY`].
pub const BEND_FIELD_KEY: &str = "seg-bend-value";

/// Persistent edit state for the Node name field
/// (Roadmap Wave 2, W2-26 — RM19 R6): the figure
/// rename field's discipline
/// (`inspector_figure::InspectorFigureEditState`) for
/// the selected NODE — a half-typed name must survive
/// re-renders, so the buffer cannot be derived from
/// the core each frame. Homed inside
/// [`SegmentNumberState`] (the segment inspector's
/// existing `State` home), so no new `State` field —
/// and none of the shell's full `State` literals —
/// had to move.
#[derive(Clone, Debug, Default)]
pub struct SegmentNameEditState {
    /// The name field's text buffer.
    pub name: TextEdit,
    /// The (figure, node) `name` was last synced from,
    /// so a selection change re-syncs the field even
    /// when two nodes happen to share a name.
    pub for_node: Option<(usize, usize)>,
}

impl SegmentNameEditState {
    pub fn new() -> Self {
        Self::default()
    }
}

/// The segment inspector's stored number-field state
/// (Roadmap Wave 1, F-3): one [`design::NumberEdit`]
/// session per adopted field, plus the scrub
/// bookkeeping of the two snapshotting families
/// (Rotate, Bend — Length/Thickness scrub through the
/// core's `*_quiet` setters and need none). Homed on
/// `State` by the integrator (IA-RM4-2), like the
/// camera panel's state. Roadmap Wave 2 (W2-26) adds
/// the Node name field's session ([`SegmentNameEditState`])
/// to the same home.
#[derive(Clone, Debug, Default)]
pub struct SegmentNumberState {
    /// Length field session (`insp-len-value`).
    pub len_edit: design::NumberEdit,
    /// Thickness field session (`insp-th-value`).
    pub th_edit: design::NumberEdit,
    /// Rotate field session (`seg-rot-value`).
    pub rot_edit: design::NumberEdit,
    /// Bend field session (`seg-bend-value`).
    pub bend_edit: design::NumberEdit,
    /// Rotate scrub bookkeeping.
    pub rot_scrub: ScrubGesture,
    /// Bend scrub bookkeeping.
    pub bend_scrub: ScrubGesture,
    /// Node name field session (`seg-node-name`,
    /// W2-26).
    pub name_edit: SegmentNameEditState,
}

/// The Length field's contract: the op family's own
/// range home and step (`EditorCore::LENGTH_MIN/MAX`,
/// the row's `LENGTH_STEP`).
pub fn length_spec() -> design::NumSpec {
    design::NumSpec::new(
        EditorCore::LENGTH_MIN,
        EditorCore::LENGTH_MAX,
        LENGTH_STEP,
    )
}

/// The Thickness field's contract: `EditorCore::
/// THICKNESS_MIN/MAX` and the row's `THICKNESS_STEP`.
pub fn thickness_spec() -> design::NumSpec {
    design::NumSpec::new(
        EditorCore::THICKNESS_MIN,
        EditorCore::THICKNESS_MAX,
        THICKNESS_STEP,
    )
}

/// The Rotate field's contract: degrees at the panel's
/// one-decimal display precision (`format_deg`); the
/// stored pose angle is unclamped by its op family, so
/// the field funnels typed input to ±360° — one full
/// turn either way, past which a stored angle is the
/// same direction anyway.
pub fn angle_spec() -> design::NumSpec {
    let mut spec = design::NumSpec::degrees(-360.0, 360.0, 1.0);
    spec.decimals = 1;
    spec
}

/// The Bend field's contract: stored RADIANS (the
/// storage unit) displayed in degrees at the panel's
/// one-decimal precision, over the panel's own bend
/// range (±`BEND_LIMIT_RAD`); one displayed degree per
/// scrub step.
pub fn bend_spec() -> design::NumSpec {
    design::NumSpec {
        min: -BEND_LIMIT_RAD,
        max: BEND_LIMIT_RAD,
        step: (1.0f32).to_radians(),
        decimals: 1,
        suffix: "°",
        display_scale: 180.0 / std::f32::consts::PI,
        points_per_step: 2.0,
    }
}

/// Poll sync (the `sync_name_edit` idiom, called from
/// the shell's `poll` — IA-RM4-1): revert any segment
/// number-field session the framework has blurred —
/// Escape lands here as a revert, per the kit's
/// session rules ([`design::NumberEdit::reconcile`]).
/// Roadmap Wave 2 (W2-26): the same poll call also
/// keeps the Node name field seeded ([`sync_segment_name_field`]),
/// so the name session needs no new poll line.
pub fn sync_number_edits(s: &mut State) {
    s.segment_numbers.len_edit.reconcile();
    s.segment_numbers.th_edit.reconcile();
    s.segment_numbers.rot_edit.reconcile();
    s.segment_numbers.bend_edit.reconcile();
    sync_segment_name_field(&s.core, &mut s.segment_numbers.name_edit);
}

/// Keep the Node name field showing the selected
/// node's current name — `inspector_figure::
/// sync_name_field`'s focus-guarded pattern, keyed on
/// the (figure, node) pair: while the field is focused
/// it is never touched (a sync mid-typing would eat
/// keystrokes); otherwise the field is re-seeded
/// whenever the selection moved or the node's name
/// changed elsewhere (a rig op, a load, an undo).
pub fn sync_segment_name_field(core: &EditorCore, edit: &mut SegmentNameEditState) {
    if edit.name.focused {
        return;
    }
    let Some(st) = segment_state(core) else {
        edit.for_node = None;
        return;
    };
    if edit.for_node != Some((st.figure, st.node)) || edit.name.text != st.name {
        edit.name.replace_text(st.name);
        edit.for_node = Some((st.figure, st.node));
    }
}

/// Enter (submit) in the Node name field: apply the
/// field's text to the selected node through the
/// core's setter (trim, empty refusal and the one
/// snapshot live there — `EditorCore::set_node_name`).
fn apply_segment_name(s: &mut State) {
    let raw = s.segment_numbers.name_edit.name.text.clone();
    let fi = s.core.selected_figure;
    if let Some(ni) = s.core.selected_node {
        s.core.set_node_name(fi, ni, &raw);
    }
}

/// The Node name row (W2-26): label, the field, and
/// no button — RM19 §5 priced exactly the field key
/// for the name (Enter submits, the `on_submit`
/// below); the figure rename's pencil button key was
/// not in R6's price. The field binds
/// [`SegmentNumberState`]'s `name_edit` buffer;
/// [`sync_segment_name_field`] keeps it seeded.
fn node_name_row(s: &State, t: &Theme) -> El {
    row()
        .w_full()
        .center_y()
        .gap(8.0)
        .child(text("Node name").font(type_::LABEL).color(t.text_secondary))
        .child(
            design::text_field(
                &s.segment_numbers.name_edit.name,
                type_::BODY,
                s.segment_numbers.name_edit.name.focused,
                t,
            )
            .key("seg-node-name")
            .single_line()
            .grow(1.0)
            .color(t.text)
            .on_edit(|s: &mut State| &mut s.segment_numbers.name_edit.name)
            .on_submit(|s: &mut State, _tx: &mut Tx| apply_segment_name(s)),
        )
}

/// Format a bend in degrees as the readout line's
/// text: whole degrees, the sign shown on negatives
/// ("Bend -20°"), a straight segment reading exactly
/// "Bend 0°" (a negative zero is normalised away —
/// `-0.0 == 0.0`, so the remap costs nothing). The
/// single formatter behind the `seg-bend-readout`
/// line, so the wording lives in exactly one place
/// (and is unit testable without elements). Wave U
/// (U7): the wording resolves through the
/// `segment_bend_readout` template key ("Bend
/// {degrees}°", IA-U7-1); the `{degrees}` slot takes
/// [`format_whole_number`]'s display form verbatim.
pub fn format_bend_readout(core: &EditorCore, deg: f32) -> String {
    let deg = if deg == 0.0 { 0.0 } else { deg };
    i18n_wire::tr(core, "segment_bend_readout")
        .replace("{degrees}", &format_whole_number(deg))
}

/// Straighten the selected node's segment: set its
/// bend at the current frame to exactly 0, through the
/// steppers' own write path (`bend_selected` with the
/// negated current bend), so a press is exactly one
/// undo step with `bend_selected`'s status. Two
/// details keep it honest:
///
/// * The delta is the exact negation of the STORED
///   RADIANS, read here — not `-bend_degrees().to_radians()`:
///   the radians → degrees → radians round trip can be
///   an ulp off, while `cur + (-cur)` is bit-exact 0.0.
/// * At bend 0 the press is a silent no-op — no write,
///   no status, NO undo step. `bend_selected` takes
///   its snapshot unconditionally once its guards pass,
///   so delegating a zero delta would still snapshot;
///   the guard has to live here. With no (valid)
///   selection there is no bend to read, so the call
///   delegates to `bend_selected` with a zero delta
///   purely for its refusals, which all return
///   status-only BEFORE the snapshot — nothing is
///   written on that path either.
pub fn straighten_selected(core: &mut EditorCore) {
    let Some(deg) = bend_degrees(core) else {
        bend_selected(core, 0.0);
        return;
    };
    if deg == 0.0 {
        return;
    }
    // `bend_degrees` succeeded, so the selection is
    // valid; the `unwrap_or` is unreachable armour, and
    // node 0 would simply take `bend_selected`'s root
    // refusal.
    let fi = core.selected_figure;
    let ni = core.selected_node.unwrap_or(0);
    let cur = core
        .project
        .frames
        .get(core.current)
        .and_then(|f| f.bends.get(fi))
        .and_then(|b| b.get(ni))
        .copied()
        .unwrap_or(0.0);
    bend_selected(core, -cur);
}

/// Step size for the joint-limit steppers, degrees —
/// the Rotate/Bend rows' coarse step, so the panel
/// speaks one step language for angles everywhere.
pub const LIMIT_STEP: f32 = 5.0;

/// Which joint limit a stepper press adjusts
/// (`adjust_joint_limit`'s target).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LimitBound {
    Min,
    Max,
}

/// Display form of one joint-limit bound for its
/// stepper row: the degrees for a set bound (the
/// panel's one angle formatter, `format_deg`), the
/// localised "Off" for an open side (the
/// `segment_limit_off` key, IA-X6-1). The single
/// formatter behind both limit rows, so the two can
/// never disagree about how a bound reads.
pub fn format_limit_bound(core: &EditorCore, bound: Option<f32>) -> String {
    match bound {
        Some(v) => format_deg(v),
        None => i18n_wire::tr(core, "segment_limit_off"),
    }
}

/// Adjust the selected node's joint limit by
/// `delta` degrees (Wave X, P-4) — the write behind
/// the Min/Max angle stepper rows. The core has no
/// limit op, so — exactly as `attach_asset` writes
/// the media slot the core has no by-index op for —
/// this writes the definition directly in the core
/// ops' own pattern: validate → ONE `snapshot()` →
/// mutate → status, so one press is exactly one undo
/// step. Refusals (no selection, the origin, a stale
/// node) are status-only, no snapshot.
///
/// Two deliberate details:
///
/// * An unset bound steps from 0°: the first press
///   engages it one step out in the pressed
///   direction, so engaging a knee's max at +5° or
///   its min at −5° is one press, not a mode switch.
/// * A press that lands on the value already stored
///   (stepping at the ±180° hard clamp) is a SILENT
///   no-op — no write, no status, no undo step —
///   the Straighten-at-zero rule.
///
/// The stored bound is hard-clamped to ±180°: past
/// the half-circle a bound's normalised form would
/// wrap and silently become its mirror, which no
/// press may do by accident. Setting min above max
/// is NOT refused: it is the model's wrap arc (the
/// allowed set runs clockwise from min through
/// ±180° to max — see `NodeDef::clamp_angle`), and
/// the steppers are how a user builds one on purpose.
pub fn adjust_joint_limit(core: &mut EditorCore, bound: LimitBound, delta: f32) {
    let Some(ni) = core.selected_node else {
        core.status = "Select a segment first, then set its joint limits.".into();
        return;
    };
    let fi = core.selected_figure;
    let (is_root, cur) = match core
        .project
        .figures
        .get(fi)
        .and_then(|f| f.def.nodes.get(ni))
    {
        Some(node) => (
            node.parent.is_none(),
            match bound {
                LimitBound::Min => node.min_angle,
                LimitBound::Max => node.max_angle,
            },
        ),
        None => {
            core.status = format!(
                "Figure {} has no segment {} to limit.",
                fi + 1,
                ni
            );
            return;
        }
    };
    if is_root {
        // The origin's angle is never solved (it has
        // no parent segment), so a limit on it could
        // never bind anything — refuse rather than
        // store a dead value.
        core.status = "The origin has no joint to limit.".into();
        return;
    }
    let new = (cur.unwrap_or(0.0) + delta).clamp(-180.0, 180.0);
    if cur == Some(new) {
        return;
    }
    core.snapshot_labeled(match bound {
        LimitBound::Min => "Joint limit minimum",
        LimitBound::Max => "Joint limit maximum",
    });
    // Validated above; the `if let` is unreachable
    // armour, not an expected path.
    if let Some(node) = core
        .project
        .figures
        .get_mut(fi)
        .and_then(|f| f.def.nodes.get_mut(ni))
    {
        match bound {
            LimitBound::Min => node.min_angle = Some(new),
            LimitBound::Max => node.max_angle = Some(new),
        }
    }
    core.status = format!(
        "Segment {} {} angle set to {}.",
        ni,
        match bound {
            LimitBound::Min => "minimum",
            LimitBound::Max => "maximum",
        },
        format_deg(new)
    );
}

/// Clear BOTH of the selected node's joint limits
/// (Wave X, P-4) — the write behind the Clear limits
/// button, which renders only while a limit is set.
/// Same discipline as `adjust_joint_limit`: validate
/// → ONE `snapshot()` → mutate → status; refusals
/// are status-only; and clearing a node that carries
/// Flip the stage's joint-limit arc display
/// (`State::limit_arcs`, Roadmap Wave 2, W2-27): the
/// shell feeds the flag to the stage's overlay, which
/// draws the selected node's limit partition at its
/// joint (see `stage_view::limit_arc_prims`). Shell
/// view state only — no snapshot, no status, no project
/// write: the guides toggles' footing in
/// settings_popover.
pub fn toggle_limit_arcs(s: &mut State) {
    s.limit_arcs = !s.limit_arcs;
}

/// The Limit arc readout line's text (Roadmap Wave 2,
/// W2-27): the selected node's allowed/blocked sweep in
/// whole degrees, from the model's own partition
/// ([`limit_spans`] — the arc `clamp_angle` enforces, so
/// the line and the stage drawing can never disagree).
/// `None` while the node carries no limits — the line
/// then does not render. Literal English like this
/// wave's other new panel strings (the W2-26/W2-33
/// precedent); the i18n template adoption is
/// IA-W2-27-3.
pub fn format_limit_arc(min_angle: Option<f32>, max_angle: Option<f32>) -> Option<String> {
    let spans = limit_spans(min_angle, max_angle)?;
    Some(format!(
        "Limit arc: {}° allowed, {}° blocked",
        format_whole_number(spans.allowed_degrees()),
        format_whole_number(spans.forbidden_degrees())
    ))
}

/// no limits is a silent no-op (the button is not
/// rendered then — this is the armour, not an
/// expected path).
pub fn clear_joint_limits(core: &mut EditorCore) {
    let Some(ni) = core.selected_node else {
        core.status = "Select a segment first, then clear its joint limits.".into();
        return;
    };
    let fi = core.selected_figure;
    let (is_root, any) = match core
        .project
        .figures
        .get(fi)
        .and_then(|f| f.def.nodes.get(ni))
    {
        Some(node) => (node.parent.is_none(), node.has_limits()),
        None => {
            core.status = format!(
                "Figure {} has no segment {} to limit.",
                fi + 1,
                ni
            );
            return;
        }
    };
    if is_root {
        core.status = "The origin has no joint to limit.".into();
        return;
    }
    if !any {
        return;
    }
    core.snapshot_labeled("Clear joint limits");
    if let Some(node) = core
        .project
        .figures
        .get_mut(fi)
        .and_then(|f| f.def.nodes.get_mut(ni))
    {
        node.min_angle = None;
        node.max_angle = None;
    }
    core.status = format!("Segment {} joint limits cleared.", ni);
}

/// Which anatomical preset range a preset-row press
/// applies (`apply_joint_limit_preset`'s target): the
/// humanoid constructor's own one-way ranges, held as
/// named constants on the model (`NodeDef::ELBOW_LIMITS`
/// / `NodeDef::KNEE_LIMITS`) so the panel and the
/// constructor share exactly one source for the values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LimitPreset {
    Elbow,
    Knee,
}

impl LimitPreset {
    /// The (min, max) this preset stores, degrees.
    fn range(self) -> (f32, f32) {
        match self {
            LimitPreset::Elbow => NodeDef::ELBOW_LIMITS,
            LimitPreset::Knee => NodeDef::KNEE_LIMITS,
        }
    }

    /// The preset's name in the status line.
    fn label(self) -> &'static str {
        match self {
            LimitPreset::Elbow => "elbow",
            LimitPreset::Knee => "knee",
        }
    }
}

/// Apply an anatomical preset range to the selected
/// node's joint limits (Wave Y, Y12) — the write behind
/// the preset row under the limit steppers. Before this
/// op the shipped ranges existed only inside
/// `FigureDef::humanoid()`'s constructor: a figure built
/// any other way, an imported figure, or a node whose
/// limits were cleared could re-acquire one only by
/// stepping 5° at a time (up to 34 presses per bound).
/// Same discipline as `adjust_joint_limit` /
/// `clear_joint_limits`: validate → ONE `snapshot()` →
/// set BOTH bounds → status, so one press is exactly
/// one undo step; refusals (no selection, the origin, a
/// stale node) are status-only with the limit ops' own
/// strings; and a press on a node already carrying the
/// preset's exact range is a silent no-op (the
/// Straighten rule). The stored range is the model's
/// constant verbatim — never a panel-side copy.
pub fn apply_joint_limit_preset(core: &mut EditorCore, preset: LimitPreset) {
    let Some(ni) = core.selected_node else {
        core.status = "Select a segment first, then set its joint limits.".into();
        return;
    };
    let fi = core.selected_figure;
    let (is_root, cur) = match core
        .project
        .figures
        .get(fi)
        .and_then(|f| f.def.nodes.get(ni))
    {
        Some(node) => (node.parent.is_none(), (node.min_angle, node.max_angle)),
        None => {
            core.status = format!(
                "Figure {} has no segment {} to limit.",
                fi + 1,
                ni
            );
            return;
        }
    };
    if is_root {
        // The origin refusal, verbatim from the limit
        // ops: its angle is never solved, so a preset
        // range on it could never bind anything.
        core.status = "The origin has no joint to limit.".into();
        return;
    }
    let (lo, hi) = preset.range();
    if cur == (Some(lo), Some(hi)) {
        return;
    }
    core.snapshot_labeled("Limit preset");
    // Validated above; the `if let` is unreachable
    // armour, not an expected path.
    if let Some(node) = core
        .project
        .figures
        .get_mut(fi)
        .and_then(|f| f.def.nodes.get_mut(ni))
    {
        node.min_angle = Some(lo);
        node.max_angle = Some(hi);
    }
    core.status = format!(
        "Segment {} joint limits set to the {} range ({} to {}).",
        ni,
        preset.label(),
        format_deg(lo),
        format_deg(hi)
    );
}

/// How many per-asset buttons the Media section's
/// picker row shows before the "Next sprite" cycle
/// button takes over (Wave 12). The row wraps
/// (fx_browser's idiom), so the cap is about panel
/// height, not width: past it, one button per asset
/// would crowd out the geometry controls that are this
/// panel's first job, while the cycle button still
/// reaches every asset.
pub const ASSET_BUTTON_CAP: usize = 8;

/// Readout text for a node's media slot: the single
/// formatter behind the Media section's
/// `seg-media-readout` line (and stored on
/// [`InspectorSegmentState`]), so the wording lives in
/// exactly one place. Display copies are fitted to the
/// header-name budget — an imported file name or label
/// is unbounded while the card is not; the model's
/// name/content is never touched. A stored asset index
/// past the end of the table (a stale import, a hand-
/// edited file) reads as missing rather than panicking
/// or inventing a name. Wave U (U7): each slot state
/// resolves through its own key (the `segment_media_*`
/// keys, IA-U7-1) — the conditional-key idiom the
/// camera `follow_label` uses; the `{name}` / `{content}`
/// slots take the fitted display copies above as data.
pub fn media_label(
    core: &EditorCore,
    media: &Option<SegmentMedia>,
    assets: &[ImageAsset],
) -> String {
    match media {
        None => i18n_wire::tr(core, "segment_media_none"),
        Some(SegmentMedia::Image { asset, .. }) => match assets.get(*asset) {
            Some(a) => i18n_wire::tr(core, "segment_media_sprite")
                .replace("{name}", &fit_segment_header_name(&a.name)),
            None => i18n_wire::tr(core, "segment_media_sprite_missing")
                .replace("{index}", &(*asset + 1).to_string()),
        },
        Some(SegmentMedia::Text { content, .. }) => {
            if content.is_empty() {
                i18n_wire::tr(core, "segment_media_text_empty")
            } else {
                i18n_wire::tr(core, "segment_media_text")
                    .replace("{content}", &fit_segment_header_name(content))
            }
        }
    }
}

/// The selected node's attached sprite asset index, or
/// `None` when it carries a text label, no media, or
/// the selection is stale — the defensive read the
/// "Next sprite" cycle button steps from, mirroring
/// `bend_degrees`' discipline.
pub fn attached_asset(core: &EditorCore) -> Option<usize> {
    let ni = core.selected_node?;
    let node = core
        .project
        .figures
        .get(core.selected_figure)?
        .def
        .nodes
        .get(ni)?;
    match &node.media {
        Some(SegmentMedia::Image { asset, .. }) => Some(*asset),
        _ => None,
    }
}

/// The asset the "Next sprite" cycle button attaches:
/// the one after the currently attached sprite,
/// wrapping at the end of the table; asset 0 when no
/// sprite is attached (no media, a text label, or a
/// stale stored index); `None` when the project holds
/// no assets at all (the button is not rendered then —
/// this is the armour, not an expected path).
pub fn next_asset_target(current: Option<usize>, count: usize) -> Option<usize> {
    if count == 0 {
        return None;
    }
    Some(match current {
        Some(i) if i + 1 < count => i + 1,
        _ => 0,
    })
}

/// Attach the project asset `asset` to the selected
/// node's media slot (Wave 12) — the panel-side twin of
/// the core's `attach_image_to_node`, minus the import
/// folder: that op can only attach the first PNG in
/// ./import as a NEW asset, so until this fn the user
/// had no way to attach an asset the project already
/// holds. The core has no attach-by-index op, so —
/// exactly as `rotate_selected` writes the pose angle
/// the core has no op for — this writes the definition
/// directly in the core ops' own pattern: validate →
/// ONE `snapshot()` → mutate → status, so one press is
/// exactly one undo step. Refusals (no selection, a
/// stale asset index, a stale node) are status-only,
/// no snapshot. Two deliberate details:
///
/// * Re-attaching the asset that is already attached
///   is a SILENT no-op — no write, no status, no undo
///   step — the Straighten-at-zero rule: the picker
///   button for the attached asset renders active, and
///   pressing it must not mint a phantom undo step.
/// * The slot holds ONE media value, so attaching a
///   sprite over a text label replaces it; the status
///   says so plainly (undo restores the label).
pub fn attach_asset(core: &mut EditorCore, asset: usize) {
    let Some(ni) = core.selected_node else {
        core.status = "Select a segment first, then attach a sprite.".into();
        return;
    };
    let Some(name) = core
        .project
        .assets
        .get(asset)
        .map(|a| a.name.clone())
    else {
        core.status = format!("There is no asset {} to attach.", asset + 1);
        return;
    };
    let fi = core.selected_figure;
    let (already, replaced_text) = match core
        .project
        .figures
        .get(fi)
        .and_then(|f| f.def.nodes.get(ni))
    {
        Some(node) => (
            node.media == Some(SegmentMedia::Image { asset, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false, opacity: 1.0 }),
            matches!(node.media, Some(SegmentMedia::Text { .. })),
        ),
        None => {
            core.status = format!(
                "Figure {} has no segment {} to attach to.",
                fi + 1,
                ni
            );
            return;
        }
    };
    if already {
        return;
    }
    core.snapshot_labeled("Attach asset");
    // Validated above; the `if let` is unreachable
    // armour, not an expected path.
    if let Some(node) = core
        .project
        .figures
        .get_mut(fi)
        .and_then(|f| f.def.nodes.get_mut(ni))
    {
        node.media = Some(SegmentMedia::Image { asset, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false, opacity: 1.0 });
    }
    core.status = if replaced_text {
        format!(
            "Sprite \"{}\" attached to segment {}, replacing its text label.",
            fit_segment_header_name(&name),
            ni
        )
    } else {
        format!(
            "Sprite \"{}\" attached to segment {}.",
            fit_segment_header_name(&name),
            ni
        )
    };
}

/// Which media offset axis a stepper press adjusts
/// (`adjust_media_offset`'s target).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaAxis {
    X,
    Y,
}

/// Display form of a media scale value for its
/// stepper row: up to two decimals, trailing zeros
/// trimmed ("1", "1.5", "0.25") — the stepper steps
/// in tenths, so two decimals always show the stored
/// step exactly, and a finer MCP-written value reads
/// rounded, never truncated mid-digit.
pub fn format_media_scale(scale: f32) -> String {
    let s = format!("{scale:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() {
        "0".to_string()
    } else {
        s.to_string()
    }
}

/// The media funnel's sanitisation of a stored
/// payload opacity (media.rs, Z5): a non-finite
/// value reads as the 1.0 default, a finite one
/// clamps to 0..=1. The panel reads and steps from
/// exactly the value every surface renders at, so
/// what the Opacity row shows is what the stage
/// draws.
pub fn sanitise_media_opacity(opacity: f32) -> f32 {
    if opacity.is_finite() {
        opacity.clamp(0.0, 1.0)
    } else {
        1.0
    }
}

/// Display form of a media opacity value for its
/// stepper row: the whole-percent form the figure
/// and style panels use ("100%", "55%", "0%").
pub fn format_media_opacity(opacity: f32) -> String {
    format!("{}%", (opacity * 100.0).round() as i32)
}

/// Resolve the selected node's sprite payload for
/// an edit (Wave Y, P-8; Wave AA, AA1): the
/// figure/node indices, the transform as stored and
/// the payload's stored opacity, or `None` with the
/// refusal already on the status line — no selection,
/// a stale node, or media that is not a sprite (none
/// at all, or a text label, whose editing lives in
/// the text tool). Shared by all five transform ops
/// and the opacity stepper so their refusals cannot
/// drift. Stored values are sanitised field by field
/// on the way out: a non-finite value (a hand-edited
/// file is the only source — the MCP handlers refuse
/// them) reads as its default, so the op's write
/// repairs it instead of stepping from NaN forever.
fn resolve_image_media(
    core: &mut EditorCore,
) -> Option<(usize, usize, MediaTransformState, f32)> {
    let Some(ni) = core.selected_node else {
        core.status = "Select a segment first, then adjust its sprite.".into();
        return None;
    };
    let fi = core.selected_figure;
    let node = match core
        .project
        .figures
        .get(fi)
        .and_then(|f| f.def.nodes.get(ni))
    {
        Some(node) => node,
        None => {
            core.status = format!(
                "Figure {} has no segment {} to adjust.",
                fi + 1,
                ni
            );
            return None;
        }
    };
    match &node.media {
        // Sanitised read, mirroring the composition
        // funnel's rule (Y3): non-finite components read
        // as their identity counterparts and a
        // non-positive scale as 1, so an op's write
        // repairs a corrupt stored value instead of
        // stepping from NaN forever. The opacity is
        // sanitised by the funnel's own rule (Z5):
        // `sanitise_media_opacity`.
        Some(SegmentMedia::Image {
            offset,
            scale,
            angle_offset,
            flip,
            opacity,
            ..
        }) => Some((
            fi,
            ni,
            MediaTransformState {
                offset: [
                    if offset[0].is_finite() { offset[0] } else { 0.0 },
                    if offset[1].is_finite() { offset[1] } else { 0.0 },
                ],
                scale: if scale.is_finite() && *scale > 0.0 {
                    *scale
                } else {
                    1.0
                },
                angle_offset: if angle_offset.is_finite() {
                    *angle_offset
                } else {
                    0.0
                },
                flip: *flip,
            },
            sanitise_media_opacity(*opacity),
        )),
        _ => {
            core.status =
                "The selected segment carries no sprite to adjust.".into();
            None
        }
    }
}

/// The transform ops' view of `resolve_image_media`:
/// the indices and the transform, with the payload
/// opacity projected away (the transform write-back
/// preserves it through its `..`, untouched).
fn image_media_for_edit(
    core: &mut EditorCore,
) -> Option<(usize, usize, MediaTransformState)> {
    resolve_image_media(core).map(|(fi, ni, t, _)| (fi, ni, t))
}

/// Write a transform back onto the node's sprite
/// (the armour half of the transform ops: indices
/// were validated by `image_media_for_edit`, so the
/// `if let`s are unreachable armour, not an expected
/// path).
fn store_media_transform(
    core: &mut EditorCore,
    fi: usize,
    ni: usize,
    t: MediaTransformState,
) {
    if let Some(node) = core
        .project
        .figures
        .get_mut(fi)
        .and_then(|f| f.def.nodes.get_mut(ni))
    {
        if let Some(SegmentMedia::Image {
            offset,
            scale,
            angle_offset,
            flip,
            ..
        }) = &mut node.media
        {
            *offset = t.offset;
            *scale = t.scale;
            *angle_offset = t.angle_offset;
            *flip = t.flip;
        }
    }
}

/// Step the selected sprite's offset along one axis
/// by `delta` stage units (Wave Y, P-8) — the write
/// behind the Offset X/Y stepper rows. The core has
/// no media-transform op, so — exactly as
/// `attach_asset` writes the media slot — this writes
/// the definition directly in the core ops' own
/// pattern: validate → ONE `snapshot()` → mutate →
/// status, so one press is exactly one undo step.
/// Refusals are status-only, no snapshot; a press
/// that lands on the stored value (stepping at the
/// ±`MEDIA_OFFSET_LIMIT` clamp) is a silent no-op.
pub fn adjust_media_offset(core: &mut EditorCore, axis: MediaAxis, delta: f32) {
    let Some((fi, ni, cur)) = image_media_for_edit(core) else {
        return;
    };
    let mut new = cur;
    let cell = match axis {
        MediaAxis::X => &mut new.offset[0],
        MediaAxis::Y => &mut new.offset[1],
    };
    *cell = (*cell + delta).clamp(-MEDIA_OFFSET_LIMIT, MEDIA_OFFSET_LIMIT);
    if new == cur {
        return;
    }
    core.snapshot_labeled("Sprite offset");
    store_media_transform(core, fi, ni, new);
    core.status = format!(
        "Sprite offset set to ({}, {}).",
        format_whole_number(new.offset[0]),
        format_whole_number(new.offset[1])
    );
}

/// Step the selected sprite's scale by `delta`
/// (Wave Y, P-8) — the write behind the Scale stepper
/// row, in `adjust_media_offset`'s discipline. The
/// stepped value is rounded to two decimals before
/// the clamp, so repeated presses walk the exact
/// tenths the readout shows instead of accumulating
/// float dust the display then hides.
pub fn adjust_media_scale(core: &mut EditorCore, delta: f32) {
    let Some((fi, ni, cur)) = image_media_for_edit(core) else {
        return;
    };
    let mut new = cur;
    new.scale = ((cur.scale + delta) * 100.0).round() / 100.0;
    new.scale = new.scale.clamp(MEDIA_SCALE_MIN, MEDIA_SCALE_MAX);
    if new == cur {
        return;
    }
    core.snapshot_labeled("Sprite scale");
    store_media_transform(core, fi, ni, new);
    core.status = format!("Sprite scale set to {}.", format_media_scale(new.scale));
}

/// Step the selected sprite's rotation offset by
/// `delta` degrees (Wave Y, P-8) — the write behind
/// the Rotation stepper row, in `adjust_media_offset`'s
/// discipline. The stored offset is hard-clamped to
/// ±180°, like a joint limit: past the half-circle a
/// normalised form would wrap and silently become
/// its mirror, which no press may do by accident.
pub fn adjust_media_rotate(core: &mut EditorCore, delta: f32) {
    let Some((fi, ni, cur)) = image_media_for_edit(core) else {
        return;
    };
    let mut new = cur;
    new.angle_offset = (cur.angle_offset + delta).clamp(-180.0, 180.0);
    if new == cur {
        return;
    }
    core.snapshot_labeled("Sprite rotation");
    store_media_transform(core, fi, ni, new);
    core.status = format!(
        "Sprite rotation set to {}.",
        format_deg(new.angle_offset)
    );
}

/// Flip the selected sprite's mirror flag (Wave Y,
/// P-8) — the write behind the Flip toggle, in
/// `adjust_media_offset`'s discipline. A toggle
/// always changes the value, so it always spends
/// exactly one undo step.
pub fn toggle_media_flip(core: &mut EditorCore) {
    let Some((fi, ni, cur)) = image_media_for_edit(core) else {
        return;
    };
    let mut new = cur;
    new.flip = !cur.flip;
    core.snapshot_labeled("Flip sprite");
    store_media_transform(core, fi, ni, new);
    core.status = if new.flip {
        "Sprite flip on.".into()
    } else {
        "Sprite flip off.".into()
    };
}

/// Restore the selected sprite's transform to the
/// defaults (Wave Y, P-8) — the write behind the
/// Reset button, in `adjust_media_offset`'s
/// discipline. Rendered only while the transform
/// differs from the defaults; a press at the
/// defaults (reachable only by calling the op
/// directly) is a silent no-op.
pub fn reset_media_transform(core: &mut EditorCore) {
    let Some((fi, ni, cur)) = image_media_for_edit(core) else {
        return;
    };
    if cur.is_identity() {
        return;
    }
    core.snapshot_labeled("Reset sprite transform");
    store_media_transform(core, fi, ni, MediaTransformState::IDENTITY);
    core.status = "Sprite transform reset to defaults.".into();
}

/// Write an opacity back onto the node's sprite
/// (the armour half of `adjust_media_opacity`:
/// indices were validated by `resolve_image_media`,
/// so the `if let`s are unreachable armour, not an
/// expected path). Only the opacity field moves —
/// the transform payload rides the `..` untouched,
/// the mirror of the transform write-back's own
/// preservation.
fn store_media_opacity(core: &mut EditorCore, fi: usize, ni: usize, opacity: f32) {
    if let Some(node) = core
        .project
        .figures
        .get_mut(fi)
        .and_then(|f| f.def.nodes.get_mut(ni))
    {
        if let Some(SegmentMedia::Image {
            opacity: slot, ..
        }) = &mut node.media
        {
            *slot = opacity;
        }
    }
}

/// Step the selected sprite's stored payload opacity
/// by `delta` (Wave AA, AA1) — the write behind the
/// Opacity stepper row, in `adjust_media_offset`'s
/// discipline: validate → ONE `snapshot()` → mutate →
/// status, so one press is exactly one undo step.
/// Refusals are status-only, no snapshot; a press
/// that lands on the current value (stepping at the
/// 0 / 1 clamp) is a silent no-op. The stepped value
/// is clamped, then rounded to whole percents — the
/// style panel's `opacity_stepped` order — so
/// repeated presses walk the exact percents the
/// readout shows instead of accumulating float dust.
/// The press moves only the opacity: the transform
/// payload is preserved whole, as the transform ops
/// preserve the opacity.
pub fn adjust_media_opacity(core: &mut EditorCore, delta: f32) {
    let Some((fi, ni, _, cur)) = resolve_image_media(core) else {
        return;
    };
    let new = ((cur + delta).clamp(0.0, 1.0) * 100.0).round() / 100.0;
    if new == cur {
        return;
    }
    core.snapshot_labeled("Sprite opacity");
    store_media_opacity(core, fi, ni, new);
    core.status = format!(
        "Sprite opacity set to {}.",
        format_media_opacity(new)
    );
}

/// Label for a node shape, naming the polygons `cycle_shape`
/// steps through (editor.rs:494).
pub fn shape_label(shape: &Shape) -> String {
    match shape {
        Shape::Line => "Line".to_string(),
        Shape::Circle => "Circle".to_string(),
        Shape::Polygon(3) => "Triangle".to_string(),
        Shape::Polygon(4) => "Square".to_string(),
        Shape::Polygon(5) => "Pentagon".to_string(),
        Shape::Polygon(6) => "Hexagon".to_string(),
        Shape::Polygon(n) => format!("Polygon {n}"),
    }
}

/// Compute the panel's view-state from the core, or `None`
/// when no (valid) node is selected. All indexing is
/// defensive: a stale selection left over from a figure edit
/// reads as "nothing selected" instead of panicking.
pub fn segment_state(core: &EditorCore) -> Option<InspectorSegmentState> {
    let ni = core.selected_node?;
    let fi = core.selected_figure;
    let fig = core.project.figures.get(fi)?;
    let node = fig.def.nodes.get(ni)?;
    // World angle from the solved positions at the current
    // frame (graph-aware, like the stage's `positions_f`),
    // measured with the model's own `angle_of`, so the
    // readout speaks the same convention as the pose
    // angles the stage solver writes: a segment pointing
    // straight up (the demo chest, absolute pose angle
    // 180°) reads 180°, not the 270° a screen-space
    // `atan2(dy, dx)` would give.
    let angle_deg = match node.parent {
        Some(p) => {
            let pos = core.positions_f(core.current as f32, fi);
            match (pos.get(p), pos.get(ni)) {
                (Some(a), Some(b)) => {
                    norm_deg_360(angle_of([b[0] - a[0], b[1] - a[1]]))
                }
                _ => 0.0,
            }
        }
        None => core
            .project
            .frames
            .get(core.current)
            .and_then(|f| f.poses.get(fi))
            .and_then(|pose| pose.angles.get(ni))
            .map(|a| norm_deg_360(*a))
            .unwrap_or(0.0),
    };
    // The bend read mirrors `cycle_bend` (editor.rs:1106):
    // missing table entries read as 0.
    let bend_deg = core
        .project
        .frames
        .get(core.current)
        .and_then(|f| f.bends.get(fi))
        .and_then(|b| b.get(ni))
        .copied()
        .unwrap_or(0.0)
        .to_degrees();
    // Wave AC (AC5): the clip-def fallback, resolved
    // with the funnel's own rule (media.rs, Wave 14
    // C1): the node's own media wins; only an empty
    // slot falls back to the same node index on the
    // figure's clip def, via the same `get` chains —
    // a stale clip index or an out-of-range clip node
    // simply has no fallback. The presentation is
    // derived here so the Media section shows the
    // media every surface actually renders instead of
    // "No media attached"; it stays read-only in the
    // builder because the section's presses write the
    // node's own slot (the clip def's media is edited
    // through the MCP clip doors, `clip_*_node_media`).
    let clip_media = match &node.media {
        Some(_) => None,
        None => fig.clip.and_then(|ci| {
            let clip = core.project.clips.get(ci)?;
            let clip_node = clip.def.nodes.get(ni)?;
            let media = clip_node.media.as_ref()?;
            Some(ClipMediaState {
                clip_name: fit_segment_header_name(&clip.name),
                label: media_label(
                    core,
                    &clip_node.media,
                    &core.project.assets,
                ),
                transform: match media {
                    SegmentMedia::Image {
                        offset,
                        scale,
                        angle_offset,
                        flip,
                        ..
                    } => Some(MediaTransformState {
                        offset: *offset,
                        scale: *scale,
                        angle_offset: *angle_offset,
                        flip: *flip,
                    }),
                    _ => None,
                },
                opacity: match media {
                    SegmentMedia::Image { opacity, .. } => {
                        Some(sanitise_media_opacity(*opacity))
                    }
                    _ => None,
                },
            })
        }),
    };
    Some(InspectorSegmentState {
        figure: fi,
        node: ni,
        name: node.name.clone(),
        is_root: node.parent.is_none(),
        head: node.head,
        ik_end: node.ik_end,
        bend_side: node.bend,
        length: node.length,
        thickness: node.thickness,
        angle_deg,
        bend_deg,
        min_angle: node.min_angle,
        max_angle: node.max_angle,
        color: node.color,
        shape_label: shape_label(&node.shape),
        media_label: media_label(core, &node.media, &core.project.assets),
        attached_asset: match &node.media {
            Some(SegmentMedia::Image { asset, .. }) => Some(*asset),
            _ => None,
        },
        media_transform: match &node.media {
            Some(SegmentMedia::Image {
                offset,
                scale,
                angle_offset,
                flip,
                ..
            }) => Some(MediaTransformState {
                offset: *offset,
                scale: *scale,
                angle_offset: *angle_offset,
                flip: *flip,
            }),
            _ => None,
        },
        media_opacity: match &node.media {
            Some(SegmentMedia::Image { opacity, .. }) => {
                Some(sanitise_media_opacity(*opacity))
            }
            _ => None,
        },
        clip_media,
        media: node.media.clone(),
        asset_names: core
            .project
            .assets
            .iter()
            .map(|a| a.name.clone())
            .collect(),
    })
}

/// The selected node's place in its figure's tree: its
/// own index, its parent's index (`None` for the origin)
/// and how many nodes hang directly off it. Returns
/// `None` when no (valid) node is selected — the same
/// defensive indexing as `segment_state`, so a stale
/// selection reads as "nothing selected" instead of
/// panicking.
pub fn segment_topology(core: &EditorCore) -> Option<(usize, Option<usize>, usize)> {
    let ni = core.selected_node?;
    let fig = core.project.figures.get(core.selected_figure)?;
    let node = fig.def.nodes.get(ni)?;
    let children = fig
        .def
        .nodes
        .iter()
        .filter(|n| n.parent == Some(ni))
        .count();
    Some((ni, node.parent, children))
}

/// Format a `segment_topology` triple as the panel's
/// Index line: "Node 3 · parent 1 · 1 child" for a
/// mid-tree node, "Node 0 · root · 3 children" for the
/// origin. The single formatter behind the line, so the
/// wording lives in exactly one place (and is unit
/// testable without elements). Wave U (U7): the line
/// resolves through the `segment_topology` template
/// key (IA-U7-1) and composes exactly as before — the
/// `{parent}` slot takes the parent part
/// (`segment_topology_parent`, or `segment_topology_root`
/// at the origin) and the `{children}` slot the child
/// part (`segment_topology_one_child` for exactly one,
/// `segment_topology_children` otherwise), each
/// composed from its own key first.
pub fn format_topology(
    core: &EditorCore,
    node: usize,
    parent: Option<usize>,
    children: usize,
) -> String {
    let parent_part = match parent {
        Some(p) => i18n_wire::tr(core, "segment_topology_parent")
            .replace("{parent}", &p.to_string()),
        None => i18n_wire::tr(core, "segment_topology_root"),
    };
    let child_part = if children == 1 {
        i18n_wire::tr(core, "segment_topology_one_child")
    } else {
        i18n_wire::tr(core, "segment_topology_children")
            .replace("{count}", &children.to_string())
    };
    i18n_wire::tr(core, "segment_topology")
        .replace("{node}", &node.to_string())
        .replace("{parent}", &parent_part)
        .replace("{children}", &child_part)
}

/// The parent of node `ni` in a figure definition, or
/// `None` for the origin — and for an out-of-range
/// index, so a stale selection navigates nowhere
/// instead of panicking. The pure walk behind the
/// panel's Parent joint button: the same `parent`
/// field the Index line (`segment_topology`) reads.
pub fn parent_of(nodes: &[NodeDef], ni: usize) -> Option<usize> {
    nodes.get(ni)?.parent
}

/// The first child of node `ni`: the lowest-index node
/// whose parent is `ni`, or `None` for a leaf (and for
/// an out-of-range index). "First" is definition
/// order — the order the Index line's child count
/// enumerates, and the order a reader meets the
/// children in the definition.
pub fn first_child_of(nodes: &[NodeDef], ni: usize) -> Option<usize> {
    nodes.get(ni)?;
    nodes.iter().position(|n| n.parent == Some(ni))
}

/// Move the selection to the selected node's parent
/// (the Parent joint button). Selection is not an
/// undoable edit — shell selection changes are bare
/// `selected_node` field writes — so this takes no
/// snapshot and sets no status; at the origin, or with
/// no (valid) selection, the press is a silent no-op.
pub fn select_parent(core: &mut EditorCore) {
    let target = core.selected_node.and_then(|ni| {
        let fig = core.project.figures.get(core.selected_figure)?;
        parent_of(&fig.def.nodes, ni)
    });
    if let Some(p) = target {
        core.selected_node = Some(p);
    }
}

/// Move the selection to the selected node's first
/// child (the Child joint button) — the downward twin
/// of [`select_parent`], with the same discipline: no
/// snapshot, no status, and a leaf press is a silent
/// no-op.
pub fn select_first_child(core: &mut EditorCore) {
    let target = core.selected_node.and_then(|ni| {
        let fig = core.project.figures.get(core.selected_figure)?;
        first_child_of(&fig.def.nodes, ni)
    });
    if let Some(c) = target {
        core.selected_node = Some(c);
    }
}

/// One read-only label/value row.
fn readout_row(label: &str, value: String, t: &Theme) -> El {
    row()
        .w_full()
        .center_y()
        .gap(8.0)
        .child(
            text(label)
                .font(type_::LABEL)
                .color(t.text_secondary)
                .grow(1.0),
        )
        .child(
            text(value)
                .font(type_::MONO_CAPTION.mono())
                .color(t.text),
        )
}

/// A readout row whose value is prose (the Shape name),
/// not a count: the P11 §4D prose exception keeps LABEL,
/// matching inspector_figure's `prose_readout_row`
/// (Wave 9 integration parity with B2's figure fix).
fn prose_readout_row(label: &str, value: String, t: &Theme) -> El {
    row()
        .w_full()
        .center_y()
        .gap(8.0)
        .child(
            text(label)
                .font(type_::LABEL)
                .color(t.text_secondary)
                .grow(1.0),
        )
        .child(text(value).font(type_::LABEL).color(t.text))
}

/// One stepper row in the charter's F-STEP order
/// (charter §6): the label grows, then the − button,
/// the value as the kit's centred mono readout
/// (`design::stepper_value`), and the + button (chrome
/// `btn` styling, GPUI inspector keys). The buttons
/// are the kit's `design::step_btn` (Wave Q): the
/// caller passes each direction's [`design::StepForm`]
/// computed from the same value it renders, mirroring
/// its op's clamp — `adjust_length` clamps to
/// 4..=220 and `adjust_thickness` to 1..=18
/// (editor.rs), so at those bounds the step can only
/// no-op and the button renders inert instead of
/// reporting the unchanged value as a step.
fn stepper_row(
    label: &str,
    value: String,
    dn_key: &str,
    up_key: &str,
    dn_form: design::StepForm,
    up_form: design::StepForm,
    t: &Theme,
    dn: impl Fn(&mut State) + 'static,
    up: impl Fn(&mut State) + 'static,
) -> El {
    row()
        .w_full()
        .center_y()
        .gap(8.0)
        .child(
            text(label)
                .font(type_::LABEL)
                .color(t.text_secondary)
                .grow(1.0),
        )
        .child(design::step_btn(dn_key, "−", dn_form, t, dn))
        .child(design::stepper_value(&value, t))
        .child(design::step_btn(up_key, "+", up_form, t, up))
}

/// [`stepper_row`] with the kit number field in the
/// readout's place (Roadmap Wave 1, F-3): the same
/// F-STEP order — label, −, field, + — with the caller
/// building the field (`design::number_field` on the
/// row's spec and wiring) so the row's steppers and
/// its field stay three doors onto the one value. The
/// step buttons keep the row's existing keys and
/// handlers; the field carries its own new key.
fn stepper_field_row(
    label: &str,
    field: El,
    dn_key: &str,
    up_key: &str,
    dn_form: design::StepForm,
    up_form: design::StepForm,
    t: &Theme,
    dn: impl Fn(&mut State) + 'static,
    up: impl Fn(&mut State) + 'static,
) -> El {
    row()
        .w_full()
        .center_y()
        .gap(8.0)
        .child(
            text(label)
                .font(type_::LABEL)
                .color(t.text_secondary)
                .grow(1.0),
        )
        .child(design::step_btn(dn_key, "−", dn_form, t, dn))
        .child(field)
        .child(design::step_btn(up_key, "+", up_form, t, up))
}

/// One colour swatch: the kit's single swatch
/// construction ([`design::swatch`] — the Phase N
/// adoption retired this file's hand-rolled 22×22
/// square and its white selected border for the kit's
/// 24×24 square with the accent selected ring),
/// carrying this panel's `seg-color-{rgb:06x}` key.
/// Pressing writes the colour via `set_node_color`.
fn swatch(rgb: u32, selected: bool, t: &Theme) -> El {
    let fill = hex_color(rgb).unwrap_or(Color::WHITE);
    design::swatch(
        &format!("seg-color-{rgb:06x}"),
        fill,
        selected,
        t,
        move |s: &mut State| s.core.set_node_color(rgb),
    )
}

/// The Segment inspector panel. Mount it wherever the shell
/// shows selection details; it reads only `State.core`.
pub fn inspector_segment(s: &State, t: &Theme) -> El {
    let panel = design::panel_col("inspector-segment", t);
    let Some(st) = segment_state(&s.core) else {
        // No selection: the header still renders — the
        // style panel's form (audit s08 I7) — over the
        // kit empty state, and the message names THIS
        // panel's job (geometry), so the two stacked
        // segment panels no longer show near-duplicate
        // bare lines.
        return panel
            .child(
                row()
                    .w_full()
                    .gap(8.0)
                    .center_y()
                    .child(design::panel_title(
                        Some("shapes"),
                        &i18n_wire::tr(&s.core, "segment_title"),
                        None,
                        t,
                    )),
            )
            .child(design::divider(t))
            .child(design::empty_state(
                "bezier-curve",
                &i18n_wire::tr(&s.core, "segment_select_a_segment"),
                t,
            ));
    };
    // Panel header (Phase N): the kit title cluster —
    // the `shapes` glyph leads when the icon set has it
    // (the kit checks `has_icon` itself, the Phase H
    // guard's exact behaviour), the title text is
    // byte-identical to before, and a hairline divider
    // sits under the header (charter §2). The meta name
    // is the fitted display copy (`fit_segment_header_name`);
    // the model's full name is untouched.
    let header_name = fit_segment_header_name(&st.name);
    // The header title is the panel's one i18n template
    // (`segment_title_node`, "Segment {index}" — the
    // camera `follow_label` precedent): the word
    // resolves through the table and the node index is
    // substituted here, so the English title is
    // byte-identical to the composed literal it
    // replaces.
    let header_title = i18n_wire::tr(&s.core, "segment_title_node")
        .replace("{index}", &st.node.to_string());
    let mut panel = panel
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(design::panel_title(
                    Some("shapes"),
                    &header_title,
                    Some(&header_name),
                    t,
                )),
        )
        .child(design::divider(t));
    // The Index line: one muted line under the header
    // naming the node's place in the figure tree (see
    // `segment_topology`). `segment_state` succeeded
    // above, so this read succeeds too; the `if let` is
    // belt-and-braces, not an expected path.
    if let Some((ni, parent, children)) = segment_topology(&s.core) {
        panel = panel.child(
            text(format_topology(&s.core, ni, parent, children))
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    }
    // The Bend readout line: the selected segment's
    // stored bend in whole degrees (see `bend_degrees`),
    // keyed so shell tests can find it. `segment_state`
    // succeeded above, so this read succeeds too; the
    // `if let` mirrors the Index line's idiom.
    if let Some(deg) = bend_degrees(&s.core) {
        panel = panel.child(
            text(format_bend_readout(&s.core, deg))
                .key("seg-bend-readout")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    }
    // Joint navigation (Phase K): walk the selection up
    // to the parent joint or down to the first child —
    // the Index line's topology as buttons. Always live
    // while a node is selected; at the root or a leaf
    // the press is a silent no-op (see `select_parent`).
    panel = panel.child(
        row()
            .key("seg-nav-row")
            .w_full()
            .gap(8.0)
            .center_y()
            .child(btn(
                "seg-nav-parent",
                &i18n_wire::tr(&s.core, "segment_parent_joint"),
                false,
                t,
                |s| select_parent(&mut s.core),
            ))
            .child(btn(
                "seg-nav-child",
                &i18n_wire::tr(&s.core, "segment_child_joint"),
                false,
                t,
                |s| select_first_child(&mut s.core),
            )),
    );
    // Rig properties (Roadmap Wave 2, W2-26 — RM19
    // R6): the constructor-only NodeDef properties,
    // mounted directly under the header cluster per
    // RM19 §5. Each press resolves the selection at
    // press time and calls the core setter, so a
    // press can never write a stale node; the setters
    // own the refusals (locked figure, the IK chain)
    // and the undo discipline. Labels are literals
    // this wave (IA-W2-26-2 files the i18n splice).
    panel = panel
        .child(node_name_row(s, t))
        .child(design::toggle_row("seg-node-head", "Head", st.head, t, |s| {
            if let Some(cur) = segment_state(&s.core) {
                let on = !cur.head;
                s.core.set_node_head(cur.figure, cur.node, on);
            }
        }))
        .child(design::toggle_row(
            "seg-node-ik-end",
            "IK end",
            st.ik_end,
            t,
            |s| {
                if let Some(cur) = segment_state(&s.core) {
                    let on = !cur.ik_end;
                    s.core.set_node_ik_end(cur.figure, cur.node, on);
                }
            },
        ))
        .child(
            row()
                .w_full()
                .center_y()
                .gap(8.0)
                .child(
                    text("Bend side")
                        .font(type_::LABEL)
                        .color(t.text_secondary)
                        .grow(1.0),
                )
                .child(btn("seg-bend-side-neg", "−", st.bend_side < 0.0, t, |s| {
                    if let Some(cur) = segment_state(&s.core) {
                        s.core.set_node_bend_side(cur.figure, cur.node, -1.0);
                    }
                }))
                .child(btn("seg-bend-side-pos", "+", st.bend_side > 0.0, t, |s| {
                    if let Some(cur) = segment_state(&s.core) {
                        s.core.set_node_bend_side(cur.figure, cur.node, 1.0);
                    }
                })),
        );
    panel = panel
        .child(readout_row(
            &i18n_wire::tr(&s.core, "segment_length"),
            format_whole_number(st.length),
            t,
        ))
        .child(readout_row(
            &i18n_wire::tr(&s.core, "segment_thickness"),
            format_whole_number(st.thickness),
            t,
        ))
        .child(readout_row(
            &i18n_wire::tr(&s.core, "segment_angle"),
            format_deg(st.angle_deg),
            t,
        ))
        .child(readout_row(
            &i18n_wire::tr(&s.core, "segment_bend"),
            format_deg_display(st.bend_deg),
            t,
        ))
        .child(prose_readout_row(
            &i18n_wire::tr(&s.core, "segment_shape"),
            st.shape_label.clone(),
            t,
        ));
    // The origin has no segment to lengthen (the op no-ops);
    // the GPUI inspector omitted the row there too.
    if !st.is_root {
        panel = panel.child(stepper_field_row(
            &i18n_wire::tr(&s.core, "segment_length"),
            design::number_field(
                LEN_FIELD_KEY,
                &s.segment_numbers.len_edit,
                &length_spec(),
                st.length,
                &length_spec().format(st.length),
                &i18n_wire::tr(&s.core, "number_field_help"),
                Some(&i18n_wire::tr(&s.core, "number_field_invalid")),
                t,
                design::NumWiring {
                    access: |s: &mut State| &mut s.segment_numbers.len_edit,
                    commit: |s: &mut State, v: f32| s.core.set_length(v),
                    reject: |s: &mut State| {
                        s.core.status =
                            i18n_wire::tr(&s.core, "number_field_invalid");
                    },
                    scrub_write: |s: &mut State, v: f32| s.core.set_length_quiet(v),
                    scrub_finish: |s: &mut State, v: f32| s.core.set_length(v),
                },
            ),
            "insp-len-dn",
            "insp-len-up",
            if st.length <= 4.0 {
                design::StepForm::Noop
            } else {
                design::StepForm::Live
            },
            if st.length >= 220.0 {
                design::StepForm::Noop
            } else {
                design::StepForm::Live
            },
            t,
            |s| s.core.adjust_length(-LENGTH_STEP),
            |s| s.core.adjust_length(LENGTH_STEP),
        ));
    }
    panel = panel
        .child(stepper_field_row(
            &i18n_wire::tr(&s.core, "segment_thickness"),
            design::number_field(
                TH_FIELD_KEY,
                &s.segment_numbers.th_edit,
                &thickness_spec(),
                st.thickness,
                &thickness_spec().format(st.thickness),
                &i18n_wire::tr(&s.core, "number_field_help"),
                Some(&i18n_wire::tr(&s.core, "number_field_invalid")),
                t,
                design::NumWiring {
                    access: |s: &mut State| &mut s.segment_numbers.th_edit,
                    commit: |s: &mut State, v: f32| s.core.set_thickness(v),
                    reject: |s: &mut State| {
                        s.core.status =
                            i18n_wire::tr(&s.core, "number_field_invalid");
                    },
                    scrub_write: |s: &mut State, v: f32| {
                        s.core.set_thickness_quiet(v)
                    },
                    scrub_finish: |s: &mut State, v: f32| s.core.set_thickness(v),
                },
            ),
            "insp-th-dn",
            "insp-th-up",
            if st.thickness <= 1.0 {
                design::StepForm::Noop
            } else {
                design::StepForm::Live
            },
            if st.thickness >= 18.0 {
                design::StepForm::Noop
            } else {
                design::StepForm::Live
            },
            t,
            |s| s.core.adjust_thickness(-THICKNESS_STEP),
            |s| s.core.adjust_thickness(THICKNESS_STEP),
        ))
        .child(
            row()
                .w_full()
                .center_y()
                .gap(8.0)
                .child(
                    text(i18n_wire::tr(&s.core, "segment_rotate"))
                        .font(type_::LABEL)
                        .color(t.text_secondary)
                        .grow(1.0),
                )
                .child(match stored_angle_deg(&s.core) {
                    // F-3 (RM4): the field reads the STORED
                    // pose angle — the quantity the row's
                    // buttons step (see `stored_angle_deg`);
                    // with no stored angle the op refuses,
                    // so the plain readout stays.
                    Some(stored) => design::number_field(
                        ROT_FIELD_KEY,
                        &s.segment_numbers.rot_edit,
                        &angle_spec(),
                        stored,
                        &angle_spec().format(stored),
                        &i18n_wire::tr(&s.core, "number_field_help"),
                        Some(&i18n_wire::tr(&s.core, "number_field_invalid")),
                        t,
                        design::NumWiring {
                            access: |s: &mut State| {
                                &mut s.segment_numbers.rot_edit
                            },
                            commit: |s: &mut State, v: f32| {
                                set_angle_selected(&mut s.core, v)
                            },
                            reject: |s: &mut State| {
                                s.core.status = i18n_wire::tr(
                                    &s.core,
                                    "number_field_invalid",
                                );
                            },
                            scrub_write: |s: &mut State, v: f32| {
                                let current =
                                    stored_angle_deg(&s.core).unwrap_or(v);
                                scrub_write(
                                    &mut s.segment_numbers.rot_scrub,
                                    &mut s.core,
                                    v,
                                    current,
                                    write_angle_quiet,
                                    "Set segment angle",
                                );
                            },
                            scrub_finish: move |s: &mut State, v: f32| {
                                let current =
                                    stored_angle_deg(&s.core).unwrap_or(v);
                                let status = format!(
                                    "Segment {} angle {} on frame {}.",
                                    st.node,
                                    format_deg(v),
                                    s.core.current + 1
                                );
                                scrub_finish(
                                    &mut s.segment_numbers.rot_scrub,
                                    &mut s.core,
                                    v,
                                    current,
                                    write_angle_quiet,
                                    status,
                                    "Set segment angle",
                                );
                            },
                        },
                    ),
                    None => design::stepper_value(&format_deg(st.angle_deg), t),
                })
                .child(btn("seg-rot-m5", "−5°", false, t, |s| {
                    rotate_selected(&mut s.core, -5.0)
                }))
                .child(btn("seg-rot-m1", "−1°", false, t, |s| {
                    rotate_selected(&mut s.core, -1.0)
                }))
                .child(btn("seg-rot-p1", "+1°", false, t, |s| {
                    rotate_selected(&mut s.core, 1.0)
                }))
                .child(btn("seg-rot-p5", "+5°", false, t, |s| {
                    rotate_selected(&mut s.core, 5.0)
                })),
        );
    // The Bend stepper row, mirroring Rotate — omitted
    // for the origin, which has no segment to bend (the
    // Length row precedent; `bend_selected` refuses the
    // root exactly as `cycle_bend` does). Buttons speak
    // degrees like every other bend surface in the panel;
    // `bend_selected` takes radians, the storage unit.
    // Two lines, not one (UI mission, audit s08 D1): on a
    // single line the row overflowed the card — measured
    // `seg-bend-zero` ended at x=1446.5 against the 1412
    // content edge (316 budget), past the window edge
    // too. Straighten therefore drops to its own line
    // under the stepper cluster (the figures-panel fix-5
    // precedent), leaving line 1 exactly the Rotate row's
    // composition, which fits; every key, label and
    // handler below is unchanged.
    if !st.is_root {
        panel = panel
            .child(
                row()
                    .w_full()
                    .center_y()
                    .gap(8.0)
                    .child(
                        text(i18n_wire::tr(&s.core, "segment_bend"))
                            .font(type_::LABEL)
                            .color(t.text_secondary)
                            .grow(1.0),
                    )
                    .child({
                        // F-3 (RM4): the field edits the
                        // STORED bend (radians under a
                        // degree display — `bend_spec`) —
                        // the quantity the row's buttons
                        // step. `stored_bend_rad` is None
                        // only without a valid selection,
                        // which the panel has already
                        // excluded; the fallback is the
                        // plain readout.
                        match stored_bend_rad(&s.core) {
                            Some(stored) => design::number_field(
                                BEND_FIELD_KEY,
                                &s.segment_numbers.bend_edit,
                                &bend_spec(),
                                stored,
                                &bend_spec().format(stored),
                                &i18n_wire::tr(&s.core, "number_field_help"),
                                Some(&i18n_wire::tr(
                                    &s.core,
                                    "number_field_invalid",
                                )),
                                t,
                                design::NumWiring {
                                    access: |s: &mut State| {
                                        &mut s.segment_numbers.bend_edit
                                    },
                                    commit: |s: &mut State, v: f32| {
                                        set_bend_selected(&mut s.core, v)
                                    },
                                    reject: |s: &mut State| {
                                        s.core.status = i18n_wire::tr(
                                            &s.core,
                                            "number_field_invalid",
                                        );
                                    },
                                    scrub_write: |s: &mut State, v: f32| {
                                        let current = stored_bend_rad(&s.core)
                                            .unwrap_or(v);
                                        scrub_write(
                                            &mut s.segment_numbers.bend_scrub,
                                            &mut s.core,
                                            v,
                                            current,
                                            write_bend_quiet,
                                            "Set segment bend",
                                        );
                                    },
                                    scrub_finish: move |s: &mut State,
                                                          v: f32| {
                                        let current = stored_bend_rad(&s.core)
                                            .unwrap_or(v);
                                        let status = format!(
                                            "Segment {} bend {} on frame {}.",
                                            st.node,
                                            format_deg(v.to_degrees()),
                                            s.core.current + 1
                                        );
                                        scrub_finish(
                                            &mut s.segment_numbers.bend_scrub,
                                            &mut s.core,
                                            v,
                                            current,
                                            write_bend_quiet,
                                            status,
                                            "Set segment bend",
                                        );
                                    },
                                },
                            ),
                            None => design::stepper_value(
                                &format_deg_display(st.bend_deg),
                                t,
                            ),
                        }
                    })
                    .child(btn("seg-bend-m5", "−5°", false, t, |s| {
                        bend_selected(&mut s.core, (-5.0f32).to_radians())
                    }))
                    .child(btn("seg-bend-m1", "−1°", false, t, |s| {
                        bend_selected(&mut s.core, (-1.0f32).to_radians())
                    }))
                    .child(btn("seg-bend-p1", "+1°", false, t, |s| {
                        bend_selected(&mut s.core, (1.0f32).to_radians())
                    }))
                    .child(btn("seg-bend-p5", "+5°", false, t, |s| {
                        bend_selected(&mut s.core, (5.0f32).to_radians())
                    })),
            )
            .child(
                row()
                    .w_full()
                    .center_y()
                    .gap(8.0)
                    .child(btn(
                        "seg-bend-zero",
                        &i18n_wire::tr(&s.core, "segment_straighten"),
                        false,
                        t,
                        |s| straighten_selected(&mut s.core),
                    )),
            );
    }
    // The Joint limits rows (Wave X, P-4): the node's
    // min/max relative angle, stepped in 5° increments
    // through `adjust_joint_limit`. An open side reads
    // "Off" and either step engages it one step out
    // from 0° (see the op). The StepForms mirror the
    // op's ±180° hard clamp exactly, like the Length
    // row mirrors `adjust_length`'s range: at a clamp
    // end the step can only no-op, so the button
    // renders inert. Omitted for the origin like the
    // Length and Bend rows — its angle is never solved,
    // and the op refuses it. Clear limits rides its
    // own line under the rows, rendered only while a
    // limit is set (the Straighten line's form). The
    // preset row under that (Wave Y, Y12) applies the
    // model's anatomical ranges in one press and
    // renders whenever the steppers do.
    if !st.is_root {
        panel = panel
            .child(stepper_row(
                &i18n_wire::tr(&s.core, "segment_limit_min"),
                format_limit_bound(&s.core, st.min_angle),
                "seg-lim-min-dn",
                "seg-lim-min-up",
                if st.min_angle == Some(-180.0) {
                    design::StepForm::Noop
                } else {
                    design::StepForm::Live
                },
                if st.min_angle == Some(180.0) {
                    design::StepForm::Noop
                } else {
                    design::StepForm::Live
                },
                t,
                |s| adjust_joint_limit(&mut s.core, LimitBound::Min, -LIMIT_STEP),
                |s| adjust_joint_limit(&mut s.core, LimitBound::Min, LIMIT_STEP),
            ))
            .child(stepper_row(
                &i18n_wire::tr(&s.core, "segment_limit_max"),
                format_limit_bound(&s.core, st.max_angle),
                "seg-lim-max-dn",
                "seg-lim-max-up",
                if st.max_angle == Some(-180.0) {
                    design::StepForm::Noop
                } else {
                    design::StepForm::Live
                },
                if st.max_angle == Some(180.0) {
                    design::StepForm::Noop
                } else {
                    design::StepForm::Live
                },
                t,
                |s| adjust_joint_limit(&mut s.core, LimitBound::Max, -LIMIT_STEP),
                |s| adjust_joint_limit(&mut s.core, LimitBound::Max, LIMIT_STEP),
            ));
        if st.min_angle.is_some() || st.max_angle.is_some() {
            panel = panel.child(
                row()
                    .w_full()
                    .center_y()
                    .gap(8.0)
                    .child(btn(
                        "seg-lim-clear",
                        &i18n_wire::tr(&s.core, "segment_limit_clear"),
                        false,
                        t,
                        |s| clear_joint_limits(&mut s.core),
                    )),
            );
        }
        panel = panel.child(
            row()
                .w_full()
                .center_y()
                .gap(8.0)
                .child(btn(
                    "seg-lim-preset-elbow",
                    &i18n_wire::tr(&s.core, "segment_limit_preset_elbow"),
                    false,
                    t,
                    |s| apply_joint_limit_preset(&mut s.core, LimitPreset::Elbow),
                ))
                .child(btn(
                    "seg-lim-preset-knee",
                    &i18n_wire::tr(&s.core, "segment_limit_preset_knee"),
                    false,
                    t,
                    |s| apply_joint_limit_preset(&mut s.core, LimitPreset::Knee),
                )),
        );
        // Limit arc display (Roadmap Wave 2, W2-27):
        // while the node carries a limit, a readout line
        // names its allowed/blocked sweep (the model's
        // partition, `format_limit_arc` — the bend
        // readout's muted form), and a kit toggle row
        // flips the stage overlay that draws that same
        // partition at the joint. Display only: neither
        // control writes the project — the toggle is
        // shell view state, and no set-rest clamp is
        // built (the standing IA-AA9-1 decision).
        if let Some(line) = format_limit_arc(st.min_angle, st.max_angle) {
            panel = panel.child(
                text(line)
                    .key("seg-lim-arc-readout")
                    .font(type_::LABEL)
                    .color(t.text_secondary),
            );
        }
        panel = panel.child(design::toggle_row(
            "seg-lim-arc-toggle",
            "Limit arc on stage",
            s.limit_arcs,
            t,
            |s: &mut State| toggle_limit_arcs(s),
        ));
    }
    panel = panel
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(icon_btn(
                    "cycle-bend",
                    "repeat",
                    &i18n_wire::tr(&s.core, "segment_cycle_bend"),
                    false,
                    t,
                    |s| s.core.cycle_bend(),
                ))
                .child(btn(
                    "node-shape",
                    &i18n_wire::tr(&s.core, "segment_cycle_shape"),
                    false,
                    t,
                    |s| s.core.cycle_shape(),
                )),
        )
        .child(design::section_label(
            &i18n_wire::tr(&s.core, "segment_colour"),
            t,
        ));
    let mut swatches = row().gap(6.0).center_y();
    for rgb in PALETTE {
        swatches = swatches.child(swatch(rgb, st.color == Some(rgb), t));
    }
    swatches = swatches.child(btn(
        "node-color",
        &i18n_wire::tr(&s.core, "segment_cycle_colour"),
        false,
        t,
        |s| s.core.cycle_node_color(),
    ));
    panel = panel.child(swatches);
    // The Media section (Wave 12): what the segment
    // carries, and the means to change it — see the
    // module docs. The readout is keyed like the Bend
    // readout above it; the picker row wraps
    // (fx_browser's idiom) with the attached asset's
    // button active; past the cap the cycle button
    // reaches the rest.
    panel = panel
        .child(design::section_label(
            &i18n_wire::tr(&s.core, "segment_media"),
            t,
        ))
        .child(
            text(match &st.clip_media {
                // Wave AC (AC5): while the clip
                // fallback is live, the readout names
                // the media that actually renders —
                // the provenance line below says whose
                // it is.
                Some(cm) => cm.label.clone(),
                None => st.media_label.clone(),
            })
            .key("seg-media-readout")
            .font(type_::LABEL)
            .color(t.text_secondary),
        );
    // Wave AC (AC5): the clip fallback's provenance
    // line. The picker below writes the node's OWN
    // slot (attaching over a fallback is the funnel's
    // documented override — the instance's media
    // wins); this line keeps that press from ever
    // reading as an edit of the clip's store.
    if let Some(cm) = &st.clip_media {
        panel = panel.child(
            text(
                i18n_wire::tr(&s.core, "segment_media_from_clip")
                    .replace("{name}", &cm.clip_name),
            )
            .key("seg-media-from-clip")
            .font(type_::LABEL)
            .color(t.text_secondary),
        );
    }
    if !st.asset_names.is_empty() {
        let mut picker = row().gap(6.0).w_full().flex_wrap();
        for (idx, name) in st
            .asset_names
            .iter()
            .take(ASSET_BUTTON_CAP)
            .enumerate()
        {
            let label = fit_segment_header_name(name);
            picker = picker.child(btn(
                &format!("seg-media-attach-{idx}"),
                &label,
                st.attached_asset == Some(idx),
                t,
                move |s: &mut State| attach_asset(&mut s.core, idx),
            ));
        }
        if st.asset_names.len() > ASSET_BUTTON_CAP {
            picker = picker.child(btn(
                "seg-media-cycle",
                &i18n_wire::tr(&s.core, "segment_next_sprite"),
                false,
                t,
                |s: &mut State| {
                    if let Some(a) = next_asset_target(
                        attached_asset(&s.core),
                        s.core.project.assets.len(),
                    ) {
                        attach_asset(&mut s.core, a);
                    }
                },
            ));
        }
        panel = panel.child(picker);
    }
    // The sprite transform block (Wave Y, P-8): the
    // steppers edit the attached sprite's local
    // transform — see the module docs. Rendered only
    // while the segment carries a sprite (a text
    // label's transform is the text tool's business,
    // and bare segments have nothing to transform).
    // Each row's StepForms mirror its op's clamp
    // exactly, like the limit rows above: at a clamp
    // end the step can only no-op, so the button
    // renders inert. The Opacity row (Wave AA, AA1)
    // closes the block under the same rule — it
    // edits the payload's stored opacity, not the
    // transform, and renders under the same
    // sprite-only condition (the two state fields
    // are `Some` together by construction).
    if let (Some(mt), Some(media_opacity)) = (st.media_transform, st.media_opacity) {
        panel = panel
            .child(stepper_row(
                &i18n_wire::tr(&s.core, "segment_media_offset_x"),
                format_whole_number(mt.offset[0]),
                "seg-media-offx-dn",
                "seg-media-offx-up",
                if mt.offset[0] <= -MEDIA_OFFSET_LIMIT {
                    design::StepForm::Noop
                } else {
                    design::StepForm::Live
                },
                if mt.offset[0] >= MEDIA_OFFSET_LIMIT {
                    design::StepForm::Noop
                } else {
                    design::StepForm::Live
                },
                t,
                |s| {
                    adjust_media_offset(
                        &mut s.core,
                        MediaAxis::X,
                        -MEDIA_OFFSET_STEP,
                    )
                },
                |s| {
                    adjust_media_offset(
                        &mut s.core,
                        MediaAxis::X,
                        MEDIA_OFFSET_STEP,
                    )
                },
            ))
            .child(stepper_row(
                &i18n_wire::tr(&s.core, "segment_media_offset_y"),
                format_whole_number(mt.offset[1]),
                "seg-media-offy-dn",
                "seg-media-offy-up",
                if mt.offset[1] <= -MEDIA_OFFSET_LIMIT {
                    design::StepForm::Noop
                } else {
                    design::StepForm::Live
                },
                if mt.offset[1] >= MEDIA_OFFSET_LIMIT {
                    design::StepForm::Noop
                } else {
                    design::StepForm::Live
                },
                t,
                |s| {
                    adjust_media_offset(
                        &mut s.core,
                        MediaAxis::Y,
                        -MEDIA_OFFSET_STEP,
                    )
                },
                |s| {
                    adjust_media_offset(
                        &mut s.core,
                        MediaAxis::Y,
                        MEDIA_OFFSET_STEP,
                    )
                },
            ))
            .child(stepper_row(
                &i18n_wire::tr(&s.core, "segment_media_scale"),
                format_media_scale(mt.scale),
                "seg-media-scale-dn",
                "seg-media-scale-up",
                if mt.scale <= MEDIA_SCALE_MIN {
                    design::StepForm::Noop
                } else {
                    design::StepForm::Live
                },
                if mt.scale >= MEDIA_SCALE_MAX {
                    design::StepForm::Noop
                } else {
                    design::StepForm::Live
                },
                t,
                |s| adjust_media_scale(&mut s.core, -MEDIA_SCALE_STEP),
                |s| adjust_media_scale(&mut s.core, MEDIA_SCALE_STEP),
            ))
            .child(stepper_row(
                &i18n_wire::tr(&s.core, "segment_media_rotate"),
                format_deg(mt.angle_offset),
                "seg-media-rot-dn",
                "seg-media-rot-up",
                if mt.angle_offset <= -180.0 {
                    design::StepForm::Noop
                } else {
                    design::StepForm::Live
                },
                if mt.angle_offset >= 180.0 {
                    design::StepForm::Noop
                } else {
                    design::StepForm::Live
                },
                t,
                |s| adjust_media_rotate(&mut s.core, -MEDIA_ROTATE_STEP),
                |s| adjust_media_rotate(&mut s.core, MEDIA_ROTATE_STEP),
            ))
            .child(stepper_row(
                &i18n_wire::tr(&s.core, "segment_media_opacity"),
                format_media_opacity(media_opacity),
                "seg-media-opacity-dn",
                "seg-media-opacity-up",
                if media_opacity <= 0.0 {
                    design::StepForm::Noop
                } else {
                    design::StepForm::Live
                },
                if media_opacity >= 1.0 {
                    design::StepForm::Noop
                } else {
                    design::StepForm::Live
                },
                t,
                |s| adjust_media_opacity(&mut s.core, -MEDIA_OPACITY_STEP),
                |s| adjust_media_opacity(&mut s.core, MEDIA_OPACITY_STEP),
            ));
        // Flip + Reset row: the toggle renders active
        // while flipped; Reset rides the same row and
        // renders only while the transform differs
        // from the defaults, so it can never be a dead
        // press.
        let mut flip_row = row().w_full().gap(8.0).center_y().child(btn(
            "seg-media-flip",
            &i18n_wire::tr(&s.core, "segment_media_flip"),
            mt.flip,
            t,
            |s| toggle_media_flip(&mut s.core),
        ));
        if !mt.is_identity() {
            flip_row = flip_row.child(btn(
                "seg-media-reset",
                &i18n_wire::tr(&s.core, "segment_media_reset_transform"),
                false,
                t,
                |s| reset_media_transform(&mut s.core),
            ));
        }
        panel = panel.child(flip_row);
    }
    // Wave AC (AC5): the clip fallback's values,
    // read-only — the writable block's own labels and
    // formatters as plain readout rows, with no press
    // behind any of them, so nothing here can be
    // mistaken for an edit of the clip's store. The
    // block is mutually exclusive with the writable
    // one above by construction (`clip_media` is
    // `Some` only while the node's own slot is
    // empty). The Flip row renders only while
    // flipped — the Reset rule: a state at its
    // default is not presented.
    if let Some(cm) = &st.clip_media {
        if let (Some(mt), Some(opacity)) = (cm.transform, cm.opacity) {
            panel = panel
                .child(readout_row(
                    &i18n_wire::tr(&s.core, "segment_media_offset_x"),
                    format_whole_number(mt.offset[0]),
                    t,
                ))
                .child(readout_row(
                    &i18n_wire::tr(&s.core, "segment_media_offset_y"),
                    format_whole_number(mt.offset[1]),
                    t,
                ))
                .child(readout_row(
                    &i18n_wire::tr(&s.core, "segment_media_scale"),
                    format_media_scale(mt.scale),
                    t,
                ))
                .child(readout_row(
                    &i18n_wire::tr(&s.core, "segment_media_rotate"),
                    format_deg(mt.angle_offset),
                    t,
                ))
                .child(readout_row(
                    &i18n_wire::tr(&s.core, "segment_media_opacity"),
                    format_media_opacity(opacity),
                    t,
                ));
            if mt.flip {
                panel = panel.child(prose_readout_row(
                    &i18n_wire::tr(&s.core, "segment_media_flip"),
                    i18n_wire::tr(&s.core, "segment_media_flipped"),
                    t,
                ));
            }
        }
    }
    // Import + Clear row: "Attach from import" is the
    // core's import-folder op (with an empty asset
    // table it is the only attach path); Clear renders
    // only while media is attached, so it can never be
    // a dead press.
    let mut media_row = row()
        .w_full()
        .gap(8.0)
        .center_y()
        .child(btn(
            "seg-media-import",
            &i18n_wire::tr(&s.core, "segment_attach_from_import"),
            false,
            t,
            |s| s.core.attach_image_to_node(),
        ));
    if st.media.is_some() {
        media_row = media_row.child(btn(
            "seg-media-clear",
            &i18n_wire::tr(&s.core, "segment_clear_media"),
            false,
            t,
            |s| s.core.clear_node_media(),
        ));
    }
    panel = panel.child(media_row);
    panel.child(
        row()
            .w_full()
            .gap(8.0)
            .center_y()
            .child(btn(
                "add-seg",
                &i18n_wire::tr(&s.core, "segment_add"),
                false,
                t,
                |s| s.core.add_segment(),
            ))
            .child(danger_icon_btn(
                "del-seg",
                "trash-can",
                &i18n_wire::tr(&s.core, "segment_delete"),
                false,
                t,
                |s| s.core.delete_segment(),
            )),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn core_with_selection(ni: Option<usize>) -> EditorCore {
        let mut core = EditorCore::new();
        core.selected_node = ni;
        core
    }

    #[test]
    fn segment_state_is_none_without_selection() {
        // EditorCore::new selects no node.
        assert!(segment_state(&EditorCore::new()).is_none());
        assert!(segment_state(&core_with_selection(None)).is_none());
    }

    #[test]
    fn segment_state_guards_a_stale_selection() {
        assert!(segment_state(&core_with_selection(Some(999))).is_none());
    }

    #[test]
    fn segment_state_reads_the_selected_node() {
        let core = core_with_selection(Some(1));
        let st = segment_state(&core).expect("node 1 exists in the demo figure");
        let node = &core.project.figures[core.selected_figure].def.nodes[1];
        assert_eq!(st.node, 1);
        assert_eq!(st.figure, core.selected_figure);
        assert_eq!(st.name, node.name);
        assert_eq!(st.length, node.length);
        assert_eq!(st.thickness, node.thickness);
        assert_eq!(st.color, node.color);
        assert!(!st.is_root, "node 1 hangs off the origin");
        assert!(
            (0.0..360.0).contains(&st.angle_deg),
            "angle not normalised: {}",
            st.angle_deg
        );
        // The chest points straight up from the hips:
        // its absolute pose angle is 180°, and the
        // readout speaks the model's convention, so it
        // must read 180° — a screen-space atan2(dy, dx)
        // would report 270° for the same segment.
        assert!(
            (st.angle_deg - 180.0).abs() < 0.01,
            "chest angle {}, want 180 (model convention)",
            st.angle_deg
        );
        assert_eq!(st.bend_deg, 0.0, "fresh frames store no bend");
    }

    #[test]
    fn segment_state_rereads_external_edits_on_every_call() {
        // The panel holds no cached copy of the model:
        // `segment_state` is computed from the core on
        // every call (and `inspector_segment` calls it on
        // every render), so edits made outside this panel
        // — an MCP `set_node` / `set_bend` write, an
        // import, another panel's op — are what the next
        // call reports, byte for byte.
        let mut core = core_with_selection(Some(1));
        let fi = core.selected_figure;
        let frame = core.current;
        let before = segment_state(&core).unwrap();
        {
            let node = &mut core.project.figures[fi].def.nodes[1];
            node.length = 123.0;
            node.thickness = 9.0;
        }
        let nodes = core.project.figures[fi].def.nodes.len();
        core.project.frames[frame].bends = vec![vec![0.0; nodes]];
        core.project.frames[frame].bends[fi][1] = 0.35;
        let after = segment_state(&core).unwrap();
        assert_ne!(before.length, after.length, "the first read was live");
        assert_eq!(after.length, 123.0);
        assert_eq!(after.thickness, 9.0);
        assert!(
            (after.bend_deg - 0.35f32.to_degrees()).abs() < 0.01,
            "bend readout {}, want the externally stored 0.35 rad",
            after.bend_deg
        );
    }

    #[test]
    fn stage_drag_moves_the_angle_readout_in_model_convention() {
        // A stage drag is the edit made furthest from
        // this panel: `begin_drag` / `drag_to` let the
        // model's own solver (`aim_node`) write the pose.
        // Aiming the chest at a target 30° from straight
        // down must move the panel's Angle readout to
        // 30° — the value `model::angle_of` itself
        // reports for the solved segment.
        let mut core = core_with_selection(Some(1));
        let fi = core.selected_figure;
        let frame = core.current as f32;
        let pos = core.positions_f(frame, fi);
        let (parent, child) = (pos[0], pos[1]);
        assert!(core.begin_drag(child), "the chest joint is grabbable");
        let len = core.project.figures[fi].def.nodes[1].length;
        let dir = stykanimate_rs::model::dir(30.0);
        core.drag_to([parent[0] + dir[0] * len, parent[1] + dir[1] * len]);
        core.end_drag();
        assert_eq!(core.selected_node, Some(1), "the drag keeps the joint");
        let st = segment_state(&core).unwrap();
        assert!(
            (st.angle_deg - 30.0).abs() < 0.01,
            "angle after drag {}, want 30 (model convention)",
            st.angle_deg
        );
    }

    #[test]
    fn stage_selection_and_the_panel_share_one_selection_field() {
        // There is no panel-side selection cache to fall
        // out of sync: the stage's selection-only press
        // writes `EditorCore::selected_node` (via
        // `select_figure` + the field write in
        // `secondary_select`), and this panel reads that
        // same field on its next `segment_state` call.
        let mut core = core_with_selection(Some(3));
        let fi = core.selected_figure;
        let chest = core.positions(core.current, fi)[1];
        assert!(
            crate::stage_view::secondary_select(&mut core, chest),
            "the chest joint is selectable from the stage"
        );
        assert_eq!(core.selected_node, Some(1));
        let st = segment_state(&core).unwrap();
        assert_eq!(st.node, 1, "the panel follows the stage's selection");
        assert_eq!(st.figure, fi);
    }

    #[test]
    fn segment_state_marks_the_root() {
        let core = core_with_selection(Some(0));
        let st = segment_state(&core).expect("the origin exists");
        assert!(st.is_root);
    }

    #[test]
    fn segment_state_carries_the_rig_flags() {
        // W2-26 (RM19 R6): the derived state carries
        // the three constructor-only properties the new
        // controls edit — head (node 2 is the
        // constructor's head), IK end (node 4, a hand)
        // and the bend-side sign (node 4 bends −1, the
        // elbow convention) — read live from the def.
        let core = core_with_selection(Some(2));
        let st = segment_state(&core).expect("the head node exists");
        assert!(st.head, "node 2 is the constructor's head");
        assert!(!st.ik_end);
        assert_eq!(st.bend_side, 1.0);
        let core = core_with_selection(Some(4));
        let st = segment_state(&core).expect("the hand node exists");
        assert!(!st.head);
        assert!(st.ik_end, "node 4 is a constructor IK end");
        assert_eq!(st.bend_side, -1.0, "elbows bend backwards");
    }

    #[test]
    fn sync_segment_name_field_seeds_follows_and_respects_focus() {
        // W2-26: the name session seeds from the
        // selected node, follows selection changes and
        // external renames, and is never touched while
        // focused (a sync mid-typing would eat
        // keystrokes — the figure field's discipline).
        let mut core = core_with_selection(Some(1));
        let mut edit = SegmentNameEditState::new();
        sync_segment_name_field(&core, &mut edit);
        assert_eq!(edit.name.text, "chest");
        assert_eq!(edit.for_node, Some((0, 1)));
        core.selected_node = Some(3);
        sync_segment_name_field(&core, &mut edit);
        assert_eq!(edit.name.text, "upper-arm L");
        assert_eq!(edit.for_node, Some((0, 3)));
        // An external rename (the core setter, an MCP
        // write, an undo) re-seeds the unfocused field.
        assert!(core.set_node_name(0, 3, "bicep L"));
        sync_segment_name_field(&core, &mut edit);
        assert_eq!(edit.name.text, "bicep L");
        // While focused the buffer is the user's, even
        // when the model moves under it.
        edit.name.focused = true;
        assert!(core.set_node_name(0, 3, "other"));
        sync_segment_name_field(&core, &mut edit);
        assert_eq!(edit.name.text, "bicep L", "focused field untouched");
        // No selection stands the session down.
        let bare = EditorCore::new();
        let mut edit2 = SegmentNameEditState::new();
        edit2.for_node = Some((0, 1));
        sync_segment_name_field(&bare, &mut edit2);
        assert_eq!(edit2.for_node, None);
    }

    #[test]
    fn segment_topology_reads_the_demo_tree() {
        // The demo figure is `FigureDef::humanoid`
        // (model.rs): 0 hips is the root; 1 chest, 7 thigh
        // L and 9 thigh R hang off it; 2 head, 3 upper-arm
        // L and 5 upper-arm R hang off 1.
        let core = core_with_selection(Some(1));
        assert_eq!(segment_topology(&core), Some((1, Some(0), 3)));
        let core = core_with_selection(Some(3));
        assert_eq!(segment_topology(&core), Some((3, Some(1), 1)));
    }

    #[test]
    fn segment_topology_marks_the_root_and_leaves() {
        let core = core_with_selection(Some(0));
        assert_eq!(segment_topology(&core), Some((0, None, 3)));
        // Node 2 (head) is a leaf: a parent, no children.
        let core = core_with_selection(Some(2));
        assert_eq!(segment_topology(&core), Some((2, Some(1), 0)));
    }

    #[test]
    fn segment_topology_is_none_without_a_valid_selection() {
        assert!(segment_topology(&EditorCore::new()).is_none());
        assert!(segment_topology(&core_with_selection(None)).is_none());
        assert!(segment_topology(&core_with_selection(Some(999))).is_none());
    }

    #[test]
    fn format_topology_names_root_parent_and_children() {
        // Wave U (U7): the formatter composes through
        // the `segment_topology*` template keys, so —
        // like the i18n pin below — this resolves only
        // once IA-U7-1 lands (until then `tr` falls back
        // to the key itself; the Q7 pattern).
        let core = EditorCore::new();
        assert_eq!(
            format_topology(&core, 0, None, 3),
            "Node 0 · root · 3 children"
        );
        assert_eq!(
            format_topology(&core, 3, Some(1), 1),
            "Node 3 · parent 1 · 1 child"
        );
        assert_eq!(
            format_topology(&core, 2, Some(1), 0),
            "Node 2 · parent 1 · 0 children"
        );
    }

    #[test]
    fn shape_label_names_every_cycle_step() {
        assert_eq!(shape_label(&Shape::Line), "Line");
        assert_eq!(shape_label(&Shape::Circle), "Circle");
        assert_eq!(shape_label(&Shape::Polygon(3)), "Triangle");
        assert_eq!(shape_label(&Shape::Polygon(4)), "Square");
        assert_eq!(shape_label(&Shape::Polygon(5)), "Pentagon");
        assert_eq!(shape_label(&Shape::Polygon(6)), "Hexagon");
        assert_eq!(shape_label(&Shape::Polygon(9)), "Polygon 9");
    }

    #[test]
    fn palette_mirrors_the_core_cycle_colours() {
        // editor.rs's private NODE_COLORS, minus the leading
        // None (figure colour) that set_node_color cannot
        // write. If the core palette changes, this fails and
        // the swatches get updated with it.
        assert_eq!(
            PALETTE,
            [0xFF5D5D, 0x5DB2FF, 0x7BE495, 0xFFC65D, 0xC792FF, 0x232830]
        );
        let mut sorted = PALETTE.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), PALETTE.len(), "palette colours repeat");
    }

    #[test]
    fn norm_deg_360_wraps() {
        assert_eq!(norm_deg_360(0.0), 0.0);
        assert_eq!(norm_deg_360(360.0), 0.0);
        assert_eq!(norm_deg_360(-90.0), 270.0);
        assert_eq!(norm_deg_360(725.0), 5.0);
        assert!((norm_deg_360(-0.5) - 359.5).abs() < 1e-6);
    }

    #[test]
    fn format_deg_shows_one_decimal() {
        assert_eq!(format_deg(0.0), "0.0°");
        assert_eq!(format_deg(45.5), "45.5°");
        assert_eq!(format_deg(270.0), "270.0°");
        assert_eq!(format_deg(12.34), "12.3°");
        assert_eq!(format_deg(12.36), "12.4°");
    }

    #[test]
    fn rotate_selected_steps_the_pose_angle_exactly() {
        let mut core = core_with_selection(Some(1));
        let fi = core.selected_figure;
        let frame = core.current;
        let before = core.project.frames[frame].poses[fi].angles[1];
        rotate_selected(&mut core, 5.0);
        assert_eq!(core.project.frames[frame].poses[fi].angles[1], before + 5.0);
        rotate_selected(&mut core, -1.0);
        rotate_selected(&mut core, -1.0);
        assert_eq!(core.project.frames[frame].poses[fi].angles[1], before + 3.0);
    }

    #[test]
    fn rotate_selected_is_one_undo_step_per_press() {
        let mut core = core_with_selection(Some(1));
        let fi = core.selected_figure;
        let frame = core.current;
        let before = core.project.frames[frame].poses[fi].angles[1];
        let depth = core.undo_depth();
        rotate_selected(&mut core, 1.0);
        assert_eq!(core.undo_depth(), depth + 1, "one snapshot per press");
        core.undo();
        assert_eq!(
            core.project.frames[frame].poses[fi].angles[1], before,
            "undo restores the pre-press angle"
        );
    }

    #[test]
    fn rotate_selected_without_selection_is_status_only() {
        let mut core = EditorCore::new();
        let depth = core.undo_depth();
        rotate_selected(&mut core, 5.0);
        assert_eq!(core.undo_depth(), depth, "no snapshot without a selection");
        assert!(
            core.status.contains("Select a segment"),
            "refusal explains itself: {}",
            core.status
        );
    }

    #[test]
    fn rotate_selected_moves_the_readout_by_the_same_step() {
        // The readout is the solved world direction in
        // the model's own angle convention
        // (`model::angle_of` — the convention drag posing
        // and graph keys write), so a local +5° step
        // moves the readout by exactly +5° (mod 360):
        // same direction, same magnitude, one convention
        // across the solver, the readout and the Rotate
        // row's value.
        let mut core = core_with_selection(Some(1));
        let before = segment_state(&core).unwrap().angle_deg;
        rotate_selected(&mut core, 5.0);
        let after = segment_state(&core).unwrap().angle_deg;
        let moved = norm_deg_360(after - before);
        assert!(
            (moved - 5.0).abs() < 0.01,
            "readout moved {moved}, want +5 in model convention"
        );
    }

    #[test]
    fn bend_selected_steps_the_stored_bend_exactly() {
        // The demo project's frames carry no bend table at
        // all; the first press must grow it (sparse storage)
        // and land exactly the delta, in radians.
        let mut core = core_with_selection(Some(1));
        let fi = core.selected_figure;
        let frame = core.current;
        assert!(core.project.frames[frame].bends.is_empty());
        let step = (5.0f32).to_radians();
        bend_selected(&mut core, step);
        assert_eq!(core.project.frames[frame].bends[fi][1], step);
        // Neighbours grew to zero, not to the step.
        assert_eq!(core.project.frames[frame].bends[fi][0], 0.0);
        assert_eq!(core.project.frames[frame].bends[fi][2], 0.0);
        // Steps accumulate on the stored value.
        bend_selected(&mut core, (-1.0f32).to_radians());
        bend_selected(&mut core, (-1.0f32).to_radians());
        let got = core.project.frames[frame].bends[fi][1];
        assert!(
            (got - (3.0f32).to_radians()).abs() < 1e-6,
            "bend {got}, want 3° in radians"
        );
        // And the panel's readout sees the same value.
        let st = segment_state(&core).unwrap();
        assert!((st.bend_deg - 3.0).abs() < 0.01, "readout {}", st.bend_deg);
    }

    #[test]
    fn bend_selected_is_one_undo_step_per_press() {
        let mut core = core_with_selection(Some(1));
        let fi = core.selected_figure;
        let frame = core.current;
        let depth = core.undo_depth();
        bend_selected(&mut core, (1.0f32).to_radians());
        assert_eq!(core.undo_depth(), depth + 1, "one snapshot per press");
        core.undo();
        assert!(
            core.project.bends_at(frame as f32, fi).is_empty(),
            "undo restores the pre-press (bend-free) frame"
        );
    }

    #[test]
    fn bend_selected_clamps_at_the_range_ends() {
        let mut core = core_with_selection(Some(1));
        let fi = core.selected_figure;
        let frame = core.current;
        let nodes = core.project.figures[fi].def.nodes.len();
        core.project.frames[frame].bends = vec![vec![0.0; nodes]];
        // At the limit, a further step stays at the limit.
        core.project.frames[frame].bends[fi][1] = BEND_LIMIT_RAD;
        bend_selected(&mut core, (5.0f32).to_radians());
        assert_eq!(core.project.frames[frame].bends[fi][1], BEND_LIMIT_RAD);
        core.project.frames[frame].bends[fi][1] = -BEND_LIMIT_RAD;
        bend_selected(&mut core, (-5.0f32).to_radians());
        assert_eq!(core.project.frames[frame].bends[fi][1], -BEND_LIMIT_RAD);
        // Crossing the limit lands exactly on it.
        core.project.frames[frame].bends[fi][1] = BEND_LIMIT_RAD - 0.01;
        bend_selected(&mut core, (5.0f32).to_radians());
        assert_eq!(core.project.frames[frame].bends[fi][1], BEND_LIMIT_RAD);
    }

    #[test]
    fn bend_selected_clamps_in_the_panel_not_in_the_ik_solver() {
        // The panel's bend is a RENDER arc (`Frame.bends`
        // through `model::bend_mid`), not a joint angle:
        // it never enters FK/IK at all. The two-bone
        // solver's only bend input is `NodeDef.bend`, the
        // ±1 side flag — a different field that this
        // panel never writes. So the ±0.7 rad limit is
        // applied HERE, before the write: an oversized
        // request is stored at exactly the limit, the
        // renderer's `bends_at` hands that same value to
        // `bend_mid` unclamped, and neither the solved
        // joint positions nor the IK side flag move.
        // There is no downstream clamp to hide behind.
        let mut core = core_with_selection(Some(1));
        let fi = core.selected_figure;
        let frame = core.current;
        let positions_before = core.positions_f(frame as f32, fi);
        let ik_side_before = core.project.figures[fi].def.nodes[1].bend;
        bend_selected(&mut core, 10.0);
        assert_eq!(
            core.project.frames[frame].bends[fi][1], BEND_LIMIT_RAD,
            "the panel clamps its own request before storing it"
        );
        assert_eq!(
            core.project.bends_at(frame as f32, fi)[1],
            BEND_LIMIT_RAD,
            "the renderer receives the stored value verbatim"
        );
        assert_eq!(
            core.positions_f(frame as f32, fi),
            positions_before,
            "a bend never moves a solved joint"
        );
        assert_eq!(
            core.project.figures[fi].def.nodes[1].bend, ik_side_before,
            "the IK bend-side flag is a different field, untouched"
        );
    }

    #[test]
    fn bend_selected_without_selection_is_status_only() {
        let mut core = EditorCore::new();
        let depth = core.undo_depth();
        bend_selected(&mut core, (5.0f32).to_radians());
        assert_eq!(core.undo_depth(), depth, "no snapshot without a selection");
        assert!(
            core.status.contains("Select a segment"),
            "refusal explains itself: {}",
            core.status
        );
    }

    #[test]
    fn bend_selected_refuses_the_root_snapshot_free() {
        // The origin has no parent segment to arc — the
        // same refusal cycle_bend makes (editor.rs:1135).
        let mut core = core_with_selection(Some(0));
        let depth = core.undo_depth();
        bend_selected(&mut core, (5.0f32).to_radians());
        assert_eq!(core.undo_depth(), depth, "no snapshot for the root");
        assert!(
            core.status.contains("origin has no segment"),
            "refusal explains itself: {}",
            core.status
        );
        assert!(core.project.frames[core.current].bends.is_empty());
    }

    // ---- F-3 number fields (Roadmap Wave 1, RM4) ----

    #[test]
    fn bend_spec_displays_degrees_over_stored_radians() {
        let spec = bend_spec();
        assert_eq!(spec.format((20.0f32).to_radians()), "20.0°");
        match spec.parse("20") {
            design::NumParse::Value(v) => {
                assert!((v - (20.0f32).to_radians()).abs() < 1e-6)
            }
            other => panic!("parse: {other:?}"),
        }
        assert_eq!(spec.clamp(2.0), BEND_LIMIT_RAD);
        assert_eq!(spec.clamp(-2.0), -BEND_LIMIT_RAD);
    }

    #[test]
    fn set_bend_selected_writes_exactly_and_clamps_in_one_step() {
        let mut core = core_with_selection(Some(1));
        let fi = core.selected_figure;
        let frame = core.current;
        let depth = core.undo_depth();
        set_bend_selected(&mut core, 0.5);
        assert_eq!(core.undo_depth(), depth + 1, "one commit is one undo step");
        assert_eq!(core.project.frames[frame].bends[fi][1], 0.5);
        set_bend_selected(&mut core, 2.0);
        assert_eq!(
            core.project.frames[frame].bends[fi][1],
            BEND_LIMIT_RAD,
            "the panel clamp, applied by the write"
        );
        // The stored read behind the field agrees.
        assert_eq!(stored_bend_rad(&core), Some(BEND_LIMIT_RAD));
    }

    #[test]
    fn set_bend_selected_refuses_the_root_snapshot_free() {
        let mut core = core_with_selection(Some(0));
        let depth = core.undo_depth();
        set_bend_selected(&mut core, 0.5);
        assert_eq!(core.undo_depth(), depth);
        assert!(core.status.contains("origin has no segment"));
    }

    #[test]
    fn set_angle_selected_writes_the_stored_pose_angle() {
        let mut core = core_with_selection(Some(1));
        let fi = core.selected_figure;
        let frame = core.current;
        let depth = core.undo_depth();
        set_angle_selected(&mut core, 33.0);
        assert_eq!(core.undo_depth(), depth + 1, "one commit is one undo step");
        assert_eq!(core.project.frames[frame].poses[fi].angles[1], 33.0);
        assert_eq!(stored_angle_deg(&core), Some(33.0));
        assert!(core.status.contains("angle 33.0°"), "{}", core.status);
    }

    #[test]
    fn angle_scrub_is_one_undo_step_and_cancel_restores() {
        let mut core = core_with_selection(Some(1));
        let start = stored_angle_deg(&core).unwrap();
        let mut g = ScrubGesture::default();
        let depth = core.undo_depth();
        scrub_write(&mut g, &mut core, start + 10.0, start, write_angle_quiet);
        let cur = stored_angle_deg(&core).unwrap();
        scrub_write(&mut g, &mut core, start + 20.0, cur, write_angle_quiet);
        assert_eq!(core.undo_depth(), depth + 1, "one scrub is one undo step");
        // Cancel: the press value comes back through the
        // scrub write and seals the gesture.
        let cur = stored_angle_deg(&core).unwrap();
        scrub_write(&mut g, &mut core, start, cur, write_angle_quiet);
        assert!(!g.open);
        assert_eq!(stored_angle_deg(&core), Some(start));
    }

    #[test]
    fn number_fields_construct_and_sync_reverts_blurred_sessions() {
        let mut s = crate::init();
        s.core.selected_node = Some(1);
        let t = rune::tok::theme(true);
        let _ = inspector_segment(&s, &t);
        s.segment_numbers.len_edit.begin("64".to_string());
        s.segment_numbers.bend_edit.begin("20".to_string());
        let _ = inspector_segment(&s, &t);
        s.segment_numbers.len_edit.edit.focused = false;
        s.segment_numbers.bend_edit.edit.focused = false;
        sync_number_edits(&mut s);
        assert!(!s.segment_numbers.len_edit.is_open());
        assert!(!s.segment_numbers.bend_edit.is_open());
    }

    #[test]
    fn bend_degrees_is_none_without_a_valid_selection() {
        assert!(bend_degrees(&EditorCore::new()).is_none());
        assert!(bend_degrees(&core_with_selection(None)).is_none());
        assert!(bend_degrees(&core_with_selection(Some(999))).is_none());
    }

    #[test]
    fn bend_degrees_reads_zero_for_a_fresh_selection() {
        // Sparse storage: no bend table at all reads as 0.
        let core = core_with_selection(Some(1));
        assert_eq!(bend_degrees(&core), Some(0.0));
        // The origin too — it simply can never be bent.
        let core = core_with_selection(Some(0));
        assert_eq!(bend_degrees(&core), Some(0.0));
    }

    #[test]
    fn bend_degrees_converts_the_stored_radians() {
        let mut core = core_with_selection(Some(1));
        bend_selected(&mut core, 0.35);
        assert_eq!(bend_degrees(&core), Some(0.35f32.to_degrees()));
        bend_selected(&mut core, -0.7);
        assert_eq!(bend_degrees(&core), Some((-0.35f32).to_degrees()));
        // And it agrees with the panel's existing Bend
        // readout, which reads the same stored value.
        let st = segment_state(&core).unwrap();
        assert_eq!(bend_degrees(&core), Some(st.bend_deg));
    }

    #[test]
    fn format_bend_readout_shows_whole_degrees_with_sign() {
        // Wave U (U7): IA-U7-1-dependent, like the
        // format_topology test above.
        let core = EditorCore::new();
        assert_eq!(format_bend_readout(&core, 0.0), "Bend 0°");
        assert_eq!(format_bend_readout(&core, -0.0), "Bend 0°");
        assert_eq!(format_bend_readout(&core, 20.053_523), "Bend 20°");
        assert_eq!(format_bend_readout(&core, -20.053_523), "Bend -20°");
        assert_eq!(format_bend_readout(&core, 40.107_046), "Bend 40°");
    }

    #[test]
    fn display_formatters_compact_only_extreme_values() {
        // Ordinary values are byte-identical to the
        // historical forms — the compact form exists
        // solely so an imported or MCP-written extreme
        // cannot push a readout or stepper row past the
        // 316 pt content budget.
        assert_eq!(format_whole_number(220.0), "220");
        assert_eq!(format_whole_number(5.0), "5");
        assert_eq!(format_whole_number(1e20), "1.0e20");
        assert_eq!(format_whole_number(-1e20), "-1.0e20");
        assert_eq!(format_whole_number(f32::INFINITY), "inf");
        assert_eq!(format_whole_number(f32::NAN), "NaN");
        assert_eq!(format_deg_display(40.107_046), "40.1°");
        assert_eq!(format_deg_display(1e20), "1.0e20°");
        // The readout lines compose through the Wave U
        // (U7) template key: IA-U7-1-dependent, like the
        // format_bend_readout test above.
        let core = EditorCore::new();
        assert_eq!(format_bend_readout(&core, 1e20), "Bend 1.0e20°");
        assert_eq!(format_bend_readout(&core, -1e20), "Bend -1.0e20°");
        for s in [
            format_whole_number(1e30),
            format_deg_display(-1e30),
            format_bend_readout(&core, 1e30),
        ] {
            assert!(
                s.chars().count() <= DISPLAY_NUMBER_MAX_CHARS + 5,
                "display value {s:?} is still too wide"
            );
        }
    }

    #[test]
    fn header_name_is_fitted_for_display_only() {
        // The state keeps the model's full name; only
        // the header's display copy is fitted to the
        // figure panel's 24-character budget, so a long
        // imported node name cannot push the header row
        // past the card edge.
        let long = "y".repeat(200);
        let mut core = core_with_selection(Some(1));
        core.project.figures[core.selected_figure].def.nodes[1].name = long.clone();
        assert_eq!(segment_state(&core).unwrap().name, long);
        let fitted = fit_segment_header_name(&long);
        assert_eq!(fitted.chars().count(), 24);
        assert!(fitted.ends_with('…'), "fitted name {fitted:?} is elided");
        assert_eq!(
            fit_segment_header_name("Captain Scarlet Protector Of Worlds"),
            "Captain Scarlet…",
            "a space in the second half cuts at the word boundary"
        );
        assert_eq!(fit_segment_header_name("chest"), "chest");
    }

    #[test]
    fn straighten_zeroes_a_bent_segment_in_one_undo_step() {
        let mut core = core_with_selection(Some(1));
        let fi = core.selected_figure;
        let frame = core.current;
        bend_selected(&mut core, 0.35);
        let depth = core.undo_depth();
        straighten_selected(&mut core);
        assert_eq!(
            core.project.frames[frame].bends[fi][1], 0.0,
            "the stored bend is bit-exact zero"
        );
        assert_eq!(core.undo_depth(), depth + 1, "one snapshot for the press");
        assert_eq!(bend_degrees(&core), Some(0.0));
        assert_eq!(core.status, "Segment 1 bend 0.0° on frame 1.");
        // Undo restores the bend.
        core.undo();
        assert_eq!(core.project.frames[frame].bends[fi][1], 0.35);
    }

    #[test]
    fn straighten_at_zero_is_a_silent_no_op() {
        // `bend_selected` snapshots unconditionally once
        // its guards pass, so the zero guard in
        // `straighten_selected` is what stands between a
        // press at bend 0 and a phantom undo step.
        let mut core = core_with_selection(Some(1));
        let depth = core.undo_depth();
        core.status = "Sentinel.".into();
        straighten_selected(&mut core);
        assert_eq!(core.undo_depth(), depth, "no snapshot at bend 0");
        assert_eq!(core.status, "Sentinel.", "no status churn at bend 0");
        assert!(
            core.project.frames[core.current].bends.is_empty(),
            "the bend table is not even grown"
        );
    }

    #[test]
    fn straighten_without_selection_refuses_via_bend_selected() {
        let mut core = EditorCore::new();
        let depth = core.undo_depth();
        straighten_selected(&mut core);
        assert_eq!(core.undo_depth(), depth, "no snapshot without a selection");
        assert!(
            core.status.contains("Select a segment"),
            "refusal explains itself: {}",
            core.status
        );
    }

    #[test]
    fn phase_h_glyphs_exist_in_the_icon_set() {
        // The header, Cycle bend and Delete segment glyphs
        // must name real icons — `has_icon` false would
        // silently drop the header/Delete glyphs (and the
        // menus' convention is to pin this in a test).
        for name in ["shapes", "repeat", "trash-can"] {
            assert!(
                crate::rune_sec::icons::has_icon(name),
                "glyph {name} not in the icon set"
            );
        }
    }

    #[test]
    fn stepper_row_constructs_in_every_form() {
        // Wave Q: the row builds with each StepForm
        // branch on each side, under both themes.
        for dark in [true, false] {
            let t = rune::tok::theme(dark);
            for form in [
                design::StepForm::Live,
                design::StepForm::Noop,
                design::StepForm::MutedLive,
            ] {
                let _ = stepper_row(
                    "Length",
                    "42".to_string(),
                    "dn",
                    "up",
                    form,
                    form,
                    &t,
                    |_| {},
                    |_| {},
                );
            }
        }
    }

    #[test]
    fn panel_builds_with_and_without_selection() {
        let t = rune::tok::theme(true);
        let bare = State {
            core: EditorCore::new(),
            last_poll: None,
            wheel_zoom: 0.0,
            figures_panel: crate::rune_sec::figures_panel::FiguresPanelState::default(),
            figure_add: crate::rune_sec::figure_add::FigureAddState::default(),
            library_panel: crate::rune_sec::library_panel::LibraryPanelState::default(),
            sprites_panel: crate::rune_sec::sprites_panel::SpritesPanelState::default(),
            home_panel: crate::rune_sec::home_panel::HomePanelState::default(),
            migration_panel: rune_sec::migration_prompt::initial_state(),
            frame_ops: crate::rune_sec::frame_ops::FrameOpsState::default(),
            graph_panel: crate::rune_sec::graph_panel::GraphPanelState::default(),
            audio_panel: crate::rune_sec::audio_panel::AudioPanelState::new(),
            subtitle_panel: crate::rune_sec::subtitle_panel::SubtitlePanelState::new(),
            menus: crate::rune_sec::menus::MenusState::default(),
            palette: crate::rune_sec::command_palette::PaletteState::new(),
            settings: crate::rune_sec::settings_popover::SettingsState::default(),
            toasts: crate::rune_sec::toasts::ToastsState::new(),
            text_tool: crate::rune_sec::text_tool::TextToolState::new(),
            name_edit: rune::TextEdit::new(""),
            export_use_range: false,
            fx_selected: None,
            subtitle_selected: None,
            pose_clipboard: None,
            last_status: String::new(),
            toast_now: 0.0,
            last_dark: true,
            workspace: crate::rune_sec::workspaces::Workspace::Animate,
            setup_mode_before_build: false,
            guides: false,
            show_left: true,
            show_right: true,
            stage_focus_restore: None,
            key_clipboard: None,
            key_follow_graph: false,
            key_sel: Default::default(),
            keyframe_tools: Default::default(),
            help_panel: Default::default(),
            camera_panel: crate::rune_sec::camera_panel::CameraPanelState::default(),
            segment_numbers: SegmentNumberState::default(),
            export_recap: String::new(),
            export_job: None,
            inspector_figure: crate::rune_sec::inspector_figure::InspectorFigureEditState::new(),
            clips_rename: crate::rune_sec::clips_panel::ClipRenameState::new(),
            fx_browser: crate::rune_sec::fx_browser::FxBrowserState::default(),
            draw_tools: crate::rune_sec::draw_tools::DrawToolsState::default(),
stroke_selected: None,
particle_selected: None,
guides_frustum: true,
guides_safe: true,
guides_golden: false,
guides_diagonals: false,
guides_stored: true,
rulers: false,
limit_arcs: false,
onion_ui: Default::default(),
last_frame: 0,
pose_rename: crate::rune_sec::pose_tools::PoseRenameState::new(),
pose_library: crate::rune_sec::pose_tools::PoseLibraryState::new(),
loop_playback: true,
playback_speed: 1.0,
            was_playing: false,
            confirm_action: None,
            home_rename: crate::rune_sec::home_panel::HomeRenameState::new(),
        };
        let _ = inspector_segment(&bare, &t);
        let selected = State {
            core: core_with_selection(Some(1)),
            last_poll: None,
            wheel_zoom: 0.0,
            figures_panel: crate::rune_sec::figures_panel::FiguresPanelState::default(),
            figure_add: crate::rune_sec::figure_add::FigureAddState::default(),
            library_panel: crate::rune_sec::library_panel::LibraryPanelState::default(),
            sprites_panel: crate::rune_sec::sprites_panel::SpritesPanelState::default(),
            home_panel: crate::rune_sec::home_panel::HomePanelState::default(),
            migration_panel: rune_sec::migration_prompt::initial_state(),
            frame_ops: crate::rune_sec::frame_ops::FrameOpsState::default(),
            graph_panel: crate::rune_sec::graph_panel::GraphPanelState::default(),
            audio_panel: crate::rune_sec::audio_panel::AudioPanelState::new(),
            subtitle_panel: crate::rune_sec::subtitle_panel::SubtitlePanelState::new(),
            menus: crate::rune_sec::menus::MenusState::default(),
            palette: crate::rune_sec::command_palette::PaletteState::new(),
            settings: crate::rune_sec::settings_popover::SettingsState::default(),
            toasts: crate::rune_sec::toasts::ToastsState::new(),
            text_tool: crate::rune_sec::text_tool::TextToolState::new(),
            name_edit: rune::TextEdit::new(""),
            export_use_range: false,
            fx_selected: None,
            subtitle_selected: None,
            pose_clipboard: None,
            last_status: String::new(),
            toast_now: 0.0,
            last_dark: true,
            workspace: crate::rune_sec::workspaces::Workspace::Animate,
            setup_mode_before_build: false,
            guides: false,
            show_left: true,
            show_right: true,
            stage_focus_restore: None,
            key_clipboard: None,
            key_follow_graph: false,
            key_sel: Default::default(),
            keyframe_tools: Default::default(),
            help_panel: Default::default(),
            camera_panel: crate::rune_sec::camera_panel::CameraPanelState::default(),
            segment_numbers: SegmentNumberState::default(),
            export_recap: String::new(),
            export_job: None,
            inspector_figure: crate::rune_sec::inspector_figure::InspectorFigureEditState::new(),
            clips_rename: crate::rune_sec::clips_panel::ClipRenameState::new(),
            fx_browser: crate::rune_sec::fx_browser::FxBrowserState::default(),
            draw_tools: crate::rune_sec::draw_tools::DrawToolsState::default(),
stroke_selected: None,
particle_selected: None,
guides_frustum: true,
guides_safe: true,
guides_golden: false,
guides_diagonals: false,
guides_stored: true,
rulers: false,
limit_arcs: false,
onion_ui: Default::default(),
last_frame: 0,
pose_rename: crate::rune_sec::pose_tools::PoseRenameState::new(),
pose_library: crate::rune_sec::pose_tools::PoseLibraryState::new(),
loop_playback: true,
playback_speed: 1.0,
            was_playing: false,
            confirm_action: None,
            home_rename: crate::rune_sec::home_panel::HomeRenameState::new(),
        };
        let _ = inspector_segment(&selected, &t);
        let root = State {
            core: core_with_selection(Some(0)),
            last_poll: None,
            wheel_zoom: 0.0,
            figures_panel: crate::rune_sec::figures_panel::FiguresPanelState::default(),
            figure_add: crate::rune_sec::figure_add::FigureAddState::default(),
            library_panel: crate::rune_sec::library_panel::LibraryPanelState::default(),
            sprites_panel: crate::rune_sec::sprites_panel::SpritesPanelState::default(),
            home_panel: crate::rune_sec::home_panel::HomePanelState::default(),
            migration_panel: rune_sec::migration_prompt::initial_state(),
            frame_ops: crate::rune_sec::frame_ops::FrameOpsState::default(),
            graph_panel: crate::rune_sec::graph_panel::GraphPanelState::default(),
            audio_panel: crate::rune_sec::audio_panel::AudioPanelState::new(),
            subtitle_panel: crate::rune_sec::subtitle_panel::SubtitlePanelState::new(),
            menus: crate::rune_sec::menus::MenusState::default(),
            palette: crate::rune_sec::command_palette::PaletteState::new(),
            settings: crate::rune_sec::settings_popover::SettingsState::default(),
            toasts: crate::rune_sec::toasts::ToastsState::new(),
            text_tool: crate::rune_sec::text_tool::TextToolState::new(),
            name_edit: rune::TextEdit::new(""),
            export_use_range: false,
            fx_selected: None,
            subtitle_selected: None,
            pose_clipboard: None,
            last_status: String::new(),
            toast_now: 0.0,
            last_dark: true,
            workspace: crate::rune_sec::workspaces::Workspace::Animate,
            setup_mode_before_build: false,
            guides: false,
            show_left: true,
            show_right: true,
            stage_focus_restore: None,
            key_clipboard: None,
            key_follow_graph: false,
            key_sel: Default::default(),
            keyframe_tools: Default::default(),
            help_panel: Default::default(),
            camera_panel: crate::rune_sec::camera_panel::CameraPanelState::default(),
            segment_numbers: SegmentNumberState::default(),
            export_recap: String::new(),
            export_job: None,
            inspector_figure: crate::rune_sec::inspector_figure::InspectorFigureEditState::new(),
            clips_rename: crate::rune_sec::clips_panel::ClipRenameState::new(),
            fx_browser: crate::rune_sec::fx_browser::FxBrowserState::default(),
            draw_tools: crate::rune_sec::draw_tools::DrawToolsState::default(),
stroke_selected: None,
particle_selected: None,
guides_frustum: true,
guides_safe: true,
guides_golden: false,
guides_diagonals: false,
guides_stored: true,
rulers: false,
limit_arcs: false,
onion_ui: Default::default(),
last_frame: 0,
pose_rename: crate::rune_sec::pose_tools::PoseRenameState::new(),
pose_library: crate::rune_sec::pose_tools::PoseLibraryState::new(),
loop_playback: true,
playback_speed: 1.0,
            was_playing: false,
            confirm_action: None,
            home_rename: crate::rune_sec::home_panel::HomeRenameState::new(),
        };
        let _ = inspector_segment(&root, &t);
    }

    /// A minimal node definition for the navigation
    /// fixtures: only `parent` varies (the field set
    /// mirrors the model constructors, model.rs:181).
    fn nav_node(parent: Option<usize>) -> NodeDef {
        NodeDef {
            min_angle: None,
            max_angle: None,
            name: "n".into(),
            parent,
            length: 10.0,
            rest_angle: 0.0,
            head: false,
            bend: 1.0,
            thickness: 5.0,
            color: None,
            ik_end: false,
            shape: Shape::Line,
            style: stykanimate_rs::model::NodeStyle::default(),
            media: None,
        }
    }

    #[test]
    fn parent_of_walks_a_chain_and_stops_at_the_root() {
        // A straight chain 0 <- 1 <- 2 <- 3.
        let nodes = vec![
            nav_node(None),
            nav_node(Some(0)),
            nav_node(Some(1)),
            nav_node(Some(2)),
        ];
        assert_eq!(parent_of(&nodes, 3), Some(2));
        assert_eq!(parent_of(&nodes, 2), Some(1));
        assert_eq!(parent_of(&nodes, 1), Some(0));
        assert_eq!(parent_of(&nodes, 0), None, "the root has no parent");
        assert_eq!(parent_of(&nodes, 99), None, "out of range reads as none");
        assert_eq!(first_child_of(&nodes, 0), Some(1));
        assert_eq!(first_child_of(&nodes, 2), Some(3));
        assert_eq!(first_child_of(&nodes, 3), None, "the chain's end is a leaf");
        assert_eq!(first_child_of(&nodes, 99), None);
    }

    #[test]
    fn first_child_of_picks_the_lowest_index_child() {
        // A branching fixture: the root has three
        // children (1, 2, 3); node 2 has one child (4).
        let nodes = vec![
            nav_node(None),
            nav_node(Some(0)),
            nav_node(Some(0)),
            nav_node(Some(0)),
            nav_node(Some(2)),
        ];
        assert_eq!(
            first_child_of(&nodes, 0),
            Some(1),
            "first means lowest index, not any child"
        );
        assert_eq!(first_child_of(&nodes, 2), Some(4));
        assert_eq!(first_child_of(&nodes, 1), None, "a leaf has no child");
        assert_eq!(first_child_of(&nodes, 3), None);
        assert_eq!(parent_of(&nodes, 4), Some(2));
    }

    #[test]
    fn select_parent_moves_up_and_stops_at_the_root() {
        // The demo humanoid (model.rs humanoid): node 4
        // (forearm L) hangs off 3 (upper-arm L), off 1
        // (chest), off 0 (hips, the root).
        let mut core = core_with_selection(Some(4));
        let status = core.status.clone();
        let depth = core.undo_depth();
        select_parent(&mut core);
        assert_eq!(core.selected_node, Some(3));
        select_parent(&mut core);
        assert_eq!(core.selected_node, Some(1));
        select_parent(&mut core);
        assert_eq!(core.selected_node, Some(0));
        select_parent(&mut core);
        assert_eq!(core.selected_node, Some(0), "the root press is a no-op");
        // Selection is not an undoable edit: no snapshot
        // was taken and no status was set, across all
        // four presses.
        assert_eq!(core.undo_depth(), depth);
        assert_eq!(core.status, status);
    }

    #[test]
    fn select_first_child_moves_down_and_stops_at_a_leaf() {
        // Node 0's children are 1, 7 and 9 — the first
        // is 1 (chest); node 1's children are 2, 3 and
        // 5 — the first is 2 (head), a leaf.
        let mut core = core_with_selection(Some(0));
        let status = core.status.clone();
        let depth = core.undo_depth();
        select_first_child(&mut core);
        assert_eq!(core.selected_node, Some(1));
        select_first_child(&mut core);
        assert_eq!(core.selected_node, Some(2));
        select_first_child(&mut core);
        assert_eq!(core.selected_node, Some(2), "the leaf press is a no-op");
        assert_eq!(core.undo_depth(), depth);
        assert_eq!(core.status, status);
    }

    #[test]
    fn navigation_without_a_valid_selection_changes_nothing() {
        let mut core = core_with_selection(None);
        let status = core.status.clone();
        let depth = core.undo_depth();
        select_parent(&mut core);
        select_first_child(&mut core);
        assert_eq!(core.selected_node, None);
        assert_eq!(core.undo_depth(), depth);
        assert_eq!(core.status, status);

        // A stale index navigates nowhere either (the
        // panel is not even rendered for it — this pins
        // the handler's own discipline).
        let mut stale = core_with_selection(Some(999));
        select_parent(&mut stale);
        select_first_child(&mut stale);
        assert_eq!(stale.selected_node, Some(999));
    }

    /// An asset fixture: the picker and readout read
    /// only the name, so the PNG bytes stay empty.
    fn asset(name: &str) -> ImageAsset {
        ImageAsset {
            name: name.into(),
            png: Vec::new(),
        }
    }

    fn core_with_assets(ni: Option<usize>, names: &[&str]) -> EditorCore {
        let mut core = core_with_selection(ni);
        core.project.assets = names.iter().map(|n| asset(n)).collect();
        core
    }

    #[test]
    fn media_label_names_each_slot_state() {
        // Wave U (U7): each slot state composes through
        // its own `segment_media_*` key, so this is
        // IA-U7-1-dependent, like the i18n pin below.
        let core = EditorCore::new();
        let assets = vec![asset("hero.png"), asset("cape.png")];
        assert_eq!(media_label(&core, &None, &assets), "No media attached");
        assert_eq!(
            media_label(&core, &Some(SegmentMedia::Image { asset: 1, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false, opacity: 1.0 }), &assets),
            "Sprite: cape.png"
        );
        // A stored index past the table reads as
        // missing — never a panic, never an invented
        // name.
        assert_eq!(
            media_label(&core, &Some(SegmentMedia::Image { asset: 7, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false, opacity: 1.0 }), &assets),
            "Sprite: asset 8 (missing)"
        );
        assert_eq!(
            media_label(
                &core,
                &Some(SegmentMedia::Text {
                    content: "Hello".into(),
                    size: 14.0
                }),
                &assets
            ),
            "Text: Hello"
        );
        assert_eq!(
            media_label(
                &core,
                &Some(SegmentMedia::Text {
                    content: String::new(),
                    size: 14.0
                }),
                &assets
            ),
            "Text label (empty)"
        );
    }

    #[test]
    fn next_asset_target_cycles_and_wraps() {
        assert_eq!(next_asset_target(None, 0), None);
        assert_eq!(next_asset_target(Some(0), 0), None);
        assert_eq!(next_asset_target(None, 3), Some(0), "from nothing, first");
        assert_eq!(next_asset_target(Some(0), 3), Some(1));
        assert_eq!(next_asset_target(Some(2), 3), Some(0), "wraps at the end");
        assert_eq!(
            next_asset_target(Some(9), 3),
            Some(0),
            "a stale stored index restarts at the first asset"
        );
    }

    #[test]
    fn segment_state_carries_the_media_surface() {
        // The readout strings compose through the Wave
        // U (U7) `segment_media_*` keys —
        // IA-U7-1-dependent, like the media_label test
        // above.
        // Fresh: no assets, no media.
        let core = core_with_selection(Some(1));
        let st = segment_state(&core).unwrap();
        assert_eq!(st.media, None);
        assert_eq!(st.attached_asset, None);
        assert_eq!(st.media_label, "No media attached");
        assert!(st.asset_names.is_empty());
        // Populated: names in asset order, the attached
        // index resolved, the readout naming the asset.
        let mut core = core_with_assets(Some(1), &["hero.png", "cape.png"]);
        core.project.figures[core.selected_figure].def.nodes[1].media =
            Some(SegmentMedia::Image { asset: 1, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false, opacity: 1.0 });
        let st = segment_state(&core).unwrap();
        assert_eq!(st.asset_names, ["hero.png", "cape.png"]);
        assert_eq!(st.attached_asset, Some(1));
        assert_eq!(st.media_label, "Sprite: cape.png");
    }

    #[test]
    fn segment_state_rereads_external_media_edits() {
        // The readout strings are IA-U7-1-dependent,
        // like the test above.
        // An MCP attach / clear writes the same model
        // fields; the panel holds no cache, so the next
        // `segment_state` call reports the new media.
        let mut core = core_with_assets(Some(1), &["hero.png"]);
        assert_eq!(segment_state(&core).unwrap().media, None);
        core.project.figures[core.selected_figure].def.nodes[1].media =
            Some(SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false, opacity: 1.0 });
        let st = segment_state(&core).unwrap();
        assert_eq!(st.attached_asset, Some(0));
        assert_eq!(st.media_label, "Sprite: hero.png");
        core.project.figures[core.selected_figure].def.nodes[1].media = None;
        assert_eq!(segment_state(&core).unwrap().media_label, "No media attached");
    }

    #[test]
    fn attach_asset_writes_the_chosen_asset_in_one_undo_step() {
        let mut core = core_with_assets(Some(1), &["hero.png", "cape.png"]);
        let fi = core.selected_figure;
        let depth = core.undo_depth();
        attach_asset(&mut core, 1);
        assert_eq!(
            core.project.figures[fi].def.nodes[1].media,
            Some(SegmentMedia::Image { asset: 1, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false, opacity: 1.0 }),
            "the CHOSEN asset is attached, not an import-folder pick"
        );
        assert_eq!(core.undo_depth(), depth + 1, "one snapshot per press");
        assert!(
            core.status.contains("cape.png"),
            "status names the asset: {}",
            core.status
        );
        assert_eq!(attached_asset(&core), Some(1));
        core.undo();
        assert_eq!(core.project.figures[fi].def.nodes[1].media, None);
    }

    #[test]
    fn attach_asset_reattaching_the_current_asset_is_a_silent_no_op() {
        // The attached asset's picker button renders
        // active; pressing it must not mint a phantom
        // undo step (the Straighten-at-zero rule).
        let mut core = core_with_assets(Some(1), &["hero.png"]);
        attach_asset(&mut core, 0);
        let depth = core.undo_depth();
        core.status = "Sentinel.".into();
        attach_asset(&mut core, 0);
        assert_eq!(core.undo_depth(), depth, "no snapshot for a no-change press");
        assert_eq!(core.status, "Sentinel.", "no status churn");
    }

    /// A core whose selected node (1) carries the
    /// first asset as a sprite with the identity
    /// transform — the starting point every media
    /// transform test steps from.
    fn core_with_sprite() -> EditorCore {
        let mut core = core_with_assets(Some(1), &["hero.png"]);
        attach_asset(&mut core, 0);
        core
    }

    fn media_transform_of(core: &EditorCore) -> MediaTransformState {
        match &core.project.figures[core.selected_figure].def.nodes[1].media {
            Some(SegmentMedia::Image {
                offset,
                scale,
                angle_offset,
                flip,
                ..
            }) => MediaTransformState {
                offset: *offset,
                scale: *scale,
                angle_offset: *angle_offset,
                flip: *flip,
            },
            other => panic!("expected image media, got {other:?}"),
        }
    }

    fn set_media_transform(core: &mut EditorCore, t: MediaTransformState) {
        core.project.figures[core.selected_figure].def.nodes[1].media =
            Some(SegmentMedia::Image {
                asset: 0,
                offset: t.offset,
                scale: t.scale,
                angle_offset: t.angle_offset,
                flip: t.flip,
                opacity: 1.0,
            });
    }

    fn media_opacity_of(core: &EditorCore) -> f32 {
        match &core.project.figures[core.selected_figure].def.nodes[1].media {
            Some(SegmentMedia::Image { opacity, .. }) => *opacity,
            other => panic!("expected image media, got {other:?}"),
        }
    }

    fn set_media_opacity(core: &mut EditorCore, opacity: f32) {
        match &mut core.project.figures[core.selected_figure].def.nodes[1].media {
            Some(SegmentMedia::Image { opacity: slot, .. }) => *slot = opacity,
            other => panic!("expected image media, got {other:?}"),
        }
    }

    #[test]
    fn media_offset_steps_write_in_one_undo_step_each() {
        let mut core = core_with_sprite();
        let depth = core.undo_depth();
        adjust_media_offset(&mut core, MediaAxis::X, MEDIA_OFFSET_STEP);
        assert_eq!(core.undo_depth(), depth + 1, "one snapshot per press");
        assert_eq!(media_transform_of(&core).offset, [2.0, 0.0]);
        assert!(
            core.status.contains("(2, 0)"),
            "status names the offset: {}",
            core.status
        );
        adjust_media_offset(&mut core, MediaAxis::Y, -MEDIA_OFFSET_STEP);
        assert_eq!(core.undo_depth(), depth + 2);
        assert_eq!(media_transform_of(&core).offset, [2.0, -2.0]);
        core.undo();
        assert_eq!(media_transform_of(&core).offset, [2.0, 0.0]);
    }

    #[test]
    fn media_offset_step_at_the_limit_is_a_silent_no_op() {
        let mut core = core_with_sprite();
        let mut t = MediaTransformState::IDENTITY;
        t.offset = [MEDIA_OFFSET_LIMIT, -MEDIA_OFFSET_LIMIT];
        set_media_transform(&mut core, t);
        let depth = core.undo_depth();
        core.status = "Sentinel.".into();
        adjust_media_offset(&mut core, MediaAxis::X, MEDIA_OFFSET_STEP);
        adjust_media_offset(&mut core, MediaAxis::Y, -MEDIA_OFFSET_STEP);
        assert_eq!(core.undo_depth(), depth, "no snapshot at the clamp");
        assert_eq!(core.status, "Sentinel.", "no status churn");
    }

    #[test]
    fn media_scale_steps_clamp_at_both_bounds() {
        let mut core = core_with_sprite();
        adjust_media_scale(&mut core, MEDIA_SCALE_STEP);
        assert_eq!(media_transform_of(&core).scale, 1.1);
        // At the floor a downward step is silent.
        let mut t = MediaTransformState::IDENTITY;
        t.scale = MEDIA_SCALE_MIN;
        set_media_transform(&mut core, t);
        let depth = core.undo_depth();
        core.status = "Sentinel.".into();
        adjust_media_scale(&mut core, -MEDIA_SCALE_STEP);
        assert_eq!(core.undo_depth(), depth);
        assert_eq!(core.status, "Sentinel.");
        // At the ceiling an upward step is silent.
        t.scale = MEDIA_SCALE_MAX;
        set_media_transform(&mut core, t);
        adjust_media_scale(&mut core, MEDIA_SCALE_STEP);
        assert_eq!(core.undo_depth(), depth);
    }

    #[test]
    fn media_rotate_steps_clamp_at_180() {
        let mut core = core_with_sprite();
        adjust_media_rotate(&mut core, MEDIA_ROTATE_STEP);
        assert_eq!(media_transform_of(&core).angle_offset, 5.0);
        let mut t = MediaTransformState::IDENTITY;
        t.angle_offset = 180.0;
        set_media_transform(&mut core, t);
        let depth = core.undo_depth();
        adjust_media_rotate(&mut core, MEDIA_ROTATE_STEP);
        assert_eq!(core.undo_depth(), depth, "no step past the half-circle");
        t.angle_offset = -180.0;
        set_media_transform(&mut core, t);
        adjust_media_rotate(&mut core, -MEDIA_ROTATE_STEP);
        assert_eq!(core.undo_depth(), depth);
    }

    #[test]
    fn media_flip_toggles_in_one_step_each() {
        let mut core = core_with_sprite();
        let depth = core.undo_depth();
        toggle_media_flip(&mut core);
        assert!(media_transform_of(&core).flip);
        assert_eq!(core.status, "Sprite flip on.");
        toggle_media_flip(&mut core);
        assert!(!media_transform_of(&core).flip);
        assert_eq!(core.status, "Sprite flip off.");
        assert_eq!(core.undo_depth(), depth + 2);
    }

    #[test]
    fn media_reset_restores_the_identity_in_one_step() {
        let mut core = core_with_sprite();
        set_media_transform(
            &mut core,
            MediaTransformState {
                offset: [10.0, -4.0],
                scale: 2.5,
                angle_offset: 45.0,
                flip: true,
            },
        );
        let depth = core.undo_depth();
        reset_media_transform(&mut core);
        assert_eq!(core.undo_depth(), depth + 1);
        assert!(media_transform_of(&core).is_identity());
        // A second press (the button no longer renders,
        // but the op stays honest) is a silent no-op.
        core.status = "Sentinel.".into();
        reset_media_transform(&mut core);
        assert_eq!(core.undo_depth(), depth + 1);
        assert_eq!(core.status, "Sentinel.");
    }

    #[test]
    fn media_transform_refusals_are_status_only() {
        // No media on the selected node.
        let mut core = core_with_assets(Some(1), &["hero.png"]);
        let depth = core.undo_depth();
        adjust_media_offset(&mut core, MediaAxis::X, MEDIA_OFFSET_STEP);
        assert_eq!(core.undo_depth(), depth);
        assert!(
            core.status.contains("carries no sprite"),
            "refusal explains itself: {}",
            core.status
        );
        // A text label is not a sprite.
        core.project.figures[core.selected_figure].def.nodes[1].media =
            Some(SegmentMedia::Text {
                content: "HEY".into(),
                size: 24.0,
            });
        toggle_media_flip(&mut core);
        assert_eq!(core.undo_depth(), depth);
        assert!(core.status.contains("carries no sprite"));
        // No selection.
        let mut core = core_with_assets(None, &["hero.png"]);
        let depth = core.undo_depth();
        adjust_media_scale(&mut core, MEDIA_SCALE_STEP);
        assert_eq!(core.undo_depth(), depth);
        assert!(core.status.contains("Select a segment"));
    }

    #[test]
    fn segment_state_carries_the_media_transform() {
        let mut core = core_with_sprite();
        let st = segment_state(&core).expect("node 1 is selected");
        assert_eq!(
            st.media_transform,
            Some(MediaTransformState::IDENTITY),
            "a fresh attach reads as the identity transform"
        );
        let t = MediaTransformState {
            offset: [6.0, 2.0],
            scale: 0.5,
            angle_offset: -30.0,
            flip: true,
        };
        set_media_transform(&mut core, t);
        let st = segment_state(&core).expect("node 1 is selected");
        assert_eq!(st.media_transform, Some(t));
        // A text label carries no transform state.
        core.project.figures[core.selected_figure].def.nodes[1].media =
            Some(SegmentMedia::Text {
                content: "HEY".into(),
                size: 24.0,
            });
        let st = segment_state(&core).expect("node 1 is selected");
        assert_eq!(st.media_transform, None);
    }

    #[test]
    fn media_opacity_steps_write_in_one_undo_step_each() {
        let mut core = core_with_sprite();
        let depth = core.undo_depth();
        adjust_media_opacity(&mut core, -MEDIA_OPACITY_STEP);
        assert_eq!(core.undo_depth(), depth + 1, "one snapshot per press");
        assert_eq!(media_opacity_of(&core), 0.95);
        assert_eq!(core.status, "Sprite opacity set to 95%.");
        adjust_media_opacity(&mut core, -MEDIA_OPACITY_STEP);
        assert_eq!(core.undo_depth(), depth + 2);
        assert_eq!(media_opacity_of(&core), 0.90);
        core.undo();
        assert_eq!(media_opacity_of(&core), 0.95, "undo restores the step");
    }

    #[test]
    fn media_opacity_steps_clamp_at_both_bounds() {
        let mut core = core_with_sprite();
        // Fully opaque: an upward press is silent.
        let depth = core.undo_depth();
        core.status = "Sentinel.".into();
        adjust_media_opacity(&mut core, MEDIA_OPACITY_STEP);
        assert_eq!(core.undo_depth(), depth, "no snapshot at the ceiling");
        assert_eq!(core.status, "Sentinel.", "no status churn");
        // A partial step onto the ceiling still lands.
        set_media_opacity(&mut core, 0.98);
        adjust_media_opacity(&mut core, MEDIA_OPACITY_STEP);
        assert_eq!(media_opacity_of(&core), 1.0);
        assert_eq!(core.undo_depth(), depth + 1);
        // Fully transparent: a downward press is silent.
        set_media_opacity(&mut core, 0.0);
        let depth = core.undo_depth();
        core.status = "Sentinel.".into();
        adjust_media_opacity(&mut core, -MEDIA_OPACITY_STEP);
        assert_eq!(core.undo_depth(), depth, "no snapshot at the floor");
        assert_eq!(core.status, "Sentinel.");
    }

    #[test]
    fn media_opacity_step_rounds_to_whole_percents() {
        // An MCP-written off-grid value steps onto the
        // percent grid the readout shows, like the
        // style panel's `opacity_stepped`.
        let mut core = core_with_sprite();
        set_media_opacity(&mut core, 0.333);
        adjust_media_opacity(&mut core, MEDIA_OPACITY_STEP);
        assert_eq!(media_opacity_of(&core), 0.38);
        adjust_media_opacity(&mut core, -MEDIA_OPACITY_STEP);
        assert_eq!(media_opacity_of(&core), 0.33);
    }

    #[test]
    fn media_opacity_step_repairs_a_non_finite_store() {
        // The sanitised read is 1.0, so a downward
        // press writes a real 0.95 over the corrupt
        // value instead of stepping from NaN forever.
        let mut core = core_with_sprite();
        set_media_opacity(&mut core, f32::NAN);
        let depth = core.undo_depth();
        adjust_media_opacity(&mut core, -MEDIA_OPACITY_STEP);
        assert_eq!(core.undo_depth(), depth + 1);
        assert_eq!(media_opacity_of(&core), 0.95);
        // An upward press at the sanitised ceiling
        // changes nothing and stays silent.
        set_media_opacity(&mut core, f32::INFINITY);
        let depth = core.undo_depth();
        core.status = "Sentinel.".into();
        adjust_media_opacity(&mut core, MEDIA_OPACITY_STEP);
        assert_eq!(core.undo_depth(), depth);
        assert_eq!(core.status, "Sentinel.");
    }

    #[test]
    fn media_opacity_and_transform_writes_preserve_each_other() {
        // Z5's guarantee, both directions: an opacity
        // press leaves the transform payload whole,
        // and transform presses — Reset included —
        // leave the stored opacity whole.
        let mut core = core_with_sprite();
        let t = MediaTransformState {
            offset: [6.0, 2.0],
            scale: 0.5,
            angle_offset: -30.0,
            flip: true,
        };
        set_media_transform(&mut core, t);
        set_media_opacity(&mut core, 0.5);
        adjust_media_opacity(&mut core, -MEDIA_OPACITY_STEP);
        assert_eq!(media_opacity_of(&core), 0.45);
        assert_eq!(media_transform_of(&core), t, "transform untouched");
        adjust_media_scale(&mut core, MEDIA_SCALE_STEP);
        assert_eq!(media_opacity_of(&core), 0.45, "opacity untouched");
        reset_media_transform(&mut core);
        assert!(media_transform_of(&core).is_identity());
        assert_eq!(media_opacity_of(&core), 0.45, "Reset spares the opacity");
    }

    #[test]
    fn media_opacity_refusals_are_status_only() {
        // No media on the selected node.
        let mut core = core_with_assets(Some(1), &["hero.png"]);
        let depth = core.undo_depth();
        adjust_media_opacity(&mut core, -MEDIA_OPACITY_STEP);
        assert_eq!(core.undo_depth(), depth);
        assert!(
            core.status.contains("carries no sprite"),
            "refusal explains itself: {}",
            core.status
        );
        // A text label is not a sprite.
        core.project.figures[core.selected_figure].def.nodes[1].media =
            Some(SegmentMedia::Text {
                content: "HEY".into(),
                size: 24.0,
            });
        adjust_media_opacity(&mut core, MEDIA_OPACITY_STEP);
        assert_eq!(core.undo_depth(), depth);
        assert!(core.status.contains("carries no sprite"));
        // No selection.
        let mut core = core_with_assets(None, &["hero.png"]);
        let depth = core.undo_depth();
        adjust_media_opacity(&mut core, -MEDIA_OPACITY_STEP);
        assert_eq!(core.undo_depth(), depth);
        assert!(core.status.contains("Select a segment"));
    }

    #[test]
    fn segment_state_carries_the_media_opacity_sanitised() {
        let mut core = core_with_sprite();
        let st = segment_state(&core).expect("node 1 is selected");
        assert_eq!(st.media_opacity, Some(1.0), "a fresh attach is opaque");
        // The state shows the funnel's value, not the
        // raw store: out-of-range and non-finite
        // values read as what renders.
        set_media_opacity(&mut core, 0.5);
        assert_eq!(segment_state(&core).unwrap().media_opacity, Some(0.5));
        set_media_opacity(&mut core, 1.7);
        assert_eq!(segment_state(&core).unwrap().media_opacity, Some(1.0));
        set_media_opacity(&mut core, -0.25);
        assert_eq!(segment_state(&core).unwrap().media_opacity, Some(0.0));
        set_media_opacity(&mut core, f32::NAN);
        assert_eq!(segment_state(&core).unwrap().media_opacity, Some(1.0));
        // A text label carries no opacity state.
        core.project.figures[core.selected_figure].def.nodes[1].media =
            Some(SegmentMedia::Text {
                content: "HEY".into(),
                size: 24.0,
            });
        let st = segment_state(&core).expect("node 1 is selected");
        assert_eq!(st.media_opacity, None);
        // Neither does a bare segment.
        let core = core_with_assets(Some(1), &["hero.png"]);
        let st = segment_state(&core).expect("node 1 is selected");
        assert_eq!(st.media_opacity, None);
    }

    #[test]
    fn format_media_opacity_reads_whole_percents() {
        assert_eq!(format_media_opacity(1.0), "100%");
        assert_eq!(format_media_opacity(0.55), "55%");
        assert_eq!(format_media_opacity(0.05), "5%");
        assert_eq!(format_media_opacity(0.0), "0%");
    }

    #[test]
    fn sanitise_media_opacity_matches_the_funnel_rule() {
        assert_eq!(sanitise_media_opacity(0.5), 0.5);
        assert_eq!(sanitise_media_opacity(1.7), 1.0);
        assert_eq!(sanitise_media_opacity(-0.25), 0.0);
        assert_eq!(sanitise_media_opacity(f32::NAN), 1.0);
        assert_eq!(sanitise_media_opacity(f32::INFINITY), 1.0);
        assert_eq!(sanitise_media_opacity(f32::NEG_INFINITY), 1.0);
    }

    /// A real PNG for the funnel pins below (Wave AB,
    /// AB10): `media_items` asks the asset for its
    /// dimensions, so the bytes must really decode —
    /// the scenes.rs `solid_png` pattern. (The `asset`
    /// fixture's empty bytes exercise only the funnel's
    /// skip path.)
    fn funnel_png() -> Vec<u8> {
        let img =
            image::RgbaImage::from_pixel(4, 2, image::Rgba([200, 40, 40, 255]));
        let mut buf = Vec::new();
        let _ = image::DynamicImage::ImageRgba8(img).write_to(
            &mut std::io::Cursor::new(&mut buf),
            image::ImageFormat::Png,
        );
        buf
    }

    /// The alpha of the first image item the funnel
    /// emits for `core`'s project — the selected node's
    /// sprite (segment media precedes instances in the
    /// funnel's order, and the demo project carries no
    /// other segment media).
    fn funnel_alpha(core: &EditorCore) -> f32 {
        stykanimate_rs::media::media_items(&core.project, 0.0)
            .iter()
            .find_map(|i| match i {
                stykanimate_rs::media::MediaItem::Image { alpha, .. } => {
                    Some(*alpha)
                }
                _ => None,
            })
            .expect("an image item")
    }

    #[test]
    fn panel_opacity_readout_is_the_funnels_media_factor() {
        // Wave AB (AB10): the cross-surface pin AA1's
        // panel pins and Z5's funnel pins each stop
        // short of. On ONE project, the value the panel
        // state carries (and the row formats) is the
        // media factor of the funnel's composed alpha —
        // figure × carrier style × media — so the
        // readout cannot drift from what renders.
        let mut core = core_with_sprite();
        core.project.assets[0].png = funnel_png();
        let fi = core.selected_figure;
        core.project.figures[fi].opacity = 0.5;
        core.project.figures[fi].def.nodes[1].style.opacity = 0.5;
        set_media_opacity(&mut core, 0.5);
        let st = segment_state(&core).expect("node 1 is selected");
        assert_eq!(st.media_opacity, Some(0.5));
        assert_eq!(
            format_media_opacity(st.media_opacity.expect("an opacity")),
            "50%"
        );
        assert_eq!(funnel_alpha(&core), 0.125, "figure × style × media");
        // A second factor value: readout and funnel
        // move together.
        set_media_opacity(&mut core, 0.25);
        let st = segment_state(&core).expect("node 1 is selected");
        assert_eq!(
            format_media_opacity(st.media_opacity.expect("an opacity")),
            "25%"
        );
        assert_eq!(funnel_alpha(&core), 0.0625);
    }

    #[test]
    fn corrupt_store_reads_and_repairs_identically_on_panel_and_funnel() {
        // Wave AB (AB10): a corrupt stored opacity
        // (a hand-edited file — the only source) shows
        // on the panel exactly what the funnel renders,
        // and one downward press through the panel's
        // own op repairs the store so state, readout
        // and funnel agree on a real value afterwards.
        let mut core = core_with_sprite();
        core.project.assets[0].png = funnel_png();
        set_media_opacity(&mut core, f32::NAN);
        let st = segment_state(&core).expect("node 1 is selected");
        assert_eq!(st.media_opacity, Some(1.0));
        assert_eq!(
            format_media_opacity(st.media_opacity.expect("an opacity")),
            "100%"
        );
        assert_eq!(funnel_alpha(&core), 1.0, "the funnel reads NaN as 1 too");
        set_media_opacity(&mut core, 1.7);
        assert_eq!(segment_state(&core).unwrap().media_opacity, Some(1.0));
        assert_eq!(funnel_alpha(&core), 1.0, "a stored 1.7 clamps on both");
        // The repair: the panel's downward press turns
        // the NaN store into a real 0.95 everywhere.
        set_media_opacity(&mut core, f32::NAN);
        adjust_media_opacity(&mut core, -MEDIA_OPACITY_STEP);
        assert_eq!(media_opacity_of(&core), 0.95);
        assert_eq!(segment_state(&core).unwrap().media_opacity, Some(0.95));
        assert_eq!(funnel_alpha(&core), 0.95);
    }

    #[test]
    fn reset_stays_hidden_and_harmless_for_an_opacity_only_difference() {
        // Wave AB (AB10): the panel-flow half of AA1's
        // Reset guarantee. The Reset button renders
        // only while the state's transform is
        // non-identity, and the state derives the
        // transform WITHOUT the opacity — so a sprite
        // differing only in opacity never offers a
        // Reset that could destroy the value; when
        // Reset does fire (a real transform
        // difference), the stored opacity survives and
        // the row keeps reading it.
        let mut core = core_with_sprite();
        set_media_opacity(&mut core, 0.45);
        let st = segment_state(&core).expect("node 1 is selected");
        assert!(
            st.media_transform
                .expect("a transform state")
                .is_identity(),
            "opacity is not part of the transform identity"
        );
        assert_eq!(st.media_opacity, Some(0.45));
        adjust_media_scale(&mut core, MEDIA_SCALE_STEP);
        reset_media_transform(&mut core);
        let st = segment_state(&core).expect("node 1 is selected");
        assert!(
            st.media_transform
                .expect("a transform state")
                .is_identity()
        );
        assert_eq!(st.media_opacity, Some(0.45), "Reset spared the opacity");
        assert_eq!(media_opacity_of(&core), 0.45);
    }

    #[test]
    fn media_transform_labels_resolve_through_the_catalogue() {
        // IA-Y4-1 (the Q7 note): these six keys enter
        // the catalogue through the integrator; until
        // the insertion lands `tr` returns the key
        // itself and this pin fails BY DESIGN — it
        // passes on insertion, proving the panel's
        // labels are the catalogue's English strings,
        // byte-identical. Wave AA (AA1) extends the
        // pin with the Opacity row's key: the seventh
        // pair lands via IA-AA1-1, so the pin as a
        // whole passes only once that IA lands too.
        let core = EditorCore::new();
        for (key, english) in [
            ("segment_media_offset_x", "Sprite offset X"),
            ("segment_media_offset_y", "Sprite offset Y"),
            ("segment_media_scale", "Sprite scale"),
            ("segment_media_rotate", "Sprite rotation"),
            ("segment_media_opacity", "Sprite opacity"),
            ("segment_media_flip", "Flip sprite"),
            ("segment_media_reset_transform", "Reset transform"),
        ] {
            assert_eq!(i18n_wire::tr(&core, key), english, "key {key}");
        }
    }

    /// A core whose selected figure is a movieclip
    /// instance (Wave AC, AC5): node 1's own media
    /// slot is empty and the clip def's node 1
    /// carries the first asset as a sprite with a
    /// non-identity transform — the fallback case.
    /// The clip's single frame is the demo pose, so
    /// the funnel evaluates the demo geometry (the
    /// media.rs `clip_project` pattern).
    fn core_with_clip_fallback() -> EditorCore {
        let mut core = core_with_assets(Some(1), &["hero.png"]);
        let fi = core.selected_figure;
        let mut def = core.project.figures[fi].def.clone();
        def.nodes[1].media = Some(SegmentMedia::Image {
            asset: 0,
            offset: [4.0, -2.0],
            scale: 1.5,
            angle_offset: 10.0,
            flip: true,
            opacity: 0.5,
        });
        let pose = core.project.frames[0].poses[fi].clone();
        core.project.clips.push(stykanimate_rs::model::ClipDef {
            name: "Leap".into(),
            def,
            frames: vec![pose],
            loop_mode: stykanimate_rs::model::ClipLoop::Loop,
        });
        core.project.figures[fi].clip = Some(0);
        core
    }

    fn set_clip_media_opacity(core: &mut EditorCore, opacity: f32) {
        match &mut core.project.clips[0].def.nodes[1].media {
            Some(SegmentMedia::Image { opacity: slot, .. }) => {
                *slot = opacity
            }
            other => panic!("expected clip image media, got {other:?}"),
        }
    }

    #[test]
    fn clip_fallback_media_is_presented_read_only_in_state() {
        // Wave AC (AC5): the state carries the
        // fallback the funnel renders — label,
        // transform and opacity from the CLIP def's
        // node — while every instance-store field
        // stays empty, so the writable surface's
        // state (and the presses behind it) still
        // speaks only about the node's own slot.
        let core = core_with_clip_fallback();
        let st = segment_state(&core).expect("node 1 is selected");
        let cm = st.clip_media.expect("the fallback is live");
        assert_eq!(cm.clip_name, "Leap");
        assert_eq!(cm.label, "Sprite: hero.png");
        assert_eq!(
            cm.transform,
            Some(MediaTransformState {
                offset: [4.0, -2.0],
                scale: 1.5,
                angle_offset: 10.0,
                flip: true,
            })
        );
        assert_eq!(cm.opacity, Some(0.5));
        assert_eq!(st.media, None);
        assert_eq!(st.media_label, "No media attached");
        assert_eq!(st.attached_asset, None);
        assert_eq!(st.media_transform, None);
        assert_eq!(st.media_opacity, None);
    }

    #[test]
    fn clip_fallback_yields_to_the_nodes_own_media() {
        // The funnel's precedence, mirrored: the
        // moment the node's own slot carries media,
        // the fallback presentation disappears and
        // the writable surface takes over.
        let mut core = core_with_clip_fallback();
        let fi = core.selected_figure;
        core.project.figures[fi].def.nodes[1].media =
            Some(SegmentMedia::Image {
                asset: 0,
                offset: [0.0, 0.0],
                scale: 1.0,
                angle_offset: 0.0,
                flip: false,
                opacity: 1.0,
            });
        let st = segment_state(&core).expect("node 1 is selected");
        assert_eq!(st.clip_media, None);
        assert!(st.media_transform.is_some());
        assert_eq!(st.media_label, "Sprite: hero.png");
    }

    #[test]
    fn clip_fallback_needs_the_whole_clip_chain() {
        // No clip at all: the plain demo figure.
        let core = core_with_assets(Some(1), &["hero.png"]);
        assert_eq!(segment_state(&core).unwrap().clip_media, None);
        // A stale clip index has no fallback (the
        // funnel's `get` chain, never an index).
        let mut core = core_with_clip_fallback();
        core.project.figures[core.selected_figure].clip = Some(9);
        assert_eq!(segment_state(&core).unwrap().clip_media, None);
        // A clip node carrying no media supplies none.
        let mut core = core_with_clip_fallback();
        core.project.clips[0].def.nodes[1].media = None;
        assert_eq!(segment_state(&core).unwrap().clip_media, None);
        // A clip def shorter than the instance def:
        // the trailing node simply has no fallback.
        let mut core = core_with_clip_fallback();
        core.project.clips[0].def.nodes.truncate(1);
        assert_eq!(segment_state(&core).unwrap().clip_media, None);
    }

    #[test]
    fn clip_fallback_text_media_presents_label_without_values() {
        // A text payload falls back through the same
        // funnel arm; its presentation is the label
        // alone — text has no transform/opacity rows
        // on ANY surface (its editing is the text
        // tool's).
        let mut core = core_with_clip_fallback();
        core.project.clips[0].def.nodes[1].media =
            Some(SegmentMedia::Text {
                content: "Boo".into(),
                size: 14.0,
            });
        let st = segment_state(&core).expect("node 1 is selected");
        let cm = st.clip_media.expect("the fallback is live");
        assert_eq!(cm.label, "Text: Boo");
        assert_eq!(cm.transform, None);
        assert_eq!(cm.opacity, None);
    }

    #[test]
    fn clip_fallback_opacity_matches_the_funnels_factor() {
        // Wave AC (AC5): the cross-surface pin in
        // AB10's shape, on the fallback carrier. On
        // ONE project, the presented opacity is the
        // media factor of the funnel's composed
        // alpha — figure (instance) × style (the
        // CLIP node, the carrier) × media — and a
        // corrupt store sanitises identically on
        // both surfaces.
        let mut core = core_with_clip_fallback();
        core.project.assets[0].png = funnel_png();
        let fi = core.selected_figure;
        core.project.figures[fi].opacity = 0.5;
        core.project.clips[0].def.nodes[1].style.opacity = 0.5;
        let st = segment_state(&core).expect("node 1 is selected");
        let cm = st.clip_media.expect("the fallback is live");
        assert_eq!(cm.opacity, Some(0.5));
        assert_eq!(
            format_media_opacity(cm.opacity.expect("an opacity")),
            "50%"
        );
        assert_eq!(funnel_alpha(&core), 0.125, "figure × clip style × media");
        // Corrupt store: NaN reads as 1.0 on the
        // panel and in the funnel alike.
        let mut core = core_with_clip_fallback();
        core.project.assets[0].png = funnel_png();
        set_clip_media_opacity(&mut core, f32::NAN);
        let st = segment_state(&core).expect("node 1 is selected");
        assert_eq!(
            st.clip_media.expect("the fallback is live").opacity,
            Some(1.0)
        );
        assert_eq!(funnel_alpha(&core), 1.0, "the funnel reads NaN as 1 too");
    }

    #[test]
    fn clip_media_labels_resolve_through_the_catalogue() {
        // IA-AC5-1-dependent, in the IA-Y4-1 pattern:
        // these two keys enter the catalogue through
        // the integrator; until the insertion lands
        // `tr` returns the key itself and this pin
        // fails BY DESIGN — it passes on insertion,
        // proving the panel's provenance and flipped
        // labels are the catalogue's strings.
        let core = EditorCore::new();
        assert_eq!(
            i18n_wire::tr(&core, "segment_media_from_clip")
                .replace("{name}", "Leap"),
            "Media from clip: Leap"
        );
        assert_eq!(i18n_wire::tr(&core, "segment_media_flipped"), "Flipped");
    }

    #[test]
    fn attach_asset_refusals_are_status_only() {
        // Stale asset index.
        let mut core = core_with_assets(Some(1), &["hero.png"]);
        let depth = core.undo_depth();
        attach_asset(&mut core, 5);
        assert_eq!(core.undo_depth(), depth);
        assert!(
            core.status.contains("no asset 6"),
            "refusal explains itself: {}",
            core.status
        );
        // No selection.
        let mut core = core_with_assets(None, &["hero.png"]);
        let depth = core.undo_depth();
        attach_asset(&mut core, 0);
        assert_eq!(core.undo_depth(), depth);
        assert!(core.status.contains("Select a segment"));
        // Stale node index.
        let mut core = core_with_assets(Some(999), &["hero.png"]);
        let depth = core.undo_depth();
        attach_asset(&mut core, 0);
        assert_eq!(core.undo_depth(), depth);
        assert!(core.status.contains("no segment 999"));
    }

    #[test]
    fn attach_asset_replacing_a_text_label_says_so() {
        // One media slot: the sprite replaces the
        // label, the status owns up to it, and undo
        // restores the label whole.
        let mut core = core_with_assets(Some(1), &["hero.png"]);
        let fi = core.selected_figure;
        let label = SegmentMedia::Text {
            content: "Hero".into(),
            size: 14.0,
        };
        core.project.figures[fi].def.nodes[1].media = Some(label.clone());
        attach_asset(&mut core, 0);
        assert_eq!(
            core.project.figures[fi].def.nodes[1].media,
            Some(SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false, opacity: 1.0 })
        );
        assert!(
            core.status.contains("replacing its text label"),
            "status owns the replacement: {}",
            core.status
        );
        core.undo();
        assert_eq!(core.project.figures[fi].def.nodes[1].media, Some(label));
    }

    #[test]
    fn attached_asset_reads_only_image_media() {
        let mut core = core_with_assets(Some(1), &["hero.png"]);
        assert_eq!(attached_asset(&core), None, "no media");
        let fi = core.selected_figure;
        core.project.figures[fi].def.nodes[1].media = Some(SegmentMedia::Text {
            content: "Hi".into(),
            size: 12.0,
        });
        assert_eq!(attached_asset(&core), None, "a label is not a sprite");
        core.project.figures[fi].def.nodes[1].media =
            Some(SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false, opacity: 1.0 });
        assert_eq!(attached_asset(&core), Some(0));
        assert_eq!(attached_asset(&EditorCore::new()), None, "no selection");
        assert_eq!(
            attached_asset(&core_with_assets(Some(999), &["hero.png"])),
            None,
            "stale selection"
        );
    }

    #[test]
    fn joint_limit_steppers_engage_from_off_and_step_in_one_undo_step_each() {
        // Wave X (P-4). Node 1 (the demo chest) carries
        // no limits, so both sides start open.
        let mut core = core_with_selection(Some(1));
        let fi = core.selected_figure;
        let depth = core.undo_depth();
        // A first press on an open side engages it one
        // step out from 0° in the pressed direction.
        adjust_joint_limit(&mut core, LimitBound::Max, LIMIT_STEP);
        assert_eq!(core.project.figures[fi].def.nodes[1].max_angle, Some(5.0));
        assert_eq!(core.undo_depth(), depth + 1, "one snapshot per press");
        assert!(
            core.status.contains("maximum angle set to 5.0°"),
            "status names the bound and value: {}",
            core.status
        );
        // Stepping continues from the stored value, and
        // the other side steps independently.
        adjust_joint_limit(&mut core, LimitBound::Max, LIMIT_STEP);
        assert_eq!(core.project.figures[fi].def.nodes[1].max_angle, Some(10.0));
        adjust_joint_limit(&mut core, LimitBound::Min, -LIMIT_STEP);
        assert_eq!(core.project.figures[fi].def.nodes[1].min_angle, Some(-5.0));
        // segment_state carries the definition's limits
        // to the builder.
        let st = segment_state(&core).unwrap();
        assert_eq!(st.min_angle, Some(-5.0));
        assert_eq!(st.max_angle, Some(10.0));
        // One undo restores the last press whole.
        core.undo();
        assert_eq!(core.project.figures[fi].def.nodes[1].min_angle, None);
        assert_eq!(core.project.figures[fi].def.nodes[1].max_angle, Some(10.0));
    }

    #[test]
    fn limit_arc_readout_reports_the_models_partition() {
        // Roadmap Wave 2 (W2-27): the readout's numbers
        // are `limit_spans`' sweeps in whole degrees —
        // elbow/knee (170 allowed either way), the wrap
        // arc (20), a single bound (150), a pinned joint
        // (0), and nothing at all without limits.
        assert_eq!(
            format_limit_arc(Some(-10.0), Some(160.0)).as_deref(),
            Some("Limit arc: 170° allowed, 190° blocked")
        );
        assert_eq!(
            format_limit_arc(Some(-160.0), Some(10.0)).as_deref(),
            Some("Limit arc: 170° allowed, 190° blocked")
        );
        assert_eq!(
            format_limit_arc(Some(170.0), Some(-170.0)).as_deref(),
            Some("Limit arc: 20° allowed, 340° blocked")
        );
        assert_eq!(
            format_limit_arc(Some(30.0), None).as_deref(),
            Some("Limit arc: 150° allowed, 210° blocked")
        );
        assert_eq!(
            format_limit_arc(Some(45.0), Some(45.0)).as_deref(),
            Some("Limit arc: 0° allowed, 360° blocked")
        );
        assert_eq!(format_limit_arc(None, None), None);
        // The panel builds with the toggle on and a
        // limited node selected (the readout + toggle row
        // mount without incident).
        let mut s = crate::init();
        s.core = core_with_selection(Some(1));
        let fi = s.core.selected_figure;
        s.core.project.figures[fi].def.nodes[1].min_angle = Some(-10.0);
        s.core.project.figures[fi].def.nodes[1].max_angle = Some(160.0);
        s.limit_arcs = true;
        let t = rune::tok::theme(true);
        let _ = inspector_segment(&s, &t);
    }

    #[test]
    fn limit_arc_toggle_flips_only_its_own_flag() {
        // W2-27: the seed is OFF (IA-W2-27-1), the helper
        // flips it and nothing else, and a second flip
        // restores it — the guides toggles' discipline.
        let mut s = crate::init();
        assert!(!s.limit_arcs, "seeded off");
        toggle_limit_arcs(&mut s);
        assert!(s.limit_arcs);
        toggle_limit_arcs(&mut s);
        assert!(!s.limit_arcs);
    }

    #[test]
    fn joint_limit_stepper_at_the_clamp_end_is_a_silent_no_op() {
        // Stepping at the ±180° hard clamp lands on the
        // stored value: no write, no status, no undo
        // step (the Straighten-at-zero rule).
        let mut core = core_with_selection(Some(1));
        let fi = core.selected_figure;
        core.project.figures[fi].def.nodes[1].max_angle = Some(180.0);
        let depth = core.undo_depth();
        core.status = "Sentinel.".into();
        adjust_joint_limit(&mut core, LimitBound::Max, LIMIT_STEP);
        assert_eq!(core.project.figures[fi].def.nodes[1].max_angle, Some(180.0));
        assert_eq!(core.undo_depth(), depth, "no snapshot for a no-change press");
        assert_eq!(core.status, "Sentinel.", "no status churn");
    }

    #[test]
    fn joint_limit_refusals_are_status_only() {
        // No selection.
        let mut core = core_with_selection(None);
        let depth = core.undo_depth();
        adjust_joint_limit(&mut core, LimitBound::Min, -LIMIT_STEP);
        assert_eq!(core.undo_depth(), depth);
        assert!(core.status.contains("Select a segment"));
        // The origin: its angle is never solved, so a
        // limit on it could never bind anything.
        let mut core = core_with_selection(Some(0));
        let depth = core.undo_depth();
        adjust_joint_limit(&mut core, LimitBound::Max, LIMIT_STEP);
        assert_eq!(core.undo_depth(), depth);
        assert!(
            core.status.contains("origin has no joint"),
            "refusal explains itself: {}",
            core.status
        );
        // Stale node index.
        let mut core = core_with_selection(Some(999));
        let depth = core.undo_depth();
        clear_joint_limits(&mut core);
        assert_eq!(core.undo_depth(), depth);
        assert!(core.status.contains("no segment 999"));
    }

    #[test]
    fn clear_joint_limits_clears_both_sides_in_one_undo_step() {
        // Node 4 is the demo forearm: the constructor's
        // presets (-10..160) are set, and clearing
        // removes both sides in a single undo step.
        let mut core = core_with_selection(Some(4));
        let fi = core.selected_figure;
        assert!(core.project.figures[fi].def.nodes[4].has_limits());
        let depth = core.undo_depth();
        clear_joint_limits(&mut core);
        assert_eq!(core.project.figures[fi].def.nodes[4].min_angle, None);
        assert_eq!(core.project.figures[fi].def.nodes[4].max_angle, None);
        assert_eq!(core.undo_depth(), depth + 1, "one snapshot for the clear");
        assert!(core.status.contains("joint limits cleared"));
        core.undo();
        assert_eq!(core.project.figures[fi].def.nodes[4].min_angle, Some(-10.0));
        assert_eq!(core.project.figures[fi].def.nodes[4].max_angle, Some(160.0));
    }

    #[test]
    fn clear_joint_limits_without_limits_is_a_silent_no_op() {
        let mut core = core_with_selection(Some(1));
        let depth = core.undo_depth();
        core.status = "Sentinel.".into();
        clear_joint_limits(&mut core);
        assert_eq!(core.undo_depth(), depth, "no snapshot when nothing is set");
        assert_eq!(core.status, "Sentinel.", "no status churn");
    }

    #[test]
    fn apply_joint_limit_preset_sets_both_bounds_in_one_undo_step() {
        // Node 1 carries no limits: one press stores the
        // model's elbow range verbatim — the same values
        // the humanoid constructor writes — under exactly
        // one snapshot, and undo restores the open sides.
        let mut core = core_with_selection(Some(1));
        let fi = core.selected_figure;
        assert!(!core.project.figures[fi].def.nodes[1].has_limits());
        let depth = core.undo_depth();
        apply_joint_limit_preset(&mut core, LimitPreset::Elbow);
        let node = &core.project.figures[fi].def.nodes[1];
        assert_eq!(node.min_angle, Some(NodeDef::ELBOW_LIMITS.0));
        assert_eq!(node.max_angle, Some(NodeDef::ELBOW_LIMITS.1));
        assert_eq!(core.undo_depth(), depth + 1, "one snapshot for the preset");
        assert!(
            core.status.contains("elbow range"),
            "status names the applied range: {}",
            core.status
        );
        core.undo();
        let node = &core.project.figures[fi].def.nodes[1];
        assert_eq!(node.min_angle, None);
        assert_eq!(node.max_angle, None);
    }

    #[test]
    fn apply_joint_limit_preset_replaces_an_existing_range() {
        // Node 4 is the demo forearm: it already carries
        // the elbow range from the constructor, and the
        // knee press replaces BOTH bounds with the knee
        // range in one step — never a half-applied mix.
        let mut core = core_with_selection(Some(4));
        let fi = core.selected_figure;
        let depth = core.undo_depth();
        apply_joint_limit_preset(&mut core, LimitPreset::Knee);
        let node = &core.project.figures[fi].def.nodes[4];
        assert_eq!(node.min_angle, Some(NodeDef::KNEE_LIMITS.0));
        assert_eq!(node.max_angle, Some(NodeDef::KNEE_LIMITS.1));
        assert_eq!(core.undo_depth(), depth + 1, "one snapshot for the replace");
        assert!(core.status.contains("knee range"));
    }

    #[test]
    fn apply_joint_limit_preset_already_applied_is_a_silent_no_op() {
        // Node 4 already carries the elbow range (the
        // constructor's presets): pressing Elbow again
        // stores nothing and spends no undo step.
        let mut core = core_with_selection(Some(4));
        let depth = core.undo_depth();
        core.status = "Sentinel.".into();
        apply_joint_limit_preset(&mut core, LimitPreset::Elbow);
        assert_eq!(core.undo_depth(), depth, "no snapshot for a no-change press");
        assert_eq!(core.status, "Sentinel.", "no status churn");
    }

    #[test]
    fn apply_joint_limit_preset_refusals_are_status_only() {
        // No selection.
        let mut core = core_with_selection(None);
        let depth = core.undo_depth();
        apply_joint_limit_preset(&mut core, LimitPreset::Elbow);
        assert_eq!(core.undo_depth(), depth);
        assert!(core.status.contains("Select a segment"));
        // The origin, with the limit ops' own refusal.
        let mut core = core_with_selection(Some(0));
        let depth = core.undo_depth();
        apply_joint_limit_preset(&mut core, LimitPreset::Knee);
        assert_eq!(core.undo_depth(), depth);
        assert!(core.status.contains("origin has no joint"));
        // Stale node index.
        let mut core = core_with_selection(Some(999));
        let depth = core.undo_depth();
        apply_joint_limit_preset(&mut core, LimitPreset::Elbow);
        assert_eq!(core.undo_depth(), depth);
        assert!(core.status.contains("no segment 999"));
    }

    #[test]
    fn format_limit_bound_reads_off_or_degrees() {
        // IA-X6-1-dependent, like the i18n pin below:
        // the "Off" side resolves through
        // `segment_limit_off`, so this passes once the
        // integrator lands the key.
        let core = EditorCore::new();
        assert_eq!(format_limit_bound(&core, None), "Off");
        assert_eq!(format_limit_bound(&core, Some(-10.0)), "-10.0°");
        assert_eq!(format_limit_bound(&core, Some(160.0)), "160.0°");
    }

    #[test]
    fn segment_i18n_keys_exist_and_match_the_english_labels() {
        // Wave T (T4): every label the panel wires
        // through `i18n_wire::tr` must resolve to exactly
        // the English string the panel rendered before
        // the wiring. The keys land via IA-T4-1 (the
        // integrator appends them to all 28 tables after
        // the threed block), so this pin passes only
        // once that IA lands — until then `tr` falls back
        // to the key itself and `key_in_table` is false
        // (the Q7 pattern: wiring and keys land together
        // at integration). `segment_title_node` is the
        // one template: the pin holds its stored form,
        // placeholders intact (the camera
        // `camera_following_figure` precedent).
        //
        // Wave U (U7) extends the pin with the composed
        // readouts' keys: the last eleven pairs below
        // land via IA-U7-1, so the pin as a whole passes
        // only once that IA lands too. Templates are
        // asserted in their stored form, placeholders
        // intact, like `segment_title_node`.
        //
        // Wave Y (Y12) extends the pin with the preset
        // row's keys: the last two pairs land via
        // IA-Y12-1, so the pin as a whole passes only
        // once that IA lands too.
        let core = EditorCore::new();
        for (key, english) in [
            ("segment_title", "Segment"),
            ("segment_title_node", "Segment {index}"),
            (
                "segment_select_a_segment",
                "Select a segment on the stage to edit its geometry.",
            ),
            ("segment_parent_joint", "Parent joint"),
            ("segment_child_joint", "Child joint"),
            ("segment_length", "Length"),
            ("segment_thickness", "Thickness"),
            ("segment_angle", "Angle"),
            ("segment_bend", "Bend"),
            ("segment_shape", "Shape"),
            ("segment_rotate", "Rotate"),
            ("segment_straighten", "Straighten"),
            ("segment_cycle_bend", "Cycle bend"),
            ("segment_cycle_shape", "Cycle shape"),
            ("segment_colour", "Colour"),
            ("segment_cycle_colour", "Cycle"),
            ("segment_media", "Media"),
            ("segment_next_sprite", "Next sprite"),
            ("segment_attach_from_import", "Attach from import"),
            ("segment_clear_media", "Clear media"),
            ("segment_add", "+ Segment"),
            ("segment_delete", "Delete segment"),
            // Wave U (U7) readout keys — IA-U7-1.
            (
                "segment_topology",
                "Node {node} · {parent} · {children}",
            ),
            ("segment_topology_parent", "parent {parent}"),
            ("segment_topology_root", "root"),
            ("segment_topology_one_child", "1 child"),
            ("segment_topology_children", "{count} children"),
            ("segment_bend_readout", "Bend {degrees}°"),
            ("segment_media_none", "No media attached"),
            ("segment_media_sprite", "Sprite: {name}"),
            (
                "segment_media_sprite_missing",
                "Sprite: asset {index} (missing)",
            ),
            ("segment_media_text", "Text: {content}"),
            ("segment_media_text_empty", "Text label (empty)"),
            // Wave X (X6) joint-limit keys — IA-X6-1.
            ("segment_limit_min", "Min angle"),
            ("segment_limit_max", "Max angle"),
            ("segment_limit_off", "Off"),
            ("segment_limit_clear", "Clear limits"),
            // Wave Y (Y12) preset-row keys — IA-Y12-1.
            ("segment_limit_preset_elbow", "Elbow range"),
            ("segment_limit_preset_knee", "Knee range"),
        ] {
            assert!(i18n_wire::key_in_table(key), "missing i18n key: {key}");
            assert_eq!(i18n_wire::tr(&core, key), english);
        }
    }
}
