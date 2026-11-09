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
//!
//! Snapshot discipline (Wave 13, B8 — the Wave 12 E8 census
//! applied to this file): the free functions here are pure
//! over `&mut Project` / `&mut Stroke` and NEVER snapshot —
//! `EditorCore::snapshot` is the only write that bumps the
//! edit count behind the dirty dot and the autosave cadence,
//! so a mutation without one is invisible to both. Every
//! caller therefore takes exactly one snapshot per user edit
//! BEFORE calling in: the `tool_*` handlers below do (or
//! delegate to an `EditorCore` op that does), and the
//! editor's own gesture compositions do (one snapshot at
//! eraser-drag start covers every `erase` call in the drag;
//! smoothing/nib finalisation rides the drawn stroke's own
//! step). A caller that skips the snapshot ships an
//! un-undoable, never-dirty edit — the failure this census
//! exists to catch. The session tools at the bottom
//! (`tool_set_brush`, `tool_cycle_brush`, `tool_set_draw_tool`)
//! write editor session state only (`brush_index`,
//! `draw_tool`), which is not part of the saved project, so
//! they correctly take no snapshot and mark nothing dirty;
//! the `dirty_contract_tests` module pins every one of these
//! verdicts against `undo_depth` / `edits_since_autosave`.
//! The Wave 14 parity fill below extends the same split:
//! its session tools (`tool_get_brush`, `tool_set_draw_mode`,
//! `tool_step_brush_param`, `tool_set_brush_nib`) write only
//! `brush_index` / `draw_mode` or nothing, and its mutating
//! tools snapshot exactly once — and only when a change
//! actually happens.

use crate::editor::EditorCore;
use crate::model::{Project, Stroke, V2};
use crate::paint::{erase_at, mirror_points_x, nib_widths, simplify_points, smooth_path, Nib};
use serde_json::{json, Value};

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
///
/// CALLER MUST SNAPSHOT FIRST (Wave 13, B8): like every free
/// function in this module, this takes no snapshot, so the
/// caller owns the one undo step / dirty mark for the edit
/// (see the module docs). This warning sits on this function
/// in particular because a frame-wide nib rewrite is a large,
/// silent project mutation when the step is skipped.
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


// ---------------------------------------------------------------------
// MCP handlers (Wave 11, B3): the drawing domain's tool surface,
// written against `EditorCore` with the registry handlers'
// standard signature so `mcp_tools::tools()` can register them
// directly (`handler: crate::draw_ops::tool_*`).
//
// The argument helpers are LOCAL to this section (`dops_`
// prefix) because `mcp_tools`' helpers are private to that
// file. They differ from those helpers in one deliberate way:
// every number is checked `is_finite` BEFORE and AFTER the
// f64 -> f32 cast. A JSON text cannot write NaN literally, but
// an overflowing literal (for example `1e999`) can surface as
// an infinite f64, and the plain `as f32` cast would carry
// infinity (or a finite-but-huge f64) into the project, where
// the renderers' dab/spray geometry turns it into NaN
// positions. A non-finite argument is refused here instead.
// ---------------------------------------------------------------------

fn dops_get<'a>(args: &'a Value, key: &str) -> Option<&'a Value> {
    args.get(key).filter(|v| !v.is_null())
}

fn dops_opt_f32(args: &Value, key: &str) -> Result<Option<f32>, String> {
    match dops_get(args, key) {
        Some(v) => {
            let raw = v
                .as_f64()
                .ok_or_else(|| format!("argument '{key}' must be a number"))?;
            let f = raw as f32;
            if !raw.is_finite() || !f.is_finite() {
                return Err(format!("argument '{key}' must be a finite number"));
            }
            Ok(Some(f))
        }
        None => Ok(None),
    }
}

fn dops_req_f32(args: &Value, key: &str) -> Result<f32, String> {
    dops_opt_f32(args, key)?.ok_or_else(|| format!("missing required argument '{key}'"))
}

fn dops_as_usize(v: &Value, key: &str) -> Result<usize, String> {
    if let Some(u) = v.as_u64() {
        return usize::try_from(u).map_err(|_| format!("argument '{key}' is too large"));
    }
    if let Some(f) = v.as_f64() {
        if f >= 0.0 && f.fract() == 0.0 && f <= usize::MAX as f64 {
            return Ok(f as usize);
        }
    }
    Err(format!("argument '{key}' must be a non-negative integer"))
}

fn dops_opt_usize(args: &Value, key: &str) -> Result<Option<usize>, String> {
    match dops_get(args, key) {
        Some(v) => dops_as_usize(v, key).map(Some),
        None => Ok(None),
    }
}

fn dops_req_usize(args: &Value, key: &str) -> Result<usize, String> {
    dops_opt_usize(args, key)?.ok_or_else(|| format!("missing required argument '{key}'"))
}

/// A signed small integer (`direction`, `subdiv`): a JSON
/// integer, or a float with no fractional part.
fn dops_opt_i32(args: &Value, key: &str) -> Result<Option<i32>, String> {
    match dops_get(args, key) {
        Some(v) => {
            if let Some(i) = v.as_i64() {
                return i32::try_from(i)
                    .map(Some)
                    .map_err(|_| format!("argument '{key}' is too large"));
            }
            if let Some(f) = v.as_f64() {
                if f.is_finite() && f.fract() == 0.0 && f >= i32::MIN as f64 && f <= i32::MAX as f64 {
                    return Ok(Some(f as i32));
                }
            }
            Err(format!("argument '{key}' must be an integer"))
        }
        None => Ok(None),
    }
}

fn dops_opt_bool(args: &Value, key: &str) -> Result<Option<bool>, String> {
    match dops_get(args, key) {
        Some(v) => v
            .as_bool()
            .map(Some)
            .ok_or_else(|| format!("argument '{key}' must be a boolean")),
        None => Ok(None),
    }
}

fn dops_opt_str<'a>(args: &'a Value, key: &str) -> Result<Option<&'a str>, String> {
    match dops_get(args, key) {
        Some(v) => v
            .as_str()
            .map(Some)
            .ok_or_else(|| format!("argument '{key}' must be a string")),
        None => Ok(None),
    }
}

/// Colours travel as `0xRRGGBB` integers, as in `mcp_tools`.
fn dops_opt_color(args: &Value, key: &str) -> Result<Option<u32>, String> {
    match dops_get(args, key) {
        Some(v) => {
            let n = dops_as_usize(v, key)?;
            u32::try_from(n)
                .map(Some)
                .map_err(|_| format!("argument '{key}' is too large for a colour"))
        }
        None => Ok(None),
    }
}

fn dops_check_frame(core: &EditorCore, frame: usize) -> Result<(), String> {
    if frame < core.project.frames.len() {
        Ok(())
    } else {
        Err(format!(
            "frame index {frame} out of range (project has {} frame(s))",
            core.project.frames.len()
        ))
    }
}

/// A point list with every coordinate finite-checked, the
/// hardened counterpart of `mcp_tools::req_points`.
fn dops_req_points(args: &Value, key: &str) -> Result<Vec<V2>, String> {
    let v = dops_get(args, key).ok_or_else(|| format!("missing required argument '{key}'"))?;
    let arr = v
        .as_array()
        .ok_or_else(|| format!("argument '{key}' must be an array of [x, y] points"))?;
    let mut points = Vec::with_capacity(arr.len());
    for (i, p) in arr.iter().enumerate() {
        let pair = p
            .as_array()
            .filter(|a| a.len() == 2)
            .ok_or_else(|| format!("argument '{key}[{i}]' must be an [x, y] pair"))?;
        let mut pt = [0.0f32; 2];
        for (axis, slot) in pt.iter_mut().enumerate() {
            let raw = pair[axis]
                .as_f64()
                .ok_or_else(|| format!("argument '{key}[{i}][{axis}]' must be a number"))?;
            let f = raw as f32;
            if !raw.is_finite() || !f.is_finite() {
                return Err(format!(
                    "argument '{key}[{i}][{axis}]' must be a finite number"
                ));
            }
            *slot = f;
        }
        points.push(pt);
    }
    Ok(points)
}

/// One stroke as JSON: every stored field, so an MCP client
/// can see exactly what the panel's stroke list shows (and
/// what `set_stroke_style` can change). `points` carries the
/// full polyline only when `include_points` is set; otherwise
/// the count alone travels, under `points`.
fn dops_stroke_json(stroke: &Stroke, index: usize, include_points: bool) -> Value {
    let points_json = if include_points {
        json!(stroke.points)
    } else {
        json!(stroke.points.len())
    };
    let spray_json = stroke.spray.map(|m| {
        json!({
            "seed": m.seed,
            "radius": m.radius,
            "opacity": m.opacity,
        })
    });
    json!({
        "index": index,
        "frame": stroke.frame,
        "points": points_json,
        "color": stroke.color,
        "width": stroke.width,
        "widths": stroke.widths.len(),
        "fill": stroke.fill,
        "spray": spray_json,
        "opacity": stroke.opacity,
        "scatter": stroke.scatter,
    })
}

/// Read parity for the panel's stroke list (the shell's
/// `stroke_inspector::stroke_rows`): without this, MCP could
/// write strokes but never enumerate them — `delete_stroke`'s
/// index space was undiscoverable. Read-only: no snapshot,
/// no status change.
pub fn tool_list_strokes(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let frame = dops_opt_usize(args, "frame")?;
    if let Some(f) = frame {
        dops_check_frame(core, f)?;
    }
    let include_points = dops_opt_bool(args, "points")?.unwrap_or(false);
    let strokes: Vec<Value> = core
        .project
        .strokes
        .iter()
        .enumerate()
        .filter(|(_, s)| frame.is_none() || frame == Some(s.frame))
        .map(|(i, s)| dops_stroke_json(s, i, include_points))
        .collect();
    Ok(json!({
        "ok": true,
        "count": strokes.len(),
        "total": core.project.strokes.len(),
        "strokes": strokes,
    }))
}

/// The Draw panel's readout line (`draw_tools::drawing_stats`)
/// as data: totals over every stroke in the project.
pub fn tool_get_drawing_stats(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    let mut points = 0usize;
    let mut total_length = 0.0f32;
    let mut spray_strokes = 0usize;
    for stroke in &core.project.strokes {
        points += stroke.points.len();
        if stroke.spray.is_some() {
            spray_strokes += 1;
        }
        for pair in stroke.points.windows(2) {
            let dx = pair[1][0] - pair[0][0];
            let dy = pair[1][1] - pair[0][1];
            total_length += (dx * dx + dy * dy).sqrt();
        }
    }
    Ok(json!({
        "ok": true,
        "strokes": core.project.strokes.len(),
        "points": points,
        "total_length": total_length,
        "spray_strokes": spray_strokes,
        "strokes_on_current_frame": stroke_count(&core.project, core.current),
        "current_frame": core.current,
    }))
}

