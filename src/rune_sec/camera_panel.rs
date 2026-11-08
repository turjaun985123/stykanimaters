//! Camera panel + stage-overlay cluster for the Rune editor
//! shell (Phase D, section 6). Bin-crate code — declared under
//! `rune_sec` by the integrator, not part of the engine library.
//!
//! Every control calls exactly one real `EditorCore` camera op
//! (verified in `editor.rs`):
//!
//! * Fit → `frame_all_figures` (editor.rs:582)
//! * Centre → `centre_on_selection` (editor.rs:629)
//! * 1:1, zoom steppers → `camera_zoom_by` (editor.rs:534)
//! * Rotation −15°/+15° → `rotate_camera` (editor.rs:1089)
//! * Set key → `rotate_camera(0.0)`: the engine's own
//!   `set_camera_key_here` upsert is private (editor.rs:555),
//!   and the other public upsert path (`camera_zoom_by` via
//!   `model::set_camera_key`) zeroes a fresh key's angle;
//!   `rotate_camera(0.0)` is the public op that upserts a key
//!   at the current frame carrying the pan/zoom/angle already
//!   in effect — exactly "set key here".
//! * Clear key → `camera_clear_key` (editor.rs:542)
//! * Follow → `toggle_camera_follow` (editor.rs:1699); its
//!   active state is `project.camera_follow ==
//!   Some(selected_figure)` (model.rs:699), the same condition
//!   the op itself toggles on.
//!
//! Readouts come from `EditorCore::camera_here` (editor.rs:530)
//! and `Project::camera_angle_at` (model.rs:904) — the camera
//! in effect at the current frame, interpolated keys included.
//!
//! Two entry points: [`panel`] is the full inspector-style
//! panel (labelled rows, rotation + key + follow controls);
//! [`overlay`] is the compact cluster the integrator floats
//! over the stage (Fit / Centre / 1:1 / zoom steppers + %
//! readout). Element keys reuse the GPUI-era ids where they
//! existed (`view-fit`, `view-centre`, `view-actual`).

use rune::tok::{self, type_, Theme};
use rune::{col, el, row, text, El};
use stykanimate_rs::editor::EditorCore;

use crate::chrome_view::btn;
use crate::State;

/// Zoom multiplier applied by one stepper press.
pub const ZOOM_STEP: f32 = 1.25;

/// Camera rotation applied by one rotation press, in degrees.
pub const ROTATE_STEP: f32 = 15.0;

/// The camera zoom in effect at the current frame, as a whole
/// percentage (100 = 1:1).
pub fn zoom_pct(core: &EditorCore) -> i64 {
    (core.camera_here().2 * 100.0).round() as i64
}

/// The zoom readout text, e.g. `"100%"`.
pub fn zoom_label(core: &EditorCore) -> String {
    format!("{}%", zoom_pct(core))
}

/// The camera angle in effect at the current frame, in
/// degrees (interpolated between keys).
pub fn angle_deg(core: &EditorCore) -> f32 {
    core.project.camera_angle_at(core.current as f32)
}

/// The angle readout text, e.g. `"0°"` / `"-15°"`.
pub fn angle_label(core: &EditorCore) -> String {
    format!("{:.0}°", angle_deg(core))
}

/// Whether the camera currently follows the selected figure —
/// the active state of the Follow toggle, matching the
/// condition `toggle_camera_follow` itself checks.
pub fn follow_active(core: &EditorCore) -> bool {
    core.project.camera_follow == Some(core.selected_figure)
}

/// The `camera_zoom_by` factor that takes `zoom` to exactly
/// 1.0 (the 1:1 / "actual size" view), or `None` when no op
/// call is needed: already at 1:1 (within a tenth of a
/// percent) or a non-positive zoom, which `camera_here` never
/// yields for a real project. Skipping the call matters —
/// `camera_zoom_by` snapshots, so a no-op 1:1 press would
/// otherwise push a dead entry onto the undo stack.
pub fn actual_factor(zoom: f32) -> Option<f32> {
    if zoom <= 0.0 || (zoom - 1.0).abs() <= 0.001 {
        None
    } else {
        Some(1.0 / zoom)
    }
}

/// 1:1 button handler: zoom to exactly 1.0 via
/// `camera_zoom_by`, unless already there.
fn zoom_actual(s: &mut State) {
    if let Some(f) = actual_factor(s.core.camera_here().2) {
        s.core.camera_zoom_by(f);
    }
}

/// "Set key here" handler: pin the camera in effect at the
/// current frame (pan, zoom and angle) as a key on this frame
/// — see the module docs for why this is `rotate_camera(0.0)`.
fn set_key_here(s: &mut State) {
    s.core.rotate_camera(0.0);
}

/// The zoom readout chip (`cam-zoom-label`): the current zoom
/// percentage as centred label text.
fn zoom_readout(core: &EditorCore, t: &Theme) -> El {
    el()
        .key("cam-zoom-label")
        .px(6.0)
        .center()
        .child(text(zoom_label(core)).font(type_::LABEL).color(t.text))
}

/// The angle readout chip (`cam-angle-label`).
fn angle_readout(core: &EditorCore, t: &Theme) -> El {
    el()
        .key("cam-angle-label")
        .px(6.0)
        .center()
        .child(text(angle_label(core)).font(type_::LABEL).color(t.text))
}

