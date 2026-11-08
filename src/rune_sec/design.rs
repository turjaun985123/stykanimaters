//! The Phase N design kit: one card shell, one header,
//! one row, one toggle, one slider visual — the shared
//! builders every panel adopts so the shell stops
//! re-inventing them per file (docs/PHASE_N_CHARTER.md,
//! "card + hairline"). Declared from `rune_sec/mod.rs`
//! only — bin-crate code, not part of the engine library.
//!
//! The API in this file is FIXED by the charter (plus its
//! ratified amendments); adoption agents import
//! `crate::rune_sec::design` and call these builders
//! rather than restyling by hand. Three discipline rules
//! hold throughout:
//!
//! * **Keys come from callers.** No builder invents a
//!   key; the ones that take `key` put exactly that key
//!   on the pressable/root element and nothing else.
//! * **Handlers move verbatim.** Only [`toggle_row`]
//!   and [`swatch`] take a handler, and they attach it
//!   untouched to the root — what a press does is the
//!   caller's business, exactly as with
//!   `chrome_view::btn`.
//! * **Restyle roots, never nest wrappers** (P19): a
//!   panel's card IS its existing root element passed
//!   through [`panel_col`]'s styling, not a new wrapper
//!   inside it.
//!
//! ## Rune API notes for consumers (learned building this)
//!
//! * `.hover(c)` / `.press(c)` are OVERLAYS drawn over
//!   the element's fill while hovered/pressed, not
//!   replacement fills (rune `el/visuals.rs`). That is
//!   why an unselected [`list_row`] carries no resting
//!   fill at all — the `t.hover` overlay alone paints
//!   the highlight, straight over the panel card.
//! * `Color::alpha(a)` REPLACES the alpha channel; it
//!   does not multiply. [`selected_fill`] and
//!   [`text_muted`] rely on that.
//! * `type_::MONO_CAPTION` does not set the Mono family
//!   in the pinned Rune rev — the token is metrically a
//!   LABEL. [`kbd`] and [`stepper_value`] therefore
//!   apply `.mono()` on top of the token (charter
//!   amendment, P9/P11).
//! * `icons::icon_el` degrades an unknown name to an
//!   empty spacer rather than omitting the slot.
//!   [`panel_title`] and [`empty_state`] check
//!   `icons::has_icon` first and omit the glyph entirely
//!   when the name is unknown.
//! * Element internals are opaque to bin-crate unit
//!   tests (the Phase G §12 finding), so this file's
//!   tests pin the pure colour maths and the frac clamp,
//!   and smoke-test that every builder constructs on
//!   both of its branches — layout truth comes from the
//!   shell's Runtime/headless checks, as elsewhere.
//!
//! ## UI mission wave 2 — kit states & one retirement
//!
//! * [`list_row`] / [`row_shell`] gained a press
//!   overlay, [`swatch`] gained hover/press overlays,
//!   and [`toggle_row`] gained hover/press feedback
//!   (audit S17 items I-3/I-4/I-5) — all from the
//!   theme's overlay tokens (`t.hover` / `t.press`), so
//!   the kit's state language stays one derivation.
//! * [`kbd_on_accent`] is the shortcut chip for accent
//!   fills, where the plain [`kbd`] measures ~1.18:1
//!   (audit S18); its ink matches the menus' on-accent
//!   hint treatment (white at 75% alpha).
//! * **Retired: the bare `toggle` builder.** It had
//!   zero production callers — every settings/onion
//!   toggle in the shell is a labelled [`toggle_row`],
//!   which remains the kit's one toggle form, over the
//!   same [`toggle_pill`] visual the retired builder
//!   wrapped. [`text_muted`] was reviewed in the same
//!   pass and is KEPT: it is the kit's muted-text role
//!   (see its own docs).
//!
//! ## UI mission wave 4 — the tangent preset chip
//!
//! * [`tangent_preset_chip`] lifts the graph panel's
//!   private `preset_chip` (audit S12-09: the canonical
//!   of the three co-mounted tangent-preset copies)
//!   into the kit: the preset's real preview curve over
//!   its name, one pressable card. The keyframe tools'
//!   bare text buttons (`key-preset-*`) and the easing
//!   panel's preview-plus-button rows (`ease-graph-*`)
//!   now build through it, keeping their own keys,
//!   labels and handlers. **The graph panel keeps its
//!   private copy this wave** — its chip and this one
//!   are the same construction by design, and merging
//!   them is a later, graph-owned step; do not "fix"
//!   the duplication from a consumer side.
//!
//! ## UI mission wave 6 — tooltip adoption
//!
//! * Under the convention documented in
//!   `crate::rune_sec::icon_btn`'s module docs (a
//!   builder that receives a human label wires it as
//!   the element's help text), this file's two
//!   label-receiving pressables adopt it:
//!   [`toggle_row`] and [`tangent_preset_chip`].
//! * The remaining pressables here carry NO help, and
//!   that is deliberate, not an oversight: [`swatch`]
//!   receives a bare `Color` — it has no colour name
//!   to show, and a hex string would be junk, not a
//!   tooltip (naming swatches is audit S18-10
//!   call-site work); [`list_row`] / [`row_shell`]
//!   receive no label at all (their content arrives
//!   as children, and their only string is the
//!   caller's key — an internal id); [`slider_track`]
//!   is a visual with no key, handler or text — the
//!   caller's drag root owns whatever naming it gets.
//!   The display-only members ([`kbd`],
//!   [`panel_title`], [`empty_state`], …) are not
//!   controls and stay unwired.

