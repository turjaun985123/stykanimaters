//! Graph panel for the Rune editor shell (Phase D, section 9
//! of the GPUI→Rune migration): the channel selector, the
//! curve canvas, the interpolation segmented control, the
//! tangent-preset picker and the channel transform actions.
//!
//! Declared from `main.rs` as `crate::rune_sec::graph_panel`;
//! bin-crate code, not part of the engine library. The public
//! entry point is [`graph_panel`].
//!
//! ## Where the data and the math live
//!
//! Channels are project data (`core.project.channels`); the
//! curve drawn here is sampled through the REAL evaluation —
//! `graph::Channel::value_at`, the cubic-Hermite / Linear /
//! Step evaluator the renderer itself uses — mapped to panel
//! pixels by the engine's own view-model
//! (`graph_panel::GraphView::auto_fit` + `curve_points`, and
//! `graph_panel::preset_preview` for the preset chips). This
//! module adds no evaluation math of its own.
//!
//! ## Where the edits go
//!
//! Every control calls a real engine op: an `EditorCore`
//! graph op (`key_graph_angle`,
//! `apply_graph_ease_preset`, `smooth_graph_tangents`,
//! `bake_graph_channels`, `clear_graph`) — or, for the
//! interp segmented control since the UI mission
//! (S12-08), the graph-keys library op it shares with
//! MCP and the keyframe tools,
//! `graph_keys::set_key_interp` (via
//! [`set_drawn_interp`], falling back to the core's
//! `set_graph_key_interp` when no channel is drawn).
//! Three actions have
//! NO dedicated core op — mirror-in-time, scale-values and
//! reduce-keys exist only as `graph::` free functions — so
//! their handlers follow the exact precedent the MCP tools
//! set (`mcp_tools::tool_mirror_channel_time` /
//! `tool_scale_channel_values`): `core.snapshot()` for undo,
//! apply the `graph::` function to the selected channel,
//! restore key order, and write the outcome to `core.status`.
//! Those handlers are factored as `apply_*(&mut EditorCore,
//! ..)` so they are testable without a `State`. A fourth
//! action has no core op either — Wave Y's "Key bend"
//! ([`key_bend`]): it composes `graph::channel_mut` +
//! `Channel::add_key` panel-side, the same two calls
//! `key_graph_angle` makes inside the core, so the Bend
//! channel (P-7) can be authored from this panel like the
//! angle channel can.
//!
//! ## Canvas pointer interaction
//!
//! The curve canvas also takes drags (`graph_drag`, factored
//! into the testable `drag_down` / `drag_move` / `drag_up`
//! over a bare `EditorCore` + [`GraphPanelState`]):
//!
//! - **Down on a tangent handle** of the selected key
//!   (hit-tested in data space by [`hit_tangent_handle`])
//!   takes priority over the key hit-test below: one
//!   snapshot, and the drag edits that key's tangent
//!   instead — see "Tangent drags" further down.
//! - **Down on a key handle** (within [`GRAB_RADIUS`] px,
//!   hit-tested by `GraphView::hit_key`): one
//!   `EditorCore::snapshot()` — the only snapshot of the
//!   gesture — and the key is grabbed.
//! - **Move**: the pointer converts back to data through the
//!   SAME fit the drawing used (see [`drag_view`]), the key's
//!   frame is clamped to `>= 0.0`, its value is unclamped,
//!   and the channel is re-sorted after every move
//!   (`drag_key_to`), so `value_at` — and the graph-aware
//!   posing that reads these channels — stays valid on every
//!   redraw, even mid-drag. Angle channels need no extra
//!   work: the stage's `positions_f` evaluates channels
//!   itself.
//! - **Up**: the final key position is written to
//!   `core.status`. A gesture the runtime reports as
//!   cancelled (Escape / focus loss) instead restores the
//!   pre-drag project with one `core.undo()` — the Down
//!   snapshot is exactly one undo step.
//! - **A click that hits no key** (Down+Up within
//!   [`CLICK_TRAVEL`] px) seeks: `core.goto` of the frame
//!   under the pointer, rounded (`seek_frame`). A longer
//!   drag across empty canvas does nothing.
//!
//! The handlers recompute the fit per phase from the drag's
//! own `w`/`h` — the size the element was actually drawn at
//! for this gesture — via the same [`drag_view`] the builder
//! draws with, so hit math can never disagree with the
//! drawing. One honest consequence: `auto_fit` derives its
//! value range from the channel's keys, so dragging the
//! extreme key of a non-angle channel re-pads the fit
//! slightly as it moves (the key trails the pointer by the
//! padding factor); angle channels are immune (their fit is
//! always the full -180..=180 circle), and the frame range
//! only stretches when a key is dragged past the current
//! end. The drawing and the drag always share the fit, so
//! they never disagree with each other.
//!
//! ## Tangent drags
//!
//! The selected key also grows a pair of tangent handles —
//! small diamonds joined to the key by a thin line, drawn at
//! the Bézier control-point position on each side's tangent
//! ray ([`tangent_handle_data`]: a third of the neighbouring
//! segment's span along the ray). A press on one grabs the
//! HANDLE, not the key: Moves rewrite only that side's slope
//! from the pointer's data position ([`tangent_slope`]) —
//! per-side, i.e. broken tangents, which is the model's own
//! convention: `in_tan`/`out_tan` are independent fields,
//! `Channel::set_tangents` writes them independently, and
//! the EaseIn/EaseOut presets set them asymmetrically on
//! purpose. Aligning the two sides is what the batch ops
//! are for (`auto_tangents`, the presets), not a drag
//! constraint.
//!
//! Gesture discipline matches the key drag exactly: one
//! snapshot at Down (only on a hit), none during Moves, one
//! `core.undo()` rollback when the runtime cancels. Two
//! simplifications are safe here, documented rather than
//! hidden. First, frames and values never change during a
//! tangent drag, so keys never re-sort and the grabbed
//! index needs no anchor/re-resolution (contrast
//! [`GraphPanelState::drag_key`]). Second, `auto_fit`'s
//! ranges derive from key frames and values alone, so the
//! fit is bit-stable across the whole gesture — the pointer
//! mapping cannot drift mid-drag the way an extreme key
//! drag can re-pad it. The honest cost of that stability: a
//! strong hand-set tangent can bend the drawn curve outside
//! the fitted value range, where the canvas clips it — the
//! same thing that already happens to preset-shaped curves.
//!
//! ## Scoping, honestly
//!
//! The core's graph ops scope by SELECTION, not by this
//! panel's drawn channel: presets apply to every channel
//! of the selected figure. The interp segmented control
//! used to scope that way too — it edited the selected
//! figure's `Angle(selected_node)` key nearest the
//! current frame, whatever curve was actually drawn, and
//! was labelled by that hidden target. Since the UI
//! mission (S12-08) the control edits the DRAWN channel:
//! its highlight reads the drawn channel's key at the
//! playhead and its press writes that same key, both
//! through the graph-keys library ([`drawn_interp`] /
//! [`set_drawn_interp`] over `graph_keys::key_interp_at`
//! / `set_key_interp` — the ops MCP and the keyframe
//! tools' interp cycle already use), and the row is
//! labelled with the drawn channel's own label. Only
//! when NO channel is drawn does the control fall back
//! to the original angle-key path
//! (`EditorCore::set_graph_key_interp`, read back by
//! [`current_interp`]), and the label says so, exactly
//! as it always did. Nothing here pretends otherwise.
//!
//! The tangent presets exist three times in the Graph
//! workspace, and since UI mission Wave 9 they share
//! ONE construction: the kit's
//! [`design::tangent_preset_chip`], which Wave 4 lifted
//! from this panel's own chip (the canonical surface —
//! the only copy whose presentation carried the drawn
//! preview) and which the keyframe tools and the
//! easing panel adopted then. This panel kept a private
//! copy until Wave 9 (agent B6) verified the two
//! identical metric for metric — preview box 52×30,
//! 24 samples, padding 4, gap 2, radius 6, card fill
//! with `control_track` / `control_off` hover/press,
//! LABEL text in `t.text`, the same
//! `apply_graph_ease_preset` press — and converged.
//! The one real difference the convergence brings is
//! the kit chip's `.help(label)` tooltip, the Wave 6
//! convention this panel's chips were still missing.
//! [`preview_points`] stays below as this module's
//! pinned reference for the preview mapping (the kit
//! keeps its own private copy; design's docs note the
//! two agree point for point), exercised by its unit
//! test like [`selected_key_index`].
//!
//! ## Channel list (Phase G)
//!
//! Below the header sits a LIST of every channel in the
//! project ([`channel_rows`]: index, "F{n} {target}", key
//! count; rows keyed `graph-chan-{i}` with the channel's
//! index in `project.channels`). A list, not the prev/next
//! cycling some panels use: this panel's idiom is already
//! direct-pick (the chip selector), a project's channel
//! count is small, and a list shows figure, target and key
//! count for every channel at a glance — cycling would hide
//! all but one and cost a press per step of travel.
//!
//! The chip selector above the canvas stays: it offers the
//! SELECTED figure's targets, including the keyless pinned
//! roots a user may be about to key. The list is the
//! complementary browser — only channels that actually
//! exist, across ALL figures. A row press ([`select_channel`])
//! records the row's INDEX in
//! `GraphPanelState::selected_channel` and keeps
//! `GraphPanelState::target` in sync with that channel's
//! target (plus `EditorCore::select_figure` when the
//! channel's figure is not the selected one); a chip press
//! writes the target alone and clears the index. Key drags,
//! tangent drags, click-seek and the transform actions all
//! resolve through [`effective_channel_index`], so they
//! operate on exactly the channel a row names. Switching
//! figures through a row press behaves like any figure
//! switch in the app (`select_figure` clears the node
//! selection and is not an undo step); pressing a row for
//! the already-selected figure leaves the node selection
//! untouched.
//!
//! One honesty note, updated for Phase J: the target state
//! names a `(figure, target)` pair, and the engine itself
//! treats a second channel with the same pair as a shadowed
//! duplicate ("the later one in the slice wins" —
//! graph.rs's module docs), while `channel_index` resolves
//! the FIRST match. Before Phase J that meant both
//! duplicate rows selected the same effective channel and
//! the second could never be drawn or edited here. Row
//! selection is now by index, so each duplicate can be
//! drawn, dragged and transformed individually in this
//! panel. What has NOT changed is the engine's own
//! evaluation of duplicates for the stage and exports —
//! that resolution lives in graph.rs, not in this panel.
//! Project data written by the editor's own ops never
//! creates duplicates (`graph::channel_mut` reuses the
//! existing channel).
//!
//! The empty state names the verified way a first channel
//! appears: `EditorCore::key_graph_angle` creates one via
//! `graph::channel_mut` — it is this panel's "Key angle"
//! action and the keyframe tools' "Key angle here" alike.
//! (Pasting keys requires an existing channel, baking
//! consumes channels, and file imports may also carry
//! channels in.)
//!
//! ## Icons (Phase H)
//!
//! Two decorative glyphs, both the catalogue's
//! `chart-line`, drawn via [`icon_el`]: one leads the
//! panel header (14 pt), one leads every channel-list
//! row (12 pt), each in the muted palette tone
//! [`glyph_muted`]. They are pure decoration — no key,
//! no press, no state — so row keys (`graph-chan-{i}`),
//! labels and handlers are byte-identical to before.
//!
//! Everything else deliberately stays text. The interp
//! segmented control (Hermite / Linear / Step) and the
//! tangent-preset chips (Smooth, Ease in, …) name
//! CONCEPTS, not glyph-clear actions; the preset chips
//! already carry the most honest glyph possible — a
//! drawn preview of the preset's own curve. The actions
//! row (Key angle, Auto tangents, Mirror time, …) is
//! likewise labelled operations whose names are the
//! meaning; a wand on "Auto tangents" or "Smooth" would
//! claim magic the ops do not perform.
//!
//! ## Stats block (Phase I)
//!
//! Under the channel list sits a stats block (keyed
//! `graph-stats`, one line per channel keyed
//! `graph-stat-{i}`): the glanceable numbers for every
//! channel — figure name, target, key count and value
//! range — from the engine's read-only
//! `graph_keys::channel_stats` ([`stat_rows`] projects it,
//! [`stat_line`] formats one row). Frames are deliberately
//! omitted: keys + range is what a glance needs, and the
//! canvas above already shows the span.
//!
//! The block exists only when the project has channels —
//! with none, the channel list's own empty-state line
//! already says everything. The DRAWN channel's line (the
//! channel the canvas shows, resolved by
//! [`effective_channel_index`] exactly as the list's active
//! row is) renders in the normal text colour; every other
//! line is muted. Duplicate `(figure, target)` channels
//! highlight the PICKED row when one was picked by index
//! (Phase J); a target-only state still highlights the
//! first match — the same resolution every edit path uses.
//!
//! Wave 10 bounds the block: it was the channel list's
//! unbounded-growth defect in sibling form (one line per
//! channel in the whole project, no cap — 17 pt of column
//! per channel, without limit). The builder now renders
//! [`stat_display_rows`]' projection instead of the raw
//! rows: at most [`STATS_MAX_LINES`] lines
//! ([`STATS_MAX_H`] tall) — the drawn channel's line in
//! full, the first other lines in project order, and a
//! `graph-stats-more` summary line counting the folded
//! channels and naming the way back to them (a list row
//! press draws the channel, restoring its full line).
//! [`stat_rows`] itself is unchanged and still returns
//! every channel's row.
//!
//! ## Value readout (Phase L)
//!
//! Directly under the canvas, a muted line keyed
//! `graph-value` ([`value_line`]) states the drawn
//! channel's value at the playhead — "Value at frame
//! {current+1}: {v:.1}", the real `Channel::value_at`
//! evaluation. It duplicates the header readout's tail
//! deliberately: the header is a wide row of small print,
//! and the value under the curve is the number a graph
//! edit is checked against. The line exists only while a
//! channel is drawn, resolved by
//! [`effective_channel_index`] exactly as the canvas is.
//!
//! Wave Y (Y12) adds the enforcement context both
//! readouts were missing: an Angle channel's target node
//! may carry joint limits (Wave X, P-4) that
//! `apply_channels` clamps through at evaluation, so the
//! raw key range and the raw playhead value are not
//! always what applies. [`joint_limit_note`] appends
//! "· joint limits {min}..{max}" to the stat row and the
//! value line of a limited Angle channel — the raw
//! numbers stay first and unchanged (stored data is
//! never rewritten; the note names what the solvers
//! will enforce).
//!
//! ## Channel selection by index (Phase J)
//!
//! `GraphPanelState` now carries `selected_channel:
//! Option<usize>` beside the legacy `target`. The rule is
//! one line: [`effective_channel_index`] returns the index
//! while it names a live channel of the selected figure,
//! and the legacy `(figure, target)` first-match resolution
//! otherwise. Every drawn-channel consumer in this module
//! — the builder's readout and canvas, the list's active
//! row, the stats highlight, the three drag phases and the
//! mirror/scale/reduce actions (via their new `*_at`
//! index forms) — resolves through that one helper, so the
//! second of two duplicate channels can finally be seen
//! and edited. Staleness is handled by the same helper: a
//! deleted channel or a figure switch makes the index fall
//! back to the target path, and when that also finds
//! nothing, nothing is drawn or highlighted. The target
//! field is NOT vestigial: chips write it, the fallback
//! reads it, and [`select_channel`] keeps it in sync on
//! every row press for the consumers that still read it
//! (the selector chips' active state among them).
//!
//! ## Delete channel (Phase K)
//!
//! The channel list gains a `graph-chan-delete` button
//! beside the Phase J `graph-chan-duplicate`, shown under
//! the same condition (a channel is drawn). Its handler
//! [`delete_drawn_channel`] removes the drawn channel
//! through `graph_keys::delete_channel` — the op owns
//! the snapshot — and then repairs the stored pick with
//! [`selection_after_delete`]: a pick on the deleted row
//! is cleared (target resolution takes over), a pick
//! past it shifts down one row, and `target` is never
//! touched.
//!
//! ## Shift channel keys (Phase M)
//!
//! The same conditional block gains a `graph-shift-l` /
//! `graph-shift-r` pair (icon buttons, "Shift -1" /
//! "Shift +1"): each press moves EVERY key of the drawn
//! channel one frame earlier or later through
//! `graph_keys::shift_keys`. The handler
//! [`shift_drawn_channel`] resolves through
//! [`effective_channel_index`] exactly as Delete does,
//! so the pair acts on the channel Delete would remove;
//! the op owns the snapshot and the frame-0 refusal
//! (the whole shift is refused, never clamped per key).
//!
//! ## Phase N adoption
//!
//! The panel root is the design kit's card
//! ([`design::panel_col`]): card fill, hairline border,
//! radius 12, padding 12. The header is the kit
//! [`design::panel_title`] — the `chart-line` glyph at
//! the kit's 13 pt in `text_secondary`, the title in
//! SECTION — with the readout kept as the header row's
//! trailing LABEL line, over a kit [`design::divider`].
//! Channel-list rows are kit [`design::list_row`]
//! shells (selected = the kit's accent-at-16% fill,
//! replacing the `control_track` / `card` fills) with
//! the press and children verbatim, and the list's
//! empty line is the kit [`design::empty_state`] under
//! the panel's own glyph. The interp row's caption is
//! a kit [`design::section_label`]. The curve canvas
//! (`graph-canvas`, its `.on_drag` and every hit-test),
//! the tangent handles, the preset chips (each carries
//! its own drawn curve) and the stats / value lines
//! are unchanged: canvas work and readouts, not
//! chrome. The "No graph keys on this channel yet"
//! line is also the kit [`design::empty_state`] under
//! the same glyph (UI mission Wave 6, audit S12-16 —
//! the mission §5G / Phase O charter rule 7 supersede
//! the earlier Phase N ruling that kept it a bare
//! muted LABEL line as per-channel guidance).
//!
//! ## Wave 9 polish (agent B6)
//!
//! Three audit leftovers closed, each verified at
//! source before it was touched:
//!
//! - **S12-09, this panel's half.** The tangent-preset
//!   chips build through the kit (see the scoping
//!   section above); the private copy is gone.
//! - **S12-07 / D2.** The channel list's scroll
//!   viewport gained its missing bound
//!   ([`CHAN_LIST_MAX_H`], five rows), so the canvas
//!   keeps its place however many channels the project
//!   grows; the Duplicate / Delete / Shift block moved
//!   from inside the list to directly under the
//!   viewport so the bound cannot hide it.
//! - **S12-12 / D8, the boundary half.** The curve
//!   canvas takes `.border(1.0, t.card_border)` — the
//!   kit hairline the panel card itself wears — so the
//!   viewport reads as a viewport against the identical
//!   `t.card` fill in both themes. The rest of D8
//!   (gridlines and fit-range labels, S12-11) is a
//!   separate item and was not attempted here.
//!
//! ## Wave 10 (agent B8)
//!
//! The deferred sibling of Wave 9's S12-07 fix: the
//! stats block's unbounded growth is bounded in the
//! same kit idiom (see the Stats block section above
//! and [`STATS_MAX_H`] / [`stat_display_rows`]). The
//! rest of the panel was re-audited for the same class:
//! the channel list is Wave 9's bounded viewport; the
//! key / tangent handle loops paint `Shape` geometry
//! inside the fixed [`GRAPH_H`] canvas, not element
//! rows; the selector chips dedup by target over the
//! SELECTED figure's channels only (bounded by that
//! figure's node count, in a `flex_wrap` row — a
//! different, much smaller growth class, left as is);
//! the interp row, the preset chips and the actions
//! row are fixed hand-written rows. The stats block
//! was the only remaining per-project-channel element
//! growth in the panel.
//!
//! ## State the shell must provide
//!
//! `State` needs one field (integrator wires it):
//! `pub graph_panel: crate::rune_sec::graph_panel::GraphPanelState`
//! — see [`GraphPanelState`].

use rune::tok::{self, type_, Theme};
use rune::{canvas, col, row, text, Color, Cx, Drag, El, LineCap, Phase, ScrollSpec, Shape, Tx};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::graph::{Channel, ChannelTarget, Interp, Key, TangentPreset};
use stykanimate_rs::graph_keys::{channel_stats, ChannelStat};
use stykanimate_rs::graph_panel::{preset_preview, GraphView};
use stykanimate_rs::model::V2;

use crate::chrome_view::{btn, danger_btn};
use crate::rune_sec::app_theme;
use crate::rune_sec::design;
use crate::rune_sec::icon_btn::icon_btn;
use crate::rune_sec::icons::icon_el_rgb;
use crate::stage_view::hex_color;
use crate::State;

