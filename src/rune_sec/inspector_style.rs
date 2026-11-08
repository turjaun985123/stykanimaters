//! Segment style inspector (Rune Phase D, section 5/40;
//! direct setters in Phase E, section 13/40).
//! Declared under `crate::rune_sec` — bin-crate code, not part
//! of the engine library.
//!
//! Style panel for the currently selected segment. Every
//! control calls exactly one real `EditorCore` op:
//!
//! * Cap segmented control → `set_node_cap` (editor.rs, added
//!   by Phase E section 7 beside the style cyclers): each
//!   chip sets exactly its own kind — Round, Square or Point.
//! * Gradient swatches → `set_node_color2` (same addition):
//!   each swatch sets exactly its own gradient end colour —
//!   None (off), white or #222222.
//! * Gradient RGB steppers (Phase K) → `set_node_color2`
//!   again: three channel rows under the swatches give the
//!   gradient end the fine control the primary colour's
//!   steppers have always had — rendered only while an end
//!   colour is set (the swatches' Off state has nothing to
//!   step). Each press steps one channel by [`RGB_STEP`]
//!   off the node's CURRENT end colour and writes the
//!   composed colour back through the same op a swatch
//!   press uses, snapshot behaviour included (unlike
//!   `set_node_color`, `set_node_color2` snapshots —
//!   editor.rs:1978).
//! * Opacity stepper → `set_node_opacity` (same addition):
//!   − / + in 0.05 steps over the model's 0.0..=1.0 range,
//!   with a % readout.
//! * Colour RGB steppers (Phase F) → `set_node_color`
//!   (editor.rs:532 — the same op the segment inspector's
//!   colour swatches press): R / G / B rows step one channel
//!   by 8 (clamped 0..=255) off the node's effective colour
//!   and write the composed `#RRGGBB` back. Like the swatch
//!   op and the op it mirrors, it does not snapshot; the
//!   readout shows the composed hex value.
//! * Taper stepper → `cycle_node_taper` (editor.rs:1796):
//!   taper cycles 1.0 → 0.6 → 0.3 → 0.0 → 1.0. Taper stays a
//!   cycle control on purpose: the engine has NO
//!   `set_node_taper`, and adding one is an engine change,
//!   not a panel change.
//! * Copy / Paste buttons (Phase G; atomic paste Phase H)
//!   → the panel-local style clipboard ([`capture_style`] /
//!   [`apply_style`], held in a `thread_local` like
//!   `camera_panel`'s key clipboard). The clipboard holds
//!   exactly the fields this panel can write through a
//!   direct core setter — cap (`set_node_cap`), gradient
//!   end (`set_node_color2`), colour (`set_node_color`) and
//!   opacity (`set_node_opacity`) — and paste applies all
//!   four at once through
//!   `EditorCore::set_node_style_atomic` (Phase H, engine
//!   section 38), which writes them through those same
//!   setters' write paths under a single snapshot.
//!
//!   The captured colour is the node's *effective* colour
//!   ([`selected_color`]): `set_node_color` always writes
//!   `Some(rgb)`, so the inherit-from-figure state (`None`)
//!   is itself not copyable — a paste makes the captured
//!   colour explicit on the target node.
//!
//!   Excluded, honestly: taper and thickness. Taper IS a
//!   `NodeStyle` field, but with no `set_node_taper` this
//!   panel can only cycle it, so an arbitrary captured
//!   taper could not be written back. Thickness is not a
//!   `NodeStyle` field at all (it is `NodeDef.thickness`)
//!   and this panel has no thickness control: the segment
//!   inspector owns it through the delta-only
//!   `adjust_thickness`, which cannot set an exact value
//!   either.
//!
//!   Undo: paste is atomic (Phase H). Phase G applied the
//!   four fields through the panel's separate setters and
//!   inherited their mixed snapshot semantics — one paste
//!   landed as three undo steps, and a single undo reverted
//!   only the last setter's field. `set_node_style_atomic`
//!   snapshots exactly once before writing all four fields,
//!   so one paste is exactly ONE undo step and one undo
//!   after a paste reverts cap, gradient, colour and
//!   opacity together.
//!
//! * Preset chips (Phase I) → [`STYLE_PRESETS`]: six
//!   curated colour + cap pairs. A preset changes colour
//!   and cap ONLY — it applies through the same atomic op
//!   (`set_node_style_atomic`) with the selected node's
//!   CURRENT gradient end and opacity passed straight
//!   back, so both survive a preset press untouched
//!   ([`preset_application`] computes the call's
//!   arguments; [`apply_preset`] makes it).
//!
//! * Recent colours row (Phase J) → [`RECENT_COLORS`]: a
//!   panel-local `thread_local` list (most recent first,
//!   no duplicates, capped at [`RECENT_COLORS_CAP`]) of
//!   the colours this panel's own handlers have applied
//!   to the selected node. Recorded by [`note_recent_color`]
//!   from exactly four sites: a gradient swatch press
//!   (its end colour, when not Off), a gradient stepper
//!   press (Phase K — its stepped end colour, recorded
//!   like a swatch press: the press applies a finished
//!   gradient end through `set_node_color2`), a preset
//!   application ([`apply_preset`]) and a paste
//!   ([`paste_style`]). The primary-colour RGB steppers
//!   deliberately do NOT record — each press
//!   is an intermediate value on the way to a colour, and
//!   recording them would flood the row with near-misses.
//!   The row renders only when the list is non-empty (a
//!   selection is already required for the panel to show
//!   controls at all); pressing a chip applies the colour
//!   as the node's own colour via `set_node_color` — the
//!   op the segment inspector's palette swatches press —
//!   and notes it again, moving it to the front.
//!
//! History: Phase D shipped cap and gradient as single
//! press targets advancing `cycle_node_cap` /
//! `cycle_node_gradient` one step per press, because no
//! direct-set op existed. Phase E section 7 added the
//! setters to the engine, so the chips and swatches now set
//! their own values directly.
//!
//! Deliberately absent: shade steps (fig3d's
//! `segment_shades` is a computed depth shading over a 3D
//! pose, not a stored per-segment style an op can set).
//!
//! Phase N restyle — pure presentation, no behaviour
//! change: the root is the design kit's `panel_col`, the
//! header the kit's `panel_title` over a kit `divider`,
//! row labels the kit's `section_label`, the cap and
//! preset chips the button family's `chip_btn` (the
//! preset chip's 12×12 colour square goes away —
//! `chip_btn` has no content slot, and the charter names
//! `preset_chip` among the chips it absorbs), the
//! gradient and recent-colour swatches the kit's one
//! `swatch` construction (24×24; the two local copies
//! collapse onto it), and the RGB / gradient-RGB /
//! opacity steppers the charter order (label grows, −,
//! kit `stepper_value`, +) — `channel_row` and
//! `gradient_channel_row` merge into one builder with a
//! key-stem parameter, as plan P03 prescribed. The
//! colour header row's 18×18 preview square is
//! display-only (no key, no handler) and stays a plain
//! element, like the channel rows' mini-swatches.

use std::cell::RefCell;

use rune::tok::{self, type_, Theme};
use rune::{col, el, row, text, Color, El};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::{CapKind, NodeStyle};

use crate::chrome_view::btn;
use crate::rune_sec::design;
use crate::rune_sec::icon_btn::{chip_btn, icon_btn};
use crate::stage_view::hex_color;
use crate::State;

/// The cap kinds in segmented-control display order (the
/// engine's historical cycle order, Round → Square → Point).
pub const CAP_ORDER: [CapKind; 3] = [CapKind::Round, CapKind::Square, CapKind::Point];

/// One curated style preset: a name, a colour and a cap.
/// Applying a preset changes exactly those two fields of
/// the selected node's style (see [`preset_application`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StylePreset {
    pub name: &'static str,
    pub color: u32,
    pub cap: CapKind,
}