use rune::tok::{self, type_, Theme};
use rune::{canvas, col, el, row, text, Color, El, Shape};
use stykanimate_rs::graph::TangentPreset;
use stykanimate_rs::graph_panel::preset_preview;

use crate::rune_sec::icons;
use crate::State;

/// Panel card corner radius (tok LG).
pub const PANEL_RADIUS: f32 = 12.0;
/// Panel card padding on all sides (tok S3).
pub const PANEL_PAD: f32 = 12.0;
/// Internal gap between a panel card's children (tok S2).
pub const PANEL_GAP: f32 = 8.0;
/// List-row corner radius (tok MD).
pub const ROW_RADIUS: f32 = 8.0;
/// Small-control corner radius (tok SM): buttons,
/// swatches, chips.
pub const CONTROL_RADIUS: f32 = 6.0;

/// The shell's one accent colour — the same
/// `tok::accent(0, true)` `chrome_view::btn` fills its
/// active state with, named once here so kit builders
/// and panels agree on it.
pub fn accent() -> Color {
    tok::accent(0, true)
}

/// The selected-list-row fill: the accent at 16% alpha
/// (charter §5). Painted over the panel card, it reads
/// as a tint, not a solid block.
pub fn selected_fill() -> Color {
    accent().alpha(0.16)
}

/// The third text level (charter amendment, P2):
/// `text_secondary` at 0.6 alpha, for text that should
/// sit below even the secondary tone. Derived from the
/// theme — never a new hex literal.
///
/// This is the kit's muted-text role (UI mission
/// wave 2, kept on review): the colour for text that
/// must read as present-but-quiet — hints, retired
/// values, secondary annotations — as distinct from
/// `section_label`'s secondary tone. New muted text in
/// kit consumers should take this role rather than
/// re-deriving the alpha locally.
pub fn text_muted(t: &Theme) -> Color {
    t.text_secondary.alpha(0.6)
}

/// A panel's card root: a `col` carrying the panel's
/// existing root `key`, restyled in place — card fill,
/// hairline border, LG radius, S3 padding, S2 internal
/// gap, full width (charter §1). Children are appended
/// by the caller; the first is normally a [`header_row`]
/// holding a [`panel_title`], followed by a [`divider`].
pub fn panel_col(key: &str, t: &Theme) -> El {
    col()
        .key(key)
        .w_full()
        .fill(t.card)
        .border(1.0, t.card_border)
        .radius(PANEL_RADIUS)
        .padding(PANEL_PAD)
        .gap(PANEL_GAP)
}

/// The header row shell for a panel card: full width,
/// centre-aligned, S2 gap, carrying the caller's `key`
/// (existing header keys survive byte-identical — e.g.
/// `figures-panel-header`). Typically holds one
/// [`panel_title`] cluster followed by trailing
/// controls.
pub fn header_row(key: &str) -> El {
    row().key(key).w_full().gap(PANEL_GAP).center_y()
}

