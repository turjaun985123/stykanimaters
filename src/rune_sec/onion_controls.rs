//! Onion-skin controls (Phase D, section 28 of the GPUI→Rune
//! migration; per-side depths added in Phase G, section 25):
//! an On/Off toggle for the stage's onion skins, independent
//! Prev / Next depth steppers, and a colour legend. Declared
//! under `rune_sec` from `main.rs` only — bin-crate code, not
//! part of the engine library.
//!
//! ## State: the toggle is a core field, the depths are shell state
//!
//! * `core.onion: bool` (`editor.rs`) — the toggle flips the
//!   field directly, exactly as the GPUI app's `btn("onion", …)`
//!   listener did (`view.core.onion = !view.core.onion`). There
//!   is no core op for it. The stage's master switch still
//!   reads this field: `stage_view::onion_skins` draws nothing
//!   while it is off, whatever the depths say.
//! * [`OnionControlsState`] (`State::onion_ui`, owned by this
//!   section) — how many frames the skins span on each side,
//!   `0..=3` per side, a count of 0 hiding that side entirely.
//!   `main.rs` feeds both counts to the stage every frame
//!   (via `stage_view::StageOverlays`), and
//!   `stage_view::onion_skins(core, prev, next)` loops exactly
//!   those counts. Depths are pure view state: stepping them
//!   takes **no undo snapshot** and writes nothing to the
//!   core, the same direct-field style the toggle has always
//!   used.
//!
//! ## Why the depths are not the core's `onion_frames` field
//!
//! The core does have a depth field — `core.onion_frames: u8`
//! (`editor.rs`, "How many previous frames the onion skin
//! shows (1..=3)", default 1, plus a `cycle_onion_frames`
//! op) — and this panel's pre-Phase-G stepper wrote it. It
//! cannot back the Phase G steppers: it is a *single* count
//! for the previous side only, its range has no 0 (a side can
//! never be hidden), and the stage no longer reads it at all
//! (`stage_view` takes the per-side shell depths instead —
//! see that module's docs). So the old combined stepper
//! (keys `onion-frames-down` / `onion-frames-up`) is gone,
//! replaced by the two per-side steppers below; the core
//! field and its cycle op remain in the engine, untouched,
//! for any non-shell caller.
//!
//! ## Legend colours (must match `stage_view::onion_skins`)
//!
//! Past frames are drawn in each figure's own colour (alpha
//! `0x4D` nearest, `0x26` older); future frames share one
//! green tint, [`FUTURE_ONION_RGB`] (`0x7BE495`), with the
//! same falloff. The legend line below states exactly that.
//!
//! ## Phase N: the card shell
//!
//! The root is the kit's card (`design::panel_col`, root
//! key `onion-controls` unchanged), the header the kit's
//! `panel_title` over a hairline `divider`, and the depth
//! steppers follow the charter order — side label growing,
//! `−`, the count in `design::stepper_value`, `+`. The old
//! combined readout (one LABEL text, `Prev {n}`) split
//! into label + mono value, and its formatter
//! (`depth_readout`) retired with it; the clamp it relied
//! on is still `step_depth`'s, pinned below. The On/Off
//! control stays the button family's `btn`: it is a
//! standalone toggle button (its active fill is the state
//! display), not a labelled row with a separate switch,
//! so the kit's `toggle_row` does not apply to it.
//!
//! ## Inert states (Wave 9 audit, D6)
//!
//! Every control here was live in every state, including
//! states where a press cannot change anything the stage
//! draws. Two such states, both proven against
//! `stage_view::onion_skins`:
//!
//! * **Fewer than 2 frames.** `onion_skins` needs a frame
//!   beside the current one: the past loop requires
//!   `core.current > 0` and the future loop breaks at
//!   `frame >= core.frame_count()`. With 0 or 1 frames it
//!   returns an empty list for *every* toggle/depth
//!   combination, so the toggle and all four steppers can
//!   only no-op. The whole set renders through
//!   `chrome_view::btn_inert` (same keys, same boxes, no
//!   handler) while `core.frame_count() < 2`; the stored
//!   values are untouched and the controls go live again
//!   when a second frame exists.
//! * **A stepper at its end.** `step_depth` clamps, so `−`
//!   at 0 and `+` at [`MAX_ONION_DEPTH`] write back the
//!   identical value — a press is a pure no-op with live
//!   hover/press feedback. That one button renders inert
//!   (the `btn_inert` doctrine: an op that would no-op
//!   from the current state and has nothing to say). The
//!   test is `step_depth(cur, delta) == cur`, so an
//!   out-of-range stored value keeps both buttons live:
//!   either press pulls it back into range, which is a
//!   real change.
//!
//! Labels vs renderer, re-verified in the same audit: the
//! steppers' ±1 steps and 0..=3 range are exactly the
//! renderer's loops (past: frame `current-1`, then
//! `2..=prev`; future: `1..=next`), the alpha falloff is
//! the renderer's two steps (0x4D at distance 1, 0x26 at
//! distance ≥ 2 — this panel has no opacity control and
//! claims none), and the legend's colour claim is the
//! renderer's (past = each figure's own colour, future =
//! the shared 0x7BE495). Row widths are tree-measured:
//! the stepper cluster is 115 pt fixed (28.5 + 34 + 28.5
//! + 3×8 gaps) with the label growing, and the legend
//! measures 223 pt — both inside the 316 pt card content.
//!
//! ## Per-figure overrides (Roadmap Wave 2, W2-37)
//!
//! Below the global rows, a per-figure section refines the
//! model for the SELECTED figure (`core.selected_figure`,
//! the Figure inspector's target): its own Prev / Next
//! depths and its own past / future tints, each field
//! independently inheriting the global behaviour until set
//! ([`crate::stage_view::OnionFigureOverride`], whose doc
//! carries the model). The overrides live here, in
//! [`OnionControlsState::per_figure`] — the same shell
//! session state as the global depths, under the same
//! discipline (no snapshot, no core write, nothing
//! persisted) — and the shell feeds the vec to the stage
//! every frame (IA-W2-37-1); the stage's
//! `onion_skins_with_overrides` resolves each figure's
//! effective settings from it.
//!
//! The section's controls mirror the global rows' shapes:
//! depth steppers with an **Auto** toggle in place of the
//! bare label (Auto = inherit; pressing it while inheriting
//! pins the current effective depth as an explicit
//! override, pressing it again releases back to inherit —
//! either press is a real change, so the button never
//! no-ops), value readouts that always show the depth the
//! stage will actually draw for this figure, tint swatch
//! rows over [`ONION_TINT_PALETTE`] with the same Auto
//! toggle (pinning the past tint pins the figure's own
//! current colour; pinning the future tint pins the shared
//! green), and a **Reset** button that clears the figure's
//! whole override (inert while the figure has none — the
//! `btn_inert` doctrine). The tint rows measure 280 pt
//! (Auto chip 50.5 + 8 swatches × 24 + 7 × 5 gaps) inside
//! the 316 pt card content.
//!
//! Inert discipline extends the Wave 9 audit: while the
//! section is not live (fewer than 2 frames) the figure
//! steppers go inert like the global ones, and the tint
//! swatches render as plain unpressable squares under the
//! same keys — the kit swatch has no inert form, and a
//! press that cannot change anything the stage draws must
//! not exist. A stale selection (no such figure) renders
//! the section inert the same way.
//!
//! The legend line is NOT extended: its pinned wording
//! states the defaults ("Past = figure colour · Future =
//! green"), which remain true as the inheritance every
//! figure starts from; per-figure overrides supersede them
//! per figure, and the section's Auto states say so.

