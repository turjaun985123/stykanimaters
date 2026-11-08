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

/// Stabilise a freehand stroke with one left-to-right pass:
/// hand jitter is dropped first, then each interior point is
/// pulled toward the midpoint of its neighbours. A point closer
/// than 0.5 units to the previous *kept* point counts as jitter
/// and is dropped — except the first and last points, which are
/// always kept exactly. Each remaining interior point `p` moves
/// to `p + (mid - p) * strength`, where `mid` is the midpoint of
/// the previous (already stabilised) point and the next kept
/// point, so the correction propagates along the stroke instead
/// of snapping every point independently. `strength` is clamped
/// to `0..=1`: 0.0 returns the input completely unchanged
/// (no dropping either), 1.0 puts every interior point exactly
/// on its neighbours' midpoint. Empty and single-point inputs
/// are returned as-is; NaN `strength` behaves as 0.0.
pub fn stabilise(points: &[V2], strength: f32) -> Vec<V2> {
    let s = strength.clamp(0.0, 1.0);
    if !(s > 0.0) || points.len() < 2 {
        return points.to_vec();
    }
    // De-jitter: keep a point only when it is at least 0.5 units
    // from the previously kept one; the endpoints are exempt.
    let mut kept: Vec<V2> = Vec::with_capacity(points.len());
    kept.push(points[0]);
    for &p in &points[1..points.len() - 1] {
        if distance(p, *kept.last().unwrap()) >= 0.5 {
            kept.push(p);
        }
    }
    kept.push(*points.last().unwrap());
    if kept.len() < 3 {
        return kept;
    }
    let mut out: Vec<V2> = Vec::with_capacity(kept.len());
    out.push(kept[0]);
    for i in 1..kept.len() - 1 {
        let prev = out[i - 1];
        let next = kept[i + 1];
        let mid = [(prev[0] + next[0]) * 0.5, (prev[1] + next[1]) * 0.5];
        let p = kept[i];
        out.push([p[0] + (mid[0] - p[0]) * s, p[1] + (mid[1] - p[1]) * s]);
    }
    out.push(*kept.last().unwrap());
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

/// A named brush preset: the base width a stroke is recorded
/// with, the nib that profiles its per-point widths when the
/// stroke is finished, and how many Chaikin smoothing iterations
/// are applied to the recorded path at that same moment. Wave 12
/// added `opacity` and `scatter` so low-opacity, spread marks
/// (the Spray preset) are expressible as data; the stroke
/// pipeline does not consume those two fields yet. Pure data —
/// the editor owns the behaviour (see `editor.rs`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Brush {
    pub name: &'static str,
    /// Base stroke width (the nib size for profiled brushes).
    pub width: f32,
    pub nib: Nib,
    /// Nib edge orientation in degrees (see [`nib_widths`]).
    pub nib_angle: f32,
    /// Chaikin iterations applied to the finished stroke;
    /// 0 = keep the path exactly as recorded.
    pub smoothing: usize,
    /// Stroke opacity, 0.0 (invisible) to 1.0 (fully opaque).
    /// Model data for now: `Stroke` carries no opacity yet, so
    /// the stroke pipeline does not apply this field — a
    /// follow-up wires it into rendering.
    pub opacity: f32,
    /// Scatter radius in stage units: 0.0 lands marks exactly
    /// on the recorded path; above 0.0, spray-style marks are
    /// spread within this radius of it (see [`spray_offsets`]).
    /// Like `opacity`, carried as model data until the stroke
    /// pipeline consumes it.
    pub scatter: f32,
}