/// A panel header's growing left cluster: an optional
/// 13 pt glyph in `text_secondary` (omitted when `icon`
/// is `None` or names no curated icon), the title in
/// SECTION / `t.text`, and an optional meta/count in
/// LABEL / `text_secondary`. The cluster grows, so
/// controls appended after it in a [`header_row`] sit
/// at the right edge (charter §2).
pub fn panel_title(icon: Option<&str>, title: &str, meta: Option<&str>, t: &Theme) -> El {
    let mut cluster = row().grow(1.0).gap(PANEL_GAP).center_y();
    if let Some(name) = icon {
        if icons::has_icon(name) {
            cluster = cluster.child(icons::icon_el(name, 13.0, t.text_secondary));
        }
    }
    cluster = cluster.child(text(title).font(type_::SECTION).color(t.text));
    if let Some(meta) = meta {
        cluster = cluster.child(text(meta).font(type_::LABEL).color(t.text_secondary));
    }
    cluster
}

/// A section label inside a panel: SECTION in
/// `text_secondary` (charter §3). Replaces the five-plus
/// private `section()` / `section_label()` helpers the
/// panels grew before the kit.
pub fn section_label(label: &str, t: &Theme) -> El {
    text(label).font(type_::SECTION).color(t.text_secondary)
}

/// A hairline divider: 1 pt tall, full width, in the
/// theme's separator colour. One per panel, under the
/// header (P19: section separation comes from labels +
/// gap, not more hairlines).
pub fn divider(t: &Theme) -> El {
    el().h(1.0).w_full().fill(t.separator)
}

/// [`divider`] carrying a key — menu separators are
/// keyed elements (`menu-file-sep`, …) whose keys must
/// survive adoption (charter amendment, P7).
pub fn divider_keyed(key: &str, t: &Theme) -> El {
    divider(t).key(key)
}

/// A keyboard-shortcut chip: px 5 / py 1, radius 4,
/// `control_off` fill, the label in MONO_CAPTION with
/// the Mono family applied, `text_secondary` ink
/// (charter §11). Used by help rows, menu hints and the
/// palette footer; supersedes help_panel's local
/// `key_cap`.
pub fn kbd(label: &str, t: &Theme) -> El {
    el()
        .px(5.0)
        .py(1.0)
        .radius(4.0)
        .fill(t.control_off)
        .child(
            text(label)
                .font(type_::MONO_CAPTION.mono())
                .color(t.text_secondary),
        )
}

/// [`kbd`] for accent fills (UI mission wave 2, audit
/// S17/S18): the command palette's selected row fills
/// with the accent at 0.85, where the plain chip —
/// `control_off` fill, `text_secondary` ink — measures
/// ~1.18:1 and is effectively unreadable. This variant
/// keeps the chip (same padding, radius and mono
/// caption) and takes its colours from the treatment
/// the menus already use for hints on their active
/// (accent) row: white ink at 75% alpha (menus.rs
/// `hint_el`), here over a white fill at 16% alpha so
/// the chip still reads as a chip on the accent. The
/// colours are white-derived, not theme-derived, so
/// `t` is accepted only for signature parity with
/// [`kbd`] — call sites can swap the two freely.
pub fn kbd_on_accent(label: &str, t: &Theme) -> El {
    let _ = t;
    el()
        .px(5.0)
        .py(1.0)
        .radius(4.0)
        .fill(Color::WHITE.alpha(0.16))
        .child(
            text(label)
                .font(type_::MONO_CAPTION.mono())
                .color(Color::WHITE.alpha(0.75)),
        )
}

/// A panel's empty state: a centred 20 pt glyph over a
/// LABEL message, both in `text_secondary`, with 12 pt
/// of vertical air (charter §10). An unknown icon name
/// omits the glyph (the message still renders) rather
/// than leaving a blank slot. Replaces the bare
/// "nothing here" text rows panels used to hand-roll.
pub fn empty_state(icon: &str, message: &str, t: &Theme) -> El {
    let mut body = col().w_full().py(12.0).gap(PANEL_GAP).items_center();
    if icons::has_icon(icon) {
        body = body.child(icons::icon_el(icon, 20.0, t.text_secondary));
    }
    // The message WRAPS at the card's content width
    // (UI mission wave 2): a centred unwrapped line
    // wider than its card spills past BOTH edges — the
    // clips empty state measured 442pt in a 316pt card
    // (+63 past the edge, every Animate tree) and the
    // particles one +17.8. `.wrap().w_full()` is the
    // shell's proven prose treatment (settings'
    // note_line); short messages are unaffected.
    body.child(
        text(message)
            .font(type_::LABEL)
            .color(t.text_secondary)
            .wrap()
            .w_full(),
    )
}

