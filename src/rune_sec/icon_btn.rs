//! Icon-bearing buttons for the Rune editor shell (Phase H,
//! section 3). Declared from `rune_sec/mod.rs` only — bin-crate
//! code, not part of the engine library.
//!
//! Why this element exists: the shell's button —
//! `crate::chrome_view::btn` (chrome_view.rs:37), the helper
//! every `rune_sec` panel imports — renders a SINGLE text run;
//! there is no icon slot. Panels that want a Font Awesome
//! glyph beside (or instead of) a label use the two builders
//! here, which mirror `btn`'s styling value for value so an
//! icon button is indistinguishable from a text one:
//!
//! * padding `.px(10.0).py(5.0)`, radius `6.0`
//!   (chrome_view.rs:44-46);
//! * fill: active → `tok::accent(0, true)`, inactive →
//!   `t.card`; hover: active → accent, inactive →
//!   `t.control_track`; press: active → accent, inactive →
//!   `t.control_off` (chrome_view.rs:47-49);
//! * label in `type_::LABEL` (12 pt / 500, rune tok.rs:139),
//!   coloured `Color::WHITE` when active, `t.text` when not
//!   (chrome_view.rs:52-55) — the glyph takes the SAME colour
//!   as the label in each state, via [`glyph_color`].
//!
//! The handler type matches `btn`'s exactly:
//! `impl Fn(&mut State) + 'static`, attached with `.on_press`
//! on the root element.
//!
//! The glyph itself is drawn by section 2's
//! `crate::rune_sec::icons::icon_el(name, size, colour)`;
//! this file owns no path data.
//!
//! FALLBACK RULE: an icon name the catalogue doesn't know
//! must never break a panel. [`icon_btn`] checks
//! [`use_icon`] first and, when the name is unknown, returns
//! the plain `btn(key, label, active, t, on_press)` — same
//! key, same label, same handler, just no glyph.
//! [`icon_only_btn`] renders no text, so there is no text
//! form to fall back to: an unknown name yields the same
//! square button with no glyph child: right size, right
//! key, still pressable (its label still rides as help).
//!
//! INERT VARIANTS (Phase I, section 11): some panels gate a
//! button on state its op would no-op without — a pose
//! paste with an empty clipboard, a 3D op outside a
//! session. Those panels used to grow local muted wrappers
//! (pose_tools' `pibtn`, three_d_panel's `ictl`) or lose
//! the disabled tint entirely (frame_ops, keyframe_tools).
//! [`icon_btn_inert`] and [`icon_only_btn_inert`] are the
//! shared form they migrate to: the SAME box as the live
//! builder (same padding, radius, gap, glyph size), resting
//! on the card fill with glyph and label in the muted
//! colour ([`glyph_color_muted`]), and NO press handler at
//! all — pressing one does nothing because there is nothing
//! attached, which is stronger than a wired handler that
//! refuses: no status churn, no accidental snapshot.
//!
//! PHASE N ADDITIONS (foundation F3): the two LIVE builders
//! gain `.press_scale(0.97)` (plan P15, item M1 — the
//! spring-back press feedback Rune disables under Reduce
//! Motion; layout and hit-testing keep the resting size,
//! and the inert twins have no press to feed back, so they
//! stay untouched). The family also grows the two members
//! the Phase N charter's amendments name in this file:
//! [`chip_btn`], the compact chip that absorbs the panels'
//! local compact clones (`row_toggle`, `cap_chip`,
//! `preset_chip`, `picker_cell`), and [`danger_icon_btn`],
//! the icon form of `crate::chrome_view::danger_btn`,
//! styled from the family constants
//! `chrome_view::DANGER_FILL` / `DANGER_HOVER` /
//! `DANGER_PRESS` — retiring inspector_segment's local
//! hex-literal copy. (The text-only family members —
//! `btn`, `btn_inert`, `btn_muted_live`, `danger_btn` —
//! live in chrome_view.rs; this file owns the icon forms.)
//!
//! UI MISSION WAVE 2 ADDITIONS: [`chip_btn_inert`], the
//! chip's inert twin — the one family member the S17
//! state matrix found missing, which panels had been
//! faking with live chips that refuse — and the named
//! icon size scale ([`ICON_SM`] / [`ICON_MD`] /
//! [`ICON_LG`] / [`ICON_XL`]) the adoption waves
//! converge direct `icon_el` call sites onto.
//!
//! TOOLTIP CONVENTION (UI mission wave 6, audit S17
//! item I-1 / defect D2): Rune renders tooltips
//! natively from an element's help text — no app-side
//! wiring, and the text rides the `--tree` dump, so
//! adoption is headlessly verifiable. The rule for
//! this kit:
//!
//! * **A builder that receives a human label wires
//!   that label as the element's help text.** Every
//!   call site then inherits a tooltip by construction,
//!   with no signature change and no geometry change
//!   (help is metadata; Rune's layout never reads it).
//!   In this file: [`icon_btn`], [`icon_btn_inert`],
//!   [`chip_btn`], [`chip_btn_inert`] and
//!   [`danger_icon_btn`]; the text family in
//!   chrome_view.rs follows the same rule, so the
//!   fallback branches that delegate to it inherit too.
//! * **Icon-only controls MUST carry help** — a glyph
//!   with no visible label is undiscoverable without
//!   one. [`icon_only_btn`] and
//!   [`icon_only_btn_inert`] therefore take a label
//!   parameter (UI mission wave 6-A2, the builder
//!   change audit S18-8 specifies): it is wired as
//!   help text exactly like the labelled builders'
//!   labels, and rendered nowhere — the square stays
//!   icon-only on screen. The label is the CALL
//!   SITE's word for the action, never derived from
//!   the two strings the builders already had: the
//!   key is an internal id (`fig-eye-0`), and the
//!   icon name is a catalogue identifier
//!   (`trash-can`, `arrow-rotate-left`) whose meaning
//!   is state-dependent besides — the same `eye`
//!   glyph means show on one row and hide on
//!   another, so a state-dependent call site passes
//!   the label for the state it is rendering (the
//!   press performs the named action, the
//!   audio-panel mute-toggle precedent).
//! * Labelled controls SHOULD additionally carry
//!   shortcut hints (`help_keys`) where a shortcut
//!   exists — but a builder cannot know its call
//!   site's shortcut, so that half of I-1 stays with
//!   the call sites.

