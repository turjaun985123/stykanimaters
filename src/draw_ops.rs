//! Project-level drawing operations: the whole-project callers
//! for the pure stroke geometry in [`crate::paint`]. Where
//! `paint.rs` works on bare point lists and stroke slices, this
//! module knows about [`Project`] and frame numbers, so the editor
//! (and any other frontend) can smooth, nib-profile and erase the
//! strokes of one frame — or mirror every stroke in the project at
//! once — without re-implementing the bookkeeping.
//!
//! Width conventions (shared with `model::Stroke`, the rasterizer
//! in `export.rs` and `paint::erase_at`): `Stroke.widths` is
//! either empty — the stroke has the uniform `Stroke.width` — or
//! exactly parallel to `Stroke.points`, one width per point. A
//! non-empty, non-parallel `widths` vector is meaningless to every
//! consumer (the rasterizer only honours the parallel form), so
//! the operations here never produce one and normalise it away
//! where they rewrite a stroke, matching `erase_at`'s behaviour.
//! One deliberate exception: [`simplify_stroke_at`] never
//! rewrites `widths` unless they are exactly parallel — its
//! contract is index-exactness, so a broken vector is left
//! exactly as found rather than normalised (see its docs).

use crate::model::{Project, Stroke, V2};
use crate::paint::{erase_at, mirror_points_x, nib_widths, simplify_points, smooth_path, Nib};

fn distance(a: V2, b: V2) -> f32 {
    ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt()
}

/// How many strokes in `project` belong to `frame`.
pub fn stroke_count(project: &Project, frame: usize) -> usize {
    project.strokes.iter().filter(|s| s.frame == frame).count()
}

/// Cumulative chord lengths along a polyline: entry `i` is the
/// travelled distance from point 0 to point `i` (entry 0 is 0).
fn cumulative_lengths(points: &[V2]) -> Vec<f32> {
    let mut out = Vec::with_capacity(points.len());
    let mut acc = 0.0f32;
    out.push(0.0);
    for w in points.windows(2) {
        acc += distance(w[0], w[1]);
        out.push(acc);
    }
    out
}

/// Widths for `new_points` sampled from a per-point width profile
/// defined on `orig_points`: each new point takes the width the
/// original profile has at the same *normalised arc position*
/// (travelled distance ÷ total length), linearly interpolated
/// between the two original points bracketing that position.
/// Endpoints map exactly, so the first and last widths are
/// preserved. If either polyline has zero total length there is
/// no arc position to speak of and every new point gets the
/// original first width.
fn sample_widths(orig_points: &[V2], orig_widths: &[f32], new_points: &[V2]) -> Vec<f32> {
    debug_assert_eq!(orig_points.len(), orig_widths.len());
    if new_points.is_empty() {
        return Vec::new();
    }
    let orig_cum = cumulative_lengths(orig_points);
    let new_cum = cumulative_lengths(new_points);
    let total_orig = *orig_cum.last().unwrap_or(&0.0);
    let total_new = *new_cum.last().unwrap_or(&0.0);
    if total_orig <= 0.0 || total_new <= 0.0 {
        return vec![orig_widths.first().copied().unwrap_or(0.0); new_points.len()];
    }
    let last_seg = orig_points.len() - 2;
    new_cum
        .iter()
        .map(|&d| {
            let target = d / total_new * total_orig;
            // Find the original segment bracketing `target`.
            let mut seg = last_seg;
            for k in 0..=last_seg {
                if target <= orig_cum[k + 1] || k == last_seg {
                    seg = k;
                    break;
                }
            }
            let seg_len = orig_cum[seg + 1] - orig_cum[seg];
            let t = if seg_len > 1e-9 {
                ((target - orig_cum[seg]) / seg_len).clamp(0.0, 1.0)
            } else {
                0.0
            };
            orig_widths[seg] + (orig_widths[seg + 1] - orig_widths[seg]) * t
        })
        .collect()
}

/// Smooth one stroke in place with Chaikin corner cutting
/// (`subdiv` iterations, see [`smooth_path`]). Endpoints are
/// pinned; all other metadata (`frame`, `color`, `width`, `fill`)
/// is untouched — unlike the eraser, smoothing never cuts the
/// outline, so a fill stays valid.
///
/// Per-point widths are resampled onto the smoothed points by
/// normalised arc position (see [`sample_widths`]), so the result
/// stays parallel. A uniform stroke (empty `widths`) stays
/// uniform. A non-parallel `widths` vector — meaningless to every
/// consumer — is normalised to the uniform form rather than kept
/// at a length that could accidentally become parallel to the new
/// point count. Strokes with fewer than 2 points, and `subdiv ==
/// 0`, are left completely untouched.
pub fn smooth_stroke(stroke: &mut Stroke, subdiv: usize) {
    if subdiv == 0 || stroke.points.len() < 2 {
        return;
    }
    let new_points = smooth_path(&stroke.points, subdiv);
    let new_widths = if stroke.widths.len() == stroke.points.len() {
        sample_widths(&stroke.points, &stroke.widths, &new_points)
    } else {
        // Empty stays empty (uniform); a broken non-parallel
        // vector is normalised to empty for the reason above.
        Vec::new()
    };
    stroke.points = new_points;
    stroke.widths = new_widths;
}

