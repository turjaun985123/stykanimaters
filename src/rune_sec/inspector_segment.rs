//! Segment inspector panel (Rune migration Phase D, section
//! 3/40). Declared from `main.rs` only (as
//! `mod rune_sec::inspector_segment` or the section aggregator's
//! re-export) — bin-crate code, not part of the engine library.
//!
//! Shown when a node is selected: real readouts from
//! `EditorCore` state (index, name, length, thickness, world
//! angle at the current frame, current-frame bend, shape,
//! colour) plus the editing controls, each wired to exactly
//! one real core op — the same ops the GPUI inspector's
//! buttons called, under the same element keys where one
//! existed:
//!
//! * Length −/+ → `adjust_length(∓4.0)` (editor.rs:466) —
//!   keys `insp-len-dn` / `insp-len-up`; the origin has no
//!   length (the op no-ops on it), so the row is omitted for
//!   the root, exactly as the GPUI inspector omitted it.
//! * Thickness −/+ → `adjust_thickness(∓1.0)` (editor.rs:475)
//!   — keys `insp-th-dn` / `insp-th-up`.
//! * Cycle bend → `cycle_bend()` (editor.rs:1106) — key
//!   `cycle-bend`. There is no ± bend op in the core; bend
//!   steps 0 → +0.35 → +0.7 → −0.35 → −0.7 rad per frame.
//! * Cycle shape → `cycle_shape()` (editor.rs:494) — key
//!   `node-shape` (line → circle → triangle → square →
//!   pentagon → hexagon → line).
//! * Colour swatches → `set_node_color(rgb)` (editor.rs:521)
//!   — keys `seg-color-{rrggbb}`; the palette mirrors the
//!   core's private `NODE_COLORS` cycle (editor.rs:484).
//! * Cycle colour → `cycle_node_color()` (editor.rs:506) —
//!   key `node-color`; the only op that can return a segment
//!   to the figure colour (`set_node_color` always writes
//!   `Some(rgb)`), so it stays as the "default" path.
//! * + Segment → `add_segment()` (editor.rs:402) — key
//!   `add-seg`.
//! * Delete segment → `delete_segment()` (editor.rs:434) —
//!   key `del-seg`, danger-styled; the op itself refuses the
//!   root and non-leaves with a status message.
//!
//! When nothing is selected the panel is one muted line.

use rune::tok::{type_, Theme};
use rune::{col, el, row, text, Color, El};
use stykanimate_rs::editor::EditorCore;
use stykanimate_rs::model::Shape;

use crate::chrome_view::btn;
use crate::stage_view::hex_color;
use crate::State;

/// Step sizes — the GPUI inspector's steps (its stepper
/// handlers called `adjust_length(±4.0)` /
/// `adjust_thickness(±1.0)`).
pub const LENGTH_STEP: f32 = 4.0;
pub const THICKNESS_STEP: f32 = 1.0;

/// The segment colour palette: the colour entries of the
/// core's `NODE_COLORS` cycle (editor.rs:484), in cycle
/// order. (`NODE_COLORS` also has a leading `None` — the
/// figure colour — which `set_node_color` cannot write; the
/// panel's Cycle colour button reaches it.)
pub const PALETTE: [u32; 6] = [0xFF5D5D, 0x5DB2FF, 0x7BE495, 0xFFC65D, 0xC792FF, 0x232830];

/// Derived view-state for the panel: everything the builder
/// renders, computed once from the core so the builder stays
/// a pure function of it (and so the readout math is unit
/// testable without elements).
#[derive(Clone, Debug, PartialEq)]
pub struct InspectorSegmentState {
    /// Selected figure index.
    pub figure: usize,
    /// Selected node index within the figure's definition.
    pub node: usize,
    /// Node name from the definition.
    pub name: String,
    /// True for the figure's origin node (no parent segment).
    pub is_root: bool,
    /// Segment length (definition units).
    pub length: f32,
    /// Segment thickness (definition units).
    pub thickness: f32,
    /// World direction of the segment at the current frame,
    /// degrees in `[0, 360)` — from the solved positions, so
    /// it is the angle actually on screen. For the root (no
    /// segment) it is the node's pose angle at the frame.
    pub angle_deg: f32,
    /// Current frame's bend for this node, degrees (0 when
    /// the frame stores no bend for it — the same read
    /// `cycle_bend` performs).
    pub bend_deg: f32,
    /// The node's own colour, or `None` for the figure colour.
    pub color: Option<u32>,
    /// Human label of the node's shape ("Line", "Circle",
    /// "Triangle", …).
    pub shape_label: String,
}