/// The built-in brush library, in the order the editor's
/// brush cycle walks it. Index 0 is **Pen**, and it is the
/// editor's default for a reason: width 4.0, a round nib and no
/// smoothing reproduce exactly how strokes were recorded before
/// the library existed, so picking up any other brush is a
/// deliberate change, never a silent one.
pub fn brush_library() -> Vec<Brush> {
    vec![
        Brush {
            name: "Pen",
            width: 4.0,
            nib: Nib::Round,
            nib_angle: 0.0,
            smoothing: 0,
            opacity: 1.0,
            scatter: 0.0,
        },
        Brush {
            name: "Pencil",
            width: 2.0,
            nib: Nib::Round,
            nib_angle: 0.0,
            smoothing: 0,
            opacity: 1.0,
            scatter: 0.0,
        },
        Brush {
            name: "Marker",
            width: 8.0,
            nib: Nib::Round,
            nib_angle: 0.0,
            smoothing: 1,
            opacity: 1.0,
            scatter: 0.0,
        },
        Brush {
            name: "Calligraphy",
            width: 7.0,
            nib: Nib::Calligraphy,
            nib_angle: 30.0,
            smoothing: 1,
            opacity: 1.0,
            scatter: 0.0,
        },
        Brush {
            name: "Brush",
            width: 9.0,
            nib: Nib::Square,
            nib_angle: 0.0,
            smoothing: 1,
            opacity: 1.0,
            scatter: 0.0,
        },
        Brush {
            name: "Paint Brush",
            width: 12.0,
            nib: Nib::Square,
            nib_angle: 45.0,
            smoothing: 2,
            opacity: 1.0,
            scatter: 0.0,
        },
        // Wave 10 additions — appended only: the indices above
        // must never move, because a saved `brush_index` refers
        // into this list. At the time, everything a preset could
        // promise had to be expressible through width / nib /
        // nib_angle / smoothing alone: the model had no opacity
        // or scatter, so there was deliberately no "spray" or
        // low-opacity entry. Wave 12 (below) added those two
        // fields to `Brush`, which is what makes Spray
        // expressible as data.
        Brush {
            // The left-hand mirror of Calligraphy: a broader
            // broad-edge nib whose edge sits at 315° instead of
            // 30°, so the thick/thin directions swap diagonals —
            // thin travelling ↘/↖, thick travelling ↗/↙.
            name: "Calligraphy L",
            width: 10.0,
            nib: Nib::Calligraphy,
            nib_angle: 315.0,
            smoothing: 1,
            opacity: 1.0,
            scatter: 0.0,
        },
        Brush {
            // A chisel-tip marker: the square nib at 45° marks
            // full width horizontally and vertically and slims
            // to base/√2 on the diagonals (the opposite of the
            // 0° Brush), and no smoothing keeps the recorded
            // path — and its corners — exactly as drawn.
            name: "Chisel Marker",
            width: 5.0,
            nib: Nib::Square,
            nib_angle: 45.0,
            smoothing: 0,
            opacity: 1.0,
            scatter: 0.0,
        },
        Brush {
            // The widest mark in the library: a flat square nib
            // twice the Paint Brush's width, with the maximum
            // smoothing so broad washes follow a very even path.
            name: "Broad Brush",
            width: 20.0,
            nib: Nib::Square,
            nib_angle: 0.0,
            smoothing: 3,
            opacity: 1.0,
            scatter: 0.0,
        },
        // Wave 12 addition — appended only, index 9.
        Brush {
            // Spray: a wide round nib at half opacity whose
            // marks scatter up to 12 units around the recorded
            // path (placement: `spray_offsets`). The opacity and
            // scatter are model data today — the stroke pipeline
            // does not apply them yet, so until the follow-up
            // wires the renderer, this preset records like a
            // wide round pen and the fields describe the mark it
            // is meant to make.
            name: "Spray",
            width: 14.0,
            nib: Nib::Round,
            nib_angle: 0.0,
            smoothing: 0,
            opacity: 0.5,
            scatter: 12.0,
        },
    ]
}