/// Height of the curve canvas, in points.
pub const GRAPH_H: f32 = 170.0;
/// Canvas size assumed before the first committed layout
/// exists (the `Cx::layout_size` read settles after one frame,
/// exactly as the stage's does).
const FALLBACK_SIZE: [f32; 2] = [720.0, GRAPH_H];
/// Maximum height of the channel list's scroll viewport,
/// in points (Wave 9, audit S12-07): five channel rows.
/// The kit row's own metrics set the price — `list_row`
/// rows are `min_h` 28 (design's `row_base`) and the
/// list gaps 4 — so five full rows are 5 × 28 + 4 × 4 =
/// 156. Past five channels the list scrolls inside its
/// own viewport instead of pushing the canvas, the
/// interp row and everything below them further down
/// the column with every added channel (the audit's
/// D2: the viewport previously carried `.min_h(0.0)`
/// but no bound, and a Rune scroll viewport without a
/// bounded height grows to its content).
const CHAN_LIST_MAX_H: f32 = 156.0;
/// Height of one stats-block line, in points: the
/// block's lines are `type_::LABEL` text (12 pt), whose
/// effective line height is size × 1.25 (rune
/// `TextStyle`'s line-height fallback) = 15.
const STAT_LINE_H: f32 = 15.0;
/// Maximum height of the stats block, in points
/// (Wave 10): six stats lines — 6 × 15
/// ([`STAT_LINE_H`]) + 5 × 2 (the block's own gap) =
/// 100. Before this bound the block was a plain
/// unbounded column, one line per channel in the whole
/// project at 17 pt per channel (15 line + 2 gap), so
/// 24 channels cost 406 pt and 40 cost 678 pt of the
/// right column, pushing the canvas, the interp row and
/// everything under them down and eventually out of
/// the card — the channel list's pre-Wave-9 defect
/// (audit S12-07) in its sibling block, deferred by
/// Wave 9's agent B6 and closed here. Past
/// [`STATS_MAX_LINES`] the block stops growing:
/// [`stat_display_rows`] folds the overflow into a
/// summary line instead.
pub const STATS_MAX_H: f32 = 100.0;
/// Maximum number of lines the stats block renders,
/// the summary line included: the line count
/// [`STATS_MAX_H`] buys at [`STAT_LINE_H`] a line plus
/// the 2 pt gap — (100 + 2) / (15 + 2) = 6. One slot
/// of the six is the summary line whenever the project
/// overflows, and the DRAWN channel's line is never
/// the one folded away (see [`stat_display_rows`]).
pub const STATS_MAX_LINES: usize = 6;
/// The decorative `chart-line` glyphs' muted tone
/// (channel-list rows; the panel header's glyph moved
/// to the kit title in Phase N), in the `0xRRGGBB` form
/// [`icon_el_rgb`] takes: the shell's muted-text role
/// ([`app_theme::muted_text`]) packed at the use site —
/// `#737D8C` in dark, `text_secondary` mixed toward
/// `bg` in light. (Before Wave 7 this was a private
/// literal, `0x707B8C`: a pre-Wave-2 palette leftover
/// that never quite matched the token it claimed to
/// be.)
fn glyph_muted(t: &Theme) -> u32 {
    pack(app_theme::muted_text(t))
}

/// Pack a theme colour into the `0xRRGGBB` form the
/// icon renderer takes (the `fig3_preview` precedent:
/// via the colour's `#RRGGBBAA` hex string).
fn pack(c: Color) -> u32 {
    let hex = c.hex();
    u32::from_str_radix(&hex[1..7], 16).unwrap_or(0)
}
/// Key-reduction tolerance, in channel value units (degrees
/// for angle channels, stage units for root/length): interior
/// keys whose removal moves the curve by at most this much are
/// dropped. Deliberately conservative — it removes
/// near-duplicates, not shape.
pub const REDUCE_TOLERANCE: f32 = 0.5;
/// Grab radius for key handles on the curve canvas, in
/// points. The handles DRAW at radius 4.5; the pick radius is
/// deliberately more forgiving.
pub const GRAB_RADIUS: f32 = 10.0;
/// Pointer travel, in points, under which a Down+Up on empty
/// canvas counts as a click (and seeks) rather than a drag.
/// Matches the 4 pt click-run threshold Rune itself uses for
/// click counting (see the `Drag::clicks` docs).
pub const CLICK_TRAVEL: f32 = 4.0;
/// Where along the neighbouring segment a tangent handle
/// sits, as a fraction of the segment's frame span: exactly
/// the Hermite→Bézier control-point position (1/3), so the
/// handle lies on the key's tangent line at the point the
/// equivalent cubic Bézier would put its control point.
pub const TANGENT_HANDLE_FRAC: f32 = 1.0 / 3.0;
/// Drawn radius of a tangent-handle diamond, in points —
/// smaller than the key diamonds (4.5) they hang off. The
/// pick radius is the shared [`GRAB_RADIUS`].
pub const TANGENT_HANDLE_RADIUS: f32 = 3.0;
/// Closest a pointer may sit to a key's frame, in frames,
/// for a tangent drag to read a slope from it: nearer than
/// this the slope would explode toward vertical, so the
/// move writes nothing and the tangent keeps its value.
pub const MIN_TANGENT_DX: f32 = 0.05;

/// Which end of a key a tangent handle belongs to: `In`
/// carries the key's `in_tan` (the slope of the curve
/// arriving at the key, governing the segment that ENDS
/// there); `Out` carries its `out_tan` (the slope leaving
/// it, governing the segment that STARTS there).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TangentSide {
    In,
    Out,
}

impl TangentSide {
    /// Lower-case name for status lines ("in" / "out").
    fn label(self) -> &'static str {
        match self {
            TangentSide::In => "in",
            TangentSide::Out => "out",
        }
    }
}

/// Panel-local view state: which channel the user picked in
/// the selector, plus the in-progress canvas drags.
/// Everything else the panel shows is derived from
/// `EditorCore` per frame.
#[derive(Clone, Debug, Default)]
pub struct GraphPanelState {
    /// The last selector pick. `None` — and any pick that no
    /// longer names a selector entry (figure switched, node
    /// deselected) — falls back to [`effective_target`]'s
    /// automatic choice, so the panel never draws a channel
    /// the selector doesn't offer.
    pub target: Option<ChannelTarget>,
    /// The channel-list row pick, as an index into
    /// `Project::channels` (Phase J). A `(figure, target)`
    /// pair cannot tell two duplicate channels apart —
    /// [`channel_index`] resolves the FIRST match, so a
    /// target-only selection could never draw or edit the
    /// second duplicate. A row press therefore records the
    /// row's index here (and keeps [`Self::target`] in sync
    /// with that channel's target, so every consumer that
    /// still reads the target sees the same channel's pair).
    ///
    /// The index is honoured by
    /// [`effective_channel_index`] only while it names a
    /// live channel OF THE SELECTED FIGURE — the same
    /// scoping [`channel_index`] has always applied. A
    /// channel deleted elsewhere, or a figure switch that
    /// leaves the index pointing at another figure's
    /// channel, makes it stale: resolution falls back to
    /// the legacy target path and nothing panics. Chip
    /// presses clear it (a chip names a target, not a row),
    /// so the target takes over again.
    pub selected_channel: Option<usize>,
    /// Index into the drawn channel's `keys` of the key an
    /// in-progress canvas drag grabbed at Down. `None`
    /// outside a key drag.
    ///
    /// STALE-INDEX RISK, and the mitigation: `drag_key_to`
    /// re-sorts the channel after every Move, so the grabbed
    /// key's index can change mid-drag (dragging a key past
    /// its neighbour permutes both). The index is therefore
    /// never trusted across a Move on its own: it is only a
    /// hint, re-resolved on every Move by
    /// [`resolve_drag_index`] against [`Self::drag_anchor`]
    /// (the grabbed key's frame), and refreshed to the key's
    /// post-sort index after each move. Two keys sitting at
    /// the EXACT same frame and value are interchangeable to
    /// the resolver — degenerate duplicate data; the curve
    /// they produce is identical either way, though a later
    /// move could in principle continue with the twin (they
    /// may carry different tangents).
    pub drag_key: Option<usize>,
    /// The grabbed key's frame as of Down / the last Move —
    /// the drag's identity anchor for [`resolve_drag_index`].
    /// Meaningless while `drag_key` is `None`.
    pub drag_anchor: f32,
    /// The tangent handle an in-progress canvas drag grabbed
    /// at Down: `(key index, side)` into the drawn channel.
    /// `None` outside a tangent drag; mutually exclusive
    /// with [`Self::drag_key`] (Down sets exactly one).
    ///
    /// Unlike `drag_key`, the index needs NO anchor: a
    /// tangent drag never changes a key's frame or value, so
    /// the channel never re-sorts mid-gesture and the index
    /// recorded at Down stays the grabbed key for the whole
    /// drag.
    pub drag_handle: Option<(usize, TangentSide)>,
}

/// Human label for a channel target, in UI casing: "Root X",
/// "Root Y", "Angle 3", "Length 3", "Bend 3". (The engine
/// view-model's own legend labels are lower-case and
/// private; the panel owns its casing.)
pub fn channel_label(target: ChannelTarget) -> String {
    match target {
        ChannelTarget::RootX => "Root X".to_string(),
        ChannelTarget::RootY => "Root Y".to_string(),
        ChannelTarget::Angle(n) => format!("Angle {n}"),
        ChannelTarget::Length(n) => format!("Length {n}"),
        ChannelTarget::Bend(n) => format!("Bend {n}"),
    }
}

/// The selector's entries for the selected figure, in display
/// order: Root X and Root Y always; the selected node's Angle
/// and Bend when a node is selected; then every other target
/// the figure actually has a channel for, in project order,
/// without duplicates. Root/length channels appear even
/// before they have keys only via the pinned entries — keying
/// a root channel has no core op (see the module docs), so an
/// unpinned target is offered only once a channel exists.
/// Bend is pinned like Angle because this panel CAN author
/// its first key ([`key_bend`], the "Key bend" button).
pub fn selector_entries(core: &EditorCore) -> Vec<ChannelTarget> {
    let mut out = vec![ChannelTarget::RootX, ChannelTarget::RootY];
    if let Some(ni) = core.selected_node {
        out.push(ChannelTarget::Angle(ni));
        out.push(ChannelTarget::Bend(ni));
    }
    for ch in &core.project.channels {
        if ch.figure == core.selected_figure && !out.contains(&ch.target) {
            out.push(ch.target);
        }
    }
    out
}

/// The channel the panel draws and the transform actions
/// edit: the user's pick when it is still a selector entry;
/// otherwise the figure's first real channel; otherwise the
/// selected node's Angle (the channel `key_graph_angle`
/// authors, so the empty state points at a channel the user
/// can create from this panel); otherwise Root X.
pub fn effective_target(core: &EditorCore, sel: Option<ChannelTarget>) -> ChannelTarget {
    if let Some(t) = sel {
        if selector_entries(core).contains(&t) {
            return t;
        }
    }
    if let Some(ch) = core
        .project
        .channels
        .iter()
        .find(|ch| ch.figure == core.selected_figure)
    {
        return ch.target;
    }
    if let Some(ni) = core.selected_node {
        return ChannelTarget::Angle(ni);
    }
    ChannelTarget::RootX
}

/// Index into `core.project.channels` of the SELECTED
/// FIGURE's channel for `target`, if one exists.
pub fn channel_index(core: &EditorCore, target: ChannelTarget) -> Option<usize> {
    core.project
        .channels
        .iter()
        .position(|ch| ch.figure == core.selected_figure && ch.target == target)
}

/// The selected figure's channel for `target`, if one exists.
pub fn channel_for<'a>(core: &'a EditorCore, target: ChannelTarget) -> Option<&'a Channel> {
    channel_index(core, target).map(|i| &core.project.channels[i])
}

/// The channel the panel actually draws and edits, as an
/// index into `core.project.channels` (Phase J): the row
/// pick [`GraphPanelState::selected_channel`] when it is
/// still valid — it names a live channel of the selected
/// figure — and the legacy resolution otherwise
/// ([`channel_index`] of the [`effective_target`], i.e. the
/// FIRST `(figure, target)` match, exactly as before this
/// state existed). `None` when neither path finds a
/// channel (no channels, or none for the effective
/// target). This is the single resolution the builder's
/// drawn channel, the channel list's active row, the stats
/// block's highlight and the canvas drags all share, so
/// they can never disagree about which channel is live —
/// and with duplicate `(figure, target)` channels, a row
/// press on the second one makes THIS return the second.
pub fn effective_channel_index(core: &EditorCore, sel: &GraphPanelState) -> Option<usize> {
    if let Some(i) = sel.selected_channel {
        if let Some(ch) = core.project.channels.get(i) {
            if ch.figure == core.selected_figure {
                return Some(i);
            }
        }
    }
    channel_index(core, effective_target(core, sel.target))
}

/// Duplicate the drawn channel onto the NEXT figure
/// (Phase J integrator wiring for the graph_keys op):
/// resolves through [`effective_channel_index`] — a picked
/// duplicate row duplicates THAT twin — and forwards to
/// `graph_keys::duplicate_channel_to_figure`, which owns
/// the snapshot and every refusal (same figure, existing
/// channel, node the target rig lacks). With fewer than
/// two figures there is nowhere to duplicate onto, so the
/// press is a status-only refusal here.
pub fn duplicate_channel_to_next(core: &mut EditorCore, sel: &GraphPanelState) {
    let Some(idx) = effective_channel_index(core, sel) else {
        core.status = "No graph channel selected to duplicate.".into();
        return;
    };
    let figures = core.project.figures.len();
    if figures < 2 {
        core.status = "Only one figure — nowhere to duplicate the channel onto.".into();
        return;
    }
    let from = core.project.channels[idx].figure;
    let to = (from + 1) % figures;
    stykanimate_rs::graph_keys::duplicate_channel_to_figure(core, idx, to);
}

/// Where a stored row pick points after the channel at
/// `deleted` is removed from the project list (Phase K):
/// no pick stays no pick; a pick BEFORE the deleted row
/// is unmoved; a pick ON the deleted row is cleared (the
/// panel falls back to target resolution, exactly as an
/// unpicked state does); a pick AFTER it shifts down one
/// row, still naming the same channel. Pure — the delete
/// handler below is its only caller, and the four cases
/// are pinned by its unit test.
pub fn selection_after_delete(selected: Option<usize>, deleted: usize) -> Option<usize> {
    match selected {
        None => None,
        Some(j) if j == deleted => None,
        Some(j) if j > deleted => Some(j - 1),
        some => some,
    }
}

/// Delete the drawn channel (Phase K): resolves through
/// [`effective_channel_index`] — a picked duplicate row
/// deletes THAT twin — and forwards to
/// `graph_keys::delete_channel`, which owns the snapshot
/// and the removal. On success the stored pick is
/// repaired through [`selection_after_delete`], so it
/// can never dangle past the end of the shorter list or
/// silently name the channel that slid into the deleted
/// row; `target` is left as-is, so a cleared pick falls
/// back to target resolution. With nothing drawn the
/// press is a status-only refusal, mirroring
/// [`duplicate_channel_to_next`].
pub fn delete_drawn_channel(core: &mut EditorCore, sel: &mut GraphPanelState) {
    let Some(idx) = effective_channel_index(core, sel) else {
        core.status = "No graph channel selected to delete.".into();
        return;
    };
    if stykanimate_rs::graph_keys::delete_channel(core, idx) {
        sel.selected_channel = selection_after_delete(sel.selected_channel, idx);
    }
}

/// Shift the drawn channel's keys in time (Phase M):
/// resolves through [`effective_channel_index`] — the
/// very helper [`delete_drawn_channel`] resolves
/// through, so the shift buttons and Delete can never
/// disagree about which channel they act on (a picked
/// duplicate row shifts THAT twin) — and forwards to
/// `graph_keys::shift_keys`, which owns the snapshot,
/// the success wording and every refusal, including the
/// frame-0 wall (the whole shift is refused, never
/// clamped per key). The pick needs no repair — no
/// channel is added or removed — so `sel` is shared,
/// like [`duplicate_channel_to_next`]. With nothing
/// drawn the press is a status-only refusal, mirroring
/// Delete's.
pub fn shift_drawn_channel(core: &mut EditorCore, sel: &GraphPanelState, delta: f32) {
    let Some(idx) = effective_channel_index(core, sel) else {
        core.status = "No graph channel selected to shift.".into();
        return;
    };
    stykanimate_rs::graph_keys::shift_keys(core, idx, delta);
}

/// The channel list's rows: EVERY channel in the project, in
/// project order, as `(index, label, key count)` — the index
/// is the channel's position in `core.project.channels` (the
/// row key's `graph-chan-{i}` and the value a row press hands
/// to [`select_channel`]); the label is "F{n} {target}" with
/// the 1-based figure number the panel header and the core's
/// status lines use, and the target named by the panel's own
/// [`channel_label`] (graph.rs has no display-name fn of its
/// own — its legend labels are lower-case and private). Pure
/// projection of project data: selection-independent, so the
/// list is identical whichever figure is selected.
pub fn channel_rows(core: &EditorCore) -> Vec<(usize, String, usize)> {
    core.project
        .channels
        .iter()
        .enumerate()
        .map(|(i, ch)| {
            (
                i,
                format!("F{} {}", ch.figure + 1, channel_label(ch.target)),
                ch.keys.len(),
            )
        })
        .collect()
}

/// The panel target a channel list row selects: the target
/// of the channel at `index` in `core.project.channels`, or
/// `None` when the index names no channel. This is the exact
/// value a row press writes into `GraphPanelState::target`
/// (via [`select_channel`], which also records the index
/// itself in `GraphPanelState::selected_channel`).
pub fn target_for_channel(core: &EditorCore, index: usize) -> Option<ChannelTarget> {
    core.project.channels.get(index).map(|ch| ch.target)
}

/// A channel list row press, factored out of the view for
/// testability: point the panel at the channel at `index` in
/// `core.project.channels`. The channel's figure becomes the
/// selected figure first (through `EditorCore::select_figure`
/// — the app's own figure switch — but only when it differs,
/// so a same-figure pick never clears the user's node
/// selection), then the pick is recorded BOTH ways:
/// `st.selected_channel` takes the row's index (Phase J —
/// the only state that can name a duplicate channel) and
/// `st.target` takes the channel's target, kept in sync so
/// target-reading consumers see the same channel's pair.
/// From there [`effective_channel_index`] resolves the pick
/// to exactly this channel — even when another channel
/// shares its `(figure, target)` — so the canvas, the stats
/// highlight and the drags operate on the listed channel.
/// No snapshot: selection is view state everywhere in this
/// app, and `select_figure` doesn't snapshot either. Returns
/// false — nothing changed, neither field written — when
/// `index` names no channel, or when the channel's figure
/// cannot be selected (a stale figure index in hand-edited
/// project data; `select_figure` bounds-checks and refuses).
pub fn select_channel(core: &mut EditorCore, st: &mut GraphPanelState, index: usize) -> bool {
    let Some(ch) = core.project.channels.get(index) else {
        return false;
    };
    let (figure, target) = (ch.figure, ch.target);
    if figure != core.selected_figure {
        core.select_figure(figure);
        if core.selected_figure != figure {
            return false;
        }
    }
    st.target = Some(target);
    st.selected_channel = Some(index);
    true
}

/// One stats-block line for `stat`:
/// "{figure_name} · {target_label}: {keys} keys,
/// {min}..{max}", values at one decimal. Frames
/// (`first_frame` / `last_frame`) are deliberately not
/// shown — key count and value range are the glanceable
/// part; the canvas shows the span. Pure formatting: the
/// caller supplies the stat (in the panel, from
/// `graph_keys::channel_stats`).
pub fn stat_line(stat: &ChannelStat) -> String {
    format!(
        "{} · {}: {} keys, {:.1}..{:.1}",
        stat.figure_name, stat.target_label, stat.keys, stat.min, stat.max
    )
}

