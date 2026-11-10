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
//!
//! ## UI mission wave 9 — the vertical splitter
//!
//! * [`v_splitter`] is the kit's drag handle for a
//!   resizable panel boundary: the horizontal
//!   [`divider`] turned upright, given a grip and a
//!   drag. It extends the wave 6 tooltip convention to
//!   the kit's one drag builder — its label wires as
//!   help text — and it is the kit's one ADAPTING
//!   builder: where [`toggle_row`] / [`swatch`] attach
//!   their handlers untouched (a press needs no
//!   translation), a drag arrives as the shell-level
//!   `Drag`, and the splitter's contract is precisely
//!   to hide that behind a pointer-x delta (see its
//!   own docs for the exact semantics).
//!
//! ## Roadmap Wave 1 — the numeric primitive (F-3)
//!
//! * [`NumSpec`] / [`NumberEdit`] / [`number_field`]
//!   are the kit's click-to-type numeric field (audit
//!   A7 items R1–R3, roadmap F-3): the StepForm
//!   discipline generalised — the field's form, its
//!   clamps and its steppers all compute from the one
//!   spec the caller's setter clamps with, so the
//!   three cannot disagree. The display form is also
//!   a drag-scrub surface (the kit's second adapting
//!   builder, after [`v_splitter`]); [`bounded_value_row`]
//!   composes slider + field + steppers for bounded
//!   values. [`text_field`] gains the invalid state
//!   ([`text_field_flagged`]) these fields need — the
//!   state its docs used to declare unreachable,
//!   because word fields never validate; number fields
//!   do, live, from the buffer.
//! * **Adoption recipe** (RM4's panels, and any later
//!   one): (1) store a [`NumberEdit`] per field in the
//!   panel's state struct; (2) write the [`NumSpec`]
//!   from the op's own range home (e.g.
//!   `NumSpec::new(EditorCore::LENGTH_MIN,
//!   EditorCore::LENGTH_MAX, 4.0)`); (3) render
//!   [`number_field`] with a [`NumWiring`] whose
//!   `commit` calls the exact setter, `reject` posts
//!   the explanation to `core.status`, `scrub_write`
//!   is a SILENT idempotent write (a `*_quiet` setter
//!   or the family's write half) and `scrub_finish`
//!   announces once; (4) call [`NumberEdit::reconcile`]
//!   from the panel's poll sync fn — that is where an
//!   Escape (the framework blurs the field) lands as a
//!   revert. New copy (help text, the reject status)
//!   is the panel's i18n, passed in — the kit invents
//!   no strings.

use rune::tok::{self, type_, Theme};
use rune::{
    canvas, col, el, row, text, text_input, Color, Drag, El, Phase, Shape, TextEdit, TextStyle, Tx,
};
use stykanimate_rs::graph::TangentPreset;
use stykanimate_rs::graph_panel::preset_preview;

use crate::chrome_view;
use crate::rune_sec::app_theme;
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
/// the Mono family applied, primary-text ink
/// (charter §11, as amended by Roadmap Wave 1 RM25 /
/// PL-1: the charter's `text_secondary` ink measured
/// 2.88:1 on this chip — S18's rendered figure —
/// under the 4.5 text bar for must-read hint text;
/// the primary tone measures 6.32 dark / 13.46
/// light on the chip over the card, fill unchanged).
/// Used by help rows, menu hints and the palette
/// footer; supersedes help_panel's local `key_cap`.
pub fn kbd(label: &str, t: &Theme) -> El {
    el()
        .px(5.0)
        .py(1.0)
        .radius(4.0)
        .fill(t.control_off)
        .child(
            text(label)
                .font(type_::MONO_CAPTION.mono())
                .color(t.text),
        )
}

/// [`kbd`] for accent fills (UI mission wave 2, audit
/// S17/S18): the command palette's selected row fills
/// with the accent at 0.85, where the plain chip —
/// `control_off` fill, secondary ink — measures
/// ~1.18:1 and is effectively unreadable. This variant
/// keeps the chip (same padding, radius and mono
/// caption) and inverts it into a keycap: a white fill
/// at 85% alpha with black ink at 85% alpha.
///
/// The wave-2 colours (white @ 0.75 ink over a white
/// @ 0.16 fill, mirroring the menus' active-row hint)
/// were never adopted — zero call sites — and measure
/// only 2.14 dark / 1.71 light on the palette row when
/// recomputed (Roadmap Wave 1, RM25 / PL-1): white ink
/// on an accent-family fill cannot clear the 4.5 text
/// bar while the accent itself stands (the standing
/// D-1 decision), so the chip inverts instead of
/// borrowing the row's ink. The keycap pair measures
/// 5.12 dark / 5.15 light on the row. The colours are
/// not theme-derived, so `t` is accepted only for
/// signature parity with [`kbd`] — call sites can swap
/// the two freely.
pub fn kbd_on_accent(label: &str, t: &Theme) -> El {
    let _ = t;
    let (fill, ink) = kbd_on_accent_pair();
    el()
        .px(5.0)
        .py(1.0)
        .radius(4.0)
        .fill(fill)
        .child(
            text(label)
                .font(type_::MONO_CAPTION.mono())
                .color(ink),
        )
}

