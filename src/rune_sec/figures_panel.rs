//! Figures panel (Phase D, section 1/40 of the Rune rebuild;
//! completed in Phase E, section 29/40): the figures sidebar —
//! one row per figure with a thumbnail spanning two control
//! lines beside it: colour dot, name, opacity readout and
//! lock/solo toggles on the first line, and reorder (up/down),
//! opacity-cycle, eye, delete and the opacity steppers on the
//! second (the steppers moved down in the UI-mission S07-1
//! fix so the name has room to read), a header with the
//! figure count and "+ Figure", and a footer bulk visibility
//! row (All / None / Invert).
//!
//! Direction convention for reorder (shared with the layers
//! panel, `rune_sec::layers_panel::move_layer`): "up" means
//! toward the front of the draw stack — a higher `figures`
//! index, drawn later/on top — and dispatches
//! `move_figure_full(+1)`; "down" is toward the back (index 0)
//! and dispatches `move_figure_full(-1)`. The figures list
//! itself is shown in project order, so an "up" press moves
//! the row one slot later in this list while moving the figure
//! one slot toward the front of the stack.
//!
//! Every control calls the same real op the GPUI sidebar's
//! listener called (ids and listener bodies were recovered from
//! `git show a5e0b18:src/app.rs`; the per-figure GPUI tuple ids
//! `("fig-sel", fi)` become Rune keys `fig-sel-{fi}`, matching
//! the `frame-{n}` key style `chrome_view` already uses).
//! Declared from `main.rs` only — bin-crate code, not part of
//! the engine library.
//!
//! Required shell state: `State` must gain
//! `pub figures_panel: FiguresPanelState` (see the struct
//! below); the panel builder reads it and the header toggle
//! writes it. Nothing else in `State` is touched — all figure
//! state lives in `EditorCore`, as before.
//!
//! Phase F (section 18/40) adds a live thumbnail to each
//! row's first line: a [`THUMB_SIZE`]-point canvas drawing
//! the figure's rig at the current display frame — the same
//! `EditorCore::positions_f` evaluation the stage draws —
//! fitted into the box by [`thumb_fit`] and built by the
//! pure [`thumb_shape`] helper. The thumbnail is an identity
//! preview, so it draws every figure in its own colour
//! regardless of hidden/solo state (the row's name dimming
//! and eye toggle already carry visibility).
//!
//! Wave 12 (D6 audit) makes the miniature's SEGMENT colours
//! per-node: each segment carries its child node's resolved
//! colour (`NodeDef::color` else the figure colour — the
//! stage's own resolution in `stage_view`), so a figure
//! whose def styles individual segments shows those colours
//! on its card, not one flat figure colour. Head rings keep
//! the figure colour, exactly as the stage draws them (its
//! ring pass uses the figure colour, never the head node's
//! override).
//!
//! Wave 13 (C4) rebuilds the miniature's geometry on the
//! shared construction, closing the D6 remainder: every
//! segment now goes through `paint::styled_segment_pieces`
//! — the same generator the stage, the raster exporter
//! and the Wave 12 library thumbnails build from — so
//! per-node taper, cap geometry, `color2` gradients and
//! style opacity reach the card, a bent segment arcs
//! through its bend midpoint, and Circle/Polygon nodes
//! draw their glyphs (the stage's shape pass). Positions
//! stay the LIVE pose (`EditorCore::positions_f` at the
//! display frame, with that frame's bends) — the library
//! thumbnail's rest-pose choice is deliberately NOT
//! imported, because a project figure has a live pose to
//! show. The base colour each prim family resolves
//! against is the figure's own colour: a library def
//! carries no figure colour, so its thumbnails resolve
//! inheritance against the theme's text colour, but a
//! project figure HAS a live colour, and segments and
//! shape glyphs alike resolve `node.color` else
//! `fig.color` — the stage's own resolution. Head rings
//! keep the D6 doctrine (the figure colour plain, never
//! the head node's override). Prim alphas carry the
//! node's style opacity only: the figure's layer opacity
//! is NOT composed into the card — the thumbnail is an
//! identity preview (above), the row's opacity readout
//! already carries that number, and hidden/solo figures
//! are likewise drawn undimmed. Piece and ring widths
//! are the stage widths scaled by the fit, floored at
//! [`THUMB_MIN_WIDTH`] so a hard taper stays visible at
//! 44 points (the library thumbnail's floor, and the
//! stage's own dot-floor precedent).
//!
//! Phase G (section 17/40) adds a name filter: a `TextEdit`
//! in [`FiguresPanelState::filter`] (key `figures-filter`,
//! directly under the header) narrows the list through the
//! pure [`visible_figures`] helper — case-insensitive
//! substring on the figure name, query trimmed. Filtering
//! is view-only: rows are still built from the real figure
//! index end-to-end, so every row control and thumbnail
//! acts on the figure it shows. With a filter active the
//! header count reads "{shown} of {total} figures" (see
//! [`count_line`]); with none it is the bare total, exactly
//! as before. A filter that hides everything shows a
//! "No figures match" line instead of rows.
//!
//! Phase H (section 8/40) switches the row's unambiguous
//! micro-buttons to Font Awesome glyphs through
//! `icon_btn::icon_only_btn`: the lock toggle (`lock` when
//! locked, `lock-open` when not), the visibility toggle
//! (`eye` when shown, `eye-slash` when hidden), reorder
//! (`chevron-up` / `chevron-down`) and delete
//! (`trash-can`), and puts a `magnifying-glass` glyph
//! before the filter field. Keys and handlers are
//! unchanged. Solo, the opacity steppers (`−` / `+`) and
//! the opacity-cycle button (`opac`) stay text: the icon
//! map names no glyph for them, and a cycle action is not
//! a state a single static glyph would carry honestly.
//!
//! Phase I (section 20/40) adds one header control: a
//! "Recolour" button (key `fig-recolor`, `paintbrush`
//! glyph via `icon_btn`) that paints the WHOLE selected
//! figure with the colour its selected joint resolves to,
//! through `EditorCore::recolor_figure` (figure_ops.rs —
//! one snapshot, per-node `color2` untouched, the op sets
//! its own status). The colour decision lives in the pure
//! [`recolor_source_color`] helper; the press dispatch is
//! [`recolor_row`]. With no figure or no joint selected the
//! press is silent — no status, no snapshot — the guarded
//! style the editor's own selection ops use (e.g.
//! `cycle_node_color` returns silently with no selection).
//!
//! Phase L (section 12/40) makes the thumbnail's head ring
//! follow the project: [`thumb_prims`] reads the radius from
//! `Project::head_ring_radius()` (the project's head scale
//! applied to the base constant) and [`thumb_fit`] takes it
//! as a parameter, so a project whose heads are scaled draws
//! miniature rings — and fit room — at the same proportion
//! as its stage. At the default scale the radius is exactly
//! [`HEAD_RING_R`] and the geometry is unchanged.
//!
//! Phase M (section 21/40) adds a "Duplicate" button to the
//! header (key `fig-duplicate`, `copy` glyph via
//! `icon_btn`), beside Recolour: it duplicates the selected
//! figure through `EditorCore::duplicate_figure`
//! (figure_ops.rs — one snapshot, per-frame poses and bend
//! rows cloned, every figure-indexed reference remapped).
//! The op names the copy "<name> copy", selects it itself
//! (with no node selected) and sets its own status, so the
//! dispatch in [`duplicate_row`] adds neither. The button
//! is always live like Recolour, and with no figure at the
//! selected index — an empty project, a stale selection —
//! the press is silent: the dispatch resolves the figure
//! first, so no status is set and nothing is snapshotted.
//!
//! Phase N (adoption) restyles the panel onto the design
//! kit (`rune_sec::design`): the root is a `panel_col`
//! card, the header is a kit `header_row` + `panel_title`
//! (title in SECTION, the count as its meta) over a
//! hairline `divider`, each row root carries the two-line
//! list-row anatomy (px 8, `ROW_RADIUS`, selected =
//! `selected_fill()`, hover `t.hover`), the private
//! `row_toggle` helper is retired onto `icon_btn::
//! chip_btn` (the kit chip that absorbed its metrics),
//! the opacity readout moves to MONO_CAPTION, the filter
//! glyph takes the kit's 13px search size, and the
//! filter-no-match line becomes a kit `empty_state` with
//! its message byte-identical. Keys, handlers and status
//! strings are unchanged throughout.
//!
//! Wave 10 (round-2 audit) bounds the figure list: the
//! rows now ride in their own scroll viewport
//! (`figures-list`, capped at [`FIG_LIST_MAX_H`] — five
//! rows) instead of stacking unbounded in the card. This
//! was the shell's last unbounded list: the library list
//! (fixed-height scroll region), the graph channel list
//! (Wave 9, audit S12-07) and the FX clip list all bound
//! theirs, while here ten figures measured ≈770 pt of
//! panel — taller than the whole left-column viewport —
//! pushing the panel's own bulk footer a full screen
//! below its header and every panel under it (figure
//! add, library, layers) as far down the column scroll.
//! Lists of five or fewer figures lay out byte-identical
//! to before (the cap only bites past five); the rows'
//! keys and handlers are unchanged, and the no-match
//! empty state still replaces the list outright, the
//! library precedent. The same audit verified the rest
//! of the panel clean: every row state field is derived
//! fresh from the core in [`figure_rows`] at render (the
//! panel's own state holds only collapse + filter text,
//! so no toggle can go stale), the selected row reads
//! `core.selected_figure` — the same field the Wave 10
//! dopesheet lane's cell presses write — and the one
//! unbounded user string left in the card, the filter
//! query echoed by the no-match line, goes through the
//! kit `empty_state`, which wraps.

use rune::tok::{type_, Theme};
use rune::{canvas, col, el, row, text, Color, El, LineCap, ScrollSpec, Shape, TextEdit};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::{self, V2};

use crate::rune_sec::design;
use crate::rune_sec::i18n_wire;
use crate::rune_sec::icon_btn::{chip_btn, chip_btn_inert, icon_btn, icon_only_btn};
use crate::rune_sec::icons::icon_el;
use crate::stage_view::{hex_color, ngon};
use crate::State;

/// View-local state for the figures panel. The engine tracks
/// everything about the figures themselves; the shell owns
/// only the panel's collapse flag and the name-filter text.
/// The derived `Default` covers both (`TextEdit` defaults
/// to empty), so `main.rs`'s `Default::default()` init is
/// unchanged.
#[derive(Clone, Debug, Default)]
pub struct FiguresPanelState {
    /// When true, only the header renders (rows and the bulk
    /// footer are omitted). Toggled by the header's Hide/Show
    /// control — a pure view-state change, no engine op.
    pub collapsed: bool,
    /// Name-filter box contents (Rune core text edit). The
    /// row list filters through [`visible_figures`] on this
    /// text every render; typing is the only writer.
    /// Filtering never touches the project.
    pub filter: TextEdit,
}

