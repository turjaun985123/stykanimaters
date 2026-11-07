//! Pure geometry for the drawing tools: point-in-polygon tests,
//! serpentine scanline fill and the continuous path built from it,
//! plus path smoothing / resampling, nib width profiles and the
//! stroke-splitting eraser. No editor state lives here; `editor.rs`
//! adapts these helpers to strokes. Everything is deterministic
//! and unit-tested below.

use crate::model::{Stroke, V2};

fn distance(a: V2, b: V2) -> f32 {
    ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt()
}

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

/// Smooth an open polyline with Chaikin corner cutting: every
/// segment `[a, b]` is replaced by the two points `0.75a + 0.25b`
/// and `0.25a + 0.75b`, pulling the path toward the segment
/// midpoints and rounding every corner. The first and last points
/// are pinned (never moved), unlike the closed-polygon variant.
/// `subdiv` is the number of iterations; 0 returns the input
/// unchanged. For `n >= 2` input points, iteration multiplies the
/// count by two each time, so the result has `n * 2^subdiv` points.
/// Empty and single-point inputs are returned as-is.
pub fn smooth_path(points: &[V2], subdiv: usize) -> Vec<V2> {
    if points.len() < 2 || subdiv == 0 {
        return points.to_vec();
    }
    let mut current: Vec<V2> = points.to_vec();
    for _ in 0..subdiv {
        let mut next: Vec<V2> = Vec::with_capacity(current.len() * 2);
        next.push(current[0]);
        for w in current.windows(2) {
            let (a, b) = (w[0], w[1]);
            next.push([
                a[0] * 0.75 + b[0] * 0.25,
                a[1] * 0.75 + b[1] * 0.25,
            ]);
            next.push([
                a[0] * 0.25 + b[0] * 0.75,
                a[1] * 0.25 + b[1] * 0.75,
            ]);
        }
        next.push(*current.last().unwrap());
        current = next;
    }
    current
}

/// Resample a polyline at even arc-length intervals: the result
/// starts at the first point, places samples every `spacing` units
/// of travelled distance (interpolated along segments), and always
/// ends at the last point — so the final step is shorter than
/// `spacing` unless the total length is an exact multiple of it.
/// Inputs with fewer than 2 points, and non-positive `spacing`,
/// return the input unchanged. Zero-length segments are skipped.
pub fn resample(points: &[V2], spacing: f32) -> Vec<V2> {
    if points.len() < 2 || spacing <= 0.0 {
        return points.to_vec();
    }
    let mut out: Vec<V2> = Vec::new();
    out.push(points[0]);
    // Arc length of the next sample, and of the current segment start.
    let mut next_mark = spacing;
    let mut covered = 0.0f32;
    for w in points.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = distance(a, b);
        if len < 1e-9 {
            continue;
        }
        while covered + len >= next_mark {
            let t = (next_mark - covered) / len;
            out.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
            next_mark += spacing;
        }
        covered += len;
    }
    let last = *points.last().unwrap();
    if out.last() != Some(&last) {
        out.push(last);
    }
    out
}

/// Pen nib shape for width profiling in [`nib_widths`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nib {
    /// Round nib: constant width in every direction.
    Round,
    /// Square nib: the nib footprint is a square, so the mark is
    /// widest when travelling diagonally across it.
    Square,
    /// Broad-edge calligraphy nib: the mark is thinnest when
    /// travelling along the nib edge, widest across it.
    Calligraphy,
}

/// Per-point stroke widths for a nib of size `base` held at
/// `nib_angle_deg` (the nib edge's orientation, degrees), given the
/// stroke's travel direction at each point. The direction at an
/// interior point is taken from its two neighbours; at the ends,
/// from the single adjacent segment. (A zero-length step has no
/// direction; it falls back to angle 0.) One width is returned per
/// input point. Formulas, with `psi` = travel direction minus nib
/// angle:
///
/// - `Round`: `base` everywhere.
/// - `Square`: the silhouette width of a square nib,
///   `base * (|cos psi| + |sin psi|) / sqrt(2)`. `base` is the
///   nib's *widest* mark (travelling at 45° to the edge); moving
///   along or across the edge gives `base / sqrt(2)` (≈ 0.71·base).
/// - `Calligraphy`: the projection of the broad edge onto the axis
///   across the travel direction, `base * |sin psi|`, clamped to a
///   minimum of `0.15 * base` so along-the-edge strokes stay a
///   visible hairline instead of vanishing.
///
/// Empty input gives an empty vector; a single point has no travel
/// direction and gets the plain `base` width.
pub fn nib_widths(points: &[V2], base: f32, nib: Nib, nib_angle_deg: f32) -> Vec<f32> {
    if points.is_empty() {
        return Vec::new();
    }
    if points.len() == 1 {
        return vec![base];
    }
    let nib_rad = nib_angle_deg.to_radians();
    let last = points.len() - 1;
    (0..points.len())
        .map(|i| {
            let (a, b) = if i == 0 {
                (points[0], points[1])
            } else if i == last {
                (points[last - 1], points[last])
            } else {
                (points[i - 1], points[i + 1])
            };
            let psi = (b[1] - a[1]).atan2(b[0] - a[0]) - nib_rad;
            match nib {
                Nib::Round => base,
                Nib::Square => {
                    base * (psi.cos().abs() + psi.sin().abs()) / std::f32::consts::SQRT_2
                }
                Nib::Calligraphy => base * psi.sin().abs().clamp(0.15, 1.0),
            }
        })
        .collect()
}

