//! Editor state and operations, independent of GPUI: selection,
//! dragging (root move / FK aim / two-bone IK), frame operations,
//! undo snapshots, save/load and export triggers. The GPUI layer in
//! `app.rs` only translates mouse events and buttons into these calls.

use crate::export::{export_gif as render_gif, export_png_sequence};
use crate::model::{
    aim_node, apply_ik, dist, evaluate, FigureDef, NodeDef, Project, V2,
};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DragKind {
    Root,
    Ik(usize),
    Aim(usize),
    Camera,
}

pub struct EditorCore {
    pub project: Project,
    pub current: usize,
    pub selected_figure: usize,
    pub selected_node: Option<usize>,
    pub dragging: Option<DragKind>,
    pub onion: bool,
    pub playing: bool,
    /// When true, canvas drags move the camera instead of posing.
    pub camera_mode: bool,
    camera_last: Option<V2>,
    pub status: String,
    undo: Vec<String>,
}

const FIGURE_COLORS: [u32; 6] = [0xE8E8EC, 0xFF5D5D, 0x5DB2FF, 0x7BE495, 0xFFC65D, 0xC792FF];

impl EditorCore {
    pub fn new() -> Self {
        EditorCore {
            project: Project::new_demo(),
            current: 0,
            selected_figure: 0,
            selected_node: None,
            dragging: None,
            onion: true,
            playing: false,
            camera_mode: false,
            camera_last: None,
            status: "Drag a joint to pose. Hands and feet use IK; drag the hips to move.".into(),
            undo: Vec::new(),
        }
    }

    pub fn frame_count(&self) -> usize {
        self.project.frames.len()
    }

    /// Joint positions of figure `fi` in frame `frame`.
    pub fn positions(&self, frame: usize, fi: usize) -> Vec<V2> {
        let fig = &self.project.figures[fi];
        evaluate(&fig.def, &self.project.frames[frame].poses[fi])
    }

    fn snapshot(&mut self) {
        if let Ok(s) = serde_json::to_string(&self.project) {
            self.undo.push(s);
            if self.undo.len() > 60 {
                self.undo.remove(0);
            }
        }
    }

    pub fn undo(&mut self) {
        if let Some(s) = self.undo.pop() {
            if let Ok(p) = serde_json::from_str::<Project>(&s) {
                self.project = p;
                self.current = self.current.min(self.project.frames.len().saturating_sub(1));
                self.selected_figure = self
                    .selected_figure
                    .min(self.project.figures.len().saturating_sub(1));
                self.status = "Undone.".into();
            }
        } else {
            self.status = "Nothing to undo.".into();
        }
    }

    /// Hit test in stage coordinates. Returns (figure, node).
    pub fn hit_test(&self, p: V2) -> Option<(usize, usize)> {
        let mut best: Option<(usize, usize, f32)> = None;
        for fi in 0..self.project.figures.len() {
            for (ni, joint) in self.positions(self.current, fi).iter().enumerate() {
                let d = dist(p, *joint);
                if d <= 13.0 && best.map(|(_, _, bd)| d < bd).unwrap_or(true) {
                    best = Some((fi, ni, d));
                }
            }
        }
        best.map(|(fi, ni, _)| (fi, ni))
    }

    pub fn begin_drag(&mut self, p: V2) -> bool {
        if self.playing {
            return false;
        }
        let Some((fi, ni)) = self.hit_test(p) else {
            return false;
        };
        self.snapshot();
        self.selected_figure = fi;
        self.selected_node = Some(ni);
        let fig = &self.project.figures[fi];
        let has_grandparent = fig.def.nodes[ni]
            .parent
            .and_then(|par| fig.def.nodes[par].parent)
            .is_some();
        let kind = if fig.def.nodes[ni].parent.is_none() {
            DragKind::Root
        } else if fig.def.nodes[ni].ik_end && has_grandparent {
            DragKind::Ik(ni)
        } else {
            DragKind::Aim(ni)
        };
        self.dragging = Some(kind);
        self.drag_to(p);
        true
    }

