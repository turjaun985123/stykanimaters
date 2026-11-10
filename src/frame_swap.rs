//! Frame swapping: exchange the CONTENT of two timeline
//! frames while every index stays where it is.
//!
//! The editor could already reverse a range of frames
//! (`EditorCore::reverse_range`) but had no way to trade two
//! frames' places. [`swap_frames`] is that operation, and
//! [`swap_frame_with_neighbour`] is its one-step form (swap
//! the current frame with the one before or after it), the
//! shape a frame-ops panel or a command-palette entry calls.
//!
//! ## Semantics
//!
//! * **Content moves, indices do not.** `project.frames[a]`
//!   and `project.frames[b]` trade places wholesale — poses,
//!   duration, easing and bends all live inside [`Frame`], so
//!   they travel together. The playhead (`core.current`),
//!   the loop range and every selection keep naming the same
//!   slot they named before; only what sits IN the slots
//!   changes.
//! * **Attached data follows its frame.** Everything in the
//!   project that is pinned to a frame index is remapped, so
//!   a camera key, a particle burst or a graph key that
//!   belonged to frame a's content now sits on frame b and
//!   vice versa. The full census (each entry verified in
//!   `model.rs` / `vfx.rs` / `graph.rs`):
//!   - `camera` — `CameraKey.frame: usize`
//!   - `particles` — `ParticleEvent.frame: usize`
//!   - `strokes` — `Stroke.frame: usize`
//!   - `audio` — `AudioClip.start_frame: usize`
//!   - `fx_clips` — `FxClipEntry.clip.start: f32` (swapped
//!     when it equals `a`/`b` exactly)
//!   - `channels` — every graph `Key.frame: f32`
//!   - `texts`, `sprites` — every `InstanceKey.frame: f32`
//! * **Order invariants are restored, not assumed.** Camera
//!   keys are kept sorted by frame (`Project::set_camera_key`
//!   sorts on insert), graph channels rely on ascending keys
//!   (`Channel::sort_keys`), and instance key lists are
//!   sampled in ascending order (`InstanceKey::sample`), so
//!   each of those lists is re-sorted after its frames move.
//!   The remaining collections are matched by frame wherever
//!   they are read, so their stored order does not matter.
//! * **What is NOT frame-indexed** (verified in `model.rs`):
//!   `clips` (`ClipDef.frames` is a clip-internal pose
//!   sequence, not timeline indices), `figures`
//!   (`FigureInstance.clip` indexes `Project::clips`),
//!   `assets` (bare image data), `camera_follow` (a figure
//!   index) and `weather` (continuous over the whole
//!   timeline). None of those are touched.
//! * **Media references cannot be scrambled by a swap**
//!   (Wave 12, C8 audit; pinned by
//!   `swap_preserves_media_and_table_references`). The only
//!   fields this module ever writes are frame pins —
//!   `frame` / `start_frame` / `start` / `Key.frame` /
//!   `InstanceKey.frame` — and the [`Frame`] values
//!   themselves. A sprite's `asset` index
//!   (`SpriteInstance.asset`), a particle's or FX clip's
//!   `figure` / `node` indices, a graph channel's `figure`
//!   and `camera_follow` are never written, and the tables
//!   they index (`assets`, `figures`, `clips`) never move,
//!   so every such index means the same thing after the swap
//!   as before. Segment media (`SegmentMedia::Image`) lives
//!   on figure DEFS (`NodeDef.media`, on `FigureInstance.def`
//!   and `ClipDef.def`), not on frames, so it is outside the
//!   swap entirely; a movieclip's own `ClipDef.frames` are a
//!   clip-local pose sequence holding no asset references at
//!   all. And because `Frame.bends` (outer index = figure)
//!   travels inside the same [`Frame`] value as
//!   `Frame.poses`, a frame's bend rows can never be
//!   separated from the poses they belong to.
//!
//! ## Instance keys on timeline-length edits (Wave 13, C7 verdict)
//!
//! Sprite and text instance keys ARE frame-pinned data —
//! they sit in the swap census above, and [`swap_frames`]
//! remaps them like everything else pinned. The question
//! Wave 12's C8 audit left open is whether they also track
//! timeline-LENGTH edits — frames being inserted or
//! deleted — and the answer, pinned by the tests
//! `insert_frames_keep_instance_keys_and_all_pins_absolute`,
//! `add_keeps_all_pins_absolute_delete_frame_remaps_only_the_camera` and
//! `delete_range_remaps_only_the_camera_and_leaves_instance_keys_absolute`,
//! is **no — deliberately, and exactly as every other
//! pinned collection behaves**:
//!
//! * Every length-changing path leaves ALL frame pins at
//!   their stored frame numbers: `EditorCore::add_frame` /
//!   `duplicate_frame`, `EditorCore::delete_frame`,
//!   `EditorCore::insert_inbetween` and
//!   `EditorCore::paste_frame` (editor.rs),
//!   `insert_breakdowns` (this file, Roadmap Wave 1
//!   RM17) and `insert_breakdown_chain` (this file,
//!   Roadmap Wave 2 W2-38),
//!   `EditorCore::delete_range` and
//!   `EditorCore::paste_range` (whose own doc comments
//!   state the rule outright), `shift_frames` /
//!   `reverse_range` (frame VALUES move; pins do not), and
//!   both copies of `insert_frames_after_current` — this
//!   file's, and the frame-ops panel's, whose census
//!   paragraph names `sprites` and `texts` among the
//!   "sparse collections whose entries carry their own
//!   absolute frame numbers". (End-appends — `add_frame`'s
//!   zero-frame recovery, `fig3_bake_animation`'s
//!   held-frame padding — cannot move a pin by
//!   construction: no existing frame changes index.)
//!   Only the editor's navigation state — `current` and
//!   the loop range — is remapped on those paths. Frame
//!   pins in this editor are
//!   **timeline-absolute**: a pin names a position on the
//!   timeline, not a frame's content. The swap is the one
//!   operation that moves pins, because exchanging two
//!   frames' contents is its whole purpose.
//! * Nothing corrupts when a pin's frame stops existing.
//!   Deleting frames never deletes pins (a stroke or a
//!   camera key on a deleted frame survives too), and
//!   `InstanceKey::sample` clamps at both ends of a key
//!   list, so a sprite/text key left beyond a shortened
//!   timeline's end keeps resolving to its stored transform
//!   — the instance holds its keyed pose instead of
//!   vanishing or wrapping. What the author sees after an
//!   insert or delete is every pin — camera, particles,
//!   strokes, audio, FX, graph, sprites, texts alike —
//!   holding its timeline position while frame contents
//!   move underneath it; re-timing pins after a length
//!   edit is a manual pass today, uniformly across the
//!   whole pinned census.
//! * If the house rule ever flips, it must flip for the
//!   whole pinned census at once, and the work lives outside
//!   this file: in `EditorCore`'s frame-length paths
//!   (editor.rs — `add_frame`, `delete_frame`,
//!   `delete_range`, `paste_range`, `paste_frame`,
//!   `insert_inbetween` and their `remap_after_frame_*`
//!   helpers) and in both `insert_frames_after_current`
//!   copies. Shifting ONLY
//!   instance keys from here would make sprites and texts
//!   disagree with the camera, audio and graph keys they
//!   were authored against — a worse misalignment than the
//!   uniform rule. The pinning tests below therefore assert
//!   against all the collections at once: a future flip
//!   fails them together, in the open, instead of drifting
//!   one collection at a time.
//!
//! One exception now exists, by the roadmap's explicit
//! order (RM18, P-6b): CAMERA keys follow the delete /
//! shift / reverse / paste mappings in `EditorCore`'s
//! five ops, because a camera move detached from its
//! action is invisible until export. The other six
//! collections keep the timeline-absolute rule above,
//! unchanged; flipping them is a separate decision, not
//! a consequence of this one.
//!
//! ## Undo discipline
//!
//! The house rules, as in `graph_keys` / `fx_ops`: a refusal
//! (same frame twice, an index out of range, a neighbour
//! swap at either end of the timeline) sets the status line,
//! returns `false` and takes **no snapshot**; a success
//! snapshots exactly once, before mutating, through
//! [`EditorCore::snapshot`], so one undo restores both
//! frames and every remapped collection together.

use crate::editor::EditorCore;
use crate::model::{Frame, InstanceKey};
use serde_json::{json, Value};

/// Trade a `usize` frame index between `a` and `b`: an index
/// equal to `a` becomes `b`, one equal to `b` becomes `a`,
/// anything else is left alone.
fn swap_usize_frame(frame: &mut usize, a: usize, b: usize) {
    if *frame == a {
        *frame = b;
    } else if *frame == b {
        *frame = a;
    }
}

/// Trade an `f32` frame position between `a` and `b`, the
/// float counterpart of [`swap_usize_frame`]: a position
/// exactly equal to `a as f32` becomes `b as f32` and vice
/// versa. Frame-pinned floats (clip starts, graph keys,
/// instance keys) sit on whole frames, so the exact compare
/// is the right test — a fractional position between the two
/// frames belongs to neither and does not move.
fn swap_f32_frame(frame: &mut f32, a: usize, b: usize) {
    if *frame == a as f32 {
        *frame = b as f32;
    } else if *frame == b as f32 {
        *frame = a as f32;
    }
}

