//! Slim status strip for the bottom of the shell.
//!
//! Wave 15 gives the application a real status bar: project
//! name, playhead position, frame rate, viewport zoom, the
//! current selection and the playback state, in one quiet row
//! that replaces status text scattered across the window.
//!
//! Presentation only, like [`crate::ui_hud`]: the view builds
//! a [`StatusState`] snapshot each render and this module lays
//! it out. Every label string and accent decision lives in the
//! pure [`status_items`] seam, so tests can pin the strip's
//! contents without a window.

use crate::ui_kit::{info_chip, C_SURFACE, SP_MD, SP_SM, SP_XS, TEXT_XS};
use gpui::*;

/// Snapshot of the session state the status bar displays.
/// Built by the view in `render()` from `EditorCore` / project
/// fields; all values are display-ready.
pub struct StatusState {
    /// Name of the open project (its display name, not a path).
    pub project_name: String,
    /// Current frame, 0-based; shown 1-based.
    pub frame: usize,
    /// Total frame count; shown as the denominator.
    pub frames: usize,
    /// Project frame rate; shown rounded to a whole number.
    pub fps: f32,
    /// Viewport zoom as a percentage (100 = actual size).
    pub zoom_pct: f32,
    /// Name of the current selection (figure, joint, clip…);
    /// `None` hides the chip.
    pub selection: Option<String>,
    /// Whether playback is running; shown as a chip only then.
    pub playing: bool,
}

/// The chips the status bar shows for `state`, as `(label,
/// active)` pairs in render order — the pure seam behind
/// [`status_strip`]. Order is fixed: project name, frame
/// position (accent — the bar's anchor readout), frame rate,
/// zoom, then the engaged-state chips: selection when there
/// is one, and "Playing" while playback runs.
pub fn status_items(state: &StatusState) -> Vec<(String, bool)> {
    let mut items = vec![
        (state.project_name.clone(), false),
        (format!("Frame {} / {}", state.frame + 1, state.frames), true),
        (format!("{:.0} fps", state.fps), false),
        (format!("Zoom {:.0}%", state.zoom_pct), false),
    ];
    if let Some(selection) = &state.selection {
        items.push((selection.clone(), true));
    }
    if state.playing {
        items.push(("Playing".to_string(), true));
    }
    items
}

/// Build the status strip: a surface-toned row of
/// [`info_chip`]s laid out from [`status_items`], the single
/// source of truth for which chips appear, their labels and
/// their accent state.
pub fn status_strip(state: &StatusState) -> Div {
    let mut row = div()
        .flex()
        .items_center()
        .gap(px(SP_SM))
        .bg(rgb(C_SURFACE))
        .px(px(SP_MD))
        .py(px(SP_XS))
        .text_size(px(TEXT_XS));
    for (label, active) in status_items(state) {
        row = row.child(info_chip(label, active));
    }
    row
}

#[cfg(test)]
mod tests {
    use super::{status_items, status_strip, StatusState};

    /// A paused session with nothing selected.
    fn idle_state() -> StatusState {
        StatusState {
            project_name: "Demo".to_string(),
            frame: 0,
            frames: 12,
            fps: 30.0,
            zoom_pct: 100.0,
            selection: None,
            playing: false,
        }
    }

    #[test]
    fn idle_items_are_the_four_base_readouts_in_order() {
        assert_eq!(
            status_items(&idle_state()),
            vec![
                ("Demo".to_string(), false),
                ("Frame 1 / 12".to_string(), true),
                ("30 fps".to_string(), false),
                ("Zoom 100%".to_string(), false),
            ]
        );
    }

    #[test]
    fn frame_position_is_shown_one_based() {
        let mut state = idle_state();
        state.frame = 11;
        assert_eq!(status_items(&state)[1].0, "Frame 12 / 12");
    }

    #[test]
    fn fps_and_zoom_round_to_whole_numbers() {
        let mut state = idle_state();
        state.fps = 23.976;
        state.zoom_pct = 66.6;
        let items = status_items(&state);
        assert_eq!(items[2].0, "24 fps");
        assert_eq!(items[3].0, "Zoom 67%");
    }

    #[test]
    fn selection_appends_an_accent_chip_after_zoom() {
        let mut state = idle_state();
        state.selection = Some("Styk".to_string());
        let items = status_items(&state);
        assert_eq!(items.len(), 5);
        assert_eq!(items[4], ("Styk".to_string(), true));
    }

    #[test]
    fn playing_appends_the_playing_chip_last() {
        let mut state = idle_state();
        state.playing = true;
        let items = status_items(&state);
        assert_eq!(items.len(), 5);
        assert_eq!(items[4], ("Playing".to_string(), true));
    }

    #[test]
    fn selection_and_playing_both_show_selection_first() {
        let mut state = idle_state();
        state.selection = Some("Arm".to_string());
        state.playing = true;
        let items = status_items(&state);
        assert_eq!(items.len(), 6);
        assert_eq!(items[4], ("Arm".to_string(), true));
        assert_eq!(items[5], ("Playing".to_string(), true));
    }

    #[test]
    fn status_strip_builds_for_idle_and_playing_states() {
        // Construction only — a built Div's contents are not
        // inspectable from a unit test; the labels themselves
        // are pinned by the status_items tests above.
        let _idle = status_strip(&idle_state());
        let mut playing = idle_state();
        playing.playing = true;
        let _playing = status_strip(&playing);
    }
}
