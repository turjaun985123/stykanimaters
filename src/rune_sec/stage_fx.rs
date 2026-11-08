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

use stykanimate_rs::editor::EditorCore;
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

#[cfg(test)]
mod tests {
    use super::*;
    use stykanimate_rs::model::FxClipEntry;
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
}
