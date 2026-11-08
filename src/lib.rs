//! StykAnimate-RS library: model, editor core, formats, exporters.
#![recursion_limit = "256"]
//! The GPUI binary (src/main.rs) and the MCP server binary
//! (src/bin/mcp_server.rs) are both built on this library.

pub mod app;
pub mod audio;
pub mod audio_edit;
pub mod dialogs;
pub mod draw_ops;
pub mod editor;
pub mod export;
pub mod fig3d;
pub mod fx_edit;
pub mod graph;
pub mod graph_panel;
pub mod i18n;
pub mod layers;
pub mod mcp;
pub mod mcp_io;
pub mod mcp_tools;
pub mod media;
pub mod model;
pub mod nodes;
pub mod nodes_anim;
pub mod paint;
pub mod pivot;
pub mod projects;
pub mod shell;
pub mod stats;
pub mod style;
pub mod svg;
pub mod text_edit;
pub mod touch;
pub mod ui;
// Flat aliases: the whole crate (and the components themselves)
// refer to UI modules by their historical ui_* names.
pub use ui::badge as ui_badge;
pub use ui::banner as ui_banner;
pub use ui::breadcrumb as ui_breadcrumb;
pub use ui::clip_row as ui_clip_row;
pub use ui::color as ui_color;
pub use ui::color_field as ui_color_field;
pub use ui::dialog as ui_dialog;
pub use ui::dope as ui_dope;
pub use ui::easing as ui_easing;
pub use ui::empty as ui_empty;
pub use ui::file_row as ui_file_row;
pub use ui::frame_chip as ui_frame_chip;
pub use ui::gauge as ui_gauge;
pub use ui::graph_axis as ui_graph_axis;
pub use ui::help as ui_help;
pub use ui::hud as ui_hud;
pub use ui::inspector as ui_inspector;
pub use ui::kbd as ui_kbd;
pub use ui::kit as ui_kit;
pub use ui::layer_row as ui_layer_row;
pub use ui::legend as ui_legend;
pub use ui::menu as ui_menu;
pub use ui::node_row as ui_node_row;
pub use ui::overlay as ui_overlay;
pub use ui::panel as ui_panel;
pub use ui::progress_steps as ui_progress_steps;
pub use ui::rows as ui_rows;
pub use ui::scrubber as ui_scrubber;
pub use ui::segmented as ui_segmented;
pub use ui::slider as ui_slider;
pub use ui::sparkline as ui_sparkline;
pub use ui::stat_grid as ui_stat_grid;
pub use ui::status_bar as ui_status_bar;
pub use ui::stepper as ui_stepper;
pub use ui::swatch_grid as ui_swatch_grid;
pub use ui::tabs as ui_tabs;
pub use ui::timecode as ui_timecode;
pub use ui::timeline_ruler as ui_timeline_ruler;
pub use ui::toast as ui_toast;
pub use ui::toolbar as ui_toolbar;
pub use ui::waveform as ui_waveform;
pub mod vfx;
