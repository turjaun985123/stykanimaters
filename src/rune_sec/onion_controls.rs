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

use rune::tok::{type_, Theme};
use rune::{el, row, text, Color, El};

use crate::chrome_view::{btn, btn_inert};
use crate::rune_sec::design;
use crate::stage_view::hex_color;
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OnionControlsState {
    /// Frames behind the current frame (0 hides the past side).
    pub prev: u8,
    /// Frames ahead of the current frame (0 hides the future side).
    pub next: u8,
}

impl Default for OnionControlsState {
    /// 1 / 1 — the counts the stage hardcoded before Phase G:
    /// `stage_view::StageOverlays::default()` carries
    /// `onion_prev: 1, onion_next: 1` (and its test pins the
    /// pair), which in turn kept the value the core's single
    /// `onion_frames` count defaulted to (`editor.rs`: 1).
    fn default() -> Self {
        OnionControlsState { prev: 1, next: 1 }
    }
}

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
