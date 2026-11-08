//! Playback-audio glue between [`EditorCore`] and the engine's
//! existing audio player (Rune Phase F).
//!
//! ## What already existed
//!
//! The engine has had a real output player since Wave 10:
//! [`crate::audio::AudioPlayer`] spawns the system `ffplay`
//! (which ships with the ffmpeg the exporter already depends
//! on) and its `play_project` entry point plays *every* clip
//! audible at a playhead, rendering a mix with
//! [`crate::audio_edit::mix`] when more than one clip sounds —
//! the same mixer the MP4 export muxes with (`export.rs` calls
//! `audio_edit::mix` for its audio track). What the Rune
//! migration removed was not the player but its *owner*: in the
//! GPUI shell the app state held the `AudioPlayer` and its
//! `toggle_play` called `play_project` on Play and `stop` when
//! playback ended. Phase B deleted that shell, and no Rune code
//! has called the player since — which is why pressing Play in
//! the Rune editor has been silent.
//!
//! ## What this module adds
//!
//! The `EditorCore` methods the Rune shell needs —
//! [`EditorCore::start_playback_audio`],
//! [`EditorCore::stop_playback_audio`],
//! [`EditorCore::sync_playback_audio_on_wrap`],
//! [`EditorCore::sync_playback_audio_on_seek`],
//! [`EditorCore::sync_playback_audio_on_drift`],
//! [`EditorCore::sync_playback_audio_on_full_loop`],
//! [`EditorCore::playback_audio_active`] and
//! [`EditorCore::note_playback_speed`] — delegating to that
//! same player and mix. Start uses the GPUI shell's exact
//! playhead convention: `current` frame over `fps` (floored at
//! 1), so the audio starts at the frame the user sees selected.
//! The wrap sync closes the loop-wrap limit (Phase G): when
//! the shell's per-tick frame tracking shows the playhead
//! wrapped while playing, it restarts the audio from the
//! playhead, so each visual loop gets its own audio pass. The
//! seek sync closes the scrub-follow limit (Phase I): when
//! the tracking shows the playhead jumping forward while
//! playing, it restarts the audio from the new playhead, so a
//! seek is heard from where the picture landed. The drift
//! sync (Phase J) bounds the last gap — the missing
//! sample-clock sync: every start records a playback clock
//! (wall-clock instant + the frame the sound started at),
//! and when the shell's fractional playhead has wandered
//! further from the clock's prediction than
//! [`DRIFT_TOLERANCE_FRAMES`], the mix restarts at the
//! playhead, re-basing the clock. The prediction itself
//! walks the timeline at each frame's real duration and
//! at the speed the shell notes on every playing tick
//! (Phase K), so slow frames and fast playback are
//! predicted as the picture actually moves. The full-loop
//! sync (Phase L) reads the same clock and walk a second
//! way — counting its folds — to close the last loop
//! shape: a whole loop completed inside one poll tick,
//! which the frame-comparing wrap sync cannot see.
//!
//! ## Where the player lives, and why
//!
//! `EditorCore` cannot grow a field from this file (the struct
//! is defined in `editor.rs`), and a process handle would not
//! belong in the core's snapshot model anyway, so the player
//! lives in a thread-local beside the glue — the same ownership
//! shape as the GPUI shell's app state, one level down. The
//! editor is a single-threaded UI, so the UI thread's player is
//! the only one the shell can ever reach; as a side benefit,
//! each unit-test thread gets an isolated player. The player's
//! own `Drop` stops its `ffplay` process, including when a
//! thread exits.
//!
//! ## Honest limits (inherited, not new)
//!
//! The player does not synchronise against a sample clock
//! (`audio.rs` documents this). Phase J's drift sync bounds
//! what that costs: within one pass the audio and the picture
//! can still drift apart, but only until the gap exceeds
//! [`DRIFT_TOLERANCE_FRAMES`] (2.5 frames), at which point
//! [`EditorCore::sync_playback_audio_on_drift`] restarts the
//! mix at the playhead. The correction is a restart, not a
//! nudge — a listener may hear the seam. The prediction
//! behind it is no longer the naive one: since Phase K it
//! walks the timeline the way `advance_play` moves through
//! it ([`expected_frame_walk`]) — each frame costs its real
//! duration (`duration / fps` seconds, the duration floored
//! at 0.05 exactly as `advance_play` floors it) scaled by
//! the noted playback speed, and the walk folds at the
//! loop-range end or the timeline end exactly as
//! `advance_play` folds — so a long slow frame no longer
//! trips the tolerance on its own, and the prediction
//! stays comparable to the playhead even in the gap
//! between a wrap and the wrap sync's restart. Two
//! approximations remain, both small against the 2.5-frame
//! tolerance:
//!
//! - **Boundary slicing vs per-tick charging.** The walk
//!   slices the elapsed time at frame boundaries, while
//!   `advance_play` charges each poll tick's whole `dt` at
//!   the rate of the frame current when the tick began.
//!   The two agree everywhere except on ticks that
//!   straddle a boundary, and there only by a fraction of
//!   a frame.
//! - **The walk's crossing cap.** The walk takes at most
//!   2 × frame count + 2 frame crossings, so after an
//!   absurd gap (a stall spanning many full loops) the
//!   prediction saturates short of the playhead. The
//!   drift then reads as exceeded and the sync re-bases
//!   at the playhead — the safe answer to a gap that
//!   large.
//!
//! The loop limit is closed for the wrap case:
//! [`EditorCore::sync_playback_audio_on_wrap`] restarts the
//! audio whenever the visual playhead wraps — at the end of
//! the timeline, or at the end of a loop range (the restart
//! starts from the wrapped-to frame, so a range that begins
//! mid-timeline restarts its audio at the range start, which
//! is where the picture is).
//!
//! The scrub-follow limit is closed in both directions
//! (Phase I): a backward seek while playing lowers `current`,
//! which the wrap restart already reads as its signal, and
//! [`EditorCore::sync_playback_audio_on_seek`] restarts the
//! audio whenever a tick shows the playhead jumping forward
//! by more than one frame. The shell's poll calls the two
//! syncs side by side each tick (integrator wiring).
//!
//! The last loop shape is closed too (Phase L): a tick
//! that advanced the playhead by a full loop or more and
//! landed on the previous frame or the one after it was,
//! to the frame comparison, indistinguishable from holding
//! or ordinary forward motion, so no restart fired — the
//! standing miss since Phase I.
//! [`EditorCore::sync_playback_audio_on_full_loop`] detects
//! that loop from the playback clock instead: [`walk_wraps`]
//! counts the folds in the same duration- and speed-aware
//! walk the drift sync predicts with, and a completed fold
//! since the running pass started restarts the mix at the
//! current playhead. Its guard stands down whenever the
//! playhead decreased across the tick — that shape is the
//! wrap sync's, and visible to it — so the two never
//! restart for the same tick.

use std::cell::RefCell;
use std::time::Instant;

use crate::audio::AudioPlayer;
use crate::editor::EditorCore;

thread_local! {
    /// The one playback player for this thread (see the module
    /// docs for why it lives here rather than in `EditorCore`).
    static PLAYER: RefCell<AudioPlayer> = RefCell::new(AudioPlayer::new());

    /// The playback clock for the current pass: the wall-clock
    /// instant the running sound started, and the frame it
    /// started at. Written by
    /// [`EditorCore::start_playback_audio`] on every successful
    /// start (so the wrap, seek and drift restarts — all of
    /// which funnel through stop + start — re-base it), and
    /// cleared by [`EditorCore::stop_playback_audio`]. It lives
    /// beside the player for the same reason the player does:
    /// it is process state, not project state, and does not
    /// belong in the core's snapshot model.
    static CLOCK: RefCell<Option<(Instant, f32)>> = const { RefCell::new(None) };

    /// The playback speed the shell last noted through
    /// [`EditorCore::note_playback_speed`] (Phase K), already
    /// sanitised by that call. It lives beside the clock
    /// because the drift prediction consumes it together
    /// with the clock: the picture advances at
    /// `dt * speed`, so the sound's expected position must
    /// be predicted at the same rate. Unlike the clock it
    /// is *not* cleared by stop or re-based by start — the
    /// speed is the shell's setting, not a property of one
    /// pass, and the shell re-notes it on every playing
    /// tick before the syncs run.
    static SPEED: RefCell<f32> = const { RefCell::new(1.0) };
}