use rune::tok::{self, type_, Theme};
use rune::{el, row, text, Color, El};

use crate::chrome_view::{btn, danger_btn};
use crate::rune_sec::icons;
use crate::stage_view::hex_color;
use crate::State;

/// Glyph edge length (pt) in [`icon_btn`] — close to the
/// 12 pt label's cap height, so glyph and text sit on one
/// visual line.
pub const ICON_SIZE: f32 = 13.0;

/// Horizontal gap (pt) between glyph and label in
/// [`icon_btn`]. The kit button has no icon slot, so there
/// is no kit value to mirror; 6 pt matches the gap the
/// shell's dense rows use between small controls.
pub const ICON_LABEL_GAP: f32 = 6.0;

/// Press feedback scale for every LIVE builder in this
/// file (Phase N, plan P15 item M1): the root springs to
/// 97% while pressed and back on release. Rune keeps the
/// resting layout and hit area while scaled, and turns
/// the effect off entirely under the host's Reduce Motion
/// setting. The inert twins carry no press handler and
/// deliberately do NOT scale — there is no press to feed
/// back.
pub const PRESS_SCALE: f32 = 0.97;

/// The kit's icon size scale (UI mission wave 2, from
/// the S17 census of sizes the kit itself already
/// uses): direct `icon_el` call sites in panels had
/// drifted across 11/12/13/14/16/20 pt with no named
/// scale — one search glyph alone at four sizes. New
/// call sites pick a step here instead of a bare
/// literal; existing sites converge in the adoption
/// waves. (`glyph_size_for`'s cap still applies inside
/// the square buttons.)
///
/// Small: compact/inline glyphs — the [`chip_btn`]
/// glyph step ([`CHIP_GLYPH_SIZE`]), inline row icons.
pub const ICON_SM: f32 = 12.0;

/// Medium: the standard button/panel glyph —
/// [`ICON_SIZE`], also `design::panel_title`'s 13 pt.
pub const ICON_MD: f32 = 13.0;

/// Large: hero glyphs — the command palette's 16 pt
/// row glyphs are the measured use.
pub const ICON_LG: f32 = 16.0;

/// Extra large: display glyphs — `design::empty_state`'s
/// 20 pt is the measured use.
pub const ICON_XL: f32 = 20.0;