/// Smooth every stroke on `frame` (see [`smooth_stroke`]) and
/// return how many strokes actually changed — a stroke counts
/// when its point list differs afterwards, so degenerate strokes
/// (< 2 points) and `subdiv == 0` never count. Strokes on other
/// frames are not touched.
pub fn smooth_frame(project: &mut Project, frame: usize, subdiv: usize) -> usize {
    let mut changed = 0;
    for stroke in &mut project.strokes {
        if stroke.frame != frame {
            continue;
        }
        let before = stroke.points.clone();
        smooth_stroke(stroke, subdiv);
        if stroke.points != before {
            changed += 1;
        }
    }
    changed
}

/// Give a stroke a nib width profile: replace its per-point
/// widths with [`nib_widths`] computed from its points and its
/// uniform `width` as the nib size. This always produces widths
/// parallel to the points (empty for an empty stroke, `[width]`
/// for a single point), converting a uniform stroke into the
/// per-point form; any previous per-point widths are discarded.
pub fn apply_nib(stroke: &mut Stroke, nib: Nib, nib_angle_deg: f32) {
    stroke.widths = nib_widths(&stroke.points, stroke.width, nib, nib_angle_deg);
}

/// Erase around `centre` on one frame: the frame's strokes are
/// replaced by [`erase_at`]'s output (points within `radius`
/// removed, strokes split where cut, fills cleared on modified
/// strokes), spliced back at the position of the frame's first
/// stroke so strokes on other frames keep their places and the
/// frame's own pieces keep their relative order.
///
/// Returns how many of the frame's strokes the eraser *changed*:
/// each source stroke that was trimmed, split (counted once,
/// however many pieces it became) or removed entirely counts 1;
/// untouched strokes count 0. A stroke is changed exactly when
/// at least one of its points lies within `radius` of `centre`
/// — the same test `erase_at` applies.
pub fn erase(project: &mut Project, frame: usize, centre: V2, radius: f32) -> usize {
    let frame_strokes: Vec<Stroke> = project
        .strokes
        .iter()
        .filter(|s| s.frame == frame)
        .cloned()
        .collect();
    if frame_strokes.is_empty() {
        return 0;
    }
    let changed = frame_strokes
        .iter()
        .filter(|s| s.points.iter().any(|&p| distance(p, centre) <= radius))
        .count();
    if changed == 0 {
        return 0;
    }
    let pieces = erase_at(&frame_strokes, centre, radius);
    let mut out: Vec<Stroke> = Vec::with_capacity(project.strokes.len());
    let mut inserted = false;
    for stroke in std::mem::take(&mut project.strokes) {
        if stroke.frame == frame {
            if !inserted {
                out.extend(pieces.iter().cloned());
                inserted = true;
            }
            // Original frame strokes are superseded by `pieces`.
        } else {
            out.push(stroke);
        }
    }
    project.strokes = out;
    changed
}

/// Mirror every stroke in the project across the vertical line
/// `x = cx` (see [`mirror_points_x`]) and return how many strokes
/// were mirrored: every stroke in `project.strokes`, on every
/// frame, so a strokeless project returns 0.
///
/// Only `points` change: mirroring preserves the point count, so
/// per-point `widths` stay parallel and keep describing the same
/// points, and a `fill` stays valid because the mirrored outline
/// is congruent to the original (unlike the eraser, which cuts
/// outlines and must clear fills). Frame, colour and width
/// metadata are untouched.
pub fn mirror_strokes_all(project: &mut Project, cx: f32) -> usize {
    let mut mirrored = 0;
    for stroke in &mut project.strokes {
        stroke.points = mirror_points_x(&stroke.points, cx);
        mirrored += 1;
    }
    mirrored
}

/// Simplify every stroke on `frame` with Douglas-Peucker (see
/// [`simplify_points`]): interior points that stay within
/// `epsilon` of the simplified path are dropped. Returns how
/// many strokes actually lost points — a stroke whose point
/// count is unchanged (already simple, degenerate, or
/// `epsilon <= 0.0`) does not count, and strokes on other
/// frames are not touched.
///
/// Per-point widths are resampled onto the surviving points by
/// normalised arc position (see [`sample_widths`], the same
/// treatment [`smooth_stroke`] gives its rewritten points), so
/// the result stays parallel and the profile's endpoint widths
/// are preserved; a uniform stroke (empty `widths`) stays
/// uniform, and a broken non-parallel vector is normalised to
/// the uniform form. Like smoothing, simplification never cuts
/// the outline — kept points are a subset of the original — so
/// a `fill` stays valid and is kept.
pub fn simplify_strokes(project: &mut Project, frame: usize, epsilon: f32) -> usize {
    let mut reduced = 0;
    for stroke in &mut project.strokes {
        if stroke.frame != frame {
            continue;
        }
        let new_points = simplify_points(&stroke.points, epsilon);
        if new_points.len() == stroke.points.len() {
            continue;
        }
        let new_widths = if stroke.widths.len() == stroke.points.len() {
            sample_widths(&stroke.points, &stroke.widths, &new_points)
        } else {
            // Empty stays empty (uniform); a broken non-parallel
            // vector is normalised to empty, as in smooth_stroke.
            Vec::new()
        };
        stroke.points = new_points;
        stroke.widths = new_widths;
        reduced += 1;
    }
    reduced
}

