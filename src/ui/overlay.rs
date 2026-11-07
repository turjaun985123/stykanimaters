//! Viewport overlay helpers for the stage canvas: a
//! safe-frame guide rectangle, a centre cross, and the
//! stage-dimension label text.
//!
//! Presentation only — no state, no handlers, no editor
//! knowledge (the same contract as [`crate::ui_kit`]). The
//! view in `app.rs` layers [`overlay_canvas`] over the stage
//! viewport and renders [`stage_dimensions_label`] as a
//! normal text element wherever it wants the readout.
//!
//! ## Coordinate mapping
//!
//! The overlay draws in STAGE space through the same
//! transform `paint_stage` uses in `app.rs`:
//!
//! ```text
//! fit   = min(bounds_w / stage_w, bounds_h / stage_h)
//! s     = fit * zoom
//! pixel = bounds_origin + bounds_centre + (stage_pt - cam_centre) * s
//! ```
//!
//! `paint_stage` receives its camera (`cam_centre`, `zoom`)
//! as data. [`overlay_canvas`]'s fixed signature carries no
//! camera, so it paints the DEFAULT camera — centre on the
//! stage centre, zoom 1.0 — which is exactly the mapping
//! above with those values substituted. In other words the
//! guides align with the stage as fitted into the viewport
//! when the camera is untouched; if the caller pans/zooms
//! the camera, this overlay does NOT follow (it has no way
//! to know), and wiring a camera-aware variant means
//! threading `cam`/`zoom` parameters through, the way
//! `paint_stage` already does.
//!
//! ## Dimension labels
//!
//! No canvas in this codebase paints text (there is no
//! `paint_text`/text-shaping idiom to mirror — canvases here
//! only stroke paths), so the label is a pure string helper,
//! [`stage_dimensions_label`], for the caller to place as an
//! ordinary GPUI text element (e.g. in the HUD strip) rather
//! than something this canvas draws.

use gpui::*;

use crate::ui_kit::{C_BORDER, C_MUTED};

/// Safe-frame inset used by [`overlay_canvas`], as a
/// fraction of each stage dimension, applied on every side
/// (0.05 = the guide sits 5% in from each edge).
pub const SAFE_MARGIN_FRAC: f32 = 0.05;

/// Half-length of the centre cross's arms, as a fraction of
/// the corresponding stage dimension (the full arm spans
/// twice this, centred on the stage centre).
pub const CROSS_ARM_FRAC: f32 = 0.05;

/// The safe-frame guide rectangle for a `stage_w` ×
/// `stage_h` stage, inset by `margin_frac` of each dimension
/// on every side. Returns `(x, y, w, h)` in stage units.
///
/// Bad inputs are clamped sane rather than propagated:
/// negative or non-finite stage dimensions become 0, the
/// margin is clamped to `0.0..=0.5` (non-finite becomes 0),
/// and the returned width/height never go negative — at a
/// margin of 0.5 the rect degenerates to the zero-size rect
/// at the stage centre.
pub fn safe_frame_rect(
    stage_w: f32,
    stage_h: f32,
    margin_frac: f32,
) -> (f32, f32, f32, f32) {
    let w = if stage_w.is_finite() {
        stage_w.max(0.0)
    } else {
        0.0
    };
    let h = if stage_h.is_finite() {
        stage_h.max(0.0)
    } else {
        0.0
    };
    let f = if margin_frac.is_finite() {
        margin_frac.clamp(0.0, 0.5)
    } else {
        0.0
    };
    let x = w * f;
    let y = h * f;
    (x, y, (w - 2.0 * x).max(0.0), (h - 2.0 * y).max(0.0))
}

/// Display label for the stage dimensions, e.g. `"960 x 720"`
/// — the same `as i32` formatting the stage-size status line
/// in `app.rs` uses. Non-finite dimensions read as 0. Pure
/// string: the caller renders it as a text element (see the
/// module docs for why the canvas itself draws no text).
pub fn stage_dimensions_label(stage_w: f32, stage_h: f32) -> String {
    let w = if stage_w.is_finite() { stage_w } else { 0.0 };
    let h = if stage_h.is_finite() { stage_h } else { 0.0 };
    format!("{} x {}", w as i32, h as i32)
}

/// A full-size canvas painting the viewport guides for a
/// `stage_w` × `stage_h` stage: the safe-frame outline
/// (inset [`SAFE_MARGIN_FRAC`], in `C_BORDER`) when
/// `show_safe`, and a centre cross (in `C_MUTED`) when
/// `show_cross`. The canvas fills its parent (`w_full` /
/// `h_full`); the caller stacks it over the stage viewport.
///
/// Stage points are mapped with `paint_stage`'s transform
/// under the default camera (centre = stage centre,
/// zoom = 1.0) — see the module docs for the exact maths
/// and the camera caveat.
pub fn overlay_canvas(
    stage_w: f32,
    stage_h: f32,
    show_safe: bool,
    show_cross: bool,
) -> impl IntoElement {
    canvas(
        |_bounds, _window, _cx| (),
        move |bounds, _state, window, _cx| {
            paint_overlay(window, bounds, stage_w, stage_h, show_safe, show_cross);
        },
    )
    .w_full()
    .h_full()
}