/// The (fill, ink) pair [`kbd_on_accent`] paints —
/// factored out so the pair's contrast is pinnable
/// without a rendered tree (the `strip_colors`
/// precedent). See the builder's doc for the
/// measured figures.
fn kbd_on_accent_pair() -> (Color, Color) {
    (Color::WHITE.alpha(0.85), Color::BLACK.alpha(0.85))
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

/// [`empty_state`] with the action that fills it
/// (Roadmap Wave 1, RM28 — PL-7's actionable empty
/// states): the same glyph and message, then the chrome
/// `btn` whose press performs the filling action — the
/// panel's own real op (add the first figure, key the
/// first channel, clear the filter that hid
/// everything). The handler is a caller-supplied
/// `State` closure, exactly like every other kit
/// control. A state whose remedy is not an op the
/// panel owns — a stage click, a name to type first —
/// keeps the plain [`empty_state`]: this builder
/// exists so the remedy is one press, and it must
/// never carry a fake button.
pub fn empty_state_action(
    icon: &str,
    message: &str,
    key: &str,
    label: &str,
    t: &Theme,
    f: impl Fn(&mut State) + 'static,
) -> El {
    empty_state(icon, message, t).child(chrome_view::btn(key, label, false, t, f))
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
///
/// AT (Roadmap Wave 2, W2-51 / audit A20-11): this is
/// the kit's intrinsic toggle form, so the builder —
/// not each call site — carries the AT semantics: the
/// label as the accessible name (`El::label`, beside
/// the wave-6 help text) and `checked(on)`, so the
/// toggle's state is readable by assistive technology
/// and not carried by the pill's position alone.
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
        .label(label)
        .checked(on)
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

/// Which of the button family's three forms a stepper
/// button takes (Wave P, charter item 3 — the stepper's
/// disabled states, named once in the kit).
///
/// The shell's stepper panels each grew this exact
/// three-way split privately (onion_controls, text_tool,
/// sprites_panel, draw_tools — four copies of one
/// doctrine, the chrome_view P05 §4/§5.2 rule):
///
/// * [`StepForm::Live`] — the step would change the
///   value: the plain `chrome_view::btn`.
/// * [`StepForm::Noop`] — the step can only no-op (the
///   value sits at the clamp the step pushes against)
///   and the op has nothing to say about it: the
///   handler-free `btn_inert`, same key, same box, so a
///   press at the limit cannot fire a snapshot or
///   re-report an unchanged value as if it had stepped.
/// * [`StepForm::MutedLive`] — the control is gated
///   (its target does not exist yet) but the handler
///   HAS a refusal worth delivering ("Load a text
///   first.", "Weather is off — pick a kind first."):
///   `btn_muted_live` — the inert visual with the
///   handler kept, so the status line answers instead.
///
/// Callers compute the form from the same values they
/// render (the panels' pure `*_after` / `*_stepped`
/// helpers mirror their ops' clamps, so the button and
/// the op cannot disagree).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepForm {
    /// The step would change the value.
    Live,
    /// The step can only no-op and has nothing to say.
    Noop,
    /// Gated, but the handler's refusal is the feedback.
    MutedLive,
}

/// One stepper button in the form its state dictates
/// (see [`StepForm`]): the kit's single implementation
/// of the stepper disabled-state doctrine, over the
/// chrome button family. The key, label and handler are
/// the caller's, attached untouched in the forms that
/// keep a handler — exactly as with [`toggle_row`].
pub fn step_btn(
    key: &str,
    label: &str,
    form: StepForm,
    t: &Theme,
    f: impl Fn(&mut State) + 'static,
) -> El {
    match form {
        StepForm::Live => chrome_view::btn(key, label, false, t, f),
        StepForm::Noop => chrome_view::btn_inert(key, label, t),
        StepForm::MutedLive => chrome_view::btn_muted_live(key, label, false, t, f),
    }
}

/// A text field's border colour (Wave P, charter
/// item 3 — the input focus state): the accent while
/// the field holds keyboard focus, the card hairline
/// at rest. Pure, so the state logic is pinnable
/// without a Runtime.
pub fn field_border(focused: bool, t: &Theme) -> Color {
    if focused {
        accent()
    } else {
        t.card_border
    }
}

/// A text field's caret and selection colours (Wave
/// P): the accent for the caret, the accent at 30%
/// alpha for the selection wash. These exist because
/// Rune's `text_input` defaults are dark-theme-only —
/// a WHITE caret and a white-at-20% selection
/// (rune `el.rs` `DEFAULT_SELECTION`), which on the
/// light theme's near-white card are effectively
/// invisible: before this kit item, a focused field in
/// the light theme showed no caret and no selection at
/// all. The accent reads on both themes' cards (it is
/// the same token the button family's active state
/// fills with). Pure, pinnable.
pub fn field_caret() -> (Color, Color) {
    (accent(), accent().alpha(0.30))
}

/// The kit's one text field (Wave P, charter item 3):
/// Rune's `text_input` with the shell's input states
/// applied uniformly — the construction all fourteen
/// hand-rolled field sites previously repeated minus
/// the states none of them had:
///
/// * **Rest:** `control_off` fill (the kit's inset-
///   control fill, as on [`kbd`]), a 1 pt hairline
///   border in [`field_border`]'s resting colour,
///   [`CONTROL_RADIUS`], 8 pt of horizontal air so the
///   text never touches the border.
/// * **Focus:** the border turns accent — the field's
///   designed focus state, driven by the caller's
///   `TextEdit::focused` flag (the same flag the
///   shell's shortcut guard reads, so the visual and
///   the behaviour can never disagree) — and the caret
///   / selection take [`field_caret`]'s colours.
/// * **Hover:** NOT expressible without faking it —
///   Rune offers only overlay fills on hover, and an
///   overlay would wash over the field's own glyphs;
///   a hover border colour does not exist in the
///   pinned rev. Named here as an engine limit per
///   charter rule 3, not imitated.
/// * **Invalid:** a builder form since Roadmap Wave 1
///   (F-3) — [`text_field_flagged`] adds the error-ink
///   border, because number fields DO validate their
///   text (live, from the buffer). When this doc was
///   written no field in the shell validated (the Wave
///   P P5 findings), so the state was unreachable and,
///   per the rule below, undrawn; the numeric
///   primitive made it reachable and it is drawn
///   there, not here — word fields keep this builder
///   and its never-invalid form.
/// * **Disabled:** no builder form, by design — no
///   field is ever disabled (rename fields unmount
///   instead). A state with no reachable path is not
///   drawn.
///
/// Keys, placeholders, submit/edit handlers and
/// `.grow` stay at the call site, chained onto the
/// returned element exactly as they were chained onto
/// the bare `text_input` — the builder owns the box
/// and the states, the caller owns the field's
/// identity and behaviour.
pub fn text_field(edit: &TextEdit, font: TextStyle, focused: bool, t: &Theme) -> El {
    text_field_flagged(edit, font, focused, false, t)
}

/// A text field's border colour with the invalid state
/// included (Roadmap Wave 1, F-3): the theme's error
/// ink when the field's content is invalid — it beats
/// the focus accent on purpose, so the error stays
/// visible while the user is typing the fix — else
/// [`field_border`]'s focus/rest pair. Pure, pinnable,
/// like `field_border`.
pub fn field_border_state(focused: bool, invalid: bool, t: &Theme) -> Color {
    if invalid {
        app_theme::error_on(t)
    } else {
        field_border(focused, t)
    }
}

/// [`text_field`] with the invalid state drawn: the
/// same box, fills, caret and states, but the border
/// comes from [`field_border_state`], so an invalid
/// field carries the error-ink hairline in place of
/// the focus/rest pair. The number fields
/// ([`number_field`]) are the consumers; a caller
/// computes `invalid` from its own validation, live —
/// the builder draws the state, it does not judge the
/// text.
pub fn text_field_flagged(
    edit: &TextEdit,
    font: TextStyle,
    focused: bool,
    invalid: bool,
    t: &Theme,
) -> El {
    let (caret, selection) = field_caret();
    text_input(edit, font)
        .single_line()
        .color(t.text)
        .caret(caret, selection)
        .fill(t.control_off)
        .border(1.0, field_border_state(focused, invalid, t))
        .radius(CONTROL_RADIUS)
        .px(8.0)
        .py(3.0)
}

/// The kit's multi-line text area (Roadmap Wave 2,
/// W2-99): [`text_field`]'s box, states and caret
/// colours, but WITHOUT `.single_line()` — Rune's
/// `text_input` is multi-line by default (pinned rev
/// 87caf2a, `el.rs`: the constructor's `InputSpec`
/// carries `single_line: false`), so omitting the
/// builder call is the whole mechanism; nothing is
/// faked. The editing keys are the engine's own for a
/// plain (non-rich) field, verified in `edit/input.rs`:
/// plain Enter still SUBMITS (the shell's committing-
/// field convention is unchanged), Shift+Return and
/// Option+Return insert a line break, Up/Down move by
/// wrapped line, and pasted text keeps its line breaks
/// (the engine normalises a pasted CRLF pair to LF —
/// the sidecar domain's own storage form). The
/// vertical padding grows from the field's 3 pt to
/// 6 pt, rune-kit's own multi-line proportion, so a
/// multi-line box breathes like the engine's reference
/// construction.
///
/// Height is the CALLER's: wrap the returned field in
/// `rune::auto_grow(field, min_lines, max_lines)` (key
/// the field and attach its handlers first, per that
/// constructor's doc) so a long value scrolls inside a
/// bounded box instead of pushing the card open. No
/// invalid form: like [`text_field`], an area's text
/// is judged by its op at apply time, not live — a
/// caller that needs the error border composes
/// `field_border_state` itself.
pub fn text_area(edit: &TextEdit, font: TextStyle, focused: bool, t: &Theme) -> El {
    let (caret, selection) = field_caret();
    text_input(edit, font)
        .color(t.text)
        .caret(caret, selection)
        .fill(t.control_off)
        .border(1.0, field_border(focused, t))
        .radius(CONTROL_RADIUS)
        .px(8.0)
        .py(6.0)
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

/// Points of pointer travel below which a press on a
/// number field's display still counts as a click
/// (begin typing) rather than a scrub: Rune's own
/// click-run distance (`Drag`'s docs — a press that
/// drags past 4 pt ends the run), reused so the field
/// and the framework agree on what a click is.
pub const NUM_CLICK_TRAVEL: f32 = 4.0;

/// One numeric field's contract (Roadmap Wave 1, F-3
/// — audit A7 R1–R3): the range its op clamps with,
/// the step its buttons take, and how the stored
/// value is written for display. Everything a
/// [`number_field`] does — the stepper forms, the
/// clamp, the parse, the format, the slider fraction,
/// the scrub — computes from this one struct, which
/// the caller writes FROM the op's own range home
/// (e.g. `EditorCore::LENGTH_MIN..=LENGTH_MAX`), so
/// the field can never disagree with the setter it
/// commits through: the StepForm discipline
/// ([`StepForm`]) generalised from buttons to the
/// whole field.
///
/// All values are in the STORED domain (the domain
/// the setter receives) except formatting/parsing,
/// which pass through `display_scale`: the displayed
/// number is `value × display_scale`. `suffix` is the
/// unit mark shown after the number and accepted
/// after typed input. `points_per_step` is the scrub
/// rate: points of pointer travel per `step`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NumSpec {
    /// Lowest storable value (the op's clamp floor).
    pub min: f32,
    /// Highest storable value (the op's clamp ceiling).
    pub max: f32,
    /// One stepper press / one scrub step.
    pub step: f32,
    /// Decimals in the displayed number.
    pub decimals: usize,
    /// Unit mark after the number ("°", "%", "×", "").
    pub suffix: &'static str,
    /// Displayed number = stored value × this.
    pub display_scale: f32,
    /// Pointer points travelled per scrub step.
    pub points_per_step: f32,
}

/// Decimals a step size implies for display: whole
/// steps read whole, finer steps keep just the
/// precision the step can express.
fn decimals_for_step(step: f32) -> usize {
    if step >= 1.0 {
        0
    } else if step >= 0.1 {
        1
    } else if step >= 0.01 {
        2
    } else {
        3
    }
}

impl NumSpec {
    /// A plain spec: no suffix, stored values displayed
    /// as themselves, decimals derived from `step`,
    /// 2 pt of scrub travel per step.
    pub fn new(min: f32, max: f32, step: f32) -> Self {
        NumSpec {
            min,
            max,
            step,
            decimals: decimals_for_step(step),
            suffix: "",
            display_scale: 1.0,
            points_per_step: 2.0,
        }
    }

    /// A percent spec: the stored value is a fraction
    /// (0.05..=1.0 opacity domain), displayed ×100 with
    /// a "%" mark; decimals derive from the DISPLAYED
    /// step, so a 0.05 stored step reads "5%".
    pub fn percent(min: f32, max: f32, step: f32) -> Self {
        let mut spec = Self::new(min, max, step);
        spec.suffix = "%";
        spec.display_scale = 100.0;
        spec.decimals = decimals_for_step(step * 100.0);
        spec
    }

    /// A degrees spec: the stored value IS the degree
    /// count (the graph/camera angle domain), displayed
    /// with a "°" mark.
    pub fn degrees(min: f32, max: f32, step: f32) -> Self {
        let mut spec = Self::new(min, max, step);
        spec.suffix = "°";
        spec
    }

    /// Clamp a value into the range. A non-finite input
    /// reads as the floor rather than poisoning the
    /// stored project (the [`clamp_frac`] discipline);
    /// on every finite input this is exactly the clamp
    /// the range's op applies.
    pub fn clamp(&self, value: f32) -> f32 {
        if value.is_finite() {
            value.clamp(self.min, self.max)
        } else {
            self.min
        }
    }

    /// The value one step of `dir` (−1/+1) from `value`
    /// lands on, clamped.
    pub fn stepped(&self, value: f32, dir: i32) -> f32 {
        self.clamp(value + dir as f32 * self.step)
    }

    /// The form a stepper button for `dir` takes at
    /// `value` (see [`StepForm`]): [`StepForm::Noop`]
    /// when the step can only push against the clamp
    /// it would land on, [`StepForm::Live`] otherwise.
    /// Computed from the same numbers the op clamps
    /// with, so the button and the op cannot disagree.
    /// (A gated control's [`StepForm::MutedLive`] is the
    /// caller's to layer on, as with `step_btn` today.)
    pub fn step_form(&self, value: f32, dir: i32) -> StepForm {
        if self.stepped(value, dir) == self.clamp(value) {
            StepForm::Noop
        } else {
            StepForm::Live
        }
    }

    /// The displayed number for a stored value, WITHOUT
    /// the suffix — the text a type session seeds with
    /// (a field being edited shows the bare number, as
    /// in Blender). A negative zero reads as "0".
    pub fn edit_text(&self, value: f32) -> String {
        let shown = value * self.display_scale;
        let shown = if shown == 0.0 { 0.0 } else { shown };
        format!("{:.*}", self.decimals, shown)
    }

    /// The full readout text: [`NumSpec::edit_text`]
    /// plus the suffix.
    pub fn format(&self, value: f32) -> String {
        format!("{}{}", self.edit_text(value), self.suffix)
    }

    /// Parse typed input (see [`NumParse`]): a bare
    /// number, or a number with ONE trailing unit mark
    /// — the field's own suffix, or any of the shell's
    /// unit marks (a user types what a readout showed
    /// them, whichever panel's suffix they learned).
    /// The result is converted back to the stored
    /// domain and NOT clamped — clamping is the commit
    /// path's visible act, through [`NumSpec::clamp`].
    /// Spellings of NaN/infinity parse as `f32` in
    /// Rust; they are rejected here, so a field can
    /// never write a non-finite value into a project.
    pub fn parse(&self, text: &str) -> NumParse {
        let mut body = text.trim();
        if body.is_empty() {
            return NumParse::Empty;
        }
        for suffix in [self.suffix, "°", "%", "×", "x"] {
            if !suffix.is_empty() && body.ends_with(suffix) {
                body = body[..body.len() - suffix.len()].trim_end();
                break;
            }
        }
        if body.is_empty() {
            return NumParse::Empty;
        }
        match body.parse::<f32>() {
            Ok(v) if v.is_finite() => NumParse::Value(v / self.display_scale),
            _ => NumParse::Invalid,
        }
    }

    /// The value's fraction along the range — the
    /// fraction [`slider_track`] draws, and the inverse
    /// of [`NumSpec::value_at_frac`] (the mapping the
    /// layers/scene sliders each keep a private copy of
    /// today, generalised to the spec's range).
    pub fn frac_of(&self, value: f32) -> f32 {
        let span = self.max - self.min;
        if span > 0.0 {
            clamp_frac((self.clamp(value) - self.min) / span)
        } else {
            0.0
        }
    }

    /// The value at a slider fraction — see
    /// [`NumSpec::frac_of`].
    pub fn value_at_frac(&self, frac: f32) -> f32 {
        self.min + clamp_frac(frac) * (self.max - self.min)
    }

    /// The value a scrub lands on: the press (anchor)
    /// value plus the pointer's travel at the spec's
    /// rate, clamped. Non-finite travel (a NaN out of
    /// some drag maths) holds the anchor, per the
    /// `clamp_frac` discipline.
    ///
    /// KNOWN LIMIT, recorded not faked: audit A7 R2
    /// asks for Shift = fine / Ctrl = coarse scrub
    /// rates, but `Drag` in the pinned Rune rev carries
    /// no modifier state, so a mid-drag rate change is
    /// not expressible; the one rate is the spec's.
    pub fn scrubbed(&self, anchor: f32, dx_points: f32) -> f32 {
        if !dx_points.is_finite() || self.points_per_step <= 0.0 {
            return self.clamp(anchor);
        }
        self.clamp(anchor + (dx_points / self.points_per_step) * self.step)
    }
}

/// What parsing a number field's text yields (see
/// [`NumSpec::parse`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NumParse {
    /// A finite number, in the stored domain, not yet
    /// clamped.
    Value(f32),
    /// Nothing (or only a unit mark) was typed.
    Empty,
    /// Text that is not a finite number.
    Invalid,
}

