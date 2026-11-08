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
//! * **Loop range** — `loop-start` / `loop-end` move one end
//!   of `EditorCore::loop_range` (`editor.rs:61`) to the
//!   current frame via `EditorCore::set_loop_range`
//!   (`editor.rs:738`, orders + clamps; the other end is kept,
//!   or the current frame when no range exists — the GPUI
//!   handlers at old `app.rs:3933` / `:3939` verbatim);
//!   `loop-clear` calls `EditorCore::clear_loop_range`
//!   (`editor.rs:752`). Playback wraps inside the range in
//!   `advance_play` (`editor.rs:687`).
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
//!   either end of the key list the press is a silent
//!   no-op, like the transport's other edges.
//! * **Frame duration** — the `dur-dn` / `dur-up` steppers
//!   call `EditorCore::adjust_frame_duration`
//!   (`editor.rs:1675`) with the GPUI factors 0.5 / 2.0
//!   (multiplies the current frame's `duration`, clamps
//!   0.25..=8.0).
//! * **Timecode** — a read-only `current / total` readout in
//!   `M:SS.t`, ported verbatim from the GPUI `ui/timecode.rs`
//!   chip: frame index ÷ fps (NOT duration-weighted; the
//!   duration-weighted total is `Project::total_time`,
//!   `model.rs:855`, and is deliberately not what this chip
//!   showed).
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
//!   the loop row, beside `pb-loop` / `pb-speed`, and
//!   rendered ONLY while a range is set — with no range
//!   there is nothing to jump to, so the buttons are
//!   absent rather than inert (this file has no inert
//!   button variant). Like the keyframe jumps they only
//!   `goto` (`editor.rs:701`) — `playing` is never
//!   touched, so jumping while paused stays paused and
//!   jumping mid-playback keeps playing. The targets are
//!   NOT clamped here: `set_loop_range` (`editor.rs:769`)
//!   orders and clamps when the range is set, but frames
//!   deleted afterwards can leave an end past the
//!   timeline, and `goto`'s own bounds check then refuses
//!   the press silently — the keyframe jumps' no-op
//!   discipline at their edges. Not to be confused with
//!   the `loop-start` / `loop-end` buttons further down,
//!   which SET the range ends to the current frame; these
//!   jump TO them.
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
//!
//! `State` fields: everything reads `s.core` live
//! (`project.fps` `model.rs:683`, `loop_range`, `current`,
//! `project.frames[..].duration`) except the loop toggle,
//! which reads/writes the shell-level `State::loop_playback`
//! (integrator-owned, initialised `true`), and the speed
//! cycle, which reads/writes the shell-level
//! `State::playback_speed` (integrator-owned, initialised
//! `1.0`).

use rune::tok::{self, type_, Theme};
use rune::{row, text, El};

use crate::chrome_view::btn;
use crate::rune_sec::design;
use crate::rune_sec::icon_btn::icon_btn;
use crate::rune_sec::timeline_marks::graph_key_frame;
use crate::State;
use stykanimate_rs::editor::EditorCore;

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

/// Readout for the loop-range row: the inclusive range in
/// 1-based frame numbers, or the whole-timeline note when no
/// range is set.
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