/// Display-ready data for one figure row, derived from the
/// core in one pass so the builder never re-reads the project
/// per element.
#[derive(Clone, Debug, PartialEq)]
pub struct FigureRow {
    /// Figure index (the `fi` every per-figure op takes).
    pub index: usize,
    pub name: String,
    /// Figure colour as `0xRRGGBB` (the row's dot).
    pub color: u32,
    pub hidden: bool,
    pub locked: bool,
    /// This figure is the solo target (`core.solo == Some(fi)`).
    pub solo: bool,
    /// This figure is `core.selected_figure`.
    pub selected: bool,
    /// Opacity as a whole percentage (0–100).
    pub opacity_pct: i32,
}

/// Whole-percentage opacity for display (`1.0 → 100`).
pub fn opacity_pct(opacity: f32) -> i32 {
    (opacity.clamp(0.0, 1.0) * 100.0).round() as i32
}

/// One [`FigureRow`] per figure in the project, in project
/// order (the sidebar's top-to-bottom order).
pub fn figure_rows(core: &EditorCore) -> Vec<FigureRow> {
    core.project
        .figures
        .iter()
        .enumerate()
        .map(|(fi, fig)| FigureRow {
            index: fi,
            name: fig.name.clone(),
            color: fig.color,
            hidden: fig.hidden,
            locked: fig.locked,
            solo: core.solo == Some(fi),
            selected: fi == core.selected_figure,
            opacity_pct: opacity_pct(fig.opacity),
        })
        .collect()
}

/// True when the filter `query` actually narrows the list:
/// non-empty once trimmed. A whitespace-only query is no
/// filter at all.
pub fn filter_active(query: &str) -> bool {
    !query.trim().is_empty()
}

/// True when a figure `name` matches the filter `query`:
/// case-insensitive substring; an empty (or whitespace-
/// only) query matches everything. Mirrors
/// `library_panel::name_matches`, the shell's other search.
pub fn figure_name_matches(name: &str, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    q.is_empty() || name.to_lowercase().contains(&q)
}

/// The real indices of the figures the filter `query`
/// leaves visible, in project order. An empty query yields
/// every index. The result indexes `core.project.figures`
/// directly — row controls built from these indices act on
/// the figure the row shows, filtered or not.
pub fn visible_figures(core: &EditorCore, query: &str) -> Vec<usize> {
    core.project
        .figures
        .iter()
        .enumerate()
        .filter(|(_, fig)| figure_name_matches(&fig.name, query))
        .map(|(fi, _)| fi)
        .collect()
}

/// The header count line: with a filter active,
/// "{shown} of {total} figures"; with no filter, the bare
/// total — byte-identical to the count the header showed
/// before the filter existed.
pub fn count_line(shown: usize, total: usize, query: &str) -> String {
    if filter_active(query) {
        format!("{shown} of {total} figures")
    } else {
        format!("{total}")
    }
}

/// Edge length (points) of a figure row's thumbnail canvas.
pub const THUMB_SIZE: f32 = 44.0;

/// Inset (points) kept between the fitted silhouette and the
/// thumbnail's edge.
const THUMB_MARGIN: f32 = 4.0;

/// Legibility floor (points) for thumbnail stroke
/// widths: a hard taper's far pieces would otherwise
/// shrink under a fifth of a point at this scale and
/// the segment would visually detach from its joint.
/// The library thumbnail's floor (`library_panel`'s
/// `THUMB_MIN_WIDTH`), and the stage floors its spray
/// dots at the same 0.8; endpoints — and every width
/// above the floor — stay exact.
pub const THUMB_MIN_WIDTH: f32 = 0.8;

/// Base head-ring radius in stage units, re-exported under
/// the panel's long-standing name: the single source is the
/// model's `HEAD_RING_RADIUS`. Since Phase L the thumbnails
/// themselves read the project's scaled radius through
/// `Project::head_ring_radius()`; this name stays as the
/// unscaled base value (and other panels keep their own
/// re-exports of the same constant).
pub use stykanimate_rs::model::HEAD_RING_RADIUS as HEAD_RING_R;

/// Sides of the thumbnail head-ring polygon (the stage uses
/// 24 at full size; 16 is smooth at 44 points).
const HEAD_RING_SIDES: usize = 16;

/// One piece of thumbnail geometry in canvas points.
/// `thumb_shape` turns these into a `Shape`; keeping them
/// separate makes the geometry unit-testable (a built
/// `Shape`'s ops are crate-private to rune) — the library
/// thumbnail's `LibraryThumbPrim` discipline, carried
/// over verbatim (Wave 13, C4). Each prim carries the
/// width, colour and alpha it paints at: a styled piece
/// takes them from `paint::styled_segment_pieces` (its
/// colour resolved against the figure colour), a shape
/// ring/disc takes the node's resolved colour at the
/// node's style opacity, and a head ring takes the
/// figure colour plain — the stage's own choices, with
/// the D6 ring doctrine (Wave 12) preserved.
#[derive(Clone, Debug, PartialEq)]
pub enum ThumbPrim {
    /// One styled segment piece: draw `a`→`b` as a
    /// round-capped line of `width` points in `color`
    /// at `alpha` (the piece's style opacity).
    Piece {
        a: [f32; 2],
        b: [f32; 2],
        width: f32,
        color: Color,
        alpha: f32,
    },
    /// A closed ring — a head ring or a `Circle` shape
    /// node: stroke `pts` closed at `width` points in
    /// `color` at `alpha`.
    Ring {
        pts: Vec<[f32; 2]>,
        width: f32,
        color: Color,
        alpha: f32,
    },
    /// A filled disc — a `Polygon` shape node: fill
    /// `pts` in `color` at `alpha`.
    Disc {
        pts: Vec<[f32; 2]>,
        color: Color,
        alpha: f32,
    },
}

/// The bounding-box fit of a thumbnail: stage points map to
/// canvas points centred in the `size` square.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThumbFit {
    /// Canvas points per stage unit.
    pub scale: f32,
    /// Stage-space centre of the fitted box.
    pub centre: [f32; 2],
    /// Canvas edge length the fit targets.
    pub size: f32,
}

impl ThumbFit {
    /// Stage point → canvas point under this fit.
    pub fn map(&self, p: V2) -> [f32; 2] {
        [
            (p[0] - self.centre[0]) * self.scale + self.size / 2.0,
            (p[1] - self.centre[1]) * self.scale + self.size / 2.0,
        ]
    }
}

/// Fit the bounding box of `positions` into a `size` square,
/// keeping [`THUMB_MARGIN`] to spare on the limiting axis.
/// When the figure has a head node the box first grows by
/// `head_r` — the project's head-ring radius, passed in by
/// the caller (Phase L) — on every side, so the ring stays
/// inside the canvas instead of clipping at the box edge.
/// Degenerate boxes (one point, a straight vertical line)
/// clamp their extents to 1 stage unit so the scale stays
/// finite and the content lands centred.
pub fn thumb_fit(positions: &[V2], has_head: bool, size: f32, head_r: f32) -> ThumbFit {
    let mut lo = [f32::INFINITY; 2];
    let mut hi = [f32::NEG_INFINITY; 2];
    for p in positions {
        lo = [lo[0].min(p[0]), lo[1].min(p[1])];
        hi = [hi[0].max(p[0]), hi[1].max(p[1])];
    }
    if positions.is_empty() {
        lo = [0.0, 0.0];
        hi = [0.0, 0.0];
    }
    if has_head {
        lo = [lo[0] - head_r, lo[1] - head_r];
        hi = [hi[0] + head_r, hi[1] + head_r];
    }
    let extents = [(hi[0] - lo[0]).max(1.0), (hi[1] - lo[1]).max(1.0)];
    let avail = (size - 2.0 * THUMB_MARGIN).max(1.0);
    ThumbFit {
        scale: (avail / extents[0]).min(avail / extents[1]),
        centre: [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0],
        size,
    }
}

/// The thumbnail geometry for figure `fi` at the current
/// display frame, fitted into the `size` box by
/// [`thumb_fit`]: one styled piece run per parented node,
/// then the shape-node rings/discs, then one head ring
/// per head node — the stage's own draw order. Positions
/// come from `EditorCore::positions_f` at
/// `EditorCore::display_frame` — the stage's own
/// evaluation, graph channels included — and each
/// segment takes that frame's bend for its node
/// (`Project::bends_at`, the stage's own fetch). The
/// figure's hidden/solo state is never consulted: the
/// thumbnail is an identity preview (see the module
/// docs). Empty when `fi` is out of range or the figure
/// has no nodes.
pub fn thumb_prims(core: &EditorCore, fi: usize, size: f32) -> Vec<ThumbPrim> {
    let Some(fig) = core.project.figures.get(fi) else {
        return Vec::new();
    };
    if fig.def.nodes.is_empty() {
        return Vec::new();
    }
    let display = core.display_frame();
    let pos = core.positions_f(display, fi);
    let bends = core.project.bends_at(display, fi);
    // Phase L: the ring — and the room the fit leaves for
    // it — follow the project's head scale, not the bare
    // base constant.
    let head_r = core.project.head_ring_radius();
    thumb_prims_for(fig, &pos, &bends, head_r, size)
}

