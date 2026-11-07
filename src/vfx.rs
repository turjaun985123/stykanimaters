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

    const KINDS: [ClipKind; 6] = [
        ClipKind::Burst,
        ClipKind::Sparkle,
        ClipKind::Smoke,
        ClipKind::Beam,
        ClipKind::Aura,
        ClipKind::Trail,
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
