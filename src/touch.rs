//! Touch gesture recognition: a deterministic, platform-neutral
//! state machine that turns raw pointer events into editor gestures.
//!
//! The original StykAnimate is touch-capable; this port is mouse-only
//! today. GPUI delivers touch input as ordinary pointer events, so
//! the missing core is not platform code but this recogniser: feed it
//! [`PointerEvent`]s (from mouse or touch, it cannot tell the
//! difference) plus periodic [`Recognizer::on_tick`] calls, and it
//! emits a stream of [`Gesture`]s.
//!
//! # Mapping to editor actions (wiring is a later step)
//!
//! - `DragStart` / `Drag` / `DragEnd` — **pose**: drag the grabbed
//!   joint (hands/feet solve IK, hips move the figure), exactly like
//!   a mouse drag on the canvas.
//! - `PinchStart` / `Pinch` / `PinchEnd` — **camera**: `scale` zooms
//!   the camera (cumulative, relative to the distance the two
//!   fingers had at `PinchStart`), `pan` moves the camera centre by
//!   the same cumulative offset the pinch centre has travelled.
//! - `Tap` — **select**: pick the joint or figure under the point.
//! - `LongPress` — **context**: open the context action for the
//!   joint/figure under the point (the touch equivalent of a
//!   right-click).
//!
//! # Semantics
//!
//! - One pointer: movement beyond a 3-unit slop starts a drag.
//!   `DragStart` reports the original down position, followed
//!   immediately by a `Drag` carrying the full delta from that
//!   anchor, so no motion is lost to the slop; later `Drag`s carry
//!   per-event deltas.
//! - A quick press (released within 300 ms, never straying 6 units
//!   from the down point) is a `Tap`, emitted on release.
//! - Holding still for 500 ms is a `LongPress`, emitted by
//!   `on_tick` (never by an event), exactly once. After it fires the
//!   pointer is consumed: moving or releasing it emits nothing more.
//! - A second pointer down cancels any drag in progress (a
//!   `DragEnd` is emitted first, if one was running) and starts a
//!   pinch. Pinch moves emit `Pinch` with the current centre, the
//!   cumulative `scale` (current finger distance / start distance)
//!   and the cumulative `pan` (current centre − start centre).
//! - Lifting either pinch finger emits `PinchEnd`; the remaining
//!   finger becomes **inert** — it does not resume the drag — and
//!   any further pointers that go down are inert too, until every
//!   pointer is up and the recogniser is idle again.
//! - Events for unknown pointer ids (stray moves/ups, duplicate
//!   downs, a third finger during a pinch) are ignored.
//!
//! Everything is a pure function of the event stream and tick
//! times: no clocks, no randomness, so a scripted stream always
//! produces the same gesture sequence (which the tests assert).

use crate::model::V2;

/// Movement (stage units) a press may wander before it becomes a drag.
pub const DRAG_SLOP: f32 = 3.0;
/// A press released sooner than this (and within
/// [`TAP_MAX_DISTANCE`]) counts as a tap.
pub const TAP_MAX_MS: u64 = 300;
/// Maximum distance (stage units) between down and up for a tap.
pub const TAP_MAX_DISTANCE: f32 = 6.0;
/// Hold time after which `on_tick` reports a long-press.
pub const LONG_PRESS_MS: u64 = 500;
/// A press that has moved this far (stage units) can no longer
/// become a long-press.
pub const LONG_PRESS_MAX_DISTANCE: f32 = 6.0;

/// One raw pointer event, from mouse or touch indifferently.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerEvent {
    /// Pointer identity: constant from down to up for one contact.
    pub id: u32,
    /// Position in stage coordinates.
    pub pos: V2,
    pub kind: PointerKind,
    /// Event time in milliseconds (any monotonic epoch).
    pub t_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerKind {
    Down,
    Move,
    Up,
}