/// The slowest playback speed the drift prediction will
/// honour: half a twentieth of normal rate. The floor keeps
/// a noted 0 (or a denormal) from freezing the prediction
/// entirely; it is a sanity bound, not a product limit —
/// the shell's own speed control lives far inside it.
pub const PLAYBACK_SPEED_MIN: f32 = 0.05;

/// The fastest playback speed the drift prediction will
/// honour: eight times normal rate, a generous fast-forward
/// bound on the same sanity-only terms as
/// [`PLAYBACK_SPEED_MIN`].
pub const PLAYBACK_SPEED_MAX: f32 = 8.0;

/// The contract [`EditorCore::note_playback_speed`] stores
/// by: a finite, positive speed is clamped into
/// [`PLAYBACK_SPEED_MIN`]..=[`PLAYBACK_SPEED_MAX`]; anything
/// else — zero, negative, NaN, infinite — means "no sane
/// speed was noted" and falls back to 1.0, normal rate,
/// rather than to a clamp bound (a noted 0 clamped to the
/// floor would make the prediction crawl at a twentieth of
/// the picture's rate, which is a worse lie than normal
/// rate for input that meant nothing).
fn sanitise_playback_speed(speed: f32) -> f32 {
    if speed.is_finite() && speed > 0.0 {
        speed.clamp(PLAYBACK_SPEED_MIN, PLAYBACK_SPEED_MAX)
    } else {
        1.0
    }
}

/// The playback speed the drift prediction should use: the
/// last noted (and sanitised) value, 1.0 until the shell's
/// first note.
fn playback_speed() -> f32 {
    SPEED.with(|speed| *speed.borrow())
}

/// The playhead position `start_playback_audio` starts from, in
/// seconds: the current frame over the project frame rate, with
/// the rate floored at 1 fps — the GPUI shell's formula, and the
/// same defensive floor the MP4 audio mux uses
/// (`audio::clip_offset_secs`).
fn playhead_secs(core: &EditorCore) -> f32 {
    core.current as f32 / core.project.fps.max(1.0)
}

/// Did the playhead wrap between two poll ticks? Pure decision
/// behind [`EditorCore::sync_playback_audio_on_wrap`].
///
/// The core's wrap semantics (`advance_play` in `editor.rs`):
/// the fractional playhead advances monotonically and is folded
/// back by the timeline length — or, with a loop range set, by
/// the range's span — so the integer `current` frame it lands
/// on *decreases* exactly when a wrap happened: from the last
/// frame (or the range end) back to frame 0 (or the range
/// start). A decrease while playing is therefore the wrap
/// signal, and nothing else in normal playback produces one —
/// forward motion never lowers `current`, and a paused playhead
/// does not move at all.
///
/// The guards keep stale or degenerate inputs from reading as
/// a wrap: both ticks must be playing (a frame drop across a
/// pause is a seek the user made while stopped, and Play
/// already restarts the audio for it), and the timeline needs
/// at least two frames — with zero frames `advance_play` never
/// runs, and with one frame `current` is always 0, so no
/// decrease can occur.
fn playhead_wrapped(
    was_playing: bool,
    now_playing: bool,
    prev_frame: usize,
    cur_frame: usize,
    frame_count: usize,
) -> bool {
    was_playing && now_playing && frame_count > 1 && cur_frame < prev_frame
}

/// Did the playhead jump forward between two poll ticks? Pure
/// decision behind [`EditorCore::sync_playback_audio_on_seek`].
///
/// Normal playback advances `current` by exactly one frame
/// per tick (or holds it, on a long-duration frame), so a
/// tick that lands more than one frame ahead is a
/// discontinuity: the user seeked or scrubbed forward, and
/// the audio — still playing its original course — no longer
/// matches the picture. Backward motion is deliberately not
/// this function's business: a decrease is the wrap signal
/// `playhead_wrapped` reads, and a backward seek while
/// playing produces exactly that decrease, so the wrap
/// restart already re-syncs it.
///
/// A stall counts too: a tick delayed long enough for the
/// playhead to skip two or more frames reads as a seek. That
/// is desirable, not a false positive — the two clocks were
/// free to drift across the stall (there is no sample-clock
/// sync; see the module docs), and restarting re-anchors the
/// audio to where the picture actually landed.
pub fn playhead_seeked(prev_frame: usize, cur_frame: usize) -> bool {
    cur_frame > prev_frame && cur_frame - prev_frame > 1
}

/// How far apart, in frames, the picture and the sound may
/// drift before [`EditorCore::sync_playback_audio_on_drift`]
/// restarts the mix. Two and a half frames is a little under
/// a tenth of a second at the rates this editor targets —
/// large enough that ordinary scheduling jitter between the
/// shell's poll ticks and the player's process never trips
/// it, small enough that a listener is unlikely to notice
/// lip-sync error before the correction lands.
pub const DRIFT_TOLERANCE_FRAMES: f32 = 2.5;

/// The signed drift between where the playback clock says
/// the sound should be (`expected`) and where the picture
/// actually is (`actual`), in frames: positive when the
/// playhead is ahead of the sound, negative when it trails
/// it. Pure arithmetic behind
/// [`EditorCore::sync_playback_audio_on_drift`].
pub fn drift_frames(expected: f32, actual: f32) -> f32 {
    actual - expected
}

/// Has the drift grown past [`DRIFT_TOLERANCE_FRAMES`] in
/// either direction? The comparison is strict: a drift of
/// exactly the tolerance is still within bounds.
pub fn drift_exceeded(expected: f32, actual: f32) -> bool {
    drift_frames(expected, actual).abs() > DRIFT_TOLERANCE_FRAMES
}

/// Where the playback clock predicts the playhead should be
/// `elapsed_secs` after a pass started at `start_frame`, on
/// a timeline where every frame lasts exactly one frame
/// period at speed 1.0: the start frame plus elapsed time
/// at the project rate, the rate floored at 1 fps exactly
/// as `playhead_secs` floors it. This is the uniform special
/// case [`expected_frame_walk`] reduces to when all
/// durations are 1.0 and the speed is 1.0 (before any
/// folding); the drift sync predicts with the walk, and
/// this plain form is kept as the reference the walk is
/// pinned against.
pub fn expected_frame(start_frame: f32, elapsed_secs: f32, fps: f32) -> f32 {
    start_frame + elapsed_secs * fps.max(1.0)
}