/// Restore the ascending-frame order `InstanceKey::sample`
/// walks, after a swap may have traded two keys' frames.
/// Stable, so keys that share a frame keep their order.
fn sort_instance_keys(keys: &mut [InstanceKey]) {
    keys.sort_by(|x, y| {
        x.frame
            .partial_cmp(&y.frame)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

/// Exchange the content of frames `a` and `b` (0-based
/// indices into `project.frames`): the two [`Frame`] values
/// trade places wholesale, and every frame-pinned collection
/// in the project is remapped so attached data follows the
/// content it was authored for (see the module docs for the
/// census). The playhead and every other index stay
/// numerically unchanged. On success exactly one snapshot is
/// taken and the status line reads "Swapped frames {a+1} and
/// {b+1}."; returns `true`.
///
/// Returns `false` — no snapshot, project untouched — when
/// either index is out of range or the two indices are the
/// same frame; the status line says which, with 1-based
/// numbers, so a UI binding never fails silently.
///
/// [`Frame`]: crate::model::Frame
pub fn swap_frames(core: &mut EditorCore, a: usize, b: usize) -> bool {
    let count = core.project.frames.len();
    if a >= count {
        core.status = format!("No frame {} to swap.", a + 1);
        return false;
    }
    if b >= count {
        core.status = format!("No frame {} to swap.", b + 1);
        return false;
    }
    if a == b {
        core.status = format!("Frame {} cannot be swapped with itself.", a + 1);
        return false;
    }
    core.snapshot_labeled("Swap frames");
    let project = &mut core.project;
    project.frames.swap(a, b);
    // Camera keys: remapped, then re-sorted — the list is
    // kept in ascending frame order everywhere else.
    for key in &mut project.camera {
        swap_usize_frame(&mut key.frame, a, b);
    }
    project.camera.sort_by_key(|k| k.frame);
    for event in &mut project.particles {
        swap_usize_frame(&mut event.frame, a, b);
    }
    for stroke in &mut project.strokes {
        swap_usize_frame(&mut stroke.frame, a, b);
    }
    for clip in &mut project.audio {
        swap_usize_frame(&mut clip.start_frame, a, b);
    }
    for entry in &mut project.fx_clips {
        swap_f32_frame(&mut entry.clip.start, a, b);
    }
    // Graph channels: remapped, then re-sorted — `value_at`
    // relies on ascending keys (see `Channel::sort_keys`).
    for channel in &mut project.channels {
        for key in &mut channel.keys {
            swap_f32_frame(&mut key.frame, a, b);
        }
        channel.sort_keys();
    }
    // Text and sprite instances: remapped, then re-sorted —
    // `InstanceKey::sample` walks the list in ascending
    // frame order.
    for text in &mut project.texts {
        for key in &mut text.keys {
            swap_f32_frame(&mut key.frame, a, b);
        }
        sort_instance_keys(&mut text.keys);
    }
    for sprite in &mut project.sprites {
        for key in &mut sprite.keys {
            swap_f32_frame(&mut key.frame, a, b);
        }
        sort_instance_keys(&mut sprite.keys);
    }
    core.status = format!("Swapped frames {} and {}.", a + 1, b + 1);
    true
}

/// Swap the current frame with its neighbour: `dir` −1
/// trades it with the frame before it, +1 with the frame
/// after it — [`swap_frames`] on (`current`, `current + dir`)
/// once the move is known to exist, so the swap itself, its
/// remap census and its status line are exactly
/// `swap_frames`'. The playhead stays on the same index: the
/// frame the user was looking at is now the neighbour's
/// content, which is what "move this frame one step" means
/// in a content-swapping editor.
///
/// Returns `false` — no snapshot, project untouched — at
/// either end of the timeline (the status line says the
/// current frame is the first/last and has no neighbour),
/// and for `dir` 0, which is delegated to `swap_frames`' own
/// same-frame refusal.
pub fn swap_frame_with_neighbour(core: &mut EditorCore, dir: i32) -> bool {
    if dir == 0 {
        // No direction is a same-frame swap; let
        // `swap_frames` phrase the refusal (it also covers a
        // stale playhead on an empty timeline).
        let current = core.current;
        return swap_frames(core, current, current);
    }
    let count = core.project.frames.len();
    let current = core.current;
    let target = current as i64 + dir as i64;
    if target < 0 {
        core.status = format!(
            "Frame {} is the first frame — no neighbour to swap with.",
            current + 1
        );
        return false;
    }
    if target >= count as i64 {
        core.status = format!(
            "Frame {} is the last frame — no neighbour to swap with.",
            current + 1
        );
        return false;
    }
    swap_frames(core, current, target as usize)
}

// ---------------------------------------------------------------------
// Wave 11 (B2): MCP frame/timeline parity — the operations the
// frame-ops panel and the command palette expose that had no MCP
// surface, plus the timeline read side. Handlers follow the
// registry idioms of `mcp_tools` / the `mcp.rs` built-ins:
// explicit index arguments, the same argument-error wording,
// refusals surfaced as `Err(core.status)` where the operation
// reports them through the status line, and an `ok` object on
// success. None of them can panic: indices are validated or
// read through `get`, and the one arithmetic input (a count) is
// clamped before it sizes an allocation.
// ---------------------------------------------------------------------

/// Largest batch [`insert_frames_after_current`] will insert —
/// the frame-ops panel's `INSERT_COUNT_MAX`
/// (`rune_sec/frame_ops.rs`), mirrored: a whole minute of
/// frames at the demo's 12 fps is 720; 60 keeps one accidental
/// call recoverable in one undo.
pub const INSERT_FRAMES_MAX: usize = 60;

/// Argument helper mirroring `mcp::arg_usize` (a JSON null
/// counts as absent; integral floats are accepted). Local to
/// this module because neither `mcp`'s nor `mcp_tools`' helper
/// is public.
fn fs_arg_usize(args: &Value, key: &str) -> Result<usize, String> {
    let Some(v) = args.get(key).filter(|v| !v.is_null()) else {
        return Err(format!("missing required argument '{key}'"));
    };
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

/// Float argument helper for the Wave 14 handler below,
/// the finite counterpart of [`fs_arg_usize`]: the value
/// must be a JSON number whose f32 cast stays finite, so
/// a huge double (which casts to infinity) is refused up
/// front, never stored.
fn fs_arg_f32(args: &Value, key: &str) -> Result<f32, String> {
    let Some(v) = args.get(key).filter(|v| !v.is_null()) else {
        return Err(format!("missing required argument '{key}'"));
    };
    let f = v
        .as_f64()
        .map(|f| f as f32)
        .ok_or_else(|| format!("argument '{key}' must be a number"))?;
    if !f.is_finite() {
        return Err(format!("argument '{key}' must be a finite number"));
    }
    Ok(f)
}

/// Figure-index guard mirroring `mcp::check_figure`'s wording.
fn fs_check_figure(core: &EditorCore, fi: usize) -> Result<(), String> {
    if fi < core.project.figures.len() {
        Ok(())
    } else {
        Err(format!(
            "figure index {fi} out of range (project has {} figure(s))",
            core.project.figures.len()
        ))
    }
}

/// Insert `count` blank frames directly after the current
/// frame, as ONE undo step, leaving `current` on the original
/// frame — the library twin of the frame-ops panel's batch
/// insert (`rune_sec::frame_ops::insert_frames_after_current`,
/// Phase G), which the binary crate keeps locally because the
/// shell cannot call back into a panel-shaped helper either
/// way. The semantics are the panel's, exactly:
///
/// * "Blank" is the frame one press of `EditorCore::add_frame`
///   creates — `poses` and `duration` cloned from the current
///   frame, `easing: None`, `bends` emptied — repeated, so the
///   result is content-identical to N single presses and
///   differs only in undo granularity and in `current` staying
///   put. Frame-pinned collections (camera keys, particles,
///   …) are NOT remapped, matching `add_frame` — instance
///   keys included; see the module docs' "Instance keys on
///   timeline-length edits" section for the pinned verdict
///   and why.
/// * `count` is clamped to `1..=INSERT_FRAMES_MAX`, the
///   panel's `clamp_insert_count` range.
///
/// Returns how many frames were inserted: 0 — no snapshot,
/// status line set — when there is no current frame (an empty
/// project, or a stale playhead past the end).
pub fn insert_frames_after_current(core: &mut EditorCore, count: usize) -> usize {
    let n = count.clamp(1, INSERT_FRAMES_MAX);
    let Some(source) = core.project.frames.get(core.current) else {
        core.status = "No frame to insert after.".into();
        return 0;
    };
    // Exactly the Frame value `add_frame` constructs (see the
    // doc comment): poses and duration cloned from the current
    // frame, easing reset to None, bends emptied.
    let blank = Frame {
        poses: source.poses.clone(),
        duration: source.duration,
        easing: None,
        bends: Vec::new(),
    };
    let at = core.current + 1;
    let after = core.current;
    core.snapshot_labeled("Insert frames");
    let blanks: Vec<Frame> = (0..n).map(|_| blank.clone()).collect();
    // Collecting the Splice forces the replacement to
    // complete; the replaced range is empty, so nothing is
    // removed.
    let _removed: Vec<Frame> = core.project.frames.splice(at..at, blanks).collect();
    core.status = format!("Inserted {n} frames after frame {}.", after + 1);
    n
}

/// MCP handler for [`swap_frame_with_neighbour`]: the Frames
/// panel's "Swap with previous" / "Swap with next" buttons and
/// the palette's swap-frame-prev / swap-frame-next commands —
/// until now reachable only as a two-step `swap_frames` call
/// (read the playhead, then swap the pair), with none of this
/// op's own end-of-timeline refusals. Registered by the
/// integrator as `swap_frame_with_neighbour`.
pub fn tool_swap_frame_with_neighbour(
    core: &mut EditorCore,
    args: &Value,
) -> Result<Value, String> {
    let dir = match args.get("direction").filter(|v| !v.is_null()) {
        None => return Err("missing required argument 'direction'".to_string()),
        Some(v) => match v.as_str() {
            Some("previous") => -1,
            Some("next") => 1,
            Some(other) => {
                return Err(format!(
                    "unknown direction '{other}' (expected \"previous\" or \"next\")"
                ));
            }
            None => {
                return Err("argument 'direction' must be a string".to_string());
            }
        },
    };
    if !swap_frame_with_neighbour(core, dir) {
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "current_frame": core.current,
        "status": core.status,
    }))
}

/// MCP handler for [`insert_frames_after_current`]: the
/// frame-ops panel's Insert-N row. Registered by the
/// integrator as `insert_frames`.
pub fn tool_insert_frames(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let count = fs_arg_usize(args, "count")?;
    if count == 0 {
        return Err("argument 'count' must be at least 1".to_string());
    }
    let inserted = insert_frames_after_current(core, count);
    if inserted == 0 {
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "inserted": inserted,
        "frame_count": core.project.frames.len(),
        "current_frame": core.current,
        "status": core.status,
    }))
}

/// Smallest breakdown favour [`insert_breakdowns`] will
/// use: a breakdown at (or beyond) either key is a copy of
/// that key, not a breakdown. The 0.1 floor is audit A4
/// R1's slider range.
pub const BREAKDOWN_FAVOUR_MIN: f32 = 0.1;
/// Largest breakdown favour — see [`BREAKDOWN_FAVOUR_MIN`].
pub const BREAKDOWN_FAVOUR_MAX: f32 = 0.9;
/// Most breakdowns one [`insert_breakdowns`] call authors.
/// Between two keys, eight spacing samples is a generous
/// single gesture; the insert is one undo step either way,
/// so the cap is about intent, not recoverability.
pub const BREAKDOWN_COUNT_MAX: usize = 8;

/// The segment fractions at which [`insert_breakdowns`]
/// samples, for `count` breakdowns biased by `favour`
/// (already clamped to
/// [`BREAKDOWN_FAVOUR_MIN`]..=[`BREAKDOWN_FAVOUR_MAX`]).
/// Even fractions `u = k / (count + 1)` are warped by the
/// power `gamma = ln(favour) / ln(0.5)`, chosen so the
/// middle of the parameter range maps exactly onto
/// `favour`: favour 0.5 is gamma 1 (even spacing — the
/// single breakdown lands at the midpoint, the legacy
/// `insert_inbetween` point), favour below 0.5 is
/// gamma > 1 (samples crowd toward the current key),
/// above is gamma < 1 (toward the next key). With
/// `count == 1` the single fraction IS `favour`.
/// Strictly increasing, always inside (0, 1).
pub fn breakdown_fractions(favour: f32, count: usize) -> Vec<f32> {
    let gamma = favour.ln() / 0.5f32.ln();
    (1..=count)
        .map(|k| (k as f32 / (count + 1) as f32).powf(gamma))
        .collect()
}

/// One breakdown frame's bend rows: each figure's row
/// lerps between the two frames' rows at the RAW
/// fraction `u` (`Project::bends_at`'s rule — bends
/// carry no easing), a missing row reading as zeros
/// padded to the figure's node count. When neither
/// frame carries bend data at all the result is empty,
/// preserving `insert_inbetween`'s `bends: Vec::new()`
/// representation exactly.
fn breakdown_bends(a: &Frame, b: &Frame, node_counts: &[usize], u: f32) -> Vec<Vec<f32>> {
    if a.bends.is_empty() && b.bends.is_empty() {
        return Vec::new();
    }
    let rows = a.bends.len().max(b.bends.len());
    (0..rows)
        .map(|fi| {
            let n = node_counts.get(fi).copied().unwrap_or(0);
            let mut ra = a.bends.get(fi).cloned().unwrap_or_default();
            let mut rb = b.bends.get(fi).cloned().unwrap_or_default();
            ra.resize(n, 0.0);
            rb.resize(n, 0.0);
            ra.iter().zip(rb.iter()).map(|(x, y)| x + (y - x) * u).collect()
        })
        .collect()
}

/// Insert `count` breakdown frames between the current
/// frame and the next (Roadmap Wave 1, RM17 — roadmap
/// P-5a, audit A4 R1): the favour-biased generalisation
/// of `EditorCore::insert_inbetween`, which stays as
/// the fixed-midpoint special case the palette has
/// always run.
///
/// Each breakdown SAMPLES the segment the way playback
/// shows it: poses lerp at the segment's eased fraction
/// (`frames[current].easing`, else the project easing —
/// the resolution `Project::poses_at` uses), bends lerp
/// at the raw fraction (the resolution
/// `Project::bends_at` uses), so a breakdown frame holds
/// exactly the pose and bends the tween preview showed
/// at that point. Fractions come from
/// [`breakdown_fractions`]; `favour` is clamped to
/// [`BREAKDOWN_FAVOUR_MIN`]..=[`BREAKDOWN_FAVOUR_MAX`]
/// and `count` to 1..=[`BREAKDOWN_COUNT_MAX`] (the
/// `insert_frames_after_current` clamp discipline).
///
/// Inserted frames are authored frames: duration 1.0,
/// easing None — `insert_inbetween`'s construction. The
/// segment's easing override stays on the current frame
/// and governs the first sub-segment from it; the
/// breakdown poses themselves carry the shaped spacing.
/// Frame pins are NOT remapped (the module docs'
/// timeline-length verdict); only `current` moves, onto
/// the first inserted breakdown.
///
/// Returns how many frames were inserted: 0 — status
/// set, NO snapshot — when there is no following frame
/// (the `insert_inbetween` refusal) or `favour` is not
/// finite. On success exactly one snapshot covers the
/// whole insert.
pub fn insert_breakdowns(core: &mut EditorCore, favour: f32, count: usize) -> usize {
    if !favour.is_finite() {
        core.status = "Breakdown favour must be a finite fraction between 0 and 1.".into();
        return 0;
    }
    let cur = core.current;
    let (Some(a), Some(b)) = (
        core.project.frames.get(cur),
        core.project.frames.get(cur + 1),
    ) else {
        core.status = "Breakdown needs a following frame.".into();
        return 0;
    };
    let a = a.clone();
    let b = b.clone();
    let favour = favour.clamp(BREAKDOWN_FAVOUR_MIN, BREAKDOWN_FAVOUR_MAX);
    let count = count.clamp(1, BREAKDOWN_COUNT_MAX);
    let easing = a.easing.unwrap_or(core.project.easing);
    let node_counts: Vec<usize> = core
        .project
        .figures
        .iter()
        .map(|f| f.def.nodes.len())
        .collect();
    let frames: Vec<Frame> = breakdown_fractions(favour, count)
        .into_iter()
        .map(|u| {
            let t = easing.apply(u);
            let poses = a
                .poses
                .iter()
                .zip(b.poses.iter())
                .map(|(pa, pb)| pa.lerp(pb, t))
                .collect();
            let bends = breakdown_bends(&a, &b, &node_counts, u);
            Frame { poses, duration: 1.0, easing: None, bends }
        })
        .collect();
    core.snapshot_labeled("Insert breakdowns");
    let at = cur + 1;
    let _removed: Vec<Frame> = core.project.frames.splice(at..at, frames).collect();
    core.current = at;
    core.status = if count == 1 {
        format!("Breakdown inserted at frame {}.", core.current + 1)
    } else {
        format!("Inserted {count} breakdowns after frame {}.", cur + 1)
    };
    count
}

/// MCP handler for [`insert_breakdowns`] (Roadmap Wave 1,
/// RM17): the Frames panel's breakdown insert.
/// Registered by the integrator as `insert_breakdown`
/// (see the TOOL ENTRY SPEC below).
pub fn tool_insert_breakdown(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let favour = match args.get("favour").filter(|v| !v.is_null()) {
        Some(_) => {
            let f = fs_arg_f32(args, "favour")?;
            if f <= 0.0 || f >= 1.0 {
                return Err("argument 'favour' must be between 0 and 1 (exclusive)".into());
            }
            f
        }
        None => 0.5,
    };
    let count = match args.get("count").filter(|v| !v.is_null()) {
        Some(_) => {
            let c = fs_arg_usize(args, "count")?;
            if c == 0 {
                return Err("argument 'count' must be at least 1".into());
            }
            c
        }
        None => 1,
    };
    let inserted = insert_breakdowns(core, favour, count);
    if inserted == 0 {
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "inserted": inserted,
        "frame_count": core.project.frames.len(),
        "current_frame": core.current,
        "status": core.status,
    }))
}