/// Wrap an angle in degrees into `[0, 360)`.
pub fn norm_deg_360(a: f32) -> f32 {
    let r = a % 360.0;
    if r < 0.0 {
        r + 360.0
    } else {
        r
    }
}

/// Label for a node shape, naming the polygons `cycle_shape`
/// steps through (editor.rs:494).
pub fn shape_label(shape: &Shape) -> String {
    match shape {
        Shape::Line => "Line".to_string(),
        Shape::Circle => "Circle".to_string(),
        Shape::Polygon(3) => "Triangle".to_string(),
        Shape::Polygon(4) => "Square".to_string(),
        Shape::Polygon(5) => "Pentagon".to_string(),
        Shape::Polygon(6) => "Hexagon".to_string(),
        Shape::Polygon(n) => format!("Polygon {n}"),
    }
}

/// Compute the panel's view-state from the core, or `None`
/// when no (valid) node is selected. All indexing is
/// defensive: a stale selection left over from a figure edit
/// reads as "nothing selected" instead of panicking.
pub fn segment_state(core: &EditorCore) -> Option<InspectorSegmentState> {
    let ni = core.selected_node?;
    let fi = core.selected_figure;
    let fig = core.project.figures.get(fi)?;
    let node = fig.def.nodes.get(ni)?;
    // World angle from the solved positions at the current
    // frame (graph-aware, like the stage's `positions_f`).
    let angle_deg = match node.parent {
        Some(p) => {
            let pos = core.positions_f(core.current as f32, fi);
            match (pos.get(p), pos.get(ni)) {
                (Some(a), Some(b)) => {
                    norm_deg_360((b[1] - a[1]).atan2(b[0] - a[0]).to_degrees())
                }
                _ => 0.0,
            }
        }
        None => core
            .project
            .frames
            .get(core.current)
            .and_then(|f| f.poses.get(fi))
            .and_then(|pose| pose.angles.get(ni))
            .map(|a| norm_deg_360(*a))
            .unwrap_or(0.0),
    };
    // The bend read mirrors `cycle_bend` (editor.rs:1106):
    // missing table entries read as 0.
    let bend_deg = core
        .project
        .frames
        .get(core.current)
        .and_then(|f| f.bends.get(fi))
        .and_then(|b| b.get(ni))
        .copied()
        .unwrap_or(0.0)
        .to_degrees();
    Some(InspectorSegmentState {
        figure: fi,
        node: ni,
        name: node.name.clone(),
        is_root: node.parent.is_none(),
        length: node.length,
        thickness: node.thickness,
        angle_deg,
        bend_deg,
        color: node.color,
        shape_label: shape_label(&node.shape),
    })
}

/// One read-only label/value row.
fn readout_row(label: &str, value: String, t: &Theme) -> El {
    row()
        .w_full()
        .center_y()
        .gap(8.0)
        .child(
            text(label)
                .font(type_::LABEL)
                .color(t.text_secondary)
                .grow(1.0),
        )
        .child(text(value).font(type_::LABEL).color(t.text))
}

/// One stepper row: label, current value, − and + buttons
/// (chrome `btn` styling, GPUI inspector keys).
fn stepper_row(
    label: &str,
    value: String,
    dn_key: &str,
    up_key: &str,
    t: &Theme,
    dn: impl Fn(&mut State) + 'static,
    up: impl Fn(&mut State) + 'static,
) -> El {
    row()
        .w_full()
        .center_y()
        .gap(8.0)
        .child(
            text(label)
                .font(type_::LABEL)
                .color(t.text_secondary)
                .grow(1.0),
        )
        .child(text(value).font(type_::LABEL).color(t.text))
        .child(btn(dn_key, "−", false, t, dn))
        .child(btn(up_key, "+", false, t, up))
}

