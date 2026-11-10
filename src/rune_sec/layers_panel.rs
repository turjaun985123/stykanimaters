//! Layers panel for the Rune editor shell (Phase D,
//! section 32). Bin-crate code — declared under `rune_sec`
//! by the integrator, not part of the engine library.
//!
//! ## What a "layer" is in this engine (the truth from the code)
//!
//! There is **no separate layer list and no per-figure layer
//! assignment field**. The layers engine (`src/layers.rs`
//! module docs) defines draw order as the order of
//! `Project::figures`: index 0 is drawn first, i.e. at the
//! back; the last index is drawn last, i.e. in front. Every
//! figure *is* a layer, and "move up / move down" is figure
//! reordering. The panel states this in its note text rather
//! than pretending a distinct layer stack exists.
//!
//! ## Controls → real ops (all via `s.core`)
//!
//! * Row name press → `EditorCore::select_figure(fi)`
//!   (editor.rs:1646).
//! * Move up (toward front) / Move down (toward back) →
//!   the handler first selects the row's figure, then calls
//!   `EditorCore::move_figure_full(+1 / -1)` (editor.rs:1088).
//!   The "full" wrapper is used deliberately: the simpler
//!   `EditorCore::move_figure` (editor.rs:1655) only swaps
//!   `figures` + frame poses, while `move_figure_full` goes
//!   through `layers::move_figure` (layers.rs:162), which
//!   permutes poses and remaps camera-follow, particle, FX
//!   clip and graph-channel figure references identically,
//!   and the wrapper remaps the editor's selection and solo
//!   index. At the ends of the list the op is a safe no-op.
//! * Top / Bottom (Phase G) → the same handler shape with
//!   the *whole* distance as the delta:
//!   `move_figure_full(target - fi)` where `target` comes
//!   from [`jump_target`] (`len - 1` for Top — the panel's
//!   top row is the front figure — `0` for Bottom). One op
//!   call, not a loop of single steps: `layers::move_figure`
//!   takes a delta of any size, clamps it to the ends, and
//!   does a single remove + insert with the same reference
//!   remap, and the wrapper takes exactly one snapshot —
//!   so a jump costs one undo entry, same as a step. When
//!   [`jump_target`] is `None` (already at that end, or
//!   fewer than two figures) the handler is inert: it only
//!   posts a status line — no selection change, no op call.
//! * Opacity − / + steppers →
//!   `EditorCore::step_figure_opacity(fi, ∓OPACITY_STEP)`
//!   (editor.rs:1177), which writes through
//!   `layers::set_opacity` (layers.rs:126) — clamped to
//!   `MIN_OPACITY..=MAX_OPACITY` (0.05..=1.0, layers.rs:34-36),
//!   so a figure can be faint but never invisible via opacity.
//! * Opacity drag slider (Phase F) → the same write for the
//!   *selected* figure, at an absolute value: the backing
//!   [`set_figure_opacity`] rounds to the stepper's whole-
//!   percent grid and calls `layers::set_opacity` exactly as
//!   `step_figure_opacity` does, with the op's own status
//!   string — but only on release (see below).
//!
//! The slider follows the scene panel's weather sliders
//! (Phase E): the Rune settings example's `speed_control`
//! shape — `on_drag` with the press value stored in
//! `Drag::data`, a write on every Move, `tx.instant()` while
//! scrubbing, and a cancelled drag restoring the press
//! value. The value comes from the pure [`opacity_at`]
//! mapping over the element's own width (no grab offset —
//! a track click jumps, as in the example), and the status
//! line updates **only on release** — mid-drag writes stay
//! silent so a scrub does not spam the status line or the
//! toasts fed from it. The per-row steppers stay for fine
//! control.
//!
//! No undo snapshots, on purpose: opacity writes were never
//! snapshotted — `step_figure_opacity` and
//! `layers::set_opacity` never call `EditorCore::snapshot()`,
//! so a stepper press costs zero undo entries, and the
//! slider mirrors that exactly: a whole drag also costs
//! zero. (Contrast the camera dial, whose backing op *does*
//! snapshot; this one does not.)
//!
//! Rows are listed in draw order with the front figure on
//! top (reverse index order). The selected figure's row is
//! the marked one, matching `core.selected_figure` (the
//! name button took the accent before Phase N; the kit
//! row shell carries the mark now — see the Phase N note
//! below).
//!
//! Phase H (section 9/40) switches the row's step buttons
//! to Font Awesome glyphs through
//! `icon_btn::icon_only_btn`: Down is `chevron-down`, Up is
//! `chevron-up`. Keys and handlers are unchanged. The
//! Top/Bottom jumps stayed text on purpose at the time:
//! they are jumps to an end, not steps, the curated icon
//! set then carried no jump-to-end glyph (`angles-up` /
//! `angles-down` were not in it), and giving them the
//! same chevrons as the step buttons would make a jump
//! look like a step. (Phase N's spillover reversed this
//! once the glyphs existed — see the Phase N note
//! below.) The opacity
//! −/+ steppers also stay text, matching the figures
//! panel's identical steppers.
//!
//! Phase I (section 35/40) adds a small visibility footer
//! row at the panel's end — the per-row eye/solo toggles
//! still live on the figures panel's rows, but the two
//! bulk shortcuts that panel keeps in its footer are
//! useful here too, where layer work happens:
//!
//! * Show all → [`show_all_figures`]: the figures footer's
//!   All handler verbatim — `layers::set_all_visible(..,
//!   true)`, solo cleared, and the same status string
//!   ("All figures shown ({changed} changed)."). The solo
//!   clear is deliberate: `set_all_visible`'s own docs
//!   leave solo to the caller, and a stale solo would keep
//!   the stage showing only one figure despite the report.
//! * Solo selected → [`solo_selected`]: the engine's
//!   `EditorCore::toggle_solo` (editor.rs:1222) aimed at
//!   the selected figure — the same call the figures
//!   panel's per-row solo buttons make. The op owns the
//!   status strings ("Solo: figure {n}." with the 1-based
//!   figure *number*, "Solo off."), and because it is a
//!   toggle, pressing the button while the selected figure
//!   is already the solo turns solo off.
//!
//! Phase J (section 30/40) adds a small colour dot at the
//! start of each row, before the figure name, so a layer
//! can be matched to its figure's colour at a glance. The
//! dot's colour comes from [`layer_dot_color`]: the
//! figure's **root node** (node 0) resolved with the
//! engine's own idiom, `node.color.unwrap_or(fig.color)`
//! (export.rs's segment writer; the same resolution
//! figures_panel's recolour source uses) — an explicit
//! root colour wins, an inheriting root shows the figure
//! colour. A layer row has no meaningful "selected node",
//! so the root is the honest representative, and because
//! `FigureInstance::color` is a plain `u32` (never an
//! option) a colour always resolves; a figure whose def
//! somehow has no nodes at all falls back to the figure
//! colour directly.
//!
//! Phase L (section 35/40) adds a muted trailing readout
//! to each row: the figure's joint count from
//! [`joint_label`] ("{n} joints", "1 joint" singular) —
//! `fig.def.nodes.len()`, nothing more. It sits after the
//! Top button as plain unkeyed text (the opacity readout's
//! idiom), short enough that the row's controls keep
//! their room in the 300pt column; the name button is the
//! row's flexible element and absorbs any squeeze.
//!
//! Phase N (adoption) restyles the panel to the charter's
//! card language: the root is the kit's
//! `design::panel_col` (card fill, hairline border,
//! radius 12, padding 12), the header is the kit's
//! `panel_title` over a kit `divider`, and each layer row
//! is a kit `design::list_row` shell (min height 28,
//! px 8, radius 8, hover) whose selected fill carries the
//! selection — the name button inside no longer takes the
//! accent fill, keeping only its key and its
//! `select_figure` press. The "Opacity" caption is the
//! kit's `section_label`. The opacity slider's drag root
//! grows from 22 to 28pt tall (P18: the drag height
//! clears the 24pt floor; the drag maps x only, so the
//! track/knob arithmetic — all derived from `SLIDER_H` —
//! is unchanged in shape). Keys, handlers and status
//! strings are all byte-identical to before.
//!
//! Phase N (V-wave spillover): the Bottom/Top jumps are
//! icon buttons now. The foundation wave curated the
//! `angles-up` / `angles-down` glyphs the Phase H note
//! above found missing, so the jumps take `icon_btn`
//! with those glyphs beside their labels — a jump no
//! longer looks like the chevron steps, and the Phase H
//! paragraph's reason for keeping them text no longer
//! holds. Keys (`layer-bottom-{fi}` / `layer-top-{fi}`),
//! labels and handlers are unchanged.
//!
//! UI mission, Wave 1 (P05 item O-3; S07 items 2, 5b
//! and 6): the layer row is two lines now. Measured from
//! the Phase N headless tree dumps, the single line ran
//! ≥134 pt past the card: `layer-down-{fi}` already
//! started past the content edge, `layer-top-{fi}` ended
//! at x≈438 and the Phase L joint readout at ≈466 —
//! painted into the centre gutter, unreachable by the
//! mouse. Line 1 keeps the dot, the name and the opacity
//! steppers; line 2 takes the reorder cluster
//! (Bottom/down/up/Top, keys verbatim) with the joint
//! readout trailing: 77 + 26 + 26 + 58 + 3 gaps × 6 +
//! readout ≈ 253 of the row's 260 pt inner width. The
//! row root is the figures panel's two-line variant of
//! the kit list-row anatomy (the Phase N paragraph
//! above describes the single-line shell it replaces):
//! the same px, radius, selected fill and hover as
//! `design::list_row`, applied to a `col` because the
//! kit builder itself is a single-line `row`.
//!
//! Two behaviours ride the same restructure. The
//! reorder buttons now render their kit inert twins at
//! the ends of the list — Down/Up keyed off the row's
//! `can_move_down` / `can_move_up` (computed and tested
//! since the panel's first version, but never read by
//! the builder until now), Bottom/Top off
//! [`jump_target`] — so an impossible move reads as
//! unavailable instead of silently no-oping, the pose
//! panel's gating discipline. And a hidden figure's row
//! dims: the name takes the chrome family's
//! `btn_muted_live` form (same box, key and
//! `select_figure` handler; secondary ink, no
//! hover/press fills), the layers counterpart of the
//! figures panel's hidden-name dimming — the dot keeps
//! the figure colour, as there. Keys, labels and
//! handler bodies are byte-identical throughout; the
//! only new row data is the pure derivations on
//! [`LayerRow`] (`hidden`, `can_jump_top`,
//! `can_jump_bottom`).
//!
//! UI mission, Wave 9 (the gap the Wave 6 empty-state
//! sweep flagged and skipped): with zero figures the
//! panel now renders the kit's `design::empty_state`
//! (`layer-group` glyph, [`EMPTY_MESSAGE`]) between the
//! rows area and the opacity block, on the predicate
//! `s.core.project.figures.is_empty()`. Before, the
//! panel was header + note + footer with a silent gap
//! where the layers belong. The message points at the
//! panel's real next action: this panel has no
//! add-figure control of its own, and Layers mounts
//! only in the Animate workspace (workspaces.rs), where
//! the Figures section — the "+ Figure" button and the
//! figure-add card — renders directly above it in the
//! same column, so "add one with + Figure above" is
//! literally where the control sits. The note and the
//! visibility footer stay in the empty state too: the
//! note explains what a layer is, and the footer
//! presses remain the harmless, truthfully-reporting
//! no-ops they already were with no figures ("All
//! figures shown (0 changed)."; solo is a guarded
//! no-op — both pinned by tests below).

