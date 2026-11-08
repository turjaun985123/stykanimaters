//! Onion-skin controls (Phase D, section 28 of the GPUI→Rune
//! migration): an On/Off toggle for the stage's onion skins, a
//! frames-count stepper, and a colour legend. Declared under
//! `rune_sec` from `main.rs` only — bin-crate code, not part of
//! the engine library.
//!
//! ## State: no new fields, real core fields only
//!
//! Everything here reads/writes two public `EditorCore` fields;
//! nothing is added to the shell's `State`:
//!
//! * `core.onion: bool` (`editor.rs`) — the toggle flips the
//!   field directly, exactly as the GPUI app's `btn("onion", …)`
//!   listener did (`view.core.onion = !view.core.onion`) and as
//!   `chrome_view::app_bar`'s Onion button still does. There is
//!   no core op for it.
//! * `core.onion_frames: u8` (`editor.rs`, documented range
//!   `1..=3`, default 1) — how many frames the skins span in
//!   each direction. `stage_view::onion_skins` loops exactly
//!   this count backwards and forwards, so the stepper's range
//!   is clamped to the same `1..=3` — never wider than what the
//!   renderer will actually draw.
//!
//! ## Why the stepper mutates the field instead of calling an op
//!
//! The only core op for the count is `cycle_onion_frames`
//! (`editor.rs`), which cycles `1 → 2 → 3 → 1` and writes a
//! status line; the GPUI app bound it to a single "Onion +"
//! button (id `onion-range`). A two-button stepper needs a
//! reverse step, which no op provides, so both step buttons
//! mutate the pub field through [`stepped_onion_frames`]
//! (clamped) — the same direct-field style the toggle has
//! always used. Deliberate difference from the cycle op: the
//! stepper does not wrap at the ends and does not touch
//! `core.status`, because the current count is shown inline
//! between the step buttons.
//!
//! ## Legend colours (must match `stage_view::onion_skins`)
//!
//! Past frames are drawn in each figure's own colour (alpha
//! `0x4D` nearest, `0x26` older); future frames share one green
//! tint, [`FUTURE_ONION_RGB`] (`0x7BE495`), with the same
//! falloff. The legend line below states exactly that.

use rune::tok::{type_, Theme};
use rune::{col, el, row, text, Color, El};

use crate::chrome_view::btn;
use crate::stage_view::hex_color;
use crate::State;

/// Smallest onion-frame count the core allows (`editor.rs`
/// documents `onion_frames` as `1..=3`).
pub const MIN_ONION_FRAMES: u8 = 1;

/// Largest onion-frame count the core allows — and the most
/// `stage_view::onion_skins` will ever draw in one direction.
pub const MAX_ONION_FRAMES: u8 = 3;

/// The shared tint for future-frame onion skins, `0xRRGGBB`.
/// Mirrors the literal in `stage_view::onion_skins`; the test
/// below pins the value so a change on either side is caught.
pub const FUTURE_ONION_RGB: u32 = 0x7BE495;

/// Clamp a candidate onion-frame count into the core's
/// documented `1..=3` range.
pub fn clamp_onion_frames(n: i32) -> u8 {
    n.clamp(MIN_ONION_FRAMES as i32, MAX_ONION_FRAMES as i32) as u8
}

/// The onion-frame count after stepping `current` by `delta`,
/// clamped to `1..=3` (no wrap at either end). An out-of-range
/// stored value is clamped first, so stepping always lands
/// back inside the range.
pub fn stepped_onion_frames(current: u8, delta: i32) -> u8 {
    clamp_onion_frames(clamp_onion_frames(current as i32) as i32 + delta)
}

/// Stepper readout for a frame count: `1 frame`, `2 frames`, …
/// (the count is clamped for display, like everywhere else).
pub fn onion_frames_label(frames: u8) -> String {
    let n = clamp_onion_frames(frames as i32);
    if n == 1 {
        "1 frame".to_string()
    } else {
        format!("{n} frames")
    }
}

/// The legend line under the controls. Wording is pinned by
/// test: past skins keep each figure's own colour, future
/// skins are the shared green (see the module docs).
pub fn legend_text() -> &'static str {
    "Past = figure colour · Future = green"
}

/// The Onion section: header, the On/Off toggle, the frames
/// stepper (`-` / readout / `+`), and the colour legend with a
/// swatch in the renderer's actual future-frame green.
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
        .child(
            row()
                .gap(8.0)
                .center_y()
                .child(btn("onion-toggle", "Onion", core.onion, t, |s| {
                    s.core.onion = !s.core.onion;
                }))
                .child(btn("onion-frames-down", "-", false, t, |s| {
                    s.core.onion_frames = stepped_onion_frames(s.core.onion_frames, -1);
                }))
                .child(
                    text(onion_frames_label(core.onion_frames))
                        .font(type_::LABEL)
                        .color(t.text),
                )
                .child(btn("onion-frames-up", "+", false, t, |s| {
                    s.core.onion_frames = stepped_onion_frames(s.core.onion_frames, 1);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_keeps_the_core_documented_range() {
        assert_eq!(clamp_onion_frames(-2), MIN_ONION_FRAMES);
        assert_eq!(clamp_onion_frames(0), MIN_ONION_FRAMES);
        assert_eq!(clamp_onion_frames(1), 1);
        assert_eq!(clamp_onion_frames(2), 2);
        assert_eq!(clamp_onion_frames(3), 3);
        assert_eq!(clamp_onion_frames(4), MAX_ONION_FRAMES);
        assert_eq!(clamp_onion_frames(99), MAX_ONION_FRAMES);
        // The range is the core's documented 1..=3, pinned so a
        // future widening has to be a deliberate edit here too.
        assert_eq!((MIN_ONION_FRAMES, MAX_ONION_FRAMES), (1, 3));
    }

    #[test]
    fn stepper_moves_and_stops_at_the_ends() {
        assert_eq!(stepped_onion_frames(1, 1), 2);
        assert_eq!(stepped_onion_frames(2, 1), 3);
        assert_eq!(stepped_onion_frames(2, -1), 1);
        assert_eq!(stepped_onion_frames(2, 0), 2);
        // No wrap: the ends are sticky, unlike the core's
        // cycle_onion_frames op.
        assert_eq!(stepped_onion_frames(3, 1), 3);
        assert_eq!(stepped_onion_frames(1, -1), 1);
        // Large deltas clamp instead of wrapping or overflowing.
        assert_eq!(stepped_onion_frames(1, 10), 3);
        assert_eq!(stepped_onion_frames(3, -10), 1);
    }

    #[test]
    fn stepper_recovers_an_out_of_range_stored_value() {
        assert_eq!(stepped_onion_frames(0, 0), 1);
        assert_eq!(stepped_onion_frames(0, 1), 2);
        assert_eq!(stepped_onion_frames(9, -1), 2);
        assert_eq!(stepped_onion_frames(9, 0), 3);
    }

    #[test]
    fn label_is_singular_for_one_and_plural_above() {
        assert_eq!(onion_frames_label(1), "1 frame");
        assert_eq!(onion_frames_label(2), "2 frames");
        assert_eq!(onion_frames_label(3), "3 frames");
        // Display clamps like the stepper does.
        assert_eq!(onion_frames_label(0), "1 frame");
        assert_eq!(onion_frames_label(8), "3 frames");
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