/// Whether `icon` names a glyph the icon catalogue can
/// draw — the fallback decision for both builders, factored
/// out so it is unit-testable without building elements.
pub fn use_icon(icon: &str) -> bool {
    icons::has_icon(icon)
}

/// The colour a button's foreground (label text AND glyph)
/// takes in each state — `btn`'s own choice
/// (chrome_view.rs:54): white on the accent fill, the
/// theme's text colour on the card fill.
pub fn glyph_color(active: bool, t: &Theme) -> Color {
    if active {
        Color::WHITE
    } else {
        t.text
    }
}

/// Resting fill, mirroring `btn` (chrome_view.rs:47).
pub fn fill_color(active: bool, t: &Theme) -> Color {
    if active {
        tok::accent(0, true)
    } else {
        t.card
    }
}

/// The muted foreground (glyph AND label) of the inert
/// builders. Rune's `Theme` exposes no `text_muted` field;
/// its muted-text token is `text_secondary` — the colour
/// the shell's headings already use, and the one the local
/// wrappers this file replaces (pose_tools' `pibtn`,
/// three_d_panel's `ictl`, frame_ops' muted `fbtn` branch)
/// all chose for their dimmed state. One helper, so every
/// migrated panel dims to exactly the same tone.
pub fn glyph_color_muted(t: &Theme) -> Color {
    t.text_secondary
}

/// The inert builders' fill: the live button's RESTING
/// fill (`t.card`, [`fill_color`] with `active == false`)
/// and nothing else — no hover or press fill is attached,
/// so the box cannot flash a highlight it cannot act on.
pub fn fill_color_inert(t: &Theme) -> Color {
    t.card
}

/// Hover fill, mirroring `btn` (chrome_view.rs:48).
pub fn hover_color(active: bool, t: &Theme) -> Color {
    if active {
        tok::accent(0, true)
    } else {
        t.control_track
    }
}

/// Pressed fill, mirroring `btn` (chrome_view.rs:49).
pub fn press_color(active: bool, t: &Theme) -> Color {
    if active {
        tok::accent(0, true)
    } else {
        t.control_off
    }
}

/// Resting fill of [`danger_icon_btn`]: the family's
/// danger red, `chrome_view::DANGER_FILL` — the deeper
/// red plan P12 verified at ≈5:1 against white text (the
/// drifted local copies' `0xF16F78` measured 2.88:1 and
/// failed). The constant is a `0xRRGGBB` value, converted
/// with the shell's shared [`hex_color`]; the fallback is
/// unreachable for a well-formed constant.
pub fn danger_fill_color() -> Color {
    hex_color(crate::chrome_view::DANGER_FILL).unwrap_or(Color::WHITE)
}

/// Hover fill of [`danger_icon_btn`]
/// (`chrome_view::DANGER_HOVER`).
pub fn danger_hover_color() -> Color {
    hex_color(crate::chrome_view::DANGER_HOVER).unwrap_or(Color::WHITE)
}

/// Pressed fill of [`danger_icon_btn`]
/// (`chrome_view::DANGER_PRESS`).
pub fn danger_press_color() -> Color {
    hex_color(crate::chrome_view::DANGER_PRESS).unwrap_or(Color::WHITE)
}

/// Glyph edge length inside an [`icon_only_btn`] of
/// `button_size`: the shared [`ICON_SIZE`], but never
/// larger than the button itself, so a small square can
/// never overflow its own box.
pub fn glyph_size_for(button_size: f32) -> f32 {
    ICON_SIZE.min(button_size)
}

/// A kit-`btn`-styled button with a Font Awesome glyph
/// before its label: a horizontal row of
/// `icons::icon_el(icon, 13.0, glyph_colour)` and the label
/// text, root keyed `key`, press handler on the root.
///
/// Unknown icon name → the plain kit `btn` (see the module
/// docs' fallback rule).
pub fn icon_btn(
    key: &str,
    icon: &str,
    label: &str,
    active: bool,
    t: &Theme,
    on_press: impl Fn(&mut State) + 'static,
) -> El {
    if !use_icon(icon) {
        // The fallback is chrome_view::btn, which wires
        // the same label as its help text — the tooltip
        // survives the degrade (see the module docs'
        // tooltip convention).
        return btn(key, label, active, t, on_press);
    }
    let glyph = glyph_color(active, t);
    row()
        .key(key)
        .help(label)
        .px(10.0)
        .py(5.0)
        .radius(6.0)
        .fill(fill_color(active, t))
        .hover(hover_color(active, t))
        .press(press_color(active, t))
        .press_scale(PRESS_SCALE)
        .on_press(on_press)
        .gap(ICON_LABEL_GAP)
        .center_y()
        .child(icons::icon_el(icon, ICON_SIZE, glyph))
        .child(text(label).font(type_::LABEL).color(glyph))
}

