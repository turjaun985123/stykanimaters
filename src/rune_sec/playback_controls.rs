//! Playback extras for the Rune editor shell (Phase D,
//! section 29 — GPUI→Rune migration). Declared from `main.rs`
//! only (as `rune_sec::playback_controls`) — bin-crate code,
//! not part of the engine library.
//!
//! The basic transport (Prev / Play-Pause / Next, frame ops)
//! already lives in `chrome_view::transport_row`; this section
//! ADDS the playback controls the GPUI Playback tool group
//! carried, each wired to the same real `EditorCore` op the
//! GPUI button called (ids and semantics verified against
//! `editor.rs` and the GPUI app at commit `1ea3633`):
//!
//! * **Frame rate** — preset chips for the GPUI cycler's
//!   preset list `[6, 12, 24, 30, 60]` (old `app.rs` `fps-
//!   preset` handler), each applied through
//!   `EditorCore::adjust_fps` (`editor.rs:1681`, adds a delta,
//!   clamps 1..=60) as `adjust_fps(preset - current)` — the
//!   same delta route the cycler used, so the status line
//!   stays consistent. Plus the `fps-dn` / `fps-up` steppers
//!   (`adjust_fps(∓1.0)`, old `app.rs:4177` / `:4181`).
//! * **Tween play** — `EditorCore::toggle_tween_playback`
//!   (`editor.rs:1691`), flipping `Project.tween_playback`
//!   (`model.rs:694`); shown active from that field.
//! * **Loop range** — REMOVED in UI mission Wave 5
//!   (audit D3/T4, integrator-approved recorded key
//!   change). This panel used to mount `loop-start` /
//!   `loop-end` / `loop-clear`, duplicating the keys
//!   the timeline's loop row mounts, with "Loop start" /
//!   "Loop end" labels that SET a range end here while
//!   the same words on the jump buttons below MOVED to
//!   it. Loop-range editing now lives in one home —
//!   the timeline's loop row (`timeline_marks`), where
//!   the range is drawn; the Frames section keeps its
//!   own mounts (outside this wave). What remains here
//!   is playback's distinct business: the loop toggle,
//!   the speed cycle and the jumps to the range's ends.
//!   Playback still wraps inside the range in
//!   `advance_play` (`editor.rs:687`), and
//!   [`loop_range_text`] is kept as a helper for the
//!   integrator.
//! * **Restart / Play range** (Phase F) — `pb-restart`
//!   goes to frame 0 and plays; `pb-play-range` goes to the
//!   loop range's start and plays (frame 0 when no range
//!   is set — the Restart behaviour). Both compose
//!   `EditorCore::goto` (`editor.rs:681`, sets `current` +
//!   `playhead`) with the `playing` field (`editor.rs:38`)
//!   set true — the same field the chrome transport flips.
//!   Audio start is NOT called here: the shell's poll hooks
//!   playback start (Phase F audio glue); these buttons
//!   only set the state it observes.
//! * **Loop toggle** (Phase G) — `pb-loop` flips the
//!   shell-level `State::loop_playback` flag (integrator-
//!   owned, initialised `true` = the behaviour every earlier
//!   phase shipped). The core still has no loop field:
//!   `advance_play` (`editor.rs:710`) wraps unconditionally
//!   — at the timeline end, or inside `loop_range` when one
//!   is set — so "off" is enforced by the shell's poll,
//!   which consults `should_stop_at_end` before each advance
//!   and pauses instead of letting the playhead wrap. The
//!   button itself touches no core state and takes no undo
//!   snapshot.
//! * **Loop MODE + ping-pong** (Roadmap Wave 2, W2-40) —
//!   `pb-loop` is now a three-state cycle ([`LoopMode`]:
//!   Off → Loop → PingPong → Off) through
//!   [`cycle_loop_mode`]; key, icon and label shape are the
//!   Phase G toggle's. The mode's ping-pong half lives in
//!   this module's thread-local (see [`LoopMode`] for why
//!   no `State` field was added); poll advances through
//!   `advance_ping_pong` while it is on, the stop check
//!   treats the mode as looping, and the bounce flips the
//!   core's travel direction — poll stops the playback
//!   audio at the end bounce (sound does not play in
//!   reverse) and restarts it at the start bounce, so each
//!   forward pass carries its sound. The command palette's
//!   older "Toggle loop playback" still flips only the
//!   Phase G flag: with ping-pong on, the mode stays
//!   PingPong until the panel cycle resyncs it (a palette
//!   mode-cycle command is filed as a follow-up, not
//!   built here).
//! * **Transport icons** (Phase H) — the three transport
//!   buttons render through `icon_btn` with Font Awesome
//!   glyphs: `pb-restart` = `backward-step`, `pb-play-range`
//!   = `play`, `pb-loop` = `repeat`. Keys, labels (the loop
//!   label still carries the on/off state), active states
//!   and handlers are byte-identical to the text-button
//!   versions. The basic transport (Prev / Play-Pause /
//!   Next) is not in this file — it lives in
//!   `chrome_view::transport_row` and is converted there.
//! * **Keyframe jumps** (Phase I) — `pb-prev-key` /
//!   `pb-next-key` step the playhead to the nearest key
//!   frame strictly before / after the current frame. The
//!   key frames are the sorted, de-duplicated union of the
//!   camera keys (`Project::camera`, `model.rs:690`;
//!   `CameraKey.frame`, `model.rs:542`, already a `usize`)
//!   and every graph channel's keys (`Project::channels`,
//!   `model.rs:714`; `graph::Key.frame`, `graph.rs:70`, an
//!   `f32`) mapped through
//!   `timeline_marks::graph_key_frame` — the graph panel
//!   seek's own rounding (`f32::round`, halves away from
//!   zero), with out-of-timeline keys dropped, never
//!   clamped; camera keys past the timeline end drop the
//!   same way. Plain text buttons: no Font Awesome glyph
//!   honestly means "previous / next keyframe". Unlike
//!   Restart / Play range the jumps only `goto` — they
//!   never touch `playing`, so stepping while paused stays
//!   paused and stepping mid-playback keeps playing. At
//!   either end of the key list the press used to be a
//!   silent no-op; Wave 9 renders the button inert
//!   whenever no key exists on its side of the playhead
//!   (see the Wave 9 bullet below).
//! * **Frame duration** — the `dur-dn` / `dur-up` steppers
//!   call `EditorCore::adjust_frame_duration`
//!   (`editor.rs:1675`) with the GPUI factors 0.5 / 2.0
//!   (multiplies the current frame's `duration`, clamps
//!   0.25..=8.0).
//! * **Timecode** — a read-only `current / total` readout in
//!   `M:SS.t`, ported from the GPUI `ui/timecode.rs` chip.
//!   Since Roadmap Wave 1 (RM7 / F-6) both sides read on the
//!   project's shared duration-weighted clock
//!   (`Project::seconds_at_frame_time` / `total_seconds`):
//!   the current side is the timeline seconds at the current
//!   frame's start, every earlier frame contributing its
//!   real duration. The GPUI chip's frame index ÷ fps shape
//!   disagreed with the played and exported timeline
//!   whenever a frame was held, and is gone.
//! * **Playback speed** (Phase K) — `pb-speed` cycles the
//!   shell-level `State::playback_speed` through `SPEEDS`
//!   (0.25×–2×) via `cycle_playback_speed`. Like the loop
//!   toggle it is shell state, not core state: the button
//!   only rewrites the field — poll scales each tick's
//!   advance by it and reports it to the audio clock, so
//!   picture and sound speed up together. The label IS
//!   the current speed (`speed_label`), and the press is
//!   silent, exactly like `pb-loop`'s: no core call, no
//!   status line, no undo snapshot.
//! * **Loop jumps** (Phase L) — `pb-loop-start` /
//!   `pb-loop-end` go to the loop range's start / end
//!   frame (`loop_jump_targets` reads `EditorCore::
//!   loop_range`, `editor.rs:61`). Plain text buttons in
//!   the loop row, beside `pb-loop` (the speed cycle sat
//!   here too until Wave 9 moved it to the transport
//!   row), and rendered ONLY while a range is set — with
//!   no range there is nothing to jump to, so the
//!   buttons are absent rather than inert. Like the
//!   keyframe jumps they only
//!   `goto` (`editor.rs:701`) — `playing` is never
//!   touched, so jumping while paused stays paused and
//!   jumping mid-playback keeps playing. The targets are
//!   NOT clamped here: `set_loop_range` (`editor.rs:769`)
//!   orders and clamps when the range is set, but frames
//!   deleted afterwards can leave an end past the
//!   timeline, and `goto`'s own bounds check then refuses
//!   the press — the handler keeps that refusal as the
//!   backstop, and Wave 9 renders such a button inert
//!   (see `loop_jump_inert`), so the refusal is what the
//!   control announces in advance rather than what a
//!   press silently meets. Wave 5 relabelled the
//!   buttons "Go to start" / "Go to end" (keys
//!   unchanged): they jump TO the range's ends, and
//!   since the same wave removed this panel's old
//!   `loop-start` / `loop-end` buttons — which SET the
//!   ends under near-identical labels — nothing in the
//!   panel can be misread as setting the range any
//!   more.
//! * **Card adoption** (Phase N) — the panel root is the
//!   kit card (`design::panel_col`, key `playback-
//!   controls` unchanged) with a `design::panel_title`
//!   header and a hairline divider; the private
//!   `section()` helper is retired onto
//!   `design::section_label` (section texts byte-
//!   identical, several of them dynamic). The timecode
//!   readout's font goes TITLE → MONO_CAPTION with the
//!   Mono family applied (the charter §12 numeric
//!   treatment, mirroring the status strip's
//!   `numeric_font`). The loop toggle, speed cycler,
//!   tween toggle and fps steppers stay on the button
//!   family by ruling — state buttons with labels, not
//!   kit switch-rows.
//! * **Honest inert states + transport grouping** (UI
//!   mission Wave 9) — the panel applies the Wave 3
//!   undo/redo rule (`btn_inert` / the live form, key and
//!   label unchanged) to every control whose press can
//!   provably no-op from the current state with no
//!   visual difference: the keyframe jumps when no key
//!   exists on their side of the playhead (a project
//!   with no keys left both permanently dead yet
//!   live-looking), a loop jump whose stored target has
//!   gone stale past the timeline, the fps steppers at
//!   the 1/60 clamp bounds, and the duration steppers at
//!   the 0.25/8.0 bounds or with no current frame. The
//!   inert rules ([`fps_stepper_inert`],
//!   [`duration_stepper_inert`], [`loop_jump_inert`])
//!   mirror the core ops' own clamp/refusal arithmetic,
//!   so a button is inert exactly when its press cannot
//!   change anything. Grouping: the speed cycle moved
//!   from the Loop row into the Transport row, so the
//!   panel reads play / step / speed under "Transport"
//!   and the Loop row carries only loop business (the
//!   toggle, its jumps and its state caption). Keys,
//!   handlers, labels and active rules are unchanged
//!   throughout — `pb-speed` still calls
//!   `cycle_playback_speed`, the jumps still `goto`.
//!
//! `State` fields: everything reads `s.core` live
//! (`project.fps` `model.rs:683`, `loop_range`, `current`,
//! `project.frames[..].duration`) except the loop cycle,
//! which reads/writes the shell-level `State::loop_playback`
//! (integrator-owned, initialised `true`) plus this
//! module's ping-pong thread-local (W2-40, see
//! [`LoopMode`]), and the speed cycle, which reads/writes
//! the shell-level `State::playback_speed`
//! (integrator-owned, initialised `1.0`).