/// The prim construction behind [`thumb_prims`], split
/// out over an explicit figure, joint positions, bends
/// and head-ring radius so the geometry is testable
/// without a whole editor session (the library
/// thumbnail's `library_thumb_prims` split, mirrored).
/// `pos` must carry one joint per node of `fig.def`
/// (the caller's evaluation guarantees it); a node
/// missing from `bends` takes the straight path.
///
/// Every segment goes through
/// `paint::styled_segment_pieces` with the node's own
/// thickness, style, bend and resolved colour
/// (`node.color`, else the figure's live colour — a
/// project figure HAS one, where a library def does
/// not), so taper, caps, `color2` gradients and style
/// opacity are the stage's, not a miniature's
/// approximation of them. Piece widths are the stage's
/// widths scaled by the fit, floored at
/// [`THUMB_MIN_WIDTH`]. Segment media is not drawn —
/// the stage itself can only outline it (Rune `Shape`
/// has no bitmap primitive), and an identity card does
/// not claim it.
///
/// Wave 14 (C8 — clip-def media): the exclusion covers
/// clip-def media too, by the same argument, and
/// deliberately. The clip-media spec
/// (`docs/wave13/CLIP_MEDIA.md` §6) resolves clip-def
/// media in the ONE sampling funnel
/// (`media::media_items`) feeding the raster, SVG, the
/// stage placeholders and the project thumbnails; it
/// never names these card thumbnails, which do not
/// consume the funnel at all. The stage-consistent
/// choice for the card is provenance-blindness: media
/// the funnel would source from the figure's clip def
/// is treated exactly as media on the figure's own
/// def — not drawn. Anything else would make the card
/// the one surface where the SAME media shows or hides
/// depending on which def carries it (the clone-shadow
/// asymmetry the spec exists to remove); and at 44
/// points the stage's own placeholder — an outline
/// plus a fitted text label — cannot be drawn
/// honestly: the quad would be a few points across,
/// its label illegible, an unlabelled box a reader
/// would take for a shape-node glyph. If media ever
/// comes to the cards, own-def and clip-def media must
/// arrive together, resolved through the funnel's set.
/// The tests below pin the invariance: the prims are
/// identical for a plain figure, an own-def-media
/// figure, and a clip instance whose media lives only
/// in its clip def.
fn thumb_prims_for(
    fig: &model::FigureInstance,
    pos: &[V2],
    bends: &[f32],
    head_r: f32,
    size: f32,
) -> Vec<ThumbPrim> {
    if fig.def.nodes.is_empty() {
        return Vec::new();
    }
    let has_head = fig.def.nodes.iter().any(|n| n.head);
    let fit = thumb_fit(pos, has_head, size, head_r);
    let s = fit.scale;
    let base = hex_color(fig.color).unwrap_or(Color::WHITE);
    let mut prims = Vec::new();
    // Segments: the stage's construction verbatim
    // (stage_view's figure pass) — pieces are produced
    // in stage space from the raw joints, thickness,
    // style and bend, then mapped; piece widths scale
    // by the fit's scale exactly as the stage scales
    // them by its view scale.
    for (i, node) in fig.def.nodes.iter().enumerate() {
        if let Some(p) = node.parent {
            let bend = bends.get(i).copied().unwrap_or(0.0);
            let pieces = stykanimate_rs::paint::styled_segment_pieces(
                pos[p],
                pos[i],
                bend,
                node.thickness,
                &node.style,
                node.color.unwrap_or(fig.color),
            );
            for piece in pieces {
                prims.push(ThumbPrim::Piece {
                    a: fit.map(piece.a),
                    b: fit.map(piece.b),
                    width: (piece.width * s).max(THUMB_MIN_WIDTH),
                    color: hex_color(piece.color).unwrap_or(Color::WHITE),
                    alpha: piece.alpha,
                });
            }
        }
    }
    // Shape nodes: the stage's shape pass — a `Circle`
    // node rings at its own thickness, a `Polygon`
    // fills, both centred on the node with radius half
    // its length and both at the node's style opacity.
    // Their base is the same live figure colour the
    // segments resolve against.
    for (i, node) in fig.def.nodes.iter().enumerate() {
        let centre = fit.map(pos[i]);
        let r = node.length / 2.0 * s;
        let color = node
            .color
            .map(|hex| hex_color(hex).unwrap_or(base))
            .unwrap_or(base);
        match node.shape {
            model::Shape::Circle => prims.push(ThumbPrim::Ring {
                pts: ngon(centre, r, 24),
                width: (node.thickness * s).max(THUMB_MIN_WIDTH),
                color,
                alpha: node.style.opacity,
            }),
            model::Shape::Polygon(sides) => prims.push(ThumbPrim::Disc {
                pts: ngon(centre, r, sides as usize),
                color,
                alpha: node.style.opacity,
            }),
            model::Shape::Line => {}
        }
    }
    // Head rings last, at the figure colour plain, as
    // the stage draws them (no node colour, no style
    // opacity — the D6 doctrine): the radius is the
    // project's own head-ring radius the caller passed,
    // and the width is the stage's 5-unit ring width
    // scaled by the fit, floored like every other width.
    for (i, node) in fig.def.nodes.iter().enumerate() {
        if node.head {
            prims.push(ThumbPrim::Ring {
                pts: ngon(fit.map(pos[i]), head_r * s, HEAD_RING_SIDES),
                width: (5.0 * s).max(THUMB_MIN_WIDTH),
                color: base,
                alpha: 1.0,
            });
        }
    }
    prims
}

/// Figure `fi`'s rig at the current display frame as a
/// miniature `Shape` for a `size`-point square canvas:
/// the prims of [`thumb_prims`] folded into canvas ops
/// — pieces as round-capped lines, rings as closed
/// strokes, discs as fills, each at its prim's width
/// and alpha.
/// An out-of-range `fi` yields an empty shape.
pub fn thumb_shape(core: &EditorCore, fi: usize, size: f32) -> Shape {
    if core.project.figures.get(fi).is_none() {
        return Shape::new();
    }
    let mut shape = Shape::new();
    for prim in thumb_prims(core, fi, size) {
        shape = match prim {
            ThumbPrim::Piece {
                a,
                b,
                width,
                color,
                alpha,
            } => shape.line(a, b, width, LineCap::Round, color.alpha(alpha)),
            ThumbPrim::Ring {
                pts,
                width,
                color,
                alpha,
            } => shape.stroke_with(pts, true, width, LineCap::Round, color.alpha(alpha)),
            ThumbPrim::Disc { pts, color, alpha } => shape.fill(pts, color.alpha(alpha)),
        };
    }
    shape
}

/// Reorder dispatch shared by the up/down buttons: select
/// figure `fi`, then move it one slot with the full layers
/// engine (`EditorCore::move_figure_full` acts on the
/// selection and routes through `layers::move_figure`, so
/// poses and every figure reference travel with the figure —
/// the layers panel's `move_layer` precedent). `dir` is +1
/// toward the front (drawn on top), -1 toward the back; at
/// the ends of the list the op is a safe no-op.
fn move_row(core: &mut EditorCore, fi: usize, dir: i32) {
    core.select_figure(fi);
    core.move_figure_full(dir);
}

/// Opacity-cycle dispatch: select figure `fi`, then cycle its
/// opacity (1.0 → 0.6 → 0.3 → 1.0) via
/// `EditorCore::cycle_figure_opacity`, which takes the index
/// directly — the select keeps the panel's "row actions act
/// on the row's figure and select it" behaviour uniform.
fn cycle_row_opacity(core: &mut EditorCore, fi: usize) {
    core.select_figure(fi);
    core.cycle_figure_opacity(fi);
}

/// Delete dispatch: select figure `fi`, then
/// `EditorCore::delete_figure` (which acts on the selection,
/// snapshots for undo, and itself refuses to remove the last
/// figure — "Keep at least one figure.").
fn delete_row(core: &mut EditorCore, fi: usize) {
    core.select_figure(fi);
    core.delete_figure();
}

/// The recolour source for the header's "Recolour"
/// button: the selected figure's index paired with the
/// colour its selected joint resolves to, or `None` when
/// there is no usable selection — no figure at the selected
/// index, or no joint selected.
///
/// Colour resolution mirrors the engine's own rule,
/// `node.color.unwrap_or(fig.color)` (export.rs's segment
/// writer, and the stage's text pass): when the selected
/// node carries an explicit colour (`NodeDef.color` is
/// `Some`), that colour is the source; when it is `None`
/// the node inherits, so the figure's own colour is the
/// resolved value and is the source instead. Either way the
/// returned colour is the one the stage would draw that
/// joint in.
pub fn recolor_source_color(core: &EditorCore) -> Option<(usize, u32)> {
    let fi = core.selected_figure;
    let fig = core.project.figures.get(fi)?;
    let ni = core.selected_node?;
    let node = fig.def.nodes.get(ni)?;
    Some((fi, node.color.unwrap_or(fig.color)))
}

/// Recolour dispatch for the header button: paint the whole
/// selected figure with its selected joint's resolved
/// colour via `EditorCore::recolor_figure` (one snapshot;
/// the op sets its own status, including its refusals).
/// With no figure or no joint selected the press is silent
/// — no status is set and nothing is snapshotted.
fn recolor_row(core: &mut EditorCore) {
    if let Some((fi, color)) = recolor_source_color(core) {
        core.recolor_figure(fi, color);
    }
}

/// Duplicate dispatch for the header button: duplicate the
/// selected figure via `EditorCore::duplicate_figure` (one
/// snapshot; the op names the copy "<name> copy", inserts
/// it right after the original, selects it with no node
/// selected, remaps every figure-indexed reference, and
/// sets its own status — so the dispatch adds neither a
/// selection nor a status of its own). With no figure at
/// the selected index — an empty project or a stale
/// selection — the press is silent, the Recolour
/// convention: the guard resolves first, so no status is
/// set and nothing is snapshotted.
fn duplicate_row(core: &mut EditorCore) {
    if core.project.figures.get(core.selected_figure).is_some() {
        let _ = core.duplicate_figure(core.selected_figure);
    }
}

/// Gap (points) between line 2's controls. The line's
/// width budget beside the thumbnail is 210 pt (the
/// panel's 276 pt content width, less the row's px 8 on
/// both sides, the 44 pt thumbnail and the 6 pt row
/// gap); the seven controls measure 186 pt (four 26 pt
/// icon-only squares, the 41 pt `opac` chip and two
/// 20.5 pt stepper chips — chip widths as laid out in
/// the headless tree dump), and the grow spacer makes
/// EIGHT children, so there are seven gaps to charge:
/// at gap 4 the line totals 214 pt and its last stepper
/// ends 4 pt past the row's content edge (the dump
/// shows both lines ending at x 300 against a 296 pt
/// content edge); at this gap the total is 207 pt and
/// the spacer absorbs the remaining 3 pt. Pinned by
/// `line_two_controls_fit_beside_the_thumbnail`.
const LINE2_GAP: f32 = 3.0;

