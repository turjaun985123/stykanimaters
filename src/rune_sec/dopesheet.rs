//! The dopesheet lane for the Rune editor shell (UI
//! mission Wave 10) — the per-figure keyframe sheet the
//! original app had (the GPUI era's `ui_dope.rs`) and the
//! GPUI→Rune migration never rebuilt: the engine half
//! survived (`stykanimate_rs::stats::figure_keyframes`,
//! whose docs still cite the old sheet's rule), the panel
//! did not. This module rebuilds it as a lane INSIDE the
//! timeline card, under the marker row and the clips lane.
//!
//! Integrator wiring (main.rs / timeline_marks.rs /
//! rune_sec/mod.rs are NOT edited here): declare
//! `pub mod dopesheet;` in `rune_sec/mod.rs`, and in
//! `timeline_marks::timeline_marks` append
//! `dopesheet_lane(s, t)` to the scrolled `content`
//! column after the clips lane, gated on the Animate
//! workspace (the intent — the lane is an Animate tool):
//!
//! ```ignore
//! if s.workspace == crate::rune_sec::workspaces::Workspace::Animate {
//!     content = content.child(crate::rune_sec::dopesheet::dopesheet_lane(s, t));
//! }
//! ```
//!
//! ## Scroll sharing (the mechanism, exactly)
//!
//! The timeline has exactly ONE horizontal scroll
//! viewport: `timeline_marks` wraps one content column
//! (ruler, chips, marker row, clips lane) in
//! `row().key("timeline").child(content).scroll_x()`.
//! There is no per-row scroll offset anywhere — rows
//! share the offset by construction, because they are
//! children of the single translated column. This lane
//! uses the same mechanism and nothing else: it is
//! appended as the column's last child, so it translates
//! with the chips at any scroll offset, and its cells
//! are laid out from the same content x = 0 basis in the
//! chips' own geometry ([`CHIP_W`] wide, [`CHIP_GAP`]
//! apart — the pitch is imported from `timeline_marks`,
//! never re-typed here, so the two cannot drift).
//!
//! One deliberate deviation from the rows above, stated
//! plainly: each figure row opens with a fixed-width
//! name gutter ([`DOPE_HEADER_W`]) so a row can say
//! WHICH figure it belongs to. The cells after the
//! gutter keep the chips' exact pitch and the shared
//! scroll offset, but the lane's frame columns sit a
//! constant gutter-plus-gap to the right of the chips'
//! columns — a dopesheet that cannot label its rows is
//! useless, and a gutter that scrolled separately would
//! need a second viewport (and a second offset to keep
//! in sync). The offset is constant, so a keyframe still
//! sits under its frame's chip up to that one fixed
//! shift, and the current-frame tint (below) keeps the
//! playhead column readable at a glance.
//!
//! ## What a keyframe is here
//!
//! The data is [`figure_keyframes`]'s, unmodified: a
//! frame is a key for a figure when the figure HAS a
//! pose on that frame and either it is frame 0, or the
//! pose differs from the previous frame's pose (root
//! moved / an angle moved, past the engine's epsilons),
//! or the figure was missing from the previous frame
//! (appearing is a change). That is why the no-keys
//! empty state can honestly say "pose a figure on a
//! frame to create one": posing writes the frame's pose,
//! and a first or changed pose is a key by this rule.
//!
//! ## Rows, cap, presses
//!
//! One row per figure, capped at [`DOPE_MAX_ROWS`]: with
//! more figures than the cap, the selected figure's row
//! is pinned first (it is the row the user is posing)
//! and the remaining slots fill in project order; the
//! figures left out are counted by an honest
//! "+N more figures" line under the rows — text, not a
//! dead scroll or a fake expander. The header cell
//! ([`row_key`]) presses to `EditorCore::select_figure`
//! — the figures panel's own select op. A frame cell
//! ([`cell_key`]) presses to select that figure AND
//! `EditorCore::goto` that frame — the figures panel's
//! select plus the marker row's seek, both real core
//! writes, neither snapshotted (selection and playhead
//! are runtime state, as everywhere else in the shell).
//!
//! The current frame's column is tinted in every row
//! (the kit's selected-fill token at cell scale), and
//! the selected figure's whole row carries the same
//! tint behind its cells — the two read apart by shape:
//! a vertical band for the playhead, a horizontal band
//! for the selection. A keyframe shows the timeline's
//! own diamond glyph (`◆`, the marker row's camera
//! marker) in the accent, so the sheet reads in the
//! timeline's existing visual language; the figure's
//! identity colour is carried by the header dot (the
//! figures panel's dot, same construction) instead of
//! the diamond, because a figure colour can be white
//! or near-white and would vanish on the card in one
//! theme or the other — the accent cannot.
//!
//! ## Labels
//!
//! The "Dopesheet" header label routes through
//! `i18n_wire::tr` and the i18n table's `dopesheet`
//! key, genuinely translated in all 28 languages
//! (i18n.rs; the key is registered in SHELL_KEYS, so
//! the coverage guard protects it). The figure name
//! is fitted to
//! [`DOPE_NAME_BUDGET`] by [`fit_dope_name`] (the FX
//! browser's measured LABEL width model does the
//! measuring) and Rune's `.truncate()` is chained as
//! well, the figures panel's belt-and-braces for an
//! unbounded user string in a fixed-sum row.
//!
//! ## Sprite rows (Wave X, X5 — SpriteLoop GAP P-3)
//!
//! Under the figure rows, a "Sprites" section gives the
//! free sprite instances (`Project.sprites`) the same
//! display + navigation: one row per instance (asset
//! name in the header cell, a diamond on each frame
//! holding a stored transform key — [`sprite_key_frames`]),
//! the same [`DOPE_MAX_ROWS`] cap with the Sprites
//! panel's selected instance pinned first and a
//! "+N more sprites" line. A header press selects the
//! instance in the Sprites panel (panel state — sprite
//! selection has no core field, exactly as the stage's
//! sprite prims take it); a cell press selects it AND
//! goes to the frame. The section renders whenever
//! instances exist, on its own condition: the two
//! empty states above now apply only when neither the
//! figure sheet nor the sprite section has anything to
//! show, so a sprites-only project (or a figures project
//! with no keys yet) gets its rows instead of a message
//! about figures.
//!
//! ## Keys (all additive, Wave 10)
//!
//! `dopesheet-lane` on the lane root, [`row_key`]
//! (`dope-row-{fi}`) on each figure's header cell,
//! [`cell_key`] (`dope-cell-{fi}-{frame}`, 0-based
//! frame — the marker keys' numbering, not the chips'
//! 1-based labels) on each frame cell. Wave X (X5)
//! adds, in the same scheme: `dope-sprite-row-{i}` on
//! each sprite's header cell and
//! `dope-sprite-cell-{i}-{frame}` on its frame cells.
//!
//! ## State-key marks (Wave Y, Y11 — SpriteLoop GAP P-1 deepening)
//!
//! A figure's animation state keys (Wave X, P-1:
//! `FigureInstance::states` — the frames where the
//! figure switches which clip it plays, with what
//! offset/speed) are frame events of exactly the kind
//! this lane shows, but the pose-keyframe derivation
//! cannot see them: a clip-driven figure whose timeline
//! poses never change showed one ◆ at frame 0 and no
//! trace of the frames where its animation actually
//! changes. Each figure row now also carries its
//! state-key frames ([`state_key_frames`]): a state key
//! alone on a frame shows `◇` — the pose diamond's
//! hollow twin — in the theme's secondary accent (the
//! brief's second accent, #9B7CFF, via
//! `app_theme::secondary_accent`), so the two event
//! kinds read apart by shape AND colour; a frame
//! holding both a pose key and a state key keeps the
//! accent `◆` and gains a 1 pt secondary-accent cell
//! border, so the state channel survives the
//! coincidence without a second glyph overflowing the
//! 30 pt chip. Cell presses are unchanged (select +
//! goto — the mark rides the existing cells, no new
//! element keys). The lane's gating widens with the
//! marks: rows render when pose keys OR state keys
//! exist ([`any_state_keys`]), and the "No keyframes
//! yet" empty state now requires both silent — a
//! state-keyed but unposed figure gets its rows, not a
//! message denying its keys exist.
//!
//! ## Channel-key marks (Wave AA, AA3 — §6.20 (2))
//!
//! A figure's graph channel keys (`Project::channels`)
//! are the third event kind this lane shows. The pose
//! derivation cannot see them at all: keying a channel
//! writes no pose, so a figure animated purely from the
//! Graph panel — the normal way to animate a bend, a
//! length or a root drift — showed whatever its stored
//! poses happened to do and no trace of the frames its
//! channels actually turn on. Each figure row now also
//! carries its channel-key frames
//! ([`channel_key_frames`]): the frames holding a key
//! in ANY channel targeting the figure, every
//! `ChannelTarget` at parity — root (X and Y), angle,
//! length and bend mark identically, exactly as the
//! timeline's marker row already marks every channel
//! with one glyph. Bend keys, the newest target, are
//! visible here precisely where angle keys are.
//!
//! The mark is the marker row's own: `●` in the
//! secondary text tone (`marker_glyph_color`'s graph
//! colour), so a channel key reads as the same event in
//! both rows of the timeline card. Graph keys live at
//! fractional times, so "on a frame" is the marker
//! row's mapping — [`graph_key_frame`]'s rounding
//! (nearest frame, halves away from zero), imported
//! from `timeline_marks`, never re-typed here, so the
//! two rows cannot disagree about which frame a key
//! sits on; a key rounding outside the timeline marks
//! nothing, rather than piling onto an end frame. (The
//! sprite rows above skip fractional keys instead:
//! sprite keys are frame-indexed storage, graph keys
//! are timed curves — different data, different honest
//! rule.) Channels pointing at a figure that no longer
//! exists mark no row — the graph module's standing
//! rule for stale channels.
//!
//! Coincidences compose without losing an event. The
//! glyph slot keeps the pose/state precedence (pose `◆`
//! in the accent, else state `◇` in the secondary
//! accent, the state border on a pose+state frame
//! unchanged); a channel key on the same frame adds
//! its `●` beside the glyph — the marker row already
//! ships two glyphs in one 30 pt cell at this gap, and
//! the glyph row here never holds more than two — and
//! on a pose+state+channel frame the state keeps the
//! border while the channel takes the second glyph, so
//! all three kinds stay visible. A channel key alone
//! shows the lone `●`. On the current frame's tinted
//! cell the dot keeps the secondary text tone, as the
//! diamond keeps the accent there — this lane's own
//! current-frame convention, not the marker row's
//! white-on-accent one. (RM25 / PL-1: on the tint —
//! the current cell, the selected row, or both
//! stacked — the diamond and the state glyph take
//! `app_theme`'s on-tint accent variants, which hold
//! the 3:1 non-text guidance where the plain accents
//! dip under it; see the cell builder and
//! `app_theme`'s Polish re-audit.) Cell presses are unchanged
//! (select + goto — the mark rides the existing cells,
//! no new element keys). The lane's gating widens with
//! the marks: rows render when pose keys OR state keys
//! OR channel keys exist ([`any_channel_keys`]), and
//! the "No keyframes yet" empty state requires all
//! three silent — a channel-keyed but unposed figure
//! gets its rows, not a message denying its keys
//! exist.
//!
//! ## Frame-bend marks (Wave AB, AB3 — §6.22)
//!
//! A figure's stored frame bends (`Frame::bends` —
//! the per-frame, per-node bend table the posing
//! surfaces write: `cycle_bend`, the bend steppers,
//! `set_bend`, Pivot import, and the graph bake) are
//! the fourth event kind this lane shows. The other
//! three derivations are each blind to it: the pose
//! rule reads poses, the state derivation reads the
//! states vec, and the channel derivation reads the
//! graph curves — a bend authored by hand writes none
//! of those, so a figure bent entirely from the pose
//! tools showed its pose keys and no trace of the
//! frames its segments actually arc on. Each figure
//! row now also carries its bend-change frames
//! ([`bend_key_frames`]): [`figure_keyframes`]' own
//! change rule transposed to the bend table — a frame
//! marks when the figure's stored bend row,
//! normalised exactly as the model's `bends_at` reads
//! it, is non-zero there and differs from the
//! previous frame's row (past [`BEND_EPS`]); a bend
//! disappearing marks nothing, the pose rule's
//! appearance asymmetry. A baked bend channel
//! therefore marks where its stored rows truly
//! change — every frame of a varying bake, once for
//! a flat one — exactly as a baked pose channel
//! already does.
//!
//! The mark is NOT a fourth glyph, by measurement:
//! the shell's LABEL width model charges a full em
//! (12 pt) for each of ◆/◇/●, so two glyphs and their
//! gap already take 26 of the cell's 30 pt — a third
//! glyph (40 pt with gaps) would spill into the
//! neighbouring cells in precisely the coincidence
//! cases (pose + channel + bend) the composition
//! exists to keep lossless. The border is the state
//! channel's, the diamond's accent is the pose's, and
//! the dot's secondary tone is the graph channel's;
//! none can be borrowed without making one signal
//! mean two events. The bend mark is instead a thin
//! bar at the cell's bottom — [`BEND_STRIP_W`] ×
//! [`BEND_STRIP_H`], centred so its ends clear the
//! cell's rounded corners, in the primary text tone
//! no cell mark uses: a position channel, not a
//! glyph. It coexists with the border, both fills
//! and either glyph pairing in every combination,
//! and a lone bend shows the bar alone; in a bend
//! cell the glyphs centre in the height above the
//! bar, and every other cell's tree is unchanged.
//! Cell presses are unchanged (select + goto — the
//! mark rides the existing cells, no new element
//! keys). The lane's gating widens with the marks:
//! rows render when any of the four kinds exists
//! ([`any_bend_keys`]), and the "No keyframes yet"
//! empty state requires all four silent — its copy
//! stands, as it did for Y11 and AA3: posing remains
//! a true way to create a key, and the bend tools
//! are posing tools on the same frames.
//!
//! ## Key selection (Roadmap Wave 1, RM5 — F-4)
//!
//! The lane is a consumer of the shell's ONE shared key
//! selection (`key_selection::KeySel`, `State::key_sel`
//! — IA-RM5-1), on its read/selection side only:
//! dopesheet EDITING is RM14's seat. A cell whose frame
//! carries a channel key resolves that event through
//! [`channel_key_event`] — the FIRST channel (project
//! order) of the figure holding a key that rounds to
//! the cell's frame, the per-cell twin of
//! [`channel_key_frames`]' aggregate. Pressing such a
//! cell keeps its historical navigation (select figure
//! + goto) and additionally `select_one`s that event in
//! the shared model, so the graph panel's handles, the
//! delete-selected-keys action and this lane all agree
//! on the selected key. A cell holding one renders with
//! an accent border while its event is the selection —
//! applied after the glyph composition, so the
//! selection border supersedes the pose+state
//! coincidence border (a documented trade: both cannot
//! draw, and the selection is the live state). Cells
//! with no channel key never touch the selection; pose,
//! state and bend events are not in the model (F-4's
//! scope is channel keys; per-kind events await A5
//! R4's one-key-definition work).
//!
//! ## Key editing (Roadmap Wave 1, RM14 — P-2)
//!
//! The lane is an EDITOR now: the read/selection side
//! above gains the verbs. Every figure cell carries a
//! drag instead of a press (a draggable element folds
//! its click into the drag's Down — the subtitle
//! lane's settlement, the chips row's before it): Down
//! performs the historical press work verbatim (select
//! figure + goto + the F-4 `select_one`) and plants a
//! gesture record when the cell carries a key event;
//! Move steps the grabbed key toward the pointer's
//! whole-frame travel ([`frame_delta`], the subtitle
//! lane's own — the timeline has one pitch, imported
//! never re-typed; IA-RM13-4's chip_w threading must
//! thread this call when it lands); Up settles the
//! status. A cancelled gesture rolls its one staged
//! snapshot back. The gesture's snapshot is LAZY —
//! staged by the first Move that actually moves a key
//! — so a plain click spends no undo step, and the
//! whole drag is exactly one undo step however far it
//! travels.
//!
//! Which key a cell drags resolves by ONE priority,
//! [`key_event_at`]: the channel event the F-4 press
//! selects, else a pose key, else a state key, else a
//! bend key — glyph precedence made operational. A
//! channel drag on a key that is part of the live
//! selection moves the WHOLE selection (the pro
//! idiom); on any other channel key it selects that
//! key first, exactly as the graph drag does. Per-kind
//! move semantics, each planned against its own store
//! with the graph nudge's collision honesty (a move
//! onto or across another key of the same kind is
//! refused, never destructive):
//!
//! * Channel keys: whole-frame shifts of the selected
//!   set under `key_selection`'s shared check
//!   (`selected_move_error` / `move_selected_keys_by`).
//! * Pose keys: the dense model's fill rule — moving
//!   later fills the vacated interval with the pose
//!   the previous frame holds (the `delete_keyframe`
//!   hold, run forwards) and lands the key's pose on
//!   the target; moving earlier fills the interval
//!   with the key's own pose. The frame-0 base key and
//!   an appearance key (no earlier pose to hold) do
//!   not move — the delete refusals, transposed.
//! * State keys: the stored key's frame changes; the
//!   vec stays sorted; another state key at the
//!   target refuses (the model's later-wins upsert
//!   would silently eat it).
//! * Bend keys: the bend-table twin of the pose fill,
//!   over normalised rows ([`bend_row`]); a step can
//!   merge a following mark into the moved key's span
//!   when the following row already equals the key's
//!   row — the value there is unchanged, it simply
//!   stops CHANGING there, which is the dense model's
//!   honest arithmetic (poses behave identically).
//!
//! Above the rows, an actions row (`dopesheet-actions`)
//! mounts while there is something to act on — a live
//! selection, a key event under the playhead on the
//! selected figure, or a pasteable clipboard — and
//! carries the same verbs as buttons: Move key −1/+1
//! (the selection when live, else the playhead event),
//! Copy key, Delete key, and Paste keys while the
//! clipboard's kind has a channel on the selected
//! figure. No dead controls: a button whose context is
//! absent is not mounted. Copy is store-shaped, and
//! says so: channel keys (the selection, or the single
//! playhead channel key) go to the shell's graph
//! clipboard (`KeyClipboard`, the AC1 tag and all), so
//! the Graph workspace's paste accepts them and the
//! lane's own Paste keys writes them onto the selected
//! figure's channel of the clipboard's kind — the exact
//! target when it exists, else the figure's first
//! channel of that kind (AC1's kind looseness, the
//! lane's first-match discipline), rebased to the
//! playhead by `graph_keys::paste_keys`. The dense
//! stores have no clipboard to hold them, so a pose,
//! state or bend Copy STAMPS the key
//! [`STAMP_OFFSET_FRAMES`] frames later — the keyframe
//! tools' `copy_key_to_frame` precedent — under the
//! same collision refusals. Delete dispatches by the
//! same priority as the drag: the live selection
//! through `delete_selected_keys`, else the playhead
//! event through its per-kind path (channel:
//! `graph_keys::remove_key_at`; pose: the engine's
//! `delete_keyframe`; state: the stored key goes;
//! bend: the frame holds the previous row, the
//! `delete_keyframe` bend hold). Status sentences are
//! this lane's own, in the shell's established shapes.
//! All of it is literal-English like the graph
//! workspace (the future keying pass owns these
//! strings too — IA-RM14-2).
//!
//! Sprite cells keep their press: sprite keys are not
//! in this seat's verb set (their store is the Sprites
//! panel's; nothing here pretends otherwise).
//!
//! ## Exposure spans (Roadmap Wave 1, RM14 — P-2)
//!
//! Holds are timing, and the sheet now draws them: a
//! pose key's EXPOSURE runs from its frame to the next
//! pose key's frame (or the timeline's end) —
//! [`exposure_spans`] — and a span of two or more
//! frames (a HOLD: one pose exposed across several
//! frames, the x-sheet's basic unit) draws a slim bar
//! across the TOP of every cell it covers
//! ([`EXPO_STRIP_H`]): the accent at half strength on
//! the key's own cell, the secondary text tone, fainter,
//! on the held cells after it, so a held pose reads as
//! one continuous exposure with its key at its head.
//! A single-frame span draws nothing — the ◆ already
//! says everything a one-frame exposure has to say.
//! The bar is a position channel like the bend bar
//! (which keeps the cell's bottom): the two compose in
//! [`strip_cell`] with the glyphs centred between
//! them, and a cell with neither keeps its historical
//! tree exactly. The editable half of A5 R3 — an edge
//! grip that extends a hold by inserting frames — is
//! NOT built here: inserting frames is project-wide
//! frame surgery (every figure shifts), its natural
//! home is the frame-ops surface, and it is recorded
//! as IA-RM14-3 with the design questions priced.