/// The joint-limit suffix for one channel's readout
/// lines (Wave Y, Y12): `joint limits {min}..{max}`
/// when the channel targets `Angle(n)` and that node
/// in the channel's figure carries limits (Wave X,
/// P-4) — the range `apply_channels` clamps the
/// channel's values through at evaluation, which the
/// raw stat range and playhead value alone never
/// showed: a channel keyed to 170 on an elbow applies
/// as 160, and before this note nothing in the panel
/// said so. An open side reads "Off", the segment
/// inspector's own word for an unset bound. `None`
/// for every other target and for unlimited nodes,
/// so unlimited channels' lines stay byte-identical.
/// Bounds print at `{:.0}` in the panel's plain style
/// — its values carry no degree glyph.
pub fn joint_limit_note(core: &EditorCore, ch: &Channel) -> Option<String> {
    let ChannelTarget::Angle(ni) = ch.target else {
        return None;
    };
    let node = core
        .project
        .figures
        .get(ch.figure)
        .and_then(|f| f.def.nodes.get(ni))?;
    if !node.has_limits() {
        return None;
    }
    let side = |b: Option<f32>| match b {
        Some(v) => format!("{v:.0}"),
        None => "Off".to_string(),
    };
    Some(format!(
        "joint limits {}..{}",
        side(node.min_angle),
        side(node.max_angle)
    ))
}

/// The stats block's rows: one `(line, is_drawn)` per
/// channel in the project, in project order — the lines
/// from [`stat_line`] over `graph_keys::channel_stats`,
/// with [`joint_limit_note`]'s suffix appended for a
/// limited Angle channel (Wave Y, Y12).
/// `channel_stats` walks `core.project.channels` in order,
/// so a row's position IS its channel index, and the drawn
/// channel's row is the one at [`effective_channel_index`]
/// of the panel state — the exact resolution the channel
/// list's active row and the canvas use. With duplicate
/// `(figure, target)` channels, a row pick flags the
/// PICKED row (Phase J); a target-only state (no row pick)
/// still flags the first match, as before. A stale row
/// pick falls back the same way [`effective_channel_index`]
/// does, so a deleted channel never leaves a phantom
/// highlight — and when no channel resolves at all, no row
/// is flagged. Empty when the project has no channels (the
/// builder then shows no block).
pub fn stat_rows(core: &EditorCore, sel: &GraphPanelState) -> Vec<(String, bool)> {
    let drawn = effective_channel_index(core, sel);
    channel_stats(core)
        .iter()
        .enumerate()
        .map(|(i, stat)| {
            // Wave Y (Y12): a limited Angle channel's row
            // carries its joint-limit note after the raw
            // range — the row position IS the channel
            // index (see the doc above), so the channel
            // itself resolves directly.
            let mut line = stat_line(stat);
            if let Some(ch) = core.project.channels.get(i) {
                if let Some(note) = joint_limit_note(core, ch) {
                    line = format!("{line} · {note}");
                }
            }
            (line, drawn == Some(i))
        })
        .collect()
}

/// One RENDERED stats-block row (Wave 10): either a
/// channel's own line — carrying its channel index, so
/// the builder keys it `graph-stat-{index}` by CHANNEL
/// index exactly as the unbounded block keyed by
/// position (the two agree whenever nothing is folded)
/// — or the single summary line that stands for the
/// folded channels, carrying how many it stands for.
#[derive(Debug, Clone, PartialEq)]
pub enum StatDisplay {
    Line {
        index: usize,
        line: String,
        drawn: bool,
    },
    More {
        hidden: usize,
    },
}

/// The summary line standing for `hidden` folded
/// channels: the count, honestly, plus the way back to
/// the folded stats — a channel list row press draws
/// that channel ([`select_channel`]), and the drawn
/// channel's line is always rendered in full.
pub fn stats_more_line(hidden: usize) -> String {
    if hidden == 1 {
        "+1 more channel — pick it in the list above to see its stats".to_string()
    } else {
        format!("+{hidden} more channels — pick one in the list above to see its stats")
    }
}

/// The stats block's RENDERED rows (Wave 10): the bound
/// the builder paints, projected from [`stat_rows`] —
/// which stays the full, unfolded source every other
/// consumer reads. While the project has at most
/// [`STATS_MAX_LINES`] channels this is every row, in
/// project order, exactly as the block always rendered.
/// Past the cap the block holds at [`STATS_MAX_LINES`]
/// lines: the DRAWN channel's line (never folded — it
/// is the channel the canvas shows and the interp
/// control edits, the panel's primary channel since
/// Wave 4), the first `STATS_MAX_LINES - 2` other lines
/// in project order, and one [`StatDisplay::More`]
/// summary for the rest. Project order is preserved
/// among the shown lines, and nothing vanishes
/// silently: the summary counts the folded channels
/// and names the way to any of them (a list row press
/// draws it, which puts its line back in full).
pub fn stat_display_rows(core: &EditorCore, sel: &GraphPanelState) -> Vec<StatDisplay> {
    let rows = stat_rows(core, sel);
    let total = rows.len();
    if total <= STATS_MAX_LINES {
        return rows
            .into_iter()
            .enumerate()
            .map(|(index, (line, drawn))| StatDisplay::Line { index, line, drawn })
            .collect();
    }
    // Over the cap: one of the remaining slots is the
    // summary line, and the drawn channel's line is
    // never the one dropped, so the non-drawn budget
    // is the cap minus the drawn line minus the
    // summary — filled in project order.
    let mut out: Vec<StatDisplay> = Vec::new();
    let mut others = STATS_MAX_LINES - 2;
    for (index, (line, drawn)) in rows.into_iter().enumerate() {
        if drawn {
            out.push(StatDisplay::Line { index, line, drawn });
        } else if others > 0 {
            others -= 1;
            out.push(StatDisplay::Line { index, line, drawn });
        }
    }
    let hidden = total - out.len();
    out.push(StatDisplay::More { hidden });
    out
}

/// The drawn channel's value at the playhead, as the
/// panel's `graph-value` readout line (Phase L): "Value
/// at frame {current+1}: {v:.1}" — the frame 1-based, the
/// value the REAL evaluation (`Channel::value_at`) at the
/// current frame, to one decimal like the header
/// readout's tail. Resolves through
/// [`effective_channel_index`], so a picked duplicate
/// reports ITS value, never its twin's. `None` when no
/// channel is drawn (the builder then shows no line).
/// Wave Y (Y12): a limited Angle channel's line carries
/// [`joint_limit_note`]'s suffix after the raw value —
/// the value stays the channel's own; the note names
/// the range evaluation will clamp it into.
pub fn value_line(core: &EditorCore, sel: &GraphPanelState) -> Option<String> {
    let idx = effective_channel_index(core, sel)?;
    let ch = &core.project.channels[idx];
    let mut line = format!(
        "Value at frame {}: {:.1}",
        core.current + 1,
        ch.value_at(core.current as f32)
    );
    if let Some(note) = joint_limit_note(core, ch) {
        line = format!("{line} · {note}");
    }
    Some(line)
}

/// Index of the key nearest `frame` — the same pick
/// `EditorCore::set_graph_key_interp` makes (an exact hit
/// wins; ties go to the EARLIER key, because only a strictly
/// smaller distance replaces the incumbent).
pub fn nearest_key(ch: &Channel, frame: f32) -> Option<usize> {
    let mut best: Option<usize> = None;
    let mut best_dist = f32::INFINITY;
    for (i, k) in ch.keys.iter().enumerate() {
        let d = (k.frame - frame).abs();
        if d < best_dist {
            best_dist = d;
            best = Some(i);
        }
    }
    best
}

/// The interp mode the segmented control shows as active
/// when NO channel is drawn (the fallback read, UI mission
/// S12-08): the mode of the key nearest the current frame
/// on the selected figure's `Angle(selected_node)` channel
/// — exactly the key `set_graph_key_interp` would edit.
/// `None` (nothing highlighted) when there is no node, no
/// channel or no key. While a channel IS drawn,
/// [`drawn_interp`] is the read instead.
pub fn current_interp(core: &EditorCore) -> Option<Interp> {
    let ni = core.selected_node?;
    let ch = channel_for(core, ChannelTarget::Angle(ni))?;
    let ki = nearest_key(ch, core.current as f32)?;
    Some(ch.keys[ki].interp)
}

/// Index of the key of `ch` that sits AT `frame` — the
/// pick `graph_keys::key_interp_at` reads: the nearest
/// key within half a frame, ties to the earlier key (only
/// a strictly smaller distance replaces the incumbent).
/// `None` when no key lies inside the window — a frame
/// between keys two or more frames apart names no key.
/// This is the write-side twin of the library's read, so
/// the segmented control's press edits exactly the key
/// its highlight reads (the keyframe tools' interp cycle
/// keeps the same agreement by hand).
pub fn key_index_at(ch: &Channel, frame: f32) -> Option<usize> {
    let mut best: Option<usize> = None;
    let mut best_dist = f32::INFINITY;
    for (i, k) in ch.keys.iter().enumerate() {
        let d = (k.frame - frame).abs();
        if d <= 0.5 && d < best_dist {
            best_dist = d;
            best = Some(i);
        }
    }
    best
}

/// The interp mode the segmented control shows as active
/// while a channel is DRAWN (UI mission S12-08): the mode
/// of the drawn channel's key at the current frame, read
/// through the library's `graph_keys::key_interp_at` over
/// [`effective_channel_index`] — the same resolution the
/// canvas draws by, so the highlight can never name a
/// key on a channel that is not on screen. `None`
/// (nothing highlighted, and a press refuses with a
/// status) when the channel is empty or no key sits at
/// the playhead.
pub fn drawn_interp(core: &EditorCore, sel: &GraphPanelState) -> Option<Interp> {
    let idx = effective_channel_index(core, sel)?;
    stykanimate_rs::graph_keys::key_interp_at(core, idx, core.current as f32)
}

/// The interp segmented control's press (UI mission
/// S12-08): set the mode of the DRAWN channel's key at
/// the current frame through the library's
/// `graph_keys::set_key_interp` — the op MCP and the
/// keyframe tools' Cycle already use — so the control
/// edits the curve it sits under. The key is the one
/// [`key_index_at`] names, exactly the key
/// [`drawn_interp`]'s highlight reads; with no key at
/// the playhead the press is a status-only refusal (no
/// snapshot, nothing edited). When NO channel is drawn
/// there is no curve to edit, and the press falls back
/// to the panel's original path,
/// `EditorCore::set_graph_key_interp` on the selected
/// joint's angle channel, whose own refusals cover the
/// no-joint / no-channel states.
pub fn set_drawn_interp(core: &mut EditorCore, sel: &GraphPanelState, mode: Interp) {
    let Some(idx) = effective_channel_index(core, sel) else {
        core.set_graph_key_interp(mode);
        return;
    };
    let Some(ki) = key_index_at(&core.project.channels[idx], core.current as f32) else {
        core.status = format!("No key at frame {} to set interp on.", core.current + 1);
        return;
    };
    stykanimate_rs::graph_keys::set_key_interp(core, idx, ki, mode);
}

/// The drawn curve as pixel points: `steps + 1` samples of
/// the REAL evaluation (`Channel::value_at` via
/// `GraphView::curve_points`) across the view's frame range.
/// Kept separate from the `Shape` assembly so tests can
/// assert on plain data.
pub fn sample_curve(view: &GraphView, ch: &Channel, steps: usize) -> Vec<V2> {
    view.curve_points(ch, steps)
}

/// A key diamond: four points (top, right, bottom, left) of
/// radius `r` around `centre`, ready for `Shape::fill`.
pub fn diamond(centre: V2, r: f32) -> [V2; 4] {
    [
        [centre[0], centre[1] - r],
        [centre[0] + r, centre[1]],
        [centre[0], centre[1] + r],
        [centre[0] - r, centre[1]],
    ]
}

/// A tangent preset's preview curve as pixel points in a
/// `w` x `h` box: `preset_preview`'s normalised 0.0..=1.0
/// samples with y flipped (value 1.0 at the TOP, screen
/// convention), x spread evenly edge to edge.
pub fn preview_points(preset: TangentPreset, w: f32, h: f32, samples: usize) -> Vec<V2> {
    let values = preset_preview(preset, samples);
    let n = values.len();
    values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let fx = if n > 1 {
                i as f32 / (n - 1) as f32
            } else {
                0.0
            };
            [fx * w, h * (1.0 - v)]
        })
        .collect()
}

/// Key the selected node's bend on its graph channel at the
/// current frame — the "Key bend" button (Wave Y, P-7).
/// There is no core `key_graph_bend` op, so this follows the
/// panel-side precedent of the `apply_*` family below, and
/// mirrors `EditorCore::key_graph_angle`'s semantics step
/// for step: no selected joint is a status-only no-op (no
/// snapshot); otherwise ONE `core.snapshot()`, the
/// `(figure, Bend(node))` channel is upserted through
/// `graph::channel_mut`, and the key lands through
/// `Channel::add_key` (which replaces a key at the same
/// frame and keeps the channel sorted) — one undo step.
///
/// The keyed value is the bend IN FORCE at the playhead:
/// when a Bend channel already exists it overrides the pose
/// bend at evaluation, so its own `value_at` is keyed —
/// keying the pose value instead would move the curve.
/// Without a channel, the pose bend
/// (`Project::bends_at`, 0.0 when the project carries no
/// bend data) is keyed, exactly as `key_graph_angle` keys
/// the pose angle. Bend values are radians throughout —
/// the `bends_at` convention Y1's channel evaluation reads.
pub fn key_bend(core: &mut EditorCore) {
    let Some(ni) = core.selected_node else {
        core.status = "Select a joint first, then Key bend.".into();
        return;
    };
    let fi = core.selected_figure;
    let frame = core.current as f32;
    let target = ChannelTarget::Bend(ni);
    let value = match channel_for(core, target) {
        Some(ch) => ch.value_at(frame),
        None => core
            .project
            .bends_at(frame, fi)
            .get(ni)
            .copied()
            .unwrap_or(0.0),
    };
    core.snapshot();
    let ch = stykanimate_rs::graph::channel_mut(&mut core.project.channels, fi, target);
    ch.add_key(frame, value);
    core.status = format!(
        "Graph key: figure {} joint {} bend = {value:.2} rad @ frame {}.",
        fi + 1,
        ni,
        core.current + 1
    );
}

/// Mirror the `(selected figure, target)` channel in time
/// around the timeline's middle frame — the MCP
/// `graph_mirror_channel_time` behaviour, default centre
/// included. No such channel: status only, no snapshot.
/// The target resolves through [`channel_index`] (FIRST
/// match); [`apply_mirror_at`] is the index form the panel
/// uses when a specific row is picked, so a duplicate
/// channel can be mirrored too (Phase J).
pub fn apply_mirror(core: &mut EditorCore, target: ChannelTarget) {
    let Some(idx) = channel_index(core, target) else {
        core.status = format!(
            "No {} channel for figure {} to mirror.",
            channel_label(target),
            core.selected_figure + 1
        );
        return;
    };
    apply_mirror_at(core, idx);
}

/// Mirror the channel AT `idx` in `core.project.channels`
/// in time around the timeline's middle frame — the index
/// form of [`apply_mirror`]. A stale index (no channel
/// there): status only, no snapshot.
pub fn apply_mirror_at(core: &mut EditorCore, idx: usize) {
    if core.project.channels.get(idx).is_none() {
        core.status = "No graph channel to mirror.".to_string();
        return;
    }
    let centre = (core.project.frames.len().saturating_sub(1)) as f32 / 2.0;
    core.snapshot();
    let ch = &mut core.project.channels[idx];
    stykanimate_rs::graph::mirror_keys_time(&mut ch.keys, centre);
    // Mirroring reverses the key order; restore ascending
    // order (the MCP tool does the same after the call).
    ch.sort_keys();
    core.status = format!("Mirrored graph channel time around frame {centre}.");
}

/// Scale the `(selected figure, target)` channel's values by
/// `factor` around 0 — the MCP `graph_scale_channel_values`
/// behaviour with its default pivot. No such channel: status
/// only, no snapshot. The target resolves through
/// [`channel_index`] (FIRST match); [`apply_scale_at`] is
/// the index form for a picked row (Phase J).
pub fn apply_scale(core: &mut EditorCore, target: ChannelTarget, factor: f32) {
    let Some(idx) = channel_index(core, target) else {
        core.status = format!(
            "No {} channel for figure {} to scale.",
            channel_label(target),
            core.selected_figure + 1
        );
        return;
    };
    apply_scale_at(core, idx, factor);
}

/// Scale the channel AT `idx`'s values by `factor` around
/// 0 — the index form of [`apply_scale`]. A stale index:
/// status only, no snapshot.
pub fn apply_scale_at(core: &mut EditorCore, idx: usize, factor: f32) {
    if core.project.channels.get(idx).is_none() {
        core.status = "No graph channel to scale.".to_string();
        return;
    }
    core.snapshot();
    let ch = &mut core.project.channels[idx];
    stykanimate_rs::graph::scale_key_values(&mut ch.keys, factor, 0.0);
    core.status = format!("Scaled graph channel values by {factor} around 0.");
}

/// Reduce the `(selected figure, target)` channel with
/// [`REDUCE_TOLERANCE`] (`graph::reduce_keys` drops interior
/// keys that contribute at most the tolerance and always
/// keeps the endpoints). No such channel: status only, no
/// snapshot. The target resolves through [`channel_index`]
/// (FIRST match); [`apply_reduce_at`] is the index form
/// for a picked row (Phase J).
pub fn apply_reduce(core: &mut EditorCore, target: ChannelTarget) {
    let Some(idx) = channel_index(core, target) else {
        core.status = format!(
            "No {} channel for figure {} to reduce.",
            channel_label(target),
            core.selected_figure + 1
        );
        return;
    };
    apply_reduce_at(core, idx);
}

/// Reduce the channel AT `idx` with [`REDUCE_TOLERANCE`] —
/// the index form of [`apply_reduce`]. A stale index:
/// status only, no snapshot.
pub fn apply_reduce_at(core: &mut EditorCore, idx: usize) {
    if core.project.channels.get(idx).is_none() {
        core.status = "No graph channel to reduce.".to_string();
        return;
    }
    core.snapshot();
    let removed = stykanimate_rs::graph::reduce_keys(
        &mut core.project.channels[idx],
        REDUCE_TOLERANCE,
    );
    core.status = format!("Reduced graph channel: removed {removed} key(s).");
}

/// The fit the panel draws AND drags with, for a canvas of
/// `size`: `GraphView::auto_fit` over the drawn channel alone
/// (an angle channel gets the full -180..=180 circle, a value
/// channel its keys with 10% padding; no channel fits the
/// bare timeline). [`graph_shape`] builds its view through
/// here and so does every drag phase, recomputed per phase
/// from the then-current project — pointer math and pixels
/// can never be based on different transforms.
pub fn drag_view(core: &EditorCore, target: ChannelTarget, size: [f32; 2]) -> GraphView {
    view_for(core, channel_for(core, target), size)
}

/// The fit over ONE specific channel (or none): the
/// index-based sibling of [`drag_view`], used wherever the
/// drawn channel was resolved by index
/// ([`effective_channel_index`]) — with duplicate
/// `(figure, target)` channels, fitting the channel the
/// target happens to resolve to would fit the WRONG twin,
/// so the drawing and the drags fit the picked channel
/// itself.
pub fn view_for(core: &EditorCore, ch: Option<&Channel>, size: [f32; 2]) -> GraphView {
    let fit: &[Channel] = match ch {
        Some(c) => std::slice::from_ref(c),
        None => &[],
    };
    GraphView::auto_fit(fit, core.frame_count(), size[0], size[1])
}

/// Resolve a drag's grabbed key under the channel's CURRENT
/// key order. `hint` is the index recorded at Down / the last
/// Move and `anchor` the grabbed key's frame then. The hint
/// wins when the key sitting there still has the anchor
/// frame (the common no-crossing case); otherwise the key
/// nearest the anchor is the grabbed one (a re-sort moved
/// it). `None` when the channel has no keys at all.
pub fn resolve_drag_index(ch: &Channel, hint: usize, anchor: f32) -> Option<usize> {
    if let Some(k) = ch.keys.get(hint) {
        if (k.frame - anchor).abs() <= 1e-4 {
            return Some(hint);
        }
    }
    nearest_key(ch, anchor)
}

/// Move key `idx` of `ch` to (`frame`, `value`) — frame
/// clamped to `>= 0.0`, value unclamped — and re-sort the
/// channel. Returns the key's NEW index after the sort (the
/// sort can permute it past a crossed neighbour), re-found
/// by its exact frame+value with a nearest-frame fallback,
/// or `None` when `idx` is out of range (channel untouched).
pub fn drag_key_to(ch: &mut Channel, idx: usize, frame: f32, value: f32) -> Option<usize> {
    let k = ch.keys.get_mut(idx)?;
    k.frame = frame.max(0.0);
    k.value = value;
    let (f, v) = (k.frame, k.value);
    ch.sort_keys();
    ch.keys
        .iter()
        .position(|k| k.frame == f && k.value == v)
        .or_else(|| nearest_key(ch, f))
}