/// Distance from `p` to the segment `a`–`b`, computed exactly as
/// `paint`'s private helper of the same name (which is not
/// exported): the perpendicular distance when the projection of
/// `p` falls between the endpoints, the distance to the nearer
/// endpoint otherwise; a zero-length segment degrades to the
/// plain distance to `a`. Kept in step with the measure
/// [`simplify_points`] uses so [`douglas_peucker_indices`]
/// selects precisely the points that function keeps.
fn point_segment_distance(p: V2, a: V2, b: V2) -> f32 {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let len_sq = ab[0] * ab[0] + ab[1] * ab[1];
    let t = if len_sq > 0.0 {
        let ap = [p[0] - a[0], p[1] - a[1]];
        ((ap[0] * ab[0] + ap[1] * ab[1]) / len_sq).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let closest = [a[0] + t * ab[0], a[1] + t * ab[1]];
    let d = [p[0] - closest[0], p[1] - closest[1]];
    (d[0] * d[0] + d[1] * d[1]).sqrt()
}

/// The indices [`simplify_points`] would keep, without losing
/// the mapping back to the original polyline: the
/// Douglas-Peucker kept set of `points` at tolerance `epsilon`,
/// as ascending indices into `points`. The endpoints are always
/// kept, so a non-empty result starts with 0 and ends with
/// `points.len() - 1`; the traversal is the same keep-flag/stack
/// walk `simplify_points` performs (same distance measure, same
/// first-farthest tie-breaking), so for every finite
/// `epsilon > 0.0`, `points` gathered at these indices equals
/// `simplify_points(points, epsilon)` exactly.
///
/// Degenerate cases: an empty polyline gives no indices, one
/// point gives `[0]`, two points give `[0, 1]`. A tolerance that
/// cannot honestly drop anything — `epsilon <= 0.0`, or a
/// non-finite `epsilon` (NaN or infinite, a caller error) —
/// returns every index: unlike `simplify_points`, whose
/// comparisons simply go false against a non-finite tolerance
/// and collapse the polyline to its endpoints, the index form
/// refuses to destroy points over a meaningless tolerance.
/// Pairing the kept indices with a parallel per-point vector
/// (widths, say) is what this function exists for: see
/// [`simplify_stroke_at`].
pub fn douglas_peucker_indices(points: &[[f32; 2]], epsilon: f32) -> Vec<usize> {
    let n = points.len();
    if n < 3 || epsilon <= 0.0 || !epsilon.is_finite() {
        return (0..n).collect();
    }
    let mut keep = vec![false; n];
    keep[0] = true;
    keep[n - 1] = true;
    // Spans between two kept endpoints still to examine.
    let mut stack = vec![(0, n - 1)];
    while let Some((first, last)) = stack.pop() {
        if last <= first + 1 {
            continue;
        }
        let (a, b) = (points[first], points[last]);
        let mut farthest = first + 1;
        let mut max_dist = 0.0f32;
        for i in first + 1..last {
            let d = point_segment_distance(points[i], a, b);
            if d > max_dist {
                max_dist = d;
                farthest = i;
            }
        }
        if max_dist > epsilon {
            keep[farthest] = true;
            stack.push((first, farthest));
            stack.push((farthest, last));
        }
    }
    (0..n).filter(|&i| keep[i]).collect()
}

/// Simplify the single stroke at project-wide `index` (the same
/// indexing `EditorCore::delete_stroke_at` and the shell's
/// stroke picking use — position in `Project.strokes`, with no
/// regard to frames) with Douglas-Peucker at tolerance
/// `epsilon`, keeping the stroke's per-point widths aligned with
/// its points. Returns `true` when the stroke actually lost
/// points; `false` for a bad index and when nothing changed
/// (already simple, degenerate, or a tolerance that drops
/// nothing). Like its project-level siblings this function takes
/// no snapshot and sets no status — the caller (the shell's
/// stroke panel) owns the undo step.
///
/// Widths are carried by *index*, not resampled: when
/// `Stroke.widths` is exactly parallel to the points going in,
/// the surviving points keep their own widths, gathered at the
/// same kept indices (see [`douglas_peucker_indices`]). This is
/// the deliberate difference from the frame-wide
/// [`simplify_strokes`], which resamples the profile onto the
/// survivors by arc position and so shifts interior widths.
/// When `widths` is not exactly parallel going in — empty (the
/// uniform form) or a broken non-parallel vector — it is left
/// byte-for-byte untouched: a non-parallel vector defines no
/// association between its entries and the points, so there is
/// no honest subset of it to pick, and indexing into it anyway
/// would fabricate a width profile. (The frame-wide ops
/// normalise such a vector away instead; this function's
/// contract is index-exactness, so it does not.)
///
/// A spray stroke is simplified by its *base* polyline, like
/// every other stroke: the `spray` mark itself (seed, radius,
/// opacity) is untouched, and the scatter dots consumers derive
/// from the base points follow the simplified path on their own.
/// A `fill` is kept, as in [`simplify_strokes`]: the kept points
/// are a subset of the original, so the outline stays valid.
pub fn simplify_stroke_at(project: &mut Project, index: usize, epsilon: f32) -> bool {
    let Some(stroke) = project.strokes.get_mut(index) else {
        return false;
    };
    let keep = douglas_peucker_indices(&stroke.points, epsilon);
    if keep.len() >= stroke.points.len() {
        return false;
    }
    let new_points: Vec<V2> = keep.iter().map(|&i| stroke.points[i]).collect();
    // Only an exactly-parallel widths vector has entries that
    // belong to points; gather it at the same indices so each
    // surviving point keeps its own width. Anything else (empty
    // = uniform, or broken) is left exactly as found.
    let new_widths = if stroke.widths.len() == stroke.points.len() {
        Some(
            keep.iter()
                .map(|&i| stroke.widths[i])
                .collect::<Vec<f32>>(),
        )
    } else {
        None
    };
    stroke.points = new_points;
    if let Some(widths) = new_widths {
        stroke.widths = widths;
    }
    true
}

#[cfg(test)]
mod mirror_tests {
    use crate::draw_ops::mirror_strokes_all;
    use crate::model::{Project, Stroke, V2};

    fn stroke(frame: usize, points: Vec<V2>) -> Stroke {
        Stroke {
            frame,
            points,
            color: 0xFF0000,
            width: 4.0,
            widths: Vec::new(),
            fill: None,
            spray: None,
            opacity: 1.0,
        }
    }

    fn project_with(strokes: Vec<Stroke>) -> Project {
        let mut p = Project::new_demo();
        p.strokes = strokes;
        p
    }

    #[test]
    fn mirror_strokes_all_mirrors_every_frame_and_counts() {
        let first = stroke(0, vec![[10.0, 5.0], [30.0, 5.0]]);
        let second = stroke(3, vec![[0.0, 0.0], [100.0, 50.0], [40.0, 20.0]]);
        let mut p = project_with(vec![first, second]);

        let mirrored = mirror_strokes_all(&mut p, 50.0);

        assert_eq!(mirrored, 2, "both strokes, across both frames");
        // x' = 2 * 50 - x = 100 - x; y untouched.
        assert_eq!(p.strokes[0].points, vec![[90.0, 5.0], [70.0, 5.0]]);
        assert_eq!(
            p.strokes[1].points,
            vec![[100.0, 0.0], [0.0, 50.0], [60.0, 20.0]]
        );
        // Metadata untouched.
        assert_eq!(p.strokes[0].frame, 0);
        assert_eq!(p.strokes[1].frame, 3);
        assert_eq!(p.strokes[0].color, 0xFF0000);
        assert_eq!(p.strokes[0].width, 4.0);
    }

    #[test]
    fn mirror_strokes_all_on_strokeless_project_is_zero() {
        let mut p = project_with(Vec::new());
        assert_eq!(mirror_strokes_all(&mut p, 50.0), 0);
        assert!(p.strokes.is_empty(), "nothing added, nothing changed");
    }

    #[test]
    fn mirror_strokes_all_twice_restores_points_exactly() {
        let original = vec![[12.5, 3.0], [87.25, 40.0], [50.0, 99.0]];
        let mut p = project_with(vec![stroke(1, original.clone())]);
        assert_eq!(mirror_strokes_all(&mut p, 33.5), 1);
        assert_ne!(p.strokes[0].points, original, "first mirror moves the points");
        assert_eq!(mirror_strokes_all(&mut p, 33.5), 1);
        assert_eq!(
            p.strokes[0].points, original,
            "second mirror restores the points exactly"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stroke(frame: usize, points: Vec<V2>) -> Stroke {
        Stroke {
            frame,
            points,
            color: 0xFF0000,
            width: 4.0,
            widths: Vec::new(),
            fill: None,
            spray: None,
            opacity: 1.0,
        }
    }

    fn project_with(strokes: Vec<Stroke>) -> Project {
        let mut p = Project::new_demo();
        p.strokes = strokes;
        p
    }

    /// Zigzag with parallel widths ramping 2, 4, 6, 8, 10.
    fn jaggy_pressured() -> Stroke {
        let mut s = stroke(
            0,
            vec![
                [0.0, 0.0],
                [10.0, 10.0],
                [20.0, 0.0],
                [30.0, 10.0],
                [40.0, 0.0],
            ],
        );
        s.widths = vec![2.0, 4.0, 6.0, 8.0, 10.0];
        s
    }

    #[test]
    fn smooth_stroke_grows_points_keeps_endpoints_and_parallel_widths() {
        let mut s = jaggy_pressured();
        let (first, last) = (s.points[0], *s.points.last().unwrap());
        smooth_stroke(&mut s, 1);
        assert_eq!(s.points.len(), 10, "5 points * 2^1");
        assert_eq!(s.points.first(), Some(&first), "start pinned");
        assert_eq!(s.points.last(), Some(&last), "end pinned");
        assert_eq!(s.widths.len(), s.points.len(), "widths stay parallel");
        // Arc-position sampling preserves the profile's endpoints.
        assert!((s.widths[0] - 2.0).abs() < 1e-4);
        assert!((s.widths[s.widths.len() - 1] - 10.0).abs() < 1e-4);
        // And stays within the original profile's range throughout.
        assert!(s.widths.iter().all(|&w| (1.99..=10.01).contains(&w)));
        // Other metadata untouched.
        assert_eq!(s.frame, 0);
        assert_eq!(s.color, 0xFF0000);
        assert_eq!(s.width, 4.0);
    }

    #[test]
    fn smooth_stroke_uniform_stays_uniform() {
        let mut s = stroke(0, vec![[0.0, 0.0], [10.0, 10.0], [20.0, 0.0]]);
        smooth_stroke(&mut s, 2);
        assert_eq!(s.points.len(), 12);
        assert!(s.widths.is_empty(), "uniform form is preserved, not fabricated");
    }

    #[test]
    fn smooth_stroke_normalises_broken_widths() {
        let mut s = stroke(0, vec![[0.0, 0.0], [10.0, 0.0], [20.0, 0.0]]);
        s.widths = vec![1.0, 2.0]; // non-parallel: meaningless
        smooth_stroke(&mut s, 1);
        assert!(s.widths.is_empty(), "broken widths drop to the uniform form");
    }

    #[test]
    fn smooth_frame_counts_only_changed_strokes_on_that_frame() {
        let jaggy = stroke(0, vec![[0.0, 0.0], [10.0, 10.0], [20.0, 0.0]]);
        let single = stroke(0, vec![[5.0, 5.0]]);
        let other_frame = stroke(1, vec![[0.0, 0.0], [10.0, 10.0], [20.0, 0.0]]);
        let other_before = other_frame.points.clone();
        let mut p = project_with(vec![jaggy, single, other_frame]);
        assert_eq!(stroke_count(&p, 0), 2);
        assert_eq!(stroke_count(&p, 1), 1);
        assert_eq!(stroke_count(&p, 7), 0);

        let changed = smooth_frame(&mut p, 0, 1);
        assert_eq!(changed, 1, "only the multi-point stroke on frame 0 changes");
        assert_eq!(p.strokes[0].points.len(), 6);
        assert_eq!(p.strokes[1].points, vec![[5.0, 5.0]], "degenerate stroke untouched");
        assert_eq!(p.strokes[2].points, other_before, "other frame untouched");
        // subdiv 0 changes nothing, anywhere.
        assert_eq!(smooth_frame(&mut p, 0, 0), 0);
    }

    #[test]
    fn apply_nib_calligraphy_varies_widths_on_a_bent_stroke() {
        // Right then up: with the nib edge at 0°, the horizontal
        // run is a hairline and the vertical run is full width.
        let mut s = stroke(0, vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]]);
        apply_nib(&mut s, Nib::Calligraphy, 0.0);
        assert_eq!(s.widths.len(), s.points.len(), "widths parallel");
        let max = s.widths.iter().copied().fold(f32::MIN, f32::max);
        let min = s.widths.iter().copied().fold(f32::MAX, f32::min);
        assert!(max > min, "calligraphy widths vary: {:?}", s.widths);
        assert!((max - 4.0).abs() < 1e-4, "across the edge = full base width");
        assert!((min - 0.6).abs() < 1e-4, "along the edge = 0.15 * base");
        // Round nib: parallel and constant.
        let mut r = stroke(0, vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]]);
        apply_nib(&mut r, Nib::Round, 30.0);
        assert!(r.widths.iter().all(|&w| (w - 4.0).abs() < 1e-6));
    }

    #[test]
    fn erase_splits_one_stroke_and_leaves_other_frames_alone() {
        let axis = stroke(
            1,
            (0..=10).map(|i| [i as f32 * 10.0, 0.0]).collect(),
        );
        let bystander = stroke(2, vec![[0.0, 50.0], [10.0, 50.0]]);
        let bystander_points = bystander.points.clone();
        let mut p = project_with(vec![axis, bystander]);

        // Erase the middle of the frame-1 stroke (x = 40, 50, 60 go).
        let changed = erase(&mut p, 1, [50.0, 0.0], 15.0);
        assert_eq!(changed, 1, "one source stroke was changed");
        assert_eq!(stroke_count(&p, 1), 2, "it split into two pieces");
        assert_eq!(stroke_count(&p, 2), 1);
        let pieces: Vec<&Stroke> = p.strokes.iter().filter(|s| s.frame == 1).collect();
        assert_eq!(pieces[0].points.last(), Some(&[30.0, 0.0]));
        assert_eq!(pieces[1].points.first(), Some(&[70.0, 0.0]));
        let other = p.strokes.iter().find(|s| s.frame == 2).unwrap();
        assert_eq!(other.points, bystander_points, "other frame untouched");

        // An eraser that misses everything changes and reports 0.
        let changed = erase(&mut p, 1, [500.0, 500.0], 10.0);
        assert_eq!(changed, 0);
        assert_eq!(stroke_count(&p, 1), 2);
        // Erasing a frame with no strokes is a no-op.
        assert_eq!(erase(&mut p, 9, [0.0, 0.0], 100.0), 0);
    }

    #[test]
    fn erase_counts_each_changed_stroke_once() {
        let hit_and_split = stroke(
            0,
            (0..=10).map(|i| [i as f32 * 10.0, 0.0]).collect(),
        );
        let hit_and_gone = stroke(0, vec![[45.0, 0.0], [55.0, 0.0]]);
        let missed = stroke(0, vec![[0.0, 90.0], [10.0, 90.0]]);
        let mut p = project_with(vec![hit_and_split, hit_and_gone, missed]);
        let changed = erase(&mut p, 0, [50.0, 0.0], 15.0);
        assert_eq!(changed, 2, "split counts once, removal counts once, miss counts 0");
        // Split -> 2 pieces, removed -> 0, missed -> 1.
        assert_eq!(stroke_count(&p, 0), 3);
    }

    #[test]
    fn degenerate_strokes_survive_every_op() {
        let empty = stroke(0, Vec::new());
        let single = stroke(0, vec![[3.0, 4.0]]);

        let mut s = empty.clone();
        smooth_stroke(&mut s, 3);
        assert!(s.points.is_empty());
        apply_nib(&mut s, Nib::Square, 15.0);
        assert!(s.widths.is_empty(), "no points, no widths");

        let mut s = single.clone();
        smooth_stroke(&mut s, 3);
        assert_eq!(s.points, vec![[3.0, 4.0]], "single point is not smoothed away");
        assert!(s.widths.is_empty());
        apply_nib(&mut s, Nib::Calligraphy, 0.0);
        assert_eq!(s.widths, vec![4.0], "single point gets the base width");

        // A distant eraser leaves degenerate strokes exactly as-is,
        // and an on-target one removes the single point's stroke
        // without panicking (single-point runs vanish by design).
        let mut p = project_with(vec![empty, single]);
        assert_eq!(erase(&mut p, 0, [100.0, 100.0], 5.0), 0);
        assert_eq!(stroke_count(&p, 0), 2);
        assert_eq!(erase(&mut p, 0, [3.0, 4.0], 5.0), 1);
        assert_eq!(stroke_count(&p, 0), 1, "only the empty stroke remains");
        assert_eq!(smooth_frame(&mut p, 0, 2), 0);
    }
}

