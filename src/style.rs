//! Pure helpers for per-segment rendering styles (see
//! `model::NodeStyle`): cap cycling for the editor, taper labels,
//! and the colour interpolation / blending the rasterizer and the
//! SVG exporter share. Everything here is deterministic.

use crate::model::CapKind;

/// Cycle a segment's end cap: Round → Square → Point → Round.
pub fn cycle_cap(c: CapKind) -> CapKind {
    match c {
        CapKind::Round => CapKind::Square,
        CapKind::Square => CapKind::Point,
        CapKind::Point => CapKind::Round,
    }
}

/// Short human label for a taper value (1.0 = uniform width),
/// e.g. `Taper 100%` or `Taper 25%`.
pub fn taper_label(t: f32) -> String {
    format!("Taper {:.0}%", t * 100.0)
}

/// Interpolate two `0xRRGGBB` colours; `t` is clamped to 0..=1
/// and channels are rounded to the nearest step.
pub fn lerp_color(a: u32, b: u32, t: f32) -> u32 {
    let t = t.clamp(0.0, 1.0);
    let ch = |shift: u32| -> u32 {
        let x = ((a >> shift) & 0xFF) as f32;
        let y = ((b >> shift) & 0xFF) as f32;
        (x + (y - x) * t).round() as u32
    };
    (ch(16) << 16) | (ch(8) << 8) | ch(0)
}

/// Blend `src` over the current pixel `dst` with the given
/// opacity (clamped to 0..=1). Opacity 1 returns `src` exactly,
/// which keeps default-styled rendering pixel-identical.
pub fn blend_over(dst: [u8; 3], src: [u8; 3], opacity: f32) -> [u8; 3] {
    let o = opacity.clamp(0.0, 1.0);
    [
        (dst[0] as f32 * (1.0 - o) + src[0] as f32 * o) as u8,
        (dst[1] as f32 * (1.0 - o) + src[1] as f32 * o) as u8,
        (dst[2] as f32 * (1.0 - o) + src[2] as f32 * o) as u8,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cap_cycles_through_all_three_and_back() {
        assert_eq!(cycle_cap(CapKind::Round), CapKind::Square);
        assert_eq!(cycle_cap(CapKind::Square), CapKind::Point);
        assert_eq!(cycle_cap(CapKind::Point), CapKind::Round);
    }

    #[test]
    fn taper_labels_show_percent() {
        assert_eq!(taper_label(1.0), "Taper 100%");
        assert_eq!(taper_label(0.25), "Taper 25%");
        assert_eq!(taper_label(0.0), "Taper 0%");
    }

    #[test]
    fn lerp_color_hits_endpoints_and_midpoint() {
        assert_eq!(lerp_color(0xFF0000, 0x0000FF, 0.0), 0xFF0000);
        assert_eq!(lerp_color(0xFF0000, 0x0000FF, 1.0), 0x0000FF);
        // 255 * 0.5 = 127.5 rounds to 128 on both fading channels.
        assert_eq!(lerp_color(0xFF0000, 0x0000FF, 0.5), 0x800080);
        assert_eq!(lerp_color(0x000000, 0xFFFFFF, 0.5), 0x808080);
    }

    #[test]
    fn lerp_color_clamps_out_of_range_t() {
        assert_eq!(lerp_color(0x102030, 0x405060, -1.0), 0x102030);
        assert_eq!(lerp_color(0x102030, 0x405060, 2.0), 0x405060);
    }

    #[test]
    fn blend_over_respects_opacity() {
        assert_eq!(blend_over([0, 0, 0], [255, 0, 0], 1.0), [255, 0, 0]);
        assert_eq!(blend_over([0, 0, 0], [255, 0, 0], 0.0), [0, 0, 0]);
        // 255 * 0.5 = 127.5 truncates to 127 in the u8 cast.
        assert_eq!(blend_over([0, 0, 0], [255, 0, 0], 0.5), [127, 0, 0]);
        assert_eq!(blend_over([200, 100, 0], [0, 100, 200], 0.5), [100, 100, 100]);
    }
}
