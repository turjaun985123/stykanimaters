//! Keyframe tools section (Rune Phase D, section 31) — the
//! graph-key controls for the selected figure's selected joint:
//! key the angle here, delete the key here, auto tangents, and
//! the engine's tangent presets, plus a live indicator showing
//! whether the current frame already holds a key on the channel.
//! Declared from `main.rs` only — bin-crate code, not part of
//! the engine library.
//!
//! Op map (verified by reading `src/editor.rs` / `src/graph.rs`):
//!
//! * "Key angle here" → `EditorCore::key_graph_angle`
//!   (editor.rs:1203) — keys `Angle(selected_node)` at the
//!   current frame with its current keyframed value.
//! * "Auto tangents" → `EditorCore::smooth_graph_tangents`
//!   (editor.rs:1240). This is the core's one smoothing op: it
//!   runs `graph::smooth_channel` over every channel, which in
//!   turn fits `graph::auto_tangents` (graph.rs:348, :305). The
//!   brief's "auto tangents" and "smooth channel" are the same
//!   op at core level, so there is deliberately ONE button —
//!   two buttons calling one op would be a fake distinction.
//! * Tangent presets → `EditorCore::apply_graph_ease_preset`
//!   (editor.rs:1263), one button per `graph::TangentPreset`
//!   variant (graph.rs:358): Smooth, EaseIn, EaseOut, EaseInOut,
//!   Linear. Scoped to the selected figure's channels. (The
//!   per-key interp modes are section 9's `set_graph_key_interp`
//!   — not duplicated here.)
//! * "Delete key here" → NO `EditorCore` op exists (verified:
//!   the core's graph family is key/smooth/preset/interp/bake/
//!   clear only). The handler below composes the engine's
//!   public API exactly the way `set_graph_key_interp`
//!   (editor.rs:1305) does internally: locate the selected
//!   figure's `Angle(selected_node)` channel, `snapshot()`
//!   (editor.rs:202, pub), then `Channel::remove_key`
//!   (graph.rs:144 — removes the key nearest the frame, within
//!   0.5 frames), then set `status`. Nothing is removed when no
//!   key is near the current frame, and no snapshot is taken
//!   for a no-op.
//! * Copy keys → NOT BUILT: no copy/paste-keys op or key
//!   clipboard exists anywhere in the engine (grep-verified
//!   across editor.rs/graph.rs). Reported as an engine gap
//!   rather than faked.
//!
//! The keyed-frame indicator is a read-only graph query over
//! `core.project.channels` — the same channel
//! `key_graph_angle` authors.

use rune::tok::{type_, Theme};
use rune::{col, row, text, El};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::graph::{Channel, ChannelTarget, Key, TangentPreset};

use crate::chrome_view::btn;
use crate::State;

/// The graph channel this section acts on: the selected
/// figure's `Angle(selected_node)` channel — the one
/// `key_graph_angle` authors and `set_graph_key_interp` edits.
/// `None` when no joint is selected or no channel exists yet.
pub fn selected_angle_channel(core: &EditorCore) -> Option<&Channel> {
    let ni = core.selected_node?;
    core.project
        .channels
        .iter()
        .find(|ch| ch.figure == core.selected_figure && ch.target == ChannelTarget::Angle(ni))
}

/// Whether `keys` holds a key at `frame` (the engine's own
/// tolerance: keys land on whole frames; 0.5 matches
/// `Channel::remove_key`'s nearest-key window, graph.rs:144).
pub fn keyed_here(keys: &[Key], frame: usize) -> bool {
    let f = frame as f32;
    keys.iter().any(|k| (k.frame - f).abs() <= 0.5)
}

/// The tangent preset buttons: every `TangentPreset` variant,
/// in menu order, as (preset, button label, element-key suffix).
pub fn preset_options() -> [(TangentPreset, &'static str, &'static str); 5] {
    [
        (TangentPreset::Smooth, "Smooth", "smooth"),
        (TangentPreset::EaseIn, "Ease in", "ease-in"),
        (TangentPreset::EaseOut, "Ease out", "ease-out"),
        (TangentPreset::EaseInOut, "Ease in-out", "ease-in-out"),
        (TangentPreset::Linear, "Linear", "linear"),
    ]
}

/// "Figure 1 · joint 3" for the current selection, or the
/// prompt to select one.
pub fn target_text(core: &EditorCore) -> String {
    match core.selected_node {
        Some(ni) => format!("Figure {} · joint {}", core.selected_figure + 1, ni),
        None => "No joint selected".to_string(),
    }
}

/// The keyed-frame indicator line: whether the current frame
/// holds a key on the selected angle channel, and how many
/// keys the channel holds in total.
pub fn indicator_text(core: &EditorCore) -> String {
    if core.selected_node.is_none() {
        return "Select a joint to edit its angle keys.".to_string();
    }
    match selected_angle_channel(core) {
        None => format!("No keys yet on this joint · frame {}", core.current + 1),
        Some(ch) => {
            let n = ch.keys.len();
            let noun = if n == 1 { "key" } else { "keys" };
            if keyed_here(&ch.keys, core.current) {
                format!("● Key at frame {} · {n} {noun} on channel", core.current + 1)
            } else {
                format!("○ No key at frame {} · {n} {noun} on channel", core.current + 1)
            }
        }
    }
}