use rune::tok::{self, type_, Theme};
use rune::{row, text, El};

use crate::chrome_view::{btn, btn_inert};
use crate::rune_sec::design;
use crate::rune_sec::i18n_wire;
use crate::rune_sec::icon_btn::icon_btn;
use crate::rune_sec::timeline_marks::graph_key_frame;
use crate::State;
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::Project;

/// The GPUI fps cycler's preset frame rates, ascending
/// (old `app.rs`, `fps-preset` handler: `[6, 12, 24, 30,
/// 60]`). Every value sits inside `adjust_fps`'s 1..=60
/// clamp, so a chip press always lands exactly on its preset.
pub const FPS_PRESETS: [f32; 5] = [6.0, 12.0, 24.0, 30.0, 60.0];

/// Element key for a preset chip (stable, section-prefixed;
/// the presets are whole numbers, so the key carries the
/// integer rate).
fn preset_key(preset: f32) -> String {
    format!("fps-preset-{}", preset as i64)
}

/// Chip label for a preset (`"12"` — the row's heading and
/// the readout already say "fps").
fn preset_label(preset: f32) -> String {
    format!("{}", preset as i64)
}

/// Whether `fps` is (within float tolerance) exactly the
/// given preset — drives the chip's active fill. Tolerance
/// covers fps values reached by repeated ±1 steps, which are
/// exact in practice; anything off-preset (e.g. 13) lights no
/// chip.
pub fn is_active_preset(fps: f32, preset: f32) -> bool {
    (fps - preset).abs() < 0.01
}

/// The frame-rate bounds `adjust_fps` clamps to
/// (`editor.rs`, `adjust_fps`: `(fps + delta).clamp(1.0,
/// 60.0)`), named here so the stepper-inert rule below
/// and the core can be checked against each other.
pub const FPS_FLOOR: f32 = 1.0;
pub const FPS_CEILING: f32 = 60.0;

/// Which fps steppers a press cannot move from `fps`,
/// as `(down_inert, up_inert)` (Wave 9). The rule mirrors
/// `adjust_fps`'s own arithmetic exactly — a stepper is
/// inert iff its clamped result equals the current rate —
/// so it stays truthful for any stored rate, including
/// one outside the clamp range in a loaded project
/// (where either press would slam the rate to a bound,
/// i.e. change it, and both steppers stay live). At the
/// bounds inside the range the press provably changes
/// nothing, so the button renders through `btn_inert`
/// instead of looking live — the Wave 3 undo/redo rule.
pub fn fps_stepper_inert(fps: f32) -> (bool, bool) {
    (
        (fps - 1.0).clamp(FPS_FLOOR, FPS_CEILING) == fps,
        (fps + 1.0).clamp(FPS_FLOOR, FPS_CEILING) == fps,
    )
}

/// The current rate as the chrome status line writes it:
/// whole rates as an integer, fractional rates with one
/// decimal (`"12 fps"`, `"29.9 fps"` — same convention as
/// `chrome_view::status_text`).
pub fn fps_text(fps: f32) -> String {
    if fps.fract() == 0.0 {
        format!("{} fps", fps as i64)
    } else {
        format!("{fps:.1} fps")
    }
}

/// How playback behaves at the end of its window
/// (Roadmap Wave 2, W2-40): the loop behaviour is a
/// three-state MODE, not a lone boolean — stop at the end,
/// wrap to the start (the behaviour every earlier phase
/// shipped), or ping-pong (reverse at each end and bounce,
/// continuously). The mode is derived from two pieces of
/// session state, following the precedents those pieces
/// already have: the wrap half is the shell-level
/// `State::loop_playback` flag (Phase G, initialised
/// `true`), and the ping-pong half is this module's
/// [`PING_PONG`] thread-local (the RM7 `PLAYBACK_DEADLINE`
/// pattern — session state with no new `State` field, so
/// the shell's State census and its literal sites in other
/// sections' fixtures are untouched). Neither half is
/// persisted, exactly like the flag and the playback speed
/// it sits beside. Ping-pong wins when both halves are
/// set: it is the later, more specific choice — and the
/// cycle below never rests in that combined state anyway.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LoopMode {
    /// Stop on the window's last frame (the shell poll's
    /// `should_stop_at_end` enforcement).
    Off,
    /// Wrap to the window's start and keep playing
    /// (`advance_play`'s own fold).
    Loop,
    /// Bounce: reverse at each end of the window and keep
    /// playing (`advance_ping_pong`'s reflection).
    PingPong,
}

impl LoopMode {
    /// The mode a session with these two flags is in.
    pub fn from_flags(loop_playback: bool, ping_pong: bool) -> LoopMode {
        if ping_pong {
            LoopMode::PingPong
        } else if loop_playback {
            LoopMode::Loop
        } else {
            LoopMode::Off
        }
    }

    /// The mode one press of the loop control moves to:
    /// Off → Loop → PingPong → Off. From the combined
    /// state (both flags set — reachable only by flipping
    /// the palette's loop toggle while ping-pong is on)
    /// the cycle also lands on Off, so the control always
    /// resyncs the two flags to a named mode.
    pub fn next(self) -> LoopMode {
        match self {
            LoopMode::Off => LoopMode::Loop,
            LoopMode::Loop => LoopMode::PingPong,
            LoopMode::PingPong => LoopMode::Off,
        }
    }

    /// The two flags this mode is stored as.
    pub fn flags(self) -> (bool, bool) {
        match self {
            LoopMode::Off => (false, false),
            LoopMode::Loop => (true, false),
            LoopMode::PingPong => (true, true),
        }
    }

    /// Whether the mode keeps playing at the window's
    /// ends (either looping flavour) — the value the
    /// shell poll's stop check consults.
    pub fn loops(self) -> bool {
        self != LoopMode::Off
    }
}