    /// Start a camera drag (camera mode): snapshots once, then
    /// `drag_to` pans the key at the current frame.
    pub fn begin_camera_drag(&mut self, p: V2) {
        self.snapshot();
        self.dragging = Some(DragKind::Camera);
        self.camera_last = Some(p);
    }

    pub fn drag_to(&mut self, p: V2) {
        let Some(kind) = self.dragging else { return };
        if let DragKind::Camera = kind {
            if let Some(last) = self.camera_last {
                let (x, y, zoom) = self.camera_here();
                self.project.set_camera_key(
                    self.current,
                    x - (p[0] - last[0]),
                    y - (p[1] - last[1]),
                    zoom,
                );
            }
            self.camera_last = Some(p);
            return;
        }
        let fi = self.selected_figure;
        let frame = self.current;
        let fig = &self.project.figures[fi];
        let pose = &mut self.project.frames[frame].poses[fi];
        match kind {
            DragKind::Camera => {} // handled above
            DragKind::Root => pose.root = p,
            DragKind::Ik(end) => {
                apply_ik(&fig.def, pose, end, p);
            }
            DragKind::Aim(node) => {
                aim_node(&fig.def, pose, node, p);
            }
        }
    }

    pub fn end_drag(&mut self) {
        self.dragging = None;
        self.camera_last = None;
    }

    pub fn add_frame(&mut self) {
        self.snapshot();
        let poses = self.project.frames[self.current].poses.clone();
        self.project.frames.insert(self.current + 1, crate::model::Frame { poses });
        self.current += 1;
        self.status = format!("Frame {} added.", self.current + 1);
    }

    pub fn duplicate_frame(&mut self) {
        self.add_frame();
        self.status = format!("Frame {} duplicated.", self.current + 1);
    }

    pub fn delete_frame(&mut self) {
        if self.project.frames.len() <= 1 {
            self.status = "Cannot delete the only frame.".into();
            return;
        }
        self.snapshot();
        self.project.frames.remove(self.current);
        if self.current >= self.project.frames.len() {
            self.current = self.project.frames.len() - 1;
        }
        self.status = "Frame deleted.".into();
    }

    pub fn add_figure(&mut self) {
        self.snapshot();
        let idx = self.project.figures.len();
        let color = FIGURE_COLORS[idx % FIGURE_COLORS.len()];
        let root = [300.0 + 90.0 * idx as f32, 470.0];
        self.project.add_figure(&format!("Styk {}", idx + 1), color, root);
        self.selected_figure = idx;
        self.status = format!("Figure {} added.", idx + 1);
    }

    /// Insert a tween frame halfway between the current frame and
    /// the next (the original's Insert Inbetween).
    pub fn insert_inbetween(&mut self) {
        if self.current + 1 >= self.project.frames.len() {
            self.status = "Inbetween needs a following frame.".into();
            return;
        }
        self.snapshot();
        let a = self.project.frames[self.current].poses.clone();
        let b = self.project.frames[self.current + 1].poses.clone();
        let poses: Vec<crate::model::Pose> = a
            .iter()
            .zip(b.iter())
            .map(|(pa, pb)| pa.lerp(pb, 0.5))
            .collect();
        self.project
            .frames
            .insert(self.current + 1, crate::model::Frame { poses });
        self.current += 1;
        self.status = format!("Inbetween inserted at frame {}.", self.current + 1);
    }

    // ---- Figure builder (definition edits, like the original) ----

    /// Append a segment to the selected node of the selected figure.
    pub fn add_segment(&mut self) {
        let fi = self.selected_figure;
        let def = &mut self.project.figures[fi].def;
        let parent = self
            .selected_node
            .filter(|n| *n < def.nodes.len())
            .unwrap_or(def.nodes.len() - 1);
        self.snapshot();
        let ni = self.project.figures[fi].def.nodes.len();
        self.project.figures[fi].def.nodes.push(NodeDef {
            name: format!("segment {ni}"),
            parent: Some(parent),
            length: 36.0,
            rest_angle: 0.0,
            head: false,
            bend: 1.0,
            thickness: 5.0,
            color: None,
            ik_end: false,
        });
        for frame in &mut self.project.frames {
            frame.poses[fi].angles.push(0.0);
        }
        self.selected_node = Some(ni);
        self.status = format!("Segment {ni} added. Drag it to pose.");
    }