#[cfg(test)]
mod simplify_tests {
    use crate::draw_ops::simplify_strokes;
    use crate::model::{Project, Stroke, V2};

    fn stroke(frame: usize, points: Vec<V2>) -> Stroke {
        Stroke {
            frame,
            points,
            color: 0xFF0000,
            width: 4.0,
            widths: Vec::new(),
            fill: None,
            spray: None,
            opacity: 1.0,
        }
    }

    fn project_with(strokes: Vec<Stroke>) -> Project {
        let mut p = Project::new_demo();
        p.strokes = strokes;
        p
    }

    /// Five points jittering within 0.4 of their chord: at
    /// epsilon 1.0 the stroke collapses to its two endpoints.
    fn jittered(frame: usize) -> Stroke {
        stroke(
            frame,
            vec![
                [0.0, 0.0],
                [10.0, 0.4],
                [20.0, -0.3],
                [30.0, 0.2],
                [40.0, 0.0],
            ],
        )
    }

    #[test]
    fn simplify_strokes_counts_only_reduced_strokes_on_the_frame() {
        let collapsible = jittered(0);
        // A sharp corner (20, 0) sits ~14.14 from its chord, so
        // it survives epsilon 1.0 with its point count unchanged.
        let corner = stroke(0, vec![[0.0, 0.0], [20.0, 0.0], [20.0, 20.0]]);
        let other_frame = jittered(1);
        let other_before = other_frame.points.clone();
        let mut p = project_with(vec![collapsible, corner, other_frame]);

        let reduced = simplify_strokes(&mut p, 0, 1.0);

        assert_eq!(reduced, 1, "only the jittered frame-0 stroke lost points");
        assert_eq!(p.strokes[0].points, vec![[0.0, 0.0], [40.0, 0.0]]);
        assert_eq!(
            p.strokes[1].points,
            vec![[0.0, 0.0], [20.0, 0.0], [20.0, 20.0]],
            "corner stroke unchanged, so it did not count"
        );
        assert_eq!(p.strokes[2].points, other_before, "other frame untouched");
        // Metadata untouched.
        assert_eq!(p.strokes[0].color, 0xFF0000);
        assert_eq!(p.strokes[0].width, 4.0);
    }