thread_local! {
    /// The ping-pong half of the loop mode (Roadmap Wave
    /// 2, W2-40) — see [`LoopMode`]. A `thread_local` for
    /// the RM7 reason: it is pure session state with the
    /// shell's own lifetime, and a new `State` field would
    /// owe an exhaustive literal-site splice across other
    /// sections' fixtures for a value no fixture reads.
    /// Written by [`cycle_loop_mode`], read through
    /// [`ping_pong_enabled`].
    static PING_PONG: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether ping-pong is the session's active loop mode
/// half — the shell poll's advance switch and this
/// panel's mode derivation both read it here.
pub fn ping_pong_enabled() -> bool {
    PING_PONG.with(|c| c.get())
}

/// The session's current loop mode, from its two stored
/// halves (the `State` flag and [`PING_PONG`]).
pub fn loop_mode(s: &State) -> LoopMode {
    LoopMode::from_flags(s.loop_playback, ping_pong_enabled())
}

/// One press of the `pb-loop` control (Roadmap Wave 2,
/// W2-40): advance the loop MODE one step (Off → Loop →
/// PingPong → Off) and store it back into its two halves.
///
/// Silent by design, mirroring the Phase G toggle it
/// extends: the press rewrites session state only — no
/// core call, no status line, no undo snapshot — and the
/// button's own label shows the new mode on the next
/// render while poll does the rest. Entering ping-pong
/// also resets the core's travel direction to forward
/// (`EditorCore::play_reverse`), so a fresh bounce always
/// starts by playing toward the end rather than resuming
/// whatever direction a previous ping-pong session — or
/// a seek across one — left behind.
pub fn cycle_loop_mode(s: &mut State) {
    let next = loop_mode(s).next();
    let (loop_playback, ping_pong) = next.flags();
    s.loop_playback = loop_playback;
    PING_PONG.with(|c| c.set(ping_pong));
    if next == LoopMode::PingPong {
        s.core.play_reverse = false;
    }
}

/// Button label for the loop-mode cycle — the mode is in
/// the label itself ("Loop: off" / "Loop: on" /
/// "Loop: ping-pong"), so the control reads correctly at
/// a glance in any state, like the speed cycle's.
pub fn loop_mode_label(mode: LoopMode) -> &'static str {
    match mode {
        LoopMode::Off => "Loop: off",
        LoopMode::Loop => "Loop: on",
        LoopMode::PingPong => "Loop: ping-pong",
    }
}

/// The caption beside the loop-mode cycle, naming what
/// the mode does at the window's ends.
pub fn loop_mode_caption(mode: LoopMode) -> &'static str {
    match mode {
        LoopMode::Off => "Off — stops at the end",
        LoopMode::Loop => "On — wraps at the end",
        LoopMode::PingPong => "Ping-pong — bounces at the ends",
    }
}

/// Readout text for a loop range: the inclusive range in
/// 1-based frame numbers, or the whole-timeline note when no
/// range is set. Kept for the integrator: this panel's own
/// loop-range row (its only caller) was removed in Wave 5
/// when loop-range editing consolidated into the
/// timeline's loop row.
pub fn loop_range_text(range: Option<(usize, usize)>) -> String {
    match range {
        Some((a, b)) => format!("Looping frames {}–{}", a + 1, b + 1),
        None => "No loop range — whole timeline loops".to_string(),
    }
}

/// The frame **Play range** starts from: the loop range's
/// start when a range is set, else frame 0 — which is
/// exactly Restart's behaviour, so Play range degrades to
/// Restart on a timeline with no range.
pub fn play_range_start(loop_range: Option<(usize, usize)>) -> usize {
    loop_range.map(|(a, _)| a).unwrap_or(0)
}

/// The frames the **Go to start** / **Go to end** jump
/// buttons go to: the loop range's ends exactly as stored,
/// or `None` when no range is set (the buttons are then
/// not rendered at all — see the module docs).
///
/// Deliberately NOT clamped to the timeline. A range is
/// ordered and clamped when it is set (`set_loop_range`,
/// `editor.rs:769`), but frames deleted afterwards can
/// leave its end past the last frame; the handlers hand
/// the stored frame straight to `EditorCore::goto`
/// (`editor.rs:701`), whose own bounds check refuses it,
/// leaving `current` and `playhead` where they were.
/// That refusal is the backstop: since Wave 9 the
/// builder renders a button whose target is stale
/// inert ([`loop_jump_inert`]), so the refusal is
/// announced by the control instead of met silently.
pub fn loop_jump_targets(core: &EditorCore) -> Option<(usize, usize)> {
    core.loop_range
}

/// Which loop jumps a press cannot move, as
/// `(start_inert, end_inert)` (Wave 9): a jump is inert
/// iff its stored target sits at or past the timeline's
/// frame count — exactly the targets `EditorCore::goto`
/// refuses (`frame < frames.len()`), so every press on
/// such a button would no-op with no visual difference
/// (proven by `loop_end_jump_to_a_stale_end_refuses_
/// silently` below). With no range the buttons are not
/// rendered at all; the `(true, true)` for `None` is
/// never consulted by the builder, it only keeps this
/// helper total.
pub fn loop_jump_inert(range: Option<(usize, usize)>, frame_count: usize) -> (bool, bool) {
    match range {
        None => (true, true),
        Some((a, b)) => (a >= frame_count, b >= frame_count),
    }
}

/// Every key frame in the project, sorted ascending with
/// duplicates removed: the union of the camera keys and
/// all graph channel keys.
///
/// Camera keys carry their frame directly (`CameraKey::
/// frame: usize`); a key stored past the timeline end (the
/// timeline shrank after it was set) is dropped, matching
/// `timeline_marks::markers_for_project`. Graph keys carry
/// a fractional time (`Key::frame: f32`) and map through
/// [`graph_key_frame`] — the graph seek's rounding, with
/// out-of-timeline results dropped rather than clamped —
/// so the jumps land exactly where the timeline's key
/// markers are drawn.
pub fn key_frames(core: &EditorCore) -> Vec<usize> {
    let count = core.frame_count();
    let mut frames: Vec<usize> = Vec::new();
    for key in &core.project.camera {
        if key.frame < count {
            frames.push(key.frame);
        }
    }
    for channel in &core.project.channels {
        for key in &channel.keys {
            if let Some(f) = graph_key_frame(key.frame, count) {
                frames.push(f);
            }
        }
    }
    frames.sort_unstable();
    frames.dedup();
    frames
}

/// The key frame a **Prev key** press goes to from `cur`:
/// the largest key strictly before it, or `None` when
/// `cur` sits at or before the first key (the press is
/// then a no-op). Standing exactly on a key steps to the
/// PREVIOUS key — the strict inequality is what makes
/// repeated presses walk the key list.
pub fn prev_key(cur: usize, keys: &[usize]) -> Option<usize> {
    keys.iter().copied().filter(|k| *k < cur).max()
}

/// The key frame a **Next key** press goes to from `cur`:
/// the smallest key strictly after it, or `None` when
/// `cur` sits at or past the last key. Standing exactly
/// on a key steps to the NEXT key, mirroring [`prev_key`].
pub fn next_key(cur: usize, keys: &[usize]) -> Option<usize> {
    keys.iter().copied().filter(|k| *k > cur).min()
}

/// Button label for the loop toggle — the state is in the
/// label itself ("Loop: on" / "Loop: off"), so the control
/// reads correctly at a glance in either state. Kept for
/// the palette's flag and the pre-W2-40 vocabulary:
/// [`loop_mode_label`] is the cycle's label now, and its
/// Loop/Off arms return these exact strings.
pub fn loop_toggle_label(loop_playback: bool) -> &'static str {
    if loop_playback {
        "Loop: on"
    } else {
        "Loop: off"
    }
}

/// The playback-speed cycle's steps, ascending (Phase K):
/// quarter speed to double speed around the `1.0` normal.
/// The shell's `State::playback_speed` is initialised to
/// the middle entry; nothing clamps the field itself —
/// `next_speed` maps ANY value (including off-table ones
/// a future control might write) onto this table.
pub const SPEEDS: [f32; 5] = [0.25, 0.5, 1.0, 1.5, 2.0];

/// The speed one `pb-speed` press moves to from
/// `current`: the next `SPEEDS` entry strictly greater
/// than `current`, wrapping to the first entry when none
/// is greater. A `current` not in the table goes to the
/// next higher entry (or wraps, when it sits above the
/// maximum). Total: `NaN` — which no comparison orders —
/// returns `1.0`, normal speed, rather than falling
/// through to the wrap.
pub fn next_speed(current: f32) -> f32 {
    if current.is_nan() {
        return 1.0;
    }
    for step in SPEEDS {
        if step > current {
            return step;
        }
    }
    SPEEDS[0]
}

/// Button label for the speed cycle — the value is in the
/// label itself ("Speed 0.25×" … "Speed 2×"), so the
/// control reads correctly at a glance at any step, like
/// the loop toggle's. Whole speeds render without a
/// fraction ("1×", "2×"); the rest keep exactly the
/// digits they need ("0.25×", "0.5×", "1.5×").
pub fn speed_label(speed: f32) -> String {
    let digits = format!("{speed:.2}");
    let trimmed = digits.trim_end_matches('0').trim_end_matches('.');
    format!("Speed {trimmed}×")
}

/// One `pb-speed` press (Phase K): advance
/// `State::playback_speed` to the next step of the cycle.
/// The signature is a cross-section contract — the View
/// menu's speed item (menus section) calls this exact
/// function.
///
/// Silent by design, mirroring `pb-loop`'s handler: that
/// toggle flips its shell flag without touching the core,
/// and this cycle does the same — no core call, no status
/// line, no undo snapshot. The button's own label already
/// shows the new speed on the next render, and poll does
/// the rest (it scales the advance and tells the audio
/// clock the new rate).
pub fn cycle_playback_speed(s: &mut State) {
    s.playback_speed = next_speed(s.playback_speed);
}