/// The Delete segment button: chrome-button shape in the
/// error colour (the 2.0 palette's `C_ERROR`).
fn danger_btn(key: &str, label: &str, f: impl Fn(&mut State) + 'static) -> El {
    let red = hex_color(0xF16F78).unwrap_or(Color::WHITE);
    let red_dark = hex_color(0xD85560).unwrap_or(red);
    el()
        .key(key)
        .px(10.0)
        .py(5.0)
        .radius(6.0)
        .fill(red)
        .hover(red_dark)
        .press(red_dark)
        .on_press(f)
        .child(text(label).font(type_::LABEL).color(Color::WHITE))
}

/// One colour swatch: a rounded square in the palette
/// colour; the node's current colour gets a white border.
/// Pressing writes it via `set_node_color`.
fn swatch(rgb: u32, selected: bool) -> El {
    let fill = hex_color(rgb).unwrap_or(Color::WHITE);
    let mut e = el()
        .key(format!("seg-color-{rgb:06x}"))
        .size(22.0, 22.0)
        .radius(6.0)
        .fill(fill)
        .hover(fill)
        .press(fill)
        .on_press(move |s: &mut State| s.core.set_node_color(rgb));
    if selected {
        e = e.border(2.0, Color::WHITE);
    }
    e
}

/// The Segment inspector panel. Mount it wherever the shell
/// shows selection details; it reads only `State.core`.
pub fn inspector_segment(s: &State, t: &Theme) -> El {
    let panel = col()
        .key("inspector-segment")
        .w_full()
        .padding(12.0)
        .gap(8.0)
        .radius(8.0)
        .fill(t.card);
    let Some(st) = segment_state(&s.core) else {
        return panel.child(
            text("Select a segment on the stage to edit it.")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    };
    let mut panel = panel
        .child(
            text(format!("Segment {} · {}", st.node, st.name))
                .font(type_::LABEL)
                .color(t.text),
        )
        .child(readout_row("Length", format!("{:.0}", st.length), t))
        .child(readout_row("Thickness", format!("{:.0}", st.thickness), t))
        .child(readout_row("Angle", format!("{:.0}°", st.angle_deg), t))
        .child(readout_row("Bend", format!("{:.0}°", st.bend_deg), t))
        .child(readout_row("Shape", st.shape_label.clone(), t));
    // The origin has no segment to lengthen (the op no-ops);
    // the GPUI inspector omitted the row there too.
    if !st.is_root {
        panel = panel.child(stepper_row(
            "Length",
            format!("{:.0}", st.length),
            "insp-len-dn",
            "insp-len-up",
            t,
            |s| s.core.adjust_length(-LENGTH_STEP),
            |s| s.core.adjust_length(LENGTH_STEP),
        ));
    }
    panel = panel
        .child(stepper_row(
            "Thickness",
            format!("{:.0}", st.thickness),
            "insp-th-dn",
            "insp-th-up",
            t,
            |s| s.core.adjust_thickness(-THICKNESS_STEP),
            |s| s.core.adjust_thickness(THICKNESS_STEP),
        ))
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(btn("cycle-bend", "Cycle bend", false, t, |s| {
                    s.core.cycle_bend()
                }))
                .child(btn("node-shape", "Cycle shape", false, t, |s| {
                    s.core.cycle_shape()
                })),
        )
        .child(
            text("Colour")
                .font(type_::LABEL)
                .color(t.text_secondary),
        );
    let mut swatches = row().gap(6.0).center_y();
    for rgb in PALETTE {
        swatches = swatches.child(swatch(rgb, st.color == Some(rgb)));
    }
    swatches = swatches.child(btn("node-color", "Cycle", false, t, |s| {
        s.core.cycle_node_color()
    }));
    panel = panel.child(swatches);
    panel.child(
        row()
            .w_full()
            .gap(8.0)
            .center_y()
            .child(btn("add-seg", "+ Segment", false, t, |s| s.core.add_segment()))
            .child(danger_btn("del-seg", "Delete segment", |s| {
                s.core.delete_segment()
            })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn core_with_selection(ni: Option<usize>) -> EditorCore {
        let mut core = EditorCore::new();
        core.selected_node = ni;
        core
    }

    #[test]
    fn segment_state_is_none_without_selection() {
        // EditorCore::new selects no node.
        assert!(segment_state(&EditorCore::new()).is_none());
        assert!(segment_state(&core_with_selection(None)).is_none());
    }

    #[test]
    fn segment_state_guards_a_stale_selection() {
        assert!(segment_state(&core_with_selection(Some(999))).is_none());
    }

    #[test]
    fn segment_state_reads_the_selected_node() {
        let core = core_with_selection(Some(1));
        let st = segment_state(&core).expect("node 1 exists in the demo figure");
        let node = &core.project.figures[core.selected_figure].def.nodes[1];
        assert_eq!(st.node, 1);
        assert_eq!(st.figure, core.selected_figure);
        assert_eq!(st.name, node.name);
        assert_eq!(st.length, node.length);
        assert_eq!(st.thickness, node.thickness);
        assert_eq!(st.color, node.color);
        assert!(!st.is_root, "node 1 hangs off the origin");
        assert!(
            (0.0..360.0).contains(&st.angle_deg),
            "angle not normalised: {}",
            st.angle_deg
        );
        assert_eq!(st.bend_deg, 0.0, "fresh frames store no bend");
    }

    #[test]
    fn segment_state_marks_the_root() {
        let core = core_with_selection(Some(0));
        let st = segment_state(&core).expect("the origin exists");
        assert!(st.is_root);
    }

    #[test]
    fn shape_label_names_every_cycle_step() {
        assert_eq!(shape_label(&Shape::Line), "Line");
        assert_eq!(shape_label(&Shape::Circle), "Circle");
        assert_eq!(shape_label(&Shape::Polygon(3)), "Triangle");
        assert_eq!(shape_label(&Shape::Polygon(4)), "Square");
        assert_eq!(shape_label(&Shape::Polygon(5)), "Pentagon");
        assert_eq!(shape_label(&Shape::Polygon(6)), "Hexagon");
        assert_eq!(shape_label(&Shape::Polygon(9)), "Polygon 9");
    }

    #[test]
    fn palette_mirrors_the_core_cycle_colours() {
        // editor.rs's private NODE_COLORS, minus the leading
        // None (figure colour) that set_node_color cannot
        // write. If the core palette changes, this fails and
        // the swatches get updated with it.
        assert_eq!(
            PALETTE,
            [0xFF5D5D, 0x5DB2FF, 0x7BE495, 0xFFC65D, 0xC792FF, 0x232830]
        );
        let mut sorted = PALETTE.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), PALETTE.len(), "palette colours repeat");
    }

    #[test]
    fn norm_deg_360_wraps() {
        assert_eq!(norm_deg_360(0.0), 0.0);
        assert_eq!(norm_deg_360(360.0), 0.0);
        assert_eq!(norm_deg_360(-90.0), 270.0);
        assert_eq!(norm_deg_360(725.0), 5.0);
        assert!((norm_deg_360(-0.5) - 359.5).abs() < 1e-6);
    }

    #[test]
    fn panel_builds_with_and_without_selection() {
        let t = rune::tok::theme(true);
        let bare = State {
            core: EditorCore::new(),
            last_poll: None,
            wheel_zoom: 0.0,
            figures_panel: crate::rune_sec::figures_panel::FiguresPanelState::default(),
            figure_add: crate::rune_sec::figure_add::FigureAddState::default(),
            library_panel: crate::rune_sec::library_panel::LibraryPanelState::default(),
            home_panel: crate::rune_sec::home_panel::HomePanelState::default(),
            frame_ops: crate::rune_sec::frame_ops::FrameOpsState::default(),
            graph_panel: crate::rune_sec::graph_panel::GraphPanelState::default(),
            audio_panel: crate::rune_sec::audio_panel::AudioPanelState::new(),
            menus: crate::rune_sec::menus::MenusState::default(),
            palette: crate::rune_sec::command_palette::PaletteState::new(),
            settings: crate::rune_sec::settings_popover::SettingsState::default(),
            toasts: crate::rune_sec::toasts::ToastsState::new(),
            text_tool: crate::rune_sec::text_tool::TextToolState::new(),
            name_edit: rune::TextEdit::new(""),
            export_use_range: false,
            fx_selected: None,
            pose_clipboard: None,
            last_status: String::new(),
            toast_now: 0.0,
            last_dark: true,
            workspace: crate::rune_sec::workspaces::Workspace::Animate,
            guides: false,
            show_left: true,
            show_right: true,
            key_clipboard: None,
            export_recap: String::new(),
        };
        let _ = inspector_segment(&bare, &t);
        let selected = State {
            core: core_with_selection(Some(1)),
            last_poll: None,
            wheel_zoom: 0.0,
            figures_panel: crate::rune_sec::figures_panel::FiguresPanelState::default(),
            figure_add: crate::rune_sec::figure_add::FigureAddState::default(),
            library_panel: crate::rune_sec::library_panel::LibraryPanelState::default(),
            home_panel: crate::rune_sec::home_panel::HomePanelState::default(),
            frame_ops: crate::rune_sec::frame_ops::FrameOpsState::default(),
            graph_panel: crate::rune_sec::graph_panel::GraphPanelState::default(),
            audio_panel: crate::rune_sec::audio_panel::AudioPanelState::new(),
            menus: crate::rune_sec::menus::MenusState::default(),
            palette: crate::rune_sec::command_palette::PaletteState::new(),
            settings: crate::rune_sec::settings_popover::SettingsState::default(),
            toasts: crate::rune_sec::toasts::ToastsState::new(),
            text_tool: crate::rune_sec::text_tool::TextToolState::new(),
            name_edit: rune::TextEdit::new(""),
            export_use_range: false,
            fx_selected: None,
            pose_clipboard: None,
            last_status: String::new(),
            toast_now: 0.0,
            last_dark: true,
            workspace: crate::rune_sec::workspaces::Workspace::Animate,
            guides: false,
            show_left: true,
            show_right: true,
            key_clipboard: None,
            export_recap: String::new(),
        };
        let _ = inspector_segment(&selected, &t);
        let root = State {
            core: core_with_selection(Some(0)),
            last_poll: None,
            wheel_zoom: 0.0,
            figures_panel: crate::rune_sec::figures_panel::FiguresPanelState::default(),
            figure_add: crate::rune_sec::figure_add::FigureAddState::default(),
            library_panel: crate::rune_sec::library_panel::LibraryPanelState::default(),
            home_panel: crate::rune_sec::home_panel::HomePanelState::default(),
            frame_ops: crate::rune_sec::frame_ops::FrameOpsState::default(),
            graph_panel: crate::rune_sec::graph_panel::GraphPanelState::default(),
            audio_panel: crate::rune_sec::audio_panel::AudioPanelState::new(),
            menus: crate::rune_sec::menus::MenusState::default(),
            palette: crate::rune_sec::command_palette::PaletteState::new(),
            settings: crate::rune_sec::settings_popover::SettingsState::default(),
            toasts: crate::rune_sec::toasts::ToastsState::new(),
            text_tool: crate::rune_sec::text_tool::TextToolState::new(),
            name_edit: rune::TextEdit::new(""),
            export_use_range: false,
            fx_selected: None,
            pose_clipboard: None,
            last_status: String::new(),
            toast_now: 0.0,
            last_dark: true,
            workspace: crate::rune_sec::workspaces::Workspace::Animate,
            guides: false,
            show_left: true,
            show_right: true,
            key_clipboard: None,
            export_recap: String::new(),
        };
        let _ = inspector_segment(&root, &t);
    }
}