/// Paint the guides into `bounds`, following `paint_stage`'s
/// mapping (`PathBuilder` + `window.paint_path`, strokes
/// built in stage space and mapped point by point).
fn paint_overlay(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    stage_w: f32,
    stage_h: f32,
    show_safe: bool,
    show_cross: bool,
) {
    if !show_safe && !show_cross {
        return;
    }
    let bw = f32::from(bounds.size.width);
    let bh = f32::from(bounds.size.height);
    if bw <= 0.0 || bh <= 0.0 || stage_w <= 0.0 || stage_h <= 0.0 {
        return;
    }
    let origin = bounds.origin;
    // paint_stage's transform with the default camera:
    // cam centre = stage centre, zoom = 1.0, so s = fit.
    let fit = (bw / stage_w).min(bh / stage_h);
    let s = fit;
    let cxw = bw / 2.0;
    let cyw = bh / 2.0;
    let (cam_x, cam_y) = (stage_w / 2.0, stage_h / 2.0);
    let map = |x: f32, y: f32| -> Point<Pixels> {
        point(
            origin.x + px(cxw + (x - cam_x) * s),
            origin.y + px(cyw + (y - cam_y) * s),
        )
    };

    if show_safe {
        let (x, y, w, h) = safe_frame_rect(stage_w, stage_h, SAFE_MARGIN_FRAC);
        let mut pb = PathBuilder::stroke(px(1.0));
        pb.move_to(map(x, y));
        pb.line_to(map(x + w, y));
        pb.line_to(map(x + w, y + h));
        pb.line_to(map(x, y + h));
        pb.close();
        if let Ok(path) = pb.build() {
            window.paint_path(path, rgb(C_BORDER));
        }
    }

    if show_cross {
        let (cx, cy) = (stage_w / 2.0, stage_h / 2.0);
        let (ax, ay) = (stage_w * CROSS_ARM_FRAC, stage_h * CROSS_ARM_FRAC);
        let mut pb = PathBuilder::stroke(px(1.0));
        pb.move_to(map(cx - ax, cy));
        pb.line_to(map(cx + ax, cy));
        pb.move_to(map(cx, cy - ay));
        pb.line_to(map(cx, cy + ay));
        if let Ok(path) = pb.build() {
            window.paint_path(path, rgb(C_MUTED));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{safe_frame_rect, stage_dimensions_label};

    #[test]
    fn safe_frame_rect_insets_each_side_by_the_margin() {
        // Dyadic values, exact in binary floating point:
        // 25% of 800 = 200, 25% of 400 = 100.
        let (x, y, w, h) = safe_frame_rect(800.0, 400.0, 0.25);
        assert_eq!((x, y, w, h), (200.0, 100.0, 400.0, 200.0));
    }

    #[test]
    fn safe_frame_rect_zero_margin_is_the_full_stage() {
        let (x, y, w, h) = safe_frame_rect(960.0, 720.0, 0.0);
        assert_eq!((x, y, w, h), (0.0, 0.0, 960.0, 720.0));
    }

    #[test]
    fn safe_frame_rect_five_percent_matches_the_overlay_guide() {
        // The fraction overlay_canvas uses; 0.05 is not exact
        // in binary, so compare with a small tolerance.
        let (x, y, w, h) = safe_frame_rect(960.0, 720.0, 0.05);
        let close = |a: f32, b: f32| (a - b).abs() < 0.01;
        assert!(close(x, 48.0), "x = {x}");
        assert!(close(y, 36.0), "y = {y}");
        assert!(close(w, 864.0), "w = {w}");
        assert!(close(h, 648.0), "h = {h}");
    }

    #[test]
    fn safe_frame_rect_clamps_an_oversized_margin_to_half() {
        // Anything past 0.5 would invert the rect; it clamps
        // to the degenerate zero-size rect at the centre.
        let (x, y, w, h) = safe_frame_rect(800.0, 400.0, 0.9);
        assert_eq!((x, y, w, h), (400.0, 200.0, 0.0, 0.0));
        let half = safe_frame_rect(800.0, 400.0, 0.5);
        assert_eq!(half, (400.0, 200.0, 0.0, 0.0));
    }

    #[test]
    fn safe_frame_rect_bad_inputs_are_clamped_sane() {
        // Negative margin behaves as no margin.
        assert_eq!(
            safe_frame_rect(800.0, 400.0, -0.25),
            (0.0, 0.0, 800.0, 400.0)
        );
        // NaN margin behaves as no margin; NaN stage as empty.
        assert_eq!(
            safe_frame_rect(800.0, 400.0, f32::NAN),
            (0.0, 0.0, 800.0, 400.0)
        );
        assert_eq!(
            safe_frame_rect(f32::NAN, -720.0, 0.25),
            (0.0, 0.0, 0.0, 0.0)
        );
        // Negative stage dimensions never produce a
        // negative-size rect.
        let (_, _, w, h) = safe_frame_rect(-800.0, -400.0, 0.25);
        assert_eq!((w, h), (0.0, 0.0));
    }

    #[test]
    fn stage_dimensions_label_formats_like_the_status_line() {
        assert_eq!(stage_dimensions_label(960.0, 720.0), "960 x 720");
        // Fractional sizes truncate, matching `as i32`.
        assert_eq!(stage_dimensions_label(960.9, 719.2), "960 x 719");
        // Non-finite input reads as 0 rather than garbage.
        assert_eq!(stage_dimensions_label(f32::NAN, f32::INFINITY), "0 x 0");
    }
}
