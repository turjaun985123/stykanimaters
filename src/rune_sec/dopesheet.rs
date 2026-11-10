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

use rune::tok::{type_, Theme};
use rune::{col, el, row, text, Color, El};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::SpriteInstance;
use stykanimate_rs::stats::figure_keyframes;

use crate::rune_sec::design;
use crate::rune_sec::fx_browser::preset_label_width;
use crate::rune_sec::timeline_marks::{CHIP_GAP, CHIP_W};
use crate::stage_view::hex_color;
use crate::State;

/// Height of the lane's header row ("Dopesheet" +
/// the frame readout): a slim strip in the ruler's
/// spirit ([`crate::rune_sec::timeline_marks::RULER_H`]
/// is 12), just tall enough for one LABEL line plus
/// air.
pub const DOPE_HEADER_H: f32 = 18.0;

/// Height of one figure row — header cell and frame
/// cells alike: compact like the marker row (14) but
/// tall enough for the 12 pt diamond and a real press
/// target.
pub const DOPE_ROW_H: f32 = 20.0;

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

/// The total width of one figure row (and of the
/// header row, which mirrors the row's box so its
/// readout can sit over the last frame column): the
/// header cell, one chip gap, then one chip slot per
/// frame at the chips' pitch. Zero frames is just the
/// header cell.
pub fn lane_width(frame_count: usize) -> f32 {
    if frame_count == 0 {
        return DOPE_HEADER_W;
    }
    DOPE_HEADER_W
        + CHIP_GAP
        + frame_count as f32 * CHIP_W
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
        header = header.size(lane_width(frame_count), DOPE_HEADER_H);
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

    // The sprite section (Wave X, X5) renders on its own
    // condition — instances exist — not on the figure
    // sheet's: a project can carry sprites before it has
    // a single figure key, and the lane is their
    // navigation surface too. The two empty states below
    // therefore apply only when NEITHER section has
    // anything to show.
    let sprite_rows = sprite_dope_rows(core, s.sprites_panel.selected);
    let figures_keyed = any_keyframes(core);

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
            lane = lane.child(figure_row(r, core, t));
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
            lane = lane.child(sprite_row(r, core, t));
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
fn figure_row(r: &DopeRow, core: &EditorCore, t: &Theme) -> El {
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
        .on_press(move |s: &mut State| s.core.select_figure(fi))
        .child(el().size(10.0, 10.0).radius(5.0).fill(dot))
        .child(
            text(fit_dope_name(&r.name))
                .font(type_::LABEL)
                .truncate()
                .color(if r.hidden { t.text_secondary } else { t.text }),
        );

    let mut cells = row().gap(CHIP_GAP).child(header);
    for frame in 0..core.frame_count() {
        let mut cell = el()
            .key(format!("dope-cell-{fi}-{frame}"))
            .size(CHIP_W, DOPE_ROW_H)
            .radius(design::CONTROL_RADIUS)
            .center()
            .hover(t.control_track)
            .press(t.press)
            .on_press(move |s: &mut State| {
                s.core.select_figure(fi);
                s.core.goto(frame);
            });
        if frame == core.current {
            cell = cell.fill(design::selected_fill());
        }
        if is_key(&r.keys, frame) {
            cell = cell.child(text("◆").font(type_::LABEL).color(design::accent()));
        }
        cells = cells.child(cell);
    }

    if r.selected {
        cells.fill(design::selected_fill()).radius(design::ROW_RADIUS)
    } else {
        cells
    }
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
fn sprite_row(r: &SpriteDopeRow, core: &EditorCore, t: &Theme) -> El {
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
        .child(el().size(10.0, 10.0).radius(2.5).fill(design::accent()))
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
            .size(CHIP_W, DOPE_ROW_H)
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
            cell = cell.child(text("◆").font(type_::LABEL).color(design::accent()));
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
    use stykanimate_rs::model::{
        FigureDef, FigureInstance, Frame, ImageAsset, InstanceKey, Pose, SpriteInstance,
    };

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
    fn frame_readout_is_one_based_and_clamped() {
        assert_eq!(frame_readout(0, 5), "Frame 1 / 5");
        assert_eq!(frame_readout(4, 5), "Frame 5 / 5");
        assert_eq!(frame_readout(9, 5), "Frame 5 / 5");
        assert_eq!(frame_readout(0, 0), "Frame 0 / 0");
    }

    #[test]
    fn lane_width_matches_the_row_geometry() {
        assert_eq!(lane_width(0), DOPE_HEADER_W);
        assert_eq!(lane_width(1), DOPE_HEADER_W + CHIP_GAP + CHIP_W);
        assert_eq!(
            lane_width(5),
            DOPE_HEADER_W + CHIP_GAP + 5.0 * CHIP_W + 4.0 * CHIP_GAP
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
            // Past the row cap, selected pinned.
            let s = many_figure_state(7);
            let _ = dopesheet_lane(&s, &t);
            // The two-figure keyframe sheet.
            let s = two_figure_state();
            let _ = dopesheet_lane(&s, &t);
        }
    }
}