    #[test]
    fn simplify_strokes_keeps_widths_parallel_and_endpoints() {
        let mut s = jittered(0);
        s.widths = vec![2.0, 4.0, 6.0, 8.0, 10.0];
        let mut p = project_with(vec![s]);

        assert_eq!(simplify_strokes(&mut p, 0, 1.0), 1);
        let s = &p.strokes[0];
        assert_eq!(s.points.len(), 2);
        assert_eq!(s.widths.len(), s.points.len(), "widths stay parallel");
        // Arc-position resampling preserves the profile's ends.
        assert!((s.widths[0] - 2.0).abs() < 1e-4);
        assert!((s.widths[1] - 10.0).abs() < 1e-4);

        // A uniform stroke stays uniform.
        let mut p = project_with(vec![jittered(0)]);
        assert_eq!(simplify_strokes(&mut p, 0, 1.0), 1);
        assert!(p.strokes[0].widths.is_empty());
    }

    #[test]
    fn simplify_strokes_with_zero_epsilon_and_empty_frames_change_nothing() {
        let mut p = project_with(vec![jittered(0), jittered(1)]);
        let before: Vec<Vec<V2>> =
            p.strokes.iter().map(|s| s.points.clone()).collect();
        assert_eq!(
            simplify_strokes(&mut p, 0, 0.0),
            0,
            "epsilon 0 reduces nothing"
        );
        assert_eq!(
            simplify_strokes(&mut p, 9, 1.0),
            0,
            "no strokes on frame 9"
        );
        let after: Vec<Vec<V2>> =
            p.strokes.iter().map(|s| s.points.clone()).collect();
        assert_eq!(before, after);
    }
}