/// The inert twin of [`icon_btn`]: identical box metrics
/// (padding 10 × 5, radius 6, the shared gap and glyph
/// size, so its computed width and height match the live
/// button for the same icon and label), but resting on
/// [`fill_color_inert`], glyph and label in
/// [`glyph_color_muted`], and NO press, hover or press-fill
/// handler — a plain element that ignores the pointer.
///
/// For controls whose op would no-op from the current
/// state: render this instead of the live button, so the
/// gating reads at a glance and a stray click cannot fire
/// a refusal status or an undo snapshot.
///
/// Unknown icon name → the same degrade as [`icon_btn`]'s,
/// minus the handler [`icon_btn`] would have kept: the
/// plain label-only box (mirroring `btn`'s `el()` root and
/// metrics) in the muted colour.
pub fn icon_btn_inert(key: &str, icon: &str, label: &str, t: &Theme) -> El {
    let ink = glyph_color_muted(t);
    if !use_icon(icon) {
        return el()
            .key(key)
            .help(label)
            .px(10.0)
            .py(5.0)
            .radius(6.0)
            .fill(fill_color_inert(t))
            .child(text(label).font(type_::LABEL).color(ink));
    }
    row()
        .key(key)
        .help(label)
        .px(10.0)
        .py(5.0)
        .radius(6.0)
        .fill(fill_color_inert(t))
        .gap(ICON_LABEL_GAP)
        .center_y()
        .child(icons::icon_el(icon, ICON_SIZE, ink))
        .child(text(label).font(type_::LABEL).color(ink))
}

/// A square icon-only button, `size` × `size`, same fills
/// as [`icon_btn`], glyph centred. Unlike [`icon_btn`] it
/// takes NO padding: the caller fixes the box, and padding
/// would break the square contract — centring does the
/// placement instead.
///
/// TOOLTIP: `label` is wired as the element's help text
/// and rendered nowhere (wave 6-A2, audit S18-8) — it is
/// the call site's word for the action, and a
/// state-dependent site passes the label for the state
/// it is rendering. See the module docs' tooltip
/// convention.
///
/// Unknown icon name → the same square with no glyph child
/// (see the module docs' fallback rule).
pub fn icon_only_btn(
    key: &str,
    icon: &str,
    label: &str,
    size: f32,
    active: bool,
    t: &Theme,
    on_press: impl Fn(&mut State) + 'static,
) -> El {
    let mut root = row()
        .key(key)
        .help(label)
        .w(size)
        .h(size)
        .radius(6.0)
        .fill(fill_color(active, t))
        .hover(hover_color(active, t))
        .press(press_color(active, t))
        .press_scale(PRESS_SCALE)
        .on_press(on_press)
        .center();
    if use_icon(icon) {
        root = root.child(icons::icon_el(
            icon,
            glyph_size_for(size),
            glyph_color(active, t),
        ));
    }
    root
}

/// The inert twin of [`icon_only_btn`]: the same square
/// contract — `size` × `size`, no padding, glyph centred
/// at [`glyph_size_for`] — resting on
/// [`fill_color_inert`] with the glyph in
/// [`glyph_color_muted`], and NO press, hover or press-fill
/// handler.
///
/// TOOLTIP: `label` is wired as the element's help text
/// and rendered nowhere, exactly as the live twin (see
/// [`icon_only_btn`]) — an inert control is just as
/// undiscoverable without a name.
///
/// Unknown icon name → the same square with no glyph
/// child, exactly as [`icon_only_btn`] degrades.
pub fn icon_only_btn_inert(key: &str, icon: &str, label: &str, size: f32, t: &Theme) -> El {
    let mut root = row()
        .key(key)
        .help(label)
        .w(size)
        .h(size)
        .radius(6.0)
        .fill(fill_color_inert(t))
        .center();
    if use_icon(icon) {
        root = root.child(icons::icon_el(
            icon,
            glyph_size_for(size),
            glyph_color_muted(t),
        ));
    }
    root
}