use std::cell::RefCell;

use rune::tok::{type_, Theme};
use rune::{col, el, row, text, Color, Drag, El, Phase, Tx};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::graph::Key;
use stykanimate_rs::model::{FigureInstance, Project, SpriteInstance};
use stykanimate_rs::stats::figure_keyframes;

use crate::chrome_view::btn;
use crate::rune_sec::design;
use crate::rune_sec::fx_browser::preset_label_width;
use crate::rune_sec::graph_panel::channel_label;
use crate::rune_sec::key_selection::{
    copy_selected_keys, delete_selected_keys, move_selected_keys, move_selected_keys_by,
    selected_move_error, KeySel, KEY_MATCH_TOL,
};
use crate::rune_sec::keyframe_tools::{same_target_kind, KeyClipboard};
use crate::rune_sec::subtitle_lane::frame_delta;
use crate::rune_sec::timeline_marks::{graph_key_frame, CHIP_GAP, CHIP_W};
use crate::stage_view::hex_color;
use crate::State;

/// Height of the lane's header row ("Dopesheet" +
/// the frame readout): a slim strip in the ruler's
/// spirit ([`crate::rune_sec::timeline_marks::RULER_H`]
/// is 12), just tall enough for one LABEL line plus
/// air.
pub const DOPE_HEADER_H: f32 = 18.0;

/// Height of one figure row — header cell and frame
/// cells alike (Roadmap Wave 2, W2-55 — A20-14): the
/// WCAG 2.5.8 target-size floor, 24 pt. The row was
/// 20 — under the floor for a press/drag target the
/// user hits on every pose (S18/A20 §1.8). The glyphs
/// are unchanged (12 pt diamonds, centred), so the
/// pressable grows while the visual stays as drawn;
/// the strip cells' glyph area grows with the row
/// (see [`strip_cell`]). Sprite rows take this height
/// too — same press discipline, same constant.
pub const DOPE_ROW_H: f32 = 24.0;

/// Height of the frame-bend mark's bar (Wave AB,
/// AB3): a 2 pt rule at the bottom of a bend cell —
/// thick enough to read at a glance against either
/// fill, thin enough to leave the glyphs their
/// height (they centre in the 22 pt above it —
/// [`DOPE_ROW_H`] minus this bar; W2-55 raised the
/// row from 20, when the figure was 18).
pub const BEND_STRIP_H: f32 = 2.0;

/// Width of the frame-bend mark's bar: the cell
/// width minus the cell's corner radius on each
/// side (30 − 2 × 6 = 18), so the bar's ends sit
/// inside the rounded corners instead of poking
/// past them.
pub const BEND_STRIP_W: f32 = 18.0;

/// Height of the exposure bar (Roadmap Wave 1,
/// RM14 — P-2): a 3 pt rule at the TOP of a cell in a
/// hold span — the bend bar's twin at the other edge,
/// thick enough to read as one continuous exposure
/// across the span's cells, thin enough to leave the
/// glyphs their height (they centre in what remains).
pub const EXPO_STRIP_H: f32 = 3.0;

/// Width of a figure row's header cell: 6 pt of
/// padding on each side, the figures panel's 10 pt
/// colour dot, a 6 pt gap, and the name fitted to
/// [`DOPE_NAME_BUDGET`] — 6 + 10 + 6 + 92 + 6 = 120.
pub const DOPE_HEADER_W: f32 = 120.0;

/// The name's share of the header cell, in points —
/// see [`DOPE_HEADER_W`]'s arithmetic. [`fit_dope_name`]
/// fits to exactly this, measured with the shell's
/// LABEL width model.
pub const DOPE_NAME_BUDGET: f32 = 92.0;

/// Most figure rows the lane shows at once. The lane
/// lives inside the timeline card, whose height is
/// stage space; four rows plus the header keep the
/// whole sheet slimmer than the panels it summarises.
/// Past the cap the selected figure is pinned first
/// (see [`dope_row_order`]) and the rest are counted,
/// not hidden silently.
pub const DOPE_MAX_ROWS: usize = 4;

/// The header cell's key for figure `fi`:
/// `dope-row-{fi}`.
pub fn row_key(fi: usize) -> String {
    format!("dope-row-{fi}")
}

/// The frame cell's key for figure `fi`, frame `frame`
/// (0-based, the marker keys' numbering):
/// `dope-cell-{fi}-{frame}`.
pub fn cell_key(fi: usize, frame: usize) -> String {
    format!("dope-cell-{fi}-{frame}")
}

/// The header's current-frame readout: `Frame {cur+1} /
/// {count}`, the status strip's 1-based wording. A
/// zero-frame project reads `Frame 0 / 0` rather than
/// claiming a frame 1 that does not exist, and a stale
/// current past the end clamps to the last frame.
pub fn frame_readout(current: usize, count: usize) -> String {
    if count == 0 {
        return "Frame 0 / 0".to_string();
    }
    format!("Frame {} / {}", current.min(count - 1) + 1, count)
}

/// Fit a figure name into [`DOPE_NAME_BUDGET`] points
/// of LABEL text: the name unchanged while the shell's
/// measured width model ([`preset_label_width`], the
/// FX browser's — an upper-bound model, so a name it
/// passes truly fits) says it fits; otherwise cut with
/// an ellipsis, at a word boundary when a space sits
/// in the kept prefix's second half (the figure
/// inspector's `fit_header_name` rule), else a hard
/// cut. The ellipsis is charged a full em (12 pt),
/// the FX browser's convention. Character-counted
/// throughout, so a multibyte name is never split
/// mid-character. Display only — selection always
/// works on the figure index, never the fitted string.
pub fn fit_dope_name(name: &str) -> String {
    if preset_label_width(name) <= DOPE_NAME_BUDGET {
        return name.to_string();
    }
    let room = DOPE_NAME_BUDGET - 12.0;
    let chars: Vec<char> = name.chars().collect();
    let mut used = 0.0f32;
    let mut kept = 0usize;
    for &c in &chars {
        let w = preset_label_width(&c.to_string());
        if used + w > room {
            break;
        }
        used += w;
        kept += 1;
    }
    let prefix: String = chars[..kept].iter().collect();
    if let Some(pos) = prefix.rfind(' ') {
        if pos >= prefix.len() / 2 {
            return format!("{}…", &prefix[..pos]);
        }
    }
    format!("{prefix}…")
}

/// The order figure rows are shown in: project order,
/// capped at [`DOPE_MAX_ROWS`] — except that past the
/// cap the selected figure's row is pinned first (the
/// row being posed is the one row that must never be
/// the one cut), and the remaining slots fill in
/// project order around it. A stale selection (past
/// the last figure, which the core tolerates between
/// edits) pins nothing and selects nothing.
pub fn dope_row_order(figure_count: usize, selected: usize) -> Vec<usize> {
    let mut order: Vec<usize> = Vec::new();
    if figure_count > DOPE_MAX_ROWS && selected < figure_count {
        order.push(selected);
    }
    for fi in 0..figure_count {
        if order.len() >= DOPE_MAX_ROWS {
            break;
        }
        if !order.contains(&fi) {
            order.push(fi);
        }
    }
    order
}

/// One shown figure row's data, computed once per
/// render by [`dope_rows`] so the builder is a pure
/// projection of it (the marker row's
/// `markers_for_project` discipline: never per-cell
/// searches).
pub struct DopeRow {
    /// The figure's project index.
    pub fi: usize,
    /// The full figure name (fitted only at paint time).
    pub name: String,
    /// The figure's own colour, `0xRRGGBB` (the header
    /// dot's fill source).
    pub color: u32,
    /// The figure is hidden (its name greys, the
    /// figures panel's treatment).
    pub hidden: bool,
    /// This is `core.selected_figure`.
    pub selected: bool,
    /// The figure's keyframe indices, ascending
    /// ([`figure_keyframes`]'s output, verbatim).
    pub keys: Vec<usize>,
    /// The figure's state-key frames, ascending and
    /// deduped ([`state_key_frames`]'s output) — the
    /// frames where an animation state key switches
    /// the clip this figure plays (Wave Y, Y11).
    pub state_keys: Vec<usize>,
    /// The figure's channel-key frames, ascending and
    /// deduped ([`channel_key_frames`]'s output) — the
    /// frames holding a graph channel key for this
    /// figure, any target (Wave AA, AA3).
    pub channel_keys: Vec<usize>,
    /// The figure's frame-bend key frames, ascending
    /// ([`bend_key_frames`]'s output) — the frames
    /// where the figure's stored bend row changes
    /// (Wave AB, AB3).
    pub bend_keys: Vec<usize>,
    /// The figure's exposure spans, ascending
    /// ([`exposure_spans`]' output) — each pose key's
    /// run to the next key or the timeline's end
    /// (Roadmap Wave 1, RM14 — P-2).
    pub spans: Vec<ExposureSpan>,
}

/// The shown rows for the current project, in display
/// order (see [`dope_row_order`]), each with its
/// keyframes resolved once.
pub fn dope_rows(core: &EditorCore) -> Vec<DopeRow> {
    let project = &core.project;
    dope_row_order(project.figures.len(), core.selected_figure)
        .into_iter()
        .map(|fi| {
            let fig = &project.figures[fi];
            DopeRow {
                fi,
                name: fig.name.clone(),
                color: fig.color,
                hidden: fig.hidden,
                selected: fi == core.selected_figure,
                keys: figure_keyframes(project, fi),
                state_keys: state_key_frames(fig),
                channel_keys: channel_key_frames(project, fi),
                bend_keys: bend_key_frames(project, fi),
                spans: exposure_spans(project, fi),
            }
        })
        .collect()
}

/// Whether ANY figure in the project has a keyframe —
/// over every figure, not just the shown rows: a key
/// on a capped-out figure still means the sheet has
/// something to say, so the rows (and the "+N more"
/// line) render instead of the no-keys empty state.
pub fn any_keyframes(core: &EditorCore) -> bool {
    (0..core.project.figures.len()).any(|fi| !figure_keyframes(&core.project, fi).is_empty())
}

/// Whether `frame` is one of a row's keys. `keys` is
/// [`figure_keyframes`]'s ascending output, so this is
/// a binary search.
pub fn is_key(keys: &[usize], frame: usize) -> bool {
    keys.binary_search(&frame).is_ok()
}

/// A figure's state-key frames (Wave Y, Y11), ascending
/// and deduped — the display derivation over
/// `FigureInstance::states`, the sprite rows'
/// [`sprite_key_frames`] precedent applied to figures.
/// Storage is canonical (the authoring ops keep the
/// vec sorted, upserting by frame), but a hand-edited
/// project can repeat a frame; the model's tie rule
/// makes the later key win there, and the mark is
/// per-frame, so dedupe is the honest display.
pub fn state_key_frames(fig: &FigureInstance) -> Vec<usize> {
    let mut frames: Vec<usize> = fig.states.iter().map(|k| k.frame).collect();
    frames.sort_unstable();
    frames.dedup();
    frames
}

/// Whether `frame` holds one of a row's state keys.
/// `state_keys` is [`state_key_frames`]'s ascending
/// output, so this is a binary search — [`is_key`]'s
/// twin for the state channel.
pub fn is_state_key(state_keys: &[usize], frame: usize) -> bool {
    state_keys.binary_search(&frame).is_ok()
}

/// Whether ANY figure in the project carries a state
/// key — [`any_keyframes`]'s sibling for the state
/// channel, so the lane's gating can hear either kind
/// of key (Wave Y, Y11).
pub fn any_state_keys(core: &EditorCore) -> bool {
    core.project
        .figures
        .iter()
        .any(|fig| !fig.states.is_empty())
}

/// A figure's channel-key frames (Wave AA, AA3),
/// ascending and deduped: the frames holding at least
/// one key in ANY graph channel targeting figure
/// `fi` — every `ChannelTarget` at parity (root X/Y,
/// angle, length, bend), the marker row's
/// target-blind rule per figure. Graph keys live at
/// fractional times, so each key lands by
/// [`graph_key_frame`]'s rounding — imported from
/// `timeline_marks`, the same mapping the marker row
/// and the graph panel's seek use — and a key rounding
/// outside the timeline marks nothing. Channels whose
/// `figure` names no row here (a stale channel, the
/// graph module's standing ignore rule) contribute
/// nothing, and several channels keying the same frame
/// still mark it once: the mark is per-frame, as with
/// [`state_key_frames`].
pub fn channel_key_frames(project: &Project, fi: usize) -> Vec<usize> {
    let count = project.frames.len();
    let mut frames: Vec<usize> = project
        .channels
        .iter()
        .filter(|ch| ch.figure == fi)
        .flat_map(|ch| ch.keys.iter())
        .filter_map(|k| graph_key_frame(k.frame, count))
        .collect();
    frames.sort_unstable();
    frames.dedup();
    frames
}

/// The channel-key EVENT a single cell names
/// (Roadmap Wave 1, RM5 — F-4): the first channel, in
/// project order, belonging to figure `fi` that holds a
/// key rounding to `frame` ([`graph_key_frame`] over a
/// timeline of `count`), returned as `(channel index,
/// key frame)` — the shared key selection's currency.
/// [`channel_key_frames`]' per-cell twin: that function
/// aggregates every such frame for the row's marks;
/// this one names WHICH channel a press on the cell
/// selects. A stale channel (figure names no row here)
/// is only reachable by asking for its own `fi`, which
/// no row does; keys rounding outside the timeline name
/// nothing, exactly as they mark nothing.
pub fn channel_key_event(
    project: &Project,
    fi: usize,
    frame: usize,
    count: usize,
) -> Option<(usize, f32)> {
    for (ci, ch) in project.channels.iter().enumerate() {
        if ch.figure != fi {
            continue;
        }
        for k in &ch.keys {
            if graph_key_frame(k.frame, count) == Some(frame) {
                return Some((ci, k.frame));
            }
        }
    }
    None
}

/// Whether `frame` holds one of a row's channel keys.
/// `channel_keys` is [`channel_key_frames`]'s ascending
/// output, so this is a binary search — [`is_key`]'s
/// twin for the graph channel.
pub fn is_channel_key(channel_keys: &[usize], frame: usize) -> bool {
    channel_keys.binary_search(&frame).is_ok()
}

/// Whether ANY figure in the project carries a
/// channel key that lands on the timeline —
/// [`any_keyframes`]'s sibling for the graph channel,
/// so the lane's gating can hear all three kinds of
/// key (Wave AA, AA3). Keys on stale channels and keys
/// rounding outside the timeline show no mark, so
/// they do not speak for the lane either: this is
/// computed through [`channel_key_frames`] itself,
/// never a raw channel count.
pub fn any_channel_keys(core: &EditorCore) -> bool {
    (0..core.project.figures.len())
        .any(|fi| !channel_key_frames(&core.project, fi).is_empty())
}

/// The bend-change epsilon, in radians (Wave AB,
/// AB3): a stored bend row "changes" only when a
/// component moves by more than this. It is the pose
/// rule's epsilon discipline transposed — far below
/// every authored granularity (`cycle_bend`'s steps
/// are 0.35 rad apart, the smallest authored bend
/// movement there is) and above the float dust of
/// resized and re-evaluated rows.
pub const BEND_EPS: f32 = 0.01;

/// Figure `fi`'s stored bend row at `frame`,
/// normalised exactly as the model's `bends_at` reads
/// it: a missing figure entry (or a short row) pads
/// with zeros to the figure's node count and a long
/// row truncates. One addition of this lane's own: a
/// non-finite component reads as 0.0 — the shell's
/// sanitised-read rule for corrupt stores, so a
/// hand-edited NaN shows no mark rather than a
/// phantom one. An out-of-range `fi` or frame yields
/// the all-zero row.
fn bend_row(project: &Project, fi: usize, frame: usize) -> Vec<f32> {
    let n = project
        .figures
        .get(fi)
        .map(|f| f.def.nodes.len())
        .unwrap_or(0);
    let mut row: Vec<f32> = project
        .frames
        .get(frame)
        .and_then(|f| f.bends.get(fi))
        .map(|r| {
            r.iter()
                .map(|b| if b.is_finite() { *b } else { 0.0 })
                .collect()
        })
        .unwrap_or_default();
    row.resize(n, 0.0);
    row
}

/// A figure's frame-bend key frames (Wave AB, AB3),
/// ascending: the frames where the figure's STORED
/// bend row (`Frame::bends`) changes — the fourth
/// event kind this lane shows, and the frame table's
/// own data, invisible to the other three derivations
/// (poses read poses, state keys read the states vec,
/// channel keys read the graph curves; a bend keyed
/// from the posing surfaces writes none of those).
/// The rule is [`figure_keyframes`]' change rule
/// transposed: a frame marks when its normalised row
/// ([`bend_row`]) is non-zero past [`BEND_EPS`] AND
/// differs from the previous frame's row past the
/// same epsilon — frame 0 against the implicit
/// all-zero row, so a bend authored on the first
/// frame marks. A bend DISAPPEARING (the row
/// returning to zero) does not mark, the pose rule's
/// appearance/disappearance asymmetry exactly. A
/// baked bend channel therefore marks where its
/// stored rows actually change — every frame of a
/// varying bake, once for a flat one — as baked poses
/// already do. The comparison runs over the whole
/// normalised row, the row `bends_at` returns; the
/// writers refuse origin bends, so index 0 is zero in
/// authored data. An out-of-range `fi` yields an
/// empty vec.
pub fn bend_key_frames(project: &Project, fi: usize) -> Vec<usize> {
    if fi >= project.figures.len() {
        return Vec::new();
    }
    let n = project.figures[fi].def.nodes.len();
    let mut prev = vec![0.0; n];
    let mut keys = Vec::new();
    for f in 0..project.frames.len() {
        let cur = bend_row(project, fi, f);
        let nonzero = cur.iter().any(|b| b.abs() > BEND_EPS);
        let changed = cur
            .iter()
            .zip(prev.iter())
            .any(|(a, b)| (a - b).abs() > BEND_EPS);
        if nonzero && changed {
            keys.push(f);
        }
        prev = cur;
    }
    keys
}

/// Whether `frame` holds one of a row's bend keys.
/// `bend_keys` is [`bend_key_frames`]' ascending
/// output, so this is a binary search — [`is_key`]'s
/// twin for the bend table.
pub fn is_bend_key(bend_keys: &[usize], frame: usize) -> bool {
    bend_keys.binary_search(&frame).is_ok()
}

/// Whether ANY figure in the project carries a
/// stored bend change — [`any_keyframes`]' sibling
/// for the bend table, so the lane's gating can hear
/// all four kinds of key (Wave AB, AB3). Computed
/// through [`bend_key_frames`] itself, as
/// [`any_channel_keys`] is: silent tables do not
/// speak for the lane.
pub fn any_bend_keys(core: &EditorCore) -> bool {
    (0..core.project.figures.len())
        .any(|fi| !bend_key_frames(&core.project, fi).is_empty())
}

