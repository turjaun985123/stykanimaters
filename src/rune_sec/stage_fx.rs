//! Phase E, section 3/40 — STAGE WEATHER + FX PARTICLES:
//! pure stage-space geometry for the two continuous particle
//! layers, so the Rune stage can draw exactly what exports
//! render. Declared from `main.rs` only (bin-crate code, not
//! part of the engine library).
//!
//! ## Source of truth — the rasterizer (`src/export.rs`)
//!
//! Nothing here re-implements particle motion. Both builders
//! call the engine's own closed-form evaluators, the same
//! ones `render_frame_at` (export.rs:265) calls:
//!
//! - **Weather:** export.rs:487–493 appends
//!   `vfx::weather_particles(weather, frame_f, project.stage,
//!   project.fps)` (vfx.rs:1070) to the particle list, after
//!   the FX clips — so in exports weather draws OVER FX. The
//!   two builders here are separate; a consumer matching the
//!   rasterizer draws `fx_geometry` first, `weather_geometry`
//!   last.
//! - **FX clips:** export.rs:447–485 loops `project.fx_clips`:
//!   entries whose figure index is out of range or whose
//!   figure is hidden are skipped; the anchor is
//!   `evaluate(&fig.def, &poses[figure])` with
//!   `poses = project.poses_at(frame_f)` (export.rs:273), and
//!   the motes are `vfx::clip_particles(&entry.clip,
//!   frame_f, anchor)` (export.rs:453). A live `Trail` clip
//!   additionally emits the node's real recent path from
//!   `export::node_trail(project, frame_f, figure, node, 8,
//!   span)` with `span = (duration * 0.5).clamp(2.0, 12.0)`
//!   (export.rs:467) into the rasterizer's *segment* list,
//!   which its pixel pass blends before the particle list —
//!   trail lines sit UNDER the discs. `fx_geometry` mirrors
//!   that pass order: every `Line` prim precedes every `Disc`.
//!
//! The frame both builders evaluate at is
//! `core.display_frame()` (editor.rs:723) — the same frame
//! the stage draws poses at (fractional while playing with
//! tween playback on, else the authored frame).
//!
//! Determinism: `weather_particles` and `clip_particles`
//! are stateless closed forms of `(seed, index, frame_f)`
//! built on the shared xorshift `prand` hash (vfx.rs:186) —
//! no wall clock, no accumulation, no unseeded randomness
//! anywhere in this file. Same core state → same geometry,
//! every call.
//!
//! ## Deltas vs the rasterizer, honestly
//!
//! 1. **Alpha is carried** (`alpha` on both variants). The
//!    brief's literal prim shape had no alpha field, but the
//!    rasterizer's particle pass is pure alpha blending and
//!    several effects are nothing without it — Fog discs sit
//!    at alpha 0.05–0.10 (vfx.rs Fog arm), rain at 0.38–0.62,
//!    trail lines fade to 0.5. Dropping alpha would render
//!    fog banks as solid pale discs. Colour is packed
//!    `0xRRGGBB`, the packing the rasterizer itself uses for
//!    trail segments (export.rs, hex conversion in the
//!    fx_clips loop) and `stage_view::hex_color` consumes.
//! 2. **Anchor evaluation:** the rasterizer anchors clips
//!    with raw `project.poses_at(frame_f)` (export.rs:273).
//!    This file uses `EditorCore::positions_f`
//!    (editor.rs:1049), which is *the same evaluation* when
//!    the project has no graph channels, and applies graph
//!    channel overrides when it does. That is deliberate:
//!    the stage itself draws figures through `positions_f`,
//!    so stage FX stay glued to the joint the user sees.
//!    On a channelled project an export can anchor a clip
//!    slightly differently from the stage preview.
//! 3. **Fog ellipse bands do not exist.** A comment at
//!    export.rs:443–446 describes screen-space fog bands
//!    "filled by the weather block below", but no such code
//!    exists anywhere in the tree (grep-verified); in the
//!    real rasterizer Fog is exactly the large faint discs
//!    `weather_particles` returns, and that is what
//!    `weather_geometry` returns.
//! 4. **Rain is discs, not streaks.** The rasterizer's pixel
//!    pass draws every particle as a disc (`dist <= r`);
//!    rain's radius is 1.2. The `Line` variant exists for
//!    Trail paths (delta-free), not for rain.
//! 5. **Stage space, unscaled.** Positions and radii are raw
//!    stage units; the rasterizer scales radii by
//!    `zoom * supersample` at draw time (export.rs:453 area).
//!    The consumer applies its own view transform, exactly
//!    as `stage_view` does for figure geometry.
//! 6. **Scope:** the one-shot `project.particles` events
//!    (`export::particles_at`, export.rs:120) are not FX
//!    clips and are not included here.
//! 7. **Gizmo layer:** `gizmo_geometry` has no rasterizer
//!    counterpart at all — it is an editor aid marking where
//!    one `project.particles` event is anchored, drawn by
//!    the shell only for the selected event. Its two
//!    deliberate un-render-like choices (live anchor
//!    resolution, no frame gating) are documented on the
//!    function itself.
//! 8. **FX clip gizmo:** `fx_clip_gizmo_geometry` is the
//!    same marker for a `project.fx_clips` entry — same
//!    crosshair, same ring, same colour, built by the same
//!    helper. Its one deliberate difference from the event
//!    gizmo is *when* it evaluates the anchor: at the
//!    clip's start frame, not the display frame. The
//!    marker sits at the pose where the effect FIRES (its
//!    start frame), so it is visible from any playhead
//!    position — that is deliberate: it answers "where
//!    will this effect appear?", not "where is the joint
//!    right now?". The full contract is documented on the
//!    function itself.