/// Hardened replacement for `mcp_tools`' `draw_stroke`
/// handler (same tool name and response shape; the integrator
/// swaps the handler and merges the `opacity` property into
/// the existing schema). Differences from the current
/// handler, each deliberate:
///
/// * every number is finite-checked (`dops_req_points` /
///   `dops_opt_f32`) — the current handler casts unchecked,
///   and its `scatter < 0.0` test alone passes both NaN and
///   +infinity through into the project;
/// * `opacity` is settable (the current handler hard-codes
///   1.0, so MCP could never record the translucent stroke
///   the Pencil preset draws);
/// * the append is one undo step (`core.snapshot()`), like
///   the editor's own stamps — the current handler's strokes
///   cannot be undone.
pub fn tool_draw_stroke(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let frame = dops_req_usize(args, "frame")?;
    dops_check_frame(core, frame)?;
    let points = dops_req_points(args, "points")?;
    if points.is_empty() {
        return Err("argument 'points' must contain at least one point".into());
    }
    // Defaults mirror the editor's own pen: light grey, width 4.
    let color = dops_opt_color(args, "color")?.unwrap_or(0xE8E8EC);
    let width = dops_opt_f32(args, "width")?.unwrap_or(4.0);
    if width <= 0.0 {
        return Err("argument 'width' must be positive".into());
    }
    // Dab scatter in stage units (Wave 10): 0 (the default)
    // draws a clean line; a positive radius jitters the
    // stroke's dabs via paint::stroke_dabs in every renderer.
    // Negative values are refused — the model treats them as
    // "no scatter", and accepting one would record a lie.
    let scatter = dops_opt_f32(args, "scatter")?.unwrap_or(0.0);
    if scatter < 0.0 {
        return Err("argument 'scatter' must be zero or positive".into());
    }
    let opacity = dops_opt_f32(args, "opacity")?.unwrap_or(1.0);
    if !(0.0..=1.0).contains(&opacity) {
        return Err("argument 'opacity' must be between 0 and 1".into());
    }
    let n_points = points.len();
    let index = core.project.strokes.len();
    core.snapshot();
    core.project.strokes.push(Stroke {
        frame,
        points,
        color,
        width,
        widths: Vec::new(),
        fill: None,
        spray: None,
        opacity,
        scatter,
    });
    Ok(json!({
        "ok": true,
        "index": index,
        "frame": frame,
        "points": n_points,
        "strokes_on_frame": stroke_count(&core.project, frame),
    }))
}

/// The Draw panel's Arrow stamp (`draw_tools`' `stamp-arrow`
/// button, `EditorCore::stamp_arrow`) had no MCP counterpart —
/// `draw_shape` / `draw_polygon` / `draw_star` cover the other
/// stamps. Geometry is `paint::arrow_points`, the generator
/// both editor paths use. One undo step, like a stamp.
pub fn tool_draw_arrow(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let frame = dops_opt_usize(args, "frame")?.unwrap_or(core.current);
    dops_check_frame(core, frame)?;
    let x1 = dops_req_f32(args, "x1")?;
    let y1 = dops_req_f32(args, "y1")?;
    let x2 = dops_req_f32(args, "x2")?;
    let y2 = dops_req_f32(args, "y2")?;
    if [x1, y1] == [x2, y2] {
        return Err("arrow tail and tip must differ (a zero-length arrow has no direction)".into());
    }
    // The editor's stamp head length (editor.rs `stamp_arrow`).
    let head = dops_opt_f32(args, "head")?.unwrap_or(22.0);
    if head <= 0.0 {
        return Err("argument 'head' must be positive".into());
    }
    let color = dops_opt_color(args, "color")?.unwrap_or(0xE8E8EC);
    let width = dops_opt_f32(args, "width")?.unwrap_or_else(|| core.active_brush().width);
    if width <= 0.0 {
        return Err("argument 'width' must be positive".into());
    }
    let points = crate::paint::arrow_points([x1, y1], [x2, y2], head);
    let n_points = points.len();
    let index = core.project.strokes.len();
    core.snapshot();
    core.project.strokes.push(Stroke {
        frame,
        points,
        color,
        width,
        widths: Vec::new(),
        fill: None,
        spray: None,
        opacity: 1.0,
        scatter: 0.0,
    });
    Ok(json!({
        "ok": true,
        "index": index,
        "frame": frame,
        "points": n_points,
        "strokes_on_frame": stroke_count(&core.project, frame),
    }))
}

/// Per-stroke style editing: the absolute-value counterpart
/// of the stroke panel's Width / Opacity steppers
/// (`stroke_inspector::set_selected_stroke_width` /
/// `set_selected_stroke_opacity`), extended to the fields the
/// panel exposes only at draw time (`color`, `scatter`,
/// `fill`). Panel discipline is kept:
///
/// * width must lie in the panel's stepper range
///   [0.5, 40] (`MIN_STROKE_WIDTH` / `MAX_STROKE_WIDTH`
///   there), and a per-point `widths` taper is scaled by the
///   same new/old ratio the panel's press applies;
/// * a spray stroke's visible opacity lives on its spray
///   mark and its rendering never reaches `stroke_dabs`, so
///   `opacity` and `scatter` writes to a spray stroke are
///   refused instead of recorded as lies;
/// * exactly one snapshot — one undo step — and only when
///   every argument validated.
pub fn tool_set_stroke_style(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let index = dops_req_usize(args, "index")?;
    let Some(stroke) = core.project.strokes.get(index) else {
        return Err(format!("no stroke at index {index}"));
    };
    let is_spray = stroke.spray.is_some();
    let color = dops_opt_color(args, "color")?;
    let width = dops_opt_f32(args, "width")?;
    let opacity = dops_opt_f32(args, "opacity")?;
    let scatter = dops_opt_f32(args, "scatter")?;
    let fill = dops_opt_color(args, "fill")?;
    let clear_fill = dops_opt_bool(args, "clear_fill")?.unwrap_or(false);
    if color.is_none()
        && width.is_none()
        && opacity.is_none()
        && scatter.is_none()
        && fill.is_none()
        && !clear_fill
    {
        return Err(
            "nothing to set: pass at least one of 'color', 'width', 'opacity', \
             'scatter', 'fill' or 'clear_fill'"
                .into(),
        );
    }
    if fill.is_some() && clear_fill {
        return Err("pass either 'fill' or 'clear_fill', not both".into());
    }
    if let Some(w) = width {
        if !(0.5..=40.0).contains(&w) {
            return Err(
                "argument 'width' must be between 0.5 and 40 (the stroke panel's range)".into(),
            );
        }
    }
    if let Some(o) = opacity {
        if !(0.0..=1.0).contains(&o) {
            return Err("argument 'opacity' must be between 0 and 1".into());
        }
        if is_spray {
            return Err(format!(
                "stroke {index} is a spray stroke: its opacity lives on its spray \
                 mark, not on the stroke — refusing to record a value nothing renders"
            ));
        }
    }
    if let Some(sc) = scatter {
        if sc < 0.0 {
            return Err("argument 'scatter' must be zero or positive".into());
        }
        if is_spray {
            return Err(format!(
                "stroke {index} is a spray stroke: it renders from its spray mark \
                 and never reaches the scatter dab path — refusing to record a \
                 value nothing renders"
            ));
        }
    }
    let old_width = core.project.strokes[index].width;
    core.snapshot();
    let stroke = &mut core.project.strokes[index];
    if let Some(c) = color {
        stroke.color = c;
    }
    if let Some(w) = width {
        stroke.width = w;
        // The panel's taper rule: a parallel per-point profile
        // scales by the same ratio, preserving its shape; the
        // scaling is skipped when the old width cannot form a
        // ratio (zero, negative or non-finite in a loaded file).
        if old_width.is_finite() && old_width > 0.0 && !stroke.widths.is_empty() {
            let scale = w / old_width;
            for entry in &mut stroke.widths {
                *entry *= scale;
            }
        }
    }
    if let Some(o) = opacity {
        stroke.opacity = o;
    }
    if let Some(sc) = scatter {
        stroke.scatter = sc;
    }
    if let Some(f) = fill {
        stroke.fill = Some(f);
    }
    if clear_fill {
        stroke.fill = None;
    }
    core.status = format!("Stroke {} style updated.", index + 1);
    let stroke = &core.project.strokes[index];
    Ok(json!({
        "ok": true,
        "status": core.status,
        "stroke": dops_stroke_json(stroke, index, false),
    }))
}

/// Single-stroke simplify: the stroke panel's Simplify press
/// (`stroke_inspector::simplify_selected`, epsilon 2.0 there)
/// for one named stroke — MCP previously had only the
/// frame-wide `simplify_strokes`. Honesty rule from the
/// panel: when the stroke is already simple at the epsilon,
/// NO snapshot is taken, so the undo history never gains a
/// step for a change that does not exist.
pub fn tool_simplify_stroke(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let index = dops_req_usize(args, "index")?;
    let epsilon = dops_opt_f32(args, "epsilon")?.unwrap_or(2.0);
    if epsilon <= 0.0 {
        return Err("argument 'epsilon' must be positive".into());
    }
    let Some(stroke) = core.project.strokes.get(index) else {
        return Err(format!("no stroke at index {index}"));
    };
    let before = stroke.points.len();
    let kept = douglas_peucker_indices(&stroke.points, epsilon);
    if kept.len() >= before {
        core.status = format!("Stroke {} is already simple.", index + 1);
        return Ok(json!({
            "ok": true,
            "simplified": false,
            "index": index,
            "points": before,
            "status": core.status,
        }));
    }
    core.snapshot();
    simplify_stroke_at(&mut core.project, index, epsilon);
    let after = core.project.strokes[index].points.len();
    core.status = format!("Stroke {} simplified to {} points.", index + 1, after);
    Ok(json!({
        "ok": true,
        "simplified": true,
        "index": index,
        "points_before": before,
        "points_after": after,
        "status": core.status,
    }))
}

/// Fill the most recent stroke on the current frame
/// (`EditorCore::fill_current_stroke`): sets its `fill` to
/// its own colour and appends the scanline fill as a
/// companion stroke. The op reports refusal on its status
/// line instead of returning a result; a "Nothing to fill"
/// status is surfaced as the tool error it is.
pub fn tool_fill_stroke(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    core.fill_current_stroke();
    if core.status.starts_with("Nothing to fill") {
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "status": core.status,
        "frame": core.current,
        "strokes_on_frame": stroke_count(&core.project, core.current),
        "count": core.project.strokes.len(),
    }))
}

/// Erase the current frame's strokes lying mostly inside a
/// lasso polygon (`EditorCore::lasso_erase`). The op had NO
/// caller anywhere — no shell surface and no MCP tool (the
/// stroke inspector's module docs record exactly that) — so
/// this tool is its first reachable surface. The op
/// snapshots itself when it removes something; its refusal
/// statuses surface as errors.
pub fn tool_lasso_erase(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let poly = dops_req_points(args, "points")?;
    if poly.len() < 3 {
        return Err("argument 'points' must contain at least 3 points (a lasso polygon)".into());
    }
    let before = core.project.strokes.len();
    core.lasso_erase(&poly);
    if core.status.starts_with("Lasso needs") || core.status.starts_with("Lasso erased nothing") {
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "status": core.status,
        "frame": core.current,
        "removed": before - core.project.strokes.len(),
        "count": core.project.strokes.len(),
    }))
}

/// Mirror the current frame's strokes across the stage's
/// vertical centre — the Draw panel's Mirror strokes button
/// (`EditorCore::mirror_strokes_x`, one undo step taken by
/// the op itself). An empty frame is an error, not a
/// silent success.
pub fn tool_mirror_strokes(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    core.mirror_strokes_x();
    if core.status.starts_with("No strokes to mirror") {
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "status": core.status,
        "frame": core.current,
        "strokes_on_frame": stroke_count(&core.project, core.current),
    }))
}

/// Clear the current frame's strokes — the stroke list
/// header's Clear button (`EditorCore::clear_frame_drawings`).
/// The core op takes no snapshot, so this handler takes the
/// one undo step itself, and only when the frame actually
/// carries strokes (the panel bulk ops' discipline: no undo
/// step for a change that does not exist).
pub fn tool_clear_frame_strokes(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let frame = dops_opt_usize(args, "frame")?.unwrap_or(core.current);
    dops_check_frame(core, frame)?;
    if frame != core.current {
        return Err(format!(
            "clear_frame_strokes acts on the current frame (frame {}); \
             move the playhead to frame {} first",
            core.current + 1,
            frame + 1
        ));
    }
    let n = stroke_count(&core.project, frame);
    // Wave 13 (census A17): the core op owns the undo step
    // now (it snapshots when the frame carries strokes) —
    // a handler snapshot here would count two edits for
    // one action.
    core.clear_frame_drawings();
    Ok(json!({
        "ok": true,
        "status": core.status,
        "frame": frame,
        "cleared": n,
        "count": core.project.strokes.len(),
    }))
}