use rune::tok::{self, type_, Theme};
use rune::{col, el, row, text, Drag, El, Phase, Tx};
use stykanimate_rs::editor::EditorCore;

use crate::chrome_view::{btn, btn_muted_live};
use crate::rune_sec::design;
use crate::rune_sec::icon_btn::{icon_btn, icon_btn_inert, icon_only_btn, icon_only_btn_inert};
use crate::State;

/// Opacity change applied by one stepper press.
pub const OPACITY_STEP: f32 = 0.1;

/// The panel's honest note about ordering in this engine.
pub const ORDER_NOTE: &str =
    "Order = figure order: top row is drawn in front, bottom row at the back.";

/// The zero-figures empty state's message (Wave 9 — see
/// the module docs). It names the real next action and
/// where it lives: the Figures area's "+ Figure" button,
/// which renders above this panel in the only workspace
/// that mounts it.
pub const EMPTY_MESSAGE: &str = "No figures yet — add one with + Figure above.";

/// One display row of the panel, in panel order (front first).
#[derive(Clone, Debug, PartialEq)]
pub struct LayerRow {
    /// Index into `Project::figures`.
    pub fi: usize,
    /// Figure name as shown.
    pub name: String,
    /// The row's colour-dot colour: the figure's root-node
    /// colour resolved by [`layer_dot_color`].
    pub dot_color: u32,
    /// Layer opacity as a whole percentage (5..=100).
    pub opacity_pct: i64,
    /// Trailing joint-count readout text ([`joint_label`]).
    pub joint_text: String,
    /// True when this is the editor's selected figure.
    pub selected: bool,
    /// True when the figure is hidden — the row dims
    /// (name in secondary ink; see the mission note in
    /// the module docs).
    pub hidden: bool,
    /// True when this row can still move up (toward front).
    pub can_move_up: bool,
    /// True when this row can still move down (toward back).
    pub can_move_down: bool,
    /// True when a Top jump would move this row
    /// ([`jump_target`] is `Some`) — the Top button is
    /// live exactly then, inert otherwise.
    pub can_jump_top: bool,
    /// True when a Bottom jump would move this row
    /// ([`jump_target`] is `Some`) — the Bottom button
    /// is live exactly then, inert otherwise.
    pub can_jump_bottom: bool,
}