/// The frames the **Loop start** / **Loop end** jump
/// buttons go to: the loop range's ends exactly as stored,
/// or `None` when no range is set (the buttons are then
/// not rendered at all — see the module docs).
///
/// Deliberately NOT clamped to the timeline. A range is
/// ordered and clamped when it is set (`set_loop_range`,
/// `editor.rs:769`), but frames deleted afterwards can
/// leave its end past the last frame; the handlers hand
/// the stored frame straight to `EditorCore::goto`
/// (`editor.rs:701`), whose own bounds check refuses it
/// silently, leaving `current` and `playhead` where they
/// were — the same silent no-op the keyframe jumps make
/// at the ends of their key list.
pub fn loop_jump_targets(core: &EditorCore) -> Option<(usize, usize)> {
    core.loop_range
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
/// reads correctly at a glance in either state.
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

/// The readout's text: `"<current> / <total>"`, where current
/// is `current_frame / fps` seconds and total is
/// `frame_count / fps` (the GPUI chip's semantics, ported
/// verbatim). A non-positive `fps` falls back to 1.0 — the
/// codebase's convention wherever a frame rate divides time —
/// so the label still resolves instead of dividing by zero.
pub fn timecode_label(current_frame: usize, frame_count: usize, fps: f32) -> String {
    let fps = if fps > 0.0 { fps } else { 1.0 };
    format!(
        "{} / {}",
        format_timecode(current_frame as f32 / fps),
        format_timecode(frame_count as f32 / fps)
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
/// `fps-up`, `tween`, `loop-start`, `loop-end`, `loop-clear`,
/// `dur-dn`, `dur-up`); the Phase F transport buttons are
/// `pb-restart` / `pb-play-range`, the Phase G loop
/// toggle is `pb-loop` and the Phase I keyframe jumps are
/// `pb-prev-key` / `pb-next-key`; the Phase K speed cycle
/// beside the loop toggle is `pb-speed`, and the Phase L
/// loop jumps beside it are `pb-loop-start` / `pb-loop-end`
/// (present only while a loop range is set), so headless
/// `--tree` checks can find every control.
pub fn playback_controls(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let fps = core.project.fps;
    let tween_on = core.project.tween_playback;
    let loop_range = core.loop_range;
    let loop_playback = s.loop_playback;
    let playback_speed = s.playback_speed;
    let cur_duration = core
        .project
        .frames
        .get(core.current)
        .map(|f| f.duration);

    let mut panel = design::panel_col("playback-controls", t)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(design::panel_title(None, "Playback", None, t)),
        )
        .child(design::divider(t));

    // ---- Transport basics: restart from the top, or play
    // from the loop range's start (frame 0 without a range).
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
            .child(btn("pb-prev-key", "Prev key", false, t, |s| {
                let keys = key_frames(&s.core);
                if let Some(f) = prev_key(s.core.current, &keys) {
                    s.core.goto(f);
                }
            }))
            .child(btn("pb-next-key", "Next key", false, t, |s| {
                let keys = key_frames(&s.core);
                if let Some(f) = next_key(s.core.current, &keys) {
                    s.core.goto(f);
                }
            })),
    );

    // ---- Loop playback: shell-level on/off for the wrap
    // advance_play always performs. Off = poll pauses at the
    // end (see should_stop_at_end); the button itself only
    // flips the State flag — no core call, no undo snapshot.
    panel = panel.child(design::section_label("Loop playback", t));
    let mut loop_row = row()
        .gap(6.0)
        .center_y()
        .flex_wrap()
        .child(icon_btn(
            "pb-loop",
            "repeat",
            loop_toggle_label(loop_playback),
            loop_playback,
            t,
            |s| {
                s.loop_playback = !s.loop_playback;
            },
        ))
        .child(icon_btn(
            "pb-speed",
            "gauge",
            &speed_label(playback_speed),
            playback_speed != 1.0,
            t,
            cycle_playback_speed,
        ));
    // The loop jumps exist only while a range is set:
    // absent, not inert, when there is nothing to jump to.
    if loop_range.is_some() {
        loop_row = loop_row
            .child(btn("pb-loop-start", "Loop start", false, t, |s| {
                if let Some((a, _)) = loop_jump_targets(&s.core) {
                    s.core.goto(a);
                }
            }))
            .child(btn("pb-loop-end", "Loop end", false, t, |s| {
                if let Some((_, b)) = loop_jump_targets(&s.core) {
                    s.core.goto(b);
                }
            }));
    }
    loop_row = loop_row.child(design::section_label(
        if loop_playback {
            "On — wraps at the end"
        } else {
            "Off — stops at the end"
        },
        t,
    ));
    panel = panel.child(loop_row);

    // ---- Frame rate: stepper + current-rate readout.
    panel = panel.child(design::section_label(
        &format!("Frame rate — now {}", fps_text(fps)),
        t,
    ));
    let mut rate_row = row().gap(6.0).center_y();
    rate_row = rate_row.child(btn("fps-dn", "FPS −", false, t, |s| {
        s.core.adjust_fps(-1.0);
    }));
    rate_row = rate_row.child(btn("fps-up", "FPS +", false, t, |s| {
        s.core.adjust_fps(1.0);
    }));
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
            .child(btn("tween", "Tween play", tween_on, t, |s| {
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

    // ---- Loop range: move either end to the current frame
    // (GPUI handlers verbatim), or clear.
    panel = panel.child(design::section_label(&loop_range_text(loop_range), t));
    panel = panel.child(
        row()
            .gap(6.0)
            .center_y()
            .child(btn(
                "loop-start",
                "Loop start",
                loop_range.is_some(),
                t,
                |s| {
                    let cur = s.core.current;
                    let (_, b) = s.core.loop_range.unwrap_or((cur, cur));
                    s.core.set_loop_range(cur, b);
                },
            ))
            .child(btn(
                "loop-end",
                "Loop end",
                loop_range.is_some(),
                t,
                |s| {
                    let cur = s.core.current;
                    let (a, _) = s.core.loop_range.unwrap_or((cur, cur));
                    s.core.set_loop_range(a, cur);
                },
            ))
            .child(btn("loop-clear", "Loop clear", false, t, |s| {
                s.core.clear_loop_range();
            })),
    );

    // ---- Current-frame duration stepper (0.5x / 2x, the
    // GPUI factors; the op clamps 0.25..=8.0).
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
    panel = panel.child(
        row()
            .gap(6.0)
            .center_y()
            .child(btn("dur-dn", "Dur −", false, t, |s| {
                s.core.adjust_frame_duration(0.5);
            }))
            .child(btn("dur-up", "Dur +", false, t, |s| {
                s.core.adjust_frame_duration(2.0);
            })),
    );

    // ---- Timecode readout (read-only).
    panel = panel.child(design::section_label("Time", t));
    panel = panel.child(
        text(timecode_label(core.current, core.frame_count(), fps))
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
    fn timecode_label_is_current_over_total_at_fps() {
        // The GPUI chip's own example shape: 5 frames at 12 fps.
        assert_eq!(timecode_label(0, 5, 12.0), "0:00.0 / 0:00.4");
        assert_eq!(timecode_label(24, 48, 24.0), "0:01.0 / 0:02.0");
        assert_eq!(timecode_label(0, 0, 12.0), "0:00.0 / 0:00.0");
    }

    #[test]
    fn timecode_label_falls_back_to_1fps_for_bad_rates() {
        assert_eq!(timecode_label(30, 60, 0.0), "0:30.0 / 1:00.0");
        assert_eq!(timecode_label(30, 60, -12.0), "0:30.0 / 1:00.0");
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
}