/// One pose key's exposure (Roadmap Wave 1, RM14 —
/// P-2): the half-open run `[start, end)` over which
/// the key's pose is what the figure shows — from the
/// key's own frame to the next pose key's frame, or
/// to the timeline's end for the last key. The
/// x-sheet's basic unit: a span of one frame is an
/// ordinary exposure; a span of two or more is a
/// HOLD (one pose exposed across several frames),
/// which is what the cell bars draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExposureSpan {
    /// The pose key's frame (the span's first frame).
    pub start: usize,
    /// The next pose key's frame, or the frame count.
    pub end: usize,
}

impl ExposureSpan {
    /// Whether this span is a hold — a pose exposed
    /// across two or more frames.
    pub fn is_hold(&self) -> bool {
        self.end - self.start >= 2
    }

    /// Whether `frame` sits inside the span.
    pub fn covers(&self, frame: usize) -> bool {
        frame >= self.start && frame < self.end
    }
}

/// A figure's exposure spans, ascending — one per
/// pose key ([`figure_keyframes`]), each ending where
/// the next begins. A figure with no pose keys has no
/// spans. Spans are a POSE-store notion: channel keys
/// interpolate continuously and state keys switch
/// clips, so neither "holds" in the x-sheet sense;
/// the bend table's runs are visible through its own
/// marks.
pub fn exposure_spans(project: &Project, fi: usize) -> Vec<ExposureSpan> {
    let keys = figure_keyframes(project, fi);
    let count = project.frames.len();
    keys.iter()
        .enumerate()
        .map(|(i, &start)| ExposureSpan {
            start,
            end: keys.get(i + 1).copied().unwrap_or(count),
        })
        .collect()
}

// ---------------------------------------------------------------------
// Key editing (Roadmap Wave 1, RM14 — P-2): the verbs.
// Channel keys ride the shared selection model
// (key_selection); the dense stores (pose, state,
// bend) get per-kind paths here, each with the graph
// nudge's collision honesty and exactly one snapshot
// per landed op. See the module docs for the full
// semantics.
// ---------------------------------------------------------------------

/// A key event one dopesheet cell (or the playhead on
/// a figure's row) names, in the lane's ONE priority
/// order: the channel event the F-4 press selects,
/// else a pose key, else a state key, else a bend
/// key — the cell's glyph precedence (pose ◆ over
/// state ◇, the channel ● beside them, the bend bar
/// under all) made operational for editing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DopeKeyRef {
    /// A graph channel key: the channel index and the
    /// key's own (possibly fractional) frame.
    Channel { channel: usize, key_frame: f32 },
    /// A pose keyframe (dense store).
    Pose,
    /// An animation state key (sparse store).
    State,
    /// A stored bend change (dense table).
    Bend,
}

/// The key event at (`fi`, `frame`) under the lane's
/// priority ([`DopeKeyRef`]). `count` is the timeline
/// length the channel rounding reads against
/// ([`channel_key_event`]'s contract). `None` when the
/// cell carries no key of any kind.
pub fn key_event_at(
    project: &Project,
    fi: usize,
    frame: usize,
    count: usize,
) -> Option<DopeKeyRef> {
    if let Some((ci, kf)) = channel_key_event(project, fi, frame, count) {
        return Some(DopeKeyRef::Channel {
            channel: ci,
            key_frame: kf,
        });
    }
    if is_key(&figure_keyframes(project, fi), frame) {
        return Some(DopeKeyRef::Pose);
    }
    let fig = project.figures.get(fi)?;
    if is_state_key(&state_key_frames(fig), frame) {
        return Some(DopeKeyRef::State);
    }
    if is_bend_key(&bend_key_frames(project, fi), frame) {
        return Some(DopeKeyRef::Bend);
    }
    None
}

/// Whether the shared selection names at least one
/// LIVE key — the actions row's first context test
/// (the model's reads are defensive, so this is the
/// honest "is there a selection to act on").
pub fn selection_is_live(core: &EditorCore, sel: &KeySel) -> bool {
    let Some(ci) = sel.channel else {
        return false;
    };
    core.project
        .channels
        .get(ci)
        .is_some_and(|ch| !sel.indices(ci, ch).is_empty())
}

/// Write figure `fi`'s bend row at `frame` (the
/// normalised shape [`bend_row`] reads): the frame's
/// bends vec grows to hold the entry, and the entry
/// takes `row` verbatim. An out-of-range frame is a
/// no-op — the writers below plan inside the timeline.
fn set_bend_row(project: &mut Project, fi: usize, frame: usize, row: &[f32]) {
    let Some(fr) = project.frames.get_mut(frame) else {
        return;
    };
    if fr.bends.len() <= fi {
        fr.bends.resize(fi + 1, Vec::new());
    }
    fr.bends[fi] = row.to_vec();
}

/// The bend row BEFORE `frame` for a move/delete at
/// `frame`: frame 0's predecessor is the implicit
/// all-zero row (the derivation's own rule).
fn prev_bend_row(project: &Project, fi: usize, frame: usize) -> Vec<f32> {
    if frame == 0 {
        let n = project
            .figures
            .get(fi)
            .map(|f| f.def.nodes.len())
            .unwrap_or(0);
        vec![0.0; n]
    } else {
        bend_row(project, fi, frame - 1)
    }
}

/// One raw step of a pose key: shift the keyframe at
/// `at` one frame in direction `dir` (−1 or +1) with
/// NO snapshot and NO status — the gesture/stepping
/// layer owns both. The step is the dense fill rule
/// at unit distance: later, the key's frame takes the
/// previous frame's pose (and bend row) and the next
/// frame takes the key's; earlier, the previous frame
/// takes the key's pose (and bend row) and the key's
/// frame simply becomes a hold of it. `false` —
/// nothing written — when the target is out of range,
/// is itself a keyframe, or a pose the step needs is
/// missing (appearance/absence). Callers pre-validate
/// the whole move ([`move_pose_key`]) or step until
/// this refuses (the drag).
fn shift_pose_key_once(core: &mut EditorCore, fi: usize, at: usize, dir: i32) -> bool {
    let count = core.project.frames.len();
    let target = at as i32 + dir;
    if target < 0 || target as usize >= count {
        return false;
    }
    let target = target as usize;
    if is_key(&figure_keyframes(&core.project, fi), target) {
        return false;
    }
    let frames = &mut core.project.frames;
    if dir > 0 {
        let (Some(key_pose), Some(prev_pose), true) = (
            frames[at].poses.get(fi).cloned(),
            frames[at - 1].poses.get(fi).cloned(),
            frames[target].poses.get(fi).is_some(),
        ) else {
            return false;
        };
        let key_row = bend_row(&core.project, fi, at);
        let prev_row = bend_row(&core.project, fi, at - 1);
        frames[at].poses[fi] = prev_pose;
        frames[target].poses[fi] = key_pose;
        set_bend_row(&mut core.project, fi, at, &prev_row);
        set_bend_row(&mut core.project, fi, target, &key_row);
    } else {
        let (Some(key_pose), true) = (
            frames[at].poses.get(fi).cloned(),
            frames[target].poses.get(fi).is_some(),
        ) else {
            return false;
        };
        let key_row = bend_row(&core.project, fi, at);
        frames[target].poses[fi] = key_pose;
        set_bend_row(&mut core.project, fi, target, &key_row);
    }
    true
}

/// Whether one raw step of the key at `at` in
/// direction `dir` would land — the pure pre-check
/// the drag runs BEFORE staging its snapshot, so a
/// gesture that never moves spends no undo step (the
/// shift fns re-run the same conditions as they
/// write; between the check and the write nothing
/// else can mutate the store inside one phase).
fn step_valid(core: &EditorCore, kind: DopeKeyRef, fi: usize, at: usize, dir: i32) -> bool {
    let count = core.project.frames.len();
    let target = at as i32 + dir;
    if target < 0 || target as usize >= count {
        return false;
    }
    let target = target as usize;
    match kind {
        DopeKeyRef::Pose => {
            !is_key(&figure_keyframes(&core.project, fi), target)
                && core.project.frames[at].poses.get(fi).is_some()
                && core.project.frames[target].poses.get(fi).is_some()
                && (dir < 0 || (at > 0 && core.project.frames[at - 1].poses.get(fi).is_some()))
        }
        DopeKeyRef::State => {
            let Some(fig) = core.project.figures.get(fi) else {
                return false;
            };
            fig.states.iter().any(|k| k.frame == at)
                && !fig.states.iter().any(|k| k.frame == target)
        }
        DopeKeyRef::Bend => !is_bend_key(&bend_key_frames(&core.project, fi), target),
        DopeKeyRef::Channel { .. } => false,
    }
}

/// Move figure `fi`'s pose keyframe from `from` to
/// `to` as ONE undo step (Roadmap Wave 1, RM14 —
/// P-2; A4 R5's dopesheet retiming). Refusals —
/// status-only, no snapshot: no keyframe at `from`;
/// `from` is frame 0 (the base pose, `delete_keyframe`'s
/// refusal transposed); the figure APPEARS at `from`
/// (no earlier pose to hold); `to` is out of range or
/// holds (or crosses) another keyframe of the figure;
/// the figure is missing from any frame in the span.
pub fn move_pose_key(core: &mut EditorCore, fi: usize, from: usize, to: usize) -> bool {
    let keys = figure_keyframes(&core.project, fi);
    if !is_key(&keys, from) {
        core.status = format!("No keyframe for figure {} at frame {}.", fi + 1, from + 1);
        return false;
    }
    if from == 0 {
        core.status = "Frame 1 is the base pose — its keyframe cannot be moved.".into();
        return false;
    }
    if core.project.frames[from - 1].poses.get(fi).is_none() {
        core.status = format!(
            "Figure {} appears at frame {} — no earlier pose to hold.",
            fi + 1,
            from + 1
        );
        return false;
    }
    if to == from {
        return false;
    }
    if to >= core.project.frames.len() {
        core.status = format!(
            "Frame {} is past the last frame — the keyframe has nowhere to land.",
            to + 1
        );
        return false;
    }
    let (lo, hi) = if from < to { (from, to) } else { (to, from) };
    if let Some(&k) = keys.iter().find(|&&k| k != from && k >= lo && k <= hi) {
        core.status = format!(
            "Frame {} already has a keyframe for figure {}.",
            k + 1,
            fi + 1
        );
        return false;
    }
    for f in lo..=hi {
        if core.project.frames[f].poses.get(fi).is_none() {
            core.status = format!(
                "Figure {} is missing from frame {} in the move's span.",
                fi + 1,
                f + 1
            );
            return false;
        }
    }
    core.snapshot_labeled("Move pose key");
    let dir = if to > from { 1 } else { -1 };
    let mut at = from;
    while at != to {
        let stepped = shift_pose_key_once(core, fi, at, dir);
        debug_assert!(stepped, "a validated pose move steps cleanly");
        if !stepped {
            break;
        }
        at = (at as i32 + dir) as usize;
    }
    core.status = format!(
        "Moved figure {} keyframe from frame {} to frame {}.",
        fi + 1,
        from + 1,
        to + 1
    );
    true
}

/// One raw step of a state key: the stored key at
/// `at` (the LAST one when a hand-edited vec repeats
/// the frame — the model's later-wins tie rule makes
/// it the effective key) takes frame `at + dir`, and
/// the vec re-sorts; the frame's other entries do not
/// move. NO snapshot, NO status (see
/// [`shift_pose_key_once`]). `false` when no key sits
/// at `at`, the target is out of range, or another
/// state key already sits at the target (the upsert
/// would eat it).
fn shift_state_key_once(core: &mut EditorCore, fi: usize, at: usize, dir: i32) -> bool {
    let count = core.project.frames.len();
    let target = at as i32 + dir;
    if target < 0 || target as usize >= count {
        return false;
    }
    let target = target as usize;
    let Some(fig) = core.project.figures.get_mut(fi) else {
        return false;
    };
    if fig.states.iter().any(|k| k.frame == target) {
        return false;
    }
    let Some(idx) = fig.states.iter().rposition(|k| k.frame == at) else {
        return false;
    };
    fig.states[idx].frame = target;
    fig.states.sort_by_key(|k| k.frame);
    true
}

/// Move figure `fi`'s state key from `from` to `to`
/// as ONE undo step (Roadmap Wave 1, RM14 — P-2).
/// Refusals mirror [`move_pose_key`]'s honesty for
/// the sparse store: no state key at `from`; `to` out
/// of range or holding (or crossed by) another state
/// key of the figure.
pub fn move_state_key(core: &mut EditorCore, fi: usize, from: usize, to: usize) -> bool {
    let Some(fig) = core.project.figures.get(fi) else {
        return false;
    };
    if !fig.states.iter().any(|k| k.frame == from) {
        core.status = format!("No state key for figure {} at frame {}.", fi + 1, from + 1);
        return false;
    }
    if to == from {
        return false;
    }
    if to >= core.project.frames.len() {
        core.status = format!(
            "Frame {} is past the last frame — the state key has nowhere to land.",
            to + 1
        );
        return false;
    }
    let (lo, hi) = if from < to { (from, to) } else { (to, from) };
    if let Some(k) = core.project.figures[fi]
        .states
        .iter()
        .find(|k| k.frame != from && k.frame >= lo && k.frame <= hi)
    {
        core.status = format!(
            "Frame {} already has a state key for figure {}.",
            k.frame + 1,
            fi + 1
        );
        return false;
    }
    core.snapshot_labeled("Move state key");
    let dir = if to > from { 1 } else { -1 };
    let mut at = from;
    while at != to {
        let stepped = shift_state_key_once(core, fi, at, dir);
        debug_assert!(stepped, "a validated state move steps cleanly");
        if !stepped {
            break;
        }
        at = (at as i32 + dir) as usize;
    }
    core.status = format!(
        "Moved figure {} state key from frame {} to frame {}.",
        fi + 1,
        from + 1,
        to + 1
    );
    true
}

/// One raw step of a bend key: shift the bend change
/// at `at` one frame in direction `dir` — the pose
/// fill rule over normalised bend rows (later: `at`
/// takes the previous row and `at + 1` the key's row;
/// earlier: `at − 1` takes the key's row). NO
/// snapshot, NO status. `false` when the target is
/// out of range or is itself a bend-change frame.
fn shift_bend_key_once(core: &mut EditorCore, fi: usize, at: usize, dir: i32) -> bool {
    let count = core.project.frames.len();
    let target = at as i32 + dir;
    if target < 0 || target as usize >= count {
        return false;
    }
    let target = target as usize;
    if is_bend_key(&bend_key_frames(&core.project, fi), target) {
        return false;
    }
    let key_row = bend_row(&core.project, fi, at);
    if dir > 0 {
        let prev_row = prev_bend_row(&core.project, fi, at);
        set_bend_row(&mut core.project, fi, at, &prev_row);
        set_bend_row(&mut core.project, fi, target, &key_row);
    } else {
        set_bend_row(&mut core.project, fi, target, &key_row);
    }
    true
}

/// Move figure `fi`'s bend key from `from` to `to` as
/// ONE undo step (Roadmap Wave 1, RM14 — P-2): the
/// bend-table twin of [`move_pose_key`], over
/// normalised rows. Frame 0's bend key CAN move (its
/// predecessor is the implicit zero row — the pose
/// base rule does not transpose). Refusals: no bend
/// change at `from`; `to` out of range or holding (or
/// crossed by) another bend change.
pub fn move_bend_key(core: &mut EditorCore, fi: usize, from: usize, to: usize) -> bool {
    let keys = bend_key_frames(&core.project, fi);
    if !is_bend_key(&keys, from) {
        core.status = format!("No bend key for figure {} at frame {}.", fi + 1, from + 1);
        return false;
    }
    if to == from {
        return false;
    }
    if to >= core.project.frames.len() {
        core.status = format!(
            "Frame {} is past the last frame — the bend key has nowhere to land.",
            to + 1
        );
        return false;
    }
    let (lo, hi) = if from < to { (from, to) } else { (to, from) };
    if let Some(&k) = keys.iter().find(|&&k| k != from && k >= lo && k <= hi) {
        core.status = format!(
            "Frame {} already has a bend key for figure {}.",
            k + 1,
            fi + 1
        );
        return false;
    }
    core.snapshot_labeled("Move bend key");
    let dir = if to > from { 1 } else { -1 };
    let mut at = from;
    while at != to {
        let stepped = shift_bend_key_once(core, fi, at, dir);
        debug_assert!(stepped, "a validated bend move steps cleanly");
        if !stepped {
            break;
        }
        at = (at as i32 + dir) as usize;
    }
    core.status = format!(
        "Moved figure {} bend key from frame {} to frame {}.",
        fi + 1,
        from + 1,
        to + 1
    );
    true
}

/// Delete the key event at (`fi`, `frame`) through
/// its per-kind path (Roadmap Wave 1, RM14 — P-2),
/// under the lane's priority ([`key_event_at`]).
/// Channel keys go through `graph_keys::remove_key_at`
/// and pose keys through the engine's
/// `delete_keyframe` — both carry their own snapshot
/// and status. State keys remove every stored entry
/// at the frame (the mark is per-frame; a hand-edited
/// duplicate is the same event). A bend key holds the
/// previous row, exactly as `delete_keyframe` holds
/// bends. One undo step per landed delete; `false`
/// (the paths' own refusals) when nothing is there.
pub fn delete_key_event(core: &mut EditorCore, fi: usize, frame: usize) -> bool {
    let count = core.project.frames.len();
    match key_event_at(&core.project, fi, frame, count) {
        Some(DopeKeyRef::Channel {
            channel,
            key_frame,
        }) => stykanimate_rs::graph_keys::remove_key_at(core, channel, key_frame),
        Some(DopeKeyRef::Pose) => core.delete_keyframe(fi, frame),
        Some(DopeKeyRef::State) => {
            core.snapshot_labeled("Delete state key");
            core.project.figures[fi].states.retain(|k| k.frame != frame);
            core.status = format!(
                "Deleted figure {} state key at frame {}.",
                fi + 1,
                frame + 1
            );
            true
        }
        Some(DopeKeyRef::Bend) => {
            let prev = prev_bend_row(&core.project, fi, frame);
            core.snapshot_labeled("Delete bend key");
            set_bend_row(&mut core.project, fi, frame, &prev);
            core.status = format!(
                "Deleted figure {} bend key at frame {} — the frame now holds the previous bend.",
                fi + 1,
                frame + 1
            );
            true
        }
        None => {
            core.status = format!("No key for figure {} at frame {}.", fi + 1, frame + 1);
            false
        }
    }
}

/// Frames ahead of the source key that
/// [`stamp_key_event`] stamps its copy at — the
/// keyframe tools' `COPY_KEY_OFFSET_FRAMES`, the
/// shell's one stamp distance.
pub const STAMP_OFFSET_FRAMES: usize = 10;

