//! Icon builder — Font Awesome glyphs as Rune shapes.
//!
//! The rendering half of the shell's icon system. The
//! curated path data lives in [`super::icon_data`] (102
//! Font Awesome Free solid icons, view-box height always
//! 512, width varies per icon); the SVG path parser lives
//! in the lib (`stykanimate_rs::icon_path`). This module
//! joins them: name → path data → parsed segments → a
//! Rune [`Path`] → a filled [`Shape`] the stage/canvas
//! machinery can draw, plus an [`El`] wrapper that sizes
//! the canvas by the icon's real aspect ratio.
//!
//! ## Cost
//!
//! Parsing an icon's path data is the expensive step, so
//! parsed segment lists are cached in a thread-local map
//! keyed by the path-data string itself (two icons never
//! share path data; if they did, sharing the parse would
//! still be correct). The Rune [`Path`] is rebuilt on
//! every call — its builders flatten curves as they go,
//! so a `Path` is a plain point list, cheap next to a
//! parse — and [`Shape::fill_path`] borrows it, so
//! nothing here clones geometry per frame beyond that.
//!
//! ## Fill rule — NOT pixel-verified
//!
//! Several curated icons are compound paths whose glyph
//! has holes (a ring with a counter, letterforms). How
//! Rune's `fill_path` treats nested subpaths — whether
//! the hole stays hollow — has never been verified on
//! screen: there is no screenshot proof yet. The set was
//! chosen as solid-style icons so most glyphs read either
//! way, but treat hole rendering as unproven until the
//! first screenshot lands.
//!
//! ## Unknown names
//!
//! Nothing here panics on a bad name: [`icon_shape`]
//! returns `None`, [`has_icon`] returns `false`, and
//! [`icon_el`] returns an empty fixed-size spacer
//! (`size` × `size`, no canvas) so a typo degrades to a
//! blank slot in the layout instead of a crash.

use std::cell::RefCell;
use std::collections::HashMap;

use rune::{canvas, row, Color, El, Path, Shape};
use stykanimate_rs::icon_path::{parse_path, Seg};

use super::icon_data::icon_data;

/// The view-box height every icon in the curated set
/// shares (the width is per icon).
const VIEW_HEIGHT: f32 = 512.0;

thread_local! {
    /// Parsed segment lists, keyed by the path-data
    /// string (a `&'static str` out of the data table).
    static SEG_CACHE: RefCell<HashMap<&'static str, Vec<Seg>>> =
        RefCell::new(HashMap::new());
}

/// Convert a `0xRRGGBB` colour to a Rune colour — the
/// same conversion `stage_view::hex_color` performs.
fn icon_color(rgb: u32) -> Color {
    rune::color::hex(&format!("#{:06x}", rgb & 0xFF_FFFF)).unwrap_or(Color::WHITE)
}

/// Build the Rune path for one parsed segment list.
/// The builders consume and return the path, flattening
/// curves against the running current point — exactly
/// the order the segments arrive in.
fn path_from_segs(segs: &[Seg]) -> Path {
    let mut path = Path::new();
    for seg in segs {
        path = match *seg {
            Seg::MoveTo(p) => path.move_to(p),
            Seg::LineTo(p) => path.line_to(p),
            Seg::QuadTo(c, p) => path.quad_to(c, p),
            Seg::CubicTo(c1, c2, p) => path.cubic_to(c1, c2, p),
            Seg::Close => path.close(),
        };
    }
    path
}

/// The Rune path for one raw path-data string, through
/// the parse cache: a cache hit never re-parses.
fn path_for(d: &'static str) -> Path {
    SEG_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let segs = cache.entry(d).or_insert_with(|| parse_path(d));
        path_from_segs(segs)
    })
}

/// Does the curated set contain an icon called `name`?
pub fn has_icon(name: &str) -> bool {
    icon_data(name).is_some()
}

/// The filled shape for icon `name` in `color`, drawn
/// in the icon's own view box (per-icon width × 512) so
/// the caller's canvas sizing decides the on-screen
/// size. `None` for an unknown name.
pub fn icon_shape(name: &str, color: Color) -> Option<Shape> {
    let (width, d) = icon_data(name)?;
    let path = path_for(d);
    Some(
        Shape::new()
            .fill_path(&path, color)
            .view_box(width, VIEW_HEIGHT),
    )
}

