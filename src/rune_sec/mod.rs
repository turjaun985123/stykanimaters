//! Rune Phase D section modules — one file per feature area
//! of the editor shell, each owning its panel builders and
//! view-state. Declared from `main.rs` (bin-crate code, not
//! part of the engine library). Every control in these files
//! calls a real `EditorCore` op or writes the same public
//! core field the GPUI app wrote; sections that found no op
//! for a briefed control say so in their module docs instead
//! of faking one.
//!
//! `snap_tests.rs` is deliberately NOT declared here: it is
//! test-only and `main.rs` mounts it under `#[cfg(test)]`
//! with an explicit `#[path]`.

pub mod audio_panel;
pub mod camera_panel;
pub mod clips_panel;
pub mod command_palette;
pub mod design;
pub mod draw_tools;
pub mod easing_panel;
pub mod export_panel;
pub mod figure_add;
pub mod figures_panel;
pub mod frame_ops;
pub mod fx_browser;
pub mod fx_clips;
pub mod fig3_preview;
pub mod particles_panel;
pub mod graph_panel;
pub mod help_panel;
pub mod home_panel;
pub mod i18n_wire;
pub mod icon_btn;
pub mod icon_data;
pub mod icons;
pub mod inspector_figure;
pub mod inspector_segment;
pub mod inspector_style;
pub mod keyframe_tools;
pub mod layers_panel;
pub mod library_panel;
pub mod menus;
pub mod onion_controls;
pub mod playback_controls;
pub mod pose_tools;
pub mod project_tools;
pub mod scene_panel;
pub mod scenes;
pub mod settings_popover;
pub mod stage_camera;
pub mod stage_fx;
pub mod stage_fig3;
pub mod stage_layers;
pub mod stage_text;
pub mod stats_view;
pub mod status_strip;
pub mod stroke_inspector;
pub mod text_tool;
pub mod three_d_panel;
pub mod timeline_marks;
pub mod toasts;
pub mod waveform_view;
pub mod workspaces;