/// Stamp a copy of the DENSE-store key at (`fi`,
/// `frame`) [`STAMP_OFFSET_FRAMES`] frames later, as
/// ONE undo step (Roadmap Wave 1, RM14 — P-2): the
/// Copy verb for the stores that have no clipboard.
/// Pose: the target frame takes the key's pose (and
/// bend row). State: the stored key clones at the
/// target. Bend: the target frame takes the key's
/// row. Refusals — status-only, no snapshot: the
/// target is past the last frame, already carries a
/// key of the same kind, or (bend) already holds the
/// key's row, so no change would appear. Channel keys
/// refuse here: they copy to the graph clipboard
/// ([`copy_active_keys`]' channel arm).
pub fn stamp_key_event(core: &mut EditorCore, fi: usize, frame: usize) -> bool {
    let count = core.project.frames.len();
    let target = frame + STAMP_OFFSET_FRAMES;
    match key_event_at(&core.project, fi, frame, count) {
        Some(DopeKeyRef::Pose) => {
            if target >= count {
                core.status = format!(
                    "Frame {} is past the last frame — the stamp has nowhere to land.",
                    target + 1
                );
                return false;
            }
            if is_key(&figure_keyframes(&core.project, fi), target) {
                core.status = format!(
                    "Frame {} already has a keyframe for figure {}.",
                    target + 1,
                    fi + 1
                );
                return false;
            }
            let (Some(pose), true) = (
                core.project.frames[frame].poses.get(fi).cloned(),
                core.project.frames[target].poses.get(fi).is_some(),
            ) else {
                core.status = format!(
                    "Figure {} is missing from frame {} — the stamp cannot land there.",
                    fi + 1,
                    target + 1
                );
                return false;
            };
            let row = bend_row(&core.project, fi, frame);
            core.snapshot_labeled("Stamp pose key");
            core.project.frames[target].poses[fi] = pose;
            set_bend_row(&mut core.project, fi, target, &row);
            core.status = format!(
                "Copied figure {} keyframe from frame {} to frame {}.",
                fi + 1,
                frame + 1,
                target + 1
            );
            true
        }
        Some(DopeKeyRef::State) => {
            if target >= count {
                core.status = format!(
                    "Frame {} is past the last frame — the stamp has nowhere to land.",
                    target + 1
                );
                return false;
            }
            let fig = &core.project.figures[fi];
            if fig.states.iter().any(|k| k.frame == target) {
                core.status = format!(
                    "Frame {} already has a state key for figure {}.",
                    target + 1,
                    fi + 1
                );
                return false;
            }
            let Some(mut key) = fig
                .states
                .iter()
                .rev()
                .find(|k| k.frame == frame)
                .cloned()
            else {
                return false;
            };
            key.frame = target;
            core.snapshot_labeled("Stamp state key");
            let fig = &mut core.project.figures[fi];
            fig.states.push(key);
            fig.states.sort_by_key(|k| k.frame);
            core.status = format!(
                "Copied figure {} state key from frame {} to frame {}.",
                fi + 1,
                frame + 1,
                target + 1
            );
            true
        }
        Some(DopeKeyRef::Bend) => {
            if target >= count {
                core.status = format!(
                    "Frame {} is past the last frame — the stamp has nowhere to land.",
                    target + 1
                );
                return false;
            }
            if is_bend_key(&bend_key_frames(&core.project, fi), target) {
                core.status = format!(
                    "Frame {} already has a bend key for figure {}.",
                    target + 1,
                    fi + 1
                );
                return false;
            }
            let row = bend_row(&core.project, fi, frame);
            let before = bend_row(&core.project, fi, target - 1);
            if row.len() == before.len()
                && row
                    .iter()
                    .zip(before.iter())
                    .all(|(a, b)| (a - b).abs() <= BEND_EPS)
            {
                core.status = format!(
                    "Frame {} already holds this bend — the stamp would change nothing.",
                    target + 1
                );
                return false;
            }
            core.snapshot_labeled("Stamp bend key");
            set_bend_row(&mut core.project, fi, target, &row);
            core.status = format!(
                "Copied figure {} bend key from frame {} to frame {}.",
                fi + 1,
                frame + 1,
                target + 1
            );
            true
        }
        Some(DopeKeyRef::Channel { .. }) => {
            core.status =
                "Graph keys copy to the clipboard — Copy key already put them there.".into();
            false
        }
        None => {
            core.status = format!("No key for figure {} at frame {}.", fi + 1, frame + 1);
            false
        }
    }
}

/// The channel a clipboard paste lands on for figure
/// `fi`: the channel carrying the clipboard's exact
/// target when the figure has one, else the figure's
/// FIRST channel of the same target KIND (project
/// order) — AC1's kind looseness (a copy from joint 1
/// pastes onto joint 2's angle channel: degrees to
/// degrees), resolved with the lane's first-match
/// discipline. `None` when the figure has no channel
/// of the kind at all.
pub fn paste_target_channel(
    core: &EditorCore,
    fi: usize,
    target: stykanimate_rs::graph::ChannelTarget,
) -> Option<usize> {
    if let Some(ci) = stykanimate_rs::graph_keys::channel_index_of(core, fi, target) {
        return Some(ci);
    }
    core.project
        .channels
        .iter()
        .position(|ch| ch.figure == fi && same_target_kind(ch.target, target))
}

/// Move the ACTIVE keys by `delta` whole frames (the
/// actions row's Move −1/+1): the live selection when
/// there is one (through `key_selection`'s one-step
/// move), else the key event under the playhead on
/// the selected figure, through its per-kind path — a
/// channel event selects itself first, so the button
/// and the drag share one resolution. Refusals are
/// the paths' own, status-only.
pub fn move_active_keys(s: &mut State, delta: i32) -> bool {
    if selection_is_live(&s.core, &s.key_sel) {
        return move_selected_keys(&mut s.core, &mut s.key_sel, delta) > 0;
    }
    let fi = s.core.selected_figure;
    let frame = s.core.current;
    let count = s.core.frame_count();
    match key_event_at(&s.core.project, fi, frame, count) {
        Some(DopeKeyRef::Channel {
            channel,
            key_frame,
        }) => {
            s.key_sel.select_one(channel, key_frame);
            move_selected_keys(&mut s.core, &mut s.key_sel, delta) > 0
        }
        Some(DopeKeyRef::Pose) => {
            let to = frame as i32 + delta;
            if to < 0 {
                s.core.status = "The keyframe is already at the first frame.".into();
                return false;
            }
            move_pose_key(&mut s.core, fi, frame, to as usize)
        }
        Some(DopeKeyRef::State) => {
            let to = frame as i32 + delta;
            if to < 0 {
                s.core.status = "The state key is already at the first frame.".into();
                return false;
            }
            move_state_key(&mut s.core, fi, frame, to as usize)
        }
        Some(DopeKeyRef::Bend) => {
            let to = frame as i32 + delta;
            if to < 0 {
                s.core.status = "The bend key is already at the first frame.".into();
                return false;
            }
            move_bend_key(&mut s.core, fi, frame, to as usize)
        }
        None => {
            s.core.status = "No key here to move.".into();
            false
        }
    }
}

/// Copy the ACTIVE keys (the actions row's Copy key):
/// channel keys — the live selection, or the single
/// playhead channel key — go to the shell's graph
/// clipboard, tagged exactly as the keyframe tools'
/// own copy tags them, so either surface's paste
/// accepts them; dense-store keys stamp
/// [`STAMP_OFFSET_FRAMES`] frames later
/// ([`stamp_key_event`]). Copying to the clipboard
/// never snapshots (nothing in the project changes);
/// a stamp is one undo step, like every landed edit.
pub fn copy_active_keys(s: &mut State) -> bool {
    if selection_is_live(&s.core, &s.key_sel) {
        let ci = s.key_sel.channel.expect("a live selection names a channel");
        let keys = copy_selected_keys(&s.core, &s.key_sel).expect("a live selection copies");
        set_clipboard(s, ci, keys);
        return true;
    }
    let fi = s.core.selected_figure;
    let frame = s.core.current;
    let count = s.core.frame_count();
    match key_event_at(&s.core.project, fi, frame, count) {
        Some(DopeKeyRef::Channel {
            channel,
            key_frame,
        }) => {
            let ch = &s.core.project.channels[channel];
            let Some(key) = ch
                .keys
                .iter()
                .find(|k| (k.frame - key_frame).abs() <= KEY_MATCH_TOL)
                .copied()
            else {
                return false;
            };
            set_clipboard(s, channel, vec![key]);
            true
        }
        Some(_) => stamp_key_event(&mut s.core, fi, frame),
        None => {
            s.core.status = "No key here to copy.".into();
            false
        }
    }
}

/// Put `keys` from channel `ci` onto the shell's
/// graph clipboard with AC1's tag (source figure +
/// target), and say so in the clipboard's own shape.
fn set_clipboard(s: &mut State, ci: usize, keys: Vec<Key>) {
    let ch = &s.core.project.channels[ci];
    let (figure, target) = (ch.figure, ch.target);
    let n = keys.len();
    s.key_clipboard = Some(KeyClipboard {
        figure,
        target,
        keys,
    });
    s.core.status = format!(
        "Copied {} graph key{} from figure {} {}.",
        n,
        if n == 1 { "" } else { "s" },
        figure + 1,
        channel_label(target)
    );
}

/// Delete the ACTIVE keys (the actions row's Delete
/// key): the live selection through
/// `delete_selected_keys` (one undo step for the
/// batch), else the playhead event through
/// [`delete_key_event`].
pub fn delete_active_keys(s: &mut State) -> bool {
    if selection_is_live(&s.core, &s.key_sel) {
        return delete_selected_keys(&mut s.core, &mut s.key_sel) > 0;
    }
    let fi = s.core.selected_figure;
    let frame = s.core.current;
    if key_event_at(&s.core.project, fi, frame, s.core.frame_count()).is_none() {
        s.core.status = "No key here to delete.".into();
        return false;
    }
    delete_key_event(&mut s.core, fi, frame)
}

/// Paste the graph clipboard at the playhead onto
/// the selected figure (the actions row's Paste
/// keys): the target channel resolves by
/// [`paste_target_channel`] (exact target, else first
/// of the kind), and `graph_keys::paste_keys` does
/// the write — rebased so the clipboard's earliest
/// key lands on the playhead, one undo step, its own
/// status. Refusals: no clipboard, or no channel of
/// the clipboard's kind on the selected figure.
pub fn paste_clipboard_at_playhead(s: &mut State) -> bool {
    let Some(clip) = s.key_clipboard.clone() else {
        s.core.status = "Nothing to paste — copy some graph keys first.".into();
        return false;
    };
    let fi = s.core.selected_figure;
    let Some(ci) = paste_target_channel(&s.core, fi, clip.target) else {
        s.core.status = format!(
            "No {} channel on figure {} to paste into.",
            channel_label(clip.target),
            fi + 1
        );
        return false;
    };
    stykanimate_rs::graph_keys::paste_keys(&mut s.core, ci, &clip.keys, s.core.current as f32)
}

// ---------------------------------------------------------------------
// Cell drag (Roadmap Wave 1, RM14 — P-2): the gesture
// composition the module docs describe. The handlers
// are factored to take the gesture record as a
// parameter (the subtitle lane's `CueDrag` shape), so
// every rule is pinned by unit tests driving them
// directly; `cell_drag` is the thin runtime adapter
// over the module's `thread_local`.
// ---------------------------------------------------------------------

/// One in-flight dopesheet drag: everything the
/// gesture's later phases need that the pointer alone
/// cannot say. For a channel drag the record names
/// the channel and the dragged set AS GRABBED (the
/// selection may be rewritten by another surface
/// mid-gesture; the identity check compares figure +
/// target, the channel's stable identity); for the
/// dense stores it tracks the key's CURRENT frame
/// (`at`), each step re-deriving the key from the
/// store, so a mid-gesture rewrite that removes the
/// key simply stops the stepping.
#[derive(Clone, Debug, PartialEq)]
pub struct DopeDrag {
    /// The grabbed row's figure.
    pub fi: usize,
    /// The grabbed cell's frame (the gesture's origin).
    pub frame: usize,
    /// Which store the grabbed key lives in.
    pub kind: DopeKeyRef,
    /// Channel drags: the grabbed channel's target —
    /// half of the channel's identity guard (the slot
    /// plus this target; a replaced channel at the
    /// same slot with a different target is a different
    /// channel, and the drag abandons).
    pub ch_target: Option<stykanimate_rs::graph::ChannelTarget>,
    /// Channel drags: the delta currently applied to
    /// the dragged set (dense drags track `at` instead).
    pub applied_delta: i32,
    /// Dense drags: the key's current frame.
    pub at: usize,
    /// Whether the gesture's one snapshot is taken —
    /// staged lazily by the first Move that actually
    /// moves a key, so a plain click spends no undo
    /// step.
    pub snapshot_taken: bool,
    /// Whether any key has actually moved.
    pub moved: bool,
}

thread_local! {
    /// The dopesheet drag in flight, if any. A
    /// `thread_local` because it is gesture state with
    /// the gesture's own lifetime — the timeline's
    /// `PRESSED_CHIP` / the subtitle lane's `CUE_DRAG`
    /// pattern. Written and read only by `cell_drag`;
    /// the factored handlers take the record as a
    /// parameter and never touch this cell.
    static DOPE_DRAG: RefCell<Option<DopeDrag>> = const { RefCell::new(None) };
}

/// Whether the record's channel still names the
/// channel it grabbed (same slot, same figure +
/// target): the guard every writing phase of a
/// channel drag runs.
fn drag_channel_holds(s: &State, g: &DopeDrag) -> bool {
    let DopeKeyRef::Channel {
        channel,
        key_frame: _,
    } = g.kind
    else {
        return false;
    };
    s.core
        .project
        .channels
        .get(channel)
        .is_some_and(|ch| ch.figure == g.fi && Some(ch.target) == g.ch_target)
}

/// The gesture's Down phase: the click, then the
/// grab. The click is the cell's historical press
/// verbatim — select the figure, go to the frame, and
/// (for a channel event) the F-4 selection — because
/// the framework delivers `on_press` only on release
/// and after the drag's Up, so a draggable cell folds
/// its click into Down (the subtitle lane's
/// settlement). The grab then plants the record from
/// the event the cell carries, with the channel
/// refinement the graph drag taught: a channel key
/// that is part of the live selection drags the WHOLE
/// selection; any other channel key selects itself
/// first. A pose appearance/base key plants no record
/// (it cannot move — its click still happened). A
/// record left over from an earlier gesture (its Up
/// never arrived) is settled first — rolled back if
/// it had moved anything.
pub fn drag_down(s: &mut State, drag: &mut Option<DopeDrag>, fi: usize, frame: usize) {
    if let Some(old) = drag.take() {
        if old.snapshot_taken {
            s.core.undo();
            if let DopeKeyRef::Channel { channel, .. } = old.kind {
                if let Some(ch) = s.core.project.channels.get(channel) {
                    let ch = ch.clone();
                    s.key_sel.retain_live(channel, &ch);
                }
            }
        }
    }
    s.core.select_figure(fi);
    s.core.goto(frame);
    let count = s.core.frame_count();
    let event = key_event_at(&s.core.project, fi, frame, count);
    let record = match event {
        Some(kind @ DopeKeyRef::Channel { channel, key_frame }) => {
            let ch = &s.core.project.channels[channel];
            let in_selection =
                s.key_sel.channel == Some(channel) && s.key_sel.contains(channel, ch, key_frame);
            if !in_selection {
                s.key_sel.select_one(channel, key_frame);
            }
            Some(DopeDrag {
                fi,
                frame,
                kind,
                ch_target: Some(ch.target),
                applied_delta: 0,
                at: frame,
                snapshot_taken: false,
                moved: false,
            })
        }
        Some(kind @ DopeKeyRef::Pose) => {
            // The base key and an appearance key
            // cannot move (move_pose_key's refusals):
            // no record — the click above is the whole
            // gesture.
            let movable = frame > 0
                && s.core.project.frames[frame]
                    .poses
                    .get(fi)
                    .is_some()
                && s.core.project.frames[frame - 1]
                    .poses
                    .get(fi)
                    .is_some();
            movable.then(|| DopeDrag {
                fi,
                frame,
                kind,
                ch_target: None,
                applied_delta: 0,
                at: frame,
                snapshot_taken: false,
                moved: false,
            })
        }
        Some(kind @ (DopeKeyRef::State | DopeKeyRef::Bend)) => Some(DopeDrag {
            fi,
            frame,
            kind,
            ch_target: None,
            applied_delta: 0,
            at: frame,
            snapshot_taken: false,
            moved: false,
        }),
        None => None,
    };
    *drag = record;
}

/// The gesture's Move phase (and the release's final
/// application): step the grabbed key toward the
/// pointer's whole-frame travel ([`frame_delta`] over
/// `dx`). Steps are one frame at a time under each
/// store's own collision rule, so a blocked direction
/// simply stops the key at the last frame it could
/// reach — the rest of the travel is ignored, never
/// forced. The first step that lands stages the
/// gesture's ONE snapshot. A channel drag whose
/// channel was rewritten mid-gesture (identity lost)
/// abandons: the record drops and nothing more is
/// written — the staged snapshot, if any, stays as
/// the honest pre-drag state.
pub fn drag_apply(s: &mut State, drag: &mut Option<DopeDrag>, dx: f32) {
    let Some(g) = drag.as_ref() else {
        return;
    };
    let target_delta = frame_delta(dx, s.frame_ops.chip_w());
    match g.kind {
        DopeKeyRef::Channel { .. } => {
            if !drag_channel_holds(s, g) {
                *drag = None;
                return;
            }
            loop {
                let applied = drag.as_ref().map(|g| g.applied_delta).unwrap_or(0);
                if applied == target_delta {
                    break;
                }
                let step = if target_delta > applied { 1 } else { -1 };
                if selected_move_error(&s.core, &s.key_sel, step).is_some() {
                    break;
                }
                let Some(g) = drag.as_mut() else { break };
                if !g.snapshot_taken {
                    s.core.snapshot_labeled("Move graph keys");
                    g.snapshot_taken = true;
                }
                move_selected_keys_by(&mut s.core, &mut s.key_sel, step);
                let g = drag.as_mut().expect("the record survives its step");
                g.applied_delta += step;
                g.moved = true;
            }
        }
        DopeKeyRef::Pose | DopeKeyRef::State | DopeKeyRef::Bend => {
            let count = s.core.frame_count();
            if count == 0 {
                return;
            }
            let origin = g.frame as i32;
            let target_at = (origin + target_delta).clamp(0, count as i32 - 1) as usize;
            loop {
                let Some(g) = drag.as_ref() else { break };
                if g.at == target_at {
                    break;
                }
                let dir = if target_at > g.at { 1 } else { -1 };
                let at = g.at;
                let kind = g.kind;
                let fi = g.fi;
                if !step_valid(&s.core, kind, fi, at, dir) {
                    break;
                }
                // The step is known to land: stage the
                // gesture's ONE snapshot BEFORE the
                // first write, never after.
                let Some(g) = drag.as_mut() else { break };
                if !g.snapshot_taken {
                    s.core.snapshot_labeled("Move key");
                    g.snapshot_taken = true;
                }
                let stepped = match kind {
                    DopeKeyRef::Pose => shift_pose_key_once(&mut s.core, fi, at, dir),
                    DopeKeyRef::State => shift_state_key_once(&mut s.core, fi, at, dir),
                    DopeKeyRef::Bend => shift_bend_key_once(&mut s.core, fi, at, dir),
                    DopeKeyRef::Channel { .. } => unreachable!(),
                };
                debug_assert!(stepped, "a validated step lands");
                if !stepped {
                    break;
                }
                let Some(g) = drag.as_mut() else { break };
                g.at = (at as i32 + dir) as usize;
                g.moved = true;
            }
        }
    }
}