/// Should the shell's poll stop playback at the end instead
/// of letting `EditorCore::advance_play` wrap the playhead?
///
/// **Integrator contract (poll, `main.rs`).** Call this on
/// every poll tick at the point where poll currently calls
/// `advance_play` — after the `!core.playing` early return,
/// before the advance. When it returns `true`, set
/// `core.playing = false` and skip `advance_play` for that
/// tick entirely. The stop has to be decided from the
/// *pre-advance* state: `advance_play` wraps the fractional
/// playhead itself (`editor.rs:710`) and reports nothing,
/// so after an advance there is no way to tell a wrap
/// happened. The existing audio flip hook at the top of
/// poll observes the cleared `playing` flag on the next
/// tick and stops playback audio through the Phase F path,
/// like any other pause.
///
/// The check is frame-granular: it reads `current`, not the
/// fractional playhead, so the final frame is left on
/// screen and playback halts at the first tick that finds
/// it current — the last frame is not guaranteed its full
/// duration of display time.
///
/// The "end" is the end playback would actually wrap at.
/// `advance_play` loops inside `loop_range` when one is set
/// (the playhead wraps at the range's end and never reaches
/// the project end), so with a range the effective last
/// frame is the range's end — clamped to the timeline
/// exactly as `advance_play` clamps it (`editor.rs:719`) —
/// and without one it is `frame_count - 1`. `current` at
/// *or past* that frame stops: a playhead parked beyond a
/// range's end (e.g. after a `goto` outside the range)
/// stops too, rather than being yanked back into the range
/// by the wrap.
///
/// `false` whenever playback is paused, looping is on, or
/// the project has no frames.
pub fn should_stop_at_end(
    playing: bool,
    loop_playback: bool,
    loop_range: Option<(usize, usize)>,
    current: usize,
    frame_count: usize,
) -> bool {
    if !playing || loop_playback || frame_count == 0 {
        return false;
    }
    let last = match loop_range {
        Some((_, b)) => b.min(frame_count - 1),
        None => frame_count - 1,
    };
    current >= last
}

/// Go to `frame` and make sure playback is running — the
/// shared body of the Restart and Play range buttons.
/// `goto` also resets the fractional `playhead`, so a
/// restart mid-tween starts cleanly on the frame. Only the
/// `playing` field is set: audio start is the shell poll's
/// business (Phase F audio glue), not a button's.
fn start_playback_at(core: &mut EditorCore, frame: usize) {
    core.goto(frame);
    core.playing = true;
}

/// The duration bounds `adjust_frame_duration` clamps
/// to (`editor.rs`, `adjust_frame_duration`: `duration
/// * factor` clamped to `0.25..=8.0`), named here so the
/// stepper-inert rule below and the core can be checked
/// against each other.
pub const DURATION_FLOOR: f32 = 0.25;
pub const DURATION_CEILING: f32 = 8.0;

/// Which duration steppers a press cannot move, as
/// `(down_inert, up_inert)` (Wave 9). Like
/// [`fps_stepper_inert`] the rule mirrors the core op's
/// own arithmetic — a stepper is inert iff its clamped
/// result equals the current duration. `None` (no
/// current frame — an empty project) inerts both: there
/// is nothing to scale, and the op indexes
/// `frames[current]` unconditionally, so a live button
/// there would be worse than silent.
pub fn duration_stepper_inert(duration: Option<f32>) -> (bool, bool) {
    match duration {
        None => (true, true),
        Some(d) => (
            (d * 0.5).clamp(DURATION_FLOOR, DURATION_CEILING) == d,
            (d * 2.0).clamp(DURATION_FLOOR, DURATION_CEILING) == d,
        ),
    }
}

/// Readout for the current frame's duration multiplier, in
/// the same `{:.2}x` shape the core's own status message
/// uses (`adjust_frame_duration`, `editor.rs:1675`).
pub fn frame_duration_text(duration: f32) -> String {
    format!("{duration:.2}x")
}

/// Format a duration in seconds as `M:SS.t` — minutes (not
/// capped at 59), two-digit seconds, one decimal tenth.
/// Ported verbatim from the GPUI `ui/timecode.rs`: the value
/// is rounded once, to total tenths (half away from zero),
/// before being split, so 59.97 s formats as "1:00.0", never
/// "0:60.0" — and 3.25 s (exactly 32.5 tenths) rounds up to
/// "0:03.3". Negative and NaN inputs clamp to "0:00.0".
pub fn format_timecode(total_secs: f32) -> String {
    let clamped = if total_secs.is_nan() || total_secs < 0.0 {
        0.0
    } else {
        total_secs
    };
    let tenths = (clamped * 10.0).round() as u64;
    let minutes = tenths / 600;
    let seconds = (tenths % 600) / 10;
    let tenth = tenths % 10;
    format!("{minutes}:{seconds:02}.{tenth}")
}

/// The next absolute playback deadline (Roadmap Wave 1,
/// RM7 / F-6): playback wakes are anchored to a running
/// deadline instead of being re-scheduled as "one interval
/// from whenever this poll happened to finish", so the work
/// a poll does no longer accumulates as drift — the pre-RM7
/// shape made the real period `work + 1/fps`, never `1/fps`.
///
/// The returned deadline is the first one strictly after
/// `now` on the anchored grid: a poll that fires early (an
/// unrelated wake) keeps the standing deadline; a poll that
/// fires on time steps it by exactly one interval
/// (`1 / (fps × speed)`) from the PREVIOUS deadline, not
/// from `now`; a poll that fires late skips the missed
/// deadlines wholesale (drop-to-clock — the picture resyncs
/// to the clock rather than playing catch-up in slow
/// motion). A standing deadline more than one interval in
/// the future is stale — the rate or speed changed, or
/// playback restarted — and re-anchors to `now + interval`.
/// Rates floor at 1 fps and speeds at the audio side's
/// sanitised range, as everywhere else time is divided.
pub fn playback_deadline(previous: Option<f64>, now: f64, fps: f32, speed: f32) -> f64 {
    let fps = if fps.is_finite() { fps.max(1.0) } else { 1.0 };
    let speed = if speed.is_finite() && speed > 0.0 {
        speed.clamp(0.05, 8.0)
    } else {
        1.0
    };
    let interval = 1.0 / (fps as f64 * speed as f64);
    if !now.is_finite() {
        return interval;
    }
    let mut deadline = previous
        .filter(|d| d.is_finite())
        .unwrap_or(now + interval);
    if deadline <= now {
        let missed = ((now - deadline) / interval).floor() + 1.0;
        deadline += missed * interval;
    } else if deadline > now + interval {
        deadline = now + interval;
    }
    deadline
}

/// The readout's text: `"<current> / <total>"` on the
/// project's shared duration-weighted clock (Roadmap Wave 1,
/// RM7 / F-6): current is the timeline seconds at the start
/// of `current_frame` — every earlier frame contributing its
/// real duration — and total is `Project::total_seconds()`,
/// so the readout, the played timeline and the exported
/// timeline are the same number of seconds. The pre-RM7
/// label divided frame counts by fps, which disagreed with
/// all three whenever a frame was held. The project's rate
/// floors at 1 fps inside the clock, so a bad rate still
/// resolves instead of dividing by zero.
pub fn timecode_label(current_frame: usize, project: &Project) -> String {
    format!(
        "{} / {}",
        format_timecode(project.seconds_at_frame_time(current_frame as f32)),
        format_timecode(project.total_seconds())
    )
}

/// The numeric readout style (charter §12, mirroring the
/// status strip's `numeric_font`): `type_::MONO_CAPTION`
/// with the Mono family actually applied — the token
/// alone keeps the Sans family in the pinned Rune rev.
/// Used by the timecode readout only; every other label
/// in the panel stays on its existing font.
fn numeric_font() -> tok::TextStyle {
    type_::MONO_CAPTION.mono()
}

