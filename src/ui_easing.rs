//! Easing-picker building blocks: a tiny canvas that draws one
//! easing curve, and display names for the variants.
//!
//! Presentation only — no state, no handlers, no editor
//! knowledge (the same contract as [`crate::ui_kit`]). The
//! panel in `app.rs` pairs each curve card with an Apply
//! button whose listener calls `EditorCore::set_frame_easing`.
//!
//! The curve is sampled from the engine's OWN easing
//! evaluation, [`Easing::apply`] — the exact function
//! `Project::poses_at` uses to tween poses — so what a card
//! draws is what playback does. Nothing is re-implemented
//! here.

use gpui::*;

use crate::model::Easing;
use crate::ui_kit::{C_ACCENT, C_BORDER};

/// Display name for a picker card ("Ease In" — title case,
/// unlike the kebab-case `Easing::label` the status line
/// uses).
pub fn easing_name(e: Easing) -> &'static str {
    match e {
        Easing::Linear => "Linear",
        Easing::Smooth => "Smooth",
        Easing::EaseIn => "Ease In",
        Easing::EaseOut => "Ease Out",
    }
}

/// A 64×40 canvas drawing `easing`'s curve: time runs left to
/// right, eased progress bottom to top. Faint axes (left +
/// bottom edges of the plot area) in `C_BORDER`, the curve
/// itself in `C_ACCENT`.
pub fn curve_canvas(easing: Easing) -> impl IntoElement {
    canvas(
        |_bounds, _window, _cx| (),
        move |bounds, _state, window, _cx| {
            paint_curve(window, bounds, easing);
        },
    )
    .w(px(64.0))
    .h(px(40.0))
}

/// Paint one easing curve into `bounds`, following the graph
/// panel canvas's idiom (`PathBuilder` + `window.paint_path`).
fn paint_curve(window: &mut Window, bounds: Bounds<Pixels>, easing: Easing) {
    let origin = bounds.origin;
    let w = f32::from(bounds.size.width);
    let h = f32::from(bounds.size.height);
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    // Inset so the strokes never clip the canvas edge.
    let pad = 3.0;
    let (x0, x1) = (pad, w - pad);
    let (y_top, y_bot) = (pad, h - pad);
    let at = |x: f32, y: f32| point(origin.x + px(x), origin.y + px(y));
    // Axes: left and bottom edges of the plot area.
    let mut axes = PathBuilder::stroke(px(1.0));
    axes.move_to(at(x0, y_top));
    axes.line_to(at(x0, y_bot));
    axes.line_to(at(x1, y_bot));
    if let Ok(path) = axes.build() {
        window.paint_path(path, rgb(C_BORDER));
    }
    // The curve: 24 straight segments sampled from
    // `Easing::apply` (y flipped — screen y grows downward,
    // eased progress grows upward).
    let mut pb = PathBuilder::stroke(px(2.0));
    let steps = 24;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let v = easing.apply(t);
        let p = at(x0 + t * (x1 - x0), y_bot - v * (y_bot - y_top));
        if i == 0 {
            pb.move_to(p);
        } else {
            pb.line_to(p);
        }
    }
    if let Ok(path) = pb.build() {
        window.paint_path(path, rgb(C_ACCENT));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easing_name_covers_every_variant() {
        assert_eq!(easing_name(Easing::Linear), "Linear");
        assert_eq!(easing_name(Easing::Smooth), "Smooth");
        assert_eq!(easing_name(Easing::EaseIn), "Ease In");
        assert_eq!(easing_name(Easing::EaseOut), "Ease Out");
    }

    #[test]
    fn easing_names_are_distinct_and_non_empty() {
        let names = [
            easing_name(Easing::Linear),
            easing_name(Easing::Smooth),
            easing_name(Easing::EaseIn),
            easing_name(Easing::EaseOut),
        ];
        for (i, name) in names.iter().enumerate() {
            assert!(!name.trim().is_empty(), "name {i} is empty");
            assert!(
                !names[..i].contains(name),
                "duplicate easing name: {name}"
            );
        }
    }
}