/// The gesture's Up phase. Cancelled: the staged
/// snapshot, if one was taken, rolls back with one
/// `undo()` — the pre-drag project returns and the
/// net undo cost of the gesture is zero (the subtitle
/// lane's rule); a channel drag also prunes its
/// selection to what is live again. Clean release:
/// the release position applies once more, then a
/// gesture that moved settles its status sentence. A
/// gesture that never moved was a click: [`drag_down`]
/// already did its whole work, and no undo step was
/// ever staged.
pub fn drag_up(s: &mut State, drag: &mut Option<DopeDrag>, dx: f32, cancelled: bool) {
    if cancelled {
        if let Some(g) = drag.take() {
            if g.snapshot_taken {
                s.core.undo();
                if let DopeKeyRef::Channel { channel, .. } = g.kind {
                    if let Some(ch) = s.core.project.channels.get(channel) {
                        let ch = ch.clone();
                        s.key_sel.retain_live(channel, &ch);
                    }
                }
            }
        }
        return;
    }
    drag_apply(s, drag, dx);
    let Some(g) = drag.take() else {
        return;
    };
    if !g.moved {
        return;
    }
    match g.kind {
        DopeKeyRef::Channel { channel, .. } => {
            let n = s.key_sel.len();
            if let Some(ch) = s.core.project.channels.get(channel) {
                s.core.status = format!(
                    "Moved {} selected key{} by {:+} frame{} on figure {} {}.",
                    n,
                    if n == 1 { "" } else { "s" },
                    g.applied_delta,
                    if g.applied_delta.abs() == 1 { "" } else { "s" },
                    ch.figure + 1,
                    channel_label(ch.target)
                );
            }
        }
        DopeKeyRef::Pose => {
            s.core.status = format!(
                "Moved figure {} keyframe from frame {} to frame {}.",
                g.fi + 1,
                g.frame + 1,
                g.at + 1
            );
        }
        DopeKeyRef::State => {
            s.core.status = format!(
                "Moved figure {} state key from frame {} to frame {}.",
                g.fi + 1,
                g.frame + 1,
                g.at + 1
            );
        }
        DopeKeyRef::Bend => {
            s.core.status = format!(
                "Moved figure {} bend key from frame {} to frame {}.",
                g.fi + 1,
                g.frame + 1,
                g.at + 1
            );
        }
    }
}

/// A figure cell's drag handler — the thin adapter
/// between the runtime's [`Drag`] and the factored
/// handlers above: Down plants the record from the
/// grabbed cell, Move and Up read the pointer's total
/// travel (`Drag::dx` — window points since the press,
/// unaffected by the cell itself being rebuilt as its
/// key is written live) and the release's
/// cancellation.
fn cell_drag(s: &mut State, d: &mut Drag, fi: usize, frame: usize) {
    DOPE_DRAG.with(|cell| {
        let mut slot = cell.borrow_mut();
        match d.phase {
            Phase::Down => drag_down(s, &mut slot, fi, frame),
            Phase::Move => drag_apply(s, &mut slot, d.dx),
            Phase::Up => drag_up(s, &mut slot, d.dx, d.cancelled()),
        }
    });
}

/// The key-editing actions row (Roadmap Wave 1,
/// RM14 — P-2): mounted by [`dopesheet_lane`] directly
/// under the header while there is something to act
/// on — the Move/Copy/Delete buttons while a live
/// selection or a playhead key event exists, Paste
/// while the clipboard's kind has a channel on the
/// selected figure — and not at all otherwise (no
/// dead controls). Labels are literal English, the
/// graph workspace's standing posture (IA-RM14-2).
fn key_actions_row(s: &State, t: &Theme) -> Option<El> {
    let core = &s.core;
    let live = selection_is_live(core, &s.key_sel);
    let has_event = live
        || key_event_at(
            &core.project,
            core.selected_figure,
            core.current,
            core.frame_count(),
        )
        .is_some();
    let pasteable = s
        .key_clipboard
        .as_ref()
        .is_some_and(|c| paste_target_channel(core, core.selected_figure, c.target).is_some());
    if !has_event && !pasteable {
        return None;
    }
    let mut row_el = row().key("dopesheet-actions").gap(4.0).center_y();
    if has_event {
        row_el = row_el
            .child(btn("dope-key-move-left", "Move key -1", false, t, |s| {
                move_active_keys(s, -1);
            }))
            .child(btn("dope-key-move-right", "Move key +1", false, t, |s| {
                move_active_keys(s, 1);
            }))
            .child(btn("dope-key-copy", "Copy key", false, t, |s| {
                copy_active_keys(s);
            }))
            .child(btn("dope-key-delete", "Delete key", false, t, |s| {
                delete_active_keys(s);
            }));
    }
    if pasteable {
        row_el = row_el.child(btn("dope-key-paste", "Paste keys", false, t, |s| {
            paste_clipboard_at_playhead(s);
        }));
    }
    Some(row_el)
}

/// The total width of one figure row (and of the
/// header row, which mirrors the row's box so its
/// readout can sit over the last frame column): the
/// header cell, one chip gap, then one chip slot per
/// frame at the chips' pitch. Zero frames is just the
/// header cell.
pub fn lane_width(frame_count: usize, chip_w: f32) -> f32 {
    if frame_count == 0 {
        return DOPE_HEADER_W;
    }
    DOPE_HEADER_W
        + CHIP_GAP
        + frame_count as f32 * chip_w
        + (frame_count - 1) as f32 * CHIP_GAP
}

/// The "+N more figures" line for the figures the row
/// cap left out — a count, honestly labelled, with the
/// singular/plural the number needs.
pub fn more_label(more: usize) -> String {
    if more == 1 {
        "+1 more figure".to_string()
    } else {
        format!("+{more} more figures")
    }
}

// ---------------------------------------------------------------------
// Sprite rows (Wave X, X5 — SpriteLoop GAP P-3's
// dopesheet lane: display + navigation for the free
// sprite instances, which the figure sheet above never
// knew about).
// ---------------------------------------------------------------------

/// The whole frames at which sprite instance `inst`
/// holds a STORED transform key, ascending, deduplicated
/// — the sprite row's diamonds. Instance keys are
/// authored at whole frames by every op in the shell;
/// an imported key at a fractional frame (`.nodes`
/// writes frames as text) names no cell of a whole-frame
/// sheet and is skipped rather than rounded onto a frame
/// it was not authored at — it stays fully visible in
/// the Sprites panel and `sprite_list`. A stale key past
/// the timeline's end likewise has no cell to mark.
pub fn sprite_key_frames(inst: &SpriteInstance) -> Vec<usize> {
    let mut frames: Vec<usize> = inst
        .keys
        .iter()
        .filter(|k| k.frame >= 0.0 && k.frame.fract() == 0.0)
        .map(|k| k.frame as usize)
        .collect();
    frames.sort_unstable();
    frames.dedup();
    frames
}

/// One shown sprite row's data, the [`DopeRow`]
/// discipline for instances: computed once per render
/// by [`sprite_dope_rows`].
pub struct SpriteDopeRow {
    /// The instance's index in `Project.sprites`.
    pub index: usize,
    /// The asset's display name (the Sprites panel's
    /// own `asset_name`, missing-asset fallback and
    /// all; fitted only at paint time).
    pub name: String,
    /// The instance's stored-key frames, ascending
    /// ([`sprite_key_frames`]' output, verbatim).
    pub keys: Vec<usize>,
    /// This is the Sprites panel's selected instance —
    /// sprite selection is panel state (there is no
    /// core sprite selection), so the caller passes it
    /// in, exactly as the stage's sprite prims take it.
    pub selected: bool,
}

/// The shown sprite rows, in display order: table
/// order capped at [`DOPE_MAX_ROWS`] with the panel-
/// selected instance pinned first past the cap — the
/// figure rows' [`dope_row_order`] rule, reused
/// verbatim (its "selected" is an index into the same
/// kind of table). A stale panel selection pins
/// nothing.
pub fn sprite_dope_rows(
    core: &EditorCore,
    panel_selected: Option<usize>,
) -> Vec<SpriteDopeRow> {
    let project = &core.project;
    dope_row_order(
        project.sprites.len(),
        panel_selected.unwrap_or(usize::MAX),
    )
    .into_iter()
    .map(|index| {
        let inst = &project.sprites[index];
        SpriteDopeRow {
            index,
            name: crate::rune_sec::sprites_panel::asset_name(core, inst.asset),
            keys: sprite_key_frames(inst),
            selected: panel_selected == Some(index),
        }
    })
    .collect()
}

/// The sprite section's enrichment readout (Wave Y,
/// Y14): for the Sprites panel's selected instance,
/// the key SAMPLED at the current frame, rendered as
/// one line — `Sprite {n} · frame {f} · ` plus the
/// panel's own `enrichment_readout` wording, so the
/// two surfaces can never drift. The sample is what
/// the stage draws at the playhead: scrubbing is
/// exactly when an imported sprite's opacity / flip
/// / scale_y / pivot changes need to be readable
/// without leaving the timeline. `None` when no
/// instance is selected, the selection is stale, or
/// the instance has no key to sample.
pub fn sprite_enrichment_line(core: &EditorCore, selected: Option<usize>) -> Option<String> {
    let index = selected?;
    let inst = core.project.sprites.get(index)?;
    let key = stykanimate_rs::model::InstanceKey::sample(&inst.keys, core.current as f32)?;
    Some(format!(
        "Sprite {} · frame {} · {}",
        index + 1,
        core.current + 1,
        crate::rune_sec::sprites_panel::enrichment_readout(&key)
    ))
}

/// The "+N more sprites" line for the instances the
/// row cap left out — [`more_label`]'s sprite twin.
pub fn more_sprites_label(more: usize) -> String {
    if more == 1 {
        "+1 more sprite".to_string()
    } else {
        format!("+{more} more sprites")
    }
}

/// The dopesheet lane: header, then either an empty
/// state (no figures at all, or figures but no
/// keyframes anywhere) or one row per shown figure
/// plus the "+N more figures" line when the cap cut
/// any. Built to be appended to the timeline's scrolled
/// content column — see the module docs for the wiring
/// and the scroll-sharing mechanism.
pub fn dopesheet_lane(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let frame_count = core.frame_count();

    // The header row: the lane's name at the left,
    // localised through the i18n table's `dopesheet`
    // key (all 28 languages; Wave 10 integration wired
    // the accessor A9 registered), the current-frame
    // readout in mono at the right.
    // The row is sized to the figure rows' width so the
    // readout lands over the last frame column; on a
    // zero-frame project there are no columns to mirror
    // and the two simply sit together.
    let mut header = row().gap(8.0).center_y();
    if frame_count > 0 {
        header = header.size(lane_width(frame_count, s.frame_ops.chip_w()), DOPE_HEADER_H);
    } else {
        header = header.h(DOPE_HEADER_H);
    }
    let header = header
        .child(design::section_label(
            &crate::rune_sec::i18n_wire::tr(core, "dopesheet"),
            t,
        ))
        .child(el().grow(1.0))
        .child(
            text(frame_readout(core.current, frame_count))
                .font(type_::MONO_CAPTION.mono())
                .color(t.text_secondary),
        );

    let mut lane = col().key("dopesheet-lane").gap(2.0).child(header);

    // The key-editing actions row (RM14 — P-2):
    // mounted while a live selection, a playhead key
    // event, or a pasteable clipboard gives it work —
    // including above the empty states below, where a
    // paste is exactly how a first channel key can
    // arrive on a keyless-but-channelled project.
    if let Some(actions) = key_actions_row(s, t) {
        lane = lane.child(actions);
    }

    // The sprite section (Wave X, X5) renders on its own
    // condition — instances exist — not on the figure
    // sheet's: a project can carry sprites before it has
    // a single figure key, and the lane is their
    // navigation surface too. The two empty states below
    // therefore apply only when NEITHER section has
    // anything to show.
    let sprite_rows = sprite_dope_rows(core, s.sprites_panel.selected);
    // Wave Y (Y11): state keys speak for the figure
    // rows too — a figure can be fully clip-driven
    // (state keys keyed, timeline poses static or even
    // absent), and its rows are where those keys show.
    // Wave AA (AA3): channel keys speak the same way —
    // a figure can be fully graph-driven (channels
    // keyed, timeline poses static or even absent).
    // Wave AB (AB3): stored bend changes speak the
    // same way again — a figure can be bent entirely
    // from the pose tools with static poses and no
    // channels at all.
    let figures_keyed = any_keyframes(core)
        || any_state_keys(core)
        || any_channel_keys(core)
        || any_bend_keys(core);

    if core.project.figures.is_empty() && sprite_rows.is_empty() {
        // No figures and no sprites: nothing can have a
        // keyframe, and the next action lives in the
        // Figures panel — the message says so in words;
        // this lane has no figure-creating handler of
        // its own to offer as a button, and a fake one
        // is not an option.
        return lane.child(design::empty_state(
            "person",
            "No figures yet — add a figure from the Figures panel to start posing.",
            t,
        ));
    }
    if !figures_keyed && sprite_rows.is_empty() {
        // Figures but no keys anywhere, and no sprites:
        // posing a figure on a frame is exactly how a
        // key comes to exist (see the module docs), so
        // the state can name the real next action.
        return lane.child(design::empty_state(
            "film",
            "No keyframes yet — pose a figure on a frame to create one.",
            t,
        ));
    }

    if figures_keyed {
        let rows = dope_rows(core);
        for r in &rows {
            lane = lane.child(figure_row(r, core, &s.key_sel, s.frame_ops.chip_w(), t));
        }
        let more = core.project.figures.len() - rows.len();
        if more > 0 {
            lane = lane.child(
                row()
                    .h(DOPE_HEADER_H)
                    .center_y()
                    .child(text(more_label(more)).font(type_::LABEL).color(t.text_secondary)),
            );
        }
    }

    if !sprite_rows.is_empty() {
        lane = lane.child(
            row()
                .h(DOPE_HEADER_H)
                .center_y()
                .child(design::section_label("Sprites", t)),
        );
        for r in &sprite_rows {
            lane = lane.child(sprite_row(r, core, s.frame_ops.chip_w(), t));
        }
        let more = core.project.sprites.len() - sprite_rows.len();
        if more > 0 {
            lane = lane.child(
                row()
                    .h(DOPE_HEADER_H)
                    .center_y()
                    .child(
                        text(more_sprites_label(more))
                            .font(type_::LABEL)
                            .color(t.text_secondary),
                    ),
            );
        }
        // The selected sprite's enrichment at the
        // playhead (Wave Y, Y14), under its rows —
        // the "+N more" line's row shape, keyed so
        // the lane's census can see it.
        if let Some(line) = sprite_enrichment_line(core, s.sprites_panel.selected) {
            lane = lane.child(
                row()
                    .key("dope-sprite-readout")
                    .h(DOPE_HEADER_H)
                    .center_y()
                    .child(text(line).font(type_::LABEL).color(t.text_secondary)),
            );
        }
    }
    lane
}

/// One figure's row: the header cell (dot + fitted
/// name, press selects the figure) then one cell per
/// frame at the chips' pitch (press selects the figure
/// and goes to the frame). The selected figure's row
/// carries the kit's selected tint behind the whole
/// row; the current frame's cell carries it too, in
/// every row.
fn figure_row(r: &DopeRow, core: &EditorCore, ksel: &KeySel, chip_w: f32, t: &Theme) -> El {
    let fi = r.fi;
    let dot = hex_color(r.color).unwrap_or(Color::WHITE);
    let header = row()
        .key(format!("dope-row-{fi}"))
        .size(DOPE_HEADER_W, DOPE_ROW_H)
        .px(6.0)
        .gap(6.0)
        .center_y()
        .radius(design::ROW_RADIUS)
        .hover(t.hover)
        .press(t.press)
        .on_press(move |s: &mut State| {
            s.core.select_figure(fi);
            crate::stage_view::note_character_selected(fi);
        })
        .child(el().size(10.0, 10.0).radius(5.0).fill(dot))
        .child(
            text(fit_dope_name(&r.name))
                .font(type_::LABEL)
                .truncate()
                .color(if r.hidden { t.text_secondary } else { t.text }),
        );

    let mut cells = row().gap(CHIP_GAP).child(header);
    for frame in 0..core.frame_count() {
        // The cell's channel-key event (F-4): the drag's
        // Down selects it in the shared model beside the
        // historical select + goto; a cell with no
        // channel key leaves the selection untouched.
        // The cell carries a DRAG, not a press (RM14 —
        // P-2): the click folds into the drag's Down,
        // the subtitle lane's settlement, so the same
        // gesture that navigates can also move the key
        // the cell carries (see `drag_down`).
        let sel_event = channel_key_event(&core.project, fi, frame, core.frame_count());
        let mut cell = el()
            .key(format!("dope-cell-{fi}-{frame}"))
            .size(chip_w, DOPE_ROW_H)
            .radius(design::CONTROL_RADIUS)
            .center()
            .hover(t.control_track)
            .press(t.press)
            .on_drag(move |s: &mut State, d: &mut Drag, _tx: &mut Tx| {
                cell_drag(s, d, fi, frame)
            });
        if frame == core.current {
            cell = cell.fill(design::selected_fill());
        }
        // Wave Y (Y11): the state channel — a state key
        // alone shows the hollow diamond in the
        // secondary accent; a frame holding both kinds
        // keeps the pose diamond and gains a
        // secondary-accent border, so neither event is
        // lost to the coincidence.
        // Wave AA (AA3): the graph channel — a channel
        // key (any target) shows the marker row's dot in
        // the secondary text tone; beside a pose or
        // state glyph it rides as the second glyph in a
        // gap-2 row (the marker row's own two-glyph
        // cell), so a pose+state+channel frame keeps
        // the state border AND the dot and all three
        // events stay visible. A lone kind of any of
        // the three keeps its historical single text
        // child — the trees below change only where a
        // channel key actually sits.
        // Wave AB (AB3): the bend table — a stored bend
        // change on this frame adds the bottom bar
        // under whatever the glyph composition already
        // made (see `strip_cell`); it is a position
        // channel, not a fourth glyph, so the full
        // pose+state+channel+bend stack keeps the
        // border, both glyphs AND the bar, and a cell
        // with no bend change keeps its historical
        // single child exactly.
        // RM25 (PL-1): on the selected tint — this
        // cell's own fill on the current frame, the
        // row's fill on the selected figure — the pose
        // and state glyphs (and the coincidence
        // border) take app_theme's on-tint variants:
        // the plain accents measure 3.07 dark / 2.80
        // light on one tint layer and 2.31 / 2.51
        // (state border colour 2.32 dark) where the
        // two layers stack, under the 3:1 non-text
        // guidance; the variants hold ≥ 3.54 in the
        // worst case (app_theme's Polish re-audit).
        let on_tint = frame == core.current || r.selected;
        let state_key = is_state_key(&r.state_keys, frame);
        let channel_key = is_channel_key(&r.channel_keys, frame);
        let primary = if is_key(&r.keys, frame) {
            if state_key {
                cell = cell.border(
                    1.0,
                    if on_tint {
                        crate::rune_sec::app_theme::secondary_accent_on_tint(t)
                    } else {
                        crate::rune_sec::app_theme::secondary_accent(t)
                    },
                );
            }
            Some(text("◆").font(type_::LABEL).color(if on_tint {
                crate::rune_sec::app_theme::accent_on_tint(t)
            } else {
                design::accent()
            }))
        } else if state_key {
            Some(
                text("◇")
                    .font(type_::LABEL)
                    .color(if on_tint {
                        crate::rune_sec::app_theme::secondary_accent_on_tint(t)
                    } else {
                        crate::rune_sec::app_theme::secondary_accent(t)
                    }),
            )
        } else {
            None
        };
        let dot = if channel_key {
            Some(text("●").font(type_::LABEL).color(t.text_secondary))
        } else {
            None
        };
        let glyphs = match (primary, dot) {
            (Some(glyph), Some(dot)) => {
                Some(row().gap(2.0).center_y().child(glyph).child(dot))
            }
            (Some(glyph), None) => Some(glyph),
            (None, Some(dot)) => Some(dot),
            (None, None) => None,
        };
        // Exposure (RM14 — P-2): a cell inside a HOLD
        // span carries the span's bar at its top — the
        // accent at half strength on the span's key
        // cell, the fainter secondary tone on the held
        // cells after it. The bar and the bend bar
        // compose in `strip_cell`; a cell with neither
        // keeps its historical single-child tree.
        let bend = is_bend_key(&r.bend_keys, frame);
        let strip = r
            .spans
            .iter()
            .find(|sp| sp.is_hold() && sp.covers(frame))
            .map(|_| {
                if is_key(&r.keys, frame) {
                    design::accent().alpha(0.55)
                } else {
                    t.text_secondary.alpha(0.40)
                }
            });
        cell = match (strip, bend) {
            (None, false) => match glyphs {
                Some(glyph) => cell.child(glyph),
                None => cell,
            },
            _ => cell.child(strip_cell(glyphs, strip, bend, chip_w, t)),
        };
        // Key selection (F-4): a cell whose channel-key
        // event IS the shared selection wears the accent
        // border. Applied after the glyph composition,
        // so it supersedes the pose+state coincidence
        // border when both would draw (see the module
        // docs for the trade).
        if let Some((ci, kf)) = sel_event {
            if ksel.contains(ci, &core.project.channels[ci], kf) {
                cell = cell.border(1.0, design::accent());
            }
        }
        cells = cells.child(cell);
    }

    if r.selected {
        cells.fill(design::selected_fill()).radius(design::ROW_RADIUS)
    } else {
        cells
    }
}