/// The playback extras panel body. Mounting (and any
/// show/hide state) is the caller's business; this builds the
/// full panel for the current core state. Element keys carry
/// the old GPUI control ids where one existed (`fps-dn`,
/// `fps-up`, `tween`, `dur-dn`, `dur-up` — the GPUI
/// `loop-start` / `loop-end` / `loop-clear` mounts were
/// removed in Wave 5; loop-range editing lives in the
/// timeline's loop row now); the Phase F transport buttons are
/// `pb-restart` / `pb-play-range`, the Phase G loop
/// toggle is `pb-loop` and the Phase I keyframe jumps are
/// `pb-prev-key` / `pb-next-key`; the Phase K speed cycle
/// in the transport row is `pb-speed`, and the Phase L
/// loop jumps beside it are `pb-loop-start` / `pb-loop-end`
/// (present only while a loop range is set), so headless
/// `--tree` checks can find every control.
pub fn playback_controls(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let fps = core.project.fps;
    let tween_on = core.project.tween_playback;
    let loop_range = core.loop_range;
    let playback_speed = s.playback_speed;
    let cur_duration = core
        .project
        .frames
        .get(core.current)
        .map(|f| f.duration);
    let frame_count = core.frame_count();
    // The keyframe jumps' targets from the current frame
    // (Wave 9): the handlers recompute them live, exactly
    // as before — computing them here too decides whether
    // each jump button can act at all from this frame, so
    // a jump with nowhere to go renders inert (the Wave 3
    // undo/redo rule) instead of failing silently.
    let jump_keys = key_frames(core);
    let prev_target = prev_key(core.current, &jump_keys);
    let next_target = next_key(core.current, &jump_keys);

    let mut panel = design::panel_col("playback-controls", t)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(design::panel_title(None, "Playback", None, t)),
        )
        .child(design::divider(t));

    // ---- Transport: the play actions (restart from the
    // top, or play from the loop range's start — frame 0
    // without a range), the keyframe steps, and the speed
    // cycle (Wave 9: moved here from the Loop row — speed
    // is a transport control, and the codebase already
    // calls this "the transport row's `pb-speed` button"
    // (menus section); the Loop header did not describe
    // it). A keyframe step with no key on its side of the
    // playhead renders inert: its press could only no-op,
    // with no visual difference (Wave 9, the Wave 3
    // undo/redo rule — key, label and the live handler
    // are unchanged in the live form).
    let prev_key_btn = if prev_target.is_some() {
        btn("pb-prev-key", "Prev key", false, t, |s| {
            let keys = key_frames(&s.core);
            if let Some(f) = prev_key(s.core.current, &keys) {
                s.core.goto(f);
            }
        })
    } else {
        btn_inert("pb-prev-key", "Prev key", t)
    };
    let next_key_btn = if next_target.is_some() {
        btn("pb-next-key", "Next key", false, t, |s| {
            let keys = key_frames(&s.core);
            if let Some(f) = next_key(s.core.current, &keys) {
                s.core.goto(f);
            }
        })
    } else {
        btn_inert("pb-next-key", "Next key", t)
    };
    panel = panel.child(design::section_label("Transport", t));
    panel = panel.child(
        row()
            .gap(6.0)
            .center_y()
            .flex_wrap()
            .child(icon_btn(
                "pb-restart",
                "backward-step",
                "Restart",
                false,
                t,
                |s| {
                    start_playback_at(&mut s.core, 0);
                },
            ))
            .child(icon_btn(
                "pb-play-range",
                "play",
                "Play range",
                loop_range.is_some(),
                t,
                |s| {
                    let start = play_range_start(s.core.loop_range);
                    start_playback_at(&mut s.core, start);
                },
            ))
            .child(prev_key_btn)
            .child(next_key_btn)
            .child(icon_btn(
                "pb-speed",
                "gauge",
                &speed_label(playback_speed),
                playback_speed != 1.0,
                t,
                cycle_playback_speed,
            )),
    );

    // ---- Loop playback: the loop MODE cycle (Roadmap
    // Wave 2, W2-40) — one press steps Off → Loop →
    // PingPong → Off through `cycle_loop_mode`, storing
    // into the Phase G flag and this module's ping-pong
    // thread-local. Key, icon and row are the Phase G
    // toggle's, unchanged; only the press's meaning grew.
    // Off = poll pauses at the end (see
    // should_stop_at_end); the button itself touches no
    // project state and takes no undo snapshot.
    panel = panel.child(design::section_label("Loop playback", t));
    let mut loop_row = row()
        .gap(6.0)
        .center_y()
        .flex_wrap()
        .child(icon_btn(
            "pb-loop",
            "repeat",
            loop_mode_label(loop_mode(s)),
            loop_mode(s) != LoopMode::Off,
            t,
            cycle_loop_mode,
        ));
    // The loop jumps exist only while a range is set:
    // absent when there is nothing to jump to. Wave 9:
    // a jump whose stored end has gone stale (at or past
    // the frame count after frame deletions) renders
    // inert rather than live — `goto` would refuse every
    // press on it (see `loop_jump_inert`), so a live
    // rendering would promise a jump that cannot happen.
    // Key, label and the live handler are unchanged in
    // the live form.
    if let Some(range) = loop_range {
        let (start_inert, end_inert) = loop_jump_inert(Some(range), frame_count);
        let start_btn = if start_inert {
            btn_inert("pb-loop-start", &i18n_wire::tr(core, "go_to_start"), t)
        } else {
            btn("pb-loop-start", &i18n_wire::tr(core, "go_to_start"), false, t, |s| {
                if let Some((a, _)) = loop_jump_targets(&s.core) {
                    s.core.goto(a);
                }
            })
        };
        let end_btn = if end_inert {
            btn_inert("pb-loop-end", &i18n_wire::tr(core, "go_to_end"), t)
        } else {
            btn("pb-loop-end", &i18n_wire::tr(core, "go_to_end"), false, t, |s| {
                if let Some((_, b)) = loop_jump_targets(&s.core) {
                    s.core.goto(b);
                }
            })
        };
        loop_row = loop_row.child(start_btn).child(end_btn);
    }
    loop_row = loop_row.child(design::section_label(
        loop_mode_caption(loop_mode(s)),
        t,
    ));
    panel = panel.child(loop_row);

    // ---- Frame rate: stepper + current-rate readout.
    panel = panel.child(design::section_label(
        &format!("Frame rate — now {}", fps_text(fps)),
        t,
    ));
    // Wave 9: a stepper at the rate's clamp bound renders
    // inert — its press would clamp back to the same rate
    // (see `fps_stepper_inert`); key, label and the live
    // handler are unchanged in the live form.
    let (fps_dn_inert, fps_up_inert) = fps_stepper_inert(fps);
    let fps_dn_btn = if fps_dn_inert {
        btn_inert("fps-dn", "FPS −", t)
    } else {
        btn("fps-dn", "FPS −", false, t, |s| {
            s.core.adjust_fps(-1.0);
        })
    };
    let fps_up_btn = if fps_up_inert {
        btn_inert("fps-up", "FPS +", t)
    } else {
        btn("fps-up", "FPS +", false, t, |s| {
            s.core.adjust_fps(1.0);
        })
    };
    let mut rate_row = row().gap(6.0).center_y();
    rate_row = rate_row.child(fps_dn_btn);
    rate_row = rate_row.child(fps_up_btn);
    panel = panel.child(rate_row);

    // ---- Frame rate: one chip per GPUI preset, applied as a
    // delta through adjust_fps (the cycler's route).
    let mut preset_row = row().gap(6.0).center_y();
    for p in FPS_PRESETS {
        preset_row = preset_row.child(btn(
            &preset_key(p),
            &preset_label(p),
            is_active_preset(fps, p),
            t,
            move |s: &mut State| {
                let cur = s.core.project.fps;
                s.core.adjust_fps(p - cur);
            },
        ));
    }
    panel = panel.child(preset_row);

    // ---- Tween playback toggle (project.tween_playback).
    panel = panel.child(design::section_label("Tween playback", t));
    panel = panel.child(
        row()
            .gap(6.0)
            .center_y()
            .child(btn("tween", &i18n_wire::tr(core, "tween_play"), tween_on, t, |s| {
                s.core.toggle_tween_playback();
            }))
            .child(design::section_label(
                if tween_on {
                    "On — in-between frames while playing"
                } else {
                    "Off — authored frames only"
                },
                t,
            )),
    );

    // ---- Loop range: REMOVED in UI mission Wave 5
    // (audit D3/T4, integrator-approved recorded key
    // change). This panel used to mount `loop-start` /
    // `loop-end` / `loop-clear` — the same keys the
    // timeline's loop row mounts — so one key named two
    // different actions in one tree ("Loop start" here
    // SET a range end to the current frame; "Loop start"
    // on the jumps above MOVES to it). Loop-range
    // editing now lives in one home, the timeline's loop
    // row; this panel keeps only its distinct actions —
    // the loop toggle, the speed cycle (in the Transport
    // row since Wave 9) and the jumps below, relabelled
    // "Go to start" / "Go to end" so
    // no label promises a set it does not perform.
    // [`loop_range_text`] stays as a helper for the
    // integrator.

    // ---- Current-frame duration stepper (0.5x / 2x, the
    // GPUI factors; the op clamps 0.25..=8.0). Wave 9: a
    // stepper at the duration's clamp bound — or with no
    // current frame at all — renders inert (see
    // `duration_stepper_inert`); key, label and the live
    // handler are unchanged in the live form.
    panel = panel.child(design::section_label(
        &match cur_duration {
            Some(d) => format!(
                "Frame {} duration — now {}",
                core.current + 1,
                frame_duration_text(d)
            ),
            None => "Frame duration".to_string(),
        },
        t,
    ));
    let (dur_dn_inert, dur_up_inert) = duration_stepper_inert(cur_duration);
    let dur_dn_btn = if dur_dn_inert {
        btn_inert("dur-dn", "Dur −", t)
    } else {
        btn("dur-dn", "Dur −", false, t, |s| {
            s.core.adjust_frame_duration(0.5);
        })
    };
    let dur_up_btn = if dur_up_inert {
        btn_inert("dur-up", "Dur +", t)
    } else {
        btn("dur-up", "Dur +", false, t, |s| {
            s.core.adjust_frame_duration(2.0);
        })
    };
    panel = panel.child(
        row()
            .gap(6.0)
            .center_y()
            .child(dur_dn_btn)
            .child(dur_up_btn),
    );

    // ---- Timecode readout (read-only).
    panel = panel.child(design::section_label("Time", t));
    panel = panel.child(
        text(timecode_label(core.current, &core.project))
            .key("timecode")
            .font(numeric_font())
            .color(t.text),
    );

    panel
}

#[cfg(test)]
mod tests {
    use super::*;
    use stykanimate_rs::editor::EditorCore;