/// A number field's live validity, derived from its
/// buffer on every render (see [`NumberEdit::validity`])
/// — the classification the invalid border draws from.
/// Derived, never stored, so it cannot drift from the
/// text it describes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumValidity {
    /// The buffer parses to a finite number.
    Valid,
    /// The buffer is empty (or only a unit mark).
    Empty,
    /// The buffer does not parse.
    Invalid,
}

/// A number field's session state (Roadmap Wave 1,
/// F-3): the typing buffer plus the session bookkeeping,
/// stored by the adopting panel next to its other field
/// states (the `TextEdit` fields' idiom). The kit owns
/// the state machine; the panel owns the instance.
///
/// The session rules (audit A7 R1, Blender's):
/// * [`NumberEdit::begin`] opens a session seeded with
///   the current value's [`NumSpec::edit_text`], the
///   fresh buffer claiming keyboard focus (the shell's
///   palette/begin-rename pattern).
/// * Enter commits — the builder's submit path parses,
///   clamps and hands the value to the caller's setter,
///   then [`NumberEdit::close`]s. An unparseable Enter
///   reverts and lets the caller's reject hook explain
///   in the status line.
/// * Escape reverts: the framework blurs the field on
///   Escape (main.rs's `on_key` docs), and the panel's
///   poll sync calls [`NumberEdit::reconcile`], which
///   reverts a session the framework has blurred. A
///   click-away blur lands in the same place — in this
///   framework the two are indistinguishable, and a
///   reverted value is one click from being retyped,
///   while a silently committed half-typed value is a
///   wrong number in the project.
#[derive(Clone, Default)]
pub struct NumberEdit {
    /// The typing buffer (the shell's `TextEdit`).
    pub edit: TextEdit,
    /// A type session is open on this field.
    pub armed: bool,
    /// The text the open session was seeded with — what
    /// a revert restores.
    pub seed: String,
}

