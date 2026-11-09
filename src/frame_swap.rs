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
    core.snapshot();
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
///   …) are NOT remapped, matching `add_frame`.
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
    core.snapshot();
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
            CameraKey { frame: 0, x: 1.0, y: 0.0, zoom: 1.0, angle: 0.0 },
            CameraKey { frame: A, x: 10.0, y: 0.0, zoom: 1.0, angle: 0.0 },
            CameraKey { frame: B, x: 30.0, y: 0.0, zoom: 1.0, angle: 0.0 },
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
        }];
        let pose = |x: f32| crate::model::Pose { root: [x, 0.0], angles: vec![0.0] };
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
        def_with_media.nodes[3].media = Some(SegmentMedia::Image { asset: 1 });
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
            },
            FigureInstance {
                name: "Clipper".into(),
                color: 0x00FF00,
                def: FigureDef::humanoid(),
                hidden: false,
                locked: false,
                opacity: 1.0,
                clip: Some(0),
            },
        ];
        core.project.clips = vec![ClipDef {
            name: "wave".into(),
            def: FigureDef::humanoid(),
            frames: vec![
                Pose { root: [1.0, 2.0], angles: vec![0.0] },
                Pose { root: [3.0, 4.0], angles: vec![0.5] },
            ],
            loop_mode: ClipLoop::Loop,
        }];
        let two_pose_frame = |r0: f32, r1: f32, b0: f32, b1: f32, duration: f32| Frame {
            poses: vec![
                Pose { root: [r0, 0.0], angles: vec![0.0] },
                Pose { root: [r1, 0.0], angles: vec![0.0] },
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
            Some(SegmentMedia::Image { asset: 1 }),
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
}