/// Delete every stroke in the project — the stroke panel's
/// bulk Delete all press, whose composition
/// (`stroke_inspector::delete_all_strokes`) is one snapshot
/// and a clear; looping `delete_stroke` over MCP would cost
/// one undo step (and one round trip) per stroke.
pub fn tool_delete_all_strokes(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    let n = core.project.strokes.len();
    if n == 0 {
        core.status = "No strokes to delete.".into();
        return Ok(json!({
            "ok": true,
            "deleted": 0,
            "status": core.status,
        }));
    }
    core.snapshot();
    core.project.strokes.clear();
    core.status = format!("Deleted {n} stroke{}.", if n == 1 { "" } else { "s" });
    Ok(json!({
        "ok": true,
        "deleted": n,
        "status": core.status,
    }))
}

/// Recolour strokes to an explicit colour — the stroke
/// panel's bulk Recolour / Frame Recolour presses, except
/// that the panel's target colour is implicit (the selected
/// figure's colour) while an MCP caller has no selection to
/// imply it from, so the colour is a required argument.
/// `scope` is "frame" (the current frame, the default) or
/// "all". One snapshot, taken only when strokes match.
pub fn tool_recolor_strokes(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let color = dops_opt_color(args, "color")?
        .ok_or_else(|| "missing required argument 'color'".to_string())?;
    let scope = dops_opt_str(args, "scope")?.unwrap_or("frame");
    if scope != "frame" && scope != "all" {
        return Err(format!(
            "unknown scope '{scope}' (expected \"frame\" or \"all\")"
        ));
    }
    let frame = core.current;
    let targets: Vec<usize> = core
        .project
        .strokes
        .iter()
        .enumerate()
        .filter(|(_, s)| scope == "all" || s.frame == frame)
        .map(|(i, _)| i)
        .collect();
    let n = targets.len();
    if n == 0 {
        core.status = if scope == "all" {
            "No strokes to recolour.".into()
        } else {
            format!("No strokes on frame {} to recolour.", frame + 1)
        };
        return Ok(json!({
            "ok": true,
            "recolored": 0,
            "scope": scope,
            "status": core.status,
        }));
    }
    core.snapshot();
    for i in targets {
        core.project.strokes[i].color = color;
    }
    core.status = if scope == "all" {
        format!("Recoloured {n} stroke{}.", if n == 1 { "" } else { "s" })
    } else {
        format!(
            "Recoloured {n} stroke{} on frame {}.",
            if n == 1 { "" } else { "s" },
            frame + 1
        )
    };
    Ok(json!({
        "ok": true,
        "recolored": n,
        "scope": scope,
        "status": core.status,
    }))
}

/// The brush preset library (`paint::brush_library`) as
/// data, with the active selection — the read side of the
/// Draw panel's brush chips. Read-only.
pub fn tool_list_brushes(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    let lib = crate::paint::brush_library();
    let brushes: Vec<Value> = lib
        .iter()
        .enumerate()
        .map(|(i, b)| {
            json!({
                "index": i,
                "name": b.name,
                "width": b.width,
                "nib": match b.nib {
                    crate::paint::Nib::Round => "round",
                    crate::paint::Nib::Square => "square",
                    crate::paint::Nib::Calligraphy => "calligraphy",
                },
                "nib_angle": b.nib_angle,
                "smoothing": b.smoothing,
                "opacity": b.opacity,
                "scatter": b.scatter,
            })
        })
        .collect();
    let active = core.active_brush();
    Ok(json!({
        "ok": true,
        "count": brushes.len(),
        "brushes": brushes,
        "active_index": core.brush_index.min(lib.len().saturating_sub(1)),
        "active_name": active.name,
    }))
}

/// Select the active brush preset by `name` (exact, then
/// case-insensitive) or `index` — the Draw panel's brush
/// chips. The write is the panel's own: `brush_index` plus
/// the status string `cycle_brush` sets. Selection only —
/// nothing is drawn.
pub fn tool_set_brush(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let lib = crate::paint::brush_library();
    let index = if let Some(name) = dops_opt_str(args, "name")? {
        lib.iter()
            .position(|b| b.name == name)
            .or_else(|| lib.iter().position(|b| b.name.eq_ignore_ascii_case(name)))
            .ok_or_else(|| {
                let names: Vec<&str> = lib.iter().map(|b| b.name).collect();
                format!("unknown brush '{name}' (library: {})", names.join(", "))
            })?
    } else if let Some(i) = dops_opt_usize(args, "index")? {
        if i >= lib.len() {
            return Err(format!(
                "brush index {i} out of range (library has {} brushes)",
                lib.len()
            ));
        }
        i
    } else {
        return Err("pass 'name' or 'index' to select a brush".into());
    };
    core.brush_index = index;
    let b = lib[index];
    core.status = format!("Brush: {} (width {:.0}).", b.name, b.width);
    Ok(json!({
        "ok": true,
        "index": index,
        "name": b.name,
        "status": core.status,
    }))
}

/// Cycle to the next brush preset, wrapping — the Draw
/// panel's brush-cycle button (`EditorCore::cycle_brush`).
pub fn tool_cycle_brush(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    core.cycle_brush();
    Ok(json!({
        "ok": true,
        "index": core.brush_index,
        "name": core.active_brush().name,
        "status": core.status,
    }))
}

/// Set the active draw tool — the Draw panel's tool toggle
/// (`EditorCore::toggle_draw_tool` flips; this sets, so an
/// MCP caller can state the tool it wants idempotently).
/// Affects subsequent interactive drawing and the eraser's
/// sizing only; it draws nothing itself.
pub fn tool_set_draw_tool(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let tool = dops_opt_str(args, "tool")?
        .ok_or_else(|| "missing required argument 'tool'".to_string())?;
    match tool {
        "brush" => {
            core.draw_tool = crate::editor::DrawTool::Brush;
            core.status = format!("Draw tool: Brush ({}).", core.active_brush().name);
        }
        "eraser" => {
            core.draw_tool = crate::editor::DrawTool::Eraser;
            core.status = "Draw tool: Eraser.".into();
        }
        other => {
            return Err(format!(
                "unknown draw tool '{other}' (expected \"brush\" or \"eraser\")"
            ));
        }
    }
    Ok(json!({
        "ok": true,
        "tool": tool,
        "status": core.status,
    }))
}

// ---------------------------------------------------------------------
// MCP handlers (Wave 14, A3): the draw/stroke parity fill —
// every draw capability the Wave 11/12 surface left without
// a tool, each written against the same discipline as the
// handlers above (validate first, finiteness, refusals quote
// the offending index/value, exactly one undo step per
// mutation, no step for a change that does not exist).
// ---------------------------------------------------------------------

/// The active brush preset in full, plus the draw-session
/// facts the Draw panel's readouts show (Phase J/K and the
/// Wave 10 scatter readout in `draw_tools`, and the stroke
/// inspector's preset header): preset index / name / width /
/// nib / nib angle / smoothing / opacity / scatter, the
/// active draw tool, the eraser radius the core derives
/// from the preset (`active width × 2`, editor.rs
/// `eraser_radius`) and the draw-mode flag `get_state`
/// already exposes. `list_brushes` carries the same preset
/// fields buried in the library array; this is the direct
/// read of the one preset that is active. Read-only.
///
// TOOL ENTRY SPEC: for the integrator's registry splice
// in `mcp_tools::tools()`, beside the other
// `crate::draw_ops` entries (after `list_brushes`):
//
//   Tool {
//       name: "get_brush",
//       description: "Read the ACTIVE brush preset in full (the Draw panel's preset readouts, as data): index, name, width, nib, nib angle, smoothing, opacity and scatter, plus the active draw tool ('brush' or 'eraser'), the eraser radius the preset sizes (twice its width) and the draw-mode flag. Read-only.",
//       schema: obj_schema(json!({}), &[]),
//       handler: crate::draw_ops::tool_get_brush,
//   }
//
// MCP.md ROW SPEC: for the draw-tools table, after the
// `list_brushes` row:
//
//   | `get_brush` | Read the ACTIVE brush preset in full (the Draw panel's preset readouts, as data): index, name, width, nib, nib angle, smoothing, opacity and scatter, plus the active draw tool, the eraser radius the preset sizes (twice its width) and the draw-mode flag. Read-only. | — | — |
pub fn tool_get_brush(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    let lib = crate::paint::brush_library();
    let index = core.brush_index.min(lib.len().saturating_sub(1));
    let b = core.active_brush();
    Ok(json!({
        "ok": true,
        "index": index,
        "name": b.name,
        "width": b.width,
        "nib": match b.nib {
            crate::paint::Nib::Round => "round",
            crate::paint::Nib::Square => "square",
            crate::paint::Nib::Calligraphy => "calligraphy",
        },
        "nib_angle": b.nib_angle,
        "smoothing": b.smoothing,
        "opacity": b.opacity,
        "scatter": b.scatter,
        "tool": if core.draw_tool_is_eraser() { "eraser" } else { "brush" },
        "eraser_radius": b.width * 2.0,
        "draw_mode": core.draw_mode,
    }))
}

/// Set the draw-mode flag — the Draw panel's Draw toggle
/// and the palette's `draw-mode` command both flip the
/// public `EditorCore::draw_mode` field (there is no op);
/// this is the idempotent set form, like `tool_set_draw_tool`
/// beside it, with the panel button's own status strings.
/// Session state only: no snapshot, no dirty mark (the
/// session family `tool_set_brush` belongs to).
///
// TOOL ENTRY SPEC: for the integrator's registry splice
// in `mcp_tools::tools()`, beside the other
// `crate::draw_ops` entries (after `set_draw_tool`):
//
//   Tool {
//       name: "set_draw_mode",
//       description: "Set draw mode on or off (the Draw panel's Draw toggle / the palette's draw-mode command, as an idempotent set): in draw mode, dragging on the canvas paints a stroke on the current frame instead of posing. Session state only — not an undo step.",
//       schema: obj_schema(
//           json!({
//               "enabled": { "type": "boolean" }
//           }),
//           &["enabled"],
//       ),
//       handler: crate::draw_ops::tool_set_draw_mode,
//   }
//
// MCP.md ROW SPEC: for the draw-tools table, after the
// `set_draw_tool` row:
//
//   | `set_draw_mode` | Set draw mode on or off (the Draw panel's Draw toggle / the palette's draw-mode command, as an idempotent set): in draw mode, dragging on the canvas paints a stroke on the current frame instead of posing. Session state only — not an undo step. | `enabled` | — |
pub fn tool_set_draw_mode(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let enabled = dops_opt_bool(args, "enabled")?
        .ok_or_else(|| "missing required argument 'enabled'".to_string())?;
    core.draw_mode = enabled;
    core.status = if enabled {
        "Draw mode: drag on the canvas to paint a stroke on this frame.".into()
    } else {
        "Pose mode.".into()
    };
    Ok(json!({
        "ok": true,
        "draw_mode": core.draw_mode,
        "status": core.status,
    }))
}

/// One brush parameter a preset stepper can step, in the
/// stroke inspector's own vocabulary (`Param` there): the
/// preset-bundle fields of `paint::Brush`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum BrushParam {
    Size,
    Opacity,
    Scatter,
    Smoothing,
}