use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::{FxClipEntry, ParticleEvent};
use stykanimate_rs::vfx::{clip_particles, weather_particles, ClipKind};

/// One drawable particle primitive in stage coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ParticlePrim {
    /// A filled disc: a weather particle or one FX mote.
    Disc {
        pos: [f32; 2],
        radius: f32,
        /// Packed `0xRRGGBB`.
        color: u32,
        /// Blend alpha, 0..=1, from the engine evaluator.
        alpha: f32,
    },
    /// A line segment: a Trail clip's real node-path segment
    /// (the rasterizer draws these in its segment pass, under
    /// the discs).
    Line {
        a: [f32; 2],
        b: [f32; 2],
        /// Packed `0xRRGGBB` (the anchored figure's colour).
        color: u32,
        alpha: f32,
    },
}

/// Pack an `[r, g, b]` byte triple as `0xRRGGBB` — the same
/// packing the rasterizer uses for trail segment colours.
fn pack_rgb(c: [u8; 3]) -> u32 {
    ((c[0] as u32) << 16) | ((c[1] as u32) << 8) | c[2] as u32
}

/// Weather particles for the frame the stage is showing, in
/// stage coordinates. Empty when the project has no weather
/// or its amount rounds to zero.
///
/// This is a straight conversion of
/// `vfx::weather_particles(weather, display_frame,
/// project.stage, project.fps)` — the exact call the
/// rasterizer makes at export.rs:489 — so counts match
/// exports one for one: `amount` rounded, clamped to
/// 0..=200 (Storm: `amount * 1.8`, clamped to 0..=360, plus
/// its lightning disc on frames whose index mod 47 is 0
/// or 1).
pub fn weather_geometry(core: &EditorCore) -> Vec<ParticlePrim> {
    let Some(weather) = &core.project.weather else {
        return Vec::new();
    };
    let frame_f = core.display_frame();
    weather_particles(weather, frame_f, core.project.stage, core.project.fps)
        .into_iter()
        .map(|(pos, radius, color, alpha)| ParticlePrim::Disc {
            pos,
            radius,
            color: pack_rgb(color),
            alpha,
        })
        .collect()
}

