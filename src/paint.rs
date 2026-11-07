//! Pure geometry for the drawing tools: point-in-polygon tests,
//! serpentine scanline fill and the continuous path built from it.
//! No editor state lives here; `editor.rs` adapts these helpers to
//! strokes. Everything is deterministic and unit-tested below.

use crate::model::V2;

/// Even-odd ray-casting test: is `p` inside the (implicitly closed)
/// polygon `poly`? Polygons with fewer than 3 points contain nothing.
/// Points exactly on an edge are unspecified (either answer is fine
/// for lasso/fill purposes).
pub fn point_in_polygon(p: V2, poly: &[V2]) -> bool {
    if poly.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (xi, yi) = (poly[i][0], poly[i][1]);
        let (xj, yj) = (poly[j][0], poly[j][1]);
        // Does edge j->i straddle the horizontal line through p?
        // (Half-open in y so vertices are not double-counted.)
        if (yi > p[1]) != (yj > p[1]) {
            let x_at_y = xi + (p[1] - yi) * (xj - xi) / (yj - yi);
            if p[0] < x_at_y {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

/// Horizontal intersections of the scanline `y` with the polygon,
/// sorted by x. Interior spans are the consecutive pairs.
fn scanline_spans(poly: &[V2], y: f32) -> Vec<[f32; 2]> {
    let mut xs: Vec<f32> = Vec::new();
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let a = poly[j];
        let b = poly[i];
        // Half-open straddle test: each vertex counted once.
        if (a[1] <= y && b[1] > y) || (b[1] <= y && a[1] > y) {
            let t = (y - a[1]) / (b[1] - a[1]);
            xs.push(a[0] + t * (b[0] - a[0]));
        }
        j = i;
    }
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    xs.chunks_exact(2).map(|c| [c[0], c[1]]).collect()
}

/// Serpentine (boustrophedon) scanline segments clipped to `poly`:
/// rows are spaced `spacing` apart in y, and consecutive rows run in
/// opposite x directions, so joining the segments end-to-start (see
/// [`polyline_fill_path`]) draws a continuous back-and-forth fill.
/// Each returned segment is `[start, end]` (two points). Rows sit at
/// `min_y + spacing/2 + k*spacing` while inside the polygon's y range;
/// a polygon thinner than `spacing` still gets one mid-height row.
pub fn fill_segments(poly: &[V2], spacing: f32) -> Vec<[V2; 2]> {
    if poly.len() < 3 || spacing <= 0.0 {
        return Vec::new();
    }
    let min_y = poly.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
    let max_y = poly.iter().map(|p| p[1]).fold(f32::NEG_INFINITY, f32::max);
    if !(max_y > min_y) {
        return Vec::new();
    }
    let mut rows: Vec<f32> = Vec::new();
    let mut y = min_y + spacing * 0.5;
    while y < max_y {
        rows.push(y);
        y += spacing;
    }
    if rows.is_empty() {
        rows.push((min_y + max_y) * 0.5);
    }
    let mut segments: Vec<[V2; 2]> = Vec::new();
    for (row_idx, y) in rows.into_iter().enumerate() {
        let mut spans = scanline_spans(poly, y);
        if row_idx % 2 == 1 {
            // Serpentine: odd rows travel right-to-left.
            spans.reverse();
            for span in &mut spans {
                span.reverse();
            }
        }
        for [x0, x1] in spans {
            segments.push([[x0, y], [x1, y]]);
        }
    }
    segments
}

/// The fill segments joined into one continuous path, suitable to be
/// stored as a single stroke's `points`: each segment contributes its
/// start (skipped when it duplicates the previous point) and its end,
/// so the path walks the serpentine without lifting the pen.
pub fn polyline_fill_path(poly: &[V2], spacing: f32) -> Vec<V2> {
    let mut path: Vec<V2> = Vec::new();
    for [a, b] in fill_segments(poly, spacing) {
        if path.last() != Some(&a) {
            path.push(a);
        }
        path.push(b);
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Vec<V2> {
        vec![[0.0, 0.0], [100.0, 0.0], [100.0, 100.0], [0.0, 100.0]]
    }

    #[test]
    fn point_in_polygon_inside_and_outside() {
        let sq = square();
        assert!(point_in_polygon([50.0, 50.0], &sq));
        assert!(point_in_polygon([1.0, 99.0], &sq));
        assert!(!point_in_polygon([150.0, 50.0], &sq));
        assert!(!point_in_polygon([-5.0, 50.0], &sq));
        assert!(!point_in_polygon([50.0, 150.0], &sq));
        // Degenerate polygons contain nothing.
        assert!(!point_in_polygon([0.0, 0.0], &[]));
        assert!(!point_in_polygon([0.0, 0.0], &[[0.0, 0.0], [10.0, 10.0]]));
    }

    #[test]
    fn point_in_polygon_concave_notch_is_outside() {
        // U shape: the notch at the top centre is outside.
        let u = vec![
            [0.0, 0.0],
            [100.0, 0.0],
            [100.0, 100.0],
            [60.0, 100.0],
            [60.0, 40.0],
            [40.0, 40.0],
            [40.0, 100.0],
            [0.0, 100.0],
        ];
        assert!(point_in_polygon([20.0, 50.0], &u), "left arm is inside");
        assert!(point_in_polygon([80.0, 50.0], &u), "right arm is inside");
        assert!(!point_in_polygon([50.0, 80.0], &u), "notch is outside");
        assert!(point_in_polygon([50.0, 20.0], &u), "base is inside");
    }

    #[test]
    fn fill_segments_stay_within_bounds_and_inside() {
        let sq = square();
        let segs = fill_segments(&sq, 10.0);
        assert_eq!(segs.len(), 10, "one span per 10px row in a 100px square");
        for [a, b] in &segs {
            for p in [a, b] {
                assert!(p[0] >= -0.01 && p[0] <= 100.01, "x in bounds: {p:?}");
                assert!(p[1] >= -0.01 && p[1] <= 100.01, "y in bounds: {p:?}");
            }
            let mid = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
            assert!(point_in_polygon(mid, &sq), "segment midpoint inside: {mid:?}");
            assert!((a[1] - b[1]).abs() < f32::EPSILON, "segments are horizontal");
        }
    }

    #[test]
    fn fill_segments_triangle_midpoints_inside() {
        let tri = vec![[0.0, 0.0], [100.0, 20.0], [20.0, 100.0]];
        let segs = fill_segments(&tri, 7.0);
        assert!(!segs.is_empty());
        for [a, b] in &segs {
            let mid = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
            assert!(point_in_polygon(mid, &tri), "midpoint inside triangle: {mid:?}");
        }
    }

    #[test]
    fn fill_segments_degenerate_inputs_are_empty() {
        assert!(fill_segments(&[], 10.0).is_empty());
        assert!(fill_segments(&[[0.0, 0.0], [10.0, 10.0]], 10.0).is_empty());
        assert!(fill_segments(&square(), 0.0).is_empty());
        assert!(fill_segments(&square(), -5.0).is_empty());
        // Flat (zero-height) polygon: no fill.
        let flat = vec![[0.0, 5.0], [100.0, 5.0], [50.0, 5.0]];
        assert!(fill_segments(&flat, 10.0).is_empty());
    }

    #[test]
    fn fill_segments_serpentine_alternates_direction() {
        let sq = square();
        let segs = fill_segments(&sq, 25.0);
        assert_eq!(segs.len(), 4);
        // Row 0 runs left-to-right, row 1 right-to-left, and so on.
        assert!(segs[0][0][0] < segs[0][1][0], "row 0: {:?}", segs[0]);
        assert!(segs[1][0][0] > segs[1][1][0], "row 1: {:?}", segs[1]);
        assert!(segs[2][0][0] < segs[2][1][0], "row 2: {:?}", segs[2]);
        assert!(segs[3][0][0] > segs[3][1][0], "row 3: {:?}", segs[3]);
    }

    #[test]
    fn polyline_fill_path_is_continuous_serpentine() {
        let sq = square();
        let path = polyline_fill_path(&sq, 10.0);
        let segs = fill_segments(&sq, 10.0);
        assert_eq!(path.len(), segs.len() * 2);
        // Continuity: every step is either a full row (100 wide) or a
        // one-row hop (spacing 10) — never a teleport across the shape.
        for w in path.windows(2) {
            let d = ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt();
            let is_row = (d - 100.0).abs() < 0.01;
            let is_hop = (d - 10.0).abs() < 0.01;
            assert!(is_row || is_hop, "unexpected jump of {d} between {:?} and {:?}", w[0], w[1]);
        }
        // The path covers the shape: it reaches both x extremes and
        // its lowest and highest rows bracket the middle.
        let min_x = path.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
        let max_x = path.iter().map(|p| p[0]).fold(f32::NEG_INFINITY, f32::max);
        assert!(min_x.abs() < 0.01 && (max_x - 100.0).abs() < 0.01);
    }

    #[test]
    fn thin_polygon_still_gets_a_fill_row() {
        // Height 3 < spacing 10: the mid-height fallback row fills it.
        let thin = vec![[0.0, 0.0], [80.0, 0.0], [80.0, 3.0], [0.0, 3.0]];
        let segs = fill_segments(&thin, 10.0);
        assert_eq!(segs.len(), 1);
        assert!((segs[0][0][1] - 1.5).abs() < 0.01);
        assert!(!polyline_fill_path(&thin, 10.0).is_empty());
    }
}