// TOOL ENTRY SPEC: add to tools() in src/mcp_tools.rs,
// in the Wave 11 domain-file block beside insert_frames.
//   Tool {
//       name: "insert_breakdown",
//       description: "Insert 'count' breakdown frames between the current frame and the next (the Frames panel's breakdown insert): the breakdowns sample the segment at favour-biased fractions — 'favour' 0.5 spaces them evenly (a single breakdown is the legacy midpoint inbetween), below 0.5 crowds them toward the current key, above 0.5 toward the next — with poses shaped by the segment's easing and bends interpolated linearly, exactly what tween playback shows at those points. 'favour' is clamped to 0.1–0.9 and 'count' to 8 by the op. One undo step; frame pins are not remapped (the timeline-length rule).",
//       schema: obj_schema(
//           json!({
//               "favour": { "type": "number", "description": "Breakdown spacing bias between 0 and 1 (exclusive), default 0.5" },
//               "count": { "type": "integer", "description": "Breakdowns to insert, at least 1 (default 1)" }
//           }),
//           &[],
//       ),
//       handler: crate::frame_swap::tool_insert_breakdown,
//   }
// MCP.md row:
//   | `insert_breakdown` | Insert 'count' breakdown frames between the current frame and the next (the Frames panel's breakdown insert): the breakdowns sample the segment at favour-biased fractions — 'favour' 0.5 spaces them evenly (a single breakdown is the legacy midpoint inbetween), below 0.5 crowds them toward the current key, above 0.5 toward the next — with poses shaped by the segment's easing and bends interpolated linearly, exactly what tween playback shows at those points. 'favour' is clamped to 0.1–0.9 and 'count' to 8 by the op. One undo step; frame pins are not remapped (the timeline-length rule). | — | `count`, `favour` |

/// Most breakdowns one [`insert_breakdown_chain`] call
/// authors across a whole span. The per-segment cap
/// ([`BREAKDOWN_COUNT_MAX`]) bounds one segment's
/// samples; a chain is one gesture across up to
/// [`BREAKDOWN_CHAIN_SPAN_MAX`] segments, so its cap is
/// the span-scale counterpart — about intent, not
/// recoverability (the insert is one undo step either
/// way).
pub const BREAKDOWN_CHAIN_COUNT_MAX: usize = 32;
/// Most segments one [`insert_breakdown_chain`] call
/// spans: the distance from the current frame to the
/// chain's target key. At 24 fps, 64 frames is over two
/// and a half seconds of timeline — past that, a "chain
/// toward one target" is really several moves.
pub const BREAKDOWN_CHAIN_SPAN_MAX: usize = 64;
/// The span the MCP handler and the Frames panel start
/// from: three segments — the shortest span whose even
/// midpoint (favour 0.5, count 1) lands INSIDE a segment
/// rather than exactly on the middle frame.
pub const BREAKDOWN_CHAIN_SPAN_DEFAULT: usize = 3;

/// Where [`insert_breakdown_chain`] samples: the
/// `(segment, local fraction)` of every breakdown the
/// chain authors, in timeline order. The distribution
/// is [`breakdown_fractions`] over the WHOLE span — one
/// favour curve from the current key to the target key
/// `span` segments later — so a global fraction `u`
/// lands at span position `u * span`: segment
/// `floor(u * span)`, at the local fraction left over.
///
/// A position that lands EXACTLY on an existing frame
/// (a whole-number span position — even spacing over a
/// commensurate span does this: favour 0.5, count 3,
/// span 4 puts all three positions on frames) is
/// SKIPPED: that frame already holds a pose at that
/// point of the span, and inserting a sampled copy
/// beside it would author a duplicate, not a breakdown.
/// The Frames panel's chain preview draws exactly these
/// positions against the span's frame ticks, so a
/// skipped position is visible before the press, never
/// a surprise after it.
///
/// `favour` is clamped to
/// [`BREAKDOWN_FAVOUR_MIN`]..=[`BREAKDOWN_FAVOUR_MAX`],
/// `count` to 1..=[`BREAKDOWN_CHAIN_COUNT_MAX`] and
/// `span` to 1..=[`BREAKDOWN_CHAIN_SPAN_MAX`]; a
/// non-finite `favour` yields no positions. Positions
/// are strictly increasing in timeline order, every
/// local fraction inside (0, 1).
pub fn breakdown_chain_positions(favour: f32, count: usize, span: usize) -> Vec<(usize, f32)> {
    if !favour.is_finite() {
        return Vec::new();
    }
    let favour = favour.clamp(BREAKDOWN_FAVOUR_MIN, BREAKDOWN_FAVOUR_MAX);
    let count = count.clamp(1, BREAKDOWN_CHAIN_COUNT_MAX);
    let span = span.clamp(1, BREAKDOWN_CHAIN_SPAN_MAX);
    breakdown_fractions(favour, count)
        .into_iter()
        .filter_map(|u| {
            let g = u * span as f32;
            let seg = g.floor() as usize;
            let local = g - seg as f32;
            if seg >= span || local <= 0.0 {
                None
            } else {
                Some((seg, local))
            }
        })
        .collect()
}

/// Insert a chain of `count` breakdowns across the
/// `span` segments from the current frame toward the
/// target key at `current + span` (Roadmap Wave 2,
/// W2-38 — the board's "breakdowns of breakdowns"):
/// [`insert_breakdowns`] generalised from one segment
/// to a span, under ONE favour curve.
///
/// Each breakdown SAMPLES ITS OWN segment exactly the
/// way [`insert_breakdowns`] samples its one segment —
/// poses at the segment's eased fraction (that
/// segment's frame easing override, else the project
/// easing), bends at the raw fraction — so the chain
/// authors precisely the poses tween playback shows at
/// those span positions, each a real breakdown of the
/// segment it lands in. Positions come from
/// [`breakdown_chain_positions`]: one global
/// distribution; positions landing exactly on existing
/// frames are skipped. With `span == 1` and `count`
/// inside the per-segment cap, the chain authors
/// exactly the frames [`insert_breakdowns`] would —
/// poses, bends, playhead and status (pinned); the two
/// ops differ in reach and in the count cap, not in
/// sampling.
///
/// Clamps and refusals follow the house discipline: a
/// non-finite `favour` refuses pre-snapshot (the
/// [`insert_breakdowns`] sentence); `count` clamps to
/// 1..=[`BREAKDOWN_CHAIN_COUNT_MAX`]; `span` clamps to
/// 1..=[`BREAKDOWN_CHAIN_SPAN_MAX`] and then to the
/// segments the timeline actually has left, so a chain
/// reaching past the last frame covers what exists and
/// the status names the span covered. No following
/// frame — or every position landing on an existing
/// frame — refuses with a status and NO snapshot. On
/// success exactly ONE snapshot covers the whole chain
/// (label "Insert breakdown chain"); the playhead lands
/// on the first inserted breakdown; frame pins are NOT
/// remapped (the module docs' timeline-length verdict).
///
/// Returns how many frames were inserted (0 on a
/// refusal).
pub fn insert_breakdown_chain(core: &mut EditorCore, favour: f32, count: usize, span: usize) -> usize {
    if !favour.is_finite() {
        core.status = "Breakdown favour must be a finite fraction between 0 and 1.".into();
        return 0;
    }
    let cur = core.current;
    let available = core.project.frames.len().saturating_sub(cur + 1);
    if available == 0 {
        core.status = "Breakdown chain needs a following frame.".into();
        return 0;
    }
    let favour = favour.clamp(BREAKDOWN_FAVOUR_MIN, BREAKDOWN_FAVOUR_MAX);
    let count = count.clamp(1, BREAKDOWN_CHAIN_COUNT_MAX);
    let span = span.clamp(1, BREAKDOWN_CHAIN_SPAN_MAX).min(available);
    let positions = breakdown_chain_positions(favour, count, span);
    if positions.is_empty() {
        core.status = "Breakdown chain lands only on existing frames — nothing to insert.".into();
        return 0;
    }
    let node_counts: Vec<usize> = core
        .project
        .figures
        .iter()
        .map(|f| f.def.nodes.len())
        .collect();
    let project_easing = core.project.easing;
    // Sample every new frame from the ORIGINAL segments
    // before anything moves, bucketed by segment; the
    // splice below runs the buckets in reverse so
    // earlier segment indices stay valid throughout.
    let originals: Vec<Frame> = core.project.frames[cur..=cur + span].to_vec();
    let mut by_segment: Vec<Vec<Frame>> = vec![Vec::new(); span];
    for (seg, u) in &positions {
        let a = &originals[*seg];
        let b = &originals[seg + 1];
        let t = a.easing.unwrap_or(project_easing).apply(*u);
        let poses = a
            .poses
            .iter()
            .zip(b.poses.iter())
            .map(|(pa, pb)| pa.lerp(pb, t))
            .collect();
        let bends = breakdown_bends(a, b, &node_counts, *u);
        by_segment[*seg].push(Frame { poses, duration: 1.0, easing: None, bends });
    }
    let inserted = positions.len();
    core.snapshot_labeled("Insert breakdown chain");
    for (seg, frames) in by_segment.iter().enumerate().rev() {
        if frames.is_empty() {
            continue;
        }
        let at = cur + seg + 1;
        let _removed: Vec<Frame> =
            core.project.frames.splice(at..at, frames.iter().cloned()).collect();
    }
    core.current = cur + positions[0].0 + 1;
    core.status = match (inserted, span) {
        (1, 1) => format!("Breakdown inserted at frame {}.", core.current + 1),
        (1, s) => format!("Inserted 1 breakdown across {s} segments after frame {}.", cur + 1),
        (n, 1) => format!("Inserted {n} breakdowns after frame {}.", cur + 1),
        (n, s) => format!("Inserted {n} breakdowns across {s} segments after frame {}.", cur + 1),
    };
    inserted
}

/// MCP handler for [`insert_breakdown_chain`] (Roadmap
/// Wave 2, W2-38): the Frames panel's breakdown chain.
/// Registered by the integrator as
/// `insert_breakdown_chain` (see the TOOL ENTRY SPEC
/// below — IA-W2-38-1).
pub fn tool_insert_breakdown_chain(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let favour = match args.get("favour").filter(|v| !v.is_null()) {
        Some(_) => {
            let f = fs_arg_f32(args, "favour")?;
            if f <= 0.0 || f >= 1.0 {
                return Err("argument 'favour' must be between 0 and 1 (exclusive)".into());
            }
            f
        }
        None => 0.5,
    };
    let count = match args.get("count").filter(|v| !v.is_null()) {
        Some(_) => {
            let c = fs_arg_usize(args, "count")?;
            if c == 0 {
                return Err("argument 'count' must be at least 1".into());
            }
            c
        }
        None => 1,
    };
    let span = match args.get("span").filter(|v| !v.is_null()) {
        Some(_) => {
            let s = fs_arg_usize(args, "span")?;
            if s == 0 {
                return Err("argument 'span' must be at least 1".into());
            }
            s
        }
        None => BREAKDOWN_CHAIN_SPAN_DEFAULT,
    };
    let inserted = insert_breakdown_chain(core, favour, count, span);
    if inserted == 0 {
        return Err(core.status.clone());
    }
    Ok(json!({
        "ok": true,
        "inserted": inserted,
        "frame_count": core.project.frames.len(),
        "current_frame": core.current,
        "status": core.status,
    }))
}

// TOOL ENTRY SPEC: add to tools() in src/mcp_tools.rs,
// in the Wave 11 domain-file block immediately AFTER
// the insert_breakdown Tool entry (executed by the
// integrator as IA-W2-38-1 — the registry and
// docs/MCP.md are integrator surfaces; the census takes
// the surface 396 -> 397 and check 9's doc sync needs
// the catalogue row in the same push).
//   Tool {
//       name: "insert_breakdown_chain",
//       description: "Insert a chain of 'count' breakdown frames across the 'span' segments from the current frame toward the target key 'span' frames later (the Frames panel's breakdown chain): one favour curve distributes the breakdowns over the whole span — 'favour' 0.5 spaces them evenly, below 0.5 crowds them toward the current key, above 0.5 toward the target — and each breakdown samples its own segment the way insert_breakdown does (poses at the segment's eased fraction, bends interpolated linearly). A distribution position landing exactly on an existing frame is skipped — that pose already exists as a frame. 'favour' is clamped to 0.1–0.9, 'count' to 32 and 'span' to 64 (and to the segments remaining) by the op. One undo step; frame pins are not remapped (the timeline-length rule).",
//       schema: obj_schema(
//           json!({
//               "favour": { "type": "number", "description": "Breakdown spacing bias between 0 and 1 (exclusive), default 0.5" },
//               "count": { "type": "integer", "description": "Breakdowns to insert across the span, at least 1 (default 1)" },
//               "span": { "type": "integer", "description": "Segments from the current frame to the target key, at least 1 (default 3)" }
//           }),
//           &[],
//       ),
//       handler: crate::frame_swap::tool_insert_breakdown_chain,
//   }
// MCP.md row (beside the insert_breakdown row):
//   | `insert_breakdown_chain` | Insert a chain of 'count' breakdown frames across the 'span' segments from the current frame toward the target key 'span' frames later (the Frames panel's breakdown chain): one favour curve distributes the breakdowns over the whole span — 'favour' 0.5 spaces them evenly, below 0.5 crowds them toward the current key, above 0.5 toward the target — and each breakdown samples its own segment the way insert_breakdown does (poses at the segment's eased fraction, bends interpolated linearly). A distribution position landing exactly on an existing frame is skipped — that pose already exists as a frame. 'favour' is clamped to 0.1–0.9, 'count' to 32 and 'span' to 64 (and to the segments remaining) by the op. One undo step; frame pins are not remapped (the timeline-length rule). | — | `count`, `favour`, `span` |