/// The shared list-row styling (charter §5): min
/// height 28, px 8, MD radius, centre-aligned, S2 gap.
/// Selected rows fill with [`selected_fill`]; unselected
/// rows rest transparent on the card and paint the
/// `t.hover` overlay while hovered (see the module docs
/// on overlays). Both branches paint the `t.press`
/// overlay while pressed (UI mission wave 2, audit S17
/// item I-3) — the press treatment the kit's button
/// families already have, in the theme's overlay token
/// so the row's fill (or the card under it) still shows
/// through. No handler — callers chain `.on_press`
/// or children onto the returned element.
fn row_base(selected: bool, t: &Theme) -> El {
    let base = row()
        .min_h(28.0)
        .px(8.0)
        .radius(ROW_RADIUS)
        .center_y()
        .gap(PANEL_GAP)
        .press(t.press);
    if selected {
        base.fill(selected_fill())
    } else {
        base.hover(t.hover)
    }
}

/// A styled list row carrying the caller's `key` —
/// see [`row_base`] for the styling contract.
pub fn list_row(key: &str, selected: bool, t: &Theme) -> El {
    row_base(selected, t).key(key)
}

/// [`list_row`] without a key, for the row kinds that
/// have no key of their own (text segment rows —
/// charter amendment, P8).
pub fn row_shell(selected: bool, t: &Theme) -> El {
    row_base(selected, t)
}

/// Toggle track width (charter §7).
const TOGGLE_W: f32 = 28.0;
/// Toggle track height.
const TOGGLE_H: f32 = 16.0;
/// Toggle knob diameter.
const TOGGLE_KNOB: f32 = 12.0;
/// Knob inset from the track's edge, both ends.
const TOGGLE_INSET: f32 = (TOGGLE_H - TOGGLE_KNOB) / 2.0;

/// The toggle's pill visual, with no key and no
/// handler: a 28×16 track (accent when on,
/// `control_off` when off, radius 999) with a 12 pt
/// `t.knob` dot inset at the left (off) or right (on)
/// end. The knob is an absolutely-positioned child —
/// the same construction the shell's sliders use — so
/// its travel is exact, not spacer arithmetic.
///
/// The track carries the `t.hover` overlay (UI mission
/// wave 2, audit S17 item I-5), so the switch itself —
/// not just its row — answers the pointer. Known
/// limit, recorded not faked: the knob does NOT slide
/// between its two stops; it teleports, because
/// nothing in the pinned Rune rev animates a layout
/// offset, and this kit does not fake motion. If knob
/// motion is ever added, it must ride the shell's
/// spring pairs (`tok::motion::pair::SNAP` /
/// `SETTLE`) and no other curve.
fn toggle_pill(on: bool, t: &Theme) -> El {
    let knob_x = if on {
        TOGGLE_W - TOGGLE_INSET - TOGGLE_KNOB
    } else {
        TOGGLE_INSET
    };
    el()
        .size(TOGGLE_W, TOGGLE_H)
        .radius(999.0)
        .fill(if on { accent() } else { t.control_off })
        .hover(t.hover)
        .child(
            el()
                .size(TOGGLE_KNOB, TOGGLE_KNOB)
                .absolute(knob_x, TOGGLE_INSET)
                .radius(999.0)
                .fill(t.knob),
        )
}

/// A labelled toggle row: min height 28, pressable
/// root carrying the caller's `key` and handler — a
/// press anywhere on the row flips the setting. The
/// LABEL text in `t.text` grows; the pill at the right
/// is the [`toggle_pill`] visual and is NOT separately
/// pressable. The row itself paints the `t.hover` /
/// `t.press` overlays (UI mission wave 2, audit S17
/// item I-5), matching [`list_row`]'s state treatment
/// — before this, the kit's one toggle form gave no
/// interaction feedback at all.
pub fn toggle_row(
    key: &str,
    label: &str,
    on: bool,
    t: &Theme,
    f: impl Fn(&mut State) + 'static,
) -> El {
    row()
        .key(key)
        .help(label)
        .min_h(28.0)
        .center_y()
        .gap(PANEL_GAP)
        .hover(t.hover)
        .press(t.press)
        .on_press(f)
        .child(text(label).font(type_::LABEL).color(t.text).grow(1.0))
        .child(toggle_pill(on, t))
}