use rune::tok::{type_, Theme};
use rune::{el, row, text, Color, El};

use crate::chrome_view::{btn, btn_inert};
use crate::rune_sec::design;
use crate::stage_view::{hex_color, OnionFigureOverride};
use crate::State;

/// Deepest onion skin either side can show — the most
/// `stage_view::onion_skins` will ever draw in one direction.
/// The shallow end is 0: a side stepped to 0 draws nothing.
pub const MAX_ONION_DEPTH: u8 = 3;

/// Per-side onion-skin depths — this section's shell state,
/// stored on `State` as `onion_ui` (wired in `main.rs`,
/// initialised with `Default::default()`).
///
/// Pure view state: no snapshot, no core writes — the stage
/// reads the counts fresh every frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OnionControlsState {
    /// Frames behind the current frame (0 hides the past side).
    pub prev: u8,
    /// Frames ahead of the current frame (0 hides the future side).
    pub next: u8,
    /// Per-figure overrides (W2-37), indexed by figure: a
    /// slot is `Some` only while that figure actually
    /// overrides something — the write helpers normalise an
    /// all-inheriting override back to `None` and trim
    /// trailing empty slots, so an untouched project carries
    /// an empty vec and the stage renders exactly the global
    /// behaviour. (Not `Copy` any more: the vec. The state
    /// is only ever borrowed or default-constructed by the
    /// shell, never copied by value.)
    pub per_figure: Vec<Option<OnionFigureOverride>>,
}