impl NumberEdit {
    /// A closed, empty session state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Open a session seeded with `seed` (the current
    /// value's [`NumSpec::edit_text`]): the fresh buffer
    /// claims keyboard focus, as the shell's fields do.
    pub fn begin(&mut self, seed: String) {
        let mut edit = TextEdit::new(seed.clone());
        edit.focused = true;
        self.edit = edit;
        self.seed = seed;
        self.armed = true;
    }

    /// Whether a type session is open.
    pub fn is_open(&self) -> bool {
        self.armed
    }

    /// The buffer's current text.
    pub fn text(&self) -> &str {
        &self.edit.text
    }

    /// The buffer's live validity under `spec` — what
    /// the field's border draws from. An unopened
    /// session's buffer is empty, hence
    /// [`NumValidity::Empty`]; consumers only read this
    /// while a session is open.
    pub fn validity(&self, spec: &NumSpec) -> NumValidity {
        match spec.parse(&self.edit.text) {
            NumParse::Value(_) => NumValidity::Valid,
            NumParse::Empty => NumValidity::Empty,
            NumParse::Invalid => NumValidity::Invalid,
        }
    }

    /// Close the session after a commit: the value has
    /// been written by the caller's setter; the buffer
    /// is left as typed (the display re-derives from
    /// the stored value on the next render).
    pub fn close(&mut self) {
        self.armed = false;
    }

    /// Abandon the session: the buffer returns to the
    /// seed text and the field stands down — nothing
    /// was written, and the display (which never stopped
    /// deriving from the stored value) is unchanged.
    pub fn revert(&mut self) {
        let seed = std::mem::take(&mut self.seed);
        self.edit.replace_text(seed);
        self.edit.focused = false;
        self.armed = false;
    }

    /// The poll-sync reconciliation (the shell's
    /// `sync_name_edit` idiom): a session the framework
    /// has blurred — Escape, or focus moving elsewhere
    /// — is reverted, and `true` is returned so the
    /// panel can note it if it cares. A session still
    /// holding focus is left exactly as it is.
    pub fn reconcile(&mut self) -> bool {
        if self.armed && !self.edit.focused {
            self.revert();
            true
        } else {
            false
        }
    }
}

/// The five closures a [`number_field`] needs, bundled
/// (Roadmap Wave 1, F-3) — the caller's half of the
/// field, in one named construction so an adopting
/// panel writes it once per field:
///
/// * `access` — projects the caller's [`State`] to the
///   panel's stored [`NumberEdit`] (the `.on_edit`
///   accessor idiom; must be a non-capturing projection
///   — it is cloned for the field's two bindings).
/// * `commit` — Enter with a parseable value: receives
///   the value CLAMPED through the spec; calls the
///   field's exact setter. The session is already
///   closed when this runs.
/// * `reject` — Enter with unparseable/empty text: the
///   session has already reverted; post the explanation
///   to `core.status` here (the copy is the panel's
///   i18n — see IA-RM3-1).
/// * `scrub_write` — a mid-scrub write: SILENT (no
///   status recap, no toast feed) and idempotent (a
///   cancelled scrub calls it once more with the press
///   value). A `*_quiet` setter or the op family's
///   write half; for a snapshotting family, the panel
///   takes its one gesture snapshot in its own drag
///   bookkeeping, as the pose drag does — never one
///   snapshot per Move.
/// * `scrub_finish` — the scrub's release: the final
///   value, for the caller's one announcing write /
///   status recap.
pub struct NumWiring<A, C, R, W, F> {
    /// `State` → the stored [`NumberEdit`].
    pub access: A,
    /// Enter-commit: the clamped value.
    pub commit: C,
    /// Enter on unparseable text (already reverted).
    pub reject: R,
    /// Silent mid-scrub write.
    pub scrub_write: W,
    /// Scrub release: the final value.
    pub scrub_finish: F,
}

/// Translate a number field's display drag into the
/// wiring's calls (the [`number_field`] display form's
/// adapter — the kit's second adapting builder, on the
/// [`v_splitter`] precedent): a press anchors the
/// rendered value in `Drag::data` (the layers slider's
/// slot); travel at or past [`NUM_CLICK_TRAVEL`] is a
/// scrub — silent writes from the anchor, `tx.instant()`
/// while scrubbing (the layers idiom); a release with
/// less travel is a CLICK and opens the type session,
/// seeded from the press value; a release after travel
/// finishes the scrub; a cancelled drag restores the
/// press value silently. The anchor is written only on
/// the press, from that frame's rendered value, so the
/// shell's per-frame tree rebuilds cannot drift it —
/// moves read `Drag::data` and `Drag::dx` alone.
fn number_drag<A, C, R, W, F>(
    s: &mut State,
    d: &mut Drag,
    tx: &mut Tx,
    spec: &NumSpec,
    press_value: f32,
    wiring: &NumWiring<A, C, R, W, F>,
) where
    A: Fn(&mut State) -> &mut NumberEdit,
    W: Fn(&mut State, f32),
    F: Fn(&mut State, f32),
{
    match d.phase {
        Phase::Down => {
            d.data = press_value;
        }
        Phase::Move => {
            if d.dx.abs() >= NUM_CLICK_TRAVEL {
                (wiring.scrub_write)(s, spec.scrubbed(d.data, d.dx));
                tx.instant();
            }
        }
        Phase::Up => {
            if d.cancelled() {
                (wiring.scrub_write)(s, d.data);
            } else if d.dx.abs() < NUM_CLICK_TRAVEL {
                let seed = spec.edit_text(d.data);
                (wiring.access)(s).begin(seed);
            } else {
                (wiring.scrub_finish)(s, spec.scrubbed(d.data, d.dx));
            }
        }
    }
}