/// Delete the key at (nearest to) the current frame on the
/// selected angle channel. Returns whether a key was removed.
/// See the module docs for why this is composed from the
/// engine's public API instead of calling a core op.
pub fn delete_key_here_core(core: &mut EditorCore) -> bool {
    let Some(ni) = core.selected_node else {
        core.status = "Select a joint first, then Delete key.".into();
        return false;
    };
    let fi = core.selected_figure;
    let frame = core.current as f32;
    let Some(ci) = core
        .project
        .channels
        .iter()
        .position(|ch| ch.figure == fi && ch.target == ChannelTarget::Angle(ni))
    else {
        core.status = format!("No graph keys for figure {} joint {}.", fi + 1, ni);
        return false;
    };
    if !keyed_here(&core.project.channels[ci].keys, core.current) {
        core.status = format!("No key at frame {} to delete.", core.current + 1);
        return false;
    }
    core.snapshot();
    let removed = core.project.channels[ci].remove_key(frame);
    if removed {
        core.status = format!("Deleted graph key at frame {}.", core.current + 1);
    }
    removed
}

/// `State` wrapper for [`delete_key_here_core`].
pub fn delete_key_here(s: &mut State) {
    delete_key_here_core(&mut s.core);
}

/// The keyframe tools section: header, selection target,
/// keyed-frame indicator, the key/delete/auto buttons, and
/// the five tangent preset buttons (scoped by the op to the
/// selected figure). Buttons highlight with real state: Delete
/// is accent-filled exactly when a key sits at this frame.
pub fn keyframe_section(s: &State, t: &Theme) -> El {
    let core = &s.core;
    let keyed = selected_angle_channel(core).is_some_and(|ch| keyed_here(&ch.keys, core.current));
    let mut presets = row().key("keyframe-presets").gap(6.0);
    for (preset, label, suffix) in preset_options() {
        presets = presets.child(btn(
            &format!("key-preset-{suffix}"),
            label,
            false,
            t,
            move |s: &mut State| s.core.apply_graph_ease_preset(preset),
        ));
    }
    col()
        .key("keyframe-tools")
        .w_full()
        .gap(6.0)
        .child(text("Keyframes").font(type_::TITLE).color(t.text))
        .child(
            text(target_text(core))
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(
            text(indicator_text(core))
                .font(type_::LABEL)
                .color(t.text_secondary),
        )
        .child(
            row()
                .key("keyframe-actions")
                .gap(6.0)
                .child(btn("key-angle", "Key angle here", false, t, |s| {
                    s.core.key_graph_angle()
                }))
                .child(btn("key-delete", "Delete key here", keyed, t, |s| {
                    delete_key_here(s)
                }))
                .child(btn("key-auto", "Auto tangents", false, t, |s| {
                    s.core.smooth_graph_tangents()
                })),
        )
        .child(presets)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyed_here_matches_whole_frames_within_half_a_frame() {
        let keys = [Key::flat(0.0, 10.0), Key::flat(4.0, 20.0), Key::flat(9.0, 30.0)];
        assert!(keyed_here(&keys, 0));
        assert!(keyed_here(&keys, 4));
        assert!(keyed_here(&keys, 9));
        assert!(!keyed_here(&keys, 1));
        assert!(!keyed_here(&keys, 5));
        assert!(!keyed_here(&[], 0));
    }

    #[test]
    fn preset_options_cover_every_tangent_preset_once() {
        let opts = preset_options();
        assert_eq!(opts.len(), 5);
        let mut seen: Vec<TangentPreset> = opts.iter().map(|(p, _, _)| *p).collect();
        seen.sort_by_key(|p| format!("{p:?}"));
        seen.dedup();
        assert_eq!(seen.len(), 5, "presets must be distinct: {opts:?}");
        assert!(opts.iter().any(|(p, _, _)| *p == TangentPreset::Smooth));
        assert!(opts.iter().any(|(p, _, _)| *p == TangentPreset::Linear));
        // Element-key suffixes must be unique too.
        let mut suffixes: Vec<&str> = opts.iter().map(|(_, _, s)| *s).collect();
        suffixes.sort();
        suffixes.dedup();
        assert_eq!(suffixes.len(), 5);
    }

    #[test]
    fn indicator_prompts_for_a_joint_when_none_selected() {
        let core = EditorCore::new();
        // A fresh core may or may not preselect a node; force
        // the no-selection case the text is written for.
        let mut core = core;
        core.selected_node = None;
        assert_eq!(target_text(&core), "No joint selected");
        assert!(indicator_text(&core).starts_with("Select a joint"));
    }

    #[test]
    fn delete_key_here_removes_only_the_key_at_the_current_frame() {
        let mut core = EditorCore::new();
        core.selected_node = Some(1);
        core.key_graph_angle();
        let at_first = core.current;
        assert!(
            selected_angle_channel(&core).is_some_and(|ch| keyed_here(&ch.keys, at_first)),
            "key_graph_angle must key the current frame"
        );
        assert!(indicator_text(&core).starts_with('●'));

        // Key a second frame, then delete only the first.
        core.goto((at_first + 2) % core.frame_count().max(3));
        core.key_graph_angle();
        core.goto(at_first);
        assert!(delete_key_here_core(&mut core));
        let ch = selected_angle_channel(&core).expect("channel survives key deletion");
        assert!(!keyed_here(&ch.keys, at_first), "the key here is gone");
        assert_eq!(ch.keys.len(), 1, "the other frame's key is untouched");
        assert!(indicator_text(&core).starts_with('○'));

        // Deleting again is a clean no-op (status only).
        assert!(!delete_key_here_core(&mut core));
    }

    #[test]
    fn delete_key_here_noops_without_a_selection() {
        let mut core = EditorCore::new();
        core.selected_node = None;
        assert!(!delete_key_here_core(&mut core));
        assert!(core.status.contains("Select a joint"));
    }
}