/// The panel's selected key on the drawn channel: the key
/// nearest the current frame (ties go to the earlier key,
/// via [`nearest_key`]). This is the key whose tangent
/// handles are drawn and grabbable. (Distinct from the
/// interp control's at-the-playhead pick — see
/// [`key_index_at`].)
pub fn selected_key_index(core: &EditorCore, target: ChannelTarget) -> Option<usize> {
    let ch = channel_for(core, target)?;
    nearest_key(ch, core.current as f32)
}

/// Data position of key `idx`'s `side` tangent handle: on
/// the key's tangent ray, [`TANGENT_HANDLE_FRAC`] of the way
/// toward the neighbouring key on that side — the cubic
/// Bézier control point of the segment that side governs.
/// `None` when that side governs no segment (the first
/// key's In, the last key's Out), when the neighbour sits
/// at a non-increasing frame (degenerate span), or when
/// `idx` is out of range.
pub fn tangent_handle_data(ch: &Channel, idx: usize, side: TangentSide) -> Option<(f32, f32)> {
    let k = ch.keys.get(idx)?;
    match side {
        TangentSide::Out => {
            let next = ch.keys.get(idx + 1)?;
            let span = next.frame - k.frame;
            if span <= 0.0 {
                return None;
            }
            Some((
                k.frame + TANGENT_HANDLE_FRAC * span,
                k.value + TANGENT_HANDLE_FRAC * span * k.out_tan,
            ))
        }
        TangentSide::In => {
            if idx == 0 {
                return None;
            }
            let prev = &ch.keys[idx - 1];
            let span = k.frame - prev.frame;
            if span <= 0.0 {
                return None;
            }
            Some((
                k.frame - TANGENT_HANDLE_FRAC * span,
                k.value - TANGENT_HANDLE_FRAC * span * k.in_tan,
            ))
        }
    }
}

/// Hit-test key `key_idx`'s tangent handles against pixel
/// `px`, in DATA space: the pointer converts through `view`
/// once, and each handle's data position is compared with
/// per-axis tolerances — `radius` pixels expressed in frames
/// and in values through the view's own scale, so the
/// elliptical pick region means the same thing on screen
/// despite the two axes' different units. The nearer handle
/// (normalised distance) wins; `None` when neither side has
/// a handle, both lie outside the region, or the view is
/// degenerate.
pub fn hit_tangent_handle(
    view: &GraphView,
    ch: &Channel,
    key_idx: usize,
    px: V2,
    radius: f32,
) -> Option<TangentSide> {
    if view.width <= 0.0 || view.height <= 0.0 {
        return None;
    }
    let tol_f = radius / view.width * (view.frame_max - view.frame_min).abs();
    let tol_v = radius / view.height * (view.val_max - view.val_min).abs();
    if tol_f <= 0.0 || tol_v <= 0.0 {
        return None;
    }
    let (df, dv) = view.to_data(px);
    let mut best: Option<(TangentSide, f32)> = None;
    for side in [TangentSide::In, TangentSide::Out] {
        let Some((hf, hv)) = tangent_handle_data(ch, key_idx, side) else {
            continue;
        };
        let nf = (df - hf) / tol_f;
        let nv = (dv - hv) / tol_v;
        let d2 = nf * nf + nv * nv;
        if d2 <= 1.0 && best.map_or(true, |(_, b)| d2 < b) {
            best = Some((side, d2));
        }
    }
    best.map(|(side, _)| side)
}

/// The slope a tangent drag assigns from a pointer at data
/// `data`: the slope of the line through the key and the
/// pointer — one formula for both sides, because the In and
/// Out rays are the same line through the key. `None` when
/// the pointer sits within [`MIN_TANGENT_DX`] frames of the
/// key's frame, where the slope would explode toward
/// vertical: the caller keeps the stored tangent for that
/// move instead of writing garbage.
pub fn tangent_slope(key: &Key, data: (f32, f32)) -> Option<f32> {
    let dx = data.0 - key.frame;
    if dx.abs() < MIN_TANGENT_DX {
        None
    } else {
        Some((data.1 - key.value) / dx)
    }
}

/// Write the tangent `side` of key `idx` from a pointer at
/// data `data` ([`tangent_slope`]) — ONLY that side: the
/// model's tangents are per-side (broken) by design (see
/// the module docs). The other side, the key's frame and
/// value, and the key order are all untouched, so no
/// re-sort is needed or done. Returns whether a tangent was
/// written (false: index out of range, or the pointer too
/// close to the key's frame to read a slope from).
pub fn drag_tangent_to(ch: &mut Channel, idx: usize, side: TangentSide, data: (f32, f32)) -> bool {
    let Some(k) = ch.keys.get_mut(idx) else {
        return false;
    };
    let Some(slope) = tangent_slope(k, data) else {
        return false;
    };
    match side {
        TangentSide::In => k.in_tan = slope,
        TangentSide::Out => k.out_tan = slope,
    }
    true
}

/// The frame a click at pixel `x` seeks to: the view's data
/// frame at `x`, rounded to the nearest whole frame (halves
/// round away from zero, `f32::round` semantics) and clamped
/// into the timeline. `None` when there are no frames.
pub fn seek_frame(view: &GraphView, x: f32, frame_count: usize) -> Option<usize> {
    if frame_count == 0 {
        return None;
    }
    let (frame, _) = view.to_data([x, 0.0]);
    let f = frame.round().clamp(0.0, (frame_count - 1) as f32);
    Some(f as usize)
}

/// Canvas drag, Down phase: hit-test the drawn channel at
/// `px` (a canvas of `size`) through the panel's own fit —
/// the selected key's tangent handles first (they win over
/// the key hit-test), then the key handles themselves. On a
/// hit: snapshot ONCE for the whole gesture, grab into `st`,
/// return true. On a miss nothing is recorded — whether the
/// gesture becomes a seek is decided at Up from its travel.
pub fn drag_down(core: &mut EditorCore, st: &mut GraphPanelState, px: V2, size: [f32; 2]) -> bool {
    if size[0] <= 0.0 || size[1] <= 0.0 {
        return false;
    }
    // The drawn channel by INDEX (Phase J): a picked
    // duplicate is the one whose keys and handles are live.
    let Some(ci) = effective_channel_index(core, st) else {
        return false;
    };
    let view = view_for(core, Some(&core.project.channels[ci]), size);
    if let Some(sel) = nearest_key(&core.project.channels[ci], core.current as f32) {
        if let Some(side) =
            hit_tangent_handle(&view, &core.project.channels[ci], sel, px, GRAB_RADIUS)
        {
            core.snapshot();
            st.drag_handle = Some((sel, side));
            st.drag_key = None;
            return true;
        }
    }
    let hit = view.hit_key(&core.project.channels[ci], px, GRAB_RADIUS);
    match hit {
        Some(i) => {
            core.snapshot();
            st.drag_anchor = core.project.channels[ci].keys[i].frame;
            st.drag_key = Some(i);
            st.drag_handle = None;
            true
        }
        None => false,
    }
}

/// Canvas drag, Move phase: while a tangent handle is
/// grabbed, rewrite that side's slope from the pointer's
/// data position; while a key is grabbed, move it to the
/// pointer's data position (converted through the fit as of
/// this Move's start) and re-anchor the drag state to its
/// post-sort index. No snapshot either way — the Down
/// snapshot covers the whole gesture. Returns true when
/// something was written; a stale grab (channel gone, key
/// gone) ends the drag.
pub fn drag_move(core: &mut EditorCore, st: &mut GraphPanelState, px: V2, size: [f32; 2]) -> bool {
    if let Some((idx, side)) = st.drag_handle {
        if size[0] <= 0.0 || size[1] <= 0.0 {
            return false;
        }
        let Some(ci) = effective_channel_index(core, st) else {
            st.drag_handle = None;
            return false;
        };
        if core.project.channels[ci].keys.get(idx).is_none() {
            st.drag_handle = None;
            return false;
        }
        let view = view_for(core, Some(&core.project.channels[ci]), size);
        let data = view.to_data(px);
        return drag_tangent_to(&mut core.project.channels[ci], idx, side, data);
    }
    let Some(hint) = st.drag_key else {
        return false;
    };
    if size[0] <= 0.0 || size[1] <= 0.0 {
        return false;
    }
    let Some(ci) = effective_channel_index(core, st) else {
        st.drag_key = None;
        return false;
    };
    let idx = resolve_drag_index(&core.project.channels[ci], hint, st.drag_anchor);
    let Some(idx) = idx else {
        st.drag_key = None;
        return false;
    };
    let view = view_for(core, Some(&core.project.channels[ci]), size);
    let (frame, value) = view.to_data(px);
    let ch = &mut core.project.channels[ci];
    match drag_key_to(ch, idx, frame, value) {
        Some(new_idx) => {
            st.drag_key = Some(new_idx);
            st.drag_anchor = ch.keys[new_idx].frame;
            true
        }
        None => {
            st.drag_key = None;
            false
        }
    }
}

/// Canvas drag, Up phase. With a tangent handle grabbed:
/// end the drag — the final slope goes to `core.status`, or,
/// when the runtime cancelled the gesture (`cancelled`), the
/// Down snapshot is rolled back with one `core.undo()`. With
/// a key grabbed: the same discipline on the key's final
/// position. With neither grabbed: a click (travel within
/// [`CLICK_TRAVEL`]) seeks the playhead to the frame under
/// the pointer via `core.goto`; a longer drag over empty
/// canvas does nothing. Returns true when the gesture
/// changed anything.
pub fn drag_up(
    core: &mut EditorCore,
    st: &mut GraphPanelState,
    px: V2,
    size: [f32; 2],
    travel: f32,
    cancelled: bool,
) -> bool {
    if let Some((idx, side)) = st.drag_handle.take() {
        if cancelled {
            core.undo();
            return true;
        }
        if let Some(ci) = effective_channel_index(core, st) {
            if let Some(k) = core.project.channels[ci].keys.get(idx) {
                let slope = match side {
                    TangentSide::In => k.in_tan,
                    TangentSide::Out => k.out_tan,
                };
                core.status = format!(
                    "Graph {} tangent set: key frame {:.1}, slope {:.2}.",
                    side.label(),
                    k.frame,
                    slope
                );
            }
        }
        return true;
    }
    if let Some(hint) = st.drag_key.take() {
        if cancelled {
            core.undo();
            return true;
        }
        if let Some(ci) = effective_channel_index(core, st) {
            let ch = &core.project.channels[ci];
            if let Some(idx) = resolve_drag_index(ch, hint, st.drag_anchor) {
                let k = ch.keys[idx];
                core.status = format!(
                    "Graph key moved: frame {:.1}, value {:.1}.",
                    k.frame, k.value
                );
            }
        }
        return true;
    }
    if cancelled || travel > CLICK_TRAVEL || size[0] <= 0.0 {
        return false;
    }
    // The seek fits the drawn channel too (when there is
    // one), so the click reads frames off the same fit the
    // curve was drawn with.
    let view = match effective_channel_index(core, st) {
        Some(ci) => view_for(core, Some(&core.project.channels[ci]), size),
        None => drag_view(core, effective_target(core, st.target), size),
    };
    match seek_frame(&view, px[0], core.frame_count()) {
        Some(f) => {
            core.goto(f);
            true
        }
        None => false,
    }
}

/// The canvas element's drag handler: element-local points
/// and the element's own drawn size go straight to the
/// phase helpers (pointer capture is Rune's — a drag that
/// leaves the canvas still reports its Moves and Up here).
fn graph_drag(s: &mut State, d: &mut Drag) {
    let px = [d.x, d.y];
    let size = [d.w, d.h];
    match d.phase {
        Phase::Down => {
            drag_down(&mut s.core, &mut s.graph_panel, px, size);
        }
        Phase::Move => {
            drag_move(&mut s.core, &mut s.graph_panel, px, size);
        }
        Phase::Up => {
            let travel =
                ((d.x - d.start_x).powi(2) + (d.y - d.start_y).powi(2)).sqrt();
            drag_up(
                &mut s.core,
                &mut s.graph_panel,
                px,
                size,
                travel,
                d.cancelled(),
            );
        }
    }
}

/// The curve canvas as one `Shape`: the zero line (when the
/// fitted range crosses zero), the current-frame line, the
/// sampled curve, a diamond per key, the selected key's
/// tangent handles (a thin line from the key to a small
/// diamond at each side's [`tangent_handle_data`] position;
/// the dragged side in the accent colour), and an accent
/// diamond on the curve at the current frame. The view is
/// `GraphView::auto_fit` over the drawn channel alone, so an
/// angle channel gets the full -180..=180 circle and a value
/// channel gets its keys with 10% padding. `drag` is the
/// grabbed key's index (resolved for the CURRENT key order),
/// drawn larger in the accent colour while it is dragged;
/// `sel` is the selected key (see [`selected_key_index`])
/// and `drag_handle` the tangent drag in progress, if any.
/// `idx` is the drawn channel's index
/// ([`effective_channel_index`]) when the builder resolved
/// one: the channel drawn is the channel AT that index —
/// with duplicate `(figure, target)` channels, the picked
/// twin, not the target's first match (Phase J). `target`
/// remains the fallback resolution for the no-index case.
fn graph_shape(
    core: &EditorCore,
    target: ChannelTarget,
    size: [f32; 2],
    t: &Theme,
    drag: Option<usize>,
    sel: Option<usize>,
    drag_handle: Option<(usize, TangentSide)>,
    idx: Option<usize>,
) -> Shape {
    let (w, h) = (size[0], size[1]);
    let accent = tok::accent(0, true);
    let ch = idx
        .and_then(|i| core.project.channels.get(i))
        .or_else(|| channel_for(core, target));
    let view = view_for(core, ch, size);
    let mut shape = Shape::new();
    if view.val_min < 0.0 && view.val_max > 0.0 {
        let y = view.to_px(0.0, 0.0)[1];
        shape = shape.line([0.0, y], [w, y], 1.0, LineCap::Round, t.text.alpha(0.18));
    }
    let now_x = view.to_px(core.current as f32, 0.0)[0];
    shape = shape.line(
        [now_x, 0.0],
        [now_x, h],
        1.0,
        LineCap::Round,
        accent.alpha(0.55),
    );
    if let Some(ch) = ch {
        let steps = (w as usize).clamp(48, 256);
        let pts = sample_curve(&view, ch, steps);
        if pts.len() > 1 {
            shape = shape.stroke(pts, 2.0, accent);
        }
        let amber = hex_color(0xFFC65D).unwrap_or(Color::WHITE);
        for (i, handle) in view.key_handles(ch).into_iter().enumerate() {
            shape = if drag == Some(i) {
                shape.fill(diamond(handle, 6.5).to_vec(), accent)
            } else {
                shape.fill(diamond(handle, 4.5).to_vec(), amber)
            };
        }
        // Tangent handles of the selected key only — one
        // pair per key at most, so the canvas stays readable
        // (and matches the hit-test, which only offers the
        // selected key's handles).
        if let Some(sel) = sel {
            if let Some(k) = ch.keys.get(sel) {
                let kp = view.to_px(k.frame, k.value);
                for side in [TangentSide::In, TangentSide::Out] {
                    if let Some((hf, hv)) = tangent_handle_data(ch, sel, side) {
                        let hp = view.to_px(hf, hv);
                        let col = if drag_handle == Some((sel, side)) {
                            accent
                        } else {
                            t.text_secondary
                        };
                        shape = shape.line(kp, hp, 1.0, LineCap::Round, col.alpha(0.6));
                        shape =
                            shape.fill(diamond(hp, TANGENT_HANDLE_RADIUS).to_vec(), col);
                    }
                }
            }
        }
        let now_pt = view.to_px(core.current as f32, ch.value_at(core.current as f32));
        shape = shape.fill(diamond(now_pt, 3.5).to_vec(), accent);
    }
    shape
}

