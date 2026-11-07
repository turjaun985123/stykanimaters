//! Extended visual effects: timed particle *clips*.
//!
//! Where `export::particles_at` handles simple one-shot events
//! (burst / sparkle / smoke anchored to a stored stage point), a
//! [`ParticleClip`] is a longer, reusable effect with an explicit
//! start frame and duration in frames. The caller supplies the
//! anchor point for each frame (typically a solved joint position),
//! so this module stays rig-agnostic: it never reads the project.
//!
//! Everything is closed-form in the frame time and the clip seed —
//! no state, no accumulation — so the same clip at the same frame
//! always produces identical particles, in the editor and in every
//! export, just like the simpler events.
//!
//! Angles follow the model convention: degrees, 0° pointing down,
//! direction `(sin θ, cos θ)`.

use crate::model::V2;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ClipKind {
    /// Radial explosion of sparks with drag and a touch of gravity.
    Burst,
    /// Small twinkling motes drifting gently upward.
    Sparkle,
    /// Slow, expanding puffs that rise and fade.
    Smoke,
    /// A streaming ray from the anchor along `angle`, out to
    /// `length`, brightest near the source.
    Beam,
    /// A soft ring of motes around the anchor, radius pulsing
    /// between 26 and 34.
    Aura,
    /// A fading chain of ghosts trailing behind the anchor,
    /// opposite `angle`, spaced `length / 8` apart.
    Trail,
    /// A celebratory pop: multi-coloured pieces that arc up,
    /// flutter, and fall back under gravity.
    Confetti,
    /// An expanding ring of motes: the ring radius grows linearly
    /// to `length` while the motes fade out.
    ShockRing,
    /// Glowing embers born together at the clip's start: they rise
    /// with an ease-out (fast launch, decelerating climb), wobble
    /// on a sine, and shrink and fade in warm orange shades derived
    /// from the clip colour.
    Ember,
    /// Ballistic droplets launched from the anchor along `angle`
    /// with a seeded spread and speed, pulled back down by
    /// gravity; each droplet fades over its own arc and vanishes
    /// when it lands back at the anchor's height.
    Fountain,
    /// Motes circling the anchor on three concentric rings (radii
    /// are fractions of `length`): inner rings run faster, the
    /// middle ring runs backwards, and each mote drags two fainter
    /// ghosts behind it.
    Orbit,
    /// A jagged lightning bolt from the anchor along `angle`,
    /// reaching `length` (140 when unset): dots along a seeded
    /// zigzag polyline whose sideways jitter is re-seeded every
    /// frame, so the bolt re-strikes constantly while staying a
    /// pure function of the frame time. Brightest at the strike's
    /// start and near the anchor, with a seeded per-frame flicker.
    Lightning,
    /// Bubbles that rise from the anchor on individual life
    /// cycles: each wobbles as it climbs, grows slightly, and
    /// pops — its alpha collapses across the last 10% of its
    /// life.
    Bubbles,
    /// Leaves shed from the anchor: they fall on individual
    /// cycles, fluttering with a slow sideways sway and a slow
    /// apparent-width oscillation, in green shades derived from
    /// the clip colour.
    Leaves,
    /// A white-hot head streaking along `angle` for `length`
    /// (240 when unset), accelerating slightly, dragging a trail
    /// of sparks at fixed lags behind it: each spark appears when
    /// the head passes its lag and fades as the lag grows, so the
    /// trail stretches exactly like a real meteor's.
    Meteor,
    /// Petals shed continuously: each petal runs its own falling
    /// cycle (fract, like Beam's stream), drifting down across a
    /// wide spread on a gentle sine sway, tumbling slowly, in
    /// pink shades derived from the clip colour.
    Petals,
    /// A single upward burst: droplets leave together in an even
    /// fan across a ±50° cone around `angle` (seeded speeds, not
    /// seeded angles — the even fan reads as a crown splash),
    /// arc under strong gravity, and vanish when they fall back
    /// to the anchor's height.
    Splash,
    /// Motes streaming outward on three spiral arms: each mote
    /// cycles from the anchor to `length` (90 when unset) at its
    /// own pace while the pattern rotates, and its angle winds
    /// by an extra twist across the cycle, bending the three
    /// spokes into arms.
    Spiral,
    /// A single shell bursting at the anchor: seeded stars
    /// radiate in every direction, decelerate under drag, sag
    /// under gravity and twinkle out by the clip's end, in a
    /// small palette derived from the clip colour (the colour
    /// itself, a white-hot pull, and its channel rotation).
    Fireworks,
    /// Slow curtains of light above the anchor: motes in three
    /// horizontal sine-wave bands whose crests travel sideways,
    /// so the ribbons ripple and drift, in three lightnesses of
    /// the clip colour (palest on the highest band).
    Aurora,
    /// A large bright head gliding along `angle` at a steady
    /// pace out to `length` (200 when unset), dragging a smooth,
    /// wide taper of softer tail motes at fixed lags behind it —
    /// Meteor's bigger, statelier cousin.
    Comet,
    /// A rotating three-arm spiral galaxy: every star keeps its
    /// seeded place along its arm while the whole pattern turns
    /// and its radius grows across the clip, so the galaxy
    /// blooms outward as it spins — Spiral's motes stream and
    /// cycle; Galaxy's hold formation. Each arm carries its own
    /// channel rotation of the clip colour.
    Galaxy,
    /// A rainbow fan above the anchor: motes pinned along an
    /// arc of radius `length` (70 when unset) that sweeps
    /// gently, coloured by a channel rotation of the clip
    /// colour that steps along the arc and cycles with time,
    /// so bands of hue drift around the fan.
    Prism,
    /// A one-shot inward spiral: every mote starts near the
    /// rim at the clip's start and winds into the anchor,
    /// turning faster as it falls, shrinking and fading as it
    /// arrives, until the whole vortex has been swallowed.
    Vortex,
}

/// A timed particle effect. `start` and `duration` are in frames;
/// the clip is live for `start <= frame < start + duration`.
/// `angle` (degrees, model convention) aims Beam and Trail;
/// `length` is the beam's reach and the trail's total span.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ParticleClip {
    pub start: f32,
    pub duration: f32,
    pub kind: ClipKind,
    pub color: u32,
    pub seed: u32,
    pub angle: f32,
    pub length: f32,
}

/// Deterministic per-particle random in [0, 1) from a seed + index.
/// (Same xorshift the simple events in `export.rs` use.)
fn prand(seed: u32, i: u32, salt: u32) -> f32 {
    let mut h = seed ^ i.wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7FEB_352D);
    h ^= h >> 15;
    (h % 100_000) as f32 / 100_000.0
}

/// Cycle a colour's channels (r,g,b) -> (g,b,r) -> (b,r,g).
/// A cheap hue-ish rotation used to build the confetti palette
/// from a clip's single base colour.
fn rotate_channels(c: [u8; 3], k: u32) -> [u8; 3] {
    match k % 3 {
        0 => c,
        1 => [c[1], c[2], c[0]],
        _ => [c[2], c[0], c[1]],
    }
}

fn rgb(hex: u32) -> [u8; 3] {
    [
        ((hex >> 16) & 0xFF) as u8,
        ((hex >> 8) & 0xFF) as u8,
        (hex & 0xFF) as u8,
    ]
}

/// Unit direction for an angle in degrees, model convention:
/// 0° = down, positive = clockwise, `dir = (sin θ, cos θ)`.
fn dir_from_deg(deg: f32) -> V2 {
    let r = deg.to_radians();
    [r.sin(), r.cos()]
}