    #[test]
    fn playback_i18n_keys_exist_and_match_the_english_labels() {
        // Wave V (V11, from U10's catalogue-hygiene
        // ledger): the loop-jump pair and the tween
        // toggle resolve through i18n_wire::tr under
        // pre-existing catalogue keys. The keys already
        // existed x28, so this pin passes on its own
        // merits — no IA was owed.
        let core = EditorCore::new();
        for (key, english) in [
            ("go_to_start", "Go to start"),
            ("go_to_end", "Go to end"),
            ("tween_play", "Tween play"),
        ] {
            assert!(i18n_wire::key_in_table(key), "missing i18n key: {key}");
            assert_eq!(i18n_wire::tr(&core, key), english);
        }
    }
    use stykanimate_rs::graph::{Channel, ChannelTarget, Key};

    /// A demo core (5 frames) with the demo's own camera
    /// keys and channels cleared, so each key test states
    /// its whole key set explicitly.
    fn bare_core() -> EditorCore {
        let mut core = EditorCore::new();
        core.project.camera.clear();
        core.project.channels.clear();
        core
    }

    /// Push one graph channel keyed at the given fractional
    /// frames (flat keys — only the frames matter here).
    fn add_channel(core: &mut EditorCore, frames: &[f32]) {
        core.project.channels.push(Channel {
            figure: 0,
            target: ChannelTarget::RootX,
            keys: frames.iter().map(|f| Key::flat(*f, 0.0)).collect(),
        });
    }

    #[test]
    fn presets_are_the_gpui_cycler_list_ascending() {
        assert_eq!(FPS_PRESETS, [6.0, 12.0, 24.0, 30.0, 60.0]);
        assert!(FPS_PRESETS.windows(2).all(|w| w[0] < w[1]));
        // Every preset is reachable: inside adjust_fps's clamp.
        assert!(FPS_PRESETS.iter().all(|p| (1.0..=60.0).contains(p)));
    }

    #[test]
    fn preset_keys_and_labels_carry_the_whole_rate() {
        assert_eq!(preset_key(6.0), "fps-preset-6");
        assert_eq!(preset_key(24.0), "fps-preset-24");
        assert_eq!(preset_key(60.0), "fps-preset-60");
        assert_eq!(preset_label(12.0), "12");
        assert_eq!(preset_label(30.0), "30");
    }

    #[test]
    fn active_preset_matches_exact_rates_only() {
        assert!(is_active_preset(24.0, 24.0));
        assert!(is_active_preset(12.0, 12.0));
        assert!(!is_active_preset(13.0, 12.0));
        assert!(!is_active_preset(13.0, 24.0));
        assert!(!is_active_preset(25.0, 24.0));
    }

    #[test]
    fn fps_text_writes_whole_rates_as_integers() {
        assert_eq!(fps_text(12.0), "12 fps");
        assert_eq!(fps_text(60.0), "60 fps");
        assert_eq!(fps_text(29.97), "30.0 fps");
    }

    #[test]
    fn loop_range_text_is_one_based_and_handles_none() {
        assert_eq!(loop_range_text(Some((1, 3))), "Looping frames 2–4");
        assert_eq!(loop_range_text(Some((0, 0))), "Looping frames 1–1");
        assert!(loop_range_text(None).starts_with("No loop range"));
    }

    #[test]
    fn frame_duration_text_uses_the_core_status_shape() {
        assert_eq!(frame_duration_text(1.0), "1.00x");
        assert_eq!(frame_duration_text(0.5), "0.50x");
        assert_eq!(frame_duration_text(2.25), "2.25x");
    }

    #[test]
    fn timecode_rounds_to_total_tenths_before_splitting() {
        assert_eq!(format_timecode(0.0), "0:00.0");
        // Just under a minute boundary carries cleanly.
        assert_eq!(format_timecode(59.97), "1:00.0");
        // Exactly 32.5 tenths rounds half away from zero.
        assert_eq!(format_timecode(3.25), "0:03.3");
        assert_eq!(format_timecode(61.0), "1:01.0");
        // Minutes are not capped at 59.
        assert_eq!(format_timecode(600.0), "10:00.0");
    }

    #[test]
    fn timecode_clamps_negative_and_nan_to_zero() {
        assert_eq!(format_timecode(-5.0), "0:00.0");
        assert_eq!(format_timecode(f32::NAN), "0:00.0");
    }

    #[test]
    fn timecode_label_is_duration_weighted() {
        // Roadmap Wave 1 (RM7 / F-6): at 24 fps with frame 0
        // held 4 beats and frame 1 held 4 beats, frame 1
        // starts at 4/24 s (reads 0:00.2) and the project
        // totals 11/24 s (reads 0:00.5) — the same seconds
        // `Project::total_seconds` reports and export renders.
        // The pre-RM7 label divided frame counts by fps and
        // read 0:00.0 / 0:00.2 for this project.
        let mut core = EditorCore::new();
        core.project.fps = 24.0;
        core.project.frames[0].duration = 4.0;
        core.project.frames[1].duration = 4.0;
        assert_eq!(timecode_label(0, &core.project), "0:00.0 / 0:00.5");
        assert_eq!(timecode_label(1, &core.project), "0:00.2 / 0:00.5");
        // Uniform durations reduce to the frame-count reading.
        let mut plain = EditorCore::new();
        plain.project.fps = 24.0;
        assert_eq!(timecode_label(0, &plain.project), "0:00.0 / 0:00.2");
        // An empty timeline reads zero over zero.
        plain.project.frames.clear();
        assert_eq!(timecode_label(0, &plain.project), "0:00.0 / 0:00.0");
    }

    #[test]
    fn timecode_label_falls_back_to_1fps_for_bad_rates() {
        // The shared clock floors the rate at 1 fps, so a
        // zero or negative rate still resolves: five 1-beat
        // frames total five seconds at the floor.
        let mut core = EditorCore::new();
        core.project.fps = 0.0;
        assert_eq!(timecode_label(2, &core.project), "0:02.0 / 0:05.0");
        core.project.fps = -12.0;
        assert_eq!(timecode_label(2, &core.project), "0:02.0 / 0:05.0");
    }

    #[test]
    fn playback_deadline_anchors_to_the_previous_deadline() {
        // Roadmap Wave 1 (RM7 / F-6): at 24 fps the interval
        // is 1/24 s, and each on-time poll steps the deadline
        // by exactly one interval from the PREVIOUS deadline
        // — the poll's own work time never enters the sum, so
        // the period is 1/fps, not work + 1/fps.
        let interval = 1.0 / 24.0;
        let first = playback_deadline(None, 2.0, 24.0, 1.0);
        assert!((first - (2.0 + interval)).abs() < 1e-9, "first: {first}");
        let second = playback_deadline(Some(first), first, 24.0, 1.0);
        assert!(
            (second - (2.0 + 2.0 * interval)).abs() < 1e-9,
            "second: {second}"
        );
        // An early poll (an unrelated wake) keeps the
        // standing deadline untouched.
        let early = playback_deadline(Some(first), 2.0 + 0.5 * interval, 24.0, 1.0);
        assert!((early - first).abs() < 1e-9, "early: {early}");
        // A late poll drops to the clock: the missed
        // deadlines are skipped, and the next deadline is
        // the first grid point strictly after `now`.
        let late_now = first + 2.3 * interval;
        let late = playback_deadline(Some(first), late_now, 24.0, 1.0);
        assert!(
            (late - (first + 3.0 * interval)).abs() < 1e-9,
            "late: {late}"
        );
        // Speed scales the interval: at 2x the deadline
        // steps half-intervals.
        let fast = playback_deadline(None, 2.0, 24.0, 2.0);
        assert!(
            (fast - (2.0 + interval / 2.0)).abs() < 1e-9,
            "fast: {fast}"
        );
    }

    #[test]
    fn play_range_start_is_the_range_start_or_zero() {
        assert_eq!(play_range_start(Some((2, 4))), 2);
        assert_eq!(play_range_start(Some((0, 3))), 0);
        // No range: Play range falls back to Restart's frame.
        assert_eq!(play_range_start(None), 0);
    }

    #[test]
    fn restart_goes_to_frame_zero_and_plays() {
        // The demo project has 5 frames; start mid-timeline,
        // paused, as a user pressing Restart would find it.
        let mut core = EditorCore::new();
        core.goto(3);
        core.playing = false;
        start_playback_at(&mut core, 0);
        assert_eq!(core.current, 0);
        // goto resets the fractional playhead too.
        assert_eq!(core.playhead, 0.0);
        assert!(core.playing, "Restart ensures playback runs");
    }

    #[test]
    fn play_range_starts_at_the_range_start_and_plays() {
        let mut core = EditorCore::new();
        core.set_loop_range(2, 4);
        core.goto(0);
        core.playing = false;
        // The pb-play-range handler's exact composition.
        let start = play_range_start(core.loop_range);
        start_playback_at(&mut core, start);
        assert_eq!(core.current, 2);
        assert!(core.playing);
    }

    #[test]
    fn start_playback_at_keeps_playing_when_already_playing() {
        // Pressing Restart mid-playback restarts, never pauses.
        let mut core = EditorCore::new();
        core.goto(2);
        core.playing = true;
        start_playback_at(&mut core, 0);
        assert!(core.playing);
        assert_eq!(core.current, 0);
    }

    #[test]
    fn loop_toggle_label_states_the_state() {
        assert_eq!(loop_toggle_label(true), "Loop: on");
        assert_eq!(loop_toggle_label(false), "Loop: off");
    }