#[cfg(test)]
mod simplify_at_tests {
    use super::{douglas_peucker_indices, simplify_stroke_at, simplify_strokes};
    use crate::model::{Project, SprayMark, Stroke, V2};
    use crate::paint::simplify_points;

    fn stroke(frame: usize, points: Vec<V2>) -> Stroke {
        Stroke {
            frame,
            points,
            color: 0xFF0000,
            width: 4.0,
            widths: Vec::new(),
            fill: None,
            spray: None,
            opacity: 1.0,
        }
    }

    fn project_with(strokes: Vec<Stroke>) -> Project {
        let mut p = Project::new_demo();
        p.strokes = strokes;
        p
    }

    /// Symmetric zigzag, peaks 10 above the baseline. At
    /// epsilon 7 the kept set is [0, 1, 4]: the first peak is
    /// 10 from the full chord, and both later interior points
    /// sit ~6.32 from the peak-to-end chord, within tolerance.
    fn zigzag() -> Vec<V2> {
        vec![
            [0.0, 0.0],
            [10.0, 10.0],
            [20.0, 0.0],
            [30.0, 10.0],
            [40.0, 0.0],
        ]
    }

    fn pressured_zigzag(frame: usize) -> Stroke {
        let mut s = stroke(frame, zigzag());
        s.widths = vec![2.0, 4.0, 6.0, 8.0, 10.0];
        s
    }