/// Deterministic pseudo-random offsets inside a disc of
/// `radius`, for spray-style marks: `count` points, each at
/// distance `radius * sqrt(u)` in direction `2π * v`, where `u`
/// and `v` are successive outputs of a xorshift64 generator
/// seeded from `seed`. The square root makes the points uniform
/// over the disc's area instead of clustering at the centre.
/// The sequence depends only on `seed`, so a spray stroke
/// renders identically on every machine and in every export —
/// no external RNG, no clock, no thread state. `count` 0
/// returns an empty vector; `radius` 0 returns all-zero
/// offsets. A zero `seed` is remapped to a fixed nonzero
/// constant, because xorshift's zero state is a fixed point
/// that would otherwise emit the same offset forever.
pub fn spray_offsets(seed: u64, count: usize, radius: f32) -> Vec<[f32; 2]> {
    let mut state: u64 = if seed == 0 {
        0x9E37_79B9_7F4A_7C15
    } else {
        seed
    };
    let mut next_unit = || {
        // xorshift64 (Marsaglia shifts 13/7/17): full period
        // 2^64 - 1 over nonzero states.
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        // Top 24 bits scaled into [0, 1).
        ((state >> 40) as f32) / 16_777_216.0
    };
    (0..count)
        .map(|_| {
            let r = radius * next_unit().sqrt();
            let theta = next_unit() * std::f32::consts::TAU;
            [r * theta.cos(), r * theta.sin()]
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

/// The shapes the shape tool can draw from a drag. The outline is
/// generated as a plain polyline (see [`shape_points`]), so a
/// shape becomes an ordinary stroke once it is committed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShapeKind {
    Line,
    Rect,
    Ellipse,
}

/// Outline points for the shape `kind` dragged from corner `a` to
/// corner `b` (stage coordinates, y down). Rects and ellipses are
/// normalised to the axis-aligned box spanned by the two corners,
/// so the drag direction never changes the figure — only, for a
/// rect, which corner comes first:
///
/// - `Line`: exactly `[a, b]`.
/// - `Rect`: the box's four corners, closed (5 points, last ==
///   first), starting at `a`'s own corner and running clockwise
///   as seen on screen (y down).
/// - `Ellipse`: the ellipse inscribed in the box, 24 segments,
///   closed (25 points, last == first), starting at the rightmost
///   point and running clockwise as seen on screen.
///
/// Degenerate drags (`a == b`, zero width or zero height) never
/// panic and return finite points: lines and rects collapse onto
/// the shared corner(s), ellipses onto the centre line/point.
pub fn shape_points(kind: ShapeKind, a: [f32; 2], b: [f32; 2]) -> Vec<[f32; 2]> {
    match kind {
        ShapeKind::Line => vec![a, b],
        ShapeKind::Rect => {
            let (minx, maxx) = (a[0].min(b[0]), a[0].max(b[0]));
            let (miny, maxy) = (a[1].min(b[1]), a[1].max(b[1]));
            // Clockwise on screen (y down): top-left, top-right,
            // bottom-right, bottom-left.
            let cycle = [[minx, miny], [maxx, miny], [maxx, maxy], [minx, maxy]];
            // (a[0], a[1]) always appears verbatim among the four
            // corners (min/max pick the input values themselves),
            // so the drag's own corner can start the run.
            let start = cycle.iter().position(|&c| c == a).unwrap_or(0);
            let mut out: Vec<[f32; 2]> = (0..4).map(|i| cycle[(start + i) % 4]).collect();
            out.push(out[0]);
            out
        }
        ShapeKind::Ellipse => {
            const SEGMENTS: usize = 24;
            let cx = (a[0] + b[0]) * 0.5;
            let cy = (a[1] + b[1]) * 0.5;
            let rx = (a[0] - b[0]).abs() * 0.5;
            let ry = (a[1] - b[1]).abs() * 0.5;
            let mut out: Vec<[f32; 2]> = Vec::with_capacity(SEGMENTS + 1);
            for i in 0..SEGMENTS {
                // Angle 0 is the rightmost point; with y down an
                // increasing angle runs clockwise on screen.
                let t = (i as f32) * std::f32::consts::TAU / (SEGMENTS as f32);
                out.push([cx + rx * t.cos(), cy + ry * t.sin()]);
            }
            // Close with the first point verbatim, so the last
            // point equals it exactly (sin/cos of 2π would drift
            // by a rounding error instead).
            out.push(out[0]);
            out
        }
    }
}

/// Outline points for a regular polygon with `sides` sides,
/// centred on `center` with circumradius `radius` (stage
/// coordinates, y down). The first vertex sits at `rotation_deg`
/// (0° points right, along +x; positive angles run clockwise as
/// seen on screen), the remaining vertices follow clockwise, and
/// the outline is closed: `sides + 1` points with last == first,
/// the closing point a verbatim copy of the first — the same
/// conventions as [`shape_points`]' ellipse.
///
/// Degenerate case: `sides < 3` cannot form a polygon, so it
/// returns the two-point placeholder `[center, center]` instead
/// of panicking.
pub fn regular_polygon_points(
    center: [f32; 2],
    radius: f32,
    sides: usize,
    rotation_deg: f32,
) -> Vec<[f32; 2]> {
    if sides < 3 {
        return vec![center, center];
    }
    let start = rotation_deg.to_radians();
    let mut out: Vec<[f32; 2]> = Vec::with_capacity(sides + 1);
    for i in 0..sides {
        // Angle 0 is the rightmost point; with y down an
        // increasing angle runs clockwise on screen.
        let t = start + (i as f32) * std::f32::consts::TAU / (sides as f32);
        out.push([center[0] + radius * t.cos(), center[1] + radius * t.sin()]);
    }
    // Close with the first point verbatim, so the last point
    // equals it exactly.
    out.push(out[0]);
    out
}

/// Outline points for a star with `points` points, centred on
/// `center` (stage coordinates, y down). Vertices alternate
/// between the `outer` and `inner` radii, starting with an outer
/// vertex at `rotation_deg` (0° points right; positive angles
/// run clockwise as seen on screen) and proceeding clockwise.
/// The outline is closed: `2 * points + 1` points with
/// last == first, the closing point a verbatim copy of the
/// first.
///
/// Degenerate case: `points < 2` cannot form a star, so it
/// returns the two-point placeholder `[center, center]`.
pub fn star_points(
    center: [f32; 2],
    outer: f32,
    inner: f32,
    points: usize,
    rotation_deg: f32,
) -> Vec<[f32; 2]> {
    if points < 2 {
        return vec![center, center];
    }
    let start = rotation_deg.to_radians();
    let total = 2 * points;
    let mut out: Vec<[f32; 2]> = Vec::with_capacity(total + 1);
    for i in 0..total {
        let t = start + (i as f32) * std::f32::consts::TAU / (total as f32);
        let r = if i % 2 == 0 { outer } else { inner };
        out.push([center[0] + r * t.cos(), center[1] + r * t.sin()]);
    }
    // Close with the first point verbatim (see above).
    out.push(out[0]);
    out
}

/// Points for an arrow from `a` to `b` (stage coordinates, y
/// down) as a single open polyline: `[a, b, wing1, b, wing2]` —
/// out along the shaft, back along one wing of the head, out to
/// the tip again, and back along the other wing. Each wing runs
/// from the tip `b` in the direction back toward `a`, rotated
/// ±35° away from the shaft, and is exactly `head_len` long.
///
/// Degenerate case: `a == b` has no shaft direction, so only
/// `[a, b]` is returned.
pub fn arrow_points(a: [f32; 2], b: [f32; 2], head_len: f32) -> Vec<[f32; 2]> {
    if a == b {
        return vec![a, b];
    }
    // Unit vector from the tip back toward the tail (nonzero
    // because a != b).
    let d = [a[0] - b[0], a[1] - b[1]];
    let len = (d[0] * d[0] + d[1] * d[1]).sqrt();
    let u = [d[0] / len, d[1] / len];
    let spread = (35.0f32).to_radians();
    let wing = |sign: f32| {
        let t = sign * spread;
        let (c, s) = (t.cos(), t.sin());
        [
            b[0] + head_len * (u[0] * c - u[1] * s),
            b[1] + head_len * (u[0] * s + u[1] * c),
        ]
    };
    vec![a, b, wing(1.0), b, wing(-1.0)]
}

/// Mirror `points` across the vertical line `x = cx` (stage
/// coordinates): each x becomes `2 * cx - x`, y is unchanged,
/// and the point order is preserved (so a closed outline stays
/// closed). Mirroring twice with the same `cx` returns the
/// original points.
pub fn mirror_points_x(points: &[[f32; 2]], cx: f32) -> Vec<[f32; 2]> {
    points.iter().map(|p| [2.0 * cx - p[0], p[1]]).collect()
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

    // ---- stabilise ----

    #[test]
    fn stabilise_zero_strength_is_identity() {
        // Even points packed closer than the 0.5 de-jitter radius
        // survive: strength 0.0 drops nothing.
        let pts = vec![[0.0, 0.0], [0.1, 0.1], [0.2, 0.0], [5.0, 5.0]];
        assert_eq!(stabilise(&pts, 0.0), pts);
        // Out-of-range strengths clamp: negative behaves as 0.0.
        assert_eq!(stabilise(&pts, -2.0), pts);
    }

    #[test]
    fn stabilise_preserves_endpoints() {
        let pts = vec![
            [0.0, 0.0],
            [1.0, 2.0],
            [2.0, 0.0],
            [3.0, 2.0],
            [4.0, 0.0],
        ];
        for s in [0.25, 0.5, 1.0] {
            let out = stabilise(&pts, s);
            assert_eq!(out.first(), Some(&pts[0]), "start pinned at strength {s}");
            assert_eq!(out.last(), Some(&pts[4]), "end pinned at strength {s}");
        }
        // Over-range strength clamps to the 1.0 result.
        assert_eq!(stabilise(&pts, 7.0), stabilise(&pts, 1.0));
    }

    #[test]
    fn stabilise_reduces_jitter_on_a_zigzag() {
        let pts = vec![
            [0.0, 0.0],
            [1.0, 2.0],
            [2.0, 0.0],
            [3.0, 2.0],
            [4.0, 0.0],
        ];
        let out = stabilise(&pts, 1.0);
        // Full strength: each interior point lands on the midpoint
        // of the previous stabilised point and the next kept point.
        assert_eq!(
            out,
            vec![
                [0.0, 0.0],
                [1.0, 0.0],
                [2.0, 1.0],
                [3.0, 0.5],
                [4.0, 0.0],
            ]
        );
        // Local roughness (deviation from the neighbours' mean)
        // strictly drops.
        let roughness = |path: &[V2]| -> f32 {
            (1..path.len() - 1)
                .map(|i| (path[i][1] - (path[i - 1][1] + path[i + 1][1]) * 0.5).abs())
                .sum()
        };
        assert!(
            roughness(&out) < roughness(&pts),
            "roughness {} !< {}",
            roughness(&out),
            roughness(&pts)
        );
    }

    #[test]
    fn stabilise_drops_close_jitter_but_keeps_endpoints() {
        // An interior point within 0.5 of the previous kept point
        // is dropped, whatever the strength.
        let pts = vec![[0.0, 0.0], [0.25, 0.0], [10.0, 0.0]];
        assert_eq!(stabilise(&pts, 0.8), vec![[0.0, 0.0], [10.0, 0.0]]);
        // The last point is an endpoint: kept even when it sits
        // within 0.5 of its predecessor.
        let pts = vec![[0.0, 0.0], [10.0, 0.0], [10.25, 0.0]];
        assert_eq!(
            stabilise(&pts, 1.0),
            vec![[0.0, 0.0], [5.125, 0.0], [10.25, 0.0]]
        );
    }

    #[test]
    fn stabilise_degenerate_inputs() {
        assert!(stabilise(&[], 0.5).is_empty());
        assert!(stabilise(&[], 1.0).is_empty());
        assert_eq!(stabilise(&[[3.0, 4.0]], 1.0), vec![[3.0, 4.0]]);
        // Two points have no interior: returned unchanged.
        let pts = vec![[0.0, 0.0], [10.0, 0.0]];
        assert_eq!(stabilise(&pts, 1.0), pts);
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

    // ---- brush_library ----

    #[test]
    fn brush_library_is_sane_and_pen_is_the_legacy_default() {
        let lib = brush_library();
        assert!(lib.len() >= 5, "a handful of presets: {}", lib.len());
        // Names are unique (the editor reports them in its status).
        for (i, a) in lib.iter().enumerate() {
            for b in &lib[i + 1..] {
                assert_ne!(a.name, b.name, "duplicate brush name");
            }
            assert!(a.width > 0.0, "{} has a usable width", a.name);
            assert!(
                (0.0..360.0).contains(&a.nib_angle),
                "{} nib angle in range",
                a.name
            );
            assert!(a.smoothing <= 3, "{} smoothing is bounded", a.name);
        }
        // Index 0 is the default, and it must reproduce the
        // pre-library recording exactly: width 4.0, round nib
        // (no profiling), no smoothing.
        let pen = lib[0];
        assert_eq!(pen.name, "Pen");
        assert_eq!(pen.width, 4.0);
        assert_eq!(pen.nib, Nib::Round);
        assert_eq!(pen.smoothing, 0);
        // The library exercises every nib shape.
        for nib in [Nib::Round, Nib::Square, Nib::Calligraphy] {
            assert!(
                lib.iter().any(|b| b.nib == nib),
                "some brush uses {nib:?}"
            );
        }
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

// ---- Wave 10 brush additions ----
//
// Separate module on purpose: the tests below import exact names
// only (never a glob), and they pin the append-only contract of
// `brush_library` — the six original presets keep their indices
// and the three Wave 10 presets sit after them, so a saved
// `brush_index` can never silently change brushes.
#[cfg(test)]
mod brush_wave10_tests {
    use crate::paint::{brush_library, nib_widths, Nib};

    fn assert_close(actual: f32, expected: f32, what: &str) {
        assert!(
            (actual - expected).abs() < 1e-3,
            "{what}: expected {expected}, got {actual}"
        );
    }

    #[test]
    fn library_grew_by_exactly_three_and_originals_did_not_move() {
        let lib = brush_library();
        assert_eq!(
            lib.len(),
            10,
            "six originals + three Wave 10 presets + Spray (Wave 12)"
        );
        // The pre-existing order, pinned by name: a saved index
        // into this list must keep meaning the same brush.
        let originals = [
            "Pen",
            "Pencil",
            "Marker",
            "Calligraphy",
            "Brush",
            "Paint Brush",
        ];
        for (i, name) in originals.iter().enumerate() {
            assert_eq!(lib[i].name, *name, "index {i} moved");
        }
        // Two originals pinned by their defining fields as well,
        // not just by name.
        assert_eq!(lib[3].width, 7.0);
        assert_eq!(lib[3].nib, Nib::Calligraphy);
        assert_eq!(lib[3].nib_angle, 30.0);
        assert_eq!(lib[5].width, 12.0);
        assert_eq!(lib[5].nib, Nib::Square);
        assert_eq!(lib[5].smoothing, 2);
    }

    #[test]
    fn calligraphy_l_is_the_wide_mirrored_broad_edge() {
        let lib = brush_library();
        let b = lib[6];
        assert_eq!(b.name, "Calligraphy L");
        assert_eq!(b.width, 10.0);
        assert_eq!(b.nib, Nib::Calligraphy);
        assert_eq!(b.nib_angle, 315.0);
        assert_eq!(b.smoothing, 1);
        // Behaviour, not just data: on a horizontal stroke the
        // 315° edge gives base * |sin(45°)| ≈ 7.07, while the
        // original 30° Calligraphy gives base * |sin(30°)| = 5.0
        // (at the same base width) — the mirror really does swap
        // which strokes come out thick.
        let horiz = vec![[0.0, 0.0], [10.0, 0.0]];
        let mirrored = nib_widths(&horiz, b.width, b.nib, b.nib_angle);
        assert_close(mirrored[0], 10.0 * std::f32::consts::FRAC_1_SQRT_2, "L on horizontal");
        let original = nib_widths(&horiz, b.width, Nib::Calligraphy, 30.0);
        assert_close(original[0], 5.0, "original angle on horizontal");
    }

    #[test]
    fn chisel_marker_is_thin_square_at_45_and_unsmoothed() {
        let lib = brush_library();
        let b = lib[7];
        assert_eq!(b.name, "Chisel Marker");
        assert_eq!(b.width, 5.0);
        assert_eq!(b.nib, Nib::Square);
        assert_eq!(b.nib_angle, 45.0);
        assert_eq!(b.smoothing, 0);
        // Full width travelling horizontally (psi = -45° puts the
        // square's silhouette at its widest) ...
        let horiz = vec![[0.0, 0.0], [10.0, 0.0]];
        let w = nib_widths(&horiz, b.width, b.nib, b.nib_angle);
        assert_close(w[0], 5.0, "chisel on horizontal");
        // ... and base/√2 travelling along the nib's own diagonal.
        let diag = vec![[0.0, 0.0], [10.0, 10.0]];
        let w = nib_widths(&diag, b.width, b.nib, b.nib_angle);
        assert_close(w[0], 5.0 / std::f32::consts::SQRT_2, "chisel on its diagonal");
    }

    #[test]
    fn broad_brush_is_the_widest_and_most_smoothed() {
        let lib = brush_library();
        let b = lib[8];
        assert_eq!(b.name, "Broad Brush");
        assert_eq!(b.width, 20.0);
        assert_eq!(b.nib, Nib::Square);
        assert_eq!(b.nib_angle, 0.0);
        assert_eq!(b.smoothing, 3);
        // It really is the widest entry in the library.
        assert!(
            lib.iter().all(|other| other.width <= b.width),
            "Broad Brush must be the widest preset"
        );
    }
}

// Wave 11: shape-tool geometry. Exact-name imports only — this
// file has no gpui import, but the rule is kept anyway so the
// pattern stays safe to copy (see the Wave 9 #[test] trap).
#[cfg(test)]
mod shape_wave11_tests {
    use crate::paint::{shape_points, ShapeKind};

    fn assert_all_finite(pts: &[[f32; 2]], what: &str) {
        for p in pts {
            assert!(
                p[0].is_finite() && p[1].is_finite(),
                "{what}: non-finite point {p:?}"
            );
        }
    }

    #[test]
    fn line_is_exactly_the_two_endpoints() {
        let pts = shape_points(ShapeKind::Line, [1.0, 2.0], [3.0, 4.0]);
        assert_eq!(pts, vec![[1.0, 2.0], [3.0, 4.0]]);
        // Drag direction is preserved: a line is [a, b], not a
        // normalised pair.
        let back = shape_points(ShapeKind::Line, [3.0, 4.0], [1.0, 2.0]);
        assert_eq!(back, vec![[3.0, 4.0], [1.0, 2.0]]);
    }

    #[test]
    fn rect_is_closed_and_starts_at_a() {
        let pts = shape_points(ShapeKind::Rect, [10.0, 20.0], [30.0, 50.0]);
        assert_eq!(pts.len(), 5, "4 corners + closing repeat");
        assert_eq!(pts[0], [10.0, 20.0], "starts at a's corner");
        assert_eq!(pts[4], pts[0], "closed: last == first");
    }

    #[test]
    fn rect_corners_clockwise_for_down_right_drag() {
        // a is the top-left corner: clockwise on screen (y down)
        // runs right along the top edge first.
        let pts = shape_points(ShapeKind::Rect, [10.0, 20.0], [30.0, 50.0]);
        assert_eq!(
            pts,
            vec![
                [10.0, 20.0],
                [30.0, 20.0],
                [30.0, 50.0],
                [10.0, 50.0],
                [10.0, 20.0],
            ]
        );
    }

    #[test]
    fn rect_corners_clockwise_when_a_is_below_right_of_b() {
        // Same box dragged the opposite way: a is the bottom-right
        // corner. The run still starts at a and stays clockwise —
        // from bottom-right, clockwise heads left along the bottom
        // edge (the same rotational sense as the forward drag).
        let pts = shape_points(ShapeKind::Rect, [30.0, 50.0], [10.0, 20.0]);
        assert_eq!(
            pts,
            vec![
                [30.0, 50.0],
                [10.0, 50.0],
                [10.0, 20.0],
                [30.0, 20.0],
                [30.0, 50.0],
            ]
        );
        // Normalisation: both drag directions trace the same four
        // corners (only the starting corner differs).
        let fwd = shape_points(ShapeKind::Rect, [10.0, 20.0], [30.0, 50.0]);
        let mut a: Vec<[f32; 2]> = pts[..4].to_vec();
        let mut b: Vec<[f32; 2]> = fwd[..4].to_vec();
        a.sort_by(|p, q| p.partial_cmp(q).unwrap());
        b.sort_by(|p, q| p.partial_cmp(q).unwrap());
        assert_eq!(a, b, "same corner set regardless of drag direction");
    }

    #[test]
    fn rect_corners_clockwise_for_mixed_drag() {
        // a is top-right, b is bottom-left: clockwise from
        // top-right heads down the right edge.
        let pts = shape_points(ShapeKind::Rect, [30.0, 20.0], [10.0, 50.0]);
        assert_eq!(
            pts,
            vec![
                [30.0, 20.0],
                [30.0, 50.0],
                [10.0, 50.0],
                [10.0, 20.0],
                [30.0, 20.0],
            ]
        );
        // And from bottom-left, clockwise heads up the left edge.
        let pts = shape_points(ShapeKind::Rect, [10.0, 50.0], [30.0, 20.0]);
        assert_eq!(
            pts,
            vec![
                [10.0, 50.0],
                [10.0, 20.0],
                [30.0, 20.0],
                [30.0, 50.0],
                [10.0, 50.0],
            ]
        );
    }

    #[test]
    fn ellipse_is_closed_25_points_starting_at_rightmost() {
        let pts = shape_points(ShapeKind::Ellipse, [0.0, 0.0], [100.0, 60.0]);
        assert_eq!(pts.len(), 25, "24 segments + closing repeat");
        // Centre (50, 30), rx 50: the rightmost point is exact
        // (angle 0 → cos 1, sin 0 with no rounding).
        assert_eq!(pts[0], [100.0, 30.0], "starts at the rightmost point");
        assert_eq!(pts[24], pts[0], "closed: last == first");
        // Clockwise on screen (y down): the next point after the
        // rightmost dips DOWN toward the bottom of the ellipse.
        assert!(pts[1][1] > pts[0][1], "clockwise: y grows after the rightmost point");
        // Quarter turn (segment 6 of 24) is the bottom point.
        assert!((pts[6][0] - 50.0).abs() < 1e-3, "bottom point x is the centre x");
        assert!((pts[6][1] - 60.0).abs() < 1e-3, "bottom point y is the box bottom");
    }

    #[test]
    fn ellipse_points_satisfy_the_ellipse_equation() {
        let pts = shape_points(ShapeKind::Ellipse, [0.0, 0.0], [100.0, 60.0]);
        let (cx, cy, rx, ry) = (50.0f32, 30.0f32, 50.0f32, 30.0f32);
        for p in &pts {
            let v = ((p[0] - cx) / rx).powi(2) + ((p[1] - cy) / ry).powi(2);
            assert!((v - 1.0).abs() < 1e-3, "point {p:?} off the ellipse: {v}");
        }
    }

    #[test]
    fn ellipse_ignores_drag_direction() {
        // The start is defined as the rightmost point, not a's
        // corner, so both drags produce the identical outline.
        let fwd = shape_points(ShapeKind::Ellipse, [0.0, 0.0], [100.0, 60.0]);
        let back = shape_points(ShapeKind::Ellipse, [100.0, 60.0], [0.0, 0.0]);
        assert_eq!(fwd, back);
    }

    #[test]
    fn degenerate_drags_stay_finite() {
        // a == b for every kind.
        let p = [7.0, 9.0];
        assert_eq!(shape_points(ShapeKind::Line, p, p), vec![p, p]);
        let r = shape_points(ShapeKind::Rect, p, p);
        assert_eq!(r.len(), 5);
        assert!(r.iter().all(|&q| q == p), "zero-size rect is the point itself");
        let e = shape_points(ShapeKind::Ellipse, p, p);
        assert_eq!(e.len(), 25);
        assert!(e.iter().all(|&q| q == p), "zero-size ellipse is the point itself");

        // Zero width: a vertical drag.
        let r = shape_points(ShapeKind::Rect, [5.0, 0.0], [5.0, 40.0]);
        assert_eq!(r.len(), 5);
        assert_all_finite(&r, "zero-width rect");
        let e = shape_points(ShapeKind::Ellipse, [5.0, 0.0], [5.0, 40.0]);
        assert_eq!(e.len(), 25);
        assert_all_finite(&e, "zero-width ellipse");
        assert!(
            e.iter().all(|q| q[0] == 5.0),
            "zero-width ellipse stays on the centre line"
        );

        // Zero height: a horizontal drag.
        let r = shape_points(ShapeKind::Rect, [0.0, 8.0], [40.0, 8.0]);
        assert_all_finite(&r, "zero-height rect");
        let e = shape_points(ShapeKind::Ellipse, [0.0, 8.0], [40.0, 8.0]);
        assert_all_finite(&e, "zero-height ellipse");
        assert!(
            e.iter().all(|q| q[1] == 8.0),
            "zero-height ellipse stays on the centre line"
        );
    }
}

// Wave 12: shape generators round 2 + mirror helper. Exact-name
// imports only (see the Wave 9 #[test] trap note above).
#[cfg(test)]
mod shape_wave12_tests {
    use crate::paint::{arrow_points, mirror_points_x, regular_polygon_points, star_points};

    fn dist(p: [f32; 2], q: [f32; 2]) -> f32 {
        ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)).sqrt()
    }

    fn assert_point_near(p: [f32; 2], q: [f32; 2], eps: f32) {
        assert!(
            (p[0] - q[0]).abs() <= eps && (p[1] - q[1]).abs() <= eps,
            "{p:?} not within {eps} of {q:?}"
        );
    }

    #[test]
    fn polygon_square_at_rotation_zero() {
        let pts = regular_polygon_points([10.0, 20.0], 5.0, 4, 0.0);
        assert_eq!(pts.len(), 5, "4 sides + closing repeat");
        // First vertex at the rotation angle: rotation 0 is
        // exactly center + (radius, 0) (cos 0 = 1, sin 0 = 0).
        assert_eq!(pts[0], [15.0, 20.0]);
        assert_eq!(pts[4], pts[0], "closed: last == first");
        // Clockwise on screen (y down): from the rightmost point
        // the run heads DOWN to the bottom point, then left,
        // then top.
        assert_point_near(pts[1], [10.0, 25.0], 1e-4);
        assert_point_near(pts[2], [5.0, 20.0], 1e-4);
        assert_point_near(pts[3], [10.0, 15.0], 1e-4);
    }

    #[test]
    fn polygon_counts_rotation_and_circumcircle() {
        let hex = regular_polygon_points([0.0, 0.0], 10.0, 6, 0.0);
        assert_eq!(hex.len(), 7, "6 sides + closing repeat");
        assert_eq!(hex[6], hex[0], "closed: last == first");
        for p in &hex {
            assert!(
                (dist(*p, [0.0, 0.0]) - 10.0).abs() < 1e-3,
                "vertex {p:?} off the circumcircle"
            );
        }
        // Rotation turns the first vertex: at 90° it is the
        // bottom point (clockwise from +x in y-down coordinates).
        let pts = regular_polygon_points([10.0, 20.0], 5.0, 4, 90.0);
        assert_point_near(pts[0], [10.0, 25.0], 1e-4);
    }

    #[test]
    fn polygon_degenerate_sides_return_centre_pair() {
        let c = [3.0, 4.0];
        for sides in [0usize, 1, 2] {
            assert_eq!(regular_polygon_points(c, 5.0, sides, 0.0), vec![c, c]);
        }
    }

    #[test]
    fn star_five_points_has_11_points_closed() {
        let pts = star_points([0.0, 0.0], 10.0, 4.0, 5, 0.0);
        assert_eq!(pts.len(), 11, "2*5 vertices + closing repeat");
        assert_eq!(pts[10], pts[0], "closed: last == first");
        // First vertex is outer, at the rotation angle: exactly
        // center + (outer, 0).
        assert_eq!(pts[0], [10.0, 0.0]);
    }

    #[test]
    fn star_alternates_outer_and_inner_radii() {
        let c = [100.0, 50.0];
        let pts = star_points(c, 10.0, 4.0, 5, 0.0);
        for (i, p) in pts[..10].iter().enumerate() {
            let want = if i % 2 == 0 { 10.0 } else { 4.0 };
            assert!(
                (dist(*p, c) - want).abs() < 1e-3,
                "vertex {i} at the wrong radius: {p:?}"
            );
        }
        // Clockwise on screen (y down): the first inner vertex
        // (36° on) sits below the centre line.
        assert!(pts[1][1] > c[1], "clockwise: y grows after the first vertex");
    }

    #[test]
    fn star_degenerate_points_return_centre_pair() {
        let c = [3.0, 4.0];
        assert_eq!(star_points(c, 10.0, 4.0, 0, 0.0), vec![c, c]);
        assert_eq!(star_points(c, 10.0, 4.0, 1, 0.0), vec![c, c]);
    }

    #[test]
    fn arrow_polyline_shape_and_wings() {
        let pts = arrow_points([0.0, 0.0], [10.0, 0.0], 5.0);
        assert_eq!(pts.len(), 5);
        assert_eq!(pts[0], [0.0, 0.0], "starts at the tail");
        assert_eq!(pts[1], [10.0, 0.0], "out to the tip");
        assert_eq!(pts[3], [10.0, 0.0], "back to the tip between the wings");
        let (w1, w2) = (pts[2], pts[4]);
        // Both wings are exactly head_len from the tip.
        assert!((dist(w1, [10.0, 0.0]) - 5.0).abs() < 1e-4, "wing1 length");
        assert!((dist(w2, [10.0, 0.0]) - 5.0).abs() < 1e-4, "wing2 length");
        // Hand-computed: the direction from b back toward a is
        // (-1, 0); rotated ±35° and scaled by 5 it lands at
        // b + (-5·cos35°, ∓5·sin35°) ≈ (5.9042, ∓2.8679).
        assert_point_near(w1, [5.9042, -2.8679], 1e-3);
        assert_point_near(w2, [5.9042, 2.8679], 1e-3);
    }

    #[test]
    fn arrow_degenerate_returns_endpoints_only() {
        let p = [4.0, 6.0];
        assert_eq!(arrow_points(p, p, 5.0), vec![p, p]);
    }

    #[test]
    fn mirror_known_point_and_order() {
        assert_eq!(mirror_points_x(&[[3.0, 7.0]], 10.0), vec![[17.0, 7.0]]);
        // Order and y are preserved; x reflects about cx, and a
        // point on the mirror line stays put.
        let pts = vec![[0.0, 1.0], [2.0, 3.0], [10.0, 5.0]];
        assert_eq!(
            mirror_points_x(&pts, 10.0),
            vec![[20.0, 1.0], [18.0, 3.0], [10.0, 5.0]]
        );
        assert!(mirror_points_x(&[], 10.0).is_empty());
    }

    #[test]
    fn mirror_twice_returns_the_original_exactly() {
        let pts = vec![[1.5, 2.0], [30.0, 40.5], [99.0, 0.0], [12.25, 7.75]];
        let once = mirror_points_x(&pts, 50.0);
        assert_eq!(mirror_points_x(&once, 50.0), pts);
    }
}

// ---- Wave 12 brush model depth ----
//
// Separate module on purpose (same pattern as the Wave 10 and
// Wave 11 modules): exact-name imports only, never a glob. Pins
// the opacity/scatter model fields, the append-only Spray preset
// at index 9, and the determinism contract of `spray_offsets`.
#[cfg(test)]
mod brush_wave12_tests {
    use crate::paint::{brush_library, spray_offsets, Nib};

    #[test]
    fn spray_is_appended_at_index_9() {
        let lib = brush_library();
        assert_eq!(lib.len(), 10, "nine earlier presets + Spray");
        // Indices 0..=8 did not move: the previous tail is intact.
        assert_eq!(lib[8].name, "Broad Brush");
        let b = lib[9];
        assert_eq!(b.name, "Spray");
        assert_eq!(b.width, 14.0);
        assert_eq!(b.nib, Nib::Round);
        assert_eq!(b.nib_angle, 0.0);
        assert_eq!(b.smoothing, 0);
        assert!((b.opacity - 0.5).abs() < 1e-6, "half opacity: {}", b.opacity);
        assert_eq!(b.scatter, 12.0);
        assert!(b.scatter > 0.0, "Spray is the first scattered preset");
    }

    #[test]
    fn existing_brushes_keep_opaque_unscattered_defaults() {
        let lib = brush_library();
        // Behaviour unchanged for every pre-Wave-12 preset: fully
        // opaque, marks exactly on the path.
        for b in &lib[..9] {
            assert_eq!(b.opacity, 1.0, "{} stays fully opaque", b.name);
            assert_eq!(b.scatter, 0.0, "{} stays unscattered", b.name);
        }
        // Every preset carries sane model values.
        for b in &lib {
            assert!(
                (0.0..=1.0).contains(&b.opacity),
                "{} opacity in range",
                b.name
            );
            assert!(b.scatter >= 0.0, "{} scatter not negative", b.name);
        }
    }

    #[test]
    fn spray_offsets_are_deterministic_per_seed() {
        let a = spray_offsets(42, 32, 12.0);
        let b = spray_offsets(42, 32, 12.0);
        assert_eq!(a, b, "same seed, same offsets, every run");
        let c = spray_offsets(43, 32, 12.0);
        assert_ne!(a, c, "a different seed gives a different spray");
    }

    #[test]
    fn spray_offsets_stay_within_radius() {
        for seed in [0u64, 1, 7, 0xDEAD_BEEF] {
            let pts = spray_offsets(seed, 64, 7.5);
            assert_eq!(pts.len(), 64);
            for p in &pts {
                let len = (p[0] * p[0] + p[1] * p[1]).sqrt();
                assert!(
                    len <= 7.5 + 1e-3,
                    "seed {seed}: offset {p:?} at distance {len} escapes the disc"
                );
            }
            // The spray actually spreads: with a sqrt-uniform
            // disc, 64 points cannot all sit in the inner half.
            let max_len = pts
                .iter()
                .map(|p| (p[0] * p[0] + p[1] * p[1]).sqrt())
                .fold(0.0f32, f32::max);
            assert!(
                max_len > 7.5 * 0.5,
                "seed {seed}: spray hugs the centre (max {max_len})"
            );
        }
    }

    #[test]
    fn spray_offsets_count_and_degenerate_cases() {
        // Count is honoured exactly, including zero.
        assert!(spray_offsets(9, 0, 5.0).is_empty());
        assert_eq!(spray_offsets(9, 1, 5.0).len(), 1);
        assert_eq!(spray_offsets(9, 5, 5.0).len(), 5);
        assert_eq!(spray_offsets(9, 100, 5.0).len(), 100);
        // Radius zero collapses every offset to the centre.
        let zeros = spray_offsets(9, 8, 0.0);
        assert!(
            zeros.iter().all(|&p| p == [0.0, 0.0]),
            "radius 0 gives all-zero offsets: {zeros:?}"
        );
        // Seed zero is remapped off the xorshift fixed point: it
        // still produces a spread, not one repeated offset.
        let seeded_zero = spray_offsets(0, 16, 5.0);
        assert!(
            seeded_zero.iter().any(|&p| p != seeded_zero[0]),
            "seed 0 must not emit a single repeated offset"
        );
    }
}