    #[test]
    fn w2_40_loop_mode_derives_from_its_two_flags() {
        // Roadmap Wave 2 (W2-40): ping-pong wins over the
        // wrap flag (the later, more specific choice); the
        // wrap flag alone is the historical Loop.
        assert_eq!(LoopMode::from_flags(false, false), LoopMode::Off);
        assert_eq!(LoopMode::from_flags(true, false), LoopMode::Loop);
        assert_eq!(LoopMode::from_flags(true, true), LoopMode::PingPong);
        assert_eq!(LoopMode::from_flags(false, true), LoopMode::PingPong);
        // The cycle walks all three modes and every mode
        // stores as flags that derive straight back to it.
        let mut mode = LoopMode::Off;
        for want in [LoopMode::Loop, LoopMode::PingPong, LoopMode::Off] {
            mode = mode.next();
            assert_eq!(mode, want);
            let (lp, pp) = mode.flags();
            assert_eq!(LoopMode::from_flags(lp, pp), mode, "flags round-trip");
        }
        assert!(LoopMode::Loop.loops());
        assert!(LoopMode::PingPong.loops());
        assert!(!LoopMode::Off.loops());
    }

    #[test]
    fn w2_40_loop_mode_labels_state_the_mode() {
        assert_eq!(loop_mode_label(LoopMode::Off), "Loop: off");
        assert_eq!(loop_mode_label(LoopMode::Loop), "Loop: on");
        assert_eq!(loop_mode_label(LoopMode::PingPong), "Loop: ping-pong");
        // The two pre-W2-40 modes keep their Phase G
        // captions byte-for-byte; only the mode is new.
        assert_eq!(loop_mode_caption(LoopMode::Off), "Off — stops at the end");
        assert_eq!(loop_mode_caption(LoopMode::Loop), "On — wraps at the end");
        assert_eq!(
            loop_mode_caption(LoopMode::PingPong),
            "Ping-pong — bounces at the ends"
        );
    }

    #[test]
    fn stop_at_end_never_fires_with_loop_on() {
        // At the project end, at a range end, single frame:
        // looping on always wraps, never stops.
        assert!(!should_stop_at_end(true, true, None, 4, 5));
        assert!(!should_stop_at_end(true, true, Some((1, 3)), 3, 5));
        assert!(!should_stop_at_end(true, true, None, 0, 1));
    }

    #[test]
    fn stop_at_end_never_fires_while_paused() {
        assert!(!should_stop_at_end(false, false, None, 4, 5));
        assert!(!should_stop_at_end(false, false, Some((1, 3)), 3, 5));
        assert!(!should_stop_at_end(false, false, None, 0, 1));
    }

    #[test]
    fn stop_at_end_only_on_the_last_frame() {
        // 5-frame project, no range: frames 0..3 keep going,
        // frame 4 (the last) stops.
        for current in 0..4 {
            assert!(
                !should_stop_at_end(true, false, None, current, 5),
                "frame {current} is mid-project"
            );
        }
        assert!(should_stop_at_end(true, false, None, 4, 5));
    }

    #[test]
    fn stop_at_end_single_frame_project_stops_at_once() {
        // Frame 0 is also the last frame.
        assert!(should_stop_at_end(true, false, None, 0, 1));
    }

    #[test]
    fn stop_at_end_empty_project_never_stops() {
        assert!(!should_stop_at_end(true, false, None, 0, 0));
    }

    #[test]
    fn stop_at_end_uses_the_range_end_when_a_range_is_set() {
        // advance_play wraps inside (1, 3): frame 3 is the
        // end even though the project has a frame 4.
        assert!(!should_stop_at_end(true, false, Some((1, 3)), 2, 5));
        assert!(should_stop_at_end(true, false, Some((1, 3)), 3, 5));
        // Before the range, playback still plays into it.
        assert!(!should_stop_at_end(true, false, Some((2, 4)), 0, 5));
        // Parked past the range end: stops instead of being
        // wrapped back into the range.
        assert!(should_stop_at_end(true, false, Some((1, 3)), 4, 5));
    }

    #[test]
    fn stop_at_end_clamps_a_stale_range_end() {
        // Range end past the timeline (frames deleted since
        // it was set): advance_play clamps to the last frame
        // (editor.rs:719), so the stop check does too.
        assert!(!should_stop_at_end(true, false, Some((1, 99)), 3, 5));
        assert!(should_stop_at_end(true, false, Some((1, 99)), 4, 5));
    }

    #[test]
    fn key_frames_unions_camera_and_graph_keys_sorted_and_deduped() {
        let mut core = bare_core();
        // Camera keys land out of order in the call order;
        // set_camera_key keeps its own list sorted.
        core.project.set_camera_key(3, 0.0, 0.0, 1.0);
        core.project.set_camera_key(1, 0.0, 0.0, 1.0);
        // Graph keys: 1.0 duplicates camera key 1, 2.5
        // rounds onto camera key 3, 4.4 rounds to 4.
        add_channel(&mut core, &[1.0, 2.5, 4.4]);
        // A second channel contributes frame 0 (0.2 rounds
        // down) — the union spans every channel.
        add_channel(&mut core, &[0.2]);
        assert_eq!(key_frames(&core), vec![0, 1, 3, 4]);
    }

    #[test]
    fn key_frames_rounds_graph_keys_like_the_graph_seek() {
        let mut core = bare_core();
        // f32::round: halves away from zero, exactly the
        // convention timeline_marks' own tests pin.
        add_channel(&mut core, &[0.5, 2.49, 2.5]);
        assert_eq!(key_frames(&core), vec![1, 2, 3]);
    }

    #[test]
    fn key_frames_drops_out_of_timeline_keys() {
        let mut core = bare_core();
        // A camera key past the 5-frame timeline end drops,
        // like timeline_marks' marker sweep drops it.
        core.project.set_camera_key(9, 0.0, 0.0, 1.0);
        // Graph keys: 4.6 rounds to 5 (past the end), 99.0
        // is far out, -1.0 rounds below zero, NaN is not
        // finite — all dropped, none clamped onto an edge.
        add_channel(&mut core, &[4.6, 99.0, -1.0, f32::NAN]);
        // One in-range key survives so the drops above are
        // proven selective, not a blanket empty result.
        add_channel(&mut core, &[4.4]);
        assert_eq!(key_frames(&core), vec![4]);
    }

    #[test]
    fn key_frames_is_empty_without_keys() {
        let core = bare_core();
        assert!(key_frames(&core).is_empty());
    }

    #[test]
    fn prev_and_next_key_step_strictly_at_boundaries() {
        let keys = [1, 3];
        // Before the first key: no previous, next is 1.
        assert_eq!(prev_key(0, &keys), None);
        assert_eq!(next_key(0, &keys), Some(1));
        // Exactly on a key: strictly before / after, so a
        // press standing on key 1 goes BACK to nothing and
        // FORWARD to 3 — repeated presses walk the list.
        assert_eq!(prev_key(1, &keys), None);
        assert_eq!(next_key(1, &keys), Some(3));
        // Between keys.
        assert_eq!(prev_key(2, &keys), Some(1));
        assert_eq!(next_key(2, &keys), Some(3));
        // Exactly on the last key.
        assert_eq!(prev_key(3, &keys), Some(1));
        assert_eq!(next_key(3, &keys), None);
        // After the last key: previous is 3, no next.
        assert_eq!(prev_key(4, &keys), Some(3));
        assert_eq!(next_key(4, &keys), None);
    }

    #[test]
    fn prev_and_next_key_on_an_empty_list_are_none() {
        assert_eq!(prev_key(0, &[]), None);
        assert_eq!(next_key(0, &[]), None);
        assert_eq!(prev_key(3, &[]), None);
        assert_eq!(next_key(3, &[]), None);
    }

    #[test]
    fn key_jump_handlers_step_and_keep_the_playing_state() {
        // The pb-prev-key / pb-next-key handler bodies,
        // composed exactly as the buttons run them.
        let mut core = bare_core();
        core.project.set_camera_key(1, 0.0, 0.0, 1.0);
        core.project.set_camera_key(3, 0.0, 0.0, 1.0);
        core.goto(3);
        core.playing = false;
        // Prev from key 3 lands on key 1, still paused —
        // the jumps goto only, never start playback.
        let keys = key_frames(&core);
        if let Some(f) = prev_key(core.current, &keys) {
            core.goto(f);
        }
        assert_eq!(core.current, 1);
        assert!(!core.playing);
        // Next from key 1 returns to key 3.
        let keys = key_frames(&core);
        if let Some(f) = next_key(core.current, &keys) {
            core.goto(f);
        }
        assert_eq!(core.current, 3);
        // Prev at the first key is a silent no-op.
        core.goto(1);
        let keys = key_frames(&core);
        if let Some(f) = prev_key(core.current, &keys) {
            core.goto(f);
        }
        assert_eq!(core.current, 1);
    }

    #[test]
    fn speeds_are_the_five_steps_ascending_around_normal() {
        assert_eq!(SPEEDS, [0.25, 0.5, 1.0, 1.5, 2.0]);
        assert!(SPEEDS.windows(2).all(|w| w[0] < w[1]));
        assert!(SPEEDS.contains(&1.0), "normal speed is a step");
    }