/// The six style presets the panel offers, in display
/// order (two rows of three chips). Every cap is a real
/// [`CapKind`] variant — the model has Round, Square and
/// Point only, so Violet's briefed Butt cap is Square, the
/// other flat-ended cap (Point tapers to a tip).
pub const STYLE_PRESETS: [StylePreset; 6] = [
    StylePreset { name: "Ink", color: 0x20_242C, cap: CapKind::Round },
    StylePreset { name: "Crimson", color: 0xD2_3B3B, cap: CapKind::Round },
    StylePreset { name: "Ocean", color: 0x2B_6FD2, cap: CapKind::Square },
    StylePreset { name: "Forest", color: 0x2E_9E5B, cap: CapKind::Round },
    StylePreset { name: "Violet", color: 0x7A_4FD0, cap: CapKind::Square },
    StylePreset { name: "Amber", color: 0xE8_A33D, cap: CapKind::Square },
];

/// The selected node's current style, if a node is selected.
/// Pure read of the core's public selection fields — the same
/// fields `EditorCore::selected_node_mut` resolves internally.
pub fn selected_style(core: &EditorCore) -> Option<NodeStyle> {
    let ni = core.selected_node?;
    core.project
        .figures
        .get(core.selected_figure)?
        .def
        .nodes
        .get(ni)
        .map(|node| node.style)
}

/// Short display label for a cap kind.
pub fn cap_label(cap: CapKind) -> &'static str {
    match cap {
        CapKind::Round => "Round",
        CapKind::Square => "Square",
        CapKind::Point => "Point",
    }
}

/// Position of `cap` in [`CAP_ORDER`] (the segmented control's
/// highlighted segment).
pub fn cap_index(cap: CapKind) -> usize {
    CAP_ORDER.iter().position(|&c| c == cap).unwrap_or(0)
}

/// Which of the gradient states `color2` is in:
/// 0 = off (None), 1 = white, 2 = #222222, 3 = any other
/// colour (reachable only from a loaded file — it matches no
/// swatch; pressing a swatch sets that swatch's value
/// directly via `set_node_color2`).
pub fn gradient_state(color2: Option<u32>) -> usize {
    match color2 {
        None => 0,
        Some(0xFF_FFFF) => 1,
        Some(0x22_2222) => 2,
        Some(_) => 3,
    }
}

/// The `color2` value the swatch for `state` sets when
/// pressed: 0 = None (off), 1 = white, 2 = #222222. Only
/// states 0..3 are rendered as swatches; any other input
/// maps to None.
pub fn gradient_value(state: usize) -> Option<u32> {
    match state {
        1 => Some(0xFF_FFFF),
        2 => Some(0x22_2222),
        _ => None,
    }
}

/// Display label for a gradient state: `Off`, or the end
/// colour as `#RRGGBB`.
pub fn gradient_label(color2: Option<u32>) -> String {
    match color2 {
        None => "Off".to_string(),
        Some(c) => format!("#{c:06X}"),
    }
}

/// Display label for a taper value — the engine's own wording
/// (`style::taper_label`, e.g. `Taper 60%`), so the panel and
/// the status line never disagree.
pub fn taper_text(taper: f32) -> String {
    stykanimate_rs::style::taper_label(taper)
}

/// Opacity step for the − / + stepper.
pub const OPACITY_STEP: f32 = 0.05;

/// The opacity after one stepper press in direction `dir`
/// (−1 / +1) from `current`: stepped by [`OPACITY_STEP`],
/// clamped to the model's 0.0..=1.0 range, and rounded to
/// whole percents so repeated steps don't accumulate float
/// dust (0.95000004).
pub fn opacity_stepped(current: f32, dir: i32) -> f32 {
    let stepped = current + dir as f32 * OPACITY_STEP;
    (stepped.clamp(0.0, 1.0) * 100.0).round() / 100.0
}

/// % readout for an opacity value (`100%`, `55%`, `5%`).
pub fn opacity_text(opacity: f32) -> String {
    format!("{}%", (opacity * 100.0).round() as i32)
}

/// One opacity stepper press against the core: read the
/// selected node's current opacity, step it, and set it back
/// through the real `set_node_opacity` op. No selection →
/// no-op (the panel renders no stepper without a selection
/// anyway).
pub fn step_opacity(core: &mut EditorCore, dir: i32) {
    let Some(style) = selected_style(core) else {
        return;
    };
    core.set_node_opacity(opacity_stepped(style.opacity, dir));
}

/// Per-press channel step for the RGB colour steppers.
pub const RGB_STEP: i32 = 8;

/// Split a `0xRRGGBB` colour into its `(r, g, b)` channels.
pub fn rgb_split(rgb: u32) -> (u8, u8, u8) {
    (
        ((rgb >> 16) & 0xFF) as u8,
        ((rgb >> 8) & 0xFF) as u8,
        (rgb & 0xFF) as u8,
    )
}

/// Compose a `0xRRGGBB` colour from its channels (the exact
/// inverse of [`rgb_split`]).
pub fn rgb_join(r: u8, g: u8, b: u8) -> u32 {
    ((r as u32) << 16) | ((g as u32) << 8) | b as u32
}

/// One channel after a stepper press in direction `dir`
/// (−1 / +1): stepped by [`RGB_STEP`], clamped to 0..=255.
pub fn step_channel(value: u8, dir: i32) -> u8 {
    (value as i32 + dir * RGB_STEP).clamp(0, 255) as u8
}

/// Hex readout for a colour (`#E8E8EC`) — the same format
/// the gradient label uses.
pub fn color_text(rgb: u32) -> String {
    format!("#{rgb:06X}")
}

/// The selected node's effective colour: its own
/// `NodeDef.color` when set, else the figure's colour (what
/// the stage actually draws — `stage_view` resolves it the
/// same way). `None` only when nothing is selected.
pub fn selected_color(core: &EditorCore) -> Option<u32> {
    let ni = core.selected_node?;
    let fig = core.project.figures.get(core.selected_figure)?;
    let node = fig.def.nodes.get(ni)?;
    Some(node.color.unwrap_or(fig.color))
}

/// One RGB channel stepper press against the core: read the
/// selected node's effective colour, step channel `channel`
/// (0 = R, 1 = G, 2 = B) by [`RGB_STEP`], and write the
/// composed colour back through `set_node_color` — the op
/// the segment inspector's swatches press (field write +
/// status, no snapshot, mirroring `cycle_node_color`).
/// No selection (or a bad channel index) → no-op.
pub fn step_color_channel(core: &mut EditorCore, channel: usize, dir: i32) {
    let Some(rgb) = selected_color(core) else {
        return;
    };
    let (r, g, b) = rgb_split(rgb);
    let (r, g, b) = match channel {
        0 => (step_channel(r, dir), g, b),
        1 => (r, step_channel(g, dir), b),
        2 => (r, g, step_channel(b, dir)),
        _ => return,
    };
    core.set_node_color(rgb_join(r, g, b));
}

/// Whether the gradient stepper block renders: the
/// selected node must carry a gradient end colour
/// (`color2` set). The swatches' Off state (no end
/// colour) has nothing to step, so the block is absent
/// entirely rather than rendered inert.
pub fn gradient_steppers_visible(core: &EditorCore) -> bool {
    matches!(selected_style(core), Some(style) if style.color2.is_some())
}

/// One gradient RGB channel stepper press against the
/// core (Phase K): the exact twin of
/// [`step_color_channel`] for the gradient end — read the
/// selected node's current `color2` (via
/// [`selected_style`], the panel's style resolution),
/// step channel `channel` (0 = R, 1 = G, 2 = B) by
/// [`RGB_STEP`], and write the composed colour back
/// through `set_node_color2` — the op the gradient
/// swatches press, with that op's own snapshot behaviour.
/// The stepped colour is recorded via
/// [`note_recent_color`], like a swatch press: the press
/// applies a finished gradient end colour. No selection,
/// no gradient end, or a bad channel index → no-op (and
/// nothing recorded).
pub fn step_gradient_channel(core: &mut EditorCore, channel: usize, dir: i32) {
    let Some(style) = selected_style(core) else {
        return;
    };
    let Some(rgb) = style.color2 else {
        return;
    };
    let (r, g, b) = rgb_split(rgb);
    let (r, g, b) = match channel {
        0 => (step_channel(r, dir), g, b),
        1 => (r, step_channel(g, dir), b),
        2 => (r, g, step_channel(b, dir)),
        _ => return,
    };
    let stepped = rgb_join(r, g, b);
    core.set_node_color2(Some(stepped));
    note_recent_color(stepped);
}