// TOOL ENTRY SPEC: add to tools() in src/mcp_tools.rs,
// in the Wave 11 domain-file block beside insert_frames.
//   Tool {
//       name: "adjust_frame_duration",
//       description: "Adjust a frame's duration multiplier by 'factor' (the frame-ops panel's Dur −/+/×0.5/×2 buttons): the stored duration is multiplied and clamped to 0.25-8.0 by the op, and the reply reports the duration read back after clamping. The relative form of set_frame_duration. Applies to the current frame unless 'frame' names another (which moves the playhead there first, the set_frame_easing convention). One undo step on a real change.",
//       schema: obj_schema(
//           json!({
//               "factor": { "type": "number", "description": "Duration multiplier (positive, finite); 0.5 halves the hold, 2.0 doubles it" },
//               "frame": { "type": "integer", "description": "0-based frame index (default: the current frame)" }
//           }),
//           &["factor"],
//       ),
//       handler: crate::frame_swap::tool_adjust_frame_duration,
//   }
// MCP.md row:
//   | `adjust_frame_duration` | Adjust a frame's duration multiplier by 'factor' (the frame-ops panel's Dur −/+/×0.5/×2 buttons): the stored duration is multiplied and clamped to 0.25–8.0 by the op; the reply reports the duration read back, so a step past a bound truthfully reports the bound. The relative form of `set_frame_duration`. Applies to the current frame unless 'frame' names another (which moves the playhead there first). One undo step on a real change. | `factor` | `frame` |
/// MCP handler for [`EditorCore::adjust_frame_duration`]
/// (Wave 14, A2): the frame-ops panel's relative duration
/// buttons. The op reads `frames[current]` directly, so
/// the handler refuses an empty project and an
/// out-of-range frame BEFORE anything runs — the op
/// itself would index-panic on either. Validation order
/// is factor, then frame, then the op.
pub fn tool_adjust_frame_duration(
    core: &mut EditorCore,
    args: &Value,
) -> Result<Value, String> {
    let factor = fs_arg_f32(args, "factor")?;
    if factor <= 0.0 {
        return Err("argument 'factor' must be positive".to_string());
    }
    let frame = match args.get("frame").filter(|v| !v.is_null()) {
        Some(_) => fs_arg_usize(args, "frame")?,
        None => core.current,
    };
    if frame >= core.project.frames.len() {
        return Err(format!(
            "frame index {frame} out of range (project has {} frame(s))",
            core.project.frames.len()
        ));
    }
    if frame != core.current {
        core.goto(frame);
    }
    core.adjust_frame_duration(factor);
    Ok(json!({
        "ok": true,
        "frame": frame,
        "duration": core.project.frames[frame].duration,
        "status": core.status,
    }))
}

/// MCP read handler: one figure's keyframe list — the exact
/// data the dopesheet lane draws (`stats::figure_keyframes`).
/// The `goto_next_keyframe` / `goto_prev_keyframe` built-ins
/// only reveal the list after a jump that succeeds; this is
/// the read on its own. Read-only: no snapshot, no mutation.
/// Registered by the integrator as `get_keyframes`.
pub fn tool_get_keyframes(core: &mut EditorCore, args: &Value) -> Result<Value, String> {
    let fi = fs_arg_usize(args, "figure")?;
    fs_check_figure(core, fi)?;
    Ok(json!({
        "ok": true,
        "figure": fi,
        "key_frames": crate::stats::figure_keyframes(&core.project, fi),
        "current_frame": core.current,
        "frame_count": core.project.frames.len(),
    }))
}