    /// Delete the selected leaf segment. Root and segments that
    /// still have children are refused.
    pub fn delete_segment(&mut self) {
        let Some(ni) = self.selected_node else {
            self.status = "Select a segment first.".into();
            return;
        };
        let fi = self.selected_figure;
        let def = &self.project.figures[fi].def;
        if def.nodes[ni].parent.is_none() {
            self.status = "Cannot delete the root.".into();
            return;
        }
        if def.nodes.iter().any(|n| n.parent == Some(ni)) {
            self.status = "That segment has children; delete them first.".into();
            return;
        }
        self.snapshot();
        let def = &mut self.project.figures[fi].def;
        def.nodes.remove(ni);
        for node in def.nodes.iter_mut() {
            if let Some(par) = node.parent {
                if par > ni {
                    node.parent = Some(par - 1);
                }
            }
        }
        for frame in &mut self.project.frames {
            frame.poses[fi].angles.remove(ni);
        }
        self.selected_node = None;
        self.status = "Segment deleted.".into();
    }

    pub fn adjust_length(&mut self, delta: f32) {
        let Some(ni) = self.selected_node else { return };
        let node = &mut self.project.figures[self.selected_figure].def.nodes[ni];
        if node.parent.is_some() {
            node.length = (node.length + delta).clamp(4.0, 220.0);
            self.status = format!("{} length {:.0}", node.name, node.length);
        }
    }

    pub fn adjust_thickness(&mut self, delta: f32) {
        let Some(ni) = self.selected_node else { return };
        let node = &mut self.project.figures[self.selected_figure].def.nodes[ni];
        node.thickness = (node.thickness + delta).clamp(1.0, 18.0);
        self.status = format!("{} thickness {:.0}", node.name, node.thickness);
    }

    const NODE_COLORS: [Option<u32>; 7] = [
        None,
        Some(0xFF5D5D),
        Some(0x5DB2FF),
        Some(0x7BE495),
        Some(0xFFC65D),
        Some(0xC792FF),
        Some(0x232830),
    ];

    pub fn cycle_node_color(&mut self) {
        let Some(ni) = self.selected_node else { return };
        let node = &mut self.project.figures[self.selected_figure].def.nodes[ni];
        let idx = Self::NODE_COLORS
            .iter()
            .position(|c| *c == node.color)
            .unwrap_or(0);
        node.color = Self::NODE_COLORS[(idx + 1) % Self::NODE_COLORS.len()];
        self.status = format!("{} colour changed.", node.name);
    }

    // ---- Camera ----

    pub fn camera_here(&self) -> (f32, f32, f32) {
        self.project.camera_at(self.current as f32)
    }

    pub fn camera_zoom_by(&mut self, factor: f32) {
        self.snapshot();
        let (x, y, zoom) = self.camera_here();
        let zoom = (zoom * factor).clamp(0.3, 4.0);
        self.project.set_camera_key(self.current, x, y, zoom);
        self.status = format!("Camera zoom {zoom:.2} at frame {}.", self.current + 1);
    }

    pub fn camera_clear_key(&mut self) {
        if self.project.remove_camera_key(self.current) {
            self.status = "Camera key removed.".into();
        } else {
            self.status = "No camera key on this frame.".into();
        }
    }

    pub fn add_quadruped(&mut self) {
        self.snapshot();
        let idx = self.project.figures.len();
        let color = FIGURE_COLORS[idx % FIGURE_COLORS.len()];
        let root = [260.0 + 60.0 * idx as f32, 430.0];
        self.project
            .add_figure_with(FigureDef::quadruped(), &format!("Quad {}", idx + 1), color, root);
        self.selected_figure = idx;
        self.selected_node = None;
        self.status = "Quadruped added.".into();
    }