    #[test]
    fn indices_degenerate_inputs_keep_everything() {
        let empty: Vec<V2> = Vec::new();
        assert_eq!(douglas_peucker_indices(&empty, 1.0), Vec::<usize>::new());
        assert_eq!(douglas_peucker_indices(&[[3.0, 4.0]], 1.0), vec![0]);
        // Two points are both endpoints, even at a huge epsilon.
        assert_eq!(
            douglas_peucker_indices(&[[0.0, 0.0], [9.0, 9.0]], 1000.0),
            vec![0, 1]
        );
        // Tolerances that may not drop anything: zero, negative,
        // NaN and infinite all return the identity index list.
        let pts = zigzag();
        let all: Vec<usize> = (0..pts.len()).collect();
        for eps in [0.0, -1.5, f32::NAN, f32::INFINITY] {
            assert_eq!(douglas_peucker_indices(&pts, eps), all, "epsilon {eps}");
        }
    }

    #[test]
    fn indices_are_ascending_endpoints_included_and_match_simplify_points() {
        let cases: Vec<(Vec<V2>, f32)> = vec![
            (zigzag(), 7.0),
            (zigzag(), 1.0),
            (
                vec![
                    [0.0, 0.0],
                    [10.0, 0.4],
                    [20.0, -0.3],
                    [30.0, 0.2],
                    [40.0, 0.0],
                ],
                1.0,
            ),
            (vec![[0.0, 0.0], [20.0, 0.0], [20.0, 20.0]], 1.0),
            (
                vec![
                    [0.0, 0.0],
                    [5.0, 3.0],
                    [11.0, -2.0],
                    [17.0, 8.0],
                    [23.0, 1.0],
                    [29.0, -4.0],
                    [35.0, 0.0],
                ],
                2.5,
            ),
        ];
        for (pts, eps) in cases {
            let idx = douglas_peucker_indices(&pts, eps);
            assert_eq!(idx.first(), Some(&0), "first endpoint kept");
            assert_eq!(idx.last(), Some(&(pts.len() - 1)), "last endpoint kept");
            assert!(
                idx.windows(2).all(|w| w[0] < w[1]),
                "indices strictly ascending: {idx:?}"
            );
            // The gathered points are exactly what the point-only
            // simplifier returns — the index form loses nothing.
            let gathered: Vec<V2> = idx.iter().map(|&i| pts[i]).collect();
            assert_eq!(gathered, simplify_points(&pts, eps), "epsilon {eps}");
        }
        // The brief's zigzag at epsilon 7, pinned exactly.
        assert_eq!(douglas_peucker_indices(&zigzag(), 7.0), vec![0, 1, 4]);
        // A collinear run collapses to its endpoints.
        let line = vec![[0.0, 0.0], [10.0, 0.0], [20.0, 0.0], [30.0, 0.0]];
        assert_eq!(douglas_peucker_indices(&line, 0.5), vec![0, 3]);
    }

    #[test]
    fn zigzag_simplifies_with_widths_following_their_points_exactly() {
        let mut p = project_with(vec![pressured_zigzag(0)]);

        assert!(simplify_stroke_at(&mut p, 0, 7.0));

        let s = &p.strokes[0];
        assert_eq!(s.points, vec![[0.0, 0.0], [10.0, 10.0], [40.0, 0.0]]);
        // Kept indices are [0, 1, 4], so the widths are the
        // points' own original widths — exactly, not resampled.
        assert_eq!(s.widths, vec![2.0, 4.0, 10.0]);
        // Metadata untouched.
        assert_eq!(s.frame, 0);
        assert_eq!(s.color, 0xFF0000);
        assert_eq!(s.width, 4.0);
    }

    #[test]
    fn per_stroke_widths_are_index_exact_where_frame_wide_resamples() {
        // The frame-wide op resamples the profile by arc
        // position: on the same stroke its interior width lands
        // at ~4.47, between the original 4 and 6. The per-stroke
        // op keeps the surviving point's own 4.0. This pins the
        // documented difference between the two treatments.
        let mut p = project_with(vec![pressured_zigzag(0)]);
        assert_eq!(simplify_strokes(&mut p, 0, 7.0), 1);
        let resampled = p.strokes[0].widths[1];
        assert!(
            (resampled - 4.0).abs() > 0.1,
            "frame-wide simplify resamples widths (got {resampled})"
        );

        let mut p = project_with(vec![pressured_zigzag(0)]);
        assert!(simplify_stroke_at(&mut p, 0, 7.0));
        assert_eq!(p.strokes[0].widths[1], 4.0, "per-stroke keeps the exact width");
    }