impl BrushParam {
    fn value(self, b: &crate::paint::Brush) -> f32 {
        match self {
            BrushParam::Size => b.width,
            BrushParam::Opacity => b.opacity,
            BrushParam::Scatter => b.scatter,
            BrushParam::Smoothing => b.smoothing as f32,
        }
    }

    /// The inspector's status text when no preset steps in
    /// direction `dir` (copied from `Param::no_step_message`).
    fn no_step_message(self, dir: i32) -> &'static str {
        match (self, dir > 0) {
            (BrushParam::Size, true) => "No wider brush in the library.",
            (BrushParam::Size, false) => "No narrower brush in the library.",
            (BrushParam::Opacity, true) => "No more opaque brush in the library.",
            (BrushParam::Opacity, false) => "No fainter brush in the library.",
            (BrushParam::Scatter, true) => "No broader-scatter brush in the library.",
            (BrushParam::Scatter, false) => "No tighter-scatter brush in the library.",
            (BrushParam::Smoothing, true) => "No smoother brush in the library.",
            (BrushParam::Smoothing, false) => "No rougher brush in the library.",
        }
    }
}

/// Step the active brush selection by one preset parameter
/// — the stroke inspector's Preset size / opacity / scatter /
/// smoothing steppers (`stroke_inspector::step_param`):
/// select the nearest library preset whose value for that
/// parameter is strictly higher (`direction` 1) or lower
/// (−1), smallest difference winning, ties to the lowest
/// library index. The inspector lives in the shell crate, so
/// its stepping rule is re-stated here against
/// `paint::brush_library` rather than called. When no preset
/// steps that way the selection stays put and the reply
/// reports `changed: false` with the panel's own status
/// text — a no-op is not an error, exactly as on the panel.
/// Session state only: no snapshot, no dirty mark.
///
// TOOL ENTRY SPEC: for the integrator's registry splice
// in `mcp_tools::tools()`, beside the other
// `crate::draw_ops` entries (after `cycle_brush`):
//
//   Tool {
//       name: "step_brush_param",
//       description: "Step the active brush selection by one preset parameter (the stroke inspector's Preset size / opacity / scatter / smoothing steppers): selects the nearest library preset whose value for 'param' is strictly higher ('direction' 1) or lower (-1). Because presets are bundles the other parameters follow the chosen preset. Reports changed: false, with the panel's status text, when no preset steps that way. Session state only — not an undo step.",
//       schema: obj_schema(
//           json!({
//               "param": { "type": "string", "enum": ["size", "width", "opacity", "scatter", "smoothing"] },
//               "direction": { "type": "integer", "description": "1 to step up, -1 to step down" }
//           }),
//           &["param", "direction"],
//       ),
//       handler: crate::draw_ops::tool_step_brush_param,
//   }
//
// MCP.md ROW SPEC: for the draw-tools table, after the
// `cycle_brush` row:
//
//   | `step_brush_param` | Step the active brush selection by one preset parameter (the stroke inspector's Preset size / opacity / scatter / smoothing steppers): selects the nearest library preset whose value for 'param' is strictly higher ('direction' 1) or lower (-1); the other parameters follow the chosen preset, because presets are bundles. Reports changed: false, with the panel's status text, when no preset steps that way. Session state only — not an undo step. | `param`, `direction` | — |
pub fn tool_step_brush_param(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let param = match dops_opt_str(args, "param")?
        .ok_or_else(|| "missing required argument 'param'".to_string())?
    {
        "size" | "width" => BrushParam::Size,
        "opacity" => BrushParam::Opacity,
        "scatter" => BrushParam::Scatter,
        "smoothing" => BrushParam::Smoothing,
        other => {
            return Err(format!(
                "unknown brush param '{other}' (expected \"size\", \"opacity\", \
                 \"scatter\" or \"smoothing\")"
            ))
        }
    };
    let dir = dops_opt_i32(args, "direction")?
        .ok_or_else(|| "missing required argument 'direction'".to_string())?;
    if dir != 1 && dir != -1 {
        return Err(format!(
            "argument 'direction' must be 1 (step up) or -1 (step down), got {dir}"
        ));
    }
    let lib = crate::paint::brush_library();
    if lib.is_empty() {
        return Err("the brush library is empty".into());
    }
    let cur = core.brush_index.min(lib.len() - 1);
    // The inspector's stepping rule (`step_index_by`):
    // nearest value strictly in the stepped direction,
    // smallest difference wins, ties keep the lowest index.
    let cur_value = param.value(&lib[cur]);
    let mut best: Option<(f32, usize)> = None;
    for (i, b) in lib.iter().enumerate() {
        if i == cur {
            continue;
        }
        let v = param.value(b);
        if (dir > 0 && v <= cur_value) || (dir < 0 && v >= cur_value) {
            continue;
        }
        let d = (v - cur_value).abs();
        match best {
            Some((bd, _)) if d >= bd => {}
            _ => best = Some((d, i)),
        }
    }
    match best {
        Some((_, i)) => {
            core.brush_index = i;
            let b = lib[i];
            core.status = format!("Brush: {} (width {:.0}).", b.name, b.width);
            Ok(json!({
                "ok": true,
                "changed": true,
                "index": i,
                "name": b.name,
                "status": core.status,
            }))
        }
        None => {
            core.status = param.no_step_message(dir).into();
            Ok(json!({
                "ok": true,
                "changed": false,
                "index": cur,
                "name": lib[cur].name,
                "status": core.status,
            }))
        }
    }
}

/// Select the active brush by nib — the stroke inspector's
/// Preset nib segmented control (`stroke_inspector::select_nib`):
/// the FIRST library preset carrying the chosen nib (Round →
/// Pen, Square → Brush, Calligraphy → Calligraphy in the
/// shipped library). The nib a preset carries was otherwise
/// only reachable by naming the exact preset to `set_brush`.
/// Session state only: no snapshot, no dirty mark.
///
// TOOL ENTRY SPEC: for the integrator's registry splice
// in `mcp_tools::tools()`, beside the other
// `crate::draw_ops` entries (after `step_brush_param`):
//
//   Tool {
//       name: "set_brush_nib",
//       description: "Select the active brush preset by its nib (the stroke inspector's Preset nib segmented control): the first library preset carrying nib 'round', 'square' or 'calligraphy' becomes active. Session state only — not an undo step.",
//       schema: obj_schema(
//           json!({
//               "nib": { "type": "string", "enum": ["round", "square", "calligraphy"] }
//           }),
//           &["nib"],
//       ),
//       handler: crate::draw_ops::tool_set_brush_nib,
//   }
//
// MCP.md ROW SPEC: for the draw-tools table, after the
// `step_brush_param` row:
//
//   | `set_brush_nib` | Select the active brush preset by its nib (the stroke inspector's Preset nib segmented control): the first library preset carrying nib 'round', 'square' or 'calligraphy' becomes active. Session state only — not an undo step. | `nib` | — |
pub fn tool_set_brush_nib(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let nib = match dops_opt_str(args, "nib")?
        .ok_or_else(|| "missing required argument 'nib'".to_string())?
    {
        "round" => crate::paint::Nib::Round,
        "square" => crate::paint::Nib::Square,
        "calligraphy" => crate::paint::Nib::Calligraphy,
        other => {
            return Err(format!(
                "unknown nib '{other}' (expected \"round\", \"square\" or \"calligraphy\")"
            ))
        }
    };
    let lib = crate::paint::brush_library();
    let Some(index) = lib.iter().position(|b| b.nib == nib) else {
        return Err("no brush in the library carries that nib".into());
    };
    let changed = core.brush_index.min(lib.len().saturating_sub(1)) != index;
    core.brush_index = index;
    let b = lib[index];
    core.status = format!("Brush: {} (width {:.0}).", b.name, b.width);
    Ok(json!({
        "ok": true,
        "changed": changed,
        "index": index,
        "name": b.name,
        "status": core.status,
    }))
}

/// Apply a nib width profile to ONE stroke — the
/// single-stroke form of the frame-wide `apply_nib` tool,
/// standing to it exactly as `simplify_stroke` stands to
/// `simplify_strokes`: the same [`apply_nib`] rewrite the
/// editor's own stroke finalisation runs, addressed by the
/// stroke's project-wide index instead of by frame. One
/// undo step.
///
// TOOL ENTRY SPEC: for the integrator's registry splice
// in `mcp_tools::tools()`, beside the other
// `crate::draw_ops` entries (after `simplify_stroke`):
//
//   Tool {
//       name: "apply_nib_stroke",
//       description: "Give ONE stroke, by its project-wide index (see list_strokes), a nib width profile: \"round\" (uniform), \"square\" or \"calligraphy\" (angle-dependent, angle in degrees, default 45) — the single-stroke form of apply_nib, computed from the stroke's own width as the nib size. Replaces any per-point widths the stroke carried. One undo step.",
//       schema: obj_schema(
//           json!({
//               "index": { "type": "integer" },
//               "nib": { "type": "string", "enum": ["round", "square", "calligraphy"] },
//               "angle": { "type": "number", "description": "Nib angle in degrees (default 45)" }
//           }),
//           &["index", "nib"],
//       ),
//       handler: crate::draw_ops::tool_apply_nib_stroke,
//   }
//
// MCP.md ROW SPEC: for the draw-tools table, after the
// `apply_nib` row:
//
//   | `apply_nib_stroke` | Give ONE stroke, by its project-wide index (see list_strokes), a nib width profile: "round" (uniform), "square" or "calligraphy" (angle-dependent, angle in degrees, default 45) — the single-stroke form of apply_nib, computed from the stroke's own width as the nib size. Replaces any per-point widths the stroke carried. One undo step. | `index`, `nib` | `angle` |
pub fn tool_apply_nib_stroke(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let index = dops_req_usize(args, "index")?;
    let Some(stroke) = core.project.strokes.get(index) else {
        return Err(format!("no stroke at index {index}"));
    };
    let nib = match dops_opt_str(args, "nib")?
        .ok_or_else(|| "missing required argument 'nib'".to_string())?
    {
        "round" => Nib::Round,
        "square" => Nib::Square,
        "calligraphy" => Nib::Calligraphy,
        other => {
            return Err(format!(
                "unknown nib '{other}' (expected \"round\", \"square\" or \"calligraphy\")"
            ))
        }
    };
    let angle = dops_opt_f32(args, "angle")?.unwrap_or(45.0);
    let points = stroke.points.len();
    core.snapshot();
    apply_nib(&mut core.project.strokes[index], nib, angle);
    core.status = format!("Stroke {} nib profiled.", index + 1);
    Ok(json!({
        "ok": true,
        "index": index,
        "points": points,
        "widths": core.project.strokes[index].widths.len(),
        "status": core.status,
    }))
}