/// MCP read handler: the timeline's per-frame timing table —
/// every frame's hold-time multiplier and easing override,
/// plus the playhead and loop range. The write side
/// (`set_frame_duration`, `set_frame_durations`,
/// `set_frame_easing`) had no read counterpart: a client
/// could set durations blind but never read them back.
/// Read-only: no snapshot, no mutation. Registered by the
/// integrator as `list_frames`.
pub fn tool_list_frames(core: &mut EditorCore, _args: &Value) -> Result<Value, String> {
    let frames: Vec<Value> = core
        .project
        .frames
        .iter()
        .enumerate()
        .map(|(i, f)| {
            json!({
                "frame": i,
                "duration": f.duration,
                "easing": f.easing.map(|e| e.label()),
            })
        })
        .collect();
    let loop_range = match core.loop_range {
        Some((a, b)) => json!([a, b]),
        None => Value::Null,
    };
    Ok(json!({
        "ok": true,
        "frame_count": core.project.frames.len(),
        "current_frame": core.current,
        "loop_range": loop_range,
        "frames": frames,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Channel, ChannelTarget, Key};
    use crate::model::{
        AudioClip, CameraKey, ClipDef, ClipLoop, Easing, FigureDef, FigureInstance, Frame,
        FxClipEntry, ImageAsset, ParticleEvent, ParticleKind, Pose, SegmentMedia,
        SpriteInstance, Stroke, TextInstance,
    };
    use crate::vfx::{ClipKind, ParticleClip};

    /// The two frames under trade throughout the suite.
    const A: usize = 1;
    const B: usize = 3;

    fn frame(duration: f32, easing: Option<Easing>, bend: f32) -> Frame {
        Frame {
            poses: Vec::new(),
            duration,
            easing,
            bends: vec![vec![bend]],
        }
    }

    /// An FX entry starting at `start`, told apart by colour
    /// (construction mirrors the `fx_ops` tests' helper, with
    /// the entry's `hidden` field included).
    fn fx_entry(start: f32, color: u32) -> FxClipEntry {
        FxClipEntry {
            clip: ParticleClip {
                start,
                duration: 4.0,
                kind: ClipKind::Burst,
                color,
                color2: None,
                seed: 7,
                angle: 0.0,
                length: 130.0,
            },
            figure: 0,
            node: 0,
            hidden: false,
        }
    }

    fn instance_key(frame: f32, x: f32) -> InstanceKey {
        InstanceKey {
            frame,
            x,
            y: 0.0,
            scale: 1.0,
            angle: 0.0,
            opacity: 1.0,
            flip_x: false,
            flip_y: false,
            scale_y: 0.0,
            pivot_x: 0.5,
            pivot_y: 0.5,
        }
    }

    /// A four-frame project whose frames 1 (A) and 3 (B) are
    /// distinguishable in duration, easing and bends, with
    /// one item pinned to A and one to B in EVERY remapped
    /// collection — plus bystanders on other frames that must
    /// not move. The playhead sits on frame 2, neither A nor
    /// B, so "current unchanged" is a real assertion.
    fn staged_core() -> EditorCore {
        let mut core = EditorCore::new();
        core.project.frames = vec![
            frame(1.0, None, 0.0),
            frame(2.0, Some(Easing::EaseIn), 0.5),
            frame(1.5, None, 0.0),
            frame(5.0, Some(Easing::EaseOut), -0.25),
        ];
        core.current = 2;
        core.project.camera = vec![
            CameraKey { frame: 0, x: 1.0, y: 0.0, zoom: 1.0, angle: 0.0, easing: crate::model::CameraEase::Linear },
            CameraKey { frame: A, x: 10.0, y: 0.0, zoom: 1.0, angle: 0.0, easing: crate::model::CameraEase::Linear },
            CameraKey { frame: B, x: 30.0, y: 0.0, zoom: 1.0, angle: 0.0, easing: crate::model::CameraEase::Linear },
        ];
        core.project.particles = vec![
            ParticleEvent {
                frame: A,
                figure: None,
                node: None,
                x: 0.0,
                y: 0.0,
                kind: ParticleKind::Burst,
                color: 0xAAAA,
                seed: 1,
            },
            ParticleEvent {
                frame: 2,
                figure: None,
                node: None,
                x: 0.0,
                y: 0.0,
                kind: ParticleKind::Smoke,
                color: 0xCCCC,
                seed: 3,
            },
            ParticleEvent {
                frame: B,
                figure: None,
                node: None,
                x: 0.0,
                y: 0.0,
                kind: ParticleKind::Sparkle,
                color: 0xBBBB,
                seed: 2,
            },
        ];
        core.project.strokes = vec![
            Stroke {
                frame: A,
                points: vec![[0.0, 0.0]],
                color: 0x1111,
                width: 2.0,
                widths: Vec::new(),
                fill: None,
                spray: None,
                opacity: 1.0,
                scatter: 0.0,
            },
            Stroke {
                frame: B,
                points: vec![[9.0, 9.0]],
                color: 0x2222,
                width: 2.0,
                widths: Vec::new(),
                fill: None,
                spray: None,
                opacity: 1.0,
                scatter: 0.0,
            },
        ];
        core.project.audio = vec![
            AudioClip { path: "a.wav".into(), start_frame: A, gain: 1.0, muted: false },
            AudioClip { path: "b.wav".into(), start_frame: B, gain: 1.0, muted: false },
        ];
        core.project.fx_clips = vec![fx_entry(A as f32, 0x3333), fx_entry(B as f32, 0x4444)];
        core.project.channels = vec![Channel {
            figure: 0,
            target: ChannelTarget::RootX,
            keys: vec![
                Key::flat(0.0, 99.0),
                Key::flat(A as f32, 10.0),
                Key::flat(B as f32, 30.0),
            ],
        }];
        core.project.texts = vec![TextInstance {
            content: "label".into(),
            size: 24.0,
            color: 0xFFFFFF,
            keys: vec![
                instance_key(A as f32, 1.0),
                instance_key(2.0, 2.0),
                instance_key(B as f32, 3.0),
            ],
        }];
        core.project.sprites = vec![SpriteInstance {
            asset: 0,
            keys: vec![instance_key(A as f32, 5.0), instance_key(B as f32, 7.0)],
        }];
        core
    }

    #[test]
    fn swap_frames_exchanges_contents_and_remaps_every_collection() {
        let mut core = staged_core();
        let undo_before = core.undo_depth();

        assert!(swap_frames(&mut core, A, B));
        assert_eq!(core.undo_depth(), undo_before + 1, "exactly one snapshot");
        assert_eq!(
            core.status, "Swapped frames 2 and 4.",
            "success status, 1-based"
        );
        assert_eq!(core.current, 2, "the playhead index does not move");

        // Frame contents exchanged wholesale: duration,
        // easing and bends all travelled together.
        let frames = &core.project.frames;
        assert_eq!(frames[A].duration, 5.0, "B's duration now sits at A");
        assert_eq!(frames[A].easing, Some(Easing::EaseOut));
        assert_eq!(frames[A].bends, vec![vec![-0.25]]);
        assert_eq!(frames[B].duration, 2.0, "A's duration now sits at B");
        assert_eq!(frames[B].easing, Some(Easing::EaseIn));
        assert_eq!(frames[B].bends, vec![vec![0.5]]);
        assert_eq!(frames[0].duration, 1.0, "bystander frame untouched");
        assert_eq!(frames[2].duration, 1.5, "bystander frame untouched");

        // Camera: the key identified by x = 10 sat on A, the
        // one with x = 30 on B; they traded frames. The frame
        // 0 bystander stayed. (The finder takes the core as a
        // parameter so it can be reused after `core.undo()`.)
        let cam_frame_of = |core: &EditorCore, x: f32| {
            core.project
                .camera
                .iter()
                .find(|k| k.x == x)
                .map(|k| k.frame)
        };
        assert_eq!(cam_frame_of(&core, 10.0), Some(B), "camera key followed its content");
        assert_eq!(cam_frame_of(&core, 30.0), Some(A));
        assert_eq!(cam_frame_of(&core, 1.0), Some(0), "bystander camera key unmoved");
        let cam_frames: Vec<usize> = core.project.camera.iter().map(|k| k.frame).collect();
        assert_eq!(cam_frames, vec![0, A, B], "camera list still sorted by frame");

        // Particles, told apart by colour.
        let particle_frame_of = |color: u32| {
            core.project
                .particles
                .iter()
                .find(|e| e.color == color)
                .map(|e| e.frame)
        };
        assert_eq!(particle_frame_of(0xAAAA), Some(B));
        assert_eq!(particle_frame_of(0xBBBB), Some(A));
        assert_eq!(particle_frame_of(0xCCCC), Some(2), "bystander particle unmoved");

        // Strokes, told apart by colour.
        let stroke_frame_of = |color: u32| {
            core.project
                .strokes
                .iter()
                .find(|s| s.color == color)
                .map(|s| s.frame)
        };
        assert_eq!(stroke_frame_of(0x1111), Some(B));
        assert_eq!(stroke_frame_of(0x2222), Some(A));

        // Audio, told apart by file name.
        let audio_start_of = |path: &str| {
            core.project
                .audio
                .iter()
                .find(|c| c.path == path)
                .map(|c| c.start_frame)
        };
        assert_eq!(audio_start_of("a.wav"), Some(B));
        assert_eq!(audio_start_of("b.wav"), Some(A));

        // FX clips: start is f32, told apart by colour.
        let fx_start_of = |color: u32| {
            core.project
                .fx_clips
                .iter()
                .find(|e| e.clip.color == color)
                .map(|e| e.clip.start)
        };
        assert_eq!(fx_start_of(0x3333), Some(B as f32));
        assert_eq!(fx_start_of(0x4444), Some(A as f32));

        // Graph channel keys, told apart by value; the list
        // is ascending again after the trade.
        let keys = &core.project.channels[0].keys;
        let key_frame_of = |value: f32| keys.iter().find(|k| k.value == value).map(|k| k.frame);
        assert_eq!(key_frame_of(10.0), Some(B as f32), "A's key followed its content");
        assert_eq!(key_frame_of(30.0), Some(A as f32));
        assert_eq!(key_frame_of(99.0), Some(0.0), "bystander key unmoved");
        let key_frames: Vec<f32> = keys.iter().map(|k| k.frame).collect();
        assert_eq!(key_frames, vec![0.0, A as f32, B as f32], "channel keys re-sorted");

        // Text instance keys, told apart by x.
        let text_keys = &core.project.texts[0].keys;
        let text_frame_of = |x: f32| text_keys.iter().find(|k| k.x == x).map(|k| k.frame);
        assert_eq!(text_frame_of(1.0), Some(B as f32));
        assert_eq!(text_frame_of(3.0), Some(A as f32));
        assert_eq!(text_frame_of(2.0), Some(2.0), "bystander instance key unmoved");

        // Sprite instance keys, told apart by x.
        let sprite_keys = &core.project.sprites[0].keys;
        let sprite_frame_of = |x: f32| sprite_keys.iter().find(|k| k.x == x).map(|k| k.frame);
        assert_eq!(sprite_frame_of(5.0), Some(B as f32));
        assert_eq!(sprite_frame_of(7.0), Some(A as f32));

        // The whole swap is ONE undo step: undoing restores
        // the frames and the remapped collections together.
        core.undo();
        assert_eq!(core.project.frames[A].duration, 2.0, "undo restores A's content");
        assert_eq!(core.project.frames[B].duration, 5.0, "undo restores B's content");
        assert_eq!(cam_frame_of(&core, 10.0), Some(A), "undo restores the camera remap");
    }

    #[test]
    fn swap_frames_refuses_same_frame_and_out_of_range_without_snapshot() {
        let mut core = staged_core();
        let undo_before = core.undo_depth();

        assert!(!swap_frames(&mut core, A, A), "a frame cannot swap with itself");
        assert!(
            core.status.contains("Frame 2 cannot be swapped with itself."),
            "status: {}",
            core.status
        );
        assert!(!swap_frames(&mut core, 0, 99), "b out of range");
        assert!(
            core.status.contains("No frame 100 to swap."),
            "status: {}",
            core.status
        );
        assert!(!swap_frames(&mut core, 99, 0), "a out of range");
        assert!(
            core.status.contains("No frame 100 to swap."),
            "status: {}",
            core.status
        );

        assert_eq!(core.undo_depth(), undo_before, "no refusal took a snapshot");
        assert_eq!(core.project.frames[A].duration, 2.0, "frames untouched");
        assert_eq!(core.project.frames[B].duration, 5.0, "frames untouched");
        assert_eq!(
            core.project.camera.iter().find(|k| k.x == 10.0).map(|k| k.frame),
            Some(A),
            "no refusal remapped anything"
        );
    }

    #[test]
    fn swap_frames_twice_restores_the_original_arrangement() {
        let mut core = staged_core();
        let undo_before = core.undo_depth();

        assert!(swap_frames(&mut core, A, B));
        assert!(swap_frames(&mut core, A, B), "the same swap is its own inverse");
        assert_eq!(core.undo_depth(), undo_before + 2, "one snapshot per swap");

        assert_eq!(core.project.frames[A].duration, 2.0);
        assert_eq!(core.project.frames[B].duration, 5.0);
        assert_eq!(
            core.project.camera.iter().find(|k| k.x == 10.0).map(|k| k.frame),
            Some(A),
            "attached data followed the content both ways"
        );
        assert_eq!(
            core.project.audio.iter().find(|c| c.path == "a.wav").map(|c| c.start_frame),
            Some(A)
        );
    }

    #[test]
    fn swap_frame_with_neighbour_swaps_and_refuses_at_the_ends() {
        let mut core = staged_core();

        // First frame, stepping back: refused, no snapshot.
        core.current = 0;
        let undo_before = core.undo_depth();
        assert!(!swap_frame_with_neighbour(&mut core, -1), "no frame before the first");
        assert!(
            core.status.contains("no neighbour to swap with"),
            "status: {}",
            core.status
        );
        assert_eq!(core.undo_depth(), undo_before, "refusal took no snapshot");

        // Last frame, stepping forward: refused, no snapshot.
        core.current = 3;
        assert!(!swap_frame_with_neighbour(&mut core, 1), "no frame after the last");
        assert!(
            core.status.contains("no neighbour to swap with"),
            "status: {}",
            core.status
        );
        assert_eq!(core.undo_depth(), undo_before, "refusal took no snapshot");
        assert_eq!(core.project.frames[3].duration, 5.0, "frames untouched");

        // Forward from frame 1: frames 1 and 2 trade contents
        // (durations 2.0 and 1.5), the playhead index stays,
        // and the status is swap_frames' own.
        core.current = 1;
        assert!(swap_frame_with_neighbour(&mut core, 1));
        assert_eq!(core.undo_depth(), undo_before + 1, "exactly one snapshot");
        assert_eq!(core.current, 1, "the playhead index does not move");
        assert_eq!(core.project.frames[1].duration, 1.5, "frame 2's content moved in");
        assert_eq!(core.project.frames[2].duration, 2.0, "frame 1's content moved out");
        assert_eq!(core.status, "Swapped frames 2 and 3.");

        // Back from frame 1 trades frames 0 and 1 the same
        // way. The status names the pair in `swap_frames`'
        // argument order — current frame first.
        assert!(swap_frame_with_neighbour(&mut core, -1));
        assert_eq!(core.project.frames[0].duration, 1.5);
        assert_eq!(core.project.frames[1].duration, 1.0);
        assert_eq!(core.status, "Swapped frames 2 and 1.");
        assert_eq!(core.current, 1);
    }

    // ---- Wave 11 (B2): MCP parity handlers ---------------------------

    /// A three-frame project for one figure whose root moves
    /// on frames 0 and 2 only — the keyframe census is
    /// therefore [0, 2], frame 1 holding frame 0's pose.
    fn keyed_core() -> EditorCore {
        let mut core = EditorCore::new();
        core.project.figures = vec![crate::model::FigureInstance {
            name: "Solo".into(),
            color: 0xFFFFFF,
            def: crate::model::FigureDef::humanoid(),
            hidden: false,
            locked: false,
            opacity: 1.0,
            clip: None,
            states: Vec::new(),
        }];
        let pose = |x: f32| crate::model::Pose { root: [x, 0.0], angles: vec![0.0], sx: 1.0, sy: 1.0 };
        core.project.frames = vec![
            Frame { poses: vec![pose(0.0)], duration: 1.0, easing: None, bends: Vec::new() },
            Frame { poses: vec![pose(0.0)], duration: 2.0, easing: None, bends: Vec::new() },
            Frame { poses: vec![pose(40.0)], duration: 1.0, easing: None, bends: Vec::new() },
        ];
        core.current = 1;
        core
    }

    #[test]
    fn insert_frames_after_current_matches_add_frame_shape_and_one_undo() {
        let mut core = staged_core();
        core.current = 1;
        let undo_before = core.undo_depth();

        assert_eq!(insert_frames_after_current(&mut core, 3), 3);
        assert_eq!(core.undo_depth(), undo_before + 1, "the batch is ONE snapshot");
        assert_eq!(core.current, 1, "the playhead stays on the original frame");
        assert_eq!(core.status, "Inserted 3 frames after frame 2.");
        assert_eq!(core.project.frames.len(), 7);
        // The inserted frames are add_frame's shape: poses and
        // duration cloned from frame 1 (duration 2.0), easing
        // reset and bends emptied — NOT whole-frame clones
        // (frame 1 carries EaseIn and a bend row).
        for inserted in &core.project.frames[2..5] {
            assert_eq!(inserted.duration, 2.0, "duration cloned from the source frame");
            assert_eq!(inserted.easing, None, "easing reset, like add_frame");
            assert!(inserted.bends.is_empty(), "bends emptied, like add_frame");
            assert_eq!(inserted.poses.len(), 0, "poses cloned from the source frame");
        }
        assert_eq!(core.project.frames[5].duration, 1.5, "later frames shifted, untouched");

        // One undo removes the whole batch.
        core.undo();
        assert_eq!(core.project.frames.len(), 4, "one undo removes the batch");
    }

    #[test]
    fn insert_frames_after_current_clamps_and_refuses_without_a_frame() {
        let mut core = staged_core();
        core.current = 0;
        assert_eq!(
            insert_frames_after_current(&mut core, 500),
            INSERT_FRAMES_MAX,
            "counts above the panel cap clamp to it"
        );
        assert_eq!(core.project.frames.len(), 4 + INSERT_FRAMES_MAX);

        // No current frame: an empty project refuses with the
        // status line and takes no snapshot.
        let mut empty = EditorCore::new();
        empty.project.frames = Vec::new();
        let undo_before = empty.undo_depth();
        assert_eq!(insert_frames_after_current(&mut empty, 4), 0);
        assert_eq!(empty.status, "No frame to insert after.");
        assert_eq!(empty.undo_depth(), undo_before, "refusal took no snapshot");
        assert!(empty.project.frames.is_empty());
    }

    #[test]
    fn tool_insert_frames_validates_count_and_reports_the_batch() {
        let mut core = staged_core();
        core.current = 0;

        let err = tool_insert_frames(&mut core, &json!({})).unwrap_err();
        assert!(err.contains("missing required argument 'count'"), "err: {err}");
        let err = tool_insert_frames(&mut core, &json!({ "count": 0 })).unwrap_err();
        assert!(err.contains("at least 1"), "err: {err}");
        let err = tool_insert_frames(&mut core, &json!({ "count": -2 })).unwrap_err();
        assert!(err.contains("non-negative integer"), "err: {err}");
        assert_eq!(core.project.frames.len(), 4, "no error path inserted anything");

        let out = tool_insert_frames(&mut core, &json!({ "count": 2 })).unwrap();
        assert_eq!(out["ok"], json!(true));
        assert_eq!(out["inserted"], json!(2));
        assert_eq!(out["frame_count"], json!(6));
        assert_eq!(out["current_frame"], json!(0));

        // Empty project: the op's refusal surfaces as the error.
        let mut empty = EditorCore::new();
        empty.project.frames = Vec::new();
        let err = tool_insert_frames(&mut empty, &json!({ "count": 2 })).unwrap_err();
        assert!(err.contains("No frame to insert after."), "err: {err}");
    }

    #[test]
    fn tool_swap_frame_with_neighbour_swaps_and_surfaces_refusals() {
        let mut core = staged_core();
        core.current = 1;

        let out = tool_swap_frame_with_neighbour(&mut core, &json!({ "direction": "next" }))
            .unwrap();
        assert_eq!(out["ok"], json!(true));
        assert_eq!(out["status"], json!("Swapped frames 2 and 3."));
        assert_eq!(core.project.frames[1].duration, 1.5, "frame 2's content moved in");
        assert_eq!(core.current, 1, "the playhead index does not move");

        // At the first frame, "previous" is the op's own
        // refusal, carried on the status line.
        core.current = 0;
        let err = tool_swap_frame_with_neighbour(&mut core, &json!({ "direction": "previous" }))
            .unwrap_err();
        assert!(err.contains("no neighbour to swap with"), "err: {err}");

        // Bad arguments are argument errors, never panics.
        let err = tool_swap_frame_with_neighbour(&mut core, &json!({})).unwrap_err();
        assert!(err.contains("missing required argument 'direction'"), "err: {err}");
        let err = tool_swap_frame_with_neighbour(&mut core, &json!({ "direction": "up" }))
            .unwrap_err();
        assert!(err.contains("unknown direction 'up'"), "err: {err}");
        let err = tool_swap_frame_with_neighbour(&mut core, &json!({ "direction": 1 }))
            .unwrap_err();
        assert!(err.contains("must be a string"), "err: {err}");
    }

    #[test]
    fn tool_get_keyframes_reads_the_dopesheet_census() {
        let mut core = keyed_core();
        let out = tool_get_keyframes(&mut core, &json!({ "figure": 0 })).unwrap();
        assert_eq!(out["key_frames"], json!([0, 2]), "pose changes at frames 0 and 2");
        assert_eq!(out["current_frame"], json!(1));
        assert_eq!(out["frame_count"], json!(3));

        let err = tool_get_keyframes(&mut core, &json!({ "figure": 1 })).unwrap_err();
        assert!(err.contains("figure index 1 out of range"), "err: {err}");
        let err = tool_get_keyframes(&mut core, &json!({})).unwrap_err();
        assert!(err.contains("missing required argument 'figure'"), "err: {err}");

        // Read-only: no snapshot, no mutation.
        let undo_before = core.undo_depth();
        let _ = tool_get_keyframes(&mut core, &json!({ "figure": 0 })).unwrap();
        assert_eq!(core.undo_depth(), undo_before, "a read takes no snapshot");
    }

    #[test]
    fn tool_list_frames_reports_the_timing_table() {
        let mut core = staged_core();
        core.set_loop_range(1, 2);
        let out = tool_list_frames(&mut core, &json!({})).unwrap();
        assert_eq!(out["frame_count"], json!(4));
        assert_eq!(out["current_frame"], json!(2));
        assert_eq!(out["loop_range"], json!([1, 2]));
        let frames = out["frames"].as_array().unwrap();
        assert_eq!(frames.len(), 4);
        assert_eq!(frames[0]["duration"], json!(1.0));
        assert_eq!(frames[1]["duration"], json!(2.0));
        assert_eq!(frames[1]["easing"], json!("ease-in"));
        assert_eq!(frames[2]["easing"], Value::Null);
        assert_eq!(frames[3]["duration"], json!(5.0));
        assert_eq!(frames[3]["easing"], json!("ease-out"));

        // An empty project lists zero frames instead of
        // panicking.
        let mut empty = EditorCore::new();
        empty.project.frames = Vec::new();
        let out = tool_list_frames(&mut empty, &json!({})).unwrap();
        assert_eq!(out["frame_count"], json!(0));
        assert_eq!(out["frames"], json!([]));
        assert_eq!(out["loop_range"], Value::Null);
    }

    #[test]
    fn tool_adjust_frame_duration_scales_clamps_and_reports_back() {
        // Wave 14 (A2): the relative duration handler —
        // the stored duration is multiplied and clamped by
        // the op, the reply reads the stored value back,
        // and the timing-tweak contract holds: no undo
        // step, one dirty edit per real change.
        let mut core = staged_core();
        core.current = 2;
        let depth = core.undo_depth();
        let edits = core.edits_since_autosave();

        // Current frame (index 2, duration 1.5) doubled.
        let out = tool_adjust_frame_duration(&mut core, &json!({ "factor": 2.0 })).unwrap();
        assert_eq!(out["ok"], json!(true));
        assert_eq!(out["frame"], json!(2));
        assert_eq!(out["duration"], json!(3.0));
        assert_eq!(core.project.frames[2].duration, 3.0);
        assert_eq!(core.undo_depth(), depth, "a timing tweak is not an undo step");
        assert_eq!(
            core.edits_since_autosave(),
            edits + 1,
            "the real change counts one dirty edit"
        );

        // An explicit frame moves the playhead there first
        // (the set_frame_easing convention), then adjusts.
        let out = tool_adjust_frame_duration(&mut core, &json!({ "factor": 0.5, "frame": 3 }))
            .unwrap();
        assert_eq!(core.current, 3, "the playhead followed the frame argument");
        assert_eq!(out["duration"], json!(2.5), "frame 3's 5.0 halved");

        // The op's clamp binds: 2.5 × 100 lands on 8.0.
        let out = tool_adjust_frame_duration(&mut core, &json!({ "factor": 100.0 })).unwrap();
        assert_eq!(out["duration"], json!(8.0));
    }

    #[test]
    fn tool_adjust_frame_duration_refuses_bad_input_without_side_effects() {
        // Wave 14 (A2): factor and frame are validated
        // BEFORE the op runs (the op indexes
        // frames[current] directly and would panic on an
        // empty project), and no refusal moves the
        // playhead, changes a duration or marks dirty.
        let mut core = staged_core();
        core.current = 1;
        let edits = core.edits_since_autosave();

        let err = tool_adjust_frame_duration(&mut core, &json!({})).unwrap_err();
        assert!(err.contains("missing required argument 'factor'"), "err: {err}");
        let err = tool_adjust_frame_duration(&mut core, &json!({ "factor": 0.0 })).unwrap_err();
        assert!(err.contains("must be positive"), "err: {err}");
        let err = tool_adjust_frame_duration(&mut core, &json!({ "factor": -2.0 })).unwrap_err();
        assert!(err.contains("must be positive"), "err: {err}");
        let err = tool_adjust_frame_duration(&mut core, &json!({ "factor": 1e300 })).unwrap_err();
        assert!(err.contains("must be a finite number"), "err: {err}");
        let err =
            tool_adjust_frame_duration(&mut core, &json!({ "factor": 2.0, "frame": 99 }))
                .unwrap_err();
        assert!(err.contains("frame index 99 out of range"), "err: {err}");

        assert_eq!(core.current, 1, "no refusal moved the playhead");
        assert_eq!(core.project.frames[1].duration, 2.0, "no refusal changed a duration");
        assert_eq!(core.edits_since_autosave(), edits, "no refusal marked dirty");

        // An empty project is a clean refusal, not the
        // op's index panic.
        let mut empty = EditorCore::new();
        empty.project.frames = Vec::new();
        let err = tool_adjust_frame_duration(&mut empty, &json!({ "factor": 2.0 })).unwrap_err();
        assert!(err.contains("frame index 0 out of range"), "err: {err}");
    }

    // ---- Wave 12 (C8): media / table reference integrity ----------

    /// A two-figure project exercising every project-table
    /// reference a swap passes near: sprite instances pointing
    /// into `assets`, segment media on a figure DEF pointing
    /// into `assets`, a movieclip (figure 1 plays clip 0, whose
    /// own frames are a clip-local pose sequence), a particle
    /// and an FX entry anchored to figure/node indices, a graph
    /// channel owned by figure 1, and `camera_follow` naming
    /// figure 1. Frames A and B carry two-figure poses and
    /// TWO-row bends, so bend-row ↔ pose alignment per figure
    /// is observable.
    fn media_core() -> EditorCore {
        let mut core = EditorCore::new();
        core.project.assets = vec![
            ImageAsset { name: "a.png".into(), png: vec![1, 2, 3] },
            ImageAsset { name: "b.png".into(), png: vec![4, 5, 6] },
        ];
        let mut def_with_media = FigureDef::humanoid();
        def_with_media.nodes[3].media = Some(SegmentMedia::Image { asset: 1, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false, opacity: 1.0 });
        def_with_media.nodes[4].media =
            Some(SegmentMedia::Text { content: "hi".into(), size: 12.0 });
        core.project.figures = vec![
            FigureInstance {
                name: "Media".into(),
                color: 0xFFFFFF,
                def: def_with_media,
                hidden: false,
                locked: false,
                opacity: 1.0,
                clip: None,
                states: Vec::new(),
            },
            FigureInstance {
                name: "Clipper".into(),
                color: 0x00FF00,
                def: FigureDef::humanoid(),
                hidden: false,
                locked: false,
                opacity: 1.0,
                clip: Some(0),
                states: Vec::new(),
            },
        ];
        core.project.clips = vec![ClipDef {
            name: "wave".into(),
            def: FigureDef::humanoid(),
            frames: vec![
                Pose { root: [1.0, 2.0], angles: vec![0.0], sx: 1.0, sy: 1.0 },
                Pose { root: [3.0, 4.0], angles: vec![0.5], sx: 1.0, sy: 1.0 },
            ],
            loop_mode: ClipLoop::Loop,
        }];
        let two_pose_frame = |r0: f32, r1: f32, b0: f32, b1: f32, duration: f32| Frame {
            poses: vec![
                Pose { root: [r0, 0.0], angles: vec![0.0], sx: 1.0, sy: 1.0 },
                Pose { root: [r1, 0.0], angles: vec![0.0], sx: 1.0, sy: 1.0 },
            ],
            duration,
            easing: None,
            bends: vec![vec![b0], vec![b1]],
        };
        core.project.frames = vec![
            frame(1.0, None, 0.0),
            two_pose_frame(10.0, 20.0, 0.11, 0.22, 2.0),
            frame(1.5, None, 0.0),
            two_pose_frame(30.0, 40.0, 0.33, 0.44, 5.0),
        ];
        core.project.sprites = vec![
            SpriteInstance {
                asset: 0,
                keys: vec![instance_key(A as f32, 5.0), instance_key(B as f32, 7.0)],
            },
            SpriteInstance { asset: 1, keys: vec![instance_key(A as f32, 9.0)] },
        ];
        core.project.particles = vec![ParticleEvent {
            frame: A,
            figure: Some(1),
            node: Some(2),
            x: 0.0,
            y: 0.0,
            kind: ParticleKind::Burst,
            color: 0xAAAA,
            seed: 1,
        }];
        let mut fx = fx_entry(A as f32, 0x3333);
        fx.figure = 1;
        fx.node = 3;
        core.project.fx_clips = vec![fx];
        core.project.channels = vec![Channel {
            figure: 1,
            target: ChannelTarget::RootX,
            keys: vec![Key::flat(A as f32, 10.0), Key::flat(B as f32, 30.0)],
        }];
        core.project.camera_follow = Some(1);
        core
    }

    #[test]
    fn swap_preserves_media_and_table_references() {
        let mut core = media_core();

        assert!(swap_frames(&mut core, A, B));
        let project = &core.project;

        // Sprite instances: the KEYS followed their frames
        // (that is the swap's job) but the `asset` index each
        // sprite points into `Project::assets` with was not
        // rewritten — and the assets table itself is
        // byte-for-byte where it was.
        assert_eq!(project.sprites[0].asset, 0, "sprite 0 still names asset 0");
        assert_eq!(project.sprites[1].asset, 1, "sprite 1 still names asset 1");
        let sprite0_frame_of = |x: f32| {
            project.sprites[0].keys.iter().find(|k| k.x == x).map(|k| k.frame)
        };
        assert_eq!(sprite0_frame_of(5.0), Some(B as f32), "sprite key followed its frame");
        assert_eq!(sprite0_frame_of(7.0), Some(A as f32));
        assert_eq!(
            project.sprites[1].keys[0].frame, B as f32,
            "sprite 1's only key followed its frame"
        );
        assert_eq!(project.assets.len(), 2, "assets table did not move");
        assert_eq!(project.assets[0].name, "a.png");
        assert_eq!(project.assets[0].png, vec![1, 2, 3]);
        assert_eq!(project.assets[1].name, "b.png");
        assert_eq!(project.assets[1].png, vec![4, 5, 6]);

        // Segment media lives on figure DEFS, not frames: the
        // swap never reaches it, so the def's asset reference
        // and its text label are exactly as authored.
        assert_eq!(
            project.figures[0].def.nodes[3].media,
            Some(SegmentMedia::Image { asset: 1, offset: [0.0, 0.0], scale: 1.0, angle_offset: 0.0, flip: false, opacity: 1.0 }),
            "def-level image media untouched, asset index intact"
        );
        assert_eq!(
            project.figures[0].def.nodes[4].media,
            Some(SegmentMedia::Text { content: "hi".into(), size: 12.0 }),
            "def-level text media untouched"
        );

        // Movieclip integrity: figure 1 still plays clip 0,
        // and the clip's own clip-local pose sequence is
        // untouched — the timeline swap has no business inside
        // a clip.
        assert_eq!(project.figures[1].clip, Some(0), "figure still indexes clip 0");
        assert_eq!(project.figures[0].clip, None);
        assert_eq!(project.clips.len(), 1);
        assert_eq!(project.clips[0].name, "wave");
        assert_eq!(project.clips[0].frames.len(), 2, "clip frames are clip-local");
        assert_eq!(project.clips[0].frames[0].root, [1.0, 2.0]);
        assert_eq!(project.clips[0].frames[1].root, [3.0, 4.0]);

        // Bends ↔ poses alignment per figure: frame A now
        // holds frame B's poses AND frame B's two bend rows —
        // row 0 still belongs to figure 0's pose, row 1 to
        // figure 1's, because both arrays travel inside the
        // one Frame value.
        assert_eq!(project.frames[A].poses[0].root, [30.0, 0.0]);
        assert_eq!(project.frames[A].poses[1].root, [40.0, 0.0]);
        assert_eq!(
            project.frames[A].bends,
            vec![vec![0.33], vec![0.44]],
            "B's bend rows travelled with B's poses"
        );
        assert_eq!(project.frames[B].poses[0].root, [10.0, 0.0]);
        assert_eq!(project.frames[B].poses[1].root, [20.0, 0.0]);
        assert_eq!(
            project.frames[B].bends,
            vec![vec![0.11], vec![0.22]],
            "A's bend rows travelled with A's poses"
        );

        // Figure/node indices on frame-pinned entries: the
        // frame pins traded, the table indices did not — the
        // figures table never moves, so they still name the
        // same figure and node.
        assert_eq!(project.particles[0].frame, B, "particle followed its frame");
        assert_eq!(project.particles[0].figure, Some(1));
        assert_eq!(project.particles[0].node, Some(2));
        assert_eq!(project.fx_clips[0].clip.start, B as f32, "FX start followed its frame");
        assert_eq!(project.fx_clips[0].figure, 1);
        assert_eq!(project.fx_clips[0].node, 3);
        assert_eq!(project.channels[0].figure, 1, "channel still owned by figure 1");
        let key_frame_of = |value: f32| {
            project.channels[0].keys.iter().find(|k| k.value == value).map(|k| k.frame)
        };
        assert_eq!(key_frame_of(10.0), Some(B as f32));
        assert_eq!(key_frame_of(30.0), Some(A as f32));
        assert_eq!(project.camera_follow, Some(1), "camera still follows figure 1");

        // One undo restores the traded frames and keys; the
        // references asserted above were never written, so
        // there is nothing for undo to restore there.
        core.undo();
        assert_eq!(core.project.frames[A].bends, vec![vec![0.11], vec![0.22]]);
        assert_eq!(core.project.sprites[0].keys[0].frame, A as f32);
        assert_eq!(core.project.sprites[0].asset, 0);
    }

    // ---- Wave 13 (C7): instance keys on timeline-length edits ----
    //
    // The verdict pinned here (module docs, "Instance keys on
    // timeline-length edits"): frame pins are
    // timeline-absolute. Insert/delete paths shift NO pinned
    // collection — instance keys behave exactly like camera
    // keys, particles, audio and the rest. These tests assert
    // against all the collections at once on purpose: if the
    // house rule ever flips, it must flip for all seven
    // together, and these tests are the tripwire that fails
    // in the open instead of letting one collection drift.

    /// The frame every pin in `core`'s staged project sits
    /// on, collection by collection — the shared assertion
    /// the three length-edit tests compare before and after.
    fn all_pin_frames(
        core: &EditorCore,
    ) -> (Vec<usize>, Vec<usize>, Vec<usize>, Vec<f32>, Vec<f32>) {
        let camera: Vec<usize> = core.project.camera.iter().map(|k| k.frame).collect();
        let particles: Vec<usize> = core.project.particles.iter().map(|e| e.frame).collect();
        let audio: Vec<usize> = core.project.audio.iter().map(|c| c.start_frame).collect();
        let sprite: Vec<f32> = core.project.sprites[0].keys.iter().map(|k| k.frame).collect();
        let text: Vec<f32> = core.project.texts[0].keys.iter().map(|k| k.frame).collect();
        (camera, particles, audio, sprite, text)
    }

    #[test]
    fn insert_frames_keep_instance_keys_and_all_pins_absolute() {
        let mut core = staged_core();
        let pins_before = all_pin_frames(&core);
        // The staged pins, for the record: camera [0, A, B],
        // particles [A, 2, B], audio [A, B], sprite keys
        // [A, B] as f32, text keys [A, 2, B] as f32.
        assert_eq!(
            pins_before,
            (
                vec![0, A, B],
                vec![A, 2, B],
                vec![A, B],
                vec![A as f32, B as f32],
                vec![A as f32, 2.0, B as f32],
            )
        );
        let undo_before = core.undo_depth();

        // This file's batch insert: two frames after the
        // current one (frame 2). Frame B's content now sits
        // at index 5 — and every pin still names its old
        // timeline position, instance keys included.
        assert_eq!(insert_frames_after_current(&mut core, 2), 2);
        assert_eq!(core.undo_depth(), undo_before + 1, "the batch is ONE snapshot");
        assert_eq!(core.project.frames.len(), 6, "the timeline grew");
        assert_eq!(
            all_pin_frames(&core),
            pins_before,
            "no pinned collection shifted — pins are timeline-absolute"
        );
    }

    #[test]
    fn add_keeps_all_pins_absolute_delete_frame_remaps_only_the_camera() {
        let mut core = staged_core();
        let pins_before = all_pin_frames(&core);

        // EditorCore::add_frame (editor.rs): inserts a clone
        // after the current frame (2) and steps onto it.
        core.add_frame();
        assert_eq!(core.project.frames.len(), 5);
        assert_eq!(core.current, 3);
        assert_eq!(
            all_pin_frames(&core),
            pins_before,
            "add_frame shifted no pin, instance keys included"
        );

        // EditorCore::delete_frame removes the frame just
        // added (the playhead sits on it), restoring the
        // four-frame timeline. Every pin still sits exactly
        // where it was authored — EXCEPT the camera key
        // that sat on the deleted index (after the insert
        // that is the staged B key, x = 30): since RM18
        // (P-6b) the camera follows the delete mapping, so
        // that key is removed with its frame and the
        // camera leg is [0, A].
        core.delete_frame();
        assert_eq!(core.project.frames.len(), 4);
        let (camera, particles, audio, sprite, text) = all_pin_frames(&core);
        assert_eq!(camera, vec![0, A], "the camera key on the deleted frame went with it");
        assert_eq!(particles, pins_before.1);
        assert_eq!(audio, pins_before.2);
        assert_eq!(sprite, pins_before.3);
        assert_eq!(text, pins_before.4);
    }

    #[test]
    fn delete_range_remaps_only_the_camera_and_leaves_instance_keys_absolute() {
        let mut core = staged_core();
        let pins_before = all_pin_frames(&core);

        // EditorCore::delete_range cuts frames 2 and 3:
        // frame B (index 3) stops existing. The sprite key
        // at B now sits beyond the end of a two-frame
        // timeline — for the instance keys nothing is
        // deleted and nothing is renumbered; the pins
        // survive at their absolute positions. The camera
        // key at B is the one exception (RM18, P-6b): it
        // was removed with its frame, the camera leg is
        // [0, A], and the sprite key at B is now the
        // beyond-end example the sampling half below uses.
        assert!(core.delete_range(2, 2));
        assert_eq!(core.project.frames.len(), 2);
        let (camera, particles, audio, sprite, text) = all_pin_frames(&core);
        assert_eq!(camera, vec![0, A], "the camera key inside the cut went with its frame");
        assert_eq!(particles, pins_before.1);
        assert_eq!(audio, pins_before.2);
        assert_eq!(sprite, pins_before.3);
        assert_eq!(text, pins_before.4);

        // The beyond-end sprite key still RESOLVES:
        // InstanceKey::sample clamps at the ends of the key
        // list, so sampling at the key's own frame returns
        // its stored transform (x = 7), and sampling between
        // the surviving key and the beyond-end key still
        // interpolates across the cut (x = 6 at frame 2) —
        // the instance holds its keyed motion instead of
        // vanishing or wrapping.
        let keys = &core.project.sprites[0].keys;
        let at_last = InstanceKey::sample(keys, B as f32).expect("keys are non-empty");
        assert_eq!(at_last.x, 7.0, "the beyond-end key resolves to its stored transform");
        let at_first = InstanceKey::sample(keys, A as f32).expect("keys are non-empty");
        assert_eq!(at_first.x, 5.0);
        let midway = InstanceKey::sample(keys, 2.0).expect("keys are non-empty");
        assert_eq!(midway.x, 6.0, "interpolation across the cut still works");
    }

    #[test]
    fn frame_ops_remap_camera_keys_with_their_frames() {
        // shift_frames(0, 2, +1): the block permutation —
        // key 0 -> 1, key 1 -> 2, key 3 unmoved.
        let mut core = staged_core();
        let depth = core.undo_depth();
        assert!(core.shift_frames(0, 2, 1));
        assert_eq!(core.undo_depth(), depth + 1, "one snapshot incl. the remap");
        let frames: Vec<usize> = core.project.camera.iter().map(|k| k.frame).collect();
        assert_eq!(frames, vec![1, 2, 3], "sorted after the move");
        core.undo();
        let frames: Vec<usize> = core.project.camera.iter().map(|k| k.frame).collect();
        assert_eq!(frames, vec![0, A, B], "one undo restores the camera too");

        // reverse_range(0, 4): keys flip inside the block —
        // x=30 lands on 0, x=10 on 2, x=1 on 3.
        let mut core = staged_core();
        core.reverse_range(0, 4);
        let at = |core: &EditorCore, x: f32| {
            core.project.camera.iter().find(|k| k.x == x).map(|k| k.frame)
        };
        assert_eq!(at(&core, 30.0), Some(0));
        assert_eq!(at(&core, 10.0), Some(2));
        assert_eq!(at(&core, 1.0), Some(3));

        // delete_range(1, 2): key at 1 removed with its
        // frame, key at 3 shifts to 1, key at 0 stays.
        let mut core = staged_core();
        assert!(core.delete_range(1, 2));
        let frames: Vec<usize> = core.project.camera.iter().map(|k| k.frame).collect();
        assert_eq!(frames, vec![0, 1]);
        assert_eq!(at(&core, 30.0), Some(1), "the survivor kept its identity");

        // delete_frame at current = 1: key at 1 removed,
        // key at 3 shifts to 2.
        let mut core = staged_core();
        core.current = 1;
        core.delete_frame();
        let frames: Vec<usize> = core.project.camera.iter().map(|k| k.frame).collect();
        assert_eq!(frames, vec![0, 2]);

        // paste_range: keys at/after the paste point
        // shift by the pasted count — [0, 3, 5]. This leg
        // needs a pose-valid core (paste_range validates
        // the copied frames against the live rigs, which
        // staged_core's synthetic frames do not satisfy),
        // so it stages the same three keys on a demo core
        // whose frames are clones of its own first frame.
        let mut core = EditorCore::new();
        while core.project.frames.len() < 4 {
            let f = core.project.frames[0].clone();
            core.project.frames.push(f);
        }
        core.project.camera = vec![
            CameraKey { frame: 0, x: 1.0, y: 0.0, zoom: 1.0, angle: 0.0, easing: crate::model::CameraEase::Linear },
            CameraKey { frame: A, x: 10.0, y: 0.0, zoom: 1.0, angle: 0.0, easing: crate::model::CameraEase::Linear },
            CameraKey { frame: B, x: 30.0, y: 0.0, zoom: 1.0, angle: 0.0, easing: crate::model::CameraEase::Linear },
        ];
        assert!(core.copy_range(0, 2));
        assert!(core.paste_range(1));
        let frames: Vec<usize> = core.project.camera.iter().map(|k| k.frame).collect();
        assert_eq!(frames, vec![0, 3, 5]);
    }

    // ---- Roadmap Wave 1 (RM17): breakdown insert ----

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    fn bd_pose(root: [f32; 2], angles: &[f32]) -> Pose {
        Pose { root, angles: angles.to_vec(), sx: 1.0, sy: 1.0 }
    }

    /// Two frames with distinct poses and bends, the
    /// playhead on the first: the breakdown testbed. The
    /// project easing is the caller's; neither frame
    /// carries an easing override unless a test adds one.
    fn breakdown_core(easing: Easing) -> EditorCore {
        let mut core = EditorCore::new();
        core.project.easing = easing;
        core.project.frames = vec![
            Frame {
                poses: vec![bd_pose([100.0, 200.0], &[0.0, 10.0])],
                duration: 1.0,
                easing: None,
                bends: vec![vec![0.0, 0.0]],
            },
            Frame {
                poses: vec![bd_pose([300.0, 200.0], &[40.0, 50.0])],
                duration: 1.0,
                easing: None,
                bends: vec![vec![0.8, -0.4]],
            },
        ];
        core.current = 0;
        core
    }

    #[test]
    fn breakdown_fractions_are_even_at_favour_half() {
        assert_eq!(breakdown_fractions(0.5, 1), vec![0.5]);
        let three = breakdown_fractions(0.5, 3);
        assert_eq!(three.len(), 3);
        assert!(approx(three[0], 0.25) && approx(three[1], 0.5) && approx(three[2], 0.75));
    }

    #[test]
    fn breakdown_fractions_crowd_toward_the_favoured_key() {
        // A single breakdown sits exactly at the favour.
        assert!(approx(breakdown_fractions(0.25, 1)[0], 0.25));
        assert!(approx(breakdown_fractions(0.75, 1)[0], 0.75));
        // Favouring the current key pulls every sample
        // below its even position; favouring the next key
        // pushes every sample above it. Order and the
        // open interval hold at the extremes.
        let even = breakdown_fractions(0.5, 4);
        let toward_a = breakdown_fractions(0.1, 4);
        let toward_b = breakdown_fractions(0.9, 4);
        for k in 0..4 {
            assert!(toward_a[k] < even[k], "favour A crowds toward A: {toward_a:?}");
            assert!(toward_b[k] > even[k], "favour B crowds toward B: {toward_b:?}");
        }
        for fr in [toward_a, toward_b] {
            assert!(fr.windows(2).all(|w| w[0] < w[1]), "strictly increasing: {fr:?}");
            assert!(fr.iter().all(|u| *u > 0.0 && *u < 1.0), "inside (0, 1): {fr:?}");
        }
    }

    #[test]
    fn breakdown_at_favour_half_linear_matches_legacy_inbetween_poses() {
        let mut legacy = breakdown_core(Easing::Linear);
        legacy.insert_inbetween();
        let mut core = breakdown_core(Easing::Linear);
        assert_eq!(insert_breakdowns(&mut core, 0.5, 1), 1);
        assert_eq!(core.project.frames.len(), legacy.project.frames.len());
        assert_eq!(core.current, legacy.current);
        let got = &core.project.frames[1].poses[0];
        let want = &legacy.project.frames[1].poses[0];
        assert!(approx(got.root[0], want.root[0]) && approx(got.root[1], want.root[1]));
        assert!(got.angles.iter().zip(want.angles.iter()).all(|(g, w)| approx(*g, *w)));
        // The designed difference: the legacy insert
        // stores NO bends (the breakdown frame rendered
        // unbent between two bent frames); the breakdown
        // interpolates them — 0.8 and -0.4 at u = 0.5.
        assert!(legacy.project.frames[1].bends.is_empty());
        let bends = &core.project.frames[1].bends[0];
        assert!(approx(bends[0], 0.4) && approx(bends[1], -0.2), "bends: {bends:?}");
    }

    #[test]
    fn breakdown_carries_the_log_lerped_pose_scale() {
        // W2-31 (P-5b): breakdowns sample through Pose::lerp,
        // so a breakdown between a squashed key (0.5, 0.25)
        // and a stretched key (2.0, 4.0) authors the
        // LOG-space midpoint scale (1.0, 1.0) with no
        // breakdown-code change — the fields ride the pose.
        let mut core = breakdown_core(Easing::Linear);
        core.project.frames[0].poses[0].sx = 0.5;
        core.project.frames[0].poses[0].sy = 0.25;
        core.project.frames[1].poses[0].sx = 2.0;
        core.project.frames[1].poses[0].sy = 4.0;
        assert_eq!(insert_breakdowns(&mut core, 0.5, 1), 1);
        let mid = &core.project.frames[1].poses[0];
        assert!(approx(mid.sx, 1.0), "breakdown sx = {}", mid.sx);
        assert!(approx(mid.sy, 1.0), "breakdown sy = {}", mid.sy);
    }

    #[test]
    fn breakdown_pose_is_shaped_by_the_segment_easing() {
        // EaseIn at u = 0.5 shapes the pose fraction to
        // 0.25: root x = 100 + 200 * 0.25 = 150 — the
        // pose tween playback shows halfway in TIME is
        // not the pose the breakdown authors.
        let mut core = breakdown_core(Easing::EaseIn);
        assert_eq!(insert_breakdowns(&mut core, 0.5, 1), 1);
        let p = &core.project.frames[1].poses[0];
        assert!(approx(p.root[0], 150.0), "root: {:?}", p.root);
        assert!(approx(p.angles[0], 10.0) && approx(p.angles[1], 20.0), "angles: {:?}", p.angles);
    }

    #[test]
    fn breakdown_pose_prefers_the_frame_easing_override() {
        // The frame's own easing override beats the
        // project easing (poses_at's resolution): EaseOut
        // at u = 0.5 shapes to 0.75.
        let mut core = breakdown_core(Easing::Linear);
        core.project.frames[0].easing = Some(Easing::EaseOut);
        assert_eq!(insert_breakdowns(&mut core, 0.5, 1), 1);
        let p = &core.project.frames[1].poses[0];
        assert!(approx(p.root[0], 250.0), "root: {:?}", p.root);
    }

    #[test]
    fn breakdown_bends_lerp_at_the_raw_fraction_not_the_eased_one() {
        // EaseIn at favour 0.25: the POSE fraction shapes
        // to 0.0625 (root x = 112.5) while the BENDS take
        // the raw 0.25 (0.8 * 0.25 = 0.2) — bends_at's
        // asymmetry, pinned so the two samplers can never
        // silently converge.
        let mut core = breakdown_core(Easing::EaseIn);
        assert_eq!(insert_breakdowns(&mut core, 0.25, 1), 1);
        let f = &core.project.frames[1];
        assert!(approx(f.poses[0].root[0], 112.5), "root: {:?}", f.poses[0].root);
        assert!(approx(f.bends[0][0], 0.2) && approx(f.bends[0][1], -0.1), "bends: {:?}", f.bends);
    }

    #[test]
    fn breakdown_without_bend_data_keeps_the_legacy_representation() {
        let mut core = breakdown_core(Easing::Linear);
        core.project.frames[0].bends = Vec::new();
        core.project.frames[1].bends = Vec::new();
        assert_eq!(insert_breakdowns(&mut core, 0.5, 1), 1);
        assert!(core.project.frames[1].bends.is_empty());
    }

    #[test]
    fn breakdown_refuses_on_the_last_frame_without_a_snapshot() {
        let mut core = breakdown_core(Easing::Linear);
        core.current = 1;
        let depth = core.undo_depth();
        assert_eq!(insert_breakdowns(&mut core, 0.5, 1), 0);
        assert_eq!(core.status, "Breakdown needs a following frame.");
        assert_eq!(core.project.frames.len(), 2);
        assert_eq!(core.undo_depth(), depth, "the refusal spends no undo step");
        // A non-finite favour refuses the same way.
        core.current = 0;
        assert_eq!(insert_breakdowns(&mut core, f32::NAN, 1), 0);
        assert_eq!(core.undo_depth(), depth);
        assert_eq!(core.project.frames.len(), 2);
    }

    #[test]
    fn breakdown_is_one_undo_step_for_the_whole_insert() {
        let mut core = breakdown_core(Easing::Linear);
        let depth = core.undo_depth();
        assert_eq!(insert_breakdowns(&mut core, 0.5, 4), 4);
        assert_eq!(core.undo_depth(), depth + 1, "exactly one snapshot");
        assert_eq!(core.project.frames.len(), 6);
        assert_eq!(core.current, 1, "the playhead lands on the first breakdown");
        assert!(
            approx(core.project.frames[5].poses[0].root[0], 300.0),
            "the old next frame shifted past all four breakdowns"
        );
        core.undo();
        assert_eq!(core.project.frames.len(), 2, "one undo removes the whole insert");
    }

    #[test]
    fn breakdown_clamps_count_and_favour() {
        let mut core = breakdown_core(Easing::Linear);
        assert_eq!(insert_breakdowns(&mut core, 0.5, 99), BREAKDOWN_COUNT_MAX);
        // Favour below the floor clamps to 0.1 (Linear:
        // root x = 100 + 200 * 0.1 = 120).
        let mut core = breakdown_core(Easing::Linear);
        assert_eq!(insert_breakdowns(&mut core, 0.0, 1), 1);
        assert!(approx(core.project.frames[1].poses[0].root[0], 120.0));
    }

    #[test]
    fn breakdown_leaves_frame_pins_absolute() {
        // The module census rule, on the swap suite's own
        // staged project: pins at frames 1 and 3 keep
        // their stored numbers while two breakdowns are
        // spliced in after the playhead (frame 2).
        let mut core = staged_core();
        let pins_before = all_pin_frames(&core);
        assert_eq!(insert_breakdowns(&mut core, 0.5, 2), 2);
        assert_eq!(core.project.frames.len(), 6);
        assert_eq!(all_pin_frames(&core), pins_before, "no pin moved");
    }

    #[test]
    fn breakdown_handler_validates_and_reports() {
        let mut core = breakdown_core(Easing::Linear);
        let out = tool_insert_breakdown(&mut core, &json!({})).unwrap();
        assert_eq!(out["inserted"], 1);
        assert_eq!(out["frame_count"], 3);
        let err = tool_insert_breakdown(&mut core, &json!({ "favour": 1.5 })).unwrap_err();
        assert!(err.contains("between 0 and 1"), "{err}");
        let err = tool_insert_breakdown(&mut core, &json!({ "count": 0 })).unwrap_err();
        assert!(err.contains("at least 1"), "{err}");
        // On the last frame the op's refusal surfaces.
        core.current = core.project.frames.len() - 1;
        let err = tool_insert_breakdown(&mut core, &json!({})).unwrap_err();
        assert_eq!(err, "Breakdown needs a following frame.");
    }

    // ---- Roadmap Wave 2 (W2-38): breakdown chains ----

    /// Four frames whose roots step 100 units apart on x
    /// (0, 100, 200, 300), angles stepping 10 per frame,
    /// bends stepping 0.4 per frame, Linear project
    /// easing: the chain testbed — a span of three
    /// segments with a known sample at every fraction.
    fn chain_core() -> EditorCore {
        let mut core = EditorCore::new();
        core.project.easing = Easing::Linear;
        core.project.frames = (0..4)
            .map(|k| Frame {
                poses: vec![bd_pose([100.0 * k as f32, 200.0], &[10.0 * k as f32, 10.0])],
                duration: 1.0,
                easing: None,
                bends: vec![vec![0.4 * k as f32, 0.0]],
            })
            .collect();
        core.current = 0;
        core
    }

    #[test]
    fn breakdown_chain_positions_distribute_over_the_whole_span() {
        // Even spacing over three segments: the global
        // fractions 0.25 / 0.5 / 0.75 land at span
        // positions 0.75 / 1.5 / 2.25 — the local
        // fractions DESCEND (0.75, 0.5, 0.25) as the
        // chain crosses segments, the signature of one
        // curve over the span rather than one per
        // segment.
        let pos = breakdown_chain_positions(0.5, 3, 3);
        assert_eq!(pos.len(), 3);
        assert_eq!(pos[0].0, 0);
        assert!(approx(pos[0].1, 0.75), "positions: {pos:?}");
        assert_eq!(pos[1].0, 1);
        assert!(approx(pos[1].1, 0.5), "positions: {pos:?}");
        assert_eq!(pos[2].0, 2);
        assert!(approx(pos[2].1, 0.25), "positions: {pos:?}");
        // A single even breakdown over three segments
        // lands inside the MIDDLE segment.
        assert_eq!(breakdown_chain_positions(0.5, 1, 3), vec![(1, 0.5)]);
        // Favouring the current key crowds the whole
        // chain toward it, across segments too.
        let toward_a = breakdown_chain_positions(0.25, 2, 4);
        assert_eq!(toward_a.len(), 2);
        assert!(toward_a.iter().all(|(seg, _)| *seg <= 1), "crowded early: {toward_a:?}");
    }

    #[test]
    fn breakdown_chain_positions_skip_positions_on_existing_frames() {
        // Even spacing over a commensurate span lands
        // every position exactly on a frame: all skipped.
        assert!(breakdown_chain_positions(0.5, 3, 4).is_empty());
        assert!(breakdown_chain_positions(0.5, 1, 2).is_empty());
        assert!(breakdown_chain_positions(0.5, 1, 4).is_empty());
        // Mixed: over six segments the middle position
        // (span position 3.0) is a frame; the other two
        // land inside segments 1 and 4.
        assert_eq!(
            breakdown_chain_positions(0.5, 3, 6),
            vec![(1, 0.5), (4, 0.5)]
        );
        // A non-finite favour yields no positions.
        assert!(breakdown_chain_positions(f32::NAN, 3, 3).is_empty());
    }

    #[test]
    fn breakdown_chain_positions_are_monotonic_and_clamped() {
        let pos = breakdown_chain_positions(0.9, 99, 999);
        assert!(pos.len() <= BREAKDOWN_CHAIN_COUNT_MAX, "count clamped: {}", pos.len());
        assert!(
            pos.iter().all(|(seg, u)| *seg < BREAKDOWN_CHAIN_SPAN_MAX && *u > 0.0 && *u < 1.0),
            "inside the span: {pos:?}"
        );
        assert!(
            pos.windows(2).all(|w| w[0].0 < w[1].0 || (w[0].0 == w[1].0 && w[0].1 < w[1].1)),
            "strictly increasing: {pos:?}"
        );
        // A zero span clamps to one segment.
        let one = breakdown_chain_positions(0.5, 2, 0);
        assert_eq!(one.len(), 2);
        assert!(one.iter().all(|(seg, _)| *seg == 0), "one segment: {one:?}");
    }

    #[test]
    fn breakdown_chain_authors_the_span_distribution() {
        let mut core = chain_core();
        assert_eq!(insert_breakdown_chain(&mut core, 0.5, 3, 3), 3);
        assert_eq!(core.project.frames.len(), 7);
        // The roots read the whole distribution off the
        // timeline: 75 (seg 0 @ 0.75), 150 (seg 1 @ 0.5),
        // 225 (seg 2 @ 0.25) spliced between the keys.
        let roots: Vec<f32> = core
            .project
            .frames
            .iter()
            .map(|f| f.poses[0].root[0])
            .collect();
        let want = [0.0, 75.0, 100.0, 150.0, 200.0, 225.0, 300.0];
        assert_eq!(roots.len(), want.len());
        assert!(
            roots.iter().zip(want.iter()).all(|(g, w)| approx(*g, *w)),
            "roots: {roots:?}"
        );
        // Bends sample at the RAW local fraction: seg 0
        // @ 0.75 lerps 0.0 -> 0.4 to 0.3.
        assert!(approx(core.project.frames[1].bends[0][0], 0.3));
        assert_eq!(core.current, 1, "the playhead lands on the first breakdown");
        assert_eq!(
            core.status,
            "Inserted 3 breakdowns across 3 segments after frame 1."
        );
    }

    #[test]
    fn breakdown_chain_samples_each_segment_with_its_own_easing() {
        // An easing override on the span's FIRST frame
        // shapes only its own segment's samples: seg 0
        // @ 0.75 with EaseIn poses at 0.5625 (root
        // 56.25), while seg 1 stays Linear (150).
        let mut core = chain_core();
        core.project.frames[0].easing = Some(Easing::EaseIn);
        assert_eq!(insert_breakdown_chain(&mut core, 0.5, 3, 3), 3);
        let roots: Vec<f32> = core
            .project
            .frames
            .iter()
            .map(|f| f.poses[0].root[0])
            .collect();
        assert!(approx(roots[1], 56.25), "roots: {roots:?}");
        assert!(approx(roots[3], 150.0), "roots: {roots:?}");
        assert!(approx(roots[5], 225.0), "roots: {roots:?}");
    }

    #[test]
    fn breakdown_chain_with_span_one_matches_insert_breakdowns() {
        let mut chained = breakdown_core(Easing::Linear);
        assert_eq!(insert_breakdown_chain(&mut chained, 0.25, 2, 1), 2);
        let mut single = breakdown_core(Easing::Linear);
        assert_eq!(insert_breakdowns(&mut single, 0.25, 2), 2);
        assert_eq!(chained.project.frames.len(), single.project.frames.len());
        assert_eq!(chained.current, single.current);
        assert_eq!(chained.status, single.status);
        for (a, b) in chained.project.frames.iter().zip(single.project.frames.iter()) {
            assert_eq!(a.poses.len(), b.poses.len());
            for (pa, pb) in a.poses.iter().zip(b.poses.iter()) {
                assert!(approx(pa.root[0], pb.root[0]) && approx(pa.root[1], pb.root[1]));
                assert!(pa.angles.iter().zip(pb.angles.iter()).all(|(x, y)| approx(*x, *y)));
            }
            assert_eq!(a.bends, b.bends);
        }
    }

    #[test]
    fn breakdown_chain_is_one_undo_step_for_the_whole_chain() {
        let mut core = chain_core();
        let depth = core.undo_depth();
        assert_eq!(insert_breakdown_chain(&mut core, 0.5, 3, 3), 3);
        assert_eq!(core.undo_depth(), depth + 1, "exactly one snapshot");
        core.undo();
        assert_eq!(core.project.frames.len(), 4, "one undo removes the whole chain");
    }

    #[test]
    fn breakdown_chain_refusals_spend_no_snapshot() {
        // No following frame.
        let mut core = chain_core();
        core.current = 3;
        let depth = core.undo_depth();
        assert_eq!(insert_breakdown_chain(&mut core, 0.5, 3, 3), 0);
        assert_eq!(core.status, "Breakdown chain needs a following frame.");
        assert_eq!(core.undo_depth(), depth);
        assert_eq!(core.project.frames.len(), 4);
        // A non-finite favour refuses the same way.
        core.current = 0;
        assert_eq!(insert_breakdown_chain(&mut core, f32::NAN, 3, 3), 0);
        assert_eq!(
            core.status,
            "Breakdown favour must be a finite fraction between 0 and 1."
        );
        assert_eq!(core.undo_depth(), depth);
        // Every position on an existing frame: an even
        // chain over a four-segment span of five frames.
        let mut five = chain_core();
        five.project.frames.push(Frame {
            poses: vec![bd_pose([400.0, 200.0], &[40.0, 10.0])],
            duration: 1.0,
            easing: None,
            bends: vec![vec![1.6, 0.0]],
        });
        let depth = five.undo_depth();
        assert_eq!(insert_breakdown_chain(&mut five, 0.5, 3, 4), 0);
        assert_eq!(
            five.status,
            "Breakdown chain lands only on existing frames — nothing to insert."
        );
        assert_eq!(five.undo_depth(), depth);
        assert_eq!(five.project.frames.len(), 5);
    }

    #[test]
    fn breakdown_chain_clamps_span_to_the_timeline_and_count_to_the_cap() {
        // Span 64 asked of a three-segment timeline: the
        // chain covers the three segments that exist and
        // the status says so.
        let mut core = chain_core();
        assert_eq!(insert_breakdown_chain(&mut core, 0.25, 1, 64), 1);
        assert_eq!(
            core.status,
            "Inserted 1 breakdown across 3 segments after frame 1."
        );
        assert!(approx(core.project.frames[1].poses[0].root[0], 75.0));
        // Count 99 clamps to the chain cap (32): the op
        // authors exactly the positions the public
        // helper computes for the clamped parameters —
        // never more than the cap. (Over three segments
        // the even positions k/11 also land on frames at
        // k = 11 and k = 22, so the helper's list is
        // shorter than 32; the skip rule itself is
        // pinned exactly in the positions tests above.)
        let mut core = chain_core();
        let expected = breakdown_chain_positions(0.5, 99, 3).len();
        assert!(expected <= BREAKDOWN_CHAIN_COUNT_MAX);
        assert_eq!(insert_breakdown_chain(&mut core, 0.5, 99, 3), expected);
        assert_eq!(core.project.frames.len(), 4 + expected);
    }

    #[test]
    fn breakdown_chain_leaves_frame_pins_absolute() {
        // The module census rule, on the staged project:
        // its playhead has exactly one segment after it,
        // so a span-1 chain splices two breakdowns in —
        // and no pin in any collection moves.
        let mut core = staged_core();
        let pins_before = all_pin_frames(&core);
        assert_eq!(insert_breakdown_chain(&mut core, 0.5, 2, 1), 2);
        assert_eq!(core.project.frames.len(), 6);
        assert_eq!(all_pin_frames(&core), pins_before, "no pin moved");
    }

    #[test]
    fn breakdown_chain_handler_validates_and_reports() {
        let mut core = chain_core();
        let out = tool_insert_breakdown_chain(&mut core, &json!({})).unwrap();
        assert_eq!(out["inserted"], 1, "defaults: favour 0.5, count 1, span 3");
        assert_eq!(out["frame_count"], 5);
        let err = tool_insert_breakdown_chain(&mut core, &json!({ "favour": 1.5 })).unwrap_err();
        assert!(err.contains("between 0 and 1"), "{err}");
        let err = tool_insert_breakdown_chain(&mut core, &json!({ "count": 0 })).unwrap_err();
        assert!(err.contains("at least 1"), "{err}");
        let err = tool_insert_breakdown_chain(&mut core, &json!({ "span": 0 })).unwrap_err();
        assert!(err.contains("at least 1"), "{err}");
        // On the last frame the op's refusal surfaces.
        core.current = core.project.frames.len() - 1;
        let err = tool_insert_breakdown_chain(&mut core, &json!({})).unwrap_err();
        assert_eq!(err, "Breakdown chain needs a following frame.");
    }
}