/// A copied segment style: exactly the fields this panel
/// can write through a direct `EditorCore` setter (see the
/// module docs for what is excluded and why — taper and
/// thickness). `color` is the source node's *effective*
/// colour; paste writes it back as an explicit `Some`.
/// `opacity` replays through the atomic paste op
/// (`set_node_style_atomic`), whose opacity write path is
/// `set_node_opacity`'s, so the setter's 0.05..=1.0 clamp
/// applies on paste just as it does to the panel's own
/// stepper.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StyleClipboard {
    pub color: u32,
    pub color2: Option<u32>,
    pub cap: CapKind,
    pub opacity: f32,
}

thread_local! {
    /// The panel's style clipboard. Panel-local by
    /// necessity: the style panel has no state struct, so —
    /// like `camera_panel`'s key clipboard and
    /// `clips_panel`'s selection — the one piece of
    /// panel-owned state lives in a `thread_local`. It
    /// survives workspace switches and panel remounts for
    /// the life of the process, which is what a clipboard
    /// should do; it is not persisted.
    static STYLE_CLIPBOARD: RefCell<Option<StyleClipboard>> =
        RefCell::new(None);
}

/// The clipboard's current contents, if a style was copied.
pub fn clipboard_style() -> Option<StyleClipboard> {
    STYLE_CLIPBOARD.with(|c| *c.borrow())
}

/// Store `clip` as the clipboard's contents.
fn store_clipboard(clip: Option<StyleClipboard>) {
    STYLE_CLIPBOARD.with(|c| *c.borrow_mut() = clip);
}

/// Capture the selected node's full writable style (the
/// copy button's source value). Pure read of the same
/// fields the panel's controls read — `None` when no node
/// is selected.
pub fn capture_style(core: &EditorCore) -> Option<StyleClipboard> {
    let style = selected_style(core)?;
    Some(StyleClipboard {
        color: selected_color(core)?,
        color2: style.color2,
        cap: style.cap,
        opacity: style.opacity,
    })
}

/// Apply `clip` to the selected node through the engine's
/// atomic style op (`set_node_style_atomic`, Phase H):
/// one snapshot, then all four fields written through the
/// same write paths the panel's own setters use — so a
/// paste is exactly one undo step (Phase G's setter-by-
/// setter paste cost three). Returns `false` — changing
/// nothing, not even a status line — when no node is
/// selected, so the caller owns the refusal wording.
pub fn apply_style(core: &mut EditorCore, clip: &StyleClipboard) -> bool {
    if selected_style(core).is_none() {
        return false;
    }
    core.set_node_style_atomic(clip.color, clip.color2, clip.cap, clip.opacity);
    true
}

/// The arguments for applying `preset` to the currently
/// selected node, in `set_node_style_atomic`'s parameter
/// order `(color, color2, cap, opacity)`: the preset's own
/// colour and cap, with the node's CURRENT gradient end
/// and opacity passed through untouched — a preset press
/// changes colour and cap only. `None` when no node is
/// selected (the atomic op would refuse anyway).
pub fn preset_application(
    core: &EditorCore,
    preset: &StylePreset,
) -> Option<(u32, Option<u32>, CapKind, f32)> {
    let style = selected_style(core)?;
    Some((preset.color, style.color2, preset.cap, style.opacity))
}

/// Preset press: apply `preset` to the selected node
/// through the engine's atomic style op — one snapshot,
/// so one preset press is exactly one undo step. Returns
/// `false`, changing nothing at all, when no node is
/// selected. On success the status names the preset,
/// superseding the atomic op's own status line (the paste
/// path does the same with "Style pasted.").
pub fn apply_preset(core: &mut EditorCore, preset: &StylePreset) -> bool {
    let Some((color, color2, cap, opacity)) = preset_application(core, preset) else {
        return false;
    };
    core.set_node_style_atomic(color, color2, cap, opacity);
    core.status = format!("Style preset: {}.", preset.name);
    note_recent_color(color);
    true
}

/// Copy button: capture the selected node's style into the
/// clipboard. With no selection, status-only (the setters'
/// own refusal wording) and any earlier copy is left
/// untouched.
pub fn copy_style(core: &mut EditorCore) {
    match capture_style(core) {
        Some(clip) => {
            store_clipboard(Some(clip));
            core.status = "Style copied.".into();
        }
        None => core.status = "Select a joint first.".into(),
    }
}

/// Paste button: apply the clipboard's style to the
/// selected node via [`apply_style`]. Refusals (empty
/// clipboard, no selection) are status-only and change no
/// project state. On success the status is exactly
/// "Style pasted." — written after the atomic op, whose
/// own status line the paste supersedes.
pub fn paste_style(core: &mut EditorCore) -> bool {
    let Some(clip) = clipboard_style() else {
        core.status = "Copy a style first.".into();
        return false;
    };
    if !apply_style(core, &clip) {
        core.status = "Select a joint first.".into();
        return false;
    }
    note_recent_color(clip.color);
    core.status = "Style pasted.".into();
    true
}

/// Maximum number of recent colours the panel keeps.
pub const RECENT_COLORS_CAP: usize = 8;

thread_local! {
    /// The colours this panel's own handlers have applied
    /// to a selected node, most recent first, no
    /// duplicates, capped at [`RECENT_COLORS_CAP`]. Like
    /// the style clipboard above, it is panel-owned state
    /// in a `thread_local` because the style panel has no
    /// state struct; it survives workspace switches and
    /// panel remounts for the life of the process and is
    /// not persisted. The only writer is
    /// [`note_recent_color`], which funnels through
    /// [`push_recent`], so the ordering / dedupe / cap
    /// policy lives in one pure function.
    static RECENT_COLORS: RefCell<Vec<u32>> =
        RefCell::new(Vec::new());
}

/// The recent-colours policy, pure: `color` moves to the
/// front of `list`, any earlier occurrence of it is
/// dropped (no duplicates), and the result is truncated
/// to [`RECENT_COLORS_CAP`] entries, oldest last. The
/// input is assumed duplicate-free — always true for the
/// panel's list, whose only writer is this function.
pub fn push_recent(list: Vec<u32>, color: u32) -> Vec<u32> {
    let mut out = Vec::with_capacity(RECENT_COLORS_CAP + 1);
    out.push(color);
    out.extend(list.into_iter().filter(|&c| c != color));
    out.truncate(RECENT_COLORS_CAP);
    out
}

/// The panel's current recent colours, most recent first.
pub fn recent_colors() -> Vec<u32> {
    RECENT_COLORS.with(|c| c.borrow().clone())
}

/// Record `color` as the panel's most recent colour. The
/// recording sites are the handlers that apply a colour
/// to the selected node — the gradient swatch press, a
/// gradient stepper press, a preset application, a paste,
/// and a recent chip press itself (which re-notes its
/// colour, moving it to the front). The primary-colour
/// RGB steppers deliberately never call this: their steps
/// are intermediate values, not chosen colours (see the
/// module docs).
pub fn note_recent_color(color: u32) {
    RECENT_COLORS.with(|c| {
        let mut slot = c.borrow_mut();
        let list = std::mem::take(&mut *slot);
        *slot = push_recent(list, color);
    });
}

/// The one RGB channel stepper row builder (Phase N
/// merged the primary and gradient copies — plan P03:
/// the gradient row was "a copy made for a key prefix").
/// `stem` is the key infix (`r` / `g` / `b` for the
/// node's own colour, `gr` / `gg` / `gb` for the
/// gradient end); `step` is the press op
/// ([`step_color_channel`] or [`step_gradient_channel`] —
/// both share the `(core, channel, dir)` shape).
///
/// Layout is the charter stepper order: the label (kit
/// `section_label`) grows, a small square in the
/// channel's own colour (the R row shows red at the
/// channel's value, and so on — the Colour header row's
/// preview idiom; display-only, kept beside the label),
/// the − button, the value in the kit's
/// `stepper_value` mono readout, and the + button
/// stepping by [`RGB_STEP`]. Keys are
/// `style-<stem>-dn` / `style-<stem>-up` in a row keyed
/// `style-<stem>-row`.
fn channel_row_impl(
    stem: &str,
    label: &str,
    channel: usize,
    value: u8,
    step: fn(&mut EditorCore, usize, i32),
    t: &Theme,
) -> El {
    let swatch_rgb = match channel {
        0 => rgb_join(value, 0, 0),
        1 => rgb_join(0, value, 0),
        _ => rgb_join(0, 0, value),
    };
    let swatch = el()
        .size(12.0, 12.0)
        .radius(3.0)
        .fill(hex_color(swatch_rgb).unwrap_or(Color::WHITE));
    row()
        .key(format!("style-{stem}-row"))
        .gap(8.0)
        .center_y()
        .child(design::section_label(label, t).grow(1.0))
        .child(swatch)
        .child(btn(&format!("style-{stem}-dn"), "−", false, t, move |s| {
            step(&mut s.core, channel, -1)
        }))
        .child(design::stepper_value(&value.to_string(), t))
        .child(btn(&format!("style-{stem}-up"), "+", false, t, move |s| {
            step(&mut s.core, channel, 1)
        }))
}