/// Live particles of `clip` at fractional frame time `frame_f`,
/// emitted around `anchor`: (centre, radius, colour, alpha).
/// Empty when `frame_f` is outside the clip's window
/// `[start, start + duration)` or the duration is not positive.
pub fn clip_particles(
    clip: &ParticleClip,
    frame_f: f32,
    anchor: [f32; 2],
) -> Vec<([f32; 2], f32, [u8; 3], f32)> {
    let mut out = Vec::new();
    if clip.duration <= 0.0 {
        return out;
    }
    let local = frame_f - clip.start;
    if local < 0.0 || local >= clip.duration {
        return out;
    }
    let t = local / clip.duration; // 0..1 through the clip
    let color = rgb(clip.color);
    let seed = clip.seed;
    // Ramp shared by the sustained effects: 0 at both ends, 1 mid-clip.
    let envelope = (std::f32::consts::PI * t).sin();

    let push = |out: &mut Vec<([f32; 2], f32, [u8; 3], f32)>, pos: V2, size: f32, alpha: f32| {
        if alpha > 0.01 {
            out.push((pos, size.max(0.4), color, alpha.clamp(0.0, 1.0)));
        }
    };
    // Same, with a per-particle colour (confetti's palette).
    let push_c = |out: &mut Vec<([f32; 2], f32, [u8; 3], f32)>,
                  pos: V2,
                  size: f32,
                  alpha: f32,
                  col: [u8; 3]| {
        if alpha > 0.01 {
            out.push((pos, size.max(0.4), col, alpha.clamp(0.0, 1.0)));
        }
    };

    match clip.kind {
        ClipKind::Burst => {
            for i in 0..26 {
                let a = prand(seed, i, 1) * std::f32::consts::TAU;
                let reach = 30.0 + prand(seed, i, 2) * 90.0;
                // Ease-out expansion (drag), then gravity sag.
                let d = reach * (1.0 - (1.0 - t) * (1.0 - t));
                let pos = [
                    anchor[0] + a.cos() * d,
                    anchor[1] + a.sin() * d + 22.0 * t * t,
                ];
                push(&mut out, pos, 3.2 * (1.0 - 0.6 * t), 1.0 - t);
            }
        }
        ClipKind::Sparkle => {
            for i in 0..14 {
                let a = prand(seed, i, 1) * std::f32::consts::TAU;
                let d = 8.0 + prand(seed, i, 2) * 26.0;
                let twinkle = 0.55 + 0.45 * (local * 1.7 + i as f32 * 2.3).sin();
                let pos = [anchor[0] + a.cos() * d, anchor[1] + a.sin() * d - 12.0 * t];
                push(&mut out, pos, 2.6 * twinkle.max(0.2), (1.0 - t) * twinkle);
            }
        }
        ClipKind::Smoke => {
            for i in 0..12 {
                let a = prand(seed, i, 1) * std::f32::consts::TAU;
                let d = 4.0 + prand(seed, i, 2) * 16.0;
                let pos = [
                    anchor[0] + a.cos() * d + 4.0 * t,
                    anchor[1] - 20.0 * t + a.sin() * d * 0.4,
                ];
                push(&mut out, pos, 5.0 + 10.0 * t, 0.32 * (1.0 - t));
            }
        }
        ClipKind::Beam => {
            let dir = dir_from_deg(clip.angle);
            let perp = [-dir[1], dir[0]];
            for i in 0..30 {
                // Stream outward: each mote cycles along the ray as
                // the clip plays, at its own pace.
                let pace = 0.5 + prand(seed, i, 2);
                let s = (prand(seed, i, 1) + local * 0.05 * pace).fract();
                let jitter = (prand(seed, i, 3) - 0.5) * 5.0 * (0.4 + 0.6 * s);
                let along = s * clip.length.max(0.0);
                let pos = [
                    anchor[0] + dir[0] * along + perp[0] * jitter,
                    anchor[1] + dir[1] * along + perp[1] * jitter,
                ];
                // Brighter (and slightly larger) near the source.
                let alpha = envelope * (1.0 - 0.75 * s);
                push(&mut out, pos, 3.4 * (1.0 - 0.5 * s), alpha);
            }
        }
        ClipKind::Aura => {
            // Ring radius pulses 26..34 over the clip.
            let ring = 30.0 + 4.0 * (std::f32::consts::TAU * t).sin();
            for i in 0..24 {
                let a = (i as f32 / 24.0) * std::f32::consts::TAU
                    + t * 1.2
                    + (prand(seed, i, 1) - 0.5) * 0.2;
                let r = ring + (prand(seed, i, 2) - 0.5) * 3.0;
                let pos = [anchor[0] + a.cos() * r, anchor[1] + a.sin() * r];
                let alpha = 0.35 * envelope * (0.6 + 0.4 * prand(seed, i, 3));
                push(&mut out, pos, 3.0, alpha);
            }
        }
        ClipKind::Trail => {
            let dir = dir_from_deg(clip.angle);
            let perp = [-dir[1], dir[0]];
            let spacing = clip.length.max(0.0) / 8.0;
            for k in 1..=8u32 {
                let back = k as f32 * spacing;
                let jitter = (prand(seed, k, 1) - 0.5) * 1.5;
                let pos = [
                    anchor[0] - dir[0] * back + perp[0] * jitter,
                    anchor[1] - dir[1] * back + perp[1] * jitter,
                ];
                // Older ghosts (further back) are fainter; the whole
                // chain fades as the clip ends.
                let alpha = (1.0 - k as f32 / 9.0) * (1.0 - t) * 0.55;
                push(&mut out, pos, 5.5 * (1.0 - 0.05 * k as f32), alpha);
            }
        }
        ClipKind::Confetti => {
            // Six-piece palette: the clip colour under three channel
            // rotations, at full and 65% brightness.
            let palette: [[u8; 3]; 6] = std::array::from_fn(|k| {
                let rot = rotate_channels(color, k as u32);
                let f = if k < 3 { 1.0 } else { 0.65 };
                [
                    (rot[0] as f32 * f) as u8,
                    (rot[1] as f32 * f) as u8,
                    (rot[2] as f32 * f) as u8,
                ]
            });
            for i in 0..28 {
                let a = prand(seed, i, 1) * std::f32::consts::TAU;
                let reach = 26.0 + prand(seed, i, 2) * 84.0;
                let ease = 1.0 - (1.0 - t) * (1.0 - t);
                let arc = 46.0 + prand(seed, i, 3) * 64.0;
                // Sideways flutter that grows as the piece falls.
                let flutter =
                    7.0 * (local * 0.9 + prand(seed, i, 4) * std::f32::consts::TAU).sin() * t;
                let pos = [
                    anchor[0] + a.cos() * reach * ease + flutter,
                    anchor[1] + a.sin() * reach * ease * 0.6 + arc * (2.0 * t * t - t) * 2.0,
                ];
                // Pieces tumble: apparent width oscillates.
                let spin = 0.55
                    + 0.45 * (local * 1.3 + prand(seed, i, 5) * std::f32::consts::TAU)
                        .sin()
                        .abs();
                let alpha = (1.0 - t * t) * (0.75 + 0.25 * prand(seed, i, 6));
                push_c(&mut out, pos, 3.0 * spin, alpha, palette[(i % 6) as usize]);
            }
        }
        ClipKind::ShockRing => {
            // One clean expanding ring: radius = t * length, fading.
            let radius = t * clip.length.max(0.0);
            for k in 0..36u32 {
                let a = (k as f32 / 36.0) * std::f32::consts::TAU
                    + (prand(seed, k, 1) - 0.5) * 0.06;
                let r = (radius + (prand(seed, k, 2) - 0.5) * 4.0).max(0.0);
                let pos = [anchor[0] + a.cos() * r, anchor[1] + a.sin() * r];
                push(&mut out, pos, 3.2 * (1.0 - 0.45 * t), (1.0 - t) * 0.9);
            }
        }
        ClipKind::Ember => {
            // All embers are born at the clip's start. Each one's
            // age `s` runs 0..1 at its own pace; the rise eases out
            // (1 - (1-s)^2), so the climb decelerates as the ember
            // cools, shrinks and fades. Alpha stays well above the
            // 0.01 emitter cutoff for the whole clip (envelope *
            // 0.35 * flicker >= ~0.05), so the particle list is
            // index-stable at every frame.
            for i in 0..22u32 {
                let speed = 1.0 + 0.6 * prand(seed, i, 5);
                let s = (t * speed).clamp(0.0, 1.0);
                let height = 55.0 + prand(seed, i, 2) * 85.0;
                let rise = height * (1.0 - (1.0 - s) * (1.0 - s));
                let sway = (2.5 + 4.5 * s)
                    * (s * std::f32::consts::TAU * (1.0 + prand(seed, i, 4))
                        + prand(seed, i, 6) * std::f32::consts::TAU)
                        .sin();
                let pos = [
                    anchor[0] + (prand(seed, i, 1) - 0.5) * 14.0 + sway,
                    anchor[1] - rise,
                ];
                let flicker = 0.8 + 0.2 * (local * 2.6 + i as f32 * 1.7).sin();
                let alpha = envelope * (0.35 + 0.65 * (1.0 - s)) * flicker;
                // Warm palette from the clip colour: the colour
                // itself, a red-hot shift, a yellow-hot shift, and
                // a dark coal.
                let col = match i % 4 {
                    0 => color,
                    1 => [
                        color[0],
                        (color[1] as f32 * 0.5) as u8,
                        (color[2] as f32 * 0.2) as u8,
                    ],
                    2 => [
                        color[0],
                        (color[1] as f32 * 1.25).min(255.0) as u8,
                        (color[2] as f32 * 0.45) as u8,
                    ],
                    _ => [
                        (color[0] as f32 * 0.75) as u8,
                        (color[1] as f32 * 0.32) as u8,
                        (color[2] as f32 * 0.12) as u8,
                    ],
                };
                push_c(&mut out, pos, 3.4 * (1.0 - 0.65 * s), alpha, col);
            }
        }
        ClipKind::Fountain => {
            // Droplets in free flight: position is the ballistic
            // integral of the launch velocity plus gravity, all in
            // per-frame units. A droplet's `land` is when its arc
            // returns to the anchor's height; it fades across its
            // own flight and is gone once it lands.
            let g = 1.3;
            for i in 0..24u32 {
                let dir = dir_from_deg(clip.angle + (prand(seed, i, 1) - 0.5) * 56.0);
                let v = 7.5 + prand(seed, i, 2) * 3.0;
                let land = if dir[1] < -0.05 {
                    2.0 * v * (-dir[1]) / g
                } else {
                    clip.duration
                };
                let age = if land > 0.0 { local / land } else { 1.0 };
                if age >= 1.0 {
                    continue;
                }
                let pos = [
                    anchor[0] + dir[0] * v * local,
                    anchor[1] + dir[1] * v * local + 0.5 * g * local * local,
                ];
                push(&mut out, pos, 2.6 * (1.0 - 0.3 * age), 0.9 * (1.0 - age));
            }
        }
        ClipKind::Orbit => {
            // Ring assignment, radius, speed and phase all come
            // from the index/seed, so a mote stays on its ring for
            // the whole clip. Ghosts share the mote's radius and
            // lag it by a fixed angle against its direction of
            // travel, with trailing alpha.
            let base = if clip.length > 0.0 { clip.length } else { 60.0 };
            for i in 0..18u32 {
                let ring = (i % 3) as usize;
                let radius = base * [0.45, 0.7, 0.95][ring];
                let speed = (0.11 - 0.03 * ring as f32) * if ring == 1 { -1.0 } else { 1.0 };
                let theta = prand(seed, i, 1) * std::f32::consts::TAU + local * speed;
                let bright = 0.65 + 0.35 * prand(seed, i, 2);
                for (lag, fade) in [(0.0f32, 1.0f32), (0.16, 0.55), (0.32, 0.28)] {
                    let a = theta - speed.signum() * lag;
                    let pos = [anchor[0] + a.cos() * radius, anchor[1] + a.sin() * radius];
                    push(&mut out, pos, 3.0, envelope * 0.85 * bright * fade);
                }
            }
        }
        ClipKind::Lightning => {
            // The bolt is a polyline of SEG segments from the
            // anchor along `angle`. Vertex jitter is seeded by
            // (seed, vertex, frame index), so each frame strikes a
            // fresh bolt while remaining a pure function of
            // `frame_f`. The first vertex is pinned to the anchor.
            let dir = dir_from_deg(clip.angle);
            let perp = [-dir[1], dir[0]];
            let reach = if clip.length > 0.0 { clip.length } else { 140.0 };
            const SEG: u32 = 7;
            let strike = local.floor().max(0.0) as u32;
            let salt = 100u32.wrapping_add(strike.wrapping_mul(31));
            // White-hot core: the clip colour pulled toward white.
            let core = [
                color[0] + ((255 - color[0] as u16) * 65 / 100) as u8,
                color[1] + ((255 - color[1] as u16) * 65 / 100) as u8,
                color[2] + ((255 - color[2] as u16) * 65 / 100) as u8,
            ];
            let mut verts: Vec<V2> = Vec::with_capacity((SEG + 1) as usize);
            for k in 0..=SEG {
                let s = k as f32 / SEG as f32;
                let off = if k == 0 {
                    0.0
                } else {
                    (prand(seed, k, salt) - 0.5) * 26.0 * (0.35 + 0.65 * s)
                };
                verts.push([
                    anchor[0] + dir[0] * reach * s + perp[0] * off,
                    anchor[1] + dir[1] * reach * s + perp[1] * off,
                ]);
            }
            // Brightest just after the strike begins; the seeded
            // flicker varies the whole bolt's intensity per frame.
            let flicker = 0.6 + 0.4 * prand(seed, strike, 77);
            let fade = (1.0 - 0.55 * t) * flicker;
            for k in 0..SEG {
                let (a, b) = (verts[k as usize], verts[k as usize + 1]);
                for j in 0..4 {
                    let f = j as f32 / 4.0;
                    let pos = [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f];
                    let s = (k as f32 + f) / SEG as f32;
                    // The tail of the bolt is dimmer than its head.
                    push_c(&mut out, pos, 2.8 * (1.0 - 0.4 * s), fade * (1.0 - 0.45 * s), core);
                }
            }
            push_c(&mut out, verts[SEG as usize], 3.4, fade * 0.55, core);
        }
        ClipKind::Bubbles => {
            // Each bubble runs its own life cycle `s` at its own
            // pace: it fades in over the first 10% of its life,
            // grows slightly as it climbs, and pops — alpha
            // collapses across the last 10%, with a final swell.
            let pale = [
                color[0] + ((255 - color[0] as u16) * 45 / 100) as u8,
                color[1] + ((255 - color[1] as u16) * 45 / 100) as u8,
                color[2] + ((255 - color[2] as u16) * 45 / 100) as u8,
            ];
            for i in 0..16u32 {
                let speed = 0.85 + prand(seed, i, 5) * 0.4;
                let s = (t * speed).clamp(0.0, 1.0);
                let height = 70.0 + prand(seed, i, 2) * 90.0;
                let wobble =
                    (2.0 + 3.0 * s) * (local * 1.1 + prand(seed, i, 4) * std::f32::consts::TAU).sin();
                let pos = [
                    anchor[0] + (prand(seed, i, 1) - 0.5) * 20.0 + wobble,
                    anchor[1] - height * s,
                ];
                let fade_in = (s / 0.1).min(1.0);
                let pop = if s > 0.9 { (1.0 - s) / 0.1 } else { 1.0 };
                let bright = 0.7 + 0.3 * prand(seed, i, 6);
                let swell = if s > 0.9 { 1.0 + 0.3 * ((s - 0.9) / 0.1) } else { 1.0 };
                push_c(
                    &mut out,
                    pos,
                    (2.2 + 1.9 * s) * swell,
                    0.8 * fade_in * pop * bright,
                    pale,
                );
            }
        }
        ClipKind::Leaves => {
            // Three leafy shades from the clip colour: the green
            // channel dominates in all of them.
            let g = color[1].max(120);
            let palette: [[u8; 3]; 3] = [
                [(color[0] as f32 * 0.35) as u8, g, (color[2] as f32 * 0.3) as u8],
                [
                    (color[0] as f32 * 0.5) as u8 + 30,
                    (g as u16 + 40).min(255) as u8,
                    (color[2] as f32 * 0.4) as u8 + 10,
                ],
                [
                    (color[0] as f32 * 0.28) as u8,
                    (g as f32 * 0.62) as u8,
                    (color[2] as f32 * 0.22) as u8,
                ],
            ];
            for i in 0..16u32 {
                let speed = 0.8 + prand(seed, i, 5) * 0.45;
                let s = (t * speed).clamp(0.0, 1.0);
                let fall = 80.0 + prand(seed, i, 2) * 80.0;
                let phase = prand(seed, i, 4) * std::f32::consts::TAU;
                // Slow flutter: sideways sway that widens as the
                // leaf falls, plus a gentle vertical bob.
                let sway = (6.0 + 10.0 * s) * (local * 0.5 + phase).sin();
                let pos = [
                    anchor[0] + (prand(seed, i, 1) - 0.5) * 30.0 + sway,
                    anchor[1] + fall * s + 6.0 * (local * 0.8 + phase).sin(),
                ];
                // Tumbling like confetti but slower (0.6 vs 1.3).
                let spin =
                    0.5 + 0.5 * (local * 0.6 + prand(seed, i, 6) * std::f32::consts::TAU).sin().abs();
                let alpha = envelope * 0.85 * (0.7 + 0.3 * prand(seed, i, 3));
                push_c(&mut out, pos, 3.6 * spin + 0.8, alpha, palette[(i % 3) as usize]);
            }
        }
        ClipKind::Meteor => {
            // The head travels head_d(t) along `angle`, with a
            // slight acceleration (0.75t + 0.25t^2 reaches exactly
            // `travel` at t = 1). Trail sparks sit at fixed lags
            // behind the head: a spark only exists once the head
            // has passed its lag (d >= 0), spreads a little wider
            // the further back it sits, and fades with its lag.
            let dir = dir_from_deg(clip.angle);
            let perp = [-dir[1], dir[0]];
            let travel = if clip.length > 0.0 { clip.length } else { 240.0 };
            let head_d = travel * (0.75 * t + 0.25 * t * t);
            // White-hot head: the clip colour pulled 70% to white.
            let core = [
                color[0] + ((255 - color[0] as u16) * 70 / 100) as u8,
                color[1] + ((255 - color[1] as u16) * 70 / 100) as u8,
                color[2] + ((255 - color[2] as u16) * 70 / 100) as u8,
            ];
            let fade_in = (t / 0.05).min(1.0);
            for i in 0..22u32 {
                let lag = 7.0 + i as f32 * 5.5 + prand(seed, i, 1) * 4.0;
                let d = head_d - lag;
                if d < 0.0 {
                    continue;
                }
                let spread = (prand(seed, i, 2) - 0.5) * (2.0 + lag * 0.07);
                let pos = [
                    anchor[0] + dir[0] * d + perp[0] * spread,
                    anchor[1] + dir[1] * d + perp[1] * spread,
                ];
                let tail = 1.0 - i as f32 / 22.0;
                push(
                    &mut out,
                    pos,
                    3.2 * (0.45 + 0.55 * tail),
                    fade_in * (1.0 - 0.4 * t) * 0.75 * tail,
                );
            }
            // The head itself, on top of its own trail.
            let head = [anchor[0] + dir[0] * head_d, anchor[1] + dir[1] * head_d];
            push_c(&mut out, head, 4.6, fade_in * (1.0 - 0.3 * t), core);
        }
        ClipKind::Petals => {
            // Each petal owns a falling cycle `s` (fract, like
            // Beam's stream) at its own pace, so the air stays
            // full for the whole clip. Alpha is floored
            // (0.25 + 0.75 sin(pi s)) so a petal never blinks out
            // at the wrap — it only softens there. Two pink shades
            // from the clip colour: a saturated petal pink and a
            // paler one pulled toward white.
            let pink = [
                color[0].max(215),
                (color[1] as f32 * 0.55) as u8 + 70,
                (color[2] as f32 * 0.5) as u8 + 85,
            ];
            let pale = [
                pink[0] + ((255 - pink[0] as u16) * 30 / 100) as u8,
                pink[1] + ((255 - pink[1] as u16) * 45 / 100) as u8,
                pink[2] + ((255 - pink[2] as u16) * 35 / 100) as u8,
            ];
            for i in 0..18u32 {
                let speed = 0.7 + prand(seed, i, 5) * 0.5;
                let s = (t * speed + prand(seed, i, 7)).fract();
                let fall = 90.0 + prand(seed, i, 2) * 60.0;
                let phase = prand(seed, i, 4) * std::f32::consts::TAU;
                // Gentle sway that widens as the petal falls.
                let sway = (5.0 + 11.0 * s) * (local * 0.55 + phase).sin();
                let pos = [
                    anchor[0] + (prand(seed, i, 1) - 0.5) * 140.0 + sway,
                    anchor[1] - 14.0 + fall * s,
                ];
                // Tumbling like leaves, at the same slow rate.
                let spin = 0.5
                    + 0.5 * (local * 0.7 + prand(seed, i, 6) * std::f32::consts::TAU)
                        .sin()
                        .abs();
                let alpha = (0.25 + 0.75 * (std::f32::consts::PI * s).sin()) * 0.85;
                let col = if i % 3 == 2 { pale } else { pink };
                push_c(&mut out, pos, 3.8 * spin + 0.7, alpha, col);
            }
        }
        ClipKind::Splash => {
            // Ballistic droplets in per-frame units, exactly
            // Fountain's convention (age = local / land, gone at
            // age 1) but one even fan and stronger gravity, so
            // the arcs are compact and crown-like. `land` is when
            // a droplet's arc returns to the anchor's height;
            // droplets launched with no upward component live out
            // the clip instead (Fountain's rule).
            let g = 1.6;
            for i in 0..24u32 {
                let fan = (i as f32 / 23.0 - 0.5) * 100.0; // -50..=+50 deg
                let dir = dir_from_deg(clip.angle + fan);
                let v = 5.5 + prand(seed, i, 2) * 3.5;
                let land = if dir[1] < -0.05 {
                    2.0 * v * (-dir[1]) / g
                } else {
                    clip.duration
                };
                let age = if land > 0.0 { local / land } else { 1.0 };
                if age >= 1.0 {
                    continue;
                }
                let pos = [
                    anchor[0] + dir[0] * v * local,
                    anchor[1] + dir[1] * v * local + 0.5 * g * local * local,
                ];
                push(&mut out, pos, 2.8 * (1.0 - 0.35 * age), 0.92 * (1.0 - age));
            }
        }
        ClipKind::Spiral => {
            // Mote i belongs to arm (i % 3); arms are offset by a
            // third of a turn. The extra `s * 2.4` wind across a
            // mote's own cycle bends each spoke into an arm, and
            // the whole pattern rotates at 0.16 rad/frame. Motes
            // fade in at the hub and dim toward the rim.
            let max_r = if clip.length > 0.0 { clip.length } else { 90.0 };
            for i in 0..30u32 {
                let arm = (i % 3) as f32;
                let speed = 0.8 + prand(seed, i, 5) * 0.5;
                let s = (t * speed + prand(seed, i, 7)).fract();
                let theta = arm * (std::f32::consts::TAU / 3.0)
                    + prand(seed, i, 1) * 0.35
                    + local * 0.16
                    + s * 2.4;
                let r = max_r * s;
                let pos = [anchor[0] + theta.cos() * r, anchor[1] + theta.sin() * r];
                let fade_in = (s / 0.12).min(1.0);
                let alpha = envelope * 0.9 * fade_in * (1.0 - s * 0.75);
                push(&mut out, pos, 3.1 * (1.0 - 0.35 * s), alpha);
            }
        }
        ClipKind::Fireworks => {
            // One shell bursts at the anchor at the clip's start:
            // every star leaves at t = 0 with a seeded direction
            // and reach, decelerates under drag (the ease-out
            // reach Burst uses), sags under gravity, and twinkles
            // as the whole shell fades. The palette mixes the
            // clip colour with a white-hot pull and its channel
            // rotation, so one burst carries several shades.
            let white = [
                color[0] + ((255 - color[0] as u16) * 55 / 100) as u8,
                color[1] + ((255 - color[1] as u16) * 55 / 100) as u8,
                color[2] + ((255 - color[2] as u16) * 55 / 100) as u8,
            ];
            let spun = rotate_channels(color, 1);
            for i in 0..36u32 {
                let a = prand(seed, i, 1) * std::f32::consts::TAU;
                let reach = 45.0 + prand(seed, i, 2) * 95.0;
                let d = reach * (1.0 - (1.0 - t) * (1.0 - t));
                let pos = [
                    anchor[0] + a.cos() * d,
                    anchor[1] + a.sin() * d + 30.0 * t * t,
                ];
                let twinkle = 0.7 + 0.3 * (local * 3.1 + i as f32 * 2.9).sin();
                let col = match i % 3 {
                    0 => color,
                    1 => white,
                    _ => spun,
                };
                push_c(&mut out, pos, 3.0 * (1.0 - 0.55 * t), (1.0 - t) * twinkle, col);
            }
        }
        ClipKind::Aurora => {
            // Three horizontal bands above the anchor, 13 motes
            // each. A mote's height is a sine of its place along
            // the band whose phase runs with `local`, so the
            // crests travel sideways and the curtains ripple;
            // each band also sways slowly as a whole, at its own
            // rate. Bands differ in wavelength, height and
            // lightness (factors 1.05 / 0.8 / 0.55 of the clip
            // colour, top band palest).
            for b in 0..3u32 {
                let f = [1.05, 0.8, 0.55][b as usize];
                let col = [
                    (color[0] as f32 * f).min(255.0) as u8,
                    (color[1] as f32 * f).min(255.0) as u8,
                    (color[2] as f32 * f).min(255.0) as u8,
                ];
                let base_y = anchor[1] - 26.0 - b as f32 * 16.0;
                let wave_len = 46.0 + b as f32 * 14.0;
                let phase = prand(seed, b, 11) * std::f32::consts::TAU;
                for i in 0..13u32 {
                    let x0 = -78.0 + i as f32 * 13.0;
                    let wave = (x0 / wave_len * std::f32::consts::TAU
                        - local * (0.25 + 0.12 * b as f32)
                        + phase)
                        .sin();
                    let pos = [
                        anchor[0] + x0 + 6.0 * (local * 0.18 + phase).sin(),
                        base_y + 7.0 * wave,
                    ];
                    let shimmer = 0.65 + 0.35 * (local * 0.9 + i as f32 * 1.3 + b as f32).sin();
                    push_c(&mut out, pos, 4.2, envelope * 0.5 * shimmer, col);
                }
            }
        }
        ClipKind::Comet => {
            // The head glides at a steady pace (head_d = travel
            // * t — Meteor accelerates; the comet does not) and
            // its tail motes sit at fixed lags behind it, like
            // Meteor's sparks but larger and softer, in a smooth
            // taper. The whole comet fades in fast and out across
            // the last 15% of the clip, rather than burning to
            // the very end.
            let dir = dir_from_deg(clip.angle);
            let perp = [-dir[1], dir[0]];
            let travel = if clip.length > 0.0 { clip.length } else { 200.0 };
            let head_d = travel * t;
            // White-hot head: the clip colour pulled 70% to white
            // (the same core blend Meteor uses).
            let core = [
                color[0] + ((255 - color[0] as u16) * 70 / 100) as u8,
                color[1] + ((255 - color[1] as u16) * 70 / 100) as u8,
                color[2] + ((255 - color[2] as u16) * 70 / 100) as u8,
            ];
            let fade_in = (t / 0.04).min(1.0);
            let fade_out = ((1.0 - t) / 0.15).min(1.0);
            let glow = fade_in * fade_out;
            for i in 0..24u32 {
                let lag = 6.0 + i as f32 * 4.5 + prand(seed, i, 1) * 3.0;
                let d = head_d - lag;
                if d < 0.0 {
                    continue;
                }
                let spread = (prand(seed, i, 2) - 0.5) * (1.5 + lag * 0.05);
                let pos = [
                    anchor[0] + dir[0] * d + perp[0] * spread,
                    anchor[1] + dir[1] * d + perp[1] * spread,
                ];
                let tail = 1.0 - i as f32 / 24.0;
                push(&mut out, pos, 5.2 * (0.35 + 0.65 * tail), glow * 0.6 * tail);
            }
            // The head itself, on top of its own tail.
            let head = [anchor[0] + dir[0] * head_d, anchor[1] + dir[1] * head_d];
            push_c(&mut out, head, 6.4, glow, core);
        }
        ClipKind::Galaxy => {
            // Three arms of fourteen stars. A star's place is
            // fixed by its seeds — arm (i % 3) and fractional
            // radius u along the arm — and the extra `u * 2.6`
            // wind bends each spoke into a spiral arm, exactly
            // like Spiral's. What differs is the motion: nothing
            // cycles. The whole pattern rotates at 0.45 rad per
            // frame and the pattern radius grows from a quarter
            // of `length` (110 when unset) to all of it, so the
            // galaxy blooms as it turns. Each arm is drawn in
            // its own channel rotation of the clip colour, and
            // stars dim toward the rim.
            let max_r = if clip.length > 0.0 { clip.length } else { 110.0 };
            let radius = max_r * (0.25 + 0.75 * t);
            for i in 0..42u32 {
                let arm = i % 3;
                let u = 0.15 + 0.85 * prand(seed, i, 4);
                let theta = arm as f32 * (std::f32::consts::TAU / 3.0)
                    + u * 2.6
                    + local * 0.45;
                let r = u * radius;
                let pos = [anchor[0] + theta.cos() * r, anchor[1] + theta.sin() * r];
                let col = rotate_channels(color, arm);
                push_c(&mut out, pos, 3.2 * (1.0 - 0.4 * u), envelope * (1.0 - 0.5 * u), col);
            }
        }
        ClipKind::Prism => {
            // Thirty-six motes pinned along an arc above the
            // anchor: mote i sits at a fixed fraction of the
            // fan's ±66° spread on a circle of radius `length`
            // (70 when unset), and the whole fan sweeps slowly
            // from side to side. The colour is a channel
            // rotation of the clip colour whose step advances
            // along the arc and cycles with `local`, so the hue
            // bands travel around the fan while the motes
            // themselves hold their places on the arc.
            let radius = if clip.length > 0.0 { clip.length } else { 70.0 };
            let sweep = 0.12 * (local * 0.6).sin();
            let shift = (local * 1.5) as u32;
            for i in 0..36u32 {
                let a = -1.15 + 2.3 * (i as f32 / 35.0) + sweep;
                let pos = [
                    anchor[0] + a.sin() * radius,
                    anchor[1] - a.cos() * radius,
                ];
                let col = rotate_channels(color, i / 4 + shift);
                let shimmer = 0.7 + 0.3 * (local * 1.1 + i as f32 * 0.7).sin();
                push_c(&mut out, pos, 3.4, envelope * 0.85 * shimmer, col);
            }
        }
        ClipKind::Vortex => {
            // A one-shot inward spiral, the mirror of the
            // fireworks shell: every mote starts at its seeded
            // share of the rim radius (0.55..1 of `length`, 110
            // when unset) and its radius shrinks with (1 - t*t),
            // so the fall accelerates as the centre nears. The
            // wind angle grows with the distance fallen, winding
            // each mote around the anchor as it drops in, and
            // the alpha rides the same (1 - t) fall, so motes
            // fade and shrink as they arrive and the vortex is
            // fully swallowed by the clip's end.
            let max_r = if clip.length > 0.0 { clip.length } else { 110.0 };
            for i in 0..36u32 {
                let u = 0.55 + 0.45 * prand(seed, i, 6);
                let theta0 = prand(seed, i, 5) * std::f32::consts::TAU;
                let r = max_r * u * (1.0 - t * t);
                let theta = theta0 + t * (3.0 + 2.0 * u);
                let pos = [anchor[0] + theta.cos() * r, anchor[1] + theta.sin() * r];
                let fade_in = (t / 0.06).min(1.0);
                let alpha = envelope * fade_in * (1.0 - t);
                push(&mut out, pos, 3.4 * (0.35 + 0.65 * (1.0 - t)), alpha);
            }
        }
    }
    out
}


