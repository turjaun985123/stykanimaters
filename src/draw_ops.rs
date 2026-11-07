//! Project-level drawing operations: the whole-project callers
//! for the pure stroke geometry in [`crate::paint`]. Where
//! `paint.rs` works on bare point lists and stroke slices, this
//! module knows about [`Project`] and frame numbers, so the editor
//! (and any other frontend) can smooth, nib-profile and erase the
//! strokes of one frame without re-implementing the bookkeeping.
//!
//! Width conventions (shared with `model::Stroke`, the rasterizer
//! in `export.rs` and `paint::erase_at`): `Stroke.widths` is
//! either empty — the stroke has the uniform `Stroke.width` — or
//! exactly parallel to `Stroke.points`, one width per point. A
//! non-empty, non-parallel `widths` vector is meaningless to every
//! consumer (the rasterizer only honours the parallel form), so
//! the operations here never produce one and normalise it away
//! where they rewrite a stroke, matching `erase_at`'s behaviour.

use crate::model::{Project, Stroke, V2};
use crate::paint::{erase_at, nib_widths, smooth_path, Nib};

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