/// Chaikin-smooth ONE stroke — the single-stroke form of
/// the frame-wide `smooth_strokes` tool, running the same
/// [`smooth_stroke`] the editor's stroke finalisation runs
/// with the active brush's smoothing count. `subdiv` is the
/// number of Chaikin passes (default 1, the frame-wide
/// tool's count), bounded at 4 — the broadest library preset
/// smooths 3 times, and each pass roughly doubles the point
/// count. Honesty rule from `tool_simplify_stroke`: when the
/// stroke cannot change (fewer than 2 points), NO snapshot
/// is taken and the reply reports `smoothed: false`. One
/// undo step when it smooths.
///
// TOOL ENTRY SPEC: for the integrator's registry splice
// in `mcp_tools::tools()`, beside the other
// `crate::draw_ops` entries (after `apply_nib_stroke`):
//
//   Tool {
//       name: "smooth_stroke",
//       description: "Chaikin-smooth ONE stroke by its project-wide index (see list_strokes) — the single-stroke form of smooth_strokes: 'subdiv' corner-cutting passes (default 1, at most 4), endpoints pinned, per-point widths resampled onto the new points. Returns smoothed: false — and takes no undo step — when the stroke has fewer than 2 points and cannot change. One undo step when it smooths.",
//       schema: obj_schema(
//           json!({
//               "index": { "type": "integer" },
//               "subdiv": { "type": "integer", "description": "Chaikin passes, 1-4 (default 1)" }
//           }),
//           &["index"],
//       ),
//       handler: crate::draw_ops::tool_smooth_stroke,
//   }
//
// MCP.md ROW SPEC: for the draw-tools table, after the
// `smooth_strokes` row:
//
//   | `smooth_stroke` | Chaikin-smooth ONE stroke by its project-wide index (see list_strokes) — the single-stroke form of smooth_strokes: 'subdiv' corner-cutting passes (default 1, at most 4), endpoints pinned, per-point widths resampled onto the new points. Returns smoothed: false — and takes no undo step — when the stroke has fewer than 2 points and cannot change. One undo step when it smooths. | `index` | `subdiv` |
pub fn tool_smooth_stroke(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let index = dops_req_usize(args, "index")?;
    let subdiv = dops_opt_i32(args, "subdiv")?.unwrap_or(1);
    if !(1..=4).contains(&subdiv) {
        return Err(format!(
            "argument 'subdiv' must be between 1 and 4, got {subdiv}"
        ));
    }
    let Some(stroke) = core.project.strokes.get(index) else {
        return Err(format!("no stroke at index {index}"));
    };
    let before_points = stroke.points.clone();
    if before_points.len() < 2 {
        core.status = format!("Stroke {} is too short to smooth.", index + 1);
        return Ok(json!({
            "ok": true,
            "smoothed": false,
            "index": index,
            "points": before_points.len(),
            "status": core.status,
        }));
    }
    core.snapshot();
    smooth_stroke(&mut core.project.strokes[index], subdiv as usize);
    let after = core.project.strokes[index].points.len();
    core.status = format!("Stroke {} smoothed to {} points.", index + 1, after);
    Ok(json!({
        "ok": true,
        "smoothed": true,
        "index": index,
        "points_before": before_points.len(),
        "points_after": after,
        "status": core.status,
    }))
}

/// Mirror EVERY stroke in the project — the project-wide
/// form of the current-frame `mirror_strokes` tool, running
/// this module's [`mirror_strokes_all`], which until now no
/// surface could reach (the Draw panel's button and the
/// existing tool both stop at the current frame). `cx` is
/// the mirror line's x in stage coordinates, defaulting to
/// the stage's vertical centre, the frame-wide form's line.
/// An empty project is an error, not a silent success (the
/// `mirror_strokes` discipline). One undo step.
///
// TOOL ENTRY SPEC: for the integrator's registry splice
// in `mcp_tools::tools()`, beside the other
// `crate::draw_ops` entries (after `mirror_strokes`):
//
//   Tool {
//       name: "mirror_all_strokes",
//       description: "Mirror EVERY stroke in the project, on all frames, across the vertical line x = 'cx' (default: the stage's vertical centre) — the project-wide form of mirror_strokes. Only points change. Errors on a project with no strokes. One undo step.",
//       schema: obj_schema(
//           json!({
//               "cx": { "type": "number", "description": "Mirror line x in stage coordinates (default: stage centre)" }
//           }),
//           &[],
//       ),
//       handler: crate::draw_ops::tool_mirror_all_strokes,
//   }
//
// MCP.md ROW SPEC: for the draw-tools table, after the
// `mirror_strokes` row:
//
//   | `mirror_all_strokes` | Mirror EVERY stroke in the project, on all frames, across the vertical line x = 'cx' (default: the stage's vertical centre) — the project-wide form of mirror_strokes. Only points change. Errors on a project with no strokes. One undo step. | — | `cx` |
pub fn tool_mirror_all_strokes(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let cx = dops_opt_f32(args, "cx")?.unwrap_or_else(|| core.project.stage[0] / 2.0);
    if core.project.strokes.is_empty() {
        return Err("no strokes to mirror (the project has no strokes)".into());
    }
    core.snapshot();
    let mirrored = mirror_strokes_all(&mut core.project, cx);
    core.status = format!("Mirrored {mirrored} stroke(s) across x = {cx}.");
    Ok(json!({
        "ok": true,
        "mirrored": mirrored,
        "cx": cx,
        "status": core.status,
    }))
}

/// Edit one stroke's spray mark — the one rendered stroke
/// field no tool could write: `set_stroke_style` refuses
/// `opacity` and `scatter` on a spray stroke precisely
/// because its visible opacity lives on the mark, and until
/// now the mark itself (seed / radius / opacity) was
/// writeable only at creation time, by `draw_spray` or a
/// scatter brush. This is its editor, in `set_stroke_style`'s
/// shape: pass `radius` (> 0), `opacity` (0–1) and/or `seed`
/// to set the mark's fields — on a plain stroke they CREATE
/// the mark (defaults: the Spray preset's radius 12 and
/// opacity 0.5, and the deterministic seed `begin_draw`
/// derives, `0x5EED_0000 + frame × 131 + index`, so an
/// MCP-made spray renders identically to a hand-drawn one) —
/// or pass `clear_spray: true` alone to remove the mark and
/// return the stroke to a plain polyline. Clearing a stroke
/// that carries no mark is a no-op reported with
/// `changed: false` and spends no undo step. The stroke's
/// own `opacity` / `scatter` fields are NOT touched: the
/// mark takes render precedence over both, exactly as
/// `begin_draw` records it. One undo step when the mark
/// changes.
///
// TOOL ENTRY SPEC: for the integrator's registry splice
// in `mcp_tools::tools()`, beside the other
// `crate::draw_ops` entries (after `set_stroke_style`):
//
//   Tool {
//       name: "set_stroke_spray",
//       description: "Edit one stroke's spray mark by its project-wide index (see list_strokes) — the field set_stroke_style cannot reach: 'radius' (> 0), 'opacity' (0-1) and/or 'seed' set the mark's fields, creating the mark on a plain stroke (defaults: radius 12, opacity 0.5, the deterministic begin_draw seed); 'clear_spray': true removes the mark instead, returning the stroke to a plain polyline. At least one field is required. Clearing a markless stroke reports changed: false and takes no undo step. One undo step when the mark changes.",
//       schema: obj_schema(
//           json!({
//               "index": { "type": "integer" },
//               "radius": { "type": "number", "description": "Spray radius in stage units" },
//               "opacity": { "type": "number" },
//               "seed": { "type": "integer" },
//               "clear_spray": { "type": "boolean", "description": "Remove the spray mark" }
//           }),
//           &["index"],
//       ),
//       handler: crate::draw_ops::tool_set_stroke_spray,
//   }
//
// MCP.md ROW SPEC: for the draw-tools table, after the
// `set_stroke_style` row:
//
//   | `set_stroke_spray` | Edit one stroke's spray mark by its project-wide index (see list_strokes) — the field set_stroke_style cannot reach: 'radius' (> 0), 'opacity' (0-1) and/or 'seed' set the mark's fields, creating the mark on a plain stroke (defaults: radius 12, opacity 0.5, the deterministic begin_draw seed); 'clear_spray': true removes the mark instead, returning the stroke to a plain polyline. At least one field is required. Clearing a markless stroke reports changed: false and takes no undo step. One undo step when the mark changes. | `index` | `radius`, `opacity`, `seed`, `clear_spray` |
pub fn tool_set_stroke_spray(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let index = dops_req_usize(args, "index")?;
    let Some(stroke) = core.project.strokes.get(index) else {
        return Err(format!("no stroke at index {index}"));
    };
    let radius = dops_opt_f32(args, "radius")?;
    let opacity = dops_opt_f32(args, "opacity")?;
    let seed = dops_opt_usize(args, "seed")?.map(|s| s as u64);
    let clear = dops_opt_bool(args, "clear_spray")?.unwrap_or(false);
    if clear && (radius.is_some() || opacity.is_some() || seed.is_some()) {
        return Err("pass either spray fields or 'clear_spray', not both".into());
    }
    if !clear && radius.is_none() && opacity.is_none() && seed.is_none() {
        return Err(
            "nothing to set: pass at least one of 'radius', 'opacity', 'seed' \
             or 'clear_spray'"
                .into(),
        );
    }
    if let Some(r) = radius {
        if r <= 0.0 {
            return Err("argument 'radius' must be positive".into());
        }
    }
    if let Some(o) = opacity {
        if !(0.0..=1.0).contains(&o) {
            return Err("argument 'opacity' must be between 0 and 1".into());
        }
    }
    if clear {
        if stroke.spray.is_none() {
            core.status = format!("Stroke {} has no spray mark to clear.", index + 1);
            return Ok(json!({
                "ok": true,
                "changed": false,
                "index": index,
                "spray": Value::Null,
                "status": core.status,
            }));
        }
        core.snapshot();
        core.project.strokes[index].spray = None;
        core.status = format!("Stroke {} spray mark cleared.", index + 1);
        return Ok(json!({
            "ok": true,
            "changed": true,
            "index": index,
            "spray": Value::Null,
            "status": core.status,
        }));
    }
    let frame = stroke.frame;
    core.snapshot();
    let stroke = &mut core.project.strokes[index];
    let mark = stroke.spray.get_or_insert(crate::model::SprayMark {
        seed: 0x5EED_0000u64 + (frame as u64) * 131 + index as u64,
        radius: 12.0,
        opacity: 0.5,
    });
    if let Some(r) = radius {
        mark.radius = r;
    }
    if let Some(o) = opacity {
        mark.opacity = o;
    }
    if let Some(s) = seed {
        mark.seed = s;
    }
    let mark = *mark;
    core.status = format!("Stroke {} spray mark updated.", index + 1);
    Ok(json!({
        "ok": true,
        "changed": true,
        "index": index,
        "spray": json!({
            "seed": mark.seed,
            "radius": mark.radius,
            "opacity": mark.opacity,
        }),
        "status": core.status,
    }))
}

