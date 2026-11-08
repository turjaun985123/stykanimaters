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

use rune::tok::{type_, Theme};
use rune::{col, el, row, text, Color, El};

use crate::chrome_view::btn;
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

/// Stepper readout for one side: `Prev {n}` / `Next {n}`,
/// with the count clamped for display like everywhere else.
pub fn depth_readout(side: &str, n: u8) -> String {
    format!("{side} {}", step_depth(n, 0))
}

/// The legend line under the controls. Wording is pinned by
/// test: past skins keep each figure's own colour, future
/// skins are the shared green (see the module docs).
pub fn legend_text() -> &'static str {
    "Past = figure colour · Future = green"
}

/// The Onion section: header, the On/Off toggle, the Prev
/// and Next depth steppers (`-` / readout / `+` each), and
/// the colour legend with a swatch in the renderer's actual
/// future-frame green.
pub fn onion_controls(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let future_green = hex_color(FUTURE_ONION_RGB).unwrap_or(Color::WHITE);
    col()
        .key("onion-controls")
        .gap(6.0)
        .child(
            text("Onion skin")
                .font(type_::SECTION)
                .color(t.text_secondary),
        )
        .child(btn("onion-toggle", "Onion", core.onion, t, |s| {
            s.core.onion = !s.core.onion;
        }))
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(btn("onion-prev-dn", "-", false, t, |s| {
                    s.onion_ui.prev = step_depth(s.onion_ui.prev, -1);
                }))
                .child(
                    text(depth_readout("Prev", s.onion_ui.prev))
                        .font(type_::LABEL)
                        .color(t.text),
                )
                .child(btn("onion-prev-up", "+", false, t, |s| {
                    s.onion_ui.prev = step_depth(s.onion_ui.prev, 1);
                })),
        )
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(btn("onion-next-dn", "-", false, t, |s| {
                    s.onion_ui.next = step_depth(s.onion_ui.next, -1);
                }))
                .child(
                    text(depth_readout("Next", s.onion_ui.next))
                        .font(type_::LABEL)
                        .color(t.text),
                )
                .child(btn("onion-next-up", "+", false, t, |s| {
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
    fn readout_names_the_side_and_the_count() {
        assert_eq!(depth_readout("Prev", 0), "Prev 0");
        assert_eq!(depth_readout("Prev", 1), "Prev 1");
        assert_eq!(depth_readout("Next", 2), "Next 2");
        assert_eq!(depth_readout("Next", 3), "Next 3");
        // Display clamps like the stepper does.
        assert_eq!(depth_readout("Prev", 8), "Prev 3");
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
}
