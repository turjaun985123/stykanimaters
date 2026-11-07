//! Canvas HUD: a slim strip of state chips that sits directly
//! above the framed canvas in the stage column.
//!
//! Mode state — drawing, camera mode, a 3D session, a set loop
//! range, active weather — is otherwise scattered across toolbar
//! buttons you have to hunt for. The HUD gathers the live state
//! into one row of [`info_chip`] pills over the canvas, so the
//! strip doubles as the canvas header: even in the plainest
//! state it still shows the mode and camera chips.
//!
//! Presentation only, like [`crate::ui_kit`]: the view assembles
//! a [`HudState`] snapshot each render and this module just lays
//! the chips out. No editor knowledge lives here — every field is
//! a plain string/flag the caller already formatted.

use crate::ui_kit::{info_chip, top_bar};
use gpui::*;

/// Snapshot of the editor state the HUD displays. Built by the
/// view in `render()` from `EditorCore` fields; all strings are
/// display-ready.
pub struct HudState {
    /// Current interaction mode: "3D", "Draw", "Camera" or "Pose".
    pub mode: String,
    /// Active drawing tool ("Brush: …" / "Eraser") while draw
    /// mode is on; `None` hides the chip.
    pub tool: Option<String>,
    /// Camera readout (position + zoom), always shown.
    pub camera: String,
    /// Weather summary ("Rain · 90") when weather is set on the
    /// project; `None` hides the chip.
    pub weather: Option<String>,
    /// Playback loop range as 0-based frame indices when set;
    /// shown 1-based. `None` hides the chip.
    pub loop_range: Option<(usize, usize)>,
    /// Onion skinning on/off; the chip shows only when on.
    pub onion: bool,
}

/// The chips the HUD shows for `state`, as `(label, active)`
/// pairs in render order — the pure seam behind [`hud_strip`].
/// Every label string and accent decision lives here so tests
/// can pin the strip's contents without a window: mode (accent
/// whenever the mode is not plain posing), tool, camera, then
/// the engaged-state chips (weather, loop, onion) with accent
/// backgrounds.
pub fn chip_specs(state: &HudState) -> Vec<(String, bool)> {
    let mut specs = vec![(state.mode.clone(), state.mode != "Pose")];
    if let Some(tool) = &state.tool {
        specs.push((tool.clone(), false));
    }
    specs.push((state.camera.clone(), false));
    if let Some(weather) = &state.weather {
        specs.push((weather.clone(), true));
    }
    if let Some((a, b)) = state.loop_range {
        specs.push((format!("Loop {}–{}", a + 1, b + 1), true));
    }
    if state.onion {
        specs.push(("Onion".to_string(), true));
    }
    specs
}

/// Build the HUD strip: a wrapping row of chips laid out from
/// [`chip_specs`], the single source of truth for which chips
/// appear, their labels and their accent state.
pub fn hud_strip(s: &HudState) -> Div {
    let mut row = top_bar().flex_wrap();
    for (label, active) in chip_specs(s) {
        row = row.child(info_chip(label, active));
    }
    row
}

#[cfg(test)]
mod tests {
    use super::{chip_specs, HudState};

    /// A plain posing state: every optional chip off.
    fn minimal() -> HudState {
        HudState {
            mode: "Pose".to_string(),
            tool: None,
            camera: "Cam 0,0".to_string(),
            weather: None,
            loop_range: None,
            onion: false,
        }
    }

    /// Build an expected chip list from string literals.
    fn specs(pairs: &[(&str, bool)]) -> Vec<(String, bool)> {
        pairs
            .iter()
            .map(|(label, active)| (label.to_string(), *active))
            .collect()
    }

    #[test]
    fn minimal_state_shows_mode_and_camera_only() {
        assert_eq!(
            chip_specs(&minimal()),
            specs(&[("Pose", false), ("Cam 0,0", false)])
        );
    }

    #[test]
    fn non_pose_mode_chip_is_accented() {
        let mut s = minimal();
        s.mode = "Draw".to_string();
        assert_eq!(chip_specs(&s)[0], ("Draw".to_string(), true));
    }

    #[test]
    fn tool_chip_sits_between_mode_and_camera_inactive() {
        let mut s = minimal();
        s.tool = Some("Brush: Soft".to_string());
        assert_eq!(
            chip_specs(&s),
            specs(&[("Pose", false), ("Brush: Soft", false), ("Cam 0,0", false)])
        );
    }

    #[test]
    fn weather_chip_toggles_with_weather() {
        let mut s = minimal();
        assert_eq!(chip_specs(&s).len(), 2);
        s.weather = Some("Rain · 90".to_string());
        assert_eq!(
            chip_specs(&s),
            specs(&[("Pose", false), ("Cam 0,0", false), ("Rain · 90", true)])
        );
    }

    #[test]
    fn loop_chip_shows_one_based_range() {
        let mut s = minimal();
        s.loop_range = Some((2, 5));
        assert_eq!(
            chip_specs(&s),
            specs(&[("Pose", false), ("Cam 0,0", false), ("Loop 3–6", true)])
        );
    }

    #[test]
    fn onion_chip_only_when_on_and_accented() {
        let mut s = minimal();
        assert!(!chip_specs(&s).iter().any(|(label, _)| label == "Onion"));
        s.onion = true;
        let chips = chip_specs(&s);
        assert_eq!(chips[chips.len() - 1], ("Onion".to_string(), true));
    }

    #[test]
    fn full_state_chip_order() {
        let s = HudState {
            mode: "3D".to_string(),
            tool: Some("Eraser".to_string()),
            camera: "Cam 10,20".to_string(),
            weather: Some("Snow · 40".to_string()),
            loop_range: Some((0, 9)),
            onion: true,
        };
        assert_eq!(
            chip_specs(&s),
            specs(&[
                ("3D", true),
                ("Eraser", false),
                ("Cam 10,20", false),
                ("Snow · 40", true),
                ("Loop 1–10", true),
                ("Onion", true),
            ])
        );
    }
}