/// A recognised gesture. See the module docs for how each maps to
/// an editor action.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Gesture {
    /// A drag has begun; `pos` is where the pointer went down.
    DragStart { pos: V2 },
    /// Drag motion: `delta` since the previously reported position,
    /// `pos` the current position.
    Drag { delta: V2, pos: V2 },
    /// The dragging pointer was released (or its drag was cancelled
    /// by a second finger — a `PinchStart` follows in that case).
    DragEnd,
    /// A quick, still press, emitted on release at the up position.
    Tap { pos: V2 },
    /// A still hold of [`LONG_PRESS_MS`], emitted once by `on_tick`
    /// at the down position.
    LongPress { pos: V2 },
    /// A second finger went down; `centre` is the midpoint of the
    /// two pointers.
    PinchStart { centre: V2 },
    /// Pinch motion: current `centre`, cumulative `scale` relative
    /// to the finger distance at `PinchStart`, and cumulative `pan`
    /// of the centre since `PinchStart`.
    Pinch { centre: V2, scale: f32, pan: V2 },
    /// One pinch finger lifted; the other is inert until all
    /// pointers are up.
    PinchEnd,
}

/// What a lone pointer is currently doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// Down, but neither dragging nor long-pressed yet.
    Pending,
    /// Past the slop: a drag is in progress.
    Dragging,
    /// Long-press already fired; the pointer is consumed.
    LongPressed,
}

#[derive(Clone, Copy, Debug)]
struct OnePointer {
    id: u32,
    down_pos: V2,
    last_pos: V2,
    down_t: u64,
    phase: Phase,
}

#[derive(Clone, Copy, Debug)]
struct PinchState {
    a_id: u32,
    a_pos: V2,
    b_id: u32,
    b_pos: V2,
    start_dist: f32,
    start_centre: V2,
}

#[derive(Clone, Debug)]
enum State {
    Idle,
    One(OnePointer),
    Pinch(PinchState),
    /// Pointer ids still down after a pinch ended (or that went
    /// down while any were): tracked only so their ups can be
    /// counted; they emit nothing.
    Inert(Vec<u32>),
}

/// The gesture recogniser. One instance per canvas.
pub struct Recognizer {
    state: State,
}

impl Default for Recognizer {
    fn default() -> Self {
        Self::new()
    }
}

fn sub(a: V2, b: V2) -> V2 {
    [a[0] - b[0], a[1] - b[1]]
}

fn distance(a: V2, b: V2) -> f32 {
    let d = sub(a, b);
    (d[0] * d[0] + d[1] * d[1]).sqrt()
}

fn midpoint(a: V2, b: V2) -> V2 {
    [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0]
}

impl Recognizer {
    pub fn new() -> Self {
        Recognizer { state: State::Idle }
    }