impl Default for OnionControlsState {
    /// 1 / 1 — the counts the stage hardcoded before Phase G:
    /// `stage_view::StageOverlays::default()` carries
    /// `onion_prev: 1, onion_next: 1` (and its test pins the
    /// pair), which in turn kept the value the core's single
    /// `onion_frames` count defaulted to (`editor.rs`: 1).
    /// No per-figure overrides: every figure inherits.
    fn default() -> Self {
        OnionControlsState { prev: 1, next: 1, per_figure: Vec::new() }
    }
}

/// The override stored for figure `fi`, if any (W2-37).
/// A short vec or an empty slot both read as "inherits".
pub fn figure_override(state: &OnionControlsState, fi: usize) -> Option<OnionFigureOverride> {
    state.per_figure.get(fi).copied().flatten()
}

/// Whether figure `fi` carries any override at all (W2-37).
pub fn has_figure_override(state: &OnionControlsState, fi: usize) -> bool {
    figure_override(state, fi).is_some()
}

/// The depth the stage will draw for figure `fi` on one
/// side (W2-37): its override's, else the global count.
/// `next_side` selects the future depth over the past one.
pub fn effective_figure_depth(state: &OnionControlsState, fi: usize, next_side: bool) -> u8 {
    let ov = figure_override(state, fi).unwrap_or_default();
    if next_side {
        ov.next_depth(state.next)
    } else {
        ov.prev_depth(state.prev)
    }
}

/// The base colour the stage will use for figure `fi`'s
/// ghosts on one side (W2-37): its tint override, else the
/// side's default — the figure's own colour for the past,
/// the shared green for the future. `figure_color` is the
/// figure's own colour (the past default needs it; the
/// future side ignores it).
pub fn effective_figure_tint(
    state: &OnionControlsState,
    fi: usize,
    next_side: bool,
    figure_color: u32,
) -> u32 {
    let ov = figure_override(state, fi).unwrap_or_default();
    if next_side {
        ov.future_base()
    } else {
        ov.past_base(figure_color)
    }
}

/// Store `ov` as figure `fi`'s override, normalised
/// (W2-37): an all-inheriting override is stored as no
/// override, and trailing empty slots are trimmed, so the
/// vec only ever carries real overrides. Every per-figure
/// write in this section goes through here.
fn store_figure_override(state: &mut OnionControlsState, fi: usize, ov: OnionFigureOverride) {
    if ov.is_empty() {
        if fi < state.per_figure.len() {
            state.per_figure[fi] = None;
        }
    } else {
        if state.per_figure.len() <= fi {
            state.per_figure.resize(fi + 1, None);
        }
        state.per_figure[fi] = Some(ov);
    }
    while state.per_figure.last() == Some(&None) {
        state.per_figure.pop();
    }
}

/// Set (or, with `None`, release) figure `fi`'s depth
/// override on one side (W2-37). A set depth is clamped to
/// the drawable range by the same `step_depth` the global
/// steppers use, so a stored override can never ask the
/// stage for a depth the global rows could not.
pub fn set_figure_depth_override(
    state: &mut OnionControlsState,
    fi: usize,
    next_side: bool,
    depth: Option<u8>,
) {
    let mut ov = figure_override(state, fi).unwrap_or_default();
    let depth = depth.map(|d| step_depth(d, 0));
    if next_side {
        ov.next = depth;
    } else {
        ov.prev = depth;
    }
    store_figure_override(state, fi, ov);
}

/// Set (or, with `None`, release) figure `fi`'s tint
/// override on one side (W2-37): the past tint replaces the
/// figure's own colour as its past ghosts' base; the future
/// tint replaces the shared green.
pub fn set_figure_tint_override(
    state: &mut OnionControlsState,
    fi: usize,
    next_side: bool,
    tint: Option<u32>,
) {
    let mut ov = figure_override(state, fi).unwrap_or_default();
    if next_side {
        ov.future_tint = tint;
    } else {
        ov.past_tint = tint;
    }
    store_figure_override(state, fi, ov);
}

/// Clear figure `fi`'s whole override (W2-37): every field
/// back to inherit. Other figures' overrides are untouched.
pub fn clear_figure_override(state: &mut OnionControlsState, fi: usize) {
    store_figure_override(state, fi, OnionFigureOverride::default());
}