/// FX clip particles for the frame the stage is showing, in
/// stage coordinates: for each entry of `project.fx_clips`,
/// the anchor is the entry node's world position via
/// `core.positions_f` and the motes come from
/// `vfx::clip_particles` — the rasterizer's own eval
/// (export.rs:447–485), including its guards: out-of-range
/// figure, hidden figure and out-of-range node entries are
/// skipped, and clips outside their `[start, start +
/// duration)` window emit nothing (that window check lives
/// inside `clip_particles` itself).
///
/// Live `Trail` clips also contribute their node's real
/// path as `Line` prims via `export::node_trail` with the
/// rasterizer's exact parameters (8 samples, span =
/// `(duration * 0.5).clamp(2.0, 12.0)`, export.rs:467).
/// All lines are returned before all discs, matching the
/// rasterizer's segment-pass-then-particle-pass order.
pub fn fx_geometry(core: &EditorCore) -> Vec<ParticlePrim> {
    let frame_f = core.display_frame();
    let project = &core.project;
    let mut lines: Vec<ParticlePrim> = Vec::new();
    let mut discs: Vec<ParticlePrim> = Vec::new();
    for entry in &project.fx_clips {
        // The rasterizer's guards, verbatim (export.rs:447+).
        if entry.figure >= project.figures.len() {
            continue;
        }
        if project.figures[entry.figure].hidden {
            continue;
        }
        let pos = core.positions_f(frame_f, entry.figure);
        let Some(&anchor) = pos.get(entry.node) else {
            continue;
        };
        if entry.clip.kind == ClipKind::Trail
            && frame_f >= entry.clip.start
            && frame_f < entry.clip.start + entry.clip.duration
        {
            let span = (entry.clip.duration * 0.5).clamp(2.0, 12.0);
            for (a, b, c, alpha) in stykanimate_rs::export::node_trail(
                project,
                frame_f,
                entry.figure,
                entry.node,
                8,
                span,
            ) {
                lines.push(ParticlePrim::Line {
                    a,
                    b,
                    color: pack_rgb(c),
                    alpha,
                });
            }
        }
        for (p, r, c, a) in clip_particles(&entry.clip, frame_f, anchor) {
            discs.push(ParticlePrim::Disc {
                pos: p,
                radius: r,
                color: pack_rgb(c),
                alpha: a,
            });
        }
    }
    lines.extend(discs);
    lines
}

/// Gizmo accent colour: the 2.0 palette's primary accent,
/// fixed for every event regardless of the event's own
/// colour, so the selection marker reads the same everywhere.
pub const GIZMO_COLOR: u32 = 0x6D8CFF;
/// Gizmo blend alpha (crosshair and ring alike).
const GIZMO_ALPHA: f32 = 0.9;
/// Crosshair half-arm length, in stage units.
const GIZMO_ARM: f32 = 12.0;
/// Radius of the ring of dots around the anchor, stage units.
const GIZMO_RING_R: f32 = 17.0;
/// Radius of each ring dot, in stage units.
const GIZMO_DOT_R: f32 = 2.2;
/// Number of dots in the ring.
const GIZMO_RING_DOTS: usize = 8;

/// Where a `project.particles` event is anchored, resolved
/// live: through `EditorCore::positions_f` at the display
/// frame — the same positions path `fx_geometry` uses — when
/// the event names a figure (node `None` means the figure's
/// root, node 0). When the event is free (`figure` is `None`)
/// or its anchor no longer resolves (figure deleted, node
/// out of range), the fallback is the event's stored stage
/// point `[x, y]` — for a free event that point *is* the
/// anchor; for a dangling one it is the creation-time joint
/// position `add_particle` captured, and exactly where the
/// rasterizer's `particles_at` would draw the event anyway.
fn event_anchor(core: &EditorCore, ev: &ParticleEvent) -> [f32; 2] {
    if let Some(fi) = ev.figure {
        if fi < core.project.figures.len() {
            let pos = core.positions_f(core.display_frame(), fi);
            if let Some(&p) = pos.get(ev.node.unwrap_or(0)) {
                return p;
            }
        }
    }
    [ev.x, ev.y]
}