/// Where the playback clock predicts the playhead should be
/// `elapsed_secs` after a pass started at the fractional
/// `start_frame`, walking the real timeline: each frame
/// occupies `duration / fps` seconds (its duration
/// multiplier over the rate, the rate floored at 1 fps),
/// the elapsed time is spent at `speed` (poll advances the
/// picture with `advance_play(dt * speed)`, so `speed`
/// seconds of timeline pass per wall second), and the
/// position folds at the loop range's end — or the
/// timeline's, with no range — stepping back by the span,
/// clamped and folded exactly as `advance_play` clamps and
/// folds (range end to the last frame, range start to the
/// end, fold when the playhead reaches the end of the last
/// frame in the window). Durations are floored at 0.05,
/// `advance_play`'s own floor, so a zero or negative
/// duration still costs time instead of none.
///
/// `durations` carries one entry per frame, so its length
/// is the frame count; an empty slice means no timeline,
/// and the start frame comes back unchanged. A
/// non-positive or NaN elapsed budget moves nothing
/// either — the folded start comes back, folding being
/// what `advance_play` does to a standing playhead, not
/// motion.
///
/// One difference from `advance_play` is deliberate: the
/// walk slices the elapsed time at frame boundaries,
/// charging each frame only for the time spent inside it,
/// where `advance_play` charges a whole tick's `dt` at the
/// rate of the frame current when the tick began. Over the
/// many ticks between two clock readings the two agree to
/// within a fraction of a frame (see the module docs).
///
/// The walk takes at most `2 * frame count + 2` frame
/// crossings — two full traversals plus the partial frames
/// at each end — so an absurd elapsed can never make it
/// spin; on saturation it stops where it is, and the
/// caller's drift reading simply exceeds tolerance and
/// re-bases (the safe answer to a gap that large).
pub fn expected_frame_walk(
    start_frame: f32,
    elapsed_secs: f32,
    speed: f32,
    fps: f32,
    durations: &[f32],
    loop_range: Option<(usize, usize)>,
) -> f32 {
    let n = durations.len();
    if n == 0 {
        return start_frame;
    }
    let fps = fps.max(1.0);
    // The fold window, clamped exactly as `advance_play`
    // clamps it; with no range the window is the whole
    // timeline (start 0, end n, span n — the same numbers
    // `advance_play`'s no-range fold uses).
    let (lo, end) = match loop_range {
        Some((a, b)) => {
            let b = b.min(n - 1);
            let a = a.min(b);
            (a as f32, (b + 1) as f32)
        }
        None => (0.0, n as f32),
    };
    let span = end - lo;
    let mut pos = start_frame;
    // A stale range can leave a start past the end; fold it
    // as `advance_play`'s fold would before walking — that
    // fold runs even for a zero `dt`, so it runs here even
    // when no time turns out to have passed. (A start
    // *below* `lo` is left alone — `advance_play` plays
    // forward into the range from there too.)
    while pos >= end {
        pos -= span;
    }
    // The wall-clock budget in timeline seconds: at speed
    // `s`, `s` seconds of timeline pass per wall second.
    let mut budget = elapsed_secs * sanitise_playback_speed(speed);
    if !(budget > 0.0) {
        return pos;
    }
    let max_crossings = 2 * n + 2;
    let mut crossings = 0usize;
    while budget > 0.0 {
        // The frame the position sits in — `advance_play`'s
        // own clamp of the `current` it derives.
        let idx = (pos.floor() as usize).min(n - 1);
        let frame_secs = durations[idx].max(0.05) / fps;
        // The budget left inside this frame: the part of it
        // not yet crossed, at this frame's real length.
        let rest = (1.0 - (pos - pos.floor())) * frame_secs;
        if budget < rest {
            // The budget runs out inside this frame.
            pos += budget / frame_secs;
            budget = 0.0;
        } else {
            // The crossing cap bounds the walk, checked
            // before a crossing is performed: once it is
            // spent, a budget that still runs out inside
            // the current frame may finish there (the
            // final partial frame is part of the walk,
            // not another crossing), but no further
            // boundary is crossed.
            if crossings >= max_crossings {
                break;
            }
            budget -= rest;
            pos = pos.floor() + 1.0;
            while pos >= end {
                pos -= span;
            }
            crossings += 1;
        }
    }
    pos
}

/// How many times the walk in [`expected_frame_walk`]
/// folds: the same walk, over the same arguments, counting
/// each fold — each moment the advancing position reaches
/// the fold end (the loop range's end, or the timeline's)
/// and steps back by the span to the fold start. A count of
/// 1 or more therefore means at least one full loop of the
/// fold window completed inside the walked time, which is
/// the signal [`EditorCore::sync_playback_audio_on_full_loop`]
/// exists to catch: a loop the frame-comparing wrap sync
/// cannot see because the playhead landed on or past the
/// frame it started the tick from.
///
/// The conventions are [`expected_frame_walk`]'s own,
/// unchanged: the fold window is clamped as `advance_play`
/// clamps it, durations are floored at 0.05, the elapsed
/// time is spent at the sanitised `speed`, and the position
/// is derived with `advance_play`'s frame clamp — so with
/// uniform durations and speed 1.0 the count agrees with
/// the walk's folded result by construction (half a loop
/// counts 0, a full loop 1, two and a half loops 2).
///
/// Two details, stated plainly:
///
/// - The stale-start fold the walk applies before walking
///   (a start already past the fold end, folded back even
///   at zero elapsed) is *not* counted: it normalises a
///   standing position rather than completing a loop, and
///   in the one shape it can occur the picture's `current`
///   decreases on the next advance, which the wrap sync
///   already owns.
/// - The walk's crossing cap (`2 * frame count + 2`)
///   bounds this count too: an absurd elapsed saturates
///   the crossings exactly as it saturates the walk, and
///   the count so far comes back — still at least 1
///   whenever a real loop completed well inside the cap,
///   which is all the caller asks.
///
/// An empty `durations` slice (no timeline) and a
/// non-positive or NaN elapsed budget both count 0.
pub fn walk_wraps(
    start_frame: f32,
    elapsed_secs: f32,
    speed: f32,
    fps: f32,
    durations: &[f32],
    loop_range: Option<(usize, usize)>,
) -> u32 {
    let n = durations.len();
    if n == 0 {
        return 0;
    }
    let fps = fps.max(1.0);
    // The fold window, clamped exactly as
    // `expected_frame_walk` (and `advance_play`) clamp it.
    let (lo, end) = match loop_range {
        Some((a, b)) => {
            let b = b.min(n - 1);
            let a = a.min(b);
            (a as f32, (b + 1) as f32)
        }
        None => (0.0, n as f32),
    };
    let span = end - lo;
    let mut pos = start_frame;
    // The stale-start fold, applied as the walk applies it
    // but not counted (see the docs above).
    while pos >= end {
        pos -= span;
    }
    let mut budget = elapsed_secs * sanitise_playback_speed(speed);
    if !(budget > 0.0) {
        return 0;
    }
    let max_crossings = 2 * n + 2;
    let mut crossings = 0usize;
    let mut folds: u32 = 0;
    while budget > 0.0 {
        // The frame the position sits in — the walk's own
        // clamp, as in `expected_frame_walk`.
        let idx = (pos.floor() as usize).min(n - 1);
        let frame_secs = durations[idx].max(0.05) / fps;
        // The budget left inside this frame.
        let rest = (1.0 - (pos - pos.floor())) * frame_secs;
        if budget < rest {
            // The budget runs out inside this frame: no
            // boundary is crossed, so nothing can fold.
            pos += budget / frame_secs;
            budget = 0.0;
        } else {
            // The same cap placement as the walk: checked
            // before a crossing is performed, so at most
            // `max_crossings` boundaries are ever crossed.
            if crossings >= max_crossings {
                break;
            }
            budget -= rest;
            pos = pos.floor() + 1.0;
            while pos >= end {
                pos -= span;
                folds += 1;
            }
            crossings += 1;
        }
    }
    folds
}

/// The full-loop sync's fold count for one recorded clock:
/// [`walk_wraps`] over the core's real frame durations, its
/// loop range and the noted playback speed, for the time
/// since the clock's start — `predicted_frame`'s exact
/// inputs and collection, counting folds instead of
/// predicting a position.
fn predicted_folds(core: &EditorCore, started: Instant, start_frame: f32) -> u32 {
    let durations: Vec<f32> = core
        .project
        .frames
        .iter()
        .map(|frame| frame.duration)
        .collect();
    walk_wraps(
        start_frame,
        started.elapsed().as_secs_f32(),
        playback_speed(),
        core.project.fps,
        &durations,
        core.loop_range,
    )
}

/// The drift sync's prediction for one recorded clock:
/// [`expected_frame_walk`] over the core's real frame
/// durations, its loop range and the noted playback speed,
/// for the time since the clock's start. The durations are
/// collected per call — the sync runs at most once per
/// frame-changed poll tick, and a project's frame count
/// makes the copy trivial next to the process-liveness
/// check beside it.
fn predicted_frame(core: &EditorCore, started: Instant, start_frame: f32) -> f32 {
    let durations: Vec<f32> = core
        .project
        .frames
        .iter()
        .map(|frame| frame.duration)
        .collect();
    expected_frame_walk(
        start_frame,
        started.elapsed().as_secs_f32(),
        playback_speed(),
        core.project.fps,
        &durations,
        core.loop_range,
    )
}

/// The drift restart decision, pure: restart only while
/// playing, only while sound is actually running, and only
/// when a playback clock exists to predict from and its
/// prediction has been exceeded. `expected` is `None` when
/// no start has recorded a clock (or a stop cleared it).
fn drift_restart_needed(
    playing: bool,
    audio_active: bool,
    expected: Option<f32>,
    actual: f32,
) -> bool {
    playing
        && audio_active
        && expected.is_some_and(|expected| drift_exceeded(expected, actual))
}