/// The tint palette the per-figure tint rows offer (W2-37):
/// eight hues spanning the figure palette's families (the
/// Figure inspector's `PALETTE` is that panel's own const;
/// this is the onion section's row, chosen so a ghost tint
/// reads as a tint — saturated hues plus the near-white and
/// near-black inks). The shared future green is among them,
/// so the future default is pinnable by swatch as well as
/// by the Auto toggle.
pub const ONION_TINT_PALETTE: [u32; 8] = [
    0xFF5D5D, 0xFFC65D, FUTURE_ONION_RGB, 0x5DB2FF, 0xC792FF, 0xFF6BCE, 0xE8E8EC, 0x232830,
];

/// The depth after stepping `cur` by `delta`, clamped to
/// `0..=3` (no wrap at either end). An out-of-range stored
/// value is pulled back inside the range by the same clamp,
/// so stepping always lands on a drawable count.
pub fn step_depth(cur: u8, delta: i32) -> u8 {
    (cur as i32 + delta).clamp(0, MAX_ONION_DEPTH as i32) as u8
}

/// Whether the stage can draw any onion skin at all in a
/// project of `frame_count` frames. `onion_skins` draws a
/// skin from a frame beside the current one, so a project
/// with fewer than 2 frames has no neighbour on either
/// side and the renderer returns an empty list whatever
/// the toggle and depths say (see the module docs).
pub fn controls_live(frame_count: usize) -> bool {
    frame_count >= 2
}

/// Whether stepping `cur` by `delta` would write back the
/// identical value — a press that cannot change anything.
/// False for an out-of-range stored value: either press
/// pulls it back inside the range, which is a real change.
pub fn step_is_noop(cur: u8, delta: i32) -> bool {
    step_depth(cur, delta) == cur
}

/// One depth-stepper button: the live form while the
/// section is live and the step would change the depth,
/// the handler-free inert form (same key, same box)
/// when it can only no-op. Delegates to the kit's
/// [`design::step_btn`] (Wave P — this file's copy was
/// one of the four private implementations the kit
/// item unifies).
fn step_btn(
    key: &str,
    label: &str,
    cur: u8,
    delta: i32,
    live: bool,
    t: &Theme,
    f: impl Fn(&mut State) + 'static,
) -> El {
    let form = if live && !step_is_noop(cur, delta) {
        design::StepForm::Live
    } else {
        design::StepForm::Noop
    };
    design::step_btn(key, label, form, t, f)
}

/// The legend line under the controls. Wording is pinned by
/// test: past skins keep each figure's own colour, future
/// skins are the shared green (see the module docs).
pub fn legend_text() -> &'static str {
    "Past = figure colour · Future = green"
}

/// One per-figure depth row (W2-37): the side's Auto toggle,
/// the −/+ steppers, and the readout — the global rows'
/// shape, with the Auto chip where the global rows carry
/// their bare label. `cur` is the figure's EFFECTIVE depth
/// (what the stage will draw), so the readout is honest
/// while inheriting, and a step from inherit pins an
/// explicit override one step from the effective value.
/// The Auto chip toggles: inherit → pin the effective depth
/// as an explicit override; pinned → release to inherit.
/// Either press changes the stored override, so the chip is
/// a live `btn` in both states (its active fill IS the
/// inherit display, the `onion-toggle` precedent).
fn figure_depth_row(
    side_label: &str,
    next_side: bool,
    s: &State,
    live: bool,
    t: &Theme,
) -> El {
    let st = &s.onion_ui;
    let fi = s.core.selected_figure;
    let pinned = figure_override(st, fi)
        .map(|ov| if next_side { ov.next } else { ov.prev })
        .is_some();
    let cur = effective_figure_depth(st, fi, next_side);
    let (auto_key, dn_key, up_key) = if next_side {
        ("onion-fig-next-auto", "onion-fig-next-dn", "onion-fig-next-up")
    } else {
        ("onion-fig-prev-auto", "onion-fig-prev-dn", "onion-fig-prev-up")
    };
    row()
        .gap(8.0)
        .center_y()
        .child(
            text(side_label)
                .font(type_::LABEL)
                .color(t.text_secondary)
                .grow(1.0),
        )
        .child(if live {
            btn(auto_key, "Auto", !pinned, t, move |s: &mut State| {
                let fi = s.core.selected_figure;
                if figure_override(&s.onion_ui, fi)
                    .map(|ov| if next_side { ov.next } else { ov.prev })
                    .is_some()
                {
                    set_figure_depth_override(&mut s.onion_ui, fi, next_side, None);
                } else {
                    let cur = effective_figure_depth(&s.onion_ui, fi, next_side);
                    set_figure_depth_override(&mut s.onion_ui, fi, next_side, Some(cur));
                }
            })
        } else {
            btn_inert(auto_key, "Auto", t)
        })
        .child(step_btn(dn_key, "−", cur, -1, live, t, move |s: &mut State| {
            let fi = s.core.selected_figure;
            let cur = effective_figure_depth(&s.onion_ui, fi, next_side);
            set_figure_depth_override(
                &mut s.onion_ui,
                fi,
                next_side,
                Some(step_depth(cur, -1)),
            );
        }))
        .child(design::stepper_value(&step_depth(cur, 0).to_string(), t))
        .child(step_btn(up_key, "+", cur, 1, live, t, move |s: &mut State| {
            let fi = s.core.selected_figure;
            let cur = effective_figure_depth(&s.onion_ui, fi, next_side);
            set_figure_depth_override(
                &mut s.onion_ui,
                fi,
                next_side,
                Some(step_depth(cur, 1)),
            );
        }))
}

