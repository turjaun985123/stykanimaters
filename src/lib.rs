//! StykAnimate-RS library: model, editor core, formats, exporters.
#![recursion_limit = "256"]
//! The editor binary (src/main.rs) and the MCP server binary
//! (src/bin/mcp_server.rs) are both built on this library.

pub mod audio;
pub mod audio_edit;
pub mod audio_ops;
pub mod audio_play;
pub mod dialogs;
pub mod draw_ops;
pub mod editor;
pub mod export;
pub mod fig3d;
pub mod figure_ops;
pub mod frame_swap;
pub mod fx_edit;
pub mod fx_ops;
pub mod graph;
pub mod graph_keys;
pub mod graph_panel;
pub mod i18n;
pub mod icon_path;
pub mod layers;
pub mod mcp;
pub mod mcp_io;
pub mod mcp_tools;
pub mod media;
pub mod model;
pub mod nodes;
pub mod nodes_anim;
pub mod paint;
pub mod particle_ops;
pub mod pivot;
pub mod projects;
pub mod shell;
pub mod sprite_ops;
pub mod stats;
pub mod style;
pub mod svg;
pub mod text_edit;
pub mod touch;
pub mod vfx;