/// The content of a cell carrying a strip: the
/// exposure bar at the top when the cell sits in a
/// hold span (`strip` is its colour — Roadmap Wave 1,
/// RM14), the bend bar at the bottom when the frame
/// carries a stored bend change (Wave AB, AB3 — this
/// fn is that wave's `bend_cell`, generalised), and
/// the cell's glyph content — possibly none — centred
/// in the height between them. The stack is exactly
/// the cell's own size, so neither bar can push a
/// glyph out of the cell, and the cell's border (the
/// state coincidence, the selection) and its fills
/// draw around the stack unchanged. A bend change
/// alone shows its bar alone in an otherwise empty
/// cell; a held frame with no glyph shows the
/// exposure bar alone.
fn strip_cell(glyphs: Option<El>, strip: Option<Color>, bend: bool, chip_w: f32, t: &Theme) -> El {
    let top_h = if strip.is_some() { EXPO_STRIP_H } else { 0.0 };
    let bot_h = if bend { BEND_STRIP_H } else { 0.0 };
    let area = el().size(chip_w, DOPE_ROW_H - top_h - bot_h).center();
    let area = match glyphs {
        Some(g) => area.child(g),
        None => area,
    };
    let mut stack = col().size(chip_w, DOPE_ROW_H);
    if let Some(color) = strip {
        stack = stack.child(
            el()
                .size(chip_w, EXPO_STRIP_H)
                .child(
                    el()
                        .size(chip_w, EXPO_STRIP_H)
                        .radius(EXPO_STRIP_H / 2.0)
                        .fill(color),
                ),
        );
    }
    stack = stack.child(area);
    if bend {
        stack = stack.child(
            el()
                .size(chip_w, BEND_STRIP_H)
                .center()
                .child(
                    el()
                        .size(BEND_STRIP_W.min(chip_w), BEND_STRIP_H)
                        .radius(BEND_STRIP_H / 2.0)
                        .fill(t.text),
                ),
        );
    }
    stack
}