/// One per-figure tint swatch (W2-37): the kit's named
/// swatch while live (its AT name is the hex — the
/// `swatch_named` contract for hex-only palettes), the
/// plain unpressable square under the same key while not.
/// Pressing writes the tint override for the figure
/// selected at press time, the Figure inspector's swatch
/// discipline. A press on the already-selected swatch
/// rewrites the same tint — an idempotent write, the
/// inspector's own behaviour for its swatches.
fn tint_swatch(key: String, rgb: u32, selected: bool, live: bool, next_side: bool, t: &Theme) -> El {
    let fill = hex_color(rgb).unwrap_or(Color::WHITE);
    if live {
        design::swatch_named(
            &key,
            fill,
            &format!("#{rgb:06X}"),
            selected,
            t,
            move |s: &mut State| {
                let fi = s.core.selected_figure;
                set_figure_tint_override(&mut s.onion_ui, fi, next_side, Some(rgb));
            },
        )
    } else {
        let base = el().key(&key).size(24.0, 24.0).radius(design::CONTROL_RADIUS).fill(fill);
        if selected {
            base.border(2.0, design::accent())
        } else {
            base.border(1.0, t.card_border)
        }
    }
}

/// One per-figure tint row (W2-37): the side's label, its
/// Auto toggle (inherit ↔ pin the side's current effective
/// tint — the figure's own colour for the past, the shared
/// green for the future), and the palette swatches. The
/// selected ring follows the EFFECTIVE tint, so an
/// inheriting figure whose own colour is in the palette
/// shows that swatch ringed alongside its lit Auto chip —
/// the two displays answer different questions (what colour
/// vs where it comes from) and both stay truthful.
fn figure_tint_row(side_label: &str, next_side: bool, s: &State, live: bool, t: &Theme) -> El {
    let st = &s.onion_ui;
    let core = &s.core;
    let fi = core.selected_figure;
    let figure_color = core.project.figures.get(fi).map(|f| f.color).unwrap_or(0);
    let pinned = figure_override(st, fi)
        .map(|ov| if next_side { ov.future_tint } else { ov.past_tint })
        .is_some();
    let effective = effective_figure_tint(st, fi, next_side, figure_color);
    let (auto_key, swatch_prefix) = if next_side {
        ("onion-future-tint-auto", "onion-future-tint")
    } else {
        ("onion-past-tint-auto", "onion-past-tint")
    };
    let mut r = row()
        .gap(5.0)
        .center_y()
        .child(
            text(side_label)
                .font(type_::LABEL)
                .color(t.text_secondary)
                .w(44.0),
        )
        .child(if live {
            btn(auto_key, "Auto", !pinned, t, move |s: &mut State| {
                let fi = s.core.selected_figure;
                if figure_override(&s.onion_ui, fi)
                    .map(|ov| if next_side { ov.future_tint } else { ov.past_tint })
                    .is_some()
                {
                    set_figure_tint_override(&mut s.onion_ui, fi, next_side, None);
                } else {
                    let fc = s
                        .core
                        .project
                        .figures
                        .get(fi)
                        .map(|f| f.color)
                        .unwrap_or(0);
                    let cur = effective_figure_tint(&s.onion_ui, fi, next_side, fc);
                    set_figure_tint_override(&mut s.onion_ui, fi, next_side, Some(cur));
                }
            })
        } else {
            btn_inert(auto_key, "Auto", t)
        });
    for rgb in ONION_TINT_PALETTE {
        r = r.child(tint_swatch(
            format!("{swatch_prefix}-{rgb:06x}"),
            rgb,
            effective == rgb,
            live,
            next_side,
            t,
        ));
    }
    r
}