/// The full-loop restart decision, pure: restart only
/// while playing, only while sound is actually running,
/// only when a playback clock exists to count folds from
/// (`folds` is `None` when no start has recorded one), and
/// only when the playhead did not *decrease* across the
/// tick. The last guard is what keeps this sync out of the
/// wrap sync's shape: a decrease is the wrap signal
/// [`EditorCore::sync_playback_audio_on_wrap`] reads, and
/// if it already restarted for this tick a second restart
/// here would be pure waste. The missed shape is precisely
/// the complement — the playhead landed on its previous
/// frame (`cur_frame == prev_frame`) or just past it after
/// advancing a full loop or more inside one tick — so
/// equality must pass, not fail.
fn full_loop_detected(
    playing: bool,
    audio_active: bool,
    prev_frame: usize,
    cur_frame: usize,
    folds: Option<u32>,
) -> bool {
    playing
        && audio_active
        && cur_frame >= prev_frame
        && folds.is_some_and(|folds| folds >= 1)
}

impl EditorCore {
    /// Start hearing the project: every audio clip audible at
    /// the current frame is played together through
    /// [`AudioPlayer::play_project`] — a lone unity-gain clip
    /// plays directly, anything else is rendered through
    /// [`crate::audio_edit::mix`] first (the MP4 export's mix).
    ///
    /// Any sound a previous start left running is replaced
    /// (`play_project` stops first). When nothing is audible at
    /// the playhead — including a project with no clips, or
    /// clips whose files are missing — this succeeds silently
    /// and [`EditorCore::playback_audio_active`] stays false.
    /// A failure to launch the player is reported on
    /// `core.status`; success writes nothing there.
    pub fn start_playback_audio(&mut self) {
        let fps = self.project.fps.max(1.0);
        let result = PLAYER.with(|player| {
            player
                .borrow_mut()
                .play_project(&self.project.audio, fps, playhead_secs(self))
        });
        if let Err(error) = result {
            self.status = format!("Audio playback failed: {error}.");
        } else {
            // Re-base the playback clock. The sound starts at
            // the integer `current` frame (`playhead_secs`'
            // convention), so that — not the fractional
            // playhead, which may sit part-way through the
            // frame — is the frame the clock counts from.
            // This is the only start path: the wrap, seek and
            // drift syncs all restart through stop + start,
            // so every pass re-bases here. A start where
            // nothing is audible also records a clock, but it
            // can do no harm: the drift sync checks
            // `playback_audio_active` before it ever reads
            // the clock, and the next start overwrites it.
            CLOCK.with(|clock| {
                *clock.borrow_mut() = Some((Instant::now(), self.current as f32));
            });
        }
    }

    /// Stop any sound a previous
    /// [`EditorCore::start_playback_audio`] started. Safe to
    /// call when nothing is playing. Also clears the playback
    /// clock: with no pass running there is nothing for the
    /// drift sync to predict from.
    pub fn stop_playback_audio(&mut self) {
        PLAYER.with(|player| player.borrow_mut().stop());
        CLOCK.with(|clock| *clock.borrow_mut() = None);
    }

    /// Note the shell's current playback speed multiplier
    /// (Phase K): 1.0 is normal rate, 2.0 double, 0.5 half.
    /// The shell calls this on every playing tick with the
    /// same speed it scales `advance_play`'s `dt` by, and
    /// the drift sync's prediction
    /// ([`expected_frame_walk`], via `predicted_frame`)
    /// spends the clock's elapsed time at the noted rate —
    /// without it, a pass at any speed but 1.0 would read
    /// as permanent drift and restart on every tolerance.
    ///
    /// The value is stored sanitised (see
    /// `sanitise_playback_speed`): a finite positive speed
    /// is clamped into
    /// [`PLAYBACK_SPEED_MIN`]..=[`PLAYBACK_SPEED_MAX`];
    /// zero, negative, NaN or infinite input falls back to
    /// 1.0. The note survives stops and starts — it is the
    /// shell's setting, not one pass's state — and until
    /// the first note the prediction runs at 1.0.
    pub fn note_playback_speed(&self, speed: f32) {
        SPEED.with(|stored| *stored.borrow_mut() = sanitise_playback_speed(speed));
    }

    /// Re-sync the audio after a loop wrap: if the playhead
    /// wrapped between the shell's previous poll tick
    /// (`prev_frame`) and this one (`cur_frame`) while playing
    /// — the decision lives in `playhead_wrapped` — and sound
    /// is actually running, restart it from the top of the
    /// wrapped-to position via stop + start.
    ///
    /// The restart reuses [`EditorCore::start_playback_audio`],
    /// so it inherits that method's behaviour exactly: it
    /// starts at the current frame (frame 0 after a full loop,
    /// the range start after a loop-range wrap), succeeds
    /// silently when nothing is audible there, and reports a
    /// player launch failure on `core.status`.
    ///
    /// When there was no wrap, or no audio is active (nothing
    /// audible, or the player already exited), this is a
    /// no-op and writes nothing to the status line. The shell
    /// calls it every poll tick with the frame it saw last
    /// tick and `self.current`; the per-tick cost when nothing
    /// wraps is the comparison plus one process-liveness
    /// check.
    ///
    /// Both of the helper's playing flags are this core's
    /// current `playing`: the call happens inside the same
    /// tick that advanced the playhead, so the state that
    /// produced the motion and the state now are one. The
    /// two-flag guard still earns its keep at this level —
    /// on a stopped tick `playing` is false and nothing is
    /// considered, and a seek made while stopped is followed
    /// by Play's own fresh start, so a redundant restart on
    /// the first playing tick after one would start from the
    /// same current frame anyway.
    pub fn sync_playback_audio_on_wrap(&mut self, prev_frame: usize, cur_frame: usize) {
        let wrapped = playhead_wrapped(
            self.playing,
            self.playing,
            prev_frame,
            cur_frame,
            self.project.frames.len(),
        );
        if wrapped && self.playback_audio_active() {
            self.stop_playback_audio();
            self.start_playback_audio();
        }
    }

    /// Re-sync the audio after a forward seek: if the playhead
    /// jumped forward between the shell's previous poll tick
    /// (`prev_frame`) and this one (`cur_frame`) while playing
    /// — the decision lives in `playhead_seeked` — and sound
    /// is actually running, restart it from the new position
    /// via stop + start.
    ///
    /// The restart reuses [`EditorCore::start_playback_audio`],
    /// so it inherits that method's behaviour exactly: it
    /// starts at the current frame (the frame the seek landed
    /// on), succeeds silently when nothing is audible there,
    /// and reports a player launch failure on `core.status`.
    /// Backward seeks need no path of their own here: they
    /// lower `current`, which
    /// [`EditorCore::sync_playback_audio_on_wrap`] already
    /// reads as its wrap signal and restarts for.
    ///
    /// When there was no jump, playback is stopped, or no
    /// audio is active (nothing audible, or the player already
    /// exited), this is a no-op and writes nothing to the
    /// status line. The shell calls it every poll tick beside
    /// the wrap sync, with the frame it saw last tick and
    /// `self.current`; the per-tick cost when nothing jumps is
    /// the comparison plus one process-liveness check.
    ///
    /// The `playing` guard mirrors the wrap sync's: the call
    /// happens inside the same tick that advanced the
    /// playhead, so the state that produced the motion is the
    /// state now. A seek made while stopped is followed by
    /// Play's own fresh start, so no restart is owed for it.
    pub fn sync_playback_audio_on_seek(&mut self, prev_frame: usize, cur_frame: usize) {
        let seeked = self.playing && playhead_seeked(prev_frame, cur_frame);
        if seeked && self.playback_audio_active() {
            self.stop_playback_audio();
            self.start_playback_audio();
        }
    }

    /// Re-sync the audio after clock drift: if the fractional
    /// playhead (`core.playhead` — the shell's sub-frame
    /// position, of which `current` is the floor) has wandered
    /// further from the playback clock's prediction than
    /// [`DRIFT_TOLERANCE_FRAMES`] while playing and sound is
    /// actually running — the decision lives in
    /// `drift_restart_needed`, over `drift_exceeded` and the
    /// prediction from `predicted_frame` (the duration- and
    /// speed-aware [`expected_frame_walk`] over the recorded
    /// clock) — restart the mix at the playhead via stop +
    /// start.
    ///
    /// The restart reuses [`EditorCore::start_playback_audio`],
    /// so it inherits that method's behaviour exactly: it
    /// starts at the current frame, succeeds silently when
    /// nothing is audible there, reports a player launch
    /// failure on `core.status`, and re-bases the playback
    /// clock, so a pass that keeps drifting is corrected at
    /// most once per tolerance of accumulated error rather
    /// than on every tick.
    ///
    /// When the drift is within tolerance, playback is
    /// stopped, no audio is active, or no clock exists (no
    /// successful start since the last stop), this is a no-op
    /// and writes nothing to the status line. The shell calls
    /// it from poll beside the wrap and seek syncs, throttled
    /// to ticks where the frame changed (integrator wiring);
    /// the per-call cost is one clock read plus one
    /// process-liveness check.
    pub fn sync_playback_audio_on_drift(&mut self) {
        let clock = CLOCK.with(|clock| *clock.borrow());
        let expected = clock.map(|(started, start_frame)| predicted_frame(self, started, start_frame));
        if drift_restart_needed(self.playing, self.playback_audio_active(), expected, self.playhead)
        {
            self.stop_playback_audio();
            self.start_playback_audio();
        }
    }

