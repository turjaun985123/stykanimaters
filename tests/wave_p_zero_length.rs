//! Wave P (P18) — zero-length parity pins: the SETTLED cell.
//!
//! The full audit (raster exporter / SVG exporter / Rune
//! stage × zero-length shapes and segments × cap / taper /
//! gradient) lives in ~/workspace/wave-p/findings/p18.md.
//! Most cells DISAGREE across the three surfaces on this
//! tree, so per board contract 4 (parity rule) they are
//! reported, not changed and not pinned — a pin would
//! entrench one surface's accident as the contract.
//!
//! Exactly one cell already agrees on all three surfaces,
//! so it is universal today and pinned here:
//!
//!   A zero-length SEGMENT in the plain style (the default
//!   `NodeStyle`: round cap, taper 1.0, no `color2`) paints
//!   a DISC of radius thickness/2 centred on the joint, in
//!   the segment's near colour.
//!
//!   - Raster (this file): `export::styled_seg_color` has
//!     an explicit coincident-endpoints branch — covered
//!     iff `dist(p, a) <= half_near`. Proven below at the
//!     pixel level.
//!   - SVG (this file): the writer emits a `<line>` with
//!     coincident endpoints and `stroke-linecap="round"`;
//!     per the SVG spec a zero-length subpath with a round
//!     cap paints exactly that disc. Proven below at the
//!     emitted-markup level (the disc itself is renderer
//!     behaviour the spec mandates).
//!   - Stage (src/stage_view.rs tests): the shared piece
//!     generator yields one coincident piece at full
//!     thickness, and the stage strokes every piece as a
//!     round-cap line — a zero-length capsule is the same
//!     disc. Pinned in stage_view.rs's test module
//!     (`zero_length_flat_segment_is_one_coincident_piece`,
//!     `stage_shape_draws_zero_length_segment_as_coincident_round_stroke`).
//!
//! The fixture is one figure on a 200x200 black stage: a
//! root at (100, 100) and one child with length 0, so the
//! child's joint coincides with the root's. The child is
//! 12 thick and red, so the settled disc has radius 6.
//! With the camera cleared, the default camera is the
//! stage centre at zoom 1 — stage units map 1:1 onto
//! output pixels in both exporters.

use std::path::PathBuf;

use stykanimate_rs::export::render_frame_at;
use stykanimate_rs::model::{
    FigureDef, FigureInstance, Frame, NodeDef, NodeStyle, Pose, Project, Shape,
};
use stykanimate_rs::svg::write_frame_svg;

const RED: image::Rgb<u8> = image::Rgb([255, 0, 0]);
const BLACK: image::Rgb<u8> = image::Rgb([0, 0, 0]);

fn node(name: &str, parent: Option<usize>, length: f32, thickness: f32) -> NodeDef {
    NodeDef {
        name: name.into(),
        parent,
        length,
        rest_angle: 0.0,
        head: false,
        bend: 1.0,
        thickness,
        color: Some(0xFF0000),
        ik_end: false,
        shape: Shape::Line,
        style: NodeStyle::default(),
        media: None,
    }
}

/// One figure whose only segment has length zero, rooted
/// at the stage centre (100, 100); everything else that
/// could paint is cleared.
fn zero_length_segment_project() -> Project {
    let mut project = Project::new_demo();
    project.fps = 24.0;
    project.stage = [200.0, 200.0];
    project.background = 0x000000;
    project.camera.clear();
    project.camera_follow = None;
    project.strokes.clear();
    project.particles.clear();
    project.weather = None;
    project.sprites.clear();
    project.texts.clear();
    project.figures = vec![FigureInstance {
        name: "zero".into(),
        color: 0xFF0000,
        def: FigureDef {
            nodes: vec![
                node("root", None, 0.0, 5.0),
                node("dot", Some(0), 0.0, 12.0),
            ],
        },
        hidden: false,
        clip: None,
        locked: false,
        opacity: 1.0,
    }];
    project.frames = vec![Frame {
        poses: vec![Pose {
            root: [100.0, 100.0],
            angles: vec![0.0, 0.0],
        }],
        duration: 1.0,
        easing: None,
        bends: Vec::new(),
    }];
    project
}

#[test]
fn raster_zero_length_plain_segment_paints_a_disc_of_half_thickness() {
    let project = zero_length_segment_project();
    let img = render_frame_at(&project, 0.0);
    assert_eq!(img.dimensions(), (200, 200));
    // Inside the disc (radius 6 around (100, 100)): the
    // centre and points 5 units out along both axes are
    // fully covered at every supersample, so they carry
    // the pure segment colour.
    for (x, y) in [(100, 100), (105, 100), (95, 100), (100, 95), (100, 105)] {
        assert_eq!(img.get_pixel(x, y), &RED, "pixel ({x}, {y}) is inside the disc");
    }
    // Outside: 7 units out along both axes no supersample
    // reaches the disc, so the background is untouched.
    for (x, y) in [(107, 100), (93, 100), (100, 93), (100, 107)] {
        assert_eq!(img.get_pixel(x, y), &BLACK, "pixel ({x}, {y}) is outside the disc");
    }
    // The disc is the ONLY paint: red over black can only
    // ever produce (r, 0, 0) pixels.
    assert!(
        img.pixels().all(|p| p.0[1] == 0 && p.0[2] == 0),
        "nothing but the red disc may paint"
    );
    // Coverage census: a radius-6 disc covers ~113 output
    // pixels including its anti-aliased fringe; the fully
    // covered core (pure red) is most of it. Loose bounds
    // on purpose — the arm assertions above carry the
    // exact radius; these catch a missing or doubled disc.
    let lit = img.pixels().filter(|p| p.0[0] > 0).count();
    let pure = img.pixels().filter(|p| p.0 == [255, 0, 0]).count();
    assert!((90..=150).contains(&lit), "disc coverage census off: {lit} lit pixels");
    assert!(pure >= 50, "disc core census off: {pure} pure-red pixels");
}

#[test]
fn svg_zero_length_plain_segment_is_a_round_capped_coincident_line() {
    let project = zero_length_segment_project();
    let dir: PathBuf = std::env::temp_dir().join(format!(
        "styk-wave-p-zero-length-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join("frame.svg");
    write_frame_svg(&project, 0, &path).expect("write frame svg");
    let text = std::fs::read_to_string(&path).expect("read frame svg");
    // The emitted form of the settled cell: coincident
    // endpoints, the full thickness as stroke-width, and
    // the round cap that makes a renderer paint the disc.
    assert!(
        text.contains(
            "<line x1=\"100.0\" y1=\"100.0\" x2=\"100.0\" y2=\"100.0\" \
             stroke=\"#FF0000\" stroke-width=\"12.0\" stroke-linecap=\"round\"/>"
        ),
        "zero-length plain segment must emit as a round-capped coincident line:\n{text}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