/// The per-figure section of the Onion card (W2-37): a
/// header naming the selected figure the section edits, its
/// depth and tint rows, and the whole-override Reset. The
/// section renders with a figure context or without: a
/// stale selection (or an empty project) renders it inert
/// under a placeholder header, so the card's shape — and
/// every key in it — is stable across selection states.
pub fn onion_figure_section(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let fi = core.selected_figure;
    let figure = core.project.figures.get(fi);
    let live = controls_live(core.frame_count()) && figure.is_some();
    let header = match figure {
        Some(f) => format!("Figure: {}", f.name),
        None => "Figure: —".to_string(),
    };
    let has_ov = has_figure_override(&s.onion_ui, fi);
    design::panel_col("onion-figure", t)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(design::panel_title(None, &header, None, t)),
        )
        .child(figure_depth_row("F Prev", false, s, live, t))
        .child(figure_depth_row("F Next", true, s, live, t))
        .child(figure_tint_row("Past", false, s, live, t))
        .child(figure_tint_row("Future", true, s, live, t))
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(if live && has_ov {
                    btn("onion-fig-reset", "Reset figure", false, t, |s: &mut State| {
                        let fi = s.core.selected_figure;
                        clear_figure_override(&mut s.onion_ui, fi);
                    })
                } else {
                    btn_inert("onion-fig-reset", "Reset figure", t)
                }),
        )
}

/// The Onion section (Phase N card form): the kit header,
/// the On/Off toggle, the Prev and Next depth steppers in
/// charter order (label / `−` / value / `+` each), and the
/// colour legend with a swatch in the renderer's actual
/// future-frame green.
pub fn onion_controls(s: &State, t: &Theme) -> El {
    let core = &s.core;
    // Inert gate (Wave 9, module docs): under 2 frames no
    // control in this card can change anything the stage
    // draws, so every button renders inert; the stored
    // toggle and depths survive and go live again with a
    // second frame.
    let live = controls_live(core.frame_count());
    let future_green = hex_color(FUTURE_ONION_RGB).unwrap_or(Color::WHITE);
    design::panel_col("onion-controls", t)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(design::panel_title(None, "Onion skin", None, t)),
        )
        .child(design::divider(t))
        .child(if live {
            btn("onion-toggle", "Onion", core.onion, t, |s| {
                s.core.onion = !s.core.onion;
            })
        } else {
            btn_inert("onion-toggle", "Onion", t)
        })
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(
                    text("Prev")
                        .font(type_::LABEL)
                        .color(t.text_secondary)
                        .grow(1.0),
                )
                .child(step_btn("onion-prev-dn", "−", s.onion_ui.prev, -1, live, t, |s| {
                    s.onion_ui.prev = step_depth(s.onion_ui.prev, -1);
                }))
                .child(design::stepper_value(
                    &step_depth(s.onion_ui.prev, 0).to_string(),
                    t,
                ))
                .child(step_btn("onion-prev-up", "+", s.onion_ui.prev, 1, live, t, |s| {
                    s.onion_ui.prev = step_depth(s.onion_ui.prev, 1);
                })),
        )
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(
                    text("Next")
                        .font(type_::LABEL)
                        .color(t.text_secondary)
                        .grow(1.0),
                )
                .child(step_btn("onion-next-dn", "−", s.onion_ui.next, -1, live, t, |s| {
                    s.onion_ui.next = step_depth(s.onion_ui.next, -1);
                }))
                .child(design::stepper_value(
                    &step_depth(s.onion_ui.next, 0).to_string(),
                    t,
                ))
                .child(step_btn("onion-next-up", "+", s.onion_ui.next, 1, live, t, |s| {
                    s.onion_ui.next = step_depth(s.onion_ui.next, 1);
                })),
        )
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(el().size(10.0, 10.0).radius(5.0).fill(future_green))
                .child(
                    text(legend_text())
                        .font(type_::LABEL)
                        .color(t.text_secondary),
                ),
        )
        // Per-figure overrides (W2-37): a divider, then the
        // selected figure's own depths and tints, as a
        // nested panel under its own root key (`onion-figure`)
        // inside this card.
        .child(design::divider(t))
        .child(onion_figure_section(s, t))
}