    /// Re-sync the audio after a full loop completed inside
    /// a single poll tick (Phase L): the one wrap shape the
    /// frame comparison in
    /// [`EditorCore::sync_playback_audio_on_wrap`] cannot
    /// see. That sync reads a *decrease* in `current` as its
    /// signal, so a tick that advanced the playhead by a
    /// whole loop or more — a stall of a full loop's
    /// duration — and landed it on the frame it started
    /// from, or just past it, looks exactly like holding or
    /// ordinary forward motion, and the sound keeps playing
    /// its first pass while the picture starts its second.
    ///
    /// This sync detects the loop from the playback clock
    /// instead of the frame delta: [`walk_wraps`] counts
    /// how many times the clock's own walk — the drift
    /// sync's duration- and speed-aware prediction, folded
    /// at the same end `advance_play` folds at — has folded
    /// since the running pass started. One or more folds
    /// means a loop completed on the sound's own timeline;
    /// when the tick's frames also pass the
    /// `full_loop_detected` guard, the mix restarts at the
    /// current playhead via stop + start, exactly as the
    /// wrap sync restarts.
    ///
    /// `prev_frame` is the frame the shell saw before the
    /// tick's advance (the same value it hands the wrap
    /// sync); the current frame is read from the core. The
    /// guard's `current >= prev_frame` condition keeps the
    /// two syncs disjoint per tick: when the playhead
    /// decreased, the wrap was visible to the wrap sync and
    /// belongs to it, and this sync stands down even if the
    /// clock's walk has folded — so a tick never restarts
    /// twice. When the playhead landed ahead by more than a
    /// frame the seek sync may also fire for the same tick;
    /// both restarts start from the landed frame, so the
    /// outcome is the same single re-anchor.
    ///
    /// The restart reuses
    /// [`EditorCore::start_playback_audio`], so it inherits
    /// that method's behaviour exactly: it starts at the
    /// current frame, succeeds silently when nothing is
    /// audible there, reports a player launch failure on
    /// `core.status`, and re-bases the playback clock — so
    /// after a restart the fold count starts again from the
    /// new pass and this sync cannot fire twice for one
    /// loop.
    ///
    /// When playback is stopped, no audio is active, no
    /// clock exists, or no full loop has completed on the
    /// clock's walk, this is a no-op and writes nothing to
    /// the status line. The shell calls it from poll beside
    /// the other syncs (integrator wiring); the per-call
    /// cost is one clock read, the fold walk over the
    /// project's durations, and one process-liveness check.
    pub fn sync_playback_audio_on_full_loop(&mut self, prev_frame: usize) {
        let clock = CLOCK.with(|clock| *clock.borrow());
        let folds = clock.map(|(started, start_frame)| predicted_folds(self, started, start_frame));
        if full_loop_detected(
            self.playing,
            self.playback_audio_active(),
            prev_frame,
            self.current,
            folds,
        ) {
            self.stop_playback_audio();
            self.start_playback_audio();
        }
    }

    /// True while the player's `ffplay` process is still
    /// running. Note this is process liveness, not audibility:
    /// on a machine with no audio device the process may exit
    /// immediately after a successful start, and this returns
    /// false even though the start itself succeeded.
    pub fn playback_audio_active(&self) -> bool {
        PLAYER.with(|player| player.borrow_mut().is_playing())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::{ffplay_available, playback_plan, PlaybackPlan};
    use crate::audio_edit::{mix, read_wav, write_wav, WavData};
    use crate::model::AudioClip;

    /// Write a mono WAV of constant `amplitude`, `secs` long at
    /// 8 kHz, to a unique temp path and return the path.
    fn const_wav_path(tag: &str, amplitude: f32, secs: f32) -> String {
        let wav = WavData {
            sample_rate: 8_000,
            channels: 1,
            samples: vec![amplitude; (8_000.0 * secs) as usize],
        };
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!(
            "stykanimate-rs-audio-play-{tag}-{}-{unique}.wav",
            std::process::id()
        ));
        std::fs::write(&path, write_wav(&wav)).expect("write temp WAV fixture");
        path.to_string_lossy().into_owned()
    }

    fn clip(path: &str, start_frame: usize) -> AudioClip {
        AudioClip {
            path: path.to_string(),
            start_frame,
            gain: 1.0,
        }
    }

    #[test]
    fn playhead_secs_uses_current_frame_over_fps() {
        let mut core = EditorCore::new();
        core.project.fps = 24.0;
        core.current = 48;
        assert_eq!(playhead_secs(&core), 2.0);
        core.current = 0;
        assert_eq!(playhead_secs(&core), 0.0);
        // Non-positive rates floor at 1 fps rather than dividing
        // by zero (the mux's convention).
        core.project.fps = 0.0;
        core.current = 48;
        assert_eq!(playhead_secs(&core), 48.0);
    }

    #[test]
    fn start_with_no_clips_is_silent_and_inactive() {
        let mut core = EditorCore::new();
        core.project.audio.clear();
        core.status = "Sentinel.".into();
        core.start_playback_audio();
        // The success path writes nothing to the status line,
        // and with nothing audible no process is running.
        assert_eq!(core.status, "Sentinel.");
        assert!(!core.playback_audio_active());
        // Stop is safe and leaves the flag down.
        core.stop_playback_audio();
        assert!(!core.playback_audio_active());
    }

    #[test]
    fn start_with_missing_clip_files_is_silent_not_an_error() {
        // The plan probe cannot resolve a duration for a missing
        // file, so the clip is excluded and the plan is Silent —
        // the same degradation `play_project` documents.
        let mut core = EditorCore::new();
        core.project.audio = vec![clip("/nonexistent/stykanimate-nope.wav", 0)];
        core.status = "Sentinel.".into();
        core.start_playback_audio();
        assert_eq!(core.status, "Sentinel.");
        assert!(!core.playback_audio_active());
        core.stop_playback_audio();
        assert!(!core.playback_audio_active());
    }

    #[test]
    fn mix_at_a_playhead_contains_each_clip_at_its_offset() {
        // The exact composition `start_playback_audio` triggers
        // through `play_project`: `playback_plan` at the
        // playhead, then `audio_edit::mix` (the export's mixer).
        // Clip A (0.5) starts at frame 0; clip B (0.25) at frame
        // 48 — 2.0s in at 24 fps. Both files last 4s.
        let a = const_wav_path("mix-a", 0.5, 4.0);
        let b = const_wav_path("mix-b", 0.25, 4.0);
        let clips = vec![clip(&a, 0), clip(&b, 48)];
        let durations = |path: &str| read_wav(path).ok().map(|w| w.duration_secs());
        match playback_plan(&clips, 3.0, 24.0, &durations) {
            PlaybackPlan::Mixed {
                mix_clips,
                offset_secs,
            } => {
                assert_eq!(offset_secs, 3.0);
                let mixed = mix(&mix_clips).expect("mix renders");
                assert_eq!(mixed.sample_rate, 8_000);
                // Latest end wins: B ends at 2.0 + 4.0 = 6.0s.
                assert_eq!(mixed.samples.len(), 48_000);
                let at = |t: f32| mixed.samples[(t * 8_000.0) as usize + 4_000];
                // 1.0s: only A sounds.
                assert!((at(1.0) - 0.5).abs() < 1e-3, "at 1.0s: {}", at(1.0));
                // 3.0s (the playhead): A + B summed.
                assert!((at(3.0) - 0.75).abs() < 1e-3, "at 3.0s: {}", at(3.0));
                // 5.0s: A has ended, only B remains.
                assert!((at(5.0) - 0.25).abs() < 1e-3, "at 5.0s: {}", at(5.0));
            }
            other => panic!("expected a Mixed plan, got {other:?}"),
        }
        let _ = std::fs::remove_file(&a);
        let _ = std::fs::remove_file(&b);
    }