/// One RGB channel stepper row over the node's own
/// colour: keys are `style-<letter>-dn` / `style-<letter>-up`.
fn channel_row(channel: usize, label: &str, value: u8, t: &Theme) -> El {
    let lower = label.to_lowercase();
    channel_row_impl(&lower, label, channel, value, step_color_channel, t)
}

/// One gradient RGB channel stepper row (Phase K):
/// [`channel_row`]'s construction with a `g` key infix —
/// the full "Gradient R" / "Gradient G" / "Gradient B"
/// label, stepping the gradient end by [`RGB_STEP`] via
/// [`step_gradient_channel`]. Keys are
/// `style-g<letter>-dn` / `style-g<letter>-up` in a row
/// keyed `style-g<letter>-row` — the primary rows'
/// `style-<letter>-…` pattern with the infix.
fn gradient_channel_row(channel: usize, label: &str, value: u8, t: &Theme) -> El {
    let stem = match channel {
        0 => "gr",
        1 => "gg",
        _ => "gb",
    };
    channel_row_impl(stem, label, channel, value, step_gradient_channel, t)
}

/// One cap segment chip: a button-family `chip_btn`,
/// active when it is the current cap. Each chip is its
/// own press target — pressing it sets exactly that cap
/// through `EditorCore::set_node_cap`. (Phase N: the
/// hand-rolled chip had no press state — hover only;
/// the family chip supplies it.)
fn cap_chip(cap: CapKind, current: bool, t: &Theme) -> El {
    chip_btn(
        &format!("style-cap-{}", cap_label(cap).to_lowercase()),
        None,
        cap_label(cap),
        current,
        t,
        move |s: &mut State| s.core.set_node_cap(cap),
    )
}

/// One preset chip: a button-family `chip_btn` carrying
/// the preset's name. Pressing it applies the preset via
/// [`apply_preset`]. Keys are `style-preset-{i}` with `i`
/// the preset's index in [`STYLE_PRESETS`]. (Phase N:
/// the hand-rolled chip's 12×12 colour square beside the
/// name is gone — `chip_btn` has no content slot, and
/// the charter names `preset_chip` among the chips it
/// absorbs; the name alone identifies the preset now.)
fn preset_chip(index: usize, preset: StylePreset, t: &Theme) -> El {
    chip_btn(
        &format!("style-preset-{index}"),
        None,
        preset.name,
        false,
        t,
        move |s: &mut State| {
            apply_preset(&mut s.core, &preset);
        },
    )
}

/// One gradient swatch: the kit's one `swatch`
/// construction (24×24; the current state gets the 2 pt
/// accent border). The `Off` state keeps its
/// `control_off` fill — it is a value, not an empty
/// slot. Each swatch is its own press target — pressing
/// it sets exactly that gradient end colour through
/// `EditorCore::set_node_color2` ([`gradient_value`]).
fn gradient_swatch(state: usize, current: bool, t: &Theme) -> El {
    let fill = match state {
        1 => hex_color(0xFF_FFFF).unwrap_or(Color::WHITE),
        2 => hex_color(0x22_2222).unwrap_or(t.text),
        _ => t.control_off,
    };
    design::swatch(
        &format!("style-gradient-{state}"),
        fill,
        current,
        t,
        move |s: &mut State| {
            let value = gradient_value(state);
            s.core.set_node_color2(value);
            // A swatch press applies a colour to the node:
            // record it (the Off state applies no colour).
            if let Some(color) = value {
                note_recent_color(color);
            }
        },
    )
}

/// One recent-colour chip: the kit's one `swatch`
/// construction, like a gradient swatch (the two local
/// copies collapsed onto the kit at Phase N). Pressing
/// it applies the colour as the node's own colour
/// through `set_node_color` — the op the segment
/// inspector's palette swatches press — and notes it
/// again, moving it to the front of the recent list.
/// Keys are `style-recent-{i}` with `i` the colour's
/// position in the recent list.
fn recent_chip(index: usize, color: u32, current: bool, t: &Theme) -> El {
    design::swatch(
        &format!("style-recent-{index}"),
        hex_color(color).unwrap_or(Color::WHITE),
        current,
        t,
        move |s: &mut State| {
            s.core.set_node_color(color);
            note_recent_color(color);
        },
    )
}

/// The Recent colours row: a `Recent` label plus one
/// chip per recent colour, most recent first. Built from
/// the list passed in (rather than reading the
/// `thread_local` itself) so the row's visibility policy
/// is testable without panel state: `None` when the list
/// is empty — the caller only renders what this returns.
/// `current` is the selected node's effective colour; the
/// chip matching it gets the accent frame.
pub fn recents_row(recents: &[u32], current: Option<u32>, t: &Theme) -> Option<El> {
    if recents.is_empty() {
        return None;
    }
    let mut chips = row().gap(4.0).center_y();
    for (i, &color) in recents.iter().enumerate() {
        chips = chips.child(recent_chip(i, color, current == Some(color), t));
    }
    Some(
        row()
            .key("style-recents")
            .gap(8.0)
            .center_y()
            .child(design::section_label("Recent", t))
            .child(chips),
    )
}