/// Figure indices in panel order: front (highest index,
/// drawn last) first, back (index 0, drawn first) last.
pub fn display_order(figure_count: usize) -> Vec<usize> {
    (0..figure_count).rev().collect()
}

/// A figure's layer opacity as a whole percentage.
pub fn opacity_pct(opacity: f32) -> i64 {
    (opacity * 100.0).round() as i64
}

/// The opacity readout text, e.g. `"100%"`.
pub fn opacity_label(opacity: f32) -> String {
    format!("{}%", opacity_pct(opacity))
}

/// True when figure `fi` can move one slot toward the front
/// (a higher draw index exists).
pub fn can_move_up(fi: usize, figure_count: usize) -> bool {
    figure_count > 0 && fi + 1 < figure_count
}

/// True when figure `fi` can move one slot toward the back
/// (it is not already index 0).
pub fn can_move_down(fi: usize, _figure_count: usize) -> bool {
    fi > 0
}

/// Where a Top/Bottom jump would land figure `fi` in a list
/// of `len` figures: `Some(len - 1)` for Top (the front —
/// the panel's top row), `Some(0)` for Bottom (the back).
/// `None` when the jump would not move anything: the figure
/// is already at that end, the list has fewer than two
/// figures, or `fi` is not a figure index at all — the
/// caller treats `None` as inert (status line only).
pub fn jump_target(fi: usize, len: usize, to_top: bool) -> Option<usize> {
    if len < 2 || fi >= len {
        return None;
    }
    let target = if to_top { len - 1 } else { 0 };
    if target == fi {
        None
    } else {
        Some(target)
    }
}

/// The colour of a figure's layer-row dot: its root
/// node's colour resolved the engine's way,
/// `node.color.unwrap_or(fig.color)` — an explicit root
/// colour (`NodeDef::color` is `Some`) wins; an inheriting
/// root resolves to the figure's own colour.
/// `FigureInstance::color` is a plain `u32`, so this
/// always yields a colour; a def with no nodes at all
/// falls back to the figure colour directly.
pub fn layer_dot_color(fig: &stykanimate_rs::model::FigureInstance) -> u32 {
    match fig.def.nodes.first() {
        Some(root) => root.color.unwrap_or(fig.color),
        None => fig.color,
    }
}

/// The layer row's trailing joint-count readout: the
/// figure's def node count as "{n} joints", with the
/// singular "1 joint" for a one-node def (and "0 joints"
/// for an empty one). Display only — the count is
/// `fig.def.nodes.len()`, read from the same def the dot
/// colour resolves. Kept to the count alone so the short
/// muted label fits the 300pt column after the row's
/// existing controls.
pub fn joint_label(fig: &stykanimate_rs::model::FigureInstance) -> String {
    match fig.def.nodes.len() {
        1 => "1 joint".to_string(),
        n => format!("{n} joints"),
    }
}

/// Build the panel's row data from the core, front first.
pub fn rows(core: &EditorCore) -> Vec<LayerRow> {
    let figures = &core.project.figures;
    display_order(figures.len())
        .into_iter()
        .map(|fi| LayerRow {
            fi,
            name: figures[fi].name.clone(),
            dot_color: layer_dot_color(&figures[fi]),
            opacity_pct: opacity_pct(figures[fi].opacity),
            joint_text: joint_label(&figures[fi]),
            selected: fi == core.selected_figure,
            hidden: figures[fi].hidden,
            can_move_up: can_move_up(fi, figures.len()),
            can_move_down: can_move_down(fi, figures.len()),
            can_jump_top: jump_target(fi, figures.len(), true).is_some(),
            can_jump_bottom: jump_target(fi, figures.len(), false).is_some(),
        })
        .collect()
}

/// Slider track width in points (the scene panel's slider
/// geometry, so every drag slider in the shell feels the
/// same).
const SLIDER_W: f32 = 200.0;
/// Slider drag-root height in points. 28 since Phase N
/// (P18: a 22pt drag root fell under the 24pt hit-target
/// floor; the drag maps x only, so growing the root is
/// safe). The scene panel's copy of this const grows to
/// the same 28 in its own adoption, so the two sliders
/// do not drift.
const SLIDER_H: f32 = 28.0;
/// The design kit's slider-track element height in
/// points (`design::slider_track` draws a 200x22 track
/// with a 16pt knob, so this file keeps no knob constant
/// of its own); the opacity slider centres it inside its
/// 28pt drag root, exactly as the scene panel's Amount
/// slider does.
const KIT_SLIDER_TRACK_H: f32 = 22.0;

/// Slider mapping: the opacity at local `x` on a track `w`
/// points wide — 0.0 at the left edge, 1.0 at the right,
/// linear between, clamped outside. A zero-width track (no
/// layout yet) maps to 0.0 rather than dividing by zero.
/// The engine's 0.05 floor is *not* applied here — it lives
/// in `layers::set_opacity`, the one write path, exactly as
/// a stepper press can also request a below-floor value.
pub fn opacity_at(x: f32, w: f32) -> f32 {
    if w <= 0.0 {
        return 0.0;
    }
    (x / w).clamp(0.0, 1.0)
}

/// Track fill fraction for an opacity — the inverse of
/// [`opacity_at`], so the knob sits where a press at the
/// same fraction would land.
pub fn opacity_frac(opacity: f32) -> f32 {
    opacity.clamp(0.0, 1.0)
}

/// Slider backing: set figure `fi`'s opacity to an absolute
/// `value`, through the stepper's own write — rounded to
/// the whole-percent grid `step_figure_opacity` computes,
/// then `layers::set_opacity` (which clamps to
/// 0.05..=1.0). Records nothing itself: the drag owns the
/// gesture's one undo step (see [`opacity_drag`]). `report`
/// is true only on drag release: a reporting write posts
/// the op's own status string (or its "No such figure."
/// refusal); mid-drag writes stay silent.
pub fn set_figure_opacity(core: &mut EditorCore, fi: usize, value: f32, report: bool) {
    let next = (value * 100.0).round() / 100.0;
    if stykanimate_rs::layers::set_opacity(&mut core.project, fi, next) {
        if report {
            let o = core.project.figures[fi].opacity;
            core.status = format!("Figure {} opacity {:.0}%.", fi + 1, o * 100.0);
        }
    } else if report {
        core.status = "No such figure.".into();
    }
}