/// One figure row: the live thumbnail canvas
/// (`fig-thumb-{fi}`, Phase F) spans two control lines
/// beside it. Line 1 is the colour dot + name (press
/// selects), the opacity readout, then the lock / solo
/// toggles; line 2 carries the remaining controls,
/// right-aligned: reorder up/down (`fig-up-{fi}` /
/// `fig-dn-{fi}`), opacity cycle (`fig-opac-{fi}`), the
/// eye toggle (`fig-eye-{fi}` — moved down from line 1
/// by the Phase N pixel-check fix), delete
/// (`fig-del-{fi}`) and the opacity steppers
/// (`fig-op-dn-{fi}` / `fig-op-up-{fi}` — moved down
/// from line 1 by the UI-mission S07-1 fix, which gave
/// the name ≈82 pt and took the row from 80 pt to
/// 62 pt tall).
/// Two lines keep every control at the same compact
/// toggle size instead of squeezing nine onto one line.
fn figure_row(r: &FigureRow, core: &EditorCore, t: &Theme) -> El {
    let fi = r.index;
    let dot = hex_color(r.color).unwrap_or(Color::WHITE);
    // Two-line variant of the kit list-row anatomy (P8
    // §1/FIG-1): the same px, radius, selected fill and
    // hover as `design::list_row`, applied to a `row`
    // whose thumbnail spans a `col` of the two control
    // lines beside it (S07-1; the kit builder itself is
    // a single-line `row`). Unselected rows rest
    // transparent on the card, exactly as the kit shell
    // does — the hover overlay paints the highlight.
    let root = row()
        .key(format!("fig-row-{fi}"))
        .w_full()
        .gap(6.0)
        .px(8.0)
        .py(4.0)
        .radius(design::ROW_RADIUS)
        .min_h(28.0)
        .center_y();
    let root = if r.selected {
        root.fill(design::selected_fill())
    } else {
        root.hover(t.hover)
    };
    root
        .child(
            // Live silhouette of this figure's rig at
            // the display frame (Phase F); a pure view
            // element — no press handler, no op. Since
            // S07-1 it spans both control lines: the
            // lines ride in the column beside it.
            canvas(thumb_shape(core, fi, THUMB_SIZE))
                .key(format!("fig-thumb-{fi}"))
                .size(THUMB_SIZE, THUMB_SIZE),
        )
        .child(
            col()
                .grow(1.0)
                .gap(2.0)
                .child(
                    // Line 1: dot + name, the opacity
                    // readout, lock and solo. The
                    // opacity steppers moved to line 2
                    // (S07-1): on this line they left
                    // the name ≈28 pt; without them
                    // the select cluster measures
                    // 98 pt, ≈82 of it name text.
                    row()
                        .w_full()
                        .gap(6.0)
                        .center_y()
                        .child(
                            // Leading cluster: dot + name; pressing it selects
                            // the figure (old `fig-sel`). It is the row's
                            // main press target, so it carries the kit's
                            // hover/press overlays like every other
                            // pressable in the panel (the button/chip
                            // builders all carry both) — without them a
                            // press on the already-selected figure gave
                            // no visual answer at all. `min_h` matches
                            // the line's 26 pt control height: the hit
                            // box was the 15 pt text line alone, in a
                            // line whose other controls are 26 pt tall
                            // (the line's height is unchanged — the
                            // lock button already sets it). The name
                            // truncates: it is an unbounded user string
                            // in a fixed-sum line (its budget is the
                            // ≈82 pt the S07-1 comment measures), and
                            // without Rune's one-line truncate mode a
                            // longer name's min-content width forces
                            // the whole line past the card's edge
                            // instead of eliding.
                            row()
                                .key(format!("fig-sel-{fi}"))
                                .gap(6.0)
                                .center_y()
                                .grow(1.0)
                                .min_h(26.0)
                                .radius(design::ROW_RADIUS)
                                .hover(t.hover)
                                .press(t.press)
                                .on_press(move |s: &mut State| s.core.select_figure(fi))
                                .child(el().size(10.0, 10.0).radius(5.0).fill(dot))
                                .child(
                                    text(r.name.clone())
                                        .font(type_::LABEL)
                                        .truncate()
                                        .color(if r.hidden { t.text_secondary } else { t.text }),
                                ),
                        )
                        .child(
                            text(format!("{}%", r.opacity_pct))
                                .font(type_::MONO_CAPTION.mono())
                                .color(t.text_secondary),
                        )
                        .child(icon_only_btn(
                            &format!("fig-lock-{fi}"),
                            if r.locked { "lock" } else { "lock-open" },
                            if r.locked { "Unlock figure" } else { "Lock figure" },
                            26.0, r.locked,
                            t,
                            move |s: &mut State| s.core.toggle_figure_lock(fi),
                        ))
                        .child(chip_btn(
                            &format!("fig-solo-{fi}"),
                            None,
                            "solo",
                            r.solo,
                            t,
                            move |s: &mut State| s.core.toggle_solo(fi),
                        )),
                )
                .child(
                    // Line 2: reorder / opacity cycle /
                    // eye / delete / opacity steppers,
                    // right-aligned under line 1. The
                    // eye rides this line since the
                    // Phase N pixel check: as line 1's
                    // last toggle it measured past the
                    // row's right edge (and the card's).
                    // The steppers joined it in S07-1.
                    // The gap is LINE2_GAP (see its
                    // budget arithmetic): an earlier
                    // comment here claimed gap 4 totals
                    // 208 pt, but that counted only the
                    // six gaps between the controls and
                    // under-measured the chips — with
                    // the spacer's gap charged and the
                    // chips at their laid-out widths the
                    // line totals 214 pt, 4 over budget.
                    row()
                        .w_full()
                        .gap(LINE2_GAP)
                        .center_y()
                        .child(el().grow(1.0))
                        .child(icon_only_btn(
                            &format!("fig-up-{fi}"),
                            "chevron-up",
                            "Move up",
                            26.0, false,
                            t,
                            move |s: &mut State| move_row(&mut s.core, fi, 1),
                        ))
                        .child(icon_only_btn(
                            &format!("fig-dn-{fi}"),
                            "chevron-down",
                            "Move down",
                            26.0, false,
                            t,
                            move |s: &mut State| move_row(&mut s.core, fi, -1),
                        ))
                        .child(chip_btn(
                            &format!("fig-opac-{fi}"),
                            None,
                            "opac",
                            false,
                            t,
                            move |s: &mut State| cycle_row_opacity(&mut s.core, fi),
                        ))
                        .child(icon_only_btn(
                            &format!("fig-eye-{fi}"),
                            if r.hidden { "eye-slash" } else { "eye" },
                            if r.hidden { "Show figure" } else { "Hide figure" },
                            26.0, r.hidden,
                            t,
                            // The old listener selected the figure first:
                            // `toggle_figure_hidden` acts on the selection.
                            move |s: &mut State| {
                                s.core.select_figure(fi);
                                s.core.toggle_figure_hidden();
                            },
                        ))
                        .child(icon_only_btn(
                            &format!("fig-del-{fi}"),
                            "trash-can",
                            "Delete figure",
                            26.0, false,
                            t,
                            move |s: &mut State| delete_row(&mut s.core, fi),
                        ))
                        // Wave Q: the opacity chips take the
                        // chip family's inert form at the
                        // readout's own bounds — the same
                        // StepForm split Wave P gave the
                        // inspector's opacity pair (the op
                        // clamps to 0..=1, so at 0% / 100%
                        // the step can only no-op).
                        .child(if r.opacity_pct == 0 {
                            chip_btn_inert(&format!("fig-op-dn-{fi}"), None, "−", t)
                        } else {
                            chip_btn(
                                &format!("fig-op-dn-{fi}"),
                                None,
                                "−",
                                false,
                                t,
                                move |s: &mut State| s.core.step_figure_opacity(fi, -0.1),
                            )
                        })
                        .child(if r.opacity_pct == 100 {
                            chip_btn_inert(&format!("fig-op-up-{fi}"), None, "+", t)
                        } else {
                            chip_btn(
                                &format!("fig-op-up-{fi}"),
                                None,
                                "+",
                                false,
                                t,
                                move |s: &mut State| s.core.step_figure_opacity(fi, 0.1),
                            )
                        }),
                ),
        )
}

/// Height (points) of one figure row as laid out by
/// [`figure_row`]: the two control lines at 26 pt each
/// (line 1's select cluster `min_h` and the 26 pt
/// icon-only buttons set the line heights), the 2 pt
/// gap between them, and the root's 4 pt vertical
/// padding on both sides — 26 + 2 + 26 + 8 = 62. The
/// 44 pt thumbnail is shorter than the line pair, so
/// it never sets the height.
pub const FIG_ROW_H: f32 = 62.0;

/// Cap (points) on the figure list's scroll viewport:
/// five rows at [`FIG_ROW_H`] plus the four
/// [`design::PANEL_GAP`]s between them (342 pt). Five
/// visible rows is the shell's list convention — the
/// graph channel list's `CHAN_LIST_MAX_H` shows five
/// channel rows the same way. Past the cap the list
/// scrolls internally; at or under it the viewport is
/// exactly the rows' height, so small projects lay out
/// byte-identical to the unbounded panel.
pub const FIG_LIST_MAX_H: f32 = 5.0 * FIG_ROW_H + 4.0 * design::PANEL_GAP;

