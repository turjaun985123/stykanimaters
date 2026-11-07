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

/// Build the HUD strip: a wrapping row of chips — mode (accent
/// whenever the mode is not plain posing), tool, camera, then
/// the engaged-state chips (weather, loop, onion) with accent
/// backgrounds.
pub fn hud_strip(s: &HudState) -> Div {
    let mut row = top_bar()
        .flex_wrap()
        .child(info_chip(s.mode.clone(), s.mode != "Pose"));
    if let Some(tool) = &s.tool {
        row = row.child(info_chip(tool.clone(), false));
    }
    row = row.child(info_chip(s.camera.clone(), false));
    if let Some(weather) = &s.weather {
        row = row.child(info_chip(weather.clone(), true));
    }
    if let Some((a, b)) = s.loop_range {
        row = row.child(info_chip(format!("Loop {}–{}", a + 1, b + 1), true));
    }
    if s.onion {
        row = row.child(info_chip("Onion".to_string(), true));
    }
    row
}