    pub fn add_simple(&mut self) {
        self.snapshot();
        let idx = self.project.figures.len();
        let color = FIGURE_COLORS[(idx + 2) % FIGURE_COLORS.len()];
        let root = [340.0 + 70.0 * idx as f32, 470.0];
        self.project
            .add_figure_with(FigureDef::simple(), &format!("Mini {}", idx + 1), color, root);
        self.selected_figure = idx;
        self.selected_node = None;
        self.status = "Mini figure added.".into();
    }

    pub fn goto(&mut self, frame: usize) {
        if frame < self.project.frames.len() {
            self.current = frame;
        }
    }

    pub fn advance(&mut self) {
        if !self.project.frames.is_empty() {
            self.current = (self.current + 1) % self.project.frames.len();
        }
    }

    pub fn save_path() -> PathBuf {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("project.styk.json")
    }

    pub fn save(&mut self) {
        let path = Self::save_path();
        match serde_json::to_string_pretty(&self.project)
            .map_err(std::io::Error::other)
            .and_then(|s| std::fs::write(&path, s))
        {
            Ok(()) => self.status = format!("Saved to {}", path.display()),
            Err(e) => self.status = format!("Save failed: {e}"),
        }
    }

    pub fn load(&mut self) {
        let path = Self::save_path();
        match std::fs::read_to_string(&path)
            .map_err(std::io::Error::other)
            .and_then(|s| serde_json::from_str::<Project>(&s).map_err(std::io::Error::other))
        {
            Ok(p) => {
                self.project = p;
                self.current = 0;
                self.selected_figure = 0;
                self.selected_node = None;
                self.undo.clear();
                self.status = format!("Loaded {}", path.display());
            }
            Err(e) => self.status = format!("Load failed: {e}"),
        }
    }

    pub fn export_gif(&mut self) {
        let path = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("export")
            .join("animation.gif");
        match render_gif(&self.project, &path) {
            Ok(n) => self.status = format!("Exported {n}-frame GIF to {}", path.display()),
            Err(e) => self.status = format!("GIF export failed: {e}"),
        }
    }

    pub fn export_png(&mut self) {
        let dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("export");
        match export_png_sequence(&self.project, &dir) {
            Ok(n) => self.status = format!("Exported {n} PNG frames to {}", dir.display()),
            Err(e) => self.status = format!("Export failed: {e}"),
        }
    }
}

impl Default for EditorCore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_add_and_delete_segment_stays_consistent() {
        let mut core = EditorCore::new();
        core.selected_node = Some(2); // head: has no children after we add one
        let nodes_before = core.project.figures[0].def.nodes.len();
        core.add_segment();
        assert_eq!(core.project.figures[0].def.nodes.len(), nodes_before + 1);
        for frame in &core.project.frames {
            assert_eq!(frame.poses[0].angles.len(), nodes_before + 1);
        }
        // The new node is selected and is a leaf: deletion succeeds.
        core.delete_segment();
        assert_eq!(core.project.figures[0].def.nodes.len(), nodes_before);
        for frame in &core.project.frames {
            assert_eq!(frame.poses[0].angles.len(), nodes_before);
        }
        // A node with children refuses deletion.
        core.selected_node = Some(1); // chest has children
        core.delete_segment();
        assert_eq!(core.project.figures[0].def.nodes.len(), nodes_before);
    }

    #[test]
    fn inbetween_lands_halfway() {
        let mut core = EditorCore::new();
        core.goto(0);
        core.insert_inbetween();
        assert_eq!(core.frame_count(), 6);
        // Demo frames 0 and 1 have root_x 480 and 494: midpoint 487.
        let root = core.project.frames[1].poses[0].root;
        assert!((root[0] - 487.0).abs() < 0.01, "root {root:?}");
    }
}