/// Clear the strokes on ANY frame — the frame-addressed
/// form of `clear_frame_strokes`, whose core op
/// (`EditorCore::clear_frame_drawings`) is current-frame-only
/// and whose handler therefore refuses every other frame.
/// Every other frame-wide draw tool (`erase_strokes`,
/// `smooth_strokes`, `simplify_strokes`, `apply_nib`) takes
/// its frame as an argument; this closes that asymmetry
/// with the same retain the core op performs, snapshot
/// first and only when the frame actually carries strokes
/// (the op's own discipline). The current frame may be
/// named too — the result is identical there.
///
// TOOL ENTRY SPEC: for the integrator's registry splice
// in `mcp_tools::tools()`, beside the other
// `crate::draw_ops` entries (after `clear_frame_strokes`):
//
//   Tool {
//       name: "clear_strokes_on_frame",
//       description: "Remove every stroke on the NAMED frame (the frame-addressed form of clear_frame_strokes, whose core op is current-frame-only): 'frame' is required and may name any frame, including the current one. One undo step when the frame carried strokes.",
//       schema: obj_schema(
//           json!({
//               "frame": { "type": "integer" }
//           }),
//           &["frame"],
//       ),
//       handler: crate::draw_ops::tool_clear_strokes_on_frame,
//   }
//
// MCP.md ROW SPEC: for the draw-tools table, after the
// `clear_frame_strokes` row:
//
//   | `clear_strokes_on_frame` | Remove every stroke on the NAMED frame (the frame-addressed form of clear_frame_strokes, whose core op is current-frame-only): 'frame' is required and may name any frame, including the current one. One undo step when the frame carried strokes. | `frame` | — |
pub fn tool_clear_strokes_on_frame(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let frame = dops_req_usize(args, "frame")?;
    dops_check_frame(core, frame)?;
    let n = stroke_count(&core.project, frame);
    if n == 0 {
        core.status = format!("No strokes on frame {} to clear.", frame + 1);
        return Ok(json!({
            "ok": true,
            "frame": frame,
            "cleared": 0,
            "count": core.project.strokes.len(),
            "status": core.status,
        }));
    }
    core.snapshot();
    core.project.strokes.retain(|s| s.frame != frame);
    core.status = format!("Cleared {n} stroke(s) on frame {}.", frame + 1);
    Ok(json!({
        "ok": true,
        "frame": frame,
        "cleared": n,
        "count": core.project.strokes.len(),
        "status": core.status,
    }))
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
            scatter: 0.0,
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
            scatter: 0.0,
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
            scatter: 0.0,
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
            scatter: 0.0,
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

#[cfg(test)]
mod mcp_handler_tests {
    use super::*;
    use serde_json::json;

    fn draw_line(core: &mut EditorCore, scatter: f32) -> Value {
        tool_draw_stroke(
            core,
            &json!({
                "frame": 0,
                "points": [[0.0, 0.0], [40.0, 0.0], [80.0, 0.0]],
                "scatter": scatter,
            }),
        )
        .expect("draw_stroke succeeds")
    }

    #[test]
    fn draw_stroke_records_scatter_and_the_dabs_follow() {
        let mut core = EditorCore::new();
        let out = draw_line(&mut core, 5.0);
        assert_eq!(out["index"], 0);
        let stroke = &core.project.strokes[0];
        assert_eq!(stroke.scatter, 5.0);
        // The placements every renderer uses displace at
        // least one dab off the unscattered resample.
        let dabs = crate::paint::stroke_dabs(
            stroke,
            0,
            crate::paint::stroke_dab_spacing(stroke),
        );
        let plain = crate::paint::resample(&stroke.points, crate::paint::stroke_dab_spacing(stroke));
        assert_eq!(dabs.len(), plain.len());
        assert!(dabs != plain, "scatter 5 must move dabs: {dabs:?}");
        assert!(
            dabs.iter().all(|p| p[0].is_finite() && p[1].is_finite()),
            "dab positions stay finite"
        );
    }

    #[test]
    fn draw_stroke_refuses_negative_scatter_and_bad_width() {
        let mut core = EditorCore::new();
        let err = tool_draw_stroke(
            &mut core,
            &json!({"frame": 0, "points": [[0.0, 0.0]], "scatter": -1.0}),
        )
        .unwrap_err();
        assert!(err.contains("scatter"), "{err}");
        let err = tool_draw_stroke(
            &mut core,
            &json!({"frame": 0, "points": [[0.0, 0.0]], "width": 0.0}),
        )
        .unwrap_err();
        assert!(err.contains("width"), "{err}");
        assert!(core.project.strokes.is_empty(), "refusals record nothing");
    }

    #[test]
    fn draw_stroke_refuses_non_finite_numbers() {
        // `1e999` overflows f64 on the way in; if the JSON
        // parser itself refuses it, the value can never reach
        // a handler and there is nothing to prove here.
        let Ok(args) = serde_json::from_str::<Value>(
            "{\"frame\": 0, \"points\": [[0.0, 0.0], [10.0, 0.0]], \"scatter\": 1e999}",
        ) else {
            return;
        };
        if args["scatter"].as_f64().map(|f| f.is_finite()).unwrap_or(true) {
            return; // parser normalised it away; nothing reaches the handler
        }
        let mut core = EditorCore::new();
        let err = tool_draw_stroke(&mut core, &args).unwrap_err();
        assert!(err.contains("finite"), "{err}");
        assert!(core.project.strokes.is_empty());

        let Ok(args) = serde_json::from_str::<Value>(
            "{\"frame\": 0, \"points\": [[1e999, 0.0], [10.0, 0.0]]}",
        ) else {
            return;
        };
        if args["points"][0][0]
            .as_f64()
            .map(|f| f.is_finite())
            .unwrap_or(true)
        {
            return;
        }
        let err = tool_draw_stroke(&mut core, &args).unwrap_err();
        assert!(err.contains("finite"), "{err}");
        assert!(core.project.strokes.is_empty());
    }

    #[test]
    fn list_strokes_reports_every_field_and_filters_by_frame() {
        let mut core = EditorCore::new();
        draw_line(&mut core, 3.0);
        let out = tool_list_strokes(&mut core, &json!({})).unwrap();
        assert_eq!(out["count"], 1);
        let s = &out["strokes"][0];
        assert_eq!(s["index"], 0);
        assert_eq!(s["frame"], 0);
        assert_eq!(s["points"], 3, "count travels when points are not requested");
        assert_eq!(s["scatter"], 3.0);
        assert_eq!(s["opacity"], 1.0);
        assert!(s["spray"].is_null());
        let out = tool_list_strokes(&mut core, &json!({"points": true})).unwrap();
        assert_eq!(out["strokes"][0]["points"][1], json!([40.0, 0.0]));
        let out = tool_list_strokes(&mut core, &json!({"frame": 1})).unwrap();
        assert_eq!(out["count"], 0);
        assert_eq!(out["total"], 1);
    }

    #[test]
    fn set_stroke_style_sets_fields_and_scales_the_taper() {
        let mut core = EditorCore::new();
        draw_line(&mut core, 0.0);
        core.project.strokes[0].widths = vec![4.0, 4.0, 4.0];
        let out = tool_set_stroke_style(
            &mut core,
            &json!({"index": 0, "color": 0xFF0000u32, "width": 8.0, "opacity": 0.5, "scatter": 2.0, "fill": 0x00FF00u32}),
        )
        .unwrap();
        let stroke = &core.project.strokes[0];
        assert_eq!(stroke.color, 0xFF0000);
        assert_eq!(stroke.width, 8.0);
        assert_eq!(stroke.widths, vec![8.0, 8.0, 8.0], "taper scales by the ratio");
        assert_eq!(stroke.opacity, 0.5);
        assert_eq!(stroke.scatter, 2.0);
        assert_eq!(stroke.fill, Some(0x00FF00));
        assert_eq!(out["stroke"]["width"], 8.0);
        // And clear_fill removes it again.
        tool_set_stroke_style(&mut core, &json!({"index": 0, "clear_fill": true})).unwrap();
        assert_eq!(core.project.strokes[0].fill, None);
    }

    #[test]
    fn set_stroke_style_refuses_lies() {
        let mut core = EditorCore::new();
        draw_line(&mut core, 0.0);
        // Width outside the panel's range.
        assert!(tool_set_stroke_style(&mut core, &json!({"index": 0, "width": 41.0})).is_err());
        assert!(tool_set_stroke_style(&mut core, &json!({"index": 0, "width": 0.1})).is_err());
        // Nothing to set.
        assert!(tool_set_stroke_style(&mut core, &json!({"index": 0})).is_err());
        // Bad index.
        assert!(tool_set_stroke_style(&mut core, &json!({"index": 9, "width": 4.0})).is_err());
        // A spray stroke's opacity/scatter live elsewhere.
        core.project.strokes[0].spray = Some(crate::model::SprayMark {
            seed: 7,
            radius: 12.0,
            opacity: 0.5,
        });
        let err = tool_set_stroke_style(&mut core, &json!({"index": 0, "opacity": 0.5}))
            .unwrap_err();
        assert!(err.contains("spray"), "{err}");
        let err = tool_set_stroke_style(&mut core, &json!({"index": 0, "scatter": 2.0}))
            .unwrap_err();
        assert!(err.contains("spray"), "{err}");
        // But its colour and width remain editable.
        assert!(tool_set_stroke_style(&mut core, &json!({"index": 0, "color": 1u32})).is_ok());
    }

    #[test]
    fn simplify_stroke_reduces_and_stays_honest_when_already_simple() {
        let mut core = EditorCore::new();
        tool_draw_stroke(
            &mut core,
            &json!({
                "frame": 0,
                "points": [[0.0, 0.0], [10.0, 0.4], [20.0, -0.3], [30.0, 0.2], [40.0, 0.0]],
            }),
        )
        .unwrap();
        let undo_before = core.undo_depth();
        let out = tool_simplify_stroke(&mut core, &json!({"index": 0, "epsilon": 1.0})).unwrap();
        assert_eq!(out["simplified"], true);
        assert_eq!(core.project.strokes[0].points.len(), 2);
        assert_eq!(core.undo_depth(), undo_before + 1, "one undo step");
        // Already simple now: no further snapshot.
        let out = tool_simplify_stroke(&mut core, &json!({"index": 0, "epsilon": 1.0})).unwrap();
        assert_eq!(out["simplified"], false);
        assert_eq!(core.undo_depth(), undo_before + 1, "no phantom undo step");
    }

    #[test]
    fn draw_arrow_stamps_the_paint_geometry_and_refuses_degenerate() {
        let mut core = EditorCore::new();
        let out = tool_draw_arrow(
            &mut core,
            &json!({"x1": 10.0, "y1": 20.0, "x2": 110.0, "y2": 20.0}),
        )
        .unwrap();
        assert_eq!(out["points"], 5);
        assert_eq!(
            core.project.strokes[0].points,
            crate::paint::arrow_points([10.0, 20.0], [110.0, 20.0], 22.0),
            "the paint generator's points, exactly"
        );
        let err = tool_draw_arrow(
            &mut core,
            &json!({"x1": 5.0, "y1": 5.0, "x2": 5.0, "y2": 5.0}),
        )
        .unwrap_err();
        assert!(err.contains("differ"), "{err}");
    }

    #[test]
    fn fill_stroke_fills_the_latest_closed_stroke() {
        let mut core = EditorCore::new();
        let err = tool_fill_stroke(&mut core, &json!({})).unwrap_err();
        assert!(err.contains("Nothing to fill"), "{err}");
        tool_draw_stroke(
            &mut core,
            &json!({
                "frame": 0,
                "points": [[0.0, 0.0], [100.0, 0.0], [50.0, 80.0], [0.0, 0.0]],
            }),
        )
        .unwrap();
        let out = tool_fill_stroke(&mut core, &json!({})).unwrap();
        assert_eq!(out["ok"], true);
        assert_eq!(core.project.strokes[0].fill, Some(core.project.strokes[0].color));
        assert_eq!(core.project.strokes.len(), 2, "the scanline companion was appended");
    }

    #[test]
    fn lasso_erase_removes_mostly_inside_strokes_only() {
        let mut core = EditorCore::new();
        draw_line(&mut core, 0.0);
        let out = tool_lasso_erase(
            &mut core,
            &json!({"points": [[-10.0, -10.0], [200.0, -10.0], [200.0, 200.0], [-10.0, 200.0]]}),
        )
        .unwrap();
        assert_eq!(out["removed"], 1);
        assert!(core.project.strokes.is_empty());
        let err = tool_lasso_erase(
            &mut core,
            &json!({"points": [[0.0, 0.0], [1.0, 1.0]]}),
        )
        .unwrap_err();
        assert!(err.contains("at least 3"), "{err}");
    }

    #[test]
    fn mirror_recolor_clear_and_delete_all_compose() {
        let mut core = EditorCore::new();
        let err = tool_mirror_strokes(&mut core, &json!({})).unwrap_err();
        assert!(err.contains("No strokes to mirror"), "{err}");
        draw_line(&mut core, 0.0);
        let cx = core.project.stage[0];
        tool_mirror_strokes(&mut core, &json!({})).unwrap();
        assert_eq!(core.project.strokes[0].points[0], [cx - 0.0, 0.0]);
        assert_eq!(core.project.strokes[0].points[2], [cx - 80.0, 0.0]);

        let out = tool_recolor_strokes(&mut core, &json!({"color": 0x112233u32})).unwrap();
        assert_eq!(out["recolored"], 1);
        assert_eq!(core.project.strokes[0].color, 0x112233);

        let out = tool_clear_frame_strokes(&mut core, &json!({})).unwrap();
        assert_eq!(out["cleared"], 1);
        assert!(core.project.strokes.is_empty());

        draw_line(&mut core, 0.0);
        let out = tool_delete_all_strokes(&mut core, &json!({})).unwrap();
        assert_eq!(out["deleted"], 1);
        let out = tool_delete_all_strokes(&mut core, &json!({})).unwrap();
        assert_eq!(out["deleted"], 0);
    }

    #[test]
    fn brush_tools_list_select_and_cycle() {
        let mut core = EditorCore::new();
        let out = tool_list_brushes(&mut core, &json!({})).unwrap();
        assert_eq!(out["count"], 10);
        assert_eq!(out["active_name"], "Pen");
        let out = tool_set_brush(&mut core, &json!({"name": "spray"})).unwrap();
        assert_eq!(out["index"], 9);
        assert_eq!(core.active_brush().name, "Spray");
        let out = tool_cycle_brush(&mut core, &json!({})).unwrap();
        assert_eq!(out["index"], 0, "cycle wraps past the last preset");
        assert!(tool_set_brush(&mut core, &json!({"name": "Nope"})).is_err());
        assert!(tool_set_brush(&mut core, &json!({"index": 99})).is_err());
        assert!(tool_set_brush(&mut core, &json!({})).is_err());
    }

    #[test]
    fn draw_tool_selection_sets_and_reports() {
        let mut core = EditorCore::new();
        let out = tool_set_draw_tool(&mut core, &json!({"tool": "eraser"})).unwrap();
        assert_eq!(out["status"], "Draw tool: Eraser.");
        assert!(core.draw_tool_is_eraser());
        let out = tool_set_draw_tool(&mut core, &json!({"tool": "brush"})).unwrap();
        assert!(out["status"].as_str().unwrap().starts_with("Draw tool: Brush"));
        assert!(tool_set_draw_tool(&mut core, &json!({"tool": "laser"})).is_err());
    }

    #[test]
    fn drawing_stats_counts_strokes_points_and_spray() {
        let mut core = EditorCore::new();
        draw_line(&mut core, 0.0);
        core.project.strokes[0].spray = Some(crate::model::SprayMark {
            seed: 1,
            radius: 6.0,
            opacity: 1.0,
        });
        let out = tool_get_drawing_stats(&mut core, &json!({})).unwrap();
        assert_eq!(out["strokes"], 1);
        assert_eq!(out["points"], 3);
        assert_eq!(out["total_length"], 80.0);
        assert_eq!(out["spray_strokes"], 1);
        assert_eq!(out["strokes_on_current_frame"], 1);
    }

    // ---- Wave 14 (A3) parity fill ----

    #[test]
    fn get_brush_reports_the_active_preset_and_session() {
        let mut core = EditorCore::new();
        let out = tool_get_brush(&mut core, &json!({})).unwrap();
        assert_eq!(out["name"], "Pen");
        assert_eq!(out["width"], 4.0);
        assert_eq!(out["nib"], "round");
        assert_eq!(out["tool"], "brush");
        assert_eq!(out["eraser_radius"], 8.0);
        assert_eq!(out["draw_mode"], false);
        tool_set_brush(&mut core, &json!({"name": "spray"})).unwrap();
        let out = tool_get_brush(&mut core, &json!({})).unwrap();
        assert_eq!(out["name"], "Spray");
        assert_eq!(out["scatter"], 12.0);
        assert_eq!(out["opacity"], 0.5);
    }

    #[test]
    fn set_draw_mode_sets_the_flag_and_the_panel_status() {
        let mut core = EditorCore::new();
        let undo_before = core.undo_depth();
        let out = tool_set_draw_mode(&mut core, &json!({"enabled": true})).unwrap();
        assert_eq!(out["draw_mode"], true);
        assert!(core.draw_mode);
        assert!(out["status"].as_str().unwrap().starts_with("Draw mode:"));
        let out = tool_set_draw_mode(&mut core, &json!({"enabled": false})).unwrap();
        assert_eq!(out["status"], "Pose mode.");
        assert!(!core.draw_mode);
        assert!(tool_set_draw_mode(&mut core, &json!({})).is_err());
        assert_eq!(core.undo_depth(), undo_before, "session write: no undo step");
    }

    #[test]
    fn step_brush_param_walks_the_library_by_value() {
        let mut core = EditorCore::new();
        // From Pen (width 4): the nearest wider preset is
        // Chisel Marker (5), the inspector's stepping rule.
        let out = tool_step_brush_param(
            &mut core,
            &json!({"param": "size", "direction": 1}),
        )
        .unwrap();
        assert_eq!(out["changed"], true);
        assert_eq!(out["name"], "Chisel Marker");
        assert_eq!(core.active_brush().name, "Chisel Marker");
        // Spray is the only scattered preset: stepping
        // scatter up from Pen lands on it.
        let mut core = EditorCore::new();
        let out = tool_step_brush_param(
            &mut core,
            &json!({"param": "scatter", "direction": 1}),
        )
        .unwrap();
        assert_eq!(out["name"], "Spray");
        // Nothing scatters tighter than Pen's 0: an honest
        // no-step report, not an error, and no move.
        let mut core = EditorCore::new();
        let out = tool_step_brush_param(
            &mut core,
            &json!({"param": "scatter", "direction": -1}),
        )
        .unwrap();
        assert_eq!(out["changed"], false);
        assert_eq!(out["status"], "No tighter-scatter brush in the library.");
        assert_eq!(core.active_brush().name, "Pen");
        // Bad inputs refuse, quoting the offender.
        let err = tool_step_brush_param(
            &mut core,
            &json!({"param": "size", "direction": 2}),
        )
        .unwrap_err();
        assert!(err.contains("direction"), "{err}");
        assert!(tool_step_brush_param(
            &mut core,
            &json!({"param": "weight", "direction": 1})
        )
        .is_err());
    }

    #[test]
    fn set_brush_nib_selects_the_first_preset_with_that_nib() {
        let mut core = EditorCore::new();
        let out = tool_set_brush_nib(&mut core, &json!({"nib": "square"})).unwrap();
        assert_eq!(out["name"], "Brush", "the first square-nib preset");
        let out = tool_set_brush_nib(&mut core, &json!({"nib": "calligraphy"})).unwrap();
        assert_eq!(out["name"], "Calligraphy");
        let out = tool_set_brush_nib(&mut core, &json!({"nib": "round"})).unwrap();
        assert_eq!(out["name"], "Pen");
        assert!(tool_set_brush_nib(&mut core, &json!({"nib": "airbrush"})).is_err());
    }

    #[test]
    fn apply_nib_stroke_profiles_only_that_stroke() {
        let mut core = EditorCore::new();
        draw_line(&mut core, 0.0);
        draw_line(&mut core, 0.0);
        let undo_before = core.undo_depth();
        let out = tool_apply_nib_stroke(
            &mut core,
            &json!({"index": 1, "nib": "calligraphy", "angle": 30.0}),
        )
        .unwrap();
        assert_eq!(out["widths"], 3, "one width per point");
        assert_eq!(core.project.strokes[1].widths.len(), 3);
        assert!(
            core.project.strokes[0].widths.is_empty(),
            "the other stroke is untouched"
        );
        assert_eq!(core.undo_depth(), undo_before + 1, "one undo step");
        let err = tool_apply_nib_stroke(
            &mut core,
            &json!({"index": 9, "nib": "round"}),
        )
        .unwrap_err();
        assert!(err.contains("index 9"), "{err}");
        assert!(tool_apply_nib_stroke(
            &mut core,
            &json!({"index": 0, "nib": "airbrush"})
        )
        .is_err());
    }

    #[test]
    fn smooth_stroke_smooths_one_and_stays_honest_when_too_short() {
        let mut core = EditorCore::new();
        tool_draw_stroke(
            &mut core,
            &json!({"frame": 0, "points": [[0.0, 0.0], [20.0, 10.0], [40.0, 0.0]]}),
        )
        .unwrap();
        let undo_before = core.undo_depth();
        let out = tool_smooth_stroke(&mut core, &json!({"index": 0})).unwrap();
        assert_eq!(out["smoothed"], true);
        assert!(core.project.strokes[0].points.len() > 3, "Chaikin adds points");
        assert_eq!(core.undo_depth(), undo_before + 1, "one undo step");
        // A one-point stroke cannot change: no snapshot.
        tool_draw_stroke(&mut core, &json!({"frame": 0, "points": [[5.0, 5.0]]})).unwrap();
        let undo_before = core.undo_depth();
        let out = tool_smooth_stroke(&mut core, &json!({"index": 1})).unwrap();
        assert_eq!(out["smoothed"], false);
        assert_eq!(core.undo_depth(), undo_before, "no phantom undo step");
        assert!(tool_smooth_stroke(&mut core, &json!({"index": 0, "subdiv": 9})).is_err());
        assert!(tool_smooth_stroke(&mut core, &json!({"index": 7})).is_err());
    }

    #[test]
    fn mirror_all_strokes_mirrors_every_frame() {
        let mut core = EditorCore::new();
        draw_line(&mut core, 0.0);
        tool_draw_stroke(
            &mut core,
            &json!({"frame": 1, "points": [[0.0, 0.0], [10.0, 0.0]]}),
        )
        .unwrap();
        let cx = core.project.stage[0] / 2.0;
        let undo_before = core.undo_depth();
        let out = tool_mirror_all_strokes(&mut core, &json!({})).unwrap();
        assert_eq!(out["mirrored"], 2);
        assert_eq!(core.project.strokes[1].points[0], [2.0 * cx, 0.0]);
        assert_eq!(core.undo_depth(), undo_before + 1, "one undo step");
        // An empty project is refused, like mirror_strokes.
        let mut core = EditorCore::new();
        assert!(tool_mirror_all_strokes(&mut core, &json!({})).is_err());
    }

    #[test]
    fn set_stroke_spray_creates_edits_and_clears_the_mark() {
        let mut core = EditorCore::new();
        draw_line(&mut core, 0.0);
        // Create with defaults: the Spray preset's radius /
        // opacity and the deterministic begin_draw seed
        // (frame 0, index 0).
        let undo_before = core.undo_depth();
        let out = tool_set_stroke_spray(&mut core, &json!({"index": 0, "radius": 9.0})).unwrap();
        assert_eq!(out["changed"], true);
        let mark = core.project.strokes[0].spray.expect("mark created");
        assert_eq!(mark.radius, 9.0);
        assert_eq!(mark.opacity, 0.5);
        assert_eq!(mark.seed, 0x5EED_0000u64);
        assert_eq!(core.undo_depth(), undo_before + 1, "one undo step");
        // Edit the mark's own opacity — the write
        // set_stroke_style refuses on a spray stroke.
        tool_set_stroke_spray(&mut core, &json!({"index": 0, "opacity": 0.25})).unwrap();
        assert_eq!(core.project.strokes[0].spray.unwrap().opacity, 0.25);
        // Clear returns the stroke to a plain polyline.
        tool_set_stroke_spray(&mut core, &json!({"index": 0, "clear_spray": true})).unwrap();
        assert!(core.project.strokes[0].spray.is_none());
        // Clearing again is an honest no-op: no undo step.
        let undo_before = core.undo_depth();
        let out = tool_set_stroke_spray(&mut core, &json!({"index": 0, "clear_spray": true}))
            .unwrap();
        assert_eq!(out["changed"], false);
        assert_eq!(core.undo_depth(), undo_before, "no phantom undo step");
        // Refusals: nothing to set, fields + clear together,
        // bad values, bad index.
        assert!(tool_set_stroke_spray(&mut core, &json!({"index": 0})).is_err());
        assert!(tool_set_stroke_spray(
            &mut core,
            &json!({"index": 0, "radius": 5.0, "clear_spray": true})
        )
        .is_err());
        assert!(tool_set_stroke_spray(&mut core, &json!({"index": 0, "radius": 0.0})).is_err());
        assert!(tool_set_stroke_spray(&mut core, &json!({"index": 3, "radius": 5.0})).is_err());
    }

    #[test]
    fn clear_strokes_on_frame_clears_any_named_frame() {
        let mut core = EditorCore::new();
        draw_line(&mut core, 0.0);
        tool_draw_stroke(
            &mut core,
            &json!({"frame": 1, "points": [[0.0, 0.0], [10.0, 0.0]]}),
        )
        .unwrap();
        // Frame 1 is not the current frame — the case
        // clear_frame_strokes refuses.
        let undo_before = core.undo_depth();
        let out = tool_clear_strokes_on_frame(&mut core, &json!({"frame": 1})).unwrap();
        assert_eq!(out["cleared"], 1);
        assert_eq!(core.project.strokes.len(), 1);
        assert_eq!(core.project.strokes[0].frame, 0);
        assert_eq!(core.undo_depth(), undo_before + 1, "one undo step");
        // An already-empty frame spends nothing.
        let undo_before = core.undo_depth();
        let out = tool_clear_strokes_on_frame(&mut core, &json!({"frame": 1})).unwrap();
        assert_eq!(out["cleared"], 0);
        assert_eq!(core.undo_depth(), undo_before, "no phantom undo step");
        // Out-of-range frame refuses, quoting it.
        let err = tool_clear_strokes_on_frame(&mut core, &json!({"frame": 9})).unwrap_err();
        assert!(err.contains("frame index 9"), "{err}");
    }
}

#[cfg(test)]
mod dirty_contract_tests {
    //! Wave 13 (B8): the Wave 12 E8 census applied to this
    //! file, as executable pins. `EditorCore::snapshot` is
    //! the only write that bumps the edit count behind the
    //! dirty dot and the autosave cadence, and it adds
    //! exactly one undo step — so every mutator's contract
    //! is a pair of deltas over (undo_depth,
    //! edits_since_autosave): a real project edit moves
    //! BOTH by exactly one; a refusal, a no-change call and
    //! a session-only write move NEITHER. (The pre-existing
    //! suites pin the geometry and reply shapes; this module
    //! pins only the dirty/undo bookkeeping, per op.)
    use super::*;
    use serde_json::json;

    fn witnesses(core: &EditorCore) -> (usize, u64) {
        (core.undo_depth(), core.edits_since_autosave())
    }

    fn draw_line(core: &mut EditorCore) {
        tool_draw_stroke(
            core,
            &json!({"frame": 0, "points": [[0.0, 0.0], [40.0, 0.0], [80.0, 0.0]]}),
        )
        .expect("draw_stroke succeeds");
    }

    #[test]
    fn draw_and_arrow_spend_exactly_one_step_and_one_edit() {
        let mut core = EditorCore::new();
        let before = witnesses(&core);
        draw_line(&mut core);
        assert_eq!(witnesses(&core), (before.0 + 1, before.1 + 1), "draw_stroke");
        let before = witnesses(&core);
        tool_draw_arrow(
            &mut core,
            &json!({"x1": 10.0, "y1": 20.0, "x2": 110.0, "y2": 20.0}),
        )
        .unwrap();
        assert_eq!(witnesses(&core), (before.0 + 1, before.1 + 1), "draw_arrow");
        // Refusals spend nothing: bad width, negative
        // scatter, out-of-range frame, degenerate arrow.
        let before = witnesses(&core);
        assert!(tool_draw_stroke(
            &mut core,
            &json!({"frame": 0, "points": [[0.0, 0.0]], "width": 0.0})
        )
        .is_err());
        assert!(tool_draw_stroke(
            &mut core,
            &json!({"frame": 0, "points": [[0.0, 0.0]], "scatter": -1.0})
        )
        .is_err());
        assert!(tool_draw_stroke(
            &mut core,
            &json!({"frame": 99, "points": [[0.0, 0.0]]})
        )
        .is_err());
        assert!(tool_draw_arrow(
            &mut core,
            &json!({"x1": 5.0, "y1": 5.0, "x2": 5.0, "y2": 5.0})
        )
        .is_err());
        assert_eq!(witnesses(&core), before, "refusals spend nothing");
    }

    #[test]
    fn set_stroke_style_spends_one_step_and_refusals_spend_nothing() {
        let mut core = EditorCore::new();
        draw_line(&mut core);
        let before = witnesses(&core);
        tool_set_stroke_style(&mut core, &json!({"index": 0, "width": 8.0})).unwrap();
        assert_eq!(witnesses(&core), (before.0 + 1, before.1 + 1));
        // A mutating call whose value equals the stored one
        // is still one user edit — exactly one step, never
        // zero and never two (the census verdict for the
        // absolute-value setters).
        let before = witnesses(&core);
        tool_set_stroke_style(&mut core, &json!({"index": 0, "width": 8.0})).unwrap();
        assert_eq!(witnesses(&core), (before.0 + 1, before.1 + 1), "same-value set");
        let before = witnesses(&core);
        assert!(tool_set_stroke_style(&mut core, &json!({"index": 0, "width": 41.0})).is_err());
        assert!(tool_set_stroke_style(&mut core, &json!({"index": 0})).is_err());
        assert!(tool_set_stroke_style(&mut core, &json!({"index": 9, "width": 4.0})).is_err());
        assert_eq!(witnesses(&core), before, "refusals spend nothing");
    }

    #[test]
    fn simplify_stroke_marks_dirty_only_when_it_changes() {
        let mut core = EditorCore::new();
        tool_draw_stroke(
            &mut core,
            &json!({
                "frame": 0,
                "points": [[0.0, 0.0], [10.0, 0.4], [20.0, -0.3], [30.0, 0.2], [40.0, 0.0]],
            }),
        )
        .unwrap();
        let before = witnesses(&core);
        let out = tool_simplify_stroke(&mut core, &json!({"index": 0, "epsilon": 1.0})).unwrap();
        assert_eq!(out["simplified"], true);
        assert_eq!(witnesses(&core), (before.0 + 1, before.1 + 1), "a real simplify");
        let before = witnesses(&core);
        let out = tool_simplify_stroke(&mut core, &json!({"index": 0, "epsilon": 1.0})).unwrap();
        assert_eq!(out["simplified"], false);
        assert_eq!(witnesses(&core), before, "already simple: no step, no dirty");
        assert!(tool_simplify_stroke(&mut core, &json!({"index": 9, "epsilon": 1.0})).is_err());
        assert_eq!(witnesses(&core), before, "bad index spends nothing");
    }

    #[test]
    fn fill_and_lasso_spend_one_step_and_refusals_spend_nothing() {
        let mut core = EditorCore::new();
        let before = witnesses(&core);
        let err = tool_fill_stroke(&mut core, &json!({})).unwrap_err();
        assert!(err.contains("Nothing to fill"), "{err}");
        assert_eq!(witnesses(&core), before, "nothing-to-fill spends nothing");
        tool_draw_stroke(
            &mut core,
            &json!({
                "frame": 0,
                "points": [[0.0, 0.0], [100.0, 0.0], [50.0, 80.0], [0.0, 0.0]],
            }),
        )
        .unwrap();
        let before = witnesses(&core);
        tool_fill_stroke(&mut core, &json!({})).unwrap();
        assert_eq!(witnesses(&core), (before.0 + 1, before.1 + 1), "fill via the core op");
        // A lasso around nothing is a refusal, not a step.
        let before = witnesses(&core);
        let err = tool_lasso_erase(
            &mut core,
            &json!({"points": [[500.0, 500.0], [600.0, 500.0], [600.0, 600.0], [500.0, 600.0]]}),
        )
        .unwrap_err();
        assert!(err.contains("erased nothing"), "{err}");
        assert_eq!(witnesses(&core), before, "empty lasso spends nothing");
        let before = witnesses(&core);
        tool_lasso_erase(
            &mut core,
            &json!({"points": [[-10.0, -10.0], [200.0, -10.0], [200.0, 200.0], [-10.0, 200.0]]}),
        )
        .unwrap();
        assert_eq!(witnesses(&core), (before.0 + 1, before.1 + 1), "lasso via the core op");
    }

    #[test]
    fn mirror_clear_delete_all_and_recolor_spend_only_on_real_changes() {
        let mut core = EditorCore::new();
        // Empty project: every bulk op refuses or reports
        // zero WITHOUT a step or a dirty mark.
        let before = witnesses(&core);
        assert!(tool_mirror_strokes(&mut core, &json!({})).is_err());
        let out = tool_clear_frame_strokes(&mut core, &json!({})).unwrap();
        assert_eq!(out["cleared"], 0);
        let out = tool_delete_all_strokes(&mut core, &json!({})).unwrap();
        assert_eq!(out["deleted"], 0);
        let out = tool_recolor_strokes(&mut core, &json!({"color": 0x112233u32})).unwrap();
        assert_eq!(out["recolored"], 0);
        assert_eq!(witnesses(&core), before, "empty-project bulk ops spend nothing");
        // With a stroke present, each is exactly one step.
        draw_line(&mut core);
        let before = witnesses(&core);
        tool_mirror_strokes(&mut core, &json!({})).unwrap();
        assert_eq!(witnesses(&core), (before.0 + 1, before.1 + 1), "mirror via the core op");
        let before = witnesses(&core);
        tool_recolor_strokes(&mut core, &json!({"color": 0x445566u32})).unwrap();
        assert_eq!(witnesses(&core), (before.0 + 1, before.1 + 1), "recolor");
        let before = witnesses(&core);
        tool_clear_frame_strokes(&mut core, &json!({})).unwrap();
        assert_eq!(witnesses(&core), (before.0 + 1, before.1 + 1), "clear frame");
        draw_line(&mut core);
        let before = witnesses(&core);
        tool_delete_all_strokes(&mut core, &json!({})).unwrap();
        assert_eq!(witnesses(&core), (before.0 + 1, before.1 + 1), "delete all");
    }

    #[test]
    fn session_tools_and_reads_spend_nothing() {
        // The brush / draw-tool family writes editor session
        // state (brush_index, draw_tool) — not part of the
        // saved project — so the writes provably happen
        // while neither witness moves. The read family
        // trivially spends nothing.
        let mut core = EditorCore::new();
        draw_line(&mut core);
        let before = witnesses(&core);
        tool_list_strokes(&mut core, &json!({})).unwrap();
        tool_get_drawing_stats(&mut core, &json!({})).unwrap();
        tool_list_brushes(&mut core, &json!({})).unwrap();
        tool_set_brush(&mut core, &json!({"name": "spray"})).unwrap();
        assert_eq!(core.active_brush().name, "Spray", "the session write happened");
        tool_cycle_brush(&mut core, &json!({})).unwrap();
        assert_eq!(core.active_brush().name, "Pen", "cycle wrapped to the first preset");
        tool_set_draw_tool(&mut core, &json!({"tool": "eraser"})).unwrap();
        assert!(core.draw_tool_is_eraser(), "the tool write happened");
        assert_eq!(
            witnesses(&core),
            before,
            "session writes and reads: no undo step, no dirty mark"
        );
    }
}