    /// Feed one pointer event; returns the gestures it produced
    /// (usually zero or one; two when a move crosses the drag slop,
    /// or a second finger cancels a drag into a pinch).
    pub fn on_event(&mut self, ev: PointerEvent) -> Vec<Gesture> {
        let mut out = Vec::new();
        // Take the state out so arms can move it freely; every arm
        // either puts a state back or leaves `next` as the result.
        let state = std::mem::replace(&mut self.state, State::Idle);
        let mut next = State::Idle;
        match state {
            State::Idle => {
                if ev.kind == PointerKind::Down {
                    next = State::One(OnePointer {
                        id: ev.id,
                        down_pos: ev.pos,
                        last_pos: ev.pos,
                        down_t: ev.t_ms,
                        phase: Phase::Pending,
                    });
                }
                // Stray Move/Up with nothing down: ignored.
            }
            State::One(mut one) => {
                if ev.id != one.id {
                    if ev.kind == PointerKind::Down {
                        // Second finger: cancel any drag, start pinch.
                        if one.phase == Phase::Dragging {
                            out.push(Gesture::DragEnd);
                        }
                        let centre = midpoint(one.last_pos, ev.pos);
                        next = State::Pinch(PinchState {
                            a_id: one.id,
                            a_pos: one.last_pos,
                            b_id: ev.id,
                            b_pos: ev.pos,
                            start_dist: distance(one.last_pos, ev.pos),
                            start_centre: centre,
                        });
                        out.push(Gesture::PinchStart { centre });
                    } else {
                        // Another pointer's move/up: not ours, ignore.
                        next = State::One(one);
                    }
                    self.state = next;
                    return out;
                }
                match ev.kind {
                    PointerKind::Down => {
                        // Duplicate down for the active id: ignore.
                        next = State::One(one);
                    }
                    PointerKind::Move => {
                        match one.phase {
                            Phase::Pending => {
                                if distance(one.down_pos, ev.pos) > DRAG_SLOP {
                                    one.phase = Phase::Dragging;
                                    out.push(Gesture::DragStart { pos: one.down_pos });
                                    out.push(Gesture::Drag {
                                        delta: sub(ev.pos, one.down_pos),
                                        pos: ev.pos,
                                    });
                                }
                            }
                            Phase::Dragging => {
                                if ev.pos != one.last_pos {
                                    out.push(Gesture::Drag {
                                        delta: sub(ev.pos, one.last_pos),
                                        pos: ev.pos,
                                    });
                                }
                            }
                            Phase::LongPressed => {}
                        }
                        one.last_pos = ev.pos;
                        next = State::One(one);
                    }
                    PointerKind::Up => {
                        match one.phase {
                            Phase::Dragging => out.push(Gesture::DragEnd),
                            Phase::Pending => {
                                let held = ev.t_ms.saturating_sub(one.down_t);
                                if held < TAP_MAX_MS
                                    && distance(one.down_pos, ev.pos) < TAP_MAX_DISTANCE
                                {
                                    out.push(Gesture::Tap { pos: ev.pos });
                                }
                            }
                            Phase::LongPressed => {}
                        }
                        // Pointer gone: back to Idle (next stays Idle).
                    }
                }
            }
            State::Pinch(mut pinch) => {
                match ev.kind {
                    PointerKind::Move => {
                        if ev.id == pinch.a_id {
                            pinch.a_pos = ev.pos;
                        } else if ev.id == pinch.b_id {
                            pinch.b_pos = ev.pos;
                        } else {
                            next = State::Pinch(pinch);
                            self.state = next;
                            return out;
                        }
                        let centre = midpoint(pinch.a_pos, pinch.b_pos);
                        let scale = if pinch.start_dist > 0.0 {
                            distance(pinch.a_pos, pinch.b_pos) / pinch.start_dist
                        } else {
                            1.0
                        };
                        out.push(Gesture::Pinch {
                            centre,
                            scale,
                            pan: sub(centre, pinch.start_centre),
                        });
                        next = State::Pinch(pinch);
                    }
                    PointerKind::Up => {
                        if ev.id == pinch.a_id || ev.id == pinch.b_id {
                            out.push(Gesture::PinchEnd);
                            let remaining =
                                if ev.id == pinch.a_id { pinch.b_id } else { pinch.a_id };
                            next = State::Inert(vec![remaining]);
                        } else {
                            next = State::Pinch(pinch);
                        }
                    }
                    PointerKind::Down => {
                        // A third finger during a pinch is not
                        // tracked at all; its later events are stray.
                        next = State::Pinch(pinch);
                    }
                }
            }
            State::Inert(mut ids) => {
                match ev.kind {
                    PointerKind::Down => {
                        if !ids.contains(&ev.id) {
                            ids.push(ev.id);
                        }
                    }
                    PointerKind::Up => {
                        ids.retain(|&id| id != ev.id);
                    }
                    PointerKind::Move => {}
                }
                if !ids.is_empty() {
                    next = State::Inert(ids);
                }
            }
        }
        self.state = next;
        out
    }

