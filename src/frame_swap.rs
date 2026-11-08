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
use crate::model::InstanceKey;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Channel, ChannelTarget, Key};
    use crate::model::{
        AudioClip, CameraKey, Easing, Frame, FxClipEntry, ParticleEvent, ParticleKind,
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
}
