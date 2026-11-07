//! StykAnimate-RS library: model, editor core, formats, exporters.
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
pub mod style;
pub mod svg;
pub mod text_edit;
pub mod touch;
pub mod ui_kit;
pub mod vfx;