/// The eraser: remove every point within `radius` of `centre`
/// (distance `<= radius`) from every stroke, splitting a stroke
/// into several where the erasure cuts its middle. Each surviving
/// run of consecutive points becomes a stroke that inherits the
/// original's `frame`, `color` and `width`; per-point `widths` are
/// sliced in parallel when the original's were parallel to its
/// points (a non-parallel `widths` is dropped to empty, the
/// uniform-width form). Runs shorter than 2 points vanish, as do
/// strokes whose points are all erased. Strokes the eraser does
/// not touch are returned unchanged.
///
/// `fill` is cleared on every stroke the eraser modifies — split
/// or merely trimmed. The fill colour was generated for the
/// stroke's original closed outline; once that outline is cut,
/// the pieces no longer trace the filled shape, so carrying the
/// colour over would claim a fill that matches nothing.
pub fn erase_at(strokes: &[Stroke], centre: V2, radius: f32) -> Vec<Stroke> {
    let mut out: Vec<Stroke> = Vec::new();
    for stroke in strokes {
        if !stroke.points.iter().any(|&p| distance(p, centre) <= radius) {
            out.push(stroke.clone());
            continue;
        }
        let parallel = stroke.widths.len() == stroke.points.len();
        // Flush one maximal run of surviving point indices.
        let mut run: Vec<usize> = Vec::new();
        let mut flush = |run: &Vec<usize>, out: &mut Vec<Stroke>| {
            if run.len() >= 2 {
                out.push(Stroke {
                    frame: stroke.frame,
                    points: run.iter().map(|&i| stroke.points[i]).collect(),
                    color: stroke.color,
                    width: stroke.width,
                    widths: if parallel {
                        run.iter().map(|&i| stroke.widths[i]).collect()
                    } else {
                        Vec::new()
                    },
                    fill: None,
                });
            }
        };
        for (i, &p) in stroke.points.iter().enumerate() {
            if distance(p, centre) <= radius {
                flush(&run, &mut out);
                run.clear();
            } else {
                run.push(i);
            }
        }
        flush(&run, &mut out);
    }
    out
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

    // ---- smooth_path ----

    #[test]
    fn smooth_path_keeps_endpoints_and_multiplies_count() {
        let pts = vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]];
        let s1 = smooth_path(&pts, 1);
        // Count formula: n * 2^subdiv for n >= 2.
        assert_eq!(s1.len(), 6);
        assert_eq!(smooth_path(&pts, 2).len(), 12);
        assert_eq!(s1.first(), Some(&pts[0]), "start point pinned");
        assert_eq!(s1.last(), Some(&pts[2]), "end point pinned");
        // Zero iterations is the identity.
        assert_eq!(smooth_path(&pts, 0), pts);
        // The corner itself is cut away by the smoothing.
        assert!(!s1.contains(&[10.0, 0.0]), "corner point is cut: {s1:?}");
        assert_eq!(
            s1,
            vec![
                [0.0, 0.0],
                [2.5, 0.0],
                [7.5, 0.0],
                [10.0, 2.5],
                [10.0, 7.5],
                [10.0, 10.0],
            ]
        );
    }

    #[test]
    fn smooth_path_degenerate_inputs() {
        assert!(smooth_path(&[], 3).is_empty());
        assert_eq!(smooth_path(&[[1.0, 2.0]], 3), vec![[1.0, 2.0]]);
        // Two points: subdiv 1 gives the pinned ends plus Q and R.
        assert_eq!(
            smooth_path(&[[0.0, 0.0], [10.0, 0.0]], 1),
            vec![[0.0, 0.0], [2.5, 0.0], [7.5, 0.0], [10.0, 0.0]]
        );
    }

    // ---- resample ----

    #[test]
    fn resample_spaces_points_evenly() {
        // Total length 70 (30 across, 40 down), spacing 10 divides it.
        let pts = vec![[0.0, 0.0], [30.0, 0.0], [30.0, 40.0]];
        let out = resample(&pts, 10.0);
        assert_eq!(out.len(), 8, "samples at 0,10,...,70: {out:?}");
        assert_eq!(out.first(), Some(&pts[0]));
        assert_eq!(out.last(), Some(&pts[2]), "last point preserved");
        for w in out.windows(2) {
            let d = distance(w[0], w[1]);
            assert!((d - 10.0).abs() < 0.05, "step of {d} between {:?} and {:?}", w[0], w[1]);
        }
        // The corner is one of the samples (30 is a multiple of 10).
        assert!(out.contains(&[30.0, 0.0]));
    }

    #[test]
    fn resample_keeps_short_remainder_before_last_point() {
        let pts = vec![[0.0, 0.0], [25.0, 0.0]];
        let out = resample(&pts, 10.0);
        assert_eq!(out.len(), 4);
        assert_eq!(out[1], [10.0, 0.0]);
        assert_eq!(out[2], [20.0, 0.0]);
        assert_eq!(out[3], [25.0, 0.0], "last point preserved after a 5-unit remainder");
    }

    #[test]
    fn resample_degenerate_inputs() {
        assert!(resample(&[], 10.0).is_empty());
        assert_eq!(resample(&[[3.0, 4.0]], 10.0), vec![[3.0, 4.0]]);
        let pts = vec![[0.0, 0.0], [10.0, 0.0]];
        assert_eq!(resample(&pts, 0.0), pts, "zero spacing is identity");
        assert_eq!(resample(&pts, -1.0), pts, "negative spacing is identity");
        // Spacing larger than the whole path: just the endpoints.
        assert_eq!(resample(&pts, 50.0), pts);
    }

    // ---- nib_widths ----

    fn assert_widths(actual: &[f32], expected: f32) {
        assert!(!actual.is_empty());
        for &w in actual {
            assert!((w - expected).abs() < 1e-3, "width {w} != {expected} in {actual:?}");
        }
    }

    #[test]
    fn nib_widths_key_values() {
        let base = 6.0f32;
        let horiz = vec![[0.0, 0.0], [10.0, 0.0], [20.0, 0.0]];
        let vert = vec![[0.0, 0.0], [0.0, 10.0], [0.0, 20.0]];
        let diag = vec![[0.0, 0.0], [10.0, 10.0], [20.0, 20.0]];
        let s2 = std::f32::consts::SQRT_2;

        // Round: base in every direction.
        assert_widths(&nib_widths(&horiz, base, Nib::Round, 0.0), base);
        assert_widths(&nib_widths(&diag, base, Nib::Round, 35.0), base);

        // Calligraphy, edge at 0°: horizontal travel runs along the
        // edge (hairline clamp 0.15), vertical runs across it (full).
        assert_widths(&nib_widths(&horiz, base, Nib::Calligraphy, 0.0), 0.15 * base);
        assert_widths(&nib_widths(&vert, base, Nib::Calligraphy, 0.0), base);
        assert_widths(&nib_widths(&diag, base, Nib::Calligraphy, 0.0), base / s2);
        // Rotating the nib by 90° swaps the roles.
        assert_widths(&nib_widths(&horiz, base, Nib::Calligraphy, 90.0), base);
        assert_widths(&nib_widths(&vert, base, Nib::Calligraphy, 90.0), 0.15 * base);

        // Square, edge at 0°: along/across the edge gives base/√2,
        // the 45° diagonal gives the full base.
        assert_widths(&nib_widths(&horiz, base, Nib::Square, 0.0), base / s2);
        assert_widths(&nib_widths(&vert, base, Nib::Square, 0.0), base / s2);
        assert_widths(&nib_widths(&diag, base, Nib::Square, 0.0), base);
    }

    #[test]
    fn nib_widths_degenerate_inputs() {
        assert!(nib_widths(&[], 6.0, Nib::Square, 0.0).is_empty());
        assert_eq!(nib_widths(&[[1.0, 1.0]], 6.0, Nib::Calligraphy, 0.0), vec![6.0]);
        // One width per point, always.
        let pts = vec![[0.0, 0.0], [5.0, 3.0], [9.0, 3.0], [9.0, 9.0]];
        assert_eq!(nib_widths(&pts, 4.0, Nib::Square, 20.0).len(), pts.len());
    }

    // ---- erase_at ----

    /// Points along the x axis at 0, 10, ..., 100, with parallel
    /// widths 4, 5, ..., 14 and a fill colour set.
    fn axis_stroke() -> Stroke {
        Stroke {
            frame: 3,
            points: (0..=10).map(|i| [i as f32 * 10.0, 0.0]).collect(),
            color: 0xFF0000,
            width: 4.0,
            widths: (0..=10).map(|i| 4.0 + i as f32).collect(),
            fill: Some(0x00FF00),
        }
    }

    #[test]
    fn erase_at_splits_a_stroke_in_the_middle() {
        let out = erase_at(&[axis_stroke()], [50.0, 0.0], 15.0);
        // Points at x = 40, 50, 60 are within 15 of the centre.
        assert_eq!(out.len(), 2, "one cut, two pieces: {out:?}");
        assert_eq!(
            out[0].points,
            vec![[0.0, 0.0], [10.0, 0.0], [20.0, 0.0], [30.0, 0.0]]
        );
        assert_eq!(
            out[1].points,
            vec![[70.0, 0.0], [80.0, 0.0], [90.0, 0.0], [100.0, 0.0]]
        );
        for piece in &out {
            assert_eq!(piece.frame, 3);
            assert_eq!(piece.color, 0xFF0000);
            assert_eq!(piece.width, 4.0);
            assert_eq!(piece.fill, None, "fill drops on a cut stroke");
            assert_eq!(piece.widths.len(), piece.points.len(), "widths stay parallel");
        }
        assert_eq!(out[0].widths, vec![4.0, 5.0, 6.0, 7.0]);
        assert_eq!(out[1].widths, vec![11.0, 12.0, 13.0, 14.0]);
    }

    #[test]
    fn erase_at_trims_an_endpoint() {
        let out = erase_at(&[axis_stroke()], [0.0, 0.0], 15.0);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].points.len(), 9, "x = 0 and 10 erased");
        assert_eq!(out[0].points[0], [20.0, 0.0]);
        assert_eq!(out[0].widths.len(), 9);
        assert_eq!(out[0].fill, None, "fill drops on a trimmed stroke too");
    }

    #[test]
    fn erase_at_missing_everything_is_identity() {
        let strokes = vec![
            axis_stroke(),
            Stroke {
                frame: 0,
                points: vec![[500.0, 500.0]],
                color: 0x123456,
                width: 2.0,
                widths: Vec::new(),
                fill: Some(0xABCDEF),
            },
        ];
        let out = erase_at(&strokes, [1000.0, 1000.0], 10.0);
        assert_eq!(out.len(), 2);
        for (a, b) in out.iter().zip(strokes.iter()) {
            assert_eq!(a.frame, b.frame);
            assert_eq!(a.points, b.points);
            assert_eq!(a.color, b.color);
            assert_eq!(a.width, b.width);
            assert_eq!(a.widths, b.widths);
            assert_eq!(a.fill, b.fill, "untouched strokes keep their fill");
        }
    }

    #[test]
    fn erase_at_removes_strokes_and_singleton_runs() {
        // Everything inside the radius: the stroke is gone.
        let out = erase_at(&[axis_stroke()], [50.0, 0.0], 500.0);
        assert!(out.is_empty());
        // Cutting only the middle point of a 3-point stroke leaves
        // two singletons, which vanish.
        let short = Stroke {
            frame: 0,
            points: vec![[0.0, 0.0], [10.0, 0.0], [20.0, 0.0]],
            color: 0,
            width: 3.0,
            widths: Vec::new(),
            fill: None,
        };
        assert!(erase_at(&[short], [10.0, 0.0], 5.0).is_empty());
        // No strokes in, no strokes out.
        assert!(erase_at(&[], [0.0, 0.0], 10.0).is_empty());
    }

    #[test]
    fn erase_at_uniform_widths_stay_uniform() {
        // A stroke with empty (uniform) widths keeps them empty
        // after a split — no fabricated per-point data.
        let mut s = axis_stroke();
        s.widths = Vec::new();
        let out = erase_at(&[s], [50.0, 0.0], 15.0);
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|p| p.widths.is_empty()));
    }
}