/// Continuous weather over the whole timeline (wave-5 prep;
/// emission lives in vfx.rs, rendering in export.rs).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Weather {
    pub kind: WeatherKind,
    /// Particles per frame at steady state (0..=200).
    pub amount: f32,
    /// Horizontal drift in stage units per second.
    pub wind: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeatherKind {
    Rain,
    Snow,
    Fireflies,
    Fog,
}

/// Wrap a coordinate into `[0, m)`. The weather field lives on a
/// torus: positions are computed modulo the stage size, so a
/// particle that leaves one edge re-enters at the opposite one.
fn wrap(v: f32, m: f32) -> f32 {
    if m > 0.0 {
        v.rem_euclid(m)
    } else {
        0.0
    }
}

/// Live weather particles at fractional frame time `frame_f`:
/// (centre, radius, colour, alpha), in stage coordinates.
///
/// Emission is closed-form and stateless, like everything else in
/// this module. Each particle `i` owns a fixed home position and
/// phase, spread over the stage by the shared `prand` hash, and its
/// position is a pure function of `(i, frame_f)`:
///
/// - Rain and snow fall: `y = wrap(home_y + speed_i * t)`. The
///   fall wraps modulo the stage height, so drops leaving the
///   bottom re-enter at the top — at any instant the field is
///   uniformly filled, with no spawn or death bookkeeping.
/// - Wind drifts `x` the same way, modulo the stage width; rain
///   additionally slants — its sideways offset grows with the
///   distance fallen, so stronger wind means a more slanted fall.
/// - Snow adds a per-flake sinusoidal sway on top of the drift.
/// - Fireflies wander around their home on slow Lissajous paths
///   (sums of sines), drift at half the wind speed, and pulse.
/// - Fog banks are huge, faint discs that drift slower than the
///   wind with a slow vertical bob; it is their overlap that
///   makes the field read as fog rather than as particles.
///
/// `t` is `frame_f / fps` seconds (30 fps assumed if `fps` is not
/// positive), so speeds are in stage units per second and look the
/// same at any project frame rate. The particle count is
/// `amount` rounded, clamped to `0..=200`.
pub fn weather_particles(
    w: &Weather,
    frame_f: f32,
    stage: [f32; 2],
    fps: f32,
) -> Vec<(V2, f32, [u8; 3], f32)> {
    let sw = stage[0].max(1.0);
    let sh = stage[1].max(1.0);
    let count = w.amount.round().clamp(0.0, 200.0) as usize;
    let mut out = Vec::with_capacity(count);
    if count == 0 {
        return out;
    }
    let secs = frame_f / if fps > 0.0 { fps } else { 30.0 };
    let wind = w.wind;
    for i in 0..count as u32 {
        let p = match w.kind {
            WeatherKind::Rain => {
                let seed = 0x9A17_0001;
                let bx = prand(seed, i, 1) * sw;
                let off = prand(seed, i, 2) * sh;
                let speed = 520.0 + prand(seed, i, 3) * 320.0;
                let travel = off + speed * secs;
                let y = wrap(travel, sh);
                let slant = (wind / 600.0).clamp(-0.6, 0.6);
                let x = wrap(bx + wind * secs + travel * slant, sw);
                let alpha = 0.38 + 0.24 * prand(seed, i, 4);
                ([x, y], 1.2, [198, 220, 242], alpha)
            }
            WeatherKind::Snow => {
                let seed = 0x9A17_0002;
                let bx = prand(seed, i, 1) * sw;
                let off = prand(seed, i, 2) * sh;
                let speed = 26.0 + prand(seed, i, 3) * 40.0;
                let y = wrap(off + speed * secs, sh);
                let sway_amp = 8.0 + prand(seed, i, 4) * 14.0;
                let sway_freq = 0.6 + prand(seed, i, 5) * 1.2;
                let phase = prand(seed, i, 6) * std::f32::consts::TAU;
                let x = wrap(
                    bx + wind * secs + sway_amp * (sway_freq * secs + phase).sin(),
                    sw,
                );
                let size = 1.5 + prand(seed, i, 7);
                let alpha = 0.65 + 0.3 * prand(seed, i, 8);
                ([x, y], size, [248, 250, 255], alpha)
            }
            WeatherKind::Fireflies => {
                let seed = 0x9A17_0003;
                let hx = prand(seed, i, 1) * sw;
                let hy = prand(seed, i, 2) * sh;
                let fm = 0.7 + prand(seed, i, 3) * 0.6;
                let ox = 30.0 * (0.31 * fm * secs + prand(seed, i, 4) * std::f32::consts::TAU).sin()
                    + 12.0 * (0.83 * fm * secs + prand(seed, i, 5) * std::f32::consts::TAU).sin();
                let oy = 24.0 * (0.26 * fm * secs + prand(seed, i, 6) * std::f32::consts::TAU).sin()
                    + 10.0 * (0.71 * fm * secs + prand(seed, i, 7) * std::f32::consts::TAU).sin();
                let x = wrap(hx + wind * secs * 0.5 + ox, sw);
                let y = wrap(hy + oy, sh);
                let alpha = 0.55
                    + 0.35
                        * ((1.2 + prand(seed, i, 8) * 1.4) * secs
                            + prand(seed, i, 9) * std::f32::consts::TAU)
                            .sin();
                let size = 1.6 + prand(seed, i, 10) * 0.9;
                let g = prand(seed, i, 11);
                let col = [
                    (232.0 - 64.0 * g) as u8,
                    (216.0 + 14.0 * g) as u8,
                    (106.0 + 4.0 * g) as u8,
                ];
                ([x, y], size, col, alpha)
            }
            WeatherKind::Fog => {
                let seed = 0x9A17_0004;
                let hx = prand(seed, i, 1) * sw;
                let hy = prand(seed, i, 2) * sh;
                // Banks drift slower than the wind itself, plus a
                // slow per-bank wander, and bob gently. Centres
                // wrap on the torus like every other kind.
                let drift = wind * 0.5 + 6.0 * prand(seed, i, 3);
                let x = wrap(hx + drift * secs, sw);
                let bob = 14.0
                    * (0.11 * secs + prand(seed, i, 4) * std::f32::consts::TAU).sin();
                let y = wrap(hy + bob, sh);
                let size = 46.0 + prand(seed, i, 5) * 54.0;
                let alpha = 0.05 + 0.05 * prand(seed, i, 6);
                ([x, y], size, [216, 222, 232], alpha)
            }
        };
        out.push(p);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(kind: ClipKind) -> ParticleClip {
        ParticleClip {
            start: 10.0,
            duration: 24.0,
            kind,
            color: 0xFFC65D,
            seed: 777,
            angle: 90.0, // dir = (1, 0): pointing right
            length: 160.0,
        }
    }

    const KINDS: [ClipKind; 24] = [
        ClipKind::Burst,
        ClipKind::Sparkle,
        ClipKind::Smoke,
        ClipKind::Beam,
        ClipKind::Aura,
        ClipKind::Trail,
        ClipKind::Confetti,
        ClipKind::ShockRing,
        ClipKind::Ember,
        ClipKind::Fountain,
        ClipKind::Orbit,
        ClipKind::Lightning,
        ClipKind::Bubbles,
        ClipKind::Leaves,
        ClipKind::Meteor,
        ClipKind::Petals,
        ClipKind::Splash,
        ClipKind::Spiral,
        ClipKind::Fireworks,
        ClipKind::Aurora,
        ClipKind::Comet,
        ClipKind::Galaxy,
        ClipKind::Prism,
        ClipKind::Vortex,
    ];

    #[test]
    fn clips_are_deterministic() {
        let anchor = [300.0, 280.0];
        for kind in KINDS {
            let c = clip(kind);
            let a = clip_particles(&c, 17.3, anchor);
            let b = clip_particles(&c, 17.3, anchor);
            assert_eq!(a, b, "{kind:?} differs between identical calls");
            assert!(!a.is_empty(), "{kind:?} should be live mid-clip");
        }
    }

    #[test]
    fn clips_live_only_inside_their_window() {
        let anchor = [300.0, 280.0];
        for kind in KINDS {
            let c = clip(kind);
            assert!(
                clip_particles(&c, c.start - 0.01, anchor).is_empty(),
                "{kind:?} alive before start"
            );
            assert!(
                clip_particles(&c, c.start + c.duration, anchor).is_empty(),
                "{kind:?} alive at end (window is half-open)"
            );
            assert!(
                clip_particles(&c, c.start + c.duration + 5.0, anchor).is_empty(),
                "{kind:?} alive after end"
            );
            assert!(
                !clip_particles(&c, c.start + c.duration * 0.5, anchor).is_empty(),
                "{kind:?} dead mid-clip"
            );
        }
        // A clip with no positive duration never emits.
        let mut dead = clip(ClipKind::Burst);
        dead.duration = 0.0;
        assert!(clip_particles(&dead, dead.start, anchor).is_empty());
    }

    #[test]
    fn beam_particles_stay_near_the_ray() {
        let anchor = [200.0, 200.0];
        let c = clip(ClipKind::Beam); // angle 90 -> dir (1, 0)
        let dir = dir_from_deg(c.angle);
        for frame in [14.0, 20.0, 28.0] {
            let ps = clip_particles(&c, frame, anchor);
            assert!(!ps.is_empty());
            for (p, _, _, _) in &ps {
                let rel = [p[0] - anchor[0], p[1] - anchor[1]];
                let along = rel[0] * dir[0] + rel[1] * dir[1];
                let perp = (rel[0] * dir[1] - rel[1] * dir[0]).abs();
                assert!(perp <= 3.0, "beam mote {perp} off the ray at frame {frame}");
                assert!(
                    (-0.01..=c.length + 0.01).contains(&along),
                    "beam mote at {along} along the ray, length {}",
                    c.length
                );
            }
        }
    }

    #[test]
    fn aura_particles_stay_near_the_ring() {
        let anchor = [400.0, 300.0];
        let c = clip(ClipKind::Aura);
        for frame in [16.0, 22.0, 30.0] {
            let t = (frame - c.start) / c.duration;
            let ring = 30.0 + 4.0 * (std::f32::consts::TAU * t).sin();
            let ps = clip_particles(&c, frame, anchor);
            assert!(!ps.is_empty());
            for (p, _, _, _) in &ps {
                let d = ((p[0] - anchor[0]).powi(2) + (p[1] - anchor[1]).powi(2)).sqrt();
                assert!(
                    (d - ring).abs() <= 3.0,
                    "aura mote at radius {d}, ring is {ring} at frame {frame}"
                );
            }
        }
    }

    #[test]
    fn trail_ghosts_chain_behind_the_anchor() {
        let anchor = [320.0, 240.0];
        let c = clip(ClipKind::Trail); // angle 90 -> ghosts trail toward -x
        let dir = dir_from_deg(c.angle);
        let ps = clip_particles(&c, 22.0, anchor);
        assert_eq!(ps.len(), 8, "one ghost per spacing step");
        for (p, _, _, alpha) in &ps {
            let rel = [p[0] - anchor[0], p[1] - anchor[1]];
            let along = rel[0] * dir[0] + rel[1] * dir[1];
            assert!(along <= 0.01, "ghost ahead of the anchor: {along}");
            assert!(*alpha > 0.0 && *alpha <= 0.55);
        }
    }


    #[test]
    fn confetti_uses_several_colours() {
        let c = clip(ClipKind::Confetti);
        let ps = clip_particles(&c, 16.0, [300.0, 300.0]);
        assert!(!ps.is_empty());
        let mut colours: Vec<[u8; 3]> = ps.iter().map(|p| p.2).collect();
        colours.sort();
        colours.dedup();
        assert!(
            colours.len() >= 3,
            "confetti should mix colours from the palette, got {colours:?}"
        );
    }

    #[test]
    fn confetti_pops_up_then_falls() {
        let c = clip(ClipKind::Confetti);
        let anchor = [300.0, 300.0];
        let mean_y = |frame: f32| {
            let ps = clip_particles(&c, frame, anchor);
            assert!(!ps.is_empty());
            ps.iter().map(|p| p.0[1]).sum::<f32>() / ps.len() as f32
        };
        assert!(
            mean_y(c.start + c.duration * 0.12) < anchor[1],
            "confetti should be above the anchor early (popped up)"
        );
        assert!(
            mean_y(c.start + c.duration * 0.9) > anchor[1],
            "confetti should have fallen below the anchor late"
        );
    }

    #[test]
    fn shock_ring_expands_and_fades() {
        let c = clip(ClipKind::ShockRing); // length 160
        let anchor = [300.0, 300.0];
        let stats = |frame: f32| {
            let ps = clip_particles(&c, frame, anchor);
            assert!(!ps.is_empty());
            let r = ps
                .iter()
                .map(|p| ((p.0[0] - anchor[0]).powi(2) + (p.0[1] - anchor[1]).powi(2)).sqrt())
                .sum::<f32>()
                / ps.len() as f32;
            let a = ps.iter().map(|p| p.3).sum::<f32>() / ps.len() as f32;
            (r, a)
        };
        let (r1, a1) = stats(c.start + c.duration * 0.25);
        let (r2, a2) = stats(c.start + c.duration * 0.75);
        assert!((r1 - 40.0).abs() < 4.0, "ring radius at t=0.25 is {r1}, want ~40");
        assert!((r2 - 120.0).abs() < 4.0, "ring radius at t=0.75 is {r2}, want ~120");
        assert!(r2 > r1, "ring should expand");
        assert!(a2 < a1, "ring should fade ({a1} -> {a2})");
    }

    #[test]
    fn embers_rise_decelerate_shrink_and_fade() {
        let c = clip(ClipKind::Ember);
        let anchor = [300.0, 300.0];
        let at = |t: f32| clip_particles(&c, c.start + c.duration * t, anchor);
        // Every ember stays emitted for the whole clip, so indices
        // are stable and means are comparable across frames.
        let n = at(0.1).len();
        assert!(n > 0);
        for t in [0.3, 0.55, 0.75] {
            assert_eq!(at(t).len(), n, "ember count changed at t={t}");
        }
        // Embers only ever rise above the anchor.
        assert!(
            at(0.3).iter().all(|p| p.0[1] < anchor[1] + 0.01),
            "an ember fell below the anchor"
        );
        // Mean height above the anchor: the gain between two equal
        // windows shrinks as the climb decelerates.
        let height = |t: f32| {
            let ps = at(t);
            anchor[1] - ps.iter().map(|p| p.0[1]).sum::<f32>() / ps.len() as f32
        };
        let early = height(0.3) - height(0.1);
        let late = height(0.75) - height(0.55);
        assert!(
            early > late && late >= 0.0,
            "embers should decelerate: rose {early} early vs {late} late"
        );
        // They shrink and fade as they cool.
        let mean = |t: f32, pick: fn(&([f32; 2], f32, [u8; 3], f32)) -> f32| {
            let ps = at(t);
            ps.iter().map(pick).sum::<f32>() / ps.len() as f32
        };
        assert!(mean(0.75, |p| p.1) < mean(0.2, |p| p.1), "embers should shrink");
        assert!(mean(0.75, |p| p.3) < mean(0.3, |p| p.3), "embers should fade");
    }

    #[test]
    fn fountain_peaks_then_falls_and_lands() {
        let mut c = clip(ClipKind::Fountain);
        c.angle = 180.0; // straight up in the model convention
        let anchor = [300.0, 300.0];
        let mean_y = |local: f32| {
            let ps = clip_particles(&c, c.start + local, anchor);
            assert!(!ps.is_empty(), "fountain dry at local {local}");
            ps.iter().map(|p| p.0[1]).sum::<f32>() / ps.len() as f32
        };
        let y3 = mean_y(3.0);
        let y7 = mean_y(7.0);
        let y14 = mean_y(14.0);
        assert!(y3 < anchor[1], "droplets above the anchor early: {y3}");
        assert!(y7 < y3, "still rising toward the peak: {y7} vs {y3}");
        assert!(y14 > y7, "falling back down late: {y14} vs {y7}");
        // The longest flight is 2 * 10.5 / 1.3 ~= 16.2 frames, so
        // by local 20 every droplet has landed and vanished.
        assert!(
            clip_particles(&c, c.start + 20.0, anchor).is_empty(),
            "droplets should all have landed by local 20"
        );
    }

    #[test]
    fn orbit_motes_hold_their_rings_trail_and_move() {
        let c = clip(ClipKind::Orbit); // length 160
        let anchor = [300.0, 300.0];
        let radii = [0.45 * c.length, 0.7 * c.length, 0.95 * c.length];
        let a = clip_particles(&c, 16.0, anchor);
        assert!(!a.is_empty());
        assert_eq!(a.len(), 54, "18 motes with a head and two ghosts each");
        for (p, _, _, _) in &a {
            let d = ((p[0] - anchor[0]).powi(2) + (p[1] - anchor[1]).powi(2)).sqrt();
            assert!(
                radii.iter().any(|r| (d - r).abs() <= 0.75),
                "orbit mote off its ring at radius {d}"
            );
        }
        // Within each mote's triple, alpha trails off head -> ghost.
        for triple in a.chunks_exact(3) {
            assert!(
                triple[0].3 > triple[1].3 && triple[1].3 > triple[2].3,
                "trailing alpha should decrease: {:?}",
                triple.iter().map(|p| p.3).collect::<Vec<_>>()
            );
        }
        // And the whole system actually rotates.
        let b = clip_particles(&c, 18.0, anchor);
        assert_ne!(a, b, "orbit should move between frames");
    }

    #[test]
    fn lightning_bolt_runs_along_the_angle_and_is_jagged() {
        let c = clip(ClipKind::Lightning); // angle 90 -> dir (1, 0), length 160
        let anchor = [300.0, 300.0];
        let dir = dir_from_deg(c.angle);
        let ps = clip_particles(&c, 14.0, anchor);
        assert!(!ps.is_empty());
        // The bolt starts at the anchor and reaches ~length along
        // the ray; sideways offsets stay within the jitter bound.
        let mut max_along: f32 = 0.0;
        for (p, _, _, _) in &ps {
            let rel = [p[0] - anchor[0], p[1] - anchor[1]];
            let along = rel[0] * dir[0] + rel[1] * dir[1];
            let perp = (rel[0] * dir[1] - rel[1] * dir[0]).abs();
            assert!(along >= -0.01, "bolt dot behind the anchor: {along}");
            assert!(perp <= 13.5, "bolt dot {perp} off the zigzag bound");
            max_along = max_along.max(along);
        }
        assert!(
            (max_along - c.length).abs() < 1.0,
            "bolt tip at {max_along} along the ray, want ~{}",
            c.length
        );
        // Jagged: across a few frames, at least one dot sits well
        // off the straight ray.
        let mut worst = 0.0f32;
        for frame in [11.0, 13.0, 15.0, 17.0, 19.0] {
            for (p, _, _, _) in clip_particles(&c, frame, anchor) {
                let rel = [p[0] - anchor[0], p[1] - anchor[1]];
                worst = worst.max((rel[0] * dir[1] - rel[1] * dir[0]).abs());
            }
        }
        assert!(worst > 3.0, "bolt never zigzags: max sideways offset {worst}");
    }

    #[test]
    fn lightning_restricks_each_frame_but_stays_deterministic() {
        let c = clip(ClipKind::Lightning);
        let anchor = [300.0, 300.0];
        // Same fractional frame, identical bolt (covered globally
        // too, but pin it here against the frame-salted design).
        assert_eq!(
            clip_particles(&c, 17.3, anchor),
            clip_particles(&c, 17.3, anchor)
        );
        // The next integer frame re-seeds the jitter: new bolt.
        assert_ne!(
            clip_particles(&c, 17.0, anchor),
            clip_particles(&c, 18.0, anchor),
            "bolt shape should change between frames"
        );
    }

    #[test]
    fn bubbles_rise_grow_and_pop() {
        let c = clip(ClipKind::Bubbles);
        let anchor = [300.0, 300.0];
        let at = |t: f32| clip_particles(&c, c.start + c.duration * t, anchor);
        // Mid-life: the full set is up, and every bubble is above
        // the anchor.
        let mid = at(0.45);
        assert_eq!(mid.len(), 16, "all bubbles should be alive mid-clip");
        assert!(
            mid.iter().all(|p| p.0[1] < anchor[1]),
            "a bubble sank below the anchor"
        );
        // Rising: the mean height above the anchor grows with t.
        let height = |t: f32| {
            let ps = at(t);
            anchor[1] - ps.iter().map(|p| p.0[1]).sum::<f32>() / ps.len() as f32
        };
        assert!(
            height(0.45) > height(0.15),
            "bubbles should climb: {} vs {}",
            height(0.45),
            height(0.15)
        );
        // Growing: mean radius is larger later in the life cycle.
        let size = |t: f32| {
            let ps = at(t);
            ps.iter().map(|p| p.1).sum::<f32>() / ps.len() as f32
        };
        assert!(size(0.45) > size(0.15), "bubbles should grow as they rise");
        // Popping: by the very end of the clip most bubbles have
        // burst (their life cycles ran out / alpha collapsed).
        let late = at(0.99);
        assert!(
            late.len() < mid.len(),
            "bubbles should pop late: {} alive at t=0.99 vs {} mid-clip",
            late.len(),
            mid.len()
        );
    }

    #[test]
    fn leaves_fall_flutter_and_stay_green() {
        let c = clip(ClipKind::Leaves); // clip colour 0xFFC65D (amber)
        let anchor = [300.0, 300.0];
        let mean_y = |t: f32| {
            let ps = clip_particles(&c, c.start + c.duration * t, anchor);
            assert!(!ps.is_empty());
            ps.iter().map(|p| p.0[1]).sum::<f32>() / ps.len() as f32
        };
        // Falling: mean y grows (down-screen) through the clip and
        // ends below the anchor.
        assert!(
            mean_y(0.65) > mean_y(0.25),
            "leaves should descend: {} vs {}",
            mean_y(0.65),
            mean_y(0.25)
        );
        assert!(mean_y(0.65) > anchor[1], "leaves should end below the anchor");
        // Flutter: the sway pushes leaves to both sides of the
        // anchor over the clip.
        let mut xs: Vec<f32> = Vec::new();
        for t in [0.3, 0.45, 0.6] {
            xs.extend(
                clip_particles(&c, c.start + c.duration * t, anchor)
                    .iter()
                    .map(|p| p.0[0]),
            );
        }
        assert!(xs.iter().any(|x| *x < anchor[0] - 4.0), "no leaf swayed left");
        assert!(xs.iter().any(|x| *x > anchor[0] + 4.0), "no leaf swayed right");
        // Palette: green dominates every leaf colour even though
        // the clip colour is amber.
        for (_, _, col, _) in clip_particles(&c, c.start + c.duration * 0.5, anchor) {
            assert!(
                col[1] >= col[0] && col[1] >= col[2],
                "leaf colour {col:?} is not green-dominant"
            );
        }
    }

    #[test]
    fn meteor_head_streaks_and_drags_a_trail_behind_it() {
        let c = clip(ClipKind::Meteor); // angle 90 -> dir (1, 0), length 160
        let anchor = [300.0, 300.0];
        let dir = dir_from_deg(c.angle);
        let along = |p: &[f32; 2]| {
            (p[0] - anchor[0]) * dir[0] + (p[1] - anchor[1]) * dir[1]
        };
        let stats = |t: f32| {
            let ps = clip_particles(&c, c.start + c.duration * t, anchor);
            assert!(!ps.is_empty());
            let hi = ps.iter().map(|p| along(&p.0)).fold(f32::MIN, f32::max);
            let lo = ps.iter().map(|p| along(&p.0)).fold(f32::MAX, f32::min);
            let big = ps.iter().map(|p| p.1).fold(0.0f32, f32::max);
            (lo, hi, big)
        };
        let (_, hi_early, _) = stats(0.25);
        let (lo_mid, hi_mid, big_mid) = stats(0.5);
        // The head travels: its leading edge is much further along
        // the ray later in the clip, and never past `length`.
        assert!(
            hi_mid > hi_early + 20.0,
            "meteor head should streak forward: {hi_early} -> {hi_mid}"
        );
        assert!(hi_mid <= c.length + 0.01, "head overshot its travel: {hi_mid}");
        // The trail stretches from near the anchor to the head.
        assert!(
            hi_mid - lo_mid > 40.0,
            "trail should span a long stretch: {lo_mid}..{hi_mid}"
        );
        // Only the head is drawn at full head size.
        assert!(big_mid >= 4.0, "meteor head missing (max size {big_mid})");
    }

    #[test]
    fn petals_fill_the_air_drift_down_and_sway() {
        let c = clip(ClipKind::Petals);
        let anchor = [300.0, 300.0];
        // Continuous cycles with a floored alpha: the full set is
        // airborne at every sampled moment of the clip.
        for t in [0.2, 0.5, 0.8] {
            let ps = clip_particles(&c, c.start + c.duration * t, anchor);
            assert_eq!(ps.len(), 18, "petal count changed at t={t}");
        }
        let mid = clip_particles(&c, c.start + c.duration * 0.5, anchor);
        // Wide spread: petals span a broad band across the anchor.
        let xs: Vec<f32> = mid.iter().map(|p| p.0[0]).collect();
        let span = xs.iter().fold(f32::MIN, |a, &x| a.max(x))
            - xs.iter().fold(f32::MAX, |a, &x| a.min(x));
        assert!(span > 70.0, "petals bunched up: x span only {span}");
        // Over the clip, petals traverse a tall falling band: some
        // are caught near the top of a cycle, some near the bottom.
        let mut lo = f32::MAX;
        let mut hi = f32::MIN;
        for step in 0..20 {
            let t = step as f32 / 20.0;
            for p in clip_particles(&c, c.start + c.duration * t, anchor) {
                lo = lo.min(p.0[1]);
                hi = hi.max(p.0[1]);
            }
        }
        assert!(lo < anchor[1] + 30.0, "no petal near its cycle top: min y {lo}");
        assert!(hi > anchor[1] + 80.0, "no petal fell far: max y {hi}");
        // Sway: between two frames ~3 apart, at least one petal's
        // x (a pure sway term — the spread is fixed per petal) has
        // clearly moved sideways.
        let a = clip_particles(&c, c.start + 6.0, anchor);
        let b = clip_particles(&c, c.start + 9.0, anchor);
        let sway = a
            .iter()
            .zip(&b)
            .map(|(pa, pb)| (pb.0[0] - pa.0[0]).abs())
            .fold(0.0f32, f32::max);
        assert!(sway > 3.0, "petals never sway: max sideways move {sway}");
        // Petals stay pink: red dominates every petal colour even
        // though the clip colour is amber.
        for (_, _, col, _) in &mid {
            assert!(
                col[0] >= col[1] && col[0] >= col[2],
                "petal colour {col:?} is not pink-dominant"
            );
        }
    }

    #[test]
    fn splash_fans_up_arcs_and_lands() {
        let mut c = clip(ClipKind::Splash);
        c.angle = 180.0; // straight up in the model convention
        let anchor = [300.0, 300.0];
        // Just after launch the whole fan is airborne.
        let early = clip_particles(&c, c.start + 2.0, anchor);
        assert_eq!(early.len(), 24, "splash fan incomplete at local 2");
        let mean_y = |local: f32| {
            let ps = clip_particles(&c, c.start + local, anchor);
            assert!(!ps.is_empty(), "splash dry at local {local}");
            ps.iter().map(|p| p.0[1]).sum::<f32>() / ps.len() as f32
        };
        // The highest droplet (min y). The fastest droplet also
        // flies longest (land = 2u/g grows with u), so the same
        // droplet tops the arc at every sampled time: its height
        // -u*local + 0.8*local^2 peaks at local = u/1.6 and the
        // comparisons below hold for any launch strength u.
        let top = |local: f32| {
            let ps = clip_particles(&c, c.start + local, anchor);
            assert!(!ps.is_empty(), "splash dry at local {local}");
            ps.iter().map(|p| p.0[1]).fold(f32::MAX, f32::min)
        };
        assert!(
            mean_y(2.0) < anchor[1],
            "droplets should burst upward: mean y {}",
            mean_y(2.0)
        );
        assert!(
            top(5.0) < top(2.0),
            "still climbing toward the crown: {} vs {}",
            top(5.0),
            top(2.0)
        );
        assert!(
            top(9.0) > top(5.0),
            "arcing back down late: {} vs {}",
            top(9.0),
            top(5.0)
        );
        // The longest flight is 2 * 9.0 / 1.6 = 11.25 frames, so
        // by local 14 every droplet has landed and vanished.
        assert!(
            clip_particles(&c, c.start + 14.0, anchor).is_empty(),
            "splash droplets should all have landed by local 14"
        );
    }

    #[test]
    fn spiral_motes_stream_outward_and_rotate() {
        let c = clip(ClipKind::Spiral); // length 160
        let anchor = [300.0, 300.0];
        let mid = clip_particles(&c, c.start + c.duration * 0.5, anchor);
        assert!(!mid.is_empty());
        // Radii span the whole stream: some motes are still near
        // the hub while others have nearly reached the rim.
        let radii: Vec<f32> = mid
            .iter()
            .map(|p| ((p.0[0] - anchor[0]).powi(2) + (p.0[1] - anchor[1]).powi(2)).sqrt())
            .collect();
        let lo = radii.iter().fold(f32::MAX, |a, &r| a.min(r));
        let hi = radii.iter().fold(f32::MIN, |a, &r| a.max(r));
        assert!(lo < 0.3 * c.length, "no mote near the hub: min radius {lo}");
        assert!(hi > 0.55 * c.length, "no mote near the rim: max radius {hi}");
        assert!(hi <= c.length + 0.01, "mote escaped its rim: {hi}");
        // The pattern rotates between frames.
        let later = clip_particles(&c, c.start + c.duration * 0.5 + 3.0, anchor);
        assert_ne!(mid, later, "spiral should rotate between frames");
    }

    #[test]
    fn fireworks_burst_outward_and_fade() {
        let c = clip(ClipKind::Fireworks);
        let anchor = [300.0, 300.0];
        let stats = |t: f32| {
            let ps = clip_particles(&c, c.start + c.duration * t, anchor);
            assert!(!ps.is_empty());
            let reach = ps
                .iter()
                .map(|p| ((p.0[0] - anchor[0]).powi(2) + (p.0[1] - anchor[1]).powi(2)).sqrt())
                .sum::<f32>()
                / ps.len() as f32;
            let alpha = ps.iter().map(|p| p.3).sum::<f32>() / ps.len() as f32;
            (ps.len(), reach, alpha)
        };
        let (n, r_early, a_early) = stats(0.2);
        let (_, r_mid, _) = stats(0.5);
        let (_, _, a_late) = stats(0.85);
        assert_eq!(n, 36, "the whole shell should be out early");
        // Stars fly outward: mean distance from the anchor grows
        // as the burst expands.
        assert!(
            r_mid > r_early + 10.0,
            "firework stars should spread: {r_early} -> {r_mid}"
        );
        // And the shell fades over the clip.
        assert!(a_late < a_early, "fireworks should fade: {a_early} -> {a_late}");
        // The burst mixes shades from its derived palette.
        let mid = clip_particles(&c, c.start + c.duration * 0.5, anchor);
        let mut colours: Vec<[u8; 3]> = mid.iter().map(|p| p.2).collect();
        colours.sort();
        colours.dedup();
        assert!(
            colours.len() >= 2,
            "fireworks should mix colours, got {colours:?}"
        );
    }

    #[test]
    fn aurora_bands_hang_above_the_anchor_and_ripple() {
        let c = clip(ClipKind::Aurora);
        let anchor = [300.0, 300.0];
        let mid = clip_particles(&c, c.start + c.duration * 0.5, anchor);
        assert_eq!(mid.len(), 39, "three bands of thirteen motes");
        // The curtains hang above the anchor: band bases are
        // 26/42/58 above it and the wave adds at most 7 either
        // way, so every mote sits between 19 and 65 above.
        assert!(
            mid.iter().all(|p| p.0[1] < anchor[1] - 15.0),
            "a mote drifted down to the anchor"
        );
        assert!(
            mid.iter().all(|p| p.0[1] > anchor[1] - 70.0),
            "a band floated too high"
        );
        // Three bands: the y values span more than one band gap.
        let lo = mid.iter().map(|p| p.0[1]).fold(f32::MAX, f32::min);
        let hi = mid.iter().map(|p| p.0[1]).fold(f32::MIN, f32::max);
        assert!(hi - lo > 24.0, "bands collapsed: y span only {}", hi - lo);
        // The ribbons flow between frames.
        let later = clip_particles(&c, c.start + c.duration * 0.5 + 3.0, anchor);
        assert_ne!(mid, later, "aurora should ripple between frames");
    }

    #[test]
    fn comet_head_glides_steadily_and_drags_a_tapered_tail() {
        let c = clip(ClipKind::Comet); // angle 90 -> dir (1, 0), length 160
        let anchor = [300.0, 300.0];
        let dir = dir_from_deg(c.angle);
        let along = |p: &[f32; 2]| {
            (p[0] - anchor[0]) * dir[0] + (p[1] - anchor[1]) * dir[1]
        };
        let stats = |t: f32| {
            let ps = clip_particles(&c, c.start + c.duration * t, anchor);
            assert!(!ps.is_empty());
            let hi = ps.iter().map(|p| along(&p.0)).fold(f32::MIN, f32::max);
            let lo = ps.iter().map(|p| along(&p.0)).fold(f32::MAX, f32::min);
            let big = ps.iter().map(|p| p.1).fold(0.0f32, f32::max);
            (lo, hi, big)
        };
        let (_, hi_early, _) = stats(0.25);
        let (lo_mid, hi_mid, big_mid) = stats(0.5);
        // Steady glide: the head's leading edge sits at t * length
        // (Meteor's accelerating head would be behind this pace
        // early and is a different design point).
        assert!(
            (hi_early - 0.25 * c.length).abs() < 1.0,
            "comet head at {hi_early} at t=0.25, want ~{}",
            0.25 * c.length
        );
        assert!(
            (hi_mid - 0.5 * c.length).abs() < 1.0,
            "comet head at {hi_mid} at t=0.5, want ~{}",
            0.5 * c.length
        );
        // Only the head is drawn at full head size.
        assert!(big_mid >= 6.0, "comet head missing (max size {big_mid})");
        // The tail stretches a long way behind the head.
        assert!(
            hi_mid - lo_mid > 50.0,
            "comet tail should stretch: {lo_mid}..{hi_mid}"
        );
    }

    #[test]
    fn wave10_kinds_differ_from_each_other() {
        let anchor = [300.0, 300.0];
        let at = |kind: ClipKind| clip_particles(&clip(kind), 22.0, anchor);
        let fireworks = at(ClipKind::Fireworks);
        let aurora = at(ClipKind::Aurora);
        let comet = at(ClipKind::Comet);
        assert!(!fireworks.is_empty() && !aurora.is_empty() && !comet.is_empty());
        assert_ne!(fireworks, aurora);
        assert_ne!(fireworks, comet);
        assert_ne!(aurora, comet);
        // Comet is Meteor's cousin, not its clone.
        assert_ne!(comet, at(ClipKind::Meteor), "comet renders like meteor");
    }

    #[test]
    fn galaxy_expands_and_rotates_in_formation() {
        let c = clip(ClipKind::Galaxy); // length 160
        let anchor = [300.0, 300.0];
        let mean_r = |t: f32| {
            let ps = clip_particles(&c, c.start + c.duration * t, anchor);
            assert!(!ps.is_empty());
            ps.iter()
                .map(|p| ((p.0[0] - anchor[0]).powi(2) + (p.0[1] - anchor[1]).powi(2)).sqrt())
                .sum::<f32>()
                / ps.len() as f32
        };
        // The whole formation is out mid-clip: 42 stars, three
        // arms of fourteen.
        let mid = clip_particles(&c, c.start + c.duration * 0.5, anchor);
        assert_eq!(mid.len(), 42, "three arms of fourteen stars");
        // The pattern radius grows across the clip, so the mean
        // star distance grows with it.
        let r_early = mean_r(0.15);
        let r_late = mean_r(0.85);
        assert!(
            r_late > r_early + 20.0,
            "galaxy should bloom outward: {r_early} -> {r_late}"
        );
        // Star 0 holds its arm and its fractional radius, so
        // between two frames one second apart in clip time its
        // offset must turn by the pattern's rotation (0.45 rad
        // per frame) and lengthen with the growing radius.
        let p1 = clip_particles(&c, c.start + 7.0, anchor)[0].0;
        let p2 = clip_particles(&c, c.start + 8.0, anchor)[0].0;
        let v1 = [p1[0] - anchor[0], p1[1] - anchor[1]];
        let v2 = [p2[0] - anchor[0], p2[1] - anchor[1]];
        let cross = v1[0] * v2[1] - v1[1] * v2[0];
        let r1 = (v1[0].powi(2) + v1[1].powi(2)).sqrt();
        let r2 = (v2[0].powi(2) + v2[1].powi(2)).sqrt();
        assert!(
            cross > 0.3 * r1 * r2,
            "galaxy should rotate: cross {cross} for radii {r1}, {r2}"
        );
        assert!(r2 > r1 * 1.03, "star 0 should ride the expansion: {r1} -> {r2}");
        // One colour per arm: three channel rotations in play.
        let mut colours: Vec<[u8; 3]> = mid.iter().map(|p| p.2).collect();
        colours.sort();
        colours.dedup();
        assert_eq!(colours.len(), 3, "one channel rotation per arm: {colours:?}");
    }

    #[test]
    fn prism_motes_hold_their_arc_above_the_anchor() {
        let c = clip(ClipKind::Prism); // length 160 -> arc radius 160
        let anchor = [300.0, 300.0];
        let mid = clip_particles(&c, c.start + c.duration * 0.5, anchor);
        assert_eq!(mid.len(), 36, "the full fan should be out mid-clip");
        for (p, _, _, _) in &mid {
            let d = ((p[0] - anchor[0]).powi(2) + (p[1] - anchor[1]).powi(2)).sqrt();
            assert!(
                (d - c.length).abs() <= 2.0,
                "prism mote off its arc: radius {d}, want {}",
                c.length
            );
            // The fan's spread plus the sweep keeps every mote
            // well above the anchor.
            assert!(p[1] < anchor[1] - 10.0, "prism mote at/below the anchor: {p:?}");
        }
        // The sweep moves the fan between frames.
        let later = clip_particles(&c, c.start + c.duration * 0.5 + 3.0, anchor);
        assert_ne!(mid, later, "prism fan should sweep between frames");
        // The hue rotation cycles along the arc: at least the
        // three channel rotations of the clip colour appear.
        let mut colours: Vec<[u8; 3]> = mid.iter().map(|p| p.2).collect();
        colours.sort();
        colours.dedup();
        assert!(
            colours.len() >= 3,
            "prism should cycle colours along the arc, got {colours:?}"
        );
    }

    #[test]
    fn vortex_spirals_inward_and_is_swallowed() {
        let c = clip(ClipKind::Vortex); // length 160
        let anchor = [300.0, 300.0];
        let mean_r = |t: f32| {
            let ps = clip_particles(&c, c.start + c.duration * t, anchor);
            assert!(!ps.is_empty());
            ps.iter()
                .map(|p| ((p.0[0] - anchor[0]).powi(2) + (p.0[1] - anchor[1]).powi(2)).sqrt())
                .sum::<f32>()
                / ps.len() as f32
        };
        // Every mote is out early: the vortex is a single
        // inward fall, not a stream.
        let early = clip_particles(&c, c.start + c.duration * 0.2, anchor);
        assert_eq!(early.len(), 36, "the whole vortex should start at the rim");
        // Inward: the mean radius shrinks hard across the clip
        // (the exact opposite of the fireworks shell).
        let r_early = mean_r(0.2);
        let r_late = mean_r(0.7);
        assert!(
            r_late < r_early - 25.0,
            "vortex should spiral inward: {r_early} -> {r_late}"
        );
        // And it fades as it arrives.
        let a_early = early.iter().map(|p| p.3).sum::<f32>() / early.len() as f32;
        let late = clip_particles(&c, c.start + c.duration * 0.7, anchor);
        let a_late = late.iter().map(|p| p.3).sum::<f32>() / late.len() as f32;
        assert!(a_late < a_early, "vortex should fade as it arrives: {a_early} -> {a_late}");
    }

    #[test]
    fn wave11_kinds_differ_from_each_other() {
        let anchor = [300.0, 300.0];
        let at = |kind: ClipKind| clip_particles(&clip(kind), 22.0, anchor);
        let galaxy = at(ClipKind::Galaxy);
        let prism = at(ClipKind::Prism);
        let vortex = at(ClipKind::Vortex);
        assert!(!galaxy.is_empty() && !prism.is_empty() && !vortex.is_empty());
        assert_ne!(galaxy, prism);
        assert_ne!(galaxy, vortex);
        assert_ne!(prism, vortex);
        // Galaxy holds formation; Spiral cycles its motes.
        assert_ne!(galaxy, at(ClipKind::Spiral), "galaxy renders like spiral");
        // Vortex falls inward; Fireworks bursts outward.
        assert_ne!(vortex, at(ClipKind::Fireworks), "vortex renders like fireworks");
    }

    // ---- Weather ----

    fn weather(kind: WeatherKind, amount: f32, wind: f32) -> Weather {
        Weather { kind, amount, wind }
    }

    const STAGE: [f32; 2] = [960.0, 720.0];
    const WEATHER_KINDS: [WeatherKind; 4] = [
        WeatherKind::Rain,
        WeatherKind::Snow,
        WeatherKind::Fireflies,
        WeatherKind::Fog,
    ];

    #[test]
    fn weather_is_deterministic() {
        for kind in WEATHER_KINDS {
            let w = weather(kind, 80.0, 12.0);
            let a = weather_particles(&w, 37.5, STAGE, 30.0);
            let b = weather_particles(&w, 37.5, STAGE, 30.0);
            assert_eq!(a, b, "{kind:?} differs between identical calls");
            assert!(!a.is_empty());
        }
    }

    #[test]
    fn weather_count_respects_amount_and_clamps() {
        let at = |kind, amount| {
            weather_particles(&weather(kind, amount, 0.0), 0.0, STAGE, 30.0).len()
        };
        assert_eq!(at(WeatherKind::Rain, 40.0), 40);
        assert_eq!(at(WeatherKind::Rain, 10.4), 10, "amount is rounded");
        assert_eq!(at(WeatherKind::Snow, 0.0), 0);
        assert_eq!(at(WeatherKind::Snow, -5.0), 0, "negative clamps to 0");
        assert_eq!(at(WeatherKind::Fireflies, 999.0), 200, "clamped to 200");
    }

    #[test]
    fn fog_weather_round_trips_through_serde() {
        let w = weather(WeatherKind::Fog, 120.0, -8.0);
        let text = serde_json::to_string(&w).unwrap();
        assert!(text.contains("\"Fog\""), "kind wire name changed: {text}");
        let back: Weather = serde_json::from_str(&text).unwrap();
        assert_eq!(back, w);
        // Project JSON written before Fog existed still loads.
        let old: Weather =
            serde_json::from_str("{\"kind\":\"Rain\",\"amount\":90.0,\"wind\":25.0}")
                .unwrap();
        assert_eq!(old.kind, WeatherKind::Rain);
    }

    #[test]
    fn weather_stays_on_stage() {
        for kind in WEATHER_KINDS {
            let w = weather(kind, 120.0, 60.0);
            for frame in [0.0, 13.7, 400.25, 5000.0] {
                for (p, _, _, _) in weather_particles(&w, frame, STAGE, 30.0) {
                    assert!(
                        (0.0..STAGE[0]).contains(&p[0]) && (0.0..STAGE[1]).contains(&p[1]),
                        "{kind:?} particle off stage at {p:?} (frame {frame})"
                    );
                }
            }
        }
    }

    #[test]
    fn rain_falls_fast() {
        let w = weather(WeatherKind::Rain, 60.0, 0.0);
        let a = weather_particles(&w, 10.0, STAGE, 30.0);
        let b = weather_particles(&w, 13.0, STAGE, 30.0); // 0.1 s later
        assert_eq!(a.len(), b.len());
        for (pa, pb) in a.iter().zip(&b) {
            // Forward motion modulo the wrap: 0.1 s of fall is
            // 52..84 units, never backwards or a full wrap.
            let dy = (pb.0[1] - pa.0[1]).rem_euclid(STAGE[1]);
            assert!(
                (20.0..STAGE[1] * 0.5).contains(&dy),
                "rain drop moved {dy} in 0.1 s"
            );
        }
    }

    #[test]
    fn snow_sways_sideways() {
        // No wind: pure drift would keep every flake's x fixed.
        // Sinusoidal sway must push some flakes left, others right.
        let w = weather(WeatherKind::Snow, 60.0, 0.0);
        let a = weather_particles(&w, 0.0, STAGE, 30.0);
        let b = weather_particles(&w, 45.0, STAGE, 30.0); // 1.5 s later
        let mut dxs: Vec<f32> = a
            .iter()
            .zip(&b)
            .map(|(pa, pb)| {
                (pb.0[0] - pa.0[0] + STAGE[0] * 0.5).rem_euclid(STAGE[0]) - STAGE[0] * 0.5
            })
            .collect();
        dxs.sort_by(|x, y| x.partial_cmp(y).unwrap());
        assert!(dxs[0] < -1.0, "no flake swayed left: min dx {}", dxs[0]);
        assert!(
            dxs[dxs.len() - 1] > 1.0,
            "no flake swayed right: max dx {}",
            dxs[dxs.len() - 1]
        );
    }

    #[test]
    fn firefly_alpha_pulses() {
        let w = weather(WeatherKind::Fireflies, 1.0, 0.0);
        let mut lo = 1.0f32;
        let mut hi = 0.0f32;
        for f in 0..240 {
            // 8 s at 30 fps: at least one full pulse period.
            let ps = weather_particles(&w, f as f32, STAGE, 30.0);
            assert_eq!(ps.len(), 1);
            lo = lo.min(ps[0].3);
            hi = hi.max(ps[0].3);
        }
        assert!(lo < 0.35, "firefly never dims: min alpha {lo}");
        assert!(hi > 0.75, "firefly never glows: max alpha {hi}");
        assert!(lo >= 0.19 && hi <= 0.91, "alpha outside the 0.2..0.9 band");
    }

    #[test]
    fn particle_counts_are_bounded() {
        let anchor = [300.0, 280.0];
        for kind in KINDS {
            let c = clip(kind);
            for step in 0..12 {
                let frame = c.start + c.duration * (step as f32 / 12.0);
                let n = clip_particles(&c, frame, anchor).len();
                assert!(n <= 64, "{kind:?} emitted {n} particles at frame {frame}");
            }
        }
    }
}