/// Glyph edge length (pt) in [`chip_btn`] — one step
/// below [`ICON_SIZE`], matching the compact rows the
/// chip replaces (the menu items and the segment panel's
/// old local danger button both drew at 12).
pub const CHIP_GLYPH_SIZE: f32 = 12.0;

/// Horizontal gap (pt) between glyph and label in
/// [`chip_btn`] — tighter than [`ICON_LABEL_GAP`], in
/// proportion to the chip's tighter padding.
pub const CHIP_ICON_LABEL_GAP: f32 = 4.0;

/// The family's compact chip (Phase N, plan P05): the
/// shared form that absorbs figures_panel's `row_toggle`,
/// inspector_style's `cap_chip` / `preset_chip` and
/// easing_panel's `picker_cell` — four local clones of a
/// small toggle that had drifted apart (two of them had
/// lost their press fill entirely).
///
/// Metrics are the compact set the P05 state spec fixes:
/// padding px 6 / py 3, radius 5 — `row_toggle`'s, the
/// only compact metrics the shell already had. Fills,
/// hover, press and ink are the family state table
/// ([`fill_color`] / [`hover_color`] / [`press_color`] /
/// [`glyph_color`]), so a chip is a small `icon_btn`,
/// not a new look; the root carries the same
/// [`PRESS_SCALE`] press feedback as the other live
/// builders.
///
/// The icon is optional: `None` renders a text-only chip.
/// `Some(name)` with a name the catalogue doesn't know
/// degrades to text-only too — the module fallback rule
/// (same key, same label, same handler, just no glyph).
pub fn chip_btn(
    key: &str,
    icon: Option<&str>,
    label: &str,
    active: bool,
    t: &Theme,
    on_press: impl Fn(&mut State) + 'static,
) -> El {
    let ink = glyph_color(active, t);
    let mut root = row()
        .key(key)
        .help(label)
        .px(6.0)
        .py(3.0)
        .radius(5.0)
        .fill(fill_color(active, t))
        .hover(hover_color(active, t))
        .press(press_color(active, t))
        .press_scale(PRESS_SCALE)
        .on_press(on_press)
        .center_y();
    if let Some(name) = icon.filter(|&name| use_icon(name)) {
        root = root
            .gap(CHIP_ICON_LABEL_GAP)
            .child(icons::icon_el(name, CHIP_GLYPH_SIZE, ink));
    }
    root.child(text(label).font(type_::LABEL).color(ink))
}

/// The inert twin of [`chip_btn`] (UI mission wave 2,
/// audit S17 item I-7): identical compact metrics —
/// padding px 6 / py 3, radius 5, the shared
/// [`CHIP_ICON_LABEL_GAP`] and [`CHIP_GLYPH_SIZE`], so
/// its computed width and height match the live chip
/// for the same icon and label — but resting on
/// [`fill_color_inert`], glyph and label in
/// [`glyph_color_muted`], and NO press, hover,
/// press-fill or press-scale handler at all.
///
/// The chip was the one button family without an
/// inert form (S17 state matrix), so a gated compact
/// toggle had to fake its disabled state with a live
/// chip whose handler refuses — the exact pattern the
/// family's other inert twins exist to retire. Render
/// this instead when the chip's op would no-op from
/// the current state.
///
/// The icon follows [`chip_btn`]'s rule: `None` — or
/// a name the catalogue doesn't know — renders the
/// text-only chip (same key, same label, just no
/// glyph).
pub fn chip_btn_inert(key: &str, icon: Option<&str>, label: &str, t: &Theme) -> El {
    let ink = glyph_color_muted(t);
    let mut root = row()
        .key(key)
        .help(label)
        .px(6.0)
        .py(3.0)
        .radius(5.0)
        .fill(fill_color_inert(t))
        .center_y();
    if let Some(name) = icon.filter(|&name| use_icon(name)) {
        root = root
            .gap(CHIP_ICON_LABEL_GAP)
            .child(icons::icon_el(name, CHIP_GLYPH_SIZE, ink));
    }
    root.child(text(label).font(type_::LABEL).color(ink))
}