/// The figures panel: header (title, count, collapse toggle,
/// "+ Figure"), the name-filter field, one row per visible
/// figure, and the bulk visibility footer (All / None /
/// Invert). When `s.figures_panel.collapsed` is set, only
/// the header and its divider render.
pub fn figures_panel(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let collapsed = s.figures_panel.collapsed;
    let query = s.figures_panel.filter.text.clone();
    let visible = visible_figures(core, &query);
    let total = core.project.figures.len();
    let count = count_line(visible.len(), total, &query);
    let mut panel = design::panel_col("figures-panel", t)
        .child(
            design::header_row("figures-panel-header")
                .child(design::panel_title(None, &i18n_wire::tr(core, "figures"), Some(&count), t))
                .child(crate::chrome_view::btn(
                    "fig-collapse",
                    if collapsed { "Show" } else { "Hide" },
                    false,
                    t,
                    |s| s.figures_panel.collapsed = !s.figures_panel.collapsed,
                ))
                .child(crate::chrome_view::btn("add-figure", &i18n_wire::tr(core, "add_figure"), false, t, |s| {
                    s.core.add_figure()
                })),
        )
        // Header actions row (Phase N pixel-check fix):
        // Recolour and Duplicate used to ride the header
        // row itself, but the Phase N headless tree dump
        // measured that row ~127pt wider than the card's
        // 276pt content width — `fig-duplicate` landed
        // entirely outside the card and `fig-recolor`
        // past the content edge. The two labelled
        // actions keep their labels on their own row
        // instead of pushing past the card edge.
        .child(
            row()
                .gap(6.0)
                .child(icon_btn(
                    "fig-recolor",
                    "paintbrush",
                    "Recolour",
                    false,
                    t,
                    |s: &mut State| recolor_row(&mut s.core),
                ))
                .child(icon_btn(
                    "fig-duplicate",
                    "copy",
                    "Duplicate",
                    false,
                    t,
                    |s: &mut State| duplicate_row(&mut s.core),
                )),
        )
        .child(design::divider(t));
    if collapsed {
        return panel;
    }
    // Filter field (Phase G), mirroring the library search:
    // typing is the only writer, and the list below filters
    // on the text every render. View-only — no engine op.
    panel = panel.child(
        row()
            .key("figures-filter-row")
            .w_full()
            .gap(8.0)
            .center_y()
            .child(icon_el("magnifying-glass", 13.0, t.text_secondary))
            .child(
                design::text_field(&s.figures_panel.filter, type_::BODY.lh(26.0), s.figures_panel.filter.focused, t)
                    .key("figures-filter")
                    .single_line()
                    .grow(1.0)
                    .color(t.text)
                    .placeholder("Filter figures", t.text_secondary)
                    .on_edit(|s: &mut State| &mut s.figures_panel.filter),
            ),
    );
    // Rows for the visible figures only, built from the
    // full row list by real index, so every control and
    // thumbnail still addresses its own figure. The rows
    // ride in a bounded scroll viewport (Wave 10 — see
    // FIG_LIST_MAX_H): past five rows the list scrolls
    // inside the card instead of pushing the bulk footer
    // and every panel below this one down the column.
    // The no-match state replaces the list outright (the
    // library panel's precedent), and an empty project
    // shows neither, exactly as before.
    let rows = figure_rows(core);
    if visible.is_empty() {
        if total > 0 {
            panel = panel.child(design::empty_state(
                "magnifying-glass",
                &format!("No figures match \"{}\".", query.trim()),
                t,
            ));
        }
    } else {
        let mut list = col()
            .key("figures-list")
            .w_full()
            .gap(design::PANEL_GAP);
        for fi in &visible {
            list = list.child(figure_row(&rows[*fi], core, t));
        }
        panel = panel.child(
            list.scroll(ScrollSpec {
                stick_bottom: false,
            })
            .min_h(0.0)
            .max_h(FIG_LIST_MAX_H),
        );
    }
    // Bulk visibility footer: the layers ops own the `hidden`
    // flags; each handler also clears solo so the bulk result
    // is the effective visibility, and reports the change
    // count in the status line — the old listeners verbatim.
    panel.child(
        row()
            .key("figures-panel-footer")
            .w_full()
            .gap(8.0)
            .center_y()
            .child(crate::chrome_view::btn("fig-vis-all", "All", false, t, |s| {
                let changed = stykanimate_rs::layers::set_all_visible(&mut s.core.project, true);
                s.core.solo = None;
                s.core.status = format!("All figures shown ({changed} changed).");
            }))
            .child(crate::chrome_view::btn("fig-vis-none", "None", false, t, |s| {
                let changed = stykanimate_rs::layers::set_all_visible(&mut s.core.project, false);
                s.core.solo = None;
                s.core.status = format!("All figures hidden ({changed} changed).");
            }))
            .child(crate::chrome_view::btn("fig-vis-inv", "Invert", false, t, |s| {
                let flipped = stykanimate_rs::layers::invert_visibility(&mut s.core.project);
                s.core.solo = None;
                s.core.status = format!("Figure visibility inverted ({flipped} flipped).");
            })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn figures_i18n_keys_exist_and_match_the_english_labels() {
        // Wave V (V11, from U10's catalogue-hygiene
        // ledger): the panel title and the add-figure
        // button resolve through i18n_wire::tr under
        // pre-existing catalogue keys. The keys already
        // existed x28, so this pin passes on its own
        // merits — no IA was owed.
        let core = EditorCore::new();
        for (key, english) in [
            ("figures", "Figures"),
            ("add_figure", "+ Figure"),
        ] {
            assert!(i18n_wire::key_in_table(key), "missing i18n key: {key}");
            assert_eq!(i18n_wire::tr(&core, key), english);
        }
    }

    #[test]
    fn panel_state_defaults_to_expanded() {
        assert!(!FiguresPanelState::default().collapsed);
    }

    #[test]
    fn panel_state_default_filter_is_empty() {
        let st = FiguresPanelState::default();
        assert!(st.filter.text.is_empty());
        assert!(!st.filter.focused);
    }

    /// A core carrying the given names on its first figures:
    /// topped up with `add_figure` when the demo project
    /// seeds fewer (the call the other tests here use), and
    /// any extra seeded figures are renamed "Zzz" so they
    /// cannot match the queries under test.
    fn named_core(names: &[&str]) -> EditorCore {
        let mut core = EditorCore::new();
        while core.project.figures.len() < names.len() {
            core.project.add_figure("Zzz", 0xFF0000, [200.0, 200.0]);
        }
        for (i, fig) in core.project.figures.iter_mut().enumerate() {
            fig.name = names.get(i).copied().unwrap_or("Zzz").to_string();
        }
        core
    }

    #[test]
    fn visible_figures_empty_query_lists_everything_in_order() {
        let core = named_core(&["Alpha", "Beta", "Gamma"]);
        let all: Vec<usize> = (0..core.project.figures.len()).collect();
        assert!(!all.is_empty());
        assert_eq!(visible_figures(&core, ""), all);
        // A whitespace-only query is no filter either.
        assert_eq!(visible_figures(&core, "   "), all);
    }

    #[test]
    fn visible_figures_matches_case_insensitive_substrings() {
        let core = named_core(&["Alpha", "beta", "Alphabet", "Gamma"]);
        // Substring match, case-insensitive on both sides.
        assert_eq!(visible_figures(&core, "alpha"), vec![0, 2]);
        assert_eq!(visible_figures(&core, "ALPHA"), vec![0, 2]);
        assert_eq!(visible_figures(&core, "Beta"), vec![1]);
        // The returned indices are the real project slots.
        let vis = visible_figures(&core, "alpha");
        assert_eq!(core.project.figures[vis[0]].name, "Alpha");
        assert_eq!(core.project.figures[vis[1]].name, "Alphabet");
    }

    #[test]
    fn visible_figures_trims_the_query() {
        let core = named_core(&["Alpha", "Beta"]);
        assert_eq!(visible_figures(&core, "  beta  "), vec![1]);
    }

    #[test]
    fn visible_figures_no_match_is_empty() {
        let core = named_core(&["Alpha", "Beta"]);
        assert!(visible_figures(&core, "qqq").is_empty());
    }

    #[test]
    fn count_line_is_the_bare_total_without_a_filter() {
        // Byte-identical to the pre-filter header count.
        assert_eq!(count_line(3, 3, ""), "3");
        assert_eq!(count_line(3, 3, "   "), "3");
        assert_eq!(count_line(0, 0, ""), "0");
    }

    #[test]
    fn count_line_shows_shown_of_total_with_a_filter() {
        assert_eq!(count_line(2, 5, "alpha"), "2 of 5 figures");
        assert_eq!(count_line(0, 5, "qqq"), "0 of 5 figures");
        assert_eq!(count_line(5, 5, "a"), "5 of 5 figures");
    }

    #[test]
    fn opacity_pct_rounds_and_clamps() {
        assert_eq!(opacity_pct(1.0), 100);
        assert_eq!(opacity_pct(0.5), 50);
        assert_eq!(opacity_pct(0.333), 33);
        assert_eq!(opacity_pct(0.0), 0);
        assert_eq!(opacity_pct(1.7), 100);
        assert_eq!(opacity_pct(-0.2), 0);
    }

    #[test]
    fn figure_rows_mirror_the_project() {
        let core = EditorCore::new();
        let rows = figure_rows(&core);
        assert_eq!(rows.len(), core.project.figures.len());
        assert!(!rows.is_empty(), "the demo project seeds figures");
        for (r, fig) in rows.iter().zip(core.project.figures.iter()) {
            assert_eq!(r.name, fig.name);
            assert_eq!(r.color, fig.color);
            assert_eq!(r.hidden, fig.hidden);
            assert_eq!(r.locked, fig.locked);
            assert_eq!(r.selected, r.index == core.selected_figure);
            assert_eq!(r.solo, core.solo == Some(r.index));
        }
    }

    #[test]
    fn figure_rows_construct_at_the_opacity_bounds() {
        // Wave Q: the opacity chips build in their
        // inert branches (0% dn, 100% up) and in a
        // live mid row, under both themes.
        let core = EditorCore::new();
        for dark in [true, false] {
            let t = rune::tok::theme(dark);
            for pct in [0, 50, 100] {
                let r = FigureRow {
                    index: 0,
                    name: "A".to_string(),
                    color: 0xFF0000,
                    hidden: false,
                    locked: false,
                    solo: false,
                    selected: true,
                    opacity_pct: pct,
                };
                let _ = figure_row(&r, &core, &t);
            }
        }
    }

    #[test]
    fn figure_rows_reflect_hidden_solo_and_opacity() {
        let mut core = EditorCore::new();
        core.project.figures[0].hidden = true;
        core.project.figures[0].opacity = 0.5;
        core.toggle_solo(0);
        let rows = figure_rows(&core);
        assert!(rows[0].hidden);
        assert!(rows[0].solo);
        assert_eq!(rows[0].opacity_pct, 50);
    }

    #[test]
    fn figure_rows_follow_selection() {
        let mut core = EditorCore::new();
        if core.project.figures.len() > 1 {
            core.select_figure(1);
            let rows = figure_rows(&core);
            assert!(!rows[0].selected);
            assert!(rows[1].selected);
        }
    }

    #[test]
    fn move_row_reorders_and_selection_follows() {
        let mut core = EditorCore::new();
        core.project.add_figure("B", 0xFF0000, [200.0, 200.0]);
        let back_name = core.project.figures[0].name.clone();
        // "Up" (+1, toward the front): the back figure swaps
        // one slot later and the selection follows it.
        move_row(&mut core, 0, 1);
        assert_eq!(core.project.figures[1].name, back_name);
        assert_eq!(core.selected_figure, 1);
        // Moving up again at the front is a no-op.
        move_row(&mut core, 1, 1);
        assert_eq!(core.project.figures[1].name, back_name);
        // "Down" (-1, toward the back) restores the order.
        move_row(&mut core, 1, -1);
        assert_eq!(core.project.figures[0].name, back_name);
        assert_eq!(core.selected_figure, 0);
    }

    #[test]
    fn cycle_row_opacity_walks_the_cycle_and_selects() {
        let mut core = EditorCore::new();
        core.project.add_figure("B", 0xFF0000, [200.0, 200.0]);
        // add_figure seeds opacity 1.0; the cycle is
        // 1.0 → 0.6 → 0.3 → 1.0 (layers::cycle_opacity).
        // Cycling row 1 while row 0 is selected also selects
        // the cycled row.
        core.select_figure(0);
        cycle_row_opacity(&mut core, 1);
        assert_eq!(core.selected_figure, 1);
        assert_eq!(core.project.figures[1].opacity, 0.6);
        cycle_row_opacity(&mut core, 1);
        assert_eq!(core.project.figures[1].opacity, 0.3);
        cycle_row_opacity(&mut core, 1);
        assert_eq!(core.project.figures[1].opacity, 1.0);
        // The untouched figure keeps its opacity.
        assert_eq!(core.project.figures[0].opacity, 1.0);
    }

    #[test]
    fn delete_row_removes_and_the_last_figure_is_refused() {
        let mut core = EditorCore::new();
        core.project.add_figure("B", 0xFF0000, [200.0, 200.0]);
        let n = core.project.figures.len();
        assert!(n >= 2);
        // Deleting row 1 removes exactly that figure.
        delete_row(&mut core, 1);
        assert_eq!(core.project.figures.len(), n - 1);
        assert!(core.project.figures.iter().all(|f| f.name != "B"));
        // Delete down to one figure, then the op refuses.
        while core.project.figures.len() > 1 {
            delete_row(&mut core, 0);
        }
        delete_row(&mut core, 0);
        assert_eq!(core.project.figures.len(), 1);
        assert_eq!(core.status, "Keep at least one figure.");
    }

    #[test]
    fn thumb_prims_match_the_demo_figures_rig() {
        let core = EditorCore::new();
        assert!(!core.project.figures.is_empty());
        for (fi, fig) in core.project.figures.iter().enumerate() {
            let prims = thumb_prims(&core, fi, THUMB_SIZE);
            let pieces = prims
                .iter()
                .filter(|p| matches!(p, ThumbPrim::Piece { .. }))
                .count();
            let rings = prims
                .iter()
                .filter(|p| matches!(p, ThumbPrim::Ring { .. }))
                .count();
            let discs = prims
                .iter()
                .filter(|p| matches!(p, ThumbPrim::Disc { .. }))
                .count();
            // The demo defs are flat-styled (default
            // NodeStyle throughout) and the demo project
            // carries no bend data, so every parented node
            // contributes exactly ONE styled piece; a
            // styled or bent node would contribute more
            // (the fixture test below pins those counts).
            // Rings are one per head node plus one per
            // Circle shape node, discs one per Polygon
            // shape node — the stage's draw list,
            // miniaturised.
            assert_eq!(
                pieces,
                fig.def.nodes.iter().filter(|n| n.parent.is_some()).count(),
                "piece count for figure {fi}"
            );
            assert_eq!(
                rings,
                fig.def.nodes.iter().filter(|n| n.head).count()
                    + fig
                        .def
                        .nodes
                        .iter()
                        .filter(|n| n.shape == model::Shape::Circle)
                        .count(),
                "ring count for figure {fi}"
            );
            assert_eq!(
                discs,
                fig.def
                    .nodes
                    .iter()
                    .filter(|n| matches!(n.shape, model::Shape::Polygon(_)))
                    .count(),
                "disc count for figure {fi}"
            );
            assert!(!prims.is_empty(), "demo figure {fi} draws something");
        }
    }

    #[test]
    fn thumb_points_stay_inside_the_box() {
        let core = EditorCore::new();
        for fi in 0..core.project.figures.len() {
            for prim in thumb_prims(&core, fi, THUMB_SIZE) {
                let pts: Vec<[f32; 2]> = match &prim {
                    ThumbPrim::Piece { a, b, .. } => vec![*a, *b],
                    ThumbPrim::Ring { pts, .. } => pts.clone(),
                    ThumbPrim::Disc { pts, .. } => pts.clone(),
                };
                assert!(!pts.is_empty());
                for p in pts {
                    assert!(
                        p[0] >= -1e-3 && p[0] <= THUMB_SIZE + 1e-3,
                        "x of {p:?} outside the {THUMB_SIZE} box (figure {fi})"
                    );
                    assert!(
                        p[1] >= -1e-3 && p[1] <= THUMB_SIZE + 1e-3,
                        "y of {p:?} outside the {THUMB_SIZE} box (figure {fi})"
                    );
                }
            }
        }
    }

    #[test]
    fn thumb_shape_builds_scales_and_guards_range() {
        let core = EditorCore::new();
        let shape = thumb_shape(&core, 0, THUMB_SIZE);
        assert!(!shape.is_empty(), "the demo figure draws a thumbnail");
        // A bigger box produces a different drawing.
        assert_ne!(shape, thumb_shape(&core, 0, THUMB_SIZE * 2.0));
        // Out-of-range figure: empty results, no panic.
        assert!(thumb_shape(&core, 999, THUMB_SIZE).is_empty());
        assert!(thumb_prims(&core, 999, THUMB_SIZE).is_empty());
    }

    #[test]
    fn thumb_prims_carry_each_nodes_resolved_colour() {
        // Wave 12 (D6): a segment paints in its child node's
        // resolved colour — the node's own override when it
        // has one, else the figure colour (the stage's rule);
        // a head ring paints in the figure colour even when
        // the head node carries an override (the stage's ring
        // pass never reads the node colour).
        let mut core = EditorCore::new();
        let fi = core
            .project
            .figures
            .iter()
            .position(|f| {
                f.def.nodes.iter().any(|n| n.head)
                    && f.def
                        .nodes
                        .iter()
                        .filter(|n| n.parent.is_some() && !n.head)
                        .count()
                        >= 2
            })
            .expect("a demo figure with a head and 2+ plain segments");
        let fig_color = core.project.figures[fi].color;
        let head = core.project.figures[fi]
            .def
            .nodes
            .iter()
            .position(|n| n.head)
            .expect("a head node");
        let plain: Vec<usize> = core.project.figures[fi]
            .def
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.parent.is_some() && !n.head)
            .map(|(i, _)| i)
            .take(2)
            .collect();
        // One plain segment gets an explicit colour, the
        // other is pinned to inheriting, and the head node
        // gets an override its ring must ignore.
        core.project.figures[fi].def.nodes[plain[0]].color = Some(0x112233);
        core.project.figures[fi].def.nodes[plain[1]].color = None;
        core.project.figures[fi].def.nodes[head].color = Some(0x445566);
        let prims = thumb_prims(&core, fi, THUMB_SIZE);
        // Piece colours, in node order, mirror the def's
        // resolution exactly (every node here is
        // flat-styled, so each parented node contributes
        // exactly one piece, in node order).
        let seg_colors: Vec<Color> = prims
            .iter()
            .filter_map(|p| match p {
                ThumbPrim::Piece { color, .. } => Some(*color),
                _ => None,
            })
            .collect();
        let expected: Vec<Color> = core.project.figures[fi]
            .def
            .nodes
            .iter()
            .filter(|n| n.parent.is_some())
            .map(|n| hex_color(n.color.unwrap_or(fig_color)).unwrap())
            .collect();
        assert_eq!(seg_colors, expected);
        assert!(
            seg_colors.contains(&hex_color(0x112233).unwrap()),
            "the override shows"
        );
        assert!(
            seg_colors.contains(&hex_color(fig_color).unwrap()),
            "the inheriting node shows the figure colour"
        );
        // Every ring carries the figure colour, never the
        // head node's 0x445566 override.
        let ring_colors: Vec<Color> = prims
            .iter()
            .filter_map(|p| match p {
                ThumbPrim::Ring { color, .. } => Some(*color),
                _ => None,
            })
            .collect();
        assert!(!ring_colors.is_empty());
        assert!(ring_colors.iter().all(|c| *c == hex_color(fig_color).unwrap()));
    }

    #[test]
    fn thumb_prims_honour_node_colour_style_and_shape() {
        // Wave 13 (C4): the library panel's styled-fixture
        // test, mirrored for a PROJECT figure (its
        // `thumb_prims_honour_node_colour_style_and_shape`
        // over `library_thumb_prims`). A hand-built def
        // exercising every styled path: node 1 plain
        // (inherits the figure colour) and the head;
        // node 2 a red override tapered to a point;
        // node 3 a colour→color2 gradient; node 4 style
        // opacity 0.5; node 5 a Circle shape node at
        // opacity 0.25; node 6 a Polygon shape node.
        // Shape nodes still draw their segment (the
        // stage's rule), so the piece run is one run per
        // parented node, in node order. The figure's
        // colour is white, so inheritance resolves to
        // white exactly as the library fixture's base
        // colour does — the prim counts and widths below
        // are the library test's own numbers.
        let n = |parent: Option<usize>, length: f32, rest: f32| model::NodeDef {
            min_angle: None,
            max_angle: None,
            name: "n".into(),
            parent,
            length,
            rest_angle: rest,
            head: false,
            bend: 1.0,
            thickness: 5.0,
            color: None,
            ik_end: false,
            shape: model::Shape::Line,
            style: model::NodeStyle::default(),
            media: None,
        };
        let mut nodes = vec![
            n(None, 0.0, 0.0),       // 0 root
            n(Some(0), 40.0, 180.0), // 1 plain, head
            n(Some(1), 30.0, 0.0),   // 2 red point taper
            n(Some(1), 30.0, 90.0),  // 3 gradient
            n(Some(0), 35.0, -90.0), // 4 opacity 0.5
            n(Some(0), 20.0, 45.0),  // 5 circle
            n(Some(0), 20.0, -45.0), // 6 polygon
        ];
        nodes[1].head = true;
        nodes[2].color = Some(0xFF0000);
        nodes[2].style = model::NodeStyle {
            taper: 0.0,
            cap: model::CapKind::Point,
            color2: None,
            opacity: 1.0,
        };
        nodes[3].style = model::NodeStyle {
            taper: 1.0,
            cap: model::CapKind::Round,
            color2: Some(0x0000FF),
            opacity: 1.0,
        };
        nodes[4].style.opacity = 0.5;
        nodes[5].shape = model::Shape::Circle;
        nodes[5].style.opacity = 0.25;
        nodes[6].shape = model::Shape::Polygon(5);
        let def = model::FigureDef { nodes };
        let fig = model::FigureInstance {
            name: "Fixture".into(),
            color: 0xFFFFFF,
            def: def.clone(),
            hidden: false,
            clip: None,
            states: Vec::new(),
            locked: false,
            opacity: 1.0,
        };
        let pos = model::evaluate(&def, &def.rest_pose([0.0, 0.0]));
        let prims = thumb_prims_for(&fig, &pos, &[], HEAD_RING_R, THUMB_SIZE);
        let pieces: Vec<&ThumbPrim> = prims
            .iter()
            .filter(|p| matches!(p, ThumbPrim::Piece { .. }))
            .collect();
        // Node order: 1 flat piece, 4 taper pieces, 4
        // gradient pieces, then nodes 4, 5, 6 flat.
        assert_eq!(pieces.len(), 12);
        // Node 1 inherits the figure colour at full alpha.
        match pieces[0] {
            ThumbPrim::Piece { color, alpha, .. } => {
                assert_eq!(*color, Color::WHITE);
                assert_eq!(*alpha, 1.0);
            }
            _ => unreachable!(),
        }
        // Node 2: every piece pure red; widths shrink
        // along the taper, the tail reaching the
        // legibility floor rather than vanishing.
        let mut widths = Vec::new();
        for prim in &pieces[1..5] {
            match prim {
                ThumbPrim::Piece {
                    width,
                    color,
                    alpha,
                    ..
                } => {
                    assert_eq!(*color, Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 });
                    assert_eq!(*alpha, 1.0);
                    widths.push(*width);
                }
                _ => unreachable!(),
            }
        }
        assert!(widths.windows(2).all(|w| w[0] >= w[1]), "widths: {widths:?}");
        assert!(widths[0] > widths[3]);
        assert_eq!(widths[3], THUMB_MIN_WIDTH);
        // Node 3: the gradient actually travels — first
        // and last piece colours differ, the last one
        // blue-dominant (white → blue).
        let grad: Vec<Color> = pieces[5..9]
            .iter()
            .map(|prim| match prim {
                ThumbPrim::Piece { color, .. } => *color,
                _ => unreachable!(),
            })
            .collect();
        assert_ne!(grad[0], grad[3]);
        assert!(grad[3].b > grad[3].r, "last gradient colour: {:?}", grad[3]);
        // Node 4: the style opacity is the piece alpha.
        match pieces[9] {
            ThumbPrim::Piece { alpha, .. } => assert_eq!(*alpha, 0.5),
            _ => unreachable!(),
        }
        // After the pieces: the Circle node's ring (24
        // pts, at ITS style opacity), the Polygon node's
        // disc (5 pts), and finally the head ring (16
        // pts) at the figure colour, full alpha — the
        // stage's head-ring rule.
        assert_eq!(prims.len(), 15);
        match &prims[12] {
            ThumbPrim::Ring { pts, alpha, .. } => {
                assert_eq!(pts.len(), 24);
                assert_eq!(*alpha, 0.25);
            }
            other => panic!("expected circle ring, got {other:?}"),
        }
        match &prims[13] {
            ThumbPrim::Disc { pts, .. } => assert_eq!(pts.len(), 5),
            other => panic!("expected polygon disc, got {other:?}"),
        }
        match &prims[14] {
            ThumbPrim::Ring {
                pts,
                color,
                alpha,
                ..
            } => {
                assert_eq!(pts.len(), 16);
                assert_eq!(*color, Color::WHITE);
                assert_eq!(*alpha, 1.0);
            }
            other => panic!("expected head ring, got {other:?}"),
        }
    }

    #[test]
    fn thumb_prims_ignore_own_def_segment_media() {
        // Wave 14 (C8): segment media — an image on one
        // node, text on another — on the figure's OWN def
        // contributes nothing to the card (see
        // `thumb_prims_for`'s docs). The prim vectors are
        // compared whole, not just counted: media must
        // change nothing at all.
        let n = |parent: Option<usize>, length: f32, rest: f32| model::NodeDef {
            min_angle: None,
            max_angle: None,
            name: "n".into(),
            parent,
            length,
            rest_angle: rest,
            head: false,
            bend: 1.0,
            thickness: 5.0,
            color: None,
            ik_end: false,
            shape: model::Shape::Line,
            style: model::NodeStyle::default(),
            media: None,
        };
        let mut nodes = vec![
            n(None, 0.0, 0.0),       // 0 root
            n(Some(0), 40.0, 180.0), // 1 head
            n(Some(1), 30.0, 0.0),   // 2
        ];
        nodes[1].head = true;
        let plain = model::FigureDef {
            nodes: nodes.clone(),
        };
        nodes[1].media = Some(model::SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false, opacity: 1.0 });
        nodes[2].media = Some(model::SegmentMedia::Text {
            content: "hi".into(),
            size: 12.0,
        });
        let carried = model::FigureDef { nodes };
        let prims_of = |def: &model::FigureDef| {
            let fig = model::FigureInstance {
                name: "Fixture".into(),
                color: 0xFFFFFF,
                def: def.clone(),
                hidden: false,
                clip: None,
                states: Vec::new(),
                locked: false,
                opacity: 1.0,
            };
            let pos = model::evaluate(def, &def.rest_pose([0.0, 0.0]));
            thumb_prims_for(&fig, &pos, &[], HEAD_RING_R, THUMB_SIZE)
        };
        let plain_prims = prims_of(&plain);
        assert!(!plain_prims.is_empty(), "the fixture draws a card");
        assert_eq!(prims_of(&carried), plain_prims);
    }

    #[test]
    fn thumb_prims_ignore_clip_def_media() {
        // Wave 14 (C8): the clip-def half of the same
        // invariance, through the public `thumb_prims`:
        // a clip instance whose own def is media-free
        // thumbs identically whether or not its CLIP def
        // carries media — the funnel's per-node fallback
        // (CLIP_MEDIA §6.1) does not reach the card. Both
        // cores carry the same clip with the same single
        // frame (the figure's own frame-0 pose), so the
        // evaluated pose — and therefore every prim — is
        // held constant; only the clip def's media
        // differs.
        let build = |with_media: bool| {
            let mut core = EditorCore::new();
            let pose0 = core.project.poses_at(0.0)[0].clone();
            let mut clip_def = core.project.figures[0].def.clone();
            if with_media {
                let ni = clip_def
                    .nodes
                    .iter()
                    .position(|nd| nd.parent.is_some())
                    .expect("the demo def has a parented node");
                clip_def.nodes[ni].media =
                    Some(model::SegmentMedia::Image { asset: 0, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false, opacity: 1.0 });
            }
            core.project.clips.push(model::ClipDef {
                name: "clip".into(),
                def: clip_def,
                frames: vec![pose0],
                loop_mode: model::ClipLoop::Loop,
            });
            core.project.figures[0].clip = Some(0);
            core
        };
        let plain = thumb_prims(&build(false), 0, THUMB_SIZE);
        assert!(!plain.is_empty(), "the demo figure draws a card");
        assert_eq!(thumb_prims(&build(true), 0, THUMB_SIZE), plain);
    }

    #[test]
    fn thumb_prims_arc_bent_segments() {
        // Wave 13 (C4): a nonzero bend at the display
        // frame reaches the card — the generator splits
        // the bent segment at its bend midpoint, so a
        // flat-styled bent node yields TWO pieces where
        // the straight node yields one, and the two
        // halves meet exactly at the mapped bend
        // midpoint (the stage's arc, miniaturised).
        let def = model::FigureDef::simple();
        let fig = model::FigureInstance {
            name: "Bent".into(),
            color: 0xFFFFFF,
            def: def.clone(),
            hidden: false,
            clip: None,
            states: Vec::new(),
            locked: false,
            opacity: 1.0,
        };
        let pos = model::evaluate(&def, &def.rest_pose([0.0, 0.0]));
        let piece_count = |prims: &[ThumbPrim]| {
            prims
                .iter()
                .filter(|p| matches!(p, ThumbPrim::Piece { .. }))
                .count()
        };
        let straight = thumb_prims_for(&fig, &pos, &[], HEAD_RING_R, THUMB_SIZE);
        // Six parented nodes, all flat and straight.
        assert_eq!(piece_count(&straight), 6);
        // Bend node 3 (an arm): three extra pieces — the
        // bent arc is four quarters (Wave Y, P-7).
        let mut bends = vec![0.0; def.nodes.len()];
        bends[3] = 0.5;
        let bent = thumb_prims_for(&fig, &pos, &bends, HEAD_RING_R, THUMB_SIZE);
        assert_eq!(piece_count(&bent), 9);
        // The bent node's pieces are the run's 3rd through
        // 6th (nodes 1 and 2 contribute one piece each
        // before it): its 2nd and 3rd quarters meet at the
        // bend midpoint of the node's evaluated span,
        // mapped by the same fit the straight run used.
        let fit = thumb_fit(&pos, true, THUMB_SIZE, HEAD_RING_R);
        let parent = def.nodes[3].parent.expect("node 3 is parented");
        let mid = fit.map(model::bend_mid(pos[parent], pos[3], 0.5));
        let pieces: Vec<&ThumbPrim> = bent
            .iter()
            .filter(|p| matches!(p, ThumbPrim::Piece { .. }))
            .collect();
        match (pieces[3], pieces[4]) {
            (
                ThumbPrim::Piece { b, .. },
                ThumbPrim::Piece { a, .. },
            ) => {
                assert_eq!(*b, mid, "second quarter ends at the bend midpoint");
                assert_eq!(*a, mid, "third quarter starts at the bend midpoint");
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn thumb_prims_do_not_dim_hidden_figures() {
        // The identity-preview doctrine, pinned (Phase F,
        // restated Wave 13): a hidden figure's card draws
        // EXACTLY the prims it draws while visible — same
        // pieces, colours, widths and alphas; the row's
        // name dimming and eye toggle carry visibility,
        // never the thumbnail. Solo is likewise never
        // consulted.
        let mut core = EditorCore::new();
        assert!(!core.project.figures.is_empty());
        let shown = thumb_prims(&core, 0, THUMB_SIZE);
        assert!(!shown.is_empty());
        core.project.figures[0].hidden = true;
        assert_eq!(thumb_prims(&core, 0, THUMB_SIZE), shown);
        if core.project.figures.len() > 1 {
            core.solo = Some(1);
            assert_eq!(thumb_prims(&core, 0, THUMB_SIZE), shown);
        }
    }

    #[test]
    fn thumb_fit_centres_and_contains_the_bbox() {
        // A 100×50 rig in a 44 box: the width limits
        // (avail 36 / 100 = 0.36), the bbox centre lands on
        // the box centre, and the corners sit on or inside
        // the margin on the limiting axis.
        let pos = vec![[0.0, 0.0], [100.0, 0.0], [100.0, 50.0], [0.0, 50.0]];
        let fit = thumb_fit(&pos, false, THUMB_SIZE, HEAD_RING_R);
        assert!((fit.scale - 0.36).abs() < 1e-6);
        assert_eq!(fit.map([50.0, 25.0]), [22.0, 22.0]);
        for p in pos {
            let m = fit.map(p);
            assert!(m[0] >= THUMB_MARGIN - 1e-4 && m[0] <= THUMB_SIZE - THUMB_MARGIN + 1e-4);
            assert!(m[1] >= THUMB_MARGIN - 1e-4 && m[1] <= THUMB_SIZE - THUMB_MARGIN + 1e-4);
        }
        // A single point maps to the centre at a finite scale.
        let dot = thumb_fit(&[[7.0, 9.0]], false, THUMB_SIZE, HEAD_RING_R);
        assert!(dot.scale.is_finite() && dot.scale > 0.0);
        assert_eq!(dot.map([7.0, 9.0]), [22.0, 22.0]);
        // No points at all: still a finite, positive scale.
        let empty = thumb_fit(&[], true, THUMB_SIZE, HEAD_RING_R);
        assert!(empty.scale.is_finite() && empty.scale > 0.0);
    }

    #[test]
    fn thumb_fit_grows_the_box_for_the_head_ring() {
        // Same rig with and without a head: the head fit must
        // zoom out (the ring needs 17 stage units of room on
        // every side) yet still centre the rig.
        let pos = vec![[0.0, 0.0], [100.0, 0.0], [100.0, 50.0], [0.0, 50.0]];
        let plain = thumb_fit(&pos, false, THUMB_SIZE, HEAD_RING_R);
        let headed = thumb_fit(&pos, true, THUMB_SIZE, HEAD_RING_R);
        assert!(headed.scale < plain.scale);
        assert_eq!(headed.map([50.0, 25.0]), [22.0, 22.0]);
    }

    #[test]
    fn thumb_fit_scales_the_head_room_with_the_radius() {
        // Phase L: the fit takes the radius as a parameter.
        // Doubling it (a project with heads scaled 2x) grows
        // the box by the extra radius on every side: the
        // 100-wide rig's limiting extent goes 134 -> 168, so
        // the scale drops from 36/134 to 36/168, and the rig
        // still centres.
        let pos = vec![[0.0, 0.0], [100.0, 0.0], [100.0, 50.0], [0.0, 50.0]];
        let base = thumb_fit(&pos, true, THUMB_SIZE, HEAD_RING_R);
        let doubled = thumb_fit(&pos, true, THUMB_SIZE, HEAD_RING_R * 2.0);
        assert!((base.scale - 36.0 / 134.0).abs() < 1e-6);
        assert!((doubled.scale - 36.0 / 168.0).abs() < 1e-6);
        assert!(doubled.scale < base.scale);
        assert_eq!(doubled.map([50.0, 25.0]), [22.0, 22.0]);
        // A ring of the doubled radius around a corner head
        // still lands inside the canvas under the doubled
        // fit: centre mapped, edge at centre + r * scale on
        // the limiting axis reaches exactly the margin.
        let corner = doubled.map([0.0, 0.0]);
        let edge = corner[0] - HEAD_RING_R * 2.0 * doubled.scale;
        assert!((edge - THUMB_MARGIN).abs() < 1e-4, "edge {edge}");
    }

    #[test]
    fn thumb_prims_ring_radius_uses_the_project_radius() {
        // At the default head scale the project's radius is
        // the base constant, and the ring polygon the panel
        // builds has exactly that radius times the fit scale
        // — the Phase L wiring, pinned at scale 1.0.
        let core = EditorCore::new();
        assert_eq!(core.project.head_ring_radius(), HEAD_RING_R);
        let fi = core
            .project
            .figures
            .iter()
            .position(|f| f.def.nodes.iter().any(|n| n.head))
            .expect("a demo figure with a head node");
        let pos = core.positions_f(core.display_frame(), fi);
        let fit = thumb_fit(&pos, true, THUMB_SIZE, HEAD_RING_R);
        let prims = thumb_prims(&core, fi, THUMB_SIZE);
        let ring = prims
            .iter()
            .find_map(|p| match p {
                ThumbPrim::Ring { pts, .. } => Some(pts),
                _ => None,
            })
            .expect("a head ring prim");
        let centre = [
            ring.iter().map(|p| p[0]).sum::<f32>() / ring.len() as f32,
            ring.iter().map(|p| p[1]).sum::<f32>() / ring.len() as f32,
        ];
        let radius = ring
            .iter()
            .map(|p| (p[0] - centre[0]).hypot(p[1] - centre[1]))
            .fold(0.0f32, f32::max);
        assert!(
            (radius - HEAD_RING_R * fit.scale).abs() < 1e-3,
            "ring radius {radius}, expected {}",
            HEAD_RING_R * fit.scale
        );
    }

    #[test]
    fn thumb_prims_ring_radius_follows_a_scaled_project() {
        // The same figure in a project whose head scale is
        // 2.0: `thumb_prims` must draw the ring at the
        // project's doubled radius (times the new fit's
        // scale), not at the base constant.
        let mut core = EditorCore::new();
        core.project.head_ring_scale = 2.0;
        assert_eq!(core.project.head_ring_radius(), HEAD_RING_R * 2.0);
        let fi = core
            .project
            .figures
            .iter()
            .position(|f| f.def.nodes.iter().any(|n| n.head))
            .expect("a demo figure with a head node");
        let pos = core.positions_f(core.display_frame(), fi);
        let fit = thumb_fit(&pos, true, THUMB_SIZE, HEAD_RING_R * 2.0);
        let prims = thumb_prims(&core, fi, THUMB_SIZE);
        let ring = prims
            .iter()
            .find_map(|p| match p {
                ThumbPrim::Ring { pts, .. } => Some(pts),
                _ => None,
            })
            .expect("a head ring prim");
        let centre = [
            ring.iter().map(|p| p[0]).sum::<f32>() / ring.len() as f32,
            ring.iter().map(|p| p[1]).sum::<f32>() / ring.len() as f32,
        ];
        let radius = ring
            .iter()
            .map(|p| (p[0] - centre[0]).hypot(p[1] - centre[1]))
            .fold(0.0f32, f32::max);
        assert!(
            (radius - HEAD_RING_R * 2.0 * fit.scale).abs() < 1e-3,
            "ring radius {radius}, expected {}",
            HEAD_RING_R * 2.0 * fit.scale
        );
        // And the ring still stays inside the box.
        for p in ring {
            assert!(p[0] >= -1e-3 && p[0] <= THUMB_SIZE + 1e-3);
            assert!(p[1] >= -1e-3 && p[1] <= THUMB_SIZE + 1e-3);
        }
    }

    #[test]
    fn recolor_source_color_is_none_without_a_joint() {
        // The demo core starts with no joint selected, so
        // there is no colour source — and the dispatch press
        // is silent: status untouched, nothing recoloured.
        let mut core = EditorCore::new();
        assert_eq!(core.selected_node, None);
        assert_eq!(recolor_source_color(&core), None);
        let status = core.status.clone();
        recolor_row(&mut core);
        assert_eq!(core.status, status);
        // A selected index past the end of the project is
        // no figure at all: still None, even with a joint.
        core.selected_figure = 999;
        core.selected_node = Some(0);
        assert_eq!(recolor_source_color(&core), None);
    }

    #[test]
    fn recolor_source_color_resolves_the_inherited_colour() {
        // A selected joint whose node colour is None
        // inherits: the source is the figure's own colour.
        let mut core = EditorCore::new();
        let fi = core.selected_figure;
        assert!(!core.project.figures[fi].def.nodes.is_empty());
        core.project.figures[fi].def.nodes[0].color = None;
        core.selected_node = Some(0);
        let fig_color = core.project.figures[fi].color;
        assert_eq!(recolor_source_color(&core), Some((fi, fig_color)));
    }

    #[test]
    fn recolor_source_color_uses_the_explicit_node_colour() {
        // A selected joint with an explicit colour: that
        // colour is the source, and the dispatch paints
        // every node of the figure with it (color2 aside).
        let mut core = EditorCore::new();
        let fi = core.selected_figure;
        core.selected_node = Some(0);
        core.set_node_color(0x123456);
        assert_eq!(
            core.project.figures[fi].def.nodes[0].color,
            Some(0x123456)
        );
        assert_eq!(recolor_source_color(&core), Some((fi, 0x123456)));
        recolor_row(&mut core);
        assert!(core.project.figures[fi]
            .def
            .nodes
            .iter()
            .all(|n| n.color == Some(0x123456)));
        assert_eq!(core.status, format!("Figure {} recoloured.", fi + 1));
    }

    #[test]
    fn duplicate_row_clones_the_selected_figure_and_selects_the_copy() {
        let mut core = EditorCore::new();
        let fi = core.selected_figure;
        let name = core.project.figures[fi].name.clone();
        let n = core.project.figures.len();
        // A selected joint proves the op's contract: the
        // copy becomes the selection with no node selected.
        core.selected_node = Some(0);
        duplicate_row(&mut core);
        assert_eq!(core.project.figures.len(), n + 1);
        // The copy sits right after the original and carries
        // the op's naming, "<name> copy"; the original is
        // untouched. The op itself selects the copy and sets
        // the status — the dispatch adds neither.
        assert_eq!(core.project.figures[fi].name, name);
        assert_eq!(core.project.figures[fi + 1].name, format!("{name} copy"));
        assert_eq!(core.selected_figure, fi + 1);
        assert_eq!(core.selected_node, None);
        assert_eq!(core.status, "Figure duplicated.");
    }

    #[test]
    fn duplicate_row_on_an_empty_project_is_a_silent_no_op() {
        // No figure at the selected index: the press is
        // silent — no panic, no figure appears, and the
        // status line is left exactly as it was (the op's
        // own "No figure to duplicate." never fires because
        // the dispatch resolves first).
        let mut core = EditorCore::new();
        core.project.figures.clear();
        let status = core.status.clone();
        duplicate_row(&mut core);
        assert!(core.project.figures.is_empty());
        assert_eq!(core.status, status);
    }

    #[test]
    fn line_two_controls_fit_beside_the_thumbnail() {
        // The 210 pt beside the thumbnail: the panel's
        // 276 pt content width, less the row's px 8 on
        // both sides (260), the 44 pt thumbnail and the
        // 6 pt row gap. The seven controls' widths are
        // their laid-out widths in the headless tree
        // dump (tree-default): four icon-only squares at
        // the 26 pt size this builder passes, the `opac`
        // chip at 41, the two stepper chips at 20.5. The
        // grow spacer is an eighth child, so seven gaps
        // are charged — the count an earlier version of
        // this budget got wrong (it charged six gaps and
        // claimed 208 pt at gap 4; the true gap-4 total
        // is 214, and the dump showed the line ending
        // 4 pt past the row's content edge).
        let controls = 26.0 + 26.0 + 41.0 + 26.0 + 26.0 + 20.5 + 20.5;
        assert_eq!(controls, 186.0);
        assert_eq!(controls + 7.0 * 4.0, 214.0, "gap 4 overflows the 210 budget");
        assert_eq!(LINE2_GAP, 3.0);
        let total = controls + 7.0 * LINE2_GAP;
        assert_eq!(total, 207.0);
        assert!(total <= 210.0, "line 2 totals {total} against the 210 budget");
    }

    #[test]
    fn fig_list_viewport_caps_at_five_rows() {
        // The Wave 10 bound, as an arithmetic pin in the
        // line-two budget's style. One row lays out at
        // 62 pt: two 26 pt control lines, the 2 pt gap
        // between them, and the root's 4 pt padding top
        // and bottom.
        assert_eq!(FIG_ROW_H, 26.0 + 2.0 + 26.0 + 8.0);
        assert_eq!(FIG_ROW_H, 62.0);
        // The cap is five rows plus the four panel gaps
        // between them — the graph channel list's
        // five-row convention (CHAN_LIST_MAX_H).
        assert_eq!(
            FIG_LIST_MAX_H,
            5.0 * FIG_ROW_H + 4.0 * crate::rune_sec::design::PANEL_GAP
        );
        assert_eq!(FIG_LIST_MAX_H, 342.0);
        // Five rows exactly fill the viewport (no scroll,
        // byte-identical layout to the unbounded panel);
        // the sixth row is the first that scrolls.
        let gap = crate::rune_sec::design::PANEL_GAP;
        let content_h = |n: f32| n * FIG_ROW_H + (n - 1.0) * gap;
        assert_eq!(content_h(5.0), FIG_LIST_MAX_H);
        assert!(content_h(6.0) > FIG_LIST_MAX_H);
        // Ten figures — the audit's case — now cost the
        // panel the capped 342 pt of list, not ≈688 pt.
        assert!(content_h(10.0) > 2.0 * FIG_LIST_MAX_H);
    }

    #[test]
    fn figure_rows_build_for_a_many_figure_project() {
        // The round-2 audit's case: well past the list
        // cap, every row still derives from its real
        // index (the bounded viewport changes layout
        // only, never addressing) and constructs.
        let core = named_core(&[
            "One", "Two", "Three", "Four", "Five", "Six", "Seven", "Eight", "Nine", "Ten",
            "Eleven", "Extra figure",
        ]);
        let rows = figure_rows(&core);
        assert!(rows.len() >= 12, "the project carries 12+ figures");
        let t = rune::tok::theme(true);
        for (i, r) in rows.iter().enumerate() {
            assert_eq!(r.index, i);
            let _ = figure_row(r, &core, &t);
        }
        // The filter still addresses real indices at
        // this size: only the twelfth figure matches.
        assert_eq!(visible_figures(&core, "extra"), vec![11]);
    }

    #[test]
    fn figure_row_constructs_on_all_state_branches() {
        // Construction-only pin (element internals are
        // opaque to a bin-crate unit test — the Phase G
        // §12 finding): the row builds for selected and
        // unselected, hidden and visible figures, with
        // the select cluster's hover/press overlays and
        // the name's truncate mode wired. Layout truth
        // is the headless tree's job, as elsewhere.
        let core = EditorCore::new();
        let t = rune::tok::theme(true);
        let mut r = figure_rows(&core)[0].clone();
        for selected in [true, false] {
            for hidden in [true, false] {
                r.selected = selected;
                r.hidden = hidden;
                let _ = figure_row(&r, &core, &t);
            }
        }
        // A name far past the ≈82 pt line-1 budget
        // (a renamed figure) still constructs.
        r.name = "A very long figure name indeed, far past the budget".to_string();
        let _ = figure_row(&r, &core, &t);
    }
}