/// One sprite instance's row (Wave X, X5): the header
/// cell (a square accent swatch — the quad's shape,
/// where a figure's round dot is its joint — plus the
/// fitted asset name; press selects the instance in
/// the Sprites panel, a panel-state write only) then
/// one cell per frame at the chips' pitch (press
/// selects the instance AND goes to the frame — the
/// figure cells' navigation, both real writes, neither
/// snapshotted). A diamond marks each frame holding a
/// stored key; the panel-selected instance's row
/// carries the selected tint, the current frame's cell
/// carries it in every row — the figure rows' two
/// bands, unchanged.
fn sprite_row(r: &SpriteDopeRow, core: &EditorCore, chip_w: f32, t: &Theme) -> El {
    let i = r.index;
    let header = row()
        .key(format!("dope-sprite-row-{i}"))
        .size(DOPE_HEADER_W, DOPE_ROW_H)
        .px(6.0)
        .gap(6.0)
        .center_y()
        .radius(design::ROW_RADIUS)
        .hover(t.hover)
        .press(t.press)
        .on_press(move |s: &mut State| s.sprites_panel.selected = Some(i))
        .child(el().size(10.0, 10.0).radius(2.5).fill(if r.selected {
            // RM25 (PL-1): the header swatch sits on
            // the row's selected tint when the row is
            // selected — the on-tint variant, as the
            // cells' diamonds take.
            crate::rune_sec::app_theme::accent_on_tint(t)
        } else {
            design::accent()
        }))
        .child(
            text(fit_dope_name(&r.name))
                .font(type_::LABEL)
                .truncate()
                .color(t.text),
        );

    let mut cells = row().gap(CHIP_GAP).child(header);
    for frame in 0..core.frame_count() {
        let mut cell = el()
            .key(format!("dope-sprite-cell-{i}-{frame}"))
            .size(chip_w, DOPE_ROW_H)
            .radius(design::CONTROL_RADIUS)
            .center()
            .hover(t.control_track)
            .press(t.press)
            .on_press(move |s: &mut State| {
                s.sprites_panel.selected = Some(i);
                s.core.goto(frame);
            });
        if frame == core.current {
            cell = cell.fill(design::selected_fill());
        }
        if is_key(&r.keys, frame) {
            // RM25 (PL-1): the on-tint variant on the
            // current frame's cell or the selected
            // instance's row, as the figure cells take.
            let glyph = if frame == core.current || r.selected {
                crate::rune_sec::app_theme::accent_on_tint(t)
            } else {
                design::accent()
            };
            cell = cell.child(text("◆").font(type_::LABEL).color(glyph));
        }
        cells = cells.child(cell);
    }

    if r.selected {
        cells.fill(design::selected_fill()).radius(design::ROW_RADIUS)
    } else {
        cells
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stykanimate_rs::graph::{Channel, ChannelTarget, Key};
    use stykanimate_rs::model::{
        FigureDef, FigureInstance, Frame, ImageAsset, InstanceKey, Pose, SpriteInstance,
        StateKey,
    };

    fn channel(figure: usize, target: ChannelTarget, frames: &[f32]) -> Channel {
        Channel {
            figure,
            target,
            keys: frames.iter().map(|&f| Key::flat(f, 0.0)).collect(),
        }
    }

    fn figure(name: &str) -> FigureInstance {
        FigureInstance {
            name: name.into(),
            color: 0xFFFFFF,
            def: FigureDef::humanoid(),
            hidden: false,
            clip: None,
            states: Vec::new(),
            locked: false,
            opacity: 1.0,
        }
    }

    fn pose(root_x: f32, angle: f32) -> Pose {
        Pose {
            root: [root_x, 200.0],
            angles: vec![angle, 0.0, 0.0],
            sx: 1.0,
            sy: 1.0,
        }
    }

    fn frame(poses: Vec<Pose>) -> Frame {
        Frame {
            poses,
            duration: 1.0,
            easing: None,
            bends: Vec::new(),
        }
    }

    /// Alpha swings an angle on frame 2 and holds;
    /// Beta appears on frame 1 and moves its root on
    /// frame 3 — the stats suite's choreography, whose
    /// keyframes are Alpha [0, 2], Beta [1, 3].
    fn two_figure_state() -> State {
        let alpha = [
            pose(100.0, 10.0),
            pose(100.3, 10.004),
            pose(100.3, 25.0),
            pose(100.3, 25.0),
        ];
        let beta = pose(300.0, 0.0);
        let frames = vec![
            frame(vec![alpha[0].clone()]),
            frame(vec![alpha[1].clone(), beta.clone()]),
            frame(vec![alpha[2].clone(), beta.clone()]),
            frame(vec![alpha[3].clone(), pose(310.0, 0.0)]),
        ];
        let mut s = crate::init();
        s.core.project.figures = vec![figure("Alpha"), figure("Beta")];
        s.core.project.frames = frames;
        s
    }

    fn many_figure_state(count: usize) -> State {
        let mut s = crate::init();
        s.core.project.figures = (0..count)
            .map(|i| figure(&format!("Figure {}", i + 1)))
            .collect();
        s.core.project.frames = vec![frame(
            (0..count).map(|i| pose(100.0 + 10.0 * i as f32, 0.0)).collect(),
        )];
        s
    }

    #[test]
    fn keys_follow_the_dope_scheme() {
        assert_eq!(row_key(3), "dope-row-3");
        assert_eq!(cell_key(2, 5), "dope-cell-2-5");
        assert_eq!(cell_key(0, 0), "dope-cell-0-0");
    }

    #[test]
    fn row_order_is_natural_up_to_the_cap() {
        assert_eq!(dope_row_order(0, 0), Vec::<usize>::new());
        assert_eq!(dope_row_order(3, 1), vec![0, 1, 2]);
        assert_eq!(dope_row_order(4, 0), vec![0, 1, 2, 3]);
        // At the cap exactly, no pinning reshuffles the
        // project order even for a late selection.
        assert_eq!(dope_row_order(4, 3), vec![0, 1, 2, 3]);
    }

    #[test]
    fn row_order_pins_the_selected_figure_first_past_the_cap() {
        assert_eq!(dope_row_order(6, 4), vec![4, 0, 1, 2]);
        assert_eq!(dope_row_order(6, 5), vec![5, 0, 1, 2]);
        // The selected figure already first stays a
        // plain project-order prefix.
        assert_eq!(dope_row_order(6, 0), vec![0, 1, 2, 3]);
        // A stale selection pins nothing.
        assert_eq!(dope_row_order(6, 999), vec![0, 1, 2, 3]);
    }

    #[test]
    fn fit_dope_name_keeps_names_that_fit() {
        assert_eq!(fit_dope_name("Neo"), "Neo");
        assert_eq!(fit_dope_name(""), "");
        assert_eq!(fit_dope_name("Figure 1"), "Figure 1");
    }

    #[test]
    fn fit_dope_name_truncates_long_names_to_the_budget() {
        let fitted = fit_dope_name("Alexander The Great Stickman");
        assert!(fitted.ends_with('…'), "an ellipsis marks the cut: {fitted}");
        assert!(
            preset_label_width(&fitted) <= DOPE_NAME_BUDGET,
            "the fitted name measures {} pt against the {} pt budget",
            preset_label_width(&fitted),
            DOPE_NAME_BUDGET
        );
        // The word-boundary rule: the kept prefix the
        // width model affords is "Alexander T" (75 pt of
        // the 80 pt room), whose last space sits in its
        // second half, so the cut lands there, not
        // mid-word.
        assert_eq!(fitted, "Alexander…");
    }

    #[test]
    fn fit_dope_name_never_splits_a_multibyte_character() {
        let name = "スティックマン・フィギュア";
        let fitted = fit_dope_name(name);
        assert!(fitted.ends_with('…'));
        assert!(preset_label_width(&fitted) <= DOPE_NAME_BUDGET);
        // The kept part is a true prefix of the name.
        assert!(name.starts_with(fitted.trim_end_matches('…')));
    }

    #[test]
    fn dope_rows_map_keyframes_per_figure() {
        let mut s = two_figure_state();
        s.core.selected_figure = 1;
        let rows = dope_rows(&s.core);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].fi, 0);
        assert_eq!(rows[0].name, "Alpha");
        assert_eq!(rows[0].keys, vec![0, 2]);
        assert!(!rows[0].selected);
        assert_eq!(rows[1].fi, 1);
        assert_eq!(rows[1].keys, vec![1, 3]);
        assert!(rows[1].selected, "the selection follows core.selected_figure");
        assert!(is_key(&rows[0].keys, 2));
        assert!(!is_key(&rows[0].keys, 1));
        assert!(any_keyframes(&s.core));
    }

    #[test]
    fn dope_rows_cap_and_pin_on_a_crowded_project() {
        let mut s = many_figure_state(6);
        s.core.selected_figure = 5;
        let rows = dope_rows(&s.core);
        assert_eq!(rows.len(), DOPE_MAX_ROWS);
        assert_eq!(rows[0].fi, 5, "the selected row is pinned first");
        assert!(rows[0].selected);
        let shown: Vec<usize> = rows.iter().map(|r| r.fi).collect();
        assert_eq!(shown, vec![5, 0, 1, 2]);
    }

    #[test]
    fn no_keyframes_anywhere_reads_as_such() {
        // Figures exist but no frame holds a pose for
        // any of them — no keys can exist.
        let mut s = crate::init();
        s.core.project.figures = vec![figure("Alpha"), figure("Beta")];
        s.core.project.frames = vec![frame(vec![]), frame(vec![])];
        assert!(!any_keyframes(&s.core));
        let rows = dope_rows(&s.core);
        assert!(rows.iter().all(|r| r.keys.is_empty()));
    }

    #[test]
    fn state_key_frames_sorts_and_dedupes() {
        // Wave Y (Y11): the display derivation over a
        // hand-edited (unsorted, frame-repeating) states
        // vec — the stored order is canonical, the mark
        // is per-frame.
        let mut fig = figure("Alpha");
        assert!(state_key_frames(&fig).is_empty());
        fig.states = vec![
            StateKey { frame: 5, clip: 0, offset: 0, speed: 1.0 },
            StateKey { frame: 2, clip: 0, offset: 0, speed: 1.0 },
            StateKey { frame: 5, clip: 1, offset: 0, speed: 1.0 },
        ];
        assert_eq!(state_key_frames(&fig), vec![2, 5]);
        assert!(is_state_key(&state_key_frames(&fig), 5));
        assert!(!is_state_key(&state_key_frames(&fig), 3));
    }

    #[test]
    fn dope_rows_carry_state_key_frames() {
        let mut s = two_figure_state();
        s.core.project.figures[0].states = vec![
            StateKey { frame: 3, clip: 0, offset: 0, speed: 1.0 },
            StateKey { frame: 1, clip: 0, offset: 2, speed: 0.5 },
        ];
        let rows = dope_rows(&s.core);
        assert_eq!(rows[0].state_keys, vec![1, 3]);
        assert!(rows[1].state_keys.is_empty());
        assert!(any_state_keys(&s.core));
    }

    #[test]
    fn state_keys_alone_speak_for_the_lane() {
        // Figures with no poses on any frame: no pose
        // keyframes can exist, yet a state key still
        // brings the figure rows up — the lane's gating
        // hears the state channel, so the "No keyframes
        // yet" empty state cannot deny the key (Wave Y,
        // Y11). Construction is the assertable level
        // here (the module's discipline).
        let mut s = crate::init();
        let mut fig = figure("Alpha");
        fig.states = vec![StateKey { frame: 1, clip: 0, offset: 0, speed: 1.0 }];
        s.core.project.figures = vec![fig];
        s.core.project.frames = vec![frame(vec![]), frame(vec![])];
        assert!(!any_keyframes(&s.core));
        assert!(any_state_keys(&s.core));
        let t = rune::tok::theme(true);
        let _ = dopesheet_lane(&s, &t);
    }

    #[test]
    fn channel_key_event_names_the_first_channel_holding_the_frame() {
        // F-4: two channels key the same frame — the
        // event names the FIRST in project order, and
        // carries the key's own (fractional) frame.
        let mut s = crate::init();
        s.core.project.figures = vec![figure("Alpha"), figure("Beta")];
        s.core.project.frames = (0..4).map(|_| frame(vec![])).collect();
        s.core.project.channels = vec![
            channel(0, ChannelTarget::RootY, &[1.0]),
            channel(0, ChannelTarget::RootX, &[2.4, 1.0]),
            channel(1, ChannelTarget::RootX, &[1.0]),
        ];
        assert_eq!(channel_key_event(&s.core.project, 0, 1, 4), Some((0, 1.0)));
        assert_eq!(
            channel_key_event(&s.core.project, 0, 2, 4),
            Some((1, 2.4)),
            "a fractional key names its rounding cell with its own frame"
        );
        assert_eq!(channel_key_event(&s.core.project, 1, 1, 4), Some((2, 1.0)));
        assert_eq!(channel_key_event(&s.core.project, 0, 3, 4), None);
        assert_eq!(channel_key_event(&s.core.project, 0, 0, 4), None);
    }

    #[test]
    fn channel_key_event_ignores_out_of_timeline_and_keyless() {
        let mut s = crate::init();
        s.core.project.figures = vec![figure("Alpha")];
        s.core.project.frames = (0..2).map(|_| frame(vec![])).collect();
        s.core.project.channels = vec![
            channel(0, ChannelTarget::Angle(0), &[9.2]),
            channel(0, ChannelTarget::Bend(0), &[]),
        ];
        assert_eq!(channel_key_event(&s.core.project, 0, 0, 2), None);
        assert_eq!(channel_key_event(&s.core.project, 0, 1, 2), None);
        assert_eq!(channel_key_event(&s.core.project, 0, 0, 0), None);
    }

    #[test]
    fn channel_key_frames_covers_every_target_at_parity() {
        // Wave AA (AA3): one key per target kind on
        // figure 0 — root X, root Y, angle, length,
        // bend — each marks its frame; the bend key
        // doubling the angle/length frames still marks
        // each frame once.
        let mut s = crate::init();
        s.core.project.figures = vec![figure("Alpha"), figure("Beta")];
        s.core.project.frames = (0..4).map(|_| frame(vec![])).collect();
        s.core.project.channels = vec![
            channel(0, ChannelTarget::RootX, &[0.0]),
            channel(0, ChannelTarget::RootY, &[1.0]),
            channel(0, ChannelTarget::Angle(1), &[2.0]),
            channel(0, ChannelTarget::Length(1), &[3.0]),
            channel(0, ChannelTarget::Bend(1), &[2.0, 3.0]),
            channel(1, ChannelTarget::Bend(0), &[0.0]),
        ];
        assert_eq!(channel_key_frames(&s.core.project, 0), vec![0, 1, 2, 3]);
        assert_eq!(channel_key_frames(&s.core.project, 1), vec![0]);
        assert!(is_channel_key(&channel_key_frames(&s.core.project, 0), 3));
        assert!(!is_channel_key(&channel_key_frames(&s.core.project, 1), 3));
    }

    #[test]
    fn channel_key_frames_rounds_like_the_marker_row() {
        // The imported `graph_key_frame` mapping:
        // nearest frame, halves away from zero; a key
        // rounding outside the timeline (or a
        // non-finite one) marks nothing rather than
        // piling onto an end frame.
        let mut s = crate::init();
        s.core.project.figures = vec![figure("Alpha")];
        s.core.project.frames = (0..4).map(|_| frame(vec![])).collect();
        s.core.project.channels = vec![channel(
            0,
            ChannelTarget::Angle(0),
            &[0.4, 2.5, 9.2, -1.0, f32::NAN],
        )];
        assert_eq!(channel_key_frames(&s.core.project, 0), vec![0, 3]);
        // A zero-frame project has no cell to mark.
        s.core.project.frames = Vec::new();
        assert!(channel_key_frames(&s.core.project, 0).is_empty());
    }

    #[test]
    fn stale_channels_and_keyless_channels_mark_nothing() {
        // A channel pointing past the figure table is
        // the graph module's standing ignore case: it
        // opens no row and does not speak for the lane.
        // A channel with no keys is equally silent.
        let mut s = crate::init();
        s.core.project.figures = vec![figure("Alpha")];
        s.core.project.frames = (0..2).map(|_| frame(vec![])).collect();
        s.core.project.channels = vec![
            channel(9, ChannelTarget::Bend(1), &[1.0]),
            channel(0, ChannelTarget::Angle(0), &[]),
        ];
        assert!(channel_key_frames(&s.core.project, 0).is_empty());
        assert!(!any_channel_keys(&s.core));
    }

    #[test]
    fn dope_rows_carry_channel_key_frames() {
        let mut s = two_figure_state();
        s.core.project.channels = vec![
            channel(0, ChannelTarget::Bend(1), &[2.0, 3.0]),
            channel(1, ChannelTarget::RootX, &[1.0]),
        ];
        let rows = dope_rows(&s.core);
        assert_eq!(rows[0].channel_keys, vec![2, 3]);
        assert_eq!(rows[1].channel_keys, vec![1]);
        assert!(any_channel_keys(&s.core));
    }

    #[test]
    fn channel_keys_alone_speak_for_the_lane() {
        // A figure with no poses and no state keys,
        // animated only by a bend channel: the lane's
        // gating hears the graph channel, so the "No
        // keyframes yet" empty state cannot deny the
        // key (Wave AA, AA3 — Y11's state-key rule, one
        // channel over). Construction is the assertable
        // level here (the module's discipline).
        let mut s = crate::init();
        s.core.project.figures = vec![figure("Alpha")];
        s.core.project.frames = vec![frame(vec![]), frame(vec![])];
        s.core.project.channels = vec![channel(0, ChannelTarget::Bend(2), &[1.0])];
        assert!(!any_keyframes(&s.core));
        assert!(!any_state_keys(&s.core));
        assert!(any_channel_keys(&s.core));
        for dark in [true, false] {
            let t = rune::tok::theme(dark);
            let _ = dopesheet_lane(&s, &t);
        }
    }

    #[test]
    fn lane_constructs_with_channel_coincidences() {
        // Every cell composition the AA3 marks can
        // make, across four frames of one figure:
        // frame 0 pose+channel, frame 1 channel alone,
        // frame 2 pose+state+channel, frame 3
        // state+channel. Construction is the assertable
        // level (the module's discipline); the
        // derivations under the compositions are pinned
        // exactly.
        let mut s = crate::init();
        let mut fig = figure("Alpha");
        fig.states = vec![
            StateKey { frame: 2, clip: 0, offset: 0, speed: 1.0 },
            StateKey { frame: 3, clip: 0, offset: 0, speed: 1.0 },
        ];
        s.core.project.figures = vec![fig];
        let a = pose(100.0, 10.0);
        let b = pose(100.0, 25.0);
        s.core.project.frames = vec![
            frame(vec![a.clone()]),
            frame(vec![a.clone()]),
            frame(vec![b.clone()]),
            frame(vec![b]),
        ];
        s.core.project.channels = vec![
            channel(0, ChannelTarget::Angle(0), &[0.0]),
            channel(0, ChannelTarget::Length(1), &[1.0]),
            channel(0, ChannelTarget::Bend(1), &[2.0]),
            channel(0, ChannelTarget::RootX, &[3.0]),
        ];
        let rows = dope_rows(&s.core);
        assert_eq!(rows[0].keys, vec![0, 2]);
        assert_eq!(rows[0].state_keys, vec![2, 3]);
        assert_eq!(rows[0].channel_keys, vec![0, 1, 2, 3]);
        for dark in [true, false] {
            let t = rune::tok::theme(dark);
            let _ = dopesheet_lane(&s, &t);
        }
    }

    fn bend_frame(poses: Vec<Pose>, bends: Vec<Vec<f32>>) -> Frame {
        let mut f = frame(poses);
        f.bends = bends;
        f
    }

    #[test]
    fn bend_key_frames_marks_stored_bend_changes() {
        // Wave AB (AB3): the change rule over stored
        // rows — a bend appearing marks, holding the
        // same bend does not, changing it marks again,
        // and its disappearance marks nothing (the
        // pose rule's asymmetry).
        let mut s = crate::init();
        s.core.project.figures = vec![figure("Alpha")];
        let bent = vec![vec![0.0, 0.35, 0.0]];
        let bent_more = vec![vec![0.0, 0.7, 0.0]];
        s.core.project.frames = vec![
            frame(vec![]),
            frame(vec![]),
            bend_frame(vec![], bent.clone()),
            bend_frame(vec![], bent),
            bend_frame(vec![], bent_more),
            frame(vec![]),
        ];
        assert_eq!(bend_key_frames(&s.core.project, 0), vec![2, 4]);
        assert!(is_bend_key(&bend_key_frames(&s.core.project, 0), 4));
        assert!(!is_bend_key(&bend_key_frames(&s.core.project, 0), 3));
        // An out-of-range figure has no row to change.
        assert!(bend_key_frames(&s.core.project, 7).is_empty());
    }

    #[test]
    fn bend_key_frames_frame_zero_epsilon_and_corrupt_rows() {
        let mut s = crate::init();
        s.core.project.figures = vec![figure("Alpha")];
        // A bend on frame 0 marks, against the implicit
        // all-zero row before it.
        s.core.project.frames = vec![bend_frame(vec![], vec![vec![0.0, 0.35, 0.0]])];
        assert_eq!(bend_key_frames(&s.core.project, 0), vec![0]);
        // All-zero rows, however they are stored, mark
        // nothing.
        s.core.project.frames = vec![
            bend_frame(vec![], vec![vec![0.0, 0.0, 0.0]]),
            bend_frame(vec![], vec![Vec::new()]),
        ];
        assert!(bend_key_frames(&s.core.project, 0).is_empty());
        // A sub-epsilon wiggle is dust, not a change —
        // and a sub-epsilon bend alone is not a bend.
        s.core.project.frames = vec![
            bend_frame(vec![], vec![vec![0.0, 0.35, 0.0]]),
            bend_frame(vec![], vec![vec![0.0, 0.355, 0.0]]),
            bend_frame(vec![], vec![vec![0.0, 0.005, 0.0]]),
        ];
        assert_eq!(bend_key_frames(&s.core.project, 0), vec![0]);
        // A non-finite component reads as zero: no
        // phantom mark at its own frame, and no phantom
        // change against the row it corrupts.
        s.core.project.frames = vec![
            bend_frame(vec![], vec![vec![0.0, f32::NAN, 0.0]]),
            bend_frame(vec![], vec![vec![0.0, 0.35, 0.0]]),
            bend_frame(vec![], vec![vec![0.0, 0.35, f32::INFINITY]]),
        ];
        assert_eq!(bend_key_frames(&s.core.project, 0), vec![1]);
    }

    #[test]
    fn bend_key_frames_normalises_missing_short_and_long_rows() {
        // The `bends_at` normalisation, mirrored: a
        // missing figure entry and a short row pad with
        // zeros; a long row's surplus past the node
        // count is not the figure's row and changes
        // nothing.
        let mut s = crate::init();
        let alpha = figure("Alpha");
        let n = alpha.def.nodes.len();
        s.core.project.figures = vec![alpha, figure("Beta")];
        let mut long_row = vec![0.0; n];
        long_row.push(0.9);
        s.core.project.frames = vec![
            frame(vec![]),
            bend_frame(vec![], vec![vec![0.0, 0.35, 0.0]]),
            bend_frame(vec![], vec![Vec::new(), vec![0.35]]),
            bend_frame(vec![], vec![long_row]),
        ];
        assert_eq!(bend_key_frames(&s.core.project, 0), vec![1]);
        assert_eq!(bend_key_frames(&s.core.project, 1), vec![2]);
    }

    #[test]
    fn dope_rows_carry_bend_key_frames() {
        let mut s = two_figure_state();
        s.core.project.frames[2].bends = vec![vec![0.0, 0.35, 0.0]];
        s.core.project.frames[3].bends = vec![vec![0.0, 0.35, 0.0]];
        let rows = dope_rows(&s.core);
        // The row arrives on frame 2 and is held,
        // unchanged, on frame 3: one mark.
        assert_eq!(rows[0].bend_keys, vec![2]);
        assert!(rows[1].bend_keys.is_empty());
        assert!(any_bend_keys(&s.core));
    }

    #[test]
    fn bend_keys_alone_speak_for_the_lane() {
        // A figure with no poses, no state keys and no
        // channels, bent only from the pose tools: the
        // lane's gating hears the bend table, so the
        // "No keyframes yet" empty state cannot deny
        // the bend (Wave AB, AB3 — Y11's state-key rule,
        // one table over). Construction is the
        // assertable level here (the module's
        // discipline).
        let mut s = crate::init();
        s.core.project.figures = vec![figure("Alpha")];
        s.core.project.frames = vec![
            frame(vec![]),
            bend_frame(vec![], vec![vec![0.0, 0.35, 0.0]]),
        ];
        assert!(!any_keyframes(&s.core));
        assert!(!any_state_keys(&s.core));
        assert!(!any_channel_keys(&s.core));
        assert!(any_bend_keys(&s.core));
        for dark in [true, false] {
            let t = rune::tok::theme(dark);
            let _ = dopesheet_lane(&s, &t);
        }
    }

    #[test]
    fn lane_constructs_with_bend_coincidences() {
        // Every composition the bend bar can meet,
        // across four frames of one figure: frame 0
        // pose+bend, frame 1 bend alone, frame 2
        // pose+state+channel+bend (the full stack —
        // border, both glyphs and the bar in one cell),
        // frame 3 state+bend. Construction is the
        // assertable level (the module's discipline);
        // the derivations under the compositions are
        // pinned exactly.
        let mut s = crate::init();
        let mut fig = figure("Alpha");
        fig.states = vec![
            StateKey { frame: 2, clip: 0, offset: 0, speed: 1.0 },
            StateKey { frame: 3, clip: 0, offset: 0, speed: 1.0 },
        ];
        s.core.project.figures = vec![fig];
        let a = pose(100.0, 10.0);
        let b = pose(100.0, 25.0);
        s.core.project.frames = vec![
            bend_frame(vec![a.clone()], vec![vec![0.0, 0.35, 0.0]]),
            bend_frame(vec![a.clone()], vec![vec![0.0, 0.0, 0.35]]),
            bend_frame(vec![b.clone()], vec![vec![0.0, 0.7, 0.0]]),
            bend_frame(vec![b], vec![vec![0.0, -0.35, 0.0]]),
        ];
        s.core.project.channels = vec![channel(0, ChannelTarget::Bend(1), &[2.0])];
        let rows = dope_rows(&s.core);
        assert_eq!(rows[0].keys, vec![0, 2]);
        assert_eq!(rows[0].state_keys, vec![2, 3]);
        assert_eq!(rows[0].channel_keys, vec![2]);
        assert_eq!(rows[0].bend_keys, vec![0, 1, 2, 3]);
        for dark in [true, false] {
            let t = rune::tok::theme(dark);
            let _ = dopesheet_lane(&s, &t);
        }
    }

    #[test]
    fn frame_readout_is_one_based_and_clamped() {
        assert_eq!(frame_readout(0, 5), "Frame 1 / 5");
        assert_eq!(frame_readout(4, 5), "Frame 5 / 5");
        assert_eq!(frame_readout(9, 5), "Frame 5 / 5");
        assert_eq!(frame_readout(0, 0), "Frame 0 / 0");
    }

    #[test]
    fn dope_row_height_meets_the_target_floor() {
        // W2-55 (A20-14): the figure/sprite row height
        // is the WCAG 2.5.8 target-size floor — 24 pt,
        // not the 20 pt S18 measured. Header cells and
        // frame cells both size from DOPE_ROW_H (see
        // `figure_row` / `sprite_row`), so this
        // constant IS the press height; and the strip
        // cells' glyph area (row minus the two strips)
        // stays positive at the floor.
        assert_eq!(DOPE_ROW_H, 24.0);
        assert!(DOPE_ROW_H - EXPO_STRIP_H - BEND_STRIP_H > 0.0);
    }

    #[test]
    fn lane_width_matches_the_row_geometry() {
        assert_eq!(lane_width(0, CHIP_W), DOPE_HEADER_W);
        assert_eq!(lane_width(1, CHIP_W), DOPE_HEADER_W + CHIP_GAP + CHIP_W);
        assert_eq!(
            lane_width(5, CHIP_W),
            DOPE_HEADER_W + CHIP_GAP + 5.0 * CHIP_W + 4.0 * CHIP_GAP
        );
        // At the narrowest zoom the row is the header
        // plus five 8 pt cells and their gaps.
        assert_eq!(
            lane_width(5, 8.0),
            DOPE_HEADER_W + CHIP_GAP + 5.0 * 8.0 + 4.0 * CHIP_GAP
        );
    }

    #[test]
    fn more_label_counts_honestly() {
        assert_eq!(more_label(1), "+1 more figure");
        assert_eq!(more_label(3), "+3 more figures");
    }

    fn skey(frame: f32) -> InstanceKey {
        InstanceKey {
            frame,
            x: 100.0,
            y: 100.0,
            scale: 1.0,
            angle: 0.0,
            opacity: 1.0,
            flip_x: false,
            flip_y: false,
            scale_y: 0.0,
            pivot_x: 0.5,
            pivot_y: 0.5,
        }
    }

    fn sprite(asset: usize, key_frames: &[f32]) -> SpriteInstance {
        SpriteInstance {
            asset,
            keys: key_frames.iter().map(|&f| skey(f)).collect(),
        }
    }

    /// A state whose project carries `assets` named
    /// assets and `sprites` instances over the demo
    /// frames, figures untouched (one demo figure).
    fn sprite_state(assets: &[&str], sprites: Vec<SpriteInstance>) -> State {
        let mut s = crate::init();
        s.core.project.assets = assets
            .iter()
            .map(|name| ImageAsset {
                name: name.to_string(),
                png: Vec::new(),
            })
            .collect();
        s.core.project.sprites = sprites;
        s
    }

    #[test]
    fn sprite_key_frames_takes_whole_frames_only() {
        let inst = sprite(0, &[0.0, 2.0, 2.0, 4.0]);
        assert_eq!(sprite_key_frames(&inst), vec![0, 2, 4], "sorted + deduplicated");
        // A fractional imported key names no whole-frame
        // cell and is skipped, never rounded onto one.
        let inst = sprite(0, &[1.5, 2.0]);
        assert_eq!(sprite_key_frames(&inst), vec![2]);
        // A negative frame is not a cell either.
        let inst = sprite(0, &[-1.0, 0.0]);
        assert_eq!(sprite_key_frames(&inst), vec![0]);
        assert!(sprite_key_frames(&sprite(0, &[])).is_empty());
    }

    #[test]
    fn sprite_dope_rows_carry_name_keys_and_selection() {
        let s = sprite_state(
            &["hero.png", "tree.png"],
            vec![sprite(1, &[0.0, 3.0]), sprite(0, &[2.0]), sprite(9, &[])],
        );
        let rows = sprite_dope_rows(&s.core, Some(1));
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].index, 0);
        assert_eq!(rows[0].name, "tree.png");
        assert_eq!(rows[0].keys, vec![0, 3]);
        assert!(!rows[0].selected);
        assert_eq!(rows[1].index, 1);
        assert_eq!(rows[1].name, "hero.png");
        assert!(rows[1].selected, "the panel selection marks its row");
        // A dangling asset reads through the panel's own
        // missing-asset fallback.
        assert_eq!(rows[2].name, "asset 10 (missing)");
        assert!(rows[2].keys.is_empty());
    }

    #[test]
    fn sprite_dope_rows_cap_and_pin_the_panel_selection() {
        let sprites: Vec<SpriteInstance> = (0..6).map(|_| sprite(0, &[0.0])).collect();
        let s = sprite_state(&["a.png"], sprites);
        let rows = sprite_dope_rows(&s.core, Some(5));
        let shown: Vec<usize> = rows.iter().map(|r| r.index).collect();
        assert_eq!(shown, vec![5, 0, 1, 2], "the figure rows' pin rule, reused");
        assert!(rows[0].selected);
        // No selection (or a stale one) pins nothing.
        let rows = sprite_dope_rows(&s.core, None);
        let shown: Vec<usize> = rows.iter().map(|r| r.index).collect();
        assert_eq!(shown, vec![0, 1, 2, 3]);
        assert!(rows.iter().all(|r| !r.selected));
        let rows = sprite_dope_rows(&s.core, Some(99));
        let shown: Vec<usize> = rows.iter().map(|r| r.index).collect();
        assert_eq!(shown, vec![0, 1, 2, 3]);
    }

    #[test]
    fn sprite_enrichment_line_reads_the_selected_sprite_at_the_playhead() {
        // Wave Y (Y14): the lane's readout of the
        // panel-selected sprite's sampled key.
        let mut s = sprite_state(&["a.png"], vec![sprite(0, &[0.0, 4.0])]);
        s.core.project.sprites[0].keys[1].opacity = 0.5;
        s.core.project.sprites[0].keys[1].flip_x = true;
        s.core.current = 4;
        assert_eq!(
            sprite_enrichment_line(&s.core, Some(0)).as_deref(),
            Some(
                "Sprite 1 · frame 5 · Opacity 50% · Flip X · \
                 Scale Y ×1.00 (follows Scale) · Pivot 0.50, 0.50"
            ),
        );
        // Between keys the line reads the SAMPLE (what
        // the stage draws), never a stored key: the
        // opacity lerps, the flip is still the start
        // key's.
        s.core.current = 2;
        let line = sprite_enrichment_line(&s.core, Some(0)).expect("a sampled line");
        assert!(line.contains("Opacity 75%"), "the lerp of 1.0 and 0.5: {line}");
        assert!(line.contains("Flip none"), "the start key's flip: {line}");
        // No selection, a stale selection, and a
        // keyless sprite each earn no line.
        assert!(sprite_enrichment_line(&s.core, None).is_none());
        assert!(sprite_enrichment_line(&s.core, Some(9)).is_none());
        let keyless = sprite_state(&["a.png"], vec![sprite(0, &[])]);
        assert!(sprite_enrichment_line(&keyless.core, Some(0)).is_none());
    }

    #[test]
    fn more_sprites_label_counts_honestly() {
        assert_eq!(more_sprites_label(1), "+1 more sprite");
        assert_eq!(more_sprites_label(4), "+4 more sprites");
    }

    #[test]
    fn lane_constructs_with_sprite_sections() {
        // Construction is the assertable level (the
        // figure lane's own discipline); what is pinned
        // here is that every new branch builds.
        for dark in [true, false] {
            let t = rune::tok::theme(dark);
            // Sprites but NO figures at all: the sprite
            // section renders instead of the no-figures
            // empty state.
            let mut s = sprite_state(&["a.png"], vec![sprite(0, &[0.0, 2.0])]);
            s.core.project.figures = Vec::new();
            let _ = dopesheet_lane(&s, &t);
            // Figures with no keyframes anywhere, plus
            // sprites: the sprite section renders instead
            // of the no-keys empty state.
            let mut s = sprite_state(&["a.png"], vec![sprite(0, &[1.0])]);
            s.core.project.frames = vec![frame(vec![])];
            let _ = dopesheet_lane(&s, &t);
            // Both sections populated, sprites past the
            // cap with the panel selection pinned.
            let mut s = two_figure_state();
            s.core.project.assets = vec![ImageAsset {
                name: "a.png".into(),
                png: Vec::new(),
            }];
            s.core.project.sprites = (0..6).map(|_| sprite(0, &[0.0])).collect();
            s.sprites_panel.selected = Some(4);
            let _ = dopesheet_lane(&s, &t);
        }
    }

    /// The bend fixture from `bend_key_frames_marks_
    /// stored_bend_changes`, as a State: rows arrive on
    /// frame 2, hold on 3, deepen on 4, vanish on 5 —
    /// bend keys [2, 4] over six frames.
    fn bend_state() -> State {
        let mut s = crate::init();
        s.core.project.figures = vec![figure("Alpha")];
        let bent = vec![vec![0.0, 0.35, 0.0]];
        let bent_more = vec![vec![0.0, 0.7, 0.0]];
        s.core.project.frames = vec![
            frame(vec![]),
            frame(vec![]),
            bend_frame(vec![], bent.clone()),
            bend_frame(vec![], bent),
            bend_frame(vec![], bent_more),
            frame(vec![]),
        ];
        s
    }

    #[test]
    fn exposure_spans_run_key_to_key_and_mark_holds() {
        // RM14 (P-2): Alpha keys [0, 2] over 4 frames —
        // two spans, both holds; Beta keys [1, 3] — a
        // hold and a single-frame tail span.
        let s = two_figure_state();
        let alpha = exposure_spans(&s.core.project, 0);
        assert_eq!(
            alpha,
            vec![
                ExposureSpan { start: 0, end: 2 },
                ExposureSpan { start: 2, end: 4 }
            ]
        );
        assert!(alpha.iter().all(|sp| sp.is_hold()));
        let beta = exposure_spans(&s.core.project, 1);
        assert_eq!(
            beta,
            vec![
                ExposureSpan { start: 1, end: 3 },
                ExposureSpan { start: 3, end: 4 }
            ]
        );
        assert!(beta[0].is_hold());
        assert!(!beta[1].is_hold(), "a one-frame span is no hold");
        assert!(beta[0].covers(2) && !beta[0].covers(3));
        // The rows carry the same spans for the builder.
        let rows = dope_rows(&s.core);
        assert_eq!(rows[0].spans, alpha);
        assert_eq!(rows[1].spans, beta);
        // No pose keys, no spans.
        let mut bare = crate::init();
        bare.core.project.figures = vec![figure("Alpha")];
        bare.core.project.frames = vec![frame(vec![])];
        assert!(exposure_spans(&bare.core.project, 0).is_empty());
    }

    #[test]
    fn key_event_at_resolves_the_lane_priority() {
        // RM14: channel over pose over state over bend,
        // one fixture per step down the ladder.
        let mut s = two_figure_state();
        s.core.project.channels = vec![channel(0, ChannelTarget::RootX, &[2.0])];
        assert_eq!(
            key_event_at(&s.core.project, 0, 2, 4),
            Some(DopeKeyRef::Channel {
                channel: 0,
                key_frame: 2.0
            })
        );
        s.core.project.channels.clear();
        assert_eq!(key_event_at(&s.core.project, 0, 2, 4), Some(DopeKeyRef::Pose));
        assert_eq!(key_event_at(&s.core.project, 0, 1, 4), None);
        // State over bend: static poses (keys [0] only),
        // a state key and a bend change both on frame 2.
        let mut s = bend_state();
        s.core.project.frames = vec![
            frame(vec![pose(100.0, 10.0)]),
            frame(vec![pose(100.0, 10.0)]),
            bend_frame(vec![pose(100.0, 10.0)], vec![vec![0.0, 0.35, 0.0]]),
        ];
        s.core.project.figures[0].states = vec![StateKey {
            frame: 2,
            clip: 0,
            offset: 0,
            speed: 1.0,
        }];
        assert_eq!(key_event_at(&s.core.project, 0, 2, 3), Some(DopeKeyRef::State));
        s.core.project.figures[0].states.clear();
        assert_eq!(key_event_at(&s.core.project, 0, 2, 3), Some(DopeKeyRef::Bend));
    }

    #[test]
    fn move_pose_key_retimes_by_the_dense_fill() {
        // RM14 (A4 R5): Alpha's key at frame 2 moves to
        // frame 3 — frame 2 takes the held pose, frame 3
        // takes the key's, and the derivation agrees.
        let mut s = two_figure_state();
        let depth = s.core.undo_depth();
        assert!(move_pose_key(&mut s.core, 0, 2, 3));
        assert_eq!(s.core.undo_depth(), depth + 1, "one undo step");
        assert_eq!(figure_keyframes(&s.core.project, 0), vec![0, 3]);
        assert_eq!(s.core.project.frames[2].poses[0].root, [100.3, 200.0]);
        assert_eq!(s.core.project.frames[3].poses[0].angles[0], 25.0);
        assert!(s.core.status.contains("from frame 3 to frame 4"));
        s.core.undo();
        assert_eq!(figure_keyframes(&s.core.project, 0), vec![0, 2]);
        // Moving EARLIER fills the interval with the
        // key's own pose: Beta's key at 3 moves to 2.
        let mut s = two_figure_state();
        assert!(move_pose_key(&mut s.core, 1, 3, 2));
        assert_eq!(figure_keyframes(&s.core.project, 1), vec![1, 2]);
        assert_eq!(s.core.project.frames[2].poses[1].root, [310.0, 200.0]);
    }

    #[test]
    fn move_pose_key_refusals_spend_no_snapshot() {
        let mut s = two_figure_state();
        let depth = s.core.undo_depth();
        // The base pose cannot move.
        assert!(!move_pose_key(&mut s.core, 0, 0, 1));
        assert!(s.core.status.contains("base pose"), "{}", s.core.status);
        // Beta APPEARS at frame 1: no earlier pose.
        assert!(!move_pose_key(&mut s.core, 1, 1, 2));
        assert!(s.core.status.contains("appears at frame 2"), "{}", s.core.status);
        // Onto another key: Beta 3 -> 1.
        assert!(!move_pose_key(&mut s.core, 1, 3, 1));
        assert!(
            s.core.status.contains("already has a keyframe"),
            "{}",
            s.core.status
        );
        // Past the last frame, and no key at all.
        assert!(!move_pose_key(&mut s.core, 0, 2, 4));
        assert!(!move_pose_key(&mut s.core, 0, 1, 2));
        assert_eq!(s.core.undo_depth(), depth, "no refusal snapshots");
    }

    #[test]
    fn move_state_key_shifts_the_stored_key() {
        let mut s = crate::init();
        let mut fig = figure("Alpha");
        fig.states = vec![
            StateKey { frame: 1, clip: 0, offset: 0, speed: 1.0 },
            StateKey { frame: 3, clip: 1, offset: 2, speed: 0.5 },
        ];
        s.core.project.figures = vec![fig];
        s.core.project.frames = (0..6).map(|_| frame(vec![])).collect();
        let depth = s.core.undo_depth();
        assert!(move_state_key(&mut s.core, 0, 1, 2));
        assert_eq!(s.core.undo_depth(), depth + 1);
        let frames: Vec<usize> = s.core.project.figures[0]
            .states
            .iter()
            .map(|k| k.frame)
            .collect();
        assert_eq!(frames, vec![2, 3], "moved and still sorted");
        assert_eq!(s.core.project.figures[0].states[0].clip, 0, "the key itself moved");
        // Onto the other key refuses; past the end too.
        assert!(!move_state_key(&mut s.core, 0, 2, 3));
        assert!(!move_state_key(&mut s.core, 0, 2, 9));
        assert_eq!(s.core.undo_depth(), depth + 1);
        s.core.undo();
        let frames: Vec<usize> = s.core.project.figures[0]
            .states
            .iter()
            .map(|k| k.frame)
            .collect();
        assert_eq!(frames, vec![1, 3]);
    }

    #[test]
    fn move_bend_key_shifts_the_change_point() {
        let mut s = bend_state();
        assert_eq!(bend_key_frames(&s.core.project, 0), vec![2, 4]);
        let depth = s.core.undo_depth();
        assert!(move_bend_key(&mut s.core, 0, 2, 3));
        assert_eq!(s.core.undo_depth(), depth + 1);
        assert_eq!(bend_key_frames(&s.core.project, 0), vec![3, 4]);
        // Onto the other change refuses, snapshot-free.
        assert!(!move_bend_key(&mut s.core, 0, 3, 4));
        assert_eq!(s.core.undo_depth(), depth + 1);
        // Frame 0 CAN gain a bend key by a move (the
        // implicit zero row is its predecessor).
        assert!(move_bend_key(&mut s.core, 0, 3, 0));
        assert_eq!(bend_key_frames(&s.core.project, 0), vec![0, 4]);
    }

    #[test]
    fn delete_key_event_dispatches_per_kind() {
        // Channel wins over pose on the same frame.
        let mut s = two_figure_state();
        s.core.project.channels = vec![channel(0, ChannelTarget::RootX, &[2.0])];
        assert!(delete_key_event(&mut s.core, 0, 2));
        assert!(s.core.project.channels[0].keys.is_empty());
        assert_eq!(figure_keyframes(&s.core.project, 0), vec![0, 2], "the pose key stays");
        // Pose through the engine op.
        let mut s = two_figure_state();
        assert!(delete_key_event(&mut s.core, 0, 2));
        assert_eq!(figure_keyframes(&s.core.project, 0), vec![0]);
        // State: the stored key goes.
        let mut s = crate::init();
        let mut fig = figure("Alpha");
        fig.states = vec![StateKey { frame: 1, clip: 0, offset: 0, speed: 1.0 }];
        s.core.project.figures = vec![fig];
        s.core.project.frames = (0..3).map(|_| frame(vec![])).collect();
        assert!(delete_key_event(&mut s.core, 0, 1));
        assert!(s.core.project.figures[0].states.is_empty());
        // Bend: the frame holds the previous row, so
        // the change point moves to where the held row
        // first differs from its own predecessor.
        let mut s = bend_state();
        assert!(delete_key_event(&mut s.core, 0, 2));
        assert_eq!(bend_key_frames(&s.core.project, 0), vec![3, 4]);
        // Nothing there at all.
        let mut s = two_figure_state();
        let depth = s.core.undo_depth();
        assert!(!delete_key_event(&mut s.core, 0, 1));
        assert_eq!(s.core.undo_depth(), depth);
    }

    #[test]
    fn stamp_key_event_stamps_dense_keys_ten_frames_on() {
        // Pose: keys at 0 and 1 over 12 frames; the
        // frame-1 key stamps to 11.
        let mut s = crate::init();
        s.core.project.figures = vec![figure("Alpha")];
        let a = pose(100.0, 10.0);
        let b = pose(100.0, 25.0);
        s.core.project.frames = (0..12)
            .map(|f| frame(vec![if f == 1 { b.clone() } else { a.clone() }]))
            .collect();
        assert_eq!(figure_keyframes(&s.core.project, 0), vec![0, 1, 2]);
        assert!(stamp_key_event(&mut s.core, 0, 1));
        assert_eq!(figure_keyframes(&s.core.project, 0), vec![0, 1, 2, 11]);
        // Again: frame 11 is a key now — refused.
        assert!(!stamp_key_event(&mut s.core, 0, 1));
        // State: 1 stamps to 11; a repeat refuses.
        let mut s = crate::init();
        let mut fig = figure("Alpha");
        fig.states = vec![StateKey { frame: 1, clip: 0, offset: 0, speed: 1.0 }];
        s.core.project.figures = vec![fig];
        s.core.project.frames = (0..12).map(|_| frame(vec![])).collect();
        assert!(stamp_key_event(&mut s.core, 0, 1));
        let frames: Vec<usize> = s.core.project.figures[0]
            .states
            .iter()
            .map(|k| k.frame)
            .collect();
        assert_eq!(frames, vec![1, 11]);
        assert!(!stamp_key_event(&mut s.core, 0, 1));
        // Bend: the row at 1 stamps to 11.
        let mut s = crate::init();
        s.core.project.figures = vec![figure("Alpha")];
        s.core.project.frames = (0..12)
            .map(|f| {
                if f == 1 {
                    bend_frame(vec![], vec![vec![0.0, 0.35, 0.0]])
                } else {
                    frame(vec![])
                }
            })
            .collect();
        assert_eq!(bend_key_frames(&s.core.project, 0), vec![1]);
        assert!(stamp_key_event(&mut s.core, 0, 1));
        assert_eq!(bend_key_frames(&s.core.project, 0), vec![1, 11]);
    }

    #[test]
    fn copy_and_paste_ride_the_graph_clipboard() {
        // RM14: the dopesheet's Copy fills the shell
        // clipboard from the SELECTION; Paste lands it
        // on the selected figure's channel of the kind
        // (Beta has no Angle(1) — its Angle(2) takes
        // the paste, AC1's kind looseness), rebased to
        // the playhead.
        let mut s = two_figure_state();
        s.core.project.channels = vec![
            channel(0, ChannelTarget::Angle(1), &[0.0, 2.0]),
            channel(1, ChannelTarget::Angle(2), &[]),
        ];
        let ch0 = s.core.project.channels[0].clone();
        s.key_sel.select_range(&ch0, 0, 0.0, 2.0);
        assert!(copy_active_keys(&mut s));
        let clip = s.key_clipboard.clone().expect("the clipboard is filled");
        assert_eq!(clip.figure, 0);
        assert_eq!(clip.target, ChannelTarget::Angle(1));
        assert_eq!(clip.keys.len(), 2);
        s.core.select_figure(1);
        s.core.goto(1);
        assert!(paste_clipboard_at_playhead(&mut s));
        let frames: Vec<f32> = s.core.project.channels[1]
            .keys
            .iter()
            .map(|k| k.frame)
            .collect();
        assert_eq!(frames, vec![1.0, 3.0], "rebased: earliest lands on the playhead");
        // No clipboard: refusal, in paste_keys' words.
        let mut bare = two_figure_state();
        assert!(!paste_clipboard_at_playhead(&mut bare));
        assert!(bare.core.status.contains("Nothing to paste"));
    }

    #[test]
    fn drag_moves_a_channel_key_and_cancel_restores_it() {
        // RM14: the cell drag at State level — Down
        // selects + grabs, one pitch of travel moves
        // the key one frame under ONE snapshot, Up
        // settles the status; a cancelled drag spends
        // its snapshot on the rollback and no more.
        let mut s = two_figure_state();
        s.core.project.channels = vec![channel(0, ChannelTarget::RootX, &[2.0])];
        let pitch = CHIP_W + CHIP_GAP;
        let mut drag: Option<DopeDrag> = None;
        drag_down(&mut s, &mut drag, 0, 2);
        let g = drag.as_ref().expect("a record is planted");
        assert!(matches!(g.kind, DopeKeyRef::Channel { .. }));
        assert_eq!(s.key_sel.frames(), vec![2.0]);
        assert_eq!(s.core.current, 2);
        let depth = s.core.undo_depth();
        drag_apply(&mut s, &mut drag, pitch);
        assert_eq!(s.core.project.channels[0].keys[0].frame, 3.0);
        assert_eq!(s.core.undo_depth(), depth + 1, "one snapshot for the gesture");
        drag_up(&mut s, &mut drag, pitch, false);
        assert!(drag.is_none());
        assert!(
            s.core.status.contains("Moved 1 selected key by +1 frame"),
            "{}",
            s.core.status
        );
        // Cancel: the key returns, the depth with it.
        let mut drag: Option<DopeDrag> = None;
        drag_down(&mut s, &mut drag, 0, 3);
        drag_apply(&mut s, &mut drag, pitch);
        assert_eq!(s.core.project.channels[0].keys[0].frame, 4.0);
        let depth = s.core.undo_depth();
        drag_up(&mut s, &mut drag, pitch, true);
        assert_eq!(s.core.project.channels[0].keys[0].frame, 3.0);
        assert_eq!(s.core.undo_depth(), depth - 1, "the rollback consumes the staged step");
    }

    #[test]
    fn drag_moves_a_pose_key_and_a_click_spends_nothing() {
        let mut s = two_figure_state();
        let pitch = CHIP_W + CHIP_GAP;
        let mut drag: Option<DopeDrag> = None;
        drag_down(&mut s, &mut drag, 0, 2);
        assert!(matches!(
            drag.as_ref().map(|g| g.kind),
            Some(DopeKeyRef::Pose)
        ));
        drag_apply(&mut s, &mut drag, pitch);
        drag_up(&mut s, &mut drag, pitch, false);
        assert_eq!(figure_keyframes(&s.core.project, 0), vec![0, 3]);
        assert!(
            s.core.status.contains("Moved figure 1 keyframe from frame 3 to frame 4"),
            "{}",
            s.core.status
        );
        // A plain click (Down + Up, no travel) stages
        // no snapshot and moves nothing.
        let depth = s.core.undo_depth();
        let mut drag: Option<DopeDrag> = None;
        drag_down(&mut s, &mut drag, 0, 3);
        drag_up(&mut s, &mut drag, 0.0, false);
        assert_eq!(s.core.undo_depth(), depth);
        assert_eq!(figure_keyframes(&s.core.project, 0), vec![0, 3]);
        assert_eq!(s.core.current, 3, "the click still navigates");
    }

    #[test]
    fn active_verbs_dispatch_selection_then_playhead() {
        // Selection first: a live selection moves and
        // deletes as a batch.
        let mut s = two_figure_state();
        s.core.project.channels = vec![channel(0, ChannelTarget::RootX, &[2.0])];
        s.key_sel.select_one(0, 2.0);
        assert!(move_active_keys(&mut s, 1));
        assert_eq!(s.core.project.channels[0].keys[0].frame, 3.0);
        assert!(delete_active_keys(&mut s));
        assert!(s.core.project.channels[0].keys.is_empty());
        // Playhead next: no selection — the pose key
        // under the playhead moves, then deletes.
        let mut s = two_figure_state();
        s.core.goto(2);
        assert!(move_active_keys(&mut s, -1));
        assert_eq!(figure_keyframes(&s.core.project, 0), vec![0, 1]);
        s.core.goto(1);
        assert!(delete_active_keys(&mut s));
        assert_eq!(figure_keyframes(&s.core.project, 0), vec![0]);
        // Nothing under the playhead: honest refusals.
        let mut s = two_figure_state();
        s.core.goto(1);
        assert!(!move_active_keys(&mut s, 1));
        assert!(!delete_active_keys(&mut s));
        assert!(!copy_active_keys(&mut s));
    }

    #[test]
    fn lane_constructs_with_the_actions_row_contexts() {
        // RM14: the actions row's three contexts — a
        // live selection, a playhead event (the plain
        // sheet), and a pasteable clipboard on a
        // keyless project — each build the lane.
        for dark in [true, false] {
            let t = rune::tok::theme(dark);
            let mut s = two_figure_state();
            s.core.project.channels = vec![channel(0, ChannelTarget::RootX, &[2.0])];
            s.key_sel.select_one(0, 2.0);
            let _ = dopesheet_lane(&s, &t);
            let s = two_figure_state();
            let _ = dopesheet_lane(&s, &t);
            let mut s = crate::init();
            s.core.project.figures = vec![figure("Alpha")];
            s.core.project.frames = vec![frame(vec![])];
            s.core.project.channels = vec![channel(0, ChannelTarget::RootX, &[])];
            s.key_clipboard = Some(KeyClipboard {
                figure: 0,
                target: ChannelTarget::RootX,
                keys: vec![Key::flat(0.0, 1.0)],
            });
            let _ = dopesheet_lane(&s, &t);
        }
    }

    #[test]
    fn lane_constructs_on_every_state_branch() {
        // Element trees are not readable from unit
        // tests — construction is the assertable level
        // here (the design kit's own discipline); the
        // presented-tree proof belongs to the Runtime
        // suite and the headless snaps.
        for dark in [true, false] {
            let t = rune::tok::theme(dark);
            // The demo project (one figure, keyframes
            // on every frame).
            let s = crate::init();
            let _ = dopesheet_lane(&s, &t);
            // No figures at all.
            let mut s = crate::init();
            s.core.project.figures = Vec::new();
            let _ = dopesheet_lane(&s, &t);
            // Figures but no keyframes anywhere.
            let mut s = crate::init();
            s.core.project.figures = vec![figure("Alpha")];
            s.core.project.frames = vec![frame(vec![])];
            let _ = dopesheet_lane(&s, &t);
            // State keys only (no poses anywhere): the
            // rows branch, not the no-keys empty state
            // (Wave Y, Y11).
            let mut s = crate::init();
            let mut fig = figure("Alpha");
            fig.states = vec![StateKey { frame: 0, clip: 0, offset: 0, speed: 1.0 }];
            s.core.project.figures = vec![fig];
            s.core.project.frames = vec![frame(vec![])];
            let _ = dopesheet_lane(&s, &t);
            // Past the row cap, selected pinned.
            let s = many_figure_state(7);
            let _ = dopesheet_lane(&s, &t);
            // The two-figure keyframe sheet.
            let s = two_figure_state();
            let _ = dopesheet_lane(&s, &t);
        }
    }
}