    #[test]
    fn straight_line_collapses_to_its_endpoints() {
        let mut s = stroke(0, vec![[0.0, 0.0], [10.0, 0.0], [20.0, 0.0], [30.0, 0.0]]);
        s.widths = vec![1.0, 2.0, 3.0, 4.0];
        let mut p = project_with(vec![s]);

        assert!(simplify_stroke_at(&mut p, 0, 1.0));
        assert_eq!(p.strokes[0].points, vec![[0.0, 0.0], [30.0, 0.0]]);
        assert_eq!(p.strokes[0].widths, vec![1.0, 4.0]);
    }

    #[test]
    fn epsilon_zero_and_friends_change_nothing_and_report_false() {
        for eps in [0.0, -2.0, f32::NAN] {
            let mut p = project_with(vec![pressured_zigzag(0)]);
            let (pts, wds) = (p.strokes[0].points.clone(), p.strokes[0].widths.clone());
            assert!(!simplify_stroke_at(&mut p, 0, eps), "epsilon {eps}");
            assert_eq!(p.strokes[0].points, pts);
            assert_eq!(p.strokes[0].widths, wds);
        }
        // Nothing to lose also reports false: two points, one
        // point, and an already-simple corner at a fine epsilon.
        let mut p = project_with(vec![
            stroke(0, vec![[0.0, 0.0], [30.0, 0.0]]),
            stroke(0, vec![[5.0, 5.0]]),
            stroke(0, vec![[0.0, 0.0], [20.0, 0.0], [20.0, 20.0]]),
        ]);
        for i in 0..3 {
            assert!(!simplify_stroke_at(&mut p, i, 1.0), "stroke {i}");
        }
        assert_eq!(p.strokes[2].points.len(), 3);
    }

    #[test]
    fn bad_index_reports_false_and_touches_nothing() {
        let mut p = project_with(Vec::new());
        assert!(!simplify_stroke_at(&mut p, 0, 1.0), "empty project");

        let mut p = project_with(vec![pressured_zigzag(0)]);
        let before = p.strokes[0].points.clone();
        assert!(!simplify_stroke_at(&mut p, 1, 7.0));
        assert!(!simplify_stroke_at(&mut p, 99, 7.0));
        assert_eq!(p.strokes[0].points, before);
    }

    #[test]
    fn index_is_project_wide_not_frame_relative() {
        // Two collapsible strokes share frame 3; a third sits on
        // frame 0. The index names the position in
        // Project.strokes, whichever frame that stroke is on.
        let collapsible = |frame: usize| {
            stroke(
                frame,
                vec![
                    [0.0, 0.0],
                    [10.0, 0.4],
                    [20.0, -0.3],
                    [30.0, 0.2],
                    [40.0, 0.0],
                ],
            )
        };
        let mut p = project_with(vec![collapsible(3), collapsible(3), collapsible(0)]);

        assert!(simplify_stroke_at(&mut p, 1, 1.0));
        assert_eq!(p.strokes[0].points.len(), 5, "same-frame neighbour untouched");
        assert_eq!(p.strokes[1].points, vec![[0.0, 0.0], [40.0, 0.0]]);
        assert_eq!(p.strokes[2].points.len(), 5, "other frame untouched");

        assert!(simplify_stroke_at(&mut p, 2, 1.0));
        assert_eq!(p.strokes[2].points.len(), 2, "frame is no barrier");
    }

    #[test]
    fn uniform_and_broken_widths_are_left_exactly_as_found() {
        // Uniform form: empty widths stay empty.
        let mut s = stroke(0, zigzag());
        let mut p = project_with(vec![s.clone()]);
        assert!(simplify_stroke_at(&mut p, 0, 7.0));
        assert!(p.strokes[0].widths.is_empty());

        // Broken form: a non-empty, non-parallel vector defines
        // no point association, so it is left byte-for-byte as
        // found even though the points were reduced.
        s.widths = vec![1.0, 2.0];
        let mut p = project_with(vec![s]);
        assert!(simplify_stroke_at(&mut p, 0, 7.0));
        assert_eq!(p.strokes[0].points.len(), 3);
        assert_eq!(p.strokes[0].widths, vec![1.0, 2.0], "broken widths untouched");
    }

    #[test]
    fn spray_stroke_simplifies_its_base_polyline_and_keeps_the_mark() {
        let mut s = pressured_zigzag(0);
        s.spray = Some(SprayMark {
            seed: 7,
            radius: 12.0,
            opacity: 0.5,
        });
        let mut p = project_with(vec![s]);

        assert!(simplify_stroke_at(&mut p, 0, 7.0));
        let s = &p.strokes[0];
        assert_eq!(s.points, vec![[0.0, 0.0], [10.0, 10.0], [40.0, 0.0]]);
        assert_eq!(s.widths, vec![2.0, 4.0, 10.0]);
        assert_eq!(
            s.spray,
            Some(SprayMark {
                seed: 7,
                radius: 12.0,
                opacity: 0.5
            }),
            "the spray mark itself is untouched"
        );
    }
}