/// A click-to-type number field (Roadmap Wave 1, F-3):
/// one builder, two forms, swapped on the session
/// state the caller stores ([`NumberEdit`]).
///
/// **Display form** (no session open): [`stepper_value`]'s
/// box — mono value, centred, at least 34 pt wide —
/// with the hover/press overlays and a drag surface:
/// a click opens the type session, a horizontal drag
/// scrubs (see [`number_drag`] for the exact contract).
/// `label` wires as the element's help text (the wave
/// 6 convention) and — Roadmap Wave 2, W2-51 — as its
/// accessibility name in BOTH forms (`El::label`): the
/// key names the value across the form swap, and the
/// name now survives it too, instead of the display
/// form naming itself by its readout digits. Rune
/// exposes no cursor control, so
/// the text-entry / scrub affordance is carried by the
/// overlays and that help text — the `v_splitter`
/// situation, named rather than faked.
///
/// **Editing form** (session open): the kit text field
/// ([`text_field_flagged`]) in the readout's mono
/// face, bound to the stored buffer through the
/// wiring's `access`; the border follows the buffer's
/// live validity — error ink while the text does not
/// parse, with `invalid_hint` (the caller's copy)
/// under the field while it is invalid. Enter commits
/// or reverts per the session rules ([`NumberEdit`]);
/// Escape reverts through the panel's poll sync
/// ([`NumberEdit::reconcile`]).
///
/// `key` is the caller's, on the root in BOTH forms —
/// the key names the VALUE, not the widget, so the
/// element census sees one element across the swap.
/// `value` is the stored value this render (the scrub's
/// press anchor); `display` is its readout text
/// (normally `spec.format(value)` — the caller's to
/// format, as `stepper_value`'s text always was).
#[allow(clippy::too_many_arguments)]
pub fn number_field<A, C, R, W, F>(
    key: &str,
    state: &NumberEdit,
    spec: &NumSpec,
    value: f32,
    display: &str,
    label: &str,
    invalid_hint: Option<&str>,
    t: &Theme,
    wiring: NumWiring<A, C, R, W, F>,
) -> El
where
    A: Fn(&mut State) -> &mut NumberEdit + Clone + 'static,
    C: Fn(&mut State, f32) + 'static,
    R: Fn(&mut State) + 'static,
    W: Fn(&mut State, f32) + 'static,
    F: Fn(&mut State, f32) + 'static,
{
    let spec = *spec;
    if state.armed {
        let invalid = state.validity(&spec) == NumValidity::Invalid;
        let access = wiring.access.clone();
        let field = text_field_flagged(
            &state.edit,
            type_::MONO_CAPTION.mono(),
            state.edit.focused,
            invalid,
            t,
        )
        .key(key)
        .label(label)
        .min_w(34.0)
        .on_edit(move |s: &mut State| &mut (access)(s).edit)
        .on_submit(move |s: &mut State, _tx: &mut Tx| {
            let text = (wiring.access)(s).text().to_string();
            match spec.parse(&text) {
                NumParse::Value(v) => {
                    let v = spec.clamp(v);
                    (wiring.access)(s).close();
                    (wiring.commit)(s, v);
                }
                NumParse::Empty | NumParse::Invalid => {
                    (wiring.access)(s).revert();
                    (wiring.reject)(s);
                }
            }
        });
        if invalid {
            if let Some(hint) = invalid_hint {
                return col()
                    .gap(2.0)
                    .child(field)
                    .child(text(hint).font(type_::LABEL).color(app_theme::error_on(t)));
            }
        }
        field
    } else {
        el()
            .key(key)
            .help(label)
            .label(label)
            .min_w(34.0)
            .center()
            .hover(t.hover)
            .press(t.press)
            .on_drag(move |s: &mut State, d: &mut Drag, tx: &mut Tx| {
                number_drag(s, d, tx, &spec, value, &wiring);
            })
            .child(
                text(display)
                    .font(type_::MONO_CAPTION.mono())
                    .color(t.text),
            )
    }
}

/// The bounded-value composite (Roadmap Wave 1, F-3 —
/// audit A7 R3): one row composing the kit slider, a
/// [`number_field`] and the two steppers, for a bounded
/// continuous value — replacing the per-panel choice
/// between a bare slider and bare steppers (layers'
/// opacity and scene's Amount are hand-built copies of
/// this shape). Layout follows the panels' `stepper_row`
/// order, with the slider before the steppers: growing
/// label, slider, −, field, +.
///
/// * The slider root carries the caller's `slider_key`
///   and `slider_drag` UNTOUCHED — the absolute-position
///   drag semantics stay the caller's (scene/layers keep
///   theirs); the track visual is [`slider_track`] at
///   the spec's own fraction ([`NumSpec::frac_of`]), and
///   a caller maps a track position back with
///   [`NumSpec::value_at_frac`].
/// * AT (Roadmap Wave 2, W2-51 / audit A20-11): the
///   slider root declares the `slider` role, the row's
///   label as its accessible name, and the value as a
///   range (`El::range_value` + `range_step`) in the
///   DISPLAY domain (stored × `display_scale`) — the
///   numbers a sighted user reads off the field are the
///   numbers assistive technology reports. Before this,
///   the root was an unnamed drag surface that the AT
///   tree could only present as a button with no value.
/// * The steppers are [`step_btn`]s whose forms come
///   from the spec ([`NumSpec::step_form`]); `step`
///   receives the direction (−1/+1) — normally the op's
///   own step call, kept so a stepper press stays the
///   op's, not a synthesized setter call.
/// * The field is [`number_field`] on the same spec and
///   wiring — slider, steppers and field are three
///   doors onto the one value, and the spec is the one
///   contract they share.
#[allow(clippy::too_many_arguments)]
pub fn bounded_value_row<A, C, R, W, F, D>(
    slider_key: &str,
    field_key: &str,
    dn_key: &str,
    up_key: &str,
    label: &str,
    state: &NumberEdit,
    spec: &NumSpec,
    value: f32,
    display: &str,
    invalid_hint: Option<&str>,
    t: &Theme,
    wiring: NumWiring<A, C, R, W, F>,
    step: impl Fn(&mut State, i32) + Clone + 'static,
    slider_drag: D,
) -> El
where
    A: Fn(&mut State) -> &mut NumberEdit + Clone + 'static,
    C: Fn(&mut State, f32) + 'static,
    R: Fn(&mut State) + 'static,
    W: Fn(&mut State, f32) + 'static,
    F: Fn(&mut State, f32) + 'static,
    D: Fn(&mut State, &mut Drag, &mut Tx) + 'static,
{
    let step_up = step.clone();
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
            el()
                .key(slider_key)
                .role("slider")
                .label(label)
                .range_value(
                    (value * spec.display_scale) as f64,
                    (spec.min * spec.display_scale) as f64,
                    (spec.max * spec.display_scale) as f64,
                )
                .range_step((spec.step * spec.display_scale) as f64)
                .size(SLIDER_W, SLIDER_H)
                .on_drag(slider_drag)
                .child(slider_track(spec.frac_of(value), t)),
        )
        .child(step_btn(dn_key, "−", spec.step_form(value, -1), t, move |s: &mut State| {
            step(s, -1)
        }))
        .child(number_field(
            field_key, state, spec, value, display, label, invalid_hint, t, wiring,
        ))
        .child(step_btn(up_key, "+", spec.step_form(value, 1), t, move |s: &mut State| {
            step_up(s, 1)
        }))
}

/// Splitter hit-strip width: the 1 pt hairline centred
/// in a grabbable strip (see [`v_splitter`]). Callers
/// mount the splitter in their row layout, so this is
/// the width a wiring layout must budget for it.
pub const SPLITTER_W: f32 = 10.0;
/// Width of a splitter's hairline — the same 1 pt as
/// the horizontal [`divider`].
pub const SPLITTER_LINE_W: f32 = 1.0;
/// A splitter's grip glyph size: small enough to sit
/// inside [`SPLITTER_W`] with air either side. Named
/// exception to the icon size scale (Wave 9, E3's size
/// census): 8 pt is a kit-internal metric for a grip
/// mark, not a content icon — snapping it to ICON_SM
/// (12) would break the splitter geometry the tests
/// pin (`SPLITTER_GRIP < SPLITTER_W`).
const SPLITTER_GRIP: f32 = 8.0;
/// Vertical air between a splitter's hairline halves
/// and its grip glyph (tok S1).
const SPLITTER_GAP: f32 = 4.0;