/// The Graph panel: header + readout, channel selector,
/// curve canvas, interp segmented control, tangent presets
/// and the transform actions. Mounted by the shell wherever
/// the Graph workspace lives.
pub fn graph_panel(s: &State, cx: &mut Cx<State>, t: &Theme) -> El {
    let core = &s.core;
    // The drawn channel resolves by INDEX first (Phase J):
    // a picked duplicate row IS the drawn channel. The
    // target below is that channel's own target (its label
    // for the readout and the selector chips), or the
    // legacy effective target when no channel resolves.
    let drawn_idx = effective_channel_index(core, &s.graph_panel);
    let target = match drawn_idx {
        Some(i) => core.project.channels[i].target,
        None => effective_target(core, s.graph_panel.target),
    };
    let ch = drawn_idx.map(|i| &core.project.channels[i]);
    let size = cx.layout_size("graph-canvas").unwrap_or(FALLBACK_SIZE);

    let readout = match ch {
        Some(ch) => format!(
            "{} · {} keys · frame {} = {:.1}",
            channel_label(target),
            ch.keys.len(),
            core.current + 1,
            ch.value_at(core.current as f32)
        ),
        None => format!("{} · no keys yet", channel_label(target)),
    };

    // Channel selector: one chip per entry; the drawn channel
    // is the active one.
    let mut selector = row().key("graph-channels").gap(4.0).w_full().flex_wrap();
    for entry in selector_entries(core) {
        let label = channel_label(entry);
        let key = format!("graph-ch-{}", label.replace(' ', "-").to_lowercase());
        selector = selector.child(btn(
            &key,
            &label,
            entry == target,
            t,
            move |s: &mut State| {
                // A chip names a TARGET, not a row: clear
                // any row pick so the target resolution
                // takes over again (Phase J).
                s.graph_panel.selected_channel = None;
                s.graph_panel.target = Some(entry);
            },
        ));
    }

    // Channel list (Phase G): every channel in the project,
    // one pressable row per channel — "F{n} {target}" plus
    // its key count — recording the pick via select_channel
    // (which also selects the channel's figure when it
    // differs). The active row is the channel the panel
    // actually draws: `drawn_idx`, resolved by
    // effective_channel_index at the top of this builder —
    // a picked row highlights ITSELF, even when it is a
    // duplicate of the row above it (Phase J); a
    // target-only state highlights the first match, as
    // before. With no channels at all, the list is the
    // empty-state line naming how the first channel
    // appears (Key angle — key_graph_angle creates it; see
    // the module docs).
    let mut chan_list = col().key("graph-channel-list").gap(4.0);
    let rows = channel_rows(core);
    if rows.is_empty() {
        chan_list = chan_list.child(design::empty_state(
            "chart-line",
            "No graph channels yet — select a joint on the stage, then Key angle starts the first one.",
            t,
        ));
    }
    for (i, label, count) in rows {
        let active = drawn_idx == Some(i);
        chan_list = chan_list.child(
            design::list_row(&format!("graph-chan-{i}"), active, t)
                .w_full()
                .on_press(move |s: &mut State| {
                    select_channel(&mut s.core, &mut s.graph_panel, i);
                })
                .child(icon_el_rgb("chart-line", 12.0, glyph_muted(t)))
                .child(text(label).font(type_::LABEL).color(t.text).grow(1.0))
                .child(
                    text(if count == 1 {
                        "1 key".to_string()
                    } else {
                        format!("{count} keys")
                    })
                    .font(type_::LABEL)
                    .color(t.text_secondary),
                ),
        );
    }

    // The channel actions (Duplicate / Delete / Shift)
    // are NOT children of this list: since Wave 9 the
    // list's scroll viewport is height-bounded (see the
    // panel assembly below), and actions inside a bounded
    // viewport would scroll away under their own list in
    // a many-channel project. They mount on the panel
    // directly under the viewport instead, under the same
    // `drawn_idx.is_some()` condition as before.

    // Stats block (Phase I): one muted line per channel
    // under the list — figure, target, key count, value
    // range (stat_rows / stat_line over the engine's
    // channel_stats). The drawn channel's line takes the
    // normal text colour instead (stat_rows flags it by
    // effective_channel_index — the picked row on
    // duplicates, exactly like the list's active row). No
    // channels: no block at all — the list's empty-state
    // line above already covers that case. Wave 10 bound:
    // the block renders stat_display_rows' projection,
    // capped at STATS_MAX_LINES lines (STATS_MAX_H) with
    // the overflow folded into the `graph-stats-more`
    // summary line, so a many-channel project can no
    // longer push the canvas and everything under it
    // down the column; shown lines keep their
    // `graph-stat-{channel index}` keys.
    let stat_display = stat_display_rows(core, &s.graph_panel);
    let mut stats_block = col().key("graph-stats").gap(2.0);
    for row in &stat_display {
        match row {
            StatDisplay::Line { index, line, drawn } => {
                stats_block = stats_block.child(
                    text(line.clone())
                        .key(format!("graph-stat-{index}"))
                        .font(type_::LABEL)
                        .color(if *drawn { t.text } else { t.text_secondary }),
                );
            }
            StatDisplay::More { hidden } => {
                stats_block = stats_block.child(
                    text(stats_more_line(*hidden))
                        .key("graph-stats-more")
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                );
            }
        }
    }

    // Interp segmented control (UI mission S12-08). While
    // a channel is drawn, the control edits THAT channel:
    // the active state reads the drawn channel's key at
    // the playhead (drawn_interp) and a press writes that
    // same key through graph_keys::set_key_interp
    // (set_drawn_interp) — so the row is labelled with
    // the drawn channel's own label, never "angle {ni}"
    // while it edits Root X. With nothing drawn, the
    // control falls back to its original target — the
    // selected joint's angle key (current_interp /
    // EditorCore::set_graph_key_interp) — and the label
    // says so, exactly as it always did.
    let interp_now = match drawn_idx {
        Some(_) => drawn_interp(core, &s.graph_panel),
        None => current_interp(core),
    };
    let interp_label = match drawn_idx {
        Some(_) => format!("Interp · {} key", channel_label(target)),
        None => match core.selected_node {
            Some(ni) => format!("Interp · angle {ni} key"),
            None => "Interp · no joint selected".to_string(),
        },
    };
    let interp_row = row()
        .key("graph-interp")
        .gap(4.0)
        .center_y()
        .child(design::section_label(&interp_label, t))
        .child(btn("graph-interp-hermite", "Hermite", interp_now == Some(Interp::Hermite), t, |s| {
            set_drawn_interp(&mut s.core, &s.graph_panel, Interp::Hermite)
        }))
        .child(btn("graph-interp-linear", "Linear", interp_now == Some(Interp::Linear), t, |s| {
            set_drawn_interp(&mut s.core, &s.graph_panel, Interp::Linear)
        }))
        .child(btn("graph-interp-step", "Step", interp_now == Some(Interp::Step), t, |s| {
            set_drawn_interp(&mut s.core, &s.graph_panel, Interp::Step)
        }));

    // Tangent presets (Wave 9 convergence, audit
    // S12-09): the kit's drawn-preview chip — this
    // panel's own construction, lifted into design in
    // Wave 4 and adopted by the keyframe tools and the
    // easing panel then; the private copy kept here was
    // verified identical (preview box, samples, fills,
    // handler) and retired. Keys, labels and the
    // `apply_graph_ease_preset` presses are unchanged;
    // presets are actions, not state, so never active.
    let presets = row()
        .key("graph-presets")
        .gap(4.0)
        .w_full()
        .flex_wrap()
        .child(design::tangent_preset_chip(
            "graph-preset-smooth",
            TangentPreset::Smooth,
            "Smooth",
            false,
            t,
            |s: &mut State| s.core.apply_graph_ease_preset(TangentPreset::Smooth),
        ))
        .child(design::tangent_preset_chip(
            "graph-preset-ease-in",
            TangentPreset::EaseIn,
            "Ease in",
            false,
            t,
            |s: &mut State| s.core.apply_graph_ease_preset(TangentPreset::EaseIn),
        ))
        .child(design::tangent_preset_chip(
            "graph-preset-ease-out",
            TangentPreset::EaseOut,
            "Ease out",
            false,
            t,
            |s: &mut State| s.core.apply_graph_ease_preset(TangentPreset::EaseOut),
        ))
        .child(design::tangent_preset_chip(
            "graph-preset-ease-in-out",
            TangentPreset::EaseInOut,
            "Ease in-out",
            false,
            t,
            |s: &mut State| s.core.apply_graph_ease_preset(TangentPreset::EaseInOut),
        ))
        .child(design::tangent_preset_chip(
            "graph-preset-linear",
            TangentPreset::Linear,
            "Linear",
            false,
            t,
            |s: &mut State| s.core.apply_graph_ease_preset(TangentPreset::Linear),
        ));

    let actions = row()
        .key("graph-actions")
        .gap(4.0)
        .w_full()
        .flex_wrap()
        .child(btn("graph-key", "Key angle", false, t, |s| s.core.key_graph_angle()))
        .child(btn("graph-key-bend", "Key bend", false, t, |s| key_bend(&mut s.core)))
        .child(btn("graph-smooth", "Auto tangents", false, t, |s| {
            s.core.smooth_graph_tangents()
        }))
        .child(btn("graph-mirror", "Mirror time", false, t, |s| {
            // A picked row edits THAT channel (Phase J);
            // without a pick, the target resolution (and
            // its "no such channel" status) applies.
            match effective_channel_index(&s.core, &s.graph_panel) {
                Some(i) => apply_mirror_at(&mut s.core, i),
                None => {
                    let target = effective_target(&s.core, s.graph_panel.target);
                    apply_mirror(&mut s.core, target);
                }
            }
        }))
        .child(btn("graph-scale-up", "Scale ×2", false, t, |s| {
            match effective_channel_index(&s.core, &s.graph_panel) {
                Some(i) => apply_scale_at(&mut s.core, i, 2.0),
                None => {
                    let target = effective_target(&s.core, s.graph_panel.target);
                    apply_scale(&mut s.core, target, 2.0);
                }
            }
        }))
        .child(btn("graph-scale-down", "Scale ÷2", false, t, |s| {
            match effective_channel_index(&s.core, &s.graph_panel) {
                Some(i) => apply_scale_at(&mut s.core, i, 0.5),
                None => {
                    let target = effective_target(&s.core, s.graph_panel.target);
                    apply_scale(&mut s.core, target, 0.5);
                }
            }
        }))
        .child(btn("graph-reduce", "Reduce keys", false, t, |s| {
            match effective_channel_index(&s.core, &s.graph_panel) {
                Some(i) => apply_reduce_at(&mut s.core, i),
                None => {
                    let target = effective_target(&s.core, s.graph_panel.target);
                    apply_reduce(&mut s.core, target);
                }
            }
        }))
        .child(btn("graph-bake", "Bake", false, t, |s| {
            let last = s.core.frame_count().saturating_sub(1);
            s.core.bake_graph_channels(0, last);
        }))
        // Clear (UI mission S12-10): clear_graph wipes
        // EVERY graph channel in the project, so the
        // button wears the danger family and its label
        // names the true scope — it used to read just
        // "Clear" in the same plain style as "Bake",
        // one panel away from "Clear channel keys",
        // which clears a single channel.
        .child(danger_btn("graph-clear", "Clear all channels", t, |s| {
            s.core.clear_graph()
        }));

    // The grabbed key, resolved against the CURRENT key
    // order for the highlight (the stored index is only a
    // hint once re-sorts start permuting it — see
    // GraphPanelState::drag_key).
    let drag_idx = s.graph_panel.drag_key.and_then(|hint| {
        let ch = ch?;
        resolve_drag_index(ch, hint, s.graph_panel.drag_anchor)
    });
    // The selected key, whose tangent handles the canvas
    // draws and the drag hit-tests: the key nearest the
    // playhead ON THE DRAWN CHANNEL (which the index, not
    // the target, resolved — a picked duplicate shows its
    // own handles).
    let sel_idx = ch.and_then(|c| nearest_key(c, core.current as f32));

    let mut panel = design::panel_col("graph-panel", t)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(design::panel_title(
                    Some("chart-line"),
                    &format!("Graph — figure {}", core.selected_figure + 1),
                    None,
                    t,
                ))
                .child(text(readout).font(type_::LABEL).color(t.text_secondary)),
        )
        .child(design::divider(t))
        // The list's viewport is bounded (Wave 9, audit
        // S12-07): at most five rows tall
        // (CHAN_LIST_MAX_H), scrolling internally past
        // that, so a growing project can no longer push
        // the canvas and everything below it down the
        // column (the audit's D2).
        .child(
            chan_list
                .scroll(ScrollSpec { stick_bottom: false })
                .min_h(0.0)
                .max_h(CHAN_LIST_MAX_H),
        );
    // Duplicate-to-next-figure (Phase J integrator
    // wiring): shown only while a channel is drawn; the
    // press duplicates the drawn channel — twin-aware via
    // the effective index — onto the following figure.
    // Delete channel (Phase K) sits directly beside it,
    // under the same condition, in the chrome family's
    // danger styling (UI mission S12-10: the press deletes
    // a whole channel — it should look like it): the press
    // removes the drawn channel through
    // delete_drawn_channel, which repairs the stored pick.
    // The Shift pair (Phase M) closes the block under the
    // same condition: icon buttons moving every key of the
    // drawn channel one frame either way through
    // shift_drawn_channel — repeated presses accumulate,
    // and the op refuses cleanly at the frame-0 wall.
    // (Wave 9: the block moved out of `chan_list` to sit
    // directly under its bounded viewport — see the note
    // at the list's construction — with keys, order and
    // handlers unchanged.)
    if drawn_idx.is_some() {
        panel = panel
            .child(btn(
                "graph-chan-duplicate",
                "Duplicate channel to next figure",
                false,
                t,
                |s: &mut State| duplicate_channel_to_next(&mut s.core, &s.graph_panel),
            ))
            .child(danger_btn(
                "graph-chan-delete",
                "Delete channel",
                t,
                |s: &mut State| delete_drawn_channel(&mut s.core, &mut s.graph_panel),
            ))
            .child(
                row()
                    .key("graph-shift-row")
                    .gap(4.0)
                    .child(icon_btn(
                        "graph-shift-l",
                        "chevron-left",
                        "Shift -1",
                        false,
                        t,
                        |s: &mut State| shift_drawn_channel(&mut s.core, &s.graph_panel, -1.0),
                    ))
                    .child(icon_btn(
                        "graph-shift-r",
                        "chevron-right",
                        "Shift +1",
                        false,
                        t,
                        |s: &mut State| shift_drawn_channel(&mut s.core, &s.graph_panel, 1.0),
                    )),
            );
    }
    if !stat_display.is_empty() {
        panel = panel.child(stats_block);
    }
    panel = panel
        .child(selector)
        // The canvas wears the kit hairline (Wave 9,
        // audit S12-12): its fill is the panel's own
        // `t.card`, so without a border the viewport had
        // no visible edge in either theme — only its
        // radius distinguished it. Rune borders are
        // paint-only (crates/rune/src/el/visuals.rs), so
        // the layout box, the fit and the drag geometry
        // are all untouched.
        .child(
            canvas(graph_shape(
                core,
                target,
                size,
                t,
                drag_idx,
                sel_idx,
                s.graph_panel.drag_handle,
                drawn_idx,
            ))
                .key("graph-canvas")
                .w_full()
                .h(GRAPH_H)
                .fill(t.card)
                .radius(8.0)
                .border(1.0, t.card_border)
                .on_drag(|s: &mut State, d: &mut Drag, _tx: &mut Tx| graph_drag(s, d)),
        );
    // Value readout (Phase L): the drawn channel's value
    // at the playhead on its own muted line directly
    // under the canvas — the number the header readout's
    // tail carries, readable without scanning the header.
    // Absent when no channel is drawn: value_line
    // resolves through effective_channel_index, the same
    // index the canvas drew, so the line can never name
    // a value for a channel that is not on screen.
    if let Some(line) = value_line(core, &s.graph_panel) {
        panel = panel.child(
            text(line)
                .key("graph-value")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    }
    if ch.is_none() {
        panel = panel.child(design::empty_state(
            "chart-line",
            "No graph keys on this channel yet — select a joint on the stage, then Key angle.",
            t,
        ));
    }
    panel
        .child(interp_row)
        .child(presets)
        .child(actions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use stykanimate_rs::graph::Key;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    fn channel(target: ChannelTarget, keys: Vec<Key>) -> Channel {
        Channel {
            figure: 0,
            target,
            keys,
        }
    }

    #[test]
    fn labels_name_every_target() {
        assert_eq!(channel_label(ChannelTarget::RootX), "Root X");
        assert_eq!(channel_label(ChannelTarget::RootY), "Root Y");
        assert_eq!(channel_label(ChannelTarget::Angle(3)), "Angle 3");
        assert_eq!(channel_label(ChannelTarget::Length(12)), "Length 12");
        assert_eq!(channel_label(ChannelTarget::Bend(3)), "Bend 3");
    }

    #[test]
    fn selector_pins_roots_then_node_angle_then_figure_channels() {
        let mut core = EditorCore::new();
        // Demo core: no node selected, no channels.
        assert_eq!(
            selector_entries(&core),
            vec![ChannelTarget::RootX, ChannelTarget::RootY]
        );
        core.selected_node = Some(2);
        assert_eq!(
            selector_entries(&core),
            vec![
                ChannelTarget::RootX,
                ChannelTarget::RootY,
                ChannelTarget::Angle(2),
                ChannelTarget::Bend(2)
            ]
        );
        // The figure's own channels join in project order,
        // without duplicating the pinned entries; another
        // figure's channels do not appear.
        core.project.channels = vec![
            channel(ChannelTarget::Angle(5), vec![Key::flat(0.0, 0.0)]),
            channel(ChannelTarget::RootX, vec![Key::flat(0.0, 0.0)]),
            Channel {
                figure: 1,
                target: ChannelTarget::Length(1),
                keys: vec![Key::flat(0.0, 0.0)],
            },
        ];
        assert_eq!(
            selector_entries(&core),
            vec![
                ChannelTarget::RootX,
                ChannelTarget::RootY,
                ChannelTarget::Angle(2),
                ChannelTarget::Bend(2),
                ChannelTarget::Angle(5)
            ]
        );
    }

    #[test]
    fn key_bend_authors_the_channel_with_the_pose_bend() {
        // Wave Y (P-7): with no Bend channel yet, Key bend
        // creates the selected node's channel and keys the
        // pose bend in force at the playhead — one undo step.
        let mut core = EditorCore::new();
        core.selected_node = Some(2);
        let node_counts: Vec<usize> = core
            .project
            .figures
            .iter()
            .map(|f| f.def.nodes.len())
            .collect();
        core.project.frames[0].bends =
            node_counts.iter().map(|&n| vec![0.0; n]).collect();
        core.project.frames[0].bends[0][2] = 0.7;
        let depth = core.undo_depth();
        key_bend(&mut core);
        assert_eq!(core.undo_depth(), depth + 1, "exactly one undo step");
        let ch = channel_for(&core, ChannelTarget::Bend(2)).expect("bend channel authored");
        assert_eq!(ch.keys.len(), 1);
        assert_eq!(ch.keys[0].frame, 0.0);
        assert_eq!(ch.keys[0].value, 0.7, "the pose bend at the playhead");
        assert!(core.status.contains("bend = 0.70 rad"), "{}", core.status);
        // The pick the button enables: Bend resolves as the
        // drawn channel once selected in the selector.
        assert!(selector_entries(&core).contains(&ChannelTarget::Bend(2)));
        // Undo removes the authored channel wholesale.
        core.undo();
        assert!(channel_for(&core, ChannelTarget::Bend(2)).is_none());
    }

    #[test]
    fn key_bend_keys_the_channel_value_once_a_channel_exists() {
        // A Bend channel overrides the pose bend at
        // evaluation, so re-keying must key the CHANNEL's
        // value at the playhead — keying the pose value
        // would move the curve.
        let mut core = EditorCore::new();
        core.selected_node = Some(1);
        core.project.channels = vec![channel(
            ChannelTarget::Bend(1),
            vec![Key::flat(0.0, 0.3), Key::flat(4.0, 0.3)],
        )];
        key_bend(&mut core);
        let ch = channel_for(&core, ChannelTarget::Bend(1)).unwrap();
        assert_eq!(ch.keys.len(), 2, "the playhead key replaces, not appends");
        assert_eq!(ch.keys[0].value, 0.3, "the channel's own value, keyed flat");
        // No joint selected: status only, nothing authored,
        // no undo step spent.
        let mut bare = EditorCore::new();
        let depth = bare.undo_depth();
        key_bend(&mut bare);
        assert!(bare.project.channels.is_empty());
        assert_eq!(bare.undo_depth(), depth);
        assert!(bare.status.contains("Select a joint first"));
    }

    #[test]
    fn effective_target_fallback_order() {
        let mut core = EditorCore::new();
        core.selected_node = Some(1);
        // No channels, no pick: the node angle (the channel
        // Key angle would author).
        assert_eq!(effective_target(&core, None), ChannelTarget::Angle(1));
        // A valid pick wins.
        assert_eq!(
            effective_target(&core, Some(ChannelTarget::RootY)),
            ChannelTarget::RootY
        );
        // A stale pick (not a selector entry) falls through to
        // the figure's first real channel once one exists.
        core.project.channels = vec![channel(
            ChannelTarget::RootY,
            vec![Key::flat(0.0, 5.0)],
        )];
        assert_eq!(
            effective_target(&core, Some(ChannelTarget::Length(9))),
            ChannelTarget::RootY
        );
        assert_eq!(effective_target(&core, None), ChannelTarget::RootY);
        // Nothing at all: Root X.
        let bare = EditorCore::new();
        assert_eq!(effective_target(&bare, None), ChannelTarget::RootX);
    }

    #[test]
    fn channel_index_scopes_to_the_selected_figure() {
        let mut core = EditorCore::new();
        core.project.channels = vec![
            Channel {
                figure: 1,
                target: ChannelTarget::RootX,
                keys: vec![],
            },
            channel(ChannelTarget::RootX, vec![Key::flat(0.0, 0.0)]),
        ];
        assert_eq!(channel_index(&core, ChannelTarget::RootX), Some(1));
        assert_eq!(channel_index(&core, ChannelTarget::RootY), None);
        assert!(channel_for(&core, ChannelTarget::RootX).is_some());
    }

    #[test]
    fn nearest_key_picks_closest_and_breaks_ties_early() {
        let ch = channel(
            ChannelTarget::RootX,
            vec![Key::flat(0.0, 0.0), Key::flat(10.0, 100.0)],
        );
        assert_eq!(nearest_key(&ch, 0.0), Some(0));
        assert_eq!(nearest_key(&ch, 4.9), Some(0));
        // Exactly between the two keys: the earlier key wins,
        // matching set_graph_key_interp's documented pick.
        assert_eq!(nearest_key(&ch, 5.0), Some(0));
        assert_eq!(nearest_key(&ch, 5.1), Some(1));
        let empty = channel(ChannelTarget::RootX, vec![]);
        assert_eq!(nearest_key(&empty, 3.0), None);
    }

    #[test]
    fn current_interp_reads_the_key_the_op_would_edit() {
        let mut core = EditorCore::new();
        // No node selected: nothing to read.
        assert_eq!(current_interp(&core), None);
        core.selected_node = Some(0);
        // Keys at frames 0 and 4 — inside the demo's 5-frame
        // timeline, so `goto` can park next to each of them.
        core.project.channels = vec![channel(
            ChannelTarget::Angle(0),
            vec![
                Key::flat(0.0, 0.0).with_interp(Interp::Step),
                Key::flat(4.0, 90.0),
            ],
        )];
        core.goto(1);
        assert_eq!(current_interp(&core), Some(Interp::Step));
        // Near the second key (default Hermite) the readout
        // follows the op's target key.
        core.goto(3);
        assert_eq!(current_interp(&core), Some(Interp::Hermite));
    }

    #[test]
    fn drawn_interp_reads_the_drawn_channel_not_the_angle() {
        // UI mission S12-08: with a Root X channel drawn,
        // the segmented control's highlight is read from
        // Root X's key at the playhead — even while a
        // joint is selected whose angle channel carries a
        // different mode at the same frame (the old
        // control read the angle key in exactly this
        // state, under a curve it did not describe).
        let mut core = EditorCore::new();
        core.selected_node = Some(0);
        core.project.channels = vec![
            channel(
                ChannelTarget::RootX,
                vec![Key::flat(0.0, 0.0).with_interp(Interp::Step)],
            ),
            channel(
                ChannelTarget::Angle(0),
                vec![Key::flat(0.0, 0.0).with_interp(Interp::Linear)],
            ),
        ];
        let mut sel = GraphPanelState::default();
        sel.selected_channel = Some(0);
        core.goto(0);
        assert_eq!(drawn_interp(&core, &sel), Some(Interp::Step));
        // The legacy read still sees the angle key — the
        // two reads genuinely name different keys.
        assert_eq!(current_interp(&core), Some(Interp::Linear));
        // Playhead parked away from Root X's only key:
        // no key sits at the playhead, so nothing
        // highlights on the drawn channel.
        core.goto(2);
        assert_eq!(drawn_interp(&core, &sel), None);
    }

    #[test]
    fn set_drawn_interp_edits_the_drawn_channels_key() {
        // UI mission S12-08: a press writes the drawn
        // channel's key at the playhead through
        // graph_keys::set_key_interp — one snapshot, the
        // op's own status — and leaves the selected
        // joint's angle key untouched.
        let mut core = EditorCore::new();
        core.selected_node = Some(0);
        core.project.channels = vec![
            channel(
                ChannelTarget::RootX,
                vec![Key::flat(0.0, 0.0).with_interp(Interp::Step)],
            ),
            channel(
                ChannelTarget::Angle(0),
                vec![Key::flat(0.0, 0.0).with_interp(Interp::Linear)],
            ),
        ];
        let mut sel = GraphPanelState::default();
        sel.selected_channel = Some(0);
        core.goto(0);
        let depth = core.undo_depth();
        set_drawn_interp(&mut core, &sel, Interp::Hermite);
        assert_eq!(
            core.project.channels[0].keys[0].interp,
            Interp::Hermite,
            "the drawn channel's key takes the mode"
        );
        assert_eq!(
            core.project.channels[1].keys[0].interp,
            Interp::Linear,
            "the angle key is not the target anymore"
        );
        assert_eq!(core.undo_depth(), depth + 1, "one snapshot via the op");
    }

    #[test]
    fn set_drawn_interp_refuses_with_no_key_at_the_playhead() {
        // Playhead parked away from every key of the
        // drawn channel: the press writes a status and
        // changes nothing — no snapshot, and no silent
        // edit of a distant key.
        let mut core = EditorCore::new();
        core.project.channels = vec![channel(
            ChannelTarget::RootX,
            vec![Key::flat(0.0, 0.0).with_interp(Interp::Step)],
        )];
        let mut sel = GraphPanelState::default();
        sel.selected_channel = Some(0);
        core.goto(3);
        let depth = core.undo_depth();
        set_drawn_interp(&mut core, &sel, Interp::Linear);
        assert_eq!(core.project.channels[0].keys[0].interp, Interp::Step);
        assert_eq!(core.undo_depth(), depth, "a refusal takes no snapshot");
        assert!(
            core.status.contains("No key at frame 4"),
            "the status names the refusal: {}",
            core.status
        );
    }

    #[test]
    fn set_drawn_interp_falls_back_when_no_channel_is_drawn() {
        // The panel's target names Root Y and no Root Y
        // channel exists, so nothing is drawn: the press
        // takes the original path and edits the selected
        // joint's angle key, exactly as
        // EditorCore::set_graph_key_interp always did.
        let mut core = EditorCore::new();
        core.selected_node = Some(0);
        core.project.channels = vec![channel(
            ChannelTarget::Angle(0),
            vec![Key::flat(0.0, 0.0).with_interp(Interp::Step)],
        )];
        let mut sel = GraphPanelState::default();
        sel.target = Some(ChannelTarget::RootY);
        assert_eq!(effective_channel_index(&core, &sel), None);
        core.goto(0);
        set_drawn_interp(&mut core, &sel, Interp::Linear);
        assert_eq!(core.project.channels[0].keys[0].interp, Interp::Linear);
    }

    #[test]
    fn sampled_curve_passes_through_its_keys() {
        let ch = channel(
            ChannelTarget::RootX,
            vec![
                Key::flat(0.0, 0.0),
                Key::flat(5.0, 80.0),
                Key::flat(10.0, 20.0),
            ],
        );
        let view = GraphView::auto_fit(std::slice::from_ref(&ch), 11, 500.0, 250.0);
        // 100 steps over 10 frames lands a sample exactly on
        // every key frame.
        let pts = sample_curve(&view, &ch, 100);
        assert_eq!(pts.len(), 101);
        for handle in view.key_handles(&ch) {
            let nearest = pts
                .iter()
                .map(|p| (p[0] - handle[0]).abs() + (p[1] - handle[1]).abs())
                .fold(f32::INFINITY, f32::min);
            assert!(nearest < 1e-2, "curve misses key {handle:?} by {nearest}");
        }
        assert!(sample_curve(&view, &ch, 0).is_empty());
    }

    #[test]
    fn diamond_is_four_points_on_the_radius() {
        let d = diamond([10.0, 20.0], 4.0);
        assert_eq!(d[0], [10.0, 16.0], "top first: {d:?}");
        assert_eq!(d[1], [14.0, 20.0]);
        assert_eq!(d[2], [10.0, 24.0]);
        assert_eq!(d[3], [6.0, 20.0]);
    }

    #[test]
    fn preview_points_put_value_zero_at_the_bottom() {
        // The Linear preview is the exact diagonal of the
        // canonical ramp, so its pixels are the box diagonal
        // from bottom-left to top-right.
        let pts = preview_points(TangentPreset::Linear, 48.0, 28.0, 5);
        assert_eq!(pts.len(), 5);
        for (got, want) in pts.iter().zip([[0.0, 28.0], [12.0, 21.0], [24.0, 14.0], [36.0, 7.0], [48.0, 0.0]]) {
            assert!(
                approx(got[0], want[0]) && approx(got[1], want[1]),
                "linear preview point {got:?} != {want:?}"
            );
        }
        // Ease-in lags the diagonal early, so at 25% its pixel
        // sits BELOW the diagonal's (larger y = lower value).
        let ease = preview_points(TangentPreset::EaseIn, 48.0, 28.0, 5);
        assert!(ease[1][1] > 21.0, "ease-in starts slow: {ease:?}");
    }

    #[test]
    fn apply_scale_doubles_values_and_reports() {
        let mut core = EditorCore::new();
        core.project.channels = vec![channel(
            ChannelTarget::RootX,
            vec![Key::flat(0.0, 10.0), Key::flat(4.0, 30.0)],
        )];
        apply_scale(&mut core, ChannelTarget::RootX, 2.0);
        let keys = &core.project.channels[0].keys;
        assert_eq!(keys[0].value, 20.0);
        assert_eq!(keys[1].value, 60.0);
        assert!(core.status.contains("Scaled graph channel values by 2"));
        // No channel: status only, nothing created.
        apply_scale(&mut core, ChannelTarget::RootY, 2.0);
        assert_eq!(core.project.channels.len(), 1);
        assert!(core.status.contains("to scale"));
    }

    #[test]
    fn apply_mirror_reflects_frames_around_the_timeline_middle() {
        let mut core = EditorCore::new();
        // The demo timeline has 5 frames, so the mirror centre
        // is frame 2: keys at 0 and 4 swap places and values.
        assert_eq!(core.frame_count(), 5);
        core.project.channels = vec![channel(
            ChannelTarget::RootX,
            vec![Key::flat(0.0, 10.0), Key::flat(4.0, 30.0)],
        )];
        apply_mirror(&mut core, ChannelTarget::RootX);
        let keys = &core.project.channels[0].keys;
        assert_eq!(keys.len(), 2);
        assert_eq!(keys[0].frame, 0.0);
        assert_eq!(keys[0].value, 30.0, "ascending order restored");
        assert_eq!(keys[1].frame, 4.0);
        assert_eq!(keys[1].value, 10.0);
    }

    #[test]
    fn apply_reduce_drops_only_redundant_keys() {
        let mut core = EditorCore::new();
        // A constant channel's middle key contributes nothing.
        core.project.channels = vec![channel(
            ChannelTarget::RootX,
            vec![
                Key::flat(0.0, 42.0),
                Key::flat(2.0, 42.0),
                Key::flat(4.0, 42.0),
            ],
        )];
        apply_reduce(&mut core, ChannelTarget::RootX);
        assert_eq!(core.project.channels[0].keys.len(), 2);
        assert!(core.status.contains("removed 1 key"));
        // A zigzag's middle key carries the shape: kept.
        core.project.channels = vec![channel(
            ChannelTarget::RootX,
            vec![
                Key::flat(0.0, 0.0),
                Key::flat(2.0, 100.0),
                Key::flat(4.0, 0.0),
            ],
        )];
        apply_reduce(&mut core, ChannelTarget::RootX);
        assert_eq!(core.project.channels[0].keys.len(), 3);
    }

    #[test]
    fn pointer_roundtrip_through_the_panels_own_fit() {
        let mut core = EditorCore::new();
        core.project.channels = vec![channel(
            ChannelTarget::RootX,
            vec![
                Key::flat(0.0, 0.0),
                Key::flat(4.0, 80.0),
                Key::flat(8.0, 20.0),
            ],
        )];
        let size = [480.0, 170.0];
        let view = drag_view(&core, ChannelTarget::RootX, size);
        let ch = &core.project.channels[0];
        // Every key's drawn pixel converts back to its data
        // and hit-tests to its own index — the drag uses the
        // exact transform the drawing used.
        for (i, k) in ch.keys.iter().enumerate() {
            let px = view.to_px(k.frame, k.value);
            let (f, v) = view.to_data(px);
            assert!(
                approx(f, k.frame) && approx(v, k.value),
                "key {i} roundtrip ({}, {}) -> ({f}, {v})",
                k.frame,
                k.value
            );
            assert_eq!(view.hit_key(ch, px, GRAB_RADIUS), Some(i));
        }
        // The middle of the canvas is far from every handle.
        assert_eq!(view.hit_key(ch, [240.0, 160.0], GRAB_RADIUS), None);
    }

    #[test]
    fn drag_key_to_crosses_neighbours_clamps_and_reindexes() {
        let mut ch = channel(
            ChannelTarget::RootX,
            vec![
                Key::flat(0.0, 0.0),
                Key::flat(4.0, 40.0),
                Key::flat(8.0, 80.0),
            ],
        );
        // Drag key 0 past its neighbour: it lands at index 1
        // and the order is ascending again.
        assert_eq!(drag_key_to(&mut ch, 0, 6.5, 55.0), Some(1));
        let frames: Vec<f32> = ch.keys.iter().map(|k| k.frame).collect();
        assert_eq!(frames, vec![4.0, 6.5, 8.0]);
        assert_eq!(ch.keys[1].value, 55.0);
        // Frames clamp at 0; values do not clamp.
        assert_eq!(drag_key_to(&mut ch, 2, -2.0, -10.0), Some(0));
        assert_eq!(ch.keys[0].frame, 0.0);
        assert_eq!(ch.keys[0].value, -10.0);
        // Out-of-range index: no change, no new index.
        assert_eq!(drag_key_to(&mut ch, 99, 1.0, 1.0), None);
        assert_eq!(ch.keys.len(), 3);
        assert_eq!(ch.keys[1].frame, 4.0);
    }

    #[test]
    fn resolve_drag_index_uses_hint_then_anchor() {
        let ch = channel(
            ChannelTarget::RootX,
            vec![
                Key::flat(0.0, 10.0),
                Key::flat(4.0, 20.0),
                Key::flat(8.0, 30.0),
            ],
        );
        // Fast path: the hinted key still sits at the anchor.
        assert_eq!(resolve_drag_index(&ch, 1, 4.0), Some(1));
        // A re-sort permuted the vec (here by hand): the hint
        // now points at the frame-8 key, so the anchor decides.
        let permuted = channel(
            ChannelTarget::RootX,
            vec![
                Key::flat(4.0, 20.0),
                Key::flat(8.0, 30.0),
                Key::flat(0.0, 10.0),
            ],
        );
        assert_eq!(resolve_drag_index(&permuted, 1, 4.0), Some(0));
        // A stale out-of-range hint still resolves by anchor.
        assert_eq!(resolve_drag_index(&ch, 99, 4.0), Some(1));
        // No keys: nothing to resolve.
        let empty = channel(ChannelTarget::RootX, vec![]);
        assert_eq!(resolve_drag_index(&empty, 0, 4.0), None);
    }

    #[test]
    fn seek_frame_rounds_and_clamps() {
        // 60 px per frame over frames 0..=8.
        let view = GraphView {
            frame_min: 0.0,
            frame_max: 8.0,
            val_min: -8.0,
            val_max: 88.0,
            width: 480.0,
            height: 170.0,
        };
        assert_eq!(seek_frame(&view, 144.0, 10), Some(2), "frame 2.4 rounds down");
        assert_eq!(seek_frame(&view, 150.0, 10), Some(3), "frame 2.5 rounds up");
        assert_eq!(seek_frame(&view, -30.0, 10), Some(0), "before the start clamps");
        assert_eq!(seek_frame(&view, 10_000.0, 5), Some(4), "past the end clamps");
        assert_eq!(seek_frame(&view, 144.0, 0), None, "no frames: no seek");
    }

    #[test]
    fn drag_gesture_end_to_end_and_click_seek() {
        let mut core = EditorCore::new();
        core.project.channels = vec![channel(
            ChannelTarget::RootX,
            vec![
                Key::flat(0.0, 0.0),
                Key::flat(4.0, 80.0),
                Key::flat(8.0, 20.0),
            ],
        )];
        let mut st = GraphPanelState::default();
        let size = [480.0, 170.0];

        // Down on key 1's handle grabs it.
        let view = drag_view(&core, ChannelTarget::RootX, size);
        let grab = view.to_px(4.0, 80.0);
        assert!(drag_down(&mut core, &mut st, grab, size));
        assert_eq!(st.drag_key, Some(1));
        assert!(approx(st.drag_anchor, 4.0));

        // Move to data (6, 10): the key follows, still sorted.
        let dest = view.to_px(6.0, 10.0);
        assert!(drag_move(&mut core, &mut st, dest, size));
        let keys = &core.project.channels[0].keys;
        assert!(approx(keys[1].frame, 6.0) && approx(keys[1].value, 10.0));
        assert_eq!(st.drag_key, Some(1));

        // Up ends the drag and reports the final position.
        assert!(drag_up(&mut core, &mut st, dest, size, 40.0, false));
        assert_eq!(st.drag_key, None);
        assert!(
            core.status.contains("Graph key moved"),
            "status: {}",
            core.status
        );

        // A click on empty canvas seeks: x=470 is past the
        // fitted end (frame 8) but the demo has only 5 frames,
        // so the seek clamps to frame index 4.
        let far = [470.0, 165.0];
        assert!(!drag_down(&mut core, &mut st, far, size));
        core.goto(0);
        assert!(drag_up(&mut core, &mut st, far, size, 1.0, false));
        assert_eq!(core.current, 4);
        // A long drag over empty canvas does NOT seek, and
        // neither does a cancelled click.
        core.goto(0);
        assert!(!drag_up(&mut core, &mut st, far, size, 30.0, false));
        assert_eq!(core.current, 0);
        assert!(!drag_up(&mut core, &mut st, far, size, 1.0, true));
        assert_eq!(core.current, 0);

        // A cancelled key drag rolls back through the Down
        // snapshot: the key returns to (6, 10).
        let grab2 = drag_view(&core, ChannelTarget::RootX, size).to_px(6.0, 10.0);
        assert!(drag_down(&mut core, &mut st, grab2, size));
        let dest2 = drag_view(&core, ChannelTarget::RootX, size).to_px(2.0, 70.0);
        assert!(drag_move(&mut core, &mut st, dest2, size));
        assert!(drag_up(&mut core, &mut st, dest2, size, 200.0, true));
        assert_eq!(st.drag_key, None);
        let keys = &core.project.channels[0].keys;
        assert!(
            keys.iter()
                .any(|k| approx(k.frame, 6.0) && approx(k.value, 10.0)),
            "cancel restored the pre-drag key: {keys:?}"
        );
    }

    #[test]
    fn selected_key_index_is_the_key_nearest_the_playhead() {
        let mut core = EditorCore::new();
        core.project.channels = vec![channel(
            ChannelTarget::RootX,
            vec![
                Key::flat(0.0, 0.0),
                Key::flat(4.0, 80.0),
                Key::flat(8.0, 20.0),
            ],
        )];
        core.goto(3);
        assert_eq!(selected_key_index(&core, ChannelTarget::RootX), Some(1));
        core.goto(0);
        assert_eq!(selected_key_index(&core, ChannelTarget::RootX), Some(0));
        // No channel for the target: no selection.
        assert_eq!(selected_key_index(&core, ChannelTarget::RootY), None);
    }

    #[test]
    fn tangent_handle_data_sits_on_the_tangent_ray() {
        let mut mid = Key::flat(10.0, 100.0);
        mid.in_tan = 2.0;
        mid.out_tan = -1.0;
        let ch = channel(
            ChannelTarget::RootX,
            vec![Key::flat(0.0, 0.0), mid, Key::flat(20.0, 40.0)],
        );
        // Segment spans are 10 frames each way; the handle
        // sits a third of the span along the tangent line:
        // In: (10 − 10/3, 100 − (10/3)·2); Out: (10 + 10/3,
        // 100 + (10/3)·(−1)).
        let (f, v) = tangent_handle_data(&ch, 1, TangentSide::In).unwrap();
        assert!(
            approx(f, 10.0 - 10.0 / 3.0) && approx(v, 100.0 - 20.0 / 3.0),
            "in handle ({f}, {v})"
        );
        let (f, v) = tangent_handle_data(&ch, 1, TangentSide::Out).unwrap();
        assert!(
            approx(f, 10.0 + 10.0 / 3.0) && approx(v, 100.0 - 10.0 / 3.0),
            "out handle ({f}, {v})"
        );
        // Endpoints lack the side that governs no segment.
        assert_eq!(tangent_handle_data(&ch, 0, TangentSide::In), None);
        assert_eq!(tangent_handle_data(&ch, 2, TangentSide::Out), None);
        // A lone key has neither handle.
        let one = channel(ChannelTarget::RootX, vec![Key::flat(1.0, 1.0)]);
        assert_eq!(tangent_handle_data(&one, 0, TangentSide::In), None);
        assert_eq!(tangent_handle_data(&one, 0, TangentSide::Out), None);
        // Out of range: nothing.
        assert_eq!(tangent_handle_data(&ch, 9, TangentSide::In), None);
    }

    #[test]
    fn hit_tangent_handle_picks_the_side_in_data_space() {
        let mut core = EditorCore::new();
        let mut mid = Key::flat(4.0, 80.0);
        mid.in_tan = 5.0;
        mid.out_tan = -5.0;
        core.project.channels = vec![channel(
            ChannelTarget::RootX,
            vec![Key::flat(0.0, 0.0), mid, Key::flat(8.0, 20.0)],
        )];
        let size = [480.0, 170.0];
        let view = drag_view(&core, ChannelTarget::RootX, size);
        let ch = &core.project.channels[0];
        // Each handle's own pixel hits its own side.
        for side in [TangentSide::In, TangentSide::Out] {
            let (hf, hv) = tangent_handle_data(ch, 1, side).unwrap();
            let px = view.to_px(hf, hv);
            assert_eq!(
                hit_tangent_handle(&view, ch, 1, px, GRAB_RADIUS),
                Some(side)
            );
        }
        // The key's own pixel is a third of a segment away
        // from both handles: no tangent hit there (the key
        // hit-test owns that press).
        let kp = view.to_px(4.0, 80.0);
        assert_eq!(hit_tangent_handle(&view, ch, 1, kp, GRAB_RADIUS), None);
        // Key 0 has no In handle, and its Out handle is far
        // from kp: no hit for it either.
        assert_eq!(hit_tangent_handle(&view, ch, 0, kp, GRAB_RADIUS), None);
    }

    #[test]
    fn tangent_slope_reads_the_line_through_the_key() {
        let k = Key::flat(4.0, 40.0);
        // Down-right and up-left on the same line: slope 10 —
        // one formula serves both sides.
        assert_eq!(tangent_slope(&k, (8.0, 80.0)), Some(10.0));
        assert_eq!(tangent_slope(&k, (0.0, 0.0)), Some(10.0));
        // A falling line: negative slope.
        assert_eq!(tangent_slope(&k, (6.0, 20.0)), Some(-10.0));
        // Pointer (nearly) on the key's own frame: no
        // meaningful slope — the tangent stays as it was.
        assert_eq!(tangent_slope(&k, (4.0, 90.0)), None);
        assert_eq!(tangent_slope(&k, (4.01, 90.0)), None);
    }

    #[test]
    fn drag_tangent_to_sets_only_the_dragged_side() {
        let mut k = Key::flat(4.0, 40.0);
        k.in_tan = 1.5;
        k.out_tan = -2.0;
        let mut ch = channel(
            ChannelTarget::RootX,
            vec![Key::flat(0.0, 0.0), k, Key::flat(8.0, 20.0)],
        );
        // Drag the Out handle to data (8, 80): slope
        // (80 − 40) / (8 − 4) = 10; in_tan untouched — the
        // model's broken-tangent convention.
        assert!(drag_tangent_to(&mut ch, 1, TangentSide::Out, (8.0, 80.0)));
        assert!(approx(ch.keys[1].out_tan, 10.0));
        assert!(approx(ch.keys[1].in_tan, 1.5), "other side untouched");
        // Drag the In handle to (0, 0): slope
        // (0 − 40) / (0 − 4) = 10; the first edit survives.
        assert!(drag_tangent_to(&mut ch, 1, TangentSide::In, (0.0, 0.0)));
        assert!(approx(ch.keys[1].in_tan, 10.0));
        assert!(approx(ch.keys[1].out_tan, 10.0), "first edit kept");
        // Frames and values never move, and the key order is
        // untouched — no re-sort is needed or done.
        let frames: Vec<f32> = ch.keys.iter().map(|k| k.frame).collect();
        assert_eq!(frames, vec![0.0, 4.0, 8.0]);
        assert!(approx(ch.keys[1].value, 40.0));
        // Pointer on the key's frame writes nothing, and
        // neither does an out-of-range index.
        assert!(!drag_tangent_to(&mut ch, 1, TangentSide::Out, (4.0, 99.0)));
        assert!(!drag_tangent_to(&mut ch, 7, TangentSide::Out, (8.0, 80.0)));
    }

    #[test]
    fn tangent_drag_gesture_end_to_end_and_cancel() {
        let mut core = EditorCore::new();
        core.project.channels = vec![channel(
            ChannelTarget::RootX,
            vec![
                Key::flat(0.0, 0.0),
                Key::flat(4.0, 80.0),
                Key::flat(8.0, 20.0),
            ],
        )];
        // Park the playhead on the middle key: it is the
        // selected key, whose tangent handles are live.
        core.goto(4);
        let mut st = GraphPanelState::default();
        let size = [480.0, 170.0];
        let view = drag_view(&core, ChannelTarget::RootX, size);

        // Priority is narrow: a press on the selected key's
        // own diamond still starts a KEY drag — the handles
        // sit a third of a segment away from it.
        let kp = view.to_px(4.0, 80.0);
        assert!(drag_down(&mut core, &mut st, kp, size));
        assert_eq!(st.drag_key, Some(1));
        assert_eq!(st.drag_handle, None);
        assert!(drag_up(&mut core, &mut st, kp, size, 0.0, false));

        // Down on the middle key's Out handle grabs the
        // handle, not the key.
        let (hf, hv) =
            tangent_handle_data(&core.project.channels[0], 1, TangentSide::Out).unwrap();
        let grab = view.to_px(hf, hv);
        assert!(drag_down(&mut core, &mut st, grab, size));
        assert_eq!(st.drag_handle, Some((1, TangentSide::Out)));
        assert_eq!(st.drag_key, None);

        // Move to data (6, 30): the out slope becomes
        // (30 − 80) / (6 − 4) = −25. The fit cannot drift —
        // tangent edits touch no key frame or value — so the
        // pre-drag view still maps the pointer exactly.
        let dest = view.to_px(6.0, 30.0);
        assert!(drag_move(&mut core, &mut st, dest, size));
        let k = core.project.channels[0].keys[1];
        assert!(approx(k.out_tan, -25.0), "out tangent follows: {}", k.out_tan);
        assert!(approx(k.in_tan, 0.0), "in tangent untouched");

        // Up reports the tangent and ends the gesture.
        assert!(drag_up(&mut core, &mut st, dest, size, 30.0, false));
        assert_eq!(st.drag_handle, None);
        assert!(
            core.status.contains("out tangent set"),
            "status: {}",
            core.status
        );

        // A cancelled handle drag rolls back through the Down
        // snapshot: out_tan returns to −25 (the handle has
        // moved with the new tangent, so re-derive its pixel).
        let (hf2, hv2) =
            tangent_handle_data(&core.project.channels[0], 1, TangentSide::Out).unwrap();
        let view2 = drag_view(&core, ChannelTarget::RootX, size);
        let grab2 = view2.to_px(hf2, hv2);
        assert!(drag_down(&mut core, &mut st, grab2, size));
        let dest2 = view2.to_px(6.0, 130.0);
        assert!(drag_move(&mut core, &mut st, dest2, size));
        assert!(approx(core.project.channels[0].keys[1].out_tan, 25.0));
        assert!(drag_up(&mut core, &mut st, dest2, size, 30.0, true));
        assert_eq!(st.drag_handle, None);
        assert!(
            approx(core.project.channels[0].keys[1].out_tan, -25.0),
            "cancel restored the pre-drag tangent"
        );
    }

    /// A two-figure core whose project carries four channels
    /// across both figures — the channel-list fixture. Figure
    /// 0 stays selected; figure 1 exists for real (via the
    /// core's own add op) so `select_figure` can reach it.
    fn core_with_project_channels() -> EditorCore {
        let mut core = EditorCore::new();
        core.add_figure();
        core.select_figure(0);
        core.project.channels = vec![
            channel(
                ChannelTarget::RootX,
                vec![
                    Key::flat(0.0, 0.0),
                    Key::flat(2.0, 10.0),
                    Key::flat(4.0, 0.0),
                ],
            ),
            channel(ChannelTarget::Angle(2), vec![Key::flat(1.0, 30.0)]),
            Channel {
                figure: 1,
                target: ChannelTarget::RootY,
                keys: vec![],
            },
            Channel {
                figure: 1,
                target: ChannelTarget::Length(4),
                keys: vec![Key::flat(0.0, 90.0), Key::flat(3.0, 95.0)],
            },
        ];
        core
    }

    #[test]
    fn channel_rows_list_every_channel_with_label_and_count() {
        let core = core_with_project_channels();
        // Project order, 1-based figure numbers, the panel's
        // own target labels, exact key counts — including
        // figure 1's channels while figure 0 is selected
        // (the list is project-wide, not selection-scoped),
        // and an empty channel still lists with 0 keys.
        assert_eq!(
            channel_rows(&core),
            vec![
                (0, "F1 Root X".to_string(), 3),
                (1, "F1 Angle 2".to_string(), 1),
                (2, "F2 Root Y".to_string(), 0),
                (3, "F2 Length 4".to_string(), 2),
            ]
        );
    }

    #[test]
    fn channel_rows_empty_project_is_empty() {
        // The demo core has no channels: no rows (the builder
        // shows the empty-state line instead).
        let core = EditorCore::new();
        assert!(channel_rows(&core).is_empty());
    }

    #[test]
    fn target_for_channel_maps_row_index_to_target() {
        let core = core_with_project_channels();
        assert_eq!(target_for_channel(&core, 0), Some(ChannelTarget::RootX));
        assert_eq!(target_for_channel(&core, 1), Some(ChannelTarget::Angle(2)));
        assert_eq!(target_for_channel(&core, 3), Some(ChannelTarget::Length(4)));
        assert_eq!(target_for_channel(&core, 4), None);
        assert_eq!(target_for_channel(&core, 99), None);
    }

    #[test]
    fn select_channel_points_the_existing_plumbing_at_the_row() {
        let mut core = core_with_project_channels();
        let mut st = GraphPanelState::default();
        // Row 3 is figure 1's Length(4) channel: the press
        // selects figure 1 AND writes the same target state
        // the chips write.
        assert!(select_channel(&mut core, &mut st, 3));
        assert_eq!(core.selected_figure, 1);
        assert_eq!(st.target, Some(ChannelTarget::Length(4)));
        // Phase J: the row's index is recorded too, and the
        // index resolution lands on exactly the listed
        // channel — this is the channel the canvas draws
        // and the transforms edit.
        assert_eq!(st.selected_channel, Some(3));
        assert_eq!(effective_channel_index(&core, &st), Some(3));
        let target = effective_target(&core, st.target);
        assert_eq!(target, ChannelTarget::Length(4));
        assert_eq!(channel_index(&core, target), Some(3));
        // Proof by edit: scaling the effective target moves
        // the listed channel's keys (90 -> 180) and leaves
        // the other figure's channel alone.
        apply_scale(&mut core, target, 2.0);
        assert_eq!(core.project.channels[3].keys[0].value, 180.0);
        assert_eq!(core.project.channels[0].keys[1].value, 10.0);
    }

    #[test]
    fn select_channel_same_figure_keeps_the_node_selection() {
        let mut core = core_with_project_channels();
        core.selected_node = Some(2);
        let mut st = GraphPanelState::default();
        // Row 1 is figure 0's own Angle(2): no figure switch,
        // so the node selection survives (select_figure,
        // which would clear it, is never called).
        assert!(select_channel(&mut core, &mut st, 1));
        assert_eq!(core.selected_figure, 0);
        assert_eq!(core.selected_node, Some(2));
        assert_eq!(st.target, Some(ChannelTarget::Angle(2)));
        assert_eq!(st.selected_channel, Some(1));
        assert_eq!(channel_index(&core, ChannelTarget::Angle(2)), Some(1));
    }

    #[test]
    fn select_channel_out_of_range_changes_nothing() {
        let mut core = core_with_project_channels();
        let mut st = GraphPanelState::default();
        assert!(!select_channel(&mut core, &mut st, 99));
        assert_eq!(core.selected_figure, 0);
        assert_eq!(st.target, None);
        assert_eq!(st.selected_channel, None);
    }

    /// A fabricated stat, so the formatting tests never
    /// depend on `graph_keys::channel_stats` landing.
    fn stat(
        figure_name: &str,
        target_label: &str,
        keys: usize,
        min: f32,
        max: f32,
    ) -> ChannelStat {
        ChannelStat {
            figure: 0,
            figure_name: figure_name.to_string(),
            target_label: target_label.to_string(),
            keys,
            min,
            max,
            first_frame: 0.0,
            last_frame: 0.0,
        }
    }

    #[test]
    fn stat_line_formats_name_target_keys_and_range() {
        // Name and target verbatim, key count plain, range
        // at one decimal — and the frames the stat carries
        // are NOT in the line (the glanceable part only).
        let s = stat("Styk", "Angle 3", 5, -12.34, 90.0);
        assert_eq!(stat_line(&s), "Styk · Angle 3: 5 keys, -12.3..90.0");
        // A wholly negative range keeps both signs.
        let s = stat("Figure 2", "Root Y", 2, -3.26, -1.04);
        assert_eq!(stat_line(&s), "Figure 2 · Root Y: 2 keys, -3.3..-1.0");
    }

    #[test]
    fn stat_line_empty_channel_prints_zeroes() {
        // channel_stats reports an empty channel as zeroes;
        // the line prints them without a special case.
        let s = stat("Styk", "Root X", 0, 0.0, 0.0);
        assert_eq!(stat_line(&s), "Styk · Root X: 0 keys, 0.0..0.0");
    }

    #[test]
    fn stat_rows_flag_only_the_drawn_channel() {
        let mut core = EditorCore::new();
        core.project.channels = vec![
            channel(
                ChannelTarget::RootX,
                vec![Key::flat(0.0, 0.0), Key::flat(2.0, 10.0)],
            ),
            channel(ChannelTarget::RootY, vec![Key::flat(1.0, 5.0)]),
        ];
        // The picked target's row is flagged; the other is
        // not. Lines carry the same numbers channel_stats
        // reports (2 keys over 0.0..10.0 here).
        let st = GraphPanelState {
            target: Some(ChannelTarget::RootX),
            ..Default::default()
        };
        let rows = stat_rows(&core, &st);
        assert_eq!(rows.len(), 2);
        assert!(rows[0].0.contains("Root X: 2 keys, 0.0..10.0"), "{}", rows[0].0);
        assert!(rows[0].1, "the drawn channel is flagged");
        assert!(!rows[1].1);
        // No pick: the fallback drawn channel (the figure's
        // first) is the flagged one — the block highlights
        // what the canvas actually draws.
        let rows = stat_rows(&core, &GraphPanelState::default());
        assert!(rows[0].1 && !rows[1].1);
    }

    #[test]
    fn stat_rows_duplicates_flag_the_first_match_only() {
        let mut core = EditorCore::new();
        // Two channels with the SAME (figure, target), and a
        // TARGET-ONLY state (no row pick): the legacy
        // resolution flags the first match only. (A row
        // pick flags the picked duplicate instead — see
        // duplicate_second_row_is_drawn_highlighted_and_edited.)
        core.project.channels = vec![
            channel(ChannelTarget::RootX, vec![Key::flat(0.0, 1.0)]),
            channel(
                ChannelTarget::RootX,
                vec![Key::flat(0.0, 2.0), Key::flat(3.0, 4.0)],
            ),
        ];
        let st = GraphPanelState {
            target: Some(ChannelTarget::RootX),
            ..Default::default()
        };
        let rows = stat_rows(&core, &st);
        assert_eq!(rows.len(), 2, "both duplicates still list");
        assert!(rows[0].1, "first match highlighted");
        assert!(!rows[1].1, "the shadowed duplicate is not");
    }

    #[test]
    fn stat_rows_empty_without_channels() {
        // The demo core has no channels: no rows, so the
        // builder shows no stats block at all.
        let core = EditorCore::new();
        assert!(stat_rows(&core, &GraphPanelState::default()).is_empty());
    }

    #[test]
    fn stats_bound_constants_agree() {
        // The Wave 10 cap, pinned both ways: six lines,
        // and the height those six lines cost at
        // STAT_LINE_H with the block's 2 pt gap between
        // them is exactly STATS_MAX_H — the constant the
        // docs quote as the block's maximum height.
        assert_eq!(STATS_MAX_LINES, 6);
        assert_eq!(STATS_MAX_H, 100.0);
        let height = STATS_MAX_LINES as f32 * STAT_LINE_H
            + (STATS_MAX_LINES as f32 - 1.0) * 2.0;
        assert_eq!(height, STATS_MAX_H);
    }

    #[test]
    fn stats_more_line_counts_and_points_at_the_list() {
        // The summary is honest both ways: the exact
        // folded count, singular and plural, and the way
        // back to a folded channel's stats (a list row
        // press draws it — select_channel).
        assert_eq!(
            stats_more_line(1),
            "+1 more channel — pick it in the list above to see its stats"
        );
        assert_eq!(
            stats_more_line(7),
            "+7 more channels — pick one in the list above to see its stats"
        );
    }

    /// Ten channels on figure 0 — Angle(0) … Angle(9),
    /// one flat key each — the synthetic many-channel
    /// state the Wave 10 fold tests share.
    fn ten_channel_core() -> EditorCore {
        let mut core = EditorCore::new();
        core.project.channels = (0..10)
            .map(|i| {
                channel(
                    ChannelTarget::Angle(i),
                    vec![Key::flat(0.0, i as f32)],
                )
            })
            .collect();
        core
    }

    #[test]
    fn stat_display_rows_under_the_cap_are_every_row() {
        // Three channels (≤ STATS_MAX_LINES): the
        // projection is every row in project order, no
        // summary — the block renders exactly as it did
        // before the bound.
        let mut core = EditorCore::new();
        core.project.channels = vec![
            channel(ChannelTarget::RootX, vec![Key::flat(0.0, 0.0)]),
            channel(ChannelTarget::RootY, vec![Key::flat(0.0, 1.0)]),
            channel(ChannelTarget::Angle(2), vec![Key::flat(0.0, 2.0)]),
        ];
        let display = stat_display_rows(&core, &GraphPanelState::default());
        assert_eq!(display.len(), 3);
        for (want, row) in display.iter().enumerate() {
            match row {
                StatDisplay::Line { index, drawn, .. } => {
                    assert_eq!(*index, want, "project order, channel indices");
                    assert_eq!(*drawn, want == 0, "the fallback drawn row is flagged");
                }
                StatDisplay::More { .. } => panic!("no summary under the cap"),
            }
        }
        // And no channels at all: no rows, so the
        // builder still shows no block.
        let core = EditorCore::new();
        assert!(stat_display_rows(&core, &GraphPanelState::default()).is_empty());
    }

    #[test]
    fn stat_display_rows_fold_the_overflow_and_keep_the_drawn_line() {
        // Ten channels with the LAST-but-one picked as
        // drawn (a list row press — select_channel): the
        // block holds at STATS_MAX_LINES rendered lines —
        // the first STATS_MAX_LINES - 2 other lines in
        // project order, the drawn channel's own line in
        // full, and the summary counting the folded five.
        let mut core = ten_channel_core();
        let mut st = GraphPanelState::default();
        assert!(select_channel(&mut core, &mut st, 8));
        let display = stat_display_rows(&core, &st);
        assert_eq!(display.len(), STATS_MAX_LINES, "the block is capped");
        let mut indices: Vec<usize> = Vec::new();
        let mut drawn_at: Option<usize> = None;
        for row in &display {
            match row {
                StatDisplay::Line { index, drawn, .. } => {
                    indices.push(*index);
                    if *drawn {
                        drawn_at = Some(*index);
                    }
                }
                StatDisplay::More { .. } => {}
            }
        }
        assert_eq!(indices, vec![0, 1, 2, 3, 8], "order kept, drawn kept");
        assert_eq!(drawn_at, Some(8), "the drawn line is the picked channel's");
        match display.last() {
            Some(StatDisplay::More { hidden }) => {
                assert_eq!(*hidden, 5, "ten channels, five lines shown");
                assert_eq!(
                    stats_more_line(*hidden),
                    "+5 more channels — pick one in the list above to see its stats"
                );
            }
            other => panic!("the last row is the summary, got {other:?}"),
        }
        // The folded set is exactly the complement: the
        // summary never claims a shown channel, and
        // picking a folded channel (index 6) draws it —
        // its line returns in full and the fold moves.
        let mut st = GraphPanelState::default();
        assert!(select_channel(&mut core, &mut st, 6));
        let display = stat_display_rows(&core, &st);
        assert!(display.iter().any(|row| matches!(
            row,
            StatDisplay::Line { index: 6, drawn: true, .. }
        )));
    }

    #[test]
    fn stat_display_rows_fold_without_a_drawn_channel() {
        // Channels exist but none is drawn (the selected
        // figure has none — the stats cover ALL figures):
        // the fold still caps the block, one line under
        // the cap because no drawn line claims a slot,
        // and the summary counts the rest.
        let mut core = ten_channel_core();
        core.selected_figure = 1;
        let display = stat_display_rows(&core, &GraphPanelState::default());
        assert_eq!(display.len(), STATS_MAX_LINES - 1);
        assert!(display.iter().all(|row| !matches!(
            row,
            StatDisplay::Line { drawn: true, .. }
        )));
        match display.last() {
            Some(StatDisplay::More { hidden }) => assert_eq!(*hidden, 6),
            other => panic!("the last row is the summary, got {other:?}"),
        }
    }

    #[test]
    fn value_line_reports_the_flat_value_at_the_playhead() {
        // Flat keys (Key::flat: Hermite, zero tangents):
        // at the segment midpoint the tangent terms vanish
        // and the basis weights are both 0.5, so frame 2
        // evaluates to exactly (10.0 + 30.0) / 2 = 20.0 —
        // reported exactly, frame 1-based.
        let mut core = EditorCore::new();
        core.project.channels = vec![channel(
            ChannelTarget::RootX,
            vec![Key::flat(0.0, 10.0), Key::flat(4.0, 30.0)],
        )];
        core.goto(2);
        assert_eq!(
            value_line(&core, &GraphPanelState::default()).as_deref(),
            Some("Value at frame 3: 20.0")
        );
        // At a key frame the stored value comes back
        // exactly (value_at's contract).
        core.goto(0);
        assert_eq!(
            value_line(&core, &GraphPanelState::default()).as_deref(),
            Some("Value at frame 1: 10.0")
        );
    }

    #[test]
    fn value_line_is_none_without_a_drawn_channel() {
        // The demo core has no channels: nothing is drawn,
        // so there is no line.
        let core = EditorCore::new();
        assert_eq!(value_line(&core, &GraphPanelState::default()), None);
    }

    #[test]
    fn joint_limit_note_names_the_range_for_a_limited_angle_channel() {
        // The demo figure is the humanoid: node 4 carries
        // the constructor's elbow range (-10..160).
        let core = EditorCore::new();
        let ch = channel(ChannelTarget::Angle(4), vec![Key::flat(0.0, 170.0)]);
        assert_eq!(
            joint_limit_note(&core, &ch).as_deref(),
            Some("joint limits -10..160")
        );
        // An unlimited node on the same figure: no note.
        let ch = channel(ChannelTarget::Angle(1), vec![Key::flat(0.0, 10.0)]);
        assert_eq!(joint_limit_note(&core, &ch), None);
        // A non-Angle target on the limited node: no note.
        let ch = channel(ChannelTarget::Length(4), vec![Key::flat(0.0, 10.0)]);
        assert_eq!(joint_limit_note(&core, &ch), None);
        // A stale figure index resolves to no note,
        // never a panic.
        let ch = Channel {
            figure: 99,
            target: ChannelTarget::Angle(4),
            keys: vec![],
        };
        assert_eq!(joint_limit_note(&core, &ch), None);
    }

    #[test]
    fn joint_limit_note_reads_an_open_side_as_off() {
        // One bound set by hand on an unlimited node:
        // the open side reads "Off", the segment
        // inspector's word for an unset bound.
        let mut core = EditorCore::new();
        core.project.figures[0].def.nodes[1].max_angle = Some(45.0);
        let ch = channel(ChannelTarget::Angle(1), vec![Key::flat(0.0, 10.0)]);
        assert_eq!(
            joint_limit_note(&core, &ch).as_deref(),
            Some("joint limits Off..45")
        );
    }

    #[test]
    fn value_line_carries_the_limit_note_for_a_limited_angle_channel() {
        // Angle(4) keyed to 170 on the demo humanoid
        // (elbow range -10..160): the raw value stays
        // first and unchanged, and the note names the
        // range evaluation will clamp it into — the
        // line no longer reads as if 170 is what
        // will apply.
        let mut core = EditorCore::new();
        core.project.channels = vec![channel(
            ChannelTarget::Angle(4),
            vec![Key::flat(0.0, 170.0)],
        )];
        core.goto(0);
        assert_eq!(
            value_line(&core, &GraphPanelState::default()).as_deref(),
            Some("Value at frame 1: 170.0 · joint limits -10..160")
        );
    }

    #[test]
    fn stat_rows_carry_the_limit_note_only_for_limited_angle_channels() {
        let mut core = EditorCore::new();
        core.project.channels = vec![
            channel(ChannelTarget::Angle(4), vec![Key::flat(0.0, 170.0)]),
            channel(ChannelTarget::Angle(1), vec![Key::flat(0.0, 10.0)]),
            channel(ChannelTarget::RootX, vec![Key::flat(0.0, 5.0)]),
        ];
        let rows = stat_rows(&core, &GraphPanelState::default());
        assert_eq!(rows.len(), 3);
        assert!(
            rows[0].0.ends_with("· joint limits -10..160"),
            "limited row carries the note: {}",
            rows[0].0
        );
        assert!(
            !rows[1].0.contains("joint limits"),
            "unlimited row unchanged: {}",
            rows[1].0
        );
        assert!(
            !rows[2].0.contains("joint limits"),
            "root row unchanged: {}",
            rows[2].0
        );
    }

    #[test]
    fn value_line_follows_the_playhead() {
        // The same channel read at two playhead positions
        // reports each frame's own value — the line tracks
        // goto, it is not a static channel summary.
        let mut core = EditorCore::new();
        core.project.channels = vec![channel(
            ChannelTarget::RootX,
            vec![
                Key::flat(0.0, 0.0).with_interp(Interp::Linear),
                Key::flat(4.0, 40.0),
            ],
        )];
        let st = GraphPanelState::default();
        core.goto(1);
        assert_eq!(
            value_line(&core, &st).as_deref(),
            Some("Value at frame 2: 10.0")
        );
        core.goto(3);
        assert_eq!(
            value_line(&core, &st).as_deref(),
            Some("Value at frame 4: 30.0")
        );
    }

    #[test]
    fn value_line_reports_the_picked_duplicate() {
        // Two (figure, target) twins: a row pick makes the
        // SECOND the drawn channel, and the line reports
        // its value (2.0), not the first match's (1.0) —
        // the resolution is effective_channel_index's.
        let mut core = EditorCore::new();
        core.project.channels = vec![
            channel(ChannelTarget::RootX, vec![Key::flat(0.0, 1.0)]),
            channel(ChannelTarget::RootX, vec![Key::flat(0.0, 2.0)]),
        ];
        let st = GraphPanelState {
            target: Some(ChannelTarget::RootX),
            selected_channel: Some(1),
            ..Default::default()
        };
        assert_eq!(
            value_line(&core, &st).as_deref(),
            Some("Value at frame 1: 2.0")
        );
    }

    /// A core whose project carries two channels with the
    /// SAME `(figure, target)` — the duplicate fixture for
    /// the Phase J tests. The twins are told apart by their
    /// keys: the first has one key at value 1.0, the second
    /// two keys at values 2.0 and 4.0.
    fn core_with_duplicate_channels() -> EditorCore {
        let mut core = EditorCore::new();
        core.project.channels = vec![
            channel(ChannelTarget::RootX, vec![Key::flat(0.0, 1.0)]),
            channel(
                ChannelTarget::RootX,
                vec![Key::flat(0.0, 2.0), Key::flat(3.0, 4.0)],
            ),
        ];
        core
    }

    #[test]
    fn duplicate_second_row_is_drawn_highlighted_and_edited() {
        let mut core = core_with_duplicate_channels();
        let mut st = GraphPanelState::default();
        // The legacy resolver still sees only the first
        // twin — that is the limit being fixed, kept visible
        // here so the contrast is pinned, not hidden.
        assert_eq!(channel_index(&core, ChannelTarget::RootX), Some(0));
        // Press the SECOND row: the index is recorded, the
        // target stays in sync, and the effective resolution
        // returns the second channel.
        assert!(select_channel(&mut core, &mut st, 1));
        assert_eq!(st.selected_channel, Some(1));
        assert_eq!(st.target, Some(ChannelTarget::RootX));
        assert_eq!(effective_channel_index(&core, &st), Some(1));
        // The drawn channel is the second twin (two keys,
        // not one) — the builder reads exactly this index.
        let drawn = effective_channel_index(&core, &st).unwrap();
        assert_eq!(core.project.channels[drawn].keys.len(), 2);
        // The stats highlight follows the pick: the second
        // row is flagged, the first no longer is.
        let rows = stat_rows(&core, &st);
        assert_eq!(rows.len(), 2);
        assert!(!rows[0].1, "the first twin is not highlighted");
        assert!(rows[1].1, "the picked twin is highlighted");
        // Proof by edit: scaling by index moves the second
        // twin's keys (2 -> 4) and leaves the first alone.
        apply_scale_at(&mut core, drawn, 2.0);
        assert_eq!(core.project.channels[1].keys[0].value, 4.0);
        assert_eq!(core.project.channels[0].keys[0].value, 1.0);
    }

    #[test]
    fn duplicate_second_row_key_drag_edits_the_second_channel() {
        let mut core = core_with_duplicate_channels();
        let mut st = GraphPanelState::default();
        assert!(select_channel(&mut core, &mut st, 1));
        let size = [480.0, 170.0];
        // Fit the SECOND twin through the panel's own
        // index-based view and grab its frame-3 key.
        let view = view_for(&core, Some(&core.project.channels[1]), size);
        let grab = view.to_px(3.0, 4.0);
        assert!(drag_down(&mut core, &mut st, grab, size));
        assert_eq!(st.drag_key, Some(1));
        // Move it to data (2, 9): the second twin's key
        // follows; the first twin is untouched.
        let dest = view.to_px(2.0, 9.0);
        assert!(drag_move(&mut core, &mut st, dest, size));
        assert!(drag_up(&mut core, &mut st, dest, size, 40.0, false));
        let keys = &core.project.channels[1].keys;
        assert!(
            keys.iter()
                .any(|k| approx(k.frame, 2.0) && approx(k.value, 9.0)),
            "second twin's key moved: {keys:?}"
        );
        assert_eq!(core.project.channels[0].keys[0].value, 1.0);
    }

    #[test]
    fn duplicate_channel_to_next_clones_onto_the_second_figure() {
        // Integrator wiring (Phase J): two figures, one
        // RootX channel on figure 0 — the press appends the
        // clone onto figure 1 through the graph_keys op.
        let mut core = EditorCore::new();
        core.add_figure();
        assert_eq!(core.project.figures.len(), 2);
        // add_figure selects the new figure; the channel
        // under test lives on figure 0, and channel
        // resolution scopes to the selected figure.
        core.selected_figure = 0;
        core.project.channels = vec![channel(
            ChannelTarget::RootX,
            vec![Key::flat(0.0, 1.0), Key::flat(3.0, 4.0)],
        )];
        let st = GraphPanelState {
            target: Some(ChannelTarget::RootX),
            ..Default::default()
        };
        let depth = core.undo_depth();
        duplicate_channel_to_next(&mut core, &st);
        assert_eq!(core.project.channels.len(), 2);
        let clone = &core.project.channels[1];
        assert_eq!(clone.figure, 1);
        assert_eq!(clone.target, ChannelTarget::RootX);
        assert_eq!(clone.keys.len(), 2);
        assert_eq!(core.undo_depth(), depth + 1, "one snapshot via the op");
        // A second press refuses: figure 1 already has the
        // channel (the op's own wording, no snapshot).
        duplicate_channel_to_next(&mut core, &st);
        assert_eq!(core.project.channels.len(), 2);
        assert_eq!(core.undo_depth(), depth + 1);
    }

    #[test]
    fn duplicate_channel_to_next_refuses_with_one_figure() {
        let mut core = EditorCore::new();
        core.project.channels = vec![channel(
            ChannelTarget::RootX,
            vec![Key::flat(0.0, 1.0)],
        )];
        let st = GraphPanelState {
            target: Some(ChannelTarget::RootX),
            ..Default::default()
        };
        let depth = core.undo_depth();
        duplicate_channel_to_next(&mut core, &st);
        assert_eq!(core.project.channels.len(), 1);
        assert_eq!(core.undo_depth(), depth, "no snapshot on the refusal");
        assert!(core.status.contains("Only one figure"));
    }

    #[test]
    fn selection_after_delete_covers_all_four_cases() {
        // Phase K: the pure repair the delete handler
        // applies to the stored pick.
        // No pick stays no pick, wherever the row was.
        assert_eq!(selection_after_delete(None, 0), None);
        assert_eq!(selection_after_delete(None, 3), None);
        // A pick before the deleted row is unmoved.
        assert_eq!(selection_after_delete(Some(0), 1), Some(0));
        assert_eq!(selection_after_delete(Some(1), 3), Some(1));
        // A pick on the deleted row is cleared — the
        // panel falls back to target resolution.
        assert_eq!(selection_after_delete(Some(0), 0), None);
        assert_eq!(selection_after_delete(Some(2), 2), None);
        // A pick after the deleted row shifts down one,
        // still naming the same channel.
        assert_eq!(selection_after_delete(Some(1), 0), Some(0));
        assert_eq!(selection_after_delete(Some(3), 1), Some(2));
    }

    #[test]
    fn delete_drawn_channel_removes_the_picked_twin_and_clears_the_pick() {
        // Phase K: pick the SECOND of the duplicate twins
        // and delete — that twin goes, the first survives,
        // the pick is cleared and exactly one undo step is
        // taken (the op's snapshot).
        let mut core = core_with_duplicate_channels();
        let mut st = GraphPanelState::default();
        assert!(select_channel(&mut core, &mut st, 1));
        assert_eq!(st.selected_channel, Some(1));
        let depth = core.undo_depth();
        delete_drawn_channel(&mut core, &mut st);
        assert_eq!(core.project.channels.len(), 1);
        // The survivor is the FIRST twin (one key, 1.0).
        assert_eq!(core.project.channels[0].keys.len(), 1);
        assert_eq!(core.project.channels[0].keys[0].value, 1.0);
        assert_eq!(st.selected_channel, None, "pick on the deleted row clears");
        assert_eq!(st.target, Some(ChannelTarget::RootX), "target stays as-is");
        assert_eq!(core.undo_depth(), depth + 1, "one snapshot via the op");
    }

    #[test]
    fn delete_drawn_channel_shifts_a_pick_past_the_deleted_row() {
        // Phase K: the stored pick names row 1 — a
        // channel of figure 1 — while figure 0 is
        // selected, so the pick is stale and the DRAWN
        // channel is figure 0's row 0 (target fallback;
        // the pick goes stale exactly this way when the
        // user switches figure in the figures panel).
        // Deleting row 0 must shift the pick to Some(0),
        // still naming figure 1's channel.
        let mut core = EditorCore::new();
        let mut other = channel(ChannelTarget::RootX, vec![Key::flat(0.0, 5.0)]);
        other.figure = 1;
        core.project.channels = vec![
            channel(ChannelTarget::RootX, vec![Key::flat(0.0, 1.0)]),
            other,
        ];
        let mut st = GraphPanelState {
            target: Some(ChannelTarget::RootX),
            selected_channel: Some(1),
            ..Default::default()
        };
        assert_eq!(
            effective_channel_index(&core, &st),
            Some(0),
            "premise: the stale pick falls back to row 0"
        );
        let depth = core.undo_depth();
        delete_drawn_channel(&mut core, &mut st);
        assert_eq!(core.project.channels.len(), 1);
        assert_eq!(core.project.channels[0].figure, 1, "figure 1's channel survives");
        assert_eq!(st.selected_channel, Some(0), "the pick shifts down one row");
        assert_eq!(core.undo_depth(), depth + 1, "one snapshot via the op");
    }

    #[test]
    fn delete_drawn_channel_without_channels_changes_nothing() {
        // Phase K refusal: the demo core has no channels,
        // so nothing is drawn — no removal, no snapshot,
        // no pick, only the refusal wording.
        let mut core = EditorCore::new();
        let mut st = GraphPanelState::default();
        let depth = core.undo_depth();
        delete_drawn_channel(&mut core, &mut st);
        assert!(core.project.channels.is_empty());
        assert_eq!(core.undo_depth(), depth, "no snapshot on the refusal");
        assert_eq!(st.selected_channel, None);
        assert!(core.status.contains("No graph channel selected to delete"));
    }

    #[test]
    fn shift_drawn_channel_moves_the_picked_twin_only() {
        // Phase M: the shift handler resolves through the
        // SAME helper as Delete — pick the second of the
        // duplicate twins and shift: only that twin's keys
        // move (0/3 -> 1/4), the first twin is untouched,
        // and exactly one undo step is taken (the op's
        // snapshot). Delete on an identical setup removes
        // that same index (the Phase K test above), so the
        // two buttons can never disagree about their
        // channel.
        let mut core = core_with_duplicate_channels();
        let mut st = GraphPanelState::default();
        assert!(select_channel(&mut core, &mut st, 1));
        assert_eq!(effective_channel_index(&core, &st), Some(1));
        let depth = core.undo_depth();
        shift_drawn_channel(&mut core, &st, 1.0);
        let frames: Vec<f32> = core.project.channels[1]
            .keys
            .iter()
            .map(|k| k.frame)
            .collect();
        assert_eq!(frames, vec![1.0, 4.0]);
        let frames: Vec<f32> = core.project.channels[0]
            .keys
            .iter()
            .map(|k| k.frame)
            .collect();
        assert_eq!(frames, vec![0.0], "the first twin never moves");
        assert_eq!(core.undo_depth(), depth + 1, "one snapshot via the op");
        assert_eq!(st.selected_channel, Some(1), "shifting keeps the pick");
    }

    #[test]
    fn shift_drawn_channel_moves_keys_by_the_delta() {
        // Phase M, the buttons' own handler path: a
        // channel keyed at frames 2 and 5 shifts +1 to 3
        // and 6 (the task's pinned case); presses
        // accumulate, and one -1 press walks it back.
        let mut core = EditorCore::new();
        core.project.channels = vec![channel(
            ChannelTarget::RootX,
            vec![Key::flat(2.0, 1.0), Key::flat(5.0, 2.0)],
        )];
        let st = GraphPanelState {
            target: Some(ChannelTarget::RootX),
            ..Default::default()
        };
        let frames = |core: &EditorCore| -> Vec<f32> {
            core.project.channels[0]
                .keys
                .iter()
                .map(|k| k.frame)
                .collect()
        };
        let depth = core.undo_depth();
        shift_drawn_channel(&mut core, &st, 1.0);
        assert_eq!(frames(&core), vec![3.0, 6.0]);
        shift_drawn_channel(&mut core, &st, 1.0);
        assert_eq!(frames(&core), vec![4.0, 7.0], "presses accumulate");
        shift_drawn_channel(&mut core, &st, -1.0);
        assert_eq!(frames(&core), vec![3.0, 6.0]);
        assert_eq!(core.undo_depth(), depth + 3, "one snapshot per shift");
        assert!(core.status.contains("Shifted 2 keys"));
    }

    #[test]
    fn shift_drawn_channel_refuses_at_the_frame_zero_wall() {
        // Phase M: the op refuses the WHOLE shift when
        // any key would land before frame 0 — keys
        // unmoved, no snapshot, the op's own wording. A
        // key landing exactly ON frame 0 is fine.
        let mut core = EditorCore::new();
        core.project.channels = vec![channel(
            ChannelTarget::RootX,
            vec![Key::flat(2.0, 1.0), Key::flat(5.0, 2.0)],
        )];
        let st = GraphPanelState {
            target: Some(ChannelTarget::RootX),
            ..Default::default()
        };
        let frames = |core: &EditorCore| -> Vec<f32> {
            core.project.channels[0]
                .keys
                .iter()
                .map(|k| k.frame)
                .collect()
        };
        let depth = core.undo_depth();
        shift_drawn_channel(&mut core, &st, -3.0);
        assert_eq!(frames(&core), vec![2.0, 5.0], "a refused shift moves nothing");
        assert_eq!(core.undo_depth(), depth, "no snapshot on the refusal");
        assert!(core.status.contains("before frame 1"));
        shift_drawn_channel(&mut core, &st, -2.0);
        assert_eq!(frames(&core), vec![0.0, 3.0], "landing on frame 0 is allowed");
        assert_eq!(core.undo_depth(), depth + 1);
    }

    #[test]
    fn shift_drawn_channel_without_channels_changes_nothing() {
        // Phase M refusal, mirroring Delete's: the demo
        // core has no channels, so nothing is drawn — no
        // movement, no snapshot, only the refusal wording.
        let mut core = EditorCore::new();
        let st = GraphPanelState::default();
        let depth = core.undo_depth();
        shift_drawn_channel(&mut core, &st, 1.0);
        assert!(core.project.channels.is_empty());
        assert_eq!(core.undo_depth(), depth, "no snapshot on the refusal");
        assert!(core.status.contains("No graph channel selected to shift"));
    }

    #[test]
    fn stale_selected_channel_falls_back_to_target_resolution() {
        let mut core = core_with_duplicate_channels();
        // An index past the end of the channel list (the
        // channel was deleted elsewhere): the legacy target
        // resolution takes over — the first match.
        let st = GraphPanelState {
            target: Some(ChannelTarget::RootX),
            selected_channel: Some(99),
            ..Default::default()
        };
        assert_eq!(effective_channel_index(&core, &st), Some(0));
        let rows = stat_rows(&core, &st);
        assert!(rows[0].1 && !rows[1].1, "fallback highlight: {rows:?}");
        // The same fallback after a REAL deletion: pick the
        // second row, then remove that channel — the index
        // now names nothing, and resolution lands back on
        // the surviving first twin.
        let mut st = GraphPanelState::default();
        assert!(select_channel(&mut core, &mut st, 1));
        core.project.channels.truncate(1);
        assert_eq!(effective_channel_index(&core, &st), Some(0));
        // And when nothing can resolve at all — no channels
        // left — the answer is None and no row highlights.
        core.project.channels.clear();
        assert_eq!(effective_channel_index(&core, &st), None);
        assert!(stat_rows(&core, &st).is_empty());
    }

    #[test]
    fn selected_channel_for_another_figure_is_stale() {
        // The index is figure-scoped, exactly like
        // channel_index: a pick that names figure 1's
        // channel while figure 0 is selected does not draw
        // it — the target fallback for the SELECTED figure
        // decides instead.
        let mut core = core_with_project_channels();
        let st = GraphPanelState {
            target: Some(ChannelTarget::RootX),
            selected_channel: Some(3),
            ..Default::default()
        };
        // Channel 3 is figure 1's Length(4); figure 0 is
        // selected, so the fallback resolves figure 0's
        // Root X channel (index 0).
        assert_eq!(core.selected_figure, 0);
        assert_eq!(effective_channel_index(&core, &st), Some(0));
    }

    #[test]
    fn legacy_target_only_state_resolves_as_before() {
        // No row pick anywhere in this state: the Phase G
        // behaviour is byte-for-byte the fallback — the
        // effective target's first match, for both the
        // index resolution and the stats highlight.
        let core = core_with_duplicate_channels();
        let st = GraphPanelState {
            target: Some(ChannelTarget::RootX),
            ..Default::default()
        };
        assert_eq!(st.selected_channel, None);
        assert_eq!(effective_channel_index(&core, &st), Some(0));
        let rows = stat_rows(&core, &st);
        assert!(rows[0].1 && !rows[1].1);
        // And a wholly default state still falls back to
        // the figure's first channel.
        assert_eq!(
            effective_channel_index(&core, &GraphPanelState::default()),
            Some(0)
        );
    }
}