    /// Periodic time update; the only source of `LongPress`.
    /// Call it from the editor's frame/timer loop with the current
    /// time in the same epoch as the events.
    pub fn on_tick(&mut self, t_ms: u64) -> Vec<Gesture> {
        let mut out = Vec::new();
        if let State::One(one) = &mut self.state {
            if one.phase == Phase::Pending
                && t_ms.saturating_sub(one.down_t) >= LONG_PRESS_MS
                && distance(one.down_pos, one.last_pos) < LONG_PRESS_MAX_DISTANCE
            {
                one.phase = Phase::LongPressed;
                out.push(Gesture::LongPress { pos: one.down_pos });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn down(id: u32, x: f32, y: f32, t: u64) -> PointerEvent {
        PointerEvent { id, pos: [x, y], kind: PointerKind::Down, t_ms: t }
    }
    fn mv(id: u32, x: f32, y: f32, t: u64) -> PointerEvent {
        PointerEvent { id, pos: [x, y], kind: PointerKind::Move, t_ms: t }
    }
    fn up(id: u32, x: f32, y: f32, t: u64) -> PointerEvent {
        PointerEvent { id, pos: [x, y], kind: PointerKind::Up, t_ms: t }
    }

    #[test]
    fn tap_on_quick_still_release() {
        let mut r = Recognizer::new();
        assert!(r.on_event(down(1, 10.0, 10.0, 0)).is_empty());
        assert_eq!(
            r.on_event(up(1, 10.0, 10.0, 120)),
            vec![Gesture::Tap { pos: [10.0, 10.0] }]
        );
        // Recogniser is reusable afterwards.
        assert!(r.on_event(down(1, 0.0, 0.0, 200)).is_empty());
        assert_eq!(
            r.on_event(up(1, 0.0, 0.0, 250)),
            vec![Gesture::Tap { pos: [0.0, 0.0] }]
        );
    }

    #[test]
    fn slow_press_is_not_a_tap() {
        let mut r = Recognizer::new();
        r.on_event(down(1, 10.0, 10.0, 0));
        assert!(r.on_event(up(1, 10.0, 10.0, 400)).is_empty());
    }

    #[test]
    fn long_press_fires_once_via_tick() {
        let mut r = Recognizer::new();
        r.on_event(down(1, 10.0, 10.0, 0));
        assert!(r.on_tick(499).is_empty());
        assert_eq!(
            r.on_tick(500),
            vec![Gesture::LongPress { pos: [10.0, 10.0] }]
        );
        // Once only.
        assert!(r.on_tick(600).is_empty());
        assert!(r.on_tick(5000).is_empty());
        // The consumed pointer emits nothing on release: no tap.
        assert!(r.on_event(up(1, 10.0, 10.0, 5100)).is_empty());
    }

    #[test]
    fn movement_before_timeout_kills_long_press() {
        let mut r = Recognizer::new();
        r.on_event(down(1, 0.0, 0.0, 0));
        // Crossing the slop starts a drag instead.
        let g = r.on_event(mv(1, 20.0, 0.0, 100));
        assert_eq!(g[0], Gesture::DragStart { pos: [0.0, 0.0] });
        assert!(r.on_tick(600).is_empty());
        assert_eq!(r.on_event(up(1, 20.0, 0.0, 700)), vec![Gesture::DragEnd]);
    }

    #[test]
    fn drag_respects_slop_then_streams_deltas() {
        let mut r = Recognizer::new();
        r.on_event(down(1, 0.0, 0.0, 0));
        // Inside the 3-unit slop: nothing yet.
        assert!(r.on_event(mv(1, 2.0, 0.0, 16)).is_empty());
        // Crossing it: DragStart at the anchor, then a Drag with
        // the full delta from the anchor (slop motion not lost).
        assert_eq!(
            r.on_event(mv(1, 10.0, 0.0, 32)),
            vec![
                Gesture::DragStart { pos: [0.0, 0.0] },
                Gesture::Drag { delta: [10.0, 0.0], pos: [10.0, 0.0] },
            ]
        );
        // Later moves carry per-event deltas.
        assert_eq!(
            r.on_event(mv(1, 13.0, 4.0, 48)),
            vec![Gesture::Drag { delta: [3.0, 4.0], pos: [13.0, 4.0] }]
        );
        assert_eq!(r.on_event(up(1, 13.0, 4.0, 64)), vec![Gesture::DragEnd]);
    }

    #[test]
    fn pinch_scale_doubles_when_distance_doubles() {
        let mut r = Recognizer::new();
        r.on_event(down(1, 0.0, 0.0, 0));
        assert_eq!(
            r.on_event(down(2, 10.0, 0.0, 10)),
            vec![Gesture::PinchStart { centre: [5.0, 0.0] }]
        );
        // Finger 2 moves 10 -> 20: distance 10 -> 20, centre 5 -> 10.
        assert_eq!(
            r.on_event(mv(2, 20.0, 0.0, 20)),
            vec![Gesture::Pinch { centre: [10.0, 0.0], scale: 2.0, pan: [5.0, 0.0] }]
        );
        // Finger 1 moves 0 -> -10: distance 20 -> 30, centre back to 5.
        assert_eq!(
            r.on_event(mv(1, -10.0, 0.0, 30)),
            vec![Gesture::Pinch { centre: [5.0, 0.0], scale: 3.0, pan: [0.0, 0.0] }]
        );
        assert_eq!(r.on_event(up(2, 20.0, 0.0, 40)), vec![Gesture::PinchEnd]);
    }

    #[test]
    fn second_finger_cancels_drag_into_pinch() {
        let mut r = Recognizer::new();
        r.on_event(down(1, 0.0, 0.0, 0));
        let g = r.on_event(mv(1, 10.0, 0.0, 16));
        assert_eq!(g.len(), 2); // DragStart + Drag
        // The drag is ended first, then the pinch starts.
        assert_eq!(
            r.on_event(down(2, 20.0, 0.0, 32)),
            vec![Gesture::DragEnd, Gesture::PinchStart { centre: [15.0, 0.0] }]
        );
    }

    #[test]
    fn no_drag_resume_after_pinch_ends() {
        let mut r = Recognizer::new();
        r.on_event(down(1, 0.0, 0.0, 0));
        r.on_event(down(2, 10.0, 0.0, 10));
        assert_eq!(r.on_event(up(2, 10.0, 0.0, 20)), vec![Gesture::PinchEnd]);
        // The remaining finger is inert: moving it is not a drag,
        // and a fresh finger going down is inert too.
        assert!(r.on_event(mv(1, 50.0, 0.0, 30)).is_empty());
        assert!(r.on_event(down(3, 90.0, 0.0, 35)).is_empty());
        assert!(r.on_event(mv(3, 99.0, 0.0, 40)).is_empty());
        assert!(r.on_event(up(1, 50.0, 0.0, 50)).is_empty());
        assert!(r.on_event(up(3, 99.0, 0.0, 55)).is_empty());
        // Only once every pointer is up does the next press work.
        r.on_event(down(1, 0.0, 0.0, 100));
        let g = r.on_event(mv(1, 10.0, 0.0, 116));
        assert_eq!(g[0], Gesture::DragStart { pos: [0.0, 0.0] });
        assert_eq!(r.on_event(up(1, 10.0, 0.0, 132)), vec![Gesture::DragEnd]);
    }

    #[test]
    fn stray_events_for_unknown_ids_are_ignored() {
        let mut r = Recognizer::new();
        assert!(r.on_event(mv(7, 1.0, 1.0, 0)).is_empty());
        assert!(r.on_event(up(7, 1.0, 1.0, 0)).is_empty());
        r.on_event(down(1, 0.0, 0.0, 0));
        // Another id's move/up do not disturb the tracked pointer.
        assert!(r.on_event(mv(9, 100.0, 100.0, 10)).is_empty());
        assert!(r.on_event(up(9, 100.0, 100.0, 20)).is_empty());
        assert_eq!(
            r.on_event(up(1, 0.0, 0.0, 30)),
            vec![Gesture::Tap { pos: [0.0, 0.0] }]
        );
    }
}