    #[test]
    fn start_and_stop_with_a_real_clip() {
        if !ffplay_available() {
            eprintln!("skipping real-clip playback test: ffplay is not available");
            return;
        }
        // Silence, so the test makes no sound on machines that
        // do have an audio device; 2s at unity gain takes the
        // player's Direct path.
        let path = const_wav_path("real", 0.0, 2.0);
        let mut core = EditorCore::new();
        core.project.audio = vec![clip(&path, 0)];
        core.current = 0;
        core.start_playback_audio();
        // ffplay may exit at once on a device-less machine, so
        // liveness here is not asserted — the portable facts are
        // that the start reported no failure and that stop
        // leaves the player inactive.
        assert!(
            !core.status.starts_with("Audio playback failed"),
            "unexpected status: {}",
            core.status
        );
        core.stop_playback_audio();
        assert!(!core.playback_audio_active());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn wrapped_detects_the_end_of_timeline_wrap() {
        // Last frame → frame 0, the `advance_play` no-range
        // fold (playhead crossing the frame count).
        assert!(playhead_wrapped(true, true, 9, 0, 10));
        assert!(playhead_wrapped(true, true, 1, 0, 2));
        // A loop-range fold lands on the range start, not
        // necessarily 0 — still a decrease, still a wrap.
        assert!(playhead_wrapped(true, true, 7, 3, 10));
    }

    #[test]
    fn wrapped_ignores_ordinary_forward_motion() {
        assert!(!playhead_wrapped(true, true, 3, 4, 10));
        // Holding on the same frame (a long-duration frame the
        // playhead has not crossed yet) is not a wrap.
        assert!(!playhead_wrapped(true, true, 4, 4, 10));
        assert!(!playhead_wrapped(true, true, 0, 0, 10));
        assert!(!playhead_wrapped(true, true, 0, 9, 10));
    }

    #[test]
    fn wrapped_requires_playing_on_both_ticks() {
        // Paused now: the playhead did not move this tick, so
        // any decrease is a seek made while stopped — Play
        // restarts the audio for those, not the wrap sync.
        assert!(!playhead_wrapped(true, false, 9, 0, 10));
        // Not playing last tick either (the pass just started;
        // start_playback_audio already fired for it).
        assert!(!playhead_wrapped(false, true, 9, 0, 10));
        assert!(!playhead_wrapped(false, false, 9, 0, 10));
    }

    #[test]
    fn wrapped_needs_at_least_two_frames() {
        // One frame: `current` is always 0, no decrease can
        // occur — and stale indices must not read as one.
        assert!(!playhead_wrapped(true, true, 0, 0, 1));
        assert!(!playhead_wrapped(true, true, 3, 0, 1));
        // Zero frames: advance_play returns before moving.
        assert!(!playhead_wrapped(true, true, 0, 0, 0));
        assert!(!playhead_wrapped(true, true, 3, 0, 0));
    }

    #[test]
    fn sync_without_audio_is_a_silent_no_op() {
        // No clips → the player is never active, so even a
        // textbook wrap shape must leave everything untouched.
        let mut core = EditorCore::new();
        core.project.audio.clear();
        core.playing = true;
        let last = core.frame_count() - 1;
        assert!(last > 0, "demo project should have several frames");
        core.current = 0;
        core.status = "Sentinel.".into();
        core.sync_playback_audio_on_wrap(last, core.current);
        assert_eq!(core.status, "Sentinel.");
        assert!(!core.playback_audio_active());
        // Ordinary advance and a paused wrap shape alike.
        core.sync_playback_audio_on_wrap(0, 1);
        core.playing = false;
        core.sync_playback_audio_on_wrap(last, 0);
        assert_eq!(core.status, "Sentinel.");
        assert!(!core.playback_audio_active());
    }

    #[test]
    fn seeked_detects_forward_jumps() {
        // Two or more frames ahead in one tick: a seek, a
        // scrub, or a stall long enough to skip frames.
        assert!(playhead_seeked(3, 5));
        assert!(playhead_seeked(0, 2));
        assert!(playhead_seeked(4, 10));
        assert!(playhead_seeked(0, 9));
    }

    #[test]
    fn seeked_ignores_advance_holds_and_backward_motion() {
        // Exactly one frame ahead is ordinary advance.
        assert!(!playhead_seeked(3, 4));
        assert!(!playhead_seeked(0, 1));
        // Holding on the same frame (a long-duration frame the
        // playhead has not crossed yet) is not a seek.
        assert!(!playhead_seeked(4, 4));
        assert!(!playhead_seeked(0, 0));
        // Backward motion is the wrap path's business: the
        // full-timeline fold and the loop-range fold alike.
        assert!(!playhead_seeked(9, 0));
        assert!(!playhead_seeked(7, 3));
    }

    #[test]
    fn drift_frames_is_the_signed_gap() {
        // Positive: the playhead is ahead of the sound.
        assert_eq!(drift_frames(10.0, 12.5), 2.5);
        // Negative: the playhead trails the sound.
        assert_eq!(drift_frames(10.0, 7.5), -2.5);
        assert_eq!(drift_frames(4.0, 4.0), 0.0);
        assert_eq!(drift_frames(0.0, 0.25), 0.25);
    }

    #[test]
    fn drift_exceeded_only_past_the_tolerance() {
        // Exactly at the tolerance is still within bounds, in
        // both directions (the comparison is strict).
        assert!(!drift_exceeded(10.0, 10.0 + DRIFT_TOLERANCE_FRAMES));
        assert!(!drift_exceeded(10.0, 10.0 - DRIFT_TOLERANCE_FRAMES));
        assert!(!drift_exceeded(10.0, 10.0));
        assert!(!drift_exceeded(10.0, 11.0));
        // A hair past it, in either direction, is exceeded.
        assert!(drift_exceeded(10.0, 10.0 + DRIFT_TOLERANCE_FRAMES + 0.01));
        assert!(drift_exceeded(10.0, 10.0 - DRIFT_TOLERANCE_FRAMES - 0.01));
        assert!(drift_exceeded(0.0, 100.0));
        assert!(drift_exceeded(100.0, 0.0));
    }

    #[test]
    fn expected_frame_counts_up_from_the_start() {
        // Two seconds at 24 fps from frame 10 lands on 58.
        assert_eq!(expected_frame(10.0, 2.0, 24.0), 58.0);
        assert_eq!(expected_frame(0.0, 0.0, 24.0), 0.0);
        assert_eq!(expected_frame(3.0, 0.5, 12.0), 9.0);
        // Non-positive rates floor at 1 fps, the same
        // defensive floor `playhead_secs` uses.
        assert_eq!(expected_frame(10.0, 2.0, 0.0), 12.0);
    }

    #[test]
    fn drift_restart_needs_playing_active_audio_a_clock_and_real_drift() {
        // The one shape that restarts: playing, sound running,
        // a clock predicting 10.0, the playhead far from it.
        assert!(drift_restart_needed(true, true, Some(10.0), 20.0));
        assert!(drift_restart_needed(true, true, Some(10.0), 0.0));
        // Within tolerance: no restart.
        assert!(!drift_restart_needed(true, true, Some(10.0), 12.5));
        assert!(!drift_restart_needed(true, true, Some(10.0), 10.0));
        // Paused: the playhead is not supposed to be moving,
        // and Play makes its own fresh start.
        assert!(!drift_restart_needed(false, true, Some(10.0), 20.0));
        // No sound running: nothing to re-sync, however large
        // the nominal gap (the clock may be a silent start's).
        assert!(!drift_restart_needed(true, false, Some(10.0), 20.0));
        // No clock: no start has recorded one (or a stop
        // cleared it), so there is nothing to predict from.
        assert!(!drift_restart_needed(true, true, None, 20.0));
    }

    #[test]
    fn start_records_the_clock_and_stop_clears_it() {
        let mut core = EditorCore::new();
        core.project.audio.clear();
        // A fresh core's thread has no clock.
        assert!(CLOCK.with(|clock| clock.borrow().is_none()));
        core.current = 3;
        core.start_playback_audio();
        // The clock is based at the integer frame the sound
        // starts from — `current`, not the fractional
        // playhead — even though nothing was audible.
        let recorded = CLOCK.with(|clock| *clock.borrow());
        let (started, start_frame) = recorded.expect("start records a clock");
        assert_eq!(start_frame, 3.0);
        assert!(started.elapsed().as_secs_f32() < 5.0);
        core.stop_playback_audio();
        assert!(CLOCK.with(|clock| clock.borrow().is_none()));
    }

    #[test]
    fn drift_sync_without_audio_is_a_silent_no_op() {
        // Plant a clock predicting far from the playhead:
        // started 10 s ago at frame 0 of a 24 fps project, so
        // the prediction is frame 240 while the picture sits
        // at frame 0 — drift far past tolerance. With no clips
        // the player is never active, so even this textbook
        // drift shape must leave everything untouched.
        let mut core = EditorCore::new();
        core.project.audio.clear();
        core.project.fps = 24.0;
        core.playing = true;
        core.playhead = 0.0;
        CLOCK.with(|clock| {
            *clock.borrow_mut() = Some((Instant::now() - std::time::Duration::from_secs(10), 0.0));
        });
        core.status = "Sentinel.".into();
        core.sync_playback_audio_on_drift();
        assert_eq!(core.status, "Sentinel.");
        assert!(!core.playback_audio_active());
        // A no-op does not consume the clock either.
        assert!(CLOCK.with(|clock| clock.borrow().is_some()));
        core.stop_playback_audio();
    }

    #[test]
    fn seek_sync_without_audio_is_a_silent_no_op() {
        // No clips → the player is never active, so even a
        // textbook seek shape must leave everything untouched.
        let mut core = EditorCore::new();
        core.project.audio.clear();
        core.playing = true;
        core.current = 3;
        core.status = "Sentinel.".into();
        core.sync_playback_audio_on_seek(0, core.current);
        assert_eq!(core.status, "Sentinel.");
        assert!(!core.playback_audio_active());
        // Ordinary advance and a paused seek shape alike.
        core.sync_playback_audio_on_seek(2, 3);
        core.playing = false;
        core.sync_playback_audio_on_seek(0, 3);
        assert_eq!(core.status, "Sentinel.");
        assert!(!core.playback_audio_active());
    }

    #[test]
    fn walk_reduces_to_the_uniform_formula_when_durations_are_all_one() {
        // The Phase K pin: with every duration 1.0 and speed
        // 1.0, the walk is the old `expected_frame` count-up
        // (fps 8 and these times are exact in binary, so the
        // equality is exact, not approximate). 20 frames, so
        // nothing here reaches the fold.
        let durations = [1.0; 20];
        assert_eq!(
            expected_frame_walk(2.0, 1.0, 1.0, 8.0, &durations, None),
            expected_frame(2.0, 1.0, 8.0)
        );
        assert_eq!(expected_frame_walk(2.0, 1.0, 1.0, 8.0, &durations, None), 10.0);
        // A fractional start spends the rest of its frame
        // first, then whole frames, then a part frame.
        assert_eq!(
            expected_frame_walk(2.5, 0.5, 1.0, 8.0, &durations, None),
            expected_frame(2.5, 0.5, 8.0)
        );
        assert_eq!(expected_frame_walk(2.5, 0.5, 1.0, 8.0, &durations, None), 6.5);
        // Zero elapsed is the start, at any speed.
        assert_eq!(expected_frame_walk(4.0, 0.0, 2.0, 8.0, &durations, None), 4.0);
    }

    #[test]
    fn walk_stays_within_tolerance_through_a_duration_four_frame() {
        // The Phase J false trip, closed: frame 1 lasts four
        // frame periods (0.5 s at fps 8; a normal frame is
        // 0.125 s). After 0.6875 s the true playhead — the
        // continuous walk — is half-way through frame 2,
        // while the old uniform prediction has raced ahead
        // to 5.5: a drift of 3.0 frames, past the tolerance,
        // with nothing actually wrong.
        let durations = [1.0, 4.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0];
        let walked = expected_frame_walk(0.0, 0.6875, 1.0, 8.0, &durations, None);
        assert!((walked - 2.5).abs() < 1e-4, "walked: {walked}");
        let true_playhead = 2.5;
        assert!(
            !drift_exceeded(walked, true_playhead),
            "the walk must stay within tolerance through the slow frame"
        );
        // …and the old formula really would have tripped on
        // the same pass — the contrast is the point.
        let uniform = expected_frame(0.0, 0.6875, 8.0);
        assert_eq!(uniform, 5.5);
        assert!(drift_exceeded(uniform, true_playhead));
    }

    #[test]
    fn walk_wraps_inside_a_loop_range() {
        // Range (2, 4) on 8 frames: the fold window ends at
        // 5 and spans 3, exactly `advance_play`'s. From the
        // range's last frame, 0.3125 s at fps 8 crosses it,
        // folds back to the range start, crosses frame 2 and
        // lands half-way through frame 3 — folded, where the
        // playhead itself would be, not counted up to 6.5.
        let durations = [1.0; 8];
        let walked = expected_frame_walk(4.0, 0.3125, 1.0, 8.0, &durations, Some((2, 4)));
        assert!((walked - 3.5).abs() < 1e-4, "walked: {walked}");
        assert!((2.0..5.0).contains(&walked));
        // A stale range is clamped as `advance_play` clamps
        // it: (2, 99) on 8 frames behaves as (2, 7) — the
        // window ends at 8, so this walk does not fold at all
        // and matches the plain timeline walk.
        assert_eq!(
            expected_frame_walk(4.0, 0.3125, 1.0, 8.0, &durations, Some((2, 99))),
            expected_frame_walk(4.0, 0.3125, 1.0, 8.0, &durations, None)
        );
    }

    #[test]
    fn walk_speed_scales_the_advance() {
        // Speed multiplies the timeline time the elapsed
        // wall time buys: at 2.0 the prediction advances
        // twice as far, at 0.5 half as far.
        let durations = [1.0; 32];
        let normal = expected_frame_walk(0.0, 1.0, 1.0, 8.0, &durations, None);
        let double = expected_frame_walk(0.0, 1.0, 2.0, 8.0, &durations, None);
        let half = expected_frame_walk(0.0, 1.0, 0.5, 8.0, &durations, None);
        assert_eq!(normal, 8.0);
        assert_eq!(double, 16.0);
        assert_eq!(double, 2.0 * normal);
        assert_eq!(half, 4.0);
    }

    #[test]
    fn walk_crossing_cap_bounds_an_absurd_elapsed() {
        // One frame, an hour of elapsed at 30 fps: over a
        // hundred thousand crossings unfolded. The walk caps
        // at 2 * 1 + 2 crossings and returns folded, inside
        // the single frame — promptly, by construction.
        let walked = expected_frame_walk(0.0, 3600.0, 1.0, 30.0, &[1.0], None);
        assert_eq!(walked, 0.0);
        // Three frames, a similar absurdity: the result is
        // still a folded position inside the timeline.
        let walked = expected_frame_walk(0.0, 1000.0, 1.0, 10.0, &[1.0, 1.0, 1.0], None);
        assert!((0.0..3.0).contains(&walked), "walked: {walked}");
    }

    #[test]
    fn walk_without_frames_or_elapsed_time_stays_at_the_start() {
        // No timeline: nothing to walk, the start comes back
        // (advance_play returns before moving, too).
        assert_eq!(expected_frame_walk(3.0, 5.0, 1.0, 12.0, &[], None), 3.0);
        // Time that did not pass — or negative, or NaN —
        // moves nothing.
        let durations = [1.0; 6];
        assert_eq!(expected_frame_walk(2.0, -1.0, 1.0, 12.0, &durations, None), 2.0);
        assert_eq!(expected_frame_walk(2.0, f32::NAN, 1.0, 12.0, &durations, None), 2.0);
    }

    #[test]
    fn noted_speed_is_sanitised_and_defaults_to_normal() {
        // A fresh test thread has never noted a speed.
        assert_eq!(playback_speed(), 1.0);
        let core = EditorCore::new();
        core.note_playback_speed(2.0);
        assert_eq!(playback_speed(), 2.0);
        // Finite and positive, but outside the sanity
        // bounds: clamped to them, not rejected.
        core.note_playback_speed(100.0);
        assert_eq!(playback_speed(), PLAYBACK_SPEED_MAX);
        core.note_playback_speed(0.01);
        assert_eq!(playback_speed(), PLAYBACK_SPEED_MIN);
        // Not a sane speed at all: normal rate, not a bound.
        core.note_playback_speed(0.0);
        assert_eq!(playback_speed(), 1.0);
        core.note_playback_speed(-3.0);
        assert_eq!(playback_speed(), 1.0);
        core.note_playback_speed(f32::NAN);
        assert_eq!(playback_speed(), 1.0);
        core.note_playback_speed(f32::INFINITY);
        assert_eq!(playback_speed(), 1.0);
    }

    #[test]
    fn drift_prediction_walks_the_projects_real_durations_and_speed() {
        // The sync's own prediction path, end to end except
        // the restart: a project at fps 8 whose second frame
        // lasts four periods. 687 ms after a start at frame
        // 0 the prediction sits at ≈2.5 (0.125 s for frame 0,
        // 0.5 s for the slow frame, the remainder half-way
        // through frame 2) — the tolerances here are for the
        // milliseconds of real time between planting the
        // clock and reading it.
        let mut core = EditorCore::new();
        core.project.fps = 8.0;
        for frame in &mut core.project.frames {
            frame.duration = 1.0;
        }
        assert!(core.project.frames.len() > 2, "demo project needs the slow frame");
        core.project.frames[1].duration = 4.0;
        core.loop_range = None;
        let started = Instant::now() - std::time::Duration::from_millis(687);
        let predicted = predicted_frame(&core, started, 0.0);
        assert!((predicted - 2.5).abs() < 0.05, "predicted: {predicted}");
        // Noting double speed halves the wall time to the
        // same prediction: 343 ms at 2.0 buys the same
        // timeline time as 687 ms at 1.0.
        core.note_playback_speed(2.0);
        let started = Instant::now() - std::time::Duration::from_millis(343);
        let predicted = predicted_frame(&core, started, 0.0);
        assert!((predicted - 2.5).abs() < 0.05, "predicted at 2x: {predicted}");
    }

    #[test]
    fn walk_wraps_counts_completed_loops_on_a_uniform_timeline() {
        // Five frames at fps 8: a frame is 0.125 s and a
        // full loop 0.625 s — all exact in binary, so the
        // counts (and the agreement checks against the
        // walk's own folded result) are exact.
        let durations = [1.0; 5];
        // Half a loop folds nothing and lands mid-timeline.
        assert_eq!(walk_wraps(0.0, 0.3125, 1.0, 8.0, &durations, None), 0);
        assert_eq!(expected_frame_walk(0.0, 0.3125, 1.0, 8.0, &durations, None), 2.5);
        // Exactly one loop: one fold, back at the start.
        assert_eq!(walk_wraps(0.0, 0.625, 1.0, 8.0, &durations, None), 1);
        assert_eq!(expected_frame_walk(0.0, 0.625, 1.0, 8.0, &durations, None), 0.0);
        // Two and a half loops: two folds, landing where
        // the half loop landed.
        assert_eq!(walk_wraps(0.0, 1.5625, 1.0, 8.0, &durations, None), 2);
        assert_eq!(expected_frame_walk(0.0, 1.5625, 1.0, 8.0, &durations, None), 2.5);
        // A start inside the loop folds as soon as its
        // remaining frame time carries it past the end.
        assert_eq!(walk_wraps(4.0, 0.125, 1.0, 8.0, &durations, None), 1);
        // No time passed: nothing completes.
        assert_eq!(walk_wraps(0.0, 0.0, 1.0, 8.0, &durations, None), 0);
    }

    #[test]
    fn walk_wraps_counts_folds_inside_a_loop_range() {
        // Range (2, 4) on 8 frames: the fold window is
        // [2, 5) with span 3, so one loop of the range is
        // 3 frames — 0.375 s at fps 8.
        let durations = [1.0; 8];
        let range = Some((2, 4));
        // The companion of `walk_wraps_inside_a_loop_range`:
        // from the range's last frame, 2.5 frames of travel
        // fold once and land half-way through frame 3.
        assert_eq!(walk_wraps(4.0, 0.3125, 1.0, 8.0, &durations, range), 1);
        assert_eq!(expected_frame_walk(4.0, 0.3125, 1.0, 8.0, &durations, range), 3.5);
        // Seven frames from the range start wrap the range
        // twice and land one frame in.
        assert_eq!(walk_wraps(2.0, 0.875, 1.0, 8.0, &durations, range), 2);
        assert_eq!(expected_frame_walk(2.0, 0.875, 1.0, 8.0, &durations, range), 3.0);
        // Two frames from the range start never reach the
        // fold end.
        assert_eq!(walk_wraps(2.0, 0.25, 1.0, 8.0, &durations, range), 0);
    }

    #[test]
    fn walk_wraps_counts_non_uniform_loops_and_saturates() {
        // Two frames of duration 2.0 at fps 8: each lasts
        // 0.25 s, so the loop is 0.5 s of real time however
        // the frames are counted.
        let durations = [2.0, 2.0];
        assert_eq!(walk_wraps(0.0, 0.25, 1.0, 8.0, &durations, None), 0);
        assert_eq!(walk_wraps(0.0, 0.5, 1.0, 8.0, &durations, None), 1);
        assert_eq!(walk_wraps(0.0, 1.0, 1.0, 8.0, &durations, None), 2);
        // Saturation: one frame and an hour of elapsed. The
        // crossing cap (2 * 1 + 2) stops the count at 4 —
        // bounded, and still ≥ 1, which is all the caller
        // asks of a gap that large.
        let folds = walk_wraps(0.0, 3600.0, 1.0, 30.0, &[1.0], None);
        assert_eq!(folds, 4);
        assert!(folds <= 2 * 1 + 2);
        // Nothing to walk, or no time to walk in: zero.
        assert_eq!(walk_wraps(3.0, 5.0, 1.0, 12.0, &[], None), 0);
        assert_eq!(walk_wraps(0.0, f32::NAN, 1.0, 8.0, &durations, None), 0);
    }

    #[test]
    fn full_loop_detected_needs_playing_audio_no_decrease_and_a_fold() {
        // The missed shape itself: the playhead landed on
        // the frame it started the tick from, with a full
        // loop completed on the clock's walk.
        assert!(full_loop_detected(true, true, 3, 3, Some(1)));
        // Landed just past the previous frame, two loops in.
        assert!(full_loop_detected(true, true, 3, 4, Some(2)));
        // No completed loop: ordinary motion, however the
        // frames moved.
        assert!(!full_loop_detected(true, true, 3, 3, Some(0)));
        assert!(!full_loop_detected(true, true, 3, 4, Some(0)));
        // A decrease belongs to the wrap sync — stand down
        // even when the walk folded, so a tick never
        // restarts twice.
        assert!(!full_loop_detected(true, true, 3, 2, Some(1)));
        assert!(!full_loop_detected(true, true, 3, 0, Some(3)));
        // Paused, silent, or clockless: nothing to re-sync.
        assert!(!full_loop_detected(false, true, 3, 3, Some(1)));
        assert!(!full_loop_detected(true, false, 3, 3, Some(1)));
        assert!(!full_loop_detected(true, true, 3, 3, None));
    }

    #[test]
    fn full_loop_sync_without_audio_is_a_silent_no_op() {
        // Plant a clock ten seconds old at fps 8 — many
        // full loops on its walk — in the textbook missed
        // shape (playhead on the frame the tick started
        // from). With no clips the player is never active,
        // so even this must leave everything untouched.
        let mut core = EditorCore::new();
        core.project.audio.clear();
        core.project.fps = 8.0;
        core.playing = true;
        core.current = 3;
        CLOCK.with(|clock| {
            *clock.borrow_mut() = Some((Instant::now() - std::time::Duration::from_secs(10), 0.0));
        });
        core.status = "Sentinel.".into();
        core.sync_playback_audio_on_full_loop(3);
        assert_eq!(core.status, "Sentinel.");
        assert!(!core.playback_audio_active());
        // A no-op does not consume the clock either.
        assert!(CLOCK.with(|clock| clock.borrow().is_some()));
        // Paused, and the decrease shape, are equally inert.
        core.playing = false;
        core.sync_playback_audio_on_full_loop(3);
        core.playing = true;
        core.current = 0;
        core.sync_playback_audio_on_full_loop(3);
        assert_eq!(core.status, "Sentinel.");
        assert!(!core.playback_audio_active());
        core.stop_playback_audio();
    }
}