/// One growing half of a splitter's hairline: a 1 pt
/// `separator` segment that stretches to fill whatever
/// height the grip leaves it (see [`v_splitter`]).
fn splitter_line(t: &Theme) -> El {
    el().w(SPLITTER_LINE_W).grow(1.0).fill(t.separator)
}

/// Translate a splitter drag into pointer-x deltas for
/// the caller's callback (see [`v_splitter`] for the
/// contract). The press position anchors in
/// `Drag::data` — the shell sliders' established slot
/// for press state — and each delivered delta re-
/// anchors, so the increments always sum to the
/// pointer's total travel. A non-finite delta (a NaN
/// out of some drag maths) is dropped rather than
/// delivered, the [`clamp_frac`] discipline.
fn splitter_drag(s: &mut State, d: &mut Drag, tx: &mut Tx, f: &impl Fn(&mut State, f32)) {
    match d.phase {
        Phase::Down => {
            d.data = d.x;
        }
        Phase::Move => {
            let dx = d.x - d.data;
            if dx.is_finite() && dx != 0.0 {
                f(s, dx);
                d.data = d.x;
            }
            tx.instant();
        }
        Phase::Up => {
            if !d.cancelled() {
                let dx = d.x - d.data;
                if dx.is_finite() && dx != 0.0 {
                    f(s, dx);
                }
            }
        }
    }
}

/// A vertical splitter: the drag handle for a
/// resizable panel boundary (UI mission wave 9). The
/// horizontal [`divider`] turned upright — a 1 pt
/// `separator` hairline spanning the full height of
/// the row it is mounted in, broken at the middle by
/// a `grip-vertical` glyph in the [`text_muted`] role
/// — carried on a [`SPLITTER_W`]-wide hit strip whose
/// hover/press feedback is the theme's `t.hover` /
/// `t.press` overlays, the state treatment
/// [`list_row`] and [`swatch`] use. If the grip glyph
/// ever leaves the curated icon set the builder
/// degrades to one unbroken hairline rather than a
/// blank slot (the [`panel_title`] omission idiom).
///
/// Mount it between two panels in a `row`: the root
/// is `w(SPLITTER_W)` / `h_full()`, so the caller's
/// layout budgets exactly [`SPLITTER_W`] of width for
/// it. The builder holds no state and mounts nothing
/// until a caller places it — adding it to the kit
/// changes no existing tree.
///
/// The callback contract — the kit's one adapting
/// builder, because a press handler needs no
/// translation but a drag arrives as the shell-level
/// `Drag` (position inside the element), which is
/// exactly what a width adjustment does not want:
///
/// * `f(state, dx)` receives the pointer's horizontal
///   movement in points SINCE THE PREVIOUS delivered
///   event — incremental deltas that sum to the total
///   travel, delivered verbatim and unclamped
///   (clamping the resulting width is the caller's
///   business, as what a press does is elsewhere).
/// * A press anchors silently; moves deliver, and a
///   non-cancelled release delivers the final
///   increment. A zero or non-finite movement delivers
///   nothing.
/// * **Known limit, recorded not faked:** a cancelled
///   drag does NOT rewind increments already
///   delivered, the way the shell's absolute-value
///   sliders restore their press value — an
///   incremental callback has no press value to
///   restore to. Callers needing a transactional drag
///   must accumulate the deltas in their own state and
///   commit them on a signal only they own.
///
/// Tooltip: `label` wires as the element's help text
/// (the kit's wave 6 convention). Rune exposes no
/// cursor-setting surface for an element, so the
/// east–west resize affordance is carried by the
/// grip glyph, the state overlays and this text —
/// pass a label that names the gesture, e.g.
/// "Drag to resize the sidebar".
///
/// AT (Roadmap Wave 2, W2-51 / audit A20-11): the
/// label also wires as the accessible name, and the
/// root declares the `separator` role — with a drag
/// handler the pinned Rune rev maps that to the AT
/// splitter role, so the boundary reads as a resizer
/// rather than as the unnamed button the role-less
/// drag surface used to produce in the tree census.
pub fn v_splitter(
    key: &str,
    label: &str,
    t: &Theme,
    f: impl Fn(&mut State, f32) + 'static,
) -> El {
    let mut bar = col()
        .key(key)
        .help(label)
        .label(label)
        .role("separator")
        .w(SPLITTER_W)
        .h_full()
        .items_center()
        .gap(SPLITTER_GAP)
        .hover(t.hover)
        .press(t.press)
        .on_drag(move |s: &mut State, d: &mut Drag, tx: &mut Tx| {
            splitter_drag(s, d, tx, &f);
        });
    if icons::has_icon("grip-vertical") {
        bar = bar
            .child(splitter_line(t))
            .child(icons::icon_el("grip-vertical", SPLITTER_GRIP, splitter_grip_color(t)))
            .child(splitter_line(t));
    } else {
        bar = bar.child(splitter_line(t));
    }
    bar
}