/// The danger button with a leading glyph (Phase N, plan
/// P05): [`icon_btn`]'s construction — same box (px 10 /
/// py 5, radius 6), same gap, the family glyph at the
/// full [`ICON_SIZE`] (the local copy this replaces drew
/// at 12) — but filled from the danger constants
/// ([`danger_fill_color`] / [`danger_hover_color`] /
/// [`danger_press_color`]) with glyph and label in white.
/// This is the shared form inspector_segment's local
/// hex-literal `danger_btn` copy migrates to; the
/// text-only form is `crate::chrome_view::danger_btn`.
///
/// Danger is a permanent state, not a toggle, so the
/// `active` parameter — kept for signature parity with
/// [`icon_btn`], whose call shape this mirrors — does
/// not alter the fills: a delete button is red whether
/// or not the panel considers it "current".
///
/// Unknown icon name → the text-only
/// `crate::chrome_view::danger_btn` (the same fallback
/// shape as [`icon_btn`] → `btn`: same key, same label,
/// same handler, just no glyph).
pub fn danger_icon_btn(
    key: &str,
    icon: &str,
    label: &str,
    active: bool,
    t: &Theme,
    on_press: impl Fn(&mut State) + 'static,
) -> El {
    if !use_icon(icon) {
        // The fallback is chrome_view::danger_btn, which
        // wires the same label as its help text — the
        // tooltip survives the degrade.
        return danger_btn(key, label, t, on_press);
    }
    // See the doc comment: `active` is signature parity
    // only; danger fills do not vary with it.
    let _ = active;
    row()
        .key(key)
        .help(label)
        .px(10.0)
        .py(5.0)
        .radius(6.0)
        .fill(danger_fill_color())
        .hover(danger_hover_color())
        .press(danger_press_color())
        .press_scale(PRESS_SCALE)
        .on_press(on_press)
        .gap(ICON_LABEL_GAP)
        .center_y()
        .child(icons::icon_el(icon, ICON_SIZE, Color::WHITE))
        .child(text(label).font(type_::LABEL).color(Color::WHITE))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_color_matches_the_kit_label_choice() {
        let t = rune::tok::theme(true);
        assert_eq!(glyph_color(true, &t), Color::WHITE);
        assert_eq!(glyph_color(false, &t), t.text);
        let light = rune::tok::theme(false);
        assert_eq!(glyph_color(true, &light), Color::WHITE);
        assert_eq!(glyph_color(false, &light), light.text);
    }

    #[test]
    fn state_fills_mirror_the_kit_button() {
        let t = rune::tok::theme(true);
        let accent = tok::accent(0, true);
        assert_eq!(fill_color(true, &t), accent);
        assert_eq!(hover_color(true, &t), accent);
        assert_eq!(press_color(true, &t), accent);
        assert_eq!(fill_color(false, &t), t.card);
        assert_eq!(hover_color(false, &t), t.control_track);
        assert_eq!(press_color(false, &t), t.control_off);
    }

    #[test]
    fn use_icon_follows_the_catalogue() {
        // "play" ships in the curated Font Awesome set
        // (icon_data.rs) the icon system is built on.
        assert!(use_icon("play"));
        assert!(!use_icon("definitely-not-an-icon"));
        assert!(!use_icon(""));
    }

    #[test]
    fn glyph_size_is_icon_size_capped_by_the_button() {
        assert_eq!(glyph_size_for(26.0), ICON_SIZE);
        assert_eq!(glyph_size_for(13.0), ICON_SIZE);
        assert_eq!(glyph_size_for(10.0), 10.0);
    }

    #[test]
    fn muted_foreground_is_the_theme_secondary_text() {
        let t = rune::tok::theme(true);
        assert_eq!(glyph_color_muted(&t), t.text_secondary);
        // A real dim, not the live button's resting ink.
        assert_ne!(glyph_color_muted(&t), glyph_color(false, &t));
        let light = rune::tok::theme(false);
        assert_eq!(glyph_color_muted(&light), light.text_secondary);
        assert_ne!(glyph_color_muted(&light), glyph_color(false, &light));
    }

    #[test]
    fn inert_fill_is_the_live_resting_fill() {
        let t = rune::tok::theme(true);
        assert_eq!(fill_color_inert(&t), t.card);
        assert_eq!(fill_color_inert(&t), fill_color(false, &t));
        let light = rune::tok::theme(false);
        assert_eq!(fill_color_inert(&light), fill_color(false, &light));
    }

    #[test]
    fn inert_builders_construct_on_both_fallback_branches() {
        // Same limits as the live builders' test above: an
        // element's key and laid-out size are not readable
        // from a bin-crate unit test, so these pin that
        // neither branch panics. Metric parity with the
        // live variants is structural rather than asserted
        // numerically: the inert builders are written from
        // the same padding (10 × 5), radius (6), gap
        // (ICON_LABEL_GAP), glyph size (ICON_SIZE /
        // glyph_size_for) and centreing calls as icon_btn /
        // icon_only_btn, so the same icon + label + size
        // inputs lay out to the same box.
        let t = rune::tok::theme(true);
        let _ = icon_btn_inert("k-inert", "play", "Play", &t);
        let _ = icon_btn_inert("k-inert-fallback", "nope-not-real", "Play", &t);
        let _ = icon_only_btn_inert("k-only-inert", "play", "Play", 26.0, &t);
        let _ = icon_only_btn_inert("k-only-inert-empty", "nope-not-real", "Play", 26.0, &t);
        // A square smaller than the glyph caps the same way
        // the live square does.
        let _ = icon_only_btn_inert("k-only-inert-small", "play", "Play", 10.0, &t);
    }

    #[test]
    fn builders_construct_on_both_fallback_branches() {
        // Element construction is pure (a press needs a
        // Runtime; the tree's keys/children are not readable
        // from a bin-crate unit test — the Phase G §12
        // finding), so these pin that neither branch panics:
        // known icon → glyph row, unknown icon → the plain
        // kit btn, and the icon-only square both ways.
        let t = rune::tok::theme(true);
        let _ = icon_btn("k-icon", "play", "Play", false, &t, |_| {});
        let _ = icon_btn("k-fallback", "nope-not-real", "Play", true, &t, |_| {});
        let _ = icon_only_btn("k-only", "play", "Play", 26.0, false, &t, |_| {});
        let _ = icon_only_btn("k-only-empty", "nope-not-real", "Play", 26.0, true, &t, |_| {});
    }

    #[test]
    fn press_scale_is_the_pinned_family_value() {
        // P15 item M1 fixes the value for the whole live
        // family. (That the inert twins never scale is
        // structural — they attach no press handler or
        // press_scale call — not something a value test
        // can reach.)
        assert_eq!(PRESS_SCALE, 0.97);
    }

    #[test]
    fn danger_fills_read_the_chrome_constants() {
        // The VALUES are chrome_view's (plan P12's ≈5:1
        // deep red lives in DANGER_FILL / DANGER_HOVER /
        // DANGER_PRESS); this file pins only that its
        // helpers read those constants through the shared
        // hex_color conversion, and that the danger rest
        // fill is neither family resting fill — a
        // copy-paste back to fill_color would silently
        // un-danger the button.
        let t = rune::tok::theme(true);
        assert_eq!(
            danger_fill_color(),
            hex_color(crate::chrome_view::DANGER_FILL).unwrap_or(Color::WHITE)
        );
        assert_eq!(
            danger_hover_color(),
            hex_color(crate::chrome_view::DANGER_HOVER).unwrap_or(Color::WHITE)
        );
        assert_eq!(
            danger_press_color(),
            hex_color(crate::chrome_view::DANGER_PRESS).unwrap_or(Color::WHITE)
        );
        assert_ne!(danger_fill_color(), fill_color(false, &t));
        assert_ne!(danger_fill_color(), fill_color(true, &t));
    }

    #[test]
    fn chip_btn_constructs_across_icon_branches() {
        // Construction-only pins, under the same limits
        // as the builders' tests above: None → text chip,
        // a known icon → glyph chip (inactive and
        // active), an unknown icon → the degraded text
        // chip. None of the branches panics.
        let t = rune::tok::theme(true);
        let _ = chip_btn("k-chip-text", None, "Round", false, &t, |_| {});
        let _ = chip_btn("k-chip-icon", Some("play"), "Play", false, &t, |_| {});
        let _ = chip_btn("k-chip-icon-active", Some("play"), "Play", true, &t, |_| {});
        let _ = chip_btn("k-chip-degraded", Some("nope-not-real"), "Play", false, &t, |_| {});
        // The compact metrics the P05 spec fixes: glyph
        // one step below the family size, a tighter gap.
        assert_eq!(CHIP_GLYPH_SIZE, 12.0);
        assert_eq!(CHIP_ICON_LABEL_GAP, 4.0);
    }

    #[test]
    fn chip_btn_inert_constructs_across_icon_branches() {
        // Same construction-only limits as chip_btn's
        // test: None → text chip, known icon → glyph
        // chip, unknown icon → the degraded text chip.
        // Metric parity with the live chip is structural
        // (same padding 6 × 3, radius 5, gap and glyph
        // size calls), as with the other inert twins.
        let t = rune::tok::theme(true);
        let _ = chip_btn_inert("k-chip-inert-text", None, "Round", &t);
        let _ = chip_btn_inert("k-chip-inert-icon", Some("play"), "Play", &t);
        let _ = chip_btn_inert("k-chip-inert-degraded", Some("nope-not-real"), "Play", &t);
        // The muted ink is the family inert ink, not the
        // live chip's resting ink at either active value.
        assert_eq!(glyph_color_muted(&t), t.text_secondary);
        assert_ne!(glyph_color_muted(&t), glyph_color(false, &t));
    }

    #[test]
    fn icon_size_scale_matches_the_kits_measured_sizes() {
        // The scale names the sizes the kit already
        // uses (S17 census): SM is the chip glyph step,
        // MD is the family glyph size; LG/XL are the
        // palette and empty-state sizes. Pinning the
        // equalities keeps a future ICON_SIZE or
        // CHIP_GLYPH_SIZE change from silently
        // desynchronising the scale.
        assert_eq!(ICON_SM, CHIP_GLYPH_SIZE);
        assert_eq!(ICON_MD, ICON_SIZE);
        assert_eq!(ICON_LG, 16.0);
        assert_eq!(ICON_XL, 20.0);
        assert!(ICON_SM < ICON_MD && ICON_MD < ICON_LG && ICON_LG < ICON_XL);
    }

    #[test]
    fn labelled_builders_construct_with_help_text() {
        // UI mission wave 6: every label-receiving builder
        // in this file wires its label as the element's
        // help text (the module docs' tooltip convention).
        // The help string itself is not readable back from
        // an element in a bin-crate unit test, so — as with
        // the construction tests above — these pin that
        // wiring the call changes nothing constructible:
        // both fallback branches, both themes' inputs, and
        // an empty label (Rune's help setter maps an empty
        // string to NO tooltip, so an unlabelled call site
        // degrades to silence rather than an empty bubble).
        let t = rune::tok::theme(true);
        let _ = icon_btn("k-h1", "play", "Play", false, &t, |_| {});
        let _ = icon_btn("k-h2", "nope-not-real", "Play", false, &t, |_| {});
        let _ = icon_btn_inert("k-h3", "play", "Play", &t);
        let _ = icon_btn_inert("k-h4", "nope-not-real", "Play", &t);
        let _ = chip_btn("k-h5", Some("play"), "Play", false, &t, |_| {});
        let _ = chip_btn("k-h6", None, "Round", false, &t, |_| {});
        let _ = chip_btn_inert("k-h7", None, "Round", &t);
        let _ = danger_icon_btn("k-h8", "trash-can", "Delete", false, &t, |_| {});
        let _ = icon_btn("k-h9", "play", "", false, &t, |_| {});
        let _ = chip_btn_inert("k-h10", None, "", &t);
        // Wave 6-A2: the icon-only pair takes a label too
        // (audit S18-8) — same construction-only pin, both
        // fallback branches, plus an empty label.
        let _ = icon_only_btn("k-h11", "play", "Play", 26.0, false, &t, |_| {});
        let _ = icon_only_btn("k-h12", "nope-not-real", "Play", 26.0, false, &t, |_| {});
        let _ = icon_only_btn_inert("k-h13", "play", "Play", 26.0, &t);
        let _ = icon_only_btn_inert("k-h14", "play", "", 26.0, &t);
    }

    #[test]
    fn danger_icon_btn_constructs_on_both_fallback_branches() {
        // Known icon → the glyph row, at either `active`
        // value (the fills ignore it by design); unknown
        // icon → chrome_view::danger_btn's text-only form.
        let t = rune::tok::theme(true);
        let _ = danger_icon_btn("k-danger", "trash-can", "Delete segment", false, &t, |_| {});
        let _ = danger_icon_btn("k-danger-active", "trash-can", "Delete segment", true, &t, |_| {});
        let _ = danger_icon_btn("k-danger-fallback", "nope-not-real", "Delete segment", false, &t, |_| {});
    }
}
