//! Editor UI components.
//!
//! This folder holds all of the editor's UI component modules:
//! the shared design kit, panels, overlays and strips that
//! `crate::app` assembles into the editor window. Each component
//! lives in its own file here, named after what it renders.
//!
//! The rest of the crate refers to these modules by their
//! historical flat names (`crate::ui_kit`, `crate::ui_toast`, …).
//! To keep every one of those imports working unchanged, `lib.rs`
//! declares this folder as a module tree.
pub mod badge;
pub mod banner;
pub mod breadcrumb;
pub mod clip_row;
pub mod color;
pub mod color_field;
pub mod dialog;
pub mod dope;
pub mod easing;
pub mod empty;
pub mod file_row;
pub mod frame_chip;
pub mod gauge;
pub mod graph_axis;
pub mod help;
pub mod hud;
pub mod inspector;
pub mod kbd;
pub mod kit;
pub mod layer_row;
pub mod legend;
pub mod node_row;
pub mod overlay;
pub mod panel;
pub mod progress_steps;
pub mod rows;
pub mod scrubber;
pub mod segmented;
pub mod slider;
pub mod sparkline;
pub mod stat_grid;
pub mod stepper;
pub mod swatch_grid;
pub mod tabs;
pub mod timecode;
pub mod timeline_ruler;
pub mod toast;
pub mod toolbar;
pub mod waveform;