    #[test]
    fn next_speed_walks_the_whole_table_and_wraps() {
        // The full cycle, one press at a time, from normal.
        let mut cur = 1.0;
        for want in [1.5, 2.0, 0.25, 0.5, 1.0] {
            cur = next_speed(cur);
            assert_eq!(cur, want);
        }
        // Every entry's own successor, pinned directly.
        assert_eq!(next_speed(0.25), 0.5);
        assert_eq!(next_speed(0.5), 1.0);
        assert_eq!(next_speed(1.5), 2.0);
        assert_eq!(next_speed(2.0), 0.25, "the top wraps to the first entry");
    }

    #[test]
    fn next_speed_off_table_goes_to_the_next_higher_entry() {
        assert_eq!(next_speed(0.75), 1.0);
        assert_eq!(next_speed(1.2), 1.5);
        assert_eq!(next_speed(0.1), 0.25, "below the table: the first entry");
        assert_eq!(next_speed(0.0), 0.25);
        assert_eq!(next_speed(3.0), 0.25, "above the maximum: wraps");
        assert_eq!(next_speed(f32::INFINITY), 0.25, "infinity wraps too");
    }

    #[test]
    fn next_speed_nan_is_normal_speed() {
        // NaN orders against nothing; the contract sends
        // it to 1.0 rather than the wrap entry.
        assert_eq!(next_speed(f32::NAN), 1.0);
    }

    #[test]
    fn speed_label_renders_all_five_steps() {
        assert_eq!(speed_label(0.25), "Speed 0.25×");
        assert_eq!(speed_label(0.5), "Speed 0.5×");
        assert_eq!(speed_label(1.0), "Speed 1×");
        assert_eq!(speed_label(1.5), "Speed 1.5×");
        assert_eq!(speed_label(2.0), "Speed 2×");
    }

    #[test]
    fn cycle_playback_speed_advances_the_field_and_stays_silent() {
        // The handler itself, run against a real shell
        // State (crate::init — the fixture snap_tests uses;
        // the groundwork initialises playback_speed to 1.0).
        let mut s = crate::init();
        assert_eq!(s.playback_speed, 1.0);
        let status_before = s.core.status.clone();
        let depth_before = s.core.undo_depth();
        cycle_playback_speed(&mut s);
        assert_eq!(s.playback_speed, 1.5);
        cycle_playback_speed(&mut s);
        assert_eq!(s.playback_speed, 2.0);
        cycle_playback_speed(&mut s);
        assert_eq!(s.playback_speed, 0.25, "the cycle wraps through the handler");
        // Silence, mirroring pb-loop: no status line was
        // written and no undo snapshot was taken.
        assert_eq!(s.core.status, status_before);
        assert_eq!(s.core.undo_depth(), depth_before);
    }

    #[test]
    fn loop_jump_targets_follow_the_loop_range() {
        let mut core = bare_core();
        // No range: no targets (and the buttons are not
        // rendered — the builder gates on the same value).
        assert_eq!(loop_jump_targets(&core), None);
        core.set_loop_range(1, 3);
        assert_eq!(loop_jump_targets(&core), Some((1, 3)));
        // Clearing the range takes the targets with it.
        core.clear_loop_range();
        assert_eq!(loop_jump_targets(&core), None);
    }

    #[test]
    fn loop_jump_targets_are_not_clamped_to_the_timeline() {
        let mut core = bare_core();
        // A stale range, written directly as a timeline
        // shrink would leave it (set_loop_range clamps at
        // set time, so this state only arises afterwards):
        // the helper hands the stored ends back untouched
        // and leaves the refusing to goto.
        core.loop_range = Some((1, 99));
        assert_eq!(loop_jump_targets(&core), Some((1, 99)));
    }

    #[test]
    fn loop_start_jump_lands_on_the_range_start_and_keeps_the_playing_state() {
        // The pb-loop-start handler body, composed exactly
        // as the button runs it (the keyframe-jump test's
        // precedent in this module).
        let mut core = bare_core();
        core.set_loop_range(1, 3);
        // Paused, parked past the range: the press lands
        // on the start and stays paused — the jumps goto
        // only, never start playback.
        core.goto(4);
        core.playing = false;
        if let Some((a, _)) = loop_jump_targets(&core) {
            core.goto(a);
        }
        assert_eq!(core.current, 1);
        assert_eq!(core.playhead, 1.0);
        assert!(!core.playing);
        // Playing, parked before the range: the same press
        // lands on the start and keeps playing.
        core.goto(0);
        core.playing = true;
        if let Some((a, _)) = loop_jump_targets(&core) {
            core.goto(a);
        }
        assert_eq!(core.current, 1);
        assert!(core.playing, "jumping mid-playback keeps playing");
    }

    #[test]
    fn loop_end_jump_lands_on_the_range_end_and_keeps_the_playing_state() {
        // The pb-loop-end handler body, composed as above.
        let mut core = bare_core();
        core.set_loop_range(1, 3);
        core.goto(0);
        core.playing = false;
        if let Some((_, b)) = loop_jump_targets(&core) {
            core.goto(b);
        }
        assert_eq!(core.current, 3);
        assert_eq!(core.playhead, 3.0);
        assert!(!core.playing);
        core.goto(2);
        core.playing = true;
        if let Some((_, b)) = loop_jump_targets(&core) {
            core.goto(b);
        }
        assert_eq!(core.current, 3);
        assert!(core.playing);
    }

    #[test]
    fn loop_jump_handlers_are_noops_without_a_range() {
        // With no range the buttons are absent, but the
        // composed bodies must still be harmless if run:
        // no target, no goto, position untouched.
        let mut core = bare_core();
        core.goto(2);
        if let Some((a, _)) = loop_jump_targets(&core) {
            core.goto(a);
        }
        if let Some((_, b)) = loop_jump_targets(&core) {
            core.goto(b);
        }
        assert_eq!(core.current, 2);
    }

    #[test]
    fn loop_end_jump_to_a_stale_end_refuses_silently() {
        let mut core = bare_core();
        // Stale end past the 5-frame timeline: goto's own
        // bounds check refuses, so the press changes
        // nothing — no clamp onto the last frame.
        core.loop_range = Some((1, 99));
        core.goto(4);
        if let Some((_, b)) = loop_jump_targets(&core) {
            core.goto(b);
        }
        assert_eq!(core.current, 4);
        // The in-range start of the same stale range still
        // jumps normally.
        if let Some((a, _)) = loop_jump_targets(&core) {
            core.goto(a);
        }
        assert_eq!(core.current, 1);
    }

    // ---- Wave 9 inert rules ----

    #[test]
    fn fps_stepper_inert_only_at_the_clamp_bounds() {
        // Mid-range: both steppers can move the rate.
        assert_eq!(fps_stepper_inert(12.0), (false, false));
        assert_eq!(fps_stepper_inert(29.97), (false, false));
        // At the floor only the down stepper is spent;
        // at the ceiling only the up stepper.
        assert_eq!(fps_stepper_inert(FPS_FLOOR), (true, false));
        assert_eq!(fps_stepper_inert(FPS_CEILING), (false, true));
        // Just inside the ceiling the up stepper still
        // lands on it — a real change, so it stays live.
        assert_eq!(fps_stepper_inert(59.5), (false, false));
        // A stored rate above the ceiling (a loaded
        // project): either press clamps DOWN to the
        // ceiling, which changes the rate, so neither
        // stepper may render inert.
        assert_eq!(fps_stepper_inert(120.0), (false, false));
    }

    #[test]
    fn duration_stepper_inert_only_where_the_press_cannot_move() {
        // No current frame: nothing to scale, both inert.
        assert_eq!(duration_stepper_inert(None), (true, true));
        // Mid-range: both factors change the duration.
        assert_eq!(duration_stepper_inert(Some(1.0)), (false, false));
        // Halving 0.5 lands on 0.25 — a change, so the
        // down stepper stays live just above the floor;
        // at the floor itself it cannot move.
        assert_eq!(duration_stepper_inert(Some(0.5)), (false, false));
        assert_eq!(duration_stepper_inert(Some(DURATION_FLOOR)), (true, false));
        // Doubling 5.0 clamps to 8.0 — a change; at the
        // ceiling the up stepper cannot move.
        assert_eq!(duration_stepper_inert(Some(5.0)), (false, false));
        assert_eq!(duration_stepper_inert(Some(DURATION_CEILING)), (false, true));
    }

    #[test]
    fn loop_jump_inert_only_for_stale_targets() {
        // No range: the buttons are absent; the helper's
        // total answer is both-inert.
        assert_eq!(loop_jump_inert(None, 5), (true, true));
        // Both ends on the timeline: both live, including
        // a range ending exactly on the last frame.
        assert_eq!(loop_jump_inert(Some((1, 3)), 5), (false, false));
        assert_eq!(loop_jump_inert(Some((0, 4)), 5), (false, false));
        // A stale end inerts only its own button — the
        // in-range start of the same range stays live,
        // mirroring loop_end_jump_to_a_stale_end above.
        assert_eq!(loop_jump_inert(Some((1, 99)), 5), (false, true));
        // A range wholly past the timeline inerts both.
        assert_eq!(loop_jump_inert(Some((99, 120)), 5), (true, true));
        // An empty timeline refuses every target.
        assert_eq!(loop_jump_inert(Some((0, 0)), 0), (true, true));
    }
}