/// The full camera panel: a "Camera" header, the view row
/// (Fit / Centre / 1:1), the zoom stepper row, the rotation
/// stepper row, the key row (Set key / Clear key) and the
/// follow toggle.
pub fn panel(s: &State, t: &Theme) -> El {
    let core = &s.core;
    col()
        .key("camera-panel")
        .gap(6.0)
        .child(
            text("Camera")
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(btn("view-fit", "Fit", false, t, |s: &mut State| {
                    s.core.frame_all_figures()
                }))
                .child(btn("view-centre", "Centre", false, t, |s: &mut State| {
                    s.core.centre_on_selection()
                }))
                .child(btn("view-actual", "1:1", false, t, zoom_actual)),
        )
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(btn("cam-zoom-out", "-", false, t, |s: &mut State| {
                    s.core.camera_zoom_by(1.0 / ZOOM_STEP)
                }))
                .child(zoom_readout(core, t))
                .child(btn("cam-zoom-in", "+", false, t, |s: &mut State| {
                    s.core.camera_zoom_by(ZOOM_STEP)
                })),
        )
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(btn("cam-rot-left", "-15°", false, t, |s: &mut State| {
                    s.core.rotate_camera(-ROTATE_STEP)
                }))
                .child(angle_readout(core, t))
                .child(btn("cam-rot-right", "+15°", false, t, |s: &mut State| {
                    s.core.rotate_camera(ROTATE_STEP)
                })),
        )
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(btn("cam-set-key", "Set key", false, t, set_key_here))
                .child(btn("cam-clear-key", "Clear key", false, t, |s: &mut State| {
                    s.core.camera_clear_key()
                })),
        )
        .child(
            row()
                .gap(6.0)
                .center_y()
                .child(btn(
                    "cam-follow",
                    "Follow",
                    follow_active(core),
                    t,
                    |s: &mut State| s.core.toggle_camera_follow(),
                )),
        )
}

/// The compact stage-overlay cluster: Fit / Centre / 1:1 and
/// the zoom steppers around the % readout, on one row. The
/// integrator positions it over the stage (the audit's
/// `.absolute`/`.pin` out-of-flow placement); this builder
/// only supplies the cluster itself. Takes just the core —
/// readouts and active states need nothing else — and uses
/// the dark theme, as the shell's `view` does
/// (`tok::theme(true)` in `main.rs`).
pub fn overlay(core: &EditorCore) -> El {
    let t = tok::theme(true);
    row()
        .key("camera-overlay")
        .gap(4.0)
        .center_y()
        .child(btn("view-fit", "Fit", false, &t, |s: &mut State| {
            s.core.frame_all_figures()
        }))
        .child(btn("view-centre", "Centre", false, &t, |s: &mut State| {
            s.core.centre_on_selection()
        }))
        .child(btn("view-actual", "1:1", false, &t, zoom_actual))
        .child(btn("cam-zoom-out", "-", false, &t, |s: &mut State| {
            s.core.camera_zoom_by(1.0 / ZOOM_STEP)
        }))
        .child(zoom_readout(core, &t))
        .child(btn("cam-zoom-in", "+", false, &t, |s: &mut State| {
            s.core.camera_zoom_by(ZOOM_STEP)
        }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_label_is_100_percent_for_a_fresh_core() {
        let core = EditorCore::new();
        assert_eq!(zoom_pct(&core), 100);
        assert_eq!(zoom_label(&core), "100%");
    }

    #[test]
    fn zoom_label_tracks_camera_zoom_by() {
        let mut core = EditorCore::new();
        core.camera_zoom_by(2.0);
        assert_eq!(zoom_label(&core), "200%");
        core.camera_zoom_by(0.25);
        assert_eq!(zoom_label(&core), "50%");
    }

    #[test]
    fn actual_factor_inverts_the_current_zoom() {
        assert_eq!(actual_factor(2.0), Some(0.5));
        assert_eq!(actual_factor(0.5), Some(2.0));
        assert_eq!(actual_factor(4.0), Some(0.25));
    }

    #[test]
    fn actual_factor_skips_the_call_when_already_actual() {
        assert_eq!(actual_factor(1.0), None);
        assert_eq!(actual_factor(1.0005), None);
        // Non-positive zooms never come from `camera_here`;
        // treat them as "nothing to do" rather than dividing.
        assert_eq!(actual_factor(0.0), None);
        assert_eq!(actual_factor(-2.0), None);
    }

    #[test]
    fn angle_label_is_zero_for_a_fresh_core() {
        let core = EditorCore::new();
        assert_eq!(angle_label(&core), "0°");
    }

    #[test]
    fn angle_label_tracks_rotate_camera() {
        let mut core = EditorCore::new();
        core.rotate_camera(ROTATE_STEP);
        assert_eq!(angle_label(&core), "15°");
        core.rotate_camera(-2.0 * ROTATE_STEP);
        assert_eq!(angle_label(&core), "-15°");
    }

    #[test]
    fn follow_active_matches_the_ops_own_condition() {
        let mut core = EditorCore::new();
        assert!(!follow_active(&core));
        core.toggle_camera_follow();
        assert!(follow_active(&core));
        // Following a different figure is not "follow active"
        // for the selected one.
        core.project.camera_follow = Some(core.selected_figure + 1);
        assert!(!follow_active(&core));
        core.toggle_camera_follow();
        assert!(follow_active(&core));
    }

    #[test]
    fn steps_are_sane() {
        assert!(ZOOM_STEP > 1.0);
        assert_eq!(ROTATE_STEP, 15.0);
    }
}
