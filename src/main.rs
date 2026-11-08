//! StykAnimate RS — Rune shell (Phase B of the GPUI→Rune migration).
//!
//! This binary proves the pinned Rune framework compiles against the
//! engine and opens a window. It holds a real [`EditorCore`] (demo
//! project) and draws the first figure's current pose on a Rune
//! canvas; the full editor UI arrives in Phase C.

use rune::tok::{self, type_};
use rune::{canvas, col, text, AppSpec, Cx, El, LineCap, Shape};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model;

struct State {
    core: EditorCore,
}

fn init() -> State {
    State {
        core: EditorCore::new(),
    }
}

/// Convert a `0xRRGGBB` engine colour to a Rune colour.
fn hex_color(rgb: u32) -> Option<rune::Color> {
    rune::color::hex(&format!("#{:06x}", rgb & 0xFF_FFFF))
}

/// The stage as one vector shape: every segment of the first figure
/// at the current frame, in stage coordinates (stretched to the
/// canvas element by the shape's view box).
fn stage_shape(core: &EditorCore) -> Shape {
    let project = &core.project;
    let mut shape = Shape::new().view_box(project.stage[0], project.stage[1]);
    let poses = project.poses_at(core.current as f32);
    if let (Some(fig), Some(pose)) = (project.figures.first(), poses.first()) {
        let color = hex_color(fig.color).unwrap_or(rune::Color::WHITE);
        let pts = model::evaluate(&fig.def, pose);
        for (i, node) in fig.def.nodes.iter().enumerate() {
            if let Some(p) = node.parent {
                shape = shape.line(
                    pts[p],
                    pts[i],
                    node.thickness.max(1.0),
                    LineCap::Round,
                    color,
                );
            }
        }
    }
    shape
}

fn view(s: &State, _: &mut Cx<State>) -> El {
    let t = tok::theme(true);
    let core = &s.core;
    let project = &core.project;
    let stage_fill = hex_color(project.background).unwrap_or(t.card);
    col()
        .w_full()
        .h_full()
        .fill(t.bg)
        .padding(16.0)
        .gap(12.0)
        .child(
            text(format!("{} — {} frames", project.name, core.frame_count()))
                .font(type_::TITLE)
                .color(t.text),
        )
        .child(
            canvas(stage_shape(core))
                .grow(1.0)
                .w_full()
                .fill(stage_fill)
                .radius(8.0),
        )
        .child(text("Rune shell — editor UI in Phase C").color(t.text_secondary))
}

fn main() {
    rune::run(AppSpec::<State> {
        min_size: Some((1280.0, 720.0)),
        ..AppSpec::new("StykAnimate RS", (1440.0, 900.0), init, view)
    });
}