/// Opacity slider drag (the settings example's shape, via
/// the scene panel): Down stores the selected figure's
/// opacity in `Drag::data` and jumps to the pressed point,
/// Move rewrites the opacity and asks for an instant
/// redraw, Up reports — or, when the gesture was cancelled,
/// restores the press value (silently, through the same
/// backing write).
fn opacity_drag(s: &mut State, d: &mut Drag, tx: &mut Tx) {
    let core = &mut s.core;
    let fi = core.selected_figure;
    match d.phase {
        Phase::Down => {
            d.data = core
                .project
                .figures
                .get(fi)
                .map(|f| f.opacity)
                .unwrap_or(1.0);
            // Roadmap Wave 1 (RM24, P-14): the gesture takes
            // its one undo step up front (labelled, never
            // folded — a fold would leave the Up rollback
            // nothing of its own to pop; the scene sliders'
            // shape). No figure, no step.
            if core.project.figures.get(fi).is_some() {
                core.snapshot_labeled("Figure opacity");
            }
            set_figure_opacity(core, fi, opacity_at(d.x, d.w), false);
        }
        Phase::Move => {
            set_figure_opacity(core, fi, opacity_at(d.x, d.w), false);
            tx.instant();
        }
        Phase::Up => {
            if d.cancelled() {
                set_figure_opacity(core, fi, d.data, false);
                core.rollback_coalesced("Figure opacity");
            } else {
                set_figure_opacity(core, fi, opacity_at(d.x, d.w), true);
                // RM24: a gesture that ends on its press
                // value spent nothing — roll the Down step
                // back.
                if core.project.figures.get(fi).map(|f| f.opacity) == Some(d.data) {
                    core.rollback_coalesced("Figure opacity");
                }
            }
        }
    }
}

/// The selected figure's opacity slider: a fixed-size
/// drag root whose whole visual (track, fill, knob) is
/// the kit's `design::slider_track`, centred inside the
/// 28pt root — the scene panel's Amount-slider shape, so
/// the two unipolar sliders in the shell share one track
/// construction and one set of rail/knob colours (the
/// pre-kit hand-drawn copy used `control_off` for the
/// rail and `text` for the knob; the V-wave audit found
/// the drift and the integrator converted it). All
/// gesture logic lives in [`opacity_drag`].
fn opacity_slider(frac: f32, t: &Theme) -> El {
    el()
        .key("layers-opacity-slider")
        .size(SLIDER_W, SLIDER_H)
        .on_drag(opacity_drag)
        .child(
            design::slider_track(frac, t)
                .absolute(0.0, (SLIDER_H - KIT_SLIDER_TRACK_H) / 2.0),
        )
}

/// Move handler shared by the up/down buttons: select the
/// row's figure, then reorder it with the full layers engine.
/// `dir` is +1 toward the front, -1 toward the back.
fn move_layer(core: &mut EditorCore, fi: usize, dir: i32) {
    core.select_figure(fi);
    core.move_figure_full(dir);
}

/// Jump handler shared by the Top/Bottom buttons: like
/// [`move_layer`], but the delta is the whole distance to
/// the end, so the figure lands at the front (`to_top`) or
/// the back in a single `move_figure_full` call — one
/// snapshot, one reference remap, exactly like a step.
/// When [`jump_target`] has nowhere to go the press is
/// inert: only the status line changes.
fn jump_layer(core: &mut EditorCore, fi: usize, to_top: bool) {
    let len = core.project.figures.len();
    match jump_target(fi, len, to_top) {
        Some(target) => {
            core.select_figure(fi);
            core.move_figure_full(target as i32 - fi as i32);
        }
        None => {
            core.status = if len < 2 {
                "Only one figure: nothing to reorder.".into()
            } else if to_top {
                "Already at the top.".into()
            } else {
                "Already at the bottom.".into()
            };
        }
    }
}

/// Show-all handler (the footer's left button): every
/// figure's `hidden` flag cleared through the layers
/// engine, solo cleared with it so the bulk result is the
/// effective visibility, and the figures panel footer's
/// own status string. Since Roadmap Wave 1 (RM24, P-14)
/// the bulk write takes exactly one undo step when at
/// least one figure was hidden — before, this handler
/// took no snapshot AND counted no edit (a census hole:
/// the write was invisible to undo and autosave alike).
fn show_all_figures(core: &mut EditorCore) {
    if core.project.figures.iter().any(|f| f.hidden) {
        core.snapshot_labeled("Figure visibility");
    }
    let changed = stykanimate_rs::layers::set_all_visible(&mut core.project, true);
    core.solo = None;
    core.status = format!("All figures shown ({changed} changed).");
}

/// Solo-selected handler (the footer's right button): the
/// engine's solo toggle aimed at the selected figure —
/// the same call the figures panel's per-row solo buttons
/// make, status strings included ("Solo: figure {n}."
/// when it engages, "Solo off." when the selected figure
/// was already the solo). The op guards the index itself,
/// so a press with no figures is a safe no-op.
fn solo_selected(core: &mut EditorCore) {
    core.toggle_solo(core.selected_figure);
}