/// The splitter grip's glyph colour (Roadmap Wave 1,
/// RM25 / PL-1 marginal sweep): the muted role in
/// dark, where it measures 5.15 on the background and
/// clears the 3:1 non-text guidance; in light the
/// same role measures 1.94 there, so the grip takes
/// the full secondary tone instead (5.15 on the light
/// background). The grip is a glyph, never text, and
/// the hairlines beside it carry the divider in both
/// appearances.
fn splitter_grip_color(t: &Theme) -> Color {
    if app_theme::is_dark(t) {
        text_muted(t)
    } else {
        t.text_secondary
    }
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
///
/// AT NAME (Roadmap Wave 2, W2-51 / audit A20-11):
/// the S18-10 adoption now has its kit path —
/// [`swatch_named`], which takes the call site's own
/// colour name and wires it as `El::label` (the
/// accessible name) ONLY: `label` is not `help`, so
/// the no-tooltip rule above stands untouched, and a
/// hex string passed by a hex-only palette is a
/// legitimate AT name even though it would be junk as
/// a tooltip. This builder stays as it is; palettes
/// adopt the named form call site by call site.
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

/// [`swatch`] with an accessible name (Roadmap Wave 2,
/// W2-51 / audit A20-11): the same 24×24 pressable,
/// with the call site's colour name wired as
/// `El::label`. The name is the CALL SITE's word for
/// the colour — a palette entry's name where the
/// palette has one, its `#RRGGBB` hex where the palette
/// is hex-only (the shell's palettes are) — never
/// derived inside the kit from the `Color`, whose
/// float channels do not round-trip to the palette's
/// authored hex. No help text is set: a swatch still
/// shows no tooltip (see [`swatch`]'s doc).
pub fn swatch_named(
    key: &str,
    color: Color,
    name: &str,
    selected: bool,
    t: &Theme,
    f: impl Fn(&mut State) + 'static,
) -> El {
    swatch(key, color, selected, t, f).label(name)
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
        .label(label)
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

    /// WCAG ratio between two opaque colours (Rune
    /// semantics — luminance over linear values).
    fn ratio(fg: Color, bg: Color) -> f32 {
        let (a, b) = (fg.luminance(), bg.luminance());
        let (hi, lo) = if a >= b { (a, b) } else { (b, a) };
        (hi + 0.05) / (lo + 0.05)
    }

    /// `fg` (carrying its alpha) composited over an
    /// opaque `bg`.
    fn over(fg: Color, bg: Color) -> Color {
        bg.mix(Color { a: 1.0, ..fg }, fg.a)
    }

    #[test]
    fn kbd_ink_clears_the_text_bar_on_the_chip() {
        // RM25 (PL-1): the primary ink on the
        // unchanged `control_off` chip clears 4.5 on
        // the surfaces the chip sits on (card and
        // elevated, both appearances); the retired
        // secondary ink reproduces S18's failing
        // figure in dark.
        for dark in [true, false] {
            let t = app_theme::app_theme(dark);
            for surface in [t.card, app_theme::elevated(&t)] {
                let chip = over(t.control_off, surface);
                assert!(ratio(t.text, chip) >= 4.5, "dark={dark}");
            }
        }
        let t = app_theme::app_theme(true);
        let chip = over(t.control_off, t.card);
        assert!(ratio(t.text_secondary, chip) < 4.5);
        let _ = kbd("Ctrl+S", &t);
    }

    #[test]
    fn kbd_on_accent_pair_clears_the_text_bar_on_the_palette_row() {
        // RM25 (PL-1): the keycap pair against the
        // command palette's selected row — the accent
        // at 0.85 over the panel card — in both
        // appearances.
        let (fill, ink) = kbd_on_accent_pair();
        for dark in [true, false] {
            let t = app_theme::app_theme(dark);
            let row = over(accent().alpha(0.85), t.card);
            let chip = over(fill, row);
            let painted = over(ink, chip);
            assert!(
                ratio(painted, chip) >= 4.5,
                "dark={dark}: {}",
                ratio(painted, chip)
            );
        }
        let t = app_theme::app_theme(true);
        let _ = kbd_on_accent("Enter", &t);
    }

    #[test]
    fn splitter_grip_clears_the_non_text_bar_in_both_themes() {
        // RM25 (PL-1): the grip glyph against the app
        // background clears the 3:1 non-text guidance
        // in both appearances — dark via the muted
        // role, light via the full secondary tone.
        for dark in [true, false] {
            let t = app_theme::app_theme(dark);
            let grip = over(splitter_grip_color(&t), t.bg);
            assert!(ratio(grip, t.bg) >= 3.0, "dark={dark}");
        }
        let d = app_theme::app_theme(true);
        assert_eq!(splitter_grip_color(&d), text_muted(&d));
        let l = app_theme::app_theme(false);
        assert_eq!(splitter_grip_color(&l), l.text_secondary);
    }

    #[test]
    fn field_border_is_accent_only_when_focused() {
        for dark in [true, false] {
            let t = tok::theme(dark);
            assert_eq!(field_border(true, &t), accent());
            assert_eq!(field_border(false, &t), t.card_border);
            // A real state change, not a restatement:
            // the two branches must differ on both
            // themes or the focus state would be
            // invisible by construction.
            assert_ne!(field_border(true, &t), field_border(false, &t));
        }
    }

    #[test]
    fn field_caret_is_the_accent_never_the_white_default() {
        let (caret, selection) = field_caret();
        assert_eq!(caret, accent());
        // The defect this fixes: Rune's defaults are
        // a white caret and a white 20% selection —
        // invisible on the light theme's card.
        assert_ne!(caret, Color::WHITE);
        assert_eq!(
            (selection.r, selection.g, selection.b),
            (caret.r, caret.g, caret.b)
        );
        assert_eq!(selection.a, 0.30);
    }

    /// A [`NumWiring`] for construction smoke tests:
    /// builders only STORE the closures while building
    /// (handlers run on events, never during a build),
    /// so the access projection is never called here —
    /// it panics if that ever stops being true.
    fn test_wiring() -> NumWiring<
        fn(&mut State) -> &mut NumberEdit,
        fn(&mut State, f32),
        fn(&mut State),
        fn(&mut State, f32),
        fn(&mut State, f32),
    > {
        fn access(_: &mut State) -> &mut NumberEdit {
            panic!("construction only")
        }
        fn sink_v(_: &mut State, _: f32) {}
        fn sink(_: &mut State) {}
        NumWiring {
            access,
            commit: sink_v,
            reject: sink,
            scrub_write: sink_v,
            scrub_finish: sink_v,
        }
    }

    #[test]
    fn field_border_state_puts_invalid_above_focus() {
        // The invalid border must survive focus (the
        // user sees it WHILE fixing the text), and the
        // non-invalid states are field_border's own.
        for dark in [true, false] {
            let t = tok::theme(dark);
            assert_eq!(
                field_border_state(false, true, &t),
                app_theme::error_on(&t)
            );
            assert_eq!(
                field_border_state(true, true, &t),
                app_theme::error_on(&t)
            );
            assert_eq!(field_border_state(true, false, &t), field_border(true, &t));
            assert_eq!(
                field_border_state(false, false, &t),
                field_border(false, &t)
            );
            assert_ne!(field_border_state(true, true, &t), field_border(true, &t));
        }
    }

    #[test]
    fn num_spec_clamps_and_steps_on_its_own_numbers() {
        let spec = NumSpec::new(4.0, 220.0, 4.0);
        assert_eq!(spec.clamp(2.0), 4.0);
        assert_eq!(spec.clamp(120.0), 120.0);
        assert_eq!(spec.clamp(500.0), 220.0);
        // A NaN reads as the floor, never into a project.
        assert_eq!(spec.clamp(f32::NAN), 4.0);
        assert_eq!(spec.stepped(120.0, 1), 124.0);
        assert_eq!(spec.stepped(120.0, -1), 116.0);
        assert_eq!(spec.stepped(220.0, 1), 220.0);
        // Forms: Noop only where the step pushes on the
        // clamp it would land on — the StepForm rule,
        // computed from the spec's own numbers.
        assert_eq!(spec.step_form(220.0, 1), StepForm::Noop);
        assert_eq!(spec.step_form(4.0, -1), StepForm::Noop);
        assert_eq!(spec.step_form(220.0, -1), StepForm::Live);
        assert_eq!(spec.step_form(4.0, 1), StepForm::Live);
        assert_eq!(spec.step_form(120.0, 1), StepForm::Live);
    }

    #[test]
    fn num_spec_formats_and_parses_in_display_units() {
        let len = NumSpec::new(4.0, 220.0, 4.0);
        assert_eq!(len.format(120.0), "120");
        assert_eq!(len.edit_text(120.0), "120");
        assert_eq!(len.format(-0.0), "0", "no negative zero in a readout");
        assert_eq!(len.parse("120"), NumParse::Value(120.0));
        assert_eq!(len.parse(" 36 "), NumParse::Value(36.0));
        assert_eq!(len.parse(""), NumParse::Empty);
        assert_eq!(len.parse("abc"), NumParse::Invalid);
        // NaN/infinity spellings parse as f32 in Rust;
        // the field refuses them.
        assert_eq!(len.parse("NaN"), NumParse::Invalid);
        assert_eq!(len.parse("inf"), NumParse::Invalid);
        // Parse does NOT clamp — the commit path does,
        // visibly, through the same spec.
        assert_eq!(len.parse("999"), NumParse::Value(999.0));
        assert_eq!(len.clamp(999.0), 220.0);

        let deg = NumSpec::degrees(-180.0, 180.0, 5.0);
        assert_eq!(deg.format(-45.0), "-45°");
        assert_eq!(deg.edit_text(-45.0), "-45");
        assert_eq!(deg.parse("30°"), NumParse::Value(30.0));
        assert_eq!(deg.parse("30"), NumParse::Value(30.0));
        // Any of the shell's unit marks is accepted, not
        // only the field's own suffix.
        assert_eq!(deg.parse("30%"), NumParse::Value(30.0));
        assert_eq!(deg.parse("°"), NumParse::Empty);

        let pct = NumSpec::percent(0.05, 1.0, 0.05);
        assert_eq!(pct.format(0.5), "50%");
        assert_eq!(pct.edit_text(0.5), "50");
        assert_eq!(pct.parse("50%"), NumParse::Value(0.5));
        assert_eq!(pct.parse("50"), NumParse::Value(0.5));
        assert_eq!(pct.format(0.0), "0%");
    }

    #[test]
    fn num_spec_slider_mapping_and_scrub() {
        let spec = NumSpec::percent(0.05, 1.0, 0.05);
        assert_eq!(spec.frac_of(0.05), 0.0);
        assert_eq!(spec.frac_of(1.0), 1.0);
        assert!((spec.frac_of(0.525) - 0.5).abs() < 1e-6);
        assert!((spec.value_at_frac(0.5) - 0.525).abs() < 1e-6);
        assert_eq!(spec.value_at_frac(-1.0), 0.05);
        assert_eq!(spec.value_at_frac(2.0), 1.0);
        // An empty range has no fraction.
        assert_eq!(NumSpec::new(5.0, 5.0, 1.0).frac_of(5.0), 0.0);

        // Scrub: 2 pt per 4-unit step — travel converts
        // at the spec's rate and clamps at the range.
        let len = NumSpec::new(4.0, 220.0, 4.0);
        assert_eq!(len.scrubbed(100.0, 0.0), 100.0);
        assert_eq!(len.scrubbed(100.0, 20.0), 140.0);
        assert_eq!(len.scrubbed(100.0, -20.0), 60.0);
        assert_eq!(len.scrubbed(100.0, 10_000.0), 220.0);
        assert_eq!(len.scrubbed(100.0, f32::NAN), 100.0);
    }

    #[test]
    fn number_edit_session_machine() {
        let spec = NumSpec::new(4.0, 220.0, 4.0);
        let mut edit = NumberEdit::new();
        assert!(!edit.is_open());
        edit.begin(spec.edit_text(120.0));
        assert!(edit.is_open());
        assert!(edit.edit.focused, "a fresh session claims focus");
        assert_eq!(edit.text(), "120");
        assert_eq!(edit.validity(&spec), NumValidity::Valid);
        // Live validity follows the buffer.
        edit.edit.replace_text("abc".to_string());
        assert_eq!(edit.validity(&spec), NumValidity::Invalid);
        edit.edit.replace_text("".to_string());
        assert_eq!(edit.validity(&spec), NumValidity::Empty);
        // A session still holding focus is untouched by
        // the poll reconciliation.
        assert!(!edit.reconcile());
        assert!(edit.is_open());
        // Escape blurs the field (framework); the sync
        // reconciliation reverts to the seed.
        edit.edit.focused = false;
        assert!(edit.reconcile());
        assert!(!edit.is_open());
        assert_eq!(edit.text(), "120");
        // The commit path: close leaves the buffer as
        // typed (the display re-derives from the value).
        edit.begin("120".to_string());
        edit.edit.replace_text("150".to_string());
        edit.close();
        assert!(!edit.is_open());
        assert_eq!(edit.text(), "150");
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
            let _ = empty_state_action("play", "Nothing here", "empty-act", "Add one", &t, |_| {});
            let _ = empty_state_action("no-such-icon", "Nothing here", "empty-act", "Add one", &t, |_| {});
            let _ = list_row("row", true, &t);
            let _ = list_row("row", false, &t);
            let _ = row_shell(true, &t);
            let _ = row_shell(false, &t);
            let _ = toggle_row("tog-row", "Loop", true, &t, |_| {});
            let _ = toggle_row("tog-row", "Loop", false, &t, |_| {});
            let _ = stepper_value("42", &t);
            let _ = step_btn("step", "-", StepForm::Live, &t, |_| {});
            let _ = step_btn("step", "-", StepForm::Noop, &t, |_| {});
            let _ = step_btn("step", "-", StepForm::MutedLive, &t, |_| {});
            let _ = text_field(&TextEdit::new("x"), type_::BODY, true, &t);
            let _ = text_field(&TextEdit::new(""), type_::BODY, false, &t);
            let _ = text_field_flagged(&TextEdit::new("x"), type_::BODY, true, true, &t);
            let _ = text_field_flagged(&TextEdit::new(""), type_::BODY, false, false, &t);
            // The numeric primitive (Roadmap Wave 1,
            // F-3): the field in both forms (closed and
            // open sessions, invalid hint shown and not)
            // and the bounded composite.
            let spec = NumSpec::new(4.0, 220.0, 4.0);
            let closed = NumberEdit::new();
            let _ = number_field(
                "nf", &closed, &spec, 120.0, "120", "Length", None, &t, test_wiring(),
            );
            let mut open = NumberEdit::new();
            open.begin("120".to_string());
            let _ = number_field(
                "nf",
                &open,
                &spec,
                120.0,
                "120",
                "Length",
                Some("Not a number"),
                &t,
                test_wiring(),
            );
            let _ = bounded_value_row(
                "b-slider",
                "b-field",
                "b-dn",
                "b-up",
                "Length",
                &closed,
                &spec,
                120.0,
                "120",
                None,
                &t,
                test_wiring(),
                |_, _| {},
                |_, _, _| {},
            );
            let _ = slider_track(0.0, &t);
            let _ = slider_track(0.5, &t);
            let _ = slider_track(1.0, &t);
            let _ = slider_track(f32::NAN, &t);
            let _ = v_splitter("vs", "Drag to resize", &t, |_, _| {});
            let _ = swatch("sw", Color::WHITE, true, &t, |_| {});
            let _ = swatch("sw", Color::BLACK, false, &t, |_| {});
            let _ = swatch_named("swn", Color::WHITE, "#FFFFFF", true, &t, |_| {});
            let _ = swatch_named("swn", Color::BLACK, "#000000", false, &t, |_| {});
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
    fn a11y_wiring_constructs_on_the_kit_controls() {
        // Roadmap Wave 2 (W2-51, audit A20-11): the kit
        // controls that gained AT wiring in this wave —
        // the toggle row's name + checked state, the
        // bounded row's slider role/name/range (percent
        // spec included, so the display-domain scaling
        // of the range is exercised), the number field's
        // name in both forms, the splitter's separator
        // role + name, the named swatch, the tangent
        // chip's name. The wired values are not readable
        // back from an element in a bin-crate unit test
        // (the standing limit), so these pin
        // construction; the presented-tree proof is the
        // headless census re-run (IA-W2-51-9).
        let t = tok::theme(true);
        let _ = toggle_row("a11y-tog", "Loop", true, &t, |_| {});
        let _ = toggle_row("a11y-tog", "Loop", false, &t, |_| {});
        let spec = NumSpec::percent(0.05, 1.0, 0.05);
        let closed = NumberEdit::new();
        let _ = number_field(
            "a11y-nf", &closed, &spec, 0.5, "50%", "Opacity", None, &t, test_wiring(),
        );
        let _ = bounded_value_row(
            "a11y-slider",
            "a11y-field",
            "a11y-dn",
            "a11y-up",
            "Opacity",
            &closed,
            &spec,
            0.5,
            "50%",
            None,
            &t,
            test_wiring(),
            |_, _| {},
            |_, _, _| {},
        );
        let _ = v_splitter("a11y-split", "Drag to resize the sidebar", &t, |_, _| {});
        let _ = swatch_named("a11y-sw", Color::WHITE, "White", true, &t, |_| {});
        let _ = tangent_preset_chip(
            "a11y-tp",
            TangentPreset::Linear,
            "Linear",
            false,
            &t,
            |_| {},
        );
    }

    #[test]
    fn v_splitter_geometry_is_a_hairline_in_a_grabbable_strip() {
        // The wiring layout budgets SPLITTER_W for the
        // strip; the hairline inside it is the divider's
        // 1 pt, and the grip glyph must fit the strip
        // with air either side rather than spilling
        // over the panels it separates.
        assert_eq!(SPLITTER_LINE_W, 1.0);
        assert!(SPLITTER_LINE_W < SPLITTER_W);
        assert!(SPLITTER_GRIP < SPLITTER_W);
        assert!(SPLITTER_GAP > 0.0);
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