/// Anchor gizmo for the `project.particles` event at
/// `clip_index`: a crosshair (two `Line` prims through the
/// anchor, half-arm [`GIZMO_ARM`]) plus a ring of
/// [`GIZMO_RING_DOTS`] `Disc` prims on a circle of radius
/// [`GIZMO_RING_R`] around it, all in the fixed accent
/// [`GIZMO_COLOR`] at alpha 0.9. Lines come before discs,
/// the same prim order `fx_geometry` returns. Stage space,
/// unscaled, like everything in this file — the consumer
/// applies its view transform. An out-of-range `clip_index`
/// returns an empty vec.
///
/// This is an **editor gizmo, not a render layer**, and two
/// choices follow from that:
///
/// - **No frame gating.** `particles_at` only draws an
///   event inside its live window `[frame, frame +
///   0.7 * fps]`; the gizmo ignores that window and shows
///   the anchor at *any* display frame. A marker that
///   vanished whenever you scrubbed away from the event's
///   frame could never be used to find, select or move the
///   event — locating it is the gizmo's whole job.
///   `fx_geometry` / `weather_geometry` remain the
///   frame-accurate layers.
/// - **Live anchor.** An anchored event is marked at the
///   joint's solved position *now* (via `positions_f`, so
///   graph channels apply, exactly as the stage draws the
///   figure), not at the frozen `[x, y]` the event captured
///   at creation and the rasterizer renders from. Scrubbing
///   or re-posing moves the marker with the joint, which is
///   what "where is this anchored" means while editing.
///
/// Further deltas from the render path, by the same logic:
/// a hidden figure's anchor still resolves (`positions_f`
/// is visibility-independent, unlike `fx_geometry`'s hidden
/// guard) — a hidden anchor is precisely when the marker is
/// needed.
pub fn gizmo_geometry(core: &EditorCore, clip_index: usize) -> Vec<ParticlePrim> {
    let Some(ev) = core.project.particles.get(clip_index) else {
        return Vec::new();
    };
    gizmo_prims_at(event_anchor(core, ev))
}

/// The gizmo marker itself, centred on `anchor`: a
/// crosshair (two `Line` prims through the anchor,
/// half-arm [`GIZMO_ARM`]) plus a ring of
/// [`GIZMO_RING_DOTS`] `Disc` prims on a circle of radius
/// [`GIZMO_RING_R`] around it, all in [`GIZMO_COLOR`] at
/// alpha [`GIZMO_ALPHA`]. Lines before discs, the order
/// `fx_geometry` returns. Shared by both gizmo builders
/// so the event marker and the FX clip marker can never
/// drift apart visually — they are siblings by
/// construction, not by convention.
fn gizmo_prims_at(anchor: [f32; 2]) -> Vec<ParticlePrim> {
    let [ax, ay] = anchor;
    let mut prims = Vec::with_capacity(2 + GIZMO_RING_DOTS);
    prims.push(ParticlePrim::Line {
        a: [ax - GIZMO_ARM, ay],
        b: [ax + GIZMO_ARM, ay],
        color: GIZMO_COLOR,
        alpha: GIZMO_ALPHA,
    });
    prims.push(ParticlePrim::Line {
        a: [ax, ay - GIZMO_ARM],
        b: [ax, ay + GIZMO_ARM],
        color: GIZMO_COLOR,
        alpha: GIZMO_ALPHA,
    });
    for i in 0..GIZMO_RING_DOTS {
        let a = i as f32 * std::f32::consts::TAU / GIZMO_RING_DOTS as f32;
        prims.push(ParticlePrim::Disc {
            pos: [ax + a.cos() * GIZMO_RING_R, ay + a.sin() * GIZMO_RING_R],
            radius: GIZMO_DOT_R,
            color: GIZMO_COLOR,
            alpha: GIZMO_ALPHA,
        });
    }
    prims
}