/// The filled shape for icon `name` in a `0xRRGGBB`
/// colour — the integer-colour twin of [`icon_shape`]
/// for callers holding raw hex (particle events, toast
/// severities).
pub fn icon_shape_rgb(name: &str, rgb: u32) -> Option<Shape> {
    icon_shape(name, icon_color(rgb))
}

/// The on-screen width of icon `name` rendered at
/// height `size`: the view box scales uniformly, so the
/// width follows the icon's aspect ratio. `None` for an
/// unknown name. Exposed for the aspect maths (and its
/// tests); [`icon_el`] is built on it.
pub fn icon_width(name: &str, size: f32) -> Option<f32> {
    let (width, _) = icon_data(name)?;
    Some(size * width / VIEW_HEIGHT)
}

/// A canvas element drawing icon `name` at height
/// `size` (width from the icon's aspect ratio — see
/// [`icon_width`]). For an unknown name, an empty
/// `size` × `size` spacer element: the layout keeps its
/// slot, nothing draws, nothing panics.
pub fn icon_el(name: &str, size: f32, color: Color) -> El {
    match (icon_shape(name, color), icon_width(name, size)) {
        (Some(shape), Some(width)) => canvas(shape).w(width).h(size),
        _ => row().w(size).h(size),
    }
}

/// [`icon_el`] with a `0xRRGGBB` colour, for callers
/// holding raw hex.
pub fn icon_el_rgb(name: &str, size: f32, rgb: u32) -> El {
    icon_el(name, size, icon_color(rgb))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rune_sec::icon_data::ICON_NAMES;
    use stykanimate_rs::icon_path::path_bbox;

    /// The whole curated set survives the pipeline:
    /// every name resolves, parses to at least one
    /// segment, and stays inside its own view box (with
    /// a point of slack for rounding at the edges).
    #[test]
    fn every_curated_icon_parses_inside_its_view_box() {
        assert_eq!(ICON_NAMES.len(), 102, "the curated set is 102 icons");
        for name in ICON_NAMES {
            let (width, d) = icon_data(name)
                .unwrap_or_else(|| panic!("{name} is listed but has no data"));
            let segs = parse_path(d);
            assert!(!segs.is_empty(), "{name} parsed to no segments");
            let bb = path_bbox(&segs).expect("a non-empty parse has a bbox");
            assert!(
                bb[0] >= -1.0 && bb[1] >= -1.0 && bb[2] <= width + 1.0 && bb[3] <= 513.0,
                "{name} escapes its view box: bbox {bb:?}, width {width}"
            );
        }
    }

    #[test]
    fn icon_shape_known_and_unknown_names() {
        assert!(icon_shape("play", Color::WHITE).is_some());
        assert!(icon_shape("no-such-icon", Color::WHITE).is_none());
        assert!(icon_shape_rgb("play", 0xFFFFFF).is_some());
        assert!(has_icon("play"));
        assert!(!has_icon("no-such-icon"));
    }

    /// Aspect: at the same height, a 448-wide icon's
    /// element is narrower than a 640-wide one's, by
    /// exactly the view-box ratio.
    #[test]
    fn icon_width_follows_the_view_box_aspect() {
        // "check" is 448 wide, "bezier-curve" is 640.
        let narrow = icon_width("check", 16.0).expect("check is in the set");
        let wide = icon_width("bezier-curve", 16.0).expect("bezier-curve is in the set");
        assert_eq!(narrow, 16.0 * 448.0 / 512.0);
        assert_eq!(wide, 16.0 * 640.0 / 512.0);
        assert!(narrow < wide, "448-wide icon must be narrower: {narrow} !< {wide}");
        assert_eq!(icon_width("no-such-icon", 16.0), None);
    }

    /// The element builder never panics, known or not
    /// (its internals are opaque; the contract is that
    /// the call returns an element either way).
    #[test]
    fn icon_el_builds_for_known_and_unknown_names() {
        let _known = icon_el("play", 14.0, icon_color(0x6D8CFF));
        let _unknown = icon_el("no-such-icon", 14.0, icon_color(0x6D8CFF));
        let _rgb = icon_el_rgb("play", 14.0, 0x6D8CFF);
    }

    /// The parse cache serves the same icon twice
    /// (second call is a hit) with identical geometry.
    #[test]
    fn repeated_shapes_match() {
        let a = icon_shape("bolt", icon_color(0xFFCC45)).expect("bolt is in the set");
        let b = icon_shape("bolt", icon_color(0xFFCC45)).expect("bolt is in the set");
        assert_eq!(a, b, "cached parse must rebuild the identical shape");
    }
}