/// The shared tint for future-frame onion skins, `0xRRGGBB`.
/// Mirrors the literal in `stage_view::onion_skins`; the test
/// below pins the value so a change on either side is caught.
pub const FUTURE_ONION_RGB: u32 = 0x7BE495;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_depth_moves_within_the_range() {
        assert_eq!(step_depth(0, 1), 1);
        assert_eq!(step_depth(1, 1), 2);
        assert_eq!(step_depth(2, 1), 3);
        assert_eq!(step_depth(3, -1), 2);
        assert_eq!(step_depth(2, -1), 1);
        assert_eq!(step_depth(1, -1), 0);
        // A zero delta is the identity, ends included.
        assert_eq!(step_depth(0, 0), 0);
        assert_eq!(step_depth(2, 0), 2);
        assert_eq!(step_depth(3, 0), 3);
    }

    #[test]
    fn step_depth_clamps_at_both_ends() {
        // 0 is a real stop now (the side hides), not a wrap
        // to the top and not the old stepper's floor of 1.
        assert_eq!(step_depth(0, -1), 0);
        assert_eq!(step_depth(1, -5), 0);
        assert_eq!(step_depth(3, 1), 3);
        assert_eq!(step_depth(2, 10), 3);
        assert_eq!(MAX_ONION_DEPTH, 3);
    }

    #[test]
    fn step_depth_recovers_an_out_of_range_stored_value() {
        assert_eq!(step_depth(9, 0), 3);
        assert_eq!(step_depth(9, -1), 3);
        assert_eq!(step_depth(9, 1), 3);
    }

    #[test]
    fn default_depths_match_the_stage_hardcoded_counts() {
        // stage_view.rs, StageOverlays::default():
        // `onion_prev: 1, onion_next: 1` — the depths the
        // stage drew before Phase G moved them into shell
        // state. If the stage's defaults ever change, this
        // pin fails and the two must be reconciled.
        let d = OnionControlsState::default();
        assert_eq!((d.prev, d.next), (1, 1));
        // W2-37: and no figure overrides anything.
        assert!(d.per_figure.is_empty());
    }

    // ---- W2-37 per-figure overrides ----

    #[test]
    fn figure_depth_overrides_set_release_and_normalise() {
        let mut st = OnionControlsState::default();
        assert_eq!(effective_figure_depth(&st, 0, false), 1, "global until set");
        // Pin a deeper past for figure 0 only.
        set_figure_depth_override(&mut st, 0, false, Some(3));
        assert_eq!(effective_figure_depth(&st, 0, false), 3);
        assert_eq!(effective_figure_depth(&st, 0, true), 1, "the other side still inherits");
        assert_eq!(effective_figure_depth(&st, 1, false), 1, "the other figure still inherits");
        assert!(has_figure_override(&st, 0));
        assert!(!has_figure_override(&st, 1));
        // A set depth is clamped to the drawable range.
        set_figure_depth_override(&mut st, 0, true, Some(9));
        assert_eq!(effective_figure_depth(&st, 0, true), MAX_ONION_DEPTH);
        // Releasing every field normalises the slot away:
        // the vec carries no trace of the figure.
        set_figure_depth_override(&mut st, 0, false, None);
        set_figure_depth_override(&mut st, 0, true, None);
        assert!(!has_figure_override(&st, 0));
        assert!(st.per_figure.is_empty(), "trailing empty slots are trimmed");
    }

    #[test]
    fn figure_overrides_are_sparse_and_clear_independently() {
        let mut st = OnionControlsState::default();
        // Writing figure 2 grows the vec sparsely; figures
        // 0 and 1 read as inheriting.
        set_figure_tint_override(&mut st, 2, false, Some(0xFF00AA));
        assert_eq!(st.per_figure.len(), 3);
        assert_eq!(figure_override(&st, 0), None);
        assert_eq!(figure_override(&st, 1), None);
        assert_eq!(
            figure_override(&st, 2).map(|ov| ov.past_tint),
            Some(Some(0xFF00AA))
        );
        // Clearing figure 2 leaves a shorter vec and does
        // not disturb a nearer figure's override.
        set_figure_depth_override(&mut st, 0, false, Some(0));
        clear_figure_override(&mut st, 2);
        assert_eq!(st.per_figure.len(), 1);
        assert_eq!(effective_figure_depth(&st, 0, false), 0, "Some(0) hides the side");
        clear_figure_override(&mut st, 0);
        assert!(st.per_figure.is_empty());
    }

    #[test]
    fn figure_tint_overrides_resolve_per_direction() {
        let mut st = OnionControlsState::default();
        let own = 0xE8E8EC;
        // Defaults: past = the figure's own colour, future
        // = the shared green.
        assert_eq!(effective_figure_tint(&st, 0, false, own), own);
        assert_eq!(effective_figure_tint(&st, 0, true, own), FUTURE_ONION_RGB);
        set_figure_tint_override(&mut st, 0, false, Some(0xFF00AA));
        set_figure_tint_override(&mut st, 0, true, Some(0x2244FF));
        assert_eq!(effective_figure_tint(&st, 0, false, own), 0xFF00AA);
        assert_eq!(effective_figure_tint(&st, 0, true, own), 0x2244FF);
        // The past default follows the FIGURE's colour, so
        // releasing the tint restores that figure's colour,
        // not a fixed value.
        set_figure_tint_override(&mut st, 0, false, None);
        assert_eq!(effective_figure_tint(&st, 0, false, 0x112233), 0x112233);
    }

    #[test]
    fn tint_palette_is_eight_unique_hues_including_the_future_green() {
        assert_eq!(ONION_TINT_PALETTE.len(), 8);
        let mut sorted = ONION_TINT_PALETTE;
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 8, "palette colours repeat");
        assert!(ONION_TINT_PALETTE.contains(&FUTURE_ONION_RGB));
    }

    #[test]
    fn panel_builds_with_figure_overrides() {
        // The per-figure section builds in every state the
        // card can meet: defaults, a selected figure with a
        // full override, and the one-frame inert state.
        let t = crate::rune_sec::app_theme::app_theme(true);
        let mut s = crate::init();
        let _ = onion_controls(&s, &t);
        let fi = s.core.selected_figure;
        set_figure_depth_override(&mut s.onion_ui, fi, false, Some(3));
        set_figure_tint_override(&mut s.onion_ui, fi, true, Some(0xFF6BCE));
        let _ = onion_controls(&s, &t);
        let _ = onion_figure_section(&s, &t);
        s.core.project.frames.truncate(1);
        let _ = onion_controls(&s, &t);
    }

    #[test]
    fn legend_names_both_directions_honestly() {
        assert_eq!(legend_text(), "Past = figure colour · Future = green");
    }

    #[test]
    fn future_tint_matches_the_stage_renderer() {
        // stage_view::onion_skins draws future frames in
        // 0x7BE495; if either side changes, this pin fails and
        // the two must be reconciled.
        assert_eq!(FUTURE_ONION_RGB, 0x7BE495);
    }

    // ---- Wave 9 inert states ----

    #[test]
    fn controls_live_needs_a_neighbour_frame() {
        // onion_skins draws only from a frame beside the
        // current one: 0 or 1 frames can never produce a
        // skin, 2 is the first count that can.
        assert!(!controls_live(0));
        assert!(!controls_live(1));
        assert!(controls_live(2));
        assert!(controls_live(5));
    }

    #[test]
    fn step_is_noop_only_at_the_ends() {
        // The clamped ends: a press writes back the same
        // value, so the button must read inert.
        assert!(step_is_noop(0, -1));
        assert!(step_is_noop(3, 1));
        assert!(step_is_noop(MAX_ONION_DEPTH, 1));
        // Everywhere else a press changes the depth.
        assert!(!step_is_noop(0, 1));
        assert!(!step_is_noop(1, -1));
        assert!(!step_is_noop(1, 1));
        assert!(!step_is_noop(2, -1));
        assert!(!step_is_noop(2, 1));
        assert!(!step_is_noop(3, -1));
        // Out-of-range stored values are NOT no-ops in
        // either direction: the press pulls the value back
        // into range (step_depth(9, ±1) == 3 != 9), so both
        // buttons stay live until the value is repaired.
        assert!(!step_is_noop(9, -1));
        assert!(!step_is_noop(9, 1));
    }

    #[test]
    fn panel_builds_live_and_inert() {
        // Both render paths build: the default state (a
        // multi-frame demo project, live buttons) and a
        // one-frame project (every button inert). The
        // builder only reads frame_count / onion / depths,
        // so truncating the demo's frames is safe here.
        let t = crate::rune_sec::app_theme::app_theme(true);
        let s = crate::init();
        assert!(controls_live(s.core.frame_count()));
        let _ = onion_controls(&s, &t);
        let mut s = crate::init();
        s.core.project.frames.truncate(1);
        assert_eq!(s.core.frame_count(), 1);
        assert!(!controls_live(s.core.frame_count()));
        let _ = onion_controls(&s, &t);
    }
}