/// Where an FX clip entry fires from: the entry node's
/// world position via `EditorCore::positions_f` — the
/// same evaluation path [`event_anchor`] uses — but at
/// the clip's start frame rather than the display frame.
/// The start is rounded to the nearest authored frame and
/// clamped into the timeline exactly the way the editor
/// clamps its playhead (`min(frames.len() - 1)`,
/// saturating at zero), so an out-of-timeline start still
/// resolves to the nearest real pose instead of
/// extrapolating one. (`FxClipEntry.node` is a plain
/// `usize` in the model — node 0 is the root joint — and
/// an out-of-range node is clamped to the figure's last
/// joint, so a stale node id marks the figure's end
/// rather than losing the marker.) `None` when the anchor
/// cannot be evaluated at all: the entry's figure is out
/// of range — checked *before* the call, because
/// `positions_f` indexes the figure directly — or the
/// figure evaluates to no joints.
fn fx_clip_anchor(core: &EditorCore, entry: &FxClipEntry) -> Option<[f32; 2]> {
    if entry.figure >= core.project.figures.len() {
        return None;
    }
    let last = core.project.frames.len().saturating_sub(1) as f32;
    let start = entry.clip.start.round().clamp(0.0, last);
    let pos = core.positions_f(start, entry.figure);
    if pos.is_empty() {
        return None;
    }
    Some(pos[entry.node.min(pos.len() - 1)])
}