/// A stepper's value readout (charter §6): the value
/// in MONO_CAPTION with the Mono family applied,
/// `t.text` ink, centred in a box at least 34 pt wide
/// so the − / + buttons either side of it never shift
/// as the value's digit count changes.
pub fn stepper_value(value: &str, t: &Theme) -> El {
    el().min_w(34.0).center().child(
        text(value)
            .font(type_::MONO_CAPTION.mono())
            .color(t.text),
    )
}

/// Slider geometry — the shell's established drag
/// slider proportions (scene panel's weather sliders,
/// layers' opacity slider), so a kit track feels
/// identical to the sliders already in the shell.
const SLIDER_W: f32 = 200.0;
/// Slider element height.
const SLIDER_H: f32 = 22.0;
/// Slider knob diameter.
const SLIDER_KNOB: f32 = 16.0;

/// Clamp a slider fraction into [0, 1]; a non-finite
/// input (a NaN out of some drag maths) reads as 0
/// rather than poisoning the layout.
fn clamp_frac(frac: f32) -> f32 {
    if frac.is_finite() {
        frac.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// A slider's visual track (charter §8): 200×22, a
/// 4 pt `control_track` rail, the accent fill running
/// from the left knob centre to the value's knob
/// centre, and a 16 pt `t.knob` dot at the value —
/// the scene/layers slider construction, restyled to
/// the kit tokens. VISUAL ONLY: no key, no handler.
/// The caller's own root keeps its key, size and
/// `.on_drag` and takes this as its child, exactly as
/// the weather and opacity sliders are built today.
/// Unipolar: the wind slider's bipolar centre-fill is
/// a documented exception that stays local (P9 D2).
pub fn slider_track(frac: f32, t: &Theme) -> El {
    let frac = clamp_frac(frac);
    let travel = SLIDER_W - SLIDER_KNOB;
    let knob_cx = SLIDER_KNOB / 2.0 + frac * travel;
    el()
        .size(SLIDER_W, SLIDER_H)
        .child(
            el()
                .size(SLIDER_W, 4.0)
                .absolute(0.0, (SLIDER_H - 4.0) / 2.0)
                .radius(2.0)
                .fill(t.control_track),
        )
        .child(
            el()
                .size((knob_cx - SLIDER_KNOB / 2.0).max(4.0), 4.0)
                .absolute(SLIDER_KNOB / 2.0, (SLIDER_H - 4.0) / 2.0)
                .radius(2.0)
                .fill(accent()),
        )
        .child(
            el()
                .size(SLIDER_KNOB, SLIDER_KNOB)
                .absolute(frac * travel, (SLIDER_H - SLIDER_KNOB) / 2.0)
                .radius(SLIDER_KNOB / 2.0)
                .fill(t.knob),
        )
}

/// One colour swatch construction for the whole shell
/// (charter amendment, P3/P18): a 24×24 pressable
/// carrying the caller's `key` and handler, radius
/// [`CONTROL_RADIUS`], filled with `color`. Selected
/// swatches get a 2 pt accent border; unselected ones
/// get a 1 pt `card_border` hairline so pale colours
/// stay visible against the panel card. Hover and
/// press paint the theme's `t.hover` / `t.press`
/// OVERLAYS (UI mission wave 2, audit S17 item I-4),
/// so the swatch's own colour stays visible under the
/// feedback — before this, a press gave no signal at
/// all until the border changed after the fact.
///
/// TOOLTIP: none, deliberately (UI mission wave 6).
/// The kit's tooltip convention wires a builder's
/// human label as help text, but this builder receives
/// a bare `Color` — no colour name reaches it, and the
/// only derivable string is the hex value, which is
/// junk as a tooltip. Naming swatches means call
/// sites passing their palette names in (audit S18-10
/// adoption); do not "fix" this from the kit side with
/// a hex string.
pub fn swatch(
    key: &str,
    color: Color,
    selected: bool,
    t: &Theme,
    f: impl Fn(&mut State) + 'static,
) -> El {
    let base = el()
        .key(key)
        .size(24.0, 24.0)
        .radius(CONTROL_RADIUS)
        .fill(color)
        .hover(t.hover)
        .press(t.press)
        .on_press(f);
    if selected {
        base.border(2.0, accent())
    } else {
        base.border(1.0, t.card_border)
    }
}

/// Width of a tangent-preset chip's preview box — the
/// graph panel chip's box, carried into the kit with
/// the construction (see [`tangent_preset_chip`]).
pub const TANGENT_PREVIEW_W: f32 = 52.0;
/// Height of a tangent-preset chip's preview box.
pub const TANGENT_PREVIEW_H: f32 = 30.0;
/// Samples along a tangent-preset chip's preview curve
/// (the graph panel chip's count).
const TANGENT_PREVIEW_SAMPLES: usize = 24;

/// A tangent preset's preview curve as pixel points in
/// a `w` × `h` box: [`preset_preview`]'s normalised
/// 0.0..=1.0 samples with y flipped (value 1.0 at the
/// TOP, screen convention), x spread evenly edge to
/// edge. The graph panel's `preview_points` maps the
/// same way; the two agree point for point.
fn tangent_preview_points(
    preset: TangentPreset,
    w: f32,
    h: f32,
    samples: usize,
) -> Vec<[f32; 2]> {
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

/// One tangent-preset chip: the preset's real preview
/// curve (a tiny canvas, sampled from the engine's own
/// [`preset_preview`]) over its name, the whole card
/// pressable under the caller's `key`, with the
/// caller's handler attached untouched (the graph
/// chip's metrics: padding 4, gap 2,
/// [`CONTROL_RADIUS`]).
///
/// This is the graph panel's private `preset_chip`
/// lifted into the kit (UI mission wave 4, audit
/// S12-09 — the graph chips are the canonical surface:
/// the only copy whose presentation carries the drawn
/// preview). The keyframe tools and the easing panel
/// adopt it for their preset rows, keeping their own
/// key families (`key-preset-*`, `ease-graph-*`),
/// labels and `apply_graph_ease_preset` handlers —
/// only the construction converges.
///
/// `active` is the generalisation the graph original
/// does not need (presets there are actions, not
/// state, so its chips never highlight): an active
/// chip takes the button family's accent treatment —
/// accent fill in every state, curve and label in
/// white — exactly as `chrome_view::btn` and the
/// easing picker's cells render their active state.
/// An inactive chip is the graph chip verbatim: card
/// fill, `control_track` / `control_off` hover/press,
/// curve and label in `t.text`.
pub fn tangent_preset_chip(
    key: &str,
    preset: TangentPreset,
    label: &str,
    active: bool,
    t: &Theme,
    f: impl Fn(&mut State) + 'static,
) -> El {
    let ink = if active { Color::WHITE } else { t.text };
    let shape = Shape::new().stroke(
        tangent_preview_points(
            preset,
            TANGENT_PREVIEW_W,
            TANGENT_PREVIEW_H,
            TANGENT_PREVIEW_SAMPLES,
        ),
        1.5,
        ink,
    );
    let chip = col()
        .key(key)
        .help(label)
        .padding(4.0)
        .gap(2.0)
        .radius(CONTROL_RADIUS)
        .center()
        .on_press(f)
        .child(canvas(shape).size(TANGENT_PREVIEW_W, TANGENT_PREVIEW_H))
        .child(text(label).font(type_::LABEL).color(ink));
    if active {
        chip.fill(accent()).hover(accent()).press(accent())
    } else {
        chip.fill(t.card).hover(t.control_track).press(t.control_off)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accent_is_the_shells_button_accent() {
        // The same token call chrome_view::btn makes;
        // if the kit accent ever drifts from the button
        // family's, active buttons and selected rows
        // would stop matching.
        assert_eq!(accent(), tok::accent(0, true));
    }

    #[test]
    fn selected_fill_is_accent_at_sixteen_percent() {
        let fill = selected_fill();
        let a = accent();
        assert_eq!(fill.a, 0.16);
        assert_eq!((fill.r, fill.g, fill.b), (a.r, a.g, a.b));
    }

    #[test]
    fn text_muted_is_secondary_at_sixty_percent() {
        for dark in [true, false] {
            let t = tok::theme(dark);
            let muted = text_muted(&t);
            assert_eq!(muted.a, 0.6);
            assert_eq!(
                (muted.r, muted.g, muted.b),
                (t.text_secondary.r, t.text_secondary.g, t.text_secondary.b)
            );
            // A real third level: dimmer than secondary,
            // not a restatement of it.
            assert_ne!(muted, t.text_secondary);
        }
    }

    #[test]
    fn clamp_frac_bounds_and_rejects_non_finite() {
        assert_eq!(clamp_frac(-0.5), 0.0);
        assert_eq!(clamp_frac(0.0), 0.0);
        assert_eq!(clamp_frac(0.25), 0.25);
        assert_eq!(clamp_frac(1.0), 1.0);
        assert_eq!(clamp_frac(1.5), 1.0);
        assert_eq!(clamp_frac(f32::NAN), 0.0);
        assert_eq!(clamp_frac(f32::INFINITY), 0.0);
        assert_eq!(clamp_frac(f32::NEG_INFINITY), 0.0);
    }

    /// Every builder constructs on both of its branches
    /// (element internals are opaque to a bin-crate unit
    /// test, so — as in icon_btn's tests — these pin
    /// that nothing panics; layout truth is the Runtime
    /// tests' job).
    #[test]
    fn builders_construct_on_all_branches() {
        for dark in [true, false] {
            let t = tok::theme(dark);
            let _ = panel_col("p", &t);
            let _ = header_row("h");
            // Known icon, unknown icon, no icon; meta and
            // no meta.
            let _ = panel_title(Some("play"), "Title", Some("3 items"), &t);
            let _ = panel_title(Some("no-such-icon"), "Title", None, &t);
            let _ = panel_title(None, "Title", Some("meta"), &t);
            let _ = section_label("Section", &t);
            let _ = divider(&t);
            let _ = divider_keyed("d-sep", &t);
            let _ = kbd("Ctrl+K", &t);
            let _ = kbd_on_accent("Ctrl+K", &t);
            let _ = empty_state("play", "Nothing here", &t);
            let _ = empty_state("no-such-icon", "Nothing here", &t);
            let _ = list_row("row", true, &t);
            let _ = list_row("row", false, &t);
            let _ = row_shell(true, &t);
            let _ = row_shell(false, &t);
            let _ = toggle_row("tog-row", "Loop", true, &t, |_| {});
            let _ = toggle_row("tog-row", "Loop", false, &t, |_| {});
            let _ = stepper_value("42", &t);
            let _ = slider_track(0.0, &t);
            let _ = slider_track(0.5, &t);
            let _ = slider_track(1.0, &t);
            let _ = slider_track(f32::NAN, &t);
            let _ = swatch("sw", Color::WHITE, true, &t, |_| {});
            let _ = swatch("sw", Color::BLACK, false, &t, |_| {});
            let _ = tangent_preset_chip(
                "tp",
                TangentPreset::EaseInOut,
                "Ease in-out",
                true,
                &t,
                |_| {},
            );
            let _ = tangent_preset_chip(
                "tp",
                TangentPreset::Linear,
                "Linear",
                false,
                &t,
                |_| {},
            );
        }
    }

    #[test]
    fn tangent_preview_points_span_the_box_edge_to_edge() {
        // The chip's curve is the graph panel's mapping:
        // x runs 0..=w across the samples, y is the
        // flipped normalised value — a preset running
        // 0→1 starts at the box's bottom-left and ends
        // at its top-right.
        for preset in [
            TangentPreset::Smooth,
            TangentPreset::EaseIn,
            TangentPreset::EaseOut,
            TangentPreset::EaseInOut,
            TangentPreset::Linear,
        ] {
            let pts = tangent_preview_points(
                preset,
                TANGENT_PREVIEW_W,
                TANGENT_PREVIEW_H,
                TANGENT_PREVIEW_SAMPLES,
            );
            assert_eq!(pts.len(), TANGENT_PREVIEW_SAMPLES);
            let first = pts.first().expect("samples exist");
            let last = pts.last().expect("samples exist");
            assert_eq!(first[0], 0.0);
            assert!(
                (first[1] - TANGENT_PREVIEW_H).abs() < 0.05,
                "{preset:?}: starts at y {:?}, not the box bottom",
                first[1]
            );
            assert_eq!(last[0], TANGENT_PREVIEW_W);
            assert!(
                last[1].abs() < 0.05,
                "{preset:?}: ends at y {:?}, not the box top",
                last[1]
            );
        }
    }
}
