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

    const KINDS: [ClipKind; 11] = [
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

    // ---- Weather ----

    fn weather(kind: WeatherKind, amount: f32, wind: f32) -> Weather {
        Weather { kind, amount, wind }
    }

    const STAGE: [f32; 2] = [960.0, 720.0];
    const WEATHER_KINDS: [WeatherKind; 3] =
        [WeatherKind::Rain, WeatherKind::Snow, WeatherKind::Fireflies];

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