/// One layer row, on two lines (UI mission Wave 1 —
/// see the module docs' mission note). The root is the
/// figures panel's two-line variant of the kit list-row
/// anatomy: the same px, radius, selected fill and hover
/// as `design::list_row`, applied to a `col` because the
/// row carries two control lines (the kit builder itself
/// is a single-line `row`). The root carries the
/// selected mark — the name button takes no accent fill.
///
/// Line 1: a colour dot (the figure's resolved root
/// colour, [`layer_dot_color`]), the name (press
/// selects; a hidden figure's name renders through the
/// chrome family's muted-live member — same key, same
/// handler, secondary ink), and the opacity readout
/// with its −/+ steppers. Line 2: the reorder cluster —
/// Bottom/down/up/Top; Down/Up are chevron icon-only
/// buttons, Bottom/Top are angles-glyph icon buttons
/// (Phase N spillover; see the module docs) — and the
/// muted trailing joint-count readout ([`joint_label`];
/// unkeyed text, like the opacity readout). A reorder
/// button whose move is impossible from this row (the
/// ends of the list) renders its kit inert twin — same
/// key, same glyph, no handler — instead of a live
/// button whose press silently no-ops. Keys are
/// per-figure: `layer-row-{fi}`, `layer-dot-{fi}`,
/// `layer-op-dec-{fi}`, `layer-op-inc-{fi}`,
/// `layer-bottom-{fi}`, `layer-down-{fi}`, `layer-up-{fi}`,
/// `layer-top-{fi}` (1-based in the key is avoided — keys
/// carry the engine index so headless `--tree` checks map
/// straight back to `Project::figures`).
fn layer_row(r: &LayerRow, t: &Theme) -> El {
    let fi = r.fi;
    let root = col()
        .key(format!("layer-row-{fi}"))
        .w_full()
        .gap(2.0)
        .px(8.0)
        .py(4.0)
        .radius(design::ROW_RADIUS)
        .min_h(28.0);
    let root = if r.selected {
        root.fill(design::selected_fill())
    } else {
        root.hover(t.hover)
    };
    // The name button: `btn` exactly as before, or its
    // muted-live family member while the figure is
    // hidden — the dimming the figures panel applies to
    // a hidden figure's name, in this panel's button
    // dialect. Key, label and handler are identical
    // either way.
    let name_btn = if r.hidden {
        btn_muted_live(
            &format!("layer-name-{fi}"),
            &r.name,
            false,
            t,
            move |s: &mut State| s.core.select_figure(fi),
        )
    } else {
        btn(
            &format!("layer-name-{fi}"),
            &r.name,
            false,
            t,
            move |s: &mut State| s.core.select_figure(fi),
        )
    };
    root.child(
        // Line 1: dot / name / opacity steppers.
        row()
            .w_full()
            .gap(6.0)
            .center_y()
            .child(
                el()
                    .key(format!("layer-dot-{fi}"))
                    .size(10.0, 10.0)
                    .radius(5.0)
                    .fill(
                        crate::stage_view::hex_color(r.dot_color)
                            .unwrap_or(rune::Color::WHITE),
                    ),
            )
            .child(name_btn)
            .child(el().grow(1.0))
            .child(design::step_btn(
                &format!("layer-op-dec-{fi}"),
                "-",
                // Wave Q: inert at the readout's own
                // bounds, the StepForm split Wave P gave
                // the inspector's opacity pair (the op
                // clamps to 0..=1).
                if r.opacity_pct == 0 {
                    design::StepForm::Noop
                } else {
                    design::StepForm::Live
                },
                t,
                move |s: &mut State| s.core.step_figure_opacity(fi, -OPACITY_STEP),
            ))
            .child(
                text(format!("{}%", r.opacity_pct))
                    .font(type_::LABEL)
                    .color(t.text),
            )
            .child(design::step_btn(
                &format!("layer-op-inc-{fi}"),
                "+",
                if r.opacity_pct == 100 {
                    design::StepForm::Noop
                } else {
                    design::StepForm::Live
                },
                t,
                move |s: &mut State| s.core.step_figure_opacity(fi, OPACITY_STEP),
            )),
    )
    .child(
        // Line 2: the reorder cluster, with the joint
        // readout trailing at the right edge.
        row()
            .w_full()
            .gap(6.0)
            .center_y()
            .child(if r.can_jump_bottom {
                icon_btn(
                    &format!("layer-bottom-{fi}"),
                    "angles-down",
                    "Bottom",
                    false,
                    t,
                    move |s: &mut State| jump_layer(&mut s.core, fi, false),
                )
            } else {
                icon_btn_inert(
                    &format!("layer-bottom-{fi}"),
                    "angles-down",
                    "Bottom",
                    t,
                )
            })
            .child(if r.can_move_down {
                icon_only_btn(
                    &format!("layer-down-{fi}"),
                    "chevron-down",
                    "Move down",
                    26.0, false,
                    t,
                    move |s: &mut State| move_layer(&mut s.core, fi, -1),
                )
            } else {
                icon_only_btn_inert(
                    &format!("layer-down-{fi}"),
                    "chevron-down",
                    "Move down",
                    26.0,
                    t,
                )
            })
            .child(if r.can_move_up {
                icon_only_btn(
                    &format!("layer-up-{fi}"),
                    "chevron-up",
                    "Move up",
                    26.0, false,
                    t,
                    move |s: &mut State| move_layer(&mut s.core, fi, 1),
                )
            } else {
                icon_only_btn_inert(
                    &format!("layer-up-{fi}"),
                    "chevron-up",
                    "Move up",
                    26.0,
                    t,
                )
            })
            .child(if r.can_jump_top {
                icon_btn(
                    &format!("layer-top-{fi}"),
                    "angles-up",
                    "Top",
                    false,
                    t,
                    move |s: &mut State| jump_layer(&mut s.core, fi, true),
                )
            } else {
                icon_btn_inert(&format!("layer-top-{fi}"), "angles-up", "Top", t)
            })
            .child(el().grow(1.0))
            .child(
                text(r.joint_text.clone())
                    .font(type_::LABEL)
                    .color(t.text_secondary),
            ),
    )
}