/// Anchor gizmo for the `project.fx_clips` entry at
/// `clip_index`: the identical marker [`gizmo_geometry`]
/// draws for a particle event — crosshair plus dot ring
/// in [`GIZMO_COLOR`] — centred on the joint the clip is
/// anchored to. Stage space, unscaled; the consumer
/// applies its view transform. An out-of-range
/// `clip_index`, an out-of-range figure, or an anchor
/// that cannot be evaluated returns an empty vec.
///
/// This is an **editor gizmo, not a render layer**, like
/// its sibling — but where the event gizmo resolves its
/// anchor *live* (at the display frame), the clip gizmo
/// evaluates at the clip's **start frame**: the marker
/// sits at the pose where the effect FIRES, so it is
/// visible from any playhead position. That is
/// deliberate — it answers "where will this effect
/// appear?", which is the question a selection marker
/// for a timed effect has to answer; scrubbing to the
/// clip and reading its live anchor from `fx_geometry`'s
/// motes remains the frame-accurate view. Two further
/// choices mirror the event gizmo's documented logic: a
/// hidden figure's anchor still resolves (a hidden
/// anchor is precisely when the marker is needed), and
/// the anchor goes through `positions_f`, so graph
/// channels apply exactly as the stage draws the figure.
pub fn fx_clip_gizmo_geometry(core: &EditorCore, clip_index: usize) -> Vec<ParticlePrim> {
    let Some(entry) = core.project.fx_clips.get(clip_index) else {
        return Vec::new();
    };
    let Some(anchor) = fx_clip_anchor(core, entry) else {
        return Vec::new();
    };
    gizmo_prims_at(anchor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use stykanimate_rs::model::{FxClipEntry, ParticleEvent, ParticleKind};
    use stykanimate_rs::vfx::{ParticleClip, Weather, WeatherKind};

    fn core_with_weather(kind: WeatherKind, amount: f32, wind: f32) -> EditorCore {
        let mut core = EditorCore::new();
        core.project.weather = Some(Weather { kind, amount, wind });
        core
    }

    fn burst_entry(start: f32, duration: f32) -> FxClipEntry {
        FxClipEntry {
            clip: ParticleClip {
                start,
                duration,
                kind: ClipKind::Burst,
                color: 0xFFC65D,
                color2: None,
                seed: 777,
                angle: 0.0,
                length: 0.0,
            },
            figure: 0,
            node: 1,
        }
    }

    #[test]
    fn weather_off_is_empty() {
        let mut core = EditorCore::new();
        core.project.weather = None;
        assert!(weather_geometry(&core).is_empty());
        // A weather entry whose amount rounds to zero is
        // also an empty field.
        let core = core_with_weather(WeatherKind::Rain, 0.0, 0.0);
        assert!(weather_geometry(&core).is_empty());
    }

    #[test]
    fn rain_count_scales_with_amount() {
        let few = weather_geometry(&core_with_weather(WeatherKind::Rain, 40.0, 10.0));
        assert_eq!(few.len(), 40);
        let many = weather_geometry(&core_with_weather(WeatherKind::Rain, 120.0, 10.0));
        assert_eq!(many.len(), 120);
        // Rain renders as discs inside the stage bounds (the
        // field wraps on the stage torus).
        let stage = EditorCore::new().project.stage;
        for prim in few.iter().chain(many.iter()) {
            let ParticlePrim::Disc { pos, .. } = prim else {
                panic!("rain must be discs, got {prim:?}");
            };
            assert!(pos[0] >= 0.0 && pos[0] < stage[0], "x out of stage: {pos:?}");
            assert!(pos[1] >= 0.0 && pos[1] < stage[1], "y out of stage: {pos:?}");
        }
    }

    #[test]
    fn storm_scales_amount_by_1_8() {
        let mut core = core_with_weather(WeatherKind::Storm, 100.0, 20.0);
        core.goto(3); // frame 3: no lightning disc (3 mod 47 > 1)
        let prims = weather_geometry(&core);
        assert_eq!(prims.len(), 180);
    }

    #[test]
    fn weather_is_deterministic_at_the_same_frame() {
        let mut core = core_with_weather(WeatherKind::Snow, 60.0, -15.0);
        assert_eq!(weather_geometry(&core), weather_geometry(&core));
        core.goto(3);
        assert_eq!(weather_geometry(&core), weather_geometry(&core));
        // And a different frame really is a different field.
        let f3 = weather_geometry(&core);
        core.goto(4);
        assert_ne!(f3, weather_geometry(&core));
    }

    #[test]
    fn fx_outside_its_window_is_empty() {
        let mut core = EditorCore::new();
        core.project.fx_clips.push(burst_entry(10.0, 5.0));
        assert!(fx_geometry(&core).is_empty(), "frame 0 is before the window");
        core.goto(2);
        assert!(fx_geometry(&core).is_empty(), "frame 2 is still before it");
    }

    #[test]
    fn fx_inside_its_window_emits_the_burst() {
        let mut core = EditorCore::new();
        core.project.fx_clips.push(burst_entry(0.0, 4.0));
        let prims = fx_geometry(&core);
        // Burst emits 26 motes; at local time 0 every mote
        // sits exactly on the anchor (ease-out reach is 0).
        assert_eq!(prims.len(), 26);
        let anchor = core.positions_f(0.0, 0)[1];
        for prim in &prims {
            let ParticlePrim::Disc { pos, color, .. } = prim else {
                panic!("burst must be discs, got {prim:?}");
            };
            assert_eq!(*pos, anchor);
            assert_eq!(*color, 0xFFC65D);
        }
    }

    #[test]
    fn fx_skips_hidden_figures_and_bad_indices() {
        // Hidden figure: rasterizer parity (export.rs guard).
        let mut core = EditorCore::new();
        core.project.fx_clips.push(burst_entry(0.0, 4.0));
        core.project.figures[0].hidden = true;
        assert!(fx_geometry(&core).is_empty());

        // Out-of-range figure and node indices: skipped, no panic.
        let mut core = EditorCore::new();
        let mut bad_fig = burst_entry(0.0, 4.0);
        bad_fig.figure = 99;
        core.project.fx_clips.push(bad_fig);
        let mut bad_node = burst_entry(0.0, 4.0);
        bad_node.node = 999;
        core.project.fx_clips.push(bad_node);
        assert!(fx_geometry(&core).is_empty());
    }

    #[test]
    fn fx_is_deterministic_at_the_same_frame() {
        let mut core = EditorCore::new();
        core.project.fx_clips.push(burst_entry(0.0, 4.0));
        core.goto(2);
        assert_eq!(fx_geometry(&core), fx_geometry(&core));
    }

    fn free_event(x: f32, y: f32) -> ParticleEvent {
        ParticleEvent {
            frame: 0,
            figure: None,
            node: None,
            x,
            y,
            kind: ParticleKind::Sparkle,
            color: 0xFF5D5D,
            seed: 5,
        }
    }

    /// The full marker contract: 2 crosshair lines through
    /// `centre` (one horizontal, one vertical, each
    /// `2 * GIZMO_ARM` long), then `GIZMO_RING_DOTS` discs on
    /// the ring around it — everything in `GIZMO_COLOR`.
    fn assert_gizmo_at(prims: &[ParticlePrim], centre: [f32; 2]) {
        assert_eq!(prims.len(), 2 + GIZMO_RING_DOTS, "crosshair + ring: {prims:?}");
        let (lines, dots) = prims.split_at(2);
        let mut saw_horizontal = false;
        let mut saw_vertical = false;
        for prim in lines {
            let ParticlePrim::Line { a, b, color, .. } = prim else {
                panic!("crosshair arms must be lines, got {prim:?}");
            };
            assert_eq!(*color, GIZMO_COLOR);
            let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
            assert_eq!(mid, centre, "crosshair arm not centred on the anchor");
            if a[1] == b[1] {
                saw_horizontal = true;
                assert_eq!((b[0] - a[0]).abs(), 2.0 * GIZMO_ARM);
            } else {
                saw_vertical = true;
                assert_eq!((b[1] - a[1]).abs(), 2.0 * GIZMO_ARM);
            }
        }
        assert!(saw_horizontal && saw_vertical, "one arm per axis: {lines:?}");
        for prim in dots {
            let ParticlePrim::Disc { pos, radius, color, .. } = prim else {
                panic!("ring must be discs, got {prim:?}");
            };
            assert_eq!(*color, GIZMO_COLOR);
            assert_eq!(*radius, GIZMO_DOT_R);
            let d = ((pos[0] - centre[0]).powi(2) + (pos[1] - centre[1]).powi(2)).sqrt();
            assert!((d - GIZMO_RING_R).abs() < 0.01, "dot off the ring: {pos:?}");
        }
    }

    #[test]
    fn gizmo_missing_index_is_empty() {
        let core = EditorCore::new();
        assert!(gizmo_geometry(&core, 0).is_empty());
        let mut core = EditorCore::new();
        core.project.particles.push(free_event(10.0, 10.0));
        assert!(gizmo_geometry(&core, 1).is_empty());
        assert!(!gizmo_geometry(&core, 0).is_empty());
    }

    #[test]
    fn gizmo_anchored_event_marks_the_solved_joint() {
        let mut core = EditorCore::new();
        // The stored x/y is deliberately wrong: the gizmo
        // must follow the live anchor (the joint's solved
        // position via positions_f), not the creation-time
        // snapshot the rasterizer renders from.
        let mut ev = free_event(-999.0, -999.0);
        ev.figure = Some(0);
        ev.node = Some(1);
        core.project.particles.push(ev);
        let anchor = core.positions_f(core.display_frame(), 0)[1];
        assert_gizmo_at(&gizmo_geometry(&core, 0), anchor);
    }

    #[test]
    fn gizmo_free_event_marks_its_stage_point() {
        let mut core = EditorCore::new();
        core.project.particles.push(free_event(123.0, 456.0));
        assert_gizmo_at(&gizmo_geometry(&core, 0), [123.0, 456.0]);
    }

    #[test]
    fn gizmo_ignores_the_events_live_window() {
        // The event fires at frame 40 while the display frame
        // is 0 — long outside its [frame, frame + 0.7*fps]
        // render window. The gizmo shows the anchor anyway:
        // locating the event is its job (see the fn docs).
        let mut core = EditorCore::new();
        let mut ev = free_event(50.0, 60.0);
        ev.frame = 40;
        core.project.particles.push(ev);
        assert_gizmo_at(&gizmo_geometry(&core, 0), [50.0, 60.0]);
    }

    #[test]
    fn gizmo_dangling_anchor_falls_back_to_the_stored_point() {
        let mut core = EditorCore::new();
        let mut ev = free_event(70.0, 80.0);
        ev.figure = Some(99);
        ev.node = Some(3);
        core.project.particles.push(ev);
        assert_gizmo_at(&gizmo_geometry(&core, 0), [70.0, 80.0]);
    }

    #[test]
    fn fx_gizmo_missing_index_is_empty() {
        let core = EditorCore::new();
        assert!(fx_clip_gizmo_geometry(&core, 0).is_empty());
        let mut core = EditorCore::new();
        core.project.fx_clips.push(burst_entry(0.0, 4.0));
        assert!(fx_clip_gizmo_geometry(&core, 1).is_empty());
        assert!(!fx_clip_gizmo_geometry(&core, 0).is_empty());
    }

    #[test]
    fn fx_gizmo_bad_figure_is_empty() {
        // The guard matters beyond tidiness: positions_f
        // indexes the figure directly, so an out-of-range
        // figure must be refused before the call, never
        // reached.
        let mut core = EditorCore::new();
        let mut bad = burst_entry(0.0, 4.0);
        bad.figure = 99;
        core.project.fx_clips.push(bad);
        assert!(fx_clip_gizmo_geometry(&core, 0).is_empty());
    }

    #[test]
    fn fx_gizmo_marks_the_anchor_at_the_clips_start_frame() {
        // The playhead sits on frame 0, but the clip fires
        // at frame 2: the marker belongs to the pose where
        // the effect fires, evaluated at the start frame
        // through the same positions_f path the production
        // geometry uses — not to wherever the joint is now.
        let mut core = EditorCore::new();
        core.project.fx_clips.push(burst_entry(2.0, 4.0));
        assert_eq!(core.display_frame(), 0.0);
        let anchor = core.positions_f(2.0, 0)[1];
        assert_gizmo_at(&fx_clip_gizmo_geometry(&core, 0), anchor);
    }

    #[test]
    fn fx_gizmo_start_past_the_timeline_clamps_to_the_last_frame() {
        let mut core = EditorCore::new();
        core.project.fx_clips.push(burst_entry(100.0, 4.0));
        let last = (core.project.frames.len() - 1) as f32;
        let anchor = core.positions_f(last, 0)[1];
        assert_gizmo_at(&fx_clip_gizmo_geometry(&core, 0), anchor);
    }

    #[test]
    fn fx_gizmo_node_zero_anchors_at_the_root_joint() {
        // FxClipEntry.node is a plain index (no Option in
        // the model); node 0 is the figure's root joint.
        let mut core = EditorCore::new();
        let mut entry = burst_entry(1.0, 4.0);
        entry.node = 0;
        core.project.fx_clips.push(entry);
        let anchor = core.positions_f(1.0, 0)[0];
        assert_gizmo_at(&fx_clip_gizmo_geometry(&core, 0), anchor);
    }

    #[test]
    fn fx_gizmo_out_of_range_node_clamps_to_the_last_joint() {
        let mut core = EditorCore::new();
        let mut entry = burst_entry(0.0, 4.0);
        entry.node = 999;
        core.project.fx_clips.push(entry);
        let pos = core.positions_f(0.0, 0);
        let anchor = pos[pos.len() - 1];
        assert_gizmo_at(&fx_clip_gizmo_geometry(&core, 0), anchor);
    }
}