/// The segment style panel: a header naming the selected
/// segment, then the taper stepper, cap segmented control,
/// gradient swatches and opacity stepper. With no segment
/// selected it renders the header plus the kit's empty
/// state and no controls (the style ops would only
/// answer "Select a joint first."). The empty state is
/// the UI mission Wave 4 adoption (audit s08 I7): this
/// panel's header-always form is the one the figure and
/// segment panels now share, and its message names this
/// panel's job ("…its style.") against the segment
/// panel's "…its geometry.".
pub fn style_panel(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let mut panel = design::panel_col("inspector-style", t);
    let header = match core.selected_node {
        Some(ni) => format!("Segment style · Segment {}", ni + 1),
        None => "Segment style".to_string(),
    };
    // Card header (Phase N, charter §2): the kit's
    // `panel_title` over a hairline divider. The header
    // has never carried a key and invents none now: it
    // is a plain row wearing `header_row`'s layout
    // (full width, gap 8, centre-aligned) rather than
    // the keyed builder, exactly as the settings card's
    // header is built.
    panel = panel
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(design::panel_title(None, &header, None, t)),
        )
        .child(design::divider(t));
    let Some(style) = selected_style(core) else {
        // No selection: header + divider (already above)
        // over the kit empty state (audit s08 I7), with
        // the message naming THIS panel's job (style) —
        // the segment panel directly below says
        // "geometry" for its own.
        return panel.child(design::empty_state(
            "paintbrush",
            "Select a segment on the stage to edit its style.",
            t,
        ));
    };

    // Taper: current value (engine wording) + one Step button
    // — the only taper op is the forward cycler (no direct
    // setter exists; see module docs).
    panel = panel.child(
        row()
            .key("style-taper-row")
            .gap(8.0)
            .center_y()
            .child(design::section_label("Taper", t))
            .child(
                text(taper_text(style.taper))
                    .font(type_::LABEL)
                    .color(t.text_secondary)
                    .grow(1.0),
            )
            .child(btn("style-taper", "Step", false, t, |s| {
                s.core.cycle_node_taper()
            })),
    );

    // Cap: segmented control of all three kinds, current
    // accented; each chip sets its own kind directly via
    // `set_node_cap`.
    let mut cap_row = row().gap(4.0);
    for cap in CAP_ORDER {
        cap_row = cap_row.child(cap_chip(cap, cap == style.cap, t));
    }
    panel = panel.child(
        row()
            .key("style-cap-row")
            .gap(8.0)
            .center_y()
            .child(design::section_label("Cap", t))
            .child(el().key("style-cap").radius(6.0).child(cap_row)),
    );

    // Gradient: one swatch per end-colour state, current
    // framed; each swatch sets its own colour directly via
    // `set_node_color2`.
    let gstate = gradient_state(style.color2);
    let mut swatches = row().gap(4.0).center_y();
    for state in 0..3 {
        swatches = swatches.child(gradient_swatch(state, state == gstate, t));
    }
    panel = panel.child(
        row()
            .key("style-gradient-row")
            .gap(8.0)
            .center_y()
            .child(design::section_label("Gradient", t))
            .child(el().key("style-gradient").radius(6.0).child(swatches))
            .child(
                text(gradient_label(style.color2))
                    .font(type_::LABEL)
                    .color(t.text_secondary),
            ),
    );

    // Gradient steppers (Phase K): one stepper row per RGB
    // channel over the gradient end colour, mirroring the
    // Colour steppers below — rendered only while an end
    // colour is set (the swatch's Off state has nothing to
    // step, so the block is absent rather than inert).
    if gradient_steppers_visible(core) {
        if let Some(rgb) = style.color2 {
            let (r, g, b) = rgb_split(rgb);
            panel = panel.child(gradient_channel_row(0, "Gradient R", r, t));
            panel = panel.child(gradient_channel_row(1, "Gradient G", g, t));
            panel = panel.child(gradient_channel_row(2, "Gradient B", b, t));
        }
    }

    // Colour: hex readout + a live preview square, then one
    // stepper row per RGB channel stepping the node's own
    // colour via `set_node_color` (the swatch op). Stepping
    // starts from the effective colour (the node's own, or
    // the figure's while unset), so the first press never
    // jumps to near-black.
    if let Some(rgb) = selected_color(core) {
        let (r, g, b) = rgb_split(rgb);
        let preview = el()
            .size(18.0, 18.0)
            .radius(4.0)
            .fill(hex_color(rgb).unwrap_or(Color::WHITE));
        panel = panel.child(
            row()
                .key("style-color-row")
                .gap(8.0)
                .center_y()
                .child(design::section_label("Colour", t))
                .child(preview)
                .child(
                    text(color_text(rgb))
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                ),
        );
        panel = panel.child(channel_row(0, "R", r, t));
        panel = panel.child(channel_row(1, "G", g, t));
        panel = panel.child(channel_row(2, "B", b, t));
    }

    // Recent colours: the colours this panel's own
    // handlers have applied to a node (see
    // `RECENT_COLORS`), as one-press chips under the
    // colour controls. Hidden while the list is empty;
    // a selection is already guaranteed here (the panel
    // returned early without one).
    let recents = recent_colors();
    if let Some(recent_row) = recents_row(&recents, selected_color(core), t) {
        panel = panel.child(recent_row);
    }

    // Opacity: − / + stepper (0.05 steps, % readout) setting
    // the selected node's opacity via `set_node_opacity`.
    // Charter stepper order (Phase N): the label grows,
    // then −, the kit `stepper_value` readout, +.
    panel = panel.child(
        row()
            .key("style-opacity-row")
            .gap(8.0)
            .center_y()
            .child(design::section_label("Opacity", t).grow(1.0))
            .child(btn("style-opacity-dn", "−", false, t, |s| {
                step_opacity(&mut s.core, -1)
            }))
            .child(design::stepper_value(&opacity_text(style.opacity), t))
            .child(btn("style-opacity-up", "+", false, t, |s| {
                step_opacity(&mut s.core, 1)
            })),
    );

    // Copy / Paste: the panel-local style clipboard (see
    // the module docs for what is — and is not — captured,
    // and for the undo consequence of a paste). Icon
    // buttons (Phase H): Font Awesome `copy` / `clipboard`
    // glyphs beside the labels, keys unchanged.
    panel = panel.child(
        row()
            .key("style-copy-paste-row")
            .gap(8.0)
            .center_y()
            .child(design::section_label("Style", t))
            .child(icon_btn("style-copy", "copy", "Copy", false, t, |s| {
                copy_style(&mut s.core)
            }))
            .child(icon_btn("style-paste", "clipboard", "Paste", false, t, |s| {
                paste_style(&mut s.core);
            })),
    );

    // Presets: the six curated colour + cap pairs as
    // swatch chips, two rows of three so the row stays
    // inside the panel's width. A press changes colour
    // and cap only — the node's gradient end and opacity
    // are passed through untouched (see
    // `preset_application`).
    let mut preset_grid = col().gap(4.0);
    for start in (0..STYLE_PRESETS.len()).step_by(3) {
        let mut preset_row = row().gap(4.0);
        for i in start..(start + 3).min(STYLE_PRESETS.len()) {
            preset_row = preset_row.child(preset_chip(i, STYLE_PRESETS[i], t));
        }
        preset_grid = preset_grid.child(preset_row);
    }
    panel = panel.child(
        row()
            .key("style-presets")
            .gap(8.0)
            .child(design::section_label("Presets", t))
            .child(preset_grid),
    );

    panel
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cap_index_follows_the_engine_cycle_order() {
        // The segmented highlight must advance exactly the way
        // `style::cycle_cap` advances the op's state.
        for (i, &cap) in CAP_ORDER.iter().enumerate() {
            assert_eq!(cap_index(cap), i);
            let next = stykanimate_rs::style::cycle_cap(cap);
            assert_eq!(next, CAP_ORDER[(i + 1) % CAP_ORDER.len()]);
        }
    }

    #[test]
    fn cap_labels_are_the_three_kind_names() {
        assert_eq!(cap_label(CapKind::Round), "Round");
        assert_eq!(cap_label(CapKind::Square), "Square");
        assert_eq!(cap_label(CapKind::Point), "Point");
    }

    #[test]
    fn gradient_states_cover_the_cycle_and_loaded_colours() {
        assert_eq!(gradient_state(None), 0);
        assert_eq!(gradient_state(Some(0xFF_FFFF)), 1);
        assert_eq!(gradient_state(Some(0x22_2222)), 2);
        // A colour loaded from a file is outside the cycle.
        assert_eq!(gradient_state(Some(0x12_3456)), 3);
        assert_eq!(gradient_label(None), "Off");
        assert_eq!(gradient_label(Some(0xFF_FFFF)), "#FFFFFF");
        assert_eq!(gradient_label(Some(0x22_2222)), "#222222");
    }

    #[test]
    fn taper_text_uses_engine_wording() {
        assert_eq!(taper_text(1.0), "Taper 100%");
        assert_eq!(taper_text(0.6), "Taper 60%");
        assert_eq!(taper_text(0.0), "Taper 0%");
    }

    #[test]
    fn selected_style_is_none_without_a_selection() {
        let mut core = EditorCore::new();
        core.selected_node = None;
        assert!(selected_style(&core).is_none());
    }

    #[test]
    fn selected_style_reads_the_selected_nodes_style() {
        let mut core = EditorCore::new();
        core.selected_figure = 0;
        core.selected_node = Some(1);
        let style = selected_style(&core).expect("demo figure has a node 1");
        // A fresh demo project carries default-styled nodes.
        assert_eq!(style, NodeStyle::default());
    }

    #[test]
    fn cap_chips_set_exactly_that_cap_via_the_op() {
        let mut core = EditorCore::new();
        core.selected_figure = 0;
        core.selected_node = Some(1);
        // The default cap is Round. One direct set must land
        // on Point — a single cycle step would give Square,
        // so this proves the chip dispatch is a direct set.
        core.set_node_cap(CapKind::Point);
        assert_eq!(selected_style(&core).unwrap().cap, CapKind::Point);
        core.set_node_cap(CapKind::Square);
        assert_eq!(selected_style(&core).unwrap().cap, CapKind::Square);
        core.set_node_cap(CapKind::Round);
        assert_eq!(selected_style(&core).unwrap().cap, CapKind::Round);
    }

    #[test]
    fn gradient_swatches_set_exactly_that_colour_via_the_op() {
        let mut core = EditorCore::new();
        core.selected_figure = 0;
        core.selected_node = Some(1);
        // From Off, one direct set must land on #222222 — a
        // single cycle step would give white, so this proves
        // the swatch dispatch is a direct set.
        core.set_node_color2(gradient_value(2));
        assert_eq!(selected_style(&core).unwrap().color2, Some(0x22_2222));
        core.set_node_color2(gradient_value(1));
        assert_eq!(selected_style(&core).unwrap().color2, Some(0xFF_FFFF));
        core.set_node_color2(gradient_value(0));
        assert_eq!(selected_style(&core).unwrap().color2, None);
    }

    #[test]
    fn gradient_values_match_the_swatch_states() {
        assert_eq!(gradient_value(0), None);
        assert_eq!(gradient_value(1), Some(0xFF_FFFF));
        assert_eq!(gradient_value(2), Some(0x22_2222));
        // Round-trip: each swatch value reads back as its own
        // state, so the pressed swatch is the framed one.
        for state in 0..3 {
            assert_eq!(gradient_state(gradient_value(state)), state);
        }
    }

    #[test]
    fn opacity_stepper_steps_by_five_percent_and_clamps() {
        assert_eq!(opacity_stepped(1.0, -1), 0.95);
        assert_eq!(opacity_stepped(0.5, 1), 0.55);
        assert_eq!(opacity_stepped(0.5, -1), 0.45);
        // Clamped at both ends of the model's 0.0..=1.0 range.
        assert_eq!(opacity_stepped(1.0, 1), 1.0);
        assert_eq!(opacity_stepped(0.98, 1), 1.0);
        assert_eq!(opacity_stepped(0.0, -1), 0.0);
        assert_eq!(opacity_stepped(0.02, -1), 0.0);
    }

    #[test]
    fn opacity_stepper_drives_the_real_op() {
        let mut core = EditorCore::new();
        core.selected_figure = 0;
        core.selected_node = Some(1);
        // Default opacity is 1.0: + is clamped at 100%, then
        // each − press removes exactly one 0.05 step.
        step_opacity(&mut core, 1);
        assert_eq!(selected_style(&core).unwrap().opacity, 1.0);
        step_opacity(&mut core, -1);
        assert_eq!(selected_style(&core).unwrap().opacity, 0.95);
        step_opacity(&mut core, -1);
        assert_eq!(selected_style(&core).unwrap().opacity, 0.90);
        step_opacity(&mut core, 1);
        assert_eq!(selected_style(&core).unwrap().opacity, 0.95);
    }

    #[test]
    fn rgb_split_and_join_round_trip() {
        for rgb in [0x00_0000, 0xFF_FFFF, 0xE8_E8EC, 0x12_3456, 0x00_FF80] {
            let (r, g, b) = rgb_split(rgb);
            assert_eq!(rgb_join(r, g, b), rgb, "round trip of {rgb:#08X}");
        }
        assert_eq!(rgb_split(0xE8_E8EC), (232, 232, 236));
        assert_eq!(rgb_split(0x00_0000), (0, 0, 0));
        assert_eq!(rgb_join(255, 0, 128), 0xFF_0080);
    }

    #[test]
    fn step_channel_steps_by_eight_and_clamps() {
        assert_eq!(step_channel(100, 1), 108);
        assert_eq!(step_channel(100, -1), 92);
        assert_eq!(step_channel(0, 1), 8);
        assert_eq!(step_channel(255, -1), 247);
        // Clamped at both ends of the 0..=255 range.
        assert_eq!(step_channel(255, 1), 255);
        assert_eq!(step_channel(250, 1), 255);
        assert_eq!(step_channel(0, -1), 0);
        assert_eq!(step_channel(4, -1), 0);
    }

    #[test]
    fn selected_color_prefers_the_node_then_the_figure() {
        let mut core = EditorCore::new();
        core.selected_figure = 0;
        core.selected_node = Some(1);
        // Unset node colour → the figure's colour is the
        // effective (drawn) colour.
        core.project.figures[0].def.nodes[1].color = None;
        let figure_color = core.project.figures[0].color;
        assert_eq!(selected_color(&core), Some(figure_color));
        // A set node colour wins.
        core.project.figures[0].def.nodes[1].color = Some(0x12_3456);
        assert_eq!(selected_color(&core), Some(0x12_3456));
        // No selection → no colour.
        core.selected_node = None;
        assert_eq!(selected_color(&core), None);
    }

    #[test]
    fn color_stepper_drives_the_real_op_and_clamps() {
        let mut core = EditorCore::new();
        core.selected_figure = 0;
        core.selected_node = Some(1);
        core.set_node_color(0x64_320A);
        // R +1: 100 → 108, other channels untouched.
        step_color_channel(&mut core, 0, 1);
        assert_eq!(selected_color(&core), Some(0x6C_320A));
        // G −1: 50 → 42.
        step_color_channel(&mut core, 1, -1);
        assert_eq!(selected_color(&core), Some(0x6C_2A0A));
        // B clamps at 0 (10 − 8 − 8 would go negative).
        step_color_channel(&mut core, 2, -1);
        step_color_channel(&mut core, 2, -1);
        assert_eq!(selected_color(&core), Some(0x6C_2A00));
        // And at 255: set R near the top, one press clamps.
        core.set_node_color(0xFA_2A00);
        step_color_channel(&mut core, 0, 1);
        assert_eq!(selected_color(&core), Some(0xFF_2A00));
        // A bad channel index is a no-op.
        step_color_channel(&mut core, 3, 1);
        assert_eq!(selected_color(&core), Some(0xFF_2A00));
        // The op stored it as the node's own colour.
        assert_eq!(
            core.project.figures[0].def.nodes[1].color,
            Some(0xFF_2A00)
        );
    }

    #[test]
    fn color_text_is_uppercase_hex() {
        assert_eq!(color_text(0xE8_E8EC), "#E8E8EC");
        assert_eq!(color_text(0x00_0000), "#000000");
        assert_eq!(color_text(0xFF_0080), "#FF0080");
    }

    #[test]
    fn opacity_text_is_a_whole_percent() {
        assert_eq!(opacity_text(1.0), "100%");
        assert_eq!(opacity_text(0.55), "55%");
        assert_eq!(opacity_text(0.05), "5%");
        assert_eq!(opacity_text(0.0), "0%");
    }

    #[test]
    fn capture_style_is_none_without_a_selection() {
        let mut core = EditorCore::new();
        core.selected_node = None;
        assert!(capture_style(&core).is_none());
        // The copy button agrees: status-only refusal, and
        // the clipboard stays empty.
        copy_style(&mut core);
        assert_eq!(core.status, "Select a joint first.");
        assert!(clipboard_style().is_none());
    }

    #[test]
    fn capture_style_reads_every_writable_field() {
        let mut core = EditorCore::new();
        core.selected_figure = 0;
        core.selected_node = Some(1);
        core.set_node_color(0x12_3456);
        // A non-swatch gradient colour: the clipboard must
        // carry the full value, not a swatch state.
        core.set_node_color2(Some(0x34_5678));
        core.set_node_cap(CapKind::Point);
        core.set_node_opacity(0.55);
        let clip = capture_style(&core).expect("node 1 is selected");
        assert_eq!(
            clip,
            StyleClipboard {
                color: 0x12_3456,
                color2: Some(0x34_5678),
                cap: CapKind::Point,
                opacity: 0.55,
            }
        );
    }

    #[test]
    fn copy_then_paste_round_trips_between_two_nodes() {
        // The clipboard is a per-thread `thread_local` and
        // each test runs on its own thread, so it starts
        // empty here.
        let mut core = EditorCore::new();
        core.selected_figure = 0;

        // Source node 1: a distinctive writable style.
        core.selected_node = Some(1);
        core.set_node_color(0x12_3456);
        core.set_node_color2(Some(0x34_5678));
        core.set_node_cap(CapKind::Point);
        core.set_node_opacity(0.55);
        copy_style(&mut core);
        assert_eq!(core.status, "Style copied.");
        assert!(clipboard_style().is_some());

        // Target node 2: a different style, plus values in
        // the two EXCLUDED fields (taper, thickness) written
        // directly — the paste must leave both untouched.
        core.selected_node = Some(2);
        core.set_node_color(0xFF_0000);
        core.set_node_color2(None);
        core.set_node_cap(CapKind::Square);
        core.set_node_opacity(0.9);
        core.project.figures[0].def.nodes[2].style.taper = 0.3;
        core.project.figures[0].def.nodes[2].thickness = 9.0;

        assert!(paste_style(&mut core));
        assert_eq!(core.status, "Style pasted.");

        // Every captured field landed on the target.
        let target = &core.project.figures[0].def.nodes[2];
        assert_eq!(target.color, Some(0x12_3456));
        assert_eq!(target.style.color2, Some(0x34_5678));
        assert_eq!(target.style.cap, CapKind::Point);
        assert_eq!(target.style.opacity, 0.55);
        // Excluded fields are exactly as they were.
        assert_eq!(target.style.taper, 0.3);
        assert_eq!(target.thickness, 9.0);
        // The source node kept its style.
        let source = &core.project.figures[0].def.nodes[1];
        assert_eq!(source.color, Some(0x12_3456));
        assert_eq!(source.style.cap, CapKind::Point);
        assert_eq!(source.style.opacity, 0.55);
    }

    #[test]
    fn paste_is_one_atomic_undo_step() {
        // Phase H: paste goes through
        // `set_node_style_atomic` (one snapshot), replacing
        // Phase G's setter-by-setter paste, which cost
        // THREE undo steps and reverted field-by-field.
        // This test pins the new count — exactly one — and
        // that a single undo reverts all four fields
        // together. (No older test pinned the 3-step count;
        // the round-trip test above is count-agnostic.)
        let mut core = EditorCore::new();
        core.selected_figure = 0;

        // Source node 1: a distinctive writable style.
        core.selected_node = Some(1);
        core.set_node_color(0x12_3456);
        core.set_node_color2(Some(0x34_5678));
        core.set_node_cap(CapKind::Point);
        core.set_node_opacity(0.55);
        copy_style(&mut core);
        assert_eq!(core.status, "Style copied.");

        // Target node 2: a different value in every field.
        core.selected_node = Some(2);
        core.set_node_color(0xFF_0000);
        core.set_node_color2(None);
        core.set_node_cap(CapKind::Square);
        core.set_node_opacity(0.9);

        let depth_before = core.undo_depth();
        assert!(paste_style(&mut core));
        assert_eq!(core.status, "Style pasted.");
        // Exactly ONE new undo step for the whole paste.
        assert_eq!(core.undo_depth(), depth_before + 1);

        // All four fields landed.
        let target = &core.project.figures[0].def.nodes[2];
        assert_eq!(target.color, Some(0x12_3456));
        assert_eq!(target.style.color2, Some(0x34_5678));
        assert_eq!(target.style.cap, CapKind::Point);
        assert_eq!(target.style.opacity, 0.55);

        // One undo reverts all four together — the
        // atomicity the old three-step paste lacked.
        core.undo();
        let target = &core.project.figures[0].def.nodes[2];
        assert_eq!(target.color, Some(0xFF_0000));
        assert_eq!(target.style.color2, None);
        assert_eq!(target.style.cap, CapKind::Square);
        assert_eq!(target.style.opacity, 0.9);
    }

    #[test]
    fn paste_with_an_empty_clipboard_changes_nothing() {
        let mut core = EditorCore::new();
        core.selected_figure = 0;
        core.selected_node = Some(1);
        core.set_node_color(0x12_3456);
        let style_before = selected_style(&core).unwrap();
        let depth_before = core.undo_depth();

        assert!(!paste_style(&mut core));
        assert_eq!(core.status, "Copy a style first.");
        // No field moved and no snapshot was taken.
        assert_eq!(selected_style(&core).unwrap(), style_before);
        assert_eq!(
            core.project.figures[0].def.nodes[1].color,
            Some(0x12_3456)
        );
        assert_eq!(core.undo_depth(), depth_before);
    }

    #[test]
    fn apply_style_without_a_selection_changes_nothing() {
        let mut core = EditorCore::new();
        core.selected_node = None;
        let clip = StyleClipboard {
            color: 0x12_3456,
            color2: Some(0x34_5678),
            cap: CapKind::Point,
            opacity: 0.55,
        };
        let status_before = core.status.clone();
        let depth_before = core.undo_depth();
        assert!(!apply_style(&mut core, &clip));
        // Not even a setter ran: status and undo history
        // are exactly as they were.
        assert_eq!(core.status, status_before);
        assert_eq!(core.undo_depth(), depth_before);
    }

    #[test]
    fn style_presets_table_is_six_distinct_colours_with_valid_caps() {
        assert_eq!(STYLE_PRESETS.len(), 6);
        // The briefed table, name by name. Violet's cap is
        // Square: the model has no Butt variant (Round,
        // Square and Point only), and Square is the other
        // flat-ended cap.
        let expected = [
            ("Ink", 0x20_242C, CapKind::Round),
            ("Crimson", 0xD2_3B3B, CapKind::Round),
            ("Ocean", 0x2B_6FD2, CapKind::Square),
            ("Forest", 0x2E_9E5B, CapKind::Round),
            ("Violet", 0x7A_4FD0, CapKind::Square),
            ("Amber", 0xE8_A33D, CapKind::Square),
        ];
        for (preset, (name, color, cap)) in STYLE_PRESETS.iter().zip(expected) {
            assert_eq!(preset.name, name);
            assert_eq!(preset.color, color);
            assert_eq!(preset.cap, cap);
            // A valid cap is one the segmented control can
            // show — CAP_ORDER holds every CapKind variant.
            assert!(CAP_ORDER.contains(&preset.cap));
        }
        // All six colours (and names) are distinct, so no
        // two chips can be confused for each other.
        for (i, a) in STYLE_PRESETS.iter().enumerate() {
            for b in &STYLE_PRESETS[i + 1..] {
                assert_ne!(a.color, b.color);
                assert_ne!(a.name, b.name);
            }
        }
    }

    #[test]
    fn preset_application_is_none_without_a_selection() {
        let mut core = EditorCore::new();
        core.selected_node = None;
        for preset in &STYLE_PRESETS {
            assert!(preset_application(&core, preset).is_none());
        }
        // And a press changes nothing at all.
        let depth_before = core.undo_depth();
        assert!(!apply_preset(&mut core, &STYLE_PRESETS[0]));
        assert_eq!(core.undo_depth(), depth_before);
    }

    #[test]
    fn preset_application_preserves_current_color2_and_opacity() {
        let mut core = EditorCore::new();
        core.selected_figure = 0;
        core.selected_node = Some(1);
        // Current state a preset must NOT touch: a
        // non-swatch gradient end and a mid opacity. The
        // current colour and cap differ from the preset's,
        // so the returned pair proves whose values won.
        core.set_node_color(0xFF_0000);
        core.set_node_color2(Some(0x34_5678));
        core.set_node_cap(CapKind::Point);
        core.set_node_opacity(0.55);

        // Ocean: colour and cap come from the preset,
        // gradient end and opacity from the current node.
        let args = preset_application(&core, &STYLE_PRESETS[2])
            .expect("node 1 is selected");
        assert_eq!(args, (0x2B_6FD2, Some(0x34_5678), CapKind::Square, 0.55));

        // With the gradient off, None passes through too.
        core.set_node_color2(None);
        let args = preset_application(&core, &STYLE_PRESETS[0])
            .expect("node 1 is selected");
        assert_eq!(args, (0x20_242C, None, CapKind::Round, 0.55));
    }

    #[test]
    fn apply_preset_changes_colour_and_cap_only_in_one_undo_step() {
        let mut core = EditorCore::new();
        core.selected_figure = 0;
        core.selected_node = Some(1);
        core.set_node_color(0xFF_0000);
        core.set_node_color2(Some(0x34_5678));
        core.set_node_cap(CapKind::Point);
        core.set_node_opacity(0.55);

        let depth_before = core.undo_depth();
        // Violet (index 4).
        assert!(apply_preset(&mut core, &STYLE_PRESETS[4]));
        assert_eq!(core.status, "Style preset: Violet.");
        assert_eq!(core.undo_depth(), depth_before + 1);

        let node = &core.project.figures[0].def.nodes[1];
        assert_eq!(node.color, Some(0x7A_4FD0));
        assert_eq!(node.style.cap, CapKind::Square);
        // Untouched by the preset: gradient end + opacity.
        assert_eq!(node.style.color2, Some(0x34_5678));
        assert_eq!(node.style.opacity, 0.55);
    }

    #[test]
    fn push_recent_puts_the_colour_at_the_front() {
        // Into an empty list.
        assert_eq!(push_recent(vec![], 0x11_1111), vec![0x11_1111]);
        // Each push lands ahead of everything before it:
        // most recent first.
        let list = push_recent(vec![], 0x11_1111);
        let list = push_recent(list, 0x22_2222);
        let list = push_recent(list, 0x33_3333);
        assert_eq!(list, vec![0x33_3333, 0x22_2222, 0x11_1111]);
    }

    #[test]
    fn push_recent_dedupes_by_moving_to_the_front() {
        let list = vec![0x33_3333, 0x22_2222, 0x11_1111];
        // Re-pushing an existing colour moves it to the
        // front; it does not duplicate it and the relative
        // order of the others survives.
        let list = push_recent(list, 0x11_1111);
        assert_eq!(list, vec![0x11_1111, 0x33_3333, 0x22_2222]);
        // Pushing the front colour again is a no-op.
        let list = push_recent(list, 0x11_1111);
        assert_eq!(list, vec![0x11_1111, 0x33_3333, 0x22_2222]);
    }

    #[test]
    fn push_recent_caps_at_eight_dropping_the_oldest() {
        let mut list = vec![];
        for i in 0..9u32 {
            list = push_recent(list, 0x10_0000 + i);
        }
        assert_eq!(list.len(), RECENT_COLORS_CAP);
        // The newest push is at the front; the first-ever
        // colour (the oldest) is the one dropped.
        assert_eq!(list[0], 0x10_0008);
        assert!(!list.contains(&0x10_0000));
        // Re-pushing an existing colour at the cap keeps
        // the length and still leaves no duplicate.
        let list = push_recent(list, 0x10_0005);
        assert_eq!(list.len(), RECENT_COLORS_CAP);
        assert_eq!(list[0], 0x10_0005);
        assert_eq!(
            list.iter().filter(|&&c| c == 0x10_0005).count(),
            1
        );
    }

    #[test]
    fn note_recent_color_records_most_recent_first() {
        // `RECENT_COLORS` is a per-thread `thread_local`
        // and each test runs on its own thread (the same
        // isolation the clipboard tests above rely on), so
        // the list starts empty here.
        assert!(recent_colors().is_empty());
        note_recent_color(0x11_1111);
        note_recent_color(0x22_2222);
        assert_eq!(recent_colors(), vec![0x22_2222, 0x11_1111]);
        // Noting an existing colour moves it to the front.
        note_recent_color(0x11_1111);
        assert_eq!(recent_colors(), vec![0x11_1111, 0x22_2222]);
    }

    #[test]
    fn recent_chip_press_path_applies_the_colour_and_refronts_it() {
        // The chip's press body, driven without the UI:
        // `set_node_color` (the palette swatch op) then
        // `note_recent_color` again.
        let mut core = EditorCore::new();
        core.selected_figure = 0;
        core.selected_node = Some(1);
        note_recent_color(0x22_2222);
        note_recent_color(0x11_1111);
        assert_eq!(recent_colors(), vec![0x11_1111, 0x22_2222]);

        core.set_node_color(0x22_2222);
        note_recent_color(0x22_2222);
        // The colour landed as the node's own colour...
        assert_eq!(selected_color(&core), Some(0x22_2222));
        assert_eq!(
            core.project.figures[0].def.nodes[1].color,
            Some(0x22_2222)
        );
        // ...and moved to the front of the recent list.
        assert_eq!(recent_colors(), vec![0x22_2222, 0x11_1111]);
    }

    #[test]
    fn preset_and_paste_record_their_colours() {
        // Preset application records the preset's colour.
        let mut core = EditorCore::new();
        core.selected_figure = 0;
        core.selected_node = Some(1);
        assert!(recent_colors().is_empty());
        assert!(apply_preset(&mut core, &STYLE_PRESETS[2])); // Ocean
        assert_eq!(recent_colors(), vec![0x2B_6FD2]);

        // A paste records the clipboard's colour, ahead of
        // the preset's. Give the clipboard a colour the
        // panel never applied — a direct core write does
        // not record (only panel handlers do) — so the
        // paste is the only site that can put it in front.
        core.set_node_color(0x12_3456);
        assert_eq!(recent_colors(), vec![0x2B_6FD2]);
        copy_style(&mut core);
        core.selected_node = Some(2);
        assert!(paste_style(&mut core));
        assert_eq!(recent_colors(), vec![0x12_3456, 0x2B_6FD2]);
    }

    #[test]
    fn recents_row_is_hidden_for_an_empty_list() {
        let t = tok::theme(true);
        // No colours recorded yet → no row at all.
        assert!(recents_row(&[], None, &t).is_none());
        assert!(recents_row(&[], Some(0x11_1111), &t).is_none());
        // A non-empty list builds a row, with or without a
        // current colour to frame.
        assert!(recents_row(&[0x11_1111], None, &t).is_some());
        assert!(recents_row(&[0x11_1111, 0x22_2222], Some(0x22_2222), &t).is_some());
    }

    #[test]
    fn gradient_stepper_drives_the_real_op_and_records() {
        // The gradient stepper buttons' press path, driven
        // without the UI: `step_gradient_channel` steps one
        // channel of the node's gradient end through
        // `set_node_color2` and records the stepped colour.
        let mut core = EditorCore::new();
        core.selected_figure = 0;
        core.selected_node = Some(1);
        core.set_node_color(0x11_2233);
        // Give the node a gradient end through the real op
        // (a direct core write records nothing — only this
        // panel's handlers do).
        core.set_node_color2(Some(0x64_320A));
        assert!(recent_colors().is_empty());
        // R +1: 100 → 108, other channels untouched, and
        // the stepped colour is recorded like a swatch
        // press.
        step_gradient_channel(&mut core, 0, 1);
        assert_eq!(selected_style(&core).unwrap().color2, Some(0x6C_320A));
        assert_eq!(recent_colors(), vec![0x6C_320A]);
        // G −1: 50 → 42, recorded ahead of the R step.
        step_gradient_channel(&mut core, 1, -1);
        assert_eq!(selected_style(&core).unwrap().color2, Some(0x6C_2A0A));
        assert_eq!(recent_colors(), vec![0x6C_2A0A, 0x6C_320A]);
        // B clamps at 0 (10 − 8 − 8 would go negative) —
        // the clamp itself is `step_channel`'s; this pins
        // that the wiring composes it into the written
        // colour.
        step_gradient_channel(&mut core, 2, -1);
        step_gradient_channel(&mut core, 2, -1);
        assert_eq!(selected_style(&core).unwrap().color2, Some(0x6C_2A00));
        // A bad channel index is a no-op.
        step_gradient_channel(&mut core, 3, 1);
        assert_eq!(selected_style(&core).unwrap().color2, Some(0x6C_2A00));
        // The gradient writes never touched the node's own
        // (primary) colour.
        assert_eq!(
            core.project.figures[0].def.nodes[1].color,
            Some(0x11_2233)
        );
    }

    #[test]
    fn gradient_stepper_is_a_no_op_without_a_gradient_end() {
        let mut core = EditorCore::new();
        core.selected_figure = 0;
        core.selected_node = Some(1);
        // The demo node starts with no gradient end: a
        // press applies nothing and records nothing.
        assert_eq!(selected_style(&core).unwrap().color2, None);
        step_gradient_channel(&mut core, 0, 1);
        assert_eq!(selected_style(&core).unwrap().color2, None);
        assert!(recent_colors().is_empty());
        // No selection at all: still a quiet no-op.
        core.selected_node = None;
        step_gradient_channel(&mut core, 0, 1);
        assert!(recent_colors().is_empty());
    }

    #[test]
    fn gradient_steppers_visible_follows_the_gradient_end() {
        let mut core = EditorCore::new();
        core.selected_figure = 0;
        // No selection → hidden.
        core.selected_node = None;
        assert!(!gradient_steppers_visible(&core));
        // Selected, but the gradient is Off → hidden.
        core.selected_node = Some(1);
        assert!(!gradient_steppers_visible(&core));
        // An end colour set → visible.
        core.set_node_color2(Some(0x22_2222));
        assert!(gradient_steppers_visible(&core));
        // Back to Off → hidden again.
        core.set_node_color2(None);
        assert!(!gradient_steppers_visible(&core));
    }
}