/// The full layers panel: a "Layers" header, one row per
/// figure in draw order (front on top), the selected
/// figure's opacity slider with its % readout, the
/// ordering note, and the visibility footer row
/// (`layers-vis-row`: Show all + Solo selected). With no
/// figures the panel is header + the kit empty state
/// (Wave 9; no slider — there is no selected figure to
/// fade) + note + footer; the footer presses are
/// harmless no-ops then.
pub fn panel(s: &State, t: &Theme) -> El {
    // The Solo selected button renders active while the
    // solo target is the selected figure (computed up
    // front — the panel body consumes `s` piece by piece
    // below only by reference, but the flag is cheapest
    // read once here).
    let solo_on_selected = s.core.solo == Some(s.core.selected_figure);
    // Card root + header (Phase N): the kit's `panel_col`
    // under its existing key, then `panel_title` in a
    // plain row — the header has never carried a key and
    // invents none now; it wears `header_row`'s layout
    // (full width, gap 8, centre-aligned) rather than the
    // keyed builder — over a kit hairline divider
    // (charter §2).
    let mut body = design::panel_col("layers-panel", t)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(design::panel_title(None, "Layers", None, t)),
        )
        .child(design::divider(t));
    for r in rows(&s.core) {
        body = body.child(layer_row(&r, t));
    }
    // Zero figures (Wave 9): the kit empty state fills
    // the rows area — the one bare gap the Wave 6 sweep
    // left. The opacity block below self-guards on a
    // selected figure existing, so nothing else in the
    // empty panel changes shape.
    if s.core.project.figures.is_empty() {
        // RM28 (PL-7): the state offers its filling
        // action — the figures panel's + Figure op.
        body = body.child(design::empty_state_action(
            "layer-group",
            EMPTY_MESSAGE,
            "layers-empty-add-figure",
            "Add figure",
            t,
            |s| s.core.add_figure(),
        ));
    }
    if let Some(fig) = s.core.project.figures.get(s.core.selected_figure) {
        body = body
            .child(
                row()
                    .key("layers-opacity-row")
                    .gap(6.0)
                    .center_y()
                    .child(design::section_label("Opacity", t))
                    .child(el().grow(1.0))
                    .child(
                        text(opacity_label(fig.opacity))
                            .font(type_::LABEL)
                            .color(t.text),
                    ),
            )
            .child(opacity_slider(opacity_frac(fig.opacity), t));
    }
    body.child(
        text(ORDER_NOTE)
            .font(type_::LABEL)
            .color(t.text_secondary),
    )
    .child(
        row()
            .key("layers-vis-row")
            .gap(6.0)
            .center_y()
            .child(btn(
                "layer-show-all",
                "Show all",
                false,
                t,
                |s: &mut State| show_all_figures(&mut s.core),
            ))
            .child(btn(
                "layer-solo-selected",
                "Solo selected",
                solo_on_selected,
                t,
                |s: &mut State| solo_selected(&mut s.core),
            )),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_order_is_front_first() {
        assert_eq!(display_order(3), vec![2, 1, 0]);
        assert_eq!(display_order(1), vec![0]);
        assert!(display_order(0).is_empty());
    }

    #[test]
    fn opacity_label_rounds_to_whole_percent() {
        assert_eq!(opacity_label(1.0), "100%");
        assert_eq!(opacity_label(0.6), "60%");
        assert_eq!(opacity_label(0.05), "5%");
        assert_eq!(opacity_pct(0.3), 30);
    }

    #[test]
    fn move_bounds_follow_draw_order() {
        // Back figure (0) can only move up; front figure (2 of
        // 3) can only move down; the middle moves both ways.
        assert!(!can_move_down(0, 3) && can_move_up(0, 3));
        assert!(can_move_down(1, 3) && can_move_up(1, 3));
        assert!(can_move_down(2, 3) && !can_move_up(2, 3));
        assert!(!can_move_up(0, 0) && !can_move_down(0, 0));
    }

    #[test]
    fn rows_are_front_first_with_selection_marked() {
        let mut core = EditorCore::new();
        core.project.add_figure("B", 0xFF0000, [200.0, 200.0]);
        core.project.add_figure("C", 0x00FF00, [300.0, 200.0]);
        core.select_figure(1);
        let rs = rows(&core);
        assert_eq!(rs.len(), 3);
        assert_eq!(rs[0].name, "C");
        assert_eq!(rs[0].fi, 2);
        assert!(!rs[0].can_move_up && rs[0].can_move_down);
        assert_eq!(rs[1].name, "B");
        assert!(rs[1].selected);
        assert_eq!(rs[2].fi, 0);
        assert!(rs[2].can_move_up && !rs[2].can_move_down);
        assert!(rs.iter().all(|r| r.opacity_pct == 100));
    }

    #[test]
    fn move_layer_reorders_and_selection_follows() {
        let mut core = EditorCore::new();
        core.project.add_figure("B", 0xFF0000, [200.0, 200.0]);
        let back_name = core.project.figures[0].name.clone();
        // Move the back figure up: it becomes the front figure
        // and the selection follows it to index 1.
        move_layer(&mut core, 0, 1);
        assert_eq!(core.project.figures[1].name, back_name);
        assert_eq!(core.selected_figure, 1);
        // Moving up again at the front is a no-op.
        move_layer(&mut core, 1, 1);
        assert_eq!(core.project.figures[1].name, back_name);
        // And back down restores the original order.
        move_layer(&mut core, 1, -1);
        assert_eq!(core.project.figures[0].name, back_name);
        assert_eq!(core.selected_figure, 0);
    }

    #[test]
    fn jump_target_covers_ends_middle_and_degenerate_lists() {
        // Ends and middle of a 3-figure list (Top = front =
        // highest index, Bottom = back = index 0).
        assert_eq!(jump_target(0, 3, true), Some(2));
        assert_eq!(jump_target(1, 3, true), Some(2));
        assert_eq!(jump_target(2, 3, true), None);
        assert_eq!(jump_target(2, 3, false), Some(0));
        assert_eq!(jump_target(1, 3, false), Some(0));
        assert_eq!(jump_target(0, 3, false), None);
        // Two figures: the only possible jumps.
        assert_eq!(jump_target(0, 2, true), Some(1));
        assert_eq!(jump_target(1, 2, false), Some(0));
        assert_eq!(jump_target(1, 2, true), None);
        assert_eq!(jump_target(0, 2, false), None);
        // Degenerate lists never jump.
        assert_eq!(jump_target(0, 1, true), None);
        assert_eq!(jump_target(0, 1, false), None);
        assert_eq!(jump_target(0, 0, true), None);
        // A stale index past the end never jumps either.
        assert_eq!(jump_target(3, 3, true), None);
        assert_eq!(jump_target(5, 3, false), None);
    }

    #[test]
    fn jump_layer_moves_to_the_ends_and_selection_follows() {
        let mut core = EditorCore::new();
        core.project.add_figure("B", 0xFF0000, [200.0, 200.0]);
        core.project.add_figure("C", 0x00FF00, [300.0, 200.0]);
        let back_name = core.project.figures[0].name.clone();
        // One press: the back figure lands at the front and
        // the op's own status names its new 1-based position.
        jump_layer(&mut core, 0, true);
        assert_eq!(core.project.figures[2].name, back_name);
        assert_eq!(core.selected_figure, 2);
        assert_eq!(core.status, "Figure moved to position 3.");
        // And one press back to the bottom restores the order.
        jump_layer(&mut core, 2, false);
        assert_eq!(core.project.figures[0].name, back_name);
        assert_eq!(core.selected_figure, 0);
        assert_eq!(core.status, "Figure moved to position 1.");
    }

    #[test]
    fn jump_layer_carries_figure_references_in_the_same_call() {
        // The jump goes through `layers::move_figure`, so the
        // references the engine tracks travel with their
        // figures — this pins that the panel's one call is
        // the full engine path, not the bare swap.
        let mut core = EditorCore::new();
        core.project.add_figure("B", 0xFF0000, [200.0, 200.0]);
        core.project.add_figure("C", 0x00FF00, [300.0, 200.0]);
        core.project.camera_follow = Some(2); // follows C
        core.project.channels.push(stykanimate_rs::graph::Channel {
            figure: 2, // on C
            target: stykanimate_rs::graph::ChannelTarget::RootX,
            keys: Vec::new(),
        });
        core.project.fx_clips.push(stykanimate_rs::model::FxClipEntry {
            clip: stykanimate_rs::vfx::ParticleClip {
                start: 0.0,
                duration: 8.0,
                kind: stykanimate_rs::vfx::ClipKind::Burst,
                color: 0xFFFFFF,
                seed: 1,
                angle: 0.0,
                length: 100.0,
                color2: None,
            },
            figure: 0, // on the jumping figure
            node: 2,
            hidden: false,
        });
        core.project.particles.push(stykanimate_rs::model::ParticleEvent {
            frame: 0,
            figure: Some(1), // on B
            node: Some(3),
            x: 0.0,
            y: 0.0,
            kind: stykanimate_rs::model::ParticleKind::Burst,
            color: 0xFFFFFF,
            seed: 0,
        });

        // Figure 0 jumps to the front (2); B and C shift down.
        jump_layer(&mut core, 0, true);
        assert_eq!(core.project.fx_clips[0].figure, 2);
        assert_eq!(core.project.particles[0].figure, Some(0));
        assert_eq!(core.project.channels[0].figure, 1);
        assert_eq!(core.project.camera_follow, Some(1));
    }

    #[test]
    fn jump_layer_is_status_only_when_already_at_the_end() {
        let mut core = EditorCore::new();
        core.project.add_figure("B", 0xFF0000, [200.0, 200.0]);
        core.project.add_figure("C", 0x00FF00, [300.0, 200.0]);
        let names_before: Vec<String> =
            core.project.figures.iter().map(|f| f.name.clone()).collect();
        core.select_figure(1);
        // The front figure is already at the top: no reorder,
        // no selection steal — only the status line changes.
        jump_layer(&mut core, 2, true);
        assert_eq!(core.status, "Already at the top.");
        assert_eq!(core.selected_figure, 1);
        // Same for the back figure and the bottom.
        jump_layer(&mut core, 0, false);
        assert_eq!(core.status, "Already at the bottom.");
        assert_eq!(core.selected_figure, 1);
        let names_after: Vec<String> =
            core.project.figures.iter().map(|f| f.name.clone()).collect();
        assert_eq!(names_before, names_after);
    }

    #[test]
    fn jump_layer_is_status_only_for_a_single_figure() {
        let mut core = EditorCore::new();
        assert_eq!(core.project.figures.len(), 1);
        jump_layer(&mut core, 0, true);
        assert_eq!(core.status, "Only one figure: nothing to reorder.");
        jump_layer(&mut core, 0, false);
        assert_eq!(core.status, "Only one figure: nothing to reorder.");
        assert_eq!(core.project.figures.len(), 1);
    }

    #[test]
    fn opacity_stepper_op_clamps_through_the_layers_engine() {
        let mut core = EditorCore::new();
        core.step_figure_opacity(0, -OPACITY_STEP);
        assert_eq!(opacity_label(core.project.figures[0].opacity), "90%");
        // Stepping above 1.0 clamps to fully opaque.
        core.step_figure_opacity(0, 5.0);
        assert_eq!(core.project.figures[0].opacity, 1.0);
    }

    #[test]
    fn opacity_at_maps_edges_middle_and_clamps() {
        assert_eq!(opacity_at(0.0, 200.0), 0.0);
        assert_eq!(opacity_at(200.0, 200.0), 1.0);
        assert_eq!(opacity_at(100.0, 200.0), 0.5);
        assert_eq!(opacity_at(50.0, 200.0), 0.25);
        assert_eq!(opacity_at(-30.0, 200.0), 0.0);
        assert_eq!(opacity_at(260.0, 200.0), 1.0);
        // A zero-width track maps to 0, never NaN.
        assert_eq!(opacity_at(10.0, 0.0), 0.0);
    }

    #[test]
    fn opacity_frac_is_the_inverse_of_opacity_at() {
        for x in [0.0, 25.0, 50.0, 100.0, 150.0, 200.0] {
            assert_eq!(opacity_frac(opacity_at(x, 200.0)), x / 200.0);
        }
        // Out-of-range stored values clamp onto the track.
        assert_eq!(opacity_frac(1.4), 1.0);
        assert_eq!(opacity_frac(-0.2), 0.0);
        assert_eq!(opacity_frac(0.05), 0.05);
    }

    #[test]
    fn slider_set_writes_percent_grid_and_reports_only_on_release() {
        let mut core = EditorCore::new();
        core.status = "untouched".into();
        set_figure_opacity(&mut core, 0, 0.456, false);
        // Same whole-percent grid the stepper writes.
        assert_eq!(core.project.figures[0].opacity, 0.46);
        assert_eq!(core.status, "untouched", "mid-drag writes stay silent");
        set_figure_opacity(&mut core, 0, 0.5, true);
        assert_eq!(core.project.figures[0].opacity, 0.5);
        assert_eq!(core.status, "Figure 1 opacity 50%.");
        // The layers floor and ceiling apply on release too.
        set_figure_opacity(&mut core, 0, 0.0, true);
        assert_eq!(core.project.figures[0].opacity, 0.05);
        assert_eq!(core.status, "Figure 1 opacity 5%.");
        set_figure_opacity(&mut core, 0, 2.0, true);
        assert_eq!(core.project.figures[0].opacity, 1.0);
    }

    #[test]
    fn slider_set_matches_the_stepper_ops_status_and_grid() {
        // The release report is the op's own string: drive
        // both write paths to the same value and compare.
        let mut via_op = EditorCore::new();
        via_op.step_figure_opacity(0, -0.5);
        let mut via_slider = EditorCore::new();
        set_figure_opacity(&mut via_slider, 0, 0.5, true);
        assert_eq!(
            via_slider.project.figures[0].opacity,
            via_op.project.figures[0].opacity
        );
        assert_eq!(via_slider.status, via_op.status);
    }

    #[test]
    fn slider_set_is_a_silent_no_op_for_a_missing_figure() {
        let mut core = EditorCore::new();
        core.status = "untouched".into();
        set_figure_opacity(&mut core, 9, 0.5, false);
        assert_eq!(core.status, "untouched");
        set_figure_opacity(&mut core, 9, 0.5, true);
        assert_eq!(core.status, "No such figure.");
        assert_eq!(core.project.figures.len(), 1);
    }

    #[test]
    fn show_all_figures_unhides_counts_changes_and_clears_solo() {
        let mut core = EditorCore::new();
        core.project.add_figure("B", 0xFF0000, [200.0, 200.0]);
        core.project.add_figure("C", 0x00FF00, [300.0, 200.0]);
        // Hide two of the three figures directly through
        // the layers engine, and park a solo on figure 1 —
        // the stale-solo case the handler must clear.
        core.project.figures[0].hidden = true;
        core.project.figures[2].hidden = true;
        core.solo = Some(1);
        let depth = core.undo_depth();
        show_all_figures(&mut core);
        assert!(core.project.figures.iter().all(|f| !f.hidden));
        assert_eq!(core.solo, None);
        assert_eq!(core.status, "All figures shown (2 changed).");
        // RM24: the bulk write is exactly one undo step.
        assert_eq!(core.undo_depth(), depth + 1);
        // A second press changes nothing and says so —
        // and spends nothing.
        show_all_figures(&mut core);
        assert_eq!(core.status, "All figures shown (0 changed).");
        assert!(core.project.figures.iter().all(|f| !f.hidden));
        assert_eq!(core.undo_depth(), depth + 1);
    }

    #[test]
    fn show_all_figures_matches_the_layers_ops_count() {
        // The reported count is the op's own: hiding every
        // figure through set_all_visible makes Show all
        // report the full figure count.
        let mut core = EditorCore::new();
        core.project.add_figure("B", 0xFF0000, [200.0, 200.0]);
        let hidden = stykanimate_rs::layers::set_all_visible(&mut core.project, false);
        assert_eq!(hidden, 2);
        show_all_figures(&mut core);
        assert_eq!(core.status, "All figures shown (2 changed).");
    }

    #[test]
    fn solo_selected_uses_the_ops_status_and_toggles() {
        let mut core = EditorCore::new();
        core.project.add_figure("B", 0xFF0000, [200.0, 200.0]);
        core.project.add_figure("C", 0x00FF00, [300.0, 200.0]);
        core.select_figure(2);
        // Engaging: the op's own string, with the 1-based
        // figure number — not the figure name.
        solo_selected(&mut core);
        assert_eq!(core.solo, Some(2));
        assert_eq!(core.status, "Solo: figure 3.");
        // Pressing again on the same figure is the op's
        // toggle-off, exactly like the figures panel's
        // per-row solo buttons.
        solo_selected(&mut core);
        assert_eq!(core.solo, None);
        assert_eq!(core.status, "Solo off.");
        // A different selection solos that figure instead.
        core.select_figure(0);
        solo_selected(&mut core);
        assert_eq!(core.solo, Some(0));
        assert_eq!(core.status, "Solo: figure 1.");
    }

    #[test]
    fn solo_selected_is_a_safe_no_op_with_no_figures() {
        let mut core = EditorCore::new();
        core.project.figures.clear();
        core.status = "untouched".into();
        solo_selected(&mut core);
        assert_eq!(core.solo, None);
        assert_eq!(core.status, "untouched");
    }

    #[test]
    fn layer_dot_color_prefers_the_root_nodes_explicit_colour() {
        let mut core = EditorCore::new();
        // The demo figure's own colour is 0xE8E8EC; an
        // explicit root-node colour overrides it for the
        // dot, exactly as it would on the stage.
        core.project.figures[0].def.nodes[0].color = Some(0x123456);
        assert_eq!(layer_dot_color(&core.project.figures[0]), 0x123456);
        // ...and the built row carries the same colour.
        assert_eq!(rows(&core)[0].dot_color, 0x123456);
    }

    #[test]
    fn layer_dot_color_falls_back_to_the_figure_colour() {
        let core = EditorCore::new();
        // The demo root node inherits (its colour is
        // None), so the dot is the figure colour itself.
        assert_eq!(core.project.figures[0].def.nodes[0].color, None);
        assert_eq!(
            layer_dot_color(&core.project.figures[0]),
            core.project.figures[0].color
        );
        assert_eq!(rows(&core)[0].dot_color, 0xE8E8EC);
    }

    #[test]
    fn layer_dot_color_without_nodes_is_the_figure_colour() {
        let mut core = EditorCore::new();
        core.project.figures[0].def.nodes.clear();
        assert_eq!(
            layer_dot_color(&core.project.figures[0]),
            core.project.figures[0].color
        );
    }

    #[test]
    fn joint_label_counts_the_demo_figures_nodes() {
        // The expected count comes from the fixture
        // itself, not a hard-coded number: the label is
        // the demo figure's real def node count, plural.
        let core = EditorCore::new();
        let fig = &core.project.figures[0];
        let n = fig.def.nodes.len();
        assert!(n > 1, "the demo figure is not a one-joint figure");
        assert_eq!(joint_label(fig), format!("{n} joints"));
        // ...and the built row carries the same text.
        assert_eq!(rows(&core)[0].joint_text, format!("{n} joints"));
    }

    #[test]
    fn joint_label_singular_for_a_one_node_figure() {
        // The dot tests' own trick: shrink the demo def
        // to a single node rather than fabricating a
        // FigureInstance literal.
        let mut core = EditorCore::new();
        core.project.figures[0].def.nodes.truncate(1);
        assert_eq!(core.project.figures[0].def.nodes.len(), 1);
        assert_eq!(joint_label(&core.project.figures[0]), "1 joint");
        assert_eq!(rows(&core)[0].joint_text, "1 joint");
    }

    #[test]
    fn joint_label_zero_for_an_empty_def() {
        let mut core = EditorCore::new();
        core.project.figures[0].def.nodes.clear();
        assert_eq!(joint_label(&core.project.figures[0]), "0 joints");
    }

    #[test]
    fn layer_rows_build_on_the_demo() {
        // The row builder — the element that carries the
        // new dot — builds for every row of the demo
        // project (panel() itself needs a full shell
        // State; the rows are its only per-figure input).
        let core = EditorCore::new();
        let t = tok::theme(true);
        let rs = rows(&core);
        assert_eq!(rs.len(), core.project.figures.len());
        for r in &rs {
            let _ = layer_row(r, &t);
        }
    }

    #[test]
    fn layer_rows_construct_at_the_opacity_bounds() {
        // Wave Q: the opacity pair builds in its
        // inert branches (0% dec, 100% inc) and at a
        // live mid value, under both themes.
        for dark in [true, false] {
            let t = tok::theme(dark);
            for opacity in [0.0, 0.5, 1.0] {
                let mut core = EditorCore::new();
                core.project.figures[0].opacity = opacity;
                let rs = rows(&core);
                let _ = layer_row(&rs[0], &t);
            }
        }
    }

    #[test]
    fn slider_builds_at_the_track_ends() {
        let t = tok::theme(true);
        let _ = opacity_slider(0.0, &t);
        let _ = opacity_slider(0.05, &t);
        let _ = opacity_slider(1.0, &t);
    }

    #[test]
    fn rows_carry_hidden_and_jump_reachability() {
        let mut core = EditorCore::new();
        core.project.add_figure("B", 0xFF0000, [200.0, 200.0]);
        core.project.add_figure("C", 0x00FF00, [300.0, 200.0]);
        core.project.figures[0].hidden = true;
        let rs = rows(&core);
        // Front figure (fi 2, first row): a Bottom jump
        // would move it; a Top jump is already there —
        // the flags the inert twins key off.
        assert!(rs[0].can_jump_bottom && !rs[0].can_jump_top);
        assert!(!rs[0].hidden);
        // Back figure (fi 0, last row): the inverse, and
        // its hidden flag is carried onto the row.
        assert!(rs[2].can_jump_top && !rs[2].can_jump_bottom);
        assert!(rs[2].hidden);
        // A single figure can jump neither way, so both
        // of its jump buttons render inert.
        let single = rows(&EditorCore::new());
        assert_eq!(single.len(), 1);
        assert!(!single[0].can_jump_top && !single[0].can_jump_bottom);
    }

    // ---- UI mission Wave 9: the zero-figures empty state ----

    #[test]
    fn empty_message_names_the_real_next_action() {
        // The message points at the Figures area's
        // "+ Figure" button — the control that actually
        // creates a figure, mounted above this panel in
        // the Animate workspace (the only workspace whose
        // section set includes Layers). It must not name
        // a control this panel does not have.
        assert!(EMPTY_MESSAGE.contains("+ Figure"));
        assert!(EMPTY_MESSAGE.starts_with("No figures yet"));
        // The glyph the empty state renders is in the
        // curated set (design::empty_state silently drops
        // an unknown icon, which would hide the mistake).
        assert!(crate::rune_sec::icons::has_icon("layer-group"));
    }

    #[test]
    fn panel_builds_with_no_figures() {
        // The Wave 9 state: the predicate the builder
        // branches on is the project's figure list
        // itself, and the full panel — header, empty
        // state, note, visibility footer — builds on an
        // empty project without panicking (the footer's
        // handlers are the guarded no-ops pinned above).
        let mut s = crate::init();
        s.core.project.figures.clear();
        assert!(s.core.project.figures.is_empty());
        assert!(rows(&s.core).is_empty());
        let t = tok::theme(true);
        let _ = panel(&s, &t);
        let t = tok::theme(false);
        let _ = panel(&s, &t);
    }

    #[test]
    fn panel_builds_with_figures_present() {
        // The non-empty path is untouched by the Wave 9
        // branch: the demo project (one figure) builds
        // the panel in both themes, rows and slider
        // included.
        let s = crate::init();
        assert!(!s.core.project.figures.is_empty());
        let t = tok::theme(true);
        let _ = panel(&s, &t);
        let t = tok::theme(false);
        let _ = panel(&s, &t);
    }
}
